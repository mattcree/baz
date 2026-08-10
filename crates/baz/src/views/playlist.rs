//! **A playlist's page**: one list a person made, at the width of the window
//! (ADR-0024 §4) — the record page's sibling in arrangement, the queue
//! place's sibling in row anatomy.
//!
//! # The shared composition, the queue place's rows
//!
//! Since the sleeve amendment (ADR-0024 §A2) the page wears the album page's
//! own two-column arrangement: **the object beside what is written about
//! it** — the collage sleeve at [`theme::ALBUM_SLEEVE`] over `Play` and the
//! quieter acts in the aside, the name at hero scale over the rows in the
//! main column, stacking below the same breakpoint by the same arithmetic.
//!
//! Since *one page, two subjects* (2026-08-10) that is not a resemblance but
//! an identity: the arrangement is [`views::page`](super::page) and this
//! module hands it a playlist. Everything here is what a *made list* puts in
//! the composition's slots, and nothing here lays out a page.
//!
//! The rows themselves stay the queue place's — one anatomy for every list in
//! baz — plus the reserved edit slots a durable artefact earns: the ✕ that
//! takes an entry out and the ▲▼ steppers that reorder, the no-drag pointer
//! route the visible-control rule requires, with drag-to-reorder deferred to
//! the shared pointer-capture widget (ADR-0024 §6 layer 3).
//!
//! **The arrangement is the album page's; the declared hierarchy is not**
//! (ADR-0024 §A4.2). §A2 imported both, and the second does not transfer:
//! *the work ≫ `Play` → the title → the rows* is right on a record's page
//! because **the sleeve is the work** and the title captions it. A playlist's
//! collage is not an image *of* the list — it is four quotations *from* it,
//! evidence about rows that are further down the same page, and the contents
//! change constantly while the name does not. So this page declares
//! **the name ≫ `Play` → the collage → the rows**.
//!
//! Demoting the collage in the *declaration* does not shrink it in the
//! *layout*: it stays at [`theme::ALBUM_SLEEVE`] in a 320 px aside, because
//! that width is what lets the aside's blocks share one x-edge (law L5) and
//! shrinking it would buy nothing. What the demotion buys is the byline line
//! under the name — see [`identity`], which is where the owner's
//! *"the playlist name isn't really prominent"* is actually answered.
//!
//! # The honesty on this surface
//!
//! - **A missing entry stays.** It renders dimmed from its path's stem, with
//!   the path itself on the row, unplayable — never silently pruned
//!   (ADR-0024 §3), and the summary says the arithmetic out loud:
//!   `38 of 40 · 2 missing`.
//! - **A row click is the same rule every list surface spends**:
//!   [`PlayerState::play_from`] over the playable subset — a jump when the
//!   engine already holds exactly this list, `SetQueue` + `JumpTo` when it
//!   does not.
//! - **The lamp dot marks a row only when the queue is exactly this list**
//!   ([`PlayerState::playing_row_in`]) — a page listing something other than
//!   what the engine holds marks nothing.
//! - **Delete is one press, into the platform trash** (doc 11 §5 P2): the
//!   confirm dialog and its sentence — *"The file goes; your music stays"* —
//!   retired with honour, because the trash keeps the promise the sentence
//!   made. Forgiveness beats warning: reversibility first, per the 1992
//!   HIG's own ranking, and the desktop's Restore is the road back.
//! - **Undo stands beside the counts while there is an edit to take back**
//!   (P2 again): remove, reorder and append are whole-file rewrites, and
//!   the file as it stood is one press — or <kbd>Ctrl</kbd>+<kbd>Z</kbd> —
//!   away, through the same fingerprint guard as the edit it reverses.

use iced::widget::{button, column, container, mouse_area, row, text, text_input};
use iced::{Element, Length, alignment};

