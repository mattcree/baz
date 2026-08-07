//! Volume: the control unit, the taper that maps it to gain, and the fader
//! that applies it on the pump path.
//!
//! The design decision this module encodes is ADR-0010, and its two halves are
//! worth stating up front because both are easy to get quietly wrong.
//!
//! # The unit on the wire is a control position, not an amplitude
//!
//! [`Command::SetVolume`](crate::protocol::Command::SetVolume) carries an
//! integer **control position** in `0..=`[`MAX_POSITION`] — thousandths of a
//! fader's travel — and *this module* is where it becomes a linear gain.
//! Three consequences, deliberately chosen:
//!
//! 1. **The taper lives in `baz-core`, so every front end agrees.** A slider
//!    that reports linear amplitude feels wrong to a human: half-travel sounds
//!    far louder than half-loud, and the bottom third of the control does
//!    almost nothing. If each front end applied its own correction they would
//!    disagree about what "50 %" means, and a remote transport would disagree
//!    with the GUI. [`Volume::amplitude`] is the single answer.
//! 2. **The wire stays byte-pinnable.** `protocol`'s "Time on the wire"
//!    section makes the case for integers over `f64` in full — one canonical
//!    JSON encoding, and [`Command`](crate::protocol::Command) keeps its `Eq`.
//!    The same argument decided the seek unit, and the same argument decides
//!    this one; a float amplitude would have cost both.
//! 3. **1000 steps is finer than the control can be driven.** Over the taper's
//!    ~60 dB useful range that is ~0.06 dB per step at the top, which is two
//!    orders of magnitude below the ~1 dB a listener can hear as a change, and
//!    finer than any pointer gesture across a slider a few hundred pixels wide.
//!
//! # The taper is a cube, and unity is exact by construction
//!
//! `amplitude = (position / 1000)³`. This is the classical "60 dB fader law":
//! 10 % of travel is −60 dB, 50 % is −18.06 dB, and the control feels roughly
//! linear in loudness rather than in voltage.
//!
//! It was chosen over a dB-linear (exponential) taper for one structural
//! reason beyond feel: **a cube reaches exactly 0 and exactly 1, and reaches
//! them without a special case.** A dB-linear law approaches silence
//! asymptotically and so needs a hard-coded "and below this, actually zero"
//! branch at the bottom of the control — a hidden discontinuity in exactly the
//! place a listener drags to. More importantly at the other end:
//!
//! - `1000 / 1000` is `1.0f32` exactly (both operands are exactly
//!   representable and the quotient is exact), and `1.0 * 1.0 * 1.0` is `1.0`
//!   exactly. So **[`MAX_POSITION`] is unity, provably, on every platform** —
//!   not "unity to within a rounding error". That is what lets the engine
//!   recognise unity by an exact comparison and skip the multiply outright,
//!   which is what keeps ADR-0009's bit-perfect guarantee reachable.
//! - `0 / 1000` is `0.0`, cubed is `0.0`. Silence is a real position on the
//!   control, not a limit approached.
//!
//! [`Volume::decibels`] is provided so a front end can *display* dB without
//! re-deriving the taper.
//!
//! # Mute is not gain zero
//!
//! It is a separate command and a separate piece of state
//! ([`Command::SetMute`](crate::protocol::Command::SetMute)), even though
//! position 0 is already silent, because the two say different things:
//!
//! - Mute means *silence now, and restore what I had*. Encoding it as position
//!   0 would destroy the position it must restore, forcing every front end to
//!   keep a shadow copy — and two front ends attached to one engine would then
//!   disagree about what unmuting should do. Keeping it in the engine keeps one
//!   answer.
//! - Position 0 means *this is how loud I want it*, and survives a mute/unmute
//!   round trip like any other position.
//!
//! The engine folds them into one effective gain (`muted ? 0 : amplitude`), so
//! the pump path still reads a single number.
//!
//! # The fader: what runs on the pump path
//!
//! The `Fader` (crate-internal — a front end sets a volume, it does not drive a
//! fader) is the realtime half. Per pump *block* it takes one branch; per
//! *sample* it does one multiply and nothing else. It allocates nothing, locks
//! nothing, and cannot panic — `docs/ENGINEERING.md`, "the audio thread is
//! sacred". (The atomic load that carries a new target to it happens in the
//! engine's pump loop, once per block, just above.) Two properties earn their
//! own paragraphs:
//!
//! **Unity is a short circuit, not a multiply by one.** When the current and
//! target gains are both exactly `1.0` the fader reports itself transparent and
//! the engine hands the ring's slices to the sink *without copying them* — the
//! same code path, instruction for instruction, that existed before volume
//! control did. Bit-exactness at unity is therefore structural: there is no
//! arithmetic to be exact about. (`x * 1.0 == x` for every finite `x` anyway,
//! but it is not true for a signalling NaN, and "we multiply by one and it
//! happens to be a no-op" is a claim that needs re-checking every time the
//! code moves. "We do not multiply" does not.)
//!
//! **Changes are slewed, not stepped.** Jumping the gain between two pump
//! blocks puts a step discontinuity in the waveform, which is audible as a
//! click — zipper noise, when it happens repeatedly during a drag. The fader
//! moves at a constant slew rate of full scale per [`RAMP_MS`] milliseconds, so
//! a full-travel change completes in [`RAMP_MS`] and a smaller one sooner, and
//! the trajectory is monotonic by construction: each frame steps by a fixed
//! signed amount and is clamped at the target, so it can never overshoot,
//! reverse, or oscillate. The gain is per *frame*, applied identically to both
//! channels, so a ramp never pulls the stereo image sideways.
//!
//! The slew is skipped when nothing is audible — before a session starts, and
//! while paused. There is no discontinuity to hide in silence, and skipping it
//! is what makes "set the volume, then play" deliver a stream at exactly the
//! requested gain from its first sample.
//!
//! # A note on `clippy::float_cmp`
//!
//! It is allowed for this module, deliberately and only here. The lint's
//! ordinary advice — compare within an epsilon — would **destroy the property
//! this module exists to provide**: "is the gain exactly one?" is precisely the
//! question, because an epsilon-wide band around unity is a band in which baz
//! would multiply the samples while claiming it had not. The same goes for "has
//! the slew reached its target?", whose answer must be exact or the fader never
//! settles and the transparent path is never re-entered.
//!
//! Every comparison below is between values that are exactly representable and
//! produced by exact operations (the taper's endpoints; a clamped accumulator
//! against its own clamp bound), which is what makes exactness a fact rather
//! than a hope. The tests assert the same way and for the same reason.
#![allow(clippy::float_cmp)]

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use crate::playback::CHANNELS;
use crate::protocol::VolumePath;

