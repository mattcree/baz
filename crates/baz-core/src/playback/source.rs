//! [`AudioSource`]: a format-agnostic streaming decoder built on Symphonia.
//!
//! Every source yields interleaved **stereo** f32 blocks regardless of the
//! container or codec (WAV f32, WAV i16, FLAC all take the same path), so
//! bit-exactness comparisons between engine output and reference single-file
//! decodes are apples-to-apples. Mono is upmixed by duplication; a
//! multichannel source is folded with the ITU-R BS.775 matrix
//! (`playback::downmix`, ADR-0039), and a layout that recommendation does not
//! place is refused with [`PlaybackError::UnsupportedChannelLayout`] rather
//! than folded on a guess.
//!
//! **The fold is built from the layout, never from the channel count.** Which
//! plane of a decoded packet holds the centre channel is a property of the
//! container and the codec, and WAVE, FLAC, Vorbis and ALAC do not agree; the
//! only thing they agree on is Symphonia's own contract, that the *n*-th plane
//! is the *n*-th speaker of `SignalSpec::channels` in ascending bit order.
//! [`AudioSource`] therefore keeps the `Channels` **set** the decoder reports
//! and hands it to the matrix, and a decoder whose plane order disagreed with
//! its own declared layout would fail
//! `each_speaker_lands_where_the_layout_says` rather than sound faintly wrong.
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
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;
use std::sync::OnceLock;

use symphonia::core::audio::{Channels, SampleBuffer};
use symphonia::core::codecs::{Decoder, DecoderOptions};
use symphonia::core::errors::{Error as SymphoniaError, SeekErrorKind};
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo};
use symphonia::core::io::{MediaSource, MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::{MetadataOptions, MetadataRevision};
use symphonia::core::probe::{Hint, Probe};
use symphonia::core::units::Time;
use symphonia::default::formats::{FlacReader, IsoMp4Reader, MpaReader, OggReader, WavReader};
use symphonia_metadata::id3v2::Id3v2Reader;

use super::downmix::Downmix;
use super::{CHANNELS, PlaybackError};
use crate::replaygain::{ReplayGainReader, ReplayGainTags, field_of_key};

/// The formats baz probes for — **exactly the ones `AUDIO_EXTENSIONS` names**,
/// and not one more.
///
/// This is `symphonia::default::register_enabled_formats` with a single
/// omission: **`AdtsReader`, the raw-ADTS demuxer.** The reasoning is
/// ADR-0040 §2.5, and it starts from what a probe *is*. `Probe::format`
/// identifies a stream by searching its bytes for a registered marker, not by
/// trusting the extension — so every registered reader is a parser that
/// arbitrary bytes can reach, whatever the file is called. ADTS's marker is a
/// twelve-bit sync word, which random data carries constantly.
///
/// baz has no use for the reader that marker selects. `.aac` is not an
/// [`AUDIO_EXTENSIONS`](crate::library::AUDIO_EXTENSIONS) member, so no raw
/// ADTS stream is ever *listed*, so none is ever played; every AAC baz decodes
/// arrives inside an MP4, through `IsoMp4Reader` and the AAC **decoder**,
/// which stays registered. In production the ADTS *reader* could therefore
/// only ever fire on a file that is not what its name says — and that is
/// precisely where it fired: a seven-minute fuzz sweep produced 650 crash
/// artifacts and **every one of them** was this reader, panicking on bytes
/// handed to it under a `.flac`, `.wav` or `.mp3` name
/// (`assertion failed: step != 0`, `attempt to subtract with overflow`).
///
/// **It was also competing for baz's own bytes.** MPEG audio's frame sync is
/// eleven set bits and ADTS's is twelve, so the two markers overlap and both
/// readers claimed the same corrupt `.mp3`. Removing the one baz cannot use
/// leaves such a file to `MpaReader`, which is the reader that should have had
/// it: of the three artifacts in `tests/hostile_media.rs`, two now come back
/// as `end of stream` from it and one as *no suitable format reader*. That
/// split is why the second phrase is asserted nowhere — which of the two a
/// given corrupt file gets is a property of symphonia's marker table, not a
/// promise baz makes.
///
/// What it costs is stated rather than hidden: a raw ADTS stream misnamed
/// `.m4a` used to play and now does not. It was never listed by the scanner,
/// so reaching it meant opening it by another route.
///
/// `Id3v2Reader` is registered because the default registry registers it and
/// baz depends on it — ReplayGain tags on an MP3 live in the ID3v2 block, and
/// `absorb_replay_gain` reads them off this probe's `metadata`.
fn probe() -> &'static Probe {
    static PROBE: OnceLock<Probe> = OnceLock::new();
    PROBE.get_or_init(|| {
        let mut probe = Probe::default();
        probe.register_all::<FlacReader>();
        probe.register_all::<MpaReader>();
        probe.register_all::<IsoMp4Reader>();
        probe.register_all::<OggReader>();
        probe.register_all::<WavReader>();
        probe.register_all::<Id3v2Reader>();
        probe
    })
}

