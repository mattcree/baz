//! **The record's page**: one album, at the width of the window.
//!
//! # It is a place now, and that is a re-lay rather than a re-parent
//!
//! This was `side_panel.rs`, a 340 px column beside the shelf. ADR-0022 removed
//! every side surface baz had, and the album's content is the half of the
//! inspector worth keeping — so it is here, and it is **laid out again** rather
//! than stretched. A 340 px column and a 1200 px page are not the same
//! composition at two sizes:
//!
//! | | the column | this page |
//! |---|---|---|
//! | shape | one lane, everything stacked | two columns: the object, and what is written about it |
//! | the title | `SIZE_TITLE` 19, fifth of eight by ink | `SIZE_HERO` 28, second, and first among type |
//! | the sleeve | capped at 120 so it could not dominate | `ART_MAX` 320 — it is the *only* image of the record on screen |
//! | `Details` | below the fold, reached by scrolling past the tracks | beside the sleeve, above the fold at every shipped width |
//!
//! # The sleeve is allowed to be first here, and that is not the audit's defect
//!
//! `docs/design/06-composition-audit.md` found the inspector's sleeve at
//! **93.6 %** of the panel's ink with the album's own name **fifth of eight**,
//! and `INSPECTOR_SLEEVE` capped it at 120 to fix it. That defect was not "a
//! large sleeve"; it was *a second, larger copy of a work already on the wall
//! 24 px to the left*, drowning the one thing the panel added.
//!
//! A place has replaced the wall. There is no other copy, the record **is** the
//! subject, and so the declared hierarchy (law L6) is:
//!
//! > **the work ≫ `Play album` → the title → the artist → the catalogue line →
//! > the track list → the condition** — and among *type*, the title is first.
//!
//! The work is first *by declaration*, the way the wall's sleeves are, and the
//! declaration says by how much: measured, it is **88.5 %** of the page's
//! contrast-weighted ink, against the wall's ~135× sleeve-to-label ratio.
//!
//! `Play album` outranks the title and that is not an inversion: it is a 1 px
//! amber border around a 320 × 32 box — 704 px of full-contrast accent — beside
//! five glyphs of 28 px type, and it is *the one commitment the page makes*.
//! What the audit's defect 5 was actually about is the line under it, and that
//! line holds: **the title is the loudest type on the page by a clear step**
//! (`SIZE_HERO` over `SIZE_TITLE` over `SIZE_META`), where in the 340 px column
//! the album's own name came fifth of eight. The measured tables are at
//! `docs/design/impl/places/README.md`.
//!
//! # The track list is a now-playing view of the album
//!
//! Unchanged from the column, and deliberately: **the playing track carries the
//! lamp dot** in the [`theme::TRACK_NO_W`] column in place of its number — the
//! same mark, the same token and the same width as the queue place's rows — and
//! **a row is a control**, playing the album from there (ADR-0014's `JumpTo`, or
//! a `SetQueue` first when this album is not what the engine is holding; the
//! decision is [`PlayerState::play_from`](crate::player::PlayerState::play_from)'s
//! and this module makes none of it).
//!
//! Nothing here marks a row optimistically. The dot follows `TrackStarted`
//! through [`crate::player`] like every other reading in the interface.

use std::time::Duration;

use iced::widget::{
    Column, Space, button, column, container, image as iced_image, mouse_area, row, scrollable,
    text,
};
use iced::{Element, Length, alignment};

use crate::app::{Message, Shelf};
use crate::player::{Availability, PlayerState};
use crate::playlists::Collecting;
use crate::views::{gradient_block, place_header, place_pad, section_rule};
use crate::{icon, theme, vm};

