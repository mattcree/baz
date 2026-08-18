//! View composition — ADR-0006's layer 3, and the only disposable one.
//!
//! One module per surface of the interface:
//!
//! - [`app_bar`] — **the app bar**: the band across the top of the window, in
//!   every place, identical — the window's own chrome, drawn by baz
//!   (ADR-0040). The display options, the gear, the window controls, and the
//!   window's name.
//! - [`setup`] — the first-run "Where's your music?" screen.
//! - [`blocked`] — its sibling and its opposite (ADR-0041): the library is
//!   there and baz will not open it. A **statement** where [`setup`] asks a
//!   question — a database from a newer baz, one that cannot be read, or a
//!   machine with nowhere to keep one. Neither wears the app bar, for the
//!   reason [`blocked`]'s own docs give.
//! - [`top_bar`] — the search well, the group-key row, and the quiet counts.
//! - [`shelf`] — the wall: the shelved, virtualized album grid, its pinned
//!   group headers, the index rail, its tiles and its empty states.
//! - [`page`] — **one page, two subjects**: the composition [`album`] and
//!   [`playlist`] both wear, and the leaf widgets their rows share. Not a
//!   surface of its own — it draws whichever of the two is handed to it
//!   (ADR-0024 §A2's arrangement, made literal).
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
//!   needle across the playback bar's top edge.
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
//! [`queue`] and [`settings`] are three more. Places fill the window and
//! replace each other ([`crate::place`]), and [`bottom_bar`] is in every one of
//! them and never moves.
//!
//! That is why [`place_header_with`] is shared rather than copied: the places
//! that wear one draw the same strip, in the same geometry as the Library's
//! [`top_bar`], because
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
pub(crate) mod app_bar;
pub(crate) mod artist;
pub(crate) mod blocked;
pub(crate) mod bottom_bar;
pub(crate) mod compose;
pub(crate) mod context_menu;
pub(crate) mod drag_ghost;
pub(crate) mod favourites;
pub(crate) mod home;
pub(crate) mod lane;
pub(crate) mod list_tile;
pub(crate) mod new_playlist;
pub(crate) mod now_playing;
pub(crate) mod page;
pub(crate) mod playlist;
pub(crate) mod playlist_page;
pub(crate) mod playlist_panel;
pub(crate) mod playlists;
pub(crate) mod queue;
pub(crate) mod search;
pub(crate) mod settings;
pub(crate) mod setup;
pub(crate) mod shelf;
pub(crate) mod shortcuts;
pub(crate) mod status;
pub(crate) mod top_bar;

use std::sync::LazyLock;

use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use iced::widget::{Space, button, column, container, image as iced_image, row, rule, text};
use iced::{Color, Element, Length, alignment};

use crate::app::{Message, Shelf};
use crate::{font, icon, theme, vm};

/// The bundled Regular face, for measuring a string before Iced sets it.
pub(crate) static FIT_REGULAR: LazyLock<FontRef<'static>> = LazyLock::new(|| {
    FontRef::try_from_slice(font::SANS_REGULAR).expect("the bundled regular face is valid")
});
/// The bundled Medium face, likewise.
pub(crate) static FIT_MEDIUM: LazyLock<FontRef<'static>> = LazyLock::new(|| {
    FontRef::try_from_slice(font::SANS_MEDIUM).expect("the bundled medium face is valid")
});
/// The bundled `SemiBold` face — Now playing's display title is set in it, and a
/// line measured against the wrong weight is a line fitted to the wrong width.
pub(crate) static FIT_SEMIBOLD: LazyLock<FontRef<'static>> = LazyLock::new(|| {
    FontRef::try_from_slice(font::SANS_SEMIBOLD).expect("the bundled semibold face is valid")
});

/// One fixed-height line of type, shortened with a **visible** end ellipsis
/// when it does not fit its measure.
///
/// # Why this exists rather than `Wrapping::None` and a clip
///
/// A clip stops a long string **mid-glyph, with nothing to say it continues**.
/// The eye reads that as a rendering fault rather than as a shortened name,
/// and it is what the owner met in the bottom bar: *"the now playing song
/// title seems cut off when it is long"*.
///
/// Iced's own `…` is not the answer either. iced 0.14 can still break
/// `Wrapping::None` text at a constrained width, which puts the ellipsis on an
/// invisible second line — so the failure sign disappears in exactly the case
/// it exists for. The returns lane worked this out first and solved it with
/// two clipped subslots: the prefix, fitted against the **real bundled face at
/// the real size**, and the ellipsis in a slot of its own that the fitting has
/// already reserved. Nothing depends on the renderer's own rounding.
///
/// This is that reading, lifted out of `lane.rs` so the bar can have it rather
/// than growing a second one. The lane passes its fixed row measure; the bar
/// passes [`theme::bar_title_lane_w`], which is the same thing computed from
/// the window rather than declared.
pub(crate) struct Fitted<'a> {
    pub content: &'a str,
    pub face: &'a FontRef<'static>,
    pub size: f32,
    pub leading: f32,
    pub line_height: f32,
    pub font: iced::Font,
    pub color: Color,
    /// The lane the whole line — prefix and ellipsis together — must fit.
    pub measure: f32,
}

