//! The album shelf: the virtualized grid, one tile, and the empty states.
//!
//! The *math* this surface spends — how many columns fit, how large the works
//! in them are, which rows intersect the viewport, how tall the spacers
//! standing in for the rest are — is [`crate::shelf::Grid`], so the two
//! shelves never read as one thing (see [`crate::views`]). This file draws a
//! grid it is handed; it computes none of it.

use iced::widget::{
    Space, button, column, container, image as iced_image, mouse_area, row, scrollable, stack, text,
};
use iced::{Element, Length, alignment};

use crate::app::{Message, Shelf, scroll_id};
use crate::player::PlayerState;
use crate::playlists::Collecting;
use crate::shelf::{Grid, Run, Shelves};
use crate::spine::{Slot, Spine};
use crate::views::{gradient_block, section_rule};
use crate::{icon, rail, theme, vm};

/// **The wall**: the shelved, virtualized grid, its pinned group header, and
/// the index rail down its right-hand side.
///
/// Three things sit side by side and on top of each other here, and the
/// arrangement is the whole of step 8's composition:
///
/// ```text
/// ┌───────────────────────────────────────────┬──────┬─┐
/// │ scrollable( shelf headers + rows )        ╎ rail ╎█│  the scrollable takes
/// │ ─────────────────────────────────────     ╎      ╎█│  the whole width and
/// │ ↑ the pinned header, stacked over the top ╎      ╎█│  reserves the two
/// └───────────────────────────────────────────┴──────┴─┘  right-hand lanes
///  ←────────── Shelf::grid_width ────────────→ ←── 112 ─→  (theme::WALL_RESERVE)
/// ```
///
/// # The bar is on the window's edge, and the rail is stacked under it
///
/// The bar used to be drawn at the right edge of the *scrollable*, with the
/// rail's [`theme::INDEX_LANE_W`] 108 standing outboard of it — measured on a
/// 1280 × 860 frame, a bar at x 1168–1171 with the window's edge at 1280. The
/// owner: *"scroll bar is in a strange location… it seems to have padding on
/// the right"*. The fix is the one the returns lane already made in his words
/// (*"the scrollbar should be at the edge of it"*, [`crate::views::lane`]):
/// **the content keeps its inset; only the bar reaches the edge.**
///
/// So the scrollable is given the whole body width and reserves both lanes
/// ([`theme::shelf_scrollbar`]), and the rail is a stacked layer *under* it,
/// right-aligned, at exactly the x it always had. iced draws a vertical bar at
/// the far right of a scrollable's outer bounds regardless of what it
/// reserves, so the bar lands in the rail's own window gutter — 4 px of a
/// 40 px gutter that never held ink. The rail is the **lower** layer because
/// iced hands the topmost layer the pointer first; a rail on top would own the
/// 4 px the bar is drawn in and the bar would be ungrabbable.
///
/// What that costs: the rail's press band ran to the window's edge, which made
/// flinging the pointer at the edge a guaranteed hit (Fitts). It now stops
/// 4 px short, and those 4 px belong to the bar. The band is still 104 px
/// wide, and what the edge now hits is the *other* affordance for the same
/// surface — the one whose whole reason for existing is the gesture the rail
/// cannot do. That is the trade, and it is the price of the bar being where
/// people look for it.
///
/// # The one alignment edge everything shares
///
/// Every row is [`Grid::block_width`] wide — the width the *columns* take, not
/// the width the items in that row happen to fill — and the shelf headers are
/// laid out in exactly the same block. So **a header's left edge is the first
/// column's left edge**, pinned or not, at every width and every column count.
/// The wall introduces no x-position of its own.
///
/// That is also what keeps a filtered shelf anchored: narrowing 29 albums to 1
/// used to teleport the survivor from the first column position to the middle
/// of the window, because the row it was in was only as wide as itself and the
/// block was centred on that. The block is the same width whatever survives.
///
/// # The vertical unit is `HANG`
///
/// The wall's top edge, the gap between two rows, the gap above a header and
/// the header's own band are all `HANG` or arithmetic on it — see
/// [`crate::shelf::Shelves`], which owns the numbers and proves the pinned
/// header's hand-over from them.
///
/// The grid comes from [`Shelf::grid`], not from the viewport directly, so a
/// tile click that opens the inspector does not reflow the grid — nor resize
/// every sleeve in it — out from under the double-click it might be the first
/// half of.
pub(crate) fn view<'a>(
    shelf: &'a Shelf,
    player: &'a PlayerState,
    lamp: f32,
    collecting: Collecting,
) -> Element<'a, Message> {
    if shelf.visible.is_empty() {
        return empty_state(shelf);
    }
    let hang = shelf.grid();
    let shelves = shelf.shelves();
    let runs = shelves.runs();
    let (first_run, end_run) = shelves.visible_runs(shelf.scroll_offset, shelf.grid_size.height);

    // One column, laid out in content coordinates: everything not on screen is
    // a single spacer, so a wall of 500 genre shelves costs the same per frame
    // as a wall of five.
    let mut grid = column![].width(Length::Fixed(hang.block_width()));
    let mut drawn = 0.0_f32;
    let spacer = |grid: iced::widget::Column<'a, Message>, to: f32, drawn: &mut f32| {
        let gap = (to - *drawn).max(0.0);
        *drawn = to;
        grid.push(Space::with_height(Length::Fixed(gap)))
    };
    for run in &runs[first_run..end_run] {
        grid = spacer(grid, run.top, &mut drawn);
        grid = grid.push(header_band(shelf, hang, *run, hang.block_width()));
        drawn = run.rows_top(hang);
        let (first_row, end_row) = hang.visible_rows(
            shelf.scroll_offset - run.rows_top(hang),
            shelf.grid_size.height,
            run.rows,
        );
        grid = spacer(
            grid,
            run.rows_top(hang) + hang.spacer_height(first_row),
            &mut drawn,
        );
        for r in first_row..end_row {
            grid = grid.push(shelf_row(shelf, player, hang, *run, r, lamp, collecting));
        }
        drawn += hang.spacer_height(end_row - first_row);
    }
    grid = spacer(grid, shelves.height(), &mut drawn);

    let room = theme::active();
    let wall = scrollable(
        container(grid)
            .width(Length::Fill)
            .align_x(alignment::Horizontal::Center),
    )
    .id(scroll_id())
    .on_scroll(Message::Scrolled)
    // **The wall's scrollbar** ([`theme::shelf_scrollbar`]): 4 px, in the
    // room's hairline, reserving [`theme::WALL_RESERVE`] — its own lane *and*
    // the rail's — inside the scrollable, so no cover is ever drawn under
    // either and the bar itself is drawn on the window's edge. The rail under
    // it still says *where you are* and still names the shelf it jumps to;
    // what the bar adds is the one gesture the rail has no answer to — drag to
    // the end. The owner's decision, 2026-08-09; the product's
    // two-vertical-strips entry records it.
    .direction(scrollable::Direction::Vertical(theme::shelf_scrollbar()))
    .style(move |_theme, status| theme::scrollbar(room, room.wall, status))
    .width(Length::Fill)
    .height(Length::Fill);

    // **The pinned layer is always in the tree**, even with nothing pinned,
    // and that is load-bearing rather than tidy. iced 0.13 keys widget state
    // by position *and type* in the tree: a `stack` that appeared the moment a
    // header pinned put the `scrollable` one level deeper, `Tree::diff` saw a
    // different widget where the scrollable had been, and its state — the
    // scroll offset — was rebuilt from nothing. The wall then snapped back to
    // the top, which un-pinned the header, which removed the stack, which
    // restored the offset: a two-frame oscillation that made the wall
    // unscrollable past the first shelf. The layer is constant; only what it
    // draws changes.
    let pinned = shelves
        .sticky(shelf.scroll_offset)
        .and_then(|index| runs.get(index))
        .copied();
    let wall = stack![wall, pinned_header(shelf, hang, pinned, hang.block_width())];
    // **The Songs section** (doc 09 §5): under a query with matching tracks,
    // the ranked track-level answers render above the filtered wall — two
    // sections, separate. In the same left cell as the wall, so both centre
    // against the same width and the rail stays the sibling of the whole
    // body; absent (not empty) whenever there are no song answers, in which
    // case the composition is exactly what it always was.
    let body: Element<'a, Message> = if shelf.songs.is_empty() {
        wall.into()
    } else {
        column![
            songs_section(shelf, player, collecting, hang.block_width()),
            wall
        ]
        .width(Length::Fill)
        .into()
    };
    // **The rail is the layer under the body**, right-aligned in its own lane,
    // at the same x it occupied as a `row!` sibling — see this function's docs
    // for why it is under rather than over, and what the 4 px it yields to the
    // bar cost.
    stack![
        container(index_rail(shelf, &shelves))
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(alignment::Horizontal::Right),
        body
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

/// **The Songs section's block**: a `Songs` rule, up to [`vm::SONGS`] ranked
/// track rows, then an `Albums` rule naming the filtered wall below — the
/// two sections the owner asked for, visibly separate (doc 09 §5, S1).
///
/// It is laid out **on the wall's own ruler**: the block is
/// [`Grid::block_width`] wide and centred exactly as the wall's rows are, so
/// its left edge is the first column's left edge and the section introduces
/// no x-position of its own (law L5 — the wall permits `HANG` and the hang's
/// derived column edges, nothing else). Its top air is [`theme::HANG`], the
/// wall's own top-edge unit.
///
/// The rows are the full match set's ranked head, not its whole: the wall
/// below is the exhaustive answer, in covers.
fn songs_section<'a>(
    shelf: &'a Shelf,
    player: &'a PlayerState,
    collecting: Collecting,
    block: f32,
) -> Element<'a, Message> {
    let interactive = player.engine_ready();
    let mut rows = column![].spacing(theme::GAP_XS);
    for (index, song) in shelf.songs.iter().enumerate() {
        // The song resolved onto the record the wall holds: the wall id and
        // the row in its **selected edition** — what the press and the `+`
        // both spend ([`vm::song_row`]). A row a rescan has just unmoored
        // asks for nothing rather than playing a track nobody pointed at.
        let resolved = shelf.album(song.album_id).and_then(|album| {
            let chosen = shelf.edition_choice.get(&album.id).copied();
            vm::song_row(album, chosen, song).map(|row| (album.id, row))
        });
        let press = resolved
            .filter(|_| interactive)
            .map(|(id, row)| Message::PlayTrack(id, row));
        // The row's mark follows `TrackStarted`, never the click (S1): the
        // dot lights when the engine says this file is sounding.
        let playing = player.now_playing_path() == Some(song.path.as_path());
        rows = rows.push(song_row(
            song,
            index,
            playing,
            press,
            resolved,
            collecting,
            shelf.hovered_song == Some(index),
        ));
    }
    container(
        column![
            // The rule carries the accelerator it accelerates (doc 11 §5
            // P6.4): Enter's meaning while a query stands was true and
            // unannounced — the era printed the shortcut beside the verb,
            // and without menus the section's own rule is where the verb
            // lives.
            crate::views::section_rule_noted("Songs", "Enter plays the first match."),
            rows,
            section_rule("Albums")
        ]
        .spacing(theme::GAP_SM)
        .width(Length::Fixed(block)),
    )
    .width(Length::Fill)
    .padding(iced::Padding {
        top: theme::HANG,
        // **The same two lanes the wall's scrollable reserves**
        // ([`theme::WALL_RESERVE`]): the section is a sibling of the
        // scrollable, not a child of it, so nothing reserves them on its
        // behalf — and a block centred in the whole body would sit 56 px right
        // of the wall's own centre line, which is the one x-position this file
        // is not allowed to introduce.
        right: theme::WALL_RESERVE,
        ..iced::Padding::ZERO
    })
    .align_x(alignment::Horizontal::Center)
    .into()
}

