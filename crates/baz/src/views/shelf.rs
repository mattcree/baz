//! The album shelf: the virtualized grid, one tile, and the empty states.
//!
//! The *math* this surface spends — how many columns fit, which rows
//! intersect the viewport, how tall the spacers standing in for the rest
//! are — is [`crate::shelf`], imported here as `geometry` so the two shelves
//! never read as one thing (see [`crate::views`]).

use iced::widget::{Space, button, column, container, image as iced_image, row, scrollable, text};
use iced::{Element, Length, alignment};

use crate::app::{Message, Shelf, scroll_id};
use crate::player::PlayerState;
use crate::shelf::{self as geometry, ART_PX, CELL_H, CELL_W, GRID_PADDING};
use crate::views::gradient_block;
use crate::{theme, vm};

/// Horizontal tile padding: centers [`ART_PX`] artwork inside [`CELL_W`].
const TILE_PAD_H: f32 = (CELL_W - ART_PX) / 2.0;
/// Vertical tile padding.
const TILE_PAD_V: f32 = theme::GAP_MD;

/// The virtualized grid: spacer, visible rows, spacer (see
/// [`geometry`](crate::shelf)). The grid block is centered in the viewport;
/// spacers are width-shrunk so the column keeps the rows' width and partial
/// last rows stay left-aligned within the shelf.
pub(crate) fn view<'a>(shelf: &'a Shelf, player: &'a PlayerState) -> Element<'a, Message> {
    if shelf.visible.is_empty() {
        return empty_state(shelf);
    }
    let cols = geometry::columns(shelf.grid_size.width);
    let total_rows = geometry::total_rows(shelf.visible.len(), cols);
    let (first_row, end_row) =
        geometry::visible_rows(shelf.scroll_offset, shelf.grid_size.height, total_rows);

    let mut grid = column![].padding(GRID_PADDING);
    grid = grid.push(Space::with_height(Length::Fixed(geometry::spacer_height(
        first_row,
    ))));
    for r in first_row..end_row {
        let mut cells = row![];
        for c in 0..cols {
            let Some(&album_index) = shelf.visible.get(r * cols + c) else {
                break;
            };
            if let Some(album) = shelf.albums.get(album_index) {
                cells = cells.push(tile(shelf, album, player.playing_album() == Some(album.id)));
            }
        }
        grid = grid.push(container(cells).height(Length::Fixed(CELL_H)));
    }
    grid = grid.push(Space::with_height(Length::Fixed(geometry::spacer_height(
        total_rows - end_row,
    ))));

    scrollable(
        container(grid)
            .width(Length::Fill)
            .align_x(alignment::Horizontal::Center),
    )
    .id(scroll_id())
    .on_scroll(Message::Scrolled)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

/// The shelf with nothing to show: a zero-result search, the first
/// moments of a scan, or a genuinely empty folder. Quiet text, no modal.
fn empty_state(shelf: &Shelf) -> Element<'_, Message> {
    let query = shelf.query.trim();
    let (line, hint) = if query.is_empty() {
        if shelf.scanning {
            (
                "The shelf fills as the scan finds your music…".to_owned(),
                None,
            )
        } else {
            (
                "No albums here yet".to_owned(),
                Some("baz rescans this folder each time it starts"),
            )
        }
    } else {
        (
            format!("Nothing matches “{query}”"),
            Some("Esc clears the search"),
        )
    };
    let mut content = column![
        text(line)
            .size(theme::SIZE_EMPHASIS)
            .color(theme::PAPER_DIM)
    ]
    .spacing(theme::GAP_SM)
    .align_x(iced::Alignment::Center);
    if let Some(hint) = hint {
        content = content.push(text(hint).size(theme::SIZE_META).color(theme::PAPER_FAINT));
    }
    container(content).center(Length::Fill).into()
}

/// One album tile: the sleeve (thumbnail or gradient placeholder, with
/// a soft shelf shadow) over a quiet two-line caption. The playing
/// album swaps the shadow for a lamp-amber halo and gains a lamp dot by
/// its title; selection and hover raise the tile's card.
fn tile<'a>(shelf: &'a Shelf, album: &'a vm::AlbumVm, playing: bool) -> Element<'a, Message> {
    let art: Element<'_, Message> = match shelf.thumbs.peek(&album.id) {
        Some(handle) => iced_image(handle.clone())
            .width(Length::Fixed(ART_PX))
            .height(Length::Fixed(ART_PX))
            .into(),
        None => gradient_block(album.id, ART_PX),
    };
    let sleeve = container(art).style(move |_theme| theme::sleeve(playing));
    let title = album.title.as_deref().unwrap_or("Unknown Album");
    // The *album* artist: one tile per album, captioned by whoever the
    // album is filed under, not by whichever composer happened to be
    // first (see `vm::AlbumArtistVm`).
    let artist = album.artist.label();
    let caption = match album.year {
        Some(year) => format!("{artist} · {year}"),
        None => artist.to_owned(),
    };
    let mut title_row = row![]
        .spacing(theme::GAP_XS)
        .align_y(iced::Alignment::Center);
    if playing {
        title_row = title_row.push(lamp_dot());
    }
    title_row = title_row.push(
        text(title)
            .size(theme::SIZE_BODY)
            .font(theme::MEDIUM)
            .wrapping(text::Wrapping::None),
    );
    let selected = shelf.selected == Some(album.id);
    button(
        column![
            sleeve,
            column![
                title_row,
                text(caption)
                    .size(theme::SIZE_META)
                    .color(theme::PAPER_DIM)
                    .wrapping(text::Wrapping::None),
            ]
            .spacing(theme::GAP_XXS),
        ]
        .spacing(theme::GAP_SM)
        .width(Length::Fixed(ART_PX)),
    )
    .width(Length::Fixed(CELL_W))
    .height(Length::Fixed(CELL_H))
    .padding(theme::pad(TILE_PAD_V, TILE_PAD_H))
    .style(move |_theme, status| theme::tile(status, selected))
    .on_press(Message::AlbumClicked(album.id))
    .into()
}

/// The playing album's lamp dot: a small amber circle, the amplifier's
/// power light.
fn lamp_dot() -> Element<'static, Message> {
    container(Space::new(
        Length::Fixed(theme::DOT),
        Length::Fixed(theme::DOT),
    ))
    .style(theme::lamp_dot)
    .into()
}
