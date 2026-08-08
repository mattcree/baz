//! The pointer machinery every hand-built widget in baz shares.
//!
//! baz draws two things by hand — [`crate::groove`] (the volume fader) and
//! [`crate::needle`] (the queue's seek line) — and ADR-0017 §5 records why
//! that is now the norm rather than the exception: *"a ~400-line hand-built
//! `Widget` with pointer geometry and its own tests is not disposable view
//! composition. It is the **second** such widget after `groove.rs`."* The
//! answer to a second one is not a second copy of the first one's lessons.
//!
//! So everything the groove learned the hard way lives here, once:
//!
//! - a **cursor** that says the thing under it can be pressed;
//! - a **hover** report, which is what a preview is drawn from;
//! - **tracking a held pointer wherever it wanders**, including past the
//!   widget's own bounds, which is the whole reason neither `slider` nor
//!   `mouse_area` could carry either control (the measurements are in
//!   [`crate::groove`]'s module docs and are not repeated here);
//! - **ending the gesture when the pointer stops being ours** — the bug that
//!   welded the fader to the pointer, and the two events that stand in for
//!   the grab-broken notification iced 0.13 does not publish.
//!
//! What is *not* here is any decision about what a gesture means. This module
//! reports geometry — a [`crate::player::Pointer`], a distance and a width —
//! and [`crate::player`] decides whether that is a click or a scrub, which
//! segment it landed in, and what to ask the engine for. That split is
//! ADR-0006's layer boundary and it is why the click-vs-drag threshold is
//! unit-tested without a window.
//!
//! # The hit band is separate from the bounds
//!
//! The one thing that genuinely differs between the two widgets is **where
//! the pointer may aim**. A groove's hit band is its layout box: it reserves
//! [`crate::theme::RAIL_HIT`] of height and draws a 4 px rail centred in it.
//! The needle cannot do that — it is 2 px flush on the window's bottom edge
//! and the whole point of ADR-0017 §1.1 is that it costs the collection 2 px
//! rather than 45 — so it reserves 2 px of layout and claims its aiming band
//! *upward*, into the empty lane the bar keeps under its transport.
//!
//! Hence every function here takes `bounds` (what the report is measured
//! against, so `x` means the same thing in both widgets) **and** `hit` (what
//! the pointer must be inside for a press to start). For a groove they are
//! the same rectangle; for the needle they are not, and the assertion that
//! the needle's band reaches no control lives in [`crate::theme`].

use iced::advanced::Shell;
use iced::{Event, Point, Rectangle, event, mouse, touch, window};

use crate::player::Pointer;
use crate::theme;

/// The handlers a live control reports through.
///
/// Absent on an inert one — a track of undeclared length, a queue that was
/// never sent — which is how a widget knows to ignore the pointer entirely
/// rather than to look identical and do nothing.
pub struct Pointers<'a, Message> {
    /// The button went down inside the hit band, this far along the bounds.
    press: Box<dyn Fn(Pointer) -> Message + 'a>,
    /// The pointer moved while held — anywhere, including off the widget.
    drag: Box<dyn Fn(Pointer) -> Message + 'a>,
    /// The pointer moved over the hit band with nothing held.
    hover: Box<dyn Fn(Pointer) -> Message + 'a>,
    /// The gesture ended.
    release: Message,
    /// The pointer is no longer resting on the control.
    exit: Message,
}

impl<'a, Message> Pointers<'a, Message> {
    /// Wires a control up. See the field docs for what each one means.
    pub fn new(
        press: impl Fn(Pointer) -> Message + 'a,
        drag: impl Fn(Pointer) -> Message + 'a,
        hover: impl Fn(Pointer) -> Message + 'a,
        release: Message,
        exit: Message,
    ) -> Self {
        Self {
            press: Box::new(press),
            drag: Box::new(drag),
            hover: Box::new(hover),
            release,
            exit,
        }
    }
}

/// A widget's own transient input state — **not** playback state.
#[derive(Default)]
pub struct State {
    /// The pointer went down on the control and has not come up yet. While
    /// this is set the widget tracks the pointer wherever it goes, which is
    /// what makes a scrub off the end of the bar (and the release that ends
    /// it) work at all.
    ///
    /// "Has not come up yet" is only knowable while the pointer is still
    /// ours, so this is also cleared by every event that says it is not —
    /// see [`lost`].
    held: bool,
    /// The pointer is resting on the control, so a departure is worth
    /// reporting.
    hovered: bool,
}

/// Where `position` falls on `bounds`, in the control's own coordinates. `x`
/// may land outside `0..=width`; clamping is the state machine's job, and it
/// is tested there.
fn measure(position: Point, bounds: Rectangle) -> Pointer {
    Pointer::new(position.x - bounds.x, bounds.width)
}

