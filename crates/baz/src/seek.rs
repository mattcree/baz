//! The seek groove: the bottom bar's scrub bar as a custom iced widget.
//!
//! This is view-layer code (ADR-0006 layer 3) and holds **no** playback
//! state: it measures the pointer against its own bounds and reports what it
//! saw as a [`Pointer`]; every decision about what that means — click or
//! scrub, preview or not, what position to seek to — belongs to
//! [`crate::player`], where it is pure and unit-tested. The widget's own
//! `held`/`hovered` flags exist only to know which raw events to forward and
//! which cursor to ask for.
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

use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::widget::{Tree, tree};
use iced::advanced::{Clipboard, Shell, Widget};
use iced::widget::slider::{HandleShape, Style};
use iced::{Border, Element, Event, Length, Point, Rectangle, Size, Theme, event, mouse, touch};

use crate::player::Pointer;
use crate::theme;

/// How the groove is painted in each of its states. The same shape iced's
/// own slider uses, so [`crate::theme`] keeps expressing the bar with
/// `slider::Style` and this widget stays a drawing detail.
type StyleFn = fn(&Theme, iced::widget::slider::Status) -> Style;

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
    style: StyleFn,
    pointers: Option<Pointers<'a, Message>>,
}

/// The widget's own transient input state — not playback state.
#[derive(Default)]
struct State {
    /// The pointer went down on the groove and has not come up yet. While
    /// this is set the widget tracks the pointer wherever it goes, which is
    /// what makes a scrub off the end of the bar (and the release that ends
    /// it) work at all.
    held: bool,
    /// The pointer is resting on the groove, so a departure is worth
    /// reporting.
    hovered: bool,
}

impl<'a, Message> Groove<'a, Message> {
    /// A groove whose handle sits at `position` (`0.0..=1.0`), painted by
    /// `style`. Inert until [`Self::on_pointer`] wires it up.
    pub fn new(position: f32, style: StyleFn) -> Self {
        Self {
            position,
            width: Length::Fill,
            height: theme::RAIL_HIT,
            style,
            pointers: None,
        }
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
            Event::Mouse(mouse::Event::CursorLeft) => {
                if state.hovered {
                    state.hovered = false;
                    shell.publish(pointers.exit.clone());
                }
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
            return theme::SEEK_CURSOR_INERT;
        }
        if state.held {
            theme::SEEK_CURSOR_HELD
        } else if cursor.is_over(layout.bounds()) {
            theme::SEEK_CURSOR
        } else {
            theme::SEEK_CURSOR_INERT
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
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
        let style = (self.style)(theme, status);

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
        let offset = (bounds.width - handle_width) * self.position.clamp(0.0, 1.0);
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
