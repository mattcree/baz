//! **The contour**: the shape a generated playlist is asked to follow, drawn
//! as a line you can take hold of.
//!
//! The owner, twice. First: *"can you maybe allow for the vibe playlist to be
//! configured via a few curves instead where we can control the prompt
//! somewhat?"* That shipped as four labelled buttons behind a disclosure —
//! `Slow build`, `Peak and fall` — whose whole effect was to append the words
//! *"energy shape: Slow build"* to the text prompt. Then, looking at it:
//! *"the vibe flow just looks crap… I wanted something more graphical, like
//! tuning it via curves and so on. it makes no sense to anyone trying to use
//! it."*
//!
//! He was right twice over, and the second reading is the damning one: the
//! control was not merely ugly, it was **not connected to anything**.
//! `baz_vibe::select_semantic` had no position term at all, so no arrangement
//! of those buttons could have changed where in a list a track landed. The
//! engine that *could* — position-aware targets interpolated across the
//! playlist — was sitting in the same crate with no caller but its own tests.
//!
//! # What this widget is
//!
//! A line over two axes that are both the collection's own:
//!
//! - **across** — position through the finished playlist, its first track at
//!   the left edge and its last at the right;
//! - **up** — energy, from the calmest end of *your* library to the loudest.
//!   Not an absolute: `baz_vibe::levels` stretches the analysed pool's own
//!   range onto −2…+2, so the top of this box is the loudest music you own
//!   rather than a number the analysis could not know.
//!
//! Behind the line it draws **the library itself** — how much of your music
//! sits at each height — because a shape is a request and a request you cannot
//! fill is worth seeing before you spend a minute of analysis on it. A wall of
//! ambient records shows a mass along the bottom and thin air at the top, and
//! a contour drawn up there will visibly be asking for something that is not
//! in the room.
//!
//! After a list is composed it draws **what it got**: one dot per track, at
//! that track's own place on the same axes. Request and result in one picture,
//! which is the whole of what the four buttons could never do.
//!
//! # Why it is drawn in quads
//!
//! iced's `canvas` would give strokes and curves directly, and it is not in
//! this build's feature set — it pulls a tessellation stack (`lyon`) into a
//! dependency graph this project prices deliberately (`deny.toml`, ADR-0025's
//! reasoning for `rfd`). So the line is drawn the way [`crate::visualizer`]'s
//! spectrum is: a column per step, filled from the line down. That is not a
//! compromise dressed up — an energy profile filled to its baseline reads as
//! a *band of loudness*, which is what it is, and it puts this control in the
//! same visual language as the spectrum analyser two places away.
//!
//! # It holds no state that matters
//!
//! Layer 3 (ADR-0006): the widget knows which point the pointer has hold of
//! and nothing else. Every level it publishes is a fraction and a level, and
//! what those mean — clamping between neighbours, the shape the presets load,
//! what gets sent to the selector — is [`crate::vibe`]'s, where it is pure and
//! tested without a window.

use iced::advanced::widget::Tree;
use iced::advanced::widget::{Widget, tree};
use iced::advanced::{Clipboard, Layout, Shell, layout, renderer};
use iced::keyboard::{self, key};
use iced::{Color, Element, Event, Length, Point, Rectangle, Size, Theme, mouse};

use crate::theme;
use crate::vibe::ContourPoint;

/// What a drag reports: which point, and the raw position and level the
/// pointer described. Named so the widget's field is readable — the rule for
/// what a listener may actually ask for is `crate::vibe`'s.
type OnDrag<'a, Message> = Box<dyn Fn(usize, f32, f32) -> Message + 'a>;

/// A contour over its two axes, optionally pointer-driven.
pub(crate) struct Contour<'a, Message> {
    points: &'a [ContourPoint],
    field: &'a [f32],
    result: Vec<(f32, f32)>,
    highlight: Option<usize>,
    palette: &'static theme::Palette,
    height: f32,
    marks: bool,
    on_drag: Option<OnDrag<'a, Message>>,
    on_release: Option<Message>,
}

/// Which point the pointer has hold of, and which it is over. Both are
/// pointer facts about *this* widget, which is the only kind of state a
/// drawing layer may keep.
#[derive(Debug, Default)]
struct ContourState {
    held: Option<usize>,
    hovered: Option<usize>,
    /// **Which point the keys move**, and the one that wears the ring.
    ///
    /// The quorum, on the drawn line: *it is pointer-only and needs keys*.
    /// A press inside the control takes focus, a press outside gives it up,
    /// and while it is held the arrows belong to this widget rather than to
    /// the transport — which is true by construction, because capturing the
    /// event is what tells `crate::keys` that somebody else has already made
    /// the decision.
    focused: Option<usize>,
}

