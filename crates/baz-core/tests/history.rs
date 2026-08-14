//! Integration tests for the play-history ledger (ADR-0018): a real engine
//! run, and then assertions **on the file** rather than on intentions.
//!
//! Everything here is headless (`spawn_offline`) and everything here writes to
//! a `tempfile` directory. Nothing touches the user's data directory: the
//! engine's ledger slot is empty by default and these tests open one by path.

use std::f64::consts::PI;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, SystemTime};

use baz_core::engine::{EngineHandle, OfflineOutput, spawn_offline};
use baz_core::history::{
    History, HistoryLedger, PLAY_THRESHOLD_CAP_MS, PlayRecord, Recency, play_threshold_ms,
};
use baz_core::playback::{CHANNELS, EngineConfig};
use baz_core::protocol::{Command, Event, PlayOutcome};

/// Test tone parameters. Nothing here listens, so the shape is arbitrary.
const FREQ: f64 = 440.0;
const AMP: f64 = 0.5;
const RATE: u32 = 44_100;

/// Five seconds — comfortably past its own play threshold (2 500 ms) when
/// played through, so "played" is a fact about the run rather than a race.
const A_SECS: usize = 5;
/// One second, for the second track of the album.
const B_SECS: usize = 1;
/// Thirty seconds. Its threshold is 15 000 ms, which at the pump rate these
/// tests configure takes seconds of wall time to deliver — so a `Next` sent as
/// soon as the track starts is a skip by a margin no CI machine can close.
const LONG_SECS: usize = 30;

const EVENT_TIMEOUT: Duration = Duration::from_secs(20);

/// Sample capacity for the offline sink: everything these tests can play.
const SINK_CAPACITY: usize = (LONG_SECS + A_SECS + B_SECS + 8) * RATE as usize * CHANNELS;

fn config() -> EngineConfig {
    EngineConfig {
        // A small chunk and a real pace, so that a command sent right after
        // `TrackStarted` lands while there is still most of a track left.
        ring_frames: 4096,
        consumer_chunk_frames: 1024,
        consumer_pace: Duration::from_millis(5),
        ..EngineConfig::default()
    }
}

fn write_sine_wav(path: &Path, secs: usize) {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: RATE,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(path, spec).expect("create wav");
    #[allow(clippy::cast_precision_loss)] // frame indices are far below 2^52
    for n in 0..secs * RATE as usize {
        let t = n as f64 / f64::from(RATE);
        #[allow(clippy::cast_possible_truncation)] // f64 sine -> f32 sample
        let sample = (AMP * (2.0 * PI * FREQ * t).sin()) as f32;
        writer.write_sample(sample).expect("write sample");
        writer.write_sample(sample).expect("write sample");
    }
    writer.finalize().expect("finalize wav");
}

/// An engine with a ledger already attached, in a scratch directory.
struct Rig {
    _dir: tempfile::TempDir,
    ledger_path: PathBuf,
    ledger: Arc<HistoryLedger>,
    engine: Option<EngineHandle>,
    events: Receiver<Event>,
    output: Option<OfflineOutput>,
    a: PathBuf,
    b: PathBuf,
    long: PathBuf,
}

impl Rig {
    fn new() -> Self {
        Self::with_config(config())
    }

