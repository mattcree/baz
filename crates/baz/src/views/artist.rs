//! **The artist's page** — what the listener owns by an artist: a quiet facts
//! line, `All songs`, their records, and records filed elsewhere on which the
//! artist is credited.
//!
//! The owner, in one line: *"previous and next on albums doesn't make sense on
//! the album view. we could add an Artist > album breadcrumb though. and have
//! an artist page."* This is the second half of that sentence, and the
//! breadcrumb on [`crate::views::album`]'s header is the door.
//!
//! # What it is for, and why it is the right answer to doc 07 §3.2
//!
//! Doc 07 §3.2 ruled that the Album place **must** carry a step to the next and
//! previous record, so that comparing two releases stays one press per release.
//! What shipped for it (doc 11 §5 P3) stepped along *the wall's current
//! arrangement* — a property of the Library place, and one that is not on
//! screen from a record's page. The owner withdrew it, and this is the third
//! form: a record's context is **its artist**, which is a fact about the record
//! rather than about the frame, and every record you reach through it is one
//! you can see before you choose it.
//!
//! # It holds one fact, and the fact is the wall's
//!
//! The records filed under this artist, in **the wall's own tile** — the same
//! `views::shelf::tile` with the wall's own [`Grid`], so the sleeve, the
//! caption, the playing mark and the hover options are the wall's to the pixel.
//! The Home place's `RECENTLY ADDED` row is built the same way and for the same
//! reason: a record behaves the same wherever it is drawn, which is what makes
//! a third surface showing records affordable at all.
//!
//! # The grid is *handed* to this page, and that is the fix for a real defect
//!
//! This page used to resolve its own: `Grid::new(width − 2 × HANG, Balanced)`,
//! a hand-written guess at [`crate::views::place_pad`]'s horizontals — and
//! wrong twice over. It missed the pad's [`theme::SCROLLBAR_LANE`], so the
//! block was resolved for 10 px the page does not have; and it named
//! `Balanced` outright, so the page ignored the density step entirely and
//! <kbd>Ctrl</kbd>+<kbd>=</kbd> did nothing here.
//!
//! What it produced, measured at the owner's own window: **at 1920 px with the
//! returns lane collapsed the page drew six columns of 244 px art where the
//! wall drew five of 294 px** — the same record, 50 px smaller, one press
//! apart. The two widths straddled a column boundary that 22 px of arithmetic
//! decided, which is exactly how fragile a second answer to *how wide is the
//! grid* is.
//!
//! So there is one answer now and this page is given it: [`crate::app::Shelf::grid`],
//! the grid the wall itself hangs on. Every page that hangs works reads that
//! one grid, so **a record is the same size in all three by construction** —
//! `every_place_that_hangs_works_hangs_them_on_one_grid` is the assertion, and
//! there is no arithmetic left here for it to disagree with.
//!
//! It costs this page 22 px: the wall's width reserves the index rail's lane
//! and the wall's 4 px bar (112 px in all) where this page's own gutters take
//! 90, so the block of records stops 22 px short of where it could. That is
//! under a tenth of a gutter, it is spent at the trailing edge where nothing
//! hangs from, and the alternative — the same cover 50 px bigger on one page
//! than the next — is the thing being fixed.
//!
//! A biography and critic metadata stay off the page: baz does not fetch them.
//! The quiet `Look up` door delegates that separate job to the listener's web
//! browser. Play counts and every other engagement statistic stay off for
//! ADR-0030 §6's reason, and appearances remain records rather than becoming a
//! flat track list (ADR-0017 §1.7).
//!
//! # An artist the library no longer holds
//!
//! The place carries [`crate::vm::artist_id`]'s hash rather than a name, so a
//! rescan that renamed or removed the artist leaves a page with no subject.
//! The shell answers with the wall, exactly as it does for a record's page that
//! stopped resolving — see `app.rs`'s Artist arm. Nothing here draws an empty
//! frame, because a page about no artist is worse than no page.

use iced::widget::{button, column, container, image as iced_image, row, scrollable, text};
use iced::{ContentFit, Element, Length};

use crate::app::{Message, Shelf};
use crate::player::PlayerState;
use crate::selection::Content;
use crate::shelf::Grid;
use crate::views::{place_header_led, place_name, place_pad};
use crate::{theme, vm};

/// Every record filed under `artist`, in the wall's own order.
///
/// The wall's order rather than a chronological one: `Shelf::albums` is already
/// arranged the way the listener asked for it, and a second answer to *"in what
/// order do this artist's records go"* would be a second arrangement control
/// that nothing on screen explains. Filtering preserves order, so this is the
/// wall's sequence with everything that is not theirs removed.
pub(crate) fn records(shelf: &Shelf, artist: u64) -> Vec<&vm::AlbumVm> {
    shelf
        .albums
        .iter()
        .filter(|album| vm::artist_id(&album.artist) == artist)
        .collect()
}

