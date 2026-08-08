//! The baz design system: palette, type scale, spacing, radii, and the
//! widget styles built from them. Every color, size, and padding the UI
//! renders comes from this module — `app.rs` holds layout, not values.
//!
//! # Palette rationale
//!
//! **A record archive after closing time. The works are lit; the room is not.**
//!
//! baz is a hang, not a dashboard. The wall is near-black and *neutral-cool* —
//! the matte paint of a black-cube gallery, never the warm charcoal of the
//! listening room this replaced and never the blue-grey of a stock dark theme.
//! The type is warm ivory, the colour of archival mount board. **The room is
//! cold and the paper is warm**, which is what a gallery looks like at night,
//! and it is the one decision that keeps a near-black grid of covers from
//! reading as every other media app. The chrome recedes so that 10 000 sleeves
//! — the actual interface — supply every other colour in the room.
//!
//! There is one light in it, and it is pointed at one thing: the record that is
//! playing. Everything else — every control, count, setting and state — is made
//! of surface, edge and ink. The long argument is
//! `docs/design/02-visual-language.md`; the condensed version that governs is
//! `.interface-design/system.md`.
//!
//! # The accent discipline
//!
//! There is exactly one accent, **lamp amber** — the power lamp / VU-meter
//! glow of an amplifier — and it means **playback truth**: a fact about the
//! audio the engine is producing *right now*. Which album is sounding, which
//! track within it, and where the playhead is in that track. Nothing else
//! qualifies: not what is queued, not what is selected, not what has focus,
//! not what the scanner is doing, not how a gain stage is configured.
//!
//! [`Palette::lamp`] and its relatives may appear in exactly five places
//! (`docs/design/02-visual-language.md` §2.1.1), and
//! `the_lamp_is_spent_only_on_playback_truth` below is what enforces it rather
//! than leaving it to be remembered:
//!
//! 1. the playing album's halo — [`sleeve`] with `playing`;
//! 2. the playing dot — [`lamp_dot`], beside a tile's title or in a row's
//!    number column;
//! 3. the seek groove's elapsed fill and knob — [`seek`];
//! 4. a seek in flight — the elapsed timestamp warms to [`Palette::lamp`] while a
//!    position has been asked for and not yet confirmed, because a position
//!    being asked for is a claim about the playhead;
//! 5. the primary Play action — [`primary`], the one argued exception: it is
//!    the only control in the product that *creates* playback truth, it
//!    appears at most once per screen, and it is the only lamp-*filled*
//!    rectangle anywhere in baz.
//!
//! Two uses were **cut** in the redesign's first pass, both of them a lamp
//! that was on when nothing was playing: input focus (now [`Palette::paper_ring`], and
//! the search field takes focus at launch, so the first frame baz ever drew
//! was an amber ring with no music), and the scanning note (now [`Palette::paper_dim`]
//! — a scan is the library working, not the music). Blue, every streaming
//! app's accent, remains deliberately absent.
//!
//! # Depth strategy: surface steps, and nothing else
//!
//! Four planes — [`Palette::recess`] below the wall, [`Palette::wall`], [`Palette::plinth`] one step up,
//! [`Palette::plinth_lit`] one above that — whisper-quiet in bytes (8 apart) and plainly
//! felt in linear light (nearly 2× per step, which is what the eye actually
//! uses at these levels). Squint and you perceive four planes and no edges.
//!
//! **Not shadows**, and that is measured rather than preferred: black at 55 %
//! over `#0C0D0E` composites to `#050606`, a contrast ratio of **1.04 : 1**. On
//! near-black a drop shadow is not a design tool, it is a rounding error, so
//! the sleeve's contact shadow is deleted rather than tuned (that deletion is
//! B1 of the adoption order, not this pass). The one shadow primitive left in
//! the product is the playing halo, and it is not elevation — it is light.
//!
//! Hairlines survive in three structural roles — under the top bar, above the
//! now-playing bar, and dividing the inspector from the shelf — plus a tile's
//! own hover rule and control borders. Corners: artwork is always square, like
//! the physical object; controls are barely rounded, because an archive is
//! rectilinear.
//!
//! # Rooms
//!
//! There is no longer one palette. ADR-0017 §1.5 adopts the critique's
//! **room** model — a whole coordinated set of surfaces, inks and one accent,
//! switched together — and every value below is a field on [`Palette`] rather
//! than a `pub const Color`. Two rooms are defined: [`CLOSING_TIME`], the
//! near-black gallery baz has always been, and [`READING_ROOM`], its light
//! mirror. Stone and Plaster are deferred (§1.5).
//!
//! The indirection lands **before** any per-surface styling is rewritten,
//! which is the whole reason it is step 2 of the build plan: ~30 style
//! functions take a `&Palette`, so the tile, the inspector and the bar are
//! written against a room once instead of against constants and then again
//! against a room.
//!
//! [`READING_ROOM`] is **defined but not yet selectable** ([`follow`]), which
//! is exactly what the plan's step-2 row says and what §1.5 gates: the light
//! room ships only with an answer to "what happens to a pale sleeve on a paper
//! ground that is not a border on artwork", and that answer is step 20's.
//! The follow-the-OS resolution is written and tested here so that step is a
//! one-constant change rather than a design of its own.
//!
//! # Contrast
//!
//! iced 0.13 publishes no accessibility tree, so contrast and hit-target size
//! are the only accessibility guarantees baz can make — which is a reason to
//! honour them exactly rather than a reason to shrug. **Two laws govern, over
//! disjoint domains** (ADR-0017 §1.6): surface against surface is measured in
//! oklch L and must step by ≥ 0.03, and ink against surface is measured in
//! WCAG 2.1 and must clear 4.5 : 1 to be read or 3 : 1 to be found. Opacity is
//! composited *before* either is taken, so a token expressed as an alpha
//! cannot smuggle an unreadable value past a test that only sees opaque
//! colours. `every_ink_and_every_surface_clears_its_floor` sweeps both, over
//! every room.

use std::sync::{LazyLock, OnceLock};

use iced::font::Weight;
use iced::widget::rule::FillMode;
use iced::widget::slider::{Handle, HandleShape, Rail};
use iced::widget::{button, checkbox, container, rule, scrollable, slider, text_input};
use iced::{Background, Border, Color, Font, Padding, Shadow, Theme, Vector, mouse};

// ---------------------------------------------------------------------------
// Palette
// ---------------------------------------------------------------------------

/// Which room the app is in.
///
/// A *room* is a whole coordinated palette — four surfaces, four inks, one
/// accent — switched together, never mixed. Two ship as tokens; Stone and
/// Plaster are deferred (ADR-0017 §1.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Room {
    /// The near-black gallery after hours: cool room, warm paper.
    ClosingTime,
    /// Its mirror: warm paper ground, cool ink, oxblood lamp.
    ReadingRoom,
}

impl Room {
    /// Every room, in the order the tests sweep them.
    pub const ALL: [Self; 2] = [Self::ClosingTime, Self::ReadingRoom];

    /// The room's resolved palette.
    #[must_use]
    pub const fn palette(self) -> &'static Palette {
        match self {
            Self::ClosingTime => &CLOSING_TIME,
            Self::ReadingRoom => &READING_ROOM,
        }
    }
}

/// A resolved room: every colour the interface can paint, in one value.
///
/// **This replaces ~24 `pub const Color`s**, and the replacement is the point
/// (ADR-0017 §1.5, build-plan step 2). A style function takes a `&Palette`, so
/// a surface is styled against *whatever room is standing* rather than against
/// the near-black one — and the per-surface work of the redesign's later steps
/// is written once instead of once per room.
///
/// The four alpha-expressed marks — the two hairlines, the focus ring and the
/// selection wash — are **methods rather than fields**, because they are the
/// room's own ink at a fixed opacity and deriving them is what stops a room
/// being defined with a hairline that belongs to a different one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    /// Which room this is.
    pub room: Room,
    /// The room's name, as a user would read it.
    pub name: &'static str,
    /// **The shadow gap** where the wall meets the floor: the now-playing bar,
    /// input wells, groove troughs and the backing behind a sleeve —
    /// everything that sits *below* the wall.
    ///
    /// It is the darkest plane in a dark room and the **lightest** in a light
    /// one: surfaces rise toward the lamp, which means lighter in a dark room
    /// and darker in a light one, and a recess inverts with them (§1.5).
    pub recess: Color,
    /// **The hanging wall**: the app background behind the shelf.
    pub wall: Color,
    /// **One step up from the wall**: the album inspector's column, the
    /// popover, a resting control.
    ///
    /// A plinth is the thing a work stands on. It was called `CARD`, which is
    /// web-app vocabulary and, under this direction, a lie — there are no
    /// cards, and the shelf in particular may never be drawn on one.
    pub plinth: Color,
    /// **One step above [`Palette::plinth`]**: a selected segment, the playing
    /// row, a hovered control. Never a resting state.
    pub plinth_lit: Color,
    /// Primary text — the ink the wall label is printed in.
    ///
    /// [`Palette::paper_dim`], [`Palette::paper_faint`] and
    /// [`Palette::paper_muted`] are **the same r : g : b ratios scaled**, so
    /// the ink family is one board at four levels of light rather than four
    /// greys that drifted apart. Each is the *smallest* point on that ramp
    /// that clears its floor on every surface it can land on, with 0.1 of
    /// margin.
    pub paper: Color,
    /// Secondary text: artists, captions, subtitles. Never a figure that
    /// ticks — those are primary or tertiary, never in between.
    pub paper_dim: Color,
    /// Tertiary text: counts, durations, hints, signal notes, the resting
    /// fader — present, never loud. This carries the whole of baz's readout
    /// vocabulary, so it is the ink with the least margin over its floor and
    /// the one the contrast test exists for.
    pub paper_faint: Color,
    /// A control that is *set* but not currently sounding: the volume fader
    /// while muted, or a stepper at the end of its travel. Not text a user
    /// must read, so the 3 : 1 non-text floor applies.
    pub paper_muted: Color,
    /// The accent. **Playback truth only** — see the module's
    /// accent-discipline note for the five places it may appear.
    ///
    /// Amber in a dark room, oxblood in a light one: a *different mark*, not a
    /// recoloured one, which is the critique's answer to `02` §10's objection
    /// that an amber halo has almost no contrast on a paper ground.
    pub lamp: Color,
    /// The accent, brightened — the seek fill under the pointer, Play hovered.
    pub lamp_bright: Color,
    /// The accent, deepened — the seek fill while dragged, Play pressed.
    pub lamp_deep: Color,
    /// Ink for text sitting *on* the accent: the Play button's label today,
    /// and nothing after step 14 revokes the lamp-filled rectangle.
    pub lamp_ink: Color,
    /// Problems, stated quietly. No alarm klaxon.
    pub alert: Color,
    /// Success (theme palette slot; nothing renders it directly yet).
    pub success: Color,
    /// The sleeve and popover drop shadow's colour.
    pub shadow: Color,
    /// The focus ring's opacity, **per room**.
    ///
    /// The one alpha a room may set for itself, and it has to be: the same
    /// opacity over a lighter ground is a *smaller* step, so the ring that
    /// measures 3.80 : 1 in Closing Time at 45 % measures 2.67 : 1 in Reading
    /// Room. It is the only alpha-expressed mark with a floor to clear (§1.6's
    /// exemption list covers the others), so it is the only one that varies.
    ring_alpha: f32,
}

/// Opacity of [`Palette::hairline`]: the room's ink at **7 %**.
///
/// Down from 8 %, and the *perceived* weight was unchanged: the same alpha
/// over a darker ground is a larger step, so holding a hairline steady across
/// the repaint meant lowering its number. iced 0.13's `Border` is four-sided,
/// so every single line in the product is a `rule` widget.
const HAIRLINE_A: f32 = 0.07;
/// Opacity of [`Palette::hairline_strong`]: the room's ink at **15 %** (down
/// from 17 %, for the reason [`HAIRLINE_A`] gives).
const HAIRLINE_STRONG_A: f32 = 0.15;
/// Opacity of [`Palette::select_wash`]: the room's ink at **18 %**.
const SELECT_WASH_A: f32 = 0.18;
/// Opacity of [`Palette::lamp_glow`]: the accent at **30 %**, blurred.
const LAMP_GLOW_A: f32 = 0.30;

impl Palette {
    /// Hairline border: findable when you look, invisible when you don't.
    /// The room's ink at [`HAIRLINE_A`].
    #[must_use]
    pub const fn hairline(&self) -> Color {
        alpha(self.paper, HAIRLINE_A)
    }

    /// The hairline, firmer — a selected control's edge, the playing row's
    /// edge. The room's ink at [`HAIRLINE_STRONG_A`].
    #[must_use]
    pub const fn hairline_strong(&self) -> Color {
        alpha(self.paper, HAIRLINE_STRONG_A)
    }

    /// Keyboard focus, on the focused `text_input`'s border and nowhere else.
    ///
    /// Deliberately **not** the accent. Where the keyboard is has nothing to
    /// do with where the music is, and the search field takes focus at
    /// launch — so an amber focus ring made the first frame baz ever drew a
    /// lit lamp with nothing playing.
    #[must_use]
    pub const fn paper_ring(&self) -> Color {
        alpha(self.paper, self.ring_alpha)
    }

    /// Selected text in a `text_input`.
    ///
    /// Also not the accent, and for the same reason as
    /// [`Palette::paper_ring`]: a selection is a fact about the keyboard, not
    /// about the music. A wash rather than a fill, so the glyphs under it keep
    /// their own ink — which is why the contrast test measures the *ink on the
    /// composited wash* rather than the wash itself.
    #[must_use]
    pub const fn select_wash(&self) -> Color {
        alpha(self.paper, SELECT_WASH_A)
    }

    /// The accent as a glow: the playing sleeve's halo, and nothing else.
    #[must_use]
    pub const fn lamp_glow(&self) -> Color {
        alpha(self.lamp, LAMP_GLOW_A)
    }

    /// The transport glyphs' ink — the same primary ink the labels they
    /// replaced are set in.
    #[must_use]
    pub const fn glyph(&self) -> Color {
        self.paper
    }

