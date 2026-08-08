//! The baz design system: palette, type scale, spacing, radii, and the
//! widget styles built from them. Every color, size, and padding the UI
//! renders comes from this module — `app.rs` holds layout, not values.
//!
//! # Palette rationale
//!
//! baz's room is a **dim listening room**: charcoal walls, matte record
//! sleeves, low warm light. The chrome must recede so 10 000 covers — the
//! actual interface — supply all the chroma; every surface is therefore a
//! *warm* near-neutral (a hint of brown, never the blue-grey of a stock
//! dark theme), and text is warm off-white like liner-note paper. The one
//! accent is **lamp amber** — the power lamp / VU-meter glow of an
//! amplifier — used only where playback truth lives: the playing album's
//! halo and dot, the primary Play action, input focus, and the scanning
//! note. Blue (every streaming app's accent) is deliberately absent.
//!
//! Depth strategy: hairline borders plus whisper-quiet surface steps
//! (`WALL` → `CARD` → `CARD_HIGH`, with `RECESS` for inset chrome), and one
//! soft shadow under artwork so sleeves sit *on* the shelf. Corners: sleeves
//! are square like the physical object; controls are gently rounded.

use std::sync::LazyLock;

use iced::font::Weight;
use iced::widget::rule::FillMode;
use iced::widget::slider::{Handle, HandleShape, Rail};
use iced::widget::{button, checkbox, container, rule, scrollable, slider, text_input};
use iced::{Background, Border, Color, Font, Padding, Shadow, Theme, Vector, mouse};

// ---------------------------------------------------------------------------
// Palette
// ---------------------------------------------------------------------------

/// The room: the app background behind the shelf. Warm near-black.
pub const WALL: Color = Color::from_rgb(0.075, 0.067, 0.061);
/// Inset chrome — the now-playing bar and text-input wells sit *below* the
/// wall.
pub const RECESS: Color = Color::from_rgb(0.051, 0.045, 0.041);
/// Raised card surface: the side panel, resting controls, hovered tiles.
pub const CARD: Color = Color::from_rgb(0.106, 0.096, 0.088);
/// One step above [`CARD`]: selected tiles, hovered controls.
pub const CARD_HIGH: Color = Color::from_rgb(0.133, 0.121, 0.110);
/// Hairline border: findable when you look, invisible when you don't.
pub const HAIRLINE: Color = Color::from_rgba(0.93, 0.89, 0.85, 0.08);
/// The hairline, slightly firmer — selection edges, hovered controls.
pub const HAIRLINE_STRONG: Color = Color::from_rgba(0.93, 0.89, 0.85, 0.17);
/// Primary text: warm off-white, liner-note paper.
pub const PAPER: Color = Color::from_rgb(0.918, 0.902, 0.878);
/// Secondary text: artists, captions, subtitles.
pub const PAPER_DIM: Color = Color::from_rgb(0.659, 0.635, 0.604);
/// Tertiary text: counts, durations, hints — present, never loud.
pub const PAPER_FAINT: Color = Color::from_rgb(0.447, 0.427, 0.400);
/// The accent: amplifier-lamp amber. Playback truth and primary action only.
pub const LAMP: Color = Color::from_rgb(0.890, 0.631, 0.306);
/// Lamp amber, brightened — primary-action hover.
pub const LAMP_BRIGHT: Color = Color::from_rgb(0.945, 0.702, 0.384);
/// Lamp amber, deepened — primary-action press.
pub const LAMP_DEEP: Color = Color::from_rgb(0.780, 0.533, 0.239);
/// Lamp amber at half strength: input focus rings.
pub const LAMP_SOFT: Color = Color::from_rgba(0.890, 0.631, 0.306, 0.55);
/// Lamp amber as a glow: the playing sleeve's halo, text selection.
pub const LAMP_GLOW: Color = Color::from_rgba(0.890, 0.631, 0.306, 0.30);
/// Near-black ink for text sitting *on* the amber lamp.
pub const LAMP_INK: Color = Color::from_rgb(0.106, 0.078, 0.043);
/// A control that is *set* but not currently sounding: the volume fader
/// while muted. Dimmer than [`PAPER_FAINT`] and still plainly above
/// [`RECESS`], so the position the listener chose stays readable while the
/// control stops claiming to be audible.
pub const PAPER_MUTED: Color = Color::from_rgb(0.290, 0.278, 0.263);
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

