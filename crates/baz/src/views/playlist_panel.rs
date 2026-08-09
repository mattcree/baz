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
//! `New playlist`, then one row per playlist — the name (a door to
//! [`Place::Playlist`](crate::place::Place)) and the receive target that arms
//! it for adding (ADR-0024 §6 layer 2). Rename and delete are *not* here:
//! destruction lives on the page, where the contents are visible at the
//! moment of the decision. While a layer-1 pick is in flight the whole row
//! becomes the pick's target and says so — the panel is the picker.

use iced::widget::{Column, Space, button, column, container, row, scrollable, text, text_input};
use iced::{Element, Length, alignment};

use crate::app::Message;
use crate::playlists::{NameEntry, PanelRow, Playlists};
use crate::theme;

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
pub(crate) fn view(playlists: &Playlists) -> Element<'_, Message> {
    let room = theme::active();
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
            text(format!("{} — pick a playlist", pending.label))
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.paper_dim),
        );
    }
    body = body.push(match &playlists.naming {
        None => new_playlist_door(),
        Some(entry) => name_field(entry),
    });
    let rows: Element<'_, Message> = if playlists.rows.is_empty() {
        empty_words(playlists)
    } else {
        let picking = playlists.pending.is_some();
        let mut listed: Vec<Element<'_, Message>> = Vec::new();
        for entry in &playlists.rows {
            listed.push(playlist_row(
                entry,
                playlists.armed == Some(entry.id),
                picking,
            ));
        }
        scrollable(Column::with_children(listed).spacing(theme::GAP_XS))
            .direction(scrollable::Direction::Vertical(theme::list_scrollbar()))
            .style(move |_theme, status| theme::scrollbar(room, room.plinth, status))
            .height(Length::Fill)
            .into()
    };
    body = body.push(rows);
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

/// One playlist's row: the name (a door to its page) over its counts, and the
/// receive target beside them.
///
/// While a pick is in flight the whole row is the pick's target instead —
/// pressing anywhere on it appends what the hand holds — because a picker
/// whose rows kept their two ordinary meanings would make the most important
/// press on the surface the hardest to aim.
fn playlist_row(entry: &PanelRow, armed: bool, picking: bool) -> Element<'_, Message> {
    let room = theme::active();
    let name_block = column![
        text(entry.name.clone())
            .size(theme::SIZE_BODY)
            .line_height(theme::LEADING_BODY)
            .font(theme::MEDIUM)
            .color(room.paper)
            .wrapping(text::Wrapping::None),
        text(entry.counts())
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            .color(room.paper_faint)
            .wrapping(text::Wrapping::None),
    ]
    .spacing(theme::GAP_XXS);
    if picking {
        // The pick's target: one press, one append, and the row says so.
        return container(
            button(
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
            .on_press(Message::PickPlaylist(entry.id)),
        )
        .style(move |_theme| theme::panel_row(room, false))
        .into();
    }
    let door = button(name_block)
        .width(Length::Fill)
        .padding(theme::pad(theme::GAP_XS, theme::GAP_SM))
        .style(move |_theme, status| theme::track_row(room, status, false))
        .on_press(Message::OpenPlaylist(entry.id));
    // The receive target (ADR-0024 §6 layer 2): a `+` that arms this list to
    // collect, or puts it down when it is the one armed. `STEPPER_HIT`, the
    // room's secondary square, and never the accent — arming is a collecting
    // state, not playback truth.
    let receive = iced::widget::tooltip(
        button(
            container(
                text(if armed { "\u{2212}" } else { "+" })
                    .size(theme::SIZE_BODY)
                    .line_height(theme::LEADING_BODY)
                    .color(room.paper),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(alignment::Horizontal::Center)
            .align_y(alignment::Vertical::Center),
        )
        .width(Length::Fixed(theme::STEPPER_HIT))
        .height(Length::Fixed(theme::STEPPER_HIT))
        .padding(0)
        .style(move |_theme, status| theme::transport(room, room.plinth, status))
        .on_press(Message::ArmPlaylist(entry.id)),
        text(if armed {
            "Stop adding"
        } else {
            "Open for adding: one press per record or track"
        })
        .size(theme::SIZE_CAPTION)
        .line_height(theme::LEADING_CAPTION),
        iced::widget::tooltip::Position::Left,
    )
    .gap(theme::GAP_XS)
    .padding(theme::GAP_XS)
    .style(move |_theme| theme::tooltip(room));
    container(
        row![door, receive]
            .spacing(theme::GAP_XS)
            .align_y(iced::Alignment::Center),
    )
    .padding(theme::pad(0.0, 0.0))
    .style(move |_theme| theme::panel_row(room, armed))
    .into()
}

/// No playlists yet: said plainly, with both doors in — the row above, and
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