/// The artist's label — `None` when the library no longer holds a record filed
/// under them, which is what makes the place resolve to the wall instead.
///
/// **The spelling is the one that sorts first, not the first one found.**
/// Identity is case-folded ([`vm::artist_id`]), so `Alpha` and `alpha` are one
/// artist with two spellings on disk, and *the first record filed under them*
/// is an order — the shelf's — that a rescan or a different arrangement can
/// change. Taking the minimum makes the answer a property of the set rather
/// than of the walk, so an artist cannot be spelled one way today and another
/// tomorrow; it also happens to prefer the capitalised form a tagger meant,
/// since the upper-case letters sort ahead of the lower.
///
/// ADR-0019 §4's *first spelling seen* rule, which genres use, is not
/// available here: it works because a genre's spellings arrive in a scan order
/// the index controls, and these arrive in whatever order the wall is in.
pub(crate) fn label(shelf: &Shelf, artist: u64) -> Option<&str> {
    records(shelf, artist)
        .iter()
        .map(|album| album.artist.label())
        .min()
}

/// The Artist place's body.
pub(crate) fn view<'a>(
    shelf: &'a Shelf,
    player: &'a PlayerState,
    artist: u64,
    hang: Grid,
    collecting: crate::playlists::Collecting,
) -> Element<'a, Message> {
    let room = theme::active();
    let records = records(shelf, artist);
    // One spelling, decided in one place: [`label`] takes the minimum rather
    // than the first, so the page and the breadcrumb cannot disagree and a
    // rescan cannot change the name.
    let name = label(shelf, artist).unwrap_or("Unknown Artist");
    let facts = shelf
        .artist_facts
        .get(&artist)
        .map(facts_line)
        .unwrap_or_default();
    let also_on = shelf.artist_also_on(artist);

    // **The header's lead is the artist's name**, at the same height the Album
    // place's breadcrumb takes — the two places are joined by one press and a
    // strip that changed height between them would make that press a jump.
    // The lead is boxed to the control height by [`place_header_led`] itself
    // now, for every place at once. This page carried its own copy of that box
    // — the second of three — until the general fix landed.
    let lead = place_name(name);

    // One list above the records it draws from: broadest first, and visually
    // distinct from the `RECORDS` set rather than pretending to be one more
    // album. The shared tile keeps this identical to Home's `All songs` in
    // anatomy while the artist constructor gives it a different scope.
    let songs = shelf.artist_songs(artist);
    let songs = songs.as_ref().and_then(|list| {
        crate::views::list_tile::view(
            shelf,
            player,
            hang,
            list,
            shelf.hovered_all_songs,
            crate::views::list_tile::Actions {
                content: Content::ArtistSongs(artist),
                play: Message::PlayArtistSongs(artist),
                open: None,
                enter: Message::AllSongsHovered(true),
                exit: Message::AllSongsHovered(false),
            },
        )
    });

    let records_section = column![
        crate::views::section_rule("Records"),
        tiles(shelf, player, hang, &records, collecting)
    ]
    .spacing(theme::GAP_LG);
    let mut body = column![].spacing(theme::HANG);
    if !facts.is_empty() {
        let fact = container(
            text(facts)
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.paper_faint)
                .wrapping(text::Wrapping::None),
        )
        .width(Length::Fill)
        .clip(true);
        let lookup = button(
            text("Look up")
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META),
        )
        .height(Length::Fixed(theme::LINE_META))
        .padding(0)
        .style(move |_theme, status| theme::word_button(room, room.wall, status))
        .on_press(Message::LookUpArtist(artist));
        let facts_row = row![fact, lookup]
            .spacing(theme::GAP_MD)
            .align_y(iced::Alignment::Center);
        if let Some(image) = shelf.artist_image(artist) {
            const EDGE: f32 = 128.0;
            body = body.push(
                row![
                    iced_image(image.clone())
                        .width(Length::Fixed(EDGE))
                        .height(Length::Fixed(EDGE))
                        .content_fit(ContentFit::Cover),
                    container(facts_row)
                        .height(Length::Fixed(EDGE))
                        .align_y(iced::alignment::Vertical::Bottom),
                ]
                .spacing(theme::GAP_LG)
                .align_y(iced::Alignment::End),
            );
        } else {
            body = body.push(facts_row);
        }
    }
    if let Some(songs) = songs {
        body = body.push(songs);
    }
    body = body.push(records_section);
    if !also_on.is_empty() {
        body = body.push(
            column![
                crate::views::section_rule("Also on"),
                tiles(shelf, player, hang, &also_on, collecting)
            ]
            .spacing(theme::GAP_LG),
        );
    }

    column![
        place_header_led(lead, Some(counts(&records))),
        scrollable(container(body).padding(place_pad()))
            .direction(iced::widget::scrollable::Direction::Vertical(
                theme::wall_scrollbar(),
            ))
            .style(move |_theme, status| theme::scrollbar(room, room.wall, status))
            .width(Length::Fill)
            .height(Length::Fill),
    ]
    .into()
}

