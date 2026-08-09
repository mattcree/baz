//! **The returns lane** — the resident surface at the window's left edge
//! (ADR-0030, as the owner amended it).
//!
//! Three parts, top to bottom:
//!
//! 1. **The head** — `Home`, `Library`, `Now playing`, always all three and
//!    always in that order. The owner's decision: *"home will appear at the
//!    top of the left hand sidebar always either way and it will contain the
//!    top level concerns. think spotify"*, extended by *"as an extension we
//!    will want a Now playing page at the top with the Home and Library"*.
//!    The place you are in is drawn in full paper ink; the other two rest at
//!    `paper_dim`. **`Now playing` carries the lamp dot when something is
//!    sounding** — the accent's one reserved meaning, spent so the lane can
//!    answer *is anything on?* without being read.
//! 2. **`RECENT`** — every playlist and the last
//!    [`crate::lane::RECENT_ALBUMS`] records, one list, last touched first
//!    ([`crate::lane`] owns the order and is tested without a window).
//! 3. **The two marks** at the foot: expanded and collapsed, in the density
//!    detents' exact anatomy (ADR-0028) — the current state's mark at full
//!    ink and **inert**, because it is the fact, and the other pressable,
//!    because it is the control.
//!
//! # The collapse is a hard cut
//!
//! No tween (ADR-0030 §3.1). A width tween would re-resolve `Grid::new` on
//! every frame of the slide and pop columns mid-flight; one frame is cheaper
//! *and* better. This is the one press in the product that re-hangs the
//! collection, and it is safe because it lands **outside the wall** — no wall
//! gesture can be in flight when it fires.
//!
//! # The ground
//!
//! The lane stands on [`theme::Palette::recess`] — one plane *below* the wall
//! — so it reads as cut into the room rather than stuck onto it, with a
//! hairline on its right edge and nothing else. That choice is also what makes
//! its rows answer the pointer correctly for free: a row steps one plane up
//! from whatever it stands on ([`theme::Palette::step_up`]), so on the recess
//! a hovered row lands on the wall's own colour.

use iced::widget::{
    Space, button, column, container, horizontal_rule, image as iced_image, row, scrollable, text,
};
use iced::{Element, Length, alignment};

use crate::app::{Message, Shelf};
use crate::lane::{Destination, Subject, Touched};
use crate::place::Place;
use crate::playlists::Playlists;
use crate::views::playlist_sleeve;
use crate::{icon, theme};

/// The lane, at the width its state says.
///
/// `rows` is already resolved and ordered by [`crate::lane::resolve`] — this
/// function decides nothing about membership, which is what keeps the
/// ordering testable without a window.
pub(crate) fn view<'a>(
    shelf: &'a Shelf,
    playlists: &'a Playlists,
    place: Place,
    rows: &'a [Touched],
    sounding: bool,
    window_w: f32,
) -> Element<'a, Message> {
    let room = theme::active();
    let open = theme::sidebar_w(window_w, shelf.lane_open) >= theme::SIDEBAR_W;
    let width = theme::sidebar_w(window_w, shelf.lane_open);

    let mut head = column![];
    for to in Destination::ALL {
        head = head.push(destination_row(to, place, open, sounding));
    }

    let mut list = column![];
    for entry in rows {
        list = list.push(lane_row(shelf, playlists, entry, open));
    }
    let list = scrollable(list)
        .direction(scrollable::Direction::Vertical(theme::wall_scrollbar()))
        .style(move |_theme, status| theme::scrollbar(room, room.recess, status))
        .width(Length::Fill)
        .height(Length::Fill);

    let body = column![
        head,
        // The head's one rule: three destinations above it, the things you
        // have touched below. The lane has exactly one seam because it has
        // exactly two parts.
        container(horizontal_rule(1).style(move |_theme| theme::hairline(room, room.recess)))
            .padding(theme::pad(theme::GAP_MD, 0.0)),
        heading(open),
        list,
        marks(shelf.lane_open, theme::sidebar_can_expand(window_w)),
    ]
    .width(Length::Fill)
    .height(Length::Fill);

    // The lane's own gutter is `GAP_XL` on both flanks — the measure both
    // widths are built from (`theme::SIDEBAR_W`) — never `HANG`: the lane is a
    // surface inside the window, not one hanging off its edge, and law L1's
    // window gutter belongs to the wall on the other side of it.
    let lane = container(body)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(theme::pad(theme::GAP_XL, theme::GAP_XL))
        .style(move |_theme| theme::lane_ground(room));

    row![
        lane,
        // The hairline on the right edge, in the lane's own width. Drawn as a
        // sibling rather than as a border because iced 0.13's `Border` is
        // four-sided, which is why every single line in the product is a rule.
        container(Space::new(Length::Fixed(1.0), Length::Fill))
            .height(Length::Fill)
            .style(move |_theme| theme::lane_seam(room)),
    ]
    .width(Length::Fixed(width))
    .height(Length::Fill)
    .into()
}

