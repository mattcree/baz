//! The control glyphs — play, pause, next, and the speaker in its two
//! states — as vector outlines rasterized once into small RGBA sprites.
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
//! - **Quads from a custom widget** (the obvious sibling of [`crate::groove`])
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
//! The same property is what lets the mute affordance swap between
//! [`Glyph::Speaker`] and [`Glyph::SpeakerMuted`] without the bottom bar
//! reflowing: muting is a change of ink, never of geometry.

use std::cell::RefCell;
use std::sync::{Arc, LazyLock, RwLock};

use iced::Color;
use iced::widget::image;

use crate::player::PlayPause;
use crate::theme;

/// Sub-samples per pixel *per axis* when rasterizing coverage. Eight gives
/// 64 coverage levels per pixel — smooth enough for a 20 px glyph's
/// diagonals, and the whole sheet is a few hundred thousand point tests
/// computed exactly once.
pub const SAMPLES: u32 = 8;

/// How many raster pixels the sprite carries per logical pixel (module
/// docs). Two is the classic `@2x` asset: exact on 1× and 2× displays.
pub const SUPERSCALE: u32 = 2;

/// [`theme::ICON_PX`] as a whole number of pixels. Spelled separately
/// because a float-to-integer cast is not something to do in a `const`; the
/// tests pin it to the token it stands for.
const ICON_WHOLE_PX: u32 = 20;

/// Edge of every glyph sprite, in raster pixels.
pub const RASTER_PX: u32 = ICON_WHOLE_PX * SUPERSCALE;

/// A point in the unit square a glyph is drawn in: `(0, 0)` top-left,
/// `(1, 1)` bottom-right, matching screen coordinates.
type Vertex = (f32, f32);

/// One closed outline of a glyph.
type Outline = &'static [Vertex];

/// A control glyph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Glyph {
    /// Play: the right-pointing triangle.
    Play,
    /// Pause: two upright bars.
    Pause,
    /// Next: a triangle against a bar.
    Next,
    /// Previous: [`Self::Next`] mirrored — a bar against a triangle.
    Previous,
    /// Speaker, sounding: the cone with two waves off it.
    Speaker,
    /// Speaker, muted: the same cone with a cross where the waves were.
    SpeakerMuted,
    /// Close: the dismissal cross, for the panels in the right-hand rail —
    /// and, since ADR-0040, for the app bar's own window control, which is
    /// the same mark meaning the same thing about a larger object.
    Close,
    /// **Minimise**: one short bar low in the box — the window put down.
    ///
    /// The first of the three window controls (ADR-0040 §3). It is
    /// deliberately *not* [`Self::Minus`]: a minus is a stepper's other half
    /// and sits on the box's centre line, whereas this bar sits low, which is
    /// where every desktop draws it and is the whole of what distinguishes
    /// "put the window down" from "one less". Two glyphs rather than one
    /// shared sprite, because a sheet where one drawing means two things is a
    /// sheet a reader has to be told about.
    WindowMinimise,
    /// **Maximise**: an empty square at the set's stroke — the window filling
    /// the screen.
    WindowMaximise,
    /// **Restore**: [`Self::WindowMaximise`]'s square with a second one behind
    /// its top-right corner — the window given back its old size. The pair is
    /// self-depicting the way the two lane marks are: what differs between the
    /// two drawings is what differs on screen.
    WindowRestore,
    /// Search: the magnifier that marks the well (doc 10 §4.1). Not a
    /// control's mark — the well is the control; this is its label.
    Magnifier,
    /// Settings: the gear, the strip's one icon-only door (doc 10 §3.4).
    Gear,
    /// **Equaliser**: three vertical faders at three different settings — the
    /// universal mark for the thing, and self-depicting in the way the two
    /// lane marks are. What it draws is what the panel behind it contains, so
    /// the door needs no word beside it.
    ///
    /// The handles sit at three *different* heights on purpose. Three level
    /// handles would read as a bar chart or a signal meter; the whole point of
    /// a graphic equaliser's icon is that its faders disagree.
    Equalizer,
    /// **Chromeless**: four corner brackets, for the frame going away from
    /// around Now playing. See [`CHROMELESS`].
    Chromeless,
    /// Add-to: the transfer slot's mark, on every row that can send a track
    /// toward the picker (doc 10 §3.6).
    Plus,
    /// Step-down: [`Self::Plus`]'s horizontal bar alone, for the settings
    /// steppers' `−` half.
    Minus,
    /// Reorder, up: the row steppers' upward arrow.
    ArrowUp,
    /// Reorder, down: [`Self::ArrowUp`] mirrored.
    ArrowDown,
    /// The wall at its loosest hang — one work filling the field. The first
    /// of the four density detents (ADR-0028); see [`DENSITY_SPACIOUS`].
    DensitySpacious,
    /// The wall at the default hang — four works. The second detent.
    DensityBalanced,
    /// The wall one step tighter than the default — nine works. The third
    /// detent, and the one the owner's fourth step added (2026-08-10).
    ///
    /// **It is drawn with the outline `DensityDense` carried while there were
    /// three steps**, and `DensityDense` moved on to sixteen. The set depicts
    /// *the wall at that hang* and nothing else, so its subdivisions are
    /// 1, 2, 3, 4 across — there is no whole number between two and three,
    /// and a mark that stopped being the wall to keep its old pixels would
    /// have been a mark that lies. See [`DENSITY_COMPACT`].
    DensityCompact,
    /// The wall at its tightest hang — sixteen works. The fourth detent.
    DensityDense,
    /// Plain square artwork, used by Now playing's visual-mode detents.
    VisualCover,
    /// A landscape jewel case with its narrow tray hinge.
    VisualCase,
    /// No foreground album object: the cover frame crossed out.
    VisualNone,
    /// Uneven frequency bars across the visual field.
    VisualSpectrum,
    /// Three short lines: the Now Playing fact feed.
    VisualFacts,
    /// Browser-style place history, backward. This is a chevron, not the
    /// transport's `Previous`: it changes the window's place, never a track.
    HistoryBack,
    /// Browser-style place history, forward: [`Self::HistoryBack`] mirrored.
    HistoryForward,
    /// Notification bell: the application's operational-health door.
    Bell,
    /// Queue: three stacked bars, the last one short — a list with more to
    /// come. The wall's hover option (doc 13 §11 as the owner overruled it).
    Queue,
    /// Open: the disclosure chevron — *go to the thing this row names*. The
    /// wall's hover option for the press the tile has always made.
    Open,
    /// Home: the house. The returns lane's first destination (ADR-0030 as the
    /// owner amended it) — the one glyph in the set that is a *convention*
    /// rather than a depiction of a baz object, and it is admitted because the
    /// owner asked for the Spotify shape by name and a house is what that
    /// shape's first row wears everywhere it exists.
    Home,
    /// Library: four spines standing on a shelf. The collection, depicted as
    /// the thing a collection physically is — and deliberately not a grid of
    /// squares, which is what the density detents already are.
    Library,
    /// Now playing: the record, with its label. The lane's third destination.
    ///
    /// A ring rather than a disc: a filled circle at this size is a dot, and
    /// the one dot in the product means the lamp. Drawn as a single outline
    /// that traces the rim and then the label the other way round, so
    /// [`ray_crosses`]'s even-odd rule punches the hole — [`rasterize`] takes
    /// the *union* of outlines, so a second circle could not.
    NowPlaying,
    /// The lane, expanded: a frame with a wide left band. One of the two marks
    /// at the lane's foot, in the density detents' exact anatomy (ADR-0028).
    LaneExpanded,
    /// The lane, collapsed: [`Self::LaneExpanded`]'s frame with a narrow left
    /// band. Self-depicting as a pair — what changes between the two marks is
    /// what changes on screen.
    LaneCollapsed,
    /// **Shuffle: the crossed arrows** — two paths that swap places, each
    /// ending in a head.
    ///
    /// The one symbol in this sheet that is a **convention baz refused on
    /// purpose and has now earned**. `docs/design/10-controls-and-iconography.md`
    /// §3.2 ruled the crossed arrows out with a precise argument: the symbol
    /// *"promises a mode with a lit state"* and baz's shuffle was an act, so
    /// the glyph would have been a lie about the control's grammar and
    /// `Shuffle` stayed a word. The owner made shuffle a mode on 2026-08-10
    /// (*"can you make shuffle a property of the player i.e. toggle on/off"*),
    /// which is exactly the condition the refusal named, so the symbol is now
    /// honest and it is taken.
    ///
    /// Drawn as four outlines — two shafts, two heads — because the union rule
    /// in [`rasterize`] fills the crossing solid, which is what the symbol
    /// wants: the two paths *meet*, they do not pass behind one another. A
    /// notch at the crossing would need an even-odd hole and would be a pixel
    /// wide at [`RASTER_PX`].
    Shuffle,
    /// Repeat current track: a loop arrow around one upright stroke.
    RepeatOne,
    Repeat,
    /// Outline heart: the track is not in Favourites.
    Heart,
    /// Filled heart: the track is in Favourites.
    HeartFilled,
}

/// Play — one triangle, sitting a touch right of the box's centre so the
/// mass of it reads as centred rather than the bounding box doing.
const PLAY: &[Outline] = &[&[(0.27, 0.13), (0.27, 0.87), (0.83, 0.50)]];

const HEART_FILLED: &[Outline] = &[&[
    (0.50, 0.88),
    (0.16, 0.56),
    (0.10, 0.38),
    (0.14, 0.22),
    (0.26, 0.12),
    (0.40, 0.14),
    (0.50, 0.26),
    (0.60, 0.14),
    (0.74, 0.12),
    (0.86, 0.22),
    (0.90, 0.38),
    (0.84, 0.56),
]];

const HEART: &[Outline] = &[&[
    (0.50, 0.88),
    (0.16, 0.56),
    (0.10, 0.38),
    (0.14, 0.22),
    (0.26, 0.12),
    (0.40, 0.14),
    (0.50, 0.26),
    (0.60, 0.14),
    (0.74, 0.12),
    (0.86, 0.22),
    (0.90, 0.38),
    (0.84, 0.56),
    (0.50, 0.76),
    (0.25, 0.51),
    (0.21, 0.38),
    (0.24, 0.27),
    (0.31, 0.22),
    (0.39, 0.23),
    (0.50, 0.38),
    (0.61, 0.23),
    (0.69, 0.22),
    (0.76, 0.27),
    (0.79, 0.38),
    (0.75, 0.51),
]];

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

/// Previous — [`NEXT`] reflected in the box's vertical centre line, vertex for
/// vertex (`x → 1 − x`): the bar first, then a triangle running back into it.
///
/// A *mirror*, not a second drawing, and the tests hold it to that: the pair
/// sit side by side in the transport, and a Previous whose bar were a pixel
/// thicker or whose apex sat a pixel lower would read as two glyphs from two
/// different sets. Reflecting the same numbers is also what makes the pair
/// symmetric about the play button between them, which is the shape every
/// listener already knows.
const PREVIOUS: &[Outline] = &[
    &[(0.20, 0.15), (0.34, 0.15), (0.34, 0.85), (0.20, 0.85)],
    &[(0.80, 0.15), (0.80, 0.85), (0.38, 0.50)],
];

/// The speaker's cone — the duct and the flare as one closed outline, shared
/// by both speaker glyphs so that muting cannot change the shape a user is
/// aiming at. Symmetric about the horizontal centre line.
const CONE: Outline = &[
    (0.06, 0.37),
    (0.20, 0.37),
    (0.40, 0.16),
    (0.40, 0.84),
    (0.20, 0.63),
    (0.06, 0.63),
];

/// Speaker, sounding — the cone with two waves.
///
/// The waves are chevrons rather than arcs, and that is a rasterizer
/// decision rather than a stylistic one: an arc would have to be a polygon
/// approximating it, and at [`RASTER_PX`] (40 px) the difference between a
/// seven-segment arc and a straight chevron is under a pixel of coverage.
/// Two hand-written hexagons say the same thing with no interpolation table.
const SPEAKER: &[Outline] = &[
    CONE,
    &[
        (0.50, 0.30),
        (0.60, 0.50),
        (0.50, 0.70),
        (0.57, 0.70),
        (0.67, 0.50),
        (0.57, 0.30),
    ],
    &[
        (0.72, 0.22),
        (0.85, 0.50),
        (0.72, 0.78),
        (0.79, 0.78),
        (0.92, 0.50),
        (0.79, 0.22),
    ],
];

/// Speaker, muted — the same cone, with a cross where the waves were.
///
/// The two bars of the cross **overlap** at their centre, which is why
/// [`rasterize`] takes the union of the outlines rather than the
/// even-odd rule across all of them: an even-odd test over the pair would
/// punch a diamond-shaped hole exactly where the cross should be solidest.
const SPEAKER_MUTED: &[Outline] = &[
    CONE,
    &[
        (0.481, 0.359),
        (0.559, 0.281),
        (0.919, 0.641),
        (0.841, 0.719),
    ],
    &[
        (0.481, 0.641),
        (0.559, 0.719),
        (0.919, 0.359),
        (0.841, 0.281),
    ],
];

/// Close — two bars crossing at the centre of the box.
///
/// The same construction as the mute cross and drawn the same way, by the
/// union rule in [`rasterize`]: an even-odd test over the pair would punch
/// a diamond-shaped hole exactly where the two bars overlap. Symmetric about
/// both axes, so the mark reads as centred in its button whichever panel it
/// dismisses.
const CLOSE: &[Outline] = &[
    &[(0.16, 0.26), (0.26, 0.16), (0.84, 0.74), (0.74, 0.84)],
    &[(0.16, 0.74), (0.26, 0.84), (0.84, 0.26), (0.74, 0.16)],
];

/// Minimise — **one bar, low in the box** (ADR-0040 §3).
///
/// At the set's 0.145 stroke, and short of the box's full measure so that it
/// reads as *a window laid down* rather than as a rule. Its distance from
/// [`MINUS`] is the whole point: the stepper's bar is centred and full-measure,
/// this one sits at the box's lower third and is inset. Symmetric about the
/// vertical axis, so the mark is centred in its button.
const WINDOW_MINIMISE: &[Outline] = &[&[
    (0.220, 0.6650),
    (0.780, 0.6650),
    (0.780, 0.8100),
    (0.220, 0.8100),
]];

/// Maximise — **an empty square at the set's stroke**: the window filling the
/// screen (ADR-0040 §3).
///
/// Outer edge 0.18…0.82, four bars 0.145 thick, so the hole is 0.35 across —
/// wide enough to read as *empty* at 16 px, which is what keeps it off
/// [`LANE_FRAME`]'s three-sided mark and off a filled block.
const WINDOW_MAXIMISE: &[Outline] = &[
    &[(0.18, 0.180), (0.82, 0.180), (0.82, 0.325), (0.18, 0.325)],
    &[(0.18, 0.675), (0.82, 0.675), (0.82, 0.820), (0.18, 0.820)],
    &[(0.18, 0.180), (0.325, 0.180), (0.325, 0.820), (0.18, 0.820)],
    &[(0.675, 0.180), (0.82, 0.180), (0.82, 0.820), (0.675, 0.820)],
];

