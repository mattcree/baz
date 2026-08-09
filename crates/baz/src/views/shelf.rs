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
use crate::shelf::{Density, Grid, Run, Shelves};
use crate::spine::{Slot, Spine};
use crate::views::album::add_slot;
use crate::views::{gradient_block, section_rule};
use crate::{icon, rail, theme, vm};

/// **The wall**: the shelved, virtualized grid, its pinned group header, and
/// the index rail down its right-hand side.
///
/// Three things sit side by side and on top of each other here, and the
/// arrangement is the whole of step 8's composition:
///
/// ```text
/// ┌──────────────────────────────────────────┬──────┐
/// │ scrollable( shelf headers + rows )        │ rail │  the rail is a sibling
/// │ ────────────────────────────────────────  │      │  of the wall, so the
/// │ ↑ the pinned header, stacked over the top  │      │  grid's width is what
/// └──────────────────────────────────────────┴──────┘  the scrollable measures
/// ```
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
    // **The wall's scrollbar** ([`theme::wall_scrollbar`]): 4 px, in the room's
    // hairline, reserving its own lane inside the scrollable so no cover is
    // ever drawn under it. The rail beside it still says *where you are* and
    // still names the shelf it jumps to; what the bar adds is the one gesture
    // the rail has no answer to — drag to the end. The owner's decision,
    // 2026-08-09; `docs/REFUSALS.md`'s two-vertical-strips entry records it.
    .direction(scrollable::Direction::Vertical(theme::wall_scrollbar()))
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
        .into()
    };
    row![body, index_rail(shelf, &shelves)].into()
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
        Some((id, row)) => add_slot(id, row, offered),
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
        let Some(&album_index) = shelf.visible.get(run.first + offset) else {
            break;
        };
        if let Some(album) = shelf.albums.get(album_index) {
            // **The two marks the pool makes**, decided here rather than in the
            // tile so that a wall with no shuffle running asks the pool nothing
            // at all: `None` is the ordinary state and it costs one branch per
            // row, not one per cover.
            let (dimmed, ringed) = shelf.pool.as_ref().map_or((false, false), |pool| {
                (
                    !pool.holds(album.id),
                    pool.ringed(album.id, player.playing_album()),
                )
            });
            cells = cells.push(tile(
                shelf,
                player,
                hang,
                album,
                lamp,
                (dimmed, ringed),
                collecting,
            ));
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
/// around it zoomed would put the pinned header a few pixels out at two of the
/// three steps, which is the one place in the wall a few pixels are visible.
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
/// it needs no transition (`docs/REFUSALS.md`: *no motion — hard cuts by
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
fn header_line(shelf: &Shelf, run: Run, block: f32) -> Element<'_, Message> {
    let room = theme::active();
    let label = shelf
        .groups
        .get(run.group)
        .map_or_else(String::new, |group| group.header.label());
    container(
        text(theme::tracked(&label.to_uppercase()))
            .size(theme::SIZE_HEADING)
            .line_height(theme::LEADING_HEADING)
            .font(theme::MEDIUM)
            .color(room.paper_faint)
            .wrapping(text::Wrapping::None),
    )
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
/// `docs/REFUSALS.md` guards against from the other side.
///
/// # Where its edges are
///
/// Entries are right-aligned to the lane, so the rail's right edge is
/// [`theme::HANG`] from the window's — the one window gutter (law L1), which
/// is the alignment edge the `Settings` word above already established. An
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
/// # The lane's foot carries the density detents
///
/// Below the spine's strip, the three density marks ([`density_control`],
/// ADR-0028) close the lane. The spine's height is what the marks leave it,
/// and its per-frame elision absorbs the shorter lane exactly as it absorbs
/// a short window — the fisheye never sees the marks because they are
/// outside its bounds, and the lane's *width* is [`theme::INDEX_LANE_W`] at
/// every step, so the wall beside it cannot reflow by a pixel.
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
    column![
        Spine::new(slots, current, theme::active(), Message::RailJumped),
        density_control(shelf.grid().density),
    ]
    .width(Length::Fixed(theme::INDEX_LANE_W))
    .into()
}

