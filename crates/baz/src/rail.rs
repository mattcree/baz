//! The index rail: **a pure projection of the active group key**, and nothing
//! else (ADR-0017 §1.7, `.interface-design/system.md` §7.2).
//!
//! `03-interface-prior-art.md` R8 found that losing jump-to-letter was the
//! single most concrete regression Sonos users named, and a gallery direction
//! makes scrolling *longer* rather than shorter — so a wall of covers that is
//! beautiful at 200 albums is unusable at 5 000 without an index.
//!
//! # It holds no state
//!
//! Everything in this module is a function of the shelves the wall is
//! currently drawing. There is no "current letter" stored anywhere, no cached
//! alphabet, and no per-key configuration: change the group key and the rail
//! is simply built again from the new headers. ADR-0019 §1 is what makes that
//! affordable — a rail is `shelves(key).map(|s| s.header.label())` plus the
//! gaps — and it is why a future CRATES or MOOD key costs this file nothing.
//!
//! # Absent values are drawn, never hidden
//!
//! §7.2 is explicit: *an index that hides its gaps lies about the collection*.
//! A rail showing `A C D` tells you there is no B only if you were counting;
//! a rail showing `A B C D` with B in a quieter ink tells you at a glance. So
//! the rail draws the key's **whole universe between its extremes**, and marks
//! the values nothing landed on.
//!
//! What that universe *is* depends on the key, and the difference is not
//! cosmetic — it is the difference between a value that could have been there
//! and one that could not:
//!
//! | Key | Universe | Why |
//! |---|---|---|
//! | ARTIST | `#` and `A`–`Z`, always | The alphabet exists whether or not the collection uses it. Non-Latin initials ([`Initial::Letter`] is not ASCII-only) join it where they sort. |
//! | YEAR | Every decade from the earliest to the latest present | A run of decades with a hole in it is a fact about the collection. |
//! | GENRE | The **initials** of the genres present, on the `A`–`Z` frame | **There is still no universe of genres** (ADR-0019 §4), and the rail does not draw one — it indexes their *spellings*, which live in an alphabet the reader already knows. The names themselves were the vocabulary at first, and failed as an index; see [`genre`]. |
//! | ADDED / PLAYED | Every [`Recency`] bucket between the newest and the oldest present | The buckets are an ordered, enumerable scale, so a gap in it is real. |
//!
//! The two anonymous ARTIST buckets — `Unknown` and `Various` — are drawn only
//! when they are occupied. They are not letters, and a permanently-drawn
//! `Unknown` on a well-tagged library would be a gap that can never be filled
//! rather than one that can.
//!
//! # Long value sets elide
//!
//! A GENRE rail on a messy library has hundreds of entries and a viewport has
//! room for a few dozen. [`elide`] keeps the **first, the last and a window
//! around where you are**, and marks each omission with a single `·` — the
//! pattern every phone contact list uses, which §7.2 names as the answer when
//! 27 keys do not fit.

use baz_core::history::Recency;
use baz_core::index::{GroupKey, Initial};

use crate::vm::GroupHeaderVm;

/// One value in the rail: what it says, and what it jumps to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RailEntry {
    /// The header's own text — the rail is a projection, so this is
    /// [`GroupHeaderVm::label`] and never a second spelling of it.
    pub label: String,
    /// Which shelf clicking jumps to, or `None` for a value the collection has
    /// nothing under. An absent value is **drawn and inert**: it states a gap,
    /// and a control that did nothing when pressed would be the lie
    /// `docs/REFUSALS.md` guards against from the other direction.
    pub shelf: Option<usize>,
}

impl RailEntry {
    /// Whether the collection has a shelf here.
    #[must_use]
    pub fn present(&self) -> bool {
        self.shelf.is_some()
    }
}

/// What the rail draws in one slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RailSlot {
    /// An entry, by index into the list [`entries`] produced.
    Entry(usize),
    /// Values that did not fit, collapsed. Drawn as a single `·`.
    Gap,
}

/// What an elided run is drawn as: one middle dot, at the absent ink.
///
/// A character rather than an ellipsis, because three dots at 10 px on a
/// 36 px lane read as a word; one dot reads as *there is more here*.
pub const GAP_MARK: &str = "·";

