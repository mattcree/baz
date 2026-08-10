//! The spine: the index rail's own widget, and the fisheye that makes a
//! 10 px letter easy to land on.
//!
//! The owner's ask, verbatim: *"a magnification style attempt to allow you to
//! select things. like mac OS dock. you move your mouse and it makes the
//! hovered item bigger, and the surrounding ones."* The rail's letters are the
//! smallest type in the product in the tightest column, which is exactly the
//! acquisition problem a fisheye answers — the target under the pointer grows
//! without spending the screen that enlarging the whole strip would cost.
//!
//! Until now the rail was a `column` of `button`s, and that composition could
//! not carry the ask: a letter's size is layout, iced 0.13 styles cannot touch
//! layout, and a widget's neighbours know nothing of its hover. So the rail is
//! the **third hand-built widget** after [`crate::groove`] and
//! [`crate::needle`] (ADR-0017 §5 records why that is the norm for pointer
//! semantics now). Unlike those two it borrows nothing from
//! [`crate::pointer`]: it has no gesture — no drag, no held state, nothing to
//! lose with the pointer — and therefore **no state at all**. A press either
//! lands on a shelf and says so, or it does not and says nothing.
//!
//! # The deformation contract (ADR-0020's amendment)
//!
//! Every letter's size **and place** is a pure function of where the pointer
//! is right now — [`theme::magnify`] and [`theme::magnify_shift`] of the
//! distance from the pointer to the slot's rest centre — read in `draw` from
//! the live cursor. There is no tween, no clock, no subscription and no
//! message: iced requests a redraw for every window event, so the lens moves
//! exactly while the pointer does and costs nothing while it rests. When the
//! pointer leaves the lane the input is gone and the next frame is the rest
//! frame — the snap back is a hard cut, argued in the ADR.
//!
//! # The strip spreads under the lens — the dock's own mechanism
//!
//! It shipped scale-only first, bounded so no glyph left its 20 px slot, and
//! the owner's desktop verdict was *"make sure the magnification is more
//! dramatic"*. Drama needs room, and the dock's answer is where the room comes
//! from: glyphs **displace along the strip as well as scale**, positions given
//! by the integral of the scale function, so the swollen letters sit in space
//! their neighbours vacated and the pitch under the lens is the scaled pitch.
//! Distances are still measured against *rest* centres — the deformation has
//! no feedback through its own output, so a resting pointer draws a stable
//! frame — and the far field's shift is capped at the strip's real head-room
//! ([`theme::MAGNIFY_SPREAD`], which the widget's own elision capacity reserves), so
//! no letter is ever pushed out of the lane: on cramped strips the spread
//! degrades before anything clips. The lane's *width* is untouched and the
//! wall beside it cannot reflow by a pixel.
//!
//! # The hit lane is the whole lane, and it agrees with the lens
//!
//! The old rail's targets were the letters' own boxes — ~7 × 12 px for a
//! letter, with dead gaps between and a dead gutter beside. Here every press
//! inside the strip's band belongs to the **nearest slot**: targets are
//! [`theme::RAIL_PITCH`] tall, contiguous, and as wide as the whole lane —
//! clearance, ink and the [`theme::HANG`] gutter to the window's edge, so the
//! easiest gesture a pointer has (throw it at the edge) lands on the rail.
//!
//! Displacement does not move the targets, and that is a theorem rather than
//! a choice: `|d + shift(d)|` grows with `|d|` ([`theme::magnify_shift`]), so
//! the slot whose **displaced** centre is nearest the pointer is exactly the
//! slot whose rest centre is — the glyph the lens holds biggest, the slot the
//! press fires, and the chip the hover draws are one answer by construction.
//! And a displaced glyph cannot be stranded away from its target either: as
//! the pointer approaches a letter, `p + shift(c − p) → c`, so the glyph
//! *converges onto its own rest slot* under the arriving pointer rather than
//! sliding off it. Absent values and elision marks stay inert
//! (a standing rule of the product: a control that did nothing when pressed would be a
//! lie), and inert slots leave the cursor alone.
//!
//! # Hover says what a press would take
//!
//! Swelling alone did not read as *"this is what a click selects"* (the
//! owner's third finding), so the winning slot — [`target_at`]'s answer, never
//! any other letter — carries the rail's press vocabulary back from its
//! button days: the [`theme::Palette::ink_wash`] chip behind it and its ink
//! lifted to full paper, the same family `theme::group_key` gives the group
//! words in the top bar. Never the accent (an index is navigation, not
//! playback truth), and never on an absent letter — a highlight on a dead
//! value would promise a jump it cannot make.

use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::text::{self, Paragraph, Text};
use iced::advanced::widget::Tree;
use iced::advanced::{Clipboard, Shell, Widget};
use iced::{
    Element, Event, Font, Length, Pixels, Point, Rectangle, Size, Theme, alignment, event, mouse,
    touch,
};

use crate::{rail, theme};

