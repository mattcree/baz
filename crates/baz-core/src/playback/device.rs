//! Real audio-device output via cpal (shared mode), behind the non-default
//! `device-output` feature.
//!
//! [`DeviceSink`] adapts the engine's push-side [`Sink`] to cpal's pull-model
//! callback with a second `rtrb` ring:
//!
//! ```text
//! engine consumer loop --Sink::write--> device ring --wait-free pop--> cpal callback
//! ```
//!
//! The realtime path here is the **cpal callback**, and it upholds the
//! sacred-thread rules (`docs/ENGINEERING.md`): wait-free pops from the ring,
//! zero-fill on underrun, an atomic flag for stream errors — no allocation,
//! no locks, no I/O, no panics. `DeviceSink::write` runs on the engine's
//! consumer thread, which for device output is a pump feeding real hardware,
//! not the realtime thread — so it may sleep for backpressure; that sleep is
//! the device-output analog of [`EngineConfig::consumer_pace`].
//!
//! # Discarding buffered audio (the seek-latency mechanism)
//!
//! The ring is the last place a user's seek can be defeated: the engine can
//! abandon its session instantly, but audio already handed to this sink is
//! still queued in front of the new position. At the ring size the app uses
//! (8192 frames) that is up to ~186 ms of the *old* position playing on after
//! the click — well past the ~100 ms at which a person stops experiencing a
//! control as immediate. So [`Sink::discard_buffered`] must actually empty
//! the ring, and it must do so without either side breaking the realtime
//! rules.
//!
//! Only the consumer end of an `rtrb` ring may advance the read index, and
//! that end lives inside the callback. The two sides therefore coordinate
//! through a **monotone watermark**, not a request/acknowledge handshake:
//!
//! - `DeviceSink` counts every sample it has ever committed to the ring
//!   (`written`, plain non-atomic state — the engine thread is its only
//!   writer). A discard publishes that running total into the shared
//!   `discard_before` atomic and returns. That is the whole producer side:
//!   **one release store, no waiting.**
//! - The callback counts every sample it has ever taken out of the ring. When
//!   it sees `discard_before` ahead of its own count it advances the read
//!   index over the difference — one `read_chunk` + `commit_all`, O(1) and
//!   allocation-free regardless of how much is being dropped — and then fills
//!   the output block normally, which yields silence until the engine's new
//!   session pushes its first post-seek block.
//!
//! Because the watermark is a count of samples the producer had *already*
//! committed when the discard was requested, and the ring is FIFO, the
//! callback drops exactly the pre-discard samples however late it observes
//! the store. A stale read of `discard_before` can only *delay* the drop by
//! one callback period; it can never consume audio pushed after the discard.
//! That is what makes the no-handshake design safe.
//!
//! **If the callback never runs again** (device stalled, stream dead, host
//! wedged) the store is simply never observed. Nothing blocks: the engine
//! made a fire-and-forget request and moved on, so a seek cannot hang on an
//! acknowledgement that will never arrive. The stale samples stay in the ring
//! — inaudible by definition, since nothing is draining it — and if the
//! device does resume, the still-pending watermark is honoured on the very
//! next callback and the correct audio is what is heard.
//! [`DeviceSink::discard_pending`] exposes that state rather than hiding it;
//! [`DeviceSink::failed`] reports a stream error the host did tell us about.
//!
//! # Sizing the ring
//!
//! The ring must comfortably exceed the largest block cpal's
//! `BufferSize::Default` ever asks for in one callback, or the callback
//! cannot be satisfied even once and the stream underruns continuously.
//! Measured on an ordinary Fedora/PipeWire desktop at 44.1 kHz, that block is
//! **1881–1882 frames in steady state (~43 ms) with a single 4410-frame
//! (100 ms) priming call at stream start**. A 20 s continuous playthrough
//! recorded, via [`DeviceSink::underrun_samples`], zero steady-state
//! underruns at 8192 and 4096 frames — idle and with every core saturated —
//! and a hard cliff below that: 1024 frames produced 16.4 s of silence inside
//! a 20 s track, 512 frames 46–52 s.
//!
//! 8192 frames (~186 ms) is therefore kept as the app's default. It is 4.35
//! steady-state callbacks of headroom and 1.86x the priming request; 4096 is
//! *smaller than a single priming call* on this very machine, so its clean
//! steady-state result does not make it a safe default for hosts whose period
//! is larger still. Since the discard above removes latency from seek, skip,
//! stop, and queue replacement outright, shrinking the ring would no longer
//! buy responsiveness where it was the complaint — only in pause-to-silence
//! and the progress readout's lead, both of which are deliberate and
//! documented in [`crate::engine`]. Trading measured underrun margin for that
//! is not a good trade.
//!
//! # Following the source rate (ADR-0009)
//!
//! The stream is opened at whatever rate the engine asks for and **reopened**
//! when it asks for a different one, which is how baz plays a 48 kHz album at
//! 48 kHz and a 44.1 kHz album at 44.1 kHz with no conversion of its own.
//! [`Sink::negotiate_rate`] is that request; it consults
//! `supported_output_configs` rather than guessing, and when the device cannot
//! do the asked-for rate it answers with the nearest rate it *can* do, so the
//! engine knows to report the chain as not bit-perfect instead of quietly
//! converting.
//!
//! Reopening tears the old stream down, so the engine drains it first
//! ([`Sink::drain_buffered`]) at a rate boundary and discards it outright when
//! a transport command abandoned the audio anyway. A reopen therefore subsumes
//! [`Sink::discard_buffered`]: nothing survives it.
//!
//! **Sample format.** Every stream is opened as f32. f32's 24-bit
//! mantissa holds every 24-bit integer PCM value exactly, so a 24-bit master
//! reaches the device untruncated and undithered; the format is a lossless
//! carrier for the depths this decoder produces, not a compromise.
//!
//! **What "no conversion" does and does not claim.** This is shared-mode
//! output. The guarantee here is that *baz* performs no sample-rate
//! conversion: the samples handed to the host are the decoder's, at the file's
//! rate, in a lossless format. What the system mixer then does with them —
//! `PipeWire` or `CoreAudio` may still resample to a graph rate it is holding for
//! another client — is outside this backend's control and is not claimed.
//! Removing that last hop is what exclusive-mode backends (ALSA `hw:`, WASAPI
//! exclusive, `CoreAudio` hog) are for, and they remain a later phase; ADR-0009
//! states the boundary of the claim in the same terms.
//!
//! # Why cpal is first touched from a thread that never exits
//!
//! Every call into cpal made anywhere in baz goes through one private
//! `default_output_device` funnel in this module, and the first one blocks until a
//! dedicated, permanently parked thread — `baz-cpal-anchor` — has made that
//! call *first*. That is not a nicety; without it, baz corrupts its own
//! process on Windows.
//!
//! cpal's WASAPI backend keeps a **process-global** `IMMDeviceEnumerator` in a
//! `static ENUMERATOR: OnceLock<Enumerator>` (`cpal-0.16.0`,
//! `src/host/wasapi/device.rs`) and hands it to every thread through
//! `unsafe impl Send + Sync`. COM initialisation, however, is **thread-local**:
//! `src/host/wasapi/com.rs` puts each calling thread into an apartment with
//! `CoInitializeEx(COINIT_APARTMENTTHREADED)` from a `thread_local!` whose
//! destructor calls `CoUninitialize()` when that thread exits. So the global
//! enumerator is created inside the apartment of whichever thread happened to
//! touch cpal first, and when *that* thread exits its apartment is torn down
//! underneath the still-published static. The next thread to ask for a device
//! gets the stale pointer back out of the `OnceLock` and calls through a vtable
//! that no longer belongs to anything: `STATUS_ACCESS_VIOLATION`.
//!
//! baz walks straight into this, because the sink is deliberately opened on the
//! engine thread ([`crate::engine::spawn_device_with`] — cpal streams are not
//! `Send`) and the engine thread exits at shutdown. One engine per process is
//! fine; a *second* [`crate::engine::spawn_device`] in the same process — an
//! output-mode change, a retry after a device error, a front end that stops and
//! restarts playback, or the test suite doing any of those — is a use of freed
//! COM state. It is also how this was found: the `device-output` integration
//! tests spawn one engine per test, and the Windows CI job died with
//! `0xc0000005` the moment a third device test started after an earlier engine
//! thread had exited.
//!
//! The anchor thread is the fix that is available from outside cpal: it makes
//! the first cpal call of the process, so the global enumerator is built in
//! *its* apartment, and then it parks forever. A thread that never exits never
//! runs its thread-local destructors, so `CoUninitialize()` is never called for
//! the apartment that owns cpal's global state and the static stays valid for
//! as long as the process does. (Not calling `CoUninitialize` at all is the
//! documented advice for exactly this situation; the apartment is reclaimed by
//! process teardown either way.)
//!
//! It is deliberately **not** `#[cfg(windows)]`. Backends are where this class
//! of bug hides precisely because the other platforms never exercise the
//! ordering, and a Linux or macOS test run that does not open the code path
//! Windows depends on is not evidence about it. One parked thread and one
//! device enumeration, once per process, is the whole cost.
//!
//! [`EngineConfig::consumer_pace`]: super::EngineConfig
//! [`Sink::negotiate_rate`]: super::Sink::negotiate_rate
//! [`Sink::drain_buffered`]: super::Sink::drain_buffered
//! [`Sink::discard_buffered`]: super::Sink::discard_buffered

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock, mpsc};
use std::thread;
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rtrb::{Producer, RingBuffer};