/// The rail for `key`, given the headers of the shelves the wall is drawing,
/// in wall order.
///
/// The returned list is in rail order — which is shelf order, with the key's
/// absent values interleaved where they belong.
#[must_use]
pub fn entries(key: GroupKey, headers: &[GroupHeaderVm]) -> Vec<RailEntry> {
    // A wall with nothing on it indexes nothing. The alphabet is drawn for a
    // library that has *some* records and not all the letters; drawing 27
    // absent letters beside a zero-result search would be an index of nothing.
    if headers.is_empty() {
        return Vec::new();
    }
    match key {
        GroupKey::Artist => artist(headers),
        GroupKey::Year => year(headers),
        GroupKey::Genre => genre(headers),
        GroupKey::Added | GroupKey::Played => recency(headers),
    }
}

/// Every entry the collection actually has, in order — the whole rail for a
/// key whose values have no universe to be absent from.
fn present_only(headers: &[GroupHeaderVm]) -> Vec<RailEntry> {
    headers
        .iter()
        .enumerate()
        .map(|(shelf, header)| RailEntry {
            label: header.label(),
            shelf: Some(shelf),
        })
        .collect()
}

/// ARTIST: the alphabet, always, with the collection's own initials merged in.
///
/// Ordered by [`Initial`]'s own `Ord`, which *is* the wall's shelf order
/// (ADR-0019 §2: "variant order is shelf order"), so the rail cannot disagree
/// with the shelves it indexes.
fn artist(headers: &[GroupHeaderVm]) -> Vec<RailEntry> {
    let mut slots: Vec<(Initial, Option<usize>)> = headers
        .iter()
        .enumerate()
        .filter_map(|(shelf, header)| match header {
            GroupHeaderVm::Initial(initial) => Some((*initial, Some(shelf))),
            _ => None,
        })
        .collect();
    let alphabet = std::iter::once(Initial::Other).chain(('A'..='Z').map(Initial::Letter));
    for value in alphabet {
        if !slots.iter().any(|(present, _)| *present == value) {
            slots.push((value, None));
        }
    }
    slots.sort_by_key(|(initial, _)| *initial);
    slots
        .into_iter()
        .map(|(initial, shelf)| RailEntry {
            label: initial.label(),
            shelf,
        })
        .collect()
}

/// GENRE: **the initials of what the tags spell**, on the alphabet's frame —
/// not the genre names themselves, and not a taxonomy.
///
/// The names were the vocabulary at first, and the owner's finding — *"I
/// dunno if the rail needs to be there for all types of grouping"* — exposed
/// why that failed §7.2's own premise: an index works when the reader can
/// guess the vocabulary and its order without reading it. Nobody can guess
/// which of `Ambient · Jazz · Zeuhl` a particular library holds, and once the
/// list elides, the `·` marks between arbitrary words are not aimable — you
/// cannot throw the pointer at a word you do not know is there. The
/// *spellings*, though, live in an alphabet everyone knows: the genre rail
/// speaks letters, exactly as ARTIST does, and a letter jumps to the first
/// genre spelled with it. Bounded (≤ the alphabet plus the odd digit or
/// non-Latin initial), guessable, and aimable.
///
/// **This invents no genre** — ADR-0019 §4's refusal stands untouched. An
/// absent `B` in the muted ink states "no genre here starts with B", which is
/// a fact about spellings, not a claim that some canonical B-genre exists.
/// Initials outside `A`–`Z` (a `8-Bit`, a CJK tag) get their own entry where
/// the wall sorts them, exactly as ARTIST's non-Latin initials do; and the
/// `No genre` bucket follows the anonymous-bucket rule ARTIST's `Unknown`
/// does — drawn only when occupied, as itself, at the front where the wall
/// shelves it.
fn genre(headers: &[GroupHeaderVm]) -> Vec<RailEntry> {
    let mut entries: Vec<(usize, RailEntry)> = Vec::new();
    // Present initials, in the wall's own order, first shelf of each run —
    // deduplicated rather than run-collapsed, so a fold quirk that separated
    // two same-initial genres could not mint the letter twice.
    let mut letters: Vec<(char, usize)> = Vec::new();
    for (shelf, header) in headers.iter().enumerate() {
        if matches!(header, GroupHeaderVm::Genre(None)) {
            entries.push((
                0,
                RailEntry {
                    label: header.label(),
                    shelf: Some(shelf),
                },
            ));
            continue;
        }
        let Some(initial) = initial_of(&header.label()) else {
            continue;
        };
        if !letters.iter().any(|(seen, _)| *seen == initial) {
            letters.push((initial, shelf));
        }
    }
    for (position, (letter, shelf)) in letters.iter().enumerate() {
        entries.push((
            (position + 1) * 2,
            RailEntry {
                label: letter.to_string(),
                shelf: Some(*shelf),
            },
        ));
    }
    // The alphabet's holes, placed after the last present initial that folds
    // before them — the same fill-the-holes rule `merge_headers` applies to
    // decades, so the rail's order stays the wall's order with gaps drawn in.
    for letter in 'A'..='Z' {
        if letters.iter().any(|(seen, _)| *seen == letter) {
            continue;
        }
        let order = letters
            .iter()
            .rposition(|(seen, _)| folds_before(*seen, letter))
            .map_or(1, |position| (position + 1) * 2 + 1);
        entries.push((
            order,
            RailEntry {
                label: letter.to_string(),
                shelf: None,
            },
        ));
    }
    entries.sort_by_key(|(order, _)| *order);
    entries.into_iter().map(|(_, entry)| entry).collect()
}

