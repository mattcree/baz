//! The groove: a horizontal rail with a handle, as a custom iced widget.
//!
//! Two of the bottom bar's controls are grooves — the seek bar and the volume
//! fader — and they are the *same* widget deliberately. Everything that made
//! the seek bar worth writing by hand (a cursor that says "clickable", a
//! hover position to preview from, a press-to-release movement threshold, and
//! tracking a held pointer wherever it wanders) is exactly what a volume
//! fader needs, and a second widget would be a second place for those lessons
//! to be forgotten. The only thing the fader adds is an optional
//! [`Detent`] — a mark on the travel, drawn where it can be seen rather than
//! where the handle will cover it.
//!
//! This is view-layer code (ADR-0006 layer 3) and holds **no** playback
//! state: it measures the pointer against its own bounds and reports what it
//! saw as a [`Pointer`]; every decision about what that means — click or
//! scrub, preview or not, what position to seek to, whether the detent is
//! engaged — belongs to [`crate::player`], where it is pure and unit-tested.
//! The widget's own `held`/`hovered` flags exist only to know which raw
//! events to forward and which cursor to ask for.
//!
//! # Why not `slider`, and why not `mouse_area`
//!
//! Both were tried against the three affordances the bar needs — a cursor
//! that says "clickable", a hover position to preview from, and a
//! press-to-release movement threshold — and both fall short in iced 0.13
//! (versions inspected: `iced_widget` 0.13.4, `iced_core` 0.13.2):
//!
//! - **`slider`** reports only *values*, never pointer geometry: `on_change`
//!   fires on press and on every drag step with a range value, and
//!   `on_release` carries nothing. A pixel threshold cannot be expressed
//!   from that stream without hard-coding the widget's width, and the
//!   cursor is fixed at `Grab`/`Grabbing` by its own `mouse_interaction`
//!   with no way to override it.
//! - **`mouse_area`** reports positions (`on_move` gives coordinates local
//!   to the area) and can set a cursor, but its event handling returns early
//!   unless the cursor is *over* the area: a release that lands one pixel
//!   outside publishes nothing, so a scrub dragged off the end of the bar
//!   would never end — the bar would stay stuck to the pointer forever. It
//!   also has no press *position* (`on_press` is a bare message), so the
//!   anchor of a gesture would have to be guessed from the last hover.
//!
//! A grabbing widget has to keep tracking the pointer once it has captured
//! it, which is exactly what `Widget::on_event` allows and neither of the
//! two built-ins exposes. Everything visual still comes from
//! [`crate::theme`] — this module draws with tokens, it does not invent
//! them.
//!
//! # Losing the pointer
//!
//! Tracking a held pointer wherever it wanders raises the question the
//! toolkit does not answer for us: what happens when the pointer stops being
//! ours? A gesture that can only be ended by a release we might never hear
//! is a gesture that can last forever.
//!
//! **iced 0.13 has no pointer grab, and no capture of any other kind.** The
//! only "grab" in the API is [`mouse::Interaction::Grab`]/`Grabbing`, which
//! are cursor *pictures* — `iced_winit` maps them straight to
//! `winit::window::CursorIcon` and nothing else. `iced::window`'s task API
//! (open, close, resize, move, `gain_focus`, `enable_mouse_passthrough`, …)
//! has no entry that holds the pointer, and `winit`'s own
//! `Window::set_cursor_grab` is neither wrapped nor re-exported anywhere in
//! `iced` 0.13.1 / `iced_core` 0.13.2 / `iced_runtime` 0.13.2 /
//! `iced_winit` 0.13.0. `event::Status::Captured` is a *routing* verdict for
//! one event within one frame, not a claim on the device. So there is
//! nothing baz can *ask* for that would make the pointer stay ours.
//!
//! What the platforms give unasked is an **implicit** grab, and it is worth
//! stating what was measured rather than assumed (winit 0.30.13, a probe
//! printing every `WindowEvent`, driven by synthetic input on a private
//! display):
//!
//! - **X11** (Xvfb, `WINIT_UNIX_BACKEND=x11`): the button press starts X11's
//!   implicit passive grab. Dragging out of the window emits `CursorLeft`
//!   **and then keeps delivering** `CursorMoved` with out-of-window
//!   coordinates, and the `ButtonReleased` is delivered wherever the button
//!   comes up. Dragging back in while held emits `CursorEntered`.
//! - **Wayland** (a private headless `sway`/wlroots session): the compositor
//!   holds pointer focus for the whole press, so `CursorLeft` is **never**
//!   emitted while the button is down; motion continues past the surface
//!   edge, and the release arrives, followed by `CursorLeft`.
//!
//! So the ordinary "drag off the window and let go" *does* work by itself on
//! both. The failure this section exists for is the other one: something
//! else takes the pointer — a compositor overview or lock, another client's
//! popup grab, an alt-tab, a focus steal — the grab is broken, and the
//! release we are waiting for is delivered to them. Nothing tells us
//! directly; we simply never hear the button come up, and `held` stays set
//! forever. The groove is then welded to the pointer: every later move
//! scrubs the seek bar, and (worse, because the fader commits on every step)
//! drags the listener's volume around the screen until they think to click
//! the bar again. That is the reported bug.
//!
//! Since no event says "your grab was broken", the widget ends the gesture
//! on the events that say the pointer is no longer demonstrably ours:
//!
//! - [`mouse::Event::CursorLeft`] — it left the window. On Wayland this
//!   cannot happen mid-press unless the grab is already gone, so it is
//!   exactly the signal wanted. On X11 it also fires on an ordinary drag
//!   past the window edge, and ending there is a deliberate, stated price:
//!   a drag that crosses an X11 window's edge now commits at the edge
//!   instead of continuing. That is a bounded, self-evident early commit —
//!   the value is the one the user was looking at, and pressing again
//!   resumes — weighed against a control that stays stuck to the pointer
//!   with no way back but a click. Treating it as ambiguous instead (wait,
//!   and let a following move re-confirm the grab) was considered and
//!   rejected: in the owner's report the groove *was* still receiving motion
//!   while the pointer was elsewhere, so a rule that lets motion resurrect
//!   the gesture would not have fixed the thing that went wrong.
//! - [`window::Event::Unfocused`] — another window took focus mid-drag,
//!   which is the common real path (click something else while holding the
//!   fader) and the one that costs the pointer *without* necessarily moving
//!   it off our surface. It too can fire while a grab is still alive —
//!   switching workspaces mid-press on the sway probe emitted it and the
//!   release still arrived — and it ends the gesture anyway, on the same
//!   reasoning: a window that is no longer the one being talked to has no
//!   business still following the hand.
//!
//! Touch needs no third case: a finger that leaves the surface is
//! *cancelled* rather than dropped, and `winit` reports the cancellation as
//! [`touch::Event::FingerLost`], which is already handled beside
//! `FingerLifted` as an ordinary release. `Unfocused` covers the touch
//! equivalent of the focus steal for free, since the arm is shared.
//!
//! Neither arm returns [`event::Status::Captured`]. Losing the pointer is a
//! broadcast fact, not an interaction to consume: the seek bar and the
//! fader are two grooves in one window and *both* have to hear it, and so
//! does everything else that tracks the pointer or the focus.
//!
//! ## Commit, not cancel — and the same on both bars
//!
//! Ending the gesture publishes the *same* `release` the ordinary path
//! publishes, so the position under the pointer when we last saw it is the
//! position that gets asked for. It is a commit, not a rollback, and that is
//! deliberate on both controls:
//!
//! - **The fader has already committed.** A volume drag asks for every step
//!   as it happens ([`crate::player::PlayerState::drag_volume`]) — the
//!   listener has been *hearing* the last position for as long as the drag
//!   has been at it. "Cancelling" would mean sending a fresh `SetVolume`
//!   back to the pre-drag level: a real, audible change caused by nothing
//!   the listener did, and one the state machine would have to grow a
//!   restore path to express. Silence-by-surprise is a worse failure than
//!   any position they can see the handle sitting on.
//! - **The seek bar has been showing it.** A scrub renders the pointer's
//!   position in place of the engine's reports for the whole gesture, so the
//!   number under the pointer at the boundary is the number the user was
//!   looking at when the pointer left. Snapping back to the engine's
//!   position would be a visible jump they did not ask for, and
//!   indistinguishable from baz having dropped the input. A sub-threshold
//!   gesture commits to where the button went *down*, exactly as an ordinary
//!   click does — the aim was that spot, and the boundary crossing does not
//!   change it.
//! - **They must agree.** Two grooves that are deliberately one widget
//!   cannot answer "what does leaving the window do" two different ways
//!   without making both unpredictable.
//!
//! This also matches what toolkits do when a capture is broken off — the
//! slider keeps the value it was last dragged to; discarding one is an
//! explicit gesture (Escape), not a side effect of where the mouse went.
//!
//! The hover goes with the drag: the arm clears `hovered` and publishes
//! `exit` too, so the preview cannot be stranded on screen by a departure
//! that skipped the `CursorMoved` that normally retires it.

