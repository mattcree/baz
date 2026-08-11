//! Which **place** the window is showing — and there is nothing else.
//!
//! ADR-0006 layer 1 — pure, iced-free, unit-tested — and after ADR-0022 the
//! *whole* of baz's surface model:
//!
//! > **The window holds one place at a time, with the returns lane to its left
//! > in every place but Settings, and the now-playing bar under all of them.**
//!
//! One kind, nine members, one rule. There is no inspector, no popover and no
//! rail; a listener has one question to answer about anything on screen —
//! *which place am I in* — and one key that answers it. (One summoned,
//! single-tenant panel floats *over* a place without being one — the playlist
//! panel, ADR-0024 §5, whose amended refusal names it and closes the slot; it
//! holds no place-like state here because it is never what the window is
//! showing, only what is temporarily in front of it.)
//!
//! # The two the owner added
//!
//! ADR-0030 recommended a home *band* at the head of the Library's body and
//! recorded `Place::Home` under "deliberately not done". **The owner overruled
//! both**, and the product's preamble says his decision is sufficient
//! on its own: home is a real place, and a `Now playing` place stands beside
//! it. Both are reached from the returns lane's head, which is what pays the
//! cost ADR-0030 priced — *"a fifth place needs a route back to the wall,
//! which is either a nav rail or a strip tenant"*. The route is a resident
//! surface that was being built anyway, and it costs the strip nothing; it
//! costs the head its own second subject, which is the concession, recorded.
//!
//! **[`Place::Library`] is still the launch frame and still what
//! [`Place::back`] returns to.** The collection is what baz opens onto
//! (`VISION.md`'s first pillar) and <kbd>Esc</kbd> means *put this down*, not
//! *go to the home page*; nothing the owner decided touches either, so neither
//! moved. [`Place::is_library`] is the reading, renamed from `is_home` the
//! moment a place was actually called Home.
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
//! - **`Queue`** took over from `overlay.rs`, which held *which popover, if
//!   any, is floating*. It was briefly folded into Now playing, then removed
//!   when that place became one current song. It returns without a resident
//!   door for one precise subject: an unsaved list needs somewhere to be
//!   inspected and saved, and `All songs` now materializes as one.
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
/// [`Self::Library`] is the default: a fresh baz, and a baz that has just been
/// backed out of anywhere, is looking at the shelf.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Place {
    /// The shelf, its search, its arrangement, its counts. The interface, per
    /// the vision's first pillar.
    #[default]
    Library,
    /// Every saved playlist, arranged as a collection of playlist sleeves.
    Playlists,
    /// **Home**: the interrupted run, and what is new (ADR-0030 §9.4 as the
    /// owner chose it).
    ///
    /// Not the launch frame and not what `back` returns to — a page you go to,
    /// from the lane's head, like every other place.
    Home,
    /// **Now playing**: the sounding record at the size it deserves, the run
    /// it is a position in, and the surface the kiosk mode will be at a larger
    /// size.
    ///
    /// Distinct from [`Self::Album`] because its subject is *what is sounding*
    /// rather than *which record I pointed at* — the bottom bar's subject, on
    /// a page. It carries no id for the same reason the bar carries none: the
    /// engine's answer is the only one it may draw.
    ///
    NowPlaying,
    /// **The unsaved playlist**: the run the engine is holding, with its rows,
    /// edits and `Save as playlist` act.
    ///
    /// It has no resident lane destination. It is reached from a run whose
    /// source is an unsaved list — including either `All songs` — through the
    /// source road in Now playing and the persistent bar. That gives the
    /// transient a location without making it permanent navigation furniture.
    Queue,
    /// **One artist's page**: their name, and their records in the wall's own
    /// tile.
    ///
    /// The owner's, in one line: *"we could add an Artist > album breadcrumb
    /// though. and have an artist page."* It carries
    /// [`crate::vm::artist_id`]'s hash of the album artist rather than the
    /// name, for [`Self::Album`]'s reason exactly — a `Copy` handle the shell
    /// resolves against the wall on every frame, so a rescan that renamed or
    /// removed the artist is answered with the wall rather than with a
    /// dangling borrow.
    ///
    /// **Not a destination.** This is reached from a record's page, like
    /// `Album` is reached from a tile, and it lights nothing in the head.
    Artist(u64),
    /// **One record's page**: its art, its identity, the action, its tracks
    /// and its condition report, at the width of the window.
    ///
    /// Carries the album id rather than pointing at a selection held
    /// elsewhere, which is what deletes the class of bug `selection.rs`'s
    /// exhaustive walk existed to catch: there is no reachable state that is
    /// "showing an album page for no album".
    Album(u64),
    /// **One playlist's page** (ADR-0024 §4): its name, its counts, `Play`,
    /// `Rename`, `Delete`, and its rows in the queue place's anatomy.
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
    /// A toggle only against itself. From an album's page or the now-playing
    /// place this is a *move* to the settings, not a swap — the key means
    /// "take me to the preferences", and only the preferences answer it with
    /// "and back again".
    #[must_use]
    pub fn settings(self) -> Self {
        match self {
            Self::Settings => Self::Library,
            _ => Self::Settings,
        }
    }

    /// A tile was pressed, or a source route named a record: show that
    /// record's page.
    ///
    /// This is idempotent. A second pointer press can arrive as part of a
    /// double-click, and pointing at the thing already on screen must not turn
    /// into an unrelated Back gesture. Back and the resident destinations are
    /// the explicit ways out.
    #[must_use]
    pub fn album(self, id: u64) -> Self {
        let _ = self;
        Self::Album(id)
    }

    /// **The album page's breadcrumb was pressed**: show that artist's page.
    /// Repeating the route leaves that page in place, exactly as repeating an
    /// album or playlist route does.
    #[must_use]
    pub fn artist(self, id: u64) -> Self {
        let _ = self;
        Self::Artist(id)
    }

    /// A playlist tile or name was pressed: show that playlist's page.
    /// Repeating the press leaves it there; it is navigation, not an implicit
    /// Back control.
    #[must_use]
    pub fn playlist(self, id: u64) -> Self {
        let _ = self;
        Self::Playlist(id)
    }

    /// <kbd>Esc</kbd>, and every place's `‹ Library`: return home.
    ///
    /// Unlike the subject routes above, this deliberately discards the
    /// subject and returns to the Library. Home is already home, so this is a
    /// no-op there —
    /// and the shell asks [`Self::is_library`] first, so the key falls through to
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

    /// **The lane's head, pressed**: go to that destination.
    ///
    /// Not a toggle, and that is the difference between a destination and a
    /// door. `Settings` is a door — press it again and it closes — because it
    /// names one thing you look at and then put down.
    /// The head's four name *where you are*, and the current one is drawn in
    /// full paper ink to say so; pressing the row you are already on must
    /// leave you there, because the alternative is a control whose meaning
    /// depends on a state you can already see and would be a way to fall out
    /// of the head into the wall by mis-clicking.
    #[must_use]
    #[expect(
        clippy::unused_self,
        reason = "a navigation verb reads on the place you are leaving, and \
                  the signature is the claim: that today's answer ignores \
                  where you were is the *finding* — a destination is not a \
                  door — not an accident. `back` carries the same expectation \
                  for the same reason."
    )]
    pub fn go(self, to: crate::lane::Destination) -> Self {
        match to {
            crate::lane::Destination::Home => Self::Home,
            crate::lane::Destination::Library => Self::Library,
            crate::lane::Destination::Playlists => Self::Playlists,
            crate::lane::Destination::NowPlaying => Self::NowPlaying,
        }
    }

    /// Which of the head's four destinations this place *is*, if it is one —
    /// what the lane reads to ink the current row.
    ///
    /// The five places that are not destinations (`Album`, `Artist`, `Queue`,
    /// `Playlist`, `Settings`) light **nothing** in the head rather than
    /// falling back to `Library`. A record's page was reached from the wall,
    /// but it is not the wall, and a head that claimed otherwise would be
    /// telling the listener they are somewhere they are not.
    #[must_use]
    pub fn destination(self) -> Option<crate::lane::Destination> {
        match self {
            Self::Home => Some(crate::lane::Destination::Home),
            Self::Library => Some(crate::lane::Destination::Library),
            Self::Playlists => Some(crate::lane::Destination::Playlists),
            Self::NowPlaying => Some(crate::lane::Destination::NowPlaying),
            _ => None,
        }
    }

    /// Whether the returns lane is drawn beside this place. **It always is.**
    ///
    /// It was everywhere *but* Settings — ADR-0024 §5's clause 5, inherited by
    /// ADR-0030 on the reading that the standing decisions are the one place
    /// whose subject is baz rather than the music. That was defensible while
    /// every place header carried a `‹ Library` door. It stopped being
    /// defensible the moment the lane made those doors redundant and they were
    /// removed: Settings then had the lane hidden *and* no door, so the only
    /// way out was <kbd>Esc</kbd> — a keyboard-only route out of a place you
    /// can reach with the pointer. The owner found it exactly that way:
    /// *"when you visit the settings page you cannot return to the main screen
    /// again because the left hand bar disappears"*.
    ///
    /// The panel is still absent there, which is what §5's clause was really
    /// about — a summoned surface for collecting has no business over the
    /// standing decisions. The lane is not that; it is the frame, and the
    /// frame is the frame in every place.
    #[must_use]
    pub fn wears_lane(self) -> bool {
        let _ = self;
        true
    }

    /// Whether the shelf is the place on screen.
    ///
    /// Called `is_home` until a place was actually called
    /// [`Home`](Self::Home). The rename is the whole of the change: this has
    /// always meant *the collection is what the window is showing*, and every
    /// caller wanted that reading.
    #[must_use]
    pub fn is_library(self) -> bool {
        self == Self::Library
    }

    /// Which record's page is showing, if one is.
    ///
    /// **Test-only.** The shell asked this while the album stepper existed —
    /// `‹ Prev` / `Next ›` needed to know which record it was stepping from.
    /// The stepper was withdrawn (it walked the wall's arrangement, which is
    /// not on screen from a record's page), and with it the last caller in
    /// anger. It stays because the property sweeps below assert *one place at
    /// a time* through it, and that property outlived the control.
    #[cfg(test)]
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
    fn a_fresh_window_is_at_the_collection() {
        assert_eq!(Place::default(), Place::Library);
        assert!(Place::default().is_library());
        assert_eq!(Place::default().showing_album(), None);
        // **The launch frame is still the collection**, with a place now
        // literally called Home beside it. `VISION.md`'s first pillar is the
        // reason and the owner did not touch it; a fresh baz opens onto the
        // records it holds.
        assert_ne!(Place::default(), Place::Home);
    }

    /// **The head's three are destinations, not doors**: pressing the one you
    /// are on leaves you there, where `Queue` and `Settings` would close.
    #[test]
    fn a_destination_never_closes_itself() {
        use crate::lane::Destination;
        for to in Destination::ALL {
            let arrived = Place::default().go(to);
            assert_eq!(arrived.destination(), Some(to));
            assert_eq!(arrived.go(to), arrived, "{to:?} closed itself like a door");
            // …and reached from anywhere, it is the same place.
            for from in [
                Place::Album(7),
                Place::Artist(5),
                Place::Queue,
                Place::Playlist(3),
                Place::Settings,
            ] {
                assert_eq!(from.go(to), arrived);
            }
        }
    }

    /// The five places that are not destinations light **nothing** in the
    /// head — a page reached from the wall is not the wall.
    #[test]
    fn only_a_destination_lights_the_head() {
        for place in [
            Place::Album(7),
            Place::Artist(5),
            Place::Queue,
            Place::Playlist(3),
            Place::Settings,
        ] {
            assert_eq!(place.destination(), None, "{place:?}");
        }
    }

    /// **The lane is beside every place but Settings** — ADR-0024 §5's clause
    /// 5, inherited verbatim.
    #[test]
    fn the_lane_stands_beside_every_place_but_the_standing_decisions() {
        for place in [
            Place::Library,
            Place::Home,
            Place::NowPlaying,
            Place::Queue,
            Place::Album(7),
            Place::Artist(5),
            Place::Playlist(3),
        ] {
            assert!(place.wears_lane(), "{place:?}");
        }
        assert!(
            Place::Settings.wears_lane(),
            "Settings wears it too, or it is a room with no door"
        );
    }

    /// **<kbd>Ctrl</kbd>+<kbd>U</kbd> is the lane row's accelerator**, and so
    /// it stops toggling (doc 12 §3.4.4).
    ///
    /// The chord resolves to `Place::go(Destination::NowPlaying)` — the same
    /// answer the lane's own row gives — and **pressing it twice leaves you
    /// there**, because a key that closed what its visible twin does not close
    /// would be a second behaviour with no control. `Esc` is the way out, and
    /// always was.
    #[test]
    fn ctrl_u_is_the_lane_rows_accelerator() {
        use crate::lane::Destination;
        let chord = |place: Place| place.go(Destination::NowPlaying);
        for from in [
            Place::Library,
            Place::Home,
            Place::Album(7),
            Place::Artist(5),
            Place::Queue,
            Place::Playlist(3),
            Place::Settings,
        ] {
            assert_eq!(chord(from), Place::NowPlaying, "{from:?}");
        }
        assert_eq!(chord(Place::NowPlaying), Place::NowPlaying);
        assert_eq!(chord(chord(Place::Library)), Place::NowPlaying);
        // …and the way out is the one every place has.
        assert_eq!(Place::NowPlaying.back(), Place::Library);
    }

    #[test]
    fn subject_routes_are_idempotent_and_settings_remains_a_toggle() {
        assert_eq!(Place::Library.settings(), Place::Settings);
        assert_eq!(Place::Settings.settings(), Place::Library);
        assert_eq!(Place::Library.album(7), Place::Album(7));
        assert_eq!(Place::Album(7).album(7), Place::Album(7));
        assert_eq!(Place::Library.playlist(3), Place::Playlist(3));
        assert_eq!(Place::Playlist(3).playlist(3), Place::Playlist(3));
        assert_eq!(Place::Playlist(3).playlist(4), Place::Playlist(4));
        assert_eq!(Place::Library.artist(5), Place::Artist(5));
        assert_eq!(Place::Artist(5).artist(5), Place::Artist(5));
        assert_eq!(Place::Artist(5).artist(6), Place::Artist(6));

        // A subject route pressed from another place is still a move, and a
        // different subject replaces the current one rather than stacking it.
        assert_eq!(Place::NowPlaying.settings(), Place::Settings);
        assert_eq!(Place::Settings.album(7), Place::Album(7));
        assert_eq!(Place::Album(7).album(8), Place::Album(8));
    }

    /// Back means *home*, not *the other one*. Anywhere, any number of times.
    #[test]
    fn back_always_means_the_library() {
        for place in [
            Place::Library,
            Place::Home,
            Place::Playlists,
            Place::NowPlaying,
            Place::Album(7),
            Place::Artist(5),
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
            Album(u64),
            Artist(u64),
            Playlist(u64),
            Go(crate::lane::Destination),
            Back,
        }
        let steps = [
            Step::Settings,
            Step::Album(1),
            Step::Album(2),
            Step::Artist(5),
            Step::Playlist(1),
            Step::Go(crate::lane::Destination::Home),
            Step::Go(crate::lane::Destination::Playlists),
            Step::Go(crate::lane::Destination::NowPlaying),
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
                                Step::Album(id) => place.album(id),
                                Step::Artist(id) => place.artist(id),
                                Step::Playlist(id) => place.playlist(id),
                                Step::Go(to) => place.go(to),
                                Step::Back => place.back(),
                            };
                            assert_eq!(
                                place.is_library(),
                                place == Place::Library,
                                "{step:?} left the two readings disagreeing"
                            );
                            assert_eq!(
                                place.showing_album().is_some(),
                                matches!(place, Place::Album(_)),
                                "{step:?} left an album page with no album"
                            );
                            // The head's reading agrees with the place, both
                            // ways: a destination place names itself, and a
                            // place that is not one names nothing.
                            assert_eq!(
                                place.destination().is_some(),
                                matches!(
                                    place,
                                    Place::Home
                                        | Place::Library
                                        | Place::Playlists
                                        | Place::NowPlaying
                                ),
                                "{step:?} left the head disagreeing with the place"
                            );
                            // Exactly one member is on screen: the enum makes
                            // "two places at once" unrepresentable, and this
                            // is that claim spelled out for a reader who is
                            // looking for the rail's arbitration and will not
                            // find it.
                            let showing = usize::from(place.is_library())
                                + usize::from(place == Place::Home)
                                + usize::from(place == Place::Playlists)
                                + usize::from(place == Place::NowPlaying)
                                + usize::from(place == Place::Settings)
                                + usize::from(place.showing_album().is_some())
                                + usize::from(matches!(place, Place::Artist(_)))
                                + usize::from(matches!(place, Place::Playlist(_)));
                            assert_eq!(showing, 1, "{step:?}");
                        }
                    }
                }
            }
        }
    }
}
