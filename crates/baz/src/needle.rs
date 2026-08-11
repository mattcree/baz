//! The needle: a 2 px current-song seek line across the top of the persistent
//! playback bar.
//!
//! ADR-0017 §1.1 calls this the best single idea in any of the three design
//! documents, and the reason is a measurement rather than a preference. The
//! composition audit ranked the bottom bar's own contents by contrast-weighted
//! ink and found **position last of six**, at 2.5 %, while the seek row it was
//! drawn in occupied 37 of the bar's 77 px of content height. The needle states
//! the same position in 2 px without giving it a separate row.
//!
//! # What it is, precisely
//!
//! - **The fill is the lamp**; the unplayed track is the room's hairline. Both
//!   choices are argued in [`theme::needle`].
//! - **Every press is a seek within the current song.** The queue remains a
//!   list of explicit rows; the bar-wide line never pretends to measure it.
//!
//! # The aiming band is claimed downward, out of layout
//!
//! A 2 px mark is a 2 px target, which is a miss waiting to happen. The groove
//! solves that by *reserving* [`theme::RAIL_HIT`] and drawing a 4 px rail
//! centred in it; the needle cannot, because the whole bargain of ADR-0017
//! §1.1 is that it costs the collection 2 px rather than 45. So it reserves
//! [`theme::NEEDLE_H`] of layout and tests the pointer against a band
//! [`theme::NEEDLE_HIT`] tall reaching *down* into the empty lane the bar keeps
//! above its transport, and no further. That bound is
//! the safety property and it is asserted in [`theme`]:
//! `NEEDLE_HIT <= BAR_LEAD`, so a press aimed at Next can never be taken by a
//! line at the bottom of the window.
//!
//! Everything else about the pointer — the cursor, the 4 px click-versus-drag
//! threshold, tracking a held pointer past either end, and ending the gesture
//! when the pointer stops being ours — is [`crate::pointer`]'s, shared with
//! the fader, and the evidence for each rule is in [`crate::groove`]'s module
//! docs where it was gathered.

use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::widget::{Tree, tree};
use iced::advanced::{Clipboard, Shell, Widget};
use iced::widget::slider::Style;
use iced::{Element, Event, Length, Rectangle, Size, Theme, event, mouse};

use crate::player::{NeedleBar, Pointer};
use crate::pointer::{self, Pointers, State};
use crate::theme;

/// How the needle is painted in each of its states — the same shape the groove
/// takes, so [`crate::theme`] keeps expressing both hand-built controls with
/// `slider::Style` and the widgets stay a drawing detail.
type StyleFn = fn(&theme::Palette, iced::widget::slider::Status) -> Style;

/// The current song's seek line.
pub struct Needle<'a, Message> {
    /// The current song's position and interaction state.
    bar: NeedleBar,
    /// The room this needle is painted in, carried rather than looked up, so
    /// the widget draws the same room the view that built it did.
    palette: &'static theme::Palette,
    style: StyleFn,
    pointers: Option<Pointers<'a, Message>>,
}

impl<'a, Message> Needle<'a, Message> {
    /// A needle drawing `bar`, painted by `style`. Inert until
    /// [`Self::on_pointer`] wires it up.
    pub fn new(bar: NeedleBar, palette: &'static theme::Palette, style: StyleFn) -> Self {
        Self {
            bar,
            palette,
            style,
            pointers: None,
        }
    }

    /// Wires the pointer up: `press` when the button goes down inside the
    /// aiming band, `drag` for every move while it is held (including moves
    /// past either end), `hover` for moves with nothing held, `release` when
    /// the button comes up, and `exit` when the pointer leaves.
    #[must_use]
    pub fn on_pointer(
        mut self,
        press: impl Fn(Pointer) -> Message + 'a,
        drag: impl Fn(Pointer) -> Message + 'a,
        hover: impl Fn(Pointer) -> Message + 'a,
        release: Message,
        exit: Message,
    ) -> Self {
        self.pointers = Some(Pointers::new(press, drag, hover, release, exit));
        self
    }
}

