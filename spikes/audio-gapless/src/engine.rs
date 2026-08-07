//! Playlist engine: decode thread -> lock-free SPSC ring -> consumer.
//!
//! Threading model (the architecture this spike exists to prove):
//!
//! - **Decode (producer) thread** streams the current track packet-by-packet
//!   into an `rtrb` ring buffer, blocking (sleep-poll) on backpressure.
//! - **Prefetch thread** decodes track N+1 fully into memory while track N is
//!   still streaming/draining, without ever touching the ring.
//! - **Consumer** (stand-in for the audio callback) pulls from the ring in
//!   bounded chunks. Its pull path is wait-free by construction:
//!   `Consumer::read_chunk` + a preallocated sink — no allocation, no locks,
//!   no I/O. The pacing sleep emulates a device draining at finite speed and
//!   would not exist in a real callback (the device provides the cadence).
//!
//! Sample-rate change at a boundary is handled by one of two measurable
//! strategies (`RateStrategy`), compared in the tests for ADR-0004.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread::{self, Scope, ScopedJoinHandle};
use std::time::{Duration, Instant};

use rtrb::{Consumer, Producer, RingBuffer};

use crate::resample::resample_interleaved;
use crate::sink::Sink;
use crate::source::AudioSource;
use crate::{Error, Result};

/// What to do when the next track's sample rate differs from the stream rate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RateStrategy {
    /// Flush/drain the stream and notionally reopen the device at the new
    /// rate. Sample-accurate within each segment; produces an audible gap on
    /// real hardware. The engine measures the cost.
    Reopen,
    /// Convert the next track to the current stream rate with `rubato` and
    /// splice seamlessly.
    Resample,
}

/// Engine tuning for a run.
#[derive(Clone, Copy, Debug)]
pub struct EngineConfig {
    /// Ring capacity in frames.
    pub ring_frames: usize,
    /// Max frames the consumer pulls per iteration.
    pub consumer_chunk_frames: usize,
    /// Sleep between consumer pulls (emulates device drain cadence).
    pub consumer_pace: Duration,
    /// Boundary strategy on sample-rate change.
    pub strategy: RateStrategy,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            ring_frames: 8192,
            consumer_chunk_frames: 2048,
            consumer_pace: Duration::from_micros(500),
            strategy: RateStrategy::Resample,
        }
    }
}

/// A notional stream reconfigure produced by [`RateStrategy::Reopen`].
#[derive(Clone, Debug)]
pub struct Reconfigure {
    /// Output frame index at which the new rate begins.
    pub at_output_frame: usize,
    /// Stream rate before the boundary.
    pub from_rate: u32,
    /// Stream rate after the boundary.
    pub to_rate: u32,
    /// Frames still buffered in the ring at the boundary — this is what a
    /// hard flush would discard, or what a drain must wait for.
    pub buffered_frames_at_boundary: usize,
    /// The same, in milliseconds of audio at the old rate.
    pub buffered_ms_at_boundary: f64,
    /// Wall time the producer waited for the consumer to drain the ring
    /// before the notional reopen (drain-then-reconfigure policy).
    pub drain_wait_ms: f64,
}

/// Evidence that decode-ahead of track N+1 overlapped playback of track N.
#[derive(Clone, Debug, Default)]
pub struct PrefetchEvidence {
    /// Total frames in track N+1.
    pub next_track_frames_total: usize,
    /// Frames of track N+1 already decoded at the instant the consumer
    /// finished draining track N.
    pub next_frames_decoded_when_prev_drained: usize,
    /// Whether track N+1 decode had fully completed by that instant.
    pub next_decode_finished_before_prev_drained: bool,
    /// When (ms from engine start) the consumer drained the last sample of
    /// track N.
    pub prev_drain_ms_from_start: f64,
    /// When (ms from engine start) track N+1 decode completed.
    pub next_decode_done_ms_from_start: f64,
    /// Wall time spent decoding track N+1.
    pub next_decode_ms: f64,
}

/// Result of one playlist run.
#[derive(Clone, Debug)]
pub struct PlayReport {
    /// The stream rate (rate of the first track).
    pub stream_rate: u32,
    /// Channel count.
    pub channels: usize,
    /// Output frame index where each track begins.
    pub track_start_frames: Vec<usize>,
    /// Reconfigure events (Reopen strategy only).
    pub reconfigures: Vec<Reconfigure>,
    /// Decode-ahead instrumentation for the first boundary.
    pub prefetch: PrefetchEvidence,
    /// Wall time spent resampling (Resample strategy only).
    pub resample_ms: Option<f64>,
}

#[derive(Default)]
struct Shared {
    producer_done: AtomicBool,
    track0_out_samples: AtomicUsize,
    prefetch_frames: AtomicUsize,
    prefetch_total_frames: AtomicUsize,
    prefetch_done_ns: AtomicU64,
    prefetch_decode_ns: AtomicU64,
    track0_drain_ns: AtomicU64,
    prefetch_frames_at_drain: AtomicUsize,
    prefetch_done_at_drain: AtomicBool,
}