/// One of the head's three destinations: the glyph, and — expanded — its word.
///
/// **Not a toggle.** [`Place::go`] argues it: pressing the destination you are
/// already at leaves you there, where `Queue` and `Settings` close themselves.
/// The row is still a live control in that state rather than an inert mark,
/// which is the one place this departs from the density detents' anatomy —
/// a destination that stopped answering the pointer when you arrived would
/// read as broken, and pressing it costs nothing.
fn destination_row(
    to: Destination,
    place: Place,
    open: bool,
    sounding: bool,
) -> Element<'static, Message> {
    let room = theme::active();
    let here = place.destination() == Some(to);
    let glyph = match to {
        Destination::Home => icon::Glyph::Home,
        Destination::Library => icon::Glyph::Library,
        Destination::NowPlaying => icon::Glyph::NowPlaying,
    };
    let mark = iced_image(icon::handle(glyph))
        .width(Length::Fixed(theme::ICON_PX))
        .height(Length::Fixed(theme::ICON_PX))
        .opacity(if here {
            theme::GLYPH_OPACITY_HOVER
        } else {
            theme::GLYPH_OPACITY
        });
    // **The lamp dot, tucked against the glyph's corner** — and it survives
    // the collapse, which is the whole reason it is stacked on the glyph
    // rather than set after the word: collapsed there is no word to set it
    // after, and *is anything on?* is precisely the question a 96 px lane has
    // to keep answering.
    let glyph_block: Element<'static, Message> = if to == Destination::NowPlaying && sounding {
        iced::widget::stack![
            container(mark)
                .width(Length::Fixed(theme::SIDEBAR_GLYPH_BOX))
                .height(Length::Fixed(theme::SIDEBAR_GLYPH_BOX))
                .align_x(alignment::Horizontal::Left)
                .align_y(alignment::Vertical::Center),
            container(lamp_dot())
                .width(Length::Fixed(theme::SIDEBAR_GLYPH_BOX))
                .height(Length::Fixed(theme::SIDEBAR_GLYPH_BOX))
                .align_x(alignment::Horizontal::Right)
                .align_y(alignment::Vertical::Top),
        ]
        .into()
    } else {
        container(mark)
            .width(Length::Fixed(theme::SIDEBAR_GLYPH_BOX))
            .height(Length::Fixed(theme::SIDEBAR_GLYPH_BOX))
            .align_x(alignment::Horizontal::Left)
            .align_y(alignment::Vertical::Center)
            .into()
    };
    let mut line = row![glyph_block]
        .spacing(theme::GAP_MD)
        .align_y(iced::Alignment::Center);
    if open {
        line = line.push(
            text(to.label())
                .size(theme::SIZE_BODY)
                .line_height(theme::LEADING_BODY)
                .font(theme::MEDIUM)
                .color(if here { room.paper } else { room.paper_dim })
                .wrapping(text::Wrapping::None),
        );
    }
    let row_button = button(
        container(line)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_y(alignment::Vertical::Center)
            .clip(true),
    )
    .width(Length::Fill)
    .height(Length::Fixed(theme::SIDEBAR_DEST_H))
    .padding(theme::pad(0.0, theme::GAP_SM))
    // The row family, on the lane's own ground: one step up under the
    // pointer, the whole hit area painted (`theme::track_row`).
    .style(move |_theme, status| theme::track_row(room, room.recess, status, here))
    .on_press(Message::GoTo(to));
    if open {
        return row_button.into();
    }
    // Collapsed, the word is the tooltip — the icon-only law (doc 10 §3.1):
    // a control with no visible label carries its name where a pointer can
    // find it.
    iced::widget::tooltip(
        row_button,
        text(to.label())
            .size(theme::SIZE_CAPTION)
            .line_height(theme::LEADING_CAPTION),
        iced::widget::tooltip::Position::Right,
    )
    .gap(theme::GAP_XS)
    .padding(theme::GAP_XS)
    .style(move |_theme| theme::tooltip(room))
    .into()
}

