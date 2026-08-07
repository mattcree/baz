//! The engine service: the running half of the ADR-0003 command/event API.
//!
//! Front ends drive playback by sending [`Command`]s through an
//! [`EngineHandle`] and reacting to the [`Event`]s the engine emits; this
//! module is the authoritative contract for what each message does at
//! runtime. The wire shapes live in [`crate::protocol`]; the audio machinery
//! (decode, gapless splice, resample) lives in [`crate::playback`] and is
//! reused here unchanged.
//!
//! # Spawning
//!
//! [`spawn_offline`] runs the engine against a preallocated in-memory
//! [`OfflineSink`] — the headless configuration every test uses, and the way
//! to render a queue offline. With the non-default `device-output` feature,
//! `spawn_device` (feature `device-output`) plays through the default audio device instead. Both
//! return an [`EngineHandle`] plus the event [`Receiver`].
//!
//! # Threading model
//!
//! - **Engine (control + pump) thread** — spawned by `spawn_*`, owns the
//!   sink. It alternates between processing commands and pumping decoded
//!   audio from the session ring buffer into the sink. Because commands and
//!   pumping share one thread, control is race-free by construction: after
//!   [`Event::Paused`] is emitted, *no* further samples reach the sink until
//!   resume — there is no "one more chunk in flight". The pump iteration
//!   itself keeps the realtime discipline of `playback::engine::consume`:
//!   wait-free ring reads, writes into the preallocated sink, atomic
//!   flag/counter updates — no locks and no allocation on the pump path
//!   (event emission and command receipt happen *between* pump iterations,
//!   and for device output the true realtime thread is the cpal callback
//!   inside the device sink, which never runs any of this module's code).
//! - **Producer thread** — one per playback session (a session is one run
//!   through the queue, started by [`Command::Play`]). It streams the
//!   current track into an `rtrb` SPSC ring and decodes the next track ahead
//!   on a **prefetch thread**, exactly like [`run_playlist`](crate::playback::run_playlist), so track
//!   boundaries stay gapless by construction. Per-track boundary and failure
//!   notices travel to the engine thread over two more SPSC rings, keeping
//!   the pump side lock-free.
//!
//! All cross-thread control flags (`stop`, `producer_done`) are atomics; the
//! pause gate is plain single-threaded state on the engine thread.
//!
//! # Command semantics
//!
//! | Command | While stopped | While playing | While paused |
//! |---|---|---|---|
//! | [`Command::SetQueue`] | replaces queue | stops playback ([`Event::Stopped`]), replaces queue | same |
//! | [`Command::Play`] | starts at the queue top (or emits [`Event::QueueEnded`] if the queue is empty) | no-op | resumes ([`Event::Resumed`]) |
//! | [`Command::Pause`] | no-op | pauses ([`Event::Paused`]) | no-op |
//! | [`Command::Stop`] | no-op | stops ([`Event::Stopped`]); a later `Play` starts from the queue top | same |
//! | [`Command::Next`] | no-op | skips to the next queue position (see below) | skips and *resumes playing* |
//! | [`Command::Seek`] | no-op | jumps within the current track, keeps playing | jumps within the current track, **stays paused** |
//!
//! # Event semantics
//!
//! - [`Event::TrackStarted`] fires when a track's first samples are
//!   delivered to the sink (not when they are decoded — decode-ahead runs
//!   seconds early). A [`Command::Seek`] restarts the current track, so it
//!   fires again for that same track when the post-seek audio reaches the
//!   sink — the statement it makes ("this track's audio is now arriving") is
//!   true both times, and a front end that folds it idempotently sees
//!   nothing unusual.
//! - [`Event::Progress`] reports the position inside the current track at
//!   the cadence its protocol docs pin: one per quarter-second of delivered
//!   audio, plus one immediately after every `TrackStarted`, `Resumed`, and
//!   accepted `Seek`. See "Elapsed time" below for what "position" means
//!   precisely.
//! - [`Event::TrackFailed`] fires when a track cannot be opened or decoded;
//!   the queue continues with the next track. Because failures are found by
//!   decode-ahead, a `TrackFailed` for position *n+1* can arrive while
//!   position *n* is still audible. Per-track events are always emitted in
//!   queue order.
//! - [`Event::QueueEnded`] fires when every queued track has played, failed,
//!   or been skipped. Playback position resets to the queue top.
//! - Events are emitted only for state that changed: redundant commands
//!   (pausing while paused, stopping while stopped) emit nothing.
//!
//! # Pause, stop, and skip — implementation honesty
//!
//! **Pause** gates the pump: the session (ring, producer, decode-ahead)
//! stays intact and for device output the stream stays open, so resume is
//! gapless-instant and the delivered sample stream is bit-identical to an
//! unpaused run. (Device output has up to one device-ring's worth of already
//! -pumped audio that keeps draining after `Paused` — ordinary output
//! latency, ~0.2 s at the size the app uses.) Pause is therefore the one
//! transport command that does *not* call [`Sink::discard_buffered`]:
//! throwing that audio away is exactly what would cost resume its
//! bit-identical continuation, so the short trailing drain is the price and
//! it is knowingly paid.
//!
//! **Stop** and **Next** abort the session: an atomic stop flag releases the
//! producer, its threads are joined, and undelivered ring audio is
//! discarded. They also call [`Sink::discard_buffered`], which drops the
//! audio the sink itself had queued but not yet made audible — for device
//! output, the contents of the device ring. Abandoning the session without
//! that leaves up to a full device ring of the *abandoned* position playing
//! on afterwards, which is precisely how a transport command comes to feel
//! late. **Next is drain-and-restart**: a fresh session starts at the next
//! queue position, meaning a new decode of that track (first audio within
//! milliseconds for local files) rather than a sample-accurate splice out of
//! the running stream. That trade is deliberate for v0.1; the gapless path
//! stays reserved for its one guarantee — *adjacent* tracks playing to
//! completion.
//!
//! **Seek is the same drain-and-restart**, aimed at the *current* queue
//! position instead of the next one, with the new session's first track
//! opened and [`AudioSource::seek`]ed to the target before its first block is
//! pushed. The cost is identical and identically documented: the running
//! session's undelivered ring audio *and* the sink's buffered audio are
//! discarded and the target track is decoded afresh, so first audio at the
//! new position arrives within tens of milliseconds rather than instantly.
//! What the listener does **not** hear in between is the old position: the
//! discard is what keeps the gap a short silence rather than a fifth of a
//! second of audio the user already asked to leave. Two further consequences
//! worth stating plainly:
//!
//! - **Seeking while paused** moves the position and stays paused: the new
//!   session is created in the paused state, so not one sample reaches the
//!   sink until the next [`Command::Play`]. An [`Event::Progress`] is emitted
//!   immediately so the position is never stale on screen.
//! - **Seeking at or past the end of the track** is [`Command::Next`]: the
//!   following queue position starts from its beginning, or the queue ends.
//!   The engine decides this from the track length it already knows; when a
//!   length was never declared, [`AudioSource::seek`] reports the overrun and
//!   the producer skips that track instead.
//!
//! # Elapsed time
//!
//! [`Event::Progress::elapsed_ms`] is `seek target + delivered audio since
//! the current track began`, where "delivered audio" is counted **in output
//! frames at the session's stream rate** — not in the source file's frames.
//!
//! That distinction is the whole correctness argument, and it matters
//! exactly when the two rates differ. A 48 kHz track played into a 44.1 kHz
//! stream is resampled by the ADR-0004 boundary policy before it reaches the
//! ring, so one second of that track occupies 44 100 delivered frames, not
//! 48 000. Dividing delivered frames by the *file's* rate would report a
//! 60-second track as running 55.1 seconds — wrong by 8 %, and wrong in a way
//! that grows over the track. Dividing by the stream rate is wall-clock true
//! by construction, because the stream rate is the rate the audio is
//! actually being consumed at. The producer therefore publishes the
//! negotiated stream rate to the engine thread and the arithmetic uses only
//! that.
//!
//! [`Event::Progress::track_ms`] is the track's own length, computed from
//! its native frame count at its native rate, so it is unaffected by
//! resampling — as it must be: converting a track's sample rate does not
//! change how long it plays for.
//!
//! Two honest caveats:
//!
//! - Position is measured at the **sink**, so with device output it leads
//!   what is audible by up to one device ring (~0.19 s at the size the app
//!   uses), the same ordinary output latency the pause docs above describe.
//!   The lead is a steady-state property of continuous playback only: every
//!   command that abandons a session empties the sink's buffer as part of
//!   abandoning it, so a seek's own [`Event::Progress`] is never reporting
//!   across a bufferful of stale audio.
//! - `track_ms` is `None` for a stream that declares no length. Progress is
//!   still reported; there is simply no total to render against.
//!
//! # Shutdown
//!
//! Dropping the [`EngineHandle`] (or calling [`EngineHandle::shutdown`])
//! closes the command channel; the engine thread aborts any session, joins
//! its workers, drops the sink, and exits. The drop blocks until that
//! completes — bounded by at most one decode block per worker — so no
//! threads outlive the handle.
//!
//! # Event delivery
//!
//! Events arrive on a single `std::sync::mpsc` [`Receiver`] returned at
//! spawn: **one consumer** by design. A front end that needs fan-out (GUI +
//! remote transport at once) must forward from this receiver itself;
//! broadcast delivery is future protocol-layer work. If the receiver is
//! dropped, further events are discarded and the engine keeps running.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use rtrb::RingBuffer;

