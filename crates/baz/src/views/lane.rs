//! **The returns lane** — the resident surface at the window's left edge
//! (ADR-0030, as the owner amended it).
//!
//! Three parts, top to bottom:
//!
//! 1. **The head** — **the search well**, then `Home`, `Library`,
//!    `Now playing`, always all three and always in that order. The owner's
//!    decisions: *"home will appear at the top of the left hand sidebar always
//!    either way and it will contain the top level concerns. think spotify"*,
//!    extended by *"as an extension we will want a Now playing page at the top
//!    with the Home and Library"*, and — the one this file's [`well`] answers
//!    — *"the design does not match properly… the search should really be in
//!    the sidebar"*, then *"search belongs at the top"*.
//!    The place you are in is drawn in full paper ink; the other two rest at
//!    `paper_dim`. **`Now playing` carries the lamp dot when something is
//!    sounding** — the accent's one reserved meaning, spent so the lane can
//!    answer *is anything on?* without being read.
//! 2. **`PLAYLISTS`**, then **`RECENT`** — every list, then the last
//!    [`crate::lane::RECENT_ALBUMS`] records, each section last touched first
//!    ([`crate::lane`] owns the membership and the order, and is tested
//!    without a window). **One scroller over both**, for the reason in
//!    [`sections`].
//! 3. **`Collapse`** at the foot — one control, a chevron and its word, the
//!    chevron pointing the way the lane will move. Collapsed it is the
//!    chevron alone under its tooltip, like the destinations above it.
//!
//! # Why the lists have a section, when they used to be mixed in
//!
//! The owner, 2026-08-10: *"I guess we need to add playlists into their own
//! section under library"*. This **reverses his own brief** for this surface —
//! *"the side bar will have recent albums and playlists mixed based on some
//! order"* — and the reversal is recorded rather than smoothed over
//! (ADR-0030's sixth amendment). What it changes is **membership**: `RECENT`
//! is records now, `PLAYLISTS` is lists, and nothing is in both, because a
//! list drawn in two sections would be one door drawn twice.
//!
//! What it does **not** change is the key: both sections are last touched
//! first, which is the one total order [`crate::lane`] has always had. A list
//! you played this morning is still the top row of the lists; it moved
//! section, not rank — so the recency the mixed list gave him is not spent to
//! buy the heading.
//!
//! *"Under library"* is read as **under the head**, not literally between
//! `Library` and `Now playing`: the three destinations are a closed set
//! *always all three and always in that order* (§1 above, ADR-0030's first
//! amendment), and a section between two of them would split the one triple
//! this surface is not allowed to grow.
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
//! what you asked for and how much of the collection answered — the match
//! count sits inside the field, which is what [`well`] argues. A readout of
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
use crate::views::{clear_mark, playlist_sleeve};
use crate::{icon, theme};

/// **What the well searches, said in the field itself** — ADR-0036 §2.
///
/// The well is resident in all eight places and searches exactly one of them,
/// so `Search` alone was a promise about the control rather than about its
/// subject. `Search library` is the noun on the destination row two rows below
/// it, which is where the query lands.
///
/// Set in the field's *placeholder* lane, which is free exactly when the query
/// is empty — the same slot the collection's counts held before they left for
/// Home, and the whole reason this costs nothing.
pub(crate) const SCOPE: &str = "Search library";

