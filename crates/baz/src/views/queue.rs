//! **The run column**: what the engine is holding, and where it is in it.
//!
//! # It was a popover, then a rail panel, and is a place again for unsaved runs
//!
//! ADR-0016 moved the queue out of the right-hand rail and into a 360 px card
//! floating over the wall, anchored to the bar it describes. ADR-0022 removed
//! every side surface baz had — the owner's verdict on the pair was *"I really
//! hate the way queue and selected albums appear"* — so the queue became a
//! **place**, at the width of the window.
//!
//! The queue was then folded into Now playing while the two surfaces were being
//! reconciled. That made Now playing a second album/playlist page, which was
//! the wrong destination: it is now deliberately only the current song. The
//! owner subsequently gave one run a real reason to reopen this surface:
//! playing `All songs` materializes an **unsaved playlist**, and its source
//! road needs a location where that list can be inspected, edited and saved.
//! `Place::Queue` is that location. It has no resident navigation row and
//! reserves no space in Now playing; the source footer and bottom bar lead here
//! only while the current run is unsaved.
//!
//! Every run fact and gesture survives here — one list with a cursor, the
//! summary that reads *what is left*, click-to-jump, ✕, steppers, transfer `+`,
//! drag, `Save as playlist`, `Undo` and a virtual window. The presentation does
//! not: saved and unsaved lists now enter
//! [`playlist_page`], and each row wears the same artwork
//! and Album context as a saved entry rather than a private record-heading form.
//!
//! The bar's own third line still states the continuation ambiently
//! ([`crate::views::bottom_bar`]) — and earns its place harder than before,
//! since it is now the only statement about the run outside this surface.
//!
//! # One list with a cursor
//!
//! The model is `MusicBee`'s, adopted deliberately rather than arrived at:
//! **history behind the cursor, queue ahead, one surface**
//! (`docs/design/03-interface-prior-art.md` §5.3(3), R5). What follows from
//! naming it is the summary line: it reads **what is left** (`3 of 12 · 38:12
//! left`), not what the list contains. A queue is a thing you are partway
//! through, and the total running time answers a question nobody came here to
//! ask.
//!
//! # Marking
//!
//! The playing row is marked the way the shelf marks the playing album and the
//! way the record's page marks the playing track — the amber lamp dot, in place
//! of the row's number — because that is the one thing on this surface that
//! *is* playback truth, and the palette reserves the accent for exactly that.
//!
//! # Edit parity, and the virtual window
//!
//! Since doc 09 §13 step 5 the rows carry the playlist page's whole reserved
//! edit set — ▲▼ steppers, ✕, and the transfer `+`. Since the owner's
//! 2026-08-12 review, the claim that the queue place and playlist page are
//! **the same editor** is literal at page level too: both call one
//! parameterized playlist compositor. The file spends its slots on
//! Play/Rename/Delete and durable counts; the run spends them on its
//! cursor/remaining-time reading and `Save as playlist`.
//!
//! **That is the whole of the difference, and until 2026-08-10 the surface
//! never said so.** The owner: *"'save as playlist' really makes no sense on
//! the playlist page for a CD"* — he was reading this column, and the reading
//! was fair: the strip said `1 of 24 · 1:56:19 left`, a run reading with no
//! subject; the word beside it offered to save something; and 57 px below
//! stood the record's own title. So the strip now leads with a noun in both
//! branches (`Run · …`, or the list's name) and the word states what it is
//! saving — see [`save_control`] and ADR-0024 §A5. Those readings now sit in
//! the shared identity and acts slots rather than a private summary strip.
//!
//! And since `Play all`
//! (09 §7.1) can reify a whole library into this list, the rows are drawn
//! through the saved playlist's fixed-pitch row window — everything off screen
//! is two spacers, the wall's own discipline at list scale.

use std::borrow::Cow;

use iced::widget::{Space, button, container, mouse_area, row, text};
use iced::{Element, Length, alignment};

use crate::app::{Message, Shelf};
use crate::player::{PlayerState, QueueRow, QueueRowState, RunOrigin};
use crate::playlists::{Collecting, NameEntry};
use crate::selection::Content;
use crate::views::playlist_page::{self, PlaylistPage};
use crate::views::{page, place_name};
use crate::{icon, theme};

