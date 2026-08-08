//! The right-hand rail: which panel is on screen, and whether any is.
//!
//! v0.1's scope promised "a fixed layout with hideable panels" and this is the
//! hiding half of it — a small state machine, pure and iced-free (ADR-0006
//! layer 1), that the shell asks one question of per frame: what, if anything,
//! is the rail showing.
//!
//! # One rail, one width
//!
//! baz has three things worth putting beside the shelf — the selected album,
//! the play queue and the settings — and they share **one** slot,
//! [`theme::PANEL_W`](crate::theme::PANEL_W)
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
//! It also makes the layout property the shell needs almost free: since every
//! panel is the same width, **switching between them reflows nothing**. Only
//! opening or closing the rail moves the shelf, by exactly one panel width,
//! which is the reflow that was asked for. [`Panels::rail`] answering
//! `Some`/`None` is therefore the whole of what the grid geometry has to
//! track.
//!
//! # Why settings are a rail panel, and not a popover
//!
//! baz had no settings surface at all before ReplayGain needed one (ADR-0013),
//! so whatever went in became the pattern for every setting that follows —
//! the output device, exclusive mode, watch folders, the enrichment toggles
//! the vision's fifth pillar promises are off by default. That is the decision
//! being made here, and the rail wins it on four counts:
//!
//! - **It is the layer the sixth pillar already describes.** *Progressive
//!   disclosure*: a Devon-simple surface, with Karl's output chain and Sam's
//!   server settings "one deliberate layer down". The rail *is* that layer —
//!   it is where baz already puts everything that is not the shelf — and a
//!   settings surface that invented a second one would make the interface two
//!   layers deep to save a panel.
//! - **It cannot cover the music or the transport.** A floating popover sits
//!   *on* the shelf; the rail sits beside it, and the bottom bar keeps every
//!   pixel it reserves. A settings sheet that hid the covers would contradict
//!   the pillar it is supposed to serve.
//! - **It inherits every dismissal baz already has** — the ✕, Escape, and
//!   <kbd>Ctrl</kbd>+<kbd>B</kbd> — rather than hand-rolling click-outside
//!   dismissal, focus containment and placement, which iced 0.13 gives no
//!   primitive for. That machinery would live in the *disposable* view layer
//!   (ADR-0006 layer 3), which is precisely where a redesign should find the
//!   least invention.
//! - **It scales the way settings actually grow.** The next setting is a
//!   section in this panel, not a second popover; the panel scrolls, and the
//!   rail's width never changes, so nothing about the layout has to be
//!   revisited to add one.
//!
//! What it costs is honest and is the same cost the queue pays: opening the
//! rail when it is empty takes [`theme::PANEL_W`](crate::theme::PANEL_W) from
//! the shelf. Opening the settings over a panel that is *already* up — which
//! is the ordinary case, since selecting an album is how anyone reaches the
//! music — moves nothing at all.
//!
//! # The rule
//!
//! **The rail shows the last panel that was asked for.** Clicking an album
//! asks for the album panel; the queue toggle asks for the queue, and the
//! settings toggle for the settings. Asking for one puts it up over the others
//! rather than beside them, and the album selection survives underneath — so
//! pressing a toggle twice returns you exactly where you were, and the album
//! panel is never lost to a glance at what is next.
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
//! *Visibility* is session state, and that is unchanged by the settings panel
//! arriving — including by the fact that what the settings panel *contains* is
//! now persisted (see [`crate::config`]). The two are different questions and
//! it is worth being explicit that the answer to one did not move the other.
//!
//! Every panel here is *contents*-driven, and no panel's contents survive a
//! restart in a way that would make reopening it useful: the album panel needs
//! a selection (session state — the shelf is rebuilt from the library on every
//! launch), the queue panel needs a queue (which lives in the engine process
//! and is never re-sent at startup), and the settings panel is a place you go
//! to change something and then leave. So a remembered "the queue was open"
//! would restore a 340 px panel whose entire content is the words *Nothing
//! queued*, taking two columns off the shelf on every launch to say so, and a
//! remembered "the settings were open" would greet every launch with a
//! control nobody is reaching for. Both are worse first frames than the one
//! baz has now, so the choice stays session-scoped.
//!
//! The mechanical half of the old argument has expired and is recorded rather
//! than quietly dropped: `config.rs` was a hand-rolled single-key TOML writer,
//! and spending the move to the `toml` crate on a preference with nothing to
//! show at startup was the wrong first reason to make it. ReplayGain was the
//! right one, the move has been made, and persisting a panel's visibility is
//! now cheap — and still not worth doing, for the reason above, which was
//! always the larger half.

/// Which panel the right-hand rail is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rail {
    /// The selected album: art, header, edition selector, Play, track list.
    Album,
    /// The play queue: what baz handed the engine, and where it is in it.
    Queue,
    /// The settings: today, ReplayGain (ADR-0013). See the module's note on
    /// why this is a rail panel and not a popover.
    Settings,
}

