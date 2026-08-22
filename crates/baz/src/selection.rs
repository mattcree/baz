//! Product-wide selection and activation for playable content.
//!
//! One press selects. A second press on the same object inside the desktop
//! double-click interval activates it. Views only publish [`Content`]; this
//! state machine owns timing for every tile and row, so no surface can invent
//! a different gesture.
//!
//! # More than one, and why it is the same object
//!
//! ADR-0017 made selection deliberately **one** content item, which is right
//! for activation — a double-press has to mean *this record* — and wrong for
//! building a list by hand, which is what `docs/design/18-feature-parity.md`
//! §4 calls the floor every other list feature stands on. So a selection is a
//! **set** now, and the single-selection case is the set of one:
//!
//! - a plain press replaces the set with what was pressed;
//! - <kbd>Ctrl</kbd> adds or removes one ([`State::toggle`]);
//! - <kbd>Shift</kbd> takes everything from the anchor to what was pressed
//!   ([`State::extend`]).
//!
//! **[`State::is`] answers for the whole set**, which is what kept this from
//! being a sweep through every view: the album page, the queue, a playlist's
//! page and the wall all ask *is this one selected* and get the same answer
//! they always did, for more things.
//!
//! # A set never spans two lists
//!
//! *Everything from here to there* is only a sentence inside one list, and a
//! set holding two rows of one playlist and a tile from the wall has no verb
//! that means anything. So every [`Content`] belongs to a [`Run`], and a press
//! in a different one starts again rather than adding to a set the listener
//! cannot see the ends of.

use std::time::{Duration, Instant};

/// This matches the app bar's desktop-like interval. Baz keeps the shared
/// state machine because selection-then-activation spans custom tiles and rows,
/// rather than mapping directly to a stock widget's double-click message.
pub(crate) const DOUBLE_CLICK: Duration = Duration::from_millis(400);

/// A playable object that can be selected and activated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Content {
    Album(u64),
    Playlist(u64),
    AllSongs,
    ArtistSongs(u64),
    AlbumTrack { album: u64, row: usize },
    SearchTrack { album: u64, row: usize },
    PlaylistTrack { playlist: u64, row: usize },
    QueueTrack(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Press {
    Selected,
    Activated,
}

/// **Which list a piece of content lives in.**
///
/// A selection never spans two of these — see the module note. Tiles are one
/// run rather than several because the wall, Home and the playlists place all
/// draw the same kind of object and every bulk verb means the same thing for
/// all of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Run {
    /// Album tiles, wherever they are drawn.
    Records,
    /// Playlists and the implicit collections — the tiles that stand for a
    /// *list* rather than for a record.
    ///
    /// **These cannot be marked more than one at a time**, and the reason is
    /// [`Run::bulkable`]'s: baz cannot read a saved playlist's tracks without
    /// opening it, so there is no verb a set of them could spend. A selection
    /// a listener can build and then find nothing to do with is worse than one
    /// they cannot build.
    Lists,
    /// One album page's track rows.
    AlbumTracks(u64),
    /// The app bar's search results.
    SearchTracks,
    /// One playlist page's rows.
    PlaylistTracks(u64),
    /// The Queue place's rows.
    Queue,
}

impl Run {
    /// **Whether more than one of these can be selected at once.**
    ///
    /// Everything but [`Run::Lists`], for that variant's own reason: a set
    /// exists to be *spent*, and a set nothing can be done with is a state a
    /// listener can reach and then have to undo.
    pub(crate) const fn bulkable(self) -> bool {
        !matches!(self, Self::Lists)
    }
}

impl Content {
    /// The list this content lives in.
    pub(crate) const fn run(self) -> Run {
        match self {
            Self::Album(_) => Run::Records,
            Self::Playlist(_) | Self::AllSongs | Self::ArtistSongs(_) => Run::Lists,
            Self::AlbumTrack { album, .. } => Run::AlbumTracks(album),
            Self::SearchTrack { .. } => Run::SearchTracks,
            Self::PlaylistTrack { playlist, .. } => Run::PlaylistTracks(playlist),
            Self::QueueTrack(_) => Run::Queue,
        }
    }
}

/// The selection/activation state machine shared by every content surface.
/// Selection is session state; the click clock is deliberately not.
#[derive(Debug, Default)]
pub(crate) struct State {
    /// **Everything selected**, in the order it was added, and never empty
    /// while [`Self::selected`] is set.
    marked: Vec<Content>,
    /// The one an activation acts on: the last thing pressed.
    selected: Option<Content>,
    /// Where a <kbd>Shift</kbd> range starts from — the last thing pressed
    /// *without* Shift. `None` until something has been.
    anchor: Option<Content>,
    last_press: Option<(Content, Instant)>,
}

impl State {
    #[must_use]
    pub(crate) fn selected(&self) -> Option<Content> {
        self.selected
    }

