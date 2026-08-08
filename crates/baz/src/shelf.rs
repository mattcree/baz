//! Shelf grid geometry: the virtualization math, kept pure and tested.
//!
//! The shelf renders only the rows intersecting the scroll viewport (plus
//! [`OVERSCAN_ROWS`] above and below), with fixed-height spacers standing in
//! for everything else — the technique proven by the Phase 1 iced spike
//! (`git show dc13d7e:spikes/shelf-iced/src/main.rs`). A 10k-album shelf
//! therefore costs ~40 live widgets per frame, not 10 000.
//!
//! # The grid block is a column block, not a content block
//!
//! [`block_width`] is the width the centred grid occupies, and it is
//! `cols × CELL_W` — what the *columns* need, never what the items in a row
//! happen to fill. The distinction is invisible on a full shelf and glaring on
//! a filtered one: a row sized to its contents makes the last surviving album
//! of a search jump from the first column position to the middle of the
//! window, so the eye has to go and find the thing it just narrowed to.
//! Reserving the full block leaves every result where its column is.
//!
//! # Holding the columns still under a double-click
//!
//! Clicking a tile opens the album inspector, which takes [`crate::theme`]'s
//! panel width off the shelf and reflows the grid — five columns to three at
//! the shipped window size. Done immediately, that moves the tile out from
//! under the pointer *between the two presses of a double-click*, and since
//! the inspector's own footer advertises "double-click a tile to play", the
//! gesture then fails silently: the second press lands on empty shelf, or on
//! a different album.
//!
//! [`ColumnHold`] is the fix, and it is deliberately the smallest one that
//! works: the column count in force is pinned to what it was when the click
//! landed, for [`DOUBLE_CLICK`] — the same window the double-click detector
//! itself uses, because it is the same fact — and the reflow simply happens
//! when the gesture can no longer be one. Every other reflow (a resize, a
//! panel swap, closing the rail from the keyboard) is untouched.

/// Tile width including inter-tile padding (logical px). Art leads: the
/// tile is mostly artwork ([`ART_PX`]) with a 32 px art-to-art gutter.
pub const CELL_W: f32 = 240.0;
/// Tile height including caption (logical px).
pub const CELL_H: f32 = 284.0;
/// Artwork edge inside a tile (logical px). Generous by design — the shelf
/// pillar says art *is* the interface (docs/VISION.md pillar 5).
pub const ART_PX: f32 = 208.0;
/// Outer padding around the whole grid (logical px).
pub const GRID_PADDING: f32 = 24.0;
/// Extra rows rendered beyond each edge of the viewport so fast flings meet
/// already-built rows instead of blank space.
pub const OVERSCAN_ROWS: usize = 2;

/// Two presses on the same tile within this window play the album, and the
/// grid holds its column count for exactly as long (module docs).
///
/// One constant for both because they are one fact: the window in which a
/// second press is still part of the first press's gesture. Two numbers here
/// would be a bug waiting for somebody to change one of them — a hold shorter
/// than the detector leaves the gap the reflow used to fall into, and a longer
/// one delays a reflow nobody is still gesturing at.
pub const DOUBLE_CLICK: std::time::Duration = std::time::Duration::from_millis(400);

/// Columns fitting in a viewport `width`; at least 1.
#[must_use]
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "floor() of a non-negative finite quotient of small screen sizes"
)]
pub fn columns(width: f32) -> usize {
    (((width - 2.0 * GRID_PADDING) / CELL_W).floor() as usize).max(1)
}

/// Half-open row range `[first, end)` to render for a scroll offset and
/// viewport height, overscan included, clamped to `total_rows`.
#[must_use]
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "floor()/ceil() of non-negative finite pixel counts"
)]
pub fn visible_rows(scroll_offset: f32, viewport_height: f32, total_rows: usize) -> (usize, usize) {
    let first = ((scroll_offset.max(0.0) / CELL_H).floor() as usize)
        .saturating_sub(OVERSCAN_ROWS)
        .min(total_rows);
    let on_screen = (viewport_height.max(0.0) / CELL_H).ceil() as usize + 1;
    let end = (first + on_screen + 2 * OVERSCAN_ROWS).min(total_rows);
    (first, end)
}

/// Total rows needed for `items` laid out over `cols` columns.
#[must_use]
pub fn total_rows(items: usize, cols: usize) -> usize {
    items.div_ceil(cols.max(1))
}

/// Spacer height standing in for `rows` unrendered rows.
#[must_use]
#[expect(
    clippy::cast_precision_loss,
    reason = "row counts are far below f32's 2^24 exact-integer range"
)]
pub fn spacer_height(rows: usize) -> f32 {
    rows as f32 * CELL_H
}