    fn with_config(config: EngineConfig) -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let a = dir.path().join("01 a.wav");
        let b = dir.path().join("02 b.wav");
        let long = dir.path().join("03 long.wav");
        write_sine_wav(&a, A_SECS);
        write_sine_wav(&b, B_SECS);
        write_sine_wav(&long, LONG_SECS);
        let ledger_path = dir.path().join("history.tsv");
        let ledger = Arc::new(HistoryLedger::open(&ledger_path).expect("open ledger"));
        let (engine, events, output) = spawn_offline(config, SINK_CAPACITY).expect("spawn");
        engine.set_history(Some(Arc::clone(&ledger)));
        Self {
            _dir: dir,
            ledger_path,
            ledger,
            engine: Some(engine),
            events,
            output: Some(output),
            a,
            b,
            long,
        }
    }

    fn send(&self, command: Command) {
        self.engine
            .as_ref()
            .expect("engine")
            .send(command)
            .expect("send");
    }

    /// Wait for the next event matching `want`, returning it.
    fn wait_for(&self, want: impl Fn(&Event) -> bool) -> Event {
        let mut observed = Vec::new();
        loop {
            match self.events.recv_timeout(EVENT_TIMEOUT) {
                Ok(event) if want(&event) => return event,
                Ok(event) => observed.push(format!("{event:?}")),
                Err(RecvTimeoutError::Timeout) => panic!(
                    "timed out waiting for an event; observed={observed:#?}, written={}, failures={}",
                    self.ledger.written(),
                    self.ledger.write_failures()
                ),
                Err(RecvTimeoutError::Disconnected) => panic!("the engine went away"),
            }
        }
    }

    /// Shut the engine down and wait for every queued line to be written.
    fn finish(&mut self) {
        drop(self.engine.take());
        if let Some(output) = self.output.take() {
            let _ = output.wait();
        }
        self.ledger.flush();
    }

    /// Every record line in the file, in order.
    fn records(&self) -> Vec<PlayRecord> {
        std::fs::read_to_string(&self.ledger_path)
            .expect("read ledger")
            .lines()
            .filter_map(PlayRecord::from_line)
            .collect()
    }

    /// Every record line, as `(outcome, path)` — the part of a line that is a
    /// fact about the run rather than about the clock or the pump.
    fn outcomes(&self) -> Vec<(PlayOutcome, PathBuf)> {
        self.records()
            .into_iter()
            .map(|record| (record.outcome, record.path))
            .collect()
    }
}

/// The headline assertion the ADR promises: a run of playback produces exactly
/// these lines, in the file, in this order.
#[test]
fn a_run_of_playback_writes_exactly_the_expected_lines() {
    let mut rig = Rig::new();
    let before = SystemTime::now();
    rig.send(Command::SetQueue {
        paths: vec![rig.a.clone(), rig.b.clone()],
        origin: None,
    });
    rig.send(Command::Play);
    rig.wait_for(|event| matches!(event, Event::QueueEnded));
    rig.finish();

    let records = rig.records();
    assert_eq!(records.len(), 2, "{records:#?}");

    // Track A: five seconds, all of it delivered.
    assert_eq!(records[0].path, rig.a);
    assert_eq!(records[0].outcome, PlayOutcome::Played);
    assert_eq!(records[0].listened_ms, (A_SECS as u64) * 1_000);
    assert_eq!(records[0].track_ms, Some((A_SECS as u64) * 1_000));
    // Track B: one second, all of it delivered.
    assert_eq!(records[1].path, rig.b);
    assert_eq!(records[1].outcome, PlayOutcome::Played);
    assert_eq!(records[1].listened_ms, (B_SECS as u64) * 1_000);
    assert_eq!(records[1].track_ms, Some((B_SECS as u64) * 1_000));

    // The timestamps are this run's, in order, and are when each play
    // *started* rather than when it was written.
    let after = SystemTime::now();
    for record in &records {
        assert!(record.started() >= before - Duration::from_secs(1));
        assert!(record.started() <= after);
    }
    assert!(records[0].started_unix_s <= records[1].started_unix_s);

    // And the bytes themselves are the documented format, header included.
    let text = std::fs::read_to_string(&rig.ledger_path).expect("read");
    assert!(text.starts_with("# baz play history."));
    assert!(text.ends_with('\n'));
    let record_lines: Vec<&str> = text
        .lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
        .collect();
    assert_eq!(record_lines.len(), 2);
    for line in record_lines {
        assert_eq!(line.matches('\t').count(), 4, "{line:?}");
    }
}

/// The engine writes nothing at all until a front end hands it a ledger — the
/// default that keeps every other test in this workspace off the disk.
#[test]
fn an_engine_with_no_ledger_writes_nothing() {
    let mut rig = Rig::new();
    rig.engine.as_ref().expect("engine").set_history(None);
    rig.send(Command::SetQueue {
        paths: vec![rig.a.clone(), rig.b.clone()],
        origin: None,
    });
    rig.send(Command::Play);
    rig.wait_for(|event| matches!(event, Event::QueueEnded));
    rig.finish();
    assert!(rig.records().is_empty());
    assert_eq!(rig.ledger.written(), 0);
}

