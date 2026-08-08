//! Shelf grid geometry: the hang's arithmetic and the virtualization math,
//! kept pure and tested.
//!
//! The shelf renders only the rows intersecting the scroll viewport (plus
//! [`OVERSCAN_ROWS`] above and below), with fixed-height spacers standing in
//! for everything else — the technique proven by the Phase 1 iced spike
//! (`git show dc13d7e:spikes/shelf-iced/src/main.rs`). A 10k-album shelf
//! therefore costs ~40 live widgets per frame, not 10 000.
//!
//! # The cell is a function of the grid, not a constant
//!
//! Until ADR-0017 step 5 the tile was 240 × 284 with 208 px of art in it, at
//! every window size, and every pixel the window had that 240 did not divide
//! **pooled at the edges of the wall**: 154 px of dead gutter at 1280 with the
//! inspector open, 192 px at 1920. A designer's constant, and the wall paid
//! for it.
//!
//! [`Grid`] replaces it with the arithmetic of `.interface-design/system.md`
//! §7, driven by one number — [`crate::theme::HANG`], the distance from a work to its
//! neighbour *and* from a work to the edge of the wall:
//!
//! ```text
//! columns(w) = clamp(floor((w + HANG) / (ART_TARGET + HANG) + 0.5),
//!                    1,
//!                    max(1, floor((w - HANG) / (ART_MIN + HANG))))
//! art(w)     = min(ART_MAX, (w - (columns + 1) * HANG) / columns)
//! gutter(w)  = columns > 1 ? (w - 2*HANG - columns*art) / (columns - 1) : 0
//! row_h(w)   = art(w) + GAP_LG + LABEL_H + HANG
//! ```
//!
//! **`floor(x + 0.5)`, never a language's `round`**: Rust's `f32::round` is
//! half-away-from-zero and Python's is banker's, and a grid whose column count
//! depends on which language expressed it is not a specification.
//!
//! # Why there is no dead gutter, at any width
//!
//! The property the whole section is for, and it is algebra rather than a
//! table of measurements. When the art is *not* at [`crate::theme::ART_MAX`],
//! `columns × art = w − (columns + 1) × HANG` by construction, so
//!
//! ```text
//! gutter = (w − 2·HANG − columns·art) / (columns − 1)
//!        = ((columns + 1)·HANG − 2·HANG) / (columns − 1)
//!        = (columns − 1)·HANG / (columns − 1)
//!        = HANG
//! ```
//!
//! and the margin the block leaves is `(w − block) / 2 = HANG` by the same
//! substitution. Work-to-work and work-to-wall are then the same number,
//! which is what `HANG` being one token rather than two is claiming. Every
//! spare pixel is in the artwork; none of it is at the edges.
//!
//! Above `ART_MAX` the art stops growing and the *margins* take the slack
//! instead — the one asymmetric padding in the product other than
//! [`crate::theme::scroll_gutter`] — so the gutter rises to at most `2 × HANG` and
//! the block stays centred. That is the only case in which gutter ≠ HANG, and
//! it is a case in which the alternative is upscaling a thumbnail.
//!
//! # The grid block is a column block, not a content block
//!
//! [`Grid::block_width`] is the width the centred grid occupies, and it is
//! what the *columns* need, never what the items in a row happen to fill. The
//! distinction is invisible on a full shelf and glaring on a filtered one: a
//! row sized to its contents makes the last surviving album of a search jump
//! from the first column position to the middle of the window, so the eye has
//! to go and find the thing it just narrowed to. Reserving the full block
//! leaves every result where its column is.
//!
//! # Holding the grid still under a double-click
//!
//! Clicking a tile opens the album inspector, which takes [`crate::theme::PANEL_W`]
//! off the shelf and reflows the grid — four columns to three at the shipped
//! window size, *and* every sleeve's size with them. Done immediately, that
//! moves the tile out from under the pointer *between the two presses of a
//! double-click*, and since the inspector's own footer advertises
//! "double-click a tile to play", the gesture then fails silently: the second
//! press lands on empty shelf, or on a different album.
//!
//! [`GridHold`] is the fix, and it is deliberately the smallest one that
//! works: **the grid width in force** is pinned to what it was when the click
//! landed, for [`DOUBLE_CLICK`] — the same window the double-click detector
//! itself uses, because it is the same fact — and the reflow simply happens
//! when the gesture can no longer be one. Every other reflow (a resize, a
//! panel swap, closing the inspector from the keyboard) is untouched.
//!
//! It pins the *width* rather than the column count, which is what step 5
//! changed about it and had to: with a fluid cell, holding five columns while
//! the width moved would have held the count still and let every sleeve in
//! those columns change size, which is the same tile moving under the same
//! pointer by another route.

use crate::theme::{ART_MAX, ART_MIN, ART_TARGET, GAP_LG, HANG, LABEL_H, SHELF_HEADER_H};

/// Extra rows rendered beyond each edge of the viewport so fast flings meet
/// already-built rows instead of blank space.
pub const OVERSCAN_ROWS: usize = 2;

/// The smallest edge the wall will draw a sleeve at, whatever the arithmetic
/// says (logical px).
///
/// Only reachable below ~80 px of grid, which no window baz can be given
/// produces — it exists so the geometry is total rather than nearly total,
/// and so a degenerate width yields a small wall instead of an inverted one.
const ART_FLOOR: f32 = 1.0;

/// Two presses on the same tile within this window play the album, and the
/// grid holds its width for exactly as long (module docs).
///
/// One constant for both because they are one fact: the window in which a
/// second press is still part of the first press's gesture. Two numbers here
/// would be a bug waiting for somebody to change one of them — a hold shorter
/// than the detector leaves the gap the reflow used to fall into, and a longer
/// one delays a reflow nobody is still gesturing at.
pub const DOUBLE_CLICK: std::time::Duration = std::time::Duration::from_millis(400);