/// A label's initial, uppercased — the letter the genre rail files it under.
fn initial_of(label: &str) -> Option<char> {
    label
        .chars()
        .next()
        .and_then(|first| first.to_uppercase().next())
}

/// Whether initial `a` sorts before initial `b` on a case-folded wall —
/// compared through their lowercase forms, which is the fold the shelf order
/// itself uses (ADR-0019 §4).
fn folds_before(a: char, b: char) -> bool {
    a.to_lowercase().lt(b.to_lowercase())
}

/// YEAR: every decade between the earliest and the latest the collection has.
fn year(headers: &[GroupHeaderVm]) -> Vec<RailEntry> {
    let decades: Vec<u32> = headers
        .iter()
        .filter_map(|header| match header {
            GroupHeaderVm::Decade(decade) => *decade,
            _ => None,
        })
        .collect();
    let mut universe = Vec::new();
    if let (Some(&first), Some(&last)) = (decades.iter().min(), decades.iter().max()) {
        universe.extend(
            (first..=last)
                .step_by(10)
                .filter(|decade| !decades.contains(decade))
                .map(|decade| GroupHeaderVm::Decade(Some(decade))),
        );
    }
    merge_headers(headers, universe)
}

/// ADDED / PLAYED: every bucket between the newest and the oldest present.
///
/// The scale is enumerated rather than derived from the shelves, because that
/// is the whole point — a library played this evening and then not for three
/// years should show the three years it skipped.
fn recency(headers: &[GroupHeaderVm]) -> Vec<RailEntry> {
    let present: Vec<Recency> = headers
        .iter()
        .filter_map(|header| match header {
            GroupHeaderVm::Recency(recency) => Some(*recency),
            _ => None,
        })
        .collect();
    // `Never` and `Unrecorded` are the two buckets that are not elapsed time,
    // and both are positive statements rather than gaps: "this was not played"
    // and "baz holds no date for this". They bound nothing, so the scale runs
    // between the extremes of the buckets that *are* elapsed time, and the two
    // ends are drawn only when occupied — the same argument the anonymous
    // ARTIST buckets get in the module docs.
    let elapsed: Vec<Recency> = present
        .iter()
        .copied()
        .filter(|bucket| !matches!(bucket, Recency::Never | Recency::Unrecorded))
        .collect();
    let (Some(&newest), Some(&oldest)) = (elapsed.first(), elapsed.last()) else {
        return present_only(headers);
    };
    let oldest_years = match oldest {
        Recency::YearsAgo(years) => years,
        _ => 0,
    };
    let mut scale = vec![
        Recency::ThisEvening,
        Recency::Today,
        Recency::ThisWeek,
        Recency::ThisMonth,
    ];
    scale.extend((1..=12).map(Recency::MonthsAgo));
    scale.extend((1..=oldest_years).map(Recency::YearsAgo));
    let universe: Vec<GroupHeaderVm> = scale
        .into_iter()
        .filter(|bucket| *bucket >= newest && *bucket <= oldest && !present.contains(bucket))
        .map(GroupHeaderVm::Recency)
        .collect();
    merge_headers(headers, universe)
}