/// The top of the control's travel, and the position at which the gain is
/// exactly unity — see the module docs for why exactness here is load-bearing.
pub const MAX_POSITION: u16 = 1000;

/// How long a full-travel volume change takes to complete, in milliseconds.
///
/// The fader moves at a constant slew rate of full scale per this many
/// milliseconds, so this is the *longest* a change can take; a change of a
/// tenth of the travel completes in a tenth of the time.
///
/// 20 ms is long enough that the step is spread over ~880 frames at 44.1 kHz —
/// far below the slew rate at which a gain change is heard as a click — and
/// short enough that the control still feels attached to the pointer (two
/// orders of magnitude below the ~100 ms at which a person stops experiencing
/// a control as immediate).
pub const RAMP_MS: u32 = 20;

/// A volume setting: a position on the control's travel, `0..=`[`MAX_POSITION`].
///
/// The mapping to a linear gain is [`Volume::amplitude`] and it is defined here
/// rather than in any front end, so that every client of the protocol means the
/// same thing by "half way up". See the module docs for the taper and why it is
/// a cube.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Volume(u16);

impl Volume {
    /// Unity: the gain is exactly `1.0` and the engine applies no arithmetic
    /// to the sample stream at all (module docs).
    pub const UNITY: Self = Self(MAX_POSITION);