/// `floor(x + 0.5)` — half-up rounding, spelled out (module docs).
fn round_half_up(value: f32) -> f32 {
    (value + 0.5).floor()
}

/// The hang, resolved for one grid width: how many columns, how large the
/// works are, and what sits between them.
///
/// Cheap enough to build per layout pass — six multiplications and a floor —
/// which is what `.interface-design/system.md` §11 costs the fluid cell at:
/// arithmetic per layout pass, not per tile.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Grid {
    /// The width this grid was resolved for (logical px).
    pub width: f32,
    /// Columns the wall hangs, at least 1.
    pub columns: usize,
    /// Edge of one work (logical px), never above [`ART_MAX`].
    pub art: f32,
    /// Work-to-work gap (logical px). Exactly [`HANG`] whenever the art is
    /// uncapped, and at most `2 × HANG` when it is.
    pub gutter: f32,
    /// Work-to-wall-edge gap (logical px), the block being centred.
    pub margin: f32,
    /// Row pitch (logical px): the work, the gap to its label, the label, and
    /// the hang to the row below.
    pub row_h: f32,
}

impl Grid {
    /// Resolve the hang for a grid of `width` logical pixels.
    ///
    /// `width` is the width the *shelf* has — the window less the inspector —
    /// not the window's. The scrollbar overlays the right margin rather than
    /// taking width from the block, which it can now do without clipping
    /// anything: the margin is [`HANG`] 40 and the bar's lane is 10.
    #[must_use]
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        reason = "column counts are small non-negative integers, far below \
                  f32's exact-integer range, and every quotient floored here \
                  is finite and clamped"
    )]
    pub fn new(width: f32) -> Self {
        let width = width.max(0.0);
        // The count the wall wants, and the count the smallest acceptable work
        // allows. The second is a ceiling rather than a preference: a window
        // gains a column only when the column it gains is still worth looking
        // at.
        let wanted = round_half_up((width + HANG) / (ART_TARGET + HANG));
        let ceiling = ((width - HANG) / (ART_MIN + HANG)).floor().max(1.0);
        let columns = wanted.clamp(1.0, ceiling).max(1.0) as usize;

        let count = columns as f32;
        let art = ((width - (count + 1.0) * HANG) / count).clamp(ART_FLOOR, ART_MAX);
        let gutter = if columns > 1 {
            ((width - 2.0 * HANG - count * art) / (count - 1.0)).clamp(0.0, 2.0 * HANG)
        } else {
            0.0
        };
        let block = count * art + (count - 1.0) * gutter;
        Self {
            width,
            columns,
            art,
            gutter,
            margin: ((width - block) / 2.0).max(0.0),
            row_h: art + GAP_LG + LABEL_H + HANG,
        }
    }

    /// Width of the centred grid block: `columns` works and the gutters
    /// between them (module docs).
    ///
    /// What the *columns* occupy, whether or not there are enough items to
    /// fill them — so a partial last row, and a search narrowed to one result,
    /// stay left-aligned in the block instead of re-centring on their own
    /// contents.
    #[must_use]
    #[expect(
        clippy::cast_precision_loss,
        reason = "column counts are far below f32's exact-integer range"
    )]
    pub fn block_width(self) -> f32 {
        let count = self.columns as f32;
        count * self.art + (count - 1.0).max(0.0) * self.gutter
    }

    /// Total rows needed for `items` laid out over this grid's columns.
    #[must_use]
    pub fn rows(self, items: usize) -> usize {
        items.div_ceil(self.columns.max(1))
    }

    /// Half-open row range `[first, end)` to render for a scroll offset and
    /// viewport height, overscan included, clamped to `total_rows`.
    #[must_use]
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "floor()/ceil() of non-negative finite pixel counts"
    )]
    pub fn visible_rows(
        self,
        scroll_offset: f32,
        viewport_height: f32,
        total_rows: usize,
    ) -> (usize, usize) {
        let pitch = self.row_h.max(1.0);
        let first = ((scroll_offset.max(0.0) / pitch).floor() as usize)
            .saturating_sub(OVERSCAN_ROWS)
            .min(total_rows);
        let on_screen = (viewport_height.max(0.0) / pitch).ceil() as usize + 1;
        let end = (first + on_screen + 2 * OVERSCAN_ROWS).min(total_rows);
        (first, end)
    }

    /// Spacer height standing in for `rows` unrendered rows.
    #[must_use]
    #[expect(
        clippy::cast_precision_loss,
        reason = "row counts are far below f32's 2^24 exact-integer range"
    )]
    pub fn spacer_height(self, rows: usize) -> f32 {
        rows as f32 * self.row_h
    }
}

/// One shelf's place on the wall: its header band, its rows, and which slice
/// of the visible list it holds.
///
/// Produced by [`Shelves`]. Every measurement is from the top of the
/// scrollable's *content*, which is the coordinate the scroll offset is in, so
/// nothing here needs to know what a viewport is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Run {
    /// Which group this run draws — an index into the caller's own list of
    /// shelves, carried so the rail and the header can be looked up without a
    /// second parallel vector.
    pub group: usize,
    /// Index of this shelf's first album within the visible list.
    pub first: usize,
    /// How many albums survive the filter on this shelf. Never zero: a shelf
    /// with nothing left on it is not drawn at all.
    pub len: usize,
    /// How many grid rows those albums take.
    pub rows: usize,
    /// The top of the header band, in content coordinates.
    pub top: f32,
}

impl Run {
    /// The top of this shelf's first row of covers: the band, spent.
    #[must_use]
    pub fn rows_top(self) -> f32 {
        self.top + SHELF_HEADER_H
    }