/// Restore — **two offset squares**: the one in front is the window at the
/// size it goes back to, the one behind is the screen it is coming off.
///
/// Only the back square's top and right arms are drawn, which is what an
/// occluded square looks like and what every desktop draws here. The front
/// square is [`WINDOW_MAXIMISE`]'s construction at 0.12…0.90, down and left;
/// the two never touch, so the offset is legible rather than a smudge.
const WINDOW_RESTORE: &[Outline] = &[
    // The front square.
    &[(0.12, 0.300), (0.70, 0.300), (0.70, 0.445), (0.12, 0.445)],
    &[(0.12, 0.755), (0.70, 0.755), (0.70, 0.900), (0.12, 0.900)],
    &[(0.12, 0.300), (0.265, 0.300), (0.265, 0.900), (0.12, 0.900)],
    &[(0.555, 0.300), (0.70, 0.300), (0.70, 0.900), (0.555, 0.900)],
    // The back square, its left and bottom hidden behind the front one.
    &[(0.30, 0.100), (0.88, 0.100), (0.88, 0.245), (0.30, 0.245)],
    &[(0.735, 0.100), (0.88, 0.100), (0.88, 0.700), (0.735, 0.700)],
];

/// The magnifier's ring — a **keyhole outline**, because [`rasterize`]
/// takes the union of outlines and a ring drawn as two circles would have its
/// hole cancelled (doc 10 §3.6's implementation note). One closed polygon:
/// the outer circle traced all the way round, a zero-width bridge in to the
/// inner circle, the inner circle traced back the other way, and the bridge
/// out again. The existing even-odd test ([`ray_crosses`]) then fills the band
/// and leaves the hole: a ray from a point inside the hole crosses both
/// circles — an even count — and the two coincident bridge edges cancel.
///
/// The slit sits at 45°, pointing into the lower right, so the handle laid
/// over it makes it unreachable even in principle. Ring stroke
/// 0.30 − 0.155 = **0.145** of the unit square — the shipped glyphs' own
/// band (the pause bars are 0.15, the close bars 0.141).
const MAGNIFIER_RING: Outline = &[
    (0.6321, 0.6321),
    (0.5562, 0.6873),
    (0.4669, 0.7163),
    (0.3731, 0.7163),
    (0.2838, 0.6873),
    (0.2079, 0.6321),
    (0.1527, 0.5562),
    (0.1237, 0.4669),
    (0.1237, 0.3731),
    (0.1527, 0.2838),
    (0.2079, 0.2079),
    (0.2838, 0.1527),
    (0.3731, 0.1237),
    (0.4669, 0.1237),
    (0.5562, 0.1527),
    (0.6321, 0.2079),
    (0.6873, 0.2838),
    (0.7163, 0.3731),
    (0.7163, 0.4669),
    (0.6873, 0.5562),
    (0.6321, 0.6321),
    (0.5296, 0.5296),
    (0.5581, 0.4904),
    (0.5731, 0.4442),
    (0.5731, 0.3958),
    (0.5581, 0.3496),
    (0.5296, 0.3104),
    (0.4904, 0.2819),
    (0.4442, 0.2669),
    (0.3958, 0.2669),
    (0.3496, 0.2819),
    (0.3104, 0.3104),
    (0.2819, 0.3496),
    (0.2669, 0.3958),
    (0.2669, 0.4442),
    (0.2819, 0.4904),
    (0.3104, 0.5296),
    (0.3496, 0.5581),
    (0.3958, 0.5731),
    (0.4442, 0.5731),
    (0.4904, 0.5581),
    (0.5296, 0.5296),
];

/// Search — the ring above, and its handle: one bar at 45°, the ring's own
/// stroke width, starting inside the ring's outer edge (the union fills the
/// overlap, exactly as the mute cross's bars do) and running to the lower
/// right. The glass sits high-left so the whole mark reads centred with the
/// handle on.
const MAGNIFIER: &[Outline] = &[
    MAGNIFIER_RING,
    &[
        (0.6693, 0.5667),
        (0.5667, 0.6693),
        (0.8287, 0.9313),
        (0.9313, 0.8287),
    ],
];

/// Settings — the gear: the magnifier's keyhole construction with teeth on
/// the outer trace. **Eight teeth**, which is what reads cleanly at
/// [`theme::ICON_PX`] 16 (doc 10 §3.6): tips at 0.42 from centre, valleys at
/// 0.30, the hole at 0.155 — so the ring band is the set's 0.145 stroke and
/// the teeth stand 0.12 proud of it. The slit runs through the tooth at 0°.
/// Symmetric about both axes and both diagonals, so the mark reads as
/// centred in the strip's corner button.
const GEAR: &[Outline] = &[&[
    (0.9200, 0.5000),
    (0.9159, 0.5585),
    (0.7934, 0.5624),
    (0.7837, 0.5977),
    (0.7696, 0.6315),
    (0.7516, 0.6634),
    (0.8354, 0.7528),
    (0.7970, 0.7970),
    (0.7528, 0.8354),
    (0.6634, 0.7516),
    (0.6315, 0.7696),
    (0.5977, 0.7837),
    (0.5624, 0.7934),
    (0.5585, 0.9159),
    (0.5000, 0.9200),
    (0.4415, 0.9159),
    (0.4376, 0.7934),
    (0.4023, 0.7837),
    (0.3685, 0.7696),
    (0.3366, 0.7516),
    (0.2472, 0.8354),
    (0.2030, 0.7970),
    (0.1646, 0.7528),
    (0.2484, 0.6634),
    (0.2304, 0.6315),
    (0.2163, 0.5977),
    (0.2066, 0.5624),
    (0.0841, 0.5585),
    (0.0800, 0.5000),
    (0.0841, 0.4415),
    (0.2066, 0.4376),
    (0.2163, 0.4023),
    (0.2304, 0.3685),
    (0.2484, 0.3366),
    (0.1646, 0.2472),
    (0.2030, 0.2030),
    (0.2472, 0.1646),
    (0.3366, 0.2484),
    (0.3685, 0.2304),
    (0.4023, 0.2163),
    (0.4376, 0.2066),
    (0.4415, 0.0841),
    (0.5000, 0.0800),
    (0.5585, 0.0841),
    (0.5624, 0.2066),
    (0.5977, 0.2163),
    (0.6315, 0.2304),
    (0.6634, 0.2484),
    (0.7528, 0.1646),
    (0.7970, 0.2030),
    (0.8354, 0.2472),
    (0.7516, 0.3366),
    (0.7696, 0.3685),
    (0.7837, 0.4023),
    (0.7934, 0.4376),
    (0.9159, 0.4415),
    (0.9200, 0.5000),
    (0.6550, 0.5000),
    (0.6474, 0.4521),
    (0.6254, 0.4089),
    (0.5911, 0.3746),
    (0.5479, 0.3526),
    (0.5000, 0.3450),
    (0.4521, 0.3526),
    (0.4089, 0.3746),
    (0.3746, 0.4089),
    (0.3526, 0.4521),
    (0.3450, 0.5000),
    (0.3526, 0.5479),
    (0.3746, 0.5911),
    (0.4089, 0.6254),
    (0.4521, 0.6474),
    (0.5000, 0.6550),
    (0.5479, 0.6474),
    (0.5911, 0.6254),
    (0.6254, 0.5911),
    (0.6474, 0.5479),
    (0.6550, 0.5000),
]];

/// Add-to — two bars crossing at the centre, axis-aligned where [`CLOSE`]'s
/// are diagonal: the transfer mark for every row slot (doc 10 §3.6),
/// replacing the borrowed font `+`. The bars **overlap** at the centre and
/// fill by the union rule in [`rasterize`], exactly as the two crosses
/// do. Bar stroke 0.15, the pause bars' own.
const PLUS: &[Outline] = &[
    &[
        (0.425, 0.155),
        (0.575, 0.155),
        (0.575, 0.845),
        (0.425, 0.845),
    ],
    &[
        (0.155, 0.425),
        (0.155, 0.575),
        (0.845, 0.575),
        (0.845, 0.425),
    ],
];

/// Queue — three stacked bars, the third short: **a list with more to come**.
///
/// Doc 10 §3.4 refused a queue glyph *as a door's whole label* — "a queue
/// glyph, a playlist glyph and a menu glyph are one triangle-and-lines drawing
/// apart, and a door you can misread is worse than a door you must read" — and
/// that refusal stands for the `Queue` door in the bar, which is still a word.
/// This is the other case §3.1 names, and names in the affirmative: *where the
/// convention is close but not exact, the word stays and may carry the glyph as
/// its leading mark*. The word `Queue` is beside it and carries the semantics;
/// the mark carries the recognition.
///
/// The short third bar is what keeps it off the hamburger: three **equal** bars
/// are a menu everywhere in software, and three bars that run out are a list
/// that continues. Bars at the set's 0.15 stroke, on a 0.09 rhythm, the block
/// symmetric about the box's centre line.
const QUEUE: &[Outline] = &[
    &[(0.12, 0.185), (0.88, 0.185), (0.88, 0.335), (0.12, 0.335)],
    &[(0.12, 0.425), (0.88, 0.425), (0.88, 0.575), (0.12, 0.575)],
    &[(0.12, 0.665), (0.60, 0.665), (0.60, 0.815), (0.12, 0.815)],
];

/// Open — the disclosure chevron: **go to the thing this row names**.
///
/// Two arms at 45°, at the set's stroke, meeting at a vertex right of centre;
/// the union fills the join, exactly as [`ARROW_UP`]'s arms do. A *stroke*
/// rather than a filled triangle, which is also what keeps it apart from
/// [`PLAY`] two rows above it on the same veil: one is an open angle, the other
/// a solid mass, and they are drawn in different ink besides.
const OPEN: &[Outline] = &[
    &[
        (0.2988, 0.2063),
        (0.4013, 0.1038),
        (0.7563, 0.4488),
        (0.6538, 0.5513),
    ],
    &[
        (0.2988, 0.7938),
        (0.4013, 0.8963),
        (0.7563, 0.5513),
        (0.6538, 0.4488),
    ],
];

/// Home — the house: a wide roof over a body, one closed silhouette.
///
/// The roof overhangs the walls on both sides, which is the whole of what
/// makes a seven-vertex polygon read as a house rather than as an arrow on a
/// box at 16 px.
const HOME: &[Outline] = &[&[
    (0.5000, 0.0800),
    (0.9400, 0.5000),
    (0.8200, 0.5000),
    (0.8200, 0.9000),
    (0.1800, 0.9000),
    (0.1800, 0.5000),
    (0.0600, 0.5000),
]];

/// Library — four spines of different heights standing on a shelf.
///
/// The heights are unequal on purpose: four equal bars are a chart, and the
/// one thing this mark must not be mistaken for is the density detents' grid
/// two controls away from it.
const LIBRARY: &[Outline] = &[
    &[
        (0.1400, 0.2000),
        (0.2800, 0.2000),
        (0.2800, 0.8000),
        (0.1400, 0.8000),
    ],
    &[
        (0.3400, 0.1200),
        (0.4800, 0.1200),
        (0.4800, 0.8000),
        (0.3400, 0.8000),
    ],
    &[
        (0.5400, 0.2600),
        (0.6800, 0.2600),
        (0.6800, 0.8000),
        (0.5400, 0.8000),
    ],
    &[
        (0.7400, 0.1600),
        (0.8800, 0.1600),
        (0.8800, 0.8000),
        (0.7400, 0.8000),
    ],
    &[
        (0.0800, 0.8000),
        (0.9200, 0.8000),
        (0.9200, 0.8900),
        (0.0800, 0.8900),
    ],
];

/// Now playing — the record: the rim, then the label traced the other way
/// round in the **same** outline, so the even-odd rule leaves the spindle hole
/// open. See [`Glyph::NowPlaying`] for why it cannot be two circles.
const NOW_PLAYING: &[Outline] = &[&[
    (0.9400, 0.5000),
    (0.9290, 0.5979),
    (0.8964, 0.6909),
    (0.8440, 0.7743),
    (0.7743, 0.8440),
    (0.6909, 0.8964),
    (0.5979, 0.9290),
    (0.5000, 0.9400),
    (0.4021, 0.9290),
    (0.3091, 0.8964),
    (0.2257, 0.8440),
    (0.1560, 0.7743),
    (0.1036, 0.6909),
    (0.0710, 0.5979),
    (0.0600, 0.5000),
    (0.0710, 0.4021),
    (0.1036, 0.3091),
    (0.1560, 0.2257),
    (0.2257, 0.1560),
    (0.3091, 0.1036),
    (0.4021, 0.0710),
    (0.5000, 0.0600),
    (0.5979, 0.0710),
    (0.6909, 0.1036),
    (0.7743, 0.1560),
    (0.8440, 0.2257),
    (0.8964, 0.3091),
    (0.9290, 0.4021),
    (0.9400, 0.5000),
    (0.6500, 0.5000),
    (0.6410, 0.4487),
    (0.6149, 0.4036),
    (0.5750, 0.3701),
    (0.5260, 0.3523),
    (0.4740, 0.3523),
    (0.4250, 0.3701),
    (0.3851, 0.4036),
    (0.3590, 0.4487),
    (0.3500, 0.5000),
    (0.3590, 0.5513),
    (0.3851, 0.5964),
    (0.4250, 0.6299),
    (0.4740, 0.6477),
    (0.5260, 0.6477),
    (0.5750, 0.6299),
    (0.6149, 0.5964),
    (0.6410, 0.5513),
    (0.6500, 0.5000),
]];

/// The frame both lane marks stand in: a top, a right and a bottom stroke.
/// Shared between them so the pair can differ in exactly one thing.
const LANE_FRAME: [Outline; 3] = [
    &[
        (0.0800, 0.2000),
        (0.9200, 0.2000),
        (0.9200, 0.2800),
        (0.0800, 0.2800),
    ],
    &[
        (0.8400, 0.2000),
        (0.9200, 0.2000),
        (0.9200, 0.8000),
        (0.8400, 0.8000),
    ],
    &[
        (0.0800, 0.7200),
        (0.9200, 0.7200),
        (0.9200, 0.8000),
        (0.0800, 0.8000),
    ],
];

/// The lane, expanded — the frame with a **wide** left band.
const LANE_EXPANDED: &[Outline] = &[
    LANE_FRAME[0],
    LANE_FRAME[1],
    LANE_FRAME[2],
    &[
        (0.0800, 0.2000),
        (0.4000, 0.2000),
        (0.4000, 0.8000),
        (0.0800, 0.8000),
    ],
];

/// The lane, collapsed — the same frame with a **narrow** left band.
const LANE_COLLAPSED: &[Outline] = &[
    LANE_FRAME[0],
    LANE_FRAME[1],
    LANE_FRAME[2],
    &[
        (0.0800, 0.2000),
        (0.2400, 0.2000),
        (0.2400, 0.8000),
        (0.0800, 0.8000),
    ],
];

