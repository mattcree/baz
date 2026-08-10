//! **Implicit playlists** — the lists nobody made a file for, and the type they
//! share.
//!
//! ADR-0006 layer 1: pure, iced-free, unit-tested. Records and the wall's own
//! filter in; an origin, two counts, a queue and four sleeve quotations out. No
//! window, no engine, no disk — and *no file*, which is the whole point.
//!
//! # The model this is shaped for
//!
//! The owner, 2026-08-10: *"probably the basic model is that every album has a
//! playlist implicitly… and so when we track the state of what is playing now
//! or what our recent plays were… it should be basically which playlist and
//! which track."*
//!
//! So **everything that plays is a list and a cursor**, and lists differ only
//! in what they are made of and what identity they have. A named playlist has a
//! *file*; an album's implicit list would have an *album id*; a draw's list has
//! nothing durable at all; **All songs** has only a name. This module is the
//! type for the ones with no file behind them, and it is deliberately built as
//! a **kind with an origin** rather than as one bespoke thing, so that the
//! remaining kinds can be named without it being rewritten.
//!
//! Two origins are built here: the library-wide [`Origin::AllSongs`] and an
//! [`Origin::Artist`] for the `All songs` tile on each artist page. [`Origin`]'s
//! own docs say exactly where an album's list and a draw's list slot in and what
//! each would carry. The queue carries this origin as inert identity: it can
//! label the run, lead back to its source page and be written into the play
//! ledger without deciding a single track in the order.
//!
//! # What an implicit list is, and what it is not
//!
//! `docs/design/09-implicit-playlists.md` §2 has listed these since the study
//! was written — *"the wall, in its arrangement"*, a draw, the queue — and its
//! model line is *"baz has one kind of list. One of them is sounding and has no
//! name; the rest are named and silent."* The vocabulary existed and the type
//! did not: `grep -rn "implicit playlist" crates/` returned one comment.
//!
//! The load-bearing property, and the one every origin shares: **an implicit
//! list is playable and viewable, never a destination.** There is no file to
//! append to, so the picker must never offer `Add to "…"` for one. That is
//! structural rather than remembered — the picker's rows are
//! [`crate::playlists::PanelRow`]s read out of the playlists folder, and this
//! type is not one and cannot become one: it has no id, no path and no `save`,
//! and [`Origin::file`] answers `None` by construction. `crate::menu`'s own
//! sweep asserts it anyway, because "structurally impossible" is what the last
//! surface that grew a destination looked like from the inside too.
//!
//! # All songs: what it is ordered by, stated plainly
//!
//! **The wall's own arrangement and the wall's own filter.** It is a *view of a
//! live thing*, not a snapshot, and this module will not pretend otherwise.
//!
//! The alternative was a stable order of its own — ARTIST, say, regardless of
//! what the wall is doing — which would make "playing it twice is the same
//! list" true by construction. It is not taken, and the reason is that it would
//! put two statements in the product that cannot both be true. `Play all`'s
//! scope has always been *exactly what the wall shows, in the wall's own
//! order* (doc 09 §7.1), and `Play all` is now this list's own `Play` — one
//! concept, not two. A list that played a different order from the one on
//! screen would buy a property nobody asked for at the cost of the one the
//! gesture already promised.
//!
//! So: **playing it twice with the wall unchanged is the same list**, and the
//! wall states its own arrangement and its own query on screen, in the strip
//! and in the well. Nothing is hidden; what changes the list is a control the
//! listener pressed.
//!
//! **And the name stays honest under a query.** A list called *All songs* that
//! held seven of twelve hundred records would be lying, so
//! [`ImplicitList::counts`] says which case it is in — the plain figures at
//! rest, and `7 of 1284 records` while the wall is filtered. The honesty is in
//! the readout rather than in a second name, because the list *is* the wall and
//! the wall is one thing.
//!
//! # Where you look at one
//!
//! For All songs: **the wall**, and there is deliberately no second page. Doc
//! 09 §2 names the wall itself as where this list is seen, and doc 07 L8.6
//! refuses one fact drawn twice; a page listing the same music as text would be
//! the same collection drawn worse, without the art, and would need its own
//! virtual window, its own scroll memory and its own search to catch up with
//! the surface baz opens onto. The panel's row is the *handle* — name, counts,
//! sleeve, and a press that takes you to the wall.

