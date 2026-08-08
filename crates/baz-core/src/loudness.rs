//! EBU R128 loudness measurement, per ITU-R BS.1770-4.
//!
//! [`crate::replaygain`] reads the ReplayGain figures a file already carries.
//! This module *computes* them for a file that carries none: it is the meter,
//! and nothing else. It opens no files, writes nothing, and knows nothing about
//! libraries or sessions — [`crate::analysis`] drives it over decoded audio and
//! [`crate::index`] stores what it produces. That split is the same one
//! [`crate::replaygain`] makes and for the same reason: everything here is a
//! pure function of samples, which is what lets it be checked against the
//! standard's own published numbers rather than against itself.
//!
//! The governing decision is ADR-0015.
//!
//! # What is implemented, precisely
//!
//! **Gated integrated loudness** (BS.1770-4 §4, the "gating" clause EBU R128
//! adds): K-weight each channel, take the mean square over 400 ms blocks
//! overlapping by 75 %, sum the channels with their weights, drop blocks below
//! the **absolute** threshold of −70 LUFS, then drop blocks more than 10 LU
//! below the mean of what is left, and report the loudness of the mean of what
//! survives.
//!
//! **Sample peak**, not true peak. The `REPLAYGAIN_*_PEAK` convention is a
//! linear sample peak — it is what ReplayGain 2.0 scanners write and it is
//! exactly what [`ReplayGainSettings::resolve`](crate::replaygain::ReplayGainSettings::resolve)
//! clip-checks against — so a sample peak is the figure this unit needs and the
//! one it produces. Inter-sample overshoot after reconstruction is *not*
//! modelled, which ADR-0013 already said of the tag reader and which stays true
//! here. True peak means BS.1770-4 Annex 2's four-times oversampling filter and
//! its own compliance vectors; that is its own unit, and claiming a guarantee
//! this code does not implement would be worse than saying so.
//!
//! # Why the filter is derived rather than tabulated
//!
//! BS.1770-4 publishes the K-weighting coefficients **at 48 kHz only**, and a
//! music library is not at 48 kHz. The filter is therefore built from the
//! stage parameters the standard's own design is expressed in
//! (a centre frequency, a gain and a Q per stage) and evaluated at whatever rate the file
//! is stored at. `the_k_weighting_coefficients_match_the_published_table`
//! asserts that at 48 kHz this reproduces the standard's Tables 1 and 2 to
//! within 1e-12 — so the derivation is checked against the specification, not
//! against a recording of its own output.
//!
//! # Verification
//!
//! `tests/loudness.rs` generates the **EBU Tech 3341 compliance signals** and
//! asserts baz's reading lands inside the ±0.1 LU the specification states, at
//! 48 kHz *and* at 44.1 kHz. Cases 1–5 are the ones a stereo meter can express;
//! case 6 is 5.0-channel material, which baz's decode path (stereo, always)
//! cannot carry, and it is skipped for that stated reason rather than silently
//! omitted.

use crate::replaygain::{MAX_TAG_GAIN_CENTIDB, MAX_TAG_PEAK_MICRO, PEAK_UNITY};

/// The loudness ReplayGain normalises to, in LUFS: **−18**.
///
/// ReplayGain 2.0's reference. EBU R128 broadcast material aims at −23 LUFS
/// instead, which is the same five decibels
/// [`R128_REFERENCE_OFFSET_CENTIDB`](crate::replaygain::R128_REFERENCE_OFFSET_CENTIDB)
/// already adds when a file carries the R128 form of the tag — stated in one
/// place there and used in one place here, so the two cannot drift.
pub const REFERENCE_LUFS: f64 = -18.0;

/// The absolute gating threshold, in LUFS: **−70** (BS.1770-4).
///
/// Blocks quieter than this are silence for the purposes of a loudness
/// measurement, and averaging them in would drag a whole album's figure down
/// because it happens to have a long fade-out.
pub const ABSOLUTE_GATE_LUFS: f64 = -70.0;

/// The relative gating threshold, in LU below the ungated mean: **−10**
/// (EBU R128).
///
/// This is the gate that makes the measurement about the *programme* rather
/// than about how much quiet there is around it.
pub const RELATIVE_GATE_LU: f64 = -10.0;

