//! The right-hand rail: which panel is on screen, and whether any is.
//!
//! v0.1's scope promised "a fixed layout with hideable panels" and this is the
//! hiding half of it — a small state machine, pure and iced-free (ADR-0006
//! layer 1), that the shell asks one question of per frame: what, if anything,
//! is the rail showing.
//!
//! # One rail, one width
//!
//! baz has two things worth putting beside the shelf — the selected album and
//! the play queue — and they share **one** slot, [`theme::PANEL_W`](crate::theme::PANEL_W)
//! wide. That is a product decision before it is a layout one. The vision's
//! first pillar is that *the library is the interface*: the shelf is the app,
//! and everything else is something you glance at and dismiss. A queue that
//! could sit alongside the album panel would take a second 340 px bite out of
//! the shelf — on the 1280 px default window, five columns of covers become
//! two — and a surface that can occupy half the window on its own is no longer
//! a glance. Sharing the slot bounds the cost of *all* the chrome baz has at
//! the cost of the chrome it already had, and the fifth pillar's "queues are
//! transient" is the same idea from the other end.
//!
//! It also makes the layout property the shell needs almost free: since both
//! panels are the same width, **switching between them reflows nothing**. Only
//! opening or closing the rail moves the shelf, by exactly one panel width,
//! which is the reflow that was asked for. [`Panels::rail`] answering
//! `Some`/`None` is therefore the whole of what the grid geometry has to
//! track.
//!
//! # The rule
//!
//! **The rail shows the last panel that was asked for.** Clicking an album
//! asks for the album panel; the queue toggle asks for the queue. Asking for
//! one puts it up over the other rather than beside it, and the album
//! selection survives underneath — so pressing the queue toggle twice returns
//! you exactly where you were, and the album panel is never lost to a glance
//! at what is next.
//!
//! Dismissal has two spellings and they mean different things:
//!
//! - [`Panels::close`] — the panel's own ✕, and Escape — closes *what is
//!   showing*. Closing the queue reveals the album panel again when one is
//!   selected: you closed the queue, not the rail.
//! - [`Panels::toggle_hidden`] — the keyboard's hide binding — dismisses the
//!   **rail**, whatever is in it, and brings the same thing back on the next
//!   press. This is the "hideable panels" affordance proper: reclaim the width
//!   without losing your place.
//!
//! # Why none of this is persisted
//!
//! Both panels are *contents*-driven, and neither's contents survive a
//! restart: the album panel needs a selection (session state — the shelf is
//! rebuilt from the library on every launch) and the queue panel needs a queue
//! (which lives in the engine process and is never re-sent at startup). So a
//! remembered "the queue was open" would restore a 340 px panel whose entire
//! content is the words *Nothing queued*, taking two columns off the shelf on
//! every launch to say so. That is a worse first frame than the one baz has
//! now, so the choice is deliberately session-scoped.
//!
//! The mechanical cost is the smaller half of the argument but points the same
//! way: `config.rs` is a hand-rolled single-key TOML writer whose documented
//! plan of record is to adopt the `toml` crate once configuration grows past a
//! couple of keys (see its module docs and `docs/BACKLOG.md`). Spending that
//! move on a preference with nothing to show at startup is the wrong first
//! reason to make it.

/// Which panel the right-hand rail is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rail {
    /// The selected album: art, header, edition selector, Play, track list.
    Album,
    /// The play queue: what baz handed the engine, and where it is in it.
    Queue,
}

/// What the right-hand rail is showing, and what it would show.
///
/// The album *selection* is kept here rather than beside it because selection
/// and visibility are one question in practice — "is the album panel up" is
/// "is an album selected, is the queue not covering it, and is the rail not
/// hidden" — and splitting them is how the three get out of step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Panels {
    /// The album whose panel is (or would be) showing. Survives the queue
    /// covering it and the rail being hidden, so both are reversible.
    selected: Option<u64>,
    /// Whether the queue was asked for. Wins the rail while true — it is the
    /// more recent request by construction, since asking for an album clears
    /// it.
    queue: bool,
    /// Whether the rail is dismissed outright. Cleared by any request for a
    /// panel, so hiding is never a state a click gets stuck behind.
    hidden: bool,
}

impl Panels {
    /// Nothing open: the shelf has the whole window, which is where a fresh
    /// baz starts.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// What the rail is showing, if anything. The shell's one question: `Some`
    /// costs the shelf one panel width, `None` costs it nothing, and *which*
    /// panel never enters into it.
    #[must_use]
    pub fn rail(self) -> Option<Rail> {
        if self.hidden {
            None
        } else if self.queue {
            Some(Rail::Queue)
        } else if self.selected.is_some() {
            Some(Rail::Album)
        } else {
            None
        }
    }

