//! The transport glyphs — play, pause, next — as vector outlines rasterized
//! once into small RGBA sprites.
//!
//! This is view-layer code (ADR-0006 layer 3): it draws with
//! [`crate::theme`]'s tokens and holds no state. What lives here is the
//! *shape* of each glyph, expressed as polygons in a unit square, plus the
//! pure rasterizer that turns them into pixels. Both are unit-tested without
//! a window.
//!
//! # Why a rasterized sprite and not something else
//!
//! iced 0.13 ships no icon set, so the glyphs have to come from somewhere.
//! Four routes were considered; the evidence for each was read out of the
//! vendored sources (`iced_core` 0.13.2, `iced_widget` 0.13.4, `iced_wgpu`
//! 0.13.5) rather than assumed:
//!
//! - **Quads from a custom widget** (the obvious sibling of [`crate::seek`])
//!   cannot draw a triangle. `iced::advanced::renderer::Renderer` exposes
//!   exactly one primitive — [`fill_quad`](iced::advanced::renderer::Renderer::fill_quad),
//!   an axis-aligned rectangle — and `Transformation` offers only
//!   `translate` and uniform `scale`, no rotation, so a rotated or clipped
//!   quad is not available either. A triangle would have to be stacked out
//!   of thin horizontal spans, and the wgpu quad shader rules that out on
//!   quality: `solid.wgsl` inflates every quad by one pixel and feathers its
//!   edge with `smoothstep(radius - 0.5, radius + 0.5, …)`, so spans thinner
//!   than a pixel are *all* feather. Overlapping spans then accumulate alpha
//!   (`1 - (1 - a)^n`) along the hypotenuse and the diagonal comes out fat
//!   and dark; non-overlapping spans leave a visible seam between every row.
//! - **`iced`'s `canvas` feature** would give real path filling, at the cost
//!   of `lyon_tessellation` and its dependency tree — new crates for three
//!   glyphs, against a project rule that a dependency is a reviewed decision.
//! - **`iced`'s `svg` feature** is the same trade, worse: `resvg`/`usvg` and
//!   their XML stack.
//! - **An icon font** (`iced::font::load`) adds no crate but does add a
//!   binary asset with its own license to vet and subset, and iced 0.13 has
//!   no OpenType feature control to lean on. Unicode media glyphs from
//!   system fonts were rejected outright: they are missing or wildly
//!   different across platforms, and a player should look the same
//!   everywhere.
//!
//! So the glyphs are described as polygons, rasterized once at startup with
//! [`SAMPLES`]×[`SAMPLES`] supersampled coverage, and drawn through the
//! `image` pipeline that is already compiled in for album art. The cost is
//! honest and bounded: a sprite is resolution-*dependent* in a way a path is
//! not. It is drawn at [`theme::ICON_PX`] logical pixels from a raster
//! [`SUPERSCALE`]× that size, so a 1× display minifies 2:1 (bilinear over a
//! 2×2 block — an exact box filter) and a 2× display lands 1:1. Beyond 2×
//! the glyph magnifies and softens slightly; that is the one thing a real
//! vector path would do better, and it is the price of not taking a
//! dependency.
//!
//! # Pixel stability
//!
//! Every glyph rasterizes to the same square, and the view draws it into a
//! fixed-size box inside a fixed-size button. Swapping play for pause
//! therefore cannot move anything — which is half of the fix for the
//! bottom bar's flash (see [`crate::player`]'s pending-affordance note).

use std::sync::LazyLock;

use iced::Color;
use iced::widget::image;

use crate::player::PlayPause;
use crate::theme;

/// Sub-samples per pixel *per axis* when rasterizing coverage. Eight gives
/// 64 coverage levels per pixel — smooth enough for a 16 px glyph's
/// diagonals, and the whole sheet is a few hundred thousand point tests
/// computed exactly once.
pub const SAMPLES: u32 = 8;

/// How many raster pixels the sprite carries per logical pixel (module
/// docs). Two is the classic `@2x` asset: exact on 1× and 2× displays.
pub const SUPERSCALE: u32 = 2;

/// [`theme::ICON_PX`] as a whole number of pixels. Spelled separately
/// because a float-to-integer cast is not something to do in a `const`; the
/// tests pin it to the token it stands for.
const ICON_WHOLE_PX: u32 = 16;