use crate::app::{Message, Shelf};
use crate::player::{Availability, PlayerState};
use crate::playlists::{Collecting, NameEntry, OpenPlaylist, PageRow};
use crate::views::page::{self, Identity, Page};
use crate::views::{place_name, playlist_sleeve};
use crate::{icon, theme};

/// The rename field's id, so the caret can land in it the moment `Rename` is
/// pressed.
pub(crate) fn rename_id() -> text_input::Id {
    text_input::Id::new("baz-playlist-rename")
}

/// The playlist's page: [`views::page`](crate::views::page)'s composition, with
/// a made list in it.
///
/// The arrangement is the shared one and this module supplies what is *about a
/// list*: the collage sleeve, `Play`, the three acts a durable artefact earns,
/// the rename field while it stands, the sans name over the byline over the
/// counts, and the rows with their edit slots.
///
/// **The strip leads with the list's name.** It led with the word `Playlist`
/// until this change — see `views::page`'s own docs: the strip names the
/// subject on every page whose subject changes, and the kind is stated in the
/// byline 19 px under the name, where design 14 §3.5 argued it belongs.
#[expect(
    clippy::too_many_arguments,
    reason = "each argument is one independent reading the page renders — \
              the drag in flight and the undo affordance arrived from two \
              different studies, and bundling them into a struct would name \
              nothing the call site does not already say"
)]
pub(crate) fn view<'a>(
    shelf: &'a Shelf,
    open: &'a OpenPlaylist,
    player: &'a PlayerState,
    window_width: f32,
    hovered: Option<usize>,
    collecting: Collecting,
    drag: Option<&'a crate::drag::DragState>,
    can_undo: bool,
) -> Element<'a, Message> {
    let live = player.engine_ready();
    let playable = !open.queue.is_empty();
    let mut aside_tail: Vec<Element<'a, Message>> = Vec::new();
    if let Some(renaming) = &open.renaming {
        aside_tail.push(rename_field(renaming));
    }
    page::view(
        Page {
            lead: place_name(open.name()),
            // The collage of quotations (§A1), at the record page's own sleeve
            // edge.
            sleeve: playlist_sleeve(shelf, &open.art, open.name(), theme::ALBUM_SLEEVE),
            // Drawn only where there is an engine in the build to send it to —
            // the record page's rule, which this page did not have: a `Play`
            // that could never act in any state of any run is not a control.
            commitment: (*player.availability() != Availability::NotBuilt)
                .then(|| page::commitment("Play", live && playable, Message::PlaylistPlay)),
            acts: vec![
                page::act("Queue", live && playable, Message::PlaylistQueue),
                page::act("Rename", true, Message::PlaylistRenameStart),
                // One press, into the platform trash (doc 11 §5 P2): the
                // confirm died when the act became reversible — the desktop's
                // own Restore is the road back, so a warning would be the
                // fallback posture shipped as the default.
                page::act("Delete", true, Message::PlaylistDelete),
            ],
            aside_tail,
            identity: identity(open, can_undo),
            rows: entry_rows(open, player, hovered, collecting, drag, live),
            // The words the armed mode left behind went with it (doc 09 §9):
            // the route in is the transfer gesture — a row's `+`, or the
            // record page's `Add to playlist…`, then this list in the picker.
            empty: "Nothing here yet. Press + on any track row, or Add to playlist… on a record's page, and pick this list.",
        },
        window_width,
    )
}