#[cfg(feature = "device-output")]
use crate::playback::device::DeviceSink;
use crate::playback::engine::push_with_backpressure;
use crate::playback::resample::resample_interleaved;
use crate::playback::source::frames_to_ms;
use crate::playback::{
    AudioSource, BoundaryPolicy, CHANNELS, DecodedAudio, EngineConfig, OfflineSink, PlaybackError,
    Sink,
};
use crate::protocol::{Command, Event};

/// Sleep per engine-loop iteration while paused: long enough to idle
/// cheaply, short enough that resume feels instant.
const PAUSED_POLL: Duration = Duration::from_millis(2);
/// Sleep when the ring is empty but the producer is still working
/// (mirrors `playback::engine::consume`).
const STARVED_POLL: Duration = Duration::from_micros(50);
/// [`Event::Progress`] cadence divisor: one report per `1/PROGRESS_HZ` of
/// *delivered audio*. Deriving the cadence from the sample counter rather
/// than a clock keeps it exactly 4 Hz of playing time (never faster when the
/// pump runs ahead, never slower when it is starved) and keeps the check on
/// the engine loop down to one integer comparison.
const PROGRESS_HZ: u32 = 4;

/// The engine could not accept the command because its thread has shut
/// down (the handle was already consumed by shutdown, or the engine
/// thread terminated).
#[derive(Debug, thiserror::Error)]
#[error("the engine has shut down")]
pub struct EngineClosed;

/// A front end's connection to a running engine: send [`Command`]s, observe
/// progress, shut down.
///
/// Dropping the handle shuts the engine down cleanly (see the module docs).
#[derive(Debug)]
pub struct EngineHandle {
    commands: Option<Sender<Command>>,
    thread: Option<JoinHandle<()>>,
    delivered: Arc<AtomicUsize>,
}

impl EngineHandle {
    /// Send a command to the engine.
    ///
    /// # Errors
    ///
    /// [`EngineClosed`] if the engine thread is no longer running.
    pub fn send(&self, command: Command) -> Result<(), EngineClosed> {
        self.commands
            .as_ref()
            .ok_or(EngineClosed)?
            .send(command)
            .map_err(|_| EngineClosed)
    }

    /// Total interleaved samples delivered to the sink since spawn,
    /// monotonically increasing across tracks and sessions. Divide by
    /// [`CHANNELS`] for frames. While paused this value does not advance —
    /// the tests use exactly that as the pause guarantee.
    #[must_use]
    pub fn samples_delivered(&self) -> usize {
        self.delivered.load(Ordering::Acquire)
    }

    /// Shut the engine down and wait for its threads to finish. Equivalent
    /// to dropping the handle; provided so intent reads explicitly.
    pub fn shutdown(self) {
        drop(self);
    }
}

impl Drop for EngineHandle {
    fn drop(&mut self) {
        // Closing the command channel is the shutdown signal; the engine
        // thread observes the disconnect, aborts any session, and exits.
        self.commands = None;
        if let Some(handle) = self.thread.take() {
            // A panicked engine thread is a bug (docs/ENGINEERING.md); all
            // drop can do is not propagate it into the caller's unwind.
            let _ = handle.join();
        }
    }
}

/// The collected output of an engine spawned with [`spawn_offline`].
#[derive(Debug)]
pub struct OfflineOutput {
    output: Receiver<Vec<f32>>,
}

impl OfflineOutput {
    /// Wait for the engine to shut down and return every interleaved stereo
    /// sample it delivered to the sink, in order.
    ///
    /// This blocks until the engine thread exits, so shut the engine down
    /// first (drop the [`EngineHandle`]) or call this from another thread.
    /// Returns `None` only if the engine thread died without reporting —
    /// i.e. it panicked, which is a bug.
    #[must_use]
    pub fn wait(self) -> Option<Vec<f32>> {
        self.output.recv().ok()
    }
}

