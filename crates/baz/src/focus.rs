//! **A focus stop** — the one primitive that makes a control reachable from
//! the keyboard.
//!
//! *(`docs/BACKLOG.md`, "Nothing in baz can be reached by keyboard except the
//! search well", opened 2026-08-15.)*
//!
//! # What was actually missing
//!
//! Not the bindings. `crate::keys` has read iced's own capture report rather
//! than tracking focus itself since it was written, which is precisely the
//! seam a focus order plugs into. What was missing is that **iced has no
//! focusable button**: in 0.14 exactly two widgets in the whole toolkit
//! implement [`iced::advanced::widget::operation::Focusable`], `text_input`
//! and `text_editor`, so
//! `focus_next()` traversed a wall of two hundred controls and found the
//! search well. Every button, chip, tile, row and stepper in the product was
//! pointer-only, and no binding table could change that.
//!
//! So this is a **wrapper**, not a new control. It takes an `Element` that is
//! already a button — already carrying its own paint, its own hit box, its own
//! message — and adds the three things a keyboard needs and iced does not
//! supply: a place in the traversal, a ring that says where you are, and a
//! press.
//!
//! # Why a wrapper rather than a focusable button
//!
//! Because the product has one button *style* per role and a dozen roles, and
//! a focusable `button` would mean a fork of `iced::widget::button` that every
//! one of those roles then had to be ported to. A wrapper composes with the
//! roles that already exist: the transport's [`crate::theme::transport`], the
//! header door's `word_button`, a tile, a stepper. Each keeps its own paint
//! and gains a stop.
//!
//! # The order is the tree, deliberately
//!
//! The backlog asked for "the order, per place". It is already written: iced
//! walks the widget tree in construction order, so `focus_next` visits
//! controls in the order the view builds them, which is the order they are
//! read. A hand-maintained focus index would be a second statement of the
//! layout that could disagree with the first — and it would have to be
//! rewritten every time a row moved. The one thing a view owes this module is
//! that it builds its controls in the order a listener meets them, which is
//! what a view owes a reader anyway.
//!
//! # The ring is not the hover
//!
//! Hover says *the pointer is here*; the ring says *the keyboard is here*, and
//! the two can be in different places at once. So the ring is drawn by this
//! wrapper rather than folded into each style's `Hovered` branch: one ring,
//! one colour, one width, everywhere in the product, and no style can forget
//! to draw it.

use iced::advanced::renderer::Renderer as _;
use iced::advanced::widget::{Operation, Tree};
use iced::advanced::widget::{Widget, tree};
use iced::advanced::{Clipboard, Layout, Shell, layout, mouse, overlay, renderer};
use iced::keyboard::{self, key};
use iced::{Element, Event, Length, Rectangle, Size, Theme, Vector};

use crate::theme;

/// **Is this stop the one the keyboard is on?**
///
/// The whole of the state, and it is deliberately the whole of it: a stop that
/// remembered anything else would be a control, and the control is the thing
/// inside it.
#[derive(Debug, Default, Clone, Copy)]
struct Focused {
    now: bool,
    /// Whether [`Stop::announced`]'s message has been published for the
    /// *current* arrival. Reset on leaving, so re-entering announces again.
    told: bool,
}

impl iced::advanced::widget::operation::Focusable for Focused {
    fn is_focused(&self) -> bool {
        self.now
    }

    fn focus(&mut self) {
        self.now = true;
    }

    fn unfocus(&mut self) {
        self.now = false;
    }
}

/// One stop on a place's keyboard order.
pub(crate) struct Stop<'a, Message> {
    content: Element<'a, Message, Theme, iced::Renderer>,
    /// What <kbd>Enter</kbd> and <kbd>Space</kbd> do here, or `None` while the
    /// control is disabled — a stop with nothing to press is skipped rather
    /// than focused, so the ring never lands somewhere it can do nothing.
    activate: Option<Message>,
    /// The ground the ring is drawn against, because a ring is an opaque
    /// colour rather than an alpha (see [`theme::Palette::ink_over`]).
    ground: iced::Color,
    /// The corner the ring follows, so it does not box a rounded control in a
    /// square. Every stop that draws a ring is a control today and every
    /// control in baz is [`theme::RADIUS_CTRL`]; this is a field rather than
    /// that constant inlined because the *ring* has to know the shape, and a
    /// setter with no caller would be speculative API pretending to be one.
    radius: f32,
    /// **What the arrows do while the ring is here**, for a stop that is a
    /// *region* rather than a control — see [`Stop::steered`].
    steer: Option<Box<dyn Fn(crate::search::Direction) -> Message + 'a>>,
    /// Published once each time the keyboard arrives — see
    /// [`Stop::announced`].
    on_focus: Option<Message>,
    /// Whether this stop draws the ring itself. A region does not: see
    /// [`Stop::announced`].
    ring: bool,
}

