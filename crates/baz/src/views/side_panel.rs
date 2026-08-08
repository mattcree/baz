//! The **album inspector**: large art, header, edition selector, Play, and the
//! selected edition's track list.
//!
//! # The sole tenant of the column
//!
//! It shared the right-hand rail with the play queue and the settings; both
//! have left (ADR-0016), and this is what remains — because it is the one
//! surface that genuinely needs the shelf beside it. The browse loop is *click,
//! read, click the next sleeve*, and a full-window album view would turn a
//! one-click compare into a three-step round trip. Prior art agrees from a
//! direction the audit did not have: every cataloguing product studied in
//! `docs/design/03-interface-prior-art.md` runs a right-hand inspector, and it
//! is the consumption products that run a full page.
//!
//! With one tenant, the rule the column obeys collapses to a sentence — **it is
//! open exactly when an album is selected** ([`crate::selection`]) — and the
//! dismissal model with it.
//!
//! # The track list is a now-playing view of the album
//!
//! Two things the list did not used to do, and they are one change:
//!
//! - **The playing track carries the lamp dot**, in the
//!   [`theme::TRACK_NO_W`] column, in place of its number — exactly as the
//!   queue panel marks it, with the same token behind it, so the column never
//!   changes width and a listener who has seen one surface has learned the
//!   other. Which row (if any) is
//!   [`PlayerState::playing_row_in`](crate::player::PlayerState::playing_row_in)'s
//!   answer, and it is `None` unless the tracks listed here are *exactly* the
//!   queue that is playing — an inspector switched to a different edition of
//!   the album that is sounding marks nothing rather than marking a file the
//!   engine is not reading.
//!
//!   The audit's finding was that the same thirteen titles appeared in two
//!   panels and only the one you did *not* open marked your place. For the
//!   only queue baz can build today — an album, whole, in order — this makes
//!   the queue optional for the listener who plays albums front to back and
//!   never opens one.
//!
//! - **A row is a control**: clicking it plays the album from there
//!   (ADR-0014's `JumpTo`, or a `SetQueue` first when this album is not what
//!   the engine is holding — the decision is
//!   [`PlayerState::play_from`](crate::player::PlayerState::play_from)'s, and
//!   this module makes none of it). A track row in an album list has meant
//!   "play from here" since CD players had displays.
//!
//!   The rows carried no hover affordance and no pointer cursor before,
//!   deliberately, because there was no command to send and "an affordance
//!   that does nothing is a lie". They carry one now, and the rule is the same
//!   rule read forwards.
//!
//! Nothing here marks a row optimistically. The dot follows `TrackStarted`
//! through [`crate::player`] like every other reading in the interface, so a
//! click that the engine answers differently from — or not at all — leaves the
//! list saying what is true.

use std::time::Duration;

use iced::widget::{
    Column, Space, button, column, container, horizontal_rule, image as iced_image, row,
    scrollable, text,
};
use iced::{Element, Length, alignment};

use crate::app::{Message, Shelf};
use crate::player::{Availability, PlayerState};
use crate::views::{close_button, gradient_block};
use crate::{icon, theme, vm};

/// Side-panel inner padding (logical px).
const PANEL_PAD: f32 = theme::GAP_XL;