use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::widget::{Tree, tree};
use iced::advanced::{Clipboard, Shell, Widget};
use iced::widget::slider::{HandleShape, Style};
use iced::{
    Border, Element, Event, Length, Point, Rectangle, Size, Theme, event, mouse, touch, window,
};

use crate::player::Pointer;
use crate::theme;

/// How the groove is painted in each of its states. The same shape iced's
/// own slider uses, so [`crate::theme`] keeps expressing the bar with
/// `slider::Style` and this widget stays a drawing detail.
///
/// It takes a [`theme::Palette`] rather than iced's `Theme`, because that is
/// what every style function in baz takes since ADR-0017 §1.5: the room is
/// resolved at startup and carried, and iced's five-colour `Theme` never had
/// anything this widget could ask it.
type StyleFn = fn(&theme::Palette, iced::widget::slider::Status) -> Style;

/// A marked position on the travel — the volume fader's unity detent.
///
/// It is drawn *above* the rail rather than on it, clear of the handle by
/// [`theme::DETENT_GAP`], because a mark the handle covers is a mark that
/// disappears at exactly the position it exists to advertise. `engaged` is
/// pure state decided in [`crate::player`] (the control sits on the detent),
/// never a float comparison made here.
#[derive(Debug, Clone, Copy)]
pub struct Detent {
    /// Where the mark sits along the travel, `0.0..=1.0`.
    pub at: f32,
    /// Whether the handle is currently on it.
    pub engaged: bool,
}

