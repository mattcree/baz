//! The album side panel: large art, header, edition selector, Play, and the
//! selected edition's track list.

use std::time::Duration;

use iced::widget::{
    Column, Space, button, column, container, image as iced_image, row, scrollable, text,
};
use iced::{Element, Length, alignment};

use crate::app::{Message, Shelf};
use crate::player::{Availability, PlayerState};
use crate::views::{close_button, gradient_block};
use crate::{theme, vm};

/// Side-panel inner padding (logical px).
const PANEL_PAD: f32 = theme::GAP_XL;

/// The album side panel: large art, a title/artist/meta header, the
/// edition selector when the album is owned in more than one format, the
/// primary Play action, and the selected edition's numbered track list
/// (durations in monospace, right-hugged). In a build without audio
/// output the button is hidden; with an unusable or closed engine it
/// renders disabled.
pub(crate) fn view<'a>(
    shelf: &'a Shelf,
    album: &'a vm::AlbumVm,
    player: &'a PlayerState,
) -> Element<'a, Message> {
    let playing = player.playing_album() == Some(album.id);
    let art_edge = theme::PANEL_W - 2.0 * PANEL_PAD;
    let art: Element<'_, Message> = match shelf.thumbs.peek(&album.id) {
        Some(handle) => iced_image(handle.clone())
            .width(Length::Fixed(art_edge))
            .into(),
        None => gradient_block(album.id, art_edge),
    };
    let sleeve = container(art).style(move |_theme| theme::sleeve(playing));
    let chosen = shelf.edition_choice.get(&album.id).copied();
    let edition = vm::selected_edition(album, chosen);
    // A soundtrack grouped under one album artist keeps its per-cue
    // composer credits; an ordinary album gains no extra line.
    let per_track_artists = album.track_artists_vary;
    let rows: Vec<Element<'_, Message>> = edition
        .map(|edition| {
            edition
                .tracks
                .iter()
                .map(|track| track_row(track, per_track_artists))
                .collect()
        })
        .unwrap_or_default();

    // The dismissal ✕ sits in a row of its own above the sleeve — the same
    // slot, in the same panel width, as the queue's, so closing a panel is one
    // control in one place whichever is on screen.
    let header_row = row![
        Space::with_width(Length::Fill),
        close_button("Close the album panel"),
    ]
    .align_y(iced::Alignment::Center);
    let mut content =
        column![header_row, sleeve, album_header(album, edition)].spacing(theme::GAP_MD);
    // Only a genuinely multi-format album gets a control; a single-format
    // album must look exactly as it always did.
    if album.editions.len() > 1 {
        content = content.push(edition_selector(album, edition));
    }
    if *player.availability() != Availability::NotBuilt {
        content = content.push(
            button(
                container(
                    text("Play album")
                        .size(theme::SIZE_BODY)
                        .font(theme::MEDIUM),
                )
                .width(Length::Fill)
                .align_x(alignment::Horizontal::Center),
            )
            .width(Length::Fill)
            .padding(theme::pad(theme::GAP_SM, 0.0))
            .style(theme::primary)
            .on_press_maybe(
                player
                    .engine_ready()
                    .then_some(Message::PlayAlbum(album.id)),
            ),
        );
    }
    let hint = if *player.availability() == Availability::NotBuilt {
        "Esc closes · built without audio output"
    } else {
        "Esc closes · double-click a tile to play"
    };
    content = content.push(track_list(rows)).push(
        text(hint)
            .size(theme::SIZE_CAPTION)
            .color(theme::PAPER_FAINT),
    );

    container(content)
        .width(Length::Fixed(theme::PANEL_W))
        .height(Length::Fill)
        .padding(PANEL_PAD)
        .style(theme::panel)
        .into()
}

