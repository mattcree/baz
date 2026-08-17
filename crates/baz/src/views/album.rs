//! **The record's page**: one album, at the width of the window.
//!
//! # The composition is [`views::page`](super::page)'s
//!
//! Since *one page, two subjects* (2026-08-10) this module lays out nothing.
//! The gutter, the breakpoint, the 320 px aside, the identity block, the
//! `TRACKS` rule and the one scroll are the composition a record's page and a
//! playlist's page both wear; what is here is everything that is *about a
//! record* — the breadcrumb, the cover, `Play album`, the edition selector,
//! `DETAILS`, the serif title over the artist over the catalogue line, and the
//! track rows. The sections below are the arguments for those, and they are
//! unchanged: what moved is where the arrangement is written down, which is
//! once.
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
//! | the title | `SIZE_TITLE` 19, fifth of eight by ink | `SIZE_HERO` 28 in `theme::WORK_TITLE`, second, and first among type |
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
//! and this module makes none of it). One click selects the row; double click
//! activates it through that path.
//!
//! Nothing here marks a row optimistically. The dot follows `TrackStarted`
//! through [`crate::player`] like every other reading in the interface.

use std::time::Duration;

use iced::widget::{button, column, container, image as iced_image, mouse_area, row, text};
use iced::{Element, Length, alignment};

use crate::app::{Message, Shelf};
use crate::player::PlayerState;
use crate::playlists::Collecting;
use crate::selection::Content;
use crate::views::page::{self, Identity, Page};
use crate::views::{gradient_block, place_name, section_rule};
use crate::{theme, vm};

/// The record's page: [`views::page`](crate::views::page)'s composition, with a
/// record in it.
///
/// The arrangement — the gutter, the breakpoint, the 320 px aside, the identity
/// block, the `TRACKS` rule, the one scroll — is the shared one, and what this
/// module supplies is everything the composition asks for that is *about a
/// record*: the breadcrumb, the cover, `Play album`, the edition selector, the
/// condition report, the serif title over the artist over the catalogue line,
/// and the track rows.
pub(crate) fn view<'a>(
    shelf: &'a Shelf,
    album: &'a vm::AlbumVm,
    player: &'a PlayerState,
    window_width: f32,
    lamp: f32,
    collecting: Collecting,
    hovered_row: Option<usize>,
) -> Element<'a, Message> {
    let chosen = shelf.edition_choice.get(&album.id).copied();
    let edition = vm::selected_edition(album, chosen);
    page::view(
        Page {
            lead: breadcrumb(album),
            sleeve: sleeve(shelf, album, player, lamp),
            commitment: Some(page::commitment(
                "Play album",
                player.engine_ready(),
                Message::PlayAlbum(album.id),
            )),
            // The transfer gesture (09 §8.1): the record, whole, toward a
            // destination of the user's choosing. It reads the selected album —
            // L8.1 puts it with the album — and stands under the page's one
            // commitment, quiet, no accent: collecting is not playback truth.
            // The press opens the panel as the picker; the ellipsis honestly
            // promises the second press.
            acts: if collecting.available {
                vec![page::act(
                    "Add to playlist…",
                    true,
                    Message::AddAlbumToPlaylist(album.id),
                )]
            } else {
                Vec::new()
            },
            // Only a genuinely multi-format album gets a control; a
            // single-format album must look exactly as it always did.
            aside_held: if album.editions.len() > 1 {
                vec![edition_selector(album, edition)]
            } else {
                Vec::new()
            },
            aside_tail: aside_tail(album, edition),
            identity: identity(album, edition),
            rows: track_rows(shelf, album, edition, player, collecting, hovered_row),
            side_by_side: page::is_two_column(window_width),
            row_spacing: theme::GAP_XS,
            on_scroll: None,
            // A record with no readable edition ruled off its track list in
            // silence before the composition was shared; the slot is not
            // optional now.
            empty: "No tracks here. The scan read no files for this record.",
        },
        window_width,
    )
}

