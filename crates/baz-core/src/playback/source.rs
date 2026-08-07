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
//!
//! # Seeking
//!
//! [`AudioSource::seek`] is sample-accurate and shares the trim timeline
//! above. `FormatReader::seek` in [`SeekMode::Accurate`] positions the reader
//! at a *packet boundary at or before* the requested frame and reports both
//! numbers (`required_ts`, `actual_ts`); all three enabled formats agree on
//! that contract, and for MP3 both are already delay-shifted so the seek
//! timeline is the same post-trim timeline `next_block` emits. The residue —
//! `required_ts - actual_ts` frames of the first packet(s), which for MP3
//! also covers the reference frames the demuxer rewinds to so the decoder's
//! bit reservoir is warm — is discarded by [`AudioSource::next_block`] on the
//! way out. Those discarded frames still count toward the `n_frames`
//! emission cap, which keeps the cap an absolute position in the stream
//! rather than a per-seek allowance.

use std::fs::File;
use std::io::Cursor;
use std::path::Path;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{Decoder, DecoderOptions};
use symphonia::core::errors::{Error as SymphoniaError, SeekErrorKind};
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo};
use symphonia::core::io::{MediaSource, MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::units::Time;

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
    /// Frames still to be discarded from the front of the decoder's output
    /// after a [`Self::seek`] (module docs). Zero on the un-seeked path, so
    /// steady-state decoding is byte-for-byte what it always was.
    skip_frames: u64,
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
            skip_frames: 0,
        })
    }

    /// Native sample rate of the stream in Hz.
    #[must_use]
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Total frames the container declares for this stream, post gapless
    /// trim; `None` when the header declares no length (an MP3 without a
    /// Xing/Info header). This is the same number that caps emission, so it
    /// is exactly the length [`Self::next_block`] will yield.
    #[must_use]
    pub fn total_frames(&self) -> Option<u64> {
        self.emit_cap
    }

    /// Declared playing time in whole milliseconds, when [`Self::total_frames`]
    /// is known. Rounded to nearest: a track is reported as the millisecond
    /// closest to its true length, never systematically short.
    #[must_use]
    pub fn duration_ms(&self) -> Option<u64> {
        self.emit_cap
            .map(|frames| frames_to_ms(frames, self.sample_rate))
    }

    /// Seek so that the next block decoded starts at `position_ms` from the
    /// beginning of the stream, sample-accurately (module docs).
    ///
    /// Positions are in the same post-gapless-trim timeline the source
    /// emits, so `seek(0)` returns to the first emitted frame, not to the
    /// encoder delay. The source stays usable either way: a failed seek
    /// leaves the reader wherever the format reader left it, and callers are
    /// expected to abandon the source rather than continue from an
    /// unspecified position.
    ///
    /// # Errors
    ///
    /// [`PlaybackError::SeekPastEnd`] when the target is at or beyond the
    /// declared length — checked here first so the answer does not depend on
    /// which format reader happens to range-check its input — and
    /// [`PlaybackError::Decode`] for a stream that cannot be seeked at all
    /// (an unseekable media source, a container without the index it needs).
    pub fn seek(&mut self, position_ms: u64) -> Result<(), PlaybackError> {
        let target = ms_to_frames(position_ms, self.sample_rate);
        if let Some(total) = self.emit_cap
            && target >= total
        {
            return Err(PlaybackError::SeekPastEnd {
                position_ms,
                track_ms: self.duration_ms(),
            });
        }
        let seeked = match self.format.seek(
            SeekMode::Accurate,
            SeekTo::Time {
                time: Time::from(ms_to_secs(position_ms)),
                track_id: Some(self.track_id),
            },
        ) {
            Ok(seeked) => seeked,
            Err(SymphoniaError::SeekError(SeekErrorKind::OutOfRange)) => {
                // A reader that range-checks more strictly than our declared
                // length does (or a stream with no declared length at all).
                return Err(PlaybackError::SeekPastEnd {
                    position_ms,
                    track_ms: self.duration_ms(),
                });
            }
            Err(e) => return Err(e.into()),
        };
        // The decoder's state (and, for MP3, its bit reservoir) belongs to
        // the old position.
        self.decoder.reset();
        self.frames_seen = seeked.actual_ts;
        self.skip_frames = seeked.required_ts.saturating_sub(seeked.actual_ts);
        Ok(())
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

            // Post-seek residue: frames decoded before the requested
            // position (module docs). Always 0 without a seek, so the
            // arithmetic below reduces to what it was.
            let skip = self.skip_frames.min(frames);
            self.skip_frames -= skip;

            // Cap emission at the declared frame count (no-op for
            // well-formed files; stops overrun past a lying header). Skipped
            // frames consume the budget too — the cap is an absolute
            // position in the stream, not a per-seek allowance.
            let remaining = self
                .emit_cap
                .map_or(u64::MAX, |cap| cap.saturating_sub(self.frames_seen));
            self.frames_seen += frames;
            // A packet is far smaller than usize.
            #[allow(clippy::cast_possible_truncation)] // packet-local counts fit usize
            let take = (frames - skip).min(remaining.saturating_sub(skip)) as usize;
            #[allow(clippy::cast_possible_truncation)] // ditto
            let skip = skip as usize;
            if take == 0 {
                continue; // wholly-skipped packet
            }

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

/// Whole-millisecond playing time of `frames` at `rate` Hz, rounded to
/// nearest. Integer arithmetic throughout: `u64` holds `frames * 1000` for
/// any track length that fits in a filesystem (10⁹ frames — six hours at
/// 48 kHz — is 10¹², eight orders of magnitude below `u64::MAX`).
pub(crate) fn frames_to_ms(frames: u64, rate: u32) -> u64 {
    let rate = u64::from(rate.max(1));
    (frames * 1000 + rate / 2) / rate
}

/// The inverse of [`frames_to_ms`], truncating: a seek asks for "at or after
/// this instant", so landing on the frame *before* the requested millisecond
/// would overshoot backwards past it.
pub(crate) fn ms_to_frames(ms: u64, rate: u32) -> u64 {
    ms * u64::from(rate) / 1000
}

/// Milliseconds as fractional seconds, for the format readers' `Time` input.
fn ms_to_secs(ms: u64) -> f64 {
    // Track positions are far below f64's exact-integer range.
    #[allow(clippy::cast_precision_loss)]
    let secs = ms as f64 / 1000.0;
    secs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_millisecond_conversions_round_trip() {
        // Exact at rate boundaries.
        assert_eq!(frames_to_ms(44_100, 44_100), 1000);
        assert_eq!(ms_to_frames(1000, 44_100), 44_100);
        assert_eq!(frames_to_ms(0, 44_100), 0);
        assert_eq!(ms_to_frames(0, 44_100), 0);
        // Rounds to nearest rather than always down: 22 frames at 44.1 kHz
        // is 0.4989 ms, 23 frames is 0.5215 ms.
        assert_eq!(frames_to_ms(22, 44_100), 0);
        assert_eq!(frames_to_ms(23, 44_100), 1);
        // A seek truncates so it never lands before the requested instant.
        assert_eq!(ms_to_frames(1, 44_100), 44);
        // Long tracks stay exact.
        assert_eq!(frames_to_ms(48_000 * 3600, 48_000), 3_600_000);
    }

    #[test]
    fn a_zero_rate_cannot_divide_by_zero() {
        // Unreachable in practice (`UnknownSampleRate` is rejected at open),
        // but the helper must be total rather than a latent panic.
        assert_eq!(frames_to_ms(1000, 0), 1_000_000);
    }
}