    /// The bottom of the travel. Silent, and — unlike [`mute`][mute] — a
    /// position the listener chose and that survives a mute round trip.
    ///
    /// [mute]: crate::protocol::Command::SetMute
    pub const SILENT: Self = Self(0);

    /// A volume at `position`, clamped into `0..=`[`MAX_POSITION`].
    ///
    /// Clamping rather than rejecting: a front end that computes a position
    /// from a pointer coordinate will land one past the end on the last pixel,
    /// and the honest answer to "louder than the loudest" is the loudest.
    #[must_use]
    pub const fn new(position: u16) -> Self {
        Self(if position > MAX_POSITION {
            MAX_POSITION
        } else {
            position
        })
    }

    /// The position on the control's travel, `0..=`[`MAX_POSITION`].
    #[must_use]
    pub const fn position(self) -> u16 {
        self.0
    }

    /// The linear gain this position means: `(position / 1000)³`.
    ///
    /// Exactly `1.0` at [`MAX_POSITION`] and exactly `0.0` at 0, on every
    /// platform — the module docs give the argument, and
    /// `the_taper_hits_its_endpoints_exactly` pins it.
    #[must_use]
    pub fn amplitude(self) -> f32 {
        // Both operands are exactly representable and 1000/1000 is an exact
        // quotient, so the top of the travel is exactly 1.0 (module docs).
        let x = f32::from(self.0) / f32::from(MAX_POSITION);
        x * x * x
    }

    /// Whether this is [`Volume::UNITY`] — the position at which baz touches
    /// not one sample.
    #[must_use]
    pub const fn is_unity(self) -> bool {
        self.0 == MAX_POSITION
    }

    /// The gain in decibels relative to unity, for display. `None` at
    /// [`Volume::SILENT`], where the honest reading is −∞ rather than a very
    /// large negative number.
    ///
    /// Provided so a front end that wants to label the control in dB does not
    /// have to re-derive the taper and risk disagreeing with the engine.
    #[must_use]
    pub fn decibels(self) -> Option<f32> {
        (self.0 != 0).then(|| 20.0 * self.amplitude().log10())
    }
}

impl Default for Volume {
    /// Unity — a player that has never been told otherwise plays the file as
    /// it is (ADR-0009).
    fn default() -> Self {
        Self::UNITY
    }
}

/// Everything a front end can observe about the volume, as
/// [`EngineHandle::volume`](crate::engine::EngineHandle::volume) reports it.
///
/// The same three facts travel as
/// [`Event::VolumeChanged`](crate::protocol::Event::VolumeChanged); this is the
/// pull-side snapshot, for a front end that wants the state at start-up rather
/// than waiting for someone to change it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct VolumeState {
    /// Where the control sits.
    pub volume: Volume,
    /// Whether output is muted, independently of [`Self::volume`].
    pub muted: bool,
    /// Where the volume is being applied — and therefore whether the sample
    /// stream is still literally untouched. See [`VolumePath`].
    pub path: VolumePath,
}

/// The volume state shared between the engine thread (sole writer), the pump
/// path (reads the gain), and any [`EngineHandle`](crate::engine::EngineHandle)
/// (reads all of it, from any thread).
///
/// Atomics only, so nothing on the pump path ever waits: the gain is one
/// `AtomicU32` holding the f32's bit pattern, which is the wait-free way to
/// publish a float without a lock or a `Mutex<f32>`.
#[derive(Debug)]
pub(crate) struct SharedVolume {
    /// Control position, `0..=`[`MAX_POSITION`].
    position: AtomicU32,
    /// Mute, independent of the position.
    muted: AtomicBool,
    /// The **effective** linear gain — `muted ? 0.0 : volume.amplitude()`, or
    /// `1.0` when the device is carrying the volume itself — as raw f32 bits.
    /// This is the one value the pump path reads.
    gain_bits: AtomicU32,
    /// [`VolumePath`] as a discriminant, so the handle can report where the
    /// volume is applied without a lock.
    path: AtomicU32,
}

