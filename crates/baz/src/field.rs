//! **The ambient field** — the room, in the record's own colours.
//!
//! The owner, looking at the merged Now playing surface at full screen: *"also
//! fullscreen the now playing looks weird"*. He is right, and the cause was
//! designed before it was seen: a sleeve bounded at a flat 720 px leaves a
//! 1096 px column with a small square floating in it, and a square floating in
//! `#0C0D0E` is not a kiosk — doc 12 §5.5 calls it *"a postage stamp in a
//! void"*. [`crate::art::load_hero`] makes the square as large as the source
//! honestly allows; **this module is what fills what is left**.
//!
//! `docs/design/12-now-playing-and-kiosk.md` §5.3 and
//! [ADR-0029](../../../docs/adr/0029-the-ambient-surface.md) §2 are the
//! design. What follows is the arithmetic and the two things it is not.
//!
//! # It is data, not a copy of the work
//!
//! > **Three colours sampled from the cover, lightness and chroma pinned into
//! > the room's own range, composited as a wash over the room.**
//!
//! That is [`Palette::lamp`](crate::theme::Palette::lamp)'s own rule — *"hue
//! read from the record, lightness and chroma pinned … **data**, not a
//! preference"* — with three colours instead of one and a large area instead
//! of a small one. **If hue-read-from-the-record is data at 6 px it does not
//! become decoration at 1920**, and the four properties that make the claim
//! checkable rather than asserted are:
//!
//! 1. **It is not invertible.** [`Field`] is three hue angles. Three angles
//!    cannot reconstruct an image: you cannot see *what* the record is from
//!    the field, only that the room changed colour. A blurred enlarged copy —
//!    Apple Music's, `YouTube Music`'s — *is* the work, which is exactly why it
//!    is beautiful and exactly why it is a duplicate (doc 03 §232).
//! 2. **It has no resolution**, so *larger than its source* is not a predicate
//!    that applies to it. A gradient fills a 4K panel exactly as well as a
//!    1280 px one, which is the whole reason it is the right object for the
//!    space the artwork cannot honestly take.
//! 3. **It carries no lightness from the cover.** Only hue is read. A pale
//!    sleeve and a near-black one with the same hue give the same field,
//!    because the ladder is the room's, not the record's — which is what makes
//!    [`CEILING_L`] a property rather than a hope.
//! 4. **Amberol ships the honest version of this** and is the one treatment in
//!    doc 03's table that draws no copy of the art at all.
//!
//! # It is not a scrim, and the distinction is structural
//!
//! The product's standing rule objects to *"a surface laid over **the
//! collection** to make something else readable"*. The field is laid over
//! nothing: it is **under** everything, it dims no artwork, and it is the
//! room's own colour changed. Nothing is drawn on the sleeve; everything
//! ambient is drawn on the field, and the sleeve is the one object on the
//! surface with nothing on top of it.
//!
//! # One object, one value, no domains — and the reason is a measurement
//!
//! The field shipped with a **lower ceiling where type is**: under the run
//! column it was clamped flat to the room's own
//! [`wall`](crate::theme::Palette::wall) lightness, so that no run row carried
//! a contrast claim that was not already shipped and tested (doc 12 §5.4,
//! ADR-0029 §8.4 term 2).
//!
//! **The owner, 2026-08-10:** *"the background fade behind the album art seems
//! to abruptly end beside the track list which looks bad -- the fade should
//! continue under the playlist area too"*. He is right, and the defect is worse
//! than a taste: two washes drawn side by side do not merely step in lightness,
//! they **restart the ramp**, so the seam was a hard vertical edge announcing
//! the layout rather than a room lit by a record.
//!
//! It is now **one wash across the whole body**, at full amplitude, with no
//! domains at all — and that is settled by measurement rather than by nerve.
//! `every_run_row_is_legible_over_the_brightest_field` sweeps every room ×
//! every hue × every ink the run column draws, against the floor each ink's
//! *use* implies — the same instrument and the same floors `theme`'s own
//! contrast suite uses:
//!
//! | ink | over `wall` | over the field, worst hue | floor |
//! |---|---|---|---|
//! | `paper` | 15.33 | **13.54** | 4.5 |
//! | `paper_dim` | 8.20 | **7.24** | 4.5 |
//! | `paper_faint` | 5.34 | **4.71** | 4.5 |
//! | `paper_muted` | 3.61 | **3.19** | 3.0 |
//! | `alert` | 6.30 | **5.57** | 4.5 |
//!
//! (Closing Time; Reading Room's figures are within 0.02 of these at the
//! binding inks and are swept identically.) **The field costs every ink about
//! an eighth of its ratio and no ink its floor.** The binding case is
//! `paper_faint` — the durations and the summary — at 4.71 against a 4.5
//! floor, which is 4.7 % of margin and is the number to watch if [`CEILING_L`]
//! is ever raised.
//!
//! So the constraint that produced the seam was real, and the answer to it was
//! never a boundary: it was to check that the ceiling the field already has is
//! low enough. It is.
//!
//! # What is deliberately not here yet
//!
//! **No motion, no shader, no toggle.** This is the still field, which every
//! renderer draws and which the drifting one degrades to when the backend is
//! `tiny-skia` (doc 12 §7.5). Doc 12 §5.3 asks for *"a slow radial-plus-linear
//! wash"* and iced 0.13 publishes **only** [`iced::gradient::Linear`] — there
//! is no radial variant in the toolkit — so the radial half arrives with the
//! shader in step A7 or not at all, and the linear half is drawn honestly
//! rather than faked with stacked containers.

