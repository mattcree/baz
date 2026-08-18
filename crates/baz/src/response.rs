//! **The curve behind the faders** — what the equaliser is actually doing to
//! the sound, drawn under the controls that ask for it.
//!
//! The owner, 2026-08-18: *"sliders help show graphically how high the current
//! of frequencies is biased."* Ten handles do part of that job and stop short
//! of the interesting half, because **the handles are not the curve**. Two
//! neighbouring bands at +6 dB do not make a pair of +6 dB bumps; they overlap
//! and make about +9 dB between them, and there is no arrangement of ten
//! separate grips that shows it. A listener who pushes 63 and 125 together and
//! wonders why it sounds heavier than either number promised is looking at a
//! control that is telling them the truth and not showing it.
//!
//! So this draws the real magnitude response — the same arithmetic the filters
//! run, evaluated on the unit circle rather than over samples
//! ([`baz_core::equalizer::response_curve`]) — and the faders stand on top of
//! it. The handles are the request; this is the answer.
//!
//! # Why it is drawn in quads
//!
//! The same reason [`crate::contour`] is, and its note is the one to read:
//! iced's `canvas` would give strokes directly and costs a tessellation stack
//! this project prices deliberately. So the curve is a column per pixel,
//! filled from the zero line to the response — which is not a compromise
//! dressed up. A filled band reads as *a region of boost*, which is what it
//! is, and it puts the equaliser in the same visual language as the spectrum
//! analyser and the vibe contour.
//!
//! # It agrees with the faders by construction
//!
//! Both axes are shared rather than matched:
//!
//! - **up** is [`crate::fader::y_of_db`], the fader's own mapping, called
//!   here — so the curve passes *through* the handles instead of near them;
//! - **across** is log frequency, and [`baz_core::equalizer::CENTRES`] are
//!   octave-spaced, so equal steps in log frequency are equal steps in
//!   column pitch. Band *i*'s peak lands on band *i*'s fader because both are
//!   derived from the same pitch, not because the numbers were tuned until it
//!   looked right. The test named `the_curve_peaks_under_the_fader_that_raised_it`
//!   holds that.
//!
//! # It holds no state
//!
//! Layer 3 (ADR-0006), and less than most: no pointer, no events, no tree
//! state. It is given a curve and it draws it.

use iced::advanced::widget::Tree;
use iced::advanced::widget::Widget;
use iced::advanced::{Layout, layout, renderer};
use iced::{Border, Color, Element, Length, Rectangle, Size, Theme, mouse};

use crate::fader;
use crate::theme;

/// How many columns the curve is drawn in.
///
/// One per pixel of a panel this wide, near enough — the band row is a little
/// over 400 px. Sampling finer than the screen buys nothing; sampling coarser
/// shows as steps on the steep sides of a single boosted band.
const POINTS: usize = 420;

/// A drawn frequency response over a fader row's own geometry.
pub(crate) struct Response {
    /// Decibels, sampled log-evenly across the span [`span`] returns.
    curve: Vec<f32>,
    limit: f32,
    height: f32,
    palette: &'static theme::Palette,
}

impl Response {
    /// Sample `bands` across exactly the span a row of faders covers.
    ///
    /// **The pre-amp is deliberately not in this**, though
    /// [`baz_core::equalizer::response_curve`] would take it. It is a uniform
    /// offset: it moves every frequency by the same amount and so contributes
    /// nothing to which frequencies are lifted over which others — which is
    /// the only thing this picture is for. Folding it in slides the whole
    /// curve off the handles that asked for it, so a listener who has taken
    /// the suggested pre-amp sees a shape that no longer touches a single one
    /// of their own controls, and reads that as the panel being broken.
    ///
    /// The headroom is not hidden by leaving it out — it has its own fader,
    /// its own number, and its own rule separating it from the bands.
    pub(crate) fn new(
        bands: baz_core::equalizer::Bands,
        limit: f32,
        height: f32,
        palette: &'static theme::Palette,
    ) -> Self {
        let (from_hz, to_hz) = span();
        let mut curve = vec![0.0; POINTS];
        baz_core::equalizer::response_curve(bands, 0.0, from_hz, to_hz, &mut curve);
        Self {
            curve,
            limit,
            height,
            palette,
        }
    }
}

