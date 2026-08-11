//! The slim Library strip: **how the wall is arranged**.
//!
//! # What it is now, after the well left
//!
//! The audit's §1.4 finding was that this bar had no subject: it carried
//! search, the counts, `Queue` and `Settings`, and only the first two were
//! about the library. Three of those four have since gone somewhere truer.
//! `Queue`'s count went to the now-playing bar beside the track it counts. The
//! `Playlists` door went when the returns lane became the resident index
//! (ADR-0030 §5). And the **well and the counts have gone to the lane**, on
//! the owner's decision — *"the design does not match properly… the search
//! should really be in the sidebar"* — because the query is the frame's state
//! and the frame's resident surface is the lane.
//!
//! What is left, since ADR-0040, is **one vocabulary and one subject**: the
//! states — A–Z · ARTIST · YEAR · GENRE · ADDED · PLAYED, caps and tracked,
//! one of them current. Narrow-then-arrange used to read left to right across
//! this strip; the narrowing is in the lane now and the strip is the
//! arrangement and nothing else.
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
//! # The well is still drawn here at the widths the lane cannot hold it
//!
//! [`theme::strip_holds_the_well`] is the one predicate: below
//! [`theme::SIDEBAR_FLOOR`] the lane is a rail that cannot open, so the well
//! comes back to the strip's frame line in the exact form doc 10 §4.1 drew for
//! it — the counts as the placeholder, the match count in its reserved
//! [`MATCH_W`] slot. **Two forms, never two at once**, and the breakpoint is
//! the lane's own floor rather than a second one this file gets to choose.

use baz_core::index::GroupKey;
use iced::widget::{
    Space, column, container, horizontal_rule, image as iced_image, row, stack, text, text_input,
};
use iced::{Element, Length, alignment};

use crate::app::{Message, Shelf, search_id};

use crate::{icon, theme};

/// The search well's width in the strip (logical px) — **200**, flat.
///
/// **The fluid range is gone, and it is gone because it is unreachable.** It
/// was `clamp(W − 1000, 200, 280)`: 280 at the shipped window, spending 80 px
/// down to 1200, then holding the floor — the strip's one fluid tenant, and
/// the reason doc 10 §4.3 could call the collapse order *one step then the
/// split*. The strip now carries the well only below [`theme::SIDEBAR_FLOOR`]
/// ([`theme::strip_holds_the_well`]), where the strip's own width is at most
/// `SIDEBAR_FLOOR − SIDEBAR_RAIL_W` = 904 and the clamp returns its floor at
/// every one of those widths. A ramp no width can climb is a comment that can
/// rot, so the constant states the number the strip actually draws and **the
/// split is now the whole of the collapse order**.
pub(crate) const WELL_W: f32 = 200.0;

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

