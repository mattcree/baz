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
//!
//! # The group keys sit here, beside the well
//!
//! ARTIST · YEAR · GENRE · ADDED · PLAYED (ADR-0019) are the bar's third
//! tenant, and they belong to its subject: search narrows the collection and
//! the keys arrange it, so both are about the library and both are on the left.
//! The application's own affairs — the counts it can report, the route to the
//! Settings — stay on the right. See [`group_key`] for why the row is five
//! words and not a menu.

use baz_core::index::GroupKey;
use iced::widget::{Space, button, column, container, horizontal_rule, row, text, text_input};
use iced::{Element, Length, alignment};

use crate::app::{Message, Shelf, search_id};

use crate::theme;

/// The search field's width in the top bar (logical px).
pub(crate) const SEARCH_W: f32 = 360.0;

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
    let room = theme::active();
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
        .style(move |_theme, status| theme::input(room, status));
    let mut keys = row![]
        .spacing(theme::GAP_MD)
        .align_y(iced::Alignment::Center);
    for key in GroupKey::ALL {
        keys = keys.push(group_key(key, key == shelf.group_key));
    }
    let mut status = row![
        text(counts_line(shelf))
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            .color(room.paper_faint)
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
                .color(room.paper_dim),
        );
    }
    if shelf.files_skipped > 0 {
        status = status.push(
            text(format!("{} files skipped", shelf.files_skipped))
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.paper_faint),
        );
    }
    if let Some(problem) = &shelf.problem {
        status = status.push(
            text(problem.as_str())
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.alert),
        );
    }
    status = status.push(settings_toggle());
    column![
        container(
            row![
                // The well and the keys are one cluster — both are about the
                // library — held apart by the ladder's largest gap so they
                // read as two groups on one line rather than as six controls.
                row![search, keys]
                    .spacing(theme::GAP_XL)
                    .align_y(iced::Alignment::Center),
                Space::with_width(Length::Fill),
                status
            ]
            .spacing(theme::GAP_LG)
            .align_y(iced::Alignment::Center),
        )
        .padding(theme::pad(theme::GAP_SM + 2.0, theme::GAP_LG)),
        horizontal_rule(1).style(move |_theme| theme::hairline(room, room.wall)),
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
    let room = theme::active();
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
    .style(move |_theme, status| theme::word_button(room, room.wall, status))
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

/// One of the five words the wall is arranged by.
///
/// # The seam this closes
///
/// This file used to carry a note saying the row was deliberately *not* wired,
/// because "five words that do nothing would be a lie". The rest of step 8
/// has landed — the active key on `Shelf`, persisted in `config.rs`, the wall
/// rebuilt as [`crate::shelf::Shelves`], the sticky headers in the virtualizer
/// and the rail down the wall's right edge — so the words now do the thing
/// they name.
///
/// # It is a word, and nothing else
///
/// No menu, no dropdown, no segmented control, no chip around the live one.
/// `docs/REFUSALS.md` refuses view-options menus by name, and a pill drawn
/// around the active key would be the dropdown's ghost — the same "this is a
/// widget" statement one step quieter. What says *active* is
/// [`theme::group_key`]: full paper in the Medium face against
/// [`theme::Palette::paper_faint`] in Regular. Two axes, neither of them
/// colour and neither of them size.
///
/// # The type is the shelf headers' type
///
/// Caps, tracked ([`theme::tracked`]), at the metadata size (12) — the same
/// vocabulary the shelf headers use at the heading size (10), one step
/// larger. That is the whole hierarchy of the wall's chrome in two sizes of
/// one voice: **the key names the arrangement, the header names a break in
/// it.** A third size, or a second face, would make them two systems.
///
/// The word is a `button`, so the keyboard's `1`–`5` ([`crate::keys`]) and
/// this control send the identical message and the visible-control rule holds.
fn group_key(key: GroupKey, active: bool) -> Element<'static, Message> {
    let room = theme::active();
    button(
        text(theme::tracked(&key.label().to_uppercase()))
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            .font(if active { theme::MEDIUM } else { theme::SANS })
            .wrapping(text::Wrapping::None),
    )
    // The same 32 px as the well beside it and every other control in the
    // product, so the bar's clusters sit on one grid rather than on one centre
    // line. Horizontal padding is `GAP_XS` — enough that the hover wash is not
    // tight against the glyphs, small enough that five of them stay one line of
    // type rather than five boxes.
    .height(Length::Fixed(theme::TRANSPORT_HIT))
    .padding(theme::pad(0.0, theme::GAP_XS))
    .style(move |_theme, status| theme::group_key(room, room.wall, status, active))
    .on_press(Message::GroupKeySelected(key))
    .into()
}