/// The album side panel: large art, a title/artist/meta header, the
/// edition selector when the album is owned in more than one format, the
/// primary Play action, and the selected edition's numbered track list
/// (durations right-hugged). In a build without audio
/// output the button is hidden; with an unusable or closed engine it
/// renders disabled.
pub(crate) fn view<'a>(
    shelf: &'a Shelf,
    album: &'a vm::AlbumVm,
    player: &'a PlayerState,
) -> Element<'a, Message> {
    let room = theme::active();
    let playing = player.playing_album() == Some(album.id);
    let art_edge = theme::PANEL_W - 2.0 * PANEL_PAD;
    let art: Element<'_, Message> = match shelf.thumbs.peek(&album.id) {
        Some(handle) => iced_image(handle.clone())
            .width(Length::Fixed(art_edge))
            .into(),
        None => gradient_block(album.id, art_edge),
    };
    let sleeve = container(art).style(move |_theme| theme::sleeve(room, playing));
    let chosen = shelf.edition_choice.get(&album.id).copied();
    let edition = vm::selected_edition(album, chosen);
    // A soundtrack grouped under one album artist keeps its per-cue
    // composer credits; an ordinary album gains no extra line.
    let per_track_artists = album.track_artists_vary;
    // Where the music is in *this* list — `None` unless what is listed is
    // exactly the queue that is playing (module docs). Asked once for the
    // whole list rather than once per row, because it is one fact about the
    // list and not a property of any row.
    let playing_row = edition.and_then(|edition| player.playing_row_in(&edition.tracks));
    // A row is only a control when there is an engine to send its command to,
    // exactly as `Play album` above it is.
    let interactive = player.engine_ready();
    // The rows, with a disc header pushed in ahead of each disc **after the
    // first** — and only when the tags actually carry disc numbers. See
    // [`disc_header`].
    let mut rows: Vec<Element<'_, Message>> = Vec::new();
    if let Some(edition) = edition {
        let multi_disc = vm::discs(edition).is_some_and(|discs| discs > 1);
        let mut current: Option<u32> = None;
        for (index, track) in edition.tracks.iter().enumerate() {
            if multi_disc && track.disc.is_some() && track.disc != current {
                current = track.disc;
                if let Some(disc) = current {
                    rows.push(disc_header(disc));
                }
            }
            rows.push(track_row(
                track,
                per_track_artists,
                playing_row == Some(index),
                interactive.then_some(Message::PlayTrack(album.id, index)),
            ));
        }
    }

    // The dismissal ✕ sits in a row of its own above the sleeve — the same
    // slot, in the same panel width, as the queue's, so closing a panel is one
    // control in one place whichever is on screen.
    let header_row = row![
        Space::with_width(Length::Fill),
        close_button("Close the album panel", Message::ClosePanel),
    ]
    .align_y(iced::Alignment::Center);
    let mut content =
        column![header_row, sleeve, album_header(album, edition)].spacing(theme::GAP_MD);
    // Only a genuinely multi-format album gets a control; a single-format
    // album must look exactly as it always did.
    if album.editions.len() > 1 {
        content = content.push(edition_selector(album, edition));
    }
    if *player.availability() != Availability::NotBuilt {
        content = content.push(play_album(album.id, player.engine_ready()));
    }
    let hint = if *player.availability() == Availability::NotBuilt {
        "Esc closes · built without audio output"
    } else {
        "Esc closes · double-click a tile to play"
    };
    content = content
        .push(track_list(rows, details(album, edition)))
        .push(
            text(hint)
                .size(theme::SIZE_CAPTION)
                .line_height(theme::LEADING_CAPTION)
                .color(room.paper_faint),
        );

    container(content)
        .width(Length::Fixed(theme::PANEL_W))
        .height(Length::Fill)
        .padding(PANEL_PAD)
        .style(move |_theme| theme::panel(room))
        .into()
}

/// The scrolling track list, with the lane its scrollbar needs kept clear.
///
/// iced draws a `scrollable`'s bar **over** the right edge of its contents
/// rather than beside them, so a list long enough to scroll was clipping the
/// last character of every duration — `1:15` read as `1:1`. The rows stop at
/// [`theme::scroll_gutter`] instead, which reserves exactly the width the bar
/// occupies (the two are one token, see [`theme::SCROLLBAR_W`]).
///
/// The lane is reserved **whether or not the list currently overflows**, so a
/// twelfth track arriving does not shunt eleven durations sideways; it is the
/// same reserved-slot rule the bottom bar's timestamps and signal note follow.
/// Nothing else about the panel moves: the padding, the number column, the
/// gaps and the panel width are untouched.
///
/// # Details rides in the same scroll
///
/// `details` is appended **inside** this scrollable rather than under it, and
/// that is the whole of the interaction the block was specified with: *no
/// disclosure, no click* — it is the back of the record's card, and you turn it
/// over by scrolling past the last track. Devon never sees it; Marta never has
/// to ask for it. A separate scroll region, or a twisty, would make it a
/// feature you have to discover instead of a page you reach.
fn track_list<'a>(
    rows: Vec<Element<'a, Message>>,
    details: Element<'a, Message>,
) -> Element<'a, Message> {
    let room = theme::active();
    scrollable(
        column![Column::with_children(rows).spacing(theme::GAP_XXS), details]
            .spacing(theme::GAP_XL)
            .padding(theme::scroll_gutter()),
    )
    .direction(scrollable::Direction::Vertical(theme::list_scrollbar()))
    .style(move |_theme, status| theme::scrollbar(room, status))
    .height(Length::Fill)
    .into()
}