    /// The album the panel is for, whether or not it is currently on screen.
    ///
    /// Kept through a hide and through the queue covering it — that is what
    /// makes both reversible — so a caller that wants "is the album panel
    /// showing" must ask [`Self::rail`], not this.
    #[must_use]
    pub fn selected(self) -> Option<u64> {
        self.selected
    }

    /// Whether the album panel is on screen for `id` — the shelf tile's
    /// selected styling.
    #[must_use]
    pub fn showing_album(self, id: u64) -> bool {
        self.rail() == Some(Rail::Album) && self.selected == Some(id)
    }

    /// A tile was clicked: show that album's panel, or close it if its panel
    /// is the one already showing.
    ///
    /// Asking for an album is an unambiguous request to *see* it, so it also
    /// clears the queue and un-hides the rail. The toggle-off arm is
    /// deliberately conditioned on the panel being on screen: clicking the
    /// selected album while the queue covers it brings the album back, which
    /// is what the click was asking for, rather than deselecting something the
    /// user cannot currently see.
    pub fn select(&mut self, id: u64) {
        if self.showing_album(id) {
            self.selected = None;
            return;
        }
        self.selected = Some(id);
        self.queue = false;
        self.hidden = false;
    }

    /// The queue toggle: show the queue, or put back whatever it covered.
    pub fn toggle_queue(&mut self) {
        if self.rail() == Some(Rail::Queue) {
            self.queue = false;
        } else {
            self.queue = true;
            self.hidden = false;
        }
    }

    /// Close what the rail is showing — the panel's ✕, and Escape.
    ///
    /// One step, not a clean sweep: closing the queue reveals the album panel
    /// again when an album is selected, and a second press closes that. A rail
    /// that is already empty (or hidden) has nothing to close and this does
    /// nothing.
    pub fn close(&mut self) {
        match self.rail() {
            Some(Rail::Queue) => self.queue = false,
            Some(Rail::Album) => self.selected = None,
            None => {}
        }
    }

