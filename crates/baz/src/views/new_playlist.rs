//! The canonical, resumable Manual/Vibe playlist-creation place.
//!
//! # The flow was reviewed on 2026-08-15 and rebuilt
//!
//! The owner: *"we need to examine the flow for the vibe playlist. the ux is
//! terrible and it makes no sense right now."* Six things were wrong with it,
//! and each is answered here rather than in a note:
//!
//! 1. **Four names for one act.** He picked `Vibe`, a rule said `Make a mix`,
//!    a button said `Create mix`, the save said `Save playlist`, and Home's
//!    door said `Make a vibe playlist`. One vocabulary now: the place makes a
//!    **playlist**; the two ways in are **Manual** and **Vibe**; the Vibe
//!    route **composes**, and what it composes is a playlist you name and
//!    save.
//! 2. **The order was inverted.** `Shape the journey` — the energy shape and
//!    the waypoints, which exist to *inform* the request — stood below the
//!    button that spends the request, and `Save playlist` stood above the
//!    name field it needs. The form reads top to bottom now: describe, shape,
//!    compose, review, name, save.
//! 3. **The consent gate stood in the middle of the flow.** A first run was
//!    prompt → `Create mix` → a paragraph → a second, differently named
//!    button. The engine never needed two presses: `Message::VibeCreate`
//!    already starts the analysis and composes when it lands
//!    (`App`'s `VibePrepared`/`VibeAnalyzed` arms honour `awaiting_create`).
//!    So the paragraph moved **above** the press, where consent belongs, and
//!    the second button is gone.
//! 4. **Two first screens.** Home's shortcut opens this place with Vibe
//!    already chosen; the Playlists wall's ghost tile opens the fork. Both are
//!    kept — a shortcut that skips a fork is a shortcut — but they now land on
//!    the same drawing, with the same way back to the fork.
//! 5. **Manual and Vibe were not the same act twice.** Manual's rows were
//!    bare `Up | Down | Remove` word buttons with no artwork while Vibe's were
//!    `page::track_row` with the shared slots. Both draw [`draft_row`] now,
//!    and both hold `QueueItemVm`s, which is what made that possible.
//! 6. **The composer lived in `views::home`.** It had exactly one caller and
//!    it was this place. It lives here; Home keeps the door.

use std::path::Path;

use iced::widget::{Space, button, column, container, row, scrollable, text};
use iced::{Element, Length, alignment};

use crate::app::{Message, Shelf};
use crate::playlists::{CreationMode, Playlists};
use crate::vm::QueueItemVm;
use crate::{theme, views};

pub(crate) fn view<'a>(
    shelf: &'a Shelf,
    playlists: &'a Playlists,
    size: iced::Size,
) -> Element<'a, Message> {
    let width = size.width;
    let room = theme::active();
    let draft = &playlists.creation;
    // **The fork is gone, and the strip sentence with it.**
    //
    // *"Manual and Vibe become the same ordinary playlist"* was a sentence
    // explaining a fork; deleting the fork deletes its reason. The owner asked
    // for the sentence to go, and this is the way it goes that leaves nothing
    // behind — rather than removing the words and keeping the screen that made
    // them necessary.
    //
    // The page now **lands on composing**, which is the thing this place is
    // for, and *start with an empty list* is a quiet act inside it. A fork
    // asked every listener to classify themselves before seeing anything; this
    // asks nobody anything and takes one press either way.
    // The place names what it is making. A smart playlist is a different
    // thing from a hand-made one — it is composed from how the music sounds —
    // and a header that called both *New playlist* would be the fork's
    // ambiguity back without the fork.
    let header = views::place_header_with(
        match draft.mode {
            Some(CreationMode::Manual) => "New playlist",
            None | Some(CreationMode::Vibe) => "New smart playlist",
        },
        None,
    );
    let body: Element<'a, Message> = match draft.mode {
        Some(CreationMode::Manual) => manual_form(shelf, playlists, width),
        // The composing route is a place of its own now, not a section of this
        // form: two panes, its own states, its own readouts. It draws its own
        // scroller, so it returns before this one wraps the body.
        None | Some(CreationMode::Vibe) => {
            return column![header, views::compose::view(shelf, playlists, size)].into();
        }
    };
    column![
        header,
        scrollable(container(body).padding(views::place_pad()))
            .direction(scrollable::Direction::Vertical(theme::wall_scrollbar()))
            .style(move |_theme, status| theme::scrollbar(room, room.wall, status))
            .width(Length::Fill)
            .height(Length::Fill)
    ]
    .into()
}