/// The offset in the block-loudness formula, in LU: **−0.691** (BS.1770-4 §4).
///
/// It exists so that a 1 kHz tone measures the same number in LUFS as it does
/// in dBFS — the K-weighting has about +0.6977 dB of gain at 1 kHz, and this
/// takes it back out. That is exactly what EBU Tech 3341's first two test cases
/// check, which is why they are worth generating.
pub const LOUDNESS_OFFSET_LU: f64 = -0.691;

/// Length of a gating block, in milliseconds: **400** (BS.1770-4).
pub const BLOCK_MS: u32 = 400;

/// How many gating steps make one block: **4**, i.e. blocks overlap by 75 %
/// and a step is 100 ms.
pub const STEPS_PER_BLOCK: usize = 4;

/// Centre frequency of the K-weighting head-effect shelf, in Hz.
const SHELF_FREQUENCY_HZ: f64 = 1_681.974_450_955_533;
/// Gain of the K-weighting head-effect shelf, in decibels.
const SHELF_GAIN_DB: f64 = 3.999_843_853_973_347;
/// Q of the K-weighting head-effect shelf.
const SHELF_Q: f64 = 0.707_175_236_955_419_6;
/// The exponent relating the shelf's band gain to its high-frequency gain.
/// One of the two numbers that make the derivation reproduce BS.1770-4's
/// published 48 kHz table exactly rather than approximately.
const SHELF_BAND_EXPONENT: f64 = 0.499_666_774_154_541_6;
/// Corner frequency of the RLB high-pass, in Hz.
const RLB_FREQUENCY_HZ: f64 = 38.135_470_876_024_44;
/// Q of the RLB high-pass.
const RLB_Q: f64 = 0.500_327_037_323_877_3;

/// Channel weights `G_i` from BS.1770-4 for a stereo (or mono) signal: unity.
///
/// The standard gives 1.41 to the surround channels of a 5.1 layout and 0 to
/// LFE. baz's decode path is stereo, always ([`crate::playback::CHANNELS`]), so
/// every channel this meter ever sees is a unity-weighted one; the constant is
/// named rather than inlined so that the assumption is visible if a
/// multichannel path is ever added.
const CHANNEL_WEIGHT: f64 = 1.0;

/// A two-pole IIR section in direct form I, run in `f64`.
///
/// `f64` rather than the `f32` the samples arrive as: this is a
/// measurement, run once per file off any realtime path, and an accumulator
/// that loses precision over a five-minute track would be a worse trade than
/// the arithmetic costs.
#[derive(Clone, Copy, Debug, Default)]
struct Biquad {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    x1: f64,
    x2: f64,
    y1: f64,
    y2: f64,
}

impl Biquad {
    /// One sample through the section.
    fn run(&mut self, x: f64) -> f64 {
        let y = self.b0 * x + self.b1 * self.x1 + self.b2 * self.x2
            - self.a1 * self.y1
            - self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
}

/// The two K-weighting sections at one sample rate: the head-effect shelf and
/// the RLB high-pass, as coefficients rather than as state.
#[derive(Clone, Copy, Debug)]
struct KWeighting {
    shelf: Biquad,
    rlb: Biquad,
}

impl KWeighting {
    /// Derive both sections for `rate_hz`.
    ///
    /// Bilinear-transformed from the same stage parameters BS.1770-4's design
    /// is expressed in, so the 48 kHz case reproduces the standard's published
    /// tables and every other rate is the same filter at that rate rather than
    /// an interpolation of a table.
    fn new(rate_hz: f64) -> Self {
        // Head-effect shelf, in the Vh/Vb (band-gain) form: at 48 kHz this is
        // BS.1770-4 Table 1 to the last digit, which
        // `the_k_weighting_coefficients_match_the_published_table` asserts.
        let k = (std::f64::consts::PI * SHELF_FREQUENCY_HZ / rate_hz).tan();
        let vh = 10f64.powf(SHELF_GAIN_DB / 20.0);
        let vb = vh.powf(SHELF_BAND_EXPONENT);
        let a0 = 1.0 + k / SHELF_Q + k * k;
        let shelf = Biquad {
            b0: (vh + vb * k / SHELF_Q + k * k) / a0,
            b1: 2.0 * (k * k - vh) / a0,
            b2: (vh - vb * k / SHELF_Q + k * k) / a0,
            a1: 2.0 * (k * k - 1.0) / a0,
            a2: (1.0 - k / SHELF_Q + k * k) / a0,
            ..Biquad::default()
        };
        // RLB high-pass. Its numerator is exactly (1, −2, 1) — the standard
        // states it that way, normalised to unity gain well above the corner.
        let k = (std::f64::consts::PI * RLB_FREQUENCY_HZ / rate_hz).tan();
        let a0 = 1.0 + k / RLB_Q + k * k;
        let rlb = Biquad {
            b0: 1.0,
            b1: -2.0,
            b2: 1.0,
            a1: 2.0 * (k * k - 1.0) / a0,
            a2: (1.0 - k / RLB_Q + k * k) / a0,
            ..Biquad::default()
        };
        Self { shelf, rlb }
    }

