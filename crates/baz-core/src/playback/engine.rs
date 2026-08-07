//! The playlist engine: decode thread → lock-free SPSC ring → consumer.
//!
//! Threading model (proven in Spike B, ratified in ADR-0003/ADR-0004):
//!
//! - **Decode (producer) thread** streams the current track block-by-block
//!   into an `rtrb` ring buffer, sleeping briefly on backpressure.
//! - **Prefetch thread** decodes track N+1 fully into memory while track N
//!   is still streaming/draining. It never touches the ring, so the
//!   consumer's guarantees are structurally unaffected by decode-ahead. Any
//!   boundary resampling happens here too — which, since ADR-0009 made
//!   following the source the default, means only when a fixed output rate
//!   was explicitly selected.
//! - **Consumer** (the stand-in for the audio callback) pulls from the ring
//!   in bounded chunks on the caller's thread. Its pull path is wait-free by
//!   construction: `rtrb::Consumer::read_chunk` plus a preallocated
//!   [`Sink`] — no allocation, no locks, no I/O, no panics. The pacing sleep
//!   emulates a device draining at finite speed and sits *between* pulls,
//!   not on the pull path; a real audio callback gets its cadence from the
//!   device instead.
//!
//! Track boundaries are bookkeeping, not audio events: the splice is plain
//! sample-accurate concatenation through the ring, which is what makes
//! playback gapless.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::thread::{self, Scope, ScopedJoinHandle};
use std::time::{Duration, Instant};

use rtrb::{Consumer, Producer, RingBuffer};

use super::resample::resample_interleaved;
use super::sink::Sink;
use super::source::{AudioSource, DecodedAudio};
use super::{CHANNELS, PlaybackError};

/// What to do when a track's sample rate differs from the rate the output
/// stream is running at.
///
/// The two-mode contract is ADR-0004's; **which mode is the default was
/// inverted by ADR-0009**. baz never converts sample rates unless it is asked
/// to, or unless the hardware leaves it no choice — and in the latter case it
/// says so ([`Event::SignalPath`](crate::protocol::Event::SignalPath)) rather
/// than converting quietly.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum BoundaryPolicy {
    /// Resample the incoming track to the session's stream rate on the
    /// prefetch thread (rubato windowed sinc, splice-exact alignment). The
    /// stream never closes between tracks: gapless across a rate change, at
    /// the cost of sample-rate conversion. Zero cost on same-rate boundaries.
    ///
    /// ADR-0004's default; ADR-0009 demoted it to an explicit opt-in, because
    /// converting a master the device could have played untouched is a
    /// fidelity loss taken for a convenience the listener did not ask for.
    ResampleToStreamRate,
    /// **The default.** Never resample: follow the source. The output stream
    /// is opened at the rate of the track that starts a session, and a track
    /// at a different rate ends that session and begins a new one, reopening
    /// the device at *its* rate ([`Sink::negotiate_rate`]).
    ///
    /// Gapless is preserved within a run of same-rate tracks — an album is one
    /// rate, so this is the ordinary case — and a boundary between *different*
    /// rates carries a short gap while the device is reconfigured. ADR-0009
    /// records the measured length of that gap and accepts it.
    ///
    /// The one thing this mode cannot promise is what a device it cannot
    /// negotiate with will do: if the hardware offers no mode at the source
    /// rate, the engine plays at the nearest rate it does offer, resamples,
    /// and reports the chain as
    /// [`Converting`](crate::protocol::SignalChain::Converting) with reason
    /// [`DeviceRateUnavailable`](crate::protocol::ConversionReason::DeviceRateUnavailable).
    /// Playing the music is the right answer there; doing it silently is the
    /// outcome this mode exists to prevent.
    #[default]
    BitPerfectReopen,
}

/// Engine tuning for one run.
#[derive(Clone, Copy, Debug)]
pub struct EngineConfig {
    /// Ring capacity in stereo frames.
    pub ring_frames: usize,
    /// Maximum frames the consumer pulls per iteration.
    pub consumer_chunk_frames: usize,
    /// Sleep between consumer pulls (emulates device drain cadence; zero
    /// means drain as fast as the producer fills).
    pub consumer_pace: Duration,
    /// What to do when a track's sample rate differs from the output's
    /// (ADR-0004's two modes, ADR-0009's default).
    pub boundary: BoundaryPolicy,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            ring_frames: 8192,
            consumer_chunk_frames: 2048,
            consumer_pace: Duration::from_micros(500),
            boundary: BoundaryPolicy::default(),
        }
    }
}

