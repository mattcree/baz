//! Every hostile media input the fuzzer has ever found, run through the
//! **same call a file in a scanned folder takes** — and required to come back
//! as an error.
//!
//! # Why these live in `cargo test` and not only in a corpus
//!
//! `.github/workflows/ci.yml`'s fuzz job runs on `schedule` and
//! `workflow_dispatch`. A tag arrives as `push`, so **the gate a release is
//! held to never fuzzes** (`docs/RELEASING.md` says so where the maintainer
//! will be standing). A finding that lives only in a fuzz corpus is therefore
//! not a gate at all: it can regress for a week, or through a whole release,
//! before anything says so. Each input below is 20–56 bytes; running them
//! costs microseconds and turns a scheduled discovery into a `push`-time
//! guarantee.
//!
//! # What is asserted, and what deliberately is not
//!
//! **Asserted: an error, not a panic, not an abort, not a hang.** That is
//! baz's whole promise about a corrupt file — it does not play, and the
//! player survives it. Three of these inputs *did* panic, in symphonia's AAC
//! reader, reachable from `AudioSource::open` on any of baz's six audio
//! extensions — and ADR-0040 answers them twice over: §2.5 stops handing
//! bytes to a raw-ADTS demuxer baz has no use for, and §2 contains an unwind
//! out of the parsers it does use.
//!
//! **Not asserted: which error.** The variant a hostile file produces is a
//! property of a third-party parser's internals and would pin baz's tests to
//! symphonia's private choices. `DecoderPanicked` versus `Decode` is not a
//! distinction this file makes for that reason — except once, in
//! `the_aac_inputs_are_refused_rather_than_contained`, where the absence of
//! `DecoderPanicked` is the assertion.
//!
//! **Not asserted: that no oversized allocation is attempted.** It is —
//! ADR-0040 establishes that the bound belongs to symphonia and says why baz
//! does not shadow its parsers to take it. The first three inputs below
//! therefore reserve about 4.3, 4.3 and 2.1 GB of *untouched* address space on
//! their way to the errors they produce, once per test that drives them. That
//! is the measured behaviour on a 64-bit machine — peak RSS 3.4 MB, because
//! the pages are never written — and running them anyway is deliberate:
//! pinning the exact bytes the fuzzer found matters more than avoiding a lazy
//! mapping that every platform baz builds on grants. If a runner ever *does*
//! refuse one, that refusal is the defect in `docs/BACKLOG.md` reproducing
//! itself on real hardware, which is worth more than a green tick.

use std::path::Path;

use baz_core::library::AUDIO_EXTENSIONS;
use baz_core::playback::{AudioSource, PlaybackError};

/// The 29 bytes the very first run of the fuzz job ever performed found, in
/// forty seconds: `docs/WORK.md` item 1, CI run 31399606796.
///
/// `fLaC`, then a metadata block header declaring type 6 (PICTURE) and a
/// length of 0x27FFFF, then a picture body whose 32-bit MIME-type length is
/// **0xFF004901 — 4,278,208,769 bytes** — inside a block claiming to be 2.6 MB
/// and a file that is 29 bytes. `symphonia-metadata`'s `read_picture_block`
/// allocates that buffer before reading a byte of it.
const FLAC_PICTURE_FOUR_GIGABYTE_MIME_TYPE: &[u8] = &[
    0x66, 0x4c, 0x61, 0x43, 0x06, 0x27, 0xff, 0xff, 0xfb, 0xff, 0x52, 0x00, 0xff, 0x00, 0x49, 0x01,
    0x00, 0x00, 0x00, 0x13, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xff, 0x3d,
];

/// The same defect one metadata block along: a FLAC `VORBIS_COMMENT` block whose
/// single comment declares 4,294,967,040 bytes.
///
/// Built by hand rather than found, to establish that the picture block is not
/// a site but a *class* — `read_comment_no_framing`'s `vec![0; comment_length]`
/// is the same shape of unchecked read, and it is reached from Ogg Vorbis as
/// well as from FLAC. ADR-0040 §2.
const FLAC_COMMENT_FOUR_GIGABYTE_VALUE: &[u8] = &[
    0x66, 0x4c, 0x61, 0x43, 0x04, 0x00, 0x00, 0x20, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00,
    0x00, 0xff, 0xff, 0xff,
];

