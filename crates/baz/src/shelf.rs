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
//! # What left with the inspector
//!
//! `GridHold` used to live here: a tile click opened the album inspector, which
//! took 340 px off the shelf and re-hung every sleeve *between the two presses
//! of a double-click*, so the width in force was pinned for 400 ms while the
//! gesture finished. ADR-0022 deleted the inspector, and with it the only thing
//! in the product that could re-hang the wall in answer to a press — **the
//! grid's width is a function of the window and nothing else now**. There is no
//! reflow left to defer, so there is no hold, no `DOUBLE_CLICK` window and no
//! clock ticking behind a gesture.
//!
//! # Density is the hang's four numbers, not a fifth number beside them
//!
//! ADR-0017 step 6. [`Density`] does not *override* anything above: it
//! supplies `HANG`, `ART_MIN`, `ART_TARGET` and `ART_MAX`, and the arithmetic
//! is then the same arithmetic. That is the whole of why the properties
//! survive the step — `gutter == margin == hang` wherever the art is uncapped
//! is an algebraic consequence of the formula, and the formula did not change,
//! so it is true at three steps for the same reason it was true at one.

use crate::theme::{GAP_LG, HANG, LABEL_H};

/// **The token and the default band are one number.** The wall reads its
/// header band off [`Grid::header_h`] now, because the band is the density's
/// hang; [`crate::theme::SHELF_HEADER_H`] remains the value the type scale was
/// derived against and the one a reviewer measures against a screenshot of the
/// default. They may not drift, and this is where they cannot.
const _: () = assert!(crate::theme::SHELF_HEADER_H == HANG);

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

/// `floor(x + 0.5)` — half-up rounding, spelled out (module docs).
fn round_half_up(value: f32) -> f32 {
    (value + 0.5).floor()
}

/// **How closely the wall hangs its works** — three named steps, and the
/// four numbers each one gives [`Grid`] (`.interface-design/system.md` §7.1,
/// ADR-0017 step 6).
///
/// # Three steps, not a slider
///
/// `03-interface-prior-art.md` R7: density control is universal outside music
/// — Lightroom, Calibre, Steam, Plex, Google Photos all ship one — and **two
/// products that removed one took durable damage**. Under a direction that
/// deliberately shows fewer, larger covers this matters more rather than less:
/// 300 albums and 40 000 albums do not want the same wall.
///
/// It is three *named* steps and not a free zoom because a slider makes every
/// screenshot of baz different and every layout bug unreproducible. Three is
/// also the number that can be spent from a keyboard without a readout: a step
/// either side of the default, and you are never more than two presses from
/// any of them.
///
/// # A control in the place's body, not a setting
///
/// ADR-0017 §1.3 takes the better half of the critique's argument — *Settings
/// must never be the answer to a **view** question* — and supersedes
/// `02` §2.7's placement in Settings → Appearance. The step persists in
/// `config.toml` as *state*, the way the group key does, rather than as a
/// preference somebody goes somewhere to set, and there is still no density
/// row and no zoom readout.
///
/// The visible control is **three detent marks at the foot of the index
/// rail's lane** (ADR-0028, doc 11 §5 P8 — the owner's choice): density
/// reads the viewport, so its home is the place's body (doc 07 L8.1), and
/// the lane is the body's one resident view-subject strip.
/// <kbd>Ctrl</kbd>+<kbd>-</kbd> / <kbd>Ctrl</kbd>+<kbd>=</kbd> and
/// <kbd>Ctrl</kbd>+scroll remain as the marks' accelerators, sending the
/// same [`crate::app::Message::DensityStep`] a mark's press sends
/// ([`Self::steps_to`] — the mirror rule, L8.7).
///
/// # Why these four numbers per step
///
/// [`Density::Balanced`] **is** the shipped hang, token for token — asserted
/// below — so the default wall is the wall that was measured, and the two
/// other steps are the same wall zoomed rather than three walls that happen to
/// share a formula.
///
/// - **Spacious** raises `ART_MIN` to 288 and pins `ART_TARGET` at `ART_MAX`,
///   so above about 1100 px of grid the art is capped and the *margins* take
///   the slack. That is the one case in which `gutter > hang`, and it is the
///   case in which the alternative would be upscaling a thumbnail.
/// - **Dense** is today's shelf: at the shipped 1280 px window the wall is
///   1172 px wide once the rail's lane is off it, and Dense hangs
///   **5 × 200.8** there against the 5 × 208 baz drew before the hang landed.
///   Nobody loses what they have by the default moving.
///
/// `ART_MAX` never exceeds [`crate::art::THUMB_PX`] at any step, so *nothing
/// upscales* is a property of the system rather than of the default —
/// `the_wall_never_draws_art_larger_than_its_source` sweeps all three.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Density {
    /// The largest works and the widest hang: `HANG` 48, art 288 … 320.
    Spacious,
    /// The default, and the hang `theme.rs` publishes: `HANG` 40, art
    /// 240 … 320.
    #[default]
    Balanced,
    /// The most works on screen: `HANG` 28, art 176 … 240 — the shelf baz
    /// shipped before density existed.
    Dense,
}

impl Density {
    /// The steps, **loosest first** — the order the zoom travels in, so
    /// `ALL[0]` is what <kbd>Ctrl</kbd>+<kbd>=</kbd> walks towards.
    ///
    /// Written once and read by [`Self::step`], [`Self::from_code`] and every
    /// sweep in the tests, so a fourth step is one variant and one row here.
    pub const ALL: [Self; 3] = [Self::Spacious, Self::Balanced, Self::Dense];