/// **Manual**: name it, then add tracks to it from anywhere in the product.
fn manual_form<'a>(shelf: &'a Shelf, playlists: &'a Playlists, width: f32) -> Element<'a, Message> {
    let draft = &playlists.creation;
    let mut form = column![
        back_button(),
        views::caption_word("MANUAL"),
        named("PLAYLIST NAME", views::name_input(&draft.name)),
        views::hint(
            "Use the app-bar search and choose Add to playlist. Nothing is written until Save."
        ),
    ]
    .spacing(theme::GAP_SM);
    for (index, item) in draft.items.iter().enumerate() {
        form = form.push(
            iced::widget::mouse_area(draft_row(
                shelf,
                item,
                index,
                draft.items.len(),
                width,
                draft.hovered_row == Some(index),
                None,
                &DraftEdits {
                    shift: &|row, delta| Message::PlaylistCreationShift(row, delta),
                    remove: &Message::PlaylistCreationRemove,
                },
            ))
            .on_enter(Message::DraftRowEntered(index))
            .on_exit(Message::DraftRowLeft(index)),
        );
    }
    if let Some(reason) = playlists.creation_refusal() {
        form = form.push(views::alert(&reason));
    }
    form.push(action_button(
        "Save playlist",
        playlists
            .creation_can_save(false)
            .then_some(Message::PlaylistCreationSave),
    ))
    .into()
}

/// **One row of a draft list, drawn the same way in both routes.**
///
/// Manual's rows were bare `Up | Down | Remove` word buttons with no artwork
/// while Vibe's preview used the shared track row — two anatomies for one act,
/// in one place, three lines apart in the same file. Both hold
/// [`QueueItemVm`]s, so both draw this.
pub(crate) struct DraftEdits<'a> {
    pub(crate) shift: &'a dyn Fn(usize, i32) -> Message,
    pub(crate) remove: &'a dyn Fn(usize) -> Message,
}

/// `ticks` is the composing route's match strength — one, two or three, or
/// `None` where there is nothing to report. It is drawn as filled marks and
/// **never** as a colour: the standing rule in this product is that no reading
/// may rest on telling two hues apart, and this one carries its meaning in
/// count and in height.
#[expect(
    clippy::too_many_arguments,
    reason = "one row's whole anatomy: what it is, where it is, how wide, and its three states"
)]
pub(crate) fn draft_row<'a>(
    shelf: &'a Shelf,
    item: &'a QueueItemVm,
    position: usize,
    len: usize,
    width: f32,
    hovered: bool,
    ticks: Option<u8>,
    edits: &DraftEdits<'_>,
) -> Element<'a, Message> {
    let (shift, remove) = (edits.shift, edits.remove);
    let room = theme::active();
    let marker: Element<'a, Message> = text(format!("{:02}", position + 1))
        .size(theme::SIZE_META)
        .line_height(theme::LEADING_META)
        .color(room.paper_faint)
        .into();
    let under = item
        .artist
        .as_deref()
        .or(item.album_artist.as_deref())
        .map(|artist| (artist.into(), room.paper_dim, None));
    let context = item
        .album
        .as_deref()
        .map(|album| (album.into(), None, width >= theme::PLAYLIST_BREAKPOINT));
    let track = views::page::track_row(views::page::TrackRow {
        marker,
        artwork: None,
        title: item.title.as_str().into(),
        ink: room.paper,
        under,
        context,
        duration: item
            .duration
            .map(crate::vm::format_duration)
            .unwrap_or_default()
            .into(),
        playing: false,
        press: None,
    });
    let row = row![
        track,
        match_ticks(ticks),
        views::page::favourite_slot(&item.path, is_favourite(shelf, &item.path)),
        views::page::icon_slot(
            crate::icon::Glyph::ArrowUp,
            "Move up",
            position > 0,
            true,
            shift(position, -1),
        ),
        views::page::icon_slot(
            crate::icon::Glyph::ArrowDown,
            "Move down",
            position + 1 < len,
            true,
            shift(position, 1),
        ),
        views::page::icon_slot(
            crate::icon::Glyph::Close,
            "Remove",
            true,
            true,
            remove(position),
        ),
    ]
    .spacing(theme::GAP_XS)
    .align_y(iced::Alignment::Center);
    // The card reaches the row's editing controls rather than stopping at the
    // body, so a lit row is lit all the way across (item 53).
    views::page::row_card(hovered, false, false, row)
}

