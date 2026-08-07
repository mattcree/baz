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
//! - **Vorbis in Ogg**: Symphonia's Ogg reader *is* gapless-capable, and it
//!   has exact numbers to work from — every Ogg page carries an absolute
//!   granule position, which for Vorbis is a PCM sample count. It compares
//!   the first page's granule position against the duration of the packets on
//!   that page; the shortfall is the start delay (the lapped block that
//!   cannot be rendered yet). The last page's granule position gives the end
//!   trim the same way. Both are applied to the packets before we see them,
//!   and `n_frames` is the exact post-trim length. Verified against
//!   synthesized ground truth in `tests/playback.rs`: an encode of an
//!   N-frame WAV decodes to exactly N frames.
//! - **ALAC in MP4**: nothing to trim. `n_frames` (the `mdhd` media
//!   duration) is the exact stream length and the codec is lossless, so
//!   decode is bit-exact and the cap below is a no-op.
//! - **AAC in MP4**: Symphonia's ISO-MP4 reader is *not* gapless-capable and
//!   ignores `enable_gapless`. It applies neither the edit list nor
//!   `iTunSMPB`, so the encoder's priming frames arrive as ordinary audio;
//!   `n_frames` counts them. Measured excess over the source: 1024 frames
//!   (23.2 ms at 44.1 kHz) with ffmpeg's native AAC encoder. See
//!   [`crate::playback`] for the full statement and the tests that pin it.
//!
//! Because the reader/decoder pair already applies the trim when gapless is
//! enabled, this module must **not** apply `codec_params.delay` again (MP3
//! and Ogg both set it even though their packets arrive pre-trimmed;
//! re-applying it would double-trim). The only trim-related job left here is
//! capping emission at `n_frames`, which is a no-op for well-formed files and
//! stops overrun on streams that decode to more frames than their header
//! declared.
//!
//! # What MP4 does not declare
//!
//! WAV, FLAC and MP3 describe their audio fully in the container header. MP4
//! does not, and two gaps have to be closed before the first block is
//! emitted:
//!
//! - **Channel layout** is absent for AAC and ALAC — it lives in the codec's
//!   own setup data (the `AudioSpecificConfig`, the ALAC magic cookie), which
//!   only the decoder parses.
//! - **Sample rate** is present but can be wrong for our purposes: an HE-AAC
//!   sample entry advertises the SBR *output* rate, while Symphonia 0.5
//!   implements no SBR and hands back the AAC-LC core at half of it.
//!
//! Both are settled at open time by `AudioSource::probe_first_packet`, which
//! decodes the first packet that yields audio and reads the answers off the
//! decoder's own buffer. The probed frames are kept, not discarded, so the
//! probe costs no audio and needs no rewind. When the decoder's rate
//! disagrees with the container's, the decoder wins and the emission cap is
//! rescaled through the same ratio — the cap arrives as a count in the
//! container's timeline, and time is what the two timelines agree on.
//!
//! One more MP4 wrinkle is settled next to it: an MP4 lists *every* track, so
//! `default_track` (the container's first) is the video track in a `.mp4`.
//! [`AudioSource::open`] falls back to the first track that declares a sample
//! rate, because `AUDIO_EXTENSIONS` accepts `.mp4` and a file the shelf lists
//! has to play.
//!
//! # Seeking
//!
//! [`AudioSource::seek`] is sample-accurate for every format but Vorbis (see
//! the note at the end of this section) and shares the trim timeline
//! above. `FormatReader::seek` in [`SeekMode::Accurate`] positions the reader
//! at a *packet boundary at or before* the requested frame and reports both
//! numbers (`required_ts`, `actual_ts`); every enabled format agrees on
//! that contract, and for MP3 both are already delay-shifted so the seek
//! timeline is the same post-trim timeline `next_block` emits. The residue —
//! `required_ts - actual_ts` frames of the first packet(s), which for MP3
//! also covers the reference frames the demuxer rewinds to so the decoder's
//! bit reservoir is warm — is discarded by [`AudioSource::next_block`] on the
//! way out. Those discarded frames still count toward the `n_frames`
//! emission cap, which keeps the cap an absolute position in the stream
//! rather than a per-seek allowance.
//!
//! **Vorbis is the exception, and it is the decoder's, not the reader's.**
//! Symphonia's Vorbis decoder cannot render a block until it has the next
//! one to overlap-add with, so the first packet after [`Decoder::reset`]
//! returns an empty buffer — and that packet's audio is simply gone. The
//! reader's `required_ts`/`actual_ts` are unaffected and the residue skip
//! above still runs, so the net effect is a constant one-lapped-block
//! offset: measured at **1024 frames (23.2 ms at 44.1 kHz)** of content
//! delay, with the same 1024 frames missing from the remaining length, at
//! every seek target tested. It is not corrected here because the frames are
//! not recoverable at this point — the only fix is to seek earlier than
//! asked and re-derive the skip from the packet timestamps, which changes
//! the seek path all five other formats currently get right. Left as a
//! measured, tested and backlogged limitation rather than a silent one; see
//! `seek_into_vorbis_ogg_costs_one_lapped_block`.

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
    /// Bit depth the source declared, when it declared one
    /// ([`AudioSource::bits_per_sample`]). Carried for the signal-path
    /// readout; the samples themselves are f32 either way.
    pub bits_per_sample: Option<u32>,
}

