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
//! Every source is probed with `FormatOptions::enable_gapless`, which makes
//! Symphonia's gapless-capable format readers do the encoder delay/padding
//! trim themselves: the reader shifts packet timestamps by the encoder
//! delay, stamps per-packet trim counts, and the codec applies those counts
//! to its decoded buffer before we ever see it. Concretely, per format:
//!
//! - **WAV/FLAC**: the container carries an exact sample count and no
//!   delay/padding exists; `n_frames` is the exact stream length and decode
//!   is bit-exact (verified bit-for-bit in the integration tests).
//! - **MP3**: when a Xing/Info header with a LAME extension is present
//!   (written by LAME, and by ffmpeg's `libmp3lame`), Symphonia reads the
//!   encoder delay and padding from it, adds the fixed 529-frame decoder
//!   delay, and trims both ends; `codec_params.n_frames` is then the
//!   **post-trim** count. The result is sample-count-exact: decoding an
//!   encode of an N-frame WAV yields exactly N frames (verified against
//!   synthesized ground truth in `tests/playback.rs`). An MP3 *without* a
//!   LAME tag carries no trim metadata anywhere in the bitstream; it decodes
//!   with its delay and padding intact — there is nothing to honestly trim
//!   by, so no heuristic trim is attempted.
//! - **AAC**: not enabled — see [`crate::playback`] module docs.
//!
//! Because the reader/decoder pair already applies the trim when gapless is
//! enabled, this module must **not** apply `codec_params.delay` again (MP3
//! sets it even though its packets arrive pre-trimmed; re-applying it would
//! double-trim). The only trim-related job left here is capping emission at
//! `n_frames`, which is a no-op for well-formed files and stops overrun on
//! streams that decode to more frames than their header declared.

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
    /// Frames received from the decoder so far (post Symphonia's own gapless
    /// trim — see the module docs).
    frames_seen: u64,
    /// Emission cap: `codec_params.n_frames` if the header declares it. With
    /// gapless enabled this is the post-trim count for MP3 and the exact
    /// stream length for WAV/FLAC; either way, exact.
    emit_cap: Option<u64>,
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
        // `enable_gapless` makes gapless-capable readers (MP3) shift packet
        // timestamps and stamp per-packet trim counts, which the decoder
        // applies to its output buffer; `n_frames` becomes the post-trim
        // count. WAV/FLAC ignore the flag (they have nothing to trim).
        let format_opts = FormatOptions {
            enable_gapless: true,
            ..FormatOptions::default()
        };
        let probed = symphonia::default::get_probe().format(
            hint,
            mss,
            &format_opts,
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
        // Emission cap only. Deliberately NOT `params.delay`: with gapless
        // enabled the reader/decoder pair has already trimmed delay and
        // padding out of the buffers we receive (and MP3 still reports
        // `delay` in its params, so applying it here would double-trim).
        // See the module docs.
        let emit_cap = params.n_frames;
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
            emit_cap,
        })
    }

    /// Native sample rate of the stream in Hz.
    #[must_use]
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Decode the next block; `Ok(None)` at end of stream (or once the
    /// declared frame count is exhausted). The returned slice is interleaved
    /// stereo f32 and valid until the next call.
    ///
    /// # Errors
    ///
    /// [`PlaybackError::Decode`] on any mid-stream demux or decode failure
    /// other than a clean end of stream.
    pub fn next_block(&mut self) -> Result<Option<&[f32]>, PlaybackError> {
        loop {
            if let Some(cap) = self.emit_cap
                && self.frames_seen >= cap
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
            // `decoded` is post-trim when the format did gapless trimming
            // (module docs): a fully-trimmed packet yields zero frames.
            let decoded = self.decoder.decode(&packet)?;
            let frames = decoded.frames() as u64;
            if frames == 0 {
                continue;
            }

            // Cap emission at the declared frame count (no-op for
            // well-formed files; stops overrun past a lying header).
            let remaining = self.emit_cap.map_or(u64::MAX, |cap| cap - self.frames_seen);
            self.frames_seen += frames;
            // A packet is far smaller than usize.
            #[allow(clippy::cast_possible_truncation)] // packet-local counts fit usize
            let take = frames.min(remaining) as usize;

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

            // Normalize to stereo into the reusable output block.
            self.block.clear();
            self.block.reserve(take * CHANNELS);
            match self.source_channels {
                1 => {
                    for &s in &native[..take] {
                        self.block.push(s);
                        self.block.push(s);
                    }
                }
                _ => {
                    self.block.extend_from_slice(&native[..take * CHANNELS]);
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
