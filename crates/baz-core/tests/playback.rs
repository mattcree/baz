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
//! MP3 tests skip with a notice when that encoder is unavailable. The `.m4a`
//! (ISO-MP4) fixtures — ALAC, AAC-LC, HE-AAC and a video-first `.mp4` — are
//! encoded the same way and skip the same way; the HE-AAC one additionally
//! needs `libfdk_aac`, since ffmpeg's native AAC encoder cannot produce SBR.
//! The Ogg fixtures — Vorbis via `libvorbis`, FLAC-in-Ogg, and one real Ogg
//! Opus file for the probe test — follow the same pattern, with the Opus one
//! additionally requiring `libopus`.
//!
//! No test here reads the developer's own music library: every fixture is
//! generated from the synthesized reference at run time, so the suite is
//! reproducible on any machine with ffmpeg and honest on any machine
//! without it.

use std::f64::consts::PI;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::time::Duration;

use baz_core::library::AUDIO_EXTENSIONS;
use baz_core::playback::{
    AudioSource, BoundaryPolicy, CHANNELS, EngineConfig, OfflineSink, PlaybackError, run_playlist,
};
// Symphonia is a regular dependency of `baz-core`, so an integration test can
// reach it. The Opus probe test needs it: the claim being asserted is about
// what the *probe* decides the bytes are, which no baz-level API exposes (and
// should not — nothing in baz needs to ask).
use symphonia::core::codecs::CODEC_TYPE_OPUS;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

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

/// Interleaved stereo silence — **the block every test that feeds real
/// hardware writes.**
///
/// A device test's assertions are about the *transport*: that a stream opened,
/// that the ring drained, that a reopen left something that still takes
/// samples, that the driver reported no xruns. None of them is an assertion
/// about what the samples were, and a driver moves silence exactly as it moves
/// a tone — same frame counts, same callbacks, same buffer arithmetic. So the
/// tone bought nothing except a 440 Hz beep out of the developer's speakers on
/// every `cargo test --all-features`, several times a run.
///
/// Audible verification has not gone away; it has moved to where hearing it is
/// the point. The `device_engine_*` tests in `tests/engine.rs` play real
/// decoded fixture audio through the real output, and they are opt-in behind
/// `BAZ_DEVICE_TESTS=1` (`docs/DEVELOPMENT.md`).
///
/// Only the tests that touch hardware use it, and those exist only in a
/// `device-output` build (which `exclusive-output` implies).
#[cfg(feature = "device-output")]
fn silence_stereo(frames: usize) -> Vec<f32> {
    vec![0.0; frames * CHANNELS]
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

/// Vorbis-in-Ogg encodings of the reference set, plus the one Ogg *Opus*
/// file the probe test needs. Opus lives here because it is the same
/// container: the point of the fixture is that the container is identified
/// correctly and the codec inside it is named honestly.
struct OggFixtures {
    full: PathBuf,
    part1: PathBuf,
    part2: PathBuf,
    /// FLAC in an Ogg container — the other codec `.ogg` can carry, and one
    /// the shelf therefore has to play.
    flac_in_ogg: PathBuf,
    /// A real Ogg Opus file, present only if ffmpeg carries `libopus`.
    opus: Option<PathBuf>,
    encoder: &'static str,
}

/// One codec's worth of `.m4a` (ISO-MP4) encodings of the reference set.
struct M4aCodecFixtures {
    full: PathBuf,
    part1: PathBuf,
    part2: PathBuf,
    encoder: &'static str,
}

struct M4aFixtures {
    /// ALAC (lossless) in MP4.
    alac: M4aCodecFixtures,
    /// AAC-LC (lossy) in MP4.
    aac: M4aCodecFixtures,
    /// HE-AAC v1 (AAC-LC core + SBR), if ffmpeg carries `libfdk_aac` —
    /// ffmpeg's native AAC encoder is LC-only. This is what streaming rips
    /// and downloaded video soundtracks in a real library actually are.
    he_aac: Option<PathBuf>,
    /// A `.mp4` whose *first* track is video and whose audio is AAC — the
    /// layout the `mp4` entry in `AUDIO_EXTENSIONS` inevitably meets.
    video_first: PathBuf,
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
    /// Ogg encodings (Vorbis, FLAC-in-Ogg, and Opus), if ffmpeg was found.
    ogg: Option<OggFixtures>,
    /// ALAC and AAC in MP4 (`.m4a`), if ffmpeg with both encoders was found.
    m4a: Option<M4aFixtures>,
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

/// Encode the i16 reference WAVs to Vorbis in Ogg with ffmpeg's `libvorbis`
/// (the reference Vorbis encoder), plus a FLAC-in-Ogg copy of the reference
/// and — if ffmpeg carries `libopus` — one real Ogg Opus file.
///
/// `-q:a 6` is a high-quality VBR setting (~192 kbps on real music). Unlike
/// the MP3 and AAC fixtures, the setting barely matters here: **measured**,
/// `-q:a 10` (libvorbis's maximum) moves the whole-file accuracy from 1.30e-2
/// to 1.23e-2, half a decibel, because what bounds it is libvorbis's
/// noise-normalisation on a *steady pure tone* — a pathological input for it,
/// the same way ffmpeg's native AAC encoder is (cf. [`AAC_STEADY_TOL`]) and
/// unlike LAME, which is three decades better on this signal. So [`VORBIS_TOL`]
/// is a statement about a 440 Hz sine, not about Vorbis on music.
///
/// Nothing about the *gapless* claim depends on any of that: the trim comes
/// from Ogg granule positions, which are exact at every bitrate, and the
/// splice assertion is against the file's own steady-state error rather than
/// an absolute number.
fn encode_ogg(dir: &Path, full: &Path, part1: &Path, part2: &Path) -> Option<OggFixtures> {
    if !have_ffmpeg_encoder("libvorbis") {
        return None;
    }
    let ogg_full = dir.join("ref_10s.ogg");
    let ogg_part1 = dir.join("part1.ogg");
    let ogg_part2 = dir.join("part2.ogg");
    for (wav, ogg) in [(full, &ogg_full), (part1, &ogg_part1), (part2, &ogg_part2)] {
        run_encoder(
            Command::new("ffmpeg")
                .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
                .arg(wav)
                .args(["-c:a", "libvorbis", "-q:a", "6"])
                .arg(ogg),
        );
    }

    // FLAC in an Ogg container: `.ogg` is not synonymous with Vorbis, and the
    // shelf lists `.ogg` by extension.
    let flac_in_ogg = dir.join("flac_in_ogg.ogg");
    run_encoder(
        Command::new("ffmpeg")
            .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
            .arg(full)
            .args(["-c:a", "flac", "-f", "ogg"])
            .arg(&flac_in_ogg),
    );

    // Real Ogg Opus bytes for the probe test. Not playable — that is the
    // point of the test that consumes it.
    let opus = have_ffmpeg_encoder("libopus").then(|| {
        let out = dir.join("ref_10s.opus");
        run_encoder(
            Command::new("ffmpeg")
                .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
                .arg(full)
                .args(["-c:a", "libopus", "-b:a", "128k"])
                .arg(&out),
        );
        out
    });

    Some(OggFixtures {
        full: ogg_full,
        part1: ogg_part1,
        part2: ogg_part2,
        flac_in_ogg,
        opus,
        encoder: "ffmpeg libvorbis -q:a 6",
    })
}

/// Is `ffmpeg` present and does it carry encoder `name`?
fn have_ffmpeg_encoder(name: &str) -> bool {
    Command::new("ffmpeg")
        .args(["-hide_banner", "-h"])
        .arg(format!("encoder={name}"))
        .output()
        .map(|o| {
            o.status.success()
                && String::from_utf8_lossy(&o.stdout).contains(&format!("Encoder {name}"))
        })
        .unwrap_or(false)
}

/// Encode the i16 reference WAVs into `.m4a` (ISO-MP4) twice over: once with
/// ALAC (lossless) and once with AAC-LC at 256 kbps.
///
/// Both are plain `ffmpeg -c:a …` invocations, i.e. exactly what a user's
/// library holds. The AAC bitrate is high on purpose for the same reason the
/// MP3 fixture is 320 kbps: the numbers these tests pin describe the *best*
/// case honestly labelled as such. ffmpeg's native `aac` encoder is AAC-LC
/// only; HE-AAC (SBR) is a separate fixture below because it needs
/// `libfdk_aac` and because Symphonia treats it very differently.
fn encode_m4a(dir: &Path, full: &Path, part1: &Path, part2: &Path) -> Option<M4aFixtures> {
    if !have_ffmpeg_encoder("alac") || !have_ffmpeg_encoder("aac") {
        return None;
    }
    let encode = |codec: &str, extra: &[&str], stem: &str| -> M4aCodecFixtures {
        let paths = [
            dir.join(format!("{stem}_ref_10s.m4a")),
            dir.join(format!("{stem}_part1.m4a")),
            dir.join(format!("{stem}_part2.m4a")),
        ];
        for (wav, out) in [full, part1, part2].into_iter().zip(&paths) {
            run_encoder(
                Command::new("ffmpeg")
                    .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
                    .arg(wav)
                    .args(["-c:a", codec])
                    .args(extra)
                    .arg(out),
            );
        }
        let [full, part1, part2] = paths;
        M4aCodecFixtures {
            full,
            part1,
            part2,
            encoder: if codec == "alac" {
                "ffmpeg alac"
            } else {
                "ffmpeg aac (native LC) 256 kbps"
            },
        }
    };
    let lossless = encode("alac", &[], "alac");
    let lossy = encode("aac", &["-b:a", "256k"], "aac");

    // HE-AAC needs libfdk_aac; optional, tested separately.
    let he_aac = have_ffmpeg_encoder("libfdk_aac").then(|| {
        let out = dir.join("he_aac_ref_10s.m4a");
        run_encoder(
            Command::new("ffmpeg")
                .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
                .arg(full)
                .args(["-c:a", "libfdk_aac", "-profile:a", "aac_he", "-b:a", "64k"])
                .arg(&out),
        );
        out
    });

    // A video-first `.mp4`: a still colour source muxed ahead of the AAC
    // audio, which is how every real `.mp4` is laid out.
    let video_first = dir.join("video_first.mp4");
    run_encoder(
        Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "color=c=black:s=64x64:r=5",
                "-i",
            ])
            .arg(full)
            .args(["-c:v", "mpeg4", "-c:a", "aac", "-b:a", "256k", "-shortest"])
            .arg(&video_first),
    );

    Some(M4aFixtures {
        alac: lossless,
        aac: lossy,
        he_aac,
        video_first,
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
        let ogg = encode_ogg(&dir, &pcm_ref, &pcm_part1, &pcm_part2);
        let m4a = encode_m4a(&dir, &pcm_ref, &pcm_part1, &pcm_part2);

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
            ogg,
            m4a,
        }
    })
}

