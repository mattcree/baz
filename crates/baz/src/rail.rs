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
//! | GENRE | Exactly the genres present | **There is no universe of genres.** Genre is verbatim from the tags (ADR-0019 §4), so an "absent" genre is not a thing that exists; drawing one would mean inventing a taxonomy, which that ADR refuses forever. |
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
        GroupKey::Genre => present_only(headers),
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

/// Fit `entries` into `capacity` slots, keeping the first, the last and a
/// window around `focus` (§7.2, ADR-0017 step 8: *long value sets elide to
/// near-viewport entries plus first and last*).
///
/// `focus` is where the wall is — the shelf at the top of the viewport — so
/// what survives an elision is the part of the index you are standing in.
/// `None` (an empty wall, or a shelf the rail has no entry for) centres the
/// window, which is what a rail nobody has scrolled yet should show.
///
/// The whole set is returned untouched whenever it fits, which is the ordinary
/// case for ARTIST (27 entries) at any viewport a window can have.
#[must_use]
pub fn elide(entries: &[RailEntry], capacity: usize, focus: Option<usize>) -> Vec<RailSlot> {
    let count = entries.len();
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

    /// **GENRE draws exactly what the tags say** — no universe, no gaps, no
    /// taxonomy (ADR-0019 §4).
    #[test]
    fn the_genre_rail_invents_no_genre() {
        let headers = [
            GroupHeaderVm::Genre(None),
            GroupHeaderVm::Genre(Some("Post-Rock".to_owned())),
            GroupHeaderVm::Genre(Some("post rock".to_owned())),
        ];
        let rail = entries(GroupKey::Genre, &headers);
        assert_eq!(labels(&rail), ["No genre", "Post-Rock", "post rock"]);
        assert!(rail.iter().all(RailEntry::present), "genres have no gaps");
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
        let rail: Vec<RailEntry> = (0..40)
            .map(|n| RailEntry {
                label: n.to_string(),
                shelf: Some(n),
            })
            .collect();

        // It fits: nothing is elided and no gap is drawn.
        let whole = elide(&rail, 40, Some(0));
        assert_eq!(whole.len(), 40);
        assert!(!whole.contains(&RailSlot::Gap));

        // It does not fit: first, gap, a window around the focus, gap, last —
        // and never more slots than the viewport has room for.
        let slots = elide(&rail, 11, Some(20));
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
        let slots = elide(&rail, 11, Some(0));
        assert_eq!(slots[0], RailSlot::Entry(0));
        assert_ne!(slots[1], RailSlot::Gap);
        // …and at the bottom, no trailing one.
        let slots = elide(&rail, 11, Some(39));
        assert_ne!(slots[slots.len() - 2], RailSlot::Gap);
        assert_eq!(slots[slots.len() - 1], RailSlot::Entry(39));
    }

    /// Elision is total: every capacity from nothing to everything produces a
    /// list that fits, keeps the ends where it can, and never repeats an
    /// entry.
    #[test]
    fn elision_fits_whatever_room_it_is_given() {
        let rail: Vec<RailEntry> = (0..64)
            .map(|n| RailEntry {
                label: n.to_string(),
                shelf: Some(n),
            })
            .collect();
        for capacity in 0..80 {
            for focus in [Some(0), Some(31), Some(63), None] {
                let slots = elide(&rail, capacity, focus);
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
        assert!(elide(&[], 20, None).is_empty());
    }
}
