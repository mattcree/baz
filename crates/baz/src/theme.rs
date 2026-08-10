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
//! 3. the needle's fill — [`needle`], where the queue has played;
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
//! the search field used to take focus at launch, so the first frame baz ever
//! drew was an amber ring with no music — type-anywhere has since removed the
//! launch focus too), and the scanning note (now [`Palette::paper_dim`]
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

use iced::font::{self, Weight};
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
/// The synthetic surface step [`Palette::step_up`] uses **above the ladder's
/// top plane**: the room's ink at **5 %**.
///
/// It is not a fifth plane; it is the ladder's own rise, measured. The real
/// steps `wall → plinth` and `plinth → plinth_lit` work out at 3.6–4.4 % of
/// the ink in Closing Time and 5.3–6.3 % in Reading Room, so 5 % sits inside
/// the band both rooms already draw — which is the property
/// `a_surface_step_is_the_ladders_own_rise` asserts rather than assumes.
const SURFACE_STEP_A: f32 = 0.05;
/// Opacity of [`Palette::lamp_glow`]: the accent at **30 %**, blurred.
const LAMP_GLOW_A: f32 = 0.30;
/// Opacity of [`Palette::ink_wash`]: the room's ink at **6 %** — the whole of
/// what an icon button gains under the pointer, and the whole of the critique's
/// `hover = ink 6% overlay`.
///
/// A wash rather than a surface step, on purpose: [`Palette::plinth`] is a
/// *place* (a panel, a popover, a resting control), and a 32 px square that
/// becomes a panel for as long as a pointer crosses it is exactly the clunk the
/// surface pass removes.
const INK_WASH_A: f32 = 0.06;
/// Opacity of [`Palette::ink_wash_press`]: the room's ink at **10 %**.
///
/// One step stronger than [`INK_WASH_A`] and nothing else — no inversion, no
/// recess, no border. A press is a moment; it should read as the same mark
/// leaning on the button, not as a second design.
const INK_WASH_PRESS_A: f32 = 0.10;
/// Opacity of [`Palette::lamp_wash`]: the accent at **10 %**.
///
/// The only accent-coloured ground in baz, and it is a wash rather than a fill
/// — the product's standing rules: *the accent is never an opaque fill*. At 10 % over the
/// plinth it composites to a warmth rather than to a colour, so `Play album`
/// warms under the pointer without becoming the brightest object in a room
/// whose brightest object is supposed to be a record sleeve.
const LAMP_WASH_A: f32 = 0.10;
/// Opacity of [`Palette::lamp_wash_press`]: the accent at **20 %**.
const LAMP_WASH_PRESS_A: f32 = 0.20;

impl Palette {
    /// `ink` at `opacity` over `ground`, **as an opaque colour** — the fix for
    /// the single largest visual defect in the product
    /// (`docs/design/05-toolkit-and-visual-gap.md` D1).
    ///
    /// # Why an opaque colour and not an alpha
    ///
    /// **CSS composites `rgba()` in sRGB. iced composites in linear light.**
    /// `iced_graphics-0.13.0/src/color.rs:33` packs every colour through
    /// `into_linear()` before it reaches the shader; the GPU blends
    /// source-over in linear space and the sRGB surface encodes on write. Every
    /// number in this room's spec was written in the first model and drawn in
    /// the second.
    ///
    /// Measured off the committed renders, predicted to the byte in all three
    /// channels: `hairline` asked for 7 % and drew `#454442`, which **reads as
    /// ink 26 %** — 3.7× its specified weight. `hairline_strong` 15 % drew
    /// `#63615D`, ink 39 %. `paper_ring` 45 % drew `#A3A099`, ink 68 %. Every
    /// separator, every control edge, the focus ring, the scrollbar and the
    /// selection wash were all one to two ink-steps too heavy at once. Nothing
    /// was individually broken; everything was one notch loud, which is the
    /// whole of *"it doesn't look amazing"*.
    ///
    /// And it **inverts between rooms**: on Reading Room's light ground the same
    /// 7 % draws `#E7E4DD`, which reads as ink **4 %** — half its weight instead
    /// of four times it. So one token drew a shout in the dark room and a
    /// whisper in the light one, and the "one token, four rooms" abstraction the
    /// whole palette indirection was built for did not hold.
    ///
    /// An **opaque** colour is immune to the blend space by construction: there
    /// is no blend. So every mark below is composited *here*, in sRGB, in the
    /// model its number was written in, and handed to the renderer as a colour
    /// rather than as an instruction. It costs one parameter — the ground the
    /// mark lands on — which every call site already knows.
    ///
    /// The alternative was iced's `web-colors` feature, and it is rejected: it
    /// is a whole-renderer switch that also changes the surface format, the
    /// image-atlas format and glyphon's colour mode, so it would change how
    /// **album art** renders. Fixing chrome by changing how the works are drawn
    /// is not a fix.
    #[must_use]
    pub fn ink_over(ink: Color, ground: Color, opacity: f32) -> Color {
        let mix = |over: f32, under: f32| opacity.mul_add(over - under, under);
        Color {
            r: mix(ink.r, ground.r),
            g: mix(ink.g, ground.g),
            b: mix(ink.b, ground.b),
            a: 1.0,
        }
    }

    /// **One surface step up from `ground`** — the room's four-plane ladder
    /// walked upward, and the whole of what "the pointer is on this row" is
    /// allowed to say.
    ///
    /// # Why this exists, and what it fixed
    ///
    /// The row styles used to name their two surfaces as *values*
    /// ([`Palette::plinth`] under the pointer, [`Palette::plinth_lit`] for the
    /// playing row), which is correct exactly as long as every row in the
    /// product stands on the wall. The playlist panel's rows do not: the
    /// panel's own ground *is* `plinth`, so a row that painted `plinth` under
    /// the pointer painted **the colour that was already there**. The rows
    /// were pressable, correctly wired and completely mute — the owner's
    /// words, 2026-08-09: *"a more clear indicator that something is a click
    /// area… right now it's a bit… unresponsive"*.
    ///
    /// The fix is not a second style for the panel. It is that a hover is a
    /// **relation** — one step above whatever you are standing on — and the
    /// call site already knows its ground, exactly as it does for
    /// [`Palette::hairline`] and [`word_button`]. Every row-shaped control in
    /// the product now names its ground and steps from it, so a surface
    /// composed on a different plane cannot go mute again.
    ///
    /// Above the ladder's top plane the step is synthesised at
    /// [`SURFACE_STEP_A`] rather than saturating, so a control on
    /// `plinth_lit` — the menu card's rows — still answers the pointer.
    #[must_use]
    pub fn step_up(&self, ground: Color) -> Color {
        let same = |a: Color, b: Color| {
            (a.r - b.r).abs() < 1e-4 && (a.g - b.g).abs() < 1e-4 && (a.b - b.b).abs() < 1e-4
        };
        if same(ground, self.recess) {
            self.wall
        } else if same(ground, self.wall) {
            self.plinth
        } else if same(ground, self.plinth) {
            self.plinth_lit
        } else {
            Self::ink_over(self.paper, ground, SURFACE_STEP_A)
        }
    }

    /// Hairline border on `ground`: findable when you look, invisible when you
    /// don't. The room's ink at [`HAIRLINE_A`], composited by [`Palette::ink_over`].
    #[must_use]
    pub fn hairline(&self, ground: Color) -> Color {
        Self::ink_over(self.paper, ground, HAIRLINE_A)
    }

    /// The hairline, firmer — a selected control's edge, the playing row's
    /// edge. The room's ink at [`HAIRLINE_STRONG_A`].
    #[must_use]
    pub fn hairline_strong(&self, ground: Color) -> Color {
        Self::ink_over(self.paper, ground, HAIRLINE_STRONG_A)
    }

    /// Keyboard focus, on the focused `text_input`'s border and nowhere else.
    ///
    /// Deliberately **not** the accent. Where the keyboard is has nothing to
    /// do with where the music is, and the search field takes focus at
    /// launch — so an amber focus ring made the first frame baz ever drew a
    /// lit lamp with nothing playing.
    #[must_use]
    pub fn paper_ring(&self, ground: Color) -> Color {
        Self::ink_over(self.paper, ground, self.ring_alpha)
    }

    /// Selected text in a `text_input`.
    ///
    /// Also not the accent, and for the same reason as
    /// [`Palette::paper_ring`]: a selection is a fact about the keyboard, not
    /// about the music. A wash rather than a fill, so the glyphs under it keep
    /// their own ink — which is why the contrast test measures the *ink on the
    /// composited wash* rather than the wash itself.
    #[must_use]
    pub fn select_wash(&self, ground: Color) -> Color {
        Self::ink_over(self.paper, ground, SELECT_WASH_A)
    }

    /// The accent as a glow: the playing sleeve's halo, and nothing else.
    #[must_use]
    pub const fn lamp_glow(&self) -> Color {
        alpha(self.lamp, LAMP_GLOW_A)
    }

    /// The halo `warmth` of the way up — the lamp warming (ADR-0020 §2.5).
    ///
    /// **The light's strength, never its geometry.** The halo's blur is
    /// [`HALO_BLUR`] in every frame of the warm; what moves is how much light
    /// there is, which is what a filament coming up actually does and the only
    /// reading of "a lamp warming" that leaves the sleeve exactly where it is.
    /// At `warmth` 1 this is [`Palette::lamp_glow`], to the bit.
    ///
    /// It is the accent, deliberately and at every point of the ramp: the halo
    /// is playback truth, which is the one thing the accent is for.
    #[must_use]
    pub fn lamp_glow_at(&self, warmth: f32) -> Color {
        alpha(self.lamp, LAMP_GLOW_A * warmth.clamp(0.0, 1.0))
    }

    /// An icon button's hover wash — the room's ink at [`INK_WASH_A`].
    #[must_use]
    pub fn ink_wash(&self, ground: Color) -> Color {
        Self::ink_over(self.paper, ground, INK_WASH_A)
    }

    /// A tile's hover rule, `strength` of the way in — the room's ink at
    /// [`HAIRLINE_STRONG_A`] × `strength`, over `ground`.
    ///
    /// **Still an opaque pre-composite at every point of the fade**, which is
    /// the property the whole [`Palette::ink_over`] correction bought: a
    /// transition that expressed itself as an alpha would draw at three to four
    /// times its weight in the dark room and half of it in the light one, and
    /// the mid-flight frames would be wrong in a way no endpoint test could see.
    /// At `strength` 1 this *is* [`Palette::hairline_strong`], to the bit.
    #[must_use]
    pub fn hover_rule(&self, ground: Color, strength: f32) -> Color {
        Self::ink_over(
            self.paper,
            ground,
            HAIRLINE_STRONG_A * strength.clamp(0.0, 1.0),
        )
    }

    /// `from` a fraction `t` of the way to `to`, channel by channel.
    ///
    /// The one interpolation the room does, and it is between two inks that are
    /// already on the same ramp ([`Palette::paper`] and its relatives are one
    /// board at four levels of light), so a mixture of two of them is a point on
    /// that ramp rather than a new colour. Opaque in, opaque out.
    #[must_use]
    pub fn mix(from: Color, to: Color, t: f32) -> Color {
        let t = t.clamp(0.0, 1.0);
        let blend = |a: f32, b: f32| t.mul_add(b - a, a);
        Color {
            r: blend(from.r, to.r),
            g: blend(from.g, to.g),
            b: blend(from.b, to.b),
            a: blend(from.a, to.a),
        }
    }

    /// An icon button's pressed wash — the room's ink at [`INK_WASH_PRESS_A`].
    #[must_use]
    pub fn ink_wash_press(&self, ground: Color) -> Color {
        Self::ink_over(self.paper, ground, INK_WASH_PRESS_A)
    }

    /// The primary action's hovered ground — the accent at [`LAMP_WASH_A`].
    #[must_use]
    pub fn lamp_wash(&self, ground: Color) -> Color {
        Self::ink_over(self.lamp, ground, LAMP_WASH_A)
    }

    /// The primary action's pressed ground — the accent at
    /// [`LAMP_WASH_PRESS_A`].
    #[must_use]
    pub fn lamp_wash_press(&self, ground: Color) -> Color {
        Self::ink_over(self.lamp, ground, LAMP_WASH_PRESS_A)
    }

    /// A group or section heading: the room's quietest voice, and the only
    /// chrome voice it has.
    ///
    /// The critique's *"9–10 px caps at ink 40 %, the only chrome voice"*,
    /// resolved against what iced 0.13 can draw. There is no letter-spacing and
    /// no small-caps in the toolkit (§12), so the caps are the view's own
    /// `to_uppercase` and the tracking the design wanted is simply not
    /// available; what is left — small, quiet, capitalised — is still
    /// unmistakably a different voice from the type around it, which is the
    /// job.
    #[must_use]
    pub const fn heading(&self) -> Color {
        self.paper_muted
    }