use super::sink::Sink;
use super::{CHANNELS, PlaybackError};

/// How long [`Sink::drain_buffered`] will wait for the callback to empty the
/// ring before giving up. Generously past the ~186 ms the app's ring holds, so
/// a healthy device always finishes; short enough that a stalled one cannot
/// wedge the engine (the module docs' rule for anything that waits on a
/// callback).
const DRAIN_BUDGET: Duration = Duration::from_millis(2_000);
/// Poll interval while draining. One millisecond is a twentieth of a callback
/// period on this host: fine-grained relative to what is being waited for, and
/// ~200 wake-ups for a full ring.
const DRAIN_POLL: Duration = Duration::from_millis(1);

/// The one sample format the engine emits, and the format every stream is
/// opened with.
///
/// f32 is not a compromise for high-resolution sources: its 24-bit mantissa
/// represents every value of a 24-bit integer PCM sample exactly, so a
/// 24-bit master reaches the device without truncation or dither. (16- and
/// 20-bit sources are exact for the same reason.) Requesting it explicitly —
/// rather than accepting whatever the device lists first — is what keeps
/// [`negotiated_rate`] from selecting a rate that is only available in a
/// narrower integer format.
const SAMPLE_FORMAT: cpal::SampleFormat = cpal::SampleFormat::F32;

