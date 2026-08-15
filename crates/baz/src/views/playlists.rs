//! The saved-playlist collection: every playlist file as one collage tile.
//!
//! This is the root page for playlist browsing. It does not replace the
//! summoned picker panel: the panel answers “where should this track go?”,
//! while this place answers “which playlist do I want to open?”.
//!
//! # It is the Library's wall, with playlists on it
//!
//! The owner, twice: *"the Playlists page does not have the rail on the right
//! and seems to be different from the Library? it should not be significantly
//! different"*, and then *"a-z playlists should group alphabetically — use the
//! exact same pattern as the library please."* So the layout engine is
//! [`crate::shelf::Shelves`] — the record wall's own — the heading band and its
//! pinned copy are [`crate::views::shelf::group_band`] and
//! [`crate::views::shelf::pinned_band`], and the rail is the shared `Spine`.
//! What this module owns is what a cell and a heading *mean* here, which is the
//! only thing that differs: a made thing rather than a found one.
//!
//! # The strip says less than it did
//!
//! It led with the word `Playlists` and closed with a tally (`13 playlists`)
//! until the owner removed both — *"the playlists page does not need the word
//! 'playlists' at the top"*, *"no need for the playlist count and another
//! noise"*. The Library's strip ([`crate::views::top_bar`]) never carried
//! either, and this place is meant to be that place with different tiles on it.
//! What remains is the arrangement keys and, while a deletion is pending, the
//! name of the list awaiting the confirmation — a statement about the *place*,
//! which is what [`crate::views::place_header_led`]'s note slot is for.

use iced::widget::{
    Space, button, column, container, image as iced_image, mouse_area, row, scrollable, stack, text,
};
use iced::{Element, Length, alignment};

use crate::app::{Message, Shelf};
use crate::icon;
use crate::player::PlayerState;
use crate::playlists::{Cell, PanelRow, PlaylistOrder, Playlists, Wall};
use crate::selection::Content;
use crate::shelf::{Grid, Shelves};
use crate::theme;
use crate::views::{arrangement_key, place_header_led};

/// Draw every saved playlist in the shelf's shared work grid, grouped under the
/// Library's own heading bands.
pub(crate) fn view<'a>(
    shelf: &'a Shelf,
    playlists: &'a Playlists,
    player: &PlayerState,
    hang: Grid,
    scroll_offset: f32,
) -> Element<'a, Message> {
    let room = theme::active();
    // **The note is the pending deletion and nothing else.** A tally stood
    // here; the Library shows none, and a count of the tiles you are looking at
    // is not a statement about the place.
    let note = playlists.confirming_overview_delete.and_then(|id| {
        playlists
            .rows
            .iter()
            .find(|row| row.id == id)
            .map(|row| format!("Delete “{}”?", row.name))
    });
    let mut order = row![].spacing(theme::GAP_MD);
    for choice in PlaylistOrder::ALL {
        order = order.push(arrangement_key(
            choice.label(),
            choice == playlists.order,
            Message::PlaylistOrderSelected(choice),
        ));
    }
    let header = place_header_led(order.into(), note);

    let wall = playlists.wall();
    let shelves = Shelves::new(hang, &wall.counts);
    let runs = shelves.runs();
    let (first_run, end_run) = shelves.visible_runs(scroll_offset, shelf.grid_size.height);
    // One column in content coordinates, exactly as the record wall builds it:
    // everything off screen is a single spacer, so a collection of five
    // playlists and one of five hundred cost the same per frame.
    let mut grid = column![].width(Length::Fixed(hang.block_width()));
    let mut drawn = 0.0_f32;
    let spacer = |grid: iced::widget::Column<'a, Message>, to: f32, drawn: &mut f32| {
        let gap = (to - *drawn).max(0.0);
        *drawn = to;
        grid.push(Space::new().height(Length::Fixed(gap)))
    };
    for run in &runs[first_run..end_run] {
        grid = spacer(grid, run.top, &mut drawn);
        grid = grid.push(band(&wall, run.group, hang));
        drawn = run.rows_top(hang);
        let (first_row, end_row) = hang.visible_rows(
            scroll_offset - run.rows_top(hang),
            shelf.grid_size.height,
            run.rows,
        );
        grid = spacer(
            grid,
            run.rows_top(hang) + hang.spacer_height(first_row),
            &mut drawn,
        );
        for row_index in first_row..end_row {
            grid = grid.push(cells_row(
                shelf,
                playlists,
                player,
                &wall,
                hang,
                run.first + row_index * hang.columns,
                (run.len)
                    .saturating_sub(row_index * hang.columns)
                    .min(hang.columns),
            ));
        }
        drawn += hang.spacer_height(end_row - first_row);
    }
    grid = spacer(grid, shelves.height(), &mut drawn);

    // **No place padding here, and that is load-bearing.** The block is
    // centred in the scrollable's *content* measure — the outer width less
    // `theme::WALL_RESERVE`, which `theme::shelf_scrollbar` reserves — because
    // the pinned band centres in exactly that measure
    // (`views::shelf::pinned_band`). A `place_pad` on top of it would centre
    // the in-flow rows in a narrower box than the pinned heading and the two
    // would disagree, which is the 56 px jump `impl/sticky-header-alignment/`
    // records. The wall's own top air is `Shelves`' first `grid.hang`.
    let body = scrollable(
        container(grid)
            .width(Length::Fill)
            .align_x(alignment::Horizontal::Center),
    )
    .id(scroll_id())
    .on_scroll(Message::PlaylistsScrolled)
    // The body spans the window edge while reserving the rail's lane, just
    // like Library: the bar remains at the outer edge and tiles can never
    // slide beneath the index.
    .direction(scrollable::Direction::Vertical(theme::shelf_scrollbar()))
    .style(move |_theme, status| theme::scrollbar(room, room.wall, status))
    .width(Length::Fill)
    .height(Length::Fill);
    // **The pinned layer is always in the tree** — see `views::shelf::view` for
    // why a `stack` that came and went would rebuild the scrollable's state and
    // make the wall unscrollable. The lead run is never pinned: it has no
    // heading, and an opaque band with nothing in it is a blank strip over the
    // covers passing under it.
    let pinned = shelves
        .sticky(scroll_offset)
        .filter(|&run| wall.pinned(runs.get(run).map_or(usize::MAX, |run| run.group)))
        .and_then(|run| runs.get(run))
        .map(|run| band(&wall, run.group, hang));
    let body: Element<'a, Message> = stack![
        body,
        crate::views::shelf::pinned_band(pinned, hang, hang.block_width())
    ]
    .into();

    let (entries, current) = rail(&wall, &shelves, scroll_offset);
    crate::views::shelf::collection_scaffold(
        column![header, body].into(),
        crate::views::shelf::index_rail_from(entries, current, Message::PlaylistRailJumped),
    )
}