    /// One stop of a placeholder sleeve's gradient, quietened toward the
    /// [`Palette::recess`] the sleeve is backed with.
    ///
    /// The placeholder is deterministic two-colour art derived from the album's
    /// id ([`crate::vm::gradient_colors`]), and at full strength it was the
    /// loudest thing on the wall — a wall of real covers, most of them dark,
    /// punctuated by saturated gradients belonging to the records baz knows
    /// *least* about. That inverts the hierarchy: an album with no art should be
    /// the quietest tile in its row, not the brightest.
    ///
    /// Pulled back rather than desaturated, because the point of the gradient is
    /// that two albums with no art still look like two different albums; hue is
    /// the whole of the identification and it survives the mix.
    ///
    /// It mixes toward the *room's* recess, so a light room quietens a
    /// placeholder by lightening it — surfaces rise toward the lamp and a
    /// missing sleeve sinks with the plane it sits on.
    #[must_use]
    pub fn placeholder_ink(&self, stop: Color) -> Color {
        let mix = |from: f32, to: f32| PLACEHOLDER_MIX.mul_add(to - from, from);
        Color {
            r: mix(stop.r, self.recess.r),
            g: mix(stop.g, self.recess.g),
            b: mix(stop.b, self.recess.b),
            a: 1.0,
        }
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
//
// # The leading is derived from the line box, not the other way round
//
// `docs/design/06-composition-audit.md` §2 measured the consequence of choosing
// the factors first: the six line boxes came to 15.95, 16.20, 18.20, 20.25,
// 22.80 and 32.20, **none of them a multiple of the spacing unit**, so every
// stack of type in the product accumulated a different fractional error the
// moment it had more than one line. Pooled over the whole application a 4 px
// lattice caught 77–80 % of the drawn edges against a 75 % null — chance. There
// was no vertical rhythm and there could not be one, because the type was not
// in it.
//
// So the **line box** is the token now (law L2, `.interface-design/system.md`
// §13), each an exact multiple of 4, and the leading is `LINE / SIZE`. The
// quantisation cost at most 1.8 px on one token and it hands the system two
// numbers it already wanted: `SIZE_BODY`'s box becomes 20, so [`LABEL_H`] is
// **40 = [`HANG`]** and a wall label is exactly one hang tall; and caption and
// meta collapse onto one 16 px box, so the bar's left zone is stacked out of two
// numbers instead of five.

/// Hints and footnotes (11 px).
pub const SIZE_CAPTION: f32 = 11.0;
/// Line box of [`SIZE_CAPTION`]: **16** — the loosest ratio in the scale,
/// because the smallest type is the type that needs the air.
pub const LINE_CAPTION: f32 = 16.0;
/// Leading for [`SIZE_CAPTION`], derived from [`LINE_CAPTION`].
pub const LEADING_CAPTION: f32 = LINE_CAPTION / SIZE_CAPTION;
/// Metadata: captions, durations, status counts (12 px).
pub const SIZE_META: f32 = 12.0;
/// Line box of [`SIZE_META`]: **16**, the same box the caption takes — one lane
/// serves both of the room's two quiet voices.
pub const LINE_META: f32 = 16.0;
/// Leading for [`SIZE_META`], derived from [`LINE_META`].
pub const LEADING_META: f32 = LINE_META / SIZE_META;
/// Body: tile titles, track titles, control labels (13 px).
pub const SIZE_BODY: f32 = 13.0;
/// Line box of [`SIZE_BODY`]: **20**, and the largest single correction the
/// quantisation made (from 18.2). Two of them are [`LABEL_H`] 40 = [`HANG`].
pub const LINE_BODY: f32 = 20.0;
/// Leading for [`SIZE_BODY`] — and, through [`CAPTION_LINE_H`], the height of
/// a wall label's line.
pub const LEADING_BODY: f32 = LINE_BODY / SIZE_BODY;
/// Emphasis: search text, panel artist, empty-state lines (15 px).
pub const SIZE_EMPHASIS: f32 = 15.0;
/// Line box of [`SIZE_EMPHASIS`]: **20**, the same lane the body takes, so a
/// heading and the line under it share a rhythm.
pub const LINE_EMPHASIS: f32 = 20.0;
/// Leading for [`SIZE_EMPHASIS`], derived from [`LINE_EMPHASIS`].
pub const LEADING_EMPHASIS: f32 = LINE_EMPHASIS / SIZE_EMPHASIS;
/// Titles: the side panel's album title (19 px).
pub const SIZE_TITLE: f32 = 19.0;
/// Line box of [`SIZE_TITLE`]: **24** — tight, because a two-line album title
/// is one object and should look like one.
pub const LINE_TITLE: f32 = 24.0;
/// Leading for [`SIZE_TITLE`], derived from [`LINE_TITLE`].
pub const LEADING_TITLE: f32 = LINE_TITLE / SIZE_TITLE;
/// Hero: the first-run question (28 px).
pub const SIZE_HERO: f32 = 28.0;
/// Line box of [`SIZE_HERO`]: **32**, the tightest ratio in the scale.
pub const LINE_HERO: f32 = 32.0;
/// Leading for [`SIZE_HERO`], derived from [`LINE_HERO`].
pub const LEADING_HERO: f32 = LINE_HERO / SIZE_HERO;

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
/// Height of a wall label: two lines at [`SIZE_BODY`]'s line box — **40**,
/// which is exactly [`HANG`].
///
/// The name `.interface-design/system.md` §8 gives [`CAPTION_H`], which is the
/// same number in the module that draws it. Kept as an alias rather than
/// collapsed, because the hang's row pitch is arithmetic about a *label* and
/// the tile's reserved block is arithmetic about a *caption*, and they are the
/// same 40 for a reason worth being able to state twice.
///
/// It was **36.4**, and the change is the composition audit's §2.1 falling out
/// rather than a number being retuned: quantising [`LINE_BODY`] to 20 makes a
/// wall label exactly one hang tall, so the tile's pitch becomes `art + 96` and
/// the label's block is on the same lattice as the wall it hangs on.
pub const LABEL_H: f32 = CAPTION_H;

// ---------------------------------------------------------------------------
// The shelf break and the index rail (ADR-0017 step 8, §1.7).
//
// Everything below is arithmetic on `HANG`, `GAP_LG` and the type scale. The
// wall has exactly one number; a group key that introduced a second would be a
// second grid.
// ---------------------------------------------------------------------------

/// Height of a shelf's group-header band at the **default** density —
/// exactly [`HANG`].
///
/// The band holds the header's line box at its **top** and clear wall for the
/// rest, so the vertical rhythm of a break is stated in three numbers and all
/// three are the grid's own:
///
/// ```text
/// HANG                    40   the trailing hang of the row above (or the
///                              wall's own top hang, for the first shelf)
/// HEADING_LINE_H          12   the header's line box
/// SHELF_HEADER_H - line   28   clear wall, then the shelf's first row
/// ```
///
/// Air above the ink is 40 and air below it is 28 — **10 : 7** — so a header
/// sits nearer the shelf it names than the shelf it follows, which is the one
/// thing a section heading has to say with position. Neither number was
/// chosen: 40 is `HANG` and 28 is `HANG − HEADING_LINE_H`, and both are on the
/// 4 px lattice because the header's line box is (law L2).
///
/// It is also what makes the sticky header exact. The band is the hang, the
/// trailing hang of the row above is the hang, so the scroll offset at which a
/// shelf's last row of covers leaves the top of the viewport is precisely the
/// offset at which the next shelf's band enters it — see
/// [`crate::shelf::Shelves::sticky`].
///
/// **The wall reads it from the grid, not from here** —
/// [`crate::shelf::Grid::header_h`] — because ADR-0017 step 6 makes the hang a
/// function of the density step, and a band fixed at 40 while the rows around
/// it zoomed would break the exactness in the paragraph above at two of the
/// three steps. This constant is the default's value and the one the type
/// scale was derived against; it is *asserted* equal to the grid's band at
/// `Balanced` rather than read by the view.
pub const SHELF_HEADER_H: f32 = HANG;

/// The index rail's ink lane (logical px) — `.interface-design/system.md`
/// §7.2's spine index at the width ADR-0017 §1.7 gives it.
///
/// The rail is **type, not chrome**: no ground, no edge, no chips. This is the
/// width its type is laid out in, not the width of a widget with a background.
///
/// **60, where ADR-0017 §1.7 first said 36** — an amendment recorded on that
/// ADR, and it is a correction rather than a preference. At 36 the lane clipped
/// `Unknown`, every recency bucket (`This month`, `This year`, `Earlier`) and
/// most genre names: it held the letters of the alphabet keys and failed for
/// three of the keys the wall can be arranged by, which is a rail that only
/// works in one arrangement. 60 holds every label the keys can *produce* except
/// arbitrary genre names — and those still elide, with the full value set in the
/// shelf header one gutter to the left, in the same voice, at the same moment.
/// `crate::font`'s `the_index_rail_holds_the_labels_its_keys_produce` measures
/// the whole set against this number.
pub const INDEX_W: f32 = 60.0;

/// Clearance between the wall's scrollbar and the rail's ink.
///
/// The bar overlays the right of the grid's margin, and the rail's lane begins
/// where the grid's width ends, so the two are neighbours by construction and
/// the rail's ink has to be told to stand off. [`GAP_SM`] is the ladder's
/// "siblings within a group", which is what a scrollbar and an index are.
pub const INDEX_CLEARANCE: f32 = GAP_SM;

/// What the rail costs the wall: the scrollbar's clearance, the rail's ink
/// lane, and the gutter between the lane and the window's edge — **108**.
///
/// Three tokens and no new number:
///
/// ```text
///  8  INDEX_CLEARANCE   the wall's scrollbar is on the other side of this
/// 60  INDEX_W           the rail's type
/// 40  HANG              the gutter to the window's edge
/// ```
///
/// The right gutter is [`HANG`] because **there is one window gutter**
/// (law L1): every surface that touches a window edge hangs from `x = HANG` and
/// `x = W − HANG`, and the rail's right edge is therefore the same x as the
/// `Settings` word above it and the same x as the last column of covers. It was
/// [`GAP_LG`], which put it on the *old* chrome gutter of 16 and made the rail
/// the only thing on the wall that did not line up with the wall.
///
/// With the grid resolved for `width − INDEX_LANE_W`, the hang then leaves
/// exactly [`HANG`] between the last column of covers and the start of the
/// rail's lane — the rail hangs off the wall at the same distance as every
/// work hangs off its neighbour. That is asserted over the whole width band by
/// `the_rail_lane_hangs_at_exactly_one_hang_from_the_last_column`.
pub const INDEX_LANE_W: f32 = INDEX_CLEARANCE + INDEX_W + HANG;

/// **The returns lane, open** (ADR-0030 §2): [`GAP_XL`] 24 + the content lane
/// 232 + [`GAP_XL`] 24 = **280**.
///
/// The content lane is [`MENU_W`] 232 — the product's existing float measure —
/// so the lane introduces no width of its own, and the two gutters are the
/// panel-interior gap rather than the window's [`HANG`]: the lane is a surface
/// *inside* the window rather than one hanging off its edge, which is what its
/// own ground already says.
///
/// It lands on the industry's number from the other direction: Material's
/// navigation drawer is 280 dp at its default maximum. Corroboration, not
/// derivation.
pub const SIDEBAR_W: f32 = GAP_XL + SIDEBAR_MEASURE + GAP_XL;

/// The lane's content measure — [`MENU_W`] 232 — which is the width every head
/// row, the well and every list row are drawn at, and the width `font.rs`
/// measures the well's two strings against.
pub const SIDEBAR_MEASURE: f32 = MENU_W;

/// **The returns lane, collapsed**: [`GAP_XL`] 24 + [`SIDEBAR_SLEEVE`] 48 +
/// [`GAP_XL`] 24 = **96**. Material's expressive navigation rail, again from
/// the other direction.
pub const SIDEBAR_RAIL_W: f32 = GAP_XL + SIDEBAR_SLEEVE + GAP_XL;

/// The sleeve on a lane row: **48**, one step above [`PANEL_SLEEVE`] 40.
///
/// Collapsed, the sleeve is the *only* thing identifying the row, so it is
/// drawn one step larger than the panel's — the same face at the size a person
/// can recognise a record by without a word beside it.
pub const SIDEBAR_SLEEVE: f32 = PANEL_SLEEVE + GAP_SM;

/// A lane row's pitch: **64** — [`SIDEBAR_SLEEVE`] 48 and one [`GAP_SM`] above
/// and below.
///
/// It holds the two-line block ([`LINE_BODY`] 20 over [`LINE_META`] 16 = 36)
/// centred, and it is above the hit-target floor at every density.
pub const SIDEBAR_ROW_H: f32 = SIDEBAR_SLEEVE + 2.0 * GAP_SM;

/// The head's destination row: **40** — [`TRANSPORT_HIT`] 32, the product's
/// control hit box, with one [`GAP_SM`] of air under it.
///
/// A destination is a *control*, not a listing, so its box is the box every
/// other pressable thing in the product stands in; the gap is what separates
/// three of them stacked without a rule between.
pub const SIDEBAR_DEST_H: f32 = TRANSPORT_HIT + GAP_SM;

/// **The width below which the lane is always collapsed**: **1000**.
///
/// The smallest window at which the *expanded* lane still leaves the wall two
/// columns at or above [`ART_MIN`] — 988 by arithmetic, rounded up onto the 4 px
/// lattice. Below it the lane draws its rail and the `Expanded` mark is inert:
/// a control that would leave the collection one column of covers is not a
/// control, it is a trap.
pub const SIDEBAR_FLOOR: f32 = 1000.0;

/// **The lane's width at a window width, in the state the listener asked
/// for** — the one function `Shelf::grid_width`, the composition and every
/// width test read, so the geometry cannot be resolved two ways.
///
/// `open` is what is *persisted*; below [`SIDEBAR_FLOOR`] the answer is the
/// rail whatever it says (ADR-0030 §3), which is why this takes the width and
/// not just the bool.
#[must_use]
pub fn sidebar_w(window_w: f32, open: bool) -> f32 {
    if open && window_w >= SIDEBAR_FLOOR {
        SIDEBAR_W
    } else {
        SIDEBAR_RAIL_W
    }
}

/// Whether the lane may be expanded at all at this window width — what makes
/// the `Expanded` mark inert rather than merely unpressed (ADR-0030 §3).
#[must_use]
pub fn sidebar_can_expand(window_w: f32) -> bool {
    window_w >= SIDEBAR_FLOOR
}

/// The box a head destination's glyph stands in: [`ICON_PX`] 16 with room for
/// the lamp dot tucked against its top-right corner.
///
/// The dot must survive the collapse (the owner's requirement), so it is
/// stacked *on the glyph* rather than set after the word — which means the
/// glyph needs a box slightly larger than itself to tuck it into, in both
/// states, at the same offset. [`GAP_SM`] of headroom is exactly the
/// [`DOT`] 6 plus the 2 px that keeps it off the ink.
pub const SIDEBAR_GLYPH_BOX: f32 = ICON_PX + GAP_SM;

/// **Where a head row's glyph is centred**, from the row's own left edge:
/// [`GAP_SM`] 8 + half of [`SIDEBAR_GLYPH_BOX`] = **20**.
///
/// Declared because the search well has to land on it. The well draws its
/// magnifier as a layer over its own left padding ([`GAP_MD`] 12) and the
/// glyph is [`ICON_PX`] 16 wide, so its centre is `12 + 8` = 20 — the same
/// vertical the three destinations' marks stand on. That equality is asserted
/// rather than eyeballed (`the_lane_head_stands_on_two_verticals`): a well
/// whose mark sat 4 px off the marks above it would say *this is a different
/// surface* louder than any of its words.
pub const SIDEBAR_HEAD_GLYPH_X: f32 = GAP_SM + SIDEBAR_GLYPH_BOX / 2.0;

/// **Where a head row's words begin**, from the row's own left edge:
/// [`GAP_SM`] 8 + [`SIDEBAR_GLYPH_BOX`] 24 + [`GAP_MD`] 12 = **44**.
///
/// The destination rows get it from the row's padding, its glyph box and its
/// spacing; the well gets it by declaring it as the input's left padding, and
/// the readout line under the well is indented to it too. One vertical for
/// every word in the head.
pub const SIDEBAR_HEAD_TEXT_X: f32 = GAP_SM + SIDEBAR_GLYPH_BOX + GAP_MD;

/// The lead the well's magnifier needs to stand on [`SIDEBAR_HEAD_GLYPH_X`]:
/// the vertical, less half the glyph. **12** — which is [`GAP_MD`], but stated
/// as the derivation so that a change to the head's box moves the mark with
/// it rather than leaving it 4 px off three glyphs above it.
pub const SIDEBAR_WELL_GLYPH_LEAD: f32 = SIDEBAR_HEAD_GLYPH_X - ICON_PX / 2.0;

/// **The search well's block in the lane's head**: **32** — the field, and
/// nothing under it.
///
/// It was 52: the field over an always-drawn [`LINE_META`] line carrying the
/// collection's counts at rest and the query's match count while one narrowed
/// the collection. The owner read the built thing — *"the album and track count
/// below the search bar doesn't look good… maybe this should go into the home
/// as some basic stats?"* — and the two figures on that line turned out to have
/// two different jobs. **The resting counts are a statistic about the
/// collection** and are the Home place's footer now; **the match count is
/// feedback about the query** and went *inside* the field, right-aligned in
/// [`SIDEBAR_MATCH_W`], which is where a search field has always put it and
/// which costs no line at all.
///
/// So the block is one control tall, the head is 20 px shorter, and the list
/// below it gets that back: **11 whole `RECENT` rows at 1920 × 1080 where
/// there were 10**, measured off the frames in
/// `docs/design/impl/home-stats/`. At 1280 × 860 the same 20 px buys three
/// eighths of a row and no whole one — 7 rows either way — which is stated
/// because the arithmetic that preceded the measurement predicted the opposite.
/// The height is still *fixed* for the reason it always was: nothing below the
/// well may move when a key lands in it.
pub const SIDEBAR_WELL_H: f32 = TRANSPORT_HIT;

/// **The match count's reserved slot inside the lane's well**: **72**.
///
/// The lane's own figure, not the strip's. [`crate::views::top_bar::MATCH_W`]
/// is 88 because the strip could afford room for `40000 / 40000` beside a
/// 280 px well; at [`SIDEBAR_MEASURE`] 232 that reservation would leave the
/// query 88 px, which is what drove both figures out of the field in the first
/// place. 72 holds `9999 / 9999` — a collection ten times the owner's — and
/// leaves the query **104 px**, which `font.rs` measures rather than assumes
/// (`the_lanes_well_holds_a_query_beside_its_match_count`).
///
/// Fixed, for the reason every readout slot in the product is fixed: the
/// figures change as the query narrows, and a right-aligned slot of constant
/// width means they change **in place**. The field reserves it only while a
/// query stands, and the reservation is on the *right*, so the caret and the
/// first character it sets never move.
pub const SIDEBAR_MATCH_W: f32 = 72.0;

/// **A work's own title**, and the two places in the product it is set: the
/// Home place's `CONTINUE` placard and the record page's hero.
///
/// IBM Plex Serif **Italic** — the museum-placard convention, where the
/// work's title is italic and every fact around it (the artist, the date, the
/// medium) is not. baz's identity is a gallery and its icon is a work under a
/// wall label; this is that label's own typography, used for the strings on
/// screen that are a work's name standing beside its own facts.
///
/// # The line, and it is a rule rather than a count of call sites
///
/// It began as *the one placard's* type. That boundary was a **quantity**, and
/// a quantity cannot say whether the next string may have it — which is how a
/// display face arrives one surface at a time. The boundary is now a rule
/// (ADR-0024 §A4.4, design 14 §5.2):
///
/// > **The serif italic sets an album's title, on the surface whose subject
/// > that album is.** Not a track's title, not an artist's, not a playlist's
/// > name — and not an album's title standing as a *fact about something
/// > else*, which is what `views::now_playing` prints under the sounding
/// > track.
///
/// That is what makes the record page's hero italic and the playlist page's
/// hero — same size, same ink, same slot — **deliberately sans**: a record's
/// title is a work someone published, a playlist's name is a label the owner
/// typed, and every other typed string in this product is already sans. The
/// asymmetry is the design (doc 14 §2's last row) and costs no pixels.
///
/// Whether the rule should also reach the wall's tile captions and the returns
/// lane's rows is **the owner's question**, open (`docs/WORK.md`): it is sixty
/// italic serif captions on a wall of covers, and an italic serif at
/// [`SIZE_BODY`] 13 is answerable only from a frame.
///
/// **The typographic risk, seen and approved by the owner** (2026-08-09). It
/// is one token so that it is one line to revert: change this to
/// [`MEDIUM`] and the serif leaves the product, because
/// `crate::font::SERIF_ITALIC` has no other consumer and
/// `the_serif_is_the_work_titles_and_nothing_else` says so — an **enumerated**
/// list of consumers, never a `contains`, so a third arrives only on purpose.
pub const WORK_TITLE: Font = Font {
    style: font::Style::Italic,
    ..Font::with_name(crate::font::SERIF)
};

/// The needle's tick: **1 px** at the position, in the brightest accent.
///
/// What turns a proportion into a *position*. A bar alone reads as "how
/// much"; a mark on it reads as "where", which is the question the placard is
/// answering. It is taken out of the line's own width rather than added to it,
/// so the needle is the sleeve's measure at every position.
pub const NEEDLE_TICK_W: f32 = 1.0;

/// The `CONTINUE` placard's sleeve: **132**, between the panel's 40 and the
/// wall's smallest work.
///
/// Large enough that the record is identified by its cover rather than by its
/// name, small enough that the placard beside it — four lines and a needle —
/// is the thing being read. On the 4 px lattice, and it is exactly the width
/// the needle takes, which is the rule that makes the whole band read as one
/// object.
pub const CONTINUE_SLEEVE: f32 = 132.0;

/// **One cell of the Home place's `COLLECTION` footer**: **96**, a figure over
/// its word.
///
/// A *pitch*, not a maximum: the four cells stand on one lattice so the figures
/// line up as a row rather than drifting with the length of the words under
/// them. 96 is [`GAP_XL`] × 4, and `font.rs` measures both lines of every cell
/// against it (`the_home_collection_cells_hold_their_figures_and_their_words`).
///
/// Four of them with [`GAP_XL`] between comes to 456, which fits the narrowest
/// body the product can produce — the const-assert below is that claim, and it
/// is why this footer needs no responsive form.
pub const STAT_W: f32 = GAP_XL * 4.0;

/// The `COLLECTION` footer fits the narrowest place body baz can draw.
///
/// The floor window is `TOP_BAR_FLOOR + SIDEBAR_RAIL_W` (`app.rs`'s `min_size`),
/// where the lane is always the rail, so the body is [`TOP_BAR_FLOOR`] and the
/// content lane is that less [`place_pad`]'s two [`HANG`]s and the scrollbar's.
const _: () = assert!(
    4.0 * STAT_W + 3.0 * GAP_XL <= TOP_BAR_FLOOR - 2.0 * HANG - SCROLLBAR_LANE,
    "the COLLECTION footer overflows the narrowest place body"
);

/// **The lane's ground**: [`Palette::recess`], one plane *below* the wall.
///
/// The lane reads as cut into the room rather than stuck onto it, which is
/// what a resident surface has to do to stop looking like a panel that forgot
/// to close. It is also why the lane needs no drawn shadow and no card: a
/// plane below is a statement the ladder already makes.
#[must_use]
pub fn lane_ground(p: &Palette) -> container::Style {
    container::Style {
        background: Some(Background::Color(p.recess)),
        text_color: Some(p.paper),
        ..container::Style::default()
    }
}

/// **The collapsed lane's mark for the place you are in**: the destination's
/// card, drawn as a square around its glyph rather than as a band across the
/// rail.
///
/// Open, the current destination wears [`track_row`]'s card and hairline like
/// any emphasised row, and it reads correctly because a word stands in it.
/// Collapsed there is no word, so that same band became a rectangle drawn
/// around a lone glyph — the owner's *"in collapsed mode the now playing thing
/// has a rectangular outline around it"*. Two changes fix it: the card is the
/// glyph's own box, and **there is no border at all**. The hairline in
/// `track_row` marks *the row that is sounding*, which is a different fact
/// from *the place you are standing in*; borrowing it here made a selection
/// wear playback's mark.
pub fn lane_current(p: &Palette) -> container::Style {
    container::Style {
        background: Some(Background::Color(p.step_up(p.step_up(p.recess)))),
        text_color: Some(p.paper),
        border: iced::border::rounded(RADIUS_SEGMENT),
        ..container::Style::default()
    }
}

/// The hairline on the lane's right edge — the one mark that separates it
/// from the wall.
///
/// [`Palette::hairline_strong`] rather than the plain hairline because it is
/// read *across* a surface step: the two grounds either side of it already
/// differ, and a line that is quieter than the step it sits in reads as an
/// artefact of the step rather than as an edge.
#[must_use]
pub fn lane_seam(p: &Palette) -> container::Style {
    container::Style {
        background: Some(Background::Color(p.hairline_strong(p.recess))),
        ..container::Style::default()
    }
}

/// Group heading type: **10 px**, the caps size the critique names ("9–10 px
/// caps at ink 40 %, the only chrome voice").
///
/// Smaller than [`SIZE_CAPTION`], and the only size in the scale below it,
/// because a heading in this direction is the quietest thing on the wall
/// rather than the loudest.
pub const SIZE_HEADING: f32 = 10.0;
/// Line box of [`SIZE_HEADING`]: **12** — the tightest lane in the scale, and a
/// multiple of 4 like every other (law L2). It was 14, which is the one number
/// in the type scale that the audit's six-token table did not reach and the one
/// that kept the shelf break's arithmetic off the lattice.
pub const LINE_HEADING: f32 = 12.0;
/// Leading for [`SIZE_HEADING`], derived from [`LINE_HEADING`].
pub const LEADING_HEADING: f32 = LINE_HEADING / SIZE_HEADING;
/// A heading's line box: **12** (module docs on [`SHELF_HEADER_H`]).
pub const HEADING_LINE_H: f32 = LINE_HEADING;

/// One rail entry's line box — the heading's, because the rail speaks the
/// shelf header's voice in a column instead of a line.
pub const RAIL_LINE_H: f32 = LINE_HEADING;
/// Rail entry pitch: the line box and the gap to the next entry — **20**.
///
/// [`GAP_SM`] rather than [`GAP_XS`], because the lane's own line box came down
/// to 12 and §7.2's *the smallest type is the type that needs the air* has to be
/// spent somewhere: it is now spent on the gap rather than on the leading, which
/// keeps the pitch on the 4 px lattice where a 14.5 px line box never could be.
///
/// The view drew a 16 px pitch for a while — it stacked the entries with
/// [`GAP_XS`] while this token, its doc and the capacity arithmetic all said
/// 20 — and the divergence was caught on the pixels
/// (docs/design/impl/index-magnification/, the `before` frames measure 16
/// exactly). [`crate::spine`] lays the strip out from this number now, so the
/// slot the capacity budgets is the slot that is drawn.
pub const RAIL_PITCH: f32 = RAIL_LINE_H + GAP_SM;

/// How large the fisheye grows the rail entry under the pointer, as a factor
/// of its rest size (ADR-0020's amendment: pointer-derived deformation).
///
/// **2.5 — the dock's own territory.** It shipped at 1.9 first, bounded by the
/// fixed pitch so that no glyph left its slot; the owner's desktop verdict was
/// that 1.9 *"reads too subtle"*, and going dramatic means adopting the dock's
/// other half: the strip **displaces** as well as scales ([`magnify_shift`]),
/// so the swollen letters have room because their neighbours were pushed
/// aside, not because a cap kept them small. What bounds 2.5 now is the lane's
/// width: the widest letter at the peak measures ~26 px against the 60 px ink
/// lane (asserted in `font.rs`), and the peak line box (30 px) stays inside
/// the spread pitch beside it. Judged against real renders at 1280 and 1920
/// (docs/design/impl/index-magnification/).
pub const MAGNIFY_MAX: f32 = 2.5;

/// How far the fisheye's swell reaches: the rest distance (logical px, along
/// the strip) at which an entry stands at rest again — **60**, three slots
/// each side of the pointer.
///
/// The dock's feel is a generous peak with a *local* skirt: at three slots the
/// letter under the pointer has two visibly-swollen neighbours each side
/// (≈2.1× and ≈1.4× at the 2.5 peak), and the fourth letter out has not moved
/// — the deformation reads as a lens passing over the strip rather than the
/// strip breathing. Narrower pops single letters; wider stirs half the
/// alphabet. Judged on the captures beside [`MAGNIFY_MAX`]'s.
pub const MAGNIFY_REACH: f32 = 3.0 * RAIL_PITCH;

/// How far the fisheye pushes the rest of the strip out of the lens's way:
/// the displacement of everything at or beyond [`MAGNIFY_REACH`] — **45**,
/// the area under the falloff's hump, `(MAGNIFY_MAX − 1) × MAGNIFY_REACH / 2`.
///
/// This is the number the lane budgets for: [`crate::views::shelf`] sizes the
/// rail's elision capacity against `height − 2 × MAGNIFY_SPREAD`, so a strip
/// the capacity admitted always has this much air at each end and the lens can
/// push the strip's extremes without pushing any letter out of the lane. (A
/// strip that fits *without* elision may have less air; [`magnify_shift`]'s
/// callers cap the shift at the air that exists, and the spread degrades
/// before a letter ever clips.)
pub const MAGNIFY_SPREAD: f32 = (MAGNIFY_MAX - 1.0) * MAGNIFY_REACH / 2.0;

/// The fisheye's falloff: how large a rail entry stands, as a factor of its
/// rest size, given how far its rest centre is from the pointer.
///
/// A raised cosine — [`MAGNIFY_MAX`] at zero, easing monotonically to exactly
/// 1 at [`MAGNIFY_REACH`], 1 beyond. Smooth at both ends, so no letter pops as
/// the pointer travels and there is no seam where the swell meets the rest of
/// the strip; symmetric, because a lens has no upstream side.
///
/// Pure, and **a function of the pointer rather than of time** (ADR-0020
/// §Amendment): given the same distance it returns the same scale forever.
/// Distances are measured to the slot's *rest* centre, so the mapping from
/// pointer to deformation has no feedback through the deformed geometry and a
/// resting pointer draws a stable frame.
#[must_use]
pub fn magnify(distance: f32) -> f32 {
    let distance = distance.abs();
    if distance >= MAGNIFY_REACH {
        return 1.0;
    }
    let wave = (std::f32::consts::PI * distance / MAGNIFY_REACH).cos();
    (MAGNIFY_MAX - 1.0).mul_add(0.5 * (wave + 1.0), 1.0)
}

/// The fisheye's displacement: how far a rail entry stands from its rest
/// centre, given how far that centre is from the pointer — **the integral of
/// [`magnify`]`− 1`**, which is the dock's own mechanism. Each gap between
/// two entries stretches by exactly the mean magnification across it, so the
/// swollen letters sit in room their neighbours vacated and the drawn pitch
/// under the lens is the scaled pitch.
///
/// Signed and odd: an entry below the pointer moves down, one above moves up,
/// the entry exactly under the pointer does not move at all (`shift(0) = 0` —
/// the lens is anchored on the pointer, not sliding past it). Saturates at
/// ±[`MAGNIFY_SPREAD`] beyond the reach: the far field shifts as one piece,
/// which is what keeps the strip's *spacing* at rest out there.
///
/// Two properties the widget leans on, both asserted in the tests:
///
/// - **monotone**: `d + shift(d)` is strictly increasing (its derivative is
///   `magnify(d) ≥ 1`), so displaced entries keep their order and can never
///   cross;
/// - **hit-order preserving**: `|d + shift(d)|` grows with `|d|`, so the
///   entry whose *displaced* centre is nearest the pointer is exactly the one
///   whose rest centre is — the press math and the lens agree on the winner
///   by construction, not by coordination.
#[must_use]
pub fn magnify_shift(distance: f32) -> f32 {
    let reach = distance.abs().min(MAGNIFY_REACH);
    let wave = (std::f32::consts::PI * reach / MAGNIFY_REACH).sin();
    let area = (MAGNIFY_MAX - 1.0)
        * (MAGNIFY_REACH / (2.0 * std::f32::consts::PI)).mul_add(wave, reach / 2.0);
    area.copysign(distance)
}

/// The letter-spacing baz applies to a heading, as the string it is spelled
/// with: U+2009 THIN SPACE, one fifth of an em.
///
/// iced 0.13 has no `letter-spacing` — [`Palette::heading`] recorded that and
/// gave the tracking up. It is available after all, at the cost of spelling it
/// into the string: the bundled faces all map U+2009 (verified against the
/// `cmap` of each of the three, `the_bundled_faces_carry_the_tracking_space`),
/// so [`tracked`] is a real 0.2 em track and not a synthesised one.
///
/// **Headings only**, and only the short ones baz draws in caps. It is
/// deliberately not applied to anything a user reads as prose, anything
/// searchable, or anything measured for a reserved slot.
pub const TRACKING: &str = "\u{2009}";

/// A heading's text, tracked: [`TRACKING`] between every pair of characters.
///
/// Caps at 10 px with no tracking read as a small word; caps at 10 px with
/// 0.2 em of track read as a *label*, which is the difference between a header
/// and a caption that happens to be short. The design asked for both the caps
/// and the track (`.interface-design/system.md` §7.2, ADR-0017 step 8) and the
/// caps were already the view's own `to_uppercase`; this is the other half.
///
/// Pure and total: an empty string tracks to an empty string, one character
/// tracks to itself, and the space is inserted *between* characters rather
/// than after them, so a tracked heading never carries a trailing space that
/// would push a right-aligned label off its edge.
#[must_use]
pub fn tracked(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 2);
    for (index, character) in text.chars().enumerate() {
        if index > 0 {
            out.push_str(TRACKING);
        }
        out.push(character);
    }
    out
}

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
/// groove. A 4 px rail is a 4 px target, which is a miss waiting to
/// happen (Fitts); the pointer gets a band an order of magnitude taller to
/// aim at, and the cursor changes across the whole of it. [`NEEDLE_HIT`] is the
/// same idea for a control that cannot afford to reserve the height.
///
/// **10, where it was 9**: [`RAIL_HIT`] is a reserved slot height and law L2
/// puts every reserved slot on the 4 px lattice, so the band is 24 rather than
/// 22. The target got larger, which is the only direction a hit band is allowed
/// to move.
pub const HIT_SLOP: f32 = 10.0;
/// Hit height of a groove: the rail plus [`HIT_SLOP`] on each side. The widget
/// draws the rail centered in it.
pub const RAIL_HIT: f32 = RAIL + 2.0 * HIT_SLOP;
/// Radius of the fader's handle.
pub const KNOB: f32 = 5.0;
// ---------------------------------------------------------------------------
// The needle (ADR-0017 §1.1, step 9)
// ---------------------------------------------------------------------------

/// Thickness of the needle — the seek line flush on the window's bottom edge,
/// segmented by the queue's real entry lengths.
///
/// **2**, and the number is the argument. The 260 px groove plus its two stamps
/// and its hit band spent 45 of the bar's 102 px saying *where the playhead is*,
/// and the composition audit measured what that bought: the seek row was **last
/// of six** in the bar's own ink hierarchy, at 2.5 %, while occupying 37 of the
/// 77 px of content height. The needle states position *and* structure in 2 px —
/// you can see that you are three minutes into a nine-minute closer, which no
/// scalar groove has ever said — and gives the other 43 back to the collection.
pub const NEEDLE_H: f32 = 2.0;

/// The band the pointer may aim at above the needle.
///
/// A 2 px mark is a 2 px target, which is a miss waiting to happen (Fitts), so
/// the needle claims height the way [`HIT_SLOP`] does — except **upward, and
/// out of layout**: it reserves [`NEEDLE_H`] of row and tests the pointer
/// against a band [`NEEDLE_HIT`] tall reaching into the empty lane the bar
/// keeps under its transport. That is the only way a 2 px control can be
/// aimed at without charging the collection for the aiming.
///
/// **12 = [`GAP_MD`] = [`BAR_LEAD`]**, and the equality is the safety property
/// rather than a coincidence: the band is exactly the bar's bottom lane, which
/// is empty recess, so it can never take a press meant for a control.
/// ADR-0017's `NEEDLE_HIT 22` is amended here — 22 would reach 8 px into the
/// transport row's boxes, and a needle that swallows a press aimed at Next is
/// a worse bargain than a smaller band.
///
/// It is a **third** pointer height beside law L7's `TRANSPORT_HIT` 32 and
/// `STEPPER_HIT`/`RAIL_HIT` 24, and it is named here rather than smuggled: L7's
/// two heights are the heights of *boxes*, and the alternative for a line
/// flush on the window's edge is either 10 px of the transport row or 22 px of
/// the wall. The bound that keeps it honest is asserted, not asserted-about:
/// `NEEDLE_HIT <= BAR_LEAD`.
pub const NEEDLE_HIT: f32 = GAP_MD;

/// The gap the needle leaves between two consecutive queue entries.
///
/// [`GAP_XXS`], the lattice's one named exception (law L2) — an *intra-block*
/// gap, which is exactly what this is: the segments are one line, not a row of
/// slots. The critique specified 2 px here and this is that 2 px.
pub const SEGMENT_GAP: f32 = GAP_XXS;

/// The gap the needle leaves where one record ends and the next begins.
///
/// **8, where the critique said 6.** Six is off the 4 px lattice law L2 puts
/// every gap on, and 8 is the lattice's neighbour in the direction that makes
/// the break *more* legible on a 2 px line — four times the track gap rather
/// than three. It is the critique's "side break", generalised: baz's queue is
/// one list with a cursor (ADR-0016), so the wide gap falls at an **album
/// boundary** and the critique's spec is the single-album case of this one.
pub const ALBUM_GAP: f32 = GAP_XS * 2.0;

/// The narrowest a segment may be drawn.
///
/// Every entry in the queue is a control — clicking it jumps there — and the
/// visible-control rule (a standing rule of the product) does not have a "unless the track
/// is short" clause. So a segment's width is a floor plus a proportional share,
/// never a bare proportion: a 40-second interlude between two 12-minute sides
/// stays clickable, and an entry whose length the scan never read gets the
/// floor **and no proportional claim at all** — which is the honest drawing of
/// "we do not know how long this is".
pub const SEGMENT_MIN: f32 = GAP_XS;

/// Width of the needle's hover tip.
///
/// **160**: two thirds of the queue popover's own title lane (246 px), which is
/// where the same titles are listed in full. A floating chip wider than that
/// stops reading as a label and starts reading as a panel. Longer titles elide,
/// as they do in the popover and in the index rail — a title is free text and
/// no slot can bound it (§1.7's amendment made the same call for genre names).
pub const NEEDLE_TIP_W: f32 = 160.0;
/// Width reserved for each of the bar's two timestamps: enough for `h:mm:ss`
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
/// Height of the lane the hover preview floats in, directly above the groove.
///
/// **16, where it was 15**, and — for the *seek* groove — it is no longer a row
/// in the bar's centre column. The audit's defect 2 is that the bar centres its
/// zones as blocks, so the transport ends up 22.5 px above the bar's own
/// mid-line; the arithmetic of putting it back is unforgiving (see
/// [`BAR_LEAD`]), and 40 px of reserved lane below the transport is what made it
/// impossible. The tip is drawn as a **layer over the [`BAR_LEAD`] gap** it
/// already floated in instead — the same pixels, the same distance above the
/// rail, and no height at all.
///
/// It is still a reserved *lane* in the volume block, which is symmetric about
/// its own rail ([`VOLUME_ROW_H`]) and needs no such trick.
pub const PREVIEW_H: f32 = 16.0;

// ---------------------------------------------------------------------------
// The volume control
// ---------------------------------------------------------------------------

/// Width of the volume fader's groove.
///
/// Short on purpose: a fader is a setting and wants to sit quietly in the
/// corner (the thing that is a *map* now runs the whole width of the window and
/// is called [`NEEDLE_H`]). 96 px still gives ~10 control positions per pixel, which is
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
/// Height of the reorder drag's insertion line ([`crate::drag`]) — the
/// needle's own 2 px, the thinnest mark this room treats as a statement
/// rather than a border.
pub const INSERT_LINE_H: f32 = 2.0;
/// Hit height of the volume groove: the rail plus, on each side, room for
/// the knob and the detent mark above it. Taller than [`RAIL_HIT`] because
/// the mark has to live somewhere the handle is not.
pub const VOLUME_HIT: f32 = RAIL + 2.0 * (KNOB + DETENT_GAP + DETENT_H);
/// Height of the volume block — **32**, [`TRANSPORT_HIT`], one control height.
///
/// It was `2 × PREVIEW_H + VOLUME_HIT` = 60: a level-preview lane, the groove's
/// hit band, and an empty lane of the same height under it, so that the block
/// was *symmetric about its own rail* and centring the block centred the
/// **rail** rather than the block (law L4). That was right, and 60 no longer
/// fits: [`BAR_CONTENT_H`] is 56 once ADR-0017 step 10 takes the seek row out,
/// and a zone taller than the band would be the thing setting the bar's height.
///
/// The symmetry survives without the two lanes, because the fader's own hit
/// band is already symmetric about its rail: [`VOLUME_HIT`] 28 centred in 32
/// puts the rail on the block's centre line with 2 px of clearance to spare, and
/// the mute button beside it is [`TRANSPORT_HIT`] centred in the same 32. Both
/// marks land on the bar's one line, the block is now one of law L7's two
/// heights instead of a third, and the preview lane becomes a **layer** over the
/// [`BAR_LEAD`] gap above the fader — the same move [`PREVIEW_H`] documents for
/// the seek groove, generalised to the control that still exists.
pub const VOLUME_ROW_H: f32 = TRANSPORT_HIT;
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
    if engaged {
        p.paper
    } else {
        p.hairline(p.recess)
    }
}