/// Which thread made the process's first cpal call, set once that call has
/// returned. [`OnceLock::get_or_init`] blocks every other caller until it has,
/// so "the anchor goes first" is enforced rather than hoped for.
///
/// `Some` is the `baz-cpal-anchor` thread, which by construction never exits;
/// `None` is the degraded inline fallback taken when no thread could be
/// spawned at all. Recording it is what lets the invariant be *asserted* (see
/// this module's tests) rather than merely intended.
static HOST_ANCHORED: OnceLock<Option<thread::ThreadId>> = OnceLock::new();

/// The default output device, and the **only** door into cpal in this crate.
///
/// Anchors the host on the first call (see the module docs' "Why cpal is first
/// touched from a thread that never exits") and is a plain lookup on every call
/// after that. Everything that needs a device goes through here so that no
/// future path can reintroduce a first-touch on a thread that will exit.
fn default_output_device() -> Option<cpal::Device> {
    anchor_cpal_host();
    cpal::default_host().default_output_device()
}

/// The names of the shared-mode output devices cpal can currently see.
///
/// Names are sorted and deduplicated for a stable settings list. cpal does
/// not expose a portable endpoint id; where a host reports duplicate names,
/// selecting that name resolves to the first matching endpoint.
///
/// # Errors
///
/// [`PlaybackError::Device`] when the platform audio host cannot enumerate
/// its output endpoints.
pub fn shared_output_devices() -> Result<Vec<String>, PlaybackError> {
    anchor_cpal_host();
    let devices = cpal::default_host().output_devices().map_err(|error| {
        PlaybackError::Device(format!("could not list output devices: {error}"))
    })?;
    let mut names = devices
        .filter_map(|device| device.name().ok())
        .collect::<Vec<_>>();
    names.sort_unstable_by_key(|name| name.to_lowercase());
    names.dedup();
    Ok(names)
}

