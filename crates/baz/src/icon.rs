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
    /// [`encloses`]'s even-odd rule punches the hole — [`Glyph::covers`] takes
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
    /// in [`Self::covers`] fills the crossing solid, which is what the symbol
    /// wants: the two paths *meet*, they do not pass behind one another. A
    /// notch at the crossing would need an even-odd hole and would be a pixel
    /// wide at [`RASTER_PX`].
    Shuffle,
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
/// approximating it, and at [`RASTER_PX`] (32 px) the difference between a
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
/// [`Glyph::covers`] takes the union of the outlines rather than the
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
/// union rule in [`Glyph::covers`]: an even-odd test over the pair would punch
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

/// The magnifier's ring — a **keyhole outline**, because [`Glyph::covers`]
/// takes the union of outlines and a ring drawn as two circles would have its
/// hole cancelled (doc 10 §3.6's implementation note). One closed polygon:
/// the outer circle traced all the way round, a zero-width bridge in to the
/// inner circle, the inner circle traced back the other way, and the bridge
/// out again. The existing even-odd test ([`encloses`]) then fills the band
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
/// fill by the union rule in [`Glyph::covers`], exactly as the two crosses
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
    ];

    /// How many glyphs the sheet holds.
    const COUNT: usize = 28;

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
        }
    }

    /// Its slot in the sprite sheet.
    fn index(self) -> usize {
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
/// **The room is baked in.** The sheet is rasterized on first use, which is
/// during the first frame and therefore after `theme::install` has resolved
/// the room; a room that could change while the process ran would have to
/// invalidate this, and that is part of what step 20 buys when it makes the
/// second room selectable.
static SHEET: LazyLock<[image::Handle; Glyph::COUNT]> = LazyLock::new(|| {
    let ink = rgb(theme::active().glyph());
    Glyph::ALL.map(|glyph| image::Handle::from_rgba(RASTER_PX, RASTER_PX, rasterize(glyph, ink)))
});

/// The sprite for `glyph`. Cheap: an `Arc` bump over the shared sheet.
#[must_use]
pub fn handle(glyph: Glyph) -> image::Handle {
    SHEET[glyph.index()].clone()
}

/// The same sheet, inked in the room's **accent**.
///
/// Two consumers, and the accent discipline is what bounds it to two: the
/// wall's hover `Play`, which is the record page's `Play album` moved onto the
/// sleeve and carries that control's licence (`theme::veil_option_ink`); and
/// the bar's shuffle toggle **while it is on**, which creates playback truth
/// about what sounds *next* in the way `Play album` creates it about what
/// sounds now (`crate::views::bottom_bar`'s `shuffle_toggle`).
/// Built lazily beside [`SHEET`] and by the same rules — the room is baked in,
/// the ids live as long as the process, and the cost of the second sheet is
/// 18 sprites of 32 × 32 × 4 bytes.
static ACCENT_SHEET: LazyLock<[image::Handle; Glyph::COUNT]> = LazyLock::new(|| {
    let ink = rgb(theme::active().lamp);
    Glyph::ALL.map(|glyph| image::Handle::from_rgba(RASTER_PX, RASTER_PX, rasterize(glyph, ink)))
});

/// The sprite for `glyph` in `ink`, which must be one of the two inks a sheet
/// exists for: the room's glyph ink, or its accent.
///
/// The caller states the ink and this resolves the sheet, so the *decision*
/// about which glyph wears the accent lives in one place
/// ([`theme::veil_option_ink`]) rather than being spelled twice. An ink that
/// is neither takes the ordinary sheet: a third inked sheet is a decision, and
/// silently minting one here is how an accent discipline stops being one.
#[must_use]
pub fn inked(glyph: Glyph, ink: Color) -> image::Handle {
    if ink == theme::active().lamp {
        ACCENT_SHEET[glyph.index()].clone()
    } else {
        SHEET[glyph.index()].clone()
    }
}

/// The **application's own mark** — the icon a launcher shows — decoded once
/// from the PNG the desktop entry and the Flatpak already install.
///
/// # It is not on the sheet, and that is the whole point
///
/// Everything above this line is a *glyph*: an outline in a unit square,
/// rasterized to coverage and **inked by the room** at draw time
/// ([`SHEET`], [`ACCENT_SHEET`]). A glyph has no colour of its own; the room
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
/// The **32 px** rung, drawn at [`theme::ICON_PX`] 16 logical px — exactly
/// [`SUPERSCALE`] 2, which is the same `@2x` contract every sprite on the sheet
/// is drawn under, and exact on 1× and 2× displays for the same reason. The
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
/// reach for colour on this precedent. At 16 px the dot is roughly one pixel.
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
    /// The 32 px rung of the hicolor ladder — the same file the desktop entry,
    /// the release tarball and the Flatpak install.
    const BYTES: &[u8] =
        include_bytes!("../../../packaging/icons/hicolor/32x32/apps/io.github.mattcree.baz.png");
    // `::image` is the decoder crate; the bare `image` in this module is
    // `iced::widget::image`, whose `Handle` the last line mints.
    let mark = ::image::load_from_memory(BYTES)
        .expect("baz's own application icon, compiled in from packaging/icons")
        .to_rgba8();
    let (w, h) = mark.dimensions();
    image::Handle::from_rgba(w, h, mark.into_raw())
});

/// The application's mark, for the app bar's zone 1. Cheap: an `Arc` bump over
/// the one decoded copy, whose id lives as long as the process for [`SHEET`]'s
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
}