/// The primary action: **Play album**, a lamp outline with a paper triangle
/// and a paper label, and the only control in baz drawn in the accent.
///
/// It is the switch that turns the picture light on — the one control in the
/// product that *creates* playback truth — which is why it is allowed the
/// colour and why there is at most one of it on screen. The paint is
/// [`theme::primary`], where the argument for an outline rather than an amber
/// slab lives.
///
/// The glyph is [`room.paper`] rather than amber, and that is a deviation
/// from `.interface-design/system.md` §5 taken deliberately and cheaply:
/// [`crate::icon`] rasterises one sprite sheet in one ink, so a second colour
/// costs a second sheet and names an amber token in a module the accent
/// discipline's source scan would then have to exempt. The accent is on the
/// border, where a 1 px line is exactly what the refusal permits, and the
/// triangle is the *shape* that says play. Revisit if the sheet ever gains a
/// second ink for another reason.
///
/// [`theme::TRANSPORT_HIT`] tall and the column's full width, so it reads as
/// the panel's one commitment rather than as another button in a row of them.
fn play_album(album: u64, live: bool) -> Element<'static, Message> {
    let room = theme::active();
    button(
        row![
            iced_image(icon::handle(icon::Glyph::Play))
                .width(Length::Fixed(theme::ICON_PX))
                .height(Length::Fixed(theme::ICON_PX))
                .opacity(theme::glyph_opacity(live, false)),
            text("Play album")
                .size(theme::SIZE_BODY)
                .line_height(theme::LEADING_BODY)
                .font(theme::SEMIBOLD)
                .wrapping(text::Wrapping::None),
        ]
        .spacing(theme::GAP_SM)
        .align_y(iced::Alignment::Center),
    )
    .width(Length::Fill)
    .height(Length::Fixed(theme::TRANSPORT_HIT))
    .padding(theme::pad(0.0, theme::GAP_MD))
    .style(move |_theme, status| theme::primary(room, status))
    .on_press_maybe(live.then_some(Message::PlayAlbum(album)))
    .into()
}

/// A disc break in the track list — `DISC 2` in the room's quietest voice,
/// with air above it.
///
/// **Data-driven, never faked** (`docs/design/critique/02-surfaces.md`): drawn
/// only when the edition's tags carry disc numbers *and* they name more than
/// one disc. A single-disc record gets no header, and — the case that matters
/// — a two-disc rip whose tagger never wrote the field gets no header either,
/// because inventing `DISC 1` over the first eleven tracks would be the
/// interface claiming to know something it does not.
///
/// The spec asks for `SIDE A` / `SIDE B`. baz's schema carries **discs**, not
/// sides: no tag baz reads distinguishes the two halves of a record, so sides
/// would have to be inferred from a disc number, which is exactly the faking
/// the same sentence forbids. This is the same header mechanism wearing the
/// name of the data that exists; sides arrive here unchanged the day the
/// scanner reads one.
fn disc_header(disc: u32) -> Element<'static, Message> {
    let room = theme::active();
    container(
        text(format!("DISC {disc}"))
            .size(theme::SIZE_CAPTION)
            .line_height(theme::LEADING_CAPTION)
            .font(theme::MEDIUM)
            .color(room.heading())
            .wrapping(text::Wrapping::None),
    )
    .padding(theme::pad(theme::GAP_SM, theme::GAP_XS))
    .into()
}

/// **Details** — the condition report in full, below the fold.
///
/// A hairline, the word `Details` in the room's quietest voice, then one row
/// per field the scan actually read: the label right-aligned in
/// [`theme::FIELD_LABEL_W`], the value left-aligned after it, at
/// [`theme::DETAIL_ROW_H`] pitch. It is a reference table you scan, not prose
/// you read, which is why the pitch is tighter than the type's own leading.
///
/// `docs/design/03-interface-prior-art.md` R6 is the argument: fooyin shows
/// twenty fields for free and baz showed four, and baz's audience came from
/// products in the first camp. What decides the row list is
/// [`vm::details`] — including its refusal to invent a row for a field the
/// tags do not carry.
///
/// Empty when the scan read nothing at all, in which case the block is not
/// drawn: a heading over nothing is worse than no heading.
fn details<'a>(album: &'a vm::AlbumVm, edition: Option<&'a vm::EditionVm>) -> Element<'a, Message> {
    let room = theme::active();
    let rows = vm::details(album, edition);
    if rows.is_empty() {
        return Space::with_height(Length::Fixed(0.0)).into();
    }
    // The rows carry their own pitch ([`theme::DETAIL_ROW_H`]) and take no
    // spacing from the column, or the table would read at 25 px a line — a
    // page rather than a card.
    let mut table = column![];
    for (label, value) in rows {
        table = table.push(
            container(
                row![
                    container(
                        text(label)
                            .size(theme::SIZE_META)
                            .line_height(theme::LEADING_META)
                            .color(room.paper_muted)
                            .wrapping(text::Wrapping::None)
                    )
                    .width(Length::Fixed(theme::FIELD_LABEL_W))
                    .align_x(alignment::Horizontal::Right),
                    text(value)
                        .size(theme::SIZE_META)
                        .line_height(theme::LEADING_META)
                        .color(room.paper_dim)
                        .wrapping(text::Wrapping::None),
                ]
                .spacing(theme::GAP_SM),
            )
            .height(Length::Fixed(theme::DETAIL_ROW_H))
            .clip(true),
        );
    }
    column![
        horizontal_rule(1).style(move |_theme| theme::hairline(room)),
        text("Details")
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            .font(theme::MEDIUM)
            .color(room.heading()),
        table,
    ]
    .spacing(theme::GAP_SM)
    .into()
}