/// The slim Library strip — one line at [`theme::TOP_BAR_SPLIT`] and above,
/// two below it, a hairline rule under either.
///
/// `strip_width` is the **strip's** width — the window less the returns lane,
/// `App::body_width` — never the window's, because the lane is a column and a
/// strip that resolved its split against the window would split at the wrong
/// moment. It decides which regime the strip is in and nothing else; whether
/// the well is a tenant at all is [`theme::strip_holds_the_well`]'s answer,
/// read off `shelf.window_w`.
///
/// **The split is the charter drawn** (doc 10 §4.3), as ADR-0040 leaves it:
/// below [`theme::TOP_BAR_SPLIT`] the frame's furniture — the well, and the
/// transient notes beside it — stays on the window line, and the library's
/// states take a line of their own. Nothing hides, nothing overflows, no menu appears; every control
/// keeps its exact form. It can only be reached where the well is a tenant:
/// once the well is in the lane the strip's tenants sum to 608 against a
/// narrowest possible strip of 720, asserted in `theme.rs`. Below
/// [`theme::TOP_BAR_FLOOR`] nothing further collapses: 600 is the strip's
/// declared floor.
///
/// The resolved height is [`theme::top_bar_h`], and `app.rs`'s viewport
/// estimate reads the same function — the pair of tokens and the breakpoint
/// are one decision, not two that must agree.
pub(crate) fn view(shelf: &Shelf, strip_width: f32) -> Element<'_, Message> {
    let room = theme::active();
    let holds_well = theme::strip_holds_the_well(shelf.window_w);
    let mut keys_row = row![]
        .spacing(theme::GAP_MD)
        .align_y(iced::Alignment::Center);
    for key in GroupKey::ALL {
        keys_row = keys_row.push(group_key(key, key == shelf.group_key));
    }
    // **Every tenant stands in its reserved width** (L9): the clusters are
    // fixed-width slots — `font.rs` measures the words against the
    // reservations — so the budget the law adds up is the geometry actually
    // drawn, and the left cluster's landmarks survive a resize (doc 10
    // §4.2: at 1440 and 1920 the keys do not move relative to the well;
    // slack is air, not drift).
    let keys = container(keys_row)
        .width(Length::Fixed(KEYS_W))
        .align_x(alignment::Horizontal::Left);
    // The status row holds only the transient notes — the counts went with
    // the well, first into it (doc 10 §4.1) and now into the lane's own
    // readout line, which is L8.3's valve applied twice in the same direction:
    // the fact goes to where it is watched.
    let mut status = row![]
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
    if holds_well && strip_width < theme::TOP_BAR_SPLIT {
        // **The two-line regime** (doc 10 §4.3), and it is reachable only in
        // this branch's own condition: the split exists to give the *library*
        // line its 600 px, and the frame line it splits away is the well's.
        // With the well in the lane the strip's remaining tenants sum to 608
        // against a strip that is never narrower than 720, so there is nothing
        // to split — asserted in `theme.rs`, not assumed here.
        //
        // The frame line: the well, the transient notes in the slack, then the
        // notes in the slack. The library line: the arrangement's states.
        // The seam is the charter's own division — frame furniture above, the
        // library's own words below — and both lines keep every control at
        // its exact single-line form.
        let frame_line = row![well(shelf, WELL_W), Space::with_width(Length::Fill), status]
            .spacing(theme::GAP_LG)
            .align_y(iced::Alignment::Center);
        let library_line = row![keys]
            .spacing(theme::GAP_XL)
            .align_y(iced::Alignment::Center);
        return column![
            container(
                // One lead above, between and below the two lines — the
                // strip's own `TOP_BAR_PAD_V`, three times — which is the
                // whole of `TOP_BAR_2LINE_H`'s arithmetic: 8+32+8+32+8+1.
                column![frame_line, library_line].spacing(theme::TOP_BAR_PAD_V)
            )
            .padding(theme::pad(theme::TOP_BAR_PAD_V, theme::HANG)),
            horizontal_rule(1).style(move |_theme| theme::hairline(room, room.wall)),
        ]
        .into();
    }
    // **The left cluster**, in the order the gestures happen: narrow, arrange,
    // then play or draw. The narrowing is in the lane at every width the lane
    // can hold it, so at those widths the cluster begins at the arrangement
    // and the strip hangs from `HANG` with a group key rather than a field.
    let mut cluster = row![]
        .spacing(theme::GAP_XL)
        .align_y(iced::Alignment::Center);
    if holds_well {
        cluster = cluster.push(well(shelf, WELL_W));
    }
    column![
        container(
            row![
                // Held apart by the ladder's largest gap so they read as two
                // groups on one line rather than as ten controls.
                cluster.push(keys),
                // The strip's one flexible region. The row's `GAP_SM` counts
                // once each side of it, which is the 16 px status lead the
                // budget arithmetic reserves (doc 10 §4.2) — at the regime
                // floor the fill is zero and the lead is what remains.
                Space::with_width(Length::Fill),
                status
            ]
            .spacing(theme::GAP_SM)
            .align_y(iced::Alignment::Center),
        )
        // **One window gutter.** The bar hung from `GAP_LG` while the wall under
        // it hung from `HANG`, so nothing in the chrome lined up with anything
        // in the collection — the composition audit's defect 1, and the single
        // highest-yield line in this file. The vertical padding is
        // [`theme::TOP_BAR_PAD_V`], which makes the strip
        // [`theme::TOP_BAR_H`] and puts `app.rs`'s virtualizer estimate on the
        // same number the bar is actually drawn at.
        .padding(theme::pad(theme::TOP_BAR_PAD_V, theme::HANG)),
        horizontal_rule(1).style(move |_theme| theme::hairline(room, room.wall)),
    ]
    .into()
}

