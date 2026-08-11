//! The saved-playlist collection: every playlist file as one collage tile.
//!
//! This is the root page for playlist browsing. It does not replace the
//! summoned picker panel: the panel answers “where should this track go?”,
//! while this place answers “which playlist do I want to open?”.

use iced::widget::{button, column, container, mouse_area, row, scrollable, stack, text};
use iced::{Element, Length, alignment};

use crate::app::{Message, Shelf};
use crate::player::PlayerState;
use crate::playlists::{PanelRow, PlaylistOrder, Playlists};
use crate::shelf::Grid;
use crate::theme;
use crate::views::{arrangement_key, place_header_led, place_pad, playlist_sleeve};

/// Draw every saved playlist in the shelf's shared work grid.
pub(crate) fn view<'a>(
    shelf: &'a Shelf,
    playlists: &'a Playlists,
    player: &PlayerState,
    hang: Grid,
) -> Element<'a, Message> {
    let room = theme::active();
    let count = playlists.rows.len();
    let note = match count {
        1 => "1 playlist".to_owned(),
        count => format!("{count} playlists"),
    };
    let mut order = row![].spacing(theme::GAP_MD);
    for choice in PlaylistOrder::ALL {
        order = order.push(arrangement_key(
            choice.label(),
            choice == playlists.order,
            Message::PlaylistOrderSelected(choice),
        ));
    }
    let header = place_header_led(order.into(), Some(note));
    if playlists.rows.is_empty() {
        return column![
            header,
            container(
                text("No playlists yet.")
                    .size(theme::SIZE_META)
                    .line_height(theme::LEADING_META)
                    .color(room.paper_faint)
            )
            .center(Length::Fill)
        ]
        .into();
    }

    let mut tiles = column![].spacing(hang.gutter);
    let mut current = row![].spacing(hang.gutter);
    let mut in_row = 0usize;
    for playlist in playlists.ordered_rows() {
        current = current.push(tile(
            shelf,
            playlist,
            hang,
            playlists.hovered == Some(playlist.id),
            player.engine_ready(),
        ));
        in_row += 1;
        if in_row == hang.columns {
            tiles = tiles.push(current);
            current = row![].spacing(hang.gutter);
            in_row = 0;
        }
    }
    if in_row > 0 {
        tiles = tiles.push(current);
    }

    let body = tiles.width(Length::Fixed(hang.block_width()));
    column![
        header,
        scrollable(
            container(body)
                .width(Length::Fill)
                .padding(place_pad())
                .align_x(alignment::Horizontal::Center)
        )
        .direction(scrollable::Direction::Vertical(theme::wall_scrollbar()))
        .style(move |_theme, status| theme::scrollbar(room, room.wall, status))
        .width(Length::Fill)
        .height(Length::Fill)
    ]
    .into()
}

fn tile<'a>(
    shelf: &'a Shelf,
    playlist: &'a PanelRow,
    hang: Grid,
    hovered: bool,
    engine: bool,
) -> Element<'a, Message> {
    let room = theme::active();
    let edge = hang.art;
    let work = (edge - 2.0 * theme::SLEEVE_MAT).max(0.0);
    let art = playlist_sleeve(shelf, &playlist.art, &playlist.name, work);
    let art: Element<'_, Message> = if hovered {
        let mut options = Vec::new();
        if engine && playlist.playable > 0 {
            options.push(crate::views::shelf::VeilOption::accented(
                crate::icon::Glyph::Play,
                "Play",
                Message::PlayPlaylist(playlist.id),
            ));
        }
        options.push(crate::views::shelf::VeilOption::new(
            crate::icon::Glyph::Open,
            "Open",
            Message::OpenPlaylist(playlist.id),
        ));
        stack![art, crate::views::shelf::veil(work, options)].into()
    } else {
        art
    };
    let sleeve = container(
        container(art)
            .width(Length::Fixed(work))
            .height(Length::Fixed(work))
            .style(move |_theme| theme::sleeve(room, 0.0)),
    )
    .width(Length::Fixed(edge))
    .height(Length::Fixed(edge))
    .padding(theme::SLEEVE_MAT)
    .style(move |_theme| theme::sleeve_mat(room));
    let caption = |content: Element<'a, Message>| {
        container(content)
            .width(Length::Fixed(edge))
            .height(Length::Fixed(theme::CAPTION_LINE_H))
            .align_y(alignment::Vertical::Top)
            .clip(true)
    };
    let body = column![
        sleeve,
        column![
            caption(
                text(playlist.name.clone())
                    .size(theme::SIZE_BODY)
                    .line_height(theme::LEADING_BODY)
                    .font(theme::MEDIUM)
                    .wrapping(text::Wrapping::None)
                    .into()
            ),
            caption(
                text(playlist.counts())
                    .size(theme::SIZE_META)
                    .line_height(theme::LEADING_META)
                    .color(room.paper_faint)
                    .wrapping(text::Wrapping::None)
                    .into()
            )
        ]
        .height(Length::Fixed(theme::CAPTION_H)),
        crate::views::shelf::state_rule(if hovered { 1.0 } else { 0.0 }, false, edge)
    ]
    .spacing(theme::GAP_XS)
    .width(Length::Fixed(edge));
    mouse_area(
        button(body)
            .padding(0)
            .style(move |_theme, status| theme::tile(room, status, false))
            .on_press(Message::OpenPlaylist(playlist.id)),
    )
    .on_enter(Message::PlaylistTileEntered(playlist.id))
    .on_exit(Message::PlaylistTileLeft(playlist.id))
    .into()
}
