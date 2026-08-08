//! The one overlay layer: which popover, if any, is floating over the place.
//!
//! ADR-0006 layer 1 — pure, iced-free, unit-tested — and the smallest of the
//! four kinds ADR-0015's model names. The window holds one *place* at a time,
//! one *inspector* attached to that place, one *popover* attached to the
//! transport, and the now-playing *bar* always; this module is the popover.
//!
//! # Why this is not another rail tenant
//!
//! The queue used to be one of three subjects taking turns in the right-hand
//! rail, and the audit's central finding was that the rail is a *slot* rather
//! than a place (`docs/design/01-ux-audit-and-ia.md` §2.1). The queue is not
//! about the library at all: it is a live readout of the engine, and it belongs
//! next to the thing it describes. So it left the rail, and what it became is
//! anchored to the now-playing bar — one glance, one dismissal, and **no
//! reflow**: an overlay costs the shelf nothing, where the rail cost it two
//! columns of covers every time somebody wondered what was next.
//!
//! # Non-modal, deliberately and testably
//!
//! iced 0.13 has no focus containment and publishes no accessibility tree, so
//! a modal overlay is not something this toolkit can honestly offer (§4.6 of
//! the spec). Rather than imitate one, the popover is explicitly **not** modal:
//!
//! - every keyboard binding keeps working underneath it — the transport, the
//!   volume, search focus, all of it. Only <kbd>Esc</kbd> is answered by the
//!   popover first, because <kbd>Esc</kbd> is the one key whose meaning is
//!   "peel the top layer";
//! - the shelf underneath still scrolls;
//! - there is **no scrim**. Dimming ten thousand covers to show twelve rows
//!   would contradict the palette rationale the whole room is built on;
//! - the bar it is anchored to is never covered, and keeps every reserved
//!   pixel.
//!
//! What it does take is a press outside itself, which iced 0.13 *does* support
//! (`stack` + `mouse_area(…).on_press(close)` under an `opaque(popover)`), and
//! which is the dismissal a listener will reach for first.
//!
//! # One member of the kind
//!
//! [`Popover`] has one variant today and the type still earns its place: it is
//! what makes "two popovers are open at once" a state that does not exist to be
//! got into, and it is where the second one — should baz ever grow it — has to
//! declare itself rather than adding a parallel flag.

/// Which popover is floating over the current place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Popover {
    /// **Up next**: the queue the engine holds, and where it is in it.
    /// Anchored to the now-playing bar, which is the surface it describes.
    UpNext,
}

/// The overlay layer: at most one popover, over whatever place is on screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Overlay {
    showing: Option<Popover>,
}

impl Overlay {
    /// Nothing floating, which is where every session starts and where every
    /// dismissal returns.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Which popover is on screen, if any — the shell's one question per
    /// frame.
    #[must_use]
    pub fn showing(self) -> Option<Popover> {
        self.showing
    }

    /// Whether anything is floating. The layout's question, where
    /// [`Self::showing`] is the view's.
    #[must_use]
    pub fn is_open(self) -> bool {
        self.showing.is_some()
    }

    /// The **Up next** affordance and <kbd>Q</kbd>: open it, or put it away
    /// if it is what is already showing.
    pub fn toggle_up_next(&mut self) {
        self.toggle(Popover::UpNext);
    }

    /// Show `popover`, or dismiss it if it is the one already showing.
    ///
    /// There is one slot, so raising a popover over another replaces it rather
    /// than stacking — but with one member of the kind that case cannot arise
    /// yet, and the arithmetic is written so it stays a replacement when it
    /// can.
    fn toggle(&mut self, popover: Popover) {
        self.showing = if self.showing == Some(popover) {
            None
        } else {
            Some(popover)
        };
    }

    /// Dismiss whatever is showing, reporting whether there *was* anything.
    ///
    /// The boolean is the whole of <kbd>Esc</kbd>'s layering rule: the top
    /// layer answers first and says so, and the layers under it only hear the
    /// press when it did not. A close that closed nothing must not consume the
    /// key, or <kbd>Esc</kbd> would go dead over an empty overlay.
    pub fn close(&mut self) -> bool {
        self.showing.take().is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_overlay_is_empty() {
        let overlay = Overlay::new();
        assert_eq!(overlay.showing(), None);
        assert!(!overlay.is_open());
    }

    #[test]
    fn the_affordance_opens_up_next_and_puts_it_away_again() {
        let mut overlay = Overlay::new();
        overlay.toggle_up_next();
        assert_eq!(overlay.showing(), Some(Popover::UpNext));
        assert!(overlay.is_open());

        overlay.toggle_up_next();
        assert_eq!(overlay.showing(), None);
    }

    /// The layering rule <kbd>Esc</kbd> is built on: a close that closed
    /// something says so, and one that closed nothing lets the press fall
    /// through to the layer beneath.
    #[test]
    fn close_reports_whether_it_had_anything_to_close() {
        let mut overlay = Overlay::new();
        assert!(!overlay.close(), "an empty overlay must not eat the key");

        overlay.toggle_up_next();
        assert!(overlay.close(), "an open popover answers the key itself");
        assert_eq!(overlay.showing(), None);
        assert!(!overlay.close(), "and only once");
    }

    /// Over every path of four moves: at most one popover is ever showing, and
    /// `is_open` and `showing` are one fact rather than two that can disagree.
    #[test]
    fn no_reachable_state_holds_more_than_one_popover() {
        #[derive(Debug, Clone, Copy)]
        enum Step {
            UpNext,
            Close,
        }
        let steps = [Step::UpNext, Step::Close];
        for a in steps {
            for b in steps {
                for c in steps {
                    for d in steps {
                        let mut overlay = Overlay::new();
                        for step in [a, b, c, d] {
                            match step {
                                Step::UpNext => overlay.toggle_up_next(),
                                Step::Close => {
                                    overlay.close();
                                }
                            }
                            assert_eq!(
                                overlay.is_open(),
                                overlay.showing().is_some(),
                                "{step:?} left the two readings disagreeing"
                            );
                        }
                    }
                }
            }
        }
    }
}