/// One row of the Songs section: the reserved mark lane (the lamp dot when
/// this file is sounding), the title, `artist · record` with **the record's
/// name a door to its page**, the right-aligned duration, and the reserved
/// `+` slot — a list row (doc 09 §5: *rows play; tiles navigate*), one line
/// tall.
///
/// **The press is a needle-drop** (ADR-0023 §2 extended to this section):
/// [`Message::PlayTrack`] with the record's wall id and the song's row in
/// the selected edition — the record page's exact path, decided by
/// [`crate::player::PlayerState::play_from`], never a new grammar. The door
/// sends [`Message::AlbumClicked`] and captures its own press (iced's
/// `button` returns `Captured` for a child's press before its own
/// `on_press` fires), and the `+` is the album page's own slot sending
/// [`Message::AddTrackToPlaylist`].
///
/// Every lane is a token the list surfaces already share —
/// [`theme::TRACK_NO_W`], [`theme::DURATION_W`], [`theme::STEPPER_HIT`] —
/// and the row's box is `2 × GAP_XS + STEPPER_HIT` = [`theme::TRANSPORT_HIT`],
/// the product's one control height (law L7).
fn song_row<'a>(
    song: &'a vm::SongVm,
    index: usize,
    playing: bool,
    press: Option<Message>,
    resolved: Option<(u64, usize)>,
    collecting: Collecting,
    hovered: bool,
) -> Element<'a, Message> {
    let room = theme::active();
    let duration = song.duration.map(vm::format_duration).unwrap_or_default();
    let marker: Element<'a, Message> = if playing {
        lamp_dot()
    } else {
        Space::with_width(Length::Fixed(0.0)).into()
    };
    // The playing row's title takes the medium weight the bar, the queue and
    // the album page give the same string.
    let heading = text(song.title.as_str())
        .size(theme::SIZE_BODY)
        .line_height(theme::LEADING_BODY)
        .color(room.paper)
        .wrapping(text::Wrapping::None);
    let heading = if playing {
        heading.font(theme::MEDIUM)
    } else {
        heading
    };
    let lane = |content: Element<'a, Message>, width: f32| {
        container(content)
            .width(Length::Fixed(width))
            .height(Length::Fixed(theme::STEPPER_HIT))
            .align_x(alignment::Horizontal::Right)
            .align_y(alignment::Vertical::Center)
    };
    let body = button(
        row![
            // The mark lane the album page and the queue rows reserve, at
            // the same width, so the dot arriving moves no text.
            lane(marker, theme::TRACK_NO_W),
            container(
                row![
                    heading,
                    text(format!("{} ·", song.artist))
                        .size(theme::SIZE_META)
                        .line_height(theme::LEADING_META)
                        .color(room.paper_dim)
                        .wrapping(text::Wrapping::None),
                    album_door(song),
                ]
                .spacing(theme::GAP_SM)
                .align_y(iced::Alignment::Center),
            )
            .width(Length::Fill)
            .height(Length::Fixed(theme::STEPPER_HIT))
            .align_y(alignment::Vertical::Center)
            .clip(true),
            lane(
                text(duration)
                    .size(theme::SIZE_META)
                    .line_height(theme::LEADING_META)
                    .color(room.paper_faint)
                    .wrapping(text::Wrapping::None)
                    .into(),
                theme::DURATION_W,
            ),
        ]
        .spacing(theme::GAP_SM)
        .align_y(iced::Alignment::Center),
    )
    .width(Length::Fill)
    // No horizontal inset: the mark lane starts on the block's own edge and
    // the duration lane ends on it (law L5) — the album page's rule.
    .padding(theme::pad(theme::GAP_XS, 0.0))
    .style(move |_theme, status| theme::track_row(room, room.wall, status, playing))
    .on_press_maybe(press);
    // The row's right press opens the track menu (doc 09 §5.2) — the album
    // page's exact target, because a resolved song row *is* that record's
    // row: same press, same `+`, same mirror. Unresolved, there is nothing
    // for a verb to act on, so there is no menu either.
    let target = resolved.map(|(id, row)| crate::menu::Target::Track { album: id, row });
    let with_menu = |element: Element<'a, Message>| match target {
        Some(target) => crate::menu::area(element, target),
        None => element,
    };
    if !collecting.available {
        return with_menu(body.into());
    }
    let offered = collecting.panel_open || hovered;
    let slot: Element<'a, Message> = match resolved {
        Some((id, row)) => {
            crate::views::page::transfer_slot(offered, Message::AddTrackToPlaylist(id, row))
        }
        None => Space::with_width(Length::Fixed(theme::STEPPER_HIT)).into(),
    };
    with_menu(
        mouse_area(
            row![body, slot]
                .spacing(theme::GAP_XS)
                .align_y(iced::Alignment::Center),
        )
        .on_enter(Message::SongRowEntered(index))
        .on_exit(Message::SongRowLeft(index))
        .into(),
    )
}

