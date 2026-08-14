//! **One page, two subjects** — the composition a record's page and a made
//! list's page both wear, written down once.
//!
//! The owner, 2026-08-10: *"can we reuse the basic layout and view of the
//! playlist for the album view and the playlist view accessed via clicking
//! into info — right now they are different but for no good reason."*
//!
//! # What was already decided, and what was merely drift
//!
//! ADR-0024 §A2 gave a playlist's page the record page's two-column
//! arrangement, and design 14 §6 confirmed it: **share the arrangement, not
//! the hierarchy**. What §A2 could not do was make the two pages *one
//! implementation*, so [`views::album`](super::album) and
//! [`views::playlist`](super::playlist) were written weeks apart and drifted —
//! two copies of the same breakpoint arithmetic, two copies of the scroll, two
//! identity blocks held level only by a test that read both files' tokens, two
//! spellings of the quiet act, two `Play` buttons, two lamp dots and four
//! copies of one reserved icon slot.
//!
//! This module is §A2's arrangement made literal. It takes what genuinely
//! differs and draws everything else exactly once:
//!
//! | | a record's page | a playlist's page |
//! |---|---|---|
//! | the strip's lead | `Anne-Marie Puig › Ochre`, the artist half a door | `Road Trip` |
//! | the sleeve | the cover, one authored image | the collage of quotations (§A1) |
//! | the commitment | `Play album` | `Play` |
//! | the acts | `Add to playlist…` | `Rename` · `Delete`, then `Cancel` · `Move to Trash` while confirming |
//! | the aside's tail | the edition selector, then `DETAILS` | the rename field, while renaming |
//! | the hero's face | `theme::WORK_TITLE`, serif italic — a work's own title | [`theme::SEMIBOLD`], sans — a label the owner typed |
//! | the byline | the artist | `Playlist · 12 records` |
//! | the facts | `1999 · 12 tracks · 59:18 · FLAC · 16-bit · 44.1 kHz` | `14 tracks · 2:02:56`, with `Undo` beside it while there is an edit to take back |
//! | a row's trailing slots | the transfer `+` | ▲▼, ✕, and the transfer `+` |
//! | the empty state | its own sentence | its own sentence |
//!
//! Everything to the left of those two columns — the gutter, the breakpoint,
//! the aside's width and order, the identity block's three lines and their
//! pitch, the `TRACKS` rule, the row spacing, the one scroll — is here, once.
//!
//! # The strip leads with the subject, on both
//!
//! [`super::place_header_led`]'s own rule: *"Four of the places lead with
//! [`super::place_name`] and nothing else. Two do not"* — the Album place,
//! which leads with `Artist › Album`, and the Artist place, which leads with a
//! runtime string. **A place whose subject changes leads with its subject**,
//! and a playlist's page is the third member of that set. It led with the word
//! `Playlist` because it predates the breadcrumb by weeks, which is exactly the
//! *"different for no good reason"* the owner named.
//!
//! The kind word did not go anywhere. Design 14 §3.5 had already called the
//! chrome the wrong place for it — *"58 px above the name… invisible at the
//! moment the eye is actually deciding"* — and tier 1 moved it to the byline,
//! where it is stated at 19 px directly under the name
//! (`Playlist · 12 records`) instead of at 15 px in the chrome strip. So the rule is now one rule on both pages: **the strip names what
//! you are looking at; the byline says what kind of thing it is.**
//!
//! # …and one level down, the rows — 2026-08-10, the same instruction again
//!
//! The owner, the same day: *"I think ideally we could ensure our playlist view
//! in the now playing and the playlist view/album view are the same thing. the
//! only thing that changes in now playing is that we don't see file details
//! etc. — that is more like a album exploration type data"*.
//!
//! This module's first version said the rows were *"deliberately not shared…
//! each page builds its own and hands the finished list over"*, on the grounds
//! that they are built from different values and carry different edit sets.
//! **Both halves of that were true and the conclusion did not follow.** The
//! values differ, so they are arguments ([`TrackRow`]); the edit sets differ,
//! so the slots stay with the caller. What was left over once those two were
//! taken out was the *anatomy* — the [`theme::TRACK_NO_W`] number lane, the
//! title over its second line, the [`theme::DURATION_W`] duration lane, the
//! button's paint and its padding — and that was written **three** times, in
//! `views::album`, `views::playlist` and `views::queue`.
//!
//! So [`track_row`] draws it once. The third surface was the run column on
//! `Now playing`; it is now the unsaved persistence state of
//! [`super::playlist_page`], through this same document composition.
//!
//! ## What the owner named as the difference, and what else stayed
//!
//! `DETAILS` — format, bit depth, sample rate, size, folder — is *"album
//! exploration type data"* and lives on a record's page in the aside, which the
//! run column does not have. That is his own line and it holds.
//!
//! Three differences survive because they are facts about the subject rather
//! than drift, and each is named where it is drawn:
//!
//! - **the next-track ring** (`views::queue`'s `next_ring`) — a run has a
//!   cursor, so it has a *next*; a document has neither;
//! - **the trailing slots** — ▲▼✕ belong to an editable list (doc 09 §8.2) and
//!   a published record's tracks are not one;
//! - **the identity facts** — a saved page states durable counts; the run uses
//!   that same line for its live cursor and remaining-time sentence.
//!
//! The old limit of this merge was the run's private top-level composition.
//! The owner's 2026-08-12 review showed that partial reuse still drifted, so
//! [`super::playlist_page`] now parameterizes both persistence states and is
//! the only playlist-page caller of [`view`].