/// Width of the centred grid block laid out over `cols` columns.
///
/// `cols × CELL_W` — the width the *columns* occupy, whether or not there are
/// enough items to fill them (module docs). A partial last row, and a search
/// narrowed to one result, therefore stay left-aligned in the block instead of
/// re-centring on their own contents.
#[must_use]
#[expect(
    clippy::cast_precision_loss,
    reason = "column counts are far below f32's 2^24 exact-integer range"
)]
pub fn block_width(cols: usize) -> f32 {
    cols as f32 * CELL_W
}

/// The column count pinned across the reflow a tile click causes (module
/// docs).
///
/// Pure state and pure arithmetic: it is *told* what time it is rather than
/// asking, so the whole of the timing rule is unit-testable without a window
/// and without a clock. The iced layer supplies the instant, ticks while a
/// hold is live, and spends the answer on a layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ColumnHold {
    /// The columns being held and the instant the hold expires. `None` is the
    /// ordinary state: the grid follows its measured width.
    held: Option<(usize, std::time::Instant)>,
}

impl ColumnHold {
    /// Pin `cols` until [`DOUBLE_CLICK`] after `now`.
    ///
    /// Re-holding replaces the window rather than extending a stale one, so a
    /// second click's gesture gets a full window of its own — which is what
    /// makes a *triple* click (play, then a third press) behave like the
    /// double-click before it rather than like a reflow.
    pub fn hold(&mut self, cols: usize, now: std::time::Instant) {
        self.held = Some((cols, now + DOUBLE_CLICK));
    }

    /// Whether a hold is still recorded. The app ticks only while one is, so
    /// this is what keeps a subscription alive for exactly as long as it has
    /// something to do.
    #[must_use]
    pub fn holding(self) -> bool {
        self.held.is_some()
    }

    /// Drop the hold if its window has passed, reporting whether anything
    /// changed — the caller re-lays the grid out only when it did.
    pub fn expire(&mut self, now: std::time::Instant) -> bool {
        if self.held.is_some_and(|(_, until)| now >= until) {
            self.held = None;
            return true;
        }
        false
    }

    /// Drop the hold outright: the gesture ended some other way, and the
    /// grid's real width is the honest answer again.
    pub fn release(&mut self) {
        self.held = None;
    }