/// **Wrap a control in a focus stop.**
///
/// Deliberately unnamed. A stop needs an [`iced::advanced::widget::Id`] only
/// to be focused *by name*, and nothing in baz does that yet — Tab walks the
/// tree and never asks what anything is called. Adding ids before there is a
/// caller would be two hundred string constants maintained against nothing.
pub(crate) fn stop<'a, Message>(
    content: impl Into<Element<'a, Message, Theme, iced::Renderer>>,
    activate: Option<Message>,
) -> Stop<'a, Message> {
    Stop {
        content: content.into(),
        activate,
        ground: theme::active().wall,
        radius: theme::RADIUS_CTRL,
        steer: None,
        on_focus: None,
        ring: true,
    }
}

/// The arrow a key press is, or `None` for every other key.
fn direction_of(key: &keyboard::Key) -> Option<crate::search::Direction> {
    use crate::search::Direction;
    match key {
        keyboard::Key::Named(key::Named::ArrowUp) => Some(Direction::Up),
        keyboard::Key::Named(key::Named::ArrowDown) => Some(Direction::Down),
        keyboard::Key::Named(key::Named::ArrowLeft) => Some(Direction::Left),
        keyboard::Key::Named(key::Named::ArrowRight) => Some(Direction::Right),
        _ => None,
    }
}

impl<Message: Clone> Stop<'_, Message> {
    /// **Answer a key this stop owns**, reporting whether it did.
    ///
    /// Only reached while the stop is focused (see `update`), which is what
    /// makes taking these keys safe: focus is exclusive, so a field with a
    /// caret in it is the focused thing and this is not.
    fn take_key(&self, event: &Event, shell: &mut Shell<'_, Message>) -> bool {
        let Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) = event else {
            return false;
        };
        // **The arrows, for a region.** Bare only: the modified arrows are the
        // transport's and the history's, and a region that took them would
        // make where the keyboard happens to be change what Ctrl+→ means.
        if let Some(steer) = &self.steer
            && modifiers.is_empty()
            && let Some(direction) = direction_of(key)
        {
            shell.publish(steer(direction));
            shell.capture_event();
            return true;
        }
        // **Enter and Space, and nothing else.** They are the two presses
        // every desktop makes on a focused control, and baz spends Space on
        // play/pause globally — which is exactly why the ring has to take it
        // first: a key press belongs to the thing the keyboard is on, and
        // `crate::keys` reads iced's capture report, so capturing here is the
        // whole of telling it so.
        let Some(activate) = &self.activate else {
            return false;
        };
        if matches!(
            key,
            keyboard::Key::Named(key::Named::Enter | key::Named::Space)
        ) {
            shell.publish(activate.clone());
            shell.capture_event();
            return true;
        }
        false
    }
}

impl<Message> Stop<'_, Message> {
    /// State the ground the ring is drawn on, where it is not the wall — the
    /// bar, the lane's recess, a panel.
    #[must_use]
    pub(crate) fn on(mut self, ground: iced::Color) -> Self {
        self.ground = ground;
        self
    }
}

impl<'a, Message> Stop<'a, Message> {
    /// **Make this stop a region the arrows steer inside.**
    ///
    /// One Tab stop for a whole grid, and the arrows move within it. That is
    /// the pattern every grid uses — Tab between regions, arrows inside one —
    /// and the alternative is what `crate::focus`'s first landing declined to
    /// build: fifty tiles in a Tab order between the app bar and the
    /// transport, which is a worse product than no traversal at all.
    ///
    /// The arrows are only taken **while the ring is here**, so their global
    /// meanings — the volume on the vertical pair, the seek on the horizontal
    /// — are untouched everywhere else, which is everywhere the keyboard has
    /// not deliberately been put.
    #[must_use]
    pub(crate) fn steered(
        mut self,
        steer: impl Fn(crate::search::Direction) -> Message + 'a,
    ) -> Self {
        self.steer = Some(Box::new(steer));
        self
    }

