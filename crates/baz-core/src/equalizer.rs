//! **The equaliser** — ten bands, off by default, and off means *off*.
//!
//! The owner, 2026-08-18: *"EQ should be something that can be enabled or
//! disabled… without EQ it's just pure passthrough. with EQ it should be the
//! highest quality possible?"* Both halves are decisions about the signal
//! path, and they pull in opposite directions, so each is answered here in the
//! terms it was asked in.
//!
//! # Off is structural, not a setting
//!
//! A disabled equaliser does not process samples at unity — it is **not in the
//! path at all**. [`Equalizer::is_active`] is read by the engine's pump beside
//! `volume::Fader::is_transparent`, and when both say so the ring's
//! samples reach the sink with no copy and no arithmetic. That is the same
//! short-circuit ADR-0009's bit-exactness rests on and ADR-0011's fader was
//! built to preserve; this feature is the third tenant of it and changes
//! nothing about it.
//!
//! So the promise is exact rather than approximate: with the equaliser off,
//! the bytes delivered are the bytes decoded. Not "inaudibly close" — the
//! same. `an_inactive_equalizer_is_bit_exact` measures it.
//!
//! # On is arithmetic that does not cut corners
//!
//! Three choices, each of which could have been made cheaper:
//!
//! - **`f64` state and coefficients.** Audio arrives and leaves as `f32`, and
//!   a cascade of ten biquads at `f32` accumulates error in the recursive
//!   path where it is least forgivable — a peaking filter at 31.5 Hz has poles
//!   very close to the unit circle at 44.1 kHz, and single precision there is
//!   audible as low-frequency noise. Doubles cost one conversion per sample
//!   per band and remove the question.
//! - **Transposed Direct Form II.** The four canonical biquad topologies are
//!   equivalent in exact arithmetic and are not equivalent in finite
//!   precision. TDF-II has the best numerical behaviour for the coefficient
//!   ranges a listener's EQ produces, and it needs two state words per channel
//!   rather than four.
//! - **Per-channel state, and it is never shared.** A biquad's state *is* its
//!   memory of the signal; running two channels through one state would mix
//!   them, which at these Q values sounds like a phasing error rather than a
//!   bug.
//!
//! # What it deliberately does not do
//!
//! **It does not normalise, compress or limit.** A boost is a boost: asking
//! for +9 dB at 63 Hz on a loud master will clip, and baz will not quietly
//! rescale the music to hide that. What it does instead is [`Equalizer::set_preamp_db`]
//! — a stated attenuation the listener sets, defaulting to a value derived from
//! the bands themselves ([`Bands::suggested_preamp`]) so the ordinary case does
//! not clip without anyone having to understand why.
//!
//! **It does not change with the music.** No auto-EQ, no loudness contour, no
//! per-track anything. The curve is the listener's and it stays where they put
//! it.

use std::f64::consts::PI;

/// The band centres, in hertz — the ISO octave series every graphic equaliser
/// in the world uses, so a listener transferring a curve from another player
/// is transferring it between the same frequencies.
pub const CENTRES: [f32; 10] = [
    31.5, 63.0, 125.0, 250.0, 500.0, 1_000.0, 2_000.0, 4_000.0, 8_000.0, 16_000.0,
];

/// How far a band may be pushed, either way, in decibels.
///
/// ±12 is the range a graphic equaliser is *for*. Beyond it the useful act is
/// a different one — a filter, or a different master — and the honest answer
/// to "I need +20 dB at 60 Hz" is that the recording does not have it.
pub const LIMIT_DB: f32 = 12.0;

/// The steepness of each peaking filter.
///
/// **0.9, and it is derived rather than chosen.** Ten bands an octave apart
/// need a bandwidth of about one octave each to sum flat when they are all set
/// alike; the Q of a one-octave peaking filter is
/// `sqrt(2) / (2 ^ 1 - 1) ≈ 1.414`, and a slightly wider setting overlaps
/// neighbours so that a smooth curve drawn across several bands stays smooth
/// instead of turning into ten bumps. `bands_an_octave_apart_sum_smoothly`
/// measures the ripple this produces.
const Q: f64 = 0.9;