use iced::widget::scrollable::Viewport;
use iced::widget::{
    Column, Row, Space, button, column, container, image as iced_image, mouse_area, row,
    scrollable, text, text_input, tooltip,
};
use iced::{Element, Font, Length, alignment, mouse};

use crate::app::Message;
use crate::views::{place_header_led, place_pad, section_rule};
use crate::{icon, theme};

/// The shared album/playlist document scroller. Source navigation uses this
/// identity to bring the sounding row into view after opening its page.
pub(crate) fn scroll_id() -> iced::widget::Id {
    iced::widget::Id::new("baz-subject-page")
}

/// Whether the shared subject page uses its desktop table composition.
#[must_use]
pub(crate) fn is_two_column(window_width: f32) -> bool {
    window_width >= theme::ALBUM_BREAKPOINT
}

/// Whether a saved playlist has enough room for the desktop table form.
/// Its rows reserve artwork, an Album value and four edit targets in addition
/// to the ordinary track anatomy, so it stacks sooner than an album page.
#[must_use]
pub(crate) fn is_playlist_two_column(window_width: f32) -> bool {
    window_width >= theme::PLAYLIST_BREAKPOINT
}

/// **The page's identity block**: the name, the byline under it, and the line
/// of facts under that — three lines, three sizes, three inks, one falling
/// order, [`theme::GAP_XS`] between them.
///
/// It measures 32 + 4 + 24 + 4 + 16 = **80 px** on both pages, which is the
/// whole of the answer to the owner's *"we do not have the playlist name
/// really prominent"* (design 14 §3.4, ADR-0024 §A4.3): the name was always the
/// album title's own hero size, and what it was missing was the 19 px line of
/// support a record's title is given. That equality used to be
/// held by a test that read both view sources for their tokens; it is now held
/// by there being one composition.
pub(crate) struct Identity<'a> {
    /// The hero: an album's title, or a list's name.
    pub(crate) name: String,
    /// The face the hero is set in — **the axis that tells the two kinds
    /// apart** (design 14 §5.2, ADR-0024 §A4.4). A record's page passes the
    /// serif italic, because an album's title is a work someone published; a
    /// playlist's passes the sans, because its name is a label the owner
    /// typed, like the search query and the rename field two blocks away.
    ///
    /// The token itself is named at the two call sites rather than here, and
    /// that is load-bearing:
    /// `theme::the_serif_is_the_work_titles_and_nothing_else` enumerates the
    /// files that may set it, and a shared composition that named it would put
    /// the serif one argument away from every page in the product.
    pub(crate) face: Font,
    /// The title's inline editing state. When present, the hero itself becomes
    /// the field and carries its compact Save action; `None` leaves the title
    /// as ordinary text. Saved and unsaved playlists both use this slot.
    pub(crate) edit: Option<NameEdit<'a>>,
    /// The middle line: a record's artist, or `Playlist · 12 records`.
    pub(crate) byline: String,
    /// The facts line: the catalogue line, or the counts.
    pub(crate) facts: String,
    /// One transient control beside the facts — the playlist page's `Undo`,
    /// drawn exactly while there is an edit to take back (doc 11 §5 P2).
    pub(crate) beside_facts: Option<Element<'a, Message>>,
}

/// Everything the shared identity block needs to turn its hero into a focused
/// name field without learning which kind of playlist owns the edit.
pub(crate) struct NameEdit<'a> {
    pub(crate) value: &'a str,
    pub(crate) error: Option<&'a str>,
    pub(crate) id: iced::widget::Id,
    pub(crate) on_input: fn(String) -> Message,
    pub(crate) on_submit: Message,
}

/// **A page about one thing**: the header strip, then the object beside what
/// is written about it, in one scroll.
pub(crate) struct Page<'a> {
    /// The strip's lead — the subject, in whatever shape the subject has.
    pub(crate) lead: Element<'static, Message>,
    /// The object itself, at [`theme::ALBUM_SLEEVE`].
    pub(crate) sleeve: Element<'a, Message>,
    /// The page's one commitment, under the sleeve at the sleeve's whole
    /// width. `None` where there is no engine in the build at all to send it
    /// to — a control that can never act is not drawn.
    pub(crate) commitment: Option<Element<'a, Message>>,
    /// The quieter acts, in one row under the commitment.
    pub(crate) acts: Vec<Element<'a, Message>>,
    /// Whatever else the aside carries, in order, below the acts.
    pub(crate) aside_tail: Vec<Element<'a, Message>>,
    /// The identity block that heads the main column.
    pub(crate) identity: Identity<'a>,
    /// The rows, built by the page that owns them.
    pub(crate) rows: Vec<Element<'a, Message>>,
    /// Whether this page has enough width for the desktop table composition.
    /// The compositor remains shared, while each row anatomy gets to state
    /// the width at which its flexible title lane would otherwise disappear.
    pub(crate) side_by_side: bool,
    /// Gap between row elements. Album rows use the ordinary list gap;
    /// virtual playlist rows fold that gap into their fixed pitch so their
    /// top and bottom spacers remain exact.
    pub(crate) row_spacing: f32,
    /// A viewport reading for pages whose row list is virtualized.
    pub(crate) on_scroll: Option<fn(Viewport) -> Message>,
    /// What the `TRACKS` block says when there are none.
    pub(crate) empty: &'static str,
}