/// A track left before the threshold is written as a skip — not omitted, and
/// not filed as a play.
#[test]
fn a_track_left_early_is_recorded_as_a_skip() {
    let mut rig = Rig::new();
    rig.send(Command::SetQueue {
        paths: vec![rig.long.clone(), rig.b.clone()],
        origin: None,
    });
    rig.send(Command::Play);
    rig.wait_for(|event| matches!(event, Event::TrackStarted { position: 0, .. }));
    rig.send(Command::Next);
    rig.wait_for(|event| matches!(event, Event::QueueEnded));
    rig.finish();

    let records = rig.records();
    assert_eq!(
        rig.outcomes(),
        vec![
            (PlayOutcome::Skipped, rig.long.clone()),
            (PlayOutcome::Played, rig.b.clone()),
        ],
        "{records:#?}"
    );
    let skipped = &records[0];
    assert!(skipped.listened_ms > 0, "a skip is still something heard");
    assert!(
        skipped.listened_ms < play_threshold_ms(skipped.track_ms),
        "{skipped:?}"
    );
    assert_eq!(skipped.track_ms, Some((LONG_SECS as u64) * 1_000));
}

/// A queue entry nothing was ever heard of gets no line. A ledger of things
/// that did not happen is not a ledger.
#[test]
fn a_queue_entry_that_was_jumped_over_is_never_recorded() {
    let mut rig = Rig::new();
    rig.send(Command::SetQueue {
        paths: vec![rig.long.clone(), rig.a.clone(), rig.b.clone()],
        origin: None,
    });
    // Straight to the last entry: nothing before it is ever delivered.
    rig.send(Command::JumpTo { position: 2 });
    rig.wait_for(|event| matches!(event, Event::QueueEnded));
    rig.finish();
    assert_eq!(rig.outcomes(), vec![(PlayOutcome::Played, rig.b.clone())]);
}

/// Stopping mid-track still files what was heard: the play is written when the
/// listening ends, not only when a track runs out.
#[test]
fn stopping_mid_track_still_files_the_play() {
    let mut rig = Rig::new();
    rig.send(Command::SetQueue {
        paths: vec![rig.long.clone()],
        origin: None,
    });
    rig.send(Command::Play);
    rig.wait_for(|event| matches!(event, Event::TrackStarted { .. }));
    rig.send(Command::Stop);
    rig.wait_for(|event| matches!(event, Event::Stopped));
    rig.finish();
    assert_eq!(
        rig.outcomes(),
        vec![(PlayOutcome::Skipped, rig.long.clone())]
    );
}

/// Shutting the engine down is the front end going away, and a front end going
/// away must not cost the album somebody just heard.
#[test]
fn shutting_the_engine_down_files_the_play_in_progress() {
    let mut rig = Rig::new();
    rig.send(Command::SetQueue {
        paths: vec![rig.long.clone()],
        origin: None,
    });
    rig.send(Command::Play);
    rig.wait_for(|event| matches!(event, Event::TrackStarted { .. }));
    // No Stop, no QueueEnded: just drop the handle, as a closing front end does.
    rig.finish();
    let records = rig.records();
    assert_eq!(records.len(), 1, "{records:#?}");
    assert_eq!(records[0].path, rig.long);
    assert!(records[0].listened_ms > 0);
}

/// Seeking inside a track is one listening act, not several. Four drags of the
/// needle must not file five half-listens.
#[test]
fn seeking_inside_a_track_is_one_play() {
    let mut rig = Rig::new();
    rig.send(Command::SetQueue {
        paths: vec![rig.long.clone()],
        origin: None,
    });
    rig.send(Command::Play);
    rig.wait_for(|event| matches!(event, Event::TrackStarted { .. }));
    for target in [2_000, 6_000, 12_000, 20_000] {
        rig.send(Command::Seek {
            position_ms: target,
        });
        rig.wait_for(|event| matches!(event, Event::Progress { .. }));
    }
    rig.send(Command::Stop);
    rig.wait_for(|event| matches!(event, Event::Stopped));
    rig.finish();
    let records = rig.records();
    assert_eq!(records.len(), 1, "one listen, one line: {records:#?}");
    assert_eq!(records[0].path, rig.long);
}