/// The pointer handlers a live groove reports through. Absent on an inert
/// one (a track of undeclared length), which is how the widget knows to
/// ignore the pointer entirely rather than to look identical and do nothing.
struct Pointers<'a, Message> {
    press: Box<dyn Fn(Pointer) -> Message + 'a>,
    drag: Box<dyn Fn(Pointer) -> Message + 'a>,
    hover: Box<dyn Fn(Pointer) -> Message + 'a>,
    release: Message,
    exit: Message,
}

/// A horizontal groove with a handle at `position`, reporting raw pointer
/// geometry.
pub struct Groove<'a, Message> {
    position: f32,
    width: Length,
    height: f32,
    /// The room this groove is painted in, carried rather than looked up, so
    /// the widget draws the same room the view that built it did.
    palette: &'static theme::Palette,
    style: StyleFn,
    detent: Option<Detent>,
    pointers: Option<Pointers<'a, Message>>,
}

/// The widget's own transient input state — not playback state.
#[derive(Default)]
struct State {
    /// The pointer went down on the groove and has not come up yet. While
    /// this is set the widget tracks the pointer wherever it goes, which is
    /// what makes a scrub off the end of the bar (and the release that ends
    /// it) work at all.
    ///
    /// "Has not come up yet" is only knowable while the pointer is still
    /// ours, so this is also cleared by every event that says it is not —
    /// see the module's "Losing the pointer".
    held: bool,
    /// The pointer is resting on the groove, so a departure is worth
    /// reporting.
    hovered: bool,
}

impl<'a, Message> Groove<'a, Message> {
    /// A groove whose handle sits at `position` (`0.0..=1.0`), painted by
    /// `style`. Inert until [`Self::on_pointer`] wires it up.
    pub fn new(position: f32, palette: &'static theme::Palette, style: StyleFn) -> Self {
        Self {
            position,
            width: Length::Fill,
            height: theme::RAIL_HIT,
            palette,
            style,
            detent: None,
            pointers: None,
        }
    }