use crate::vm::{AlbumVm, EditionKey, QueueVm, format_duration, stacked_queue};

/// **Which list this is**, and the identity that kind of list has —
/// [`crate::origin::Origin`], which is this module's own one-variant enum
/// grown the kinds that have a file (ADR-0034 §1.4).
///
/// It is re-exported rather than kept and shadowed. Two enums both called
/// `Origin` — one naming the fileless lists, one naming runs' lists — would be
/// the worst possible outcome of two people answering the same sentence of the
/// owner's, and there is one sentence.
///
/// **What this module still owns is the *meaning* of implicit**, and the
/// promotion did not widen it: an implicit list is one with no file behind it,
/// which is `origin.file().is_none()` — exactly what this module's docs
/// already said it was. [`ImplicitList`] carries no state of its own, and the
/// run's origin lives on the run.
pub(crate) use crate::origin::Origin;

/// How many records a list's sleeve quotes, and the reason is
/// [`crate::playlists::PanelRow::art`]'s: four for the 2 × 2 collage.
const QUOTED: usize = 4;

/// **A playable list with no file behind it**, resolved and ready to draw.
///
/// One type for every [`Origin`], holding no state of its own — which is what
/// makes *"a view of a live thing"* a fact about it rather than a promise about
/// how it is used. There is nowhere here to cache an order, so there is no
/// order that can go stale.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ImplicitList {
    /// Which list this is, and the identity that kind carries.
    pub(crate) origin: Origin,
    /// What `Play` sends, and what the counts were taken from — one value for
    /// both, so the paths sent and the figures shown cannot describe different
    /// music ([`QueueVm`]'s own rule).
    pub(crate) queue: QueueVm,
    /// The sleeve's quotations: the first [`QUOTED`] records the list holds, in
    /// its own order. Fewer means "draw the first full-bleed", none means the
    /// rest tile — [`crate::views::playlist_sleeve`]'s rule, unchanged, because
    /// an implicit list is a list and gets a list's sleeve.
    pub(crate) art: Vec<u64>,
    /// How many records this list spans.
    pub(crate) records: usize,
    /// **The larger set this list is a view of**, when it is a view of one.
    ///
    /// `Some` for a list a filter can narrow — All songs is the wall, and the
    /// wall's query narrows it, so this is the library's own record count and
    /// the denominator [`ImplicitList::counts`] needs to stay honest under a
    /// query. `None` for a list whose extent is fixed by what it *is*: an
    /// album's tracks are its tracks, and no query makes them fewer.
    ///
    /// The distinction is the reason this is an `Option` and not a number. A
    /// list that is not a view of anything has no "of N" to print, and printing
    /// one would invent a whole for it to be part of.
    pub(crate) narrowed_from: Option<usize>,
}

impl ImplicitList {
    /// **All songs**, resolved from the wall: `albums` in the active group
    /// key's order, `visible` the filter's own index list, `chosen` the edition
    /// picked per record.
    ///
    /// The same three inputs, in the same order, that decide what is on screen
    /// — and there is deliberately nowhere to put a fourth. A future `MOOD`
    /// group key, or a mood spelled as a query, arrives here as a different
    /// `albums`/`visible` pair and needs not one line of new code.
    ///
    /// Named for its origin rather than called `new`, because each origin has
    /// its own inputs: an album's list would take an `AlbumVm` and a chosen
    /// edition, and would be a sibling of this rather than a branch inside it.
    pub(crate) fn all_songs(
        albums: &[AlbumVm],
        visible: &[usize],
        chosen: impl Fn(u64) -> Option<EditionKey>,
    ) -> Self {
        let picks: Vec<(&AlbumVm, Option<EditionKey>)> = visible
            .iter()
            .filter_map(|&index| albums.get(index))
            .map(|album| (album, chosen(album.id)))
            .collect();
        let art = picks
            .iter()
            .map(|(album, _)| album.id)
            .take(QUOTED)
            .collect();
        let origin = Origin::AllSongs;
        let mut queue = stacked_queue(&picks);
        queue.origin = Some(origin.clone());
        // The gesture materializes this implicit list as tonight's unsaved
        // playlist. Its rows are fixed now; a durable artefact exists only if
        // the listener later chooses `Save as playlist`.
        queue.source = crate::vm::RunSource::Assembled;
        Self {
            origin,
            records: picks.len(),
            art,
            queue,
            // The wall is a view of the library, and the query narrows it.
            narrowed_from: Some(albums.len()),
        }
    }