/// Resolve either the system default or an exact name from
/// [`shared_output_devices`].
fn output_device(name: Option<&str>) -> Result<cpal::Device, PlaybackError> {
    let Some(name) = name else {
        return default_output_device()
            .ok_or_else(|| PlaybackError::Device("no default output device".into()));
    };
    anchor_cpal_host();
    let devices = cpal::default_host().output_devices().map_err(|error| {
        PlaybackError::Device(format!("could not list output devices: {error}"))
    })?;
    for device in devices {
        if device.name().ok().as_deref() == Some(name) {
            return Ok(device);
        }
    }
    Err(PlaybackError::Device(format!(
        "no shared output device named {name:?}"
    )))
}

/// Make the process's first cpal call on a thread that will never exit, and
/// wait for it to finish.
///
/// Idempotent and cheap after the first call — an already-initialised
/// [`OnceLock`] read. The wait matters only the first time, and it is bounded
/// by one device enumeration.
///
/// If the thread cannot be spawned at all, the touch is made inline instead:
/// that is exactly the behaviour this function exists to replace, but it is
/// still better than refusing to open a device because a thread was
/// unavailable, and a process that cannot spawn a thread has a larger problem
/// than its audio backend. If cpal itself
/// panics during the touch the anchor thread dies with it and the readiness
/// channel closes; the caller stops waiting and goes on to make its own call,
/// which will fail or panic in the ordinary way rather than hanging here.
fn anchor_cpal_host() {
    HOST_ANCHORED.get_or_init(|| {
        let (ready_tx, ready_rx) = mpsc::channel::<thread::ThreadId>();
        let spawned = thread::Builder::new()
            .name("baz-cpal-anchor".into())
            .spawn(move || {
                touch_cpal_host();
                let _ = ready_tx.send(thread::current().id());
                // Never return: a thread that returns runs its thread-local
                // destructors, and one of cpal's is `CoUninitialize()`.
                loop {
                    thread::park();
                }
            });
        // The handle is dropped rather than kept: the anchor is detached on
        // purpose and there is nothing to join it for — joining it is the one
        // thing that must never happen.
        let Ok(_anchor) = spawned else {
            touch_cpal_host();
            return None;
        };
        // A closed channel means the anchor died during the touch — cpal
        // panicked — so there is nothing to wait for and nothing anchored.
        ready_rx.recv().ok()
    });
}

/// The smallest call that forces a cpal host backend to build whatever
/// process-global state it keeps: enumerate, then let the device go.
///
/// Nothing is held afterwards — no stream, no PCM, no exclusive claim — so
/// this cannot take a device away from anyone. On Windows it is what creates
/// the global `IMMDeviceEnumerator`, which is the whole point; on ALSA and
/// `CoreAudio` it is a cheap lookup that keeps the anchoring path identical
/// across platforms, and therefore testable on the ones baz is developed on.
fn touch_cpal_host() {
    drop(cpal::default_host().default_output_device());
}

/// The rate the default output device will actually run at if asked for
/// `desired` Hz, according to its own advertised capabilities.
///
/// Returns `desired` when the device supports it (the bit-perfect case), the
/// **nearest** supported rate when it does not, and `None` when the device
/// advertises nothing usable — in which case the caller should simply try
/// `desired` and let the open succeed or fail, because a device that will not
/// describe itself is still allowed to work.
///
/// Only stereo f32 configurations are considered: those are the only ones the
/// engine can feed (see [`SAMPLE_FORMAT`] and [`CHANNELS`]), so a rate offered
/// solely in some other format is not a rate we can actually use.
fn negotiated_rate(device: &cpal::Device, desired: u32) -> Option<u32> {
    let channels = u16::try_from(CHANNELS).ok()?;
    let configs = device.supported_output_configs().ok()?;
    let mut best: Option<u32> = None;
    for range in configs {
        if range.channels() != channels || range.sample_format() != SAMPLE_FORMAT {
            continue;
        }
        let min = range.min_sample_rate().0;
        let max = range.max_sample_rate().0;
        // The rate this range can get closest to. Ranges are inclusive
        // intervals, so clamping is the whole search.
        let candidate = desired.clamp(min, max);
        if candidate == desired {
            return Some(desired);
        }
        let better = best.is_none_or(|b| desired.abs_diff(candidate) < desired.abs_diff(b));
        if better {
            best = Some(candidate);
        }
    }
    best
}