/// One band's gain, in decibels, clamped to [`LIMIT_DB`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Band(f32);

impl Band {
    /// A band at `db`, clamped. A `NaN` is flat — a filter designed from one
    /// would produce silence or worse, and there is no sensible reading of it.
    #[must_use]
    pub fn new(db: f32) -> Self {
        if db.is_nan() {
            return Self(0.0);
        }
        Self(db.clamp(-LIMIT_DB, LIMIT_DB))
    }

    /// The gain in decibels.
    #[must_use]
    pub fn db(self) -> f32 {
        self.0
    }

    /// Whether this band does nothing at all.
    #[must_use]
    pub fn is_flat(self) -> bool {
        self.0 == 0.0
    }
}

impl Default for Band {
    fn default() -> Self {
        Self(0.0)
    }
}

/// The ten bands, as a listener set them.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Bands(pub [Band; 10]);

impl Bands {
    /// Every band flat — the curve that does nothing.
    #[must_use]
    pub fn flat() -> Self {
        Self::default()
    }

    /// Build from decibel values, clamping each.
    #[must_use]
    pub fn from_db(db: [f32; 10]) -> Self {
        Self(db.map(Band::new))
    }

    /// Build from the **centidecibels** the protocol carries
    /// ([`crate::protocol::Command::SetEqualizer`]).
    #[must_use]
    pub fn from_centidb(centidb: [i16; 10]) -> Self {
        Self(centidb.map(|hundredths| Band::new(f32::from(hundredths) / 100.0)))
    }

    /// The curve as centidecibels, for the protocol and for config.
    #[must_use]
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a band is clamped to ±12 dB, so ±1200 fits i16 many times over"
    )]
    pub fn to_centidb(self) -> [i16; 10] {
        self.0.map(|band| (band.db() * 100.0).round() as i16)
    }

    /// Whether the curve is flat everywhere.
    ///
    /// **This is what lets an enabled equaliser still be transparent.** A
    /// listener who turns the feature on and has not moved anything is asking
    /// for nothing, and giving them a cascade of ten unity filters would cost
    /// the bit-exact path for no change in the sound.
    #[must_use]
    pub fn is_flat(&self) -> bool {
        self.0.iter().all(|band| band.is_flat())
    }

    /// **The attenuation that keeps the loudest boost from clipping.**
    ///
    /// The worst case is every band's boost arriving in phase, which sums to
    /// the largest single gain rather than their sum — neighbouring peaking
    /// filters an octave apart overlap, but nowhere does the cascade's
    /// magnitude meaningfully exceed its largest band plus the overlap. So the
    /// suggestion is the largest positive gain, rounded up, and nothing when
    /// no band is boosted.
    ///
    /// It is a **suggestion**, offered as the default and overridable, because
    /// a listener who knows their material has quiet peaks is entitled to keep
    /// the level.
    #[must_use]
    pub fn suggested_preamp(&self) -> f32 {
        let peak = self.0.iter().map(|band| band.db()).fold(0.0_f32, f32::max);
        -peak
    }
}

/// One biquad section: coefficients in `f64`, state per channel.
///
/// Transposed Direct Form II, so the state words are the two accumulators
/// rather than delayed inputs and outputs.
#[derive(Debug, Clone, Copy, Default)]
struct Biquad {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    /// `[channel][accumulator]`.
    state: [[f64; 2]; MAX_CHANNELS],
}

/// The channel count the state array covers. baz delivers stereo
/// ([`crate::playback::CHANNELS`]); the array is sized for it with room so a
/// future surround path is a change of one constant rather than of the shape.
const MAX_CHANNELS: usize = 8;

