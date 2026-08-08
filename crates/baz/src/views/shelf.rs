//! The album shelf: the virtualized grid, one tile, and the empty states.
//!
//! The *math* this surface spends — how many columns fit, how large the works
//! in them are, which rows intersect the viewport, how tall the spacers
//! standing in for the rest are — is [`crate::shelf::Grid`], so the two
//! shelves never read as one thing (see [`crate::views`]). This file draws a
//! grid it is handed; it computes none of it.

use iced::widget::{Space, button, column, container, image as iced_image, row, scrollable, text};
use iced::{Element, Length, alignment};

use crate::app::{Message, Shelf, scroll_id};
use crate::player::PlayerState;
use crate::shelf::Grid;
use crate::views::gradient_block;
use crate::{theme, vm};

/// The virtualized grid: spacer, visible rows, spacer (see
/// [`crate::shelf::Grid`]). The grid block is centred in the viewport;
/// spacers are width-shrunk so the column keeps the rows' width and partial
/// last rows stay left-aligned within the shelf.
///
/// Each row is [`crate::shelf::Grid::block_width`] wide — the width the
/// *columns* take, not the width the items in that row happen to fill. That is what keeps a
/// filtered shelf anchored: narrowing 29 albums to 1 used to teleport the
/// survivor from the first column position to the middle of the window,
/// because the row it was in was only as wide as itself and the block was
/// centred on that. The block is now the same width whatever survives, so the
/// result stays where its column is. It is also what keeps a partial *last*
/// row left-aligned with the full rows above it.
///
/// The grid comes from [`Shelf::grid`], not from the viewport directly, so a
/// tile click that opens the inspector does not reflow the grid — nor resize
/// every sleeve in it — out from under the double-click it might be the first
/// half of.
pub(crate) fn view<'a>(shelf: &'a Shelf, player: &'a PlayerState) -> Element<'a, Message> {
    if shelf.visible.is_empty() {
        return empty_state(shelf);
    }
    let hang = shelf.grid();
    let cols = hang.columns;
    let total_rows = hang.rows(shelf.visible.len());
    let (first_row, end_row) =
        hang.visible_rows(shelf.scroll_offset, shelf.grid_size.height, total_rows);

    // The wall's top edge is a HANG like every other edge; each row carries
    // its own trailing HANG in `row_h`, so the bottom edge is one too. The
    // horizontal margins are not padding at all — they are what centring a
    // `block_width` block in the viewport leaves, which is how they come out
    // at exactly HANG whenever the art is uncapped.
    let mut grid = column![].padding(iced::Padding {
        top: theme::HANG,
        right: 0.0,
        bottom: 0.0,
        left: 0.0,
    });
    grid = grid.push(Space::with_height(Length::Fixed(
        hang.spacer_height(first_row),
    )));
    for r in first_row..end_row {
        let mut cells = row![]
            .spacing(hang.gutter)
            .align_y(alignment::Vertical::Top);
        for c in 0..cols {
            let Some(&album_index) = shelf.visible.get(r * cols + c) else {
                break;
            };
            if let Some(album) = shelf.albums.get(album_index) {
                cells = cells.push(tile(
                    shelf,
                    hang,
                    album,
                    player.playing_album() == Some(album.id),
                ));
            }
        }
        grid = grid.push(
            container(cells)
                .width(Length::Fixed(hang.block_width()))
                .height(Length::Fixed(hang.row_h))
                .align_y(alignment::Vertical::Top),
        );
    }
    grid = grid.push(Space::with_height(Length::Fixed(
        hang.spacer_height(total_rows - end_row),
    )));

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
    let room = theme::active();
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
            .line_height(theme::LEADING_EMPHASIS)
            .color(room.paper_dim)
    ]
    .spacing(theme::GAP_SM)
    .align_x(iced::Alignment::Center);
    if let Some(hint) = hint {
        content = content.push(
            text(hint)
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.paper_faint),
        );
    }
    container(content).center(Length::Fill).into()
}

/// One album tile: the sleeve (thumbnail or gradient placeholder, with
/// a soft shelf shadow) over a quiet two-line caption. The playing
/// album swaps the shadow for a lamp-amber halo and gains a lamp dot by
/// its title; selection and hover raise the tile's card.
///
/// The caption block is [`theme::CAPTION_H`] tall — **two lines, always** —
/// rather than as tall as its contents. Content-driven, a title that took two
/// lines pushed its artist line down and broke the baseline every other
/// caption in the row sat on; in a grid whose whole job is calm repetition
/// that was the loudest thing on screen after the artwork. Reserving the block
/// costs nothing (the row pitch already has the room) and the title clips at one
/// line instead, which is the failure the shelf can afford.
fn tile<'a>(
    shelf: &'a Shelf,
    hang: Grid,
    album: &'a vm::AlbumVm,
    playing: bool,
) -> Element<'a, Message> {
    let room = theme::active();
    let edge = hang.art;
    let art: Element<'_, Message> = match shelf.thumbs.peek(&album.id) {
        Some(handle) => iced_image(handle.clone())
            .width(Length::Fixed(edge))
            .height(Length::Fixed(edge))
            .into(),
        None => gradient_block(album.id, edge),
    };
    let sleeve = container(art).style(move |_theme| theme::sleeve(room, playing));
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
            .line_height(theme::LEADING_BODY)
            .font(theme::MEDIUM)
            .wrapping(text::Wrapping::None),
    );
    // Selected *and on screen*: a tile whose panel is hidden behind the queue,
    // or dismissed outright, must not keep claiming the selection styling —
    // the highlight says "that panel is showing this album".
    let selected = shelf.selection.showing_album(album.id);
    // Two one-line lanes, not one two-line box: a title iced lays out over two
    // lines despite `Wrapping::None` clips at its own lane's edge instead of
    // pushing the artist out of the block that was reserved to hold it still
    // (see [`theme::CAPTION_LINE_H`]). Both lanes are top-aligned so what
    // survives a clip is the *first* line, not the middle of two.
    let caption_lane = |content: Element<'a, Message>| {
        container(content)
            .width(Length::Fixed(edge))
            .height(Length::Fixed(theme::CAPTION_LINE_H))
            .align_y(alignment::Vertical::Top)
            .clip(true)
    };
    let caption_block = column![
        caption_lane(title_row.into()),
        caption_lane(
            text(caption)
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.paper_dim)
                .wrapping(text::Wrapping::None)
                .into(),
        ),
    ]
    .width(Length::Fixed(edge))
    .height(Length::Fixed(theme::CAPTION_H));
    button(
        column![sleeve, caption_block]
            .spacing(theme::GAP_LG)
            .width(Length::Fixed(edge)),
    )
    .width(Length::Fixed(edge))
    // The work and its label — not the row. The row's remaining `HANG` is the
    // gap to the row below, and a hit area that swallowed it would make the
    // whole wall one contiguous target with no space between the works.
    .height(Length::Fixed(hang.row_h - theme::HANG))
    .padding(0)
    .style(move |_theme, status| theme::tile(room, status, selected))
    .on_press(Message::AlbumClicked(album.id))
    .into()
}

/// The playing album's lamp dot: a small amber circle, the amplifier's
/// power light.
fn lamp_dot() -> Element<'static, Message> {
    let room = theme::active();
    container(Space::new(
        Length::Fixed(theme::DOT),
        Length::Fixed(theme::DOT),
    ))
    .style(move |_theme| theme::lamp_dot(room))
    .into()
}