struct ProducerOut {
    boundaries: Vec<usize>,
    reconfigures: Vec<Reconfigure>,
    resample_ms: Option<f64>,
}

fn ns_to_ms(ns: u64) -> f64 {
    if ns == u64::MAX {
        f64::NAN
    } else {
        ns as f64 / 1.0e6
    }
}

/// Play a playlist of files through the engine into `sink`.
///
/// The consumer loop runs on the calling thread; decode and prefetch run on
/// worker threads. Returns instrumentation for the tests.
pub fn run_playlist(
    paths: &[PathBuf],
    cfg: EngineConfig,
    sink: &mut dyn Sink,
) -> Result<PlayReport> {
    let first_path = paths.first().ok_or_else(|| Error::from("empty playlist"))?;
    let first = AudioSource::open(first_path)?;
    let stream_rate = first.sample_rate();
    let channels = first.channels();
    let (producer, mut consumer) = RingBuffer::<f32>::new(cfg.ring_frames * channels);

    let shared = Arc::new(Shared::default());
    shared.prefetch_done_ns.store(u64::MAX, Ordering::Release);
    shared.track0_drain_ns.store(u64::MAX, Ordering::Release);
    let start = Instant::now();

    let out = thread::scope(|s| -> Result<ProducerOut> {
        let sh = Arc::clone(&shared);
        let handle = s.spawn(move || {
            let res = produce(
                s,
                paths,
                first,
                producer,
                cfg,
                stream_rate,
                channels,
                &sh,
                start,
            );
            // Always release the consumer, even on error.
            sh.producer_done.store(true, Ordering::Release);
            res
        });
        consume(&mut consumer, sink, cfg, &shared, start);
        handle
            .join()
            .map_err(|_| Error::from("producer thread panicked"))?
    })?;

    Ok(PlayReport {
        stream_rate,
        channels,
        track_start_frames: out.boundaries,
        reconfigures: out.reconfigures,
        prefetch: PrefetchEvidence {
            next_track_frames_total: shared.prefetch_total_frames.load(Ordering::Acquire),
            next_frames_decoded_when_prev_drained: shared
                .prefetch_frames_at_drain
                .load(Ordering::Acquire),
            next_decode_finished_before_prev_drained: shared
                .prefetch_done_at_drain
                .load(Ordering::Acquire),
            prev_drain_ms_from_start: ns_to_ms(shared.track0_drain_ns.load(Ordering::Acquire)),
            next_decode_done_ms_from_start: ns_to_ms(
                shared.prefetch_done_ns.load(Ordering::Acquire),
            ),
            next_decode_ms: ns_to_ms(shared.prefetch_decode_ns.load(Ordering::Acquire)),
        },
        resample_ms: out.resample_ms,
    })
}

#[allow(clippy::too_many_arguments)]
fn produce<'scope, 'env: 'scope>(
    scope: &'scope Scope<'scope, 'env>,
    paths: &'env [PathBuf],
    first: AudioSource,
    mut prod: Producer<f32>,
    cfg: EngineConfig,
    stream_rate: u32,
    channels: usize,
    shared: &Arc<Shared>,
    start: Instant,
) -> Result<ProducerOut> {
    type Prefetched<'a> = ScopedJoinHandle<'a, Result<(Vec<f32>, u32, usize)>>;

    // Kick off decode-ahead of track 1 before track 0 starts streaming.
    let mut next: Option<Prefetched<'scope>> = paths.get(1).map(|p| {
        let sh = Arc::clone(shared);
        scope.spawn(move || prefetch_decode(p, &sh, start))
    });

    let mut boundaries = vec![0usize];
    let mut reconfigures = Vec::new();
    let mut resample_ms = None;
    let mut pushed_samples = 0usize;

    // Stream track 0 packet-by-packet through the ring.
    let mut src = first;
    while let Some(block) = src.next_block()? {
        push_backpressure(&mut prod, block);
        pushed_samples += block.len();
    }
    shared
        .track0_out_samples
        .store(pushed_samples, Ordering::Release);

    for i in 1..paths.len() {
        let (samples, rate, ch) = next
            .take()
            .expect("prefetch handle exists for track i")
            .join()
            .map_err(|_| Error::from("prefetch thread panicked"))??;
        if ch != channels {
            return Err(Error::from(format!(
                "channel count change ({channels} -> {ch}) not handled by this spike"
            )));
        }
        // Start decode-ahead of the following track (uninstrumented; only the
        // first boundary carries the evidence the tests need).
        next = paths
            .get(i + 1)
            .map(|p| scope.spawn(move || AudioSource::decode_all(p)));

        let samples = if rate == stream_rate {
            samples
        } else {
            match cfg.strategy {
                RateStrategy::Resample => {
                    let t0 = Instant::now();
                    let out = resample_interleaved(&samples, ch, rate, stream_rate)?;
                    resample_ms = Some(t0.elapsed().as_secs_f64() * 1.0e3);
                    out
                }
                RateStrategy::Reopen => {
                    let capacity = prod.buffer().capacity();
                    let buffered_samples = capacity - prod.slots();
                    let buffered_frames = buffered_samples / channels;
                    let t0 = Instant::now();
                    // Drain-then-reconfigure: wait for the consumer to play
                    // out everything buffered at the old rate.
                    while prod.slots() < capacity {
                        thread::sleep(Duration::from_micros(100));
                    }
                    let drain_wait_ms = t0.elapsed().as_secs_f64() * 1.0e3;
                    reconfigures.push(Reconfigure {
                        at_output_frame: pushed_samples / channels,
                        from_rate: stream_rate,
                        to_rate: rate,
                        buffered_frames_at_boundary: buffered_frames,
                        buffered_ms_at_boundary: buffered_frames as f64 / f64::from(stream_rate)
                            * 1.0e3,
                        drain_wait_ms,
                    });
                    // A real implementation would close and reopen the device
                    // here; the ring simply continues at the new nominal rate.
                    samples
                }
            }
        };

        boundaries.push(pushed_samples / channels);
        push_backpressure(&mut prod, &samples);
        pushed_samples += samples.len();
    }

    Ok(ProducerOut {
        boundaries,
        reconfigures,
        resample_ms,
    })
}

