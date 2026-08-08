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
//! Every letter's size is a **pure function of where the pointer is right
//! now** — [`theme::magnify`] of the distance from the pointer to the slot's
//! rest centre — read in `draw` from the live cursor. There is no tween, no
//! clock, no subscription and no message: iced requests a redraw for every
//! window event, so the lens moves exactly while the pointer does and costs
//! nothing while it rests. When the pointer leaves the lane the input is gone
//! and the next frame is the rest frame — the snap back is a hard cut, argued
//! in the ADR.
//!
//! # Nothing moves, including under the lens
//!
//! The strip's slots are laid at [`theme::RAIL_PITCH`] and **never move**: a
//! swollen letter grows about its own fixed centre — leftwards into the lane
//! and its clearance, vertically into its own gap — because
//! `MAGNIFY_MAX × SIZE_HEADING` is inside one pitch (asserted in [`theme`]).
//! The dock displaces its icons; a product whose law is that nothing reflows
//! under the pointer does not, and holding the slots still is also what keeps
//! the deformation feedback-free: distances are measured against rest
//! geometry, so a resting pointer draws a stable frame. The lane's width is
//! untouched and the wall beside it cannot reflow by a pixel.
//!
//! # The hit lane is the whole lane
//!
//! The old rail's targets were the letters' own boxes — ~7 × 12 px for a
//! letter, with dead gaps between and a dead gutter beside. Here every press
//! inside the strip's band belongs to the **nearest slot**: targets are
//! [`theme::RAIL_PITCH`] tall, contiguous, and as wide as the whole lane —
//! clearance, ink and the [`theme::HANG`] gutter to the window's edge, so the
//! easiest gesture a pointer has (throw it at the edge) lands on the rail.
//! That is the hit region growing *ahead of* the glyphs, taken to its limit:
//! the target under a swollen letter is the same slot whatever its size, and a
//! glyph can never outgrow it. Absent values and elision marks stay inert
//! (`docs/REFUSALS.md`: a control that did nothing when pressed would be a
//! lie), and inert slots leave the cursor alone.

use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::text::{self, Paragraph, Text};
use iced::advanced::widget::Tree;
use iced::advanced::{Clipboard, Shell, Widget};
use iced::{
    Element, Event, Font, Length, Pixels, Point, Rectangle, Size, Theme, alignment, event, mouse,
    touch,
};

use crate::theme;

/// One slot of the strip, resolved by the view: what it says, where it jumps,
/// and whether it is where the wall is standing.
///
/// A slot with no `shelf` is an absent value or an elision mark — drawn in the
/// muted ink, inert to the pointer, and still magnified: the lens is optics
/// over the whole strip, not a statement about what is under it, and a dot
/// that froze while the letters around it swelled would read as a defect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slot {
    /// The entry's text — [`crate::rail::RailEntry::label`], or
    /// [`crate::rail::GAP_MARK`].
    pub label: String,
    /// The shelf a press jumps to, or `None` for a value the collection has
    /// nothing under.
    pub shelf: Option<usize>,
    /// Whether this is the shelf at the top of the viewport — full paper, in
    /// the medium face, exactly as the shelf's own header is.
    pub current: bool,
}

/// The index rail: the strip of slots, the room it is painted in, and the one
/// message it can send.
pub struct Spine<'a, Message> {
    slots: Vec<Slot>,
    /// The room, carried rather than looked up, so the widget draws the same
    /// room the view that built it did (the same argument as
    /// [`crate::needle`]).
    palette: &'static theme::Palette,
    jump: Box<dyn Fn(usize) -> Message + 'a>,
}

impl<'a, Message> Spine<'a, Message> {
    /// A rail over `slots`, publishing `jump` with the shelf a press landed
    /// on.
    pub fn new(
        slots: Vec<Slot>,
        palette: &'static theme::Palette,
        jump: impl Fn(usize) -> Message + 'a,
    ) -> Self {
        Self {
            slots,
            palette,
            jump: Box::new(jump),
        }
    }
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

/// The pointer's position, if the lane holds it — the fisheye's one input.
fn over(cursor: mouse::Cursor, bounds: Rectangle) -> Option<Point> {
    cursor
        .position()
        .filter(|position| bounds.contains(*position))
}

/// The slot under the pointer, if a press there would land on a shelf.
fn target_at(slots: &[Slot], bounds: Rectangle, cursor: mouse::Cursor) -> Option<usize> {
    let position = over(cursor, bounds)?;
    let top = bounds.y + strip_top(bounds.height, slots.len());
    let index = slot_at(top, slots.len(), position.y)?;
    slots[index].shelf.map(|_| index)
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
            && let Some(index) = target_at(&self.slots, layout.bounds(), cursor)
            && let Some(shelf) = self.slots[index].shelf
        {
            shell.publish((self.jump)(shelf));
            return event::Status::Captured;
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
        if target_at(&self.slots, layout.bounds(), cursor).is_some() {
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
        let count = self.slots.len();
        if count == 0 {
            return;
        }
        let top = bounds.y + strip_top(bounds.height, count);
        // The fisheye's one input. `None` — the pointer elsewhere, or gone —
        // is the rest frame, which is how the snap back on exit happens by
        // construction rather than by a code path (ADR-0020's amendment).
        let pointer = over(cursor, bounds);
        let hovered = target_at(&self.slots, bounds, cursor);
        // The ink's right edge: `W − HANG`, law L1's gutter, the same x the
        // old rail's padding produced.
        let right = bounds.x + bounds.width - theme::HANG;
        for (index, slot) in self.slots.iter().enumerate() {
            let center = slot_center(top, index);
            let scale = pointer.map_or(1.0, |position| theme::magnify(position.y - center));
            // At rest the ink's lane is INDEX_W, exactly as it was; a swollen
            // entry may take the clearance too, which is air the lane already
            // reserves on the wall's side of the ink.
            let cap = if scale > 1.0 {
                theme::INDEX_CLEARANCE + theme::INDEX_W
            } else {
                theme::INDEX_W
            };
            // The hover lift the buttons used to give — resting ink up to full
            // paper — kept, on the slot a press would actually take. The ink
            // wash chip behind the letter is not: the swell is the hover
            // statement now, and a chip under a growing letter was chrome in a
            // lane that is type (§7.2: *type, not chrome*).
            let ink = if slot.current || hovered == Some(index) {
                self.palette.paper
            } else if slot.shelf.is_some() {
                self.palette.paper_faint
            } else {
                self.palette.paper_muted
            };
            let font = if slot.current {
                theme::MEDIUM
            } else {
                theme::SANS
            };
            let size = theme::SIZE_HEADING * scale;
            let width = <Renderer::Paragraph as Paragraph>::with_text(label_text(
                slot.label.as_str(),
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
            let (text, anchor) = if width <= cap {
                (
                    label_text(slot.label.clone(), size, font, alignment::Horizontal::Right),
                    Point::new(right, center),
                )
            } else {
                (
                    label_text(slot.label.clone(), size, font, alignment::Horizontal::Left),
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
            let spine = Spine::new(strip(), &theme::CLOSING_TIME, Msg::Jump);
            Self {
                tree: Tree {
                    tag: Widget::<Msg, Theme, ()>::tag(&spine),
                    state: tree::State::None,
                    children: Vec::new(),
                },
                spine,
                node: layout::Node::new(Size::new(theme::INDEX_LANE_W, LANE_H)).move_to(ORIGIN),
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
    /// jumped somewhere would be the lie `docs/REFUSALS.md` names — and the
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
}