/// The record's name as **a door to its page** — the one navigation inside a
/// section whose rows otherwise play. A quiet word control
/// ([`theme::word_button`]: `paper_dim` at rest, full paper under the
/// pointer), [`theme::STEPPER_HIT`] tall — the named secondary square — so a
/// single-line row stays one line.
fn album_door(song: &vm::SongVm) -> Element<'_, Message> {
    let room = theme::active();
    button(
        container(
            text(song.album.as_deref().unwrap_or("Unknown Album"))
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .font(theme::MEDIUM)
                .wrapping(text::Wrapping::None),
        )
        .height(Length::Fill)
        .align_y(alignment::Vertical::Center),
    )
    .height(Length::Fixed(theme::STEPPER_HIT))
    .padding(theme::pad(0.0, theme::GAP_XS))
    .style(move |_theme, status| theme::word_button(room, room.wall, status))
    .on_press(Message::AlbumClicked(song.album_id))
    .into()
}

/// One row of works, at the block's width so a partial last row stays
/// left-aligned with the full rows above it.
fn shelf_row<'a>(
    shelf: &'a Shelf,
    player: &'a PlayerState,
    hang: Grid,
    run: Run,
    row_index: usize,
    lamp: f32,
    collecting: Collecting,
) -> Element<'a, Message> {
    let mut cells = row![]
        .spacing(hang.gutter)
        .align_y(alignment::Vertical::Top);
    for column_index in 0..hang.columns {
        let offset = row_index * hang.columns + column_index;
        if offset >= run.len {
            break;
        }
        let Some(&index) = shelf.visible.get(run.first + offset) else {
            break;
        };
        if let Some(album) = shelf.albums.get(index) {
            cells = cells.push(tile(shelf, player, hang, album, lamp, collecting));
        }
    }
    container(cells)
        .width(Length::Fixed(hang.block_width()))
        .height(Length::Fixed(hang.row_h))
        .align_y(alignment::Vertical::Top)
        .into()
}

/// A shelf's header where it lies, in the flow of the wall.
///
/// The band is **one hang** and takes it from the grid rather than from
/// `theme::SHELF_HEADER_H`, because the hang is the density's
/// ([`crate::shelf::Grid::header_h`]): a band fixed at 40 while the rows
/// around it zoomed would put the pinned header a few pixels out at every step
/// but the default, which is the one place in the wall a few pixels are
/// visible.
fn header_band(shelf: &Shelf, hang: Grid, run: Run, block: f32) -> Element<'_, Message> {
    container(header_line(shelf, run, block))
        .width(Length::Fixed(block))
        .height(Length::Fixed(hang.header_h()))
        .align_y(alignment::Vertical::Top)
        .into()
}

/// The same header, **pinned** to the top of the viewport while its shelf is
/// the one on screen (`docs/design/critique/02-surfaces.md`: *sticky in the
/// virtualizer*).
///
/// # How it sticks, and why nothing moves
///
/// iced 0.13 has no sticky positioning, so the band is a second layer in a
/// `stack` over the wall, drawn at the viewport's top edge in exactly the
/// geometry the in-flow band has: the same block, the same left edge, the same
/// line box at the same offset. [`Shelves::sticky`] then decides *when* it is
/// drawn, and its two hand-overs are exact rather than eased —
///
/// - it appears at the scroll offset where the in-flow header reaches y = 0,
///   which is where this one is drawn, so the swap is invisible;
/// - it disappears at the offset where the next shelf's band enters the lane,
///   which is the same offset at which this shelf's last row of covers leaves
///   it, so what replaces the pinned header is the next one arriving in the
///   flow and never a cover.
///
/// The band is opaque [`theme::shelf_header_band`] — wall on wall — across the
/// full width rather than the block's, because the covers passing beneath it
/// are the full width of the wall. There is no rule, no shadow and no lift: a
/// pinned header differs from an unpinned one in nothing a screenshot can
/// show, which is what makes this a position rather than a state, and is why
/// it needs no transition (a standing rule of the product — *no motion — hard cuts by
/// design*).
///
/// `run` is `None` when nothing is pinned, and the layer is still built: see
/// the note at the call site for why it may not come and go.
fn pinned_header(shelf: &Shelf, hang: Grid, run: Option<Run>, block: f32) -> Element<'_, Message> {
    let room = theme::active();
    let body: Element<'_, Message> = match run {
        Some(run) => container(header_line(shelf, run, block))
            .width(Length::Fixed(block))
            .height(Length::Fixed(hang.header_h()))
            .align_y(alignment::Vertical::Top)
            .into(),
        None => Space::new(Length::Fixed(block), Length::Fixed(0.0)).into(),
    };
    container(body)
        .width(Length::Fill)
        // Only the band, never the wall: a layer as tall as the viewport would
        // be a transparent sheet over every cover, and iced hands the topmost
        // layer of a `stack` the pointer first.
        .height(Length::Fixed(if run.is_some() {
            hang.header_h()
        } else {
            0.0
        }))
        .align_x(alignment::Horizontal::Center)
        .style(move |_theme| {
            if run.is_some() {
                theme::shelf_header_band(room)
            } else {
                iced::widget::container::Style::default()
            }
        })
        .into()
}

/// A group header's line of type: caps, tracked, at the quiet ink.
///
/// **Three axes away from the caption under a sleeve and one size smaller** —
/// caps where the caption is sentence case, tracked where the caption is not,
/// [`theme::Palette::paper_faint`] where a title is full paper. Hierarchy is
/// not carried by size alone here, which is what stops the wall from needing a
/// fourth type size to say a fourth thing.
///
/// The line box is [`theme::HEADING_LINE_H`] and it sits at the **top** of the
/// band, so the air above the ink is the previous row's trailing hang and the
/// air below is `hang − HEADING_LINE_H` — the same ratio at every density
/// step, since both numbers are the step's. See [`theme::SHELF_HEADER_H`] for
/// why it is that way round.
///
/// # Under ARTIST the header is a **door** (ADR-0035)
///
/// The key shelves one artist per shelf, so the header names a person the
/// product has a *place* for — and that place is now reached by pressing their
/// name on the wall, which is the door the artist tiles used to be (ADR-0035).
/// The breadcrumb on a record's page still opens the same place with the same
/// [`crate::vm::artist_id`], so the two doors cannot land on different pages.
///
/// **The type does not change when it is a door**: same face, same size, same
/// tracking, same [`theme::Palette::paper_faint`] ink at rest, same line box,
/// same height. What it gains is [`theme::word_button`]'s ground under the
/// pointer — the paint the record page's `Artist ›` breadcrumb already wears,
/// because the two doors lead to one place and should not be two kinds of
/// control. The hit box is **the word's own box, not the shelf's width**
/// (`padding(0)`, shrink width): a band-wide ground would light the whole wall
/// on a mouse-over, and a padded one would inset the header's ink off the
/// block's left edge, which is law L1's line.
///
/// The pinned copy is the same call, so the press is available wherever the
/// name is, and the two remain pixel-identical at rest — which is what makes
/// pinning a position rather than a state (see [`pinned_header`]).
///
/// Every other key's header is inert text, because a decade is not a place —
/// and neither is a letter. **A–Z's headers stay inert** (ADR-0035's third
/// amendment): `S` is a bucket of artists rather than one, so there is no
/// single place for it to lead to. The two keys draw the same line of type and
/// only one of them is a door, which is the difference between a header that
/// names a subject and one that names a break.
fn header_line(shelf: &Shelf, run: Run, block: f32) -> Element<'_, Message> {
    let room = theme::active();
    let header = shelf.groups.get(run.group).map(|group| &group.header);
    let label = header.map_or_else(String::new, vm::GroupHeaderVm::label);
    let word = text(theme::tracked(&label.to_uppercase()))
        .size(theme::SIZE_HEADING)
        .line_height(theme::LEADING_HEADING)
        .font(theme::MEDIUM)
        .color(room.paper_faint)
        .wrapping(text::Wrapping::None);
    let line: Element<'_, Message> = match header {
        Some(vm::GroupHeaderVm::Artist(artist)) => button(word)
            .height(Length::Fixed(theme::HEADING_LINE_H))
            .padding(0)
            .style(move |_theme, status| theme::word_button(room, room.wall, status))
            .on_press(Message::OpenArtist(vm::artist_id(artist)))
            .into(),
        _ => word.into(),
    };
    container(line)
        .width(Length::Fixed(block))
        .height(Length::Fixed(theme::HEADING_LINE_H))
        .clip(true)
        .into()
}