/// **The search well, in the strip's own form** — drawn only where the
/// returns lane cannot hold it ([`theme::strip_holds_the_well`]); the lane's
/// form is [`crate::views::lane`]'s `well`, which is where the query lives at
/// every width above [`theme::SIDEBAR_FLOOR`].
///
/// Both forms share the id, the messages and the mark. What differs is where
/// the two figures go: at 232 px the lane puts them on a line under the field,
/// and at 200 px the strip keeps them *inside* it — the counts as the
/// placeholder, the match count in the reserved [`MATCH_W`] slot — because a
/// strip is one control tall and has no second line to give.
///
/// The magnifier is laid over its left padding
/// (doc 10 §4.1): recessed below the wall, like every other place in baz you
/// put something into. Its vertical padding is set so that the well stands
/// [`theme::TRANSPORT_HIT`] tall — the same 32 px as every control in the
/// product — which is what puts the bar's left and right clusters on one
/// vertical grid instead of merely on one centre line.
///
/// # The mark's box holds the label at rest and the clear control under a query
///
/// The universal mark, in the universal corner — the second of the two
/// symbols L8.4's amendment admits as door labels (doc 10 §3.4) — drawn as a
/// **layer** over the input (the mechanism the bar's tip layers already use;
/// iced 0.13's `text_input::Icon` is font-based and therefore not it). At rest
/// it answers nothing: the well remains the only focusable widget
/// (ADR-0017 §1.2), the glyph takes the resting glyph ink, and the input's own
/// left padding reserves the lane the glyph sits in, so the caret and the mark
/// cannot collide.
///
/// **While a query stands the box holds the `×` instead** (ADR-0036 §4) — one
/// box, two meanings, no reflow, and the right-hand furniture untouched because
/// the count's [`MATCH_W`] slot is already the whole of what the field's right
/// edge can spend. `crate::views::clear_mark` draws it for both wells.
///
/// # `on_submit` is what makes Enter mean one thing
///
/// With the well focused iced 0.13's `text_input` consumes <kbd>Enter</kbd>
/// and publishes this; with the well unfocused `crate::keys` binds the same
/// message. Both roads are [`Message::PlayFirstMatch`], so a listener who
/// typed from the wall and one who clicked into the well get the same record
/// (ADR-0017 §1.2, ADR-0021).
fn well(shelf: &Shelf, width: f32) -> Element<'_, Message> {
    let room = theme::active();
    let filtering = !shelf.query.trim().is_empty();
    // **The counts are the placeholder** (doc 10 §4.1): the placeholder lane
    // is by definition empty exactly when the query is — the one lane in the
    // product that is free whenever the counts have something to say. During
    // a scan the figure ticks up, which is the shelf-filling progress rule
    // (the product's standing rules) restated in figures.
    let input = text_input(&resting_counts(shelf), &shelf.query)
        .id(search_id())
        .on_input(Message::SearchChanged)
        .on_submit(Message::PlayFirstMatch)
        .padding(iced::Padding {
            top: theme::WELL_PAD_V,
            // While a query narrows the shelf, the match count holds a
            // reserved [`MATCH_W`] slot at the well's right edge, and the
            // input's own padding keeps the caret out of it. At rest the
            // lane is the placeholder's.
            right: if filtering {
                theme::GAP_MD + MATCH_W
            } else {
                theme::GAP_MD
            },
            bottom: theme::WELL_PAD_V,
            left: theme::GAP_MD + theme::ICON_PX + theme::GAP_SM,
        })
        .size(theme::SIZE_BODY)
        .line_height(theme::LEADING_BODY)
        .width(Length::Fixed(width))
        .style(move |_theme, status| theme::input(room, status));
    // The mark's box, in one of its two states — the label at rest, the clear
    // control while a query stands (ADR-0036 §4). The lane's well makes the
    // identical swap in the identical box, so the `×` is in the same place at
    // every width; `crate::views::clear_mark` is the one function.
    //
    // The left padding differs by 4 px from the lane's and the centre does not:
    // the glyph sits at `GAP_MD` 12 + `ICON_PX` / 2 = 20 here and at
    // `SIDEBAR_HEAD_GLYPH_X` = 20 there, so a `STEPPER_HIT` 24 box centred on
    // the same 20 is inset `GAP_SM` in both.
    let mark: Element<'_, Message> = if filtering {
        container(crate::views::clear_mark(room.recess))
            .height(Length::Fixed(theme::TRANSPORT_HIT))
            .padding(theme::pad(0.0, theme::GAP_SM))
            .align_y(alignment::Vertical::Center)
            .into()
    } else {
        container(
            iced_image(icon::handle(icon::Glyph::Magnifier))
                .width(Length::Fixed(theme::ICON_PX))
                .height(Length::Fixed(theme::ICON_PX))
                .opacity(theme::GLYPH_OPACITY),
        )
        .height(Length::Fixed(theme::TRANSPORT_HIT))
        .padding(theme::pad(0.0, theme::GAP_MD))
        .align_y(alignment::Vertical::Center)
        .into()
    };
    let mut layers = stack![input, mark];
    if filtering {
        // **The match count, inside the control being typed into** — the
        // in-well slot doc 07 §3.1 prescribed, delivered (doc 10 §4.1).
        // Right-aligned in a reserved-width box, so the figure shrinking
        // from `70` to `7` moves nothing; `paper_faint`, a readout's ink.
        layers = layers.push(
            container(
                container(
                    text(match_count(shelf))
                        .size(theme::SIZE_META)
                        .line_height(theme::LEADING_META)
                        .color(room.paper_faint)
                        .wrapping(text::Wrapping::None),
                )
                .width(Length::Fixed(MATCH_W))
                .align_x(alignment::Horizontal::Right),
            )
            .width(Length::Fixed(width))
            .height(Length::Fixed(theme::TRANSPORT_HIT))
            .padding(theme::pad(0.0, theme::GAP_MD))
            .align_x(alignment::Horizontal::Right)
            .align_y(alignment::Vertical::Center),
        );
    }
    layers.into()
}