/// A [`Sink`] that plays samples on the default output device.
///
/// Dropping the sink stops the stream; samples still buffered in the device
/// ring at drop are discarded. To drop them *without* closing the stream —
/// what a seek, skip, or stop needs — use [`Sink::discard_buffered`], whose
/// lock-free mechanism the module docs describe in full.
pub struct DeviceSink {
    /// Keeps the stream alive; playback stops when this is dropped.
    ///
    /// **Declared first on purpose.** Fields drop in declaration order, and
    /// this is the field whose destructor stops the thing that is still
    /// reading the ring: every cpal backend baz builds against joins its
    /// callback thread inside `Stream::drop` (WASAPI signals `Terminate` and
    /// `join()`s; ALSA sets `dropping`, wakes the poll and `join()`s), so once
    /// this field is gone the callback provably cannot run again. Releasing
    /// the producer end of the ring first would not be *unsound* — `rtrb`
    /// keeps the allocation alive from either end — but it would leave a live
    /// callback reading state its counterpart had already let go of, which is
    /// not a thing this file should have to argue about.
    _stream: cpal::Stream,
    producer: Producer<f32>,
    failed: Arc<AtomicBool>,
    /// Engine → callback: discard ring content until the callback's own
    /// take-count reaches this running total. Monotonically increasing.
    discard_before: Arc<AtomicU64>,
    /// Callback → engine: samples taken out of the ring so far, whether
    /// played or discarded. Published once per callback.
    consumed: Arc<AtomicU64>,
    /// Callback → engine: samples zero-filled because the ring was empty.
    underruns: Arc<AtomicU64>,
    /// Samples committed to the ring so far. Engine-thread-only state: it is
    /// read and written solely by [`Sink::write`] and
    /// [`Sink::discard_buffered`], which the engine calls from one thread.
    written: u64,
    /// Ring capacity in interleaved samples.
    capacity: usize,
    /// The rate this stream is open at — what [`Sink::negotiate_rate`]
    /// compares a request against, and what a reopen has to rebuild from.
    sample_rate: u32,
    /// Ring size in frames, kept so a reopen reproduces the same buffering.
    ring_frames: usize,
    /// `None` follows the system default; `Some` keeps reopening the named
    /// endpoint when the source rate changes.
    device_name: Option<String>,
}

/// Which half of an exact stream-open attempt failed.
///
/// Kept typed until [`DeviceSink::open_on`] has decided whether the one
/// recoverable case deserves a second configuration. Flattening this into
/// [`PlaybackError`] at the `build_output_stream` call would leave no reliable
/// way to distinguish an unsupported tuple from an unplugged device or a
/// backend fault.
enum OpenAttemptError {
    Build(cpal::BuildStreamError),
    Play(cpal::PlayStreamError),
}

impl OpenAttemptError {
    fn action(&self) -> &'static str {
        match self {
            Self::Build(_) => "open",
            Self::Play(_) => "start",
        }
    }

    fn unsupported_configuration(&self) -> bool {
        matches!(
            self,
            Self::Build(cpal::BuildStreamError::StreamConfigNotSupported)
        )
    }
}

impl std::fmt::Display for OpenAttemptError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Build(error) => error.fmt(formatter),
            Self::Play(error) => error.fmt(formatter),
        }
    }
}

impl DeviceSink {
    /// Open the default output device at `sample_rate` (stereo) with a
    /// device ring of `ring_frames` frames.
    ///
    /// # Errors
    ///
    /// [`PlaybackError::Device`] if there is no output device or the stream
    /// cannot be built/started (e.g. headless CI, unsupported rate).
    pub fn open(sample_rate: u32, ring_frames: usize) -> Result<Self, PlaybackError> {
        Self::open_on(None, sample_rate, ring_frames)
    }