/// The `Save as playlist` field's id, so the caret can land in it the moment
/// the word becomes a field.
pub(crate) fn save_name_id() -> iced::widget::Id {
    iced::widget::Id::new("baz-queue-save")
}

/// The unsaved playlist as a full place: its standard header and the retained
/// run capabilities in the same page a saved list wears.
#[expect(
    clippy::too_many_arguments,
    reason = "the call site hands the shared page independent run, pointer and viewport readings"
)]
pub(crate) fn view<'a>(
    shelf: &'a Shelf,
    player: &'a PlayerState,
    window: iced::Size,
    hovered: Option<usize>,
    saving: Option<&'a NameEntry>,
    collecting: Collecting,
    scroll: f32,
    drag: Option<&'a crate::drag::DragState>,
    can_undo: bool,
) -> Element<'a, Message> {
    let name = unsaved_name(player.queue_origin());
    let art = unsaved_art(shelf, player);
    let queue = player.queue();
    let list = player.queue_list();
    let records = queue.map_or(0, |queue| {
        let mut groups: Vec<(Option<&str>, &str)> = Vec::new();
        for item in &queue.items {
            let group = (
                item.album.as_deref(),
                item.album_artist.as_deref().unwrap_or(&queue.artist),
            );
            if !groups.contains(&group) {
                groups.push(group);
            }
        }
        groups.len()
    });
    let byline = match records {
        1 => "Unsaved playlist · 1 record".to_owned(),
        records => format!("Unsaved playlist · {records} records"),
    };
    let facts = list
        .as_ref()
        .map_or_else(|| "0 tracks".to_owned(), |list| list.summary.clone());
    let layout = playlist_page::layout(window.width);
    playlist_page::view(
        shelf,
        PlaylistPage {
            lead: place_name(&name),
            name: name.clone(),
            art,
            commitment: None,
            acts: vec![save_control(saving.is_none(), player.run_origin())],
            identity: page::Identity {
                name,
                face: theme::SEMIBOLD,
                edit: saving.map(|entry| page::NameEdit {
                    value: &entry.text,
                    error: entry.error.as_deref(),
                    id: save_name_id(),
                    on_input: Message::SaveQueueInput,
                    on_submit: Message::SaveQueueSubmit,
                }),
                byline,
                facts,
                beside_facts: can_undo.then(|| undo_control(true)),
            },
            rows: queue_rows(
                shelf,
                player,
                list.as_ref(),
                window,
                scroll,
                layout,
                hovered,
                collecting,
                drag,
            ),
            on_scroll: Message::QueueScrolled,
        },
        window.width,
    )
}

/// The title of an unsaved list. An artist's implicit list is deliberately
/// named like the playlist it is about to become: `All Anne-Marie Puig`, not
/// the generic action label printed on the artist tile.
pub(crate) fn unsaved_name(origin: Option<&crate::origin::Origin>) -> String {
    match origin {
        Some(crate::origin::Origin::Artist { name, .. }) => format!("All {name}"),
        Some(crate::origin::Origin::AllSongs) => "All songs".to_owned(),
        Some(origin) => origin.name().to_owned(),
        None => "Queue".to_owned(),
    }
}

/// The first four distinct records represented by the queue, in queue order.
///
/// Queue rows already carry the record title and filed-under artist used by
/// the shelf, so resolving those pairs is both cheaper and more faithful than
/// walking every path in every edition on every frame. Four ids are the whole
/// supply [`crate::views::playlist_sleeve`] can spend.
pub(crate) fn unsaved_art(shelf: &Shelf, player: &PlayerState) -> Vec<u64> {
    let Some(queue) = player.queue() else {
        return Vec::new();
    };
    let mut art = Vec::new();
    for item in &queue.items {
        let filed_under = item.album_artist.as_deref().unwrap_or(&queue.artist);
        let Some(album) = shelf
            .albums
            .iter()
            .find(|album| album.title == item.album && album.artist.label() == filed_under)
        else {
            continue;
        };
        if !art.contains(&album.id) {
            art.push(album.id);
            if art.len() == 4 {
                break;
            }
        }
    }
    art
}

