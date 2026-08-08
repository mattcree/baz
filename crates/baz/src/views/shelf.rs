//! The album shelf: the virtualized grid, one tile, and the empty states.
//!
//! The *math* this surface spends — how many columns fit, which rows
//! intersect the viewport, how tall the spacers standing in for the rest
//! are — is [`crate::shelf`], imported here as `geometry` so the two shelves
//! never read as one thing (see [`crate::views`]).

use iced::widget::{
    Space, button, column, container, image as iced_image, mouse_area, row, scrollable, text,
};
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
///
/// Each row is [`geometry::block_width`] wide — the width the *columns* take,
/// not the width the items in that row happen to fill. That is what keeps a
/// filtered shelf anchored: narrowing 29 albums to 1 used to teleport the
/// survivor from the first column position to the middle of the window,
/// because the row it was in was only as wide as itself and the block was
/// centred on that. The block is now the same width whatever survives, so the
/// result stays where its column is. It is also what keeps a partial *last*
/// row left-aligned with the full rows above it.
///
/// The column count comes from [`Shelf::columns`], not from the viewport
/// directly, so a tile click that opens the inspector does not reflow the grid
/// out from under the double-click it might be the first half of.
pub(crate) fn view<'a>(shelf: &'a Shelf, player: &'a PlayerState) -> Element<'a, Message> {
    if shelf.visible.is_empty() {
        return empty_state(shelf);
    }
    let cols = shelf.columns();
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
        grid = grid.push(
            container(cells)
                .width(Length::Fixed(geometry::block_width(cols)))
                .height(Length::Fixed(CELL_H)),
        );
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

/// The shelf with nothing to show: a zero-result search, the first moments of
/// a scan, or a genuinely empty folder. Quiet type on the wall, centred, and
/// nothing else.
///
/// # Three states, one shape
///
/// A heading at the emphasis size, one line of meta under it saying what to do
/// about it, and — where the state is *temporary* — a third line naming the
/// thing that is happening. The same shape every time,
/// so the three read as one surface in three moods rather than as three
/// screens; it is the section shape the Settings place uses, at the scale of a
/// whole wall.
///
/// **No spinner and no progress bar** (`docs/REFUSALS.md`): the shelf filling
/// with covers *is* the scan indicator, and this text exists only for the
/// seconds before the first cover lands. It is why the scanning line says what
/// will happen rather than how far along it is — a fraction here would be a
/// progress bar spelled out in words.
///
/// The lines are **left-aligned within a centred block**, not centred
/// individually. Ragged-right is what type does; three centred lines of
/// different lengths make a diamond, which is the one shape a gallery label
/// never is.
fn empty_state(shelf: &Shelf) -> Element<'_, Message> {
    let query = shelf.query.trim();
    let (line, hint, note) = if query.is_empty() {
        if shelf.scanning {
            (
                "Reading your music…".to_owned(),
                Some("Covers land on the wall as they are read."),
                Some("Nothing waits for the scan to finish."),
            )
        } else {
            (
                "Nothing on the wall yet".to_owned(),
                Some("baz reads this folder again each time it starts."),
                None,
            )
        }
    } else {
        (
            format!("Nothing matches “{query}”"),
            Some("Esc clears the search."),
            None,
        )
    };
    let mut content = column![
        text(line)
            .size(theme::SIZE_EMPHASIS)
            .line_height(theme::LEADING_EMPHASIS)
            .color(theme::PAPER_DIM)
    ]
    .spacing(theme::GAP_SM)
    .align_x(iced::Alignment::Start);
    if let Some(hint) = hint {
        content = content.push(
            text(hint)
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(theme::PAPER_FAINT),
        );
    }
    if let Some(note) = note {
        content = content.push(
            text(note)
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(theme::PAPER_MUTED),
        );
    }
    container(content).center(Length::Fill).into()
}

