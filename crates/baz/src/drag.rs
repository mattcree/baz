//! **The reorder drag**: press a row, move it, and the list is the list you
//! made — doc 09 §13 step 8, the last of the implicit-playlist steps, built
//! as doc 11 P5's promoted pointer-capture widget (ADR-0024 §6 layer 3).
//!
//! # Sugar over routes that already work
//!
//! The drag retires nothing. The ▲▼ steppers, the per-row `+`, the picker
//! and the context menu all remain exactly as shipped — the drag is an
//! accelerator layer *over* them, precisely as ADR-0024 §6 designed it and
//! for the reason the era kept the menu command beside every drag. That is
//! also the accessibility honesty this widget owes the visible-control rule
//! (a standing rule of the product): a drag is a gesture, pointer-only by nature, and no
//! action here is gesture-only — every reorder the drag can make, a stepper
//! can make; every add it can make, the `+` and the picker can make. The
//! visible controls are the route; this is the shortcut for the hand that
//! is already on the row.
//!
//! # What one gesture does
//!
//! - **Reorder**, on the two list editors (the queue place and the playlist
//!   page — "the same editor", 09 §8.2): press a row's body, move past the
//!   threshold, and an insertion line follows the pointer between rows;
//!   release commits **one** whole-list edit — one `UpdateQueue` for the
//!   run ([`crate::queue_edit::moved`]), one saved file for the artefact
//!   ([`crate::playlists::Playlists::move_entry`]) — never a stream of
//!   deltas.
//! - **Add**, when the panel stands: carry the row over the panel's playlist
//!   rows and release — the drop appends the row's track to that file, the
//!   same append the picker's row makes ([`crate::playlists::Playlists::append`]).
//!   The panel is not summoned by the drag; drag-to-add is directness for a
//!   panel already on screen, and the `+` → picker route remains the way in
//!   when it is not.
//!
//! A sub-threshold press-and-release is the row's ordinary click, published
//! by this wrapper on the row's behalf — selecting and playing survive the
//! row becoming draggable, which is the whole point of the threshold.
//! [`THRESHOLD_PX`] is the drag-and-drop distance (GTK's
//! `gtk-dnd-drag-threshold`, 8 px), deliberately larger than the groove's
//! 4 px click-vs-scrub line ([`crate::player::DRAG_THRESHOLD_PX`]): a scrub
//! previews and commits continuously, so eagerness is cheap there; a
//! reorder rearranges structure, so a click must not become one by tremor.
//!
//! # Capture, and losing the pointer
//!
//! iced 0.13 has no pointer capture — the measurements and the argument live
//! in [`crate::groove`]'s module docs and are not repeated — so this widget
//! does what the groove does: once the press is its own, it keeps reading
//! the pointer wherever the pointer goes, and it ends the gesture on the
//! two events that say the pointer is no longer demonstrably ours,
//! [`iced::mouse::Event::CursorLeft`] and
//! [`iced::window::Event::Unfocused`] (doc 04 §2.2's documented workaround,
//! P5's own citation). Ending is a **commit, not a cancel** — the groove's
//! law, held for the groove's reason: the insertion line under the pointer
//! at the boundary is the list the user was looking at, and snapping back
//! would be indistinguishable from baz dropping the input. The one
//! deviation is the *sub-threshold* loss: the groove commits a sub-threshold
//! gesture as a click because its click seeks to a position already
//! previewed; a row's click **plays music**, and an alt-tab mid-press
//! sounding a track nobody asked for would be the worse failure — so an
//! armed, un-moved press disarms silently when the pointer is lost.
//! <kbd>Esc</kbd> is the explicit discard, peeled first in `App::escape`.
//!
//! Two departures from the groove's event discipline, both deliberate:
//!
//! - **Moves are broadcast, not captured.** The groove owns its drag alone;
//!   this gesture is measured by its neighbours — every row of the same
//!   list hit-tests the held pointer against its own bounds and reports
//!   which half it is in ([`Wires`]' `over`), which is what makes the
//!   insertion index exact under virtualization (the pointer can only ever
//!   be over rows [`crate::queue_window`] actually built) and free of any
//!   estimate of where the scrollable sits in the window. Capturing the
//!   moves would starve the very widgets doing the measuring.
//! - **The press is captured before the content sees it.** The row's own
//!   button stays in the tree with its `on_press` declared — its styling,
//!   its hover, its place in the mirror tests — but the wrapper owns the
//!   pointer for it, so a drag's release can never double as the click that
//!   plays the row. On a sub-threshold release, release-only child controls
//!   get first refusal; this is how an artist or album label navigates without
//!   turning the rest of the row into anything but Play. The pressed-flash is
//!   the one thing this costs.
//!
//! # The insertion line, and why nothing parts
//!
//! The line is [`theme::INSERT_LINE_H`] of [`theme::insert_line_ink`] drawn
//! at a row's edge by the row's own wrapper — state made visible, no new
//! colour, no layout change. The rows do **not** part to make room:
//! ADR-0020 §3 forbids motion that decorates, its five transitions do not
//! include list reflow, and under the queue's virtual window a parting tween
//! would move exactly the geometry the spacer arithmetic pins. The line
//! *is* the statement; it moves instantly because it is a pure function of
//! where the pointer is, which is the amendment's own discipline for
//! pointer-derived drawing.
//!
//! # What this module holds
//!
//! [`Source`] (the per-row wrapper widget), its [`Wires`], the pure gesture
//! [`Phase`] machine, and the pure insertion arithmetic ([`slot`],
//! [`DragState::destination`]) — tested here without a window, the
//! [`crate::pointer`] split. What a commit *means* stays where edits live:
//! [`crate::queue_edit`] for the run, [`crate::playlists`] for files, and
//! `app.rs` only routes.