/// Draw a [`Page`] at `window_width`.
///
/// `window_width` sizes the arrangement and [`Page::side_by_side`] selects its
/// responsive form. The page grows
/// with the window until its list reaches [`theme::LIST_MEASURE`] and then
/// stops, centring in what is left — a measure has a comfortable range rather
/// than a single right answer, and a track list set 1500 px wide is a row of
/// two words at opposite ends of the screen. The columns stack before their
/// row anatomy can consume the title's flexible width.
///
/// In the two-column form, the identity and aside stay at the top while the
/// track table alone scrolls. The `TRACKS` rule is therefore the table's
/// sticky head. The stacked form remains one document and one scroll.
///
/// This arithmetic was written twice and is now written once, which is the
/// half of *"they are different for no good reason"* that no frame would ever
/// have shown: the two copies agreed, and nothing but a reviewer's memory kept
/// them agreeing.
#[expect(
    clippy::too_many_lines,
    reason = "the responsive forms share one destructuring and one set of page parts"
)]
pub(crate) fn view<'a>(page: Page<'a>, window_width: f32) -> Element<'a, Message> {
    let room = theme::active();
    // What the page's own block has to fit in: the window, less the one gutter
    // on both sides and the scrollbar's declared lane on the right
    // ([`super::place_pad`]).
    let content = (window_width - 2.0 * theme::HANG - theme::SCROLLBAR_LANE).max(0.0);
    let measure = if page.side_by_side {
        (content - theme::ALBUM_ASIDE_W - theme::GAP_XL).clamp(0.0, theme::LIST_MEASURE)
    } else {
        content.min(theme::LIST_MEASURE)
    };

    let Page {
        lead,
        sleeve,
        commitment,
        acts,
        aside_tail,
        identity,
        rows,
        side_by_side,
        row_spacing,
        on_scroll,
        empty,
    } = page;

    // **The aside**, fixed at [`theme::ALBUM_ASIDE_W`] — the sleeve's own edge
    // — so its blocks share one lane and the page has two x-edges on this side
    // rather than three (law L5).
    let mut aside = column![sleeve].spacing(theme::GAP_MD);
    if let Some(commitment) = commitment {
        aside = aside.push(commitment);
    }
    if !acts.is_empty() {
        aside = aside.push(
            Row::with_children(acts)
                .spacing(theme::GAP_SM)
                .align_y(iced::Alignment::Center),
        );
    }
    for block in aside_tail {
        aside = aside.push(block);
    }

    let body: Element<'a, Message> = if rows.is_empty() {
        text(empty)
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            .color(room.paper_faint)
            .into()
    } else {
        Column::with_children(rows).spacing(row_spacing).into()
    };
    // **The lead's box is [`place_header_led`]'s now**, not this page's. It
    // stood locally for one build and made pages differ in height; the shared
    // strip is the one answer now.
    let header = place_header_led(lead, None);
    if side_by_side {
        // A desktop page behaves like a table: its subject remains available
        // at the top while the rows turn beneath a sticky section head. This
        // also gives source navigation a stable scroller whose content begins
        // with row zero instead of a hero of variable height.
        let table_scroll = scrollable(container(body).padding(iced::Padding {
            bottom: theme::HANG,
            ..iced::Padding::default()
        }))
        .id(scroll_id())
        .direction(scrollable::Direction::Vertical(theme::list_scrollbar()))
        .style(move |_theme, status| theme::scrollbar(room, room.wall, status))
        .width(Length::Fill)
        .height(Length::Fill);
        let table_scroll = if let Some(on_scroll) = on_scroll {
            table_scroll.on_scroll(on_scroll)
        } else {
            table_scroll
        };
        let table = column![section_rule("Tracks"), table_scroll]
            .spacing(theme::GAP_SM)
            .height(Length::Fill);
        let main = column![identity_block(identity), table]
            .spacing(theme::GAP_XL)
            .height(Length::Fill);
        let composed = row![
            container(aside).width(Length::Fixed(theme::ALBUM_ASIDE_W)),
            container(main)
                .width(Length::Fixed(measure))
                .height(Length::Fill),
        ]
        .spacing(theme::GAP_XL)
        .align_y(iced::Alignment::Start)
        .height(Length::Fill);
        return column![
            header,
            container(composed)
                .width(Length::Fill)
                .height(Length::Fill)
                .padding(place_pad())
                .align_x(alignment::Horizontal::Center),
        ]
        .into();
    }

    let main = column![
        identity_block(identity),
        column![section_rule("Tracks"), body].spacing(theme::GAP_SM),
    ]
    .spacing(theme::GAP_XL);
    let composed = column![
        // In the stacked form the artwork and its acts are the page's hero,
        // centred over the text measure. The identity and track table below
        // keep their reading edge; centring row contents would turn a list
        // into a poster rather than a document.
        container(container(aside).width(Length::Fixed(theme::ALBUM_ASIDE_W)))
            .width(Length::Fixed(measure))
            .align_x(alignment::Horizontal::Center),
        container(main).width(Length::Fixed(measure)),
    ]
    .spacing(theme::GAP_XL);

    column![
        header,
        // **One scroll for the whole page.** A page is one document and
        // turning it over is one gesture in the stacked form.
        {
            let document = scrollable(
                container(composed)
                    .width(Length::Fill)
                    .padding(place_pad())
                    .align_x(alignment::Horizontal::Center),
            )
            .id(scroll_id())
            .direction(scrollable::Direction::Vertical(theme::list_scrollbar()))
            .style(move |_theme, status| theme::scrollbar(room, room.wall, status))
            .width(Length::Fill)
            .height(Length::Fill);
            if let Some(on_scroll) = on_scroll {
                document.on_scroll(on_scroll)
            } else {
                document
            }
        },
    ]
    .into()
}

