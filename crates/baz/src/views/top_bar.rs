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
/// Width reserved for the settings toggle (logical px) — deliberately
/// [`QUEUE_TOGGLE_W`], not a width fitted to `Settings`.
///
/// The two are a *pair* of view toggles sitting side by side, and a pair whose
/// halves were sized differently would read as two unrelated controls. Equal
/// widths also leave room for the longer word without it wrapping, which a
/// snug fit does not.
const SETTINGS_TOGGLE_W: f32 = QUEUE_TOGGLE_W;

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
        // Neither the accent nor the mono. A scan is the library working, not
        // the music — the lamp means playback truth (`theme`'s
        // accent-discipline note) and this note used to light it while nothing
        // was playing. It loses the monospace face with it, because the mono
        // is baz's tabular figures and this is a sentence fragment, not a
        // figure: set beside the counts it shares a line with, it should read
        // as prose next to numbers rather than as another readout.
        status = status.push(
            text("scanning…")
                .size(theme::SIZE_META)
                .color(theme::PAPER_DIM),
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
    status = status
        .push(queue_toggle(shelf, player))
        .push(settings_toggle(shelf));
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

/// The settings toggle: the one on-screen route to the settings panel, and
/// the only place in the interface that says baz has settings at all.
///
/// It sits **beside the Queue toggle**, in the top bar, and that adjacency is
/// the decision. Both are view controls — they say what the rail shows — and
/// the top bar is where view controls live alongside search and the counts;
/// the bottom bar is the transport, every pixel of it reserved so that nothing
/// moves as the music does, and the same promise the queue toggle kept ("that
/// row was not touched to add this feature") is kept again here.
///
/// A word rather than a gear. baz draws its glyphs itself
/// ([`crate::icon`]) from a small, deliberate set, and a cog would be a new
/// one for a control that has a short and unambiguous name — while "Settings"
/// beside "Queue" reads immediately as one pair of things the rail can show.
fn settings_toggle(shelf: &Shelf) -> Element<'_, Message> {
    let open = shelf.panels.rail() == Some(Rail::Settings);
    button(
        container(
            text("Settings")
                .size(theme::SIZE_META)
                .font(theme::MEDIUM)
                .wrapping(text::Wrapping::None),
        )
        .width(Length::Fill)
        .align_x(alignment::Horizontal::Center),
    )
    .width(Length::Fixed(SETTINGS_TOGGLE_W))
    .padding(theme::pad(theme::GAP_XS, theme::GAP_SM))
    .style(move |_theme, status| theme::panel_toggle(status, open))
    .on_press(Message::ToggleSettings)
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