use iced::advanced::layout::{self, Layout};
use iced::advanced::widget::{Operation, Tree, tree};
use iced::advanced::{Clipboard, Shell, Widget, overlay, renderer};
use iced::{Element, Event, Length, Point, Rectangle, Size, Theme, event, mouse, touch, window};

use crate::theme;
use crate::vm::QueueItemVm;

/// How far the pointer must travel from the press before the gesture is a
/// drag rather than a click — GTK's `gtk-dnd-drag-threshold` (8 px; Qt's
/// `startDragDistance` is 10). Distance, not axis: a drag toward the panel
/// is as much a drag as one down the list. See the module docs for why this
/// is not the groove's 4 px.
pub(crate) const THRESHOLD_PX: f32 = 8.0;

/// Which list a dragged row came from — the two editors of 09 §8.2. Data,
/// not pixels: the open playlist page is single, and the queue is the queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum List {
    /// The **Queue** place's rows; a commit is one `UpdateQueue`.
    Queue,
    /// The open **playlist page**'s rows; a commit is one saved file.
    Playlist,
}

/// Which edge of a row the insertion line is drawn on. `Top` of row *i*
/// states slot *i*; `Bottom` exists only on the last row, stating the slot
/// past the end — every other boundary is some row's top.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Edge {
    Top,
    Bottom,
}

/// The insertion slot a report means: the pointer in row `index`'s upper
/// half aims at the boundary above it (slot `index`), in its lower half at
/// the boundary below (slot `index + 1`).
#[must_use]
pub(crate) const fn slot(index: usize, before: bool) -> usize {
    if before { index } else { index + 1 }
}

/// One drag in flight — held on `App` as a single `Option`, which is what
/// makes "one drag at a time" structural rather than policed (the menu's
/// own construction).
#[derive(Debug, Clone)]
pub(crate) struct DragState {
    /// Which editor the row was lifted from.
    pub(crate) list: List,
    /// The lifted row's index in that list.
    pub(crate) from: usize,
    /// How many rows the list held at the lift; slots run `0..=len`.
    pub(crate) len: usize,
    /// The row's title — what the ghost says is in the hand.
    pub(crate) title: String,
    /// The track the row names, for a drop on a panel row. `None` for a
    /// missing playlist entry: its row reorders (position is real) but
    /// transfers nothing — the `+`'s own rule, held by the drag.
    pub(crate) payload: Option<QueueItemVm>,
    /// Where the pointer is, in window coordinates — the ghost's anchor.
    pub(crate) at: Point,
    /// The insertion slot the line currently states, `0..=len`. Starts at
    /// `from` — the no-op slot — and moves only on a row's report.
    pub(crate) insert: usize,
    /// The panel playlist row under the pointer, when the drag is over one:
    /// the drop becomes that file's append instead of a reorder.
    pub(crate) over_panel: Option<u64>,
}

impl DragState {
    /// A drag just lifted at `at`: the insertion starts as the no-op slot,
    /// so a release that never crossed a boundary asks for nothing.
    pub(crate) fn begin(
        list: List,
        from: usize,
        len: usize,
        title: String,
        payload: Option<QueueItemVm>,
        at: Point,
    ) -> Self {
        Self {
            list,
            from,
            len,
            title,
            payload,
            at,
            insert: from,
            over_panel: None,
        }
    }

