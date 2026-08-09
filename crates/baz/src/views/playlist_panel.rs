//! **The playlist panel**: the one summoned, single-tenant side surface baz
//! has (ADR-0024 §5), floating over the wall's right edge for the duration of
//! a collecting task and gone at rest.
//!
//! # Why this exists when the refusals ledger buried its ancestors
//!
//! `docs/REFUSALS.md`'s side-surfaces entry is **amended, not deleted**, by
//! ADR-0024 under the ledger's own editing rule, and the amendment names this
//! panel and closes the slot. The rail died of five findings; this panel has
//! none of them by construction — one tenant forever, summoned not resident,
//! overlaying without reflow — and one thing no place can have:
//! **simultaneity**. Collecting is two-surface work: the source (wall, page,
//! queue) and the destination must be on screen at once, and a place model
//! cannot show two things at once (ADR-0022 says so itself). The panel
//! *receives*; it does not display a selection, which is what the dead column
//! did and what places do better.
//!
//! The panel's tenant is **ordered lists of tracks** — the unnamed one
//! included (`docs/design/09-implicit-playlists.md` §8.1, the ADR-0024
//! amendment's restated single-tenant clause): the **Queue** heads the list
//! as the sounding, unnamed list. At rest its row is a *readout, not a door*
//! — the queue's door is the bar's labelled `Queue`, and a second door would
//! be L8.6's violation; facts may be restated everywhere, controls may not.
//!
//! # The float mechanics
//!
//! ADR-0016's verified popover mechanics, revived for a surface that earns
//! them: composed into a `stack` over the place (in `app.rs`), wrapped in
//! `opaque` so a press inside its bounds cannot fall through to a tile
//! underneath, with **no scrim** (refused outright) and wheel events passing
//! through beside it — the wall keeps scrolling while the panel stands. The
//! wall is not re-laid by a pixel: the panel is a layer, not a column, so
//! [`crate::app::Shelf::grid_width`]'s "no press re-hangs the collection"
//! survives, which is the property the render harness diffs for
//! (`docs/design/impl/playlists/`).
//!
//! # What it holds
//!
//! The Queue's row, then one row per playlist — the name and sleeve, a door
//! to [`Place::Playlist`](crate::place::Place) — then `New playlist`. Rename
//! and delete are *not* here: destruction lives on the page, where the
//! contents are visible at the moment of the decision. While a pick is in
//! flight the panel is **the picker** (09 §8.1): every row becomes the
//! pick's target — the Queue first (append to the run), the playing list
//! hoisted second when provenance stands, the named lists, `New playlist` —
//! one grammar: pick a destination, whatever the destination is. The armed
//! collecting mode that used to live on these rows is removed (09 §9); a row
//! carries one control, and the panel is a directory, not a workspace.

use iced::widget::{
    Column, Space, button, column, container, mouse_area, row, scrollable, text, text_input,
};
use iced::{Element, Length, alignment};

use crate::app::{Message, Shelf};
use crate::player::PlayerState;
use crate::playlists::{NameEntry, PanelRow, Playlists, playlist_id};
use crate::theme;
use crate::views::playlist_sleeve;
use crate::vm;

/// The panel's width: the one dimension of the dead rail nobody faulted
/// (`docs/design/08` §5.5), spent on a surface that is only ever on screen
/// while a hand is collecting.
pub(crate) const PANEL_W: f32 = 340.0;

/// The `New playlist` field's id, so `app.rs` can put the caret in it the
/// moment the row becomes a field.
pub(crate) fn new_name_id() -> text_input::Id {
    text_input::Id::new("baz-playlist-new")
}

