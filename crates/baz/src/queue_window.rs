//! The run column's virtual window: which slice of the rows the surface
//! actually builds, and the two spacers standing in for the rest.
//!
//! # Why the run column virtualizes at all
//!
//! `Play all` (doc 09 §7.1) reifies the wall — every visible record, in the
//! wall's arrangement order — into the queue in one press. At a large
//! library's scale that is a five-figure-track queue, and the engine is
//! indifferent (a `Vec<PathBuf>`), but a place that draws every row eagerly
//! is not: forty thousand `button`s per frame is a hang, not a list. Doc 09
//! names the run's virtualization as `Play all`'s implementation gate, so
//! the two ship together. The wall's own virtualizer is the in-repo
//! precedent ([`crate::shelf::Grid::visible_rows`], `views/shelf.rs`'s
//! spacer column): everything not on screen is a single [`Space`](iced::widget::Space) of the
//! right height, so a 40 000-track run costs the frame what a twelve-track
//! record does.
//!
//! # The same split as the wall's
//!
//! The *math* lives here — pure, iced-free, unit-tested (ADR-0006 layer 1)
//! — and [`crate::views::queue`] draws the slice it is handed, computing
//! none of it. The view keeps the geometry honest by construction: every
//! element it builds is wrapped in a box of exactly the pitch this module
//! declared for it, so the spacers and the drawn rows cannot add up to a
//! different list than the arithmetic did. The heights are this room's own
//! tokens ([`theme::LINE_BODY`], [`theme::GAP_XS`], …) — the module owns
//! the sums, never new numbers.
//!
//! # What a row's pitch is
//!
//! The rows column interleaves two kinds of element, both top-aligned in
//! their boxes with the inter-row gap folded into the pitch (a spacing-0
//! column, so a spacer element does not double the gaps around itself):
//!
//! - **a queue row**: `GAP_XS` padding both sides of a [`theme::LINE_BODY`]
//!   title, plus the `GAP_XXS`-spaced [`theme::LINE_META`] artist line when
//!   the row carries one;
//! - **a record's group header**: `GAP_MD` air both sides (the break
//!   belongs to the record it opens), the title line, and the artist line
//!   when the record has a title of its own to put it under.
//!
//! [`MARGIN`] rows' worth of slack is rendered beyond both viewport edges,
//! which is what lets the caller hand in an *estimate* of where the rows
//! begin inside the scrollable — the summary strip and the save field above
//! them move by less than the margin absorbs.

use crate::theme;

/// How far past the viewport's edges the window extends, in logical px —
/// slack for scroll momentum and for the caller's estimate of where the
/// rows column starts inside the scrollable content.
pub const MARGIN: f32 = 600.0;

/// One element of the run column's rows, as much of its shape as
/// the window arithmetic needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RowShape {
    /// A record's group header opens above this row — `Some(two_line)`,
    /// where `two_line` says the header sets an artist line under the
    /// record's title (it does whenever the record has a title; a header
    /// for an untitled record is its artist alone).
    pub head: Option<bool>,
    /// The row carries its own artist line under the title.
    pub two_line: bool,
}

/// The slice of the rows column the place builds this frame, and the two
/// spacers that stand in for everything else.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Window {
    /// First row index drawn (its header, when it has one, is drawn with
    /// it — a header is never separated from the row it opens).
    pub first: usize,
    /// One past the last row drawn.
    pub end: usize,
    /// The spacer above the slice: the exact height of every skipped
    /// element before `first`.
    pub top: f32,
    /// The spacer below the slice: the exact height of every skipped
    /// element from `end` on.
    pub bottom: f32,
}

/// A queue row's pitch: the row's own box plus the inter-row gap.
#[must_use]
pub const fn row_pitch(two_line: bool) -> f32 {
    let lines = if two_line {
        theme::LINE_BODY + theme::GAP_XXS + theme::LINE_META
    } else {
        theme::LINE_BODY
    };
    2.0 * theme::GAP_XS + lines + theme::GAP_XS
}

/// A record header's pitch: its air on both sides, its line or two, and the
/// inter-row gap.
#[must_use]
pub const fn header_pitch(two_line: bool) -> f32 {
    let lines = if two_line {
        theme::LINE_BODY + theme::GAP_XXS + theme::LINE_META
    } else {
        theme::LINE_BODY
    };
    2.0 * theme::GAP_MD + lines + theme::GAP_XS
}

/// A row's whole pitch: its header, when one opens above it, plus itself.
fn unit_pitch(row: RowShape) -> f32 {
    row.head.map_or(0.0, header_pitch) + row_pitch(row.two_line)
}