/// The band the pointer may aim at: the drawn line, and the empty lane below
/// it (module docs; the bound is asserted in [`theme`]).
fn aim(bounds: Rectangle) -> Rectangle {
    Rectangle {
        x: bounds.x,
        y: bounds.y,
        width: bounds.width,
        height: theme::NEEDLE_HIT,
    }
}

impl<Message, Renderer> Widget<Message, Theme, Renderer> for Needle<'_, Message>
where
    Message: Clone,
    Renderer: renderer::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fixed(theme::NEEDLE_H))
    }

    fn layout(
        &self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        layout::atomic(limits, Length::Fill, Length::Fixed(theme::NEEDLE_H))
    }

    fn on_event(
        &mut self,
        tree: &mut Tree,
        event: Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) -> event::Status {
        let bounds = layout.bounds();
        pointer::handle(
            tree.state.downcast_mut::<State>(),
            self.pointers.as_ref(),
            &event,
            bounds,
            aim(bounds),
            cursor,
            shell,
        )
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        pointer::interaction(
            tree.state.downcast_ref::<State>(),
            self.pointers.is_some(),
            aim(layout.bounds()),
            cursor,
        )
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
        let state = tree.state.downcast_ref::<State>();
        let status = pointer::status(state, self.pointers.is_some(), aim(bounds), cursor);
        let style = (self.style)(self.palette, status);
        let (fill, track) = (style.rail.backgrounds.0, style.rail.backgrounds.1);
        // The line is drawn at the top of the reservation, where the playback
        // bar begins.
        let y = bounds.y;

        // The unplayed track is always present, so starting and stopping do
        // not make the window's edge appear or disappear.
        renderer.fill_quad(
            renderer::Quad {
                bounds: Rectangle {
                    x: bounds.x,
                    y,
                    width: bounds.width,
                    height: style.rail.width,
                },
                ..renderer::Quad::default()
            },
            track,
        );
        let filled = bounds.width * self.bar.position.clamp(0.0, 1.0);
        if filled > 0.0 {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: Rectangle {
                        x: bounds.x,
                        y,
                        width: filled,
                        height: style.rail.width,
                    },
                    ..renderer::Quad::default()
                },
                fill,
            );
        }
    }
}

impl<'a, Message, Renderer> From<Needle<'a, Message>> for Element<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Renderer: renderer::Renderer + 'a,
{
    fn from(needle: Needle<'a, Message>) -> Self {
        Self::new(needle)
    }
}

#[cfg(test)]
mod tests {
    use iced::advanced::clipboard;
    use iced::{Background, Point, Transformation, touch, window};

    use super::*;

    /// Everything a needle can say, tagged so a test can read the published
    /// stream back as the sequence of decisions the widget made.
    #[derive(Debug, Clone, PartialEq)]
    enum Msg {
        Press(Pointer),
        Drag(Pointer),
        Hover(Pointer),
        Release,
        Exit,
    }

    /// A renderer that draws nothing (see [`crate::groove`]'s twin, and the
    /// reason it is carried rather than borrowed from `iced_core`).
    struct Ink;

    impl renderer::Renderer for Ink {
        fn start_layer(&mut self, _bounds: Rectangle) {}
        fn end_layer(&mut self) {}
        fn start_transformation(&mut self, _transformation: Transformation) {}
        fn end_transformation(&mut self) {}
        fn fill_quad(&mut self, _quad: renderer::Quad, _background: impl Into<Background>) {}
        fn clear(&mut self) {}
    }

    /// Where the needle under test is laid out — deliberately away from the
    /// window origin, so a report that forgot to measure against it would be
    /// visibly wrong rather than accidentally right.
    const ORIGIN: Point = Point::new(100.0, 858.0);
    /// The needle's width under test.
    const WIDTH: f32 = 260.0;