/// Medium weight of the UI face — quiet prominence for titles and labels.
pub const MEDIUM: Font = Font {
    weight: Weight::Medium,
    ..Font::DEFAULT
};
/// Semibold weight of the UI face — headings only.
pub const SEMIBOLD: Font = Font {
    weight: Weight::Semibold,
    ..Font::DEFAULT
};
/// Monospace for data: track numbers, durations, counts. iced 0.13 has no
/// OpenType feature control (no `tnum`), so the monospace face *is* our
/// tabular figures.
pub const MONO: Font = Font::MONOSPACE;

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
/// Width of the right-hand rail — the album panel and the queue panel both
/// (logical px).
///
/// **One number for both**, and that is the property the layout rests on: the
/// rail is either showing a panel or it is not, and *which* panel is showing
/// can never change how much room the shelf has. Switching between them
/// reflows nothing; only opening or closing the rail does, by exactly this
/// much. `app.rs`'s grid estimate is kept in step with it (see
/// [`crate::panels`]).
pub const PANEL_W: f32 = 340.0;
/// Width of the number column in a track or queue list (logical px). Enough
/// for three monospace figures at [`SIZE_META`], so a long queue's positions
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
/// Width reserved for each of the seek bar's timestamps: enough for
/// `h:mm:ss` at [`SIZE_META`] in [`MONO`]. Fixed, so the groove keeps its
/// place when a track crosses the hour mark or a stamp gains a digit — the
/// same reason an undeclared length renders as `--:--` rather than as
/// nothing.
pub const STAMP_W: f32 = 52.0;
/// Height of the lane the hover preview floats in, directly above the
/// groove. Reserved whether or not anything is hovering, so the bottom bar
/// never changes height under the pointer.
pub const PREVIEW_H: f32 = 15.0;
/// Width of the hover-preview tip: enough for `h:mm:ss` at
/// [`SIZE_CAPTION`] in [`MONO`] plus its padding, fixed so the tip can be
/// centered on the pointer without measuring text.
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
/// enough for `-18.1 dB` at [`SIZE_CAPTION`] in [`MONO`] plus its padding.
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
/// or nothing queued. Lands on roughly [`PAPER_FAINT`] over [`CARD`], the
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
/// `192 → 176.4 kHz`, fifteen monospace figures at [`SIZE_META`] — with room
/// to spare.
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
/// Width reserved for a setting's value readout: enough for `-20.00 dB` in
/// [`MONO`] at [`SIZE_META`].
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
        checkbox::Status::Active { is_checked } => {
            (if is_checked { CARD_HIGH } else { RECESS }, HAIRLINE_STRONG)
        }
        checkbox::Status::Hovered { .. } => (CARD_HIGH, HAIRLINE_STRONG),
        checkbox::Status::Disabled { is_checked } => {
            (if is_checked { CARD } else { RECESS }, HAIRLINE)
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
        style.background = Some(Background::Color(CARD_HIGH));
        style.border.color = HAIRLINE_STRONG;
        style.border.width = 1.0;
    } else if matches!(status, button::Status::Hovered | button::Status::Pressed) {
        style.background = Some(Background::Color(CARD));
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
        button::Status::Hovered => (CARD_HIGH, HAIRLINE_STRONG, PAPER),
        button::Status::Pressed => (RECESS, HAIRLINE_STRONG, PAPER),
        button::Status::Disabled => (CARD, HAIRLINE, PAPER_FAINT),
        button::Status::Active => (CARD, HAIRLINE, PAPER),
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
        button::Status::Disabled => (CARD, PAPER_FAINT),
    };
    button::Style {
        background: Some(Background::Color(background)),
        text_color,
        border: iced::border::rounded(RADIUS_CTRL),
        shadow: Shadow::default(),
    }
}

