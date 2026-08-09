//! **The returns lane** — the resident surface at the window's left edge
//! (ADR-0030, as the owner amended it).
//!
//! Three parts, top to bottom:
//!
//! 1. **The head** — `Home`, `Library`, `Now playing`, always all three and
//!    always in that order, and **the search well under them**. The owner's
//!    decisions: *"home will appear at the top of the left hand sidebar always
//!    either way and it will contain the top level concerns. think spotify"*,
//!    extended by *"as an extension we will want a Now playing page at the top
//!    with the Home and Library"*, and — the one this file's [`well`] answers
//!    — *"the design does not match properly… the search should really be in
//!    the sidebar"*.
//!    The place you are in is drawn in full paper ink; the other two rest at
//!    `paper_dim`. **`Now playing` carries the lamp dot when something is
//!    sounding** — the accent's one reserved meaning, spent so the lane can
//!    answer *is anything on?* without being read.
//! 2. **`RECENT`** — every playlist and the last
//!    [`crate::lane::RECENT_ALBUMS`] records, one list, last touched first
//!    ([`crate::lane`] owns the order and is tested without a window).
//! 3. **The two marks** at the foot: expanded and collapsed, in the density
//!    detents' exact anatomy (ADR-0028) — the current state's mark at full
//!    ink and **inert**, because it is the fact, and the other pressable,
//!    because it is the control.
//!
//! # Why the well is a field here and not a `Search` destination
//!
//! Spotify — the reference the owner keeps naming — makes `Search` a
//! destination you navigate to. baz must not, and the reason is a feature baz
//! has that Spotify does not: **type-anywhere** (ADR-0017 §1.2). Any printable
//! key filters the wall from anywhere in the product, so the query is already
//! open before you have decided to search; a destination row would say *go
//! somewhere first*, which is the opposite of what the product does, and it
//! would leave the thing the keystroke actually fills — the field — somewhere
//! else on screen.
//!
//! The well is also **as much a readout as an input**: it is where you read
//! what you asked for and how much of the collection answered. A readout of
//! the frame's own state belongs in the frame's own resident surface, and it
//! is the last piece of the frame that was still in the strip. With it moved,
//! the strip stops carrying identity — it is the wall's arrangement and the
//! wall's verbs, and nothing about the frame — and the eye has **one** place
//! to start, which is what the owner's *"the design does not match properly"*
//! was about.
//!
//! # Collapsed, the well is the magnifier, and pressing it opens the lane
//!
//! 96 px cannot hold a text field, so at [`theme::SIDEBAR_RAIL_W`] the well is
//! its own mark in the destinations' exact anatomy, and pressing it expands the
//! lane and puts the caret in the field. So do <kbd>/</kbd>,
//! <kbd>Ctrl</kbd>+<kbd>F</kbd> and the first key of a type-anywhere query.
//! That is Spotify's collapsed behaviour and it is defensible for the reason
//! the collapse itself is: it is **one frame, no tween** (§3.1), so the field
//! is under the caret in the same frame the press lands. The mark takes the
//! lit ink while a query stands, so a rail can still answer *is the wall
//! filtered?* without a word on it.
//!
//! Below [`theme::SIDEBAR_FLOOR`] the lane cannot open at all, so no magnifier
//! is drawn: there would be nothing for it to lead to. The well is in the strip
//! at those widths, in the form doc 10 §4.1 drew — [`theme::strip_holds_the_well`]
//! is the one predicate, and it is the lane's own floor rather than a second
//! breakpoint.
//!
//! # The collapse is a hard cut
//!
//! No tween (ADR-0030 §3.1). A width tween would re-resolve `Grid::new` on
//! every frame of the slide and pop columns mid-flight; one frame is cheaper
//! *and* better. This is the one press in the product that re-hangs the
//! collection, and it is safe because it lands **outside the wall** — no wall
//! gesture can be in flight when it fires.
//!
//! # The ground
//!
//! The lane stands on [`theme::Palette::recess`] — one plane *below* the wall
//! — so it reads as cut into the room rather than stuck onto it, with a
//! hairline on its right edge and nothing else. That choice is also what makes
//! its rows answer the pointer correctly for free: a row steps one plane up
//! from whatever it stands on ([`theme::Palette::step_up`]), so on the recess
//! a hovered row lands on the wall's own colour.

use iced::widget::{
    Space, button, column, container, horizontal_rule, image as iced_image, row, scrollable, text,
    text_input,
};
use iced::{Element, Length, alignment};