    /// Marks `detent` on the travel (see [`Detent`]).
    #[must_use]
    pub fn detent(mut self, detent: Detent) -> Self {
        self.detent = Some(detent);
        self
    }

    /// Sets the groove's width.
    #[must_use]
    pub fn width(mut self, width: Length) -> Self {
        self.width = width;
        self
    }

    /// Sets the groove's hit height — the band the pointer may aim at, not
    /// the thickness of the drawn rail (see [`theme::HIT_SLOP`]).
    #[must_use]
    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }

    /// Wires the pointer up: `press` when the button goes down on the bar,
    /// `drag` for every move while it is held (including moves past either
    /// end of the bar), `hover` for moves with nothing held, `release` when
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
        self.pointers = Some(Pointers {
            press: Box::new(press),
            drag: Box::new(drag),
            hover: Box::new(hover),
            release,
            exit,
        });
        self
    }
}

/// Where `position` falls on `bounds`, in the bar's own coordinates. `x` may
/// land outside `0..=width`; clamping is the state machine's job, and it is
/// tested there.
fn measure(position: Point, bounds: Rectangle) -> Pointer {
    Pointer::new(position.x - bounds.x, bounds.width)
}

/// End whatever the pointer was doing, because the widget can no longer be
/// sure it still has it (module docs: "Losing the pointer").
///
/// A held groove publishes its ordinary `release` — the gesture *commits* at
/// the last position it saw, it is not rolled back — which is also what
/// keeps [`crate::player`] from being left mid-drag, holding a pending
/// position and ignoring the engine's `Progress` forever. A hover publishes
/// its ordinary `exit`, and a held groove publishes that too: the pointer
/// that left is not resting on the bar, and the preview must not outlive it.
///
/// Both flags are cleared unconditionally, so this cannot itself strand
/// either one, and it is idempotent — a second loss event before the pointer
/// comes back publishes nothing.
fn lost_pointer<Message: Clone>(
    state: &mut State,
    pointers: &Pointers<'_, Message>,
    shell: &mut Shell<'_, Message>,
) {
    let held = std::mem::take(&mut state.held);
    let hovered = std::mem::take(&mut state.hovered);
    if held {
        shell.publish(pointers.release.clone());
    }
    if held || hovered {
        shell.publish(pointers.exit.clone());
    }
}