/// The lamp dot on `Now playing`: [`theme::DOT`], the accent, and nothing
/// else.
///
/// `docs/REFUSALS.md`'s amber entry is what licenses it — the lamp states what
/// is true about playback *right now*, and "something is sounding" is exactly
/// that fact. It is not lit by hover, by the queue holding music, or by the
/// place being on screen.
fn lamp_dot() -> Element<'static, Message> {
    let room = theme::active();
    container(Space::new(
        Length::Fixed(theme::DOT),
        Length::Fixed(theme::DOT),
    ))
    .style(move |_theme| theme::lamp_dot(room))
    .into()
}

/// `RECENT` — the lane's one word, in the room's quietest voice.
///
/// Absent when the lane is collapsed: at 96 px there is no measure for a
/// tracked word, and a heading over an unlabelled column of sleeves would be
/// naming nothing the eye can use.
fn heading(open: bool) -> Element<'static, Message> {
    let room = theme::active();
    if !open {
        return Space::with_height(Length::Fixed(theme::GAP_SM)).into();
    }
    container(
        text(theme::tracked("RECENT"))
            .size(theme::SIZE_HEADING)
            .line_height(theme::LEADING_HEADING)
            .font(theme::MEDIUM)
            .color(room.paper_faint),
    )
    .padding(theme::pad(0.0, theme::GAP_SM))
    .into()
}

/// One row of the lane: a sleeve, and — expanded — the name over one quiet
/// line.
///
/// **Nothing marks which kind a row is**, because the sleeve already does: a
/// record wears its cover, a playlist wears the 2 × 2 collage of the records
/// it quotes (ADR-0024 §A1). That is what makes a mixed list read as one list
/// rather than as two lists sharing a column.
fn lane_row<'a>(
    shelf: &'a Shelf,
    playlists: &'a Playlists,
    entry: &'a Touched,
    open: bool,
) -> Element<'a, Message> {
    let room = theme::active();
    let edge = if open {
        theme::PANEL_SLEEVE
    } else {
        theme::SIDEBAR_SLEEVE
    };
    let sleeve: Element<'a, Message> = match entry.subject {
        Subject::Record(id) => match shelf.thumbs.peek(&id) {
            Some(handle) => iced_image(handle.clone())
                .width(Length::Fixed(edge))
                .height(Length::Fixed(edge))
                .into(),
            None => crate::views::gradient_block(id, edge, 1.0),
        },
        // The list's sleeve is the panel's own — the same 2 × 2 collage of
        // the records it quotes, from the same cache (ADR-0024 §A1) — so a
        // list cannot look like two different objects in two surfaces.
        Subject::Playlist(id) => {
            let art = playlists.row(id).map_or(&[][..], |entry| &entry.art);
            playlist_sleeve(shelf, art, &entry.name, edge)
        }
    };
    let press = match entry.subject {
        Subject::Record(id) => Message::AlbumClicked(id),
        Subject::Playlist(id) => Message::OpenPlaylist(id),
    };
    let body: Element<'a, Message> = if open {
        row![
            sleeve,
            container(
                column![
                    text(entry.name.clone())
                        .size(theme::SIZE_BODY)
                        .line_height(theme::LEADING_BODY)
                        .font(theme::MEDIUM)
                        .color(room.paper)
                        .wrapping(text::Wrapping::None),
                    text(entry.under.clone())
                        .size(theme::SIZE_META)
                        .line_height(theme::LEADING_META)
                        .color(room.paper_faint)
                        .wrapping(text::Wrapping::None),
                ]
                .spacing(theme::GAP_XXS)
            )
            .width(Length::Fill)
            .clip(true),
        ]
        .spacing(theme::GAP_SM)
        .align_y(iced::Alignment::Center)
        .into()
    } else {
        container(sleeve)
            .width(Length::Fill)
            .align_x(alignment::Horizontal::Center)
            .into()
    };
    let row_button = button(
        container(body)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_y(alignment::Vertical::Center)
            .clip(true),
    )
    .width(Length::Fill)
    .height(Length::Fixed(theme::SIDEBAR_ROW_H))
    .padding(theme::pad(0.0, if open { theme::GAP_SM } else { 0.0 }))
    .style(move |_theme, status| theme::track_row(room, room.recess, status, false))
    .on_press(press);
    if open {
        return row_button.into();
    }
    // Collapsed the sleeve is the only identification, so the name is the
    // tooltip — the same clause that names the head's glyphs.
    iced::widget::tooltip(
        row_button,
        text(entry.name.clone())
            .size(theme::SIZE_CAPTION)
            .line_height(theme::LEADING_CAPTION),
        iced::widget::tooltip::Position::Right,
    )
    .gap(theme::GAP_XS)
    .padding(theme::GAP_XS)
    .style(move |_theme| theme::tooltip(room))
    .into()
}

