//! The pinned built-in Favourites playlist.

use iced::widget::{Space, button, container, row, text};
use iced::{Element, Length, alignment};

use crate::app::{Message, Shelf};
use crate::player::PlayerState;
use crate::views::{page, place_name, playlist_page};
use crate::{theme, vm};

#[must_use]
pub(crate) fn queue(shelf: &Shelf) -> vm::QueueVm {
    let paths: Vec<_> = shelf
        .library
        .favourite_tracks()
        .into_iter()
        .map(|track| track.path.clone())
        .collect();
    vm::restored_queue(&shelf.albums, &paths, 0, vm::RunSource::Fixed).0
}

#[expect(
    clippy::too_many_lines,
    reason = "one shallow built-in playlist composition"
)]
pub(crate) fn view<'a>(
    shelf: &'a Shelf,
    player: &'a PlayerState,
    window_width: f32,
) -> Element<'a, Message> {
    let list = queue(shelf);
    let room = theme::active();
    let mut art = Vec::new();
    let mut rows = Vec::with_capacity(list.items.len());
    for (index, item) in list.items.iter().enumerate() {
        let album_id = item.album.as_deref().and_then(|title| {
            shelf
                .albums
                .iter()
                .find(|album| {
                    album.title.as_deref() == Some(title)
                        && album.artist.label()
                            == item.album_artist.as_deref().unwrap_or(list.artist.as_str())
                })
                .map(|album| album.id)
        });
        if let Some(id) = album_id
            && art.len() < 4
            && !art.contains(&id)
        {
            art.push(id);
        }
        let playing = player.now_playing_path() == Some(item.path.as_path());
        let marker: Element<'_, Message> = if playing {
            page::lamp_dot()
        } else {
            text((index + 1).to_string())
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.paper_faint)
                .into()
        };
        let body = page::track_row(page::TrackRow {
            marker,
            artwork: Some(playlist_page::row_art(shelf, album_id)),
            title: item.title.clone().into(),
            ink: room.paper,
            under: item
                .artist
                .clone()
                .or_else(|| item.album_artist.clone())
                .map(|artist| (artist.into(), room.paper_dim, None)),
            context: item.album.clone().map(|album| {
                (
                    album.into(),
                    album_id.map(Message::OpenAlbum),
                    page::is_playlist_two_column(window_width),
                )
            }),
            duration: item
                .duration
                .map(vm::format_duration)
                .unwrap_or_default()
                .into(),
            playing,
            selected: false,
            press: player
                .engine_ready()
                .then_some(Message::FavouritesPlayTrack(index)),
        });
        rows.push(
            row![
                body,
                page::favourite_slot(&item.path, true),
                Space::new().width(Length::Fixed(
                    3.0 * theme::STEPPER_HIT + 2.0 * theme::GAP_XS
                )),
            ]
            .spacing(theme::GAP_XS)
            .align_y(iced::Alignment::Center)
            .into(),
        );
    }
    let missing = shelf.library.missing_favourites();
    let facts = if missing == 0 {
        match list.items.len() {
            1 => "1 track".to_owned(),
            count => format!("{count} tracks"),
        }
    } else {
        format!("{} available · {missing} missing", list.items.len())
    };
    playlist_page::view(
        shelf,
        playlist_page::PlaylistPage {
            lead: breadcrumb(),
            name: "Favourites".to_owned(),
            art,
            commitment: Some(page::commitment(
                "Play",
                player.engine_ready() && !list.items.is_empty(),
                Message::FavouritesPlay,
            )),
            acts: Vec::new(),
            identity: page::Identity {
                name: "Favourites".to_owned(),
                face: theme::SEMIBOLD,
                edit: None,
                byline: "Built-in playlist".to_owned(),
                facts,
                beside_facts: None,
            },
            rows,
            on_scroll: Message::FavouritesScrolled,
        },
        window_width,
    )
}

fn breadcrumb() -> Element<'static, Message> {
    let room = theme::active();
    let door = button(
        container(place_name("Playlists"))
            .height(Length::Fill)
            .align_y(alignment::Vertical::Center),
    )
    .height(Length::Fixed(theme::TRANSPORT_HIT))
    .padding(0)
    .style(move |_theme, status| theme::word_button(room, room.wall, status))
    .on_press(Message::GoTo(crate::lane::Destination::Playlists));
    row![
        door,
        text("›")
            .size(theme::SIZE_EMPHASIS)
            .line_height(theme::LEADING_EMPHASIS)
            .color(room.paper_faint),
        place_name("Favourites"),
    ]
    .spacing(theme::GAP_SM)
    .align_y(iced::Alignment::Center)
    .height(Length::Fixed(theme::TRANSPORT_HIT))
    .into()
}