/// Merge a universe of *absent* headers into the present ones.
///
/// Used where the value carries a spelling the wall owns ([`GroupHeaderVm`] is
/// deliberately not `Ord`, because GENRE's order is case-folded while its text
/// is verbatim): each absent value is placed after the last present shelf it
/// sorts behind, so the rail's order is the wall's order with holes filled in
/// rather than a second ordering that has to agree with it.
fn merge_headers(headers: &[GroupHeaderVm], absent: Vec<GroupHeaderVm>) -> Vec<RailEntry> {
    let mut entries: Vec<(usize, RailEntry)> = Vec::new();
    // Present shelves keep the wall's own order; absent values are placed
    // between the two present shelves they fall between, which the universe's
    // construction guarantees exists (it is bounded by the extremes present).
    for (shelf, header) in headers.iter().enumerate() {
        entries.push((
            shelf * 2,
            RailEntry {
                label: header.label(),
                shelf: Some(shelf),
            },
        ));
    }
    for header in absent {
        let after = headers
            .iter()
            .rposition(|present| precedes(present, &header))
            .map_or(0, |index| index * 2 + 1);
        entries.push((
            after,
            RailEntry {
                label: header.label(),
                shelf: None,
            },
        ));
    }
    entries.sort_by_key(|(order, _)| *order);
    entries.into_iter().map(|(_, entry)| entry).collect()
}

/// Whether `a` sorts before `b` on the wall — the *only* comparison this
/// module makes between two headers, and it covers exactly the two keys with
/// an enumerable universe.
fn precedes(a: &GroupHeaderVm, b: &GroupHeaderVm) -> bool {
    match (a, b) {
        (GroupHeaderVm::Decade(left), GroupHeaderVm::Decade(right)) => match (left, right) {
            // `No year` is the front of the shelf, so it precedes every decade
            // and nothing precedes it.
            (None, Some(_)) => true,
            (Some(left), Some(right)) => left < right,
            (_, None) => false,
        },
        (GroupHeaderVm::Recency(left), GroupHeaderVm::Recency(right)) => left < right,
        _ => false,
    }
}

/// Fit a rail of `count` entries into `capacity` slots, keeping the first,
/// the last and a window around `focus` (§7.2, ADR-0017 step 8: *long value
/// sets elide to near-viewport entries plus first and last*).
///
/// `focus` is where the wall is — the shelf at the top of the viewport — so
/// what survives an elision is the part of the index you are standing in.
/// `None` (an empty wall, or a shelf the rail has no entry for) centres the
/// window, which is what a rail nobody has scrolled yet should show.
///
/// Takes the count rather than the entries because the count is all it reads
/// — and because the caller with the *true* capacity is [`crate::spine`], at
/// layout time, inside its real bounds. (The capacity used to be computed in
/// the view from `Shelf::grid_size`, whose height is an estimate between
/// scroll events that ignores the bottom bar — the rail it admitted was five
/// slots too tall, which is the owner's "goes off the edge of the screen".)
///
/// The whole set is returned untouched whenever it fits, which is the
/// ordinary case for ARTIST (27 entries) at any viewport a window can have.
#[must_use]
pub fn elide(count: usize, capacity: usize, focus: Option<usize>) -> Vec<RailSlot> {
    if count == 0 || capacity == 0 {
        return Vec::new();
    }
    if count <= capacity {
        return (0..count).map(RailSlot::Entry).collect();
    }
    // Below five slots there is no room for first + gap + window + gap + last,
    // so the rail degrades to the ends of the index and a mark saying the
    // middle is missing. Fewer than three and it is the ends alone.
    if capacity < 5 {
        let mut slots = vec![RailSlot::Entry(0)];
        if capacity >= 3 {
            slots.push(RailSlot::Gap);
        }
        if capacity >= 2 {
            slots.push(RailSlot::Entry(count - 1));
        }
        return slots;
    }
    let window = capacity - 4;
    let focus = focus.unwrap_or(count / 2).min(count - 1);
    let half = window / 2;
    let start = focus.saturating_sub(half).clamp(1, count - 1 - window);
    let end = start + window - 1;

    let mut slots = vec![RailSlot::Entry(0)];
    if start > 1 {
        slots.push(RailSlot::Gap);
    }
    slots.extend((start..=end).map(RailSlot::Entry));
    if end < count - 2 {
        slots.push(RailSlot::Gap);
    }
    slots.push(RailSlot::Entry(count - 1));
    slots
}

