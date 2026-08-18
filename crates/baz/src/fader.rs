//! **The equaliser's fader** — a vertical, bipolar control whose position *is*
//! the reading.
//!
//! The owner, 2026-08-18: *"sliders help show graphically how high the current
//! of frequencies is biased."* That sentence is the whole argument for this
//! widget existing, and it is an argument the stepper rows this replaces could
//! not answer: ten numbers in a column tell you each band's gain and never
//! tell you the **shape**, which is the only thing a graphic equaliser is for.
//! A row of faders is a curve you can read at a glance and grab in one gesture.
//!
//! # Why not [`crate::groove`]
//!
//! The seek bar and the volume fader are grooves, and a groove is horizontal
//! and unipolar by construction — `position` is `0..1`, its rail fills from the
//! left, and [`crate::pointer`] measures the cursor along `x`. Teaching it a
//! second axis would put an orientation branch through the layout, the draw and
//! the hit test of the two controls the product can least afford to disturb.
//!
//! And an equaliser fader is not a groove wearing a different coat. It is
//! **bipolar**: its rest is the middle, its fill runs *from* the middle in
//! whichever direction the band was pushed, and the middle is a value a
//! listener returns to often enough that it earns a detent. None of that is a
//! seek bar with the axes swapped.
//!
//! # What carries the reading
//!
//! **Position, and then length.** The handle's height above or below the zero
//! line is the gain, and the fill between them repeats it as a length — so the
//! curve is legible without telling any two colours apart, which is a standing
//! rule of this product rather than a courtesy.
//!
//! **And it is drawn in the room's own inks, not the lamp.** The first version
//! filled with [`theme::Palette::lamp`], and
//! `the_lamp_is_named_only_where_playback_truth_is_drawn` refused it — rightly.
//! The accent means *which album is sounding, which track, where the playhead
//! is*. A band's gain is a setting; painting it in the same amber would make
//! the accent mean two things, and the one it already means is the one the
//! product is built around. The ladder here is the room's greys, which is what
//! every other setting gets.

use iced::advanced::widget::{Tree, tree};
use iced::advanced::{Clipboard, Layout, Shell, Widget, layout, renderer};
use iced::{Border, Color, Element, Event, Length, Rectangle, Size, Theme, mouse, touch};

use crate::theme;

/// How wide a fader's hit band is.
///
/// [`theme::STEPPER_HIT`], the product's named secondary target — the same
/// number the row-slot controls take. The rail drawn inside it is far
/// narrower; the reservation is what the pointer aims at (law L7).
pub const HIT_W: f32 = theme::STEPPER_HIT;

/// **The gap between one fader and the next**, and the reason it is stated
/// here rather than at the row that draws it.
///
/// The owner: *"the lines of the graphic EQ do not seem to be spread out
/// enough."* They were on [`theme::GAP_XS`] — a 4 px seam between 32 px
/// columns, which is the rhythm of a *detent run* (one control with several
/// states, touching) rather than of ten separate controls. Ten faders that
/// nearly touch read as a fence.
///
/// It lives beside [`HIT_W`] because the pair is one fact — the **pitch** —
/// and [`crate::response`] derives the drawn curve's frequency axis from that
/// same pitch. Change the gap at the row alone and the curve slides off the
/// handles it is supposed to pass through; `the_curve_peaks_under_the_fader_that_raised_it`
/// would catch it, but only after somebody had to work out why.
pub const GAP: f32 = theme::GAP_MD;

/// The rail a fader draws inside its hit band.
const RAIL_W: f32 = 4.0;

/// The handle: wider than the rail so it reads as a grip, and short enough
/// that its centre is unambiguous.
const HANDLE_W: f32 = 20.0;

/// How tall the grip is.
///
/// Shared, because [`crate::response`] draws the curve the handles sit on and
/// the two have to agree to the pixel: the grip's own height is what insets
/// the travel from the top and bottom of the bounds, so a curve that did not
/// know it would pass near the handles instead of through them.
pub(crate) const HANDLE_H: f32 = 10.0;