/// The saved-playlist collection's scroll identity. It is separate from the
/// record wall's identity because either place must retain its position while
/// the other is visited.
pub(crate) fn scroll_id() -> iced::widget::Id {
    iced::widget::Id::new("baz-playlists")
}

/// One run's heading, in the flow of the wall.
///
/// The lead run has none and draws an empty band of the same height, so the
/// create tile and `Favourites` stand on the wall's own rhythm rather than
/// starting a pixel higher than every lettered run below them.
fn band<'a>(wall: &Wall<'_>, group: usize, hang: Grid) -> Element<'a, Message> {
    let label = wall
        .headers
        .get(group)
        .and_then(Option::as_ref)
        .map(crate::vm::GroupHeaderVm::label)
        .unwrap_or_default();
    // **No door, at any key.** The Library's ARTIST headings open a person's
    // page (ADR-0035); a letter, a date bucket and the lead run name a break
    // rather than a subject, and this collection has no key that names one.
    crate::views::shelf::group_band(&label, None, hang, hang.block_width())
}

/// One row of cells, at the block's width so a partial last row stays
/// left-aligned with the full rows above it.
fn cells_row<'a>(
    shelf: &'a Shelf,
    playlists: &'a Playlists,
    player: &PlayerState,
    wall: &Wall<'a>,
    hang: Grid,
    first: usize,
    len: usize,
) -> Element<'a, Message> {
    let mut cells = row![].spacing(hang.gutter);
    for offset in 0..len {
        match wall.cells.get(first + offset) {
            Some(Cell::New) => cells = cells.push(ghost_tile(hang)),
            Some(Cell::List(playlist)) => {
                cells = cells.push(tile(
                    shelf,
                    playlist,
                    hang,
                    playlists.hovered == Some(playlist.id),
                    playlists.confirming_overview_delete == Some(playlist.id),
                    player.engine_ready(),
                ));
            }
            None => break,
        }
    }
    container(cells)
        .width(Length::Fixed(hang.block_width()))
        .height(Length::Fixed(hang.row_h))
        .align_y(alignment::Vertical::Top)
        .into()
}

/// Project the active ordering into the shared index-rail vocabulary, and say
/// which entry the wall is standing on.
///
/// Alphabetical order gets initials; chronological orders get the same elapsed
/// buckets the Library uses. Labels therefore always describe the run they jump
/// to — an A–Z rail is never painted over a date-sorted collection. The lead
/// run is not indexed: it has no heading for an entry to name, and it is
/// already at the top of the wall, which is where `Home` goes.
fn rail(
    wall: &Wall<'_>,
    shelves: &Shelves,
    scroll_offset: f32,
) -> (Vec<crate::rail::RailEntry>, Option<usize>) {
    let headers = wall.rail_headers();
    let mut entries = crate::rail::entries(wall.key, &headers);
    for entry in &mut entries {
        entry.shelf = entry.shelf.map(Wall::run_of);
    }
    // Where the wall is: the run at the top of the viewport, mapped onto the
    // rail's own list — the last present entry at or before it, which is the
    // exact entry where every run has one and the letter of the run you are in
    // where several share it. (`None < Some(_)`, so the comparison needs the
    // presence guard.)
    let here = shelves
        .run_at(scroll_offset)
        .and_then(|run| shelves.runs().get(run))
        .map(|run| run.group);
    let current = here.and_then(|group| {
        entries
            .iter()
            .rposition(|entry| entry.present() && entry.shelf <= Some(group))
    });
    (entries, current)
}