/// Spawn a headless engine delivering into an [`OfflineSink`] with room for
/// `capacity_samples` interleaved samples (the sink never grows; overflow is
/// dropped and counted, per its contract).
///
/// Returns the control handle, the event receiver (single consumer — see
/// the module docs), and the [`OfflineOutput`] that yields the delivered
/// samples after shutdown.
///
/// # Errors
///
/// [`PlaybackError::BitPerfectReopenUnimplemented`] if `cfg` selects
/// [`BoundaryPolicy::BitPerfectReopen`] (not yet implemented — same contract
/// as [`run_playlist`](crate::playback::run_playlist)); [`PlaybackError::Io`] if the engine thread cannot
/// be spawned.
pub fn spawn_offline(
    cfg: EngineConfig,
    capacity_samples: usize,
) -> Result<(EngineHandle, Receiver<Event>, OfflineOutput), PlaybackError> {
    ensure_supported(cfg)?;
    let (cmd_tx, cmd_rx) = mpsc::channel();
    let (event_tx, event_rx) = mpsc::channel();
    let (out_tx, out_rx) = mpsc::channel();
    let delivered = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&delivered);
    let thread = thread::Builder::new()
        .name("baz-engine".into())
        .spawn(move || {
            let control = Control::new(
                cmd_rx,
                event_tx,
                cfg,
                None,
                counter,
                OfflineSink::with_capacity(capacity_samples),
            );
            let sink = control.run();
            let _ = out_tx.send(sink.into_samples());
        })?;
    Ok((
        EngineHandle {
            commands: Some(cmd_tx),
            thread: Some(thread),
            delivered,
        },
        event_rx,
        OfflineOutput { output: out_rx },
    ))
}

/// Spawn an engine playing through the default audio device (shared mode)
/// at `sample_rate`, with a device ring of `device_ring_frames` frames.
///
/// The device stream is opened once and stays open for the life of the
/// engine — pause does not tear it down. Every session is delivered at
/// `sample_rate`: tracks at other rates are resampled on the prefetch side
/// (ADR-0004 default policy), including the first track of a session.
///
/// # Errors
///
/// [`PlaybackError::Device`] if no output device is usable;
/// [`PlaybackError::BitPerfectReopenUnimplemented`] if `cfg` selects
/// [`BoundaryPolicy::BitPerfectReopen`]; [`PlaybackError::Io`] if the
/// engine thread cannot be spawned.
#[cfg(feature = "device-output")]
pub fn spawn_device(
    cfg: EngineConfig,
    sample_rate: u32,
    device_ring_frames: usize,
) -> Result<(EngineHandle, Receiver<Event>), PlaybackError> {
    ensure_supported(cfg)?;
    let (cmd_tx, cmd_rx) = mpsc::channel();
    let (event_tx, event_rx) = mpsc::channel();
    let (ack_tx, ack_rx) = mpsc::channel();
    let delivered = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&delivered);
    let thread = thread::Builder::new()
        .name("baz-engine".into())
        .spawn(move || {
            // cpal streams are not Send, so the sink must be created (and
            // dropped) on the engine thread; the open result is reported
            // back through a one-shot channel.
            match DeviceSink::open(sample_rate, device_ring_frames) {
                Ok(sink) => {
                    let _ = ack_tx.send(Ok(()));
                    let control =
                        Control::new(cmd_rx, event_tx, cfg, Some(sample_rate), counter, sink);
                    drop(control.run()); // closes the device stream
                }
                Err(e) => {
                    let _ = ack_tx.send(Err(e));
                }
            }
        })?;
    let handle = EngineHandle {
        commands: Some(cmd_tx),
        thread: Some(thread),
        delivered,
    };
    match ack_rx.recv() {
        Ok(Ok(())) => Ok((handle, event_rx)),
        Ok(Err(e)) => Err(e), // dropping `handle` joins the (finished) thread
        Err(_) => Err(PlaybackError::Device(
            "engine thread terminated while opening the device".into(),
        )),
    }
}