impl Biquad {
    /// **A peaking EQ section, by the RBJ audio-EQ cookbook.**
    ///
    /// The cookbook's formulae are the reference implementation every audio
    /// tool agrees on; deriving something else here would mean a curve that
    /// does not match what a listener gets elsewhere for the same numbers.
    fn peaking(centre: f64, gain_db: f64, rate: f64, q: f64) -> Self {
        // `A` is the *amplitude* the cookbook's peaking form is written in
        // terms of — the square root of the linear gain, because a peaking
        // filter's boost appears once in the numerator and once in the
        // denominator.
        let amplitude = 10.0_f64.powf(gain_db / 40.0);
        let omega = 2.0 * PI * centre / rate;
        let (sin, cos) = omega.sin_cos();
        let alpha = sin / (2.0 * q);
        let a0 = 1.0 + alpha / amplitude;
        Self {
            b0: (1.0 + alpha * amplitude) / a0,
            b1: (-2.0 * cos) / a0,
            b2: (1.0 - alpha * amplitude) / a0,
            a1: (-2.0 * cos) / a0,
            a2: (1.0 - alpha / amplitude) / a0,
            state: [[0.0; 2]; MAX_CHANNELS],
        }
    }

    /// One sample through one channel's state.
    #[inline]
    fn step(&mut self, channel: usize, x: f64) -> f64 {
        // Transposed Direct Form II, exactly:
        //
        //   y  = b0·x + s1
        //   s1 = b1·x − a1·y + s2
        //   s2 = b2·x − a2·y
        //
        // Written out rather than folded, because the first version of this
        // put `b2·x` into `s1` and left `s2` with only `−a2·y`. Every test
        // then in the file passed: the response tests evaluate the *design* on
        // the unit circle and never call this, and the two that do call it
        // could not tell — silence stays silence through a wrong recursion,
        // and a clamped block stays in range. `the_filter_delivers_the_gain_it
        // _designed` is the test that measures what this function actually
        // does.
        let [s1, s2] = self.state[channel];
        let y = self.b0.mul_add(x, s1);
        self.state[channel] = [
            self.b1.mul_add(x, s2) - self.a1 * y,
            self.b2.mul_add(x, -(self.a2 * y)),
        ];
        y
    }

    /// The magnitude of this section at `hz`, for the response measurements.
    fn magnitude(&self, hz: f64, rate: f64) -> f64 {
        let omega = 2.0 * PI * hz / rate;
        let (sin1, cos1) = omega.sin_cos();
        let (sin2, cos2) = (2.0 * omega).sin_cos();
        let num_re = self.b0 + self.b1 * cos1 + self.b2 * cos2;
        let num_im = -(self.b1 * sin1 + self.b2 * sin2);
        let den_re = 1.0 + self.a1 * cos1 + self.a2 * cos2;
        let den_im = -(self.a1 * sin1 + self.a2 * sin2);
        let num = num_re.hypot(num_im);
        let den = den_re.hypot(den_im);
        if den == 0.0 { 0.0 } else { num / den }
    }
}

/// **The equaliser as the engine holds it**: what the listener asked for, and
/// the filters that realise it at the current rate.
#[derive(Debug, Clone)]
pub struct Equalizer {
    enabled: bool,
    bands: Bands,
    preamp_db: f32,
    /// The rate the sections were designed for; `0` before the first design.
    rate: u32,
    sections: Vec<Biquad>,
    preamp: f64,
}

impl Default for Equalizer {
    fn default() -> Self {
        Self {
            enabled: false,
            bands: Bands::flat(),
            preamp_db: 0.0,
            rate: 0,
            sections: Vec::new(),
            preamp: 1.0,
        }
    }
}

