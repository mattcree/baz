//! The slim Library strip: **how the wall is arranged and what may be done to
//! it**, with the route to the Settings in the corner.
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
//! What is left is one subject, stated in the two vocabularies doc 10 §0.3
//! separates: **the states** — ARTIST · YEAR · GENRE · ADDED · PLAYED, caps
//! and tracked, one of them current — and **the acts** — `Play all`,
//! `Shuffle`, `Pull`, sentence-case words. Narrow-then-arrange used to read
//! left to right across this strip; the narrowing is in the lane now and the
//! strip begins at the arrangement. The gear stays in the corner because it is
//! the *application's* affair rather than the frame's, and the lane's head is
//! a closed set of three (ADR-0030's amendment).
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
    Space, button, column, container, horizontal_rule, image as iced_image, mouse_area, row, stack,
    text, text_input, tooltip,
};
use iced::{Element, Length, alignment};

use crate::app::{Message, Shelf, search_id};
use crate::motion::{Control, Ink};

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

/// The group-key row's reserved width (logical px): five tracked caps words
/// in their `GAP_XS`-padded buttons with `GAP_MD` between them, measured in
/// `font.rs` against this declaration — L9's arithmetic needs every tenant
/// to *declare*, and the declaration is only worth asserting if the face is
/// measured against it.
pub(crate) const KEYS_W: f32 = 314.0;

/// The acts cluster's reserved width (logical px): the triangle and its
/// word, `Shuffle`, `Pull`, their paddings and the two `GAP_XS` gaps.
pub(crate) const ACTS_W: f32 = 182.0;

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
/// **The split is the charter drawn** (doc 10 §4.3): below
/// [`theme::TOP_BAR_SPLIT`] the frame's furniture — the well and the gear —
/// stays on the window line, and the library's verbs and states take a line of
/// their own. Nothing hides, nothing overflows, no menu appears; every control
/// keeps its exact form. It can only be reached where the well is a tenant:
/// once the well is in the lane the strip's tenants sum to 648 against a
/// narrowest possible strip of 720, asserted in `theme.rs`. Below
/// [`theme::TOP_BAR_FLOOR`] nothing further collapses: 600 is the strip's
/// declared floor.
///
/// The resolved height is [`theme::top_bar_h`], and `app.rs`'s viewport
/// estimate reads the same function — the pair of tokens and the breakpoint
/// are one decision, not two that must agree.
pub(crate) fn view(shelf: &Shelf, strip_width: f32, ink: Ink) -> Element<'_, Message> {
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
    // §4.2: at 1440 and 1920 the keys, acts and doors do not move relative
    // to the well; slack is air, not drift).
    let keys = container(keys_row)
        .width(Length::Fixed(KEYS_W))
        .align_x(alignment::Horizontal::Left);
    let acts = container(draws())
        .width(Length::Fixed(ACTS_W))
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
        // With the well in the lane the strip's remaining tenants sum to 648
        // against a strip that is never narrower than 720, so there is nothing
        // to split — asserted in `theme.rs`, not assumed here.
        //
        // The frame line: the well, the transient notes in the slack, then the
        // gear at the corner. The library line: the arrangement's states, then
        // the wall's acts. The seam is the charter's own division — frame
        // furniture above, library verbs below — and both lines keep every
        // control at its exact single-line form.
        let frame_line = row![
            well(shelf, WELL_W),
            Space::with_width(Length::Fill),
            status,
            settings_gear(ink),
        ]
        .spacing(theme::GAP_LG)
        .align_y(iced::Alignment::Center);
        let library_line = row![keys, acts]
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
    status = status.push(settings_gear(ink));
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
                cluster.push(keys).push(acts),
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