/// **The create affordance, as a tile.**
///
/// The owner: *"the new playlist should be like a ghost playlist with a + in
/// the middle called 'New Playlist' on the playlist page, not a button."* It
/// was a word button in the strip, standing in the row that says how the
/// collection is *arranged* — a control about making a thing, filed with the
/// controls about looking at things.
///
/// **Identical geometry to a real tile** — the same edge, the same mat, the
/// same caption block and the same state rule lane — so nothing moves when the
/// ghost becomes a list. That is the panel's ghost row (`views::playlist_panel`)
/// at wall scale, and it carries the same two rules: the sleeve is
/// [`theme::ghost_sleeve`] with the drawn [`icon::Glyph::Plus`] and never
/// anything resembling artwork, because a placeholder that looked like a
/// collage would be the interface inventing a playlist; and it answers the
/// pointer like its neighbours, because a pressable thing that does not is
/// unresponsive.
///
/// The mark is drawn at [`theme::GHOST_MARK_PX`], which is the sprite's own
/// raster edge — so it is pixel-exact rather than an upscale of a 20 px glyph.
fn ghost_tile<'a>(hang: Grid) -> Element<'a, Message> {
    let room = theme::active();
    let edge = hang.art;
    let work = (edge - 2.0 * theme::SLEEVE_MAT).max(0.0);
    let field = container(
        iced_image(icon::handle(icon::Glyph::Plus))
            .width(Length::Fixed(theme::GHOST_MARK_PX))
            .height(Length::Fixed(theme::GHOST_MARK_PX))
            .opacity(theme::GLYPH_OPACITY),
    )
    .width(Length::Fixed(work))
    .height(Length::Fixed(work))
    .align_x(alignment::Horizontal::Center)
    .align_y(alignment::Vertical::Center)
    .style(move |_theme| theme::ghost_sleeve(room));
    let sleeve = container(field)
        .width(Length::Fixed(edge))
        .height(Length::Fixed(edge))
        .padding(theme::SLEEVE_MAT)
        .style(move |_theme| theme::sleeve_mat(room));
    let body = column![
        sleeve,
        column![caption_lane(
            edge,
            text("New Playlist")
                .size(theme::SIZE_BODY)
                .line_height(theme::LEADING_BODY)
                .font(theme::MEDIUM)
                .color(room.paper_dim)
                .wrapping(text::Wrapping::None)
                .into()
        )]
        .height(Length::Fixed(theme::CAPTION_H)),
        crate::views::shelf::state_rule(0.0, false, edge)
    ]
    .spacing(theme::GAP_XS)
    .width(Length::Fixed(edge));
    button(body)
        .padding(0)
        .style(move |_theme, status| theme::tile(room, status, false))
        .on_press(Message::NewPlaylistOpen)
        .into()
}

/// One caption lane: a single line, top-aligned, clipped at the tile's edge —
/// the record wall's own, so a long name behaves here exactly as it does there.
fn caption_lane(edge: f32, content: Element<'_, Message>) -> Element<'_, Message> {
    container(content)
        .width(Length::Fixed(edge))
        .height(Length::Fixed(theme::CAPTION_LINE_H))
        .align_y(alignment::Vertical::Top)
        .clip(true)
        .into()
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
    let art = crate::views::playlist_sleeve_marked(
        shelf,
        &playlist.art,
        &playlist.name,
        work,
        crate::views::default_playlist_mark(playlist.id),
    );
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
    // **The name, and nothing under it.** The second lane carried
    // `Playlist · 12 · 41:03` — the kind, spelled out under every tile on a
    // wall of nothing but playlists, and a count the owner asked to be rid of.
    // The lane itself stays, empty, because [`theme::CAPTION_H`] is the grid's
    // and a tile one line shorter than the Library's would break the pitch this
    // place shares with it. `PanelRow::counts` is untouched: the returns lane
    // and the picker panel draw it beside albums, where the kind is the point.
    let body = column![
        sleeve,
        column![caption_lane(
            edge,
            text(playlist.name.clone())
                .size(theme::SIZE_BODY)
                .line_height(theme::LEADING_BODY)
                .font(theme::MEDIUM)
                .wrapping(text::Wrapping::None)
                .into()
        )]
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
