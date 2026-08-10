//! The stereo downmix: ITU-R BS.775, written down.
//!
//! A 5.1 record is a record, and until this module existed baz refused to play
//! one. The refusal was the honest answer to a real difficulty — a downmix that
//! guesses which channel is the centre sounds subtly wrong in a way no length
//! check catches — and this module is that difficulty answered rather than
//! avoided. ADR-0039 carries the full argument; what follows is the matrix and
//! the two decisions that are not in the matrix.
//!
//! # The matrix
//!
//! **ITU-R BS.775, "Multichannel stereophonic sound system with and without
//! accompanying picture"**, gives the two-channel downmix of a 3/2 (five main
//! channel) programme as
//!
//! ```text
//! Lo = L + kC·C + kS·Ls
//! Ro = R + kC·C + kS·Rs        with kC = kS = 1/√2  (−3 dB)
//! ```
//!
//! Written per speaker, which is how [`Downmix`] applies it, that is:
//!
//! | speaker | → Lo | → Ro | why |
//! |---|---|---|---|
//! | front left | 1 | 0 | it *is* the left channel |
//! | front right | 0 | 1 | it *is* the right channel |
//! | front centre | 1/√2 | 1/√2 | BS.775 `kC`: the phantom centre two speakers make together |
//! | rear/side left | 1/√2 | 0 | BS.775 `kS`, folded into its own side |
//! | rear/side right | 0 | 1/√2 | ditto |
//! | LFE | 0 | 0 | see below |
//!
//! Nothing here is invented, and the three numbers are checkable against a
//! second implementation without reading a standards document: ffmpeg's
//! libswresample calls them `center_mix_level` and `surround_mix_level` and
//! defaults both to **0.707107**, which is the same 1/√2.
//!
//! # Decision 1: the LFE is dropped, not folded
//!
//! **BS.775's downmix equations contain no LFE term**, and baz does not add
//! one. The LFE is a band-limited effects channel, mixed at +10 dB relative to
//! the main channels by the convention the same recommendation sets out; folding
//! it into a stereo pair at any level puts subsonic energy into a signal that
//! two loudspeakers will try to reproduce, at a level the mix engineer never
//! auditioned. Where a standard *does* offer the option — ATSC A/52's
//! `lfemixlevcod` — it is optional and off by default, and libswresample's
//! `lfe_mix_level` defaults to **0** for the same reason.
//!
//! In a competently mastered 5.1 music release the main channels already carry
//! full-range bass; the LFE holds what is *additional*. Dropping it is what a
//! stereo listener of that release is supposed to hear.
//!
//! # Decision 2: headroom is taken by constant attenuation, not by limiting
//!
//! The matrix overflows. Summing L, C and Ls into Lo has a worst-case gain of
//! `1 + 1/√2 + 1/√2 = 2.4142` — **+7.66 dB** — reached by any signal that is
//! full-scale and correlated across those three channels, which is neither
//! exotic nor rare in a loud passage of a 5.1 mix.
//!
//! Three answers were available, and the choice is stated here because it is
//! audible:
//!
//! - **Nothing.** Samples leave the decoder above ±1.0 and are clipped by
//!   whatever is furthest downstream — after the resampler, which turns an
//!   out-of-range sample into ringing on both sides of it, and after the volume
//!   stage, which makes whether it clips depend on the volume. Rejected: it
//!   makes the distortion's *existence* depend on unrelated settings.
//! - **A limiter.** Transparent on peaks, and stateful. The engine's decode
//!   path is a pure function of position: [`AudioSource::seek`] must produce
//!   the same samples for the same frame however you arrived at it, and the
//!   integration tests compare a seeked decode against a reference decode of
//!   the whole file. A limiter's gain depends on what it heard a moment ago,
//!   so a seek would change the samples. Rejected on that alone, before the
//!   question of whether baz wants to design a limiter.
//! - **Constant attenuation by the matrix's own worst case.** Chosen. The
//!   downmix becomes a pure linear matrix that provably cannot overflow, for
//!   any input, at any position, with no state and no dependence on playback
//!   history.
//!
//! So [`Downmix`] scales every coefficient by `1 / max(Σ|Lo row|, Σ|Ro row|)`.
//! For 5.1 that is **1/2.4142 = 0.41421, −7.66 dB**; for quadraphonic
//! (L, R, Ls, Rs) it is `1/1.7071 = 0.58579, −4.65 dB`; the exact figure per
//! layout is [`Downmix::headroom_db`].
//!
//! **The cost is real and is named here rather than discovered by a listener**:
//! a 5.1 file plays quieter than the stereo master of the same record. Two
//! things soften it. baz has ReplayGain (ADR-0013) and a ReplayGain analysis
//! pass (ADR-0015), and both measure *this* decoder's output, so an analysed
//! 5.1 file gets its level back exactly and automatically. And the attenuation
//! is a constant, so it costs no dynamics: the record is quieter, not squashed.
//!
//! # What is refused, and why that is still the right answer
//!
//! [`Downmix::for_layout`] describes exactly the speakers BS.775's equations
//! name, and refuses everything else rather than improvising:
//!
//! - **7.1** (both a rear *and* a side surround pair). BS.775's 3/2 programme
//!   has **one** surround pair. Folding two pairs at `kS` each would put 3 dB
//!   too much surround in the mix, and folding them at some other number would
//!   be a coefficient this module invented. A two-stage 7.1 → 5.1 → 2.0 fold is
//!   the ordinary answer and belongs to whoever can cite the first stage.
//! - **6.1** (a rear centre). Same reason: BS.775 does not place it.
//! - **Height, wide and top channels**, and any speaker the table above does
//!   not name.
//! - **A layout without both front speakers**, or with one half of a surround
//!   pair. Neither is a programme BS.775 describes, and inferring what the
//!   orphan channel *meant* is the guess this module exists not to make.
//!
//! A file in one of those layouts still fails with
//! [`PlaybackError::UnsupportedChannelLayout`], which names the layout it
//! found. That is the same honesty the blanket refusal had, narrowed to the
//! cases that actually need it.
//!
//! # Channel order is not assumed
//!
//! The one thing that would make all of the above sound wrong is putting the
//! centre channel where a surround belongs. **The order of channels inside a
//! decoded packet is a property of the container and the codec** — WAVE, FLAC,
//! Vorbis, AAC and ALAC each order 5.1 differently in their own bitstreams —
//! so this module never counts channels and never assumes a position.
//!
//! Symphonia's contract is that the *n*-th plane of a decoded buffer is the
//! *n*-th channel of `SignalSpec::channels` **in ascending bit order**, and its
//! Vorbis decoder contains an explicit permutation table
//! (`map_vorbis_channel`) to honour it. [`Downmix::for_layout`] takes the
//! layout and builds one coefficient pair per plane in that order; the caller
//! passes the layout Symphonia reports, never a count.
//! `each_speaker_lands_where_the_layout_says` in `tests/playback.rs` puts the
//! same 5.1 material through five containers that order it differently and
//! asserts they produce the same stereo pair; the numbers, and an independent
//! cross-check against ffmpeg's own downmix, are in
//! `docs/design/impl/multichannel-downmix/measurements.md`.

