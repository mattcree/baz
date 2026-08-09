//! View composition — ADR-0006's layer 3, and the only disposable one.
//!
//! One module per surface of the interface:
//!
//! - [`setup`] — the first-run "Where's your music?" screen.
//! - [`top_bar`] — the search well, the group-key row, and the quiet counts.
//! - [`shelf`] — the wall: the shelved, virtualized album grid, its pinned
//!   group headers, the index rail, its tiles and its empty states.
//! - [`album`] — the record's page: art, identity, `Play album`, the track
//!   list and the condition report.
//! - [`queue`] — the queue place: what baz handed the engine, and where it is
//!   in it.
//! - [`playlist`] — a playlist's page: the durable list, its acts, and its
//!   rows in the queue place's anatomy (ADR-0024 §4).
//! - [`playlist_panel`] — the one summoned panel: the directory of every
//!   list baz holds, the unnamed sounding one at its head, and the picker a
//!   transfer gesture summons (ADR-0024 §5, as amended by design doc 09).
//!   Not a place — it floats over one, which is why the "one kind of
//!   surface" sentence below now carries its named exception.
//! - [`settings`] — the Settings place: the standing decisions, today
//!   ReplayGain.
//! - [`bottom_bar`] — now-playing, transport, the two timestamps, and the
//!   needle flush on the window's bottom edge.
//! - [`context_menu`] — the mirror layer's float (doc 09 §5.2): the card of
//!   verbs at the pointer, over whichever place and the bar alike. Not a
//!   surface of its own — every item is a press some visible control also
//!   makes ([`crate::menu`]).
//!
//! # There is one kind of surface now, and a bar
//!
//! ADR-0016 had four kinds — place, inspector, popover, bar. ADR-0022 deleted
//! two of them: **every surface here except [`bottom_bar`] is a place, or part
//! of one**. [`top_bar`] and [`shelf`] compose the Library; [`album`],
//! [`queue`] and [`settings`] are the other three. Places fill the window and
//! replace each other ([`crate::place`]), and [`bottom_bar`] is in every one of
//! them and never moves.
//!
//! That is why [`place_header`] is shared rather than copied: three places draw
//! the same strip, in the same geometry as the Library's [`top_bar`], because
//! **the frame is the frame in every place** — navigating may not slide the
//! content area by a pixel.
//!
//! Everything here is iced-specific and holds no state: each module exposes a
//! `view` function that reads [`crate::app`]'s state (and [`crate::player`]'s
//! render-ready readings) and returns an [`Element`]. Composition — which
//! surface is on screen — stays in `app.rs` with the state and the update loop;
//! these modules only know how to draw one surface each. A layout or visual
//! redesign rewrites these files and nothing else, which is the whole point of
//! the split.
//!
//! Values, not layout, live in [`crate::theme`]: no view function here may
//! carry a hardcoded color, size, or padding (ADR-0006 calls that a
//! review-blocking defect). The few constants that *are* here are geometry a
//! single surface owns, and each sits in the module that draws it.
//!
//! # `views::shelf` and `shelf`
//!
//! There are two shelves and they are different layers: [`crate::shelf`] is
//! the pure virtualization *math* (layer 1, unit-tested without a window),
//! [`views::shelf`](shelf) is the *composition* that spends it. The geometry
//! module keeps its place and its name; where a view file needs it, it is
//! imported as `geometry` so the two never read as the same thing.

pub(crate) mod album;
pub(crate) mod bottom_bar;
pub(crate) mod context_menu;
pub(crate) mod playlist;
pub(crate) mod playlist_panel;
pub(crate) mod queue;
pub(crate) mod settings;
pub(crate) mod setup;
pub(crate) mod shelf;
pub(crate) mod top_bar;

use iced::widget::{
    Space, button, column, container, horizontal_rule, image as iced_image, row, text,
};
use iced::{Color, Element, Length, alignment};

use crate::app::{Message, Shelf};
use crate::{theme, vm};

