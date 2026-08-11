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
//! # The subject is one; the sections are two
//!
//! The owner, 2026-08-10: *"I guess we need to add playlists into their own
//! section under library"* — which **reverses his own brief for this surface**
//! (*"the side bar will have recent albums and playlists mixed based on some
//! order"*, ADR-0030's own epigraph). He is the authority and the reversal is
//! recorded rather than smoothed over: ADR-0030's sixth amendment.
//!
//! So [`resolve`] returns a [`Lane`] of **two sections** — `PLAYLISTS`, every
//! list, and `RECENT`, the records — and **no row is in both**, because a list
//! drawn in both sections is one door drawn twice, which is the L8.6 test the
//! whole product is held to.
//!
//! **What did *not* change is the key.** Both sections are ordered by
//! `(last touched, name)`, the one total key this module has always had, for
//! the reason directly below: a surface with two orderings has to arbitrate,
//! and the split is a split of *membership*, not of order. A list you played
//! this morning is still at the top of the lane's list half — it has moved
//! section, not rank.
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
//! The four fixed destinations — `Home`, `Library`, `Playlists`, `Now
//! playing` — are the owner's decision and they are [`crate::place`]
//! members, not lane rows. They have no order to compute and no membership to
//! decide: they are always all four, always in that order. See
//! [`Destination`].

use std::collections::HashMap;

/// **How many played records the lane holds**: 24 (ADR-0030 §1).
///
/// What falls off the end is the twenty-fifth-most-recent record, and nothing
/// is lost by it — every record is on the wall, one `Esc` away. The number is
/// a *bound on the surface*, not a window on the data: there is no "show more".
pub(crate) const RECENT_ALBUMS: usize = 24;

/// One of the lane's four fixed destinations.
///
/// The owner's decision, verbatim: *"home will appear at the top of the left
/// hand sidebar always either way and it will contain the top level concerns.
/// think spotify"*, and *"as an extension we will want a Now playing page at
/// the top with the Home and Library"*. ADR-0030 §1 had refused destination
/// rows in the lane on the grounds that a second subject is what killed the
/// last one; the owner overruled it, and the product's preamble says
/// that settles it. What survives of the argument is the *shape* of the
/// concession: the head is a fixed set of four, above a hairline, and the
/// list below it still has exactly one subject.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Destination {
    /// The home page: the interrupted run, and what is new.
    Home,
    /// The collection — the wall, its search and its arrangement.
    Library,
    /// Every saved playlist, with its own catalogue ordering.
    Playlists,
    /// The record that is sounding, at the size it deserves.
    NowPlaying,
}

impl Destination {
    /// The head, in the owner's order, and there is no other.
    pub(crate) const ALL: [Self; 4] =
        [Self::Home, Self::Library, Self::Playlists, Self::NowPlaying];