/// Evidence that decode-ahead of track N+1 overlapped playback of track N.
///
/// Instrumented for the first boundary only; the tests (and any future
/// signal-path readout) rely on it.
#[derive(Clone, Copy, Debug, Default)]
pub struct PrefetchEvidence {
    /// Total frames in track N+1 (at its native rate).
    pub next_track_frames_total: usize,
    /// Frames of track N+1 already decoded at the instant the consumer
    /// finished draining track N.
    pub next_frames_decoded_when_prev_drained: usize,
    /// Whether track N+1 decode had fully completed by that instant.
    pub next_decode_finished_before_prev_drained: bool,
    /// When (ms from engine start) the consumer drained the last sample of
    /// track N. `NaN` if never recorded.
    pub prev_drain_ms_from_start: f64,
    /// When (ms from engine start) track N+1 decode completed. `NaN` if it
    /// never completed.
    pub next_decode_done_ms_from_start: f64,
    /// Wall time spent decoding track N+1.
    pub next_decode_ms: f64,
}

/// Result of one playlist run.
#[derive(Clone, Debug)]
pub struct PlayReport {
    /// The stream rate: the rate of the first track, which is the negotiation
    /// policy ADR-0009 settled on everywhere (see [`crate::engine`] for why
    /// the anchor, and not the queue's most-common or highest rate).
    pub stream_rate: u32,
    /// Output frame index where each track begins.
    pub track_start_frames: Vec<usize>,
    /// Decode-ahead instrumentation for the first boundary.
    pub prefetch: PrefetchEvidence,
    /// Wall time spent resampling, if any boundary needed it.
    pub resample_ms: Option<f64>,
}

/// Cross-thread instrumentation. Atomics only — the consumer side reads
/// these on its loop but never blocks on them.
#[derive(Default)]
struct SharedState {
    producer_done: AtomicBool,
    track0_out_samples: AtomicUsize,
    prefetch_frames: AtomicUsize,
    prefetch_total_frames: AtomicUsize,
    prefetch_done_ns: AtomicU64,
    prefetch_decode_ns: AtomicU64,
    prefetch_frames_at_drain: AtomicUsize,
    prefetch_done_at_drain: AtomicBool,
}

/// Sentinel for "not recorded" in the nanosecond atomics.
const NS_UNSET: u64 = u64::MAX;

struct ProducerOutcome {
    track_start_frames: Vec<usize>,
    resample_ms: Option<f64>,
}

/// The rate this render runs at and what to do about a track that disagrees —
/// the two facts the producer needs at every boundary, and the only two.
#[derive(Clone, Copy)]
struct StreamSpec {
    rate: u32,
    boundary: BoundaryPolicy,
}

fn ns_to_ms(ns: u64) -> f64 {
    if ns == NS_UNSET {
        f64::NAN
    } else {
        // Durations here are milliseconds-to-seconds scale; f64 precision
        // loss is far below measurement noise.
        #[allow(clippy::cast_precision_loss)]
        let ms = ns as f64 / 1.0e6;
        ms
    }
}

fn elapsed_ns(since: Instant) -> u64 {
    u64::try_from(since.elapsed().as_nanos()).unwrap_or(NS_UNSET - 1)
}

/// Play a playlist of files through the engine into `sink`.
///
/// The consumer loop runs on the calling thread; decode and prefetch run on
/// scoped worker threads. Returns instrumentation for tests and diagnostics.
///
/// # One render, one rate
///
/// This is the offline one-shot: it fills a single `sink` with a single
/// continuous stream, so it has nowhere to put a sample-rate change. Under the
/// default [`BoundaryPolicy::BitPerfectReopen`] it therefore *refuses* a queue
/// whose rates differ, with
/// [`PlaybackError::SampleRateChangeRequiresReopen`], rather than converting
/// audio the caller asked it not to convert. Reopening the output at the new
/// rate is a thing only a live output can do, and the interactive service
/// ([`crate::engine`]) is where that lives. Callers who genuinely want one
/// buffer at one rate ask for [`BoundaryPolicy::ResampleToStreamRate`] and get
/// exactly the old behaviour.
///
/// # Errors
///
/// [`PlaybackError::EmptyPlaylist`] for an empty `paths`;
/// [`PlaybackError::SampleRateChangeRequiresReopen`] for a mixed-rate queue
/// under the bit-perfect default (see above); otherwise any decode, resample,
/// or worker failure from the pipeline.
pub fn run_playlist(
    paths: &[PathBuf],
    cfg: EngineConfig,
    sink: &mut dyn Sink,
) -> Result<PlayReport, PlaybackError> {
    let first_path = paths.first().ok_or(PlaybackError::EmptyPlaylist)?;
    let first = AudioSource::open(first_path)?;
    let stream_rate = first.sample_rate();
    let (producer, mut consumer) = RingBuffer::<f32>::new(cfg.ring_frames * CHANNELS);

    let shared = Arc::new(SharedState::default());
    shared.prefetch_done_ns.store(NS_UNSET, Ordering::Release);
    let start = Instant::now();
    let mut drain = DrainWatch::new(start);

    let outcome = thread::scope(|s| -> Result<ProducerOutcome, PlaybackError> {
        let sh = Arc::clone(&shared);
        let handle = s.spawn(move || {
            let stream = StreamSpec {
                rate: stream_rate,
                boundary: cfg.boundary,
            };
            let res = produce(s, paths, first, producer, stream, &sh, start);
            // Always release the consumer, even on error.
            sh.producer_done.store(true, Ordering::Release);
            res
        });
        consume(&mut consumer, sink, &cfg, &shared, &mut drain);
        handle
            .join()
            .map_err(|_| PlaybackError::WorkerPanicked("producer"))?
    })?;

    Ok(PlayReport {
        stream_rate,
        track_start_frames: outcome.track_start_frames,
        prefetch: PrefetchEvidence {
            next_track_frames_total: shared.prefetch_total_frames.load(Ordering::Acquire),
            next_frames_decoded_when_prev_drained: shared
                .prefetch_frames_at_drain
                .load(Ordering::Acquire),
            next_decode_finished_before_prev_drained: shared
                .prefetch_done_at_drain
                .load(Ordering::Acquire),
            prev_drain_ms_from_start: drain.recorded_ns.map_or(f64::NAN, ns_to_ms),
            next_decode_done_ms_from_start: ns_to_ms(
                shared.prefetch_done_ns.load(Ordering::Acquire),
            ),
            next_decode_ms: ns_to_ms(shared.prefetch_decode_ns.load(Ordering::Acquire)),
        },
        resample_ms: outcome.resample_ms,
    })
}