/// **The index rail**: a pure projection of the active group key, holding no
/// state of its own (`.interface-design/system.md` §7.2, ADR-0017 §1.7).
///
/// Everything it draws is derived, this frame, from the shelves the wall is
/// showing and where the wall is scrolled to — [`crate::rail`] is the whole of
/// the logic and it is pure. Change the key and the rail is simply built from
/// the new headers; there is nothing here to invalidate.
///
/// # It is type, not chrome
///
/// No ground, no edge, no chips, no rule between the lane and the wall. Three
/// inks and one of them is a weight:
///
/// | | ink | face |
/// |---|---|---|
/// | the shelf you are on | [`theme::Palette::paper`] | [`theme::MEDIUM`] |
/// | a value the collection has | [`theme::Palette::paper_faint`] | [`theme::SANS`] |
/// | a value it does not | [`theme::Palette::paper_muted`] | [`theme::SANS`] |
///
/// **Never the accent.** An index is navigation, not playback truth.
///
/// # Absent values are drawn
///
/// §7.2 is explicit that an index which hides its gaps lies about the
/// collection, so the letters, decades and buckets the library has nothing
/// under are drawn in the muted ink and are **inert** — no button, no hover,
/// no press. A control that did nothing when pressed would be the lie
/// the product's standing rules guards against from the other side.
///
/// # Where its edges are
///
/// Entries are right-aligned to the lane, so the rail's right edge is
/// [`theme::HANG`] from the window's — the one window gutter (law L1).
///
/// **This used to say the edge was one *"the `Settings` word above already
/// established"*, and that sentence outlived its subject twice.** ADR-0026
/// replaced the word with a 32 px gear, whose ink stands 8 px inside its box,
/// and ADR-0040 moved the gear into the app bar, where a phantom row seam put
/// it 16 px further in again — so the thing this lane was said to agree with
/// had been 25 px off it, unmeasured, since the morning of 2026-08-10. The
/// owner found it by looking. The rail was never the surface at fault: what it
/// draws its letters on is the same `W − HANG` the bottom bar's volume groove
/// and the last column of covers stand on, which is what made it the reference
/// once somebody measured all three
/// (`docs/design/impl/app-bar-gutter/`, ADR-0040's amendment §1).
///
/// An
/// entry wider than [`theme::INDEX_W`] grows *leftwards* to that cap and then
/// clips, which is why the lane keeps [`theme::INDEX_CLEARANCE`] between
/// itself and the last column of covers. The full value is never lost: it is
/// set in the shelf header one `HANG` to the left, at the same moment, in the
/// same voice.
///
/// # It is the wall's scroll affordance, and now it is the only one
///
/// ADR-0022 deleted the wall's scrollbar. The rail already did every job a bar
/// does — it says where you are (the current shelf in [`theme::MEDIUM`] at full
/// paper), it jumps (a press), and it does the one thing a bar cannot: it names
/// the place it is taking you to. Two vertical strips against the same edge,
/// one of them saying `ARTIST → S` and the other saying nothing, is one strip
/// too many.
///
/// # It magnifies under the pointer, and it fits itself
///
/// The drawing, the fisheye, the hit lane **and the elision** are [`Spine`]'s
/// — a hand-built widget, because a letter's size is layout and no style
/// function can touch layout, and because only the widget knows the height
/// the lane really has (fitting the rail up here against `grid_size`'s
/// estimated height is exactly how it once overflowed the window; see
/// [`Spine`]'s docs). This function owns everything the rail *says*: what the
/// entries are, and which one the wall is standing on.
///
/// # The lane's foot no longer carries anything
///
/// It closed with the density detents from ADR-0028 until the owner moved the
/// display options into the app bar on 2026-08-10 (ADR-0040 §5). The lane is
/// the spine and nothing else again, so the spine gets the whole height back —
/// its per-frame elision absorbs the taller lane exactly as it absorbed the
/// shorter one — and the lane's *width* is [`theme::INDEX_LANE_W`] either way,
/// so the wall beside it never reflowed by a pixel in either direction.
fn index_rail<'a>(shelf: &'a Shelf, shelves: &Shelves) -> Element<'a, Message> {
    let runs = shelves.runs();
    let headers: Vec<vm::GroupHeaderVm> = runs
        .iter()
        .filter_map(|run| shelf.groups.get(run.group))
        .map(|group| group.header.clone())
        .collect();
    let entries = rail::entries(shelf.group_key, &headers);
    // Where the wall is: the shelf at the top of the viewport, mapped onto the
    // rail's own list. This is the *only* thing the rail reads about scroll
    // position, and it reads it rather than remembering it.
    //
    // **The last present entry at or before the shelf you are on** — which is
    // the exact entry for the keys where every shelf has one, and the letter
    // of the run you are in for GENRE, where several shelves share an initial.
    // (`None < Some(_)`, so the comparison needs the presence guard.)
    let here = shelves.run_at(shelf.scroll_offset);
    let current = here.and_then(|run| {
        entries
            .iter()
            .rposition(|entry| entry.present() && entry.shelf <= Some(run))
    });
    let slots: Vec<Slot> = entries
        .iter()
        .enumerate()
        .map(|(index, entry)| Slot {
            label: entry.label.clone(),
            shelf: entry.shelf,
            current: Some(index) == current,
        })
        .collect();
    container(Spine::new(
        slots,
        current,
        theme::active(),
        Message::RailJumped,
    ))
    .width(Length::Fixed(theme::INDEX_LANE_W))
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
/// thing that is happening. The same shape every time, so the three read as one
/// surface in three moods rather than as three screens; it is the section shape
/// the Settings place uses, at the scale of a whole wall.
///
/// **No spinner and no progress bar** (a standing rule of the product): the shelf filling
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
    let room = theme::active();
    let query = shelf.query.trim();
    let (line, hint, note) = if query.is_empty() {
        if shelf.scanning {
            (
                "Reading your music…".to_owned(),
                Some("Covers land as they are read."),
                Some("Nothing waits for the scan to finish."),
            )
        } else {
            (
                "Nothing here yet".to_owned(),
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
            .color(room.paper_dim)
    ]
    .spacing(theme::GAP_SM)
    .align_x(iced::Alignment::Start);
    if let Some(hint) = hint {
        content = content.push(
            text(hint)
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.paper_faint),
        );
    }
    if let Some(note) = note {
        content = content.push(
            text(note)
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.heading()),
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
/// The card. Hover used to raise a plinth rectangle behind the sleeve and
/// selection raised a lit one with a 2 px hairline edge around it — a third
/// kind of thing, drawn around a work, in a gallery that has no lines on its
/// walls. The sleeve's contact shadow left with it (it measured 1.04 : 1 over
/// the wall: a cost with no signal).
///
/// # What states look like now
///
/// | State | Mark |
/// |---|---|
/// | rest | none |
/// | hover | 1 px hairline-strong rule under the label, plus the artist line lifting one rung of the ink ramp |
/// | selected | 2 px paper-faint rule under the label |
/// | playing | composes with either: the lamp halo around the art, and the dot before the title |
///
/// Hover against selection is a 2× thickness and a ~4× ink step, which is what
/// the audit's *"hover and selection are nearly the same mark"* finding asked
/// for; one surface step and a hairline never gave it that. Neither mark is the
/// accent — selecting a record is not playing one — so the amber in this grid
/// still means exactly one thing.
///
/// The rule's lane is reserved at [`theme::SELECTION_EDGE`] in every state, so
/// a pointer crossing the wall moves nothing. Hover comes from the shelf's own
/// [`Shelf::hovered_album`] rather than from the button's status, because the
/// rule is the button's *sibling* and iced 0.13 tells a style function only
/// about itself.
///
/// # The label
///
/// The caption block is [`theme::CAPTION_H`] tall — **two lines, always** —
/// rather than as tall as its contents. Content-driven, a title that took two
/// lines pushed its artist line down and broke the baseline every other
/// caption in the row sat on; in a grid whose whole job is calm repetition
/// that was the loudest thing on screen after the artwork. Reserving the block
/// costs nothing (the row pitch already has the room) and the title clips at one
/// line instead, which is the failure the shelf can afford.
pub(crate) fn tile<'a>(
    shelf: &'a Shelf,
    player: &'a PlayerState,
    hang: Grid,
    album: &'a vm::AlbumVm,
    lamp: f32,
    collecting: Collecting,
) -> Element<'a, Message> {
    let room = theme::active();
    let playing = player.playing_album() == Some(album.id);
    let engine = player.engine_ready();
    let edge = hang.art;
    // **The work, inside its reserved mat.** Every sleeve on the wall is drawn
    // at the grid's art edge less two [`theme::SLEEVE_MAT`]s, in every state.
    // The lane was the shuffle pool's ring — the mark the next two draws
    // carried — and when shuffle became a property of the player (2026-08-10)
    // there stopped being a draw to mark. The **geometry stays**: it is the
    // measure every grid constant, every capacity sum and every capture in
    // `docs/design/impl` is computed against, and re-deriving the whole wall to
    // reclaim 4 px would be a change to the collection to tidy away a mark.
    // What is gone is the ink; the mat is the wall's own colour.
    let work = (edge - 2.0 * theme::SLEEVE_MAT).max(0.0);
    let art: Element<'_, Message> = match shelf.thumbs.peek(&album.id) {
        Some(handle) => iced_image(handle.clone())
            .width(Length::Fixed(work))
            .height(Length::Fixed(work))
            .into(),
        None => gradient_block(album.id, work, 1.0),
    };
    // **The hover options** (see [`hover_options`]) — a layer over the work
    // and only while the pointer is on this tile. `stack` hands events to its
    // topmost layer first, so an option is reached before the sleeve under it.
    let art: Element<'_, Message> = if shelf.hovered_album == Some(album.id) {
        stack![art, hover_options(album.id, work, engine, collecting)].into()
    } else {
        art
    };
    // The halo warms over 200 ms when the light moves to this record, and is
    // simply absent on every other tile (ADR-0020 §2.5). The **dot** does not
    // fade: the halo is the light and the dot is the statement, and a statement
    // that arrives gradually is a statement you are not sure was made.
    let warmth = if playing { lamp } else { 0.0 };
    let sleeve = container(
        container(art)
            .width(Length::Fixed(work))
            .height(Length::Fixed(work))
            .style(move |_theme| theme::sleeve(room, warmth)),
    )
    .width(Length::Fixed(edge))
    .height(Length::Fixed(edge))
    .padding(theme::SLEEVE_MAT)
    .style(move |_theme| theme::sleeve_mat(room));
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
            .color(room.paper)
            .wrapping(text::Wrapping::None),
    );
    // **The record you last opened** ([`Shelf::opened`]), which is what the
    // 2 px rule marks now that there is no selection to mark. ADR-0022 made a
    // tile press *navigation*: the wall is replaced by the record's page, and
    // when `Esc` brings you back this rule is how you find your place again.
    // That is the whole of the mitigation for the round trip a page costs that
    // a column did not, and it is one rule under one label.
    let selected = shelf.opened == Some(album.id);
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
    // How far this tile's mark has travelled, from the shelf's one keyed tween
    // (ADR-0020 §2.3). Zero for every tile the pointer is not on, which is all
    // of them but one.
    let hovered = shelf.tile_hover.strength(album.id);
    // The artist line lifts one rung of the ink ramp under the pointer — the
    // other half of ADR-0017 step 14's hover state, and the half that still
    // reads when the rule is the thing your own hand is over.
    let caption_ink = theme::caption_ink(room, hovered);
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
    .width(Length::Fixed(edge))
    .height(Length::Fixed(theme::CAPTION_H));
    let label_block = column![caption_block, state_rule(hovered, selected, edge)]
        .spacing(theme::GAP_XS)
        .width(Length::Fixed(edge));
    // The tile's right press opens its mirror menu (doc 09 §5.2): open,
    // play, queue, add — every verb a press some visible control also
    // makes.
    crate::menu::area(
        mouse_area(
            button(
                column![sleeve, label_block]
                    .spacing(theme::GAP_LG)
                    .width(Length::Fixed(edge)),
            )
            .width(Length::Fixed(edge))
            // The work, its label and the label's rule — not the row. The row's
            // remaining hang is the gap to the row below, and a hit area that
            // swallowed it would make the whole wall one contiguous target with no
            // space between the works. `RULE_LANE_H` of that gap is spent on the
            // state rule, which is part of the label rather than part of the gap;
            // what is left between two works is still more than three quarters of a
            // hang.
            //
            // **The grid's hang, not `theme::HANG`.** This read the token, and at
            // `Dense` — where the hang is 28 — that made the box 12 px shorter than
            // the label it holds, so every tile on the wall clipped its artist line
            // while the title stayed. Caught on the pixels rather than in the
            // arithmetic, because the arithmetic was in a different file from the
            // number it was wrong about.
            .height(Length::Fixed(hang.row_h - hang.hang + RULE_LANE_H))
            .padding(0)
            .style(move |_theme, status| theme::tile(room, status, selected))
            .on_press(Message::AlbumClicked(album.id)),
        )
        .on_enter(Message::TileEntered(album.id))
        .on_exit(Message::TileLeft(album.id)),
        crate::menu::Target::Album { album: album.id },
    )
}

