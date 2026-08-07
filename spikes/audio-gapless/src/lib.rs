//! Spike B — gapless audio engine core.
//!
//! Proves: Symphonia decode -> decode-ahead worker -> lock-free SPSC ring
//! buffer -> sample-accurate splice across track boundaries, verified entirely
//! offline (no audio device required; `device-output` feature is non-default
//! because alsa-lib-devel is unavailable on this machine).
//!
//! This is spike code: it answers architecture questions and is then deleted.
//! Error handling uses a boxed error alias rather than the `thiserror` types
//! the real `baz-core` will use.

#![forbid(unsafe_code)]

pub mod engine;
pub mod fixtures;
pub mod resample;
pub mod signal;
pub mod sink;
pub mod source;

#[cfg(feature = "device-output")]
pub mod device;

/// Spike-grade error type. The production crate gets `thiserror` enums.
pub type Error = Box<dyn std::error::Error + Send + Sync + 'static>;
/// Spike-grade result alias.
pub type Result<T> = std::result::Result<T, Error>;
