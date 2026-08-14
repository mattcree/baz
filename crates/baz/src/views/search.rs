//! The resident app-bar search well and its app-wide results dropover.

use iced::widget::{
    Space, button, column, container, mouse_area, opaque, row, scrollable, stack, text, text_input,
};
use iced::{Element, Length, Size, alignment};

use crate::app::{Message, Shelf, search_id};
use crate::player::PlayerState;
use crate::search::{Action, OVERSCAN_ROWS, ROW_H, SECTION_H};
use crate::selection::Content;
use crate::{icon, theme, vm};

pub(crate) const SCOPE: &str = "Search library";
const DROPOVER_W: f32 = 640.0;
const DROPOVER_H: f32 = 520.0;

#[expect(
    clippy::cast_precision_loss,
    reason = "search results are capped at 10,000 rows, far below exact f32 integer range"
)]
fn rows(count: usize) -> f32 {
    count as f32 * ROW_H
}

pub(crate) fn scroll_id() -> iced::widget::Id {
    iced::widget::Id::new("baz-search-results")
}

/// The one full search well, resident in the app bar at every width/place.
pub(crate) fn well(shelf: &Shelf) -> Element<'_, Message> {
    let room = theme::active();
    let filtering = !shelf.query.trim().is_empty();
    let input = text_input(SCOPE, &shelf.query)
        .id(search_id())
        .on_input(Message::SearchChanged)
        .on_submit(Message::SearchConfirmed)
        .padding(iced::Padding {
            top: theme::WELL_PAD_V,
            right: if filtering {
                theme::GAP_MD + theme::SIDEBAR_MATCH_W
            } else {
                theme::GAP_MD
            },
            bottom: theme::WELL_PAD_V,
            left: theme::SIDEBAR_HEAD_TEXT_X,
        })
        .size(theme::SIZE_BODY)
        .line_height(theme::LEADING_BODY)
        .width(Length::Fixed(theme::SIDEBAR_MEASURE))
        .style(move |_theme, status| theme::input(room, status));
    let mark: Element<'_, Message> = if filtering {
        container(crate::views::clear_mark(room.recess))
            .height(Length::Fixed(theme::TRANSPORT_HIT))
            .padding(theme::pad(0.0, theme::GAP_SM))
            .align_y(alignment::Vertical::Center)
            .into()
    } else {
        container(
            iced::widget::image(icon::handle(icon::Glyph::Magnifier))
                .width(Length::Fixed(theme::ICON_PX))
                .height(Length::Fixed(theme::ICON_PX))
                .opacity(theme::GLYPH_OPACITY),
        )
        .height(Length::Fixed(theme::TRANSPORT_HIT))
        .padding(theme::pad(0.0, theme::SIDEBAR_WELL_GLYPH_LEAD))
        .align_y(alignment::Vertical::Center)
        .into()
    };
    let mut layers = stack![input, mark];
    if filtering {
        let count = shelf.search_result_count();
        layers = layers.push(
            container(
                text(format_count(count))
                    .size(theme::SIZE_META)
                    .line_height(theme::LEADING_META)
                    .color(room.paper_faint),
            )
            .width(Length::Fill)
            .height(Length::Fixed(theme::TRANSPORT_HIT))
            .padding(theme::pad(0.0, theme::GAP_MD))
            .align_x(alignment::Horizontal::Right)
            .align_y(alignment::Vertical::Center),
        );
    }
    layers.into()
}

fn format_count(count: usize) -> String {
    if count < 10_000 {
        count.to_string()
    } else {
        format!("{}k+", count / 1_000)
    }
}