#[cfg(test)]
mod tests {
    use super::*;

    fn initial(letter: char) -> GroupHeaderVm {
        GroupHeaderVm::Initial(Initial::Letter(letter))
    }

    fn labels(entries: &[RailEntry]) -> Vec<&str> {
        entries.iter().map(|entry| entry.label.as_str()).collect()
    }

    fn absent(entries: &[RailEntry]) -> Vec<&str> {
        entries
            .iter()
            .filter(|entry| !entry.present())
            .map(|entry| entry.label.as_str())
            .collect()
    }

    /// **ARTIST draws the whole alphabet**, and the letters the collection has
    /// nothing under are drawn rather than skipped (§7.2).
    #[test]
    fn the_artist_rail_draws_the_letters_the_collection_lacks() {
        let headers = [initial('A'), initial('C'), initial('Z')];
        let rail = entries(GroupKey::Artist, &headers);
        // `#` first, then A–Z: 27 entries whatever the library holds.
        assert_eq!(rail.len(), 27);
        assert_eq!(rail[0].label, "#");
        assert_eq!(labels(&rail)[1..4], ["A", "B", "C"]);
        // Only the three the library has are jumpable; the rest state a gap.
        assert_eq!(
            rail.iter().filter(|entry| entry.present()).count(),
            3,
            "{:?}",
            labels(&rail)
        );
        assert!(absent(&rail).contains(&"B"));
        assert!(absent(&rail).contains(&"#"));
        // And a present entry points at the shelf it came from.
        let a = rail.iter().find(|entry| entry.label == "A").expect("A");
        assert_eq!(a.shelf, Some(0));
        let z = rail.iter().find(|entry| entry.label == "Z").expect("Z");
        assert_eq!(z.shelf, Some(2));
    }

    /// A non-Latin initial gets a rail entry of its own, where it sorts — the
    /// rail must be usable for exactly the library that most needs one
    /// (ADR-0019 §2).
    #[test]
    fn a_non_latin_initial_joins_the_alphabet_where_it_sorts() {
        let headers = [initial('A'), initial('Ø'), initial('曲')];
        let rail = entries(GroupKey::Artist, &headers);
        assert_eq!(rail.len(), 29);
        // Both sort after Z (their code points do), which is where the wall
        // puts them too — the rail and the wall use one ordering.
        assert_eq!(labels(&rail)[26..], ["Z", "Ø", "曲"]);
    }

    /// The two anonymous ARTIST buckets are drawn only when occupied: they are
    /// not letters, and an `Unknown` that can never be filled is not a gap.
    #[test]
    fn the_anonymous_artist_buckets_appear_only_when_they_hold_something() {
        let plain = entries(GroupKey::Artist, &[initial('A')]);
        assert!(!labels(&plain).contains(&"Unknown"));
        assert!(!labels(&plain).contains(&"Various"));

        let messy = entries(
            GroupKey::Artist,
            &[
                GroupHeaderVm::Initial(Initial::Unknown),
                initial('A'),
                GroupHeaderVm::Initial(Initial::Various),
            ],
        );
        assert_eq!(messy.first().map(|e| e.label.as_str()), Some("Unknown"));
        assert_eq!(messy.last().map(|e| e.label.as_str()), Some("Various"));
        assert!(messy[0].present() && messy[messy.len() - 1].present());
    }

