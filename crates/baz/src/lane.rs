//! **The returns lane** — ADR-0006 layer 1, pure and iced-free: what is in the
//! resident left surface, and in what order.
//!
//! ADR-0030 §1 states the subject in one sentence, and this module is that
//! sentence made total:
//!
//! > **The lane's subject is *things you have touched*: records you have
//! > played, and lists you have made or edited. Its order is when you last
//! > touched them. Nothing else is admitted, ever.**
//!
//! # Why the ordering is a function and not a score
//!
//! The five findings that killed baz's last resident column (ADR-0024 §5) are
//! read here as engineering lessons, and the one this module answers is
//! *arbitration*: a surface with two orderings has to decide between them
//! every frame, and the decision is invisible. So there is one key —
//! `(last touched, name)` — it is total, and two launches over the same data
//! draw the same lane. No score, no decay, no weighting, no blend, no pinning.
//!
//! # What the shell brings and what this decides
//!
//! The shell knows how to read a play ledger and a folder of `.m3u8` files;
//! this module knows nothing about either. It is handed [`Touched`] values —
//! an identity, a name, a second line, and the moment — and returns the lane.
//! That is what makes the ordering testable without a window, a ledger or a
//! disk, and it is why the property that matters (*the same data draws the
//! same lane*) is asserted rather than asserted about.
//!
//! # The head is not in here
//!
//! The three fixed destinations — `Home`, `Library`, `Now playing` — are the
//! owner's decision (ADR-0030's amendment) and they are [`crate::place`]
//! members, not lane rows. They have no order to compute and no membership to
//! decide: they are always all three, always in that order. See
//! [`Destination`].

use std::collections::HashMap;

/// **How many played records the lane holds**: 24 (ADR-0030 §1).
///
/// What falls off the end is the twenty-fifth-most-recent record, and nothing
/// is lost by it — every record is on the wall, one `Esc` away. The number is
/// a *bound on the surface*, not a window on the data: there is no "show more".
pub(crate) const RECENT_ALBUMS: usize = 24;

/// One of the lane's three fixed destinations.
///
/// The owner's decision, verbatim: *"home will appear at the top of the left
/// hand sidebar always either way and it will contain the top level concerns.
/// think spotify"*, and *"as an extension we will want a Now playing page at
/// the top with the Home and Library"*. ADR-0030 §1 had refused destination
/// rows in the lane on the grounds that a second subject is what killed the
/// last one; the owner overruled it, and `docs/REFUSALS.md`'s preamble says
/// that settles it. What survives of the argument is the *shape* of the
/// concession: the head is a closed set of three, above a hairline, and the
/// list below it still has exactly one subject.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Destination {
    /// The home page: the interrupted run, and what is new.
    Home,
    /// The collection — the wall, its search and its arrangement.
    Library,
    /// The record that is sounding, at the size it deserves.
    NowPlaying,
}

impl Destination {
    /// The head, in the owner's order, and there is no other.
    pub(crate) const ALL: [Self; 3] = [Self::Home, Self::Library, Self::NowPlaying];

    /// The word on the row.
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Home => "Home",
            Self::Library => "Library",
            Self::NowPlaying => "Now playing",
        }
    }
}

/// What a lane row *is* — and the only thing that distinguishes the two kinds,
/// because nothing on screen does (ADR-0030 §2: the sleeve already says it).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum Subject {
    /// A record, by [`crate::vm::AlbumVm`] id.
    Record(u64),
    /// A list, by [`crate::playlists::playlist_id`] of its name.
    Playlist(u64),
}

/// One thing the listener has touched, as the shell reports it.
///
/// `at` is seconds since the Unix epoch — the ledger's own unit
/// (`baz_core::history::TrackHistory::last_played_unix_s`) and a playlist
/// file's mtime reduced to it. `None` means *touched, moment unknown*: a
/// playlist that exists on a filesystem with no usable mtime is still a
/// playlist, and it sorts last among the touched rather than vanishing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Touched {
    /// Which thing.
    pub(crate) subject: Subject,
    /// Its name — the record's title, the playlist's file stem.
    pub(crate) name: String,
    /// The quiet line under the name: a record's album artist, a playlist's
    /// counts.
    pub(crate) under: String,
    /// When it was last touched, in seconds since the Unix epoch.
    pub(crate) at: Option<u64>,
}

/// The lane, resolved: the rows in the order they are drawn.
///
/// **Membership** (ADR-0030 §1): every playlist, always — which is what lets
/// the panel stop being the index without any list becoming unreachable — and
/// the last [`RECENT_ALBUMS`] records played. **Order**: last touched, newest
/// first; ties break by name ascending, then by subject, so the key is total
/// and the lane is a function of the data rather than of the iteration order
/// of whatever collection the shell happened to build it from.
///
/// The playlists are *not* trimmed and the records are, which is the one
/// asymmetry in here and it is deliberate: a list you made is a thing you own
/// and the lane is its index, while a record you played is a thing you can
/// always find on the wall.
pub(crate) fn resolve(playlists: Vec<Touched>, records: Vec<Touched>) -> Vec<Touched> {
    let mut records = records;
    sort(&mut records);
    records.truncate(RECENT_ALBUMS);
    let mut rows = playlists;
    rows.extend(records);
    sort(&mut rows);
    rows
}

/// The lane's one ordering: **last touched first, ties by name ascending**.
///
/// A row whose moment is unknown sorts after every row whose moment is known —
/// `None` is *"touched, when is not recorded"*, which is older than any
/// recorded moment for the purpose of a returns list.
fn sort(rows: &mut [Touched]) {
    rows.sort_by(|a, b| {
        b.at.cmp(&a.at)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.subject.cmp(&b.subject))
    });
}