impl<Message, Renderer> Widget<Message, Theme, Renderer> for Groove<'_, Message>
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
        Size::new(self.width, Length::Fixed(self.height))
    }

    fn layout(
        &self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        layout::atomic(limits, self.width, Length::Fixed(self.height))
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
        let Some(pointers) = self.pointers.as_ref() else {
            return event::Status::Ignored;
        };
        let bounds = layout.bounds();
        let state = tree.state.downcast_mut::<State>();

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerPressed { .. }) => {
                if let Some(position) = cursor.position_over(bounds) {
                    state.held = true;
                    shell.publish((pointers.press)(measure(position, bounds)));
                    return event::Status::Captured;
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerLifted { .. } | touch::Event::FingerLost { .. }) => {
                if state.held {
                    state.held = false;
                    // The button may well have come up somewhere else
                    // entirely: re-read where the pointer actually is so the
                    // next move reports the right thing.
                    state.hovered = cursor.is_over(bounds);
                    shell.publish(pointers.release.clone());
                    return event::Status::Captured;
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { .. })
            | Event::Touch(touch::Event::FingerMoved { .. }) => {
                let Some(position) = cursor.position() else {
                    return event::Status::Ignored;
                };
                if state.held {
                    // Captured: a held groove owns the pointer until it is
                    // released, wherever the pointer wanders.
                    shell.publish((pointers.drag)(measure(position, bounds)));
                    return event::Status::Captured;
                }
                if cursor.is_over(bounds) {
                    state.hovered = true;
                    shell.publish((pointers.hover)(measure(position, bounds)));
                } else if state.hovered {
                    state.hovered = false;
                    shell.publish(pointers.exit.clone());
                }
                // Hovering is not an interaction: the event is left for
                // whatever else wants it.
            }
            // The pointer is no longer ours: whatever it was doing has to
            // end here, because the event that would normally end it is
            // being delivered to somebody else (module docs).
            Event::Mouse(mouse::Event::CursorLeft) | Event::Window(window::Event::Unfocused) => {
                lost_pointer(state, pointers, shell);
            }
            _ => {}
        }
        event::Status::Ignored
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        let state = tree.state.downcast_ref::<State>();
        if self.pointers.is_none() {
            return theme::GROOVE_CURSOR_INERT;
        }
        if state.held {
            theme::GROOVE_CURSOR_HELD
        } else if cursor.is_over(layout.bounds()) {
            theme::GROOVE_CURSOR
        } else {
            theme::GROOVE_CURSOR_INERT
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
        let state = tree.state.downcast_ref::<State>();
        let bounds = layout.bounds();
        let status = if state.held {
            iced::widget::slider::Status::Dragged
        } else if self.pointers.is_some() && cursor.is_over(bounds) {
            iced::widget::slider::Status::Hovered
        } else {
            iced::widget::slider::Status::Active
        };
        let style = (self.style)(self.palette, status);

        let (handle_width, handle_height, handle_radius) = match style.handle.shape {
            HandleShape::Circle { radius } => (radius * 2.0, radius * 2.0, radius.into()),
            HandleShape::Rectangle {
                width,
                border_radius,
            } => (f32::from(width), bounds.height, border_radius),
        };
        // The handle's travel is inset by its own width so it never hangs
        // off either end — the same geometry iced's slider draws, kept so
        // the bar looks exactly as it did before it grew a brain.
        let travel = |fraction: f32| (bounds.width - handle_width) * fraction.clamp(0.0, 1.0);
        let offset = travel(self.position);
        let rail_y = bounds.y + bounds.height / 2.0;
        let filled = offset + handle_width / 2.0;

        renderer.fill_quad(
            renderer::Quad {
                bounds: Rectangle {
                    x: bounds.x,
                    y: rail_y - style.rail.width / 2.0,
                    width: filled,
                    height: style.rail.width,
                },
                border: style.rail.border,
                ..renderer::Quad::default()
            },
            style.rail.backgrounds.0,
        );
        renderer.fill_quad(
            renderer::Quad {
                bounds: Rectangle {
                    x: bounds.x + filled,
                    y: rail_y - style.rail.width / 2.0,
                    width: (bounds.width - filled).max(0.0),
                    height: style.rail.width,
                },
                border: style.rail.border,
                ..renderer::Quad::default()
            },
            style.rail.backgrounds.1,
        );
        // The detent goes on before the handle but never under it: it is
        // lifted clear of the knob's own radius, so the mark that says
        // "here" is legible in the one state that matters most — when the
        // handle is sitting on it.
        if let Some(detent) = self.detent {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: Rectangle {
                        x: bounds.x + travel(detent.at) + (handle_width - theme::DETENT_W) / 2.0,
                        y: rail_y - handle_height / 2.0 - theme::DETENT_GAP - theme::DETENT_H,
                        width: theme::DETENT_W,
                        height: theme::DETENT_H,
                    },
                    ..renderer::Quad::default()
                },
                theme::detent_ink(self.palette, detent.engaged),
            );
        }
        renderer.fill_quad(
            renderer::Quad {
                bounds: Rectangle {
                    x: bounds.x + offset,
                    y: rail_y - handle_height / 2.0,
                    width: handle_width,
                    height: handle_height,
                },
                border: Border {
                    radius: handle_radius,
                    width: style.handle.border_width,
                    color: style.handle.border_color,
                },
                ..renderer::Quad::default()
            },
            style.handle.background,
        );
    }
}

impl<'a, Message, Renderer> From<Groove<'a, Message>> for Element<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Renderer: renderer::Renderer + 'a,
{
    fn from(groove: Groove<'a, Message>) -> Self {
        Self::new(groove)
    }
}

#[cfg(test)]
mod tests {
    use iced::advanced::clipboard;
    use iced::{Background, Transformation};

    use super::*;

    /// Everything a groove can say, tagged so a test can read the published
    /// stream back as the sequence of decisions the widget made.
    #[derive(Debug, Clone, PartialEq)]
    enum Msg {
        Press(Pointer),
        Drag(Pointer),
        Hover(Pointer),
        Release,
        Exit,
    }