    /// A row of the dragged list reported the pointer over itself. Clamped
    /// to the slots that exist, against a report from a stale rebuild.
    pub(crate) fn over_row(&mut self, index: usize, before: bool) {
        self.insert = slot(index, before).min(self.len);
    }

    /// The reorder the current slot means: the index the row lands at once
    /// it is taken out, or `None` when the slot is one of the two that
    /// bracket the row itself — a no-op the commit declines to send.
    #[must_use]
    pub(crate) fn destination(&self) -> Option<usize> {
        if self.insert == self.from || self.insert == self.from + 1 {
            return None;
        }
        Some(if self.insert > self.from {
            self.insert - 1
        } else {
            self.insert
        })
    }

    /// Which edge, if any, row `index` of `list` draws the insertion line
    /// on — `Top` for its own slot, `Bottom` only on the last row for the
    /// slot past the end.
    #[must_use]
    pub(crate) fn line_for_row(&self, list: List, index: usize) -> Option<Edge> {
        if list != self.list {
            return None;
        }
        if self.insert == index {
            Some(Edge::Top)
        } else if index + 1 == self.len && self.insert == self.len {
            Some(Edge::Bottom)
        } else {
            None
        }
    }
}

/// The gesture, as a pure state machine — the widget's tree state, and the
/// thing the tests below drive without a window.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub(crate) enum Phase {
    /// Nothing held.
    #[default]
    Idle,
    /// The button is down where it went down; the gesture is not yet a
    /// click and not yet a drag.
    Armed(Point),
    /// The threshold was crossed; the row is in the hand.
    Dragging,
}

/// What a live source publishes. Every closure closes over the row's own
/// list and index at the view — the widget carries no identity of its own.
pub(crate) struct Wires<'a, Message> {
    /// The threshold was crossed here: the drag begins.
    lift: Box<dyn Fn(Point) -> Message + 'a>,
    /// The held pointer moved — anywhere; the ghost follows this.
    moved: Box<dyn Fn(Point) -> Message + 'a>,
    /// The gesture ended: commit against the app's drag state.
    dropped: Message,
    /// The sub-threshold release: the row's ordinary click, published on
    /// the row's behalf. `None` when the row is not currently clickable
    /// (no engine, a missing entry) — the inner button's own gate, mirrored.
    click: Option<Message>,
}

impl<'a, Message> Wires<'a, Message> {
    /// Wires a draggable row up. See the field docs.
    pub(crate) fn new(
        lift: impl Fn(Point) -> Message + 'a,
        moved: impl Fn(Point) -> Message + 'a,
        dropped: Message,
        click: Option<Message>,
    ) -> Self {
        Self {
            lift: Box::new(lift),
            moved: Box::new(moved),
            dropped,
            click,
        }
    }
}

/// The per-row drag wrapper: a row's body made a drag source, an observer
/// of drags in flight, and the canvas the insertion line is drawn on.
///
/// Inert — `wires` absent — it forwards everything untouched, so a
/// non-draggable row (no engine behind the queue) costs nothing and changes
/// nothing.
pub(crate) struct Source<'a, Message, Renderer> {
    content: Element<'a, Message, Theme, Renderer>,
    palette: &'static theme::Palette,
    wires: Option<Wires<'a, Message>>,
    /// Report the pointer's half-row while a drag is in flight in this
    /// row's list — set by the view from the app's drag state, `bool` is
    /// "upper half".
    observe: Option<Box<dyn Fn(bool) -> Message + 'a>>,
    /// The insertion line this row currently draws, if any.
    line: Option<Edge>,
}

impl<'a, Message, Renderer> Source<'a, Message, Renderer> {
    /// Wraps a row's body. Inert until [`Self::wires`] arrives.
    pub(crate) fn new(
        content: impl Into<Element<'a, Message, Theme, Renderer>>,
        palette: &'static theme::Palette,
    ) -> Self {
        Self {
            content: content.into(),
            palette,
            wires: None,
            observe: None,
            line: None,
        }
    }

    /// Makes the row draggable. Without this the wrapper is a pass-through.
    #[must_use]
    pub(crate) fn wires(mut self, wires: Wires<'a, Message>) -> Self {
        self.wires = Some(wires);
        self
    }