/// One entry of the rail, resolved by the view: what it says, where it jumps,
/// and whether it is where the wall is standing.
///
/// An entry with no `shelf` is an absent value — drawn in the muted ink,
/// inert to the pointer, and still magnified: the lens is optics over the
/// whole strip, not a statement about what is under it, and a letter that
/// froze while its neighbours swelled would read as a defect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slot {
    /// The entry's text — [`crate::rail::RailEntry::label`]. (The `·` elision
    /// mark is the widget's own: the view never sees an elided rail.)
    pub label: String,
    /// The shelf a press jumps to, or `None` for a value the collection has
    /// nothing under.
    pub shelf: Option<usize>,
    /// Whether this is the shelf at the top of the viewport — full paper, in
    /// the medium face, exactly as the shelf's own header is.
    pub current: bool,
}

/// The index rail: the whole rail, unelided — the room it is painted in, the
/// focus its elision windows on, and the one message it can send.
///
/// **The widget elides, not the view**, and that is the fix for a shipped
/// bug rather than a preference: the view used to fit the rail against
/// `Shelf::grid_size`, whose height between scroll events is an estimate that
/// ignores the bottom bar — so at launch, and after every resize until the
/// next scroll, the rail it admitted was ~102 px (five slots) taller than the
/// lane and clipped at both ends (the owner: *"it goes off the edge of the
/// screen"*). The widget's `layout` is handed the lane's true bounds every
/// frame, so fitting here cannot disagree with the height that exists.
pub struct Spine<'a, Message> {
    entries: Vec<Slot>,
    /// The entry the elision keeps its window around — the current shelf's,
    /// so what survives is the part of the index you are standing in.
    focus: Option<usize>,
    /// The room, carried rather than looked up, so the widget draws the same
    /// room the view that built it did (the same argument as
    /// [`crate::needle`]).
    palette: &'static theme::Palette,
    jump: Box<dyn Fn(usize) -> Message + 'a>,
}

impl<'a, Message> Spine<'a, Message> {
    /// A rail over the whole of `entries`, publishing `jump` with the shelf a
    /// press landed on. What actually fits is decided against the widget's
    /// real bounds, per frame.
    pub fn new(
        entries: Vec<Slot>,
        focus: Option<usize>,
        palette: &'static theme::Palette,
        jump: impl Fn(usize) -> Message + 'a,
    ) -> Self {
        Self {
            entries,
            focus,
            palette,
            jump: Box::new(jump),
        }
    }

    /// The slots a lane this tall shows: the whole rail when it fits, the
    /// elided rail when it does not — [`crate::rail::elide`] against the
    /// capacity of the *real* height.
    fn visible(&self, lane_h: f32) -> Vec<rail::RailSlot> {
        rail::elide(self.entries.len(), capacity(lane_h), self.focus)
    }

    /// What one visible slot says and does: the entry's label, shelf and
    /// currency, or the `·` mark's nothing.
    fn resolved(&self, slot: rail::RailSlot) -> (&str, Option<usize>, bool) {
        match slot {
            rail::RailSlot::Gap => (rail::GAP_MARK, None, false),
            rail::RailSlot::Entry(index) => self
                .entries
                .get(index)
                .map_or((rail::GAP_MARK, None, false), |entry| {
                    (entry.label.as_str(), entry.shelf, entry.current)
                }),
        }
    }
}

/// How many slots a lane this tall holds: its height, less the lens's travel
/// at each end ([`theme::MAGNIFY_SPREAD`]), at [`theme::RAIL_PITCH`] a slot.
///
/// Reserving the travel means a strip this capacity admits always has the
/// air the fisheye pushes its extremes into — the spread never costs a letter
/// its place in the lane. It costs an elided rail a few entries; it costs the
/// ARTIST alphabet nothing at any window the wall ships at
/// (27 × 20 + 2 × 45 = 630).
fn capacity(lane_h: f32) -> usize {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a slot count floored from a non-negative height"
    )]
    let capacity =
        ((lane_h - 2.0 * theme::MAGNIFY_SPREAD).max(0.0) / theme::RAIL_PITCH).floor() as usize;
    capacity
}

/// The air a slot owns beyond its line box: [`theme::RAIL_PITCH`] less
/// [`theme::RAIL_LINE_H`], half above the box and half below.
const SLOT_AIR: f32 = theme::RAIL_PITCH - theme::RAIL_LINE_H;

/// The strip's height: `count` line boxes at the pitch, without the last
/// entry's trailing air — the sum a spaced `column` of line boxes reaches.
fn strip_height(count: usize) -> f32 {
    if count == 0 {
        return 0.0;
    }
    #[expect(
        clippy::cast_precision_loss,
        reason = "a rail holds tens of slots, far below f32's exact-integer range"
    )]
    let count = count as f32;
    count.mul_add(theme::RAIL_PITCH, -SLOT_AIR)
}

/// Where the strip begins: centred in the lane, which is where the old rail's
/// `align_y(Center)` hung it and therefore where the rest render already is.
fn strip_top(lane_h: f32, count: usize) -> f32 {
    (lane_h - strip_height(count)) / 2.0
}

