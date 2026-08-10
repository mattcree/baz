//! Integration tests for `baz_core::analysis`: the background ReplayGain pass,
//! end to end over real files and a real database.
//!
//! `tests/loudness.rs` proves the *meter* against EBU Tech 3341. This file
//! proves everything built on top of it — what the pass chooses to measure, what
//! it stores, what it skips, what a cancel costs, and what a second run does —
//! with fixtures whose loudness is known by construction rather than by having
//! been measured once and recorded.
//!
//! Every expected figure below is worked out from the signal, in the test's own
//! comments, before the code is asked. That is `docs/ENGINEERING.md`'s rule for
//! audio correctness: tests are written to the specification, never to the
//! implementation's output.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use baz_core::analysis::{self, AnalysisHandle};
use baz_core::index::Library;
use baz_core::library::{AudioFormat, FileStamp, TrackMeta};
use baz_core::protocol::{AnalysisCommand, Event};
use baz_core::replaygain::ReplayGainTags;

/// Fixture sample rate. 48 kHz and a 1 kHz tone means 48 samples per cycle, so
/// the sine lands exactly on its own peak — which makes the expected sample
/// peak an exact number rather than an approximation of one.
const RATE: u32 = 48_000;

/// How long to wait for an event before declaring the worker wedged. Generous:
/// a slow CI box decoding a few seconds of WAV is still far inside this, and a
/// test that hangs forever tells nobody anything.
const PATIENCE: Duration = Duration::from_secs(30);

/// A 1 kHz stereo tone of `seconds` at peak amplitude `dbfs`, as an
/// interleaved buffer.
fn tone(seconds: f64, dbfs: f64) -> Vec<f32> {
    let amplitude = 10f64.powf(dbfs / 20.0);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // seconds x rate
    let frames = (seconds * f64::from(RATE)).round() as usize;
    let mut samples = Vec::with_capacity(frames * 2);
    for frame in 0..frames {
        #[allow(clippy::cast_precision_loss)] // frame counts are small
        let t = frame as f64 / f64::from(RATE);
        #[allow(clippy::cast_possible_truncation)] // f64 sine -> f32 sample
        let value = (amplitude * (std::f64::consts::TAU * 1_000.0 * t).sin()) as f32;
        samples.push(value);
        samples.push(value);
    }
    samples
}

/// Write an interleaved stereo buffer as a 32-bit float WAV — the same
/// fixture format `tests/playback.rs` uses, so nothing is quantised between
/// the signal this test designed and the samples the meter sees.
fn write_wav(path: &Path, interleaved: &[f32]) {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: RATE,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(path, spec).expect("create wav");
    for &sample in interleaved {
        writer.write_sample(sample).expect("write sample");
    }
    writer.finalize().expect("finalize wav");
}

/// Write a mono 32-bit float WAV of the same tone.
fn write_mono_wav(path: &Path, interleaved_stereo: &[f32]) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: RATE,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(path, spec).expect("create wav");
    for sample in interleaved_stereo.iter().step_by(2) {
        writer.write_sample(*sample).expect("write sample");
    }
    writer.finalize().expect("finalize wav");
}

/// A library row for a file on disk, with the stamp a scan would have taken.
fn row(path: &Path, album: &str, title: &str, number: u32, format: AudioFormat) -> TrackMeta {
    TrackMeta {
        path: path.to_path_buf(),
        artist: Some("Karl".to_owned()),
        album_artist: Some("Karl".to_owned()),
        compilation: None,
        genre: None,
        album: Some(album.to_owned()),
        title: Some(title.to_owned()),
        track: Some(number),
        disc: None,
        year: Some(2026),
        duration: None,
        format: Some(format),
        bit_depth: Some(32),
        sample_rate: Some(RATE),
        bitrate: None,
        stamp: FileStamp::of_path(path),
        replay_gain: ReplayGainTags::default(),
    }
}

/// Drain events until one satisfies `wanted`, returning everything seen up to
/// and including it. Fails the test rather than blocking forever.
fn drain_until(events: &Receiver<Event>, wanted: impl Fn(&Event) -> bool) -> Vec<Event> {
    let deadline = Instant::now() + PATIENCE;
    let mut seen = Vec::new();
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        match events.recv_timeout(left) {
            Ok(event) => {
                let done = wanted(&event);
                seen.push(event);
                if done {
                    return seen;
                }
            }
            Err(RecvTimeoutError::Timeout) => panic!("no matching event within {PATIENCE:?}"),
            Err(RecvTimeoutError::Disconnected) => panic!("the analysis worker went away"),
        }
    }
}