    /// Report the held pointer against this row's bounds while a drag is in
    /// flight in its list (the view decides when).
    #[must_use]
    pub(crate) fn observe(mut self, over: impl Fn(bool) -> Message + 'a) -> Self {
        self.observe = Some(Box::new(over));
        self
    }

    /// Draw the insertion line on `edge`, when the app's drag state says
    /// this row's boundary is the one the pointer aims at.
    #[must_use]
    pub(crate) fn line(mut self, line: Option<Edge>) -> Self {
        self.line = line;
        self
    }
}

impl<Message, Renderer> Widget<Message, Theme, Renderer> for Source<'_, Message, Renderer>
where
    Message: Clone,
    Renderer: renderer::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<Phase>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(Phase::default())
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
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }

    #[expect(
        clippy::too_many_lines,
        reason = "iced 0.14 moved capture onto Shell; keeping the armed/dragging routing in one event transaction makes its precedence auditable"
    )]
    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let status = (|| {
            let bounds = layout.bounds();
            let phase = tree.state.downcast_mut::<Phase>();
            if let Some(wires) = &self.wires {
                match event {
                    // The press is the wrapper's before it is the content's —
                    // the one inversion of `menu::area`'s order, so a drag's
                    // release can never double as the click that plays the row
                    // (module docs).
                    Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
                    | Event::Touch(touch::Event::FingerPressed { .. }) => {
                        if let Some(position) = cursor.position()
                            && bounds.contains(position)
                        {
                            *phase = Phase::Armed(position);
                            return event::Status::Captured;
                        }
                    }
                    Event::Mouse(mouse::Event::CursorMoved { .. })
                    | Event::Touch(touch::Event::FingerMoved { .. }) => {
                        if let Some(position) = cursor.position() {
                            match *phase {
                                Phase::Armed(origin)
                                    if position.distance(origin) >= THRESHOLD_PX =>
                                {
                                    *phase = Phase::Dragging;
                                    shell.publish((wires.lift)(position));
                                    if let Some(over) = &self.observe
                                        && bounds.contains(position)
                                    {
                                        shell.publish(over(before_mid(position, bounds)));
                                    }
                                    // Broadcast, not captured: the sibling rows
                                    // measure this same move (module docs).
                                    return event::Status::Ignored;
                                }
                                Phase::Dragging => {
                                    shell.publish((wires.moved)(position));
                                    if let Some(over) = &self.observe
                                        && bounds.contains(position)
                                    {
                                        shell.publish(over(before_mid(position, bounds)));
                                    }
                                    return event::Status::Ignored;
                                }
                                _ => {}
                            }
                        }
                    }
                    Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
                    | Event::Touch(
                        touch::Event::FingerLifted { .. } | touch::Event::FingerLost { .. },
                    ) => match std::mem::take(phase) {
                        Phase::Armed(_) => {
                            // A row may contain a release-only named route (the
                            // playlist's artist and album labels). The wrapper
                            // owned the press so a drag could start anywhere, but
                            // a release over that child belongs to the child and
                            // must not also spend the row's Play click.
                            let mut child_messages = Vec::new();
                            let mut child_shell = Shell::new(&mut child_messages);
                            self.content.as_widget_mut().update(
                                &mut tree.children[0],
                                event,
                                layout,
                                cursor,
                                renderer,
                                clipboard,
                                &mut child_shell,
                                viewport,
                            );
                            let child_acted = !child_shell.is_empty();
                            shell.merge(child_shell, std::convert::identity);
                            if child_acted || shell.is_event_captured() {
                                return event::Status::Captured;
                            }
                            // Sub-threshold: the row's ordinary click, made on
                            // the row's behalf.
                            if let Some(click) = &wires.click {
                                shell.publish(click.clone());
                            }
                            return event::Status::Captured;
                        }
                        Phase::Dragging => {
                            shell.publish(wires.dropped.clone());
                            return event::Status::Captured;
                        }
                        Phase::Idle => {}
                    },
                    // The pointer is no longer demonstrably ours: a drag commits
                    // where the line was (the groove's law), an armed press
                    // disarms silently (a click that plays music must not be
                    // made by an alt-tab — module docs). Never captured: losing
                    // the pointer is a broadcast fact.
                    Event::Mouse(mouse::Event::CursorLeft)
                    | Event::Window(window::Event::Unfocused) => match std::mem::take(phase) {
                        Phase::Dragging => shell.publish(wires.dropped.clone()),
                        Phase::Armed(_) | Phase::Idle => {}
                    },
                    _ => {}
                }
            }
            // A drag in flight somewhere else in this list: measure the pointer
            // against this row's own bounds — exact geometry, no estimate, and
            // the reason the moves above are broadcast.
            if matches!(*phase, Phase::Idle)
                && let Some(over) = &self.observe
                && matches!(
                    event,
                    Event::Mouse(mouse::Event::CursorMoved { .. })
                        | Event::Touch(touch::Event::FingerMoved { .. })
                )
                && let Some(position) = cursor.position()
                && bounds.contains(position)
            {
                shell.publish(over(before_mid(position, bounds)));
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
            if shell.is_event_captured() {
                event::Status::Captured
            } else {
                event::Status::Ignored
            }
        })();
        if status == event::Status::Captured {
            shell.capture_event();
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
        // The insertion line, over the row's edge: state made visible, no
        // layout moved (module docs — nothing parts).
        if let Some(edge) = self.line {
            let bounds = layout.bounds();
            let y = match edge {
                Edge::Top => bounds.y,
                Edge::Bottom => bounds.y + bounds.height - theme::INSERT_LINE_H,
            };
            renderer.fill_quad(
                renderer::Quad {
                    bounds: Rectangle {
                        x: bounds.x,
                        y,
                        width: bounds.width,
                        height: theme::INSERT_LINE_H,
                    },
                    ..renderer::Quad::default()
                },
                theme::insert_line_ink(self.palette),
            );
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        if matches!(tree.state.downcast_ref::<Phase>(), Phase::Dragging) {
            return theme::GROOVE_CURSOR_HELD;
        }
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: iced::Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

/// Whether `position` is in the upper half of `bounds` — the half-row rule
/// the insertion [`slot`] is decided from.
fn before_mid(position: Point, bounds: Rectangle) -> bool {
    position.y < bounds.y + bounds.height / 2.0
}

impl<'a, Message, Renderer> From<Source<'a, Message, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Renderer: renderer::Renderer + 'a,
{
    fn from(source: Source<'a, Message, Renderer>) -> Self {
        Self::new(source)
    }
}

#[cfg(test)]
mod tests {
    use iced::advanced::clipboard;
    use iced::widget::{Space, mouse_area};
    use iced::{Background, Transformation};

    use super::*;

    /// Everything a source can say, tagged for reading the stream back.
    #[derive(Debug, Clone, PartialEq)]
    enum Msg {
        Lift(Point),
        Moved(Point),
        Dropped,
        Click,
        Link,
        Over(bool),
    }

    /// A renderer that draws nothing (the groove tests' own, carried for
    /// the same `--release` reason).
    struct Ink;

    impl renderer::Renderer for Ink {
        fn start_layer(&mut self, _bounds: Rectangle) {}
        fn end_layer(&mut self) {}
        fn start_transformation(&mut self, _transformation: Transformation) {}
        fn end_transformation(&mut self) {}
        fn fill_quad(&mut self, _quad: renderer::Quad, _background: impl Into<Background>) {}
        fn reset(&mut self, _new_bounds: Rectangle) {}
        fn allocate_image(
            &mut self,
            _handle: &iced::advanced::image::Handle,
            callback: impl FnOnce(
                Result<iced::advanced::image::Allocation, iced::advanced::image::Error>,
            ) + Send
            + 'static,
        ) {
            callback(Err(iced::advanced::image::Error::Unsupported));
        }
    }

    /// The row under test, away from the origin so a report measured
    /// against the window would be visibly wrong.
    const ORIGIN: Point = Point::new(80.0, 200.0);
    const W: f32 = 400.0;
    const H: f32 = 28.0;

    fn at(x: f32, y: f32) -> mouse::Cursor {
        mouse::Cursor::Available(Point::new(x, y))
    }

    /// A cursor on the row, `dx`/`dy` from its top-left corner.
    fn on_row(dx: f32, dy: f32) -> mouse::Cursor {
        at(ORIGIN.x + dx, ORIGIN.y + dy)
    }

    struct Row {
        source: Source<'static, Msg, Ink>,
        tree: Tree,
        node: layout::Node,
        renderer: Ink,
    }

    impl Row {
        fn build(source: Source<'static, Msg, Ink>) -> Self {
            let tag = Widget::<Msg, Theme, Ink>::tag(&source);
            let state = Widget::<Msg, Theme, Ink>::state(&source);
            let children = Widget::<Msg, Theme, Ink>::children(&source);
            Self {
                source,
                tree: Tree {
                    tag,
                    state,
                    children,
                },
                node: layout::Node::with_children(
                    Size::new(W, H),
                    vec![layout::Node::new(Size::new(W, H))],
                )
                .move_to(ORIGIN),
                renderer: Ink,
            }
        }

        /// A live, clickable, observing row — the queue row's full wiring.
        fn wired() -> Self {
            Self::build(
                Source::new(
                    Space::new()
                        .width(Length::Fixed(W))
                        .height(Length::Fixed(H)),
                    &theme::CLOSING_TIME,
                )
                .wires(Wires::new(
                    Msg::Lift,
                    Msg::Moved,
                    Msg::Dropped,
                    Some(Msg::Click),
                ))
                .observe(Msg::Over),
            )
        }

        /// A wired row carrying one independently actionable metadata label.
        fn linked() -> Self {
            Self::build(
                Source::new(
                    mouse_area(
                        Space::new()
                            .width(Length::Fixed(W))
                            .height(Length::Fixed(H)),
                    )
                    .on_release(Msg::Link),
                    &theme::CLOSING_TIME,
                )
                .wires(Wires::new(
                    Msg::Lift,
                    Msg::Moved,
                    Msg::Dropped,
                    Some(Msg::Click),
                )),
            )
        }

        /// A row that only observes — every other row of a list mid-drag.
        fn observer() -> Self {
            Self::build(
                Source::new(
                    Space::new()
                        .width(Length::Fixed(W))
                        .height(Length::Fixed(H)),
                    &theme::CLOSING_TIME,
                )
                .observe(Msg::Over),
            )
        }

        /// An inert row — no wires, no observation.
        fn inert() -> Self {
            Self::build(Source::new(
                Space::new()
                    .width(Length::Fixed(W))
                    .height(Length::Fixed(H)),
                &theme::CLOSING_TIME,
            ))
        }

        #[expect(
            clippy::needless_pass_by_value,
            reason = "test helper accepts constructed iced events at call sites"
        )]
        fn feed(&mut self, event: Event, cursor: mouse::Cursor) -> (event::Status, Vec<Msg>) {
            let mut messages = Vec::new();
            let mut shell = Shell::new(&mut messages);
            self.source.update(
                &mut self.tree,
                &event,
                Layout::new(&self.node),
                cursor,
                &self.renderer,
                &mut clipboard::Null,
                &mut shell,
                &Rectangle::with_size(Size::new(1400.0, 1000.0)),
            );
            let status = if shell.is_event_captured() {
                event::Status::Captured
            } else {
                event::Status::Ignored
            };
            (status, messages)
        }

        fn press(&mut self, cursor: mouse::Cursor) -> (event::Status, Vec<Msg>) {
            self.feed(
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
                cursor,
            )
        }

        fn moved(&mut self, cursor: mouse::Cursor) -> (event::Status, Vec<Msg>) {
            let position = cursor.position().expect("a move needs a position");
            self.feed(Event::Mouse(mouse::Event::CursorMoved { position }), cursor)
        }

        fn released(&mut self, cursor: mouse::Cursor) -> (event::Status, Vec<Msg>) {
            self.feed(
                Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
                cursor,
            )
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
    }

    /// A press that never travels is the row's ordinary click — published
    /// by the wrapper, exactly once, on release.
    #[test]
    fn a_sub_threshold_press_and_release_is_a_click() {
        let mut row = Row::wired();
        let (status, messages) = row.press(on_row(30.0, 10.0));
        assert_eq!(
            status,
            event::Status::Captured,
            "the press is the wrapper's"
        );
        assert!(messages.is_empty(), "arming says nothing yet");
        // Tremor inside the threshold moves nothing.
        let (_, messages) = row.moved(on_row(33.0, 13.0));
        assert!(messages.is_empty(), "sub-threshold travel is not a drag");
        let (status, messages) = row.released(on_row(33.0, 13.0));
        assert_eq!(status, event::Status::Captured);
        assert_eq!(messages, vec![Msg::Click]);
    }

    #[test]
    fn a_named_child_route_wins_the_release_without_disabling_drag() {
        let mut row = Row::linked();
        let (status, messages) = row.press(on_row(30.0, 10.0));
        assert_eq!(status, event::Status::Captured);
        assert!(messages.is_empty());
        let (status, messages) = row.released(on_row(30.0, 10.0));
        assert_eq!(
            (status, messages),
            (event::Status::Captured, vec![Msg::Link])
        );

        // Crossing the threshold turns the same initial press into a drag;
        // its release is then the drop, never the link under the pointer.
        row.press(on_row(30.0, 10.0));
        let (_, messages) = row.moved(on_row(60.0, 10.0));
        assert!(matches!(messages.as_slice(), [Msg::Lift(_)]));
        let (_, messages) = row.released(on_row(60.0, 10.0));
        assert_eq!(messages, vec![Msg::Dropped]);
    }

    /// Crossing the threshold lifts the row; every further move follows;
    /// the release commits — and no click is ever made.
    #[test]
    fn crossing_the_threshold_lifts_and_the_release_drops() {
        let mut row = Row::wired();
        row.press(on_row(30.0, 10.0));
        let (status, messages) = row.moved(on_row(30.0, 10.0 + THRESHOLD_PX));
        assert_eq!(
            messages,
            vec![
                Msg::Lift(Point::new(ORIGIN.x + 30.0, ORIGIN.y + 10.0 + THRESHOLD_PX)),
                Msg::Over(false),
            ],
            "the lift, and the row's own half-row report"
        );
        assert_eq!(
            status,
            event::Status::Ignored,
            "moves are broadcast so sibling rows can measure them"
        );
        let far = at(600.0, 700.0);
        let (_, messages) = row.moved(far);
        assert_eq!(messages, vec![Msg::Moved(Point::new(600.0, 700.0))]);
        let (status, messages) = row.released(far);
        assert_eq!(status, event::Status::Captured);
        assert_eq!(messages, vec![Msg::Dropped], "a drop, never also a click");
    }

    /// The threshold is a distance, not an axis: a sideways pull — the
    /// drag-to-panel direction — lifts the row exactly as a vertical one.
    #[test]
    fn a_horizontal_pull_lifts_too() {
        let mut row = Row::wired();
        row.press(on_row(30.0, 10.0));
        let (_, messages) = row.moved(on_row(30.0 + THRESHOLD_PX, 10.0));
        assert!(matches!(messages.first(), Some(Msg::Lift(_))));
    }

    /// The groove's reported bug, pinned for this widget: the pointer
    /// leaves the window mid-drag, the gesture commits there, and later
    /// moves are nothing but moves.
    #[test]
    fn a_drag_that_leaves_the_window_commits_there_and_drags_nothing_after() {
        let mut row = Row::wired();
        row.press(on_row(30.0, 10.0));
        row.moved(on_row(30.0, 40.0));
        let (status, messages) = row.cursor_left();
        assert_eq!(messages, vec![Msg::Dropped], "a commit, not a cancel");
        assert_eq!(
            status,
            event::Status::Ignored,
            "losing the pointer is a broadcast fact"
        );
        // The button came up out there, unseen: nothing is welded on. (The
        // `Over` report is the observation wire, which the view retires
        // when the app's drag state clears — the *gesture* is what must
        // not survive.)
        let (_, messages) = row.moved(on_row(30.0, 10.0));
        assert!(
            !messages
                .iter()
                .any(|msg| matches!(msg, Msg::Lift(_) | Msg::Moved(_) | Msg::Dropped)),
            "no drag survives the loss: {messages:?}"
        );
    }

    /// The likelier real path: another window takes the focus mid-drag.
    #[test]
    fn a_drag_interrupted_by_lost_focus_commits_the_same_way() {
        let mut row = Row::wired();
        row.press(on_row(30.0, 10.0));
        row.moved(on_row(30.0, 40.0));
        let (status, messages) = row.unfocused(on_row(30.0, 40.0));
        assert_eq!(messages, vec![Msg::Dropped]);
        assert_eq!(status, event::Status::Ignored);
        let (_, messages) = row.moved(on_row(30.0, 60.0));
        assert_eq!(messages, vec![]);
    }

    /// A press the window loses before any travel plays nothing: the
    /// deliberate deviation from the groove's sub-threshold commit — a
    /// row's click is music, and an alt-tab must not make it.
    #[test]
    fn an_armed_press_disarms_silently_when_the_pointer_is_lost() {
        for lose in [
            |row: &mut Row| row.cursor_left(),
            |row: &mut Row| row.unfocused(on_row(30.0, 10.0)),
        ] {
            let mut row = Row::wired();
            row.press(on_row(30.0, 10.0));
            let (_, messages) = lose(&mut row);
            assert_eq!(messages, vec![], "no click from a lost press");
            // And the next release is nobody's click either.
            let (_, messages) = row.released(on_row(30.0, 10.0));
            assert_eq!(messages, vec![]);
        }
    }

    /// An observing row measures the held pointer against its own bounds:
    /// upper half is "before me", lower half "after me", outside is
    /// silence — the exact-geometry half of the broadcast design.
    #[test]
    fn an_observer_reports_the_half_row_under_the_pointer() {
        let mut row = Row::observer();
        let (status, messages) = row.moved(on_row(30.0, H * 0.25));
        assert_eq!(messages, vec![Msg::Over(true)]);
        assert_eq!(status, event::Status::Ignored);
        let (_, messages) = row.moved(on_row(30.0, H * 0.75));
        assert_eq!(messages, vec![Msg::Over(false)]);
        let (_, messages) = row.moved(at(10.0, 10.0));
        assert_eq!(messages, vec![], "off the row is not a report");
    }

    /// An inert row refuses the pointer entirely: presses fall through to
    /// the content and nothing is ever published.
    #[test]
    fn an_inert_row_says_nothing_and_captures_nothing() {
        let mut row = Row::inert();
        let (status, messages) = row.press(on_row(30.0, 10.0));
        assert_eq!(status, event::Status::Ignored);
        assert!(messages.is_empty());
        let (_, messages) = row.moved(on_row(60.0, 20.0));
        assert!(messages.is_empty());
        let (status, messages) = row.cursor_left();
        assert_eq!((status, messages), (event::Status::Ignored, vec![]));
    }

    /// A fresh press after a committed drag is an ordinary new gesture.
    #[test]
    fn a_press_after_a_drop_starts_fresh() {
        let mut row = Row::wired();
        row.press(on_row(30.0, 10.0));
        row.moved(on_row(30.0, 40.0));
        row.released(on_row(30.0, 40.0));
        let (_, messages) = row.press(on_row(30.0, 10.0));
        assert!(messages.is_empty());
        let (_, messages) = row.released(on_row(31.0, 11.0));
        assert_eq!(messages, vec![Msg::Click], "the row is still a row");
    }

    // ---- the pure arithmetic ------------------------------------------

    /// The half-row rule: upper half aims above the row, lower half below.
    #[test]
    fn a_report_means_the_boundary_its_half_faces() {
        assert_eq!(slot(3, true), 3);
        assert_eq!(slot(3, false), 4);
        assert_eq!(slot(0, true), 0);
    }

    /// The two slots bracketing the lifted row are the no-op — a release
    /// there asks for nothing — and every other slot lands the row exactly
    /// where the line said.
    #[test]
    fn the_destination_is_the_slot_minus_the_hole_the_row_leaves() {
        let mut drag = DragState::begin(List::Queue, 3, 8, String::new(), None, Point::ORIGIN);
        assert_eq!(drag.destination(), None, "a lift starts at the no-op slot");
        drag.over_row(3, false);
        assert_eq!(drag.insert, 4);
        assert_eq!(drag.destination(), None, "the slot below self is self");
        drag.over_row(0, true);
        assert_eq!(drag.destination(), Some(0), "to the head");
        drag.over_row(7, false);
        assert_eq!(drag.insert, 8);
        assert_eq!(drag.destination(), Some(7), "past the tail lands last");
        drag.over_row(5, true);
        assert_eq!(drag.destination(), Some(4), "downward crosses its own hole");
        drag.over_row(1, false);
        assert_eq!(drag.destination(), Some(2), "upward does not");
    }

    /// A stale report cannot aim past the list.
    #[test]
    fn a_report_is_clamped_to_the_slots_that_exist() {
        let mut drag = DragState::begin(List::Queue, 0, 3, String::new(), None, Point::ORIGIN);
        drag.over_row(9, false);
        assert_eq!(drag.insert, 3);
    }

    /// Which row draws the line: the slot's own row on its top edge, the
    /// last row's bottom edge for the slot past the end, the other list
    /// never.
    #[test]
    fn the_line_is_drawn_by_the_row_whose_boundary_is_aimed_at() {
        let mut drag = DragState::begin(List::Queue, 1, 4, String::new(), None, Point::ORIGIN);
        drag.over_row(2, false);
        assert_eq!(drag.line_for_row(List::Queue, 3), Some(Edge::Top));
        assert_eq!(drag.line_for_row(List::Queue, 2), None);
        assert_eq!(drag.line_for_row(List::Playlist, 3), None);
        drag.over_row(3, false);
        assert_eq!(drag.insert, 4);
        assert_eq!(
            drag.line_for_row(List::Queue, 3),
            Some(Edge::Bottom),
            "the one Bottom: the slot past the end, on the last row"
        );
    }
}
