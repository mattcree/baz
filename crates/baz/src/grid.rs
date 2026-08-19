//! **Where an arrow key lands on the wall**, as arithmetic — no window, no
//! widget tree, no iced.
//!
//! `crate::focus` made the window's frame keyboard-reachable and stopped at
//! the collection, because a grid is not a Tab order: fifty tiles between the
//! app bar and the transport would be a worse product than none. The pattern
//! every grid uses instead is **typed traversal** — Tab moves between regions,
//! and the arrows move inside the one you are in — and this is the *inside*
//! half.
//!
//! # Why the shelves make this more than `index ± columns`
//!
//! The wall is one flat vector of albums cut into shelves, and every shelf
//! starts a new row. So a shelf whose album count is not a multiple of the
//! column count ends in a **partial row**, and the tile visually above
//! `index` is `index - columns` only while both are inside the same shelf.
//! Across a heading it is whatever sits in that column of the shelf above —
//! and that shelf may not have a tile in that column at all, in which case the
//! honest answer is its last one rather than nothing.
//!
//! Left and Right are the exception and are deliberately *not* shelf-aware:
//! they are reading order, and reading order runs off the end of a line onto
//! the start of the next one. A listener pressing Right at the end of `A` is
//! asking for the next record, and the next record is the first of `B`.
//!
//! # Why it is pure
//!
//! Same bargain as `crate::queue_edit` and `crate::lane`: the interesting
//! cases here are the boundaries — a partial last row, a shelf of one, the
//! first and last tiles in the collection, a column that does not exist in the
//! shelf you are moving into — and every one of them is a table row in the
//! tests below rather than a thing to be found by pressing an arrow key at a
//! window.

use crate::search::Direction;

/// **The tile an arrow moves to**, or `None` when there is nowhere to go.
///
/// `ends` is one-past-the-last index of each shelf, in order — the shape
/// `crate::app::GroupVm` already holds, because the shelves are contiguous
/// and carrying both ends of each would be two numbers that have to agree.
///
/// `None` is a real answer and not an error: it is the top row pressing Up
/// and the last tile pressing Right, and a wall that wrapped around at those
/// edges would move the eye somewhere it did not ask to go.
#[must_use]
pub(crate) fn step(
    from: usize,
    direction: Direction,
    columns: usize,
    ends: &[usize],
) -> Option<usize> {
    let total = ends.last().copied().unwrap_or(0);
    if columns == 0 || from >= total {
        return None;
    }
    match direction {
        // Reading order, across headings — see the module note.
        Direction::Left => from.checked_sub(1),
        Direction::Right => (from + 1 < total).then_some(from + 1),
        Direction::Up | Direction::Down => vertical(from, direction, columns, ends),
    }
}

/// The shelf `index` is in, as `(start, end)`.
fn shelf_of(index: usize, ends: &[usize]) -> (usize, usize) {
    let mut start = 0;
    for &end in ends {
        if index < end {
            return (start, end);
        }
        start = end;
    }
    (start, start)
}

fn vertical(from: usize, direction: Direction, columns: usize, ends: &[usize]) -> Option<usize> {
    let (start, end) = shelf_of(from, ends);
    let column = (from - start) % columns;
    if direction == Direction::Up {
        // A row up inside this shelf, while there is one.
        if from >= start + columns {
            return Some(from - columns);
        }
        // Otherwise the shelf above, in its **last** row. Its last row is
        // short whenever its count is not a multiple of the columns, so the
        // column may not exist there — and then the nearest tile in that row
        // is its last, which is what an eye moving up a ragged edge does.
        let above_end = start;
        if above_end == 0 {
            return None;
        }
        let (above_start, _) = shelf_of(above_end - 1, ends);
        let last_row_start = above_end - 1 - (above_end - 1 - above_start) % columns;
        Some((last_row_start + column).min(above_end - 1))
    } else {
        // A row down inside this shelf, while there is one.
        if from + columns < end {
            return Some(from + columns);
        }
        // **This shelf's own last row can still be below you, and short.**
        // Over four columns a shelf of five is `[0 1 2 3]` then `[4]`, so
        // pressing Down on tile 1 has a row beneath it with no tile in
        // column 1. Its nearest is the shelf's last, exactly as moving up a
        // ragged edge takes the nearest above — the rule is *never skip a
        // row*, because a row you can see is a row an arrow should reach.
        let last_row = (end - 1 - start) / columns;
        if (from - start) / columns < last_row {
            return Some(end - 1);
        }
        // Otherwise the shelf below, in its **first** row, clamped the same
        // way for the same reason.
        if end >= ends.last().copied().unwrap_or(0) {
            return None;
        }
        let (_, below_end) = shelf_of(end, ends);
        Some((end + column).min(below_end - 1))
    }
}