use iced::Color;

use crate::theme;

/// The field's ceiling in **Closing Time**: **L 0.22** (doc 12 §5.3) —
/// darker than any sleeve, brighter than the room.
///
/// Both halves are load-bearing and both are asserted
/// (`the_field_never_outshines_the_work_or_hides_the_room`). Darker than any
/// sleeve, because **the artwork must stay the brightest object on the
/// surface** and a field that competed with it would have made the artwork's
/// enlargement pointless. Brighter than the room's `wall` L 0.158, because a
/// field indistinguishable from `#0C0D0E` is not a field.
///
/// It sits between the room's `plinth` (L 0.195) and `plinth_lit` (L 0.231),
/// which is the honest description of what it is: **the room's own ladder,
/// one rung up, tinted**.
pub(crate) const CEILING_L: f32 = 0.22;

/// The field's chroma, **pinned** — the lamp's rule, at the lamp's own kind of
/// number.
///
/// **0.024, and it is a gamut measurement rather than a taste.** The field's
/// colours must survive every hue at every rung of the ladder in every room,
/// and the binding cases are the two ends: Closing Time's floor (L 0.158,
/// where cyans run out of sRGB first) and Reading Room's wall (L 0.941, where
/// oranges do). A binary search over both ladders at one-degree steps puts the
/// largest chroma that clips **nowhere** at 0.0269, and 0.024 takes 11 % of
/// margin under it. `every_hue_survives_the_ladder_without_leaving_srgb`
/// re-derives that sweep rather than trusting this sentence.
///
/// A colour that silently clips is **a hue that is no longer the record's** —
/// the derivation would still be honest and the drawing would not — which is
/// why the constant is measured rather than the clamp trusted.
///
/// It is ~9× the room's own chroma (`wall` is C 0.0027, a near-neutral), so it
/// reads as a tint at a glance while staying far under the lamp's 0.126 — the
/// accent states playback truth and the field must never be mistaken for it.
pub(crate) const CHROMA: f32 = 0.024;

/// A pixel needs at least this much oklch chroma to have a hue worth reading.
///
/// Below it a pixel is grey, and grey has an *arbitrary* hue angle — the
/// arctangent of two numbers that are both noise. Letting those vote would
/// make the field of a monochrome sleeve a coin flip between runs, which is
/// the opposite of the determinism `the_field_is_a_pure_function_of_the_cover`
/// exists to hold.
const CHROMA_FLOOR: f32 = 0.02;

/// What share of a cover must carry a hue before there is a field at all.
///
/// **2 %.** Under it the record is monochrome — a black sleeve with a white
/// mark, the fixture's `mono` family, half of ECM's catalogue — and the honest
/// answer is *the room*, not a grey wash pretending to be derived from
/// something. Story S7's first criterion, generalised from *no art at all* to
/// *no colour in the art*: both fall back to `#0C0D0E`.
const PRESENCE_FLOOR: f32 = 0.02;

/// How many pixels [`derive`] reads, at most.
///
/// A 1024² hero is a million pixels and the answer does not get better after
/// a few thousand. The stride is computed from the image's own dimensions, so
/// **which** pixels are read is a function of the cover and nothing else —
/// there is no clock, no hash seed and no thread count in the sample.
const SAMPLE_CAP: usize = 16_384;

/// Where in the sorted lightnesses the two anchors are taken.
///
/// The **10th and 90th percentile** rather than the true darkest and lightest
/// pixels: one pixel is a JPEG artefact, a hundred are a colour. Doc 12 §5.3
/// says *"the darkest, and the lightest"* and this is that, made robust —
/// stated here because the substitution is the kind a reader should not have
/// to reverse-engineer.
const ANCHOR: (f32, f32) = (0.10, 0.90);

