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
use crate::rail::{RailEntry, RailSlot};
use crate::shelf::{Grid, Run, Shelves};
use crate::views::gradient_block;
use crate::{rail, theme, vm};

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
        grid = grid.push(header_band(shelf, *run, hang.block_width()));
        drawn = run.rows_top();
        let (first_row, end_row) = hang.visible_rows(
            shelf.scroll_offset - run.rows_top(),
            shelf.grid_size.height,
            run.rows,
        );
        grid = spacer(
            grid,
            run.rows_top() + hang.spacer_height(first_row),
            &mut drawn,
        );
        for r in first_row..end_row {
            grid = grid.push(shelf_row(shelf, player, hang, *run, r, lamp));
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
    // **The wall's own scrollbar.** This `scrollable` carried no style at all,
    // so the one down the side of the collection — the longest, most-seen piece
    // of chrome in the product — was iced's stock light-grey bar with a rail
    // behind it, drawn in no palette baz owns
    // (`docs/design/05-toolkit-and-visual-gap.md` D6). `theme::scrollbar` has
    // existed the whole time, draws no rail, and its own doc comment names this
    // exact failure.
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
    row![
        stack![wall, pinned_header(shelf, pinned, hang.block_width())],
        index_rail(shelf, &shelves)
    ]
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
                hang,
                album,
                player.playing_album() == Some(album.id),
                lamp,
                dimmed,
                ringed,
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
fn header_band(shelf: &Shelf, run: Run, block: f32) -> Element<'_, Message> {
    container(header_line(shelf, run, block))
        .width(Length::Fixed(block))
        .height(Length::Fixed(theme::SHELF_HEADER_H))
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
fn pinned_header(shelf: &Shelf, run: Option<Run>, block: f32) -> Element<'_, Message> {
    let room = theme::active();
    let body: Element<'_, Message> = match run {
        Some(run) => container(header_line(shelf, run, block))
            .width(Length::Fixed(block))
            .height(Length::Fixed(theme::SHELF_HEADER_H))
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
            theme::SHELF_HEADER_H
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
/// band, so the air above the ink is the previous row's trailing `HANG` and
/// the air below is `HANG − HEADING_LINE_H`. See
/// [`theme::SHELF_HEADER_H`] for the ratio and why it is that way round.
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
/// [`theme::GAP_LG`] from the wall's — the top bar's own gutter, which is the
/// alignment edge the `Settings` word above already established. An entry
/// wider than [`theme::INDEX_W`] grows *leftwards* to that cap and then clips,
/// which is why the lane keeps [`theme::INDEX_CLEARANCE`] between itself and
/// the scrollbar. The full value is never lost: it is set in the shelf header
/// one `HANG` to the left, at the same moment, in the same voice.
fn index_rail<'a>(shelf: &'a Shelf, shelves: &Shelves) -> Element<'a, Message> {
    let room = theme::active();
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
    let here = shelves.run_at(shelf.scroll_offset);
    let focus = here.and_then(|run| entries.iter().position(|entry| entry.shelf == Some(run)));
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a slot count floored from a non-negative viewport height"
    )]
    let capacity = (shelf.grid_size.height.max(0.0) / theme::RAIL_PITCH).floor() as usize;

    // `Fill`, so that aligning the entries right aligns them to the *lane*
    // rather than to the widest of them: a column of single letters would
    // otherwise sit wherever the longest entry happened to put it.
    let mut lane = column![]
        .width(Length::Fill)
        .spacing(theme::GAP_XS)
        .align_x(alignment::Horizontal::Right);
    for slot in rail::elide(&entries, capacity, focus) {
        lane = lane.push(match slot {
            RailSlot::Gap => rail_text(rail::GAP_MARK.to_owned(), room.paper_muted, false),
            RailSlot::Entry(index) => match entries.get(index) {
                Some(entry) => rail_entry(entry, entry.shelf == here),
                None => rail_text(String::new(), room.paper_muted, false),
            },
        });
    }
    container(lane)
        .width(Length::Fixed(theme::INDEX_LANE_W))
        .height(Length::Fill)
        .padding(iced::Padding {
            top: 0.0,
            // The one window gutter (law L1): the rail's right edge is the same
            // x as `Settings` above it and as the last column of covers beside
            // it. It was `GAP_LG`, the old chrome gutter.
            right: theme::HANG,
            bottom: 0.0,
            left: theme::INDEX_CLEARANCE,
        })
        .align_y(alignment::Vertical::Center)
        .into()
}