/// **How close to the middle counts as the middle**, in pixels of travel.
///
/// A graphic equaliser's most-used value is zero — it is where every band
/// starts and what a listener returns a band to when they overdid it. Without
/// a detent, hitting it exactly with a drag is luck; with one, it is where the
/// handle wants to sit. Four pixels is the same snap the volume fader spends
/// on unity.
const DETENT_PX: f32 = 4.0;

/// **Where a decibel sits in a fader's bounds** — the one mapping.
///
/// A fader is drawn from this and so is the curve behind the row
/// ([`crate::response`]). They are the same function rather than two that
/// happen to agree, because "happen to agree" is a thing that stops being
/// true the first time one of the constants moves.
///
/// The top of the bounds is `+limit` and the bottom is `−limit`, both inset by
/// half a grip so the handle never hangs off the end of its own travel.
pub(crate) fn y_of_db(db: f32, limit: f32, bounds: Rectangle) -> f32 {
    let travel = (bounds.height - HANDLE_H).max(1.0);
    let fraction = (limit - db) / (2.0 * limit);
    bounds.y + HANDLE_H / 2.0 + travel * fraction.clamp(0.0, 1.0)
}

/// A vertical bipolar fader.
pub struct Fader<'a, Message> {
    /// The value, in decibels.
    db: f32,
    /// The travel, in decibels either side of zero.
    limit: f32,
    height: f32,
    palette: &'static theme::Palette,
    on_change: Box<dyn Fn(f32) -> Message + 'a>,
    on_release: Option<Message>,
}

/// The widget's own transient input state.
#[derive(Default)]
struct FaderState {
    /// The pointer went down on this fader and has not come up. While it is
    /// set the fader tracks the pointer **anywhere**, which is what lets a
    /// listener drag past the top of a short fader and still reach +12.
    held: bool,
    /// Accumulated wheel travel, so a high-resolution trackpad does not step
    /// the band on every pixel of movement.
    wheel: f32,
}

impl<'a, Message> Fader<'a, Message> {
    /// A fader at `db`, travelling `±limit`.
    pub fn new(
        db: f32,
        limit: f32,
        height: f32,
        palette: &'static theme::Palette,
        on_change: impl Fn(f32) -> Message + 'a,
    ) -> Self {
        Self {
            db,
            limit,
            height,
            palette,
            on_change: Box::new(on_change),
            on_release: None,
        }
    }

    /// What to send when the gesture ends — the moment a curve is worth
    /// writing to the config, as opposed to every pixel of a drag.
    #[must_use]
    pub fn on_release(mut self, message: Message) -> Self {
        self.on_release = Some(message);
        self
    }

    /// The decibel value a cursor at `y` asks for, with the centre detent
    /// applied.
    fn db_at(&self, y: f32, bounds: Rectangle) -> f32 {
        let travel = (bounds.height - HANDLE_H).max(1.0);
        let top = bounds.y + HANDLE_H / 2.0;
        let fraction = ((y - top) / travel).clamp(0.0, 1.0);
        // Top of the fader is +limit, bottom is −limit.
        let db = self.limit.mul_add(-2.0 * fraction, self.limit);
        let per_px = 2.0 * self.limit / travel;
        if db.abs() <= per_px * DETENT_PX {
            return 0.0;
        }
        db.clamp(-self.limit, self.limit)
    }

    /// Where the handle's centre sits for `db`.
    fn handle_y(&self, bounds: Rectangle) -> f32 {
        y_of_db(self.db, self.limit, bounds)
    }

    /// Where zero sits — the line the fill is measured from.
    fn zero_y(bounds: Rectangle) -> f32 {
        let travel = (bounds.height - HANDLE_H).max(1.0);
        bounds.y + HANDLE_H / 2.0 + travel / 2.0
    }
}