/// A slot's rest centre — the fixed point it swells about, and the point every
/// distance is measured to.
fn slot_center(top: f32, index: usize) -> f32 {
    #[expect(
        clippy::cast_precision_loss,
        reason = "a rail holds tens of slots, far below f32's exact-integer range"
    )]
    let index = index as f32;
    index.mul_add(theme::RAIL_PITCH, top + theme::RAIL_LINE_H / 2.0)
}

/// Which slot owns `y`: the nearest one, if `y` is inside the strip's band.
///
/// Slots tile the band — each owns one [`theme::RAIL_PITCH`] centred on its
/// line box, the two ends rounded out by their own half-air — so between two
/// letters there is a boundary, never a hole. The band ends where the strip
/// does: the lane's empty head and foot belong to nobody.
fn slot_at(top: f32, count: usize, y: f32) -> Option<usize> {
    if count == 0 {
        return None;
    }
    let offset = y - (top - SLOT_AIR / 2.0);
    if offset < 0.0 {
        return None;
    }
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a slot index floored from a non-negative offset in a checked band"
    )]
    let index = (offset / theme::RAIL_PITCH).floor() as usize;
    (index < count).then_some(index)
}

/// Where a slot's centre stands under the lens: its rest centre plus the
/// lens's displacement, the shift capped at `air` — the room the strip really
/// has at each end — so the extremes spread into space that exists and never
/// past the lane's edge.
///
/// The cap only flattens the *far* field (both neighbours of any gap hit it
/// together, so their spacing returns to rest); it cannot reorder anything,
/// because a capped monotone function is still monotone. With no pointer on
/// the lane, this **is** [`slot_center`].
fn displaced_center(top: f32, index: usize, pointer: Option<f32>, air: f32) -> f32 {
    let center = slot_center(top, index);
    let Some(pointer) = pointer else {
        return center;
    };
    let shift = theme::magnify_shift(center - pointer);
    center + shift.clamp(-air, air)
}

/// The pointer's position, if the lane holds it — the fisheye's one input.
fn over(cursor: mouse::Cursor, bounds: Rectangle) -> Option<Point> {
    cursor
        .position()
        .filter(|position| bounds.contains(*position))
}

/// The visible slot under the pointer and the shelf it jumps to, if a press
/// there would land on one.
fn target_at<Message>(
    spine: &Spine<'_, Message>,
    shown: &[rail::RailSlot],
    bounds: Rectangle,
    cursor: mouse::Cursor,
) -> Option<(usize, usize)> {
    let position = over(cursor, bounds)?;
    let top = bounds.y + strip_top(bounds.height, shown.len());
    let index = slot_at(top, shown.len(), position.y)?;
    let (_, shelf, _) = spine.resolved(shown[index]);
    shelf.map(|shelf| (index, shelf))
}

/// A slot's text, ready to measure or to fill.
///
/// The wrap box is deliberately enormous rather than infinite: the buffer
/// behind a `fill_text` wraps at its box whatever `Wrapping::None` says of the
/// layout, and a genre tag long enough to reach 4096 px at this size has
/// earned whatever happens to it.
fn label_text<Content>(
    content: Content,
    size: f32,
    font: Font,
    horizontal: alignment::Horizontal,
) -> Text<Content, Font> {
    Text {
        content,
        bounds: Size::new(4096.0, 64.0),
        size: Pixels(size),
        line_height: text::LineHeight::Relative(theme::LEADING_HEADING),
        font,
        horizontal_alignment: horizontal,
        vertical_alignment: alignment::Vertical::Center,
        shaping: text::Shaping::Basic,
        wrapping: text::Wrapping::None,
    }
}