fn produce<'scope, 'env: 'scope>(
    scope: &'scope Scope<'scope, 'env>,
    paths: &'env [PathBuf],
    first: AudioSource,
    mut producer: Producer<f32>,
    stream: StreamSpec,
    shared: &Arc<SharedState>,
    start: Instant,
) -> Result<ProducerOutcome, PlaybackError> {
    type Prefetched<'a> = ScopedJoinHandle<'a, Result<DecodedAudio, PlaybackError>>;

    // Kick off decode-ahead of track 1 before track 0 starts streaming.
    let mut pending: Option<Prefetched<'scope>> = paths.get(1).map(|p| {
        let sh = Arc::clone(shared);
        scope.spawn(move || prefetch_decode(p, &sh, start))
    });

    let mut track_start_frames = vec![0usize];
    let mut resample_ms = None;
    let mut pushed_samples = 0usize;

    // `run_playlist` is the uncontrolled one-shot: nothing ever sets this,
    // so pushes never abort (the interactive service uses its own flag).
    let never_stop = AtomicBool::new(false);

    // Stream track 0 block-by-block through the ring.
    let mut src = first;
    while let Some(block) = src.next_block()? {
        push_with_backpressure(&mut producer, block, &never_stop);
        pushed_samples += block.len();
    }
    shared
        .track0_out_samples
        .store(pushed_samples, Ordering::Release);

    for i in 1..paths.len() {
        // The handle is always present here (spawned for index 1 above and
        // for each i+1 below); the inline decode is a total-function fallback,
        // not an expected path.
        let decoded = match pending.take() {
            Some(handle) => handle
                .join()
                .map_err(|_| PlaybackError::WorkerPanicked("prefetch"))??,
            None => AudioSource::decode_all(&paths[i])?,
        };
        // Start decode-ahead of the following track (uninstrumented; only
        // the first boundary carries the evidence the tests need).
        pending = paths
            .get(i + 1)
            .map(|p| scope.spawn(move || AudioSource::decode_all(p)));

        // Same-rate boundaries — every boundary inside an album — cost
        // nothing and take this branch. A rate *change* is where the two
        // policies part company, and the default refuses rather than converts
        // (see `run_playlist`'s "One render, one rate").
        let samples = if decoded.sample_rate == stream.rate {
            decoded.samples
        } else if stream.boundary == BoundaryPolicy::BitPerfectReopen {
            return Err(PlaybackError::SampleRateChangeRequiresReopen {
                index: i,
                from: stream.rate,
                to: decoded.sample_rate,
            });
        } else {
            let t0 = Instant::now();
            let out = resample_interleaved(&decoded.samples, decoded.sample_rate, stream.rate)?;
            resample_ms = Some(t0.elapsed().as_secs_f64() * 1.0e3);
            out
        };

        track_start_frames.push(pushed_samples / CHANNELS);
        push_with_backpressure(&mut producer, &samples, &never_stop);
        pushed_samples += samples.len();
    }

    Ok(ProducerOutcome {
        track_start_frames,
        resample_ms,
    })
}