/// The engine service implements [`BoundaryPolicy::ResampleToStreamRate`]
/// only, matching [`run_playlist`](crate::playback::run_playlist)'s contract for the reopen mode.
fn ensure_supported(cfg: EngineConfig) -> Result<(), PlaybackError> {
    if cfg.boundary == BoundaryPolicy::BitPerfectReopen {
        return Err(PlaybackError::BitPerfectReopenUnimplemented);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Engine (control + pump) thread
// ---------------------------------------------------------------------------

struct Control<S: Sink> {
    commands: Receiver<Command>,
    events: Sender<Event>,
    cfg: EngineConfig,
    /// Force every session to this stream rate (device output); `None`
    /// negotiates per session from the first playable track, like
    /// [`run_playlist`](crate::playback::run_playlist).
    forced_rate: Option<u32>,
    delivered: Arc<AtomicUsize>,
    queue: Vec<PathBuf>,
    /// Queue index where the next idle-state `Play` starts.
    position: usize,
    paused: bool,
    session: Option<Session>,
    sink: S,
}

impl<S: Sink> Control<S> {
    fn new(
        commands: Receiver<Command>,
        events: Sender<Event>,
        cfg: EngineConfig,
        forced_rate: Option<u32>,
        delivered: Arc<AtomicUsize>,
        sink: S,
    ) -> Self {
        Self {
            commands,
            events,
            cfg,
            forced_rate,
            delivered,
            queue: Vec::new(),
            position: 0,
            paused: false,
            session: None,
            sink,
        }
    }

    /// The engine loop. Returns the sink at shutdown so spawners can hand
    /// its contents back (offline) or drop it in place (device).
    fn run(mut self) -> S {
        loop {
            if self.session.is_some() {
                // Active session: stay responsive to commands between pump
                // iterations without ever blocking on the channel.
                match self.commands.try_recv() {
                    Ok(cmd) => {
                        self.handle(cmd);
                        continue; // drain all pending commands first
                    }
                    Err(TryRecvError::Empty) => {}
                    Err(TryRecvError::Disconnected) => break,
                }
                self.tick();
            } else {
                // Idle: block until a command arrives or the handle drops.
                match self.commands.recv() {
                    Ok(cmd) => self.handle(cmd),
                    Err(_) => break,
                }
            }
        }
        // Shutdown: dropping the session sets its stop flag and joins the
        // producer (and its prefetch) — bounded, no leaked threads.
        self.session = None;
        self.sink
    }

    /// One pump-and-report iteration of an active session.
    fn tick(&mut self) {
        if self.paused {
            // The gate: no pulls, so the sink sees nothing until resume and
            // the ring (plus producer backpressure) preserves every sample.
            thread::sleep(PAUSED_POLL);
            return;
        }
        let Some(session) = self.session.as_mut() else {
            return;
        };
        let chunk_samples = self.cfg.consumer_chunk_frames * CHANNELS;
        let pumped = session.pump(&mut self.sink, chunk_samples, &self.delivered);
        session.report(&self.events, false);
        if session.complete() {
            session.report(&self.events, true);
            self.session = None; // joins the (already finished) producer
            let _ = self.events.send(Event::QueueEnded);
            self.position = 0;
            return;
        }
        // Between pump iterations, never inside one: `pump` above is the
        // realtime-disciplined path and stays a ring read plus a sink write.
        if session.progress_due() {
            self.emit_progress();
        }
        if pumped {
            if !self.cfg.consumer_pace.is_zero() {
                thread::sleep(self.cfg.consumer_pace);
            }
        } else {
            thread::sleep(STARVED_POLL);
        }
    }

    fn handle(&mut self, command: Command) {
        match command {
            Command::SetQueue { paths } => {
                self.stop_session();
                self.queue = paths;
                self.position = 0;
            }
            Command::Play => {
                if self.session.is_some() {
                    if self.paused {
                        self.paused = false;
                        let _ = self.events.send(Event::Resumed);
                        // Resumed is always followed by a fresh reading, so
                        // a front end that dropped the position while paused
                        // has it back before the first frame is drawn.
                        self.emit_progress();
                    }
                } else {
                    self.start_session(self.position, 0, None);
                }
            }
            Command::Pause => {
                if self.session.is_some() && !self.paused {
                    self.paused = true;
                    let _ = self.events.send(Event::Paused);
                }
            }
            Command::Stop => {
                self.stop_session();
                self.position = 0;
            }
            Command::Next => {
                if let Some(session) = self.session.take() {
                    let next = session.current + 1;
                    drop(session); // abort: stop flag + join
                    // The skipped track's audio is gone from the session ring;
                    // drop the copy already sitting in the sink too.
                    self.sink.discard_buffered();
                    self.paused = false;
                    self.start_session(next, 0, None);
                }
            }
            Command::Seek { position_ms } => self.seek(position_ms),
        }
    }

    /// [`Command::Seek`]: drain-and-restart the *current* track at
    /// `position_ms` (module docs). Same machinery as [`Command::Next`],
    /// aimed one queue position earlier and with a start offset.
    fn seek(&mut self, position_ms: u64) {
        let Some(session) = self.session.take() else {
            return; // stopped: there is no current track to seek within
        };
        let current = session.current;
        let track_ms = session.track_ms;
        let was_paused = self.paused;
        drop(session); // abort: stop flag + join, exactly as Next does
        // Aborting the session discards the audio the engine still held, but
        // the sink may hold a further bufferful of the position being left
        // behind. Dropping the session without dropping that is precisely the
        // "seek feels late" bug: see `Sink::discard_buffered`.
        self.sink.discard_buffered();
        if track_ms.is_some_and(|total| position_ms >= total) {
            // At or past the end is Next, per Command::Seek's contract.
            self.paused = false;
            self.start_session(current + 1, 0, None);
            return;
        }
        // The length carries over: it belongs to the track, not the session,
        // so the immediate Progress below can report a total straight away
        // instead of leaving the front end with a blank right-hand timestamp
        // until the new session's first bound arrives.
        self.start_session(current, position_ms, track_ms);
        if self.session.is_some() {
            // Seeking never changes whether audio is flowing.
            self.paused = was_paused;
            self.emit_progress();
        }
    }

    /// Send one [`Event::Progress`] now and re-arm the cadence, so an
    /// immediate report never doubles up with a scheduled one.
    fn emit_progress(&mut self) {
        let Some(session) = self.session.as_mut() else {
            return;
        };
        session.arm_progress();
        if let Some(event) = session.progress() {
            let _ = self.events.send(event);
        }
    }

    /// Abort any active session and emit [`Event::Stopped`] if one existed.
    ///
    /// Stopping means stopping: the sink's buffered audio goes with the
    /// session, so silence follows the command rather than trailing it by a
    /// bufferful. (Pause takes a different path for exactly that reason —
    /// it keeps its buffer on purpose.)
    fn stop_session(&mut self) {
        if let Some(session) = self.session.take() {
            drop(session);
            self.sink.discard_buffered();
            let _ = self.events.send(Event::Stopped);
        }
        self.paused = false;
    }

    /// Start a session at queue index `start`, beginning `seek_ms` into that
    /// track (0 for an ordinary start) and carrying `track_ms` as the known
    /// length of it (`None` until the producer reports one). Past the end of
    /// the queue (or on an empty queue) the run is already over:
    /// [`Event::QueueEnded`].
    fn start_session(&mut self, start: usize, seek_ms: u64, track_ms: Option<u64>) {
        self.paused = false;
        if start >= self.queue.len() {
            self.position = 0;
            let _ = self.events.send(Event::QueueEnded);
            return;
        }
        self.session = Some(Session::start(
            self.queue.clone().into(),
            start,
            seek_ms,
            track_ms,
            self.forced_rate,
            self.cfg,
        ));
    }
}

// ---------------------------------------------------------------------------
// A playback session: producer thread + rings + progress reporting
// ---------------------------------------------------------------------------

/// Flags shared between the engine thread and a session's producer side.
/// Atomics only — both sides stay lock-free.
#[derive(Default)]
struct SessionShared {
    /// Engine → producer: abandon the run (stop, skip, shutdown).
    stop: AtomicBool,
    /// Producer → engine: every track has been pushed or failed.
    producer_done: AtomicBool,
    /// Producer → engine: the rate this session's audio is delivered at,
    /// published once the first playable track has been opened (0 until
    /// then). Every elapsed-time calculation divides by this and nothing
    /// else — see "Elapsed time" in the module docs for why the source
    /// file's own rate would be the wrong denominator.
    stream_rate: AtomicU32,
}

/// A track's position and length within a session, as the producer reports
/// it to the engine thread over the bounds ring.
#[derive(Clone, Copy, Debug)]
struct TrackBound {
    /// Queue index of the track.
    index: usize,
    /// Session-relative interleaved sample offset where its audio begins.
    start_sample: usize,
    /// Its playing time, when known. The streamed anchor track reports the
    /// length its container declares (it has not finished decoding yet);
    /// decode-ahead tracks report the length they actually decoded to, which
    /// is the same number for a well-formed file and strictly more truthful
    /// for one whose header lies or is missing.
    duration_ms: Option<u64>,
}

/// One run through the queue from a starting position. Owned by the engine
/// thread; the producer half runs on its own thread and communicates only
/// through SPSC rings and the shared atomics.
struct Session {
    audio: rtrb::Consumer<f32>,
    bounds: rtrb::Consumer<TrackBound>,
    fails: rtrb::Consumer<(usize, String)>,
    shared: Arc<SessionShared>,
    producer: Option<JoinHandle<()>>,
    queue: Arc<[PathBuf]>,
    /// Interleaved samples delivered to the sink so far this session.
    pulled: usize,
    /// Start sample of each queue index's audio, once known.
    boundaries: Vec<Option<usize>>,
    /// Declared length of each queue index's track, once known.
    durations: Vec<Option<u64>>,
    /// Failure reason per queue index, once known (taken when reported).
    failures: Vec<Option<String>>,
    /// Reporting cursor: per-track events are emitted strictly in queue
    /// order, so a decode-ahead discovery never outruns the track before it.
    next_report: usize,
    /// Last queue index reported as started — what [`Command::Next`] skips
    /// from and what [`Command::Seek`] seeks within.
    current: usize,
    /// Where in the delivered stream the current track's audio begins; the
    /// origin every elapsed time is measured from.
    track_origin: usize,
    /// The current track's playing time, when known.
    track_ms: Option<u64>,
    /// Milliseconds into the *first* track of this session that its audio
    /// starts at — the [`Command::Seek`] target that created the session, 0
    /// otherwise. Applies to that track only; later tracks start at 0.
    seek_ms: u64,
    /// Queue index `seek_ms` applies to.
    seek_index: usize,
    /// `pulled` value at which the next cadence [`Event::Progress`] is due.
    next_progress: usize,
}

impl Session {
    fn start(
        queue: Arc<[PathBuf]>,
        start: usize,
        seek_ms: u64,
        track_ms: Option<u64>,
        forced_rate: Option<u32>,
        cfg: EngineConfig,
    ) -> Self {
        let (ring_tx, ring_rx) = RingBuffer::new(cfg.ring_frames * CHANNELS);
        let remaining = (queue.len() - start).max(1);
        let (bounds_tx, bounds_rx) = RingBuffer::new(remaining);
        let (fails_tx, fails_rx) = RingBuffer::new(remaining);
        let shared = Arc::new(SessionShared::default());
        let task = ProducerTask {
            queue: Arc::clone(&queue),
            start,
            seek_ms,
            forced_rate,
            ring: ring_tx,
            bounds: bounds_tx,
            fails: fails_tx,
            shared: Arc::clone(&shared),
        };
        let producer = thread::spawn(move || task.run());
        let len = queue.len();
        Self {
            audio: ring_rx,
            bounds: bounds_rx,
            fails: fails_rx,
            shared,
            producer: Some(producer),
            queue,
            pulled: 0,
            boundaries: vec![None; len],
            durations: vec![None; len],
            failures: vec![None; len],
            next_report: start,
            current: start,
            // The session's first track always begins at delivered sample 0,
            // whichever queue index turns out to be playable.
            track_origin: 0,
            track_ms,
            seek_ms,
            seek_index: start,
            // Nothing to report until a track actually starts (or a seek or
            // resume asks for a reading), so the cadence starts disarmed.
            next_progress: usize::MAX,
        }
    }

    /// The current position reading, or `None` when audio has been delivered
    /// but the producer has not yet published the rate to interpret it at —
    /// a window of microseconds at the very start of a session, during which
    /// there is nothing truthful to say.
    ///
    /// The arithmetic is the module docs' "Elapsed time" contract, in
    /// integers: delivered frames since this track's origin, converted at
    /// the **stream** rate, offset by the seek target that started it.
    fn progress(&self) -> Option<Event> {
        let frames = (self.pulled.saturating_sub(self.track_origin) / CHANNELS) as u64;
        let rate = self.shared.stream_rate.load(Ordering::Acquire);
        let delivered_ms = match (frames, rate) {
            // Nothing delivered yet — the position is the seek target, and
            // no rate is needed to say so. This is the reading a Seek's
            // immediate Progress carries, before the new session's producer
            // has so much as opened the file.
            (0, _) => 0,
            (_, 0) => return None,
            (frames, rate) => frames_to_ms(frames, rate),
        };
        let offset = if self.current == self.seek_index {
            self.seek_ms
        } else {
            0
        };
        let elapsed = offset.saturating_add(delivered_ms);
        Some(Event::Progress {
            // Never report past the end: the last pump before a boundary can
            // carry a few frames of the next track's audio into this track's
            // count, and "3:01 of 3:00" is a bug on screen.
            elapsed_ms: self.track_ms.map_or(elapsed, |total| elapsed.min(total)),
            track_ms: self.track_ms,
        })
    }

    /// Whether the cadence has come due, arming the next one. One integer
    /// comparison in the common case.
    fn progress_due(&mut self) -> bool {
        if self.pulled < self.next_progress {
            return false;
        }
        self.arm_progress();
        true
    }

    /// Schedule the next cadence report a quarter-second of delivered audio
    /// from now. Called by [`Self::progress_due`] and by every immediate
    /// report, so the two can never emit back-to-back.
    fn arm_progress(&mut self) {
        let rate = self.shared.stream_rate.load(Ordering::Acquire);
        let step = (rate / PROGRESS_HZ) as usize * CHANNELS;
        // Before the rate is known, retry on the next iteration rather than
        // arming a zero-length (and so always-due) interval.
        self.next_progress = self.pulled + step.max(1);
    }

    /// Pull up to `chunk_samples` from the ring into the sink. This is the
    /// pump path: wait-free ring read, preallocated-sink write, atomic
    /// counter — no locks, no allocation (see module docs).
    fn pump(&mut self, sink: &mut dyn Sink, chunk_samples: usize, delivered: &AtomicUsize) -> bool {
        let available = self.audio.slots();
        if available == 0 {
            return false;
        }
        let n = available.min(chunk_samples);
        let Ok(chunk) = self.audio.read_chunk(n) else {
            return false;
        };
        let (a, b) = chunk.as_slices();
        sink.write(a);
        if !b.is_empty() {
            sink.write(b);
        }
        chunk.commit_all();
        self.pulled += n;
        delivered.fetch_add(n, Ordering::Release);
        true
    }

    /// Emit per-track events in strict queue order. A track is reported
    /// started once its first samples were delivered (`pulled` passed its
    /// boundary); failures are reported as soon as order allows. With
    /// `flush` (session complete) every remaining known track is reported.
    fn report(&mut self, events: &Sender<Event>, flush: bool) {
        while let Ok(bound) = self.bounds.pop() {
            if let Some(slot) = self.boundaries.get_mut(bound.index) {
                *slot = Some(bound.start_sample);
            }
            if let Some(slot) = self.durations.get_mut(bound.index) {
                *slot = bound.duration_ms;
            }
        }
        while let Ok((i, reason)) = self.fails.pop() {
            if let Some(slot) = self.failures.get_mut(i) {
                *slot = Some(reason);
            }
        }
        while self.next_report < self.queue.len() {
            let i = self.next_report;
            if let Some(start_sample) = self.boundaries[i] {
                if self.pulled > start_sample || flush {
                    let _ = events.send(Event::TrackStarted {
                        path: self.queue[i].clone(),
                        position: i,
                    });
                    self.current = i;
                    self.track_origin = start_sample;
                    self.track_ms = self.durations[i];
                    // A new track means a new position: make the cadence due
                    // now so `Progress` follows `TrackStarted` immediately
                    // (protocol docs) instead of up to 250 ms later.
                    self.next_progress = self.pulled;
                    if let Some(reason) = self.failures[i].take() {
                        // Opened, then failed mid-decode: started AND failed.
                        let _ = events.send(Event::TrackFailed {
                            path: self.queue[i].clone(),
                            reason,
                        });
                    }
                    self.next_report += 1;
                    continue;
                }
                break; // its audio hasn't reached the sink yet
            }
            if let Some(reason) = self.failures[i].take() {
                let _ = events.send(Event::TrackFailed {
                    path: self.queue[i].clone(),
                    reason,
                });
                self.next_report += 1;
                continue;
            }
            break; // the producer hasn't reached this track yet
        }
    }

    /// Whether the session has played out: the producer pushed everything
    /// and the ring is drained. (Checked only while unpaused, so a paused
    /// session never completes underneath the user.)
    fn complete(&self) -> bool {
        // Order matters: read `producer_done` before the ring so a push
        // between the two loads can only delay completion, never lose audio.
        let done = self.shared.producer_done.load(Ordering::Acquire);
        done && self.audio.slots() == 0
    }
}

impl Drop for Session {
    /// Abort: release the producer (it observes `stop` in every
    /// backpressure and decode loop) and join it, prefetch included. Ring
    /// audio not yet delivered is discarded. Natural completion takes the
    /// same path — the flag is simply set after the producer already
    /// finished.
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Release);
        if let Some(handle) = self.producer.take() {
            let _ = handle.join();
        }
    }
}