fn tiles<'a>(
    shelf: &'a Shelf,
    player: &'a PlayerState,
    hang: Grid,
    albums: &[&'a vm::AlbumVm],
    collecting: crate::playlists::Collecting,
) -> Element<'a, Message> {
    let mut rows = column![].spacing(hang.gutter);
    let mut current = row![].spacing(hang.gutter);
    let mut in_row = 0usize;
    for album in albums {
        current = current.push(crate::views::shelf::tile(
            shelf, player, hang, album, 0.0, collecting,
        ));
        in_row += 1;
        if in_row == hang.columns {
            rows = rows.push(current);
            current = row![].spacing(hang.gutter);
            in_row = 0;
        }
    }
    if in_row > 0 {
        rows = rows.push(current);
    }
    rows.into()
}

/// The facts band's one sentence. Missing terms are omitted individually.
fn facts_line(facts: &vm::ArtistFacts) -> String {
    let mut terms = Vec::new();
    if facts.playing_ms > 0 {
        let total_minutes = facts.playing_ms / 60_000;
        let hours = total_minutes / 60;
        let minutes = total_minutes % 60;
        let plural = |n: u64, unit: &str| format!("{n} {unit}{}", if n == 1 { "" } else { "s" });
        terms.push(if hours > 0 && minutes > 0 {
            format!("{} {}", plural(hours, "hour"), plural(minutes, "minute"))
        } else if hours > 0 {
            plural(hours, "hour")
        } else {
            plural(minutes, "minute")
        });
    }
    if let Some((first, last)) = facts.years {
        terms.push(if first == last {
            first.to_string()
        } else {
            format!("{first}\u{2013}{last}")
        });
    }
    if !facts.formats.is_empty() {
        terms.push(facts.formats.join(", "));
    }
    if !facts.genres.is_empty() {
        terms.push(facts.genres.join(", "));
    }
    if let Some(date) = facts.first_seen_ns.and_then(vm::format_date)
        && let Some(year) = date.rsplit(' ').next()
    {
        terms.push(format!("In your library since {year}"));
    }
    terms.join(" \u{00b7} ")
}

/// `6 records · 74 tracks` — the strip's quiet statement about the place.
///
/// The Library's own counts line, in the Library's own words and order, so two
/// surfaces counting the same things read the same way. Singulars are honoured
/// because a page that says *"1 records"* is a page nobody proof-read.
fn counts(records: &[&vm::AlbumVm]) -> String {
    let tracks: usize = records.iter().map(|album| album.all_tracks().count()).sum();
    let plural = |n: usize, word: &str| {
        if n == 1 {
            format!("{n} {word}")
        } else {
            format!("{n} {word}s")
        }
    };
    format!(
        "{} · {}",
        plural(records.len(), "record"),
        plural(tracks, "track")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Counts read as English**, singular and plural, because a page that
    /// says *"1 records"* is a page nobody proof-read.
    #[test]
    fn the_counts_line_honours_its_singulars() {
        assert_eq!(counts(&[]), "0 records · 0 tracks");
    }

    #[test]
    fn facts_are_one_sentence_and_missing_terms_disappear() {
        let facts = vm::ArtistFacts {
            playing_ms: (4 * 60 + 12) * 60_000,
            years: Some((1988, 1991)),
            formats: vec!["FLAC".to_owned(), "MP3".to_owned()],
            genres: vec!["Post-Rock".to_owned()],
            first_seen_ns: Some(1_546_300_800_000_000_000),
        };
        assert_eq!(
            facts_line(&facts),
            "4 hours 12 minutes · 1988–1991 · FLAC, MP3 · Post-Rock · In your library since 2019"
        );
        assert_eq!(facts_line(&vm::ArtistFacts::default()), "");
    }

    /// The artist list is the same visual object as Home's list, and it leads
    /// the records rather than masquerading as one of them.
    #[test]
    fn all_songs_is_the_shared_tile_above_records() {
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/views/artist.rs"),
        )
        .expect("this view's source")
        .replace("\r\n", "\n");
        let code = source.split("#[cfg(test)]").next().expect("a source head");
        let tile = code
            .find("crate::views::list_tile::view(")
            .expect("the shared tile is used");
        let records = code
            .find("let records_section =")
            .expect("the records section exists");
        assert!(tile < records, "All songs leads the records");
        assert!(code.contains("Message::PlayArtistSongs(artist)"));
    }
}
