//! Splice-exact sample-rate conversion for the ADR-0004 default boundary
//! policy, built on `rubato`'s windowed-sinc resampler.
//!
//! Runs on the prefetch thread only — never the realtime pull path.
//!
//! # Alignment (the hard-won part — do not "fix" this)
//!
//! `SincFixedIn` initializes its interpolation index at `-sinc_len/2`, so
//! output frame 0 already corresponds to input frame 0: there is **no**
//! leading delay to trim, *despite* `output_delay()` reporting one. Trimming
//! `output_delay()` frames — the obvious reading of the docs — shifts the
//! splice by ~2.7 ms and produces an audible-scale discontinuity (measured in
//! Spike B with an impulse test; recorded in ADR-0004). The cost of the
//! alignment is instead an onset/tail transient: the first ~`sinc_len/2`
//! frames are interpolated against zero history, and the tail flushes
//! against zeros.
//!
//! To make the splice sample-accurate, the input is padded on both ends with
//! an anti-reflection (`2*x[edge] - x[edge+k]`), which continues a smooth
//! signal to second order so the transient lands entirely in the padding.
//! The pad length is a multiple of `from/gcd(from, to)` so it maps to an
//! exact integer number of output frames and trims off without sub-sample
//! phase error.

use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};

use super::{CHANNELS, PlaybackError};

/// Input frames fed to the resampler per call.
const CHUNK: usize = 1024;
/// Sinc kernel length; also determines the padding requirement.
const SINC_LEN: usize = 256;

fn gcd(a: u32, b: u32) -> u32 {
    if b == 0 { a } else { gcd(b, a % b) }
}

/// Anti-reflective pad: continue `x` past each edge as `2*x[edge] - x[edge+k]`.
fn pad_channel(x: &[f32], pad: usize) -> Vec<f32> {
    let n = x.len();
    let mut y = Vec::with_capacity(n + 2 * pad);
    for k in (1..=pad).rev() {
        y.push(2.0 * x[0] - x[k.min(n - 1)]);
    }
    y.extend_from_slice(x);
    for k in 1..=pad {
        y.push(2.0 * x[n - 1] - x[n - 1 - k.min(n - 1)]);
    }
    y
}