    /// One sample through both sections.
    fn run(&mut self, x: f64) -> f64 {
        self.rlb.run(self.shelf.run(x))
    }
}

/// A streaming EBU R128 meter for one track.
///
/// Fed decoded samples in order ([`Self::push`]) and asked for the result once
/// ([`Self::finish`]). It holds one filter per channel plus a 400 ms window's
/// worth of running sums, so its memory is a constant independent of track
/// length — a ten-hour DJ set costs the same as a three-minute single, except
/// for the per-block record the album gate needs (8 bytes per 100 ms, i.e.
/// 288 KB for that ten hours).
#[derive(Clone, Debug)]
pub struct LoudnessMeter {
    filters: Vec<KWeighting>,
    /// Samples in one gating step (100 ms), per channel.
    step_frames: usize,
    /// Frames accumulated into the step now being filled.
    step_filled: usize,
    /// Sum of squares of the current step, per channel.
    step_sums: Vec<f64>,
    /// The last [`STEPS_PER_BLOCK`] completed steps, per channel, oldest
    /// first — the sliding window a 75 %-overlapped block is made of.
    window: Vec<[f64; STEPS_PER_BLOCK]>,
    /// Completed steps so far; a block exists once this reaches
    /// [`STEPS_PER_BLOCK`].
    steps_done: usize,
    /// Mean square of every complete block, channel-summed and weighted.
    blocks: Vec<f64>,
    /// Largest absolute sample seen, before any gain.
    peak: f32,
    /// Frames pushed so far.
    frames: u64,
}

impl LoudnessMeter {
    /// A meter for `channels`-interleaved audio at `rate_hz`.
    ///
    /// `None` for a rate or channel count that cannot describe audio (zero of
    /// either), which is the honest answer to a corrupt header rather than a
    /// panic on the analysis path.
    ///
    /// **Channel count is the source's, not the engine's.** baz decodes
    /// everything to stereo, duplicating a mono file into both channels — and a
    /// duplicated channel doubles the summed power, which would read 3.01 dB
    /// louder than the same recording measured as the mono it is. A caller with
    /// a mono source therefore builds a one-channel meter and feeds it one
    /// channel; `a_mono_track_measures_the_same_as_its_stereo_duplicate` pins
    /// that the two agree.
    #[must_use]
    pub fn new(rate_hz: u32, channels: usize) -> Option<Self> {
        if rate_hz == 0 || channels == 0 {
            return None;
        }
        let rate = f64::from(rate_hz);
        // One gating step — 100 ms — at this rate, rounded to nearest. The
        // block is four of these *by construction*, so its length is exact
        // rather than a second rounding of a 400 ms figure, and the overlap is
        // exactly 75 % at every sample rate including 44 100.
        #[expect(
            clippy::cast_precision_loss,
            reason = "STEPS_PER_BLOCK is the constant 4"
        )]
        let step_ms = f64::from(BLOCK_MS) / STEPS_PER_BLOCK as f64;
        let step_frames = (rate * step_ms / 1000.0).round().max(1.0);
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a tenth of a sample rate is positive and far inside usize"
        )]
        let step_frames = step_frames as usize;
        Some(Self {
            filters: vec![KWeighting::new(rate); channels],
            step_frames,
            step_filled: 0,
            step_sums: vec![0.0; channels],
            window: vec![[0.0; STEPS_PER_BLOCK]; channels],
            steps_done: 0,
            blocks: Vec::new(),
            peak: 0.0,
            frames: 0,
        })
    }

    /// How many channels this meter measures.
    #[must_use]
    pub fn channels(&self) -> usize {
        self.filters.len()
    }

    /// Feed interleaved samples — [`Self::channels`] per frame, in order.
    ///
    /// A trailing partial frame is ignored: half a frame is not a frame, and
    /// silently padding it with a zero would invent a sample the file does not
    /// contain.
    pub fn push(&mut self, interleaved: &[f32]) {
        let channels = self.filters.len();
        for frame in interleaved.chunks_exact(channels) {
            for (channel, &sample) in frame.iter().enumerate() {
                let magnitude = sample.abs();
                if magnitude > self.peak {
                    self.peak = magnitude;
                }
                // NaN and infinities cannot come from a decoder that produced
                // finite PCM, but they can come from a corrupt float WAV. They
                // are filtered to zero rather than allowed to poison every
                // later block through the filter's own state.
                let x = if sample.is_finite() {
                    f64::from(sample)
                } else {
                    0.0
                };
                let y = self.filters[channel].run(x);
                self.step_sums[channel] += y * y;
            }
            self.frames += 1;
            self.step_filled += 1;
            if self.step_filled == self.step_frames {
                self.close_step();
            }
        }
    }

    /// Complete the 100 ms step now being filled and, if a whole block has
    /// accumulated behind it, record that block's mean square.
    fn close_step(&mut self) {
        for (channel, sums) in self.window.iter_mut().enumerate() {
            sums.rotate_left(1);
            sums[STEPS_PER_BLOCK - 1] = self.step_sums[channel];
            self.step_sums[channel] = 0.0;
        }
        self.step_filled = 0;
        self.steps_done += 1;
        if self.steps_done < STEPS_PER_BLOCK {
            return;
        }
        #[expect(
            clippy::cast_precision_loss,
            reason = "a block length in frames is at most a few hundred thousand"
        )]
        let block_frames = (self.step_frames * STEPS_PER_BLOCK) as f64;
        let mean_square: f64 = self
            .window
            .iter()
            .map(|sums| CHANNEL_WEIGHT * sums.iter().sum::<f64>() / block_frames)
            .sum();
        self.blocks.push(mean_square);
    }

    /// The measurement. Consumes the meter: a track is measured once.
    ///
    /// The partially-filled trailing step is deliberately dropped rather than
    /// scaled up into a short block — BS.1770-4's blocks are 400 ms and a
    /// 130 ms one measured as if it were 400 ms would be a different, quieter
    /// number.
    #[must_use]
    pub fn finish(self) -> Loudness {
        Loudness {
            blocks: self.blocks,
            sample_peak: self.peak,
            frames: self.frames,
        }
    }
}