/// The identity block itself — see [`Identity`] for what the 80 px is and why
/// it is the answer to *"the name isn't really prominent"*.
///
/// The hero clips at **two lines**. `Wrapping::None` does not stop iced 0.13
/// laying a long string over several lines, and a box-set title running to four
/// lines pushes everything under it down the page. Two lines is a title; more
/// is a paragraph.
pub(crate) fn identity_block(identity: Identity<'_>) -> Element<'_, Message> {
    let room = theme::active();
    let mut facts = row![
        text(identity.facts)
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            .color(room.paper_faint)
            .wrapping(text::Wrapping::None),
    ]
    .spacing(theme::GAP_SM)
    .align_y(iced::Alignment::Center);
    if let Some(beside) = identity.beside_facts {
        facts = facts.push(beside);
    }
    let title: Element<'_, Message> = match identity.edit {
        None => container(
            text(identity.name)
                .size(theme::SIZE_HERO)
                .line_height(theme::LEADING_HERO)
                .font(identity.face)
                .color(room.paper),
        )
        .max_height(2.0 * theme::LINE_HERO)
        .clip(true)
        .into(),
        Some(edit) => {
            let valid = !edit.value.trim().is_empty();
            let field = text_input("Playlist name", edit.value)
                .id(edit.id)
                .on_input(edit.on_input)
                .on_submit(edit.on_submit.clone())
                .padding(theme::pad(theme::WELL_PAD_V, theme::GAP_MD))
                .size(theme::SIZE_HERO)
                .line_height(theme::LEADING_HERO)
                .width(Length::Fill)
                .style(move |_theme, status| theme::input(room, status));
            let mut block = column![
                row![field, act("Save", valid, edit.on_submit)]
                    .spacing(theme::GAP_SM)
                    .align_y(iced::Alignment::Center)
            ]
            .spacing(theme::GAP_XS);
            if let Some(error) = edit.error {
                block = block.push(
                    text(error.to_owned())
                        .size(theme::SIZE_META)
                        .line_height(theme::LEADING_META)
                        .color(room.alert),
                );
            }
            block.into()
        }
    };
    column![
        title,
        text(identity.byline)
            .size(theme::SIZE_TITLE)
            .line_height(theme::LEADING_TITLE)
            .color(room.paper_dim),
        facts,
    ]
    .spacing(theme::GAP_XS)
    .into()
}

/// **The page's one commitment** — `Play album` on a record, `Play` on a list
/// — a lamp outline with a paper triangle and a paper label, and the only
/// control in baz drawn in the accent.
///
/// It is the switch that turns the picture light on — the one control in the
/// product that *creates* playback truth — which is why it is allowed the
/// colour and why there is at most one of it on screen. It takes the sleeve's
/// whole width and stands directly under it, which since ADR-0022 makes the
/// press that replaced the wall's double-click a 320 × 32 target in a fixed
/// place rather than a 400 ms timing gesture.
pub(crate) fn commitment(
    label: &'static str,
    live: bool,
    message: Message,
) -> Element<'static, Message> {
    let room = theme::active();
    button(
        // **The box centres the ink, in both axes** (law L3).
        container(
            row![
                iced_image(icon::handle(icon::Glyph::Play))
                    .width(Length::Fixed(theme::ICON_PX))
                    .height(Length::Fixed(theme::ICON_PX))
                    .opacity(theme::glyph_opacity(live, false)),
                text(label)
                    .size(theme::SIZE_BODY)
                    .line_height(theme::LEADING_BODY)
                    .font(theme::SEMIBOLD)
                    .wrapping(text::Wrapping::None),
            ]
            .spacing(theme::GAP_SM)
            .align_y(iced::Alignment::Center),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(alignment::Horizontal::Center)
        .align_y(alignment::Vertical::Center),
    )
    .width(Length::Fill)
    .height(Length::Fixed(theme::TRANSPORT_HIT))
    .padding(theme::pad(0.0, theme::GAP_MD))
    .style(move |_theme, status| theme::primary(room, status))
    .on_press_maybe(live.then_some(message))
    .into()
}

/// **A quiet act** — `Add to playlist…` on a record, `Queue` · `Rename` ·
/// `Delete` on a list — at the product's one control height, no accent: the
/// lamp stays spent on playback truth alone.
///
/// The two pages spelled this differently and the difference was invisible
/// until they were laid side by side. A record's single act was a *centred,
/// full-width* box in [`theme::word_button`]'s paint, resting at
/// [`theme::Palette::paper_dim`]; a list's three were natural-width words in
/// [`theme::transport`]'s, resting at [`theme::Palette::paper`]. One slot, two
/// inks, two alignments, for no reason either file could name. They are one
/// word now, hanging from the aside's own lane like everything else in it
/// (law L5) — which is what a full-width centred box could not do.
pub(crate) fn act(
    label: &'static str,
    enabled: bool,
    message: Message,
) -> Element<'static, Message> {
    let room = theme::active();
    button(
        container(
            text(label)
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .font(theme::MEDIUM)
                .color(if enabled {
                    room.paper
                } else {
                    room.paper_muted
                })
                .wrapping(text::Wrapping::None),
        )
        .height(Length::Fill)
        .align_y(alignment::Vertical::Center),
    )
    .height(Length::Fixed(theme::TRANSPORT_HIT))
    .padding(theme::pad(0.0, theme::GAP_MD))
    .style(move |_theme, status| theme::transport(room, room.wall, status))
    .on_press_maybe(enabled.then_some(message))
    .into()
}

