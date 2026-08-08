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
//! [`LAMP`] and its relatives may appear in exactly five places
//! (`docs/design/02-visual-language.md` §2.1.1), and
//! `the_lamp_is_spent_only_on_playback_truth` below is what enforces it rather
//! than leaving it to be remembered:
//!
//! 1. the playing album's halo — [`sleeve`] with `playing`;
//! 2. the playing dot — [`lamp_dot`], beside a tile's title or in a row's
//!    number column;
//! 3. the seek groove's elapsed fill and knob — [`seek`];
//! 4. a seek in flight — the elapsed timestamp warms to [`LAMP`] while a
//!    position has been asked for and not yet confirmed, because a position
//!    being asked for is a claim about the playhead;
//! 5. the primary Play action — [`primary`], the one argued exception: it is
//!    the only control in the product that *creates* playback truth, it
//!    appears at most once per screen, and it is the only lamp-*filled*
//!    rectangle anywhere in baz.
//!
//! Two uses were **cut** in the redesign's first pass, both of them a lamp
//! that was on when nothing was playing: input focus (now [`PAPER_RING`], and
//! the search field takes focus at launch, so the first frame baz ever drew
//! was an amber ring with no music), and the scanning note (now [`PAPER_DIM`]
//! — a scan is the library working, not the music). Blue, every streaming
//! app's accent, remains deliberately absent.
//!
//! # Depth strategy: surface steps, and nothing else
//!
//! Four planes — [`RECESS`] below the wall, [`WALL`], [`PLINTH`] one step up,
//! [`PLINTH_LIT`] one above that — whisper-quiet in bytes (8 apart) and plainly
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
//! # Contrast
//!
//! iced 0.13 publishes no accessibility tree, so contrast and hit-target size
//! are the only accessibility guarantees baz can make — which is a reason to
//! honour them exactly rather than a reason to shrug. Every ink-on-surface
//! pairing the room can produce is computed and checked against its WCAG 2.1
//! floor by `every_ink_clears_its_contrast_floor_on_every_surface_it_lands_on`.

use std::sync::LazyLock;

use iced::font::Weight;
use iced::widget::rule::FillMode;
use iced::widget::slider::{Handle, HandleShape, Rail};
use iced::widget::{button, checkbox, container, rule, scrollable, slider, text_input};
use iced::{Background, Border, Color, Font, Padding, Shadow, Theme, Vector, mouse};

// ---------------------------------------------------------------------------
// Palette
// ---------------------------------------------------------------------------