    /// The room's four planes, **in elevation order**, named.
    ///
    /// Order is load-bearing: it is what the oklch-L step law is asserted
    /// over, and "adjacent" means adjacent *here*.
    #[must_use]
    pub const fn surfaces(&self) -> [(&'static str, Color); 4] {
        [
            ("recess", self.recess),
            ("wall", self.wall),
            ("plinth", self.plinth),
            ("plinth_lit", self.plinth_lit),
        ]
    }

    /// Whether `color` is this room's accent or one of its relatives.
    ///
    /// Membership of the accent family **by value**, rather than a hue test:
    /// the tokens are constants, so what has to be prevented is a *style*
    /// reaching for one of them, not a new colour that happens to be warm.
    #[must_use]
    pub fn is_accent(&self, color: Color) -> bool {
        [
            self.lamp,
            self.lamp_bright,
            self.lamp_deep,
            self.lamp_glow(),
            self.lamp_ink,
        ]
        .iter()
        .any(|accent| {
            (accent.r - color.r).abs() < f32::EPSILON
                && (accent.g - color.g).abs() < f32::EPSILON
                && (accent.b - color.b).abs() < f32::EPSILON
                && (accent.a - color.a).abs() < f32::EPSILON
        })
    }
}

/// `color` at `opacity`, spelled out field by field so it is usable in a
/// `const fn`.
const fn alpha(color: Color, opacity: f32) -> Color {
    Color {
        r: color.r,
        g: color.g,
        b: color.b,
        a: opacity,
    }
}

/// **Closing Time** — a record archive after hours. The works are lit; the
/// room is not.
///
/// The wall is near-black and *neutral-cool*, the matte paint of a black-cube
/// gallery, and the type is warm ivory, the colour of archival mount board:
/// **the room is cold and the paper is warm**, which is what a gallery looks
/// like at night and is the one decision that keeps a near-black grid of
/// covers from reading as every other media app.
///
/// The four surfaces are unchanged from the values v0.1 shipped, and they were
/// kept rather than replaced by the critique's: measured in oklch L they
/// already satisfy its own ≥ 0.03 elevation law on all three steps (+0.0311 /
/// +0.0367 / +0.0360) without having been designed to, where the critique's
/// Closing Time steps `#070809` → `#0C0D0E` by **+0.0248** and fails it
/// (ADR-0017 §1.6).
pub const CLOSING_TIME: Palette = Palette {
    room: Room::ClosingTime,
    name: "Closing Time",
    // #060708 / #0C0D0E / #141517 / #1C1D20
    recess: Color::from_rgb(0.024, 0.027, 0.031),
    wall: Color::from_rgb(0.047, 0.051, 0.055),
    plinth: Color::from_rgb(0.078, 0.082, 0.090),
    plinth_lit: Color::from_rgb(0.110, 0.114, 0.125),
    // #E8E4DB / #ABA8A1 / #888680 / #6C6A66 — archival mount board at four
    // levels of light. `paper_faint` and `paper_muted` are the two v0.1
    // shipped below their floors (3.4 : 1 and 1.9 : 1); the contrast test
    // pins both corrections as corrections.
    paper: Color::from_rgb(0.910, 0.894, 0.859),
    paper_dim: Color::from_rgb(0.671, 0.659, 0.631),
    paper_faint: Color::from_rgb(0.533, 0.525, 0.502),
    paper_muted: Color::from_rgb(0.424, 0.416, 0.400),
    // #E3A14E — amplifier-lamp amber, the power lamp / VU glow.
    lamp: Color::from_rgb(0.890, 0.631, 0.306),
    lamp_bright: Color::from_rgb(0.945, 0.702, 0.384),
    lamp_deep: Color::from_rgb(0.780, 0.533, 0.239),
    lamp_ink: Color::from_rgb(0.106, 0.078, 0.043),
    alert: Color::from_rgb(0.851, 0.467, 0.420),
    success: Color::from_rgb(0.525, 0.663, 0.486),
    shadow: Color::from_rgba(0.0, 0.0, 0.0, 0.45),
    ring_alpha: 0.45,
};

/// **Reading Room** — the mirror: a warm paper ground under a cool ink.
///
/// Defined, and **not yet selectable** ([`follow`]). §1.5 ships it only with
/// an answer to the one objection the critique did not close — a pale sleeve
/// on a paper ground, whose remedy may not be a border on artwork — and that
/// answer is step 20's, not step 2's. What lands here are its tokens, so that
/// every style function below is written against a room from the first day
/// rather than against a near-black constant.
///
/// Three things invert deliberately, and each is a decision rather than a
/// negation of a number:
///
/// - **Elevation.** Surfaces rise toward the lamp, so a plinth is *darker*
///   than the wall here, and `recess` — which is below the wall — is the
///   lightest plane in the room. The steps are 0.034 / 0.036 / 0.035 oklch L,
///   the same tread Closing Time climbs.
/// - **The material.** Closing Time is a cool room with warm paper; this is a
///   warm room with cool ink, so the pairing that stops a flat grey UI is kept
///   and only its direction is reversed.
/// - **The lamp.** `oklch(0.50 0.14 35)` — oxblood, `#A33E25`. Amber on paper
///   is a stain; oxblood is a different mark, which is the critique's own
///   answer and is adopted verbatim.
pub const READING_ROOM: Palette = Palette {
    room: Room::ReadingRoom,
    name: "Reading Room",
    // #FAF6EF / #EEEBE4 / #E3DFD8 / #D7D4CD — one warm ivory at four levels,
    // descending as they rise.
    recess: Color::from_rgb(0.980, 0.965, 0.937),
    wall: Color::from_rgb(0.933, 0.922, 0.894),
    plinth: Color::from_rgb(0.890, 0.875, 0.847),
    plinth_lit: Color::from_rgb(0.843, 0.831, 0.804),
    // #1E2226 / #393E42 / #575B60 / #70757B — one cool ink at four levels.
    // `paper_faint` measures 4.62 : 1 on `plinth_lit`, the same margin the
    // dark room's tertiary ink carries on the surface it has least room on.
    paper: Color::from_rgb(0.118, 0.133, 0.149),
    paper_dim: Color::from_rgb(0.224, 0.243, 0.259),
    paper_faint: Color::from_rgb(0.341, 0.357, 0.376),
    paper_muted: Color::from_rgb(0.439, 0.459, 0.482),
    // #A33E25 — oxblood, the critique's light-room accent, verbatim.
    lamp: Color::from_rgb(0.639, 0.243, 0.145),
    lamp_bright: Color::from_rgb(0.745, 0.337, 0.239),
    lamp_deep: Color::from_rgb(0.537, 0.141, 0.031),
    lamp_ink: Color::from_rgb(0.965, 0.953, 0.925),
    alert: Color::from_rgb(0.608, 0.118, 0.133),
    success: Color::from_rgb(0.208, 0.424, 0.220),
    shadow: Color::from_rgba(0.0, 0.0, 0.0, 0.45),
    // 0.55, not 0.45: see `Palette::ring_alpha`.
    ring_alpha: 0.55,
};

/// Whether [`READING_ROOM`] may be resolved yet.
///
/// **The §1.5 gate, as one constant.** The light room ships when — and only
/// when — the pale-sleeve-on-paper question has an answer that is not a border
/// on artwork (build-plan step 20). Until then the room's tokens exist, every
/// test sweeps them, and nothing selects them. Flipping this is the whole of
/// what "ship the second room" costs in this module.
const READING_ROOM_SHIPS: bool = false;

/// What the desktop says it prefers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Appearance {
    /// A dark desktop, or no answer at all.
    Dark,
    /// A light desktop.
    Light,
}

/// The desktop's preference, read through iced.
///
/// **No new dependency**: `iced_core`'s `auto-detect-theme` feature is on by
/// default in the version baz already links, and `Theme::default()` is the
/// answer of the `dark-light` crate it pulls — which on Linux asks the
/// freedesktop portal over the same D-Bus stack MPRIS already uses. baz never
/// calls `Theme::default()` for a *theme* (it installs its own, [`theme`]), so
/// reading it here spends the detection on the only question baz has.
#[must_use]
pub fn system_appearance() -> Appearance {
    match Theme::default() {
        Theme::Light => Appearance::Light,
        _ => Appearance::Dark,
    }
}

/// The room to stand in, given what the desktop prefers.
///
/// Pure, so the whole of "follow the OS" is testable without a desktop. Note
/// the asymmetry, which is deliberate: `dark-light` reports "no preference"
/// and "light" identically once iced has mapped them, so **only a positive
/// light answer leaves Closing Time**. A machine with no portal, no session
/// bus and no answer gets the room baz is, not the room a failed probe
/// defaulted to.
#[must_use]
pub fn follow(appearance: Appearance) -> &'static Palette {
    match appearance {
        Appearance::Light if READING_ROOM_SHIPS => &READING_ROOM,
        _ => &CLOSING_TIME,
    }
}

/// The room this process resolved at startup.
static ACTIVE: OnceLock<&'static Palette> = OnceLock::new();

/// Resolve the room and stand in it, once, at startup.
///
/// Called from `main` before the first frame, so that every [`active`] read —
/// including [`crate::icon`]'s glyph sheet, which bakes the ink into a
/// sprite — sees the same room. Calling it twice is a no-op rather than a
/// panic: the room is a startup fact, and a second opinion about it is not
/// worth crashing a music player over.
pub fn install() -> &'static Palette {
    let room = match std::env::var("BAZ_ROOM").as_deref() {
        // A development hatch, not a product surface: there is no room picker
        // until step 22 and no second selectable room until step 20, and the
        // light room's surfaces still have to be *looked at* before either.
        Ok("closing-time") => &CLOSING_TIME,
        Ok("reading-room") => &READING_ROOM,
        _ => follow(system_appearance()),
    };
    let _ = ACTIVE.set(room);
    active()
}

/// The room standing now.
///
/// [`CLOSING_TIME`] until [`install`] says otherwise, which is what makes
/// every unit test in the crate deterministic without a desktop: a test that
/// cares about a room names it, and one that does not gets the room baz is.
#[must_use]
pub fn active() -> &'static Palette {
    ACTIVE.get().copied().unwrap_or(&CLOSING_TIME)
}

// ---------------------------------------------------------------------------
// Type scale
// ---------------------------------------------------------------------------

// Every size below carries its own line height, as a `LineHeight::Relative`
// factor named beside it. baz used to take iced 0.13's toolkit default of 1.3
// everywhere, which is a single compromise applied to type from 11 px to 28 px:
// small type wants air and a heading wants none, and a caption set at the same
// leading as a hero is why a two-line block can read as a paragraph. The pairs
// are the type scale of `.interface-design/system.md` §8, and a `text` widget
// that sets a size without its leading is a review-blocking defect for the same
// reason a hardcoded colour is (ADR-0006).

/// Hints and footnotes (11 px).
pub const SIZE_CAPTION: f32 = 11.0;
/// Leading for [`SIZE_CAPTION`]: the loosest in the scale, because the smallest
/// type is the type that needs the air.
pub const LEADING_CAPTION: f32 = 1.45;
/// Metadata: captions, durations, status counts (12 px).
pub const SIZE_META: f32 = 12.0;
/// Leading for [`SIZE_META`].
pub const LEADING_META: f32 = 1.35;
/// Body: tile titles, track titles, control labels (13 px).
pub const SIZE_BODY: f32 = 13.0;
/// Leading for [`SIZE_BODY`] — and, through [`CAPTION_LINE_H`], the height of
/// a wall label's line.
pub const LEADING_BODY: f32 = 1.40;
/// Emphasis: search text, panel artist, empty-state lines (15 px).
pub const SIZE_EMPHASIS: f32 = 15.0;
/// Leading for [`SIZE_EMPHASIS`].
pub const LEADING_EMPHASIS: f32 = 1.35;
/// Titles: the side panel's album title (19 px).
pub const SIZE_TITLE: f32 = 19.0;
/// Leading for [`SIZE_TITLE`]: tight, because a two-line album title is one
/// object and should look like one.
pub const LEADING_TITLE: f32 = 1.20;
/// Hero: the first-run question (28 px).
pub const SIZE_HERO: f32 = 28.0;
/// Leading for [`SIZE_HERO`]: the tightest in the scale.
pub const LEADING_HERO: f32 = 1.15;

/// The UI face at Regular: baz's default font, and the family every weight
/// below is a member of.
///
/// **Named, never generic.** `Font::DEFAULT` is `Family::SansSerif`, which
/// each platform resolves for itself — and asking an unknown family for
/// Medium or Semibold is how baz used to end up rendering tile titles in
/// whatever the host's fallback chain reached for (a monospace, on the design
/// audit's machine). The family is bundled: see [`crate::font`].
pub const SANS: Font = Font::with_name(crate::font::SANS);
/// Medium weight of the UI face — quiet prominence for titles and labels.
/// A real drawn face in the bundled family, not a synthesised weight.
pub const MEDIUM: Font = Font {
    weight: Weight::Medium,
    ..SANS
};
/// Semibold weight of the UI face — headings only. Also a real drawn face.
pub const SEMIBOLD: Font = Font {
    weight: Weight::Semibold,
    ..SANS
};

// There is deliberately **no monospace token**, and no monospace face. Every
// figure baz draws — track numbers, durations, counts, dB values, sample rates,
// queue positions — is set in [`SANS`], because Plex Sans's digits are already
// tabular: 600/1000 em in Regular, Medium and SemiBold alike, the same advance
// the deleted Plex Mono gave. `crate::font`'s
// `the_sans_carries_baz_s_tabular_figures_in_every_weight_it_sets_them_in`
// measures that, and `no_monospace_survives_anywhere_in_the_crate` below keeps
// the second face from creeping back. The argument is
// `.interface-design/system.md` §8; the owner's complaint that started it was
// that the readouts looked like a typewriter.

// ---------------------------------------------------------------------------
// Spacing (base unit 4) and shape
// ---------------------------------------------------------------------------

/// 2 px — intra-block line gaps.
pub const GAP_XXS: f32 = 2.0;
/// 4 px — caption-to-title, dot-to-label.
pub const GAP_XS: f32 = 4.0;
/// 8 px — sibling elements within a group.
pub const GAP_SM: f32 = 8.0;
/// 12 px — groups within a surface.
pub const GAP_MD: f32 = 12.0;
/// 16 px — surface padding, bar gutters.
pub const GAP_LG: f32 = 16.0;
/// 24 px — screen-level breathing room.
pub const GAP_XL: f32 = 24.0;

/// **The grid's one number** (logical px): the distance from a work to its
/// neighbour *and* from a work to the edge of the wall.
///
/// 40, and it is one token rather than two because it is one decision — a hang
/// whose works sit 40 px apart and 24 px from the wall is a grid with a frame
/// round it, which is the thing a gallery does not have. The arithmetic that
/// spends it is [`crate::shelf::Grid`], and the property it buys is that
/// **whenever the art is not capped the gutter is exactly `HANG`**, so there
/// is no dead gutter at any width (`.interface-design/system.md` §7).
///
/// It replaces the shelf's `GRID_PADDING` of 24 and its 32 px art-to-art gap,
/// both of which were constants a cell was measured against rather than a
/// number the wall was hung by.
pub const HANG: f32 = 40.0;
/// Smallest edge a sleeve may be drawn at (logical px).
///
/// The column count's *ceiling* is whatever keeps the art at or above this, so
/// a wide window gains a column only when the column it gains is still worth
/// looking at.
pub const ART_MIN: f32 = 240.0;
/// The edge the column count aims for (logical px).
///
/// Not a size the art is ever drawn at — the art absorbs whatever the chosen
/// column count leaves — but the size the *count* is chosen around, which is
/// why it sits between [`ART_MIN`] and [`ART_MAX`] rather than at either end.
pub const ART_TARGET: f32 = 272.0;
/// Largest edge a sleeve may be drawn at (logical px).
///
/// `4/3 × ART_MIN` deliberately, so at every column-count change the art hands
/// off from its largest to its smallest with no ambiguity: 320 → 240 at
/// exactly one width per transition.
///
/// It is also **exactly [`crate::art::THUMB_PX`]**, which is the refusal *no
/// artwork is ever drawn larger than its source* expressed as an equation
/// rather than as a hope; `the_wall_never_draws_art_larger_than_its_source`
/// asserts it.
pub const ART_MAX: f32 = 320.0;
/// Height of a wall label: two lines at [`SIZE_BODY`]'s leading, **36.4**.
///
/// The name `.interface-design/system.md` §8 gives [`CAPTION_H`], which is the
/// same number in the module that draws it. Kept as an alias rather than
/// collapsed, because the hang's row pitch is arithmetic about a *label* and
/// the tile's reserved block is arithmetic about a *caption*, and they are the
/// same 36.4 for a reason worth being able to state twice.
pub const LABEL_H: f32 = CAPTION_H;

// The radii come down across the board, because **an archive is rectilinear
// and a sleeve has square corners** (`.interface-design/system.md` §6). Artwork
// is radius 0 always, and every rule is too; what is left is barely rounded
// rather than softly rounded, and the nesting rule still holds — 3 inside 4.