    /// The word on the row.
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Home => "Home",
            Self::Library => "Library",
            Self::Playlists => "Playlists",
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

/// **What a play touches** — the list it came from, if it came from one;
/// otherwise the record the track belongs to.
///
/// The owner, looking at the shipped lane: *"the recent bit shows albums
/// popping up even though it was the playlist which was played"*. He is right,
/// and the defect is an attribution one: the lane's records half is folded out
/// of the **play ledger**, which is per *path*, so a run reified from a list
/// marked every album the list quoted — three or four unrelated records
/// jumping to the head of the lane, one per track, while the list that was
/// actually played sat where its file's mtime had left it.
///
/// The rule is the lane's own subject sentence read literally. ADR-0030 §1
/// says it is *things you have touched*; you touched the list. The record is
/// what the engine read, which is a different question and one the bar already
/// answers.
///
/// **Provenance is the fact that decides it**, and it already exists:
/// [`QueueVm::provenance`](crate::vm::QueueVm::provenance) is *"the name of the
/// playlist file this run was reified from"* (ADR-0023's amendment) — origin,
/// never a live link. It is `None` for every other origin: a record, `Play
/// all`, a shuffle draw, a stacked queue. So a play attributes to a list
/// exactly when the run came from one, and nothing here has to guess from the
/// paths.
/// `Some(id)` is [`Subject::Playlist`]; `None` means the play attributes to
/// [`Subject::Record`] as it always did, and the caller resolves *which*
/// record, because mapping a path to one is the shelf's job and not this
/// module's.
pub(crate) fn played_list(provenance: Option<&str>) -> Option<u64> {
    provenance.map(crate::playlists::playlist_id)
}

/// **Which lane row a run touched** — [`played_list`]'s general form, over the
/// [`Origin`](crate::origin::Origin) a *ledger* carries rather than the
/// playlist name a live queue does.
///
/// The finding ADR-0034 records is that the mapping already existed and nobody
/// had called it one: **[`Subject::Record`] *is* the album's implicit list.**
/// The lane's two subjects were list identities before there was a type for
/// them, which is why this is a `match` and not a new concept.
///
/// `None` is *nothing in the lane moves*, and it is a real answer for four
/// kinds:
///
/// - **An artist's run.** The lane has no artist row. ADR-0030 §1's subject is
///   records and lists, and a third would be the second subject that killed
///   the last resident column.
/// - **`All songs`, and a draw.** *"A draw is not somewhere you return to"* —
///   there is nothing to go back to, so there is no row to raise.
/// - **A run made by hand.** There was no list.
///
/// An artist run is the deliberate exception to the old *no row, no marker*
/// rule: its marker preserves the list attribution and stops every quoted
/// record jumping in `RECENT`, while this function refuses to invent an artist
/// row in a lane whose resident subjects remain records and playlist files.
/// Library-wide `All songs`, draws and hand-built runs are not marked, so their
/// constituent record touches remain visible.
pub(crate) fn subject_of(origin: &crate::origin::Origin) -> Option<Subject> {
    use crate::origin::Origin;
    match origin {
        Origin::Playlist { id, .. } => Some(Subject::Playlist(*id)),
        Origin::Album { id, .. } => Some(Subject::Record(*id)),
        Origin::Artist { .. } | Origin::AllSongs | Origin::Draw | Origin::Hand { .. } => None,
    }
}

/// **Which lane row is sounding** — the row the lamp dot belongs on.
///
/// The owner, on the shipped lane: *"I still see albums specifically appearing
/// as if they are playing rather than the playlist. in a sense we need to track
/// which playlist + track is playing to actually understand what is happening
/// … it is showing next to the album rather than the playlist"*. He had
/// isolated it exactly: the **order** was already right, and the **mark** was
/// not.
///
/// The rule is [`played_list`]'s, and deliberately the same call rather than a
/// second one that agrees today: **the dot follows the run's origin.** A run
/// reified from a list marks the *list*, and none of the records it quotes,
/// however many of their tracks are sounding; every other run marks the record,
/// exactly as before. Order and mark now come out of one function, so they
/// cannot drift into saying different things about the same run — which is the
/// failure mode a second reading of "the same" fact always has.
///
/// # `sounding` is the liveness, and it is not optional
///
/// **A run's origin outlives the run.** `QueueVm::provenance` is a fact about
/// the list that *remains* after `QueueEnded` — deliberately, since ADR-0023 §4
/// keeps it until a replacing `SetQueue` — whereas the record this used to read
/// (`PlayerState::playing_album`) went to `None` the moment nothing sounded, and
/// was silently carrying the liveness for the whole mark. Reading the origin
/// without `sounding` therefore leaves the lamp lit on a list that finished an
/// hour ago. So the liveness is taken as its own argument and answered first:
/// **the dot says *this is on*, and only then *which row*.**
///
/// `record` is the sounding file's record, `None` for a file the library does
/// not hold — which still marks nothing for a record's run (the head's own dot
/// answers instead) but does **not** cost a list's run its mark, because a
/// list's identity does not depend on the library resolving the file.
///
/// **At most one row is ever marked**, because a run has one origin —
/// `only_one_row_is_ever_marked` is that as a test, since two dots would mean
/// the model had been broken upstream rather than that the lane had drawn
/// something odd.
pub(crate) fn sounding_subject(
    sounding: bool,
    provenance: Option<&str>,
    record: Option<u64>,
) -> Option<Subject> {
    if !sounding {
        return None;
    }
    match played_list(provenance) {
        Some(id) => Some(Subject::Playlist(id)),
        None => record.map(Subject::Record),
    }
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

/// **The lane's body: two sections, one order, no row in both.**
///
/// The owner asked for the lists to have a section of their own (ADR-0030's
/// sixth amendment), so the membership rule that was one list is two — and
/// this type is what makes *no row is in both* a fact of the model rather than
/// a discipline in the view. `PLAYLISTS` holds every list; `RECENT` holds
/// records and nothing else.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Lane {
    /// **`PLAYLISTS`** — every list, always, and never trimmed.
    pub(crate) lists: Vec<Touched>,
    /// **`RECENT`** — the last [`RECENT_ALBUMS`] records played, and nothing
    /// else. A list that was played this morning is at the head of
    /// [`Self::lists`]; it is not here as well.
    pub(crate) records: Vec<Touched>,
}

/// The lane, resolved: the two sections, each in the order it is drawn.
///
/// **Membership** (ADR-0030 §1, as its sixth amendment splits it): every
/// playlist, always — which is what lets the panel stop being the index
/// without any list becoming unreachable — and the last [`RECENT_ALBUMS`]
/// records played. **Order**: last touched, newest first, *within each
/// section*; ties break by name ascending, then by subject, so the key is
/// total and the lane is a function of the data rather than of the iteration
/// order of whatever collection the shell happened to build it from.
///
/// The playlists are *not* trimmed and the records are, which is the one
/// asymmetry in here and it is deliberate: a list you made is a thing you own
/// and the lane is its index, while a record you played is a thing you can
/// always find on the wall. It is also why the sections are drawn inside
/// **one** scroller and not two — see [`crate::views::lane`].
pub(crate) fn resolve(playlists: Vec<Touched>, records: Vec<Touched>) -> Lane {
    let mut lists = playlists;
    sort(&mut lists);
    Lane {
        lists,
        records: recent(records),
    }
}

/// `RECENT`'s half on its own: sorted, and cut to [`RECENT_ALBUMS`].
///
/// The shelf holds this half by itself (`Shelf::lane_recent`) because it is
/// folded out of the play ledger, which is the shelf's; the shell merges it
/// with the lists it owns in [`resolve`]. One function so the trim cannot be
/// spelled twice and drift.
pub(crate) fn recent(records: Vec<Touched>) -> Vec<Touched> {
    let mut records = records;
    sort(&mut records);
    records.truncate(RECENT_ALBUMS);
    records
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

    /// **Both sections as one sequence — the lane as the eye reads it.**
    ///
    /// The claims that need this are the ones about the *surface* rather than
    /// about a section: nothing is drawn twice, and at most one row is marked.
    /// Everything else asserts on a named section, because that is what the
    /// split made the honest question.
    fn drawn(lane: &Lane) -> Vec<&Touched> {
        lane.lists.iter().chain(lane.records.iter()).collect()
    }

    /// **Newest first, in each section, and no row is in both** — the whole of
    /// the order and the whole of the split.
    ///
    /// This test used to assert the opposite arrangement, and the diff is the
    /// owner changing his mind: *"the side bar will have recent albums and
    /// playlists mixed based on some order"* became *"I guess we need to add
    /// playlists into their own section under library"*. The **key** did not
    /// change — `Road Trip` still outranks `Sunday`, and `Ochre` still
    /// outranks `Violet Ledger`.
    #[test]
    fn the_two_kinds_are_two_sections_each_last_touched_first() {
        let lane = resolve(
            vec![
                touched(Subject::Playlist(1), "Road Trip", Some(300)),
                touched(Subject::Playlist(2), "Sunday", Some(100)),
            ],
            vec![
                touched(Subject::Record(10), "Ochre", Some(400)),
                touched(Subject::Record(11), "Violet Ledger", Some(200)),
            ],
        );
        assert_eq!(names(&lane.lists), ["Road Trip", "Sunday"]);
        assert_eq!(names(&lane.records), ["Ochre", "Violet Ledger"]);
    }

    /// **One door, drawn once.** `RECENT` is records and `PLAYLISTS` is lists,
    /// so a list that was played a minute ago — the row most at risk of being
    /// admitted to both, since it is the newest thing the lane knows — appears
    /// exactly once.
    #[test]
    fn no_row_stands_in_both_sections() {
        let lane = resolve(
            vec![touched(Subject::Playlist(1), "Road Trip", Some(9_000))],
            vec![
                touched(Subject::Record(10), "Ochre", Some(400)),
                touched(Subject::Record(11), "Violet Ledger", Some(200)),
            ],
        );
        assert!(
            lane.lists
                .iter()
                .all(|row| matches!(row.subject, Subject::Playlist(_))),
            "a record reached the lists section"
        );
        assert!(
            lane.records
                .iter()
                .all(|row| matches!(row.subject, Subject::Record(_))),
            "a list reached the records section"
        );
        let mut seen: Vec<Subject> = drawn(&lane).iter().map(|row| row.subject).collect();
        let drawn = seen.len();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), drawn, "a row is drawn twice");
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
        // …and the tie at 500 broke by name, with the unknown moments last —
        // in each section, which is where the key now applies.
        assert_eq!(names(&first.lists), ["Alpha", "Beta", "Gamma"]);
        assert_eq!(names(&first.records), ["Delta", "Beta", "Epsilon"]);
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
        let lane = resolve(playlists, records);
        assert_eq!(lane.lists.len(), 40);
        assert_eq!(lane.records.len(), RECENT_ALBUMS);
        // The 24 kept are the 24 newest, not the first 24 seen.
        assert_eq!(lane.records[0].name, "record 39");
        assert!(!lane.records.iter().any(|row| row.name == "record 15"));
        assert!(lane.records.iter().any(|row| row.name == "record 16"));
        // **The section that has no cap is the one the scroller has to
        // cope with**, and 40 lists over a 24-record section is the shape the
        // frames in `docs/design/impl/playlists-section/` prove: one scroller
        // over both, so `RECENT` is reachable however many lists there are.
        assert_eq!(drawn(&lane).len(), 40 + RECENT_ALBUMS);
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