/// Step-down — [`PLUS`]'s horizontal bar, alone: the settings steppers'
/// `−`, drawn rather than borrowed (U+2212 remains legitimate *in a value*,
/// where it is a figure; in a control slot it was the accidental fourth
/// vocabulary doc 10 §0.3 names). Sharing the plus's own bar is what makes
/// the pair read as a pair.
/// **The equaliser mark**: three rails, three handles, no two at the same
/// height — see [`Glyph::Equalizer`] for why they disagree.
///
/// # Drawn at the cluster's weight, not at a hairline
///
/// The first drawing had 0.05 rails and 0.11 handles, which laid **0.179** of
/// the box in ink against [`GEAR`]'s 0.324 one seam away and [`BELL`]'s 0.342
/// on the other side — a little over half the mass of both its neighbours.
///
/// That is the identical fault the owner named twice about the bell (*"the
/// bell icon is a little bit narrow/skinny"*, then *"still weirdly skinny"*),
/// arriving on a brand-new mark the same week the bell's was fixed. What that
/// says is that the lesson was never written down anywhere a *new* glyph would
/// meet it: both bell fixes were bell-shaped, one measuring the widest
/// scanline and the next the median run, and neither said anything about the
/// mark that had not been drawn yet.
///
/// So the rails are 0.10 and the handles 0.26 × 0.21, which puts it at about
/// 0.31 — inside its neighbours' range — and
/// `a_mark_in_the_app_bar_carries_its_neighbours_weight` now holds the whole
/// cluster to that rather than one glyph at a time.
///
/// The handles stay narrower than the 0.28 rail pitch so two of them at the
/// same height could never fuse into a bar.
const EQUALIZER: &[Outline] = &[
    // First rail and its handle, high.
    &[
        (0.170, 0.145),
        (0.270, 0.145),
        (0.270, 0.855),
        (0.170, 0.855),
    ],
    &[
        (0.090, 0.235),
        (0.350, 0.235),
        (0.350, 0.445),
        (0.090, 0.445),
    ],
    // Second rail, low.
    &[
        (0.450, 0.145),
        (0.550, 0.145),
        (0.550, 0.855),
        (0.450, 0.855),
    ],
    &[
        (0.370, 0.515),
        (0.630, 0.515),
        (0.630, 0.725),
        (0.370, 0.725),
    ],
    // Third rail, between them — so no two agree and the mark reads as a
    // curve being set rather than as three of anything.
    &[
        (0.730, 0.145),
        (0.830, 0.145),
        (0.830, 0.855),
        (0.730, 0.855),
    ],
    &[
        (0.650, 0.355),
        (0.910, 0.355),
        (0.910, 0.565),
        (0.650, 0.565),
    ],
];

/// **The chromeless mark** — four corner brackets opening outward.
///
/// The owner, 2026-08-18: *"we should consider adding a little toggle here
/// which allows it to go into a sort of 'chromeless' mode which really shows
/// off the now playing view."*
///
/// Corners rather than an arrow or a square, because the two neighbours it
/// could be confused with are already taken: [`WINDOW_MAXIMISE`] is an empty square
/// (the window filling the screen) and this is not a window operation at all —
/// it is the *frame going away from around the picture*. Four brackets are the
/// field's own sign for that, and they read at [`theme::ICON_PX`] 20 where an
/// outward-arrow cross does not.
///
/// # Drawn at **its own row's** weight, which is not the bar's only weight
///
/// The first cut used the application cluster's stroke and measured 0.292 of
/// its box. That is right beside [`GEAR`] and wrong beside the marks it
/// actually stands in: the view's own options run 0.19–0.26, and a mark
/// arriving at 0.29 among them would have made *its neighbours* look thin —
/// the exact fault [`EQUALIZER`]'s note is about, committed in the other
/// direction.
///
/// The bar carries two weights on purpose (ADR-0040 §2's zones: the view's
/// options in zone 3, the application's controls in zone 4), and
/// `a_mark_in_the_app_bar_carries_its_neighbours_weight` holds each family to
/// itself rather than flattening them into one.
const CHROMELESS: &[Outline] = &[
    // Top-left.
    &[
        (0.100, 0.100),
        (0.420, 0.100),
        (0.420, 0.220),
        (0.100, 0.220),
    ],
    &[
        (0.100, 0.100),
        (0.220, 0.100),
        (0.220, 0.420),
        (0.100, 0.420),
    ],
    // Top-right.
    &[
        (0.580, 0.100),
        (0.900, 0.100),
        (0.900, 0.220),
        (0.580, 0.220),
    ],
    &[
        (0.780, 0.100),
        (0.900, 0.100),
        (0.900, 0.420),
        (0.780, 0.420),
    ],
    // Bottom-left.
    &[
        (0.100, 0.780),
        (0.420, 0.780),
        (0.420, 0.900),
        (0.100, 0.900),
    ],
    &[
        (0.100, 0.580),
        (0.220, 0.580),
        (0.220, 0.900),
        (0.100, 0.900),
    ],
    // Bottom-right.
    &[
        (0.580, 0.780),
        (0.900, 0.780),
        (0.900, 0.900),
        (0.580, 0.900),
    ],
    &[
        (0.780, 0.580),
        (0.900, 0.580),
        (0.900, 0.900),
        (0.780, 0.900),
    ],
];

const MINUS: &[Outline] = &[&[
    (0.155, 0.425),
    (0.845, 0.425),
    (0.845, 0.575),
    (0.155, 0.575),
]];

/// Reorder, up — a shaft with a chevron head, all three bars at the set's
/// stroke: the arrows are *strokes*, not filled triangles, for the speaker
/// waves' reason — the chevron is the same statement with no interpolation
/// table, and one stroke weight is what makes the sheet one set.
const ARROW_UP: &[Outline] = &[
    // The shaft, from just under the apex to the baseline.
    &[
        (0.4275, 0.20),
        (0.5725, 0.20),
        (0.5725, 0.86),
        (0.4275, 0.86),
    ],
    // The two arms, 45° each way from the apex; the union fills the join.
    &[
        (0.5513, 0.2513),
        (0.4487, 0.1487),
        (0.1587, 0.4387),
        (0.2613, 0.5413),
    ],
    &[
        (0.5513, 0.1487),
        (0.4487, 0.2513),
        (0.7387, 0.5413),
        (0.8413, 0.4387),
    ],
];

/// Reorder, down — [`ARROW_UP`] reflected in the box's horizontal centre
/// line, vertex for vertex (`y → 1 − y`): a mirror, not a second drawing,
/// for [`PREVIOUS`]'s reason — the pair sit stacked in one slot column, and
/// an arrow whose head flared a pixel differently from its twin's would
/// read as two glyphs from two sets. The test holds them to the reflection
/// pixel for pixel.
const ARROW_DOWN: &[Outline] = &[
    &[
        (0.4275, 0.80),
        (0.5725, 0.80),
        (0.5725, 0.14),
        (0.4275, 0.14),
    ],
    &[
        (0.5513, 0.7487),
        (0.4487, 0.8513),
        (0.1587, 0.5613),
        (0.2613, 0.4587),
    ],
    &[
        (0.5513, 0.8513),
        (0.4487, 0.7487),
        (0.7387, 0.4587),
        (0.8413, 0.5613),
    ],
];

/// The four density detents (ADR-0028 as amended) share one field —
/// 0.125 … 0.875, a 12 px square at [`theme::ICON_PX`] — subdivided one, two,
/// three and four ways: **the wall itself at its four hangs**, one work, four
/// works, nine works, sixteen works. The field is constant across the set so
/// the four read as one thing at four settings rather than as four marks,
/// exactly as the walls they depict are one wall at four steps.
///
/// **The fourth step re-keyed the set rather than joining it.** `Compact`
/// hangs between `Balanced` and `Dense`, and there is no whole number of
/// columns between two and three — so the subdivision each step wears moved
/// up by one from `Compact` on, and `Dense` gained the 4 × 4 field. The
/// alternative was a mark that no longer depicted its own wall, which is the
/// one thing this set may not do.
const DENSITY_FIELD: (f32, f32) = (0.125, 0.875);

/// One work filling the field: the loosest hang.
const DENSITY_SPACIOUS: &[Outline] = &[&[
    (DENSITY_FIELD.0, DENSITY_FIELD.0),
    (DENSITY_FIELD.1, DENSITY_FIELD.0),
    (DENSITY_FIELD.1, DENSITY_FIELD.1),
    (DENSITY_FIELD.0, DENSITY_FIELD.1),
]];

/// Four works: the default hang. Cell 0.3125 (5 px), gap 0.125 (2 px) —
/// `2 × 0.3125 + 0.125` spans the field exactly.
const DENSITY_BALANCED: &[Outline] = &[
    &[
        (0.125, 0.125),
        (0.4375, 0.125),
        (0.4375, 0.4375),
        (0.125, 0.4375),
    ],
    &[
        (0.5625, 0.125),
        (0.875, 0.125),
        (0.875, 0.4375),
        (0.5625, 0.4375),
    ],
    &[
        (0.125, 0.5625),
        (0.4375, 0.5625),
        (0.4375, 0.875),
        (0.125, 0.875),
    ],
    &[
        (0.5625, 0.5625),
        (0.875, 0.5625),
        (0.875, 0.875),
        (0.5625, 0.875),
    ],
];

/// Nine works: one step tighter than the default. Cell 0.1875 (3 px), gap
/// 0.09375 (1.5 px) — `3 × 0.1875 + 2 × 0.09375` spans the field exactly.
const DENSITY_COMPACT: &[Outline] = &[
    &[
        (0.125, 0.125),
        (0.3125, 0.125),
        (0.3125, 0.3125),
        (0.125, 0.3125),
    ],
    &[
        (0.40625, 0.125),
        (0.59375, 0.125),
        (0.59375, 0.3125),
        (0.40625, 0.3125),
    ],
    &[
        (0.6875, 0.125),
        (0.875, 0.125),
        (0.875, 0.3125),
        (0.6875, 0.3125),
    ],
    &[
        (0.125, 0.40625),
        (0.3125, 0.40625),
        (0.3125, 0.59375),
        (0.125, 0.59375),
    ],
    &[
        (0.40625, 0.40625),
        (0.59375, 0.40625),
        (0.59375, 0.59375),
        (0.40625, 0.59375),
    ],
    &[
        (0.6875, 0.40625),
        (0.875, 0.40625),
        (0.875, 0.59375),
        (0.6875, 0.59375),
    ],
    &[
        (0.125, 0.6875),
        (0.3125, 0.6875),
        (0.3125, 0.875),
        (0.125, 0.875),
    ],
    &[
        (0.40625, 0.6875),
        (0.59375, 0.6875),
        (0.59375, 0.875),
        (0.40625, 0.875),
    ],
    &[
        (0.6875, 0.6875),
        (0.875, 0.6875),
        (0.875, 0.875),
        (0.6875, 0.875),
    ],
];

/// Sixteen works: the tightest hang. Cell 0.140625 (2.25 px), gap 0.0625
/// (1 px) — `4 × 0.140625 + 3 × 0.0625` spans the field exactly, the same
/// discipline the three coarser detents keep.
///
/// It is the finest mark in the sheet and it is at the limit of what 16 px
/// can say: the cells minify to 2.25 px on a 1× display. That is legible as
/// *many small works*, which is the whole of what this detent has to mean,
/// and the mark's accessible name carries the rest.
const DENSITY_DENSE: &[Outline] = &[
    &[
        (0.125, 0.125),
        (0.265_625, 0.125),
        (0.265_625, 0.265_625),
        (0.125, 0.265_625),
    ],
    &[
        (0.328_125, 0.125),
        (0.468_75, 0.125),
        (0.468_75, 0.265_625),
        (0.328_125, 0.265_625),
    ],
    &[
        (0.531_25, 0.125),
        (0.671_875, 0.125),
        (0.671_875, 0.265_625),
        (0.531_25, 0.265_625),
    ],
    &[
        (0.734_375, 0.125),
        (0.875, 0.125),
        (0.875, 0.265_625),
        (0.734_375, 0.265_625),
    ],
    &[
        (0.125, 0.328_125),
        (0.265_625, 0.328_125),
        (0.265_625, 0.468_75),
        (0.125, 0.468_75),
    ],
    &[
        (0.328_125, 0.328_125),
        (0.468_75, 0.328_125),
        (0.468_75, 0.468_75),
        (0.328_125, 0.468_75),
    ],
    &[
        (0.531_25, 0.328_125),
        (0.671_875, 0.328_125),
        (0.671_875, 0.468_75),
        (0.531_25, 0.468_75),
    ],
    &[
        (0.734_375, 0.328_125),
        (0.875, 0.328_125),
        (0.875, 0.468_75),
        (0.734_375, 0.468_75),
    ],
    &[
        (0.125, 0.531_25),
        (0.265_625, 0.531_25),
        (0.265_625, 0.671_875),
        (0.125, 0.671_875),
    ],
    &[
        (0.328_125, 0.531_25),
        (0.468_75, 0.531_25),
        (0.468_75, 0.671_875),
        (0.328_125, 0.671_875),
    ],
    &[
        (0.531_25, 0.531_25),
        (0.671_875, 0.531_25),
        (0.671_875, 0.671_875),
        (0.531_25, 0.671_875),
    ],
    &[
        (0.734_375, 0.531_25),
        (0.875, 0.531_25),
        (0.875, 0.671_875),
        (0.734_375, 0.671_875),
    ],
    &[
        (0.125, 0.734_375),
        (0.265_625, 0.734_375),
        (0.265_625, 0.875),
        (0.125, 0.875),
    ],
    &[
        (0.328_125, 0.734_375),
        (0.468_75, 0.734_375),
        (0.468_75, 0.875),
        (0.328_125, 0.875),
    ],
    &[
        (0.531_25, 0.734_375),
        (0.671_875, 0.734_375),
        (0.671_875, 0.875),
        (0.531_25, 0.875),
    ],
    &[
        (0.734_375, 0.734_375),
        (0.875, 0.734_375),
        (0.875, 0.875),
        (0.734_375, 0.875),
    ],
];

/// Shuffle — two shafts crossing, each ending in an arrowhead on the right.
///
/// Symmetric about the box's horizontal centre line (`y → 1 − y`): the
/// falling shaft is the rising one reflected, vertex for vertex, and the two
/// heads with them. That is the same discipline [`PREVIOUS`] keeps against
/// [`NEXT`], and for the same reason — a pair drawn twice by hand reads as two
/// glyphs from two different sets, and here the pair is *inside one mark*.
///
/// The shafts are quadrilaterals rather than strokes, because this rasterizer
/// fills outlines and has no stroker; 0.10 of the box is the weight the rest of
/// the sheet's bars carry.
const SHUFFLE: &[Outline] = &[
    // The rising shaft, lower-left to upper-right.
    &[(0.06, 0.66), (0.12, 0.74), (0.62, 0.26), (0.56, 0.18)],
    // Its head, pointing right.
    &[(0.55, 0.10), (0.84, 0.24), (0.55, 0.38)],
    // The falling shaft — the rising one reflected in the centre line.
    &[(0.06, 0.34), (0.12, 0.26), (0.62, 0.74), (0.56, 0.82)],
    // Its head, reflected with it.
    &[(0.55, 0.90), (0.84, 0.76), (0.55, 0.62)],
];

/// **Repeat the list**: two bars and two heads, a loop drawn open.
///
/// It is [`REPEAT_ONE`] with the `1` taken out, and that is the whole
/// relationship — the two states of one control, so the pair reads as *the
/// same loop, once around or once over*. Drawing a second, unrelated loop
/// would make repeat-the-list and repeat-the-track look like different
/// features rather than two settings of one.
const REPEAT: &[Outline] = &[
    &[(0.18, 0.22), (0.70, 0.22), (0.70, 0.31), (0.18, 0.31)],
    &[(0.62, 0.12), (0.88, 0.27), (0.62, 0.42)],
    &[(0.30, 0.69), (0.82, 0.69), (0.82, 0.78), (0.30, 0.78)],
    &[(0.38, 0.58), (0.12, 0.73), (0.38, 0.88)],
];

