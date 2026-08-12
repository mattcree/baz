//! Product-wide selection and activation for playable content.
//!
//! One press selects. A second press on the same object inside the desktop
//! double-click interval activates it. Views only publish [`Content`]; this
//! state machine owns timing for every tile and row, so no surface can invent
//! a different gesture.

use std::time::{Duration, Instant};

/// iced 0.13 does not expose the platform double-click setting. This matches
/// the app bar's existing desktop-like interval; iced 0.14 can replace both
/// with the toolkit/platform click count.
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

/// The single selection/activation state machine shared by every content
/// surface. Selection is session state; the click clock is deliberately not.
#[derive(Debug, Default)]
pub(crate) struct State {
    selected: Option<Content>,
    last_press: Option<(Content, Instant)>,
}

impl State {
    #[must_use]
    pub(crate) fn selected(&self) -> Option<Content> {
        self.selected
    }

    #[must_use]
    pub(crate) fn is(&self, content: Content) -> bool {
        self.selected == Some(content)
    }

    /// Select `content`, without treating a later pointer press as the second
    /// half of a double-click. Explicit Open routes use this when navigation
    /// should leave the object marked on return.
    pub(crate) fn select(&mut self, content: Content) {
        self.selected = Some(content);
        self.last_press = None;
    }

    /// Leave no content selected and retire any half-finished double-click.
    pub(crate) fn clear(&mut self) {
        self.selected = None;
        self.last_press = None;
    }

    pub(crate) fn press(&mut self, content: Content, now: Instant) -> Press {
        let doubled = self.last_press.is_some_and(|(prior, at)| {
            prior == content && now.saturating_duration_since(at) <= DOUBLE_CLICK
        });
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