    /// **A play attributes to the list it came from, and only then.**
    ///
    /// The owner: *"the recent bit shows albums popping up even though it was
    /// the playlist which was played"*. Every origin that is not a list still
    /// touches the record — a record's own `Play`, `Play all`, a shuffle draw,
    /// a stacked queue — because for those the record *is* what was pointed
    /// at, and provenance is `None` for all of them by construction
    /// (ADR-0023's amendment: it is set only where a play gesture reified a
    /// playlist file).
    #[test]
    fn a_play_attributes_to_the_list_it_came_from_and_otherwise_to_the_record() {
        assert_eq!(
            played_list(None),
            None,
            "a run with no provenance is a record's"
        );
        assert_eq!(
            played_list(Some("Road Trip")),
            Some(crate::playlists::playlist_id("Road Trip")),
            "a run reified from a list is the list's"
        );
        // The identity is the *name*'s, which is ADR-0024 §2's rule — the
        // filename is the name — so the lane's row and the play find each
        // other without a second key.
        assert_ne!(
            played_list(Some("Road Trip")),
            played_list(Some("Late Shift"))
        );
    }

    /// **The list rises, and the records it quotes do not.**
    ///
    /// The defect, restated as the lane the owner should have seen: one run,
    /// reified from `Road Trip`, playing tracks that belong to two records he
    /// last touched long ago. Before the fix both records jumped the list;
    /// after it, the list is at the head and the records are where they were.
    ///
    /// **The sections do not repair this and do not hide it.** The lists have
    /// their own heading now, so a list is trivially above the records it
    /// quotes — but the fact under test is the *moments*, and the records must
    /// still be where June left them within `RECENT`.
    #[test]
    fn playing_a_list_moves_the_list_and_leaves_its_records_where_they_were() {
        let played_at = 900;
        // Two records the run quotes, last played long before it.
        let records = vec![
            touched(Subject::Record(10), "Ochre", Some(100)),
            touched(Subject::Record(11), "Violet Ledger", Some(200)),
        ];
        // The run came from a list, so the list is what moves.
        let list_id = crate::playlists::playlist_id("Road Trip");
        assert_eq!(played_list(Some("Road Trip")), Some(list_id));
        let lists = vec![touched(
            Subject::Playlist(list_id),
            "Road Trip",
            Some(played_at),
        )];
        let lane = resolve(lists, records);
        assert_eq!(names(&lane.lists), ["Road Trip"]);
        assert_eq!(names(&lane.records), ["Violet Ledger", "Ochre"]);
        assert!(
            matches!(drawn(&lane)[0].subject, Subject::Playlist(_)),
            "the thing that was played is at the head of the lane"
        );
    }

