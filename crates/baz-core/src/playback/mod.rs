//! The gapless playback engine.
//!
//! Architecture (proven in Spike B, `git show dc13d7e -- spikes/audio-gapless`,
//! and ratified in ADR-0003/ADR-0004):
//!
//! - [`AudioSource`] decodes one file via Symphonia into interleaved stereo
//!   f32 blocks. Mono is upmixed; more than two channels is rejected for now
//!   (see [`AudioSource`] docs).
//! - [`engine::run_playlist`] streams the current track through an `rtrb`
//!   lock-free SPSC ring buffer on a decode thread while a prefetch thread
//!   decodes track N+1, so track boundaries are bookkeeping, not audio
//!   events — gapless by construction.
//! - The consumer pull path (the stand-in for the audio callback) is
//!   realtime-disciplined **by construction**: wait-free ring reads, writes
//!   into a preallocated [`Sink`], no locking, no allocation, no I/O, no
//!   panics. See `docs/ENGINEERING.md`, "the audio thread is sacred".
//! - Sample-rate handling follows ADR-0009 (which inverted ADR-0004's
//!   default): the output **follows the source** and baz resamples nothing.
//!   A session opens the device at the rate of the track that starts it, and a
//!   track at a different rate reopens it ([`BoundaryPolicy::BitPerfectReopen`],
//!   the default) — gapless within a rate, a short reconfiguration gap
//!   between rates. Converting to one fixed rate is still available as an
//!   explicit opt-in ([`BoundaryPolicy::ResampleToStreamRate`]), and the one
//!   case where conversion happens without being asked for — hardware that
//!   cannot do the source rate at all — is reported through
//!   [`Event::SignalPath`](crate::protocol::Event::SignalPath) rather than
//!   done silently.
//!
//! # Gapless status by format
//!
//! Every format the library scanner accepts ([`crate::library::AUDIO_EXTENSIONS`])
//! plays. Gapless quality varies by format and the differences are stated
//! here as measurements, not adjectives; each number below is pinned by a
//! test in `crates/baz-core/tests/playback.rs`, so a claim that stops being
//! true fails the build rather than aging quietly in a comment.
//!
//! | Format | Gapless | Cost at a track boundary |
//! |---|---|---|
//! | WAV, FLAC | exact | none (bit-exact) |
//! | ALAC in MP4 (`.m4a`) | exact | none (bit-exact) |
//! | Vorbis in Ogg (`.ogg`) | exact | no edge artifact: the joint measures 1.07x the file's own steady-state error |
//! | MP3 with LAME header | exact | ~3 ms MDCT edge artifact, ≈ −25 dB peak |
//! | MP3 without LAME header | none available | untrimmed delay + padding |
//! | AAC in MP4 (`.m4a`, `.mp4`) | **not trimmed** | ~23 ms of encoder priming |
//!
//! Opus is not in the table because it is not in the library: Symphonia has
//! no Opus decoder in any released version, so `.opus` is deliberately absent
//! from [`crate::library::AUDIO_EXTENSIONS`] rather than listed and skipped
//! (`docs/BACKLOG.md`).
//!
//! - **WAV, FLAC**: exact sample counts in the container; gapless is exact
//!   concatenation, verified bit-for-bit in the integration tests.
//! - **ALAC** (lossless, in MP4): the same standard, met. The MP4 media
//!   header carries an exact frame count and ALAC has no encoder delay or
//!   padding to trim, so there is nothing for a gapless mode to do:
//!   `codec_params.n_frames` is the exact stream length, decode is bit-exact
//!   against the PCM the file was encoded from, and two consecutive tracks
//!   concatenate to the original sample-for-sample. Seeking is sample-exact
//!   too. (Symphonia's ISO-MP4 reader is not gapless-capable — see AAC below
//!   — but for ALAC that is a distinction without a difference.)
//! - **Vorbis** (lossy, in Ogg): **exact**, and the best-behaved lossy format
//!   here. Ogg carries an absolute granule position on every page, which is a
//!   sample count, not an estimate; with `FormatOptions::enable_gapless` the
//!   Ogg reader derives the stream's start delay (the lapped block the first
//!   page cannot yet render) and its end trim from those numbers and trims
//!   the packets itself. **Measured** on the test fixture (ffmpeg
//!   `libvorbis -q:a 6`): decoding a 441 000-frame source yields exactly
//!   441 000 frames, and the two halves of the split reference decode to
//!   exactly 220 513 and 220 487 — the source lengths to the sample. Because
//!   the trim is exact, the splice between two independently encoded Vorbis
//!   files shows **no MDCT edge artifact at all**: peak error at the joint is
//!   1.37e-2 (−35.3 dB re. full amplitude) against 1.28e-2 (−35.9 dB) in the
//!   steady state elsewhere in the same file — a ratio of **1.07**, where the
//!   same ratio for MP3 is **75** — and the largest adjacent-sample step
//!   across the joint is 5.11e-2 against the continuous-sine bound of
//!   5.01e-2, i.e. 2% over it, which is the lossy noise riding on the sine's
//!   own slope and not a click. (MP3 needs that bound widened by twice its
//!   edge tolerance before it passes the same check.)
//!   The comparison that matters is the ratio, not the absolute figure: this
//!   fixture is a steady 440 Hz sine, which libvorbis handles far less
//!   accurately than LAME does (as ffmpeg's native AAC encoder also does), so
//!   −36 dB describes the tone, not the codec on music. What it does show is
//!   that the joint is not a special place, which is the difference an
//!   exactly-trimmed lapped transform makes.
//!
//!   One thing Vorbis does **not** do exactly is *seek*. Symphonia's Vorbis
//!   decoder needs two packets before it can overlap-add, so the first packet
//!   after a mid-stream reset returns an empty buffer and its audio is lost:
//!   playback resumes exactly one lapped block late. **Measured**: 1024
//!   frames — **23.2 ms** at 44.1 kHz — of content offset, and the same 1024
//!   frames missing from the remaining length, at every seek target tried.
//!   (The size is the encoder's long block ÷ 2; 1024 is libvorbis's default.)
//!   Seeking is exact for WAV, FLAC and ALAC and time-accurate for MP3;
//!   Vorbis is the one format where it costs audio, and
//!   `seek_into_vorbis_ogg_costs_one_lapped_block` pins the number.
//! - **FLAC in Ogg** (`.ogg` carrying FLAC rather than Vorbis): plays, with
//!   FLAC's own exact frame count and lossless decode. It arrives free with
//!   the Ogg demuxer and is tested by the extension/decoder invariant test,
//!   not separately — it is the same FLAC decoder behind a different
//!   container.
//! - **MP3**: Symphonia's gapless trim is active
//!   (`FormatOptions::enable_gapless`). Files with a Xing/Info + LAME header
//!   (LAME, and ffmpeg's `libmp3lame`) decode to *exactly* the encoded
//!   sample count — encoder delay and padding are trimmed — so consecutive
//!   tracks concatenate without a gap and with continuous phase. The joint
//!   is lossy-accurate, not bit-exact: independently encoded files have an
//!   MDCT edge artifact at the splice (measured on a 320 kbps pure-tone
//!   fixture: peak ≈ −25 dB re. full amplitude at the joint, decaying to
//!   steady-state ≈ −70 dB within ~3 ms). Files without a LAME header carry
//!   no trim metadata and play with their delay/padding intact. Exact
//!   numbers and methodology: `tests/playback.rs` and the [`source`] module
//!   docs.
//! - **AAC** (lossy, in MP4): plays, but **encoder delay and padding are not
//!   trimmed**, so AAC albums do not join gaplessly. This is a limitation of
//!   Symphonia 0.5, stated plainly rather than papered over: an MP4 records
//!   encoder delay in exactly two places — the edit list (`elst`) and
//!   iTunes' `iTunSMPB` free-form atom — and the ISO-MP4 reader applies
//!   neither. It parses `elst` and never consults it; it does not read
//!   `iTunSMPB` at all, and it parses iTunes' `pgap` "gapless album" flag
//!   only to discard it. The priming frames therefore come out as audio at
//!   the head of every AAC track. **Measured**: with ffmpeg 8.1's native AAC
//!   encoder at 44.1 kHz, decoding is exactly 1024 frames — **23.2 ms** —
//!   longer than the source, all of it a leading offset, so a two-track AAC
//!   album pays that gap once per transition. The number is encoder-specific
//!   (Apple's AAC primes with 2112 frames, 47.9 ms); that the delay survives
//!   untrimmed is not. Trailing padding is bounded by the container's
//!   declared media duration and, on the fixture, disappears entirely.
//!   Everything else about AAC playback is correct: the engine's own splice
//!   still drops and duplicates nothing, and content, length and seek
//!   positions are accurate to the sample within each track.
//! - **HE-AAC** (AAC-LC + SBR — what most streaming rips are): plays, with
//!   one fidelity caveat beyond the AAC gapless story above. Symphonia 0.5
//!   implements no SBR, so it decodes the AAC-LC *core*, which sits at half
//!   the sample rate the MP4 sample entry advertises. [`AudioSource`] takes
//!   the rate from the decoder rather than the container and rescales the
//!   declared length through the same ratio, so pitch, tempo and duration
//!   are right — a 224.072562 s file reports 224.072562 s, agreeing with
//!   `ffprobe` to the microsecond. What is missing is the SBR band: the top
//!   octave is not reconstructed, so the track sounds duller than it should.
//!   Trusting the container's rate instead would play the core an octave up
//!   at double speed, which is why we do not.