    /// **All songs, whole** — the same list over the *unfiltered* library,
    /// which is what Home's tile plays.
    ///
    /// The one place two resolutions of one origin exist, and the difference is
    /// **scope, not identity**: same [`Origin`], same name, same sleeve rule,
    /// same `Play`. What differs is which wall it is a view of.
    ///
    /// Why Home's is not the filtered one. The strip's `Play all` sits beside
    /// the query and the arrangement that decide the wall's scope, and its whole
    /// contract is *exactly what you can see*. **Home shows no wall and no
    /// query.** A tile there that quietly applied a filter set five minutes ago
    /// on another page would be the interface acting on state the listener
    /// cannot see from where they are standing — which is the same rule, applied
    /// to a surface where "what you can see" is a different set.
    ///
    /// It needs no new machinery, and that is the point of
    /// [`Self::all_songs`]'s shape: a different `visible` arriving is all a
    /// different scope ever is. The whole library's index list is one such
    /// `visible`, so [`Self::filtered`] correctly answers `false` and
    /// [`Self::counts`] prints the plain form.
    pub(crate) fn everything(
        albums: &[AlbumVm],
        chosen: impl Fn(u64) -> Option<EditionKey>,
    ) -> Self {
        let visible: Vec<usize> = (0..albums.len()).collect();
        Self::all_songs(albums, &visible, chosen)
    }

    /// **One artist's All songs** — their records in release chronology, then
    /// each selected edition's own disc/track order.
    ///
    /// Chronology is the list's arrangement rather than the wall's: the wall
    /// can be regrouped by genre, format or recency, but an artist's songs have
    /// a stable history of their own. Undated records follow dated ones; ties
    /// are title then id, so two launches over the same library make the same
    /// run whatever order the wall happened to arrive in.
    pub(crate) fn artist(
        albums: &[AlbumVm],
        artist: u64,
        name: &str,
        chosen: impl Fn(u64) -> Option<EditionKey>,
    ) -> Self {
        let mut picks: Vec<(&AlbumVm, Option<EditionKey>)> = albums
            .iter()
            .filter(|album| crate::vm::artist_id(&album.artist) == artist)
            .map(|album| (album, chosen(album.id)))
            .collect();
        picks.sort_by(|(a, _), (b, _)| {
            a.year
                .unwrap_or(u32::MAX)
                .cmp(&b.year.unwrap_or(u32::MAX))
                .then_with(|| {
                    a.title
                        .as_deref()
                        .unwrap_or_default()
                        .to_lowercase()
                        .cmp(&b.title.as_deref().unwrap_or_default().to_lowercase())
                })
                .then_with(|| a.title.cmp(&b.title))
                .then_with(|| a.id.cmp(&b.id))
        });
        let art = picks
            .iter()
            .map(|(album, _)| album.id)
            .take(QUOTED)
            .collect();
        let origin = Origin::Artist {
            id: artist,
            name: name.to_owned(),
        };
        let mut queue = stacked_queue(&picks);
        queue.origin = Some(origin.clone());
        queue.source = crate::vm::RunSource::Assembled;
        Self {
            origin,
            records: picks.len(),
            art,
            queue,
            narrowed_from: None,
        }
    }