/// **The frequency span a row of faders covers**, edge to edge.
///
/// The row is `n` columns of [`fader::HIT_W`] on [`fader::GAP`] gaps, and
/// each band's fader is centred in its own column. So the row's left edge is
/// half a column *before* the first band and its right edge half a column
/// after the last — and since the centres are octave-spaced, half a column is
/// a fixed fraction of an octave.
fn span() -> (f32, f32) {
    let centres = baz_core::equalizer::CENTRES;
    let pitch = fader::HIT_W + fader::GAP;
    let overhang = (fader::HIT_W / 2.0) / pitch;
    #[expect(
        clippy::cast_precision_loss,
        reason = "ten bands; the index is exact in f32 many times over"
    )]
    let last = (centres.len() - 1) as f32;
    let bottom = centres[0];
    (
        bottom * 2.0_f32.powf(-overhang),
        bottom * 2.0_f32.powf(last + overhang),
    )
}

impl<Message, Renderer> Widget<Message, Theme, Renderer> for Response
where
    Renderer: renderer::Renderer,
{
    fn size(&self) -> Size<Length> {
        Size {
            width: Length::Fill,
            height: Length::Fixed(self.height),
        }
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        layout::Node::new(limits.resolve(
            Length::Fill,
            Length::Fixed(self.height),
            Size::new(0.0, self.height),
        ))
    }

    fn draw(
        &self,
        _tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        if bounds.width < 1.0 || self.curve.is_empty() {
            return;
        }
        let room = self.palette;
        let zero = fader::y_of_db(0.0, self.limit, bounds);

        let mut quad = |x: f32, y: f32, w: f32, h: f32, colour: Color| {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: Rectangle {
                        x,
                        y,
                        width: w,
                        height: h,
                    },
                    border: Border::default(),
                    ..renderer::Quad::default()
                },
                colour,
            );
        };

        // **The datum, unbroken across the whole row.** Each fader draws this
        // across its own hit band, which leaves it dashed at the gaps; one
        // continuous line is what turns ten readings into one shape.
        quad(bounds.x, zero - 0.5, bounds.width, 1.0, room.paper_muted);

        // The band, column by column, from the datum to the response.
        //
        // Columns **tile** — each one starts where the last ended, computed
        // from rounded pixel boundaries rather than by overlapping a hair.
        // The fill is drawn with alpha, and overlapping alpha composites
        // twice: the seams show up as a picket fence across what is meant to
        // read as one region.
        #[expect(
            clippy::cast_precision_loss,
            reason = "a few hundred columns; exact in f32"
        )]
        let last = (self.curve.len() - 1).max(1) as f32;
        let step = bounds.width / last;
        let wash = theme::alpha(room.paper_muted, 0.22);
        let mut previous: Option<f32> = None;
        for (index, db) in self.curve.iter().enumerate() {
            #[expect(clippy::cast_precision_loss, reason = "as above")]
            let at = index as f32;
            let left = bounds.x + (at * step).round();
            let right = bounds.x + ((at + 1.0) * step).round();
            let width = (right - left).max(0.0);
            let y = fader::y_of_db(*db, self.limit, bounds);

            if width > 0.0 {
                let (top, height) = if y < zero {
                    (y, zero - y)
                } else {
                    (zero, y - zero)
                };
                // A flat curve is a flat line, and the datum already drew it.
                if height > 0.5 {
                    quad(left, top, width, height, wash);
                }
            }
            // **The edge is the curve.** Drawn from the previous sample's
            // height to this one rather than as a flat dash at each x, so a
            // steep side — a single band pushed to the limit — is a line
            // rather than a ladder of disconnected rungs.
            let from = previous.unwrap_or(y);
            let top = from.min(y) - 1.0;
            let height = (from - y).abs() + 2.0;
            quad(left, top, width.max(1.0), height, room.paper_dim);
            previous = Some(y);
        }
    }
}