/// Draw a [`Fitted`].
///
/// **The measure is a ceiling, not a width.** A line that fits is as wide as
/// its own words; only a line that has to be cut spends the whole lane, and
/// then it spends it because that is what it was cut to.
///
/// This was a fixed `measure` in every state until 2026-08-17, which made the
/// bottom bar's block the full lane whatever it held — and the owner, twice:
/// *"the now playing bottom bar area still does not seem to have a min size
/// that makes sense which should only grow up to a max based on the content of
/// the artist name and song title etc. — this avoids the heart icons being out
/// in the middle of nowhere."* The heart is the block's sibling, so a block
/// that was always lane-wide put it lane-wide away from a three-word title.
pub(crate) fn fitted_line(line: &Fitted<'_>) -> Element<'static, Message> {
    let (fitted, truncated) = fit(line.content, line.face, line.size, line.measure);
    let set = |content: String, width: Length, align| {
        container(
            text(content)
                .size(line.size)
                .line_height(line.leading)
                .font(line.font)
                .color(line.color)
                .wrapping(text::Wrapping::None),
        )
        .width(width)
        .height(Length::Fixed(line.line_height))
        .align_x(align)
        .clip(true)
    };
    let prefix = set(
        fitted,
        if truncated {
            Length::Fixed(line.measure - theme::ELLIPSIS_SLOT_W)
        } else {
            // Its own words. `Fill` here is what made every line lane-wide.
            Length::Shrink
        },
        alignment::Horizontal::Left,
    );
    let ending: Element<'static, Message> = if truncated {
        set(
            "…".to_owned(),
            Length::Fixed(theme::ELLIPSIS_SLOT_W),
            alignment::Horizontal::Right,
        )
        .into()
    } else {
        Space::new().width(Length::Fixed(0.0)).into()
    };
    container(row![prefix, ending])
        .width(if truncated {
            Length::Fixed(line.measure)
        } else {
            Length::Shrink
        })
        .height(Length::Fixed(line.line_height))
        .clip(true)
        .into()
}

/// The longest prefix of `content` that fits `measure` less the ellipsis'
/// reserved slot, and whether anything was dropped.
///
/// Measured with the face and size the widget will actually draw with,
/// kerning included — a fit against a different face is a fit against a
/// different string.
pub(crate) fn fit(content: &str, face: &impl Font, size: f32, measure: f32) -> (String, bool) {
    if text_width(face, size, content) <= measure {
        return (content.to_owned(), false);
    }
    let scaled = face.as_scaled(PxScale::from(size));
    let prefix_w = measure - theme::ELLIPSIS_SLOT_W;
    let mut fitted = String::new();
    let mut width = 0.0;
    let mut previous = None;
    for character in content.chars() {
        let glyph = scaled.glyph_id(character);
        let next =
            width + previous.map_or(0.0, |was| scaled.kern(was, glyph)) + scaled.h_advance(glyph);
        if next > prefix_w {
            break;
        }
        fitted.push(character);
        width = next;
        previous = Some(glyph);
    }
    (fitted, true)
}

/// The width `text` occupies in `face` at `size`, kerning included.
pub(crate) fn text_width(face: &impl Font, size: f32, text: &str) -> f32 {
    let scaled = face.as_scaled(PxScale::from(size));
    let mut width = 0.0;
    let mut previous = None;
    for character in text.chars() {
        let glyph = scaled.glyph_id(character);
        if let Some(was) = previous {
            width += scaled.kern(was, glyph);
        }
        width += scaled.h_advance(glyph);
        previous = Some(glyph);
    }
    width
}

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
/// an opacity a real thumbnail could be composited at. A gradient background is
/// painted rather than sampled, so there is nothing to set an opacity on; it is
/// mixed toward the wall instead, which is what compositing it at that opacity
/// against the wall would have produced.
///
/// **Every caller now passes 1.0, and the mix is the identity.** The one that
/// did not was the wall's shuffle pool, which drew every record the running
/// draw could not play at `POOL_DIM` 35 %; shuffle became a property of the
/// player on 2026-08-10 and there is no pool to dim. The parameter stays
/// because the *question* it answers is a real one a placeholder must be able
/// to answer — a gradient that could not be drawn faintly would be a
/// placeholder that could not be treated like the artwork it stands in for.
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
    container(
        Space::new()
            .width(Length::Fixed(size))
            .height(Length::Fixed(size)),
    )
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
    playlist_sleeve_marked(shelf, art, name, edge, None)
}

/// **A playlist's sleeve when the listener has chosen one** — the authored
/// picture, drawn full-bleed at `edge`, or the collage where there is none.
///
/// The owner: *"lets allow setting an image/removing the image for a
/// playlist."* One picture stands in front of the generated collage; taking it
/// away gives the collage back, because the collage is what a playlist's
/// sleeve *is* when nobody has said otherwise (ADR-0024 §A1 stands — it says
/// what baz draws unasked, not that a listener may not choose).
///
/// `image` is the decoded handle from [`Shelf::playlist_image`], and `None`
/// covers three different things on purpose: no picture was set, the decode is
/// still in flight, or the file could not be read. All three draw the collage,
/// which is a true sleeve for the list in every one of them.
pub(crate) fn playlist_sleeve_authored(
    shelf: &Shelf,
    image: Option<&iced_image::Handle>,
    art: &[u64],
    name: &str,
    edge: f32,
) -> Element<'static, Message> {
    match image {
        Some(handle) => authored_sleeve(handle, edge),
        None => playlist_sleeve(shelf, art, name, edge),
    }
}

/// [`playlist_sleeve_authored`] for the surfaces that know the list by its
/// **id** — the wall's tiles, the panel's rows and the returns lane — which is
/// also where a built-in's mark belongs. One function so a list cannot look
/// like two different objects in two surfaces, which is ADR-0024 §A1's own
/// argument for the collage in the first place.
pub(crate) fn playlist_sleeve_of(
    shelf: &Shelf,
    id: u64,
    art: &[u64],
    name: &str,
    edge: f32,
) -> Element<'static, Message> {
    match shelf.playlist_image(id) {
        Some(handle) => authored_sleeve(handle, edge),
        None => playlist_sleeve_marked(shelf, art, name, edge, default_playlist_mark(id)),
    }
}