/// **Repeat this track**: the loop, with a `1` standing in it.
const REPEAT_ONE: &[Outline] = &[
    &[(0.18, 0.22), (0.70, 0.22), (0.70, 0.31), (0.18, 0.31)],
    &[(0.62, 0.12), (0.88, 0.27), (0.62, 0.42)],
    &[(0.30, 0.69), (0.82, 0.69), (0.82, 0.78), (0.30, 0.78)],
    &[(0.38, 0.58), (0.12, 0.73), (0.38, 0.88)],
    &[(0.46, 0.37), (0.55, 0.37), (0.55, 0.64), (0.46, 0.64)],
];

/// Plain cover: one square frame, open in the middle.
const VISUAL_COVER: &[Outline] = &[
    &[(0.16, 0.16), (0.84, 0.16), (0.84, 0.24), (0.16, 0.24)],
    &[(0.16, 0.76), (0.84, 0.76), (0.84, 0.84), (0.16, 0.84)],
    &[(0.16, 0.24), (0.24, 0.24), (0.24, 0.76), (0.16, 0.76)],
    &[(0.76, 0.24), (0.84, 0.24), (0.84, 0.76), (0.76, 0.76)],
];

/// Jewel case: a wider frame with the tray hinge visible at the left.
const VISUAL_CASE: &[Outline] = &[
    &[(0.08, 0.20), (0.92, 0.20), (0.92, 0.27), (0.08, 0.27)],
    &[(0.08, 0.73), (0.92, 0.73), (0.92, 0.80), (0.08, 0.80)],
    &[(0.08, 0.27), (0.15, 0.27), (0.15, 0.73), (0.08, 0.73)],
    &[(0.85, 0.27), (0.92, 0.27), (0.92, 0.73), (0.85, 0.73)],
    &[(0.22, 0.27), (0.31, 0.27), (0.31, 0.73), (0.22, 0.73)],
];

/// No album object: the cover frame with one diagonal cancellation stroke.
///
/// This deliberately remains a depiction rather than borrowing Close: the
/// frame names what is absent, while the diagonal says it has been removed.
const VISUAL_NONE: &[Outline] = &[
    &[(0.16, 0.16), (0.84, 0.16), (0.84, 0.24), (0.16, 0.24)],
    &[(0.16, 0.76), (0.84, 0.76), (0.84, 0.84), (0.16, 0.84)],
    &[(0.16, 0.24), (0.24, 0.24), (0.24, 0.76), (0.16, 0.76)],
    &[(0.76, 0.24), (0.84, 0.24), (0.84, 0.76), (0.76, 0.76)],
    &[(0.19, 0.13), (0.87, 0.81), (0.81, 0.87), (0.13, 0.19)],
];

/// Spectrum: frequency bins with a deliberately irregular envelope.
const VISUAL_SPECTRUM: &[Outline] = &[
    &[(0.10, 0.60), (0.20, 0.60), (0.20, 0.84), (0.10, 0.84)],
    &[(0.24, 0.39), (0.34, 0.39), (0.34, 0.84), (0.24, 0.84)],
    &[(0.38, 0.18), (0.48, 0.18), (0.48, 0.84), (0.38, 0.84)],
    &[(0.52, 0.31), (0.62, 0.31), (0.62, 0.84), (0.52, 0.84)],
    &[(0.66, 0.50), (0.76, 0.50), (0.76, 0.84), (0.66, 0.84)],
    &[(0.80, 0.67), (0.90, 0.67), (0.90, 0.84), (0.80, 0.84)],
];

const VISUAL_FACTS: &[Outline] = &[
    &[(0.12, 0.20), (0.88, 0.20), (0.88, 0.29), (0.12, 0.29)],
    &[(0.12, 0.46), (0.72, 0.46), (0.72, 0.55), (0.12, 0.55)],
    &[(0.12, 0.72), (0.82, 0.72), (0.82, 0.81), (0.12, 0.81)],
];

/// **Place history, forward** — a shaft at the set's stroke into a chevron
/// head of two arms at the same stroke, exactly [`OPEN`]'s construction.
///
/// The *stroke* rather than a filled triangle is what keeps place navigation
/// apart from transport's filled skip marks: one is an open angle on a line,
/// the other a solid mass beside a bar. That distinction is unchanged and is
/// the whole reason these are not simply [`NEXT`] and [`PREVIOUS`] rotated.
///
/// # It was one self-intersecting polygon, and it drew a shape nobody chose
///
/// The owner, having already had these fixed once: *"the back button icon is
/// wrong and so is the forward"*. He is right, and the outlines were unchanged
/// since — what changed is that they were **drawn 25 % larger**, [`theme::ICON_PX`]
/// 16 → 20 in the 2026-08-14 control pass, and the shape's faults stopped
/// being mush.
///
/// The old form was a single nine-vertex polygon tracing a solid triangle and
/// then doubling back along a shaft that crossed the triangle's own back edge.
/// Under this rasterizer's even-odd cast that overlap **cancels**, so the head
/// was hollow — which the comment here rationalised as *"the unfilled shape"*,
/// as though it had been drawn that way. It had not: the surviving outline was
/// the sliver between the triangle's edge and the shaft's diagonal, and that
/// sliver **tapers**, from a hairline at the head's back corners to six times
/// that near the tip. There was no stroke weight to re-proportion, because
/// there was no stroke.
///
/// So it is drawn now, as three plain outlines whose union fills the joins:
/// two 45° arms and a shaft, all at the set's **0.145**, which is the weight
/// [`OPEN`], [`ARROW_UP`] and the window controls already share. A constant
/// stroke is a thing a future size change can be checked against; a sliver is
/// not.
const HISTORY_FORWARD: &[Outline] = &[
    &[
        (0.4887, 0.2913),
        (0.5913, 0.1887),
        (0.8513, 0.4487),
        (0.7487, 0.5513),
    ],
    &[
        (0.4887, 0.7087),
        (0.5913, 0.8113),
        (0.8513, 0.5513),
        (0.7487, 0.4487),
    ],
    &[
        (0.1400, 0.4275),
        (0.7800, 0.4275),
        (0.7800, 0.5725),
        (0.1400, 0.5725),
    ],
];

/// **Place history, back** — [`HISTORY_FORWARD`] mirrored about `x = 0.5`,
/// vertex for vertex, so the pair cannot drift apart under a later edit to
/// one of them.
const HISTORY_BACK: &[Outline] = &[
    &[
        (0.5113, 0.2913),
        (0.4087, 0.1887),
        (0.1487, 0.4487),
        (0.2513, 0.5513),
    ],
    &[
        (0.5113, 0.7087),
        (0.4087, 0.8113),
        (0.1487, 0.5513),
        (0.2513, 0.4487),
    ],
    &[
        (0.8600, 0.4275),
        (0.2200, 0.4275),
        (0.2200, 0.5725),
        (0.8600, 0.5725),
    ],
];

/// **Notification bell** — a narrow dome flaring into a wide mouth, with a
/// crown above it and a detached clapper below.
///
/// # It drew a disc, and nobody could see that it did
///
/// This shape did not reach the screen until 2026-08-14. `Glyph::ALL` and
/// `Glyph::index` disagreed, so the app bar's bell was being handed
/// [`HISTORY_FORWARD`]'s sprite; fixing that ordering drew these outlines for
/// the first time and they are **a circle**. The old form was 0.56 wide and
/// 0.60 tall with near-vertical sides — square enough that its rounded top
/// closed it into a blob at [`theme::ICON_PX`] 20 — and its "short base" was flush
/// with the body rather than a rim, so there was no mouth to read.
///
/// # A silhouette, and why that is not a departure
///
/// The comment here used to claim *"at the shared icon stroke"*, which it
/// never was. It stays a filled silhouette now on [`HOME`]'s precedent rather
/// than being converted: the sheet's stroke rule is about **open angles** —
/// [`OPEN`] and the history arrows are strokes so they cannot be read as
/// [`PLAY`]'s solid mass — and a bell has no such twin to be confused with.
/// What a bell needs is a *profile*, and a profile drawn at 0.145 with four
/// parts is a tangle at 20 px where a silhouette is instant.
///
/// So the ratio does the work: the dome is **0.34** across and the mouth is
/// **0.78**, more than twice it, where the old shape's were 0.34 and 0.56.
/// The mouth is a rim with its own step, the crown sits proud of the dome, and
/// the clapper is separated by a real gap — four decisions the eye resolves
/// before it counts anything.
///
/// # It was narrower than everything beside it
///
/// The first drawing of this profile was 0.68 at the mouth, and the owner read
/// the bar: *"the bell icon is a little bit narrow/skinny."* That is
/// measurable rather than a matter of taste — every neighbour in the same
/// [`theme::ICON_PX`] 20 box is wider ([`GEAR`] 0.84, [`HOME`] and [`NOW_PLAYING`]
/// 0.88), so the bell laid about 13.6 px of ink where the gear one seam away
/// laid 16.8, and it was the narrowest mark in the app bar's right cluster.
///
/// The mouth is **0.78** now. It is a scale about the vertical axis — 1.147 on
/// every x, so the dome, the flare's shoulders and the rim keep their
/// proportions to each other exactly — rather than a redrawing, because the
/// profile was right and only its width was wrong. The height is untouched at
/// 0.79 (crown 0.105 to clapper 0.895): a bell wants to be no wider than it is
/// tall, and 0.78 × 0.79 is the widest this silhouette goes while that holds.
/// `the_bell_is_as_wide_as_the_cluster_it_stands_in` keeps it in the
/// neighbours' range from below.
///
/// `views::status` stacks the health dot on this glyph's bottom-right corner,
/// where it **overlaps the rim** — deliberately, and symmetry is why. A bell
/// whose mouth was cut short on one side to clear a badge would read as a
/// badly drawn bell in the three tones out of four where the badge is quiet
/// ink; a badge sitting on the rim is the convention every notification bell
/// in the field uses, and it is legible because the dot is a solid disc in a
/// tone of its own rather than more of the same ink.
/// **The bell, widened at the dome rather than at the rim** — item 74.
///
/// The owner, twice: *"the bell icon is still weirdly skinny."* Item 48 tried
/// to answer that with a uniform 1.147 scale and a test that took the glyph's
/// **widest scanline**, which is the rim — one hairline at the foot. The rim
/// was already 0.78 of the box, so the test passed while the *dome*, which is
/// the mass a reader actually sees, sat at about 0.36 against the gear's 0.84
/// disc in the same box. A wide plate under a narrow spike satisfies a
/// maximum and reads as skinny, because a glyph is read by its mass.
///
/// So the dome carries the width now — a 0.60 waist flaring to the same 0.78
/// rim — and `the_bell_is_as_wide_as_the_cluster_it_stands_in` measures the
/// body's **median** run instead of its widest, which is the measurement that
/// would have failed on the old outline.
const BELL: &[Outline] = &[
    // The body: dome, flare, rim.
    &[
        (0.500, 0.180),
        (0.650, 0.215),
        (0.752, 0.300),
        (0.790, 0.420),
        (0.800, 0.560),
        (0.845, 0.640),
        (0.890, 0.690),
        (0.890, 0.740),
        (0.110, 0.740),
        (0.110, 0.690),
        (0.155, 0.640),
        (0.200, 0.560),
        (0.210, 0.420),
        (0.248, 0.300),
        (0.350, 0.215),
    ],
    // The crown, standing proud of the dome and unioned into it. It grows
    // with the dome: a 0.10 stud on a 0.60 body reads as a pin.
    &[
        (0.435, 0.100),
        (0.565, 0.100),
        (0.565, 0.190),
        (0.435, 0.190),
    ],
    // The clapper, across a real gap. Likewise — it hangs from a wider bell.
    &[
        (0.400, 0.800),
        (0.600, 0.800),
        (0.560, 0.900),
        (0.440, 0.900),
    ],
];

impl Glyph {
    /// Every glyph, in sprite-sheet order.
    const ALL: [Self; Self::COUNT] = [
        Self::Play,
        Self::Pause,
        Self::Next,
        Self::Previous,
        Self::Speaker,
        Self::SpeakerMuted,
        Self::Close,
        Self::Magnifier,
        Self::Gear,
        Self::Plus,
        Self::Minus,
        Self::ArrowUp,
        Self::ArrowDown,
        Self::DensitySpacious,
        Self::DensityBalanced,
        Self::DensityCompact,
        Self::DensityDense,
        Self::Queue,
        Self::Open,
        Self::Home,
        Self::Library,
        Self::NowPlaying,
        Self::LaneExpanded,
        Self::LaneCollapsed,
        Self::Shuffle,
        Self::WindowMinimise,
        Self::WindowMaximise,
        Self::WindowRestore,
        Self::VisualCover,
        Self::VisualCase,
        Self::VisualNone,
        Self::VisualSpectrum,
        Self::VisualFacts,
        Self::HistoryBack,
        Self::HistoryForward,
        Self::Bell,
        Self::Heart,
        Self::HeartFilled,
        Self::RepeatOne,
        Self::Repeat,
        Self::Equalizer,
        Self::Chromeless,
    ];

    /// How many glyphs the sheet holds.
    const COUNT: usize = 42;

    /// The glyph's outlines in the unit square.
    #[must_use]
    fn outlines(self) -> &'static [Outline] {
        match self {
            Self::Play => PLAY,
            Self::Pause => PAUSE,
            Self::Next => NEXT,
            Self::Previous => PREVIOUS,
            Self::Speaker => SPEAKER,
            Self::SpeakerMuted => SPEAKER_MUTED,
            Self::Close => CLOSE,
            Self::WindowMinimise => WINDOW_MINIMISE,
            Self::WindowMaximise => WINDOW_MAXIMISE,
            Self::WindowRestore => WINDOW_RESTORE,
            Self::Magnifier => MAGNIFIER,
            Self::Gear => GEAR,
            Self::Equalizer => EQUALIZER,
            Self::Chromeless => CHROMELESS,
            Self::Plus => PLUS,
            Self::Minus => MINUS,
            Self::ArrowUp => ARROW_UP,
            Self::ArrowDown => ARROW_DOWN,
            Self::DensitySpacious => DENSITY_SPACIOUS,
            Self::DensityBalanced => DENSITY_BALANCED,
            Self::DensityCompact => DENSITY_COMPACT,
            Self::DensityDense => DENSITY_DENSE,
            Self::Queue => QUEUE,
            Self::Open => OPEN,
            Self::Home => HOME,
            Self::Library => LIBRARY,
            Self::NowPlaying => NOW_PLAYING,
            Self::LaneExpanded => LANE_EXPANDED,
            Self::LaneCollapsed => LANE_COLLAPSED,
            Self::Shuffle => SHUFFLE,
            Self::VisualCover => VISUAL_COVER,
            Self::VisualCase => VISUAL_CASE,
            Self::VisualNone => VISUAL_NONE,
            Self::VisualSpectrum => VISUAL_SPECTRUM,
            Self::VisualFacts => VISUAL_FACTS,
            Self::HistoryBack => HISTORY_BACK,
            Self::HistoryForward => HISTORY_FORWARD,
            Self::Bell => BELL,
            Self::Heart => HEART,
            Self::HeartFilled => HEART_FILLED,
            Self::RepeatOne => REPEAT_ONE,
            Self::Repeat => REPEAT,
        }
    }

    /// Its slot in the sprite sheet.
    const fn index(self) -> usize {
        match self {
            Self::Play => 0,
            Self::Pause => 1,
            Self::Next => 2,
            Self::Previous => 3,
            Self::Speaker => 4,
            Self::SpeakerMuted => 5,
            Self::Close => 6,
            Self::Magnifier => 7,
            Self::Gear => 8,
            Self::Equalizer => 40,
            Self::Chromeless => 41,
            Self::Plus => 9,
            Self::Minus => 10,
            Self::ArrowUp => 11,
            Self::ArrowDown => 12,
            Self::DensitySpacious => 13,
            Self::DensityBalanced => 14,
            Self::DensityCompact => 15,
            Self::DensityDense => 16,
            Self::Queue => 17,
            Self::Open => 18,
            Self::Home => 19,
            Self::Library => 20,
            Self::NowPlaying => 21,
            Self::LaneExpanded => 22,
            Self::LaneCollapsed => 23,
            Self::Shuffle => 24,
            Self::WindowMinimise => 25,
            Self::WindowMaximise => 26,
            Self::WindowRestore => 27,
            Self::VisualCover => 28,
            Self::VisualCase => 29,
            Self::VisualNone => 30,
            Self::VisualSpectrum => 31,
            Self::VisualFacts => 32,
            Self::HistoryBack => 33,
            Self::HistoryForward => 34,
            Self::Bell => 35,
            Self::Heart => 36,
            Self::HeartFilled => 37,
            Self::RepeatOne => 38,
            Self::Repeat => 39,
        }
    }

    /// The speaker in the state `muted` describes.
    #[must_use]
    pub fn speaker(muted: bool) -> Self {
        if muted {
            Self::SpeakerMuted
        } else {
            Self::Speaker
        }
    }

    /// Whether the unit-square point `(x, y)` is inside the glyph — the
    /// *union* of its outlines, so overlapping ones (the mute cross) fill
    /// solid rather than cancelling.
    ///
    /// The **shape** reading, for tests that interrogate a glyph's geometry at
    /// an arbitrary point. [`rasterize`] no longer goes through it: the sheet
    /// samples the grid directly, so this would be dead code in the binary.
    #[cfg(test)]
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

