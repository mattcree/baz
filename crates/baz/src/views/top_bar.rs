//! The slim Library strip: **how the wall is arranged**.
//!
//! Search is resident in the app bar (ADR-0040's 2026-08-12 amendment), so
//! this strip has one vocabulary and one subject: the arrangement states —
//! A–Z · ARTIST · YEAR · GENRE · ADDED · PLAYED, caps and tracked, one of
//! them current.
//!
//! # Two more tenants left on 2026-08-10, and neither was relocated here
//!
//! `Play all` **went**, on the owner's *"please remove the 'Play all' button
//! at the top of the library"*. It is not elsewhere: it was the wall's one
//! act, the owner asked for it to go, and a verb quietly re-homed into the
//! window's chrome would have been the removal not done.
//!
//! The **gear** went *up*, into [`crate::views::app_bar`] — the same control,
//! the same message, the same corner, now resident in all eight places rather
//! than only this one. That is what "the application's affair rather than the
//! frame's" always implied and what a Library-only strip could not deliver.
//!
//! Both departures make this strip **smaller**, which is the point worth
//! stating against the owner's standing complaint (*"just adding stuff into
//! that top bar isn't good"*, 2026-08-09): the answer to a crowded strip was
//! not a better arrangement of its tenants but two fewer of them.
//!
use baz_core::index::GroupKey;
use iced::widget::{Space, column, container, row, rule, text};
use iced::{Element, Length, alignment};

use crate::app::{Message, Shelf};

use crate::theme;

/// The arrangement row's reserved width (logical px): **six** tracked caps
/// words in their `GAP_XS`-padded buttons with `GAP_MD` between them, measured
/// in `font.rs` against this declaration — L9's arithmetic needs every tenant
/// to *declare*, and the declaration is only worth asserting if the face is
/// measured against it.
///
/// **314 → 360**, and the number was measured rather than reused. The row has
/// been six words before: `ARTISTS` after `PLAYED` came to 366.50 and a
/// declaration of 368. This sixth word is `A–Z` and it goes **first**
/// (ADR-0035's third amendment), so the price is different — `A–Z` is 32.92 px
/// in its box against `ARTISTS`'s 65.49 — and the row measures 357.91. The
/// declaration is the next 4 px lattice step above it, which leaves 2.09 px of
/// slack; the earlier costing's 368 would have been 10 px of reservation for a
/// word that is not there.
pub(crate) const KEYS_W: f32 = 360.0;

// Kept only for the historical strip-budget audit in `theme.rs`; no well is
// rendered by this module any more.
#[cfg(test)]
pub(crate) const WELL_W: f32 = 200.0;

/// The Library arrangement strip: one control line and its lower hairline at
/// every supported width. `strip_width` remains in the call shape because the
/// surrounding composition owns that measurement; this view no longer needs
/// a responsive branch after search moved to the app bar.
pub(crate) fn view(shelf: &Shelf, _strip_width: f32) -> Element<'_, Message> {
    let room = theme::active();
    let mut keys_row = row![]
        .spacing(theme::GAP_MD)
        .align_y(iced::Alignment::Center);
    for key in GroupKey::ALL {
        keys_row = keys_row.push(group_key(key, key == shelf.group_key));
    }
    let keys = container(keys_row)
        .width(Length::Fixed(KEYS_W))
        .align_x(alignment::Horizontal::Left);
    let mut status = row![]
        .spacing(theme::GAP_SM)
        .align_y(iced::Alignment::Center);
    if shelf.scanning {
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
    column![
        container(
            row![keys, Space::new().width(Length::Fill), status]
                .spacing(theme::GAP_SM)
                .align_y(iced::Alignment::Center),
        )
        .padding(theme::pad(theme::TOP_BAR_PAD_V, theme::HANG)),
        rule::horizontal(1).style(move |_theme| theme::hairline(room, room.wall)),
    ]
    .into()
}

/// One of the six words the **records** are arranged by.
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
/// the product's standing rules refuses view-options menus by name, and a pill drawn
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
/// The word is a `button`, so the keyboard's `1`–`6` ([`crate::keys`]) and
/// this control send the identical message and the visible-control rule holds.
///
/// **Exactly one of the six is current, always** — the wall is arranged some
/// way or it is not drawn. The first two name the same order at two densities
/// (ADR-0035's third amendment), which is a difference the row states by
/// putting them side by side rather than by explaining it.
fn group_key(key: GroupKey, active: bool) -> Element<'static, Message> {
    crate::views::arrangement_key(key.label(), active, Message::GroupKeySelected(key))
}

#[cfg(test)]
mod tests {
    /// This file's own source — the only way to assert a claim about what is
    /// *drawn* in a toolkit whose widget tree cannot be walked, and the idiom
    /// `views::lane` and `app.rs` already use for exactly this.
    fn source() -> String {
        include_str!("top_bar.rs").replace("\r\n", "\n")
    }

    /// One function's body, by name.
    fn body(source: &str, signature: &str) -> String {
        let rest = source
            .split_once(signature)
            .unwrap_or_else(|| panic!("`{signature}` exists"))
            .1;
        rest[..rest.find("\n}\n").expect("a function ends")].to_owned()
    }

    /// **The arrangement row is six words and every one of them is a key.**
    ///
    /// It carried a sixth word once before — `ARTISTS`, the wall's other
    /// subject — drawn in the keys' exact voice so the row read as one closed
    /// set, and that word was not a key. `A–Z` is (ADR-0035's third
    /// amendment), so the row is `GroupKey::ALL` and nothing else: there is no
    /// word here that is not a key, and therefore nothing that has to be
    /// argued into the keys' voice. **The count is not asserted** — the row is
    /// a walk of the library's own array, so a seventh key would join it here
    /// without an edit, and pinning the number would be pinning `baz-core`'s
    /// list from the wrong side.
    #[test]
    fn the_arrangement_row_is_the_group_keys_and_nothing_else() {
        let source = source();
        let view = body(
            &source,
            "pub(crate) fn view(shelf: &Shelf, _strip_width: f32)",
        );
        assert!(
            view.contains("for key in GroupKey::ALL {")
                && view.contains("group_key(key, key == shelf.group_key)"),
            "the row is no longer a walk of the library's own keys"
        );
        assert!(
            !view.contains("keys_row.push(")
                || view.matches("keys_row = keys_row.push(").count() == 1,
            "something other than the key walk is pushed into the row"
        );
    }
}
