//! The saved-playlist collection: every playlist file as one collage tile.
//!
//! This is the root page for playlist browsing. It does not replace the
//! summoned picker panel: the panel answers “where should this track go?”,
//! while this place answers “which playlist do I want to open?”.

use iced::widget::{Space, button, column, container, mouse_area, row, scrollable, stack, text};
use iced::{Element, Length, alignment};
use std::time::{SystemTime, UNIX_EPOCH};

use baz_core::history::Recency;
use baz_core::index::{AlbumArtist, GroupKey, Initial};

use crate::app::{Message, Shelf};
use crate::player::PlayerState;
use crate::playlists::{PanelRow, PlaylistOrder, Playlists};
use crate::selection::Content;
use crate::shelf::Grid;
use crate::theme;
use crate::views::{
    arrangement_key, place_header_led, place_name, place_pad, playlist_sleeve, section_rule,
};
use crate::vm::GroupHeaderVm;

/// Draw every saved playlist in the shelf's shared work grid.
pub(crate) fn view<'a>(
    shelf: &'a Shelf,
    playlists: &'a Playlists,
    player: &PlayerState,
    hang: Grid,
    scroll_offset: f32,
) -> Element<'a, Message> {
    let room = theme::active();
    let count = playlists.rows.len() + 1;
    let note = if let Some(id) = playlists.confirming_overview_delete {
        playlists.rows.iter().find(|row| row.id == id).map_or_else(
            || format!("{count} playlists"),
            |row| format!("Delete “{}”?", row.name),
        )
    } else {
        match count {
            1 => "1 playlist".to_owned(),
            count => format!("{count} playlists"),
        }
    };
    let mut order = row![].spacing(theme::GAP_MD);
    for choice in PlaylistOrder::ALL {
        order = order.push(arrangement_key(
            choice.label(),
            choice == playlists.order,
            Message::PlaylistOrderSelected(choice),
        ));
    }
    let create = button(
        text("New playlist")
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            .font(theme::MEDIUM),
    )
    .on_press(Message::NewPlaylistOpen)
    .height(Length::Fixed(theme::TRANSPORT_HIT))
    .padding(theme::pad(0.0, theme::GAP_SM))
    .style(move |_theme, status| theme::word_button(room, room.wall, status));
    let lead = row![place_name("Playlists"), order, create]
        .spacing(theme::GAP_MD)
        .align_y(iced::Alignment::Center);
    let header = place_header_led(lead.into(), Some(note));
    let ordered = playlists.ordered_rows();
    let total_rows = hang.rows(ordered.len());
    let (first, end) = hang.visible_rows(scroll_offset, shelf.grid_size.height, total_rows);
    let mut tiles = column![Space::new().height(Length::Fixed(hang.spacer_height(first)))];
    for row_index in first..end {
        let item_first = row_index * hang.columns;
        let item_end = (item_first + hang.columns).min(ordered.len());
        let mut current = row![].spacing(hang.gutter);
        for playlist in &ordered[item_first..item_end] {
            current = current.push(tile(
                shelf,
                playlist,
                hang,
                playlists.hovered == Some(playlist.id),
                playlists.confirming_overview_delete == Some(playlist.id),
                player.engine_ready(),
            ));
        }
        tiles = tiles.push(
            container(current)
                .height(Length::Fixed(hang.row_h))
                .align_y(alignment::Vertical::Top),
        );
    }
    tiles = tiles.push(Space::new().height(Length::Fixed(hang.spacer_height(total_rows - end))));

    let body = column![section_rule("All playlists"), tiles]
        .spacing(theme::GAP_MD)
        .width(Length::Fixed(hang.block_width()));
    let wall: Element<'a, Message> = column![
        header,
        scrollable(
            container(body)
                .width(Length::Fill)
                .padding(place_pad())
                .align_x(alignment::Horizontal::Center)
        )
        .id(scroll_id())
        .on_scroll(Message::PlaylistsScrolled)
        // The body spans the window edge while reserving the rail's lane, just
        // like Library: the bar remains at the outer edge and tiles can never
        // slide beneath the index.
        .direction(scrollable::Direction::Vertical(theme::shelf_scrollbar()))
        .style(move |_theme, status| theme::scrollbar(room, room.wall, status))
        .width(Length::Fill)
        .height(Length::Fill)
    ]
    .into();
    let (entries, current) = rail(&ordered, playlists, hang, scroll_offset);
    crate::views::shelf::collection_scaffold(
        wall,
        crate::views::shelf::index_rail_from(entries, current, Message::PlaylistRailJumped),
    )
}

/// The saved-playlist collection's scroll identity. It is separate from the
/// record wall's identity because either place must retain its position while
/// the other is visited.
pub(crate) fn scroll_id() -> iced::widget::Id {
    iced::widget::Id::new("baz-playlists")
}

