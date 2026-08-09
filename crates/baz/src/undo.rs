//! **The edit history a list surface keeps**: a bounded stack of whole-list
//! snapshots (doc 11 §5 P2 — forgiveness: reversibility first).
//!
//! ADR-0006 layer 1 — pure, iced-free, unit-tested — and deliberately tiny,
//! because the architecture already paid for undo: every queue edit is a
//! whole-list [`UpdateQueue`](baz_core::protocol::Command::UpdateQueue)
//! computed by pure functions from the previous list, and every playlist
//! edit is an atomic whole-file rewrite. The previous state was a value the
//! code held in its hand and dropped; this module is the hand that keeps it.
//!
//! One history per surface (the Queue place's run, the open playlist page's
//! file), never a global stack: undo reads *the place's own edit history*
//! (doc 07 L8 — subject is what a control reads), and its visible control is
//! the transient `Undo` word in that place, with <kbd>Ctrl</kbd>+<kbd>Z</kbd>
//! as the accelerator the visible twin makes legal (doc 09 §5.2's exact
//! construction).
//!
//! What is *not* here, by P2's own scope: no redo, no undo of playback acts
//! (play, seek and volume are not destructive — the era never undid Play
//! either), and nothing that could sound. Restoring a snapshot is the
//! caller's business and every caller restores a **list**, never a playback
//! position.

/// How many snapshots a surface keeps.
///
/// Bounded because a snapshot is a whole list and a session's edits are
/// unbounded; eight is deeper than any mis-click and shallower than a
/// memory. When the stack is full the *oldest* snapshot falls off — the
/// recent mistakes are the ones a hand reaches back for.
pub(crate) const DEPTH: usize = 8;

/// A bounded last-in-first-out history of whole-list snapshots.
#[derive(Debug, Clone, Default)]
pub(crate) struct History<T> {
    snapshots: Vec<T>,
}

impl<T> History<T> {
    /// A fresh, empty history.
    #[must_use]
    pub(crate) fn new() -> Self {
        Self {
            snapshots: Vec::new(),
        }
    }

    /// Record the state a destructive edit is about to replace. Oldest out
    /// first once [`DEPTH`] is reached.
    pub(crate) fn push(&mut self, snapshot: T) {
        if self.snapshots.len() == DEPTH {
            self.snapshots.remove(0);
        }
        self.snapshots.push(snapshot);
    }

    /// Take back the most recent snapshot — the list as it stood before the
    /// last recorded edit — or `None` when there is nothing to restore.
    pub(crate) fn pop(&mut self) -> Option<T> {
        self.snapshots.pop()
    }

    /// Whether there is anything to undo — what decides if the `Undo` word
    /// is drawn.
    #[must_use]
    pub(crate) fn can_undo(&self) -> bool {
        !self.snapshots.is_empty()
    }

    /// Drop the whole history: the surface it belonged to was left, or the
    /// run it described ended.
    pub(crate) fn clear(&mut self) {
        self.snapshots.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_history_hands_back_what_was_pushed_newest_first() {
        let mut history = History::new();
        assert!(!history.can_undo());
        history.push(vec!["a"]);
        history.push(vec!["a", "b"]);
        assert!(history.can_undo());
        assert_eq!(history.pop(), Some(vec!["a", "b"]));
        assert_eq!(history.pop(), Some(vec!["a"]));
        assert_eq!(history.pop(), None);
        assert!(!history.can_undo());
    }

    /// The bound drops the *oldest* snapshot: after `DEPTH + 3` edits the
    /// hand can reach back exactly `DEPTH` steps, and the steps it reaches
    /// are the most recent ones in order.
    #[test]
    fn the_bound_forgets_the_oldest_snapshot_first() {
        let mut history = History::new();
        for n in 0..(DEPTH + 3) {
            history.push(n);
        }
        for expected in (3..(DEPTH + 3)).rev() {
            assert_eq!(history.pop(), Some(expected));
        }
        assert_eq!(history.pop(), None);
    }

    #[test]
    fn clearing_forgets_everything_at_once() {
        let mut history = History::new();
        history.push(1);
        history.push(2);
        history.clear();
        assert!(!history.can_undo());
        assert_eq!(history.pop(), None);
    }
}