/// Run one pass to completion and return every event it emitted.
fn run_pass(handle: &AnalysisHandle, events: &Receiver<Event>, redo: bool) -> Vec<Event> {
    handle
        .send(AnalysisCommand::StartReplayGainAnalysis { redo })
        .expect("the service accepts a start");
    drain_until(events, |event| {
        matches!(event, Event::ReplayGainAnalysisFinished { .. })
    })
}

/// The gain stored for a track, in centidecibels.
fn stored_track_gain(library: &Library, path: &Path) -> Option<i16> {
    library.computed_replay_gain(path).track_gain_centidb
}

/// The album gain stored for a track, in centidecibels.
fn stored_album_gain(library: &Library, path: &Path) -> Option<i16> {
    library.computed_replay_gain(path).album_gain_centidb
}

/// A three-track album whose loudness is known by construction, plus the
/// database that holds it.
///
/// Track levels are −23, −33 and −13 dBFS peak. A steady 1 kHz tone at peak
/// amplitude X dBFS measures X LUFS (EBU Tech 3341 case 1), so:
///
/// | track | loudness | gain to −18 LUFS |
/// |---|---|---|
/// | quiet-ish | −23 LUFS | **+5.00 dB** |
/// | quiet | −33 LUFS | **+15.00 dB** |
/// | loud | −13 LUFS | **−5.00 dB** |
///
/// The **album** figure is the gated loudness of all three tracks' blocks
/// pooled, and it is worked out here rather than read off the code. Each track
/// contributes the same number of equal blocks, so with `p = 10^(L/10)`:
///
/// - ungated mean `= (10^−2.3 + 10^−3.3 + 10^−1.3)/3 = 0.018544` → −17.32 LUFS
/// - relative gate `= −27.32 LUFS`, which removes the −33 LUFS track entirely
/// - gated mean `= (10^−2.3 + 10^−1.3)/2 = 0.027565` → **−15.60 LUFS**
///
/// so the album gain is `−18 − (−15.60)` = **−2.40 dB**, i.e. −240
/// centidecibels. Note that it is *not* the mean of the three track gains
/// (+5.00), which is what makes this fixture able to tell a pooled gate from an
/// average.
struct Fixture {
    _dir: tempfile::TempDir,
    db: PathBuf,
    tracks: Vec<PathBuf>,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = dir.path().join("library.db");
        let levels = [(-23.0, "Quiet-ish"), (-33.0, "Quiet"), (-13.0, "Loud")];
        let mut tracks = Vec::new();
        let mut rows = Vec::new();
        for (n, (dbfs, title)) in levels.into_iter().enumerate() {
            let path = dir.path().join(format!("{:02} {title}.wav", n + 1));
            write_wav(&path, &tone(5.0, dbfs));
            rows.push(row(
                &path,
                "Signal Chain",
                title,
                u32::try_from(n + 1).expect("three tracks"),
                AudioFormat::Wav,
            ));
            tracks.push(path);
        }
        let mut library = Library::open(&db).expect("open");
        library.add_tracks(rows).expect("seed the library");
        drop(library);
        Self {
            _dir: dir,
            db,
            tracks,
        }
    }

    fn library(&self) -> Library {
        Library::open(&self.db).expect("open")
    }
}

/// Expected per-track gains, in centidecibels, for [`Fixture`]'s three tracks.
const EXPECTED_TRACK_GAINS: [i16; 3] = [500, 1_500, -500];
/// Expected album gain, in centidecibels — see [`Fixture`] for the arithmetic.
const EXPECTED_ALBUM_GAIN: i16 = -240;
/// How far a computed gain may sit from the figure the fixture's arithmetic
/// predicts, in centidecibels: 0.1 dB, which is EBU Tech 3341's own tolerance
/// for the measurement underneath it.
const TOLERANCE_CENTIDB: i16 = 10;

fn assert_close(what: &str, got: Option<i16>, want: i16) {
    let got = got.unwrap_or_else(|| panic!("{what}: nothing was stored"));
    assert!(
        (got - want).abs() <= TOLERANCE_CENTIDB,
        "{what}: stored {got} centidB, the signal asks for {want} ±{TOLERANCE_CENTIDB}"
    );
}