/// **The hanging wall**: the app background behind the shelf. `#0C0D0E` — a
/// neutral-cool near-black, the matte paint of a black-cube gallery.
///
/// Cool, and that is the single decision that keeps a dark grid of covers from
/// reading as every other media app: the room is cold and the paper is warm
/// ([`PAPER`]), which is what a gallery actually looks like at night. The
/// previous direction's warm charcoal is gone with the listening room it
/// belonged to.
pub const WALL: Color = Color::from_rgb(0.047, 0.051, 0.055);
/// **The shadow gap** where the wall meets the floor: the now-playing bar,
/// input wells, groove troughs and the backing behind a sleeve — everything
/// that sits *below* the wall. `#060708`.
pub const RECESS: Color = Color::from_rgb(0.024, 0.027, 0.031);
/// **One step up from the wall**: the album inspector's column, the popover,
/// a resting control. `#141517`.
///
/// A plinth is the thing a work stands on. It was called `CARD`, which is
/// web-app vocabulary and, under this direction, a lie — there are no cards,
/// and the shelf in particular may never be drawn on one.
pub const PLINTH: Color = Color::from_rgb(0.078, 0.082, 0.090);
/// **One step above [`PLINTH`]**: a selected segment, the playing row, a
/// hovered control. `#1C1D20`. Never a resting state.
pub const PLINTH_LIT: Color = Color::from_rgb(0.110, 0.114, 0.125);
/// Hairline border: findable when you look, invisible when you don't.
/// [`PAPER`] at **7 %**.
///
/// Down from 8 %, and the *perceived* weight is unchanged: the same alpha over
/// a darker ground is a larger step, so holding a hairline steady across the
/// repaint meant lowering its number. iced 0.13's `Border` is four-sided, so
/// every single line in the product is a `rule` widget.
pub const HAIRLINE: Color = Color { a: 0.07, ..PAPER };
/// The hairline, firmer — a selected control's edge, the playing row's edge.
/// [`PAPER`] at **15 %** (down from 17 %, for the reason [`HAIRLINE`] gives).
pub const HAIRLINE_STRONG: Color = Color { a: 0.15, ..PAPER };
/// Primary text: **archival mount board**, `#E8E4DB`.
///
/// A warm ivory that is a *material* rather than "white text" — the colour the
/// wall label is printed on. The room is cool ([`WALL`]) and the paper is warm,
/// and that pairing is the whole of what stops a near-black grid of covers
/// reading as a stock dark theme.
///
/// [`PAPER_DIM`], [`PAPER_FAINT`] and [`PAPER_MUTED`] are **the same r : g : b
/// ratios scaled down**, so the ink family is one board at four levels of light
/// rather than four greys that drifted apart. (The ramp baz shipped drifted
/// warmer as it darkened, which against a cool wall reads yellowish.) Each is
/// the *smallest* point on that ramp that clears its floor on every surface it
/// can land on, with 0.1 of margin.
pub const PAPER: Color = Color::from_rgb(0.910, 0.894, 0.859);
/// Secondary text: artists, captions, subtitles. `#ABA8A1`. Never a figure that
/// ticks — those are primary or tertiary, never in between.
pub const PAPER_DIM: Color = Color::from_rgb(0.671, 0.659, 0.631);
/// Tertiary text: counts, durations, hints, signal notes, the resting fader —
/// present, never loud.
///
/// `#888680`. This carries the whole of baz's readout vocabulary, and the value
/// it had through v0.1 (`#726D66`) measured **3.4 : 1** on the panel — below
/// the 4.5 : 1 AA floor for text on every surface it can land on. Re-derived
/// against the gallery's surfaces it lands two bytes from the correction that
/// preceded it, which is the interesting result: the near-black wall does not
/// demand different inks. What it changes is the margin at the top of the
/// range — this ink on [`PLINTH_LIT`] used to compute to 4.483 and be excused
/// as a rounding case, and now measures **4.62**.
/// `every_ink_clears_its_contrast_floor_on_every_surface_it_lands_on` is what
/// keeps it there, and it no longer has an exception to make.
pub const PAPER_FAINT: Color = Color::from_rgb(0.533, 0.525, 0.502);
/// The accent: amplifier-lamp amber. **Playback truth only** — see the
/// module's accent-discipline note for the five places it may appear.
pub const LAMP: Color = Color::from_rgb(0.890, 0.631, 0.306);
/// Lamp amber, brightened — the seek fill under the pointer, Play hovered.
pub const LAMP_BRIGHT: Color = Color::from_rgb(0.945, 0.702, 0.384);
/// Lamp amber, deepened — the seek fill while dragged, Play pressed.
pub const LAMP_DEEP: Color = Color::from_rgb(0.780, 0.533, 0.239);
/// Lamp amber as a glow: the playing sleeve's halo, and nothing else.
pub const LAMP_GLOW: Color = Color::from_rgba(0.890, 0.631, 0.306, 0.30);
/// Near-black ink for text sitting *on* the amber lamp.
pub const LAMP_INK: Color = Color::from_rgb(0.106, 0.078, 0.043);
/// Keyboard focus: paper at 45%, on the focused `text_input`'s border and
/// nowhere else.
///
/// Deliberately **not** the accent. Where the keyboard is has nothing to do
/// with where the music is, and the search field takes focus at launch — so
/// an amber focus ring made the first frame baz ever drew a lit lamp with
/// nothing playing.
pub const PAPER_RING: Color = Color { a: 0.45, ..PAPER };
/// Selected text in a `text_input`: paper at 18%.
///
/// Also not the accent, and for the same reason as [`PAPER_RING`]: a
/// selection is a fact about the keyboard, not about the music. A wash rather
/// than a fill so the glyphs under it keep their own ink.
pub const SELECT_WASH: Color = Color { a: 0.18, ..PAPER };
/// A control that is *set* but not currently sounding: the volume fader
/// while muted, or a stepper at the end of its travel.
///
/// `#6C6A66`. Not text a user must read, so the 3 : 1 non-text floor applies —
/// but the value it had through v0.1 (`#4A4743`) measured **1.9 : 1**, below
/// even that, which made the position the listener chose effectively invisible
/// while muted. Restoring that position is the entire reason mute leaves the
/// fader where it is. Re-derived on the gallery's ink ramp it clears 3 : 1 on
/// every surface (3.74 / 3.61 / 3.39 / 3.13) while staying plainly quieter
/// than a live control.
pub const PAPER_MUTED: Color = Color::from_rgb(0.424, 0.416, 0.400);
/// Problems, stated quietly: a soft brick red, no alarm klaxon.
pub const ALERT: Color = Color::from_rgb(0.851, 0.467, 0.420);
/// Success (theme palette slot; nothing renders it directly yet).
pub const SUCCESS: Color = Color::from_rgb(0.525, 0.663, 0.486);
/// The sleeve drop shadow's color.
pub const SHADOW: Color = Color::from_rgba(0.0, 0.0, 0.0, 0.45);

// ---------------------------------------------------------------------------
// Type scale
// ---------------------------------------------------------------------------

/// Hints and footnotes (11 px).
pub const SIZE_CAPTION: f32 = 11.0;
/// Metadata: captions, durations, status counts (12 px).
pub const SIZE_META: f32 = 12.0;
/// Body: tile titles, track titles, control labels (13 px).
pub const SIZE_BODY: f32 = 13.0;
/// Emphasis: search text, panel artist, empty-state lines (15 px).
pub const SIZE_EMPHASIS: f32 = 15.0;
/// Titles: the side panel's album title (19 px).
pub const SIZE_TITLE: f32 = 19.0;
/// Hero: the first-run question (28 px).
pub const SIZE_HERO: f32 = 28.0;

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

