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
use iced::widget::{button, container, rule, slider, text_input};
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
/// Corner radius for small floating chips (the seek preview tip).
pub const RADIUS_CHIP: f32 = 4.0;
/// Edge of the playing-album lamp dot (a [`RADIUS_CTRL`]-free circle).
pub const DOT: f32 = 6.0;

/// Thickness of the seek bar's rail — a groove, not a gauge.
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

/// The cursor over a seekable bar. `Pointer` — the pointing hand every
/// platform uses for "this responds to a click" — because clicking the bar
/// is the primary gesture here and dragging is the refinement, not the
/// other way round. (`Grab`, iced's slider default, promises a handle that
/// must be picked up first, which is not how this bar behaves.)
pub const SEEK_CURSOR: mouse::Interaction = mouse::Interaction::Pointer;
/// The cursor while the bar is held: the closed hand, so the difference
/// between "you may" and "you are" is visible without looking at the bar.
pub const SEEK_CURSOR_HELD: mouse::Interaction = mouse::Interaction::Grabbing;
/// The cursor over a bar that cannot be scrubbed (a track of undeclared
/// length): the plain arrow, promising nothing.
pub const SEEK_CURSOR_INERT: mouse::Interaction = mouse::Interaction::None;

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
    }

    #[test]
    fn a_transport_button_is_a_square_target_around_its_glyph() {
        // The hit area is larger than the mark it carries…
        const { assert!(TRANSPORT_HIT > ICON_PX) }
        // …and the pair of them fits inside the column they centre in.
        const { assert!(2.0 * TRANSPORT_HIT + GAP_SM < SEEK_ROW_W) }
    }
}