    /// The word this step is written as in `config.toml`.
    ///
    /// A stable lowercase word rather than an index, for `group_key`'s reason:
    /// the file is meant to be read, and a step added or reordered here must
    /// not silently re-hang somebody's wall.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Spacious => "spacious",
            Self::Balanced => "balanced",
            Self::Dense => "dense",
        }
    }

    /// The step a config document names, or `None` for a word this build does
    /// not know.
    ///
    /// `None` rather than a guess: the caller defaults it, and defaulting is
    /// the config module's per-key degradation rule rather than this module's
    /// opinion.
    #[must_use]
    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|step| step.code() == code)
    }

    /// The step's plain name, for a log line and for the tests' messages.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Spacious => "Spacious",
            Self::Balanced => "Balanced",
            Self::Dense => "Dense",
        }
    }

    /// Work-to-work *and* work-to-wall-edge, at this step (logical px).
    #[must_use]
    pub const fn hang(self) -> f32 {
        match self {
            Self::Spacious => 48.0,
            Self::Balanced => HANG,
            Self::Dense => 28.0,
        }
    }

    /// The smallest work this step will hang (logical px).
    #[must_use]
    pub const fn art_min(self) -> f32 {
        match self {
            Self::Spacious => 288.0,
            Self::Balanced => crate::theme::ART_MIN,
            Self::Dense => 176.0,
        }
    }

    /// The work size the column count aims at (logical px).
    #[must_use]
    pub const fn art_target(self) -> f32 {
        match self {
            Self::Spacious => 320.0,
            Self::Balanced => crate::theme::ART_TARGET,
            Self::Dense => 200.0,
        }
    }

    /// The largest work this step will hang (logical px), never above the
    /// thumbnail the cache holds.
    #[must_use]
    pub const fn art_max(self) -> f32 {
        match self {
            Self::Spacious => 320.0,
            Self::Balanced => crate::theme::ART_MAX,
            Self::Dense => 240.0,
        }
    }

    /// One press of the zoom: `+1` loosens the hang, `-1` tightens it.
    ///
    /// **Saturating, never wrapping.** A zoom that has run out does nothing,
    /// which is what every zoom a listener has ever used does; wrapping would
    /// mean the same key that has been enlarging the covers suddenly shrinks
    /// them to the smallest step, with no readout on screen to explain it.
    #[must_use]
    pub fn step(self, delta: i32) -> Self {
        let here = Self::ALL.iter().position(|step| *step == self).unwrap_or(1);
        // `ALL` is loosest-first, so a *positive* delta walks towards index 0.
        let there = i64::try_from(here).unwrap_or(1) - i64::from(delta);
        let last = i64::try_from(Self::ALL.len() - 1).unwrap_or(0);
        let index = usize::try_from(there.clamp(0, last)).unwrap_or(0);
        Self::ALL.get(index).copied().unwrap_or(self)
    }

    /// The [`Self::step`] delta that lands on `target` from here — what a
    /// density mark's press sends (ADR-0028).
    ///
    /// The marks mirror the gesture rather than growing a message of their
    /// own: a press on `target`'s mark is `DensityStep(steps_to)`, which is
    /// exactly the signed number of gesture notches between the two steps,
    /// so one press of a mark and |delta| presses of the key are the same
    /// walk (the mirror rule, doc 07 L8.7 — pinned by
    /// `a_marks_delta_is_the_gestures_own_notches`). Zero from a step to
    /// itself, which is why the active mark is inert rather than wired: a
    /// message that does nothing is not sent.
    #[must_use]
    pub fn steps_to(self, target: Self) -> i32 {
        let position = |step: Self| {
            i32::try_from(Self::ALL.iter().position(|s| *s == step).unwrap_or(1)).unwrap_or(1)
        };
        // `step` walks a positive delta towards index 0 (loosest), so the
        // delta that lands on `target` is here − there.
        position(self) - position(target)
    }
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
    /// Which step of the zoom hung it (module docs, [`Density`]).
    pub density: Density,
    /// Work-to-work *and* work-to-wall-edge, at this step: the one number the
    /// grid is driven by, and [`Density::hang`] is where it comes from.
    pub hang: f32,
    /// Columns the wall hangs, at least 1.
    pub columns: usize,
    /// Edge of one work (logical px), never above the step's `ART_MAX`.
    pub art: f32,
    /// Work-to-work gap (logical px). Exactly [`Self::hang`] whenever the art
    /// is uncapped, and at most `2 × hang` when it is.
    pub gutter: f32,
    /// Work-to-wall-edge gap (logical px), the block being centred.
    pub margin: f32,
    /// Row pitch (logical px): the work, the gap to its label, the label, and
    /// the hang to the row below.
    pub row_h: f32,
}