impl DecodedAudio {
    /// Number of stereo frames.
    #[must_use]
    pub fn frames(&self) -> usize {
        self.samples.len() / CHANNELS
    }
}

/// What a decoded packet says about the stream — the answers a container may
/// have withheld (see [`AudioSource::probe_first_packet`]).
#[derive(Debug, Clone, Copy)]
struct ProbedSpec {
    /// Channels in the decoder's output buffer.
    channels: usize,
    /// Sample rate of the decoder's output buffer, in Hz.
    rate: u32,
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
    /// Frames sitting in [`Self::sample_buf`] that have been decoded but not
    /// yet emitted. Non-zero only between the open-time channel probe (see
    /// [`Self::from_media_source`]) and the first [`Self::next_block`].
    pending_frames: u64,
    /// Reusable stereo output block returned by [`Self::next_block`].
    block: Vec<f32>,
    /// Frames received from the decoder so far (post Symphonia's own gapless
    /// trim — see the module docs).
    frames_seen: u64,
    /// Emission cap: `codec_params.n_frames` if the header declares it. With
    /// gapless enabled this is the post-trim count for MP3 and the exact
    /// stream length for WAV/FLAC/ALAC; for AAC in MP4 it is the container's
    /// media duration, which counts the untrimmed encoder delay (module
    /// docs). In every case it is the count this source will actually emit,
    /// which is what makes it usable as a duration.
    emit_cap: Option<u64>,
    /// Frames still to be discarded from the front of the decoder's output
    /// after a [`Self::seek`] (module docs). Zero on the un-seeked path, so
    /// steady-state decoding is byte-for-byte what it always was.
    skip_frames: u64,
    /// Bit depth the container declares, when it declares one. Reported, never
    /// acted on: decoding is to f32 regardless (see [`Self::bits_per_sample`]).
    bits_per_sample: Option<u32>,
}