    /// Hide the rail, or bring back what was in it — the keyboard's hide
    /// binding, and the whole of "hideable panels".
    ///
    /// Hiding keeps every other flag, so the next press restores exactly the
    /// panel that was dismissed. Un-hiding a rail that had nothing in it shows
    /// the queue: the alternative is a key that visibly does nothing, and the
    /// queue is the one panel that is always meaningful to ask for (the album
    /// panel needs a selection this state does not have).
    pub fn toggle_hidden(&mut self) {
        if self.rail().is_some() {
            self.hidden = true;
            return;
        }
        self.hidden = false;
        if self.selected.is_none() {
            self.queue = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_rail_is_empty() {
        let panels = Panels::new();
        assert_eq!(panels.rail(), None);
        assert_eq!(panels.selected(), None);
        assert!(!panels.showing_album(1));
    }

    #[test]
    fn selecting_opens_the_album_panel_and_reselecting_closes_it() {
        let mut panels = Panels::new();
        panels.select(7);
        assert_eq!(panels.rail(), Some(Rail::Album));
        assert_eq!(panels.selected(), Some(7));
        assert!(panels.showing_album(7));
        assert!(!panels.showing_album(8));

        // A different album swaps the panel's contents, not the rail.
        panels.select(8);
        assert_eq!(panels.rail(), Some(Rail::Album));
        assert_eq!(panels.selected(), Some(8));

        // The same album again closes it.
        panels.select(8);
        assert_eq!(panels.rail(), None);
        assert_eq!(panels.selected(), None);
    }

    #[test]
    fn the_queue_toggle_covers_the_album_panel_and_gives_it_back() {
        let mut panels = Panels::new();
        panels.select(7);
        panels.toggle_queue();
        assert_eq!(panels.rail(), Some(Rail::Queue));
        assert_eq!(
            panels.selected(),
            Some(7),
            "the selection survives underneath, or the toggle is not reversible"
        );
        assert!(!panels.showing_album(7), "the album panel is covered");

        panels.toggle_queue();
        assert_eq!(panels.rail(), Some(Rail::Album));
        assert!(panels.showing_album(7));
    }

    #[test]
    fn the_queue_opens_over_nothing_and_closes_to_nothing() {
        let mut panels = Panels::new();
        panels.toggle_queue();
        assert_eq!(panels.rail(), Some(Rail::Queue));
        panels.toggle_queue();
        assert_eq!(panels.rail(), None);
    }

    /// Clicking a tile is a request to see that album, whatever is in the way.
    #[test]
    fn clicking_a_tile_while_the_queue_shows_brings_the_album_up() {
        let mut panels = Panels::new();
        panels.select(7);
        panels.toggle_queue();
        // The *selected* album, the one whose panel is hidden behind the
        // queue: this must show it, not deselect it.
        panels.select(7);
        assert_eq!(panels.rail(), Some(Rail::Album));
        assert!(panels.showing_album(7));

        // And another album likewise.
        panels.toggle_queue();
        panels.select(9);
        assert_eq!(panels.rail(), Some(Rail::Album));
        assert_eq!(panels.selected(), Some(9));
    }

    #[test]
    fn close_peels_the_queue_then_the_album() {
        let mut panels = Panels::new();
        panels.select(7);
        panels.toggle_queue();

        panels.close();
        assert_eq!(panels.rail(), Some(Rail::Album));
        panels.close();
        assert_eq!(panels.rail(), None);
        assert_eq!(panels.selected(), None);
        // Closing an empty rail is a no-op, not an error state.
        panels.close();
        assert_eq!(panels.rail(), None);
    }

    #[test]
    fn hiding_reclaims_the_width_and_restores_the_same_panel() {
        let mut panels = Panels::new();
        panels.select(7);
        panels.toggle_hidden();
        assert_eq!(panels.rail(), None, "the shelf gets the width back");
        assert_eq!(panels.selected(), Some(7), "and the place is kept");

        panels.toggle_hidden();
        assert_eq!(panels.rail(), Some(Rail::Album));

        // The same for the queue.
        panels.toggle_queue();
        panels.toggle_hidden();
        assert_eq!(panels.rail(), None);
        panels.toggle_hidden();
        assert_eq!(panels.rail(), Some(Rail::Queue));
    }

    #[test]
    fn un_hiding_an_empty_rail_shows_the_queue() {
        let mut panels = Panels::new();
        panels.toggle_hidden();
        assert_eq!(panels.rail(), Some(Rail::Queue));
    }

    /// Hiding must never be a state a click gets stuck behind.
    #[test]
    fn any_request_for_a_panel_un_hides_the_rail() {
        let mut panels = Panels::new();
        panels.select(7);
        panels.toggle_hidden();
        panels.select(7);
        assert_eq!(panels.rail(), Some(Rail::Album));

        let mut panels = Panels::new();
        panels.select(7);
        panels.toggle_hidden();
        panels.toggle_queue();
        assert_eq!(panels.rail(), Some(Rail::Queue));
    }

    /// The reflow claim, stated directly: swapping which panel is up never
    /// changes *whether* one is up, so the shelf keeps its width across every
    /// switch. Only opening and closing move it.
    #[test]
    fn switching_panels_never_changes_whether_the_rail_is_occupied() {
        // Album → queue.
        let mut panels = Panels::new();
        panels.select(7);
        assert!(panels.rail().is_some());
        panels.toggle_queue();
        assert!(panels.rail().is_some());
        // Queue → album, by the toggle and by a tile click.
        panels.toggle_queue();
        assert!(panels.rail().is_some());
        panels.toggle_queue();
        panels.select(9);
        assert!(panels.rail().is_some());
        // Album → a different album.
        panels.select(11);
        assert!(panels.rail().is_some());
        // Closing the queue over a selection reveals the album panel, which is
        // a switch and not a close: the width must not come back yet.
        panels.toggle_queue();
        panels.close();
        assert_eq!(panels.rail(), Some(Rail::Album));
    }

    /// The invariants a rendering bug would hide behind, checked over every
    /// path of four moves through the machine — so a future arm cannot quietly
    /// break either one.
    #[test]
    fn no_reachable_state_shows_an_album_panel_without_an_album() {
        #[derive(Debug, Clone, Copy)]
        enum Step {
            Select(u64),
            Queue,
            Close,
            Hide,
        }
        let steps = [
            Step::Select(1),
            Step::Select(2),
            Step::Queue,
            Step::Close,
            Step::Hide,
        ];
        for a in steps {
            for b in steps {
                for c in steps {
                    for d in steps {
                        let mut panels = Panels::new();
                        for step in [a, b, c, d] {
                            match step {
                                Step::Select(id) => panels.select(id),
                                Step::Queue => panels.toggle_queue(),
                                Step::Close => panels.close(),
                                Step::Hide => panels.toggle_hidden(),
                            }
                            assert!(
                                panels.rail() != Some(Rail::Album) || panels.selected().is_some(),
                                "{step:?} left an album panel with no album"
                            );
                            // `showing_album` and `rail` must never disagree:
                            // the tile highlight and the panel are one fact.
                            let highlighted = [1, 2].iter().filter(|&&id| panels.showing_album(id));
                            assert_eq!(
                                highlighted.count(),
                                usize::from(panels.rail() == Some(Rail::Album)),
                                "{step:?} lit a tile the rail is not showing"
                            );
                        }
                    }
                }
            }
        }
    }
}