    /// Open a named shared-mode output, or the system default when `device_name`
    /// is `None`.
    ///
    /// # Errors
    ///
    /// [`PlaybackError::Device`] when the endpoint is absent, cannot describe
    /// a usable stereo `f32` configuration, or refuses to build/start it.
    pub fn open_on(
        device_name: Option<&str>,
        sample_rate: u32,
        ring_frames: usize,
    ) -> Result<Self, PlaybackError> {
        let device = output_device(device_name)?;
        let channels = u16::try_from(CHANNELS)
            .map_err(|_| PlaybackError::Device("channel count exceeds u16".into()))?;
        let device_label = device
            .name()
            .unwrap_or_else(|_| "unknown output".to_owned());

        // Try the request before believing the capability list. CoreAudio can
        // accept 44.1 kHz while advertising only its current 48 kHz nominal
        // rate; pre-emptively clamping there broke source-following and the
        // macOS device test. WASAPI produces the typed unsupported-config
        // error for the inverse case from the Windows report, so only then do
        // we retry at the nearest advertised rate.
        match Self::open_exact(&device, device_name, sample_rate, ring_frames, channels) {
            Ok(sink) => Ok(sink),
            Err(first) if first.unsupported_configuration() => {
                let fallback = negotiated_rate(&device, sample_rate)
                    .filter(|fallback| *fallback != sample_rate);
                let Some(fallback) = fallback else {
                    return Err(PlaybackError::Device(format!(
                        "{device_label}: could not open stereo f32 at {sample_rate} Hz: {first}"
                    )));
                };
                Self::open_exact(&device, device_name, fallback, ring_frames, channels).map_err(
                    |second| {
                        PlaybackError::Device(format!(
                            "{device_label}: {sample_rate} Hz was unsupported ({first}); \
                             {fallback} Hz fallback failed while trying to {}: {second}",
                            second.action(),
                        ))
                    },
                )
            }
            Err(error) => Err(PlaybackError::Device(format!(
                "{device_label}: could not {} stereo f32 at {sample_rate} Hz: {error}",
                error.action(),
            ))),
        }
    }

    /// Make one literal cpal stream attempt. No capability inference belongs
    /// here: the caller needs the typed failure before deciding whether a
    /// different rate is justified.
    fn open_exact(
        device: &cpal::Device,
        device_name: Option<&str>,
        sample_rate: u32,
        ring_frames: usize,
        channels: u16,
    ) -> Result<Self, OpenAttemptError> {
        let config = cpal::StreamConfig {
            channels,
            sample_rate: cpal::SampleRate(sample_rate),
            buffer_size: cpal::BufferSize::Default,
        };
        let capacity = ring_frames * CHANNELS;
        let (producer, mut consumer) = RingBuffer::<f32>::new(capacity);
        let failed = Arc::new(AtomicBool::new(false));
        let error_flag = Arc::clone(&failed);
        let discard_before = Arc::new(AtomicU64::new(0));
        let taken_total = Arc::new(AtomicU64::new(0));
        let underruns = Arc::new(AtomicU64::new(0));
        let discard_watermark = Arc::clone(&discard_before);
        let taken_counter = Arc::clone(&taken_total);
        let underrun_counter = Arc::clone(&underruns);
        // Callback-owned counters: the callback is their only writer, so it
        // keeps them locally and publishes with a plain store — no read-modify
        // -write on the realtime path.
        let mut taken: u64 = 0;
        let mut zero_filled: u64 = 0;
        let stream = device
            .build_output_stream(
                &config,
                move |out: &mut [f32], _| {
                    // Realtime pull path: a bounded, wait-free discard check,
                    // then a wait-free pop per sample with zero-fill on
                    // underrun. No allocation, no locks, no I/O.
                    let watermark = discard_watermark.load(Ordering::Acquire);
                    if taken < watermark {
                        // Advance the read index over stale audio in one step:
                        // `commit_all` on a read chunk is an index bump, so the
                        // cost is O(1) in the amount dropped, not O(n).
                        let stale = usize::try_from(watermark - taken).unwrap_or(usize::MAX);
                        let drop_now = stale.min(consumer.slots());
                        if drop_now > 0
                            && let Ok(chunk) = consumer.read_chunk(drop_now)
                        {
                            chunk.commit_all();
                            taken += drop_now as u64;
                        }
                    }
                    for sample in out.iter_mut() {
                        if let Ok(value) = consumer.pop() {
                            *sample = value;
                            taken += 1;
                        } else {
                            *sample = 0.0;
                            zero_filled += 1;
                        }
                    }
                    taken_counter.store(taken, Ordering::Release);
                    underrun_counter.store(zero_filled, Ordering::Release);
                },
                move |_| {
                    // May be invoked from the audio thread on some hosts:
                    // an atomic store is the only realtime-safe report.
                    error_flag.store(true, Ordering::Release);
                },
                None,
            )
            .map_err(OpenAttemptError::Build)?;
        stream.play().map_err(OpenAttemptError::Play)?;
        Ok(Self {
            _stream: stream,
            producer,
            failed,
            discard_before,
            consumed: taken_total,
            underruns,
            written: 0,
            capacity,
            sample_rate,
            ring_frames,
            device_name: device_name.map(str::to_owned),
        })
    }