impl Grid {
    /// Resolve the hang for a grid of `width` logical pixels at `density`.
    ///
    /// `width` is the width the *shelf* has — the window less the inspector
    /// and the index rail's lane — not the window's. The scrollbar overlays
    /// the right margin rather than taking width from the block, which it can
    /// do without clipping anything: the margin is the step's hang and the
    /// bar's lane is 10.
    #[must_use]
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        reason = "column counts are small non-negative integers, far below \
                  f32's exact-integer range, and every quotient floored here \
                  is finite and clamped"
    )]
    pub fn new(width: f32, density: Density) -> Self {
        let width = width.max(0.0);
        let (hang, art_min, art_target, art_max) = (
            density.hang(),
            density.art_min(),
            density.art_target(),
            density.art_max(),
        );
        // The count the wall wants, and the count the smallest acceptable work
        // allows. The second is a ceiling rather than a preference: a window
        // gains a column only when the column it gains is still worth looking
        // at.
        let wanted = round_half_up((width + hang) / (art_target + hang));
        let ceiling = ((width - hang) / (art_min + hang)).floor().max(1.0);
        let columns = wanted.clamp(1.0, ceiling).max(1.0) as usize;

        let count = columns as f32;
        let art = ((width - (count + 1.0) * hang) / count).clamp(ART_FLOOR, art_max);
        let gutter = if columns > 1 {
            ((width - 2.0 * hang - count * art) / (count - 1.0)).clamp(0.0, 2.0 * hang)
        } else {
            0.0
        };
        let block = count * art + (count - 1.0) * gutter;
        Self {
            width,
            density,
            hang,
            columns,
            art,
            gutter,
            margin: ((width - block) / 2.0).max(0.0),
            row_h: art + GAP_LG + LABEL_H + hang,
        }
    }

    /// A shelf's header band, at this step: **one hang**, exactly as
    /// [`crate::theme::SHELF_HEADER_H`] is one `HANG` at the default.
    ///
    /// It zooms with the works rather than staying at 40, and it has to: the
    /// band being the same number as a row's trailing gap is what makes the
    /// pinned header's hand-over exact (see [`Shelves::sticky`]), and two
    /// numbers that had drifted apart would put a gap or an overlap in the
    /// pinned lane at every step but one.
    #[must_use]
    pub fn header_h(self) -> f32 {
        self.hang
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
    ///
    /// It takes the grid because the band is one hang and the hang is the
    /// density's (see [`Grid::header_h`]) — a shelf laid out at one step and
    /// measured at another would be the one thing this module exists to make
    /// impossible.
    #[must_use]
    pub fn rows_top(self, grid: Grid) -> f32 {
        self.top + grid.header_h()
    }

    /// One past the bottom of this shelf — the top of the next shelf's band,
    /// or the bottom of the wall.
    #[must_use]
    #[expect(
        clippy::cast_precision_loss,
        reason = "row counts are far below f32's 2^24 exact-integer range"
    )]
    pub fn end(self, grid: Grid) -> f32 {
        self.rows_top(grid) + self.rows as f32 * grid.row_h
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
/// The wall's own top hang, then, per shelf, a band of exactly one hang
/// ([`Grid::header_h`]) and then its rows at the grid's pitch:
///
/// ```text
/// hang                                   the wall's top edge
/// ┌ header_h = hang                      the header band
/// │   HEADING_LINE_H = 12                  the header's line box, at its top
/// │   hang − 12                             clear wall
/// └ rows × row_h                         the covers; each row_h ends in a hang
/// ```
///
/// Every number is the density's hang or derived from it, so a shelf break
/// costs the wall exactly one more hang than a row break does and the whole
/// page keeps one vertical unit — at every step, because the step supplies the
/// unit rather than sitting beside it. At the default the gap a reader sees
/// above a header is the previous row's trailing hang (40) and the gap below
/// it is `HANG − HEADING_LINE_H` (28) — a header nearer the shelf it names
/// than the one it follows, and that ratio is the same at all three steps.
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
        // The wall's own top edge is one hang, the same hang its left and
        // right edges are — so the works hang from one number in both axes at
        // every density, not from one number horizontally and 40 vertically.
        let mut top = grid.hang;
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
    /// - The pin is released at `scroll == next.top − header_h`, and
    ///   since a row's pitch ends in a hang and the band *is* a hang, that
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
            Some(next) if scroll > next.top - self.grid.header_h() => None,
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
                    .visible_rows(scroll - run.rows_top(self.grid), viewport_height, run.rows);
            let columns = self.grid.columns.max(1);
            start = start.min(run.first + (row * columns).min(run.len));
            end = end.max(run.first + (row_end * columns).min(run.len));
        }
        if start > end { (0, 0) } else { (start, end) }
    }
}

