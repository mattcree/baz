//! Device output via cpal — feature-gated and UNVERIFIED on this machine
//! (alsa-lib-devel is not installed, so cpal cannot build here).
//!
//! Kept small on purpose: it shows where the ring's consumer end plugs into a
//! real audio callback, upholding the same realtime rules the offline consumer
//! proves (wait-free pops, zero-fill on underrun, no alloc/lock/I/O).

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use crate::{Error, Result};

/// Open the default output device at `sample_rate`/`channels` and drain the
/// ring consumer from the audio callback. The returned stream must be kept
/// alive by the caller.
pub fn play_ring(
    mut consumer: rtrb::Consumer<f32>,
    sample_rate: u32,
    channels: u16,
) -> Result<cpal::Stream> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| Error::from("no default output device"))?;
    let config = cpal::StreamConfig {
        channels,
        sample_rate: cpal::SampleRate(sample_rate),
        buffer_size: cpal::BufferSize::Default,
    };
    let stream = device.build_output_stream(
        &config,
        move |out: &mut [f32], _| {
            // Realtime pull path: wait-free pop per sample, zero-fill on
            // underrun. No allocation, no locks, no I/O.
            for s in out.iter_mut() {
                *s = consumer.pop().unwrap_or(0.0);
            }
        },
        |e| eprintln!("stream error: {e}"),
        None,
    )?;
    stream.play()?;
    Ok(stream)
}