/// The lane, at the width its state says.
///
/// `lane` is already resolved, split and ordered by [`crate::lane::resolve`] —
/// this function decides nothing about membership, which is what keeps the
/// ordering testable without a window.
pub(crate) fn view<'a>(
    shelf: &'a Shelf,
    playlists: &'a Playlists,
    place: Place,
    lane: &'a crate::lane::Lane,
    sounding: bool,
    sounding_row: Option<Subject>,
    window_w: f32,
) -> Element<'a, Message> {
    let room = theme::active();
    let open = theme::sidebar_w(window_w, shelf.lane_open) >= theme::SIDEBAR_W;
    let width = theme::sidebar_w(window_w, shelf.lane_open);

    // **The well leads the lane** — *"search belongs at the top"* (owner,
    // 2026-08-09). It shipped under the three destinations, on the reading
    // that `Home` being top-of-lane put everything else below it; the owner
    // read the built thing and placed the well above them instead, which is
    // also where every product he named for reference puts it.
    //
    // It is in the head because the head is the frame's own concerns and
    // searching the collection is one. Below `SIDEBAR_FLOOR` the lane cannot
    // open, so the head has no well and no mark for one — the strip carries it
    // there instead ([`theme::strip_holds_the_well`]).
    let mut head = column![];
    if theme::sidebar_can_expand(window_w) {
        head = head.push(well(shelf, open));
        head = head.push(Space::with_height(Length::Fixed(theme::GAP_SM)));
    }
    for to in Destination::ALL {
        head = head.push(destination_row(to, place, open, sounding));
    }

    let body_rows = sections(shelf, playlists, lane, open, sounding_row);
    // **The rows carry the lane's gutter, not the scrollable**, so the bar
    // rides the lane's own right edge — the owner's *"the scrollbar should be
    // at the edge of it"*. A scrollbar inset by the gutter reads as belonging
    // to the list rather than to the surface, and leaves a dead strip between
    // it and the seam. The rows keep the inset; only the bar reaches the edge.
    let list = scrollable(body_rows.padding(theme::pad(0.0, theme::GAP_XL)))
        .direction(scrollable::Direction::Vertical(theme::wall_scrollbar()))
        .style(move |_theme, status| theme::scrollbar(room, room.recess, status))
        .width(Length::Fill)
        .height(Length::Fill);

    let flanked = |e: Element<'a, Message>| {
        container(e)
            .width(Length::Fill)
            .padding(theme::pad(0.0, theme::GAP_XL))
    };
    let body = column![
        flanked(head.into()),
        // The head's one rule: three destinations above it, the things you
        // have touched below. **The lane still has exactly one seam**, and
        // that is the point of drawing the sections' headings rather than a
        // second rule: a heading names a section, a rule cuts the surface, and
        // there is one cut here because there are two parts — the frame's
        // concerns, and yours.
        container(horizontal_rule(1).style(move |_theme| theme::hairline(room, room.recess)))
            .padding(theme::pad(theme::GAP_MD, theme::GAP_XL)),
        list,
        flanked(marks(shelf.lane_open, theme::sidebar_can_expand(window_w))),
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
        .padding(theme::pad(theme::GAP_XL, 0.0))
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
    let mut glyph_box = boxed(mark.into(), glyph_x).align_y(alignment::Vertical::Center);
    // Collapsed, the place you are in is marked by a card **the size of the
    // glyph**, not by a band across the rail — see [`theme::lane_current`] for
    // why the band was wrong and why it carries no border.
    if here && !open {
        glyph_box = glyph_box.style(move |_theme| theme::lane_current(room));
    }
    let glyph_block: Element<'static, Message> = if to == Destination::NowPlaying && sounding {
        iced::widget::stack![
            glyph_box,
            boxed(lamp_dot(), alignment::Horizontal::Right).align_y(alignment::Vertical::Top),
        ]
        .into()
    } else {
        glyph_box.into()
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
    // `here` only carries the row's card while there is a word in the row to
    // stand in it; collapsed the mark is the glyph's own box, above.
    .style(move |_theme, status| theme::track_row(room, room.recess, status, here && open))
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
/// # Expanded: one field, one control tall
///
/// The magnifier is laid over the field's left padding as a `stack` (doc 10
/// §4.1: iced 0.13's `text_input::Icon` is font-based and therefore not it),
/// and the field's left padding is [`theme::SIDEBAR_HEAD_TEXT_X`], so **the
/// mark stands on the destinations' glyph vertical and the query stands on
/// their word vertical**. Four head rows, two verticals.
///
/// # The two figures separate: one is a statistic, one is feedback
///
/// The well shipped with a quiet line under it carrying both — `25 albums ·
/// 206 tracks` at rest, `12 of 25 albums` while narrowing — and the owner read
/// the built thing: *"the album and track count below the search bar doesn't
/// look good… maybe this should go into the home as some basic stats?"*. He is
/// right, and the reason is that the two states were never one readout:
///
/// - **The resting counts are a statistic about the collection.** Nothing is
///   being searched when they are on screen, and they were standing in the
///   lane's most valuable space — above the records you actually return to.
///   They are the Home place's `COLLECTION` footer now
///   ([`crate::views::home`]), where a fact about the whole library belongs.
/// - **The match count is feedback about the query**, so it stays with the
///   field that answers it — and it goes **inside** the field, right-aligned
///   in a reserved [`theme::SIDEBAR_MATCH_W`] 72 slot. That is the ordinary
///   anatomy of a search input, it is doc 07 §3.1's own prescription (`12 / 25`
///   reads inside the control it is about, where the query is its subject), and
///   it costs no line at all.
///
/// The slot is the lane's own 72 rather than the strip's
/// [`crate::views::top_bar::MATCH_W`] 88, and that is what makes it fit where
/// the shipped design said it would not:
///
/// ```text
///   SIDEBAR_MEASURE                     232
///     − SIDEBAR_HEAD_TEXT_X              44
///     − GAP_MD                           12
///     − SIDEBAR_MATCH_W                  72     (was MATCH_W 88)
///     = the query's own room            104     (was 88)
/// ```
///
/// **Nothing moves when the first character lands.** The reservation is on the
/// *right* and the query sets from the left, so the caret does not shift; the
/// well's block is [`theme::SIDEBAR_WELL_H`], one control tall in both states,
/// so no `RECENT` row below is pushed down; and the slot is a fixed width with
/// the figures right-aligned in it, so `12 / 25` becoming `3 / 25` changes in
/// place.
///
/// # Collapsed: the mark, and the press that opens the lane
///
/// The destination anatomy, tooltipped `Search`, sending
/// [`Message::FocusSearch`] — which expands the lane and lands the caret in
/// one frame (`app.rs`'s `focus_the_well`). The mark takes the lit ink while a
/// query stands, so the rail says *the wall is filtered* without a word.
///
/// # The placeholder names the scope, because the well is resident in seven
/// places and searches one
///
/// ADR-0036. The owner asked what search should do off the Library — *"should
/// it just pop to the library view when you start typing? or should it search
/// whatever page you are on?"* — and the answer is that it keeps the one
/// meaning it already has: **it searches the collection**, from anywhere, and
/// the road there is `app.rs`'s `reach_the_well`. What was missing was
/// that the field never *said* so. It said `Search`, which is a promise about
/// the control and not about its subject, and a field that reads `Search` while
/// the window is showing `Road Trip` is fairly read as offering to search
/// `Road Trip`.
///
/// So it reads **`Search library`**, in every place, permanently — the same
/// noun as the destination row two rows below it, which is the thing the query
/// will take you to. It costs no chrome and no pixel: the placeholder is drawn
/// exactly when the query is empty, which is exactly when the count's
/// [`theme::SIDEBAR_MATCH_W`] slot is *not* reserved, so the lane it sets in is
/// the field's whole 176 px rather than the 104 a query gets
/// (`crate::font`'s `the_lanes_well_names_the_scope_it_searches`).
///
/// # The clear mark takes the magnifier's box
///
/// The `×` the owner asked for, and it is on the **left**, in the mark's own
/// slot, because the right-hand furniture is full and the query's room is the
/// scarce thing. The right of the field is `GAP_MD` 12 + `SIDEBAR_MATCH_W` 72,
/// sized for `1284 / 1284` at a library ten times the owner's, and putting a
/// glyph box beside it would take the query's own room from the 104 px that
/// justified this arrangement down to 80 — *below* the 88 the design measured
/// and rejected when the count was moved into the field. The mark's box is
/// already paid for, and it is 24 px wide on the destinations' glyph vertical,
/// so the swap moves nothing at all: at rest the box holds the magnifier, which
/// is a label saying *this field searches*; with a query standing it holds the
/// cross, which is a control saying *press to stop*. A field with text and a
/// count in it does not need to be told it is a search field.
fn well(shelf: &Shelf, open: bool) -> Element<'_, Message> {
    let room = theme::active();
    let filtering = !shelf.query.trim().is_empty();
    if !open {
        return collapsed_well(filtering);
    }
    // **The placeholder names what the field searches** (ADR-0036 §2). The
    // well is resident in every place and searches exactly one of them, so the
    // one line it owes a listener standing on a playlist is which.
    let input = text_input(SCOPE, &shelf.query)
        .id(search_id())
        .on_input(Message::SearchChanged)
        // Enter plays the top-ranked match, whichever road reached the query
        // (ADR-0017 §1.2, ADR-0021) — `crate::keys` binds the identical
        // message for a listener who typed from the wall.
        .on_submit(Message::PlayFirstMatch)
        .padding(iced::Padding {
            top: theme::WELL_PAD_V,
            // While a query narrows the collection the match count holds a
            // reserved slot at the field's right edge, and the input's own
            // padding is what keeps the query out of it. At rest there is no
            // count, so the field is the query's whole width.
            right: if filtering {
                theme::GAP_MD + theme::SIDEBAR_MATCH_W
            } else {
                theme::GAP_MD
            },
            bottom: theme::WELL_PAD_V,
            left: theme::SIDEBAR_HEAD_TEXT_X,
        })
        .size(theme::SIZE_BODY)
        .line_height(theme::LEADING_BODY)
        .width(Length::Fill)
        .style(move |_theme, status| theme::input(room, status));
    // The mark's box, in one of its two states — the label at rest, the clear
    // control while a query stands. Both stand on [`theme::SIDEBAR_HEAD_GLYPH_X`],
    // the destinations' own glyph vertical, so the swap is a change of meaning
    // and not of geometry.
    let mark: Element<'_, Message> = if filtering {
        container(clear_mark(room.recess))
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
        .padding(theme::pad(0.0, theme::SIDEBAR_WELL_GLYPH_LEAD))
        .align_y(alignment::Vertical::Center)
        .into()
    };
    let mut layers = iced::widget::stack![input, mark];
    if filtering {
        // **The match count, inside the control being typed into.** Right
        // aligned in a fixed slot, so the figure shrinking from `12 / 25` to
        // `3 / 25` moves nothing; `paper_faint`, which is a readout's ink and
        // never a control's.
        layers = layers.push(
            container(
                container(
                    text(match_count(shelf))
                        .size(theme::SIZE_META)
                        .line_height(theme::LEADING_META)
                        .color(room.paper_faint)
                        .wrapping(text::Wrapping::None),
                )
                .width(Length::Fixed(theme::SIDEBAR_MATCH_W))
                .align_x(alignment::Horizontal::Right)
                .clip(true),
            )
            .width(Length::Fill)
            .height(Length::Fixed(theme::TRANSPORT_HIT))
            .padding(theme::pad(0.0, theme::GAP_MD))
            .align_x(alignment::Horizontal::Right)
            .align_y(alignment::Vertical::Center),
        );
    }
    container(layers)
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

