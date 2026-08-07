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

/// Linear chirp fixture: 6 s at [`RATE`], sweeping [`CHIRP_F0`] to
/// [`CHIRP_F1`]. Unlike a steady tone, no two moments of it look alike, so a
/// delivered block can be located in the source unambiguously — which is
/// what makes seek accuracy measurable from the *audio* rather than from a
/// counter the implementation also wrote.
const CHIRP_SECS: usize = 6;
const CHIRP_MS: u64 = 6_000;
const CHIRP_FRAMES: usize = CHIRP_SECS * RATE as usize;
const CHIRP_F0: f64 = 200.0;
const CHIRP_F1: f64 = 4400.0;

/// The rate-mismatch pair for the elapsed-under-resampling test: 1 s at the
/// stream rate, then 4 s at 48 kHz which the engine resamples down to it.
const HEAD_44K_FRAMES: usize = RATE as usize;
const TAIL_48K_RATE: u32 = 48_000;
const TAIL_48K_SECS: usize = 4;
const TAIL_48K_MS: u64 = 4_000;
const TAIL_48K_FRAMES: usize = TAIL_48K_SECS * TAIL_48K_RATE as usize;

/// How long an expected event may take before the test fails (generous for
/// CI; the engine emits within milliseconds).
const EVENT_TIMEOUT: Duration = Duration::from_secs(20);

struct Fixtures {
    a: PathBuf,
    b: PathBuf,
    bad: PathBuf,
    chirp: PathBuf,
    head_44k: PathBuf,
    tail_48k: PathBuf,
    /// Reference decodes (interleaved stereo f32).
    a_ref: Vec<f32>,
    b_ref: Vec<f32>,
    chirp_ref: Vec<f32>,
}

fn wav_spec(rate: u32) -> hound::WavSpec {
    hound::WavSpec {
        channels: 2,
        sample_rate: rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    }
}

fn write_sine_wav_at(path: &Path, rate: u32, frames: usize, t0: f64) {
    let mut w = hound::WavWriter::create(path, wav_spec(rate)).expect("create wav");
    #[allow(clippy::cast_precision_loss)] // frame indices are far below 2^52
    for n in 0..frames {
        let t = t0 + n as f64 / f64::from(rate);
        #[allow(clippy::cast_possible_truncation)] // f64 sine -> f32 sample
        let s = (AMP * (2.0 * PI * FREQ * t).sin()) as f32;
        w.write_sample(s).expect("write sample");
        w.write_sample(s).expect("write sample");
    }
    w.finalize().expect("finalize wav");
}

fn write_sine_wav(path: &Path, frames: usize, t0: f64) {
    write_sine_wav_at(path, RATE, frames, t0);
}

/// Instantaneous frequency of the chirp `t` seconds in.
fn chirp_freq_at(t: f64) -> f64 {
    #[allow(clippy::cast_precision_loss)]
    let total = CHIRP_SECS as f64;
    CHIRP_F0 + (CHIRP_F1 - CHIRP_F0) * t / total
}