/// Run one call into Symphonia, turning a **panic** inside it into
/// [`PlaybackError::DecoderPanicked`].
///
/// This is the boundary ADR-0040 draws. A file on a listener's disk is
/// arbitrary bytes, the parsers that read it are third-party, and a panic in
/// one of them is not a file that will not play — it is the decode thread
/// dying, which stops the music and leaves the engine with a track it can
/// neither finish nor abandon. Turning it into an error puts a hostile file
/// exactly where a merely unreadable one already is.
///
/// **What it does not cover, and cannot.** An oversized *allocation* is not an
/// unwind: `vec![0u8; n]` for an `n` the allocator refuses calls
/// `handle_alloc_error`, which aborts the process outright. That failure is
/// symphonia's to bound and ADR-0040 says why baz does not shadow it here.
///
/// **`panic = "abort"` defeats this**, so no profile in `Cargo.toml` may set
/// it; `unwinding_is_what_makes_the_containment_work` fails if one does.
///
/// The closure is asserted unwind-safe because nothing that a caught panic
/// leaves behind is ever read: [`AudioSource::open`] returns the error instead
/// of a source, and every caller of [`AudioSource::next_block`] and
/// [`AudioSource::seek`] abandons the source on an error — which
/// `seek`'s own documentation already required of them, for the ordinary
/// failure.
fn contain_panics<T>(
    doing: &'static str,
    call: impl FnOnce() -> Result<T, PlaybackError>,
) -> Result<T, PlaybackError> {
    match catch_unwind(AssertUnwindSafe(call)) {
        Ok(result) => result,
        // The payload is deliberately dropped rather than rendered into the
        // error: the panic's own message has already gone to stderr through
        // the standard hook, where a bug report can find it, and a decoder's
        // internal assertion text is not something to put in front of a
        // listener.
        Err(_) => Err(PlaybackError::DecoderPanicked { doing }),
    }
}

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
    /// Channels the *source* carried ([`AudioSource::channels`]). Carried for
    /// the signal-path readout, which is where a listener finds out that a
    /// BS.775 downmix is in the path; the samples here are stereo whatever it
    /// says.
    pub source_channels: usize,
    /// The ReplayGain figures the file's tags declared
    /// ([`AudioSource::replay_gain`]). Carried so the engine can apply the
    /// right gain from this track's very first delivered sample; the samples
    /// here are unscaled either way.
    pub replay_gain: ReplayGainTags,
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
    /// The speakers in the decoder's output buffer, as a set — **not** a
    /// count. The set is what fixes which plane is which (module docs), so it
    /// is what travels.
    channels: Channels,
    /// Sample rate of the decoder's output buffer, in Hz.
    rate: u32,
}

