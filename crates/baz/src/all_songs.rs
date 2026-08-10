//! **All songs** — the implicit playlist the vocabulary already had a name
//! for, given a type.
//!
//! ADR-0006 layer 1: pure, iced-free, unit-tested. Albums and the wall's own
//! filter in; a name, two counts, a queue and four sleeve quotations out. No
//! window, no engine, no disk — and *no file*, which is the whole point.
//!
//! # What it is
//!
//! The owner, 2026-08-09: *"The play all thing also does not need to exist.
//! That should be existing as a kind of playlist that is implicit."*
//!
//! `docs/design/09-implicit-playlists.md` §2 had already listed *"the wall, in
//! its arrangement"* as an implicit playlist, and §2's own model line is
//! *"baz has one kind of list. One of them is sounding and has no name; the
//! rest are named and silent."* So the vocabulary existed and the type did
//! not: `grep -rn "implicit playlist" crates/` returned one comment.
//! [`AllSongs`] is that type — a list you can see and play, with nobody's file
//! behind it.
//!
//! # What it is ordered by, stated plainly
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
//! held seven of twelve hundred records would be lying, so [`AllSongs::counts`]
//! says which case it is in — the plain figures at rest, and `7 of 1284
//! records` while the wall is filtered. The honesty is in the readout rather
//! than in a second name, because the list *is* the wall and the wall is one
//! thing.
//!
//! # It is playable and viewable, never a destination
//!
//! There is no file to append to, so **the picker must never offer
//! `Add to "All songs"`**. That is structural rather than remembered: the
//! picker's rows are [`crate::playlists::PanelRow`]s read out of the playlists
//! folder, and this type is not one and cannot become one — it has no id, no
//! path and no `save`. `crate::menu`'s own sweep asserts it anyway, because
//! "structurally impossible" is what the last surface that grew a destination
//! looked like from the inside too.
//!
//! # Where you look at it
//!
//! **The wall**, and there is deliberately no second page. Doc 09 §2 names the
//! wall itself as where this list is seen, and doc 07 L8.6 refuses one fact
//! drawn twice; a page listing the same music as text would be the same
//! collection drawn worse, without the art, and would need its own virtual
//! window, its own scroll memory and its own search to catch up with the
//! surface baz opens onto. The panel's row is the *handle* — name, counts,
//! sleeve, and a press that takes you to the wall.

use crate::vm::{AlbumVm, EditionKey, QueueVm, format_duration, stacked_queue};

/// The list's name, in the listener's language.
///
/// The owner's own words, kept: *"the 'all songs' should be an implicit
/// playlist"*. Sentence case like every other name in the product, and not
/// *Everything* — which the earlier mapping used as a placeholder and which
/// reads as a claim about the library rather than a name for a list.
pub(crate) const NAME: &str = "All songs";

/// How many records the sleeve quotes, and the reason is
/// [`crate::playlists::PanelRow::art`]'s: four for the 2 × 2 collage.
const QUOTED: usize = 4;

/// **The implicit playlist**, resolved from the wall exactly as the wall
/// resolves itself.
///
/// Built per frame from the same three things the wall reads, and holding no
/// state of its own — which is what makes *"a view of a live thing"* a fact
/// about the type rather than a promise about how it is used. There is nowhere
/// here to cache an order, so there is no order that can go stale.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AllSongs {
    /// What `Play` sends, and what the counts were taken from — one value for
    /// both, so the paths sent and the figures shown cannot describe different
    /// music ([`QueueVm`]'s own rule).
    pub(crate) queue: QueueVm,
    /// The sleeve's quotations: the first [`QUOTED`] records the wall shows,
    /// in wall order. Fewer means "draw the first full-bleed", none means the
    /// rest tile — [`crate::views::playlist_sleeve`]'s rule, unchanged, because
    /// an implicit list is a list and gets a list's sleeve.
    pub(crate) art: Vec<u64>,
    /// How many records the wall is showing.
    pub(crate) records: usize,
    /// How many records the library holds, whether or not the wall is showing
    /// them — the denominator the honesty line needs.
    pub(crate) held: usize,
}

impl AllSongs {
    /// Resolve the list from the wall: `albums` in the active group key's
    /// order, `visible` the filter's own index list, `chosen` the edition
    /// picked per record.
    ///
    /// The same three inputs, in the same order, that decide what is on screen
    /// — and there is deliberately nowhere to put a fourth. A future `MOOD`
    /// group key, or a mood spelled as a query, arrives here as a different
    /// `albums`/`visible` pair and needs not one line of new code.
    pub(crate) fn from_wall(
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
        Self {
            records: picks.len(),
            art,
            queue: stacked_queue(&picks),
            held: albums.len(),
        }
    }

    /// Whether the list is empty — an empty library, or a query that matched
    /// no record. Playing it does nothing and claims nothing, which is the
    /// rule every play gesture in baz keeps.
    pub(crate) fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Whether the wall's filter is narrowing the list.
    ///
    /// A comparison rather than a copy of the query, because what matters is
    /// not *whether somebody typed* but whether the list on screen is the whole
    /// library. A query that matches everything narrows nothing, and the
    /// readout should not claim it did.
    pub(crate) fn filtered(&self) -> bool {
        self.records < self.held
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
        let head = if self.filtered() {
            format!("{} of {} records", self.records, self.held)
        } else {
            format!("{} records", self.records)
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

    fn of(albums: &[AlbumVm], visible: &[usize]) -> AllSongs {
        AllSongs::from_wall(albums, visible, |_| None)
    }

    fn all(albums: &[AlbumVm]) -> AllSongs {
        let visible: Vec<usize> = (0..albums.len()).collect();
        of(albums, &visible)
    }

    /// **The list is the wall, in the wall's own order** — the property that
    /// makes it the implicit playlist doc 09 §2 already named, rather than a
    /// second collection with opinions of its own.
    #[test]
    fn the_list_is_the_wall_in_the_walls_own_order() {
        let albums = wall();
        let titles = |list: &AllSongs| -> Vec<String> {
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
        assert_eq!(all(&albums).queue.provenance, None);
        assert_eq!(of(&albums, &[1, 2]).queue.provenance, None);
        assert_eq!(of(&albums, &[]).queue.provenance, None);
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
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/all_songs.rs"),
        )
        .expect("this module's own source")
        .replace("\r\n", "\n");
        let start = source
            .find("pub(crate) struct AllSongs {")
            .expect("the type exists");
        let rest = &source[start..];
        let body = &rest[..rest.find("\n}\n").expect("the struct ends")];
        for forbidden in ["PathBuf", "path:", "id:", "fn save"] {
            assert!(
                !body.contains(forbidden),
                "`AllSongs` grew `{forbidden}` — an implicit list is playable and \
                 viewable, never a destination (module docs, doc 09 §2)"
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