/// Corner radius for controls (buttons, inputs, wells, steppers, the popover).
/// **4**, down from 6.
pub const RADIUS_CTRL: f32 = 4.0;
/// Corner radius of a segment inside the segmented control, a checkbox, a
/// queue or track row — one step tighter than the well enclosing it, so the
/// raised segment nests rather than straining against the edge. **3**, down
/// from 4.
pub const RADIUS_SEGMENT: f32 = 3.0;
/// Inset of the segmented control's well around its segments.
pub const SEGMENT_INSET: f32 = 2.0;
/// Width of the album inspector, the column beside the shelf (logical px).
///
/// **One number, and now for one surface.** It was one number for three — the
/// album, the queue and the settings took turns in this width — and that shared
/// width was the only thing they had in common, which is what ADR-0016 is
/// about. What survives the move is the property the layout actually rests on:
/// the column is either showing an album or it is not, and swapping which album
/// can never change how much room the shelf has. Only opening and closing
/// reflow the grid, by exactly this much, and `app.rs`'s estimate is kept in
/// step with it (see [`crate::selection`]).
pub const PANEL_W: f32 = 340.0;
/// Width of the number column in a track or queue list (logical px). Enough
/// for three figures at [`SIZE_META`], so a long queue's positions
/// stay in their column.
pub const TRACK_NO_W: f32 = 24.0;
/// Corner radius for small floating chips — the seek preview tip, the
/// tooltips. **3**, down from 4.
pub const RADIUS_CHIP: f32 = 3.0;
/// Edge of the playing-album lamp dot (a [`RADIUS_CTRL`]-free circle).
pub const DOT: f32 = 6.0;

/// Thickness of a groove's rail — a groove, not a gauge.
pub const RAIL: f32 = 4.0;
/// Vertical slop above *and* below the [`RAIL`] that still counts as the
/// seek bar. A 4 px groove is a 4 px target, which is a miss waiting to
/// happen (Fitts); the pointer gets a band an order of magnitude taller to
/// aim at, and the cursor changes across the whole of it.
pub const HIT_SLOP: f32 = 9.0;
/// Hit height of the seek bar: the groove plus [`HIT_SLOP`] on each side.
/// The widget draws the rail centered in it.
pub const RAIL_HIT: f32 = RAIL + 2.0 * HIT_SLOP;
/// Radius of the seek handle at rest.
pub const KNOB: f32 = 5.0;
/// Radius of the seek handle while hovered or held — the control grows
/// under the pointer rather than changing color alone.
pub const KNOB_ACTIVE: f32 = 7.0;
/// Minimum width the seek bar is given in the now-playing bar.
pub const SEEK_W: f32 = 260.0;
/// Width reserved for each of the seek bar's timestamps: enough for `h:mm:ss`
/// at [`SIZE_META`]. Fixed, so the groove keeps its place when a track crosses
/// the hour mark or a stamp gains a digit — the same reason an undeclared
/// length renders as `--:--` rather than as nothing.
///
/// The number is unchanged from the build that set it in the monospace, and it
/// gained a capability by standing still: `10:00:00` measures 57.60 px in Plex
/// Mono, so the shipped build *clipped* a ten-hour track in this very slot, and
/// 50.21 px in Plex Sans, which it holds with 1.79 px to spare. `crate::font`
/// measures both.
pub const STAMP_W: f32 = 52.0;
/// Height of the lane the hover preview floats in, directly above the
/// groove. Reserved whether or not anything is hovering, so the bottom bar
/// never changes height under the pointer.
pub const PREVIEW_H: f32 = 15.0;
/// Width of the hover-preview tip: enough for `h:mm:ss` at [`SIZE_CAPTION`]
/// plus its padding, fixed so the tip can be centered on the pointer without
/// measuring text.
///
/// **48**, re-derived from the Sans's real advances: `0:00:00` is 39.42 px at
/// caption size, and a [`GAP_XS`] of padding on each side is 47.42. The tip is
/// the tightest slot in the bar and it is meant to be — it floats over the
/// groove, and every pixel of it is a pixel of the track it is describing.
pub const PREVIEW_W: f32 = 48.0;

// ---------------------------------------------------------------------------
// The volume control
// ---------------------------------------------------------------------------

/// Width of the volume fader's groove.
///
/// Shorter than the seek bar on purpose: a seek bar is a *map of the track*
/// and wants resolution, while a fader is a setting and wants to sit quietly
/// in the corner. 96 px still gives ~10 control positions per pixel, which is
/// ~0.26 dB at the top of the taper — finer than a hand can aim and two
/// hundred times finer than the ~1 dB a listener hears as a change.
pub const VOLUME_W: f32 = 96.0;
/// Width of the level tip that floats over the volume groove on hover:
/// enough for `-18.1 dB` at [`SIZE_CAPTION`] plus its padding.
///
/// **48**, where the monospace needed 62: the widest thing this slot draws
/// measures 43.34 px in the Sans, because only its four figures cost 0.6 em
/// and `dB`, the point and the sign do not.
pub const LEVEL_W: f32 = 48.0;
/// Width of the detent mark on a groove's travel.
pub const DETENT_W: f32 = 2.0;
/// Height of the detent mark.
pub const DETENT_H: f32 = 5.0;
/// Clearance between the top of the handle and the bottom of the detent
/// mark. The mark is lifted clear of the knob rather than drawn under it —
/// see [`crate::groove::Detent`].
pub const DETENT_GAP: f32 = 2.0;
/// Hit height of the volume groove: the rail plus, on each side, room for
/// the knob and the detent mark above it. Taller than [`RAIL_HIT`] because
/// the mark has to live somewhere the handle is not.
pub const VOLUME_HIT: f32 = RAIL + 2.0 * (KNOB + DETENT_GAP + DETENT_H);
/// Height of the volume block: the level-preview lane over the groove,
/// reserved whether or not the pointer is anywhere near it.
pub const VOLUME_ROW_H: f32 = PREVIEW_H + VOLUME_HIT;
/// Width of the whole volume block — the mute affordance, a gap, the
/// groove. Fixed, so neither a volume change, a mute, nor the fader's own
/// hover can move anything beside it.
pub const VOLUME_BLOCK_W: f32 = TRANSPORT_HIT + GAP_SM + VOLUME_W;

/// The detent mark's ink, faint at rest and full paper when the handle is
/// sitting on it.
///
/// Deliberately *not* lamp amber even when engaged. Unity is a property of
/// the control, not a claim about what is playing, and the accent is
/// reserved (see the palette rationale). What distinguishes "on the detent"
/// from "a pixel below it" is a five-fold jump in ink weight on a 2 px mark
/// — findable when you look for it, invisible when you are not.
#[must_use]
pub fn detent_ink(p: &Palette, engaged: bool) -> Color {
    if engaged { p.paper } else { p.hairline() }
}

// ---------------------------------------------------------------------------
// The transport controls
// ---------------------------------------------------------------------------

/// Edge of a transport glyph (play/pause/next), in logical pixels. The
/// sprite is drawn into a box exactly this size, so the glyph in it can
/// never change the layout — see [`crate::icon`].
pub const ICON_PX: f32 = 16.0;
/// Edge of a transport button's square hit area. Comfortably above the
/// glyph so the pointer aims at a target rather than at a shape, and fixed
/// in both axes so play and pause occupy identically many pixels.
pub const TRANSPORT_HIT: f32 = 32.0;
/// Opacity of a glyph on a live control.
pub const GLYPH_OPACITY: f32 = 1.0;
/// Opacity of a glyph while its command is in flight: the whole of the
/// pending affordance. A control that dims a little and comes back changes
/// no size, no shape, and no meaning — which is the difference between an
/// affordance and the flash the bottom bar used to have (the argument, and
/// the measured round trip, are in [`crate::player`]'s module docs).
pub const GLYPH_OPACITY_PENDING: f32 = 0.55;
/// Opacity of a glyph on a control that genuinely cannot act — no engine,
/// or nothing queued. Lands on roughly [`Palette::paper_faint`] over [`Palette::plinth`], the
/// weight the rest of the room gives inert text.
pub const GLYPH_OPACITY_DISABLED: f32 = 0.45;

/// Height of the bottom bar's seek row: the hover-preview lane plus the
/// groove's hit band. Reserved whether or not there is anything to seek, so
/// the bar keeps its height from launch through play to stop.
pub const SEEK_ROW_H: f32 = PREVIEW_H + RAIL_HIT;
/// Width of the bottom bar's centre column: a timestamp, the groove, a
/// timestamp, and the gaps between them. The transport row centres itself
/// over this, and the column is fixed so the whole block stays put.
pub const SEEK_ROW_W: f32 = SEEK_W + 2.0 * (STAMP_W + GAP_SM);

/// Width reserved at the end of the bottom bar for the signal-path readout
/// (`48 → 44.1 kHz`, [`crate::player::PlayerState::signal_note`]).
///
/// *Reserved*, not sized to content: the readout appears only when the engine
/// is converting, and a bar that shuffled its status line sideways the moment
/// a 48 kHz album met a 44.1 kHz-only device would be announcing the thing
/// this indicator is specifically not supposed to announce. The slot is
/// always there and usually empty.
///
/// Wide enough for the longest chain a consumer device produces —
/// `192 → 176.4 kHz`, which measures 92.38 px in the Sans against the 108 the
/// monospace charged for the same fifteen glyphs — with room to spare.
///
/// **96**, down from 120. The 24 px this gives back is the largest single
/// saving of the face change, and it goes to the bar's left zone, which is the
/// zone that clips.
pub const SIGNAL_W: f32 = 96.0;

/// Width of a vertical scrollbar, and of the lane a scrolling list keeps
/// clear for it (logical px).
///
/// iced draws a `scrollable`'s bar **over** the content's right edge rather
/// than beside it, which is what clipped the side panel's durations from
/// `1:15` to `1:1` the moment a track list was long enough to scroll. The fix
/// is a lane the content does not use, and the number has to be the bar's own
/// width or the lane is a guess: [`list_scrollbar`] builds the bar from this
/// token and [`scroll_gutter`] reserves the same token, so the two are one
/// decision rather than two that have to agree.
///
/// Ten is iced 0.13's own default bar width, kept rather than changed — this
/// is a layout defect, not a restyle.
pub const SCROLLBAR_W: f32 = 10.0;
/// Clearance on each side of the scrollbar within its lane. Zero: the bar sits
/// in the lane's full width, so [`SCROLLBAR_LANE`] is [`SCROLLBAR_W`] and the
/// arithmetic stays visible rather than folded into a constant.
pub const SCROLLBAR_MARGIN: f32 = 0.0;
/// Total width a vertical scrollbar occupies: the bar and its margins.
pub const SCROLLBAR_LANE: f32 = SCROLLBAR_W + 2.0 * SCROLLBAR_MARGIN;

/// Edge of a stepper button's square hit area — the `−`/`+` beside a numeric
/// setting.
///
/// Smaller than [`TRANSPORT_HIT`] because these are not transport: a setting
/// is adjusted deliberately and rarely, where play and pause are hit in a
/// hurry. Still a square, and still fixed in both axes, so a value changing
/// under them moves nothing.
pub const STEPPER_HIT: f32 = 24.0;
/// Width reserved for a setting's value readout: enough for `−20.00 dB` at
/// [`SIZE_META`].
///
/// Fixed for the reason [`STAMP_W`] is: the digits change as the control is
/// driven, and a row that re-flowed under a repeated press would make the
/// button move away from the pointer holding it.
///
/// **60**, from a measured 56.89 px. This is also the one slot in the product
/// where a proportional face could still jiggle, and the jiggle is fixed at the
/// source rather than padded around: hyphen-minus advances 0.399 em where `+`
/// and U+2212 both advance 0.600, so `-20.00 dB` and `+20.00 dB` used to differ
/// by 2.4 px and shift this right-aligned slot's *left* edge as the pre-amp
/// stepped through zero. [`crate::replaygain::format_centidb`] emits U+2212, so
/// they now measure 56.89 px each, exactly — and the formatter agrees with the
/// `−` this very stepper already draws. The residual is `0.00 dB`, 7.2 px
/// narrower because it carries no sign at all, at one point in the travel,
/// changing only when a human presses a button.
pub const SETTING_VALUE_W: f32 = 60.0;
/// Height reserved for a setting's explanatory note: **two** lines at
/// [`SIZE_META`].
///
/// Reserved rather than fitted, because the note changes with the setting: the
/// ReplayGain modes' sentences are one line and two, so a slot that grew with
/// the text would shunt the pre-amps and the checkbox down by a line the
/// moment somebody pressed *Album* — a control moving out from under the
/// pointer that just chose it. Two lines is the tallest note the panel's
/// content width can produce (`a_setting_note_fits_the_slot_it_is_given`
/// pins it), and the empty half-slot in the short cases costs nothing.
pub const SETTING_NOTE_H: f32 = 2.0 * SIZE_META * LEADING_META;

/// The lane a scrolling list keeps clear for its scrollbar: padding on the
/// right of the list's contents and nowhere else.
///
/// Reserved **whether or not the list currently overflows**, on the same
/// principle as [`SEEK_ROW_H`] and [`SIGNAL_W`]: a gutter that appeared with
/// the scrollbar would shift every duration in the list sideways the moment
/// one more track arrived, which is a jump where there is currently a
/// clipped glyph. The cost when nothing is scrolling is ten invisible pixels.
#[must_use]
pub fn scroll_gutter() -> Padding {
    Padding {
        top: 0.0,
        right: SCROLLBAR_LANE,
        bottom: 0.0,
        left: 0.0,
    }
}

/// The scrollbar geometry a list uses, pinned to [`SCROLLBAR_W`] rather than
/// left to the toolkit's default, so that the bar and the lane
/// [`scroll_gutter`] reserves for it are the same number by construction.
#[must_use]
pub fn list_scrollbar() -> scrollable::Scrollbar {
    scrollable::Scrollbar::new()
        .width(SCROLLBAR_W)
        .scroller_width(SCROLLBAR_W)
        .margin(SCROLLBAR_MARGIN)
}

/// A list's scrollbar: no trough, and a scroller in the same hairline the room
/// uses for every other edge, one step firmer while it is being driven.
///
/// Quiet on purpose. A scrollbar is a *readout* of how much list there is, and
/// baz's chrome recedes so the covers and the type carry the interface; the
/// stock blue-grey iced draws otherwise is the one thing on screen that is not
/// from this palette.
#[must_use]
pub fn scrollbar(p: &Palette, status: scrollable::Status) -> scrollable::Style {
    let active = matches!(
        status,
        scrollable::Status::Hovered {
            is_vertical_scrollbar_hovered: true,
            ..
        } | scrollable::Status::Dragged {
            is_vertical_scrollbar_dragged: true,
            ..
        }
    );
    let rail = scrollable::Rail {
        background: None,
        border: Border::default(),
        scroller: scrollable::Scroller {
            color: if active {
                p.hairline_strong()
            } else {
                p.hairline()
            },
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: (SCROLLBAR_W / 2.0).into(),
            },
        },
    };
    scrollable::Style {
        container: container::Style::default(),
        vertical_rail: rail,
        horizontal_rail: rail,
        gap: None,
    }
}

/// A settings checkbox: the same quiet card as a resting control, with the
/// tick in paper ink.
///
/// No accent. Arming clipping prevention is a *setting*, not playback truth,
/// and the lamp is reserved (see [`panel_toggle`]); a checked box says so with
/// the surface step and the hairline the room already uses for "selected".
#[must_use]
pub fn check(p: &Palette, status: checkbox::Status) -> checkbox::Style {
    let (background, border_color) = match status {
        checkbox::Status::Active { is_checked } => (
            if is_checked { p.plinth_lit } else { p.recess },
            p.hairline_strong(),
        ),
        checkbox::Status::Hovered { .. } => (p.plinth_lit, p.hairline_strong()),
        checkbox::Status::Disabled { is_checked } => {
            (if is_checked { p.plinth } else { p.recess }, p.hairline())
        }
    };
    let disabled = matches!(status, checkbox::Status::Disabled { .. });
    checkbox::Style {
        background: Background::Color(background),
        icon_color: if disabled { p.paper_muted } else { p.paper },
        border: Border {
            color: border_color,
            width: 1.0,
            radius: RADIUS_SEGMENT.into(),
        },
        text_color: Some(if disabled { p.paper_muted } else { p.paper }),
    }
}

/// How strongly to ink a transport glyph.
///
/// Three states, one of which is not a state the *control* is in at all:
/// `pending` means a command has been sent and not yet confirmed, and the
/// only thing it is allowed to move is this number.
#[must_use]
pub fn glyph_opacity(enabled: bool, pending: bool) -> f32 {
    if !enabled {
        GLYPH_OPACITY_DISABLED
    } else if pending {
        GLYPH_OPACITY_PENDING
    } else {
        GLYPH_OPACITY
    }
}