    /// **The lane's two subjects are list identities**, and a run's origin
    /// maps onto them — the general form of the rule above, over what a
    /// *ledger* carries rather than what a live queue does.
    #[test]
    fn a_runs_origin_names_the_lane_row_it_touched() {
        use crate::origin::Origin;

        // A list's run touches the list …
        assert_eq!(
            subject_of(&Origin::playlist("Road Trip")),
            Some(Subject::Playlist(crate::playlists::playlist_id(
                "Road Trip"
            )))
        );
        // … and a record's run touches the record, because `Subject::Record`
        // *is* the album's implicit list. An album is not a playlist, and its
        // run is still credited to the thing that played.
        assert_eq!(
            subject_of(&Origin::Album {
                id: 42,
                name: "Ochre".to_owned()
            }),
            Some(Subject::Record(42))
        );
        // The kinds that are not somewhere you return to move nothing.
        for origin in [
            Origin::Artist {
                id: 7,
                name: "Talk Talk".to_owned(),
            },
            Origin::AllSongs,
            Origin::Draw,
            Origin::Hand { was: None },
        ] {
            assert_eq!(subject_of(&origin), None, "{origin:?}");
        }
    }

    /// The two readings of a playlist's run — the live one and the ledger's —
    /// name the same row. If they ever disagreed, a list would move on being
    /// played and move somewhere else on the next launch.
    #[test]
    fn the_live_reading_and_the_ledgers_agree_about_a_list() {
        let live = played_list(Some("Road Trip")).expect("a list's run");
        let filed = subject_of(&crate::origin::Origin::playlist("Road Trip"));
        assert_eq!(filed, Some(Subject::Playlist(live)));
    }