/// Restarting a track *is* a new listen, and the two gestures must not be
/// confused — `Previous` past the restart threshold is not a seek.
#[test]
fn restarting_a_track_is_a_second_play() {
    let mut rig = Rig::new();
    rig.send(Command::SetQueue {
        paths: vec![rig.a.clone()],
        origin: None,
    });
    rig.send(Command::Play);
    rig.wait_for(|event| matches!(event, Event::TrackStarted { .. }));
    // A click on the row that is already playing: restart, per `JumpTo`.
    rig.send(Command::JumpTo { position: 0 });
    rig.wait_for(|event| matches!(event, Event::QueueEnded));
    rig.finish();
    let records = rig.records();
    assert_eq!(records.len(), 2, "{records:#?}");
    assert!(records.iter().all(|record| record.path == rig.a));
    // The second one ran to the end and is a play; the first was cut short.
    assert_eq!(records[1].outcome, PlayOutcome::Played);
    assert_eq!(records[1].listened_ms, (A_SECS as u64) * 1_000);
}

/// Pausing is not listening: an hour parked mid-track adds nothing to the
/// count, so a paused track cannot cross the threshold on its own.
#[test]
fn pausing_adds_nothing_to_what_was_heard() {
    let mut rig = Rig::new();
    rig.send(Command::SetQueue {
        paths: vec![rig.long.clone()],
        origin: None,
    });
    rig.send(Command::Play);
    rig.wait_for(|event| matches!(event, Event::TrackStarted { .. }));
    rig.send(Command::Pause);
    rig.wait_for(|event| matches!(event, Event::Paused));
    let paused_at = rig.engine.as_ref().expect("engine").samples_delivered();
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(
        rig.engine.as_ref().expect("engine").samples_delivered(),
        paused_at,
        "the pause gate leaked audio"
    );
    rig.send(Command::Stop);
    rig.wait_for(|event| matches!(event, Event::Stopped));
    rig.finish();

    let records = rig.records();
    assert_eq!(records.len(), 1);
    // Delivered frames, converted at the stream rate — the ledger's count is
    // the engine's own delivered-sample counter and nothing else.
    let delivered_ms = (paused_at as u64 / CHANNELS as u64) * 1_000 / u64::from(RATE);
    assert!(
        records[0].listened_ms.abs_diff(delivered_ms) <= 1,
        "listened {} vs delivered {delivered_ms}",
        records[0].listened_ms
    );
    assert_eq!(records[0].outcome, PlayOutcome::Skipped);
}

/// The event is news about a line that is already in the file — the
/// state-before-event contract, end to end through a real engine.
#[test]
fn the_play_recorded_event_follows_the_line_into_the_file() {
    // This test is about the writer's state-before-event ordering, not the
    // consumer clock. The shortest fixture keeps that proof quick.
    let mut rig = Rig::new();
    rig.send(Command::SetQueue {
        paths: vec![rig.b.clone()],
        origin: None,
    });
    rig.send(Command::Play);
    let event = rig.wait_for(|event| matches!(event, Event::PlayRecorded { .. }));
    // Read with no flush: if the event is honest, the line is already there.
    let history = History::read(&rig.ledger_path).expect("read");
    assert_eq!(history.track(&rig.b).plays, 1);
    let Event::PlayRecorded {
        path,
        listened_ms,
        track_ms,
        outcome,
        ..
    } = event
    else {
        unreachable!("filtered above")
    };
    assert_eq!(path, rig.b);
    assert_eq!(outcome, PlayOutcome::Played);
    assert_eq!(listened_ms, (B_SECS as u64) * 1_000);
    assert_eq!(track_ms, Some((B_SECS as u64) * 1_000));
    // `QueueEnded` comes from the engine while `PlayRecorded` comes from the
    // ledger writer, so it may already have been consumed by `wait_for` above.
    // Dropping the engine is the deterministic completion barrier here.
    rig.finish();
}