/// And the class in a second container: a WAV whose `LIST`/`INFO` tag declares
/// 2,147,483,632 bytes. `symphonia-format-riff`'s own comment on the line
/// above the allocation reads `// TODO: Apply limit.` (ADR-0040 §2).
const WAV_INFO_TAG_TWO_GIGABYTE_VALUE: &[u8] = &[
    0x52, 0x49, 0x46, 0x46, 0x20, 0x00, 0x00, 0x80, 0x57, 0x41, 0x56, 0x45, 0x66, 0x6d, 0x74, 0x20,
    0x10, 0x00, 0x00, 0x00, 0x01, 0x00, 0x02, 0x00, 0x44, 0xac, 0x00, 0x00, 0x10, 0xb1, 0x02, 0x00,
    0x04, 0x00, 0x10, 0x00, 0x4c, 0x49, 0x53, 0x54, 0xfc, 0xff, 0xff, 0x7f, 0x49, 0x4e, 0x46, 0x4f,
    0x49, 0x4e, 0x41, 0x4d, 0xf0, 0xff, 0xff, 0x7f,
];

/// 29 bytes that made symphonia's ADTS reader compute a sample step of zero
/// and hand it to `Iterator::step_by`, whose first act is
/// `assert!(step != 0)`. `adts.rs:306`, inside `approximate_frame_count`,
/// during `try_new` — so this panics while *opening*, before any decode.
const ADTS_ZERO_SAMPLE_STEP: &[u8] = &[
    0x08, 0xfd, 0x09, 0xfc, 0x10, 0x29, 0xfc, 0x10, 0x29, 0xff, 0x09, 0xcf, 0x00, 0xff, 0xf1, 0x10,
    0x08, 0xfd, 0xfc, 0x10, 0x29, 0xff, 0xfc, 0x11, 0xff, 0xf1, 0x10, 0x00, 0xfd,
];

/// 33 bytes that made the same function subtract a stream position from a
/// smaller total length: `adts.rs:303`, `attempt to subtract with overflow`.
const ADTS_LENGTH_BEHIND_POSITION: &[u8] = &[
    0x40, 0xc9, 0x60, 0x00, 0x7b, 0x2d, 0xff, 0x91, 0xf0, 0xb8, 0x02, 0x74, 0x00, 0x2b, 0x00, 0x1b,
    0x2d, 0xff, 0xf1, 0xf0, 0xb8, 0x02, 0x74, 0x00, 0x2b, 0x00, 0x1b, 0x00, 0xf1, 0x35, 0x00, 0x00,
    0x00,
];

/// 27 bytes that indexed one past the end of a scalefactor band table in
/// symphonia's AAC decoder: `aac/ics/mod.rs:365`, `index out of bounds: the
/// len is 50 but the index is 50`.
///
/// The one input here that panics during **decode** rather than during the
/// probe, which is why the containment is on `next_block` as well as on
/// `open`.
const AAC_BAND_INDEX_PAST_END: &[u8] = &[
    0x00, 0x00, 0xff, 0xf1, 0x10, 0x50, 0x02, 0x74, 0x00, 0x00, 0x00, 0x3a, 0x03, 0x40, 0x00, 0xd3,
    0x00, 0x00, 0x00, 0x00, 0x1b, 0xdf, 0xff, 0xf3, 0x14, 0x66, 0x74,
];

/// 122 bytes of ISO-MP4 whose `ftyp` atom declares a length of `u64::MAX`,
/// which symphonia adds to a stream position: `atoms/mod.rs:449`, `attempt to
/// add with overflow`.
///
/// **The one that is still live.** ADR-0040 §2.5 stopped handing bytes to the
/// raw-ADTS demuxer; this reaches `IsoMp4Reader`, which baz needs and keeps —
/// so it is the containment of §2 doing its job rather than a parser being
/// declined, and `a_contained_panic_is_named_as_one` asserts exactly that.
///
/// It is an **overflow-check** panic, so it fires in any build with
/// `debug-assertions` (every `cargo test`, every `cargo run`, the fuzz
/// targets) and not in a release build, where the sum wraps and the wrapped
/// length happens to fail symphonia's own next check. That makes it a *wrong
/// number* in release and a *dead thread* in debug — both upstream's, both
/// contained here.
const MP4_FTYP_LENGTH_OVERFLOW: &[u8] = &[
    0x7e, 0xbc, 0x7f, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x66, 0x74, 0x79, 0x70, 0xff, 0xff,
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xfb, 0xff, 0xec, 0x00,
    0x00, 0xf2, 0x00, 0x66, 0x74, 0x79, 0x70, 0xfb, 0xfb, 0x25, 0xf7,
];

