//! Sink abstraction. The consumer (pull) side of the engine is what a real
//! audio callback would be; it must uphold realtime rules by construction.

/// Destination for samples pulled off the ring buffer.
///
/// Contract for implementors used on the realtime path: `write` must not
/// allocate, lock, or perform I/O. `OfflineSink` satisfies this by
/// preallocating its full capacity up front.
pub trait Sink {
    /// Accept a block of interleaved f32 samples.
    fn write(&mut self, samples: &[f32]);
}

/// Test sink that collects every sample it is handed.
///
/// Capacity is preallocated so `write` is a memcpy + length bump — no
/// allocation on the pull path (as long as the caller sized it correctly;
/// exceeding capacity would reallocate, which the tests never do).
pub struct OfflineSink {
    samples: Vec<f32>,
}

impl OfflineSink {
    /// Create a sink with room for `capacity_samples` interleaved samples.
    #[must_use]
    pub fn with_capacity(capacity_samples: usize) -> Self {
        Self {
            samples: Vec::with_capacity(capacity_samples),
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
}

impl Sink for OfflineSink {
    fn write(&mut self, samples: &[f32]) {
        self.samples.extend_from_slice(samples);
    }
}