/// A pass measures every track that needs it and stores both figures, and the
/// numbers are the ones the signals ask for.
#[test]
fn a_pass_measures_the_library_and_stores_what_the_signal_asks_for() {
    let fixture = Fixture::new();
    let (handle, events) = analysis::spawn(&fixture.db).expect("spawn");
    let seen = run_pass(&handle, &events, false);

    assert_eq!(
        seen.first(),
        Some(&Event::ReplayGainAnalysisStarted {
            tracks: 3,
            editions: 1,
        }),
        "one album in one codec is one edition, and all three tracks need measuring"
    );
    assert_eq!(
        seen.last(),
        Some(&Event::ReplayGainAnalysisFinished {
            analysed: 3,
            failed: 0,
            cancelled: false,
        })
    );
    handle.shutdown();

    let library = fixture.library();
    for (path, want) in fixture.tracks.iter().zip(EXPECTED_TRACK_GAINS) {
        assert_close(
            &format!("track gain for {}", path.display()),
            stored_track_gain(&library, path),
            want,
        );
        assert_close(
            &format!("album gain on {}", path.display()),
            stored_album_gain(&library, path),
            EXPECTED_ALBUM_GAIN,
        );
    }

    // The album gain is one number shared by every track of the edition —
    // that is what album mode exists to provide — and it is *not* the average
    // of the track gains, which this fixture is built to be able to tell apart.
    let album_gains: Vec<Option<i16>> = fixture
        .tracks
        .iter()
        .map(|path| stored_album_gain(&library, path))
        .collect();
    assert!(
        album_gains.windows(2).all(|pair| pair[0] == pair[1]),
        "every track of an edition shares one album figure: {album_gains:?}"
    );
    let mean_of_track_gains: i16 = EXPECTED_TRACK_GAINS.iter().sum::<i16>() / 3;
    assert!(
        (EXPECTED_ALBUM_GAIN - mean_of_track_gains).abs() > TOLERANCE_CENTIDB * 10,
        "the fixture must be able to tell a pooled gate from an average"
    );
}

/// The peak stored is the loudest sample in the file — which for a 1 kHz sine
/// at 48 kHz is exactly its own amplitude, because 48 samples per cycle lands
/// one of them on the crest.
#[test]
fn the_stored_peak_is_the_loudest_sample() {
    let fixture = Fixture::new();
    let (handle, events) = analysis::spawn(&fixture.db).expect("spawn");
    run_pass(&handle, &events, false);
    handle.shutdown();

    let library = fixture.library();
    // −23 dBFS peak amplitude is 0.0707946 of full scale, i.e. 70 795
    // micro-units; −13 dBFS is 0.2238721, i.e. 223 872.
    let figures = library.computed_replay_gain(&fixture.tracks[0]);
    assert_eq!(figures.track_peak_micro, Some(70_795));
    let loud = library.computed_replay_gain(&fixture.tracks[2]);
    assert_eq!(loud.track_peak_micro, Some(223_872));
    // The album peak is the loudest sample anywhere in the album, which is the
    // loud track's — and every track of the edition carries it, because that is
    // what album-mode clipping prevention checks against (ADR-0013 §3).
    for path in &fixture.tracks {
        assert_eq!(
            library.computed_replay_gain(path).album_peak_micro,
            Some(223_872),
            "{}",
            path.display()
        );
    }
}

/// A track whose file already carries ReplayGain is not measured: tags win in
/// the selection rule, so measuring it would spend a decode to produce a number
/// nothing would use.
///
/// The edition here needs no album figure either — every track declares one —
/// so the pass finds nothing to do at all, which is the state a fully scanned
/// library is in.
#[test]
fn a_fully_tagged_edition_is_not_measured_at_all() {
    let fixture = Fixture::new();
    {
        let mut library = fixture.library();
        let tagged: Vec<TrackMeta> = library
            .tracks()
            .cloned()
            .map(|meta| TrackMeta {
                replay_gain: ReplayGainTags {
                    track_gain_centidb: Some(-775),
                    track_peak_micro: Some(988_525),
                    album_gain_centidb: Some(-920),
                    album_peak_micro: Some(1_001_221),
                },
                ..meta
            })
            .collect();
        library.add_tracks(tagged).expect("tag every track");
    }

    let (handle, events) = analysis::spawn(&fixture.db).expect("spawn");
    let seen = run_pass(&handle, &events, false);
    assert_eq!(
        seen,
        vec![
            Event::ReplayGainAnalysisStarted {
                tracks: 0,
                editions: 0,
            },
            Event::ReplayGainAnalysisFinished {
                analysed: 0,
                failed: 0,
                cancelled: false,
            },
        ],
        "a tagged library still answers the question it was asked — with zero"
    );
    // `redo` does not touch tags either: baz does not write to music files, and
    // it will not measure what a tag already answers.
    let again = run_pass(&handle, &events, true);
    assert_eq!(
        again.first(),
        Some(&Event::ReplayGainAnalysisStarted {
            tracks: 0,
            editions: 0,
        })
    );
    handle.shutdown();

    let library = fixture.library();
    for path in &fixture.tracks {
        assert!(library.computed_replay_gain(path).is_empty());
    }
}