    /// **Is this one selected?** True for every member of the set, which is
    /// why no view had to learn about sets to draw them.
    #[must_use]
    pub(crate) fn is(&self, content: Content) -> bool {
        self.marked.contains(&content)
    }

    /// Everything selected, in the order it was added.
    #[must_use]
    pub(crate) fn marked(&self) -> &[Content] {
        &self.marked
    }

    /// How many are selected. One is the ordinary case and is not a *set* to
    /// anybody looking at it; the surfaces that offer bulk verbs read this to
    /// know whether to appear at all.
    #[must_use]
    pub(crate) fn count(&self) -> usize {
        self.marked.len()
    }

    /// Select `content`, without treating a later pointer press as the second
    /// half of a double-click. Explicit Open routes use this when navigation
    /// should leave the object marked on return.
    pub(crate) fn select(&mut self, content: Content) {
        self.marked = vec![content];
        self.selected = Some(content);
        self.anchor = Some(content);
        self.last_press = None;
    }

    /// Leave no content selected and retire any half-finished double-click.
    pub(crate) fn clear(&mut self) {
        self.marked.clear();
        self.selected = None;
        self.anchor = None;
        self.last_press = None;
    }

    /// **<kbd>Ctrl</kbd>: add this one, or take it back out.**
    ///
    /// A press in another list starts again rather than adding to a set whose
    /// other members are on a page the listener cannot see.
    ///
    /// It never completes a double-click. A modified press is a press about
    /// *membership*, and letting two of them activate something would make
    /// building a set of two adjacent rows start playing one of them.
    pub(crate) fn toggle(&mut self, content: Content) {
        if !content.run().bulkable()
            || self
                .marked
                .first()
                .is_some_and(|first| first.run() != content.run())
        {
            self.select(content);
            return;
        }
        self.last_press = None;
        self.anchor = Some(content);
        if let Some(at) = self.marked.iter().position(|held| *held == content) {
            self.marked.remove(at);
            // **The removed one may have been the selected one.** What is left
            // takes over, so an activation always has a subject while anything
            // is marked at all.
            if self.selected == Some(content) {
                self.selected = self.marked.last().copied();
            }
            if self.marked.is_empty() {
                self.anchor = None;
            }
        } else {
            self.marked.push(content);
            self.selected = Some(content);
        }
    }

    /// **<kbd>Shift</kbd>: everything from the anchor to here.**
    ///
    /// `run` is the list in its drawn order — the view has it, and this
    /// deliberately does not, so a set is a slice of what a listener can see
    /// rather than of an index this module would have to keep in step.
    ///
    /// With no anchor, an anchor outside `run`, or a press outside it, this is
    /// an ordinary selection: a range with only one end is not a range, and
    /// guessing the other one would select things nobody pointed at.
    pub(crate) fn extend(&mut self, content: Content, run: &[Content]) {
        if !content.run().bulkable() {
            self.select(content);
            return;
        }
        let (Some(anchor), Some(to)) = (
            self.anchor
                .and_then(|anchor| run.iter().position(|held| *held == anchor)),
            run.iter().position(|held| *held == content),
        ) else {
            self.select(content);
            return;
        };
        let (from, until) = (anchor.min(to), anchor.max(to));
        self.marked = run[from..=until].to_vec();
        self.selected = Some(content);
        // **The anchor stays put**, so dragging Shift back and forth grows and
        // shrinks one range instead of ratcheting outwards from wherever it
        // last landed.
        self.last_press = None;
    }

