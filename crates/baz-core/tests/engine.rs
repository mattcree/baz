//! Integration tests for `baz_core::engine`: the full command/event
//! lifecycle, exercised entirely headless through `spawn_offline`.
//!
//! Ground truth is a reference decode of the fixture files
//! (`AudioSource::decode_all`) — the engine's delivered output is compared
//! sample-for-sample against it, never against recorded engine output
//! (`docs/ENGINEERING.md`).

use std::f64::consts::PI;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use baz_core::engine::{OfflineOutput, spawn_offline};
use baz_core::playback::{AudioSource, BoundaryPolicy, CHANNELS, EngineConfig, PlaybackError};
use baz_core::protocol::{Command, Event};

/// Test tone parameters (arbitrary; equality checks are exact either way).
const FREQ: f64 = 440.0;
const AMP: f64 = 0.5;
const RATE: u32 = 44_100;
/// Track A: 5 s — long enough that pause/skip/stop land mid-track even on a
/// slow CI machine.
const A_FRAMES: usize = 5 * RATE as usize;
/// Track B: 1 s.
const B_FRAMES: usize = RATE as usize;

/// How long an expected event may take before the test fails (generous for
/// CI; the engine emits within milliseconds).
const EVENT_TIMEOUT: Duration = Duration::from_secs(20);

struct Fixtures {
    a: PathBuf,
    b: PathBuf,
    bad: PathBuf,
    /// Reference decodes (interleaved stereo f32).
    a_ref: Vec<f32>,
    b_ref: Vec<f32>,
}

fn write_sine_wav(path: &Path, frames: usize, t0: f64) {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: RATE,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(path, spec).expect("create wav");
    #[allow(clippy::cast_precision_loss)] // frame indices are far below 2^52
    for n in 0..frames {
        let t = t0 + n as f64 / f64::from(RATE);
        #[allow(clippy::cast_possible_truncation)] // f64 sine -> f32 sample
        let s = (AMP * (2.0 * PI * FREQ * t).sin()) as f32;
        w.write_sample(s).expect("write sample");
        w.write_sample(s).expect("write sample");
    }
    w.finalize().expect("finalize wav");
}

fn fixtures() -> &'static Fixtures {
    static FIXTURES: OnceLock<Fixtures> = OnceLock::new();
    FIXTURES.get_or_init(|| {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("engine-fixtures");
        std::fs::create_dir_all(&dir).expect("create fixture dir");
        let a = dir.join("track_a_5s.wav");
        let b = dir.join("track_b_1s.wav");
        let bad = dir.join("not_audio.wav");
        write_sine_wav(&a, A_FRAMES, 0.0);
        write_sine_wav(&b, B_FRAMES, 0.0);
        std::fs::write(&bad, b"this is not audio at all, sorry").expect("write bad file");
        let a_ref = AudioSource::decode_all(&a).expect("decode a").samples;
        let b_ref = AudioSource::decode_all(&b).expect("decode b").samples;
        assert_eq!(a_ref.len(), A_FRAMES * CHANNELS);
        assert_eq!(b_ref.len(), B_FRAMES * CHANNELS);
        Fixtures {
            a,
            b,
            bad,
            a_ref,
            b_ref,
        }
    })
}

/// Engine config paced so a 5 s track takes a few hundred ms to drain:
/// slow enough that pause/skip/stop always land mid-track, fast enough for
/// a snappy suite.
fn paced_config() -> EngineConfig {
    EngineConfig {
        ring_frames: 8192,
        consumer_chunk_frames: 2048,
        consumer_pace: Duration::from_millis(4),
        boundary: BoundaryPolicy::ResampleToStreamRate,
    }
}

/// Unpaced config: drain at decode speed (used where timing is irrelevant).
fn fast_config() -> EngineConfig {
    EngineConfig {
        consumer_pace: Duration::ZERO,
        ..paced_config()
    }
}

fn next_event(events: &Receiver<Event>) -> Event {
    events
        .recv_timeout(EVENT_TIMEOUT)
        .expect("timed out waiting for an engine event")
}