// ---------------------------------------------------------------------------
// Producer side (per session)
// ---------------------------------------------------------------------------

struct ProducerTask {
    queue: Arc<[PathBuf]>,
    start: usize,
    /// Milliseconds into the session's first playable track to begin at
    /// ([`Command::Seek`]'s target); 0 for an ordinary start.
    seek_ms: u64,
    forced_rate: Option<u32>,
    ring: rtrb::Producer<f32>,
    bounds: rtrb::Producer<TrackBound>,
    fails: rtrb::Producer<(usize, String)>,
    shared: Arc<SessionShared>,
}

type Prefetch = (usize, JoinHandle<Result<DecodedAudio, PlaybackError>>);

impl ProducerTask {
    fn run(mut self) {
        self.produce();
        self.shared.producer_done.store(true, Ordering::Release);
    }

    /// Stream the queue from `start` into the ring: the first playable
    /// track streams block-by-block (fast start), later tracks are decoded
    /// one ahead on a prefetch thread and spliced through the ring —
    /// gapless exactly as in [`run_playlist`](crate::playback::run_playlist). A track that fails to open
    /// or decode is recorded and skipped; the queue survives it.
    fn produce(&mut self) {
        let stop = Arc::clone(&self.shared);
        let stop = &stop.stop;

        let Some((idx, mut src)) = self.find_anchor(stop) else {
            return; // nothing playable (or stopping)
        };
        let stream_rate = self.forced_rate.unwrap_or_else(|| src.sample_rate());
        // Publish the rate before any bound: the engine thread's Acquire on
        // the bounds ring synchronizes with this Release, so a bound is
        // never visible without the rate that gives it meaning.
        self.shared
            .stream_rate
            .store(stream_rate, Ordering::Release);
        // The track's own length, unaffected by any resampling below.
        let anchor_ms = src.duration_ms();
        let mut pushed = 0usize;
        let mut pending: Option<Prefetch> = self.spawn_prefetch(idx + 1);

        // The anchor track. Same-rate: stream block-by-block for fast
        // start. Different rate (forced-rate/device mode): decode fully and
        // resample first — the ADR-0004 policy applied to track one. A seek
        // has already positioned the source either way; on the resampling
        // path the remaining tail must still be longer than the resampler's
        // alignment padding (a few milliseconds), or it is reported as a
        // track failure like any other decode problem.
        if src.sample_rate() == stream_rate {
            let _ = self.bounds.push(TrackBound {
                index: idx,
                start_sample: pushed,
                duration_ms: anchor_ms,
            });
            loop {
                if stop.load(Ordering::Acquire) {
                    break;
                }
                match src.next_block() {
                    Ok(Some(block)) => {
                        if !push_with_backpressure(&mut self.ring, block, stop) {
                            break;
                        }
                        pushed += block.len();
                    }
                    Ok(None) => break,
                    Err(e) => {
                        let _ = self.fails.push((idx, e.to_string()));
                        break;
                    }
                }
            }
        } else {
            match decode_open(src, stop).and_then(|d| at_rate(d, stream_rate)) {
                Ok(samples) => {
                    if !stop.load(Ordering::Acquire) {
                        let _ = self.bounds.push(TrackBound {
                            index: idx,
                            start_sample: pushed,
                            duration_ms: anchor_ms,
                        });
                        if push_with_backpressure(&mut self.ring, &samples, stop) {
                            pushed += samples.len();
                        }
                    }
                }
                Err(e) => {
                    let _ = self.fails.push((idx, e.to_string()));
                }
            }
        }

        // Subsequent tracks, one decode ahead.
        let mut i = idx + 1;
        while i < self.queue.len() && !stop.load(Ordering::Acquire) {
            let decoded = match pending.take() {
                Some((_, handle)) => handle
                    .join()
                    .unwrap_or(Err(PlaybackError::WorkerPanicked("prefetch"))),
                None => decode_all(&self.queue[i], stop),
            };
            pending = self.spawn_prefetch(i + 1);
            if stop.load(Ordering::Acquire) {
                break;
            }
            match decoded.and_then(|d| {
                // The decoded length at the *native* rate is the track's
                // playing time; take it before the samples are converted to
                // the stream rate, which changes their count but not the
                // seconds they represent.
                let duration_ms = Some(frames_to_ms(d.frames() as u64, d.sample_rate));
                at_rate(d, stream_rate).map(|samples| (samples, duration_ms))
            }) {
                Ok((samples, duration_ms)) => {
                    let _ = self.bounds.push(TrackBound {
                        index: i,
                        start_sample: pushed,
                        duration_ms,
                    });
                    if !push_with_backpressure(&mut self.ring, &samples, stop) {
                        break;
                    }
                    pushed += samples.len();
                }
                Err(e) => {
                    let _ = self.fails.push((i, e.to_string()));
                }
            }
            i += 1;
        }

        if let Some((_, handle)) = pending {
            // The prefetch loop observes `stop`, so this join is bounded.
            let _ = handle.join();
        }
    }