/// The cursor over a live groove. `Pointer` — the pointing hand every
/// platform uses for "this responds to a click" — because clicking the bar
/// is the primary gesture here and dragging is the refinement, not the
/// other way round. (`Grab`, iced's slider default, promises a handle that
/// must be picked up first, which is not how these bars behave.)
pub const GROOVE_CURSOR: mouse::Interaction = mouse::Interaction::Pointer;
/// The cursor while a groove is held: the closed hand, so the difference
/// between "you may" and "you are" is visible without looking at the bar.
pub const GROOVE_CURSOR_HELD: mouse::Interaction = mouse::Interaction::Grabbing;
/// The cursor over a groove that cannot be driven (a track of undeclared
/// length, or a volume fader with no engine behind it): the plain arrow,
/// promising nothing.
pub const GROOVE_CURSOR_INERT: mouse::Interaction = mouse::Interaction::None;

/// Symmetric padding: `vertical` on top/bottom, `horizontal` on left/right.
#[must_use]
pub fn pad(vertical: f32, horizontal: f32) -> Padding {
    Padding {
        top: vertical,
        right: horizontal,
        bottom: vertical,
        left: horizontal,
    }
}

// ---------------------------------------------------------------------------
// Theme + widget styles
// ---------------------------------------------------------------------------

/// One iced `Theme` per room, built once.
///
/// Cached because `Theme::custom` allocates a name and `theme()` is called
/// once per frame, and built **per room** rather than from whichever room
/// happened to be standing when the first frame drew — a `LazyLock` that read
/// [`active`] would be a startup-order trap the day a second room becomes
/// selectable.
static THEMES: LazyLock<[Theme; Room::ALL.len()]> =
    LazyLock::new(|| Room::ALL.map(|room| iced_theme(room.palette())));

/// The iced `Theme` a room implies.
///
/// baz styles every widget it draws itself, so this carries only the five
/// colours iced falls back to for widgets baz has not styled — which should be
/// none of them, and is the reason it is worth keeping honest rather than
/// worth elaborating.
fn iced_theme(p: &Palette) -> Theme {
    Theme::custom(
        format!("baz {}", p.name),
        iced::theme::Palette {
            background: p.wall,
            text: p.paper,
            primary: p.lamp,
            success: p.success,
            danger: p.alert,
        },
    )
}

/// The application theme for the room standing now (cached; `Theme` clones
/// are `Arc`-cheap).
#[must_use]
pub fn theme() -> Theme {
    THEMES[active().room as usize].clone()
}

/// A shelf tile's button chrome: invisible at rest (the sleeve leads),
/// a quiet raised card on hover, one step higher plus a hairline edge when
/// selected.
///
/// **Radius 0**, where the tile had a `RADIUS_TILE` of 10. That token is
/// deleted: the shelf has no rectangles that are not artwork, and artwork is
/// always square (`.interface-design/system.md` §6). What is left here is
/// square-cornered chrome, which looks odd on purpose and briefly — B1 of the
/// adoption order deletes the tile's background and border outright and gives
/// hover and selection a rule under the *label* instead. This commit is values
/// only, so it does not reach for that.
#[must_use]
pub fn tile(p: &Palette, status: button::Status, selected: bool) -> button::Style {
    let mut style = button::Style {
        background: None,
        text_color: p.paper,
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 0.0.into(),
        },
        shadow: Shadow::default(),
    };
    if selected {
        style.background = Some(Background::Color(p.plinth_lit));
        style.border.color = p.hairline_strong();
        // Two pixels, not one: see [`SELECTION_EDGE`].
        style.border.width = SELECTION_EDGE;
    } else if matches!(status, button::Status::Hovered | button::Status::Pressed) {
        style.background = Some(Background::Color(p.plinth));
    }
    style
}

/// The artwork's frame: a soft drop shadow so the sleeve sits on the shelf;
/// the playing album trades it for a lamp-amber halo.
#[must_use]
pub fn sleeve(p: &Palette, playing: bool) -> container::Style {
    let shadow = if playing {
        Shadow {
            color: p.lamp_glow(),
            offset: Vector::ZERO,
            blur_radius: 16.0,
        }
    } else {
        Shadow {
            color: p.shadow,
            offset: Vector::new(0.0, 3.0),
            blur_radius: 8.0,
        }
    };
    container::Style {
        background: Some(Background::Color(p.recess)),
        shadow,
        ..container::Style::default()
    }
}

/// The playing album's lamp dot — the amplifier power light.
#[must_use]
pub fn lamp_dot(p: &Palette) -> container::Style {
    container::Style {
        background: Some(Background::Color(p.lamp)),
        border: iced::border::rounded(DOT / 2.0),
        ..container::Style::default()
    }
}

/// Quiet transport controls (play/pause, next): a card that raises on hover
/// and sinks on press.
#[must_use]
pub fn transport(p: &Palette, status: button::Status) -> button::Style {
    let (background, border, text_color) = match status {
        button::Status::Hovered => (p.plinth_lit, p.hairline_strong(), p.paper),
        button::Status::Pressed => (p.recess, p.hairline_strong(), p.paper),
        button::Status::Disabled => (p.plinth, p.hairline(), p.paper_faint),
        button::Status::Active => (p.plinth, p.hairline(), p.paper),
    };
    button::Style {
        background: Some(Background::Color(background)),
        text_color,
        border: Border {
            color: border,
            width: 1.0,
            radius: RADIUS_CTRL.into(),
        },
        shadow: Shadow::default(),
    }
}

/// The primary action (Play album): the only lamp-filled control on screen.
#[must_use]
pub fn primary(p: &Palette, status: button::Status) -> button::Style {
    let (background, text_color) = match status {
        button::Status::Active => (p.lamp, p.lamp_ink),
        button::Status::Hovered => (p.lamp_bright, p.lamp_ink),
        button::Status::Pressed => (p.lamp_deep, p.lamp_ink),
        button::Status::Disabled => (p.plinth, p.paper_faint),
    };
    button::Style {
        background: Some(Background::Color(background)),
        text_color,
        border: iced::border::rounded(RADIUS_CTRL),
        shadow: Shadow::default(),
    }
}

/// Text inputs (search, first-run folder): an inset well with a hairline
/// edge that brightens to a paper ring on focus.
///
/// **Not lamp amber, on either the ring or the selection.** Both used to be —
/// the ring at `LAMP` 55%, the selection at [`Palette::lamp_glow`] — and since the
/// search field takes focus at launch, the first frame baz ever drew was an
/// amber-ringed box with no music playing. A reserved signal that appears
/// before there is anything to signal is not reserved. Where the keyboard is,
/// and what it has selected, are facts about the keyboard; the accent means
/// playback truth (see the module's accent-discipline note).
///
/// iced 0.13's buttons take no keyboard focus, so this ring is the *only*
/// focus affordance the toolkit can render; icon-only controls are named by
/// tooltips instead ([`tooltip`]).
#[must_use]
pub fn input(p: &Palette, status: text_input::Status) -> text_input::Style {
    let border_color = match status {
        text_input::Status::Focused => p.paper_ring(),
        text_input::Status::Hovered => p.hairline_strong(),
        text_input::Status::Active | text_input::Status::Disabled => p.hairline(),
    };
    text_input::Style {
        background: Background::Color(p.recess),
        border: Border {
            color: border_color,
            width: 1.0,
            radius: RADIUS_CTRL.into(),
        },
        icon: p.paper_faint,
        placeholder: p.paper_faint,
        value: p.paper,
        selection: p.select_wash(),
    }
}

/// The seek bar: lamp amber elapsed running through a recessed groove, with
/// a small amber knob that grows under the pointer.
///
/// Position is playback truth, so it earns the accent — the same rule that
/// gives the playing sleeve its halo. The unplayed remainder is [`Palette::recess`]:
/// the groove is *cut into* the bar rather than laid on top of it, matching
/// the inset treatment of the input wells.
#[must_use]
pub fn seek(p: &Palette, status: slider::Status) -> slider::Style {
    let (fill, radius) = match status {
        slider::Status::Active => (p.lamp, KNOB),
        slider::Status::Hovered => (p.lamp_bright, KNOB_ACTIVE),
        slider::Status::Dragged => (p.lamp_deep, KNOB_ACTIVE),
    };
    slider::Style {
        rail: Rail {
            backgrounds: (Background::Color(fill), Background::Color(p.recess)),
            width: RAIL,
            border: Border {
                color: p.hairline(),
                width: 1.0,
                radius: (RAIL / 2.0).into(),
            },
        },
        handle: Handle {
            shape: HandleShape::Circle { radius },
            background: Background::Color(fill),
            border_width: 0.0,
            border_color: Color::TRANSPARENT,
        },
    }
}

/// The seek bar with nothing to scrub: a track of undeclared length, where
/// showing a proportional fill would be inventing one. The groove stays,
/// unfilled and knobless, so the bar's place in the layout does not jump
/// when a length does arrive.
#[must_use]
pub fn seek_inert(p: &Palette, _status: slider::Status) -> slider::Style {
    slider::Style {
        rail: Rail {
            backgrounds: (Background::Color(p.recess), Background::Color(p.recess)),
            width: RAIL,
            border: Border {
                color: p.hairline(),
                width: 1.0,
                radius: (RAIL / 2.0).into(),
            },
        },
        handle: Handle {
            shape: HandleShape::Circle { radius: 0.0 },
            background: Background::Color(Color::TRANSPARENT),
            border_width: 0.0,
            border_color: Color::TRANSPARENT,
        },
    }
}

/// The volume fader: the same recessed groove as the seek bar, inked in
/// paper rather than lamp amber, with a knob that does **not** grow.
///
/// Two deliberate differences from [`seek`], each with a reason:
///
/// - **No accent.** The lamp means playback truth (see the palette
///   rationale) — where the music is, which album is playing. A volume is a
///   *setting*, the same class of thing as the edition selector, so it is
///   drawn in the room's paper inks and brightens under the pointer instead.
///   A second amber control in the bar would dilute the one signal reserved
///   for the music itself.
/// - **A constant handle radius.** The seek knob grows under the pointer,
///   which shifts its centre by two pixels at the ends of the travel. That is
///   harmless on a bar with nothing else drawn on it; here it would drag the
///   unity detent along with it, and a detent that moves is not a detent. The
///   hover affordance is the ink, the cursor, and the level tip instead.
#[must_use]
pub fn volume(p: &Palette, status: slider::Status) -> slider::Style {
    let fill = match status {
        slider::Status::Active => p.paper_faint,
        slider::Status::Hovered | slider::Status::Dragged => p.paper_dim,
    };
    volume_style(p, fill)
}

/// The volume fader while muted: the position the listener chose is still
/// shown — mute does not move the fader, and pretending otherwise would lose
/// the very setting mute exists to restore — but in the ink of something that
/// is not currently sounding.
#[must_use]
pub fn volume_muted(p: &Palette, _status: slider::Status) -> slider::Style {
    volume_style(p, p.paper_muted)
}

/// The volume fader with no engine behind it: the groove keeps its place and
/// its detent, filled with nothing at all.
#[must_use]
pub fn volume_inert(p: &Palette, _status: slider::Status) -> slider::Style {
    volume_style(p, p.recess)
}

/// The shared shape of every volume-fader state: only the ink varies, so no
/// state of this control can move a pixel.
fn volume_style(p: &Palette, fill: Color) -> slider::Style {
    slider::Style {
        rail: Rail {
            backgrounds: (Background::Color(fill), Background::Color(p.recess)),
            width: RAIL,
            border: Border {
                color: p.hairline(),
                width: 1.0,
                radius: (RAIL / 2.0).into(),
            },
        },
        handle: Handle {
            shape: HandleShape::Circle { radius: KNOB },
            background: Background::Color(fill),
            border_width: 0.0,
            border_color: Color::TRANSPARENT,
        },
    }
}

/// The well holding the album's edition selector: the same inset treatment
/// as a text input, so a segmented control reads as a place you *choose*
/// something rather than a row of buttons that each do something.
#[must_use]
pub fn segmented(p: &Palette) -> container::Style {
    container::Style {
        background: Some(Background::Color(p.recess)),
        border: Border {
            color: p.hairline(),
            width: 1.0,
            radius: RADIUS_CTRL.into(),
        },
        ..container::Style::default()
    }
}

/// The seek bar's hover preview: a small card floating over the groove with
/// the timestamp the pointer is pointing at.
///
/// Deliberately *not* amber. The lamp is reserved for playback truth and for
/// positions actually asked for; a preview is neither — it is the room's
/// quietest card with a hairline edge, readable and forgettable.
#[must_use]
pub fn preview_tip(p: &Palette) -> container::Style {
    container::Style {
        background: Some(Background::Color(p.plinth_lit)),
        text_color: Some(p.paper_dim),
        border: Border {
            color: p.hairline_strong(),
            width: 1.0,
            radius: RADIUS_CHIP.into(),
        },
        ..container::Style::default()
    }
}

/// One segment of that control: the chosen format is a raised card in full
/// paper white; the others are label-only until the pointer finds them.
///
/// Deliberately *not* lamp amber. The accent means playback truth (see the
/// palette rationale) and a format choice is a view, not a claim about what
/// is playing — a second amber control in the panel would dilute the one
/// signal the room reserves.
#[must_use]
pub fn segment(p: &Palette, status: button::Status, selected: bool) -> button::Style {
    let (background, text_color) = if selected {
        (Some(p.plinth_lit), p.paper)
    } else {
        match status {
            button::Status::Hovered | button::Status::Pressed => (Some(p.plinth), p.paper),
            button::Status::Active | button::Status::Disabled => (None, p.paper_dim),
        }
    };
    button::Style {
        background: background.map(Background::Color),
        text_color,
        border: Border {
            color: if selected {
                p.hairline_strong()
            } else {
                Color::TRANSPARENT
            },
            width: 1.0,
            radius: RADIUS_SEGMENT.into(),
        },
        shadow: Shadow::default(),
    }
}

/// The album side panel: one quiet step above the wall.
#[must_use]
pub fn panel(p: &Palette) -> container::Style {
    container::Style {
        background: Some(Background::Color(p.plinth)),
        ..container::Style::default()
    }
}

/// A panel toggle in the top bar (today: Queue): label-only until the pointer
/// finds it, a raised card with a hairline edge while its panel is open.
///
/// The same treatment as [`segment`], and for the same reason: opening a panel
/// is a *view* choice, not a claim about what is playing, so the lamp stays
/// where it belongs. What "on" looks like is therefore a surface step and an
/// edge — the room's own way of saying a thing is selected — rather than a
/// second accent competing with the playing album's dot.
#[must_use]
pub fn panel_toggle(p: &Palette, status: button::Status, active: bool) -> button::Style {
    segment(p, status, active)
}

/// The now-playing bar: recessed below the wall, like the amp under the
/// shelf.
#[must_use]
pub fn bar(p: &Palette) -> container::Style {
    container::Style {
        background: Some(Background::Color(p.recess)),
        ..container::Style::default()
    }
}

/// Hairline rules dividing chrome from shelf.
#[must_use]
pub fn hairline(p: &Palette) -> rule::Style {
    rule::Style {
        color: p.hairline(),
        width: 1,
        radius: 0.0.into(),
        fill_mode: FillMode::Full,
    }
}

/// The name that floats over an icon-only control on hover — the same quiet
/// card as the seek preview, for the same reason: it is a label, not a
/// claim about playback.
///
/// iced 0.13 exposes no accessibility tree, so this tooltip *is* the
/// control's accessible name as far as the toolkit allows.
#[must_use]
pub fn tooltip(p: &Palette) -> container::Style {
    container::Style {
        background: Some(Background::Color(p.plinth_lit)),
        text_color: Some(p.paper_dim),
        border: Border {
            color: p.hairline_strong(),
            width: 1.0,
            radius: RADIUS_CHIP.into(),
        },
        ..container::Style::default()
    }
}