/// The visible slice of the unsaved list, through the same fixed-pitch window
/// and row presentation a saved playlist uses.
#[expect(
    clippy::too_many_arguments,
    reason = "the windowed list consumes independent playback, pointer and viewport readings"
)]
fn queue_rows<'a>(
    shelf: &'a Shelf,
    player: &'a PlayerState,
    list: Option<&crate::player::QueueList>,
    window: iced::Size,
    scroll: f32,
    layout: playlist_page::Layout,
    hovered: Option<usize>,
    collecting: Collecting,
    drag: Option<&'a crate::drag::DragState>,
) -> Vec<Element<'a, Message>> {
    let Some(list) = list else {
        return Vec::new();
    };
    let Some(queue) = player.queue() else {
        return Vec::new();
    };
    // A row is only a control when there is an engine to send its command to.
    let live = player.engine_ready();
    let win =
        super::playlist::row_window(list.rows.len(), layout.rows_scroll(scroll), window.height);
    let mut rows = vec![Space::new().height(win.top).into()];
    for index in win.first..win.end {
        let row_state = list.rows[index].clone();
        let item = &queue.items[index];
        rows.push(
            container(queue_row(
                shelf,
                item,
                &queue.artist,
                row_state,
                index,
                list.rows.len(),
                layout.side_by_side(),
                live,
                hovered == Some(index),
                collecting,
                drag.and_then(|held| held.line_for_row(crate::drag::List::Queue, index)),
                drag.is_some_and(|held| held.list == crate::drag::List::Queue),
                shelf.selection.is(Content::QueueTrack(index)),
            ))
            .height(Length::Fixed(super::playlist::ROW_PITCH))
            .align_y(alignment::Vertical::Top)
            .into(),
        );
    }
    rows.push(Space::new().height(win.bottom).into());
    rows
}

/// **Undo** — the run as it stood before the last edit, restored
/// (doc 11 §5 P2). Drawn only while there is an edit to take back: a
/// standing "Undo" over a list nobody has edited would be a control that
/// cannot act pretending it can. A word beside the live facts rather than a
/// toast, because forgiveness is a fact about the place, not an
/// interruption; quiet, because restoring a list is an act on the run, not
/// on playback — nothing sounds because of it.
fn undo_control(offered: bool) -> Element<'static, Message> {
    if !offered {
        return Space::new().width(Length::Fixed(0.0)).into();
    }
    let room = theme::active();
    button(
        container(
            text("Undo")
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .font(theme::MEDIUM)
                .wrapping(text::Wrapping::None),
        )
        .height(Length::Fill)
        .align_y(alignment::Vertical::Center),
    )
    .height(Length::Fixed(theme::TRANSPORT_HIT))
    .padding(theme::pad(0.0, theme::GAP_SM))
    .style(move |_theme, status| theme::word_button(room, room.wall, status))
    .on_press(Message::Undo)
    .into()
}