/// Tuning only: the boundary policy stays at its shipped default (follow the
/// source, convert nothing — ADR-0009), so every test below that does not say
/// otherwise is exercising what baz actually does. The two tests that are
/// *about* conversion opt into `ResampleToStreamRate` explicitly.
fn test_config() -> EngineConfig {
    EngineConfig {
        ring_frames: 8192,
        consumer_chunk_frames: 2048,
        consumer_pace: Duration::from_micros(500),
        ..EngineConfig::default()
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

// ---------------------------------------------------------------------------
// Ogg — Vorbis, FLAC-in-Ogg, and the Opus that is deliberately not playable
// ---------------------------------------------------------------------------

/// The Ogg fixture set, or `None` with a skip notice where ffmpeg is absent
/// (the toolbox) — the same contract the MP3 and MP4 tests keep.
fn ogg_or_skip() -> Option<&'static OggFixtures> {
    let f = fixtures();
    if f.ogg.is_none() {
        eprintln!("SKIP: ffmpeg with libvorbis is not available; Ogg fixtures not generated");
    }
    f.ogg.as_ref()
}

/// Accuracy bound for the Vorbis fixture, as max |error| vs the ideal sine
/// (amplitude 0.8).
///
/// Measured 1.23e-2 (−36.3 dB re. amplitude) whole-file with ffmpeg 8.1's
/// `libvorbis -q:a 6`; pinned at ~3x so an encoder-version wobble does not go
/// red while a decode or trim regression does. That is far looser than
/// [`MP3_STEADY_TOL`] and looser even than [`AAC_STEADY_TOL`], and it is a
/// property of the *fixture*, not of Vorbis: a steady 440 Hz sine is a
/// pathological input for libvorbis's noise normalisation, and raising the
/// quality to its maximum improves it by half a decibel (see [`encode_ogg`]).
/// A **fixture** bound, not a general Vorbis quality claim.
///
/// One bound serves the steady state and the splice alike, deliberately: for
/// Vorbis they are the same measurement. The sharp claim — that the splice
/// is no worse than the steady state — is asserted as a *ratio* in
/// [`gapless_vorbis_ogg_no_gap_and_no_edge_artifact`], which needs no
/// tolerance at all and is what makes this loose absolute bound acceptable.
/// MP3, whose trim is equally exact but whose codec state is not, fails that
/// ratio test by 75x ([`MP3_EDGE_TOL`] vs [`MP3_SETTLED_TOL`]).
const VORBIS_TOL: f32 = 3.5e-2;

/// How much larger the error at a Vorbis splice may be than the same file's
/// steady-state error before the "no edge artifact" claim is considered
/// broken.
///
/// **Measured 1.07** at `-q:a 6` and **0.90** at `-q:a 10` — i.e. the joint is
/// indistinguishable from the rest of the file, and on the higher setting is
/// marginally cleaner. 2x leaves room for encoder noise while failing
/// unmissably on anything resembling MP3's behaviour, where the same ratio is
/// **75**. Being a ratio, it survives encoder and quality changes that would
/// invalidate any absolute number.
const VORBIS_EDGE_RATIO: f32 = 2.0;

/// Frames of a Vorbis long block ÷ 2 — the audio Symphonia's Vorbis decoder
/// drops after a mid-stream reset, because it cannot overlap-add until it has
/// a second packet.
///
/// Not a tolerance and not a choice of ours: it is the *measured* consequence
/// of `VorbisDecoder::decode` returning an empty buffer for the first packet
/// after `reset()`. 1024 is libvorbis's default 2048-sample long block ÷ 2;
/// a stream encoded with different block sizes would lose a different number.
/// That a seek costs one lapped block is not encoder-specific.
const VORBIS_SEEK_LOST_FRAMES: usize = 1024;

/// Half-width of the splice region examined for an edge artifact, in frames
/// (2.9 ms at 44.1 kHz). Same window [`gapless_mp3_no_gap_bounded_boundary`]
/// uses, so the two formats' numbers are directly comparable — which is the
/// whole point of quoting them side by side.
const VORBIS_EDGE_FRAMES: usize = 128;

/// Guard band excluded from the "steady state" measurement either side of the
/// splice, in frames (26 ms at 44.1 kHz). Generous on purpose: the steady-state
/// figure has to be a clean baseline for the splice figure to be compared
/// against.
const VORBIS_GUARD_FRAMES: usize = 1152;

/// Length exactness: Ogg pages carry an absolute granule position, so
/// Symphonia's Ogg reader knows the stream's start delay and end trim to the
/// sample and applies both. Decoding an encode of an N-frame WAV must yield
/// exactly N frames — for the whole reference and for each half of the split,
/// whose lengths are deliberately not multiples of anything.
///
/// Also drives the hint-less `open_bytes` path (the fuzz target's entry
/// point), proving Ogg probes by content and not merely by file extension.
#[test]
fn vorbis_ogg_decoded_length_is_exact() {
    let Some(ogg) = ogg_or_skip() else { return };
    let full = AudioSource::decode_all(&ogg.full).expect("decode ogg reference");
    assert_eq!(full.sample_rate, RATE);
    assert_eq!(
        full.frames(),
        TOTAL_FRAMES,
        "Vorbis in Ogg must decode to exactly the source frame count \
         (granule positions give an exact start delay and end trim)"
    );
    let p1 = AudioSource::decode_all(&ogg.part1).expect("decode ogg part1");
    let p2 = AudioSource::decode_all(&ogg.part2).expect("decode ogg part2");
    assert_eq!(p1.frames(), SPLIT_FRAME, "part1 trimmed length");
    assert_eq!(
        p2.frames(),
        TOTAL_FRAMES - SPLIT_FRAME,
        "part2 trimmed length"
    );

    // Declared length must agree with what is actually emitted, or seek bars
    // and track times lie.
    let src = AudioSource::open(&ogg.full).expect("open ogg reference");
    assert_eq!(
        src.total_frames(),
        Some(TOTAL_FRAMES as u64),
        "declared length must be the emitted length"
    );

    // Same file through the hint-less in-memory path the fuzz target drives.
    let bytes = std::fs::read(&ogg.full).expect("read ogg bytes");
    let mut src = AudioSource::open_bytes(bytes).expect("ogg must probe with no extension hint");
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
        "[vorbis-length] encoder={}: {}/{}/{} frames from the reference and its two \
         halves, exact to the sample",
        ogg.encoder,
        full.frames(),
        p1.frames(),
        p2.frames()
    );
}

/// Content sanity: the decoded Vorbis is the source sine within lossy
/// tolerance across the whole file — including the first and last frames,
/// which for a correctly trimmed Ogg stream are ordinary audio rather than
/// codec edges. (The MP3 equivalent has to exclude one MPEG frame at each
/// end; Vorbis does not need the exemption, so it is not given one.)
#[test]
fn vorbis_ogg_content_matches_source_sine() {
    let Some(ogg) = ogg_or_skip() else { return };
    let decoded = AudioSource::decode_all(&ogg.full).expect("decode ogg reference");
    let ch0 = channel(&decoded.samples, 0);
    let err = max_error_vs_sine(&ch0, RATE, 0..TOTAL_FRAMES);
    let err_db = 20.0 * f64::from(err / 0.8_f32).log10();
    println!(
        "[vorbis-content] whole-file max |error| vs ideal sine: {err:.2e} \
         ({err_db:.1} dB re. amplitude); bound {VORBIS_TOL:.2e}"
    );
    assert!(
        err <= VORBIS_TOL,
        "decoded Vorbis deviates from the source sine by {err} (bound {VORBIS_TOL})"
    );
}

/// FLAC in an Ogg container plays, losslessly. `.ogg` is a container, not a
/// codec, and the shelf lists it by extension — so the FLAC case has to work
/// too, and it must still be bit-exact against the WAV it was encoded from.
#[test]
fn flac_in_ogg_is_lossless() {
    let Some(ogg) = ogg_or_skip() else { return };
    let f = fixtures();
    let decoded = AudioSource::decode_all(&ogg.flac_in_ogg).expect("decode FLAC-in-Ogg");
    let reference = AudioSource::decode_all(&f.ref_i16).expect("decode i16 reference");
    assert_eq!(decoded.sample_rate, RATE);
    assert_samples_eq(
        &decoded.samples,
        &reference.samples,
        "FLAC-in-Ogg vs i16 WAV reference",
    );
}

/// Gapless boundary (Vorbis): two independently encoded `.ogg` files of a
/// split sine, played through the engine as a two-track queue.
///
/// "No gap" is exact — output length equals the source length to the sample —
/// and, unlike MP3, so is the *signal* at the joint: the splice error is
/// within the same [`VORBIS_TOL`] the rest of the file sits at, with no edge
/// region needing its own looser bound. That is the strong claim this test
/// exists to defend, so it is asserted at three places at once: length,
/// error against the one continuous sine both halves were cut from, and the
/// adjacent-sample step across the joint against the analytic
/// continuous-sine bound.
///
/// The splice window ([`VORBIS_EDGE_FRAMES`]) is the same one
/// [`gapless_mp3_no_gap_bounded_boundary`] measures in, so the printed
/// numbers can be read against each other directly.
#[test]
fn gapless_vorbis_ogg_no_gap_and_no_edge_artifact() {
    let Some(ogg) = ogg_or_skip() else { return };
    let mut sink = OfflineSink::with_capacity(TOTAL_FRAMES * CHANNELS);
    let report = run_playlist(
        &[ogg.part1.clone(), ogg.part2.clone()],
        test_config(),
        &mut sink,
    )
    .expect("run playlist");
    assert_eq!(report.stream_rate, RATE);
    assert_eq!(report.track_start_frames, vec![0, SPLIT_FRAME]);
    assert_eq!(
        sink.samples().len(),
        TOTAL_FRAMES * CHANNELS,
        "engine output must be exactly the source length: any untrimmed \
         lapped block or padding would show up here"
    );

    let ch0 = channel(sink.samples(), 0);
    // Error right at the joint, and error far from it, measured the same way
    // so the two numbers are comparable.
    let err_edge = max_error_vs_sine(
        &ch0,
        RATE,
        SPLIT_FRAME - VORBIS_EDGE_FRAMES..SPLIT_FRAME + VORBIS_EDGE_FRAMES,
    );
    let err_steady = max_error_vs_sine(&ch0, RATE, 0..SPLIT_FRAME - VORBIS_GUARD_FRAMES).max(
        max_error_vs_sine(&ch0, RATE, SPLIT_FRAME + VORBIS_GUARD_FRAMES..TOTAL_FRAMES),
    );
    let max_delta = assert_boundary_continuous(sink.samples(), SPLIT_FRAME, "gapless vorbis");
    let edge_db = 20.0 * f64::from(err_edge / 0.8_f32).log10();
    let steady_db = 20.0 * f64::from(err_steady / 0.8_f32).log10();
    println!(
        "[gapless-vorbis] encoder={}: output={} samples (exact, no gap); splice \
         (±{VORBIS_EDGE_FRAMES} frames) max |error| {err_edge:.2e} ({edge_db:.1} dB re. amplitude) \
         vs steady state {err_steady:.2e} ({steady_db:.1} dB) — same bound {VORBIS_TOL:.2e} \
         serves both; max adjacent delta {max_delta:.6} (continuous-sine bound {:.6})",
        ogg.encoder,
        sink.samples().len(),
        sine_adjacent_bound(RATE)
    );
    assert!(
        err_steady <= VORBIS_TOL,
        "steady-state error {err_steady} exceeds bound {VORBIS_TOL}"
    );
    assert!(
        err_edge <= VORBIS_TOL,
        "splice error {err_edge} exceeds bound {VORBIS_TOL}"
    );
    // The claim, stated without a tolerance: the joint is not a special place.
    assert!(
        err_edge <= VORBIS_EDGE_RATIO * err_steady,
        "the splice is {:.1}x the file's own steady-state error ({err_edge} vs \
         {err_steady}, limit {VORBIS_EDGE_RATIO}x): Vorbis has grown the MDCT edge \
         artifact it did not have",
        err_edge / err_steady
    );
}