    /// A renderer that draws nothing. `on_event` never touches it, but it
    /// has to name a type; `iced_core`'s own `impl Renderer for ()` lives
    /// behind `debug_assertions`, so these tests carry their own rather than
    /// quietly failing to compile under `cargo test --release`.
    struct Ink;

    impl renderer::Renderer for Ink {
        fn start_layer(&mut self, _bounds: Rectangle) {}
        fn end_layer(&mut self) {}
        fn start_transformation(&mut self, _transformation: Transformation) {}
        fn end_transformation(&mut self) {}
        fn fill_quad(&mut self, _quad: renderer::Quad, _background: impl Into<Background>) {}
        fn clear(&mut self) {}
    }

    /// Where the bar under test is laid out — deliberately away from the
    /// window origin, so a report that forgot to measure against the bar
    /// would be visibly wrong rather than accidentally right.
    const ORIGIN: Point = Point::new(100.0, 50.0);
    /// The bar's width under test.
    const WIDTH: f32 = 260.0;
    /// The bar's hit height under test.
    const HEIGHT: f32 = 20.0;

    /// A cursor `x` px into the bar, vertically centred on it.
    fn on_bar(x: f32) -> mouse::Cursor {
        mouse::Cursor::Available(Point::new(ORIGIN.x + x, ORIGIN.y + HEIGHT / 2.0))
    }

    /// A cursor inside the window but clear of the bar (well below it).
    fn off_bar(x: f32) -> mouse::Cursor {
        mouse::Cursor::Available(Point::new(ORIGIN.x + x, ORIGIN.y + HEIGHT * 8.0))
    }

    /// What a report `x` px into the bar looks like once measured.
    fn at(x: f32) -> Pointer {
        Pointer::new(x, WIDTH)
    }

    /// One way of losing the pointer, applied to a bar under test.
    type Loss = fn(&mut Bar) -> (event::Status, Vec<Msg>);

    /// One groove, its state tree, and its layout — driven event by event.
    struct Bar {
        groove: Groove<'static, Msg>,
        tree: Tree,
        node: layout::Node,
        renderer: Ink,
    }

    impl Bar {
        fn new(groove: Groove<'static, Msg>) -> Self {
            let tag = Widget::<Msg, Theme, Ink>::tag(&groove);
            let state = Widget::<Msg, Theme, Ink>::state(&groove);
            Self {
                groove,
                tree: Tree {
                    tag,
                    state,
                    children: Vec::new(),
                },
                node: layout::Node::new(Size::new(WIDTH, HEIGHT)).move_to(ORIGIN),
                renderer: Ink,
            }
        }

        /// The seek bar, wired up.
        fn seek() -> Self {
            Self::new(
                Groove::new(0.25, &theme::CLOSING_TIME, theme::seek)
                    .width(Length::Fixed(WIDTH))
                    .height(HEIGHT)
                    .on_pointer(Msg::Press, Msg::Drag, Msg::Hover, Msg::Release, Msg::Exit),
            )
        }

        /// The volume fader, wired up — the same widget with the one thing
        /// the fader adds, so "both bars" is a claim these tests can make.
        fn fader() -> Self {
            Self::new(
                Groove::new(0.8, &theme::CLOSING_TIME, theme::volume)
                    .width(Length::Fixed(WIDTH))
                    .height(HEIGHT)
                    .detent(Detent {
                        at: 1.0,
                        engaged: false,
                    })
                    .on_pointer(Msg::Press, Msg::Drag, Msg::Hover, Msg::Release, Msg::Exit),
            )
        }

        /// A groove of undeclared length: no handlers, so no pointer.
        fn inert() -> Self {
            Self::new(
                Groove::new(0.25, &theme::CLOSING_TIME, theme::seek_inert)
                    .width(Length::Fixed(WIDTH))
                    .height(HEIGHT),
            )
        }