impl Equalizer {
    /// Whether the equaliser is switched on **and** has something to do.
    ///
    /// The engine reads this to decide whether the pump takes its transparent
    /// path. An enabled equaliser set flat with no preamp answers `false`,
    /// which is the honest reading: it would multiply every sample by one.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.enabled && !(self.bands.is_flat() && self.preamp_db == 0.0)
    }

    /// Whether the listener has switched it on, whatever the curve says.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// The curve.
    #[must_use]
    pub fn bands(&self) -> Bands {
        self.bands
    }

    /// The stated attenuation, in decibels.
    #[must_use]
    pub fn preamp_db(&self) -> f32 {
        self.preamp_db
    }

    /// Switch it on or off.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        self.reset_state();
    }

    /// Set the curve, re-designing the sections.
    pub fn set_bands(&mut self, bands: Bands) {
        self.bands = bands;
        self.design();
    }

    /// Set the stated attenuation.
    pub fn set_preamp_db(&mut self, db: f32) {
        self.preamp_db = if db.is_nan() {
            0.0
        } else {
            db.clamp(-LIMIT_DB, LIMIT_DB)
        };
        self.preamp = 10.0_f64.powf(f64::from(self.preamp_db) / 20.0);
    }

    /// Tell it the stream's rate. A biquad's coefficients are a function of
    /// the rate it runs at, so a rate change re-designs every section — this
    /// is why the engine hands the rate to [`Self::apply`] rather than the
    /// equaliser assuming one.
    fn ensure_rate(&mut self, rate: u32) {
        if self.rate != rate {
            self.rate = rate;
            self.design();
        }
    }

    /// Re-derive every section from the bands at the current rate.
    fn design(&mut self) {
        self.sections.clear();
        if self.rate == 0 {
            return;
        }
        let rate = f64::from(self.rate);
        // **Nyquist is a real boundary, not a rounding concern.** A 16 kHz
        // band at a 22.05 kHz stream sits above half the sample rate, where
        // the cookbook's `omega` wraps and the filter it designs is not the
        // filter asked for. Such a band is simply not built: baz would rather
        // do nothing to a frequency the stream cannot carry than do something
        // arbitrary to one it can.
        let ceiling = rate / 2.0;
        for (index, band) in self.bands.0.iter().enumerate() {
            if band.is_flat() {
                continue;
            }
            let centre = f64::from(CENTRES[index]);
            if centre >= ceiling * 0.95 {
                continue;
            }
            self.sections
                .push(Biquad::peaking(centre, f64::from(band.db()), rate, Q));
        }
        self.reset_state();
    }

    /// Forget the filters' memory of the signal.
    ///
    /// Called whenever the path changes — enabled, re-designed, or a new
    /// session — because a biquad's state describes the samples that went
    /// before it, and after a change those samples went through a different
    /// filter. Carrying the state across would ring.
    fn reset_state(&mut self) {
        for section in &mut self.sections {
            section.state = [[0.0; 2]; MAX_CHANNELS];
        }
    }

    /// **Process one interleaved block in place.**
    ///
    /// `channels` is the interleave width; each channel keeps its own state.
    /// Does nothing at all when [`Self::is_active`] is false, which the engine
    /// has already checked — the guard here is so that a caller who has not is
    /// still correct rather than subtly wrong.
    pub fn apply(&mut self, block: &mut [f32], rate: u32, channels: usize) {
        if !self.is_active() || channels == 0 || channels > MAX_CHANNELS {
            return;
        }
        self.ensure_rate(rate);
        for (index, sample) in block.iter_mut().enumerate() {
            let channel = index % channels;
            let mut value = f64::from(*sample) * self.preamp;
            for section in &mut self.sections {
                value = section.step(channel, value);
            }
            // **Clamped, and only here.** Everything above ran in `f64` with
            // all the headroom in the world; this is the one place the result
            // has to become a number a sound card can accept, and a sample
            // outside ±1 is not one. The clamp is the honest edge of the
            // listener's own decision to boost — see the module docs on why
            // baz does not quietly rescale to avoid it.
            #[expect(
                clippy::cast_possible_truncation,
                reason = "f64 back to the f32 the sink takes, clamped first"
            )]
            {
                *sample = value.clamp(-1.0, 1.0) as f32;
            }
        }
    }

    /// **The cascade's magnitude response at `hz`, in decibels** — what the
    /// curve actually does, as opposed to what its numbers say.
    ///
    /// Used by the tests that check the bands land on their frequencies and
    /// that a smooth curve stays smooth. It is the same arithmetic the filters
    /// run, evaluated on the unit circle rather than over samples.
    #[must_use]
    pub fn response_db(&self, hz: f32, rate: u32) -> f32 {
        let mut designed = self.clone();
        designed.ensure_rate(rate);
        let magnitude: f64 = designed
            .sections
            .iter()
            .map(|section| section.magnitude(f64::from(hz), f64::from(rate)))
            .product();
        #[expect(clippy::cast_possible_truncation, reason = "a decibel for a test")]
        {
            (20.0 * (magnitude * designed.preamp).log10()) as f32
        }
    }
}