/// **Save as playlist** — the transient frozen into an artefact
/// (ADR-0024 §4): a labelled word in the shared playlist page's acts slot,
/// quiet because it is an act on a file rather than on playback.
///
/// Offered only while the name field is closed: the page title becomes that
/// field mid-gesture, and drawing both would be one act with two live buttons.
///
/// # …and only when it can usefully act (ADR-0024 §A5.2)
///
/// The owner, 2026-08-10: *"'save as playlist' really makes no sense on the
/// playlist page for a CD"*. He is reading this word, on the surface the
/// queue merged into, and he is right twice over: the strip beside it prints
/// the run's provenance, so the control offered to save a thing whose name
/// was already two inches to its left; and over a record's run it manufactured
/// a one-record playlist whose sleeve is that record's own cover, which then
/// landed in the lane above the record it came from wearing its face
/// (design 14 §0). It was conditioned on **nothing** but whether its own name
/// field was open.
///
/// # …and only for a run the listener assembled (the owner, 2026-08-10)
///
/// *"I still see save as playlist on the queue when playing a CD... we should
/// only be showing that in a situation where there isn't an existing
/// playlist"*, and, narrowing it the same afternoon, *"nah I think adding more
/// stuff to an existing playlist is fine, that does not need a save -- it's a
/// low bar to edit a playlist"*.
///
/// So it reads [`PlayerState::run_origin`], which now knows the **three kinds
/// of list** ([`crate::vm::RunSource`]) rather than only whether a file was
/// named, and takes the shape that reading permits:
///
/// | the run | the capability slot |
/// |---|---|
/// | [`RunOrigin::Fixed`] — a record's, `All songs`, `Play all` | **nothing**, in a reserved slot |
/// | [`RunOrigin::Saved`] — reified from a file, unedited | `Saved as “Road Trip”`, a **readout** |
/// | [`RunOrigin::Diverged`] — reified from a file, since edited | `From “Road Trip”`, a **readout** |
/// | [`RunOrigin::Assembled`] — built by hand, or a fixed run since edited | `Save as playlist`, live |
///
/// **The precedent is eleven lines up.** [`undo_control`] is drawn only while
/// there is an edit to take back, because *"a standing `Undo` over a list
/// nobody has edited would be a control that cannot act pretending it can"*.
/// This was that defect and this is that cure.
///
/// **`Diverged` says `From`, not `Saved as`, and not `Save as new playlist`.**
/// Two refusals in one word, and they pull opposite ways so both have to be
/// stated:
///
/// - it may not read `Saved as “Road Trip”`, because after an edit the run is
///   **not** that file — that is exactly the lie ADR-0024 §A5.2 removed, and
///   it must not come back through this door;
/// - it may not offer a new file either, because the owner's second quote
///   says a run that came from a playlist has a cheap route to changing one
///   and does not need a second.
///
/// So it names its origin and asserts nothing about its identity.
///
/// **Nothing here writes back**, and the narrowing did not change that.
/// *"A low bar to edit a playlist"* is an argument about how easy the
/// playlist's own page is to reach, not an instruction to make the run an
/// editor of files: ADR-0024 §1 decouples the two in **both** directions and
/// ADR-0023 §3 makes provenance an origin rather than a live link, so a queue
/// edit made for tonight still cannot touch a file somebody owns.
///
/// Every state is built at this control's own height and inset — including the
/// empty one, which is a `Space` of exactly [`theme::TRANSPORT_HIT`] rather
/// than an absence. It occupies the shared playlist page's acts slot, so
/// changing provenance never moves the sleeve, identity or rows.
fn save_control(offered: bool, origin: RunOrigin<'_>) -> Element<'static, Message> {
    let room = theme::active();
    let readout = |line: String| -> Element<'static, Message> {
        container(
            container(
                text(line)
                    .size(theme::SIZE_META)
                    .line_height(theme::LEADING_META)
                    .color(room.paper_faint)
                    .wrapping(text::Wrapping::None),
            )
            .height(Length::Fill)
            .align_y(alignment::Vertical::Center),
        )
        .height(Length::Fixed(theme::TRANSPORT_HIT))
        .padding(theme::pad(0.0, theme::GAP_SM))
        .into()
    };
    let label = match origin {
        RunOrigin::Assembled => "Save as playlist".to_owned(),
        // A list that already exists says nothing, and holds its height.
        RunOrigin::Fixed => {
            return Space::new()
                .height(Length::Fixed(theme::TRANSPORT_HIT))
                .into();
        }
        RunOrigin::Saved(name) => return readout(format!("Saved as “{name}”")),
        RunOrigin::Diverged(name) => return readout(format!("From “{name}”")),
    };
    button(
        container(
            text(label)
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .font(theme::MEDIUM)
                .wrapping(text::Wrapping::None),
        )
        .height(Length::Fill)
        .align_y(alignment::Vertical::Center),
    )
    .height(Length::Fixed(theme::TRANSPORT_HIT))
    .padding(theme::pad(0.0, theme::GAP_SM))
    .style(move |_theme, status| theme::word_button(room, room.wall, status))
    .on_press_maybe(offered.then_some(Message::SaveQueueStart))
    .into()
}

