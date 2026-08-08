//! The album inspector: which album it is showing, and whether it is showing.
//!
//! ADR-0006 layer 1 — pure, iced-free, unit-tested. This is `panels.rs` with
//! the roommates removed, and it is **strictly less state than it used to
//! carry**: two fields where there were three, and one question where there
//! was a paragraph.
//!
//! # What this module used to be
//!
//! It arbitrated a *rail* — one 340 px slot beside the shelf, shared by three
//! unrelated subjects: the album you had pointed at, the play queue, and the
//! application's settings. The audit's central finding was that they shared
//! nothing except a width, and that the rail was therefore a **slot rather than
//! a place** (ADR-0016; `docs/design/01-ux-audit-and-ia.md` §2.1). The
//! machinery here was not the problem — it was careful, pure and exhaustively
//! tested — it was correctly implementing a model with no answer, and the
//! give-away was in this file: un-hiding an *empty* rail used to open the
//! **queue**, because a key whose entire job is "give the shelf its width back"
//! had to invent content from somewhere.
//!
//! The queue left for a popover anchored to the bar it describes
//! ([`crate::overlay`]); the settings left for a place of their own
//! ([`crate::place`]). What is left is the one tenant that genuinely needs the
//! shelf beside it — you compare, you click the next sleeve — and with one
//! tenant the rule collapses from a paragraph to a sentence:
//!
//! > **The inspector is open exactly when an album is selected and the column
//! > is not hidden.**
//!
//! # The two dismissals, and why they are different
//!
//! - [`Selection::close`] — the inspector's ✕, and <kbd>Esc</kbd> — **forgets**
//!   the album. There is nothing underneath any more, so closing is closing.
//! - [`Selection::toggle_hidden`] — <kbd>Ctrl</kbd>+<kbd>B</kbd> — **keeps** it
//!   and reclaims the width, and the next press brings back exactly what was
//!   dismissed. This is the "hideable panels" affordance proper, and it is an
//!   honest sidebar toggle now that there is exactly one sidebar.
//!
//! Un-hiding an empty column no longer conjures a panel. It selects the
//! **playing** album if there is one — which is a real answer to "show me
//! something", built from playback truth rather than from whichever tenant was
//! most often meaningful — and otherwise does nothing at all, which is what a
//! layout key should do.
//!
//! # Why none of this is persisted
//!
//! *Visibility* is session state. The inspector needs a selection, and the
//! shelf is rebuilt from the library on every launch, so a remembered "the
//! inspector was open" would restore a column whose contents are a guess about
//! an album the listener has not looked at since. That is a worse first frame
//! than the one baz has, so the choice stays session-scoped — unchanged from
//! when this module had three tenants, and worth restating rather than
//! silently inheriting.

/// Which album the inspector is showing, and whether it is on screen.
///
/// The selection is kept *here* rather than beside it because selection and
/// visibility are one question — "is the inspector up" is "is an album
/// selected, and is the column not hidden" — and splitting them is how the two
/// get out of step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Selection {
    /// The album the inspector is (or would be) showing. Survives a hide, so
    /// hiding is reversible.
    selected: Option<u64>,
    /// Whether the column is dismissed outright. Cleared by any request for an
    /// album, so hiding is never a state a click gets stuck behind.
    hidden: bool,
}

impl Selection {
    /// Nothing selected: the shelf has the whole window, which is where a fresh
    /// baz starts.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The album the inspector is showing, if it is showing one — the shell's
    /// one question. `Some` costs the shelf one panel width and `None` costs it
    /// nothing, so this is also the whole of what the grid geometry tracks.
    #[must_use]
    pub fn inspecting(self) -> Option<u64> {
        if self.hidden { None } else { self.selected }
    }

    /// The album the inspector is *for*, whether or not it is on screen.
    ///
    /// Kept through a hide — that is what makes the hide reversible — so a
    /// caller that wants "is the inspector showing" must ask
    /// [`Self::inspecting`], not this.
    #[must_use]
    pub fn selected(self) -> Option<u64> {
        self.selected
    }