/// The panel, ready to be stacked over whichever place is standing.
///
/// `shelf` supplies the thumbnail cache the rows' sleeves quote
/// (ADR-0024 §A1) — the panel reads it exactly as the wall does and owns no
/// pixel of it. `player` supplies the two facts the Queue row states — the
/// run's size and time — and the provenance the picker hoists by (09 §6);
/// both are readings, never held here.
pub(crate) fn view<'a>(
    shelf: &'a Shelf,
    playlists: &'a Playlists,
    player: &'a PlayerState,
    drag: Option<&'a crate::drag::DragState>,
) -> Element<'a, Message> {
    let room = theme::active();
    let picking = playlists.pending.is_some();
    // A drag carrying a track can land on the named rows (doc 09 §13
    // step 8: drag-to-add, the picker row's append made direct). A drag
    // with nothing in the hand — a missing entry's row — offers no
    // targets, exactly as its `+` offers no transfer.
    let receiving = drag.is_some_and(|held| held.payload.is_some());
    // The playing list, while provenance stands *and* the file still exists —
    // a rename or delete under the run withdraws the hoist rather than
    // letting it dangle (09 §6).
    let playing = player
        .queue_provenance()
        .map(playlist_id)
        .filter(|id| playlists.row(*id).is_some());
    let mut body = column![].spacing(theme::GAP_SM);
    // The panel's own name, and how it leaves — the place-header shape at
    // panel scale, so the surface answers "what is this" the way every other
    // surface does.
    body = body.push(
        row![
            text("Playlists")
                .size(theme::SIZE_EMPHASIS)
                .line_height(theme::LEADING_EMPHASIS)
                .font(theme::MEDIUM),
            Space::with_width(Length::Fill),
            text("Esc closes")
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.paper_faint)
                .wrapping(text::Wrapping::None),
        ]
        .spacing(theme::GAP_SM)
        .align_y(iced::Alignment::Center),
    );
    // A pick in flight: the panel is the picker, and it says what is in the
    // hand so the next press is legible before it is made.
    if let Some(pending) = &playlists.pending {
        body = body.push(
            text(format!("{} — pick a destination", pending.label))
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.paper_dim),
        );
    }
    // The rows, in the picker's one order (09 §8.1): the Queue first, the
    // playing list second when one stands, the named lists, `New playlist`
    // last. At rest the same shape minus the hoist: the Queue as a readout at
    // the head, the folder's own order below it.
    let mut listed: Vec<Element<'_, Message>> = vec![queue_row(player, picking)];
    if playlists.rows.is_empty() {
        listed.push(empty_words(playlists));
    } else {
        let named: Vec<&PanelRow> = if picking {
            playlists.picker_order(playing)
        } else {
            playlists.rows.iter().collect()
        };
        for entry in named {
            listed.push(playlist_row(
                shelf,
                entry,
                picking,
                picking && playing == Some(entry.id),
                receiving,
                drag.is_some_and(|held| held.over_panel == Some(entry.id)),
            ));
        }
    }
    body = body.push(
        scrollable(Column::with_children(listed).spacing(theme::GAP_XS))
            .direction(scrollable::Direction::Vertical(theme::list_scrollbar()))
            .style(move |_theme, status| theme::scrollbar(room, room.plinth, status))
            .height(Length::Fill),
    );
    // `New playlist`, the last row (09 §8.1's picker diagram): creation is
    // the furthest destination, under every list that already exists.
    body = body.push(match &playlists.naming {
        None => new_playlist_door(),
        Some(entry) => name_field(entry),
    });
    // One hairline down the left edge is the seam between the panel and the
    // wall it floats over — the surface step does the rest; a shadow is
    // refused (`docs/REFUSALS.md`).
    row![
        container(Space::with_width(Length::Fixed(1.0)))
            .width(Length::Fixed(1.0))
            .height(Length::Fill)
            .style(move |_theme| theme::panel_seam(room)),
        container(body)
            // A panel's own content keeps `GAP_XL` from the panel's edge —
            // law L1's second clause, stated for exactly this case.
            .padding(theme::GAP_XL)
            .width(Length::Fixed(PANEL_W))
            .height(Length::Fill)
            .style(move |_theme| theme::panel(room)),
    ]
    .into()
}