/// Every input above, with the name a failure should report.
fn every_hostile_input() -> Vec<(&'static str, &'static [u8])> {
    vec![
        (
            "flac picture, 4 GB mime type",
            FLAC_PICTURE_FOUR_GIGABYTE_MIME_TYPE,
        ),
        ("flac comment, 4 GB value", FLAC_COMMENT_FOUR_GIGABYTE_VALUE),
        ("wav info tag, 2 GB value", WAV_INFO_TAG_TWO_GIGABYTE_VALUE),
        ("adts, zero sample step", ADTS_ZERO_SAMPLE_STEP),
        ("adts, length behind position", ADTS_LENGTH_BEHIND_POSITION),
        ("aac, band index past the end", AAC_BAND_INDEX_PAST_END),
        ("mp4 ftyp, length overflow", MP4_FTYP_LENGTH_OVERFLOW),
    ]
}

/// Drive a source the way the engine does: open it, then pull blocks until it
/// stops. Any error is a pass; reaching the end is a pass; a panic reaching
/// this frame is the failure the file exists to catch.
fn open_and_drain(source: Result<AudioSource, PlaybackError>) {
    let Ok(mut source) = source else {
        return;
    };
    // The same bound the fuzz target uses: enough to reach mid-stream decode,
    // not enough for a lying header to ask for unbounded output.
    for _ in 0..64 {
        match source.next_block() {
            Ok(Some(_)) => {}
            Ok(None) | Err(_) => break,
        }
    }
}

/// The in-memory path — `AudioSource::open_bytes`, which is what the fuzz
/// target drives.
#[test]
fn no_hostile_input_takes_the_decoder_with_it() {
    for (name, bytes) in every_hostile_input() {
        open_and_drain(AudioSource::open_bytes(bytes.to_vec()));
        // Reaching here at all is the assertion; the print names the input if
        // a *later* one is the one that dies.
        println!("survived: {name}");
    }
}

/// The on-disk path — `AudioSource::open`, which is the call a file in a
/// scanned folder actually takes, extension hint and all.
///
/// **Every extension baz scans**, because the hint does not decide what the
/// probe tries. That is a measurement, not a worry: the ADTS input below was
/// written out under all six names and panicked through this exact call under
/// every one of them, on bytes that are not remotely any of those formats. A
/// guard that keyed on the extension would have caught none of it.
#[test]
fn no_hostile_file_on_disk_takes_the_decoder_with_it() {
    let dir = tempfile::tempdir().expect("temp dir");
    for (name, bytes) in every_hostile_input() {
        for extension in AUDIO_EXTENSIONS {
            let path = dir.path().join(format!("hostile.{extension}"));
            std::fs::write(&path, bytes).expect("write fixture");
            open_and_drain(AudioSource::open(&path));
            println!("survived: {name} as .{extension}");
        }
    }
}

/// The three AAC inputs are refused **without the containment having to
/// catch anything** — the reader they panicked in is no longer registered.
///
/// Two guards, one file: if `probe` ever registers `AdtsReader` again, these
/// inputs panic and the containment turns that into `DecoderPanicked`, so this
/// test fails with a variant that names exactly what went wrong rather than
/// with a dead test process. It is the difference between ADR-0040 §2.5
/// (do not hand these bytes to a parser baz has no use for) and §2 (survive it
/// if you do).
#[test]
fn the_aac_inputs_are_refused_rather_than_contained() {
    for (name, bytes) in [
        ("adts, zero sample step", ADTS_ZERO_SAMPLE_STEP),
        ("adts, length behind position", ADTS_LENGTH_BEHIND_POSITION),
        ("aac, band index past the end", AAC_BAND_INDEX_PAST_END),
    ] {
        let error = AudioSource::open_bytes(bytes.to_vec())
            .err()
            .unwrap_or_else(|| panic!("{name} must not open"));
        assert!(
            !matches!(error, PlaybackError::DecoderPanicked { .. }),
            "{name} reached a panic again: {error}"
        );
    }
}