/// A linear sine sweep: phase is the integral of the instantaneous
/// frequency, `2π(f0·t + k·t²/2)`.
fn write_chirp_wav(path: &Path) {
    #[allow(clippy::cast_precision_loss)]
    let total = CHIRP_SECS as f64;
    let k = (CHIRP_F1 - CHIRP_F0) / total;
    let mut w = hound::WavWriter::create(path, wav_spec(RATE)).expect("create wav");
    #[allow(clippy::cast_precision_loss)] // frame indices are far below 2^52
    for n in 0..CHIRP_FRAMES {
        let t = n as f64 / f64::from(RATE);
        #[allow(clippy::cast_possible_truncation)] // f64 sine -> f32 sample
        let s = (AMP * (2.0 * PI * (CHIRP_F0 * t + 0.5 * k * t * t)).sin()) as f32;
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
        let chirp = dir.join("chirp_6s.wav");
        let head_44k = dir.join("head_1s_44k.wav");
        let tail_48k = dir.join("tail_4s_48k.wav");
        write_sine_wav(&a, A_FRAMES, 0.0);
        write_sine_wav(&b, B_FRAMES, 0.0);
        write_chirp_wav(&chirp);
        write_sine_wav_at(&head_44k, RATE, HEAD_44K_FRAMES, 0.0);
        write_sine_wav_at(&tail_48k, TAIL_48K_RATE, TAIL_48K_FRAMES, 0.0);
        std::fs::write(&bad, b"this is not audio at all, sorry").expect("write bad file");
        let a_ref = AudioSource::decode_all(&a).expect("decode a").samples;
        let b_ref = AudioSource::decode_all(&b).expect("decode b").samples;
        let chirp_ref = AudioSource::decode_all(&chirp)
            .expect("decode chirp")
            .samples;
        assert_eq!(a_ref.len(), A_FRAMES * CHANNELS);
        assert_eq!(b_ref.len(), B_FRAMES * CHANNELS);
        assert_eq!(chirp_ref.len(), CHIRP_FRAMES * CHANNELS);
        Fixtures {
            a,
            b,
            bad,
            chirp,
            head_44k,
            tail_48k,
            a_ref,
            b_ref,
            chirp_ref,
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

/// The next event that is not [`Event::Progress`].
///
/// `Progress` is a continuous readout rather than a transport transition,
/// and it interleaves with everything by design; the tests that assert the
/// *transport* vocabulary's ordering therefore step over it. `Progress` has
/// its own contract (cadence, immediacy, the elapsed value itself) and its
/// own tests below — it is not going unasserted.
fn next_transport_event(events: &Receiver<Event>) -> Event {
    loop {
        match next_event(events) {
            Event::Progress { .. } => {}
            other => return other,
        }
    }
}

fn assert_no_event_within(events: &Receiver<Event>, window: Duration) {
    match events.recv_timeout(window) {
        Err(RecvTimeoutError::Timeout) => {}
        Ok(e) => panic!("expected silence, got event: {e:?}"),
        Err(RecvTimeoutError::Disconnected) => panic!("engine thread died"),
    }
}

/// Wait for the next [`Event::Progress`], failing on any other event first.
fn next_progress(events: &Receiver<Event>) -> (u64, Option<u64>) {
    match next_event(events) {
        Event::Progress {
            elapsed_ms,
            track_ms,
        } => (elapsed_ms, track_ms),
        other => panic!("expected Progress, got {other:?}"),
    }
}

/// Whole milliseconds of `frames` at `rate` Hz, rounded to nearest — the
/// engine's own conversion, restated here independently so the test asserts
/// against the specification rather than against the implementation.
fn frames_to_ms(frames: usize, rate: u32) -> u64 {
    let (frames, rate) = (frames as u64, u64::from(rate));
    (frames * 1000 + rate / 2) / rate
}

fn ms_to_frames(ms: u64, rate: u32) -> usize {
    (ms * u64::from(rate) / 1000) as usize
}

/// Estimate the dominant frequency of interleaved stereo audio by counting
/// zero crossings of the left channel. Exact enough on a clean synthesized
/// sweep (no noise, no DC) to identify *where in the sweep* a block came
/// from to within a few milliseconds.
fn estimate_freq(block: &[f32], rate: u32) -> f64 {
    let ch0: Vec<f32> = block.iter().step_by(CHANNELS).copied().collect();
    let crossings = ch0
        .windows(2)
        .filter(|w| (w[0] < 0.0) != (w[1] < 0.0))
        .count();
    #[allow(clippy::cast_precision_loss)] // counts far below 2^52
    let (crossings, frames) = (crossings as f64, ch0.len() as f64);
    crossings * f64::from(rate) / (2.0 * frames)
}

/// Measure, in frames, how far a seek actually landed from `target`.
///
/// The post-seek session plays the track out to its end, so the delivered
/// output ends with exactly the reference decode from wherever the seek
/// landed. Sliding the reference against the tail of the output and finding
/// the offset that matches *bit for bit* therefore reads the true landing
/// point straight out of the audio — no counter, no timing, no tolerance in
/// the measurement itself. The chirp fixture is what makes the match unique:
/// a steady tone would align at every period.
///
/// Returns the signed frame error, or `None` if no offset within `search`
/// explains the delivered audio at all.
fn measure_seek_error(out: &[f32], reference: &[f32], target: usize, search: i64) -> Option<i64> {
    /// Frames compared to pick a candidate; the winner is then verified in
    /// full by the caller.
    const PROBE_FRAMES: usize = 4096;
    let total_frames = reference.len() / CHANNELS;
    for k in -search..=search {
        let landed = i64::try_from(target).ok()? + k;
        let Ok(landed) = usize::try_from(landed) else {
            continue;
        };
        if landed >= total_frames {
            continue;
        }
        let tail = (total_frames - landed) * CHANNELS;
        if tail > out.len() {
            continue;
        }
        let probe = PROBE_FRAMES.min(total_frames - landed) * CHANNELS;
        let from_out = &out[out.len() - tail..][..probe];
        let from_ref = &reference[landed * CHANNELS..][..probe];
        if from_out == from_ref {
            return Some(k);
        }
    }
    None
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
    assert_eq!(next_transport_event(&events), started(&f.a, 0));

    engine.send(Command::Pause).expect("send");
    assert_eq!(next_transport_event(&events), Event::Paused);
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
    assert_eq!(next_transport_event(&events), Event::Resumed);

    engine.send(Command::Next).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.b, 1));
    assert_eq!(next_transport_event(&events), Event::QueueEnded);

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
    assert_eq!(next_transport_event(&events), started(&f.a, 0));

    engine.send(Command::Pause).expect("send");
    assert_eq!(next_transport_event(&events), Event::Paused);
    let frozen = engine.samples_delivered();
    assert!(frozen < f.a_ref.len(), "pause landed after the track ended");
    thread::sleep(Duration::from_millis(50));
    assert_eq!(
        engine.samples_delivered(),
        frozen,
        "delivery advanced while paused"
    );

    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), Event::Resumed);
    assert_eq!(next_transport_event(&events), Event::QueueEnded);
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

    assert_eq!(next_transport_event(&events), started(&f.a, 0));
    match next_transport_event(&events) {
        Event::TrackFailed { path, reason } => {
            assert_eq!(path, f.bad);
            assert!(!reason.is_empty(), "failure reason must say something");
        }
        other => panic!("expected TrackFailed for the bad file, got {other:?}"),
    }
    assert_eq!(next_transport_event(&events), started(&f.b, 2));
    assert_eq!(next_transport_event(&events), Event::QueueEnded);

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
    assert_eq!(next_transport_event(&events), started(&f.a, 0));

    engine.send(Command::Stop).expect("send");
    assert_eq!(next_transport_event(&events), Event::Stopped);
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
    assert_eq!(next_transport_event(&events), started(&f.a, 0));

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
    assert_eq!(next_transport_event(&events), started(&f.a, 0));

    let (done_tx, done_rx) = mpsc::channel();
    thread::spawn(move || {
        drop(engine); // the whole shutdown path: abort session, join workers
        let _ = done_tx.send(());
    });
    done_rx
        .recv_timeout(Duration::from_secs(10))
        .expect("dropping the handle hung instead of shutting the engine down");

    // The engine thread exited: the sink came back and the event channel is
    // closed. Events queued before the shutdown are still deliverable (the
    // channel buffers them), so drain to the end rather than assuming the
    // very next receive is the disconnect.
    let out = collect(output);
    assert!(!out.is_empty(), "some audio was delivered before shutdown");
    loop {
        match events.recv_timeout(Duration::from_secs(5)) {
            Err(RecvTimeoutError::Disconnected) => break,
            Ok(_) => {}
            other => panic!("event channel should close after shutdown, got {other:?}"),
        }
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
    assert_eq!(next_transport_event(&events), Event::QueueEnded);

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
    assert_eq!(next_transport_event(&events), started(&f.a, 0));
    engine.send(Command::Next).expect("send");
    assert_eq!(next_transport_event(&events), Event::QueueEnded);
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

// ---------------------------------------------------------------------------
// Seek and progress
// ---------------------------------------------------------------------------

/// **Seek accuracy, read out of the audio.** Seek 3 s into the 6 s chirp
/// while it is playing, let it play out, and locate the delivered post-seek
/// audio inside the reference decode by exact sample match. The landing
/// point must be the requested frame — which is a claim about *content*
/// (this is the part of the sweep that belongs at 3 s), not about a counter
/// the engine also maintains.
///
/// The frequency check restates the same fact in the terms a listener would:
/// the first audible block after the seek must be sweeping through the
/// ~2.3 kHz region the chirp reaches at 3 s, not the 200 Hz it starts at.
#[test]
fn seek_while_playing_lands_on_the_target_sample() {
    let f = fixtures();
    let target = ms_to_frames(3_000, RATE);
    // Room for the pre-seek head plus the whole post-seek tail.
    let (engine, events, output) =
        spawn_offline(paced_config(), 2 * f.chirp_ref.len()).expect("spawn engine");

    engine
        .send(Command::SetQueue {
            paths: vec![f.chirp.clone()],
        })
        .expect("send");
    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.chirp, 0));

    engine
        .send(Command::Seek { position_ms: 3_000 })
        .expect("send");
    // A seek restarts the current track, so it starts again (engine docs).
    assert_eq!(next_transport_event(&events), started(&f.chirp, 0));
    assert_eq!(next_transport_event(&events), Event::QueueEnded);

    engine.shutdown();
    let out = collect(output);
    let error = measure_seek_error(&out, &f.chirp_ref, target, 512)
        .expect("delivered audio must match the reference somewhere near the target");
    println!("[seek] landed {error} frame(s) from the target ({target})");
    assert_eq!(
        error, 0,
        "seek into a WAV must be sample-exact; landed {error} frames off"
    );

    // The same claim, stated as a listener would hear it.
    let tail = (f.chirp_ref.len() / CHANNELS - target) * CHANNELS;
    let first_block = &out[out.len() - tail..][..4096 * CHANNELS];
    let got = estimate_freq(first_block, RATE);
    // The block spans ~93 ms, over which the sweep advances ~65 Hz, so the
    // mean sits half a block past the seek point.
    let want = chirp_freq_at(3.0 + 4096.0 / f64::from(RATE) / 2.0);
    assert!(
        (got - want).abs() < 20.0,
        "post-seek audio should sweep through ~{want:.0} Hz, measured {got:.0} Hz"
    );
}