/// The reorder drag's insertion line ([`crate::drag`]): where the lifted
/// row will land, stated in ink the room already owns.
///
/// [`Palette::paper_dim`], not the accent — where a row is *going* is a fact
/// about the hand, not about what is playing (the detent's own argument) —
/// and not a hairline: a mark the eye must follow mid-gesture is a
/// statement, and the engaged detent already prices a 2 px statement at
/// full-ish ink.
#[must_use]
pub const fn insert_line_ink(p: &Palette) -> Color {
    p.paper_dim
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
/// Opacity of a glyph on a live control **at rest**.
///
/// **0.57, where it was 1.00**, and the number is measured rather than chosen
/// (`docs/design/04-fluidity.md`). Once the chrome around an icon button goes
/// (see [`transport`]) the ink is the whole control, so the ink has to carry
/// the whole state ladder — and a glyph at full paper is a *pressed* control's
/// weight, not a resting one. At 0.57 the resting bar recedes into the room the
/// way every other quiet reading in the product does, and the ladder below it
/// has somewhere to go.
///
/// The step this leaves for hover is a 3.3× luminance jump, against the zero
/// the bar had before: `transport`'s hover and press states were dead code for
/// these four controls, because the mark is a rasterised sprite and a `button`
/// style's `text_color` never reaches an `image`.
pub const GLYPH_OPACITY: f32 = 0.57;
/// Opacity of a glyph under the pointer — **the top of the ink ladder**.
///
/// A 3.3× step in relative luminance over [`GLYPH_OPACITY`] (0.238 → 0.777),
/// against the *zero* the bar used to have. This one number is most of what
/// makes the transport feel like it answers.
///
/// # It was unreachable, and now it is not
///
/// iced 0.13 hands a widget its own hover status inside a *style* function, and
/// a style function cannot reach into the `image` widget that draws the sprite —
/// the glyph is a sheet rasterised once, inked at raster time, so
/// [`transport`]'s `text_color` was **dead code** for every icon button in the
/// product and the mark was byte-identical at rest and under the pointer
/// (`docs/design/04-fluidity.md` §3.1 finding 2). All the hover feedback there
/// was came from the button's own ground.
///
/// ADR-0020 §2.1 closes it: each icon button reports its own crossings with a
/// `mouse_area`, the shell holds one [`crate::motion::Control`] id, and the
/// image's opacity — which *is* reachable — carries the state. The same
/// mechanism the queue rows' ✕ and the shelf's tiles already use.
pub const GLYPH_OPACITY_HOVER: f32 = 1.0;
/// Opacity of a glyph while the pointer is held down on it.
///
/// **0.75** — the ladder's fourth rung, and it removes light rather than adding
/// a fill. A press that darkens the ground is unreadable against a near-black
/// wall (the old pressed state painted the button in the bar's own colour, so it
/// read as a *hole*); a press that takes the mark itself down a step is
/// unambiguous, and it is the only reading available to a control whose whole
/// paint is one sprite.
///
/// Between rest and hover on purpose: a press has to be visibly *not* the
/// resting state, and visibly not the "you may" of a hover either.
pub const GLYPH_OPACITY_PRESS: f32 = 0.75;
/// Opacity of a glyph while its command is in flight: the whole of the
/// pending affordance. A control that dims a little and comes back changes
/// no size, no shape, and no meaning — which is the difference between an
/// affordance and the flash the bottom bar used to have (the argument, and
/// the measured round trip, are in [`crate::player`]'s module docs).
///
/// **0.42**, following [`GLYPH_OPACITY`] down: it is a fraction of the resting
/// ink, not an absolute, or a quieter rest would have made "waiting" *louder*
/// than "ready".
pub const GLYPH_OPACITY_PENDING: f32 = 0.42;
/// Opacity of a glyph on a control that genuinely cannot act — no engine,
/// or nothing queued.
///
/// **0.28.** It was 0.45 against a resting 1.00, which sounds like a wide gap
/// and was not one: `transport` painted rest and disabled with the *same*
/// plinth fill and the same hairline, so the two states measured 1.10 : 1
/// against each other and a listener could not tell a dead Previous from a live
/// one (`docs/design/04-fluidity.md`). With the chrome gone the ink is the only
/// difference there is, so it has to be a real one — half the resting weight,
/// and below the pending weight, so the three readings of a control are three
/// readings.
pub const GLYPH_OPACITY_DISABLED: f32 = 0.28;

/// The lane the bottom bar keeps **above and below** its transport row
/// (logical px) — [`GAP_MD`], and the same on both sides.
///
/// This is the composition audit's defect 2, expressed as one number. The bar
/// used to be a row of three zones each `align_y(Center)`, which centres them as
/// *blocks*: the centre column was `TRANSPORT_HIT + GAP_SM + SEEK_ROW_H`, so it
/// set the bar's height and hung the transport at the **top** of it — 22.5 px
/// above the bar's own mid-line, with the seek groove 27.5 px below it and the
/// volume rail 6.5 px below that. Seven mark-lines in a 102 px band, and the
/// mid-line carried nothing.
///
/// The fix was to make the band symmetric about the transport rather than to
/// nudge anything, and it survives every re-derivation of the band unchanged in
/// principle: the lead is whatever is left of the band once the transport has
/// taken its 32, halved. It is **derived, never chosen** — which is what makes
/// law L4 true by construction rather than by an assertion somebody has to keep
/// re-checking — and it is what [`NEEDLE_HIT`] is bounded by.
///
/// At [`BAR_CONTENT_H`] 80 it is [`GAP_XL`] 24.
pub const BAR_LEAD: f32 = (BAR_CONTENT_H - TRANSPORT_HIT) / 2.0;

/// The bar's tallest zone: the now-playing stack's three line boxes — 20 · 16 ·
/// 20 = **56** (logical px).
///
/// Named because it is what the band is *derived from*. The title's line box,
/// the artist's, and the ambient continuation's, all reserved whether or not
/// they say anything, so the block is this tall in every state rather than in
/// its tallest one.
pub const NOW_PLAYING_H: f32 = LINE_BODY + LINE_META + CONTINUATION_H;

/// **The sounding record's sleeve in the bar** (logical px) — 52, square.
///
/// It is fitted to the band rather than the band to it, which is the whole of
/// the constraint: [`BAR_CONTENT_H`] 80, [`BAR_ZONE_LEAD`] 12 either side and
/// [`NOW_PLAYING_H`] 56 between them are all unchanged, and 52 is the largest
/// square on the 4 px lattice that sits inside the 56 with air left over. No
/// proportion of the bar is re-derived and no slot is removed
/// (the bar's ratchet: *a slot may be added, none removed*).
///
/// It joins the now-playing block's **existing** hit target rather than
/// standing beside it — the block is already the labelled control that takes
/// you back to what is playing, and a picture next to a link would be two
/// objects where the design asks for one. With no artwork the cover is simply
/// absent and the block is the pixels it has always been.
pub const BAR_COVER: f32 = 52.0;

/// **The lead a band keeps around its tallest zone** (logical px) — the
/// breathing rule, stated once and applied to both bars.
///
/// > A band's content may not touch the band's edges. The lead is a **named
/// > gap** on each side — never a ratio — because a ratio is not reachable on
/// > the 4 px lattice for two bands of different content heights, and a lead
/// > that is not on the lattice is law L2 broken to make law "proportion" true.
///
/// The top bar leads its 32 px control row by [`TOP_BAR_PAD_V`] `GAP_SM` 8. The
/// bottom bar leads its 56 px *type block* by [`GAP_MD`] 12, one rung more,
/// because a hit box already carries its own internal padding and a stack of
/// line boxes carries only its leading — 3.5 px above the title's ink and 2.5
/// below the continuation's, which is what made the 56 px band read as cramped
/// however correct each token in it was.
///
/// | | tallest zone | lead | band | ink-to-band |
/// |---|---:|---:|---:|---:|
/// | top bar | 32 | 8 | 48 | 0.67 |
/// | bottom bar | 56 | 12 | 80 | 0.70 |
pub const BAR_ZONE_LEAD: f32 = GAP_MD;

/// Height of the bottom bar's content band — **80**; the bar is this plus its
/// hairline, **81**, and the needle takes 2 px more at the window's edge.
///
/// # It is two hangs, and that is the whole argument
///
/// `2 × HANG`. [`HANG`] is the product's one structural unit — the window
/// gutter, the wall label's height, the shelf header's band, the clear wall
/// between two rows — so the bar is measured in the same unit as the collection
/// it sits under rather than in a number of its own. Every other figure in the
/// band falls out of it and every one of them is a token that already existed:
///
/// ```text
///   1  hairline
///  12  BAR_ZONE_LEAD (GAP_MD)   ─┐
///  20  the title's line box      │ NOW_PLAYING_H 56 — the tallest zone
///  16  the artist's line box     │
///  20  the continuation's lane  ─┘
///  12  BAR_ZONE_LEAD (GAP_MD)
/// ---
///  81   + 2 px needle, flush on the bottom edge  →  83 of bottom furniture
/// ```
///
/// The transport's own lead is the same band read from the other side:
/// [`BAR_LEAD`] = (80 − 32) / 2 = [`GAP_XL`] 24, so **the band's mid-line is
/// the transport's centre line** (law L4) and the type block's middle lane is
/// on the same line.
///
/// # Why it grew, having just shrunk
///
/// Step 10 took the band to **56** and the bar to 57, which was correct in
/// every part and wrong as a proportion: `NOW_PLAYING_H` is *also* 56, so the
/// three lines of type filled the band edge to edge with no air at all. The
/// owner's reading — *"proportion is becoming an issue e.g. bottom bar is too
/// short"* — is that arithmetic seen rather than computed.
///
/// The three lanes were re-argued before the band was: the continuation line
/// (`then 2 albums · 1:58:00 left`) **earns its lane and keeps it**, because
/// ADR-0022 made the queue a place — reading what is next used to cost a
/// popover that reflowed nothing and now costs leaving the wall, so the ambient
/// line is the only free reading of the queue baz has, and it got *more*
/// valuable at exactly the moment the bar got shorter.
///
/// # What it costs, stated
///
/// | | before the needle | step 10 | now |
/// |---|---:|---:|---:|
/// | band | 104 | 56 | **80** |
/// | bottom furniture | 105 | 59 | **83** |
/// | of an 860 px window | 12.2 % | 6.9 % | **9.7 %** |
/// | of a 1080 px window | 9.7 % | 5.5 % | **7.7 %** |
/// | the collection's share at 1280 × 860 | 82.1 % | 87.4 % | **84.7 %** |
///
/// The needle's work bought the wall 46 px; this spends 24 of them and keeps
/// 22. That is the minimum that buys real air on the 4 px lattice: the next
/// step down, a band of 72, leads the type block by [`GAP_SM`] 8 and is
/// defensible, and the step below *that* — 64, an [`GAP_XS`] 4 px lead — is not
/// air at all. Two hangs is chosen over 72 because it is the only one of the
/// three whose every figure is a token the composition already uses.
///
/// # 81 is reachable and 80 was never going to be
///
/// A bar is `2ℓ + TRANSPORT_HIT + 1` for a lead `ℓ`, which is **odd** for every
/// integer `ℓ` — the hairline is odd and everything else is doubled. Step 10's
/// heading said 58 and shipped 57 for exactly this reason; this one says 81 and
/// means it.
pub const BAR_CONTENT_H: f32 = NOW_PLAYING_H + 2.0 * BAR_ZONE_LEAD;

/// Vertical padding of the top bar and of the Settings place's header strip
/// (logical px) — the two strips that have to be one frame.
pub const TOP_BAR_PAD_V: f32 = GAP_SM;

/// Height of the top bar, hairline included — **49**.
///
/// `2 × TOP_BAR_PAD_V + TRANSPORT_HIT + 1`. It is stated here rather than
/// estimated in `app.rs`, which is what the audit's §2.1 aside asked for: that
/// constant was 56 against a drawn 53, and it is the virtualizer's pre-first-
/// resize viewport estimate, so an estimate that disagreed with the drawing by
/// three pixels was three pixels of shelf mis-virtualized on the first frame.
pub const TOP_BAR_H: f32 = 2.0 * TOP_BAR_PAD_V + TRANSPORT_HIT + 1.0;

/// **Strip width** below which the Library strip splits into its two lines
/// (logical px) — **824**, an exact sum rather than a rounded one.
///
/// It was 960: the tenants summed to 958 with the well at its 200 px floor and
/// the seam was declared on the next lattice step up. Three of those tenants
/// have since left, and a fourth moved. The `Playlists` door went with the
/// returns lane (ADR-0030 §5), giving back its 64 px and the [`GAP_XL`] beside
/// it — 88 — which took 960 to 872. Then, on 2026-08-10 and both on the owner's
/// decision, **`Pull` was removed** and **`Shuffle` moved to the now-playing
/// bar** as a property of the player, taking
/// [`crate::views::top_bar::ACTS_W`] from 182 to 88 and this seam from 872 to
/// 778. A split declared where the line still fits would put the strip on two
/// rows for controls it no longer draws.
///
/// The row of words has been **six** twice, and cost a different number each
/// time — which is why it is measured each time. `ARTISTS` joined it beside a
/// first key relabelled `A–Z`, taking [`crate::views::top_bar::KEYS_W`] from
/// 314 to 368 and this seam to 832; the sixth word then became the first key's
/// own grouping (ADR-0035) and both movements reversed, back to 778. The row
/// is six again now that **`A–Z` is a key of its own, first in the row**
/// (ADR-0035's third amendment) — but `A–Z` is a shorter word than `ARTISTS`,
/// so `KEYS_W` is 360 rather than 368 and the seam is **824** rather than 832.
/// The seam follows the tenants **up** as readily as it follows them down,
/// which is the property that makes it arithmetic rather than a judgement.
///
/// **The well left too** (ADR-0030's search amendment), but it left only where
/// the lane can hold it, which is exactly the widths at which this seam cannot
/// be reached: see [`strip_holds_the_well`]. So the split's own arithmetic
/// still counts the well, and 824 is the sum with it in.
///
/// The number this is compared against is the **strip's** width — the window
/// less the returns lane — never the window's. See [`top_bar_h`], which is the
/// one place that resolution happens.
pub const TOP_BAR_SPLIT: f32 = 824.0;

/// The strip's floor, and the window's sensible minimum (logical px) —
/// **600**, from the two-line regime's own arithmetic (doc 10 §4.3): the
/// library line's tenants summed to exactly this. Below it nothing further
/// collapses — there is no third regime, and a proposal that needs one has
/// outgrown the strip (doc 10 §8).
///
/// **It no longer sits exactly on that sum, and it does not follow it.** Both
/// draw words left the acts cluster on 2026-08-10 and the library line came to
/// 506; the arrangement row's sixth word took it to 560, then back to 506, and
/// `A–Z` (ADR-0035's third amendment) now puts it at **552**. The floor stays
/// at 600 through all of it because it is *also* the window's sensible
/// minimum, and a window minimum that moved every time a word joined or left a
/// strip would be a promise about the smallest usable baz that was really a
/// statement about the strip's current population. The slack — 48 px, and it
/// is the first thing to read if a seventh word is ever proposed — is recorded
/// in `the_strip_holds_its_tenants_at_the_single_line_floor`, which asserts the
/// line fits under the floor rather than meeting it.
pub const TOP_BAR_FLOOR: f32 = 600.0;

/// Height of the two-line Library strip, hairline included — **89**
/// (doc 10 §4.3): the strip's one vertical lead above, between and below its
/// two 32 px lines, then the hairline. `8 + 32 + 8 + 32 + 8 + 1`.
///
/// A pair of tokens and a breakpoint rather than a measurement: the app's
/// layout estimate reads the **resolved** height ([`top_bar_h`]), because
/// the virtualizer's pre-first-scroll viewport is derived from this number
/// and an estimate that disagreed with the drawing has already cost the rail
/// its capacity math once.
pub const TOP_BAR_2LINE_H: f32 = 3.0 * TOP_BAR_PAD_V + 2.0 * TRANSPORT_HIT + 1.0;

/// **Whether the Library strip carries the search well** — true exactly where
/// the returns lane cannot hold it.
///
/// The owner's decision (ADR-0030's search amendment): *"the search should
/// really be in the sidebar"*. The lane is where the query lives, and it can
/// hold a field at [`SIDEBAR_W`] and reach one at [`SIDEBAR_RAIL_W`] — the
/// rail's magnifier opens the lane onto the caret. Below [`SIDEBAR_FLOOR`]
/// the lane is a rail that *cannot* open ([`sidebar_can_expand`]), so there is
/// nothing for a magnifier there to lead to, and the well goes back to the
/// strip in the form doc 10 §4.1 drew for it.
///
/// **One home at a time, and the breakpoint is the lane's own.** The defect
/// the owner named — *"the design does not match properly"* — was two surfaces
/// both carrying the frame's identity; a well drawn in both would be that
/// defect with an extra field.
#[must_use]
pub fn strip_holds_the_well(window_w: f32) -> bool {
    !sidebar_can_expand(window_w)
}

/// The Library strip's resolved height: [`TOP_BAR_H`] in the single-line
/// regime, [`TOP_BAR_2LINE_H`] where the strip splits.
///
/// The one function every consumer of the strip's height must read — the
/// strip's own view and `app.rs`'s viewport estimate both — so the two regimes
/// cannot disagree about where the seam is.
///
/// **It takes the window and the lane's state, not a width**, and that is a
/// correction. The strip is drawn at `App::body_width` — the window less the
/// returns lane — while this function was still being handed the *window*, so
/// between a 1000 px and a 1056 px window with the lane open the strip drew
/// two lines and the virtualizer's viewport estimate assumed one. Taking the
/// same two facts the composition takes makes the pair impossible to disagree.
///
/// Where the well has left the strip ([`strip_holds_the_well`]) the split
/// cannot be reached at all: the narrowest strip in that regime is
/// `SIDEBAR_FLOOR − SIDEBAR_W` = 720, against a well-less single line of 608.
/// That is asserted, which is what makes this branch a fact rather than a
/// hope.
#[must_use]
pub fn top_bar_h(window_w: f32, lane_open: bool) -> f32 {
    let strip = (window_w - sidebar_w(window_w, lane_open)).max(0.0);
    if strip_holds_the_well(window_w) && strip < TOP_BAR_SPLIT {
        TOP_BAR_2LINE_H
    } else {
        TOP_BAR_H
    }
}

/// Vertical padding that makes a text well exactly [`TRANSPORT_HIT`] tall.
///
/// iced lays a `text_input` out as its padding plus one line box — the 1 px
/// border is drawn *inside* those bounds and adds nothing — so the padding is
/// the control height minus the line box, halved. **6.**
///
/// The `− 2.0` for the border is the mistake the shipped build made and the
/// reason the well stood 30 px against a published floor of 32; it is measured
/// off the render rather than reasoned about, in
/// `docs/design/impl/composition/`. Both wells baz draws — the search field at
/// [`LINE_BODY`] and the first-run folder field at [`LINE_EMPHASIS`] — take a
/// 20 px line box, so there is **one** number rather than two that would drift:
/// the search well used to stand 30 px and the first-run well 40 (law L7).
pub const WELL_PAD_V: f32 = (TRANSPORT_HIT - LINE_BODY) / 2.0;
/// Width of the bottom bar's centre column: three [`TRANSPORT_HIT`] squares and
/// the two [`GAP_SM`] gaps between them — **112**.
///
/// It was `SEEK_W + 2 × (STAMP_W + GAP_SM)` = 380, because the column held a
/// timestamp, a 260 px groove and a timestamp, and the transport centred itself
/// over that. With the seek row gone the column *is* the transport row, so the
/// centre is the buttons' own centre by construction rather than by a shared
/// width, and the 268 px the column gives up go to the two flanking zones — the
/// left one being the zone the audit found clipping below ~900 px.
pub const TRANSPORT_W: f32 = 3.0 * TRANSPORT_HIT + 2.0 * GAP_SM;

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
pub const SETTING_NOTE_H: f32 = 2.0 * LINE_META;

/// The lane a scrolling list keeps clear for its scrollbar: padding on the
/// right of the list's contents and nowhere else.
///
/// Reserved **whether or not the list currently overflows**, on the same
/// principle as [`STAMP_W`] and [`SIGNAL_W`]: a gutter that appeared with
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

/// **The wall's scrollbar: 4 px, and it reserves its own lane.**
///
/// The one thing an index rail cannot do is take you to the end. The rail
/// jumps to *shelves* by group key — a letter, a year, a genre — and "the
/// bottom" is not one of those: under `ARTIST` the last shelf may be `Z` or
/// may be `#`, and under a filter it is whatever survived. So the wall now
/// draws a bar, at the narrowest width that is still a handle.
///
/// *The owner's decision, 2026-08-09* — *"can we allow there to be a scroll
/// bar for any view? Just a very minimal scroll bar because otherwise, it's
/// hard to just jump to the end"*. the product's *two vertical strips
/// may not do one job* entry is rewritten to record it. Every **other** list
/// in baz already had one ([`list_scrollbar`]); the wall was the only surface
/// without, so "any view" is this view.
///
/// # It takes its width from the wall, not from the rail's clearance
///
/// A `Scrollbar` with `spacing` set makes iced reserve `width + 2 × margin +
/// spacing` of right padding *inside* the scrollable
/// (`iced_widget-0.13.4/src/scrollable.rs:422–436`); without it the bar is
/// drawn over the content. Overlaying was the tempting move — there is
/// [`INDEX_CLEARANCE`] 8 of empty lane to its right — but the wall's block is
/// centred and is not guaranteed to leave 4 px of slack at every width, so a
/// cover's right edge would sometimes be under the bar. Reserving costs the
/// grid 4 px, which it absorbs the way it absorbs every other width, and
/// [`INDEX_LANE_W`] is unchanged: the rail's algebra, and every test over it,
/// is untouched.
///
/// This is the bar **every list-shaped surface with a quiet bar** uses — the
/// returns lane, `Home`, an artist's page. The *wall* uses
/// [`shelf_scrollbar`], which is this bar with the rail's lane added to the
/// reservation so the bar itself lands on the window's edge.
#[must_use]
pub fn wall_scrollbar() -> scrollable::Scrollbar {
    scrollable::Scrollbar::new()
        .width(WALL_SCROLLBAR_W)
        .scroller_width(WALL_SCROLLBAR_W)
        .margin(SCROLLBAR_MARGIN)
        // Reserve rather than overlay; the gap to the rail's ink is
        // `INDEX_CLEARANCE`'s job and stays its job.
        .spacing(0.0)
}

/// **The wall's own bar: [`wall_scrollbar`] with the index rail's lane added
/// to what it reserves**, so the bar is drawn on the *window's* right edge and
/// the rail hangs inboard of it.
///
/// # The defect this fixes
///
/// The bar shipped at the right edge of the wall's *scrollable*, which is the
/// structurally honest place for it and the wrong place to look. Measured on a
/// 1280 × 860 frame with the lane open: the bar occupied x 1168–1171 and the
/// rail's letters x 1233–1239, so 108 px of window stood outboard of a bar
/// that had nothing but the rail's sparse type in it. The owner: *"scroll bar
/// is in a strange location… it seems to have padding on the right"*. He is
/// describing a real composition, not misreading one.
///
/// # Why the edge, and why this way round
///
/// The returns lane already answered this question, in his words — *"the
/// scrollbar should be at the edge of it"* ([`crate::views::lane`]): the rows
/// carry the lane's gutter so the bar can ride the surface's own edge. **The
/// content keeps its inset; only the bar reaches the edge.** The wall's
/// surface is the window, so the same move puts the bar on the window's edge
/// and moves nothing else — the rail's lane, its letters, the density detents
/// at its foot and every number in [`INDEX_LANE_W`] are exactly where they
/// were.
///
/// # How iced is made to do it
///
/// `Scrollbars::new` puts the bar at `bounds.x + bounds.width −
/// (width + 2 × margin)` — the far right of the scrollable's **outer** bounds,
/// which ignores `spacing` entirely (`scrollable.rs:1583–1602`). So the
/// scrollable is given the whole body width, the rail is stacked *under* it
/// right-aligned, and `spacing` is [`INDEX_LANE_W`]: the content is confined
/// to the same [`WALL_RESERVE`]-less width it always had, and the 4 px the bar
/// occupies fall in the rail's own window gutter, where there is no ink.
///
/// The rail stays *under* the bar in the stack rather than over it because
/// iced hands the topmost layer the pointer first, and a rail on top would own
/// the 4 px the bar is drawn in — a bar nobody can grab. Under it, the rail
/// answers everywhere except those 4 px.
#[must_use]
pub fn shelf_scrollbar() -> scrollable::Scrollbar {
    wall_scrollbar().spacing(INDEX_LANE_W)
}

/// What the wall's scrollable takes off its own right edge before the grid
/// gets what is left: the bar's 4 px **and** the index rail's lane —
/// **112**.
///
/// One number for one reservation. [`shelf_scrollbar`] spends it as iced's
/// `width + 2 × margin + spacing`, and `Shelf::grid_size` subtracts it from
/// the scrollable's measured outer bounds; `Shelf::grid_width` reaches the
/// same total the long way, through its own two terms.
pub const WALL_RESERVE: f32 = WALL_SCROLLBAR_W + 2.0 * SCROLLBAR_MARGIN + INDEX_LANE_W;

/// The width of the wall's scrollbar — **4**, the [`RAIL`] width, which is the
/// narrowest mark in the product that is still something to aim at.
///
/// Deliberately narrower than [`SCROLLBAR_W`] 10, which every list keeps: a
/// list's bar is its only readout of how much list there is, and the wall's is
/// a *second* readout beside a rail that already says where you are. It is a
/// handle for the one gesture the rail has no answer to, and it is drawn in
/// the same hairline as every other edge in the room.
pub const WALL_SCROLLBAR_W: f32 = RAIL;

const _: () = {
    // The reservation is the bar's lane plus the rail's, and nothing else: the
    // 4 px at the window's edge and the 108 the rail hangs in.
    assert!(WALL_RESERVE == WALL_SCROLLBAR_W + INDEX_LANE_W);
    assert!(WALL_RESERVE == 112.0);
    // The bar fits in the rail's window gutter with room to spare, which is
    // what lets it sit at the edge without touching a letter: the rail's ink
    // stops at `W − HANG`, the bar starts at `W − WALL_SCROLLBAR_W`.
    assert!(WALL_SCROLLBAR_W < HANG);
    // …and the rail keeps all but those 4 px of its hit lane (the Fitts band).
    assert!(INDEX_LANE_W - WALL_SCROLLBAR_W > INDEX_CLEARANCE + INDEX_W);
};

/// A list's scrollbar: no trough, and a scroller in the same hairline the room
/// uses for every other edge, one step firmer while it is being driven.
///
/// Quiet on purpose. A scrollbar is a *readout* of how much list there is, and
/// baz's chrome recedes so the covers and the type carry the interface; the
/// stock blue-grey iced draws otherwise is the one thing on screen that is not
/// from this palette.
#[must_use]
pub fn scrollbar(p: &Palette, on: Color, status: scrollable::Status) -> scrollable::Style {
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
                p.hairline_strong(on)
            } else {
                p.hairline(on)
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
/// and the lamp is reserved (see [`segment`]); a checked box says so with
/// the surface step and the hairline the room already uses for "selected".
#[must_use]
pub fn check(p: &Palette, status: checkbox::Status) -> checkbox::Style {
    let (background, border_color) = match status {
        checkbox::Status::Active { is_checked } => (
            if is_checked { p.plinth_lit } else { p.recess },
            p.hairline_strong(p.plinth_lit),
        ),
        checkbox::Status::Hovered { .. } => (p.plinth_lit, p.hairline_strong(p.plinth_lit)),
        checkbox::Status::Disabled { is_checked } => {
            let box_ground = if is_checked { p.plinth } else { p.recess };
            (box_ground, p.hairline(box_ground))
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
///
/// **A value, not a branch in a layout.** Every state of an icon button that
/// this function can see resolves to one number on one ramp, which is what
/// lets a transition interpolate it later without any of the callers changing
/// (`docs/design/04-fluidity.md`). The two readings it *cannot* see —
/// [`GLYPH_OPACITY_HOVER`] and the pressed reading beside it — are on the same ramp
/// and named for the same reason.
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

/// **The complete ink ladder for one icon button**, with the pointer's part of
/// it `hover` of the way in.
///
/// [`glyph_opacity`] is this function at `hover` 0: the three readings a control
/// has *without* a pointer. What ADR-0020 §2.1 adds is the two the pointer
/// makes, and the ramp between them — 0.57 rest / 1.00 hover / 0.75 press /
/// 0.28 disabled, with `hover` supplied by a 90 ms [`crate::motion::Tween`] so
/// the step is a fade rather than a flicker.
///
/// Three properties worth stating, because each is a decision:
///
/// - **A dead control is dead at every value of `hover`.** The pointer crossing
///   a Previous that cannot act must not lift it: an affordance that answers a
///   hover is claiming it can be pressed.
/// - **A pending control holds still too.** Waiting is a fact about a command in
///   flight, and the ink is the whole of how it is stated
///   ([`GLYPH_OPACITY_PENDING`]); letting a hover overwrite it would make
///   "waiting" and "ready" the same reading in the one moment they differ.
/// - **The press changes where the ramp is going, never where it is.** Pressing
///   mid-fade re-aims the same tween at [`GLYPH_OPACITY_PRESS`] rather than
///   jumping, so a press is continuous with the hover that preceded it.
#[must_use]
pub fn glyph_ink(enabled: bool, pending: bool, hover: f32, pressed: bool) -> f32 {
    let resting = glyph_opacity(enabled, pending);
    if !enabled || pending {
        return resting;
    }
    let peak = if pressed {
        GLYPH_OPACITY_PRESS
    } else {
        GLYPH_OPACITY_HOVER
    };
    hover.clamp(0.0, 1.0).mul_add(peak - resting, resting)
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

/// A shelf tile's button chrome: **nothing, in every state**.
///
/// This is ADR-0017 step 14, and it is the whole of the shelf's first rule:
/// *the shelf contains exactly two kinds of thing, artwork and type*
/// (`.interface-design/system.md` §1.2). A card behind a sleeve is a third
/// kind, and this function was drawing one on hover and two steps' worth on
/// selection. So the background, the border and the radius all go, and the
/// tile's state vocabulary moves to a **rule under the wall label** —
/// [`tile_rule`], drawn at art width by [`crate::views::shelf`], 1 px
/// [`Palette::hairline_strong`] hovered against 2 px [`Palette::paper_faint`]
/// selected.
///
/// The parameters survive because the caller still has the questions, and
/// because what is asserted below is that this function answers all of them
/// with nothing: no state of a tile may quietly re-grow a surface under a
/// cover.
///
/// **Radius 0**, where the tile had a `RADIUS_TILE` of 10 — that token is
/// deleted, artwork is always square, and there is no longer any rectangle
/// here to round.
#[must_use]
pub fn tile(p: &Palette, _status: button::Status, _selected: bool) -> button::Style {
    button::Style {
        background: None,
        text_color: p.paper,
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 0.0.into(),
        },
        shadow: Shadow::default(),
    }
}

/// The mark a tile's state makes: a rule under its wall label, at art width,
/// in the ink and thickness the state calls for.
///
/// The shelf's *only* state vocabulary, and deliberately a poor relation of a
/// border: it is under the type, not around the work, so rule 2 of the
/// direction — *nothing is ever drawn on top of a sleeve*, and nothing is
/// drawn around one either — holds in a screenshot as well as in prose.
///
/// Hover is 1 px [`Palette::hairline_strong`] (the room's ink at 15 %);
/// selection is [`SELECTION_EDGE`] 2 px of [`Palette::paper_faint`]. That is a
/// 2× thickness and a ~4× ink step apart, which is what the audit's *"hover and
/// selection are nearly the same mark"* finding asked for and what one surface
/// step plus a hairline could never give it. Neither is the accent: selecting a
/// record is not playing one.
///
/// The lane the rule sits in is reserved at [`SELECTION_EDGE`] whatever the
/// state, so the thick mark, the thin one and no mark at all occupy the same
/// pixels and a pointer crossing the wall moves nothing.
/// # The hover half of it fades (ADR-0020 §2.3)
///
/// `hover` is a 90 ms [`crate::motion::Tween`], and **only the ink moves**: the
/// rule is 1 px from the first frame of the fade to the last, because a rule
/// whose *thickness* interpolated would spend most of the transition asking the
/// rasteriser to draw two-thirds of a pixel, which is a blur rather than a thin
/// line. A mark that is arriving should look like the mark it will be, quietly.
///
/// Selection does not fade. It is the result of a click — a decision, not a
/// passage — and it wins over the hover ink at every value of `hover`, exactly
/// as it did when hover was a boolean.
#[must_use]
pub fn tile_rule(p: &Palette, hover: f32, selected: bool) -> container::Style {
    let ink = if selected {
        p.paper_faint
    } else if hover > 0.0 {
        p.hover_rule(p.wall, hover)
    } else {
        Color::TRANSPARENT
    };
    container::Style {
        background: Some(Background::Color(ink)),
        ..container::Style::default()
    }
}

/// How thick a tile's [`tile_rule`] is drawn, in the [`SELECTION_EDGE`] lane
/// reserved for it (logical px).
///
/// A whole number in every frame, fading or not — see [`tile_rule`].
#[must_use]
pub fn tile_rule_h(hover: f32, selected: bool) -> f32 {
    if selected {
        SELECTION_EDGE
    } else if hover > 0.0 {
        1.0
    } else {
        0.0
    }
}

/// A tile's caption ink, `hover` of the way from its resting weight to its
/// hovered one.
///
/// The other half of ADR-0017 step 14's hover state, and the half that still
/// reads when the rule is the thing your own hand is over: the artist line lifts
/// one rung of the ink ramp. It lifts *with* the rule now rather than a frame
/// before it, because both read the same tween.
///
/// Both ends are on the room's one ink ramp ([`Palette::paper`] and its
/// relatives are one board at four levels of light), so every point between them
/// is on that ramp too — the mixture cannot land on a colour the room does not
/// own, and it cannot land below the floor either end clears.
#[must_use]
pub fn caption_ink(p: &Palette, hover: f32) -> Color {
    Palette::mix(p.paper_faint, p.paper_dim, hover)
}

/// The artwork's backing: a [`Palette::recess`] square the sleeve sits in,
/// and — when the album is the one sounding — the lamp's halo around it.
///
/// **No contact shadow.** It was the room's shadow at 45 % offset 3 px, and it
/// composited to a 1.04 : 1 step over the wall: invisible, and paid for on
/// every tile of every row. Deleting it is what leaves the halo meaning
/// something, because a glow is only a glow next to sleeves that have nothing.
///
/// The blur is [`HALO_BLUR`] 24, up from 16 (`.interface-design/system.md`
/// §4): the halo is now the only light in the shelf rather than one of two
/// shadows, and at 16 it read as a rim rather than as a room's worth of lamp.
///
/// # The lamp warms rather than switching (ADR-0020 §2.5)
///
/// `warmth` is a 200 ms **linear** [`crate::motion::Tween`] — 0 for a sleeve
/// that is not sounding, 1 for one that is, and a filament coming up in between.
/// It is the light's *strength*: the blur is [`HALO_BLUR`] in every frame and
/// the sleeve does not move a pixel, so this is the one animation in the product
/// that touches the accent and it still states nothing but playback truth.
#[must_use]
pub fn sleeve(p: &Palette, warmth: f32) -> container::Style {
    let shadow = if warmth > 0.0 {
        Shadow {
            color: p.lamp_glow_at(warmth),
            offset: Vector::ZERO,
            blur_radius: HALO_BLUR,
        }
    } else {
        Shadow::default()
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

/// **An icon button: the glyph, and nothing else.**
///
/// Every icon-only control in baz takes this style — the three transport
/// glyphs and the mute speaker in the bar, every layer's dismissal ✕, a queue
/// row's removal ✕, the settings' `−`/`+` pair — which is why it is one
/// function rather than four that would drift.
///
/// It used to paint a [`Palette::plinth`] card with a hairline border at rest,
/// and that was the loudest wrong thing in the product: five little bordered
/// boxes strung along a bar whose entire thesis is that chrome recedes. The
/// mark a listener is looking for is the triangle, not the box around it, and
/// a box drawn at rest states nothing that the glyph does not.
///
/// So at rest there is **no background and no border**. Hover is a faint ink
/// wash ([`Palette::ink_wash`], the room's ink at 6 %), press a touch stronger
/// ([`Palette::ink_wash_press`], 10 %), and disabled is ink alone — the glyph's
/// own opacity carries that ([`glyph_opacity`]), because a wash under a dead
/// control would be a control offering itself.
///
/// **The target does not move.** [`TRANSPORT_HIT`] 32 is an accessibility
/// floor asserted in `a_transport_button_is_a_square_target_around_its_glyph`,
/// and it is a property of the *button*, not of its paint: what has gone is
/// the chrome, not the square. The border is 1 px in every state and merely
/// transparent in three, and the reason is the toolkit's: iced draws a border
/// inside the widget's bounds, so a border that appeared on hover would move
/// the glyph under the pointer by a pixel, in the bar, where nothing may move.
/// [`now_playing_text`] keeps the same rule by the opposite construction — no
/// edge at all rather than a transparent one — and says why there.
#[must_use]
pub fn transport(p: &Palette, on: Color, status: button::Status) -> button::Style {
    let (background, text_color) = match status {
        button::Status::Hovered => (p.ink_wash(on), p.paper),
        button::Status::Pressed => (p.ink_wash_press(on), p.paper),
        button::Status::Disabled => (Color::TRANSPARENT, p.paper_muted),
        button::Status::Active => (Color::TRANSPARENT, p.paper),
    };
    button::Style {
        background: Some(Background::Color(background)),
        text_color,
        border: Border {
            color: Color::TRANSPARENT,
            width: 1.0,
            radius: RADIUS_CTRL.into(),
        },
        shadow: Shadow::default(),
    }
}

/// **A word that is a control**: `Settings`, `‹ Library` — navigation set in
/// type rather than drawn as a button.
///
/// The two of them are the only text-labelled controls in baz that are neither
/// a segment nor the primary action, and they were borrowing `panel_toggle`,
/// which is a *selectable* control's paint: it raised a card under the pointer
/// and had a lit "on" state that navigation can never be in (the place it leads
/// to replaces the bar the control is in, so there is no frame where it could
/// be both lit and visible).
///
/// So they get the icon button's treatment with a label instead of a glyph: no
/// ground and no edge at rest, [`Palette::ink_wash`] under the pointer, and the
/// word itself lifting from [`Palette::paper_dim`] to [`Palette::paper`].
/// Chrome recedes; the word is the control. Geometry is identical in all four
/// states.
#[must_use]
pub fn word_button(p: &Palette, on: Color, status: button::Status) -> button::Style {
    let (background, text_color) = match status {
        button::Status::Hovered => (p.ink_wash(on), p.paper),
        button::Status::Pressed => (p.ink_wash_press(on), p.paper),
        button::Status::Disabled => (Color::TRANSPARENT, p.paper_muted),
        button::Status::Active => (Color::TRANSPARENT, p.paper_dim),
    };
    button::Style {
        background: Some(Background::Color(background)),
        text_color,
        border: Border {
            color: Color::TRANSPARENT,
            width: 1.0,
            radius: RADIUS_CTRL.into(),
        },
        shadow: Shadow::default(),
    }
}

/// **A group key**: one of the six words the wall is arranged by — A–Z ·
/// ARTIST · YEAR · GENRE · ADDED · PLAYED (ADR-0017 §1.3, ADR-0019).
///
/// the product's standing rules refuses view-options menus outright: *no grid-size
/// picker, no list-mode toggle, no column chooser, no sort dropdown. Group
/// keys are a row of words.* So there is no menu, no dropdown, no segmented
/// control and — the part that is easy to get wrong — **no chip and no border
/// on the active one either**. A pill drawn around the live key would be the
/// dropdown's ghost: the same "this is a widget" statement, one step quieter.
///
/// The chrome here is type, and it says *active* on two axes at once so it
/// never depends on either alone:
///
/// | | ink | face |
/// |---|---|---|
/// | active | [`Palette::paper`] | [`MEDIUM`] |
/// | at rest | [`Palette::paper_faint`] | [`SANS`] |
///
/// One size for all six, one caps treatment for all six, one tracked
/// spelling for all six — the row is a single line of type in which one word
/// is lit. The ink step is `#E8E4DB` against `#888680`, which is 2.6 × the
/// luminance; the weight step is a real drawn face rather than a synthesised
/// one. Neither is colour, so *no state is signalled by colour alone* holds
/// (a standing rule of the product).
///
/// Hover and press are [`word_button`]'s wash, because a key *is* a word that
/// is a control; what this adds over `word_button` is the resting distinction
/// between the one that is in force and the four that are not, which
/// navigation has no need of.
#[must_use]
pub fn group_key(p: &Palette, on: Color, status: button::Status, active: bool) -> button::Style {
    let resting = if active { p.paper } else { p.paper_faint };
    let (background, text_color) = match status {
        button::Status::Hovered => (p.ink_wash(on), p.paper),
        button::Status::Pressed => (p.ink_wash_press(on), p.paper),
        button::Status::Disabled => (Color::TRANSPARENT, p.paper_muted),
        button::Status::Active => (Color::TRANSPARENT, resting),
    };
    button::Style {
        background: Some(Background::Color(background)),
        text_color,
        border: Border {
            color: Color::TRANSPARENT,
            width: 1.0,
            radius: RADIUS_CTRL.into(),
        },
        shadow: Shadow::default(),
    }
}

/// The band a **pinned** shelf header is drawn in: the wall's own colour, and
/// nothing else.
///
/// It is opaque, and that is its whole job — the covers of the shelf it heads
/// scroll *under* it and have to stop being drawn at the band's edge rather
/// than through it. Wall on wall is invisible as a surface, so §1.2's claim
/// survives: the shelf still contains exactly two kinds of thing, artwork and
/// type. There is no rule under it, no shadow beneath it and no step up in
/// lightness; a pinned header differs from an unpinned one in nothing a
/// screenshot can show, which is what makes the pin a *position* rather than a
/// state.
#[must_use]
pub fn shelf_header_band(p: &Palette) -> container::Style {
    container::Style {
        background: Some(Background::Color(p.wall)),
        ..container::Style::default()
    }
}

/// The primary action (Play album): a lamp **outline**, and the only control
/// in baz drawn in the accent.
///
/// It was a solid lamp rectangle — an exception the previous direction argued
/// for and this one revokes (`.interface-design/system.md` §5). Under a room
/// this quiet an amber slab was the brightest object on screen and it was
/// *not* the artwork, which inverts the one hierarchy baz has. The refusal it
/// broke is now stated without exceptions: **the accent is never an opaque
/// fill** — a ≤ 6 px mark, a 4 px rail, a 1 px line, or light.
///
/// So: a 1 px [`Palette::lamp`] border, a [`Palette::paper`] label, no fill at
/// rest, and [`Palette::lamp_wash`] at 10 % hovered / 20 % pressed. Disabled
/// keeps the geometry and drops the accent entirely: a control that cannot
/// create playback truth has no business wearing the colour that means it.
#[must_use]
pub fn primary(p: &Palette, status: button::Status) -> button::Style {
    let (background, border, text_color) = match status {
        button::Status::Active => (Color::TRANSPARENT, p.lamp, p.paper),
        button::Status::Hovered => (p.lamp_wash(p.plinth), p.lamp_bright, p.paper),
        button::Status::Pressed => (p.lamp_wash_press(p.plinth), p.lamp_deep, p.paper),
        button::Status::Disabled => (Color::TRANSPARENT, p.hairline(p.plinth), p.paper_muted),
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

/// Text inputs (search, first-run folder): an inset well with a hairline
/// edge that brightens to a paper ring on focus.
///
/// **Not lamp amber, on either the ring or the selection.** Both used to be —
/// the ring at `LAMP` 55%, the selection at [`Palette::lamp_glow`] — and since
/// the search field took focus at launch back then, the first frame baz ever
/// drew was an amber-ringed box with no music playing. A reserved signal that
/// appears
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
        text_input::Status::Focused => p.paper_ring(p.recess),
        text_input::Status::Hovered => p.hairline_strong(p.recess),
        // **No ring at rest** — the composition audit's defect 6. A 360 × 30
        // rectangle drawn around an empty field was 33.2 % of the whole top
        // bar's contrast-weighted ink, which made the two loudest objects on the
        // first frame baz draws a box around nothing and the grey instructions
        // for it. The well is a *recess*: a surface step below the wall is what
        // says "put something here", and it says it without a line. The edge
        // comes back the moment the pointer or the keyboard arrives.
        text_input::Status::Active | text_input::Status::Disabled => p.recess,
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
        selection: p.select_wash(p.recess),
    }
}

/// **The needle**: lamp amber where the queue has played, the room's faintest
/// mark where it has not.
///
/// Position is playback truth, so it earns the accent — the same rule that
/// gives the playing sleeve its halo, and the same rule the 260 px groove this
/// replaces was drawn by. Two things change with the shape:
///
/// - **The unplayed track is [`Palette::hairline`], not [`Palette::recess`].**
///   The groove was *cut into* the bar and read as a recess against the bar's
///   own plane; a 2 px line flush on the window's bottom edge has no plane
///   behind it to be cut into, so recess-on-recess would be a line you cannot
///   see. The hairline is the room's "this is here and you are not meant to
///   read it" mark, and it is already on the contrast test's exemption list by
///   name — where §1.6 put "the needle's unfilled track" before it existed.
/// - **No handle, and no border.** A knob on a 2 px line is a dot on a hair, and
///   the fill's own leading edge is the playhead. The border a groove drew to
///   separate itself from the bar would be 1 px of edge around a 2 px mark.
///
/// The states are the ladder the groove used: amber at rest, brighter under the
/// pointer, deeper while held — so the line answers the hand the way every
/// other control in the bar does, on the ink rather than on a ground.
#[must_use]
pub fn needle(p: &Palette, status: slider::Status) -> slider::Style {
    let fill = match status {
        slider::Status::Active => p.lamp,
        slider::Status::Hovered => p.lamp_bright,
        slider::Status::Dragged => p.lamp_deep,
    };
    slider::Style {
        rail: Rail {
            backgrounds: (
                Background::Color(fill),
                Background::Color(p.hairline(p.recess)),
            ),
            width: NEEDLE_H,
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: 0.0.into(),
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

/// The needle with nothing queued, or a queue this process never sent: the
/// track alone, drawn rather than hidden.
///
/// Drawn, because a line that came and went with the music would be movement in
/// the one place ADR-0020 forbids it and the reserved-slot rule forbids it
/// twice; and unfilled, because a fill is a claim about a playhead there is no
/// queue to have.
#[must_use]
pub fn needle_inert(p: &Palette, _status: slider::Status) -> slider::Style {
    slider::Style {
        rail: Rail {
            backgrounds: (
                Background::Color(p.hairline(p.recess)),
                Background::Color(p.hairline(p.recess)),
            ),
            width: NEEDLE_H,
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: 0.0.into(),
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

/// The volume fader: a recessed groove inked in paper rather than lamp amber,
/// with a knob that does **not** grow.
///
/// Two deliberate differences from [`needle`], each with a reason:
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
                color: p.hairline(p.recess),
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
            color: p.hairline(p.recess),
            width: 1.0,
            radius: RADIUS_CTRL.into(),
        },
        ..container::Style::default()
    }
}

/// A hover preview: a small card floating over a control with
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
            color: p.hairline_strong(p.plinth_lit),
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
                p.hairline_strong(p.plinth_lit)
            } else {
                Color::TRANSPARENT
            },
            width: 1.0,
            radius: RADIUS_SEGMENT.into(),
        },
        shadow: Shadow::default(),
    }
}

// `panel_toggle` is **deleted**. It was [`segment`] under another name — a
// *selectable* control's paint, a raised `plinth_lit` card with a hairline edge
// — worn by the two things in baz that are navigation rather than selection:
// `Settings` and `‹ Library`. Navigation has no "on" state to draw (the place
// it leads to replaces the bar the control sits in, so there is no frame in
// which it could be lit and visible at once), and it should not raise a card to
// say the pointer found a word. Both now take [`word_button`].

/// The now-playing bar: recessed below the wall, like the amp under the
/// shelf.
#[must_use]
pub fn bar(p: &Palette) -> container::Style {
    container::Style {
        background: Some(Background::Color(p.recess)),
        ..container::Style::default()
    }
}

/// Hairline rules dividing chrome from shelf, drawn on `on`.
///
/// The ground is a parameter because a rule is an **opaque** colour now, not an
/// alpha the renderer blends — see [`Palette::ink_over`] for why. The three
/// structural rules land on three different planes (the wall under the top bar,
/// the wall above the now-playing bar, the panel inside the inspector), and a
/// hairline pre-composited over the wrong one is visibly the wrong hairline.
#[must_use]
pub fn hairline(p: &Palette, on: Color) -> rule::Style {
    rule::Style {
        color: p.hairline(on),
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
            color: p.hairline_strong(p.plinth_lit),
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
///
/// # `ground` — the parameter that made this style true everywhere
///
/// The hover used to be the *value* [`Palette::plinth`], which is right for
/// every row standing on the wall and wrong for the playlist panel's, whose
/// ground already **is** `plinth`: the panel's rows painted the colour that was
/// already under them and answered the pointer with nothing. The owner named it
/// (2026-08-09, *"a bit… unresponsive"*), and the fix is that a hover is one
/// step up from wherever the row stands ([`Palette::step_up`]) rather than a
/// fixed plane. On the wall this is the shipped behaviour to the bit —
/// `step_up(wall)` is `plinth` and `step_up(step_up(wall))` is `plinth_lit` —
/// which is what makes the change a correction rather than a repaint.
///
/// **The paint is the whole hit area or it is a lie.** Every caller draws this
/// on the `button` that *is* the row, at `Length::Fill`, with the row's padding
/// inside it — never on an inner block. A highlight smaller than the pressable
/// region tells the listener to aim somewhere that is not where the press is.
#[must_use]
pub fn track_row(
    p: &Palette,
    ground: Color,
    status: button::Status,
    playing: bool,
) -> button::Style {
    let lit = p.step_up(ground);
    let carded = p.step_up(lit);
    let background = match (playing, status) {
        // The playing row keeps its card whatever the pointer is doing, and
        // lifts no further under it: it is already the emphasised row.
        (true, _) => Some(carded),
        (false, button::Status::Hovered | button::Status::Pressed) => Some(lit),
        (false, button::Status::Active | button::Status::Disabled) => None,
    };
    button::Style {
        background: background.map(Background::Color),
        // The row's inks are set per-line by the view (a played row is fainter
        // than an upcoming one), so the button contributes none of its own.
        text_color: p.paper,
        border: Border {
            color: if playing {
                p.hairline_strong(carded)
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

/// **The widest a column of list rows is ever set** (logical px) — the measure,
/// 880.
///
/// The record page's track list and the queue place's rows both take it, and it
/// is the same number and the same argument as [`SETTINGS_CONTENT_MAX`]: 880 is
/// roughly 75 characters at [`SIZE_BODY`], the top of the comfortable measure.
///
/// It exists because a place fills the window and a *list* must not. A row
/// whose title is at one end of 1800 px and whose right-aligned duration is at
/// the other is not a row; it is two words the eye has to travel between, and
/// the ruled right edge [`DURATION_W`] buys stops meaning anything at that
/// distance.
///
/// So a place's body grows with the window until it reaches this, and then
/// stops and centres in what is left. Below it the body hangs from the window's
/// two gutters (law L1); at and above it the body's own two edges are the ones
/// the surface declares (law L5).
pub const LIST_MEASURE: f32 = 880.0;

/// **The run column's measure on the merged now-playing surface** (logical px)
/// — half of [`LIST_MEASURE`], **440**.
///
/// Derived rather than chosen (`docs/design/12-now-playing-and-kiosk.md`
/// §5.5a): [`LIST_MEASURE`] is the measure this product gives a list that owns
/// its surface, and the run owns *half* of one — the record has the other half.
/// It clears the run row's own anatomy with room to spare —
/// [`TRACK_NO_W`] 24 + [`GAP_SM`] 8 + title + `GAP_SM` 8 + [`DURATION_W`] 48 +
/// [`GAP_XS`] 4 + four [`STEPPER_HIT`] 96 + three `GAP_XS` 12 +
/// [`SCROLLBAR_LANE`] 10 = 210 + title — leaving 230 px of title lane, against
/// the ~224 px the bar's own left block gets for the same three-line reading.
/// **A run row is not a cramped row; it is the row the bar has always drawn,
/// given more room.**
pub const RUN_MEASURE: f32 = LIST_MEASURE / 2.0;

/// **The body width below which the merged surface stops being two columns**
/// (logical px) — **784**.
///
/// `ART_MIN` 240 + two [`HANG`] 80 + [`RUN_MEASURE`] 440 + [`GAP_XL`] 24: the
/// narrowest body in which the record can be [`ART_MIN`] *and* the run can be
/// its own measure. Below it the record cannot be the size it deserves in any
/// case — 240 px in a 704 px column is a thumbnail, not a subject — so the
/// columns re-stack into one and the record becomes the run's head block
/// (`docs/design/12-now-playing-and-kiosk.md` §5.5a). **One composition
/// degrading, not a second layout**: the same four objects, re-hung.
///
/// It bites at a 1064 px window with the returns lane open, or an 880 px window
/// with it collapsed — both below the 1280 the composition audits are taken at,
/// so the regime is a real one rather than a theoretical one.
pub const SPLIT_FLOOR: f32 = ART_MIN + 2.0 * HANG + RUN_MEASURE + GAP_XL;

/// Edge of the sleeve on a record's page (logical px) — **320**.
///
/// `ART_MAX`, which is `art::THUMB_PX`, so the decoded thumbnail is drawn at
/// exactly 1 : 1 and the refusal *no artwork is ever drawn larger than its
/// source* is satisfied at the boundary rather than approached.
///
/// The album inspector capped its sleeve at 120 because at 292 it was **93.6 %
/// of the panel's ink** and a second, larger copy of a work already on the wall
/// 24 px to the left (the audit's defect 5). A place has replaced the wall:
/// there is no other copy, and the record is the subject. See
/// [`crate::views::album`] for the declared hierarchy that follows, which puts
/// the work first *by declaration* — as the wall's own does — and holds the
/// title as the loudest type on the page.
pub const ALBUM_SLEEVE: f32 = ART_MAX;

/// Width of the record page's left column (logical px) — the sleeve, the one
/// action, and the condition report, all on one lane.
///
/// It is the sleeve's own edge, so the column introduces no x-position the
/// artwork does not already establish (law L5).
pub const ALBUM_ASIDE_W: f32 = ALBUM_SLEEVE;

/// Window width below which the record's page stacks into one column
/// (logical px) — **760**.
///
/// It is the width at which the track list stops being wider than the sleeve
/// beside it, which is the point at which two columns have stopped being two
/// columns: `744 − 2 × HANG − SCROLLBAR_LANE − ALBUM_ASIDE_W − GAP_XL` = 310,
/// against a 320 px sleeve. Below it the object goes above what is written
/// about it, which is the same branch — and the same reasoning — as
/// [`SETTINGS_BREAKPOINT`].
pub const ALBUM_BREAKPOINT: f32 = 744.0;

/// Height reserved in the bar's left zone for the ambient continuation line
/// (logical px) — `then 2 albums · 1:58:00 left`, under the title and artist.
///
/// The **third rung** of the zone's type hierarchy, and the quietest: the title
/// is [`SIZE_BODY`] in the Medium face at full paper, the artist [`SIZE_META`]
/// at [`Palette::paper_dim`], and this is [`SIZE_CAPTION`] at
/// [`Palette::paper_faint`] — the
/// metadata voice the rest of the bar already speaks in (the stamps, the
/// signal note, the skipped-tracks note). It is a statement about music that is
/// not playing yet, so it must not compete with the one that is.
///
/// **Reserved, not added.** The lane is this tall whether or not there is a
/// continuation to state, because the line comes and goes with the queue — it
/// is absent on the last track of every queue — and a left zone that grew a
/// line would push the title up under the pointer at the moment a listener was
/// reading it. `views::bottom_bar` draws an empty strip in its place, and the
/// zone's whole height stays under the centre column's, which is what keeps the
/// bar's own height a property of the transport (asserted in both modules).
///
/// # It is [`LINE_BODY`], not [`LINE_CAPTION`], and that is what centres the
/// zone
///
/// The lane is 20 px for a 16 px line, and the four spare pixels buy law L4's
/// second clause: *a zone taller than one line hangs its extra lines
/// symmetrically about the bar's centre line*. The stack is title (20) · artist
/// (16) · continuation (20) with [`GAP_XXS`] between, so the **artist's line box
/// is exactly the block's middle** — centring the block therefore puts the
/// zone's own centre line on the bar's, instead of 2 px off it.
pub const CONTINUATION_H: f32 = LINE_BODY;

// `SETTINGS_TOGGLE_W` is gone (doc 10 §7 step 1): the route to the Settings
// place is the gear — a [`TRANSPORT_HIT`] square in the strip's corner — so
// there is no word left to reserve a width for. The 52 px difference is the
// first of the strip's three reclamations (ADR-0026's arithmetic).

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

/// Greatest width the Settings place gives its content at a **wide** window
/// (logical px) — the top of the comfortable measure, 880.
///
/// The audit's defect 9: [`SETTINGS_CONTENT_W`] was a constant, so the form's
/// right edge was 878 at a 1280 px window *and* at a 1920 px one — 0.686 W and
/// then **0.457 W**, with a thousand pixels of empty wall beside it. A measure
/// should not grow without limit and it should not refuse to grow at all;
/// [`crate::views::settings`] aims at half the window and clamps into
/// `[SETTINGS_CONTENT_W, SETTINGS_CONTENT_MAX]`, which holds the form between
/// 55 and 75 characters at [`SIZE_BODY`] across every shipped width.
pub const SETTINGS_CONTENT_MAX: f32 = 880.0;

/// Narrowest width the Settings place gives its content (logical px) — **292**.
///
/// The floor of [`crate::views::settings`]'s clamp: at a small window the form
/// gets whatever there is, because a stepper row that will not fit is worse
/// than a long one. It was spelled `PANEL_W − 2 × GAP_XL` — the album
/// inspector's content lane — back when there was an inspector to borrow a
/// number from. ADR-0022 deleted the column; the floor is stated directly
/// rather than left pointing at a surface that no longer exists.
pub const SETTINGS_CONTENT_MIN: f32 = 292.0;

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
/// **The now-playing text as a control** — the bar's route back to the record
/// that is sounding (`docs/design/03-interface-prior-art.md` R3).
///
/// Invisible at rest, because the bar's left zone must go on reading as *what
/// is playing* and not as a button, and a quiet ink wash under the pointer,
/// which is the same mark [`word_button`] and [`transport`] make. It is the one
/// place in the product where the mark is a wash and there is no box to put it
/// in, so the wash is the whole affordance and the tooltip is the name.
///
/// **The border is 0 px in every state, which is a departure from every other
/// button style here and the reason this function exists.** iced draws a border
/// inside the widget's bounds, so every other control carries a transparent
/// 1 px edge to keep its geometry constant across states. This control has no
/// padding at all: its content is the left zone's three reserved line boxes,
/// exactly [`NOW_PLAYING_H`], and a 1 px edge would make the block 58 px in a
/// band derived from 56 — law L4's centre line, broken by a border nobody can
/// see. The rule is kept by having no edge rather than by having a
/// transparent one.
///
/// No accent: going to a record's page is a *view* choice, not a claim about
/// what is playing.
#[must_use]
pub fn now_playing_text(p: &Palette, status: button::Status) -> button::Style {
    let background = match status {
        button::Status::Hovered => p.ink_wash(p.recess),
        button::Status::Pressed => p.ink_wash_press(p.recess),
        button::Status::Active | button::Status::Disabled => Color::TRANSPARENT,
    };
    button::Style {
        background: Some(Background::Color(background)),
        text_color: p.paper,
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: RADIUS_CTRL.into(),
        },
        shadow: Shadow::default(),
    }
}

// ===========================================================================
// The surface-styling pass — ADR-0017 steps 14 and 15, plus the icon button
//
// Appended as one block, at the end, deliberately and for the reason the block
// above gives: a token added here conflicts with nothing when two parallel
// passes over this file meet. Nothing in this section changes an existing
// value; every name is new. Move each one up into its proper section once the
// passes either side of it have landed.
//
// The washes and the ink ramp additions this pass needs are *room-dependent*,
// so they are `Palette` methods and alpha constants up in §Palette rather than
// tokens down here; what is left below is geometry and the reserved slots the
// album inspector's new blocks need.
// ===========================================================================

/// The playing sleeve's halo blur (logical px).
///
/// **24**, where it was 16 (`.interface-design/system.md` §4). The halo used to
/// compete with a contact shadow on every other tile; with the shadows deleted
/// it is the only light in the shelf, and at 16 px it read as a rim around the
/// art rather than as a lamp pointed at it. Blur is also the only dimension of
/// a shadow iced 0.13 lets this design tune — there is no spread — so the
/// spread is expressed here.
pub const HALO_BLUR: f32 = 24.0;

/// Width reserved for a duration at the right edge of a track or queue row
/// (logical px).
///
/// A **reserved slot**, exactly like [`STAMP_W`] and [`SIGNAL_W`], and it is
/// the fix for a defect visible in every screenshot of the inspector: the
/// durations were laid out by a `text` sized to its own string, so `9:41` and
/// `12:07` ended in different columns and a thirteen-track record had a ragged
/// right edge where it should have had a ruled one. Right-aligned in a fixed
/// lane, the figures pin the edge the eye follows (§8.2: *figure columns are
/// right-aligned*).
///
/// 48 holds `1:59:59` — seven glyphs of which five are figures at
/// the face's real digit advance — with room for the two colons, which is every duration a track
/// can honestly have. Plex Sans's digits are tabular, so the slot is exact
/// rather than approximately right.
pub const DURATION_W: f32 = 48.0;

/// Width of a label in the inspector's **Details** block (logical px).
///
/// The labels are right-aligned in it and the values left-aligned after it, so
/// the block reads as two columns rather than as a dozen sentences. 96 holds
/// `Album artist` and `Sample rate`, the longest field names the block draws,
/// at [`SIZE_META`].
pub const FIELD_LABEL_W: f32 = 96.0;

/// Row pitch in the **Details** block (logical px).
///
/// Tighter than the [`SIZE_META`] line box the block's type would otherwise
/// take, deliberately: a dozen fields at a comfortable reading leading is a
/// page, and this is a reference table you scan rather than prose you read —
/// the back of the record's card.
///
/// **16, where it was 17.** A reserved slot height is a multiple of 4 (law L2),
/// and 16 is [`LINE_META`] exactly — the row is one line of its own type and
/// nothing more, which is a tighter statement of the same intent than a number
/// a pixel above it.
pub const DETAIL_ROW_H: f32 = LINE_META;

/// How far a missing sleeve's placeholder gradient is pulled back toward the
/// room's recess: **0.62 of the way**. Applied by
/// [`Palette::placeholder_ink`], where the argument is.
pub const PLACEHOLDER_MIX: f32 = 0.62;

// ===========================================================================
// The sleeve's mat
// ===========================================================================

/// Width of the mat around every sleeve on the wall (logical px).
///
/// Two, the same as [`SELECTION_EDGE`], because it is the same measure of
/// separation between a work and the wall it hangs on. There is no third mark
/// thickness in the product.
///
/// # It was the shuffle pool's ring lane
///
/// ADR-0017 step 17 reserved this lane on every tile in every state, so that
/// the faint ink ring the shuffle's next two draws carried cost no geometry and
/// moved no cover when it arrived — beside `POOL_DIM`, which composited the
/// artwork of every record outside the pool at 35 %. Both marks existed to
/// answer one question about a draw whose source was only implied: *what can
/// this shuffle play?*
///
/// **The owner made shuffle a property of the player on 2026-08-10** and there
/// stopped being a draw to mark: what a mode re-orders is the run, which is an
/// ordinary queue a listener can open and read. The ink went; the **geometry
/// stays**, because it is the measure every grid constant, capacity sum and
/// render capture in `docs/design/impl` is computed against, and re-deriving the
/// whole wall to reclaim 4 px would be a change to the collection made to tidy
/// away a mark. What is drawn in the lane now is the wall's own colour, which
/// is what it was drawn in whenever no shuffle was running — the ordinary state,
/// made the only one.
pub const SLEEVE_MAT: f32 = 2.0;

/// The mat: the wall's own colour, in the lane [`SLEEVE_MAT`] reserves.
///
/// It took a `ringed: bool` and answered [`Palette::paper_faint`] for a record
/// the shuffle would play next. There is no next draw to name any more, so
/// there is no argument left and no parameter.
pub fn sleeve_mat(p: &Palette) -> container::Style {
    container::Style {
        background: Some(Background::Color(p.wall)),
        ..container::Style::default()
    }
}

// ---------------------------------------------------------------------------
// The hover veil, and the four options laid over a sleeve
// ---------------------------------------------------------------------------

/// **The veil, as designed** — the owner's approved mockup, in the model its
/// numbers were written in: an offset across the sleeve's width paired with an
/// opacity of [`Palette::recess`], composited in **sRGB**.
///
/// It gathers at the sleeve's left edge and is gone before the right one, so
/// the right of every cover stays exactly as painted and the record stays
/// recognisable while you choose. That is the whole reason it is a gradient
/// and not a panel: a flat scrim over a sleeve hides the record you are
/// pointing at, which is the thing the wall is made of.
///
/// These numbers are **never handed to the renderer**. iced composites in
/// linear light; see [`veil_alpha`] for what is handed over instead and why
/// the difference is not a rounding error.
pub const VEIL_SPEC: [(f32, f32); 6] = [
    (0.00, 0.92),
    (0.38, 0.86),
    (0.55, 0.66),
    (0.68, 0.30),
    (0.82, 0.05),
    (1.00, 0.00),
];

/// The ground [`veil_alpha`] solves against: **sRGB mid grey**.
///
/// A single alpha cannot reproduce an sRGB composite over *every* sleeve at
/// once — the correction is ground-dependent — so the reference is stated
/// rather than assumed, and it is the perceptual midpoint of the range a
/// sleeve can occupy. The residual over the rest of that range is small and
/// measured: `the_veil_is_solved_against_a_stated_ground_and_its_residual_is_bounded`
/// holds it to ≤ 10 / 255 across sleeve grounds from sRGB 0.15 to 0.95, and
/// the sampled-pixel table in `docs/design/impl/hover-options/README.md`
/// checks it against real rendered frames rather than against this arithmetic.
pub const VEIL_GROUND: Color = Color {
    r: 0.5,
    g: 0.5,
    b: 0.5,
    a: 1.0,
};

/// sRGB → linear light, iced's own transfer function
/// (`iced_core::Color::into_linear`).
fn to_linear(u: f32) -> f32 {
    if u <= 0.040_45 {
        u / 12.92
    } else {
        ((u + 0.055) / 1.055).powf(2.4)
    }
}

/// **The alpha that draws `spec` right.**
///
/// [`Palette::ink_over`] fixes the sRGB/linear mismatch for *opaque* marks by
/// compositing here and handing the renderer a colour. The veil cannot use
/// that trick: what it lands on is album art, which the theme does not know.
/// So the blend happens in the renderer, in linear light, and the only thing
/// left to correct is the number.
///
/// Given a `spec` opacity written as an sRGB composite of `ink` over `ground`,
/// this returns the opacity that reproduces that same result when the GPU
/// blends in linear light:
///
/// ```text
///   target = spec·ink + (1 − spec)·ground              (the intent, in sRGB)
///   a      = (lin(ground) − lin(target)) / (lin(ground) − lin(ink))
/// ```
///
/// solved per channel and averaged (the three answers differ by < 0.005 for
/// every stop in [`VEIL_SPEC`], which is why one alpha per stop is honest).
///
/// # The correction runs both ways, and the veil's way is the unfamiliar one
///
/// The mismatch this module already documents at [`Palette::ink_over`] is
/// *light ink on a dark ground*, where linear compositing draws **louder** —
/// a 7 % hairline drew at ink 26 %, 3.7×. The veil is the opposite case, dark
/// ink over artwork that is mostly lighter than it, and there linear
/// compositing draws **quieter**. Handed through unchanged, the design's own
/// numbers would have drawn at roughly half their specified weight over a
/// mid-grey sleeve (`0.30` reading as an effective `0.16`, `0.05` as `0.025`)
/// — a veil that dissolves too early and an ink lane with no ground under it.
/// Applying the hairline's 3.7× in its remembered direction would have made
/// that worse, not better. The direction is a property of which side of the
/// blend is brighter, not a constant, so it is solved rather than remembered.
#[must_use]
pub fn veil_alpha(spec: f32, ink: Color, ground: Color) -> f32 {
    let channel = |ink: f32, ground: f32| {
        let target = spec.mul_add(ink - ground, ground);
        let (lin_ink, lin_ground) = (to_linear(ink), to_linear(ground));
        let span = lin_ground - lin_ink;
        if span.abs() < f32::EPSILON {
            spec
        } else {
            ((lin_ground - to_linear(target)) / span).clamp(0.0, 1.0)
        }
    };
    (channel(ink.r, ground.r) + channel(ink.g, ground.g) + channel(ink.b, ground.b)) / 3.0
}

/// The veil itself: [`VEIL_SPEC`] with every opacity put through
/// [`veil_alpha`], as a horizontal gradient across the sleeve.
///
/// `iced::Radians(FRAC_PI_2)` is left-to-right: `Radians::to_distance`
/// subtracts a quarter turn before taking `(cos, sin)`, so π/2 gives the
/// direction vector `(1, 0)` and the ramp runs from the box's left edge to
/// its right one.
#[must_use]
pub fn hover_veil(p: &Palette) -> iced::gradient::Linear {
    VEIL_SPEC.iter().fold(
        iced::gradient::Linear::new(iced::Radians(std::f32::consts::FRAC_PI_2)),
        |gradient, &(offset, spec)| {
            gradient.add_stop(
                offset,
                alpha(p.recess, veil_alpha(spec, p.recess, VEIL_GROUND)),
            )
        },
    )
}

/// Where an option's **ink lane ends**, as a fraction of the sleeve's width —
/// [`VEIL_SPEC`]'s third stop, and not a number of its own.
///
/// Past it the veil is thinner than `0.66` and the sleeve starts to come back
/// through, which is exactly where type must stop: the contrast floor is
/// measured against the *composited* veil, and over a paper-white sleeve the
/// ground at this stop is the last one that clears it. See
/// `the_option_ink_clears_its_floor_on_the_veil_over_any_sleeve`.
pub const VEIL_INK_X: f32 = VEIL_SPEC[2].0;

/// Where an option's **hit band ends**, as a fraction of the sleeve's width —
/// [`VEIL_SPEC`]'s fourth stop.
///
/// Wider than the ink lane, because a hit box is not read; and bounded well
/// short of the sleeve, because *pressing the sleeve outside an option still
/// opens the record's page*. A row that spanned the full width would take
/// that press away and leave the wall with no way to open anything.
pub const VEIL_BAND_X: f32 = VEIL_SPEC[3].0;

/// The lead between the sleeve's left edge and an option's glyph — the same
/// [`GAP_MD`] the bottom bar leads its type block by.
pub const VEIL_LEAD: f32 = GAP_MD;

/// How many options the veil carries. Each takes an equal share of the
/// sleeve's height as its hit band, which is ≥ 47 px at the tightest density
/// baz draws — well above law L7's [`TRANSPORT_HIT`] floor, and the reason
/// the options need no boxes of their own.
pub const VEIL_OPTIONS: usize = 4;

/// The row's hover wash **as designed**: an offset paired with an opacity of
/// [`Palette::paper`], brightening from the left.
///
/// A *light* wash rather than a darker one, deliberately: the veil under it is
/// already the room's darkest ground, and a second dark wash on top would say
/// "less" where the pointer means "this one". It fades out inside the ink lane,
/// so its right edge is never a drawn edge.
pub const VEIL_ROW_WASH: [(f32, f32); 3] = [(0.00, 0.10), (0.40, 0.06), (0.75, 0.00)];

/// One option's row: no ground at rest, the light wash under the pointer.
///
/// The wash's opacities are solved by [`veil_alpha`] against
/// [`Palette::recess`] rather than against [`VEIL_GROUND`], because that is
/// what this mark actually lands on — at the row's left edge the veil above it
/// is already `0.92` of the recess, so the ground under the wash is the room's
/// own, known here, whatever sleeve is behind it.
#[must_use]
pub fn veil_row(p: &Palette, status: button::Status) -> button::Style {
    let lit = matches!(status, button::Status::Hovered | button::Status::Pressed);
    let background = lit.then(|| {
        let gradient = VEIL_ROW_WASH.iter().fold(
            iced::gradient::Linear::new(iced::Radians(std::f32::consts::FRAC_PI_2)),
            |gradient, &(offset, spec)| {
                gradient.add_stop(offset, alpha(p.paper, veil_alpha(spec, p.paper, p.recess)))
            },
        );
        Background::Gradient(gradient.into())
    });
    button::Style {
        background,
        text_color: p.paper,
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 0.0.into(),
        },
        shadow: Shadow::default(),
    }
}

/// An option's glyph ink: the accent for `Play`, the room's paper for the
/// other three.
///
/// **`Play` is the accent's fifth use and it is the same use as the fourth.**
/// The module docs list `primary` — the record page's `Play album` — as the one
/// control allowed the colour, *because it is the only control in the product
/// that creates playback truth*. The wall's `Play` is that same control at the
/// same scope, moved onto the sleeve; at most one tile is hovered at a time, so
/// there is still at most one of it on screen. The licence transfers with the
/// act, and it transfers to nothing else.
///
/// **`Queue` is paper, and that is a departure from the approved mockup.** The
/// mockup gives `Queue` the accent too. the product's amber entry names
/// this exact case in these exact words — the lamp states what is true about
/// playback right now and *"not what is queued"* — and it is an entry the
/// owner's brief did not touch. The brief's own licence (*"if it reads too
/// loud, drop to paper and say so"*) is taken here rather than argued with; it
/// is one word in this function to put back.
#[must_use]
pub const fn veil_option_ink(p: &Palette, plays: bool) -> Color {
    if plays { p.lamp } else { p.paper }
}

/// The playlist panel's surface: one step up from the wall, exactly as the
/// dead rail's column and the queue popover stood (ADR-0024 §5 revives their
/// verified float without their residency).
///
/// No shadow — the product's standing rules reserves shadows for the playing halo — so
/// what separates the panel from the wall it floats over is the surface step
/// plus the 1 px hairline the view draws down its left edge, which is the
/// same two-part seam the bottom bar uses.
#[must_use]
pub fn panel(p: &Palette) -> container::Style {
    container::Style {
        background: Some(Background::Color(p.plinth)),
        text_color: Some(p.paper),
        ..container::Style::default()
    }
}

/// A playlist's sleeve in the panel's rows (logical px) — ADR-0024 §A2.
///
/// On the 4 px lattice, sized so the row it opens is the sleeve plus the
/// row's own `GAP_XS` padding: big enough that a 2 × 2 collage still reads
/// as four records (each cell 20 px), small enough that twelve rows stay a
/// panel rather than a wall. The page's sleeve is [`ART_MAX`], the album
/// page's own bound — a playlist tile is never drawn larger than a record's.
pub const PANEL_SLEEVE: f32 = 40.0;

/// **The ghost row's sleeve slot** (the owner's `New playlist`, 2026-08-09):
/// the surface *below* the panel with a hairline edge, holding the drawn
/// [`crate::icon::Glyph::Plus`].
///
/// A recess rather than a step up, and that is the whole of what makes it read
/// as *not made yet*: every real sleeve beside it is an opaque object at or
/// above the panel's own plane, and this one is a hole in the panel where an
/// object will go. Deliberately quieter even than
/// [`playlist_rest_tile`] — that stands for a made thing with nothing to
/// quote; this stands for a thing that does not exist.
#[must_use]
pub fn ghost_sleeve(p: &Palette) -> container::Style {
    container::Style {
        background: Some(Background::Color(p.recess)),
        border: Border {
            color: p.hairline(p.recess),
            width: 1.0,
            radius: RADIUS_SEGMENT.into(),
        },
        ..container::Style::default()
    }
}

/// The rest tile of a playlist with nothing to quote (ADR-0024 §A1.3): the
/// surface step with a hairline edge, the name in ink on it.
///
/// Deliberately quieter than a record's gradient placeholder: the gradient
/// stands in for artwork that exists and has not decoded, where this stands
/// for artwork that **cannot** exist yet — an empty made thing — and
/// decorating it would be the interface inventing a fact.
#[must_use]
pub fn playlist_rest_tile(p: &Palette) -> container::Style {
    container::Style {
        background: Some(Background::Color(p.plinth)),
        text_color: Some(p.paper_dim),
        border: Border {
            color: p.hairline_strong(p.plinth),
            width: 1.0,
            radius: 0.0.into(),
        },
        ..container::Style::default()
    }
}

/// The 1 px seam down the panel's left edge — the hairline half of the
/// panel/wall separation, painted as a filled lane because iced's `rule` is
/// horizontal-or-vertical by *widget* and this one has to fill a `Fill`
/// height inside a row.
#[must_use]
pub fn panel_seam(p: &Palette) -> container::Style {
    container::Style {
        background: Some(Background::Color(p.hairline_strong(p.wall))),
        ..container::Style::default()
    }
}

/// The context menu's card width (logical px) — doc 09 §5.2's mirror layer.
///
/// On the 4 px lattice. Wide enough that `Add to "{name}"` keeps a real
/// name before clipping, narrow enough that the card stays a note at the
/// pointer rather than a surface; the items' verbs are deliberately short
/// (§5.2), so the width is spent on the one label that quotes the user's
/// own words.
pub const MENU_W: f32 = 232.0;

/// The context menu's card (doc 09 §5.2): the panel's exact separation
/// strategy at float scale — the surface step ([`Palette::plinth`]) plus a
/// 1 px hairline, **no shadow** (a standing rule of the product: reserves shadows for
/// the playing halo, the same clause [`panel`] cites), and the float
/// family's [`RADIUS_CHIP`] corner shared with [`tooltip`] and
/// [`preview_tip`]. The items inside are ordinary [`track_row`] word
/// buttons, so the card introduces no colour the room does not already
/// have.
#[must_use]
pub fn menu(p: &Palette) -> container::Style {
    container::Style {
        background: Some(Background::Color(p.plinth)),
        text_color: Some(p.paper),
        border: Border {
            color: p.hairline_strong(p.plinth),
            width: 1.0,
            radius: RADIUS_CHIP.into(),
        },
        ..container::Style::default()
    }
}

#[cfg(test)]
mod tests {
    /// The bar's drawn height including its hairline — **57**. Stated here
    /// rather than as a token because nothing *draws* with it: the band is the
    /// whole bar, and a token nothing draws with is a comment that can rot.
    const BAR_H: f32 = BAR_CONTENT_H + 1.0;
    /// The whole of what the window's bottom edge costs the collection — the
    /// bar, its hairline and the needle. **59**, where it was 105.
    const BOTTOM_FURNITURE_H: f32 = BAR_H + NEEDLE_H;

    use super::*;

    /// **The ink ladder, all four rungs** — the one ADR-0020 §2.1 completes by
    /// giving the shell a hovered-control id (`docs/design/04-fluidity.md`
    /// §3.1's prescription table).
    ///
    /// It is a ladder rather than four unrelated values: every rung has to be
    /// distinguishable from the ones next to it, or the chrome's own failure —
    /// rest and disabled drawn in *pixel-identical* paint — has simply moved
    /// into the ink.
    #[test]
    fn the_icon_buttons_ink_ladder_has_four_distinguishable_rungs() {
        let rest = glyph_ink(true, false, 0.0, false);
        let hover = glyph_ink(true, false, 1.0, false);
        let press = glyph_ink(true, false, 1.0, true);
        let dead = glyph_ink(false, false, 0.0, false);
        assert!((rest - 0.57).abs() < f32::EPSILON, "rest is {rest}");
        assert!((hover - 1.00).abs() < f32::EPSILON, "hover is {hover}");
        assert!((press - 0.75).abs() < f32::EPSILON, "press is {press}");
        assert!((dead - 0.28).abs() < f32::EPSILON, "disabled is {dead}");
        // Ordered, and by margins a listener can see: the whole complaint the
        // ladder answers was that two of the readings were the same pixels.
        assert!(dead < rest && rest < press && press < hover);
        for (a, b) in [(dead, rest), (rest, press), (press, hover)] {
            assert!(b - a > 0.15, "{a} and {b} are the same reading");
        }
    }

    /// The fade between the rungs is a **ramp**, monotone and bounded, and the
    /// two readings a pointer may not overwrite stay where they are.
    #[test]
    fn the_hover_fade_only_ever_moves_along_the_ladder() {
        let mut previous = glyph_ink(true, false, 0.0, false);
        for step in 0..=100_u8 {
            let hover = f32::from(step) / 100.0;
            let value = glyph_ink(true, false, hover, false);
            assert!(
                (0.0..=1.0).contains(&value),
                "{hover}: {value} is not an alpha"
            );
            assert!(value >= previous, "{hover}: the ramp went backwards");
            previous = value;
            // A control that cannot act, and one that is waiting for the engine,
            // are unmoved by a pointer crossing them: an affordance that answers
            // a hover is claiming it can be pressed.
            for pressed in [false, true] {
                assert!(
                    (glyph_ink(false, false, hover, pressed) - GLYPH_OPACITY_DISABLED).abs()
                        < f32::EPSILON
                );
                assert!(
                    (glyph_ink(true, true, hover, pressed) - GLYPH_OPACITY_PENDING).abs()
                        < f32::EPSILON
                );
            }
        }
        // Out-of-range strengths clamp rather than escaping the ladder.
        assert!((glyph_ink(true, false, -1.0, false) - GLYPH_OPACITY).abs() < f32::EPSILON);
        assert!((glyph_ink(true, false, 9.0, false) - GLYPH_OPACITY_HOVER).abs() < f32::EPSILON);
    }

    /// **The bar's pixel stability, asserted during a transition and not only
    /// at rest.**
    ///
    /// The one promise ADR-0020 could not be allowed to cost: a transition may
    /// change ink, never the bar's geometry. Every number the bar is built from
    /// is a constant here, and the only thing a tween reaches is an opacity —
    /// which is checked by sweeping the whole ladder and asserting that what
    /// comes out is an alpha and nothing else.
    #[test]
    fn no_value_a_transition_moves_can_reach_the_bars_geometry() {
        for step in 0..=20_u8 {
            let hover = f32::from(step) / 20.0;
            for enabled in [false, true] {
                for pending in [false, true] {
                    for pressed in [false, true] {
                        let ink = glyph_ink(enabled, pending, hover, pressed);
                        assert!((0.0..=1.0).contains(&ink));
                    }
                }
            }
        }
        // The hit target, the sprite box and the row they sit in are constants:
        // there is no expression anywhere above that could vary one of them.
        const { assert!(TRANSPORT_HIT == 32.0) }
        const { assert!(ICON_PX == 16.0) }
        const { assert!(VOLUME_ROW_H == TRANSPORT_HIT) }
        // The band the bar draws in, and the lane that centres the transport in
        // it — both constants, so no transition can move the one line every
        // mark in the bar sits on (law L4).
        const { assert!(BAR_CONTENT_H == 2.0 * BAR_LEAD + TRANSPORT_HIT) }
        const { assert!(BAR_LEAD == GAP_XL) }
        // **And the needle's geometry is a constant too** — ADR-0020 forbids
        // animating bar geometry, and the needle is the one new surface a
        // transition could have been tempted onto. Its thickness, its aiming
        // band and both its gaps are literals; its *segments* move only when
        // the queue changes, and its fill only when playback does. Neither is
        // a tween (a standing rule of the product: "the needle advancing with playback
        // (data arriving) and scrolling" were never animation).
        const { assert!(NEEDLE_H == 2.0) }
        const { assert!(NEEDLE_HIT == GAP_MD) }
        const { assert!(SEGMENT_GAP == GAP_XXS && ALBUM_GAP == 8.0) }
        // **And there is nothing left above the bar to be pushed by.** The
        // queue popover's arrival was the one transition that flew over it;
        // ADR-0022 made the queue a place, so a navigation is a hard cut and
        // nothing floats.
    }

    /// **A surface step is the ladder's own rise**, so the synthetic step
    /// above the top plane is not a fifth colour invented for one call site.
    ///
    /// [`SURFACE_STEP_A`]'s 5 % has to sit inside the band the *real* steps
    /// already draw, in both rooms — otherwise a row on the menu card would
    /// answer the pointer more loudly or more quietly than a row on the wall,
    /// and the whole point of [`Palette::step_up`] is that a hover means one
    /// thing everywhere.
    #[test]
    fn a_surface_step_is_the_ladders_own_rise() {
        let mut low = f32::INFINITY;
        let mut high = 0.0_f32;
        for room in Room::ALL {
            let p = room.palette();
            for (under, over) in [(p.wall, p.plinth), (p.plinth, p.plinth_lit)] {
                for (u, o, ink) in [
                    (under.r, over.r, p.paper.r),
                    (under.g, over.g, p.paper.g),
                    (under.b, over.b, p.paper.b),
                ] {
                    let rise = (o - u) / (ink - u);
                    low = low.min(rise);
                    high = high.max(rise);
                }
            }
        }
        assert!(
            low <= SURFACE_STEP_A && SURFACE_STEP_A <= high,
            "the synthetic step {SURFACE_STEP_A} is outside the ladder's own \
             rise ({low}…{high}) — a hover would not mean one thing everywhere"
        );
    }

    /// **Every row-shaped control answers the pointer on the ground it
    /// actually stands on** — the owner's *"a bit… unresponsive"*, closed as a
    /// property rather than as a repaint of one surface.
    ///
    /// Two claims. First, the step is *visible*: hovering must not paint the
    /// ground back onto itself, which is exactly what the playlist panel did
    /// for as long as [`track_row`]'s hover was the constant
    /// [`Palette::plinth`] and the panel's own ground was `plinth`. Second, on
    /// the wall the style is the **shipped** one to the bit, which is what
    /// makes taking a ground a correction rather than a redesign.
    #[test]
    fn a_row_answers_the_pointer_on_every_ground_it_stands_on() {
        let visible =
            |a: Color, b: Color| (a.r - b.r).abs() + (a.g - b.g).abs() + (a.b - b.b).abs() > 0.005;
        for room in Room::ALL {
            let p = room.palette();
            // The four grounds a row is composed on today: the wall (album,
            // queue, playlist and songs rows), the panel and the menu card
            // (`plinth`), and the plane above them.
            for ground in [p.wall, p.plinth, p.plinth_lit, p.recess] {
                let rest = track_row(p, ground, button::Status::Active, false).background;
                let hovered = track_row(p, ground, button::Status::Hovered, false)
                    .background
                    .expect("a hovered row has a ground of its own");
                assert!(rest.is_none(), "{}: a row at rest paints nothing", p.name);
                let Background::Color(hovered) = hovered else {
                    panic!("{}: a row's hover is a colour, not a gradient", p.name);
                };
                assert!(
                    visible(hovered, ground),
                    "{}: a row hovered on {ground:?} paints {hovered:?} — the \
                     ground it is already standing on, which is no answer at all",
                    p.name
                );
                // …and the playing row is a further step, so "the pointer is
                // here" and "this is sounding" stay two different statements.
                let Some(Background::Color(playing)) =
                    track_row(p, ground, button::Status::Active, true).background
                else {
                    panic!("{}: the playing row keeps its card", p.name);
                };
                assert!(visible(playing, hovered), "{}: on {ground:?}", p.name);
            }
            // The wall's rows are the shipped values, exactly.
            let Some(Background::Color(hovered)) =
                track_row(p, p.wall, button::Status::Hovered, false).background
            else {
                panic!("a hovered row on the wall")
            };
            let Some(Background::Color(playing)) =
                track_row(p, p.wall, button::Status::Active, true).background
            else {
                panic!("the playing row on the wall")
            };
            assert!(!visible(hovered, p.plinth), "{}: hover on the wall", p.name);
            assert!(
                !visible(playing, p.plinth_lit),
                "{}: the playing row on the wall",
                p.name
            );
        }
    }

    /// **The serif is the work titles' and nothing else's** — the whole of
    /// what makes the second family a *placard convention* rather than a
    /// display face returning one weight at a time.
    ///
    /// Two claims, both over the source. `WORK_TITLE` is named in exactly the
    /// two views that set **an album's title on the surface whose subject that
    /// album is** — Home's `CONTINUE` placard and the record's page; and the
    /// serif family is reachable only through that token, so nothing can
    /// quietly set a third string in it by naming the family directly.
    ///
    /// **It is an enumeration and it stays one.** Loosening this to a
    /// `contains` would buy nothing and cost the whole guard: the risk the
    /// token carries is not that the serif is used, it is that it *spreads* —
    /// onto a track title, an artist, a playlist's name, sixty tile captions
    /// on a wall — one surface at a time, with no single change large enough
    /// to argue about. Adding a name here is that argument, made once.
    ///
    /// It is the reversion clause made mechanical: point `WORK_TITLE` at
    /// [`MEDIUM`] and `crate::font::SERIF_ITALIC` has no consumer left.
    #[test]
    fn the_serif_is_the_work_titles_and_nothing_else() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut users: Vec<String> = Vec::new();
        let mut names_family: Vec<String> = Vec::new();
        for path in rust_sources(&root) {
            let relative = path
                .strip_prefix(&root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            // This module defines the token and `font.rs` holds the bytes;
            // neither is a view setting a string in it.
            if relative == "theme.rs" || relative == "font.rs" {
                continue;
            }
            let source = std::fs::read_to_string(&path)
                .expect("a source file baz ships")
                .replace("\r\n", "\n");
            // Code lines only. A module that *names* the token in prose to
            // say it is deliberately not using it — `views/now_playing.rs`
            // does exactly that — is not a consumer, and a test that could
            // not tell the difference would punish the file for explaining
            // itself.
            let code: String = source
                .split("#[cfg(test)]")
                .next()
                .unwrap_or_default()
                .lines()
                .filter(|line| {
                    let line = line.trim_start();
                    !(line.starts_with("//") || line.starts_with("/*"))
                })
                .collect::<Vec<_>>()
                .join("\n");
            if code.contains("WORK_TITLE") {
                users.push(relative.clone());
            }
            if code.contains("font::SERIF") {
                names_family.push(relative);
            }
        }
        // `rust_sources` walks the tree in the filesystem's order, so the
        // enumeration is sorted before it is compared: the list is a set of
        // names, and a directory read order is not a fact about the design.
        users.sort();
        assert_eq!(
            users,
            ["views/album.rs", "views/home.rs"],
            "the serif italic is the museum placard's convention for a \
             *work's own title*, and it is set exactly where an album is the \
             subject being labelled: Home's `CONTINUE` placard, and the \
             record's own page. A third consumer arrives here on purpose or \
             not at all — an unenumerated one is a display face coming by the \
             back door, which is the thing `assets/fonts/README.md` records \
             as deleted and staying deleted. In particular it is **not** the \
             playlist page's hero (a label the owner typed, not a work), not \
             `views/now_playing.rs` (a track's title, with the album under it \
             as a fact about it), and not the wall or the lane, which are the \
             owner's open question."
        );
        assert!(
            names_family.is_empty(),
            "{names_family:?} name the serif family directly. It is reachable \
             through `theme::WORK_TITLE` and nowhere else, so that reverting \
             the experiment is one line."
        );
    }

    /// **The tile's hover mark fades in ink and never in geometry**, and every
    /// point of the fade is an opaque pre-composite — the property the whole
    /// [`Palette::ink_over`] correction bought, held through a transition.
    #[test]
    fn a_tiles_hover_rule_fades_its_ink_and_holds_its_thickness() {
        for room in Room::ALL {
            let p = room.palette();
            // The two ends are exactly the marks the shelf shipped.
            let full = p.hover_rule(p.wall, 1.0);
            let strong = p.hairline_strong(p.wall);
            assert!((full.r - strong.r).abs() < f32::EPSILON);
            assert!((full.g - strong.g).abs() < f32::EPSILON);
            assert!((full.b - strong.b).abs() < f32::EPSILON);
            let none = p.hover_rule(p.wall, 0.0);
            assert!(
                (none.r - p.wall.r).abs() < f32::EPSILON,
                "{}: an unhovered rule is not its own ground",
                p.name
            );

            for step in 0..=20_u8 {
                let hover = f32::from(step) / 20.0;
                let mark = p.hover_rule(p.wall, hover);
                assert!(
                    (mark.a - 1.0).abs() < f32::EPSILON,
                    "{}: a fading rule became an alpha, so what the renderer \
                     draws is not what this test measures",
                    p.name
                );
                // Thickness is a whole number in every frame: a rule drawn at
                // two thirds of a pixel is a blur, not a thin line.
                let thickness = tile_rule_h(hover, false);
                assert!(
                    thickness == 0.0 || (thickness - 1.0).abs() < f32::EPSILON,
                    "{hover}: a {thickness} px rule"
                );
                // Selection does not fade — it is a click's result, not a
                // passage — and it wins at every point of the hover.
                assert!((tile_rule_h(hover, true) - SELECTION_EDGE).abs() < f32::EPSILON);
                let selected = tile_rule(p, hover, true);
                assert_eq!(
                    container_colors(&selected),
                    container_colors(&tile_rule(p, 0.0, true))
                );
                // The caption lifts on the same ramp, stays opaque, and stays
                // on the room's own ink ramp between its two ends.
                let caption = caption_ink(p, hover);
                assert!((caption.a - 1.0).abs() < f32::EPSILON);
                let between =
                    |a: f32, b: f32, x: f32| x >= a.min(b) - 0.001 && x <= a.max(b) + 0.001;
                assert!(between(p.paper_faint.r, p.paper_dim.r, caption.r));
                assert!(between(p.paper_faint.g, p.paper_dim.g, caption.g));
                assert!(between(p.paper_faint.b, p.paper_dim.b, caption.b));
            }
            assert_eq!(
                format!("{:?}", caption_ink(p, 0.0)),
                format!("{:?}", p.paper_faint),
                "{}: a tile at rest is not the ink the shelf shipped",
                p.name
            );
            assert_eq!(
                format!("{:?}", caption_ink(p, 1.0)),
                format!("{:?}", p.paper_dim)
            );
        }
    }

    /// The lamp warms to exactly the halo baz ships, and to nothing brighter.
    #[test]
    fn the_lamp_warms_to_the_halo_and_stops_there() {
        for room in Room::ALL {
            let p = room.palette();
            let full = p.lamp_glow_at(1.0);
            let shipped = p.lamp_glow();
            assert!((full.a - shipped.a).abs() < f32::EPSILON);
            assert!((p.lamp_glow_at(0.0).a).abs() < f32::EPSILON);
            assert!(
                (p.lamp_glow_at(2.0).a - shipped.a).abs() < f32::EPSILON,
                "clamped"
            );
            // A warming halo is the *same light*, so the hue never moves — only
            // how much of it there is.
            for step in 0..=10_u8 {
                let warmth = f32::from(step) / 10.0;
                let glow = p.lamp_glow_at(warmth);
                assert!((glow.r - p.lamp.r).abs() < f32::EPSILON);
                assert!((glow.g - p.lamp.g).abs() < f32::EPSILON);
                assert!((glow.b - p.lamp.b).abs() < f32::EPSILON);
                assert!(glow.a <= shipped.a + f32::EPSILON);
                // And the sleeve it lights does not move: the blur is the same
                // number in every frame of the warm.
                let lit = sleeve(p, warmth);
                if warmth > 0.0 {
                    assert!((lit.shadow.blur_radius - HALO_BLUR).abs() < f32::EPSILON);
                }
                assert_eq!(lit.shadow.offset, Vector::ZERO);
            }
        }
    }

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
    fn the_bottom_bar_reserves_every_slot_whether_or_not_it_has_one() {
        // **The bar's whole height, stated once.** The seek row is gone and the
        // needle has it; what the window's bottom edge costs the collection is
        // the band, the hairline and 2 px of line. 105 → 59 → **83**, the last
        // step being the band re-derived from the left zone plus a stated lead
        // (see [`BAR_CONTENT_H`]).
        const { assert!(BAR_CONTENT_H == 80.0) }
        const { assert!(BAR_H == 81.0) }
        const { assert!(BOTTOM_FURNITURE_H == 83.0) }
        // The needle's aiming band reaches into the bar's bottom lane and **no
        // further**: that lane is empty recess, so a press aimed at Next can
        // never be taken by a 2 px line at the window's edge. This is the whole
        // safety argument for claiming height out of layout ([`NEEDLE_HIT`]).
        const { assert!(NEEDLE_HIT <= BAR_LEAD) }
        // The hover preview is a **layer** over that same lane rather than a row
        // in it, so it costs the column no height at all — which is the whole
        // reason the transport can sit on the bar's own centre line
        // ([`BAR_LEAD`], law L4). It floats above the needle and stops short of
        // the transport glyphs' own box.
        const { assert!(PREVIEW_H <= BAR_LEAD + HIT_SLOP) }
        // The volume block's preview lane is the same trick, and the lane it
        // floats in is the bar's lead plus the fader's own slop above its rail.
        const { assert!(PREVIEW_H <= BAR_LEAD + (VOLUME_HIT - RAIL) / 2.0) }
        // The centre column is the transport row and nothing else now, so the
        // buttons' centre is the column's centre by construction rather than by
        // a width they happen to share with a groove.
        const { assert!(TRANSPORT_W == 3.0 * TRANSPORT_HIT + 2.0 * GAP_SM) }
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
        // **The run column's measure holds a run row's whole anatomy**, and
        // holds it with more title lane than the bar's own left block gets for
        // the same reading (doc 12 §5.5a). The sum is the row's fixed furniture
        // — number column, duration, the four reserved edit slots, the
        // scrollbar's lane and the gaps between them — and what is left is the
        // title's.
        const {
            let anatomy = TRACK_NO_W
                + GAP_SM
                + GAP_SM
                + DURATION_W
                + GAP_XS
                + 4.0 * STEPPER_HIT
                + 3.0 * GAP_XS
                + SCROLLBAR_LANE;
            assert!(RUN_MEASURE - anatomy > 200.0);
        }
        // …and the split floor is the narrowest body that can hold the run at
        // that measure *and* the record at its floor, hung from the body's own
        // two gutters. Below it the columns re-stack (`SPLIT_FLOOR`'s token).
        const { assert!(SPLIT_FLOOR == ART_MIN + 2.0 * HANG + RUN_MEASURE + GAP_XL) }
    }

    /// **The ambient continuation is a reservation, not an addition.**
    ///
    /// The left zone is the bar's most contested strip: it carries the one
    /// string in the product that changes with the music, and it now carries a
    /// second one that comes and goes independently — the continuation is
    /// absent on the last track of every queue and present before it. Three
    /// things have to be true for that to cost nothing, and all three are
    /// arithmetic.
    #[test]
    fn the_left_zone_reserves_the_continuation_line_whether_or_not_it_has_one() {
        /// The zone's whole height with the third line in it: title, artist
        /// and continuation, stacked line box on line box. Every lane is
        /// reserved, so this is the zone's height in *every* state rather than
        /// its tallest.
        const LEFT_H: f32 = LINE_BODY + LINE_META + CONTINUATION_H;

        // 1. The lane holds one line of the type that draws it with air to
        //    spare, so the strip reserved when there is nothing to say is the
        //    same height as the line that says something.
        const { assert!(CONTINUATION_H >= LINE_CAPTION) }
        // 2. It is the quietest rung of the zone: smaller than the artist line
        //    under the title, which is itself smaller than the title. A
        //    continuation set as loud as the music playing would be a claim
        //    about the wrong thing. (The *lane* is `LINE_BODY`; the type in it
        //    is `SIZE_CAPTION`, and the two are different claims.)
        const { assert!(SIZE_CAPTION < SIZE_META && SIZE_META < SIZE_BODY) }
        // 3. The whole zone is the bar's content band **less one
        //    `BAR_ZONE_LEAD` a side**, so the bar's height stays a property of
        //    what the bar holds and the continuation appearing cannot grow it.
        //    This is the same claim `views::bottom_bar` asserts against the
        //    composed row; it is stated here too because the numbers are this
        //    module's.
        //
        //    It used to be *exactly* the band — 56 lanes in a 56 px band — and
        //    that is the proportion the owner read as *"the bottom bar is too
        //    short"*: correct in every part, and type touching both edges of
        //    the bar it sits in. The lead is the fix, and it is a named gap
        //    rather than a ratio, because a ratio is not reachable on the 4 px
        //    lattice for two bands of different content heights.
        const { assert!(LEFT_H == NOW_PLAYING_H) }
        const { assert!(LEFT_H + 2.0 * BAR_ZONE_LEAD == BAR_CONTENT_H) }
        const { assert!(LINE_BODY > SIZE_BODY && LINE_CAPTION > SIZE_CAPTION) }
        // 4. **The stack is symmetric about its middle lane** (law L4): the
        //    title's lane and the continuation's are the same height, so the
        //    artist's line box is the block's exact centre and centring the
        //    block puts the zone's line on the bar's.
        const { assert!(CONTINUATION_H == LINE_BODY) }
    }

    /// **A place's body is capped at a measure and centred; below the cap it
    /// hangs from the window's two gutters.**
    ///
    /// The rule the record's page and the queue place share, and the reason
    /// [`LIST_MEASURE`] is one token rather than two. A place fills the window
    /// and a *list* must not: a row whose title is at one end of 1800 px and
    /// whose right-aligned duration is at the other is two words the eye has to
    /// travel between, and the ruled right edge [`DURATION_W`] buys stops
    /// meaning anything at that distance.
    ///
    /// The popover this replaces made the opposite promise — it was 360 px
    /// **fixed**, because an overlay that grew with the window would eventually
    /// be a panel that forgot to reflow the shelf. A place has no shelf to
    /// reflow, so it may grow; what it may not do is grow without limit.
    #[test]
    fn a_places_body_is_capped_at_a_measure_and_centred() {
        /// What a queue row has left for its title once the number column, the
        /// reserved scrollbar lane, the removal target, the duration lane and
        /// the gaps between them have taken their share, at the measure.
        const ROW_TITLE_LANE: f32 =
            LIST_MEASURE - TRACK_NO_W - SCROLLBAR_LANE - STEPPER_HIT - DURATION_W - 3.0 * GAP_SM;

        // The measure is the top of the comfortable range and it is the same
        // number the Settings place already caps its form at — one measure in
        // the product, not two.
        const { assert!(LIST_MEASURE == SETTINGS_CONTENT_MAX) }
        // Wide enough for the rows it inherited from the popover, four times
        // over: the lane was 180 px there.
        const { assert!(ROW_TITLE_LANE > 4.0 * 180.0) }
        // At the shipped window a place still hangs from both gutters, so the
        // cap is a ceiling rather than a fixed width.
        const { assert!(1280.0 - 2.0 * HANG > LIST_MEASURE) }
        // The record's page: the aside is the sleeve's own edge, the sleeve is
        // the source's own size, and the two columns plus the gutters more than
        // fill the shipped window — so at 1280 the page hangs from both
        // gutters and the list is under its cap.
        const { assert!(ALBUM_ASIDE_W == ALBUM_SLEEVE && ALBUM_SLEEVE == ART_MAX) }
        const { assert!(ALBUM_ASIDE_W + GAP_XL + LIST_MEASURE + 2.0 * HANG > 1280.0) }
        // …and at the breakpoint the list would be narrower than the sleeve
        // beside it, which is what the breakpoint *is*.
        const {
            assert!(
                ALBUM_BREAKPOINT - 2.0 * HANG - SCROLLBAR_LANE - ALBUM_ASIDE_W - GAP_XL
                    <= ALBUM_SLEEVE
            );
        }
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
    fn a_list_still_fits_what_it_has_to_draw() {
        // The narrowest a list is ever set — the record page's track lane at
        // the stacking breakpoint — less the number column, the duration lane,
        // the gaps and the scrollbar lane. What is left is the title's.
        let inner =
            ALBUM_BREAKPOINT - 2.0 * HANG - SCROLLBAR_LANE - TRACK_NO_W - DURATION_W - 2.0 * GAP_SM;
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
        // The width a wrapped line actually has, at the narrowest the place is
        // ever set: the floor of `views::settings`'s clamp, less the scrollbar
        // lane.
        let content_w = SETTINGS_CONTENT_MIN - SCROLLBAR_LANE;
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

    /// **The shelf virtualizes at every width the window can produce.**
    ///
    /// One of the four properties `docs/design/01-ux-audit-and-ia.md` §5 says
    /// must not regress, checked over the **whole band at 1 px resolution**
    /// rather than at a stride: with a fluid cell the column count and every
    /// sleeve's size change together, the transitions are single-pixel events,
    /// and a coarse sweep can step straight over one.
    ///
    /// **The inspector is gone from the sweep**, and that is the finding rather
    /// than a simplification: it used to run over `[0, PANEL_W]` because a
    /// press could take 340 px off the wall, and ADR-0022 left the wall's width
    /// a function of the window and the index rail's lane. The band is the same
    /// band; it is now reached only by dragging the window's edge.
    #[test]
    fn the_shelf_virtualizes_at_every_width_the_window_can_produce() {
        use crate::shelf::{Density, Grid};

        const WINDOW_W: f32 = 1280.0;
        assert_eq!(
            Grid::new(WINDOW_W - INDEX_LANE_W, Density::Balanced).columns,
            4,
            "the shipped wall, with the rail's lane off it"
        );

        // The band: every window width baz can be dragged to, at 1 px, with a
        // full library and a single search result — and every density step,
        // since the step is what the grid's four numbers come from (ADR-0017
        // step 6).
        for window in 640..=2560 {
            #[expect(
                clippy::cast_precision_loss,
                reason = "a window width in pixels is far below f32's exact-integer range"
            )]
            let window = window as f32;
            for density in Density::ALL {
                // What the wall really measures: the window, less the index
                // rail's lane, and nothing else (ADR-0017 step 8, ADR-0022).
                let hang = Grid::new((window - INDEX_LANE_W).max(0.0), density);
                assert!(hang.columns >= 1, "the grid collapsed at {window} px");
                assert!(
                    hang.art > 0.0 && hang.art <= density.art_max(),
                    "{window} px at {density:?}: {} px of art",
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
                    // A fling to the far end of a 10 000-album wall still lands
                    // on a clamped range — the pitch is a float, so this is
                    // arithmetic worth checking rather than obvious.
                    let (first, end) = hang.visible_rows(hang.spacer_height(rows), 800.0, rows);
                    assert!(first <= end && end <= rows);
                }
            }
        }
    }

    /// **The shelf virtualizes at every width the window can produce — in
    /// both of the lane's states.**
    ///
    /// ADR-0030's own acceptance test. The sweep above ran over
    /// `window − INDEX_LANE_W`, which was the whole of the wall's width until
    /// the lane took a term of its own; this is the same sweep with the second
    /// term in, at 1 px, over both states and every density step. It is the
    /// answer to *"the collapse must not jank"* stated as arithmetic rather
    /// than as intent.
    #[test]
    fn the_shelf_virtualizes_in_both_of_the_lanes_states() {
        use crate::shelf::{Density, Grid};

        for window in 300..=2560 {
            #[expect(
                clippy::cast_precision_loss,
                reason = "a window width in pixels is far below f32's exact-integer range"
            )]
            let window = window as f32;
            for stored in [true, false] {
                let wall = (window - sidebar_w(window, stored) - INDEX_LANE_W).max(0.0);
                for density in Density::ALL {
                    let hang = Grid::new(wall, density);
                    assert!(hang.columns >= 1, "{window} px, open={stored}");
                    assert!(
                        hang.art > 0.0 && hang.art <= density.art_max(),
                        "{window} px, open={stored}, {density:?}: {} px of art",
                        hang.art
                    );
                    assert!(hang.row_h > 0.0, "{window} px, open={stored}");
                    assert!(
                        hang.block_width() <= wall + 0.01,
                        "{window} px, open={stored}: the block overruns the wall"
                    );
                    let rows = hang.rows(97);
                    let (first, end) = hang.visible_rows(0.0, 800.0, rows);
                    assert!(first < end && end <= rows, "{window} px, open={stored}");
                }
            }
        }
    }

    /// **The lane's two widths, and the floor that decides between them.**
    ///
    /// Three claims, and the third is the one that matters. The widths are
    /// built from the room's own tokens; the floor is where the *open* lane
    /// still leaves the collection two columns at or above [`ART_MIN`]; and
    /// **the stored state is not the drawn state below the floor** — which is
    /// what makes `Expanded` inert there rather than a control that produces a
    /// window the wall cannot use.
    #[test]
    fn the_lane_has_two_widths_and_a_floor_that_chooses() {
        use crate::shelf::{Density, Grid};

        const { assert!(SIDEBAR_W == 280.0 && SIDEBAR_RAIL_W == 96.0) }
        const { assert!(SIDEBAR_W == GAP_XL + MENU_W + GAP_XL) }
        const { assert!(SIDEBAR_RAIL_W == GAP_XL + SIDEBAR_SLEEVE + GAP_XL) }
        const { assert!(SIDEBAR_SLEEVE == 48.0 && SIDEBAR_ROW_H == 64.0) }
        const { assert!(SIDEBAR_DEST_H == 40.0) }
        // Every one of them on the 4 px lattice (law L2).
        const {
            assert!(
                SIDEBAR_W % 4.0 == 0.0
                    && SIDEBAR_RAIL_W % 4.0 == 0.0
                    && SIDEBAR_SLEEVE % 4.0 == 0.0
                    && SIDEBAR_ROW_H % 4.0 == 0.0
                    && SIDEBAR_DEST_H % 4.0 == 0.0
                    && SIDEBAR_FLOOR % 4.0 == 0.0
            );
        }

        // The floor, re-derived rather than asserted: the smallest window at
        // which the open lane leaves two columns at or above ART_MIN.
        let two_wide = |window: f32| {
            let wall = (window - SIDEBAR_W - INDEX_LANE_W).max(0.0);
            let hang = Grid::new(wall, Density::Balanced);
            hang.columns >= 2 && hang.art >= ART_MIN
        };
        let derived = (600..=1400)
            .find(|w| {
                #[expect(
                    clippy::cast_precision_loss,
                    reason = "a window width in pixels is exact in f32"
                )]
                let w = *w as f32;
                two_wide(w)
            })
            .expect("some window is wide enough for an open lane and two columns");
        assert!(
            f32::from(u16::try_from(derived).expect("a window width fits u16")) <= SIDEBAR_FLOOR,
            "the floor {SIDEBAR_FLOOR} is below the width the arithmetic wants ({derived})"
        );
        assert!(
            two_wide(SIDEBAR_FLOOR),
            "the floor must itself be wide enough"
        );

        // And the decision: below the floor the stored state is overruled.
        assert!((sidebar_w(SIDEBAR_FLOOR, true) - SIDEBAR_W).abs() < f32::EPSILON);
        assert!((sidebar_w(SIDEBAR_FLOOR - 1.0, true) - SIDEBAR_RAIL_W).abs() < f32::EPSILON);
        assert!((sidebar_w(2560.0, false) - SIDEBAR_RAIL_W).abs() < f32::EPSILON);
        assert!(sidebar_can_expand(SIDEBAR_FLOOR));
        assert!(!sidebar_can_expand(SIDEBAR_FLOOR - 1.0));
    }

    /// **The shelf break's vertical rhythm is `HANG` and arithmetic on it.**
    ///
    /// A group header that introduced a spacing number of its own would be a
    /// second vertical grid on a wall that has one, so every quantity in the
    /// band is asserted to be either the hang or the hang minus the type.
    ///
    /// The ratio is stated here as well as in the doc comment, because it is
    /// the thing a ruler held up to a screenshot measures: **40 px of air above
    /// a header's ink and 28 below it**, which puts a header nearer the shelf
    /// it names than the shelf it follows.
    #[test]
    fn a_shelf_break_is_a_hang_and_the_type_inside_it() {
        // The band is the hang, exactly.
        const { assert!(SHELF_HEADER_H == HANG) }
        // The line box is the heading's own, and — like every line box in the
        // scale — it is a multiple of the 4 px unit (law L2), so a shelf break
        // is three numbers and all three are on the lattice.
        assert!((HEADING_LINE_H - 12.0).abs() < f32::EPSILON);
        assert!((HEADING_LINE_H - LINE_HEADING).abs() < f32::EPSILON);
        assert!((HEADING_LINE_H - SIZE_HEADING * LEADING_HEADING).abs() < 1e-4);
        // Air above the ink is the row above's trailing hang; air below is
        // whatever the band has left. Both are derived, neither is chosen.
        let below = SHELF_HEADER_H - HEADING_LINE_H;
        assert!((below - 28.0).abs() < f32::EPSILON);
        assert!(
            HANG > below,
            "a header must sit nearer the shelf it names ({below}) than the \
             one it follows ({HANG})"
        );
        // …but not so much nearer that the break stops reading as a break: the
        // header owns more air than the label under a sleeve does.
        assert!(below > GAP_LG);
        // The heading is the smallest type in the product, below the caption.
        const { assert!(SIZE_HEADING < SIZE_CAPTION) }
    }

    /// **The index rail costs the wall three tokens and no new number**, and
    /// its right edge is the window's one gutter.
    #[test]
    fn the_index_rail_borrows_every_edge_it_stands_on() {
        // What the wall gives up is the clearance, the lane and the gutter.
        assert!((INDEX_LANE_W - (INDEX_CLEARANCE + INDEX_W + HANG)).abs() < f32::EPSILON);
        assert!((INDEX_LANE_W - 108.0).abs() < f32::EPSILON);
        // The lane is the width ADR-0017 §1.7 gives it **as amended** — 60,
        // where 36 clipped `Unknown`, every recency bucket and most genres — and
        // the gutter to the window's edge is `HANG`, the one gutter every
        // window-edge surface hangs from (law L1). The `Settings` word above the
        // rail is set against the same x, and so is the last column of covers.
        assert!((INDEX_W - 60.0).abs() < f32::EPSILON);
        const { assert!(INDEX_CLEARANCE == GAP_SM) }
        // The clearance really does clear the scrollbar, which sits in the
        // grid's right margin immediately left of the lane.
        const { assert!(INDEX_CLEARANCE > 0.0 && INDEX_CLEARANCE < SCROLLBAR_LANE) }
        // A rail entry's pitch is its line box and a gap, both on the 4 px
        // lattice, and 27 letters fit the shortest wall a window can produce
        // (860 px of window less the two bars).
        assert!((RAIL_LINE_H - LINE_HEADING).abs() < f32::EPSILON);
        assert!((RAIL_PITCH - (RAIL_LINE_H + GAP_SM)).abs() < f32::EPSILON);
        const { assert!(27.0 * RAIL_PITCH < 640.0) }
    }

    /// **The fisheye is a lens, measured**: largest under the pointer, falling
    /// off smoothly and symmetrically, at rest beyond its reach — and its
    /// largest letter still fits inside one slot, so the strip never has to
    /// move anything to make room (ADR-0020's amendment).
    #[test]
    fn the_fisheye_peaks_under_the_pointer_and_rests_beyond_its_reach() {
        // The peak is the peak, and only at zero distance.
        assert!((magnify(0.0) - MAGNIFY_MAX).abs() < 1e-6);
        // A lens has no upstream side.
        for distance in [0.5, 7.0, 19.5, 33.0, MAGNIFY_REACH - 0.5] {
            assert!((magnify(distance) - magnify(-distance)).abs() < 1e-6);
        }
        // Monotone all the way out: no letter is ever larger than one nearer
        // the pointer, which is what makes the swell read as one lens rather
        // than a ripple.
        let mut previous = magnify(0.0);
        let mut distance = 0.0;
        while distance <= MAGNIFY_REACH + 2.0 {
            let scale = magnify(distance);
            assert!(
                scale <= previous + 1e-6,
                "the falloff climbs at {distance}: {scale} after {previous}"
            );
            assert!((1.0..=MAGNIFY_MAX).contains(&scale));
            previous = scale;
            distance += 0.25;
        }
        // Rest at the reach exactly and everywhere past it — no seam where the
        // swell meets the rest of the strip, and no letter that never quite
        // settles.
        assert!((magnify(MAGNIFY_REACH) - 1.0).abs() < 1e-6);
        assert!((magnify(MAGNIFY_REACH - 0.25) - 1.0).abs() < 1e-4);
        assert!((magnify(4096.0) - 1.0).abs() < f32::EPSILON);
        // The reach is slots, not an unrelated number: three each side.
        const { assert!(MAGNIFY_REACH == 3.0 * RAIL_PITCH) }
    }

    /// **The displacement is the falloff's integral, measured as one**: the
    /// dock's mechanism is that each gap stretches by the magnification across
    /// it, and every property the widget leans on follows from that.
    #[test]
    fn the_fisheye_displaces_by_the_area_under_its_own_falloff() {
        // Anchored: the entry under the pointer does not move, and the lens
        // has no upstream side.
        assert!(magnify_shift(0.0).abs() < 1e-6);
        for distance in [3.0, 17.0, 41.0, MAGNIFY_REACH, 200.0] {
            assert!((magnify_shift(distance) + magnify_shift(-distance)).abs() < 1e-4);
        }
        // The integral relation itself, checked numerically: over any small
        // step, the shift grows by the mean of `magnify − 1` across it.
        let mut distance = 0.0;
        while distance < MAGNIFY_REACH {
            let step = 0.25;
            let grew = magnify_shift(distance + step) - magnify_shift(distance);
            let mean = f32::midpoint(magnify(distance), magnify(distance + step)) - 1.0;
            assert!(
                (grew - mean * step).abs() < 1e-3,
                "at {distance}: the shift grew {grew}, the falloff says {}",
                mean * step
            );
            distance += step;
        }
        // Monotone, order-preserving, and hit-order preserving: the displaced
        // position `d + shift(d)` strictly grows, so entries never cross and
        // the nearest displaced centre is the nearest rest centre.
        let mut previous = f32::MIN;
        let mut d = -(MAGNIFY_REACH + 40.0);
        while d <= MAGNIFY_REACH + 40.0 {
            let displaced = d + magnify_shift(d);
            assert!(displaced > previous, "entries cross at {d}");
            previous = displaced;
            d += 0.5;
        }
        // Saturation: everything at or beyond the reach moves as one piece, by
        // exactly the budgeted spread.
        assert!((magnify_shift(MAGNIFY_REACH) - MAGNIFY_SPREAD).abs() < 1e-3);
        assert!((magnify_shift(4096.0) - MAGNIFY_SPREAD).abs() < 1e-3);
        const { assert!(MAGNIFY_SPREAD == (MAGNIFY_MAX - 1.0) * MAGNIFY_REACH / 2.0) }
    }

    /// The tracking is inserted **between** characters and nowhere else.
    ///
    /// Total on every input: the empty string, one character, and a string
    /// whose characters are outside the BMP (a `char` is a scalar value, not a
    /// byte, so this must not split anything).
    #[test]
    fn tracking_goes_between_characters_and_never_after_them() {
        assert_eq!(tracked(""), "");
        assert_eq!(tracked("A"), "A");
        assert_eq!(tracked("AB"), format!("A{TRACKING}B"));
        assert_eq!(tracked("ARTIST").chars().count(), 6 + 5);
        assert!(!tracked("ARTIST").ends_with(TRACKING));
        assert!(!tracked("ARTIST").starts_with(TRACKING));
        // Non-Latin and astral input survives unsplit — a rail entry can be a
        // CJK initial, and a genre tag can be anything at all.
        assert_eq!(tracked("曲人"), format!("曲{TRACKING}人"));
        assert_eq!(tracked("𝄞𝄢"), format!("𝄞{TRACKING}𝄢"));
        // And it is exactly one character, which is what makes the width
        // arithmetic in `font.rs` one advance per gap.
        assert_eq!(TRACKING.chars().count(), 1);
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
        // The block is **one control height** with the fader centred in it, so
        // it is one of law L7's two heights rather than a third, and it fits
        // the 56 px band the bar became at step 10 (60 did not).
        const { assert!(VOLUME_ROW_H == TRANSPORT_HIT) }
        const { assert!(VOLUME_HIT < VOLUME_ROW_H) }
        const { assert!(VOLUME_ROW_H <= BAR_CONTENT_H) }
        // **The mute glyph sits on the rail, and it does so by symmetry.**
        // The block's centre *is* the fader's rail centre, so a mute button
        // centred in the block lands on it — where the shipped build bought the
        // same alignment with a `MUTE_TOP` offset that had to be re-derived
        // whenever either lane changed. This is also law L4's right-hand mark:
        // centring the block centres the rail, and the rail is what the bar's
        // one line has to carry.
        //
        // It used to be bought with two 16 px lanes, a preview lane above the
        // fader and an empty one below it. The preview became a *layer* over
        // the bar's own lead, which is what freed 28 px of the block — and a
        // hit band centred in a square is the same symmetry with two fewer
        // numbers in it.
        assert!(
            ((VOLUME_ROW_H - VOLUME_HIT) / 2.0 - (VOLUME_ROW_H / 2.0 - VOLUME_HIT / 2.0)).abs()
                < f32::EPSILON,
            "the volume block is not symmetric about the fader's rail"
        );
        // And the lane the level tip floats in is real: the bar's lead above
        // the block, plus the fader's own slop above its rail.
        const { assert!(PREVIEW_H <= BAR_LEAD + (VOLUME_HIT - RAIL) / 2.0) }
        // The level tip must hold `-18.1 dB` — four figures at caption size,
        // plus the proportional remainder `crate::font` measures — without
        // clipping.
        const { assert!(LEVEL_W > SIZE_CAPTION * 4.0 * DIGIT_EM) }
        // And the whole right-hand end has to fit beside the centre column
        // in the shipped window, or the zone would clip on launch.
        const { assert!(VOLUME_BLOCK_W + GAP_SM + SIGNAL_W < 1280.0 - TRANSPORT_W) }
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
            // Both readings are **opaque** now (the marks are pre-composited,
            // see `Palette::ink_over`), so the difference between them is a
            // difference in light rather than in alpha — and it has to be a
            // large one, because "at unity" and "a pixel below unity" are told
            // apart on sight and nothing else.
            let step = (luminance(engaged) + 0.05) / (luminance(rest) + 0.05);
            assert!(
                !(0.5..=2.0).contains(&step),
                "{}: the detent's engaged reading is {step:.2}× its resting one",
                p.name
            );
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
        const { assert!(2.0 * TRANSPORT_HIT + GAP_SM < TRANSPORT_W) }
        // …and it is *only* a target: the chrome went and the square stayed.
        // The floor is a property of the button's bounds, which no amount of
        // paint can shrink, so the two claims are independent and both are
        // asserted rather than one being read as evidence of the other.
        const { assert!(TRANSPORT_HIT >= 32.0) }
        const { assert!(STEPPER_HIT < TRANSPORT_HIT) }
    }

    /// **An icon button at rest is the glyph and nothing else.**
    ///
    /// The owner's complaint, in a test: *"the clunky looking button styles
    /// around what should be icon buttons"*. `docs/design/04-fluidity.md`
    /// measured what was actually wrong with them, and it was worse than a
    /// border — rest and disabled were **pixel-identical** at 1.10 : 1, so a
    /// dead Previous looked exactly like a live one; press set the fill to the
    /// bar's own recess, so pressing read as a *hole* punched in the bar; and
    /// hover changed nothing at all, because a `button` style's `text_color`
    /// never reaches the rasterised sprite that draws the mark.
    ///
    /// All three are the same defect: the chrome was carrying a signal it could
    /// not carry. So the chrome goes, the ink takes the whole ladder
    /// ([`glyph_opacity`]), and what is left here is a hover wash and a press
    /// wash over whatever the button is standing on.
    #[test]
    fn an_icon_button_wears_no_chrome_at_rest() {
        let p = active();
        let rest = transport(p, p.recess, button::Status::Active);
        assert_eq!(
            from_background(rest.background),
            vec![Color::TRANSPARENT],
            "an icon button at rest paints a ground: the glyph is the control"
        );
        assert_eq!(
            rest.border.color,
            Color::TRANSPARENT,
            "an icon button at rest draws an edge"
        );

        // Hover and press are ink washes over whatever the button is sitting
        // on, never a surface step: a 32 px square that becomes a panel for as
        // long as a pointer crosses it is the clunk this removes. And press is
        // *not* the recess — the bar is the recess, so that read as a hole.
        for (status, expected) in [
            (button::Status::Hovered, p.ink_wash(p.recess)),
            (button::Status::Pressed, p.ink_wash_press(p.recess)),
        ] {
            let style = transport(p, p.recess, status);
            assert_eq!(from_background(style.background), vec![expected]);
            // A wash is a wash: the mark is *between* its ground and the room's
            // ink, far nearer the ground. It is an opaque colour rather than an
            // alpha (`Palette::ink_over`), so "how strong" is a distance now
            // rather than a number that can be read off the token.
            let reach = (expected.r - p.recess.r) / (p.paper.r - p.recess.r);
            assert!(reach < 0.2, "a wash is a wash: {expected:?} is a fill");
            assert_ne!(expected, p.recess, "a hovered button is not invisible");
            assert_ne!(expected, p.plinth, "a hovered button is not a panel");
            assert_eq!(style.border.color, Color::TRANSPARENT);
        }
        // A press reads as one step firmer than a hover — never as a hole
        // punched in the bar, which is what a `recess` fill was.
        const { assert!(INK_WASH_PRESS_A > INK_WASH_A) }
        let hovered = from_background(transport(p, p.recess, button::Status::Hovered).background);
        let pressed = from_background(transport(p, p.recess, button::Status::Pressed).background);
        assert!(pressed[0].r > hovered[0].r && hovered[0].r > p.recess.r);

        // Disabled is ink alone — no wash under a control that cannot act.
        let dead = transport(p, p.recess, button::Status::Disabled);
        assert_eq!(from_background(dead.background), vec![Color::TRANSPARENT]);

        // **The three readings of a control are three readings.** This is the
        // measured defect, asserted so it cannot come back: rest, waiting and
        // dead were 1.10 : 1 apart when the chrome carried them, and they are a
        // real ramp now that the ink does.
        let live = glyph_opacity(true, false);
        let pending = glyph_opacity(true, true);
        let inert = glyph_opacity(false, false);
        assert!(
            inert < pending && pending < live,
            "{inert} {pending} {live}"
        );
        assert!(
            live - inert > 0.25,
            "a dead control and a live one are {:.2} apart in ink, which is what \
             1.10 : 1 of chrome looked like",
            live - inert
        );
        // Rest leaves the pointer somewhere to go: the hovered reading is a
        // real step above it, not a rounding difference.
        assert!(GLYPH_OPACITY_HOVER / live > 1.5);

        // And every state has the same border geometry and casts no shadow, so
        // the bar's pixel-stability claim survives the restyle untouched.
        for status in [
            button::Status::Active,
            button::Status::Hovered,
            button::Status::Pressed,
            button::Status::Disabled,
        ] {
            let style = transport(p, p.recess, status);
            assert!((style.border.width - 1.0).abs() < f32::EPSILON);
            assert!((style.border.radius.top_left - RADIUS_CTRL).abs() < f32::EPSILON);
            assert_eq!(style.shadow, Shadow::default());
        }
    }

    /// **The shelf contains exactly two kinds of thing: artwork and type.**
    ///
    /// Rule 1 of the direction (`.interface-design/system.md` §1.2), and the
    /// claim it makes for itself is that it is *checkable in one glance at a
    /// screenshot* — so it is worth a test that does not need the screenshot.
    /// A tile paints no ground, no edge, no radius and no shadow in any of its
    /// eight states; a sleeve gains no shadow unless it is the record that is
    /// sounding, in which case it gains light; and the state vocabulary lives
    /// entirely in a rule drawn *under the label*.
    #[test]
    fn the_shelf_draws_only_artwork_and_type() {
        let p = active();
        for status in [
            button::Status::Active,
            button::Status::Hovered,
            button::Status::Pressed,
            button::Status::Disabled,
        ] {
            for selected in [false, true] {
                let style = tile(p, status, selected);
                assert!(
                    style.background.is_none(),
                    "a tile paints a card behind a sleeve ({status:?}, selected={selected})"
                );
                assert!((style.border.width - 0.0).abs() < f32::EPSILON);
                assert!((style.border.radius.top_left - 0.0).abs() < f32::EPSILON);
                assert_eq!(style.shadow, Shadow::default());
            }
        }

        // No artwork casts a shadow. The playing one is lit instead, and the
        // halo is the only shadow primitive left in the product.
        assert_eq!(
            sleeve(p, 0.0).shadow,
            Shadow::default(),
            "a resting sleeve casts a shadow; on near-black that is a 1.04 : 1 rounding error"
        );
        let halo = sleeve(p, 1.0).shadow;
        assert!(
            p.is_accent(halo.color),
            "the halo is the lamp or it is nothing"
        );
        assert_eq!(halo.offset, Vector::ZERO, "light does not fall to one side");
        assert!((halo.blur_radius - HALO_BLUR).abs() < f32::EPSILON);

        // The state vocabulary: two thicknesses and two inks, a 2× step apart,
        // and neither of them the accent.
        assert!((tile_rule_h(0.0, false) - 0.0).abs() < f32::EPSILON);
        assert!((tile_rule_h(1.0, false) - 1.0).abs() < f32::EPSILON);
        assert!((tile_rule_h(0.0, true) - SELECTION_EDGE).abs() < f32::EPSILON);
        assert!(
            tile_rule_h(0.0, true) >= 2.0 * tile_rule_h(1.0, false),
            "hover and selection are nearly the same mark again"
        );
        // Selection outranks hover: pointing at a record you have already
        // opened must not un-mark it.
        assert!((tile_rule_h(1.0, true) - SELECTION_EDGE).abs() < f32::EPSILON);
        // …and the lane is reserved at the thicker of the two, so no state of
        // a tile moves a pixel of the tile beside it.
        const { assert!(SELECTION_EDGE >= 2.0) }
    }

    /// **The accent is never an opaque fill.** the product's standing rules, and the one
    /// control that used to break it.
    ///
    /// `Play album` was a solid lamp rectangle — argued as an exception by the
    /// previous direction, revoked by this one, because under a room this quiet
    /// the slab was the brightest object on screen and it was not the artwork.
    /// What is permitted is a ≤ 6 px mark, a 4 px rail, a 1 px line, or light,
    /// so the accent may reach this control's **border** at full strength and
    /// its **ground** only as a wash.
    #[test]
    fn the_accent_is_a_line_or_a_wash_but_never_a_slab() {
        let p = active();
        for status in [
            button::Status::Active,
            button::Status::Hovered,
            button::Status::Pressed,
            button::Status::Disabled,
        ] {
            let style = primary(p, status);
            for ground in from_background(style.background) {
                assert!(
                    !p.is_accent(ground) || ground.a <= LAMP_WASH_PRESS_A,
                    "`primary` fills with the accent in {status:?}: {ground:?}"
                );
            }
            // A 1 px line in every state, so the control does not resize when
            // the pointer arrives, and the label stays paper — nothing sits on
            // the accent any more.
            assert!((style.border.width - 1.0).abs() < f32::EPSILON);
            assert!(!p.is_accent(style.text_color));
        }
        // At rest it is an outline and nothing else: no fill to hover *off*.
        let rest = primary(p, button::Status::Active);
        assert_eq!(from_background(rest.background), vec![Color::TRANSPARENT]);
        assert!(p.is_accent(rest.border.color));
        // And a control that cannot create playback truth does not wear the
        // colour that means it.
        let dead = primary(p, button::Status::Disabled);
        assert!(!p.is_accent(dead.border.color));
    }

    /// The room casts **one** shadow, and it is light rather than elevation.
    ///
    /// `.interface-design/system.md` §2 deletes the shadow, and this is the
    /// assertion that keeps it deleted: black at 45 % over the wall composites
    /// to a 1.04 : 1 step, so every shadow in the product was a cost with no
    /// signal. Anything that wants to say *in front* says it with a surface
    /// step and a hairline.
    #[test]
    fn nothing_casts_a_shadow_except_the_playing_record() {
        let p = active();
        let mut shadowed: Vec<&'static str> = Vec::new();
        for (name, style) in [
            ("bar", bar(p)),
            ("tooltip", tooltip(p)),
            ("preview_tip", preview_tip(p)),
            ("segmented", segmented(p)),
            ("lamp_dot", lamp_dot(p)),
            ("menu", menu(p)),
            ("sleeve(resting)", sleeve(p, 0.0)),
            ("sleeve(playing)", sleeve(p, 1.0)),
        ] {
            if style.shadow != Shadow::default() {
                shadowed.push(name);
            }
        }
        assert_eq!(
            shadowed,
            vec!["sleeve(playing)"],
            "only the record that is sounding is lit; everything else is surface, edge and ink"
        );
    }

    /// A duration is a figure column, and figure columns are right-aligned in
    /// a slot that does not move (§8.1, §8.2).
    #[test]
    fn a_duration_has_a_lane_of_its_own() {
        // Five figures at the face's real advance, plus the two colons between
        // them, is the worst honest case — `1:59:59`.
        const { assert!(DURATION_W > SIZE_META * 5.0 * DIGIT_EM) }
        // It fits the record page's row beside the number column and the
        // reserved scrollbar lane, at the narrowest the page is ever set…
        const {
            assert!(
                TRACK_NO_W + DURATION_W + SCROLLBAR_LANE + 3.0 * GAP_SM
                    < ALBUM_BREAKPOINT - 2.0 * HANG
            );
        }
        // …and the queue place's, which additionally carries a removal target.
        const {
            assert!(
                TRACK_NO_W + DURATION_W + SCROLLBAR_LANE + STEPPER_HIT + 4.0 * GAP_SM
                    < ALBUM_BREAKPOINT - 2.0 * HANG
            );
        }
        // The Details block's two columns are a table, not prose: the label
        // lane holds the longest field name the block draws, and the values
        // start on one edge whatever it says.
        const { assert!(FIELD_LABEL_W > SIZE_META * 12.0 * 0.5) }
        const { assert!(DETAIL_ROW_H < SIZE_BODY * LEADING_BODY) }
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
    /// `under`, source-over, **in linear light — the space the renderer
    /// actually blends in**.
    ///
    /// This is the whole of ADR-0017 §1.6's second extension. A token
    /// expressed as an alpha is not measurable against a floor until it has
    /// been resolved against the surface it lands on, and a test that sees
    /// only opaque tokens cannot see an unreadable one hiding in an opacity —
    /// which is exactly the failure the critique's "ink opacity is the
    /// hierarchy" would have shipped (its 40 % tier lands between 2.09 : 1 and
    /// 3.24 : 1 across the four rooms).
    ///
    /// **It blended the sRGB components** under a doc comment promising the
    /// renderer's space, which is the CSS model and not this renderer's
    /// (`docs/design/05-toolkit-and-visual-gap.md` D1). So the contrast suite
    /// was measuring a picture the application never drew — conservative for
    /// ink-on-surface legibility, and blind to every hairline, ring and wash,
    /// which were all one to two ink-steps louder than the numbers said. The
    /// transfer functions are `linear`'s and its inverse; `iced_core-0.13.2`'s
    /// `Color::into_linear` is the same curve.
    fn composite(over: Color, under: Color) -> Color {
        let a = over.a.clamp(0.0, 1.0);
        let blend =
            |over: f32, under: f32| encode(a.mul_add(linear(over) - linear(under), linear(under)));
        Color {
            r: blend(over.r, under.r),
            g: blend(over.g, under.g),
            b: blend(over.b, under.b),
            a: 1.0,
        }
    }

    /// One channel of linear light, sRGB-encoded — the inverse of [`linear`].
    fn encode(channel: f32) -> f32 {
        if channel <= 0.003_130_8 {
            channel * 12.92
        } else {
            1.055f32.mul_add(channel.powf(1.0 / 2.4), -0.055)
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
            let selected = p.select_wash(p.recess);
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

            every_precomposited_mark_clears_its_floor(p, &surfaces);
            every_precomposited_mark_is_what_the_renderer_draws(p, &surfaces);
        }
    }

    /// **The pre-composited marks, measured where they are drawn.**
    ///
    /// These four are no longer room-level inks: an alpha over a ground was
    /// drawing at three to four times its specified weight in the dark room and
    /// half of it in the light one, because iced blends in linear light and the
    /// numbers were written for CSS's sRGB blend
    /// ([`Palette::ink_over`]). Each is now an *opaque* colour computed from the
    /// surface it lands on, so a sweep over every surface would be measuring
    /// colours that never meet — the mark and its ground arrive together or not
    /// at all.
    fn every_precomposited_mark_clears_its_floor(
        p: &Palette,
        surfaces: &[(&'static str, Color); 4],
    ) {
        /// The floor for a non-text mark, restated here because the sweep this
        /// law was carved out of holds its own copy.
        const MARK: f32 = 3.0;

        for &(surface_name, surface) in surfaces {
            // `hairline` and `hairline_strong` are on the exemption list — they
            // exist to be locatable and are never read, and are governed by the
            // oklch-L step law instead (ADR-0017 §1.6). `paper_ring` is not:
            // it is the only focus affordance iced 0.13 can draw, so a keyboard
            // user who cannot find it has no other way to know where the
            // keyboard is.
            let ring = p.paper_ring(surface);
            let ratio = contrast(ring, surface);
            assert!(
                ratio >= MARK,
                "{}: paper_ring on {surface_name} is {ratio:.2} : 1, below its \
                 {MARK} : 1 floor",
                p.name
            );
            // And the two exempt marks still have to be *there*: an edge that
            // composited to its own ground would be no edge at all, which is
            // the failure mode the pre-compositing arithmetic could introduce.
            for (name, mark) in [
                ("hairline", p.hairline(surface)),
                ("hairline_strong", p.hairline_strong(surface)),
            ] {
                assert!(
                    (mark.r - surface.r).abs() > 0.004,
                    "{}: {name} on {surface_name} composites to its own ground",
                    p.name
                );
            }
        }
    }

    /// **The drawn value, not the requested one.**
    ///
    /// A regression in the blend space is invisible by construction — the old
    /// test composited in sRGB under a doc comment promising the renderer's
    /// space, and it passed for the whole of the defect's life. So this pins
    /// what the renderer *puts on the glass*: an opaque mark is immune to the
    /// blend space, and that immunity is the thing being asserted.
    fn every_precomposited_mark_is_what_the_renderer_draws(
        p: &Palette,
        surfaces: &[(&'static str, Color); 4],
    ) {
        for &(surface_name, surface) in surfaces {
            for (mark_name, mark) in [
                ("hairline", p.hairline(surface)),
                ("hairline_strong", p.hairline_strong(surface)),
                ("ink_wash", p.ink_wash(surface)),
                ("ink_wash_press", p.ink_wash_press(surface)),
                ("lamp_wash", p.lamp_wash(surface)),
                ("paper_ring", p.paper_ring(surface)),
                ("select_wash", p.select_wash(surface)),
            ] {
                assert!(
                    (mark.a - 1.0).abs() < f32::EPSILON,
                    "{}: {mark_name} on {surface_name} is still an alpha, so \
                         what the renderer draws is not what this test measures",
                    p.name
                );
                // Blending it again — in *either* space — changes nothing,
                // which is the whole property an opaque mark buys.
                let redrawn = composite(mark, surface);
                for (a, b) in [
                    (mark.r, redrawn.r),
                    (mark.g, redrawn.g),
                    (mark.b, redrawn.b),
                ] {
                    assert!((a - b).abs() < 0.001, "{}: {mark_name} moved", p.name);
                }
            }
            // And the weight is the weight the spec asked for: the mark
            // sits between its ground and the room's ink, at the fraction
            // the token names, measured on the encoded ramp the numbers
            // were written on.
            let hair = p.hairline(surface);
            let want = HAIRLINE_A.mul_add(p.paper.r - surface.r, surface.r);
            assert!(
                (hair.r - want).abs() < 0.002,
                "{}: hairline on {surface_name} draws {:.3}, specified {want:.3}",
                p.name,
                hair.r
            );
        }
    }

    /// The two ink corrections and the elevation counter-example, kept beside
    /// the law they exist to justify.
    #[test]
    fn the_shipped_corrections_are_the_failures_this_test_exists_for() {
        /// The floor for text, restated: these are the two failures the floors
        /// above exist to catch, so they are measured against the same numbers.
        const TEXT: f32 = 4.5;
        /// The floor for a non-text mark.
        const MARK: f32 = 3.0;
        /// The smallest elevation step the eye reads as a step.
        const STEP_L: f32 = 0.03;

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
            // A gradient paints every one of its stops. Reading them is what
            // keeps the accent sweep honest now that a style can hand the
            // renderer a ramp instead of a colour ([`veil_row`]).
            Some(Background::Gradient(iced::Gradient::Linear(linear))) => linear
                .stops
                .iter()
                .filter_map(|stop| stop.map(|stop| stop.color))
                .collect(),
            None => Vec::new(),
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
            }
            painted.push(("transport", button_colors(&transport(p, p.recess, status))));
            painted.push((
                "word_button",
                button_colors(&word_button(p, p.wall, status)),
            ));
            painted.push(("primary", button_colors(&primary(p, status))));
            painted.push(("veil_row", button_colors(&veil_row(p, status))));
        }
        for status in slider_states {
            painted.push(("needle", slider_colors(&needle(p, status))));
            painted.push(("needle_inert", slider_colors(&needle_inert(p, status))));
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
            let style = scrollbar(p, p.wall, status);
            painted.push((
                "scrollbar",
                vec![
                    style.vertical_rail.scroller.color,
                    style.vertical_rail.border.color,
                ],
            ));
        }
        painted.extend(every_painted_surface(p));
        painted
    }

    /// The half of the sweep that is containers and rules rather than
    /// controls — split out only so neither half runs past the line budget the
    /// workspace lints hold every function to.
    fn every_painted_surface(p: &Palette) -> Vec<(&'static str, Vec<Color>)> {
        let mut painted: Vec<(&'static str, Vec<Color>)> = Vec::new();
        painted.push(("sleeve(resting)", container_colors(&sleeve(p, 0.0))));
        // Every point of the hover fade, not only its ends: a transition is a
        // hundred and fifty frames' worth of chances to paint something the
        // discipline forbids, and mid-flight is exactly where nobody looks.
        for hovered in [0.0, 0.25, 0.5, 0.75, 1.0] {
            for selected in [false, true] {
                painted.push((
                    "tile_rule",
                    container_colors(&tile_rule(p, hovered, selected)),
                ));
            }
        }
        // The lamp warming, swept the same way and permitted the same way: a
        // halo half way up is still the halo (see [`Palette::lamp_glow_at`]).
        for warmth in [0.25, 0.5, 0.75, 1.0] {
            painted.push(("sleeve(playing)", container_colors(&sleeve(p, warmth))));
        }
        painted.push(("lamp_dot", container_colors(&lamp_dot(p))));
        painted.push(("segmented", container_colors(&segmented(p))));
        painted.push(("preview_tip", container_colors(&preview_tip(p))));
        painted.push(("bar", container_colors(&bar(p))));
        painted.push(("tooltip", container_colors(&tooltip(p))));
        painted.push(("menu", container_colors(&menu(p))));
        painted.push(("hairline", vec![hairline(p, p.wall).color]));
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
    /// playing sleeve's halo, the playing dot, the needle's fill, and the
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
    /// The veil's **specified** opacity at `x` across the sleeve, `x` in `0..=1`:
    /// [`VEIL_SPEC`] read as the piecewise-linear ramp the renderer interpolates.
    ///
    /// In the design's sRGB terms, so it is the number to compare a sampled pixel
    /// against once that pixel has been un-composited — which is what the table in
    /// `docs/design/impl/hover-options/README.md` does. Put it through
    /// [`veil_alpha`] to get what is handed to the renderer.
    fn veil_at(x: f32) -> f32 {
        let x = x.clamp(0.0, 1.0);
        for pair in VEIL_SPEC.windows(2) {
            let [(x0, a0), (x1, a1)] = [pair[0], pair[1]];
            if (x0..=x1).contains(&x) {
                let span = x1 - x0;
                if span <= f32::EPSILON {
                    return a1;
                }
                return ((x - x0) / span).mul_add(a1 - a0, a0);
            }
        }
        VEIL_SPEC[VEIL_SPEC.len() - 1].1
    }

    /// **The veil is solved, not remembered** — and the solve is checked
    /// against a stated reference ground with its residual bounded over the
    /// whole range a sleeve can occupy.
    ///
    /// Three claims, and each of them is a way the veil could be wrong on
    /// screen while every number in the source looked right:
    ///
    /// 1. At [`VEIL_GROUND`] the rendered composite **is** the design's sRGB
    ///    composite, to within a byte. This is the claim [`veil_alpha`] makes.
    /// 2. Away from it the error stays inside 10 / 255 from sRGB 0.15 to 0.95
    ///    — dark sleeves and near-white ones — so the correction is a
    ///    reference rather than a fit to one cover.
    /// 3. The corrected alphas are **larger** than the specified ones for
    ///    every non-zero stop. That is the direction check: this repo's
    ///    remembered lesson is a 3.7× *overdraw*, and applying it here in its
    ///    remembered direction would have thinned a veil that linear light
    ///    already thins. A regression that reintroduced the old reflex would
    ///    fail on this line, with the reason attached.
    #[test]
    fn the_veil_is_solved_against_a_stated_ground_and_its_residual_is_bounded() {
        /// The worst byte error tolerated away from the reference ground, in
        /// the room the wall ships in — a **dark** veil, whose extreme case is
        /// a near-white sleeve and whose divergence there is small.
        const RESIDUAL_DARK: i32 = 10;
        /// The same, in a light room. Reading Room's veil is a near-*white*
        /// ink, so its extreme is a near-black sleeve, and that is where the
        /// sRGB curve and the linear one are furthest apart: the residual
        /// reaches 28 / 255 at the `0.68` stop over an sRGB 0.15 sleeve.
        /// Stated rather than hidden, because a single tolerance covering both
        /// rooms would have been a tolerance that measured neither — and it is
        /// the honest cost of one reference ground, which is the alternative to
        /// re-solving the veil per sleeve every frame.
        const RESIDUAL_LIGHT: i32 = 28;
        let byte = |value: f32| {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "clamped to 0..=255 and rounded on the line it is cast from"
            )]
            let byte = (value.clamp(0.0, 1.0) * 255.0).round() as i32;
            byte
        };
        for room in Room::ALL {
            let p = room.palette();
            for (offset, spec) in VEIL_SPEC {
                let solved = veil_alpha(spec, p.recess, VEIL_GROUND);
                // The direction check. Which way the correction runs is a
                // property of which side of the blend is brighter, not a
                // constant: a veil darker than the ground it is solved
                // against needs *more* alpha in linear light (Closing Time),
                // and one lighter than it needs less (Reading Room). The
                // 3.7× this repo remembers is the second case, and applying
                // it to the first — which is the shipped room — would have
                // thinned a veil that linear light already thins.
                if spec > 0.0 {
                    let moved = solved - spec;
                    let expected = VEIL_GROUND.g - p.recess.g;
                    assert!(
                        moved * expected >= 0.0,
                        "{}: stop {offset} solved to {solved:.4} from a \
                         specified {spec:.2} — the correction ran the wrong \
                         way for a veil at {:.3} over a ground at {:.3}",
                        p.name,
                        p.recess.g,
                        VEIL_GROUND.g
                    );
                }
                for ground in [0.15_f32, 0.25, 0.35, 0.5, 0.65, 0.8, 0.95] {
                    let under = Color::from_rgb(ground, ground, ground);
                    // What the design asked for: an sRGB composite.
                    let intended = Color {
                        r: spec.mul_add(p.recess.r - ground, ground),
                        g: spec.mul_add(p.recess.g - ground, ground),
                        b: spec.mul_add(p.recess.b - ground, ground),
                        a: 1.0,
                    };
                    // What the renderer draws: `composite` blends in linear
                    // light, exactly as iced's shader does.
                    let drawn = composite(alpha(p.recess, solved), under);
                    let error = [
                        (byte(drawn.r) - byte(intended.r)).abs(),
                        (byte(drawn.g) - byte(intended.g)).abs(),
                        (byte(drawn.b) - byte(intended.b)).abs(),
                    ]
                    .into_iter()
                    .max()
                    .unwrap_or(i32::MAX);
                    let floor = if (ground - VEIL_GROUND.g).abs() < f32::EPSILON {
                        1
                    } else if p.recess.g < VEIL_GROUND.g {
                        RESIDUAL_DARK
                    } else {
                        RESIDUAL_LIGHT
                    };
                    assert!(
                        error <= floor,
                        "{}: stop {offset} over an sRGB {ground} sleeve draws \
                         {error}/255 off its intent, past the {floor}/255 this \
                         solve promises",
                        p.name
                    );
                }
            }
        }
    }

    /// **The option ink clears its floor on the veil, over any sleeve.**
    ///
    /// The measurement the brief names: the ink is held against the
    /// *composited* veil, not against the sleeve, and the worst sleeve is the
    /// one that shows through most — paper white in the dark room, black in
    /// the light one. The ink lane stops at [`VEIL_INK_X`] precisely so that
    /// this passes; a lane one stop wider would not.
    #[test]
    fn the_option_ink_clears_its_floor_on_the_veil_over_any_sleeve() {
        /// The AA floor for text — the option labels are read.
        const TEXT: f32 = 4.5;
        /// The floor for a mark — the glyph beside each label.
        const MARK: f32 = 3.0;
        // Each mark is measured where it actually sits. The tightest sleeve
        // the wall draws is the worst case for both, because the same lead
        // and the same glyph are a larger fraction of a smaller work.
        let work = 200.8 - 2.0 * SLEEVE_MAT;
        // The label's far end: the ink lane's right edge, the thinnest veil
        // any type stands on.
        let label_x = VEIL_INK_X;
        // The glyph's far end: the lead plus one icon box.
        let glyph_x = (VEIL_LEAD + ICON_PX) / work;
        for room in Room::ALL {
            let p = room.palette();
            for sleeve in [0.0_f32, 0.25, 0.5, 0.75, 1.0] {
                let under = Color::from_rgb(sleeve, sleeve, sleeve);
                let ground_at = |x: f32| {
                    composite(
                        alpha(p.recess, veil_alpha(veil_at(x), p.recess, VEIL_GROUND)),
                        under,
                    )
                };
                let label = contrast(p.paper, ground_at(label_x));
                assert!(
                    label >= TEXT,
                    "{}: an option's label is {label:.2} : 1 on the veil at \
                     x {label_x} over an sRGB {sleeve} sleeve, below {TEXT} : 1",
                    p.name
                );
                for plays in [false, true] {
                    let glyph = contrast(veil_option_ink(p, plays), ground_at(glyph_x));
                    assert!(
                        glyph >= MARK,
                        "{}: an option's glyph is {glyph:.2} : 1 on the veil \
                         at x {glyph_x:.3} over an sRGB {sleeve} sleeve, below \
                         {MARK} : 1",
                        p.name
                    );
                }
            }
        }
    }

    /// **The veil's geometry is the veil's own stops.**
    ///
    /// The ink lane and the hit band are read out of [`VEIL_SPEC`] rather than
    /// declared, so a stop that moves takes them with it; and the band stops
    /// short of the sleeve, which is what leaves a press outside an option to
    /// open the record's page.
    #[test]
    fn the_veils_geometry_is_read_out_of_its_own_stops() {
        const { assert!(VEIL_INK_X == VEIL_SPEC[2].0) }
        const { assert!(VEIL_BAND_X == VEIL_SPEC[3].0) }
        const {
            assert!(
                VEIL_INK_X < VEIL_BAND_X,
                "the ink lane must end inside the hit band"
            );
        }
        const {
            assert!(
                VEIL_BAND_X < 1.0,
                "a band that reached the sleeve's edge would take the press \
                 that opens the record"
            );
        }
        // The lead, and the room the ink lane has left after it, at the
        // tightest sleeve the wall draws (`shelf.rs`'s Dense column at 1172:
        // art 200.8, work = art − 2 × SLEEVE_MAT).
        let work = 200.8 - 2.0 * SLEEVE_MAT;
        assert!(
            work.mul_add(VEIL_INK_X, -VEIL_LEAD) >= 4.0 * GAP_XL,
            "the ink lane leaves less than 96 px for a glyph and a word"
        );
        // Law L7's floor, per option, at that same tightest sleeve.
        let band = work / f32::from(u8::try_from(VEIL_OPTIONS).unwrap_or(u8::MAX));
        assert!(
            band >= TRANSPORT_HIT,
            "an option's hit band is {band} px, under law L7's {TRANSPORT_HIT}"
        );
    }

    /// **The wall's `Play` is the only glyph that wears the lamp.**
    ///
    /// [`veil_option_ink`] is the single decision, and this is what makes it
    /// one: three of the four options take the room's paper, and the fourth
    /// takes the accent because it is the control that creates playback truth.
    /// It is also the assertion that records the departure from the approved
    /// mockup — `Queue` was drawn in amber there and is paper here, under
    /// the product's *not what is queued*.
    #[test]
    fn the_walls_play_option_is_the_only_glyph_that_wears_the_lamp() {
        for room in Room::ALL {
            let p = room.palette();
            assert_eq!(veil_option_ink(p, true), p.lamp, "{}", p.name);
            assert_eq!(veil_option_ink(p, false), p.paper, "{}", p.name);
            assert!(
                !p.is_accent(veil_option_ink(p, false)),
                "{}: an option that does not sound is wearing the accent",
                p.name
            );
        }
    }

    #[test]
    fn the_lamp_is_spent_only_on_playback_truth() {
        /// The styles §2.1.1 permits the accent in. Nothing may be added here
        /// without the specification changing first.
        const PERMITTED: [&str; 4] = ["sleeve(playing)", "lamp_dot", "needle", "primary"];

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
    /// Two entries are on the list.
    ///
    /// `views/bottom_bar.rs` is §2.1.1's fourth permitted use: the elapsed
    /// timestamp warms to [`Palette::lamp`] while a position has been asked
    /// for and not yet confirmed, because a position being asked for is a
    /// claim about the playhead. It cools the moment the engine answers.
    ///
    /// `icon.rs` is the fifth, and it is the fifth rather than a sixth: the
    /// accent-inked sprite sheet exists for the wall's hover `Play`, which is
    /// the record page's `Play album` moved onto the sleeve — the one control
    /// in the product that *creates* playback truth, and still at most one of
    /// it on screen because at most one tile is hovered. `icon.rs` names the
    /// token to build the sheet; **which glyph is allowed to wear it is
    /// decided in [`veil_option_ink`]**, in this module, under the sweep
    /// above. See `the_walls_play_option_is_the_only_glyph_that_wears_the_lamp`.
    #[test]
    fn the_lamp_is_named_only_where_playback_truth_is_drawn() {
        /// `src`-relative paths that may name an accent token, and why.
        ///
        /// `views/home.rs` is the sixth, and it is the *same* use as the
        /// first: the `CONTINUE` placard's needle is **where the playhead
        /// is** — the third of the three things this test's own sentence
        /// says the lamp means — drawn at the sleeve's measure on the
        /// placard rather than on the artwork. It is one line, on one band,
        /// about one run, and there is at most one interrupted run.
        const PERMITTED: [&str; 3] = ["views/bottom_bar.rs", "icon.rs", "views/home.rs"];

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
            let source = std::fs::read_to_string(&path)
                .expect("a source file baz ships")
                .replace("\r\n", "\n");
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
            "no view names the accent at all — the elapsed stamp's in-flight \
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
            let source = std::fs::read_to_string(&path)
                .expect("a source file baz ships")
                .replace("\r\n", "\n");
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
            let source = std::fs::read_to_string(&path)
                .expect("a source file baz ships")
                .replace("\r\n", "\n");
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

    // =======================================================================
    // The seven composition laws (`.interface-design/system.md` §13)
    //
    // `docs/design/06-composition-audit.md` §9 proposes them and says why each
    // has to be pinned: *a rule which is not pinned drifts*, which the accent
    // discipline and the contrast floors have both already demonstrated from
    // the other side. One test per law, named for it, and each one fails on the
    // thing the audit actually measured rather than on a paraphrase of it.
    //
    // Three of the seven (L1, L3, L5) are read out of the view sources, in the
    // shape `no_monospace_survives_anywhere_in_the_crate` established: they are
    // claims about *composition*, and no style function is involved, so nothing
    // else in the crate can see them.
    // =======================================================================

    /// **L1 — one gutter per window edge.**
    ///
    /// Every surface that touches a window edge hangs from `x = HANG` and
    /// `x = W − HANG`. `GAP_LG` is a gap *between* things and `GAP_XL` is
    /// padding *inside* a panel; neither is ever a window margin.
    ///
    /// The audit's defect 1, and the highest-yield measurement it made: the
    /// chrome hung from 16, the panels from 24 and the collection from 40, so
    /// **nothing in either bar was aligned with anything on the wall, at either
    /// width, by exactly 24 px**. Six of the wall's sixteen x-edges were
    /// singletons because of it.
    ///
    /// There are three such surfaces and this names all three, by the literal
    /// a reviewer would have to change to break it. The queue place was a
    /// fourth until it was absorbed into `Now playing`, whose merged
    /// composition wears no header strip at all — the lane is the route, and
    /// the run's own head states the list (doc 12 §6.4.4).
    #[test]
    fn one_gutter_touches_every_window_edge() {
        let views = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/views");
        let read = |name: &str| {
            std::fs::read_to_string(views.join(name))
                .expect("a view module baz ships")
                .replace("\r\n", "\n")
        };
        // The top strip, in **every** place — they are one frame and must hang
        // from one line, or navigating between them would slide the content
        // area. `views::mod.rs` carries the strip every place that is not the
        // Library wears ([`crate::views::place_header`]); the Library's own
        // strip is `top_bar.rs`.
        let expected = "theme::pad(theme::TOP_BAR_PAD_V, theme::HANG)";
        for name in ["top_bar.rs", "mod.rs"] {
            assert!(
                read(name).contains(expected),
                "{name} no longer hangs its window-edge strip from HANG"
            );
        }
        // …and the three places that share it really do use it, rather than
        // reinventing a strip of their own — Settings included, since doc 10
        // §7 step 8 folded its private copy into the one function.
        //
        // Three of them lead with something other than a bare place name and so
        // spend the `_led` door: the Artist place's own name, which is a
        // runtime string, and the two subject pages — a record's `Artist ›
        // Album` breadcrumb and a playlist's own name — which since
        // *one page, two subjects* (ADR-0024 §A2's arrangement made literal)
        // draw one strip in `page.rs` and hand it a lead each. They are still
        // the *same strip* — that is what the shared function is for, and it is
        // why the breadcrumb did not get a header of its own.
        for (name, strip) in [
            ("page.rs", "place_header_led("),
            ("artist.rs", "place_header_led("),
            // Settings is the one place with a *note* — a statement about
            // itself, not a keyboard hint — so it spends the `_with` form.
            ("settings.rs", "place_header_with("),
        ] {
            assert!(
                read(name).contains(strip),
                "{name} draws a header of its own instead of the frame's"
            );
        }
        // …and the two pages hand that one strip their subject rather than
        // reaching for a strip of their own.
        for name in ["album.rs", "playlist.rs"] {
            let source = read(name);
            assert!(
                source.contains("lead:") && !source.contains("place_header"),
                "{name} draws a header beside the composition's"
            );
        }
        // The now-playing bar. Its vertical padding is zero because the band is
        // `BAR_CONTENT_H` and the lane that centres the transport is inside it.
        assert!(
            read("bottom_bar.rs").contains("theme::pad(0.0, theme::HANG)"),
            "the bottom bar no longer hangs from HANG"
        );
        // The Settings **place** — a place fills the window, so its content
        // hangs from the window's gutter and not from a panel's padding.
        assert!(
            read("settings.rs").contains("const PLACE_PAD: f32 = theme::HANG;"),
            "the Settings place no longer hangs from HANG"
        );
        // And the index rail, which is the wall's own right-hand edge. Its
        // gutter lives in the rail's own widget now (`crate::spine` draws the
        // lane; the view only says what the lane holds), and the law is the
        // same one: the ink's right edge is `W − HANG`.
        let spine = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/spine.rs");
        let spine = std::fs::read_to_string(spine)
            .expect("the index rail's widget")
            .replace("\r\n", "\n");
        assert!(
            spine.contains("bounds.width - theme::HANG"),
            "the index rail no longer hangs from HANG"
        );
        // **The wall's bar does not compete with the rail** (the owner's
        // decision, 2026-08-09, rewriting ADR-0022's two-vertical-strips
        // entry). The wall draws a bar now, and what this holds down is that it
        // stays the lesser of the two marks against that edge: narrower than
        // every list's, and far narrower than the rail's ink. Asserted as
        // *which geometry the wall asks for* rather than as the presence of a
        // `scrollable`, which is what it always was.
        //
        // It is [`shelf_scrollbar`] and not [`wall_scrollbar`] because the
        // wall's bar rides the *window's* edge: same 4 px, the rail's lane
        // added to what it reserves.
        assert!(
            read("shelf.rs").contains("theme::shelf_scrollbar()"),
            "the wall asks for some other scrollbar geometry than its own"
        );
        const {
            assert!(WALL_SCROLLBAR_W > 0.0);
            assert!(WALL_SCROLLBAR_W < SCROLLBAR_W);
            assert!(WALL_SCROLLBAR_W * 4.0 < INDEX_W);
        }
        // The bar on the window's edge does **not** move the rail's ink off
        // `W − HANG`: it is drawn inside the rail's own gutter, and 4 < 40.
        const { assert!(WALL_SCROLLBAR_W < HANG) }
        // The lane arithmetic agrees: the rail's gutter is the same token.
        const { assert!(INDEX_LANE_W == INDEX_CLEARANCE + INDEX_W + HANG) }
        // A panel's *internal* padding is a different edge and stays `GAP_XL`:
        // the law is about window edges, and collapsing the two would be the
        // opposite error.
        const { assert!(GAP_XL != HANG) }
    }

    /// **The playlist's sleeve obeys the laws its sizes touch**
    /// (ADR-0024 §A1–§A2): the panel tile is on the 4 px lattice (L2) and
    /// splits into whole-pixel collage cells; the page tile is `ART_MAX`
    /// exactly, so the full-bleed single is the album page's own bound and a
    /// collage cell half of it — *no artwork is ever drawn larger than its
    /// source*, by arithmetic.
    #[test]
    fn the_playlist_sleeve_sizes_hold_the_artwork_laws() {
        const { assert!(PANEL_SLEEVE == 40.0) }
        const { assert!(PANEL_SLEEVE % 4.0 == 0.0) }
        const { assert!(PANEL_SLEEVE % 2.0 == 0.0) }
        const { assert!(PANEL_SLEEVE < ART_MIN) }
        // The hero sleeve is the album's: one bound, not a second number —
        // both spellings of 320 pinned so neither can drift alone.
        const { assert!(ART_MAX == 320.0) }
        const { assert!(crate::art::THUMB_PX == 320) }
    }

    /// **L2 — the vertical unit is 4, and the type is inside it.**
    ///
    /// Every gap, every reserved slot height, every control height **and every
    /// line box** is an exact multiple of 4. A leading is chosen so that
    /// `size × leading` is a multiple of 4, not the other way round.
    ///
    /// This is the audit's defect 3 and its cause in one test. Pooled over the
    /// whole application a 4 px lattice caught 77–80 % of the drawn chrome edges
    /// against a 75 % null — indistinguishable from chance — and the reason was
    /// that the six line boxes were 15.95, 16.20, 18.20, 20.25, 22.80 and 32.20.
    /// Compile-time, so it fails the build rather than the review.
    #[test]
    fn the_vertical_unit_is_four_and_the_type_is_in_it() {
        /// Whether `value` is a whole multiple of the base unit.
        ///
        /// The round trip through `i32` is the whole test — a value on the
        /// lattice survives it exactly, and one off it does not — so the three
        /// pedantic lints it trips are the mechanism rather than a hazard. Every
        /// input is a spacing token between 0 and a few hundred.
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_precision_loss,
            clippy::float_cmp,
            reason = "truncate-and-compare *is* the lattice test, over values \
                      that are small non-negative token constants"
        )]
        const fn on_lattice(value: f32) -> bool {
            value >= 0.0 && (value / 4.0) as i32 as f32 * 4.0 == value
        }
        // The six line boxes the audit tabulated, and the seventh the scale has
        // below them.
        const {
            assert!(on_lattice(LINE_CAPTION));
            assert!(on_lattice(LINE_META));
            assert!(on_lattice(LINE_BODY));
            assert!(on_lattice(LINE_EMPHASIS));
            assert!(on_lattice(LINE_TITLE));
            assert!(on_lattice(LINE_HERO));
            assert!(on_lattice(LINE_HEADING));
        }
        // …and the leading really is derived from the box rather than beside it,
        // which is the half of the law that stops the next size drifting off.
        for (size, line, leading) in [
            (SIZE_CAPTION, LINE_CAPTION, LEADING_CAPTION),
            (SIZE_META, LINE_META, LEADING_META),
            (SIZE_BODY, LINE_BODY, LEADING_BODY),
            (SIZE_EMPHASIS, LINE_EMPHASIS, LEADING_EMPHASIS),
            (SIZE_TITLE, LINE_TITLE, LEADING_TITLE),
            (SIZE_HERO, LINE_HERO, LEADING_HERO),
            (SIZE_HEADING, LINE_HEADING, LEADING_HEADING),
        ] {
            assert!(
                (size * leading - line).abs() < 1e-3,
                "{size} px at {leading} draws a {} px line box, not {line}",
                size * leading
            );
        }
        // Every spacing token.
        const {
            assert!(on_lattice(GAP_XS));
            assert!(on_lattice(GAP_SM));
            assert!(on_lattice(GAP_MD));
            assert!(on_lattice(GAP_LG));
            assert!(on_lattice(GAP_XL));
            assert!(on_lattice(HANG));
        }
        // Every reserved slot height the interface draws into.
        const {
            assert!(on_lattice(LABEL_H));
            assert!(on_lattice(CAPTION_H));
            assert!(on_lattice(CAPTION_LINE_H));
            assert!(on_lattice(CONTINUATION_H));
            assert!(on_lattice(SETTING_NOTE_H));
            assert!(on_lattice(DETAIL_ROW_H));
            assert!(on_lattice(RAIL_HIT));
            assert!(on_lattice(VOLUME_HIT));
            assert!(on_lattice(VOLUME_ROW_H));
            assert!(on_lattice(PREVIEW_H));
            assert!(on_lattice(SHELF_HEADER_H));
            assert!(on_lattice(RAIL_LINE_H));
            assert!(on_lattice(RAIL_PITCH));
            assert!(on_lattice(BAR_LEAD));
            assert!(on_lattice(BAR_CONTENT_H));
            assert!(on_lattice(NEEDLE_HIT));
            assert!(on_lattice(ALBUM_GAP));
            assert!(on_lattice(SEGMENT_MIN));
            assert!(on_lattice(NEEDLE_TIP_W));
        }
        // Every control height.
        const {
            assert!(on_lattice(TRANSPORT_HIT));
            assert!(on_lattice(STEPPER_HIT));
            assert!(on_lattice(ICON_PX));
        }
        // The two numbers the quantisation was *for*: a wall label is exactly
        // one hang, and the tile's pitch is therefore `art + 96`.
        const { assert!(LABEL_H == HANG) }
        const { assert!(GAP_LG + LABEL_H + HANG == 96.0) }
        // GAP_XXS is the one deliberate half-step in the ladder — an intra-block
        // line gap, never a slot — and it is named rather than silently on the
        // lattice, because a law with an unnamed exception is a habit.
        const { assert!(GAP_XXS == 2.0 && !on_lattice(GAP_XXS)) }
        // …and the needle's track gap **is** that exception rather than a
        // second one: the segments are one line, not a row of slots, so the gap
        // between two of them is an intra-block gap by the same reading. The
        // album gap is a slot-scale break and is on the lattice, which is why
        // the critique's 6 became 8 (see [`ALBUM_GAP`]).
        const { assert!(SEGMENT_GAP == GAP_XXS) }
    }

    /// **L3 — optical centring: the box centres the ink, not the line box.**
    ///
    /// Content shorter than the box that holds it is centred in **both** axes by
    /// the box. A `button` with a fixed height always states its content's
    /// vertical alignment.
    ///
    /// The audit's defect 4, and it found the same bug twice: `Settings` sat
    /// **6.4 px** above its own centre (so its baseline was 8 px off the counts
    /// line it shares a row with) and `Play album` **6.0 px** above and
    /// **86.5 px** left of its. Both were a `button` with a fixed `height` and
    /// no vertical alignment on its content, which iced 0.13 lays out at the
    /// top. Every glyph in a hit box was already centred to a pixel, which is
    /// what makes these two a locatable mistake rather than a habit.
    ///
    /// Read from the sources: a fixed control height is a layout decision, and
    /// no style function can see whether the thing inside it was aligned.
    #[test]
    fn a_fixed_box_states_how_its_content_is_centred() {
        let views = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/views");
        let mut offenders: Vec<String> = Vec::new();
        for path in rust_sources(&views) {
            let source = std::fs::read_to_string(&path)
                .expect("a view module baz ships")
                .replace("\r\n", "\n");
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            for (at, _) in source.match_indices(".height(Length::Fixed(theme::TRANSPORT_HIT))") {
                // The alignment may be stated on the content (composed *above*
                // the call that fixes the box) or on the box itself (chained
                // after it), so the window is the whole element expression —
                // from the previous blank line to the next one.
                let head = source[..at].rfind("\n\n").map_or(0, |index| index + 2);
                let tail = source[at..]
                    .find("\n\n")
                    .map_or(source.len(), |index| at + index);
                let block = &source[head..tail];
                let centred =
                    block.contains("Vertical::Center") || block.contains("Alignment::Center");
                if !centred {
                    let line = source[..at].matches('\n').count() + 1;
                    offenders.push(format!("{name}:{line}"));
                }
            }
        }
        assert!(
            offenders.is_empty(),
            "a control fixes its height and does not say where its content \
             sits in it: {offenders:?}\niced 0.13 lays unaligned content out at \
             the *top* of a fixed box — which is how `Settings` ended up 6.4 px \
             and `Play album` 6.0 px above their own centres."
        );
    }

    /// **L4 — one centre line per bar.**
    ///
    /// A bar has one horizontal centre line and every *mark* in it sits on that
    /// line. Zones are centred by their marks, never by their blocks.
    ///
    /// The geometry is asserted where the bar is composed —
    /// `views::bottom_bar::every_mark_in_the_bar_sits_on_the_bars_one_centre_line`
    /// — because the composition is what the law is about. What belongs here is
    /// the token arithmetic that composition rests on, so the two cannot drift:
    /// if either the band or the lane stops being derived, both tests fail.
    #[test]
    fn the_bar_has_one_centre_line_and_every_mark_is_on_it() {
        // The band's mid-line is the transport's centre line.
        const { assert!(BAR_CONTENT_H / 2.0 == BAR_LEAD + TRANSPORT_HIT / 2.0) }
        // What is below the transport is exactly what is above it — a gap
        // now, where it used to be a gap and a seek row.
        const { assert!(BAR_LEAD == GAP_XL) }
        // The right zone's block is symmetric about its own rail, so centring
        // the block centres the rail. The fader's hit band is centred in a
        // block of one control height, which is the same claim with two fewer
        // constants than the preview-lane-above-and-empty-lane-below it
        // replaces.
        const { assert!(VOLUME_ROW_H == TRANSPORT_HIT) }
        const { assert!(VOLUME_HIT < VOLUME_ROW_H) }
        // The left zone's stack is symmetric about its middle lane, so centring
        // the block centres the zone's own line: 20 · 16 · 20, and the artist's
        // line box is the block's exact centre.
        const { assert!(CONTINUATION_H == LINE_BODY) }
        // Every zone fits the band **with air to spare**, which is the whole of
        // the breathing rule: the band is its tallest zone plus one named gap a
        // side (`BAR_ZONE_LEAD` 12), and the tallest zone is the three stacked
        // line boxes of the now-playing block. There are no gaps *between* the
        // lanes — a line box already carries its own leading, and `GAP_XXS`
        // between them would be a fourth user of the lattice's one exception —
        // so all of the air is taken outside the block, where a lead belongs.
        const { assert!(VOLUME_ROW_H < BAR_CONTENT_H) }
        const { assert!(LINE_BODY + LINE_META + CONTINUATION_H == NOW_PLAYING_H) }
        const { assert!(NOW_PLAYING_H + 2.0 * BAR_ZONE_LEAD == BAR_CONTENT_H) }
        // …and the band lands on two hangs, which is what relates the bar to
        // the composition above it rather than leaving it floating free.
        const { assert!(BAR_CONTENT_H == 2.0 * HANG) }
        // The bar is the band and its hairline; there is no padding left to be
        // asymmetric, which is the cheapest possible way to keep the centring.
        const { assert!(BAR_H == BAR_CONTENT_H + 1.0) }
        // The audit measured a **50 px** spread across seven mark-lines in a
        // 102 px band. The law's ceiling is 2 px; what the tokens prove is that
        // the marks are on one line *exactly*, and the render pass
        // (`composition/tools/census2.py`, "the marks") measures the rest.
    }

    /// **L5 — the permitted alignment edges, per surface.**
    ///
    /// Each surface declares its alignment edges; an element that introduces an
    /// edge outside the list is a defect. This is the same discipline the
    /// contrast exemption list already uses, and it is the measurement that
    /// catches the next regression before a human sees it.
    ///
    /// The audit counted **8 distinct x-edges in the inspector's 340 px column,
    /// 5 of them singletons**, and **four left edges in the popover's 358 px** —
    /// 920, 924, 925, 941 — where two are a composition and four are a leak. In
    /// both cases the extra edges came from one thing: a *row's own horizontal
    /// padding*, applied inside a surface that had already stated its lane.
    /// Both surfaces are **places** now (ADR-0022) and both kept the fix, which
    /// is the point of pinning it as a literal rather than as a count.
    ///
    /// So what is pinned here is that no list row inside a place carries a
    /// horizontal inset of its own. The full edge census is the render
    /// pass (`docs/design/composition/tools/census2.py`), which is where the
    /// counts in the table are read.
    #[test]
    fn every_surface_declares_the_edges_it_permits() {
        let views = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/views");
        let read = |name: &str| {
            std::fs::read_to_string(views.join(name))
                .expect("a view module baz ships")
                .replace("\r\n", "\n")
        };
        // The two lists are the same twelve rows in two surfaces, and both hang
        // from their surface's own content lane.
        for name in ["album.rs", "queue.rs"] {
            let source = read(name);
            assert!(
                source.contains("theme::pad(theme::GAP_XS, 0.0)"),
                "{name}'s rows carry a horizontal inset of their own"
            );
            assert!(
                !source.contains("theme::pad(theme::GAP_XS, theme::GAP_XS)"),
                "{name} still insets a row by GAP_XS, which is the 21-left / \
                 14-right asymmetry the audit measured"
            );
        }
        // The popover's album group sits on the header lane rather than 4 and
        // 5 px inside it. It gained a *vertical* inset when a queue could hold
        // more than one record — the air above a new record's name — and that is
        // a different axis: this law is about x-edges, and the assertion is that
        // the horizontal half of the pad is literally zero.
        assert!(
            read("queue.rs").contains("container(block).padding(theme::pad(air, 0.0))"),
            "the popover's album group has a horizontal inset again"
        );
        // The scrollbar's lane is *declared* rather than absorbed: it is the one
        // inset the right-hand edge is allowed, and it is a token both the bar
        // and the gutter are built from.
        const { assert!(SCROLLBAR_LANE == SCROLLBAR_W) }
    }

    /// **L6 — hierarchy is declared and then measured.**
    ///
    /// Each surface declares what a listener should notice first, second and
    /// third; the measured order — contrast-weighted ink mass over the named
    /// regions — must agree.
    ///
    /// The audit's defect 5 is what this exists for: **the album's name came
    /// fifth of eight in its own inspector**, at 1/164th of the weight of a
    /// picture already on the wall 24 px away, because the sleeve was *the panel
    /// minus its two paddings* and nothing else. The declarations live in
    /// `.interface-design/system.md` §13 and the measurement is the render pass
    /// (`composition/tools/census5.py::ink_mass`), which is a slow test and does
    /// not belong in the unit suite.
    ///
    /// # The inspector's cap is gone, and the defect it fixed is not back
    ///
    /// `INSPECTOR_SLEEVE` 120 held the sleeve down because the panel's sleeve
    /// was **a second, larger copy of a work already on the wall 24 px to the
    /// left**. ADR-0022 replaced the column with a place, so the wall is not on
    /// screen and there is no other copy: the record *is* the subject, and the
    /// record's page declares the work first the way the wall does, saying by
    /// how much (law L6's own escape clause, and the wall is the precedent).
    ///
    /// What must not regress is the rest of the order, and that is what is
    /// asserted here: **the title is the loudest type on the page by a clear
    /// step**, which is the half of defect 5 that was actually about the album's
    /// name. Three type sizes in a falling order, each a real step, with the
    /// title at the top of the whole scale.
    #[test]
    fn the_declared_hierarchy_is_the_geometry_that_produces_it() {
        // The record page's identity block: title ≫ artist ≫ catalogue line.
        const { assert!(SIZE_HERO > SIZE_TITLE && SIZE_TITLE > SIZE_META) }
        // Real steps, not nudges: each is at least a quarter again as large as
        // the one under it, so the ranking survives being measured rather than
        // being read off the source.
        const { assert!(SIZE_HERO >= 1.25 * SIZE_TITLE) }
        const { assert!(SIZE_TITLE >= 1.25 * SIZE_META) }
        // And the title is the top of the scale — there is nothing louder in
        // the product to be beaten by.
        const { assert!(SIZE_HERO >= SIZE_TITLE && SIZE_HERO >= SIZE_EMPHASIS) }
        // The sleeve is the source's own size and no larger, which is the one
        // bound the page's work has (a standing rule of the product: no artwork is ever
        // drawn larger than its source).
        const { assert!(ALBUM_SLEEVE == ART_MAX) }
        // The wall's inversion is deliberate and is declared as such: one sleeve
        // is ~135× its label, and the label is not competing with it. The
        // geometry that says so is the art-to-block ratio, which is stable.
        const { assert!(ART_MIN > 4.0 * LABEL_H) }
    }

    /// **L7 — one control height.**
    ///
    /// Every pointer target is [`TRANSPORT_HIT`] 32 tall. The only exception is
    /// [`STEPPER_HIT`] 24, and it is named.
    ///
    /// The audit's defect 7: the product stood at **five** heights — transport
    /// 32, first-run input 40, search well 30, steppers 24, checkbox 13 — while
    /// publishing a floor of 32, and `theme` asserted `TRANSPORT_HIT >= 32` and
    /// `STEPPER_HIT < TRANSPORT_HIT` and nothing at all about the other three.
    #[test]
    fn the_product_stands_at_one_control_height() {
        // The two heights, and the fact that there are two.
        const { assert!(TRANSPORT_HIT == 32.0) }
        const { assert!(STEPPER_HIT == 24.0) }
        const { assert!(STEPPER_HIT < TRANSPORT_HIT) }
        // A text well is a control: its padding is derived from the height it
        // has to stand at, rather than the height falling out of its padding.
        // 6 + a 20 px line box + 6 = 32, and iced draws the 1 px border inside
        // those bounds rather than outside them — which is the half of the
        // model the shipped build got wrong, and is measured off the render.
        assert!(
            (2.0f32.mul_add(WELL_PAD_V, LINE_BODY) - TRANSPORT_HIT).abs() < f32::EPSILON,
            "a text well no longer stands at the product's one control height"
        );
        // Both wells take the same line box, which is why there is one number
        // and not two — the search field at `SIZE_BODY` and the first-run folder
        // field at `SIZE_EMPHASIS`.
        const { assert!(LINE_BODY == LINE_EMPHASIS) }
        // A checkbox is a pointer target too, and it takes the named secondary
        // square rather than the 13 px box it had.
        let views = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/views");
        let settings = std::fs::read_to_string(views.join("settings.rs"))
            .expect("the Settings place baz ships")
            .replace("\r\n", "\n");
        assert!(
            settings.contains(".size(theme::STEPPER_HIT)"),
            "the checkbox has left the two published control sizes"
        );
        // The one control in baz that is a rail rather than a box takes the same
        // named secondary square: the groove's hit band is `RAIL_HIT`, and it is
        // 24 — so the product really does stand at two heights and not at three
        // with a rail-shaped excuse.
        const { assert!(RAIL_HIT == STEPPER_HIT) }
        // And no view sets a fixed control height that is none of them. The
        // scan is over `Length::Fixed(theme::…_HIT)`, which is how every control
        // in the product states its box.
        let mut offenders: Vec<String> = Vec::new();
        for path in rust_sources(&views) {
            let source = std::fs::read_to_string(&path)
                .expect("a view module baz ships")
                .replace("\r\n", "\n");
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            for (at, _) in source.match_indices("_HIT))") {
                let head = at.saturating_sub(48);
                let window = &source[head..at + 6];
                if window.contains("TRANSPORT_HIT")
                    || window.contains("STEPPER_HIT")
                    || window.contains("RAIL_HIT")
                {
                    continue;
                }
                offenders.push(format!("{name}: …{window}"));
            }
        }
        assert!(
            offenders.is_empty(),
            "a control stands at a third height: {offenders:#?}"
        );
    }

    /// **L9 — a strip declares its tenants and holds them at its floor**
    /// (`.interface-design/system.md` §13, via doc 10 §2.3's charter and
    /// ADR-0026 §3).
    ///
    /// The Library strip's population was bounded by nothing: L8 admits by
    /// subject, each admission was locally argued, and at the shipped window
    /// the tenants claimed 97.6 % of the line — so a transient scan note
    /// pushed the only route to Settings off the window's edge. The law is
    /// the budget as **const arithmetic**, in the shape the bottom bar
    /// already uses for its own floor
    /// (`views/bottom_bar.rs::the_transport_row_is_the_column_it_used_to_be_centred_in`):
    /// every tenant's reserved width is declared in `views::top_bar`, the
    /// sum plus the frame's gutters must fit the declared single-line floor,
    /// and the two-line pair must fit the strip's floor. `font.rs` holds the
    /// measured words against the declarations, so the two halves — face and
    /// arithmetic — cannot drift apart.
    #[test]
    fn the_strip_holds_its_tenants_at_the_single_line_floor() {
        use crate::views::top_bar;

        /// The `Playlists` door's own reserved width, kept here as a
        /// measurement rather than in `views::top_bar` as a reservation:
        /// there is no door left to reserve for, and a token nothing draws
        /// with is a comment that can rot.
        const PLAYLISTS_DOOR_W: f32 = 64.0;
        /// What the door gave back to the strip: its width and the `GAP_XL`
        /// seam beside it.
        const FREED: f32 = PLAYLISTS_DOOR_W + GAP_XL;
        /// What the two words gave back on 2026-08-10, as the drop in the acts
        /// cluster's declared width: `Pull` removed (182 → 144), then `Shuffle`
        /// moved to the now-playing bar when it became a property of the player
        /// (144 → 88).
        const ACTS_FREED: f32 = 182.0 - top_bar::ACTS_W;
        /// What the arrangement row has spent, as the rise in that cluster's
        /// declared width — **46 px, for `A–Z` back in the row and first in it**
        /// (ADR-0035's third amendment).
        ///
        /// Stated as a difference from the five-word 314 rather than as a
        /// literal, because what a *word* costs the strip is the number the
        /// next one is argued against. It was 54 the last time the row was six,
        /// when the sixth word was `ARTISTS`; a word's price is the word's, so
        /// this one is measured (`font.rs`) rather than inherited.
        const KEYS_SPENT: f32 = top_bar::KEYS_W - 314.0;
        /// The window a strip at its floor now needs, with the lane's rail
        /// always beside it.
        const STRIP_FLOOR_WINDOW: f32 = TOP_BAR_FLOOR + SIDEBAR_RAIL_W;

        /// The single-line regime **with the well** (doc 10 §4.2): the gutter,
        /// the well, the two `GAP_XL` seams and the left cluster, the fill's
        /// two `GAP_SM` flanks (the status lead), the gear, the gutter.
        const SINGLE_LINE: f32 = HANG
            + top_bar::WELL_W
            + GAP_XL
            + top_bar::KEYS_W
            + GAP_XL
            + top_bar::ACTS_W
            + 2.0 * GAP_SM
            + TRANSPORT_HIT
            + HANG;

        /// The single-line regime **without it** — the strip at every width
        /// the returns lane can hold the well, which is
        /// [`SIDEBAR_FLOOR`] and above.
        const SINGLE_LINE_NO_WELL: f32 = SINGLE_LINE - top_bar::WELL_W - GAP_XL;

        /// The narrowest strip that regime can be handed: the window at the
        /// lane's floor, less the lane at its full width.
        const WIDEST_LANE_STRIP: f32 = SIDEBAR_FLOOR - SIDEBAR_W;

        /// The **widest** strip that can still be carrying the well: the
        /// window one step below the lane's floor, less the collapsed lane.
        /// Above this the well is the lane's ([`strip_holds_the_well`]), so
        /// this is the ceiling of the band the split has to fit under for a
        /// single-line-with-well regime to exist at all.
        const WIDEST_STRIP_WITH_WELL: f32 = SIDEBAR_FLOOR - SIDEBAR_RAIL_W;

        /// The frame line below the split (doc 10 §4.3): the well, the
        /// empty status slot's flanks, the gear — `GAP_LG` between the row's
        /// four members.
        const FRAME_LINE: f32 = HANG + top_bar::WELL_W + 3.0 * GAP_LG + TRANSPORT_HIT + HANG;

        /// The library line: the states, one seam, the acts.
        const LIBRARY_LINE: f32 = HANG + top_bar::KEYS_W + GAP_XL + top_bar::ACTS_W + HANG;

        const { assert!(FRAME_LINE <= TOP_BAR_FLOOR) }
        const { assert!(LIBRARY_LINE <= TOP_BAR_FLOOR) }

        // **The split is an exact sum, not a rounded one.** It was 960 for a
        // line of 958. The `Playlists` door's 64 px and the `GAP_XL` beside it
        // went with ADR-0030 §5 — 88 px — taking it to 872; then both draw
        // words went on 2026-08-10 on the owner's decision, taking `ACTS_W`
        // from 182 to 88 and this seam to 778; then the arrangement row gained
        // a sixth word, spending 54 of that back and taking the seam to 832;
        // then that word became the first key's own grouping (ADR-0035) and
        // gave all 54 back, returning the seam to 778; and now the row is six
        // again with `A–Z` restored as a key of its own, first in it, which
        // costs 46 of the 94 and puts the seam at **824**. The seam follows
        // the tenants in **both** directions, because a split declared where
        // the line still fits would put the strip on two rows for controls it
        // no longer draws, and one declared where the line no longer fits
        // would let a word run off the window's edge.
        const { assert!(FREED == 88.0) }
        const { assert!(ACTS_FREED == 94.0) }
        const { assert!(KEYS_SPENT == 46.0) }
        const { assert!(SINGLE_LINE == 824.0) }
        const { assert!(SINGLE_LINE + FREED + ACTS_FREED - KEYS_SPENT == 960.0) }
        const { assert!(SINGLE_LINE == TOP_BAR_SPLIT) }
        // **And the two six-word rows are not the same six-word row.** The
        // earlier costing measured 368 for a sixth word that was `ARTISTS`;
        // this one is `A–Z`, 8 px cheaper in its declaration and the reason
        // every figure below is re-derived rather than read off that table.
        const { assert!(KEYS_SPENT < 54.0) }

        // **The two-line split still earns its keep, and no removal bought it
        // away.** The frame line is what the door and the well stood on, so
        // those removals made *that* line cheaper — but the split exists for
        // the **library** line, and both draw words were on it. The line now
        // comes to 552 against a floor of 600, so it fits **under** the floor
        // with 48 px to spare rather than meeting it exactly; the floor does
        // not follow, because it is also the window's sensible minimum (see
        // [`TOP_BAR_FLOOR`]). Between 600 and 824 there is still no single line
        // that fits and a two-line pair that does, which is the band the split
        // serves.
        //
        // **The 94 px the acts cluster freed is what the sixth word came out
        // of**, and both halves are stated as differences of the movements
        // that produced them rather than as numbers, so neither can be right by
        // coincidence: the acts gave 94 back, the arrangement row has spent 46
        // of it on `A–Z`, and 48 is what is left under the floor. A seventh
        // word is argued against that 48, and the two prices already on record
        // — 46 for `A–Z`, 54 for `ARTISTS` — are what it would be estimated
        // from.
        const { assert!(LIBRARY_LINE == 552.0) }
        const { assert!(TOP_BAR_FLOOR - LIBRARY_LINE == ACTS_FREED - KEYS_SPENT) }
        const { assert!(ACTS_FREED - KEYS_SPENT == 48.0) }
        const { assert!(LIBRARY_LINE < TOP_BAR_FLOOR) }
        const { assert!(TOP_BAR_FLOOR < SINGLE_LINE) }
        // The strip's width is the *body's* — the window less the returns
        // lane — so the split's band is reached at a wider window than
        // before: `TOP_BAR_SPLIT + SIDEBAR_RAIL_W` collapsed, and
        // `+ SIDEBAR_W` open. The floor a window must clear for the strip to
        // hold its tenants at all rises with it.
        const { assert!(STRIP_FLOOR_WINDOW == 696.0) }

        // **And the band the split serves is exactly the band the well is
        // still a tenant of.** Once the well is the lane's the strip wants
        // 608 px, and the narrowest strip that can happen in is 720 — the
        // lane's own floor less the lane's own width. So the strip is one line
        // at every width above `SIDEBAR_FLOOR`, in either lane state, and
        // `top_bar_h`'s `strip_holds_the_well` branch is a fact rather than a
        // hope. It was 648 before the two draw words left and 554 after; a
        // sixth word put 54 of that back for one release, ADR-0035 took it out
        // again, and `A–Z` now puts 46 back — **600**, with 120 px of headroom
        // against the narrowest strip the regime can be handed. That it lands
        // on `TOP_BAR_FLOOR`'s own figure is a coincidence of two independent
        // sums and is asserted as an inequality against 720, which is the
        // claim that matters.
        const { assert!(SINGLE_LINE_NO_WELL == 600.0) }
        const { assert!(WIDEST_LANE_STRIP == 720.0) }
        const { assert!(SINGLE_LINE_NO_WELL < WIDEST_LANE_STRIP) }
        const { assert!(WIDEST_LANE_STRIP - SINGLE_LINE_NO_WELL == 120.0) }
        // The rail is wider still, so the collapsed lane cannot reach it either.
        const { assert!(SIDEBAR_FLOOR - SIDEBAR_RAIL_W > WIDEST_LANE_STRIP) }

        // **The single-line-with-well band is wider than it has ever been.**
        // It is asserted because a costing once predicted it would not exist:
        // the proposal kept in `docs/BACKLOG.md` measured a six-word row
        // against a strip whose acts cluster was still 182 px wide and put the
        // split at 926 — *above* the widest strip that can hold the well at
        // all (`SIDEBAR_FLOOR − SIDEBAR_RAIL_W` = 904), which would have
        // deleted the band outright and made the strip two lines at every
        // width below the lane's floor. `Pull` and `Shuffle` left in between
        // and paid for the word twice over. The row is six words again and the
        // band is still there: the seam is 824 and the band is **824…904**,
        // 80 px wide. That is the narrowest it has been since the two draw
        // words left, and it is the figure a seventh word would close.
        const { assert!(WIDEST_STRIP_WITH_WELL == 904.0) }
        const { assert!(SINGLE_LINE < WIDEST_STRIP_WITH_WELL) }
        const { assert!(WIDEST_STRIP_WITH_WELL - SINGLE_LINE == 80.0) }

        // The two-line band is the single-line band's own lead three times
        // around two control rows — 8+32+8+32+8, plus the hairline: 89
        // against 49. A pair of tokens and a breakpoint, not a measurement.
        const { assert!(TOP_BAR_2LINE_H == 3.0 * TOP_BAR_PAD_V + 2.0 * TRANSPORT_HIT + 1.0) }
        const { assert!(TOP_BAR_H == 49.0 && TOP_BAR_2LINE_H == 89.0) }
        const { assert!(TOP_BAR_FLOOR < TOP_BAR_SPLIT) }

        // **The well has one width in the strip, because it can only be drawn
        // in one regime.** Its 80 px fluid range was spent between 1200 and
        // 1280, and the strip is never that wide while the well is in it.
        const { assert!(top_bar::WELL_W == 200.0) }
        const { assert!(SIDEBAR_FLOOR - SIDEBAR_RAIL_W < 1200.0) }

        // And the resolved height is the regime the window and the lane say it
        // is — the function `app.rs`'s viewport estimate reads, asserted at the
        // seam itself. The lane is collapsed at every width the seam is
        // reachable at, so the strip there is the window less the rail.
        let window = |strip: f32| strip + SIDEBAR_RAIL_W;
        assert!((top_bar_h(window(TOP_BAR_SPLIT), false) - TOP_BAR_H).abs() < f32::EPSILON);
        assert!(
            (top_bar_h(window(TOP_BAR_SPLIT - 1.0), false) - TOP_BAR_2LINE_H).abs() < f32::EPSILON
        );
        assert!((top_bar_h(window(TOP_BAR_FLOOR), false) - TOP_BAR_2LINE_H).abs() < f32::EPSILON);
        // Above the lane's floor there is no two-line regime at all, in
        // either state — the defect the old signature could not even express,
        // because it was handed the window while the strip was drawn at the
        // body's width.
        for open in [true, false] {
            for w in [SIDEBAR_FLOOR, 1056.0, 1280.0, 1920.0] {
                assert!(
                    (top_bar_h(w, open) - TOP_BAR_H).abs() < f32::EPSILON,
                    "the strip splits at {w} with the lane open={open}"
                );
            }
        }
    }

    /// **Every icon-only control carries a tooltip** — the form rule's
    /// accessibility clause (doc 10 §3.1; ADR-0017 §4c), a test rather than
    /// an audit (doc 10 §7 step 7), in the source-pinned shape
    /// `views/queue.rs`'s own parity test uses.
    ///
    /// iced 0.13 publishes no accessibility tree, so an icon-only control's
    /// tooltip *is* its accessible name; a glyph button without one is a
    /// control with no name at all. The scan walks every view function that
    /// draws a sprite (`icon::handle`) and requires a `tooltip` in the same
    /// function, with the named exemptions the rule itself makes:
    ///
    /// - **a glyph beside a word is not icon-only** — the word is the name
    ///   (`views::page`'s `commitment`, which is `Play album` on a record and
    ///   `Play` on a made list and was two functions until *one page, two
    ///   subjects*; the strip's `play_all`; the panel's `ghost_row`, whose plus
    ///   stands in the sleeve slot beside the words `New playlist`; and the
    ///   Home place's `resume_line`, whose triangle leads the word `Resume`);
    /// - **the well's magnifier is not a control** — the well is the
    ///   control; the glyph is its label (doc 10 §4.1), and the well itself
    ///   is reachable by every printable key.
    #[test]
    fn every_icon_only_control_carries_a_tooltip() {
        const EXEMPT: [&str; 5] = ["commitment", "play_all", "well", "ghost_row", "resume_line"];
        let views = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/views");
        let mut bare: Vec<String> = Vec::new();
        let mut checked = 0_u32;
        for path in rust_sources(&views) {
            let source = std::fs::read_to_string(&path)
                .expect("a view module baz ships")
                .replace("\r\n", "\n");
            let file = path.file_name().unwrap_or_default().to_string_lossy();
            // Every function item's start and name — a `fn` opening a line,
            // optionally behind a visibility, at any indentation.
            let mut functions: Vec<(usize, String)> = Vec::new();
            for (at, _) in source.match_indices("fn ") {
                let line_start = source[..at].rfind('\n').map_or(0, |index| index + 1);
                let prefix = source[line_start..at].trim();
                if !(prefix.is_empty() || prefix == "pub" || prefix == "pub(crate)") {
                    continue;
                }
                let name: String = source[at + 3..]
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if !name.is_empty() {
                    functions.push((line_start, name));
                }
            }
            for (index, (start, name)) in functions.iter().enumerate() {
                let end = functions
                    .get(index + 1)
                    .map_or(source.len(), |(next, _)| *next);
                let body = &source[*start..end];
                if !body.contains("icon::handle(") {
                    continue;
                }
                // A test that samples sprites is not a control.
                if source[..*start].trim_end().ends_with("#[test]") {
                    continue;
                }
                checked += 1;
                if EXEMPT.contains(&name.as_str()) || body.contains("tooltip") {
                    continue;
                }
                bare.push(format!("{file}::{name}"));
            }
        }
        assert!(
            bare.is_empty(),
            "an icon-only control has no tooltip — no accessible name at \
             all in a toolkit with no accessibility tree: {bare:?}"
        );
        // Not vacuous: the transport, the gear, the row slots and the
        // steppers are all in the walk.
        assert!(
            checked >= 10,
            "the scan found only {checked} sprite-drawing functions — the \
             function walk has stopped seeing the views"
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