/// The record's page: the header strip, then the object beside what is written
/// about it, in one scroll.
///
/// `window_width` decides the arrangement and nothing else. The page grows with
/// the window until its track list reaches [`theme::LIST_MEASURE`] and then
/// stops, centring in what is left — a measure has a comfortable range rather
/// than a single right answer, and a track list set 1500 px wide is a row of
/// two words at opposite ends of the screen. Below
/// [`theme::ALBUM_BREAKPOINT`] the two columns stack, because at that point the
/// list would be narrower than the sleeve beside it and two columns have
/// stopped being two columns.
pub(crate) fn view<'a>(
    shelf: &'a Shelf,
    album: &'a vm::AlbumVm,
    player: &'a PlayerState,
    window_width: f32,
    lamp: f32,
    collecting: Collecting,
    hovered_row: Option<usize>,
) -> Element<'a, Message> {
    let room = theme::active();
    // What the page's own block has to fit in: the window, less the one gutter
    // on both sides and the scrollbar's declared lane on the right
    // ([`place_pad`]).
    let content = (window_width - 2.0 * theme::HANG - theme::SCROLLBAR_LANE).max(0.0);
    let side_by_side = window_width >= theme::ALBUM_BREAKPOINT;
    let tracks_w = if side_by_side {
        (content - theme::ALBUM_ASIDE_W - theme::GAP_XL).clamp(0.0, theme::LIST_MEASURE)
    } else {
        content.min(theme::LIST_MEASURE)
    };

    let body: Element<'_, Message> = if side_by_side {
        row![
            container(aside(shelf, album, player, lamp, collecting))
                .width(Length::Fixed(theme::ALBUM_ASIDE_W)),
            container(main_column(shelf, album, player, collecting, hovered_row))
                .width(Length::Fixed(tracks_w)),
        ]
        .spacing(theme::GAP_XL)
        .align_y(iced::Alignment::Start)
        .into()
    } else {
        column![
            container(aside(shelf, album, player, lamp, collecting))
                .width(Length::Fixed(theme::ALBUM_ASIDE_W)),
            container(main_column(shelf, album, player, collecting, hovered_row))
                .width(Length::Fixed(tracks_w)),
        ]
        .spacing(theme::GAP_XL)
        .into()
    };

    column![
        place_header("Album", "Esc returns to the wall"),
        // **One scroll for the whole page.** The column had two (the panel and
        // its track list) and the popover had one inside another; a page is one
        // document and turning it over is one gesture. The gutter the bar needs
        // is reserved whether or not the page overflows, so a fourteenth track
        // arriving shunts no duration sideways.
        scrollable(
            container(body)
                .width(Length::Fill)
                .padding(place_pad())
                .align_x(alignment::Horizontal::Center)
        )
        .direction(scrollable::Direction::Vertical(theme::list_scrollbar()))
        .style(move |_theme, status| theme::scrollbar(room, room.wall, status))
        .width(Length::Fill)
        .height(Length::Fill),
    ]
    .into()
}

/// The left column: **the object, the one thing you can do to it, and its
/// condition report.**
///
/// It is fixed at [`theme::ALBUM_ASIDE_W`] — the sleeve's own edge — so the
/// three blocks in it share one lane and the page has two x-edges on this side
/// rather than three (law L5).
fn aside<'a>(
    shelf: &'a Shelf,
    album: &'a vm::AlbumVm,
    player: &'a PlayerState,
    lamp: f32,
    collecting: Collecting,
) -> Element<'a, Message> {
    let room = theme::active();
    let playing = player.playing_album() == Some(album.id);
    // The same 200 ms warm the wall gives the sounding record, on the same
    // tween — the page and the tile are two views of one lamp (ADR-0020 §2.5).
    let warmth = if playing { lamp } else { 0.0 };
    let edge = theme::ALBUM_SLEEVE;
    let art: Element<'_, Message> = match shelf.thumbs.peek(&album.id) {
        Some(handle) => iced_image(handle.clone())
            .width(Length::Fixed(edge))
            .height(Length::Fixed(edge))
            .into(),
        None => gradient_block(album.id, edge, 1.0),
    };
    // **`ALBUM_SLEEVE == ART_MAX == THUMB_PX`**, which is the refusal *no
    // artwork is ever drawn larger than its source* satisfied exactly rather
    // than approached: the decoded thumbnail is 320 px on its long edge, and
    // this draws it at 320.
    let sleeve = container(art)
        .width(Length::Fixed(edge))
        .height(Length::Fixed(edge))
        .style(move |_theme| theme::sleeve(room, warmth));

    let chosen = shelf.edition_choice.get(&album.id).copied();
    let edition = vm::selected_edition(album, chosen);
    let mut block = column![sleeve].spacing(theme::GAP_MD);
    if *player.availability() != Availability::NotBuilt {
        block = block.push(play_album(album.id, player.engine_ready()));
    }
    // The layer-1 add (ADR-0024 §6): the two-press route that ships first.
    // It reads the selected album — L8.1 puts it with the album — and stands
    // under the page's one commitment, quiet, no accent: collecting is not
    // playback truth. With a playlist armed the same press is the one-press
    // add layer 2 promises; otherwise it opens the panel as the picker.
    if collecting.available {
        block = block.push(add_to_playlist(album.id, collecting.armed));
    }
    // Only a genuinely multi-format album gets a control; a single-format
    // album must look exactly as it always did.
    if album.editions.len() > 1 {
        block = block.push(edition_selector(album, edition));
    }
    block = block.push(details(album, edition));
    block.into()
}