    /// **YEAR draws the decades the collection skipped**, and only between the
    /// ones it has: a rail is an index of this library, not of the century.
    #[test]
    fn the_year_rail_fills_the_decades_between_its_extremes() {
        let headers = [
            GroupHeaderVm::Decade(None),
            GroupHeaderVm::Decade(Some(1970)),
            GroupHeaderVm::Decade(Some(2000)),
        ];
        let rail = entries(GroupKey::Year, &headers);
        assert_eq!(
            labels(&rail),
            ["No year", "1970s", "1980s", "1990s", "2000s"]
        );
        assert_eq!(absent(&rail), ["1980s", "1990s"]);
        // Nothing outside the range: no 1960s, no 2010s.
        assert_eq!(rail.len(), 5);
    }

    /// **GENRE speaks initials, not names** — the vocabulary a reader can
    /// guess — and a letter jumps to the *first* genre spelled with it.
    #[test]
    fn the_genre_rail_speaks_initials_and_a_letter_jumps_to_its_first_genre() {
        let headers = [
            GroupHeaderVm::Genre(None),
            GroupHeaderVm::Genre(Some("Ambient".to_owned())),
            GroupHeaderVm::Genre(Some("Post-Rock".to_owned())),
            GroupHeaderVm::Genre(Some("post rock".to_owned())),
            GroupHeaderVm::Genre(Some("Prog".to_owned())),
        ];
        let rail = entries(GroupKey::Genre, &headers);
        // The anonymous bucket first, as itself; then the alphabet — one P,
        // however many genres spell themselves with it.
        assert_eq!(rail.len(), 27, "{:?}", labels(&rail));
        assert_eq!(rail[0].label, "No genre");
        assert_eq!(rail[0].shelf, Some(0));
        assert_eq!(labels(&rail)[1..4], ["A", "B", "C"]);
        let a = rail.iter().find(|entry| entry.label == "A").expect("A");
        assert_eq!(a.shelf, Some(1));
        let p = rail.iter().find(|entry| entry.label == "P").expect("P");
        assert_eq!(p.shelf, Some(2), "P jumps to the first P-genre");
        // The alphabet's holes are drawn, muted — a fact about spellings, not
        // an invented taxonomy (ADR-0019 §4's refusal stands).
        assert!(absent(&rail).contains(&"B"));
        assert_eq!(rail.iter().filter(|entry| entry.present()).count(), 3);
        // No anonymous bucket on a fully-tagged library.
        let tagged = entries(
            GroupKey::Genre,
            &[GroupHeaderVm::Genre(Some("Jazz".to_owned()))],
        );
        assert!(!labels(&tagged).contains(&"No genre"));
    }

    /// A genre whose spelling starts outside `A`–`Z` gets its own entry where
    /// the wall sorts it — digits before the alphabet, CJK after — exactly as
    /// ARTIST's non-Latin initials do.
    #[test]
    fn a_genre_initial_outside_the_alphabet_joins_where_it_sorts() {
        let headers = [
            GroupHeaderVm::Genre(Some("8-Bit".to_owned())),
            GroupHeaderVm::Genre(Some("Jazz".to_owned())),
            GroupHeaderVm::Genre(Some("演歌".to_owned())),
        ];
        let rail = entries(GroupKey::Genre, &headers);
        assert_eq!(rail.len(), 28, "{:?}", labels(&rail));
        assert_eq!(rail[0].label, "8");
        assert_eq!(rail.last().map(|entry| entry.label.as_str()), Some("演"));
        let j = rail.iter().find(|entry| entry.label == "J").expect("J");
        assert_eq!(j.shelf, Some(1));
    }

    /// Sixty arbitrary genres still make an alphabet-sized rail: the whole
    /// point of the initials vocabulary is that the index is bounded by the
    /// spellings, not by the tags.
    #[test]
    fn sixty_genres_make_an_alphabet_not_a_list() {
        let headers: Vec<GroupHeaderVm> = (0..60)
            .map(|n| {
                let letter = char::from(b'a' + u8::try_from(n % 26).expect("a letter"));
                GroupHeaderVm::Genre(Some(format!("{letter}enre {n}")))
            })
            .collect();
        let rail = entries(GroupKey::Genre, &headers);
        assert_eq!(rail.len(), 26);
        assert!(rail.iter().all(RailEntry::present));
    }