use crate::app::{Message, Shelf, search_id};
use crate::lane::{Destination, Subject, Touched};
use crate::place::Place;
use crate::playlists::Playlists;
use crate::views::playlist_sleeve;
use crate::{icon, theme};

/// The lane, at the width its state says.
///
/// `rows` is already resolved and ordered by [`crate::lane::resolve`] — this
/// function decides nothing about membership, which is what keeps the
/// ordering testable without a window.
pub(crate) fn view<'a>(
    shelf: &'a Shelf,
    playlists: &'a Playlists,
    place: Place,
    rows: &'a [Touched],
    sounding: bool,
    playing: Option<u64>,
    window_w: f32,
) -> Element<'a, Message> {
    let room = theme::active();
    let open = theme::sidebar_w(window_w, shelf.lane_open) >= theme::SIDEBAR_W;
    let width = theme::sidebar_w(window_w, shelf.lane_open);

    let mut head = column![];
    for to in Destination::ALL {
        head = head.push(destination_row(to, place, open, sounding));
    }
    // **The well, under the three destinations and above the seam.** It is in
    // the head because the head is the frame's own concerns and searching the
    // collection is one; it is *under* the destinations because the owner put
    // `Home` at the top of the lane and said so twice. Below
    // `SIDEBAR_FLOOR` the lane cannot open, so the head has no well and no
    // mark for one — the strip carries it there instead.
    if theme::sidebar_can_expand(window_w) {
        head = head.push(Space::with_height(Length::Fixed(theme::GAP_SM)));
        head = head.push(well(shelf, open));
    }

    let mut list = column![];
    for entry in rows {
        // **Which row is sounding** — doc 13 §2.6's claim, delivered. A list is
        // never "the sounding record" however many of its tracks are in the
        // run: the fact is about a record, and a list that lit because one of
        // its members was playing would be the invisible-pool posture in a
        // sleeve.
        let sounds = matches!(entry.subject, Subject::Record(id) if Some(id) == playing);
        list = list.push(lane_row(shelf, playlists, entry, open, sounds));
    }
    let list = scrollable(list)
        .direction(scrollable::Direction::Vertical(theme::wall_scrollbar()))
        .style(move |_theme, status| theme::scrollbar(room, room.recess, status))
        .width(Length::Fill)
        .height(Length::Fill);

    let body = column![
        head,
        // The head's one rule: three destinations above it, the things you
        // have touched below. The lane has exactly one seam because it has
        // exactly two parts.
        container(horizontal_rule(1).style(move |_theme| theme::hairline(room, room.recess)))
            .padding(theme::pad(theme::GAP_MD, 0.0)),
        heading(open),
        list,
        marks(shelf.lane_open, theme::sidebar_can_expand(window_w)),
    ]
    .width(Length::Fill)
    .height(Length::Fill);

    // The lane's own gutter is `GAP_XL` on both flanks — the measure both
    // widths are built from (`theme::SIDEBAR_W`) — never `HANG`: the lane is a
    // surface inside the window, not one hanging off its edge, and law L1's
    // window gutter belongs to the wall on the other side of it.
    let lane = container(body)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(theme::pad(theme::GAP_XL, theme::GAP_XL))
        .style(move |_theme| theme::lane_ground(room));

    row![
        lane,
        // The hairline on the right edge, in the lane's own width. Drawn as a
        // sibling rather than as a border because iced 0.13's `Border` is
        // four-sided, which is why every single line in the product is a rule.
        container(Space::new(Length::Fixed(1.0), Length::Fill))
            .height(Length::Fill)
            .style(move |_theme| theme::lane_seam(room)),
    ]
    .width(Length::Fixed(width))
    .height(Length::Fill)
    .into()
}