/// What the state rule costs the tile vertically: the gap under the label plus
/// the lane the rule is drawn in (logical px).
///
/// Taken out of the row's trailing hang rather than out of the work or the
/// label, because it belongs to the label and the label's block is a reserved
/// slot that may not shrink. At the `Balanced` step that leaves 34 px of clear
/// wall between one row's rule and the next row's sleeve, which is still more
/// space than any other product surveyed puts between two covers.
const RULE_LANE_H: f32 = theme::GAP_XS + theme::SELECTION_EDGE;

/// The tile's whole state vocabulary: a rule under the wall label, at art
/// width, in a lane that is [`theme::SELECTION_EDGE`] tall whatever it holds.
///
/// The lane is reserved rather than sized to the mark, which is the same
/// fixed-slot rule the bottom bar's timestamps follow and the reason the mark
/// can be 0, 1 or 2 px without a row of covers shifting under the pointer.
pub(crate) fn state_rule(hovered: f32, selected: bool, edge: f32) -> Element<'static, Message> {
    let room = theme::active();
    let thickness = theme::tile_rule_h(hovered, selected);
    container(
        container(Space::new(Length::Fill, Length::Fixed(thickness)))
            .width(Length::Fill)
            .height(Length::Fixed(thickness))
            .style(move |_theme| theme::tile_rule(room, hovered, selected)),
    )
    .width(Length::Fixed(edge))
    .height(Length::Fixed(theme::SELECTION_EDGE))
    .align_y(alignment::Vertical::Top)
    .into()
}