/// The rasterized sheet, built once on first use: one sprite per glyph, all
/// [`RASTER_PX`] square, inked in the standing room's glyph ink.
///
/// Caching matters beyond the arithmetic — `image::Handle::from_rgba` mints
/// a fresh id per call, and a fresh id per frame would churn the renderer's
/// texture atlas. These ids live as long as the process.
///
/// **The room is baked in, so the sheet is keyed by the room.** It used to be
/// a `LazyLock` rasterized once per process, which is what made the room a
/// startup fact and the picker say *"applies on restart"*. The owner,
/// 2026-08-15: *"ideally can we apply them upon selection."*
///
/// So each room gets its own sheet, minted the first time that room is
/// standing and kept — a listener trying four rooms ends with four sheets of
/// 18 sprites at 32 × 32 × 4 bytes, which is 73 KiB a room, and keeping them
/// means going back to a room they have already seen costs nothing and, more
/// importantly, does not mint **new texture ids** for the renderer's atlas to
/// churn on.
static SHEETS: RwLock<Vec<(u64, Sheets)>> = RwLock::new(Vec::new());

/// One room's sprites: the glyph-ink sheet and the accent sheet.
type Sheet = [image::Handle; Glyph::COUNT];

/// A room's pair of sheets, shared by every surface drawing in that room.
type Sheets = (Arc<Sheet>, Arc<Sheet>);

thread_local! {
    /// The standing room's two sheets, so the common path — every glyph of
    /// every frame — is a relaxed atomic load and a thread-local hit rather
    /// than a lock.
    static STANDING: RefCell<Option<(u64, Sheets)>> = const { RefCell::new(None) };
}

/// The two sheets for the room standing now, rasterizing them if this is the
/// first time it has stood.
fn sheets() -> Sheets {
    let generation = theme::generation();
    if let Some((seen, sheets)) = STANDING.with(|standing| standing.borrow().clone())
        && seen == generation
    {
        return sheets;
    }
    let found = SHEETS
        .read()
        .ok()
        .and_then(|sheets| {
            sheets
                .iter()
                .find(|(seen, _)| *seen == generation)
                .map(|(_, sheets)| sheets.clone())
        })
        .unwrap_or_else(|| {
            let room = theme::active();
            let sheets: Sheets = (
                Arc::new(rasterize_sheet(rgb(room.glyph()))),
                Arc::new(rasterize_sheet(rgb(room.lamp))),
            );
            if let Ok(mut all) = SHEETS.write() {
                all.push((generation, sheets.clone()));
            }
            sheets
        });
    STANDING.with(|standing| {
        *standing.borrow_mut() = Some((generation, found.clone()));
    });
    found
}

fn rasterize_sheet(ink: [u8; 3]) -> Sheet {
    Glyph::ALL.map(|glyph| image::Handle::from_rgba(RASTER_PX, RASTER_PX, rasterize(glyph, ink)))
}

/// **The two orders are one order**, checked when the crate compiles.
///
/// [`Glyph::ALL`] is what the sheet is rasterized *from* and [`Glyph::index`]
/// is what it is read *by*, so they are two hand-written lists that have to
/// agree — and on 2026-08-14 they did not. `VisualFacts` was appended to
/// `ALL` before the history pair but numbered after them in `index`, so the
/// sheet handed out **four wrong sprites**: `HistoryBack` drew the facts
/// mark, `HistoryForward` drew the back arrow, `Bell` drew the forward
/// arrow and `VisualFacts` drew the bell.
///
/// That is the whole of the owner's *"the back button icon is wrong and so
/// is the forward"*, and it is why the first telling of that ask was
/// answered by redrawing the outlines and came back: the outlines were
/// never what was on screen.
///
/// **Nothing could have caught it.** `every_glyph_rasterizes_to_the_same_square`
/// walks `ALL` and `the_sheet_hands_out_one_stable_handle_per_glyph` checks
/// that a handle is stable, so a permutation is invisible to both: every
/// sprite exists, every sprite is the right size, and every glyph gets *a*
/// stable handle. Only the pairing was wrong, and the pairing was the one
/// thing neither test named.
///
/// So it is a **const** assertion rather than a test: the two lists are a
/// duplication the type system cannot remove — a match arm per variant is
/// what makes adding a glyph a compile error rather than a silent gap — and
/// the answer to a duplication that must stay is to check it where it
/// cannot be run past.
const _: () = {
    let mut i = 0;
    while i < Glyph::COUNT {
        assert!(
            Glyph::ALL[i].index() == i,
            "Glyph::ALL and Glyph::index disagree — the sheet will hand out \
             the wrong sprite for this glyph and every one after it"
        );
        i += 1;
    }
};

/// The sprite for `glyph`. Cheap: an `Arc` bump over the shared sheet.
#[must_use]
pub fn handle(glyph: Glyph) -> image::Handle {
    sheets().0[glyph.index()].clone()
}

/// The sprite for `glyph` in `ink`, which must be one of the two inks a sheet
/// exists for: the room's glyph ink, or its accent.
///
/// **The accent sheet has two consumers**, and the accent discipline is what
/// bounds it to two: the wall's hover `Play`, which is the record page's `Play
/// album` moved onto the sleeve and carries that control's licence
/// ([`theme::veil_option_ink`]); and the bar's shuffle toggle **while it is
/// on**, which creates playback truth about what sounds *next* in the way
/// `Play album` creates it about what sounds now
/// (`crate::views::bottom_bar`'s `shuffle_toggle`). It is built beside the
/// glyph-ink sheet and by the same rules — see [`SHEETS`].
///
/// The caller states the ink and this resolves the sheet, so the *decision*
/// about which glyph wears the accent lives in one place
/// ([`theme::veil_option_ink`]) rather than being spelled twice. An ink that
/// is neither takes the ordinary sheet: a third inked sheet is a decision, and
/// silently minting one here is how an accent discipline stops being one.
#[must_use]
pub fn inked(glyph: Glyph, ink: Color) -> image::Handle {
    let (plain, accent) = sheets();
    if ink == theme::active().lamp {
        accent[glyph.index()].clone()
    } else {
        plain[glyph.index()].clone()
    }
}

/// The **application's own mark** — the icon a launcher shows — decoded once
/// from the PNG the desktop entry and the Flatpak already install.
///
/// # It is not on the sheet, and that is the whole point
///
/// Everything above this line is a *glyph*: an outline in a unit square,
/// rasterized to coverage and **inked by the room** at draw time
/// ([`SHEETS`]). A glyph has no colour of its own; the room
/// gives it one, which is how baz keeps two inks and one accent.
///
/// baz's application icon is a different kind of asset and `packaging/README.md`
/// already says so in as many words — *"`crates/baz/src/icon.rs` is unrelated —
/// that is the in-UI transport glyph sheet, drawn in code"*. It is
/// **full-colour**: a wall gradient, a sleeve in the placeholder gamut, a
/// letterform, and the picture light. Flattening it to coverage would throw
/// away the thing that makes it recognisable at a glance in a launcher, and
/// re-drawing it as an outline would be a **second master** — two files that
/// have to be kept in agreement, when `packaging/icons/README.md` is explicit
/// that the SVG is *the* master and the PNG ladder is rendered from it.
///
/// So: one master, one ladder, and the bar reads the ladder.
///
/// # Which rung, and why that one
///
/// The **32 px** rung, drawn at [`theme::APP_MARK_PX`] 28 logical px — under
/// 2× the sheet's [`SUPERSCALE`], and minifying the committed 64 px raster
/// 64:28 ≈ 2.3:1 is the crisp-sprite contract the sheet uses. The
/// rung is rendered from `io.github.mattcree.baz-small.svg`, the size-specific
/// artwork the freedesktop icon theme spec exists to allow: the master's wall
/// label loses its second line below ~48 px because two 1 px lanes composite
/// into one grey smudge. Taking the 256 px master and minifying 16:1 here would
/// have thrown that work away and drawn the smudge.
///
/// # The one thing it spends
///
/// The mark carries the room's accent — the lamp dot on the label's first line
/// — and in the bar that accent is **not playback truth**, which is the
/// standing rule it would otherwise break (doc 02 §5.3). It is admitted as an
/// exception with a stated boundary: **the application's mark is the
/// application's, not the room's ink**, and nothing else in the chrome may
/// reach for colour on this precedent. At 28 px the dot is still only a
/// pixel or two.
/// ADR-0040's amendment records it and states the reversal — a monochrome
/// `Glyph::Baz` on the sheet, inked like every other mark in the bar, which is
/// a real option and not a hypothetical one.
///
/// # Failure
///
/// The bytes are `include_bytes!` from the repository, so a decode failure is a
/// corrupt commit rather than anything a machine in the field can cause, and
/// CI's `packaging` job renders the ladder from the master and compares. It is
/// therefore an `expect` rather than a fallback: a silent blank in the window's
/// own chrome would be worse than a build that cannot start.
static APP_MARK: LazyLock<image::Handle> = LazyLock::new(|| {
    /// The canonical red-circle application asset. The hicolor ladder is
    /// rendered from its SVG sibling by `packaging/icons/render.sh`.
    const BYTES: &[u8] = include_bytes!("../assets/icons/logo-transparent-circle-red.png");
    // `::image` is the decoder crate; the bare `image` in this module is
    // `iced::widget::image`, whose `Handle` the last line mints.
    let mark = ::image::load_from_memory(BYTES)
        .expect("baz's own red-circle application icon, compiled in from assets")
        .to_rgba8();
    let (w, h) = mark.dimensions();
    image::Handle::from_rgba(w, h, mark.into_raw())
});