/// Decode a whole file on the prefetch thread, publishing progress atomically
/// so the consumer can snapshot "how much of track N+1 existed when track N
/// finished draining".
fn prefetch_decode(
    path: &Path,
    shared: &SharedState,
    start: Instant,
) -> Result<DecodedAudio, PlaybackError> {
    let t0 = Instant::now();
    let mut src = AudioSource::open(path)?;
    let mut samples: Vec<f32> = Vec::new();
    while let Some(block) = src.next_block()? {
        samples.extend_from_slice(block);
        shared
            .prefetch_frames
            .store(samples.len() / CHANNELS, Ordering::Release);
    }
    shared
        .prefetch_total_frames
        .store(samples.len() / CHANNELS, Ordering::Release);
    shared
        .prefetch_decode_ns
        .store(elapsed_ns(t0), Ordering::Release);
    shared
        .prefetch_done_ns
        .store(elapsed_ns(start), Ordering::Release);
    Ok(DecodedAudio {
        samples,
        sample_rate: src.sample_rate(),
        bits_per_sample: src.bits_per_sample(),
    })
}

/// Push a block into the ring, sleeping briefly whenever it is full.
/// Producer-side only — the consumer never blocks.
///
/// `stop` aborts the push (returning `false`) so a controlling thread can
/// release a producer that would otherwise sleep forever against a consumer
/// that has stopped pulling. [`run_playlist`] passes a flag that is never
/// set; the interactive engine service ([`crate::engine`]) sets it on stop,
/// skip, and shutdown.
pub(crate) fn push_with_backpressure(
    producer: &mut Producer<f32>,
    data: &[f32],
    stop: &AtomicBool,
) -> bool {
    let mut offset = 0;
    while offset < data.len() {
        if stop.load(Ordering::Acquire) {
            return false;
        }
        let free = producer.slots();
        if free == 0 {
            thread::sleep(Duration::from_micros(100));
            continue;
        }
        let n = free.min(data.len() - offset);
        if let Ok(mut chunk) = producer.write_chunk(n) {
            let (a, b) = chunk.as_mut_slices();
            let a_len = a.len();
            a.copy_from_slice(&data[offset..offset + a_len]);
            b.copy_from_slice(&data[offset + a_len..offset + n]);
            chunk.commit_all();
            offset += n;
        }
    }
    true
}

/// Tracks when the consumer drains track 0's last sample. Lives on the
/// consumer's stack — plain fields, no synchronization needed.
struct DrainWatch {
    start: Instant,
    recorded_ns: Option<u64>,
}

impl DrainWatch {
    fn new(start: Instant) -> Self {
        Self {
            start,
            recorded_ns: None,
        }
    }

    /// Record the drain instant once `consumed` passes track 0's length.
    /// Atomic loads/stores only — nothing here blocks or allocates.
    fn observe(&mut self, consumed: usize, shared: &SharedState) {
        if self.recorded_ns.is_some() {
            return;
        }
        let track0 = shared.track0_out_samples.load(Ordering::Acquire);
        if track0 > 0 && consumed >= track0 {
            self.recorded_ns = Some(elapsed_ns(self.start));
            shared.prefetch_frames_at_drain.store(
                shared.prefetch_frames.load(Ordering::Acquire),
                Ordering::Release,
            );
            shared.prefetch_done_at_drain.store(
                shared.prefetch_done_ns.load(Ordering::Acquire) != NS_UNSET,
                Ordering::Release,
            );
        }
    }
}

/// The consumer loop — the stand-in for the audio callback.
///
/// Realtime discipline on the pull path, by construction:
/// - `rtrb::Consumer::read_chunk` is wait-free (SPSC, no locks).
/// - The [`Sink`] contract requires writes into preallocated storage.
/// - No allocation, no locks, no I/O, no panics — every operation below is
///   a slice copy, an atomic load/store, or arithmetic.
///
/// The pacing sleep sits between pulls, emulating the device's drain
/// cadence; it is not part of the pull path itself.
fn consume(
    consumer: &mut Consumer<f32>,
    sink: &mut dyn Sink,
    cfg: &EngineConfig,
    shared: &SharedState,
    drain: &mut DrainWatch,
) {
    let chunk_samples = cfg.consumer_chunk_frames * CHANNELS;
    let mut pulled = 0usize;
    loop {
        let done = shared.producer_done.load(Ordering::Acquire);
        let available = consumer.slots();
        if available == 0 {
            if done {
                break;
            }
            thread::sleep(Duration::from_micros(50));
            continue;
        }
        let n = available.min(chunk_samples);
        if let Ok(chunk) = consumer.read_chunk(n) {
            let (a, b) = chunk.as_slices();
            sink.write(a);
            if !b.is_empty() {
                sink.write(b);
            }
            chunk.commit_all();
            pulled += n;
        }
        drain.observe(pulled, shared);
        if !cfg.consumer_pace.is_zero() {
            thread::sleep(cfg.consumer_pace);
        }
    }
}