/// **Seek while paused**: the position moves, playback does not. Not one
/// sample may reach the sink until the next `Play`, a `Progress` reports the
/// new position immediately (so nothing on screen is stale), and when
/// playback does resume it resumes *at the target* — asserted against an
/// exact boundary, since pause freezes the delivered count.
#[test]
fn seek_while_paused_moves_the_position_without_playing() {
    let f = fixtures();
    let target = ms_to_frames(4_500, RATE);
    let (engine, events, output) =
        spawn_offline(paced_config(), 2 * f.chirp_ref.len()).expect("spawn engine");

    engine
        .send(Command::SetQueue {
            paths: vec![f.chirp.clone()],
        })
        .expect("send");
    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.chirp, 0));
    engine.send(Command::Pause).expect("send");
    assert_eq!(next_transport_event(&events), Event::Paused);
    let frozen = engine.samples_delivered();
    assert!(
        frozen < target * CHANNELS,
        "the pause must land before the seek target for this test to mean anything"
    );

    engine
        .send(Command::Seek { position_ms: 4_500 })
        .expect("send");
    assert_eq!(
        next_event(&events),
        Event::Progress {
            elapsed_ms: 4_500,
            track_ms: Some(CHIRP_MS),
        },
        "a seek is confirmed by an immediate Progress at the new position"
    );
    // Still paused: no TrackStarted, no Resumed, no audio.
    assert_no_event_within(&events, Duration::from_millis(120));
    assert_eq!(
        engine.samples_delivered(),
        frozen,
        "seeking while paused must not deliver audio"
    );

    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), Event::Resumed);
    assert_eq!(next_transport_event(&events), started(&f.chirp, 0));
    assert_eq!(next_transport_event(&events), Event::QueueEnded);

    engine.shutdown();
    let out = collect(output);
    // Delivery was frozen across the whole seek, so `frozen` is the exact
    // boundary between the pre-seek head and the post-seek tail.
    assert_samples_eq(&out[..frozen], &f.chirp_ref[..frozen], "pre-seek head");
    assert_samples_eq(
        &out[frozen..],
        &f.chirp_ref[target * CHANNELS..],
        "post-seek tail (must begin exactly at the target frame)",
    );
}