/// The right column: **who made this, what it is, and every track on it.**
fn main_column<'a>(
    shelf: &'a Shelf,
    album: &'a vm::AlbumVm,
    player: &'a PlayerState,
    collecting: Collecting,
    hovered_row: Option<usize>,
) -> Element<'a, Message> {
    let chosen = shelf.edition_choice.get(&album.id).copied();
    let edition = vm::selected_edition(album, chosen);
    // Where the music is in *this* list — `None` unless what is listed is
    // exactly the queue that is playing. An album page switched to a different
    // edition of the record that is sounding marks nothing rather than marking
    // a file the engine is not reading.
    let playing_row = edition.and_then(|edition| player.playing_row_in(&edition.tracks));
    // A row is only a control when there is an engine to send its command to,
    // exactly as `Play album` is.
    let interactive = player.engine_ready();
    let per_track_artists = album.track_artists_vary;
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
                album.id,
                index,
                collecting,
                hovered_row == Some(index),
            ));
        }
    }
    let mut block = column![album_header(album, edition)].spacing(theme::GAP_XL);
    // Is this record the one the pull is offering? The offer is about *a*
    // record, so it is drawn only on the page that record is on.
    if let Some(note) = shelf
        .pull
        .as_ref()
        .filter(|pull| pull.album == album.id)
        .map(|pull| pull.note.as_str())
    {
        block = block.push(pull_note(note));
    }
    block = block.push(
        column![
            section_rule("Tracks"),
            Column::with_children(rows).spacing(theme::GAP_XS),
        ]
        .spacing(theme::GAP_SM),
    );
    block.into()
}

/// **The pull's line**: `The pull · Last played 3 years ago`.
///
/// Two facts and no third. The first says *this record was suggested, it is not
/// one you went looking for* — without which a page opening on its own would
/// read as a fault. The second is the ledger's own reading
/// ([`crate::shuffle::pull_note`]): a date band, never a score, never a reason,
/// never a "because you liked". History records; it never performs
/// (`docs/REFUSALS.md`).
///
/// There is no button here. The control that accepts the suggestion is the
/// page's own `Play album`, in the place it always sits — so accepting the pull
/// is the *same act*, sending the same commands, as playing a record you found
/// yourself.
fn pull_note(note: &str) -> Element<'static, Message> {
    let room = theme::active();
    row![
        text("The pull")
            .size(theme::SIZE_CAPTION)
            .line_height(theme::LEADING_CAPTION)
            .font(theme::MEDIUM)
            .color(room.paper_dim)
            .wrapping(text::Wrapping::None),
        text(note.to_owned())
            .size(theme::SIZE_CAPTION)
            .line_height(theme::LEADING_CAPTION)
            .color(room.paper_faint)
            .wrapping(text::Wrapping::None),
    ]
    .spacing(theme::GAP_SM)
    .align_y(iced::Alignment::Center)
    .into()
}

