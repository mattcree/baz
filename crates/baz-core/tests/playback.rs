//! Integration tests for `baz_core::playback`: the gapless engine verified
//! entirely headless through [`OfflineSink`].
//!
//! Ground truth is synthesized (a continuous 440 Hz sine) or a reference
//! single-file decode — never the engine's own recorded output
//! (`docs/ENGINEERING.md`: tests assert against specification, not
//! implementation). FLAC fixtures are encoded with an external encoder
//! (`ffmpeg` or the `flac` CLI, whichever is present) so the FLAC test is an
//! external-reference test; it skips with a notice when neither exists.
//! MP3 fixtures are encoded with ffmpeg's `libmp3lame` (LAME via ffmpeg),
//! which writes the Xing/Info + LAME header the gapless trim relies on; the
//! MP3 tests skip with a notice when that encoder is unavailable.

use std::f64::consts::PI;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::time::Duration;

use baz_core::playback::{
    AudioSource, BoundaryPolicy, CHANNELS, EngineConfig, OfflineSink, PlaybackError, run_playlist,
};

// ---------------------------------------------------------------------------
// Signal synthesis and analysis (ground truth by construction)
// ---------------------------------------------------------------------------

/// Test tone frequency (Hz).
const FREQ: f64 = 440.0;
/// Test tone amplitude (full scale = 1.0).
const AMP: f64 = 0.8;
/// Base stream rate.
const RATE: u32 = 44_100;
/// The "different" rate for the boundary-change pair.
const RATE_HI: u32 = 48_000;
/// Total frames of the 10 s reference at [`RATE`].
const TOTAL_FRAMES: usize = 441_000;
/// Split point. `220_513` frames = 5.0003 s; the sine's phase there is
/// ~0.73 × amplitude — decidedly not a zero crossing, so any splice error
/// shows up loudly in the numbers.
const SPLIT_FRAME: usize = 220_513;
/// Frames in the 44.1 kHz half of the rate-change pair (5 s).
const RATE_PAIR_FRAMES_44K: usize = 220_500;
/// Frames in the 48 kHz half (5 s).
const RATE_PAIR_FRAMES_48K: usize = 240_000;

/// Interleaved stereo sine (same signal on both channels). `t0` is the
/// absolute start time in seconds, so a file that begins mid-signal
/// continues the same phase.
fn sine_stereo(rate: u32, frames: usize, t0: f64) -> Vec<f32> {
    let mut v = Vec::with_capacity(frames * CHANNELS);
    for n in 0..frames {
        let s = ideal_sample_at(rate, n, t0);
        v.push(s);
        v.push(s);
    }
    v
}

/// The ideal test-sine value at frame `n` of a stream at `rate`, offset by
/// `t0` seconds.
#[allow(clippy::cast_precision_loss)] // frame indices are far below 2^52
fn ideal_sample_at(rate: u32, n: usize, t0: f64) -> f32 {
    let t = t0 + n as f64 / f64::from(rate);
    #[allow(clippy::cast_possible_truncation)] // f64 sine -> f32 sample
    let s = (AMP * (2.0 * PI * FREQ * t).sin()) as f32;
    s
}

fn write_wav_f32(path: &Path, rate: u32, interleaved: &[f32]) {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(path, spec).expect("create wav");
    for &s in interleaved {
        w.write_sample(s).expect("write sample");
    }
    w.finalize().expect("finalize wav");
}

/// 16-bit PCM WAV (quantized) — the input format for FLAC encoding.
fn write_wav_i16(path: &Path, rate: u32, interleaved: &[f32]) {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).expect("create wav");
    for &s in interleaved {
        #[allow(clippy::cast_possible_truncation)] // rounded and in i16 range
        let q = (f64::from(s) * 32767.0).round() as i16;
        w.write_sample(q).expect("write sample");
    }
    w.finalize().expect("finalize wav");
}