/// An edition that is only **partly** tagged is measured **whole**, because an
/// album gain computed from the subset that happened to be untagged would be a
/// different number — and a wrong one.
///
/// The already-tagged track keeps its tag: what the pass writes goes into the
/// computed columns, and the selection rule prefers the tag for that track and
/// the measurement for the others.
#[test]
fn a_partly_tagged_edition_is_measured_whole() {
    let fixture = Fixture::new();
    {
        let mut library = fixture.library();
        let mut rows: Vec<TrackMeta> = library.tracks().cloned().collect();
        rows[0].replay_gain = ReplayGainTags {
            track_gain_centidb: Some(-775),
            track_peak_micro: Some(988_525),
            ..ReplayGainTags::default()
        };
        library.add_tracks(rows).expect("tag one track");
    }

    let (handle, events) = analysis::spawn(&fixture.db).expect("spawn");
    let seen = run_pass(&handle, &events, false);
    assert_eq!(
        seen.first(),
        Some(&Event::ReplayGainAnalysisStarted {
            tracks: 3,
            editions: 1,
        }),
        "no album tag anywhere in the edition, so all three tracks are measured"
    );
    handle.shutdown();

    let library = fixture.library();
    // Including the tagged one: its measurement is what the album figure needed.
    assert_close(
        "the tagged track was measured too",
        stored_track_gain(&library, &fixture.tracks[0]),
        EXPECTED_TRACK_GAINS[0],
    );
    assert_close(
        "and the album figure is the whole album's",
        stored_album_gain(&library, &fixture.tracks[0]),
        EXPECTED_ALBUM_GAIN,
    );
    // The tag itself is untouched — two claims, two column groups.
    let tagged = library
        .tracks()
        .find(|meta| meta.path == fixture.tracks[0])
        .expect("the tagged row");
    assert_eq!(tagged.replay_gain.track_gain_centidb, Some(-775));
}

/// Running the pass again over a library it has already measured finds nothing
/// to do. That is what makes "analyse my library" cheap to repeat, and it is
/// the same property that makes a cancelled pass resumable.
#[test]
fn a_second_pass_over_an_unchanged_library_measures_nothing() {
    let fixture = Fixture::new();
    let (handle, events) = analysis::spawn(&fixture.db).expect("spawn");
    let first = run_pass(&handle, &events, false);
    assert_eq!(
        first.first(),
        Some(&Event::ReplayGainAnalysisStarted {
            tracks: 3,
            editions: 1,
        })
    );

    let second = run_pass(&handle, &events, false);
    assert_eq!(
        second,
        vec![
            Event::ReplayGainAnalysisStarted {
                tracks: 0,
                editions: 0,
            },
            Event::ReplayGainAnalysisFinished {
                analysed: 0,
                failed: 0,
                cancelled: false,
            },
        ],
        "everything already has a figure that still applies"
    );

    // `redo` is what a listener asks for when they want the numbers taken
    // again, and it does exactly that.
    let forced = run_pass(&handle, &events, true);
    assert_eq!(
        forced.first(),
        Some(&Event::ReplayGainAnalysisStarted {
            tracks: 3,
            editions: 1,
        }),
        "redo discards baz's own measurements and takes them again"
    );
    handle.shutdown();

    let library = fixture.library();
    assert_close(
        "and the second measurement is the same as the first",
        stored_track_gain(&library, &fixture.tracks[0]),
        EXPECTED_TRACK_GAINS[0],
    );
}