/// **How far one arrow press moves a point.**
///
/// A tenth of a level and two per cent of the list: fine enough that the keys
/// are a *tuning* route rather than a coarse alternative, and Shift takes the
/// same press four times as far for crossing the control.
const NUDGE_LEVEL: f32 = 0.1;
const NUDGE_AT: f32 = 0.02;
const NUDGE_LARGE: f32 = 4.0;

impl<'a, Message> Contour<'a, Message> {
    /// A contour drawn at `height`, inert until [`Self::on_drag`] wires it up.
    pub(crate) fn new(
        points: &'a [ContourPoint],
        palette: &'static theme::Palette,
        height: f32,
    ) -> Self {
        Self {
            points,
            field: &[],
            result: Vec::new(),
            highlight: None,
            palette,
            height,
            marks: true,
            on_drag: None,
            on_release: None,
        }
    }

    /// The library's own distribution over the up axis, lowest bucket first,
    /// each already normalised to `0.0..=1.0` against the fullest bucket.
    #[must_use]
    pub(crate) fn field(mut self, field: &'a [f32]) -> Self {
        self.field = field;
        self
    }

    /// Where the tracks of the last composed list actually landed:
    /// `(position, level)` per track, in listening order.
    #[must_use]
    pub(crate) fn result(mut self, result: Vec<(f32, f32)>) -> Self {
        self.result = result;
        self
    }

    /// **The one track the pointer is on**, over in the list of rows: its dot
    /// is drawn larger, on a guide down to the axis, so hovering a row in the
    /// composed playlist shows where on the shape that track sits.
    ///
    /// The owner's, and it is the answer to the question this whole control
    /// exists to make answerable: *"the idea here is that a person can see it
    /// really worked."*
    #[must_use]
    pub(crate) fn highlight(mut self, row: Option<usize>) -> Self {
        self.highlight = row;
        self
    }

    /// Report a point moved to `(at, level)` — the raw geometry the pointer
    /// described, unclamped except to the box. What a listener may actually
    /// ask for is [`crate::vibe`]'s rule, not this widget's.
    #[must_use]
    pub(crate) fn on_drag(mut self, on_drag: impl Fn(usize, f32, f32) -> Message + 'a) -> Self {
        self.on_drag = Some(Box::new(on_drag));
        self
    }

    /// Report the gesture's end, so the caller can spend one recomposition
    /// rather than one per pixel.
    #[must_use]
    pub(crate) fn on_release(mut self, message: Message) -> Self {
        self.on_release = Some(message);
        self
    }

    /// The drawing area inside the widget's own bounds: the box less the
    /// point radius on every side, so a handle at either end is whole rather
    /// than half-drawn against the edge.
    fn field_bounds(bounds: Rectangle) -> Rectangle {
        let pad = theme::CONTOUR_POINT / 2.0;
        Rectangle {
            x: bounds.x + pad,
            y: bounds.y + pad,
            width: (bounds.width - 2.0 * pad).max(1.0),
            height: (bounds.height - 2.0 * pad).max(1.0),
        }
    }

    fn x_of(field: Rectangle, at: f32) -> f32 {
        field.x + at.clamp(0.0, 1.0) * field.width
    }

    fn y_of(field: Rectangle, level: f32) -> f32 {
        field.y
            + (theme::CONTOUR_TOP - level.clamp(-theme::CONTOUR_TOP, theme::CONTOUR_TOP))
                / (2.0 * theme::CONTOUR_TOP)
                * field.height
    }

    fn at_of(field: Rectangle, x: f32) -> f32 {
        ((x - field.x) / field.width).clamp(0.0, 1.0)
    }

    fn level_of(field: Rectangle, y: f32) -> f32 {
        let fraction = ((y - field.y) / field.height).clamp(0.0, 1.0);
        theme::CONTOUR_TOP - fraction * 2.0 * theme::CONTOUR_TOP
    }