/// The read surfaces, read back off a ledger a real engine wrote — the card
/// and the group key, since the pull's weighting went with the pull
/// (ADR-0018's amended third surface).
#[test]
fn the_read_surfaces_answer_from_a_real_run() {
    let mut rig = Rig::new();
    rig.send(Command::SetQueue {
        paths: vec![rig.a.clone(), rig.b.clone()],
        origin: None,
    });
    rig.send(Command::Play);
    rig.wait_for(|event| matches!(event, Event::QueueEnded));
    // A second run through the same album.
    rig.send(Command::Play);
    rig.wait_for(|event| matches!(event, Event::QueueEnded));
    rig.finish();

    let history = History::read(&rig.ledger_path).expect("read");
    let now = SystemTime::now();

    // 1. The card.
    let card = history.track(&rig.a);
    assert_eq!(card.plays, 2);
    assert_eq!(card.skips, 0);
    assert!(card.ever_played());
    assert_eq!(card.listened_ms, 2 * (A_SECS as u64) * 1_000);

    // 2. The group key.
    assert_eq!(history.recency(&rig.a, now), Recency::ThisEvening);
    assert_eq!(history.recency(&rig.long, now), Recency::Never);
}

/// The threshold is the engine's, and it is reachable by a front end that wants
/// to explain the rule rather than reimplement it.
#[test]
fn the_threshold_is_public_and_says_what_the_ledger_did() {
    assert_eq!(play_threshold_ms(Some(300_000)), 150_000);
    assert_eq!(play_threshold_ms(Some(3_600_000)), PLAY_THRESHOLD_CAP_MS);
    assert_eq!(play_threshold_ms(None), PLAY_THRESHOLD_CAP_MS);
}

/// **The engine writes down which list a run came from** (ADR-0034 §2 and §4)
/// — the whole point of the origin on the command, asserted on the file.
///
/// The owner: *"when I play a song from a playlist it should only bump the
/// recency of that playlist, not the underlying albums"*. The live half of that
/// already worked; this is the half that survives a quit, because the engine
/// was finally told what it was playing from.
#[test]
fn a_run_told_which_list_it_came_from_says_so_in_the_ledger() {
    let mut rig = Rig::new();
    rig.send(Command::SetQueue {
        paths: vec![rig.a.clone(), rig.b.clone()],
        origin: Some("playlist:3b1f00c2a49d7e60:Road Trip".to_owned()),
    });
    rig.send(Command::Play);
    rig.wait_for(|event| matches!(event, Event::QueueEnded));
    rig.finish();

    let history = History::read(&rig.ledger_path).expect("read");
    assert_eq!(history.records(), 2);
    // A marker is a comment, so it is not damage — the property an older baz
    // depends on, checked here against a file a real engine wrote.
    assert_eq!(history.malformed(), 0);

    let runs = history.runs();
    assert_eq!(runs.len(), 1, "{runs:#?}");
    assert_eq!(
        runs[0].origin.as_deref(),
        Some("playlist:3b1f00c2a49d7e60:Road Trip")
    );
    assert_eq!(runs[0].plays, 2);

    // The tracks were played — and yet neither is a record the listener put
    // on, which is the attribution the lane folds at launch.
    for path in [&rig.a, &rig.b] {
        assert_eq!(history.track(path).plays, 1);
        assert_eq!(
            history.last_played_unlisted(path),
            None,
            "{} was credited to its record",
            path.display()
        );
    }

    // The marker sits above the plays it opens, and it is a comment.
    let text = std::fs::read_to_string(&rig.ledger_path).expect("read");
    let markers: Vec<&str> = text
        .lines()
        .filter(|line| line.starts_with("# baz run"))
        .collect();
    assert_eq!(markers.len(), 1, "{text}");
    assert!(
        markers[0].ends_with(" playlist:3b1f00c2a49d7e60:Road Trip"),
        "{:?}",
        markers[0]
    );
    let marker_at = text.find("# baz run").expect("the marker is in the file");
    let first_play = text
        .lines()
        .find(|line| !line.starts_with('#') && !line.is_empty())
        .expect("a play line");
    assert!(
        marker_at < text.find(first_play).expect("the play is in the file"),
        "the marker did not open the run"
    );
}