/// A file that changed since it was measured is measured again — the stamp is
/// what makes a loudness figure stop applying, and a rewritten file is a
/// different set of samples whatever its name is.
#[test]
fn a_file_that_changed_is_measured_again() {
    let fixture = Fixture::new();
    let (handle, events) = analysis::spawn(&fixture.db).expect("spawn");
    run_pass(&handle, &events, false);

    // Re-encode the first track at a different level and re-scan it, exactly
    // as a launch would.
    write_wav(&fixture.tracks[0], &tone(5.0, -13.0));
    {
        let mut library = fixture.library();
        let rescanned = TrackMeta {
            stamp: FileStamp::of_path(&fixture.tracks[0]),
            ..library
                .tracks()
                .find(|meta| meta.path == fixture.tracks[0])
                .cloned()
                .expect("the row")
        };
        library.add_tracks(vec![rescanned]).expect("rescan");
    }

    let seen = run_pass(&handle, &events, false);
    assert_eq!(
        seen.first(),
        Some(&Event::ReplayGainAnalysisStarted {
            tracks: 3,
            editions: 1,
        }),
        "the album figure went stale with the track, so the edition is measured whole"
    );
    handle.shutdown();

    let library = fixture.library();
    assert_close(
        "the changed file's new level",
        stored_track_gain(&library, &fixture.tracks[0]),
        -500,
    );
}

/// Progress events arrive one per track, in order, with cumulative counts that
/// never go backwards and never exceed the total the start announced.
#[test]
fn progress_events_arrive_in_order_and_count_up() {
    let fixture = Fixture::new();
    let (handle, events) = analysis::spawn(&fixture.db).expect("spawn");
    let seen = run_pass(&handle, &events, false);
    handle.shutdown();

    let mut iter = seen.iter();
    let Some(Event::ReplayGainAnalysisStarted { tracks, editions }) = iter.next() else {
        panic!("a pass announces itself before it does anything: {seen:?}");
    };
    assert_eq!((*tracks, *editions), (3, 1));

    let mut expected = 0;
    for event in iter.by_ref().take(*tracks) {
        let Event::ReplayGainAnalysisProgress {
            path,
            analysed,
            tracks: total,
            failed,
        } = event
        else {
            panic!("expected a progress event, got {event:?}");
        };
        expected += 1;
        assert_eq!(*analysed, expected, "counts are cumulative and in order");
        assert_eq!(*total, 3, "the total does not move mid-pass");
        assert_eq!(*failed, 0);
        assert!(
            fixture.tracks.contains(path),
            "progress names the file it finished with: {}",
            path.display()
        );
    }
    assert_eq!(
        iter.next(),
        Some(&Event::ReplayGainAnalysisFinished {
            analysed: 3,
            failed: 0,
            cancelled: false,
        }),
        "and the pass says so exactly once, at the end"
    );
    assert_eq!(iter.next(), None);
}

/// The shared readout is published **before** the event that announces it, so
/// a front end that sees `Started` and then reads `progress()` is never told
/// the pass has not begun.
///
/// This is the ordering contract the engine states for `VolumeChanged` and
/// `ReplayGainChanged` (`Control::settle_replay_gain`), applied to the second
/// service that has shared state to publish.
#[test]
fn shared_state_is_published_before_the_event_that_announces_it() {
    let fixture = Fixture::new();
    let (handle, events) = analysis::spawn(&fixture.db).expect("spawn");
    handle
        .send(AnalysisCommand::StartReplayGainAnalysis { redo: false })
        .expect("start");

    let seen = drain_until(&events, |event| {
        matches!(event, Event::ReplayGainAnalysisStarted { .. })
    });
    assert_eq!(seen.len(), 1);
    let after_started = handle.progress();
    assert_eq!(
        (after_started.tracks, after_started.editions),
        (3, 1),
        "the totals were published before the event carrying them"
    );

    let seen = drain_until(&events, |event| {
        matches!(event, Event::ReplayGainAnalysisFinished { .. })
    });
    let finished = handle.progress();
    assert!(
        !finished.running,
        "and `running` is false before `Finished`"
    );
    assert!(!finished.cancelled);
    assert_eq!(finished.analysed, 3);
    assert_eq!(
        seen.last(),
        Some(&Event::ReplayGainAnalysisFinished {
            analysed: 3,
            failed: 0,
            cancelled: false,
        })
    );
    handle.shutdown();
}