impl<Message, Renderer> Widget<Message, Theme, Renderer> for Spine<'_, Message>
where
    Renderer: renderer::Renderer + text::Renderer<Font = Font>,
{
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fixed(theme::INDEX_LANE_W), Length::Fill)
    }

    fn layout(
        &self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        layout::atomic(limits, Length::Fixed(theme::INDEX_LANE_W), Length::Fill)
    }

    fn on_event(
        &mut self,
        _tree: &mut Tree,
        event: Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) -> event::Status {
        if let Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
        | Event::Touch(touch::Event::FingerPressed { .. }) = event
        {
            let bounds = layout.bounds();
            let shown = self.visible(bounds.height);
            if let Some((_, shelf)) = target_at(self, &shown, bounds, cursor) {
                shell.publish((self.jump)(shelf));
                return event::Status::Captured;
            }
        }
        event::Status::Ignored
    }

    fn mouse_interaction(
        &self,
        _tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        // A hand over a jump, nothing over a gap: an inert slot says it cannot
        // act by leaving the cursor alone, exactly as the old rail's inert
        // entries — which were never buttons — did.
        let bounds = layout.bounds();
        let shown = self.visible(bounds.height);
        if target_at(self, &shown, bounds, cursor).is_some() {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::None
        }
    }

    fn draw(
        &self,
        _tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        // Elided against the height the lane really has, per frame — the
        // capacity can no longer disagree with the bounds (struct docs).
        let shown = self.visible(bounds.height);
        let count = shown.len();
        if count == 0 {
            return;
        }
        let top = bounds.y + strip_top(bounds.height, count);
        // The room the strip has at each end — what bounds the far field's
        // displacement, so the lens can never push a letter out of the lane.
        let air = strip_top(bounds.height, count).max(0.0);
        // The fisheye's one input. `None` — the pointer elsewhere, or gone —
        // is the rest frame, which is how the snap back on exit happens by
        // construction rather than by a code path (ADR-0020's amendment).
        let pointer = over(cursor, bounds).map(|position| position.y);
        let hovered = target_at(self, &shown, bounds, cursor).map(|(index, _)| index);
        // The ink's right edge: `W − HANG`, law L1's gutter, the same x the
        // old rail's padding produced.
        let right = bounds.x + bounds.width - theme::HANG;
        for (index, slot) in shown.iter().enumerate() {
            let (label, target, current) = self.resolved(*slot);
            let rest = slot_center(top, index);
            let center = displaced_center(top, index, pointer, air);
            let scale = pointer.map_or(1.0, |pointer| theme::magnify(pointer - rest));
            // At rest the ink's lane is INDEX_W, exactly as it was; a swollen
            // entry may take the clearance too, which is air the lane already
            // reserves on the wall's side of the ink.
            let cap = if scale > 1.0 {
                theme::INDEX_CLEARANCE + theme::INDEX_W
            } else {
                theme::INDEX_W
            };
            // The winning slot carries the press vocabulary the rail's buttons
            // had (module docs): the wash chip and the full-paper lift, on the
            // letter a press would take and never on an inert one.
            let winner = hovered == Some(index);
            let ink = if current || winner {
                self.palette.paper
            } else if target.is_some() {
                self.palette.paper_faint
            } else {
                self.palette.paper_muted
            };
            let font = if current { theme::MEDIUM } else { theme::SANS };
            let size = theme::SIZE_HEADING * scale;
            let width = <Renderer::Paragraph as Paragraph>::with_text(label_text(
                label,
                size,
                font,
                alignment::Horizontal::Right,
            ))
            .min_bounds()
            .width;
            let clip = Rectangle {
                x: right - cap,
                y: bounds.y,
                width: cap,
                height: bounds.height,
            };
            // Shrink-to-fit up to the cap, clipped there, exactly as the old
            // `rail_text` behaved: a short value hangs flush on the gutter's
            // edge, and one wider than the cap anchors left instead, so what
            // the clip costs is the tail — the head is the half you navigate
            // by.
            let fits = width <= cap;
            if winner {
                // The chip is the glyph's own box — the swollen line box by
                // the measured ink, exactly the geometry the old `button` gave
                // its wash — in the same wash, at the control radius.
                let boxed = width.min(cap);
                let line = theme::RAIL_LINE_H * scale;
                renderer.fill_quad(
                    renderer::Quad {
                        bounds: Rectangle {
                            x: if fits { right - boxed } else { right - cap },
                            y: center - line / 2.0,
                            width: boxed,
                            height: line,
                        },
                        border: iced::Border {
                            color: iced::Color::TRANSPARENT,
                            width: 0.0,
                            radius: theme::RADIUS_CTRL.into(),
                        },
                        ..renderer::Quad::default()
                    },
                    self.palette.ink_wash(self.palette.wall),
                );
            }
            let (text, anchor) = if fits {
                (
                    label_text(label.to_owned(), size, font, alignment::Horizontal::Right),
                    Point::new(right, center),
                )
            } else {
                (
                    label_text(label.to_owned(), size, font, alignment::Horizontal::Left),
                    Point::new(right - cap, center),
                )
            };
            renderer.fill_text(text, anchor, ink, clip);
        }
    }
}

impl<'a, Message, Renderer> From<Spine<'a, Message>> for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Renderer: renderer::Renderer + text::Renderer<Font = Font> + 'a,
{
    fn from(spine: Spine<'a, Message>) -> Self {
        Self::new(spine)
    }
}

#[cfg(test)]
mod tests {
    use iced::advanced::clipboard;
    use iced::advanced::widget::tree;

    use super::*;

    /// The one thing a spine can say.
    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Msg {
        Jump(usize),
    }

    /// Where the lane under test stands — away from the origin, so arithmetic
    /// that forgot to measure against it would be visibly wrong rather than
    /// accidentally right (the needle's tests make the same choice).
    const ORIGIN: Point = Point::new(1172.0, 56.0);
    /// The lane's height under test.
    const LANE_H: f32 = 640.0;

    /// A strip like ARTIST's: present letters, absent ones, and a mark.
    fn strip() -> Vec<Slot> {
        vec![
            Slot {
                label: "#".to_owned(),
                shelf: None,
                current: false,
            },
            Slot {
                label: "A".to_owned(),
                shelf: Some(0),
                current: true,
            },
            Slot {
                label: "B".to_owned(),
                shelf: None,
                current: false,
            },
            Slot {
                label: "·".to_owned(),
                shelf: None,
                current: false,
            },
            Slot {
                label: "Z".to_owned(),
                shelf: Some(7),
                current: false,
            },
        ]
    }