/// **The Queue's row** — the unnamed, sounding list at the head of the
/// directory (09 §8.1).
///
/// At rest it is a **readout, not a door**: name and counts in the room's
/// quieter voice, no press — the queue's one door is the bar's labelled
/// `Queue`, and a second would be two controls sending one message (L8.6).
/// While a pick is in flight it is the picker's first destination: one press
/// appends the held music to the run (`UpdateQueue` — the music keeps
/// playing, and appending to an empty stopped engine loads a queue without
/// starting it).
fn queue_row(player: &PlayerState, picking: bool) -> Element<'static, Message> {
    let room = theme::active();
    let counts = match player.queue().filter(|queue| !queue.is_empty()) {
        Some(queue) => {
            let time = queue.total_time();
            if time.is_zero() {
                queue.len().to_string()
            } else {
                format!("{} · {}", queue.len(), vm::format_duration(time))
            }
        }
        None => "Nothing queued".to_owned(),
    };
    let name_block = column![
        text("Queue")
            .size(theme::SIZE_BODY)
            .line_height(theme::LEADING_BODY)
            .font(theme::MEDIUM)
            .color(room.paper_dim)
            .wrapping(text::Wrapping::None),
        text(counts)
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            .color(room.paper_faint)
            .wrapping(text::Wrapping::None),
    ]
    .spacing(theme::GAP_XXS);
    if picking {
        // The pick's first target: one press, one append to the run.
        return button(
            row![
                container(name_block).width(Length::Fill),
                text("Add")
                    .size(theme::SIZE_META)
                    .line_height(theme::LEADING_META)
                    .font(theme::MEDIUM)
                    .color(room.paper_dim)
                    .wrapping(text::Wrapping::None),
            ]
            .spacing(theme::GAP_SM)
            .align_y(iced::Alignment::Center),
        )
        .width(Length::Fill)
        .padding(theme::pad(theme::GAP_XS, theme::GAP_SM))
        .style(move |_theme, status| theme::track_row(room, status, false))
        .on_press(Message::PickQueue)
        .into();
    }
    container(name_block)
        .width(Length::Fill)
        .padding(theme::pad(theme::GAP_XS, theme::GAP_SM))
        .into()
}