/// The listener's own picture, square and full-bleed at `edge`.
///
/// **Cover, not fit**: a sleeve is a square hole in every surface baz draws it
/// in, and a picture that letterboxed inside it would put the room's ground in
/// the middle of a shelf of covers. The picture is cropped to the square from
/// its centre and never enlarged past its own pixels — `art::load_picture` is
/// downscale-only, so a small picture is drawn small inside its own tile
/// rather than blown up.
fn authored_sleeve(handle: &iced_image::Handle, edge: f32) -> Element<'static, Message> {
    container(
        iced_image(handle.clone())
            .width(Length::Fixed(edge))
            .height(Length::Fixed(edge))
            .content_fit(iced::ContentFit::Cover),
    )
    .width(Length::Fixed(edge))
    .height(Length::Fixed(edge))
    .clip(true)
    .into()
}

/// **The default mark a built-in list wears** where it has no records to
/// quote — today only `Favourites`, and its mark is the heart every row in
/// the product hearts a song with.
///
/// The owner: *"can we create a default heart image on the playlists."* A
/// list with nothing in it draws the rest tile, and for a list the listener
/// *made* the honest thing to put there is its name — there is nothing else
/// true about it yet. `Favourites` is not that: it is a built-in whose
/// subject is known before it holds anything, and the mark says so at every
/// size, in every surface that draws a list's sleeve.
#[must_use]
pub(crate) fn default_playlist_mark(id: u64) -> Option<icon::Glyph> {
    (id == crate::playlists::FAVOURITES_ID).then_some(icon::Glyph::HeartFilled)
}

/// [`playlist_sleeve`], with a mark to wear instead of the name where the
/// list has no records to quote.
pub(crate) fn playlist_sleeve_marked(
    shelf: &Shelf,
    art: &[u64],
    name: &str,
    edge: f32,
    mark: Option<icon::Glyph>,
) -> Element<'static, Message> {
    let room = theme::active();
    match art {
        // A built-in list wears its mark rather than its name: see
        // [`default_playlist_mark`]. The ground is the rest tile's, so this is
        // the same *surface* the name would have stood on — a mark instead of
        // a word, not a decorated tile.
        [] if mark.is_some() => {
            let glyph = mark.unwrap_or(icon::Glyph::HeartFilled);
            let px = if edge >= theme::ART_MIN {
                theme::GHOST_MARK_PX
            } else {
                theme::ICON_PX
            };
            container(
                iced_image(icon::handle(glyph))
                    .width(Length::Fixed(px))
                    .height(Length::Fixed(px))
                    .opacity(theme::GLYPH_OPACITY),
            )
            .width(Length::Fixed(edge))
            .height(Length::Fixed(edge))
            .align_x(alignment::Horizontal::Center)
            .align_y(alignment::Vertical::Center)
            .clip(true)
            .style(move |_theme| theme::playlist_rest_tile(room))
            .into()
        }
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
    match shelf.thumb(album) {
        Some(handle) => iced_image(handle.clone())
            .width(Length::Fixed(size))
            .height(Length::Fixed(size))
            .into(),
        None => gradient_block(album, size, 1.0),
    }
}

/// **The well's clear mark** — the `×` the owner asked for, in whichever of the
/// two wells is on screen (ADR-0036 §4).
///
/// One function because there are two wells and they must not drift: the lane's
/// ([`lane`]) above [`theme::SIDEBAR_FLOOR`], the strip's ([`top_bar`]) below
/// it. Both lay it over the field's left padding in the box the magnifier
/// otherwise holds, so it is the same mark in the same place at every width,
/// and both are handed their own ground — the well is a [`theme::Palette::recess`]
/// in the lane and a recess in the strip, and a wash is a function of what it
/// is drawn on.
///
/// **It is a control, so it wears a control's anatomy**: [`theme::STEPPER_HIT`]
/// 32 square — the same box the playlist row's own removal cross takes. The
/// well's layer pads it onto the head's one vertical
/// ([`theme::SIDEBAR_HEAD_GLYPH_X`]) in the box the magnifier otherwise holds,
/// so it is the same mark in the same place at every width.
///
/// **At [`theme::glyph_opacity`]'s resting reading**, which is the same 0.57 the
/// magnifier it replaces is drawn at, and the same every live icon button in the
/// product wears at rest. It was tried at [`theme::GLYPH_OPACITY_HOVER`] — the
/// weight the playlist row's cross takes — and the frame settled it: that cross
/// is *revealed* by a hover and is on screen for as long as the pointer is,
/// whereas this one stands for the whole life of a query, and at full ink it was
/// the loudest object in the lane, louder than the query it offers to delete.
/// The pointer's answer is the button's own wash ([`theme::transport`]), which
/// is what a wash is for.
///
/// The icon-only law (doc 10 §3.1) wants a word for it, and the word says what
/// the key says — the two roads are one function in `app.rs`.
pub(crate) fn clear_mark(on: Color) -> Element<'static, Message> {
    let room = theme::active();
    let mark = container(
        iced_image(crate::icon::handle(crate::icon::Glyph::Close))
            .width(Length::Fixed(theme::ICON_PX))
            .height(Length::Fixed(theme::ICON_PX))
            .opacity(theme::glyph_opacity(true, false)),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(alignment::Horizontal::Center)
    .align_y(alignment::Vertical::Center);
    iced::widget::tooltip(
        iced::widget::button(mark)
            .width(Length::Fixed(theme::STEPPER_HIT))
            .height(Length::Fixed(theme::STEPPER_HIT))
            .padding(0)
            .style(move |_theme, status| theme::transport(room, on, status))
            .on_press(Message::ClearSearch),
        text("Clear the search (Esc)")
            .size(theme::SIZE_CAPTION)
            .line_height(theme::LEADING_CAPTION),
        iced::widget::tooltip::Position::Bottom,
    )
    .gap(theme::GAP_XS)
    .padding(theme::GAP_XS)
    .style(move |_theme| theme::tooltip(room))
    .into()
}

