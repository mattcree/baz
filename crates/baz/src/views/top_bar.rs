//! The slim top bar: the search well on the left, quiet status and the queue
//! toggle on the right.

use iced::widget::{Space, button, column, container, horizontal_rule, row, text, text_input};
use iced::{Element, Length, alignment};

use crate::app::{Message, Shelf, search_id};
use crate::panels::Rail;
use crate::player::PlayerState;
use crate::theme;

/// The search field's width in the top bar (logical px).
const SEARCH_W: f32 = 360.0;
/// Width reserved for the queue toggle (logical px).
///
/// Fixed rather than sized to its label, because the label carries the queue's
/// length once there is one: a control that grew from `Queue` to `Queue · 12`
/// would drag the counts beside it sideways the moment somebody pressed play.
const QUEUE_TOGGLE_W: f32 = 92.0;

/// The slim top bar: the search well on the left, quiet status and the queue
/// toggle on the right, a hairline rule below.
pub(crate) fn view<'a>(shelf: &'a Shelf, player: &'a PlayerState) -> Element<'a, Message> {
    let search = text_input("Search artists, albums, tracks…", &shelf.query)
        .id(search_id())
        .on_input(Message::SearchChanged)
        .padding(theme::pad(theme::GAP_SM, theme::GAP_MD))
        .size(theme::SIZE_BODY)
        .width(Length::Fixed(SEARCH_W))
        .style(theme::input);
    let mut status = row![
        text(counts_line(shelf))
            .size(theme::SIZE_META)
            .font(theme::MONO)
            .color(theme::PAPER_FAINT)
    ]
    .spacing(theme::GAP_SM)
    .align_y(iced::Alignment::Center);
    if shelf.scanning {
        status = status.push(
            text("scanning…")
                .size(theme::SIZE_META)
                .font(theme::MONO)
                .color(theme::LAMP),
        );
    }
    if shelf.files_skipped > 0 {
        status = status.push(
            text(format!("{} files skipped", shelf.files_skipped))
                .size(theme::SIZE_META)
                .font(theme::MONO)
                .color(theme::PAPER_FAINT),
        );
    }
    if let Some(problem) = &shelf.problem {
        status = status.push(
            text(problem.as_str())
                .size(theme::SIZE_META)
                .color(theme::ALERT),
        );
    }
    status = status.push(queue_toggle(shelf, player));
    column![
        container(
            row![search, Space::with_width(Length::Fill), status]
                .spacing(theme::GAP_LG)
                .align_y(iced::Alignment::Center),
        )
        .padding(theme::pad(theme::GAP_SM + 2.0, theme::GAP_LG)),
        horizontal_rule(1).style(theme::hairline),
    ]
    .into()
}

/// The queue toggle: the one on-screen route to the queue panel, and the only
/// place in the interface that says a queue exists at all.
///
/// It lives in the **top bar**, not the bottom one. The bottom bar is the
/// transport — what is playing, where in it, how loud — and every pixel of it
/// is reserved so that nothing moves as the music does; a panel toggle is a
/// view control, which is the top bar's whole subject alongside search and the
/// counts. Putting it here also keeps the promise the bottom bar makes: that
/// row was not touched to add this feature.
///
/// The label carries the queue's length once there is one, so the count a
/// listener wants is legible without opening anything — and the control is
/// [`QUEUE_TOGGLE_W`] wide either way, so gaining it moves nothing.
fn queue_toggle<'a>(shelf: &'a Shelf, player: &'a PlayerState) -> Element<'a, Message> {
    let open = shelf.panels.rail() == Some(Rail::Queue);
    let queued = player.queued();
    let label = if queued > 0 {
        format!("Queue · {queued}")
    } else {
        "Queue".to_owned()
    };
    button(
        container(
            text(label)
                .size(theme::SIZE_META)
                .font(theme::MEDIUM)
                .wrapping(text::Wrapping::None),
        )
        .width(Length::Fill)
        .align_x(alignment::Horizontal::Center),
    )
    .width(Length::Fixed(QUEUE_TOGGLE_W))
    .padding(theme::pad(theme::GAP_XS, theme::GAP_SM))
    .style(move |_theme, status| theme::panel_toggle(status, open))
    .on_press(Message::ToggleQueue)
    .into()
}

/// The unobtrusive count text: album/track counts, or the filtered
/// count while a query narrows the shelf. Status, not modal — by
/// design; scan/skip/problem notes render as separate colored segments.
fn counts_line(shelf: &Shelf) -> String {
    if shelf.query.trim().is_empty() {
        format!(
            "{} albums · {} tracks",
            shelf.albums.len(),
            shelf.library.len()
        )
    } else {
        format!("{} / {} albums", shelf.visible.len(), shelf.albums.len())
    }
}