/// Project the active playlist ordering into the common index-rail vocabulary.
/// Alphabetical order gets initials; chronological orders get the same elapsed
/// buckets the Library uses. Labels therefore always describe the row they
/// jump to — an A–Z rail is never painted over a date-sorted collection.
fn rail(
    ordered: &[&PanelRow],
    playlists: &Playlists,
    hang: Grid,
    scroll_offset: f32,
) -> (Vec<crate::rail::RailEntry>, Option<usize>) {
    let mut headers = Vec::new();
    let mut firsts = Vec::new();
    for (index, playlist) in ordered.iter().enumerate() {
        let header = match playlists.order {
            PlaylistOrder::Alphabetical => {
                GroupHeaderVm::Initial(Initial::of(AlbumArtist::Named(&playlist.name)))
            }
            PlaylistOrder::Created => {
                GroupHeaderVm::Recency(recency(playlist.created_unix_s, true))
            }
            PlaylistOrder::Played => {
                GroupHeaderVm::Recency(recency(playlists.played_at(playlist.id), false))
            }
        };
        if headers.last() != Some(&header) {
            headers.push(header);
            firsts.push(index);
        }
    }
    let key = match playlists.order {
        PlaylistOrder::Alphabetical => GroupKey::Alphabet,
        PlaylistOrder::Created => GroupKey::Added,
        PlaylistOrder::Played => GroupKey::Played,
    };
    let mut entries = crate::rail::entries(key, &headers);
    for entry in &mut entries {
        entry.shelf = entry.shelf.and_then(|group| firsts.get(group).copied());
    }
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a non-negative viewport row index is bounded by the in-memory collection"
    )]
    let first_visible = (scroll_offset / hang.row_h).floor().max(0.0) as usize * hang.columns;
    let current = entries
        .iter()
        .rposition(|entry| entry.shelf.is_some_and(|first| first <= first_visible));
    (entries, current)
}

/// Map a filesystem or session timestamp to the Library's honest age buckets.
/// `Unrecorded` describes an unavailable creation stamp; `Never` describes a
/// playlist that has not been played in this run. They are deliberately not
/// collapsed into the same quiet label.
fn recency(timestamp: Option<u64>, created: bool) -> Recency {
    let Some(timestamp) = timestamp else {
        return if created {
            Recency::Unrecorded
        } else {
            Recency::Never
        };
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = now.saturating_sub(timestamp) / 86_400;
    match days {
        0 => Recency::Today,
        1..=6 => Recency::ThisWeek,
        7..=30 => Recency::ThisMonth,
        31..=364 => Recency::MonthsAgo(u32::try_from((days / 30).max(1)).unwrap_or(u32::MAX)),
        _ => Recency::YearsAgo(u32::try_from((days / 365).max(1)).unwrap_or(u32::MAX)),
    }
}

fn tile<'a>(
    shelf: &'a Shelf,
    playlist: &'a PanelRow,
    hang: Grid,
    hovered: bool,
    confirming_delete: bool,
    engine: bool,
) -> Element<'a, Message> {
    let room = theme::active();
    let selected = shelf.selection.is(Content::Playlist(playlist.id));
    let edge = hang.art;
    let work = (edge - 2.0 * theme::SLEEVE_MAT).max(0.0);
    let art = playlist_sleeve(shelf, &playlist.art, &playlist.name, work);
    let art: Element<'_, Message> = if hovered || selected || confirming_delete {
        let mut options = Vec::new();
        if confirming_delete {
            options.push(crate::views::shelf::VeilOption::new(
                crate::icon::Glyph::Close,
                "Move to Trash",
                Message::PlaylistOverviewDelete,
            ));
            options.push(crate::views::shelf::VeilOption::new(
                crate::icon::Glyph::Open,
                "Keep",
                Message::PlaylistOverviewDeleteCancel,
            ));
        } else {
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
            if playlist.id != crate::playlists::FAVOURITES_ID {
                options.push(crate::views::shelf::VeilOption::new(
                    crate::icon::Glyph::Close,
                    "Delete",
                    Message::PlaylistOverviewDeleteStart(playlist.id),
                ));
            }
        }
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
        crate::views::shelf::state_rule(if hovered { 1.0 } else { 0.0 }, selected, edge)
    ]
    .spacing(theme::GAP_XS)
    .width(Length::Fixed(edge));
    mouse_area(
        button(body)
            .padding(0)
            .style(move |_theme, status| theme::tile(room, status, selected))
            .on_press(Message::ContentPressed(Content::Playlist(playlist.id))),
    )
    .on_enter(Message::PlaylistTileEntered(playlist.id))
    .on_exit(Message::PlaylistTileLeft(playlist.id))
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unavailable_timestamps_keep_created_and_played_honest() {
        assert_eq!(recency(None, true), Recency::Unrecorded);
        assert_eq!(recency(None, false), Recency::Never);
    }

    #[test]
    fn a_current_timestamp_is_a_current_bucket() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        assert_eq!(recency(Some(now), true), Recency::Today);
    }
}