/// A cancel stops the pass, says it was cancelled, and **keeps what it
/// measured** — so a later start resumes rather than starting over.
///
/// The fixture is deliberately long enough that the cancel lands mid-pass on
/// any machine: fifty tracks of five seconds each is four minutes of audio to
/// decode, and the cancel is sent as soon as the first progress event proves
/// the pass is running.
#[test]
fn a_cancelled_pass_stops_promptly_keeps_its_work_and_resumes() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("library.db");
    // Fifty one-track albums rather than one fifty-track album: each is its own
    // edition, so each completes and commits on its own, which is exactly the
    // granularity a resume is defined at.
    let mut rows = Vec::new();
    let mut paths = Vec::new();
    for n in 0..50 {
        let path = dir.path().join(format!("album{n:02}.wav"));
        write_wav(&path, &tone(5.0, -23.0));
        rows.push(row(
            &path,
            &format!("Album {n:02}"),
            "Only Track",
            1,
            AudioFormat::Wav,
        ));
        paths.push(path);
    }
    {
        let mut library = Library::open(&db).expect("open");
        library.add_tracks(rows).expect("seed");
    }

    let (handle, events) = analysis::spawn(&db).expect("spawn");
    handle
        .send(AnalysisCommand::StartReplayGainAnalysis { redo: false })
        .expect("start");
    drain_until(&events, |event| {
        matches!(event, Event::ReplayGainAnalysisProgress { .. })
    });
    handle
        .send(AnalysisCommand::CancelReplayGainAnalysis)
        .expect("cancel");

    let tail = drain_until(&events, |event| {
        matches!(event, Event::ReplayGainAnalysisFinished { .. })
    });
    let Some(Event::ReplayGainAnalysisFinished {
        analysed,
        failed,
        cancelled,
    }) = tail.last()
    else {
        panic!("a cancelled pass still finishes: {tail:?}");
    };
    assert!(*cancelled, "and it says which of the two things happened");
    assert_eq!(*failed, 0);
    let stopped_after = *analysed;
    assert!(
        stopped_after < 50,
        "a cancel that only landed after the last track proves nothing ({stopped_after})"
    );

    // What was measured before the cancel is still there.
    let measured_before = {
        let library = Library::open(&db).expect("open");
        paths
            .iter()
            .filter(|path| !library.computed_replay_gain(path).is_empty())
            .count()
    };
    assert!(
        measured_before > 0,
        "a cancel must cost the work it interrupted, not the work it did"
    );
    assert!(measured_before <= stopped_after);

    // Starting again resumes: it plans strictly less work than the first pass.
    handle
        .send(AnalysisCommand::StartReplayGainAnalysis { redo: false })
        .expect("resume");
    let resumed = drain_until(&events, |event| {
        matches!(event, Event::ReplayGainAnalysisStarted { .. })
    });
    let Some(Event::ReplayGainAnalysisStarted { tracks, .. }) = resumed.last() else {
        panic!("a resumed pass announces itself: {resumed:?}");
    };
    assert_eq!(
        *tracks,
        50 - measured_before,
        "a resume measures exactly what the cancel left"
    );

    drain_until(&events, |event| {
        matches!(event, Event::ReplayGainAnalysisFinished { .. })
    });
    handle.shutdown();

    let library = Library::open(&db).expect("open");
    for path in &paths {
        assert_close(
            &format!("after the resume, {}", path.display()),
            stored_track_gain(&library, path),
            500,
        );
    }
}