impl AudioSource {
    /// Open and probe a file, ready to decode its audio track.
    ///
    /// Normally that is the container's default track; in a multi-track
    /// container (a `.mp4`, where the video track comes first) it is the
    /// first track that declares a sample rate. For MP4 this also decodes the
    /// first audio packet, to learn the channel layout and true sample rate
    /// the container leaves out — see the module docs. No audio is lost to
    /// that probe.
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
        // Pick the track to play. Symphonia's `default_track` is the
        // container's first, which is right for every single-stream audio
        // file — but an MP4 lists *all* its tracks and a `.mp4` puts the
        // video track first. The library scans `.mp4`/`.m4a` by extension, so
        // "the first track declares no sample rate" cannot be the end of the
        // story: fall back to the first track that does declare one, i.e. the
        // audio. A container with tracks but no audio among them is
        // `UnknownSampleRate`; one with no tracks at all, `NoDefaultTrack`.
        let track = match format.default_track() {
            Some(t) if t.codec_params.sample_rate.is_some() => t,
            Some(_) => format
                .tracks()
                .iter()
                .find(|t| t.codec_params.sample_rate.is_some())
                .ok_or(PlaybackError::UnknownSampleRate)?,
            None => return Err(PlaybackError::NoDefaultTrack),
        };
        let track_id = track.id;
        let params = &track.codec_params;
        let sample_rate = params.sample_rate.ok_or(PlaybackError::UnknownSampleRate)?;
        // WAV/FLAC/MP3 declare the layout in the container header. MP4 does
        // not for AAC or ALAC: the layout lives in the codec's own setup data
        // (the AudioSpecificConfig / ALAC magic cookie), which only the
        // decoder parses — so `channels` is `None` here and the probe below
        // asks the decoder instead. `None` from a format that *should* have
        // told us is not distinguishable at this point, so the probe is the
        // single answer for both cases.
        let declared_channels = params.channels.map(symphonia::core::audio::Channels::count);
        // Emission cap only. Deliberately NOT `params.delay`: with gapless
        // enabled the reader/decoder pair has already trimmed delay and
        // padding out of the buffers we receive (and MP3 still reports
        // `delay` in its params, so applying it here would double-trim).
        // See the module docs.
        let emit_cap = params.n_frames;
        // Read for the signal-path readout only; decoding is to f32 whatever
        // the container says (see `Self::bits_per_sample`).
        let bits_per_sample = params.bits_per_sample;
        let decoder = symphonia::default::get_codecs().make(params, &DecoderOptions::default())?;
        let mut source = Self {
            format,
            decoder,
            track_id,
            sample_rate,
            // Provisional: overwritten by the probe below when the container
            // declared nothing. Never read before then.
            source_channels: declared_channels.unwrap_or(CHANNELS),
            sample_buf: None,
            pending_frames: 0,
            block: Vec::new(),
            frames_seen: 0,
            emit_cap,
            skip_frames: 0,
            bits_per_sample,
        };
        let channels = if let Some(n) = declared_channels {
            n
        } else {
            let probe = source.probe_first_packet()?;
            source.adopt_decoder_rate(probe.rate, sample_rate);
            probe.channels
        };
        if channels == 0 || channels > CHANNELS {
            return Err(PlaybackError::UnsupportedChannelCount { channels });
        }
        source.source_channels = channels;
        Ok(source)
    }

    /// Decode the first packet that yields audio, to learn what the container
    /// did not say (MP4).
    ///
    /// The decoded frames are *kept* — [`Self::decode_into_buf`] leaves them
    /// in [`Self::sample_buf`] and [`Self::pending_frames`] hands them to the
    /// first [`Self::next_block`] — so probing costs no audio and needs no
    /// rewind (which a non-seekable media source could not offer anyway).
    fn probe_first_packet(&mut self) -> Result<ProbedSpec, PlaybackError> {
        loop {
            match self.decode_into_buf()? {
                // A stream that ends before yielding a single frame has no
                // observable layout; report the container's omission rather
                // than guess one.
                None => return Err(PlaybackError::UnknownChannelLayout),
                // A packet the format's own trim consumed entirely: keep
                // looking.
                Some((0, _)) => {}
                Some((frames, spec)) => {
                    self.pending_frames = frames;
                    return Ok(spec);
                }
            }
        }
    }

    /// Reconcile the rate the decoder actually produces with the one the
    /// container advertised, when they disagree.
    ///
    /// They disagree for **HE-AAC** (SBR): the MP4 sample entry advertises
    /// the SBR *output* rate, Symphonia 0.5's AAC decoder implements no SBR
    /// and returns the AAC-LC core at half that rate. Believing the container
    /// would play the core band at twice its proper speed — an octave up. The
    /// decoder is the authority on the samples it hands us, so we take its
    /// rate and rescale the emission cap through the same ratio: the cap
    /// arrives as a count in the container's timeline (`mdhd` duration in the
    /// track timescale), and *time* is what the two timelines agree on.
    fn adopt_decoder_rate(&mut self, decoder_rate: u32, container_rate: u32) {
        if decoder_rate == 0 || decoder_rate == container_rate {
            return;
        }
        self.sample_rate = decoder_rate;
        self.emit_cap = self.emit_cap.map(|cap| {
            // u128 so the intermediate product cannot overflow for any
            // plausible length x rate.
            let scaled =
                u128::from(cap) * u128::from(decoder_rate) / u128::from(container_rate.max(1));
            u64::try_from(scaled).unwrap_or(u64::MAX)
        });
    }

    /// Native sample rate of the stream in Hz.
    #[must_use]
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Bit depth the container declares, when it declares one — 24 for the
    /// owner's 24-bit FLACs, 16 for a CD rip, `None` for float PCM and for
    /// containers that stay silent about it.
    ///
    /// Reported for the signal-path readout
    /// ([`Event::SignalPath`](crate::protocol::Event::SignalPath)), never
    /// acted on. Every source decodes to f32 whatever its depth, and f32's
    /// 24-bit mantissa represents 24-bit and narrower integer PCM exactly, so
    /// there is no depth at which the engine has to choose what to throw
    /// away.
    #[must_use]
    pub fn bits_per_sample(&self) -> Option<u32> {
        self.bits_per_sample
    }

    /// Total frames the container declares for this stream, post gapless
    /// trim; `None` when the header declares no length (an MP3 without a
    /// Xing/Info header). This is the same number that caps emission, so it
    /// is exactly the length [`Self::next_block`] will yield — including for
    /// AAC in MP4, where "what the file plays" is a little longer than "what
    /// was encoded" because the delay is not trimmed (module docs).
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
    /// beginning of the stream, sample-accurately — with one measured
    /// exception, Vorbis, which resumes one lapped block (1024 frames,
    /// 23.2 ms at 44.1 kHz) late because its decoder discards the first
    /// packet after a reset. Both are covered in the module docs.
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
        // the old position, and so does anything the open-time channel probe
        // left buffered.
        self.decoder.reset();
        self.pending_frames = 0;
        self.frames_seen = seeked.actual_ts;
        self.skip_frames = seeked.required_ts.saturating_sub(seeked.actual_ts);
        Ok(())
    }

    /// Demux and decode the next packet of our track into [`Self::sample_buf`]
    /// as native-interleaved f32, returning `(frames, spec)`; `Ok(None)` at
    /// end of stream.
    ///
    /// `frames` may be 0 for a packet the format's gapless trim consumed
    /// entirely — the caller loops. The buffer is reused across calls, so
    /// steady-state decoding stays allocation-free on the decode thread.
    fn decode_into_buf(&mut self) -> Result<Option<(u64, ProbedSpec)>, PlaybackError> {
        loop {
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
            let spec = *decoded.spec();
            let probed = ProbedSpec {
                channels: spec.channels.count(),
                rate: spec.rate,
            };
            if frames == 0 {
                return Ok(Some((0, probed)));
            }
            // Copy out of the decoder into native interleaved f32, reusing
            // the buffer.
            let capacity = decoded.capacity() as u64;
            let sample_buf = match &mut self.sample_buf {
                Some(b) if b.capacity() >= decoded.capacity() * probed.channels => b,
                slot => slot.insert(SampleBuffer::new(capacity, spec)),
            };
            sample_buf.copy_interleaved_ref(decoded);
            return Ok(Some((frames, probed)));
        }
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
            // Frames left over from the open-time channel probe come first;
            // after that (and always, for containers that declare a layout)
            // we pull a fresh packet.
            let frames = if self.pending_frames > 0 {
                std::mem::take(&mut self.pending_frames)
            } else {
                match self.decode_into_buf()? {
                    None => return Ok(None),
                    Some((frames, _)) => frames,
                }
            };
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

            // The decoded packet is already in `sample_buf` as native
            // interleaved f32 (`decode_into_buf`).
            let native = match &self.sample_buf {
                Some(b) => b.samples(),
                // Unreachable: a non-zero frame count means the buffer was
                // filled. Yielding nothing beats an unwrap on the decode path.
                None => return Ok(None),
            };

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
            bits_per_sample: src.bits_per_sample,
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