    /// One past the bottom of this shelf — the top of the next shelf's band,
    /// or the bottom of the wall.
    #[must_use]
    #[expect(
        clippy::cast_precision_loss,
        reason = "row counts are far below f32's 2^24 exact-integer range"
    )]
    pub fn end(self, grid: Grid) -> f32 {
        self.rows_top() + self.rows as f32 * grid.row_h
    }
}

/// **The wall, broken into shelves** (ADR-0017 step 8, ADR-0019).
///
/// [`Grid`] answers how wide a work is and which rows are on screen; this
/// answers where one shelf ends and the next begins, and it is the same kind
/// of thing: pure arithmetic over a width and a list of counts, unit-tested
/// without a window.
///
/// # The vertical rhythm, stated once
///
/// The wall's own top hang, then, per shelf, a band of exactly
/// [`crate::theme::SHELF_HEADER_H`] and then its rows at the grid's pitch:
///
/// ```text
/// HANG                                   the wall's top edge
/// ┌ SHELF_HEADER_H  = HANG               the header band
/// │   HEADING_LINE_H = 14                  the header's line box, at its top
/// │   26                                    clear wall
/// └ rows × row_h                         the covers; each row_h ends in a HANG
/// ```
///
/// Every number is `HANG` or derived from it, so a shelf break costs the wall
/// exactly one more hang than a row break does and the whole page keeps one
/// vertical unit. The gap a reader sees above a header is the previous row's
/// trailing hang (40) and the gap below it is `HANG − HEADING_LINE_H` (26) —
/// **20 : 13**, a header nearer the shelf it names than the one it follows.
///
/// # Why the sticky header is exact rather than approximate
///
/// Because the band and the row's trailing gap are the same number, the scroll
/// offset at which a shelf's last row of covers leaves the top of the viewport
/// is *precisely* the offset at which the next shelf's band enters it. So the
/// pinned lane can hold exactly one header at every offset, with no overlap and
/// no gap and nothing that moves: see [`Shelves::sticky`].
#[derive(Debug, Clone, PartialEq)]
pub struct Shelves {
    runs: Vec<Run>,
    height: f32,
    grid: Grid,
}

impl Shelves {
    /// Lay `counts` — the number of albums surviving on each shelf, in shelf
    /// order — out over `grid`.
    ///
    /// Empty shelves are skipped rather than drawn as a header with nothing
    /// under it: a filtered wall shows the breaks its *results* fall on, not
    /// the breaks the library has. [`Run::group`] keeps the original index, so
    /// the caller's headers still line up.
    #[must_use]
    pub fn new(grid: Grid, counts: &[usize]) -> Self {
        let mut runs = Vec::with_capacity(counts.len());
        let mut first = 0;
        let mut top = HANG;
        for (group, &len) in counts.iter().enumerate() {
            if len == 0 {
                continue;
            }
            let run = Run {
                group,
                first,
                len,
                rows: grid.rows(len),
                top,
            };
            top = run.end(grid);
            first += len;
            runs.push(run);
        }
        Self {
            runs,
            height: top,
            grid,
        }
    }

    /// The shelves, in wall order.
    #[must_use]
    pub fn runs(&self) -> &[Run] {
        &self.runs
    }

    /// Total content height, including the wall's top hang and the trailing
    /// hang of its last row.
    #[must_use]
    pub fn height(&self) -> f32 {
        self.height
    }

    /// How many albums are on the wall at all.
    #[must_use]
    pub fn albums(&self) -> usize {
        self.runs.last().map_or(0, |run| run.first + run.len)
    }

    /// The run containing content coordinate `y` — the shelf whose band or
    /// rows that pixel belongs to.
    ///
    /// Everything above the first band belongs to the first shelf, so a wall
    /// scrolled to the very top already names its first header.
    #[must_use]
    pub fn run_at(&self, y: f32) -> Option<usize> {
        if self.runs.is_empty() {
            return None;
        }
        let y = y.max(0.0);
        // Runs are contiguous and ascending, so the answer is the last one
        // that starts at or before `y`. Linear rather than binary: a wall has
        // tens of shelves, not thousands, and this is one pass per frame.
        let mut found = 0;
        for (index, run) in self.runs.iter().enumerate() {
            if run.top <= y {
                found = index;
            } else {
                break;
            }
        }
        Some(found)
    }

    /// **Which header is pinned at the top of the viewport, and none is ever
    /// pinned over another.**
    ///
    /// `None` means the lane holds an in-flow header instead — either because
    /// the shelf's own header has not scrolled off yet (`scroll <= top`) or
    /// because the *next* shelf's band has entered the lane, and the incoming
    /// header is drawn where it lies rather than pinned.
    ///
    /// The two hand-overs are continuous, which is the property worth having:
    ///
    /// - At `scroll == run.top` the in-flow header sits at viewport y = 0,
    ///   which is exactly where the pinned one is drawn. Nothing moves.
    /// - The pin is released at `scroll == next.top − SHELF_HEADER_H`, and
    ///   since a row's pitch ends in a `HANG` and the band *is* a `HANG`, that
    ///   is the same instant the shelf's last row of covers clears the top of
    ///   the viewport. The header stops being pinned exactly when its shelf
    ///   stops being on screen at the top, and the lane below it is clear wall
    ///   rather than covers.
    ///
    /// So the pinned band never covers a header, never covers a cover, and
    /// never needs a transition to hide behind.
    #[must_use]
    pub fn sticky(&self, scroll: f32) -> Option<usize> {
        let index = self.run_at(scroll)?;
        let run = self.runs.get(index)?;
        if scroll <= run.top {
            return None;
        }
        match self.runs.get(index + 1) {
            Some(next) if scroll > next.top - SHELF_HEADER_H => None,
            _ => Some(index),
        }
    }