/// Resample interleaved stereo audio from `from` Hz to `to` Hz,
/// splice-aligned (output frame 0 corresponds to input frame 0), returning
/// exactly `round(in_frames * to / from)` frames.
///
/// # Errors
///
/// [`PlaybackError::TrackTooShortToResample`] when the input is shorter than
/// the alignment padding, and [`PlaybackError::Resample`] if `rubato` fails.
pub fn resample_interleaved(input: &[f32], from: u32, to: u32) -> Result<Vec<f32>, PlaybackError> {
    if from == to {
        return Ok(input.to_vec());
    }
    let in_frames = input.len() / CHANNELS;
    let ratio = f64::from(to) / f64::from(from);
    // Rounded positive frame count; magnitudes are far below 2^52 so the
    // f64 round-trip is exact.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    #[allow(clippy::cast_precision_loss)]
    let expected = (in_frames as f64 * ratio).round() as usize;

    // Pad so the sinc onset/tail transients land entirely in trimmable,
    // integer-output-length regions.
    let g = gcd(from, to);
    let step_in = (from / g) as usize;
    let step_out = (to / g) as usize;
    let pad_in = step_in * (SINC_LEN / 2).div_ceil(step_in).max(1);
    let trim_out = pad_in / step_in * step_out;
    if in_frames <= pad_in {
        return Err(PlaybackError::TrackTooShortToResample {
            frames: in_frames,
            min_frames: pad_in,
        });
    }

    let params = SincInterpolationParameters {
        sinc_len: SINC_LEN,
        f_cutoff: 0.95,
        interpolation: SincInterpolationType::Cubic,
        oversampling_factor: 256,
        window: WindowFunction::BlackmanHarris2,
    };
    let mut rs = SincFixedIn::<f32>::new(ratio, 1.1, params, CHUNK, CHANNELS)?;

    // Deinterleave and pad.
    let in_ch: Vec<Vec<f32>> = (0..CHANNELS)
        .map(|c| {
            let ch: Vec<f32> = input.iter().skip(c).step_by(CHANNELS).copied().collect();
            pad_channel(&ch, pad_in)
        })
        .collect();
    let padded_frames = in_frames + 2 * pad_in;
    let want = trim_out + expected;
    let mut out_ch: Vec<Vec<f32>> = (0..CHANNELS)
        .map(|_| Vec::with_capacity(want + CHUNK))
        .collect();

    let append = |out_ch: &mut Vec<Vec<f32>>, blocks: Vec<Vec<f32>>| {
        for (dst, src) in out_ch.iter_mut().zip(blocks) {
            dst.extend_from_slice(&src);
        }
    };

    let mut pos = 0;
    while padded_frames - pos >= CHUNK {
        let ins: Vec<&[f32]> = in_ch.iter().map(|c| &c[pos..pos + CHUNK]).collect();
        append(&mut out_ch, rs.process(&ins, None)?);
        pos += CHUNK;
    }
    if pos < padded_frames {
        let ins: Vec<&[f32]> = in_ch.iter().map(|c| &c[pos..]).collect();
        append(&mut out_ch, rs.process_partial(Some(&ins), None)?);
    }
    // Flush whatever the resampler still holds.
    while out_ch[0].len() < want {
        let blocks = rs.process_partial(None::<&[&[f32]]>, None)?;
        if blocks[0].is_empty() {
            break;
        }
        append(&mut out_ch, blocks);
    }
    if out_ch[0].len() < want {
        return Err(PlaybackError::Resample(format!(
            "resampler produced {} frames, expected at least {want}",
            out_ch[0].len()
        )));
    }

    // Trim the pad (exactly `trim_out` frames), take exactly the expected
    // length, reinterleave.
    let mut out = Vec::with_capacity(expected * CHANNELS);
    for i in 0..expected {
        for c in &out_ch {
            out.push(c[trim_out + i]);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Spike B "rubato gotcha" regression guard: an impulse placed at
    /// input frame k must land at output frame round(k*to/from) — i.e. the
    /// output is time-aligned with input frame 0 and `output_delay()` must
    /// NOT have been compensated. See module docs and ADR-0004.
    #[test]
    #[allow(clippy::cast_precision_loss)] // test frame indices are far below 2^52
    fn impulse_lands_time_aligned() {
        let (from, to) = (48_000u32, 44_100u32);
        let frames = 48_000usize;
        let k = 24_000usize;
        let mut input = vec![0.0f32; frames * CHANNELS];
        input[k * CHANNELS] = 1.0;
        input[k * CHANNELS + 1] = 1.0;
        let out = resample_interleaved(&input, from, to).expect("resample");
        assert_eq!(out.len() / CHANNELS, 44_100, "output length exact");
        let ch0: Vec<f32> = out.iter().step_by(CHANNELS).copied().collect();
        let peak = ch0
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
            .map(|(i, _)| i)
            .expect("non-empty output");
        let expect = (k as f64 * f64::from(to) / f64::from(from)).round();
        assert!(
            (peak as f64 - expect).abs() <= 1.0,
            "impulse at input frame {k} landed at output frame {peak}, expected ~{expect}"
        );
    }

    #[test]
    fn same_rate_is_identity() {
        let input: Vec<f32> = (0u16..1000).map(|i| f32::from(i).sin()).collect();
        let out = resample_interleaved(&input, 44_100, 44_100).expect("identity");
        assert_eq!(out, input);
    }

    #[test]
    fn too_short_input_is_a_clear_error() {
        let input = vec![0.0f32; 16 * CHANNELS];
        let err = resample_interleaved(&input, 48_000, 44_100).expect_err("must fail");
        assert!(matches!(err, PlaybackError::TrackTooShortToResample { .. }));
    }
}
