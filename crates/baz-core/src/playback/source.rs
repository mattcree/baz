//! [`AudioSource`]: a format-agnostic streaming decoder built on Symphonia.
//!
//! Every source yields interleaved **stereo** f32 blocks regardless of the
//! container or codec (WAV f32, WAV i16, FLAC all take the same path), so
//! bit-exactness comparisons between engine output and reference single-file
//! decodes are apples-to-apples. Mono is upmixed by duplication; more than
//! two channels is rejected until proper downmix lands (see
//! [`PlaybackError::UnsupportedChannelCount`]).
//!
//! # Gapless trim
//!
//! The decoder honors the trim metadata Symphonia exposes on the codec
//! parameters: `delay` frames are skipped at the start and emission stops
//! after `n_frames` frames, which removes container/codec padding. For WAV
//! and FLAC these counts are exact and the result is verified bit-for-bit in
//! the integration tests. MP3/AAC encoder delay/padding handling is future
//! work and those codecs are not yet enabled (see the module docs in
//! [`crate::playback`]).

use std::fs::File;
use std::io::Cursor;
use std::path::Path;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{Decoder, DecoderOptions};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::{FormatOptions, FormatReader};
use symphonia::core::io::{MediaSource, MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use super::{CHANNELS, PlaybackError};

/// A fully decoded track: interleaved stereo f32 samples plus its native
/// sample rate. Produced by [`AudioSource::decode_all`], consumed by the
/// prefetch side of the engine.
#[derive(Debug, Clone)]
pub struct DecodedAudio {
    /// Interleaved stereo samples ([`CHANNELS`] per frame).
    pub samples: Vec<f32>,
    /// Native sample rate of the source in Hz.
    pub sample_rate: u32,
}

impl DecodedAudio {
    /// Number of stereo frames.
    #[must_use]
    pub fn frames(&self) -> usize {
        self.samples.len() / CHANNELS
    }
}

/// Streaming decoder for one audio file, yielding interleaved stereo f32.
pub struct AudioSource {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn Decoder>,
    track_id: u32,
    sample_rate: u32,
    /// Channel count of the *source* (1 or 2); output is always stereo.
    source_channels: usize,
    /// Reusable native-interleaved decode buffer.
    sample_buf: Option<SampleBuffer<f32>>,
    /// Reusable stereo output block returned by [`Self::next_block`].
    block: Vec<f32>,
    /// Frames decoded so far, before trimming.
    frames_seen: u64,
    /// First frame to emit (codec `delay` trim).
    trim_start: u64,
    /// One past the last frame to emit (`delay + n_frames`), if known.
    trim_end: Option<u64>,
}

impl AudioSource {
    /// Open and probe a file, ready to decode its default track.
    ///
    /// # Errors
    ///
    /// [`PlaybackError::Io`] if the file cannot be opened,
    /// [`PlaybackError::Decode`] if probing fails, and the
    /// `NoDefaultTrack`/`UnknownSampleRate`/`UnknownChannelLayout`/
    /// `UnsupportedChannelCount` variants when the stream is missing or
    /// exceeds what the engine supports.
    pub fn open(path: &Path) -> Result<Self, PlaybackError> {
        let file = File::open(path)?;
        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }
        Self::from_media_source(Box::new(file), &hint)
    }

    /// Open a stream from an in-memory buffer (no extension hint).
    ///
    /// Exists so hostile-input handling can be exercised without touching the
    /// filesystem — this is the entry point the decode fuzz target drives.
    ///
    /// # Errors
    ///
    /// Same as [`Self::open`], minus the file-open I/O error.
    pub fn open_bytes(data: Vec<u8>) -> Result<Self, PlaybackError> {
        Self::from_media_source(Box::new(Cursor::new(data)), &Hint::new())
    }

    fn from_media_source(source: Box<dyn MediaSource>, hint: &Hint) -> Result<Self, PlaybackError> {
        let mss = MediaSourceStream::new(source, MediaSourceStreamOptions::default());
        let probed = symphonia::default::get_probe().format(
            hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )?;
        let format = probed.format;
        let track = format
            .default_track()
            .ok_or(PlaybackError::NoDefaultTrack)?;
        let track_id = track.id;
        let params = &track.codec_params;
        let sample_rate = params.sample_rate.ok_or(PlaybackError::UnknownSampleRate)?;
        let source_channels = params
            .channels
            .ok_or(PlaybackError::UnknownChannelLayout)?
            .count();
        if source_channels == 0 || source_channels > CHANNELS {
            return Err(PlaybackError::UnsupportedChannelCount {
                channels: source_channels,
            });
        }
        // Gapless trim window: skip `delay` frames, stop after `n_frames`.
        // Exact for WAV/FLAC (verified in tests); lossy codecs are not yet
        // enabled (module docs).
        let trim_start = u64::from(params.delay.unwrap_or(0));
        let trim_end = params.n_frames.map(|n| trim_start + n);
        let decoder = symphonia::default::get_codecs().make(params, &DecoderOptions::default())?;
        Ok(Self {
            format,
            decoder,
            track_id,
            sample_rate,
            source_channels,
            sample_buf: None,
            block: Vec::new(),
            frames_seen: 0,
            trim_start,
            trim_end,
        })
    }

    /// Native sample rate of the stream in Hz.
    #[must_use]
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Decode the next block; `Ok(None)` at end of stream (or once the
    /// gapless trim window is exhausted). The returned slice is interleaved
    /// stereo f32 and valid until the next call.
    ///
    /// # Errors
    ///
    /// [`PlaybackError::Decode`] on any mid-stream demux or decode failure
    /// other than a clean end of stream.
    pub fn next_block(&mut self) -> Result<Option<&[f32]>, PlaybackError> {
        loop {
            if let Some(end) = self.trim_end
                && self.frames_seen >= end
            {
                return Ok(None);
            }
            let packet = match self.format.next_packet() {
                Ok(p) => p,
                Err(SymphoniaError::IoError(e))
                    if e.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    return Ok(None);
                }
                Err(e) => return Err(e.into()),
            };
            if packet.track_id() != self.track_id {
                continue;
            }
            let decoded = self.decoder.decode(&packet)?;
            let frames = decoded.frames() as u64;
            if frames == 0 {
                continue;
            }

            // Intersect this packet's frame span with the trim window.
            let span_start = self.frames_seen;
            self.frames_seen += frames;
            let keep_from = self.trim_start.max(span_start);
            let keep_to = self.trim_end.unwrap_or(u64::MAX).min(self.frames_seen);
            if keep_from >= keep_to {
                continue; // entirely delay or padding
            }
            // In-packet frame offsets; a packet is far smaller than usize.
            #[allow(clippy::cast_possible_truncation)] // packet-local counts fit usize
            let (skip, take) = (
                (keep_from - span_start) as usize,
                (keep_to - keep_from) as usize,
            );

            // Copy out of the decoder into native interleaved f32, reusing
            // the buffer (steady-state allocation-free on the decode thread).
            let spec = *decoded.spec();
            let capacity = decoded.capacity() as u64;
            let sample_buf = match &mut self.sample_buf {
                Some(b) if b.capacity() >= decoded.capacity() * self.source_channels => b,
                slot => slot.insert(SampleBuffer::new(capacity, spec)),
            };
            sample_buf.copy_interleaved_ref(decoded);
            let native = sample_buf.samples();

            // Trim and normalize to stereo into the reusable output block.
            self.block.clear();
            self.block.reserve(take * CHANNELS);
            match self.source_channels {
                1 => {
                    for &s in &native[skip..skip + take] {
                        self.block.push(s);
                        self.block.push(s);
                    }
                }
                _ => {
                    self.block
                        .extend_from_slice(&native[skip * CHANNELS..(skip + take) * CHANNELS]);
                }
            }
            return Ok(Some(&self.block));
        }
    }

    /// Decode a whole file into memory (prefetch path — never the realtime
    /// thread).
    ///
    /// # Errors
    ///
    /// Same as [`Self::open`] and [`Self::next_block`].
    pub fn decode_all(path: &Path) -> Result<DecodedAudio, PlaybackError> {
        let mut src = Self::open(path)?;
        let mut samples = Vec::new();
        while let Some(block) = src.next_block()? {
            samples.extend_from_slice(block);
        }
        Ok(DecodedAudio {
            samples,
            sample_rate: src.sample_rate,
        })
    }
}