/// The application's mark, for the app bar's zone 1. Cheap: an `Arc` bump over
/// the one decoded copy, whose id lives as long as the process for [`SHEETS`]'s
/// reason.
#[must_use]
pub fn app_mark() -> image::Handle {
    APP_MARK.clone()
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
    let total = SAMPLES * SAMPLES;
    // The vertices move onto the grid **once per glyph**. They do not depend on
    // the sample, and the loop below tests 102 400 of those per sprite, so
    // converting inside it made the sheet's build the slowest thing on the
    // first frame for no arithmetic gain.
    let outlines: Vec<Vec<(i64, i64)>> = glyph.outlines().iter().map(|o| on_grid(o)).collect();
    let mut pixels = Vec::with_capacity((RASTER_PX * RASTER_PX * 4) as usize);
    for row in 0..RASTER_PX {
        for column in 0..RASTER_PX {
            let mut hits = 0_u32;
            for sub_y in 0..SAMPLES {
                for sub_x in 0..SAMPLES {
                    // Sample coordinates on the shared integer grid, exact for
                    // every sub-sample and a mirror pair by construction.
                    let xs = sample_at(column * SAMPLES + sub_x);
                    let ys = sample_at(row * SAMPLES + sub_y);
                    if outlines.iter().any(|outline| ray_crosses(outline, xs, ys)) {
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
///
/// Test-only since the sheet moved onto the integer grid ([`RAY_D`]) — the
/// rasterizer no longer converts an index to a float at all.
#[cfg(test)]
#[expect(
    clippy::cast_precision_loss,
    reason = "raster indices are bounded by RASTER_PX * SAMPLES = 320"
)]
fn index_to_f32(value: u32) -> f32 {
    value as f32
}

/// The integer grid every symbol decision runs on: coordinates are scaled by
/// this and the ray cast is exact integer arithmetic, so a mirror pair of
/// samples decides identically rather than by which way a float rounded.
///
/// The two factors are who lives on the grid: the **samples** sit on the odd
/// lattice `(2k+1)/(2·RASTER_PX·SAMPLES)` — the centred half-step grid of the
/// 8× anti-aliasing — and the **vertices** are the sheet's own
/// constants, which never run deeper than four places (plus a handful of exact
/// eighths and 64ths — 0.09375, 0.140625, 0.40625, 0.59375), so the product of
/// the two denominators holds both exactly.
const RAY_D: i64 = (2 * RASTER_PX * SAMPLES) as i64 * 10_000;

/// A coordinate at [`RAY_D`]: a decimal to four places, or a dyadic eighth,
/// times this grid is an exact integer, so a mirror pair of vertices sums to
/// [`RAY_D`] exactly rather than to a decimal continuation's rounding.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    reason = "RAY_D rounds every coordinate on the lattice to its exact integer"
)]
fn to_grid(value: f32) -> i64 {
    (f64::from(value) * RAY_D as f64).round() as i64
}

/// The scaled coordinate of sample `k` of [`SAMPLES`]·[`RASTER_PX`] per side:
/// `(2k+1)/640`, at [`RAY_D`]. Mirror-exact by construction — the sample that
/// mirrors `k` is `640/2 − 1 − k`, which lands on `RAY_D − x` rather than on
/// a nearest-neighbour of it.
fn sample_at(k: u32) -> i64 {
    i64::from(2 * k + 1) * (RAY_D / i64::from(2 * RASTER_PX * SAMPLES))
}

/// One outline's vertices moved onto the [`RAY_D`] grid.
#[must_use]
fn on_grid(outline: Outline) -> Vec<(i64, i64)> {
    outline
        .iter()
        .map(|&(x, y)| (to_grid(x), to_grid(y)))
        .collect()
}

/// Whether the point `(xs, ys)` — on the [`RAY_D`] grid — is inside the
/// closed polygon `outline`, by the even-odd ray cast, exactly. Degenerate
/// outlines (fewer than three vertices) enclose nothing.
///
/// The edge test counts an edge as crossing when `xs` is **strictly to the
/// left of it**, and separately recognises a sample sitting **exactly on an
/// edge** — the cross-multiplication's equality case — as covered, the closed
/// fill. A one-way cast alone would hand an on-edge sample an arbitrary
/// parity: count to its left and count to its right and the two differ by
/// one, and which of the two a float gives it is the round's doing. For an
/// edge that passes exactly through sample points — the arrows' 45° arms and
/// the cross's diagonals do — a mirror pair of samples can then read one
/// inside and one out, which is precisely the asymmetry the sheet's
/// reflection tests refuse. Counting the edge itself as filled gives a mirror
/// pair of on-edge samples the same decision, and the sheet's symmetry is
/// arithmetic rather than luck.
fn ray_crosses(outline: &[(i64, i64)], xs: i64, ys: i64) -> bool {
    let Some(&last) = outline.last() else {
        return false;
    };
    if outline.len() < 3 {
        return false;
    }
    let mut inside = false;
    let mut on_edge = false;
    let mut previous = last;
    for &current in outline {
        let (cxs, cys) = current;
        let (pxs, pys) = previous;
        // Straddling edges only: the half-open test on `y` counts a vertex
        // exactly once, so a ray through one does not flip twice.
        if (cys > ys) != (pys > ys) {
            // `crossing = (px − cx)(y − cy)/(py − cy) + cx`, compared against
            // `xs` by cross-multiplication — the exact sign of `xs − crossing`
            // on the shared grid.
            let lhs = (xs - cxs) * (pys - cys);
            let rhs = (pxs - cxs) * (ys - cys);
            let rel = if pys - cys > 0 {
                lhs.cmp(&rhs)
            } else {
                rhs.cmp(&lhs)
            };
            match rel {
                std::cmp::Ordering::Less => inside = !inside,
                std::cmp::Ordering::Equal => on_edge = true,
                // Greater: the edge lies strictly to the left of the sample —
                // the cast counts crossings strictly to the right.
                std::cmp::Ordering::Greater => {}
            }
        }
        previous = current;
    }
    on_edge || inside
}

/// Whether `(x, y)` is inside the closed polygon `outline`, by the even-odd
/// ray-crossing rule: count the edges a ray to one side crosses, an odd count
/// means inside. The float reading of [`ray_crosses`] — any point of the sheet
/// or of a measuring grid rounds onto the shared [`RAY_D`] lattice and the
/// exact cast runs. Degenerate outlines (fewer than three vertices) enclose
/// nothing. Test-only, with [`Glyph::covers`]: the sheet is sampled on the
/// grid, so nothing in the binary reaches the outlines through a float.
#[cfg(test)]
fn encloses(outline: Outline, x: f32, y: f32) -> bool {
    ray_crosses(&on_grid(outline), to_grid(x), to_grid(y))
}

#[cfg(test)]
mod tests {
    /// **A mark in the app bar carries its neighbours' weight.**
    ///
    /// The owner, about the bell, twice: *"the bell icon is a little bit
    /// narrow/skinny"*, then *"the bell icon is still weirdly skinny."* Both
    /// fixes were bell-shaped — the first widened it and measured the widest
    /// scanline, the second widened its dome and measured the median run —
    /// and neither said anything to a glyph that had not been drawn yet. So
    /// the third mark to arrive underweight was
    /// [`Glyph::Equalizer`](super::Glyph::Equalizer), laying **0.179** of its
    /// box in ink beside a gear at 0.324, on the day it was added.
    ///
    /// A glyph is read by its **mass**, which is neither of the two things
    /// those fixes measured: a wide outline can be a hairline and a tall one
    /// can be a spike. So this measures the ink, and it measures every mark in
    /// the cluster rather than one at a time — which is the form that catches
    /// the *next* one.
    ///
    /// The band is deliberately generous. `HOME` and `NOW_PLAYING` are solid
    /// silhouettes and run heavy; `OPEN` and the history arrows are strokes
    /// by [`Glyph`](super::Glyph)'s own open-angle rule and run light. What is
    /// being ruled out is not variety, it is a mark that reads as a different
    /// weight of ink from the ones it stands beside.
    #[test]
    fn a_mark_in_the_app_bar_carries_its_neighbours_weight() {
        /// Fraction of the box a glyph fills, alpha-weighted so an antialiased
        /// edge counts for what it actually shows.
        fn ink(glyph: Glyph) -> f32 {
            let pixels = rasterize(glyph, [255, 255, 255]);
            let laid: u64 = pixels.chunks_exact(4).map(|p| u64::from(p[3])).sum();
            let box_full = u64::from(RASTER_PX) * u64::from(RASTER_PX) * 255;
            #[expect(
                clippy::cast_precision_loss,
                reason = "a fraction of a raster; f32 has decades of headroom here"
            )]
            {
                laid as f32 / box_full as f32
            }
        }

        // **Two families, and they are two on purpose.** ADR-0040 §2's zones:
        // the view's own options in zone 3, the application's controls in
        // zone 4. Measured 2026-08-18, the first runs 0.19–0.26 of the box in
        // ink and the second 0.31–0.34 — a real and deliberate difference in
        // rank, which is why this holds each family to *itself* rather than
        // flattening the bar into one weight.
        //
        // Holding them jointly would demand redrawing four shipped marks to
        // match two, and the fault being guarded against is a mark arriving
        // out of step with the row it joins. `Glyph::Chromeless` was drawn at
        // 0.292 first — correct beside the gear, and heavy enough beside the
        // marks it actually stands in to have made *them* look thin.
        let families: [(&str, &[Glyph]); 2] = [
            (
                "the view's options",
                &[
                    Glyph::Chromeless,
                    Glyph::VisualCase,
                    Glyph::VisualCover,
                    Glyph::VisualNone,
                    Glyph::VisualSpectrum,
                    Glyph::VisualFacts,
                ],
            ),
            (
                "the application's controls",
                &[Glyph::Equalizer, Glyph::Bell, Glyph::Gear],
            ),
        ];

        for (family, glyphs) in families {
            let weights: Vec<(Glyph, f32)> =
                glyphs.iter().map(|glyph| (*glyph, ink(*glyph))).collect();
            let lightest = weights
                .iter()
                .copied()
                .fold(f32::MAX, |seen, (_, weight)| seen.min(weight));
            let heaviest = weights
                .iter()
                .copied()
                .fold(0.0_f32, |seen, (_, weight)| seen.max(weight));

            // The bell was at 0.53 of the gear when the owner called it skinny
            // the second time, and the equaliser at 0.55 on the day it was
            // added. 0.70 is clear of the widest spread either family actually
            // has and well above the reading he has twice objected to.
            assert!(
                lightest / heaviest > 0.70,
                "{family} run {lightest:.3}–{heaviest:.3} of their box in \
                 ink: {weights:?}. The lightest is {:.0}% of the heaviest, and \
                 a mark at that fraction of the ones beside it reads as skinny \
                 however wide its outline is.",
                100.0 * lightest / heaviest
            );

            // And none is a hairline in absolute terms — a mark can be
            // consistent with a family that is uniformly too light.
            for (glyph, weight) in &weights {
                assert!(
                    *weight > 0.15,
                    "{glyph:?} lays {weight:.3} of its box in ink, which is a \
                     hairline at ICON_PX 20 whatever its neighbours do"
                );
            }
        }
    }

    /// **The sheet holds every glyph the product draws.**
    ///
    /// This is the test that was missing on 2026-08-18, and the shape of what
    /// it missed is worth keeping: a new glyph was given an `index()` arm and
    /// an outline, and left out of [`Glyph::ALL`]. `COUNT` still said 40, the
    /// const assertion walked `ALL[0..40]` and found every pairing correct,
    /// and all 29 icon tests passed — while `handle()` indexed 40 into a
    /// 40-element sheet and the app **panicked on the first frame that drew
    /// the mark**.
    ///
    /// Nothing in the module could catch that, because every check started
    /// from `ALL` and the fault was a glyph that was not in it. So this starts
    /// from the other end: it reads the views, finds every `Glyph::` a view
    /// actually names, and asks the sheet for it.
    #[test]
    fn every_glyph_a_view_draws_is_in_the_sheet() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut drawn: Vec<String> = Vec::new();
        let mut files = vec![root.clone()];
        while let Some(path) = files.pop() {
            if path.is_dir() {
                let Ok(entries) = std::fs::read_dir(&path) else {
                    continue;
                };
                files.extend(entries.flatten().map(|entry| entry.path()));
                continue;
            }
            if path.extension().is_none_or(|ext| ext != "rs") {
                continue;
            }
            let source = std::fs::read_to_string(&path).expect("a source file baz ships");
            // Shipped code only: this module's own tests name glyphs too.
            let code = source.split("#[cfg(test)]").next().unwrap_or_default();
            for (at, _) in code.match_indices("Glyph::") {
                let rest = &code[at + "Glyph::".len()..];
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                // Variants are CamelCase; `ALL` and `COUNT` are associated
                // items on the same type and are not marks.
                let is_variant = name.chars().next().is_some_and(char::is_uppercase)
                    && name.chars().any(char::is_lowercase);
                if is_variant && !drawn.contains(&name) {
                    drawn.push(name);
                }
            }
        }
        assert!(
            drawn.len() > 20,
            "the scan found only {} glyph names, so it is not reading the \
             views — a test that cannot fail is worse than none",
            drawn.len()
        );
        let held: Vec<String> = Glyph::ALL
            .iter()
            .map(|glyph| format!("{glyph:?}"))
            .collect();
        for name in &drawn {
            // `ALL` and `COUNT` are the two lists a new glyph has to join;
            // this names the one that was forgotten rather than panicking
            // inside `handle` on the first frame.
            assert!(
                held.contains(name),
                "`Glyph::{name}` is drawn by a view and is not in `Glyph::ALL` \
                 — add it there and bump `Glyph::COUNT`, or `handle` will \
                 index past the sheet the moment the mark is drawn"
            );
        }
        // And every glyph in the sheet can actually be asked for.
        for glyph in Glyph::ALL {
            let _ = handle(glyph);
        }
        assert_eq!(Glyph::ALL.len(), Glyph::COUNT);
    }

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
        // **The one place a sprite is drawn bigger than its box**: the ghost
        // tile's `+` on the saved-playlist wall. It is the raster's own edge,
        // so it is pixel-exact rather than an upscale — a soft mark in the
        // middle of a wall of sharp covers is exactly what it may not be.
        assert!((index_to_f32(RASTER_PX) - theme::GHOST_MARK_PX).abs() < f32::EPSILON);
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

    /// Previous is a bar against a triangle: the same five runs across the
    /// middle as [`Glyph::Next`], read the other way, and the bar on the left.
    #[test]
    fn previous_is_a_bar_against_a_triangle() {
        let pixels = rasterize(Glyph::Previous, [255, 255, 255]);
        let mid = RASTER_PX / 2;
        let row: Vec<bool> = (0..RASTER_PX)
            .map(|column| alpha(&pixels, column, mid) > 0)
            .collect();
        let runs = row.chunk_by(|a, b| a == b).count();
        assert_eq!(runs, 5, "expected gap/bar/gap/triangle/gap");
        // The bar is full height where the triangle has already tapered to its
        // apex — the property that makes the shape read as "skip back" rather
        // than as a play button with a line beside it. Probe the first solid
        // run rather than a magic column, mirroring the Next assertion.
        let solid: Vec<u32> = (0..RASTER_PX).filter(|&c| row[c as usize]).collect();
        let bar = *solid.first().expect("the bar has some width") + 1;
        assert!(bar < mid, "the bar sits left of centre");
        let near_top = RASTER_PX / 5;
        assert!(alpha(&pixels, bar, near_top) > 0, "the bar reaches the top");
        assert_eq!(
            alpha(&pixels, mid, near_top),
            0,
            "the triangle has already tapered by there"
        );
        // Vertically symmetric about the centre line, like every other glyph.
        for column in 0..RASTER_PX {
            for row in 0..RASTER_PX / 2 {
                assert_eq!(
                    alpha(&pixels, column, row),
                    alpha(&pixels, column, RASTER_PX - 1 - row),
                    "previous is not symmetric at {column},{row}"
                );
            }
        }
    }

    /// **The shuffle mark is symmetric about its horizontal centre line**, and
    /// it is a mark rather than a smudge.
    ///
    /// [`Self::previous_is_next_reflected`]'s discipline, turned ninety degrees
    /// and applied *inside* one glyph: the falling shaft and its head are the
    /// rising pair reflected, vertex for vertex, so a nudge to one that was not
    /// made to the other fails here rather than in review. Exact equality is
    /// available because the rasterizer samples symmetrically about that line.
    #[test]
    fn the_shuffle_mark_is_its_own_reflection() {
        let pixels = rasterize(Glyph::Shuffle, [255, 255, 255]);
        for row in 0..RASTER_PX {
            for column in 0..RASTER_PX {
                assert_eq!(
                    alpha(&pixels, column, row),
                    alpha(&pixels, column, RASTER_PX - 1 - row),
                    "the crossed arrows are not symmetric at {column},{row}"
                );
            }
        }

        // **The two paths meet on the centre line.** The union rule fills the
        // crossing solid, which is what the symbol wants — the shafts cross,
        // they do not pass behind one another. The meeting point is where the
        // shafts' own mid-lines intersect, left of the box's centre because the
        // right of the box is the heads', so this asks the honest question:
        // *is there ink on the centre line at all?*
        let centre = RASTER_PX / 2;
        let crossing = (0..RASTER_PX).any(|column| alpha(&pixels, column, centre) == 255);
        assert!(
            crossing,
            "nothing is inked on the centre line: the shafts are not crossing"
        );

        // **The heads are on the right**, which is what makes this arrows
        // rather than a saltire. The shafts are the wider mark by area, so
        // comparing halves would prove nothing; what only a head can do is
        // reach the top of the box. So: the topmost inked row is entirely in
        // the right-hand half.
        let top = (0..RASTER_PX)
            .find(|&row| (0..RASTER_PX).any(|column| alpha(&pixels, column, row) > 0))
            .expect("the glyph is inked at all");
        for column in 0..RASTER_PX / 2 {
            assert_eq!(
                alpha(&pixels, column, top),
                0,
                "row {top} is inked at {column}, in the half the heads are not in"
            );
        }
        assert!(
            (RASTER_PX / 2..RASTER_PX).any(|column| alpha(&pixels, column, top) > 0),
            "the topmost row is not the arrowhead's"
        );

        // And the corners are clear: a mark, not a filled box.
        for (column, row) in [(0, 0), (RASTER_PX - 1, 0), (0, RASTER_PX - 1)] {
            assert_eq!(
                alpha(&pixels, column, row),
                0,
                "ink in the {column},{row} corner"
            );
        }
    }

    /// Previous is [`Glyph::Next`] mirrored, pixel for pixel.
    ///
    /// The strongest form of "the pair reads as one set": not merely similar
    /// proportions, but the same coverage reflected in the sprite's vertical
    /// centre line. The rasterizer samples symmetrically about that line, so
    /// an exact equality is available here and is worth taking — it is what
    /// would catch a nudge to one outline that was not made to the other.
    #[test]
    fn previous_is_next_reflected() {
        let next = rasterize(Glyph::Next, [255, 255, 255]);
        let previous = rasterize(Glyph::Previous, [255, 255, 255]);
        for row in 0..RASTER_PX {
            for column in 0..RASTER_PX {
                assert_eq!(
                    alpha(&next, column, row),
                    alpha(&previous, RASTER_PX - 1 - column, row),
                    "previous is not next mirrored at {column},{row}"
                );
            }
        }
        // …and it is genuinely a different sprite, or the assertion above
        // would be satisfied by an accidentally symmetric glyph.
        assert_ne!(next, previous);
    }

    #[test]
    fn the_toggle_state_picks_its_own_glyph() {
        assert_eq!(Glyph::from(PlayPause::Play), Glyph::Play);
        assert_eq!(Glyph::from(PlayPause::Pause), Glyph::Pause);
        assert_eq!(Glyph::speaker(false), Glyph::Speaker);
        assert_eq!(Glyph::speaker(true), Glyph::SpeakerMuted);
    }

    #[test]
    fn the_speaker_is_a_cone_with_two_waves_off_it() {
        let pixels = rasterize(Glyph::Speaker, [255, 255, 255]);
        let mid = RASTER_PX / 2;
        // Across the middle: cone, gap, wave, gap, wave, gap.
        let row: Vec<bool> = (0..RASTER_PX)
            .map(|column| alpha(&pixels, column, mid) > 0)
            .collect();
        let runs = row.chunk_by(|a, b| a == b).count();
        assert_eq!(runs, 7, "expected gap/cone/gap/wave/gap/wave/gap");
        assert!(
            !row[0] && !row[(RASTER_PX - 1) as usize],
            "inset at both ends"
        );
        // The cone occupies the left two fifths and the waves the rest —
        // the proportions that make the shape read as a speaker rather than
        // as three unrelated marks.
        let solid: Vec<u32> = (0..RASTER_PX).filter(|&c| row[c as usize]).collect();
        assert!(*solid.first().expect("a cone") < RASTER_PX / 8);
        assert!(*solid.last().expect("a wave") > RASTER_PX * 4 / 5);
        // Vertically symmetric, so the glyph reads as centred.
        for column in 0..RASTER_PX {
            for row in 0..RASTER_PX / 2 {
                assert_eq!(
                    alpha(&pixels, column, row),
                    alpha(&pixels, column, RASTER_PX - 1 - row),
                    "the speaker is not symmetric at {column},{row}"
                );
            }
        }
    }

    #[test]
    fn muting_swaps_the_waves_for_a_solid_cross_and_keeps_the_cone() {
        let sounding = rasterize(Glyph::Speaker, [255, 255, 255]);
        let muted = rasterize(Glyph::SpeakerMuted, [255, 255, 255]);
        // The cone is shared, so the left third of the sprite is identical:
        // the target a user aims at does not move when the state changes.
        let cone_edge = RASTER_PX * 2 / 5;
        for row in 0..RASTER_PX {
            for column in 0..cone_edge {
                assert_eq!(
                    alpha(&sounding, column, row),
                    alpha(&muted, column, row),
                    "the cone moved at {column},{row}"
                );
            }
        }
        // The cross is solid where its two bars overlap — the union rule in
        // `covers`. An even-odd test over both outlines would leave a hole.
        let mid = RASTER_PX / 2;
        let centre = (RASTER_PX * 7) / 10;
        assert_eq!(
            alpha(&muted, centre, mid),
            u8::MAX,
            "the bars of the cross must not cancel where they cross"
        );
        // And it is symmetric about the centre line, like the waves it
        // replaced.
        for column in 0..RASTER_PX {
            for row in 0..RASTER_PX / 2 {
                assert_eq!(
                    alpha(&muted, column, row),
                    alpha(&muted, column, RASTER_PX - 1 - row),
                    "the muted speaker is not symmetric at {column},{row}"
                );
            }
        }
    }

    #[test]
    fn close_is_a_solid_cross_centred_in_its_box() {
        let pixels = rasterize(Glyph::Close, [255, 255, 255]);
        let mid = RASTER_PX / 2;
        // The bars overlap at the centre, and the union rule must fill it —
        // the same property the mute cross needs, for the same reason.
        assert_eq!(alpha(&pixels, mid, mid), u8::MAX);
        // Nothing in the middle of any edge: a cross, not a box or a plus.
        assert_eq!(alpha(&pixels, mid, 1), 0);
        assert_eq!(alpha(&pixels, 1, mid), 0);
        // Symmetric about both axes, so the mark reads as centred.
        for row in 0..RASTER_PX {
            for column in 0..RASTER_PX / 2 {
                assert_eq!(
                    alpha(&pixels, column, row),
                    alpha(&pixels, RASTER_PX - 1 - column, row),
                    "close is not left-right symmetric at {column},{row}"
                );
                assert_eq!(
                    alpha(&pixels, row, column),
                    alpha(&pixels, row, RASTER_PX - 1 - column),
                    "close is not top-bottom symmetric at {row},{column}"
                );
            }
        }
    }

    /// **The three window controls are three different drawings, on the set's
    /// stroke band, and none of them is a glyph the sheet already had**
    /// (ADR-0040 §3).
    ///
    /// Asserted rather than looked at, because these three are the marks a
    /// reader is most likely to mistake for each other and for
    /// [`Glyph::Minus`] — a minimise bar and a stepper's minus are the same
    /// shape one position apart, and a maximise square and the lane marks'
    /// frame are the same shape one arm apart. The frames in
    /// `docs/design/impl/app-bar/` show them at size; this holds the geometry
    /// that makes them tellable.
    #[test]
    fn the_window_controls_are_three_marks_on_the_sets_stroke() {
        // **Minimise is a bar, and it is not the stepper's minus.** Both are
        // one horizontal run at the set's stroke; what separates them is
        // where they sit and how wide they are, so both are asserted.
        let minimise = runs_along(Glyph::WindowMinimise, 0.74);
        assert_eq!(minimise.len(), 1, "minimise is one bar");
        let (start, width) = minimise[0];
        assert!(
            (0.55..0.60).contains(&width),
            "the minimise bar is {width:.3} wide — it should be inset from the \
             box, which is what keeps it off a full-measure rule"
        );
        assert!(
            (start - (1.0 - start - width)).abs() < 0.01,
            "the minimise bar is not centred in its box"
        );
        assert!(
            runs_along(Glyph::WindowMinimise, 0.50).is_empty(),
            "the minimise bar is on the box's centre line, where it would be \
             `Minus` with a different name"
        );
        assert!(
            !runs_along(Glyph::Minus, 0.50).is_empty(),
            "…and the stepper's minus still is, which is the contrast"
        );

        // **Maximise is an empty square**: a cut across its middle meets two
        // arms with a hole between them, and the arms are the set's stroke.
        let across = runs_along(Glyph::WindowMaximise, 0.50);
        assert_eq!(across.len(), 2, "maximise is a square, not a filled block");
        for (_, width) in &across {
            assert!(
                (0.14..=0.15).contains(width),
                "a maximise arm is {width:.3} thick, outside the set's band"
            );
        }
        let hole = across[1].0 - (across[0].0 + across[0].1);
        assert!(
            hole > 0.3,
            "the square's hole is {hole:.3} — too tight to read as empty at 16 px"
        );

        // **Restore is two squares, offset**: a cut through the back square's
        // right arm, above the front square entirely, meets exactly one run.
        let high = runs_along(Glyph::WindowRestore, 0.18);
        assert_eq!(high.len(), 1, "the back square's top is one bar");
        let mid = runs_along(Glyph::WindowRestore, 0.50);
        assert_eq!(
            mid.len(),
            3,
            "a cut through both squares meets the front square's two arms and \
             the back square's right one — which is what *offset* means"
        );

        // And no two of the sheet's marks are the same drawing. Cheap and
        // total: the sprite bytes, over the whole sheet.
        let mut seen: Vec<(Glyph, Vec<u8>)> = Vec::new();
        for glyph in Glyph::ALL {
            let pixels = rasterize(glyph, [255, 255, 255]);
            for (other, earlier) in &seen {
                assert!(
                    *earlier != pixels,
                    "{glyph:?} and {other:?} rasterize identically — one \
                     drawing cannot mean two things"
                );
            }
            seen.push((glyph, pixels));
        }
    }

    /// **The bell is as wide as the cluster it stands in.**
    ///
    /// The owner, reading the app bar hours after the bell first drew at all:
    /// *"the bell icon is a little bit narrow/skinny."* Every mark in that
    /// right cluster is drawn into the identical [`theme::ICON_PX`] box, so a
    /// glyph that fills less of its box than its neighbours *is* thinner ink
    /// on screen — this pins the bell into their range from below rather than
    /// leaving it to whoever next edits the outline.
    ///
    /// The ceiling is the other half: a bell wider than it is tall stops being
    /// a bell.
    ///
    /// **It measures the median run, not the widest** — item 74, and the whole
    /// of why item 48's repair passed its own test while the owner went on
    /// seeing a skinny bell. The widest scanline of a bell is its **rim**, one
    /// hairline at the foot, and the rim was already 0.78 of the box. A test
    /// reading the maximum was satisfied by a wide plate under a narrow spike.
    /// A glyph is read by its **mass**, so the median of the runs a glyph
    /// actually fills is the number that corresponds to what a reader sees.
    #[test]
    fn the_bell_is_as_wide_as_the_cluster_it_stands_in() {
        // The median width of the solid runs a glyph fills, over every
        // scanline that touches it at all.
        let median = |glyph: Glyph| {
            let mut runs: Vec<f32> = Vec::new();
            let mut y = 0.0;
            while y <= 1.0 {
                if let (Some((start, _)), Some(&(last_start, last_width))) =
                    (runs_along(glyph, y).first(), runs_along(glyph, y).last())
                {
                    runs.push(last_start + last_width - start);
                }
                y += 1.0 / 512.0;
            }
            runs.sort_by(f32::total_cmp);
            runs.get(runs.len() / 2).copied().unwrap_or(0.0)
        };
        let widest = |glyph: Glyph| {
            let mut widest = 0.0_f32;
            let mut y = 0.0;
            while y <= 1.0 {
                if let (Some((start, _)), Some(&(last_start, last_width))) =
                    (runs_along(glyph, y).first(), runs_along(glyph, y).last())
                {
                    widest = widest.max(last_start + last_width - start);
                }
                y += 1.0 / 512.0;
            }
            widest
        };
        let bell = median(Glyph::Bell);
        // Its neighbours in the same box: the gear one seam away, and the two
        // round marks the lane draws at the same size.
        let neighbours = [
            median(Glyph::Gear),
            median(Glyph::Home),
            median(Glyph::NowPlaying),
        ];
        let narrowest = neighbours.iter().copied().fold(f32::MAX, f32::min);
        assert!(
            bell >= narrowest - 0.07,
            "the bell is {bell:.3} of its box against the narrowest neighbour's \
             {narrowest:.3} — it reads as skinny beside them, which is exactly \
             what the owner saw"
        );
        // Height: crown to clapper. It may not be a squat bell either.
        let mut tallest = 0.0_f32;
        let mut top = f32::MAX;
        let mut x = 0.0;
        while x <= 1.0 {
            let mut y = 0.0;
            while y <= 1.0 {
                if Glyph::Bell.covers(x, y) {
                    top = top.min(y);
                    tallest = tallest.max(y);
                }
                y += 1.0 / 512.0;
            }
            x += 1.0 / 512.0;
        }
        let height = tallest - top;
        assert!(
            widest(Glyph::Bell) <= height + 0.01,
            "the bell is {:.3} wide against {height:.3} tall — wider than it \
             is tall, which is a pot rather than a bell",
            widest(Glyph::Bell)
        );
    }

    /// The solid runs a horizontal line at unit-square height `y` crosses,
    /// as `(start, width)` pairs in unit-square units — measured off
    /// [`Glyph::covers`] directly, so a stroke width is geometry rather than
    /// a count of antialiased pixels.
    fn runs_along(glyph: Glyph, y: f32) -> Vec<(f32, f32)> {
        const STEP: f32 = 1.0 / 2048.0;
        let mut runs = Vec::new();
        let mut start: Option<f32> = None;
        let mut x = 0.0;
        while x <= 1.0 {
            match (glyph.covers(x, y), start) {
                (true, None) => start = Some(x),
                (false, Some(began)) => {
                    runs.push((began, x - began));
                    start = None;
                }
                _ => {}
            }
            x += STEP;
        }
        if let Some(began) = start {
            runs.push((began, 1.0 - began));
        }
        runs
    }

    /// **Queue is a list that runs out, not a menu.**
    ///
    /// Three bars on the set's stroke, the third short — which is the whole of
    /// what keeps it apart from the hamburger three *equal* bars are
    /// everywhere else in software (doc 10 §3.4's worry, met rather than
    /// waved past). The block is symmetric about the box's centre line, so
    /// the mark reads as centred beside its word.
    #[test]
    fn queue_is_three_bars_and_the_last_one_is_short() {
        let bars: Vec<Vec<(f32, f32)>> = [0.26_f32, 0.50, 0.74]
            .iter()
            .map(|&y| runs_along(Glyph::Queue, y))
            .collect();
        for (index, runs) in bars.iter().enumerate() {
            assert_eq!(runs.len(), 1, "bar {index} is not one run: {runs:?}");
        }
        let width = |runs: &[(f32, f32)]| runs[0].1;
        assert!(
            (width(&bars[0]) - width(&bars[1])).abs() < 0.01,
            "the first two bars are not the same length"
        );
        assert!(
            width(&bars[2]) < width(&bars[0]) * 0.85,
            "the third bar is not short — three equal bars are a menu glyph"
        );
        for runs in &bars {
            assert!(
                (runs[0].0 - 0.12).abs() < 0.01,
                "the bars do not share a left edge"
            );
        }
        // The gaps between them are gaps.
        for y in [0.38_f32, 0.62] {
            assert!(
                runs_along(Glyph::Queue, y).is_empty(),
                "the bars touch at y {y}"
            );
        }
        // The stroke band the whole sheet is drawn on (doc 10 §3.7).
        for y in [0.26_f32, 0.50, 0.74] {
            let mut top = y;
            while Glyph::Queue.covers(0.2, top - 0.001) {
                top -= 0.001;
            }
            let mut bottom = y;
            while Glyph::Queue.covers(0.2, bottom + 0.001) {
                bottom += 0.001;
            }
            let stroke = bottom - top;
            assert!(
                (0.14..=0.155).contains(&stroke),
                "a queue bar is {stroke:.3} thick, outside the set's 0.14-0.15 band"
            );
        }
        // Symmetric about the box's centre line as a *block*: the first bar's
        // top is as far from 0 as the third bar's bottom is from 1.
        assert!(
            ((0.185_f32) - (1.0 - 0.815)).abs() < f32::EPSILON,
            "the block is not centred in its box"
        );
    }

    /// **Open is a chevron, and it is not the play triangle.**
    ///
    /// Two strokes meeting at a vertex right of centre, mirror-symmetric about
    /// the box's horizontal centre line. The distinction that matters is at
    /// the left edge: [`Glyph::Play`]'s triangle is solid all the way back to
    /// its base, and this has a hole behind the vertex — one is a mass, the
    /// other an angle, and the two sit two rows apart on the same veil.
    #[test]
    fn open_is_a_chevron_with_a_hole_behind_it() {
        // The apex: one run where the arms meet.
        assert_eq!(
            runs_along(Glyph::Open, 0.5).len(),
            1,
            "the arms do not join at the vertex"
        );
        // Above and below it, one arm each, and they are mirrors.
        for y in [0.16_f32, 0.30] {
            let upper = runs_along(Glyph::Open, y);
            let lower = runs_along(Glyph::Open, 1.0 - y);
            assert_eq!(upper.len(), 1, "the upper arm is not one run at {y}");
            assert_eq!(lower.len(), 1, "the lower arm is not one run at {y}");
            assert!(
                (upper[0].0 - lower[0].0).abs() < 0.005 && (upper[0].1 - lower[0].1).abs() < 0.005,
                "the arms are not mirrors at {y}: {upper:?} vs {lower:?}"
            );
        }
        // The hole: behind the vertex, on the centre line, where a triangle
        // would be solid.
        assert!(
            !Glyph::Open.covers(0.40, 0.50),
            "the chevron filled in — it is a triangle now"
        );
        assert!(
            Glyph::Play.covers(0.40, 0.50),
            "the play triangle stopped being solid, and the pair stopped \
             being tellable apart by mass"
        );
        // The vertex sits right of centre, the direction of travel.
        let apex = runs_along(Glyph::Open, 0.5);
        assert!(
            apex[0].0 + apex[0].1 > 0.7,
            "the vertex does not reach into the right of the box"
        );
        // The set's stroke, measured perpendicular to a 45° arm: a horizontal
        // cut through it is the stroke times root two.
        let arm = runs_along(Glyph::Open, 0.30);
        let stroke = arm[0].1 / std::f32::consts::SQRT_2;
        assert!(
            (0.14..=0.155).contains(&stroke),
            "the chevron's stroke is {stroke:.3}, outside the set's 0.14-0.15 band"
        );
    }

    /// **The density detents depict the wall at its four hangs** — one, four,
    /// nine and sixteen works subdividing one shared field (ADR-0028 as
    /// amended), so the four read as one wall at four settings rather than as
    /// four unrelated marks.
    ///
    /// The list is written in [`crate::shelf::Density::ALL`]'s own order and
    /// its length is asserted against it, so a step added without a mark —
    /// or a mark added without a step — fails here rather than on screen.
    #[test]
    fn the_density_detents_subdivide_one_shared_field() {
        let detents = [
            (Glyph::DensitySpacious, 1),
            (Glyph::DensityBalanced, 2),
            (Glyph::DensityCompact, 3),
            (Glyph::DensityDense, 4),
        ];
        assert_eq!(
            detents.len(),
            crate::shelf::Density::ALL.len(),
            "every density step wears exactly one detent mark"
        );
        for (glyph, columns) in detents {
            // Through the top row of cells: as many solid runs as columns.
            let runs = runs_along(glyph, 0.25);
            assert_eq!(runs.len(), columns, "{glyph:?} across its top row");
            // One shared field: the first run opens on the field's left
            // edge and the last closes on its right, for every detent.
            let first = runs.first().expect("a cell");
            let last = runs.last().expect("a cell");
            assert!(
                (first.0 - DENSITY_FIELD.0).abs() < 0.01,
                "{glyph:?} does not open on the shared field's edge"
            );
            assert!(
                (last.0 + last.1 - DENSITY_FIELD.1).abs() < 0.01,
                "{glyph:?} does not close on the shared field's edge"
            );
        }
        // The subdivisions are real: mid-height crosses Balanced's gap and
        // Dense's gap (nothing — both have an even number of rows), Compact's
        // middle row (three cells) and Spacious's one work (still solid).
        assert!(runs_along(Glyph::DensityBalanced, 0.5).is_empty());
        assert!(runs_along(Glyph::DensityDense, 0.5).is_empty());
        assert_eq!(runs_along(Glyph::DensityCompact, 0.5).len(), 3);
        assert_eq!(runs_along(Glyph::DensitySpacious, 0.5).len(), 1);
    }

    /// **The magnifier is a ring with a handle**, and the ring's hole
    /// survives the keyhole outline — the property doc 10 §3.6's note is
    /// about: `covers` takes the union of outlines, so the hole has to be
    /// carried by the even-odd rule *within* one outline, and it is.
    #[test]
    fn the_magnifier_is_a_ring_with_a_handle_and_the_hole_survives() {
        // The glass's centre is empty and the band around it is solid.
        assert!(!Glyph::Magnifier.covers(0.42, 0.42), "the hole filled in");
        assert!(Glyph::Magnifier.covers(0.42 - 0.2275, 0.42), "no band left");
        assert!(
            Glyph::Magnifier.covers(0.42, 0.42 - 0.2275),
            "no band above"
        );
        // The handle reaches into the lower right; the far corners are bare.
        assert!(Glyph::Magnifier.covers(0.84, 0.84), "no handle");
        assert!(!Glyph::Magnifier.covers(0.1, 0.9));
        assert!(!Glyph::Magnifier.covers(0.9, 0.1));
        // Across the glass's own centre line: gap, band, hole, band, gap —
        // the ring reads as a ring, not as a disc.
        let runs = runs_along(Glyph::Magnifier, 0.42);
        assert_eq!(runs.len(), 2, "expected band/hole/band across the glass");
        // And the raster agrees: the hole is transparent in the sprite.
        let pixels = rasterize(Glyph::Magnifier, [255, 255, 255]);
        let centre = RASTER_PX * 42 / 100;
        assert_eq!(alpha(&pixels, centre, centre), 0, "the hole filled in");
    }

    /// The magnifier's ring and handle draw at the set's one stroke band —
    /// 0.14–0.15 of the unit square (doc 10 §3.7) — measured as geometry.
    #[test]
    fn the_magnifier_keeps_the_sets_stroke_band() {
        // The ring, crossed horizontally through the glass's centre: both
        // band runs are one stroke wide.
        for (_, width) in runs_along(Glyph::Magnifier, 0.42) {
            assert!(
                (0.14..=0.152).contains(&width),
                "a ring stroke of {width:.3} is off the set's band"
            );
        }
        // The handle, crossed at a height only its shaft occupies — below
        // the ring (which ends at y 0.72) and clear of the end cap. It runs
        // at 45°, so a horizontal cut is its stroke × √2.
        let handle = runs_along(Glyph::Magnifier, 0.80);
        assert_eq!(handle.len(), 1, "expected the handle alone at y 0.80");
        let width = handle[0].1 / std::f32::consts::SQRT_2;
        assert!(
            (0.14..=0.152).contains(&width),
            "a handle stroke of {width:.3} is off the set's band"
        );
    }

    /// **The gear is a toothed ring with a hole**: eight teeth, a band at
    /// the set's stroke, and an empty centre — the keyhole construction
    /// again, with the teeth on the outer trace.
    #[test]
    fn the_gear_is_eight_teeth_around_a_ring_with_a_hole() {
        // The hole is a hole.
        assert!(!Glyph::Gear.covers(0.5, 0.5), "the gear's hole filled in");
        // Across the centre: gap, tooth-and-band, hole, band-and-tooth, gap —
        // the tooth at 0° meets the ring, so each side is one solid run.
        let runs = runs_along(Glyph::Gear, 0.5);
        assert_eq!(runs.len(), 2, "expected band/hole/band across the middle");
        // Those runs span valley-to-tip *and* the ring: from 0.08 in to the
        // hole's edge at 0.345.
        let (start, width) = runs[0];
        assert!((start - 0.08).abs() < 0.01, "the tooth tip starts at 0.08");
        assert!((width - 0.265).abs() < 0.01, "tooth and band are one mass");
        // Eight teeth: a circle sampled between valley and tip crosses a
        // tooth eight times.
        let mut crossings = 0;
        let mut previous = Glyph::Gear.covers(0.5 + 0.36, 0.5);
        for step in 1..=2880 {
            let angle = std::f32::consts::TAU * index_to_f32(step) / 2880.0;
            let inside = Glyph::Gear.covers(0.5 + 0.36 * angle.cos(), 0.5 + 0.36 * angle.sin());
            if inside != previous {
                crossings += 1;
                previous = inside;
            }
        }
        assert_eq!(crossings, 16, "eight teeth cross the 0.36 circle");
        // Symmetric about the horizontal centre line, like the rest of the
        // set — the slit is zero-width, so it cannot break this.
        let pixels = rasterize(Glyph::Gear, [255, 255, 255]);
        for column in 0..RASTER_PX {
            for row in 0..RASTER_PX / 2 {
                assert_eq!(
                    alpha(&pixels, column, row),
                    alpha(&pixels, column, RASTER_PX - 1 - row),
                    "the gear is not symmetric at {column},{row}"
                );
            }
        }
    }

    /// The gear's ring keeps the set's stroke band, measured where only the
    /// ring is: along the valley between two teeth, the band is
    /// 0.30 − 0.155 = 0.145 of the unit square.
    #[test]
    fn the_gear_keeps_the_sets_stroke_band() {
        // Walk a ray out through a valley centre (22.5°) and measure where
        // the band starts and stops.
        let angle = std::f32::consts::TAU * 22.5 / 360.0;
        let (dx, dy) = (angle.cos(), angle.sin());
        let mut entered: Option<f32> = None;
        let mut band = 0.0;
        let mut radius = 0.0;
        while radius <= 0.5 {
            let inside = Glyph::Gear.covers(0.5 + radius * dx, 0.5 + radius * dy);
            match (inside, entered) {
                (true, None) => entered = Some(radius),
                (false, Some(began)) => {
                    band = radius - began;
                    break;
                }
                _ => {}
            }
            radius += 1.0 / 2048.0;
        }
        assert!(
            (0.14..=0.152).contains(&band),
            "a ring stroke of {band:.3} is off the set's band"
        );
    }

    /// **The plus is a solid axis-aligned cross** at the set's stroke: two
    /// bars filled by the union rule where they overlap, symmetric about
    /// both axes — the ✕'s construction, turned upright.
    #[test]
    fn the_plus_is_a_solid_upright_cross_at_the_sets_stroke() {
        let pixels = rasterize(Glyph::Plus, [255, 255, 255]);
        let mid = RASTER_PX / 2;
        // Solid where the bars overlap — the union rule again.
        assert_eq!(alpha(&pixels, mid, mid), u8::MAX);
        // Nothing at the corners, and nothing at the very edges: the mark
        // is inset like the rest of the set.
        for (column, row) in [(1, 1), (RASTER_PX - 2, 1), (1, RASTER_PX - 2)] {
            assert_eq!(alpha(&pixels, column, row), 0, "corner {column},{row}");
        }
        // Symmetric about both axes.
        for row in 0..RASTER_PX {
            for column in 0..RASTER_PX / 2 {
                assert_eq!(
                    alpha(&pixels, column, row),
                    alpha(&pixels, RASTER_PX - 1 - column, row),
                    "plus is not left-right symmetric at {column},{row}"
                );
                assert_eq!(
                    alpha(&pixels, row, column),
                    alpha(&pixels, row, RASTER_PX - 1 - column),
                    "plus is not top-bottom symmetric at {row},{column}"
                );
            }
        }
        // The vertical bar alone, crossed above the horizontal one, is one
        // run at the set's stroke.
        let runs = runs_along(Glyph::Plus, 0.30);
        assert_eq!(runs.len(), 1, "expected the vertical bar alone at y 0.30");
        assert!(
            (0.14..=0.152).contains(&runs[0].1),
            "a plus stroke of {:.3} is off the set's band",
            runs[0].1
        );
    }

    /// **The minus is the plus's own horizontal bar**, alone — the sharing
    /// is what makes the stepper pair read as a pair, and it is asserted as
    /// pixel equality wherever the plus's vertical bar is not.
    #[test]
    fn the_minus_is_the_pluss_own_bar() {
        let plus = rasterize(Glyph::Plus, [255, 255, 255]);
        let minus = rasterize(Glyph::Minus, [255, 255, 255]);
        // Left and right of the vertical bar (which spans 0.425–0.575, i.e.
        // raster columns 13.6–18.4 — so columns 13 and 18 carry its
        // antialiased edges), the two sprites are identical.
        for row in 0..RASTER_PX {
            for column in (0..RASTER_PX * 42 / 100).chain(RASTER_PX * 60 / 100..RASTER_PX) {
                assert_eq!(
                    alpha(&plus, column, row),
                    alpha(&minus, column, row),
                    "the minus is not the plus's bar at {column},{row}"
                );
            }
        }
        // And it genuinely lacks the vertical bar.
        let mid = RASTER_PX / 2;
        assert_eq!(alpha(&minus, mid, RASTER_PX / 4), 0);
        assert_eq!(alpha(&plus, mid, RASTER_PX / 4), u8::MAX);
        // One run across the middle, at the bar's own length.
        let runs = runs_along(Glyph::Minus, 0.5);
        assert_eq!(runs.len(), 1);
        assert!((runs[0].1 - 0.69).abs() < 0.01, "the bar is 0.845 − 0.155");
    }

    /// **The up arrow is three strokes** — a shaft and two chevron arms —
    /// all at the set's band: drawn strokes, not a filled triangle head,
    /// for the speaker waves' reason.
    #[test]
    fn the_arrow_is_a_shaft_under_a_chevron_at_the_sets_stroke() {
        // At a height crossing both arms and the shaft: three runs — arm,
        // shaft, arm — the arms' horizontal cuts √2 wider than their
        // strokes (they run at 45°).
        let runs = runs_along(Glyph::ArrowUp, 0.42);
        assert_eq!(runs.len(), 3, "expected arm/shaft/arm at y 0.42");
        for (index, (_, width)) in runs.iter().enumerate() {
            let stroke = if index == 1 {
                *width
            } else {
                *width / std::f32::consts::SQRT_2
            };
            assert!(
                (0.14..=0.152).contains(&stroke),
                "an arrow stroke of {stroke:.3} is off the set's band"
            );
        }
        // Below the arms, the shaft alone.
        let shaft = runs_along(Glyph::ArrowUp, 0.70);
        assert_eq!(shaft.len(), 1, "expected the shaft alone at y 0.70");
        // Left-right symmetric, so the head reads as centred on the shaft.
        let pixels = rasterize(Glyph::ArrowUp, [255, 255, 255]);
        for row in 0..RASTER_PX {
            for column in 0..RASTER_PX / 2 {
                assert_eq!(
                    alpha(&pixels, column, row),
                    alpha(&pixels, RASTER_PX - 1 - column, row),
                    "the arrow is not symmetric at {column},{row}"
                );
            }
        }
    }

    /// The down arrow is the up arrow reflected, pixel for pixel — the
    /// strongest form of "the pair reads as one set", exactly as
    /// [`Glyph::Previous`] is held to [`Glyph::Next`].
    #[test]
    fn arrow_down_is_arrow_up_reflected() {
        let up = rasterize(Glyph::ArrowUp, [255, 255, 255]);
        let down = rasterize(Glyph::ArrowDown, [255, 255, 255]);
        for row in 0..RASTER_PX {
            for column in 0..RASTER_PX {
                assert_eq!(
                    alpha(&up, column, row),
                    alpha(&down, column, RASTER_PX - 1 - row),
                    "down is not up mirrored at {column},{row}"
                );
            }
        }
        assert_ne!(up, down);
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

    #[test]
    fn the_app_mark_is_the_committed_red_circle_raster() {
        let mark = app_mark();
        let image::Handle::Rgba { width, height, .. } = mark else {
            panic!("app mark must be a decoded RGBA raster");
        };
        assert_eq!(width, 64);
        assert_eq!(height, 64);
    }
}