/// What one track measured: its gating blocks, and its sample peak.
///
/// The blocks are kept rather than reduced to a single number because an
/// **album** figure is the gated loudness of the album's blocks *as one set*,
/// not the average of its tracks' answers — the relative gate is computed
/// across the whole album, and averaging per-track results would apply it
/// per track instead. [`album_lufs`] is what puts them back together.
#[derive(Clone, Debug, PartialEq)]
pub struct Loudness {
    /// Mean square per 400 ms block, channel-summed and channel-weighted.
    blocks: Vec<f64>,
    /// Largest absolute sample in the track, linear.
    sample_peak: f32,
    /// Frames measured.
    frames: u64,
}

impl Loudness {
    /// Gated integrated loudness in LUFS, or `None` when the track holds no
    /// block above the absolute gate — a track shorter than one 400 ms block,
    /// or digital silence.
    ///
    /// `None` is a state the caller must handle rather than a zero to
    /// substitute: silence has no loudness, and a gain computed from a
    /// substituted number would be the loudest wrong answer this code could
    /// give.
    #[must_use]
    pub fn integrated_lufs(&self) -> Option<f64> {
        gated_lufs(&self.blocks)
    }

    /// Largest absolute sample in the track, linear — 1.0 is digital full
    /// scale, and values above it occur (lossy codecs overshoot).
    #[must_use]
    pub fn sample_peak(&self) -> f32 {
        self.sample_peak
    }