/// Fold a ledger's per-path readings onto the records that hold those paths.
///
/// The shell owns both halves — the index knows which paths a record has, the
/// ledger knows when a path was played — and this is the join, kept here so
/// the *pure* half of "which records are recent" is testable without either.
pub(crate) fn by_record<'a, P: 'a + ?Sized>(
    tracks: impl IntoIterator<Item = (u64, &'a P)>,
    played: impl Fn(&'a P) -> Option<u64>,
) -> HashMap<u64, u64> {
    let mut newest: HashMap<u64, u64> = HashMap::new();
    for (album, path) in tracks {
        if let Some(at) = played(path) {
            newest
                .entry(album)
                .and_modify(|had| *had = (*had).max(at))
                .or_insert(at);
        }
    }
    newest
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touched(subject: Subject, name: &str, at: Option<u64>) -> Touched {
        Touched {
            subject,
            name: name.to_owned(),
            under: String::new(),
            at,
        }
    }

    fn names(rows: &[Touched]) -> Vec<&str> {
        rows.iter().map(|row| row.name.as_str()).collect()
    }

    /// **Newest first, and the two kinds mix freely** — the whole of the
    /// order, and the whole of what makes a mixed list read as one list.
    #[test]
    fn the_lane_is_last_touched_first_and_mixes_the_two_kinds() {
        let rows = resolve(
            vec![
                touched(Subject::Playlist(1), "Road Trip", Some(300)),
                touched(Subject::Playlist(2), "Sunday", Some(100)),
            ],
            vec![
                touched(Subject::Record(10), "Ochre", Some(400)),
                touched(Subject::Record(11), "Violet Ledger", Some(200)),
            ],
        );
        assert_eq!(
            names(&rows),
            ["Ochre", "Road Trip", "Violet Ledger", "Sunday"]
        );
    }

    /// **Two launches over the same data draw the same lane.** The ordering
    /// is total, so the shell may build its input in any order it likes — and
    /// it does: the playlists come off a directory listing and the records off
    /// a hash map, neither of which promises anything.
    #[test]
    fn the_same_data_draws_the_same_lane_whatever_order_it_arrives_in() {
        let build = |seed: usize| {
            let mut playlists = vec![
                touched(Subject::Playlist(1), "Alpha", Some(500)),
                touched(Subject::Playlist(2), "Beta", Some(500)),
                touched(Subject::Playlist(3), "Gamma", None),
            ];
            let mut records = vec![
                touched(Subject::Record(10), "Beta", Some(500)),
                touched(Subject::Record(11), "Delta", Some(700)),
                touched(Subject::Record(12), "Epsilon", None),
            ];
            let lists = playlists.len();
            let records_n = records.len();
            playlists.rotate_left(seed % lists);
            records.rotate_left(seed % records_n);
            resolve(playlists, records)
        };
        let first = build(0);
        for seed in 1..12 {
            assert_eq!(
                build(seed),
                first,
                "the lane moved under a re-shuffled input"
            );
        }
        // …and the tie at 500 broke by name, with the unknown moments last.
        assert_eq!(
            names(&first),
            ["Delta", "Alpha", "Beta", "Beta", "Epsilon", "Gamma"]
        );
    }

    /// **Every playlist, always; the last 24 records** — the asymmetry stated
    /// as a test, because it is the property the panel's removal rests on.
    #[test]
    fn every_list_survives_and_the_twenty_fifth_record_falls_off() {
        let playlists: Vec<Touched> = (0..40)
            .map(|i| touched(Subject::Playlist(i), &format!("list {i:02}"), Some(1)))
            .collect();
        let records: Vec<Touched> = (0..40)
            .map(|i| {
                touched(
                    Subject::Record(i),
                    &format!("record {i:02}"),
                    Some(1000 + i),
                )
            })
            .collect();
        let rows = resolve(playlists, records);
        let kept =
            |kind: fn(&Subject) -> bool| rows.iter().filter(|row| kind(&row.subject)).count();
        assert_eq!(kept(|s| matches!(s, Subject::Playlist(_))), 40);
        assert_eq!(kept(|s| matches!(s, Subject::Record(_))), RECENT_ALBUMS);
        // The 24 kept are the 24 newest, not the first 24 seen.
        assert_eq!(rows[0].name, "record 39");
        assert!(!rows.iter().any(|row| row.name == "record 15"));
        assert!(rows.iter().any(|row| row.name == "record 16"));
    }

    /// The join the shell spends: paths → records, keeping the newest.
    #[test]
    fn the_ledger_folds_onto_records_by_their_newest_track() {
        let tracks = vec![(1_u64, "a"), (1, "b"), (2, "c"), (3, "d")];
        let played = |path: &str| match path {
            "a" => Some(10),
            "b" => Some(30),
            "c" => None,
            _ => Some(5),
        };
        let folded = by_record(tracks, played);
        assert_eq!(folded.get(&1), Some(&30));
        assert_eq!(
            folded.get(&2),
            None,
            "a record nobody played is not in the lane"
        );
        assert_eq!(folded.get(&3), Some(&5));
    }

    /// The head is a closed set of three, in the owner's order. A fourth
    /// destination is a nav rail, which is the thing doc 07 L8.4 refused and
    /// the thing this head is deliberately not allowed to grow into.
    #[test]
    fn the_head_is_three_destinations_in_the_owners_order() {
        assert_eq!(
            Destination::ALL.map(Destination::label),
            ["Home", "Library", "Now playing"]
        );
    }
}