/// **The wall's three acts**: `Play all`, `Shuffle` and `Pull`, as words,
/// beside the arrangement.
///
/// # Why they are here and not in the transport
///
/// All three are questions asked *of the collection* — "play what I am looking
/// at, in order", "play what I am looking at, by chance", "suggest something I
/// have not heard" — and the answer to each is decided entirely by what the
/// wall is showing. That is this bar's subject (L8.1: a control goes where
/// what it reads is). The now-playing bar's subject is the record that is
/// sounding, and none of these is about that record; putting them there would
/// also mean moving the transport, which the product's standing rules does not permit
/// for tidiness and would not be tidy anyway.
///
/// They sit *after* the group keys, in the same cluster, because the cluster
/// reads left to right as **narrow, then arrange, then play or draw** — the
/// order the gestures actually happen in. `Play all` leads the three
/// (doc 09 §7.1, S6): it is the plainest of them — the wall, front to back —
/// and the one press that makes February's select-all-to-a-playlist workaround
/// one word. Its scope *is* the wall: the empty query plays everything, a
/// filter plays the matches, a YEAR arrangement plays the collection in
/// chronological order.
///
/// # They are controls, and that is not optional
///
/// the product's standing rules: *"Every action in baz has a visible, pointer-reachable
/// control. No action is keyboard-only, and no control's only affordance is
/// hover."* The pull has a key (<kbd>Ctrl</kbd>+<kbd>R</kbd>); shuffle and
/// `Play all` have none; all three have this. Each sends the identical message
/// any other route sends, which is the same discipline the group keys and the
/// transport already keep.
///
/// # And they are words
///
/// No dice glyph. `crate::icon` draws one small deliberate sprite sheet and a
/// die would be a new mark for a thing with a short unambiguous name — and, more
/// to the point, a dice icon is exactly the costume the refusals ledger says a
/// recommendation engine wears. baz's shuffle can afford to be spelled out
/// because it can say what it is drawing from.
///
/// Sentence case in the Medium face, like the doors: these are **actions**,
/// where the caps-and-tracked row beside them is a set of *states* one of
/// which is current — two of doc 10 §0.3's three vocabularies, and the third
/// (the drawn glyph) enters this strip only where its rule admits it: the
/// gear, the magnifier, and `Play all`'s leading triangle.
fn draws() -> Element<'static, Message> {
    row![
        play_all(),
        // The two draw words teach at the moment of relevance (doc 11 §5
        // P6.2): each carries a tooltip saying what the press will do, in
        // words, before the first press ever risks it. `Shuffle`'s word is
        // almost enough but its bound is not in it; `Pull` has no
        // convention at all (ADR-0026) and until now no explanation before
        // the press — the era licensed poetic names only with explanation
        // at first contact. ("What the Library shows", not "the wall":
        // room vocabulary stays internal, P4's rule applied to P6's
        // sentences.)
        draw_word(
            "Shuffle",
            Message::Shuffle,
            "Play 8 records drawn from what the Library shows",
        ),
        draw_word(
            "Pull",
            Message::Pull,
            "Offer one record you haven't played in years — nothing plays until you say so",
        ),
    ]
    .spacing(theme::GAP_XS)
    .align_y(iced::Alignment::Center)
    .into()
}

/// **`Play all`, wearing the triangle** (doc 10 §3.5, §7 step 4): `Play
/// album`'s glyph + word anatomy, in the strip's own quiet ink.
///
/// The act is conventional — a triangle means *press = sound now*,
/// everywhere — but the **scope** (the wall, as arranged) is baz's own, so
/// the rule of §3.1 lands it in the hybrid form: recognition from the
/// symbol, semantics from the word. One deliberate difference from `Play
/// album`: **no lamp.** The accent belongs to the pages' one commitment
/// (`02` §5.3); here the triangle takes the ordinary resting glyph ink, and
/// the button is the word-acts' own. The triangle is also the one non-type
/// mark in the left cluster, anchoring the seam where states (caps words)
/// end and acts (sentence words) begin.
fn play_all() -> Element<'static, Message> {
    let room = theme::active();
    button(
        container(
            row![
                iced_image(icon::handle(icon::Glyph::Play))
                    .width(Length::Fixed(theme::ICON_PX))
                    .height(Length::Fixed(theme::ICON_PX))
                    .opacity(theme::GLYPH_OPACITY),
                text("Play all")
                    .size(theme::SIZE_META)
                    .line_height(theme::LEADING_META)
                    .font(theme::MEDIUM)
                    .wrapping(text::Wrapping::None),
            ]
            .spacing(theme::GAP_SM)
            .align_y(iced::Alignment::Center),
        )
        .height(Length::Fill)
        .align_y(alignment::Vertical::Center),
    )
    .height(Length::Fixed(theme::TRANSPORT_HIT))
    .padding(theme::pad(0.0, theme::GAP_SM))
    .style(move |_theme, status| theme::word_button(room, room.wall, status))
    .on_press(Message::PlayAll)
    .into()
}