    /// Half-open range of runs `[first, end)` with anything to draw for a
    /// scroll offset and viewport height, [`OVERSCAN_ROWS`] included.
    ///
    /// The overscan is spent in the same unit [`Grid::visible_rows`] spends it
    /// in — rows — so a fling that crosses a shelf break meets built rows on
    /// the other side of it rather than a blank shelf.
    #[must_use]
    #[expect(
        clippy::cast_precision_loss,
        reason = "the overscan is two rows; f32 is exact far past that"
    )]
    pub fn visible_runs(&self, scroll: f32, viewport_height: f32) -> (usize, usize) {
        let slack = OVERSCAN_ROWS as f32 * self.grid.row_h;
        let top = scroll - slack;
        let bottom = scroll + viewport_height.max(0.0) + slack;
        let first = self
            .runs
            .iter()
            .position(|run| run.end(self.grid) > top)
            .unwrap_or(self.runs.len());
        let end = self
            .runs
            .iter()
            .position(|run| run.top >= bottom)
            .unwrap_or(self.runs.len());
        (first, end.max(first))
    }

    /// Half-open range of *albums* — indices into the visible list — that the
    /// viewport and its overscan touch.
    ///
    /// What the thumbnail prefetch spends: it asks for art by album, and it
    /// has to ask for the same albums the view is about to draw or it decodes
    /// the wrong ones.
    #[must_use]
    pub fn visible_albums(&self, scroll: f32, viewport_height: f32) -> (usize, usize) {
        let (first_run, end_run) = self.visible_runs(scroll, viewport_height);
        let mut start = self.albums();
        let mut end = 0;
        for run in &self.runs[first_run..end_run] {
            let (row, row_end) =
                self.grid
                    .visible_rows(scroll - run.rows_top(), viewport_height, run.rows);
            let columns = self.grid.columns.max(1);
            start = start.min(run.first + (row * columns).min(run.len));
            end = end.max(run.first + (row_end * columns).min(run.len));
        }
        if start > end { (0, 0) } else { (start, end) }
    }
}

/// The grid width pinned across the reflow a tile click causes (module docs).
///
/// Pure state and pure arithmetic: it is *told* what time it is rather than
/// asking, so the whole of the timing rule is unit-testable without a window
/// and without a clock. The iced layer supplies the instant, ticks while a
/// hold is live, and spends the answer on a layout.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct GridHold {
    /// The width being held and the instant the hold expires. `None` is the
    /// ordinary state: the grid follows its measured width.
    held: Option<(f32, std::time::Instant)>,
}

impl GridHold {
    /// Pin `width` until [`DOUBLE_CLICK`] after `now`.
    ///
    /// Re-holding replaces the window rather than extending a stale one, so a
    /// second click's gesture gets a full window of its own — which is what
    /// makes a *triple* click (play, then a third press) behave like the
    /// double-click before it rather than like a reflow.
    pub fn hold(&mut self, width: f32, now: std::time::Instant) {
        self.held = Some((width, now + DOUBLE_CLICK));
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

    /// The width to lay the grid out with: the held one while a hold stands,
    /// else the `measured` one the viewport gives.
    ///
    /// Time-free by construction. A hold that has expired is removed by
    /// [`Self::expire`], never silently ignored here, so there is exactly one
    /// place the clock is consulted and the layout is not it.
    #[must_use]
    pub fn width(self, measured: f32) -> f32 {
        self.held.map_or(measured, |(width, _)| width)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;

    /// Every grid width the shipped window and the inspector can produce
    /// between them, at 1 px resolution.
    ///
    /// The band's ends: iced will not hand baz a window narrower than the
    /// 640 px minimum, and the inspector takes [`crate::theme::PANEL_W`] 340
    /// off it, so 300 is the narrowest grid that exists; 2560 is a wall-sized
    /// monitor with the inspector closed. Stepping by 1 rather than by 20
    /// costs ~2 300 iterations of six multiplications, which is nothing, and
    /// it is what makes "at *every* width" a statement rather than a sample —
    /// the column-count transitions are single-pixel events and a coarse
    /// sweep can step straight over one.
    fn band() -> impl Iterator<Item = f32> {
        #[expect(
            clippy::cast_precision_loss,
            reason = "a grid width in pixels is far below f32's exact-integer range"
        )]
        (300..=2560).map(|width| width as f32)
    }

    /// **The proportion fix, as one assertion: dead gutter is 0 px at every
    /// width.**
    ///
    /// Whenever the art is not capped at [`ART_MAX`], the gutter is exactly
    /// [`HANG`] *and* the margin is exactly `HANG` — work-to-work and
    /// work-to-wall are the same number, and every spare pixel is inside the
    /// artwork rather than pooled at the edges (module docs prove it
    /// algebraically; this is the same claim over the real f32 arithmetic).
    ///
    /// The tolerance is 0.01 px, which is f32's rounding on numbers this size
    /// and not a design allowance: the algebra is exact.
    #[test]
    fn the_gutter_is_the_hang_wherever_the_art_is_uncapped() {
        /// f32 rounding on quantities of this magnitude, not a design
        /// allowance.
        const EPSILON: f32 = 0.01;

        let mut uncapped = 0;
        let mut capped = 0;
        for width in band() {
            let grid = Grid::new(width);
            if grid.art >= ART_MAX - EPSILON {
                capped += 1;
                // The one case gutter may exceed HANG: the art has stopped
                // growing, so the margins take the slack and the gutter rises
                // with them — never past 2 × HANG, and never below HANG.
                if grid.columns > 1 {
                    assert!(
                        grid.gutter >= HANG - EPSILON && grid.gutter <= 2.0 * HANG + EPSILON,
                        "{width} px: capped art with a {} px gutter",
                        grid.gutter
                    );
                }
                continue;
            }
            uncapped += 1;
            if grid.columns > 1 {
                assert!(
                    (grid.gutter - HANG).abs() < EPSILON,
                    "{width} px: {} columns of {} px art leave a {} px gutter, \
                     not {HANG} — that difference is dead gutter",
                    grid.columns,
                    grid.art,
                    grid.gutter
                );
            }
            assert!(
                (grid.margin - HANG).abs() < EPSILON,
                "{width} px: the block leaves a {} px margin, not {HANG}",
                grid.margin
            );
            // And the block plus its two margins is the whole width: nothing
            // is left over anywhere.
            let accounted = grid.block_width() + 2.0 * grid.margin;
            assert!(
                (accounted - width).abs() < EPSILON,
                "{width} px: {accounted} px accounted for — {} px unaccounted",
                width - accounted
            );
        }
        // Both cases have to occur in the band, or one of the two branches
        // above is being asserted about nothing.
        assert!(
            uncapped > 1000,
            "only {uncapped} uncapped widths in the band"
        );
        assert!(capped > 50, "only {capped} capped widths in the band");
    }

