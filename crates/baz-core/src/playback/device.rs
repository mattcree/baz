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
//! Exclusive-mode backends (ALSA `hw:`, WASAPI exclusive, `CoreAudio` hog) are
//! a later phase and the prerequisite for
//! [`BoundaryPolicy::BitPerfectReopen`](super::BoundaryPolicy::BitPerfectReopen).
//!
//! [`EngineConfig::consumer_pace`]: super::EngineConfig

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rtrb::{Producer, RingBuffer};

use super::sink::Sink;
use super::{CHANNELS, PlaybackError};

/// A [`Sink`] that plays samples on the default output device.
///
/// Dropping the sink stops the stream. Samples still buffered in the device
/// ring at drop are discarded; a graceful drain API arrives with the
/// playback-control unit.
pub struct DeviceSink {
    producer: Producer<f32>,
    /// Keeps the stream alive; playback stops when this is dropped.
    _stream: cpal::Stream,
    failed: Arc<AtomicBool>,
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
        let (producer, mut consumer) = RingBuffer::<f32>::new(ring_frames * CHANNELS);
        let failed = Arc::new(AtomicBool::new(false));
        let error_flag = Arc::clone(&failed);
        let stream = device
            .build_output_stream(
                &config,
                move |out: &mut [f32], _| {
                    // Realtime pull path: wait-free pop per sample, zero-fill
                    // on underrun. No allocation, no locks, no I/O.
                    for sample in out.iter_mut() {
                        *sample = consumer.pop().unwrap_or(0.0);
                    }
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
        })
    }

    /// Whether the stream reported an error since it was opened.
    #[must_use]
    pub fn failed(&self) -> bool {
        self.failed.load(Ordering::Acquire)
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
            }
        }
    }
}