/// **The query, answered**: `3 / 25`, in the well's reserved right-hand slot.
///
/// The pair rather than the bare figure, and the slash rather than a caption:
/// inside the control being typed into the query is the count's subject, so
/// `3 / 25` needs no word to say what it counts — and the denominator is what
/// turns *three* into *three of a small collection*, which is the whole of what
/// a match count is for. Doc 07 §3.1's own form, in the position it was written
/// for — and the identical string the strip's own well has drawn all along
/// (`views::top_bar`'s `match_count`), so the two regimes answer one query one
/// way.
///
/// The `Songs` section states its own count in its own heading, as it always
/// did.
fn match_count(shelf: &Shelf) -> String {
    format!("{} / {}", shelf.visible.len(), shelf.albums.len())
}

/// The lamp dot on `Now playing`: [`theme::DOT`], the accent, and nothing
/// else.
///
/// the product's amber entry is what licenses it — the lamp states what
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

/// **The lane's body: `PLAYLISTS`, then `RECENT`, inside one scroller.**
///
/// The owner's *"I guess we need to add playlists into their own section under
/// library"*, and the whole of what it costs — a heading each, a `GAP_MD`
/// between them, and one predicate for a section that has nothing in it.
///
/// # One scroller over both, and this is the load-bearing decision
///
/// `PLAYLISTS` has **no cap**: [`crate::lane::RECENT_ALBUMS`] trims the
/// records and nothing trims the lists, because the lane is the complete index
/// of them (ADR-0030 §1). So a listener with forty lists has a first section
/// taller than the lane, and the arrangement has to answer *where does `RECENT`
/// go?*
///
/// Two scrollers — one per section, each taking half the height — is the
/// obvious answer and it is wrong: it would give the surface two scroll
/// positions to arbitrate between, which is the failure mode ADR-0030 §1 wrote
/// the whole one-order rule against, and it would cap the lists after all, at
/// half a lane. A fixed-height `PLAYLISTS` block above a scrolling `RECENT` is
/// worse still: past a dozen lists it pushes `RECENT` off the bottom entirely
/// and the records become unreachable.
///
/// **So both sections and both headings are inside the one scroller the lane
/// has always had.** The headings scroll away with their rows, which is what a
/// heading in a scrolling column does everywhere else in the product; every row
/// of both sections is reachable at any list count; and the lane still has one
/// scroll position, one bar, and nothing to arbitrate.
///
/// # A section with nothing in it is absent, not empty
///
/// ADR-0030 §6's rule for the home band, applied here for the same reason: a
/// heading over no rows names nothing. On a first run both are absent and the
/// lane below the hairline is bare, which is the truth about a library nobody
/// has played yet — a permanent `PLAYLISTS` word over a permanent gap would be
/// chrome that never becomes content.
fn sections<'a>(
    shelf: &'a Shelf,
    playlists: &'a Playlists,
    lane: &'a crate::lane::Lane,
    open: bool,
    sounding_row: Option<Subject>,
) -> iced::widget::Column<'a, Message> {
    let mut body = column![];
    let mut drawn = 0;
    for (word, rows) in [("PLAYLISTS", &lane.lists), ("RECENT", &lane.records)] {
        if rows.is_empty() {
            continue;
        }
        if drawn > 0 {
            body = body.push(Space::with_height(Length::Fixed(theme::GAP_MD)));
        }
        drawn += 1;
        body = body.push(heading(word, open));
        for entry in rows {
            // **Which row is sounding** — doc 13 §2.6's claim, delivered.
            //
            // A list is never "the sounding record" however many of its tracks
            // are in the run: the fact is about a record, and a list that lit
            // because one of its members was playing would be the
            // invisible-pool posture in a sleeve. **That argument is kept and
            // it is still true — but it is about a list lighting
            // *incidentally*, and it does not reach the case where the list is
            // what the listener put on.** The owner: *"I still see albums
            // specifically appearing as if they are playing rather than the
            // playlist … in a sense we need to track which playlist + track is
            // playing"*.
            //
            // So the mark follows the **run's origin**
            // ([`crate::lane::sounding_subject`], which is the same call the
            // recency ordering makes, so the two cannot disagree): a run
            // reified from a list marks the list and none of its records; every
            // other run marks the record, as before. Nothing lights
            // incidentally either way — a list only ever marked because it *is*
            // the run, which is the most direct fact available rather than a
            // guess from membership.
            //
            // The sections do not change this and cannot: the two are disjoint
            // (`crate::lane::Lane`), so one origin still marks one row.
            let sounds = sounding_row.is_some() && sounding_row == Some(entry.subject);
            body = body.push(lane_row(shelf, playlists, entry, open, sounds));
        }
    }
    body
}