/// **The two marks** at the lane's foot (ADR-0030 §3), in the density
/// detents' exact anatomy: [`theme::STEPPER_HIT`] boxes, the current state's
/// mark at full ink and inert — it is the fact — the other at the resting ink
/// and pressable, because it is the control.
///
/// Below [`theme::SIDEBAR_FLOOR`] the `Expanded` mark is inert whichever state
/// is stored: expanding there would leave the collection one column of covers,
/// and a control that produces a state the window cannot hold is a trap. It
/// draws at the disabled ink so the inertness is visible rather than
/// discovered by pressing.
fn marks(open: bool, can_expand: bool) -> Element<'static, Message> {
    let mut marks = row![].spacing(theme::GAP_XS);
    for expanded in [true, false] {
        marks = marks.push(lane_mark(expanded, open, can_expand));
    }
    container(marks)
        .width(Length::Fill)
        .align_x(alignment::Horizontal::Center)
        .padding(theme::pad(theme::GAP_MD, 0.0))
        .into()
}

/// One of [`marks`]' two marks.
fn lane_mark(expanded: bool, open: bool, can_expand: bool) -> Element<'static, Message> {
    let room = theme::active();
    let current = expanded == open;
    let usable = !expanded || can_expand;
    let glyph = if expanded {
        icon::Glyph::LaneExpanded
    } else {
        icon::Glyph::LaneCollapsed
    };
    let opacity = if current {
        theme::GLYPH_OPACITY_HOVER
    } else if usable {
        theme::GLYPH_OPACITY
    } else {
        theme::GLYPH_OPACITY_DISABLED
    };
    let mark = container(
        iced_image(icon::handle(glyph))
            .width(Length::Fixed(theme::ICON_PX))
            .height(Length::Fixed(theme::ICON_PX))
            .opacity(opacity),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(alignment::Horizontal::Center)
    .align_y(alignment::Vertical::Center);
    let boxed: Element<'static, Message> = if current || !usable {
        container(mark)
            .width(Length::Fixed(theme::STEPPER_HIT))
            .height(Length::Fixed(theme::STEPPER_HIT))
            .into()
    } else {
        button(mark)
            .width(Length::Fixed(theme::STEPPER_HIT))
            .height(Length::Fixed(theme::STEPPER_HIT))
            .padding(0)
            .style(move |_theme, status| theme::transport(room, room.recess, status))
            .on_press(Message::ToggleLane)
            .into()
    };
    let name = if expanded { "Expanded" } else { "Collapsed" };
    iced::widget::tooltip(
        boxed,
        text(name)
            .size(theme::SIZE_CAPTION)
            .line_height(theme::LEADING_CAPTION),
        iced::widget::tooltip::Position::Top,
    )
    .gap(theme::GAP_XS)
    .padding(theme::GAP_XS)
    .style(move |_theme| theme::tooltip(room))
    .into()
}