    /// **The whole launch-time fold, over a ledger written a week ago** — the
    /// cross-quit half of the owner's defect, end to end (ADR-0034).
    ///
    /// The situation, exactly as he meets it: `Road Trip` was played on Friday
    /// and quoted tracks from two records he last put on in June. He quits. He
    /// launches baz. Before this, `Playlists::played` was gone with the process
    /// and the lane re-derived Friday from the play lines — so both records
    /// jumped to the head and the list sat back at its file's mtime. After it,
    /// the ledger's run marker survives the quit and the list does.
    ///
    /// Written against a real `baz_core::history::History` folded out of real
    /// ledger bytes, because the claim is about what a *file* says a week
    /// later, not about what a value holds.
    #[test]
    fn a_list_played_last_week_comes_back_as_the_list_and_not_its_records() {
        use std::fmt::Write as _;
        use std::path::Path;

        // The four instants the ledger below spells, in the ledger's own
        // unit — so the assertions are about the file rather than about a
        // second calendar this test would otherwise have to keep.
        const JUNE: u64 = 1_780_033_600; // 2026-05-29T05:46:40Z
        const JUNE_LATER: u64 = 1_780_033_800; // 2026-05-29T05:50:00Z
        const FRIDAY_LAST: u64 = 1_785_043_440; // 2026-07-26T05:24:00Z

        let ochre = "/music/Ochre/01.flac";
        let violet = "/music/Violet Ledger/03.flac";
        let list = crate::origin::Origin::playlist("Road Trip");

        // The file, byte for byte, as two sessions would have left it: a run
        // in June that named no list, and Friday's run that named one.
        let mut ledger = String::new();
        let _ = writeln!(ledger, "# baz play history.");
        let _ = writeln!(ledger, "# baz run 2026-05-29T05:46:40Z -");
        let _ = writeln!(
            ledger,
            "2026-05-29T05:46:40Z\tplayed\t231480\t245013\t{ochre}"
        );
        let _ = writeln!(
            ledger,
            "2026-05-29T05:50:00Z\tplayed\t231480\t245013\t{violet}"
        );
        let _ = writeln!(ledger, "# baz run 2026-07-26T05:20:00Z {}", list.encode());
        let _ = writeln!(
            ledger,
            "2026-07-26T05:20:00Z\tplayed\t231480\t245013\t{ochre}"
        );
        let _ = writeln!(
            ledger,
            "2026-07-26T05:24:00Z\tplayed\t231480\t245013\t{violet}"
        );

        let history = baz_core::history::History::from_reader(ledger.as_bytes());
        assert_eq!(history.records(), 4);
        assert_eq!(history.malformed(), 0, "the markers were read as damage");

        // **The records half.** Both tracks were played on Friday and the
        // ledger still says so — but neither is a *record the listener put
        // on* since June, which is what the lane folds.
        for path in [ochre, violet] {
            assert_eq!(history.track(Path::new(path)).plays, 2);
        }
        let records = by_record([(10_u64, ochre), (11, violet)], |path| {
            history.last_played_unlisted(Path::new(path))
        });
        assert_eq!(records.get(&10), Some(&JUNE));
        assert_eq!(records.get(&11), Some(&JUNE_LATER));

        // **The lists half.** The run credits the list, at the moment of its
        // last play — Friday.
        let credited: Vec<(Subject, u64)> = history
            .runs()
            .iter()
            .filter_map(|run| {
                let at = run.last_played_unix_s?;
                let origin = crate::origin::Origin::decode(run.origin.as_deref()?)?;
                Some((subject_of(&origin)?, at))
            })
            .collect();
        let list_id = crate::playlists::playlist_id("Road Trip");
        assert_eq!(credited, vec![(Subject::Playlist(list_id), FRIDAY_LAST)]);

        // **And the lane the owner sees.** The list is at the head, and the
        // records it quoted are where June left them.
        let lane = resolve(
            vec![touched(
                Subject::Playlist(list_id),
                "Road Trip",
                Some(credited[0].1),
            )],
            vec![
                touched(Subject::Record(10), "Ochre", records.get(&10).copied()),
                touched(
                    Subject::Record(11),
                    "Violet Ledger",
                    records.get(&11).copied(),
                ),
            ],
        );
        assert_eq!(names(&lane.lists), ["Road Trip"]);
        assert_eq!(names(&lane.records), ["Violet Ledger", "Ochre"]);
        assert!(
            matches!(drawn(&lane)[0].subject, Subject::Playlist(_)),
            "the thing that was played is not at the head of the lane"
        );
    }