pub mod engine;
pub(crate) mod resample;
pub mod sink;
pub mod source;

#[cfg(feature = "device-output")]
pub mod device;

pub use engine::{BoundaryPolicy, EngineConfig, PlayReport, PrefetchEvidence, run_playlist};
pub use sink::{OfflineSink, Sink};
pub use source::{AudioSource, DecodedAudio};

/// The engine's fixed interleaved channel count. Every [`AudioSource`] block,
/// ring-buffer slot, and [`Sink`] write is stereo-interleaved f32.
pub const CHANNELS: usize = 2;

/// Errors from the playback engine.
///
/// Worker-thread failures are joined and surfaced through the same type; the
/// realtime pull path itself has no error channel because it has no fallible
/// operations (see module docs).
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PlaybackError {
    /// Opening or reading the media file failed.
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),

    /// Symphonia could not probe or decode the stream.
    #[error("decode error: {0}")]
    Decode(#[from] symphonia::core::errors::Error),

    /// The container has no default audio track.
    #[error("no default audio track in container")]
    NoDefaultTrack,

    /// The codec parameters omit the sample rate.
    #[error("stream does not declare a sample rate")]
    UnknownSampleRate,

    /// The codec parameters omit the channel layout.
    #[error("stream does not declare a channel layout")]
    UnknownChannelLayout,

    /// The stream has more channels than the engine currently supports.
    ///
    /// TODO(downmix): >2-channel sources should be downmixed to stereo with
    /// standard coefficients; until that lands they are rejected so the
    /// engine never plays something silently wrong.
    #[error(
        "unsupported channel count {channels}: only mono and stereo are supported (multichannel downmix is future work)"
    )]
    UnsupportedChannelCount {
        /// Channel count found in the stream.
        channels: usize,
    },

    /// The playlist was empty.
    #[error("empty playlist")]
    EmptyPlaylist,

    /// A seek target lies at or past the end of the track.
    ///
    /// Not a failure of the file: the engine turns this into "advance to the
    /// next queue position" (see [`crate::protocol::Command::Seek`]), so it
    /// travels as an error only between [`AudioSource::seek`] and its caller.
    #[error("seek to {position_ms} ms is past the end of the track")]
    SeekPastEnd {
        /// The requested position in milliseconds.
        position_ms: u64,
        /// The track's declared length in milliseconds, when it has one.
        track_ms: Option<u64>,
    },

    /// Constructing or running the resampler failed.
    #[error("resampler error: {0}")]
    Resample(String),

    /// The track is too short to resample with splice-exact alignment.
    #[error("track too short to resample: {frames} frames, need more than {min_frames}")]
    TrackTooShortToResample {
        /// Frames in the track.
        frames: usize,
        /// Minimum frame count the alignment padding requires.
        min_frames: usize,
    },

    /// A queue changes sample rate part-way through and the caller asked for
    /// the bit-perfect default, which answers a rate change by reopening the
    /// output — something [`run_playlist`] has no output to do.
    ///
    /// Produced only by [`run_playlist`], never by the interactive engine
    /// ([`crate::engine`]), which *does* reopen and for which a mixed-rate
    /// queue is ordinary. Callers who want a single buffer at a single rate
    /// select [`BoundaryPolicy::ResampleToStreamRate`] and accept the
    /// conversion knowingly.
    #[error(
        "track {index} changes the sample rate from {from} Hz to {to} Hz: the \
         bit-perfect default reopens the output at the new rate, which an \
         offline one-shot render cannot do (ADR-0009). Use the interactive \
         engine, or BoundaryPolicy::ResampleToStreamRate to convert instead"
    )]
    SampleRateChangeRequiresReopen {
        /// Queue index of the track whose rate differs.
        index: usize,
        /// Rate the render is running at, in Hz.
        from: u32,
        /// Rate the track declares, in Hz.
        to: u32,
    },

    /// A decode or prefetch worker thread panicked. Panics are a bug
    /// (`docs/ENGINEERING.md`); this variant reports them instead of
    /// poisoning the caller.
    #[error("worker thread panicked: {0}")]
    WorkerPanicked(&'static str),

    /// The audio device could not be opened or started.
    ///
    /// Only produced by the `device-output` feature's `device` module; the
    /// variant is unconditional so enabling the feature never changes the
    /// error API.
    #[error("audio device error: {0}")]
    Device(String),
}

impl From<rubato::ResamplerConstructionError> for PlaybackError {
    fn from(e: rubato::ResamplerConstructionError) -> Self {
        Self::Resample(e.to_string())
    }
}

impl From<rubato::ResampleError> for PlaybackError {
    fn from(e: rubato::ResampleError) -> Self {
        Self::Resample(e.to_string())
    }
}