impl<Message, Renderer> Widget<Message, Theme, Renderer> for Fader<'_, Message>
where
    Message: Clone,
    Renderer: renderer::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<FaderState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(FaderState::default())
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fixed(HIT_W), Length::Fixed(self.height))
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        layout::atomic(limits, Length::Fixed(HIT_W), Length::Fixed(self.height))
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let state = tree.state.downcast_mut::<FaderState>();
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerPressed { .. }) => {
                if let Some(position) = cursor.position().filter(|at| bounds.contains(*at)) {
                    state.held = true;
                    shell.publish((self.on_change)(self.db_at(position.y, bounds)));
                    shell.capture_event();
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { .. })
            | Event::Touch(touch::Event::FingerMoved { .. }) => {
                // **Held means held, wherever the pointer went.** A fader is
                // 120 px tall and its travel is 24 dB; a listener reaching for
                // +12 will overshoot the top, and a drag that stopped at the
                // edge would make the ends the hardest values to set.
                if state.held
                    && let Some(position) = cursor.position()
                {
                    shell.publish((self.on_change)(self.db_at(position.y, bounds)));
                    shell.capture_event();
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerLifted { .. } | touch::Event::FingerLost { .. }) => {
                if state.held {
                    state.held = false;
                    if let Some(release) = self.on_release.clone() {
                        shell.publish(release);
                    }
                    shell.capture_event();
                }
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                if cursor.is_over(bounds) {
                    let travel = match delta {
                        mouse::ScrollDelta::Lines { y, .. } => *y,
                        mouse::ScrollDelta::Pixels { y, .. } => *y / 40.0,
                    };
                    state.wheel += travel;
                    // One decibel a notch: fine enough to tune with and coarse
                    // enough that a flick does not throw the band to an end.
                    let steps = state.wheel.trunc();
                    if steps.abs() >= 1.0 {
                        state.wheel -= steps;
                        let next = (self.db + steps).clamp(-self.limit, self.limit);
                        shell.publish((self.on_change)(next));
                        if let Some(release) = self.on_release.clone() {
                            shell.publish(release);
                        }
                    }
                    shell.capture_event();
                }
            }
            _ => {}
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        let held = tree.state.downcast_ref::<FaderState>().held;
        if held || cursor.is_over(layout.bounds()) {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::default()
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let state = tree.state.downcast_ref::<FaderState>();
        let hot = state.held || cursor.is_over(bounds);
        let room = self.palette;
        let mid_x = bounds.x + bounds.width / 2.0;
        let zero = Self::zero_y(bounds);
        let handle = self.handle_y(bounds);

        let mut quad = |x: f32, y: f32, w: f32, h: f32, colour: Color, radius: f32| {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: Rectangle {
                        x,
                        y,
                        width: w,
                        height: h,
                    },
                    border: Border {
                        radius: radius.into(),
                        ..Border::default()
                    },
                    ..renderer::Quad::default()
                },
                colour,
            );
        };

        // The rail, full travel, quiet.
        quad(
            mid_x - RAIL_W / 2.0,
            bounds.y + HANDLE_H / 2.0,
            RAIL_W,
            (bounds.height - HANDLE_H).max(0.0),
            room.plinth_lit,
            RAIL_W / 2.0,
        );
        // **The zero line**, drawn across the whole hit band rather than the
        // rail, so a row of faders reads as one curve against one datum.
        quad(
            bounds.x,
            zero - 0.5,
            bounds.width,
            1.0,
            room.paper_muted,
            0.0,
        );
        // The fill, from zero to the handle — the length that repeats what the
        // handle's position already says.
        let (fill_y, fill_h) = if handle < zero {
            (handle, zero - handle)
        } else {
            (zero, handle - zero)
        };
        if fill_h > 0.5 {
            quad(
                mid_x - RAIL_W / 2.0,
                fill_y,
                RAIL_W,
                fill_h,
                if hot {
                    room.paper_dim
                } else {
                    room.paper_muted
                },
                RAIL_W / 2.0,
            );
        }
        // The grip.
        quad(
            mid_x - HANDLE_W / 2.0,
            handle - HANDLE_H / 2.0,
            HANDLE_W,
            HANDLE_H,
            if hot { room.paper } else { room.paper_dim },
            2.0,
        );
    }
}