use symphonia::core::audio::Channels;

use super::{CHANNELS, PlaybackError};

/// BS.775's `kC` and `kS`: −3 dB, written the way the recommendation writes it.
const MINUS_3DB: f32 = std::f32::consts::FRAC_1_SQRT_2;

/// The BS.775 matrix, one row per speaker it names: `(speaker, [to Lo, to Ro])`.
///
/// A speaker absent from this table is a speaker the recommendation does not
/// place, and [`Downmix::for_layout`] refuses a layout containing one rather
/// than choosing a number for it. See the module docs for the citation and for
/// why the LFE row is zero.
const MATRIX: &[(Channels, [f32; CHANNELS])] = &[
    (Channels::FRONT_LEFT, [1.0, 0.0]),
    (Channels::FRONT_RIGHT, [0.0, 1.0]),
    (Channels::FRONT_CENTRE, [MINUS_3DB, MINUS_3DB]),
    // The LFE is dropped, not folded — module docs, "Decision 1". The row is
    // present and zero rather than absent, because "this speaker is understood
    // and contributes nothing" and "this speaker is not understood" are
    // different answers and only the second is a refusal.
    (Channels::LFE1, [0.0, 0.0]),
    (Channels::REAR_LEFT, [MINUS_3DB, 0.0]),
    (Channels::REAR_RIGHT, [0.0, MINUS_3DB]),
    // A 5.1 whose surrounds are declared as sides (which is what an ALAC magic
    // cookie and an ffmpeg `5.1(side)` layout produce for the same music) is
    // the same programme with the same BS.775 treatment.
    (Channels::SIDE_LEFT, [MINUS_3DB, 0.0]),
    (Channels::SIDE_RIGHT, [0.0, MINUS_3DB]),
];