    /// A cursor `x` px along the needle and `down` px into the bar.
    fn on_line(x: f32, down: f32) -> mouse::Cursor {
        mouse::Cursor::Available(Point::new(ORIGIN.x + x, ORIGIN.y + down + 0.5))
    }

    /// A cursor `x` px along, on the drawn line itself.
    fn at_line(x: f32) -> mouse::Cursor {
        on_line(x, 0.0)
    }

    /// A cursor inside the window but clear of the needle's aiming band.
    fn off_line(x: f32) -> mouse::Cursor {
        mouse::Cursor::Available(Point::new(ORIGIN.x + x, ORIGIN.y + theme::NEEDLE_HIT * 4.0))
    }

    /// What a report `x` px along the needle looks like once measured.
    fn at(x: f32) -> Pointer {
        Pointer::new(x, WIDTH)
    }

    /// One way of losing the pointer, applied to a needle under test.
    type Loss = fn(&mut Line) -> (event::Status, Vec<Msg>);

    /// One needle, its state tree, and its layout — driven event by event.
    struct Line {
        needle: Needle<'static, Msg>,
        tree: Tree,
        node: layout::Node,
        renderer: Ink,
    }

    fn bar(interactive: bool) -> NeedleBar {
        NeedleBar {
            position: 0.25,
            interactive,
            preview: None,
        }
    }

    impl Line {
        fn new(needle: Needle<'static, Msg>) -> Self {
            let tag = Widget::<Msg, Theme, Ink>::tag(&needle);
            let state = Widget::<Msg, Theme, Ink>::state(&needle);
            Self {
                needle,
                tree: Tree {
                    tag,
                    state,
                    children: Vec::new(),
                },
                node: layout::Node::new(Size::new(WIDTH, theme::NEEDLE_H)).move_to(ORIGIN),
                renderer: Ink,
            }
        }

        /// A live current-song line.
        fn live() -> Self {
            Self::new(
                Needle::new(bar(true), &theme::CLOSING_TIME, theme::needle).on_pointer(
                    Msg::Press,
                    Msg::Drag,
                    Msg::Hover,
                    Msg::Release,
                    Msg::Exit,
                ),
            )
        }

        /// An unseekable song line: no handlers, so no pointer.
        fn inert() -> Self {
            Self::new(Needle::new(
                bar(false),
                &theme::CLOSING_TIME,
                theme::needle_inert,
            ))
        }

        fn feed(&mut self, event: Event, cursor: mouse::Cursor) -> (event::Status, Vec<Msg>) {
            let mut messages = Vec::new();
            let mut shell = Shell::new(&mut messages);
            let status = self.needle.on_event(
                &mut self.tree,
                event,
                Layout::new(&self.node),
                cursor,
                &self.renderer,
                &mut clipboard::Null,
                &mut shell,
                &Rectangle::with_size(Size::new(1400.0, 1000.0)),
            );
            (status, messages)
        }

        fn press(&mut self, cursor: mouse::Cursor) -> Vec<Msg> {
            self.feed(
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
                cursor,
            )
            .1
        }

        fn moved(&mut self, cursor: mouse::Cursor) -> Vec<Msg> {
            let position = cursor.position().expect("a move needs a position");
            self.feed(Event::Mouse(mouse::Event::CursorMoved { position }), cursor)
                .1
        }

        fn released(&mut self, cursor: mouse::Cursor) -> Vec<Msg> {
            self.feed(
                Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
                cursor,
            )
            .1
        }

        fn cursor_left(&mut self) -> (event::Status, Vec<Msg>) {
            self.feed(
                Event::Mouse(mouse::Event::CursorLeft),
                mouse::Cursor::Unavailable,
            )
        }

