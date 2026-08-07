//! Shelf grid geometry: the virtualization math, kept pure and tested.
//!
//! The shelf renders only the rows intersecting the scroll viewport (plus
//! [`OVERSCAN_ROWS`] above and below), with fixed-height spacers standing in
//! for everything else — the technique proven by the Phase 1 iced spike
//! (`git show dc13d7e:spikes/shelf-iced/src/main.rs`). A 10k-album shelf
//! therefore costs ~40 live widgets per frame, not 10 000.

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_never_zero_and_scale_with_width() {
        assert_eq!(columns(0.0), 1);
        assert_eq!(columns(100.0), 1);
        // 1280 wide: (1280 - 48) / 240 = 5.13… → 5 columns.
        assert_eq!(columns(1280.0), 5);
        assert!(columns(2560.0) > columns(1280.0));
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