    /// A cursor `into` px into the lane's width, level with slot `index`'s
    /// rest centre plus `off`.
    fn at(into: f32, index: usize, off: f32) -> mouse::Cursor {
        let top = ORIGIN.y + strip_top(LANE_H, 5);
        mouse::Cursor::Available(Point::new(ORIGIN.x + into, slot_center(top, index) + off))
    }

    /// One spine, its (stateless) tree and its layout, driven event by event.
    struct Lane {
        spine: Spine<'static, Msg>,
        tree: Tree,
        node: layout::Node,
    }

    impl Lane {
        fn new() -> Self {
            Self::with(strip(), Some(1), LANE_H)
        }

        fn with(entries: Vec<Slot>, focus: Option<usize>, lane_h: f32) -> Self {
            let spine = Spine::new(entries, focus, &theme::CLOSING_TIME, Msg::Jump);
            Self {
                tree: Tree {
                    tag: Widget::<Msg, Theme, ()>::tag(&spine),
                    state: tree::State::None,
                    children: Vec::new(),
                },
                spine,
                node: layout::Node::new(Size::new(theme::INDEX_LANE_W, lane_h)).move_to(ORIGIN),
            }
        }

        fn press(&mut self, cursor: mouse::Cursor) -> (event::Status, Vec<Msg>) {
            let mut messages = Vec::new();
            let mut shell = Shell::new(&mut messages);
            let status = self.spine.on_event(
                &mut self.tree,
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
                Layout::new(&self.node),
                cursor,
                &(),
                &mut clipboard::Null,
                &mut shell,
                &Rectangle::with_size(Size::new(1280.0, 860.0)),
            );
            (status, messages)
        }

        fn cursor(&self, cursor: mouse::Cursor) -> mouse::Interaction {
            Widget::<Msg, Theme, ()>::mouse_interaction(
                &self.spine,
                &self.tree,
                Layout::new(&self.node),
                cursor,
                &Rectangle::with_size(Size::new(1280.0, 860.0)),
                &(),
            )
        }
    }

    /// **The strip's slots tile its band**: every y inside it belongs to
    /// exactly one slot, the boundaries fall between the letters, and the
    /// lane's empty head and foot belong to none.
    #[test]
    fn the_slots_tile_the_strip_and_nothing_else() {
        for count in [1_usize, 5, 27, 40] {
            let top = strip_top(LANE_H, count);
            // Centred: the air above the strip is the air below it.
            let below = LANE_H - top - strip_height(count);
            assert!(
                (top - below).abs() < 1e-3,
                "{count} slots: {top} vs {below}"
            );
            // Each slot owns its own centre…
            for index in 0..count {
                assert_eq!(slot_at(top, count, slot_center(top, index)), Some(index));
            }
            // …the band is contiguous and ordered…
            let head = top - SLOT_AIR / 2.0;
            let band = strip_height(count) + SLOT_AIR;
            let mut y = head + 0.25;
            let mut previous = 0;
            while y < head + band {
                let slot = slot_at(top, count, y).expect("inside the band");
                assert!(slot >= previous, "the band runs backwards at {y}");
                previous = slot;
                y += 0.25;
            }
            assert_eq!(previous, count - 1, "the band ends before its last slot");
            // …and it ends where the strip does.
            assert_eq!(slot_at(top, count, head - 0.5), None);
            assert_eq!(slot_at(top, count, head + band + 0.5), None);
        }
        assert_eq!(slot_at(strip_top(LANE_H, 0), 0, LANE_H / 2.0), None);
    }

    /// A press lands on the nearest letter's shelf **anywhere in the lane's
    /// width** — the clearance, the letter, and the gutter to the window's
    /// edge, which is what makes the screen edge a rail target (Fitts).
    #[test]
    fn a_press_anywhere_across_the_lane_jumps_to_the_nearest_letter() {
        for into in [
            1.0,
            theme::INDEX_CLEARANCE + 30.0,
            theme::INDEX_LANE_W - 1.0,
        ] {
            let mut lane = Lane::new();
            let (status, messages) = lane.press(at(into, 1, 0.0));
            assert_eq!(status, event::Status::Captured, "{into} px into the lane");
            assert_eq!(messages, vec![Msg::Jump(0)]);
        }
        // Between two letters there is a boundary, not a hole: half a pitch
        // above A's centre is still A's, less an air's width more is #'s.
        let mut lane = Lane::new();
        let inside = at(30.0, 1, -(theme::RAIL_PITCH / 2.0) + 0.5);
        assert_eq!(lane.press(inside).1, vec![Msg::Jump(0)]);
        // And the last present slot jumps to its own shelf, not its index.
        let mut lane = Lane::new();
        assert_eq!(lane.press(at(30.0, 4, 3.0)).1, vec![Msg::Jump(7)]);
    }