#[cfg(test)]
mod tests {
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
    /// width — and now at every density.**
    ///
    /// Whenever the art is not capped at the step's `ART_MAX`, the gutter is
    /// exactly the step's hang *and* the margin is exactly the step's hang —
    /// work-to-work and work-to-wall are the same number, and every spare
    /// pixel is inside the artwork rather than pooled at the edges (module
    /// docs prove it algebraically; this is the same claim over the real f32
    /// arithmetic).
    ///
    /// Extended over [`Density::ALL`] rather than replaced by a density
    /// version, because the claim is *the same claim*: density supplies the
    /// four numbers and the formula is untouched, so if the property held for
    /// one triple and fails for another, the step is what broke it.
    ///
    /// The tolerance is 0.01 px, which is f32's rounding on numbers this size
    /// and not a design allowance: the algebra is exact.
    #[test]
    fn the_gutter_is_the_hang_wherever_the_art_is_uncapped() {
        /// f32 rounding on quantities of this magnitude, not a design
        /// allowance.
        const EPSILON: f32 = 0.01;

        for density in Density::ALL {
            let (hang, art_max) = (density.hang(), density.art_max());
            let step = density.label();
            let mut uncapped = 0;
            let mut capped = 0;
            for width in band() {
                let grid = Grid::new(width, density);
                assert!(
                    (grid.hang - hang).abs() < f32::EPSILON,
                    "{step}: the grid took a {} px hang, not {hang}",
                    grid.hang
                );
                if grid.art >= art_max - EPSILON {
                    capped += 1;
                    // The one case gutter may exceed the hang: the art has
                    // stopped growing, so the margins take the slack and the
                    // gutter rises with them — never past 2 × hang, and never
                    // below it.
                    if grid.columns > 1 {
                        assert!(
                            grid.gutter >= hang - EPSILON && grid.gutter <= 2.0 * hang + EPSILON,
                            "{step} at {width} px: capped art with a {} px gutter",
                            grid.gutter
                        );
                    }
                    continue;
                }
                uncapped += 1;
                if grid.columns > 1 {
                    assert!(
                        (grid.gutter - hang).abs() < EPSILON,
                        "{step} at {width} px: {} columns of {} px art leave a {} px \
                         gutter, not {hang} — that difference is dead gutter",
                        grid.columns,
                        grid.art,
                        grid.gutter
                    );
                }
                assert!(
                    (grid.margin - hang).abs() < EPSILON,
                    "{step} at {width} px: the block leaves a {} px margin, not {hang}",
                    grid.margin
                );
                // And the block plus its two margins is the whole width:
                // nothing is left over anywhere.
                let accounted = grid.block_width() + 2.0 * grid.margin;
                assert!(
                    (accounted - width).abs() < EPSILON,
                    "{step} at {width} px: {accounted} px accounted for — {} px unaccounted",
                    width - accounted
                );
            }
            // Both cases have to occur at every step, or one of the two
            // branches above is being asserted about nothing. Spacious pins
            // its art at `ART_MAX` over most of the band by design, so the
            // floor is the count Spacious actually reaches rather than
            // Balanced's.
            assert!(
                uncapped > 900,
                "{step}: only {uncapped} uncapped widths in the band"
            );
            assert!(
                capped > 50,
                "{step}: only {capped} capped widths in the band"
            );
        }
    }

    /// The art stays inside the bounds the direction gives it, at every width
    /// and every step.
    #[test]
    fn the_art_stays_between_its_floor_and_its_cap() {
        for density in Density::ALL {
            let (hang, art_min, art_max) = (density.hang(), density.art_min(), density.art_max());
            let step = density.label();
            for width in band() {
                let grid = Grid::new(width, density);
                assert!(
                    grid.columns >= 1,
                    "{step} at {width} px: the grid collapsed"
                );
                assert!(
                    grid.art <= art_max,
                    "{step} at {width} px: {} px of art is larger than the source",
                    grid.art
                );
                // `ART_MIN` is a promise about a wall wide enough to keep it:
                // one work and its two margins. Below that there is one column
                // and it is as large as the wall allows.
                if width >= art_min + 2.0 * hang {
                    assert!(
                        grid.art >= art_min - 0.01,
                        "{step} at {width} px: {} px of art is below its floor",
                        grid.art
                    );
                }
                assert!(grid.art > 0.0, "{step} at {width} px: non-positive art");
                // The row pitch is the work plus its label block plus the
                // hang, and nothing else — the grid has no padding of its own.
                assert!((grid.row_h - (grid.art + GAP_LG + LABEL_H + hang)).abs() < f32::EPSILON);
            }
        }
    }

    /// **The three steps, at the widths the shipped windows actually give the
    /// wall** — `.interface-design/system.md` §7.1's table, recomputed from
    /// the specification's own formula and its four numbers per step.
    ///
    /// The widths are the *grid's*, not the window's: a 1280 px window with no
    /// inspector leaves 1172 px once the index rail's
    /// [`crate::theme::INDEX_LANE_W`] 108 is off it, and 1920 leaves 1812.
    /// §7.1's published table predated the rail's lane and is corrected by
    /// this test rather than reproduced by it — the same correction §7's own
    /// "Built" note records for the shipped Balanced row.
    ///
    /// The numbers come from the formula in §7 applied to §7.1's steps, not
    /// from running the code and writing down what it said (ENGINEERING.md:
    /// tests are written to specification, not to implementation).
    #[test]
    fn the_density_steps_reproduce_the_specifications_table() {
        // step, grid width, columns, art, gutter, margin.
        //
        // 1172 = 1280 − INDEX_LANE_W and 1812 = 1920 − INDEX_LANE_W: the wall
        // the shipped window and a 1920 monitor hang, inspector closed.
        let table = [
            (
                Density::Spacious,
                1172.0_f32,
                3_usize,
                320.0_f32,
                58.0_f32,
                48.0_f32,
            ),
            (Density::Spacious, 1812.0, 5, 304.8, 48.0, 48.0),
            (Density::Balanced, 1172.0, 4, 243.0, 40.0, 40.0),
            (Density::Balanced, 1812.0, 6, 255.33, 40.0, 40.0),
            (Density::Dense, 1172.0, 5, 200.8, 28.0, 28.0),
            (Density::Dense, 1812.0, 8, 195.0, 28.0, 28.0),
        ];
        for (density, width, columns, art, gutter, margin) in table {
            let grid = Grid::new(width, density);
            let step = density.label();
            assert_eq!(grid.columns, columns, "{step} at {width} px: column count");
            assert!(
                (grid.art - art).abs() < 0.5,
                "{step} at {width} px: {} px of art, the table says {art}",
                grid.art
            );
            assert!(
                (grid.gutter - gutter).abs() < 0.5,
                "{step} at {width} px: {} px gutter, the table says {gutter}",
                grid.gutter
            );
            assert!(
                (grid.margin - margin).abs() < 0.5,
                "{step} at {width} px: {} px margin, the table says {margin}",
                grid.margin
            );
        }
        // **Dense is today's shelf.** Before the hang landed the wall drew a
        // fixed 240 px cell with 208 px of art in it — five columns at the
        // shipped window — so the step that exists so nobody loses what they
        // have has to still hang five, at very nearly that size.
        let today = Grid::new(1172.0, Density::Dense);
        assert_eq!(today.columns, 5);
        assert!(
            (today.art - 208.0).abs() < 8.0,
            "Dense hangs {} px of art where the shipped shelf drew 208",
            today.art
        );
    }