/// Edge of every glyph sprite, in raster pixels.
pub const RASTER_PX: u32 = ICON_WHOLE_PX * SUPERSCALE;

/// A point in the unit square a glyph is drawn in: `(0, 0)` top-left,
/// `(1, 1)` bottom-right, matching screen coordinates.
type Vertex = (f32, f32);

/// One closed outline of a glyph.
type Outline = &'static [Vertex];

/// A transport glyph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Glyph {
    /// Play: the right-pointing triangle.
    Play,
    /// Pause: two upright bars.
    Pause,
    /// Next: a triangle against a bar.
    Next,
}

/// Play — one triangle, sitting a touch right of the box's centre so the
/// mass of it reads as centred rather than the bounding box doing.
const PLAY: &[Outline] = &[&[(0.27, 0.13), (0.27, 0.87), (0.83, 0.50)]];

/// Pause — two bars, symmetric about the centre.
const PAUSE: &[Outline] = &[
    &[(0.29, 0.14), (0.44, 0.14), (0.44, 0.86), (0.29, 0.86)],
    &[(0.56, 0.14), (0.71, 0.14), (0.71, 0.86), (0.56, 0.86)],
];

/// Next — the conventional skip-forward: a triangle running into a bar,
/// the pair symmetric about the centre.
const NEXT: &[Outline] = &[
    &[(0.20, 0.15), (0.20, 0.85), (0.62, 0.50)],
    &[(0.66, 0.15), (0.80, 0.15), (0.80, 0.85), (0.66, 0.85)],
];

impl Glyph {
    /// Every glyph, in sprite-sheet order.
    const ALL: [Self; 3] = [Self::Play, Self::Pause, Self::Next];

    /// The glyph's outlines in the unit square.
    #[must_use]
    fn outlines(self) -> &'static [Outline] {
        match self {
            Self::Play => PLAY,
            Self::Pause => PAUSE,
            Self::Next => NEXT,
        }
    }

    /// Its slot in the sprite sheet.
    fn index(self) -> usize {
        match self {
            Self::Play => 0,
            Self::Pause => 1,
            Self::Next => 2,
        }
    }

    /// Whether the unit-square point `(x, y)` is inside the glyph. The
    /// outlines never overlap, so "inside any" is the whole rule.
    #[must_use]
    pub fn covers(self, x: f32, y: f32) -> bool {
        self.outlines()
            .iter()
            .any(|outline| encloses(outline, x, y))
    }
}

impl From<PlayPause> for Glyph {
    fn from(toggle: PlayPause) -> Self {
        match toggle {
            PlayPause::Play => Self::Play,
            PlayPause::Pause => Self::Pause,
        }
    }
}

/// The rasterized sheet, built once on first use: three sprites, all
/// [`RASTER_PX`] square, inked in [`theme::GLYPH`].
///
/// Caching matters beyond the arithmetic — `image::Handle::from_rgba` mints
/// a fresh id per call, and a fresh id per frame would churn the renderer's
/// texture atlas. These three ids live as long as the process.
static SHEET: LazyLock<[image::Handle; 3]> = LazyLock::new(|| {
    let ink = rgb(theme::GLYPH);
    Glyph::ALL.map(|glyph| image::Handle::from_rgba(RASTER_PX, RASTER_PX, rasterize(glyph, ink)))
});

/// The sprite for `glyph`. Cheap: an `Arc` bump over the shared sheet.
#[must_use]
pub fn handle(glyph: Glyph) -> image::Handle {
    SHEET[glyph.index()].clone()
}

/// A theme color as the sRGB bytes the image pipeline stores. iced's
/// `Color` components are already sRGB-encoded and the renderer uploads
/// sprites as `Rgba8UnormSrgb`, so this is a straight scaling.
fn rgb(color: Color) -> [u8; 3] {
    let channel = |value: f32| {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "clamped to 0..=255 on the line above the cast"
        )]
        let byte = (value.clamp(0.0, 1.0) * 255.0).round() as u8;
        byte
    };
    [channel(color.r), channel(color.g), channel(color.b)]
}