    /// Find the session's first playable track and open it, positioned at
    /// [`Self::seek_ms`]. Tracks that cannot be opened are reported as
    /// failures and skipped; a track the seek target lies past is skipped
    /// *silently*, and the search continues from the beginning of the next
    /// one — that is [`Command::Seek`]'s "past the end means Next" contract,
    /// reached here only when the engine could not apply it itself for want
    /// of a declared track length.
    ///
    /// Returns the queue index and the positioned source, or `None` if the
    /// queue ran out (or the session is stopping).
    fn find_anchor(&mut self, stop: &AtomicBool) -> Option<(usize, AudioSource)> {
        let mut idx = self.start;
        let mut seek_ms = self.seek_ms;
        while idx < self.queue.len() && !stop.load(Ordering::Acquire) {
            let opened = AudioSource::open(&self.queue[idx]).and_then(|mut src| {
                if seek_ms > 0 {
                    src.seek(seek_ms)?;
                }
                Ok(src)
            });
            match opened {
                Ok(src) => return Some((idx, src)),
                Err(PlaybackError::SeekPastEnd { .. }) => seek_ms = 0,
                Err(e) => {
                    let _ = self.fails.push((idx, e.to_string()));
                }
            }
            idx += 1;
        }
        None
    }

    fn spawn_prefetch(&self, index: usize) -> Option<Prefetch> {
        let path = self.queue.get(index)?.clone();
        let shared = Arc::clone(&self.shared);
        let handle = thread::spawn(move || decode_all(&path, &shared.stop));
        Some((index, handle))
    }
}