/// The surround pair a 3/2 programme has one of.
const REAR_PAIR: Channels = Channels::REAR_LEFT.union(Channels::REAR_RIGHT);
/// The other spelling of the same pair.
const SIDE_PAIR: Channels = Channels::SIDE_LEFT.union(Channels::SIDE_RIGHT);
/// Both front speakers, which every layout this module accepts must have.
const FRONT_PAIR: Channels = Channels::FRONT_LEFT.union(Channels::FRONT_RIGHT);

/// A fixed linear matrix from one source layout to interleaved stereo.
///
/// Built once when a file is opened ([`Downmix::for_layout`]) and applied per
/// packet ([`Downmix::apply`]). Stateless by construction: the same input
/// frame yields the same output frame wherever in the track it is decoded,
/// which is what lets a seek stay comparable to a decode of the whole file.
#[derive(Debug, Clone)]
pub(crate) struct Downmix {
    /// One `[to Lo, to Ro]` pair per **source plane**, in the plane order
    /// Symphonia emits (the layout's set bits, ascending). Already scaled by
    /// the headroom factor, so [`Self::apply`] is a bare multiply-accumulate.
    coeffs: Vec<[f32; CHANNELS]>,
    /// The scale the coefficients were multiplied by, kept for
    /// [`Self::headroom_db`] and for the tests that pin it.
    scale: f32,
}

impl Downmix {
    /// Build the matrix for `layout`, or say why that layout is not one
    /// BS.775 describes.
    ///
    /// `layout` is the channel set **Symphonia reports for the decoded
    /// buffer**, never a channel count: the whole point is that position comes
    /// from the container and the codec rather than from arithmetic on the
    /// number of channels (module docs).
    ///
    /// Returns `Ok(None)` for mono and stereo, which are not downmixes and are
    /// handled by the caller's existing paths.
    ///
    /// # Errors
    ///
    /// [`PlaybackError::UnsupportedChannelLayout`], naming the layout, for
    /// every arrangement listed under "What is refused" in the module docs.
    pub(crate) fn for_layout(layout: Channels) -> Result<Option<Self>, PlaybackError> {
        let count = layout.count();
        if count <= CHANNELS {
            return Ok(None);
        }
        let refuse = |reason: &'static str| {
            Err(PlaybackError::UnsupportedChannelLayout {
                layout: describe(layout),
                reason,
            })
        };
        if !layout.contains(FRONT_PAIR) {
            return refuse(
                "a multichannel layout without both front speakers is not a programme ITU-R \
                 BS.775 describes, and which channel is the left one is not baz's to guess",
            );
        }
        if layout.contains(REAR_PAIR) && layout.contains(SIDE_PAIR) {
            return refuse(
                "ITU-R BS.775's downmix has one surround pair and this layout has two (7.1); \
                 folding both at −3 dB would put 3 dB too much surround in the mix, and any \
                 other number would be invented here",
            );
        }
        for half in [REAR_PAIR, SIDE_PAIR] {
            if layout.intersects(half) && !layout.contains(half) {
                return refuse(
                    "half a surround pair: ITU-R BS.775 places Ls and Rs together, and what a \
                     lone surround channel was mixed to mean is not recoverable from the file",
                );
            }
        }
        let mut coeffs = Vec::with_capacity(count);
        // `Channels::iter` walks the set bits from the lowest, which is exactly
        // the plane order `SampleBuffer::copy_interleaved_ref` writes.
        for speaker in layout.iter() {
            let Some((_, row)) = MATRIX.iter().find(|(s, _)| *s == speaker) else {
                return refuse(
                    "the layout contains a speaker ITU-R BS.775's two-channel downmix does not \
                     place (a height, wide, top or rear-centre channel)",
                );
            };
            coeffs.push(*row);
        }
        // Headroom: the largest gain any single output can see, which is the
        // larger of the two rows' absolute sums. Dividing by it makes overflow
        // impossible for any input in [-1, 1] — module docs, "Decision 2".
        let worst = (0..CHANNELS)
            .map(|out| coeffs.iter().map(|c| c[out].abs()).sum::<f32>())
            .fold(0.0f32, f32::max);
        // Unreachable for any layout that got this far (the front pair alone
        // sums to 1.0), but a division that cannot be zero is better than a
        // division that is merely never zero.
        let scale = if worst > 1.0 { 1.0 / worst } else { 1.0 };
        for row in &mut coeffs {
            for c in row {
                *c *= scale;
            }
        }
        Ok(Some(Self { coeffs, scale }))
    }

    /// Channels this matrix consumes per frame.
    pub(crate) fn source_channels(&self) -> usize {
        self.coeffs.len()
    }

    /// The constant attenuation this matrix applies, in decibels — always ≤ 0.
    ///
    /// The number the module docs promise not to make a listener discover:
    /// −7.66 dB for 5.1, −4.65 dB for quadraphonic. Reported rather than
    /// hidden, and exactly recovered by a ReplayGain pass over the same
    /// decoder's output.
    pub(crate) fn headroom_db(&self) -> f32 {
        20.0 * self.scale.log10()
    }

    /// Matrix `native` (interleaved, [`Self::source_channels`] per frame) into
    /// `out` as interleaved stereo.
    ///
    /// `out` is cleared and refilled; it is the caller's reused block, so a
    /// steady-state decode allocates nothing after the first packet.
    ///
    /// # Panics
    ///
    /// Never: a `native` length that is not a whole number of frames simply
    /// leaves its tail unread, which is what a truncated packet deserves.
    pub(crate) fn apply(&self, native: &[f32], out: &mut Vec<f32>) {
        let n = self.coeffs.len();
        out.clear();
        out.reserve(native.len() / n * CHANNELS);
        for frame in native.chunks_exact(n) {
            let mut lo = 0.0f32;
            let mut ro = 0.0f32;
            for (&s, row) in frame.iter().zip(&self.coeffs) {
                lo += s * row[0];
                ro += s * row[1];
            }
            out.push(lo);
            out.push(ro);
        }
    }
}