/// A transparent dismissal layer below the app bar plus the bounded result
/// card anchored under the well. The bar itself stays live, so the caret and
/// clear mark remain usable while the chooser is open; the current place is
/// only covered, never replaced or re-laid.
pub(crate) fn layer<'a>(
    shelf: &'a Shelf,
    player: &'a PlayerState,
    window: Size,
    adding_to_playlist: bool,
) -> Element<'a, Message> {
    let room = theme::active();
    let left = theme::HANG + theme::APP_BAR_NAME_W + theme::GAP_LG;
    let width = DROPOVER_W.min((window.width - left - theme::HANG).max(320.0));
    let height = DROPOVER_H
        .min((window.height - theme::APP_BAR_H - theme::BAR_CONTENT_H - theme::HANG).max(160.0));
    let backdrop = mouse_area(Space::new().width(Length::Fill).height(Length::Fill))
        .on_press(Message::DismissSearch);
    let card = opaque(
        container(results(shelf, player, adding_to_playlist))
            .width(Length::Fixed(width))
            .height(Length::Fixed(height))
            .style(move |_theme| theme::menu(room)),
    );
    column![
        // `Space` captures nothing, so events still reach the resident well
        // in the app bar beneath this layer.
        Space::new().height(Length::Fixed(theme::APP_BAR_H)),
        stack![
            backdrop,
            container(card)
                .width(Length::Fill)
                .height(Length::Fill)
                .padding(iced::Padding {
                    top: theme::GAP_XS,
                    right: 0.0,
                    bottom: 0.0,
                    left,
                })
                .align_x(alignment::Horizontal::Left)
                .align_y(alignment::Vertical::Top),
        ]
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn results<'a>(
    shelf: &'a Shelf,
    player: &'a PlayerState,
    adding_to_playlist: bool,
) -> Element<'a, Message> {
    if shelf.search_result_count() == 0 {
        let room = theme::active();
        return container(
            column![
                text(format!("Nothing matches “{}”", shelf.query.trim()))
                    .size(theme::SIZE_EMPHASIS)
                    .line_height(theme::LEADING_EMPHASIS)
                    .color(room.paper_dim),
                text("Esc or × clears the search.")
                    .size(theme::SIZE_META)
                    .line_height(theme::LEADING_META)
                    .color(room.paper_faint),
            ]
            .spacing(theme::GAP_SM),
        )
        .padding(theme::HANG)
        .into();
    }

    let tracks = shelf.songs.len();
    let albums = shelf.search_albums.len();
    let mut list = column![section("Tracks", tracks, true)];
    let (first_track, end_track) = visible_range(
        shelf.search_scroll_offset,
        shelf.search_viewport_h,
        SECTION_H,
        tracks,
    );
    list = list.push(Space::new().height(Length::Fixed(rows(first_track))));
    for index in first_track..end_track {
        list = list.push(track_row(shelf, player, index, adding_to_playlist));
    }
    list = list.push(Space::new().height(Length::Fixed(rows(tracks - end_track))));
    list = list.push(section("Albums", albums, false));

    let album_origin = SECTION_H + rows(tracks) + SECTION_H;
    let (first_album, end_album) = visible_range(
        shelf.search_scroll_offset,
        shelf.search_viewport_h,
        album_origin,
        albums,
    );
    list = list.push(Space::new().height(Length::Fixed(rows(first_album))));
    for index in first_album..end_album {
        list = list.push(album_row(shelf, player, index));
    }
    list = list.push(Space::new().height(Length::Fixed(rows(albums - end_album))));

    scrollable(list)
        .id(scroll_id())
        .on_scroll(Message::SearchScrolled)
        .direction(scrollable::Direction::Vertical(theme::wall_scrollbar()))
        .style(move |_theme, status| {
            theme::scrollbar(theme::active(), theme::active().plinth, status)
        })
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "non-negative pixel offsets are deliberately quantized to bounded row indices"
)]
fn visible_range(offset: f32, viewport: f32, origin: f32, len: usize) -> (usize, usize) {
    if len == 0 {
        return (0, 0);
    }
    let viewport = if viewport > 0.0 { viewport } else { DROPOVER_H };
    let local_start = (offset - origin).max(0.0);
    let local_end = (offset + viewport - origin).max(0.0);
    let first = ((local_start / ROW_H).floor() as usize).saturating_sub(OVERSCAN_ROWS);
    let end = ((local_end / ROW_H).ceil() as usize)
        .saturating_add(OVERSCAN_ROWS)
        .min(len);
    (first.min(len), end.max(first.min(len)))
}

fn section(label: &str, count: usize, show_guide: bool) -> Element<'_, Message> {
    let room = theme::active();
    let guide: Element<'_, Message> = if show_guide {
        text("↑↓ select · ←→ action · Enter confirm")
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            .color(room.paper_faint)
            .into()
    } else {
        Space::new().width(Length::Shrink).into()
    };
    container(
        row![
            text(label.to_uppercase())
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .font(theme::MEDIUM)
                .color(room.paper_dim),
            Space::new().width(Length::Fill),
            guide,
            text(count.to_string())
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.paper_faint),
        ]
        .spacing(theme::GAP_SM)
        .align_y(iced::Alignment::Center),
    )
    .height(Length::Fixed(SECTION_H))
    .padding(theme::pad(0.0, theme::GAP_MD))
    .align_y(alignment::Vertical::Center)
    .into()
}