/// Streaming decoder for one audio file, yielding interleaved stereo f32.
pub struct AudioSource {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn Decoder>,
    track_id: u32,
    sample_rate: u32,
    /// Channel count of the *source*; output is always stereo.
    source_channels: usize,
    /// The BS.775 matrix for this file's layout, or `None` for the mono and
    /// stereo sources that need no fold. Built once at open, applied per
    /// packet, and stateless — so a seeked decode and a whole-file decode
    /// agree sample for sample.
    downmix: Option<Downmix>,
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
    /// ReplayGain as the file's tags declare it, read once at open from the
    /// metadata the probe already parsed (see [`Self::replay_gain`]).
    replay_gain: ReplayGainTags,
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
        contain_panics("opening", || Self::probe_media_source(source, hint))
    }

    /// The body of [`Self::from_media_source`], which wraps this in
    /// [`contain_panics`]. Everything a hostile header can reach at open time
    /// — the format probe, the demuxer's own metadata parsing, the codec
    /// registry and (for MP4) the first decoded packet — is inside here.
    fn probe_media_source(
        source: Box<dyn MediaSource>,
        hint: &Hint,
    ) -> Result<Self, PlaybackError> {
        let mss = MediaSourceStream::new(source, MediaSourceStreamOptions::default());
        // `enable_gapless` makes gapless-capable readers (MP3) shift packet
        // timestamps and stamp per-packet trim counts, which the decoder
        // applies to its output buffer; `n_frames` becomes the post-trim
        // count. WAV/FLAC ignore the flag (they have nothing to trim).
        let format_opts = FormatOptions {
            enable_gapless: true,
            ..FormatOptions::default()
        };
        let mut probed = probe().format(hint, mss, &format_opts, &MetadataOptions::default())?;
        // ReplayGain, from the metadata the probe has already parsed — no
        // extra open, no extra read, no second tag library. Two sources,
        // because containers put tags in two places: `probed.metadata` holds
        // what sat *outside* the container (an ID3v2 block ahead of an MP3 or
        // a FLAC stream), and `format.metadata()` holds the container's own
        // (Vorbis comments, MP4 atoms).
        let mut replay_gain = ReplayGainReader::default();
        if let Some(mut outside) = probed.metadata.get()
            && let Some(revision) = outside.skip_to_latest()
        {
            absorb_replay_gain(&mut replay_gain, revision);
        }
        let mut format = probed.format;
        if let Some(revision) = format.metadata().skip_to_latest() {
            absorb_replay_gain(&mut replay_gain, revision);
        }
        let replay_gain = replay_gain.finish();
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
        //
        // The *set* is kept, not its cardinality: it is what says which plane
        // of a decoded packet is the centre channel, and the downmix is built
        // from it (module docs).
        let declared_channels = params.channels;
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
            // Provisional: overwritten below once the layout is settled.
            // Never read before then.
            source_channels: CHANNELS,
            downmix: None,
            sample_buf: None,
            pending_frames: 0,
            block: Vec::new(),
            frames_seen: 0,
            emit_cap,
            skip_frames: 0,
            bits_per_sample,
            replay_gain,
        };
        let layout = if let Some(set) = declared_channels {
            set
        } else {
            let probe = source.probe_first_packet()?;
            source.adopt_decoder_rate(probe.rate, sample_rate);
            probe.channels
        };
        let channels = layout.count();
        if channels == 0 {
            return Err(PlaybackError::UnsupportedChannelCount { channels });
        }
        // `None` for mono and stereo, which the block loop handles as it
        // always has; `Some` for a layout BS.775 places; an error for one it
        // does not (ADR-0039). Built here rather than lazily so a file that
        // cannot be folded fails at `open`, where every other unplayable
        // file already fails.
        source.downmix = Downmix::for_layout(layout)?;
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

    /// Channels the **source** carries — 1, 2, or the 3 to 6 of a layout the
    /// downmix folds — as distinct from the stereo
    /// [`next_block`](Self::next_block) always emits.
    ///
    /// Two callers, for two different reasons.
    ///
    /// [`crate::analysis`] asks because a loudness measurement is not invariant
    /// under duplicating a mono channel. BS.1770 sums the channels with unity
    /// weights, so measuring a mono file through this decoder's stereo output
    /// reads 3.01 LU louder than every other scanner would read the same file —
    /// and a library where baz's mono tracks sat 3 dB from its tagged ones is
    /// the bug this accessor exists to prevent
    /// (`a_mono_source_is_measured_as_one_channel`). It clamps to [`CHANNELS`],
    /// so a **multichannel** file is measured as the stereo downmix baz will
    /// actually play — which is the right measurement for a gain baz will
    /// apply, and is deliberately *not* what a 5.1-aware scanner would report
    /// for the same file (`a_multichannel_source_is_measured_as_its_downmix`).
    ///
    /// The engine asks so that
    /// [`Event::SignalPath`](crate::protocol::Event::SignalPath) can say a
    /// downmix is in the path. Playback itself never asks: it wants stereo and
    /// gets it.
    #[must_use]
    pub fn channels(&self) -> usize {
        self.source_channels
    }

    /// The constant attenuation the downmix applies, in decibels, or `None`
    /// for a source that is not downmixed.
    ///
    /// −7.66 dB for 5.1, −4.65 dB for quadraphonic; the matrix's own worst-case
    /// gain, and the reason a 5.1 file plays quieter than the stereo master of
    /// the same record. See `playback::downmix` for why it is a
    /// constant rather than a limiter, and why ReplayGain gets the level back.
    #[must_use]
    pub fn downmix_headroom_db(&self) -> Option<f32> {
        self.downmix.as_ref().map(Downmix::headroom_db)
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

    /// The ReplayGain figures this file's tags declare, all `None` when it
    /// carries none (ADR-0013).
    ///
    /// Read at open from the metadata the probe already parsed, so it costs no
    /// I/O beyond the header read that was happening anyway — which is what
    /// makes it affordable on the decode path, and why the engine reads
    /// ReplayGain from the file it is about to play rather than from the
    /// library index. A queue of paths is all the engine is ever given, and a
    /// path that was never scanned still has to play at the right level.
    #[must_use]
    pub fn replay_gain(&self) -> ReplayGainTags {
        self.replay_gain
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
    /// (an unseekable media source, a container without the index it needs) —
    /// and [`PlaybackError::DecoderPanicked`] when a format reader's own seek
    /// arithmetic panics on a lying header (`contain_panics`).
    pub fn seek(&mut self, position_ms: u64) -> Result<(), PlaybackError> {
        contain_panics("seeking", || self.seek_inner(position_ms))
    }

    /// The body of [`Self::seek`], which wraps this in [`contain_panics`].
    fn seek_inner(&mut self, position_ms: u64) -> Result<(), PlaybackError> {
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
            // A codec-level `DecodeError` rejects this packet, not the whole
            // stream. Symphonia's own decoding example advances to the next
            // packet in this case: damaged padding, an unwarmed MP3 bit
            // reservoir or another isolated frame must cost that frame rather
            // than an otherwise playable track. Container/I/O/reset failures
            // still end the source because continuing cannot repair them.
            //
            // `decoded` is post-trim when the format did gapless trimming
            // (module docs): a fully-trimmed packet yields zero frames.
            let decoded = match self.decoder.decode(&packet) {
                Ok(decoded) => decoded,
                Err(error) if recoverable_packet_error(&error) => continue,
                Err(error) => return Err(error.into()),
            };
            let frames = decoded.frames() as u64;
            let spec = *decoded.spec();
            let probed = ProbedSpec {
                channels: spec.channels,
                rate: spec.rate,
            };
            if frames == 0 {
                return Ok(Some((0, probed)));
            }
            // Copy out of the decoder into native interleaved f32, reusing
            // the buffer.
            let capacity = decoded.capacity() as u64;
            let sample_buf = match &mut self.sample_buf {
                Some(b) if b.capacity() >= decoded.capacity() * probed.channels.count() => b,
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
    /// [`PlaybackError::Decode`] on any non-recoverable mid-stream demux or
    /// decode failure other than a clean end of stream. A codec-level packet
    /// decode error skips that packet and continues, and
    /// [`PlaybackError::DecoderPanicked`] when the failure was a panic inside
    /// the decoder rather than an error it returned (`contain_panics`).
    pub fn next_block(&mut self) -> Result<Option<&[f32]>, PlaybackError> {
        contain_panics("decoding", || self.next_block_inner())
    }

    /// The body of [`Self::next_block`], which wraps this in
    /// [`contain_panics`].
    fn next_block_inner(&mut self) -> Result<Option<&[f32]>, PlaybackError> {
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

            // Normalize to stereo into the reusable output block. Three cases
            // and no fourth: fold a multichannel layout with the BS.775 matrix,
            // duplicate a mono channel, or hand a stereo packet straight
            // through — the last still a `memcpy` of exactly the bytes the
            // decoder produced, which is what keeps the bit-exactness tests
            // meaningful for the formats that claim it.
            if let Some(downmix) = &self.downmix {
                let n = downmix.source_channels();
                downmix.apply(&native[skip * n..(skip + take) * n], &mut self.block);
            } else {
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
            source_channels: src.source_channels,
            replay_gain: src.replay_gain,
        })
    }
}

fn recoverable_packet_error(error: &SymphoniaError) -> bool {
    matches!(error, SymphoniaError::DecodeError(_))
}

/// Feed one metadata revision's tags to a [`ReplayGainReader`].
///
/// The key filter runs first so a value string is only built for the handful
/// of tags that are ReplayGain — a well-tagged file carries dozens, and
/// `Value`'s `Display` allocates. Raw keys are passed through untouched:
/// deciding what `----:com.apple.iTunes:replaygain_track_gain` means is
/// [`field_of_key`]'s job and is shared with the library scanner, so a file
/// cannot mean one thing to the shelf and another to the engine.
fn absorb_replay_gain(reader: &mut ReplayGainReader, revision: &MetadataRevision) {
    for tag in revision.tags() {
        if field_of_key(&tag.key).is_some() {
            reader.absorb(&tag.key, &tag.value.to_string());
        }
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
    fn a_codec_packet_error_costs_the_packet_not_the_track() {
        assert!(recoverable_packet_error(&SymphoniaError::DecodeError(
            "invalid packet"
        )));
        assert!(!recoverable_packet_error(&SymphoniaError::ResetRequired));
    }

    /// The containment does what ADR-0040 §2 says: a panic out of a decoder
    /// becomes an error, and an ordinary error is left alone.
    ///
    /// The mechanism is tested here rather than through a file, and the reason
    /// is worth writing down: **there is no longer an input that reaches a
    /// panic.** All three the fuzz sweep found were in the raw-ADTS reader,
    /// and [`probe`] no longer registers it, so the demonstrated triggers are
    /// gone. The guard stays for the parsers baz *does* register — a crafted
    /// MP4 still reaches symphonia's AAC decoder, where the third of those
    /// three panics lived — and this test is what keeps it honest in the
    /// meantime. `crates/baz-core/tests/hostile_media.rs` drives the files.
    #[test]
    fn a_panic_out_of_a_decoder_becomes_an_error() {
        let contained: Result<(), PlaybackError> =
            contain_panics("opening", || panic!("a decoder came apart"));
        let Err(error) = contained else {
            panic!("a panic must not come back as success");
        };
        assert!(
            matches!(error, PlaybackError::DecoderPanicked { doing: "opening" }),
            "{error:?}"
        );
        assert_eq!(
            error.to_string(),
            "the decoder panicked while opening this file"
        );
        // An error the decoder *returned* travels unchanged — the variant is a
        // statement about how the failure happened, not a catch-all.
        let returned: Result<(), PlaybackError> =
            contain_panics("decoding", || Err(PlaybackError::NoDefaultTrack));
        assert!(matches!(returned, Err(PlaybackError::NoDefaultTrack)));
        // And a success is a success.
        assert_eq!(contain_panics("decoding", || Ok(7)).ok(), Some(7));
    }

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
