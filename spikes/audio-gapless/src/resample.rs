//! Sample-rate conversion for the "resample at track boundary" strategy,
//! built on `rubato`'s windowed-sinc resampler.
//!
//! Alignment facts (verified empirically with an impulse, see RESULTS.md):
//! `SincFixedIn` initializes its interpolation index at `-sinc_len/2`, so
//! output frame 0 already corresponds to input frame 0 — there is NO leading
//! delay to trim (despite `output_delay()` returning one). The cost of that
//! alignment is an onset transient: the first ~`sinc_len/2` input frames are
//! interpolated against zero history, and the tail likewise flushes against
//! zeros.
//!
//! To make the splice sample-accurate, the input is padded on both ends with
//! an anti-reflection (`2*x[edge] - x[edge+k]`), which continues a smooth
//! signal to second order. The pad length is chosen as a multiple of
//! `from/gcd(from, to)` so the pad maps to an exact integer number of output
//! frames and can be trimmed without sub-sample phase error.

use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};

use crate::{Error, Result};

const CHUNK: usize = 1024;
const SINC_LEN: usize = 256;

fn gcd(a: u32, b: u32) -> u32 {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}

/// Anti-reflective pad: continue `x` past its edge as `2*x[edge] - x[edge+k]`.
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

/// Resample interleaved audio from `from` Hz to `to` Hz, sample-aligned
/// (output frame 0 corresponds to input frame 0), returning exactly
/// `round(in_frames * to / from)` frames.
pub fn resample_interleaved(
    input: &[f32],
    channels: usize,
    from: u32,
    to: u32,
) -> Result<Vec<f32>> {
    if from == to {
        return Ok(input.to_vec());
    }
    let in_frames = input.len() / channels;
    let ratio = f64::from(to) / f64::from(from);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let expected = (in_frames as f64 * ratio).round() as usize;

    // Pad so the sinc onset/tail transients land entirely in trimmable,
    // integer-output-length regions.
    let g = gcd(from, to);
    let step_in = (from / g) as usize;
    let step_out = (to / g) as usize;
    let pad_in = step_in * (SINC_LEN / 2).div_ceil(step_in).max(1);
    let trim_out = pad_in / step_in * step_out;
    if in_frames <= pad_in {
        return Err(Error::from("input too short to pad for resampling"));
    }

    let params = SincInterpolationParameters {
        sinc_len: SINC_LEN,
        f_cutoff: 0.95,
        interpolation: SincInterpolationType::Cubic,
        oversampling_factor: 256,
        window: WindowFunction::BlackmanHarris2,
    };
    let mut rs = SincFixedIn::<f32>::new(ratio, 1.1, params, CHUNK, channels)?;

    // Deinterleave and pad.
    let in_ch: Vec<Vec<f32>> = (0..channels)
        .map(|c| {
            let ch: Vec<f32> = input.iter().skip(c).step_by(channels).copied().collect();
            pad_channel(&ch, pad_in)
        })
        .collect();
    let padded_frames = in_frames + 2 * pad_in;
    let want = trim_out + expected;
    let mut out_ch: Vec<Vec<f32>> = (0..channels)
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
        return Err(Error::from(format!(
            "resampler produced {} frames, expected at least {want}",
            out_ch[0].len()
        )));
    }

    // Trim the pad (exactly trim_out frames), take exactly the expected
    // length, reinterleave.
    let mut out = Vec::with_capacity(expected * channels);
    for i in 0..expected {
        for c in &out_ch {
            out.push(c[trim_out + i]);
        }
    }
    Ok(out)
}