    /// **A denser step never hangs fewer works, at any width.** The one
    /// ordering claim the three steps make to a listener: pressing towards
    /// Dense puts *more* records on screen, always, and pressing back puts
    /// fewer.
    ///
    /// The art is deliberately **not** asserted to be monotone with it, and
    /// that is not an omission: at 1120 px Spacious hangs 3 × 309.3 while
    /// Balanced hangs 3 × 320, because Balanced's art is capped there and
    /// Spacious's is not. Fewer columns is the promise; larger art is its
    /// usual consequence and not its definition.
    #[test]
    fn a_tighter_step_never_hangs_fewer_works() {
        for width in band() {
            let counts: Vec<usize> = Density::ALL
                .into_iter()
                .map(|density| Grid::new(width, density).columns)
                .collect();
            assert!(
                counts.windows(2).all(|pair| pair[0] <= pair[1]),
                "{width} px: Spacious/Balanced/Dense hang {counts:?} columns"
            );
        }
    }

    /// **Balanced is the shipped hang, token for token.** The default step is
    /// not a fourth set of numbers that happens to look like `theme.rs`'s — it
    /// *is* them, so every measurement taken of the wall before density
    /// existed still describes the wall a listener meets.
    #[test]
    fn balanced_is_the_hang_the_tokens_publish() {
        const { assert!(Density::Balanced.hang() == HANG) }
        const { assert!(Density::Balanced.art_min() == crate::theme::ART_MIN) }
        const { assert!(Density::Balanced.art_target() == crate::theme::ART_TARGET) }
        const { assert!(Density::Balanced.art_max() == crate::theme::ART_MAX) }
        // …and the header band is one hang at every step, which is what
        // `theme::SHELF_HEADER_H` says at the default.
        const { assert!(crate::theme::SHELF_HEADER_H == HANG) }
        for density in Density::ALL {
            let grid = Grid::new(1172.0, density);
            assert!((grid.header_h() - density.hang()).abs() < f32::EPSILON);
        }
        assert_eq!(Density::default(), Density::Balanced);
    }

    /// The zoom itself: one press a step, saturating at both ends, and the
    /// word each step is written as in the config document.
    #[test]
    fn the_zoom_steps_one_stop_at_a_time_and_stops_at_the_ends() {
        use Density::{Balanced, Dense, Spacious};

        assert_eq!(Balanced.step(1), Spacious, "Ctrl+= loosens the hang");
        assert_eq!(Balanced.step(-1), Dense, "Ctrl+- tightens it");
        // Saturating, never wrapping: a zoom that has run out does nothing.
        assert_eq!(Spacious.step(1), Spacious);
        assert_eq!(Dense.step(-1), Dense);
        // …including for a delta larger than the ladder, and for no delta.
        assert_eq!(Dense.step(9), Spacious);
        assert_eq!(Spacious.step(-9), Dense);
        for density in Density::ALL {
            assert_eq!(density.step(0), density);
            // A step out and back is where you were, everywhere it is not an
            // end.
            if density != Spacious {
                assert_eq!(density.step(1).step(-1), density);
            }
            if density != Dense {
                assert_eq!(density.step(-1).step(1), density);
            }
            // The word round-trips, and it is a word rather than an index.
            assert_eq!(Density::from_code(density.code()), Some(density));
            assert!(density.code().chars().all(|c| c.is_ascii_lowercase()));
        }
        // A word this build does not know is `None`, for the caller to
        // default — not a guess, and never a panic.
        for unknown in ["", "COMPACT", "balanced ", "2", "spacious!"] {
            assert_eq!(Density::from_code(unknown), None, "{unknown:?}");
        }
        // The ladder is loosest-first, which is what makes `step`'s sign the
        // direction a listener presses in.
        assert_eq!(Density::ALL, [Spacious, Balanced, Dense]);
    }

    /// **A mark's delta is the gesture's own notches** (ADR-0028): for every
    /// pair of steps, one `DensityStep(steps_to)` lands exactly where |delta|
    /// presses of the ±1 gesture land, and the sign is the direction the
    /// gesture would press in. The detent control and the zoom are one
    /// control spelled twice, by arithmetic rather than by promise.
    #[test]
    fn a_marks_delta_is_the_gestures_own_notches() {
        for here in Density::ALL {
            for target in Density::ALL {
                let delta = here.steps_to(target);
                // One press of the mark…
                assert_eq!(
                    here.step(delta),
                    target,
                    "{} → {}: DensityStep({delta}) misses",
                    here.label(),
                    target.label()
                );
                // …is the same walk as |delta| notches of the gesture.
                let mut walked = here;
                for _ in 0..delta.unsigned_abs() {
                    walked = walked.step(delta.signum());
                }
                assert_eq!(
                    walked,
                    target,
                    "{} → {}: {delta} is not the gesture's own notch count",
                    here.label(),
                    target.label()
                );
                // A mark never needs more presses than the ladder has rungs.
                assert!(delta.unsigned_abs() < u32::try_from(Density::ALL.len()).unwrap_or(0));
            }
            // The step you are on is delta zero — the inert mark's reason.
            assert_eq!(here.steps_to(here), 0);
        }
    }