/// Text inputs (search, first-run folder): an inset well with a hairline
/// edge that warms to lamp amber on focus.
#[must_use]
pub fn input(_theme: &Theme, status: text_input::Status) -> text_input::Style {
    let border_color = match status {
        text_input::Status::Focused => LAMP_SOFT,
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
        selection: LAMP_GLOW,
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
        background: Some(Background::Color(CARD_HIGH)),
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
        (Some(CARD_HIGH), PAPER)
    } else {
        match status {
            button::Status::Hovered | button::Status::Pressed => (Some(CARD), PAPER),
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
        background: Some(Background::Color(CARD)),
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

/// One row of the queue list. The playing row is a raised card with a
/// hairline edge; every other row is the panel it sits on.
///
/// The amber is spent on the lamp dot in the row's number column and nowhere
/// else — a whole row washed in accent would shout, and the dot is already the
/// mark the shelf uses to say "this one". This style only lifts the row far
/// enough to find it while scrolling a long queue.
#[must_use]
pub fn queue_row(playing: bool) -> container::Style {
    if !playing {
        return container::Style::default();
    }
    container::Style {
        background: Some(Background::Color(CARD_HIGH)),
        border: Border {
            color: HAIRLINE_STRONG,
            width: 1.0,
            radius: RADIUS_SEGMENT.into(),
        },
        ..container::Style::default()
    }
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
        background: Some(Background::Color(CARD_HIGH)),
        text_color: Some(PAPER_DIM),
        border: Border {
            color: HAIRLINE_STRONG,
            width: 1.0,
            radius: RADIUS_CHIP.into(),
        },
        ..container::Style::default()
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
        // A stamp must hold `h:mm:ss` — seven monospace figures, which are
        // around half an em wide at this size — without clipping.
        const { assert!(STAMP_W > SIZE_META * 7.0 * 0.5) }
        // The signal-path slot is reserved on the same principle, and must
        // hold the longest chain a consumer device produces —
        // `192 → 176.4 kHz`, fifteen monospace figures — so that a note
        // appearing there moves nothing beside it.
        const { assert!(SIGNAL_W > SIZE_META * 15.0 * 0.5) }
    }

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
        // `-20.00 dB` is ten monospace figures at SIZE_META; the slot is
        // fixed so a value changing cannot move the stepper beside it.
        const { assert!(SETTING_VALUE_W > SIZE_META * 10.0 * 0.5) }
        // A stepper is smaller than the transport but still a real target.
        const { assert!(STEPPER_HIT < TRANSPORT_HIT && STEPPER_HIT >= ICON_PX) }
    }

    /// Every sentence the settings panel can put in its reserved note slot
    /// fits it — otherwise the slot clips the words instead of the layout
    /// moving, which is the worse of the two failures it was chosen over.
    ///
    /// Text measurement needs a renderer, so this is an arithmetic bound
    /// rather than a shaping run: at [`SIZE_META`] the UI face averages well
    /// under half an em per character, which is the same conservative figure
    /// [`STAMP_W`] and [`SIGNAL_W`] are checked against above.
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

    /// The rail's width is one number, and the shelf still virtualizes at both
    /// of the two widths it can therefore have.
    ///
    /// The geometry helpers themselves are unchanged by the queue panel — that
    /// is the point of the panels sharing a slot — and this is what pins the
    /// claim: at the shipped window size the shelf goes from five columns to
    /// three when the rail opens, and both are real, non-degenerate grids.
    #[test]
    fn the_shelf_virtualizes_at_both_of_the_rails_two_widths() {
        use crate::shelf as geometry;

        const WINDOW_W: f32 = 1280.0;
        assert_eq!(geometry::columns(WINDOW_W), 5, "the shipped shelf");
        assert_eq!(
            geometry::columns(WINDOW_W - PANEL_W),
            3,
            "one panel open: (1280 - 340 - 48) / 240 = 3.7 -> 3"
        );
        // The rail must leave a usable shelf on the smallest window iced will
        // hand us as well, or opening a panel would collapse the grid.
        assert!(geometry::columns(640.0 - PANEL_W) >= 1);

        // Virtualization is width-independent, but the row count is not: the
        // same albums over fewer columns must still produce a covered,
        // clamped range rather than an empty or overrunning one.
        for width in [WINDOW_W, WINDOW_W - PANEL_W] {
            let cols = geometry::columns(width);
            let rows = geometry::total_rows(97, cols);
            assert_eq!(rows, 97_usize.div_ceil(cols));
            let (first, end) = geometry::visible_rows(0.0, 800.0, rows);
            assert!(first < end && end <= rows, "empty viewport at {width} px");
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
        // The level tip must hold `-18.1 dB` — eight monospace figures at
        // caption size, around half an em each — without clipping.
        const { assert!(LEVEL_W > SIZE_CAPTION * 8.0 * 0.5) }
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
}