/// **Seek past the end of the track is Next**: the following queue position
/// starts from its beginning, and past the *last* track the queue ends. No
/// clamping to the final frame — a stalled playhead is not a state anyone
/// asks for (see `Command::Seek`'s docs).
#[test]
fn seek_past_track_end_advances_to_the_next_track() {
    let f = fixtures();
    let capacity = f.a_ref.len() + f.b_ref.len();
    let (engine, events, output) = spawn_offline(paced_config(), capacity).expect("spawn engine");

    engine
        .send(Command::SetQueue {
            paths: vec![f.a.clone(), f.b.clone()],
        })
        .expect("send");
    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.a, 0));

    // Track A is 5 s; 9 s is past its end, so the seek means Next.
    engine
        .send(Command::Seek { position_ms: 9_000 })
        .expect("send");
    assert_eq!(next_transport_event(&events), started(&f.b, 1));
    assert_eq!(next_transport_event(&events), Event::QueueEnded);

    engine.shutdown();
    let out = collect(output);
    assert!(out.len() < capacity, "track A must have been cut short");
    // Track B ran to completion from its *beginning* — not from any offset
    // carried over from the seek.
    assert_samples_eq(
        &out[out.len() - f.b_ref.len()..],
        &f.b_ref,
        "the track after a past-the-end seek, played from its start",
    );

    // Past the end of the *last* track there is nowhere to advance to.
    let (engine, events, _output) =
        spawn_offline(paced_config(), f.b_ref.len()).expect("spawn engine");
    engine
        .send(Command::SetQueue {
            paths: vec![f.b.clone()],
        })
        .expect("send");
    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.b, 0));
    engine
        .send(Command::Seek { position_ms: 9_000 })
        .expect("send");
    assert_eq!(next_transport_event(&events), Event::QueueEnded);
    assert_no_event_within(&events, Duration::from_millis(120));
}