/// **The field of one record**: three hue angles in degrees, and nothing else.
///
/// The whole of what this surface reads from a cover. Three `f32`s is also the
/// proof of §5.3's first property — an image cannot be reconstructed from
/// three angles — which is why the type is this narrow on purpose rather than
/// holding three [`Color`]s and clamping them at draw time.
///
/// Ordered **light anchor, dominant, dark anchor**, which is the order they
/// are laid along the wash: the field is brightest where the work hangs and
/// falls away from it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Field {
    /// Hue angles in degrees, `0.0..360.0`.
    hues: [f32; 3],
}

impl Field {
    /// **The field of a decoded cover** — pure, total, and the same answer for
    /// the same bytes on every machine.
    ///
    /// `rgba` is [`crate::art::load_hero`]'s return, `w × h` RGBA8. `None` when
    /// the cover carries no hue worth reading ([`PRESENCE_FLOOR`]), which is
    /// story S7's *the field falls back to the room* — a monochrome sleeve gets
    /// `#0C0D0E`, not a grey wash claiming to be derived from it.
    ///
    /// Runs on the decode's own blocking worker, once per record, beside the
    /// decode that produced its input. **The UI thread never sees a pixel of a
    /// cover**, which is what keeps the field's per-frame cost at three colour
    /// conversions.
    #[must_use]
    pub(crate) fn derive(w: u32, h: u32, rgba: &[u8]) -> Option<Self> {
        let pixels = (w as usize).checked_mul(h as usize)?;
        if pixels == 0 || pixels.checked_mul(4).is_none_or(|bytes| rgba.len() < bytes) {
            return None;
        }
        let stride = pixels.div_ceil(SAMPLE_CAP).max(1);
        // (lightness, hue) of every sampled pixel that has a hue at all, and
        // a chroma-weighted vote per 15° bucket. One pass, no allocation
        // beyond the anchor list.
        let mut lit: Vec<(f32, f32)> = Vec::with_capacity(SAMPLE_CAP);
        let mut buckets = [0.0_f32; BUCKETS];
        let mut seen = 0_usize;
        for index in (0..pixels).step_by(stride) {
            seen += 1;
            let at = index * 4;
            // A transparent pixel is not part of the picture. Covers are
            // opaque in practice; a PNG with a cut corner is not, and its
            // corner is the room showing through rather than black.
            if rgba[at + 3] < 128 {
                continue;
            }
            let (lightness, chroma, hue) =
                oklch(Color::from_rgb8(rgba[at], rgba[at + 1], rgba[at + 2]));
            if chroma < CHROMA_FLOOR {
                continue;
            }
            lit.push((lightness, hue));
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "hue is in 0.0..360.0, so the quotient is in 0..BUCKETS \
                          and the min() below pins the boundary case"
            )]
            let bucket = ((hue / BUCKET_DEGREES) as usize).min(BUCKETS - 1);
            buckets[bucket] += chroma;
        }
        #[expect(
            clippy::cast_precision_loss,
            reason = "sample counts are at most SAMPLE_CAP, exact in f32"
        )]
        let present = lit.len() as f32 / seen.max(1) as f32;
        if lit.is_empty() || present < PRESENCE_FLOOR {
            return None;
        }
        // **The dominant hue**: the fullest 15° bucket, refined to the
        // chroma-weighted circular mean of the pixels *in* that bucket, so the
        // answer is a hue the record actually has rather than a bucket's
        // midpoint.
        let widest = buckets
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map_or(0, |(index, _)| index);
        #[expect(
            clippy::cast_precision_loss,
            reason = "BUCKETS is 24; the cast is exact"
        )]
        let low = widest as f32 * BUCKET_DEGREES;
        let dominant = circular_mean(
            lit.iter()
                .map(|&(_, hue)| hue)
                .filter(|hue| *hue >= low && *hue < low + BUCKET_DEGREES),
        )
        .unwrap_or(low + BUCKET_DEGREES / 2.0);
        // **The two anchors**: the hues at the 10th and 90th percentile of
        // lightness among the pixels that have a hue. `select_nth_unstable_by`
        // rather than a sort — the answer is two elements, not an order, and
        // ties between equal lightnesses cannot change a *hue* the eye can
        // tell apart.
        let last = lit.len() - 1;
        #[expect(
            clippy::cast_precision_loss,
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "lit.len() <= SAMPLE_CAP, and the product of a 0..1 \
                      fraction with it is in range by construction"
        )]
        let at = |fraction: f32| ((last as f32) * fraction).round() as usize;
        let (dark, light) = (at(ANCHOR.0), at(ANCHOR.1));
        let (_, at_light, _) = lit.select_nth_unstable_by(light, |a, b| a.0.total_cmp(&b.0));
        let lightest = at_light.1;
        let (_, at_dark, _) = lit.select_nth_unstable_by(dark, |a, b| a.0.total_cmp(&b.0));
        let darkest = at_dark.1;
        Some(Self {
            hues: [lightest, dominant, darkest],
        })
    }

    /// **The three colours the field is drawn in** in the room `room` —
    /// brightest first, which is the order they are laid along the wash.
    ///
    /// The record supplies the hue and **nothing else**: lightness comes from
    /// the room's own ladder and chroma is [`CHROMA`], pinned. Three colour
    /// conversions, run once per view build, which is the whole per-frame cost
    /// of the field.
    ///
    /// **There is no second answer for the region under the run column.** There
    /// was — a `Reach` with a flat variant — and it is gone with the seam the
    /// owner reported (module docs): one object, one value, everywhere.
    #[must_use]
    pub(crate) fn colors(self, room: &theme::Palette) -> [Color; 3] {
        let ground = lightness(room.wall);
        // The ladder climbs toward the lamp, which is *lighter* in a dark room
        // and *darker* in a light one — the same inversion `Palette::recess`
        // documents for elevation, read off the room's own `wall → plinth` step
        // rather than off a room's name, so a third room needs no edit here.
        let (floor, ceiling) = (ground, ground + rise(room));
        std::array::from_fn(|index| {
            from_oklch(
                floor + (ceiling - floor) * RUNGS[index],
                CHROMA,
                self.hues[index],
            )
        })
    }

    /// **The field, as the toolkit draws it**: a three-stop linear wash at
    /// [`ANGLE`].
    ///
    /// The gradient is the whole of the still field — no layers, no stacked
    /// containers faking a radial, and no per-frame work beyond building this
    /// value.
    ///
    /// **One wash, over the whole body.** It used to be two side by side, and
    /// two gradients do not merely step at their join — the second restarts the
    /// ramp — which is the hard vertical edge beside the run column the owner
    /// reported. A single value cannot have a seam.
    #[must_use]
    pub(crate) fn wash(self, room: &theme::Palette) -> iced::gradient::Linear {
        let [near, mid, far] = self.colors(room);
        iced::gradient::Linear::new(iced::Radians(ANGLE))
            .add_stop(0.0, near)
            .add_stop(0.5, mid)
            .add_stop(1.0, far)
    }
}