/// **`Artist › Album`** — the record's own context in the header, and the
/// artist half is a door.
///
/// The owner's, replacing the `‹ Prev` / `Next ›` pair that stood here:
/// *"previous and next on albums doesn't make sense on the album view. we
/// could add an Artist > album breadcrumb though."* The pair stepped along the
/// *wall's* current arrangement — a property of the Library place, not on
/// screen from here — so it offered a door whose destination the listener
/// could not know before pressing it. A breadcrumb names where this record
/// actually sits, and it names it with a fact about the record rather than
/// about the frame.
///
/// **The lead is the place's name now**, and that is the point rather than a
/// side effect: the strip used to read `Album`, which told you the *kind* of
/// page you were on when the page is entirely made of the answer. Every other
/// place still leads with its name because for them the name is the only
/// honest lead — `Queue`, `Settings` and `Now playing` have no subject that
/// changes.
///
/// The separator is `›` in the readout ink and it is **not** pressable: a
/// breadcrumb's chevron is punctuation, and a chevron that acts is a control
/// disguised as a comma. The album half is not pressable either — you are
/// already there, and doc 07's rule that pressing the place you are on must
/// leave you there is the returns lane's own (`Place::go`).
fn breadcrumb(album: &vm::AlbumVm) -> Element<'static, Message> {
    let room = theme::active();
    let artist = vm::artist_id(&album.artist);
    let door = button(
        container(place_name(album.artist.label()))
            .height(Length::Fill)
            .align_y(alignment::Vertical::Center),
    )
    .height(Length::Fixed(theme::TRANSPORT_HIT))
    // **No horizontal padding**, and that is law L1 rather than taste: the
    // strip's lead starts at `HANG` in every place, and a door that inset its
    // own text by `GAP_SM` would put the artist's name eight pixels right of
    // where the Artist place puts it — a visible slide across the one press
    // that joins the two. The hover ground is the word's own box, which is
    // what a breadcrumb wants anyway.
    .padding(0)
    .style(move |_theme, status| theme::word_button(room, room.wall, status))
    .on_press(Message::OpenArtist(artist));
    row![
        door,
        text("\u{203a}")
            .size(theme::SIZE_EMPHASIS)
            .line_height(theme::LEADING_EMPHASIS)
            .color(room.paper_faint),
        place_name(
            album
                .title
                .clone()
                .unwrap_or_else(|| "Unknown Album".to_owned())
                .as_str()
        ),
    ]
    .spacing(theme::GAP_SM)
    .align_y(iced::Alignment::Center)
    .height(Length::Fixed(theme::TRANSPORT_HIT))
    .into()
}

/// **The work**, at [`theme::ALBUM_SLEEVE`] — the one detail-sized image of
/// the record on screen, warmed while it is the record that is sounding.
fn sleeve<'a>(
    shelf: &'a Shelf,
    album: &'a vm::AlbumVm,
    player: &'a PlayerState,
    lamp: f32,
) -> Element<'a, Message> {
    let room = theme::active();
    let playing = player.playing_album() == Some(album.id);
    // The same 200 ms warm the wall gives the sounding record, on the same
    // tween — the page and the tile are two views of one lamp (ADR-0020 §2.5).
    let warmth = if playing { lamp } else { 0.0 };
    let edge = theme::ALBUM_SLEEVE;
    let handle = shelf
        .hero(album.id)
        .map(|hero| &hero.handle)
        .or_else(|| shelf.thumb(album.id));
    let art: Element<'_, Message> = match handle {
        Some(handle) => iced_image(handle.clone())
            .width(Length::Fixed(edge))
            .height(Length::Fixed(edge))
            .into(),
        None => gradient_block(album.id, edge, 1.0),
    };
    container(art)
        .width(Length::Fixed(edge))
        .height(Length::Fixed(edge))
        .style(move |_theme| theme::sleeve(room, warmth))
        .into()
}

/// What a *record's* aside carries below its acts: the condition report.
///
/// The edition selector used to lead this list and is now held above the
/// scroller with the commitment ([`page::Page::aside_held`]) — it chooses what
/// the page is showing, where `Details` describes it.
///
/// This is the slot a playlist fills with its rename field, and the difference
/// is not drift: a record is a found thing whose facts were read off its files,
/// and a playlist is a made one whose name you can retype.
fn aside_tail<'a>(
    album: &'a vm::AlbumVm,
    edition: Option<&'a vm::EditionVm>,
) -> Vec<Element<'a, Message>> {
    let mut tail: Vec<Element<'a, Message>> = Vec::new();
    if let Some(block) = details(album, edition) {
        tail.push(block);
    }
    tail
}