fn track_row<'a>(
    shelf: &'a Shelf,
    player: &'a PlayerState,
    index: usize,
    adding_to_playlist: bool,
) -> Element<'a, Message> {
    let room = theme::active();
    let song = &shelf.songs[index];
    let Some(Content::SearchTrack { album, row: track }) = shelf.search_result_content(index)
    else {
        return Space::new().height(Length::Fixed(ROW_H)).into();
    };
    let content = Content::SearchTrack { album, row: track };
    let selected = shelf.search_selection.is(content);
    let playing = player.now_playing_path() == Some(song.path.as_path());
    let action = |label, action| {
        button(
            text(label)
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META),
        )
        .height(Length::Fixed(theme::STEPPER_HIT))
        .padding(theme::pad(theme::GAP_XS, theme::GAP_SM))
        .style(move |_theme, status| {
            theme::segment(room, status, selected && shelf.search_action == action)
        })
        .on_press(Message::SearchAction(content, action))
    };
    let mut actions = row![action("Play", Action::Play)].spacing(theme::GAP_SM);
    if adding_to_playlist {
        actions = actions.push(action("Add to playlist", Action::End));
    } else if player.queued() == 0 {
        actions = actions.push(action("Enqueue", Action::End));
    } else {
        actions = actions
            .push(action("Next", Action::Next))
            .push(action("End", Action::End));
    }
    button(
        row![
            container(column![
                text(song.title.as_str())
                    .size(theme::SIZE_BODY)
                    .line_height(theme::LEADING_BODY)
                    .color(room.paper),
                text(format!(
                    "{} · {}",
                    song.artist,
                    song.album.as_deref().unwrap_or("Unknown Album")
                ))
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.paper_dim),
            ])
            .width(Length::Fill)
            .clip(true),
            actions,
            container(
                text(song.duration.map(vm::format_duration).unwrap_or_default())
                    .size(theme::SIZE_META)
                    .line_height(theme::LEADING_META)
                    .color(room.paper_faint),
            )
            .width(Length::Fixed(theme::DURATION_W))
            .align_x(alignment::Horizontal::Right),
        ]
        .spacing(theme::GAP_SM)
        .align_y(iced::Alignment::Center),
    )
    .width(Length::Fill)
    .height(Length::Fixed(ROW_H))
    .padding(theme::pad(theme::GAP_XS, theme::GAP_MD))
    .style(move |_theme, status| {
        theme::selectable_track_row(room, room.plinth, status, playing, selected)
    })
    .on_press(Message::ContentPressed(content))
    .into()
}

fn album_row<'a>(shelf: &'a Shelf, player: &'a PlayerState, index: usize) -> Element<'a, Message> {
    let room = theme::active();
    let id = shelf.search_albums[index];
    let Some(album) = shelf.album(id) else {
        return Space::new().height(Length::Fixed(ROW_H)).into();
    };
    let content = Content::Album(id);
    let selected = shelf.search_selection.is(content);
    let playing = player.playing_album() == Some(id);
    let title = album.title.as_deref().unwrap_or("Unknown Album");
    let under = album.year.map_or_else(
        || album.artist.label().to_owned(),
        |year| format!("{} · {year}", album.artist.label()),
    );
    let verb = |label, message| {
        button(
            text(label)
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META),
        )
        .height(Length::Fixed(theme::STEPPER_HIT))
        .padding(theme::pad(theme::GAP_XS, theme::GAP_SM))
        .style(move |_theme, status| theme::word_button(room, room.plinth, status))
        .on_press(message)
    };
    button(
        row![
            container(column![
                text(title)
                    .size(theme::SIZE_BODY)
                    .line_height(theme::LEADING_BODY)
                    .color(room.paper),
                text(under)
                    .size(theme::SIZE_META)
                    .line_height(theme::LEADING_META)
                    .color(room.paper_dim),
            ])
            .width(Length::Fill)
            .clip(true),
            verb("Play", Message::SearchAction(content, Action::Play)),
            verb("Open", Message::SearchOpenAlbum(id)),
        ]
        .spacing(theme::GAP_SM)
        .align_y(iced::Alignment::Center),
    )
    .width(Length::Fill)
    .height(Length::Fixed(ROW_H))
    .padding(theme::pad(theme::GAP_XS, theme::GAP_MD))
    .style(move |_theme, status| {
        theme::selectable_track_row(room, room.plinth, status, playing, selected)
    })
    .on_press(Message::ContentPressed(content))
    .into()
}

#[cfg(test)]
mod tests {
    use super::visible_range;

    #[test]
    fn virtualization_keeps_a_small_overscanned_window() {
        let (first, end) = visible_range(8_000.0, 400.0, 32.0, 10_000);
        assert!(first > 100);
        assert!(end - first < 20);
        assert!(end < 10_000);
    }
}