/// The window over `rows` for a viewport `viewport_h` tall whose top edge
/// is `offset` px into the rows column ([`MARGIN`] slack both ways; the
/// caller may pass an estimate that is off by less than it).
///
/// The invariant the tests pin: `top` + the drawn pitches + `bottom` is the
/// column's whole height at every offset, so scrolling moves the window and
/// never the scrollbar's idea of the list.
#[must_use]
pub fn window(rows: &[RowShape], offset: f32, viewport_h: f32) -> Window {
    let from = offset - MARGIN;
    let to = offset + viewport_h + MARGIN;
    let mut first = rows.len();
    let mut end = rows.len();
    let mut top = 0.0_f32;
    let mut bottom = 0.0_f32;
    let mut y = 0.0_f32;
    for (index, row) in rows.iter().enumerate() {
        let pitch = unit_pitch(*row);
        let below = y + pitch;
        if below <= from {
            top = below;
        } else if first == rows.len() {
            first = index;
        }
        if y >= to {
            if end == rows.len() {
                end = index;
            }
            bottom += pitch;
        }
        y = below;
    }
    if first > end {
        // The whole list sits above the window (a scroll past the end of a
        // shrunken queue): draw nothing, keep the height.
        first = end;
    }
    Window {
        first,
        end,
        top,
        bottom,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shapes(len: usize) -> Vec<RowShape> {
        (0..len)
            .map(|index| RowShape {
                // A header every twelve rows — a shuffle's stack of records —
                // and a mix of one- and two-line rows.
                head: (index % 12 == 0 && index > 0).then_some(true),
                two_line: index % 3 == 0,
            })
            .collect()
    }

    fn total(rows: &[RowShape]) -> f32 {
        rows.iter().map(|row| unit_pitch(*row)).sum()
    }

    /// **S6's implementation gate** (doc 09 §7.1): a 40 000-row run column
    /// builds a bounded slice of elements — the window never grows with the
    /// list, only with the viewport — so `Play all` over a five-figure
    /// library costs the frame what a twelve-track record does.
    #[test]
    fn a_forty_thousand_row_queue_builds_a_bounded_window() {
        let rows = shapes(40_000);
        let viewport = 1080.0;
        // The worst case per pitch is every row one-line with no header:
        // the window can hold at most the span divided by that, plus the
        // two partially-visible rows at its edges.
        let span = viewport + 2.0 * MARGIN;
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a positive row count in the tens; the assert below pins it"
        )]
        let bound = (span / row_pitch(false)).ceil() as usize + 2;
        assert!(bound < 100, "the bound itself stays two digits: {bound}");
        for offset in [0.0, 500.0, 10_000.0, 123_456.0, 1_000_000.0] {
            let window = window(&rows, offset, viewport);
            assert!(
                window.end - window.first <= bound,
                "{} rows drawn at offset {offset} — the place is not virtual",
                window.end - window.first
            );
        }
    }

    /// The spacers plus the drawn slice are the whole column at every
    /// offset: scrolling moves the window, never the scrollbar's idea of
    /// how long the list is.
    #[test]
    fn the_spacers_and_the_slice_always_sum_to_the_whole_column() {
        let rows = shapes(500);
        let whole = total(&rows);
        for offset in 0..1000 {
            #[expect(clippy::cast_precision_loss, reason = "offsets in px")]
            let offset = (offset * 37) as f32;
            let window = window(&rows, offset, 900.0);
            let drawn: f32 = rows[window.first..window.end]
                .iter()
                .map(|row| unit_pitch(*row))
                .sum();
            let sum = window.top + drawn + window.bottom;
            assert!(
                (sum - whole).abs() < 0.01,
                "at {offset}: {sum} != {whole} (window {window:?})"
            );
        }
    }

    /// The drawn slice covers the viewport and its margins: every element
    /// that intersects the padded span is inside `first..end`, so nothing
    /// on screen is a spacer.
    #[test]
    fn everything_the_padded_viewport_touches_is_drawn() {
        let rows = shapes(300);
        let viewport = 700.0;
        for offset in [0.0, 100.0, 2_000.0, 7_777.0] {
            let win = window(&rows, offset, viewport);
            let mut y = 0.0;
            for (index, row) in rows.iter().enumerate() {
                let pitch = unit_pitch(*row);
                let intersects = y + pitch > offset - MARGIN && y < offset + viewport + MARGIN;
                if intersects {
                    assert!(
                        index >= win.first && index < win.end,
                        "row {index} intersects at offset {offset} but is a spacer"
                    );
                }
                y += pitch;
            }
        }
    }

    /// A short queue is drawn whole from every offset the scrollable can
    /// actually reach — the ordinary twelve-track record never sees a
    /// spacer, so nothing about the shipped place changes until the list
    /// outgrows the window.
    #[test]
    fn a_short_queue_is_always_drawn_whole() {
        let rows = shapes(12);
        for offset in [0.0, 50.0, 300.0] {
            let win = window(&rows, offset, 800.0);
            assert_eq!((win.first, win.end), (0, 12));
            assert!(win.top.abs() < f32::EPSILON);
            assert!(win.bottom.abs() < f32::EPSILON);
        }
    }

    /// An empty queue asks for nothing and claims no height.
    #[test]
    fn an_empty_queue_is_an_empty_window() {
        let win = window(&[], 0.0, 800.0);
        assert_eq!((win.first, win.end), (0, 0));
        assert!(win.top.abs() < f32::EPSILON && win.bottom.abs() < f32::EPSILON);
    }
}