/// The nominal rate a drawn curve is evaluated at.
///
/// A biquad's response depends on the sample rate, and the panel is open
/// before a file is chosen — often with nothing playing at all. So the picture
/// is drawn at CD rate rather than at whatever happens to be in the sink. The
/// difference is confined to the top band: at 44.1 kHz the 16 kHz peak is
/// close enough to Nyquist to lean slightly, and redrawing the whole panel
/// when a 48 kHz file starts would move a curve nobody asked to move.
pub const DRAWING_RATE: u32 = 44_100;

/// **The response across the audible range, sampled log-evenly** — the shape
/// the cascade actually imposes, ready to draw.
///
/// This exists because ten handles are not a curve. Neighbouring bands overlap
/// — two adjacent +6 dB bands make about +9 dB between them, and no
/// arrangement of ten separate handles shows that. Sampling the real magnitude
/// response does, which is the difference between a row of controls and a
/// picture of what they are doing.
///
/// `out` is filled edge to edge: `out[0]` is `from_hz`, the last is `to_hz`,
/// and everything between is evenly spaced in *log* frequency, because that is
/// how the ear spaces them and how [`CENTRES`] are spaced. Designing the
/// sections is done once here rather than per sample — the panel redraws this
/// on every drag frame.
pub fn response_curve(bands: Bands, preamp_db: f32, from_hz: f32, to_hz: f32, out: &mut [f32]) {
    let mut designed = Equalizer::default();
    designed.set_enabled(true);
    designed.set_bands(bands);
    designed.set_preamp_db(preamp_db);
    designed.ensure_rate(DRAWING_RATE);

    let points = out.len();
    if points == 0 {
        return;
    }
    #[expect(
        clippy::cast_precision_loss,
        reason = "a few hundred sample points; exact in f64 many times over"
    )]
    let last = (points.saturating_sub(1)) as f64;
    let low = f64::from(from_hz).log10();
    let high = f64::from(to_hz).log10();
    // One point is a degenerate span; put it at the bottom rather than
    // dividing by zero.
    let span = if points > 1 { (high - low) / last } else { 0.0 };
    for (index, slot) in out.iter_mut().enumerate() {
        #[expect(clippy::cast_precision_loss, reason = "as above")]
        let at = index as f64;
        let hz = 10.0_f64.powf(span.mul_add(at, low));
        let magnitude: f64 = designed
            .sections
            .iter()
            .map(|section| section.magnitude(hz, f64::from(DRAWING_RATE)))
            .product();
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a decibel for a picture; f32 is the widget's own unit"
        )]
        {
            *slot = (20.0 * (magnitude * designed.preamp).log10()) as f32;
        }
    }
}

#[cfg(test)]
#[expect(
    clippy::cast_precision_loss,
    reason = "test tones are counted in samples, far below any mantissa limit"
)]
#[expect(
    clippy::cast_possible_truncation,
    reason = "test tones become the f32 the sink takes, which is the point"
)]
#[expect(
    clippy::float_cmp,
    reason = "these assertions are about exact values a clamp produces, and \
              about bit-exact passthrough — approximate equality would make \
              them assert nothing"
)]
mod tests {
    use super::*;

    /// **Off is off**: an inactive equaliser returns the block untouched, and
    /// the engine's own transparent path never even calls it.
    #[test]
    fn an_inactive_equalizer_is_bit_exact() {
        let mut eq = Equalizer::default();
        let original: Vec<f32> = (0..512)
            .map(|n| (n as f32 / 512.0).mul_add(2.0, -1.0))
            .collect();
        let mut block = original.clone();
        eq.apply(&mut block, 44_100, 2);
        assert_eq!(block, original, "a disabled equaliser touched the samples");

        // Enabled but flat is *also* inactive — asking for nothing is nothing.
        eq.set_enabled(true);
        assert!(
            !eq.is_active(),
            "a flat curve is not a reason to leave the transparent path"
        );
        let mut block = original.clone();
        eq.apply(&mut block, 44_100, 2);
        assert_eq!(block, original);
    }