    pub(crate) fn press(&mut self, content: Content, now: Instant) -> Press {
        let doubled = self.last_press.is_some_and(|(prior, at)| {
            prior == content && now.saturating_duration_since(at) <= DOUBLE_CLICK
        });
        // **A plain press is a fresh selection of one.** Anything else would
        // make the ordinary click ambiguous — a listener who has marked five
        // rows and clicks a sixth means *that one*, which is what every list
        // that has ever had this gesture does.
        self.marked = vec![content];
        self.anchor = Some(content);
        self.selected = Some(content);
        // Clear after activation: three presses are a double and a single,
        // never two overlapping doubles.
        self.last_press = (!doubled).then_some((content, now));
        if doubled {
            Press::Activated
        } else {
            Press::Selected
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Content, DOUBLE_CLICK, Press, State};
    use std::time::{Duration, Instant};

    #[test]
    fn one_press_selects_and_only_the_same_content_can_complete_the_double() {
        let start = Instant::now();
        let mut state = State::default();
        assert_eq!(state.press(Content::Album(1), start), Press::Selected);
        assert!(state.is(Content::Album(1)));
        assert_eq!(
            state.press(Content::Album(2), start + Duration::from_millis(20)),
            Press::Selected
        );
        assert!(state.is(Content::Album(2)));
    }

    #[test]
    fn the_second_press_activates_and_a_third_begins_again() {
        let start = Instant::now();
        let mut state = State::default();
        assert_eq!(state.press(Content::QueueTrack(4), start), Press::Selected);
        assert_eq!(
            state.press(Content::QueueTrack(4), start + DOUBLE_CLICK),
            Press::Activated
        );
        assert_eq!(
            state.press(Content::QueueTrack(4), start + DOUBLE_CLICK),
            Press::Selected
        );
    }

    #[test]
    fn a_late_second_press_only_reselects() {
        let start = Instant::now();
        let mut state = State::default();
        let content = Content::AlbumTrack { album: 9, row: 3 };
        assert_eq!(state.press(content, start), Press::Selected);
        assert_eq!(
            state.press(content, start + DOUBLE_CLICK + Duration::from_millis(1)),
            Press::Selected
        );
    }

    #[test]
    fn explicit_selection_never_arms_a_double_click() {
        let start = Instant::now();
        let mut state = State::default();
        state.select(Content::Playlist(7));
        assert_eq!(state.selected(), Some(Content::Playlist(7)));
        assert_eq!(state.press(Content::Playlist(7), start), Press::Selected);
    }

    /// **Ctrl adds and takes back**, and the ordinary reading of *is this
    /// selected* answers for every member — which is what let every view stay
    /// as it was while selection became a set.
    #[test]
    fn ctrl_builds_a_set_and_unbuilds_it() {
        let mut state = State::default();
        let rows = |row| Content::QueueTrack(row);
        state.press(rows(0), Instant::now());
        state.toggle(rows(2));
        state.toggle(rows(4));
        assert_eq!(state.count(), 3);
        for row in [0, 2, 4] {
            assert!(state.is(rows(row)), "row {row} is not marked");
        }
        assert!(!state.is(rows(1)));

        state.toggle(rows(2));
        assert_eq!(state.count(), 2);
        assert!(!state.is(rows(2)));
    }

    /// **Taking back the selected one leaves a subject behind.** An activation
    /// always acts on something while anything is marked at all; a set with no
    /// selected member would make Enter do nothing with five rows lit.
    #[test]
    fn removing_the_selected_member_hands_over_rather_than_emptying() {
        let mut state = State::default();
        state.press(Content::QueueTrack(0), Instant::now());
        state.toggle(Content::QueueTrack(1));
        assert_eq!(state.selected(), Some(Content::QueueTrack(1)));
        state.toggle(Content::QueueTrack(1));
        assert_eq!(state.selected(), Some(Content::QueueTrack(0)));
        assert_eq!(state.count(), 1);
        state.toggle(Content::QueueTrack(0));
        assert_eq!(state.selected(), None);
        assert_eq!(state.count(), 0);
    }

    /// **A set nothing can be spent on is not offered.** baz cannot read a
    /// saved playlist's tracks without opening it, so there is no bulk verb a
    /// set of playlist tiles could send — and a selection a listener can build
    /// and then find nothing to do with is worse than one they cannot build.
    #[test]
    fn the_tiles_that_stand_for_lists_are_selected_one_at_a_time() {
        let mut state = State::default();
        state.press(Content::Playlist(1), Instant::now());
        state.toggle(Content::Playlist(2));
        assert_eq!(state.count(), 1, "two playlists were marked at once");
        assert!(state.is(Content::Playlist(2)));

        let run = vec![
            Content::AllSongs,
            Content::Playlist(1),
            Content::Playlist(2),
        ];
        state.extend(Content::AllSongs, &run);
        assert_eq!(state.count(), 1, "a range swept up three lists");

        // Records are not lists, and a set of records has verbs.
        state.press(Content::Album(1), Instant::now());
        state.toggle(Content::Album(2));
        assert_eq!(state.count(), 2);
    }

    /// **A set never spans two lists.** *Everything from here to there* is
    /// only a sentence inside one list, and a set holding two playlist rows
    /// and an album tile has no verb that means anything.
    #[test]
    fn a_press_in_another_list_starts_again() {
        let mut state = State::default();
        state.press(Content::QueueTrack(0), Instant::now());
        state.toggle(Content::QueueTrack(1));
        state.toggle(Content::Album(9));
        assert_eq!(state.count(), 1);
        assert!(state.is(Content::Album(9)));
        assert!(!state.is(Content::QueueTrack(0)));
    }

    /// **Shift takes the range between the anchor and the press**, in either
    /// direction, and the anchor stays where it was — so dragging Shift back
    /// and forth grows and shrinks one range instead of ratcheting outwards.
    #[test]
    fn shift_takes_the_range_and_keeps_its_anchor() {
        let run: Vec<Content> = (0..8).map(Content::QueueTrack).collect();
        let mut state = State::default();
        state.press(Content::QueueTrack(5), Instant::now());

        state.extend(Content::QueueTrack(2), &run);
        assert_eq!(state.count(), 4, "5 down to 2 is four rows");
        for row in 2..=5 {
            assert!(state.is(Content::QueueTrack(row)), "row {row}");
        }

        // Back the other way, from the same anchor.
        state.extend(Content::QueueTrack(7), &run);
        assert_eq!(state.count(), 3, "5 up to 7 is three rows");
        assert!(state.is(Content::QueueTrack(7)) && state.is(Content::QueueTrack(5)));
        assert!(!state.is(Content::QueueTrack(2)), "the range ratcheted");

        // And shrinking back onto the anchor is a set of one.
        state.extend(Content::QueueTrack(5), &run);
        assert_eq!(state.count(), 1);
    }

    /// **A range with only one end is not a range.** No anchor, or a press on
    /// something the run does not hold, is an ordinary selection — guessing
    /// the other end would select things nobody pointed at.
    #[test]
    fn shift_without_an_anchor_is_an_ordinary_press() {
        let run: Vec<Content> = (0..4).map(Content::QueueTrack).collect();
        let mut state = State::default();
        state.extend(Content::QueueTrack(2), &run);
        assert_eq!(state.count(), 1);
        assert!(state.is(Content::QueueTrack(2)));

        // An anchor the run no longer holds — the list changed under it.
        state.press(Content::QueueTrack(9), Instant::now());
        state.extend(Content::QueueTrack(1), &run);
        assert_eq!(state.count(), 1);
        assert!(state.is(Content::QueueTrack(1)));
    }

    /// **A modified press never activates.** Two Ctrl-presses building a set
    /// of two adjacent rows must not start playing one of them, and two
    /// Shift-presses adjusting a range must not either.
    #[test]
    fn a_modified_press_cannot_complete_a_double_click() {
        let run: Vec<Content> = (0..4).map(Content::QueueTrack).collect();
        let now = Instant::now();
        let mut state = State::default();
        state.press(Content::QueueTrack(0), now);
        state.toggle(Content::QueueTrack(1));
        assert_eq!(
            state.press(Content::QueueTrack(1), now + Duration::from_millis(10)),
            Press::Selected,
            "a Ctrl press armed a double-click"
        );

        let mut state = State::default();
        state.press(Content::QueueTrack(0), now);
        state.extend(Content::QueueTrack(2), &run);
        assert_eq!(
            state.press(Content::QueueTrack(2), now + Duration::from_millis(10)),
            Press::Selected,
            "a Shift press armed a double-click"
        );
    }

    /// **A plain press is a fresh selection of one**, whatever was marked
    /// before it. A listener who has marked five rows and clicks a sixth means
    /// *that one*, which is what every list with this gesture does.
    #[test]
    fn a_plain_press_replaces_the_whole_set() {
        let mut state = State::default();
        state.press(Content::QueueTrack(0), Instant::now());
        state.toggle(Content::QueueTrack(1));
        state.toggle(Content::QueueTrack(2));
        state.press(Content::QueueTrack(6), Instant::now());
        assert_eq!(state.count(), 1);
        assert!(state.is(Content::QueueTrack(6)));
    }

    /// **Every content names the list it is in**, and two things in different
    /// lists never share one — which is the whole of what stops a set
    /// spanning two pages.
    #[test]
    fn every_content_belongs_to_exactly_one_run() {
        use super::Run;
        assert_eq!(Content::Album(1).run(), Run::Records);
        assert_eq!(Content::Playlist(1).run(), Run::Lists);
        assert_eq!(Content::AllSongs.run(), Run::Lists);
        assert_eq!(Content::ArtistSongs(1).run(), Run::Lists);
        assert_eq!(
            Content::AlbumTrack { album: 3, row: 0 }.run(),
            Run::AlbumTracks(3)
        );
        assert_ne!(
            Content::AlbumTrack { album: 3, row: 0 }.run(),
            Content::AlbumTrack { album: 4, row: 0 }.run(),
            "two albums' pages are two lists"
        );
        assert_eq!(
            Content::SearchTrack { album: 3, row: 0 }.run(),
            Run::SearchTracks
        );
        assert_eq!(
            Content::PlaylistTrack {
                playlist: 2,
                row: 0
            }
            .run(),
            Run::PlaylistTracks(2)
        );
        assert_eq!(Content::QueueTrack(0).run(), Run::Queue);
    }

    #[test]
    fn clearing_retires_both_the_mark_and_the_click_clock() {
        let start = Instant::now();
        let mut state = State::default();
        assert_eq!(state.press(Content::Album(7), start), Press::Selected);
        state.clear();
        assert_eq!(state.selected(), None);
        assert_eq!(
            state.press(Content::Album(7), start + Duration::from_millis(1)),
            Press::Selected
        );
    }
}