/// One of the two draw words: [`theme::TRANSPORT_HIT`] tall like every control
/// in the product (law L7), centred in its box by the box (law L3), with a
/// tooltip naming what the press does (doc 11 §5 P6.2) — the mechanism the
/// gear already spends, now spent where the words are load-bearing and not
/// quite enough. Below the control, the gear's own position rule: the strip
/// is the window's top edge and a tip above it would clip.
fn draw_word(
    label: &'static str,
    message: Message,
    tip: &'static str,
) -> Element<'static, Message> {
    let room = theme::active();
    let control = button(
        container(
            text(label)
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
    .style(move |_theme, status| theme::word_button(room, room.wall, status))
    .on_press(message);
    tooltip(
        control,
        text(tip)
            .size(theme::SIZE_CAPTION)
            .line_height(theme::LEADING_CAPTION),
        tooltip::Position::Bottom,
    )
    .gap(theme::GAP_XS)
    .padding(theme::GAP_XS)
    .style(move |_theme| theme::tooltip(room))
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
/// # The magnifier is the well's label, not a control
///
/// The universal mark, in the universal corner — the second of the two
/// symbols L8.4's amendment admits as door labels (doc 10 §3.4) — drawn as a
/// **layer** over the input (the mechanism the bar's tip layers already use;
/// iced 0.13's `text_input::Icon` is font-based and therefore not it). The
/// well remains the only focusable widget (ADR-0017 §1.2), the glyph takes
/// the resting glyph ink and answers nothing, and the input's own left
/// padding reserves the lane the glyph sits in, so the caret and the mark
/// cannot collide.
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
    let magnifier = container(
        iced_image(icon::handle(icon::Glyph::Magnifier))
            .width(Length::Fixed(theme::ICON_PX))
            .height(Length::Fixed(theme::ICON_PX))
            .opacity(theme::GLYPH_OPACITY),
    )
    .height(Length::Fixed(theme::TRANSPORT_HIT))
    .padding(theme::pad(0.0, theme::GAP_MD))
    .align_y(alignment::Vertical::Center);
    let mut layers = stack![input, magnifier];
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

/// The route to the Settings **place** — **the gear**, in the corner where
/// every application this audience arrives from keeps it.
///
/// It sits at the far right of the top bar, which is where an application's
/// own affairs belong: the bottom bar is the transport, every pixel of it
/// reserved so that nothing moves as the music does, and it was not touched
/// to put this here.
///
/// It is the one door in baz labelled by a symbol rather than a word, and
/// the licence is narrow (doc 10 §3.4, ADR-0026 §2): L8.4's amendment
/// enumerates exactly two symbols that count as labels — the gear and the
/// magnifier — because both are universal in symbol *and* position, and the
/// tooltip carries the word for the hover (the accessible name,
/// ADR-0017 §4c). It replaced an 84 px word with a 32 px square, which is
/// most of the slack the strip got back.
///
/// It is **navigation**, not a panel toggle, and it is drawn as such: no
/// "open" state, because the place it leads to fills the window and takes
/// this bar with it, so there is no frame in which the control could be lit
/// and visible at once. The same message <kbd>Ctrl</kbd>+<kbd>,</kbd> sends,
/// and the same one the Settings place's own Back sends.
///
/// The anatomy is the transport's own ([`crate::views::bottom_bar`]'s glyph
/// button): the mark is a rasterised sprite, so the ink — not the button
/// style's `text_color`, which never reaches an image — carries the state,
/// through the same `mouse_area` crossings and the same 90 ms tween
/// (ADR-0020 §2.1).
fn settings_gear(ink: Ink) -> Element<'static, Message> {
    let room = theme::active();
    let mark = container(
        iced_image(icon::handle(icon::Glyph::Gear))
            .width(Length::Fixed(theme::ICON_PX))
            .height(Length::Fixed(theme::ICON_PX))
            .opacity(theme::glyph_ink(
                true,
                false,
                ink.hover(Control::Settings),
                ink.pressed(Control::Settings),
            )),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(alignment::Horizontal::Center)
    .align_y(alignment::Vertical::Center);
    let control = button(mark)
        .width(Length::Fixed(theme::TRANSPORT_HIT))
        .height(Length::Fixed(theme::TRANSPORT_HIT))
        .padding(0)
        .style(move |_theme, status| theme::transport(room, room.wall, status))
        .on_press(Message::ToggleSettings);
    let named = tooltip(
        control,
        text("Settings")
            .size(theme::SIZE_CAPTION)
            .line_height(theme::LEADING_CAPTION),
        // Below the control rather than above it: the gear stands in the
        // window's own top corner, and a tip above it would clip.
        tooltip::Position::Bottom,
    )
    .gap(theme::GAP_XS)
    .padding(theme::GAP_XS)
    .style(move |_theme| theme::tooltip(room));
    mouse_area(named)
        .on_enter(Message::ControlEntered(Control::Settings))
        .on_exit(Message::ControlLeft(Control::Settings))
        .into()
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
/// The word is a `button`, so the keyboard's `1`–`5` ([`crate::keys`]) and
/// this control send the identical message and the visible-control rule holds.
fn group_key(key: GroupKey, active: bool) -> Element<'static, Message> {
    let room = theme::active();
    button(
        // Centred in the box by the box, like every other word that is a
        // control (law L3) — a fixed height with top-aligned content is what put
        // `Settings` 6.4 px above its own centre, and five keys doing it beside
        // the well would have been five more.
        container(
            text(theme::tracked(&key.label().to_uppercase()))
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .font(if active { theme::MEDIUM } else { theme::SANS })
                .wrapping(text::Wrapping::None),
        )
        .height(Length::Fill)
        .align_y(alignment::Vertical::Center),
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