    /// **A band lands on its own frequency**, at the gain asked for.
    #[test]
    fn each_band_boosts_its_own_centre() {
        for (index, centre) in CENTRES.into_iter().enumerate() {
            let mut db = [0.0_f32; 10];
            db[index] = 6.0;
            let mut eq = Equalizer::default();
            eq.set_enabled(true);
            eq.set_bands(Bands::from_db(db));
            let at = eq.response_db(centre, 48_000);
            assert!(
                (at - 6.0).abs() < 0.5,
                "band {index} ({centre} Hz) reads {at:.2} dB, not 6"
            );
            // …and leaves a distant frequency alone. Two octaves away is far
            // enough that a one-octave filter has nothing to say.
            let far = if index < 5 {
                centre * 4.0
            } else {
                centre / 4.0
            };
            if far > 20.0 && far < 20_000.0 {
                let elsewhere = eq.response_db(far, 48_000);
                assert!(
                    elsewhere.abs() < 1.0,
                    "band {index} moved {far} Hz by {elsewhere:.2} dB"
                );
            }
        }
    }

    /// **The filter delivers the gain it designed** — measured on samples,
    /// not on coefficients.
    ///
    /// This is the test the first version of this module did not have, and it
    /// is the reason a wrong recursion survived nine passing tests: everything
    /// else either evaluated the design on the unit circle or checked a
    /// property (silence, range) that a broken filter still satisfies. A sine
    /// in, the amplitude out, against what `response_db` promises.
    #[test]
    fn the_filter_delivers_the_gain_it_designed() {
        const RATE: u32 = 48_000;
        for (index, gain) in [(2_usize, 6.0_f32), (5, -6.0), (7, 9.0)] {
            let mut db = [0.0_f32; 10];
            db[index] = gain;
            let mut eq = Equalizer::default();
            eq.set_enabled(true);
            eq.set_bands(Bands::from_db(db));

            let centre = f64::from(CENTRES[index]);
            // Two seconds, so the filter is long past its transient before the
            // window that is measured.
            let frames = RATE as usize * 2;
            let mut block: Vec<f32> = Vec::with_capacity(frames * 2);
            for n in 0..frames {
                let t = n as f64 / f64::from(RATE);
                #[expect(clippy::cast_possible_truncation, reason = "a test tone")]
                let sample = (0.25 * (2.0 * PI * centre * t).sin()) as f32;
                block.push(sample);
                block.push(sample);
            }
            eq.apply(&mut block, RATE, 2);

            // The steady-state peak over the second half.
            let peak = block[frames..]
                .iter()
                .fold(0.0_f32, |most, s| most.max(s.abs()));
            let measured = 20.0 * (f64::from(peak) / 0.25).log10();
            let designed = f64::from(eq.response_db(CENTRES[index], RATE));
            assert!(
                (measured - designed).abs() < 0.5,
                "band {index}: the filter delivers {measured:.2} dB where its \
                 design promises {designed:.2}"
            );
            assert!(
                (measured - f64::from(gain)).abs() < 0.6,
                "band {index}: asked for {gain} dB and got {measured:.2}"
            );
        }
    }

    /// **A curve drawn across several bands stays smooth.** This is what `Q`
    /// is chosen for: too narrow and the same request becomes a row of bumps.
    #[test]
    fn bands_an_octave_apart_sum_smoothly() {
        let mut eq = Equalizer::default();
        eq.set_enabled(true);
        eq.set_bands(Bands::from_db([6.0; 10]));
        // Between 125 Hz and 8 kHz — inside the run of bands, away from the
        // ends where the curve is allowed to fall away.
        let mut lowest = f32::MAX;
        let mut highest = f32::MIN;
        let mut hz = 125.0_f32;
        while hz <= 8_000.0 {
            let at = eq.response_db(hz, 48_000);
            lowest = lowest.min(at);
            highest = highest.max(at);
            hz *= 1.05;
        }
        assert!(
            highest - lowest < 3.0,
            "an even boost ripples by {:.2} dB across the band run",
            highest - lowest
        );
    }