/// **The four options, laid over the sleeve the pointer is on** — the owner's
/// approved design, and the reversal of ADR-0032 §2 and of the product's
/// *nothing is ever drawn on a sleeve*. Both entries are rewritten to record
/// what was decided; neither is argued with here.
///
/// # The veil
///
/// A horizontal gradient of [`theme::Palette::recess`] that gathers at the
/// sleeve's **left** edge and is gone before the right one
/// ([`theme::hover_veil`]). That asymmetry is the design: the right of every
/// cover stays exactly as painted, so the record you are choosing about stays
/// the record you recognise. It is not a scrim rectangle, and a flat panel
/// would be a different decision rather than a simpler rendering of this one.
///
/// The stops were specified as an sRGB composite and are re-solved for a
/// renderer that blends in linear light before they are handed over — see
/// [`theme::veil_alpha`], which also says which way that correction runs and
/// why the direction is not the one this repo learned first.
///
/// # The reveal costs nothing
///
/// This whole layer exists only while [`Shelf::hovered_album`] names this
/// record — the boolean the `+` slot's reveal already uses
/// (`song_row`'s `offered`), not a tween. There is no new motion class, no
/// clock, and no frame is drawn because a pointer is somewhere. The tile's
/// 90 ms rule tween (ADR-0020 §2.3) is untouched and still the only thing on
/// the wall that moves.
///
/// # Geometry
///
/// Four rows sharing one left edge, each taking an equal share of the sleeve's
/// height as its hit band — 47 px at the tightest density baz draws, against
/// law L7's 32 px floor. The ink stops at [`theme::VEIL_INK_X`] and the hit
/// band at [`theme::VEIL_BAND_X`], both of them stops of the veil itself
/// rather than numbers of their own: type stops where the veil is still thick
/// enough to carry it over any sleeve, and the band stops well short of the
/// right edge so that **a press on the sleeve outside an option still opens
/// the record's page**.
///
/// # The presses
///
/// Every one of them is a message some visible control already sends, and the
/// tile's right-press menu remains the pointer-reachable twin of all four — so
/// this layer is an accelerator and never the only route (a standing rule of the product,
/// Accessibility). An option's press is **captured by the option**: iced's
/// `button` returns `Captured` for a child's press before its own `on_press`
/// fires (the same mechanism `album_door` relies on), so pressing `Play` here
/// cannot also open the page.
fn hover_options<'a>(
    album: u64,
    work: f32,
    engine: bool,
    collecting: Collecting,
) -> Element<'a, Message> {
    let listed: [Option<VeilOption>; theme::VEIL_OPTIONS] = [
        engine.then(|| VeilOption::accented(icon::Glyph::Play, "Play", Message::PlayAlbum(album))),
        engine.then(|| VeilOption::new(icon::Glyph::Queue, "Queue", Message::QueueAlbum(album))),
        collecting.available.then(|| {
            VeilOption::new(
                icon::Glyph::Plus,
                "Add to…",
                Message::AddAlbumToPlaylist(album),
            )
        }),
        Some(VeilOption::new(
            icon::Glyph::Open,
            "Open",
            Message::AlbumClicked(album),
        )),
    ];
    veil(work, listed.into_iter().flatten())
}

/// One row of a hover veil: its mark, its word, what it sends, and whether it
/// is the one that wears the accent.
///
/// A named type rather than a tuple because [`veil`] is now called from two
/// surfaces — the wall's tiles and Home's **All songs** tile — and a
/// four-element tuple read at a distance is exactly the kind of thing whose
/// third field nobody remembers.
pub(crate) struct VeilOption {
    glyph: icon::Glyph,
    label: &'static str,
    press: Message,
    accent: bool,
}

impl VeilOption {
    /// An ordinary option, in the room's own glyph ink.
    pub(crate) const fn new(glyph: icon::Glyph, label: &'static str, press: Message) -> Self {
        Self {
            glyph,
            label,
            press,
            accent: false,
        }
    }

    /// The one option that wears the accent — `Play`, and only `Play`
    /// ([`theme::veil_option_ink`] holds the discipline).
    pub(crate) const fn accented(glyph: icon::Glyph, label: &'static str, press: Message) -> Self {
        Self {
            glyph,
            label,
            press,
            accent: true,
        }
    }
}

/// **The veil and its options, over a `work`-px square.**
///
/// The geometry and the argument are [`hover_options`]'s; this is that function
/// with its *list* taken as a parameter, so that a surface which is not a record
/// — Home's **All songs** tile — can wear the wall's own layer rather than a
/// second one that looks like it. Options the caller has already filtered out
/// are simply absent, and the rows left divide the sleeve between them.
pub(crate) fn veil<'a>(
    work: f32,
    listed: impl IntoIterator<Item = VeilOption>,
) -> Element<'a, Message> {
    let room = theme::active();
    let band = work * theme::VEIL_BAND_X;
    let ink_lane = (work * theme::VEIL_INK_X - theme::VEIL_LEAD).max(0.0);
    let mut options = column![]
        .width(Length::Fixed(band))
        .height(Length::Fixed(work));
    for VeilOption {
        glyph,
        label,
        press,
        accent,
    } in listed
    {
        // One decision about which option wears the accent, made in the
        // theme and read here — see [`theme::veil_option_ink`], which also
        // records the one place this departs from the approved mockup.
        let ink = theme::veil_option_ink(room, accent);
        options = options.push(
            button(
                container(
                    row![
                        iced_image(icon::inked(glyph, ink))
                            .width(Length::Fixed(theme::ICON_PX))
                            .height(Length::Fixed(theme::ICON_PX)),
                        text(label)
                            .size(theme::SIZE_BODY)
                            .line_height(theme::LEADING_BODY)
                            .font(theme::MEDIUM)
                            .color(room.paper)
                            .wrapping(text::Wrapping::None),
                    ]
                    .spacing(theme::GAP_SM)
                    .align_y(iced::Alignment::Center),
                )
                // The ink lane, clipped: no glyph and no word may reach past
                // the veil's third stop, whatever the label says.
                .width(Length::Fixed(ink_lane))
                .height(Length::Fill)
                .align_y(alignment::Vertical::Center)
                .clip(true),
            )
            .width(Length::Fixed(band))
            .height(Length::Fill)
            .padding(iced::Padding::default().left(theme::VEIL_LEAD))
            .style(move |_theme, status| theme::veil_row(room, status))
            .on_press(press),
        );
    }
    container(options)
        .width(Length::Fixed(work))
        .height(Length::Fixed(work))
        .style(move |_theme| container::Style {
            background: Some(iced::Background::Gradient(theme::hover_veil(room).into())),
            ..container::Style::default()
        })
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

#[cfg(test)]
mod tests {
    use super::RULE_LANE_H;
    use crate::shelf::{Density, Grid};
    use crate::theme;