    /// The nine widths `.interface-design/system.md` §7 tabulates, reproduced
    /// exactly at the default step.
    ///
    /// Written as the spec's own table rather than as whatever the code
    /// produces (ENGINEERING.md: tests are written to specification, not to
    /// implementation). Art and pitch are compared to the whole pixel the
    /// table publishes.
    #[test]
    fn the_hang_reproduces_the_specifications_table() {
        // width, columns, art, gutter, margin, row pitch.
        //
        // The pitch is `art + 96` at every width now — `GAP_LG` to the label,
        // `LABEL_H` 40, `HANG` 40 — because quantising the body's line box to 20
        // makes a wall label exactly one hang tall (composition audit §2.1). It
        // was `art + 92.4`, which is the same table with a fraction in it.
        let table = [
            (640.0_f32, 2_usize, 260.0_f32, 40.0_f32, 40.0_f32, 356.0_f32),
            (760.0, 2, 320.0, 40.0, 40.0, 416.0),
            (860.0, 2, 320.0, 80.0, 70.0, 416.0),
            (922.0, 3, 254.0, 40.0, 40.0, 350.0),
            (1120.0, 3, 320.0, 40.0, 40.0, 416.0),
            (1280.0, 4, 270.0, 40.0, 40.0, 366.0),
            (1500.0, 5, 252.0, 40.0, 40.0, 348.0),
            (1920.0, 6, 273.0, 40.0, 40.0, 369.0),
            (2560.0, 8, 275.0, 40.0, 40.0, 371.0),
        ];
        for (width, columns, art, gutter, margin, pitch) in table {
            let grid = Grid::new(width, Density::Balanced);
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
        assert_eq!(Grid::new(0.0, Density::Balanced).columns, 1);
        assert_eq!(Grid::new(100.0, Density::Balanced).columns, 1);
        // Half-up, at the exact half: (w + 40) / 312 = 1.5 at w = 428.
        assert!((round_half_up(1.5) - 2.0).abs() < f32::EPSILON);
        assert!((round_half_up(-1.5) - -1.0).abs() < f32::EPSILON);
        // Monotone: a wider wall never hangs fewer works.
        let mut previous = 0;
        for width in band() {
            let columns = Grid::new(width, Density::Balanced).columns;
            assert!(
                columns >= previous,
                "{width} px: the count fell from {previous} to {columns}"
            );
            previous = columns;
        }
        // The shipped window, and the wall with the index rail's lane off it —
        // the two widths the collection is actually hung at (ADR-0022 left the
        // wall's width a function of the window and the rail alone).
        assert_eq!(Grid::new(1280.0, Density::Balanced).columns, 4);
        assert_eq!(
            Grid::new(1280.0 - crate::theme::INDEX_LANE_W, Density::Balanced).columns,
            4
        );
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
            let grid = Grid::new(width, Density::Balanced);
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

    #[test]
    fn visible_rows_clamp_to_totals() {
        let grid = Grid::new(1280.0, Density::Balanced);
        // Empty shelf: nothing to render.
        assert_eq!(grid.visible_rows(0.0, 800.0, 0), (0, 0));
        // Scrolled far past the end: empty range at the end, no underflow.
        let (first, end) = grid.visible_rows(1.0e7, 800.0, 10);
        assert!(first <= end && end <= 10);
    }

    #[test]
    fn visible_rows_cover_viewport_plus_overscan() {
        let grid = Grid::new(1280.0, Density::Balanced);
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
        let grid = Grid::new(1920.0, Density::Balanced);
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

        for density in Density::ALL {
            let (hang, art_max, step) = (density.hang(), density.art_max(), density.label());
            let mut uncapped = 0;
            for wall in band() {
                let width = wall - crate::theme::INDEX_LANE_W;
                if width <= 0.0 {
                    continue;
                }
                let grid = Grid::new(width, density);
                if grid.art >= art_max - EPSILON {
                    continue;
                }
                uncapped += 1;
                if grid.columns > 1 {
                    assert!(
                        (grid.gutter - hang).abs() < EPSILON,
                        "{step}, {wall} px of wall ({width} px of grid): a {} px gutter, \
                         not {hang}",
                        grid.gutter
                    );
                }
                assert!(
                    (grid.margin - hang).abs() < EPSILON,
                    "{step}, {wall} px of wall: a {} px margin, not {hang}",
                    grid.margin
                );
                let accounted = grid.block_width() + 2.0 * grid.margin;
                assert!(
                    (accounted - width).abs() < EPSILON,
                    "{step}, {wall} px of wall: {} px unaccounted",
                    width - accounted
                );
            }
            assert!(
                uncapped > 900,
                "{step}: only {uncapped} uncapped widths with the rail on"
            );
        }
    }

    /// **The rail's lane hangs at exactly one hang from the last column** —
    /// the rail is hung on the wall like a work, not bolted to its edge, at
    /// every step.
    ///
    /// The grid is resolved for `wall − INDEX_LANE_W` and centred in it, so
    /// the distance from the right edge of the last cover to the left edge of
    /// the rail's lane is the grid's own right margin, which the test above
    /// pins at the step's hang. Restated here as the thing a ruler held up to
    /// a screenshot actually measures.
    ///
    /// The lane's *own* right gutter is `theme::HANG` at every step and does
    /// not zoom, because it is a **window** edge and law L1 gives every
    /// window-edge surface one gutter (`theme::one_gutter_touches_every_
    /// window_edge`). The zoom is of the works; the room the works hang in is
    /// the same room.
    #[test]
    fn the_rail_lane_hangs_at_exactly_one_hang_from_the_last_column() {
        for density in Density::ALL {
            let (hang, art_max, step) = (density.hang(), density.art_max(), density.label());
            for wall in band() {
                let width = wall - crate::theme::INDEX_LANE_W;
                if width <= 0.0 {
                    continue;
                }
                let grid = Grid::new(width, density);
                if grid.art >= art_max - 0.01 {
                    continue; // capped art: the margins take the slack (above)
                }
                // The lane starts where the grid's width ends.
                let last_cover_right = grid.margin + grid.block_width();
                let lane_left = width;
                assert!(
                    (lane_left - last_cover_right - hang).abs() < 0.01,
                    "{step}, {wall} px: {} px between the last cover and the rail's lane",
                    lane_left - last_cover_right
                );
            }
        }
    }

    /// **The wall's right-hand lanes, at every width in the band** — the
    /// arrangement the owner's *"scroll bar is in a strange location… it seems
    /// to have padding on the right"* moved the bar into.
    ///
    /// Left to right, off the wall's own right edge (`crate::views::shelf`):
    ///
    /// ```text
    /// … covers │ margin │ INDEX_CLEARANCE │ the rail's ink │ HANG │
    ///          │        └──── theme::INDEX_LANE_W ───────────────┘
    ///          │                                        the bar ┤▌
    /// ```
    ///
    /// The bar is drawn on the **window's** edge, in the outer
    /// [`crate::theme::WALL_SCROLLBAR_W`] of the rail's own window gutter — so
    /// the four claims below have to hold together at every pixel, and three
    /// of them are new with the move:
    ///
    /// 1. the grid is resolved for the wall less [`crate::theme::WALL_RESERVE`]
    ///    — the bar's lane *and* the rail's, one number;
    /// 2. **no cover is ever under the bar**, by the whole rail lane's width;
    /// 3. the bar is outboard of the rail's ink rather than inboard of it,
    ///    which is the defect's whole content;
    /// 4. the rail's ink still ends on `W − HANG` (law L1) — the bar moved,
    ///    the type did not.
    #[test]
    fn the_bar_is_outboard_of_the_rails_ink_at_every_width() {
        use crate::theme::{HANG, INDEX_LANE_W, WALL_RESERVE, WALL_SCROLLBAR_W};

        // The reservation is one number and it is the two lanes.
        const { assert!(WALL_RESERVE == INDEX_LANE_W + WALL_SCROLLBAR_W) }

        for density in Density::ALL {
            let step = density.label();
            for wall in band() {
                let width = wall - WALL_RESERVE;
                if width <= 0.0 {
                    continue;
                }
                let grid = Grid::new(width, density);
                // 1. The grid never sees the reserved lanes.
                assert!(
                    (grid.width - width).abs() < 0.01,
                    "{step}, {wall} px of wall: the grid was resolved for {} px",
                    grid.width
                );

                // The three x-positions the frame is measured at, in the
                // wall's own coordinates.
                let last_cover_right = grid.margin + grid.block_width();
                let ink_right = wall - HANG;
                let bar_left = wall - WALL_SCROLLBAR_W;

                // 2. No cover is ever drawn under the bar — and the slack is
                //    not a hair, it is the whole of the rail's lane.
                assert!(
                    bar_left - last_cover_right >= INDEX_LANE_W - 0.01,
                    "{step}, {wall} px of wall: only {} px between the last \
                     cover and the bar",
                    bar_left - last_cover_right
                );

                // 3. The bar is outboard of the rail's ink. This is the
                //    inversion the owner saw: it used to be inboard by
                //    `INDEX_LANE_W`, which is the empty strip he described.
                assert!(
                    bar_left >= ink_right,
                    "{step}, {wall} px of wall: the bar at {bar_left} is inboard \
                     of the rail's ink, which ends at {ink_right}"
                );

                // 4. …and the ink did not move to let it past: law L1's one
                //    window gutter, at every width and every step.
                assert!(
                    (wall - ink_right - HANG).abs() < 0.01,
                    "{step}, {wall} px of wall: the rail's ink hangs from {ink_right}"
                );
            }
        }
    }

    /// The shelved wall's vertical rhythm, as arithmetic: the wall's top hang,
    /// then a one-hang band and its rows per shelf, and nothing else — at
    /// every step, because the wall's vertical unit is the step's hang and not
    /// a token beside it.
    #[test]
    fn a_shelved_wall_is_a_hang_then_a_band_and_its_rows_per_shelf() {
        let grid = Grid::new(1280.0 - crate::theme::INDEX_LANE_W, Density::Balanced);
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
        // The first band opens one hang below the top of the content — the
        // wall's own top edge, the same one an unshelved wall had.
        assert!((runs[0].top - grid.hang).abs() < f32::EPSILON);
        // And each band opens exactly where the shelf above it ended.
        for pair in runs.windows(2) {
            assert!((pair[1].top - pair[0].end(grid)).abs() < f32::EPSILON);
        }
        // Height is the sum and nothing more: three bands, five rows, one top
        // hang. (Each row's own trailing hang is inside `row_h`, so the
        // wall's bottom edge is a hang too.)
        let expected = grid.hang + 3.0 * grid.header_h() + 5.0 * grid.row_h;
        assert!((shelves.height() - expected).abs() < 0.01);
        assert_eq!(shelves.albums(), 14);

        // The same three statements at the other two steps: the wall's top
        // edge is the step's hang, the band is the step's hang, and the height
        // is the sum of the two and the rows. A step that broke the rhythm
        // would put a gap or an overlap in the pinned lane.
        for density in Density::ALL {
            let grid = Grid::new(1280.0 - crate::theme::INDEX_LANE_W, density);
            let shelves = Shelves::new(grid, &[4, 9, 1]);
            let runs = shelves.runs();
            let rows: usize = runs.iter().map(|run| run.rows).sum();
            assert!(
                (runs[0].top - density.hang()).abs() < f32::EPSILON,
                "{}: the wall's top edge is {} px",
                density.label(),
                runs[0].top
            );
            assert!((grid.header_h() - density.hang()).abs() < f32::EPSILON);
            #[expect(
                clippy::cast_precision_loss,
                reason = "a row count here is a single digit"
            )]
            let expected = density.hang() + 3.0 * grid.header_h() + rows as f32 * grid.row_h;
            assert!(
                (shelves.height() - expected).abs() < 0.01,
                "{}: {} px of wall, expected {expected}",
                density.label(),
                shelves.height()
            );
        }
    }