/// One of the head's three destinations: the glyph, and — expanded — its word.
///
/// **Not a toggle.** [`Place::go`] argues it: pressing the destination you are
/// already at leaves you there, where `Queue` and `Settings` close themselves.
/// The row is still a live control in that state rather than an inert mark,
/// which is the one place this departs from the density detents' anatomy —
/// a destination that stopped answering the pointer when you arrived would
/// read as broken, and pressing it costs nothing.
fn destination_row(
    to: Destination,
    place: Place,
    open: bool,
    sounding: bool,
) -> Element<'static, Message> {
    let room = theme::active();
    let here = place.destination() == Some(to);
    let glyph = match to {
        Destination::Home => icon::Glyph::Home,
        Destination::Library => icon::Glyph::Library,
        Destination::NowPlaying => icon::Glyph::NowPlaying,
    };
    let mark = iced_image(icon::handle(glyph))
        .width(Length::Fixed(theme::ICON_PX))
        .height(Length::Fixed(theme::ICON_PX))
        .opacity(if here {
            theme::GLYPH_OPACITY_HOVER
        } else {
            theme::GLYPH_OPACITY
        });
    // **The lamp dot, tucked against the glyph's corner** — and it survives
    // the collapse, which is the whole reason it is stacked on the glyph
    // rather than set after the word: collapsed there is no word to set it
    // after, and *is anything on?* is precisely the question a 96 px lane has
    // to keep answering.
    // Collapsed, the glyph is the only thing in the row, so it centres on the
    // lane's own axis — the same axis the sleeves below it centre on. Open, it
    // stands at the left of its box with the word after it.
    let glyph_x = if open {
        alignment::Horizontal::Left
    } else {
        alignment::Horizontal::Center
    };
    let boxed = |content: Element<'static, Message>, x| {
        container(content)
            .width(Length::Fixed(theme::SIDEBAR_GLYPH_BOX))
            .height(Length::Fixed(theme::SIDEBAR_GLYPH_BOX))
            .align_x(x)
    };
    let glyph_block: Element<'static, Message> = if to == Destination::NowPlaying && sounding {
        iced::widget::stack![
            boxed(mark.into(), glyph_x).align_y(alignment::Vertical::Center),
            boxed(lamp_dot(), alignment::Horizontal::Right).align_y(alignment::Vertical::Top),
        ]
        .into()
    } else {
        boxed(mark.into(), glyph_x)
            .align_y(alignment::Vertical::Center)
            .into()
    };
    let mut line = row![glyph_block]
        .spacing(theme::GAP_MD)
        .align_y(iced::Alignment::Center);
    if open {
        line = line.push(
            text(to.label())
                .size(theme::SIZE_BODY)
                .line_height(theme::LEADING_BODY)
                .font(theme::MEDIUM)
                .color(if here { room.paper } else { room.paper_dim })
                .wrapping(text::Wrapping::None),
        );
    }
    let row_button = button(
        container(line)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(glyph_x)
            .align_y(alignment::Vertical::Center)
            .clip(true),
    )
    .width(Length::Fill)
    .height(Length::Fixed(theme::SIDEBAR_DEST_H))
    // Collapsed, the row's flanks go: the glyph centres on the lane's own
    // axis, which is the axis the sleeves below it centre on, and a rail
    // whose head and body stood on two different verticals would read as two
    // surfaces.
    .padding(theme::pad(0.0, if open { theme::GAP_SM } else { 0.0 }))
    // The row family, on the lane's own ground: one step up under the
    // pointer, the whole hit area painted (`theme::track_row`).
    .style(move |_theme, status| theme::track_row(room, room.recess, status, here))
    .on_press(Message::GoTo(to));
    if open {
        return row_button.into();
    }
    // Collapsed, the word is the tooltip — the icon-only law (doc 10 §3.1):
    // a control with no visible label carries its name where a pointer can
    // find it.
    iced::widget::tooltip(
        row_button,
        text(to.label())
            .size(theme::SIZE_CAPTION)
            .line_height(theme::LEADING_CAPTION),
        iced::widget::tooltip::Position::Right,
    )
    .gap(theme::GAP_XS)
    .padding(theme::GAP_XS)
    .style(move |_theme| theme::tooltip(room))
    .into()
}

