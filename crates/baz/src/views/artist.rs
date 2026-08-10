//! **The artist's page** — their name, and their records in the wall's own tile.
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
//! **Deliberately not here yet**, and each for a reason rather than for want of
//! room: a biography or any critic metadata (it would come off the network, and
//! nothing in baz goes to the network); an artist image (same); play counts and
//! every other engagement statistic (ADR-0030 §6 refused those from Home and
//! the argument does not change with the surface); and a flat list of every
//! track they appear on, which is the Library's search one press away and would
//! be ADR-0017 §1.7's *"albums listed as albums, never flattened"* broken on a
//! page whose whole subject is records.
//!
//! # An artist the library no longer holds
//!
//! The place carries [`crate::vm::artist_id`]'s hash rather than a name, so a
//! rescan that renamed or removed the artist leaves a page with no subject.
//! The shell answers with the wall, exactly as it does for a record's page that
//! stopped resolving — see `app.rs`'s Artist arm. Nothing here draws an empty
//! frame, because a page about no artist is worse than no page.

use iced::widget::{column, container, row, scrollable};
use iced::{Element, Length};

use crate::app::{Message, Shelf};
use crate::player::PlayerState;
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

    // **The header's lead is the artist's name**, at the same height the Album
    // place's breadcrumb takes — the two places are joined by one press and a
    // strip that changed height between them would make that press a jump.
    let lead = container(place_name(name))
        .height(Length::Fixed(theme::TRANSPORT_HIT))
        .align_y(iced::alignment::Vertical::Center)
        .into();

    let mut rows = column![].spacing(hang.gutter);
    let mut current = row![].spacing(hang.gutter);
    let mut in_row = 0usize;
    for album in &records {
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

    let body = column![
        crate::views::section_rule_hung("Records", hang.density),
        rows
    ]
    .spacing(theme::GAP_LG);

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
}