    /// A shelf the filter emptied is not drawn — no header with nothing under
    /// it — and the shelves that survive keep their original identity.
    #[test]
    fn an_emptied_shelf_is_not_drawn_and_the_survivors_keep_their_group() {
        let grid = Grid::new(1280.0, Density::Balanced);
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
    ///
    /// Swept at **every density** for the same reason it is swept at every
    /// offset: the hand-over is exact only because the band and a row's
    /// trailing gap are the same number, and the density is where that number
    /// now comes from.
    #[test]
    fn exactly_one_header_is_in_the_pinned_lane_at_every_offset() {
        for density in Density::ALL {
            let step = density.label();
            let grid = Grid::new(1280.0 - crate::theme::INDEX_LANE_W, density);
            let shelves = Shelves::new(grid, &[8, 8, 8]);
            let runs = shelves.runs().to_vec();

            // Nothing is pinned while the first band is still on screen…
            assert_eq!(shelves.sticky(0.0), None, "{step}");
            assert_eq!(
                shelves.sticky(runs[0].top),
                None,
                "{step}: the hand-over instant"
            );
            // …and the first pixel past it pins the header it just replaced, in
            // the same place, so nothing moves across the hand-over.
            assert_eq!(shelves.sticky(runs[0].top + 1.0), Some(0));

            // The release: at the offset where the next band enters the lane.
            let release = runs[1].top - grid.header_h();
            assert_eq!(shelves.sticky(release), Some(0));
            assert_eq!(shelves.sticky(release + 0.5), None);
            // That offset is also exactly where shelf 0's last row of covers
            // clears the top of the viewport — which is why the lane below the
            // header is clear wall rather than artwork.
            let last_row_bottom = runs[0].end(grid) - grid.hang;
            assert!(
                (release - last_row_bottom).abs() < f32::EPSILON,
                "{step}: the pin releases at {release} but the covers end at {last_row_bottom}"
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
                    .any(|run| (run.top - scroll) >= 0.0 && (run.top - scroll) < grid.header_h());
                assert!(
                    !(pinned.is_some() && in_lane),
                    "{step} at {scroll}: {pinned:?} pinned *and* an in-flow header in the lane"
                );
                if scroll >= runs[0].top {
                    assert!(
                        pinned.is_some() || in_lane,
                        "{step} at {scroll}: the lane holds no header at all"
                    );
                }
            }
            // Above the first band the lane is empty on purpose: the wall's own
            // top hang is what is there, and pinning a header over it would put
            // chrome where the wall's edge is.
            assert_eq!(shelves.sticky(runs[0].top - 1.0), None, "{step}");
        }
    }

    /// Virtualization survives shelving: only the shelves the viewport touches
    /// are built, and the albums the prefetch asks for are the ones on screen.
    #[test]
    fn only_the_shelves_the_viewport_touches_are_built() {
        let grid = Grid::new(1280.0 - crate::theme::INDEX_LANE_W, Density::Balanced);
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
    /// an equation (the product's standing rule, `.interface-design/system.md` §1.2) —
    /// **at every density**.
    ///
    /// `.interface-design/system.md` §7.1: *`ART_MAX` never exceeds `THUMB_PX`
    /// at any step, so the nothing-upscales invariant is a property of the
    /// system, not of the default.* This is that sentence, swept.
    #[test]
    fn the_wall_never_draws_art_larger_than_its_source() {
        use crate::theme::{ART_MAX, ART_MIN};

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
        // The loosest step is the one that could exceed the cache, and it is
        // the one the thumbnail size is derived from: `max(ART_MAX over all
        // steps) == THUMB_PX`.
        let loosest = Density::ALL
            .into_iter()
            .map(Density::art_max)
            .fold(0.0_f32, f32::max);
        assert!(
            (loosest - source).abs() < f32::EPSILON,
            "the loosest step caps art at {loosest} against a {source} px thumbnail"
        );
        for density in Density::ALL {
            for width in band() {
                assert!(
                    Grid::new(width, density).art <= source,
                    "{} at {width} px upscales",
                    density.label()
                );
            }
        }
        // At the default, ART_MAX = 4/3 × ART_MIN, so the art hands off from
        // its largest to its smallest at exactly one width per column
        // transition. It is a property of *Balanced*, not of the ladder:
        // Spacious deliberately runs a narrow 288 … 320 band because its art
        // is meant to sit at the cap, and Dense's 176 … 240 is 4/3 to within a
        // third of a pixel and is written as whole numbers instead.
        assert!((ART_MAX - 4.0 / 3.0 * ART_MIN).abs() < f32::EPSILON);
    }
}