impl Default for SharedVolume {
    fn default() -> Self {
        Self {
            position: AtomicU32::new(u32::from(MAX_POSITION)),
            muted: AtomicBool::new(false),
            gain_bits: AtomicU32::new(1.0f32.to_bits()),
            path: AtomicU32::new(path_code(VolumePath::Unity)),
        }
    }
}

/// [`VolumePath`] → discriminant, for [`SharedVolume::path`]. An explicit,
/// exhaustive mapping rather than `as` on the enum: the codes are private, so
/// their only requirement is that this and [`path_from_code`] agree, and a new
/// variant should fail to compile here rather than silently pick a number.
const fn path_code(path: VolumePath) -> u32 {
    match path {
        VolumePath::Unity => 0,
        VolumePath::SoftwareGain => 1,
        VolumePath::DeviceAttenuator => 2,
    }
}

/// The inverse of [`path_code`].
const fn path_from_code(code: u32) -> VolumePath {
    match code {
        1 => VolumePath::SoftwareGain,
        2 => VolumePath::DeviceAttenuator,
        _ => VolumePath::Unity,
    }
}

impl SharedVolume {
    /// The effective gain the pump path should apply. One relaxed-acquire load;
    /// nothing synchronizes on it beyond the value itself.
    pub(crate) fn gain(&self) -> f32 {
        f32::from_bits(self.gain_bits.load(Ordering::Acquire))
    }

    /// Publish a new effective gain. Engine thread only.
    pub(crate) fn set_gain(&self, gain: f32) {
        self.gain_bits.store(gain.to_bits(), Ordering::Release);
    }

    /// The control position. Engine thread writes, anyone reads.
    pub(crate) fn volume(&self) -> Volume {
        Volume::new(u16::try_from(self.position.load(Ordering::Acquire)).unwrap_or(MAX_POSITION))
    }

    pub(crate) fn set_volume(&self, volume: Volume) {
        self.position
            .store(u32::from(volume.position()), Ordering::Release);
    }

    pub(crate) fn muted(&self) -> bool {
        self.muted.load(Ordering::Acquire)
    }

    pub(crate) fn set_muted(&self, muted: bool) {
        self.muted.store(muted, Ordering::Release);
    }

    pub(crate) fn path(&self) -> VolumePath {
        path_from_code(self.path.load(Ordering::Acquire))
    }

    pub(crate) fn set_path(&self, path: VolumePath) {
        self.path.store(path_code(path), Ordering::Release);
    }

    /// The whole observable state in one read — what
    /// [`EngineHandle::volume`](crate::engine::EngineHandle::volume) returns.
    ///
    /// The three fields are loaded independently, so a caller racing a change
    /// can see a position from before it and a path from after. That is
    /// acceptable and is not papered over: this is a status readout, the engine
    /// emits [`Event::VolumeChanged`](crate::protocol::Event::VolumeChanged) as
    /// the ordered account of every change, and a torn read here corrects
    /// itself on the next one.
    pub(crate) fn snapshot(&self) -> VolumeState {
        VolumeState {
            volume: self.volume(),
            muted: self.muted(),
            path: self.path(),
        }
    }
}

/// The realtime half: applies gain to a block of interleaved samples, slewing
/// toward a new target rather than stepping to it.
///
/// Lives on the engine (pump) thread and is plain state — the atomics above are
/// how a *target* reaches it, not where it keeps its own position. See the
/// module docs for the realtime contract it upholds and the two properties
/// (transparent unity, monotonic slew) it exists to guarantee.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Fader {
    /// The gain the last sample was multiplied by.
    current: f32,
    /// The gain being slewed toward.
    target: f32,
}

impl Default for Fader {
    fn default() -> Self {
        Self {
            current: 1.0,
            target: 1.0,
        }
    }
}