/// Name a channel set the way an error message should: `FL+FR+FC+LFE+RL+RR`.
///
/// Short spellings rather than Symphonia's `Debug`, because this string is
/// read by a person looking at a file that would not play, and
/// `FRONT_LEFT | FRONT_RIGHT | ...` is a screenful for a 7.1 file.
fn describe(layout: Channels) -> String {
    let mut out = String::new();
    for speaker in layout.iter() {
        if !out.is_empty() {
            out.push('+');
        }
        out.push_str(short_name(speaker));
    }
    if out.is_empty() {
        out.push_str("(none)");
    }
    out
}

/// The short spelling of one speaker, or its bit when Symphonia names one this
/// list does not — an unknown channel must still be *printable*, because the
/// message it appears in is the one that explains a refusal.
fn short_name(speaker: Channels) -> &'static str {
    match speaker {
        Channels::FRONT_LEFT => "FL",
        Channels::FRONT_RIGHT => "FR",
        Channels::FRONT_CENTRE => "FC",
        Channels::LFE1 => "LFE",
        Channels::REAR_LEFT => "RL",
        Channels::REAR_RIGHT => "RR",
        Channels::FRONT_LEFT_CENTRE => "FLC",
        Channels::FRONT_RIGHT_CENTRE => "FRC",
        Channels::REAR_CENTRE => "RC",
        Channels::SIDE_LEFT => "SL",
        Channels::SIDE_RIGHT => "SR",
        Channels::TOP_CENTRE => "TC",
        _ => "?",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The five-channel layout of BS.775's own equations, plus the LFE.
    const FIVE_ONE: Channels = Channels::FRONT_LEFT
        .union(Channels::FRONT_RIGHT)
        .union(Channels::FRONT_CENTRE)
        .union(Channels::LFE1)
        .union(Channels::REAR_LEFT)
        .union(Channels::REAR_RIGHT);

    /// Quadraphonic: a front pair and a rear pair, no centre and no LFE.
    const QUAD: Channels = FRONT_PAIR.union(REAR_PAIR);

    /// Coefficients before the headroom scale — what BS.775 actually writes.
    fn unscaled(dm: &Downmix) -> Vec<[f32; CHANNELS]> {
        dm.coeffs
            .iter()
            .map(|r| [r[0] / dm.scale, r[1] / dm.scale])
            .collect()
    }

    /// The matrix is BS.775's, in the plane order the layout's bits give, with
    /// the centre and the surrounds at −3 dB and the LFE at nothing.
    #[test]
    fn the_five_one_matrix_is_the_recommendation() {
        let dm = Downmix::for_layout(FIVE_ONE)
            .expect("5.1 is a layout BS.775 describes")
            .expect("5.1 is a downmix");
        let k = std::f32::consts::FRAC_1_SQRT_2;
        assert_eq!(
            unscaled(&dm),
            vec![
                [1.0, 0.0], // FL
                [0.0, 1.0], // FR
                [k, k],     // FC  — BS.775 kC
                [0.0, 0.0], // LFE — dropped, not folded
                [k, 0.0],   // RL  — BS.775 kS
                [0.0, k],   // RR
            ]
        );
    }

    /// The headroom is the matrix's own worst case and nothing else, so it can
    /// be derived on paper: 1 + 1/√2 + 1/√2 for 5.1, 1 + 1/√2 for quad.
    #[test]
    fn the_headroom_is_the_worst_case_row_sum() {
        let k = f64::from(std::f32::consts::FRAC_1_SQRT_2);
        for (layout, worst) in [(FIVE_ONE, 1.0 + k + k), (QUAD, 1.0 + k)] {
            let dm = Downmix::for_layout(layout)
                .expect("a layout BS.775 describes")
                .expect("a layout with more than two channels is a fold");
            let expected = 20.0 * (1.0 / worst).log10();
            assert!(
                (f64::from(dm.headroom_db()) - expected).abs() < 0.01,
                "{expected} dB expected, {} dB found",
                dm.headroom_db()
            );
        }
    }

    /// The reason the attenuation exists: full-scale correlated input across
    /// every channel must not leave the matrix above full scale.
    #[test]
    fn a_full_scale_correlated_frame_cannot_overflow() {
        for layout in [FIVE_ONE, QUAD] {
            let dm = Downmix::for_layout(layout)
                .expect("a layout BS.775 describes")
                .expect("a layout with more than two channels is a fold");
            let mut out = Vec::new();
            for sign in [1.0f32, -1.0] {
                dm.apply(&vec![sign; dm.source_channels()], &mut out);
                for s in &out {
                    assert!(s.abs() <= 1.0, "{layout:?} overflowed to {s}");
                }
            }
            // And it must actually *reach* full scale — an attenuation larger
            // than the matrix needs would be quiet for no reason.
            dm.apply(&vec![1.0; dm.source_channels()], &mut out);
            assert!((out[0] - 1.0).abs() < 1e-6, "left reached only {}", out[0]);
        }
    }

    /// Mono and stereo are not downmixes; the caller's existing paths keep them.
    #[test]
    fn one_and_two_channels_are_not_a_downmix() {
        for layout in [Channels::FRONT_LEFT, FRONT_PAIR] {
            assert!(
                Downmix::for_layout(layout)
                    .expect("mono and stereo are not refusals")
                    .is_none()
            );
        }
    }

    /// Every layout the module docs list as refused is refused, and the message
    /// names the layout so the refusal can be acted on.
    #[test]
    fn the_layouts_bs775_does_not_describe_are_refused() {
        let seven_one = FIVE_ONE.union(SIDE_PAIR);
        let six_one = FIVE_ONE.union(Channels::REAR_CENTRE);
        let height = FRONT_PAIR
            .union(Channels::FRONT_CENTRE)
            .union(Channels::TOP_FRONT_LEFT)
            .union(Channels::TOP_FRONT_RIGHT);
        let orphan = FRONT_PAIR
            .union(Channels::FRONT_CENTRE)
            .union(Channels::REAR_LEFT);
        let no_front = Channels::FRONT_CENTRE
            .union(Channels::LFE1)
            .union(REAR_PAIR);
        for layout in [seven_one, six_one, height, orphan, no_front] {
            let err = Downmix::for_layout(layout)
                .expect_err("a layout BS.775 does not describe must be refused, not guessed");
            let PlaybackError::UnsupportedChannelLayout { layout: named, .. } = &err else {
                panic!("wrong error for {layout:?}: {err}");
            };
            assert_eq!(named, &describe(layout));
        }
    }

    /// 5.1 with the surrounds declared as sides is the same programme and gets
    /// the same matrix — the difference is a container's spelling, not music.
    #[test]
    fn sides_and_rears_are_the_same_surround_pair() {
        let rears = Downmix::for_layout(FIVE_ONE)
            .expect("5.1 with rear surrounds")
            .expect("a fold");
        let sides = Downmix::for_layout(
            FRONT_PAIR
                .union(Channels::FRONT_CENTRE)
                .union(Channels::LFE1)
                .union(SIDE_PAIR),
        )
        .expect("5.1 with side surrounds")
        .expect("a fold");
        assert_eq!(unscaled(&rears), unscaled(&sides));
        assert!((rears.headroom_db() - sides.headroom_db()).abs() < 1e-6);
    }

    /// A layout is named in the order it is heard, so a refusal message can be
    /// compared against what `ffprobe` says about the same file.
    #[test]
    fn a_layout_is_named_left_to_right() {
        assert_eq!(describe(FIVE_ONE), "FL+FR+FC+LFE+RL+RR");
        assert_eq!(describe(QUAD), "FL+FR+RL+RR");
        assert_eq!(describe(Channels::empty()), "(none)");
    }
}