    /// **The Songs section sits on the same ruler as the wall it tops**
    /// (doc 09 §5; laws L2, L5, L7).
    ///
    /// Three claims, each the kind that drifts if unpinned:
    ///
    /// - **The lattice (L2)**: every lane a songs row reserves is a token
    ///   the list surfaces already share, on the unit of 4 — the section
    ///   adds no token of its own — and the row's box is `2 × GAP_XS +
    ///   STEPPER_HIT`, which is exactly [`theme::TRANSPORT_HIT`]: the
    ///   product's one control height (L7), by arithmetic rather than by a
    ///   new number.
    /// - **The ruler (L5)**: the section's block is `Grid::block_width` wide
    ///   and centred exactly as the wall's rows are, so its left edge *is*
    ///   the first column's left edge and the wall's permitted-edge list is
    ///   untouched. Pinned in the source, the way the alignment laws are,
    ///   because a hardcoded width here would pass every unit test and fail
    ///   the composition.
    /// - **The gutter**: the section's top air is [`theme::HANG`] — the
    ///   wall's own top-edge unit — and its rows carry no horizontal inset
    ///   of their own (the row's-own-padding defect L5 names).
    #[test]
    fn the_songs_section_sits_on_the_walls_own_ruler() {
        const { assert!(2.0 * theme::GAP_XS + theme::STEPPER_HIT == theme::TRANSPORT_HIT) }
        const { assert!(theme::TRACK_NO_W % 4.0 == 0.0) }
        const { assert!(theme::DURATION_W % 4.0 == 0.0) }
        const { assert!(theme::STEPPER_HIT % 4.0 == 0.0) }
        const { assert!(theme::HANG % 4.0 == 0.0) }

        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/views/shelf.rs"),
        )
        .expect("this module's own source")
        .replace("\r\n", "\n");
        // The call site hands the section the wall's own block width…
        assert!(
            source.contains("songs_section(shelf, player, collecting, hang.block_width())"),
            "the songs section is laid out at the wall's block width"
        );
        // …and the section spends it as its block, centred the wall's way.
        let section = source
            .split_once("fn songs_section")
            .expect("the songs section exists")
            .1;
        let section = &section[..section.find("\n}\n").expect("a function ends")];
        assert!(
            section.contains(".width(Length::Fixed(block))"),
            "the block is the width it was handed, not a width of its own"
        );
        assert!(
            section.contains("alignment::Horizontal::Center"),
            "centred exactly as the wall's rows are"
        );
        assert!(
            section.contains("top: theme::HANG"),
            "the section's top air is the wall's own unit"
        );
        // The row keeps the album page's no-inset rule: its one padding call
        // is vertical-only, on the token, with no x-inset of its own.
        let row = source
            .split_once("fn song_row")
            .expect("the songs row exists")
            .1;
        let row = &row[..row.find("\n}\n").expect("a function ends")];
        assert!(
            row.contains(".padding(theme::pad(theme::GAP_XS, 0.0))"),
            "a songs row hangs from the block's own edges (law L5)"
        );
    }

    /// **A density mark's press is the gesture's exact message** (ADR-0028;
    /// the mirror rule, doc 07 L8.7; the discipline of
    /// `every_menu_item_is_a_press_some_control_also_makes`).
    ///
    /// Two halves. The pure half: the delta a mark sends walks
    /// `Density::step` — the gesture's own function — onto the pressed step,
    /// for every pair, so the marks and the zoom cannot drift apart. The
    /// wiring half, source-pinned the way the songs-section test pins its
    /// geometry: the one press in `density_mark` is
    /// `Message::DensityStep(current.steps_to(step))` — no `DensitySet`, no
    /// second grammar — and the active mark takes the inert branch, because
    /// a control that does nothing when pressed is the lie the rail's
    /// absent letters already refuse.
    ///
    /// The mark itself moved to `views/mod.rs` when the fourth step made the
    /// control resident on three places rather than one, so that is the
    /// source this reads. It stays *this* test because the property it pins
    /// is the wall's: the marks and the wall's own zoom are one grammar.
    #[test]
    fn the_density_marks_mirror_the_gestures_exact_messages() {
        for current in Density::ALL {
            for target in Density::ALL {
                assert_eq!(
                    current.step(current.steps_to(target)),
                    target,
                    "{} mark pressed at {}",
                    target.label(),
                    current.label()
                );
            }
        }
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/views/mod.rs"),
        )
        .expect("the shared control's source")
        .replace("\r\n", "\n");
        // `fn density_mark(` — the trailing paren matters, because
        // `density_marks` (the run) shares the prefix with `density_mark`
        // (the one detent), and this test is about the detent.
        let mark = source
            .split_once("fn density_mark(")
            .expect("the density mark exists")
            .1;
        let mark = &mark[..mark.find("\n}\n").expect("a function ends")];
        assert!(
            mark.contains(".on_press(Message::DensityStep(current.steps_to(step)))"),
            "a mark's press is the gesture's message with the mirror delta"
        );
        assert_eq!(
            mark.matches(".on_press").count(),
            1,
            "one press, one message: the marks add no second grammar"
        );
        assert!(
            mark.contains("if active {\n        container(mark)"),
            "the active mark is inert — the fact, not a control"
        );
        // And every mark carries its name: the icon-only law's tooltip
        // clause (the sweep in `theme` walks this function too; this pins
        // the name being the step's own word).
        assert!(
            mark.contains("text(step.label())"),
            "the tooltip is the step's name"
        );
    }

    /// **The density marks left the lane, and left nothing behind** —
    /// ADR-0040 §5, on the owner's *"and please put the display options at
    /// the top bar"*.
    ///
    /// The placement half of ADR-0028 used to be asserted here, against the
    /// lane's own numbers. It is asserted in `views::app_bar` now, against the
    /// bar's. What this test keeps is the half that is still the wall's, and
    /// it is the half that would actually break something: **the rail's lane
    /// is the spine and nothing else**, and the marks are not drawn twice.
    ///
    /// Doc 07 L8.6 is the rule behind the second clause — *"no two controls
    /// may send the same message"* — and the way a move like this goes wrong
    /// is that the new home lands and the old one is left standing, so both
    /// are pinned rather than only the new one.
    #[test]
    fn the_rails_lane_is_the_spine_and_the_marks_are_not_drawn_twice() {
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/views/shelf.rs"),
        )
        .expect("this module's own source")
        .replace("\r\n", "\n");
        let code = source
            .split("#[cfg(test)]")
            .next()
            .expect("a source has a head");
        assert!(
            !code.contains("density_marks") && !code.contains("density_control"),
            "the wall is drawing the display options again — they are the app \
             bar's, and two controls sending `DensityStep` is L8.6's defect"
        );
        let lane = code
            .split_once("fn index_rail")
            .expect("the index rail exists")
            .1;
        let lane = &lane[..lane.find("\n}\n").expect("a function ends")];
        assert!(
            lane.contains("Spine::new(")
                && lane.contains(".width(Length::Fixed(theme::INDEX_LANE_W))"),
            "the lane is no longer the spine at the lane's declared width"
        );
        // …and there is exactly one run of marks in the product, in the bar.
        let bar = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/views/app_bar.rs"),
        )
        .expect("the app bar's source")
        .replace("\r\n", "\n");
        assert!(
            bar.split("#[cfg(test)]")
                .next()
                .expect("a head")
                .contains("crate::views::density_marks(current)"),
            "the app bar does not draw the marks it took"
        );
    }

    /// **The rail is the layer under the body, and the bar owns the window's
    /// edge** — the arrangement the owner's *"scroll bar is in a strange
    /// location… it seems to have padding on the right"* asked for.
    ///
    /// Three things have to stay true together, and each of them is a way of
    /// getting it wrong that a plausible edit would reintroduce:
    ///
    /// 1. **The wall asks for [`theme::shelf_scrollbar`]**, not the bar every
    ///    other list uses. The difference is the whole fix: it reserves the
    ///    rail's lane as well as its own, so the scrollable can span to the
    ///    window's edge and iced draws the bar there.
    /// 2. **The rail is the `stack`'s *first* child.** iced hands the topmost
    ///    layer the pointer first, so a rail pushed after the body would own
    ///    the 4 px the bar is drawn in and the bar would be ungrabbable —
    ///    which looks exactly like a bar that is merely decorative.
    /// 3. **The Songs section reserves the same two lanes**, because it is the
    ///    scrollable's sibling rather than its child and nothing reserves them
    ///    on its behalf.
    ///
    /// Read off the source, the way the density marks' placement is: what is
    /// being pinned is the *composition*, and the composition is the code.
    #[test]
    fn the_rail_hangs_under_the_body_and_the_bar_takes_the_window_edge() {
        // The bar's lane and the rail's, one number, and it is what the
        // scrollable reserves (`theme::shelf_scrollbar`'s `spacing`).
        const { assert!(theme::WALL_RESERVE == theme::INDEX_LANE_W + theme::WALL_SCROLLBAR_W) }

        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/views/shelf.rs"),
        )
        .expect("this module's own source")
        .replace("\r\n", "\n");
        let view = source
            .split_once("pub(crate) fn view<'a>(")
            .expect("the wall's view exists")
            .1;
        let view = &view[..view.find("\n}\n").expect("a function ends")];

        assert!(
            view.contains("theme::shelf_scrollbar()"),
            "the wall's bar no longer reserves the rail's lane, so it is not \
             on the window's edge"
        );
        // The stack's first child is the rail; the body is pushed over it.
        let stack = view
            .split_once("stack![\n        container(index_rail(")
            .map(|(_, rest)| rest)
            .expect("the rail is the layer under the body");
        let stack = &stack[..stack.find("\n    ]").expect("the stack ends")];
        assert!(
            stack.contains("alignment::Horizontal::Right"),
            "the rail's layer no longer hangs on the wall's right"
        );
        assert!(
            stack.trim_end().ends_with("body"),
            "the body is no longer the layer over the rail — the bar is under \
             the rail and cannot be grabbed"
        );

        let songs = source
            .split_once("fn songs_section<'a>(")
            .expect("the Songs section exists")
            .1;
        let songs = &songs[..songs.find("\n}\n").expect("a function ends")];
        assert!(
            songs.contains("right: theme::WALL_RESERVE"),
            "the Songs section centres on a different axis than the wall"
        );
    }

    /// **A tile's box holds the tile, at every density and every width.**
    ///
    /// The one number this file computes rather than receives — the hit box's
    /// height — has to be at least the work, the gap to the label, the label
    /// block, and the rule's own lane. It was written as
    /// `row_h − theme::HANG + RULE_LANE_H`, which is right at the default and
    /// **12 px short at `Dense`**, where it clipped the artist line off every
    /// tile on the wall while leaving the title. The defect was found on a
    /// screenshot; this is what would have found it first.
    #[test]
    fn a_tiles_box_holds_its_work_and_its_whole_label() {
        for density in Density::ALL {
            #[expect(
                clippy::cast_precision_loss,
                reason = "a grid width in pixels is far below f32's exact-integer range"
            )]
            for width in (300..=2560).map(|width| width as f32) {
                let grid = Grid::new(width, density);
                let box_h = grid.row_h - grid.hang + RULE_LANE_H;
                let content = grid.art
                    + theme::GAP_LG
                    + theme::CAPTION_H
                    + theme::GAP_XS
                    + theme::SELECTION_EDGE;
                assert!(
                    box_h >= content - 0.01,
                    "{} at {width} px: a {box_h} px box around {content} px of tile — \
                     the label clips",
                    density.label()
                );
            }
            // …and it is not *larger* than the row, or the hit areas of two
            // rows would meet and the wall would be one contiguous target.
            let grid = Grid::new(1172.0, density);
            assert!(grid.row_h - grid.hang + RULE_LANE_H < grid.row_h);
        }
    }
}

