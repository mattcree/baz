//! Which **place** the window is showing.
//!
//! ADR-0006 layer 1 — pure, iced-free, unit-tested — and the smallest module in
//! the crate, which is the point. ADR-0016's model is *the window holds one
//! place at a time, one inspector attached to that place, one popover attached
//! to the transport, and the now-playing bar always*; this is the first of the
//! four kinds, and there is exactly one of it on screen.
//!
//! # Why settings became a place
//!
//! They used to be a panel in the right-hand rail, sharing 340 px with the
//! album inspector and the queue. The argument that put them there was that the
//! rail is "the one deliberate layer down" the vision's progressive-disclosure
//! pillar names — which was true, and was the disease: when the only non-shelf
//! surface in the product is the rail, every new surface becomes a rail panel
//! and the rail becomes a junk drawer with a keyboard shortcut per item.
//!
//! Three things the audit found, in the settings' own case:
//!
//! - **They are not a glance.** The other two tenants were things you look at
//!   *while* browsing; a preference is a standing decision you make and leave.
//! - **The rail was simultaneously too narrow and too empty for them.** Five
//!   controls, the steppers crushed against the right edge, and roughly 360 px
//!   of nothing beneath them — two columns of covers spent on that.
//! - **They do not fit what is coming.** The output device and exclusive mode,
//!   a signal-path diagram, library roots and watch folders, per-feature
//!   enrichment consent. None of those is a section in a 292 px column.
//!
//! The cost of a place is leaving the shelf, and that is the right cost: you
//! are not browsing while you set a pre-amp. It is free to reverse, because the
//! Library's whole state — scroll, query, selection — lives in one struct that
//! nothing here touches.
//!
//! # Why an enum rather than a stack
//!
//! Because places **replace** each other; two are never on screen together, and
//! there is no history to walk. That is what makes [`Place::back`] a total
//! function with no argument and no `Option`, and what keeps <kbd>Esc</kbd>'s
//! rule to one line per layer. A navigation stack would be the right shape for
//! a product with drill-down; baz has one home and one deliberate layer beside
//! it, and modelling more would be modelling something that cannot happen.

/// The place the window is showing.
///
/// [`Self::Library`] is home and the default: a fresh baz, and a baz that has
/// just been backed out of anywhere, is looking at the shelf.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Place {
    /// The shelf, its search, its counts — and the album inspector attached to
    /// it. The interface, per the vision's first pillar.
    #[default]
    Library,
    /// Everything that is a standing decision: today ReplayGain (ADR-0013),
    /// and the shape every setting after it takes.
    Settings,
}

impl Place {
    /// <kbd>Ctrl</kbd>+<kbd>,</kbd>, and the top bar's `Settings` control: go
    /// to the settings, or come back from them.
    ///
    /// The same key it always was, now meaning *navigation* rather than
    /// *show a panel* — which is what the macOS convention it borrows has
    /// always meant, and what makes it the one binding here a listener is more
    /// likely to already know than to learn.
    #[must_use]
    pub fn toggled(self) -> Self {
        match self {
            Self::Library => Self::Settings,
            Self::Settings => Self::Library,
        }
    }

    /// <kbd>Esc</kbd>, and the Back affordance: return home.
    ///
    /// Distinct from [`Self::toggled`] because a *back* that toggled would send
    /// you into the settings from the Library, which is not what backing out
    /// of somewhere means anywhere. Home is already home, so this is a no-op
    /// there — and the shell's <kbd>Esc</kbd> asks [`Self::is_settings`] first,
    /// so the key falls through to the layers underneath rather than being
    /// silently eaten by a place that had nothing to leave.
    #[must_use]
    #[expect(
        clippy::unused_self,
        reason = "back is a navigation verb and reads on the place you are \
                  leaving; that today's answer ignores where you were is the \
                  *finding* — there is one home and no history — not an \
                  accident of the signature. A stack would change the body and \
                  not the call sites."
    )]
    pub fn back(self) -> Self {
        Self::Library
    }

    /// Whether the settings are the place on screen.
    #[must_use]
    pub fn is_settings(self) -> bool {
        self == Self::Settings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_window_is_at_home() {
        assert_eq!(Place::default(), Place::Library);
        assert!(!Place::default().is_settings());
    }

    #[test]
    fn the_settings_key_is_a_true_toggle() {
        let place = Place::default();
        assert_eq!(place.toggled(), Place::Settings);
        assert_eq!(place.toggled().toggled(), Place::Library);
        assert!(place.toggled().is_settings());
    }

    /// Back means *home*, not *the other one*. Anywhere, any number of times.
    #[test]
    fn back_always_means_the_library() {
        assert_eq!(Place::Settings.back(), Place::Library);
        assert_eq!(Place::Library.back(), Place::Library);
        assert_eq!(Place::Settings.back().back(), Place::Library);
    }

    /// Over every path of four moves: one place is on screen, it is always one
    /// of the two, and `is_settings` never disagrees with the value it reads.
    #[test]
    fn no_reachable_state_is_two_places_at_once() {
        #[derive(Debug, Clone, Copy)]
        enum Step {
            Toggle,
            Back,
        }
        let steps = [Step::Toggle, Step::Back];
        for a in steps {
            for b in steps {
                for c in steps {
                    for d in steps {
                        let mut place = Place::default();
                        for step in [a, b, c, d] {
                            place = match step {
                                Step::Toggle => place.toggled(),
                                Step::Back => place.back(),
                            };
                            assert_eq!(
                                place.is_settings(),
                                place == Place::Settings,
                                "{step:?} left the two readings disagreeing"
                            );
                        }
                    }
                }
            }
        }
    }
}