/// **The strip every place that is not the Library wears**: the way back, the
/// place's name, and one quiet line saying what the place is or how to leave
/// it.
///
/// It occupies the Library's top-bar geometry exactly — the same vertical
/// padding, the same [`theme::HANG`] window gutter (law L1), the same hairline
/// underneath — so that moving between places does not slide the content area
/// by a pixel. **The frame is the frame in every place**: Queue and — since doc
/// 10 §7 step 8 — Settings wear this form, Artist and the two subject pages
/// ([`page`]) wear [`place_header_led`]'s, and the Library's own strip is
/// [`top_bar`]. One function rather than copies that can drift.
///
/// **The header carries no way back, and that is not a missing affordance.**
/// It held a `‹ Library` door and an `Esc returns to Library` hint until the
/// returns lane shipped; the lane is resident in every place and both of its
/// states, so `Library` is permanently one press away, up and to the left, and
/// a second door in every header was the same statement made twice. The
/// keyboard is untouched — <kbd>Esc</kbd> still peels and still lands on the
/// Library — and the visible-control rule holds through the lane's own row.
/// **Do not restore a back door here**: its absence is the lane's presence.
///
/// # `note`
///
/// One quiet statement at the strip's right edge.
///
/// It is for a statement about the *place*, never a keyboard hint — the
/// Settings place's *"Kept in config.toml…"* is the only one today. The strip
/// stays one function so the geometry cannot drift between the place that
/// carries a note and the ones that do not.
///
/// It used to carry a third parameter, an extra tenant after the place's name,
/// and the Album place's `‹ Prev` / `Next ›` pair was its only customer. The
/// owner removed the pair — *"previous and next on albums doesn't make sense
/// on the album view"* — and the slot went with it rather than being left open
/// for the next thing that fancies the strip.
pub(crate) fn place_header_with(
    name: &'static str,
    note: Option<&'static str>,
) -> Element<'static, Message> {
    place_header_led(place_name(name), note.map(str::to_owned))
}

/// **The strip's standard lead**: the place's own name.
///
/// It stands where the way-back used to, so the frame's left edge is unchanged
/// (law L1) and moving between places still slides nothing.
pub(crate) fn place_name(name: &str) -> Element<'static, Message> {
    text(name.to_owned())
        .size(theme::SIZE_EMPHASIS)
        .line_height(theme::LEADING_EMPHASIS)
        .font(theme::MEDIUM)
        .wrapping(text::Wrapping::None)
        .into()
}

/// One state in a place's arrangement control: tracked capitals, a quiet
/// regular inactive state, and a medium full-ink active state.
///
/// Shared by the Library and Playlists strips so choosing how a collection is
/// ordered is one control pattern rather than two buttons that merely happen
/// to send similar messages.
pub(crate) fn arrangement_key(
    label: &str,
    active: bool,
    message: Message,
) -> Element<'static, Message> {
    let room = theme::active();
    button(
        container(
            text(theme::tracked(&label.to_uppercase()))
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .font(if active { theme::MEDIUM } else { theme::SANS })
                .wrapping(text::Wrapping::None),
        )
        .height(Length::Fill)
        .align_y(alignment::Vertical::Center),
    )
    .height(Length::Fixed(theme::TRANSPORT_HIT))
    .padding(theme::pad(0.0, theme::GAP_XS))
    .style(move |_theme, status| theme::group_key(room, room.wall, status, active))
    .on_press(message)
    .into()
}

/// [`place_header_with`], with an arbitrary **lead** and an optional quiet
/// statement at the strip's right edge.
///
/// Four of the places lead with [`place_name`] and nothing else. Two do not,
/// and they are the pair the owner's breadcrumb joins: the Album place leads
/// with `Artist › Album`, whose first half is a door, and the Artist place
/// leads with the artist's name — a *runtime* string, which is why the lead is
/// an `Element` here rather than a `&'static str`.
///
/// **The strip stays one function** so the geometry cannot drift between the
/// places that lead with a control and the ones that lead with a word. That is
/// the same reason the extra-tenant slot was deleted with the `‹ Prev` /
/// `Next ›` pair it was built for rather than left open.
pub(crate) fn place_header_led(
    lead: Element<'static, Message>,
    note: Option<String>,
) -> Element<'static, Message> {
    let room = theme::active();
    // **The lead stands in a [`theme::TRANSPORT_HIT`] box.** Without it this
    // function lays out whatever it is handed, and what it is handed differs in
    // kind: the Album place's breadcrumb and the Artist place's name are
    // *controls* and declare 32 of their own, while a bare [`place_name`] is
    // `LEADING_EMPHASIS` 20. So the strip came to 49 px in some places and
    // 37 px in others, and every place in the second group drew its whole body
    // **12 px higher** than every place in the first.
    //
    // That made this file's own sentence false — *"the frame is the frame in
    // every place; navigating may not slide the content area by a pixel"* — and
    // it was false for about a month, in the one direction a reader of the
    // source would not look: `TOP_BAR_H` is a correct constant, honoured by the
    // Library's own strip, and the drift was in the *other* strip not being
    // held to it.
    //
    // Found in a frame, not in the source, and it took a particular kind of
    // frame: the study that found it shot two pages at the same **window**
    // coordinates rather than cropping each page's header out of its own
    // picture. Cropping compares shapes; a shared crop compares positions, and
    // the two identity blocks were the same 80 px shape sitting 12 px apart.
    // `docs/design/impl/one-page-two-subjects/`.
    let mut strip = row![
        container(lead)
            .height(Length::Fixed(theme::TRANSPORT_HIT))
            .align_y(alignment::Vertical::Center)
    ]
    .spacing(theme::GAP_LG)
    .align_y(iced::Alignment::Center);
    strip = strip.push(Space::new().width(Length::Fill));
    if let Some(note) = note {
        strip = strip.push(
            text(note)
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.paper_faint)
                .wrapping(text::Wrapping::None),
        );
    }
    column![
        container(strip).padding(theme::pad(theme::TOP_BAR_PAD_V, theme::HANG)),
        rule::horizontal(1).style(move |_theme| theme::hairline(room, room.wall)),
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

/// **Several things, named as a sentence would name them** — `energy`,
/// `energy and tempo`, `energy, tempo and texture`.
///
/// Here rather than in one view because a joined list is prose, and prose in
/// this product is written once. No serial comma, which is the house
/// spelling everywhere else in the copy.
pub(crate) fn list_words(words: &[String]) -> String {
    match words {
        [] => String::new(),
        [only] => only.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// **One quiet line**: a statement about a flow rather than a control.
///
/// Shared rather than copied per view because the composing page and the
/// manual route say the same kind of thing in the same voice, and a hint that
/// changed weight between two halves of one place would read as two places.
pub(crate) fn hint(line: &str) -> Element<'static, Message> {
    let room = theme::active();
    text(line.to_owned())
        .size(theme::SIZE_META)
        .line_height(theme::LEADING_META)
        .color(room.paper_dim)
        .width(Length::Fill)
        .wrapping(text::Wrapping::Word)
        .into()
}

/// The same, in the alert ink: something the listener needs to act on.
pub(crate) fn alert(line: &str) -> Element<'static, Message> {
    let room = theme::active();
    text(line.to_owned())
        .size(theme::SIZE_META)
        .line_height(theme::LEADING_META)
        .color(room.alert)
        .width(Length::Fill)
        .wrapping(text::Wrapping::Word)
        .into()
}

/// A caption naming the control beneath it, in the caption voice.
pub(crate) fn caption_word(word: &str) -> Element<'static, Message> {
    let room = theme::active();
    text(word.to_owned())
        .size(theme::SIZE_CAPTION)
        .line_height(theme::LEADING_CAPTION)
        .font(theme::MEDIUM)
        .color(room.paper_faint)
        .into()
}