impl Fader {
    /// Aim at `target`, slewing there over the next blocks.
    pub(crate) fn aim(&mut self, target: f32) {
        self.target = target;
    }

    /// Go to `target` at once, with no slew.
    ///
    /// Correct exactly when nothing is audible — before a session's first
    /// sample, and while paused. A step in silence is not a click, and jumping
    /// is what makes "set the volume, then press play" deliver the requested
    /// gain from the very first sample rather than 20 ms later.
    pub(crate) fn jump(&mut self, target: f32) {
        self.current = target;
        self.target = target;
    }

    /// Whether the fader is at rest at exactly unity, and so has nothing to do.
    ///
    /// The engine tests this **once per block** and, when it is true, passes
    /// the ring's samples straight to the sink without copying or scaling them.
    /// Both ends of the ramp must be unity for that to be safe for the whole
    /// block, which is what makes the short circuit structural rather than a
    /// per-sample optimisation (module docs).
    pub(crate) fn is_transparent(self) -> bool {
        self.current == 1.0 && self.target == 1.0
    }

    /// The gain the next sample would be multiplied by. Tests read it; the
    /// audio path does not.
    #[cfg(test)]
    pub(crate) fn current(self) -> f32 {
        self.current
    }

    /// Gain per frame while slewing, at `rate` Hz.
    fn step(rate: u32) -> f32 {
        // Full scale per RAMP_MS: one integer-derived divisor, computed once
        // per block rather than per sample.
        #[allow(clippy::cast_precision_loss)] // frame counts are far below 2^24 here
        let frames = (rate * RAMP_MS / 1000).max(1) as f32;
        1.0 / frames
    }