/// The lane inset that stands a mark's ink on the lane's own edge: the
/// sprite is centred in its [`theme::STEPPER_HIT`] box, so the box overhangs
/// the window gutter by this much and the sprite's right edge lands on
/// `W − HANG` — the same line the rail's letters hang from. The wall's
/// permitted-edge list (law L5) gains nothing.
const MARK_INSET: f32 = (theme::STEPPER_HIT - theme::ICON_PX) / 2.0;

/// **The density detents** (ADR-0028, doc 11 §5 P8 — the owner's choice):
/// three marks at the foot of the index rail's lane, one per
/// [`Density::ALL`] step, loosest at the top — the direction
/// <kbd>Ctrl</kbd>+<kbd>=</kbd> walks.
///
/// # Why here
///
/// Density reads *the viewport, and nothing else* (doc 07 L8.1), so its home
/// is the place's body — and the lane is the body's one resident
/// view-subject strip, already reading the arrangement and the viewport. The
/// wall's own leading band was the other candidate and fails three ways: it
/// scrolls away, the pinned header claims it the moment the wall moves, and
/// its height is the step's hang, so a control there would resize itself as
/// its own effect. The lane's width is constant at every step and window;
/// nothing about the grid's algebra changes.
///
/// # What each mark is
///
/// A [`theme::STEPPER_HIT`] box (law L7's named secondary) holding the
/// step's sprite — the wall itself at that hang: one work, four, nine. The
/// current step is the full-ink mark ([`theme::GLYPH_OPACITY_HOVER`] against
/// the others' [`theme::GLYPH_OPACITY`]) — the group-key row's active
/// treatment translated to sprite ink, and **never the accent**: density is
/// not playback truth. The wall is the primary readout — the covers' own
/// size states the step — so the lift confirms rather than carries.
///
/// # The press is the gesture's own message
///
/// A mark sends [`Message::DensityStep`] with [`Density::steps_to`]'s delta
/// — the exact signed notch count the <kbd>Ctrl</kbd>+scroll /
/// <kbd>Ctrl</kbd>+<kbd>±</kbd> gesture would spend, making keys and wheel
/// *accelerators of a visible control* rather than the control itself
/// (the mirror rule, doc 07 L8.7; `docs/REFUSALS.md` as amended by
/// ADR-0028). The **active mark is inert** — pressing the step you are on
/// would do nothing, and a control that does nothing when pressed is the lie
/// the rail's absent letters already refuse. It is the fact; the other two
/// are the controls (L8.3's split).
fn density_control(current: Density) -> Element<'static, Message> {
    let mut marks = column![];
    for step in Density::ALL {
        marks = marks.push(density_mark(step, current));
    }
    container(marks)
        .width(Length::Fixed(theme::INDEX_LANE_W))
        .align_x(alignment::Horizontal::Right)
        .padding(iced::Padding {
            // The sprite's ink on `W − HANG`, the lane's one declared edge.
            right: theme::HANG - MARK_INSET,
            // One hang of air above the bar — the wall's own trailing unit,
            // and the room's gutter does not zoom (law L1).
            bottom: theme::HANG,
            ..iced::Padding::ZERO
        })
        .into()
}