/// **Three ticks of match strength**, filled by how well the words answered.
///
/// Three buckets, so drift in the underlying cosines never changes the
/// picture, and the boundaries are the eligible pool's own terciles rather
/// than absolute numbers — measured in
/// `docs/design/impl/vibe-eligibility/`, finding 5. A weak tick at position
/// five is not a failure to hide: it says the line asked for something the
/// words did not have much of, which is true and useful.
fn match_ticks(ticks: Option<u8>) -> Element<'static, Message> {
    let room = theme::active();
    let Some(ticks) = ticks else {
        // A shape-only request has no match strength. The lane is still
        // reserved so rows do not reflow between one request and the next.
        return Space::new().width(Length::Fixed(theme::TICK_LANE_W)).into();
    };
    let mut marks = row![].spacing(2.0).align_y(iced::Alignment::End);
    for mark in 1..=3_u8 {
        let filled = mark <= ticks;
        marks = marks.push(
            container(Space::new())
                .width(Length::Fixed(theme::TICK_W))
                // Height as well as ink, so the reading survives being
                // printed, dimmed, or seen by somebody who cannot separate
                // the two inks at all.
                .height(Length::Fixed(
                    theme::TICK_H * (0.45 + 0.275 * f32::from(mark)),
                ))
                .style(move |_theme| iced::widget::container::Style {
                    background: Some(iced::Background::Color(if filled {
                        room.paper
                    } else {
                        room.paper_muted
                    })),
                    ..iced::widget::container::Style::default()
                }),
        );
    }
    container(marks)
        .width(Length::Fixed(theme::TICK_LANE_W))
        .height(Length::Fixed(theme::TICK_H))
        .align_y(alignment::Vertical::Center)
        .into()
}

fn is_favourite(shelf: &Shelf, path: &Path) -> bool {
    crate::app::is_favourite(shelf, path)
}

/// A caption over the field it names.
fn named<'a>(word: &str, field: Element<'a, Message>) -> Element<'a, Message> {
    column![views::caption_word(word), field]
        .spacing(theme::GAP_XS)
        .into()
}

fn back_button<'a>() -> Element<'a, Message> {
    action_button("Back to composing", Some(Message::PlaylistCreationBack)).into()
}

fn action_button(label: &str, message: Option<Message>) -> iced::widget::Button<'_, Message> {
    let room = theme::active();
    button(
        container(
            text(label)
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .font(theme::MEDIUM),
        )
        .height(Length::Fill)
        .align_y(alignment::Vertical::Center),
    )
    .padding(theme::pad(0.0, theme::GAP_SM))
    .height(Length::Fixed(theme::TRANSPORT_HIT))
    .style(move |_theme, status| theme::word_button(room, room.wall, status))
    .on_press_maybe(message)
}
