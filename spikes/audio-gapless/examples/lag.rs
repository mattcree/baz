// scratch: verify resampler alignment (impulse) and sine continuity
use baz_spike_audio_gapless::{resample, signal};

fn main() {
    // impulse at input frame 48000 @48k -> expected output peak at 44100
    let mut input = vec![0f32; 240_000 * 2];
    input[48000 * 2] = 1.0;
    input[48000 * 2 + 1] = 1.0;
    let out = resample::resample_interleaved(&input, 2, 48000, 44100).unwrap();
    let ch0 = signal::channel(&out, 2, 0);
    let (argmax, max) = ch0.iter().enumerate().fold((0usize, 0f32), |acc, (i, &v)| {
        if v.abs() > acc.1 {
            (i, v.abs())
        } else {
            acc
        }
    });
    println!(
        "impulse: argmax={argmax} (expected 44100), peak={max:.4}, out_len={}",
        ch0.len()
    );

    let input = signal::sine_stereo(440.0, 0.8, 48000, 240_000, 5.0);
    let out = resample::resample_interleaved(&input, 2, 48000, 44100).unwrap();
    let ch0 = signal::channel(&out, 2, 0);
    let ideal: Vec<f32> = (0..ch0.len())
        .map(|n| {
            let t = 5.0 + n as f64 / 44100.0;
            (0.8 * (2.0 * std::f64::consts::PI * 440.0 * t).sin()) as f32
        })
        .collect();
    let head = (0..200)
        .map(|n| (ch0[n] - ideal[n]).abs())
        .fold(0f32, f32::max);
    let all = (0..ch0.len())
        .map(|n| (ch0[n] - ideal[n]).abs())
        .fold(0f32, f32::max);
    println!("sine: maxerr first 200 = {head:.2e}, maxerr all = {all:.2e}");
}