/// One queue row: position (or the lamp dot when it is playing), title over its
/// artist where there is one, right-aligned duration — and the full reserved
/// edit set a list in baz carries (doc 09 §8.2: **the queue place and the
/// playlist page are the same editor**).
///
/// **Clicking the row plays from there.** ADR-0014's `JumpTo`, and this list is
/// the one place it needs no decision at all: the rows are drawn from the
/// record of what was handed to the engine, so a row's index *is* a queue
/// position. Nothing is re-queued, and the mark follows `TrackStarted` rather
/// than the click.
///
/// **The ▲▼ steppers reorder** — the playlist page's exact slots
/// (`views/playlist.rs`), sending the whole edited list as `UpdateQueue`
/// through the pure [`queue_edit::shifted`](crate::queue_edit::shifted): the
/// music keeps playing (ADR-0014's guarantee), and the cursor follows its
/// track — the sounding row moves like any other, because the engine finds
/// it again by path.
///
/// **The ✕ takes the entry out**, through `UpdateQueue` and the pure
/// [`queue_edit`](crate::queue_edit) helper — an edit that does not touch the
/// playing track does not disturb one delivered sample, which is the guarantee
/// ADR-0014 exists to make and the reason this is not a `SetQueue`.
///
/// **The `+` is the transfer slot** (doc 09 §8.1): one press holds this row's
/// track and opens the panel as the picker — a destination list, the Queue's
/// own row first among them — so a run being auditioned can seed a kept list
/// row by row (S9a). It is on the sounding row too: the track you are hearing
/// is the one most worth keeping. Drawn at the row's outer edge, where the
/// album page's rows put the same slot; the ▲▼✕ keep the playlist page's
/// exact positions.
///
/// Two fixed-slot rules, because a list that changes under the reader is
/// exactly where movement is least affordable:
///
/// - the number column is [`theme::TRACK_NO_W`] wide whichever it holds, so the
///   dot arriving as a track starts moves no text;
/// - **every edit slot is reserved whether or not its control is in it.** The
///   controls appear on hover — a column of permanent crosses down a list of
///   what you are about to hear is a column of invitations to destroy
///   something — but if their width came and went with them, every duration
///   would slide sideways as the pointer crossed a row. (The `+` alone is
///   also drawn at rest while the panel stands, the album page's own rule:
///   the picker on screen is the task the mark belongs to.)
///
/// **The row's body is a drag source** (doc 09 §13 step 8; [`crate::drag`]
/// carries the gesture and its laws): press and move past the threshold to
/// lift the row — reorder against the insertion line, or carry it over the
/// standing panel's rows to add — while a sub-threshold press stays this
/// row's ordinary click. Sugar only: the ▲▼ steppers, the ✕ and the `+`
/// remain exactly as above. A sub-threshold first press selects and a second
/// matching press jumps. `line` is the insertion edge this row draws while a
/// drag is in flight; `observing` has the row measure the held
/// pointer against its own bounds — both from the shell's one drag state.
#[expect(
    clippy::too_many_arguments,
    clippy::fn_params_excessive_bools,
    reason = "a row is one anatomy and these are its readings; a struct \
              would name this call site and nothing else"
)]
fn queue_row(
    shelf: &Shelf,
    item: &crate::vm::QueueItemVm,
    queue_artist: &str,
    row_state: QueueRow,
    index: usize,
    total: usize,
    side_by_side: bool,
    live: bool,
    hovered: bool,
    collecting: Collecting,
    insert_line: Option<crate::drag::Edge>,
    observing: bool,
    selected: bool,
) -> Element<'static, Message> {
    let room = theme::active();
    let playing = row_state.state == QueueRowState::Playing;
    let ink = match row_state.state {
        QueueRowState::Played => room.paper_faint,
        QueueRowState::Playing | QueueRowState::Next | QueueRowState::Upcoming => room.paper,
    };
    // **Filled means sounding; open means about to.** The two marks share the
    // number lane, the dot's size and its lattice, so the run column says what
    // is playing and what is next in one vocabulary and one column of pixels.
    //
    // It earns its place hardest under shuffle, where the next track is *not*
    // the row below and nothing else on screen could tell you which one it is
    // — but it is drawn in both modes, because the fact is true in both and an
    // interface that only marks what is next when it is surprising is one that
    // has decided when you are allowed to know.
    //
    // **The ring is this surface's own**, and it is the one part of the row a
    // page cannot use: a document has no cursor, so it has no *next*. The dot
    // beside it is [`page::lamp_dot`], shared, because *sounding* is a fact
    // every surface states.
    let marker: Element<'static, Message> = match row_state.state {
        QueueRowState::Playing => page::lamp_dot(),
        QueueRowState::Next => next_ring(),
        _ => text(row_state.position.to_string())
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            .color(room.paper_faint)
            .into(),
    };
    let filed_under = item.album_artist.as_deref().unwrap_or(queue_artist);
    let album_id = item.album.as_deref().and_then(|title| {
        shelf
            .albums
            .iter()
            .find(|album| {
                album.title.as_deref() == Some(title) && album.artist.label() == filed_under
            })
            .map(|album| album.id)
    });
    let body = page::track_row(page::TrackRow {
        marker,
        artwork: Some(playlist_page::row_art(shelf, album_id)),
        title: row_state.title.into(),
        ink,
        under: row_state
            .artist
            .map(|artist| (Cow::Owned(artist), room.paper_dim, None)),
        context: Some((
            item.album
                .clone()
                .unwrap_or_else(|| "Unknown Album".to_owned())
                .into(),
            album_id.map(Message::OpenAlbum),
            side_by_side,
        )),
        duration: row_state.duration.into(),
        playing,
        press: live.then_some(Message::ContentPressed(Content::QueueTrack(index))),
    });
    // The drag wrapper owns the pointer for the body (crate::drag's module
    // docs): live rows lift on threshold and still click under it; every
    // row of the list measures a drag in flight against its own bounds.
    let mut source = crate::drag::Source::new(body, room);
    if live {
        source = source.wires(crate::drag::Wires::new(
            move |at| Message::DragLift(crate::drag::List::Queue, index, at),
            Message::DragMoved,
            Message::DragDropped,
            Some(Message::ContentPressed(Content::QueueTrack(index))),
        ));
    }
    if observing {
        source = source
            .observe(move |before| Message::DragOverRow(crate::drag::List::Queue, index, before));
    }
    let body: Element<'static, Message> = source.line(insert_line).into();
    let offered = live && hovered;
    let mut slots = row![
        body,
        page::favourite_slot(&item.path, crate::app::is_favourite(shelf, &item.path)),
        // The reorder pair and the removal cross, in the composition's one
        // slot anatomy ([`page::icon_slot`]) — the same three this file drew
        // for itself in three private functions that were byte-for-byte the
        // shared one.
        page::icon_slot(
            icon::Glyph::ArrowUp,
            "Move up",
            index > 0,
            offered,
            Message::ShiftQueued(index, -1)
        ),
        page::icon_slot(
            icon::Glyph::ArrowDown,
            "Move down",
            index + 1 < total,
            offered,
            Message::ShiftQueued(index, 1),
        ),
        page::icon_slot(
            icon::Glyph::Close,
            "Remove from the queue",
            true,
            offered,
            Message::RemoveQueued(index),
        ),
    ]
    .spacing(theme::GAP_XS)
    .align_y(iced::Alignment::Center);
    if collecting.available {
        // The transfer slot needs no engine — a pick can land in a file —
        // so it is offered on hover alone, and at rest while the panel
        // stands (the album page's own rule).
        slots = slots.push(page::transfer_slot(
            collecting.panel_open || hovered,
            Message::AddQueuedToPlaylist(index),
        ));
    }
    // The row's right press opens its mirror menu (doc 09 §5.2): play,
    // the transfer verbs, remove — each a press this row's own controls
    // already make.
    crate::menu::selection_area(
        mouse_area(page::row_card(hovered, playing, selected, slots))
            .on_enter(Message::QueueRowEntered(index))
            .on_exit(Message::QueueRowLeft(index)),
        crate::menu::Target::QueueRow { row: index },
    )
}