/// One detent of [`density_control`]: the step's glyph in a
/// [`theme::STEPPER_HIT`] box, named by its tooltip (the icon-only law,
/// doc 10 §3.1 — the tooltip is the accessible name in a toolkit with no
/// accessibility tree), the hover wash [`theme::transport`]'s — the lane's
/// established press vocabulary, the same family as the spine's winner chip.
fn density_mark(step: Density, current: Density) -> Element<'static, Message> {
    let room = theme::active();
    let active = step == current;
    let glyph = match step {
        Density::Spacious => icon::Glyph::DensitySpacious,
        Density::Balanced => icon::Glyph::DensityBalanced,
        Density::Dense => icon::Glyph::DensityDense,
    };
    let mark = container(
        iced_image(icon::handle(glyph))
            .width(Length::Fixed(theme::ICON_PX))
            .height(Length::Fixed(theme::ICON_PX))
            .opacity(if active {
                theme::GLYPH_OPACITY_HOVER
            } else {
                theme::GLYPH_OPACITY
            }),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(alignment::Horizontal::Center)
    .align_y(alignment::Vertical::Center);
    // The active mark is the fact and takes no press; the other two are the
    // controls and send the gesture's exact message (function docs).
    let boxed: Element<'static, Message> = if active {
        container(mark)
            .width(Length::Fixed(theme::STEPPER_HIT))
            .height(Length::Fixed(theme::STEPPER_HIT))
            .into()
    } else {
        button(mark)
            .width(Length::Fixed(theme::STEPPER_HIT))
            .height(Length::Fixed(theme::STEPPER_HIT))
            .padding(0)
            .style(move |_theme, status| theme::transport(room, room.wall, status))
            .on_press(Message::DensityStep(current.steps_to(step)))
            .into()
    };
    iced::widget::tooltip(
        boxed,
        text(step.label())
            .size(theme::SIZE_CAPTION)
            .line_height(theme::LEADING_CAPTION),
        // Leftwards: the marks stand on the window's right edge.
        iced::widget::tooltip::Position::Left,
    )
    .gap(theme::GAP_XS)
    .padding(theme::GAP_XS)
    .style(move |_theme| theme::tooltip(room))
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
    // The shuffle pool's two marks, as one argument: `(dimmed, ringed)`.
    pool: (bool, bool),
    collecting: Collecting,
) -> Element<'a, Message> {
    let room = theme::active();
    let (dimmed, ringed) = pool;
    let playing = player.playing_album() == Some(album.id);
    let engine = player.engine_ready();
    let edge = hang.art;
    // **The work, inside its reserved ring lane.** Every sleeve on the wall is
    // drawn at the grid's art edge less two [`theme::POOL_RING`]s, in every
    // state, so that the ring a shuffle's next draw carries costs no geometry
    // and moves no cover when it arrives. See [`theme::POOL_RING`].
    let work = (edge - 2.0 * theme::POOL_RING).max(0.0);
    // Outside the pool of a shuffle that is running: the artwork itself is
    // composited at [`theme::POOL_DIM`], which is not a scrim and not a layer —
    // nothing is drawn on top of the sleeve.
    let shown = if dimmed { theme::POOL_DIM } else { 1.0 };
    let art: Element<'_, Message> = match shelf.thumbs.peek(&album.id) {
        Some(handle) => iced_image(handle.clone())
            .width(Length::Fixed(work))
            .height(Length::Fixed(work))
            .opacity(shown)
            .into(),
        None => gradient_block(album.id, work, shown),
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
    .padding(theme::POOL_RING)
    .style(move |_theme| theme::pool_ring(room, ringed));
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
fn state_rule(hovered: f32, selected: bool, edge: f32) -> Element<'static, Message> {
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
/// approved design, and the reversal of ADR-0032 §2 and of `docs/REFUSALS.md`'s
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
/// this layer is an accelerator and never the only route (`docs/REFUSALS.md`,
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
    let room = theme::active();
    let band = work * theme::VEIL_BAND_X;
    let ink_lane = (work * theme::VEIL_INK_X - theme::VEIL_LEAD).max(0.0);
    let listed: [(icon::Glyph, &'static str, Option<Message>, bool); theme::VEIL_OPTIONS] = [
        (
            icon::Glyph::Play,
            "Play",
            engine.then_some(Message::PlayAlbum(album)),
            true,
        ),
        (
            icon::Glyph::Queue,
            "Queue",
            engine.then_some(Message::QueueAlbum(album)),
            false,
        ),
        (
            icon::Glyph::Plus,
            "Add to…",
            collecting
                .available
                .then_some(Message::AddAlbumToPlaylist(album)),
            false,
        ),
        (
            icon::Glyph::Open,
            "Open",
            Some(Message::AlbumClicked(album)),
            false,
        ),
    ];
    let mut options = column![]
        .width(Length::Fixed(band))
        .height(Length::Fixed(work));
    for (glyph, label, press, accent) in listed {
        // Absent, not disabled: an option with no engine or no playlists
        // folder behind it is not offered, and the rows left divide the
        // sleeve between them.
        let Some(press) = press else {
            continue;
        };
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
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/views/shelf.rs"),
        )
        .expect("this module's own source")
        .replace("\r\n", "\n");
        let mark = source
            .split_once("fn density_mark")
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

    /// **The density marks stand in the lane's own geometry** (ADR-0028) —
    /// the placement half of the control, in the lane the owner's choice
    /// named, without disturbing an edge, a height or a width the laws
    /// already pin.
    #[test]
    fn the_density_marks_stand_in_the_lanes_own_geometry() {
        // Law L7: each mark is the named secondary square, and the band of
        // three sits on the lattice (law L2).
        const { assert!(super::MARK_INSET == (theme::STEPPER_HIT - theme::ICON_PX) / 2.0) }
        const { assert!((3.0 * theme::STEPPER_HIT) % 4.0 == 0.0) }
        // Law L5/L1: the box overhangs the gutter by exactly the sprite's
        // centring inset, so the ink lands on `W − HANG` — the lane's one
        // declared edge — and the inset padding is itself on the lattice.
        const { assert!((theme::HANG - super::MARK_INSET) % 4.0 == 0.0) }
        // The lane's width is untouched at every step: the marks live inside
        // `INDEX_LANE_W`, which is what keeps every wall-width test true
        // without a character changing.
        const { assert!(theme::STEPPER_HIT + (theme::HANG - super::MARK_INSET) < theme::INDEX_LANE_W) }

        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/views/shelf.rs"),
        )
        .expect("this module's own source")
        .replace("\r\n", "\n");
        // The lane is the spine over the marks, at the lane's width…
        let lane = source
            .split_once("fn index_rail")
            .expect("the index rail exists")
            .1;
        let lane = &lane[..lane.find("\n}\n").expect("a function ends")];
        assert!(
            lane.contains("density_control(shelf.grid().density)"),
            "the lane's foot carries the detents, fed by the grid that hung \
             the frame"
        );
        assert!(
            lane.contains(".width(Length::Fixed(theme::INDEX_LANE_W))"),
            "the lane keeps its declared width"
        );
        // …and the control spends the lane's own numbers: right-aligned onto
        // the ink edge, one un-zoomed hang of air above the bar, the steps
        // in `ALL`'s loosest-first order — the direction Ctrl+= walks.
        let control = source
            .split_once("fn density_control")
            .expect("the density control exists")
            .1;
        let control = &control[..control.find("\n}\n").expect("a function ends")];
        assert!(control.contains("right: theme::HANG - MARK_INSET"));
        assert!(control.contains("bottom: theme::HANG"));
        assert!(control.contains("for step in Density::ALL"));
        assert!(control.contains("alignment::Horizontal::Right"));
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
    /// (`docs/REFUSALS.md`, Accessibility: *no control's only affordance is
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
        let options = function(&source(), "hover_options<'a>");
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
        assert_eq!(
            options.matches(".on_press(").count(),
            1,
            "the options are pressed through one arm, once"
        );
        assert_eq!(theme::VEIL_OPTIONS, 4);
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
    /// `docs/REFUSALS.md`'s *no scrim, ever* was written against dimming ten
    /// thousand covers to show twelve rows, and the owner's reversal is
    /// specifically of a **gradient over one sleeve under the pointer** — so
    /// the shape of the mark is the decision, not an implementation detail. A
    /// `Background::Color` here would be the refused thing wearing the
    /// permitted thing's name.
    #[test]
    fn the_veil_is_a_gradient_over_one_sleeve_and_never_a_flat_panel() {
        let options = function(&source(), "hover_options<'a>");
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