/// One word naming the edge of a drawn control, in the quietest voice on it:
/// the picture is the subject and these are its edges.
pub(crate) fn axis_word(word: &'static str) -> Element<'static, Message> {
    let room = theme::active();
    text(word)
        .size(theme::SIZE_CAPTION)
        .line_height(theme::LEADING_CAPTION)
        .color(room.paper_muted)
        .wrapping(text::Wrapping::None)
        .into()
}

/// A quiet act: a bare word with hover as its affordance.
pub(crate) fn word_button_maybe<'a>(label: &str, message: Option<Message>) -> Element<'a, Message> {
    let room = theme::active();
    iced::widget::button(
        container(
            text(label.to_owned())
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .font(theme::MEDIUM),
        )
        .height(Length::Fill)
        .align_y(iced::alignment::Vertical::Center),
    )
    .padding(theme::pad(0.0, theme::GAP_SM))
    .height(Length::Fixed(theme::TRANSPORT_HIT))
    .style(move |_theme, status| theme::word_button(room, room.wall, status))
    .on_press_maybe(message)
    .into()
}

/// The playlist-name field, wherever a draft is named.
pub(crate) fn name_input(value: &str) -> Element<'_, Message> {
    let room = theme::active();
    iced::widget::text_input("Playlist name", value)
        .on_input(Message::PlaylistCreationName)
        .padding(theme::pad(theme::WELL_PAD_V, theme::GAP_MD))
        .size(theme::SIZE_BODY)
        .line_height(theme::LEADING_BODY)
        .style(move |_theme, status| theme::input(room, status))
        .into()
}

/// **A block's name inside a place** — the wall's own group band, in every
/// place that names a block.
///
/// The owner: *"the home page section headers and this pages section does not
/// match the library and playlist. i prefer the library and homepage."* Two
/// treatments existed for one job: this drew the scale's smallest tracked
/// caps over a hairline, while Library and Playlists drew
/// [`shelf::group_band`]'s 15 px tracked caps with no rule. Nine call sites
/// wore the first and the wall wore the second, and a listener walking
/// between them met two different ideas of what a heading is.
///
/// **This is the band now**, so the nine call sites converted by changing one
/// function. [`shelf::group_band`] stays a separate function rather than
/// calling this: it carries the wall's own geometry — a fixed header height
/// from the grid, and an optional door on the word — which a heading inside a
/// document does not have and should not pay for.
///
/// The hairline went with the size. It was there to separate a tiny caption
/// from the body under it; a 15 px band at the emphasis size separates itself,
/// and a rule under every heading in the product would be a line where the
/// type already is one.
///
/// The one structural rule beyond the three `.interface-design/system.md` §2
/// names, and it earns its place the way the Settings readout's does: it
/// divides two kinds of content inside one column. Shared by the record page's
/// `Tracks` and `Details` because a page whose two blocks named themselves
/// differently would read as two surfaces.
pub(crate) fn section_rule(name: &'static str) -> Element<'static, Message> {
    let room = theme::active();
    text(theme::tracked(&name.to_uppercase()))
        .size(theme::SIZE_EMPHASIS)
        .line_height(theme::LEADING_EMPHASIS)
        .font(theme::MEDIUM)
        .color(room.paper_dim)
        .into()
}

/// **The density detents** (ADR-0028, doc 11 §5 P8 — the owner's choice; the
/// fourth-step amendment of 2026-08-10): one mark per [`crate::shelf::Density`]
/// step, loosest first — the direction <kbd>Ctrl</kbd>+<kbd>=</kbd> walks.
///
/// # Where they stand
///
/// **In the app bar's display-options slot, in every place that hangs works**
/// (ADR-0040 §5), on the owner's instruction of 2026-08-10: *"and please put
/// the display options at the top bar"*.
///
/// That reverses ADR-0028's fourth-step amendment §3, which had them at the
/// trailing edge of the block of works they hang — the index rail's lane on
/// the Library, a section rule on Home and an artist's page — and which
/// refused the top bar in so many words. ADR-0040 §5 records the reversal and,
/// more importantly, the **one condition it keeps**: the marks are still
/// *absent* on the five places that hang no works, rather than present and
/// inert. What is resident is the bar and its reserved slot
/// ([`theme::APP_BAR_MARKS_W`]), not the control.
///
/// There is still exactly **one** of them. They are drawn once, in one place,
/// from one function, and they send one message — which is doc 07 L8.6's whole
/// requirement, and the reason the rail's foot and the two section rules gave
/// them up rather than keeping a copy.
///
/// # What each mark is
///
/// A [`theme::STEPPER_HIT`] box (law L7's named secondary) holding the
/// step's sprite — the wall itself at that hang: one work, four, nine,
/// sixteen. The current step is the full-ink mark ([`theme::GLYPH_OPACITY_HOVER`]
/// against the others' [`theme::GLYPH_OPACITY`]) — the group-key row's active
/// treatment translated to sprite ink, and **never the accent**: density is
/// not playback truth. The works themselves are the primary readout — their
/// own size states the step — so the lift confirms rather than carries.
///
/// # The press is the gesture's own message
///
/// A mark sends [`Message::DensityStep`] with [`crate::shelf::Density::steps_to`]'s
/// delta — the exact signed notch count the <kbd>Ctrl</kbd>+scroll /
/// <kbd>Ctrl</kbd>+<kbd>±</kbd> gesture would spend, making keys and wheel
/// *accelerators of a visible control* rather than the control itself
/// (the mirror rule, doc 07 L8.7; the product's standing rules as amended by
/// ADR-0028). The **active mark is inert** — pressing the step you are on
/// would do nothing, and a control that does nothing when pressed is the lie
/// the rail's absent letters already refuse. It is the fact; the others are
/// the controls (L8.3's split).
pub(crate) fn density_marks(current: crate::shelf::Density) -> Element<'static, Message> {
    // One axis now, and that is a simplification the move paid for: the run
    // used to be laid down the index rail's lane in one place and along a
    // section rule in two others, so it carried a `DetentAxis` to say which.
    // A bar is horizontal in every place there is, so the parameter went with
    // the placements that needed it.
    row(crate::shelf::Density::ALL.map(|step| density_mark(step, current))).into()
}