    /// Frames measured. Zero means nothing was pushed.
    #[must_use]
    pub fn frames(&self) -> u64 {
        self.frames
    }
}

/// The gated loudness of several tracks taken **as one programme** — an
/// album's figure.
///
/// The gates are applied across the pooled blocks, which is what makes this
/// different from (and not equal to) an average of the tracks' own answers: a
/// quiet interlude that its own track's relative gate would have kept can fall
/// below the *album's* relative gate, and that is the behaviour the standard
/// describes and that every other scanner implements.
///
/// `None` when no track offers a block above the absolute gate.
#[must_use]
pub fn album_lufs<'a>(tracks: impl IntoIterator<Item = &'a Loudness>) -> Option<f64> {
    let mut blocks: Vec<f64> = Vec::new();
    for track in tracks {
        blocks.extend_from_slice(&track.blocks);
    }
    gated_lufs(&blocks)
}

/// The largest sample peak across several tracks — an album's peak, which is
/// what album-mode clipping prevention checks against (ADR-0013 §3).
///
/// `None` for an empty album, which is not a thing that has a peak.
#[must_use]
pub fn album_sample_peak<'a>(tracks: impl IntoIterator<Item = &'a Loudness>) -> Option<f32> {
    tracks
        .into_iter()
        .map(Loudness::sample_peak)
        .fold(None, |acc: Option<f32>, peak| {
            Some(acc.map_or(peak, |best| best.max(peak)))
        })
}

/// The loudness of one block's mean square, in LUFS.
///
/// `-inf` for a silent block, which compares correctly against both gates and
/// is never `NaN` — the one value that would make the gate's `>` comparison
/// answer `false` in both directions and silently drop a block for the wrong
/// reason.
fn block_lufs(mean_square: f64) -> f64 {
    LOUDNESS_OFFSET_LU + 10.0 * mean_square.log10()
}

/// BS.1770-4's two-pass gate over a set of block mean squares.
///
/// Pass one drops everything below the absolute threshold and takes the mean
/// of the rest; pass two drops everything more than [`RELATIVE_GATE_LU`] below
/// *that*, and reports the loudness of what is left. Both passes filter the
/// original set, not the survivors of the previous one — the relative
/// threshold is derived from the absolutely-gated mean but applied alongside
/// the absolute gate, exactly as the standard writes it.
fn gated_lufs(blocks: &[f64]) -> Option<f64> {
    let mean_above = |threshold: f64| -> Option<f64> {
        let mut sum = 0.0;
        let mut count = 0u64;
        for &z in blocks {
            if block_lufs(z) > threshold {
                sum += z;
                count += 1;
            }
        }
        #[expect(
            clippy::cast_precision_loss,
            reason = "a block count is at most millions; f64 is exact to 2^53"
        )]
        let mean = (count > 0).then(|| sum / count as f64)?;
        Some(mean)
    };
    let ungated = mean_above(ABSOLUTE_GATE_LUFS)?;
    let relative = block_lufs(ungated) + RELATIVE_GATE_LU;
    let threshold = relative.max(ABSOLUTE_GATE_LUFS);
    let gated = mean_above(threshold)?;
    Some(block_lufs(gated))
}

/// The ReplayGain a measured loudness asks for, in centidecibels.
///
/// `REFERENCE_LUFS − measured`, rounded to the nearest centidecibel and
/// clamped into ±[`MAX_TAG_GAIN_CENTIDB`] — the same bound
/// [`parse_gain`](crate::replaygain::parse_gain) applies to a figure read from
/// a file, so a computed figure and a tagged one are the same kind of number
/// and cannot be told apart by their range.
///
/// Clamping rather than refusing: a −65 LUFS measurement is a real (very
/// quiet) recording, and the honest answer to "more gain than the unit can
/// express" is the most it can express. The clamp is not reachable by any
/// master anyone has pressed.
#[must_use]
pub fn gain_centidb(measured_lufs: f64) -> i16 {
    let centidb = ((REFERENCE_LUFS - measured_lufs) * 100.0).round();
    #[expect(
        clippy::cast_possible_truncation,
        reason = "clamped into the i16 tag-gain range immediately below"
    )]
    let clamped = centidb.clamp(
        f64::from(-MAX_TAG_GAIN_CENTIDB),
        f64::from(MAX_TAG_GAIN_CENTIDB),
    ) as i16;
    clamped
}