/// A `size`×`size` block filled with the album's deterministic two-color
/// gradient (hash → HSL, see [`vm::gradient_colors`]) — a stand-in sleeve,
/// square-cornered like the artwork it substitutes.
///
/// Shared rather than owned by one surface: the same placeholder stands in
/// for a missing sleeve on a tile and on the record's page, and a redesign that
/// changed one and not the other would be a bug.
///
/// # It is quieter than a real cover, on purpose
///
/// The stops are pulled back toward the sleeve's recess backing by
/// [`theme::Palette::placeholder_ink`], and that is the fix for something
/// plainly wrong in every wide screenshot: at full strength these gradients were
/// the *brightest* objects on a wall of mostly-dark real artwork, so the eye
/// went first to the records baz knows least about. An album with no cover
/// should be the quietest tile in its row.
///
/// The hues survive the mix, which is the whole reason the gradient exists:
/// two albums with no art must still look like two different albums.
///
/// # `shown`
///
/// How strongly the placeholder is drawn, 0…1 — the gradient's own answer to
/// the opacity a real thumbnail is composited at when its record is **outside a
/// running shuffle's pool** ([`theme::POOL_DIM`]). A gradient background is
/// painted rather than sampled, so there is nothing to set an opacity on; it is
/// mixed toward the wall instead, which is what compositing it at that opacity
/// against the wall would have produced. Ordinary tiles pass 1.0 and the mix is
/// the identity.
pub(crate) fn gradient_block(album_id: u64, size: f32, shown: f32) -> Element<'static, Message> {
    let room = theme::active();
    let (c1, c2) = vm::gradient_colors(album_id);
    let to_color = |c: [u8; 3]| {
        let ink = room.placeholder_ink(Color::from_rgb8(c[0], c[1], c[2]));
        theme::Palette::mix(room.wall, ink, shown.clamp(0.0, 1.0))
    };
    let gradient = iced::gradient::Linear::new(iced::Radians(2.4))
        .add_stop(0.0, to_color(c1))
        .add_stop(1.0, to_color(c2));
    container(Space::new(Length::Fixed(size), Length::Fixed(size)))
        .style(move |_theme| container::Style {
            background: Some(iced::Background::Gradient(gradient.into())),
            ..container::Style::default()
        })
        .into()
}

/// **A playlist's sleeve** (ADR-0024 §A1): a collage of quotations from the
/// records it holds, at `edge` px — the panel's rows draw it at
/// [`theme::PANEL_SLEEVE`], the playlist's page at [`theme::ART_MAX`].
///
/// The rule, at every size: four or more distinct records → a 2 × 2 collage
/// of the first four, in playlist order; one to three → the first record's
/// sleeve, full-bleed; none the library resolves → the rest tile (the
/// surface step, the name in ink — an empty made thing is quiet, not
/// decorated). Cells come from the wall's own thumbnail cache and degrade to
/// the wall's own deterministic gradient while a decode is in flight, so a
/// playlist's sleeve can never disagree with the tiles of the records it
/// quotes.
///
/// This *constructs* a playlist's sleeve out of whole, unmarked artwork at
/// thumbnail scale; nothing is drawn on top of any record's sleeve, and no
/// cell exceeds the decoded source (§A1 argues both against the refusals by
/// name). Shared by the panel and the page for [`gradient_block`]'s reason:
/// two renderings of one identity that could drift apart would be a bug.
pub(crate) fn playlist_sleeve(
    shelf: &Shelf,
    art: &[u64],
    name: &str,
    edge: f32,
) -> Element<'static, Message> {
    let room = theme::active();
    match art {
        [] => {
            // The rest tile: the name whole at page scale, its initial at
            // panel scale — a 40 px tile has no room for words and needs
            // only to be tellable apart.
            let large = edge >= theme::ART_MIN;
            let label: String = if large {
                name.to_owned()
            } else {
                name.chars()
                    .next()
                    .map(|initial| initial.to_uppercase().to_string())
                    .unwrap_or_default()
            };
            let word = if large {
                text(label)
                    .size(theme::SIZE_TITLE)
                    .line_height(theme::LEADING_TITLE)
                    .font(theme::SEMIBOLD)
            } else {
                text(label)
                    .size(theme::SIZE_EMPHASIS)
                    .line_height(theme::LEADING_EMPHASIS)
                    .font(theme::MEDIUM)
            };
            container(word)
                .width(Length::Fixed(edge))
                .height(Length::Fixed(edge))
                .padding(if large { theme::GAP_MD } else { 0.0 })
                .align_x(alignment::Horizontal::Center)
                .align_y(alignment::Vertical::Center)
                .clip(true)
                .style(move |_theme| theme::playlist_rest_tile(room))
                .into()
        }
        [a, b, c, d, ..] => {
            let half = edge / 2.0;
            column![
                row![sleeve_cell(shelf, *a, half), sleeve_cell(shelf, *b, half)],
                row![sleeve_cell(shelf, *c, half), sleeve_cell(shelf, *d, half)],
            ]
            .width(Length::Fixed(edge))
            .height(Length::Fixed(edge))
            .into()
        }
        // Below four distinct records the first one's face is the sleeve —
        // one rule at every size, and the tiling question never opens.
        [first, ..] => sleeve_cell(shelf, *first, edge),
    }
}