#[cfg(test)]
mod tests {
    use super::step;
    use crate::search::Direction::{Down, Left, Right, Up};

    /// Three shelves — 5, 3 and 4 albums — over four columns, which is the
    /// shape that makes every boundary case reachable: shelf 0 ends in a row
    /// of one, shelf 1 is a single short row, shelf 2 is exactly one row.
    ///
    /// ```text
    ///   A   0  1  2  3
    ///       4
    ///   B   5  6  7
    ///   C   8  9 10 11
    /// ```
    const ENDS: [usize; 3] = [5, 8, 12];
    const COLUMNS: usize = 4;

    fn go(from: usize, direction: crate::search::Direction) -> Option<usize> {
        step(from, direction, COLUMNS, &ENDS)
    }

    /// **Left and Right are reading order and run off the ends of lines.**
    ///
    /// Including across a heading: pressing Right on `A`'s last record asks
    /// for the next record, and the next record is `B`'s first.
    #[test]
    fn sideways_is_reading_order_and_crosses_headings() {
        assert_eq!(go(0, Right), Some(1));
        assert_eq!(go(3, Right), Some(4), "off the end of a full row");
        assert_eq!(go(4, Right), Some(5), "off the end of a shelf");
        assert_eq!(go(1, Left), Some(0));
        assert_eq!(go(5, Left), Some(4), "back over a heading");
    }

    /// **Nothing wraps.** The first tile has nothing to its left and the last
    /// has nothing to its right; a wall that looped would move the eye
    /// somewhere it did not ask to go.
    #[test]
    fn the_collection_has_two_ends_and_does_not_join_them() {
        assert_eq!(go(0, Left), None);
        assert_eq!(go(11, Right), None);
        assert_eq!(go(0, Up), None, "the top row has nothing above it");
        assert_eq!(go(1, Up), None);
        for tile in 8..12 {
            assert_eq!(go(tile, Down), None, "the last shelf has nothing below");
        }
    }

    /// **A row up or down inside one shelf is the plain arithmetic.**
    #[test]
    fn within_a_shelf_a_row_is_the_column_count() {
        assert_eq!(go(4, Up), Some(0));
        assert_eq!(go(0, Down), Some(4));
    }

    /// **Across a heading, the column is kept where the shelf has one.**
    ///
    /// `B` is a single row of three, so moving down from `A`'s row of one
    /// lands in `B`'s first row at the same column.
    #[test]
    fn across_a_heading_the_column_is_kept() {
        assert_eq!(go(5, Up), Some(4), "column 0 up into A's short last row");
        assert_eq!(go(8, Up), Some(5), "column 0 up into B");
        assert_eq!(go(10, Up), Some(7), "column 2 up into B");
        assert_eq!(go(5, Down), Some(8));
        assert_eq!(
            go(7, Down),
            Some(10),
            "column 2 of B down into column 2 of C"
        );
    }

    /// **A column the next shelf does not have takes its nearest tile.**
    ///
    /// `A` ends in a row of one, so everything in `B` past column 0 has no
    /// tile directly above it. The nearest in that row is `A`'s last — which
    /// is what an eye moving up a ragged edge does, and it is why this is not
    /// `index - columns` with a bounds check.
    #[test]
    fn a_column_the_neighbouring_shelf_lacks_takes_its_nearest_tile() {
        assert_eq!(go(6, Up), Some(4), "column 1, and A's last row has one");
        assert_eq!(go(7, Up), Some(4), "column 2, likewise");
        assert_eq!(go(11, Up), Some(7), "column 3 into B's row of three");
        assert_eq!(
            go(1, Down),
            Some(4),
            "down onto A's short last row, which has no column 1"
        );
        assert_eq!(go(3, Down), Some(4), "and likewise from column 3");
    }

    /// **A wall with nothing on it answers nothing**, rather than dividing by
    /// a column count it does not have.
    #[test]
    fn an_empty_wall_and_a_measureless_one_answer_nothing() {
        for direction in [Up, Down, Left, Right] {
            assert_eq!(step(0, direction, COLUMNS, &[]), None);
            assert_eq!(step(0, direction, 0, &ENDS), None);
            assert_eq!(step(99, direction, COLUMNS, &ENDS), None);
        }
    }
}
