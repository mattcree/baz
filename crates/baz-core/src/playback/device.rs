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
//! Exclusive-mode backends (ALSA `hw:`, WASAPI exclusive, `CoreAudio` hog) are
//! a later phase and the prerequisite for
//! [`BoundaryPolicy::BitPerfectReopen`](super::BoundaryPolicy::BitPerfectReopen).
//!
//! [`EngineConfig::consumer_pace`]: super::EngineConfig

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rtrb::{Producer, RingBuffer};

use super::sink::Sink;
use super::{CHANNELS, PlaybackError};

/// A [`Sink`] that plays samples on the default output device.
///
/// Dropping the sink stops the stream; samples still buffered in the device
/// ring at drop are discarded. To drop them *without* closing the stream —
/// what a seek, skip, or stop needs — use [`Sink::discard_buffered`], whose
/// lock-free mechanism the module docs describe in full.
pub struct DeviceSink {
    producer: Producer<f32>,
    /// Keeps the stream alive; playback stops when this is dropped.
    _stream: cpal::Stream,
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
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| PlaybackError::Device("no default output device".into()))?;
        let config = cpal::StreamConfig {
            channels: u16::try_from(CHANNELS)
                .map_err(|_| PlaybackError::Device("channel count exceeds u16".into()))?,
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
            .map_err(|e| PlaybackError::Device(e.to_string()))?;
        stream
            .play()
            .map_err(|e| PlaybackError::Device(e.to_string()))?;
        Ok(Self {
            producer,
            _stream: stream,
            failed,
            discard_before,
            consumed: taken_total,
            underruns,
            written: 0,
            capacity,
        })
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
}