/// **The search well** — the lane's fourth head row, and the only field in the
/// frame.
///
/// # Expanded: a field over one quiet line
///
/// It takes the lane row's own anatomy — the body size over the meta size,
/// which is the wall label's own pair (doc 13 §2.6) — and that is what makes it read
/// as part of the lane rather than as a widget docked in it. The magnifier is
/// laid over the field's left padding as a `stack` (doc 10 §4.1: iced 0.13's
/// `text_input::Icon` is font-based and therefore not it), and the field's
/// left padding is [`theme::SIDEBAR_HEAD_TEXT_X`], so **the mark stands on the
/// destinations' glyph vertical and the query stands on their word vertical**.
/// Four head rows, two verticals.
///
/// # The counts and the match count are re-homed onto that quiet line
///
/// In the strip they rode *inside* the well: the counts as the placeholder,
/// the match count in a reserved [`crate::views::top_bar::MATCH_W`] 88 slot at
/// the right edge. Neither survives the move, and the reason is arithmetic
/// rather than taste. The lane's measure is [`theme::SIDEBAR_MEASURE`] 232
/// against the strip's 280, and 232 less the 44 px text inset less the 88 px
/// reserved slot leaves **100 px for the query itself** — a third of what doc
/// 10 §4.1 sized the slot to sit beside. So both figures come out of the well
/// and onto the line under it, where they get the whole measure and the query
/// gets the whole field:
///
/// - **at rest**: `25 albums · 206 tracks` — the corpus, under the glyph that
///   says *search this*, which is L8.3's valve exactly as the placeholder was.
/// - **narrowing**: `12 of 25 albums` — and the caption returns, because
///   outside the control being typed into `12 / 25` is a figure with no
///   subject. Doc 07 §3.1's own words, in doc 10's position.
///
/// The line is **always drawn** and left-aligned, which is the reserved-slot
/// discipline in its cheaper form: the first character never moves, the tail
/// shortens, and no lane row below is pushed down by a keystroke.
///
/// # Collapsed: the mark, and the press that opens the lane
///
/// The destination anatomy, tooltipped `Search`, sending
/// [`Message::FocusSearch`] — which expands the lane and lands the caret in
/// one frame (`app.rs`'s `focus_the_well`). The mark takes the lit ink while a
/// query stands, so the rail says *the wall is filtered* without a word.
fn well(shelf: &Shelf, open: bool) -> Element<'_, Message> {
    let room = theme::active();
    let filtering = !shelf.query.trim().is_empty();
    if !open {
        return collapsed_well(filtering);
    }
    let input = text_input("Search", &shelf.query)
        .id(search_id())
        .on_input(Message::SearchChanged)
        // Enter plays the top-ranked match, whichever road reached the query
        // (ADR-0017 §1.2, ADR-0021) — `crate::keys` binds the identical
        // message for a listener who typed from the wall.
        .on_submit(Message::PlayFirstMatch)
        .padding(iced::Padding {
            top: theme::WELL_PAD_V,
            right: theme::GAP_MD,
            bottom: theme::WELL_PAD_V,
            left: theme::SIDEBAR_HEAD_TEXT_X,
        })
        .size(theme::SIZE_BODY)
        .line_height(theme::LEADING_BODY)
        .width(Length::Fill)
        .style(move |_theme, status| theme::input(room, status));
    let magnifier = container(
        iced_image(icon::handle(icon::Glyph::Magnifier))
            .width(Length::Fixed(theme::ICON_PX))
            .height(Length::Fixed(theme::ICON_PX))
            .opacity(theme::GLYPH_OPACITY),
    )
    .height(Length::Fixed(theme::TRANSPORT_HIT))
    .padding(theme::pad(0.0, theme::SIDEBAR_WELL_GLYPH_LEAD))
    .align_y(alignment::Vertical::Center);
    container(
        column![
            iced::widget::stack![input, magnifier],
            container(
                text(readout(shelf, filtering))
                    .size(theme::SIZE_META)
                    .line_height(theme::LEADING_META)
                    .color(room.paper_faint)
                    .wrapping(text::Wrapping::None),
            )
            .width(Length::Fill)
            // The left inset is the head's word vertical; the right is the
            // field's own trailing padding, so the readout's lane is the
            // query's lane exactly.
            .padding(iced::Padding {
                top: 0.0,
                right: theme::GAP_MD,
                bottom: 0.0,
                left: theme::SIDEBAR_HEAD_TEXT_X,
            })
            .clip(true),
        ]
        .spacing(theme::GAP_XS),
    )
    .width(Length::Fill)
    .height(Length::Fixed(theme::SIDEBAR_WELL_H))
    .into()
}

