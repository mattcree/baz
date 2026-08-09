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
use iced::widget::{
    Space, button, column, container, horizontal_rule, image as iced_image, mouse_area, row, stack,
    text, text_input, tooltip,
};
use iced::{Element, Length, alignment};

use crate::app::{Message, Shelf, search_id};
use crate::motion::{Control, Ink};

use crate::{icon, theme};

/// The search well's width floor (logical px) — what it stands at when the
/// window has spent the fluid range.
pub(crate) const WELL_MIN: f32 = 200.0;

/// The search well's width ceiling (logical px) — 280 at the shipped window
/// and above.
///
/// It was `SEARCH_W` 360, sized in the era when you *aimed* at the field;
/// under type-anywhere (ADR-0017 §1.2) you reach it by typing, and 280 holds
/// a long query beside the reserved match slot (doc 10 §2.3). The 80 px the
/// ceiling gives back is the strip's second reclamation.
pub(crate) const WELL_MAX: f32 = 280.0;

/// The well's width at `window_width`: `clamp(W − 1000, 200, 280)`
/// (doc 10 §4.1) — 280 at ≥ 1280, spending its fluid 80 px between 1040 and
/// 960, then holding the floor. The well is the strip's **one** fluid
/// tenant, which is what makes the collapse order one step: first this
/// range, then the split (§4.3).
pub(crate) fn well_width(window_width: f32) -> f32 {
    (window_width - 1000.0).clamp(WELL_MIN, WELL_MAX)
}

/// The slim top bar: the search well on the left, quiet status and the route
/// to the settings on the right, a hairline rule below.
///
/// `window_width` decides the well's width and nothing else yet — the
/// parameter every regime needs (doc 10 §7 step 3); the strip itself takes
/// no other reading of it until the split (step 5).
pub(crate) fn view(shelf: &Shelf, window_width: f32, ink: Ink) -> Element<'_, Message> {
    let room = theme::active();
    let search = well(shelf, well_width(window_width));
    let mut keys = row![]
        .spacing(theme::GAP_MD)
        .align_y(iced::Alignment::Center);
    for key in GroupKey::ALL {
        keys = keys.push(group_key(key, key == shelf.group_key));
    }
    // The status row holds only the transient notes now — the counts moved
    // into the well they describe (doc 10 §4.1; L8.3's valve run in reverse:
    // the fact goes to where it is watched). What the move freed is exactly
    // the slack the scan notes spend, which is what repaired the strip's
    // scan-time overflow at the shipped window (doc 10 §2.1).
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
    status = status.push(settings_gear(ink));
    column![
        container(
            row![
                // The well and the keys are one cluster — both are about the
                // library — held apart by the ladder's largest gap so they
                // read as two groups on one line rather than as six controls.
                row![search, keys, draws(), playlists_door()]
                    .spacing(theme::GAP_XL)
                    .align_y(iced::Alignment::Center),
                Space::with_width(Length::Fill),
                status
            ]
            .spacing(theme::GAP_LG)
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
/// also mean moving the transport, which `docs/REFUSALS.md` does not permit
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
/// `docs/REFUSALS.md`: *"Every action in baz has a visible, pointer-reachable
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
        draw_word("Shuffle", Message::Shuffle),
        draw_word("Pull", Message::Pull),
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
/// in the product (law L7), centred in its box by the box (law L3).
fn draw_word(label: &'static str, message: Message) -> Element<'static, Message> {
    let room = theme::active();
    button(
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
    .on_press(message)
    .into()
}

/// **The playlist panel's door** (ADR-0024 §5): a labelled word in the
/// Library strip, <kbd>Ctrl</kbd>+<kbd>P</kbd> beside it.
///
/// It is in *this* strip by the placement law: a door goes where the hand
/// already is (L8.4), and playlists are about the collection — the panel is
/// summoned to collect *from* the wall. It closes the left cluster's reading
/// order: **narrow, then arrange, then draw, then collect**.
///
/// Labelled with the name of what it opens, **in words** — and confirmed as
/// a word by doc 10 §3.4: no universal symbol distinguishes *playlists* from
/// *queue* from *menu*, so this door is exactly the class L8.4's two-symbol
/// exception refuses. Unlike the gear across the frame it *is* honestly a
/// toggle, because the panel floats over this strip's own place rather than
/// replacing it, so the door stays visible while what it opened is open.
/// What it deliberately does not gain is a lit "open" state: the panel
/// standing 340 px away is its own statement, and a second one would be the
/// same fact twice.
fn playlists_door() -> Element<'static, Message> {
    let room = theme::active();
    button(
        container(
            text("Playlists")
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
    .on_press(Message::TogglePlaylists)
    .into()
}

/// **The search well**, with the magnifier laid over its left padding
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
    // (`REFUSALS.md`) restated in figures.
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
