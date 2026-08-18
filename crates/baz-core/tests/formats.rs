//! **Every extension baz scans is one it can actually decode.**
//!
//! `AUDIO_EXTENSIONS` is a promise made at the *filename* level and
//! `AudioFormat::is_decodable` is the same promise at the *codec* level, and
//! the two can drift: an extension added to the list without a decoder behind
//! it puts rows on the shelf that fail when pressed, which is the one thing
//! the scanner's positive-evidence gates exist to prevent.
//!
//! So this walks real encoded files rather than asserting over the constant.
//! The fixtures are produced by `ffmpeg` where it exists; where it does not,
//! each case says so and skips rather than passing quietly — a test that
//! cannot find an encoder has not proved anything.

use std::path::{Path, PathBuf};
use std::process::Command;

use baz_core::library::{AUDIO_EXTENSIONS, AudioFormat};
use baz_core::playback::AudioSource;

/// One second of a 440 Hz tone, as 16-bit stereo PCM at 44.1 kHz.
fn source(dir: &Path) -> PathBuf {
    let path = dir.join("src.wav");
    let mut samples = Vec::new();
    for n in 0..44_100u32 {
        let t = f64::from(n) / 44_100.0;
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a sine scaled well inside i16"
        )]
        let v = (0.4 * 32_767.0 * (std::f64::consts::TAU * 440.0 * t).sin()) as i16;
        samples.push(v);
        samples.push(v);
    }
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: 44_100,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(&path, spec).expect("create the source");
    for sample in samples {
        writer.write_sample(sample).expect("write");
    }
    writer.finalize().expect("finalize");
    path
}

fn have_ffmpeg() -> bool {
    Command::new("ffmpeg")
        .arg("-version")
        .output()
        .is_ok_and(|out| out.status.success())
}

/// **Each format baz claims decodes to real audio.**
///
/// Not merely "opens": the tone is a second long, so a decoder that produced
/// a header and no samples would pass an `is_ok` and fail this.
#[test]
fn every_claimed_extension_decodes_to_audio() {
    if !have_ffmpeg() {
        eprintln!("SKIP: no ffmpeg, so no encoded fixtures to decode");
        return;
    }
    let dir = tempfile::tempdir().expect("tempdir");
    let src = source(dir.path());

    // (extension, ffmpeg args after the input) — one case per extension this
    // test can produce. `flac`, `mp3`, `m4a`/`mp4` and `wav` are covered by
    // the playback suite's own fixtures; these are the ones added on
    // 2026-08-18 and the two Ogg spellings.
    //
    // **`.aac` is deliberately absent.** Raw ADTS needs `AdtsReader`, which
    // `playback::source`'s probe removed on fuzzing evidence (ADR-0040 §2.5).
    // It was briefly added here while broadening the list and this test is
    // what caught it — `aac_is_absent…` below now pins the refusal.
    let cases: [(&str, &[&str]); 3] = [
        ("aiff", &["-c:a", "pcm_s16be"]),
        ("oga", &["-c:a", "libvorbis", "-f", "ogg"]),
        ("ogg", &["-c:a", "libvorbis", "-f", "ogg"]),
    ];
    for (extension, args) in cases {
        assert!(
            AUDIO_EXTENSIONS.contains(&extension),
            "`{extension}` is decodable here but the scanner never looks at it"
        );
        let encoded = dir.path().join(format!("tone.{extension}"));
        let out = Command::new("ffmpeg")
            .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
            .arg(&src)
            .args(args)
            .arg(&encoded)
            .output()
            .expect("spawn ffmpeg");
        if !out.status.success() {
            eprintln!(
                "SKIP {extension}: this ffmpeg cannot encode it ({})",
                String::from_utf8_lossy(&out.stderr).trim()
            );
            continue;
        }
        let decoded = AudioSource::decode_all(&encoded)
            .unwrap_or_else(|error| panic!("{extension} did not decode: {error}"));
        assert!(
            decoded.samples.len() > 40_000,
            "{extension} decoded {} samples, which is not a second of audio",
            decoded.samples.len()
        );
        assert_eq!(decoded.sample_rate, 44_100, "{extension} changed rate");
    }
}

/// **AIFF is lossless and says so**, which is what the row's condition line
/// and the edition picker read.
#[test]
fn aiff_is_a_lossless_pcm_format() {
    assert!(AudioFormat::Aiff.is_lossless());
    assert!(AudioFormat::Aiff.is_decodable());
    // The persisted code round-trips, which is on-disk data.
    assert_eq!(AudioFormat::from_code("aiff"), Some(AudioFormat::Aiff));
    assert_eq!(AudioFormat::Aiff.code(), "aiff");
}

/// **Opus is still the one extension baz does not claim**, and this is the
/// test that will fail the day someone adds it to the list without a decoder.
#[test]
fn opus_is_absent_from_the_scanner_because_nothing_can_decode_it() {
    assert!(!AudioFormat::Opus.is_decodable());
    assert!(!AUDIO_EXTENSIONS.contains(&"opus"));
}

/// **Raw `.aac` is absent for a different reason, and a stronger one.**
///
/// The AAC *decoder* is registered and every AAC inside an MP4 plays through
/// it. What is missing is the raw-ADTS *demuxer*, which `playback::source`'s
/// probe removes because a fuzz sweep produced 650 crash artifacts and every
/// one of them was that reader firing on bytes under some other file's name
/// (ADR-0040 §2.5). Listing `.aac` would mean putting it back.
#[test]
fn aac_is_absent_because_its_demuxer_is_the_one_the_fuzzer_condemned() {
    assert!(
        !AUDIO_EXTENSIONS.contains(&"aac"),
        "listing raw .aac means registering AdtsReader again — read ADR-0040 \
         §2.5 before doing it"
    );
    // The codec itself is fine and stays claimed: this is about the container.
    assert!(AudioFormat::Aac.is_decodable());
}