/// One value in the rail: a jump when the collection has it, a statement of a
/// gap when it does not.
fn rail_entry(entry: &RailEntry, current: bool) -> Element<'static, Message> {
    let room = theme::active();
    if !entry.present() {
        return rail_text(entry.label.clone(), room.paper_muted, false);
    }
    let Some(target) = entry.shelf else {
        return rail_text(entry.label.clone(), room.paper_muted, false);
    };
    let ink = if current {
        room.paper
    } else {
        room.paper_faint
    };
    button(rail_text(entry.label.clone(), ink, current))
        .padding(0)
        .style(move |_theme, status| theme::group_key(room, room.wall, status, current))
        .on_press(Message::RailJumped(target))
        .into()
}

/// One line of rail type: shrink-to-fit up to [`theme::INDEX_W`], clipped
/// there, right-aligned by the lane that holds it.
///
/// Shrink rather than a fixed box, so a single letter sits flush against the
/// rail's right edge instead of floating at the left of a 36 px lane, while a
/// long value still fills the lane and clips at its own right edge — the head
/// of the word survives, which is the half you navigate by.
fn rail_text(label: String, ink: iced::Color, current: bool) -> Element<'static, Message> {
    container(
        text(label)
            .size(theme::SIZE_HEADING)
            .line_height(theme::LEADING_HEADING)
            .font(if current { theme::MEDIUM } else { theme::SANS })
            .color(ink)
            .wrapping(text::Wrapping::None),
    )
    .max_width(theme::INDEX_W)
    .height(Length::Fixed(theme::RAIL_LINE_H))
    .clip(true)
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
fn tile<'a>(
    shelf: &'a Shelf,
    hang: Grid,
    album: &'a vm::AlbumVm,
    playing: bool,
    lamp: f32,
    dimmed: bool,
    ringed: bool,
) -> Element<'a, Message> {
    let room = theme::active();
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
    // **Selected, full stop** — not "selected and the inspector happens to be
    // showing it". The audit's defect 14: with `Ctrl+B` the column hides and the
    // selected tile's 2 px rule vanished with it, so the wall carried no mark at
    // all for a selection that `Enter` would still play. The rule is drawn from
    // the selection because that is what it is a mark of; whether a panel is
    // open beside it is a fact about the panel.
    let selected = shelf.selection.selected() == Some(album.id);
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
    mouse_area(
        button(
            column![sleeve, label_block]
                .spacing(theme::GAP_LG)
                .width(Length::Fixed(edge)),
        )
        .width(Length::Fixed(edge))
        // The work, its label and the label's rule — not the row. The row's
        // remaining `HANG` is the gap to the row below, and a hit area that
        // swallowed it would make the whole wall one contiguous target with no
        // space between the works. `RULE_LANE_H` of that gap is spent on the
        // state rule, which is part of the label rather than part of the gap;
        // what is left between two works is still more than three quarters of a
        // `HANG`.
        .height(Length::Fixed(hang.row_h - theme::HANG + RULE_LANE_H))
        .padding(0)
        .style(move |_theme, status| theme::tile(room, status, selected))
        .on_press(Message::AlbumClicked(album.id)),
    )
    .on_enter(Message::TileEntered(album.id))
    .on_exit(Message::TileLeft(album.id))
    .into()
}

/// What the state rule costs the tile vertically: the gap under the label plus
/// the lane the rule is drawn in (logical px).
///
/// Taken out of the row's trailing `HANG` rather than out of the work or the
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