/// Where the three colours sit on the room's ladder, brightest first.
///
/// The field is **brightest where the work hangs** and falls away from it, so
/// the sleeve is never the darkest thing in its own corner of the surface.
const RUNGS: [f32; 3] = [1.0, 0.5, 0.0];

/// The wash's angle, in radians: **2.4** — [`crate::views::gradient_block`]'s
/// own, and deliberately the same number.
///
/// baz has exactly one other gradient, the wall's deterministic placeholder
/// for a record with no cover, and two gradients running at two angles on one
/// screen is a surface with two opinions about where its light comes from. A
/// record with no art gets the placeholder over the room; a record with art
/// gets the sleeve over the field; both are lit from the same direction.
const ANGLE: f32 = 2.4;

/// How many degrees one hue bucket spans, and how many there are: **24 × 15°**.
///
/// Fifteen degrees is about where two hues stop being *the same colour named
/// twice* at these chromas. Fewer buckets and a red sleeve's shadow votes with
/// its highlight; more and a JPEG's ringing splits one hue across two.
const BUCKETS: usize = 24;
/// The width of one [`BUCKETS`] bucket, in degrees. The two are one fact
/// spelled twice because a cast in a `const` is not free of a precision lint,
/// so the const-assert below is what keeps them agreeing.
const BUCKET_DEGREES: f32 = 15.0;
const _: () = assert!(BUCKETS == 24 && BUCKET_DEGREES == 15.0);

/// How far the field rises off its room's `wall`, in oklch L.
///
/// Derived from [`CEILING_L`] rather than written twice: in Closing Time the
/// wall is L 0.158 and the ceiling is 0.22, so the rise is **0.062** — and the
/// *same* rise is applied in any other room, in the direction that room's own
/// ladder climbs. Reading the direction off `wall → plinth` is what keeps this
/// function correct for a light room without naming one: surfaces rise toward
/// the lamp, so a plinth is lighter than the wall in Closing Time and darker
/// than it in Reading Room, and the field follows.
fn rise(room: &theme::Palette) -> f32 {
    let magnitude = CEILING_L - lightness(theme::CLOSING_TIME.wall);
    let up = lightness(room.plinth) - lightness(room.wall);
    if up < 0.0 { -magnitude } else { magnitude }
}