// ===========================================================================
// UX redesign, increments 1–5 (docs/design/01-ux-audit-and-ia.md §5)
//
// Appended as one block, at the end, deliberately: a parallel pass is
// rewriting the type stack and two contrast pairs above, and everything that
// pass touches is a *value* while everything here is a new name. Nothing in
// this section changes an existing token; the one edit made above it is a
// single line in `tile`, where the audit's "hover and selection are nearly the
// same mark" finding is spent (§4.4).
// ===========================================================================

/// Border width of a **selected** shelf tile (logical px).
///
/// Two, where hover is none and the surface step between the two states is one
/// [`Palette::plinth`] → [`Palette::plinth_lit`] tick. The audit's finding was that in a still
/// frame you cannot tell which tile is selected and which is merely under the
/// pointer — one surface step and a 1 px hairline apart is below the threshold
/// at which two states read as two states.
///
/// Doubling the edge is the smallest change that separates them, and it stays
/// inside the depth strategy: no shadow (reserved for artwork), no accent (
/// reserved for playback truth), no second surface step. It costs nothing in
/// layout either — iced draws a border inside the widget's bounds, so the
/// hang's column pitch is untouched.
pub const SELECTION_EDGE: f32 = 2.0;

/// Height of a shelf tile's caption block: **exactly two lines** at
/// [`SIZE_BODY`] (logical px).
///
/// Reserved rather than content-driven, which is the whole of the fix for the
/// audit's loudest complaint about the shelf: a two-line title used to push
/// its artist line down, so in one row four artists sat on one baseline and a
/// fifth sat 17 px lower. In a grid whose job is calm repetition that is the
/// most visible thing on screen after the art itself.
///
/// Two lines is the budget the caption actually needs — a title (clipped at
/// one line) over an `artist · year` line — and [`crate::shelf::Grid`]'s row
/// pitch already has the room, so nothing about the tile pitch moves. It is the same
/// reserved-slot rule as [`SETTING_NOTE_H`], [`SIGNAL_W`] and [`STAMP_W`]:
/// the space is always there and what varies is only what is in it.
///
/// Measured at [`SIZE_BODY`] for both lines even though the second is set at
/// [`SIZE_META`]: the block has to hold the *taller* possibility on each line,
/// and a slot sized to the smaller one would clip the moment a caption line
/// was set in body text.
pub const CAPTION_H: f32 = 2.0 * CAPTION_LINE_H;

/// One line of a shelf tile's caption (logical px) — the lane the title gets,
/// and the lane the artist gets.
///
/// The block is reserved as **two independent one-line lanes** rather than as
/// one two-line box, and that is the difference between fixing the defect and
/// moving it. `Wrapping::None` does not stop iced 0.13 breaking a long
/// paragraph (the same toolkit behaviour the audit caught in the bottom bar at
/// narrow widths, §1.5), so a title too long for its width still lays out two
/// lines — and inside a single two-line box it would push the artist out of
/// the bottom of the very slot that was reserved to keep it still.
///
/// Given a lane of its own, the title clips at exactly one line and **the
/// artist line sits on the same baseline on every tile of every row**, which
/// is the property §4.4 of the design spec is actually asking for. A clipped
/// title is the affordable failure here: the sleeve above it is the
/// identification a shelf is built on, and the album panel one click away
/// carries the whole string.
///
/// **18.2 px**, where it was 16.9: the lane is set from the body text's own
/// leading ([`LEADING_BODY`]) now that each type token carries one, instead of
/// from the toolkit's 1.3 default. The block is therefore 36.4 rather than
/// 33.8 — the number `.interface-design/system.md` §8 calls `LABEL_H`.
pub const CAPTION_LINE_H: f32 = SIZE_BODY * LEADING_BODY;

/// A track row — in the album inspector **and** in the **Queue** popover:
/// invisible at rest, a quiet card under the pointer, and the playing row
/// carded with a hairline edge.
///
/// The row became a control when clicking it started meaning "play from here"
/// (ADR-0014's `JumpTo`), and this is the affordance that admits it. Until
/// then the rows carried none — deliberately, because "an affordance that does
/// nothing is a lie" — so gaining one is the visible half of gaining the
/// behaviour, and it is the same rule read forwards. The queue's rows kept
/// their own container style for exactly as long as they were text; when they
/// became controls too, the two lists collapsed into **one** style function
/// rather than two that had to be kept token-for-token identical by hand. They
/// are, after all, the same twelve rows with the same mark on the same one, and
/// a listener who has seen one must not have to learn the other.
///
/// Hover sits one surface step below the playing row, so "the pointer is here"
/// and "this is what is sounding" stay distinguishable — the same separation
/// [`SELECTION_EDGE`] buys the shelf.
///
/// No accent anywhere: the lamp dot in the number column is the playback
/// truth, and a row that also washed amber would spend the signal twice.
#[must_use]
pub fn track_row(p: &Palette, status: button::Status, playing: bool) -> button::Style {
    let background = match (playing, status) {
        // The playing row keeps its card whatever the pointer is doing, and
        // lifts no further under it: it is already the emphasised row.
        (true, _) => Some(p.plinth_lit),
        (false, button::Status::Hovered | button::Status::Pressed) => Some(p.plinth),
        (false, button::Status::Active | button::Status::Disabled) => None,
    };
    button::Style {
        background: background.map(Background::Color),
        // The row's inks are set per-line by the view (a played row is fainter
        // than an upcoming one), so the button contributes none of its own.
        text_color: p.paper,
        border: Border {
            color: if playing {
                p.hairline_strong()
            } else {
                Color::TRANSPARENT
            },
            width: 1.0,
            radius: RADIUS_SEGMENT.into(),
        },
        shadow: Shadow::default(),
    }
}

// ---------------------------------------------------------------------------
// New tokens for surfaces landing in the visual redesign
//
// Deliberately parked at the end of the file rather than filed into the
// sections above: the redesign lands as several independent passes over
// different modules, and a token added here conflicts with nothing when two of
// them meet. Move each one up into its proper section once the surface that
// consumes it has shipped.
// ---------------------------------------------------------------------------

// There is no serif token either, and it never got a call site. Revision 1
// nominated Plex Serif SemiBold for exactly two jobs — the album's title and
// the first-run question — and said in the same paragraph that if one thing had
// to be cut to keep the design disciplined, it was this. The gallery direction
// is that moment: its whole thesis is that **the room supplies nothing and the
// work supplies everything**, and a display face is the room supplying
// personality. The album title is [`SEMIBOLD`] at [`SIZE_TITLE`]
// (`.interface-design/system.md` §8).

// ---------------------------------------------------------------------------
// The information-architecture move: places, an inspector, a popover, the bar
// (docs/design/01-ux-audit-and-ia.md §2, ADR-0016)
// ---------------------------------------------------------------------------

/// Width of the **Queue** popover (logical px).
///
/// 360, where the rail it left was [`PANEL_W`] 340. The extra twenty go to the
/// per-row ✕ the rows gained when they became interactive: the popover lists
/// exactly what the rail's queue panel listed, in the same row geometry, and
/// the removal target has to sit beside the duration column rather than on top
/// of it.
///
/// Fixed rather than proportional, and fixed at *less than a quarter of the
/// shipped window*: this is an overlay, and an overlay that grew with the
/// window would eventually be a panel that forgot to reflow the shelf. It
/// covers the bottom-right corner of the covers for a few seconds and no more.
pub const POPOVER_W: f32 = 360.0;

/// The tallest a popover may grow, as a fraction of the window's height.
///
/// A queue can be a box set, and a list that ran from the bar to the top bar
/// would be a place with no name. Six tenths leaves the shelf legible above it,
/// which is the whole argument for an overlay over a panel: glancing at what is
/// next must not cost the covers.
pub const POPOVER_MAX_H: f32 = 0.6;

/// Width reserved in the now-playing bar for the queue-position readout
/// (logical px) — the `3 / 12` beside the track title.
///
/// A **reserved slot**, exactly like [`SIGNAL_W`] and [`STAMP_W`]: the readout
/// is absent when nothing is playing and present when something is, and the bar
/// must not move between those two states.
///
/// **56, where it was 72 and was called `QUEUE_POS_W`.** The old number was 9
/// glyphs at the monospace's flat 0.6 em; the same string measures 53.46 px in
/// Plex Sans, because only the figures are 0.6 em and the space and the slash
/// are not. The design system names this slot `POSITION_W` and bounds it at
/// three figures a side (`199 / 240`), which is the same width as `999 / 999`
/// — the digits are tabular, so the widest three-figure queue and the widest
/// three-figure position measure identically. A four-figure queue
/// (`9999 / 9999`, 67.86 px) would clip, and that is a deliberate bound: no
/// album has 1000 tracks, and a whole-library shuffle queue is a different
/// surface's problem.
pub const POSITION_W: f32 = 56.0;

/// Width of the bar's **Queue** control (logical px) — the label, the
/// [`POSITION_W`] readout, and the padding around them.
///
/// The control is **labelled and always visible**, and that is a requirement
/// rather than a preference: `docs/design/03-interface-prior-art.md` §5.3(1)
/// and R1 record that the closest product to baz in ambition hides the same
/// surface behind an unlabelled gesture, and has generated years of "where is
/// my queue / what did I just do" complaints for it. *Transient must not mean
/// unverifiable.* So the door to the popover says what it opens, in words, in
/// every state — including with nothing playing, where the readout beside the
/// label is empty and the slot is still this wide.
pub const UP_NEXT_W: f32 = 152.0;

/// Width of the top bar's `Settings` control (logical px).
///
/// A reserved slot like the rest, but reserved for **one word** rather than for
/// a figure that changes. It was 92 px — a width fitted to the `Queue` toggle
/// it used to sit beside, so the pair would read as a pair — and at a 760 px
/// window the longer word wrapped to two lines inside it (§1.4 of the audit).
/// With the queue gone to the bar, the control has no twin to match and is
/// sized to its own label instead; `font.rs` measures `Settings` in the face
/// that draws it against this number less its padding.
pub const SETTINGS_TOGGLE_W: f32 = 84.0;

/// Width of the Settings place's section list (logical px).
///
/// A place needs a spine, and 200 px is what a list of one-word section names
/// wants: wide enough that *Appearance* and *Playback* never wrap, narrow
/// enough that it reads as navigation rather than as content. It is the one
/// piece of chrome the settings gain by becoming a place, and it is what makes
/// the next section an entry rather than a layout decision.
pub const SETTINGS_NAV_W: f32 = 200.0;

/// Greatest width the Settings place gives its content (logical px).
///
/// A settings form is a column of short labelled controls, and a control row
/// stretched across a 1600 px window is a line the eye has to travel twice to
/// read. 640 is roughly 55 characters at [`SIZE_BODY`] — the top of the
/// comfortable measure — and the content sits **left-aligned** in whatever
/// space is left rather than centred in it, so the form stays anchored to the
/// section list that names it.
pub const SETTINGS_CONTENT_W: f32 = 640.0;

/// Window width below which the Settings place stacks into one column
/// (logical px).
///
/// Under a thousand pixels the section list and a 640 px form cannot both have
/// their width, and of the two the *form* is the one being used. The list
/// becomes a heading above the content instead of a column beside it. One
/// branch, and it is the same branch the album inspector will need at its own
/// breakpoint (§4.3).
pub const SETTINGS_BREAKPOINT: f32 = 1000.0;

/// The **Queue** popover's surface: one step above the panel, a hairline
/// edge, and the room's one soft shadow.
///
/// Every part of this is chosen against something iced 0.13 cannot do (§4.6 of
/// the spec):
///
/// - **No arrow or notch.** Container borders here are four-sided only, so a
///   pointer triangle would have to be a second widget under a floating
///   element. The anchor is expressed by *position* — bottom right, above the
///   bar — and by the affordance below it taking its open styling.
/// - **No blur, no backdrop filter, and no scrim.** Separation is a surface
///   step, a hairline and the shadow, which is the depth strategy the whole
///   room already uses. Dimming ten thousand covers to show twelve rows would
///   contradict the palette rationale outright (§2.4).
///
/// The shadow is the *sleeve's* shadow, offset and blur alike: artwork is the
/// one thing in baz that casts one, and a floating layer is the one exception
/// that has to — so it borrows rather than invents.
#[must_use]
pub fn popover(p: &Palette) -> container::Style {
    container::Style {
        background: Some(Background::Color(p.plinth_lit)),
        border: Border {
            color: p.hairline_strong(),
            width: 1.0,
            radius: RADIUS_CTRL.into(),
        },
        shadow: Shadow {
            color: p.shadow,
            offset: Vector::new(0.0, 3.0),
            blur_radius: 8.0,
        },
        ..container::Style::default()
    }
}