/// **Every track on the record**, in the composition's row slot.
fn track_rows<'a>(
    shelf: &'a Shelf,
    album: &'a vm::AlbumVm,
    edition: Option<&'a vm::EditionVm>,
    player: &'a PlayerState,
    collecting: Collecting,
    hovered_row: Option<usize>,
) -> Vec<Element<'a, Message>> {
    // Where the music is in *this* list — `None` unless what is listed is
    // exactly the queue that is playing. An album page switched to a different
    // edition of the record that is sounding marks nothing rather than marking
    // a file the engine is not reading.
    let playing_row = edition.and_then(|edition| player.playing_row_in(&edition.tracks));
    // A row is only a control when there is an engine to send its command to,
    // exactly as `Play album` is.
    let interactive = player.engine_ready();
    let per_track_artists = album.track_artists_vary;
    let mut rows: Vec<Element<'a, Message>> = Vec::new();
    if let Some(edition) = edition {
        let multi_disc = vm::discs(edition).is_some_and(|discs| discs > 1);
        let mut current: Option<u32> = None;
        for (index, track) in edition.tracks.iter().enumerate() {
            if multi_disc && track.disc.is_some() && track.disc != current {
                current = track.disc;
                if let Some(disc) = current {
                    rows.push(disc_header(disc, rows.is_empty()));
                }
            }
            rows.push(track_row(
                track,
                per_track_artists,
                playing_row == Some(index),
                interactive.then_some(Message::ContentPressed(Content::AlbumTrack {
                    album: album.id,
                    row: index,
                })),
                album.id,
                index,
                shelf.selection.is(Content::AlbumTrack {
                    album: album.id,
                    row: index,
                }),
                collecting,
                hovered_row == Some(index),
                crate::app::is_favourite(shelf, &track.path),
                shelf.offline(&track.path),
            ));
        }
    }
    rows
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
///
/// `first` takes the air off the head that opens the list, which is the
/// playlist page's own rule for its record heads (`views::playlist`'s
/// `record_head`): a break needs air *above* it because it is a break, and
/// `DISC 1` sitting directly under the `TRACKS` rule is not breaking anything.
/// The two lists had two answers to one question, which is the drift this
/// change is for.
fn disc_header(disc: u32, first: bool) -> Element<'static, Message> {
    let room = theme::active();
    let air = if first { 0.0 } else { theme::GAP_MD };
    container(
        text(format!("DISC {disc}"))
            .size(theme::SIZE_CAPTION)
            .line_height(theme::LEADING_CAPTION)
            .font(theme::MEDIUM)
            .color(room.heading())
            .wrapping(text::Wrapping::None),
    )
    .padding(theme::pad(air, 0.0))
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
fn details<'a>(
    album: &'a vm::AlbumVm,
    edition: Option<&'a vm::EditionVm>,
) -> Option<Element<'a, Message>> {
    let room = theme::active();
    let rows = vm::details(album, edition);
    if rows.is_empty() {
        // A zero-height `Space` still takes the aside's `GAP_MD` above it; an
        // absent block takes nothing, which is what "no rows the scan read"
        // should cost.
        return None;
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
    Some(
        column![section_rule("Details"), table]
            .spacing(theme::GAP_SM)
            .into(),
    )
}