    /// The rate this stream is currently open at.
    ///
    /// Under the ADR-0009 default this tracks the source: it is the sample
    /// rate of the material being played whenever the device supports it, and
    /// the honest report of what the audio was converted *to* when it does
    /// not.
    #[must_use]
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Whether the stream reported an error since it was opened.
    #[must_use]
    pub fn failed(&self) -> bool {
        self.failed.load(Ordering::Acquire)
    }

    /// Interleaved samples handed to the device that the callback has not yet
    /// taken out of the ring — the audio standing between the last
    /// [`Sink::write`] and the speaker.
    ///
    /// Read straight from the ring indices, so a successful
    /// [`Sink::discard_buffered`] is visible here as a drop to zero.
    #[must_use]
    pub fn buffered_samples(&self) -> usize {
        self.capacity - self.producer.slots()
    }

    /// Whether a requested [`Sink::discard_buffered`] has not yet been
    /// honoured — i.e. the callback has not run (or has not run far enough)
    /// since the request.
    ///
    /// This is a status report, never something to spin on: the module docs
    /// explain why a stalled device must not be waited for.
    #[must_use]
    pub fn discard_pending(&self) -> bool {
        self.consumed.load(Ordering::Acquire) < self.discard_before.load(Ordering::Acquire)
    }

    /// Samples the callback zero-filled because the ring was empty when the
    /// device asked for audio.
    ///
    /// Counts *every* such sample, including the legitimate ones: before the
    /// first track is pumped, while stopped or paused past the buffer, and in
    /// the silence a discard deliberately creates. It is therefore meaningful
    /// as a **delta measured across a window of continuous playback**, where a
    /// nonzero value means the pump genuinely failed to keep the device fed —
    /// which is the evidence a device-ring size has to be justified with.
    #[must_use]
    pub fn underrun_samples(&self) -> u64 {
        self.underruns.load(Ordering::Acquire)
    }
}

impl Sink for DeviceSink {
    /// Push samples toward the device, sleeping on backpressure while the
    /// callback drains the ring. Runs on the engine's consumer (pump)
    /// thread — see the module docs for why blocking is acceptable here.
    fn write(&mut self, samples: &[f32]) {
        let mut offset = 0;
        while offset < samples.len() {
            if self.failed.load(Ordering::Acquire) {
                // The stream is dead; drop the rest rather than spin forever.
                return;
            }
            let free = self.producer.slots();
            if free == 0 {
                thread::sleep(Duration::from_micros(200));
                continue;
            }
            let n = free.min(samples.len() - offset);
            if let Ok(mut chunk) = self.producer.write_chunk(n) {
                let (a, b) = chunk.as_mut_slices();
                let a_len = a.len();
                a.copy_from_slice(&samples[offset..offset + a_len]);
                b.copy_from_slice(&samples[offset + a_len..offset + n]);
                chunk.commit_all();
                offset += n;
                self.written += n as u64;
            }
        }
    }

    /// Drop every sample already queued for the device, so the next
    /// [`Sink::write`] is the next thing heard.
    ///
    /// One release store of the running written-sample count and nothing
    /// else: no lock, no allocation, and — crucially — no wait for the
    /// callback to confirm. The module docs give the full argument for why a
    /// monotone watermark needs no handshake to be exact, and what happens
    /// when the callback never runs again.
    fn discard_buffered(&mut self) {
        self.discard_before.store(self.written, Ordering::Release);
    }

