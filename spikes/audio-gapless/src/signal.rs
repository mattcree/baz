//! Test-signal synthesis and analysis helpers.
//!
//! Ground truth is synthesized here (continuous sine), never recorded from the
//! engine's own output — per ENGINEERING.md, tests assert against
//! specification, not implementation.

use std::f64::consts::PI;
use std::path::Path;

use crate::Result;

/// Synthesize an interleaved stereo sine (same signal on both channels).
///
/// `t0` is the absolute start time in seconds, so a file that begins mid-signal
/// (e.g. the 48 kHz half of the sample-rate pair) continues the same phase.
#[must_use]
pub fn sine_stereo(freq: f64, amp: f64, rate: u32, frames: usize, t0: f64) -> Vec<f32> {
    let mut v = Vec::with_capacity(frames * 2);
    for n in 0..frames {
        let t = t0 + n as f64 / f64::from(rate);
        #[allow(clippy::cast_possible_truncation)]
        let s = (amp * (2.0 * PI * freq * t).sin()) as f32;
        v.push(s);
        v.push(s);
    }
    v
}

/// The ideal value of the test sine at output frame `n` of a stream at `rate`.
#[must_use]
pub fn ideal_sample(freq: f64, amp: f64, rate: u32, n: usize) -> f32 {
    let t = n as f64 / f64::from(rate);
    #[allow(clippy::cast_possible_truncation)]
    let s = (amp * (2.0 * PI * freq * t).sin()) as f32;
    s
}

/// Write interleaved stereo f32 samples as an IEEE-float WAV.
pub fn write_wav_f32(path: &Path, rate: u32, interleaved: &[f32]) -> Result<()> {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(path, spec)?;
    for &s in interleaved {
        w.write_sample(s)?;
    }
    w.finalize()?;
    Ok(())
}

/// Write interleaved stereo samples as 16-bit PCM WAV (quantized), the input
/// format for FLAC encoding.
pub fn write_wav_i16(path: &Path, rate: u32, interleaved: &[f32]) -> Result<()> {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec)?;
    for &s in interleaved {
        #[allow(clippy::cast_possible_truncation)]
        let q = (f64::from(s) * 32767.0).round() as i16;
        w.write_sample(q)?;
    }
    w.finalize()?;
    Ok(())
}

/// Extract one channel from interleaved samples.
#[must_use]
pub fn channel(interleaved: &[f32], channels: usize, ch: usize) -> Vec<f32> {
    interleaved
        .iter()
        .skip(ch)
        .step_by(channels)
        .copied()
        .collect()
}

/// Largest jump between adjacent samples of a mono signal. A continuous sine of
/// amplitude A at frequency f sampled at fs never exceeds `2*A*sin(pi*f/fs)`;
/// a splice gap or click shows up as a jump far above that bound.
#[must_use]
pub fn max_adjacent_delta(mono: &[f32]) -> f32 {
    mono.windows(2)
        .map(|w| (w[1] - w[0]).abs())
        .fold(0.0, f32::max)
}

/// The theoretical adjacent-sample bound for a continuous sine.
#[must_use]
pub fn sine_adjacent_bound(freq: f64, amp: f64, rate: u32) -> f32 {
    #[allow(clippy::cast_possible_truncation)]
    let b = (2.0 * amp * (PI * freq / f64::from(rate)).sin()) as f32;
    b
}

/// Max absolute error of a mono signal against the ideal sine, over
/// `range` (frame indices of the output stream at `rate`).
#[must_use]
pub fn max_error_vs_sine(
    mono: &[f32],
    freq: f64,
    amp: f64,
    rate: u32,
    range: std::ops::Range<usize>,
) -> f32 {
    range
        .filter(|&n| n < mono.len())
        .map(|n| (mono[n] - ideal_sample(freq, amp, rate, n)).abs())
        .fold(0.0, f32::max)
}