/// Seeking into a Vorbis stream costs exactly one lapped block, and the cost
/// is stated rather than hidden.
///
/// Symphonia's Vorbis decoder returns an empty buffer for the first packet
/// after a reset — it has nothing to overlap-add with — so that packet's
/// audio is lost and playback resumes [`VORBIS_SEEK_LOST_FRAMES`] frames
/// late. This test pins both halves of the statement: the *length* shortfall
/// against the un-seeked decode, and the *content* offset — the audio that
/// comes back is the source at `target + 1024`, matching there and not at
/// `target`, which is what makes the shortfall a leading offset rather than a
/// stray tail.
///
/// FLAC is measured alongside as the control: the same seek on the same
/// timeline is exact, so this is a Vorbis-decoder property and not something
/// [`AudioSource::seek`] does to everyone.
#[test]
fn seek_into_vorbis_ogg_costs_one_lapped_block() {
    /// Seek target. 3 s is inside the stream and not on any packet boundary.
    const TARGET_MS: usize = 3000;
    let Some(ogg) = ogg_or_skip() else { return };
    let target = TARGET_MS * RATE as usize / 1000;

    let tail = decode_from(&ogg.full, u64::try_from(TARGET_MS).expect("fits"));
    let frames = tail.len() / CHANNELS;
    assert_eq!(
        frames,
        TOTAL_FRAMES - target - VORBIS_SEEK_LOST_FRAMES,
        "a Vorbis seek must be short by exactly one lapped block \
         ({VORBIS_SEEK_LOST_FRAMES} frames) — no more, and no less"
    );

    // Content: matches the source at target + 1024, and does not match at
    // target. The mismatch check is what makes the first assertion mean
    // "shifted", not merely "shorter".
    let ch0 = channel(&tail, 0);
    let n = 4000.min(ch0.len());
    let err_at_shift = (0..n)
        .map(|i| (ch0[i] - ideal_sample_at(RATE, target + VORBIS_SEEK_LOST_FRAMES + i, 0.0)).abs())
        .fold(0.0, f32::max);
    let err_at_target = (0..n)
        .map(|i| (ch0[i] - ideal_sample_at(RATE, target + i, 0.0)).abs())
        .fold(0.0, f32::max);
    println!(
        "[vorbis-seek] seek({TARGET_MS} ms): {frames} frames returned, \
         {VORBIS_SEEK_LOST_FRAMES} short ({:.1} ms at {RATE} Hz); max |error| vs the source \
         at target+{VORBIS_SEEK_LOST_FRAMES} = {err_at_shift:.2e} (bound {VORBIS_TOL:.2e}), \
         vs the source at target = {err_at_target:.2e}",
        frames_ms(VORBIS_SEEK_LOST_FRAMES, RATE)
    );
    assert!(
        err_at_shift <= VORBIS_TOL,
        "seeked Vorbis audio does not line up at target+{VORBIS_SEEK_LOST_FRAMES}: \
         error {err_at_shift} exceeds {VORBIS_TOL}"
    );
    assert!(
        err_at_target > 10.0 * VORBIS_TOL,
        "seeked Vorbis audio unexpectedly lines up at the target too \
         ({err_at_target}) — the measurement below cannot distinguish an offset \
         from a match, so the claim is no longer pinned"
    );

    // Control: FLAC on the same timeline loses nothing.
    let f = fixtures();
    if let Some(flac) = &f.flac {
        let flac_tail = decode_from(&flac.full, u64::try_from(TARGET_MS).expect("fits"));
        assert_eq!(
            flac_tail.len() / CHANNELS,
            TOTAL_FRAMES - target,
            "control: a FLAC seek on the same timeline must be exact"
        );
    }
}

/// **Ogg Opus bytes must be identified as Ogg Opus.**
///
/// The regression this guards is real and was shipped: before the `ogg`
/// demuxer was enabled, Symphonia's probe had no reader that claimed the
/// bytes, the AAC/ADTS prober took them instead, and a `.opus` file failed
/// with `unsupported feature: adts: only 1 aac frame per adts packet is
/// supported` — a wrong answer about what the file *is*, not merely about
/// what we can play. Probing must be content-correct even for formats baz
/// has no decoder for, because the error a user sees is the difference
/// between "baz cannot play Opus" and "this file is corrupt".
///
/// So this asserts the container-level truth directly — the probe finds a
/// track whose codec is `CODEC_TYPE_OPUS`, with no extension hint at all —
/// and then that [`AudioSource`] turns that into an honest "unsupported
/// codec" rather than an ADTS complaint.
#[test]
fn opus_bytes_probe_as_ogg_opus_and_never_as_aac() {
    let Some(ogg) = ogg_or_skip() else { return };
    let Some(opus) = &ogg.opus else {
        eprintln!("SKIP: ffmpeg has no libopus; Ogg Opus fixture not generated");
        return;
    };
    let bytes = std::fs::read(opus).expect("read opus bytes");

    // Container level: no hint, so this is purely a claim about the bytes.
    let mss = MediaSourceStream::new(
        Box::new(std::io::Cursor::new(bytes.clone())),
        MediaSourceStreamOptions::default(),
    );
    let probed = symphonia::default::get_probe()
        .format(
            &Hint::new(),
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .expect("Ogg Opus bytes must probe as a known container with no extension hint");
    let track = probed
        .format
        .default_track()
        .expect("the Ogg stream must expose a track");
    assert_eq!(
        track.codec_params.codec, CODEC_TYPE_OPUS,
        "Ogg Opus bytes must be identified as Opus, not as {:?} — a probe that \
         hands them to the AAC/ADTS reader is the bug this test exists for",
        track.codec_params.codec
    );
    // The Ogg mapper reads OpusHead, so the pre-skip is known even though we
    // cannot decode: whoever adds a decoder inherits working gapless.
    println!(
        "[opus-probe] identified as CODEC_TYPE_OPUS; pre-skip (delay) = {:?} frames, \
         rate = {:?} Hz",
        track.codec_params.delay, track.codec_params.sample_rate
    );

    // Source level: the failure a caller sees names the codec, not ADTS.
    for (what, result) in [
        ("open_bytes", AudioSource::open_bytes(bytes)),
        ("open", AudioSource::open(opus)),
    ] {
        match result {
            Err(PlaybackError::Decode(err)) => {
                let msg = err.to_string().to_ascii_lowercase();
                assert!(
                    !msg.contains("adts") && !msg.contains("aac"),
                    "{what}: Opus bytes were claimed by the AAC prober again: {msg}"
                );
                assert!(
                    msg.contains("codec"),
                    "{what}: the failure must name the missing codec, got: {msg}"
                );
            }
            Err(other) => panic!("{what}: expected a decode error for Opus, got {other}"),
            Ok(_) => panic!(
                "{what}: Opus now decodes. That is good news, and it means \
                 AUDIO_EXTENSIONS should regain \"opus\", AudioFormat::is_decodable \
                 should stop excluding it, and docs/BACKLOG.md's Opus entry should be \
                 closed — see the reasoning recorded there"
            ),
        }
    }
}

/// **The invariant: every extension the shelf advertises really decodes.**
///
/// `AUDIO_EXTENSIONS` is a promise to the listener — a track that appears on
/// the shelf plays when clicked. The `.m4a` bug and then the `.ogg`/`.opus`
/// bug were both the same failure of that promise: an extension was added to
/// the scanner and no decoder was ever wired up behind it. Nothing in the
/// type system connects the two lists, so this test connects them, against
/// real encoded audio rather than a hard-coded table.
///
/// Every entry in `AUDIO_EXTENSIONS` must have a fixture here (an unmapped
/// extension fails the test rather than being quietly skipped, which is how a
/// future format gets caught), and every fixture must open, report a sane
/// rate and length, and decode to audio.
#[test]
fn every_advertised_extension_decodes() {
    let f = fixtures();
    let Some(ogg) = &f.ogg else {
        eprintln!("SKIP: ffmpeg is not available; per-extension fixtures not generated");
        return;
    };
    let (Some(flac), Some(mp3), Some(m4a)) = (&f.flac, &f.mp3, &f.m4a) else {
        eprintln!("SKIP: ffmpeg lacks one of libmp3lame/flac/alac/aac; fixtures not generated");
        return;
    };

    for ext in AUDIO_EXTENSIONS {
        // One representative file per advertised extension. Where an
        // extension covers several codecs, the entry here is the one that
        // was *not* already covered by a dedicated test above, so this stays
        // a breadth check rather than a duplicate of the depth ones.
        let fixture: &Path = match *ext {
            "wav" => &f.ref_f32,
            "flac" => &flac.full,
            "mp3" => &mp3.full,
            "ogg" => &ogg.full,
            "m4a" => &m4a.alac.full,
            "mp4" => &m4a.video_first,
            other => panic!(
                "AUDIO_EXTENSIONS advertises `.{other}` and this test has no fixture for it. \
                 Add one — an extension with no proven decoder is exactly the bug this \
                 test exists to prevent."
            ),
        };
        assert_eq!(
            fixture.extension().and_then(|e| e.to_str()),
            Some(*ext),
            "fixture for `.{ext}` must actually be a .{ext} file"
        );

        let mut src = AudioSource::open(fixture)
            .unwrap_or_else(|e| panic!("`.{ext}` is advertised but will not open: {e}"));
        assert_eq!(
            src.sample_rate(),
            RATE,
            "`.{ext}`: decoder must report the source rate"
        );
        assert!(
            src.total_frames().is_some_and(|n| n > 0),
            "`.{ext}`: no declared length, so the shelf could not show a duration"
        );
        let mut frames = 0usize;
        let mut peak = 0.0_f32;
        while let Some(block) = src
            .next_block()
            .unwrap_or_else(|e| panic!("`.{ext}` is advertised but will not decode: {e}"))
        {
            frames += block.len() / CHANNELS;
            peak = block.iter().fold(peak, |m, &s| m.max(s.abs()));
        }
        assert!(
            frames > RATE as usize,
            "`.{ext}`: decoded only {frames} frames — that is not a playable track"
        );
        assert!(
            peak > 0.5,
            "`.{ext}`: decoded {frames} frames of near-silence (peak {peak}); \
             the file opened but the audio did not survive"
        );
        println!("[extension-invariant] .{ext}: {frames} frames, peak {peak:.3} — plays");
    }
}

// ---------------------------------------------------------------------------
// MP4 / .m4a — ALAC (lossless) and AAC (lossy)
// ---------------------------------------------------------------------------

/// Playing time of `frames` frames at `rate` Hz, in milliseconds.
#[allow(clippy::cast_precision_loss)] // frame counts are far below 2^52
fn frames_ms(frames: usize, rate: u32) -> f64 {
    1000.0 * frames as f64 / f64::from(rate)
}

/// The `.m4a` fixture set, or `None` with a skip notice on the toolbox and
/// any other machine without ffmpeg — the same contract the MP3 tests keep.
fn m4a_or_skip() -> Option<&'static M4aFixtures> {
    let m4a = fixtures().m4a.as_ref();
    if m4a.is_none() {
        eprintln!(
            "SKIP: ffmpeg with the alac and aac encoders is not available; \
             m4a fixtures not generated"
        );
    }
    m4a
}