/// Rasterize `glyph` into a [`RASTER_PX`]-square straight-alpha RGBA buffer.
///
/// Every pixel carries the full ink color and varies only in alpha —
/// including the fully transparent ones. That is deliberate: the renderer
/// filters RGB and alpha together, so a transparent pixel whose color was
/// left at zero would bleed a dark halo into the glyph's antialiased edge
/// when the sprite is scaled.
#[must_use]
fn rasterize(glyph: Glyph, ink: [u8; 3]) -> Vec<u8> {
    let edge = index_to_f32(RASTER_PX);
    let step = index_to_f32(SAMPLES);
    let total = SAMPLES * SAMPLES;
    let mut pixels = Vec::with_capacity((RASTER_PX * RASTER_PX * 4) as usize);
    for row in 0..RASTER_PX {
        for column in 0..RASTER_PX {
            let mut hits = 0_u32;
            for sub_y in 0..SAMPLES {
                for sub_x in 0..SAMPLES {
                    let x = (index_to_f32(column) + (index_to_f32(sub_x) + 0.5) / step) / edge;
                    let y = (index_to_f32(row) + (index_to_f32(sub_y) + 0.5) / step) / edge;
                    if glyph.covers(x, y) {
                        hits += 1;
                    }
                }
            }
            let alpha = u8::try_from(hits * u32::from(u8::MAX) / total).unwrap_or(u8::MAX);
            pixels.extend_from_slice(&[ink[0], ink[1], ink[2], alpha]);
        }
    }
    pixels
}

/// A small loop index as `f32`. Every caller passes a raster coordinate or a
/// sub-sample index, all far below `f32`'s exact-integer range.
#[expect(
    clippy::cast_precision_loss,
    reason = "raster indices are bounded by RASTER_PX * SAMPLES = 256"
)]
fn index_to_f32(value: u32) -> f32 {
    value as f32
}