/// One detent of [`density_marks`]: the step's glyph in a
/// [`theme::STEPPER_HIT`] box, named by its tooltip (the icon-only law,
/// doc 10 §3.1 — the tooltip is the accessible name in a toolkit with no
/// accessibility tree), the hover wash [`theme::transport`]'s — the lane's
/// established press vocabulary, the same family as the spine's winner chip.
fn density_mark(
    step: crate::shelf::Density,
    current: crate::shelf::Density,
) -> Element<'static, Message> {
    use crate::shelf::Density;

    let room = theme::active();
    let active = step == current;
    let glyph = match step {
        Density::Spacious => crate::icon::Glyph::DensitySpacious,
        Density::Balanced => crate::icon::Glyph::DensityBalanced,
        Density::Compact => crate::icon::Glyph::DensityCompact,
        Density::Dense => crate::icon::Glyph::DensityDense,
    };
    let mark = container(
        iced_image(crate::icon::handle(glyph))
            .width(Length::Fixed(theme::ICON_PX))
            .height(Length::Fixed(theme::ICON_PX))
            .opacity(if active {
                theme::GLYPH_OPACITY_HOVER
            } else {
                theme::GLYPH_OPACITY
            }),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(alignment::Horizontal::Center)
    .align_y(alignment::Vertical::Center);
    // The active mark is the fact and takes no press; the others are the
    // controls and send the gesture's exact message (function docs).
    let boxed: Element<'static, Message> = if active {
        container(mark)
            .width(Length::Fixed(theme::STEPPER_HIT))
            .height(Length::Fixed(theme::STEPPER_HIT))
            .into()
    } else {
        iced::widget::button(mark)
            .width(Length::Fixed(theme::STEPPER_HIT))
            .height(Length::Fixed(theme::STEPPER_HIT))
            .padding(0)
            // The bar's ground, not the wall's — every control names the
            // surface it stands on, and this run moved surfaces.
            .style(move |_theme, status| theme::transport(room, room.recess, status))
            .on_press(Message::DensityStep(current.steps_to(step)))
            .into()
    };
    iced::widget::tooltip(
        boxed,
        text(step.label())
            .size(theme::SIZE_CAPTION)
            .line_height(theme::LEADING_CAPTION),
        // Below: the marks stand in the app bar, on the window's own top
        // edge, and a tip above any of them would clip off the screen — the
        // same reason the gear beside them tips downward.
        iced::widget::tooltip::Position::Bottom,
    )
    .gap(theme::GAP_XS)
    .padding(theme::GAP_XS)
    .style(move |_theme| theme::tooltip(room))
    .into()
}

#[cfg(test)]
mod tests {
    /// A joined list is prose, and prose gets a test.
    #[test]
    fn several_things_are_named_the_way_a_sentence_names_them() {
        let words = |all: &[&str]| {
            super::list_words(
                &all.iter()
                    .map(|word| (*word).to_owned())
                    .collect::<Vec<_>>(),
            )
        };
        assert_eq!(words(&[]), "");
        assert_eq!(words(&["energy"]), "energy");
        assert_eq!(words(&["energy", "tempo"]), "energy and tempo");
        assert_eq!(
            words(&["energy", "tempo", "texture"]),
            "energy, tempo and texture"
        );
    }

