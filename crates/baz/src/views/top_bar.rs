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
//! ([`crate::views::queue`]) — closer to its subject, and no longer stale:
//! the toggle went on saying `Queue · 13` after the run had ended, because it
//! reported the length of the last queue rather than what was next.

use iced::widget::{Space, button, column, container, horizontal_rule, row, text, text_input};
use iced::{Element, Length, alignment};

use crate::app::{Message, Shelf, search_id};

use crate::theme;

/// The search field's width in the top bar (logical px).
const SEARCH_W: f32 = 360.0;

/// The search well's vertical padding (logical px), derived so the well is
/// exactly [`theme::TRANSPORT_HIT`] tall.
///
/// iced lays a `text_input` out as its padding plus one line box plus its 1 px
/// border on each side, so the padding is the height minus the parts that are
/// not it, halved. It came to 8, which made the well 34 px against a 32 px
/// control everywhere else in the product — two pixels, on the one row where
/// the app's two clusters have to look like they were placed by the same hand.
const SEARCH_PAD_V: f32 =
    (theme::TRANSPORT_HIT - theme::SIZE_BODY * theme::LEADING_BODY - 2.0) / 2.0;
/// The slim top bar: the search well on the left, quiet status and the route
/// to the settings on the right, a hairline rule below.
pub(crate) fn view(shelf: &Shelf) -> Element<'_, Message> {
    // The search **well**: recessed below the wall, like every other place in
    // baz you put something into. Its vertical padding is set so that the well
    // stands [`theme::TRANSPORT_HIT`] tall — the same 32 px as every control in
    // the product — which is what puts the bar's left and right clusters on one
    // vertical grid instead of merely on one centre line.
    let search = text_input("Search artists, albums, tracks…", &shelf.query)
        .id(search_id())
        .on_input(Message::SearchChanged)
        .padding(theme::pad(SEARCH_PAD_V, theme::GAP_MD))
        .size(theme::SIZE_BODY)
        .line_height(theme::LEADING_BODY)
        .width(Length::Fixed(SEARCH_W))
        .style(theme::input);
    let mut status = row![
        text(counts_line(shelf))
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            .color(theme::PAPER_FAINT)
    ]
    .spacing(theme::GAP_SM)
    .align_y(iced::Alignment::Center);
    if shelf.scanning {
        // Not the accent. A scan is the library working, not the music — the
        // lamp means playback truth (`theme`'s accent-discipline note) and this
        // note used to light it while nothing was playing.
        status = status.push(
            text("scanning…")
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(theme::PAPER_DIM),
        );
    }
    if shelf.files_skipped > 0 {
        status = status.push(
            text(format!("{} files skipped", shelf.files_skipped))
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(theme::PAPER_FAINT),
        );
    }
    if let Some(problem) = &shelf.problem {
        status = status.push(
            text(problem.as_str())
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
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
                .line_height(theme::LEADING_META)
                .font(theme::MEDIUM)
                .wrapping(text::Wrapping::None),
        )
        .width(Length::Fill)
        .align_x(alignment::Horizontal::Center),
    )
    .width(Length::Fixed(theme::SETTINGS_TOGGLE_W))
    // The same 32 px as the search well beside it and as every control in the
    // product: a row whose two ends are 34 px and 24 px tall is centred but not
    // aligned, and the difference is exactly what "clunky" describes.
    .height(Length::Fixed(theme::TRANSPORT_HIT))
    .padding(theme::pad(0.0, theme::GAP_SM))
    .style(|_theme, status| theme::word_button(status))
    .on_press(Message::ToggleSettings)
    .into()
}

/// The unobtrusive count text: album/track counts, or the filtered
/// count while a query narrows the shelf. Status, not modal — by
/// design; scan/skip/problem notes render as separate colored segments.
///
/// The filtered form leads with the **match count**, because that is the number
/// a listener typing into the well is watching: `7 of 1 284 albums` reads as an
/// answer to the query, where the unfiltered form is a description of the
/// collection. Both end in the same word so the line does not change shape as
/// the query empties.
fn counts_line(shelf: &Shelf) -> String {
    if shelf.query.trim().is_empty() {
        format!(
            "{} albums · {} tracks",
            shelf.albums.len(),
            shelf.library.len()
        )
    } else {
        format!("{} of {} albums", shelf.visible.len(), shelf.albums.len())
    }
}

// # The group-key row is **not** wired here, and that is deliberate
//
// `Library::shelves(key)` and `GroupKey` landed in `baz-core` (ADR-0019) and
// nothing in this crate calls either yet. The row of words — ARTIST · YEAR ·
// GENRE · ADDED · PLAYED — belongs in this bar, and drawing it is the easy
// tenth of the work: the rest is the active key in `Shelf`, persisted in
// `config.rs`, the shelf rebuilt as a list of *groups* rather than a flat
// vector, sticky headers inside the virtualizer's row arithmetic, and the index
// rail projecting the same keys down the shelf's right edge. That is ADR-0017
// step 8, it lands squarely on top of `shelf.rs`'s geometry, and a parallel
// pass owns that file for the hang.
//
// So the seam is left rather than half-taken. Drawing five words that do
// nothing would be worse than drawing none: *an affordance that does nothing is
// a lie*, and `docs/REFUSALS.md` puts every action behind a visible control
// precisely so that the reverse cannot happen either. When the key arrives it
// arrives as one `row!` of `word_button`s here, one field on `Shelf`, and the
// grouping in `vm`.