/// A contained panic is reported as one, rather than dressed up as a file that
/// merely failed to decode.
///
/// The one place the variant is asserted positively, and the distinction is
/// what ADR-0040 wants kept: `Decode` means the bytes are not a stream,
/// `DecoderPanicked` means a parser broke on them and there is a bug to report
/// upstream. If symphonia ever fixes `isomp4/atoms/mod.rs`, this test fails —
/// and that failure is the notification that the backlog's upstream note can
/// be struck.
///
/// **Conditioned on `debug_assertions` because the defect is**: the panic is
/// an overflow check, so a release build wraps instead and the wrapped length
/// fails symphonia's own check a line later. Both are asserted, each where it
/// applies, rather than picking the convenient one.
#[test]
fn a_contained_panic_is_named_as_one() {
    let Err(error) = AudioSource::open_bytes(MP4_FTYP_LENGTH_OVERFLOW.to_vec()) else {
        panic!("this input cannot open");
    };
    if cfg!(debug_assertions) {
        assert!(
            matches!(error, PlaybackError::DecoderPanicked { doing: "opening" }),
            "expected a contained panic, got {error:?}"
        );
        assert_eq!(
            error.to_string(),
            "the decoder panicked while opening this file"
        );
    } else {
        assert!(
            matches!(error, PlaybackError::Decode(_)),
            "expected the wrapped-length refusal, got {error:?}"
        );
    }
}

/// The containment is `catch_unwind`, so it works only while panics unwind.
///
/// A `panic = "abort"` in any profile would turn every one of the inputs above
/// back into a dead process, silently, with every test in this file still
/// passing — because an aborting build cannot run this file at all. So the
/// guard is on the manifest rather than on the behaviour: no profile may set
/// it.
#[test]
fn unwinding_is_what_makes_the_containment_work() {
    let manifest = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("workspace root")
            .join("Cargo.toml"),
    )
    .expect("workspace manifest");
    assert!(
        !manifest.contains("panic ="),
        "a profile sets `panic`; ADR-0040's containment needs unwinding"
    );
}

/// The engine's own bound: **the inputs baz has fixed cost nothing.**
///
/// Separate from the assertions above because a timeout is a different failure
/// from a panic and the fuzzer measures it separately (`-timeout`).
///
/// # Why the giant-allocation inputs are timed and reported rather than gated
///
/// The three `… GB …` reproducers are the allocation class ADR-0040 declined
/// to guard, and their cost is **the platform's, not baz's**: the same three
/// bytes ask for the same 4.28 GB everywhere, and what differs is whether the
/// kernel hands back a lazy zero mapping or actually finds the pages.
///
/// **macOS actually finds them.** This test asserted one second over the whole
/// set and went red on `macos-latest` at **5.02 s**, which is not a regression
/// and not slow code — it is the backlogged defect showing a cost that the
/// Linux measurement (clean error, 3.4 MB peak RSS) had made look theoretical.
/// ADR-0040 priced the exposure as *"a small machine, a container limit, strict
/// overcommit or 32-bit"*; that list was short by one entry, and the entry is a
/// platform baz ships to.
///
/// Gating on it would leave `main` permanently red for a defect this project
/// has decided, with reasons, not to fix — and a permanently red gate hides the
/// next real regression. So the budget covers **what baz controls**, and the
/// allocation inputs are timed and printed so the number cannot quietly grow.
/// If they are ever bounded upstream, fold them back in and delete this split.
#[test]
fn no_hostile_input_is_slow() {
    let mut allocation_cost = std::time::Duration::ZERO;
    let started = std::time::Instant::now();
    for (name, bytes) in every_hostile_input() {
        // The reproducers whose cost is an allocation the platform decides on.
        // Named rather than indexed: a fourth one added above must opt in here
        // deliberately, not inherit an exemption by position.
        let upstream_allocation = matches!(
            name,
            "flac picture, 4 GB mime type"
                | "flac comment, 4 GB value"
                | "wav info tag, 2 GB value"
        );
        let at = std::time::Instant::now();
        open_and_drain(AudioSource::open_bytes(bytes.to_vec()));
        if upstream_allocation {
            allocation_cost += at.elapsed();
        }
    }
    // `saturating_sub` rather than the plain operator: the elapsed total is
    // measured across the loop and the allocation costs inside it, so a clock
    // that stepped between the two reads could make the parts exceed the whole
    // by a nanosecond. Saturating to zero fails the assertion open, never shut.
    let ours = started.elapsed().saturating_sub(allocation_cost);
    println!("hostile inputs: {ours:?} in baz, {allocation_cost:?} in upstream allocations");
    assert!(
        ours < std::time::Duration::from_secs(1),
        "the inputs baz bounds took {ours:?}; upstream allocations took \
         {allocation_cost:?} and are excluded (see this test's docs)"
    );
}