/// One quotation in a playlist's sleeve: the record's thumbnail from the
/// wall's cache, or — while its decode is in flight, or where no art can be
/// decoded — the same deterministic gradient the record's own tile shows.
fn sleeve_cell(shelf: &Shelf, album: u64, size: f32) -> Element<'static, Message> {
    match shelf.thumbs.peek(&album) {
        Some(handle) => iced_image(handle.clone())
            .width(Length::Fixed(size))
            .height(Length::Fixed(size))
            .into(),
        None => gradient_block(album, size, 1.0),
    }
}

/// **The strip every place that is not the Library wears**: the way back, the
/// place's name, and one quiet line saying what the place is or how to leave
/// it.
///
/// It occupies the Library's top-bar geometry exactly — the same vertical
/// padding, the same [`theme::HANG`] window gutter (law L1), the same hairline
/// underneath — so that moving between places does not slide the content area
/// by a pixel. **The frame is the frame in every place**, and with three places
/// wearing it rather than one, that is a property worth having in one function
/// instead of three copies that can drift.
///
/// **Back is a word, not a chevron.** A door is labelled with the name of what
/// it opens (doc 07 L8.4), and the amendment that let the gear and the
/// magnifier stand as symbols is a closed two-name list (doc 10 §3.4) — a back
/// arrow is merely familiar, not universal, so this door keeps its word. It
/// sends [`Message::LeavePlace`], which is the message <kbd>Esc</kbd> sends, so
/// the two are one press and the visible-control rule holds for every place.
pub(crate) fn place_header(name: &'static str, note: &'static str) -> Element<'static, Message> {
    let room = theme::active();
    let back = button(
        // Centred in its own box, like `Settings` across the frame from it
        // (law L3).
        container(
            text("‹ Library")
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .font(theme::MEDIUM)
                .wrapping(text::Wrapping::None),
        )
        .height(Length::Fill)
        .align_y(alignment::Vertical::Center),
    )
    // The same height as the top bar's `Settings`, which is the control this
    // one swaps places with: the two strips are one frame, and a way-back that
    // stood shorter than the control it replaced would make the header jump on
    // every navigation.
    .height(Length::Fixed(theme::TRANSPORT_HIT))
    .padding(theme::pad(0.0, theme::GAP_SM))
    .style(move |_theme, status| theme::word_button(room, room.wall, status))
    .on_press(Message::LeavePlace);
    column![
        container(
            row![
                back,
                text(name)
                    .size(theme::SIZE_EMPHASIS)
                    .line_height(theme::LEADING_EMPHASIS)
                    .font(theme::MEDIUM),
                Space::with_width(Length::Fill),
                text(note)
                    .size(theme::SIZE_META)
                    .line_height(theme::LEADING_META)
                    .color(room.paper_faint)
                    .wrapping(text::Wrapping::None),
            ]
            .spacing(theme::GAP_LG)
            .align_y(iced::Alignment::Center),
        )
        .padding(theme::pad(theme::TOP_BAR_PAD_V, theme::HANG)),
        horizontal_rule(1).style(move |_theme| theme::hairline(room, room.wall)),
    ]
    .into()
}

/// **The one gutter a place's body hangs from** (law L1): [`theme::HANG`] on
/// every edge, with the scrollbar's declared lane added to the right.
///
/// A place fills the window, so its content hangs from the same two lines the
/// wall and both bars do — `x = HANG` and `x = W − HANG` — and from `y = HANG`,
/// which is the free top a place has and a panel never did. `GAP_XL` is padding
/// *inside* a panel and was never a window margin; spending it as one is how
/// baz ended up with three of them.
///
/// The right edge carries [`theme::SCROLLBAR_LANE`] as well, and that is the
/// one inset the law permits there: it is *declared* rather than absorbed, so a
/// page long enough to scroll does not put its bar over the last character of
/// every duration.
pub(crate) fn place_pad() -> iced::Padding {
    iced::Padding {
        top: theme::HANG,
        right: theme::HANG + theme::SCROLLBAR_LANE,
        bottom: theme::HANG,
        left: theme::HANG,
    }
}

/// A block's name inside a place: a hairline, then the word in the room's
/// quietest voice.
///
/// The one structural rule beyond the three `.interface-design/system.md` §2
/// names, and it earns its place the way the Settings readout's does: it
/// divides two kinds of content inside one column. Shared by the record page's
/// `Tracks` and `Details` because a page whose two blocks named themselves
/// differently would read as two surfaces.
pub(crate) fn section_rule(name: &'static str) -> Element<'static, Message> {
    let room = theme::active();
    column![
        horizontal_rule(1).style(move |_theme| theme::hairline(room, room.wall)),
        text(theme::tracked(&name.to_uppercase()))
            .size(theme::SIZE_HEADING)
            .line_height(theme::LEADING_HEADING)
            .font(theme::MEDIUM)
            .color(room.paper_faint),
    ]
    .spacing(theme::GAP_SM)
    .into()
}
