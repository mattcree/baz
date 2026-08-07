//! Fixture generation shared by the `gen-signals` bin and the test suite.
//!
//! - One continuous 440 Hz sine, 10 s @ 44100 Hz stereo f32, written as a
//!   single reference WAV and as two 5 s halves split at a deliberately
//!   non-zero-crossing sample.
//! - The same signal quantized to i16 (reference + halves) as FLAC encoder
//!   input; encoded via `ffmpeg` or the `flac` CLI when available.
//! - A sample-rate-change pair: 5 s @ 44100 Hz then 5 s @ 48000 Hz, the second
//!   file continuing the sine's phase in absolute time.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::signal;
use crate::{Error, Result};

/// Test tone frequency (Hz).
pub const FREQ: f64 = 440.0;
/// Test tone amplitude (full scale = 1.0).
pub const AMP: f64 = 0.8;
/// Base stream rate.
pub const RATE: u32 = 44_100;
/// The "different" rate for the boundary-change pair.
pub const RATE_HI: u32 = 48_000;
/// Total frames of the 10 s reference at [`RATE`].
pub const TOTAL_FRAMES: usize = 441_000;
/// Split point. 220_513 frames = 5.0003 s; phase there is
/// sin(2*pi*440*220513/44100) ~= 0.73 * amplitude — decidedly not a zero
/// crossing, so any splice error is audible in the numbers.
pub const SPLIT_FRAME: usize = 220_513;
/// Frames in each half of the sample-rate pair (5 s each at its own rate).
pub const RATE_PAIR_FRAMES_44K: usize = 220_500;
/// Frames in the 48 kHz half.
pub const RATE_PAIR_FRAMES_48K: usize = 240_000;

/// FLAC encodings of the reference and its halves.
pub struct FlacFixtures {
    /// Whole-signal FLAC.
    pub full: PathBuf,
    /// First half.
    pub part1: PathBuf,
    /// Second half.
    pub part2: PathBuf,
    /// Which encoder produced them ("ffmpeg" or "flac").
    pub encoder: &'static str,
}

/// All generated fixture paths.
pub struct FixtureSet {
    /// Directory holding everything.
    pub dir: PathBuf,
    /// 10 s continuous reference, f32 WAV.
    pub ref_f32: PathBuf,
    /// First half (frames `0..SPLIT_FRAME`), f32 WAV.
    pub part1_f32: PathBuf,
    /// Second half, f32 WAV.
    pub part2_f32: PathBuf,
    /// Reference quantized to i16 WAV (FLAC encoder input / FLAC ground truth).
    pub ref_i16: PathBuf,
    /// 5 s @ 44100 Hz half of the rate-change pair, f32 WAV.
    pub rate_44k: PathBuf,
    /// 5 s @ 48000 Hz half (phase-continuous in absolute time), f32 WAV.
    pub rate_48k: PathBuf,
    /// FLAC encodings, if an encoder CLI was found.
    pub flac: Option<FlacFixtures>,
}

/// Generate every fixture into `dir` (created if needed).
pub fn generate(dir: &Path) -> Result<FixtureSet> {
    std::fs::create_dir_all(dir)?;

    // Continuous reference and its two halves — halves are slices of the SAME
    // buffer, so ground truth is exact by construction.
    let full = signal::sine_stereo(FREQ, AMP, RATE, TOTAL_FRAMES, 0.0);
    let split = SPLIT_FRAME * 2; // interleaved index
    let ref_f32 = dir.join("ref_10s_44100_f32.wav");
    let part1_f32 = dir.join("part1_f32.wav");
    let part2_f32 = dir.join("part2_f32.wav");
    signal::write_wav_f32(&ref_f32, RATE, &full)?;
    signal::write_wav_f32(&part1_f32, RATE, &full[..split])?;
    signal::write_wav_f32(&part2_f32, RATE, &full[split..])?;

    let ref_i16 = dir.join("ref_10s_44100_i16.wav");
    let part1_i16 = dir.join("part1_i16.wav");
    let part2_i16 = dir.join("part2_i16.wav");
    signal::write_wav_i16(&ref_i16, RATE, &full)?;
    signal::write_wav_i16(&part1_i16, RATE, &full[..split])?;
    signal::write_wav_i16(&part2_i16, RATE, &full[split..])?;

    // Sample-rate-change pair: the 48 kHz half continues the sine in absolute
    // time (t0 = 5.0 s).
    let rate_44k = dir.join("rate_5s_44100_f32.wav");
    let rate_48k = dir.join("rate_5s_48000_f32.wav");
    let a = signal::sine_stereo(FREQ, AMP, RATE, RATE_PAIR_FRAMES_44K, 0.0);
    let b = signal::sine_stereo(FREQ, AMP, RATE_HI, RATE_PAIR_FRAMES_48K, 5.0);
    signal::write_wav_f32(&rate_44k, RATE, &a)?;
    signal::write_wav_f32(&rate_48k, RATE_HI, &b)?;

    let flac = encode_flac(dir, &ref_i16, &part1_i16, &part2_i16)?;

    Ok(FixtureSet {
        dir: dir.to_path_buf(),
        ref_f32,
        part1_f32,
        part2_f32,
        ref_i16,
        rate_44k,
        rate_48k,
        flac,
    })
}

fn have(cmd: &str, arg: &str) -> bool {
    Command::new(cmd)
        .arg(arg)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn encode_flac(
    dir: &Path,
    full_wav: &Path,
    part1_wav: &Path,
    part2_wav: &Path,
) -> Result<Option<FlacFixtures>> {
    let full = dir.join("ref_10s.flac");
    let part1 = dir.join("part1.flac");
    let part2 = dir.join("part2.flac");
    let jobs = [(full_wav, &full), (part1_wav, &part1), (part2_wav, &part2)];

    let encoder = if have("ffmpeg", "-version") {
        for (wav, flac) in &jobs {
            run(Command::new("ffmpeg")
                .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
                .arg(wav)
                .args(["-c:a", "flac"])
                .arg(flac))?;
        }
        "ffmpeg"
    } else if have("flac", "--version") {
        for (wav, flac) in &jobs {
            run(Command::new("flac")
                .args(["--silent", "--force", "-o"])
                .arg(flac)
                .arg(wav))?;
        }
        "flac"
    } else {
        return Ok(None);
    };

    Ok(Some(FlacFixtures {
        full,
        part1,
        part2,
        encoder,
    }))
}

fn run(cmd: &mut Command) -> Result<()> {
    let out = cmd.output()?;
    if !out.status.success() {
        return Err(Error::from(format!(
            "encoder failed: {:?}\n{}",
            cmd,
            String::from_utf8_lossy(&out.stderr)
        )));
    }
    Ok(())
}
