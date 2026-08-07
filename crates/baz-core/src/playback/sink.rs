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

    /// Reconfigure the output to run at `desired` Hz, returning the rate it
    /// will **actually** run at.
    ///
    /// This is the ADR-0009 negotiation point: the engine asks for the rate of
    /// the audio it is about to play, and the sink answers with the rate it
    /// can deliver. When the two agree, samples reach the destination
    /// unconverted — the bit-perfect default. When they differ, the engine
    /// resamples and reports the chain as *not* bit-perfect
    /// ([`Event::SignalPath`](crate::protocol::Event::SignalPath)); it never
    /// converts silently.
    ///
    /// # Default: no rate of my own
    ///
    /// The default returns `None`, meaning "this sink has no fixed output rate
    /// — play at whatever rate you like". [`OfflineSink`] is exactly that: a
    /// record of delivered samples, which carries no clock. The engine reads
    /// `None` as "granted", so an offline session always runs at its source's
    /// native rate and never resamples. Only a sink bound to real hardware —
    /// `DeviceSink`, behind the `device-output` feature — has a rate to
    /// negotiate.
    ///
    /// # Contract
    ///
    /// Called on the engine's control/pump thread **between** pump iterations,
    /// at session start only, and never from a realtime callback. It may
    /// therefore block for as long as opening a device takes; it must not be
    /// called from [`Self::write`]'s caller mid-block. Reconfiguring discards
    /// whatever the sink had buffered, so the engine drains or discards first
    /// (see [`Self::drain_buffered`]).
    fn negotiate_rate(&mut self, _desired: u32) -> Option<u32> {
        None
    }

    /// Wait for audio this sink has already accepted to become audible, so
    /// that reconfiguring the output cannot cut it off.
    ///
    /// The mirror image of [`Self::discard_buffered`], and the engine calls it
    /// in exactly one place the discard is wrong: the boundary between two
    /// tracks of *different* sample rates, where the output stream is about to
    /// be reopened. The previous track's tail is audio the listener is still
    /// owed; throwing it away would turn a rate change into a truncation.
    ///
    /// # Contract
    ///
    /// Blocking is the point, so this is called only between pump iterations
    /// on the control thread, never from a realtime callback. An
    /// implementation **must** bound its wait: a stalled device that will
    /// never drain must not wedge the engine.
    ///
    /// The default is a no-op, correct for every sink whose `write` is its own
    /// destination.
    fn drain_buffered(&mut self) {}

    /// Ask the output to carry a linear `gain` in **its own** attenuator, so
    /// that baz does not have to scale the samples.
    ///
    /// This is ADR-0010's device-volume slot, and it is deliberately shaped
    /// like [`Self::negotiate_rate`]: the engine asks, the sink answers, and
    /// the engine reports honestly whichever answer it got. `Some(())` means
    /// the sink took the gain and the sample stream may be passed through
    /// untouched — the path is
    /// [`VolumePath::DeviceAttenuator`](crate::protocol::VolumePath::DeviceAttenuator)
    /// and remains bit-exact. `None` means the sink has no volume of its own,
    /// and the engine applies software gain and says so
    /// ([`VolumePath::SoftwareGain`](crate::protocol::VolumePath::SoftwareGain)).
    ///
    /// # Default: no volume of my own — and that is every sink baz ships
    ///
    /// The default returns `None`, which is the honest answer for
    /// [`OfflineSink`] (a `Vec<f32>` has no attenuator) **and** for
    /// `DeviceSink`. cpal exposes no volume API at all, so a hardware volume
    /// would mean platform-specific code — and ADR-0010's measurements found
    /// that in shared mode there is nothing correct for it to reach: the
    /// mixer behind a shared output belongs to the whole system, not to this
    /// player, and moving it would move every other application's volume with
    /// it. The trait method exists anyway, because the case where the answer
    /// changes is a known one (exclusive-mode output, where baz owns the card)
    /// and because a slot that a test double can fill is how the engine's half
    /// of the arrangement gets tested before the backend exists.
    ///
    /// # Contract
    ///
    /// Called on the engine's control thread **between** pump iterations,
    /// never from a realtime callback, so it may block for as long as talking
    /// to a mixer takes. It does not affect audio already buffered in the sink
    /// unless the underlying control does; and because
    /// [`Self::negotiate_rate`] may rebuild the output from scratch, the
    /// engine re-asks after every successful reconfiguration rather than
    /// assuming a gain survived it.
    fn set_device_volume(&mut self, _gain: f32) -> Option<()> {
        None
    }
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
/// It keeps every default the trait provides, and for the same reason in each
/// case — an offline sink is a record, not a queue in front of a clock:
///
/// - [`Sink::discard_buffered`]: it holds no pending audio, only the finished
///   record of what was delivered, and that record is what the bit-exactness
///   tests measure. Discarding could only mean deleting history.
/// - [`Sink::drain_buffered`]: nothing is in flight, so there is nothing to
///   wait for.
/// - [`Sink::negotiate_rate`]: a `Vec<f32>` has no sample rate, so every rate
///   is granted. An offline session therefore always runs at its source's
///   native rate and never resamples — which is what makes the headless suite
///   a test of the bit-perfect default rather than of a fallback.
/// - [`Sink::set_device_volume`]: a `Vec<f32>` has no attenuator either, so
///   volume is always applied in software above it. That is exactly what makes
///   the headless suite able to measure the gain: the delivered record *is*
///   the scaled stream, so "unity is bit-exact" and "half travel is exactly
///   0.125" are assertions about samples rather than about a setting.
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