/// The now-playing block in the bar, once it became the door to **Queue**.
///
/// Invisible at rest — the bar's left zone must go on reading as the track
/// name, not as a button — a quiet card under the pointer, and the raised card
/// with a hairline edge while the popover it opens is showing. That last state
/// is the anchor: with no notch available, "this control opened that layer" is
/// said by the control staying lit.
///
/// **The border width is 1 px in every state, including the invisible one.**
/// iced draws a border inside the widget's bounds, so a border that appeared on
/// hover would shrink the text under the pointer by a pixel — and this is the
/// bar, where nothing may move. Only colours vary here; the geometry is one
/// number in all four states, and `bottom_bar.rs` pins that.
///
/// No accent: opening a popover is a *view* choice, not a claim about what is
/// playing (the same argument [`panel_toggle`] makes).
#[must_use]
pub fn now_playing(p: &Palette, status: button::Status, open: bool) -> button::Style {
    let background = if open {
        p.plinth_lit
    } else {
        match status {
            button::Status::Hovered => p.plinth,
            button::Status::Pressed => p.recess,
            button::Status::Active | button::Status::Disabled => Color::TRANSPARENT,
        }
    };
    button::Style {
        background: Some(Background::Color(background)),
        text_color: p.paper,
        border: Border {
            color: if open {
                p.hairline_strong()
            } else {
                Color::TRANSPARENT
            },
            width: 1.0,
            radius: RADIUS_CTRL.into(),
        },
        shadow: Shadow::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pending_command_changes_the_glyph_ink_and_nothing_else() {
        // The pending affordance, pinned to the one property it is allowed
        // to touch: an opacity. There is no size, weight, or color token
        // that varies with it, so no pending transition can move a pixel.
        let live = glyph_opacity(true, false);
        let pending = glyph_opacity(true, true);
        assert!(pending < live, "pending must read as quieter, not louder");
        assert!(
            pending > glyph_opacity(false, false),
            "a control that is merely waiting must not look as dead as one that cannot act"
        );
        // A control that cannot act says so regardless of what is in flight.
        assert!((glyph_opacity(false, true) - glyph_opacity(false, false)).abs() < f32::EPSILON);
        for opacity in [live, pending, glyph_opacity(false, false)] {
            assert!((0.0..=1.0).contains(&opacity), "{opacity} is not an alpha");
        }
    }

    #[test]
    fn the_bottom_bar_reserves_the_seek_row_whether_or_not_it_has_one() {
        // The bar must not change height when a track starts or ends, so the
        // reserved strip has to be exactly what the real row occupies: the
        // preview lane above the groove's hit band.
        assert!((SEEK_ROW_H - (PREVIEW_H + RAIL_HIT)).abs() < f32::EPSILON);
        // The lane is part of the row's height, not decoration on top of it.
        const { assert!(SEEK_ROW_H > RAIL_HIT) }
        // And its width is the groove plus a fixed stamp on each side, so
        // the centre column never resizes as the digits tick.
        assert!((SEEK_ROW_W - (SEEK_W + 2.0 * (STAMP_W + GAP_SM))).abs() < f32::EPSILON);
        // A stamp must hold `h:mm:ss` without clipping. The face is
        // proportional everywhere except its figures, so what a const
        // assertion can bound is the *figures*: six of them, each [`DIGIT_EM`]
        // of the size, in both `0:00:00` and the ten-hour `10:00:00`. The whole
        // string, colons and all, is measured against the real advances by
        // `crate::font`'s slot test — which is where the ten-hour case is
        // actually proven, because in the *mono* it did not fit.
        const { assert!(STAMP_W > SIZE_META * 6.0 * DIGIT_EM) }
        // The signal-path slot is reserved on the same principle, and must
        // hold the longest chain a consumer device produces —
        // `192 → 176.4 kHz`, seven figures — so that a note appearing there
        // moves nothing beside it.
        const { assert!(SIGNAL_W > SIZE_META * 7.0 * DIGIT_EM) }
        // And the queue-position readout the left zone gained with the popover
        // is the same rule again: `999 / 999` is six figures, the slot holds
        // them, and it is that wide whether or not anything is playing — so
        // `3 / 12` appearing as a track starts moves no title.
        const { assert!(POSITION_W > SIZE_META * 6.0 * DIGIT_EM) }
        // …and the control that carries it holds the readout, its label and the
        // padding around both. The label itself is measured in the face that
        // draws it by `font.rs`; this is the arithmetic that leaves room.
        const { assert!(UP_NEXT_W > POSITION_W + 3.0 * GAP_SM) }
    }

    /// The popover is an overlay, and an overlay's whole promise is that it
    /// costs the surface underneath nothing. Both halves of that are geometry.
    #[test]
    fn the_popover_floats_rather_than_taking_the_shelfs_width() {
        /// What a row has left for its title once the number column, the
        /// reserved scrollbar lane, the removal target the rows gain in step 7
        /// and the gaps between them have taken their share.
        const ROW_TITLE_LANE: f32 =
            POPOVER_W - 2.0 * GAP_LG - TRACK_NO_W - SCROLLBAR_LANE - STEPPER_HIT - 3.0 * GAP_SM;

        // Narrower than a third of the shipped window: it covers the
        // bottom-right corner of the covers, not a column of them.
        const { assert!(POPOVER_W < 1280.0 / 3.0) }
        // …and wide enough for the rows it inherited from the rail.
        const { assert!(ROW_TITLE_LANE > 180.0) }
        // It never grows into a place: six tenths of the window leaves the
        // shelf legible above it, and the fraction is a fraction.
        const { assert!(POPOVER_MAX_H > 0.0 && POPOVER_MAX_H < 1.0) }
        // Its anchor inset is a rung of the spacing ladder, not a number.
        assert!((GAP_LG - 16.0).abs() < f32::EPSILON);
    }

    /// The Settings place's two columns fit the window they claim to, and the
    /// form is a readable measure rather than whatever is left over.
    ///
    /// The breakpoint is the load-bearing number: below it the section list and
    /// a full-width form cannot both have their width, so they stack. This is
    /// the arithmetic that says *where* that is true.
    #[test]
    fn the_settings_place_fits_both_of_its_arrangements() {
        // Above the breakpoint, the list, the gap between the columns and the
        // place's padding all come out before the form does — and what is left
        // at the breakpoint *itself* is already more than the cap. So in the
        // two-column arrangement the form is exactly `SETTINGS_CONTENT_W`, at
        // every window width it can be in, and the cap is the whole rule rather
        // than a limit that sometimes applies.
        const AT_BREAKPOINT: f32 = SETTINGS_BREAKPOINT - 2.0 * GAP_XL - SETTINGS_NAV_W - GAP_XL;
        const { assert!(AT_BREAKPOINT >= SETTINGS_CONTENT_W) }
        // The form is a readable measure: roughly 55 characters of body text at
        // half an em apiece, which is the top of the comfortable range and well
        // under the 60-em line the rail could never have produced anyway.
        const { assert!(SETTINGS_CONTENT_W / (SIZE_BODY * 0.5) < 100.0) }
        // Every control the section holds still fits it. These were fitted to a
        // 292 px column and are unchanged by the move, which is the claim
        // "verbatim" is making.
        const { assert!(SETTINGS_CONTENT_W > SETTING_VALUE_W + 2.0 * STEPPER_HIT + 3.0 * GAP_SM) }
        // The place's spine is narrower than its content, or it would read as a
        // second column of content rather than as navigation.
        const { assert!(SETTINGS_NAV_W < SETTINGS_CONTENT_W) }
    }

    /// The bar's now-playing affordance changes colour and **nothing else**.
    ///
    /// This is the pixel-stability claim in its smallest form: the left zone
    /// became a control, and a control that grew a border on hover would shift
    /// the track title by a pixel every time the pointer crossed it. The border
    /// is therefore present in all four states and merely transparent in three.
    #[test]
    fn the_now_playing_affordance_moves_nothing_when_it_lights_up() {
        let mut geometry: Vec<(f32, f32)> = Vec::new();
        for room in Room::ALL {
            let p = room.palette();
            for status in [
                button::Status::Active,
                button::Status::Hovered,
                button::Status::Pressed,
                button::Status::Disabled,
            ] {
                for open in [false, true] {
                    let style = now_playing(p, status, open);
                    geometry.push((style.border.width, style.border.radius.top_left));
                    assert_eq!(
                        style.shadow,
                        Shadow::default(),
                        "the bar casts no shadow; only artwork and the popover do"
                    );
                }
            }
            // And "open" is visibly different from "hovered", or the anchor the
            // popover has instead of a notch says nothing. True in every room:
            // a room whose two raised planes collapsed would lose the only
            // thing standing in for a notch.
            let open = now_playing(p, button::Status::Active, true);
            let hovered = now_playing(p, button::Status::Hovered, false);
            assert_ne!(
                from_background(open.background),
                from_background(hovered.background),
                "{}: open and hovered are the same surface",
                p.name
            );
        }
        assert!(
            geometry
                .windows(2)
                .all(|pair| (pair[0].0 - pair[1].0).abs() < f32::EPSILON
                    && (pair[0].1 - pair[1].1).abs() < f32::EPSILON),
            "the affordance's border geometry varies with state: {geometry:?}"
        );
    }

    /// The advance width of one **figure** in the bundled face, as a fraction
    /// of the type size.
    ///
    /// **0.6, and it is now a property of the Sans rather than of a second
    /// face.** IBM Plex Sans ships tabular figures by default — every digit
    /// advances 600/1000 em in Regular, Medium and `SemiBold` alike, which is
    /// exactly what Plex Mono advanced at, and is why the monospace could be
    /// deleted without re-deriving a single slot
    /// (`.interface-design/system.md` §8).
    ///
    /// The const assertions below stay because they are cheap and they fail at
    /// compile time, but they bound only the digits in a worst-case string: the
    /// face is proportional everywhere else, so `n glyphs × DIGIT_EM` is no
    /// longer arithmetic about a whole string. That claim is *measured* —
    /// against these very bytes, string by string — in `crate::font`'s
    /// `every_reserved_slot_holds_its_worst_case_in_the_bundled_face`, which is
    /// the test `docs/design/02-visual-language.md` §3.4 requires before a face
    /// change may ship.
    const DIGIT_EM: f32 = 0.6;

    /// The duration-column defect, as arithmetic: the lane a list keeps clear
    /// is exactly the lane its scrollbar occupies, and it is kept clear on the
    /// right and nowhere else.
    ///
    /// This is the whole of the fix — the bar overlays the content, so the
    /// content stops using the width the bar overlays — and the two numbers
    /// being one token is what stops them drifting apart the next time either
    /// is touched.
    #[test]
    fn a_list_reserves_exactly_the_lane_its_scrollbar_occupies() {
        let gutter = scroll_gutter();
        assert!(
            (gutter.right - SCROLLBAR_LANE).abs() < f32::EPSILON,
            "the reserved lane ({}) is not the scrollbar's lane ({SCROLLBAR_LANE})",
            gutter.right
        );
        // Nothing else moves: this must not become a general list inset.
        assert!((gutter.left).abs() < f32::EPSILON);
        assert!((gutter.top).abs() < f32::EPSILON);
        assert!((gutter.bottom).abs() < f32::EPSILON);
        // The lane has to be wide enough to hide a bar, or it is decoration.
        const { assert!(SCROLLBAR_LANE >= SCROLLBAR_W) }
        // And the bar the list actually installs is built from the same
        // token, so "the lane is the bar's width" is true by construction
        // rather than by two literals happening to match.
        assert_eq!(
            list_scrollbar(),
            scrollable::Scrollbar::new()
                .width(SCROLLBAR_LANE - 2.0 * SCROLLBAR_MARGIN)
                .scroller_width(SCROLLBAR_W)
                .margin(SCROLLBAR_MARGIN)
        );
    }

    /// A track row still has room for its title after the lane is taken, and
    /// the value slot beside a setting still holds the widest figure it can
    /// be asked to show.
    #[test]
    fn the_panel_still_fits_what_it_has_to_draw() {
        // Panel width, less its inset on both sides, less the number column,
        // the gaps, and the new lane — what is left is the title's.
        let inner = PANEL_W - 2.0 * GAP_XL - SCROLLBAR_LANE - TRACK_NO_W - 2.0 * GAP_SM;
        assert!(
            inner > 200.0,
            "the lane left only {inner} px for a track title"
        );
        // `-20.00 dB` is five figures' worth of sign and digits at SIZE_META
        // (U+2212 advances the same 0.6 em a digit does); the slot is fixed so
        // a value changing cannot move the stepper beside it.
        const { assert!(SETTING_VALUE_W > SIZE_META * 5.0 * DIGIT_EM) }
        // A stepper is smaller than the transport but still a real target.
        const { assert!(STEPPER_HIT < TRANSPORT_HIT && STEPPER_HIT >= ICON_PX) }
    }

    /// Every sentence the settings panel can put in its reserved note slot
    /// fits it — otherwise the slot clips the words instead of the layout
    /// moving, which is the worse of the two failures it was chosen over.
    ///
    /// This is the arithmetic bound: at [`SIZE_META`] the bundled Sans
    /// averages 0.42–0.46 em per character over these sentences, so half an em
    /// is a conservative budget. The same claim is made *properly* — with the
    /// face's own advance widths and a greedy word wrap — by `crate::font`'s
    /// `a_setting_note_still_wraps_inside_its_two_reserved_lines`; this one
    /// stays because it is the version that needs no asset.
    #[test]
    fn a_setting_note_fits_the_slot_it_is_given() {
        use crate::replaygain::{MODES, mode_note};

        // The slot is exactly two lines — not "about two".
        assert!((SETTING_NOTE_H - 2.0 * SIZE_META * LEADING_META).abs() < f32::EPSILON);
        // The width a wrapped line actually has: the panel, less its inset on
        // both sides, less the scrollbar lane.
        let content_w = PANEL_W - 2.0 * GAP_XL - SCROLLBAR_LANE;
        let per_line = content_w / (SIZE_META * 0.5);
        let budget = 2.0 * per_line;
        for mode in MODES {
            let note = mode_note(mode);
            #[expect(
                clippy::cast_precision_loss,
                reason = "a sentence's length is far below f32's exact-integer range"
            )]
            let length = note.chars().count() as f32;
            assert!(
                length <= budget,
                "{note:?} is {length} characters, past the {budget}-character \
                 two-line budget the reserved slot can hold"
            );
        }
    }

    /// **The shelf virtualizes at every width the inspector can produce.**
    ///
    /// One of the four properties `docs/design/01-ux-audit-and-ia.md` §5 says
    /// must not regress, and since ADR-0017 step 5 it is checked over the
    /// **whole band at 1 px resolution** rather than at a 20 px stride: with a
    /// fluid cell the column count and every sleeve's size change together,
    /// the transitions are single-pixel events, and a coarse sweep can step
    /// straight over one. Every window width from the smallest iced will hand
    /// us to a wall-sized one, with the inspector open and closed, must
    /// produce a real grid, a real hang, and a covered, clamped visible range.
    ///
    /// The popover is deliberately absent from this sweep — that is the
    /// *point* of it being an overlay: it produces no width at all.
    #[test]
    fn the_shelf_virtualizes_at_every_width_the_inspector_can_produce() {
        use crate::shelf::Grid;

        const WINDOW_W: f32 = 1280.0;
        assert_eq!(Grid::new(WINDOW_W).columns, 4, "the shipped shelf");
        assert_eq!(
            Grid::new(WINDOW_W - PANEL_W).columns,
            3,
            "the inspector open: 940 px hangs three works of 254"
        );

        // The band: every window width baz can be dragged to, at 1 px, both
        // inspector states, both a full library and a single search result.
        for window in 640..=2560 {
            #[expect(
                clippy::cast_precision_loss,
                reason = "a window width in pixels is far below f32's exact-integer range"
            )]
            let window = window as f32;
            for inspector in [0.0, PANEL_W] {
                let hang = Grid::new(window - inspector);
                assert!(
                    hang.columns >= 1,
                    "the grid collapsed at {window} px with {inspector} px of inspector"
                );
                assert!(
                    hang.art > 0.0 && hang.art <= ART_MAX,
                    "{window} px with {inspector} px of inspector: {} px of art",
                    hang.art
                );
                assert!(
                    hang.row_h > 0.0,
                    "{window} px: a non-positive row pitch virtualizes nothing"
                );
                for albums in [1_usize, 97, 10_000] {
                    let rows = hang.rows(albums);
                    assert_eq!(rows, albums.div_ceil(hang.columns));
                    let (first, end) = hang.visible_rows(0.0, 800.0, rows);
                    assert!(
                        first < end && end <= rows,
                        "empty or overrunning viewport at {window} px, {albums} albums"
                    );
                    // A fling to the far end of a 10 000-album wall still
                    // lands on a clamped range — the pitch is a float now, so
                    // this is arithmetic worth checking rather than obvious.
                    let (first, end) = hang.visible_rows(hang.spacer_height(rows), 800.0, rows);
                    assert!(first <= end && end <= rows);
                }
            }
        }

        // And the panel has to hold its own contents: the album panel insets
        // the artwork by its padding on both sides and must not go negative.
        const { assert!(PANEL_W > 2.0 * GAP_XL) }
    }

    #[test]
    fn the_volume_block_reserves_every_state_it_can_be_in() {
        // The fader's hit band has to hold the knob *and* the detent mark
        // above it on both sides, or the mark the unity detent is made of
        // would be drawn outside the widget's own bounds.
        const { assert!(VOLUME_HIT >= RAIL + 2.0 * (KNOB + DETENT_GAP + DETENT_H)) }
        // The mark clears the knob rather than hiding under it — the whole
        // reason it is lifted at all.
        const { assert!(DETENT_GAP > 0.0 && DETENT_H > 0.0) }
        // The block is the mute target plus a gap plus the groove, and its
        // height is the level lane over the fader. Both fixed, in every
        // state, so no volume change and no mute can move a pixel beside it.
        assert!((VOLUME_BLOCK_W - (TRANSPORT_HIT + GAP_SM + VOLUME_W)).abs() < f32::EPSILON);
        assert!((VOLUME_ROW_H - (PREVIEW_H + VOLUME_HIT)).abs() < f32::EPSILON);
        // The level tip must hold `-18.1 dB` — four figures at caption size,
        // plus the proportional remainder `crate::font` measures — without
        // clipping.
        const { assert!(LEVEL_W > SIZE_CAPTION * 4.0 * DIGIT_EM) }
        // And the whole right-hand end has to fit beside the centre column
        // in the shipped window, or the zone would clip on launch.
        const { assert!(VOLUME_BLOCK_W + GAP_SM + SIGNAL_W < 1280.0 - SEEK_ROW_W) }
    }

    #[test]
    fn the_volume_fader_changes_only_its_ink() {
        // Every state of this control has to draw the same geometry: the
        // detent's position is derived from the handle's width, so a knob
        // that grew under the pointer would drag the detent with it, and a
        // detent that moves is not a detent.
        let radius = |style: slider::Style| match style.handle.shape {
            HandleShape::Circle { radius } => radius,
            HandleShape::Rectangle { width, .. } => f32::from(width),
        };
        let mut widths = Vec::new();
        for room in Room::ALL {
            let p = room.palette();
            for status in [
                slider::Status::Active,
                slider::Status::Hovered,
                slider::Status::Dragged,
            ] {
                for style in [volume, volume_muted, volume_inert] {
                    let drawn = style(p, status);
                    widths.push(radius(drawn));
                    assert!(
                        (drawn.rail.width - RAIL).abs() < f32::EPSILON,
                        "the rail thickness must not vary with state"
                    );
                }
            }
            // Muted is quieter than live and still readable against the groove
            // it sits in — the fader keeps showing the position mute will
            // restore. "Quieter" is a contrast claim rather than a channel
            // one now that a room can be light: in Reading Room the muted ink
            // is the *lighter* of the two.
            assert!(
                contrast(p.paper_muted, p.recess) < contrast(p.paper_faint, p.recess),
                "{}: the muted fader is not quieter than the live one",
                p.name
            );
            assert!(
                contrast(p.paper_muted, p.recess) >= 3.0,
                "{}: the muted fader's position is not findable in its trough",
                p.name
            );
        }
        assert!(
            widths
                .windows(2)
                .all(|pair| (pair[0] - pair[1]).abs() < f32::EPSILON),
            "the volume knob must not change size: {widths:?}"
        );
    }

    #[test]
    fn the_unity_detent_is_visible_without_being_loud() {
        // Engaged has to be plainly different from at-rest — that contrast
        // is what makes "at unity" and "a pixel below" different on sight —
        // and neither may reach for the accent, which means playback truth.
        for room in Room::ALL {
            let p = room.palette();
            let rest = detent_ink(p, false);
            let engaged = detent_ink(p, true);
            assert!(engaged.a > rest.a || engaged.r > rest.r * 3.0);
            for ink in [rest, engaged] {
                assert!(!p.is_accent(ink), "{}: the detent is the accent", p.name);
                assert!(
                    (ink.r - p.lamp.r).abs() > 0.1 || (ink.b - p.lamp.b).abs() > 0.1,
                    "{}: the detent must not be the room's lamp",
                    p.name
                );
            }
        }
    }

    #[test]
    fn a_transport_button_is_a_square_target_around_its_glyph() {
        // The hit area is larger than the mark it carries…
        const { assert!(TRANSPORT_HIT > ICON_PX) }
        // …and the pair of them fits inside the column they centre in.
        const { assert!(2.0 * TRANSPORT_HIT + GAP_SM < SEEK_ROW_W) }
    }

    // -----------------------------------------------------------------------
    // Contrast
    // -----------------------------------------------------------------------

    /// One channel of an sRGB colour, linearised — the first half of WCAG
    /// 2.1's relative-luminance definition.
    ///
    /// iced's `Color` components are already sRGB-encoded (the same assumption
    /// [`crate::icon`] makes when it writes them straight into an
    /// `Rgba8UnormSrgb` sprite), so they go into this transfer function as
    /// they are.
    fn linear(channel: f32) -> f32 {
        if channel <= 0.040_45 {
            channel / 12.92
        } else {
            ((channel + 0.055) / 1.055).powf(2.4)
        }
    }

    /// WCAG 2.1 relative luminance.
    fn luminance(color: Color) -> f32 {
        0.2126 * linear(color.r) + 0.7152 * linear(color.g) + 0.0722 * linear(color.b)
    }

    /// The WCAG 2.1 contrast ratio between two opaque colours.
    fn contrast(foreground: Color, background: Color) -> f32 {
        let (a, b) = (luminance(foreground), luminance(background));
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    /// **An opacity is a colour once it is drawn.** `over` composited under
    /// `under`, source-over, in the space the renderer blends in.
    ///
    /// This is the whole of ADR-0017 §1.6's second extension. A token
    /// expressed as an alpha is not measurable against a floor until it has
    /// been resolved against the surface it lands on, and a test that sees
    /// only opaque tokens cannot see an unreadable one hiding in an opacity —
    /// which is exactly the failure the critique's "ink opacity is the
    /// hierarchy" would have shipped (its 40 % tier lands between 2.09 : 1 and
    /// 3.24 : 1 across the four rooms).
    fn composite(over: Color, under: Color) -> Color {
        let a = over.a.clamp(0.0, 1.0);
        Color {
            r: over.r * a + under.r * (1.0 - a),
            g: over.g * a + under.g * (1.0 - a),
            b: over.b * a + under.b * (1.0 - a),
            a: 1.0,
        }
    }

    /// The **oklch L** of an sRGB colour — the lightness axis of oklab, which
    /// is a perceptual space and is therefore the right instrument for
    /// surface-against-surface where WCAG is the wrong one.
    ///
    /// Twenty-five lines and no dependency (ADR-0017 §1.6). The matrices are
    /// Björn Ottosson's published oklab constants.
    fn oklch_l(color: Color) -> f32 {
        let red = linear(color.r);
        let green = linear(color.g);
        let blue = linear(color.b);
        // The LMS cone responses, then their cube roots — oklab's whole trick.
        let long = 0.412_221_5 * red + 0.536_332_54 * green + 0.051_445_995 * blue;
        let medium = 0.211_903_5 * red + 0.680_699_5 * green + 0.107_396_96 * blue;
        let short = 0.088_302_46 * red + 0.281_718_85 * green + 0.629_978_7 * blue;
        0.210_454_26 * long.cbrt() + 0.793_617_8 * medium.cbrt() - 0.004_072_047 * short.cbrt()
    }

    /// **The contrast test.** Both laws, over both rooms, with every opacity
    /// composited before it is measured.
    ///
    /// ADR-0017 §1.6 resolves the critique's *"WCAG ratios are meaningless at
    /// these lightnesses; do not use them here"* against the test baz shipped,
    /// and the resolution is that **they are not in conflict, because they
    /// measure different things**:
    ///
    /// 1. **Surface against surface** is an *elevation* question, and at these
    ///    lightnesses a ratio says nothing — Closing Time's wall on its plinth
    ///    is 1.30 : 1 and that number carries no information. oklch L is the
    ///    instrument, and the law is the critique's: **adjacent levels differ
    ///    by ≥ 0.03 L**, and no room's surfaces sit in the **dead zone**
    ///    L .45–.58. (The dead zone is a rule about *rooms*; an ink or an
    ///    accent may live there, and both of Reading Room's do.)
    /// 2. **Ink against surface** is a *legibility* question, and there the
    ///    ratio is the entire point: `paper_faint` shipped at 3.4 : 1 through
    ///    v0.1 carrying every duration, count and hint in the product, and
    ///    `paper_muted` at 1.9 : 1 made the muted fader's position invisible.
    ///    Deleting WCAG deletes the rule that catches that.
    ///
    /// Three things this test gained at step 2, each named in §1.6:
    ///
    /// - **The elevation law**, asserted rather than admired. Closing Time's
    ///   four shipped surfaces satisfy it on all three steps without having
    ///   been designed to; the critique's own Closing Time fails its own floor
    ///   at +0.0248.
    /// - **Composited measurement** ([`composite`]), so an alpha-expressed
    ///   token is resolved against each surface it can land on before its
    ///   ratio is taken.
    /// - **A named exemption list** instead of a global waiver. The bounded
    ///   concession: a *non-text mark that exists only to be locatable and is
    ///   never read* — the hairline edges, and the needle's unfilled track and
    ///   the index rail's absent letters when they arrive — is governed by the
    ///   L-step law instead. Anything a user reads keeps its floor. An
    ///   exemption list you must add a name to is a rule; "WCAG is meaningless
    ///   here" is not.
    #[test]
    fn every_ink_and_every_surface_clears_its_floor() {
        /// The AA floor for text.
        const TEXT: f32 = 4.5;
        /// The floor for a non-text mark.
        const MARK: f32 = 3.0;
        /// The smallest elevation step the eye reads as a step.
        const STEP_L: f32 = 0.03;
        /// The bottom of the lightness band no room's surface may sit in.
        const DEAD_LO: f32 = 0.45;
        /// The top of it.
        const DEAD_HI: f32 = 0.58;
        /// **The exemption list.** Marks that exist only to be locatable and
        /// are never read, exempt from the WCAG mark floor by name and
        /// governed by the L-step law instead (§1.6). The needle's unfilled
        /// track and the index rail's absent letters join this list when steps
        /// 8 and 9 build them.
        const NEVER_READ: [&str; 3] = ["hairline", "hairline_strong", "lamp_glow"];

        for room in Room::ALL {
            let p = room.palette();
            let surfaces = p.surfaces();

            // --- law 1: elevation, in oklch L ------------------------------
            for pair in surfaces.windows(2) {
                let [(below, lower), (above, upper)] = [pair[0], pair[1]];
                let step = (oklch_l(upper) - oklch_l(lower)).abs();
                assert!(
                    step >= STEP_L,
                    "{}: {below} → {above} steps {step:.4} oklch L, below the \
                     {STEP_L} floor — two planes that close are one plane",
                    p.name
                );
            }
            for (name, surface) in surfaces {
                let l = oklch_l(surface);
                assert!(
                    !(DEAD_LO..=DEAD_HI).contains(&l),
                    "{}: {name} sits at L {l:.4}, inside the dead zone \
                     {DEAD_LO}–{DEAD_HI} — a room at that lightness is neither \
                     lit nor unlit and every ink on it is a compromise",
                    p.name
                );
            }

            // --- law 2: legibility, in WCAG 2.1 ----------------------------
            // Every ink the room paints, with the floor its *use* implies.
            // `paper_muted` is the muted fader and a stepper at the end of its
            // travel — a mark, not a sentence — so it takes the lower floor;
            // the lamp is a fill and a dot, likewise.
            let inks = [
                ("paper", p.paper, TEXT),
                ("paper_dim", p.paper_dim, TEXT),
                ("paper_faint", p.paper_faint, TEXT),
                ("alert", p.alert, TEXT),
                ("paper_muted", p.paper_muted, MARK),
                ("lamp", p.lamp, MARK),
                // The alpha-expressed marks, composited before measuring.
                ("paper_ring", p.paper_ring(), MARK),
                ("hairline", p.hairline(), MARK),
                ("hairline_strong", p.hairline_strong(), MARK),
                ("lamp_glow", p.lamp_glow(), MARK),
            ];
            for (ink_name, ink, floor) in inks {
                if NEVER_READ.contains(&ink_name) {
                    continue;
                }
                for (surface_name, surface) in surfaces {
                    // An opacity is a colour once it is drawn; an opaque ink
                    // composites to itself, so this one line covers both.
                    let drawn = composite(ink, surface);
                    let ratio = contrast(drawn, surface);
                    assert!(
                        ratio >= floor,
                        "{}: {ink_name} on {surface_name} is {ratio:.2} : 1, \
                         below its {floor} : 1 floor",
                        p.name
                    );
                }
            }

            // The selection wash is exempt as a *mark* and measured as a
            // *ground*: what a user reads is the value's ink on the wash, and
            // the wash lands over the input well.
            let selected = composite(p.select_wash(), p.recess);
            let on_selection = contrast(p.paper, selected);
            assert!(
                on_selection >= TEXT,
                "{}: selected text is {on_selection:.2} : 1 on its own wash",
                p.name
            );

            // The one ink that sits on the accent rather than on a surface:
            // the Play button's label and triangle.
            let on_lamp = contrast(p.lamp_ink, p.lamp);
            assert!(
                on_lamp >= TEXT,
                "{}: lamp_ink on lamp is {on_lamp:.2} : 1, below {TEXT} : 1",
                p.name
            );

            // The ordering the room is built on: each ink is quieter than the
            // one above it. A contrast comparison rather than a channel one,
            // because in a light room "quieter" is *lighter*.
            let ramp = [p.paper, p.paper_dim, p.paper_faint, p.paper_muted];
            for pair in ramp.windows(2) {
                assert!(
                    contrast(pair[0], p.plinth) > contrast(pair[1], p.plinth),
                    "{}: the ink ramp is not monotone",
                    p.name
                );
            }
        }

        // And the two corrections, pinned as corrections: the values v0.1
        // shipped fail the floors above, so this test would have caught them.
        let dark = &CLOSING_TIME;
        let old_faint = Color::from_rgb(0.447, 0.427, 0.400);
        let old_muted = Color::from_rgb(0.290, 0.278, 0.263);
        assert!(
            contrast(old_faint, dark.plinth) < TEXT,
            "the old paper_faint is supposed to be the failure this test exists for"
        );
        assert!(contrast(old_muted, dark.plinth) < MARK);
        assert!(
            contrast(dark.paper_faint, dark.plinth) > contrast(old_faint, dark.plinth),
            "the correction must be lighter, not merely different"
        );
        assert!(contrast(dark.paper_muted, dark.plinth) > contrast(old_muted, dark.plinth));

        // The critique's own Closing Time fails its own elevation law, which
        // is why baz kept its four shipped surfaces and adopted only the rule.
        let theirs = (
            Color::from_rgb(0.027, 0.031, 0.035),
            Color::from_rgb(0.047, 0.051, 0.055),
        );
        assert!(
            (oklch_l(theirs.1) - oklch_l(theirs.0)).abs() < STEP_L,
            "the critique's #070809 → #0C0D0E is supposed to be the counter-example"
        );
    }

    // -----------------------------------------------------------------------
    // The accent discipline
    // -----------------------------------------------------------------------

    /// The colours in a `Background`, if it is a flat one.
    fn from_background(background: Option<Background>) -> Vec<Color> {
        match background {
            Some(Background::Color(color)) => vec![color],
            _ => Vec::new(),
        }
    }

    /// Every colour a `container` style paints.
    fn container_colors(style: &container::Style) -> Vec<Color> {
        let mut colors = from_background(style.background);
        colors.extend(style.text_color);
        colors.push(style.border.color);
        colors.push(style.shadow.color);
        colors
    }

    /// Every colour a `button` style paints.
    fn button_colors(style: &button::Style) -> Vec<Color> {
        let mut colors = from_background(style.background);
        colors.push(style.text_color);
        colors.push(style.border.color);
        colors.push(style.shadow.color);
        colors
    }

    /// Every colour a `slider` style paints.
    fn slider_colors(style: &slider::Style) -> Vec<Color> {
        let mut colors = from_background(Some(style.rail.backgrounds.0));
        colors.extend(from_background(Some(style.rail.backgrounds.1)));
        colors.push(style.rail.border.color);
        colors.extend(from_background(Some(style.handle.background)));
        colors.push(style.handle.border_color);
        colors
    }

    /// Every style this module exposes, in every state it has, paired with the
    /// colours it paints.
    ///
    /// Split out of the test below so the sweep can be read as a list of what
    /// the room is made of, rather than as a hundred lines of setup. Anything
    /// missing from here is invisible to the accent discipline — the length
    /// assertion in the test is the crude guard against that.
    fn every_painted_style(p: &Palette) -> Vec<(&'static str, Vec<Color>)> {
        let button_states = [
            button::Status::Active,
            button::Status::Hovered,
            button::Status::Pressed,
            button::Status::Disabled,
        ];
        let slider_states = [
            slider::Status::Active,
            slider::Status::Hovered,
            slider::Status::Dragged,
        ];
        let mut painted: Vec<(&'static str, Vec<Color>)> = Vec::new();
        for status in button_states {
            for selected in [false, true] {
                painted.push(("tile", button_colors(&tile(p, status, selected))));
                painted.push(("segment", button_colors(&segment(p, status, selected))));
                painted.push((
                    "panel_toggle",
                    button_colors(&panel_toggle(p, status, selected)),
                ));
            }
            painted.push(("transport", button_colors(&transport(p, status))));
            painted.push(("primary", button_colors(&primary(p, status))));
            for open in [false, true] {
                painted.push(("now_playing", button_colors(&now_playing(p, status, open))));
            }
        }
        for status in slider_states {
            painted.push(("seek", slider_colors(&seek(p, status))));
            painted.push(("seek_inert", slider_colors(&seek_inert(p, status))));
            painted.push(("volume", slider_colors(&volume(p, status))));
            painted.push(("volume_muted", slider_colors(&volume_muted(p, status))));
            painted.push(("volume_inert", slider_colors(&volume_inert(p, status))));
        }
        for status in [
            text_input::Status::Active,
            text_input::Status::Hovered,
            text_input::Status::Focused,
            text_input::Status::Disabled,
        ] {
            let style = input(p, status);
            painted.push((
                "input",
                vec![
                    style.border.color,
                    style.icon,
                    style.placeholder,
                    style.value,
                    style.selection,
                ],
            ));
        }
        for status in [
            checkbox::Status::Active { is_checked: false },
            checkbox::Status::Active { is_checked: true },
            checkbox::Status::Hovered { is_checked: true },
            checkbox::Status::Disabled { is_checked: true },
        ] {
            let style = check(p, status);
            let mut colors = from_background(Some(style.background));
            colors.push(style.icon_color);
            colors.push(style.border.color);
            colors.extend(style.text_color);
            painted.push(("check", colors));
        }
        for status in [
            scrollable::Status::Active,
            scrollable::Status::Hovered {
                is_horizontal_scrollbar_hovered: false,
                is_vertical_scrollbar_hovered: true,
            },
            scrollable::Status::Dragged {
                is_horizontal_scrollbar_dragged: false,
                is_vertical_scrollbar_dragged: true,
            },
        ] {
            let style = scrollbar(p, status);
            painted.push((
                "scrollbar",
                vec![
                    style.vertical_rail.scroller.color,
                    style.vertical_rail.border.color,
                ],
            ));
        }
        painted.push(("sleeve(resting)", container_colors(&sleeve(p, false))));
        painted.push(("sleeve(playing)", container_colors(&sleeve(p, true))));
        painted.push(("lamp_dot", container_colors(&lamp_dot(p))));
        painted.push(("segmented", container_colors(&segmented(p))));
        painted.push(("preview_tip", container_colors(&preview_tip(p))));
        painted.push(("panel", container_colors(&panel(p))));
        painted.push(("bar", container_colors(&bar(p))));
        painted.push(("popover", container_colors(&popover(p))));
        painted.push(("tooltip", container_colors(&tooltip(p))));
        painted.push(("hairline", vec![hairline(p).color]));
        painted.push((
            "detent_ink",
            vec![detent_ink(p, false), detent_ink(p, true)],
        ));
        painted
    }

    /// **The accent-discipline test.** The lamp is spent on playback truth and
    /// on nothing else, checked by painting every style this module exposes in
    /// every state it has and looking at what came out.
    ///
    /// The four styles on the permitted list are the four in
    /// `docs/design/02-visual-language.md` §2.1.1 that this module owns: the
    /// playing sleeve's halo, the playing dot, the seek groove, and the
    /// primary Play action. (The fifth permitted use — the elapsed timestamp
    /// warming while a seek is in flight — is a view-level colour rather than
    /// a style function, and is pinned by
    /// `the_lamp_is_named_only_where_playback_truth_is_drawn` below.)
    ///
    /// Everything else in the room — focus, selection, panel toggles, the
    /// edition and ReplayGain selectors, the volume fader, the unity detent,
    /// tooltips, previews, scrollbars, checkboxes, steppers, tile and row
    /// selection — is made of surface, edge and ink. This test is what makes
    /// that a rule rather than a habit: adding an amber to any style below
    /// fails it by name.
    ///
    /// **Swept per room**, since step 2: the discipline is about the *accent*,
    /// not about amber, so Reading Room's oxblood is held to the same list by
    /// the same code and a style that reached for one room's accent could not
    /// pass by being the other's.
    #[test]
    fn the_lamp_is_spent_only_on_playback_truth() {
        /// The styles §2.1.1 permits the accent in. Nothing may be added here
        /// without the specification changing first.
        const PERMITTED: [&str; 4] = ["sleeve(playing)", "lamp_dot", "seek", "primary"];

        for room in Room::ALL {
            let p = room.palette();
            let painted = every_painted_style(p);
            let mut seen_accent: Vec<&str> = Vec::new();
            for (name, colors) in &painted {
                let accent = colors.iter().copied().any(|color| p.is_accent(color));
                assert!(
                    !accent || PERMITTED.contains(name),
                    "{}: `{name}` paints the accent. The lamp means playback \
                     truth (theme.rs's module docs, ADR-0017 §1.6, \
                     docs/design/02-visual-language.md §2.1.1); this surface is \
                     not playback truth, so it wants a surface step, a \
                     hairline, or an ink instead.",
                    p.name
                );
                if accent {
                    seen_accent.push(name);
                }
            }
            // The rule cuts both ways: a permitted use that stopped being the
            // accent would mean the one signal reserved for the music had
            // quietly gone out, so each is asserted present rather than merely
            // allowed.
            for permitted in PERMITTED {
                assert!(
                    seen_accent.contains(&permitted),
                    "{}: `{permitted}` is supposed to be the accent and no \
                     longer paints it",
                    p.name
                );
            }
            // The room is large: if the sweep ever stopped covering it, the
            // test would pass vacuously.
            assert!(
                painted.len() > 40,
                "only {} styles swept — did a style stop being covered?",
                painted.len()
            );
        }
    }

    /// A room is a *whole* palette, and two rooms may not share a value that
    /// carries meaning.
    ///
    /// The cheap failure this catches is a room defined by copying another and
    /// editing some of it: a light room that kept the dark room's ink, or an
    /// accent that was never re-chosen, would pass every contrast assertion
    /// above by accident of the surfaces around it.
    #[test]
    fn the_two_rooms_are_two_rooms() {
        let dark = &CLOSING_TIME;
        let light = &READING_ROOM;
        // Every plane and every ink is its own decision.
        for (a, b) in dark
            .surfaces()
            .iter()
            .zip(light.surfaces().iter())
            .map(|(a, b)| (a.1, b.1))
            .chain([
                (dark.paper, light.paper),
                (dark.paper_dim, light.paper_dim),
                (dark.paper_faint, light.paper_faint),
                (dark.paper_muted, light.paper_muted),
                (dark.lamp, light.lamp),
                (dark.lamp_ink, light.lamp_ink),
                (dark.alert, light.alert),
            ])
        {
            assert_ne!(a, b, "the two rooms share a value");
        }
        // The elevation strategy inverts rather than repeating: surfaces rise
        // toward the lamp, so a plinth is lighter than its wall in a dark room
        // and darker than its wall in a light one, and the recess inverts with
        // them (ADR-0017 §1.5).
        assert!(oklch_l(dark.plinth) > oklch_l(dark.wall));
        assert!(oklch_l(dark.recess) < oklch_l(dark.wall));
        assert!(oklch_l(light.plinth) < oklch_l(light.wall));
        assert!(oklch_l(light.recess) > oklch_l(light.wall));
        // A dark room's ink is lighter than its wall and a light room's is
        // darker — which is the whole of why the ramp's *ordering* is asserted
        // in contrast rather than in channels.
        assert!(oklch_l(dark.paper) > oklch_l(dark.wall));
        assert!(oklch_l(light.paper) < oklch_l(light.wall));
        // The theme cache is indexed by the room's discriminant, so the two
        // have to agree about which is which.
        assert_eq!(Room::ClosingTime as usize, 0);
        assert_eq!(Room::ReadingRoom as usize, 1);
        for room in Room::ALL {
            assert_eq!(room.palette().room, room);
        }
    }

    /// **Following the OS, and the gate that stops it.**
    ///
    /// [`follow`] is pure, so the whole of "the rooms follow the desktop" is
    /// testable without a desktop — and what it currently answers is
    /// [`CLOSING_TIME`] either way, because §1.5 ships the light room only
    /// with an answer to the pale-sleeve question. That is asserted rather
    /// than left implicit: this test is the thing that will fail, loudly and
    /// by name, on the day somebody flips the gate without meaning to.
    #[test]
    fn the_rooms_follow_the_desktop_once_the_second_one_ships() {
        assert_eq!(follow(Appearance::Dark), &CLOSING_TIME);
        if READING_ROOM_SHIPS {
            assert_eq!(follow(Appearance::Light), &READING_ROOM);
        } else {
            assert_eq!(
                follow(Appearance::Light),
                &CLOSING_TIME,
                "the light room is defined, not selectable (ADR-0017 §1.5 and \
                 build-plan step 20): it ships with an answer to the \
                 pale-sleeve-on-paper question, and that answer may not be a \
                 border on artwork"
            );
        }
        // A room nothing installed is the room baz is, so every other test in
        // the crate is deterministic without a desktop.
        assert_eq!(active(), &CLOSING_TIME);
        // And an unknown desktop is never read as a light one: `dark-light`
        // reports "no preference" and "light" identically once iced has mapped
        // them, so only a positive light answer may leave Closing Time.
        assert_eq!(follow(Appearance::Dark), &CLOSING_TIME);
    }

    /// The other half of the discipline: the accent is not named outside this
    /// module except where §2.1.1 permits it.
    ///
    /// The style sweep above cannot see a view that writes the accent straight
    /// onto a `text`, which is exactly how the scanning note and the first-run
    /// wordmark came to be amber with nothing playing. So this reads the
    /// crate's own sources and checks who names an accent token.
    ///
    /// **What it greps for changed at step 2** along with the tokens: a view
    /// used to write `theme::LAMP` and now writes `room.lamp` — or
    /// `.lamp_bright`, `.lamp_deep`, `.lamp_glow()` — so the needle is the
    /// field access, which is a *narrower* net than the old one rather than a
    /// looser one (a style function is `theme::lamp_dot`, with no dot before
    /// the name, and is not matched).
    ///
    /// The single entry on the list is §2.1.1's fourth permitted use: the
    /// elapsed timestamp warms to [`Palette::lamp`] while a position has been
    /// asked for and not yet confirmed, because a position being asked for is
    /// a claim about the playhead. It cools the moment the engine answers.
    #[test]
    fn the_lamp_is_named_only_where_playback_truth_is_drawn() {
        /// `src`-relative paths that may name an accent token, and why.
        const PERMITTED: [&str; 1] = ["views/bottom_bar.rs"];

        // Spelled in halves so this test's own source does not match it.
        let needle = concat!(".", "lamp");
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut offenders: Vec<String> = Vec::new();
        let mut permitted_seen = false;
        for path in rust_sources(&root) {
            let relative = path
                .strip_prefix(&root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            // This module *defines* the tokens; the font module is asset
            // bytes; the groove is a widget that is handed a style function
            // rather than a colour. None is a view.
            if relative == "theme.rs" || relative == "font.rs" {
                continue;
            }
            let source = std::fs::read_to_string(&path).expect("a source file baz ships");
            if !source.contains(needle) {
                continue;
            }
            if PERMITTED.contains(&relative.as_str()) {
                permitted_seen = true;
            } else {
                offenders.push(relative);
            }
        }
        assert!(
            offenders.is_empty(),
            "{offenders:?} name the accent. The lamp means playback truth — \
             which album is sounding, which track, and where the playhead is \
             (ADR-0017 §1.6, docs/design/02-visual-language.md §2.1.1). A scan, \
             a focus ring, a selection, a wordmark and a setting are none of \
             those; they want the room's dim ink, its focus ring, its selection \
             wash, or a surface step."
        );
        assert!(
            permitted_seen,
            "no view names the accent at all — the seek bar's in-flight \
             timestamp is supposed to, and this test just stopped meaning \
             anything"
        );
    }

    /// **Every type size is drawn with its own leading.**
    ///
    /// The scale is six size/leading pairs (§8 of the design system), and the
    /// pairing only exists if the views honour it: a `text` that sets
    /// [`SIZE_CAPTION`] and leaves the line height alone gets iced 0.13's 1.3
    /// default, which is the single compromise the per-token leadings were
    /// introduced to replace. That is invisible in a screenshot of one line and
    /// obvious in a block of three.
    ///
    /// Read from the sources for the same reason the accent's second test is:
    /// no style function is involved, so nothing else can see it.
    ///
    /// The window is 80 characters because rustfmt will break the two calls
    /// onto separate lines with indentation between them. One size in the
    /// product is not type at all — `checkbox`'s `.size` is the *box*, and its
    /// label is `.text_size` / `.text_line_height` — so a size followed
    /// immediately by a `.text_size` is skipped rather than special-cased by
    /// file name.
    #[test]
    fn every_type_size_a_view_sets_is_drawn_with_its_own_leading() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/views");
        let mut offenders: Vec<String> = Vec::new();
        for path in rust_sources(&root) {
            let source = std::fs::read_to_string(&path).expect("a source file baz ships");
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            for (at, _) in source.match_indices("theme::SIZE_") {
                let window = &source[at..source.len().min(at + 80)];
                if window.contains("theme::LEADING_") || window.contains(".text_size(") {
                    continue;
                }
                offenders.push(format!("{name}: {}", window.lines().next().unwrap_or("")));
            }
        }
        assert!(
            offenders.is_empty(),
            "a type size is set without its line height: {offenders:#?}\nEvery \
             size token in `theme` has a LEADING_ beside it; taking iced's 1.3 \
             default instead is the compromise those pairs exist to replace."
        );
    }

    /// **The monospace is gone, and it stays gone.**
    ///
    /// The owner's complaint about the shipped UI was, verbatim, *"some weird
    /// monospace looking fonts which are lame"*, and
    /// `.interface-design/system.md` §8 answers it in one line: **no monospace
    /// anywhere in baz**. Deleting the token makes today's build compile
    /// without one; this is what makes tomorrow's build do the same.
    ///
    /// A second face cannot come back by accident — the compiler would ask for
    /// its bytes — but it can come back on purpose, one generic typewriter
    /// family at a time, and the reason it must not is *measured* rather than
    /// aesthetic:
    /// Plex Sans's figures are already tabular
    /// (`crate::font`'s `the_sans_carries_baz_s_tabular_figures_in_every_weight_it_sets_them_in`),
    /// so a monospace would buy nothing and cost the interface its voice.
    ///
    /// Read from the sources rather than asserted about the tokens, in the
    /// shape `the_lamp_is_named_only_where_playback_truth_is_drawn` established:
    /// a style sweep cannot see a view that names a face.
    #[test]
    fn no_monospace_survives_anywhere_in_the_crate() {
        // Spelled in halves so this test does not find itself.
        let token = concat!("MO", "NO");
        let asset = concat!("IBMPlex", "Mono");

        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut offenders: Vec<String> = Vec::new();
        for path in rust_sources(&manifest.join("src")) {
            let source = std::fs::read_to_string(&path).expect("a source file baz ships");
            if source.contains(token) || source.contains(asset) {
                offenders.push(path.to_string_lossy().into_owned());
            }
        }
        assert!(
            offenders.is_empty(),
            "{offenders:?} name a monospace. baz sets every figure in the Sans, \
             whose digits are tabular by default (.interface-design/system.md \
             §8); a second face buys nothing and reads as a typewriter."
        );

        // …and the asset directory carries no face the crate could reach for.
        let faces = std::fs::read_dir(manifest.join("assets/fonts"))
            .expect("the bundled typeface")
            .map(|entry| entry.expect("a readable directory entry").path())
            .filter(|path| path.extension().is_some_and(|kind| kind == "ttf"))
            .map(|path| {
                path.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect::<Vec<_>>();
        assert!(
            !faces.iter().any(|name| name.contains(asset)),
            "the monospace is still on disk: {faces:?}"
        );
        assert_eq!(
            faces.len(),
            crate::font::FACES.len(),
            "the bundled faces and the shipped files disagree: {faces:?}"
        );
    }

    /// Every `.rs` file under `root`, recursively.
    fn rust_sources(root: &std::path::Path) -> Vec<std::path::PathBuf> {
        let mut found = Vec::new();
        let mut pending = vec![root.to_path_buf()];
        while let Some(directory) = pending.pop() {
            for entry in std::fs::read_dir(&directory).expect("baz's own source tree") {
                let path = entry.expect("a readable directory entry").path();
                if path.is_dir() {
                    pending.push(path);
                } else if path.extension().is_some_and(|extension| extension == "rs") {
                    found.push(path);
                }
            }
        }
        found
    }
}