/// **The next row's mark**: the lamp dot's ring, unlit.
///
/// The same [`theme::DOT`] box in the same lane, drawn as a hairline circle in
/// [`theme::Palette::paper_dim`] instead of a filled disc in the lamp colour.
/// Deliberately **not** the accent: the accent is playback truth — it means
/// *this is sounding now* everywhere in the product (`theme`'s
/// accent-discipline note) — and a track that has not started is not sounding.
/// So the shape carries the relationship (this is the dot's sibling) and the
/// ink carries the difference (it is not lit yet).
fn next_ring() -> Element<'static, Message> {
    let room = theme::active();
    container(
        Space::new()
            .width(Length::Fixed(theme::DOT))
            .height(Length::Fixed(theme::DOT)),
    )
    .style(move |_theme| iced::widget::container::Style {
        border: iced::Border {
            color: room.paper_dim,
            // The product's hairline, the literal `theme` itself uses for
            // every 1 px border it draws.
            width: 1.0,
            radius: (theme::DOT / 2.0).into(),
        },
        ..iced::widget::container::Style::default()
    })
    .into()
}

#[cfg(test)]
mod tests {
    use super::unsaved_name;

    #[test]
    fn an_artist_run_is_named_as_the_unsaved_playlist_it_can_become() {
        let origin = crate::origin::Origin::Artist {
            id: 7,
            name: "Mahavishnu Orchestra".to_owned(),
        };
        assert_eq!(unsaved_name(Some(&origin)), "All Mahavishnu Orchestra");
        assert_eq!(
            unsaved_name(Some(&crate::origin::Origin::AllSongs)),
            "All songs"
        );
    }