/// The side panel's header: album title over artist over a quiet
/// year · tracks · total-time meta line, and — when the scan read one — the
/// selected edition's encoding fingerprint under it.
///
/// The counts describe `edition`, not the album: with two rips on disk, "24
/// tracks" would be a number nothing on screen adds up to.
fn album_header<'a>(
    album: &'a vm::AlbumVm,
    edition: Option<&'a vm::EditionVm>,
) -> Element<'a, Message> {
    let room = theme::active();
    let title = album.title.as_deref().unwrap_or("Unknown Album");
    let artist = album.artist.label();
    let tracks = edition.map_or(0, |edition| edition.tracks.len());
    let mut meta: Vec<String> = Vec::new();
    if let Some(year) = album.year {
        meta.push(year.to_string());
    }
    meta.push(match tracks {
        1 => "1 track".to_owned(),
        n => format!("{n} tracks"),
    });
    let total: Duration = edition
        .into_iter()
        .flat_map(|edition| edition.tracks.iter())
        .filter_map(|t| t.duration)
        .sum();
    if total > Duration::ZERO {
        meta.push(vm::format_duration(total));
    }
    // Four voices, four sizes, four inks, in one falling order: the work, who
    // made it, what it is, what it is made of. Each line is quieter than the
    // one above it, which is the whole of the hierarchy — there is no rule, no
    // surface and no colour anywhere in this header.
    let mut header = column![
        // The title clips at **two lines**. `Wrapping::None` does not stop iced
        // 0.13 laying a long string over several lines (the same behaviour the
        // shelf's caption lanes work around), and a box-set title running to
        // four lines pushes the artist, the catalogue line and the Play button
        // down the panel. Two lines is a title; more is a paragraph.
        container(
            text(title)
                .size(theme::SIZE_TITLE)
                .line_height(theme::LEADING_TITLE)
                .font(theme::SEMIBOLD)
                .color(room.paper)
        )
        .max_height(2.0 * theme::SIZE_TITLE * theme::LEADING_TITLE)
        .clip(true),
        text(artist)
            .size(theme::SIZE_EMPHASIS)
            .line_height(theme::LEADING_EMPHASIS)
            .color(room.paper_dim),
        // The catalogue line: `1992 · 13 tracks · 45:35`.
        text(meta.join(" · "))
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            .color(room.paper_faint),
    ]
    .spacing(theme::GAP_XS);
    if let Some(line) = edition.and_then(vm::EditionVm::encoding_line) {
        header = header.push(
            text(line)
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.paper_faint),
        );
    }
    header.into()
}

/// The edition selector: a quiet segmented control, one segment per format
/// the album is owned in, in the library's best-first order.
///
/// Shown only when there is a choice to make — a single-format album carries
/// no control at all, so the ordinary case gains no chrome. The choice
/// changes what the panel lists and what Play queues, and nothing else; it
/// never interrupts what is already playing.
fn edition_selector<'a>(
    album: &'a vm::AlbumVm,
    selected: Option<&'a vm::EditionVm>,
) -> Element<'a, Message> {
    let room = theme::active();
    let selected_key = selected.map(|edition| edition.key);
    let mut segments = row![].spacing(theme::GAP_XXS);
    for edition in &album.editions {
        let is_selected = selected_key == Some(edition.key);
        segments = segments.push(
            button(
                container(
                    text(edition.key.label())
                        .size(theme::SIZE_META)
                        .line_height(theme::LEADING_META)
                        .font(theme::MEDIUM)
                        .wrapping(text::Wrapping::None),
                )
                .width(Length::Fill)
                .align_x(alignment::Horizontal::Center),
            )
            .width(Length::Fill)
            .padding(theme::pad(theme::GAP_XS, theme::GAP_SM))
            .style(move |_theme, status| theme::segment(room, status, is_selected))
            .on_press(Message::EditionSelected(album.id, edition.key)),
        );
    }
    container(segments)
        .width(Length::Fill)
        .padding(theme::SEGMENT_INSET)
        .style(move |_theme| theme::segmented(room))
        .into()
}