    /// Whether the inspector is on screen for `id` — the shelf tile's selected
    /// styling.
    #[must_use]
    pub fn showing_album(self, id: u64) -> bool {
        self.inspecting() == Some(id)
    }

    /// A tile was clicked: show that album, or close the inspector if that
    /// album is the one already showing.
    ///
    /// Asking for an album is an unambiguous request to *see* it, so it also
    /// un-hides the column. The toggle-off arm is conditioned on the inspector
    /// being on screen rather than on the selection alone: clicking the
    /// selected album while the column is hidden brings it back, which is what
    /// the click was asking for, rather than deselecting something the user
    /// cannot currently see.
    pub fn select(&mut self, id: u64) {
        if self.showing_album(id) {
            self.selected = None;
            return;
        }
        self.selected = Some(id);
        self.hidden = false;
    }

    /// Close the inspector — its ✕, and <kbd>Esc</kbd>.
    ///
    /// A close, not a hide: with one tenant there is nothing underneath to
    /// reveal, so this forgets the album. An inspector that is already closed
    /// (or hidden) has nothing to close and this does nothing.
    pub fn close(&mut self) {
        if self.inspecting().is_some() {
            self.selected = None;
        }
    }

    /// Hide the column, or bring it back — <kbd>Ctrl</kbd>+<kbd>B</kbd>, and
    /// the whole of "hideable panels".
    ///
    /// Hiding keeps the selection, so the next press restores exactly what was
    /// dismissed.
    ///
    /// `playing` is the album that is sounding, if one is. It is consulted in
    /// exactly one case — un-hiding with nothing selected — and it is what
    /// replaces the rule the audit caught: the old code opened the *queue*
    /// there, because a layout key had to invent content and the queue was the
    /// one tenant always meaningful to ask for. The playing album is a real
    /// answer to "show me something" rather than an invented one, and when
    /// there is nothing playing the key does nothing, which is what a key whose
    /// job is to give the shelf its width back should do.
    pub fn toggle_hidden(&mut self, playing: Option<u64>) {
        if self.inspecting().is_some() {
            self.hidden = true;
            return;
        }
        self.hidden = false;
        if self.selected.is_none() {
            self.selected = playing;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_inspector_is_closed() {
        let selection = Selection::new();
        assert_eq!(selection.inspecting(), None);
        assert_eq!(selection.selected(), None);
        assert!(!selection.showing_album(1));
    }

    #[test]
    fn selecting_opens_the_inspector_and_reselecting_closes_it() {
        let mut selection = Selection::new();
        selection.select(7);
        assert_eq!(selection.inspecting(), Some(7));
        assert!(selection.showing_album(7));
        assert!(!selection.showing_album(8));

        // A different album swaps the contents, not the column.
        selection.select(8);
        assert_eq!(selection.inspecting(), Some(8));

        // The same album again closes it.
        selection.select(8);
        assert_eq!(selection.inspecting(), None);
        assert_eq!(selection.selected(), None);
    }

    #[test]
    fn close_forgets_the_album_and_is_a_no_op_when_there_is_none() {
        let mut selection = Selection::new();
        selection.close();
        assert_eq!(selection.inspecting(), None);

        selection.select(7);
        selection.close();
        assert_eq!(selection.inspecting(), None);
        assert_eq!(selection.selected(), None);
    }

    #[test]
    fn hiding_reclaims_the_width_and_restores_the_same_album() {
        let mut selection = Selection::new();
        selection.select(7);
        selection.toggle_hidden(None);
        assert_eq!(
            selection.inspecting(),
            None,
            "the shelf gets the width back"
        );
        assert_eq!(selection.selected(), Some(7), "and the place is kept");

        selection.toggle_hidden(None);
        assert_eq!(selection.inspecting(), Some(7));
    }

    /// The rule §4.8 deletes, and what replaced it: un-hiding an empty column
    /// no longer conjures a panel out of nothing. It offers the album that is
    /// *playing*, which is a fact rather than an invention — and offers nothing
    /// when nothing is.
    #[test]
    fn un_hiding_an_empty_column_shows_the_playing_album_or_nothing() {
        let mut selection = Selection::new();
        selection.toggle_hidden(None);
        assert_eq!(
            selection.inspecting(),
            None,
            "a layout key must not create content out of nothing"
        );

        let mut selection = Selection::new();
        selection.toggle_hidden(Some(42));
        assert_eq!(selection.inspecting(), Some(42));

        // …and it does not *override* a selection that is merely hidden.
        let mut selection = Selection::new();
        selection.select(7);
        selection.toggle_hidden(Some(42));
        assert_eq!(selection.inspecting(), None);
        selection.toggle_hidden(Some(42));
        assert_eq!(
            selection.inspecting(),
            Some(7),
            "the hide is reversible, and reversing it restores what was dismissed"
        );
    }

    /// Hiding must never be a state a click gets stuck behind.
    #[test]
    fn selecting_an_album_un_hides_the_column() {
        let mut selection = Selection::new();
        selection.select(7);
        selection.toggle_hidden(None);
        selection.select(7);
        assert_eq!(selection.inspecting(), Some(7));

        let mut selection = Selection::new();
        selection.select(7);
        selection.toggle_hidden(None);
        selection.select(9);
        assert_eq!(selection.inspecting(), Some(9));
    }

    /// The reflow claim, stated directly: swapping which album the inspector
    /// shows never changes *whether* it is showing, so the shelf keeps its
    /// width across every switch. Only opening and closing move it.
    #[test]
    fn swapping_albums_never_changes_whether_the_column_is_occupied() {
        let mut selection = Selection::new();
        selection.select(7);
        assert!(selection.inspecting().is_some());
        selection.select(9);
        assert!(selection.inspecting().is_some());
        selection.select(11);
        assert!(selection.inspecting().is_some());
    }

    /// **No reachable state shows an inspector without an album** — one of the
    /// four properties `docs/design/01-ux-audit-and-ia.md` §5 says must not
    /// regress, carried over from `panels.rs` and, as the spec predicted,
    /// simpler: with one tenant it is a property of two fields rather than of
    /// three, and `showing_album` and `inspecting` are now literally the same
    /// expression.
    ///
    /// It is still walked exhaustively, because "obviously true" is what the
    /// old rail's rule looked like from inside as well.
    #[test]
    fn no_reachable_state_shows_an_inspector_without_an_album() {
        #[derive(Debug, Clone, Copy)]
        enum Step {
            Select(u64),
            Close,
            Hide,
            HideWhilePlaying(u64),
        }
        let steps = [
            Step::Select(1),
            Step::Select(2),
            Step::Close,
            Step::Hide,
            Step::HideWhilePlaying(3),
        ];
        for a in steps {
            for b in steps {
                for c in steps {
                    for d in steps {
                        let mut selection = Selection::new();
                        for step in [a, b, c, d] {
                            match step {
                                Step::Select(id) => selection.select(id),
                                Step::Close => selection.close(),
                                Step::Hide => selection.toggle_hidden(None),
                                Step::HideWhilePlaying(id) => selection.toggle_hidden(Some(id)),
                            }
                            let showing = selection.inspecting();
                            assert!(
                                showing.is_none() || selection.selected().is_some(),
                                "{step:?} left an inspector with no album"
                            );
                            // `showing_album` and `inspecting` must never
                            // disagree: the tile highlight and the column are
                            // one fact.
                            let lit = [1, 2, 3]
                                .iter()
                                .filter(|&&id| selection.showing_album(id))
                                .count();
                            assert_eq!(
                                lit,
                                usize::from(showing.is_some()),
                                "{step:?} lit a tile the inspector is not showing"
                            );
                        }
                    }
                }
            }
        }
    }
}