        /// Deliver one event with the pointer at `cursor`.
        fn feed(&mut self, event: Event, cursor: mouse::Cursor) -> (event::Status, Vec<Msg>) {
            let mut messages = Vec::new();
            let mut shell = Shell::new(&mut messages);
            let status = self.groove.on_event(
                &mut self.tree,
                event,
                Layout::new(&self.node),
                cursor,
                &self.renderer,
                &mut clipboard::Null,
                &mut shell,
                &Rectangle::with_size(Size::new(1000.0, 1000.0)),
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

        /// The pointer left the window: iced reports the cursor as gone
        /// along with it.
        fn cursor_left(&mut self) -> (event::Status, Vec<Msg>) {
            self.feed(
                Event::Mouse(mouse::Event::CursorLeft),
                mouse::Cursor::Unavailable,
            )
        }

        /// Another window took focus. The pointer may well still be sitting
        /// on the bar, which is exactly why this case is not the one above.
        fn unfocused(&mut self, cursor: mouse::Cursor) -> (event::Status, Vec<Msg>) {
            self.feed(Event::Window(window::Event::Unfocused), cursor)
        }
    }

    /// The path that already worked, pinned: press, scrub, release, and the
    /// hover that follows — all inside the window.
    #[test]
    fn an_ordinary_drag_released_inside_the_window_is_unchanged() {
        let mut bar = Bar::seek();
        assert_eq!(bar.press(on_bar(10.0)), vec![Msg::Press(at(10.0))]);
        assert_eq!(bar.moved(on_bar(60.0)), vec![Msg::Drag(at(60.0))]);
        // A held groove still owns the pointer past either end of the bar.
        assert_eq!(bar.moved(off_bar(-40.0)), vec![Msg::Drag(at(-40.0))]);
        assert_eq!(bar.released(on_bar(90.0)), vec![Msg::Release]);
        // And the release leaves an ordinary hover behind it.
        assert_eq!(bar.moved(on_bar(95.0)), vec![Msg::Hover(at(95.0))]);
        assert_eq!(bar.moved(off_bar(95.0)), vec![Msg::Exit]);
    }

    /// The reported bug: the button comes up outside the window, so baz
    /// never sees the release. The drag has to end at the boundary, and
    /// every later move has to be nothing but a move.
    #[test]
    fn a_drag_that_leaves_the_window_ends_there_and_scrubs_nothing_after() {
        let mut bar = Bar::seek();
        bar.press(on_bar(10.0));
        assert_eq!(bar.moved(on_bar(60.0)), vec![Msg::Drag(at(60.0))]);

        // Out of the window. The gesture commits at the last position it
        // saw, and the preview goes with the pointer.
        assert_eq!(bar.cursor_left().1, vec![Msg::Release, Msg::Exit]);

        // The button came up out there, unseen. Moving back over the bar is
        // a hover, not the scrub the bug turned it into.
        assert_eq!(bar.moved(on_bar(200.0)), vec![Msg::Hover(at(200.0))]);
        assert_eq!(bar.moved(on_bar(20.0)), vec![Msg::Hover(at(20.0))]);
    }

    /// The likelier real path: the pointer never leaves, but another window
    /// takes the focus — and with it the release.
    #[test]
    fn a_drag_interrupted_by_the_window_losing_focus_ends_the_same_way() {
        let mut bar = Bar::seek();
        bar.press(on_bar(10.0));
        bar.moved(on_bar(60.0));
        assert_eq!(bar.unfocused(on_bar(60.0)).1, vec![Msg::Release, Msg::Exit]);
        assert_eq!(bar.moved(on_bar(200.0)), vec![Msg::Hover(at(200.0))]);
    }

    /// The fader is the same widget, and it must answer the same way — it
    /// is the control where a stuck drag is worst, because every step of it
    /// commits.
    #[test]
    fn the_fader_loses_the_pointer_exactly_as_the_seek_bar_does() {
        let losses: [Loss; 2] = [Bar::cursor_left, |bar| bar.unfocused(on_bar(60.0))];
        for lose in losses {
            let mut bar = Bar::fader();
            assert_eq!(bar.press(on_bar(10.0)), vec![Msg::Press(at(10.0))]);
            assert_eq!(bar.moved(on_bar(60.0)), vec![Msg::Drag(at(60.0))]);
            assert_eq!(lose(&mut bar).1, vec![Msg::Release, Msg::Exit]);
            assert_eq!(bar.moved(on_bar(200.0)), vec![Msg::Hover(at(200.0))]);
        }
    }

    /// A finger that leaves the surface is cancelled rather than dropped,
    /// and a focus steal ends a touch gesture like any other.
    #[test]
    fn a_touch_gesture_ends_on_a_cancelled_finger_or_a_lost_focus() {
        let id = touch::Finger(1);
        let position = Point::new(ORIGIN.x + 60.0, ORIGIN.y + HEIGHT / 2.0);

        let mut bar = Bar::seek();
        bar.feed(
            Event::Touch(touch::Event::FingerPressed { id, position }),
            on_bar(10.0),
        );
        assert_eq!(
            bar.feed(
                Event::Touch(touch::Event::FingerLost { id, position }),
                on_bar(60.0)
            )
            .1,
            vec![Msg::Release],
            "a cancelled finger is an ordinary release"
        );

        let mut bar = Bar::seek();
        bar.feed(
            Event::Touch(touch::Event::FingerPressed { id, position }),
            on_bar(10.0),
        );
        assert_eq!(bar.unfocused(on_bar(60.0)).1, vec![Msg::Release, Msg::Exit]);
    }

    /// Re-entering and pressing again is an ordinary gesture — the loss
    /// ended a drag, it did not disable the control.
    #[test]
    fn a_press_after_the_pointer_comes_back_starts_a_fresh_drag() {
        let mut bar = Bar::seek();
        bar.press(on_bar(10.0));
        bar.moved(on_bar(60.0));
        bar.cursor_left();

        assert_eq!(bar.press(on_bar(30.0)), vec![Msg::Press(at(30.0))]);
        assert_eq!(bar.moved(on_bar(120.0)), vec![Msg::Drag(at(120.0))]);
        assert_eq!(bar.released(on_bar(120.0)), vec![Msg::Release]);
    }

    /// The fix must not swap one stuck flag for another: after a loss the
    /// hover is off, so a move that never touches the bar says nothing.
    #[test]
    fn losing_the_pointer_cannot_strand_the_hover() {
        let mut bar = Bar::seek();
        bar.press(on_bar(10.0));
        bar.cursor_left();
        assert_eq!(
            bar.moved(off_bar(60.0)),
            vec![],
            "a stranded hover would have exited a second time here"
        );
        // And the hover still works when the pointer does come back.
        assert_eq!(bar.moved(on_bar(60.0)), vec![Msg::Hover(at(60.0))]);
        assert_eq!(bar.moved(off_bar(60.0)), vec![Msg::Exit]);
    }

    /// With nothing held, a departure is still just a departure — reported
    /// once, and only when there was a hover to retire.
    #[test]
    fn losing_the_pointer_while_merely_hovering_exits_exactly_once() {
        let mut bar = Bar::seek();
        assert_eq!(bar.moved(on_bar(60.0)), vec![Msg::Hover(at(60.0))]);
        assert_eq!(bar.cursor_left().1, vec![Msg::Exit]);
        assert_eq!(bar.cursor_left().1, vec![], "idempotent");
        assert_eq!(bar.unfocused(off_bar(60.0)).1, vec![]);
    }

    /// Losing the pointer is a broadcast fact, not an interaction: both
    /// grooves in the bar — and everything else watching the pointer or the
    /// focus — have to hear it.
    #[test]
    fn losing_the_pointer_is_never_captured() {
        let mut bar = Bar::seek();
        bar.press(on_bar(10.0));
        bar.moved(on_bar(60.0));
        assert_eq!(bar.cursor_left().0, event::Status::Ignored);

        let mut bar = Bar::seek();
        bar.press(on_bar(10.0));
        assert_eq!(bar.unfocused(on_bar(10.0)).0, event::Status::Ignored);
    }

    /// An inert groove refuses the pointer entirely, so it has nothing to
    /// lose and nothing to say when it does.
    #[test]
    fn an_inert_groove_ignores_a_lost_pointer() {
        let mut bar = Bar::inert();
        assert_eq!(bar.press(on_bar(10.0)), vec![]);
        assert_eq!(bar.moved(on_bar(60.0)), vec![]);
        assert_eq!(bar.cursor_left(), (event::Status::Ignored, vec![]));
        assert_eq!(
            bar.unfocused(on_bar(60.0)),
            (event::Status::Ignored, vec![])
        );
    }
}