/// A file that cannot be decoded is counted, not fatal — the scanner's
/// philosophy, applied to the pass. The album figure is still computed from the
/// tracks that *could* be measured, because a file baz cannot decode is a file
/// baz cannot play either, so it is not part of the album as anybody will hear
/// it.
#[test]
fn a_corrupt_file_is_counted_and_the_rest_of_the_album_is_still_measured() {
    let fixture = Fixture::new();
    std::fs::write(&fixture.tracks[1], b"this is not a wav file at all").expect("corrupt it");
    {
        let mut library = fixture.library();
        let rescanned = TrackMeta {
            stamp: FileStamp::of_path(&fixture.tracks[1]),
            ..library
                .tracks()
                .find(|meta| meta.path == fixture.tracks[1])
                .cloned()
                .expect("the row")
        };
        library.add_tracks(vec![rescanned]).expect("rescan");
    }

    let (handle, events) = analysis::spawn(&fixture.db).expect("spawn");
    let seen = run_pass(&handle, &events, false);
    assert_eq!(
        seen.last(),
        Some(&Event::ReplayGainAnalysisFinished {
            analysed: 3,
            failed: 1,
            cancelled: false,
        }),
        "the pass finished with one file it could not read"
    );
    handle.shutdown();

    let library = fixture.library();
    assert!(
        library.computed_replay_gain(&fixture.tracks[1]).is_empty(),
        "nothing is invented for a file that could not be read"
    );
    // The two that decoded were measured, and share an album figure computed
    // across the two of them: 10·log10((10^−2.3 + 10^−1.3)/2) = −15.60 LUFS,
    // so −2.40 dB. (The corrupt track was the quiet one the relative gate would
    // have removed anyway, which is why this number matches the intact album's.)
    assert_close(
        "the intact track",
        stored_track_gain(&library, &fixture.tracks[0]),
        EXPECTED_TRACK_GAINS[0],
    );
    assert_close(
        "and its album figure",
        stored_album_gain(&library, &fixture.tracks[0]),
        EXPECTED_ALBUM_GAIN,
    );
}

/// A mono file is measured as one channel, so its figure matches what every
/// other scanner would have written — the 3.01 LU that
/// `tests/loudness.rs` pins at the meter, checked here through a real decode.
///
/// baz's decoder duplicates mono into both channels, so a pass that fed the
/// meter what the decoder emits would normalise every mono track in a library
/// three decibels too quiet.
#[test]
fn a_mono_file_is_measured_as_the_mono_it_is() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("library.db");
    let stereo = dir.path().join("stereo.wav");
    let mono = dir.path().join("mono.wav");
    let signal = tone(5.0, -23.0);
    write_wav(&stereo, &signal);
    write_mono_wav(&mono, &signal);
    {
        let mut library = Library::open(&db).expect("open");
        library
            .add_tracks(vec![
                row(&stereo, "Stereo", "Tone", 1, AudioFormat::Wav),
                row(&mono, "Mono", "Tone", 1, AudioFormat::Wav),
            ])
            .expect("seed");
    }

    let (handle, events) = analysis::spawn(&db).expect("spawn");
    run_pass(&handle, &events, false);
    handle.shutdown();

    let library = Library::open(&db).expect("open");
    let stereo_gain = stored_track_gain(&library, &stereo).expect("stereo measured");
    let mono_gain = stored_track_gain(&library, &mono).expect("mono measured");
    // The mono file is 3.01 LU quieter as measured, so it asks for 3.01 dB
    // *more* gain: 301 centidecibels, to within the tolerance the measurement
    // itself carries.
    assert!(
        (mono_gain - stereo_gain - 301).abs() <= TOLERANCE_CENTIDB,
        "mono {mono_gain} vs stereo {stereo_gain} centidB: the pass must measure the \
         source's own channel count"
    );
    assert_close("the stereo tone", Some(stereo_gain), 500);
}

/// Dropping the handle stops a running pass and joins the worker, so no thread
/// outlives the front end that started it.
#[test]
fn dropping_the_handle_stops_the_pass() {
    let fixture = Fixture::new();
    let (handle, events) = analysis::spawn(&fixture.db).expect("spawn");
    handle
        .send(AnalysisCommand::StartReplayGainAnalysis { redo: false })
        .expect("start");
    drain_until(&events, |event| {
        matches!(event, Event::ReplayGainAnalysisStarted { .. })
    });
    // Blocks until the worker has stopped; a test that returns is the
    // assertion, and a leaked thread would hang it here.
    drop(handle);
    // The event channel is closed once the worker is gone, so draining it
    // terminates rather than blocking.
    while events.recv_timeout(PATIENCE).is_ok() {}
}

/// A cancel sent while nothing is running does not stop the pass that comes
/// after it: stale state is how a feature comes to refuse to work for reasons
/// nobody can see.
#[test]
fn a_cancel_while_idle_does_not_stop_the_next_pass() {
    let fixture = Fixture::new();
    let (handle, events) = analysis::spawn(&fixture.db).expect("spawn");
    handle
        .send(AnalysisCommand::CancelReplayGainAnalysis)
        .expect("cancel nothing");
    // Nothing is running, so nothing is announced.
    assert_eq!(
        events.recv_timeout(Duration::from_millis(200)),
        Err(RecvTimeoutError::Timeout),
        "a redundant command emits nothing"
    );

    let seen = run_pass(&handle, &events, false);
    assert_eq!(
        seen.last(),
        Some(&Event::ReplayGainAnalysisFinished {
            analysed: 3,
            failed: 0,
            cancelled: false,
        }),
        "the next pass runs to its end"
    );
    handle.shutdown();
}