/// Encoder delay Symphonia leaves at the head of an AAC-in-MP4 stream,
/// in frames, for the fixture encoder.
///
/// Not a choice of ours and not a tolerance: it is the *measured* consequence
/// of Symphonia 0.5's ISO-MP4 reader applying neither the edit list (`elst`)
/// nor iTunes' `iTunSMPB` atom — the only two places an MP4 records encoder
/// delay. ffmpeg's native AAC encoder primes with exactly one 1024-sample
/// frame, and every one of those samples is emitted as audio. Other encoders
/// prime differently (Apple's AAC uses 2112 frames), so the *number* is
/// encoder-specific; that the delay survives untrimmed is not.
///
/// [`aac_m4a_delay_is_untrimmed_and_measured`] pins this against the source
/// so the claim in the [`baz_core::playback`] docs cannot silently rot.
const AAC_UNTRIMMED_DELAY_FRAMES: usize = 1024;

/// One AAC-LC frame at 44.1 kHz, and the width of the codec edge region
/// excluded from steady-state content comparisons.
const AAC_FRAME: usize = 1024;

/// Steady-state accuracy bound for the 256 kbps AAC fixture, as max |error|
/// vs the ideal sine (amplitude 0.8) once the delay is accounted for.
/// Measured 1.87e-2 (−32.6 dB re. amplitude) with ffmpeg 8.1's *native* AAC
/// encoder, which is markedly less accurate on a pure tone than LAME at
/// 320 kbps (cf. [`MP3_STEADY_TOL`]); pinned at ~3x so a decode regression
/// fails while encoder-version noise does not. A *fixture* bound, not a
/// general AAC quality claim.
///
/// The bound is loose enough that it earns its keep only alongside the
/// deliberately-misaligned comparison in
/// [`aac_m4a_delay_is_untrimmed_and_measured`]: unshifted, the same
/// measurement reads 1.01e0 — 54x larger — so a change in the delay is
/// unmissable even at this tolerance.
const AAC_STEADY_TOL: f32 = 6.0e-2;

/// ALAC in MP4 is lossless and its container carries an exact frame count:
/// decoding must reproduce the i16 WAV it was encoded from, sample for
/// sample, with no length fudge at either end. This is the same bar FLAC is
/// held to — and the reason the docs may call ALAC gapless "exact".
///
/// Also drives the hint-less `open_bytes` path (the fuzz target's entry
/// point), proving MP4 probes by content and not merely by file extension.
#[test]
fn alac_m4a_is_lossless_and_length_exact() {
    let Some(m4a) = m4a_or_skip() else { return };
    let alac = &m4a.alac;
    let decoded = AudioSource::decode_all(&alac.full).expect("decode alac m4a");
    let reference = AudioSource::decode_all(&fixtures().ref_i16).expect("decode i16 reference");
    assert_eq!(decoded.sample_rate, RATE);
    assert_eq!(
        decoded.frames(),
        TOTAL_FRAMES,
        "ALAC in MP4 must decode to exactly the source frame count"
    );
    assert_samples_eq(
        &decoded.samples,
        &reference.samples,
        "ALAC vs i16 WAV reference (lossless means bit-exact)",
    );

    let p1 = AudioSource::decode_all(&alac.part1).expect("decode alac part1");
    let p2 = AudioSource::decode_all(&alac.part2).expect("decode alac part2");
    assert_eq!(p1.frames(), SPLIT_FRAME, "alac part1 length");
    assert_eq!(p2.frames(), TOTAL_FRAMES - SPLIT_FRAME, "alac part2 length");

    // The declared length is the emitted length, so the UI's duration and the
    // engine's frame budget agree with the audio.
    let src = AudioSource::open(&alac.full).expect("open alac");
    assert_eq!(src.total_frames(), Some(TOTAL_FRAMES as u64));
    assert_eq!(src.duration_ms(), Some(10_000));

    // Hint-less in-memory probe: the MP4 `ftyp` brand must be enough.
    let bytes = std::fs::read(&alac.full).expect("read alac bytes");
    let mut src = AudioSource::open_bytes(bytes).expect("m4a must probe with no extension hint");
    assert_eq!(src.sample_rate(), RATE);
    let mut frames = 0usize;
    while let Some(block) = src.next_block().expect("decode block") {
        frames += block.len() / CHANNELS;
    }
    assert_eq!(
        frames, TOTAL_FRAMES,
        "open_bytes path must decode identically"
    );
    println!(
        "[alac-m4a] encoder={}: {} frames, bit-exact vs the i16 WAV source; \
         probed by content with no extension hint",
        alac.encoder,
        decoded.frames()
    );
}

/// AAC in MP4 plays, and the encoder delay is **not** trimmed — measured, not
/// hand-waved.
///
/// Symphonia 0.5's ISO-MP4 reader reads neither the edit list nor `iTunSMPB`,
/// the only two places an MP4 records encoder delay, so the priming frames
/// come out as audio at the head of every AAC track. This test pins both
/// halves of that statement: the *length* excess against the source, and the
/// *content* alignment — the decoded sine matches the source when shifted by
/// exactly the delay and does not match when it is not, which is what makes
/// the excess a leading offset rather than a stray tail.
#[test]
fn aac_m4a_delay_is_untrimmed_and_measured() {
    let Some(m4a) = m4a_or_skip() else { return };
    let aac = &m4a.aac;
    let decoded = AudioSource::decode_all(&aac.full).expect("decode aac m4a");
    assert_eq!(decoded.sample_rate, RATE);
    let frames = decoded.frames();
    let excess = frames.saturating_sub(TOTAL_FRAMES);
    println!(
        "[aac-m4a] encoder={}: decoded {frames} frames vs {TOTAL_FRAMES} in the source \
         — +{excess} frames ({:.2} ms) of untrimmed encoder delay",
        aac.encoder,
        frames_ms(excess, RATE)
    );
    assert_eq!(
        frames,
        TOTAL_FRAMES + AAC_UNTRIMMED_DELAY_FRAMES,
        "AAC decode length changed: the documented gapless claim in \
         baz_core::playback must be re-measured"
    );

    // Content: the sine is the source's, offset by exactly the delay. Compare
    // over the steady state, one AAC frame in from each end.
    let ch0 = channel(&decoded.samples, 0);
    let mut aligned = 0.0f32;
    let mut unaligned = 0.0f32;
    for n in AAC_FRAME..TOTAL_FRAMES - AAC_FRAME {
        let ideal = ideal_sample_at(RATE, n, 0.0);
        aligned = aligned.max((ch0[n + AAC_UNTRIMMED_DELAY_FRAMES] - ideal).abs());
        unaligned = unaligned.max((ch0[n] - ideal).abs());
    }
    println!(
        "[aac-m4a] steady-state max |error| vs the ideal sine: {aligned:.2e} when \
         shifted by {AAC_UNTRIMMED_DELAY_FRAMES} frames, {unaligned:.2e} unshifted \
         (bound {AAC_STEADY_TOL:.2e})"
    );
    assert!(
        aligned <= AAC_STEADY_TOL,
        "AAC content does not match the source when shifted by the measured \
         delay ({aligned} > {AAC_STEADY_TOL})"
    );
    assert!(
        unaligned > AAC_STEADY_TOL,
        "the unshifted comparison also passed, so this test would not notice \
         a delay change at all"
    );

    // Hint-less probe, same as ALAC: content-based, and the same length.
    let bytes = std::fs::read(&aac.full).expect("read aac bytes");
    let mut src = AudioSource::open_bytes(bytes).expect("m4a must probe with no extension hint");
    let mut frames = 0usize;
    while let Some(block) = src.next_block().expect("decode block") {
        frames += block.len() / CHANNELS;
    }
    assert_eq!(frames, decoded.frames(), "open_bytes path must agree");
}

/// Gapless (ALAC): two `.m4a` files through the engine reconstruct the
/// reference exactly — the FLAC/WAV bar, met by a second lossless container.
#[test]
fn gapless_alac_m4a_bit_exact() {
    let Some(m4a) = m4a_or_skip() else { return };
    let alac = &m4a.alac;
    let reference = AudioSource::decode_all(&fixtures().ref_i16).expect("decode i16 reference");
    let mut sink = OfflineSink::with_capacity(reference.samples.len());
    let report = run_playlist(
        &[alac.part1.clone(), alac.part2.clone()],
        test_config(),
        &mut sink,
    )
    .expect("run playlist");
    assert_eq!(report.stream_rate, RATE);
    assert_eq!(report.track_start_frames, vec![0, SPLIT_FRAME]);
    assert_samples_eq(sink.samples(), &reference.samples, "gapless ALAC output");
    let max_delta = assert_boundary_continuous(sink.samples(), SPLIT_FRAME, "gapless ALAC");
    println!(
        "[gapless-alac] encoder={}, output={} samples (exact); boundary max adjacent \
         delta={max_delta:.6} (bound {:.6})",
        alac.encoder,
        sink.samples().len(),
        sine_adjacent_bound(RATE)
    );
}

/// Gapless (AAC): two `.m4a` files through the engine play cleanly and the
/// gap is exactly the untrimmed delay of the *second* track — measured here
/// at the seam so the number in the docs is enforced end to end, not just at
/// the decoder.
///
/// This is the honest AAC story: the engine's own splice is still
/// bookkeeping-only (nothing is dropped, nothing is duplicated), but the
/// second file arrives with [`AAC_UNTRIMMED_DELAY_FRAMES`] frames of encoder
/// priming in front of its music, and that is audible as a short gap. FLAC,
/// WAV, ALAC and LAME-tagged MP3 have none.
#[test]
fn gapless_aac_m4a_carries_the_measured_delay() {
    let Some(m4a) = m4a_or_skip() else { return };
    let aac = &m4a.aac;
    let p1 = AudioSource::decode_all(&aac.part1).expect("decode aac part1");
    let p2 = AudioSource::decode_all(&aac.part2).expect("decode aac part2");
    let expected = p1.frames() + p2.frames();

    let mut sink = OfflineSink::with_capacity(expected * CHANNELS);
    let report = run_playlist(
        &[aac.part1.clone(), aac.part2.clone()],
        test_config(),
        &mut sink,
    )
    .expect("run playlist");
    assert_eq!(report.stream_rate, RATE);
    assert_eq!(report.track_start_frames, vec![0, p1.frames()]);
    // The engine itself neither drops nor duplicates: output is exactly the
    // two decodes concatenated.
    assert_eq!(
        sink.samples().len(),
        expected * CHANNELS,
        "engine output must be exactly the two decoded lengths concatenated"
    );

    // Each track carries the delay, so the total overshoots the source by
    // twice it. That is the number a two-track AAC album pays.
    let gap_ms = frames_ms(AAC_UNTRIMMED_DELAY_FRAMES, RATE);
    assert_eq!(
        expected,
        TOTAL_FRAMES + 2 * AAC_UNTRIMMED_DELAY_FRAMES,
        "each AAC track must carry exactly one untrimmed delay"
    );

    // The seam: the second track's music starts `delay` frames after the
    // boundary, so the source sine resumes there and not at the boundary.
    let ch0 = channel(sink.samples(), 0);
    let seam = p1.frames() + AAC_UNTRIMMED_DELAY_FRAMES;
    let mut worst = 0.0f32;
    for n in SPLIT_FRAME + AAC_FRAME..TOTAL_FRAMES - AAC_FRAME {
        worst = worst.max((ch0[seam + n - SPLIT_FRAME] - ideal_sample_at(RATE, n, 0.0)).abs());
    }
    println!(
        "[gapless-aac] encoder={}: output={} frames = source {} + 2x{} delay; \
         per-track gap {AAC_UNTRIMMED_DELAY_FRAMES} frames ({gap_ms:.2} ms at {RATE} Hz); \
         second track's steady-state max |error| vs the source sine once the delay is \
         skipped: {worst:.2e} (bound {AAC_STEADY_TOL:.2e}). FLAC/WAV/ALAC gap: 0.",
        aac.encoder, expected, TOTAL_FRAMES, AAC_UNTRIMMED_DELAY_FRAMES,
    );
    assert!(
        worst <= AAC_STEADY_TOL,
        "second AAC track is not the source audio offset by the delay: \
         max error {worst} exceeds {AAC_STEADY_TOL}"
    );
}