/// Decode a whole file, checking `stop` between blocks so an aborting
/// session never waits for a full-track decode. On stop the partial result
/// is returned; callers observe the flag and discard it.
fn decode_all(path: &Path, stop: &AtomicBool) -> Result<DecodedAudio, PlaybackError> {
    decode_open(AudioSource::open(path)?, stop)
}

/// [`decode_all`] from an already-open source.
fn decode_open(mut src: AudioSource, stop: &AtomicBool) -> Result<DecodedAudio, PlaybackError> {
    let mut samples = Vec::new();
    while let Some(block) = src.next_block()? {
        samples.extend_from_slice(block);
        if stop.load(Ordering::Acquire) {
            break;
        }
    }
    Ok(DecodedAudio {
        samples,
        sample_rate: src.sample_rate(),
    })
}

/// Bring decoded audio to the session stream rate (ADR-0004 default
/// policy; no-op at equal rates).
fn at_rate(decoded: DecodedAudio, stream_rate: u32) -> Result<Vec<f32>, PlaybackError> {
    if decoded.sample_rate == stream_rate {
        Ok(decoded.samples)
    } else {
        resample_interleaved(&decoded.samples, decoded.sample_rate, stream_rate)
    }
}

#[cfg(test)]
mod tests {
    //! Where the engine must drop the sink's buffered audio.
    //!
    //! # Why these tests live here and not in `tests/engine.rs`
    //!
    //! The integration suite drives the engine through [`spawn_offline`],
    //! whose sink is an [`OfflineSink`] — and an offline sink has no
    //! downstream buffer to discard from. It is the record of delivered
    //! audio, not a queue standing in front of a device, so the bug these
    //! tests guard (*pre-seek audio still queued in the device ring keeps
    //! playing after the seek*) is structurally unobservable through that
    //! path. Saying so plainly and testing the two halves where each is real
    //! beats inventing an offline assertion that would pass either way:
    //!
    //! - **Does the engine ask for the discard, at exactly the right
    //!   moments?** That is a property of [`Control`], and it is what these
    //!   tests assert, by running the real control loop against a sink that
    //!   records the operations it receives. The recording sink is a test
    //!   double, but it does not stand in for the behaviour under test — the
    //!   behaviour under test is the engine's *call*, observed directly.
    //! - **Does the discard actually empty the device ring?** That is a
    //!   property of `DeviceSink`, asserted against a real audio device in
    //!   `tests/playback.rs` (`discard_buffered_empties_the_device_ring`,
    //!   feature `device-output`).

    use std::sync::Mutex;
    use std::time::Instant;

    use super::{
        Arc, AtomicUsize, BoundaryPolicy, Command, Control, Duration, EngineConfig, Event, Path,
        PathBuf, Sink, mpsc, thread,
    };

    const RATE: u32 = 44_100;
    /// Long enough that every command below lands mid-track.
    const TRACK_SECS: usize = 5;
    const TIMEOUT: Duration = Duration::from_secs(20);