/// What the shared identity block ([`Identity`]) says about a **record**: the
/// album's name, who made it, and what it is made of — in one falling order,
/// three voices, three sizes, three inks.
///
/// The title is set at the hero size where the column set it at the title size
/// — the top of the whole type scale, against what was one rung down. That is
/// law L6's repair carried across: the album's own name is the loudest type on
/// its own page by a clear step, and it is the second thing declared after the
/// work itself.
///
/// The counts describe `edition`, not the album: with two rips on disk, "24
/// tracks" would be a number nothing on screen adds up to.
///
/// # The title is set in [`theme::WORK_TITLE`], and the playlist page's is not
///
/// The second consumer of the serif italic, and the last one this change makes
/// (ADR-0024 §A4.4; design 14 §5.2). The museum placard's convention is that
/// **the work's title is italic and every fact around it is not** — and this
/// block is that placard exactly: the title, then who made it, then when and
/// in what. The artist and the catalogue line below stay in the sans, which is
/// what makes the italic mean *this string is the name of the thing* rather
/// than *this string is important*.
///
/// The asymmetry with `views::playlist`'s hero — the same size, the same ink,
/// the same slot, in the sans — **is the design and must not be flattened into
/// consistency**. A record's title is a work's, published by someone else; a
/// playlist's name is a label the owner typed, and every other string a person
/// typed in this product (the query, the rename field, the folder path) is
/// already sans. Doc 14 §2's last row, in the type itself: the two page heroes
/// are different *kinds of string* at the same size, for no pixels.
///
/// **The line, and why it can be held.** The serif sets an album's title where
/// the album is the subject being labelled — here, and on Home's `CONTINUE`
/// placard. Not a track's title, not an artist's, not a playlist's, and not an
/// album's title where it appears as a *fact about something else*
/// (`views::now_playing`'s `Ochre` under the sounding track's name). Two call
/// sites, enumerated by `theme::the_serif_is_the_work_titles_and_nothing_else`,
/// which fails the build on a third.
fn identity<'a>(album: &'a vm::AlbumVm, edition: Option<&'a vm::EditionVm>) -> Identity<'a> {
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
    Identity {
        name: title.to_owned(),
        // Serif italic: a *work's* title, against the sans name on the playlist
        // page's hero. See this function's docs.
        face: theme::WORK_TITLE,
        edit: None,
        byline: artist.to_owned(),
        facts: meta.join(" · "),
        // Nothing stands beside a record's facts. A playlist's `Undo` is in
        // that slot because a playlist is a thing you edit; a record is not.
        beside_facts: None,
    }
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
/// the picker (09 §8.1). The slot is reserved whether or not the control is
/// in it, so no duration slides as the pointer crosses a row; the control
/// itself appears on hover, and **at rest whenever the panel is open** — the
/// quiet mark that appears only while the user is collecting is the task's
/// own furniture, not permanent chrome. Hover is not its only route: the
/// page's `Add to playlist…`, always visible, reaches the same picker, which
/// is the second road the visible-control rule requires of a hover-revealed
/// control.
#[expect(
    clippy::too_many_arguments,
    clippy::fn_params_excessive_bools,
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
    selected: bool,
    collecting: Collecting,
    hovered: bool,
    favourite: bool,
    offline: bool,
) -> Element<'_, Message> {
    let room = theme::active();
    let duration = track.duration.map(vm::format_duration).unwrap_or_default();
    let marker: Element<'_, Message> = if playing {
        page::lamp_dot()
    } else {
        text(track.number.map(|n| n.to_string()).unwrap_or_default())
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            .color(room.paper_faint)
            .into()
    };
    let body = page::track_row(page::TrackRow {
        marker,
        artwork: None,
        title: track.title.as_str().into(),
        // A published record's row has no dimmed state — nothing on it can be
        // missing or already played — so the ink is the one ink, stated. It
        // used to be left unset and inherited from `theme::track_row`'s
        // `text_color`, which is this exact colour.
        ink: room.paper,
        under: track
            .artist
            .as_deref()
            .filter(|_| show_artist)
            .map(|artist| (artist.into(), room.paper_dim, None)),
        context: None,
        duration: duration.into(),
        playing,
        press,
        offline,
    });
    // The row's right press opens its mirror menu (doc 09 §5.2): the same
    // verbs the row's own controls speak, at the pointer.
    let target = crate::menu::Target::Track { album, row: index };
    let offered = collecting.panel_open || hovered;
    let mut slots = row![body, page::favourite_slot(&track.path, favourite),]
        .spacing(theme::GAP_XS)
        .align_y(iced::Alignment::Center);
    if collecting.available {
        slots = slots.push(page::transfer_slot(
            offered,
            Message::AddTrackToPlaylist(album, index),
        ));
    }
    crate::menu::selection_area(
        mouse_area(page::row_card(hovered, playing, selected, slots))
            .on_enter(Message::AlbumRowEntered(index))
            .on_exit(Message::AlbumRowLeft(index)),
        target,
    )
}