    /// **The place draws the window's slice and nothing else** (doc 09 §7.1's
    /// gate on `Play all`), and **its rows carry the playlist page's whole
    /// edit set** (09 §8.2 — one editor).
    ///
    /// Pinned over the source the way `views/shelf.rs` pins its own ruler:
    /// the properties are about which widgets this file builds, there is no
    /// `PlayerState` to construct without an engine, and the literals below
    /// are exactly what a reviewer would have to delete to break them.
    #[test]
    fn the_queue_place_is_virtual_and_its_rows_are_the_playlist_editors() {
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/views/queue.rs"),
        )
        .expect("this module's own source")
        .replace("\r\n", "\n");
        // **The code half only** — `views::page`'s own `pages()` rule, and here
        // it is load-bearing rather than tidy.
        //
        // This test searches **the file it lives in**, so until 2026-08-10
        // every needle below was satisfied by the assertion that spelled it and
        // the test could not fail. It had gone stale twice without anyone
        // noticing: it looked for `window.height` after that argument was
        // renamed `viewport_h`, and it looked for a reserved-slot literal that
        // had moved to `views::page` entirely. Both passed.
        //
        // `now_playing.rs` and `implicit.rs` solve this by spelling their
        // needles in halves; that works for a handful and not for twenty, so
        // this one takes the same cut `views::page::tests::pages` takes and
        // searches only what the module actually builds.
        let source = source
            .split("#[cfg(test)]")
            .next()
            .expect("a source has a head")
            .to_owned();

        // The window: both persistence states spend the saved playlist's one
        // fixed-pitch row window. Both spacers are built and every drawn
        // element is boxed at that exact shared pitch.
        assert!(
            source.contains("super::playlist::row_window("),
            "the run must spend the saved playlist's row window"
        );
        assert!(
            source.contains("for index in win.first..win.end"),
            "only the window's slice is built"
        );
        assert!(
            source.contains("Space::new().height(win.top)")
                && source.contains("Space::new().height(win.bottom)"),
            "everything off screen is two spacers"
        );
        assert!(
            source.contains("Length::Fixed(super::playlist::ROW_PITCH)"),
            "drawn run entries must use the playlist row pitch"
        );
        assert!(
            source.contains("playlist_page::row_art(shelf, album_id)")
                && source.contains("context: Some(("),
            "a run entry must wear the shared artwork and Album presentation"
        );