/// Corner radius for controls (buttons, inputs).
pub const RADIUS_CTRL: f32 = 6.0;
/// Corner radius of a segment inside the segmented control — one step
/// tighter than its enclosing well, so the raised segment nests rather than
/// straining against the edge.
pub const RADIUS_SEGMENT: f32 = 4.0;
/// Inset of the segmented control's well around its segments.
pub const SEGMENT_INSET: f32 = 2.0;
/// Corner radius for the tile's hover/selection card.
pub const RADIUS_TILE: f32 = 10.0;
/// Width of the album inspector, the column beside the shelf (logical px).
///
/// **One number, and now for one surface.** It was one number for three — the
/// album, the queue and the settings took turns in this width — and that shared
/// width was the only thing they had in common, which is what ADR-0015 is
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
/// Corner radius for small floating chips (the seek preview tip).
pub const RADIUS_CHIP: f32 = 4.0;
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
pub const PREVIEW_W: f32 = 58.0;

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
pub const LEVEL_W: f32 = 62.0;
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
pub fn detent_ink(engaged: bool) -> Color {
    if engaged { PAPER } else { HAIRLINE }
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
/// The transport glyphs' ink at rest — the same paper white the labels they
/// replaced were set in.
pub const GLYPH: Color = PAPER;
/// Opacity of a glyph on a live control.
pub const GLYPH_OPACITY: f32 = 1.0;
/// Opacity of a glyph while its command is in flight: the whole of the
/// pending affordance. A control that dims a little and comes back changes
/// no size, no shape, and no meaning — which is the difference between an
/// affordance and the flash the bottom bar used to have (the argument, and
/// the measured round trip, are in [`crate::player`]'s module docs).
pub const GLYPH_OPACITY_PENDING: f32 = 0.55;
/// Opacity of a glyph on a control that genuinely cannot act — no engine,
/// or nothing queued. Lands on roughly [`PAPER_FAINT`] over [`PLINTH`], the
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
/// `192 → 176.4 kHz` — with room to spare.
pub const SIGNAL_W: f32 = 120.0;

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
pub const SETTING_VALUE_W: f32 = 68.0;
/// iced 0.13's default relative line height (`LineHeight::Relative(1.3)`),
/// named here because a reserved text slot has to be measured in it.
pub const LINE_HEIGHT: f32 = 1.3;
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
pub const SETTING_NOTE_H: f32 = 2.0 * SIZE_META * LINE_HEIGHT;

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
pub fn scrollbar(_theme: &Theme, status: scrollable::Status) -> scrollable::Style {
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
            color: if active { HAIRLINE_STRONG } else { HAIRLINE },
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
pub fn check(_theme: &Theme, status: checkbox::Status) -> checkbox::Style {
    let (background, border_color) = match status {
        checkbox::Status::Active { is_checked } => (
            if is_checked { PLINTH_LIT } else { RECESS },
            HAIRLINE_STRONG,
        ),
        checkbox::Status::Hovered { .. } => (PLINTH_LIT, HAIRLINE_STRONG),
        checkbox::Status::Disabled { is_checked } => {
            (if is_checked { PLINTH } else { RECESS }, HAIRLINE)
        }
    };
    let disabled = matches!(status, checkbox::Status::Disabled { .. });
    checkbox::Style {
        background: Background::Color(background),
        icon_color: if disabled { PAPER_MUTED } else { PAPER },
        border: Border {
            color: border_color,
            width: 1.0,
            radius: RADIUS_SEGMENT.into(),
        },
        text_color: Some(if disabled { PAPER_MUTED } else { PAPER }),
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

static THEME: LazyLock<Theme> = LazyLock::new(|| {
    Theme::custom(
        "baz dark".to_owned(),
        iced::theme::Palette {
            background: WALL,
            text: PAPER,
            primary: LAMP,
            success: SUCCESS,
            danger: ALERT,
        },
    )
});

/// The application theme (cached; `Theme` clones are `Arc`-cheap).
#[must_use]
pub fn theme() -> Theme {
    THEME.clone()
}

/// A shelf tile's button chrome: invisible at rest (the sleeve leads),
/// a quiet raised card on hover, one step higher plus a hairline edge when
/// selected.
#[must_use]
pub fn tile(status: button::Status, selected: bool) -> button::Style {
    let mut style = button::Style {
        background: None,
        text_color: PAPER,
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: RADIUS_TILE.into(),
        },
        shadow: Shadow::default(),
    };
    if selected {
        style.background = Some(Background::Color(PLINTH_LIT));
        style.border.color = HAIRLINE_STRONG;
        // Two pixels, not one: see [`SELECTION_EDGE`].
        style.border.width = SELECTION_EDGE;
    } else if matches!(status, button::Status::Hovered | button::Status::Pressed) {
        style.background = Some(Background::Color(PLINTH));
    }
    style
}

/// The artwork's frame: a soft drop shadow so the sleeve sits on the shelf;
/// the playing album trades it for a lamp-amber halo.
#[must_use]
pub fn sleeve(playing: bool) -> container::Style {
    let shadow = if playing {
        Shadow {
            color: LAMP_GLOW,
            offset: Vector::ZERO,
            blur_radius: 16.0,
        }
    } else {
        Shadow {
            color: SHADOW,
            offset: Vector::new(0.0, 3.0),
            blur_radius: 8.0,
        }
    };
    container::Style {
        background: Some(Background::Color(RECESS)),
        shadow,
        ..container::Style::default()
    }
}

/// The playing album's lamp dot — the amplifier power light.
#[must_use]
pub fn lamp_dot(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(LAMP)),
        border: iced::border::rounded(DOT / 2.0),
        ..container::Style::default()
    }
}

/// Quiet transport controls (play/pause, next): a card that raises on hover
/// and sinks on press.
#[must_use]
pub fn transport(_theme: &Theme, status: button::Status) -> button::Style {
    let (background, border, text_color) = match status {
        button::Status::Hovered => (PLINTH_LIT, HAIRLINE_STRONG, PAPER),
        button::Status::Pressed => (RECESS, HAIRLINE_STRONG, PAPER),
        button::Status::Disabled => (PLINTH, HAIRLINE, PAPER_FAINT),
        button::Status::Active => (PLINTH, HAIRLINE, PAPER),
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
pub fn primary(_theme: &Theme, status: button::Status) -> button::Style {
    let (background, text_color) = match status {
        button::Status::Active => (LAMP, LAMP_INK),
        button::Status::Hovered => (LAMP_BRIGHT, LAMP_INK),
        button::Status::Pressed => (LAMP_DEEP, LAMP_INK),
        button::Status::Disabled => (PLINTH, PAPER_FAINT),
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
/// the ring at `LAMP` 55%, the selection at [`LAMP_GLOW`] — and since the
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
pub fn input(_theme: &Theme, status: text_input::Status) -> text_input::Style {
    let border_color = match status {
        text_input::Status::Focused => PAPER_RING,
        text_input::Status::Hovered => HAIRLINE_STRONG,
        text_input::Status::Active | text_input::Status::Disabled => HAIRLINE,
    };
    text_input::Style {
        background: Background::Color(RECESS),
        border: Border {
            color: border_color,
            width: 1.0,
            radius: RADIUS_CTRL.into(),
        },
        icon: PAPER_FAINT,
        placeholder: PAPER_FAINT,
        value: PAPER,
        selection: SELECT_WASH,
    }
}

/// The seek bar: lamp amber elapsed running through a recessed groove, with
/// a small amber knob that grows under the pointer.
///
/// Position is playback truth, so it earns the accent — the same rule that
/// gives the playing sleeve its halo. The unplayed remainder is [`RECESS`]:
/// the groove is *cut into* the bar rather than laid on top of it, matching
/// the inset treatment of the input wells.
#[must_use]
pub fn seek(_theme: &Theme, status: slider::Status) -> slider::Style {
    let (fill, radius) = match status {
        slider::Status::Active => (LAMP, KNOB),
        slider::Status::Hovered => (LAMP_BRIGHT, KNOB_ACTIVE),
        slider::Status::Dragged => (LAMP_DEEP, KNOB_ACTIVE),
    };
    slider::Style {
        rail: Rail {
            backgrounds: (Background::Color(fill), Background::Color(RECESS)),
            width: RAIL,
            border: Border {
                color: HAIRLINE,
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
pub fn seek_inert(_theme: &Theme, _status: slider::Status) -> slider::Style {
    slider::Style {
        rail: Rail {
            backgrounds: (Background::Color(RECESS), Background::Color(RECESS)),
            width: RAIL,
            border: Border {
                color: HAIRLINE,
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
pub fn volume(_theme: &Theme, status: slider::Status) -> slider::Style {
    let fill = match status {
        slider::Status::Active => PAPER_FAINT,
        slider::Status::Hovered | slider::Status::Dragged => PAPER_DIM,
    };
    volume_style(fill)
}

/// The volume fader while muted: the position the listener chose is still
/// shown — mute does not move the fader, and pretending otherwise would lose
/// the very setting mute exists to restore — but in the ink of something that
/// is not currently sounding.
#[must_use]
pub fn volume_muted(_theme: &Theme, _status: slider::Status) -> slider::Style {
    volume_style(PAPER_MUTED)
}

/// The volume fader with no engine behind it: the groove keeps its place and
/// its detent, filled with nothing at all.
#[must_use]
pub fn volume_inert(_theme: &Theme, _status: slider::Status) -> slider::Style {
    volume_style(RECESS)
}

/// The shared shape of every volume-fader state: only the ink varies, so no
/// state of this control can move a pixel.
fn volume_style(fill: Color) -> slider::Style {
    slider::Style {
        rail: Rail {
            backgrounds: (Background::Color(fill), Background::Color(RECESS)),
            width: RAIL,
            border: Border {
                color: HAIRLINE,
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
pub fn segmented(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(RECESS)),
        border: Border {
            color: HAIRLINE,
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
pub fn preview_tip(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(PLINTH_LIT)),
        text_color: Some(PAPER_DIM),
        border: Border {
            color: HAIRLINE_STRONG,
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
pub fn segment(status: button::Status, selected: bool) -> button::Style {
    let (background, text_color) = if selected {
        (Some(PLINTH_LIT), PAPER)
    } else {
        match status {
            button::Status::Hovered | button::Status::Pressed => (Some(PLINTH), PAPER),
            button::Status::Active | button::Status::Disabled => (None, PAPER_DIM),
        }
    };
    button::Style {
        background: background.map(Background::Color),
        text_color,
        border: Border {
            color: if selected {
                HAIRLINE_STRONG
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
pub fn panel(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(PLINTH)),
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
pub fn panel_toggle(status: button::Status, active: bool) -> button::Style {
    segment(status, active)
}

/// The now-playing bar: recessed below the wall, like the amp under the
/// shelf.
#[must_use]
pub fn bar(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(RECESS)),
        ..container::Style::default()
    }
}

/// Hairline rules dividing chrome from shelf.
#[must_use]
pub fn hairline(_theme: &Theme) -> rule::Style {
    rule::Style {
        color: HAIRLINE,
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
pub fn tooltip(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(PLINTH_LIT)),
        text_color: Some(PAPER_DIM),
        border: Border {
            color: HAIRLINE_STRONG,
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
/// [`PLINTH`] → [`PLINTH_LIT`] tick. The audit's finding was that in a still
/// frame you cannot tell which tile is selected and which is merely under the
/// pointer — one surface step and a 1 px hairline apart is below the threshold
/// at which two states read as two states.
///
/// Doubling the edge is the smallest change that separates them, and it stays
/// inside the depth strategy: no shadow (reserved for artwork), no accent (
/// reserved for playback truth), no second surface step. It costs nothing in
/// layout either — iced draws a border inside the widget's bounds, so the
/// tile's [`crate::shelf::CELL_W`] pitch is untouched.
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
/// one line) over an `artist · year` line — and [`crate::shelf::CELL_H`]
/// already has the room, so nothing about the tile pitch moves. It is the same
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
pub const CAPTION_LINE_H: f32 = SIZE_BODY * LINE_HEIGHT;

/// A track row — in the album inspector **and** in the **Up next** popover:
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
pub fn track_row(status: button::Status, playing: bool) -> button::Style {
    let background = match (playing, status) {
        // The playing row keeps its card whatever the pointer is doing, and
        // lifts no further under it: it is already the emphasised row.
        (true, _) => Some(PLINTH_LIT),
        (false, button::Status::Hovered | button::Status::Pressed) => Some(PLINTH),
        (false, button::Status::Active | button::Status::Disabled) => None,
    };
    button::Style {
        background: background.map(Background::Color),
        // The row's inks are set per-line by the view (a played row is fainter
        // than an upcoming one), so the button contributes none of its own.
        text_color: PAPER,
        border: Border {
            color: if playing {
                HAIRLINE_STRONG
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
// (docs/design/01-ux-audit-and-ia.md §2, ADR-0015)
// ---------------------------------------------------------------------------

/// Width of the **Up next** popover (logical px).
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
/// must not move between those two states. Wide enough for `999 / 999`,
/// because a queue's length is not something the front end gets to bound.
pub const QUEUE_POS_W: f32 = 72.0;

/// Width of the bar's **Up next** control (logical px) — the label, the
/// [`QUEUE_POS_W`] readout, and the padding around them.
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

/// The **Up next** popover's surface: one step above the panel, a hairline
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
pub fn popover(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(PLINTH_LIT)),
        border: Border {
            color: HAIRLINE_STRONG,
            width: 1.0,
            radius: RADIUS_CTRL.into(),
        },
        shadow: Shadow {
            color: SHADOW,
            offset: Vector::new(0.0, 3.0),
            blur_radius: 8.0,
        },
        ..container::Style::default()
    }
}

/// The now-playing block in the bar, once it became the door to **Up next**.
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
pub fn now_playing(status: button::Status, open: bool) -> button::Style {
    let background = if open {
        PLINTH_LIT
    } else {
        match status {
            button::Status::Hovered => PLINTH,
            button::Status::Pressed => RECESS,
            button::Status::Active | button::Status::Disabled => Color::TRANSPARENT,
        }
    };
    button::Style {
        background: Some(Background::Color(background)),
        text_color: PAPER,
        border: Border {
            color: if open {
                HAIRLINE_STRONG
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
        const { assert!(QUEUE_POS_W > SIZE_META * 6.0 * DIGIT_EM) }
        // …and the control that carries it holds the readout, its label and the
        // padding around both. The label itself is measured in the face that
        // draws it by `font.rs`; this is the arithmetic that leaves room.
        const { assert!(UP_NEXT_W > QUEUE_POS_W + 3.0 * GAP_SM) }
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
        for status in [
            button::Status::Active,
            button::Status::Hovered,
            button::Status::Pressed,
            button::Status::Disabled,
        ] {
            for open in [false, true] {
                let style = now_playing(status, open);
                geometry.push((style.border.width, style.border.radius.top_left));
                assert_eq!(
                    style.shadow,
                    Shadow::default(),
                    "the bar casts no shadow; only artwork and the popover do"
                );
            }
        }
        assert!(
            geometry
                .windows(2)
                .all(|pair| (pair[0].0 - pair[1].0).abs() < f32::EPSILON
                    && (pair[0].1 - pair[1].1).abs() < f32::EPSILON),
            "the affordance's border geometry varies with state: {geometry:?}"
        );
        // And "open" is visibly different from "hovered", or the anchor the
        // popover has instead of a notch says nothing.
        let open = now_playing(button::Status::Active, true);
        let hovered = now_playing(button::Status::Hovered, false);
        assert_ne!(
            from_background(open.background),
            from_background(hovered.background)
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
        assert!((SETTING_NOTE_H - 2.0 * SIZE_META * LINE_HEIGHT).abs() < f32::EPSILON);
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
    /// must not regress, and it is checked over the whole band rather than at
    /// the two widths the shipped window happens to have: every window width
    /// from the smallest iced will hand us to a wall-sized one, with the
    /// inspector open and closed, must produce a real grid and a covered,
    /// clamped visible range. The popover is deliberately absent from this
    /// sweep — that is the *point* of it being an overlay: it produces no width
    /// at all.
    #[test]
    fn the_shelf_virtualizes_at_every_width_the_inspector_can_produce() {
        use crate::shelf as geometry;

        const WINDOW_W: f32 = 1280.0;
        assert_eq!(geometry::columns(WINDOW_W), 5, "the shipped shelf");
        assert_eq!(
            geometry::columns(WINDOW_W - PANEL_W),
            3,
            "the inspector open: (1280 - 340 - 48) / 240 = 3.7 -> 3"
        );

        // The band: every window width baz can be dragged to, both inspector
        // states, both a full library and a single search result.
        let mut window = 640.0_f32;
        while window <= 2560.0 {
            for inspector in [0.0, PANEL_W] {
                let width = window - inspector;
                let cols = geometry::columns(width);
                assert!(
                    cols >= 1,
                    "the grid collapsed at {window} px with {inspector} px of inspector"
                );
                for albums in [1_usize, 97, 10_000] {
                    let rows = geometry::total_rows(albums, cols);
                    assert_eq!(rows, albums.div_ceil(cols));
                    let (first, end) = geometry::visible_rows(0.0, 800.0, rows);
                    assert!(
                        first < end && end <= rows,
                        "empty or overrunning viewport at {window} px, {albums} albums"
                    );
                }
            }
            window += 20.0;
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
        let theme = theme();
        let mut widths = Vec::new();
        for status in [
            slider::Status::Active,
            slider::Status::Hovered,
            slider::Status::Dragged,
        ] {
            for style in [volume, volume_muted, volume_inert] {
                let drawn = style(&theme, status);
                widths.push(radius(drawn));
                assert!(
                    (drawn.rail.width - RAIL).abs() < f32::EPSILON,
                    "the rail thickness must not vary with state"
                );
            }
        }
        assert!(
            widths
                .windows(2)
                .all(|pair| (pair[0] - pair[1]).abs() < f32::EPSILON),
            "the volume knob must not change size: {widths:?}"
        );
        // Muted is quieter than live and still readable above the groove it
        // sits in — the fader keeps showing the position mute will restore.
        const { assert!(PAPER_MUTED.r < PAPER_FAINT.r) }
        const { assert!(PAPER_MUTED.r > RECESS.r * 2.0) }
    }

    #[test]
    fn the_unity_detent_is_visible_without_being_loud() {
        // Engaged has to be plainly different from at-rest — that contrast
        // is what makes "at unity" and "a pixel below" different on sight —
        // and neither may reach for the accent, which means playback truth.
        let rest = detent_ink(false);
        let engaged = detent_ink(true);
        assert!(engaged.a > rest.a || engaged.r > rest.r * 3.0);
        for ink in [rest, engaged] {
            assert!(
                (ink.r - LAMP.r).abs() > 0.1 || (ink.b - LAMP.b).abs() > 0.1,
                "the detent must not be lamp amber"
            );
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

    /// **The contrast test.** Every ink the room can put on every surface it
    /// can land on, computed rather than estimated, against the floor that
    /// applies to it.
    ///
    /// This exists because two tokens shipped below their floor for the whole
    /// of v0.1 and nobody noticed: [`PAPER_FAINT`] at 3.4 : 1 on the panel —
    /// carrying every duration, count, hint and signal note in the product —
    /// and [`PAPER_MUTED`] at 1.9 : 1, which made the muted fader's position
    /// effectively invisible. Both were corrected;
    /// `.interface-design/system.md` §4.1 has the full table and the argument,
    /// and this is what stops either drifting back.
    ///
    /// Floors are WCAG 2.1's: **4.5 : 1** for anything a user has to read,
    /// **3 : 1** for a non-text mark whose job is to be locatable rather than
    /// legible. **Every ratio clears its floor outright.** The previous palette
    /// had one that did not — `PAPER_FAINT` on `CARD_HIGH` computed to 4.483,
    /// and this test had to excuse it by comparing at the one-decimal precision
    /// WCAG publishes to. On the gallery's surfaces the same ink measures
    /// **4.62**, so the excuse is deleted along with the constant that carried
    /// it. No pairing in this palette needs one, and if a future value needs
    /// one again that is the palette asking to be re-derived, not the test
    /// asking to be loosened.
    #[test]
    fn every_ink_clears_its_contrast_floor_on_every_surface_it_lands_on() {
        /// The AA floor for text.
        const TEXT: f32 = 4.5;
        /// The floor for a non-text mark.
        const MARK: f32 = 3.0;

        let surfaces = [
            ("WALL", WALL),
            ("PLINTH", PLINTH),
            ("RECESS", RECESS),
            ("PLINTH_LIT", PLINTH_LIT),
        ];
        // Every ink the theme paints, with the floor its *use* implies.
        // `PAPER_MUTED` is the muted fader and a stepper at the end of its
        // travel — a mark, not a sentence — so it takes the lower floor; the
        // lamp is a fill and a dot, likewise.
        let inks = [
            ("PAPER", PAPER, TEXT),
            ("PAPER_DIM", PAPER_DIM, TEXT),
            ("PAPER_FAINT", PAPER_FAINT, TEXT),
            ("ALERT", ALERT, TEXT),
            ("PAPER_MUTED", PAPER_MUTED, MARK),
            ("LAMP", LAMP, MARK),
        ];
        for (ink_name, ink, floor) in inks {
            for (surface_name, surface) in surfaces {
                let ratio = contrast(ink, surface);
                assert!(
                    ratio >= floor,
                    "{ink_name} on {surface_name} is {ratio:.2} : 1, below its \
                     {floor} : 1 floor"
                );
            }
        }

        // The one ink that sits on the accent rather than on a surface: the
        // Play button's label and triangle.
        let on_lamp = contrast(LAMP_INK, LAMP);
        assert!(
            on_lamp >= TEXT,
            "LAMP_INK on LAMP is {on_lamp:.2} : 1, below {TEXT} : 1"
        );

        // And the two corrections, pinned as corrections: the values v0.1
        // shipped fail the floors above, so this test would have caught them.
        let old_faint = Color::from_rgb(0.447, 0.427, 0.400);
        let old_muted = Color::from_rgb(0.290, 0.278, 0.263);
        assert!(
            contrast(old_faint, PLINTH) < TEXT,
            "the old PAPER_FAINT is supposed to be the failure this test exists for"
        );
        assert!(contrast(old_muted, PLINTH) < MARK);
        assert!(
            contrast(PAPER_FAINT, PLINTH) > contrast(old_faint, PLINTH),
            "the correction must be lighter, not merely different"
        );
        assert!(contrast(PAPER_MUTED, PLINTH) > contrast(old_muted, PLINTH));

        // The correction must not have cost the *ordering* the room is built
        // on: faint is quieter than dim, muted is quieter than faint, and
        // muted is still plainly above the groove it sits in.
        assert!(contrast(PAPER, PLINTH) > contrast(PAPER_DIM, PLINTH));
        assert!(contrast(PAPER_DIM, PLINTH) > contrast(PAPER_FAINT, PLINTH));
        assert!(contrast(PAPER_FAINT, PLINTH) > contrast(PAPER_MUTED, PLINTH));
    }

    // -----------------------------------------------------------------------
    // The accent discipline
    // -----------------------------------------------------------------------

    /// Whether `color` is the accent or one of its relatives.
    ///
    /// Membership of the amber family by value, rather than a hue test: the
    /// tokens are constants, so what has to be prevented is a *style* reaching
    /// for one of them, not a new colour that happens to be warm.
    fn is_lamp(color: Color) -> bool {
        [LAMP, LAMP_BRIGHT, LAMP_DEEP, LAMP_GLOW, LAMP_INK]
            .iter()
            .any(|amber| {
                (amber.r - color.r).abs() < f32::EPSILON
                    && (amber.g - color.g).abs() < f32::EPSILON
                    && (amber.b - color.b).abs() < f32::EPSILON
                    && (amber.a - color.a).abs() < f32::EPSILON
            })
    }

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
    fn every_painted_style() -> Vec<(&'static str, Vec<Color>)> {
        let theme = theme();
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
                painted.push(("tile", button_colors(&tile(status, selected))));
                painted.push(("segment", button_colors(&segment(status, selected))));
                painted.push((
                    "panel_toggle",
                    button_colors(&panel_toggle(status, selected)),
                ));
            }
            painted.push(("transport", button_colors(&transport(&theme, status))));
            painted.push(("primary", button_colors(&primary(&theme, status))));
            for open in [false, true] {
                painted.push(("now_playing", button_colors(&now_playing(status, open))));
            }
        }
        for status in slider_states {
            painted.push(("seek", slider_colors(&seek(&theme, status))));
            painted.push(("seek_inert", slider_colors(&seek_inert(&theme, status))));
            painted.push(("volume", slider_colors(&volume(&theme, status))));
            painted.push(("volume_muted", slider_colors(&volume_muted(&theme, status))));
            painted.push(("volume_inert", slider_colors(&volume_inert(&theme, status))));
        }
        for status in [
            text_input::Status::Active,
            text_input::Status::Hovered,
            text_input::Status::Focused,
            text_input::Status::Disabled,
        ] {
            let style = input(&theme, status);
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
            let style = check(&theme, status);
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
            let style = scrollbar(&theme, status);
            painted.push((
                "scrollbar",
                vec![
                    style.vertical_rail.scroller.color,
                    style.vertical_rail.border.color,
                ],
            ));
        }
        painted.push(("sleeve(resting)", container_colors(&sleeve(false))));
        painted.push(("sleeve(playing)", container_colors(&sleeve(true))));
        painted.push(("lamp_dot", container_colors(&lamp_dot(&theme))));
        painted.push(("segmented", container_colors(&segmented(&theme))));
        painted.push(("preview_tip", container_colors(&preview_tip(&theme))));
        painted.push(("panel", container_colors(&panel(&theme))));
        painted.push(("bar", container_colors(&bar(&theme))));
        painted.push(("popover", container_colors(&popover(&theme))));
        painted.push(("tooltip", container_colors(&tooltip(&theme))));
        painted.push(("hairline", vec![hairline(&theme).color]));
        painted.push(("detent_ink", vec![detent_ink(false), detent_ink(true)]));
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
    #[test]
    fn the_lamp_is_spent_only_on_playback_truth() {
        /// The styles §2.1.1 permits the accent in. Nothing may be added here
        /// without the specification changing first.
        const PERMITTED: [&str; 4] = ["sleeve(playing)", "lamp_dot", "seek", "primary"];

        let painted = every_painted_style();
        let mut seen_amber: Vec<&str> = Vec::new();
        for (name, colors) in &painted {
            let amber = colors.iter().copied().any(is_lamp);
            assert!(
                !amber || PERMITTED.contains(name),
                "`{name}` paints the accent. The lamp means playback truth \
                 (theme.rs's module docs, docs/design/02-visual-language.md \
                 §2.1.1); this surface is not playback truth, so it wants a \
                 surface step, a hairline, or a paper ink instead."
            );
            if amber {
                seen_amber.push(name);
            }
        }
        // The rule cuts both ways: a permitted use that stopped being amber
        // would mean the one signal reserved for the music had quietly gone
        // out, so each is asserted present rather than merely allowed.
        for permitted in PERMITTED {
            assert!(
                seen_amber.contains(&permitted),
                "`{permitted}` is supposed to be the accent and no longer paints it"
            );
        }
        // The room is large: if the sweep ever stopped covering it, the test
        // would pass vacuously.
        assert!(
            painted.len() > 40,
            "only {} styles swept — did a style stop being covered?",
            painted.len()
        );
    }

    /// The other half of the discipline: the accent is not named outside this
    /// module except where §2.1.1 permits it.
    ///
    /// The style sweep above cannot see a view that writes `theme::LAMP`
    /// straight onto a `text`, which is exactly how the scanning note and the
    /// first-run wordmark came to be amber with nothing playing. So this reads
    /// the crate's own sources and checks who names an amber token.
    ///
    /// The single entry on the list is §2.1.1's fourth permitted use: the
    /// elapsed timestamp warms to [`LAMP`] while a position has been asked for
    /// and not yet confirmed, because a position being asked for is a claim
    /// about the playhead. It cools the moment the engine answers.
    #[test]
    fn the_lamp_is_named_only_where_playback_truth_is_drawn() {
        /// `src`-relative paths that may name an amber token, and why.
        const PERMITTED: [&str; 1] = ["views/bottom_bar.rs"];

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
            // bytes. Neither is a view.
            if relative == "theme.rs" || relative == "font.rs" {
                continue;
            }
            let source = std::fs::read_to_string(&path).expect("a source file baz ships");
            if !source.contains("theme::LAMP") {
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
             (docs/design/02-visual-language.md §2.1.1). A scan, a focus ring, \
             a selection, a wordmark and a setting are none of those; they \
             want PAPER_DIM, PAPER_RING, SELECT_WASH or a surface step."
        );
        assert!(
            permitted_seen,
            "no view names the accent at all — the seek bar's in-flight \
             timestamp is supposed to, and this test just stopped meaning \
             anything"
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