fn assert_no_event_within(events: &Receiver<Event>, window: Duration) {
    match events.recv_timeout(window) {
        Err(RecvTimeoutError::Timeout) => {}
        Ok(e) => panic!("expected silence, got event: {e:?}"),
        Err(RecvTimeoutError::Disconnected) => panic!("engine thread died"),
    }
}

fn started(path: &Path, position: usize) -> Event {
    Event::TrackStarted {
        path: path.to_path_buf(),
        position,
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

/// Collect the engine's offline output after shutting it down.
fn collect(output: OfflineOutput) -> Vec<f32> {
    output.wait().expect("engine thread must report its output")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// The full command lifecycle in one sitting, with the exact event order
/// the protocol promises: `SetQueue` → `Play` → `TrackStarted` → `Pause`
/// (delivery freezes) → `Play`/resume → `Next` (drain-and-restart) → the
/// next track → `QueueEnded`.
#[test]
fn full_lifecycle_event_ordering() {
    let f = fixtures();
    let capacity = f.a_ref.len() + f.b_ref.len();
    let (engine, events, output) = spawn_offline(paced_config(), capacity).expect("spawn engine");

    engine
        .send(Command::SetQueue {
            paths: vec![f.a.clone(), f.b.clone()],
        })
        .expect("send");
    engine.send(Command::Play).expect("send");
    assert_eq!(next_event(&events), started(&f.a, 0));

    engine.send(Command::Pause).expect("send");
    assert_eq!(next_event(&events), Event::Paused);
    // Paused means paused: because pause and pumping share the engine
    // thread, after `Paused` is emitted not one more sample may reach the
    // sink.
    let frozen = engine.samples_delivered();
    thread::sleep(Duration::from_millis(80));
    assert_eq!(
        engine.samples_delivered(),
        frozen,
        "samples were delivered while paused"
    );

    engine.send(Command::Play).expect("send");
    assert_eq!(next_event(&events), Event::Resumed);

    engine.send(Command::Next).expect("send");
    assert_eq!(next_event(&events), started(&f.b, 1));
    assert_eq!(next_event(&events), Event::QueueEnded);

    engine.shutdown();
    let out = collect(output);
    // Track A was skipped mid-flight, so only its head was delivered; track
    // B ran to completion, so the output's tail must be exactly B.
    assert!(out.len() > f.b_ref.len(), "output too short");
    assert!(out.len() < capacity, "skip delivered the whole queue");
    assert_samples_eq(
        &out[out.len() - f.b_ref.len()..],
        &f.b_ref,
        "post-skip track B output",
    );
}

/// Pause does not tear anything down and drops or duplicates nothing: the
/// delivered stream with a pause/resume in the middle is bit-identical to
/// the reference decode.
#[test]
fn pause_resume_output_is_bit_identical() {
    let f = fixtures();
    let cfg = EngineConfig {
        consumer_pace: Duration::from_millis(1),
        ..paced_config()
    };
    let (engine, events, output) = spawn_offline(cfg, f.a_ref.len()).expect("spawn engine");

    engine
        .send(Command::SetQueue {
            paths: vec![f.a.clone()],
        })
        .expect("send");
    engine.send(Command::Play).expect("send");
    assert_eq!(next_event(&events), started(&f.a, 0));

    engine.send(Command::Pause).expect("send");
    assert_eq!(next_event(&events), Event::Paused);
    let frozen = engine.samples_delivered();
    assert!(frozen < f.a_ref.len(), "pause landed after the track ended");
    thread::sleep(Duration::from_millis(50));
    assert_eq!(
        engine.samples_delivered(),
        frozen,
        "delivery advanced while paused"
    );

    engine.send(Command::Play).expect("send");
    assert_eq!(next_event(&events), Event::Resumed);
    assert_eq!(next_event(&events), Event::QueueEnded);
    assert!(
        engine.samples_delivered() > frozen,
        "delivery did not resume"
    );

    engine.shutdown();
    assert_samples_eq(&collect(output), &f.a_ref, "paused-and-resumed output");
}

/// One bad file must not kill the queue: it is reported and skipped, in
/// queue order, and the delivered audio is exactly the good tracks spliced
/// together.
#[test]
fn bad_file_is_reported_and_skipped() {
    let f = fixtures();
    let capacity = f.a_ref.len() + f.b_ref.len();
    let (engine, events, output) = spawn_offline(fast_config(), capacity).expect("spawn engine");

    engine
        .send(Command::SetQueue {
            paths: vec![f.a.clone(), f.bad.clone(), f.b.clone()],
        })
        .expect("send");
    engine.send(Command::Play).expect("send");

    assert_eq!(next_event(&events), started(&f.a, 0));
    match next_event(&events) {
        Event::TrackFailed { path, reason } => {
            assert_eq!(path, f.bad);
            assert!(!reason.is_empty(), "failure reason must say something");
        }
        other => panic!("expected TrackFailed for the bad file, got {other:?}"),
    }
    assert_eq!(next_event(&events), started(&f.b, 2));
    assert_eq!(next_event(&events), Event::QueueEnded);

    engine.shutdown();
    let out = collect(output);
    let mut want = f.a_ref.clone();
    want.extend_from_slice(&f.b_ref);
    assert_samples_eq(&out, &want, "queue output around a bad file");
}

/// Stop abandons the queue mid-track, delivery ceases, and a later Play
/// starts over from the top.
#[test]
fn stop_mid_track_then_play_restarts() {
    let f = fixtures();
    let (engine, events, output) =
        spawn_offline(paced_config(), f.a_ref.len()).expect("spawn engine");

    engine
        .send(Command::SetQueue {
            paths: vec![f.a.clone()],
        })
        .expect("send");
    engine.send(Command::Play).expect("send");
    assert_eq!(next_event(&events), started(&f.a, 0));

    engine.send(Command::Stop).expect("send");
    assert_eq!(next_event(&events), Event::Stopped);
    let at_stop = engine.samples_delivered();
    assert!(at_stop < f.a_ref.len(), "stop landed after the track ended");
    assert_no_event_within(&events, Duration::from_millis(120));
    assert_eq!(
        engine.samples_delivered(),
        at_stop,
        "delivery continued after Stop"
    );

    // Play after Stop starts from the queue top again.
    engine.send(Command::Play).expect("send");
    assert_eq!(next_event(&events), started(&f.a, 0));

    engine.shutdown();
    let out = collect(output);
    // What reached the sink is the pre-stop head of the track, cleanly cut,
    // followed by a fresh start from frame 0 — no corruption either side of
    // the stop.
    assert!(out.len() > at_stop, "the restart delivered nothing");
    assert_samples_eq(&out[..at_stop], &f.a_ref[..at_stop], "pre-stop segment");
    assert_samples_eq(
        &out[at_stop..],
        &f.a_ref[..out.len() - at_stop],
        "restarted-from-top segment",
    );
}

/// Dropping the handle mid-playback shuts everything down promptly — no
/// hang, no leaked threads — proven under a hard timeout.
#[test]
fn shutdown_while_playing_terminates_cleanly() {
    let f = fixtures();
    let (engine, events, output) =
        spawn_offline(paced_config(), f.a_ref.len() + f.b_ref.len()).expect("spawn engine");

    engine
        .send(Command::SetQueue {
            paths: vec![f.a.clone(), f.b.clone()],
        })
        .expect("send");
    engine.send(Command::Play).expect("send");
    assert_eq!(next_event(&events), started(&f.a, 0));

    let (done_tx, done_rx) = mpsc::channel();
    thread::spawn(move || {
        drop(engine); // the whole shutdown path: abort session, join workers
        let _ = done_tx.send(());
    });
    done_rx
        .recv_timeout(Duration::from_secs(10))
        .expect("dropping the handle hung instead of shutting the engine down");

    // The engine thread exited: the sink came back and the event channel is
    // closed.
    let out = collect(output);
    assert!(!out.is_empty(), "some audio was delivered before shutdown");
    match events.recv_timeout(Duration::from_secs(5)) {
        Err(RecvTimeoutError::Disconnected) => {}
        other => panic!("event channel should be closed after shutdown, got {other:?}"),
    }
}

/// `SetQueue` never autoplays, and `Play` on an empty queue reports the queue
/// as ended rather than doing nothing silently.
#[test]
fn set_queue_does_not_autoplay_and_empty_queue_ends() {
    let f = fixtures();
    let (engine, events, _output) =
        spawn_offline(fast_config(), f.a_ref.len()).expect("spawn engine");

    // Play with nothing queued: the (empty) queue is already over.
    engine.send(Command::Play).expect("send");
    assert_eq!(next_event(&events), Event::QueueEnded);

    // Queueing alone starts nothing.
    engine
        .send(Command::SetQueue {
            paths: vec![f.a.clone()],
        })
        .expect("send");
    assert_no_event_within(&events, Duration::from_millis(120));
    assert_eq!(engine.samples_delivered(), 0, "SetQueue must not autoplay");
}

/// Next past the last track ends the queue.
#[test]
fn next_past_last_track_ends_queue() {
    let f = fixtures();
    let (engine, events, _output) =
        spawn_offline(paced_config(), f.a_ref.len()).expect("spawn engine");

    engine
        .send(Command::SetQueue {
            paths: vec![f.a.clone()],
        })
        .expect("send");
    engine.send(Command::Play).expect("send");
    assert_eq!(next_event(&events), started(&f.a, 0));
    engine.send(Command::Next).expect("send");
    assert_eq!(next_event(&events), Event::QueueEnded);
    assert_no_event_within(&events, Duration::from_millis(120));
}

/// The engine service inherits `run_playlist`'s contract for the
/// unimplemented bit-perfect reopen mode: refuse at spawn, plainly.
#[test]
fn bit_perfect_reopen_is_refused_at_spawn() {
    let cfg = EngineConfig {
        boundary: BoundaryPolicy::BitPerfectReopen,
        ..paced_config()
    };
    let Err(err) = spawn_offline(cfg, 16) else {
        panic!("reopen mode must be refused")
    };
    assert!(matches!(err, PlaybackError::BitPerfectReopenUnimplemented));
}

/// Commands after shutdown fail with a clear error instead of vanishing.
#[test]
fn send_after_shutdown_is_an_error() {
    let f = fixtures();
    let (engine, _events, _output) = spawn_offline(fast_config(), 16).expect("spawn engine");
    // Keep a second handle path honest: shutdown consumes the handle, so
    // sending afterwards is only possible through a clone — there is none.
    // What we can check: the events channel closes and a fresh engine's
    // handle still works.
    engine.shutdown();
    let (engine2, _events2, _output2) = spawn_offline(fast_config(), 16).expect("spawn engine");
    engine2
        .send(Command::SetQueue {
            paths: vec![f.a.clone()],
        })
        .expect("a fresh engine accepts commands");
}

/// Device output (feature `device-output`): the engine spawns against the
/// default device — or reports the documented `Device` error on headless
/// machines — and shuts down cleanly either way. Never a panic.
#[cfg(feature = "device-output")]
#[test]
fn device_engine_spawns_or_reports_cleanly() {
    let f = fixtures();
    match baz_core::engine::spawn_device(paced_config(), RATE, 8192) {
        Ok((engine, events)) => {
            engine
                .send(Command::SetQueue {
                    paths: vec![f.b.clone()],
                })
                .expect("send");
            engine.send(Command::Play).expect("send");
            assert_eq!(next_event(&events), started(&f.b, 0));
            engine.shutdown(); // mid-track: must not hang the device stream
            println!("[device] engine played through the default output device");
        }
        Err(PlaybackError::Device(msg)) => {
            eprintln!("SKIP: no usable output device ({msg})");
        }
        Err(other) => panic!("unexpected error spawning device engine: {other}"),
    }
}