    /// **An album's run still credits the album.** A fixed list is not a
    /// playlist: the marker's job is to stop a *playlist's* run crediting its
    /// albums, not to stop albums being credited when an album is what played.
    #[test]
    fn a_ledger_with_no_marked_list_folds_exactly_as_it_always_did() {
        use std::fmt::Write as _;
        use std::path::Path;

        let track = "/music/Ochre/01.flac";
        let mut unmarked = String::from("# baz play history.\n");
        let _ = writeln!(
            unmarked,
            "2026-07-26T05:20:00Z\tplayed\t231480\t245013\t{track}"
        );
        // The same plays, under a marker that names no list — an album's run,
        // `Play all`, a draw. Both must fold to the record.
        let mut marked = String::from("# baz play history.\n# baz run 2026-07-26T05:20:00Z -\n");
        let _ = writeln!(
            marked,
            "2026-07-26T05:20:00Z\tplayed\t231480\t245013\t{track}"
        );

        for text in [unmarked, marked] {
            let history = baz_core::history::History::from_reader(text.as_bytes());
            assert_eq!(history.malformed(), 0, "{text}");
            assert_eq!(
                history.last_played_unlisted(Path::new(track)),
                history.track(Path::new(track)).last_played_unix_s,
                "the record lost its own play\n{text}"
            );
            assert!(history.last_played_unlisted(Path::new(track)).is_some());
        }
    }