/// The well at [`theme::SIDEBAR_RAIL_W`]: the mark alone, in the head's own
/// box, pressing to open the lane onto the caret.
fn collapsed_well(filtering: bool) -> Element<'static, Message> {
    let room = theme::active();
    let mark = container(
        iced_image(icon::handle(icon::Glyph::Magnifier))
            .width(Length::Fixed(theme::ICON_PX))
            .height(Length::Fixed(theme::ICON_PX))
            // Lit while a query stands — the one thing a 96 px lane can say
            // about the wall's state without a word on it.
            .opacity(if filtering {
                theme::GLYPH_OPACITY_HOVER
            } else {
                theme::GLYPH_OPACITY
            }),
    )
    .width(Length::Fixed(theme::SIDEBAR_GLYPH_BOX))
    .height(Length::Fixed(theme::SIDEBAR_GLYPH_BOX))
    .align_x(alignment::Horizontal::Center)
    .align_y(alignment::Vertical::Center);
    let row_button = button(
        container(mark)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(alignment::Horizontal::Center)
            .align_y(alignment::Vertical::Center)
            .clip(true),
    )
    .width(Length::Fill)
    .height(Length::Fixed(theme::SIDEBAR_DEST_H))
    .padding(0)
    .style(move |_theme, status| theme::track_row(room, room.recess, status, filtering))
    .on_press(Message::FocusSearch);
    // The icon-only law (doc 10 §3.1), the same clause that names the head's
    // three glyphs and the list's sleeves.
    iced::widget::tooltip(
        row_button,
        text("Search")
            .size(theme::SIZE_CAPTION)
            .line_height(theme::LEADING_CAPTION),
        iced::widget::tooltip::Position::Right,
    )
    .gap(theme::GAP_XS)
    .padding(theme::GAP_XS)
    .style(move |_theme| theme::tooltip(room))
    .into()
}

/// The well's quiet line: the collection at rest, the query answered while one
/// narrows the wall.
///
/// One function so the two strings are one decision — they share a line, a
/// size, an ink and a left edge, and a figure that changed voice between the
/// two states would be two readouts sharing a slot.
fn readout(shelf: &Shelf, filtering: bool) -> String {
    if filtering {
        format!("{} of {} albums", shelf.visible.len(), shelf.albums.len())
    } else {
        format!(
            "{} albums · {} tracks",
            shelf.albums.len(),
            shelf.library.len()
        )
    }
}

/// The lamp dot on `Now playing`: [`theme::DOT`], the accent, and nothing
/// else.
///
/// `docs/REFUSALS.md`'s amber entry is what licenses it — the lamp states what
/// is true about playback *right now*, and "something is sounding" is exactly
/// that fact. It is not lit by hover, by the queue holding music, or by the
/// place being on screen.
fn lamp_dot() -> Element<'static, Message> {
    let room = theme::active();
    container(Space::new(
        Length::Fixed(theme::DOT),
        Length::Fixed(theme::DOT),
    ))
    .style(move |_theme| theme::lamp_dot(room))
    .into()
}

/// `RECENT` — the lane's one word, in the room's quietest voice.
///
/// Absent when the lane is collapsed: at 96 px there is no measure for a
/// tracked word, and a heading over an unlabelled column of sleeves would be
/// naming nothing the eye can use.
fn heading(open: bool) -> Element<'static, Message> {
    let room = theme::active();
    if !open {
        return Space::with_height(Length::Fixed(theme::GAP_SM)).into();
    }
    container(
        text(theme::tracked("RECENT"))
            .size(theme::SIZE_HEADING)
            .line_height(theme::LEADING_HEADING)
            .font(theme::MEDIUM)
            .color(room.paper_faint),
    )
    .padding(theme::pad(0.0, theme::GAP_SM))
    .into()
}

