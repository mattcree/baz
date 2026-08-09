//! Which **place** the window is showing — and there is nothing else.
//!
//! ADR-0006 layer 1 — pure, iced-free, unit-tested — and after ADR-0022 the
//! *whole* of baz's surface model:
//!
//! > **The window holds one place at a time, and the now-playing bar is in
//! > every one of them.**
//!
//! One kind, five members, one rule. There is no inspector, no popover and no
//! rail; a listener has one question to answer about anything on screen —
//! *which place am I in* — and one key that answers it. (One summoned,
//! single-tenant panel floats *over* a place without being one — the playlist
//! panel, ADR-0024 §5, whose amended refusal names it and closes the slot; it
//! holds no place-like state here because it is never what the window is
//! showing, only what is temporarily in front of it.)
//!
//! # What this replaces
//!
//! ADR-0016 named four kinds — place, inspector, popover, bar — and reduced
//! three rail tenants to one column plus a float. The owner rejected the
//! survivors: *"I really hate the way queue and selected albums appear… I hate
//! the sidebar."* ADR-0022 records the argument; what it costs this module is
//! two new members and one deleted sibling apiece:
//!
//! - **`Album(id)`** takes over from `selection.rs`, which held *which album
//!   the inspector is showing, and whether it is showing*. Both facts collapse
//!   into "is the place an `Album`, and which one" — one field where there were
//!   two, and a `hidden` flag that no longer has anything to hide.
//! - **`Queue`** takes over from `overlay.rs`, which held *which popover, if
//!   any, is floating*. There is no float, so there is no layer to peel before
//!   the place underneath it.
//!
//! # Why an enum and not a stack
//!
//! Because places still **replace** each other; two are never on screen
//! together, and there is no history to walk. [`Place::back`] is a total
//! function with no argument and no `Option`, and <kbd>Esc</kbd>'s rule is one
//! line rather than one line per layer.
//!
//! The one thing a stack would buy — *back to the album I was looking at
//! before this one* — is deliberately not offered. Every route into an
//! `Album` place starts on the wall (a tile) or from the bar (what is
//! sounding), and the wall is what `back` returns to, with its scroll, its
//! query and its arrangement untouched. A history that could land you
//! somewhere you did not navigate from is the thing ADR-0016's rail was, in a
//! different shape.
//!
//! # The cost this model has that the last one did not
//!
//! Comparing two records is a **round trip**: wall → album → wall → album,
//! where the inspector made it two clicks and no navigation. ADR-0022 states
//! it as the price rather than hiding it, and the mitigation lives on the
//! shelf rather than here — the wall keeps its scroll and marks the record you
//! last opened, so the return leg is *return* and not *re-find*.

/// The place the window is showing.
///
/// [`Self::Library`] is home and the default: a fresh baz, and a baz that has
/// just been backed out of anywhere, is looking at the shelf.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Place {
    /// The shelf, its search, its arrangement, its counts. The interface, per
    /// the vision's first pillar.
    #[default]
    Library,
    /// **One record's page**: its art, its identity, the action, its tracks
    /// and its condition report, at the width of the window.
    ///
    /// Carries the album id rather than pointing at a selection held
    /// elsewhere, which is what deletes the class of bug `selection.rs`'s
    /// exhaustive walk existed to catch: there is no reachable state that is
    /// "showing an album page for no album".
    Album(u64),
    /// **The queue**: what the engine is holding and where it is in it, as a
    /// place of its own rather than a card floating over the wall.
    Queue,
    /// **One playlist's page** (ADR-0024 §4): its name, its counts, `Play`,
    /// `Queue`, `Rename`, `Delete`, and its rows in the queue place's anatomy.
    ///
    /// Carries [`crate::playlists::playlist_id`]'s hash of the playlist's
    /// *name* — the filename is the name (ADR-0024 §2), so the name is the
    /// identity, and the id is to the name what [`Self::Album`]'s id is to the
    /// (artist, album) pair: a `Copy` handle the shell resolves against the
    /// playlists folder on every frame. A playlist deleted or renamed under
    /// this place stops resolving, and the shell answers with the wall — the
    /// same posture as a record vanishing under a rescan while its page is
    /// open.
    Playlist(u64),
    /// Everything that is a standing decision: today ReplayGain (ADR-0013),
    /// and the shape every setting after it takes.
    Settings,
}

impl Place {
    /// <kbd>Ctrl</kbd>+<kbd>,</kbd>, and the top bar's `Settings` control: go
    /// to the settings, or come back from them.
    ///
    /// A toggle only against itself. From an album's page or the queue this is
    /// a *move* to the settings, not a swap — the key means "take me to the
    /// preferences", and only the preferences answer it with "and back again".
    #[must_use]
    pub fn settings(self) -> Self {
        match self {
            Self::Settings => Self::Library,
            _ => Self::Settings,
        }
    }

    /// <kbd>Q</kbd>, and the bar's labelled `Queue` control: go to the queue,
    /// or come back from it.
    ///
    /// The same shape as [`Self::settings`], and for the same reason: a door
    /// that says *Queue* closes itself when you press it again, and does not
    /// close anything else.
    #[must_use]
    pub fn queue(self) -> Self {
        match self {
            Self::Queue => Self::Library,
            _ => Self::Queue,
        }
    }