    /// **The dot follows the run's origin, not the sounding file's record.**
    ///
    /// The owner, on the shipped lane: *"I still see albums specifically
    /// appearing as if they are playing rather than the playlist … it is
    /// showing next to the album rather than the playlist"*. Recency was
    /// already right; this is the mark.
    #[test]
    fn the_sounding_row_is_the_list_when_a_list_is_what_was_put_on() {
        let list = crate::playlists::playlist_id("Road Trip");

        // A run reified from a list marks the **list**, even though the track
        // sounding belongs to a record the lane also holds.
        assert_eq!(
            sounding_subject(true, Some("Road Trip"), Some(10)),
            Some(Subject::Playlist(list))
        );
        // Every other run marks the record, exactly as before.
        assert_eq!(
            sounding_subject(true, None, Some(10)),
            Some(Subject::Record(10))
        );
        // A *record's* run over a file the library does not hold marks nothing
        // — the head's own dot still answers, and the list has nothing to
        // point at. A **list's** run is not so limited: its identity is the
        // file's name, not the library's opinion of the track.
        assert_eq!(sounding_subject(true, None, None), None);
        assert_eq!(
            sounding_subject(true, Some("Road Trip"), None),
            Some(Subject::Playlist(list))
        );
    }

    /// **Nothing sounding, nothing marked** — its own test rather than a line
    /// in the one above, because this is the case that made `sounding` a
    /// separate argument. A run's origin **outlives the run**: ADR-0023 §4
    /// keeps it until a replacing `SetQueue`, so a lane that read the origin
    /// alone would leave the lamp lit on a list that finished an hour ago. The
    /// record this mark used to read was carrying the liveness by accident,
    /// because it went to `None` the moment the music stopped.
    #[test]
    fn a_finished_run_leaves_no_lamp_behind() {
        // The run ended: the queue still knows which list it came from, and
        // there is no sounding record any more.
        assert_eq!(sounding_subject(false, Some("Road Trip"), None), None);
        // …and silence marks nothing even where a record is still resolvable.
        assert_eq!(sounding_subject(false, Some("Road Trip"), Some(10)), None);
        assert_eq!(sounding_subject(false, None, Some(10)), None);
    }

    /// **At most one row is ever marked.** A run has one origin, so two dots
    /// would mean the model had been broken upstream rather than that the lane
    /// had drawn something odd — which is why this is asserted over the whole
    /// resolved lane rather than trusted to the row loop.
    #[test]
    fn only_one_row_is_ever_marked() {
        let list = crate::playlists::playlist_id("Road Trip");
        let lane = resolve(
            vec![
                touched(Subject::Playlist(list), "Road Trip", Some(300)),
                touched(Subject::Playlist(2), "Sunday", Some(100)),
            ],
            vec![
                touched(Subject::Record(10), "Ochre", Some(400)),
                touched(Subject::Record(11), "Violet Ledger", Some(200)),
            ],
        );
        for (provenance, record) in [
            (Some("Road Trip"), Some(10)),
            (Some("Road Trip"), None),
            (None, Some(10)),
            (None, Some(11)),
            (None, None),
            // A list the lane does not hold, and a record it does not hold:
            // nothing is marked rather than something arbitrary.
            (Some("Late Shift"), None),
            (None, Some(999)),
        ] {
            let sounding = sounding_subject(true, provenance, record);
            // Over **both** sections: a mark that appeared once per section
            // would be exactly the two-doors defect the split has to avoid.
            let marked = drawn(&lane)
                .iter()
                .filter(|row| sounding.is_some() && sounding == Some(row.subject))
                .count();
            assert!(
                marked <= 1,
                "{provenance:?}/{record:?} marked {marked} rows"
            );
        }
        // And the list's run marks the list itself — not nothing, and not one
        // of the records it quotes.
        let sounding = sounding_subject(true, Some("Road Trip"), Some(10));
        let marked: Vec<&str> = drawn(&lane)
            .iter()
            .filter(|row| sounding == Some(row.subject))
            .map(|row| row.name.as_str())
            .collect();
        assert_eq!(marked, ["Road Trip"]);
    }

    /// The head contains the four top-level collection destinations in the
    /// owner's order.
    #[test]
    fn the_head_destinations_are_in_the_owners_order() {
        assert_eq!(
            Destination::ALL.map(Destination::label),
            ["Home", "Library", "Playlists", "Now playing"]
        );
    }
}
