//! Sink abstraction: where the consumer (pull) side of the engine delivers
//! samples. The pull path is the stand-in for the real audio callback, so
//! sink implementations used on it must uphold the realtime rules by
//! construction (`docs/ENGINEERING.md`, "the audio thread is sacred").

/// Destination for interleaved stereo f32 samples pulled off the ring buffer.
///
/// # Realtime contract
///
/// `write` is called from the engine's consumer loop — the realtime stand-in.
/// Implementations used there must not allocate, lock, block, panic, or
/// perform I/O. [`OfflineSink`] satisfies this structurally: its storage is
/// preallocated and it *never* grows it (overflow is counted and dropped, not
/// reallocated), so `write` is a bounds-checked memcpy plus a length bump.
pub trait Sink {
    /// Accept a block of interleaved stereo f32 samples.
    fn write(&mut self, samples: &[f32]);
}

/// Offline sink that collects every sample into preallocated storage — the
/// headless test workhorse.
///
/// The backing buffer is allocated once in [`OfflineSink::with_capacity`] and
/// never reallocated: a `write` that would exceed capacity stores what fits
/// and counts the rest in [`OfflineSink::dropped_samples`]. Tests assert that
/// counter is zero (and that the buffer pointer never moved), which is the
/// crate's honest, `forbid(unsafe_code)`-compatible evidence that the pull
/// path performed no allocation — see the sacred-thread test in
/// `tests/playback.rs`.
#[derive(Debug)]
pub struct OfflineSink {
    samples: Vec<f32>,
    dropped: usize,
}

impl OfflineSink {
    /// Create a sink with room for exactly `capacity_samples` interleaved
    /// samples.
    #[must_use]
    pub fn with_capacity(capacity_samples: usize) -> Self {
        Self {
            samples: Vec::with_capacity(capacity_samples),
            dropped: 0,
        }
    }

    /// All samples received so far.
    #[must_use]
    pub fn samples(&self) -> &[f32] {
        &self.samples
    }

    /// Consume the sink, returning the collected samples.
    #[must_use]
    pub fn into_samples(self) -> Vec<f32> {
        self.samples
    }

    /// Samples discarded because they would have exceeded the preallocated
    /// capacity. Nonzero means the caller undersized the sink.
    #[must_use]
    pub fn dropped_samples(&self) -> usize {
        self.dropped
    }
}

impl Sink for OfflineSink {
    fn write(&mut self, samples: &[f32]) {
        let room = self.samples.capacity() - self.samples.len();
        let take = samples.len().min(room);
        // Within capacity by construction: extend_from_slice cannot
        // reallocate here.
        self.samples.extend_from_slice(&samples[..take]);
        self.dropped += samples.len() - take;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overflow_is_dropped_not_reallocated() {
        let mut sink = OfflineSink::with_capacity(4);
        let ptr = sink.samples().as_ptr();
        sink.write(&[1.0, 2.0, 3.0]);
        sink.write(&[4.0, 5.0, 6.0]);
        assert_eq!(sink.samples(), &[1.0, 2.0, 3.0, 4.0]);
        assert_eq!(sink.dropped_samples(), 2);
        assert_eq!(sink.samples().as_ptr(), ptr, "storage must never move");
    }
}