/// The cursor's position, if it is inside `hit`.
fn over(cursor: mouse::Cursor, hit: Rectangle) -> Option<Point> {
    cursor.position().filter(|position| hit.contains(*position))
}

/// End whatever the pointer was doing, because the widget can no longer be
/// sure it still has it.
///
/// A held control publishes its ordinary `release` — the gesture *commits* at
/// the last position it saw, it is not rolled back — which is also what keeps
/// [`crate::player`] from being left mid-drag, holding a pending position and
/// ignoring the engine's reports forever. A hover publishes its ordinary
/// `exit`, and a held control publishes that too: the pointer that left is not
/// resting on the control, and the preview must not outlive it.
///
/// Both flags are cleared unconditionally, so this cannot itself strand
/// either one, and it is idempotent — a second loss event before the pointer
/// comes back publishes nothing.
fn lost<Message: Clone>(
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

/// One raw event, turned into whatever the control has to say about it.
///
/// `bounds` is what a report is measured against; `hit` is what the pointer
/// has to be inside for a press or a hover to count (module docs). An inert
/// control — `pointers` is `None` — ignores everything, including the loss
/// events, because it has nothing to lose.
pub fn handle<Message: Clone>(
    state: &mut State,
    pointers: Option<&Pointers<'_, Message>>,
    event: &Event,
    bounds: Rectangle,
    hit: Rectangle,
    cursor: mouse::Cursor,
    shell: &mut Shell<'_, Message>,
) -> event::Status {
    let Some(pointers) = pointers else {
        return event::Status::Ignored;
    };
    match event {
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
        | Event::Touch(touch::Event::FingerPressed { .. }) => {
            if let Some(position) = over(cursor, hit) {
                state.held = true;
                shell.publish((pointers.press)(measure(position, bounds)));
                return event::Status::Captured;
            }
        }
        Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
        | Event::Touch(touch::Event::FingerLifted { .. } | touch::Event::FingerLost { .. }) => {
            if state.held {
                state.held = false;
                // The button may well have come up somewhere else entirely:
                // re-read where the pointer actually is so the next move
                // reports the right thing.
                state.hovered = over(cursor, hit).is_some();
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
                // Captured: a held control owns the pointer until it is
                // released, wherever the pointer wanders.
                shell.publish((pointers.drag)(measure(position, bounds)));
                return event::Status::Captured;
            }
            if hit.contains(position) {
                state.hovered = true;
                shell.publish((pointers.hover)(measure(position, bounds)));
            } else if state.hovered {
                state.hovered = false;
                shell.publish(pointers.exit.clone());
            }
            // Hovering is not an interaction: the event is left for whatever
            // else wants it.
        }
        // The pointer is no longer ours: whatever it was doing has to end
        // here, because the event that would normally end it is being
        // delivered to somebody else.
        //
        // Neither arm returns `Captured`. Losing the pointer is a broadcast
        // fact, not an interaction to consume: the fader and the needle are
        // two hand-built widgets in one window and *both* have to hear it,
        // and so does everything else that tracks the pointer or the focus.
        Event::Mouse(mouse::Event::CursorLeft) | Event::Window(window::Event::Unfocused) => {
            lost(state, pointers, shell);
        }
        _ => {}
    }
    event::Status::Ignored
}

/// Which of a control's three paint states it is in.
///
/// `slider::Status` rather than a type of our own, because [`crate::theme`]
/// expresses both hand-built controls with `slider::Style` — the styles stay
/// ordinary iced style functions and the widgets stay a drawing detail.
pub fn status(
    state: &State,
    live: bool,
    hit: Rectangle,
    cursor: mouse::Cursor,
) -> iced::widget::slider::Status {
    use iced::widget::slider::Status;
    if state.held {
        Status::Dragged
    } else if live && over(cursor, hit).is_some() {
        Status::Hovered
    } else {
        Status::Active
    }
}

/// The cursor a control asks for: a hand while the pointer is on it, a grab
/// while it is held, and nothing at all when the control is inert — a control
/// that cannot act says so by leaving the cursor alone rather than by looking
/// identical and doing nothing.
pub fn interaction(
    state: &State,
    live: bool,
    hit: Rectangle,
    cursor: mouse::Cursor,
) -> mouse::Interaction {
    if !live {
        return theme::GROOVE_CURSOR_INERT;
    }
    if state.held {
        theme::GROOVE_CURSOR_HELD
    } else if over(cursor, hit).is_some() {
        theme::GROOVE_CURSOR
    } else {
        theme::GROOVE_CURSOR_INERT
    }
}