    /// **The frame is the frame in every place**, which this file claims in
    /// prose and did not hold for about a month.
    ///
    /// [`super::place_header_led`] lays out whatever lead it is handed, and
    /// what it is handed differs in kind: the Album place's breadcrumb and the
    /// Artist place's name are controls declaring [`theme::TRANSPORT_HIT`] 32,
    /// while a bare [`super::place_name`] is `LEADING_EMPHASIS` 20. So the
    /// strip came to 49 px under a control and 37 px under a word, and **Queue,
    /// Settings and the Artist place drew their whole bodies 12 px above** the
    /// Library and the two subject pages.
    ///
    /// Two assertions, because the defect had two halves. The arithmetic one
    /// says [`theme::TOP_BAR_H`] is still built from the control height — if
    /// that stops being true the box below is holding the lead to a number the
    /// frame no longer declares, which would be a *quieter* version of this
    /// same bug. The source one says the strip actually boxes its lead.
    ///
    /// Source-scanned rather than measured because the layout is iced's to
    /// perform and this crate cannot render in a unit test; the frames that
    /// found it are at `docs/design/impl/one-page-two-subjects/`.
    #[test]
    fn every_place_leads_at_the_height_the_frame_declares() {
        use crate::theme;
        assert!(
            (theme::TOP_BAR_H - 2.0f32.mul_add(theme::TOP_BAR_PAD_V, theme::TRANSPORT_HIT + 1.0))
                .abs()
                < f32::EPSILON,
            "the strip's declared height is no longer built from the control \
             height, so holding the lead to that height means nothing"
        );
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/views/mod.rs"),
        )
        .expect("this source")
        .replace("\r\n", "\n");
        let body = source
            .split_once("pub(crate) fn place_header_led")
            .expect("the shared strip")
            .1;
        let at = body
            .find("container(lead)")
            .expect("the shared strip boxes its lead");
        assert!(
            body[at..body.len().min(at + 200)].contains("theme::TRANSPORT_HIT"),
            "the lead's box is not the control height, so a place led by a word \
             and a place led by a control are two different strips again"
        );
    }

    /// Every string literal in the view sources' *code* lines — comments
    /// stripped — which is a conservative superset of what can ship on
    /// screen.
    fn shipped_strings() -> Vec<(String, String)> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(root.join("views"))
            .expect("the views directory")
            .map(|entry| entry.expect("entry").path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
            .collect();
        // The context menu's labels ship too, and they are built in `menu.rs`.
        files.push(root.join("menu.rs"));
        let mut found = Vec::new();
        for path in files {
            let name = path
                .file_name()
                .expect("a file name")
                .to_string_lossy()
                .into_owned();
            let source = std::fs::read_to_string(&path)
                .expect("a view source")
                .replace("\r\n", "\n");
            // Only what ships: test modules (this one included) may name the
            // room's words in order to ban them.
            let source = source
                .split("#[cfg(test)]")
                .next()
                .expect("a source has a head");
            for line in source.lines() {
                if line.trim_start().starts_with("//") {
                    continue;
                }
                // Walk the line's string literals, escapes respected.
                let bytes = line.as_bytes();
                let mut i = 0;
                while i < bytes.len() {
                    if bytes[i] == b'"' {
                        let mut j = i + 1;
                        let mut literal = String::new();
                        while j < bytes.len() && bytes[j] != b'"' {
                            if bytes[j] == b'\\' {
                                j += 1;
                            }
                            if j < bytes.len() {
                                literal.push(bytes[j] as char);
                            }
                            j += 1;
                        }
                        found.push((name.clone(), literal));
                        i = j + 1;
                    } else {
                        i += 1;
                    }
                }
            }
        }
        assert!(
            found.iter().any(|(_, s)| s.contains("Play album")),
            "the sweep must actually be seeing the shipped copy"
        );
        found
    }

    /// **One vocabulary** (doc 11 §5 P4): no word from the room-vocabulary
    /// list ships in user-facing copy. "The wall", "the hang", "the stack"
    /// and "marquee" are the corpus's own names for its ideas — correctly
    /// internal, like a stage crew's slang — and the one leak the critique
    /// found (*"Esc returns to the wall"* beside `‹ Library`, two names for
    /// one destination in one strip) is exactly what this pin keeps closed.
    ///
    /// **The licence list is now empty.** It held two entries, `Pull` and its
    /// offer line `The pull` — the only shipped copy the room's vocabulary was
    /// ever allowed into, on P9's *present-to-owner* footing. The owner
    /// answered P9 on 2026-08-10 by removing the control, so the exception
    /// went with the words it excepted and the rule is now total.
    /// `Save as playlist` / `Add to playlist…` are ordinary words and were
    /// never on it.
    #[test]
    fn no_room_vocabulary_ships_in_user_facing_copy() {
        let licensed: [&str; 0] = [];
        for (file, literal) in shipped_strings() {
            if licensed.contains(&literal.as_str()) {
                continue;
            }
            let lowered = literal.to_lowercase();
            for banned in ["wall", "hang", "marquee", "pull's", "the stack"] {
                // Word boundaries: "wall" must not hide in "wallpaper" and
                // fail the sweep for the wrong reason — every hit is read
                // as its own word.
                let hit = lowered
                    .split(|c: char| !c.is_alphanumeric() && c != '\u{2019}' && c != '\'')
                    .any(|word| word == banned)
                    || (banned.contains(' ') && lowered.contains(banned));
                assert!(
                    !hit,
                    "{file}: the literal {literal:?} ships the room's own \
                     word {banned:?} — plain words wherever the software \
                     speaks (doc 11 §5 P4; `02` §2.7)"
                );
            }
        }
    }

    /// **Every row-shaped control names the ground it stands on.**
    ///
    /// [`theme::track_row`]'s hover used to be the constant
    /// `Palette::plinth`, which is right for a row on the wall and mute for a
    /// row on the panel — whose own ground *is* `plinth`, so its rows painted
    /// the colour already under them. The owner named it (2026-08-09, *"a bit…
    /// unresponsive"*); the fix was to make the hover a *relation*
    /// (`Palette::step_up`), which only works if every call site says what it
    /// stands on.
    ///
    /// Asserted over the source because the failure is invisible in a
    /// rendering and silent in a type: a ground of the wrong plane compiles,
    /// draws, and answers the pointer with nothing. A future surface composed
    /// on a new plane fails the build rather than the review.
    #[test]
    fn every_row_shaped_control_names_the_ground_it_stands_on() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut sites = 0_u32;
        let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(root.join("views"))
            .expect("the views directory")
            .map(|entry| entry.expect("entry").path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
            .collect();
        files.sort();
        for path in files {
            let source = std::fs::read_to_string(&path)
                .expect("a view source")
                .replace("\r\n", "\n");
            let source = source
                .split("#[cfg(test)]")
                .next()
                .expect("a source has a head");
            let name = path.file_name().expect("a file name").to_string_lossy();
            for (at, _) in source.match_indices("theme::track_row(") {
                // Comments name the function too; only calls are call sites.
                let line_start = source[..at].rfind('\n').map_or(0, |index| index + 1);
                if source[line_start..at].trim_start().starts_with("//") {
                    continue;
                }
                let tail: String = source[at..].chars().take(80).collect();
                let arguments = tail
                    .split_once('(')
                    .map(|(_, rest)| rest.replace(['\n', ' '], ""))
                    .unwrap_or_default();
                assert!(
                    arguments.starts_with("room,room."),
                    "{name}: a row must name the surface it stands on — \
                     `theme::track_row(room, <ground>, …)` — and this one \
                     reads `{}`",
                    arguments.chars().take(40).collect::<String>()
                );
                sites += 1;
            }
        }
        // Not vacuous: the wall's rows, the panel's, the menu card's and the
        // returns lane's are all in the walk.
        assert!(sites >= 6, "only {sites} row call sites found");
    }

    /// **Every surface that draws a track row also draws its card.**
    ///
    /// The row's highlight used to come from the row's own button, which meant
    /// it stopped where the button did — and every surface that has a track row
    /// hangs controls off the side of it (a heart, the transfer `+`, an
    /// editable list's ▲▼✕). The owner, 2026-08-15: *"can we make sure the
    /// playlist row controls are inside the highlighted row as well."* So the
    /// card moved out to [`page::row_card`], which wraps the assembled row.
    ///
    /// The failure this refuses is a **silent** one: a new surface that calls
    /// [`page::track_row`] and forgets the wrapper compiles, draws, and simply
    /// never lights — [`theme::track_row_body`] paints nothing at all. There is
    /// no type that can catch it, so the source is read instead.
    #[test]
    fn every_track_row_is_wrapped_in_its_card() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut sites = 0_u32;
        let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(root.join("views"))
            .expect("the views directory")
            .map(|entry| entry.expect("entry").path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
            .collect();
        files.sort();
        for path in files {
            let source = std::fs::read_to_string(&path)
                .expect("a view source")
                .replace("\r\n", "\n");
            let source = source
                .split("#[cfg(test)]")
                .next()
                .expect("a source has a head");
            let name = path.file_name().expect("a file name").to_string_lossy();
            // `page.rs` is where both live; it defines them rather than using
            // them.
            if name == "page.rs" {
                continue;
            }
            let draws = source.contains("page::TrackRow {") || source.contains("page::TrackRow{");
            if !draws {
                continue;
            }
            assert!(
                source.contains("row_card("),
                "{name}: draws a track row and never wraps it in \
                 `page::row_card`, so its rows can never light"
            );
            sites += 1;
        }
        // Not vacuous: a record's page, a playlist's, the queue, Favourites and
        // the new-playlist draft all draw one.
        assert!(sites >= 5, "only {sites} track-row surfaces found");
    }

    /// **Every place that hangs works hangs them on one grid** — the shell's,
    /// resolved once, handed down.
    ///
    /// This is the assertion behind the defect the fourth-step work fixed.
    /// Home and an artist's page each resolved a grid of their own,
    /// `Grid::new(width − 2 × HANG, Density::Balanced)`, and it was wrong in
    /// two ways at once: it named a step outright, so the density control and
    /// both zoom keys did nothing on those pages; and its width was a
    /// hand-written guess at [`place_pad`]'s horizontals that missed the
    /// scrollbar lane. Measured at 1920 px with the returns lane collapsed it
    /// drew **six columns of 244 px art where the wall drew five of 294 px** —
    /// the same record, a press apart, 50 px different.
    ///
    /// So the rule is that a view file may not resolve a grid at all. The
    /// shell resolves [`crate::app::Shelf::grid`] and every place that hangs
    /// works is given it, which makes the sizes equal by construction rather
    /// than by two functions agreeing.
    ///
    /// Read off the source, the way the density marks' placement is: what is
    /// being pinned is the *composition*, and the composition is the code.
    #[test]
    fn every_place_that_hangs_works_hangs_them_on_one_grid() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        // The three places that hang works, and `page.rs` — the record's and
        // the playlist's shared composition, which hangs **rows** and must
        // therefore never grow a grid at all.
        for page in ["home.rs", "artist.rs", "shelf.rs", "page.rs"] {
            let source = std::fs::read_to_string(root.join("views").join(page))
                .expect("a view source")
                .replace("\r\n", "\n");
            let code = source
                .split("#[cfg(test)]")
                .next()
                .expect("a source has a head");
            for line in code.lines() {
                let line = line.trim_start();
                assert!(
                    line.starts_with("//") || !line.contains("Grid::new("),
                    "{page} resolves a grid of its own: `{line}`"
                );
            }
        }
        // …and the shell hands the two pages that used to the wall's own.
        let shell = std::fs::read_to_string(root.join("app.rs"))
            .expect("the shell's source")
            .replace("\r\n", "\n");
        let shell = shell.split("#[cfg(test)]").next().expect("a head");
        for call in ["views::artist::view(", "views::home::view("] {
            let at = shell.find(call).expect("the page is composed here");
            // The argument list, to its own closing parenthesis — the calls
            // are several lines each and a fixed window would either miss an
            // argument or run into the next arm.
            let tail = &shell[at + call.len()..];
            let mut depth = 1usize;
            let mut end = tail.len();
            for (index, ch) in tail.char_indices() {
                match ch {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            end = index;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            assert!(
                tail[..end].contains("state.grid()"),
                "{call} is not handed the wall's own grid"
            );
        }
    }
}