/// The primary action: **Play album**, a lamp outline with a paper triangle
/// and a paper label, and the only control in baz drawn in the accent.
///
/// It is the switch that turns the picture light on — the one control in the
/// product that *creates* playback truth — which is why it is allowed the
/// colour and why there is at most one of it on screen.
///
/// It is also, since ADR-0022, **the only pointer route from a record to its
/// sound**: the wall's double-click died with the inspector, because the first
/// press now navigates and there is no tile left for the second to land on. So
/// it takes the sleeve's whole width and stands directly under it, which makes
/// the press that replaced the double-click a 320 × 32 target in a fixed place
/// rather than a 400 ms timing gesture.
fn play_album(album: u64, live: bool) -> Element<'static, Message> {
    let room = theme::active();
    button(
        // **The box centres the ink, in both axes** (law L3).
        container(
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
        .height(Length::Fill)
        .align_x(alignment::Horizontal::Center)
        .align_y(alignment::Vertical::Center),
    )
    .width(Length::Fill)
    .height(Length::Fixed(theme::TRANSPORT_HIT))
    .padding(theme::pad(0.0, theme::GAP_MD))
    .style(move |_theme, status| theme::primary(room, status))
    .on_press_maybe(live.then_some(Message::PlayAlbum(album)))
    .into()
}

/// **Add to playlist** — the record, whole, into a list of the user's
/// choosing (ADR-0024 §6 layer 1).
///
/// The sleeve's width, like `Play album` above it, but a quiet word button
/// rather than the accent: the lamp stays spent on playback truth alone.
/// While a playlist is armed the label says where the press will land, so the
/// one-press add is legible before it is made.
fn add_to_playlist(album: u64, armed: bool) -> Element<'static, Message> {
    let room = theme::active();
    button(
        container(
            text(if armed {
                "+ Add to the open playlist"
            } else {
                "Add to playlist"
            })
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            .font(theme::MEDIUM)
            .wrapping(text::Wrapping::None),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(alignment::Horizontal::Center)
        .align_y(alignment::Vertical::Center),
    )
    .width(Length::Fill)
    .height(Length::Fixed(theme::TRANSPORT_HIT))
    .padding(theme::pad(0.0, theme::GAP_MD))
    .style(move |_theme, status| theme::word_button(room, room.wall, status))
    .on_press(Message::AddAlbumToPlaylist(album))
    .into()
}

/// A disc break in the track list — `DISC 2` in the room's quietest voice.
///
/// **Data-driven, never faked** (`docs/design/critique/02-surfaces.md`): drawn
/// only when the edition's tags carry disc numbers *and* they name more than
/// one disc. A two-disc rip whose tagger never wrote the field gets no header,
/// because inventing `DISC 1` over the first eleven tracks would be the
/// interface claiming to know something it does not.
///
/// The spec asks for `SIDE A` / `SIDE B`. baz's schema carries **discs**, not
/// sides: no tag baz reads distinguishes the two halves of a record, so sides
/// would have to be inferred, which is exactly the faking the same sentence
/// forbids. Sides arrive here unchanged the day the scanner reads one.
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
    .padding(theme::pad(theme::GAP_SM, 0.0))
    .into()
}

/// **Details** — the condition report in full, beside the object.
///
/// One row per field the scan actually read: the label right-aligned in
/// [`theme::FIELD_LABEL_W`], the value left-aligned after it, at
/// [`theme::DETAIL_ROW_H`] pitch. It is a reference table you scan, not prose
/// you read, which is why the pitch is tighter than the type's own leading.
///
/// `docs/design/03-interface-prior-art.md` R6 is the argument: fooyin shows
/// twenty fields for free and baz showed four, and baz's audience came from
/// products in the first camp. What decides the row list is [`vm::details`] —
/// including its refusal to invent a row for a field the tags do not carry.
///
/// **It has moved above the fold.** In the column it rode below the track list
/// in the same scroll, which was the honest arrangement for 340 px and made it
/// a page you had to reach. Beside a 320 px sleeve it is simply *there*, at
/// every shipped width, which is what the prior-art finding actually asked for.
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
    column![section_rule("Details"), table]
        .spacing(theme::GAP_SM)
        .into()
}