/// A linear sample peak as the micro-units the index and the wire store.
///
/// Clamped into `0..=`[`MAX_TAG_PEAK_MICRO`] and floored to zero for a
/// non-finite input, so the value is in the same range
/// [`parse_peak`](crate::replaygain::parse_peak) accepts from a file.
#[must_use]
pub fn peak_micro(peak: f32) -> u32 {
    let micro = (f64::from(peak) * f64::from(PEAK_UNITY)).round();
    if !micro.is_finite() || micro <= 0.0 {
        return 0;
    }
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "positive and clamped into the peak range immediately above"
    )]
    let clamped = micro.min(f64::from(MAX_TAG_PEAK_MICRO)) as u32;
    clamped
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The one place this module can be checked against the specification
    /// itself rather than against a signal: BS.1770-4 publishes the
    /// K-weighting coefficients at 48 kHz, and the derivation must reproduce
    /// them. Tolerance 1e-12 — the published table carries fourteen digits and
    /// the derivation reproduces all of them.
    #[test]
    fn the_k_weighting_coefficients_match_the_published_table() {
        let k = KWeighting::new(48_000.0);
        // BS.1770-4 Table 1 (stage 1, the head-effect shelf).
        let shelf = [
            (k.shelf.b0, 1.535_124_859_586_97),
            (k.shelf.b1, -2.691_696_189_406_38),
            (k.shelf.b2, 1.198_392_810_852_85),
            (k.shelf.a1, -1.690_659_293_182_41),
            (k.shelf.a2, 0.732_480_774_215_85),
        ];
        // BS.1770-4 Table 2 (stage 2, the RLB high-pass).
        let rlb = [
            (k.rlb.b0, 1.0),
            (k.rlb.b1, -2.0),
            (k.rlb.b2, 1.0),
            (k.rlb.a1, -1.990_047_454_833_98),
            (k.rlb.a2, 0.990_072_250_366_21),
        ];
        for (got, want) in shelf.into_iter().chain(rlb) {
            assert!(
                (got - want).abs() < 1e-12,
                "derived {got} but BS.1770-4 publishes {want}"
            );
        }
    }

    /// The −0.691 offset exists to cancel the K-weighting's gain at 1 kHz, so
    /// a 1 kHz tone reads the same number in LUFS as in dBFS. That is a
    /// property of the filter, and it is what makes EBU Tech 3341's first two
    /// cases the end-to-end check they are.
    #[test]
    fn the_offset_cancels_the_weighting_at_one_kilohertz() {
        let mut k = KWeighting::new(48_000.0);
        // Run a 1 kHz sine through and take the power gain, after enough
        // samples for the filter's transient to have passed.
        let mut sum = 0.0;
        let mut reference = 0.0;
        for i in 0..48_000 {
            let t = f64::from(i) / 48_000.0;
            let x = (std::f64::consts::TAU * 1000.0 * t).sin();
            let y = k.run(x);
            if i >= 4_800 {
                sum += y * y;
                reference += x * x;
            }
        }
        let gain_db = 10.0 * (sum / reference).log10();
        assert!(
            (gain_db + LOUDNESS_OFFSET_LU).abs() < 0.01,
            "K-weighting is {gain_db} dB at 1 kHz; the offset is {LOUDNESS_OFFSET_LU} LU"
        );
    }

    /// Silence has no loudness, and the meter says so instead of substituting
    /// a number that would become the loudest wrong gain it could produce.
    #[test]
    #[expect(clippy::float_cmp, reason = "a peak of exactly zero is the assertion")]
    fn silence_and_fragments_measure_nothing() {
        let mut meter = LoudnessMeter::new(48_000, 2).expect("meter");
        meter.push(&vec![0.0; 48_000 * 2]);
        let measured = meter.finish();
        assert_eq!(measured.integrated_lufs(), None);
        assert_eq!(measured.sample_peak(), 0.0);

        // Shorter than one gating block: no block, no measurement.
        let mut meter = LoudnessMeter::new(48_000, 2).expect("meter");
        meter.push(&vec![0.5; 2 * 2_000]);
        let measured = meter.finish();
        assert_eq!(measured.integrated_lufs(), None);
        assert_eq!(measured.frames(), 2_000);
        assert_eq!(measured.sample_peak(), 0.5);

        assert_eq!(LoudnessMeter::new(0, 2).map(|m| m.channels()), None);
        assert_eq!(LoudnessMeter::new(48_000, 0).map(|m| m.channels()), None);
    }

    /// A non-finite sample cannot poison the measurement.
    ///
    /// Decoded PCM is finite, but a float WAV is a file somebody else wrote and
    /// `NaN` is a legal `f32`. Fed straight into a recursive filter it would
    /// make every later block `NaN`, every comparison against a gate answer
    /// `false`, and the whole track's gain a number nobody chose — which is the
    /// shape of failure `docs/ENGINEERING.md` treats media parsers as hostile
    /// for. The meter reads it as silence instead, and keeps measuring.
    #[test]
    fn a_non_finite_sample_does_not_poison_the_measurement() {
        let mut clean = LoudnessMeter::new(48_000, 2).expect("meter");
        let mut poisoned = LoudnessMeter::new(48_000, 2).expect("meter");
        let mut signal: Vec<f32> = (0..48_000i32 * 2)
            .map(|i| {
                let t = f64::from(i / 2) / 48_000.0;
                #[expect(clippy::cast_possible_truncation, reason = "f64 sine -> f32")]
                let value = (0.5 * (std::f64::consts::TAU * 1_000.0 * t).sin()) as f32;
                value
            })
            .collect();
        clean.push(&signal);
        // One `NaN` and one infinity, three quarters of the way in.
        signal[36_000] = f32::NAN;
        signal[36_001] = f32::INFINITY;
        poisoned.push(&signal);

        let clean = clean.finish().integrated_lufs().expect("a measurement");
        let poisoned = poisoned.finish().integrated_lufs().expect("a measurement");
        assert!(poisoned.is_finite(), "{poisoned}");
        assert!(
            (clean - poisoned).abs() < 0.01,
            "two bad samples in a second of audio must not move the answer: \
             {clean} vs {poisoned}"
        );
    }

    /// A gain is the distance from the reference, and the two conversions are
    /// the only place a measurement becomes the integers the rest of baz
    /// speaks.
    #[test]
    fn the_conversions_are_the_reference_and_the_units() {
        assert_eq!(gain_centidb(REFERENCE_LUFS), 0);
        assert_eq!(gain_centidb(-23.0), 500);
        assert_eq!(gain_centidb(-8.25), -975);
        // Clamped, not wrapped, at the ends of the unit.
        assert_eq!(gain_centidb(-1_000.0), MAX_TAG_GAIN_CENTIDB);
        assert_eq!(gain_centidb(1_000.0), -MAX_TAG_GAIN_CENTIDB);

        assert_eq!(peak_micro(1.0), PEAK_UNITY);
        assert_eq!(peak_micro(0.988_525), 988_525);
        assert_eq!(peak_micro(0.0), 0);
        assert_eq!(peak_micro(-1.0), 0);
        assert_eq!(peak_micro(f32::NAN), 0);
        assert_eq!(peak_micro(1e9), MAX_TAG_PEAK_MICRO);
    }

    /// An album peak is the loudest sample anywhere in the album, which is
    /// exactly what album-mode clipping prevention needs (ADR-0013 §3).
    #[test]
    fn an_album_peak_is_the_loudest_of_its_tracks() {
        let track = |peak: f32| Loudness {
            blocks: Vec::new(),
            sample_peak: peak,
            frames: 0,
        };
        let tracks = [track(0.4), track(0.98), track(0.7)];
        assert_eq!(album_sample_peak(&tracks), Some(0.98));
        assert_eq!(album_sample_peak(std::iter::empty()), None);
    }
}