    /// Absent values and elision marks are drawn and **inert** — a gap that
    /// jumped somewhere would be the lie the product's standing rules names — and the
    /// lane outside the strip presses nothing.
    #[test]
    fn absent_values_and_the_lanes_empty_ends_press_nothing() {
        for index in [0, 2, 3] {
            let mut lane = Lane::new();
            let (status, messages) = lane.press(at(30.0, index, 0.0));
            assert_eq!(status, event::Status::Ignored);
            assert_eq!(messages, vec![], "slot {index} is inert");
        }
        // Above the strip and below it: the band does not stretch to fill the
        // lane, so the empty head and foot stay dead.
        let mut lane = Lane::new();
        let above = mouse::Cursor::Available(Point::new(ORIGIN.x + 30.0, ORIGIN.y + 10.0));
        assert_eq!(lane.press(above).1, vec![]);
        // And a press left of the lane is not the rail's at all.
        let mut lane = Lane::new();
        let outside = mouse::Cursor::Available(Point::new(ORIGIN.x - 5.0, ORIGIN.y + LANE_H / 2.0));
        assert_eq!(lane.press(outside), (event::Status::Ignored, vec![]));
    }

    /// The cursor is the hand over a jump and nothing anywhere else, exactly
    /// as the old rail's mix of buttons and inert text answered.
    #[test]
    fn the_cursor_answers_over_jumps_and_leaves_gaps_alone() {
        let lane = Lane::new();
        assert_eq!(lane.cursor(at(30.0, 1, 2.0)), mouse::Interaction::Pointer);
        assert_eq!(
            lane.cursor(at(theme::INDEX_LANE_W - 1.0, 4, 0.0)),
            mouse::Interaction::Pointer,
            "the gutter is part of the lane"
        );
        assert_eq!(lane.cursor(at(30.0, 0, 0.0)), mouse::Interaction::None);
        assert_eq!(lane.cursor(at(30.0, 3, 0.0)), mouse::Interaction::None);
        assert_eq!(
            lane.cursor(mouse::Cursor::Unavailable),
            mouse::Interaction::None
        );
    }

    /// The strip stands at the pitch the token documents and the capacity
    /// arithmetic budgets. (The 16 px pitch the old view actually rendered
    /// was the defect measured in docs/design/impl/index-magnification/.)
    #[test]
    fn the_strip_stands_at_the_documented_pitch() {
        assert!((slot_center(0.0, 1) - slot_center(0.0, 0) - theme::RAIL_PITCH).abs() < 1e-6);
        assert!((strip_height(1) - theme::RAIL_LINE_H).abs() < 1e-6);
        assert!((strip_height(27) - (27.0 * theme::RAIL_PITCH - SLOT_AIR)).abs() < 1e-3);
    }

    /// **The lens and the press agree on the winner, displaced or not**: for
    /// any pointer in the band, the slot whose displaced centre is nearest is
    /// the slot [`slot_at`] answers — and the displaced strip keeps its order
    /// and stays inside the air it was given.
    #[test]
    fn the_displaced_strip_keeps_its_order_and_its_winner() {
        let count = 27;
        let top = strip_top(LANE_H, count);
        let air = top.max(0.0);
        let head = top - SLOT_AIR / 2.0;
        let mut pointer = head + 0.1;
        while pointer < head + strip_height(count) + SLOT_AIR {
            let displaced: Vec<f32> = (0..count)
                .map(|index| displaced_center(top, index, Some(pointer), air))
                .collect();
            // Order preserved: displacement can spread the strip, never
            // shuffle it.
            for pair in displaced.windows(2) {
                assert!(pair[0] < pair[1], "slots crossed at pointer {pointer}");
            }
            // Bounded: nothing leaves the lane.
            for (index, position) in displaced.iter().enumerate() {
                let shift = position - slot_center(top, index);
                assert!(shift.abs() <= air + 1e-3, "slot {index} left the lane");
            }
            // The winner: nearest displaced centre == the press's slot.
            let nearest = (0..count)
                .min_by(|a, b| {
                    (displaced[*a] - pointer)
                        .abs()
                        .total_cmp(&(displaced[*b] - pointer).abs())
                })
                .expect("a nonempty strip");
            assert_eq!(
                slot_at(top, count, pointer),
                Some(nearest),
                "the lens and the press disagree at {pointer}"
            );
            // Off the slot boundaries, so the tie at an exact midpoint (which
            // both sides resolve consistently anyway) is not what we sample.
            pointer += 1.7;
        }
        // No pointer, no deformation: the displaced strip is the rest strip.
        for index in 0..count {
            assert!(
                (displaced_center(top, index, None, air) - slot_center(top, index)).abs()
                    < f32::EPSILON
            );
        }
        // And the entry exactly under the pointer does not move at all.
        let centre = slot_center(top, 13);
        assert!((displaced_center(top, 13, Some(centre), air) - centre).abs() < 1e-4);
    }

    /// A renderer that records what the spine draws: quads and text runs, so
    /// the chip and the lens can be asserted rather than eyeballed.
    #[derive(Default)]
    struct Sheet {
        quads: Vec<(Rectangle, iced::Color)>,
        texts: Vec<(String, Point, iced::Color, f32)>,
    }

    impl renderer::Renderer for Sheet {
        fn start_layer(&mut self, _bounds: Rectangle) {}
        fn end_layer(&mut self) {}
        fn start_transformation(&mut self, _transformation: iced::Transformation) {}
        fn end_transformation(&mut self) {}
        fn clear(&mut self) {}
        fn fill_quad(&mut self, quad: renderer::Quad, background: impl Into<iced::Background>) {
            let color = match background.into() {
                iced::Background::Color(color) => color,
                iced::Background::Gradient(_) => iced::Color::BLACK,
            };
            self.quads.push((quad.bounds, color));
        }
    }