    /// The list's name — its origin's.
    ///
    /// A borrow rather than a `&'static str`, and not `const`, because
    /// [`Origin`] now holds kinds whose name differs per instance (ADR-0034
    /// §1.4). `All songs` is still one `'static` word; the signature is what
    /// changed, not the string.
    pub(crate) fn name(&self) -> &str {
        match &self.origin {
            Origin::Artist { .. } => "All songs",
            _ => self.origin.name(),
        }
    }

    /// Whether the list is empty — an empty library, or a query that matched
    /// no record. Playing it does nothing and claims nothing, which is the
    /// rule every play gesture in baz keeps.
    pub(crate) fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Whether a filter is narrowing the list below the set it is a view of.
    ///
    /// A comparison rather than a copy of the query, because what matters is
    /// not *whether somebody typed* but whether the list is the whole of what
    /// it is a view of. A query that matches everything narrows nothing, and
    /// the readout should not claim it did. A list that is a view of nothing
    /// larger ([`Self::narrowed_from`] `None`) is never narrowed.
    pub(crate) fn filtered(&self) -> bool {
        self.narrowed_from.is_some_and(|whole| self.records < whole)
    }

    /// **The row's second line**: `1284 records · 9902 songs · 84:12:07` at
    /// rest, `7 of 1284 records · 62 songs · 4:31:02` while the wall is
    /// filtered.
    ///
    /// The filtered spelling is the honesty the name cannot carry on its own
    /// (module docs): a list called *All songs* that says `7 of 1284` is not
    /// claiming to be all of them. `records` leads because the wall is made of
    /// records and the sleeve beside this line is a record's; `songs` follows
    /// because that is the list's own name and the unit that plays.
    ///
    /// The time is omitted rather than printed as `0:00` when nothing declared
    /// one — a bare imported path list has no `#EXTINF`, and the catalogue does
    /// not print a figure it has not measured
    /// ([`crate::playlists::PanelRow::counts`]'s rule).
    pub(crate) fn counts(&self) -> String {
        let head = match self.narrowed_from.filter(|_| self.filtered()) {
            Some(whole) => format!("{} of {whole} records", self.records),
            None => format!("{} records", self.records),
        };
        let songs = format!("{head} · {} songs", self.queue.len());
        let time = self.queue.total_time();
        if time.is_zero() {
            songs
        } else {
            format!("{songs} · {}", format_duration(time))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::Duration;

    use super::*;
    use crate::vm::{AlbumArtistVm, EditionVm, ReplayGainCoverage, TrackVm};

    fn track(path: &str, seconds: Option<u64>) -> TrackVm {
        TrackVm {
            number: None,
            disc: None,
            title: path.to_owned(),
            artist: None,
            duration: seconds.map(Duration::from_secs),
            path: PathBuf::from(path),
            bytes: None,
        }
    }

    /// One record, `name`, with two tracks of `seconds` each.
    fn album(name: &str, seconds: Option<u64>) -> AlbumVm {
        let tracks: Vec<TrackVm> = (1..=2)
            .map(|side| track(&format!("/m/{name}/{side}.flac"), seconds))
            .collect();
        AlbumVm {
            id: crate::vm::album_id(baz_core::index::AlbumArtist::Named(name), Some(name)),
            title: Some(name.to_owned()),
            artist: AlbumArtistVm::Named(name.to_owned()),
            track_artists_vary: false,
            year: None,
            genre: None,
            first_seen_ns: None,
            first_track: tracks[0].path.clone(),
            editions: vec![EditionVm {
                key: EditionKey(None),
                detail: None,
                bitrate: None,
                bit_depth: None,
                sample_rate: None,
                replay_gain: ReplayGainCoverage {
                    album: 0,
                    track: 0,
                    total: tracks.len(),
                },
                tracks,
            }],
        }
    }

    /// Six records, `a`..`f`, two 100-second tracks each.
    fn wall() -> Vec<AlbumVm> {
        ["a", "b", "c", "d", "e", "f"]
            .iter()
            .map(|name| album(name, Some(100)))
            .collect()
    }

    fn of(albums: &[AlbumVm], visible: &[usize]) -> ImplicitList {
        ImplicitList::all_songs(albums, visible, |_| None)
    }

    fn all(albums: &[AlbumVm]) -> ImplicitList {
        let visible: Vec<usize> = (0..albums.len()).collect();
        of(albums, &visible)
    }

    fn release(artist: &str, title: &str, year: Option<u32>) -> AlbumVm {
        let mut album = album(title, Some(100));
        album.artist = AlbumArtistVm::Named(artist.to_owned());
        album.title = Some(title.to_owned());
        album.year = year;
        album.id = crate::vm::album_id(baz_core::index::AlbumArtist::Named(artist), Some(title));
        album
    }

    /// **The list is the wall, in the wall's own order** — the property that
    /// makes it the implicit playlist doc 09 §2 already named, rather than a
    /// second collection with opinions of its own.
    #[test]
    fn the_list_is_the_wall_in_the_walls_own_order() {
        let albums = wall();
        let titles = |list: &ImplicitList| -> Vec<String> {
            list.queue
                .items
                .iter()
                .filter_map(|item| item.album.clone())
                .collect()
        };

        // Whole records, in wall order, never flattened or interleaved: two
        // rows per record, each naming the record it belongs to.
        let whole = all(&albums);
        assert_eq!(whole.records, 6);
        assert_eq!(whole.queue.len(), 12);
        assert_eq!(whole.queue.source, crate::vm::RunSource::Assembled);
        assert_eq!(
            titles(&whole),
            ["a", "a", "b", "b", "c", "c", "d", "d", "e", "e", "f", "f"]
        );

        // A query narrows it to the matches, still in wall order.
        assert_eq!(titles(&of(&albums, &[1, 3])), ["b", "b", "d", "d"]);

        // **A different arrangement is a different order**, and the list
        // follows it rather than re-sorting: a group key is a different
        // `albums` order arriving here.
        let mut reordered = albums.clone();
        reordered.reverse();
        assert_eq!(
            titles(&all(&reordered)),
            ["f", "f", "e", "e", "d", "d", "c", "c", "b", "b", "a", "a"]
        );
    }

    /// **Playing it twice is the same list** while the wall has not moved —
    /// the honest form of the stability question, since this is a view of a
    /// live thing and says so.
    #[test]
    fn resolving_it_twice_over_an_unchanged_wall_is_the_same_list() {
        let albums = wall();
        let visible: Vec<usize> = (0..albums.len()).collect();
        assert_eq!(of(&albums, &visible), of(&albums, &visible));
        assert_eq!(
            of(&albums, &visible).queue.paths(),
            of(&albums, &visible).queue.paths()
        );
    }

    #[test]
    fn an_artists_list_is_chronological_whatever_order_the_wall_is_in() {
        let albums = vec![
            release("Low", "Later", Some(2005)),
            release("Other", "Not theirs", Some(1980)),
            release("LOW", "Early B", Some(1994)),
            release("Low", "Undated", None),
            release("Low", "Early A", Some(1994)),
        ];
        let artist = crate::vm::artist_id(&AlbumArtistVm::Named("low".to_owned()));
        let list = ImplicitList::artist(&albums, artist, "Low", |_| None);
        let records: Vec<&str> = list
            .queue
            .items
            .iter()
            .step_by(2)
            .filter_map(|item| item.album.as_deref())
            .collect();

        assert_eq!(records, ["Early A", "Early B", "Later", "Undated"]);
        assert_eq!(list.records, 4);
        assert_eq!(list.queue.len(), 8);
        assert_eq!(list.queue.source, crate::vm::RunSource::Assembled);
        assert_eq!(list.name(), "All songs");
        assert_eq!(list.counts(), "4 records · 8 songs · 13:20");
        assert!(!list.filtered(), "an artist is a fixed scope");
        assert_eq!(
            list.origin,
            Origin::Artist {
                id: artist,
                name: "Low".to_owned(),
            }
        );
        assert_eq!(
            list.art,
            [albums[4].id, albums[2].id, albums[0].id, albums[3].id]
        );
        assert_eq!(list.origin.file(), None);
    }

    /// **The name stays honest under a query.** A list called *All songs* that
    /// held seven of twelve hundred records would be lying, so the readout says
    /// which case it is in.
    #[test]
    fn the_counts_say_when_the_wall_is_filtered() {
        let albums = wall();
        let whole = all(&albums);
        assert!(!whole.filtered());
        assert_eq!(whole.counts(), "6 records · 12 songs · 20:00");

        let narrowed = of(&albums, &[0, 1]);
        assert!(narrowed.filtered());
        assert_eq!(narrowed.counts(), "2 of 6 records · 4 songs · 6:40");

        // A query that matches everything narrows nothing, and must not claim
        // it did.
        let everything: Vec<usize> = (0..albums.len()).collect();
        assert!(!of(&albums, &everything).filtered());

        // Nothing declared a length: the figure is omitted rather than printed
        // as 0:00 about music baz has not measured.
        let untimed: Vec<AlbumVm> = ["a", "b"].iter().map(|n| album(n, None)).collect();
        assert_eq!(all(&untimed).counts(), "2 records · 4 songs");
    }

    /// Every empty case, because each is a real state of the wall, and a play
    /// gesture over it must do nothing and claim nothing.
    #[test]
    fn an_empty_wall_is_an_empty_list() {
        let albums = wall();
        // An empty library.
        assert!(of(&[], &[]).is_empty());
        // A query that matched nothing.
        assert!(of(&albums, &[]).is_empty());
        // An index the wall no longer holds is skipped rather than panicking:
        // the filter and the album list are rebuilt separately under a scan.
        assert!(of(&albums, &[99]).is_empty());
        assert!(!all(&albums).is_empty());
        // An empty list still answers, and answers honestly.
        assert_eq!(of(&albums, &[]).counts(), "0 of 6 records · 0 songs");
    }

    /// **The sleeve quotes the first four records the wall shows** — a
    /// playlist's collage rule (ADR-0024 §A1), applied to a list that has no
    /// file, because an implicit list is a list and gets a list's sleeve.
    #[test]
    fn the_sleeve_quotes_the_first_four_records_on_the_wall() {
        let albums = wall();
        let ids: Vec<u64> = albums.iter().map(|album| album.id).collect();
        assert_eq!(all(&albums).art, ids[..4]);

        // Under a query it quotes the matches, so the sleeve is a picture of
        // the list rather than of the library behind it.
        assert_eq!(of(&albums, &[4, 5]).art, [ids[4], ids[5]]);

        // Fewer than four, and none at all, are both ordinary: the sleeve's own
        // rule covers them and this type does not pad.
        assert_eq!(of(&albums, &[2]).art.len(), 1);
        assert!(of(&albums, &[]).art.is_empty());
    }

    /// **The list's run carries no provenance**, which is the upstream half of
    /// "the picker never offers `Add to \"All songs\"`".
    ///
    /// Playing provenance (doc 09 §6) is *the name of the playlist file this
    /// run was reified from*. Give the wall's run one and every context menu in
    /// the product immediately offers `Add to "{name}"` — a verb promising a
    /// write to a file that does not exist. That is the trap, and it is closed
    /// here rather than downstream: `menu.rs` reads provenance, so the only way
    /// it can ever see the implicit list's name is if this module writes one.
    #[test]
    fn playing_the_list_gives_the_run_no_provenance_to_offer() {
        let albums = wall();
        assert_eq!(all(&albums).queue.provenance(), None);
        assert_eq!(of(&albums, &[1, 2]).queue.provenance(), None);
        assert_eq!(of(&albums, &[]).queue.provenance(), None);
    }

    /// **Every origin answers "which file?" with `None`**, which is the
    /// property the whole kind is built on: an implicit list has nothing for
    /// the picker to append to and nothing for playing provenance to name.
    ///
    /// Swept over every variant rather than asserted about the one that exists,
    /// so that adding an album's list or a draw's list to [`Origin`] without
    /// answering this question fails here.
    #[test]
    fn no_origin_has_a_file_behind_it() {
        for origin in [
            Origin::AllSongs,
            Origin::Artist {
                id: 1,
                name: "Low".to_owned(),
            },
        ] {
            assert_eq!(origin.file(), None, "{origin:?} claimed a file");
            assert!(!origin.name().is_empty(), "{origin:?} has no name");
        }
        assert_eq!(Origin::AllSongs.name(), "All songs");
    }

    /// **Home's tile plays everything you own, whatever the wall is filtered
    /// to** — the one place two resolutions of one origin exist, differing in
    /// scope and in nothing else.
    #[test]
    fn everything_is_the_whole_library_however_the_wall_is_narrowed() {
        let albums = wall();
        let whole = ImplicitList::everything(&albums, |_| None);
        assert_eq!(whole.records, 6);
        assert_eq!(whole.queue.len(), 12);
        assert!(!whole.filtered(), "nothing narrows everything you own");
        assert_eq!(whole.counts(), "6 records \u{b7} 12 songs \u{b7} 20:00");

        // It is the same list the wall resolves when no query is standing …
        assert_eq!(whole, all(&albums));
        // … and it is *not* the narrowed one, which is the whole reason it
        // exists as its own call.
        assert_ne!(whole, of(&albums, &[0, 1]));

        // Same origin, same name, same absence of a file: a second scope is not
        // a second kind of list.
        assert_eq!(whole.origin, Origin::AllSongs);
        assert_eq!(whole.name(), "All songs");
        assert_eq!(whole.origin.file(), None);
        assert_eq!(whole.queue.provenance(), None);

        // An empty library is an empty list, not a panic.
        assert!(ImplicitList::everything(&[], |_| None).is_empty());
    }

    /// **`narrowed_from` is what makes the counts general.** A list that is a
    /// view of something larger prints "of N"; one whose extent is fixed by
    /// what it is has no whole to be part of, and must not invent one.
    #[test]
    fn a_list_that_is_a_view_of_nothing_larger_is_never_narrowed() {
        let albums = wall();
        // All songs is a view of the library, so it carries the denominator.
        assert_eq!(all(&albums).narrowed_from, Some(6));

        // The shape an album's own list would have: the same type, no whole
        // behind it. Built by hand here because that origin is not built yet —
        // what is asserted is that the *type* already answers correctly for it.
        let fixed = ImplicitList {
            narrowed_from: None,
            ..of(&albums, &[0, 1])
        };
        assert!(!fixed.filtered(), "a fixed-extent list cannot be narrowed");
        assert_eq!(fixed.counts(), "2 records · 4 songs · 6:40");
    }

    /// **There is no file behind it, and no way to make one.** The picker's
    /// destinations are `PanelRow`s read out of the playlists folder; this type
    /// carries no id, no path and no `save`, which is what makes
    /// `Add to "All songs"` unrepresentable rather than merely absent.
    ///
    /// Asserted over this module's own source, because the property is about
    /// what the type *does not have* — and a field added later would satisfy
    /// any test written over its behaviour.
    #[test]
    fn the_list_carries_nothing_that_could_be_written_to() {
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/implicit.rs"),
        )
        .expect("this module's own source")
        .replace("\r\n", "\n");
        let start = source
            .find("pub(crate) struct ImplicitList {")
            .expect("the type exists");
        let rest = &source[start..];
        let body = &rest[..rest.find("\n}\n").expect("the struct ends")];
        for forbidden in ["PathBuf", "path:", "id:", "fn save"] {
            assert!(
                !body.contains(forbidden),
                "`ImplicitList` grew `{forbidden}` — an implicit list is playable \
                 and viewable, never a destination (module docs, doc 09 §2)"
            );
        }
        // And nothing in the module writes anything anywhere. Spelled in
        // halves so these needles are not their own counter-examples: the
        // assertion must not appear in the file it searches.
        for (head, tail) in [("fs", "write"), ("File", "create"), ("Playlist", "save")] {
            let forbidden = format!("{head}::{tail}");
            assert!(
                !source.contains(&forbidden),
                "this module reached for disk through `{forbidden}`"
            );
        }
    }
}