/// **Seek while stopped is a no-op**, like `Next`: there is no current track
/// to seek within, so nothing starts and nothing is reported.
#[test]
fn seek_while_stopped_is_a_no_op() {
    let f = fixtures();
    let (engine, events, _output) =
        spawn_offline(fast_config(), f.b_ref.len()).expect("spawn engine");
    engine
        .send(Command::SetQueue {
            paths: vec![f.b.clone()],
        })
        .expect("send");
    engine
        .send(Command::Seek { position_ms: 500 })
        .expect("send");
    assert_no_event_within(&events, Duration::from_millis(150));
    assert_eq!(
        engine.samples_delivered(),
        0,
        "a stopped seek plays nothing"
    );
}

/// **Progress cadence and immediacy.** One report per quarter-second of
/// delivered audio, one immediately after `TrackStarted`, none while paused,
/// one immediately after `Resumed` — the contract in `Event::Progress`'s
/// docs, asserted end to end over a 5 s track.
#[test]
fn progress_cadence_is_quarter_second_and_immediate_after_transitions() {
    let f = fixtures();
    let (engine, events, _output) =
        spawn_offline(paced_config(), f.a_ref.len()).expect("spawn engine");
    engine
        .send(Command::SetQueue {
            paths: vec![f.a.clone()],
        })
        .expect("send");
    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.a, 0));

    // Immediately after TrackStarted, before any quarter-second has passed.
    let (first, total) = next_progress(&events);
    assert_eq!(total, Some(5_000), "track length comes with every report");
    assert!(
        first < 250,
        "the report following TrackStarted must be immediate, got {first} ms"
    );

    // Cadence: gather the run of reports up to the pause.
    let mut elapsed = vec![first];
    while elapsed.len() < 8 {
        let (ms, total) = next_progress(&events);
        assert_eq!(total, Some(5_000));
        elapsed.push(ms);
    }
    for pair in elapsed.windows(2) {
        let step = pair[1] - pair[0];
        // 250 ms of audio, plus at most one pump chunk of overshoot
        // (2048 frames ≈ 46 ms at 44.1 kHz).
        assert!(
            (250..=300).contains(&step),
            "reports should be ~250 ms of audio apart, got {step} ms in {elapsed:?}"
        );
    }

    engine.send(Command::Pause).expect("send");
    assert_eq!(next_transport_event(&events), Event::Paused);
    // A paused position is not moving, so there is nothing to report.
    assert_no_event_within(&events, Duration::from_millis(200));

    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), Event::Resumed);
    let (resumed_at, _) = next_progress(&events);
    assert!(
        resumed_at >= *elapsed.last().expect("reports collected"),
        "resume must report a position at or past the last one, got {resumed_at} ms"
    );
}