    impl text::Renderer for Sheet {
        type Font = Font;
        type Paragraph = ();
        type Editor = ();

        const ICON_FONT: Font = Font::DEFAULT;
        const CHECKMARK_ICON: char = '0';
        const ARROW_DOWN_ICON: char = '0';

        fn default_font(&self) -> Font {
            Font::default()
        }

        fn default_size(&self) -> Pixels {
            Pixels(16.0)
        }

        fn fill_paragraph(
            &mut self,
            _paragraph: &Self::Paragraph,
            _position: Point,
            _color: iced::Color,
            _clip_bounds: Rectangle,
        ) {
        }

        fn fill_editor(
            &mut self,
            _editor: &Self::Editor,
            _position: Point,
            _color: iced::Color,
            _clip_bounds: Rectangle,
        ) {
        }

        fn fill_text(
            &mut self,
            text: Text<String, Font>,
            position: Point,
            color: iced::Color,
            _clip_bounds: Rectangle,
        ) {
            self.texts
                .push((text.content, position, color, text.size.0));
        }
    }

    /// What a lane draws for `cursor`, recorded.
    fn drawn(lane: &Lane, cursor: mouse::Cursor) -> Sheet {
        let mut sheet = Sheet::default();
        Widget::<Msg, Theme, Sheet>::draw(
            &lane.spine,
            &lane.tree,
            &mut sheet,
            &Theme::Dark,
            &renderer::Style {
                text_color: iced::Color::WHITE,
            },
            Layout::new(&lane.node),
            cursor,
            &Rectangle::with_size(Size::new(1280.0, 860.0)),
        );
        sheet
    }

    /// **Hover highlight, drawn and measured**: the wash chip sits behind the
    /// press winner and only the press winner — paper ink, swollen to the
    /// lens's peak — while an inert slot under the pointer gets the lens and
    /// nothing else, and a pointer off the lane draws the rest frame.
    #[test]
    fn the_chip_and_the_lift_land_on_the_press_winner_and_nowhere_else() {
        let room = &theme::CLOSING_TIME;
        // On Z (present, index 4): one chip, at Z's centre, in the wash.
        let sheet = drawn(&Lane::new(), at(30.0, 4, 0.0));
        assert_eq!(sheet.quads.len(), 1, "one chip behind the one winner");
        let (chip, wash) = sheet.quads[0];
        let z_center = slot_center(ORIGIN.y + strip_top(LANE_H, 5), 4);
        assert!((chip.center_y() - z_center).abs() < 0.5);
        assert!((chip.height - theme::RAIL_LINE_H * theme::MAGNIFY_MAX).abs() < 0.5);
        assert_eq!(wash, room.ink_wash(room.wall));
        // Z is at the peak, in paper; its far neighbours rest, in their inks.
        let z = &sheet.texts[4];
        assert!((z.3 - theme::SIZE_HEADING * theme::MAGNIFY_MAX).abs() < 1e-3);
        assert_eq!(z.2, room.paper);
        assert_eq!(sheet.texts[0].2, room.paper_muted, "# stays muted");
        assert!(
            (sheet.texts[0].3 - theme::SIZE_HEADING).abs() < 1e-3,
            "# rests"
        );
        // The drawn strip is the displaced strip: spread apart, in order.
        let top = ORIGIN.y + strip_top(LANE_H, 5);
        let air = strip_top(LANE_H, 5);
        for (index, (_, position, _, _)) in sheet.texts.iter().enumerate() {
            let expected = displaced_center(top, index, Some(z_center), air);
            assert!(
                (position.y - expected).abs() < 1e-3,
                "slot {index} drawn at {} instead of {expected}",
                position.y
            );
        }
        assert!(
            sheet.texts[3].1.y < sheet.texts[4].1.y,
            "the strip stays ordered under the lens"
        );

        // On # (absent, index 0): the lens swells it, but no chip and no lift
        // — a highlight on a dead value would promise a jump it cannot make.
        let sheet = drawn(&Lane::new(), at(30.0, 0, 0.0));
        assert!(sheet.quads.is_empty(), "no chip over an inert slot");
        assert_eq!(sheet.texts[0].2, room.paper_muted);
        assert!((sheet.texts[0].3 - theme::SIZE_HEADING * theme::MAGNIFY_MAX).abs() < 1e-3);

        // No pointer: the rest frame — no chip, rest sizes, rest places.
        let sheet = drawn(&Lane::new(), mouse::Cursor::Unavailable);
        assert!(sheet.quads.is_empty());
        for (index, (_, position, _, size)) in sheet.texts.iter().enumerate() {
            assert!((size - theme::SIZE_HEADING).abs() < 1e-3);
            let rest = slot_center(ORIGIN.y + strip_top(LANE_H, 5), index);
            assert!((position.y - rest).abs() < 1e-3);
        }
    }