/// **A run with no origin still folds onto its records** — an album's run, a
/// draw, `Play all`. The marker's job is to stop a *playlist's* run crediting
/// its albums, not to stop an album being credited when an album is what
/// played.
#[test]
fn a_run_that_named_no_list_still_credits_the_records_it_played() {
    let mut rig = Rig::new();
    rig.send(Command::SetQueue {
        paths: vec![rig.a.clone(), rig.b.clone()],
        origin: None,
    });
    rig.send(Command::Play);
    rig.wait_for(|event| matches!(event, Event::QueueEnded));
    rig.finish();

    let history = History::read(&rig.ledger_path).expect("read");
    assert_eq!(history.malformed(), 0);
    for path in [&rig.a, &rig.b] {
        assert_eq!(
            history.last_played_unlisted(path),
            history.track(path).last_played_unix_s,
            "{} folded differently from an unmarked ledger",
            path.display()
        );
        assert!(history.last_played_unlisted(path).is_some());
    }
}

/// Two runs in one session, and the second does not inherit the first's list.
#[test]
fn a_second_run_replaces_the_first_runs_origin() {
    let mut rig = Rig::new();
    rig.send(Command::SetQueue {
        paths: vec![rig.a.clone()],
        origin: Some("playlist:1f:Road Trip".to_owned()),
    });
    rig.send(Command::Play);
    rig.wait_for(|event| matches!(event, Event::QueueEnded));
    rig.send(Command::SetQueue {
        paths: vec![rig.b.clone()],
        origin: None,
    });
    rig.send(Command::Play);
    rig.wait_for(|event| matches!(event, Event::QueueEnded));
    rig.finish();

    let history = History::read(&rig.ledger_path).expect("read");
    assert_eq!(history.records(), 2);
    assert_eq!(history.malformed(), 0);
    let runs = history.runs();
    assert_eq!(runs.len(), 2, "{runs:#?}");
    assert_eq!(runs[0].origin.as_deref(), Some("playlist:1f:Road Trip"));
    assert_eq!(runs[1].origin, None);
    // The list's track is not a record the listener put on; the second run's
    // is, because that run named no list.
    assert_eq!(history.last_played_unlisted(&rig.a), None);
    assert!(history.last_played_unlisted(&rig.b).is_some());
}

/// **A ledger this baz writes is one an older baz reads whole.** The claim is
/// made against the current parser's own reading — `PlayRecord::from_line`,
/// which is the entire vocabulary a v1 baz has — over a file a v1.1 engine
/// wrote with markers in it.
#[test]
fn an_older_baz_reads_every_play_in_a_marked_ledger() {
    let mut rig = Rig::new();
    rig.send(Command::SetQueue {
        paths: vec![rig.a.clone(), rig.b.clone()],
        origin: Some("playlist:1f:Road Trip".to_owned()),
    });
    rig.send(Command::Play);
    rig.wait_for(|event| matches!(event, Event::QueueEnded));
    rig.finish();

    let text = std::fs::read_to_string(&rig.ledger_path).expect("read");
    // What a v1 reader does, in the two lines it is: skip `#`, decode the rest.
    let mut read = 0usize;
    let mut damaged = 0usize;
    for line in text.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue; // not damage — the rule every reader baz has shipped
        }
        match PlayRecord::from_line(line) {
            Some(_) => read += 1,
            None => damaged += 1,
        }
    }
    assert_eq!(read, 2, "an older baz lost a play\n{text}");
    assert_eq!(damaged, 0, "an older baz saw damage\n{text}");
    // And every line of the file is still one of the two things it ever was.
    for line in text.lines() {
        assert!(
            line.starts_with('#') || PlayRecord::from_line(line).is_some(),
            "{line:?}"
        );
        assert_eq!(
            line.matches('\t').count(),
            usize::from(!line.starts_with('#')) * 4,
            "{line:?}"
        );
    }
}