    /// What a sink was asked to do, in order.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Op {
        Write,
        Discard,
    }

    /// A sink that records the operations the engine performs on it.
    struct RecordingSink {
        ops: Arc<Mutex<Vec<Op>>>,
    }

    impl Sink for RecordingSink {
        fn write(&mut self, samples: &[f32]) {
            if samples.is_empty() {
                return;
            }
            if let Ok(mut ops) = self.ops.lock() {
                // Collapse runs of writes: the pump writes constantly, and
                // only their ordering against `Discard` is under test.
                if ops.last() != Some(&Op::Write) {
                    ops.push(Op::Write);
                }
            }
        }

        fn discard_buffered(&mut self) {
            if let Ok(mut ops) = self.ops.lock() {
                ops.push(Op::Discard);
            }
        }
    }

    /// A five-second 440 Hz stereo WAV.
    fn fixture(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: RATE,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut writer = hound::WavWriter::create(&path, spec).expect("create fixture wav");
        for n in 0..TRACK_SECS * RATE as usize {
            #[allow(clippy::cast_precision_loss)] // frame indices are far below 2^52
            let t = n as f64 / f64::from(RATE);
            #[allow(clippy::cast_possible_truncation)] // f64 sine -> f32 sample
            let s = (0.5 * (2.0 * std::f64::consts::PI * 440.0 * t).sin()) as f32;
            writer.write_sample(s).expect("write sample");
            writer.write_sample(s).expect("write sample");
        }
        writer.finalize().expect("finalize fixture wav");
        path
    }

    /// Paced so a 5 s track takes a few hundred ms to drain: every command
    /// below then lands mid-track, with audio genuinely in flight.
    fn config() -> EngineConfig {
        EngineConfig {
            ring_frames: 8192,
            consumer_chunk_frames: 2048,
            consumer_pace: Duration::from_millis(4),
            boundary: BoundaryPolicy::ResampleToStreamRate,
        }
    }

    /// A running [`Control`] on its own thread, plus what is needed to drive
    /// and observe it.
    struct Harness {
        commands: Option<mpsc::Sender<Command>>,
        events: mpsc::Receiver<Event>,
        ops: Arc<Mutex<Vec<Op>>>,
        thread: Option<thread::JoinHandle<()>>,
        _dir: tempfile::TempDir,
        track: PathBuf,
    }

    impl Harness {
        fn start() -> Self {
            let dir = tempfile::tempdir().expect("temp dir");
            let track = fixture(dir.path(), "tone_5s.wav");
            let (cmd_tx, cmd_rx) = mpsc::channel();
            let (event_tx, event_rx) = mpsc::channel();
            let ops: Arc<Mutex<Vec<Op>>> = Arc::default();
            let sink = RecordingSink {
                ops: Arc::clone(&ops),
            };
            let thread = thread::spawn(move || {
                let control = Control::new(
                    cmd_rx,
                    event_tx,
                    config(),
                    None,
                    Arc::new(AtomicUsize::new(0)),
                    sink,
                );
                drop(control.run());
            });
            Self {
                commands: Some(cmd_tx),
                events: event_rx,
                ops,
                thread: Some(thread),
                _dir: dir,
                track,
            }
        }

        fn send(&self, command: Command) {
            self.commands
                .as_ref()
                .expect("engine running")
                .send(command)
                .expect("engine accepts commands");
        }

        /// Start playback and block until audio is genuinely flowing into the
        /// sink, so a later command has something buffered to invalidate.
        fn play_until_audio_flows(&self) {
            self.send(Command::SetQueue {
                paths: vec![self.track.clone()],
            });
            self.send(Command::Play);
            loop {
                match self.events.recv_timeout(TIMEOUT) {
                    Ok(Event::TrackStarted { .. }) => break,
                    Ok(_) => {}
                    Err(e) => panic!("no TrackStarted: {e}"),
                }
            }
            let deadline = Instant::now() + TIMEOUT;
            while Instant::now() < deadline {
                if self.ops_since(0).contains(&Op::Write) {
                    return;
                }
                thread::sleep(Duration::from_millis(1));
            }
            panic!("the engine never wrote audio to the sink");
        }

        fn ops_since(&self, mark: usize) -> Vec<Op> {
            self.ops.lock().expect("ops lock")[mark..].to_vec()
        }

        /// Where the ops log stands now, so a command's effects are read
        /// separately from the previous one's.
        fn mark(&self) -> usize {
            self.ops.lock().expect("ops lock").len()
        }

        fn shutdown(mut self) {
            self.commands = None;
            if let Some(handle) = self.thread.take() {
                handle.join().expect("engine thread");
            }
        }
    }

    impl Drop for Harness {
        fn drop(&mut self) {
            self.commands = None;
            if let Some(handle) = self.thread.take() {
                let _ = handle.join();
            }
        }
    }

    /// Poll the ops log until `done` accepts it (or time runs out) and return
    /// what it ended up as, so the assertion can be made on the caller's
    /// terms rather than on the timeout's.
    fn wait_until(harness: &Harness, mark: usize, done: impl Fn(&[Op]) -> bool) -> Vec<Op> {
        let deadline = Instant::now() + TIMEOUT;
        loop {
            let ops = harness.ops_since(mark);
            if done(&ops) || Instant::now() >= deadline {
                return ops;
            }
            thread::sleep(Duration::from_millis(1));
        }
    }

    fn discarded(ops: &[Op]) -> bool {
        ops.contains(&Op::Discard)
    }

    /// The bug: a seek abandoned the session but left the audio already
    /// handed to the sink queued in front of the new position. The engine
    /// must tell the sink to drop it, between the last pre-seek write and the
    /// first post-seek one.
    #[test]
    fn seek_discards_the_sinks_buffered_audio() {
        let harness = Harness::start();
        harness.play_until_audio_flows();
        let mark = harness.mark();
        harness.send(Command::Seek { position_ms: 3_000 });
        let ops = wait_until(&harness, mark, |ops| {
            ops.iter()
                .position(|op| *op == Op::Discard)
                .is_some_and(|i| ops[i + 1..].contains(&Op::Write))
        });
        let discard = ops
            .iter()
            .position(|op| *op == Op::Discard)
            .expect("Seek must discard the sink's buffered audio");
        assert!(
            ops[discard + 1..].contains(&Op::Write),
            "post-seek audio must reach the sink only after the discard: {ops:?}"
        );
        harness.shutdown();
    }

    /// Skipping a track abandons its audio the same way a seek does.
    #[test]
    fn next_discards_the_sinks_buffered_audio() {
        let harness = Harness::start();
        harness.play_until_audio_flows();
        let mark = harness.mark();
        harness.send(Command::Next);
        assert!(
            discarded(&wait_until(&harness, mark, discarded)),
            "Next must discard the sink's buffered audio"
        );
        harness.shutdown();
    }

    /// Stop means stop: silence follows the command instead of trailing it by
    /// a bufferful.
    #[test]
    fn stop_discards_the_sinks_buffered_audio() {
        let harness = Harness::start();
        harness.play_until_audio_flows();
        let mark = harness.mark();
        harness.send(Command::Stop);
        assert!(
            discarded(&wait_until(&harness, mark, discarded)),
            "Stop must discard the sink's buffered audio"
        );
        harness.shutdown();
    }

    /// Replacing the queue while playing stops playback (module docs), and so
    /// abandons the buffered audio with it.
    #[test]
    fn queue_replacement_discards_the_sinks_buffered_audio() {
        let harness = Harness::start();
        harness.play_until_audio_flows();
        let mark = harness.mark();
        harness.send(Command::SetQueue { paths: Vec::new() });
        assert!(
            discarded(&wait_until(&harness, mark, discarded)),
            "SetQueue while playing must discard the sink's buffered audio"
        );
        harness.shutdown();
    }

    /// Pause is the deliberate exception: it keeps its buffered audio, which
    /// is what makes resume gapless-instant and the delivered stream
    /// bit-identical to an unpaused run. Discarding here would break a
    /// documented guarantee, not improve one.
    #[test]
    fn pause_keeps_the_sinks_buffered_audio() {
        let harness = Harness::start();
        harness.play_until_audio_flows();
        let mark = harness.mark();
        harness.send(Command::Pause);
        loop {
            match harness.events.recv_timeout(TIMEOUT) {
                Ok(Event::Paused) => break,
                Ok(_) => {}
                Err(e) => panic!("no Paused event: {e}"),
            }
        }
        // Give the engine every chance to misbehave before concluding it did
        // not: many pump iterations' worth of idle time.
        thread::sleep(Duration::from_millis(50));
        assert!(
            !discarded(&harness.ops_since(mark)),
            "Pause must not discard buffered audio — resume would no longer be \
             sample-continuous"
        );
        harness.shutdown();
    }
}