/// The page's identity block: **the album's name, who made it, and what it is
/// made of** — in one falling order, four voices, four sizes, four inks.
///
/// The title is set at the hero size where the column set it at the title size
/// — the top of the whole type scale, against what was one rung down. That is
/// law L6's repair carried across: the album's own name is the loudest type on
/// its own page by a clear step, and it is the second thing declared after the
/// work itself.
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
    // The format goes on the catalogue line rather than a line of its own: at
    // page width there is room for `1992 · 13 tracks · 45:35 · FLAC 16/44.1`
    // in one measure, and it is one fact about the object rather than two.
    if let Some(line) = edition.and_then(vm::EditionVm::encoding_line) {
        meta.push(line);
    }
    column![
        // The title clips at **two lines**. `Wrapping::None` does not stop iced
        // 0.13 laying a long string over several lines, and a box-set title
        // running to four lines pushes everything under it down the page. Two
        // lines is a title; more is a paragraph.
        container(
            text(title)
                .size(theme::SIZE_HERO)
                .line_height(theme::LEADING_HERO)
                .font(theme::SEMIBOLD)
                .color(room.paper)
        )
        .max_height(2.0 * theme::LINE_HERO)
        .clip(true),
        text(artist)
            .size(theme::SIZE_TITLE)
            .line_height(theme::LEADING_TITLE)
            .color(room.paper_dim),
        text(meta.join(" · "))
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            .color(room.paper_faint),
    ]
    .spacing(theme::GAP_XS)
    .into()
}