/// **One row of a list of tracks**, in the anatomy all three surfaces wear.
///
/// The owner, 2026-08-10: *"I think ideally we could ensure our playlist view
/// in the now playing and the playlist view/album view are the same thing."*
/// The same instruction that made a record's page and a list's page one
/// composition, one level down: a record's track, a list's entry and a run's
/// row were **three literal copies** of the marker lane, the title stack, the
/// duration lane and the button's paint.
///
/// What genuinely differs is in this struct and nowhere else — what the marker
/// is, what the two lines say and in what ink, and what the press does. The
/// *geometry* (the [`theme::TRACK_NO_W`] number lane right-aligned and centred
/// on the title's own line, the title filling, the [`theme::DURATION_W`]
/// duration lane, the [`theme::GAP_SM`] between them, the top alignment, the
/// [`theme::GAP_XS`] vertical padding and the absent horizontal one) is here,
/// once.
///
/// **The trailing slots are not part of it.** A run's row and a list's entry
/// carry ▲▼✕ and a record's track does not, which is doc 09 §8.2's own
/// distinction — a durable artefact and a transient run are editors, a
/// published record is not. Each caller hangs its own slots off the returned
/// body with [`icon_slot`], which is the shared thing they *are* made of.
pub(crate) struct TrackRow<'a> {
    /// The number lane's occupant: a position, the lamp dot, or the next
    /// ring — whatever this surface has to say about where the music is.
    pub(crate) marker: Element<'a, Message>,
    /// A row-sized sleeve between its number and title, where this surface
    /// identifies records per track rather than with group headings.
    pub(crate) artwork: Option<Element<'a, Message>>,
    /// The row's own line.
    pub(crate) title: std::borrow::Cow<'a, str>,
    /// The title's ink. **Stated rather than inherited**: a record's row used
    /// to set no colour at all and take [`theme::track_row`]'s
    /// `text_color`, which is [`theme::Palette::paper`] — the same ink the
    /// other two name explicitly. Identical on screen, and one fewer fact a
    /// reader has to go to the style function to learn.
    pub(crate) ink: iced::Color,
    /// The second line, where there is one: a track artist, or the path a
    /// missing entry went to. Its optional press lets playlist metadata be a
    /// navigation target independently of the row's playback press.
    pub(crate) under: Option<(std::borrow::Cow<'a, str>, iced::Color, Option<Message>)>,
    /// The playlist's Album value, its independent navigation press, and
    /// whether it occupies the desktop table column. In the compact form it
    /// folds beside the artist on the metadata line instead of disappearing.
    /// Other track surfaces leave it absent.
    pub(crate) context: Option<(std::borrow::Cow<'a, str>, Option<Message>, bool)>,
    /// The duration, already formatted.
    pub(crate) duration: std::borrow::Cow<'a, str>,
    /// Whether this is the sounding row — the medium weight and the card.
    pub(crate) playing: bool,
    /// Whether one ordinary press selected this row.
    pub(crate) selected: bool,
    /// What pressing it does, or `None` where it cannot act: no engine, or a
    /// missing file with nothing to play.
    pub(crate) press: Option<Message>,
}

/// Draw a [`TrackRow`] — the shared body, without its trailing slots.
///
/// The number column and the duration lane are centred on the **title's own
/// line**, not on the row's block, and the row is top-aligned so they stay
/// there. Centred on the block, a soundtrack row that carries a composer under
/// its title dragged its number and its duration halfway down two lines.
///
/// **No horizontal inset**: the number column starts on the column's own
/// content lane and the duration lane ends on it, so the block a listener reads
/// down shares its edges with whatever holds it (law L5). That is why this is
/// one function across a 880 px page column and a 692 px run column — the
/// anatomy is expressed in reserved lanes and a `Fill`, so it is the same row
/// at any measure.
pub(crate) fn track_row(row: TrackRow<'_>) -> Element<'_, Message> {
    let room = theme::active();
    let TrackRow {
        marker,
        artwork,
        title,
        ink,
        under,
        context,
        duration,
        playing,
        selected,
        press,
    } = row;
    let heading = text(title)
        .size(theme::SIZE_BODY)
        .line_height(theme::LEADING_BODY)
        .color(ink)
        .wrapping(text::Wrapping::None);
    // The playing row's title takes the medium weight the now-playing bar gives
    // the same string — one more place the surfaces agree about what is
    // sounding.
    let heading = if playing {
        heading.font(theme::MEDIUM)
    } else {
        heading
    };
    let under = under.map(|(label, ink, press)| metadata_label(label, ink, press));
    let (table_context, inline_context) = match context {
        Some((label, press, true)) => (Some(metadata_label(label, room.paper_dim, press)), None),
        Some((label, press, false)) => (None, Some(metadata_label(label, room.paper_dim, press))),
        None => (None, None),
    };
    let mut stack = column![heading].spacing(theme::GAP_XXS);
    match (under, inline_context) {
        (Some(under), Some(context)) => {
            stack = stack.push(
                row![
                    under,
                    text("·")
                        .size(theme::SIZE_META)
                        .line_height(theme::LEADING_META)
                        .color(room.paper_faint),
                    context,
                ]
                .spacing(theme::GAP_XS)
                .align_y(iced::Alignment::Center),
            );
        }
        (Some(under), None) => stack = stack.push(under),
        (None, Some(context)) => stack = stack.push(context),
        (None, None) => {}
    }
    let mut contents = Row::new().push(
        container(marker)
            .width(Length::Fixed(theme::TRACK_NO_W))
            .height(Length::Fixed(theme::PANEL_SLEEVE))
            .align_x(alignment::Horizontal::Right)
            .align_y(alignment::Vertical::Center),
    );
    if let Some(artwork) = artwork {
        contents = contents.push(
            container(artwork)
                .width(Length::Fixed(theme::PANEL_SLEEVE))
                .height(Length::Fixed(theme::PANEL_SLEEVE)),
        );
    }
    // `Fill` constrains layout but not painting. No-wrap title text and the
    // independently clickable metadata children can have a larger intrinsic
    // width, so clip the flexible lane before the fixed Album column and
    // trailing controls.
    contents = contents.push(container(stack).width(Length::Fill).clip(true));
    if let Some(context) = table_context {
        contents = contents.push(
            container(context)
                .width(Length::Fixed(theme::PLAYLIST_ALBUM_W))
                .height(Length::Fixed(theme::PANEL_SLEEVE))
                .align_y(alignment::Vertical::Center)
                .clip(true),
        );
    }
    contents = contents
        .push(
            container(
                text(duration)
                    .size(theme::SIZE_META)
                    .line_height(theme::LEADING_META)
                    .color(room.paper_faint)
                    .wrapping(text::Wrapping::None),
            )
            .width(Length::Fixed(theme::DURATION_W))
            .height(Length::Fixed(theme::CAPTION_LINE_H))
            .align_x(alignment::Horizontal::Right)
            .align_y(alignment::Vertical::Center),
        )
        .spacing(theme::GAP_SM)
        .align_y(iced::Alignment::Center);
    button(contents)
        .width(Length::Fill)
        .padding(theme::pad(theme::GAP_XS, 0.0))
        .style(move |_theme, status| {
            theme::selectable_track_row(room, room.wall, status, playing, selected)
        })
        .on_press_maybe(press)
        .into()
}

/// One independently actionable fact inside a track row. The surrounding row
/// keeps its playback press; child controls capture the artist or album press
/// first, so one piece of text has one unambiguous destination.
fn metadata_label(
    label: std::borrow::Cow<'_, str>,
    ink: iced::Color,
    press: Option<Message>,
) -> Element<'_, Message> {
    let room = theme::active();
    if let Some(message) = press {
        // A real button supplies keyboard focus/activation as well as the
        // pointer cursor. Outside its exact bounds the surrounding row keeps
        // its own selection/play grammar.
        let linked_label = text(label)
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            .color(room.paper_dim)
            .wrapping(text::Wrapping::None);
        mouse_area(
            button(linked_label)
                .padding(0)
                .style(move |_theme, status| theme::word_button(room, room.wall, status))
                .on_press(message),
        )
        .interaction(mouse::Interaction::Pointer)
        .into()
    } else {
        text(label)
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            .color(ink)
            .wrapping(text::Wrapping::None)
            .into()
    }
}

/// **One row's reserved control slot**: the drawn glyph while the pointer is
/// on the row, and a space of exactly [`theme::STEPPER_HIT`] when it is not,
/// so no duration slides as the pointer crosses a row.
///
/// There were four copies of this — a record row's `+`, a playlist row's `+`,
/// its ▲ and ▼, and its ✕ — differing in the glyph, the tooltip and whether
/// the control could act. Those three are the arguments; everything else (the
/// square, the reservation, the ink, the tooltip's own anatomy) was identical
/// in all four and is written here once.
///
/// Icon-only, so the tooltip carries the name (doc 10 §3.1); `can` is false
/// where the act is unavailable at this row — the first row's ▲ — and the
/// glyph dims rather than vanishing, because a slot that emptied would move
/// the row.
pub(crate) fn icon_slot(
    glyph: icon::Glyph,
    name: &'static str,
    can: bool,
    offered: bool,
    message: Message,
) -> Element<'static, Message> {
    let room = theme::active();
    if !offered {
        return Space::new().width(Length::Fixed(theme::STEPPER_HIT)).into();
    }
    let mark = container(
        iced_image(icon::handle(glyph))
            .width(Length::Fixed(theme::ICON_PX))
            .height(Length::Fixed(theme::ICON_PX))
            .opacity(if can {
                theme::GLYPH_OPACITY_HOVER
            } else {
                theme::GLYPH_OPACITY_DISABLED
            }),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(alignment::Horizontal::Center)
    .align_y(alignment::Vertical::Center);
    tooltip(
        button(mark)
            .width(Length::Fixed(theme::STEPPER_HIT))
            .height(Length::Fixed(theme::STEPPER_HIT))
            .padding(0)
            .style(move |_theme, status| theme::transport(room, room.wall, status))
            .on_press_maybe(can.then_some(message)),
        text(name)
            .size(theme::SIZE_CAPTION)
            .line_height(theme::LEADING_CAPTION),
        tooltip::Position::Left,
    )
    .gap(theme::GAP_XS)
    .padding(theme::GAP_XS)
    .style(move |_theme| theme::tooltip(room))
    .into()
}

/// **The transfer `+`** (09 §8.1): this row's track, toward a destination of
/// the user's choosing — the picker opens holding it, its first row the Queue.
///
/// One function, and one tooltip string, for the three surfaces that draw it:
/// a record page's track row, a playlist page's entry row, and the wall's
/// `Songs` rows. They had three copies of the same words.
pub(crate) fn transfer_slot(offered: bool, message: Message) -> Element<'static, Message> {
    icon_slot(
        icon::Glyph::Plus,
        "Add to a playlist, or the queue",
        true,
        offered,
        message,
    )
}