/// A mixed `.m4a` queue (ALAC then AAC) runs end to end: the two MP4 codec
/// paths coexist in one session and the totals are each format's own
/// documented length, no crash and no cross-contamination.
#[test]
fn mixed_m4a_queue_plays_through() {
    let Some(m4a) = m4a_or_skip() else { return };
    let lossless_frames = AudioSource::decode_all(&m4a.alac.part1)
        .expect("decode alac part1")
        .frames();
    let lossy_frames = AudioSource::decode_all(&m4a.aac.part2)
        .expect("decode aac part2")
        .frames();
    assert_eq!(lossless_frames, SPLIT_FRAME);
    assert_eq!(
        lossy_frames,
        TOTAL_FRAMES - SPLIT_FRAME + AAC_UNTRIMMED_DELAY_FRAMES
    );

    let total = lossless_frames + lossy_frames;
    let mut sink = OfflineSink::with_capacity(total * CHANNELS);
    let report = run_playlist(
        &[m4a.alac.part1.clone(), m4a.aac.part2.clone()],
        test_config(),
        &mut sink,
    )
    .expect("run playlist");
    assert_eq!(report.stream_rate, RATE);
    assert_eq!(report.track_start_frames, vec![0, lossless_frames]);
    assert_eq!(sink.samples().len(), total * CHANNELS);
    assert_eq!(sink.dropped_samples(), 0);
    println!("[mixed-m4a] ALAC ({lossless_frames}) + AAC ({lossy_frames}) = {total} frames");
}

/// Mean rate of zero crossings per second on a mono signal — a
/// dependency-free pitch check. A sinusoid at f Hz crosses zero 2f times a
/// second regardless of amplitude, so this distinguishes "played at the right
/// rate" from "played an octave up" without an FFT.
fn zero_crossing_rate(mono: &[f32], rate: u32) -> f64 {
    let crossings = mono
        .windows(2)
        .filter(|w| (w[0] < 0.0) != (w[1] < 0.0))
        .count();
    #[allow(clippy::cast_precision_loss)] // sample counts are far below 2^52
    let seconds = mono.len() as f64 / f64::from(rate);
    #[allow(clippy::cast_precision_loss)] // ditto
    let rate = crossings as f64 / seconds;
    rate
}

/// HE-AAC (SBR) is the format a streaming rip actually is, and Symphonia 0.5
/// implements no SBR: it decodes the AAC-LC *core*, which is at half the rate
/// the MP4 sample entry advertises. Believing the container would play that
/// core an octave up at double speed; [`AudioSource`] takes the decoder's
/// rate instead and rescales the declared length through the same ratio.
///
/// This test is the guard on that reconciliation. It asserts the two things
/// a listener would notice: the **pitch** is the source's (measured by zero
/// crossings — 880/s for the 440 Hz test tone, not 1760/s), and the
/// **duration** is the source's, so seek bars and track lengths are right.
/// The bandwidth loss from the missing SBR band is real and documented in
/// [`baz_core::playback`]; it is not something this test can hide.
#[test]
fn he_aac_m4a_plays_at_the_core_rate_not_the_container_rate() {
    let Some(m4a) = m4a_or_skip() else { return };
    let Some(he) = &m4a.he_aac else {
        eprintln!("SKIP: ffmpeg has no libfdk_aac; HE-AAC fixture not generated");
        return;
    };
    let src = AudioSource::open(he).expect("open he-aac m4a");
    let rate = src.sample_rate();
    assert_eq!(
        rate,
        RATE / 2,
        "Symphonia decodes the AAC-LC core of an HE-AAC stream, which is at \
         half the rate the MP4 sample entry declares"
    );
    let decoded = AudioSource::decode_all(he).expect("decode he-aac m4a");
    assert_eq!(decoded.sample_rate, rate);
    assert_eq!(
        decoded.frames() as u64,
        src.total_frames().expect("declared length"),
        "the rescaled declared length must be the length actually emitted"
    );

    // Duration: the source is 10 s, and the only legitimate excess is the
    // encoder delay this format does not trim (see the AAC test). A rate
    // taken from the container instead of the decoder would read 20 s here.
    let secs = frames_ms(decoded.frames(), rate) / 1000.0;
    assert!(
        (10.0..10.5).contains(&secs),
        "decoded duration {secs:.3} s is not the source's 10 s plus untrimmed delay"
    );

    // Pitch: 440 Hz means 880 zero crossings per second. Measured over the
    // interior only, so the delay's near-silent priming frames cannot skew
    // the count. A rate error of 2x would read ~1760.
    let ch0 = channel(&decoded.samples, 0);
    let interior = &ch0[rate as usize..ch0.len() - rate as usize];
    let zcr = zero_crossing_rate(interior, rate);
    println!(
        "[he-aac] decoded {} frames at {rate} Hz ({secs:.3} s; container declared {RATE} Hz); \
         zero-crossing rate {zcr:.1}/s (440 Hz tone => 880/s)",
        decoded.frames()
    );
    assert!(
        (860.0..900.0).contains(&zcr),
        "zero-crossing rate {zcr:.1}/s is not a 440 Hz tone: the stream is \
         being played at the wrong rate"
    );
}

/// A `.mp4` whose first track is video still plays its audio.
///
/// `AUDIO_EXTENSIONS` accepts `mp4`, and Symphonia's `default_track` is
/// simply the container's first track — which in a `.mp4` is the video. The
/// source picks the first track that declares a sample rate instead, so the
/// shelf does not list a file it then refuses with "stream does not declare a
/// sample rate".
#[test]
fn video_first_mp4_plays_its_audio_track() {
    let Some(m4a) = m4a_or_skip() else { return };
    let decoded = AudioSource::decode_all(&m4a.video_first).expect("decode video-first mp4");
    assert_eq!(decoded.sample_rate, RATE);
    assert_eq!(
        decoded.frames(),
        TOTAL_FRAMES + AAC_UNTRIMMED_DELAY_FRAMES,
        "the audio track must decode in full (AAC's untrimmed delay included)"
    );
    // It is the right audio, not the video track misread as samples.
    let ch0 = channel(&decoded.samples, 0);
    let mut worst = 0.0f32;
    for n in AAC_FRAME..TOTAL_FRAMES - AAC_FRAME {
        worst =
            worst.max((ch0[n + AAC_UNTRIMMED_DELAY_FRAMES] - ideal_sample_at(RATE, n, 0.0)).abs());
    }
    println!(
        "[video-first-mp4] audio track decoded: {} frames, steady-state max |error| \
         vs the source sine {worst:.2e} (bound {AAC_STEADY_TOL:.2e})",
        decoded.frames()
    );
    assert!(
        worst <= AAC_STEADY_TOL,
        "decoded audio is not the source sine"
    );
}

/// Seeking inside a lossless MP4 is sample-exact, the same contract WAV and
/// FLAC meet: the MP4 sample table lands on a packet boundary at or before
/// the target and the residue is discarded, so what comes back is bit-for-bit
/// the tail of the un-seeked decode.
#[test]
fn seek_into_alac_m4a_is_sample_exact() {
    let Some(m4a) = m4a_or_skip() else { return };
    let path = m4a.alac.full.as_path();
    let reference = AudioSource::decode_all(path)
        .expect("reference decode")
        .samples;
    assert_eq!(reference.len(), TOTAL_FRAMES * CHANNELS);
    for position_ms in [0u64, 1, 3_000, 9_999] {
        let target = (position_ms * u64::from(RATE) / 1000) as usize;
        let got = decode_from(path, position_ms);
        assert_samples_eq(
            &got,
            &reference[target * CHANNELS..],
            &format!("alac m4a: decode after seek to {position_ms} ms"),
        );
    }
    println!("[seek alac-m4a] sample-exact at 0/1/3000/9999 ms");
}

/// Decode-ahead: track N+1 is demonstrably decoded while track N is still
/// draining (prefetch overlap evidence).
///
/// The consumer is paced slower here than in [`test_config`] — 2 ms per
/// 2048-frame chunk instead of 500 µs. That is still **23x faster than
/// realtime** (2048 frames at 44.1 kHz is 46.4 ms of audio), so the claim
/// being made is unchanged and remains far stricter than any real playback:
/// the prefetch has to finish track 2 inside ~216 ms of wall clock rather
/// than the 10.2 s a device would give it. The slower pace only stops the
/// assertion from turning into a measurement of how many other test threads
/// happen to be decoding at the same moment — at 500 µs the whole margin was
/// ~30 ms on an idle machine, which a saturated CPU erases.
#[test]
fn decode_ahead_overlaps_playback() {
    let f = fixtures();
    let cfg = EngineConfig {
        consumer_pace: Duration::from_millis(2),
        ..test_config()
    };
    let mut sink = OfflineSink::with_capacity(TOTAL_FRAMES * CHANNELS);
    let report = run_playlist(&[f.part1_f32.clone(), f.part2_f32.clone()], cfg, &mut sink)
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

/// Resample boundary, under the explicit fixed-rate opt-in
/// ([`BoundaryPolicy::ResampleToStreamRate`] — ADR-0004's default, demoted by
/// ADR-0009): a 48 kHz track after a 44.1 kHz track is resampled to the stream
/// rate on the prefetch side and spliced seamlessly. The sine must continue
/// through the boundary at −45 dB error or better (Spike B measured −45.5 dB),
/// and the output duration must be exact.
///
/// Demoting the policy did not weaken this guarantee: the mode is still
/// reachable, is what a device that cannot do the source rate falls back to,
/// and must still splice cleanly when it does. Only the config line below
/// changed.
#[test]
fn resample_boundary_is_continuous() {
    let f = fixtures();
    let cfg = EngineConfig {
        boundary: BoundaryPolicy::ResampleToStreamRate,
        ..test_config()
    };
    // The 48 kHz half resamples to exactly 220_500 frames at 44.1 kHz.
    let expected_frames = RATE_PAIR_FRAMES_44K + RATE_PAIR_FRAMES_44K;
    let mut sink = OfflineSink::with_capacity(expected_frames * CHANNELS);
    let report = run_playlist(&[f.rate_44k.clone(), f.rate_48k.clone()], cfg, &mut sink)
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

/// Under the bit-perfect default (ADR-0009) `run_playlist` refuses a queue
/// that changes sample rate rather than converting it: a single-buffer
/// one-shot render has nowhere to put a reopen, and converting behind the
/// caller's back is exactly what the default exists to prevent.
///
/// The refusal must name the position and both rates — a caller who hits this
/// has to be able to see *which* track diverged and to what.
#[test]
fn a_rate_change_is_refused_by_the_bit_perfect_default() {
    let f = fixtures();
    let mut sink = OfflineSink::with_capacity(16);
    let err = run_playlist(
        &[f.rate_44k.clone(), f.rate_48k.clone()],
        test_config(),
        &mut sink,
    )
    .expect_err("a rate change must not be silently converted");
    assert!(
        matches!(
            err,
            PlaybackError::SampleRateChangeRequiresReopen {
                index: 1,
                from: RATE,
                to: 48_000,
            }
        ),
        "wrong refusal: {err}"
    );
    let text = err.to_string();
    for expected in ["track 1", "44100", "48000"] {
        assert!(
            text.contains(expected),
            "the error must state {expected}: {text}"
        );
    }
}

/// The same queue under the explicit fixed-rate opt-in converts and plays —
/// the mode is still there, it just is not what an unconfigured baz does.
/// (`resample_boundary_is_continuous` measures the quality of that path; this
/// only pins that selecting it is what turns refusal into conversion.)
#[test]
fn the_fixed_rate_opt_in_converts_the_same_queue() {
    let f = fixtures();
    let cfg = EngineConfig {
        boundary: BoundaryPolicy::ResampleToStreamRate,
        ..test_config()
    };
    let expected_frames = RATE_PAIR_FRAMES_44K * 2;
    let mut sink = OfflineSink::with_capacity(expected_frames * CHANNELS);
    let report = run_playlist(&[f.rate_44k.clone(), f.rate_48k.clone()], cfg, &mut sink)
        .expect("the fixed-rate mode must convert rather than refuse");
    assert_eq!(report.stream_rate, RATE);
    assert!(
        report.resample_ms.is_some(),
        "the fixed-rate mode must actually have resampled"
    );
    assert_eq!(sink.samples().len(), expected_frames * CHANNELS);
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
            // 50 ms of silence: the smoke check is that the stream takes
            // samples and does not fault, which is true of any samples.
            baz_core::playback::Sink::write(&mut sink, &silence_stereo(2205));
            assert!(!sink.failed(), "stream reported an error during smoke run");
            println!("[device] opened default output device and wrote 50 ms");
        }
        Err(PlaybackError::Device(msg)) => {
            eprintln!("SKIP: no usable output device ({msg})");
        }
        Err(other) => panic!("unexpected error opening device: {other}"),
    }
}