    /// The art stays inside the bounds the direction gives it, at every width.
    #[test]
    fn the_art_stays_between_its_floor_and_its_cap() {
        for width in band() {
            let grid = Grid::new(width);
            assert!(grid.columns >= 1, "{width} px: the grid collapsed");
            assert!(
                grid.art <= ART_MAX,
                "{width} px: {} px of art is larger than the source",
                grid.art
            );
            // `ART_MIN` is a promise about a wall wide enough to keep it: one
            // work and its two margins. Below that there is one column and it
            // is as large as the wall allows.
            if width >= ART_MIN + 2.0 * HANG {
                assert!(
                    grid.art >= ART_MIN - 0.01,
                    "{width} px: {} px of art is below ART_MIN",
                    grid.art
                );
            }
            assert!(grid.art > 0.0, "{width} px: non-positive art");
            // The row pitch is the work plus its label block plus the hang,
            // and nothing else — the grid has no padding of its own.
            assert!((grid.row_h - (grid.art + GAP_LG + LABEL_H + HANG)).abs() < f32::EPSILON);
        }
    }

    /// The nine widths `.interface-design/system.md` §7 tabulates, reproduced
    /// exactly.
    ///
    /// Written as the spec's own table rather than as whatever the code
    /// produces (ENGINEERING.md: tests are written to specification, not to
    /// implementation). Art and pitch are compared to the whole pixel the
    /// table publishes.
    #[test]
    fn the_hang_reproduces_the_specifications_table() {
        // width, columns, art, gutter, margin, row pitch
        let table = [
            (640.0_f32, 2_usize, 260.0_f32, 40.0_f32, 40.0_f32, 352.0_f32),
            (760.0, 2, 320.0, 40.0, 40.0, 412.0),
            (860.0, 2, 320.0, 80.0, 70.0, 412.0),
            (922.0, 3, 254.0, 40.0, 40.0, 346.0),
            (1120.0, 3, 320.0, 40.0, 40.0, 412.0),
            (1280.0, 4, 270.0, 40.0, 40.0, 362.0),
            (1500.0, 5, 252.0, 40.0, 40.0, 344.0),
            (1920.0, 6, 273.0, 40.0, 40.0, 365.0),
            (2560.0, 8, 275.0, 40.0, 40.0, 367.0),
        ];
        for (width, columns, art, gutter, margin, pitch) in table {
            let grid = Grid::new(width);
            assert_eq!(grid.columns, columns, "{width} px: column count");
            assert!(
                (grid.art - art).abs() < 1.0,
                "{width} px: {} px of art, table says {art}",
                grid.art
            );
            assert!((grid.gutter - gutter).abs() < 1.0, "{width} px: gutter");
            assert!((grid.margin - margin).abs() < 1.0, "{width} px: margin");
            assert!((grid.row_h - pitch).abs() < 1.0, "{width} px: row pitch");
        }
    }

    /// The column count is `floor(x + 0.5)` and the ceiling that bounds it,
    /// and neither is a language's `round` (module docs).
    #[test]
    fn the_column_count_rounds_half_up_and_never_below_one() {
        assert_eq!(Grid::new(0.0).columns, 1);
        assert_eq!(Grid::new(100.0).columns, 1);
        // Half-up, at the exact half: (w + 40) / 312 = 1.5 at w = 428.
        assert!((round_half_up(1.5) - 2.0).abs() < f32::EPSILON);
        assert!((round_half_up(-1.5) - -1.0).abs() < f32::EPSILON);
        // Monotone: a wider wall never hangs fewer works.
        let mut previous = 0;
        for width in band() {
            let columns = Grid::new(width).columns;
            assert!(
                columns >= previous,
                "{width} px: the count fell from {previous} to {columns}"
            );
            previous = columns;
        }
        // The shipped window, with and without the inspector.
        assert_eq!(Grid::new(1280.0).columns, 4);
        assert_eq!(Grid::new(1280.0 - crate::theme::PANEL_W).columns, 3);
    }