/// The one shared Favourites action. It is always present in the row's
/// reserved trailing lane; changing membership changes ink, never geometry,
/// selection or playback.
pub(crate) fn favourite_slot(path: &std::path::Path, favourite: bool) -> Element<'static, Message> {
    icon_slot(
        if favourite {
            icon::Glyph::HeartFilled
        } else {
            icon::Glyph::Heart
        },
        if favourite {
            "Remove from Favourites"
        } else {
            "Add to Favourites"
        },
        true,
        true,
        Message::ToggleFavourite(path.to_path_buf()),
    )
}

/// The playing row's lamp dot — the same amber circle, and the same token, the
/// wall puts beside the playing record and the run column beside its row.
pub(crate) fn lamp_dot() -> Element<'static, Message> {
    let room = theme::active();
    container(
        Space::new()
            .width(Length::Fixed(theme::DOT))
            .height(Length::Fixed(theme::DOT)),
    )
    .style(move |_theme| theme::lamp_dot(room))
    .into()
}

#[cfg(test)]
mod tests {
    use super::{is_playlist_two_column, is_two_column};
    use crate::theme;

    #[test]
    fn the_flexible_track_identity_is_clipped_before_fixed_columns() {
        let source = include_str!("page.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("page source has a non-test head");
        assert!(
            source.contains("container(stack).width(Length::Fill).clip(true)"),
            "long title and metadata ink can escape into the Album column"
        );
    }

    /// Both view sources, **code only** — no test module and no comment lines.
    ///
    /// A file that names a token in prose in order to say it is deliberately
    /// *not* using it is not a consumer, and a sweep that could not tell the
    /// difference would punish a page for explaining itself. This is
    /// `theme::the_serif_is_the_work_titles_and_nothing_else`'s own rule.
    fn pages() -> [(&'static str, String); 2] {
        let read = |file: &str| {
            let source = std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(file),
            )
            .expect("a view's own source")
            .replace("\r\n", "\n");
            source
                .split("#[cfg(test)]")
                .next()
                .expect("a source has a head")
                .lines()
                .filter(|line| {
                    let line = line.trim_start();
                    !(line.starts_with("//") || line.starts_with("/*"))
                })
                .collect::<Vec<_>>()
                .join("\n")
        };
        [
            ("a record's", read("src/views/album.rs")),
            ("a playlist's", read("src/views/playlist.rs")),
        ]
    }

    /// **The page's two columns add up to the window** at every width the
    /// two-column arrangement is used at, and stop growing when the list
    /// reaches its measure.
    ///
    /// This is the arithmetic `views::settings`'s `content_width` needed a
    /// rendered frame to catch (the segmented control ran 998 px wide inside a
    /// 640 px cap), asserted here instead — the widths are the composition's
    /// own arithmetic and nothing about them depends on the toolkit. It lived
    /// in `views::album` while the record's page owned a copy of it; there is
    /// one copy now, so the test is where the arithmetic is.
    #[test]
    fn the_page_fills_the_window_until_its_list_reaches_its_measure() {
        let list = |w: f32| {
            (w - 2.0 * theme::HANG - theme::SCROLLBAR_LANE - theme::ALBUM_ASIDE_W - theme::GAP_XL)
                .clamp(0.0, theme::LIST_MEASURE)
        };
        let page = |w: f32| theme::ALBUM_ASIDE_W + theme::GAP_XL + list(w);

        // At the shipped window the page hangs from both gutters exactly, the
        // scrollbar's declared lane included.
        let inner = |w: f32| w - 2.0 * theme::HANG - theme::SCROLLBAR_LANE;
        assert!((page(1280.0) - inner(1280.0)).abs() < f32::EPSILON);
        // At 1920 the list has reached its measure, so the page stops growing
        // and centres in what is left rather than setting a track title and a
        // duration 1500 px apart.
        assert!((list(1920.0) - theme::LIST_MEASURE).abs() < f32::EPSILON);
        assert!(page(1920.0) < inner(1920.0));
        // And the breakpoint is where the list stops being wider than the
        // sleeve beside it, which is the point at which two columns have
        // stopped being two columns.
        assert!(list(theme::ALBUM_BREAKPOINT) <= theme::ALBUM_ASIDE_W);
        assert!(list(theme::ALBUM_BREAKPOINT + 4.0 * theme::HANG) > theme::ALBUM_ASIDE_W);
    }

    #[test]
    fn a_playlist_stacks_before_its_title_lane_collapses() {
        assert!(is_two_column(theme::PLAYLIST_BREAKPOINT - 1.0));
        assert!(!is_playlist_two_column(theme::PLAYLIST_BREAKPOINT - 1.0));
        assert!(is_playlist_two_column(theme::PLAYLIST_BREAKPOINT));

        let main = theme::PLAYLIST_BREAKPOINT
            - 2.0 * theme::HANG
            - theme::SCROLLBAR_LANE
            - theme::ALBUM_ASIDE_W
            - theme::GAP_XL;
        let fixed = theme::TRACK_NO_W
            + theme::PANEL_SLEEVE
            + theme::PLAYLIST_ALBUM_W
            + theme::DURATION_W
            + 4.0 * theme::GAP_SM
            + 4.0 * theme::STEPPER_HIT
            + 4.0 * theme::GAP_XS;
        assert!(
            main - fixed >= 140.0,
            "the desktop playlist left only {} px for its title",
            main - fixed
        );
    }

    /// **The two identity blocks are the same height** — 80 px, a record's —
    /// and that is the whole of the answer to *"we do not have the playlist
    /// name really prominent"* (ADR-0024 §A4.3).
    ///
    /// The name was never small: it is the album title's own hero size, 28 px,
    /// and always was. What made it read as a stub was
    /// that the block *stopped* after 52 px — the record's byline line was
    /// missing, so a 28 px name was followed straight by a 12 px count, where a
    /// record's is given a 19 px line of support first.
    #[test]
    fn the_identity_block_is_eighty_pixels_of_one_composition() {
        let block =
            theme::LINE_HERO + theme::GAP_XS + theme::LINE_TITLE + theme::GAP_XS + theme::LINE_META;
        assert!(
            (block - 80.0).abs() < f32::EPSILON,
            "32 + 4 + 24 + 4 + 16 = 80, the block both pages wear: {block}"
        );
    }

    /// **Neither page composes itself any more**, which is what makes the
    /// equality above a fact rather than a coincidence two files happen to
    /// share.
    ///
    /// The test this replaces read both sources for `SIZE_HERO`,
    /// `LEADING_TITLE`, `paper_dim` and six other tokens and asserted that each
    /// appeared in both — a way of checking that two hand-built blocks still
    /// matched. There is one block now, so the assertion inverts: the
    /// composition's tokens must appear in **neither** view, because a view
    /// that named one would be building a second page beside the shared one.
    ///
    /// The list is the vocabulary of the arrangement — the identity block's
    /// ramp, the aside's width, the breakpoint, the measure, the scroll and the
    /// section rule. It is deliberately not exhaustive: it is the set of things
    /// that were literally duplicated on 2026-08-10, and a new duplicate that
    /// avoids all of them is a new duplicate somebody chose.
    #[test]
    fn the_two_pages_are_one_composition() {
        let pages = pages();
        for (page, source) in &pages {
            for token in [
                "theme::SIZE_HERO",
                "theme::LEADING_HERO",
                "theme::LINE_HERO",
                "theme::ALBUM_ASIDE_W",
                "theme::ALBUM_BREAKPOINT",
                "theme::LIST_MEASURE",
                "theme::SCROLLBAR_LANE",
                "place_pad()",
                "scrollable(",
            ] {
                assert!(
                    !source.contains(token),
                    "{page} page names {token} — the arrangement is \
                     `views::page` and a view that lays itself out again is \
                     the drift ADR-0024 §A2 was made literal to end"
                );
            }
        }
        assert!(
            pages[0].1.contains("page::view(") && pages[0].1.contains("Page {"),
            "a record page must reach the shared subject composition"
        );
        assert!(
            pages[1].1.contains("playlist_page::view(")
                && !pages[1]
                    .1
                    .replace("playlist_page::view(", "")
                    .contains("page::view("),
            "a saved playlist must reach the playlist compositor rather than \
             growing another direct subject-page call"
        );
    }

    /// **The hero's face is the axis, and it is named at the call sites.**
    ///
    /// The two pages differ in what the words say and in the face the name is
    /// set in, and in nothing else about the block (design 14 §5.2). The token
    /// stays out of this module on purpose:
    /// `theme::the_serif_is_the_work_titles_and_nothing_else` enumerates the
    /// files allowed to name `WORK_TITLE`, and a shared composition holding it
    /// would put the serif one argument away from every page in the product.
    #[test]
    fn the_record_sets_its_hero_in_the_serif_and_the_list_does_not() {
        let [(_, found), (_, made)] = pages();
        assert!(
            found.contains("face: theme::WORK_TITLE"),
            "a record's title is a work's, and it is set in the placard's italic"
        );
        assert!(
            made.contains("face: theme::SEMIBOLD") && !made.contains("WORK_TITLE"),
            "a playlist's name is a label the owner typed, and it is sans"
        );
        assert!(
            !this_module().contains("WORK_TITLE"),
            "the shared composition must not name the serif"
        );
    }

    /// This module's own code, for the assertion above — comments stripped by
    /// [`pages`]'s rule, since the table at the head of this file names the
    /// token in order to say which page carries it.
    fn this_module() -> String {
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/views/page.rs"),
        )
        .expect("this module's own source");
        source
            .split("#[cfg(test)]")
            .next()
            .expect("a source has a head")
            .lines()
            .filter(|line| {
                let line = line.trim_start();
                !(line.starts_with("//") || line.starts_with("/*"))
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// **This page does not box its own lead**, because the shared strip does
    /// it for every place now.
    ///
    /// The defect was found here — a record's sleeve at y = 88 against a
    /// playlist's at y = 77, `docs/design/impl/one-page-two-subjects/` — and
    /// fixed here first, locally, because the general fix moves Queue,
    /// Settings and the Artist place on screen and did not belong in a commit
    /// about these two pages agreeing. The general fix has since landed, and
    /// this asserts the local one went with it: **a second box here would be a
    /// second answer**, and the next person to change the strip's height would
    /// move five places and not seven, which is the drift this composition
    /// exists to end.
    ///
    /// The fact itself is pinned in `views::mod`'s
    /// `every_place_leads_at_the_height_the_frame_declares`.
    #[test]
    fn this_page_leaves_the_leads_box_to_the_shared_strip() {
        assert!(
            !this_module().contains("container(lead)"),
            "this page boxes its own lead again, so the strip's height has two \
             answers and they can drift apart"
        );
    }

    /// **Both pages state what an empty list looks like.**
    ///
    /// A record's page had no empty state at all: with no readable edition it
    /// drew the `TRACKS` rule over nothing, which is the interface saying
    /// neither *"there is nothing"* nor *"something went wrong"*. The playlist
    /// page has had a sentence since doc 09 §9. The slot is the shared
    /// composition's now, so having one is not optional.
    #[test]
    fn neither_page_rules_off_an_empty_list_in_silence() {
        let [(_, record), (_, saved)] = pages();
        assert!(record.contains("empty:"));
        assert!(
            saved.contains("playlist_page::view("),
            "a saved list bypasses the component that owns its empty state"
        );
        let shared = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/views/playlist_page.rs"),
        )
        .expect("the shared playlist composition's source");
        assert!(
            shared.contains("empty: EMPTY"),
            "the shared playlist page hands no empty state to the subject page"
        );
    }
}