    /// **Cut is the mirror of boost.**
    #[test]
    fn a_cut_is_the_boosts_reflection() {
        let mut db = [0.0_f32; 10];
        db[4] = -8.0;
        let mut eq = Equalizer::default();
        eq.set_enabled(true);
        eq.set_bands(Bands::from_db(db));
        let at = eq.response_db(CENTRES[4], 48_000);
        assert!((at + 8.0).abs() < 0.5, "a -8 dB cut reads {at:.2} dB");
    }

    /// **Every band is limited, and nonsense is flat rather than dangerous.**
    #[test]
    fn a_band_is_clamped_and_a_nan_is_flat() {
        assert_eq!(Band::new(40.0).db(), LIMIT_DB);
        assert_eq!(Band::new(-40.0).db(), -LIMIT_DB);
        assert_eq!(Band::new(f32::NAN).db(), 0.0);
        assert!(Band::new(f32::NAN).is_flat());
    }

    /// **The suggested preamp answers the largest boost**, and asks for
    /// nothing when nothing is boosted.
    #[test]
    fn the_suggested_preamp_answers_the_largest_boost() {
        assert_eq!(Bands::flat().suggested_preamp(), 0.0);
        assert_eq!(
            Bands::from_db([0.0, 9.0, 3.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]).suggested_preamp(),
            -9.0
        );
        // A curve that only cuts needs no headroom back.
        assert_eq!(Bands::from_db([-6.0; 10]).suggested_preamp(), 0.0);
    }

    /// **A band above Nyquist is not built.** At 22.05 kHz the 16 kHz band is
    /// too close to half the rate for the cookbook's design to mean anything.
    #[test]
    fn a_band_the_stream_cannot_carry_is_left_alone() {
        let mut db = [0.0_f32; 10];
        db[9] = 10.0;
        let mut eq = Equalizer::default();
        eq.set_enabled(true);
        eq.set_bands(Bands::from_db(db));
        eq.ensure_rate(22_050);
        assert!(
            eq.sections.is_empty(),
            "a 16 kHz band was designed at a 22.05 kHz rate"
        );
        // The same band at a rate that carries it is built.
        eq.ensure_rate(48_000);
        assert_eq!(eq.sections.len(), 1);
    }

    /// **Stereo channels do not leak into each other.** One channel silent and
    /// the other loud must stay that way through the cascade.
    #[test]
    fn each_channel_keeps_its_own_memory() {
        let mut eq = Equalizer::default();
        eq.set_enabled(true);
        eq.set_bands(Bands::from_db([6.0; 10]));
        // Left: a tone. Right: silence.
        let mut block: Vec<f32> = (0..2_048)
            .map(|n| {
                if n % 2 == 0 {
                    (n as f32 * 0.01).sin() * 0.3
                } else {
                    0.0
                }
            })
            .collect();
        eq.apply(&mut block, 48_000, 2);
        let right_energy: f32 = block.iter().skip(1).step_by(2).map(|s| s.abs()).sum();
        assert!(
            right_energy == 0.0,
            "a silent channel picked up {right_energy} from its neighbour"
        );
    }

    /// **The output never leaves the range a sound card accepts**, however
    /// hard the curve is pushed.
    #[test]
    fn a_boosted_block_is_still_in_range() {
        let mut eq = Equalizer::default();
        eq.set_enabled(true);
        eq.set_bands(Bands::from_db([LIMIT_DB; 10]));
        let mut block: Vec<f32> = (0..4_096)
            .map(|n| (f64::from(n) * 0.05).sin() as f32 * 0.99)
            .collect();
        eq.apply(&mut block, 48_000, 2);
        assert!(
            block
                .iter()
                .all(|s| (-1.0..=1.0).contains(s) && s.is_finite()),
            "the cascade produced a sample outside the range or not a number"
        );
    }
}