        fn unfocused(&mut self, cursor: mouse::Cursor) -> (event::Status, Vec<Msg>) {
            self.feed(Event::Window(window::Event::Unfocused), cursor)
        }

        fn cursor(&self, cursor: mouse::Cursor) -> mouse::Interaction {
            Widget::<Msg, Theme, Ink>::mouse_interaction(
                &self.needle,
                &self.tree,
                Layout::new(&self.node),
                cursor,
                &Rectangle::with_size(Size::new(1400.0, 1000.0)),
                &self.renderer,
            )
        }
    }

    /// **The aiming band, measured.** A 2 px line has to be hittable without
    /// costing the collection more than 2 px, so the band is claimed downward
    /// out of layout — and the bound that makes that safe is that it stops
    /// inside the bar's empty top lane.
    #[test]
    fn the_aiming_band_reaches_down_from_the_line_and_stops_above_the_transport() {
        // The drawn line itself, and every pixel of the lane below it.
        for down in [0.0, 1.0, 5.0, theme::NEEDLE_HIT - 1.5] {
            let mut line = Line::live();
            assert_eq!(
                line.press(on_line(30.0, down)),
                vec![Msg::Press(at(30.0))],
                "{down} px below the bar's top edge is inside the band"
            );
        }
        // And one pixel past it is not — which is where the transport row
        // begins, and the reason `NEEDLE_HIT` is 12 rather than ADR-0017's 22.
        let mut line = Line::live();
        assert_eq!(
            line.press(on_line(30.0, theme::NEEDLE_HIT + 0.5)),
            vec![],
            "the band must not reach a control"
        );
        const { assert!(theme::NEEDLE_HIT <= theme::BAR_LEAD) }
    }

    /// The cursor is the whole of what a 2 px line can say about being
    /// pressable, so it says it across the aiming band and nowhere else.
    #[test]
    fn the_cursor_answers_across_the_band_and_only_on_a_live_needle() {
        let mut line = Line::live();
        assert_eq!(line.cursor(at_line(30.0)), theme::GROOVE_CURSOR);
        assert_eq!(line.cursor(on_line(30.0, 8.0)), theme::GROOVE_CURSOR);
        assert_eq!(line.cursor(off_line(30.0)), theme::GROOVE_CURSOR_INERT);
        line.press(at_line(30.0));
        assert_eq!(line.cursor(at_line(30.0)), theme::GROOVE_CURSOR_HELD);

        // An unseekable line leaves the cursor alone rather than
        // looking identical and doing nothing.
        let inert = Line::inert();
        assert_eq!(inert.cursor(at_line(30.0)), theme::GROOVE_CURSOR_INERT);
    }

    /// The gesture the fader has always had, inherited whole: press, drag past
    /// either end, release, and the hover that follows.
    #[test]
    fn an_ordinary_drag_released_inside_the_window_is_unchanged() {
        let mut line = Line::live();
        assert_eq!(line.press(at_line(10.0)), vec![Msg::Press(at(10.0))]);
        assert_eq!(line.moved(at_line(60.0)), vec![Msg::Drag(at(60.0))]);
        // A held needle still owns the pointer past either end of the line.
        assert_eq!(line.moved(off_line(-40.0)), vec![Msg::Drag(at(-40.0))]);
        assert_eq!(line.released(at_line(90.0)), vec![Msg::Release]);
        assert_eq!(line.moved(at_line(95.0)), vec![Msg::Hover(at(95.0))]);
        assert_eq!(line.moved(off_line(95.0)), vec![Msg::Exit]);
    }