    /// Reopen the stream so the device runs at `desired` Hz, returning the
    /// rate it ended up at (see the module docs' "Following the source rate").
    ///
    /// Same rate as now: nothing happens at all, and the stream — including
    /// everything buffered in it — is untouched. That is the common case (an
    /// album is one rate) and it is why following the source costs nothing
    /// track to track.
    fn negotiate_rate(&mut self, desired: u32) -> Option<u32> {
        if desired == self.sample_rate {
            return Some(self.sample_rate);
        }
        // A device that has vanished since we opened is not something a rate
        // request can fix; keep the stream we have and let `write` report.
        let Ok(device) = output_device(self.device_name.as_deref()) else {
            return Some(self.sample_rate);
        };
        // Ask the device what it can do rather than guessing. `None` means it
        // would not say, so the only honest test left is to try.
        let target = negotiated_rate(&device, desired).unwrap_or(desired);
        if target == self.sample_rate {
            return Some(self.sample_rate);
        }
        match Self::open_on(self.device_name.as_deref(), target, self.ring_frames) {
            // Build first, swap second, and only then let the old one go: a
            // failed open must leave the working stream in place rather than a
            // silent device, so the new stream has to exist before the old one
            // can be released. Shared mode permits the moment both exist.
            Ok(fresh) => {
                let stale = std::mem::replace(self, fresh);
                // Named rather than left to the end of the statement, because
                // this is the moment the old device stops: `Stream::drop` on
                // every backend baz builds against stops the stream and joins
                // its callback thread before returning, so nothing is still
                // reading the old ring once this line has run. The field order
                // above is what makes that the *first* thing `stale`'s
                // destructor does.
                drop(stale);
                Some(target)
            }
            Err(_) => Some(self.sample_rate),
        }
    }

    /// Wait for the callback to play out everything already handed over, so a
    /// following [`Sink::negotiate_rate`] cannot truncate it.
    ///
    /// Bounded (two seconds — ten times the app's ring) and abandoned
    /// immediately if the stream has
    /// faulted: the module's standing rule is that nothing waits indefinitely
    /// on a callback that may never run again. Giving up early costs at most
    /// the tail that was stuck anyway.
    fn drain_buffered(&mut self) {
        let deadline = std::time::Instant::now() + DRAIN_BUDGET;
        while self.buffered_samples() > 0 {
            if self.failed.load(Ordering::Acquire) || std::time::Instant::now() >= deadline {
                return;
            }
            thread::sleep(DRAIN_POLL);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The process's first cpal call is made on a thread that is not the
    /// caller's** — the structural half of the Windows access-violation fix
    /// (module docs, "Why cpal is first touched from a thread that never
    /// exits").
    ///
    /// The property that matters is unobservable from outside: it is that the
    /// thread which built cpal's process-global state never exits, so its
    /// thread-local `CoUninitialize()` never runs. What *can* be asserted is
    /// the thing that guarantees it — that the first call came from neither
    /// the test thread nor the short-lived thread that asked for a device, but
    /// from a third one this module owns and never joins. A regression that
    /// removed the anchor and called cpal inline would fail here on every
    /// platform, which is the point: Linux and macOS must be able to police an
    /// invariant only Windows punishes.
    #[test]
    fn the_first_cpal_call_is_made_on_a_thread_of_our_own() {
        let caller = thread::current().id();
        // A thread that asks for a device and then exits: exactly the shape
        // that used to poison the next caller.
        let short_lived = thread::spawn(|| {
            drop(default_output_device());
            thread::current().id()
        })
        .join()
        .expect("the asking thread must not take the process with it");

        let anchor = HOST_ANCHORED
            .get()
            .copied()
            .expect("asking for a device must have anchored the host")
            .expect("the anchor thread must have been spawned");
        assert_ne!(
            anchor, caller,
            "cpal must not be first touched on a caller's thread"
        );
        assert_ne!(
            anchor, short_lived,
            "cpal must not be first touched on a thread that then exits"
        );
    }

    /// Anchoring happens once and never moves: further device lookups must not
    /// re-anchor or spawn a second anchor, because the invariant is about the
    /// *first* call and there is only one global to protect.
    #[test]
    fn anchoring_happens_once_and_never_moves() {
        drop(default_output_device());
        let first = HOST_ANCHORED.get().copied().expect("anchored");
        for _ in 0..4 {
            drop(default_output_device());
            assert_eq!(HOST_ANCHORED.get().copied(), Some(first));
        }
    }
}