/// Width of the well's reserved match-count slot (logical px): room for
/// `40000 / 40000` — a library far larger than the owner's — at the meta
/// size, measured in `font.rs`'s reserved-slot test. Fixed for the reason
/// every readout slot is: the figures change as the query narrows, and a
/// right-aligned slot of constant width means they change in place.
pub(crate) const MATCH_W: f32 = 88.0;

/// The collection, described: `1284 albums · 9902 tracks` — the well's
/// placeholder, present exactly when the query is empty (doc 10 §4.1). The
/// corpus size sits behind the glyph that says "search this", which is the
/// fact landing where it is consulted (L8.3).
fn resting_counts(shelf: &Shelf) -> String {
    format!(
        "{} albums · {} tracks",
        shelf.albums.len(),
        shelf.library.len()
    )
}

/// The query, answered: `7 / 1284`, in the well's reserved right-hand slot.
/// It was `7 of 1 284 albums`, ≈ 1 100 px from the keys producing it; inside
/// the control being typed into, the figures need no caption.
fn match_count(shelf: &Shelf) -> String {
    format!("{} / {}", shelf.visible.len(), shelf.albums.len())
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

    /// **The strip's well counts records, and only records.**
    ///
    /// Both figures it draws — the resting placeholder and the match count —
    /// come off `shelf.albums` and `shelf.visible`, which are the wall itself.
    /// For one release the wall had a *subject* beside its arrangement and
    /// both figures went through a `wall_counts` / `wall_noun` pair so that a
    /// wall of artists was not described by a count of albums (ADR-0035). The
    /// subject is gone — the artists are a grouping of the records now, so
    /// every tile on the wall is a record again — and with it the noun, which
    /// is why the word is a literal here and not a call.
    #[test]
    fn the_strips_well_counts_the_records_on_the_wall() {
        let source = source();
        let resting = body(&source, "fn resting_counts(shelf: &Shelf)");
        assert!(
            resting.contains("shelf.albums.len()") && resting.contains("albums · "),
            "the placeholder's figure or its noun is not the wall's"
        );
        let count = body(&source, "fn match_count(shelf: &Shelf)");
        assert!(
            count.contains("shelf.visible.len()") && count.contains("shelf.albums.len()"),
            "the strip's match count is not the query's answer over the wall"
        );
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
            "pub(crate) fn view(shelf: &Shelf, strip_width: f32)",
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