/// The scrolling track list, with the lane its scrollbar needs kept clear.
///
/// iced draws a `scrollable`'s bar **over** the right edge of its contents
/// rather than beside them, so a list long enough to scroll was clipping the
/// last character of every duration — `1:15` read as `1:1`. The rows stop at
/// [`theme::scroll_gutter`] instead, which reserves exactly the width the bar
/// occupies (the two are one token, see [`theme::SCROLLBAR_W`]).
///
/// The lane is reserved **whether or not the list currently overflows**, so a
/// twelfth track arriving does not shunt eleven durations sideways; it is the
/// same reserved-slot rule the bottom bar's timestamps and signal note follow.
/// Nothing else about the panel moves: the padding, the number column, the
/// gaps and the panel width are untouched.
fn track_list(rows: Vec<Element<'_, Message>>) -> Element<'_, Message> {
    scrollable(
        Column::with_children(rows)
            .spacing(theme::GAP_XXS)
            .padding(theme::scroll_gutter()),
    )
    .direction(scrollable::Direction::Vertical(theme::list_scrollbar()))
    .style(theme::scrollbar)
    .height(Length::Fill)
    .into()
}

/// The side panel's header: album title over artist over a quiet
/// year · tracks · total-time meta line, and — when the scan read one — the
/// selected edition's encoding fingerprint under it.
///
/// The counts describe `edition`, not the album: with two rips on disk, "24
/// tracks" would be a number nothing on screen adds up to.
fn album_header<'a>(
    album: &'a vm::AlbumVm,
    edition: Option<&'a vm::EditionVm>,
) -> Element<'a, Message> {
    let title = album.title.as_deref().unwrap_or("Unknown Album");
    let artist = album.artist.label();
    let tracks = edition.map_or(0, |edition| edition.tracks.len());
    let mut meta: Vec<String> = Vec::new();
    if let Some(year) = album.year {
        meta.push(year.to_string());
    }
    meta.push(match tracks {
        1 => "1 track".to_owned(),
        n => format!("{n} tracks"),
    });
    let total: Duration = edition
        .into_iter()
        .flat_map(|edition| edition.tracks.iter())
        .filter_map(|t| t.duration)
        .sum();
    if total > Duration::ZERO {
        meta.push(vm::format_duration(total));
    }
    let mut header = column![
        text(title).size(theme::SIZE_TITLE).font(theme::SEMIBOLD),
        text(artist)
            .size(theme::SIZE_EMPHASIS)
            .color(theme::PAPER_DIM),
        text(meta.join(" · "))
            .size(theme::SIZE_META)
            .font(theme::MONO)
            .color(theme::PAPER_FAINT),
    ]
    .spacing(theme::GAP_XS);
    if let Some(line) = edition.and_then(vm::EditionVm::encoding_line) {
        header = header.push(
            text(line)
                .size(theme::SIZE_META)
                .font(theme::MONO)
                .color(theme::PAPER_FAINT),
        );
    }
    header.into()
}

/// The edition selector: a quiet segmented control, one segment per format
/// the album is owned in, in the library's best-first order.
///
/// Shown only when there is a choice to make — a single-format album carries
/// no control at all, so the ordinary case gains no chrome. The choice
/// changes what the panel lists and what Play queues, and nothing else; it
/// never interrupts what is already playing.
fn edition_selector<'a>(
    album: &'a vm::AlbumVm,
    selected: Option<&'a vm::EditionVm>,
) -> Element<'a, Message> {
    let selected_key = selected.map(|edition| edition.key);
    let mut segments = row![].spacing(theme::GAP_XXS);
    for edition in &album.editions {
        let is_selected = selected_key == Some(edition.key);
        segments = segments.push(
            button(
                container(
                    text(edition.key.label())
                        .size(theme::SIZE_META)
                        .font(theme::MEDIUM)
                        .wrapping(text::Wrapping::None),
                )
                .width(Length::Fill)
                .align_x(alignment::Horizontal::Center),
            )
            .width(Length::Fill)
            .padding(theme::pad(theme::GAP_XS, theme::GAP_SM))
            .style(move |_theme, status| theme::segment(status, is_selected))
            .on_press(Message::EditionSelected(album.id, edition.key)),
        );
    }
    container(segments)
        .width(Length::Fill)
        .padding(theme::SEGMENT_INSET)
        .style(theme::segmented)
        .into()
}

/// One track-list row: right-aligned number, title, monospace duration.
/// Rows are not interactive in v0.1, so they carry no hover affordance —
/// no false signals.
///
/// With `show_artist`, the track's own artist sits under its title in the
/// quiet meta style — the same title-over-artist stack the now-playing bar
/// uses. It is passed in rather than decided here because the answer is a
/// property of the whole album ([`vm::AlbumVm::track_artists_vary`]): every
/// row of a soundtrack shows its composer, or none does.
fn track_row(track: &vm::TrackVm, show_artist: bool) -> Element<'_, Message> {
    let number = track.number.map(|n| n.to_string()).unwrap_or_default();
    let duration = track.duration.map(vm::format_duration).unwrap_or_default();
    let mut title = column![
        text(track.title.as_str())
            .size(theme::SIZE_BODY)
            .wrapping(text::Wrapping::None)
    ]
    .spacing(theme::GAP_XXS);
    if let Some(artist) = track.artist.as_deref().filter(|_| show_artist) {
        title = title.push(
            text(artist)
                .size(theme::SIZE_META)
                .color(theme::PAPER_DIM)
                .wrapping(text::Wrapping::None),
        );
    }
    container(
        row![
            container(
                text(number)
                    .size(theme::SIZE_META)
                    .font(theme::MONO)
                    .color(theme::PAPER_FAINT)
            )
            .width(Length::Fixed(theme::TRACK_NO_W))
            .align_x(alignment::Horizontal::Right),
            container(title).width(Length::Fill),
            text(duration)
                .size(theme::SIZE_META)
                .font(theme::MONO)
                .color(theme::PAPER_FAINT),
        ]
        .spacing(theme::GAP_SM)
        .align_y(iced::Alignment::Center),
    )
    .padding(theme::pad(theme::GAP_XS, theme::GAP_XS))
    .into()
}