/// **Every entry**, with a record's name where its run begins.
fn entry_rows<'a>(
    open: &'a OpenPlaylist,
    player: &'a PlayerState,
    hovered: Option<usize>,
    collecting: Collecting,
    drag: Option<&'a crate::drag::DragState>,
    live: bool,
) -> Vec<Element<'a, Message>> {
    // Which display row carries the lamp: the engine's confirmed row in the
    // playable subset, mapped back through each row's own subset position —
    // and nothing at all unless the queue is exactly this list.
    let playing_playable = player.playing_row_in(&open.tracks);
    let mut rows: Vec<Element<'a, Message>> = Vec::new();
    for (index, page_row) in open.rows.iter().enumerate() {
        if let Some((album, artist)) = &page_row.head {
            rows.push(record_head(album, artist, rows.is_empty()));
        }
        let playing =
            page_row.playable_position.is_some() && page_row.playable_position == playing_playable;
        rows.push(entry_row(
            page_row,
            index,
            open.rows.len(),
            live,
            playing,
            hovered == Some(index),
            collecting,
            drag.and_then(|held| held.line_for_row(crate::drag::List::Playlist, index)),
            drag.is_some_and(|held| held.list == crate::drag::List::Playlist),
        ));
    }
    rows
}

/// What the shared identity block ([`Identity`]) says about a **made list**:
/// the name at hero scale, the byline under it, then the counts —
/// `38 of 40 · 2 missing` when entries are missing. Beside the counts, exactly
/// while there is an edit to take back, stands the transient `Undo`
/// (doc 11 §5 P2) — the queue place's word, on the page that is its sibling
/// editor, and the one thing in this block a record's page has no use for.
///
/// # The byline, and why it is the answer to *"the name isn't prominent"*
///
/// The owner, 2026-08-10: *"we do not have the playlist name really
/// prominent."* The name was **already** the album title's own `SIZE_HERO` 28
/// / `SEMIBOLD`. What it was missing was the line under it: a record's
/// identity block is three lines — title 28, artist `SIZE_TITLE` 19,
/// catalogue `SIZE_META` 12, **80 px** (`views/album.rs`'s `album_header`) —
/// and this one was two, **52 px**.
/// This page was the album page *with the byline deleted*, so the name
/// terminated after 52 px and read as a stub rather than a placard
/// (ADR-0024 §A4.3; design 14 §3.4).
///
/// So the slot is restored, at the record's own size and ink, and what fills
/// it is the word `Playlist`. The two identity blocks are now
/// **geometrically identical at 80 px** and the difference lives in *what the
/// middle line says*, which is where a difference between two kinds of thing
/// belongs. The name is not made larger: `SIZE_HERO` is the top of the ramp,
/// and the prominence problem was a missing line, not a small number.
///
/// **Not *"Made by you"***: ADR-0024 §4 admits `.m3u8` files dropped into the
/// playlists folder, which this product did not author and whose author no
/// file records. The byline claims only what the file can prove — the kind,
/// and the composition beside it ([`byline`], `Playlist · 4 records`), which
/// also explains the collage it stands next to.
///
/// # The hero stays in the sans, and that is the axis rather than an omission
///
/// `views::album`'s hero takes `theme::WORK_TITLE` — serif italic, the museum
/// placard's convention for a work's own title. This one, at the same
/// `SIZE_HERO` 28 in the same ink in the same slot, **does not**, and the
/// asymmetry is the statement (ADR-0024 §A4.4; design 14 §5.2): a record's
/// title is a work somebody published, and a playlist's name is a label the
/// owner typed — like the search query, the rename field two lines below this
/// one, and the folder path in `DETAILS`, every one of which is already sans.
/// Flattening the two heroes back into one face would delete the distinction
/// tier 1 spent three strings stating.
fn identity(open: &OpenPlaylist, can_undo: bool) -> Identity<'static> {
    Identity {
        name: open.name().to_owned(),
        // Sans, against the record page's serif italic. See this function's
        // docs — it is the axis, not an omission.
        face: theme::SEMIBOLD,
        byline: byline(open.records),
        facts: open.counts_line(),
        beside_facts: can_undo.then(undo_control),
    }
}

/// The word in the byline slot: what this object **is**, in the one place a
/// record names its artist (ADR-0024 §A3.1 — the kind stated in words, and
/// the same first token [`crate::playlists::PanelRow::counts`] gives the lane
/// and the panel).
pub(crate) const KIND: &str = "Playlist";