/// The edition selector: a quiet segmented control, one segment per format
/// the album is owned in, in the library's best-first order.
///
/// Shown only when there is a choice to make — a single-format album carries
/// no control at all, so the ordinary case gains no chrome. The choice changes
/// what the page lists and what Play queues, and nothing else; it never
/// interrupts what is already playing.
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
/// [`theme::TRACK_NO_W`], which is the queue place's arrangement and is what
/// makes the mark arriving as a track starts move no text: the column is the
/// same width whichever it holds.
///
/// `press` is `None` when there is no engine to ask, and the row then renders
/// as the inert text it always was — a disabled control rather than a live one
/// that would do nothing.
///
/// The row also carries the **reserved `+` slot** — a track's own route into
/// a playlist (ADR-0024 §6). The slot is reserved whether or not the control
/// is in it, so no duration slides as the pointer crosses a row; the control
/// itself appears on hover, and **at rest whenever the panel is open or a
/// playlist is armed** — the quiet mark that appears only while the user is
/// collecting is the task's own furniture, not permanent chrome. Hover is not
/// its only route: the open target (layer 2) is the rest-drawn second road
/// the visible-control rule requires of a hover-revealed control.
#[expect(
    clippy::too_many_arguments,
    reason = "a row is one anatomy with one long fact list; a struct per call \
              site would be the same eight names once removed"
)]
fn track_row(
    track: &vm::TrackVm,
    show_artist: bool,
    playing: bool,
    press: Option<Message>,
    album: u64,
    index: usize,
    collecting: Collecting,
    hovered: bool,
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
    let body = button(
        row![
            // The number column and the duration lane are centred on the
            // **title's own line**, not on the row's block, and the row is
            // top-aligned so they stay there. Centred on the block, a
            // soundtrack row that carries a composer under its title dragged
            // its number and its duration halfway down two lines.
            container(marker)
                .width(Length::Fixed(theme::TRACK_NO_W))
                .height(Length::Fixed(theme::CAPTION_LINE_H))
                .align_x(alignment::Horizontal::Right)
                .align_y(alignment::Vertical::Center),
            container(title).width(Length::Fill),
            // The duration lives in a reserved [`theme::DURATION_W`] lane,
            // right-aligned, so a thirteen-track record has a ruled right edge
            // rather than a ragged one.
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
    // **No horizontal inset**: the number column starts on the page's own
    // content lane and the duration lane ends on it, so the block a listener
    // reads down shares its edges with the column that holds it (law L5).
    .padding(theme::pad(theme::GAP_XS, 0.0))
    .style(move |_theme, status| theme::track_row(room, status, playing))
    .on_press_maybe(press);
    if !collecting.available {
        return body.into();
    }
    let offered = collecting.armed || collecting.panel_open || hovered;
    mouse_area(
        row![body, add_slot(album, index, offered, collecting.armed)]
            .spacing(theme::GAP_XS)
            .align_y(iced::Alignment::Center),
    )
    .on_enter(Message::AlbumRowEntered(index))
    .on_exit(Message::AlbumRowLeft(index))
    .into()
}

/// The track's `+` slot: the queue ✕'s exact anatomy — [`theme::STEPPER_HIT`]
/// square, slot reserved whether shown — sending one track toward a playlist.
/// With one armed the press adds outright; otherwise it opens the panel as
/// the picker (ADR-0024 §6 layers 2 and 1 respectively).
fn add_slot(album: u64, index: usize, offered: bool, armed: bool) -> Element<'static, Message> {
    let room = theme::active();
    if !offered {
        return Space::with_width(Length::Fixed(theme::STEPPER_HIT)).into();
    }
    iced::widget::tooltip(
        button(
            container(
                text("+")
                    .size(theme::SIZE_BODY)
                    .line_height(theme::LEADING_BODY)
                    .color(room.paper),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(alignment::Horizontal::Center)
            .align_y(alignment::Vertical::Center),
        )
        .width(Length::Fixed(theme::STEPPER_HIT))
        .height(Length::Fixed(theme::STEPPER_HIT))
        .padding(0)
        .style(move |_theme, status| theme::transport(room, room.wall, status))
        .on_press(Message::AddTrackToPlaylist(album, index)),
        text(if armed {
            "Add to the open playlist"
        } else {
            "Add to playlist"
        })
        .size(theme::SIZE_CAPTION)
        .line_height(theme::LEADING_CAPTION),
        iced::widget::tooltip::Position::Left,
    )
    .gap(theme::GAP_XS)
    .padding(theme::GAP_XS)
    .style(move |_theme| theme::tooltip(room))
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

#[cfg(test)]
mod tests {
    /// The page's two columns add up to the window at every width the two-column
    /// arrangement is used at, and stop growing when the list reaches its
    /// measure.
    ///
    /// This is the arithmetic `views::settings`'s `content_width` needed a
    /// rendered frame to catch (the segmented control ran 998 px wide inside a
    /// 640 px cap), asserted here instead — the widths are the view's own
    /// arithmetic and nothing about them depends on the toolkit.
    #[test]
    fn the_page_fills_the_window_until_its_list_reaches_its_measure() {
        use crate::theme;

        let tracks = |w: f32| {
            (w - 2.0 * theme::HANG - theme::SCROLLBAR_LANE - theme::ALBUM_ASIDE_W - theme::GAP_XL)
                .clamp(0.0, theme::LIST_MEASURE)
        };
        let page = |w: f32| theme::ALBUM_ASIDE_W + theme::GAP_XL + tracks(w);

        // At the shipped window the page hangs from both gutters exactly, the
        // scrollbar's declared lane included.
        let inner = |w: f32| w - 2.0 * theme::HANG - theme::SCROLLBAR_LANE;
        assert!((page(1280.0) - inner(1280.0)).abs() < f32::EPSILON);
        // At 1920 the list has reached its measure, so the page stops growing
        // and centres in what is left rather than setting a track title and a
        // duration 1500 px apart.
        assert!((tracks(1920.0) - theme::LIST_MEASURE).abs() < f32::EPSILON);
        assert!(page(1920.0) < inner(1920.0));
        // And the breakpoint is where the list stops being wider than the
        // sleeve beside it, which is the point at which two columns have
        // stopped being two columns.
        assert!(tracks(theme::ALBUM_BREAKPOINT) <= theme::ALBUM_ASIDE_W);
        assert!(tracks(theme::ALBUM_BREAKPOINT + 4.0 * theme::HANG) > theme::ALBUM_ASIDE_W);
    }
}