/// Decode a whole file on the prefetch thread, publishing progress atomically
/// so the consumer can snapshot "how much of track N+1 existed when track N
/// finished draining".
fn prefetch_decode(path: &Path, shared: &Shared, start: Instant) -> Result<(Vec<f32>, u32, usize)> {
    let t0 = Instant::now();
    let mut src = AudioSource::open(path)?;
    let ch = src.channels();
    let mut v: Vec<f32> = Vec::new();
    while let Some(block) = src.next_block()? {
        v.extend_from_slice(block);
        shared
            .prefetch_frames
            .store(v.len() / ch, Ordering::Release);
    }
    shared
        .prefetch_total_frames
        .store(v.len() / ch, Ordering::Release);
    shared.prefetch_decode_ns.store(
        u64::try_from(t0.elapsed().as_nanos()).unwrap_or(u64::MAX - 1),
        Ordering::Release,
    );
    shared.prefetch_done_ns.store(
        u64::try_from(start.elapsed().as_nanos()).unwrap_or(u64::MAX - 1),
        Ordering::Release,
    );
    Ok((v, src.sample_rate(), ch))
}

/// Push a block into the ring, sleeping briefly whenever it is full.
fn push_backpressure(prod: &mut Producer<f32>, data: &[f32]) {
    let mut off = 0;
    while off < data.len() {
        let free = prod.slots();
        if free == 0 {
            thread::sleep(Duration::from_micros(100));
            continue;
        }
        let n = free.min(data.len() - off);
        if let Ok(mut chunk) = prod.write_chunk(n) {
            let (a, b) = chunk.as_mut_slices();
            let al = a.len();
            a.copy_from_slice(&data[off..off + al]);
            b.copy_from_slice(&data[off + al..off + n]);
            chunk.commit_all();
            off += n;
        }
    }
}

/// The consumer loop — the stand-in for the audio callback.
///
/// Realtime discipline on the pull path, by construction:
/// - `Consumer::read_chunk` is wait-free (rtrb SPSC).
/// - The sink writes into preallocated storage.
/// - No locks, no I/O, no allocation.
///
/// The pacing sleep emulates the device's drain cadence and is not part of the
/// pull path itself.
fn consume(
    cons: &mut Consumer<f32>,
    sink: &mut dyn Sink,
    cfg: EngineConfig,
    shared: &Shared,
    start: Instant,
) {
    let channels_hint = 2; // chunk sizing only; correctness is unaffected
    let chunk_samples = cfg.consumer_chunk_frames * channels_hint;
    let mut consumed = 0usize;
    let mut drain_recorded = false;
    loop {
        let done = shared.producer_done.load(Ordering::Acquire);
        let avail = cons.slots();
        if avail == 0 {
            if done {
                break;
            }
            thread::sleep(Duration::from_micros(50));
            continue;
        }
        let n = avail.min(chunk_samples);
        if let Ok(chunk) = cons.read_chunk(n) {
            let (a, b) = chunk.as_slices();
            sink.write(a);
            if !b.is_empty() {
                sink.write(b);
            }
            chunk.commit_all();
            consumed += n;
        }
        if !drain_recorded {
            let t0 = shared.track0_out_samples.load(Ordering::Acquire);
            if t0 > 0 && consumed >= t0 {
                drain_recorded = true;
                shared.track0_drain_ns.store(
                    u64::try_from(start.elapsed().as_nanos()).unwrap_or(u64::MAX - 1),
                    Ordering::Release,
                );
                shared.prefetch_frames_at_drain.store(
                    shared.prefetch_frames.load(Ordering::Acquire),
                    Ordering::Release,
                );
                shared.prefetch_done_at_drain.store(
                    shared.prefetch_done_ns.load(Ordering::Acquire) != u64::MAX,
                    Ordering::Release,
                );
            }
        }
        if !cfg.consumer_pace.is_zero() {
            thread::sleep(cfg.consumer_pace);
        }
    }
}