    /// The point under `cursor`, within the handle's own grab radius.
    fn point_at(&self, bounds: Rectangle, cursor: Point) -> Option<usize> {
        let field = Self::field_bounds(bounds);
        self.points
            .iter()
            .enumerate()
            .map(|(index, point)| {
                let centre =
                    Point::new(Self::x_of(field, point.at), Self::y_of(field, point.level));
                (index, centre.distance(cursor))
            })
            .filter(|(_, distance)| *distance <= theme::CONTOUR_GRAB)
            .min_by(|left, right| left.1.total_cmp(&right.1))
            .map(|(index, _)| index)
    }
}

/// The level the line stands at over `at`, or the middle of the box when the
/// contour is empty — a widget with no points still draws its axes.
fn level_at(points: &[ContourPoint], at: f32) -> f32 {
    crate::vibe::level_at(points, at).unwrap_or(0.0)
}

impl<Message, Renderer> Widget<Message, Theme, Renderer> for Contour<'_, Message>
where
    Message: Clone,
    Renderer: renderer::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<ContourState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(ContourState::default())
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fixed(self.height))
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        layout::atomic(limits, Length::Fill, Length::Fixed(self.height))
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
        let Some(on_drag) = &self.on_drag else {
            return;
        };
        let bounds = layout.bounds();
        let state = tree.state.downcast_mut::<ContourState>();
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let Some(position) = cursor.position_over(bounds) else {
                    // A press somewhere else is what gives the keys back to
                    // the rest of the application. There is nowhere to publish
                    // this to and nothing to capture — the ring simply goes.
                    state.focused = None;
                    return;
                };
                // **A press anywhere in the box takes the nearest point, and
                // a press near nothing takes nothing.** A press that moved
                // the closest point wherever it landed would make an
                // accidental click a shape change, and this control's whole
                // job is that the shape is what you drew.
                if let Some(index) = self.point_at(bounds, position) {
                    state.held = Some(index);
                    state.focused = Some(index);
                    shell.capture_event();
                } else {
                    // Inside the control but not on a handle: the keys are
                    // still this widget's, on whichever point they last had.
                    state.focused = state.focused.or(Some(0));
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                let position = cursor.position();
                // Hover is only for the cursor and the handle's ink; it is
                // read from wherever the pointer is, held or not.
                state.hovered = position.and_then(|position| self.point_at(bounds, position));
                let (Some(index), Some(position)) = (state.held, position) else {
                    return;
                };
                // **The pointer is tracked wherever it wanders once it has
                // hold of a point** — off the widget, off the window — which
                // is the lesson `crate::groove` records: a drag that stops
                // publishing at the edge is a drag that sticks.
                let field = Self::field_bounds(bounds);
                shell.publish(on_drag(
                    index,
                    Self::at_of(field, position.x),
                    Self::level_of(field, position.y),
                ));
                shell.capture_event();
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                if state.held.take().is_some() {
                    if let Some(release) = &self.on_release {
                        shell.publish(release.clone());
                    }
                    shell.capture_event();
                }
            }
            // **The keys, while this control holds them.**
            //
            // Tab walks the points, the arrows move the focused one, and
            // Shift takes each press four times as far. Every one of them
            // captures, which is how `crate::keys` knows not to seek or
            // change the volume: it reads iced's own capture report rather
            // than guessing at what is focused.
            Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) => {
                let Some(index) = state.focused else {
                    return;
                };
                let Some(point) = self.points.get(index) else {
                    state.focused = None;
                    return;
                };
                let last = self.points.len().saturating_sub(1);
                let step = if modifiers.shift() { NUDGE_LARGE } else { 1.0 };
                let (at, level) = match key.as_ref() {
                    iced::keyboard::Key::Named(key::Named::Tab) => {
                        state.focused = Some(if modifiers.shift() {
                            index.checked_sub(1).unwrap_or(last)
                        } else if index >= last {
                            0
                        } else {
                            index + 1
                        });
                        shell.capture_event();
                        return;
                    }
                    iced::keyboard::Key::Named(key::Named::ArrowUp) => {
                        (point.at, step.mul_add(NUDGE_LEVEL, point.level))
                    }
                    iced::keyboard::Key::Named(key::Named::ArrowDown) => {
                        (point.at, step.mul_add(-NUDGE_LEVEL, point.level))
                    }
                    iced::keyboard::Key::Named(key::Named::ArrowRight) => {
                        (step.mul_add(NUDGE_AT, point.at), point.level)
                    }
                    iced::keyboard::Key::Named(key::Named::ArrowLeft) => {
                        (step.mul_add(-NUDGE_AT, point.at), point.level)
                    }
                    _ => return,
                };
                // The raw ask, exactly as a drag reports one: what a line may
                // actually be is `crate::vibe`'s to decide, and the ends stay
                // at the ends there rather than here.
                shell.publish(on_drag(index, at, level));
                if let Some(release) = &self.on_release {
                    shell.publish(release.clone());
                }
                shell.capture_event();
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
        let state = tree.state.downcast_ref::<ContourState>();
        if self.on_drag.is_none() {
            return mouse::Interaction::default();
        }
        if state.held.is_some() {
            return mouse::Interaction::Grabbing;
        }
        let over = cursor
            .position_over(layout.bounds())
            .and_then(|position| self.point_at(layout.bounds(), position));
        if over.is_some() {
            mouse::Interaction::Grab
        } else {
            mouse::Interaction::default()
        }
    }

    #[expect(
        clippy::too_many_lines,
        reason = "one control drawn in one pass: ground, library, axis, band, result, handles"
    )]
    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<ContourState>();
        let room = self.palette;
        let bounds = layout.bounds();
        let field = Self::field_bounds(bounds);
        let mut quad = |bounds: Rectangle, colour: Color, radius: f32| {
            renderer.fill_quad(
                renderer::Quad {
                    bounds,
                    border: iced::Border {
                        radius: radius.into(),
                        ..iced::Border::default()
                    },
                    ..renderer::Quad::default()
                },
                colour,
            );
        };

        // 1. The ground: the recess every well in the product stands in.
        quad(bounds, theme::contour_ground(room), theme::RADIUS_CTRL);

        // 2. **The library itself**, band by band: how much music sits at
        //    each height, drawn faint and full-width. It is the one thing on
        //    this control that is not a request — it is what there is.
        let buckets = self.field.len();
        if buckets > 0 {
            #[expect(
                clippy::cast_precision_loss,
                reason = "a histogram has tens of buckets, far below f32's exact range"
            )]
            let band = field.height / buckets as f32;
            for (index, density) in self.field.iter().enumerate() {
                #[expect(
                    clippy::cast_precision_loss,
                    reason = "a histogram has tens of buckets, far below f32's exact range"
                )]
                let top = field.y + field.height - (index + 1) as f32 * band;
                quad(
                    Rectangle {
                        x: field.x,
                        y: top,
                        width: field.width,
                        height: band,
                    },
                    theme::contour_library(room, density.clamp(0.0, 1.0)),
                    0.0,
                );
            }
        }

        // 3. The middle of the collection, stated once. Without it the box is
        //    a picture with no scale in it.
        quad(
            Rectangle {
                x: field.x,
                y: Self::y_of(field, 0.0) - 0.5,
                width: field.width,
                height: 1.0,
            },
            theme::contour_axis(room),
            0.0,
        );

        // 4. **The line, as a band filled to the floor.** A column per step:
        //    diagonals are not a quad's shape, and an energy profile filled
        //    to its baseline is the spectrum's own reading of loudness rather
        //    than a compromise (module docs).
        if !self.points.is_empty() {
            let mut x = field.x;
            while x < field.x + field.width {
                let width = theme::CONTOUR_STEP.min(field.x + field.width - x);
                let at = Self::at_of(field, x + width / 2.0);
                let y = Self::y_of(field, level_at(self.points, at));
                quad(
                    Rectangle {
                        x,
                        y,
                        width,
                        height: (field.y + field.height - y).max(0.0),
                    },
                    theme::contour_band(room),
                    0.0,
                );
                quad(
                    Rectangle {
                        x,
                        y: y - theme::CONTOUR_LINE / 2.0,
                        width,
                        height: theme::CONTOUR_LINE,
                    },
                    theme::contour_line(room),
                    0.0,
                );
                x += width;
            }
        }

        // 5. **What the request actually produced.** The dots are the tracks,
        //    in listening order, at their own place on the same axes; the
        //    thread between them is the shape the playlist *has*, which is
        //    the thing to compare against the shape it was asked for. A list
        //    that followed the line reads as two lines together, and one that
        //    could not — because the library holds nothing up there — reads
        //    as a thread that will not leave the floor. Either answer is
        //    worth being able to see.
        for pair in self.result.windows(2) {
            let (from, to) = (pair[0], pair[1]);
            let (x0, y0) = (Self::x_of(field, from.0), Self::y_of(field, from.1));
            let (x1, y1) = (Self::x_of(field, to.0), Self::y_of(field, to.1));
            let steps = ((x1 - x0).abs() / theme::CONTOUR_STEP).ceil().max(1.0);
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "the step count is a positive pixel distance over a bounded control"
            )]
            let count = steps as u32;
            for step in 0..count {
                let mix = f32::from(u16::try_from(step).unwrap_or(u16::MAX)) / steps;
                let next = f32::from(u16::try_from(step + 1).unwrap_or(u16::MAX)) / steps;
                let top = y0 + (y1 - y0) * mix;
                let bottom = y0 + (y1 - y0) * next;
                quad(
                    Rectangle {
                        x: x0 + (x1 - x0) * mix,
                        y: top.min(bottom) - theme::CONTOUR_THREAD / 2.0,
                        width: theme::CONTOUR_STEP,
                        height: (top - bottom).abs() + theme::CONTOUR_THREAD,
                    },
                    theme::contour_thread(room),
                    0.0,
                );
            }
        }
        // **How far each track is from what was asked**, as a length rather
        // than as a colour: a tick from the dot to the line above or below
        // it. Short ticks are a list that followed the shape; long ones are a
        // collection that had nothing at that height. It is drawn in the
        // room's own quiet ink and not in the accent, so the reading does not
        // depend on telling two hues apart — the owner is colour blind, and a
        // picture whose whole argument is *did this work* may not rest on
        // green against amber.
        if !self.points.is_empty() {
            for (at, level) in &self.result {
                let asked = Self::y_of(field, level_at(self.points, *at));
                let got = Self::y_of(field, *level);
                quad(
                    Rectangle {
                        x: Self::x_of(field, *at) - theme::CONTOUR_THREAD / 2.0,
                        y: asked.min(got),
                        width: theme::CONTOUR_THREAD,
                        height: (asked - got).abs(),
                    },
                    theme::contour_miss(room),
                    0.0,
                );
            }
        }
        for (index, (at, level)) in self.result.iter().enumerate() {
            let lit = self.highlight == Some(index);
            let size = if lit {
                theme::CONTOUR_RESULT_LIT
            } else {
                theme::CONTOUR_RESULT
            };
            let (x, y) = (Self::x_of(field, *at), Self::y_of(field, *level));
            // The lit track stands on a guide to the floor, so its position
            // through the list is readable and not only its height.
            if lit {
                quad(
                    Rectangle {
                        x: x - 0.5,
                        y,
                        width: 1.0,
                        height: (field.y + field.height - y).max(0.0),
                    },
                    theme::contour_result(room),
                    0.0,
                );
            }
            quad(
                Rectangle {
                    x: x - size / 2.0,
                    y: y - size / 2.0,
                    width: size,
                    height: size,
                },
                theme::contour_result(room),
                size / 2.0,
            );
        }

        // 6. The handles, last, so nothing is drawn over the thing the hand
        //    is reaching for.
        if self.marks {
            for (index, point) in self.points.iter().enumerate() {
                let held = state.held == Some(index);
                let size = theme::CONTOUR_POINT;
                let (x, y) = (Self::x_of(field, point.at), Self::y_of(field, point.level));
                // **The ring on the point the keys are moving**, drawn under
                // the handle so the handle stays the thing the hand reaches
                // for. It is a *ring* rather than a tint because focus has to
                // be readable without separating two inks — the standing rule
                // — and because a filled dot already means *hovered or held*.
                if state.focused == Some(index) {
                    let ring = size + theme::CONTOUR_RING;
                    quad(
                        Rectangle {
                            x: x - ring / 2.0,
                            y: y - ring / 2.0,
                            width: ring,
                            height: ring,
                        },
                        theme::contour_focus(room),
                        ring / 2.0,
                    );
                }
                quad(
                    Rectangle {
                        x: x - size / 2.0,
                        y: y - size / 2.0,
                        width: size,
                        height: size,
                    },
                    theme::contour_point(room, held || state.hovered == Some(index)),
                    size / 2.0,
                );
            }
        }
    }
}

impl<'a, Message, Renderer> From<Contour<'a, Message>> for Element<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Renderer: renderer::Renderer + 'a,
{
    fn from(contour: Contour<'a, Message>) -> Self {
        Self::new(contour)
    }
}
