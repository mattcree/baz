//! Keyboard state and geometry for the app-wide search dropover.
//!
//! The view owns no decisions: this module defines the two track actions and
//! the clamped movement used by both the update loop and its tests.

use iced::keyboard::{Key, Modifiers, key};

/// The two actions a selected track exposes, in their visual/keyboard order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Action {
    Play,
    Enqueue,
}

/// A bare arrow key. The shell gives it to the open result chooser first and
/// retains the existing volume/seek meaning everywhere else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Direction {
    Up,
    Down,
    Left,
    Right,
}

/// A bare arrow while the search chooser stands.
///
/// This is deliberately resolved before a focused `text_input`'s capture
/// report: Left/Right normally belong to its caret, but in the visible search
/// chooser they are the advertised action axis. Other modifiers remain the
/// field's, so selection shortcuts do not steal word/caret gestures.
#[must_use]
pub(crate) fn chooser_direction(key: &Key, modifiers: Modifiers) -> Option<Direction> {
    if !modifiers.is_empty() {
        return None;
    }
    match key.as_ref() {
        Key::Named(key::Named::ArrowUp) => Some(Direction::Up),
        Key::Named(key::Named::ArrowDown) => Some(Direction::Down),
        Key::Named(key::Named::ArrowLeft) => Some(Direction::Left),
        Key::Named(key::Named::ArrowRight) => Some(Direction::Right),
        _ => None,
    }
}

/// Fixed geometry makes the one long result list cheaply virtualizable and
/// lets keyboard movement reveal its selection without measuring widgets.
pub(crate) const ROW_H: f32 = 40.0;
pub(crate) const SECTION_H: f32 = 32.0;
pub(crate) const OVERSCAN_ROWS: usize = 3;

#[expect(
    clippy::cast_precision_loss,
    reason = "search results are capped at 10,000 rows, far below exact f32 integer range"
)]
fn rows(count: usize) -> f32 {
    count as f32 * ROW_H
}

#[must_use]
pub(crate) fn result_top(index: usize, tracks: usize) -> f32 {
    if index < tracks {
        SECTION_H + rows(index)
    } else {
        SECTION_H + rows(tracks) + SECTION_H + rows(index - tracks)
    }
}

impl Action {
    #[must_use]
    pub(crate) const fn moved(self, delta: i32) -> Self {
        match (self, delta.signum()) {
            (Self::Play, 1) => Self::Enqueue,
            (Self::Enqueue, -1) => Self::Play,
            _ => self,
        }
    }
}

/// Move a result selection without wrapping. With no selection, Down begins
/// at the first answer and Up begins at the last.
#[must_use]
pub(crate) fn moved_index(selected: Option<usize>, len: usize, delta: i32) -> Option<usize> {
    if len == 0 || delta == 0 {
        return selected.filter(|index| *index < len);
    }
    let last = len - 1;
    Some(match selected {
        None if delta < 0 => last,
        None => 0,
        Some(index) if delta < 0 => index.saturating_sub(1),
        Some(index) => index.saturating_add(1).min(last),
    })
}

#[cfg(test)]
mod tests {
    use super::{Action, Direction, chooser_direction, moved_index};
    use iced::keyboard::{Key, Modifiers, key};

    #[test]
    fn the_open_chooser_claims_every_bare_arrow_even_beside_the_caret() {
        assert_eq!(
            chooser_direction(&Key::Named(key::Named::ArrowUp), Modifiers::empty()),
            Some(Direction::Up)
        );
        assert_eq!(
            chooser_direction(&Key::Named(key::Named::ArrowRight), Modifiers::empty()),
            Some(Direction::Right)
        );
        assert_eq!(
            chooser_direction(&Key::Named(key::Named::ArrowRight), Modifiers::SHIFT),
            None
        );
    }

    #[test]
    fn result_movement_starts_at_the_near_end_and_clamps() {
        assert_eq!(moved_index(None, 4, 1), Some(0));
        assert_eq!(moved_index(None, 4, -1), Some(3));
        assert_eq!(moved_index(Some(0), 4, -1), Some(0));
        assert_eq!(moved_index(Some(3), 4, 1), Some(3));
        assert_eq!(moved_index(None, 0, 1), None);
    }

    #[test]
    fn track_actions_are_a_two_stop_axis() {
        assert_eq!(Action::Play.moved(1), Action::Enqueue);
        assert_eq!(Action::Enqueue.moved(-1), Action::Play);
        assert_eq!(Action::Play.moved(-1), Action::Play);
        assert_eq!(Action::Enqueue.moved(1), Action::Enqueue);
    }
}