/// A WAV with an arbitrary channel count from mono samples (each frame
/// repeats the sample across all channels).
fn write_wav_multi(path: &Path, rate: u32, channels: u16, mono: &[f32]) {
    let spec = hound::WavSpec {
        channels,
        sample_rate: rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(path, spec).expect("create wav");
    for &s in mono {
        for _ in 0..channels {
            w.write_sample(s).expect("write sample");
        }
    }
    w.finalize().expect("finalize wav");
}

/// Extract one channel from interleaved samples.
fn channel(interleaved: &[f32], ch: usize) -> Vec<f32> {
    interleaved
        .iter()
        .skip(ch)
        .step_by(CHANNELS)
        .copied()
        .collect()
}

/// Largest jump between adjacent samples of a mono signal. A continuous
/// sine of amplitude A at frequency f sampled at fs never exceeds
/// `2*A*sin(pi*f/fs)`; a splice gap or click blows far past that bound.
fn max_adjacent_delta(mono: &[f32]) -> f32 {
    mono.windows(2)
        .map(|w| (w[1] - w[0]).abs())
        .fold(0.0, f32::max)
}

/// The theoretical adjacent-sample bound for the continuous test sine.
fn sine_adjacent_bound(rate: u32) -> f32 {
    #[allow(clippy::cast_possible_truncation)] // analytic bound -> f32
    let b = (2.0 * AMP * (PI * FREQ / f64::from(rate)).sin()) as f32;
    b
}

/// Max absolute error of a mono signal against the ideal sine over `range`
/// (frame indices of the output stream at `rate`).
fn max_error_vs_sine(mono: &[f32], rate: u32, range: std::ops::Range<usize>) -> f32 {
    range
        .filter(|&n| n < mono.len())
        .map(|n| (mono[n] - ideal_sample_at(rate, n, 0.0)).abs())
        .fold(0.0, f32::max)
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

struct FlacFixtures {
    full: PathBuf,
    part1: PathBuf,
    part2: PathBuf,
    encoder: &'static str,
}

struct Mp3Fixtures {
    full: PathBuf,
    part1: PathBuf,
    part2: PathBuf,
    encoder: &'static str,
}

struct FixtureSet {
    /// 10 s continuous reference, f32 WAV.
    ref_f32: PathBuf,
    /// First half (frames `0..SPLIT_FRAME`), f32 WAV.
    part1_f32: PathBuf,
    /// Second half, f32 WAV.
    part2_f32: PathBuf,
    /// Reference quantized to i16 WAV (FLAC ground truth).
    ref_i16: PathBuf,
    /// 5 s @ 44.1 kHz half of the rate-change pair.
    rate_44k: PathBuf,
    /// 5 s @ 48 kHz half (phase-continuous in absolute time).
    rate_48k: PathBuf,
    /// Mono variant of part1 (upmix test).
    part1_mono: PathBuf,
    /// 4-channel file (rejection test).
    quad: PathBuf,
    /// FLAC encodings, if an encoder CLI was found.
    flac: Option<FlacFixtures>,
    /// MP3 encodings (LAME via ffmpeg), if the encoder was found.
    mp3: Option<Mp3Fixtures>,
}

fn have(cmd: &str, arg: &str) -> bool {
    Command::new(cmd)
        .arg(arg)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn run_encoder(cmd: &mut Command) {
    let out = cmd.output().expect("spawn encoder");
    assert!(
        out.status.success(),
        "encoder failed: {:?}\n{}",
        cmd,
        String::from_utf8_lossy(&out.stderr)
    );
}

fn encode_flac(dir: &Path, full: &Path, part1: &Path, part2: &Path) -> Option<FlacFixtures> {
    let flac_full = dir.join("ref_10s.flac");
    let flac_part1 = dir.join("part1.flac");
    let flac_part2 = dir.join("part2.flac");
    let jobs = [
        (full, &flac_full),
        (part1, &flac_part1),
        (part2, &flac_part2),
    ];
    let encoder = if have("ffmpeg", "-version") {
        for (wav, flac) in &jobs {
            run_encoder(
                Command::new("ffmpeg")
                    .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
                    .arg(wav)
                    .args(["-c:a", "flac"])
                    .arg(flac),
            );
        }
        "ffmpeg"
    } else if have("flac", "--version") {
        for (wav, flac) in &jobs {
            run_encoder(
                Command::new("flac")
                    .args(["--silent", "--force", "-o"])
                    .arg(flac)
                    .arg(wav),
            );
        }
        "flac"
    } else {
        return None;
    };
    Some(FlacFixtures {
        full: flac_full,
        part1: flac_part1,
        part2: flac_part2,
        encoder,
    })
}

/// Encode the i16 reference WAVs to MP3 with ffmpeg's `libmp3lame` (LAME).
/// 320 kbps CBR: the highest-fidelity MP3 a user's library can contain, so
/// the measured tolerances below describe the *best* case honestly labelled
/// as such. ffmpeg writes the Xing/Info header with the LAME extension
/// (encoder delay + padding) by default, which is exactly the metadata the
/// gapless trim consumes.
fn encode_mp3(dir: &Path, full: &Path, part1: &Path, part2: &Path) -> Option<Mp3Fixtures> {
    let have_lame = Command::new("ffmpeg")
        .args(["-hide_banner", "-h", "encoder=libmp3lame"])
        .output()
        .map(|o| {
            o.status.success() && String::from_utf8_lossy(&o.stdout).contains("Encoder libmp3lame")
        })
        .unwrap_or(false);
    if !have_lame {
        return None;
    }
    let mp3_full = dir.join("ref_10s.mp3");
    let mp3_part1 = dir.join("part1.mp3");
    let mp3_part2 = dir.join("part2.mp3");
    for (wav, mp3) in [(full, &mp3_full), (part1, &mp3_part1), (part2, &mp3_part2)] {
        run_encoder(
            Command::new("ffmpeg")
                .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
                .arg(wav)
                .args(["-c:a", "libmp3lame", "-b:a", "320k"])
                .arg(mp3),
        );
    }
    Some(Mp3Fixtures {
        full: mp3_full,
        part1: mp3_part1,
        part2: mp3_part2,
        encoder: "ffmpeg libmp3lame 320 kbps CBR",
    })
}

fn fixtures() -> &'static FixtureSet {
    static FIXTURES: OnceLock<FixtureSet> = OnceLock::new();
    FIXTURES.get_or_init(|| {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("playback-fixtures");
        std::fs::create_dir_all(&dir).expect("create fixture dir");

        // Continuous reference and its halves — the halves are slices of the
        // SAME buffer, so ground truth is exact by construction.
        let full = sine_stereo(RATE, TOTAL_FRAMES, 0.0);
        let split = SPLIT_FRAME * CHANNELS;
        let ref_f32 = dir.join("ref_10s_44100_f32.wav");
        let part1_f32 = dir.join("part1_f32.wav");
        let part2_f32 = dir.join("part2_f32.wav");
        write_wav_f32(&ref_f32, RATE, &full);
        write_wav_f32(&part1_f32, RATE, &full[..split]);
        write_wav_f32(&part2_f32, RATE, &full[split..]);

        // The quantized (i16 PCM) set is the FLAC encoder input and the
        // FLAC losslessness ground truth.
        let pcm_ref = dir.join("ref_10s_44100_i16.wav");
        let pcm_part1 = dir.join("part1_i16.wav");
        let pcm_part2 = dir.join("part2_i16.wav");
        write_wav_i16(&pcm_ref, RATE, &full);
        write_wav_i16(&pcm_part1, RATE, &full[..split]);
        write_wav_i16(&pcm_part2, RATE, &full[split..]);

        // Rate-change pair: the 48 kHz half continues the sine in absolute
        // time (t0 = 5.0 s).
        let rate_44k = dir.join("rate_5s_44100_f32.wav");
        let rate_48k = dir.join("rate_5s_48000_f32.wav");
        write_wav_f32(
            &rate_44k,
            RATE,
            &sine_stereo(RATE, RATE_PAIR_FRAMES_44K, 0.0),
        );
        write_wav_f32(
            &rate_48k,
            RATE_HI,
            &sine_stereo(RATE_HI, RATE_PAIR_FRAMES_48K, 5.0),
        );

        // Channel-layout fixtures.
        let mono: Vec<f32> = channel(&full[..split], 0);
        let part1_mono = dir.join("part1_mono_f32.wav");
        write_wav_multi(&part1_mono, RATE, 1, &mono);
        let quad = dir.join("quad_f32.wav");
        write_wav_multi(&quad, RATE, 4, &mono[..4410]);

        let flac = encode_flac(&dir, &pcm_ref, &pcm_part1, &pcm_part2);
        let mp3 = encode_mp3(&dir, &pcm_ref, &pcm_part1, &pcm_part2);

        FixtureSet {
            ref_f32,
            part1_f32,
            part2_f32,
            ref_i16: pcm_ref,
            rate_44k,
            rate_48k,
            part1_mono,
            quad,
            flac,
            mp3,
        }
    })
}

fn test_config() -> EngineConfig {
    EngineConfig {
        ring_frames: 8192,
        consumer_chunk_frames: 2048,
        consumer_pace: Duration::from_micros(500),
        boundary: BoundaryPolicy::ResampleToStreamRate,
    }
}

/// Exact sample equality with a useful failure message.
fn assert_samples_eq(got: &[f32], want: &[f32], what: &str) {
    assert_eq!(got.len(), want.len(), "{what}: length mismatch");
    if let Some(i) = (0..got.len()).find(|&i| got[i] != want[i]) {
        panic!(
            "{what}: first mismatch at interleaved sample {i} (frame {}): got {} want {}",
            i / CHANNELS,
            got[i],
            want[i]
        );
    }
}

/// Continuity check on channel 0 around an output frame boundary: the
/// largest adjacent-sample jump must be consistent with a continuous sine
/// (no click, no dropped/duplicated samples).
fn assert_boundary_continuous(interleaved: &[f32], boundary_frame: usize, what: &str) -> f32 {
    let ch0 = channel(interleaved, 0);
    let lo = boundary_frame.saturating_sub(1000);
    let hi = (boundary_frame + 1000).min(ch0.len());
    let max_delta = max_adjacent_delta(&ch0[lo..hi]);
    let bound = sine_adjacent_bound(RATE) * 1.05;
    assert!(
        max_delta <= bound,
        "{what}: adjacent-sample jump {max_delta} at boundary exceeds continuous-sine bound {bound}"
    );
    max_delta
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Sanity: the two split WAVs concatenate to exactly the reference at decode
/// level (no engine involved).
#[test]
fn split_wavs_reconstruct_reference() {
    let f = fixtures();
    let reference = AudioSource::decode_all(&f.ref_f32).expect("decode reference");
    assert_eq!(reference.sample_rate, RATE);
    assert_eq!(reference.samples.len(), TOTAL_FRAMES * CHANNELS);
    let p1 = AudioSource::decode_all(&f.part1_f32).expect("decode part1");
    let p2 = AudioSource::decode_all(&f.part2_f32).expect("decode part2");
    assert_eq!(p1.samples.len(), SPLIT_FRAME * CHANNELS);
    let mut joined = p1.samples;
    joined.extend_from_slice(&p2.samples);
    assert_samples_eq(&joined, &reference.samples, "split WAV concatenation");
}

/// Gapless (WAV): engine output over [part1, part2] — split at a
/// non-zero-crossing — is sample-for-sample identical to the single-file
/// reference decode, and the splice region is click-free.
#[test]
fn gapless_wav_bit_exact() {
    let f = fixtures();
    let reference = AudioSource::decode_all(&f.ref_f32).expect("decode reference");
    let mut sink = OfflineSink::with_capacity(reference.samples.len());
    let report = run_playlist(
        &[f.part1_f32.clone(), f.part2_f32.clone()],
        test_config(),
        &mut sink,
    )
    .expect("run playlist");

    assert_eq!(report.stream_rate, RATE);
    assert_eq!(report.track_start_frames, vec![0, SPLIT_FRAME]);
    assert_samples_eq(sink.samples(), &reference.samples, "gapless WAV output");
    let max_delta = assert_boundary_continuous(sink.samples(), SPLIT_FRAME, "gapless WAV");
    println!(
        "[gapless-wav] output={} samples, boundary max adjacent delta={:.6} (bound {:.6})",
        sink.samples().len(),
        max_delta,
        sine_adjacent_bound(RATE)
    );
}

/// Gapless (FLAC): same assertion through Symphonia's FLAC decode, with an
/// external encoder providing the fixtures. Also cross-checks FLAC
/// losslessness against the i16 WAV ground truth.
#[test]
fn gapless_flac_bit_exact() {
    let f = fixtures();
    let Some(flac) = &f.flac else {
        eprintln!(
            "SKIP: neither ffmpeg nor the flac CLI is available; FLAC fixtures not generated"
        );
        return;
    };
    // FLAC must decode bit-identically to the i16 WAV it was encoded from.
    let reference_flac = AudioSource::decode_all(&flac.full).expect("decode flac reference");
    let reference_wav = AudioSource::decode_all(&f.ref_i16).expect("decode i16 reference");
    assert_eq!(reference_flac.sample_rate, RATE);
    assert_samples_eq(
        &reference_flac.samples,
        &reference_wav.samples,
        "FLAC vs i16 WAV reference",
    );

    let mut sink = OfflineSink::with_capacity(reference_flac.samples.len());
    let report = run_playlist(
        &[flac.part1.clone(), flac.part2.clone()],
        test_config(),
        &mut sink,
    )
    .expect("run playlist");
    assert_eq!(report.track_start_frames, vec![0, SPLIT_FRAME]);
    assert_samples_eq(
        sink.samples(),
        &reference_flac.samples,
        "gapless FLAC output",
    );
    let max_delta = assert_boundary_continuous(sink.samples(), SPLIT_FRAME, "gapless FLAC");
    println!(
        "[gapless-flac] encoder={}, output={} samples, boundary max adjacent delta={:.6}",
        flac.encoder,
        sink.samples().len(),
        max_delta
    );
}

// ---------------------------------------------------------------------------
// MP3 (lossy) — length exactness, content sanity, gapless boundary
// ---------------------------------------------------------------------------

/// One MPEG-1 Layer III frame at 44.1 kHz (samples per frame). Used to
/// exclude codec edge regions from steady-state comparisons.
const MP3_FRAME: usize = 1152;

/// Steady-state accuracy bound for the 320 kbps CBR fixture, as max |error|
/// vs the ideal sine (amplitude 0.8), file edges excluded. Derivation:
/// measured 2.64e-4 (−69.6 dB re. amplitude) with ffmpeg 8.1 libmp3lame;
/// pinned at 8e-4 (−60 dB, ~3x) so a real decode regression fails while
/// encoder-version noise does not. This is a *fixture* bound (pure tone,
/// top bitrate), not a general MP3 quality claim.
const MP3_STEADY_TOL: f32 = 8.0e-4;

/// Half-width (in frames) of the splice *edge region* where independently
/// encoded MP3s show MDCT edge artifacts even after exact trim. Measured
/// decay profile (same fixture): error peaks at the very first/last trimmed
/// sample and falls to steady-state level within ~128 samples (2.9 ms at
/// 44.1 kHz) on both sides.
const MP3_EDGE_FRAMES: usize = 128;

/// Peak accuracy bound inside the splice edge region. Derivation: measured
/// peak 4.3e-2 (−25.4 dB re. amplitude) at the first sample of the second
/// file, 2.8e-2 at the last sample of the first; pinned at ~2x. This is the
/// honest cost of splicing *independently encoded* MP3s — a sub-3 ms, about
/// −25 dB blip at the joint, vs literally 0 for FLAC/WAV. (Album rips
/// encoded track-by-track exhibit exactly this; encoders that carry MDCT
/// state across tracks, e.g. LAME `--nogap`, would not.)
const MP3_EDGE_TOL: f32 = 8.0e-2;

/// Accuracy bound just outside the edge region (the rest of ±[`MP3_FRAME`]
/// around the splice). Derivation: measured 5.7e-4 max beyond 128 samples
/// from the joint; pinned at 2x [`MP3_STEADY_TOL`].
const MP3_SETTLED_TOL: f32 = 1.6e-3;

/// Length exactness: with the LAME header present, decoding yields exactly
/// the source sample count — encoder delay and padding are trimmed, not
/// merely "small". Also proves the hint-less `open_bytes` path (the fuzz
/// entry point) probes MP3 by content.
#[test]
fn mp3_decoded_length_is_exact() {
    let f = fixtures();
    let Some(mp3) = &f.mp3 else {
        eprintln!("SKIP: ffmpeg with libmp3lame not available; MP3 fixtures not generated");
        return;
    };
    let full = AudioSource::decode_all(&mp3.full).expect("decode mp3 reference");
    assert_eq!(full.sample_rate, RATE);
    assert_eq!(
        full.frames(),
        TOTAL_FRAMES,
        "LAME-tagged MP3 must decode to exactly the source frame count \
         (delay/padding trimmed)"
    );
    let p1 = AudioSource::decode_all(&mp3.part1).expect("decode mp3 part1");
    let p2 = AudioSource::decode_all(&mp3.part2).expect("decode mp3 part2");
    assert_eq!(p1.frames(), SPLIT_FRAME, "part1 trimmed length");
    assert_eq!(
        p2.frames(),
        TOTAL_FRAMES - SPLIT_FRAME,
        "part2 trimmed length"
    );

    // Same file through the hint-less in-memory path the fuzz target drives:
    // MP3 must probe by content, and the trim must be identical.
    let bytes = std::fs::read(&mp3.full).expect("read mp3 bytes");
    let mut src = AudioSource::open_bytes(bytes).expect("mp3 must probe with no extension hint");
    assert_eq!(src.sample_rate(), RATE);
    let mut frames = 0usize;
    while let Some(block) = src.next_block().expect("decode block") {
        frames += block.len() / CHANNELS;
    }
    assert_eq!(
        frames, TOTAL_FRAMES,
        "open_bytes path must trim identically"
    );
    println!(
        "[mp3-length] encoder={}: {} frames decoded from all three fixtures, \
         exact to the sample",
        mp3.encoder,
        full.frames()
    );
}

/// Content sanity: the decoded MP3 is the source sine within lossy
/// tolerance over the steady state (first/last MPEG frame excluded — codec
/// edges are covered by the boundary test's own bound).
#[test]
fn mp3_content_matches_source_sine() {
    let f = fixtures();
    let Some(mp3) = &f.mp3 else {
        eprintln!("SKIP: ffmpeg with libmp3lame not available; MP3 fixtures not generated");
        return;
    };
    let decoded = AudioSource::decode_all(&mp3.full).expect("decode mp3 reference");
    let ch0 = channel(&decoded.samples, 0);
    let err = max_error_vs_sine(&ch0, RATE, MP3_FRAME..TOTAL_FRAMES - MP3_FRAME);
    let err_db = 20.0 * f64::from(err / 0.8_f32).log10();
    println!(
        "[mp3-content] steady-state max |error| vs ideal sine: {err:.2e} \
         ({err_db:.1} dB re. amplitude); bound {MP3_STEADY_TOL:.2e}"
    );
    assert!(
        err <= MP3_STEADY_TOL,
        "decoded MP3 deviates from the source sine by {err} (bound {MP3_STEADY_TOL})"
    );
}

/// Gapless boundary: two independently encoded MP3s of a split sine, played
/// through the engine. "No gap" is exact — output length equals the sum of
/// the trimmed lengths, i.e. the source length to the sample, and the
/// signal phase is continuous through the joint (a trim error of even one
/// granule would be a phase slip orders of magnitude past these bounds).
/// Continuity is honest lossy continuity: a sub-3 ms edge artifact bounded
/// by [`MP3_EDGE_TOL`], settling to [`MP3_SETTLED_TOL`] beyond
/// [`MP3_EDGE_FRAMES`] — not the literal 0 of FLAC/WAV (which
/// [`gapless_flac_bit_exact`] pins). That is what users splicing
/// independently encoded MP3s actually get.
#[test]
fn gapless_mp3_no_gap_bounded_boundary() {
    let f = fixtures();
    let Some(mp3) = &f.mp3 else {
        eprintln!("SKIP: ffmpeg with libmp3lame not available; MP3 fixtures not generated");
        return;
    };
    let mut sink = OfflineSink::with_capacity(TOTAL_FRAMES * CHANNELS);
    let report = run_playlist(
        &[mp3.part1.clone(), mp3.part2.clone()],
        test_config(),
        &mut sink,
    )
    .expect("run playlist");
    assert_eq!(report.stream_rate, RATE);
    assert_eq!(report.track_start_frames, vec![0, SPLIT_FRAME]);
    // No gap, no overlap: trimmed lengths sum to the source length exactly.
    assert_eq!(
        sink.samples().len(),
        TOTAL_FRAMES * CHANNELS,
        "engine output must be exactly the source length: any leftover \
         delay/padding would show up here as extra or missing samples"
    );

    let ch0 = channel(sink.samples(), 0);
    // Accuracy across the splice, against the one continuous sine both
    // files were cut from — split into the measured edge region and the
    // settled remainder of ±one MPEG frame.
    let err_edge = max_error_vs_sine(
        &ch0,
        RATE,
        SPLIT_FRAME - MP3_EDGE_FRAMES..SPLIT_FRAME + MP3_EDGE_FRAMES,
    );
    let err_settled_before = max_error_vs_sine(
        &ch0,
        RATE,
        SPLIT_FRAME - MP3_FRAME..SPLIT_FRAME - MP3_EDGE_FRAMES,
    );
    let err_settled_after = max_error_vs_sine(
        &ch0,
        RATE,
        SPLIT_FRAME + MP3_EDGE_FRAMES..SPLIT_FRAME + MP3_FRAME,
    );
    let err_settled = err_settled_before.max(err_settled_after);
    // Click check over the edge region: adjacent-sample deltas may exceed a
    // continuous sine's bound by at most twice the edge error (the error
    // rides on the sine's own slope).
    let lo = SPLIT_FRAME - MP3_EDGE_FRAMES;
    let hi = (SPLIT_FRAME + MP3_EDGE_FRAMES).min(ch0.len());
    let max_delta = max_adjacent_delta(&ch0[lo..hi]);
    let delta_bound = sine_adjacent_bound(RATE) + 2.0 * MP3_EDGE_TOL;
    let edge_db = 20.0 * f64::from(err_edge / 0.8_f32).log10();
    println!(
        "[gapless-mp3] encoder={}: output={} samples (exact, no gap); splice edge \
         (±{MP3_EDGE_FRAMES} frames) max |error| {err_edge:.2e} ({edge_db:.1} dB re. amplitude, \
         bound {MP3_EDGE_TOL:.2e}); settled (rest of ±{MP3_FRAME}) {err_settled:.2e} \
         (bound {MP3_SETTLED_TOL:.2e}); max adjacent delta {max_delta:.6} \
         (bound {delta_bound:.6}; continuous-sine bound {:.6}; FLAC/WAV excess: 0)",
        mp3.encoder,
        sink.samples().len(),
        sine_adjacent_bound(RATE)
    );
    assert!(
        err_edge <= MP3_EDGE_TOL,
        "splice edge error {err_edge} exceeds bound {MP3_EDGE_TOL}"
    );
    assert!(
        err_settled <= MP3_SETTLED_TOL,
        "signal has not settled beyond the edge region: {err_settled} \
         exceeds bound {MP3_SETTLED_TOL} (trim misalignment?)"
    );
    assert!(
        max_delta <= delta_bound,
        "click at the splice: adjacent delta {max_delta} exceeds bound {delta_bound}"
    );
}

/// Decode-ahead: track N+1 is demonstrably decoded while track N is still
/// draining (prefetch overlap evidence).
#[test]
fn decode_ahead_overlaps_playback() {
    let f = fixtures();
    let mut sink = OfflineSink::with_capacity(TOTAL_FRAMES * CHANNELS);
    let report = run_playlist(
        &[f.part1_f32.clone(), f.part2_f32.clone()],
        test_config(),
        &mut sink,
    )
    .expect("run playlist");
    let p = &report.prefetch;
    #[allow(clippy::cast_precision_loss)] // diagnostic percentage only
    let pct =
        100.0 * p.next_frames_decoded_when_prev_drained as f64 / p.next_track_frames_total as f64;
    println!(
        "[decode-ahead] track2 decode finished at {:.2} ms; track1 drained at {:.2} ms; \
         track2 frames decoded at drain: {}/{} ({pct:.1}%); track2 decode took {:.2} ms",
        p.next_decode_done_ms_from_start,
        p.prev_drain_ms_from_start,
        p.next_frames_decoded_when_prev_drained,
        p.next_track_frames_total,
        p.next_decode_ms,
    );
    assert_eq!(p.next_track_frames_total, TOTAL_FRAMES - SPLIT_FRAME);
    assert!(
        p.next_frames_decoded_when_prev_drained > 0,
        "no overlap: track 2 had not started decoding when track 1 drained"
    );
    assert!(
        p.next_decode_finished_before_prev_drained,
        "track 2 decode did not finish before track 1 finished draining \
         (decoded {}/{} frames at drain)",
        p.next_frames_decoded_when_prev_drained, p.next_track_frames_total
    );
    assert!(p.next_decode_done_ms_from_start < p.prev_drain_ms_from_start);
}

/// Resample boundary (ADR-0004 default policy): a 48 kHz track after a
/// 44.1 kHz track is resampled to the stream rate on the prefetch side and
/// spliced seamlessly. The sine must continue through the boundary at
/// −45 dB error or better (Spike B measured −45.5 dB), and the output
/// duration must be exact.
#[test]
fn resample_boundary_is_continuous() {
    let f = fixtures();
    // The 48 kHz half resamples to exactly 220_500 frames at 44.1 kHz.
    let expected_frames = RATE_PAIR_FRAMES_44K + RATE_PAIR_FRAMES_44K;
    let mut sink = OfflineSink::with_capacity(expected_frames * CHANNELS);
    let report = run_playlist(
        &[f.rate_44k.clone(), f.rate_48k.clone()],
        test_config(),
        &mut sink,
    )
    .expect("run playlist");

    assert_eq!(report.stream_rate, RATE);
    let out = sink.samples();
    assert_eq!(
        out.len(),
        expected_frames * CHANNELS,
        "resampled output duration must be exactly 10 s at 44.1 kHz"
    );

    let ch0 = channel(out, 0);
    let boundary = RATE_PAIR_FRAMES_44K;

    // Track 1 passes through untouched: exact.
    let err_track1 = max_error_vs_sine(&ch0, RATE, 0..boundary);
    // err_track1 is a max of absolute values, so <= 0.0 means exactly zero
    // (avoids a strict float equality the lints rightly frown on).
    assert!(
        err_track1 <= 0.0,
        "track 1 must be bit-exact (no resampling applied to it), error {err_track1}"
    );

    // Boundary region and the whole resampled tail: the sine continues
    // within the resampler's passband error.
    let err_boundary = max_error_vs_sine(&ch0, RATE, boundary.saturating_sub(200)..boundary + 2000);
    let err_tail = max_error_vs_sine(&ch0, RATE, boundary..ch0.len());
    let max_delta = max_adjacent_delta(&ch0);
    // Passband ripple rides on the ideal sine slope, so adjacent samples may
    // legitimately differ by up to bound + 2*ripple; a splice error blows
    // far past this.
    let ripple_tolerance = 4.5e-3_f32; // == the −45 dB acceptance below
    let delta_bound = sine_adjacent_bound(RATE) + 2.0 * ripple_tolerance;
    let err_db = 20.0 * f64::from(err_tail.max(err_boundary) / 0.8).log10();
    println!(
        "[resample] resample took {:.2} ms for 5 s of 48 kHz stereo; \
         max |error| vs ideal sine: boundary {err_boundary:.2e}, tail {err_tail:.2e} \
         ({err_db:.1} dB re. amplitude); max adjacent delta {max_delta:.6} (bound {delta_bound:.6})",
        report.resample_ms.unwrap_or(f64::NAN),
    );

    // Spike B measured 4.25e-3 (−45.5 dB re. 0.8 amplitude); accept −45 dB
    // (4.5e-3) or better so a regression fails loudly without flaking on
    // platform float noise.
    assert!(
        err_boundary <= ripple_tolerance,
        "splice not continuous: boundary error {err_boundary} (> -45 dB) vs ideal sine"
    );
    assert!(
        err_tail <= ripple_tolerance,
        "resampled tail deviates from ideal sine by {err_tail} (> -45 dB)"
    );
    assert!(
        max_delta <= delta_bound,
        "click detected: adjacent delta {max_delta} exceeds bound {delta_bound}"
    );
}

/// The bit-perfect reopen mode is part of the API contract (ADR-0004) but
/// its implementation arrives with the exclusive-mode backends: selecting it
/// must return the documented error, not an approximation.
#[test]
fn bit_perfect_reopen_reports_unimplemented() {
    let f = fixtures();
    let cfg = EngineConfig {
        boundary: BoundaryPolicy::BitPerfectReopen,
        ..test_config()
    };
    let mut sink = OfflineSink::with_capacity(16);
    let err = run_playlist(std::slice::from_ref(&f.part1_f32), cfg, &mut sink)
        .expect_err("reopen mode must refuse");
    assert!(matches!(err, PlaybackError::BitPerfectReopenUnimplemented));
    assert!(
        err.to_string().contains("not yet implemented"),
        "error must say so plainly: {err}"
    );
}

/// Mono sources are upmixed to stereo by duplication.
#[test]
fn mono_upmixes_to_stereo() {
    let f = fixtures();
    let stereo = AudioSource::decode_all(&f.part1_f32).expect("decode stereo part1");
    let mono = AudioSource::decode_all(&f.part1_mono).expect("decode mono part1");
    // The mono file was channel 0 of the same signal; upmix must reproduce
    // the stereo file exactly (both channels of the fixture are identical).
    assert_samples_eq(&mono.samples, &stereo.samples, "mono upmix");
}

/// More than two channels is rejected with the documented error (downmix is
/// future work; silently wrong output is not acceptable).
#[test]
fn multichannel_is_rejected() {
    let f = fixtures();
    let err = AudioSource::decode_all(&f.quad).expect_err("quad must be rejected");
    assert!(
        matches!(err, PlaybackError::UnsupportedChannelCount { channels: 4 }),
        "unexpected error: {err}"
    );
}

/// Sacred-thread guard: the consumer pull path performs no allocation.
///
/// Honest scope of this test (a counting global allocator would need
/// `unsafe`, which this workspace forbids): [`OfflineSink`] preallocates and
/// by contract never grows, so we prove (a) the sink's buffer pointer never
/// moved and nothing was dropped — no reallocation happened; and (b) the
/// consumer loop's only other operations are `rtrb` wait-free chunk reads,
/// atomic loads/stores, and arithmetic — none of which allocate, enforced by
/// construction and review of `engine::consume`. Anything the pull path
/// allocated would have to go through the sink, and the sink provably did
/// not.
#[test]
fn pull_path_does_not_reallocate() {
    let f = fixtures();
    let capacity = TOTAL_FRAMES * CHANNELS; // exact output size
    let mut sink = OfflineSink::with_capacity(capacity);
    let ptr_before = sink.samples().as_ptr();
    run_playlist(
        &[f.part1_f32.clone(), f.part2_f32.clone()],
        test_config(),
        &mut sink,
    )
    .expect("run playlist");
    assert_eq!(sink.samples().len(), capacity, "sink exactly filled");
    assert_eq!(sink.dropped_samples(), 0, "nothing dropped");
    assert_eq!(
        sink.samples().as_ptr(),
        ptr_before,
        "sink storage reallocated during the run — allocation on the pull path"
    );
}

// ---------------------------------------------------------------------------
// Seeking (source level, per format)
// ---------------------------------------------------------------------------

/// Decode everything a source yields after seeking to `position_ms`.
fn decode_from(path: &Path, position_ms: u64) -> Vec<f32> {
    let mut src = AudioSource::open(path).expect("open");
    src.seek(position_ms).expect("seek");
    let mut out = Vec::new();
    while let Some(block) = src.next_block().expect("decode") {
        out.extend_from_slice(block);
    }
    out
}

/// Seeking a losslessly-coded stream is **sample-exact**: what comes back is
/// bit-for-bit the tail of the un-seeked decode. Both formats are covered
/// because they take different code paths inside Symphonia (WAV computes a
/// byte offset; FLAC binary-searches its seek table), and both land on a
/// packet boundary *before* the target — so this is really a test that the
/// residue is discarded correctly.
#[test]
fn seek_is_sample_exact_for_wav_and_flac() {
    let f = fixtures();
    let mut cases: Vec<(&str, &Path)> = vec![("wav f32", f.ref_f32.as_path())];
    match &f.flac {
        Some(flac) => cases.push(("flac", flac.full.as_path())),
        None => eprintln!("SKIP(flac): no FLAC encoder available"),
    }
    for (what, path) in cases {
        let reference = AudioSource::decode_all(path)
            .expect("reference decode")
            .samples;
        assert_eq!(reference.len(), TOTAL_FRAMES * CHANNELS, "{what}: fixture");
        for position_ms in [0u64, 1, 3_000, 9_999] {
            let target = (position_ms * u64::from(RATE) / 1000) as usize;
            let got = decode_from(path, position_ms);
            assert_samples_eq(
                &got,
                &reference[target * CHANNELS..],
                &format!("{what}: decode after seek to {position_ms} ms"),
            );
        }
    }
}

/// Seeking an MP3 is *not* bit-exact — the decoder restarts mid-stream and
/// Symphonia rewinds a few reference frames to refill the bit reservoir —
/// but it must be **accurate in time and in content**: the audio that comes
/// back has to be the ideal sine at the position asked for (not shifted, not
/// phase-slipped), and the remaining length has to be the declared length
/// minus the seek target, which is what proves the gapless trim was neither
/// lost nor applied twice by the seek.
#[test]
fn seek_into_mp3_is_time_accurate_and_keeps_the_trim() {
    let f = fixtures();
    let Some(mp3) = &f.mp3 else {
        eprintln!("SKIP(mp3): ffmpeg libmp3lame not available");
        return;
    };
    for position_ms in [0u64, 3_000, 7_500] {
        let target = (position_ms * u64::from(RATE) / 1000) as usize;
        let got = decode_from(&mp3.full, position_ms);

        // Length: exactly the declared remainder. A double-applied encoder
        // delay, or a lost one, would show up here as a shifted count.
        assert_eq!(
            got.len(),
            (TOTAL_FRAMES - target) * CHANNELS,
            "mp3 seek to {position_ms} ms must leave exactly the remaining frames"
        );

        // Content: the sine picked up at absolute time `target`, compared
        // against synthesized ground truth rather than any recorded decode.
        // The first MP3 frame after the restart is excluded — the reservoir
        // is still warming — and the file's own tail edge likewise.
        let ch0 = channel(&got, 0);
        let from = MP3_FRAME.min(ch0.len());
        let to = ch0.len().saturating_sub(MP3_EDGE_FRAMES);
        let mut worst = 0.0f32;
        for (i, &s) in ch0.iter().enumerate().take(to).skip(from) {
            worst = worst.max((s - ideal_sample_at(RATE, target + i, 0.0)).abs());
        }
        println!(
            "[seek mp3] {position_ms} ms: steady-state max error {worst:.2e} \
             ({} frames compared, encoder {})",
            to - from,
            mp3.encoder
        );
        assert!(
            worst <= MP3_STEADY_TOL,
            "mp3 seek to {position_ms} ms: max error {worst} exceeds {MP3_STEADY_TOL}; \
             a time-inaccurate seek shows up here as a large phase error"
        );
    }
}

/// A seek past the declared end is reported plainly, not guessed at or
/// silently clamped — the engine needs to be able to tell the difference in
/// order to advance to the next track.
#[test]
fn seek_past_the_declared_end_is_an_error() {
    let f = fixtures();
    let mut src = AudioSource::open(&f.ref_f32).expect("open");
    assert_eq!(src.duration_ms(), Some(10_000));
    assert_eq!(src.total_frames(), Some(TOTAL_FRAMES as u64));
    // Exactly at the end counts as past it: there is no audio there.
    match src.seek(10_000) {
        Err(PlaybackError::SeekPastEnd {
            position_ms,
            track_ms,
        }) => {
            assert_eq!(position_ms, 10_000);
            assert_eq!(track_ms, Some(10_000));
        }
        other => panic!("expected SeekPastEnd, got {other:?}"),
    }
    assert!(matches!(
        src.seek(60_000),
        Err(PlaybackError::SeekPastEnd { .. })
    ));
}

/// Repeated seeks on one source stay correct: no accumulated skip counter,
/// no drift, and backwards works as well as forwards.
#[test]
fn repeated_seeks_do_not_drift() {
    let f = fixtures();
    let reference = AudioSource::decode_all(&f.ref_f32)
        .expect("reference decode")
        .samples;
    let mut src = AudioSource::open(&f.ref_f32).expect("open");
    for position_ms in [5_000u64, 1_000, 8_000, 1_000, 0] {
        src.seek(position_ms).expect("seek");
        let target = (position_ms * u64::from(RATE) / 1000) as usize;
        let block = src
            .next_block()
            .expect("decode")
            .expect("audio after a mid-stream seek");
        assert_samples_eq(
            &block[..64],
            &reference[target * CHANNELS..][..64],
            &format!("first block after seeking to {position_ms} ms"),
        );
    }
}

/// An empty playlist is a clear error, not a silent no-op.
#[test]
fn empty_playlist_is_an_error() {
    let mut sink = OfflineSink::with_capacity(0);
    let err = run_playlist(&[], test_config(), &mut sink).expect_err("empty playlist");
    assert!(matches!(err, PlaybackError::EmptyPlaylist));
}

/// Device output (feature `device-output`): opening the default device
/// either succeeds or fails with the documented `Device` error — never a
/// panic. Runs headless-safe: CI containers have no audio device, so a
/// failure to open is a skip, not a test failure.
#[cfg(feature = "device-output")]
#[test]
fn device_sink_opens_or_reports_cleanly() {
    use baz_core::playback::device::DeviceSink;
    match DeviceSink::open(RATE, 8192) {
        Ok(mut sink) => {
            // Play 50 ms of the fixture tone for a smoke check.
            let samples = sine_stereo(RATE, 2205, 0.0);
            baz_core::playback::Sink::write(&mut sink, &samples);
            assert!(!sink.failed(), "stream reported an error during smoke run");
            println!("[device] opened default output device and wrote 50 ms");
        }
        Err(PlaybackError::Device(msg)) => {
            eprintln!("SKIP: no usable output device ({msg})");
        }
        Err(other) => panic!("unexpected error opening device: {other}"),
    }
}