    /// Scale a block of interleaved samples in place.
    ///
    /// In place, and over one contiguous block, on purpose: the ring the pump
    /// reads from can wrap at *any* sample offset, so scaling its two halves in
    /// separate calls would put the slew half a frame out of step across the
    /// join and pull the stereo image sideways for one frame. The caller copies
    /// both halves into its scratch first and hands the whole block here, which
    /// makes frame alignment a property of the buffer rather than of an
    /// invariant about where rings happen to wrap.
    ///
    /// # Realtime contract
    ///
    /// Called from the pump path. No allocation, no locking, no I/O, and no
    /// panic — every index is derived from `buf.len()`. Per sample the work is
    /// one multiply in place; the branch between the slewing and the steady
    /// portion of the block is taken once per block, never per sample.
    ///
    /// `rate` is the session's stream rate, which sets the slew step. A rate of
    /// zero means no audio has been delivered yet, so there is nothing to slew
    /// across and the fader lands on the target immediately.
    pub(crate) fn apply(&mut self, buf: &mut [f32], rate: u32) {
        if rate == 0 {
            self.current = self.target;
        }
        let n = buf.len();
        let mut i = 0usize;
        if self.current != self.target {
            let step = Self::step(rate);
            let delta = self.target - self.current;
            let signed = if delta > 0.0 { step } else { -step };
            let frames = n / CHANNELS;
            // Frames of slew left before the target is reached. Splitting the
            // block here is what keeps the per-sample work branch-free.
            let needed = delta.abs() / step;
            #[allow(clippy::cast_precision_loss)] // block frame counts are tiny
            let ramp_frames = if needed >= frames as f32 {
                frames
            } else {
                // `needed` is finite, non-negative and below `frames` here.
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let whole = needed.ceil() as usize;
                whole
            };
            for _ in 0..ramp_frames {
                // Clamped at the target every frame, so the trajectory is
                // monotonic and lands exactly on it — never past.
                self.current = if signed > 0.0 {
                    (self.current + signed).min(self.target)
                } else {
                    (self.current + signed).max(self.target)
                };
                let gain = self.current;
                // Whole frames only, so both channels always move together.
                for sample in buf.iter_mut().skip(i).take(CHANNELS) {
                    *sample *= gain;
                }
                i += CHANNELS;
            }
        }
        let gain = self.current;
        for sample in buf.iter_mut().skip(i) {
            *sample *= gain;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The whole bit-perfect guarantee rests on this: the top of the control is
    /// unity *exactly*, not to within a rounding error, so the engine can
    /// recognise it with `==` and skip the multiply.
    #[test]
    fn the_taper_hits_its_endpoints_exactly() {
        assert_eq!(Volume::new(MAX_POSITION).amplitude(), 1.0);
        assert_eq!(Volume::UNITY.amplitude(), 1.0);
        assert_eq!(Volume::new(0).amplitude(), 0.0);
        assert_eq!(Volume::SILENT.amplitude(), 0.0);
        assert!(Volume::UNITY.is_unity());
        assert!(!Volume::new(MAX_POSITION - 1).is_unity());
    }

    /// Every step of the control is louder than the one below it — no plateau a
    /// listener can drag across without hearing anything, and no reversal.
    #[test]
    fn the_taper_is_strictly_monotonic() {
        let mut previous = f32::NEG_INFINITY;
        for position in 0..=MAX_POSITION {
            let amplitude = Volume::new(position).amplitude();
            assert!(
                amplitude > previous,
                "position {position} is not louder than {}: {amplitude} <= {previous}",
                position - 1
            );
            assert!(
                (0.0..=1.0).contains(&amplitude),
                "out of range: {amplitude}"
            );
            previous = amplitude;
        }
    }

    /// The taper is the classical 60 dB fader law, and these are the numbers
    /// that phrase commits to: a tenth of the travel is a thousandth of the
    /// amplitude.
    #[test]
    fn the_taper_is_the_sixty_db_fader_law() {
        assert_eq!(Volume::new(500).amplitude(), 0.125);
        assert_eq!(Volume::new(100).amplitude(), 0.001);
        let db = Volume::new(100).decibels().expect("not silent");
        assert!(
            (db - -60.0).abs() < 0.01,
            "10 % of travel should be -60 dB: {db}"
        );
        let db = Volume::new(500).decibels().expect("not silent");
        assert!(
            (db - -18.06).abs() < 0.01,
            "half travel should be -18.06 dB: {db}"
        );
        assert_eq!(Volume::UNITY.decibels(), Some(0.0));
        assert_eq!(
            Volume::SILENT.decibels(),
            None,
            "silence is not a dB figure"
        );
    }

    #[test]
    fn positions_past_the_top_clamp_to_unity() {
        assert_eq!(Volume::new(u16::MAX), Volume::UNITY);
        assert_eq!(Volume::new(MAX_POSITION + 1).position(), MAX_POSITION);
        assert_eq!(Volume::default(), Volume::UNITY);
    }

    /// Unity is transparent and stays transparent; anything else is not.
    #[test]
    fn the_fader_is_transparent_only_at_rest_at_unity() {
        let mut fader = Fader::default();
        assert!(fader.is_transparent());
        fader.aim(0.5);
        assert!(
            !fader.is_transparent(),
            "a fader on its way away from unity is not transparent"
        );
        fader.jump(1.0);
        assert!(fader.is_transparent());
        fader.jump(0.5);
        assert!(!fader.is_transparent());
    }

    /// A jumped fader scales exactly, with no slew: `x * 0.5` for every sample,
    /// asserted as exact equality because f32 multiplication is deterministic.
    #[test]
    fn a_settled_fader_scales_exactly() {
        let mut fader = Fader::default();
        fader.jump(0.5);
        let input: Vec<f32> = (0..64u16).map(|n| f32::from(n) / 64.0 - 0.5).collect();
        let mut out = input.clone();
        fader.apply(&mut out, 44_100);
        for (got, want) in out.iter().zip(input.iter()) {
            assert_eq!(*got, *want * 0.5, "gain 0.5 must halve exactly");
        }
    }

    /// The slew is monotonic, never overshoots, and completes in exactly the
    /// documented time for a full-travel change.
    #[test]
    fn the_slew_is_monotonic_and_completes_on_time() {
        const RATE: u32 = 44_100;
        let expected_frames = (RATE * RAMP_MS / 1000) as usize;
        let mut fader = Fader::default();
        fader.aim(0.0);
        // A steady 1.0 signal, so the output *is* the gain trajectory.
        let mut out = vec![1.0f32; expected_frames * 2 * CHANNELS];
        fader.apply(&mut out, RATE);

        let trajectory: Vec<f32> = out.iter().step_by(CHANNELS).copied().collect();
        for pair in trajectory.windows(2) {
            assert!(
                pair[1] <= pair[0],
                "the slew reversed: {} then {}",
                pair[0],
                pair[1]
            );
            assert!(
                pair[1] >= 0.0,
                "the slew overshot below the target: {}",
                pair[1]
            );
        }
        let landed = trajectory
            .iter()
            .position(|g| *g == 0.0)
            .expect("the slew must reach its target");
        assert_eq!(
            landed + 1,
            expected_frames,
            "a full-travel change must take exactly RAMP_MS"
        );
        assert_eq!(fader.current(), 0.0);
        // And it stays there.
        assert!(trajectory[landed..].iter().all(|g| *g == 0.0));
    }

    /// Both channels of a frame get the same gain during a slew — otherwise a
    /// volume change would pull the stereo image sideways as it moved.
    #[test]
    fn the_slew_moves_both_channels_together() {
        let mut fader = Fader::default();
        fader.aim(0.0);
        let mut out = vec![1.0f32; 512 * CHANNELS];
        fader.apply(&mut out, 44_100);
        for frame in out.chunks_exact(CHANNELS) {
            assert_eq!(frame[0], frame[1], "channels drifted apart mid-slew");
        }
    }

    /// A slew that spans several blocks picks up exactly where it left off:
    /// the trajectory across the join is the same one a single block would
    /// have produced.
    #[test]
    fn the_slew_continues_across_blocks() {
        const RATE: u32 = 44_100;
        let mut split = Fader::default();
        split.aim(0.25);
        let mut joined = Vec::new();
        for _ in 0..8 {
            let mut out = vec![1.0f32; 128 * CHANNELS];
            split.apply(&mut out, RATE);
            joined.extend_from_slice(&out);
        }

        let mut whole = Fader::default();
        whole.aim(0.25);
        let mut long_out = vec![1.0f32; 8 * 128 * CHANNELS];
        whole.apply(&mut long_out, RATE);

        assert_eq!(
            joined, long_out,
            "the slew must not restart at a block edge"
        );
        assert_eq!(split.current(), whole.current());
    }

    /// Before a rate is known there is no audio to slew across, so the fader
    /// simply arrives.
    #[test]
    fn without_a_stream_rate_the_fader_lands_at_once() {
        let mut fader = Fader::default();
        fader.aim(0.25);
        let mut out = vec![1.0f32; 8 * CHANNELS];
        fader.apply(&mut out, 0);
        assert!(out.iter().all(|s| *s == 0.25));
    }

    #[test]
    fn shared_volume_round_trips_every_field() {
        let shared = SharedVolume::default();
        assert_eq!(
            shared.snapshot(),
            VolumeState {
                volume: Volume::UNITY,
                muted: false,
                path: VolumePath::Unity,
            }
        );
        shared.set_volume(Volume::new(250));
        shared.set_muted(true);
        shared.set_gain(0.0);
        shared.set_path(VolumePath::SoftwareGain);
        assert_eq!(
            shared.snapshot(),
            VolumeState {
                volume: Volume::new(250),
                muted: true,
                path: VolumePath::SoftwareGain,
            }
        );
        assert_eq!(shared.gain(), 0.0);
        shared.set_path(VolumePath::DeviceAttenuator);
        assert_eq!(shared.snapshot().path, VolumePath::DeviceAttenuator);
    }
}