/// One row of the lane: a sleeve, and — expanded — the name over one quiet
/// line.
///
/// **Nothing marks which kind a row is**, because the sleeve already does: a
/// record wears its cover, a playlist wears the 2 × 2 collage of the records
/// it quotes (ADR-0024 §A1). That is what makes a mixed list read as one list
/// rather than as two lists sharing a column.
fn lane_row<'a>(
    shelf: &'a Shelf,
    playlists: &'a Playlists,
    entry: &'a Touched,
    open: bool,
    playing: bool,
) -> Element<'a, Message> {
    let room = theme::active();
    // **[`theme::SIDEBAR_SLEEVE`] 48 in both states**, and that is a
    // correction: this read `PANEL_SLEEVE` 40 when open, which drew the
    // *panel's* sleeve in the lane and left [`theme::SIDEBAR_ROW_H`]'s own
    // derivation — 48 with one `GAP_SM` above and below — describing a row
    // that was not being drawn. Doc 13 §9.2's window drawing states 48 in the
    // expanded lane; `docs/design/impl/lane-and-home/01-lane-open-1280.png`
    // measures 40.
    let edge = theme::SIDEBAR_SLEEVE;
    let sleeve: Element<'a, Message> = match entry.subject {
        Subject::Record(id) => match shelf.thumbs.peek(&id) {
            Some(handle) => iced_image(handle.clone())
                .width(Length::Fixed(edge))
                .height(Length::Fixed(edge))
                .into(),
            None => crate::views::gradient_block(id, edge, 1.0),
        },
        // The list's sleeve is the panel's own — the same 2 × 2 collage of
        // the records it quotes, from the same cache (ADR-0024 §A1) — so a
        // list cannot look like two different objects in two surfaces.
        Subject::Playlist(id) => {
            let art = playlists.row(id).map_or(&[][..], |entry| &entry.art);
            playlist_sleeve(shelf, art, &entry.name, edge)
        }
    };
    let press = match entry.subject {
        Subject::Record(id) => Message::AlbumClicked(id),
        Subject::Playlist(id) => Message::OpenPlaylist(id),
    };
    // **The lamp dot before the name**, when this record is the one sounding
    // (doc 13 §2.6). The *row's* vocabulary rather than the tile's: the wall
    // marks a playing record with a halo around its art and a dot before its
    // title, and every row list in the product — the queue, a playlist's page —
    // marks it with the dot and the row's own card ([`theme::track_row`]'s
    // `playing`). A lane row is a row, so it takes the row's form; a warmed
    // halo would also need the lamp's own clock plumbed into a surface ADR-0030
    // §4 costs at zero idle CPU.
    let mut named = row![]
        .spacing(theme::GAP_XS)
        .align_y(iced::Alignment::Center);
    if playing && open {
        named = named.push(lamp_dot());
    }
    let body: Element<'a, Message> = if open {
        row![
            sleeve,
            container(
                column![
                    named.push(
                        text(entry.name.clone())
                            .size(theme::SIZE_BODY)
                            .line_height(theme::LEADING_BODY)
                            .font(theme::MEDIUM)
                            .color(room.paper)
                            .wrapping(text::Wrapping::None)
                    ),
                    text(entry.under.clone())
                        .size(theme::SIZE_META)
                        .line_height(theme::LEADING_META)
                        .color(room.paper_faint)
                        .wrapping(text::Wrapping::None),
                ]
                .spacing(theme::GAP_XXS)
            )
            .width(Length::Fill)
            .clip(true),
        ]
        .spacing(theme::GAP_SM)
        .align_y(iced::Alignment::Center)
        .into()
    } else {
        container(sleeve)
            .width(Length::Fill)
            .align_x(alignment::Horizontal::Center)
            .into()
    };
    let row_button = button(
        container(body)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_y(alignment::Vertical::Center)
            .clip(true),
    )
    .width(Length::Fill)
    .height(Length::Fixed(theme::SIDEBAR_ROW_H))
    .padding(theme::pad(0.0, if open { theme::GAP_SM } else { 0.0 }))
    // The card the sounding row keeps whatever the pointer is doing — and the
    // one mark that survives the collapse, where there is no name to set a dot
    // before and 96 px still has to answer *which of these is on?*
    .style(move |_theme, status| theme::track_row(room, room.recess, status, playing))
    .on_press(press);
    if open {
        return row_button.into();
    }
    // Collapsed the sleeve is the only identification, so the name is the
    // tooltip — the same clause that names the head's glyphs.
    iced::widget::tooltip(
        row_button,
        text(entry.name.clone())
            .size(theme::SIZE_CAPTION)
            .line_height(theme::LEADING_CAPTION),
        iced::widget::tooltip::Position::Right,
    )
    .gap(theme::GAP_XS)
    .padding(theme::GAP_XS)
    .style(move |_theme| theme::tooltip(room))
    .into()
}

/// **The two marks** at the lane's foot (ADR-0030 §3), in the density
/// detents' exact anatomy: [`theme::STEPPER_HIT`] boxes, the current state's
/// mark at full ink and inert — it is the fact — the other at the resting ink
/// and pressable, because it is the control.
///
/// Below [`theme::SIDEBAR_FLOOR`] the `Expanded` mark is inert whichever state
/// is stored: expanding there would leave the collection one column of covers,
/// and a control that produces a state the window cannot hold is a trap. It
/// draws at the disabled ink so the inertness is visible rather than
/// discovered by pressing.
fn marks(open: bool, can_expand: bool) -> Element<'static, Message> {
    let mut marks = row![].spacing(theme::GAP_XS);
    for expanded in [true, false] {
        marks = marks.push(lane_mark(expanded, open, can_expand));
    }
    container(marks)
        .width(Length::Fill)
        .align_x(alignment::Horizontal::Center)
        .padding(theme::pad(theme::GAP_MD, 0.0))
        .into()
}