    /// A tile was pressed, or the bar's now-playing block was: show that
    /// record's page — or come back from it, when it is the page already
    /// showing.
    ///
    /// The toggle-off arm is what makes a tile press reversible with the same
    /// press, which is the behaviour the inspector had and the one gesture of
    /// it worth keeping.
    #[must_use]
    pub fn album(self, id: u64) -> Self {
        if self == Self::Album(id) {
            Self::Library
        } else {
            Self::Album(id)
        }
    }

    /// A playlist's name was pressed in the panel: show that playlist's page —
    /// or come back from it, when it is the page already showing.
    ///
    /// [`Self::album`]'s shape exactly, and for its reason: pointing at the
    /// thing you are already reading puts it down, and a different playlist
    /// swaps the page rather than stacking one.
    #[must_use]
    pub fn playlist(self, id: u64) -> Self {
        if self == Self::Playlist(id) {
            Self::Library
        } else {
            Self::Playlist(id)
        }
    }

    /// <kbd>Esc</kbd>, and every place's `‹ Library`: return home.
    ///
    /// Distinct from the three toggles above because a *back* that toggled
    /// would send you somewhere from the Library, which is not what backing
    /// out of anywhere means. Home is already home, so this is a no-op there —
    /// and the shell asks [`Self::is_home`] first, so the key falls through to
    /// the layers underneath rather than being silently eaten by a place that
    /// had nothing to leave.
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

    /// Whether the shelf is the place on screen.
    #[must_use]
    pub fn is_home(self) -> bool {
        self == Self::Library
    }

    /// Which record's page is showing, if one is — the shell's one question
    /// when it needs an album, and what replaces `Selection::inspecting`.
    #[must_use]
    pub fn showing_album(self) -> Option<u64> {
        match self {
            Self::Album(id) => Some(id),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_window_is_at_home() {
        assert_eq!(Place::default(), Place::Library);
        assert!(Place::default().is_home());
        assert_eq!(Place::default().showing_album(), None);
    }

    #[test]
    fn each_door_closes_itself_and_nothing_else() {
        assert_eq!(Place::Library.settings(), Place::Settings);
        assert_eq!(Place::Settings.settings(), Place::Library);
        assert_eq!(Place::Library.queue(), Place::Queue);
        assert_eq!(Place::Queue.queue(), Place::Library);
        assert_eq!(Place::Library.album(7), Place::Album(7));
        assert_eq!(Place::Album(7).album(7), Place::Library);
        assert_eq!(Place::Library.playlist(3), Place::Playlist(3));
        assert_eq!(Place::Playlist(3).playlist(3), Place::Library);
        assert_eq!(Place::Playlist(3).playlist(4), Place::Playlist(4));

        // …and a door pressed from *another* place is a move, not a swap back
        // home. The key says where to go; only the place you are in says
        // "and back".
        assert_eq!(Place::Queue.settings(), Place::Settings);
        assert_eq!(Place::Album(7).queue(), Place::Queue);
        assert_eq!(Place::Settings.album(7), Place::Album(7));
        assert_eq!(Place::Album(7).album(8), Place::Album(8));
    }

    /// Back means *home*, not *the other one*. Anywhere, any number of times.
    #[test]
    fn back_always_means_the_library() {
        for place in [
            Place::Library,
            Place::Album(7),
            Place::Queue,
            Place::Playlist(3),
            Place::Settings,
        ] {
            assert_eq!(place.back(), Place::Library);
            assert_eq!(place.back().back(), Place::Library);
        }
    }

    /// The property `selection.rs` walked exhaustively, restated for a model
    /// that makes it structural: **no reachable state shows an album page
    /// without an album**, and `showing_album` never disagrees with the value
    /// it reads. It is now a property of one field rather than of two, which
    /// is the point — but it is still walked, because "obviously true" is what
    /// the rail's rule looked like from the inside as well.
    #[test]
    fn no_reachable_state_is_two_places_at_once() {
        #[derive(Debug, Clone, Copy)]
        enum Step {
            Settings,
            Queue,
            Album(u64),
            Playlist(u64),
            Back,
        }
        let steps = [
            Step::Settings,
            Step::Queue,
            Step::Album(1),
            Step::Album(2),
            Step::Playlist(1),
            Step::Back,
        ];
        for a in steps {
            for b in steps {
                for c in steps {
                    for d in steps {
                        let mut place = Place::default();
                        for step in [a, b, c, d] {
                            place = match step {
                                Step::Settings => place.settings(),
                                Step::Queue => place.queue(),
                                Step::Album(id) => place.album(id),
                                Step::Playlist(id) => place.playlist(id),
                                Step::Back => place.back(),
                            };
                            assert_eq!(
                                place.is_home(),
                                place == Place::Library,
                                "{step:?} left the two readings disagreeing"
                            );
                            assert_eq!(
                                place.showing_album().is_some(),
                                matches!(place, Place::Album(_)),
                                "{step:?} left an album page with no album"
                            );
                            // Exactly one member is on screen: the enum makes
                            // "two places at once" unrepresentable, and this
                            // is that claim spelled out for a reader who is
                            // looking for the rail's arbitration and will not
                            // find it.
                            let showing = usize::from(place.is_home())
                                + usize::from(place == Place::Settings)
                                + usize::from(place == Place::Queue)
                                + usize::from(place.showing_album().is_some())
                                + usize::from(matches!(place, Place::Playlist(_)));
                            assert_eq!(showing, 1, "{step:?}");
                        }
                    }
                }
            }
        }
    }
}