/// The byline: [`KIND`], then **what the list is made of** —
/// `Playlist · 4 records` (ADR-0024 §A4.3, design 14 §5.4 / tier 2 #7).
///
/// The record's byline names a person. This one cannot — §4 admits `.m3u8`
/// files dropped into the playlists folder, whose author no file records — so
/// it names the composition instead, which is a fact the file can prove. It
/// also **explains the collage beside it**: the picture is quotations from the
/// things below, and this says how many things there are to quote from.
///
/// # It is the distinct-record count, not the collage's
///
/// Design 14 §5.4 costed this as free, *"from the distinct-record list
/// `playlists.rs` already computes for the sleeve"*. **That list cannot pay
/// for it**: it stops at four, because four is all a 2 × 2 can hold. A
/// fourteen-record list would have read `Playlist · 4 records` over a page
/// listing fourteen records — a byline that is false about its own object, in
/// the slot this whole change exists to make honest. The count is walked to
/// its end instead ([`crate::playlists::OpenPlaylist::records`]).
///
/// Below one, the word stands alone. A list whose entries the library cannot
/// resolve is still a playlist and still says so; what it may not do is claim
/// a composition it cannot show. Zero geometry either way — one `SIZE_TITLE`
/// line in the same slot, and `wrapping(None)` holds it to one line at every
/// width the aside takes.
fn byline(records: usize) -> String {
    match records {
        0 => KIND.to_owned(),
        1 => format!("{KIND} · 1 record"),
        n => format!("{KIND} · {n} records"),
    }
}

/// **Undo** — the file as it stood before the last recorded edit, one press
/// away (doc 11 §5 P2): the queue place's word and rule, worn by the page.
/// Drawn only while the history holds something; `Ctrl+Z` is the
/// accelerator this word makes legal.
fn undo_control() -> Element<'static, Message> {
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

/// The rename field, with the storage layer's refusal under it when the last
/// submission was refused — the same anatomy as the panel's name field.
fn rename_field(entry: &NameEntry) -> Element<'_, Message> {
    let room = theme::active();
    let mut block = column![
        text_input("New name…", &entry.text)
            .id(rename_id())
            .on_input(Message::PlaylistRenameInput)
            .on_submit(Message::PlaylistRenameSubmit)
            .padding(theme::pad(theme::WELL_PAD_V, theme::GAP_MD))
            .size(theme::SIZE_BODY)
            .line_height(theme::LEADING_BODY)
            .width(Length::Fill)
            .style(move |_theme, status| theme::input(room, status)),
    ]
    .spacing(theme::GAP_XS);
    if let Some(error) = &entry.error {
        block = block.push(
            text(error.clone())
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.alert),
        );
    }
    block.into()
}

/// A record's name where its run begins — the queue place's group-header
/// rule, drawn over consecutive same-record runs so the playlist stays a
/// track list that still says where things came from.
fn record_head(album: &str, artist: &str, first: bool) -> Element<'static, Message> {
    let room = theme::active();
    let air = if first { 0.0 } else { theme::GAP_MD };
    let mut block = column![
        text(album.to_owned())
            .size(theme::SIZE_BODY)
            .line_height(theme::LEADING_BODY)
            .font(theme::MEDIUM)
            .color(room.paper_dim)
            .wrapping(text::Wrapping::None),
    ]
    .spacing(theme::GAP_XXS);
    if !artist.is_empty() {
        block = block.push(
            text(artist.to_owned())
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.heading())
                .wrapping(text::Wrapping::None),
        );
    }
    container(block).padding(theme::pad(air, 0.0)).into()
}