/// One track-list row: right-aligned number — or the lamp dot when this is the
/// track sounding — title, right-aligned duration; and a press that plays the
/// album from here.
///
/// The dot goes **in** the number column rather than beside it, at
/// [`theme::TRACK_NO_W`], which is the queue panel's arrangement and is what
/// makes the mark arriving as a track starts move no text: the column is the
/// same width whichever it holds.
///
/// `press` is `None` when there is no engine to ask, and the row then renders
/// as the inert text it always was — a disabled control rather than a live one
/// that would do nothing, which is the same distinction `Play album` makes
/// directly above the list.
///
/// With `show_artist`, the track's own artist sits under its title in the
/// quiet meta style — the same title-over-artist stack the now-playing bar
/// uses. It is passed in rather than decided here because the answer is a
/// property of the whole album ([`vm::AlbumVm::track_artists_vary`]): every
/// row of a soundtrack shows its composer, or none does.
fn track_row(
    track: &vm::TrackVm,
    show_artist: bool,
    playing: bool,
    press: Option<Message>,
) -> Element<'_, Message> {
    let room = theme::active();
    let duration = track.duration.map(vm::format_duration).unwrap_or_default();
    let marker: Element<'_, Message> = if playing {
        lamp_dot()
    } else {
        text(track.number.map(|n| n.to_string()).unwrap_or_default())
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            .color(room.paper_faint)
            .into()
    };
    // The playing row's title takes the medium weight the now-playing bar and
    // the queue both give the same string — one more place the three surfaces
    // agree about what is sounding.
    let heading = text(track.title.as_str())
        .size(theme::SIZE_BODY)
        .line_height(theme::LEADING_BODY)
        .wrapping(text::Wrapping::None);
    let heading = if playing {
        heading.font(theme::MEDIUM)
    } else {
        heading
    };
    let mut title = column![heading].spacing(theme::GAP_XXS);
    if let Some(artist) = track.artist.as_deref().filter(|_| show_artist) {
        title = title.push(
            text(artist)
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.paper_dim)
                .wrapping(text::Wrapping::None),
        );
    }
    button(
        row![
            // The number column and the duration lane are centred on the
            // **title's own line**, not on the row's block, and the row is
            // top-aligned so they stay there. Centred on the block, a
            // soundtrack row that carries a composer under its title dragged
            // its number and its duration halfway down two lines while every
            // single-line row above kept them on one — so a list of thirteen
            // tracks had thirteen figures on eleven different baselines. The
            // lane is [`theme::CAPTION_LINE_H`], the height of one line of body
            // text, which is what the title occupies whatever follows it.
            container(marker)
                .width(Length::Fixed(theme::TRACK_NO_W))
                .height(Length::Fixed(theme::CAPTION_LINE_H))
                .align_x(alignment::Horizontal::Right)
                .align_y(alignment::Vertical::Center),
            container(title).width(Length::Fill),
            // The duration lives in a reserved [`theme::DURATION_W`] lane,
            // right-aligned. Sized to its own string it was not a column at
            // all: `9:41` and `12:07` ended on different pixels, so a
            // thirteen-track record had a ragged right edge where the
            // proportional face's tabular figures could have given it a ruled
            // one. Figure columns are right-aligned (§8.2) — ragged-left reads
            // fine editorially and pins the edge the eye follows.
            container(
                text(duration)
                    .size(theme::SIZE_META)
                    .line_height(theme::LEADING_META)
                    .color(room.paper_faint)
                    .wrapping(text::Wrapping::None)
            )
            .width(Length::Fixed(theme::DURATION_W))
            .height(Length::Fixed(theme::CAPTION_LINE_H))
            .align_x(alignment::Horizontal::Right)
            .align_y(alignment::Vertical::Center),
        ]
        .spacing(theme::GAP_SM)
        .align_y(iced::Alignment::Start),
    )
    .width(Length::Fill)
    .padding(theme::pad(theme::GAP_XS, theme::GAP_XS))
    .style(move |_theme, status| theme::track_row(room, status, playing))
    .on_press_maybe(press)
    .into()
}

/// The playing track's lamp dot — the same amber circle, and the same token,
/// the shelf puts beside the playing album and the queue beside its row.
fn lamp_dot() -> Element<'static, Message> {
    let room = theme::active();
    container(Space::new(
        Length::Fixed(theme::DOT),
        Length::Fixed(theme::DOT),
    ))
    .style(move |_theme| theme::lamp_dot(room))
    .into()
}