/// **Elapsed time is wall-clock true across a resampled track** — the one
/// place a plausible implementation goes quietly wrong.
///
/// The queue is 1 s at 44.1 kHz (which fixes the session's stream rate)
/// followed by 4 s at 48 kHz, which the ADR-0004 boundary policy resamples
/// down to 44.1 kHz before it reaches the ring. Counting delivered frames
/// against the *file's* 48 kHz would under-report by 8 %: the 4 s track
/// would appear to run 3.675 s, and every position inside it would be short
/// by a growing margin.
///
/// The assertion is exact rather than approximate, because pause freezes
/// delivery: with the sample counter frozen, the `Progress` that follows
/// `Resumed` is computed from precisely that count, so the expected
/// millisecond value can be derived independently and compared for equality.
#[test]
fn elapsed_is_wall_clock_true_across_a_resampled_track() {
    let f = fixtures();
    // The 48 kHz track resamples to exactly 4 s at the 44.1 kHz stream rate.
    let head_samples = HEAD_44K_FRAMES * CHANNELS;
    let capacity = head_samples + TAIL_48K_SECS * RATE as usize * CHANNELS;
    let (engine, events, _output) = spawn_offline(paced_config(), capacity).expect("spawn engine");

    engine
        .send(Command::SetQueue {
            paths: vec![f.head_44k.clone(), f.tail_48k.clone()],
        })
        .expect("send");
    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.head_44k, 0));
    assert_eq!(next_transport_event(&events), started(&f.tail_48k, 1));

    // Let the resampled track run a little way in, then freeze.
    thread::sleep(Duration::from_millis(80));
    engine.send(Command::Pause).expect("send");
    assert_eq!(next_transport_event(&events), Event::Paused);
    let frozen = engine.samples_delivered();
    assert!(
        frozen > head_samples,
        "the pause must land inside the resampled track"
    );
    assert!(
        frozen < capacity,
        "the pause must land before the resampled track ends"
    );

    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), Event::Resumed);
    let (elapsed, total) = next_progress(&events);

    // Track length is a property of the track, not of the stream it is
    // played into: 4 s at 48 kHz is 4 s at any output rate.
    assert_eq!(
        total,
        Some(TAIL_48K_MS),
        "resampling must not change how long the track is"
    );

    // The audio is being consumed at the stream rate, so that is the
    // denominator. Derived here from the frozen count, independently of the
    // engine's own arithmetic.
    let delivered_frames = (frozen - head_samples) / CHANNELS;
    let want = frames_to_ms(delivered_frames, RATE);
    let naive = frames_to_ms(delivered_frames, TAIL_48K_RATE);
    assert_eq!(
        elapsed, want,
        "elapsed must be delivered frames at the stream rate ({RATE} Hz); \
         a sample-count-naive reading at the file's {TAIL_48K_RATE} Hz would say {naive} ms"
    );
    assert_ne!(
        want, naive,
        "the fixture must actually distinguish the two readings"
    );

    engine.shutdown();
}

/// Device output (feature `device-output`): the engine spawns against the
/// default device — or reports the documented `Device` error on headless
/// machines — and shuts down cleanly either way. Never a panic.
///
/// Also the one place the **forced-rate anchor** path is exercised: with a
/// fixed 44.1 kHz output stream, a 48 kHz track is decoded and resampled
/// whole before it is pushed, which is a different branch from the
/// block-by-block streaming an offline session takes. Seeking into it must
/// still report a wall-clock-true position and a rate-independent length.
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
            assert_eq!(next_transport_event(&events), started(&f.b, 0));
            engine.shutdown(); // mid-track: must not hang the device stream
            println!("[device] engine played through the default output device");
        }
        Err(PlaybackError::Device(msg)) => {
            eprintln!("SKIP: no usable output device ({msg})");
            return;
        }
        Err(other) => panic!("unexpected error spawning device engine: {other}"),
    }

    let (engine, events) =
        baz_core::engine::spawn_device(paced_config(), RATE, 8192).expect("respawn device engine");
    engine
        .send(Command::SetQueue {
            paths: vec![f.tail_48k.clone()],
        })
        .expect("send");
    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.tail_48k, 0));
    engine
        .send(Command::Seek { position_ms: 2_500 })
        .expect("send");
    // Reports already in flight when the seek was sent describe the position
    // it came *from*; the first one at or past the target is the seek's own.
    let (elapsed, total) = loop {
        let (elapsed, total) = next_progress(&events);
        assert_eq!(
            total,
            Some(TAIL_48K_MS),
            "a resampled track keeps its own length"
        );
        if elapsed >= 2_500 {
            break (elapsed, total);
        }
    };
    assert!(
        elapsed < 2_600,
        "the seek must land at the target, not past it: {elapsed} ms"
    );
    assert_eq!(next_transport_event(&events), started(&f.tail_48k, 0));
    engine.shutdown();
    println!("[device] seek into a resampled track reported {elapsed} ms of {total:?}");
}