impl<'a, Message, Renderer> From<Response> for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Renderer: renderer::Renderer + 'a,
{
    fn from(response: Response) -> Self {
        Self::new(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use baz_core::equalizer::{Bands, CENTRES, LIMIT_DB};

    fn bounds() -> Rectangle {
        Rectangle {
            x: 0.0,
            y: 0.0,
            width: 10.0 * fader::HIT_W + 9.0 * fader::GAP,
            height: 168.0,
        }
    }

    /// Where band `index`'s fader is centred, in the row's own coordinates.
    fn fader_centre_x(index: usize) -> f32 {
        #[expect(clippy::cast_precision_loss, reason = "ten bands")]
        let at = index as f32;
        at * (fader::HIT_W + fader::GAP) + fader::HIT_W / 2.0
    }

    /// Which curve sample lands nearest `x`.
    fn sample_at(x: f32, width: f32) -> usize {
        #[expect(clippy::cast_precision_loss, reason = "a few hundred columns")]
        let last = (POINTS - 1) as f32;
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "clamped into the sample range before the cast"
        )]
        let index = (x / width * last).round().clamp(0.0, last) as usize;
        index
    }

    /// **The curve peaks under the fader that raised it.**
    ///
    /// This is the whole claim the picture makes: that the bump you see is
    /// under the handle you pulled. It holds because the frequency axis and
    /// the column pitch are derived from each other, and this is the test that
    /// notices if one of them is changed alone.
    #[test]
    fn the_curve_peaks_under_the_fader_that_raised_it() {
        let (from_hz, to_hz) = span();
        for band in 0..CENTRES.len() {
            let mut db = [0.0_f32; 10];
            db[band] = 9.0;
            let mut curve = vec![0.0; POINTS];
            baz_core::equalizer::response_curve(
                Bands::from_db(db),
                0.0,
                from_hz,
                to_hz,
                &mut curve,
            );
            let peak = curve
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.total_cmp(b))
                .map(|(index, _)| index)
                .expect("a sampled curve");
            let want = sample_at(fader_centre_x(band), bounds().width);
            let drift = peak.abs_diff(want);
            assert!(
                drift <= 4,
                "band {band} ({} Hz) peaks {drift} samples from its own fader — \
                 the frequency axis and the column pitch have come apart",
                CENTRES[band]
            );
        }
    }

    /// **The curve passes through the handle.**
    ///
    /// Both use [`fader::y_of_db`], so this is checking that they are given
    /// the same bounds and the same limit — the way the two could still
    /// disagree once the mapping is shared.
    #[test]
    fn the_curve_meets_the_handle_it_belongs_to() {
        let (from_hz, to_hz) = span();
        let mut db = [0.0_f32; 10];
        db[5] = 6.0;
        let mut curve = vec![0.0; POINTS];
        baz_core::equalizer::response_curve(Bands::from_db(db), 0.0, from_hz, to_hz, &mut curve);

        let at = sample_at(fader_centre_x(5), bounds().width);
        let drawn = fader::y_of_db(curve[at], LIMIT_DB, bounds());
        let handle = fader::y_of_db(6.0, LIMIT_DB, bounds());
        assert!(
            (drawn - handle).abs() < 2.0,
            "the curve is {:.1} px from the handle at 1 kHz",
            drawn - handle
        );
    }

    /// **Neighbours add up, and the picture is why this widget exists.**
    ///
    /// Two adjacent bands at +6 read more than +6 between them. If this ever
    /// stopped being true the curve would be redundant with the handles and
    /// there would be no reason to draw it.
    #[test]
    fn the_curve_shows_what_the_handles_cannot() {
        let (from_hz, to_hz) = span();
        let mut db = [0.0_f32; 10];
        db[4] = 6.0;
        db[5] = 6.0;
        let mut curve = vec![0.0; POINTS];
        baz_core::equalizer::response_curve(Bands::from_db(db), 0.0, from_hz, to_hz, &mut curve);

        let low = sample_at(fader_centre_x(4), bounds().width);
        let high = sample_at(fader_centre_x(5), bounds().width);
        let between = curve[usize::midpoint(low, high)];
        assert!(
            between > 6.5,
            "two neighbouring +6 dB bands read {between:.1} dB between them — \
             either the filters stopped overlapping or the axis is wrong"
        );
    }

    /// **The pre-amp does not move the picture.**
    ///
    /// A listener who presses `Suggest a pre-amp` has their headroom pulled
    /// down by however much their largest boost was; if that came out of the
    /// drawn curve, every handle on the panel would stop touching it at once.
    /// The picture is of the *bias* — which bands are lifted over which — and
    /// an offset applied to all ten is not a bias.
    #[test]
    fn the_headroom_is_not_part_of_the_shape() {
        let db = [6.0, 3.0, 0.0, -3.0, 0.0, 0.0, 0.0, 2.0, 4.0, 4.0];
        let bands = Bands::from_db(db);
        let plain = Response::new(bands, LIMIT_DB, 168.0, theme::active());

        // The same bands, with the headroom the panel would suggest for them.
        let ducked = {
            let (from_hz, to_hz) = span();
            let mut curve = vec![0.0; POINTS];
            baz_core::equalizer::response_curve(
                bands,
                bands.suggested_preamp(),
                from_hz,
                to_hz,
                &mut curve,
            );
            curve
        };
        assert!(
            bands.suggested_preamp() < -1.0,
            "this fixture is meant to want real headroom"
        );
        for (drawn, with_headroom) in plain.curve.iter().zip(&ducked) {
            assert!(
                drawn > with_headroom,
                "the drawn curve moved with the pre-amp — it is showing level, \
                 not bias, and no handle on the panel touches it any more"
            );
        }
        // And the shape itself is untouched: the difference is one constant.
        let offset = plain.curve[0] - ducked[0];
        for (drawn, with_headroom) in plain.curve.iter().zip(&ducked) {
            assert!(
                ((drawn - with_headroom) - offset).abs() < 0.01,
                "the pre-amp is not a uniform offset after all"
            );
        }
    }

    /// **A flat curve is flat**, so an equaliser at rest draws a line and not
    /// a shape a listener has to interpret.
    #[test]
    fn nothing_asked_for_draws_nothing() {
        let (from_hz, to_hz) = span();
        let mut curve = vec![0.0; POINTS];
        baz_core::equalizer::response_curve(Bands::flat(), 0.0, from_hz, to_hz, &mut curve);
        for db in &curve {
            assert!(
                db.abs() < 0.01,
                "a flat equaliser draws a {db:.2} dB bump at rest"
            );
        }
    }

    /// **The span reaches the ends of the row**, not just the end bands — the
    /// curve is drawn edge to edge and a span that stopped at 31.5 Hz would
    /// leave the first half-column empty.
    #[test]
    fn the_span_covers_the_whole_row() {
        let (from_hz, to_hz) = span();
        assert!(
            from_hz < CENTRES[0],
            "the row starts at {from_hz:.1} Hz, inside its own first band"
        );
        assert!(
            to_hz > CENTRES[CENTRES.len() - 1],
            "the row ends at {to_hz:.0} Hz, inside its own last band"
        );
        // And not by much: a half column each side is a quarter octave-ish.
        assert!(from_hz > CENTRES[0] / 2.0 && to_hz < CENTRES[9] * 2.0);
    }
}