/// One album tile: **artwork, type, and a rule.**
///
/// The sleeve (thumbnail or quietened gradient placeholder) over a two-line
/// wall label, with a state rule under the label at art width. That is the
/// whole inventory, and it is the direction's first rule made literal — *the
/// shelf contains exactly two kinds of thing, artwork and type*
/// (`.interface-design/system.md` §1.2, ADR-0017 step 14).
///
/// # What left
///
/// The card. Hover used to raise a [`theme::PLINTH`] rectangle behind the
/// sleeve and selection raised a [`theme::PLINTH_LIT`] one with a 2 px
/// hairline edge around it — a third kind of thing, drawn around a work, in a
/// gallery that has no lines on its walls. The sleeve's contact shadow left
/// with it (`SHADOW` measured 1.04 : 1 over the wall: a cost with no signal).
///
/// # What states look like now
///
/// | State | Mark |
/// |---|---|
/// | rest | none |
/// | hover | 1 px [`theme::HAIRLINE_STRONG`] rule under the label |
/// | selected | 2 px [`theme::PAPER_FAINT`] rule under the label |
/// | playing | composes with either: the lamp halo around the art, and the dot before the title |
///
/// Hover against selection is a 2× thickness and a ~4× ink step, which is what
/// the audit's *"hover and selection are nearly the same mark"* finding asked
/// for; one surface step and a hairline never gave it that. Neither mark is
/// the accent — selecting a record is not playing one — so the amber in this
/// grid still means exactly one thing.
///
/// The rule's lane is reserved at [`theme::SELECTION_EDGE`] in every state, so
/// a pointer crossing the wall moves nothing. Hover comes from the shelf's own
/// [`Shelf::hovered_album`] rather than from the button's status, because the
/// rule is the button's *sibling* and iced 0.13 tells a style function only
/// about itself.
///
/// # The label
///
/// [`theme::CAPTION_H`] tall — **two lines, always** — rather than as tall as
/// its contents. Content-driven, a title that took two lines pushed its artist
/// line down and broke the baseline every other caption in the row sat on; in
/// a grid whose whole job is calm repetition that was the loudest thing on
/// screen after the artwork. Reserving the block costs nothing ([`CELL_H`]
/// already had the room) and the title clips at one line instead, which is the
/// failure the shelf can afford.
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
            .line_height(theme::LEADING_BODY)
            .font(theme::MEDIUM)
            .color(theme::PAPER)
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
            .width(Length::Fixed(ART_PX))
            .height(Length::Fixed(theme::CAPTION_LINE_H))
            .align_y(alignment::Vertical::Top)
            .clip(true)
    };
    let hovered = shelf.hovered_album == Some(album.id);
    // The artist line lifts one rung of the ink ramp under the pointer — the
    // other half of ADR-0017 step 14's hover state, and the half that works
    // when the rule is off the bottom of a scrolled viewport.
    let caption_ink = if hovered {
        theme::PAPER_DIM
    } else {
        theme::PAPER_FAINT
    };
    let caption_block = column![
        caption_lane(title_row.into()),
        caption_lane(
            text(caption)
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(caption_ink)
                .wrapping(text::Wrapping::None)
                .into(),
        ),
    ]
    .width(Length::Fixed(ART_PX))
    .height(Length::Fixed(theme::CAPTION_H));
    let label_block = column![caption_block, state_rule(hovered, selected)]
        .spacing(theme::GAP_XS)
        .width(Length::Fixed(ART_PX));
    mouse_area(
        button(
            column![sleeve, label_block]
                .spacing(theme::GAP_SM)
                .width(Length::Fixed(ART_PX)),
        )
        .width(Length::Fixed(CELL_W))
        .height(Length::Fixed(CELL_H))
        .padding(theme::pad(TILE_PAD_V, TILE_PAD_H))
        .style(move |_theme, status| theme::tile(status, selected))
        .on_press(Message::AlbumClicked(album.id)),
    )
    .on_enter(Message::TileEntered(album.id))
    .on_exit(Message::TileLeft(album.id))
    .into()
}

/// The tile's whole state vocabulary: a rule under the wall label, at art
/// width, in a lane that is [`theme::SELECTION_EDGE`] tall whatever it holds.
///
/// The lane is reserved rather than sized to the mark, which is the same
/// fixed-slot rule the bottom bar's timestamps follow and the reason the mark
/// can be 0, 1 or 2 px without a row of covers shifting under the pointer.
fn state_rule(hovered: bool, selected: bool) -> Element<'static, Message> {
    let thickness = theme::tile_rule_h(hovered, selected);
    container(
        container(Space::new(Length::Fill, Length::Fixed(thickness)))
            .width(Length::Fill)
            .height(Length::Fixed(thickness))
            .style(move |_theme| theme::tile_rule(hovered, selected)),
    )
    .width(Length::Fixed(ART_PX))
    .height(Length::Fixed(theme::SELECTION_EDGE))
    .align_y(alignment::Vertical::Top)
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