/// **Device output (feature `device-output`): a 48 kHz stream really opens on
/// this machine's hardware and carries samples.**
///
/// ADR-0009's whole premise is that the owner's 24-bit/48 kHz album can be
/// played at 48 kHz instead of converted to 44.1 kHz. That premise is a claim
/// about hardware, so it is tested against hardware: open at 48 kHz, hand it
/// samples, and check the stream neither refused nor faulted. A device with no
/// 48 kHz mode is a documented outcome, not a failure — it is exactly the
/// fallback case — so it skips with a notice.
///
/// The samples are silence ([`silence_stereo`]): a 48 kHz stream carries them
/// exactly as it would carry a tone, and the audible version of this claim is
/// `device_engine_follows_the_source_rate` in `tests/engine.rs`, which plays a
/// real 48 kHz file and is opt-in.
#[cfg(feature = "device-output")]
#[test]
fn device_sink_opens_at_48k_and_accepts_audio() {
    use baz_core::playback::device::DeviceSink;
    const HI_RATE: u32 = 48_000;
    match DeviceSink::open(HI_RATE, 8192) {
        Ok(mut sink) => {
            assert_eq!(sink.sample_rate(), HI_RATE);
            // 50 ms at the stream's own rate.
            baz_core::playback::Sink::write(&mut sink, &silence_stereo(HI_RATE as usize / 20));
            assert!(!sink.failed(), "stream reported an error during smoke run");
            println!("[device] opened the default output device at {HI_RATE} Hz and wrote 50 ms");
        }
        Err(PlaybackError::Device(msg)) => {
            eprintln!("SKIP: no usable 48 kHz output ({msg})");
        }
        Err(other) => panic!("unexpected error opening device: {other}"),
    }
}

/// **Device output (feature `device-output`): reopening at a new rate works,
/// and this is where the rate-change gap is measured.**
///
/// A rate change costs a device reconfiguration, which ADR-0009 accepts and
/// therefore has to quantify. `negotiate_rate` is that reconfiguration —
/// tearing down one cpal stream and building another — so timing it *is* the
/// measurement, and it is taken on real hardware rather than estimated.
///
/// The assertions are the correctness half (the stream really is at the new
/// rate, and really is alive afterwards); the number is printed for the ADR.
/// The bound is deliberately loose — this measures a device, not our code, and
/// a slow host must not turn into a red build — but it is tight enough that a
/// reopen which silently fell back or hung would fail.
#[cfg(feature = "device-output")]
#[test]
fn device_sink_reopens_at_the_requested_rate() {
    use std::time::Instant;

    use baz_core::playback::Sink as _;
    use baz_core::playback::device::DeviceSink;
    const HI_RATE: u32 = 48_000;
    /// Loose: a device reconfiguration on a busy host still has to beat this.
    const REOPEN_BUDGET: Duration = Duration::from_millis(1_000);

    let mut sink = match DeviceSink::open(RATE, 8192) {
        Ok(sink) => sink,
        Err(PlaybackError::Device(msg)) => {
            eprintln!("SKIP: no usable output device ({msg})");
            return;
        }
        Err(other) => panic!("unexpected error opening device: {other}"),
    };
    assert_eq!(sink.sample_rate(), RATE);

    let t0 = Instant::now();
    let granted = sink.negotiate_rate(HI_RATE).expect("a device has a rate");
    let reopen = t0.elapsed();

    if granted != HI_RATE {
        eprintln!("SKIP: this device offers no {HI_RATE} Hz mode (it answered {granted} Hz)");
        return;
    }
    assert_eq!(
        sink.sample_rate(),
        HI_RATE,
        "the sink must report the rate it actually reopened at"
    );
    assert!(
        reopen < REOPEN_BUDGET,
        "reopening at {HI_RATE} Hz took {reopen:?}"
    );

    // The new stream is alive and takes audio at the new rate.
    sink.write(&silence_stereo(HI_RATE as usize / 20));
    assert!(!sink.failed(), "the reopened stream reported an error");

    // Asking for the rate it is already at must cost nothing at all — that is
    // what makes following the source free within an album.
    let t1 = Instant::now();
    assert_eq!(sink.negotiate_rate(HI_RATE), Some(HI_RATE));
    let noop = t1.elapsed();
    assert!(
        noop < Duration::from_millis(10),
        "re-requesting the open rate must not reopen anything, took {noop:?}"
    );

    println!(
        "[device] rate change {RATE} Hz -> {HI_RATE} Hz reopened in {:.1} ms; \
         re-requesting the same rate took {:.3} ms",
        reopen.as_secs_f64() * 1e3,
        noop.as_secs_f64() * 1e3,
    );
}

/// Device output (feature `device-output`): `Sink::discard_buffered` really
/// empties the device ring, and does it far faster than the buffered audio
/// would have taken to play.
///
/// This is the test that pins the seek-latency fix at the place the bug
/// lived. The engine has always dropped its *own* undelivered audio when a
/// session is abandoned; what it could not drop was the copy already handed
/// to the device, which kept playing the old position for up to a full ring.
///
/// The ring here is deliberately sized at one full second so the two possible
/// explanations are impossible to confuse: if the audio merely played out
/// normally the ring could not be empty for ~1000 ms, so emptying inside
/// [`DISCARD_SETTLE_BUDGET`] proves it was *discarded*. The follow-up write
/// then shows the ring holds the new audio and nothing else — no stale
/// residue behind it.
#[cfg(feature = "device-output")]
#[test]
fn discard_buffered_empties_the_device_ring() {
    use std::time::Instant;

    use baz_core::playback::Sink as _;
    use baz_core::playback::device::DeviceSink;

    /// One second of ring, so "discarded" and "played out" differ by 20x.
    const RING_FRAMES: usize = RATE as usize;
    /// Generous next-callback allowance: this host's cpal callback period is
    /// ~43 ms (one 100 ms priming call at stream start), and the discard is
    /// honoured on the first callback that observes it.
    const DISCARD_SETTLE_BUDGET: Duration = Duration::from_millis(400);

    /// Playing time of `samples` interleaved samples at [`RATE`], in ms.
    #[allow(clippy::cast_precision_loss)] // sample counts are far below 2^52
    fn audio_ms(samples: usize) -> f64 {
        1000.0 * (samples / CHANNELS) as f64 / f64::from(RATE)
    }

    let mut sink = match DeviceSink::open(RATE, RING_FRAMES) {
        Ok(sink) => sink,
        Err(PlaybackError::Device(msg)) => {
            eprintln!("SKIP: no usable output device ({msg})");
            return;
        }
        Err(other) => panic!("unexpected error opening device: {other}"),
    };

    // Fill the ring with a full second of audio. What the audio *is* does not
    // enter into it: the measurement is how fast the ring empties.
    let stale = silence_stereo(RING_FRAMES);
    sink.write(&stale);
    let buffered = sink.buffered_samples();
    assert!(
        buffered > RING_FRAMES,
        "the ring must actually be holding audio for the discard to mean anything \
         (only {buffered} of {} samples buffered)",
        RING_FRAMES * CHANNELS
    );
    assert!(!sink.discard_pending(), "no discard has been requested yet");

    // The whole producer side of the mechanism: one atomic store, no wait.
    let t0 = Instant::now();
    sink.discard_buffered();
    assert!(
        sink.discard_pending() || sink.buffered_samples() == 0,
        "a discard is either still pending or already honoured"
    );
    while sink.discard_pending() && t0.elapsed() < DISCARD_SETTLE_BUDGET {
        std::thread::sleep(Duration::from_millis(1));
    }
    let settled = t0.elapsed();

    assert!(
        !sink.discard_pending(),
        "the callback did not honour the discard within {DISCARD_SETTLE_BUDGET:?}"
    );
    assert_eq!(
        sink.buffered_samples(),
        0,
        "the device ring must be empty after a discard"
    );
    assert!(
        settled < Duration::from_millis(1000),
        "{buffered} samples ({:.0} ms of audio) took {settled:?} to clear — that is \
         long enough to have simply played out, which is the bug, not the fix",
        audio_ms(buffered)
    );

    // Nothing stale is lurking behind the new audio: the ring now holds
    // exactly what was written after the discard.
    let fresh = silence_stereo(1024);
    sink.write(&fresh);
    let after = sink.buffered_samples();
    assert!(
        after <= fresh.len(),
        "the ring holds {after} samples after writing {} fresh ones — stale audio \
         survived the discard",
        fresh.len()
    );
    assert!(
        !sink.failed(),
        "stream reported an error during the discard"
    );
    println!(
        "[device] discarded {buffered} buffered samples ({:.0} ms of audio) in {settled:?}",
        audio_ms(buffered)
    );
}