    /// The grid block is as wide as its *columns*, at every width the window
    /// and the inspector between them can produce — never as wide as the items
    /// that happen to be in a row.
    ///
    /// This is the assertion behind "filtering to one result leaves it in the
    /// first column position": the block a single tile is centred in is the
    /// same block four tiles are centred in, so the survivor does not move.
    #[test]
    fn the_grid_block_is_as_wide_as_its_columns() {
        for width in band() {
            let grid = Grid::new(width);
            let expected = grid.art.mul_add(
                #[expect(
                    clippy::cast_precision_loss,
                    reason = "a column count is far below f32's exact-integer range"
                )]
                {
                    grid.columns as f32
                },
                #[expect(
                    clippy::cast_precision_loss,
                    reason = "a column count is far below f32's exact-integer range"
                )]
                {
                    (grid.columns as f32 - 1.0) * grid.gutter
                },
            );
            assert!(
                (grid.block_width() - expected).abs() < 0.01,
                "{width} px: block {} is not {} × {} plus its gutters",
                grid.block_width(),
                grid.columns,
                grid.art
            );
            // It has to fit the viewport it is centred in, or the block itself
            // would be what forces a horizontal scrollbar.
            assert!(
                grid.block_width() <= width + 0.01,
                "{width} px: a {}-column block ({}) overflows the wall",
                grid.columns,
                grid.block_width()
            );
        }
    }

    /// The grid hold, as a timing rule: pinned for [`DOUBLE_CLICK`], and the
    /// measured width again the moment the gesture can no longer be one.
    #[test]
    fn the_grid_width_is_held_for_exactly_the_double_click_window() {
        let start = Instant::now();
        let mut hold = GridHold::default();

        // At rest the grid simply follows what it measured.
        assert!(!hold.holding());
        assert!((hold.width(940.0) - 940.0).abs() < f32::EPSILON);
        assert!(!hold.expire(start), "nothing held, nothing to expire");

        // A click on a four-column shelf that opens the inspector: the shelf
        // now measures 940, and the grid must keep laying out 1280 — the same
        // count *and* the same sleeve size.
        hold.hold(1280.0, start);
        assert!(hold.holding());
        assert!(
            (hold.width(940.0) - 1280.0).abs() < f32::EPSILON,
            "the tile must not move under the pointer"
        );
        assert_eq!(Grid::new(hold.width(940.0)).columns, 4);

        // Still held one millisecond before the window closes…
        let a_moment_early = DOUBLE_CLICK
            .checked_sub(std::time::Duration::from_millis(1))
            .expect("the hold window is longer than a millisecond");
        assert!(!hold.expire(start + a_moment_early));
        assert!((hold.width(940.0) - 1280.0).abs() < f32::EPSILON);

        // …and released exactly at it, which is the first instant a second
        // press could no longer be part of the same double-click.
        assert!(hold.expire(start + DOUBLE_CLICK));
        assert!(!hold.holding());
        assert!(
            (hold.width(940.0) - 940.0).abs() < f32::EPSILON,
            "the reflow lands once the gesture ends"
        );
        assert_eq!(Grid::new(hold.width(940.0)).columns, 3);
        // Expiring an expired hold is a no-op, so the tick can be coarse.
        assert!(!hold.expire(start + DOUBLE_CLICK));
    }

    /// The two ways a hold ends other than by timing out.
    #[test]
    fn a_hold_can_be_replaced_or_dropped_outright() {
        let start = Instant::now();
        let mut hold = GridHold::default();

        // A second click re-holds: the window is the new click's, not the
        // remains of the old one's.
        hold.hold(1280.0, start);
        hold.hold(1280.0, start + DOUBLE_CLICK);
        assert!(
            !hold.expire(start + DOUBLE_CLICK),
            "re-holding starts a fresh window rather than inheriting a spent one"
        );
        assert!((hold.width(940.0) - 1280.0).abs() < f32::EPSILON);
        assert!(hold.expire(start + DOUBLE_CLICK + DOUBLE_CLICK));

        // Released outright: the measured width wins immediately.
        hold.hold(1280.0, start);
        hold.release();
        assert!(!hold.holding());
        assert!((hold.width(760.0) - 760.0).abs() < f32::EPSILON);
    }

    /// The hold window and the double-click window are the same number,
    /// because they are the same fact (module docs).
    #[test]
    fn the_hold_window_is_the_double_click_window() {
        assert_eq!(DOUBLE_CLICK, std::time::Duration::from_millis(400));
        let start = Instant::now();
        let mut hold = GridHold::default();
        hold.hold(1280.0, start);
        // Held for every instant a second press still counts as a double
        // click, and for no instant after.
        assert!(!hold.expire(start + DOUBLE_CLICK / 2));
        assert!(hold.expire(start + DOUBLE_CLICK));
    }

    #[test]
    fn visible_rows_clamp_to_totals() {
        let grid = Grid::new(1280.0);
        // Empty shelf: nothing to render.
        assert_eq!(grid.visible_rows(0.0, 800.0, 0), (0, 0));
        // Scrolled far past the end: empty range at the end, no underflow.
        let (first, end) = grid.visible_rows(1.0e7, 800.0, 10);
        assert!(first <= end && end <= 10);
    }

    #[test]
    fn visible_rows_cover_viewport_plus_overscan() {
        let grid = Grid::new(1280.0);
        let total = 1000;
        let (first, end) = grid.visible_rows(0.0, 800.0, total);
        assert_eq!(first, 0, "top of shelf starts at row 0");
        // 800 / 362.4 = 2.21 -> ceil 3 (+1 partial) + 2x2 overscan = 8.
        assert_eq!(end, 8);

        // One viewport down: overscan reaches back above the fold.
        let (first, end) = grid.visible_rows(grid.row_h * 10.0, 800.0, total);
        assert_eq!(first, 10 - OVERSCAN_ROWS);
        assert!(end >= 10 + 4);

        // The taller row pitch shows *fewer* rows than the 284 px cell did,
        // which is the 18 % the label block and the larger art cost (ADR-0017
        // §1.4, stated and paid).
        assert!(grid.row_h > 284.0);
    }

    #[test]
    fn rows_and_spacers_are_consistent() {
        let grid = Grid::new(1920.0);
        assert_eq!(grid.columns, 6);
        assert_eq!(grid.rows(0), 0);
        assert_eq!(grid.rows(6), 1);
        assert_eq!(grid.rows(7), 2);
        assert!((grid.spacer_height(3) - 3.0 * grid.row_h).abs() < f32::EPSILON);
    }

    /// **The hang survives the index rail, at every width in the band.**
    ///
    /// The rail takes [`crate::theme::INDEX_LANE_W`] off the wall before the
    /// grid is resolved, so the grid's own arithmetic is untouched — and this
    /// is the assertion that it really is untouched rather than merely
    /// believed to be. Every claim
    /// `the_gutter_is_the_hang_wherever_the_art_is_uncapped` makes about a
    /// wall of width `w` is re-made here about a wall of width
    /// `w − INDEX_LANE_W`: gutter == HANG, margin == HANG, and nothing
    /// unaccounted for.
    #[test]
    fn the_hang_holds_with_the_index_rail_taken_off_the_wall() {
        /// f32 rounding on quantities of this magnitude.
        const EPSILON: f32 = 0.01;

        let mut uncapped = 0;
        for wall in band() {
            let width = wall - crate::theme::INDEX_LANE_W;
            if width <= 0.0 {
                continue;
            }
            let grid = Grid::new(width);
            if grid.art >= ART_MAX - EPSILON {
                continue;
            }
            uncapped += 1;
            if grid.columns > 1 {
                assert!(
                    (grid.gutter - HANG).abs() < EPSILON,
                    "{wall} px of wall ({width} px of grid): a {} px gutter, not {HANG}",
                    grid.gutter
                );
            }
            assert!(
                (grid.margin - HANG).abs() < EPSILON,
                "{wall} px of wall: a {} px margin, not {HANG}",
                grid.margin
            );
            let accounted = grid.block_width() + 2.0 * grid.margin;
            assert!(
                (accounted - width).abs() < EPSILON,
                "{wall} px of wall: {} px unaccounted",
                width - accounted
            );
        }
        assert!(
            uncapped > 1000,
            "only {uncapped} uncapped widths with the rail on"
        );
    }

    /// **The rail's lane hangs at exactly one `HANG` from the last column** —
    /// the rail is hung on the wall like a work, not bolted to its edge.
    ///
    /// The grid is resolved for `wall − INDEX_LANE_W` and centred in it, so
    /// the distance from the right edge of the last cover to the left edge of
    /// the rail's lane is the grid's own right margin, which the test above
    /// pins at `HANG`. Restated here as the thing a ruler held up to a
    /// screenshot actually measures.
    #[test]
    fn the_rail_lane_hangs_at_exactly_one_hang_from_the_last_column() {
        for wall in band() {
            let width = wall - crate::theme::INDEX_LANE_W;
            if width <= 0.0 {
                continue;
            }
            let grid = Grid::new(width);
            if grid.art >= ART_MAX - 0.01 {
                continue; // capped art: the margins take the slack (see above)
            }
            // The lane starts where the grid's width ends.
            let last_cover_right = grid.margin + grid.block_width();
            let lane_left = width;
            assert!(
                (lane_left - last_cover_right - HANG).abs() < 0.01,
                "{wall} px: {} px between the last cover and the rail's lane",
                lane_left - last_cover_right
            );
        }
    }

    /// The shelved wall's vertical rhythm, as arithmetic: the wall's top hang,
    /// then a `SHELF_HEADER_H` band and its rows per shelf, and nothing else.
    #[test]
    fn a_shelved_wall_is_a_hang_then_a_band_and_its_rows_per_shelf() {
        let grid = Grid::new(1280.0 - crate::theme::INDEX_LANE_W);
        let shelves = Shelves::new(grid, &[4, 9, 1]);
        let runs = shelves.runs();
        assert_eq!(runs.len(), 3);
        // Four columns at this width: 4 → 1 row, 9 → 3 rows, 1 → 1 row.
        assert_eq!(grid.columns, 4);
        assert_eq!(
            runs.iter().map(|run| run.rows).collect::<Vec<_>>(),
            [1, 3, 1]
        );
        // Slices of the visible list, contiguous and in order.
        assert_eq!(
            runs.iter()
                .map(|run| (run.first, run.len))
                .collect::<Vec<_>>(),
            [(0, 4), (4, 9), (13, 1)]
        );
        // The first band opens one HANG below the top of the content — the
        // wall's own top edge, the same one an unshelved wall had.
        assert!((runs[0].top - HANG).abs() < f32::EPSILON);
        // And each band opens exactly where the shelf above it ended.
        for pair in runs.windows(2) {
            assert!((pair[1].top - pair[0].end(grid)).abs() < f32::EPSILON);
        }
        // Height is the sum and nothing more: three bands, five rows, one top
        // hang. (Each row's own trailing hang is inside `row_h`, so the
        // wall's bottom edge is a hang too.)
        let expected = HANG + 3.0 * SHELF_HEADER_H + 5.0 * grid.row_h;
        assert!((shelves.height() - expected).abs() < 0.01);
        assert_eq!(shelves.albums(), 14);
    }

    /// A shelf the filter emptied is not drawn — no header with nothing under
    /// it — and the shelves that survive keep their original identity.
    #[test]
    fn an_emptied_shelf_is_not_drawn_and_the_survivors_keep_their_group() {
        let grid = Grid::new(1280.0);
        let shelves = Shelves::new(grid, &[0, 3, 0, 0, 2]);
        assert_eq!(
            shelves
                .runs()
                .iter()
                .map(|run| run.group)
                .collect::<Vec<_>>(),
            [1, 4],
            "the header a run draws is still its own"
        );
        assert_eq!(shelves.albums(), 5);
        // Nothing at all: no runs, no height beyond the wall's top edge.
        let empty = Shelves::new(grid, &[0, 0]);
        assert!(empty.runs().is_empty());
        assert_eq!(empty.run_at(0.0), None);
        assert_eq!(empty.sticky(0.0), None);
        assert_eq!(empty.visible_runs(0.0, 800.0), (0, 0));
        assert_eq!(empty.visible_albums(0.0, 800.0), (0, 0));
    }

    /// **The pinned lane holds exactly one header at every scroll offset**,
    /// and the hand-over is continuous at both ends (see [`Shelves::sticky`]).
    ///
    /// Swept at 1 px over two whole shelves rather than sampled at the
    /// boundaries, because the property being claimed is "at every offset" and
    /// the interesting offsets are single pixels either side of a hand-over.
    #[test]
    fn exactly_one_header_is_in_the_pinned_lane_at_every_offset() {
        let grid = Grid::new(1280.0 - crate::theme::INDEX_LANE_W);
        let shelves = Shelves::new(grid, &[8, 8, 8]);
        let runs = shelves.runs().to_vec();

        // Nothing is pinned while the first band is still on screen…
        assert_eq!(shelves.sticky(0.0), None);
        assert_eq!(shelves.sticky(runs[0].top), None, "the hand-over instant");
        // …and the first pixel past it pins the header it just replaced, in
        // the same place, so nothing moves across the hand-over.
        assert_eq!(shelves.sticky(runs[0].top + 1.0), Some(0));

        // The release: at the offset where the next band enters the lane.
        let release = runs[1].top - SHELF_HEADER_H;
        assert_eq!(shelves.sticky(release), Some(0));
        assert_eq!(shelves.sticky(release + 0.5), None);
        // That offset is also exactly where shelf 0's last row of covers
        // clears the top of the viewport — which is why the lane below the
        // header is clear wall rather than artwork.
        let last_row_bottom = runs[0].end(grid) - HANG;
        assert!(
            (release - last_row_bottom).abs() < f32::EPSILON,
            "the pin releases at {release} but the covers end at {last_row_bottom}"
        );

        // Sweep: at every pixel, a pinned header and an in-flow header never
        // both occupy the lane — and once the wall has scrolled far enough for
        // the first band to have reached it, one of them always does.
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a scroll offset in pixels is a small non-negative integer here"
        )]
        for step in 0..(runs[2].top as u32) {
            let scroll = f32::from(u16::try_from(step).unwrap_or(u16::MAX));
            let pinned = shelves.sticky(scroll);
            let in_lane = runs
                .iter()
                .any(|run| (run.top - scroll) >= 0.0 && (run.top - scroll) < SHELF_HEADER_H);
            assert!(
                !(pinned.is_some() && in_lane),
                "at {scroll}: {pinned:?} pinned *and* an in-flow header in the lane"
            );
            if scroll >= runs[0].top {
                assert!(
                    pinned.is_some() || in_lane,
                    "at {scroll}: the lane holds no header at all"
                );
            }
        }
        // Above the first band the lane is empty on purpose: the wall's own
        // top hang is what is there, and pinning a header over it would put
        // chrome where the wall's edge is.
        assert_eq!(shelves.sticky(runs[0].top - 1.0), None);
    }

    /// Virtualization survives shelving: only the shelves the viewport touches
    /// are built, and the albums the prefetch asks for are the ones on screen.
    #[test]
    fn only_the_shelves_the_viewport_touches_are_built() {
        let grid = Grid::new(1280.0 - crate::theme::INDEX_LANE_W);
        // Twenty shelves of a dozen albums: 60 rows, ~22 000 px of wall.
        let shelves = Shelves::new(grid, &[12; 20]);
        assert_eq!(shelves.runs().len(), 20);

        let (first, end) = shelves.visible_runs(0.0, 800.0);
        assert_eq!(first, 0);
        assert!(
            end <= 3,
            "an 800 px viewport touched {end} shelves of 20 — the wall is not virtualized"
        );

        // Scrolled into the middle: the run range is a small window, not the
        // whole wall, and the prefetch's album range sits inside it.
        let middle = shelves.runs()[10].top + 40.0;
        let (first, end) = shelves.visible_runs(middle, 800.0);
        assert!(first >= 9 && end <= 13, "{first}..{end}");
        let (start, stop) = shelves.visible_albums(middle, 800.0);
        assert!(
            start >= shelves.runs()[first].first,
            "{start} is above the first built shelf"
        );
        assert!(stop <= shelves.albums());
        assert!(
            stop - start <= 4 * 12,
            "{} albums asked for at once",
            stop - start
        );

        // Scrolled past the end: an empty window, never an underflow.
        let (first, end) = shelves.visible_runs(1.0e7, 800.0);
        assert!(first <= end && end <= 20);
    }

    /// **No artwork is ever drawn larger than its source** — the refusal, as
    /// an equation (`docs/REFUSALS.md`, `.interface-design/system.md` §1.2).
    #[test]
    fn the_wall_never_draws_art_larger_than_its_source() {
        #[expect(
            clippy::cast_precision_loss,
            reason = "a thumbnail edge in pixels is far below f32's exact-integer range"
        )]
        let source = crate::art::THUMB_PX as f32;
        assert!(
            (ART_MAX - source).abs() < f32::EPSILON,
            "ART_MAX is {ART_MAX} and the thumbnail is {source}: the wall would \
             upscale a cover, which is the one thing the cache size exists to \
             prevent"
        );
        for width in band() {
            assert!(Grid::new(width).art <= source);
        }
        // ART_MAX = 4/3 x ART_MIN, so the art hands off from its largest to
        // its smallest at exactly one width per column transition.
        assert!((ART_MAX - 4.0 / 3.0 * ART_MIN).abs() < f32::EPSILON);
    }
}