/// A section's word, in the room's quietest voice.
///
/// Absent when the lane is collapsed: at 96 px there is no measure for a
/// tracked word, and a heading over an unlabelled column of sleeves would be
/// naming nothing the eye can use. **That answer is `RECENT`'s and
/// `PLAYLISTS` takes it rather than inventing a second one** — a rail that
/// grew a mark where the expanded lane has a word would be two answers to one
/// question, and 96 px is exactly where there is no room for the second. What
/// separates the two runs of sleeves on the rail is the `GAP_MD` the sections
/// carry either way, and the tooltips, which name every row collapsed.
fn heading(word: &'static str, open: bool) -> Element<'static, Message> {
    let room = theme::active();
    if !open {
        return Space::with_height(Length::Fixed(theme::GAP_SM)).into();
    }
    container(
        text(theme::tracked(word))
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
/// **Nothing is drawn *on* a row to mark its kind** — no badge, no glyph, no
/// corner — because a mark you must learn is not a hierarchy, and because
/// nothing is ever drawn on top of a sleeve (ADR-0024 §A3.3). What marks it
/// is **the line under the name**: an artist's name for a found thing, and
/// `Playlist · 14 · 42:10` for a made one
/// ([`PanelRow::counts`](crate::playlists::PanelRow::counts), ADR-0024
/// §A3.1). Same widget, same size, same ink — a different string, which is
/// what lets a mixed list stay one list rather than becoming two lists
/// sharing a column.
///
/// **This corrects what stood here.** The comment used to say that nothing
/// marks the kind *because the sleeve already does*: a record wears its
/// cover, a playlist wears the 2 × 2 collage of the records it quotes
/// (ADR-0024 §A1, ADR-0030 §2). That premise is **false for every playlist of
/// one to three distinct records** — below four, `views::playlist_sleeve`
/// draws the first record's cover full-bleed through the same `sleeve_cell`
/// this row builds for a record, from the same cache at the same edge. It is
/// therefore false for every playlist `Save as playlist` makes from a CD (one
/// record, by construction) and for every list on its way to four. The
/// collage stays exactly as §A1 designed it and stays *a* signal; it is no
/// longer *the* signal (§A3.2).
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

/// **One control at the lane's foot**: `Collapse` when the lane is open,
/// the chevron alone when it is not.
///
/// It shipped briefly as *two* marks in the density detents' anatomy — one
/// per state, the current one inert — and the owner read the built thing and
/// said it was not the design. He is right, and the two cases are not alike:
/// density has **four** named steps, so marks that name each one are the
/// only way to say which you are on. The lane has two states, and the one you
/// are not in is fully described by the word for getting there. A pair of
/// marks for a binary is a radio group where a switch belongs — and it made
/// the lane's own state something you had to read two glyphs to learn.
///
/// So: a labelled word while there is room for a word, the chevron alone when
/// there is not, and the chevron always points **the way the lane will move**.
///
/// Below [`theme::SIDEBAR_FLOOR`] it is inert and drawn at the disabled ink:
/// expanding there would leave the collection one column of covers, and a
/// control that produces a state the window cannot hold is a trap.
fn marks(open: bool, can_expand: bool) -> Element<'static, Message> {
    container(lane_toggle(open, can_expand))
        .width(Length::Fill)
        .align_x(alignment::Horizontal::Center)
        .padding(theme::pad(theme::GAP_MD, 0.0))
        .into()
}

/// [`marks`]' one control.
fn lane_toggle(open: bool, can_expand: bool) -> Element<'static, Message> {
    let room = theme::active();
    // The chevron points the way the lane will move: closing, it points at
    // the edge it is heading for; opening, away from it.
    let glyph = if open {
        icon::Glyph::LaneCollapsed
    } else {
        icon::Glyph::LaneExpanded
    };
    let usable = open || can_expand;
    let opacity = if usable {
        theme::GLYPH_OPACITY
    } else {
        theme::GLYPH_OPACITY_DISABLED
    };
    let mark = iced_image(icon::handle(glyph))
        .width(Length::Fixed(theme::ICON_PX))
        .height(Length::Fixed(theme::ICON_PX))
        .opacity(opacity);
    // The word rides with the glyph while the lane is open; collapsed, the
    // lane is 96 px and the glyph stands alone under its tooltip, exactly as
    // the destinations above it do.
    let body: Element<'static, Message> = if open {
        row![
            mark,
            text("Collapse")
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.paper_faint)
                .wrapping(text::Wrapping::None),
        ]
        .spacing(theme::GAP_SM)
        .align_y(iced::Alignment::Center)
        .into()
    } else {
        mark.into()
    };
    let boxed = container(body)
        .height(Length::Fixed(theme::STEPPER_HIT))
        .align_y(alignment::Vertical::Center);
    if !usable {
        return boxed.into();
    }
    let pressable = button(boxed)
        .height(Length::Fixed(theme::STEPPER_HIT))
        .padding(theme::pad(0.0, theme::GAP_SM))
        .style(move |_theme, status| theme::transport(room, room.recess, status))
        .on_press(Message::ToggleLane);
    if open {
        return pressable.into();
    }
    iced::widget::tooltip(
        pressable,
        text("Expand")
            .size(theme::SIZE_CAPTION)
            .line_height(theme::LEADING_CAPTION),
        iced::widget::tooltip::Position::Right,
    )
    .gap(theme::GAP_XS)
    .padding(theme::GAP_XS)
    .style(move |_theme| theme::tooltip(room))
    .into()
}