#[cfg(test)]
mod hover_option_tests {
    use crate::theme;

    /// This module's own source, for the pins below.
    fn source() -> String {
        std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/views/shelf.rs"),
        )
        .expect("this module's own source")
        .replace("\r\n", "\n")
    }

    /// The body of `fn name` in this module.
    fn function(source: &str, name: &str) -> String {
        let rest = source
            .split_once(&format!("fn {name}"))
            .unwrap_or_else(|| panic!("`{name}` exists"))
            .1;
        rest[..rest.find("\n}\n").expect("a function ends")].to_owned()
    }

    /// **Every option is a press some visible control already makes**, and the
    /// tile's right-press menu is the pointer-reachable twin of all four
    /// (the product's standing rule, Accessibility: *no control's only affordance is
    /// hover*). The reveal is an accelerator over routes that exist; it is
    /// never the only way to any of them.
    ///
    /// | Option | Message | Its twin |
    /// |---|---|---|
    /// | `Play` | `PlayAlbum` | the record page's `Play album`, and the menu's |
    /// | `Queue` | `QueueAlbum` | shift-click a sleeve, and the menu's `Queue album` |
    /// | `Add to…` | `AddAlbumToPlaylist` | the record page's `Add to playlist…`, and the menu's |
    /// | `Open` | `AlbumClicked` | the tile's own press, and the menu's `Open` |
    #[test]
    fn every_option_is_a_press_some_visible_control_already_makes() {
        let source = source();
        let options = function(&source, "hover_options<'a>");
        for (label, press) in [
            ("Play", "Message::PlayAlbum(album)"),
            ("Queue", "Message::QueueAlbum(album)"),
            ("Add to…", "Message::AddAlbumToPlaylist(album)"),
            ("Open", "Message::AlbumClicked(album)"),
        ] {
            assert!(
                options.contains(&format!("\"{label}\"")),
                "the option `{label}` is gone from the veil"
            );
            assert!(
                options.contains(press),
                "the option `{label}` no longer sends `{press}`"
            );
        }
        // Four options and four presses: no fifth verb slipped onto a sleeve.
        assert_eq!(theme::VEIL_OPTIONS, 4);
        // **One press arm for every veil in the product.** The layer is drawn
        // by [`veil`] now — Home's `All songs` tile wears the wall's own rather
        // than a second one that looks like it — so the arm is asserted where
        // it lives, and the list above is asserted where the *record's* options
        // are decided.
        assert_eq!(
            function(&source, "veil<'a>").matches(".on_press(").count(),
            1,
            "the options are pressed through one arm, once"
        );
    }

    /// **An option's press never also opens the page.**
    ///
    /// The mechanism is iced's, and it is the same one `album_door` inside a
    /// songs row relies on: `button::on_event` hands the event to its content
    /// *first* and returns `Captured` without reaching its own `on_press` if a
    /// child took it (`iced_widget-0.13.4/src/button.rs:283–295`). So the
    /// option buttons must be **inside** the tile's button, and the layer they
    /// live in must be the topmost of the `stack` — `stack::on_event` iterates
    /// `.rev()` (`iced_widget-0.13.4/src/stack.rs:222–226`).
    ///
    /// Both facts are pinned here, because the routing is a property of where
    /// the widgets sit rather than of anything either function says.
    #[test]
    fn an_options_press_is_captured_before_the_tile_sees_it() {
        let source = source();
        let tile = function(&source, "tile<'a>");
        assert!(
            tile.contains("stack![art, hover_options("),
            "the options are not the topmost layer over the work — a sleeve \
             press would reach them before they reached it, or not at all"
        );
        // The stack is inside the button, not around it: the `on_press` that
        // opens the page comes after the column that holds the sleeve.
        let sleeve_at = tile.find("let sleeve = container(").expect("the sleeve");
        let stack_at = tile.find("stack![art, hover_options(").expect("the layer");
        let press_at = tile
            .find(".on_press(Message::AlbumClicked(album.id))")
            .expect("the tile's own press");
        assert!(
            stack_at < sleeve_at && sleeve_at < press_at,
            "the options must be built into the work the tile's button holds"
        );
        // And the tile still opens the page from anywhere else on it.
        assert!(
            tile.contains(".on_press(Message::AlbumClicked(album.id))"),
            "pressing the sleeve outside an option no longer opens the record"
        );
    }

    /// **The veil is a gradient. A flat panel is not the design.**
    ///
    /// the product's *no scrim, ever* was written against dimming ten
    /// thousand covers to show twelve rows, and the owner's reversal is
    /// specifically of a **gradient over one sleeve under the pointer** — so
    /// the shape of the mark is the decision, not an implementation detail. A
    /// `Background::Color` here would be the refused thing wearing the
    /// permitted thing's name.
    #[test]
    fn the_veil_is_a_gradient_over_one_sleeve_and_never_a_flat_panel() {
        let options = function(&source(), "veil<'a>");
        assert!(
            options.contains("Background::Gradient(theme::hover_veil(room)"),
            "the veil stopped being the design's gradient"
        );
        assert!(
            !options.contains("Background::Color"),
            "a flat scrim rectangle is drawn over the sleeve"
        );
        // The gradient runs left to right and dies before the right edge.
        const { assert!(theme::VEIL_SPEC[0].0 == 0.0) }
        const { assert!(theme::VEIL_SPEC[0].1 > 0.9) }
        const { assert!(theme::VEIL_SPEC[theme::VEIL_SPEC.len() - 1].0 >= 1.0) }
        const {
            assert!(
                theme::VEIL_SPEC[theme::VEIL_SPEC.len() - 1].1 == 0.0,
                "the veil no longer dissolves before the sleeve's right edge"
            );
        }
    }

    /// **The reveal costs nothing when nothing is hovered.**
    ///
    /// A boolean, exactly as the `+` slot's reveal is (`song_row`'s
    /// `offered`) — not a tween, not a clock, not a subscription. The tile's
    /// 90 ms rule tween (ADR-0020 §2.3) is untouched and stays the only thing
    /// on the wall that moves; nothing here reads it, and nothing here asks
    /// for a frame.
    #[test]
    fn the_reveal_is_a_boolean_and_asks_for_no_frames() {
        let source = source();
        let tile = function(&source, "tile<'a>");
        assert!(
            tile.contains("if shelf.hovered_album == Some(album.id)"),
            "the reveal is no longer the `+` slot's own boolean"
        );
        let options = function(&source, "hover_options<'a>");
        for forbidden in ["tile_hover", "Tween", "motion::", "Instant"] {
            assert!(
                !options.contains(forbidden),
                "the options reach for `{forbidden}` — the reveal is a hover \
                 state, and a sixth tween is not the door it came in through"
            );
        }
    }

    /// **Options are the wall's alone.** Not on the Songs section's rows, not
    /// in the lane — a row plays and a tile navigates (doc 09 §5), and a verb
    /// group laid over a one-line row would be neither.
    #[test]
    fn only_a_wall_tile_carries_the_options() {
        let source = source();
        for surface in ["song_row<'a>", "songs_section<'a>", "header_line"] {
            assert!(
                !function(&source, surface).contains("hover_options"),
                "`{surface}` grew the wall's hover options"
            );
        }
        // Only the drawing half of the file — the tests below name it too.
        let drawn = source
            .split_once("#[cfg(test)]")
            .map_or(source.as_str(), |(drawn, _)| drawn);
        assert_eq!(
            drawn.matches("hover_options(").count(),
            1,
            "the options are built in exactly one place"
        );
    }
}
