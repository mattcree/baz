//! The slim top bar: the search well on the left, quiet status and the route
//! to the settings on the right.
//!
//! # The bar has a subject again
//!
//! It used to carry four things and only two of them were about the library:
//! search and the counts *are*, `Queue` was about the engine, and `Settings` is
//! about the application. The audit's finding was that the bar therefore had no
//! subject (§1.4).
//!
//! `Queue` has gone. Its count now sits in the now-playing bar beside the track
//! it counts, and the queue itself opens from there
//! ([`crate::views::up_next`]) — closer to its subject, and no longer stale:
//! the toggle went on saying `Queue · 13` after the run had ended, because it
//! reported the length of the last queue rather than what was next.

use iced::widget::{Space, button, column, container, horizontal_rule, row, text, text_input};
use iced::{Element, Length, alignment};

use crate::app::{Message, Shelf, search_id};

use crate::theme;

/// The search field's width in the top bar (logical px).
const SEARCH_W: f32 = 360.0;
/// The slim top bar: the search well on the left, quiet status and the route
/// to the settings on the right, a hairline rule below.
pub(crate) fn view(shelf: &Shelf) -> Element<'_, Message> {
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
    status = status.push(settings_toggle());
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

/// The route to the Settings **place**, and the only place in the interface
/// that says baz has settings at all.
///
/// It sits at the far right of the top bar, which is where an application's own
/// affairs belong: the bottom bar is the transport, every pixel of it reserved
/// so that nothing moves as the music does, and it was not touched to put this
/// here.
///
/// It is **navigation** now, not a panel toggle, and it is drawn as such: no
/// "open" state, because the place it leads to fills the window and takes this
/// bar with it, so there is no frame in which the control could be lit and
/// visible at once. The same message <kbd>Ctrl</kbd>+<kbd>,</kbd> sends, and
/// the same one the Settings place's own Back sends.
///
/// A word rather than a gear. baz draws its glyphs itself ([`crate::icon`])
/// from a small, deliberate set, and a cog would be a new one for a control
/// that has a short and unambiguous name.
fn settings_toggle() -> Element<'static, Message> {
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
    .width(Length::Fixed(theme::SETTINGS_TOGGLE_W))
    .padding(theme::pad(theme::GAP_XS, theme::GAP_SM))
    .style(|_theme, status| theme::panel_toggle(status, false))
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