    /// The bug that welded the fader to the pointer, asserted for the second
    /// widget built on the same machinery: the button comes up outside the
    /// window, baz never sees the release, and the gesture has to end anyway.
    #[test]
    fn a_drag_that_loses_the_pointer_ends_there_either_way_it_is_lost() {
        let losses: [Loss; 2] = [Line::cursor_left, |line| line.unfocused(at_line(60.0))];
        for lose in losses {
            let mut line = Line::live();
            assert_eq!(line.press(at_line(10.0)), vec![Msg::Press(at(10.0))]);
            assert_eq!(line.moved(at_line(60.0)), vec![Msg::Drag(at(60.0))]);
            assert_eq!(lose(&mut line).1, vec![Msg::Release, Msg::Exit]);
            // The button came up out there, unseen. Moving back is a hover,
            // not the scrub the bug turned it into.
            assert_eq!(line.moved(at_line(200.0)), vec![Msg::Hover(at(200.0))]);
            assert_eq!(line.moved(at_line(20.0)), vec![Msg::Hover(at(20.0))]);
        }
    }

    /// The fix must not swap one stuck flag for another: after a loss the
    /// hover is off, so a move that never touches the band says nothing.
    #[test]
    fn losing_the_pointer_cannot_strand_the_hover() {
        let mut line = Line::live();
        line.press(at_line(10.0));
        line.cursor_left();
        assert_eq!(
            line.moved(off_line(60.0)),
            vec![],
            "a stranded hover would have exited a second time here"
        );
        assert_eq!(line.moved(at_line(60.0)), vec![Msg::Hover(at(60.0))]);
        assert_eq!(line.moved(off_line(60.0)), vec![Msg::Exit]);
    }

    /// Losing the pointer is a broadcast fact, not an interaction: the fader
    /// and the needle are two widgets in one window and both have to hear it.
    #[test]
    fn losing_the_pointer_is_never_captured() {
        let mut line = Line::live();
        line.press(at_line(10.0));
        line.moved(at_line(60.0));
        assert_eq!(line.cursor_left().0, event::Status::Ignored);

        let mut line = Line::live();
        line.press(at_line(10.0));
        assert_eq!(line.unfocused(at_line(10.0)).0, event::Status::Ignored);
    }

    /// A finger that leaves the surface is cancelled rather than dropped, and
    /// a focus steal ends a touch gesture like any other.
    #[test]
    fn a_touch_gesture_ends_on_a_cancelled_finger_or_a_lost_focus() {
        let id = touch::Finger(1);
        let position = Point::new(ORIGIN.x + 60.0, ORIGIN.y);

        let mut line = Line::live();
        line.feed(
            Event::Touch(touch::Event::FingerPressed { id, position }),
            at_line(10.0),
        );
        assert_eq!(
            line.feed(
                Event::Touch(touch::Event::FingerLost { id, position }),
                at_line(60.0)
            )
            .1,
            vec![Msg::Release],
            "a cancelled finger is an ordinary release"
        );
    }

    /// An unseekable song line refuses the pointer entirely, so it has
    /// nothing to lose and nothing to say when it does.
    #[test]
    fn an_inert_needle_ignores_the_pointer_entirely() {
        let mut line = Line::inert();
        assert_eq!(line.press(at_line(10.0)), vec![]);
        assert_eq!(line.moved(at_line(60.0)), vec![]);
        assert_eq!(line.cursor_left(), (event::Status::Ignored, vec![]));
        assert_eq!(
            line.unfocused(at_line(60.0)),
            (event::Status::Ignored, vec![])
        );
    }

    /// The needle reserves [`theme::NEEDLE_H`] of layout and not one pixel
    /// more, in every state — the whole bargain of ADR-0017 §1.1 is that the
    /// collection pays 2 px for this and 2 px is what it pays.
    #[test]
    fn the_needle_reserves_two_pixels_and_nothing_else() {
        for interactive in [false, true] {
            let needle: Needle<'static, Msg> =
                Needle::new(bar(interactive), &theme::CLOSING_TIME, theme::needle);
            let size = Widget::<Msg, Theme, Ink>::size(&needle);
            assert_eq!(size.height, Length::Fixed(theme::NEEDLE_H));
            assert_eq!(size.width, Length::Fill);
        }
    }
}