/// What the right-hand rail is showing, and what it would show.
///
/// The album *selection* is kept here rather than beside it because selection
/// and visibility are one question in practice — "is the album panel up" is
/// "is an album selected, is the queue not covering it, and is the rail not
/// hidden" — and splitting them is how the three get out of step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Panels {
    /// The album whose panel is (or would be) showing. Survives another panel
    /// covering it and the rail being hidden, so both are reversible.
    selected: Option<u64>,
    /// The panel asked for over the album — the queue or the settings, never
    /// [`Rail::Album`], which is asked for by selecting an album instead. It
    /// wins the rail while set, because it is the more recent request by
    /// construction: asking for an album clears it.
    ///
    /// One slot rather than a flag per panel, so "two are open at once" is not
    /// a state that exists to be got into.
    overlay: Option<Rail>,
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
        } else if self.overlay.is_some() {
            self.overlay
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
        self.overlay = None;
        self.hidden = false;
    }

    /// The queue toggle: show the queue, or put back whatever it covered.
    pub fn toggle_queue(&mut self) {
        self.toggle_overlay(Rail::Queue);
    }

    /// The settings toggle: show the settings, or put back whatever they
    /// covered. The queue's behaviour exactly — one control, one panel, one
    /// slot — because a listener should not have to learn two dismissal rules.
    pub fn toggle_settings(&mut self) {
        self.toggle_overlay(Rail::Settings);
    }

    /// Show `panel` over whatever the rail holds, or put that back if `panel`
    /// is what is already showing.
    ///
    /// `panel` is never [`Rail::Album`]: the album panel is asked for by
    /// selecting an album, which is [`Self::select`]'s job, and routing it
    /// through here would let a toggle raise a panel with no album in it.
    fn toggle_overlay(&mut self, panel: Rail) {
        debug_assert_ne!(panel, Rail::Album, "the album panel is raised by select");
        if self.rail() == Some(panel) {
            self.overlay = None;
        } else {
            self.overlay = Some(panel);
            self.hidden = false;
        }
    }

    /// Close what the rail is showing — the panel's ✕, and Escape.
    ///
    /// One step, not a clean sweep: closing the queue or the settings reveals
    /// the album panel again when an album is selected, and a second press
    /// closes that. A rail that is already empty (or hidden) has nothing to
    /// close and this does nothing.
    pub fn close(&mut self) {
        match self.rail() {
            Some(Rail::Album) => self.selected = None,
            Some(_) => self.overlay = None,
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
        if self.overlay.is_none() && self.selected.is_none() {
            self.overlay = Some(Rail::Queue);
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
        // The settings arrive on the same terms: raising them over a panel
        // that is already up, and dismissing them again, is a switch.
        panels.toggle_settings();
        assert!(panels.rail().is_some());
        panels.toggle_queue();
        assert!(panels.rail().is_some());
        panels.toggle_settings();
        assert!(panels.rail().is_some());
        panels.close();
        assert_eq!(panels.rail(), Some(Rail::Album));
    }

    /// The settings toggle behaves exactly as the queue's does — one control,
    /// one panel, one slot — so a listener learns the rule once.
    #[test]
    fn the_settings_toggle_covers_and_uncovers_like_the_queue() {
        let mut panels = Panels::new();
        panels.toggle_settings();
        assert_eq!(panels.rail(), Some(Rail::Settings));
        panels.toggle_settings();
        assert_eq!(panels.rail(), None);

        // Over an album panel: covers it, gives it back, never loses it.
        panels.select(7);
        panels.toggle_settings();
        assert_eq!(panels.rail(), Some(Rail::Settings));
        assert_eq!(panels.selected(), Some(7));
        assert!(!panels.showing_album(7));
        panels.toggle_settings();
        assert_eq!(panels.rail(), Some(Rail::Album));

        // And the ✕ peels it the same way it peels the queue.
        panels.toggle_settings();
        panels.close();
        assert_eq!(panels.rail(), Some(Rail::Album));
    }

    /// The queue and the settings share the one slot: asking for either puts
    /// it up over the other, and neither can be open twice or both at once.
    #[test]
    fn the_queue_and_the_settings_share_the_slot() {
        let mut panels = Panels::new();
        panels.toggle_queue();
        panels.toggle_settings();
        assert_eq!(panels.rail(), Some(Rail::Settings));
        panels.toggle_queue();
        assert_eq!(panels.rail(), Some(Rail::Queue));
        // Closing the newer one does not reveal the older one — there is one
        // slot, and it was overwritten, not stacked.
        panels.close();
        assert_eq!(panels.rail(), None);
    }

    /// Hiding restores the settings, not the queue: the un-hide default is
    /// only for a rail that had nothing in it.
    #[test]
    fn hiding_restores_the_settings_rather_than_defaulting_to_the_queue() {
        let mut panels = Panels::new();
        panels.toggle_settings();
        panels.toggle_hidden();
        assert_eq!(panels.rail(), None);
        panels.toggle_hidden();
        assert_eq!(panels.rail(), Some(Rail::Settings));
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
            Settings,
            Close,
            Hide,
        }
        let steps = [
            Step::Select(1),
            Step::Select(2),
            Step::Queue,
            Step::Settings,
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
                                Step::Settings => panels.toggle_settings(),
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