        // The parity slots: ▲▼ on the queue's own edit message, the ✕, and
        // the transfer `+` toward the picker — every slot reserved.
        for spent in [
            "Message::ShiftQueued(index, -1)",
            "Message::ShiftQueued(index, 1)",
            "Message::RemoveQueued(index)",
            "Message::AddQueuedToPlaylist(index)",
        ] {
            assert!(
                source.contains(spent),
                "a queue row's reserved slots spend `{spent}`"
            );
        }
        // …and the reservation itself is `views::page::icon_slot`'s now — this
        // file had three private copies of it, byte-for-byte, until the rows
        // became one anatomy. The claim is unchanged and the address moved.
        let shared = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/views/page.rs"),
        )
        .expect("the shared composition's source")
        .replace("\r\n", "\n");
        assert!(
            shared.contains("Space::new().width(Length::Fixed(theme::STEPPER_HIT))"),
            "an unoffered slot is a space of exactly the control's width"
        );
        assert!(
            source.matches("page::icon_slot(").count() == 3
                && source.contains("page::transfer_slot("),
            "a queue row's four reserved slots are the composition's own"
        );

        // Step 8: the row's body is a drag source, and the drag is sugar —
        // it wraps the body *beside* the reserved slots above, all of
        // which the assertions before this one prove still stand. The
        // sub-threshold click is the row's own press, and the observation
        // wire is what makes the insertion index exact under this place's
        // virtualization.
        assert!(
            source.contains("crate::drag::Source::new(body, room)"),
            "the row's body is wrapped as a drag source"
        );
        assert!(
            source.contains("Some(Message::ContentPressed(Content::QueueTrack(index))),"),
            "a sub-threshold press is still the row's click"
        );
        assert!(
            source.contains("Message::DragOverRow(crate::drag::List::Queue, index, before)"),
            "every row measures a drag in flight against its own bounds"
        );
    }

    /// **The save word takes the shape the run permits** (ADR-0024 §A5.2, and
    /// the owner's narrowing of 2026-08-10) — **four** states, one slot, one
    /// height.
    ///
    /// The predicate itself is tested where it lives
    /// (`player::tests::the_save_word_offers_only_over_a_run_the_listener_assembled`);
    /// what is pinned here is that this file *spends* it, that every state is
    /// drawn at the button's own box so nothing in the strip moves when a run
    /// is edited — the empty one included, which is a reserved `Space` rather
    /// than an absence — and that no state offers a write-back. Over the
    /// source, for this module's own reason: there is no `PlayerState` to
    /// construct without an engine.
    #[test]
    fn the_save_word_is_a_readout_over_a_run_that_is_already_a_file() {
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/views/queue.rs"),
        )
        .expect("this module's own source")
        .replace("\r\n", "\n");

        assert!(
            source.contains("save_control(saving.is_none(), player.run_origin())"),
            "the strip's word reads the run, not only its own name field"
        );
        for (arm, word) in [
            ("RunOrigin::Assembled =>", "\"Save as playlist\""),
            ("RunOrigin::Saved(name) =>", "\"Saved as “{name}”\""),
            ("RunOrigin::Diverged(name) =>", "\"From “{name}”\""),
        ] {
            assert!(
                source.contains(arm) && source.contains(word),
                "the `{arm}` run no longer wears {word}"
            );
        }
        // **A fixed run says nothing, and holds its height.** The shared acts
        // lane keeps its `TRANSPORT_HIT` whichever of the four states is in
        // it, so changing provenance cannot move the identity or rows.
        let fixed = source
            .split("RunOrigin::Fixed => {")
            .nth(1)
            .expect("the fixed arm");
        let fixed = &fixed[..fixed.find("\n        }").unwrap_or(fixed.len())];
        assert!(
            fixed.contains("Space::new()")
                && fixed.contains(".height(Length::Fixed(theme::TRANSPORT_HIT))"),
            "a fixed run's slot stopped reserving the strip's height"
        );
        assert!(
            !fixed.contains("text(") && !fixed.contains("button("),
            "a list that already exists is being offered something"
        );
        // The readout is a statement, so it is not a button and it sends
        // nothing — the panel's `Queue` row's own form at rest. A disabled
        // button would still be a control claiming an act.
        let readout = source
            .split("let readout = |line: String|")
            .nth(1)
            .expect("the readout builder");
        let readout = &readout[..readout.find("\n    };").unwrap_or(readout.len())];
        assert!(
            !readout.contains("button(") && !readout.contains("on_press"),
            "a readout states; it does not offer"
        );
        assert!(
            readout.contains("Length::Fixed(theme::TRANSPORT_HIT)")
                && readout.contains("theme::pad(0.0, theme::GAP_SM)"),
            "…at the word's own box and inset, so the strip does not move"
        );
        // And a write-back is refused, in the code as well as in the record:
        // ADR-0024's 2026-08-09 amendment item 6 and ADR-0023 §3. The prose
        // is stripped first, because naming a refusal in a doc comment is how
        // the refusal survives the next reader.
        let code: String = source
            .lines()
            .filter(|line| {
                let line = line.trim_start();
                !line.starts_with("///") && !line.starts_with("//!") && !line.starts_with("//")
            })
            .collect::<Vec<_>>()
            .join("\n");
        // Spelled in two halves so this assertion is not itself the thing it
        // is looking for.
        let write_back = concat!("Save changes", " to");
        assert!(
            !code.contains(write_back),
            "provenance is an origin, never a live link — a run that wrote \
             itself back would be the two-structure confusion returning"
        );
        // The creation act survives, which is the guard that makes this a fix
        // rather than a removal (`every_queue_affordance_survives_the_merge`).
        assert!(
            source.contains("Message::SaveQueueStart"),
            "freezing a transient into a file is still one press away"
        );
    }
}