/// The **oklch L** of an sRGB colour.
///
/// The one number `theme`'s contrast test measures elevation in, published
/// here so the field and the room are measured by one instrument rather than
/// by two implementations of the same matrices that could drift.
#[must_use]
pub(crate) fn lightness(color: Color) -> f32 {
    oklch(color).0
}

/// sRGB → oklch: `(lightness, chroma, hue in degrees 0.0..360.0)`.
///
/// Björn Ottosson's published oklab constants, and no dependency — the same
/// twenty-five lines ADR-0017 §1.6 already accepted for the contrast test,
/// with the polar half added because the field reads a *hue* and the contrast
/// test never had to.
fn oklch(color: Color) -> (f32, f32, f32) {
    let red = linear(color.r);
    let green = linear(color.g);
    let blue = linear(color.b);
    let long = 0.412_221_5 * red + 0.536_332_54 * green + 0.051_445_995 * blue;
    let medium = 0.211_903_5 * red + 0.680_699_5 * green + 0.107_396_96 * blue;
    let short = 0.088_302_46 * red + 0.281_718_85 * green + 0.629_978_7 * blue;
    let (long, medium, short) = (long.cbrt(), medium.cbrt(), short.cbrt());
    let lightness = 0.210_454_26 * long + 0.793_617_8 * medium - 0.004_072_047 * short;
    let a = 1.977_998_5 * long - 2.428_592_2 * medium + 0.450_593_7 * short;
    let b = 0.025_904_037 * long + 0.782_771_77 * medium - 0.808_675_77 * short;
    let hue = b.atan2(a).to_degrees().rem_euclid(360.0);
    (lightness, a.hypot(b), hue)
}

/// oklch → sRGB, clamped into gamut.
///
/// The clamp is defensive rather than load-bearing: [`CHROMA`] is chosen so
/// that no hue leaves sRGB anywhere on the field's ladder, and
/// `every_hue_survives_the_ladder_without_leaving_srgb` is what holds that.
/// A clamp that ever fired would be a *hue* silently changed, which is why the
/// constant is measured rather than the clamp trusted.
fn from_oklch(lightness: f32, chroma: f32, hue: f32) -> Color {
    let (sin, cos) = hue.to_radians().sin_cos();
    let (a, b) = (chroma * cos, chroma * sin);
    let long = (lightness + 0.396_337_78 * a + 0.215_803_76 * b).powi(3);
    let medium = (lightness - 0.105_561_346 * a - 0.063_854_17 * b).powi(3);
    let short = (lightness - 0.089_484_18 * a - 1.291_485_5 * b).powi(3);
    Color::from_rgb(
        gamma(4.076_741_7 * long - 3.307_711_6 * medium + 0.230_969_94 * short),
        gamma(-1.268_438 * long + 2.609_757_4 * medium - 0.341_319_4 * short),
        gamma(-0.004_196_086_3 * long - 0.703_418_6 * medium + 1.707_614_7 * short),
    )
}

/// sRGB transfer function, forward (encoded channel → linear).
fn linear(channel: f32) -> f32 {
    if channel <= 0.040_45 {
        channel / 12.92
    } else {
        ((channel + 0.055) / 1.055).powf(2.4)
    }
}

/// sRGB transfer function, inverse (linear → encoded channel), gamut-clamped.
fn gamma(channel: f32) -> f32 {
    let channel = channel.clamp(0.0, 1.0);
    if channel <= 0.003_130_8 {
        channel * 12.92
    } else {
        1.055f32.mul_add(channel.powf(1.0 / 2.4), -0.055)
    }
}