/// One of [`marks`]' two marks.
fn lane_mark(expanded: bool, open: bool, can_expand: bool) -> Element<'static, Message> {
    let room = theme::active();
    let current = expanded == open;
    let usable = !expanded || can_expand;
    let glyph = if expanded {
        icon::Glyph::LaneExpanded
    } else {
        icon::Glyph::LaneCollapsed
    };
    let opacity = if current {
        theme::GLYPH_OPACITY_HOVER
    } else if usable {
        theme::GLYPH_OPACITY
    } else {
        theme::GLYPH_OPACITY_DISABLED
    };
    let mark = container(
        iced_image(icon::handle(glyph))
            .width(Length::Fixed(theme::ICON_PX))
            .height(Length::Fixed(theme::ICON_PX))
            .opacity(opacity),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(alignment::Horizontal::Center)
    .align_y(alignment::Vertical::Center);
    let boxed: Element<'static, Message> = if current || !usable {
        container(mark)
            .width(Length::Fixed(theme::STEPPER_HIT))
            .height(Length::Fixed(theme::STEPPER_HIT))
            .into()
    } else {
        button(mark)
            .width(Length::Fixed(theme::STEPPER_HIT))
            .height(Length::Fixed(theme::STEPPER_HIT))
            .padding(0)
            .style(move |_theme, status| theme::transport(room, room.recess, status))
            .on_press(Message::ToggleLane)
            .into()
    };
    let name = if expanded { "Expanded" } else { "Collapsed" };
    iced::widget::tooltip(
        boxed,
        text(name)
            .size(theme::SIZE_CAPTION)
            .line_height(theme::LEADING_CAPTION),
        iced::widget::tooltip::Position::Top,
    )
    .gap(theme::GAP_XS)
    .padding(theme::GAP_XS)
    .style(move |_theme| theme::tooltip(room))
    .into()
}

#[cfg(test)]
mod tests {
    /// This file's own source, for the pins below.
    fn source() -> String {
        include_str!("lane.rs").replace("\r\n", "\n")
    }

    /// One function's body, by name.
    fn body(source: &str, signature: &str) -> String {
        let rest = source
            .split_once(signature)
            .unwrap_or_else(|| panic!("`{signature}` exists"))
            .1;
        rest[..rest.find("\n}\n").expect("a function ends")].to_owned()
    }

    /// **The well is in the head, and it is in the head only where the lane
    /// can hold it.**
    ///
    /// The owner's decision — *"the search should really be in the sidebar"* —
    /// and its one boundary. Below [`theme::SIDEBAR_FLOOR`] the lane cannot
    /// open, so a magnifier there would lead nowhere and the strip carries the
    /// well instead ([`theme::strip_holds_the_well`], the single predicate).
    /// **Never both**: two wells would be the defect the move was made to
    /// close, with an extra field.
    #[test]
    fn the_well_stands_in_the_head_wherever_the_lane_can_hold_it() {
        let source = source();
        let view = body(&source, "pub(crate) fn view<'a>(");
        let head = view
            .split_once("for to in Destination::ALL")
            .expect("the head's three destinations")
            .1;
        let (head, _) = head.split_once("let mut list").expect("the head ends");
        assert!(
            head.contains("theme::sidebar_can_expand(window_w)"),
            "the head's well is not conditioned on the lane being able to open"
        );
        assert!(
            head.contains("head.push(well(shelf, open))"),
            "the well is not pushed onto the head, under the destinations"
        );
        // And it is *under* them: the owner put `Home` at the top of the lane
        // and said so twice (ADR-0030's amendment).
        let at_destinations = view
            .find("for to in Destination::ALL")
            .expect("the destinations");
        let at_well = view.find("head.push(well(").expect("the well");
        assert!(
            at_destinations < at_well,
            "the well is drawn above the three destinations"
        );
    }