/// `New playlist`, at rest: a quiet word that becomes a name field on press —
/// the roots field's two-state anatomy (ADR-0022's add-a-folder row).
fn new_playlist_door() -> Element<'static, Message> {
    let room = theme::active();
    button(
        container(
            text("New playlist")
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
    .style(move |_theme, status| theme::word_button(room, room.plinth, status))
    .on_press(Message::NewPlaylistStart)
    .into()
}

/// The name field the `New playlist` row becomes, with the storage layer's
/// refusal under it in plain words when the last submission was refused.
fn name_field(entry: &NameEntry) -> Element<'_, Message> {
    let room = theme::active();
    let mut block = column![
        text_input("Name the playlist…", &entry.text)
            .id(new_name_id())
            .on_input(Message::NewPlaylistInput)
            .on_submit(Message::NewPlaylistSubmit)
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

/// One playlist's row: the sleeve, then the name over its counts — one
/// control, the door to its page (09 §9's shrinking: the receive `+` is
/// gone, and a row is a directory entry rather than a workspace).
///
/// While a pick is in flight the whole row is the pick's target instead —
/// pressing anywhere on it appends what the hand holds — because a picker
/// whose rows kept their ordinary meaning would make the most important
/// press on the surface the hardest to aim. The playing list, hoisted to the
/// head of the named rows, is `marked` — *playing*, in the quieter voice —
/// and its pick appends to the **file** only, never the sounding run
/// (09 §6, S4).
///
/// While a **drag** is in flight (`receiving`, doc 09 §13 step 8) the row is
/// a drop target: it reports the held pointer crossing it, and it draws the
/// row's own hovered statement while the drop would land here (`hot`) —
/// the highlight is the room's existing hover, stated from the drag's own
/// fact rather than left to the cursor, so the frame and the commit cannot
/// disagree about where the track goes.
#[expect(
    clippy::fn_params_excessive_bools,
    reason = "four independent readings of one row — the pick, the mark, \
              the drag's standing and its aim — and a struct would name \
              this call site and nothing else"
)]
fn playlist_row<'a>(
    shelf: &'a Shelf,
    entry: &'a PanelRow,
    picking: bool,
    marked: bool,
    receiving: bool,
    hot: bool,
) -> Element<'a, Message> {
    let room = theme::active();
    // The sleeve, at the row's own scale (ADR-0024 §A2): what turns a list
    // of names into a shelf of objects.
    let sleeve = playlist_sleeve(shelf, &entry.art, &entry.name, theme::PANEL_SLEEVE);
    let mut name_line = row![
        text(entry.name.clone())
            .size(theme::SIZE_BODY)
            .line_height(theme::LEADING_BODY)
            .font(theme::MEDIUM)
            .color(room.paper)
            .wrapping(text::Wrapping::None),
    ]
    .spacing(theme::GAP_XS)
    .align_y(iced::Alignment::Center);
    if marked {
        name_line = name_line.push(
            text("— playing")
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.paper_faint)
                .wrapping(text::Wrapping::None),
        );
    }
    let name_block = column![
        name_line,
        text(entry.counts())
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            .color(room.paper_faint)
            .wrapping(text::Wrapping::None),
    ]
    .spacing(theme::GAP_XXS);
    if picking {
        // The pick's target: one press, one append to the file, and the row
        // says so.
        return button(
            row![
                sleeve,
                container(name_block).width(Length::Fill),
                text("Add")
                    .size(theme::SIZE_META)
                    .line_height(theme::LEADING_META)
                    .font(theme::MEDIUM)
                    .color(room.paper_dim)
                    .wrapping(text::Wrapping::None),
            ]
            .spacing(theme::GAP_SM)
            .align_y(iced::Alignment::Center),
        )
        .width(Length::Fill)
        .padding(theme::pad(theme::GAP_XS, theme::GAP_SM))
        .style(move |_theme, status| theme::track_row(room, status, false))
        .on_press(Message::PickPlaylist(entry.id))
        .into();
    }
    let door = button(
        row![sleeve, container(name_block).width(Length::Fill)]
            .spacing(theme::GAP_SM)
            .align_y(iced::Alignment::Center),
    )
    .width(Length::Fill)
    .padding(theme::pad(theme::GAP_XS, theme::GAP_SM))
    .style(move |_theme, status| {
        theme::track_row(
            room,
            if hot { button::Status::Hovered } else { status },
            false,
        )
    })
    .on_press(Message::OpenPlaylist(entry.id));
    if receiving {
        // The drop target's ears: enter/exit report the held pointer, and
        // the release falls through to the drag source's own commit —
        // nothing here captures it (`mouse_area` publishes without
        // consuming when only enter/exit are wired).
        return mouse_area(door)
            .on_enter(Message::DragOverPanel(entry.id))
            .on_exit(Message::DragLeftPanel(entry.id))
            .into();
    }
    door.into()
}

/// No playlists yet: said plainly, with both doors in — the row below, and
/// the folder itself (the migration story is `cp *.m3u8`, ADR-0024).
fn empty_words(playlists: &Playlists) -> Element<'_, Message> {
    let room = theme::active();
    let words = if playlists.available() {
        "None yet. New playlist starts one, and .m3u8 files dropped into the playlists folder appear here."
    } else {
        "This system has no data directory for baz, so there is nowhere to keep playlists."
    };
    text(words)
        .size(theme::SIZE_META)
        .line_height(theme::LEADING_META)
        .color(room.paper_faint)
        .into()
}
