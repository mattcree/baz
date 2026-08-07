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
//! - Sample-rate changes at track boundaries follow ADR-0004: by default the
//!   incoming track is resampled to the stream rate on the prefetch side
//!   ([`BoundaryPolicy::ResampleToStreamRate`]); the bit-perfect reopen mode
//!   is an accepted part of the API contract but is not implemented until the
//!   exclusive-mode output backends exist
//!   ([`BoundaryPolicy::BitPerfectReopen`]).
//!
//! # Gapless status by format
//!
//! WAV and FLAC carry exact sample counts, so gapless is exact concatenation
//! (verified bit-for-bit in the integration tests). MP3/AAC require encoder
//! delay/padding trim; those codecs are deliberately not enabled in the
//! Symphonia feature set until that trim work lands with its own
//! synthesized-signal tests. [`AudioSource`] already honors the
//! delay/padding/frame-count metadata Symphonia exposes, so the remaining
//! lossy work is codec enablement plus verification, not new plumbing.

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

    /// [`BoundaryPolicy::BitPerfectReopen`] was requested. The mode is part
    /// of the ADR-0004 contract but its implementation arrives with the
    /// exclusive-mode output backends; until then the engine refuses rather
    /// than approximating it.
    #[error(
        "bit-perfect reopen mode is not yet implemented: it requires the \
         exclusive-mode device backends (ADR-0004); use \
         BoundaryPolicy::ResampleToStreamRate"
    )]
    BitPerfectReopenUnimplemented,

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