#[cfg(test)]
mod tests {
    use super::SCOPE;
    use crate::theme;

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
            .split_once("let mut head")
            .expect("the head is built")
            .1;
        let (head, _) = head.split_once("let body_rows").expect("the head ends");
        assert!(
            head.contains("theme::sidebar_can_expand(window_w)"),
            "the head's well is not conditioned on the lane being able to open"
        );
        assert!(
            head.contains("head.push(well(shelf, open))"),
            "the well is not pushed onto the head"
        );
        // **The well leads** — the owner's *"search belongs at the top"*. The
        // order is the claim, so the order is what is pinned: a later edit
        // that puts the destinations first passes every other assertion here.
        let well_at = head.find("head.push(well(shelf, open))").expect("the well");
        let dests_at = head
            .find("for to in Destination::ALL")
            .expect("the three destinations");
        assert!(
            well_at < dests_at,
            "the well must stand above the three destinations, not below them"
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

    /// **The well is one control tall, and the only figure in it is the match
    /// count** — the owner's *"the album and track count below the search bar
    /// doesn't look good… maybe this should go into the home as some basic
    /// stats?"*, delivered.
    ///
    /// The two states of the retired readout had two different jobs. The
    /// resting counts are a statistic about the collection and are Home's
    /// `COLLECTION` footer now — the pin for that lives in
    /// [`crate::views::home`]'s tests, and this one is its other half: the
    /// second line is **gone from the lane**, not merely quieter. The match
    /// count is feedback about the query, so it is inside the field it answers,
    /// right-aligned in [`theme::SIDEBAR_MATCH_W`] rather than the strip's
    /// wider `MATCH_W`, which is the whole reason it fits.
    #[test]
    fn the_wells_one_figure_is_the_match_count_inside_the_field() {
        let source = source();
        let well = body(&source, "fn well(shelf: &Shelf, open: bool)");
        let shipped = source
            .split("#[cfg(test)]")
            .next()
            .expect("a source has a head");
        assert!(
            !shipped.contains("fn readout"),
            "the well's second line is still being built; the collection's \
             counts belong to the Home place now"
        );
        assert!(
            well.contains("text_input(SCOPE, &shelf.query)"),
            "the lane's well no longer names itself in its placeholder"
        );
        assert!(
            well.contains("match_count(shelf)"),
            "the match count is not drawn in the well"
        );
        // The lane's own slot, never the strip's 88 — 88 inside a 232 px well
        // is what drove both figures out of the field in the first place.
        assert!(
            well.contains("theme::GAP_MD + theme::SIDEBAR_MATCH_W")
                && well.contains("Length::Fixed(theme::SIDEBAR_MATCH_W)"),
            "the count's slot is not both reserved by the field's padding and \
             drawn at one fixed width, so typed text can collide with it"
        );
        assert!(
            !well.contains("top_bar::MATCH_W"),
            "the lane's well reserved the strip's wider match slot"
        );
        // Nothing moves as the first character lands: the reservation is on
        // the right, and the block is a fixed height whatever is in it.
        assert!(
            well.contains("Length::Fixed(theme::SIDEBAR_WELL_H)"),
            "the well's block is not held at a fixed height, so a keystroke \
             would push the lane's rows down"
        );
        let count = body(&source, "fn match_count(shelf: &Shelf)");
        assert!(
            count.contains("shelf.visible.len()") && count.contains("shelf.albums.len()"),
            "the match count is no longer the query's answer over the \
             collection's size"
        );
    }

    /// **The well says what it searches, and the answer is never the place you
    /// are standing in** — ADR-0036 §2.
    ///
    /// The owner asked what search should do off the Library. The decision is
    /// that it keeps one meaning — the collection — and that the field states
    /// it, because a resident field reading `Search` over a page called
    /// `Road Trip` is fairly read as offering to search `Road Trip`. Two things
    /// are pinned here: the placeholder is the scope word, and it is a
    /// **constant** rather than anything resolved from [`crate::place::Place`],
    /// which is what makes a scoped well a visible edit rather than a quiet
    /// drift.
    #[test]
    fn the_wells_placeholder_names_the_one_thing_it_searches() {
        let source = source();
        let well = body(&source, "fn well(shelf: &Shelf, open: bool)");
        assert_eq!(
            SCOPE, "Search library",
            "the well's placeholder no longer names the collection"
        );
        assert!(
            well.contains("text_input(SCOPE, &shelf.query)"),
            "the well's placeholder is not the scope word"
        );
        assert!(
            !well.contains("Place"),
            "the well resolves its placeholder against the place — that is a \
             scoped search, and ADR-0036 §3 refused it because type-anywhere \
             is a promise about the collection"
        );
    }

    /// **One box, two meanings**: the magnifier at rest, the `×` under a query
    /// (ADR-0036 §4, the owner's *"maybe a little x or esc to clear"*).
    ///
    /// The claim that matters is geometric. The clear mark could not go beside
    /// the count — the field's right edge already spends `GAP_MD` 12 +
    /// [`theme::SIDEBAR_MATCH_W`] 72 and a glyph box more would take the
    /// query's own room to 80 px, below the 88 the design measured and rejected
    /// (`crate::font`'s `the_lanes_well_holds_a_query_beside_its_match_count`).
    /// The mark's box on the left is already paid for and is exactly
    /// [`theme::STEPPER_HIT`] wide on [`theme::SIDEBAR_HEAD_GLYPH_X`], so the
    /// swap moves nothing — which is the same guarantee the count's fixed slot
    /// gives, on the other side of the field.
    #[test]
    fn the_wells_mark_is_a_label_at_rest_and_the_clear_under_a_query() {
        let source = source();
        let well = body(&source, "fn well(shelf: &Shelf, open: bool)");
        assert!(
            well.contains("let mark: Element<'_, Message> = if filtering {")
                && well.contains("clear_mark(room.recess)"),
            "the well's mark does not become the clear control under a query"
        );
        assert!(
            well.contains("icon::Glyph::Magnifier"),
            "the well lost the label it wears at rest"
        );
        // The swap costs the field nothing on either edge: the left lead is
        // the mark's own box and the right is the count's reserved slot,
        // unchanged.
        assert!(
            !well.contains("theme::SIDEBAR_MATCH_W + ")
                && !well.contains("+ theme::STEPPER_HIT")
                && !well.contains("theme::SIDEBAR_MATCH_W\n                    +"),
            "the clear mark took room from the query's own 104 px"
        );
        // Centred on the destinations' glyph vertical without a constant of
        // its own: SIDEBAR_HEAD_GLYPH_X = GAP_SM + SIDEBAR_GLYPH_BOX / 2, and
        // the box is STEPPER_HIT = SIDEBAR_GLYPH_BOX, so the inset is GAP_SM.
        assert!(
            well.contains(".padding(theme::pad(0.0, theme::GAP_SM))"),
            "the clear mark is not inset to the head's glyph vertical"
        );
        assert!(
            (theme::STEPPER_HIT - theme::SIDEBAR_GLYPH_BOX).abs() < f32::EPSILON,
            "a control's box and the head's glyph box have diverged, so the \
             clear mark needs an inset of its own"
        );
        assert!(
            (theme::GAP_SM + theme::STEPPER_HIT / 2.0 - theme::SIDEBAR_HEAD_GLYPH_X).abs()
                < f32::EPSILON,
            "the clear mark's centre is off the destinations' glyph vertical"
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

    /// **The lists have a section, it stands above `RECENT`, and both are
    /// inside one scroller.**
    ///
    /// The owner, 2026-08-10: *"I guess we need to add playlists into their own
    /// section under library"*. Three things are load-bearing and all three are
    /// facts about *where the code is*, which is what a source scan is good
    /// for; the membership and the order are behavioural and are pinned over
    /// real values in [`crate::lane`]'s own tests.
    ///
    /// 1. **`PLAYLISTS` before `RECENT`**, under the head rather than inside
    ///    it — *"under library"* read as under the closed triple, because a
    ///    section between two destinations would split the one thing ADR-0030's
    ///    first amendment says is always all three in that order.
    /// 2. **One scroller over both.** `PLAYLISTS` has no cap, so a second
    ///    scroller or a fixed-height first section is how `RECENT` becomes
    ///    unreachable at forty lists. This is the defect the split is most
    ///    likely to introduce, so it is the assertion with the most weight.
    /// 3. **A section with nothing in it is absent**, not an empty heading.
    #[test]
    fn the_lists_have_their_own_section_above_recent_in_one_scroller() {
        let source = source();
        let shipped = source
            .split("#[cfg(test)]")
            .next()
            .expect("a source has a head");
        let sections = body(&source, "fn sections<'a>(");
        let lists_at = sections.find("\"PLAYLISTS\"").expect("the lists' section");
        let recent_at = sections.find("\"RECENT\"").expect("the records' section");
        assert!(
            lists_at < recent_at,
            "`RECENT` is drawn above the lists; the owner asked for the \
             playlists' section under the head, ahead of the records"
        );
        assert!(
            sections.contains("if rows.is_empty() {\n            continue;"),
            "a section with no rows still draws its heading — a word naming \
             nothing, which ADR-0030 §6's absent-not-empty rule refuses"
        );
        // **One scroller**, and the sections are inside it. Two would be two
        // scroll positions to arbitrate between — the failure ADR-0030 §1's
        // one-order rule exists to prevent — and a fixed first section would
        // push `RECENT` off the bottom once the lists outgrew it.
        assert_eq!(
            shipped.matches("scrollable(").count(),
            1,
            "the lane grew a second scroller, so it has two scroll positions \
             and `RECENT` is reachable only at some list counts"
        );
        let view = body(&source, "pub(crate) fn view<'a>(");
        assert!(
            view.contains("scrollable(body_rows.padding("),
            "the sections are no longer the scroller's content"
        );
        assert!(
            !view.contains("flanked(heading"),
            "a heading is drawn outside the scroller again, so it stands \
             while its rows scroll under it"
        );
        // Collapsed, a section's word is the same nothing `RECENT` has always
        // been at 96 px — one answer to that question, not two.
        let heading = body(&source, "fn heading(word: &'static str, open: bool)");
        assert!(
            heading.contains("if !open {\n        return Space::with_height"),
            "the collapsed rail grew a section mark of its own"
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
    /// ADR-0030 §4 costs at zero idle CPU.
    ///
    /// **Which row, though, is not asserted here**, and the split is the
    /// point. *A playlist's run marks the list and not the records it quotes*
    /// is a **behavioural** claim, and it is pinned as one, over real values,
    /// in `lane.rs`'s `the_sounding_row_is_the_list_when_a_list_is_what_was_put_on`
    /// and `only_one_row_is_ever_marked`. A source scan cannot tell a correct
    /// rewrite from a broken one, so it must not be what guards a rule.
    ///
    /// What a source scan *can* say, and what is load-bearing enough to keep:
    /// **the view does not re-derive the answer.** The rule lives in
    /// `lane::sounding_subject` — the same call the recency ordering makes —
    /// and a view that went back to matching on the sounding file's record
    /// would be correct-looking, would pass every behavioural test in
    /// `lane.rs`, and would put the dot and the order back to reading two
    /// separate answers to one question. That is a fact about *where the code
    /// is*, which is the only kind of fact this form is good for. The
    /// remaining assertions are the row's own drawing, which builds iced
    /// widgets and so has nothing else to be asserted against.
    #[test]
    fn the_sounding_record_is_the_marked_row() {
        let source = source();
        // The marking moved into `sections` when the lists got a section of
        // their own, so that is what this scans — a test still reading `view`
        // would pass by looking at a function the rule left.
        let sections = body(&source, "fn sections<'a>(");
        assert!(
            sections.contains("sounding_row == Some(entry.subject)"),
            "the lane no longer marks the row `lane::sounding_subject` named"
        );
        assert!(
            !sections.contains("Subject::Record(id) if Some(id) =="),
            "the lane derived the sounding row from the sounding file's record \
             again, instead of taking `lane::sounding_subject`'s answer — the \
             dot and the recency order are now free to disagree"
        );
        assert!(
            !sections.contains("Subject::Playlist(") && !sections.contains("played_list"),
            "the lane re-derived which row is sounding rather than being handed \
             it; the rule belongs in `lane::sounding_subject`"
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