/// **Opening the output from a thread that then exits must not poison the next
/// open.** This is the regression test for the Windows
/// `STATUS_ACCESS_VIOLATION` of CI run 31227392558.
///
/// baz opens its device on the engine thread — cpal streams are not `Send` —
/// and that thread exits when the engine shuts down. cpal's WASAPI backend
/// caches a process-global `IMMDeviceEnumerator` created inside the *apartment*
/// of whichever thread touched cpal first, while its COM initialisation is
/// thread-local and calls `CoUninitialize()` from a thread-local destructor. So
/// the first engine thread to exit tore down the apartment underneath the
/// global, and the next `spawn_device` in the same process — an output-mode
/// change, a retry, a front end restarting playback, or the next test —
/// dereferenced freed COM state and took the whole process down. See
/// `playback::device`'s "Why cpal is first touched from a thread that never
/// exits".
///
/// The shape here is therefore the essential one: **open from a fresh thread,
/// join it so it has genuinely exited (thread-local destructors and all), then
/// open again.** `join` cannot catch an access violation — the point is that a
/// process which still has this bug does not survive the loop, so the failure
/// is the test binary dying rather than an assertion.
///
/// It is deliberately meaningful **without** an audio device, because that is
/// the configuration it was found in: the enumerator is built, and the stale
/// pointer is dereferenced, on the "no default output device" path too. A
/// machine with hardware exercises the same loop with real streams opened and
/// closed on top.
#[cfg(feature = "device-output")]
#[test]
fn opening_the_output_from_threads_that_exit_never_faults() {
    use baz_core::playback::device::DeviceSink;

    /// Enough that the "first toucher exits, next caller faults" ordering has
    /// happened several times over, and still under a second either way.
    const ROUNDS: usize = 8;

    let mut opened = 0usize;
    for round in 0..ROUNDS {
        let outcome = std::thread::Builder::new()
            .name(format!("device-open-{round}"))
            .spawn(|| match DeviceSink::open(RATE, 1024) {
                // Drop inside the thread: closing the stream is as much part
                // of the sequence as opening it.
                Ok(sink) => {
                    drop(sink);
                    Ok(true)
                }
                Err(PlaybackError::Device(_)) => Ok(false),
                Err(other) => Err(other.to_string()),
            })
            .expect("spawn an opening thread")
            .join()
            .expect("the opening thread must not take the process with it");
        match outcome {
            Ok(true) => opened += 1,
            Ok(false) => {}
            Err(other) => panic!("unexpected error opening device: {other}"),
        }
    }
    assert!(
        opened == 0 || opened == ROUNDS,
        "the device opened on {opened} of {ROUNDS} rounds — opening from a fresh \
         thread must not depend on which thread went first"
    );
    if opened == 0 {
        eprintln!(
            "NOTE: no usable output device — the loop still covers the enumeration path, \
             which is where this bug lived"
        );
    }
    println!(
        "[device] {ROUNDS} open/close rounds from short-lived threads ({opened} with hardware)"
    );
}

/// **Reopening the stream over and over must not fault, and every reopen must
/// leave a stream that still takes audio.**
///
/// ADR-0009's rate negotiation tears one cpal stream down and builds another
/// while a callback is live on the old one. `device_sink_reopens_at_the_
/// requested_rate` measures a single reopen; this one flaps between two rates
/// as fast as the host will allow, which is where a teardown that did not
/// actually stop the old callback before releasing its state would show up.
/// Writing audio after each reopen is what makes it more than a smoke test:
/// the ring being fed has to be the *new* stream's.
///
/// Needs hardware, and skips with a notice without it — unlike the test above,
/// there is nothing to reopen when there is no device.
#[cfg(feature = "device-output")]
#[test]
fn rapid_reopens_never_fault_and_always_leave_a_live_stream() {
    use baz_core::playback::Sink as _;
    use baz_core::playback::device::DeviceSink;

    /// Each round is two reopens, so this is 16 stream teardowns.
    const ROUNDS: usize = 8;

    let mut sink = match DeviceSink::open(RATE, 2048) {
        Ok(sink) => sink,
        Err(PlaybackError::Device(msg)) => {
            eprintln!("SKIP: no usable output device ({msg})");
            return;
        }
        Err(other) => panic!("unexpected error opening device: {other}"),
    };

    let mut reopens = 0usize;
    for _ in 0..ROUNDS {
        for asked in [RATE_HI, RATE] {
            let granted = sink
                .negotiate_rate(asked)
                .expect("a device sink always has a rate");
            assert_eq!(
                sink.sample_rate(),
                granted,
                "the sink must report the rate it actually ended up at"
            );
            if granted == asked {
                reopens += 1;
            }
            // A short block at whatever rate we landed on: the stream has to
            // still be taking audio, and `write` would spin forever on a dead
            // one if `failed` were not being set.
            sink.write(&silence_stereo(granted as usize / 200));
            assert!(
                !sink.failed(),
                "the stream faulted after a reopen to {granted} Hz"
            );
        }
    }
    if reopens == 0 {
        eprintln!("SKIP: this device offers no {RATE_HI} Hz mode, so nothing reopened");
        return;
    }
    println!("[device] {reopens} reopens across {ROUNDS} rounds, stream alive throughout");
}

// ---------------------------------------------------------------------------
// Exclusive output (feature `exclusive-output`, Linux/ALSA) — ADR-0012
//
// These run against **real hardware**. Everything they assert is a claim about
// a card rather than about a data structure, which is exactly why none of it
// can be tested any other way; the engine's half of the arrangement is
// asserted with doubles in `engine.rs`'s unit tests. A machine with no
// hardware playback PCM (a CI container) skips with a notice, the same
// convention the `device-output` tests above use.
// ---------------------------------------------------------------------------

/// The hardware device these tests run against: the one `BAZ_OUTPUT_DEVICE`
/// names, or the first that will actually open.
///
/// `None` means every device is busy or unopenable, which on a desktop usually
/// means the sound server is holding them — a skip, not a failure.
///
/// It honours the *real* opt-in variable rather than a test-only one on
/// purpose: `BAZ_OUTPUT_DEVICE=hw:3,0 cargo test --features exclusive-output`
/// points the whole suite at a particular DAC, which is exactly how a
/// maintainer verifies the claims on the hardware they care about instead of
/// on whatever the enumeration happens to list first.
#[cfg(all(target_os = "linux", feature = "exclusive-output"))]
fn first_openable_device(
    rate: u32,
) -> Option<(
    baz_core::playback::exclusive::ExclusiveDevice,
    baz_core::playback::exclusive::ExclusiveSink,
)> {
    use baz_core::playback::exclusive::{ExclusiveSink, devices};

    let requested = std::env::var("BAZ_OUTPUT_DEVICE").ok();
    for device in devices() {
        if requested.as_deref().is_some_and(|r| r != device.pcm_name) {
            continue;
        }
        match ExclusiveSink::open(&device, rate, 8192) {
            Ok(sink) => return Some((device, sink)),
            Err(e) if requested.is_some() => {
                eprintln!("[exclusive] {device} was named but will not open: {e}");
            }
            Err(_) => {}
        }
    }
    None
}

/// **Enumeration finds real cards, and never the sound server.**
///
/// ADR-0011 measured what `"default"` actually is on a `PipeWire` desktop — the
/// server's bridge, whose mixer is `PipeWire`'s own system volume — and that is
/// the reason this backend enumerates cards instead. So the invariant is not
/// "the list is non-empty", it is "everything in the list is hardware": every
/// name is a `hw:CARD,DEV`, and none of the plugin names that would quietly
/// put a converter or a mixer back in the path appears at all.
#[cfg(all(target_os = "linux", feature = "exclusive-output"))]
#[test]
fn exclusive_enumeration_offers_hardware_and_never_the_sound_server() {
    use baz_core::playback::exclusive::devices;

    let found = devices();
    if found.is_empty() {
        eprintln!("SKIP: no hardware playback device on this machine");
        return;
    }
    for device in &found {
        assert!(
            device.pcm_name.starts_with("hw:"),
            "enumeration must offer hardware PCMs only: {device}"
        );
        for plugin in [
            "default",
            "plughw",
            "pulse",
            "pipewire",
            "dmix",
            "sysdefault",
        ] {
            assert!(
                !device.pcm_name.contains(plugin),
                "{plugin} is the sound server or a converting wrapper, not a card: {device}"
            );
        }
        assert!(
            device.pcm_name.ends_with(&format!(
                ",{}",
                device
                    .pcm_name
                    .rsplit(',')
                    .next()
                    .expect("hw: names carry a device number")
            )),
            "a hw: name must carry a device number: {device}"
        );
        println!("[exclusive] {device}");
    }
}

/// **A hardware device opens exclusively, negotiates the rate that was asked
/// for, and carries the samples in a format that changes none of them.**
///
/// This is ADR-0012's headline claim measured on the hardware in front of it.
/// Three assertions, because "bit-perfect" is three separate facts:
///
/// 1. the PCM opened is a `hw:` device baz holds itself ([`Sink::is_exclusive`]);
/// 2. the **rate is the one requested**, not the nearest thing the driver felt
///    like — which is what makes `SignalChain::Exclusive { conversion: None }`
///    truthful;
/// 3. the **format carries every 24-bit code exactly** (the ladder in the
///    module docs, whose arithmetic is asserted exhaustively in that module's
///    unit tests) — so no conversion is hiding in the last hop either.
///
/// Then it plays half a second, because a claim about a device that was never
/// fed is a claim about nothing — and the xrun count printed below is only
/// meaningful over audio the card actually clocked out. It is half a second of
/// silence ([`silence_stereo`]): the driver moves it frame for frame exactly as
/// it would a tone, so nothing measured here changes, and the developer's
/// speakers stay quiet.
#[cfg(all(target_os = "linux", feature = "exclusive-output"))]
#[test]
fn an_exclusive_device_plays_at_the_requested_rate_in_an_exact_format() {
    use baz_core::playback::Sink as _;

    let Some((device, mut sink)) = first_openable_device(RATE) else {
        eprintln!("SKIP: every hardware playback device is busy or unopenable");
        return;
    };
    assert!(
        sink.is_exclusive(),
        "a sink built on a hw: PCM holds it exclusively, and must say so"
    );
    assert_eq!(
        sink.sample_rate(),
        RATE,
        "{device}: the device must run at the rate asked for, or the chain is not direct"
    );
    assert!(
        sink.format().is_exact_for_24_bit(),
        "{device}: negotiated {:?}, which cannot carry a 24-bit master unchanged — the \
         format ladder must prefer a wider carrier when the device offers one",
        sink.format()
    );

    sink.write(&silence_stereo(RATE as usize / 2));
    assert!(!sink.failed(), "{device}: the stream faulted while playing");
    sink.drain_buffered();
    println!(
        "[exclusive] {device}: {} Hz, {:?}, buffer {} frames, period {} frames, \
         {} xrun(s) over 0.5 s of tone",
        sink.sample_rate(),
        sink.format(),
        sink.buffer_frames(),
        sink.period_frames(),
        sink.xruns(),
    );
}

/// **A busy device fails cleanly, with a typed error, immediately.**
///
/// The failure mode the design has to get right: on a desktop the sound server
/// holds the card most of the time, and "baz hangs" or "baz panics" would be
/// unacceptable answers. The busy condition is created deterministically here
/// — baz opens the device, then asks for it again — rather than by hoping
/// another application is using it.
///
/// What is asserted is the *type*, not the prose:
/// [`PlaybackError::DeviceBusy`] is its own variant precisely so a front end
/// can tell "someone else has it" from "there is no such device" without
/// matching on a string.
#[cfg(all(target_os = "linux", feature = "exclusive-output"))]
#[test]
fn a_busy_exclusive_device_fails_cleanly_with_a_typed_error() {
    use std::time::Instant;

    use baz_core::playback::exclusive::ExclusiveSink;

    let Some((device, _held)) = first_openable_device(RATE) else {
        eprintln!("SKIP: every hardware playback device is busy or unopenable");
        return;
    };
    let t0 = Instant::now();
    let error = ExclusiveSink::open(&device, RATE, 8192)
        .expect_err("a device baz is already holding cannot be opened again");
    let elapsed = t0.elapsed();

    match &error {
        PlaybackError::DeviceBusy { device: named } => {
            assert!(
                named.contains(&device.pcm_name),
                "the error must name the device that is busy: {named}"
            );
        }
        other => panic!("a held device must report DeviceBusy, got {other}"),
    }
    assert!(
        elapsed < Duration::from_millis(500),
        "a busy open must fail immediately rather than wait for the holder: took {elapsed:?}"
    );
    println!("[exclusive] second open of {device} refused in {elapsed:?}: {error}");
}

