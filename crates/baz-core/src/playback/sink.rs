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

    /// Throw away audio this sink has already accepted but not yet made
    /// audible, so that the *next* [`Self::write`] is the next thing heard.
    ///
    /// The engine calls this whenever it abandons a playback session — seek,
    /// skip, stop, queue replacement — because the samples it already handed
    /// over belong to the position the user just left. It is deliberately
    /// *not* called on pause, which keeps its buffered audio by design (see
    /// [`crate::engine`]'s "Pause, stop, and skip" docs).
    ///
    /// # Default: nothing to discard
    ///
    /// The default implementation is a no-op, which is the honest behaviour
    /// for every sink whose `write` has no downstream buffer between it and
    /// its destination — there is no "accepted but not yet audible" audio for
    /// such a sink to drop. [`OfflineSink`] is exactly that case: its `write`
    /// *is* the destination. It is a record of everything the engine
    /// delivered, so discarding could only mean deleting history, which would
    /// silently corrupt the bit-exactness tests that compare that record
    /// against a reference decode. Only a sink that queues audio for someone
    /// else to play — `DeviceSink` and its ring, behind the `device-output`
    /// feature — has anything to override this with.
    ///
    /// # Realtime contract
    ///
    /// Called from the same consumer/pump thread as [`Self::write`] and bound
    /// by the same rules: no allocation, no locking, no I/O, no panics. An
    /// implementation must also not *block* waiting for a real audio callback
    /// to confirm the discard — the callback may never run again.
    fn discard_buffered(&mut self) {}
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
///
/// It keeps the default [`Sink::discard_buffered`]: an offline sink holds no
/// pending audio, only the finished record of what was delivered, and that
/// record is what the bit-exactness tests measure. See the trait method's
/// docs for the full argument.
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

    /// The offline sink is a record, not a queue: a discard must not erase
    /// delivered history, or every bit-exactness comparison in the suite
    /// would quietly start measuring a truncated stream.
    #[test]
    fn discard_does_not_erase_the_offline_record() {
        let mut sink = OfflineSink::with_capacity(8);
        sink.write(&[1.0, 2.0, 3.0, 4.0]);
        sink.discard_buffered();
        sink.write(&[5.0, 6.0]);
        assert_eq!(sink.samples(), &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
    }
}
