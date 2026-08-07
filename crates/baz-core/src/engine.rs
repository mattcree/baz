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
//!
//! # Event semantics
//!
//! - [`Event::TrackStarted`] fires when a track's first samples are
//!   delivered to the sink (not when they are decoded — decode-ahead runs
//!   seconds early).
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
//! latency, ~0.2 s at the default ring size.)
//!
//! **Stop** and **Next** abort the session: an atomic stop flag releases the
//! producer, its threads are joined, and undelivered ring audio is
//! discarded. **Next is drain-and-restart**: a fresh session starts at the
//! next queue position, meaning a new decode of that track (first audio
//! within milliseconds for local files) rather than a sample-accurate splice
//! out of the running stream. That trade is deliberate for v0.1; the gapless
//! path stays reserved for its one guarantee — *adjacent* tracks playing to
//! completion.
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
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use rtrb::RingBuffer;

#[cfg(feature = "device-output")]
use crate::playback::device::DeviceSink;
use crate::playback::engine::push_with_backpressure;
use crate::playback::resample::resample_interleaved;
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
                    }
                } else {
                    self.start_session(self.position);
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
                    self.paused = false;
                    self.start_session(next);
                }
            }
        }
    }

    /// Abort any active session and emit [`Event::Stopped`] if one existed.
    fn stop_session(&mut self) {
        if let Some(session) = self.session.take() {
            drop(session);
            let _ = self.events.send(Event::Stopped);
        }
        self.paused = false;
    }

    /// Start a session at queue index `start`; past the end of the queue
    /// (or on an empty queue) the run is already over: [`Event::QueueEnded`].
    fn start_session(&mut self, start: usize) {
        self.paused = false;
        if start >= self.queue.len() {
            self.position = 0;
            let _ = self.events.send(Event::QueueEnded);
            return;
        }
        self.session = Some(Session::start(
            self.queue.clone().into(),
            start,
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
}

/// One run through the queue from a starting position. Owned by the engine
/// thread; the producer half runs on its own thread and communicates only
/// through SPSC rings and the shared atomics.
struct Session {
    audio: rtrb::Consumer<f32>,
    bounds: rtrb::Consumer<(usize, usize)>,
    fails: rtrb::Consumer<(usize, String)>,
    shared: Arc<SessionShared>,
    producer: Option<JoinHandle<()>>,
    queue: Arc<[PathBuf]>,
    /// Interleaved samples delivered to the sink so far this session.
    pulled: usize,
    /// Start sample of each queue index's audio, once known.
    boundaries: Vec<Option<usize>>,
    /// Failure reason per queue index, once known (taken when reported).
    failures: Vec<Option<String>>,
    /// Reporting cursor: per-track events are emitted strictly in queue
    /// order, so a decode-ahead discovery never outruns the track before it.
    next_report: usize,
    /// Last queue index reported as started — what [`Command::Next`] skips
    /// from.
    current: usize,
}

impl Session {
    fn start(
        queue: Arc<[PathBuf]>,
        start: usize,
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
            failures: vec![None; len],
            next_report: start,
            current: start,
        }
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
        while let Ok((i, start_sample)) = self.bounds.pop() {
            if let Some(slot) = self.boundaries.get_mut(i) {
                *slot = Some(start_sample);
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
    forced_rate: Option<u32>,
    ring: rtrb::Producer<f32>,
    bounds: rtrb::Producer<(usize, usize)>,
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

        // Anchor: the first track that opens. Failures before it are
        // recorded and skipped.
        let mut idx = self.start;
        let mut anchor = None;
        while idx < self.queue.len() && !stop.load(Ordering::Acquire) {
            match AudioSource::open(&self.queue[idx]) {
                Ok(src) => {
                    anchor = Some(src);
                    break;
                }
                Err(e) => {
                    let _ = self.fails.push((idx, e.to_string()));
                    idx += 1;
                }
            }
        }
        let Some(mut src) = anchor else {
            return; // nothing playable (or stopping)
        };
        let stream_rate = self.forced_rate.unwrap_or_else(|| src.sample_rate());
        let mut pushed = 0usize;
        let mut pending: Option<Prefetch> = self.spawn_prefetch(idx + 1);

        // The anchor track. Same-rate: stream block-by-block for fast
        // start. Different rate (forced-rate/device mode): decode fully and
        // resample first — the ADR-0004 policy applied to track one.
        if src.sample_rate() == stream_rate {
            let _ = self.bounds.push((idx, pushed));
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
                        let _ = self.bounds.push((idx, pushed));
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
            match decoded.and_then(|d| at_rate(d, stream_rate)) {
                Ok(samples) => {
                    let _ = self.bounds.push((i, pushed));
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