/// **A discard empties the device outright, synchronously.**
///
/// Shared mode needs a monotone watermark and a callback to honour it, because
/// only cpal's callback may advance the ring's read index (`device.rs`'s module
/// docs argue the whole design). Owning the PCM removes the constraint:
/// `snd_pcm_drop` is the discard, and it is complete when it returns. So the
/// assertion here is stronger than shared mode's — not "empty within 400 ms"
/// but "empty now".
#[cfg(all(target_os = "linux", feature = "exclusive-output"))]
#[test]
fn an_exclusive_discard_empties_the_device_immediately() {
    use baz_core::playback::Sink as _;

    let Some((device, mut sink)) = first_openable_device(RATE) else {
        eprintln!("SKIP: every hardware playback device is busy or unopenable");
        return;
    };
    // A full kernel buffer, which is as much as the device can be holding at
    // once: `write` returns when the last frame has been handed over, so
    // writing *more* than a buffer would only mean the excess had already
    // played. At the size the app uses that is ~186 ms of audio standing
    // between the write and the speaker.
    let frames = sink.buffer_frames();
    sink.write(&silence_stereo(frames));
    let queued = sink.queued_frames();
    assert!(
        queued > frames as u64 / 2,
        "{device}: the device must actually be holding audio for the discard to mean \
         anything (only {queued} of a {frames}-frame buffer queued)"
    );

    sink.discard_buffered();
    // Read once, immediately: no polling loop, no settle budget, no callback
    // to wait for. That is the whole difference from the shared-mode test
    // above, which needs all three.
    let left = sink.queued_frames();
    assert!(
        left * 100 < queued,
        "{device}: a discard on a device baz owns is complete when it returns, but \
         {left} of {queued} frames are still reported"
    );
    assert!(
        left < u64::from(RATE) / 100,
        "{device}: {left} frames reported after a discard is over 10 ms of the abandoned \
         position, which is what the discard exists to remove"
    );
    assert!(
        !sink.failed(),
        "{device}: the stream faulted on the discard"
    );
    #[allow(clippy::cast_precision_loss)] // frame counts are far below 2^52
    let ms = |f: u64| 1000.0 * f as f64 / f64::from(RATE);
    println!(
        "[exclusive] {device}: {queued} frames ({:.1} ms) discarded synchronously; \
         {left} frames ({:.2} ms) still reported by snd_pcm_delay on the reprepared stream",
        ms(queued),
        ms(left),
    );
}

/// **Reopening at a new rate really produces a stream at that rate**, and
/// getting back to the rate already open costs nothing.
///
/// The exclusive counterpart of `device_sink_reopens_at_the_requested_rate`,
/// and the reason it has to exist separately: this backend must *release* the
/// device before it can reopen it, where the shared one builds the new stream
/// first and swaps. A device that only offers one of the two rates is a
/// documented outcome, not a failure.
#[cfg(all(target_os = "linux", feature = "exclusive-output"))]
#[test]
fn an_exclusive_sink_reopens_at_the_requested_rate() {
    use std::time::Instant;

    use baz_core::playback::Sink as _;

    const HI_RATE: u32 = 48_000;

    let Some((device, mut sink)) = first_openable_device(RATE) else {
        eprintln!("SKIP: every hardware playback device is busy or unopenable");
        return;
    };
    let t0 = Instant::now();
    let granted = sink.negotiate_rate(HI_RATE);
    let reopen = t0.elapsed();
    if granted != Some(HI_RATE) {
        eprintln!("SKIP: {device} has no {HI_RATE} Hz mode (granted {granted:?})");
        return;
    }
    assert_eq!(sink.sample_rate(), HI_RATE);
    assert!(
        !sink.failed(),
        "{device}: reopening must leave a working stream"
    );

    let t1 = Instant::now();
    assert_eq!(sink.negotiate_rate(HI_RATE), Some(HI_RATE));
    let noop = t1.elapsed();
    assert!(
        noop < Duration::from_millis(10),
        "{device}: re-requesting the open rate must not reopen anything, took {noop:?}"
    );

    // And back down, which is what a mixed-rate queue does at every boundary.
    assert_eq!(sink.negotiate_rate(RATE), Some(RATE));
    assert_eq!(sink.sample_rate(), RATE);
    println!(
        "[exclusive] {device}: {RATE} Hz -> {HI_RATE} Hz reopened in {:.1} ms; \
         re-requesting the open rate took {:.3} ms",
        reopen.as_secs_f64() * 1e3,
        noop.as_secs_f64() * 1e3,
    );
}

/// **The hardware volume is real, and it is the hardware that moves.**
///
/// ADR-0011 built [`Sink::set_device_volume`] and found nothing correct to put
/// behind it in shared mode. This is the measurement that says whether owning
/// the card changed that on *this* hardware: ask for −6.02 dB (half
/// amplitude), then read the element back and check it landed there.
///
/// Two things are deliberately *not* asserted here. That the samples are
/// unscaled is the engine's half, asserted in `engine.rs`'s
/// `a_sink_with_an_attenuator_carries_the_volume_and_the_stream_is_untouched`
/// against the delivered stream itself; and the exact decibel is only asserted
/// to within one element step, because a mixer's travel is quantised far more
/// coarsely than the 1000-position control is (0.2 dB per step on the element
/// this machine picks, against the control's ~0.06 dB) and the element lands on
/// the nearest value it has.
///
/// A card with no attenuator (S/PDIF and HDMI outputs generally have none) is
/// a documented outcome — software gain, honestly reported — not a failure.
#[cfg(all(target_os = "linux", feature = "exclusive-output"))]
#[test]
fn the_exclusive_hardware_volume_moves_the_cards_own_attenuator() {
    use baz_core::playback::Sink as _;

    let Some((device, mut sink)) = first_openable_device(RATE) else {
        eprintln!("SKIP: every hardware playback device is busy or unopenable");
        return;
    };
    let Some(element) = sink.hardware_volume_element().map(ToOwned::to_owned) else {
        eprintln!("SKIP: {device} has no playback attenuator with a decibel scale");
        return;
    };
    let (min_db, max_db) = sink
        .hardware_volume_db_range()
        .expect("an element was found, so it has a range");
    let restore = sink.hardware_volume_db();

    // Half amplitude: −6.0206 dB, comfortably inside every attenuator's travel.
    let taken = sink.set_device_volume(0.5);
    assert_eq!(
        taken,
        Some(()),
        "{device}: {element} has a {min_db:.2}..{max_db:.2} dB travel, so it must take −6 dB"
    );
    let landed = sink
        .hardware_volume_db()
        .expect("the element was just written");
    assert!(
        (landed - (-6.0206)).abs() < 1.5,
        "{device}: asked {element} for −6.02 dB, it landed on {landed:.2} dB — further \
         than one step of any ordinary mixer"
    );
    assert!(
        landed < 0.0,
        "{device}: the element must actually have moved off unity: {landed:.2} dB"
    );

    // Unity declines on purpose: with nothing to attenuate, "no gain stage
    // anywhere" (VolumePath::Unity) is the more precise of the two true
    // statements — and the element is parked at 0 dB so it is also accurate.
    assert_eq!(
        sink.set_device_volume(1.0),
        None,
        "{device}: at unity there is nothing for the hardware to carry"
    );
    let at_unity = sink
        .hardware_volume_db()
        .expect("the element was just written");
    assert!(
        at_unity.abs() < 1.5,
        "{device}: declining at unity must still leave the hardware at 0 dB, not wherever \
         the last change put it: {at_unity:.2} dB"
    );

    // Mute reaches exactly zero, which no attenuator does.
    assert_eq!(
        sink.set_device_volume(0.0),
        None,
        "{device}: only software gain reaches exactly zero"
    );
    // The test leaves the listener's card where the unity call put it — 0 dB,
    // the resting state — rather than wherever the −6 dB step left it.
    assert!(
        sink.hardware_volume_db().is_some_and(|db| db.abs() < 1.5),
        "{device}: a test must not leave the machine's mixer attenuated"
    );

    println!(
        "[exclusive] {device}: hardware volume on {element:?}, travel {min_db:.2}..{max_db:.2} dB; \
         asked −6.02 dB, landed {landed:.2} dB (was {restore:?})"
    );
}

/// **End to end: an engine spawned in exclusive mode plays real audio and
/// reports an exclusive chain.**
///
/// The whole of ADR-0012 in one test — the opt-in, the backend, the engine and
/// the readout — measured against a card. What it asserts is the sentence a
/// front end will render: source rate, output rate, and a chain that says baz
/// holds the device and is converting nothing.
#[cfg(all(target_os = "linux", feature = "exclusive-output"))]
#[test]
fn an_exclusive_engine_plays_and_reports_an_exclusive_chain() {
    use std::time::Instant;

    use baz_core::engine::spawn_device_with;
    use baz_core::playback::OutputMode;
    use baz_core::protocol::{Command, Event, SignalChain};

    let Some((device, sink)) = first_openable_device(RATE) else {
        eprintln!("SKIP: every hardware playback device is busy or unopenable");
        return;
    };
    // Release it: the engine is about to hold it, and exclusive means one
    // holder. (This is also the reason the busy test above is deterministic.)
    drop(sink);

    let dir = tempfile::tempdir().expect("temp dir");
    let track = dir.path().join("half_second.wav");
    // Silent fixture: what is asserted is the event stream and the reported
    // chain, not the audio, and the engine plays whatever the file holds
    // straight out of the card ([`silence_stereo`]).
    write_wav_f32(&track, RATE, &silence_stereo(RATE as usize / 2));

    let spawned = spawn_device_with(
        EngineConfig::default(),
        &OutputMode::Exclusive {
            device: Some(device.pcm_name.clone()),
        },
        RATE,
        8192,
    );
    let (engine, events) = match spawned {
        Ok(pair) => pair,
        Err(e) => {
            eprintln!("SKIP: {device} could not be held for the engine ({e})");
            return;
        }
    };
    engine
        .send(Command::SetQueue {
            paths: vec![track.clone()],
        })
        .expect("send");
    engine.send(Command::Play).expect("send");

    let deadline = Instant::now() + Duration::from_secs(20);
    let mut chain = None;
    let mut started = false;
    while Instant::now() < deadline {
        match events.recv_timeout(Duration::from_millis(200)) {
            Ok(Event::SignalPath {
                source_rate_hz,
                output_rate_hz,
                chain: reported,
                ..
            }) => {
                assert_eq!(source_rate_hz, RATE);
                assert_eq!(
                    output_rate_hz, RATE,
                    "the device was opened at the source's own rate"
                );
                chain = Some(reported);
            }
            Ok(Event::TrackStarted { .. }) => started = true,
            Ok(Event::QueueEnded) => break,
            // Everything else is noise here, and so is a poll timeout: the
            // loop's own deadline is what bounds the wait.
            Ok(_) | Err(_) => {}
        }
    }
    engine.shutdown();

    assert!(started, "{device}: the track never started");
    let chain = chain.expect("an engine session must report its signal path");
    assert_eq!(
        chain,
        SignalChain::Exclusive { conversion: None },
        "{device}: baz held the card and converted nothing, and must say exactly that"
    );
    println!("[exclusive] {device}: engine played {RATE} Hz with chain {chain:?}");
}