    /// **Collapsed, the well is the mark, and the mark opens the lane onto the
    /// caret.**
    ///
    /// 96 px cannot hold a field. What it can hold is the destination
    /// anatomy — the same box, the same tooltip clause (doc 10 §3.1) — and one
    /// press that spends [`Message::FocusSearch`], which is the same message
    /// <kbd>/</kbd> and <kbd>Ctrl</kbd>+<kbd>F</kbd> spend. One road, three
    /// doors: the mirror rule.
    #[test]
    fn the_collapsed_well_is_a_mark_that_opens_the_lane() {
        let source = source();
        let well = body(&source, "fn well(shelf: &Shelf, open: bool)");
        assert!(
            well.contains("if !open {\n        return collapsed_well(filtering);"),
            "the collapsed lane still tries to draw a text field"
        );
        let collapsed = body(&source, "fn collapsed_well(filtering: bool)");
        assert!(
            collapsed.contains("icon::Glyph::Magnifier"),
            "the collapsed well is not the magnifier"
        );
        assert!(
            collapsed.contains(".on_press(Message::FocusSearch)"),
            "the collapsed well's press is not the one `/` and Ctrl+F send"
        );
        assert!(
            collapsed.contains("theme::SIDEBAR_DEST_H"),
            "the collapsed well is not a head row's height"
        );
        assert!(
            collapsed.contains("tooltip") && collapsed.contains("\"Search\""),
            "an icon-only control with no name (doc 10 §3.1)"
        );
        // The rail's one word about the wall's state: lit while a query
        // stands, resting otherwise.
        assert!(
            collapsed.contains("if filtering {\n                theme::GLYPH_OPACITY_HOVER"),
            "the collapsed mark does not answer whether the wall is filtered"
        );
    }

    /// **The two figures are on the readout line, never back inside the
    /// field** — the re-homing the lane's 232 px measure forced.
    ///
    /// `MATCH_W` 88 inside a 232 px well would leave the query 100 px. Both
    /// figures went onto the line under the field, which is always drawn so
    /// that the first keystroke moves no row below it, and left-aligned so the
    /// figures change in place.
    #[test]
    fn the_wells_figures_share_one_always_drawn_line() {
        let source = source();
        let well = body(&source, "fn well(shelf: &Shelf, open: bool)");
        assert!(
            !well.contains("MATCH_W"),
            "the lane's well reserved the strip's match slot inside itself"
        );
        assert!(
            well.contains("text_input(\"Search\", &shelf.query)"),
            "the lane's well no longer names itself in its placeholder"
        );
        assert!(
            well.contains("readout(shelf, filtering)"),
            "the readout line is not drawn"
        );
        assert!(
            well.contains("Length::Fixed(theme::SIDEBAR_WELL_H)"),
            "the well's block is not held at a fixed height, so a keystroke \
             would push the lane's rows down"
        );
        let readout = body(&source, "fn readout(shelf: &Shelf, filtering: bool)");
        assert!(
            readout.contains("{} of {} albums") && readout.contains("{} albums · {} tracks"),
            "the readout lost one of its two states"
        );
    }

    /// **A lane row's sleeve is [`theme::SIDEBAR_SLEEVE`] in both states.**
    ///
    /// It was [`theme::PANEL_SLEEVE`] 40 when open, which drew the *panel's*
    /// sleeve in the lane and left [`theme::SIDEBAR_ROW_H`]'s derivation — 48
    /// with one `GAP_SM` above and below — describing a row nothing was drawing.
    #[test]
    fn a_lane_rows_sleeve_is_the_lanes_own_size() {
        let source = source();
        let row = body(&source, "fn lane_row<'a>(");
        assert!(
            row.contains("let edge = theme::SIDEBAR_SLEEVE;"),
            "the lane's sleeve is resolved per state again"
        );
        assert!(
            !row.contains("theme::PANEL_SLEEVE"),
            "the lane draws the playlist panel's sleeve"
        );
    }

    /// **The sounding record is marked in the lane** — doc 13 §2.6's claim,
    /// which the shipped lane did not keep: every row drew
    /// [`theme::track_row`] with `playing` hard-coded `false`, so the surface
    /// whose whole subject is *things you have touched* could not say which of
    /// them was on.
    ///
    /// The dot before the name and the row's card, which is the **row's**
    /// vocabulary — what the queue and a playlist's page already draw — rather
    /// than the tile's halo, which would want the lamp's clock in a surface
    /// ADR-0030 §4 costs at zero idle CPU. And a **record** only: a list is
    /// never "the sounding record" however many of its tracks are in the run.
    #[test]
    fn the_sounding_record_is_the_marked_row() {
        let source = source();
        let view = body(&source, "pub(crate) fn view<'a>(");
        assert!(
            view.contains("Subject::Record(id) if Some(id) == playing"),
            "the lane no longer asks which of its rows is sounding, or asks it \
             of lists as well as records"
        );
        let row = body(&source, "fn lane_row<'a>(");
        assert!(
            row.contains("if playing && open {") && row.contains("named.push(lamp_dot())"),
            "the sounding row lost its lamp dot"
        );
        assert!(
            row.contains("theme::track_row(room, room.recess, status, playing)"),
            "the sounding row lost the card that survives the collapse"
        );
    }
}