/// Write the same tone as a **5.1** float WAV, in the two front channels only.
///
/// `hound` synthesizes the channel mask as `(1 << channels) - 1`, which for six
/// channels is `0x3F` — `FL+FR+FC+LFE+BL+BR`, exactly the 5.1 layout WAVE
/// specifies. That is a coincidence worth relying on here and nowhere else;
/// `tests/playback.rs` writes its own header when it needs a mask hound cannot
/// produce.
fn write_five_one_front_pair_wav(path: &Path, interleaved_stereo: &[f32]) {
    let spec = hound::WavSpec {
        channels: 6,
        sample_rate: RATE,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(path, spec).expect("create wav");
    for frame in interleaved_stereo.chunks_exact(2) {
        writer.write_sample(frame[0]).expect("write sample"); // FL
        writer.write_sample(frame[1]).expect("write sample"); // FR
        for _ in 0..4 {
            writer.write_sample(0.0f32).expect("write sample"); // FC, LFE, BL, BR
        }
    }
    writer.finalize().expect("finalize wav");
}

/// A multichannel file is measured as **the stereo downmix baz will play**, not
/// as the six channels it stores — and that is what makes the downmix's
/// attenuation cost a listener nothing on an analysed library (ADR-0039).
///
/// The fixture is the same tone twice: once as stereo, once as a 5.1 file
/// carrying it in the front pair and silence in the other four. baz folds the
/// second with the BS.775 matrix, whose 5.1 headroom scale is
/// `1/(1 + 2/√2) = 0.41421` — **−7.66 dB** — so the audio the meter sees is
/// 7.66 dB quieter than the stereo file's, and the pass therefore asks for
/// 7.66 dB *more* gain: **766 centidecibels**, worked out from the matrix and
/// not from a previous run.
///
/// That is the whole argument for taking headroom as a constant rather than
/// limiting: it is a level change, and a level change is the one kind of damage
/// ReplayGain undoes exactly. A 5.1 record on an analysed library plays at the
/// same loudness as its stereo master.
///
/// Deliberately *not* what a 5.1-aware scanner reports for the same file:
/// BS.1770 weights a six-channel programme differently, and would give a figure
/// describing audio baz never produces. Measuring what will actually be played
/// is the right answer for a gain that will actually be applied.
#[test]
fn a_multichannel_source_is_measured_as_its_downmix() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("library.db");
    let stereo = dir.path().join("stereo.wav");
    let surround = dir.path().join("surround.wav");
    let signal = tone(5.0, -23.0);
    write_wav(&stereo, &signal);
    write_five_one_front_pair_wav(&surround, &signal);
    {
        let mut library = Library::open(&db).expect("open");
        library
            .add_tracks(vec![
                row(&stereo, "Stereo", "Tone", 1, AudioFormat::Wav),
                row(&surround, "Surround", "Tone", 1, AudioFormat::Wav),
            ])
            .expect("seed");
    }

    let (handle, events) = analysis::spawn(&db).expect("spawn");
    run_pass(&handle, &events, false);
    handle.shutdown();

    let library = Library::open(&db).expect("open");
    let stereo_gain = stored_track_gain(&library, &stereo).expect("stereo measured");
    let surround_gain = stored_track_gain(&library, &surround).expect("surround measured");
    let headroom_centidb =
        -2000.0 * (1.0 + 2.0 * f64::from(std::f32::consts::FRAC_1_SQRT_2)).log10();
    #[allow(clippy::cast_possible_truncation)] // a few hundred centidB
    let expected = -headroom_centidb.round() as i16;
    assert_eq!(expected, 766, "the 5.1 matrix takes 7.66 dB, on paper");
    assert!(
        (surround_gain - stereo_gain - expected).abs() <= TOLERANCE_CENTIDB,
        "surround {surround_gain} vs stereo {stereo_gain} centidB: the pass must measure \
         the downmix baz plays, and so ask for {expected} centidB more gain"
    );
}