/// Whether `(x, y)` is inside the closed polygon `outline`, by the
/// even-odd ray-crossing rule: count the edges a ray cast to the left
/// crosses, and an odd count means inside. Degenerate outlines (fewer than
/// three vertices) enclose nothing.
fn encloses(outline: Outline, x: f32, y: f32) -> bool {
    let Some(&last) = outline.last() else {
        return false;
    };
    if outline.len() < 3 {
        return false;
    }
    let mut inside = false;
    let mut previous = last;
    for &current in outline {
        let (cx, cy) = current;
        let (px, py) = previous;
        // Straddling edges only: the half-open test on `y` counts a vertex
        // exactly once, so a ray through one does not flip twice.
        if (cy > y) != (py > y) {
            let crossing = (px - cx) * (y - cy) / (py - cy) + cx;
            if x < crossing {
                inside = !inside;
            }
        }
        previous = current;
    }
    inside
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Alpha of the pixel at `(column, row)` in a rasterized sprite.
    fn alpha(pixels: &[u8], column: u32, row: u32) -> u8 {
        let index = ((row * RASTER_PX + column) * 4 + 3) as usize;
        pixels[index]
    }

    #[test]
    fn the_raster_size_follows_its_token() {
        // The sprite must be exactly SUPERSCALE times the box it is drawn
        // into, or the "@2x" argument in the module docs stops holding.
        assert!((index_to_f32(ICON_WHOLE_PX) - theme::ICON_PX).abs() < f32::EPSILON);
        assert_eq!(RASTER_PX, ICON_WHOLE_PX * SUPERSCALE);
    }

    #[test]
    fn every_glyph_rasterizes_to_the_same_square() {
        // The whole point of the sprite sheet: swapping play for pause can
        // never change a size, so it can never move the layout. This is the
        // pixel-stability half of the bottom bar's flash fix.
        let sizes: Vec<usize> = Glyph::ALL
            .iter()
            .map(|&glyph| rasterize(glyph, [255, 255, 255]).len())
            .collect();
        let expected = (RASTER_PX * RASTER_PX * 4) as usize;
        for (glyph, size) in Glyph::ALL.iter().zip(&sizes) {
            assert_eq!(*size, expected, "{glyph:?} is not a RASTER_PX square");
        }
    }

    #[test]
    fn glyphs_are_inked_uniformly_so_scaling_cannot_halo() {
        // Every pixel carries the ink color, transparent ones included.
        let ink = [200, 150, 90];
        let pixels = rasterize(Glyph::Play, ink);
        for chunk in pixels.chunks_exact(4) {
            assert_eq!(&chunk[..3], &ink, "a pixel lost the ink color");
        }
    }

    #[test]
    fn play_is_a_triangle_pointing_right() {
        let pixels = rasterize(Glyph::Play, [255, 255, 255]);
        let mid = RASTER_PX / 2;
        // Solid on the vertical centre line, from the flat left edge nearly
        // to the apex.
        assert_eq!(alpha(&pixels, mid, mid), u8::MAX);
        assert_eq!(alpha(&pixels, RASTER_PX / 4 + 1, mid), u8::MAX);
        // The corners are outside the triangle: it narrows to the right.
        for (column, row) in [(0, 0), (RASTER_PX - 1, 0), (0, RASTER_PX - 1)] {
            assert_eq!(alpha(&pixels, column, row), 0, "corner {column},{row}");
        }
        // Top-right and bottom-right are the diagonal's far side.
        assert_eq!(alpha(&pixels, RASTER_PX - 2, 1), 0);
        assert_eq!(alpha(&pixels, RASTER_PX - 2, RASTER_PX - 2), 0);
        // Vertically symmetric about the centre line.
        for column in 0..RASTER_PX {
            for row in 0..RASTER_PX / 2 {
                assert_eq!(
                    alpha(&pixels, column, row),
                    alpha(&pixels, column, RASTER_PX - 1 - row),
                    "play is not symmetric at {column},{row}"
                );
            }
        }
    }

    #[test]
    fn pause_is_two_bars_with_a_gap_between_them() {
        let pixels = rasterize(Glyph::Pause, [255, 255, 255]);
        let mid = RASTER_PX / 2;
        // A horizontal sweep across the middle: gap, bar, gap, bar, gap.
        let row: Vec<bool> = (0..RASTER_PX)
            .map(|column| alpha(&pixels, column, mid) > 0)
            .collect();
        let runs = row.chunk_by(|a, b| a == b).count();
        assert_eq!(runs, 5, "expected gap/bar/gap/bar/gap across the middle");
        assert!(!row[0] && !row[(RASTER_PX - 1) as usize]);
        assert!(!row[mid as usize], "the two bars must not touch");
        // Horizontally symmetric, so the pair reads as centred.
        for column in 0..RASTER_PX / 2 {
            assert_eq!(
                alpha(&pixels, column, mid),
                alpha(&pixels, RASTER_PX - 1 - column, mid),
                "pause is not symmetric at column {column}"
            );
        }
    }

    #[test]
    fn next_is_a_triangle_against_a_bar() {
        let pixels = rasterize(Glyph::Next, [255, 255, 255]);
        let mid = RASTER_PX / 2;
        let row: Vec<bool> = (0..RASTER_PX)
            .map(|column| alpha(&pixels, column, mid) > 0)
            .collect();
        let runs = row.chunk_by(|a, b| a == b).count();
        assert_eq!(runs, 5, "expected gap/triangle/gap/bar/gap");
        // The bar is full height where the triangle has already tapered to
        // its apex — that is what makes the shape read as "skip", not
        // "play". Probe the last solid run's midpoint rather than a magic
        // column, so the assertion survives a nudge to the outline.
        let bar_columns: Vec<u32> = (0..RASTER_PX).filter(|&c| row[c as usize]).collect();
        let bar = *bar_columns.last().expect("the bar has some width") - 1;
        assert!(bar > mid, "the bar sits right of centre");
        let near_top = RASTER_PX / 5;
        assert!(alpha(&pixels, bar, near_top) > 0, "the bar reaches the top");
        assert_eq!(
            alpha(&pixels, mid, near_top),
            0,
            "the triangle has already tapered by there"
        );
    }

    #[test]
    fn the_toggle_state_picks_its_own_glyph() {
        assert_eq!(Glyph::from(PlayPause::Play), Glyph::Play);
        assert_eq!(Glyph::from(PlayPause::Pause), Glyph::Pause);
    }

    #[test]
    fn degenerate_outlines_enclose_nothing() {
        assert!(!encloses(&[], 0.5, 0.5));
        assert!(!encloses(&[(0.0, 0.0)], 0.5, 0.5));
        assert!(!encloses(&[(0.0, 0.0), (1.0, 1.0)], 0.5, 0.5));
    }

    #[test]
    fn the_sheet_hands_out_one_stable_handle_per_glyph() {
        for glyph in Glyph::ALL {
            assert_eq!(
                handle(glyph).id(),
                handle(glyph).id(),
                "{glyph:?} must keep its id, or the atlas churns every frame"
            );
        }
        assert_ne!(handle(Glyph::Play).id(), handle(Glyph::Pause).id());
    }
}