    /// **Say when the keyboard arrives, and draw no ring of your own.**
    ///
    /// For a region, which is why the two go together. The first try drew the
    /// ring a *control* wears around the whole collection — a two-pixel
    /// rectangle at the window's inside edge — and the owner's reading was the
    /// correct one: *"eh… what is that… looks goofy"*. It is what a browser
    /// draws around a focused iframe, and it reads as chrome rather than as
    /// design, because a rectangle that large stops being a mark on something
    /// and becomes a border on everything.
    ///
    /// A region already had a better mark and it was drawn the whole time:
    /// **the selected tile**, with its card and its rule. So the region says
    /// nothing itself and instead makes sure there *is* a selection the moment
    /// the keyboard arrives — one message, once per arrival — and the record
    /// you would move from is the record that is lit.
    #[must_use]
    pub(crate) fn announced(mut self, arrival: Message) -> Self {
        self.on_focus = Some(arrival);
        self.ring = false;
        self
    }
}

impl<Message> Widget<Message, Theme, iced::Renderer> for Stop<'_, Message>
where
    Message: Clone,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<Focused>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(Focused::default())
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    /// **Publish the stop, then recurse.**
    ///
    /// Both, and in that order. A stop that did not recurse would hide any
    /// text field inside it from the same traversal that reaches the stop —
    /// which is how a wrapper quietly becomes a wall.
    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        // A disabled control is not a stop. Skipping it here rather than
        // graying a ring is the honest reading: the ring means *press this*,
        // and there is nothing to press. A steered region is a stop even
        // without one, because the arrows are the thing it offers.
        if self.activate.is_some() || self.steer.is_some() {
            let state = tree.state.downcast_mut::<Focused>();
            operation.focusable(None, layout.bounds(), state);
        }
        self.content
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        // **The keys this stop owns come first, and only while it is
        // focused.**
        //
        // The first landing delegated to the child before looking, on the
        // reasoning that a wrapper adds a route rather than taking one. That
        // is right for the pointer and wrong for the keyboard, and the
        // difference showed on the wall: pressing Down with the ring on the
        // collection turned the **volume** down. Something under the
        // scrollable answered the arrow, `shell.is_event_captured()` came back
        // true, and the region — the thing the keyboard was actually on —
        // never saw its own key.
        //
        // Focus is exclusive, so this cannot take a key from a field: if a
        // `text_input` inside a stop has the caret then this stop does not
        // have focus, and the branch below is skipped entirely. Everything
        // else — every pointer event, every key while unfocused — still
        // reaches the child untouched.
        //
        // **The arrival is announced here too, once per arrival.** `operate`
        // is where focus moves and it has no shell to publish through, so the
        // announcement is made on the next event this stop sees — the same
        // frame, because the press that moved the focus is itself an event.
        let state = tree.state.downcast_mut::<Focused>();
        if state.now {
            if !state.told
                && let Some(arrival) = &self.on_focus
            {
                state.told = true;
                shell.publish(arrival.clone());
            }
            if self.take_key(event, shell) {
                return;
            }
        } else {
            state.told = false;
        }
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme_: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme_,
            style,
            layout,
            cursor,
            viewport,
        );
        // A region draws no ring at all — see [`Stop::announced`].
        if !self.ring || !tree.state.downcast_ref::<Focused>().now {
            return;
        }
        // **The ring is drawn outside the control, not over it.** A ring
        // inside the bounds would eat the outer pixel row of a tile's
        // artwork and sit on the transport glyph's own edge; grown outward it
        // reads as a thing around the control, which is what it is. The
        // product's own gutter is more than [`theme::FOCUS_RING_GAP`]
        // everywhere a stop is used, so it never collides with a neighbour.
        let bounds = layout.bounds().expand(theme::FOCUS_RING_GAP);
        renderer.fill_quad(
            renderer::Quad {
                bounds,
                border: iced::Border {
                    color: theme::active().paper_ring(self.ground),
                    width: theme::FOCUS_RING_W,
                    radius: (self.radius + theme::FOCUS_RING_GAP).into(),
                },
                ..renderer::Quad::default()
            },
            iced::Color::TRANSPARENT,
        );
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, iced::Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