/// The mean of a set of angles in degrees, taken on the circle rather than on
/// the number line — `None` for an empty set.
///
/// Averaging 350° and 10° arithmetically gives 180°, which is cyan for two
/// reds. Every hue mean in this module goes through here.
fn circular_mean(hues: impl Iterator<Item = f32>) -> Option<f32> {
    let (mut x, mut y, mut count) = (0.0_f32, 0.0_f32, 0_u32);
    for hue in hues {
        let (sin, cos) = hue.to_radians().sin_cos();
        x += cos;
        y += sin;
        count += 1;
    }
    (count > 0).then(|| y.atan2(x).to_degrees().rem_euclid(360.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An `w × h` RGBA cover, painted by `paint(x, y) -> (r, g, b)`.
    fn cover(width: u32, height: u32, paint: impl Fn(u32, u32) -> [u8; 3]) -> Vec<u8> {
        let mut out = Vec::with_capacity((width * height * 4) as usize);
        for y in 0..height {
            for x in 0..width {
                out.extend_from_slice(&paint(x, y));
                out.push(255);
            }
        }
        out
    }

    /// A flat cover of one colour.
    fn flat(width: u32, height: u32, rgb: [u8; 3]) -> Vec<u8> {
        cover(width, height, |_, _| rgb)
    }

    /// **The field is a pure function of the cover** — the property the whole
    /// *it is data, not a copy* argument rests on (doc 12 §5.3).
    ///
    /// Same bytes, same answer, every time and in any order: no clock, no hash
    /// seed, no thread count and no floating-point accumulation order reaches
    /// the sample, because the stride is computed from the image's own
    /// dimensions. A field that wobbled between runs would be decoration
    /// wearing derivation's clothes.
    #[test]
    fn the_field_is_a_pure_function_of_the_cover() {
        // A cover with genuine structure: a hue ramp with a dark band and a
        // light band, so all three anchors have something to find.
        let art = cover(64, 64, |x, y| {
            let hue = u8::try_from(x * 4).unwrap_or(255);
            match y {
                0..=15 => [hue / 4, 12, 60],
                48..=63 => [255, 220 - hue / 4, 200],
                _ => [200, hue, 40],
            }
        });
        let first = Field::derive(64, 64, &art).expect("a cover with hue has a field");
        for _ in 0..16 {
            assert_eq!(
                Field::derive(64, 64, &art),
                Some(first),
                "the field moved between two derivations of the same bytes"
            );
        }
        // …and it is genuinely *of* this cover: a different cover is a
        // different field, or the derivation is reading nothing.
        let other = flat(64, 64, [40, 90, 200]);
        assert_ne!(Field::derive(64, 64, &other), Some(first));

        // Every hue is a real angle rather than a bucket edge or a NaN.
        for hue in first.hues {
            assert!(hue.is_finite() && (0.0..360.0).contains(&hue), "{hue}");
        }
    }

    /// **A cover with no chroma yields the room, not a grey wash** (doc 12
    /// §12's step 3 test, and story S7's first criterion generalised).
    ///
    /// The `mono` sleeve — near-black with one faint mark — is a real and
    /// common object, and the honest answer for it is `#0C0D0E`. A grey field
    /// would be a derivation that had nothing to derive from, drawn anyway.
    #[test]
    fn a_cover_with_no_hue_has_no_field() {
        for grey in [0_u8, 8, 64, 128, 200, 255] {
            assert_eq!(
                Field::derive(32, 32, &flat(32, 32, [grey, grey, grey])),
                None,
                "a flat grey at {grey} produced a field"
            );
        }
        // A near-neutral sleeve with one small coloured mark is still under
        // the presence floor: 1 % of the pixels cannot decide the room.
        let sprinkled = cover(100, 100, |x, y| {
            if y == 0 && x < 100 {
                [220, 30, 10]
            } else {
                [18, 18, 20]
            }
        });
        assert_eq!(Field::derive(100, 100, &sprinkled), None);
        // …and a cover that is a third coloured is comfortably over it.
        let third = cover(
            100,
            100,
            |_, y| {
                if y < 34 { [220, 30, 10] } else { [18, 18, 20] }
            },
        );
        assert!(Field::derive(100, 100, &third).is_some());

        // Degenerate inputs are `None` rather than a panic: an empty image, a
        // truncated buffer, and a size that would overflow the pixel count.
        assert_eq!(Field::derive(0, 0, &[]), None);
        assert_eq!(Field::derive(4, 4, &[0; 8]), None);
        assert_eq!(Field::derive(u32::MAX, u32::MAX, &[0; 64]), None);
    }

    /// **The field reads hue and nothing else** — §5.3's third property, which
    /// is what makes [`CEILING_L`] a fact about the room rather than a hope
    /// about covers.
    ///
    /// Two sleeves of the same hue at opposite ends of the lightness scale — a
    /// pale one and a near-black one — produce the same field, because the
    /// ladder the colours are hung on is the room's.
    #[test]
    fn only_hue_comes_from_the_record() {
        let pale = Field::derive(32, 32, &flat(32, 32, [250, 205, 190])).expect("pale");
        let deep = Field::derive(32, 32, &flat(32, 32, [90, 32, 20])).expect("deep");
        let room = &theme::CLOSING_TIME;
        for (a, b) in pale.colors(room).into_iter().zip(deep.colors(room)) {
            let (a_l, _, a_h) = oklch(a);
            let (b_l, _, b_h) = oklch(b);
            assert!(
                (a_l - b_l).abs() < 0.001,
                "the record's own lightness reached the field: {a_l} vs {b_l}"
            );
            // The two sleeves are the same hue family, so the fields agree to
            // within a bucket.
            let apart = (a_h - b_h).abs().min(360.0 - (a_h - b_h).abs());
            assert!(apart < BUCKET_DEGREES, "{a_h} vs {b_h}");
        }
    }

    /// **The field never outshines the work, and never hides the room.**
    ///
    /// Doc 12 §5.3's two testable constraints, in both rooms and at both
    /// reaches. The ceiling is what stops the field competing with the sleeve
    /// — *the artwork is the brightest object on this surface by construction*
    /// — and the floor is what stops it being an invisible change.
    #[test]
    fn the_field_never_outshines_the_work_or_hides_the_room() {
        // The stated number, asserted rather than trusted: Closing Time's
        // ceiling is L 0.22, between its `plinth` and its `plinth_lit`.
        let dark = &theme::CLOSING_TIME;
        let ground = lightness(dark.wall);
        assert!((ground + rise(dark) - CEILING_L).abs() < 0.0005);
        assert!(lightness(dark.plinth) < CEILING_L);
        assert!(CEILING_L < lightness(dark.plinth_lit));

        for room in [&theme::CLOSING_TIME, &theme::READING_ROOM] {
            let ground = lightness(room.wall);
            let ceiling = ground + rise(room);
            for hue in (0..360).step_by(1) {
                #[expect(clippy::cast_precision_loss, reason = "0..360 is exact in f32")]
                let field = Field {
                    hues: [hue as f32; 3],
                };
                for colour in field.colors(room) {
                    let l = lightness(colour);
                    // Inside the room's own band, at either end inclusive.
                    assert!(
                        l >= ground.min(ceiling) - 0.002 && l <= ground.max(ceiling) + 0.002,
                        "{}: hue {hue} left the band at L {l}",
                        room.name
                    );
                }
            }
        }
    }

    /// **Every hue survives the ladder without leaving sRGB**, which is what
    /// [`CHROMA`] is chosen for.
    ///
    /// A clamped channel is a hue silently changed — the field would stop
    /// being the record's colour and start being the nearest colour the
    /// display could manage, without saying so. Swept at one-degree steps
    /// across the whole ladder in both rooms; the check is that a round trip
    /// through sRGB comes back to the hue that went in.
    #[test]
    fn every_hue_survives_the_ladder_without_leaving_srgb() {
        for room in [&theme::CLOSING_TIME, &theme::READING_ROOM] {
            let ground = lightness(room.wall);
            let ceiling = ground + rise(room);
            for rung in 0_u8..=10 {
                let l = ground + (ceiling - ground) * f32::from(rung) / 10.0;
                for hue in 0..360 {
                    let wanted = f32::from(u16::try_from(hue).expect("0..360"));
                    let (_, chroma, got) = oklch(from_oklch(l, CHROMA, wanted));
                    let apart = (got - wanted).abs().min(360.0 - (got - wanted).abs());
                    assert!(
                        apart < 1.0,
                        "{}: hue {wanted} at L {l} came back {got} — the gamut clamp fired",
                        room.name
                    );
                    assert!(
                        (chroma - CHROMA).abs() < 0.002,
                        "{}: chroma {chroma} at hue {wanted}, L {l}",
                        room.name
                    );
                }
            }
        }
    }

    /// A mean of angles is taken **on the circle**: 350° and 10° average to
    /// 0°, not to 180°. Every hue mean in this module goes through it, so a
    /// red sleeve whose hues straddle the origin cannot come out cyan.
    #[test]
    fn hues_average_on_the_circle() {
        let mean = circular_mean([350.0, 10.0].into_iter()).expect("two angles");
        assert!(mean.min(360.0 - mean) < 0.01, "{mean}");
        assert_eq!(circular_mean(std::iter::empty()), None);
        let one = circular_mean(std::iter::once(214.5)).expect("one angle");
        assert!((one - 214.5).abs() < 0.01, "{one}");
        // The straddle is the case that matters, and it is what a red cover
        // actually looks like: hues either side of 0°.
        let red = cover(32, 32, |x, _| {
            if x % 2 == 0 {
                [200, 20, 30]
            } else {
                [200, 30, 20]
            }
        });
        let field = Field::derive(32, 32, &red).expect("red has a hue");
        for hue in field.hues {
            assert!(
                !(60.0..=300.0).contains(&hue),
                "a red cover derived hue {hue}"
            );
        }
    }

    /// **Every run row is legible over the brightest field** — the measurement
    /// that let the field become continuous, and the one to re-run before
    /// [`CEILING_L`] is ever raised.
    ///
    /// The owner, 2026-08-10: *"the background fade behind the album art seems
    /// to abruptly end beside the track list which looks bad -- the fade should
    /// continue under the playlist area too"*. The seam existed because the
    /// field was clamped flat under the run column so that no row carried a
    /// contrast claim that was not already shipped (doc 12 §5.4 term 2). The
    /// honest way to remove the clamp is not to argue that it looks fine — it
    /// is to measure what the rows are actually read against.
    ///
    /// So: every room × every hue × every ink the run column draws, against the
    /// **field's own brightest stop**, at the floor each ink's *use* implies —
    /// 4.5 : 1 to be read, 3.0 : 1 for a mark. Those are `theme`'s own floors
    /// and this is `theme`'s own instrument, re-implemented here for the reason
    /// [`lightness`] is published: one measurement, two callers, no drift.
    ///
    /// **The result, and the margin.** The field costs every ink about an
    /// eighth of its ratio and no ink its floor. The binding case is
    /// `paper_faint` — the durations, the positions and the summary — at
    /// **4.71 : 1** against a 4.5 floor, which is 4.7 % of margin. The test
    /// asserts the floor rather than the figure, because the figure moves with
    /// the palette and the floor is the promise; the figure is recorded in the
    /// module docs so a future reader can see how much room there is.
    #[test]
    fn every_run_row_is_legible_over_the_brightest_field() {
        /// WCAG 2.1 relative luminance.
        fn luminance(color: Color) -> f32 {
            fn linear(channel: f32) -> f32 {
                if channel <= 0.04045 {
                    channel / 12.92
                } else {
                    ((channel + 0.055) / 1.055).powf(2.4)
                }
            }
            0.2126 * linear(color.r) + 0.7152 * linear(color.g) + 0.0722 * linear(color.b)
        }
        fn contrast(foreground: Color, background: Color) -> f32 {
            let (a, b) = (luminance(foreground), luminance(background));
            (a.max(b) + 0.05) / (a.min(b) + 0.05)
        }
        /// To be read.
        const TEXT: f32 = 4.5;
        /// To be found — `paper_muted` is the album group's artist line and the
        /// empty state's third line, which `theme`'s own suite already treats
        /// as a mark rather than a sentence.
        const MARK: f32 = 3.0;

        for room in [&theme::CLOSING_TIME, &theme::READING_ROOM] {
            // Every ink `views::queue` puts on a row, a header or the strip.
            let inks = [
                ("paper", room.paper, TEXT),
                ("paper_dim", room.paper_dim, TEXT),
                ("paper_faint", room.paper_faint, TEXT),
                ("alert", room.alert, TEXT),
                ("paper_muted", room.paper_muted, MARK),
            ];
            for hue in 0..360 {
                #[expect(clippy::cast_precision_loss, reason = "0..360 is exact in f32")]
                let field = Field {
                    hues: [hue as f32; 3],
                };
                for ground in field.colors(room) {
                    for (name, ink, floor) in inks {
                        let ratio = contrast(ink, ground);
                        assert!(
                            ratio >= floor,
                            "{}: {name} over the field at hue {hue} is \
                             {ratio:.2} : 1, below its {floor} : 1 floor — the \
                             field may not run under the run column at this \
                             ceiling",
                            room.name
                        );
                    }
                }
            }
        }

        // …and the claim that makes the sweep meaningful: the *brightest* stop
        // is the worst case, so measuring all three and asserting the floor
        // cannot miss a darker one that happens to pair worse. In a dark room
        // the field climbs away from the ink; in a light one it falls toward
        // it — which is why both rooms are swept rather than one reasoned about.
        for room in [&theme::CLOSING_TIME, &theme::READING_ROOM] {
            let field = Field { hues: [175.0; 3] };
            let stops = field.colors(room);
            let worst = stops
                .iter()
                .map(|&stop| contrast(room.paper_faint, stop))
                .fold(f32::INFINITY, f32::min);
            assert!(
                worst >= TEXT,
                "{}: the binding ink lost its floor at {worst:.2} : 1",
                room.name
            );
            assert!(
                worst < contrast(room.paper_faint, room.wall),
                "{}: the field is supposed to cost the ink something — if it \
                 costs nothing, the ceiling has collapsed onto the wall and \
                 there is no field",
                room.name
            );
        }
    }
}