/// One entry's row: position (or the lamp dot), title over its artist — or
/// over its path, when the entry is missing — duration, and the three
/// reserved edit slots.
///
/// The queue row's fixed-slot rules, extended by two: the number column is
/// [`theme::TRACK_NO_W`] whichever it holds, and the ▲▼ and ✕ slots are
/// reserved whether or not their controls are in them, so durations never
/// slide as the pointer crosses a row. The controls appear on hover — a
/// column of permanent crosses down a list you are reading is a column of
/// invitations to destroy something — and hover is not their only route in
/// the product's terms: the file itself is the user's, editable in any text
/// editor, and the page re-reads it (ADR-0024 §2).
///
/// Since doc 09 §13 step 4 the row also carries the **transfer `+`** in the
/// queue row's outer slot — the last piece of §8.2's "same editor" claim,
/// and the visible twin the row's context-menu items mirror (§5.2). A
/// missing entry gets no `+` for the ✕'s opposite reason: there is nothing
/// there to transfer.
/// **The row's body is a drag source** (doc 09 §13 step 8; [`crate::drag`]):
/// press and travel lifts the row for a reorder — one saved file on the
/// drop, [`crate::playlists::Playlists::move_entry`] — or a carry to the
/// standing panel's rows to add. A missing entry drags too (its position
/// is real) but carries no payload, so a panel drop moves nothing — the
/// `+`'s own refusal, held by the drag. Sugar only: the ▲▼, ✕ and `+`
/// remain, and the sub-threshold press is the row's ordinary click.
#[expect(
    clippy::too_many_lines,
    clippy::too_many_arguments,
    clippy::fn_params_excessive_bools,
    reason = "a row is one anatomy — marker, title, duration, four reserved \
              slots — and splitting it would put half the reservation rules \
              out of sight of the other half"
)]
fn entry_row(
    page_row: &PageRow,
    index: usize,
    total: usize,
    live: bool,
    playing: bool,
    hovered: bool,
    collecting: Collecting,
    insert_line: Option<crate::drag::Edge>,
    observing: bool,
) -> Element<'_, Message> {
    let room = theme::active();
    let ink = if page_row.missing {
        room.paper_faint
    } else {
        room.paper
    };
    let marker: Element<'_, Message> = if playing {
        page::lamp_dot()
    } else {
        text(page_row.position.to_string())
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            .color(room.paper_faint)
            .into()
    };
    let heading = text(page_row.title.clone())
        .size(theme::SIZE_BODY)
        .line_height(theme::LEADING_BODY)
        .color(ink)
        .wrapping(text::Wrapping::None);
    let heading = if playing {
        heading.font(theme::MEDIUM)
    } else {
        heading
    };
    let mut title = column![heading].spacing(theme::GAP_XXS);
    if page_row.missing {
        // The path, one glance away (ADR-0024 §3): a missing entry's row is
        // drawn from its stem, and this line is where it went.
        title = title.push(
            text(page_row.path.display().to_string())
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.paper_muted)
                .wrapping(text::Wrapping::None),
        );
    } else if let Some(artist) = &page_row.artist {
        title = title.push(
            text(artist.clone())
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.paper_dim)
                .wrapping(text::Wrapping::None),
        );
    }
    let body = button(
        row![
            container(marker)
                .width(Length::Fixed(theme::TRACK_NO_W))
                .height(Length::Fixed(theme::CAPTION_LINE_H))
                .align_x(alignment::Horizontal::Right)
                .align_y(alignment::Vertical::Center),
            container(title).width(Length::Fill),
            container(
                text(page_row.duration.clone())
                    .size(theme::SIZE_META)
                    .line_height(theme::LEADING_META)
                    .color(room.paper_faint)
                    .wrapping(text::Wrapping::None)
            )
            .width(Length::Fixed(theme::DURATION_W))
            .height(Length::Fixed(theme::CAPTION_LINE_H))
            .align_x(alignment::Horizontal::Right)
            .align_y(alignment::Vertical::Center),
        ]
        .spacing(theme::GAP_SM)
        .align_y(iced::Alignment::Start),
    )
    .width(Length::Fill)
    .padding(theme::pad(theme::GAP_XS, 0.0))
    .style(move |_theme, status| theme::track_row(room, room.wall, status, playing))
    // A missing entry is not a control: pressing a row plays from it, and
    // there is nothing there to play.
    .on_press_maybe((live && !page_row.missing).then_some(Message::PlaylistPlayTrack(index)));
    // The drag wrapper owns the pointer for the body (crate::drag): every
    // row of the artefact is draggable — a file edit needs no engine, the
    // steppers' own rule — and the sub-threshold click keeps the button's
    // exact gate.
    let mut source = crate::drag::Source::new(body, room).wires(crate::drag::Wires::new(
        move |at| Message::DragLift(crate::drag::List::Playlist, index, at),
        Message::DragMoved,
        Message::DragDropped,
        (live && !page_row.missing).then_some(Message::PlaylistPlayTrack(index)),
    ));
    if observing {
        source = source.observe(move |before| {
            Message::DragOverRow(crate::drag::List::Playlist, index, before)
        });
    }
    let body: Element<'_, Message> = source.line(insert_line).into();
    let offered = hovered;
    let mut slots = row![
        body,
        // The reorder pair and the removal cross — the three slots a durable
        // artefact earns over a record's rows, in the composition's one slot
        // anatomy ([`page::icon_slot`]).
        page::icon_slot(
            icon::Glyph::ArrowUp,
            "Move up",
            index > 0,
            offered,
            Message::PlaylistShiftEntry(index, -1)
        ),
        page::icon_slot(
            icon::Glyph::ArrowDown,
            "Move down",
            index + 1 < total,
            offered,
            Message::PlaylistShiftEntry(index, 1),
        ),
        page::icon_slot(
            icon::Glyph::Close,
            "Remove from the playlist",
            true,
            offered,
            Message::PlaylistRemoveEntry(index),
        ),
    ]
    .spacing(theme::GAP_XS)
    .align_y(iced::Alignment::Center);
    if collecting.available {
        // The transfer slot, in the queue row's outer position and by its
        // rule: no engine needed (a pick can land in a file), offered on
        // hover and at rest while the panel stands. A missing entry keeps
        // the reserved space and no control.
        slots = slots.push(page::transfer_slot(
            !page_row.missing && (collecting.panel_open || hovered),
            Message::PlaylistAddEntry(index),
        ));
    }
    // The row's right press opens its mirror menu (doc 09 §5.2): play and
    // the transfer verbs, each a press this row's own controls already
    // make. A missing entry's menu offers nothing, exactly as its row does.
    crate::menu::area(
        mouse_area(slots)
            .on_enter(Message::PlaylistRowEntered(index))
            .on_exit(Message::PlaylistRowLeft(index)),
        crate::menu::Target::PlaylistTrack {
            row: index,
            missing: page_row.missing,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::KIND;

    /// **The byline states a composition it can prove**, and the first token
    /// is the kind in every form it takes (ADR-0024 §A4.3).
    ///
    /// The regression this exists for is arithmetic rather than wording:
    /// design 14 costed the count as free from the sleeve's quotation list,
    /// which stops at four. A byline reading `Playlist · 4 records` over a
    /// fourteen-record page would be a false statement in the slot the whole
    /// change exists to make honest, so the count is the *whole* distinct set
    /// and the plural is real.
    #[test]
    fn the_byline_names_the_kind_first_and_counts_what_it_can_prove() {
        assert_eq!(
            super::byline(0),
            "Playlist",
            "nothing resolved, nothing claimed"
        );
        assert_eq!(super::byline(1), "Playlist · 1 record");
        assert_eq!(super::byline(4), "Playlist · 4 records");
        assert_eq!(
            super::byline(14),
            "Playlist · 14 records",
            "the count is the distinct set, not the collage's four"
        );
        for records in [0usize, 1, 2, 4, 14, 206] {
            let line = super::byline(records);
            assert!(
                line.starts_with(KIND),
                "the kind is the first token in every form: {line:?}"
            );
        }
    }
}