    /// ADDED / PLAYED draw the buckets the collection skipped, between the
    /// newest and the oldest it has.
    #[test]
    fn the_recency_rail_fills_the_buckets_between_its_extremes() {
        let headers = [
            GroupHeaderVm::Recency(Recency::Today),
            GroupHeaderVm::Recency(Recency::MonthsAgo(2)),
            GroupHeaderVm::Recency(Recency::Never),
        ];
        let rail = entries(GroupKey::Played, &headers);
        assert_eq!(
            labels(&rail),
            [
                "Today",
                "This week",
                "This month",
                "1 month ago",
                "2 months ago",
                "Never played",
            ]
        );
        assert_eq!(absent(&rail), ["This week", "This month", "1 month ago"]);
        // `This evening` is *before* the newest bucket present, so it is not
        // drawn: the rail is bounded by the collection, not by the calendar.
        assert!(!labels(&rail).contains(&"This evening"));
        // The same headers under ADDED are the same rail: one vocabulary.
        assert_eq!(
            labels(&entries(GroupKey::Added, &headers)),
            labels(&entries(GroupKey::Played, &headers))
        );
    }

    /// A rail with nothing on the wall is empty, for every key — no alphabet
    /// floating beside an empty shelf.
    #[test]
    fn an_empty_wall_has_no_rail() {
        for key in GroupKey::ALL {
            assert!(entries(key, &[]).is_empty(), "{key:?}");
        }
    }

    /// Elision keeps the first, the last and where you are.
    #[test]
    fn elision_keeps_the_ends_and_the_window_you_are_in() {
        // It fits: nothing is elided and no gap is drawn.
        let whole = elide(40, 40, Some(0));
        assert_eq!(whole.len(), 40);
        assert!(!whole.contains(&RailSlot::Gap));

        // It does not fit: first, gap, a window around the focus, gap, last —
        // and never more slots than the viewport has room for.
        let slots = elide(40, 11, Some(20));
        assert_eq!(slots.len(), 11);
        assert_eq!(slots[0], RailSlot::Entry(0));
        assert_eq!(slots[1], RailSlot::Gap);
        assert_eq!(slots[slots.len() - 1], RailSlot::Entry(39));
        assert_eq!(slots[slots.len() - 2], RailSlot::Gap);
        let window: Vec<usize> = slots
            .iter()
            .filter_map(|slot| match slot {
                RailSlot::Entry(index) => Some(*index),
                RailSlot::Gap => None,
            })
            .collect();
        assert!(window.contains(&20), "the focus survived: {window:?}");

        // At the top of the index there is no leading gap to draw…
        let slots = elide(40, 11, Some(0));
        assert_eq!(slots[0], RailSlot::Entry(0));
        assert_ne!(slots[1], RailSlot::Gap);
        // …and at the bottom, no trailing one.
        let slots = elide(40, 11, Some(39));
        assert_ne!(slots[slots.len() - 2], RailSlot::Gap);
        assert_eq!(slots[slots.len() - 1], RailSlot::Entry(39));
    }

    /// Elision is total: every capacity from nothing to everything produces a
    /// list that fits, keeps the ends where it can, and never repeats an
    /// entry.
    #[test]
    fn elision_fits_whatever_room_it_is_given() {
        for capacity in 0..80 {
            for focus in [Some(0), Some(31), Some(63), None] {
                let slots = elide(64, capacity, focus);
                assert!(
                    slots.len() <= capacity.max(0),
                    "capacity {capacity}: {} slots",
                    slots.len()
                );
                let mut seen: Vec<usize> = slots
                    .iter()
                    .filter_map(|slot| match slot {
                        RailSlot::Entry(index) => Some(*index),
                        RailSlot::Gap => None,
                    })
                    .collect();
                let before = seen.len();
                seen.sort_unstable();
                seen.dedup();
                assert_eq!(before, seen.len(), "capacity {capacity}: a repeated entry");
                assert!(
                    seen.iter().all(|index| *index < 64),
                    "capacity {capacity}: an index off the end"
                );
                if capacity >= 2 {
                    assert_eq!(seen.first(), Some(&0));
                    assert_eq!(seen.last(), Some(&63));
                }
            }
        }
        // Nothing to draw, whatever the room.
        assert!(elide(0, 20, None).is_empty());
    }
}