impl<'a, Message, Renderer> From<Fader<'a, Message>> for Element<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Renderer: renderer::Renderer + 'a,
{
    fn from(fader: Fader<'a, Message>) -> Self {
        Self::new(fader)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bounds() -> Rectangle {
        Rectangle {
            x: 0.0,
            y: 0.0,
            width: HIT_W,
            height: 120.0,
        }
    }

    fn fader(db: f32) -> Fader<'static, ()> {
        Fader::new(db, 12.0, 120.0, &theme::CLOSING_TIME, |_| ())
    }

    /// **The top is the top and the bottom is the bottom**, and the middle is
    /// exactly zero — the three positions a listener aims at.
    #[test]
    fn the_travel_maps_to_the_stated_range() {
        let b = bounds();
        let f = fader(0.0);
        assert!((f.db_at(b.y, b) - 12.0).abs() < 0.01, "the top is not +12");
        assert!(
            (f.db_at(b.y + b.height, b) + 12.0).abs() < 0.01,
            "the bottom is not −12"
        );
        assert!(
            (f.db_at(Fader::<()>::zero_y(b), b)).abs() < f32::EPSILON,
            "the middle is not 0"
        );
        // Past either end is the end, not a wilder number — a held drag
        // reports from wherever the pointer went.
        assert!((f.db_at(-500.0, b) - 12.0).abs() < 0.01);
        assert!((f.db_at(500.0, b) + 12.0).abs() < 0.01);
    }

    /// **Zero has a detent.** Near the middle the fader answers exactly zero,
    /// because zero is the value a band is returned to and hitting it by luck
    /// is not a control.
    #[test]
    fn the_middle_snaps() {
        let b = bounds();
        let f = fader(6.0);
        let zero = Fader::<()>::zero_y(b);
        for offset in [-DETENT_PX + 0.5, -1.0, 0.0, 1.0, DETENT_PX - 0.5] {
            assert!(
                f.db_at(zero + offset, b).abs() < f32::EPSILON,
                "{offset} px from the middle did not snap"
            );
        }
        // …and just outside it, the fader means what the pointer said.
        assert!(f.db_at(zero + DETENT_PX * 3.0, b).abs() > 0.5);
    }

    /// **The handle draws where the value says**, which is the property the
    /// whole widget exists for: position is the reading.
    #[test]
    fn the_handle_follows_the_value() {
        let b = bounds();
        let top = fader(12.0).handle_y(b);
        let middle = fader(0.0).handle_y(b);
        let bottom = fader(-12.0).handle_y(b);
        assert!(top < middle && middle < bottom, "the fader is upside down");
        assert!(
            (middle - Fader::<()>::zero_y(b)).abs() < f32::EPSILON,
            "a flat band does not rest on the zero line"
        );
        // Symmetric: +6 and −6 are the same distance from the middle.
        let up = middle - fader(6.0).handle_y(b);
        let down = fader(-6.0).handle_y(b) - middle;
        assert!((up - down).abs() < 0.01, "boost and cut are not mirrored");
    }

    /// **Round trip**: a handle drawn for a value, read back at its own
    /// position, is that value again.
    #[test]
    fn a_position_read_back_is_the_value_that_drew_it() {
        let b = bounds();
        for db in [-12.0_f32, -9.0, -5.0, 5.0, 9.0, 12.0] {
            let f = fader(db);
            let read = f.db_at(f.handle_y(b), b);
            assert!(
                (read - db).abs() < 0.3,
                "{db} dB drew a handle that reads {read}"
            );
        }
    }
}