    /// The column count to lay the grid out with: the held one while a hold
    /// stands, else the `measured` one the viewport's width gives.
    ///
    /// Time-free by construction. A hold that has expired is removed by
    /// [`Self::expire`], never silently ignored here, so there is exactly one
    /// place the clock is consulted and the layout is not it.
    #[must_use]
    pub fn columns(self, measured: usize) -> usize {
        self.held.map_or(measured, |(cols, _)| cols)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;

    #[test]
    fn columns_never_zero_and_scale_with_width() {
        assert_eq!(columns(0.0), 1);
        assert_eq!(columns(100.0), 1);
        // 1280 wide: (1280 - 48) / 240 = 5.13… → 5 columns.
        assert_eq!(columns(1280.0), 5);
        assert!(columns(2560.0) > columns(1280.0));
    }

    /// The grid block is as wide as its *columns*, at every width the window
    /// and the rail between them can produce — never as wide as the items that
    /// happen to be in a row.
    ///
    /// This is the assertion behind "filtering to one result leaves it in the
    /// first column position": the block a single tile is centred in is the
    /// same block five tiles are centred in, so the survivor does not move.
    #[test]
    fn the_grid_block_is_as_wide_as_its_columns() {
        // Every width the shipped window and the rail can produce, plus the
        // extremes at either end of the band.
        for width in [640.0_f32, 760.0, 940.0, 1000.0, 1280.0, 1400.0, 2560.0] {
            let cols = columns(width);
            let block = block_width(cols);
            #[expect(
                clippy::cast_precision_loss,
                reason = "a column count is far below f32's exact-integer range"
            )]
            let expected = cols as f32 * CELL_W;
            assert!(
                (block - expected).abs() < f32::EPSILON,
                "{width} px: block {block} is not {cols} × {CELL_W}"
            );
            // It has to fit the viewport it is centred in, or the block itself
            // would be what forces a horizontal scrollbar.
            assert!(
                block <= width - 2.0 * GRID_PADDING || cols == 1,
                "{width} px: a {cols}-column block ({block}) overflows its padding"
            );
        }
        // One item or a full row, the block is the same width — the whole
        // point. At 1280 px that is five columns either way.
        assert!((block_width(columns(1280.0)) - 5.0 * CELL_W).abs() < f32::EPSILON);
        // A degenerate zero-column block has no width rather than a negative
        // one; `columns` never returns it, and the arithmetic still holds.
        assert!(block_width(0).abs() < f32::EPSILON);
    }

    /// The column hold, as a timing rule: pinned for [`DOUBLE_CLICK`], and the
    /// measured width again the moment the gesture can no longer be one.
    #[test]
    fn the_column_count_is_held_for_exactly_the_double_click_window() {
        let start = Instant::now();
        let mut hold = ColumnHold::default();

        // At rest the grid simply follows what it measured.
        assert!(!hold.holding());
        assert_eq!(hold.columns(3), 3);
        assert!(!hold.expire(start), "nothing held, nothing to expire");

        // A click on a five-column shelf that opens the inspector: the shelf
        // now measures three, and the grid must keep laying out five.
        hold.hold(5, start);
        assert!(hold.holding());
        assert_eq!(
            hold.columns(3),
            5,
            "the tile must not move under the pointer"
        );

        // Still held one millisecond before the window closes…
        let a_moment_early = DOUBLE_CLICK
            .checked_sub(std::time::Duration::from_millis(1))
            .expect("the hold window is longer than a millisecond");
        assert!(!hold.expire(start + a_moment_early));
        assert_eq!(hold.columns(3), 5);

        // …and released exactly at it, which is the first instant a second
        // press could no longer be part of the same double-click.
        assert!(hold.expire(start + DOUBLE_CLICK));
        assert!(!hold.holding());
        assert_eq!(hold.columns(3), 3, "the reflow lands once the gesture ends");
        // Expiring an expired hold is a no-op, so the tick can be coarse.
        assert!(!hold.expire(start + DOUBLE_CLICK));
    }

    /// The two ways a hold ends other than by timing out.
    #[test]
    fn a_hold_can_be_replaced_or_dropped_outright() {
        let start = Instant::now();
        let mut hold = ColumnHold::default();

        // A second click re-holds: the window is the new click's, not the
        // remains of the old one's.
        hold.hold(5, start);
        hold.hold(5, start + DOUBLE_CLICK);
        assert!(
            !hold.expire(start + DOUBLE_CLICK),
            "re-holding starts a fresh window rather than inheriting a spent one"
        );
        assert_eq!(hold.columns(3), 5);
        assert!(hold.expire(start + DOUBLE_CLICK + DOUBLE_CLICK));

        // Released outright: the measured width wins immediately.
        hold.hold(5, start);
        hold.release();
        assert!(!hold.holding());
        assert_eq!(hold.columns(2), 2);
    }

    /// The hold window and the double-click window are the same number,
    /// because they are the same fact (module docs).
    #[test]
    fn the_hold_window_is_the_double_click_window() {
        assert_eq!(DOUBLE_CLICK, std::time::Duration::from_millis(400));
        let start = Instant::now();
        let mut hold = ColumnHold::default();
        hold.hold(5, start);
        // Held for every instant a second press still counts as a double
        // click, and for no instant after.
        assert!(!hold.expire(start + DOUBLE_CLICK / 2));
        assert!(hold.expire(start + DOUBLE_CLICK));
    }

    #[test]
    fn visible_rows_clamp_to_totals() {
        // Empty shelf: nothing to render.
        assert_eq!(visible_rows(0.0, 800.0, 0), (0, 0));
        // Scrolled far past the end: empty range at the end, no underflow.
        let (first, end) = visible_rows(1.0e7, 800.0, 10);
        assert!(first <= end && end <= 10);
    }

    #[test]
    fn visible_rows_cover_viewport_plus_overscan() {
        let total = 1000;
        let (first, end) = visible_rows(0.0, 800.0, total);
        assert_eq!(first, 0, "top of shelf starts at row 0");
        // 800 / 284 = 2.82 → ceil 3 (+1 partial) + 2×2 overscan = 8.
        assert_eq!(end, 8);

        // One viewport down: overscan reaches back above the fold.
        let (first, end) = visible_rows(CELL_H * 10.0, 800.0, total);
        assert_eq!(first, 10 - OVERSCAN_ROWS);
        assert!(end >= 10 + 4);
    }

    #[test]
    fn rows_and_spacers_are_consistent() {
        assert_eq!(total_rows(0, 7), 0);
        assert_eq!(total_rows(7, 7), 1);
        assert_eq!(total_rows(8, 7), 2);
        assert_eq!(total_rows(5, 0), 5, "degenerate column count clamps to 1");
        let close = (spacer_height(3) - 3.0 * CELL_H).abs() < f32::EPSILON;
        assert!(close);
    }
}