impl<'a, Message> From<Stop<'a, Message>> for Element<'a, Message, Theme, iced::Renderer>
where
    Message: Clone + 'a,
{
    fn from(stop: Stop<'a, Message>) -> Self {
        Self::new(stop)
    }
}

#[cfg(test)]
mod tests {
    /// **Every control in the window's frame is a focus stop.**
    ///
    /// The frame — the lane's four destinations and its history pair, the app
    /// bar's marks, the bottom bar's transport — is the part of baz that is
    /// on screen in every place, so it is the part a keyboard has to reach
    /// first. Asserted by scanning the four helpers that build them rather
    /// than by counting call sites: each helper is the *only* way its bar
    /// makes a glyph, which is what makes one edit per bar the whole of the
    /// coverage, and what would make a hand-rolled button beside one of them
    /// visible as the second kind of control it is.
    ///
    /// **What this deliberately does not claim.** The wall is one *steered*
    /// stop rather than a tile-per-stop Tab order, and it draws no ring —
    /// see [`Stop::announced`].
    #[test]
    fn every_control_in_the_frame_is_a_focus_stop() {
        for (module, source, helper) in [
            (
                "lane destinations",
                include_str!("views/lane.rs"),
                "fn destination_row(",
            ),
            (
                "lane history",
                include_str!("views/lane.rs"),
                "fn history_button(",
            ),
            (
                "app bar",
                include_str!("views/app_bar.rs"),
                "fn glyph_button(",
            ),
            (
                "bottom bar",
                include_str!("views/bottom_bar.rs"),
                "fn glyph_button(",
            ),
        ] {
            let source = source.replace("\r\n", "\n");
            let rest = source
                .split_once(helper)
                .unwrap_or_else(|| panic!("{module}: `{helper}` exists"))
                .1;
            let body = &rest[..rest.find("\n}\n").expect("a function ends")];
            assert!(
                body.contains("crate::focus::stop("),
                "{module} builds a control the keyboard cannot reach"
            );
            assert!(
                body.contains(".on("),
                "{module}'s ring does not name the ground it is drawn on, so \
                 it is an opaque colour picked against the wrong surface"
            );
        }
    }

    /// **A region announces and does not ring; a control rings and does not
    /// announce.**
    ///
    /// They are one decision, so [`Stop::announced`] sets both. The first
    /// landing gave the collection a control's ring — a two-pixel rectangle
    /// around the whole wall — and the owner read it in one look as chrome
    /// rather than design. Pinned so the pair cannot drift back apart: a
    /// region that started drawing a ring again would be the same picture.
    #[test]
    fn a_region_announces_instead_of_drawing_a_ring() {
        let source = include_str!("focus.rs").replace("\r\n", "\n");
        let shipped = source
            .split("#[cfg(test)]")
            .next()
            .expect("a source has a head");
        let rest = shipped
            .split_once("fn announced(")
            .expect("the region's arrival")
            .1;
        let body = &rest[..rest.find("\n    }\n").expect("a method ends")];
        assert!(
            body.contains("self.ring = false"),
            "a region that announces still draws a control's ring"
        );
        let rest = shipped.split_once("fn draw(").expect("the ring").1;
        let draw = &rest[..rest.find("\n    }\n").expect("a method ends")];
        assert!(
            draw.contains("!self.ring"),
            "the ring is drawn without asking whether this stop owns one"
        );
    }

    /// **A disabled control is not a stop.**
    ///
    /// The ring means *press this*. Landing it on the Back arrow at the start
    /// of a session — where there is no history and the arrow does nothing —
    /// would be the keyboard's one affordance telling its first lie. Stated
    /// against the widget rather than a view, because it is the wrapper's rule
    /// and every call site inherits it.
    #[test]
    fn a_control_with_nothing_to_press_is_skipped_rather_than_focused() {
        let source = include_str!("focus.rs").replace("\r\n", "\n");
        let shipped = source
            .split("#[cfg(test)]")
            .next()
            .expect("a source has a head");
        let rest = shipped.split_once("fn operate(").expect("the traversal").1;
        let body = &rest[..rest.find("\n    }\n").expect("a method ends")];
        assert!(
            body.contains("if self.activate.is_some() || self.steer.is_some()"),
            "a stop publishes itself whether or not it has anything to press"
        );
        assert!(
            body.contains(".as_widget_mut()") && body.contains(".operate("),
            "the stop no longer recurses, so any field inside one has left \
             the traversal that reaches the stop itself"
        );
    }
}