    /// **The rail fits the lane it is given, for every key at every height** —
    /// the pin for the owner's "it goes off the edge of the screen". The
    /// capacity used to be computed in the view from a height that ignored the
    /// bottom bar; it is a function of the widget's real height now, and this
    /// asserts the arithmetic can never admit a strip taller than the lane,
    /// with the lens's travel still reserved on top.
    #[test]
    fn the_rail_never_outgrows_the_lane_it_is_given() {
        use baz_core::history::Recency;
        use baz_core::index::{GroupKey, Initial};

        use crate::vm::GroupHeaderVm;

        let genres: Vec<GroupHeaderVm> = (0..70)
            .map(|n| {
                let letter = char::from(b'a' + u8::try_from(n % 26).expect("a letter"));
                GroupHeaderVm::Genre(Some(format!("{letter}enre {n}")))
            })
            .collect();
        let decades: Vec<GroupHeaderVm> = (0..70)
            .map(|n| GroupHeaderVm::Decade(Some(1300 + 10 * n)))
            .collect();
        let artists: Vec<GroupHeaderVm> = ('A'..='Z')
            .chain('\u{4e00}'..='\u{4e2f}')
            .map(|letter| GroupHeaderVm::Initial(Initial::Letter(letter)))
            .collect();
        let played: Vec<GroupHeaderVm> = [Recency::ThisEvening, Recency::YearsAgo(40)]
            .into_iter()
            .map(GroupHeaderVm::Recency)
            .collect();
        for (key, headers) in [
            (GroupKey::Genre, genres),
            (GroupKey::Year, decades),
            (GroupKey::Artist, artists),
            (GroupKey::Played, played),
        ] {
            let entries = rail::entries(key, &headers);
            assert!(entries.len() >= 26, "{key:?} is not an adversarial rail");
            let mut lane_h = 0.0;
            while lane_h < 1400.0 {
                for focus in [
                    None,
                    Some(0),
                    Some(entries.len() / 2),
                    Some(entries.len() - 1),
                ] {
                    let shown = rail::elide(entries.len(), capacity(lane_h), focus).len();
                    assert!(
                        shown == 0
                            || strip_height(shown) + 2.0 * theme::MAGNIFY_SPREAD <= lane_h + 1e-3,
                        "{key:?} at {lane_h}: {shown} slots do not fit"
                    );
                }
                lane_h += 13.7;
            }
        }
    }

    /// **The widget elides against the bounds it truly has**: sixty entries in
    /// a short lane draw as first + window + last with `·` marks, every glyph
    /// inside the lane — and a press routes through the same elided strip the
    /// frame shows.
    #[test]
    fn a_short_lane_shows_the_elided_strip_it_can_hold() {
        let entries: Vec<Slot> = (0..60)
            .map(|n| Slot {
                label: format!("E{n}"),
                shelf: Some(n),
                current: n == 30,
            })
            .collect();
        let short = 300.0;
        let lane = Lane::with(entries, Some(30), short);
        let sheet = drawn(&lane, mouse::Cursor::Unavailable);
        let fits = capacity(short);
        assert!(fits < 60, "the fixture must not fit whole");
        assert_eq!(sheet.texts.len(), fits, "the lane shows what fits, fully");
        // The ends survive, the window holds the focus, the marks are drawn.
        assert_eq!(sheet.texts.first().expect("a first slot").0, "E0");
        assert_eq!(sheet.texts.last().expect("a last slot").0, "E59");
        assert!(sheet.texts.iter().any(|(label, ..)| label == "E30"));
        assert!(
            sheet
                .texts
                .iter()
                .any(|(label, ..)| label == rail::GAP_MARK)
        );
        // Every glyph inside the lane — the owner's bug was exactly this line.
        for (label, position, _, _) in &sheet.texts {
            assert!(
                position.y > ORIGIN.y && position.y < ORIGIN.y + short,
                "{label} drawn at {} in a lane from {} to {}",
                position.y,
                ORIGIN.y,
                ORIGIN.y + short
            );
        }
        // A press mid-lane fires the entry the elided strip really shows
        // there, not the entry an unelided rail would have put at that y.
        let mut lane = Lane::with(
            (0..60)
                .map(|n| Slot {
                    label: format!("E{n}"),
                    shelf: Some(n),
                    current: n == 30,
                })
                .collect(),
            Some(30),
            short,
        );
        let top = ORIGIN.y + strip_top(short, fits);
        let centre_slot = fits / 2;
        let cursor =
            mouse::Cursor::Available(Point::new(ORIGIN.x + 30.0, slot_center(top, centre_slot)));
        let (status, messages) = lane.press(cursor);
        assert_eq!(status, event::Status::Captured);
        let jumped = match messages.as_slice() {
            [Msg::Jump(shelf)] => *shelf,
            other => panic!("expected one jump, got {other:?}"),
        };
        // The elided window is centred on the focus (30), so the middle of
        // the strip is near it — nowhere near `centre_slot` itself, which is
        // what an unelided mapping would have fired.
        assert!(
            (25..=35).contains(&jumped),
            "a press mid-strip jumped to {jumped}, outside the focus window"
        );
    }
}
