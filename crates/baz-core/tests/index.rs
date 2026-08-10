//! Integration tests for `baz_core::index`: persistence roundtrip through a
//! real database file, incremental adds mid-scan, search semantics, and
//! album grouping — all through the public `Library` API.

use std::path::{Path, PathBuf};
use std::time::Duration;

use baz_core::history::{History, HistoryLedger, PlayRecord, Recency};
use baz_core::index::{AlbumArtist, GroupHeader, GroupKey, IndexError, Initial, Library};
use baz_core::library::{AudioFormat, FileStamp, TrackMeta};
use baz_core::replaygain::{ComputedReplayGain, ReplayGainTags};

/// A fully-`None` track except for its path — the shape a tagless file with
/// an uninformative folder layout produces.
fn bare(path: &str) -> TrackMeta {
    TrackMeta {
        path: PathBuf::from(path),
        artist: None,
        album_artist: None,
        compilation: None,
        genre: None,
        album: None,
        title: None,
        track: None,
        disc: None,
        year: None,
        duration: None,
        format: None,
        bit_depth: None,
        sample_rate: None,
        bitrate: None,
        stamp: None,
        replay_gain: ReplayGainTags::default(),
    }
}

fn track(path: &str, artist: &str, album: &str, title: &str, number: u32) -> TrackMeta {
    TrackMeta {
        track: Some(number),
        artist: Some(artist.to_owned()),
        album: Some(album.to_owned()),
        title: Some(title.to_owned()),
        ..bare(path)
    }
}

/// A track in a specific codec, as a real scan would produce it.
fn encoded(
    path: &str,
    album: &str,
    title: &str,
    number: u32,
    format: AudioFormat,
    bitrate: u32,
) -> TrackMeta {
    TrackMeta {
        format: Some(format),
        bitrate: Some(bitrate),
        bit_depth: format.is_lossless().then_some(16),
        sample_rate: Some(44_100),
        ..track(path, "Stan Rogers", album, title, number)
    }
}

fn titles(results: &[&TrackMeta]) -> Vec<String> {
    results
        .iter()
        .map(|meta| meta.title.clone().unwrap_or_default())
        .collect()
}

#[test]
fn persistence_roundtrips_through_reopen() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("library.db");

    let mut originals = vec![
        // Unicode-heavy fields.
        TrackMeta {
            year: Some(1999),
            disc: Some(2),
            duration: Some(Duration::new(215, 123_456_789)),
            ..track("/m/g/1.flac", "Größenwahn", "北京 Nights", "Ærø — 序曲", 1)
        },
        // None-heavy: nothing known but the path.
        bare("/m/strays/mystery.mp3"),
    ];
    // A genuinely non-UTF-8 path, representable on Unix.
    #[cfg(unix)]
    originals.push({
        use std::os::unix::ffi::OsStringExt;
        let raw = std::ffi::OsString::from_vec(b"/m/br\xF6\xFEken/tr\xFFack.flac".to_vec());
        let path = PathBuf::from(raw);
        assert!(path.to_str().is_none(), "fixture must not be valid UTF-8");
        TrackMeta {
            path,
            ..track("", "Latin-1 Survivor", "Mojibake", "Träck", 3)
        }
    });

    {
        let mut library = Library::open(&db).expect("first open");
        let added = library.add_tracks(originals.clone()).expect("add");
        assert_eq!(added, originals.len());
    } // dropped: everything below must come from the file

    let reopened = Library::open(&db).expect("reopen");
    assert_eq!(reopened.len(), originals.len());
    let mut hydrated: Vec<TrackMeta> = reopened.tracks().cloned().collect();
    hydrated.sort_by(|a, b| a.path.cmp(&b.path));
    originals.sort_by(|a, b| a.path.cmp(&b.path));
    assert_eq!(hydrated, originals);
}

#[test]
fn incremental_add_mid_scan_is_searchable_immediately() {
    let mut library = Library::open_in_memory().expect("open");

    library
        .add_tracks(vec![track("/m/1.flac", "Alpha", "First", "One", 1)])
        .expect("first batch");
    assert_eq!(library.search("alpha", 10).len(), 1);
    assert!(library.search("beta", 10).is_empty());

    // A later batch from the same still-running scan.
    library
        .add_tracks(vec![track("/m/2.flac", "Beta", "Second", "Two", 1)])
        .expect("second batch");
    assert_eq!(library.search("beta", 10).len(), 1);
    assert_eq!(library.len(), 2);

    // A rescan of an already-known path updates rather than duplicates.
    library
        .add_tracks(vec![track(
            "/m/2.flac",
            "Beta",
            "Second",
            "Two (Remaster)",
            1,
        )])
        .expect("rescan batch");
    assert_eq!(library.len(), 2);
    assert_eq!(library.search("remaster", 10).len(), 1);
}

#[test]
fn search_is_case_insensitive_across_scripts() {
    let mut library = Library::open_in_memory().expect("open");
    library
        .add_tracks(vec![
            track("/m/1.flac", "Größenwahn", "Debüt", "Lied", 1),
            track("/m/2.flac", "東京事変", "大人", "秘密", 1),
            track("/m/3.flac", "MOTÖRHEAD", "Overkill", "Overkill", 1),
        ])
        .expect("add");

    // German sharp s: folded query matches mixed-case artist.
    assert_eq!(library.search("größenwahn", 10).len(), 1);
    assert_eq!(library.search("GRÖßENWAHN", 10).len(), 1);
    // CJK has no case; exact and substring both hit.
    assert_eq!(library.search("東京事変", 10).len(), 1);
    assert_eq!(library.search("京事", 10).len(), 1);
    // Folding applies to the stored side too.
    assert_eq!(library.search("motörhead", 10).len(), 1);
    // Substring means substring: no cross-field or fuzzy matches.
    assert!(library.search("größenwahn debüt", 10).is_empty());
}

#[test]
fn search_caps_at_limit_in_deterministic_order() {
    let mut library = Library::open_in_memory().expect("open");
    // Inserted deliberately out of order.
    library
        .add_tracks(vec![
            track("/m/c.flac", "Carol", "Common", "Gamma", 1),
            track("/m/a.flac", "Alice", "Common", "Alpha", 1),
            track("/m/b.flac", "Bob", "Common", "Beta", 1),
        ])
        .expect("add");

    // All match "common"; order is artist-first regardless of insertion.
    let all = library.search("common", 10);
    assert_eq!(titles(&all), ["Alpha", "Beta", "Gamma"]);

    let capped = library.search("common", 2);
    assert_eq!(titles(&capped), ["Alpha", "Beta"]);
    assert!(library.search("common", 0).is_empty());

    // Same query, same answer.
    assert_eq!(library.search("common", 10), all);
}

#[test]
fn track_matching_in_several_fields_is_returned_once() {
    let mut library = Library::open_in_memory().expect("open");
    library
        .add_tracks(vec![track(
            "/m/1.flac",
            "Echo",
            "Echo Park",
            "Echoes of Echoes",
            1,
        )])
        .expect("add");
    assert_eq!(library.search("echo", 10).len(), 1);
}

#[test]
fn empty_query_returns_nothing() {
    let mut library = Library::open_in_memory().expect("open");
    library
        .add_tracks(vec![track("/m/1.flac", "Anyone", "Anything", "Any", 1)])
        .expect("add");
    // Every haystack contains ""; returning "everything, truncated" would
    // misrepresent the library, so the documented behavior is no results.
    assert!(library.search("", 10).is_empty());
}

// ---------------------------------------------------------------------------
// Search ranking (`docs/adr/0021-search-ranking.md`).
//
// The model as a table of cases. The wall's find is type-anywhere and `Enter`
// plays the first match (`docs/design/critique/02-surfaces.md`), so what ranks
// first is what plays — every one of these is a claim about what would come out
// of the speakers.
// ---------------------------------------------------------------------------

/// A library holding exactly `tracks`, in the order given.
fn library_of(tracks: Vec<TrackMeta>) -> Library {
    let mut library = Library::open_in_memory().expect("open");
    library.add_tracks(tracks).expect("add");
    library
}

/// The album titles a ranked album search returns, in rank order.
fn album_titles(albums: &[baz_core::index::Album<'_>]) -> Vec<String> {
    albums
        .iter()
        .map(|album| album.title.unwrap_or_default().to_owned())
        .collect()
}

#[test]
fn a_match_ranks_by_position_first_and_completeness_second() {
    // One track per album so nothing but the match's own fit is in play, and
    // inserted in an order that is neither the corpus order nor the answer.
    let library = library_of(vec![
        track("/m/1.flac", "Artist 1", "Album 1", "Skid Row", 1),
        track("/m/2.flac", "Artist 2", "Album 2", "The Kidz", 1),
        track("/m/3.flac", "Artist 3", "Album 3", "Kids", 1),
        track("/m/4.flac", "Artist 4", "Album 4", "Kid", 1),
        track("/m/5.flac", "Artist 5", "Album 5", "The Kid", 1),
        track("/m/6.flac", "Artist 6", "Album 6", "Kid A", 1),
    ]);

    // `Kid` is the whole field; `Kid A` starts it and ends a word; `Kids`
    // starts it inside a word; `The Kid` starts a later word and ends one;
    // `The Kidz` starts a later word inside it; `Skid Row` starts inside one.
    assert_eq!(
        titles(&library.search("kid", 10)),
        ["Kid", "Kid A", "Kids", "The Kid", "The Kidz", "Skid Row"],
        "position before completeness, under one exact tier that is both"
    );
}

#[test]
fn an_equally_good_match_ranks_artist_then_album_then_title() {
    // The same query is the *entire* value of a different field in each of
    // three records, so only the field can separate them.
    let library = library_of(vec![
        track("/m/1.flac", "Yolanda", "Green", "Delta", 1),
        track("/m/2.flac", "Xavier", "Delta", "Two", 1),
        track("/m/3.flac", "Delta", "Blue", "One", 1),
    ]);

    assert_eq!(
        titles(&library.search("delta", 10)),
        ["One", "Two", "Delta"],
        "at equal fit: who made it, then what record, then which song"
    );
}

#[test]
fn an_exact_title_outranks_an_artist_that_merely_contains_the_query() {
    // This is why the field is the *second* signal and not the first. Ranking
    // by field first would play a Yesterdays New Quintet track for `yesterday`
    // — an artist match, but a worse one than the song actually called that.
    let library = library_of(vec![
        track(
            "/m/1.flac",
            "Yesterdays New Quintet",
            "Angles Without Edges",
            "Sun Song",
            1,
        ),
        track("/m/2.flac", "The Beatles", "Help!", "Yesterday", 13),
    ]);

    assert_eq!(
        titles(&library.search("yesterday", 10)),
        ["Yesterday", "Sun Song"]
    );
}

#[test]
fn the_first_match_is_the_best_match_however_late_it_sits_in_the_corpus() {
    // The bug this ranking exists to fix. Corpus order puts the Aardvarks
    // first; `Enter` must still play Kid A.
    let library = library_of(vec![
        track(
            "/m/1.flac",
            "Aardvark Collective",
            "Playground",
            "Kids Everywhere",
            1,
        ),
        track(
            "/m/2.flac",
            "Radiohead",
            "Kid A",
            "Everything In Its Right Place",
            1,
        ),
    ]);

    assert_eq!(
        titles(&library.search("kid", 10)),
        ["Everything In Its Right Place", "Kids Everywhere"]
    );
    // What `Enter` plays, which is the whole point.
    assert_eq!(
        titles(&library.search("kid", 1)),
        ["Everything In Its Right Place"],
        "the limit takes the top of the ranking, not the top of the corpus"
    );
    assert_eq!(
        album_titles(&library.search_albums("kid", 10)),
        ["Kid A", "Playground"]
    );
}

#[test]
fn an_albums_matching_tracks_stay_together_under_its_best_one() {
    // Ranked purely track by track the answer would interleave: Moon (exact),
    // Moonchild (prefix), Half Moonlight (mid-word). The wall draws albums, so
    // a record's hits stay under the record.
    let library = library_of(vec![
        track("/m/1.flac", "Aa", "Dark Side", "Moon", 1),
        track("/m/2.flac", "Aa", "Dark Side", "Half Moonlight", 2),
        track("/m/3.flac", "Bb", "Harvest", "Moonchild", 1),
    ]);

    assert_eq!(
        titles(&library.search("moon", 10)),
        ["Moon", "Half Moonlight", "Moonchild"]
    );
}

#[test]
fn more_matching_tracks_never_outrank_one_better_match() {
    // No count bonus: a long compilation would win every query it brushed
    // against, for a reason the query never asked about.
    let mut tracks = vec![track("/m/z.flac", "Zz", "Single", "Halo", 1)];
    for number in 1..=5 {
        tracks.push(track(
            &format!("/m/a{number}.flac"),
            "Aa",
            "Compilation",
            &format!("Bright Halo {number}"),
            number,
        ));
    }
    let library = library_of(tracks);

    assert_eq!(
        album_titles(&library.search_albums("halo", 10)),
        ["Single", "Compilation"],
        "one exact hit beats five word-boundary hits"
    );
    assert_eq!(titles(&library.search("halo", 1)), ["Halo"]);
}

#[test]
fn ties_break_on_library_order_and_nothing_else() {
    // Every track matches the album title identically, so only the third
    // signal is left. It is library order — which is what the pre-ranking
    // contract promised and what the determinism tests have always pinned.
    let library = library_of(vec![
        track("/m/c.flac", "Carol", "Common", "Gamma", 1),
        track("/m/a.flac", "Alice", "Common", "Alpha", 1),
        track("/m/b.flac", "Bob", "Common", "Beta", 1),
    ]);

    assert_eq!(
        titles(&library.search("common", 10)),
        ["Alpha", "Beta", "Gamma"]
    );
}

#[test]
fn ranking_is_deterministic_and_independent_of_insertion_order() {
    let tracks = || {
        vec![
            track("/m/1.flac", "Aardvark Collective", "Playground", "Kids", 1),
            track("/m/2.flac", "Radiohead", "Kid A", "Idioteque", 8),
            track("/m/3.flac", "Radiohead", "Kid A", "The National Anthem", 3),
            track(
                "/m/4.flac",
                "Kid Koala",
                "Carpal Tunnel Syndrome",
                "Fender Bender",
                4,
            ),
            track("/m/5.flac", "Zeta", "Skid Marks", "Kid Gloves", 2),
        ]
    };
    let forward = library_of(tracks());
    let mut reversed = tracks();
    reversed.reverse();
    let backward = library_of(reversed);
    // And one built a batch at a time, as a scan in progress would.
    let mut piecemeal = Library::open_in_memory().expect("open");
    for one in tracks() {
        piecemeal.add_tracks(vec![one]).expect("add");
    }

    let expected = titles(&forward.search("kid", 10));
    assert_eq!(
        expected,
        // Kid Koala's artist starts with the word, then Kid A's album is the
        // same fit one field down, then `Kid Gloves` the same fit one field
        // further; `Kids` only manages a mid-word prefix. Nothing here is
        // decided by where the track sits in the corpus.
        [
            "Fender Bender",
            "The National Anthem",
            "Idioteque",
            "Kid Gloves",
            "Kids"
        ]
    );
    assert_eq!(titles(&backward.search("kid", 10)), expected);
    assert_eq!(titles(&piecemeal.search("kid", 10)), expected);
    // Same query, same answer, every time.
    for _ in 0..4 {
        assert_eq!(titles(&forward.search("kid", 10)), expected);
    }
    assert_eq!(
        album_titles(&forward.search_albums("kid", 10)),
        album_titles(&backward.search_albums("kid", 10))
    );
}

#[test]
fn search_albums_lists_each_matching_album_once_in_rank_order() {
    let library = library_of(vec![
        track("/m/1.flac", "Aa", "Ghost Signals", "One", 1),
        track("/m/2.flac", "Aa", "Ghost Signals", "Two", 2),
        track("/m/3.flac", "Aa", "Ghost Signals", "Three", 3),
        track("/m/4.flac", "Bb", "Nightfall", "Signal Fire", 1),
        track("/m/5.flac", "Signal Hill", "Debut", "Anything", 1),
    ]);

    let albums = library.search_albums("signal", 10);
    assert_eq!(
        album_titles(&albums),
        // The artist match first, then the title that starts with the word,
        // then the album that carries it mid-title.
        ["Debut", "Nightfall", "Ghost Signals"]
    );
    // Three tracks matched in `Ghost Signals`; the album appears once, and
    // carries its whole track list rather than only the matches.
    let ghosts = albums.last().expect("three albums");
    assert_eq!(
        ghosts
            .editions
            .iter()
            .map(|edition| edition.tracks.len())
            .sum::<usize>(),
        3
    );
    assert!(library.search_albums("signal", 0).is_empty());
    assert!(library.search_albums("", 10).is_empty());
    assert!(library.search_albums("ghost\nsignals", 10).is_empty());
}

#[test]
fn search_albums_does_not_lose_an_album_to_a_track_cap() {
    // Why `search_albums` exists. A front end that searches *tracks* and folds
    // them onto albums applies a track cap to an album question: this record's
    // sixty matching tracks fill any reasonable cap by themselves, and the
    // second album vanishes from the wall.
    let mut tracks: Vec<TrackMeta> = (1..=60)
        .map(|number| {
            track(
                &format!("/m/a{number:03}.flac"),
                "Aa",
                "Common Ground",
                &format!("Track {number}"),
                number,
            )
        })
        .collect();
    tracks.push(track("/m/z.flac", "Zz", "Elsewhere", "Common Time", 1));
    let library = library_of(tracks);

    let folded: Vec<String> = library
        .search("common", 50)
        .iter()
        .filter_map(|meta| meta.album.clone())
        .collect();
    assert!(
        !folded.contains(&"Elsewhere".to_owned()),
        "the track cap really does hide the second album"
    );
    assert_eq!(
        album_titles(&library.search_albums("common", 50)),
        ["Common Ground", "Elsewhere"],
        "the album search caps the answer, not the working set"
    );
}

#[test]
fn ranking_holds_across_scripts_and_folds_case() {
    let library = library_of(vec![
        track("/m/1.flac", "北風", "東京事変の秘密", "序曲", 1),
        track("/m/2.flac", "東京事変", "大人", "秘密", 1),
        track("/m/3.flac", "Größenwahn", "Debüt", "Lied", 1),
        track("/m/4.flac", "Ein Größenwahnsinn", "Zweit", "Lied Zwei", 1),
    ]);

    // Exact artist beats the album that merely starts with the same string.
    assert_eq!(titles(&library.search("東京事変", 10)), ["秘密", "序曲"]);
    // A substring of a script with no word boundaries is a fragment in both,
    // so the field decides — the artist's name over the album's — and both are
    // still found.
    assert_eq!(titles(&library.search("京事", 10)), ["秘密", "序曲"]);
    // Folding is Unicode-aware on both sides, and the exact artist outranks
    // the one whose name only starts with the query.
    assert_eq!(
        titles(&library.search("GRÖSSENWAHN", 10)),
        Vec::<String>::new()
    );
    assert_eq!(
        titles(&library.search("größenwahn", 10)),
        ["Lied", "Lied Zwei"]
    );
    assert_eq!(
        titles(&library.search("GRÖßENWAHN", 10)),
        ["Lied", "Lied Zwei"]
    );
}

#[test]
fn pathological_queries_are_answered_rather_than_survived() {
    let library = library_of(vec![
        track("/m/1.flac", "Alpha", "Anything", "A", 1),
        track("/m/2.flac", "Beta", "Bee", "Ábaco", 1),
        bare("/m/3.flac"),
    ]);

    // The corpus separator can never be searched for: it is what keeps a match
    // from spanning two fields, so a query containing it could only ask for a
    // cross-field match.
    assert!(library.search("\n", 10).is_empty());
    assert!(library.search("alpha\nanything", 10).is_empty());
    // No query is not a query.
    assert!(library.search("", 10).is_empty());
    assert!(library.search("anything", 0).is_empty());
    // A query matching nothing.
    assert!(library.search("zyzzyva", 10).is_empty());
    // A query matching everything, ranked and capped: the track *called* `a`
    // is the exact one and comes first.
    let everything = library.search("a", 10);
    assert_eq!(
        everything.len(),
        2,
        "the tagless stray has no haystack at all"
    );
    assert_eq!(titles(&everything)[0], "A");
    // A query longer than any field, one that is only punctuation, and one
    // that is only whitespace.
    assert!(library.search(&"a".repeat(4096), 10).is_empty());
    assert!(library.search("!?", 10).is_empty());
    assert!(library.search(" ", 10).is_empty());
    // The tagless stray has no haystack to match and never appears.
    assert!(
        library
            .search("a", 10)
            .iter()
            .all(|meta| meta.path != Path::new("/m/3.flac"))
    );
}

#[test]
fn ranking_examines_a_capped_prefix_of_library_order_and_says_so() {
    // The documented limit (`Library::RANKED_CANDIDATES`), asserted rather
    // than described: past the cap a better match is genuinely not seen.
    let mut crowded: Vec<TrackMeta> = (0..Library::RANKED_CANDIDATES + 100)
        .map(|number| {
            track(
                &format!("/m/a{number:05}.flac"),
                &format!("Aa {number:05}"),
                "Fragments",
                "Bazooka",
                1,
            )
        })
        .collect();
    crowded.push(track("/m/z.flac", "Zz", "Late", "Zoo", 1));
    let library = library_of(crowded);

    let found = library.search("zoo", 5);
    assert_eq!(found.len(), 5);
    assert!(
        titles(&found).iter().all(|title| title == "Bazooka"),
        "the exact match past the cap is not seen — the honest cost of a \
         bounded candidate set, and the reason the cap is large"
    );

    // Under the cap — which is every query specific enough for `Enter` to
    // mean anything — the ranking is exact.
    let mut modest: Vec<TrackMeta> = (0..100)
        .map(|number| {
            track(
                &format!("/m/a{number:05}.flac"),
                &format!("Aa {number:05}"),
                "Fragments",
                "Bazooka",
                1,
            )
        })
        .collect();
    modest.push(track("/m/z.flac", "Zz", "Late", "Zoo", 1));
    let library = library_of(modest);
    assert_eq!(titles(&library.search("zoo", 1)), ["Zoo"]);
}

#[test]
fn albums_order_tracks_by_disc_then_number_then_title() {
    let mut library = Library::open_in_memory().expect("open");
    let disc_track = |path: &str, disc: u32, number: u32, title: &str| TrackMeta {
        disc: Some(disc),
        ..track(path, "The Band", "Live Set", title, number)
    };
    library
        .add_tracks(vec![
            disc_track("/m/d2t1.flac", 2, 1, "Encore"),
            disc_track("/m/d1t2.flac", 1, 2, "Middle"),
            disc_track("/m/d1t1.flac", 1, 1, "Opener"),
        ])
        .expect("add");

    let albums = library.albums();
    assert_eq!(albums.len(), 1);
    let album = &albums[0];
    assert_eq!(album.artist, AlbumArtist::Named("The Band"));
    assert_eq!(album.title, Some("Live Set"));
    // One format in, one edition out — and no selector in the UI.
    assert_eq!(album.editions.len(), 1);
    let edition = album.default_edition().expect("an album has an edition");
    assert_eq!(edition.tracks.len(), 3);
    let order: Vec<_> = edition
        .tracks
        .iter()
        .map(|meta| (meta.disc, meta.track))
        .collect();
    assert_eq!(
        order,
        [(Some(1), Some(1)), (Some(1), Some(2)), (Some(2), Some(1))]
    );
}

#[test]
fn same_album_title_by_different_artists_stays_separate() {
    let mut library = Library::open_in_memory().expect("open");
    library
        .add_tracks(vec![
            track("/m/b1.flac", "Beta", "Greatest Hits", "B-Side", 1),
            track("/m/a1.flac", "Alpha", "Greatest Hits", "A-Side", 1),
        ])
        .expect("add");

    let albums = library.albums();
    assert_eq!(albums.len(), 2);
    // Sorted by artist; identical titles do not merge across artists.
    assert_eq!(albums[0].artist, AlbumArtist::Named("Alpha"));
    assert_eq!(albums[1].artist, AlbumArtist::Named("Beta"));
    assert!(albums.iter().all(|a| a.title == Some("Greatest Hits")));
}

#[test]
fn unknown_artist_and_album_tracks_group_together_first() {
    let mut library = Library::open_in_memory().expect("open");
    library
        .add_tracks(vec![
            track("/m/known.flac", "Zeta", "Zenith", "Known", 1),
            bare("/m/stray1.mp3"),
            bare("/m/stray2.mp3"),
        ])
        .expect("add");

    let albums = library.albums();
    assert_eq!(albums.len(), 2);
    // Documented behavior: unknowns share one shelf entry and sort first.
    assert_eq!(albums[0].artist, AlbumArtist::Unknown);
    assert_eq!(albums[0].title, None);
    assert_eq!(albums[0].editions.len(), 1);
    assert_eq!(albums[0].editions[0].tracks.len(), 2);
    assert_eq!(albums[1].artist, AlbumArtist::Named("Zeta"));
}

#[test]
fn album_year_comes_from_first_track_that_declares_one() {
    let mut library = Library::open_in_memory().expect("open");
    library
        .add_tracks(vec![
            track("/m/1.flac", "Solo", "Year Test", "First", 1),
            TrackMeta {
                year: Some(1974),
                ..track("/m/2.flac", "Solo", "Year Test", "Second", 2)
            },
        ])
        .expect("add");
    assert_eq!(library.albums()[0].year, Some(1974));
}

// ---------------------------------------------------------------------------
// Multi-disc sets (docs/adr/0038-the-record-and-its-discs.md)
//
// The four shapes a two-disc rip actually arrives in, built as real tagged
// files by `docs/design/impl/multi-disc/mkfixture.sh` and asserted here at the
// grouping layer the fixture exercises.
// ---------------------------------------------------------------------------

/// One record's worth of tracks: `(disc, track)` pairs under one album title.
fn disc_set(artist: &str, album: &str, tracks: &[(Option<u32>, u32)]) -> Vec<TrackMeta> {
    tracks
        .iter()
        .map(|&(disc, number)| {
            let disc_part = disc.map_or_else(|| "x".to_owned(), |d| d.to_string());
            TrackMeta {
                disc,
                ..track(
                    &format!("/m/{artist}/{album}/{disc_part}-{number}.flac"),
                    artist,
                    album,
                    &format!("{album} d{disc_part} t{number}"),
                    number,
                )
            }
        })
        .collect()
}

/// Every `(disc, track)` of an album's default edition, in the order the page
/// and the queue would take them.
fn disc_order(album: &baz_core::index::Album<'_>) -> Vec<(Option<u32>, Option<u32>)> {
    album
        .default_edition()
        .expect("an album has an edition")
        .tracks
        .iter()
        .map(|meta| (baz_core::index::disc_of(meta), meta.track))
        .collect()
}

/// **Shape 1 and shape 2**: one `ALBUM` tag, `DISCNUMBER` 1 and 2 — whether
/// the files sit in one folder or in `Disc 1/` and `Disc 2/`.
///
/// This already worked before the disc-marker rule existed and must keep
/// working: the grouping key is (album artist, album title) and reads no path
/// at all, so the folder split is not a fact the shelf can even see.
#[test]
fn one_album_tag_with_disc_numbers_is_one_record_however_it_is_foldered() {
    for (name, one, two) in [
        ("one folder", "Sign o' the Times", "Sign o' the Times"),
        (
            "two folders",
            "Sign o' the Times/Disc 1",
            "Sign o' the Times/Disc 2",
        ),
    ] {
        let mut tracks = Vec::new();
        for (folder, disc) in [(one, 1u32), (two, 2u32)] {
            for number in 1..=3 {
                tracks.push(TrackMeta {
                    disc: Some(disc),
                    ..track(
                        &format!("/m/Prince/{folder}/{disc}-{number}.flac"),
                        "Prince",
                        "Sign o' the Times",
                        &format!("d{disc} t{number}"),
                        number,
                    )
                });
            }
        }
        let library = library_of(tracks);
        let albums = library.albums();
        assert_eq!(albums.len(), 1, "{name}: one record");
        assert_eq!(albums[0].title, Some("Sign o' the Times"));
        assert_eq!(
            disc_order(&albums[0]),
            [
                (Some(1), Some(1)),
                (Some(1), Some(2)),
                (Some(1), Some(3)),
                (Some(2), Some(1)),
                (Some(2), Some(2)),
                (Some(2), Some(3)),
            ],
            "{name}: disc before track, or two track-ones interleave"
        );
    }
}

/// **Shape 3**, in the three spellings rips actually use. The disc lives in
/// the `ALBUM` tag itself, which before this rule shattered every such set
/// into two shelf entries.
#[test]
fn album_titles_differing_only_by_a_disc_marker_are_one_record() {
    for (artist, first, second, merged) in [
        (
            "Prince",
            "Sign o' the Times (Disc 1)",
            "Sign o' the Times (Disc 2)",
            "Sign o' the Times",
        ),
        (
            "Miles Davis",
            "Bitches Brew CD1",
            "Bitches Brew CD2",
            "Bitches Brew",
        ),
        (
            "The Clash",
            "Sandinista! [Disc 1]",
            "Sandinista! [Disc 2]",
            "Sandinista!",
        ),
    ] {
        let mut tracks = disc_set(artist, first, &[(None, 1), (None, 2)]);
        tracks.extend(disc_set(artist, second, &[(None, 1), (None, 2)]));
        let library = library_of(tracks);
        let albums = library.albums();
        assert_eq!(albums.len(), 1, "{first} + {second} are one record");
        assert_eq!(albums[0].title, Some(merged));
        // The marker also supplies the disc the tagger never wrote, so the
        // merged list plays 1·1, 1·2, 2·1, 2·2 rather than interleaving.
        assert_eq!(
            disc_order(&albums[0]),
            [
                (Some(1), Some(1)),
                (Some(1), Some(2)),
                (Some(2), Some(1)),
                (Some(2), Some(2)),
            ],
            "{merged}: the marker orders what it merged"
        );
    }
}

/// **The asymmetric rip**, and the one place the rule declines to fill in a
/// number: a tagger that marked the second disc and left the first alone.
///
/// The two spellings merge — that is the whole point of the sibling rule — and
/// the unmarked half sorts first, because an unknown disc sorts before a known
/// one and an unnumbered disc is exactly where that belongs. What it is *not*
/// given is the number 1: nothing in any file says so. The page counts it as a
/// disc (`vm::discs`) and draws no header above it, which is the honest
/// rendering of "these tracks, and then disc 2".
#[test]
fn an_unmarked_sibling_merges_and_is_not_told_which_disc_it_is() {
    let mut tracks = disc_set("Talk Talk", "Spirit of Eden", &[(None, 1), (None, 2)]);
    tracks.extend(disc_set(
        "Talk Talk",
        "Spirit of Eden - Disc 2",
        &[(None, 1), (None, 2)],
    ));
    let library = library_of(tracks);
    let albums = library.albums();
    assert_eq!(albums.len(), 1);
    assert_eq!(albums[0].title, Some("Spirit of Eden"));
    assert_eq!(
        disc_order(&albums[0]),
        [
            (None, Some(1)),
            (None, Some(2)),
            (Some(2), Some(1)),
            (Some(2), Some(2)),
        ],
        "the unnumbered disc leads, and stays unnumbered"
    );
}

/// **Shape 4**: no disc signal anywhere — no `DISCNUMBER`, no marker in the
/// title, just two folders whose track numbers collide.
///
/// These already merged, because they always shared an `ALBUM` tag, and they
/// still do. What they do *not* get is an invented disc: nothing in the files
/// says which folder is disc 1, and the folder names are not evidence baz
/// reads. The list interleaves 1, 1, 2, 2 and the page draws no disc breaks —
/// which is the honest rendering of a rip that did not say.
#[test]
fn two_folders_with_no_disc_signal_merge_with_nothing_to_order_by() {
    let mut tracks = Vec::new();
    for (folder, mark) in [("Disc 1", "a"), ("Disc 2", "b")] {
        for number in 1..=2 {
            tracks.push(track(
                &format!("/m/Prince/Sign o' the Times/{folder}/{number}.flac"),
                "Prince",
                "Sign o' the Times",
                &format!("{mark}{number}"),
                number,
            ));
        }
    }
    let library = library_of(tracks);
    let albums = library.albums();
    assert_eq!(albums.len(), 1);
    assert_eq!(albums[0].title, Some("Sign o' the Times"));
    assert_eq!(
        disc_order(&albums[0]),
        [
            (None, Some(1)),
            (None, Some(1)),
            (None, Some(2)),
            (None, Some(2)),
        ],
        "no disc is claimed, so track number is the only order there is"
    );
}

/// **The declined guess.** A listener who owns only disc 1 has a record
/// called `Bitches Brew CD1`, and baz leaves it called that: there is no
/// sibling, so nothing merges, so renaming it would be an invention that buys
/// nothing (ADR-0008's posture, ADR-0038 §3).
#[test]
fn a_disc_marker_with_no_sibling_is_left_alone() {
    let library = library_of(disc_set("Miles Davis", "Bitches Brew CD1", &[(None, 1)]));
    let albums = library.albums();
    assert_eq!(albums.len(), 1);
    assert_eq!(albums[0].title, Some("Bitches Brew CD1"));

    // A record by the same artist that merely *contains* the base is not a
    // sibling either — the base must match to the character.
    let mut tracks = disc_set("Miles Davis", "Bitches Brew CD1", &[(None, 1)]);
    tracks.extend(disc_set("Miles Davis", "Bitches Brew Live", &[(None, 1)]));
    let library = library_of(tracks);
    assert_eq!(
        album_titles(&library.albums()),
        ["Bitches Brew CD1", "Bitches Brew Live"]
    );

    // Nor is the same title under a different album artist.
    let mut tracks = disc_set("Miles Davis", "Bitches Brew CD1", &[(None, 1)]);
    tracks.extend(disc_set("Somebody Else", "Bitches Brew", &[(None, 1)]));
    let library = library_of(tracks);
    assert_eq!(library.albums().len(), 2);
    assert!(
        library
            .albums()
            .iter()
            .any(|album| album.title == Some("Bitches Brew CD1")),
        "the marked record keeps its name when the sibling is somebody else's"
    );
}

/// A `DISCNUMBER` tag outranks a marker in the title, always — the marker is
/// a fallback that fills a hole, exactly as folder inference is for artist and
/// album. A set whose titles say `CD1`/`CD2` and whose tags say 3 and 4 plays
/// in tag order.
#[test]
fn a_disc_tag_outranks_a_marker_in_the_title() {
    let mut tracks = disc_set("Miles Davis", "Bitches Brew CD1", &[(Some(4), 1)]);
    tracks.extend(disc_set("Miles Davis", "Bitches Brew CD2", &[(Some(3), 1)]));
    let library = library_of(tracks);
    let albums = library.albums();
    assert_eq!(albums.len(), 1);
    assert_eq!(albums[0].title, Some("Bitches Brew"));
    assert_eq!(
        disc_order(&albums[0]),
        [(Some(3), Some(1)), (Some(4), Some(1))]
    );
}

/// **Discs and editions are different axes** (ADR-0007 §"one album, several
/// codecs"; ADR-0038 §5). A two-disc set owned in FLAC *and* in MP3 is one
/// record, two editions, two discs in each — not four records, and not one
/// edition with every track twice.
#[test]
fn a_two_disc_set_in_two_codecs_is_one_record_with_two_editions_of_two_discs() {
    let mut tracks = Vec::new();
    for (format, ext, bitrate) in [
        (AudioFormat::Flac, "flac", 900),
        (AudioFormat::Mp3, "mp3", 320),
    ] {
        for disc in 1..=2u32 {
            for number in 1..=2u32 {
                tracks.push(TrackMeta {
                    format: Some(format),
                    bitrate: Some(bitrate),
                    bit_depth: format.is_lossless().then_some(16),
                    sample_rate: Some(44_100),
                    ..track(
                        &format!("/m/{ext}/Prince/Sign o' the Times CD{disc}/{number}.{ext}"),
                        "Prince",
                        &format!("Sign o' the Times CD{disc}"),
                        &format!("d{disc} t{number}"),
                        number,
                    )
                });
            }
        }
    }
    let library = library_of(tracks);
    let albums = library.albums();
    assert_eq!(albums.len(), 1, "one record");
    assert_eq!(albums[0].title, Some("Sign o' the Times"));
    assert_eq!(albums[0].editions.len(), 2, "two editions");
    // Lossless first, and each edition carries the whole set in disc order.
    assert_eq!(albums[0].editions[0].format, Some(AudioFormat::Flac));
    assert_eq!(albums[0].editions[1].format, Some(AudioFormat::Mp3));
    for edition in &albums[0].editions {
        let order: Vec<_> = edition
            .tracks
            .iter()
            .map(|meta| (baz_core::index::disc_of(meta), meta.track))
            .collect();
        assert_eq!(
            order,
            [
                (Some(1), Some(1)),
                (Some(1), Some(2)),
                (Some(2), Some(1)),
                (Some(2), Some(2)),
            ],
            "{:?} spans both discs, in order",
            edition.format
        );
    }
}

/// The library's answer for a *loose* track — what a search hit, a playlist
/// entry and the wall's tile identity are all derived from — is the record's
/// title, so the door a search result opens leads to the tile it named.
#[test]
fn a_tracks_record_title_agrees_with_the_shelf() {
    let mut tracks = disc_set("Prince", "Sign o' the Times (Disc 1)", &[(None, 1)]);
    tracks.extend(disc_set(
        "Prince",
        "Sign o' the Times (Disc 2)",
        &[(None, 1)],
    ));
    tracks.extend(disc_set("Miles Davis", "Bitches Brew CD1", &[(None, 1)]));
    let library = library_of(tracks);

    for meta in library.tracks() {
        let expected = if meta.artist.as_deref() == Some("Prince") {
            "Sign o' the Times"
        } else {
            "Bitches Brew CD1"
        };
        assert_eq!(
            library.record_title(meta),
            Some(expected),
            "{}",
            meta.path.display()
        );
    }
    // A path the library never held gets its tag back, verbatim.
    let stranger = track("/elsewhere/x.flac", "Prince", "Anything (Disc 2)", "x", 1);
    assert_eq!(
        library.record_title(&stranger),
        Some("Anything (Disc 2)"),
        "an unfiled track is told what its own tag says and nothing more"
    );
}

/// Searching still finds the record by what is **on disk** as well as by what
/// the shelf calls it: the corpus keeps the tag verbatim, so `disc 2` is a
/// query that works, and it returns the one merged record rather than two.
#[test]
fn a_merged_record_is_searchable_by_its_name_and_by_its_tag() {
    let mut tracks = disc_set("Prince", "Sign o' the Times (Disc 1)", &[(None, 1)]);
    tracks.extend(disc_set(
        "Prince",
        "Sign o' the Times (Disc 2)",
        &[(None, 1)],
    ));
    let library = library_of(tracks);

    assert_eq!(
        album_titles(&library.search_albums("sign o' the times", 10)),
        ["Sign o' the Times"]
    );
    assert_eq!(
        album_titles(&library.search_albums("(disc 2)", 10)),
        ["Sign o' the Times"],
        "the tag is still in the corpus; it just is not the record's name"
    );
}

/// The rule is a closed list, and this is the list — what it takes and, at
/// least as importantly, what it refuses.
#[test]
fn the_disc_marker_rule_is_narrow_and_says_where_it_stops() {
    use baz_core::index::split_disc_marker;

    for (title, base, disc) in [
        ("Sign o' the Times (Disc 1)", "Sign o' the Times", 1),
        ("Sign o' the Times [Disc 2]", "Sign o' the Times", 2),
        ("Sign o' the Times {Disc 2}", "Sign o' the Times", 2),
        ("Bitches Brew CD1", "Bitches Brew", 1),
        ("Bitches Brew cd 2", "Bitches Brew", 2),
        ("Bitches Brew - Disc 2", "Bitches Brew", 2),
        ("Bitches Brew, disk 2", "Bitches Brew", 2),
        ("Bitches Brew (CD 12)", "Bitches Brew", 12),
        ("Sandinista!  [Disc 2]  ", "Sandinista!", 2),
        ("Vol. 2 CD2", "Vol. 2", 2),
    ] {
        assert_eq!(
            split_disc_marker(title),
            (base, Some(disc)),
            "{title} carries a marker"
        );
    }

    for title in [
        // No number is no marker.
        "Compact Disc",
        "Bitches Brew CD",
        // Not one of the three words. No `part`, no `volume`, no `side`.
        "Sandinista! (Part 2)",
        "Physical Graffiti (Volume 1)",
        "Abbey Road (Side B 2)",
        // A number alone is a title, not a disc.
        "Sign o' the Times (2)",
        "Led Zeppelin II",
        "1999",
        // No boundary before the word.
        "Gamerip soundtrackcd2",
        // Not at the end.
        "Disc 2 of the Wall",
        // Nothing would be left.
        "CD 1",
        "(Disc 2)",
        // A bracket that does not close what it opened.
        "Bitches Brew [CD 1)",
        // Disc zero is not a disc, and three digits is not this rule's job.
        "Bitches Brew CD0",
        "Bitches Brew CD123",
    ] {
        assert_eq!(
            split_disc_marker(title),
            (title, None),
            "{title} carries no marker this rule will act on"
        );
    }
}

// ---------------------------------------------------------------------------
// Editions (docs/adr/0007-album-editions.md)
// ---------------------------------------------------------------------------

/// The owner's reported case: `FLAC/Stan Rogers/Northwest Passage/` and
/// `MP3/Stan Rogers/Northwest Passage/` are the same album, ripped twice.
fn northwest_passage_twice() -> Vec<TrackMeta> {
    let titles = ["Northwest Passage", "The Field Behind the Plow", "Lies"];
    let mut tracks = Vec::new();
    for (index, title) in titles.iter().enumerate() {
        let number = u32::try_from(index).expect("small") + 1;
        tracks.push(encoded(
            &format!("/m/FLAC/Stan Rogers/Northwest Passage/{number:02} {title}.flac"),
            "Northwest Passage",
            title,
            number,
            AudioFormat::Flac,
            900,
        ));
        tracks.push(encoded(
            &format!("/m/MP3/Stan Rogers/Northwest Passage/{number:02} {title}.mp3"),
            "Northwest Passage",
            title,
            number,
            AudioFormat::Mp3,
            320,
        ));
    }
    tracks
}

#[test]
fn one_album_in_two_formats_is_one_entry_with_two_editions() {
    let mut library = Library::open_in_memory().expect("open");
    library
        .add_tracks(northwest_passage_twice())
        .expect("add both rips");

    let albums = library.albums();
    assert_eq!(albums.len(), 1, "one shelf tile per album, not per format");
    let album = &albums[0];
    assert_eq!(album.editions.len(), 2);

    // Default is the lossless one, and it holds only its own tracks — no
    // interleaved duplicates.
    let default = album.default_edition().expect("a default edition");
    assert_eq!(default.format, Some(AudioFormat::Flac));
    assert!(default.is_lossless());
    assert_eq!(default.tracks.len(), 3);
    assert!(
        default
            .tracks
            .iter()
            .all(|t| t.format == Some(AudioFormat::Flac)),
        "an edition contains exactly one codec"
    );
    // In-album order survives the split.
    assert_eq!(
        default.tracks.iter().map(|t| t.track).collect::<Vec<_>>(),
        [Some(1), Some(2), Some(3)]
    );

    let lossy = album
        .edition(Some(AudioFormat::Mp3))
        .expect("the MP3 edition");
    assert_eq!(lossy.tracks.len(), 3);
    assert!(!lossy.is_lossless());
    assert_eq!(lossy.bitrate(), Some(320));
    assert_eq!(lossy.bit_depth(), None, "MP3 declares no sample width");
    assert_eq!(lossy.sample_rate(), Some(44_100));

    // Every track is in exactly one edition, and none is lost.
    let total: usize = album.editions.iter().map(|e| e.tracks.len()).sum();
    assert_eq!(total, library.len());
}

#[test]
fn edition_order_is_deterministic_regardless_of_insertion_order() {
    let forward = northwest_passage_twice();
    let mut backward = forward.clone();
    backward.reverse();

    let order = |tracks: Vec<TrackMeta>| {
        let mut library = Library::open_in_memory().expect("open");
        library.add_tracks(tracks).expect("add");
        library.albums()[0]
            .editions
            .iter()
            .map(|e| e.format)
            .collect::<Vec<_>>()
    };
    assert_eq!(order(forward.clone()), order(backward));
    assert_eq!(
        order(forward),
        [Some(AudioFormat::Flac), Some(AudioFormat::Mp3)]
    );
}

#[test]
fn lossless_beats_lossy_and_bitrate_only_breaks_ties_within_a_tier() {
    let mut library = Library::open_in_memory().expect("open");
    library
        .add_tracks(vec![
            // Three codecs, one track each: a hi-res FLAC, an ALAC, and a
            // low-bitrate MP3 that would win on nothing.
            TrackMeta {
                bit_depth: Some(24),
                sample_rate: Some(96_000),
                ..encoded("/m/a.flac", "Tiers", "Song", 1, AudioFormat::Flac, 2_400)
            },
            encoded("/m/a.m4a", "Tiers", "Song", 1, AudioFormat::Alac, 800),
            encoded("/m/a.mp3", "Tiers", "Song", 1, AudioFormat::Mp3, 128),
        ])
        .expect("add");

    let albums = library.albums();
    let formats: Vec<_> = albums[0].editions.iter().map(|e| e.format).collect();
    assert_eq!(
        formats,
        [
            Some(AudioFormat::Flac),
            Some(AudioFormat::Alac),
            Some(AudioFormat::Mp3),
        ],
        "both lossless editions outrank the lossy one; bitrate orders them"
    );
    let best = albums[0].default_edition().expect("default");
    assert_eq!(best.bit_depth(), Some(24));
    assert_eq!(best.sample_rate(), Some(96_000));
}

#[test]
fn a_partial_rip_does_not_mispair_with_the_complete_one() {
    let mut library = Library::open_in_memory().expect("open");
    let mut tracks = northwest_passage_twice();
    // The FLAC archive is complete; the MP3 copy stopped after track 1, and
    // has one extra track the FLAC rip never got.
    tracks.retain(|t| t.format != Some(AudioFormat::Mp3) || t.track == Some(1));
    tracks.push(encoded(
        "/m/MP3/Stan Rogers/Northwest Passage/09 Bonus.mp3",
        "Northwest Passage",
        "Bonus",
        9,
        AudioFormat::Mp3,
        320,
    ));
    library.add_tracks(tracks).expect("add");

    let albums = library.albums();
    assert_eq!(albums.len(), 1);
    let album = &albums[0];
    assert_eq!(album.editions.len(), 2);
    // Differing lengths: each edition keeps exactly its own tracks. Nothing
    // is paired by position, so nothing can be mis-paired.
    let flac = album.edition(Some(AudioFormat::Flac)).expect("flac");
    let mp3 = album.edition(Some(AudioFormat::Mp3)).expect("mp3");
    assert_eq!(flac.tracks.len(), 3);
    assert_eq!(mp3.tracks.len(), 2);
    assert_eq!(
        mp3.tracks.iter().map(|t| t.track).collect::<Vec<_>>(),
        [Some(1), Some(9)]
    );
    assert_eq!(
        album.default_edition().and_then(|e| e.format),
        Some(AudioFormat::Flac),
        "the complete lossless rip is still the default"
    );
}

#[test]
fn completeness_outranks_bitrate_inside_a_tier() {
    let mut library = Library::open_in_memory().expect("open");
    library
        .add_tracks(vec![
            // A one-track hi-res FLAC "edition" against a full 16/44 one:
            // the complete rip is the better default to hand a listener.
            TrackMeta {
                path: PathBuf::from("/m/hi/01.m4a"),
                bit_depth: Some(24),
                sample_rate: Some(96_000),
                ..encoded(
                    "/m/hi/01.m4a",
                    "Complete",
                    "One",
                    1,
                    AudioFormat::Alac,
                    2_400,
                )
            },
            encoded(
                "/m/lo/01.flac",
                "Complete",
                "One",
                1,
                AudioFormat::Flac,
                900,
            ),
            encoded(
                "/m/lo/02.flac",
                "Complete",
                "Two",
                2,
                AudioFormat::Flac,
                900,
            ),
        ])
        .expect("add");
    let albums = library.albums();
    assert_eq!(
        albums[0].default_edition().map(|e| e.format),
        Some(Some(AudioFormat::Flac)),
        "3 tracks beat 1, even at a third of the bitrate"
    );
}

#[test]
fn tracks_with_an_unknown_codec_form_their_own_last_ranked_edition() {
    let mut library = Library::open_in_memory().expect("open");
    library
        .add_tracks(vec![
            encoded("/m/a.mp3", "Mixed", "One", 1, AudioFormat::Mp3, 320),
            // A row a v1 upgrade could not backfill, not yet rescanned.
            track("/m/a.m4a", "Stan Rogers", "Mixed", "One", 1),
            encoded("/m/a.flac", "Mixed", "One", 1, AudioFormat::Flac, 900),
        ])
        .expect("add");
    let formats: Vec<_> = library.albums()[0]
        .editions
        .iter()
        .map(|e| e.format)
        .collect();
    assert_eq!(
        formats,
        [Some(AudioFormat::Flac), Some(AudioFormat::Mp3), None],
        "an unnamed codec is never assumed lossless"
    );
}

#[test]
fn an_edition_declines_to_summarize_properties_its_tracks_disagree_on() {
    let mut library = Library::open_in_memory().expect("open");
    library
        .add_tracks(vec![
            encoded("/m/1.flac", "Mixed Rates", "One", 1, AudioFormat::Flac, 900),
            TrackMeta {
                bit_depth: Some(24),
                sample_rate: Some(96_000),
                ..encoded(
                    "/m/2.flac",
                    "Mixed Rates",
                    "Two",
                    2,
                    AudioFormat::Flac,
                    2_400,
                )
            },
        ])
        .expect("add");
    let albums = library.albums();
    let edition = albums[0].default_edition().expect("edition");
    assert_eq!(
        edition.bit_depth(),
        None,
        "16 and 24 do not average to a claim"
    );
    assert_eq!(edition.sample_rate(), None);
    // Bitrate legitimately varies per track, so it is averaged rather than
    // withheld.
    assert_eq!(edition.bitrate(), Some(1_650));
}

// ---------------------------------------------------------------------------
// Album-artist grouping (docs/adr/0008-album-artist-grouping.md)
// ---------------------------------------------------------------------------

/// The owner's real soundtrack, as `library.db` holds it: nine files whose
/// `ARTIST` tags name four different combinations of composers, and whose
/// `ALBUMARTIST` tags all read `RODIK`. Before album artists, this shattered
/// into one shelf entry per distinct artist string.
fn cookies_bustle_gamerip() -> Vec<TrackMeta> {
    let composers = [
        "Kouhei Okamura",
        "Kouhei Okamura, Masashi Matsumoto",
        "Katsuhiko Nakamichi",
        "Miki Nagamatsu, Kouhei Okamura, Masashi Matsumoto, Katsuhiko Nakamichi",
    ];
    let cues = [
        "Main Menu",
        "Bus Level",
        "Southern Territory",
        "Temple",
        "Top Down Level",
        "Good Ending",
    ];
    cues.iter()
        .enumerate()
        .map(|(index, cue)| {
            let number = u32::try_from(index).expect("small") + 1;
            TrackMeta {
                album_artist: Some("RODIK".to_owned()),
                ..encoded(
                    &format!("/m/[GST] Cookie's Bustle/{number}. {cue}.flac"),
                    "Cookie's Bustle OST (gamerip)",
                    cue,
                    number,
                    AudioFormat::Flac,
                    900,
                )
            }
        })
        .enumerate()
        .map(|(index, mut meta)| {
            meta.artist = Some(composers[index % composers.len()].to_owned());
            meta
        })
        .collect()
}

#[test]
fn a_soundtrack_with_a_composer_per_track_is_one_album() {
    let tracks = cookies_bustle_gamerip();
    let distinct_artists: std::collections::BTreeSet<&str> =
        tracks.iter().filter_map(|t| t.artist.as_deref()).collect();
    assert_eq!(
        distinct_artists.len(),
        4,
        "the fixture must actually be the shattering case"
    );

    let mut library = Library::open_in_memory().expect("open");
    library.add_tracks(tracks).expect("add");

    let albums = library.albums();
    assert_eq!(albums.len(), 1, "one album, not one per composer");
    let album = &albums[0];
    assert_eq!(album.artist, AlbumArtist::Named("RODIK"));
    assert_eq!(album.title, Some("Cookie's Bustle OST (gamerip)"));
    assert_eq!(album.editions.len(), 1);
    assert_eq!(album.editions[0].tracks.len(), 6, "every cue is present");

    // And the per-track credits survive the merge — they are the thing a
    // collector-curator keeps a soundtrack for.
    let credits: Vec<&str> = album.editions[0]
        .tracks
        .iter()
        .filter_map(|t| t.artist.as_deref())
        .collect();
    assert_eq!(credits.len(), 6);
    assert!(credits.contains(&"Katsuhiko Nakamichi"));
}

#[test]
fn an_album_with_no_album_artist_but_one_artist_groups_as_before() {
    let mut library = Library::open_in_memory().expect("open");
    library
        .add_tracks(vec![
            track(
                "/m/1.flac",
                "Miki Nagamatsu",
                "Cookie's Bustle OST",
                "RIVER",
                6,
            ),
            track(
                "/m/2.flac",
                "Miki Nagamatsu",
                "Cookie's Bustle OST",
                "Alien Arena",
                2,
            ),
        ])
        .expect("add");

    let albums = library.albums();
    assert_eq!(albums.len(), 1);
    assert_eq!(
        albums[0].artist,
        AlbumArtist::Named("Miki Nagamatsu"),
        "with no album-artist tag the chain falls through to the artist, \
         which is exactly the pre-v3 behaviour"
    );
}

#[test]
fn differing_artists_without_a_compilation_flag_stay_separate() {
    let mut library = Library::open_in_memory().expect("open");
    library
        .add_tracks(vec![
            track("/m/a.flac", "Alpha", "Greatest Hits", "A-Side", 1),
            track("/m/b.flac", "Beta", "Greatest Hits", "B-Side", 1),
        ])
        .expect("add");

    // Nothing in either file says these belong together, and two artists
    // who each released a "Greatest Hits" is far commoner than a compilation
    // nobody tagged. Merging on title alone would be a guess; baz declines.
    assert_eq!(library.albums().len(), 2);
}

#[test]
fn a_flagged_compilation_with_differing_artists_becomes_one_various_album() {
    let mut library = Library::open_in_memory().expect("open");
    library
        .add_tracks(vec![
            TrackMeta {
                compilation: Some(true),
                ..track(
                    "/m/now/1.flac",
                    "Alpha",
                    "Now That's What I Call 42",
                    "A",
                    1,
                )
            },
            TrackMeta {
                compilation: Some(true),
                ..track("/m/now/2.flac", "Beta", "Now That's What I Call 42", "B", 2)
            },
            TrackMeta {
                compilation: Some(true),
                ..track(
                    "/m/now/3.flac",
                    "Gamma",
                    "Now That's What I Call 42",
                    "C",
                    3,
                )
            },
        ])
        .expect("add");

    let albums = library.albums();
    assert_eq!(albums.len(), 1, "the files said they belong together");
    assert_eq!(
        albums[0].artist,
        AlbumArtist::Various,
        "no name was given, so none is invented"
    );
    assert_eq!(albums[0].artist.name(), None);
    assert_eq!(albums[0].editions[0].tracks.len(), 3);
}

#[test]
fn a_compilation_names_itself_when_the_tagger_named_it() {
    let mut library = Library::open_in_memory().expect("open");
    library
        .add_tracks(vec![
            TrackMeta {
                album_artist: Some("Various Artists".to_owned()),
                compilation: Some(true),
                ..track("/m/cb/1.mp3", "Miki Nagamatsu", "Cookie's Bustle", "A", 1)
            },
            TrackMeta {
                album_artist: Some("Various Artists".to_owned()),
                compilation: Some(true),
                ..track("/m/cb/2.mp3", "Kouhei Okamura", "Cookie's Bustle", "B", 2)
            },
        ])
        .expect("add");

    let albums = library.albums();
    assert_eq!(albums.len(), 1);
    // The owner's real files carry this exact tag. It is a *name*, not baz's
    // compilation bucket, and the two must remain distinguishable.
    assert_eq!(albums[0].artist, AlbumArtist::Named("Various Artists"));
    assert_ne!(albums[0].artist, AlbumArtist::Various);
}

#[test]
fn a_grouped_soundtrack_still_splits_into_editions_by_codec() {
    // The two axes are independent: album artist decides what one album is,
    // codec decides how many editions it has (ADR-0007).
    let mut tracks = cookies_bustle_gamerip();
    tracks.extend(cookies_bustle_gamerip().into_iter().map(|meta| TrackMeta {
        path: PathBuf::from(meta.path.to_string_lossy().replace(".flac", ".m4a")),
        format: Some(AudioFormat::Aac),
        bit_depth: None,
        bitrate: Some(256),
        ..meta
    }));

    let mut library = Library::open_in_memory().expect("open");
    library.add_tracks(tracks).expect("add");

    let albums = library.albums();
    assert_eq!(albums.len(), 1, "still one album");
    assert_eq!(albums[0].artist, AlbumArtist::Named("RODIK"));
    assert_eq!(albums[0].editions.len(), 2, "still two editions");
    assert_eq!(
        albums[0].default_edition().and_then(|e| e.format),
        Some(AudioFormat::Flac),
        "lossless still wins the default"
    );
    for edition in &albums[0].editions {
        assert_eq!(edition.tracks.len(), 6);
    }
}

#[test]
fn shelf_order_puts_unknowns_first_and_unnamed_compilations_last() {
    let mut library = Library::open_in_memory().expect("open");
    library
        .add_tracks(vec![
            track("/m/m.flac", "Middle", "Album", "One", 1),
            TrackMeta {
                compilation: Some(true),
                ..track("/m/c.flac", "Someone", "Mixtape", "Two", 1)
            },
            bare("/m/stray.mp3"),
        ])
        .expect("add");

    let albums = library.albums();
    let artists: Vec<AlbumArtist<'_>> = albums.iter().map(|a| a.artist).collect();
    assert_eq!(
        artists,
        [
            AlbumArtist::Unknown,
            AlbumArtist::Named("Middle"),
            AlbumArtist::Various,
        ]
    );
}

#[test]
fn a_distinct_album_artist_is_searchable() {
    let mut library = Library::open_in_memory().expect("open");
    library.add_tracks(cookies_bustle_gamerip()).expect("add");

    // The name on the shelf tile must be a name search can find, or the
    // filtered shelf contradicts the unfiltered one.
    assert_eq!(library.search("rodik", 20).len(), 6);
    assert_eq!(library.search("RODIK", 20).len(), 6);
    // And the track artists are still searchable in their own right.
    assert!(!library.search("Katsuhiko", 20).is_empty());
}

// ---------------------------------------------------------------------------
// Schema migration
// ---------------------------------------------------------------------------

/// One row as the v1 schema stored it, for [`write_v1_database`].
struct V1Row {
    path: &'static str,
    artist: &'static str,
    album: &'static str,
    title: &'static str,
    track: u32,
    disc: u32,
    year: u32,
    duration_ns: i64,
}

/// Encode a path the way the *platform* stores it in the index, so a
/// hand-built fixture is a genuine database for the machine running the test.
///
/// baz stores paths as platform-native bytes — raw `OsStr` bytes on Unix,
/// UTF-16LE code units on Windows — because that is the only lossless
/// encoding on each (see `index`'s module docs). A fixture that wrote UTF-8
/// everywhere would be a *Unix* database, and Windows would rightly refuse to
/// decode it: exactly the false failure this helper exists to prevent. It
/// deliberately mirrors the production encoder rather than calling it, so the
/// fixture stays independent of the code under test.
#[cfg(unix)]
fn stored_path_bytes(path: &str) -> Vec<u8> {
    path.as_bytes().to_vec()
}

/// See the Unix twin: the same contract, in the encoding Windows stores.
#[cfg(windows)]
fn stored_path_bytes(path: &str) -> Vec<u8> {
    use std::os::windows::ffi::OsStrExt;
    std::ffi::OsStr::new(path)
        .encode_wide()
        .flat_map(u16::to_le_bytes)
        .collect()
}

/// Build a genuine v1 database with the v1 schema and v1 statements only —
/// no baz code involved, so this is what a real pre-editions `library.db`
/// looks like on the owner's disk.
fn write_v1_database(db: &std::path::Path) {
    let conn = rusqlite::Connection::open(db).expect("create v1 db");
    conn.execute_batch(
        "
        BEGIN;
        CREATE TABLE tracks (
            id          INTEGER PRIMARY KEY,
            path        BLOB NOT NULL UNIQUE,
            artist      TEXT,
            album       TEXT,
            title       TEXT,
            track       INTEGER,
            disc        INTEGER,
            year        INTEGER,
            duration_ns INTEGER
        ) STRICT;
        PRAGMA user_version = 1;
        COMMIT;
        ",
    )
    .expect("v1 schema");
    let rows = [
        // The double rip that motivated editions at all.
        V1Row {
            path: "/m/FLAC/Stan Rogers/Northwest Passage/01 Northwest Passage.flac",
            artist: "Stan Rogers",
            album: "Northwest Passage",
            title: "Northwest Passage",
            track: 1,
            disc: 1,
            year: 1981,
            duration_ns: 261_000_000_000,
        },
        V1Row {
            path: "/m/MP3/Stan Rogers/Northwest Passage/01 Northwest Passage.mp3",
            artist: "Stan Rogers",
            album: "Northwest Passage",
            title: "Northwest Passage",
            track: 1,
            disc: 1,
            year: 1981,
            duration_ns: 261_000_000_000,
        },
        // Unicode everywhere, to prove the upgrade is not re-encoding text.
        V1Row {
            path: "/m/misc/Größenwahn/Debüt/03 Ærø — 序曲.wav",
            artist: "Größenwahn",
            album: "Debüt",
            title: "Ærø — 序曲",
            track: 3,
            disc: 1,
            year: 1999,
            duration_ns: 215_123_456_789,
        },
        // An ambiguous container: only reading the file can say ALAC or AAC.
        V1Row {
            path: "/m/misc/Someone/Some Album/02 Track.m4a",
            artist: "Someone",
            album: "Some Album",
            title: "Track",
            track: 2,
            disc: 1,
            year: 2005,
            duration_ns: 120_000_000_000,
        },
    ];
    for row in rows {
        conn.execute(
            "INSERT INTO tracks (path, artist, album, title, track, disc, year, duration_ns)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                stored_path_bytes(row.path),
                row.artist,
                row.album,
                row.title,
                row.track,
                row.disc,
                row.year,
                row.duration_ns
            ],
        )
        .expect("insert v1 row");
    }
}

#[test]
fn a_v1_database_migrates_in_place_without_losing_anything() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("library.db");
    write_v1_database(&db);

    let library = Library::open(&db).expect("a v1 database must open");
    assert_eq!(library.len(), 4, "every v1 row survives the upgrade");

    // The schema really did move — and all the way, v1 → v2 → … → v6,
    // because migrations chain rather than jumping.
    let conn = rusqlite::Connection::open(&db).expect("raw open");
    let version: i64 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("user_version");
    assert_eq!(version, 9);

    let by_path = |needle: &str| {
        library
            .tracks()
            .find(|t| t.path.to_string_lossy().contains(needle))
            .cloned()
            .unwrap_or_else(|| panic!("{needle} must survive"))
    };

    // Descriptive metadata is untouched, Unicode included.
    let unicode = by_path("Größenwahn");
    assert_eq!(unicode.artist.as_deref(), Some("Größenwahn"));
    assert_eq!(unicode.album.as_deref(), Some("Debüt"));
    assert_eq!(unicode.title.as_deref(), Some("Ærø — 序曲"));
    assert_eq!(unicode.track, Some(3));
    assert_eq!(unicode.disc, Some(1));
    assert_eq!(unicode.year, Some(1999));
    assert_eq!(unicode.duration, Some(Duration::new(215, 123_456_789)));

    // Formats are backfilled from unambiguous extensions...
    assert_eq!(unicode.format, Some(AudioFormat::Wav));
    assert_eq!(
        by_path("Northwest Passage.flac").format,
        Some(AudioFormat::Flac)
    );
    assert_eq!(
        by_path("Northwest Passage.mp3").format,
        Some(AudioFormat::Mp3)
    );
    // ...and left unknown where the extension cannot settle it; the next
    // rescan fills it in.
    assert_eq!(by_path("Track.m4a").format, None);

    // Properties the backfill cannot know stay unset rather than invented.
    assert_eq!(unicode.bit_depth, None);
    assert_eq!(unicode.sample_rate, None);
    assert_eq!(unicode.bitrate, None);
    // Including the v4 stamp: an unstamped row is simply re-read.
    assert_eq!(unicode.stamp, None);

    // The point of the whole exercise: the double rip is now one album with
    // two editions, defaulting to the lossless one.
    let albums = library.albums();
    let passage = albums
        .iter()
        .find(|a| a.title == Some("Northwest Passage"))
        .expect("the album");
    assert_eq!(passage.editions.len(), 2);
    assert_eq!(
        passage.default_edition().and_then(|e| e.format),
        Some(AudioFormat::Flac)
    );
}

/// One row as the v2 schema stored it, for [`write_v2_database`].
struct V2Row {
    path: &'static str,
    artist: &'static str,
    album: &'static str,
    title: &'static str,
    track: u32,
    year: u32,
    duration_ns: i64,
    format: &'static str,
    bit_depth: Option<u32>,
    sample_rate: u32,
    bitrate: u32,
}

/// Build a genuine v2 database with the v2 schema and v2 statements only —
/// no baz code involved. This is the shape of the `library.db` sitting on
/// the owner's disk right now, contents included: the double rip that
/// motivated editions, and the soundtrack that motivated album artists.
fn write_v2_database(db: &std::path::Path) {
    let conn = rusqlite::Connection::open(db).expect("create v2 db");
    conn.execute_batch(
        "
        BEGIN;
        CREATE TABLE tracks (
            id          INTEGER PRIMARY KEY,
            path        BLOB NOT NULL UNIQUE,
            artist      TEXT,
            album       TEXT,
            title       TEXT,
            track       INTEGER,
            disc        INTEGER,
            year        INTEGER,
            duration_ns INTEGER,
            format      TEXT,
            bit_depth   INTEGER,
            sample_rate INTEGER,
            bitrate     INTEGER
        ) STRICT;
        PRAGMA user_version = 2;
        COMMIT;
        ",
    )
    .expect("v2 schema");

    for row in v2_rows() {
        conn.execute(
            "INSERT INTO tracks
                 (path, artist, album, title, track, disc, year, duration_ns,
                  format, bit_depth, sample_rate, bitrate)
             VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?7, ?8, ?9, ?10, ?11)",
            rusqlite::params![
                stored_path_bytes(row.path),
                row.artist,
                row.album,
                row.title,
                row.track,
                row.year,
                row.duration_ns,
                row.format,
                row.bit_depth,
                row.sample_rate,
                row.bitrate,
            ],
        )
        .expect("insert v2 row");
    }
}

/// The rows [`write_v2_database`] seeds: the owner's own library, minus the
/// paths that only exist on his disk.
fn v2_rows() -> [V2Row; 5] {
    [
        V2Row {
            path: "/m/FLAC/Stan Rogers/Northwest Passage/01 - Northwest Passage.flac",
            artist: "Stan Rogers",
            album: "Northwest Passage",
            title: "Northwest Passage",
            track: 1,
            year: 1981,
            duration_ns: 261_000_000_000,
            format: "flac",
            bit_depth: Some(16),
            sample_rate: 44_100,
            bitrate: 900,
        },
        V2Row {
            path: "/m/MP3/Stan Rogers/Northwest Passage/01 - Northwest Passage.mp3",
            artist: "Stan Rogers",
            album: "Northwest Passage",
            title: "Northwest Passage",
            track: 1,
            year: 1981,
            duration_ns: 261_000_000_000,
            format: "mp3",
            bit_depth: None,
            sample_rate: 44_100,
            bitrate: 320,
        },
        // The shattered soundtrack, exactly as the owner's v2 database holds
        // it: same album, two different `ARTIST` strings, no album artist
        // anywhere because v2 had nowhere to put one.
        V2Row {
            path: "/m/[GST] Cookie's Bustle/1. Main Menu.flac",
            artist: "Kouhei Okamura, Masashi Matsumoto, Katsuhiko Nakamichi",
            album: "Cookie's Bustle OST (gamerip)",
            title: "Main Menu",
            track: 1,
            year: 1998,
            duration_ns: 95_000_000_000,
            format: "flac",
            bit_depth: Some(16),
            sample_rate: 44_100,
            bitrate: 700,
        },
        V2Row {
            path: "/m/[GST] Cookie's Bustle/As For Dreams.m4a",
            artist: "Miki Nagamatsu, Kouhei Okamura, Masashi Matsumoto, Katsuhiko Nakamichi",
            album: "Cookie's Bustle OST (gamerip)",
            title: "As For Dreams (low quality)",
            track: 2,
            year: 1998,
            duration_ns: 130_000_000_000,
            format: "aac",
            bit_depth: None,
            sample_rate: 44_100,
            bitrate: 256,
        },
        // Unicode everywhere, to prove the upgrade is not re-encoding text.
        V2Row {
            path: "/m/misc/Größenwahn/Debüt/03 Ærø — 序曲.wav",
            artist: "Größenwahn",
            album: "Debüt",
            title: "Ærø — 序曲",
            track: 3,
            year: 1999,
            duration_ns: 215_123_456_789,
            format: "wav",
            bit_depth: Some(24),
            sample_rate: 96_000,
            bitrate: 4_608,
        },
    ]
}

#[test]
fn a_v2_database_migrates_in_place_without_losing_anything() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("library.db");
    write_v2_database(&db);

    let library = Library::open(&db).expect("a v2 database must open");
    assert_eq!(library.len(), 5, "every v2 row survives the upgrade");

    let conn = rusqlite::Connection::open(&db).expect("raw open");
    let version: i64 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("user_version");
    assert_eq!(version, 9);

    let by_path = |needle: &str| {
        library
            .tracks()
            .find(|t| t.path.to_string_lossy().contains(needle))
            .cloned()
            .unwrap_or_else(|| panic!("{needle} must survive"))
    };

    // Every v2 column is intact, Unicode and encoding properties included.
    let unicode = by_path("Größenwahn");
    assert_eq!(unicode.artist.as_deref(), Some("Größenwahn"));
    assert_eq!(unicode.album.as_deref(), Some("Debüt"));
    assert_eq!(unicode.title.as_deref(), Some("Ærø — 序曲"));
    assert_eq!(unicode.track, Some(3));
    assert_eq!(unicode.disc, Some(1));
    assert_eq!(unicode.year, Some(1999));
    assert_eq!(unicode.duration, Some(Duration::new(215, 123_456_789)));
    assert_eq!(unicode.format, Some(AudioFormat::Wav));
    assert_eq!(unicode.bit_depth, Some(24));
    assert_eq!(unicode.sample_rate, Some(96_000));
    assert_eq!(unicode.bitrate, Some(4_608));

    // The new columns are NULL, because nothing in a v2 database could
    // honestly fill them — see `migrate_v2_to_v3`.
    for track in library.tracks() {
        assert_eq!(track.album_artist, None);
        assert_eq!(track.compilation, None);
    }

    // Until the rescan, grouping is *exactly* the pre-v3 behaviour: the
    // double rip is still one album with two editions, and the soundtrack is
    // still shattered. The upgrade fixes nothing by itself and breaks
    // nothing either.
    let albums = library.albums();
    let passage = albums
        .iter()
        .find(|a| a.title == Some("Northwest Passage"))
        .expect("the double rip");
    assert_eq!(passage.artist, AlbumArtist::Named("Stan Rogers"));
    assert_eq!(passage.editions.len(), 2);
    assert_eq!(
        albums
            .iter()
            .filter(|a| a.title == Some("Cookie's Bustle OST (gamerip)"))
            .count(),
        2,
        "two artist strings, two entries — the bug, faithfully preserved"
    );
}

#[test]
fn the_rescan_after_a_v2_upgrade_collapses_the_shattered_soundtrack() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("library.db");
    write_v2_database(&db);

    let mut library = Library::open(&db).expect("open migrates to v3");
    // What baz does on every launch: rescan the music folder and upsert. The
    // files carry `ALBUMARTIST=RODIK`; v3 finally has somewhere to put it.
    let rescanned: Vec<TrackMeta> = library
        .tracks()
        .filter(|t| t.album.as_deref() == Some("Cookie's Bustle OST (gamerip)"))
        .cloned()
        .map(|meta| TrackMeta {
            album_artist: Some("RODIK".to_owned()),
            ..meta
        })
        .collect();
    assert_eq!(rescanned.len(), 2);
    library.add_tracks(rescanned).expect("rescan batch");
    assert_eq!(library.len(), 5, "an upsert, not a duplicate");

    let albums = library.albums();
    let gamerip: Vec<_> = albums
        .iter()
        .filter(|a| a.title == Some("Cookie's Bustle OST (gamerip)"))
        .collect();
    assert_eq!(gamerip.len(), 1, "two shelf entries became one");
    assert_eq!(gamerip[0].artist, AlbumArtist::Named("RODIK"));
    // One album, two codecs: the edition split is untouched by the merge.
    assert_eq!(gamerip[0].editions.len(), 2);

    // And it is durable.
    drop(library);
    let reopened = Library::open(&db).expect("reopen");
    let stored = reopened
        .tracks()
        .find(|t| t.path.to_string_lossy().ends_with("1. Main Menu.flac"))
        .expect("the track");
    assert_eq!(stored.album_artist.as_deref(), Some("RODIK"));
}

#[test]
fn migrating_twice_is_a_no_op_and_a_rescan_fills_the_gaps() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("library.db");
    write_v1_database(&db);

    drop(Library::open(&db).expect("first open migrates"));
    let mut library = Library::open(&db).expect("second open finds v2");
    assert_eq!(library.len(), 4, "reopening a v2 database changes nothing");

    // The rescan that follows every startup upserts real properties over the
    // backfilled row, including for the container the backfill left unknown.
    let rescanned = TrackMeta {
        format: Some(AudioFormat::Alac),
        bit_depth: Some(16),
        sample_rate: Some(44_100),
        bitrate: Some(850),
        ..track(
            "/m/misc/Someone/Some Album/02 Track.m4a",
            "Someone",
            "Some Album",
            "Track",
            2,
        )
    };
    library.add_tracks(vec![rescanned]).expect("rescan batch");
    assert_eq!(library.len(), 4, "an upsert, not a duplicate");

    drop(library);
    let reopened = Library::open(&db).expect("reopen");
    let track = reopened
        .tracks()
        .find(|t| t.path.to_string_lossy().ends_with("02 Track.m4a"))
        .expect("the rescanned track");
    assert_eq!(track.format, Some(AudioFormat::Alac));
    assert_eq!(track.bit_depth, Some(16));
    assert_eq!(track.sample_rate, Some(44_100));
    assert_eq!(track.bitrate, Some(850));
}

#[test]
fn encoding_properties_roundtrip_through_a_real_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("library.db");
    let original = TrackMeta {
        format: Some(AudioFormat::Flac),
        bit_depth: Some(24),
        sample_rate: Some(192_000),
        bitrate: Some(4_100),
        ..track("/m/hires.flac", "Karl", "Signal Chain", "Test Tone", 1)
    };
    {
        let mut library = Library::open(&db).expect("open");
        library.add_tracks(vec![original.clone()]).expect("add");
    }
    let reopened = Library::open(&db).expect("reopen");
    let stored = reopened.tracks().next().expect("one track").clone();
    assert_eq!(stored, original);
}

#[test]
fn newer_schema_versions_are_refused() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("library.db");
    drop(Library::open(&db).expect("create"));

    // Simulate a database left behind by a future baz.
    let conn = rusqlite::Connection::open(&db).expect("raw open");
    conn.pragma_update(None, "user_version", 99).expect("bump");
    drop(conn);

    let err = Library::open(&db).err().expect("open must fail");
    match err {
        IndexError::SchemaTooNew { found } => assert_eq!(found, 99),
        other => panic!("expected SchemaTooNew, got {other:?}"),
    }
}

/// **The load-bearing fact behind the `Newer baz` screen**: a refused open
/// does not write a byte, so *"your music and your playlists are untouched"*
/// is a statement about this code and not a hope.
///
/// The shell used to answer this error by drawing the first-run screen
/// (*"where's your music?"*), whose one control invites the listener to name a
/// folder — which calls straight back into [`Library::open`] against the same
/// file. That is the retry this asserts: **three** opens, each refused, and
/// afterwards the database still holds every row and still declares the
/// version the newer baz stamped on it. A migration that ran before the
/// version check, or a `CREATE TABLE IF NOT EXISTS` on the way past, would
/// change these bytes and turn a presentation defect into a data-loss one.
///
/// The bytes of `library.db` itself are compared, not a row count: a count
/// would miss a rewritten header, a bumped `user_version`, or a table added
/// beside the ones being counted.
#[test]
fn a_too_new_database_is_refused_without_a_byte_being_written() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("library.db");
    let originals = vec![
        track(
            "/m/a/01.flac",
            "Stan Rogers",
            "Fogarty's Cove",
            "Watching",
            1,
        ),
        track(
            "/m/a/02.flac",
            "Stan Rogers",
            "Fogarty's Cove",
            "Barrett's",
            2,
        ),
    ];
    {
        let mut library = Library::open(&db).expect("create");
        library.add_tracks(originals.clone()).expect("add");
    }
    // A database from a baz two schema versions ahead of this build. The
    // `PRAGMA` is the only difference from a library this build wrote, which
    // is deliberate: the *rows* are ones this build can read, so anything that
    // touched them would be reading and writing them happily, and the version
    // check is the only thing standing between them and a downgrade.
    let conn = rusqlite::Connection::open(&db).expect("raw open");
    conn.pragma_update(None, "user_version", baz_core::index::SCHEMA_VERSION + 2)
        .expect("bump");
    drop(conn);

    let before = std::fs::read(&db).expect("read the refused database");
    for attempt in 1..=3 {
        let err = Library::open(&db).err().expect("open must fail");
        assert!(
            matches!(err, IndexError::SchemaTooNew { found }
                     if found == baz_core::index::SCHEMA_VERSION + 2),
            "attempt {attempt} gave {err:?}"
        );
        assert_eq!(
            std::fs::read(&db).expect("re-read"),
            before,
            "attempt {attempt} changed the database this build refused to read"
        );
    }

    // …and the rows are readable the moment a build that speaks the version
    // opens it, which is what the screen promises the listener happens when
    // they put the newer baz back.
    let conn = rusqlite::Connection::open(&db).expect("raw open");
    conn.pragma_update(None, "user_version", baz_core::index::SCHEMA_VERSION)
        .expect("restore");
    drop(conn);
    let recovered = Library::open(&db).expect("open at the supported version");
    let mut titles: Vec<String> = recovered
        .tracks()
        .filter_map(|meta| meta.title.clone())
        .collect();
    titles.sort();
    assert_eq!(titles, vec!["Barrett's".to_owned(), "Watching".to_owned()]);
}

/// **Setting a library aside loses nothing**, which is the only reason the
/// blocked-library screen is allowed to offer it.
///
/// The round trip is the whole assertion: a too-new database is moved out of
/// the way, the original name opens as a first-run library, and renaming the
/// file back reproduces the original refusal byte for byte. If that holds,
/// *"nothing is deleted; renaming it back restores it exactly"* is a fact
/// about this code.
#[test]
fn setting_a_library_aside_moves_it_whole_and_is_reversible() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("library.db");
    {
        let mut library = Library::open(&db).expect("create");
        library
            .add_tracks(vec![track("/m/a/01.flac", "Stan", "Cove", "Watching", 1)])
            .expect("add");
    }
    let conn = rusqlite::Connection::open(&db).expect("raw open");
    conn.pragma_update(None, "user_version", 99).expect("bump");
    drop(conn);
    let original = std::fs::read(&db).expect("read");

    let aside = baz_core::index::set_aside(&db).expect("set aside");
    assert_eq!(aside, dir.path().join("library.db.set-aside-1"));
    assert!(
        !db.exists(),
        "the database was left where baz could open it"
    );
    assert_eq!(
        std::fs::read(&aside).expect("read the set-aside file"),
        original,
        "setting aside rewrote the file it was supposed to preserve"
    );

    // The original name is now a first run — which is the point.
    let fresh = Library::open(&db).expect("a first-run library");
    assert_eq!(fresh.len(), 0);
    drop(fresh);

    // …and the set-aside file is still exactly the library a newer baz wrote:
    // put it back and this build refuses it again, as it did at the start.
    std::fs::remove_file(&db).expect("clear the fresh index");
    for sidecar in ["-wal", "-shm"] {
        let _ = std::fs::remove_file(dir.path().join(format!("library.db{sidecar}")));
    }
    std::fs::rename(&aside, &db).expect("rename back");
    assert!(matches!(
        Library::open(&db).err().expect("still refused"),
        IndexError::SchemaTooNew { found: 99 }
    ));
}

/// The write-ahead log and the shared-memory file travel with the database.
///
/// A `library.db-wal` left behind would be recovered *into the new database*
/// by SQLite the next time one appeared under that name — a stale log applied
/// to a library it does not belong to, which is the one way this operation
/// could corrupt anything. It is asserted rather than reasoned about because
/// the sidecars are usually absent (SQLite removes them on a clean close), so
/// the failure would never show up in ordinary use.
#[test]
fn setting_aside_takes_the_write_ahead_log_with_it() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("library.db");
    drop(Library::open(&db).expect("create"));
    // Stand in for a newer baz that was killed mid-write: a database with
    // both sidecars beside it.
    for sidecar in ["-wal", "-shm"] {
        std::fs::write(dir.path().join(format!("library.db{sidecar}")), b"stale")
            .expect("write sidecar");
    }

    let aside = baz_core::index::set_aside(&db).expect("set aside");
    for sidecar in ["-wal", "-shm"] {
        assert!(
            !dir.path().join(format!("library.db{sidecar}")).exists(),
            "library.db{sidecar} was left beside a name a new database will take"
        );
        let moved = PathBuf::from(format!("{}{sidecar}", aside.display()));
        assert_eq!(std::fs::read(&moved).expect("read moved sidecar"), b"stale");
    }

    // A second set-aside does not overwrite the first.
    drop(Library::open(&db).expect("second library"));
    let second = baz_core::index::set_aside(&db).expect("set aside again");
    assert_eq!(second, dir.path().join("library.db.set-aside-2"));
    assert!(
        aside.exists(),
        "the first set-aside library was overwritten"
    );
}

// ---------------------------------------------------------------------------
// Schema v4: the file stamps incremental scanning compares, and removal.
// ---------------------------------------------------------------------------

/// One row as the v3 schema stored it, for [`write_v3_database`].
struct V3Row {
    path: &'static str,
    artist: &'static str,
    album_artist: Option<&'static str>,
    compilation: Option<i64>,
    album: &'static str,
    title: &'static str,
    track: u32,
    year: u32,
    duration_ns: i64,
    format: &'static str,
    bit_depth: Option<u32>,
    sample_rate: u32,
    bitrate: u32,
}

/// Build a genuine v3 database with the v3 schema and v3 `INSERT`s only — no
/// baz code involved. This is the shape of the `library.db` an installed baz
/// leaves on disk today, contents included: the double rip that motivated
/// editions, the soundtrack that motivated album artists, and the one file
/// whose tagger really did write `Various Artists`.
fn write_v3_database(db: &std::path::Path) {
    let conn = rusqlite::Connection::open(db).expect("create v3 db");
    conn.execute_batch(
        "
        BEGIN;
        CREATE TABLE tracks (
            id           INTEGER PRIMARY KEY,
            path         BLOB NOT NULL UNIQUE,
            artist       TEXT,
            album        TEXT,
            title        TEXT,
            track        INTEGER,
            disc         INTEGER,
            year         INTEGER,
            duration_ns  INTEGER,
            format       TEXT,
            bit_depth    INTEGER,
            sample_rate  INTEGER,
            bitrate      INTEGER,
            album_artist TEXT,
            compilation  INTEGER
        ) STRICT;
        PRAGMA user_version = 3;
        COMMIT;
        ",
    )
    .expect("v3 schema");

    for row in v3_rows() {
        conn.execute(
            "INSERT INTO tracks
                 (path, artist, album, title, track, disc, year, duration_ns,
                  format, bit_depth, sample_rate, bitrate, album_artist,
                  compilation)
             VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            rusqlite::params![
                stored_path_bytes(row.path),
                row.artist,
                row.album,
                row.title,
                row.track,
                row.year,
                row.duration_ns,
                row.format,
                row.bit_depth,
                row.sample_rate,
                row.bitrate,
                row.album_artist,
                row.compilation,
            ],
        )
        .expect("insert v3 row");
    }
}

/// The rows [`write_v3_database`] seeds.
fn v3_rows() -> [V3Row; 5] {
    [
        V3Row {
            path: "/m/FLAC/Stan Rogers/Northwest Passage/01 - Northwest Passage.flac",
            artist: "Stan Rogers",
            album_artist: Some("Stan Rogers"),
            compilation: Some(0),
            album: "Northwest Passage",
            title: "Northwest Passage",
            track: 1,
            year: 1981,
            duration_ns: 261_000_000_000,
            format: "flac",
            bit_depth: Some(16),
            sample_rate: 44_100,
            bitrate: 900,
        },
        V3Row {
            path: "/m/MP3/Stan Rogers/Northwest Passage/01 - Northwest Passage.mp3",
            artist: "Stan Rogers",
            album_artist: Some("Stan Rogers"),
            compilation: Some(0),
            album: "Northwest Passage",
            title: "Northwest Passage",
            track: 1,
            year: 1981,
            duration_ns: 261_000_000_000,
            format: "mp3",
            bit_depth: None,
            sample_rate: 44_100,
            bitrate: 320,
        },
        // The soundtrack ADR-0008 collapsed: two per-track credits, one
        // album artist.
        V3Row {
            path: "/m/[GST] Cookie's Bustle/1. Main Menu.flac",
            artist: "Kouhei Okamura, Masashi Matsumoto, Katsuhiko Nakamichi",
            album_artist: Some("RODIK"),
            compilation: None,
            album: "Cookie's Bustle OST (gamerip)",
            title: "Main Menu",
            track: 1,
            year: 1998,
            duration_ns: 95_000_000_000,
            format: "flac",
            bit_depth: Some(16),
            sample_rate: 44_100,
            bitrate: 700,
        },
        V3Row {
            path: "/m/[GST] Cookie's Bustle/As For Dreams.m4a",
            artist: "Miki Nagamatsu, Kouhei Okamura",
            album_artist: Some("RODIK"),
            compilation: None,
            album: "Cookie's Bustle OST (gamerip)",
            title: "As For Dreams (low quality)",
            track: 2,
            year: 1998,
            duration_ns: 130_000_000_000,
            format: "aac",
            bit_depth: None,
            sample_rate: 44_100,
            bitrate: 256,
        },
        // Unicode everywhere, plus a genuine `Various Artists` tag and a
        // real compilation flag, to prove the upgrade re-encodes nothing.
        V3Row {
            path: "/m/misc/Größenwahn/Debüt/03 Ærø — 序曲.wav",
            artist: "Größenwahn",
            album_artist: Some("Various Artists"),
            compilation: Some(1),
            album: "Debüt",
            title: "Ærø — 序曲",
            track: 3,
            year: 1999,
            duration_ns: 215_123_456_789,
            format: "wav",
            bit_depth: Some(24),
            sample_rate: 96_000,
            bitrate: 4_608,
        },
    ]
}

#[test]
fn a_v3_database_migrates_in_place_without_losing_anything() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("library.db");
    write_v3_database(&db);

    let library = Library::open(&db).expect("a v3 database must open");
    assert_eq!(library.len(), 5, "every v3 row survives the upgrade");

    let conn = rusqlite::Connection::open(&db).expect("raw open");
    let version: i64 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("user_version");
    assert_eq!(version, 9);

    let by_path = |needle: &str| {
        library
            .tracks()
            .find(|t| t.path.to_string_lossy().contains(needle))
            .cloned()
            .unwrap_or_else(|| panic!("{needle} must survive"))
    };

    // Every v3 column is intact — text, numbers, Unicode, and the two
    // columns ADR-0008 added.
    let unicode = by_path("Größenwahn");
    assert_eq!(unicode.artist.as_deref(), Some("Größenwahn"));
    assert_eq!(unicode.album.as_deref(), Some("Debüt"));
    assert_eq!(unicode.title.as_deref(), Some("Ærø — 序曲"));
    assert_eq!(unicode.track, Some(3));
    assert_eq!(unicode.disc, Some(1));
    assert_eq!(unicode.year, Some(1999));
    assert_eq!(unicode.duration, Some(Duration::new(215, 123_456_789)));
    assert_eq!(unicode.format, Some(AudioFormat::Wav));
    assert_eq!(unicode.bit_depth, Some(24));
    assert_eq!(unicode.sample_rate, Some(96_000));
    assert_eq!(unicode.bitrate, Some(4_608));
    assert_eq!(unicode.album_artist.as_deref(), Some("Various Artists"));
    assert_eq!(unicode.compilation, Some(true));

    let soundtrack = by_path("Main Menu.flac");
    assert_eq!(soundtrack.album_artist.as_deref(), Some("RODIK"));
    assert_eq!(soundtrack.compilation, None, "NULL is not Some(false)");

    // The new columns are NULL for every row: nothing already in a v3
    // database could honestly claim a file is unchanged, and stat'ing the
    // whole library at startup is the cost this feature exists to remove.
    for track in library.tracks() {
        assert_eq!(track.stamp, None);
    }
    assert!(
        library
            .known_files()
            .values()
            .all(|known| known.stamp.is_none()),
        "an upgraded library asks for a full first scan, and gets one"
    );

    // Grouping is *exactly* the pre-v4 behaviour — the upgrade changes what
    // scanning costs, never what the shelf shows.
    let albums = library.albums();
    let passage = albums
        .iter()
        .find(|a| a.title == Some("Northwest Passage"))
        .expect("the double rip");
    assert_eq!(passage.artist, AlbumArtist::Named("Stan Rogers"));
    assert_eq!(passage.editions.len(), 2);
    let gamerip: Vec<_> = albums
        .iter()
        .filter(|a| a.title == Some("Cookie's Bustle OST (gamerip)"))
        .collect();
    assert_eq!(gamerip.len(), 1, "still one entry, two editions");
    assert_eq!(gamerip[0].artist, AlbumArtist::Named("RODIK"));
    assert_eq!(gamerip[0].editions.len(), 2);
}

#[test]
fn the_first_scan_after_a_v3_upgrade_stamps_every_row() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("library.db");
    write_v3_database(&db);

    let mut library = Library::open(&db).expect("open migrates to v4");
    // What the first launch after the upgrade does: a full scan, because
    // nothing is stamped, writing the stamp back with every row.
    let stamp = FileStamp {
        mtime_ns: 1_700_000_000_123_456_789,
        size: 42_000_000,
    };
    let rescanned: Vec<TrackMeta> = library
        .tracks()
        .cloned()
        .map(|meta| TrackMeta {
            stamp: Some(stamp),
            ..meta
        })
        .collect();
    library.add_tracks(rescanned).expect("rescan batch");
    assert_eq!(library.len(), 5, "an upsert, not a duplicate");

    // Durable, and now the *second* launch can skip all five.
    drop(library);
    let reopened = Library::open(&db).expect("reopen");
    assert!(reopened.tracks().all(|t| t.stamp == Some(stamp)));
    let known = reopened.known_files();
    assert_eq!(known.len(), 5);
    assert!(known.values().all(|known| known.stamp == Some(stamp)));
}

#[test]
fn a_stamp_roundtrips_through_a_real_database_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("library.db");
    // Extremes that must survive the i64 column: a pre-epoch mtime and a
    // size larger than a 32-bit file offset.
    let original = TrackMeta {
        stamp: Some(FileStamp {
            mtime_ns: -86_400_000_000_001,
            size: 5_000_000_000,
        }),
        ..track("/m/ancient.flac", "Karl", "Signal Chain", "Test Tone", 1)
    };
    {
        let mut library = Library::open(&db).expect("open");
        library.add_tracks(vec![original.clone()]).expect("add");
    }
    let reopened = Library::open(&db).expect("reopen");
    let stored = reopened.tracks().next().expect("one track").clone();
    assert_eq!(stored, original);
}

/// Half a stamp is not a stamp: a row that somehow carries only one of the
/// two columns must read back as unstamped (and so be re-read) rather than
/// as a comparison nobody can complete.
#[test]
fn a_half_written_stamp_reads_back_as_no_stamp() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("library.db");
    {
        let mut library = Library::open(&db).expect("open");
        library
            .add_tracks(vec![TrackMeta {
                stamp: Some(FileStamp {
                    mtime_ns: 1_000,
                    size: 2_000,
                }),
                ..bare("/m/a.flac")
            }])
            .expect("add");
    }
    let conn = rusqlite::Connection::open(&db).expect("raw open");
    conn.execute("UPDATE tracks SET file_size = NULL", [])
        .expect("blank half the stamp");
    drop(conn);

    let library = Library::open(&db).expect("reopen");
    assert_eq!(library.tracks().next().expect("the row").stamp, None);
}

#[test]
fn known_files_reports_every_path_with_the_stamp_recorded_for_it() {
    let mut library = Library::open_in_memory().expect("open");
    let stamp = FileStamp {
        mtime_ns: 1_234,
        size: 5_678,
    };
    library
        .add_tracks(vec![
            TrackMeta {
                stamp: Some(stamp),
                ..bare("/m/stamped.flac")
            },
            bare("/m/unstamped.flac"),
        ])
        .expect("add");

    let known = library.known_files();
    assert_eq!(known.len(), 2);
    assert_eq!(known[&PathBuf::from("/m/stamped.flac")].stamp, Some(stamp));
    assert_eq!(
        known[&PathBuf::from("/m/unstamped.flac")].stamp,
        None,
        "a row with no stamp is offered as one, so it is re-read"
    );
}

#[test]
fn removing_tracks_deletes_the_rows_and_unindexes_them() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("library.db");
    let mut library = Library::open(&db).expect("open");
    library
        .add_tracks(vec![
            track(
                "/m/a.flac",
                "Stan Rogers",
                "Fogarty's Cove",
                "Barrett's Privateers",
                1,
            ),
            track(
                "/m/b.flac",
                "Stan Rogers",
                "Fogarty's Cove",
                "Fogarty's Cove",
                2,
            ),
        ])
        .expect("add");
    assert_eq!(library.len(), 2);

    let removed = library
        .remove_tracks([PathBuf::from("/m/b.flac")])
        .expect("remove");
    assert_eq!(removed, 1);
    assert_eq!(library.len(), 1);

    // Gone from the shelf, gone from search, and gone from the snapshot the
    // next scan works off.
    let albums = library.albums();
    assert_eq!(albums.len(), 1);
    assert_eq!(albums[0].editions[0].tracks.len(), 1);
    assert!(library.search("Fogarty's Cove", 10).len() == 1);
    assert!(
        !library
            .known_files()
            .contains_key(&PathBuf::from("/m/b.flac"))
    );

    // And durable: reopening does not resurrect it.
    drop(library);
    let reopened = Library::open(&db).expect("reopen");
    assert_eq!(reopened.len(), 1);
    assert_eq!(
        reopened.tracks().next().expect("the survivor").path,
        PathBuf::from("/m/a.flac")
    );
}

#[test]
fn removing_paths_the_library_does_not_hold_changes_nothing() {
    let mut library = Library::open_in_memory().expect("open");
    library.add_tracks(vec![bare("/m/a.flac")]).expect("add");

    let removed = library
        .remove_tracks([PathBuf::from("/m/never-here.flac"), PathBuf::from("/m/a")])
        .expect("remove");
    assert_eq!(removed, 0, "no row matched, so nothing was deleted");
    assert_eq!(library.len(), 1);
    // Removing nothing at all is also fine.
    assert_eq!(
        library
            .remove_tracks(Vec::<PathBuf>::new())
            .expect("remove"),
        0
    );
    assert_eq!(library.len(), 1);
}

/// Paths are bytes (module docs), and removal must key on exactly the same
/// bytes an insert did — including on paths no `str` can hold.
#[cfg(unix)]
#[test]
fn removal_matches_non_utf8_paths_exactly() {
    use std::os::unix::ffi::OsStringExt;

    let raw = PathBuf::from(std::ffi::OsString::from_vec(
        b"/m/\xFF\xFEbroken/tr\xF0ack.flac".to_vec(),
    ));
    let mut library = Library::open_in_memory().expect("open");
    library
        .add_tracks(vec![TrackMeta {
            path: raw.clone(),
            ..bare("/m/placeholder.flac")
        }])
        .expect("add");
    assert_eq!(library.len(), 1);

    assert_eq!(library.remove_tracks([&raw]).expect("remove"), 1);
    assert!(library.is_empty());
}

// ---------------------------------------------------------------------------
// Schema v5: the ReplayGain figures a file already carries (ADR-0013).
// ---------------------------------------------------------------------------

/// Build a genuine v4 database with the v4 schema and v4 `INSERT`s only — no
/// baz code involved, exactly as [`write_v3_database`] does for its own
/// version. This is the shape of the `library.db` an installed baz leaves on
/// disk today, contents included, plus the file stamps v4 added.
fn write_v4_database(db: &std::path::Path) {
    let conn = rusqlite::Connection::open(db).expect("create v4 db");
    conn.execute_batch(
        "
        BEGIN;
        CREATE TABLE tracks (
            id           INTEGER PRIMARY KEY,
            path         BLOB NOT NULL UNIQUE,
            artist       TEXT,
            album        TEXT,
            title        TEXT,
            track        INTEGER,
            disc         INTEGER,
            year         INTEGER,
            duration_ns  INTEGER,
            format       TEXT,
            bit_depth    INTEGER,
            sample_rate  INTEGER,
            bitrate      INTEGER,
            album_artist TEXT,
            compilation  INTEGER,
            mtime_ns     INTEGER,
            file_size    INTEGER
        ) STRICT;
        PRAGMA user_version = 4;
        COMMIT;
        ",
    )
    .expect("v4 schema");

    for (n, row) in v3_rows().into_iter().enumerate() {
        conn.execute(
            "INSERT INTO tracks
                 (path, artist, album, title, track, disc, year, duration_ns,
                  format, bit_depth, sample_rate, bitrate, album_artist,
                  compilation, mtime_ns, file_size)
             VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                     ?14, ?15)",
            rusqlite::params![
                // The platform's own path encoding, not UTF-8: a `library.db`
                // is a per-machine cache and Windows stores UTF-16LE, so a
                // fixture that hard-coded bytes would only be a *Unix* v4
                // database (see `stored_path_bytes`).
                stored_path_bytes(row.path),
                row.artist,
                row.album,
                row.title,
                row.track,
                row.year,
                row.duration_ns,
                row.format,
                row.bit_depth,
                row.sample_rate,
                row.bitrate,
                row.album_artist,
                row.compilation,
                // A real stamp per row, so the v5 upgrade can be shown not to
                // disturb the one thing v4 added.
                1_700_000_000_000_000_000_i64 + i64::try_from(n).expect("five rows"),
                40_000_000_i64 + i64::try_from(n).expect("five rows"),
            ],
        )
        .expect("insert v4 row");
    }
}

#[test]
fn a_v4_database_migrates_in_place_without_losing_anything() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("library.db");
    write_v4_database(&db);

    let library = Library::open(&db).expect("a v4 database must open");
    assert_eq!(library.len(), 5, "every v4 row survives the upgrade");

    let conn = rusqlite::Connection::open(&db).expect("raw open");
    let version: i64 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("user_version");
    assert_eq!(version, 9);

    let by_path = |needle: &str| {
        library
            .tracks()
            .find(|t| t.path.to_string_lossy().contains(needle))
            .cloned()
            .unwrap_or_else(|| panic!("{needle} must survive"))
    };

    // Every v4 column is intact — text, numbers, Unicode, the ADR-0008
    // columns, and the ADR-0010 stamp.
    let unicode = by_path("Größenwahn");
    assert_eq!(unicode.artist.as_deref(), Some("Größenwahn"));
    assert_eq!(unicode.album.as_deref(), Some("Debüt"));
    assert_eq!(unicode.title.as_deref(), Some("Ærø — 序曲"));
    assert_eq!(unicode.track, Some(3));
    assert_eq!(unicode.disc, Some(1));
    assert_eq!(unicode.year, Some(1999));
    assert_eq!(unicode.duration, Some(Duration::new(215, 123_456_789)));
    assert_eq!(unicode.format, Some(AudioFormat::Wav));
    assert_eq!(unicode.bit_depth, Some(24));
    assert_eq!(unicode.sample_rate, Some(96_000));
    assert_eq!(unicode.bitrate, Some(4_608));
    assert_eq!(unicode.album_artist.as_deref(), Some("Various Artists"));
    assert_eq!(unicode.compilation, Some(true));

    let soundtrack = by_path("Main Menu.flac");
    assert_eq!(soundtrack.album_artist.as_deref(), Some("RODIK"));
    assert_eq!(soundtrack.compilation, None, "NULL is not Some(false)");

    // The stamps v4 wrote are untouched, so the upgrade does not cost a
    // listener the incremental scan they already paid for.
    assert_eq!(
        library
            .known_files()
            .values()
            .filter(|known| known.stamp.is_some())
            .count(),
        5,
        "every v4 stamp survives, so the next scan is still incremental"
    );

    // The new columns are NULL for every row: nothing already in a v4
    // database implies a ReplayGain figure, and computing one means an EBU
    // R128 analysis pass that cannot happen inside a migration.
    for track in library.tracks() {
        assert!(
            track.replay_gain.is_empty(),
            "{}: an upgraded row declares no ReplayGain",
            track.path.display()
        );
    }

    // Grouping is *exactly* the pre-v5 behaviour — the upgrade adds a column,
    // never changes what the shelf shows.
    let albums = library.albums();
    let passage = albums
        .iter()
        .find(|a| a.title == Some("Northwest Passage"))
        .expect("the double rip");
    assert_eq!(passage.artist, AlbumArtist::Named("Stan Rogers"));
    assert_eq!(passage.editions.len(), 2);
    let gamerip: Vec<_> = albums
        .iter()
        .filter(|a| a.title == Some("Cookie's Bustle OST (gamerip)"))
        .collect();
    assert_eq!(gamerip.len(), 1, "still one entry, two editions");
    assert_eq!(gamerip[0].artist, AlbumArtist::Named("RODIK"));
    assert_eq!(gamerip[0].editions.len(), 2);
}

/// A rescan after the upgrade fills the new columns, and they are durable.
/// That is what makes the NULLs self-healing rather than permanent.
#[test]
fn the_first_rescan_after_a_v4_upgrade_stores_replay_gain() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("library.db");
    write_v4_database(&db);

    let mut library = Library::open(&db).expect("open migrates to v5");
    let tags = ReplayGainTags {
        track_gain_centidb: Some(-775),
        track_peak_micro: Some(988_525),
        album_gain_centidb: Some(-920),
        album_peak_micro: Some(1_001_221),
    };
    let rescanned: Vec<TrackMeta> = library
        .tracks()
        .cloned()
        .map(|meta| TrackMeta {
            replay_gain: tags,
            ..meta
        })
        .collect();
    library.add_tracks(rescanned).expect("rescan batch");
    assert_eq!(library.len(), 5, "an upsert, not a duplicate");

    drop(library);
    let reopened = Library::open(&db).expect("reopen");
    assert!(reopened.tracks().all(|t| t.replay_gain == tags));
}

/// The extremes of both units survive a real database file, and a column baz
/// never writes into degrades to "the file did not say" rather than failing
/// the open.
#[test]
fn replay_gain_round_trips_through_a_real_database_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("library.db");
    let original = TrackMeta {
        replay_gain: ReplayGainTags {
            track_gain_centidb: Some(i16::MIN),
            track_peak_micro: Some(u32::MAX),
            album_gain_centidb: Some(i16::MAX),
            album_peak_micro: Some(0),
        },
        ..track("/m/loud.flac", "Karl", "Signal Chain", "Test Tone", 1)
    };
    {
        let mut library = Library::open(&db).expect("open");
        library.add_tracks(vec![original.clone()]).expect("add");
    }
    let reopened = Library::open(&db).expect("reopen");
    assert_eq!(reopened.tracks().next().expect("one track"), &original);
    drop(reopened);

    // A value no baz could have written — a corrupt database, or one from a
    // future with a wider unit. It must read as absent, not abort the open.
    let conn = rusqlite::Connection::open(&db).expect("raw open");
    conn.execute(
        "UPDATE tracks SET rg_track_gain_centidb = 99999, rg_album_peak_micro = -1",
        [],
    )
    .expect("corrupt the row");
    drop(conn);

    let library = Library::open(&db).expect("a corrupt figure must not fail the open");
    let stored = library.tracks().next().expect("one track");
    assert_eq!(stored.replay_gain.track_gain_centidb, None);
    assert_eq!(stored.replay_gain.album_peak_micro, None);
    assert_eq!(
        stored.replay_gain.track_peak_micro,
        Some(u32::MAX),
        "the untouched figures are unaffected"
    );
}

// ---------------------------------------------------------------------------
// Schema v6: the ReplayGain figures baz measured itself (ADR-0015).
// ---------------------------------------------------------------------------

/// Build a genuine v5 database with the v5 schema and v5 `INSERT`s only — no
/// baz code involved, exactly as [`write_v4_database`] does for its own
/// version. This is the shape of the `library.db` an installed baz leaves on
/// disk today: file stamps, and the ReplayGain a scanner wrote into the files.
fn write_v5_database(db: &std::path::Path) {
    let conn = rusqlite::Connection::open(db).expect("create v5 db");
    conn.execute_batch(
        "
        BEGIN;
        CREATE TABLE tracks (
            id                    INTEGER PRIMARY KEY,
            path                  BLOB NOT NULL UNIQUE,
            artist                TEXT,
            album                 TEXT,
            title                 TEXT,
            track                 INTEGER,
            disc                  INTEGER,
            year                  INTEGER,
            duration_ns           INTEGER,
            format                TEXT,
            bit_depth             INTEGER,
            sample_rate           INTEGER,
            bitrate               INTEGER,
            album_artist          TEXT,
            compilation           INTEGER,
            mtime_ns              INTEGER,
            file_size             INTEGER,
            rg_track_gain_centidb INTEGER,
            rg_track_peak_micro   INTEGER,
            rg_album_gain_centidb INTEGER,
            rg_album_peak_micro   INTEGER
        ) STRICT;
        PRAGMA user_version = 5;
        COMMIT;
        ",
    )
    .expect("v5 schema");

    for (n, row) in v3_rows().into_iter().enumerate() {
        // Only the FLAC rip carries ReplayGain, which is what a half-scanned
        // library looks like: the tagged tracks must come through untouched and
        // the untagged ones must still read as untagged after the upgrade.
        let tagged = row.format == "flac";
        conn.execute(
            "INSERT INTO tracks
                 (path, artist, album, title, track, disc, year, duration_ns,
                  format, bit_depth, sample_rate, bitrate, album_artist,
                  compilation, mtime_ns, file_size,
                  rg_track_gain_centidb, rg_track_peak_micro,
                  rg_album_gain_centidb, rg_album_peak_micro)
             VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                     ?14, ?15, ?16, ?17, ?18, ?19)",
            rusqlite::params![
                // The platform's own path encoding, not UTF-8: a `library.db`
                // is a per-machine cache and Windows stores UTF-16LE, so a
                // fixture that hard-coded bytes would only be a *Unix* v5
                // database (see `stored_path_bytes`).
                stored_path_bytes(row.path),
                row.artist,
                row.album,
                row.title,
                row.track,
                row.year,
                row.duration_ns,
                row.format,
                row.bit_depth,
                row.sample_rate,
                row.bitrate,
                row.album_artist,
                row.compilation,
                1_700_000_000_000_000_000_i64 + i64::try_from(n).expect("five rows"),
                40_000_000_i64 + i64::try_from(n).expect("five rows"),
                tagged.then_some(-775_i64),
                tagged.then_some(988_525_i64),
                tagged.then_some(-920_i64),
                tagged.then_some(1_001_221_i64),
            ],
        )
        .expect("insert v5 row");
    }
}

#[test]
fn a_v5_database_migrates_in_place_without_losing_anything() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("library.db");
    write_v5_database(&db);

    let library = Library::open(&db).expect("a v5 database must open");
    assert_eq!(library.len(), 5, "every v5 row survives the upgrade");

    let conn = rusqlite::Connection::open(&db).expect("raw open");
    let version: i64 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("user_version");
    assert_eq!(version, 9);

    let by_path = |needle: &str| {
        library
            .tracks()
            .find(|t| t.path.to_string_lossy().contains(needle))
            .cloned()
            .unwrap_or_else(|| panic!("{needle} must survive"))
    };

    // Every v5 column is intact — text, numbers, Unicode, the ADR-0008
    // columns, the ADR-0010 stamp, and the ADR-0013 figures.
    let unicode = by_path("Größenwahn");
    assert_eq!(unicode.artist.as_deref(), Some("Größenwahn"));
    assert_eq!(unicode.album.as_deref(), Some("Debüt"));
    assert_eq!(unicode.title.as_deref(), Some("Ærø — 序曲"));
    assert_eq!(unicode.track, Some(3));
    assert_eq!(unicode.disc, Some(1));
    assert_eq!(unicode.year, Some(1999));
    assert_eq!(unicode.duration, Some(Duration::new(215, 123_456_789)));
    assert_eq!(unicode.format, Some(AudioFormat::Wav));
    assert_eq!(unicode.bit_depth, Some(24));
    assert_eq!(unicode.sample_rate, Some(96_000));
    assert_eq!(unicode.bitrate, Some(4_608));
    assert_eq!(unicode.album_artist.as_deref(), Some("Various Artists"));
    assert_eq!(unicode.compilation, Some(true));

    let soundtrack = by_path("Main Menu.flac");
    assert_eq!(soundtrack.album_artist.as_deref(), Some("RODIK"));
    assert_eq!(soundtrack.compilation, None, "NULL is not Some(false)");
    assert_eq!(
        soundtrack.replay_gain,
        ReplayGainTags {
            track_gain_centidb: Some(-775),
            track_peak_micro: Some(988_525),
            album_gain_centidb: Some(-920),
            album_peak_micro: Some(1_001_221),
        },
        "the tags a scanner wrote survive the upgrade byte for byte"
    );
    let untagged = by_path(".mp3");
    assert!(
        untagged.replay_gain.is_empty(),
        "and a row that carried no ReplayGain still carries none"
    );

    // The stamps v4 wrote are untouched, so the upgrade does not cost a
    // listener the incremental scan they already paid for.
    assert_eq!(
        library
            .known_files()
            .values()
            .filter(|known| known.stamp.is_some())
            .count(),
        5,
        "every v5 stamp survives, so the next scan is still incremental"
    );

    // The new columns are NULL for every row. Nothing in a v5 database implies
    // a *measured* loudness — the only way to know one is to decode every
    // sample of the file, which is what the background pass is for and could
    // certainly not happen inside a migration.
    for track in library.tracks() {
        assert!(
            library.computed_replay_gain(&track.path).is_empty(),
            "{}: an upgraded row carries no measurement",
            track.path.display()
        );
    }
    assert!(
        library.computed_gains().is_empty(),
        "and the engine's snapshot of an upgraded library is empty"
    );

    // Grouping is *exactly* the pre-v6 behaviour — the upgrade adds columns,
    // never changes what the shelf shows.
    let albums = library.albums();
    let passage = albums
        .iter()
        .find(|a| a.title == Some("Northwest Passage"))
        .expect("the double rip");
    assert_eq!(passage.artist, AlbumArtist::Named("Stan Rogers"));
    assert_eq!(passage.editions.len(), 2);
    let gamerip: Vec<_> = albums
        .iter()
        .filter(|a| a.title == Some("Cookie's Bustle OST (gamerip)"))
        .collect();
    assert_eq!(gamerip.len(), 1, "still one entry, two editions");
    assert_eq!(gamerip[0].editions.len(), 2);
}

/// An analysis after the upgrade fills the new columns, and they are durable.
/// That is what makes the NULLs self-healing rather than permanent — with a
/// different healer from v2–v5's, because a rescan cannot produce a
/// measurement.
#[test]
fn the_first_analysis_after_a_v5_upgrade_stores_what_it_measured() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("library.db");
    write_v5_database(&db);

    let mut library = Library::open(&db).expect("open migrates to v6");
    let paths: Vec<PathBuf> = library.tracks().map(|t| t.path.clone()).collect();
    let stamps: Vec<Option<FileStamp>> = library.tracks().map(|t| t.stamp).collect();
    let measured = ReplayGainTags {
        track_gain_centidb: Some(412),
        track_peak_micro: Some(750_000),
        album_gain_centidb: Some(318),
        album_peak_micro: Some(910_000),
    };
    let written = library
        .store_computed_replay_gain(paths.iter().cloned().zip(stamps.iter().copied()).map(
            |(path, stamp)| {
                (
                    path,
                    ComputedReplayGain {
                        figures: measured,
                        stamp,
                    },
                )
            },
        ))
        .expect("store");
    assert_eq!(written, 5);
    assert_eq!(library.len(), 5, "an update, not an insert");

    drop(library);
    let reopened = Library::open(&db).expect("reopen");
    for path in &paths {
        assert_eq!(
            reopened.computed_replay_gain(path),
            measured,
            "{}: the measurement is durable",
            path.display()
        );
    }
    assert_eq!(reopened.computed_gains().len(), 5);

    // The tags are untouched by the measurement: two claims, two column
    // groups, and the selection rule is what chooses between them.
    let flac = reopened
        .tracks()
        .find(|t| t.path.to_string_lossy().ends_with(".flac"))
        .expect("a FLAC row");
    assert_eq!(flac.replay_gain.track_gain_centidb, Some(-775));
}

/// A measurement whose file has moved on is reported as no measurement.
///
/// The stamp is the whole mechanism: a loudness figure is a claim about a
/// file's samples, and a file that has been re-encoded, re-tagged or replaced
/// is a file the claim is not about. It stays in the database — a later scan
/// may restore the very stamp it was taken at — but it does not reach a gain
/// stage in the meantime.
#[test]
fn a_measurement_of_a_file_that_has_since_changed_is_not_used() {
    let stamp = FileStamp {
        mtime_ns: 1_700_000_000_000_000_000,
        size: 40_000_000,
    };
    let mut library = Library::open_in_memory().expect("open");
    library
        .add_tracks(vec![TrackMeta {
            stamp: Some(stamp),
            ..track("/m/a.flac", "Karl", "Signal Chain", "Test Tone", 1)
        }])
        .expect("add");
    let measured = ReplayGainTags {
        track_gain_centidb: Some(412),
        track_peak_micro: Some(750_000),
        ..ReplayGainTags::default()
    };
    library
        .store_computed_replay_gain([(
            PathBuf::from("/m/a.flac"),
            ComputedReplayGain {
                figures: measured,
                stamp: Some(stamp),
            },
        )])
        .expect("store");
    assert_eq!(
        library.computed_replay_gain(std::path::Path::new("/m/a.flac")),
        measured
    );

    // The file is re-encoded: same path, different bytes, so the scan records
    // a different stamp.
    library
        .add_tracks(vec![TrackMeta {
            stamp: Some(FileStamp {
                mtime_ns: stamp.mtime_ns + 1,
                size: 41_000_000,
            }),
            ..track("/m/a.flac", "Karl", "Signal Chain", "Test Tone", 1)
        }])
        .expect("rescan");
    assert!(
        library
            .computed_replay_gain(std::path::Path::new("/m/a.flac"))
            .is_empty(),
        "a measurement of the old bytes must not be applied to the new ones"
    );
    assert!(library.computed_gains().is_empty());
}

/// A rescan does not destroy a measurement — which is the "must not fight the
/// incremental scanner" property, and it is held by the schema rather than by
/// two writers agreeing to be careful.
///
/// A scan produces a [`TrackMeta`], which has nowhere to put a measurement,
/// and the upsert names the tag columns and not the computed ones. So a file
/// that was re-read because its *tags* moved — the ordinary case after somebody
/// runs a tagger — keeps the loudness baz measured, provided the stamp still
/// matches.
#[test]
fn a_rescan_rewrites_tags_and_leaves_measurements_alone() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("library.db");
    let stamp = FileStamp {
        mtime_ns: 1_700_000_000_000_000_000,
        size: 40_000_000,
    };
    let scanned = TrackMeta {
        stamp: Some(stamp),
        ..track("/m/a.flac", "Karl", "Signal Chain", "Test Tone", 1)
    };
    let measured = ReplayGainTags {
        track_gain_centidb: Some(412),
        track_peak_micro: Some(750_000),
        album_gain_centidb: Some(318),
        album_peak_micro: Some(910_000),
    };
    {
        let mut library = Library::open(&db).expect("open");
        library.add_tracks(vec![scanned.clone()]).expect("scan");
        library
            .store_computed_replay_gain([(
                PathBuf::from("/m/a.flac"),
                ComputedReplayGain {
                    figures: measured,
                    stamp: Some(stamp),
                },
            )])
            .expect("store");

        // The same file, re-read because its title changed. Same stamp, so the
        // measurement still describes these samples.
        library
            .add_tracks(vec![TrackMeta {
                title: Some("Test Tone (2024 remaster tag fix)".to_owned()),
                replay_gain: ReplayGainTags {
                    track_gain_centidb: Some(-775),
                    ..ReplayGainTags::default()
                },
                ..scanned.clone()
            }])
            .expect("rescan");
        assert_eq!(
            library.computed_replay_gain(std::path::Path::new("/m/a.flac")),
            measured,
            "in RAM: a scan speaks about tags and must not touch a measurement"
        );
    }
    let reopened = Library::open(&db).expect("reopen");
    assert_eq!(
        reopened.computed_replay_gain(std::path::Path::new("/m/a.flac")),
        measured,
        "on disk: the upsert names the tag columns and not the computed ones"
    );
    let stored = reopened.tracks().next().expect("one track");
    assert_eq!(
        stored.title.as_deref(),
        Some("Test Tone (2024 remaster tag fix)"),
        "and the rescan did land"
    );
    assert_eq!(stored.replay_gain.track_gain_centidb, Some(-775));
}

/// A measurement for a path the library does not hold writes nothing and is
/// not an error: a file removed while a pass was running is news, not a fault.
#[test]
fn a_measurement_for_an_unknown_path_is_ignored() {
    let mut library = Library::open_in_memory().expect("open");
    let written = library
        .store_computed_replay_gain([(
            PathBuf::from("/m/gone.flac"),
            ComputedReplayGain::default(),
        )])
        .expect("store");
    assert_eq!(written, 0);
    assert!(library.computed_gains().is_empty());
}

// ---------------------------------------------------------------------------
// Group keys (ADR-0018): schema v7, and the five shelves the wall draws.
// ---------------------------------------------------------------------------

/// Seconds in a day — the unit the ledger counts in.
const DAY_S: u64 = 24 * 60 * 60;

/// The test's own clock in seconds, as the ledger records time.
fn now_unix_s() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("a clock after 1970")
        .as_secs()
}

/// The test's own clock, so nothing here depends on a baz-private helper.
fn now_ns() -> i64 {
    i64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("a clock after 1970")
            .as_nanos(),
    )
    .expect("a clock before 2262")
}

/// Build a genuine v6 database with the v6 schema and v6 `INSERT`s only — no
/// baz code involved — so the v7 upgrade is proved against a database this
/// build did not create.
fn write_v6_database(db: &std::path::Path) {
    let conn = rusqlite::Connection::open(db).expect("create v6 db");
    conn.execute_batch(
        "
        BEGIN;
        CREATE TABLE tracks (
            id                             INTEGER PRIMARY KEY,
            path                           BLOB NOT NULL UNIQUE,
            artist                         TEXT,
            album                          TEXT,
            title                          TEXT,
            track                          INTEGER,
            disc                           INTEGER,
            year                           INTEGER,
            duration_ns                    INTEGER,
            format                         TEXT,
            bit_depth                      INTEGER,
            sample_rate                    INTEGER,
            bitrate                        INTEGER,
            album_artist                   TEXT,
            compilation                    INTEGER,
            mtime_ns                       INTEGER,
            file_size                      INTEGER,
            rg_track_gain_centidb          INTEGER,
            rg_track_peak_micro            INTEGER,
            rg_album_gain_centidb          INTEGER,
            rg_album_peak_micro            INTEGER,
            rg_computed_track_gain_centidb INTEGER,
            rg_computed_track_peak_micro   INTEGER,
            rg_computed_album_gain_centidb INTEGER,
            rg_computed_album_peak_micro   INTEGER,
            rg_computed_mtime_ns           INTEGER,
            rg_computed_file_size          INTEGER
        ) STRICT;
        PRAGMA user_version = 6;
        COMMIT;
        ",
    )
    .expect("v6 schema");

    for (n, row) in v3_rows().into_iter().enumerate() {
        let tagged = row.format == "flac";
        let mtime = 1_700_000_000_000_000_000_i64 + i64::try_from(n).expect("five rows");
        let size = 40_000_000_i64 + i64::try_from(n).expect("five rows");
        conn.execute(
            "INSERT INTO tracks
                 (path, artist, album, title, track, disc, year, duration_ns,
                  format, bit_depth, sample_rate, bitrate, album_artist,
                  compilation, mtime_ns, file_size,
                  rg_track_gain_centidb, rg_track_peak_micro,
                  rg_album_gain_centidb, rg_album_peak_micro,
                  rg_computed_track_gain_centidb, rg_computed_track_peak_micro,
                  rg_computed_album_gain_centidb, rg_computed_album_peak_micro,
                  rg_computed_mtime_ns, rg_computed_file_size)
             VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                     ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25)",
            rusqlite::params![
                // The platform's own path encoding, not UTF-8: a `library.db`
                // is a per-machine cache and Windows stores UTF-16LE, so a
                // fixture that hard-coded bytes would only be a *Unix* v6
                // database (see `stored_path_bytes`).
                stored_path_bytes(row.path),
                row.artist,
                row.album,
                row.title,
                row.track,
                row.year,
                row.duration_ns,
                row.format,
                row.bit_depth,
                row.sample_rate,
                row.bitrate,
                row.album_artist,
                row.compilation,
                mtime,
                size,
                tagged.then_some(-775_i64),
                tagged.then_some(988_525_i64),
                tagged.then_some(-920_i64),
                tagged.then_some(1_001_221_i64),
                // A real measurement on the FLAC rips, stamped to match their
                // rows, so the upgrade can be shown not to disturb v6's own
                // columns either.
                tagged.then_some(412_i64),
                tagged.then_some(750_000_i64),
                tagged.then_some(318_i64),
                tagged.then_some(910_000_i64),
                tagged.then_some(mtime),
                tagged.then_some(size),
            ],
        )
        .expect("insert v6 row");
    }
}

#[test]
fn a_v6_database_migrates_in_place_without_losing_anything() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("library.db");
    write_v6_database(&db);

    let library = Library::open(&db).expect("a v6 database must open");
    assert_eq!(library.len(), 5, "every v6 row survives the upgrade");

    let conn = rusqlite::Connection::open(&db).expect("raw open");
    let version: i64 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("user_version");
    assert_eq!(version, 9);

    let by_path = |needle: &str| {
        library
            .tracks()
            .find(|t| t.path.to_string_lossy().contains(needle))
            .cloned()
            .unwrap_or_else(|| panic!("{needle} must survive"))
    };

    // Every v6 column is intact — text, numbers, Unicode, the ADR-0008
    // columns, the ADR-0010 stamp, the ADR-0013 tags and the ADR-0015
    // measurement.
    let unicode = by_path("Größenwahn");
    assert_eq!(unicode.artist.as_deref(), Some("Größenwahn"));
    assert_eq!(unicode.album.as_deref(), Some("Debüt"));
    assert_eq!(unicode.title.as_deref(), Some("Ærø — 序曲"));
    assert_eq!(unicode.track, Some(3));
    assert_eq!(unicode.disc, Some(1));
    assert_eq!(unicode.year, Some(1999));
    assert_eq!(unicode.duration, Some(Duration::new(215, 123_456_789)));
    assert_eq!(unicode.format, Some(AudioFormat::Wav));
    assert_eq!(unicode.bit_depth, Some(24));
    assert_eq!(unicode.sample_rate, Some(96_000));
    assert_eq!(unicode.bitrate, Some(4_608));
    assert_eq!(unicode.album_artist.as_deref(), Some("Various Artists"));
    assert_eq!(unicode.compilation, Some(true));

    let soundtrack = by_path("Main Menu.flac");
    assert_eq!(soundtrack.album_artist.as_deref(), Some("RODIK"));
    assert_eq!(soundtrack.compilation, None, "NULL is not Some(false)");
    assert_eq!(soundtrack.replay_gain.track_gain_centidb, Some(-775));
    assert_eq!(
        library
            .computed_replay_gain(&soundtrack.path)
            .track_gain_centidb,
        Some(412),
        "the measurement v6 stored is still fresh for the same file"
    );
    assert_eq!(
        library
            .known_files()
            .values()
            .filter(|known| known.stamp.is_some())
            .count(),
        5,
        "every v6 stamp survives, so the next scan is still incremental"
    );

    // The two new columns are NULL for every row, and they mean two different
    // things. A genre lives in the file's tags and nowhere else, so nothing in
    // a v6 database implies one...
    for track in library.tracks() {
        assert_eq!(
            track.genre,
            None,
            "{}: an upgraded row declares no genre",
            track.path.display()
        );
    }
    assert_eq!(
        library.shelves(GroupKey::Genre).len(),
        1,
        "so the whole upgraded library is on the untagged genre shelf"
    );
    // ...and baz genuinely does not know when these files arrived, so it says
    // so rather than stamping them with the moment of the upgrade.
    for album in library.albums() {
        assert_eq!(album.first_seen_ns, None);
    }
    let added = library.shelves(GroupKey::Added);
    assert_eq!(added.len(), 1);
    assert_eq!(
        added[0].header,
        GroupHeader::Recency(Recency::Unrecorded),
        "a migrated library is honest about not knowing, not reported as new"
    );

    // Grouping is *exactly* the pre-v7 behaviour — the upgrade adds columns,
    // never changes what the shelf shows.
    let albums = library.albums();
    let passage = albums
        .iter()
        .find(|a| a.title == Some("Northwest Passage"))
        .expect("the double rip");
    assert_eq!(passage.artist, AlbumArtist::Named("Stan Rogers"));
    assert_eq!(passage.editions.len(), 2);
    let gamerip: Vec<_> = albums
        .iter()
        .filter(|a| a.title == Some("Cookie's Bustle OST (gamerip)"))
        .collect();
    assert_eq!(gamerip.len(), 1, "still one entry, two editions");
    assert_eq!(gamerip[0].artist, AlbumArtist::Named("RODIK"));
    assert_eq!(gamerip[0].editions.len(), 2);
}

/// The two v7 columns heal differently, and both behaviours are load-bearing.
///
/// A rescan fills `genre`, because a genre is a tag and a scan reads tags. It
/// does **not** fill `first_seen_ns` for a row that was already there: "when
/// did this arrive" is not a fact a rescan can discover, and the row predates
/// the column.
#[test]
fn a_rescan_after_a_v6_upgrade_fills_the_genre_and_never_invents_a_first_seen() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("library.db");
    write_v6_database(&db);

    let mut library = Library::open(&db).expect("open migrates to v7");
    let rescanned: Vec<TrackMeta> = library
        .tracks()
        .cloned()
        .map(|meta| TrackMeta {
            genre: Some("Post-Rock".to_owned()),
            ..meta
        })
        .collect();
    library.add_tracks(rescanned).expect("rescan batch");
    assert_eq!(library.len(), 5, "an upsert, not a duplicate");

    drop(library);
    let reopened = Library::open(&db).expect("reopen");
    assert!(
        reopened
            .tracks()
            .all(|t| t.genre.as_deref() == Some("Post-Rock")),
        "the genre a rescan read is durable"
    );
    for album in reopened.albums() {
        assert_eq!(
            album.first_seen_ns, None,
            "a rescan is not an arrival: the pre-v7 NULL stays NULL"
        );
    }
    assert_eq!(
        reopened.shelves(GroupKey::Added)[0].header,
        GroupHeader::Recency(Recency::Unrecorded)
    );
}

/// A track a v7 library has never seen is stamped on insert, and every later
/// rescan of the same path leaves that stamp exactly where it was — across a
/// restart, which is the case an in-RAM-only guarantee would miss.
#[test]
fn first_seen_is_written_once_and_no_rescan_can_move_it() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("library.db");

    let mut library = Library::open(&db).expect("open");
    library
        .add_tracks([track(
            "/m/a/b/01.flac",
            "Stan Rogers",
            "Fogarty's Cove",
            "Fogarty's Cove",
            1,
        )])
        .expect("first scan");
    let first = library.albums()[0]
        .first_seen_ns
        .expect("a new row is stamped");
    assert!(
        (now_ns() - first).abs() < 60 * 1_000_000_000,
        "and stamped with roughly now, not with the epoch"
    );

    // A rescan that changes the tags — the ordinary case after a tag fix.
    library
        .add_tracks([TrackMeta {
            title: Some("Fogarty's Cove (2024 tag fix)".to_owned()),
            genre: Some("Folk".to_owned()),
            ..track("/m/a/b/01.flac", "Stan Rogers", "Fogarty's Cove", "x", 1)
        }])
        .expect("rescan");
    assert_eq!(library.len(), 1);
    assert_eq!(
        library.albums()[0].first_seen_ns,
        Some(first),
        "a rescan rewrites the tags and never the arrival"
    );

    drop(library);
    let reopened = Library::open(&db).expect("reopen");
    assert_eq!(
        reopened.albums()[0].first_seen_ns,
        Some(first),
        "and the database agrees with the in-RAM index across a restart"
    );
    assert_eq!(reopened.albums()[0].genre, Some("Folk"));
}

/// A track that arrives later gets its own stamp: ADDED is per row, not per
/// library. And an album is dated by its *earliest* track, so a record you
/// have owned for years does not become new because one more file of it
/// landed.
#[test]
fn a_track_added_later_carries_a_later_first_seen() {
    let mut library = Library::open_in_memory().expect("open");
    library
        .add_tracks([track("/m/a/b/01.flac", "A", "First", "One", 1)])
        .expect("first");
    let first = library.albums()[0].first_seen_ns.expect("stamped");
    std::thread::sleep(std::time::Duration::from_millis(5));
    library
        .add_tracks([track("/m/c/d/01.flac", "C", "Second", "One", 1)])
        .expect("second");
    let albums = library.albums();
    let second = albums
        .iter()
        .find(|a| a.title == Some("Second"))
        .expect("second album")
        .first_seen_ns
        .expect("stamped");
    assert!(second > first, "{second} must be later than {first}");

    library
        .add_tracks([track("/m/a/b/02.flac", "A", "First", "Two", 2)])
        .expect("late track");
    let albums = library.albums();
    let grown = albums
        .iter()
        .find(|a| a.title == Some("First"))
        .expect("first album");
    assert_eq!(grown.first_seen_ns, Some(first));
}

/// **A–Z and ARTIST are two densities of one order** — the same traversal,
/// broken into 27 letter shelves or into one shelf per artist (ADR-0035's
/// third amendment).
///
/// That identity was the argument for deleting `A–Z`, and it is now the
/// argument for keeping both: the owner uses a wall of letter shelves and a
/// wall of artist shelves differently, and neither can put a record somewhere
/// the other would not. So it is asserted from both ends — the two keys
/// flatten to the *same* list in the *same* order, and their headers differ
/// only in where the breaks fall.
#[test]
fn the_alphabet_key_is_the_artist_key_with_coarser_breaks() {
    let mut library = Library::open_in_memory().expect("open");
    library
        .add_tracks([
            track("/m/1.flac", "Stan Rogers", "Northwest Passage", "T", 1),
            track("/m/2.flac", "Sibylle Baier", "Colour Green", "T", 1),
            track("/m/3.flac", "10cc", "Sheet Music", "T", 1),
            track("/m/4.flac", "Aphex Twin", "Drukqs", "T", 1),
            TrackMeta {
                compilation: Some(true),
                ..track("/m/5.flac", "Someone", "A Compilation", "T", 1)
            },
            bare("/m/6.flac"),
        ])
        .expect("add");

    let coarse = library.shelves(GroupKey::Alphabet);
    let fine = library.shelves(GroupKey::Artist);

    // The coarse wall: the two anonymous ends, `#`, and one shelf per letter —
    // with Sibylle Baier and Stan Rogers sharing `S`, which is the whole
    // difference between the two arrangements.
    assert_eq!(
        coarse.iter().map(|s| s.header.label()).collect::<Vec<_>>(),
        ["Unknown", "#", "A", "S", "Various"]
    );
    assert_eq!(
        fine.iter().map(|s| s.header.label()).collect::<Vec<_>>(),
        [
            "Unknown",
            "10cc",
            "Aphex Twin",
            "Sibylle Baier",
            "Stan Rogers",
            "Various"
        ]
    );
    assert_eq!(coarse[3].albums.len(), 2, "one S shelf holds both");

    // And under the headers it is one wall. `albums()` is the same list a
    // third time, which is what makes *both* keys "the flat shelf with its
    // breaks named" rather than one being a second traversal.
    let flatten = |shelves: &[baz_core::index::Shelf<'_>]| -> Vec<String> {
        shelves
            .iter()
            .flat_map(|shelf| shelf.albums.iter().map(|a| format!("{:?}", a.title)))
            .collect()
    };
    assert_eq!(flatten(&coarse), flatten(&fine));
    assert_eq!(
        flatten(&coarse),
        library
            .albums()
            .iter()
            .map(|a| format!("{:?}", a.title))
            .collect::<Vec<_>>()
    );

    // The coarse key's headers are `Initial`s — the type the rail also speaks,
    // asked of `baz-core` in both places so the wall's letters and the rail's
    // letters cannot disagree.
    assert_eq!(coarse[2].header, GroupHeader::Initial(Initial::Letter('A')));
    assert_eq!(coarse[1].header, GroupHeader::Initial(Initial::Other));
    assert_eq!(coarse[0].header, GroupHeader::Initial(Initial::Unknown));
    assert_eq!(coarse[4].header, GroupHeader::Initial(Initial::Various));
}

/// ARTIST is the shelf ADR-0008 built, with **one break per artist** stated
/// (ADR-0035). The albums and their order must be identical to `albums()`, or
/// two views of one library would disagree — which is also the whole argument
/// that grouping by the artist replaced grouping by their initial rather than
/// joining it: the finer headers name breaks that were already there.
#[test]
fn the_artist_key_is_the_flat_shelf_with_its_breaks_named() {
    let mut library = Library::open_in_memory().expect("open");
    library
        .add_tracks([
            track(
                "/m/1.flac",
                "Stan Rogers",
                "Fogarty's Cove",
                "Fogarty's Cove",
                1,
            ),
            track(
                "/m/2.flac",
                "10cc",
                "Sheet Music",
                "The Wall Street Shuffle",
                1,
            ),
            track("/m/3.flac", "Sibylle Baier", "Colour Green", "Tonight", 1),
            TrackMeta {
                compilation: Some(true),
                ..track("/m/4.flac", "Someone", "A Compilation", "Track", 1)
            },
            bare("/m/5.flac"),
        ])
        .expect("add");

    let shelves = library.shelves(GroupKey::Artist);
    let headers: Vec<String> = shelves.iter().map(|s| s.header.label()).collect();
    assert_eq!(
        headers,
        ["Unknown", "10cc", "Sibylle Baier", "Stan Rogers", "Various"],
        "unknowns first, then the artists case-folded alphabetically, then \
         the unnamed compilations — the ends of the shelf ADR-0008 chose, \
         with every artist between them named"
    );
    // Stan Rogers and Sibylle Baier shared the `S` shelf before ADR-0035;
    // now they have one each, and each holds only their own records.
    assert_eq!(
        shelves[2]
            .albums
            .iter()
            .map(|a| a.title)
            .collect::<Vec<_>>(),
        [Some("Colour Green")]
    );
    assert_eq!(
        shelves[3]
            .albums
            .iter()
            .map(|a| a.title)
            .collect::<Vec<_>>(),
        [Some("Fogarty's Cove")]
    );

    let flat: Vec<Option<&str>> = library.albums().iter().map(|a| a.title).collect();
    let from_shelves: Vec<Option<&str>> = shelves
        .iter()
        .flat_map(|shelf| shelf.albums.iter().map(|a| a.title))
        .collect();
    assert_eq!(flat, from_shelves, "same albums, same order, breaks named");
}

/// **The shelves are the artists, in the library's own order, and each holds
/// its records alphabetically** — the three orderings ADR-0035 fixed, in one
/// library that exercises all of them.
///
/// The shelf order is `ArtistKey`'s: unknowns first, then names case-folded,
/// then unnamed compilations. Within a shelf it is library order, which is
/// album title — the rule ADR-0019 §1 set for every key, kept here rather than
/// swapped for release year, because a second ordering *within* a shelf would
/// be a second arrangement control that nothing on screen explains.
#[test]
fn the_artist_shelves_order_their_artists_and_their_records() {
    let mut library = Library::open_in_memory().expect("open");
    library
        .add_tracks([
            track("/m/1.flac", "Corvin", "Zenith", "T", 1),
            track("/m/2.flac", "Corvin", "Aurora", "T", 1),
            track("/m/3.flac", "anne-marie puig", "Solo", "T", 1),
            TrackMeta {
                compilation: Some(true),
                ..track("/m/4.flac", "Someone", "A Compilation", "T", 1)
            },
            bare("/m/5.flac"),
        ])
        .expect("add");

    let shelves = library.shelves(GroupKey::Artist);
    assert_eq!(
        shelves.iter().map(|s| s.header.label()).collect::<Vec<_>>(),
        ["Unknown", "anne-marie puig", "Corvin", "Various"],
        "case-folded, so a lower-case tag sorts among the names rather than \
         after them"
    );
    assert_eq!(
        shelves[2]
            .albums
            .iter()
            .map(|a| a.title)
            .collect::<Vec<_>>(),
        [Some("Aurora"), Some("Zenith")],
        "an artist's records read alphabetically, not by year"
    );
}

/// **A shelf is headed by the spelling that sorts first**, not by the first
/// one found: identity is case-folded, so one artist with two spellings on
/// disk must not be named by whichever of their records happens to sort first
/// by title. This is the same rule the front end's `views::artist::label`
/// applies, which is what stops a header and the page it opens from naming one
/// artist two ways.
#[test]
fn an_artist_shelf_is_headed_by_the_spelling_that_sorts_first() {
    let mut library = Library::open_in_memory().expect("open");
    library
        .add_tracks([
            // The lower-case spelling is on the album that sorts *first* by
            // title, so "first found" and "sorts first" disagree here.
            track("/m/1.flac", "aphex twin", "Ambient Works", "T", 1),
            track("/m/2.flac", "Aphex Twin", "Drukqs", "T", 1),
        ])
        .expect("add");

    let shelves = library.shelves(GroupKey::Artist);
    assert_eq!(shelves.len(), 1, "two spellings, one artist");
    assert_eq!(shelves[0].header.label(), "Aphex Twin");
    assert_eq!(shelves[0].albums.len(), 2);
}

/// Non-ASCII names get their own letter rather than being swept onto `#`: a
/// rail that folded every script together would fail the library that needs
/// it most. [`Initial`] is the rail's vocabulary since ADR-0035 — the wall's
/// own headers are the artists — and this is the property that made it worth
/// keeping when it stopped being a header.
#[test]
fn the_artist_rail_keeps_every_script_it_is_given() {
    let mut library = Library::open_in_memory().expect("open");
    library
        .add_tracks([
            track(
                "/m/1.flac",
                "Ólafur Arnalds",
                "Island Songs",
                "Árbakkinn",
                1,
            ),
            track("/m/2.flac", "曲人", "序曲", "序", 1),
            track("/m/3.flac", "!!!", "Louden Up Now", "Me and Giuliani", 1),
        ])
        .expect("add");

    let shelves = library.shelves(GroupKey::Artist);
    let headers: Vec<String> = shelves.iter().map(|s| s.header.label()).collect();
    assert_eq!(headers, ["!!!", "Ólafur Arnalds", "曲人"]);

    // …and the rail's letters for those same shelves, in the same order.
    let rail: Vec<String> = shelves
        .iter()
        .map(|shelf| match shelf.header {
            GroupHeader::Artist(artist) => Initial::of(artist).label(),
            ref other => panic!("the artist key headers artists, not {other:?}"),
        })
        .collect();
    assert_eq!(rail, ["#", "Ó", "曲"]);
}

/// YEAR shelves by decade, oldest first, with the albums that declare no year
/// at the front — the same "unknowns surface rather than hide" rule the rest
/// of the index follows.
#[test]
fn the_year_key_shelves_by_decade_with_the_undated_at_the_front() {
    let mut library = Library::open_in_memory().expect("open");
    library
        .add_tracks([
            TrackMeta {
                year: Some(1981),
                ..track("/m/1.flac", "Stan Rogers", "Northwest Passage", "T", 1)
            },
            TrackMeta {
                year: Some(1989),
                ..track("/m/2.flac", "The Cure", "Disintegration", "T", 1)
            },
            TrackMeta {
                year: Some(1990),
                ..track("/m/3.flac", "Cocteau Twins", "Heaven or Las Vegas", "T", 1)
            },
            TrackMeta {
                year: Some(2026),
                ..track("/m/4.flac", "Nobody", "Brand New", "T", 1)
            },
            track("/m/5.flac", "Undated", "No Year At All", "T", 1),
        ])
        .expect("add");

    let shelves = library.shelves(GroupKey::Year);
    let headers: Vec<String> = shelves.iter().map(|s| s.header.label()).collect();
    assert_eq!(headers, ["No year", "1980s", "1990s", "2020s"]);
    assert_eq!(shelves[0].albums[0].title, Some("No Year At All"));
    assert_eq!(shelves[1].albums.len(), 2, "1981 and 1989 share a decade");
    assert_eq!(
        shelves[1].header,
        GroupHeader::Decade(Some(1980)),
        "the header carries the decade, not a rendered string"
    );
}

/// GENRE is verbatim. Messy tags show, honestly: there is no mapping table and
/// nothing is merged that the files did not spell the same way.
#[test]
fn the_genre_key_shows_the_tags_exactly_as_they_are() {
    let mut library = Library::open_in_memory().expect("open");
    library
        .add_tracks([
            TrackMeta {
                genre: Some("Post-Rock".to_owned()),
                ..track("/m/1.flac", "A", "One", "T", 1)
            },
            TrackMeta {
                genre: Some("post rock".to_owned()),
                ..track("/m/2.flac", "B", "Two", "T", 1)
            },
            TrackMeta {
                genre: Some("Rock; Instrumental".to_owned()),
                ..track("/m/3.flac", "C", "Three", "T", 1)
            },
            // Same genre, different capitalisation: one shelf, because two
            // shelves that read identically on screen would be a bug rather
            // than honesty. This is the case-folding artist and album titles
            // already get, and it is the *only* thing done to a genre.
            TrackMeta {
                genre: Some("Folk".to_owned()),
                ..track("/m/4.flac", "D", "Four", "T", 1)
            },
            TrackMeta {
                genre: Some("folk".to_owned()),
                ..track("/m/5.flac", "E", "Five", "T", 1)
            },
            track("/m/6.flac", "F", "Six", "T", 1),
        ])
        .expect("add");

    let shelves = library.shelves(GroupKey::Genre);
    let headers: Vec<String> = shelves.iter().map(|s| s.header.label()).collect();
    assert_eq!(
        headers,
        [
            "No genre",
            "Folk",
            "post rock",
            "Post-Rock",
            "Rock; Instrumental"
        ],
        "no normalisation, no mapping, no splitting on `;` — and the untagged \
         shelf is at the front where it can be seen and fixed"
    );
    // The shelf that merged two spellings keeps the first one seen, verbatim.
    assert_eq!(shelves[1].header, GroupHeader::Genre(Some("Folk")));
    assert_eq!(shelves[1].albums.len(), 2);
    assert_eq!(shelves[0].albums[0].title, Some("Six"));
}

/// An album's genre is the first its tracks declare, as its year is. A record
/// whose tracks disagree still lands on a shelf rather than falling through to
/// "no genre".
#[test]
fn an_album_takes_the_first_genre_its_tracks_declare() {
    let mut library = Library::open_in_memory().expect("open");
    library
        .add_tracks([
            TrackMeta {
                genre: None,
                ..track("/m/1.flac", "A", "Mixed", "First", 1)
            },
            TrackMeta {
                genre: Some("Dub".to_owned()),
                ..track("/m/2.flac", "A", "Mixed", "Second", 2)
            },
            TrackMeta {
                genre: Some("Ska".to_owned()),
                ..track("/m/3.flac", "A", "Mixed", "Third", 3)
            },
        ])
        .expect("add");
    assert_eq!(library.albums()[0].genre, Some("Dub"));
    let shelves = library.shelves(GroupKey::Genre);
    assert_eq!(shelves.len(), 1);
    assert_eq!(shelves[0].header, GroupHeader::Genre(Some("Dub")));
}

/// A real ledger file holding the given plays, read back through the real
/// reader — no test double, so what PLAYED shelves is what a listener's own
/// `history.tsv` would shelve.
fn history_of(dir: &std::path::Path, plays: &[(&str, u64)]) -> History {
    let path = dir.join("history.tsv");
    {
        let ledger = HistoryLedger::open(&path).expect("open ledger");
        for (track, started_unix_s) in plays {
            // Built through the ledger's own constructor, so the record is
            // classified a play by the same rule real playback is.
            let record = PlayRecord::new(
                PathBuf::from(track),
                *started_unix_s,
                240_000,
                Some(240_000),
            )
            .expect("a full listen is a play");
            ledger.record(record, None);
        }
    }
    History::read(&path).expect("read ledger")
}

/// Without a ledger, PLAYED is answerable and its answer is true: nothing has
/// been played, so everything is on the NEVER PLAYED shelf.
#[test]
fn played_without_a_ledger_is_one_never_played_shelf() {
    let mut library = Library::open_in_memory().expect("open");
    library
        .add_tracks([
            track("/m/1.flac", "A", "One", "T", 1),
            track("/m/2.flac", "B", "Two", "T", 1),
        ])
        .expect("add");
    // No ledger at all — which is every baz today, since `crates/baz` does not
    // call `set_history` yet.
    let shelves = library.shelves(GroupKey::Played);
    assert_eq!(shelves.len(), 1);
    assert_eq!(shelves[0].header, GroupHeader::Recency(Recency::Never));
    assert_eq!(shelves[0].header.label(), "Never played");
    assert_eq!(shelves[0].albums.len(), 2);

    // A ledger that exists and is empty — a listener who has opened baz and
    // not yet pressed play — is the same answer, and it is not a fallback:
    // nothing has been played, so nothing has been played.
    let dir = tempfile::tempdir().expect("tempdir");
    let empty = history_of(dir.path(), &[]);
    assert_eq!(
        library.shelves_with_history(GroupKey::Played, Some(&empty)),
        shelves
    );
}

/// With a ledger, PLAYED buckets by recency and still ends on NEVER PLAYED.
/// An album's moment is the most recent of *any* of its tracks in *any*
/// edition: playing the phone copy is playing the record.
#[test]
fn played_buckets_by_recency_and_ends_on_never() {
    let mut library = Library::open_in_memory().expect("open");
    library
        .add_tracks([
            encoded(
                "/m/FLAC/1.flac",
                "Northwest Passage",
                "T",
                1,
                AudioFormat::Flac,
                900,
            ),
            encoded(
                "/m/MP3/1.mp3",
                "Northwest Passage",
                "T",
                1,
                AudioFormat::Mp3,
                320,
            ),
            track("/m/2.flac", "Aardvark", "Last Winter", "T", 1),
            track("/m/3.flac", "Zebra", "Untouched", "T", 1),
        ])
        .expect("add");

    let now = now_unix_s();
    let dir = tempfile::tempdir().expect("tempdir");
    let history = history_of(
        dir.path(),
        &[
            // Only the *lossy* edition was played, and only an hour ago.
            ("/m/MP3/1.mp3", now - 3_600),
            ("/m/2.flac", now - 200 * DAY_S),
        ],
    );

    let shelves = library.shelves_with_history(GroupKey::Played, Some(&history));
    let headers: Vec<String> = shelves.iter().map(|s| s.header.label()).collect();
    assert_eq!(
        headers,
        ["This evening", "6 months ago", "Never played"],
        "the ledger's own bucket vocabulary, in the ledger's own order"
    );
    assert_eq!(shelves[0].albums[0].title, Some("Northwest Passage"));
    assert_eq!(shelves[1].albums[0].title, Some("Last Winter"));
    assert_eq!(shelves[2].albums[0].title, Some("Untouched"));

    // The history is consulted for PLAYED and for nothing else.
    assert_eq!(
        library.shelves_with_history(GroupKey::Artist, Some(&history)),
        library.shelves(GroupKey::Artist)
    );
}

/// Whatever the key, every album the library holds appears exactly once. A
/// group key is a projection, never a filter — a wall that quietly dropped
/// your untagged records would be a wall you could not trust.
#[test]
fn every_key_shelves_every_album_exactly_once() {
    let mut library = Library::open_in_memory().expect("open");
    library
        .add_tracks([
            TrackMeta {
                year: Some(1981),
                genre: Some("Folk".to_owned()),
                ..track("/m/1.flac", "Stan Rogers", "Northwest Passage", "T", 1)
            },
            track("/m/2.flac", "10cc", "Sheet Music", "T", 1),
            bare("/m/3.flac"),
            TrackMeta {
                compilation: Some(true),
                ..track("/m/4.flac", "Someone", "A Compilation", "T", 1)
            },
        ])
        .expect("add");

    let expected: Vec<Option<&str>> = library.albums().iter().map(|a| a.title).collect();
    assert_eq!(expected.len(), 4);
    for key in GroupKey::ALL {
        let shelves = library.shelves(key);
        assert!(!shelves.is_empty(), "{key:?} must draw at least one shelf");
        let mut titles: Vec<Option<&str>> = shelves
            .iter()
            .flat_map(|shelf| {
                assert!(!shelf.albums.is_empty(), "{key:?}: no empty shelves");
                shelf.albums.iter().map(|a| a.title)
            })
            .collect();
        titles.sort_unstable();
        let mut wanted = expected.clone();
        wanted.sort_unstable();
        assert_eq!(titles, wanted, "{key:?} lost or duplicated an album");
    }
}

/// ADDED and PLAYED speak the *same* vocabulary — the ledger's [`Recency`] —
/// and the ADDED key needs exactly one bucket a play ledger never produces:
/// `Unrecorded`, for a row that predates first-seen. The bands in between are
/// `history::bucket`'s and are tested there rather than a second time here,
/// which is the whole reason there is only one set of them.
#[test]
fn added_and_played_share_one_bucket_vocabulary() {
    let mut fresh = Library::open_in_memory().expect("open");
    fresh
        .add_tracks([track("/m/1.flac", "A", "Just Imported", "T", 1)])
        .expect("add");
    assert_eq!(
        fresh.shelves(GroupKey::Added)[0].header,
        GroupHeader::Recency(Recency::ThisEvening),
        "a folder imported a moment ago is on the most recent shelf"
    );

    // The one bucket ADDED needs that a play ledger never yields, and the one
    // it must never be confused with.
    assert_ne!(Recency::Unrecorded, Recency::Never);
    assert!(
        Recency::Never < Recency::Unrecorded,
        "and a shelf baz knows nothing about sits behind one it does"
    );
    assert_eq!(Recency::Unrecorded.label(), "Not recorded");
    assert_eq!(Recency::Never.label(), "Never played");
}

/// The persisted code for the active key round-trips, and a code from a newer
/// baz degrades to `None` rather than failing a launch.
#[test]
fn group_key_codes_round_trip() {
    for key in GroupKey::ALL {
        assert_eq!(GroupKey::from_code(key.code()), Some(key));
    }
    let mut codes: Vec<&str> = GroupKey::ALL.iter().map(|k| k.code()).collect();
    codes.sort_unstable();
    codes.dedup();
    assert_eq!(codes.len(), GroupKey::ALL.len());
    assert_eq!(GroupKey::from_code("crates"), None);
    assert_eq!(GroupKey::from_code(""), None);

    // **The row's own order, and every word true of the key under it**
    // (ADR-0035's third amendment): `A–Z` first, `ARTIST` second.
    assert_eq!(GroupKey::ALL[0].label(), "A–Z");
    assert_eq!(GroupKey::ALL[1].label(), "Artist");
    assert_eq!(
        GroupKey::ALL.map(GroupKey::label),
        ["A–Z", "Artist", "Year", "Genre", "Added", "Played"]
    );

    // **`"artist"` is not given back to the key it used to name.** It meant
    // *group by the artist's initial* until ADR-0035 and *group by the artist*
    // after it — the one silent repurposing in this method's history — so
    // handing it to `A–Z` now would make one word name three things in three
    // releases. `A–Z` is `"alphabet"`, which no baz has ever written, and
    // `"artist"` keeps the meaning it has had since ADR-0035.
    assert_eq!(GroupKey::Alphabet.code(), "alphabet");
    assert_eq!(GroupKey::from_code("alphabet"), Some(GroupKey::Alphabet));
    assert_eq!(GroupKey::from_code("artist"), Some(GroupKey::Artist));
    assert_ne!(GroupKey::Alphabet.code(), GroupKey::Artist.code());
    // The other four are untouched, which is what "never change an existing
    // code" means when a key is *added*: nothing on disk changes meaning.
    for (key, code) in [
        (GroupKey::Year, "year"),
        (GroupKey::Genre, "genre"),
        (GroupKey::Added, "added"),
        (GroupKey::Played, "played"),
    ] {
        assert_eq!(key.code(), code);
    }
    // The en dash is the label's, and it is deliberately *not* the code: a
    // code is typed by hand into a config file.
    assert_eq!(GroupKey::from_code("a–z"), None);
    assert_eq!(GroupKey::from_code("a-z"), None);
}

// ---------------------------------------------------------------------------
// Schema v8: roots as first-class (ADR-0022)
// ---------------------------------------------------------------------------

/// The moment the v7 fixture's rows were first seen — an hour before the
/// migration runs, so a backfill that stamped "now" would be visible.
const V7_FIRST_SEEN_NS: i64 = 1_750_000_000_000_000_000;

/// Build a genuine v7 database with the v7 schema and v7 `INSERT`s only — no
/// baz code involved — so the v8 upgrade is proved against a database this
/// build did not create.
///
/// It carries everything v1 – v7 ever added: the double rip, the soundtrack,
/// a real `Various Artists` tag, a real compilation flag, non-ASCII paths and
/// titles, stamps, tagged and measured ReplayGain, genres and first-seen
/// timestamps.
fn write_v7_database(db: &std::path::Path) {
    let conn = rusqlite::Connection::open(db).expect("create v7 db");
    conn.execute_batch(
        "
        BEGIN;
        CREATE TABLE tracks (
            id                             INTEGER PRIMARY KEY,
            path                           BLOB NOT NULL UNIQUE,
            artist                         TEXT,
            album                          TEXT,
            title                          TEXT,
            track                          INTEGER,
            disc                           INTEGER,
            year                           INTEGER,
            duration_ns                    INTEGER,
            format                         TEXT,
            bit_depth                      INTEGER,
            sample_rate                    INTEGER,
            bitrate                        INTEGER,
            album_artist                   TEXT,
            compilation                    INTEGER,
            mtime_ns                       INTEGER,
            file_size                      INTEGER,
            rg_track_gain_centidb          INTEGER,
            rg_track_peak_micro            INTEGER,
            rg_album_gain_centidb          INTEGER,
            rg_album_peak_micro            INTEGER,
            rg_computed_track_gain_centidb INTEGER,
            rg_computed_track_peak_micro   INTEGER,
            rg_computed_album_gain_centidb INTEGER,
            rg_computed_album_peak_micro   INTEGER,
            rg_computed_mtime_ns           INTEGER,
            rg_computed_file_size          INTEGER,
            genre                          TEXT,
            first_seen_ns                  INTEGER
        ) STRICT;
        PRAGMA user_version = 7;
        COMMIT;
        ",
    )
    .expect("v7 schema");

    for (n, row) in v3_rows().into_iter().enumerate() {
        let tagged = row.format == "flac";
        let mtime = 1_700_000_000_000_000_000_i64 + i64::try_from(n).expect("five rows");
        let size = 40_000_000_i64 + i64::try_from(n).expect("five rows");
        conn.execute(
            "INSERT INTO tracks
                 (path, artist, album, title, track, disc, year, duration_ns,
                  format, bit_depth, sample_rate, bitrate, album_artist,
                  compilation, mtime_ns, file_size,
                  rg_track_gain_centidb, rg_track_peak_micro,
                  rg_album_gain_centidb, rg_album_peak_micro,
                  rg_computed_track_gain_centidb, rg_computed_track_peak_micro,
                  rg_computed_album_gain_centidb, rg_computed_album_peak_micro,
                  rg_computed_mtime_ns, rg_computed_file_size,
                  genre, first_seen_ns)
             VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                     ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25,
                     ?26, ?27)",
            rusqlite::params![
                // The platform's own path encoding, not UTF-8: a `library.db`
                // is a per-machine cache and Windows stores UTF-16LE, so a
                // fixture that hard-coded bytes would only be a *Unix* v7
                // database (see `stored_path_bytes`).
                stored_path_bytes(row.path),
                row.artist,
                row.album,
                row.title,
                row.track,
                row.year,
                row.duration_ns,
                row.format,
                row.bit_depth,
                row.sample_rate,
                row.bitrate,
                row.album_artist,
                row.compilation,
                mtime,
                size,
                tagged.then_some(-775_i64),
                tagged.then_some(988_525_i64),
                tagged.then_some(-920_i64),
                tagged.then_some(1_001_221_i64),
                tagged.then_some(412_i64),
                tagged.then_some(750_000_i64),
                tagged.then_some(318_i64),
                tagged.then_some(910_000_i64),
                tagged.then_some(mtime),
                tagged.then_some(size),
                if tagged { "Folk" } else { "Game Soundtrack" },
                V7_FIRST_SEEN_NS + i64::try_from(n).expect("five rows"),
            ],
        )
        .expect("insert v7 row");
    }
}

#[test]
fn a_v7_database_migrates_in_place_without_losing_anything() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("library.db");
    write_v7_database(&db);

    let library = Library::open(&db).expect("a v7 database must open");
    assert_eq!(library.len(), 5, "every v7 row survives the upgrade");

    let conn = rusqlite::Connection::open(&db).expect("raw open");
    let version: i64 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("user_version");
    assert_eq!(version, 9);

    let by_path = |needle: &str| {
        library
            .tracks()
            .find(|t| t.path.to_string_lossy().contains(needle))
            .cloned()
            .unwrap_or_else(|| panic!("{needle} must survive"))
    };

    // Every v7 column is intact — text, numbers, Unicode, the ADR-0008
    // columns, the ADR-0010 stamp, the ADR-0013 tags, the ADR-0015 measurement
    // and the ADR-0019 genre.
    let unicode = by_path("Größenwahn");
    assert_eq!(unicode.artist.as_deref(), Some("Größenwahn"));
    assert_eq!(unicode.album.as_deref(), Some("Debüt"));
    assert_eq!(unicode.title.as_deref(), Some("Ærø — 序曲"));
    assert_eq!(unicode.track, Some(3));
    assert_eq!(unicode.disc, Some(1));
    assert_eq!(unicode.year, Some(1999));
    assert_eq!(unicode.duration, Some(Duration::new(215, 123_456_789)));
    assert_eq!(unicode.format, Some(AudioFormat::Wav));
    assert_eq!(unicode.bit_depth, Some(24));
    assert_eq!(unicode.sample_rate, Some(96_000));
    assert_eq!(unicode.bitrate, Some(4_608));
    assert_eq!(unicode.album_artist.as_deref(), Some("Various Artists"));
    assert_eq!(unicode.compilation, Some(true));
    assert_eq!(unicode.genre.as_deref(), Some("Game Soundtrack"));

    let soundtrack = by_path("Main Menu.flac");
    assert_eq!(soundtrack.album_artist.as_deref(), Some("RODIK"));
    assert_eq!(soundtrack.compilation, None, "NULL is not Some(false)");
    assert_eq!(soundtrack.replay_gain.track_gain_centidb, Some(-775));
    assert_eq!(soundtrack.genre.as_deref(), Some("Folk"));
    assert_eq!(
        library
            .computed_replay_gain(&soundtrack.path)
            .track_gain_centidb,
        Some(412),
        "the measurement v6 stored is still fresh for the same file"
    );
    assert_eq!(
        library
            .known_files()
            .values()
            .filter(|known| known.stamp.is_some())
            .count(),
        5,
        "every v7 stamp survives, so the next scan is still incremental"
    );

    // The first-seen timestamps v7 wrote are untouched — the one column a
    // migration must never move (ADR-0019).
    for album in library.albums() {
        assert!(
            album
                .first_seen_ns
                .is_some_and(|seen| (V7_FIRST_SEEN_NS..V7_FIRST_SEEN_NS + 5).contains(&seen)),
            "the upgrade must not restamp when an album arrived"
        );
    }

    // The new column is NULL for every row, and the `roots` table starts
    // empty: no scan has finished under this schema, and `baz-core` cannot
    // know which folder a v7 row came from — the front end holds that fact and
    // states it with `adopt_root`.
    assert_eq!(library.unrooted_tracks(), 5);
    assert!(
        library
            .known_files()
            .values()
            .all(|known| known.root.is_none()),
        "an upgraded row belongs to no root until something adopts it"
    );
    assert_eq!(
        library.root_stats(Path::new("/m")),
        baz_core::index::RootStats::default(),
        "and the roots table has nothing to say yet"
    );
    let roots: i64 = conn
        .query_row("SELECT count(*) FROM roots", [], |row| row.get(0))
        .expect("the roots table exists");
    assert_eq!(roots, 0);

    // Grouping is *exactly* the pre-v8 behaviour — the upgrade adds a column
    // and a table, never changes what the shelf shows.
    let albums = library.albums();
    let passage = albums
        .iter()
        .find(|a| a.title == Some("Northwest Passage"))
        .expect("the double rip");
    assert_eq!(passage.artist, AlbumArtist::Named("Stan Rogers"));
    assert_eq!(passage.editions.len(), 2);
    let gamerip: Vec<_> = albums
        .iter()
        .filter(|a| a.title == Some("Cookie's Bustle OST (gamerip)"))
        .collect();
    assert_eq!(gamerip.len(), 1, "still one entry, two editions");
    assert_eq!(gamerip[0].editions.len(), 2);
    assert_eq!(
        library.shelves(GroupKey::Genre).len(),
        2,
        "Folk and the OST"
    );
}

/// **The v8 backfill.** A pre-v8 baz held exactly one folder, so every row it
/// wrote came from that folder — a fact the config file still holds and the
/// row's own path still confirms. `adopt_root` states it, once, and both halves
/// are checked: a row is claimed only if it names no root *and* lies under this
/// one.
#[test]
fn adopting_a_root_claims_the_rows_under_it_and_only_those() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("library.db");
    write_v7_database(&db);
    let mut library = Library::open(&db).expect("open migrates to v8");

    // The fixture's five rows all live under `/m`. A sixth, from the folder an
    // agent's fixtures once landed in, does not — and must not be claimed.
    library
        .add_tracks(vec![bare("/elsewhere/stray.flac")])
        .expect("a row outside the root");

    assert_eq!(library.adopt_root(Path::new("/m")).expect("adopt"), 5);
    assert_eq!(
        library.root_stats(Path::new("/m")).tracks,
        5,
        "every row under the folder now names it"
    );
    assert_eq!(
        library.unrooted_tracks(),
        1,
        "and the row from somewhere else still belongs to nobody"
    );
    assert_eq!(
        library.root_stats(Path::new("/m")).last_scan_ns,
        None,
        "adopting is not scanning"
    );

    // Idempotent: a second launch adopts nothing, because there is nothing left
    // to adopt.
    assert_eq!(library.adopt_root(Path::new("/m")).expect("adopt"), 0);

    // Adoption never overrules a root a scan recorded. A nested folder listed
    // second gets the rows the first one did not claim, which here is none.
    assert_eq!(library.adopt_root(Path::new("/m/FLAC")).expect("adopt"), 0);
    assert_eq!(library.root_stats(Path::new("/m/FLAC")).tracks, 0);
    assert_eq!(library.root_stats(Path::new("/m")).tracks, 5);

    // Durable, which is the whole point: the next launch reads the roots back
    // rather than adopting again.
    drop(library);
    let reopened = Library::open(&db).expect("reopen");
    assert_eq!(reopened.root_stats(Path::new("/m")).tracks, 5);
    assert_eq!(reopened.unrooted_tracks(), 1);
}

/// A scan records the root it walked, and a rescan can **re-home** a row — the
/// one way `root` differs from `first_seen_ns`, which no rescan may move.
#[test]
fn a_scan_records_its_root_and_a_later_scan_can_rehome_a_row() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("library.db");
    let mut library = Library::open(&db).expect("open");

    library
        .add_tracks_under(Some(Path::new("/first")), vec![bare("/first/a.flac")])
        .expect("add");
    let first_seen = library.albums()[0].first_seen_ns.expect("a first seen");
    assert_eq!(library.root_stats(Path::new("/first")).tracks, 1);

    // The listener removed `/first` and added `/second`, which is a symlink
    // onto the same tree: the next walk reads the same path under a new name.
    library
        .add_tracks_under(Some(Path::new("/second")), vec![bare("/first/a.flac")])
        .expect("rescan");
    assert_eq!(library.len(), 1, "an upsert, not a duplicate");
    assert_eq!(library.root_stats(Path::new("/first")).tracks, 0);
    assert_eq!(library.root_stats(Path::new("/second")).tracks, 1);
    assert_eq!(
        library.albums()[0].first_seen_ns,
        Some(first_seen),
        "re-homing a row is not re-adding it"
    );

    // A caller that names **no** root says nothing about the root, rather than
    // clearing it — `add_tracks` is not a way to orphan a row.
    library
        .add_tracks(vec![bare("/first/a.flac")])
        .expect("add");
    assert_eq!(library.root_stats(Path::new("/second")).tracks, 1);
    assert_eq!(library.unrooted_tracks(), 0);

    drop(library);
    let reopened = Library::open(&db).expect("reopen");
    assert_eq!(reopened.root_stats(Path::new("/second")).tracks, 1);
}

/// Removing a folder **forgets its tracks** (ADR-0022 §4) — keyed on the
/// recorded root, so a nested folder the listener kept does not lose the rows
/// it holds. And a scan time is recorded and read back.
#[test]
fn forgetting_a_root_takes_its_rows_and_leaves_a_nested_roots_alone() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("library.db");
    let mut library = Library::open(&db).expect("open");

    // Two folders, one nested inside the other, each holding rows that a path
    // prefix could not tell apart: every one of these paths is under `/m`.
    library
        .add_tracks_under(
            Some(Path::new("/m")),
            vec![bare("/m/Live/a.flac"), bare("/m/Studio/b.flac")],
        )
        .expect("add");
    library
        .add_tracks_under(Some(Path::new("/m/Live")), vec![bare("/m/Live/c.flac")])
        .expect("add");
    library
        .record_scan(Path::new("/m/Live"), 1_800_000_000_000_000_000)
        .expect("record");
    assert_eq!(
        library.root_stats(Path::new("/m/Live")).last_scan_ns,
        Some(1_800_000_000_000_000_000)
    );

    assert_eq!(library.forget_root(Path::new("/m")).expect("forget"), 2);
    assert_eq!(library.len(), 1, "the nested folder's row is untouched");
    assert_eq!(
        library.tracks().next().expect("the survivor").path,
        PathBuf::from("/m/Live/c.flac"),
        "a path prefix would have taken this one too"
    );
    assert_eq!(
        library.root_stats(Path::new("/m")),
        baz_core::index::RootStats::default()
    );
    assert_eq!(
        library.root_stats(Path::new("/m/Live")).last_scan_ns,
        Some(1_800_000_000_000_000_000),
        "and the folder that was kept keeps its record"
    );

    // Durable, and the search index went with it.
    assert!(library.known_files().len() == 1);
    drop(library);
    let reopened = Library::open(&db).expect("reopen");
    assert_eq!(reopened.len(), 1);
    assert_eq!(reopened.root_stats(Path::new("/m/Live")).tracks, 1);
}

/// A real, tiny, valid WAV at `path` (parents created) — for the one test in
/// this file whose subject is the filesystem coming and going, which synthetic
/// [`TrackMeta`] cannot rehearse.
fn real_wav(path: &Path) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("mkdir");
    }
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 8_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec).expect("wav");
    writer.write_sample(0i16).expect("sample");
    writer.finalize().expect("finalize");
}

/// **ADR-0022's NAS guarantee, walked through at the library level** (pinned
/// while ADR-0025 made network folders a first-class ask): an unavailable
/// folder is a *refusal to walk*, never an empty walk — so a correct caller
/// holds no removal list and the rows stand — and the remount restores the
/// same rows as the same rows: stamps identical, first-seen identical, count
/// identical, no duplicates.
#[test]
fn an_unmounted_folder_keeps_its_rows_and_the_remount_restores_them_unchanged() {
    use baz_core::library::{ScanEntry, scan, scan_incremental};

    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("library.db");
    let root = dir.path().join("NAS");
    real_wav(&root.join("Artist/Album/01.wav"));
    real_wav(&root.join("Artist/Album/02.wav"));

    // The first scan, as a launch runs it: walk the folder, record what it
    // held under it, note when the pass finished.
    let mut library = Library::open(&db).expect("open");
    let read: Vec<TrackMeta> = scan(&root)
        .expect("walk")
        .filter_map(|entry| match entry {
            ScanEntry::Track(meta) => Some(meta),
            _ => None,
        })
        .collect();
    assert_eq!(read.len(), 2, "the fixture holds two files");
    library.add_tracks_under(Some(&root), read).expect("add");
    library.record_scan(&root, 1_000).expect("record");

    let held = library.known_files();
    let first_seen: Vec<Option<i64>> = library
        .albums()
        .iter()
        .map(|album| album.first_seen_ns)
        .collect();
    assert_eq!(library.root_stats(&root).tracks, 2);

    // The unmount: the whole tree stops resolving, exactly as a share's path
    // does when the mount goes.
    let parked = dir.path().join("parked");
    std::fs::rename(&root, &parked).expect("unmount");
    assert!(
        scan(&root).is_err(),
        "an absent folder is a refusal to walk, not an empty walk"
    );
    // The refusal *is* the guarantee: no walk means no entries, and no
    // entries means nothing a caller could hand `remove_tracks`. The rows,
    // their root, and the folder's scan record all stand.
    assert_eq!(library.len(), 2);
    assert_eq!(library.root_stats(&root).tracks, 2);
    assert_eq!(library.root_stats(&root).last_scan_ns, Some(1_000));

    // The remount: the same tree returns under the same name, and the stamps
    // survived it — every file is reported unchanged, not rediscovered.
    std::fs::rename(&parked, &root).expect("remount");
    let entries: Vec<ScanEntry> = scan_incremental(&root, &held).expect("walk").collect();
    assert_eq!(entries.len(), 2);
    assert!(
        entries
            .iter()
            .all(|entry| matches!(entry, ScanEntry::Unchanged { .. })),
        "unchanged stamps: {entries:?}"
    );

    // And even the pass that re-reads everything (a force sync) is an upsert,
    // not an arrival: same count, same stamps, same first-seen.
    let reread: Vec<TrackMeta> = scan(&root)
        .expect("walk")
        .filter_map(|entry| match entry {
            ScanEntry::Track(meta) => Some(meta),
            _ => None,
        })
        .collect();
    library
        .add_tracks_under(Some(&root), reread)
        .expect("re-add");
    assert_eq!(library.len(), 2, "no duplicates");
    assert_eq!(
        library.known_files(),
        held,
        "same paths, same stamps, same recorded root"
    );
    assert_eq!(
        library
            .albums()
            .iter()
            .map(|album| album.first_seen_ns)
            .collect::<Vec<_>>(),
        first_seen,
        "returning is not arriving: first-seen stands"
    );
}

// ---------------------------------------------------------------------------
// What baz remembers about music it no longer holds (schema v9, ADR-0042)
// ---------------------------------------------------------------------------

/// **The whole point, at root scale, through real files and real scans.**
///
/// A folder is scanned, its records date from the moment they arrived, the
/// listener removes the folder and adds it back — and the wall files them where
/// it always did. The assertion that matters is not that a first-seen *exists*
/// after the round trip but that it is **the value from before**, so the test
/// plants a first-seen far in the past and checks the restored rows carry it
/// rather than today.
#[test]
fn removing_a_folder_and_adding_it_back_restores_the_first_seen_from_before() {
    use baz_core::library::{ScanEntry, scan};

    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("library.db");
    let root = dir.path().join("Music");
    real_wav(&root.join("Artist/Album/01.wav"));
    real_wav(&root.join("Artist/Album/02.wav"));

    let walk = |root: &Path| -> Vec<TrackMeta> {
        scan(root)
            .expect("walk")
            .filter_map(|entry| match entry {
                ScanEntry::Track(meta) => Some(meta),
                _ => None,
            })
            .collect()
    };

    let mut library = Library::open(&db).expect("open");
    library
        .add_tracks_under(Some(&root), walk(&root))
        .expect("first scan");
    library.record_scan(&root, 1_000).expect("record");

    // Age the library: these records were added years ago, which is the only
    // state in which "ADDED = today" is visibly a loss. Written straight into
    // the database, because baz itself structurally cannot move a first-seen.
    let long_ago = now_ns() - 4 * 365 * 24 * 60 * 60 * 1_000_000_000;
    {
        let conn = rusqlite::Connection::open(&db).expect("sqlite");
        conn.execute("UPDATE tracks SET first_seen_ns = ?1", [long_ago])
            .expect("age the rows");
    }
    let mut library = Library::open(&db).expect("reopen");
    assert_eq!(library.albums()[0].first_seen_ns, Some(long_ago));
    assert_eq!(library.forgotten_paths(), 0, "nothing forgotten yet");

    // The listener removes the folder in the Settings place.
    assert_eq!(library.forget_root(&root).expect("forget"), 2);
    assert_eq!(library.len(), 0, "the wall is empty");
    assert_eq!(library.root_stats(&root).tracks, 0);
    assert_eq!(
        library.forgotten_paths(),
        2,
        "and baz kept one fact about each"
    );
    assert_eq!(
        library.forgotten_first_seen(&root.join("Artist/Album/01.wav")),
        Some(long_ago),
    );

    // Across a restart, because a memory only in RAM is not a memory.
    drop(library);
    let mut library = Library::open(&db).expect("reopen");
    assert_eq!(library.len(), 0);
    assert_eq!(library.forgotten_paths(), 2);

    // The listener adds it back. Nothing on disk ever moved, so this is the
    // ordinary launch scan and nothing else.
    library
        .add_tracks_under(Some(&root), walk(&root))
        .expect("rescan");
    assert_eq!(library.len(), 2);
    assert_eq!(
        library
            .albums()
            .iter()
            .map(|album| album.first_seen_ns)
            .collect::<Vec<_>>(),
        vec![Some(long_ago)],
        "the record files under the year it really arrived, not today",
    );
    assert_eq!(
        library.forgotten_paths(),
        0,
        "and the memory is spent, not accumulated"
    );

    drop(library);
    let reopened = Library::open(&db).expect("reopen");
    assert_eq!(reopened.albums()[0].first_seen_ns, Some(long_ago));
    assert_eq!(reopened.forgotten_paths(), 0);
}

/// The same act at record scale: `forget_paths` takes the rows a listener
/// named and keeps the same one fact, so a record forgotten and restored from a
/// backup keeps its place on the ADDED wall.
#[test]
fn forgetting_a_record_keeps_when_it_arrived_and_the_files_coming_back_restores_it() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("library.db");
    let mut library = Library::open(&db).expect("open");
    library
        .add_tracks_under(
            Some(Path::new("/m")),
            vec![
                track(
                    "/m/Gone/01.flac",
                    "Talk Talk",
                    "Laughing Stock",
                    "Myrrhman",
                    1,
                ),
                track(
                    "/m/Gone/02.flac",
                    "Talk Talk",
                    "Laughing Stock",
                    "Ascension Day",
                    2,
                ),
                track("/m/Kept/01.flac", "Bark Psychosis", "Hex", "The Loom", 1),
            ],
        )
        .expect("add");
    let arrived = library.albums()[1].first_seen_ns.expect("stamped");

    let gone = ["/m/Gone/01.flac", "/m/Gone/02.flac"];
    assert_eq!(library.forget_paths(gone).expect("forget"), 2);
    assert_eq!(library.len(), 1, "only the record named went");
    assert_eq!(album_titles(&library.albums()), vec!["Hex"]);
    assert_eq!(library.forgotten_paths(), 2);
    assert_eq!(
        library.root_stats(Path::new("/m")).tracks,
        1,
        "the folder itself is still held — a record is not its root",
    );

    // The album comes back off a backup; the ordinary scan finds it again.
    drop(library);
    let mut library = Library::open(&db).expect("reopen");
    library
        .add_tracks_under(
            Some(Path::new("/m")),
            vec![
                track(
                    "/m/Gone/01.flac",
                    "Talk Talk",
                    "Laughing Stock",
                    "Myrrhman",
                    1,
                ),
                track(
                    "/m/Gone/02.flac",
                    "Talk Talk",
                    "Laughing Stock",
                    "Ascension Day",
                    2,
                ),
            ],
        )
        .expect("rescan");
    assert_eq!(
        library
            .albums()
            .iter()
            .find(|album| album.title == Some("Laughing Stock"))
            .expect("back on the wall")
            .first_seen_ns,
        Some(arrived),
        "restored with the date it really arrived",
    );
    assert_eq!(library.forgotten_paths(), 0);
}

/// **The two scales are one act.** Forgetting a root and forgetting every path
/// recorded under it leave the *same* memory — which is the guard against the
/// two halves of this design drifting into two mechanisms that disagree.
#[test]
fn forgetting_a_root_and_forgetting_its_paths_leave_the_same_memory() {
    let rows = || vec![bare("/m/a.flac"), bare("/m/b.flac"), bare("/m/sub/c.flac")];
    let paths = ["/m/a.flac", "/m/b.flac", "/m/sub/c.flac"];

    let mut by_root = Library::open_in_memory().expect("open");
    by_root
        .add_tracks_under(Some(Path::new("/m")), rows())
        .expect("add");
    let stamps: Vec<Option<i64>> = paths
        .iter()
        .map(|path| {
            by_root
                .tracks()
                .find(|meta| meta.path == PathBuf::from(path))
                .and_then(|_| by_root.albums().first().and_then(|a| a.first_seen_ns))
        })
        .collect();
    by_root.forget_root(Path::new("/m")).expect("forget root");

    let mut by_path = Library::open_in_memory().expect("open");
    by_path
        .add_tracks_under(Some(Path::new("/m")), rows())
        .expect("add");
    by_path.forget_paths(paths).expect("forget paths");

    assert_eq!(by_root.forgotten_paths(), by_path.forgotten_paths());
    assert_eq!(by_root.forgotten_paths(), 3);
    for path in paths {
        let path = Path::new(path);
        assert!(
            by_root.forgotten_first_seen(path).is_some(),
            "{} is remembered by the root's forget",
            path.display(),
        );
        assert!(
            by_path.forgotten_first_seen(path).is_some(),
            "{} is remembered by the record's forget",
            path.display(),
        );
    }
    assert!(stamps.iter().all(Option::is_some));
}

/// A **scan-confirmed** removal is evidence and leaves nothing behind, where a
/// listener's assertion leaves a memory. The two doors out of the library are
/// different doors on purpose.
#[test]
fn a_scan_confirmed_removal_remembers_nothing_and_a_listeners_does() {
    let mut library = Library::open_in_memory().expect("open");
    library
        .add_tracks_under(
            Some(Path::new("/m")),
            vec![bare("/m/deleted.flac"), bare("/m/asserted.flac")],
        )
        .expect("add");

    // What a scan does after `is_confirmed_gone` said yes.
    assert_eq!(
        library.remove_tracks(["/m/deleted.flac"]).expect("remove"),
        1
    );
    assert_eq!(library.forgotten_paths(), 0, "evidence needs no reversal");

    // What a listener does.
    assert_eq!(
        library.forget_paths(["/m/asserted.flac"]).expect("forget"),
        1
    );
    assert_eq!(library.forgotten_paths(), 1);

    // So a file the scan removed and the listener later restores is a genuine
    // arrival, and dates from today.
    let before = now_ns();
    library
        .add_tracks_under(Some(Path::new("/m")), vec![bare("/m/deleted.flac")])
        .expect("re-add");
    let restored = library
        .albums()
        .iter()
        .flat_map(|album| album.editions.iter())
        .flat_map(|edition| edition.tracks.iter())
        .find(|meta| meta.path == Path::new("/m/deleted.flac"))
        .map(|_| library.albums()[0].first_seen_ns.expect("stamped"))
        .expect("back");
    assert!(restored >= before, "a rediscovered deletion is an arrival");
}

/// **The tombstone table's bound, asserted rather than asserted about.** One
/// row per path however many times it is forgotten, and never a row for a path
/// the library holds — the invariant that stops it being a leak.
#[test]
fn a_path_forgotten_many_times_is_remembered_once_and_never_while_held() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("library.db");
    let mut library = Library::open(&db).expect("open");

    library
        .add_tracks_under(Some(Path::new("/m")), vec![bare("/m/a.flac")])
        .expect("add");
    let arrived = library.albums()[0].first_seen_ns.expect("stamped");

    for _ in 0..5 {
        library.forget_paths(["/m/a.flac"]).expect("forget");
        assert_eq!(
            library.forgotten_paths(),
            1,
            "one row per path, not one per act"
        );
        assert_eq!(
            library.forgotten_first_seen(Path::new("/m/a.flac")),
            Some(arrived)
        );
        library
            .add_tracks_under(Some(Path::new("/m")), vec![bare("/m/a.flac")])
            .expect("re-add");
        assert_eq!(library.forgotten_paths(), 0, "consumed by the return");
        assert_eq!(library.albums()[0].first_seen_ns, Some(arrived));
    }

    // The sweep at open makes the invariant true even after a crash between
    // the row landing and its tombstone being spent — simulated by writing the
    // contradiction straight into the file.
    {
        let conn = rusqlite::Connection::open(&db).expect("sqlite");
        conn.execute(
            "INSERT INTO forgotten (path, first_seen_ns) SELECT path, 1 FROM tracks",
            [],
        )
        .expect("plant a stale tombstone");
    }
    let library = Library::open(&db).expect("reopen");
    assert_eq!(
        library.forgotten_paths(),
        0,
        "no path is ever both held and tombstoned",
    );
    assert_eq!(library.albums()[0].first_seen_ns, Some(arrived));
}

/// A row whose own first-seen was never recorded (a pre-v7 row nothing has
/// re-read) leaves **no** tombstone: there is nothing about it to remember, and
/// a memory holding no fact would be a leak that buys nothing. It reads
/// `Not recorded` before and after.
#[test]
fn a_row_with_no_recorded_first_seen_is_not_tombstoned() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("library.db");
    let mut library = Library::open(&db).expect("open");
    library
        .add_tracks_under(Some(Path::new("/m")), vec![bare("/m/a.flac")])
        .expect("add");
    drop(library);
    {
        let conn = rusqlite::Connection::open(&db).expect("sqlite");
        conn.execute("UPDATE tracks SET first_seen_ns = NULL", [])
            .expect("un-stamp it, as a pre-v7 row is");
    }

    let mut library = Library::open(&db).expect("reopen");
    assert_eq!(library.albums()[0].first_seen_ns, None);
    assert_eq!(library.forget_root(Path::new("/m")).expect("forget"), 1);
    assert_eq!(library.forgotten_paths(), 0);

    let before = now_ns();
    library
        .add_tracks_under(Some(Path::new("/m")), vec![bare("/m/a.flac")])
        .expect("re-add");
    assert!(
        library.albums()[0].first_seen_ns.expect("stamped now") >= before,
        "nothing was remembered, so this is an arrival and says so",
    );
}

// ---------------------------------------------------------------------------
// Schema v9: the `forgotten` table (ADR-0042)
// ---------------------------------------------------------------------------

/// The moment the v8 fixture's rows were first seen — years before the
/// migration runs, so a backfill that stamped "now" would be visible at a
/// glance rather than by a millisecond.
const V8_FIRST_SEEN_NS: i64 = 1_600_000_000_000_000_000;

/// When the v8 fixture's one root last finished a scan.
const V8_LAST_SCAN_NS: i64 = 1_755_000_000_000_000_000;

/// Build a genuine v8 database with the v8 schema and v8 `INSERT`s only — no
/// baz code involved — so the v9 upgrade is proved against a database this
/// build did not create.
///
/// It carries everything v1 – v8 ever added: the double rip, the soundtrack, a
/// real `Various Artists` tag, a real compilation flag, non-ASCII paths and
/// titles, stamps, tagged and measured ReplayGain, genres, first-seen
/// timestamps, the recorded root on every row, and a populated `roots` table.
fn write_v8_database(db: &std::path::Path) {
    let conn = rusqlite::Connection::open(db).expect("create v8 db");
    write_v8_schema(&conn);
    write_v8_rows(&conn);
}

/// The v8 schema, exactly as v8 wrote it.
fn write_v8_schema(conn: &rusqlite::Connection) {
    conn.execute_batch(
        "
        BEGIN;
        CREATE TABLE tracks (
            id                             INTEGER PRIMARY KEY,
            path                           BLOB NOT NULL UNIQUE,
            artist                         TEXT,
            album                          TEXT,
            title                          TEXT,
            track                          INTEGER,
            disc                           INTEGER,
            year                           INTEGER,
            duration_ns                    INTEGER,
            format                         TEXT,
            bit_depth                      INTEGER,
            sample_rate                    INTEGER,
            bitrate                        INTEGER,
            album_artist                   TEXT,
            compilation                    INTEGER,
            mtime_ns                       INTEGER,
            file_size                      INTEGER,
            rg_track_gain_centidb          INTEGER,
            rg_track_peak_micro            INTEGER,
            rg_album_gain_centidb          INTEGER,
            rg_album_peak_micro            INTEGER,
            rg_computed_track_gain_centidb INTEGER,
            rg_computed_track_peak_micro   INTEGER,
            rg_computed_album_gain_centidb INTEGER,
            rg_computed_album_peak_micro   INTEGER,
            rg_computed_mtime_ns           INTEGER,
            rg_computed_file_size          INTEGER,
            genre                          TEXT,
            first_seen_ns                  INTEGER,
            root                           BLOB
        ) STRICT;
        CREATE TABLE roots (
            path         BLOB PRIMARY KEY,
            last_scan_ns INTEGER
        ) STRICT;
        PRAGMA user_version = 8;
        COMMIT;
        ",
    )
    .expect("v8 schema");
}

/// The v8 fixture's rows, and its one root.
fn write_v8_rows(conn: &rusqlite::Connection) {
    for (n, row) in v3_rows().into_iter().enumerate() {
        let tagged = row.format == "flac";
        let mtime = 1_700_000_000_000_000_000_i64 + i64::try_from(n).expect("five rows");
        let size = 40_000_000_i64 + i64::try_from(n).expect("five rows");
        conn.execute(
            "INSERT INTO tracks
                 (path, artist, album, title, track, disc, year, duration_ns,
                  format, bit_depth, sample_rate, bitrate, album_artist,
                  compilation, mtime_ns, file_size,
                  rg_track_gain_centidb, rg_track_peak_micro,
                  rg_album_gain_centidb, rg_album_peak_micro,
                  rg_computed_track_gain_centidb, rg_computed_track_peak_micro,
                  rg_computed_album_gain_centidb, rg_computed_album_peak_micro,
                  rg_computed_mtime_ns, rg_computed_file_size,
                  genre, first_seen_ns, root)
             VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                     ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25,
                     ?26, ?27, ?28)",
            rusqlite::params![
                // The platform's own path encoding, not UTF-8 — see
                // `write_v7_database`.
                stored_path_bytes(row.path),
                row.artist,
                row.album,
                row.title,
                row.track,
                row.year,
                row.duration_ns,
                row.format,
                row.bit_depth,
                row.sample_rate,
                row.bitrate,
                row.album_artist,
                row.compilation,
                mtime,
                size,
                tagged.then_some(-775_i64),
                tagged.then_some(988_525_i64),
                tagged.then_some(-920_i64),
                tagged.then_some(1_001_221_i64),
                tagged.then_some(412_i64),
                tagged.then_some(750_000_i64),
                tagged.then_some(318_i64),
                tagged.then_some(910_000_i64),
                tagged.then_some(mtime),
                tagged.then_some(size),
                if tagged { "Folk" } else { "Game Soundtrack" },
                V8_FIRST_SEEN_NS + i64::try_from(n).expect("five rows"),
                stored_path_bytes("/m"),
            ],
        )
        .expect("insert v8 row");
    }
    conn.execute(
        "INSERT INTO roots (path, last_scan_ns) VALUES (?1, ?2)",
        rusqlite::params![stored_path_bytes("/m"), V8_LAST_SCAN_NS],
    )
    .expect("insert v8 root");
}

#[test]
fn a_v8_database_migrates_in_place_without_losing_anything() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("library.db");
    write_v8_database(&db);

    let library = Library::open(&db).expect("a v8 database must open");
    assert_eq!(library.len(), 5, "every v8 row survives the upgrade");

    let conn = rusqlite::Connection::open(&db).expect("raw open");
    let version: i64 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("user_version");
    assert_eq!(version, 9);

    let by_path = |needle: &str| {
        library
            .tracks()
            .find(|t| t.path.to_string_lossy().contains(needle))
            .cloned()
            .unwrap_or_else(|| panic!("{needle} must survive"))
    };

    // Every v8 column is intact — text, numbers, Unicode, the ADR-0008
    // columns, the ADR-0010 stamp, the ADR-0013 tags, the ADR-0015 measurement,
    // the ADR-0019 genre and first-seen, and the ADR-0022 root.
    let unicode = by_path("Größenwahn");
    assert_eq!(unicode.artist.as_deref(), Some("Größenwahn"));
    assert_eq!(unicode.album.as_deref(), Some("Debüt"));
    assert_eq!(unicode.title.as_deref(), Some("Ærø — 序曲"));
    assert_eq!(unicode.track, Some(3));
    assert_eq!(unicode.disc, Some(1));
    assert_eq!(unicode.year, Some(1999));
    assert_eq!(unicode.duration, Some(Duration::new(215, 123_456_789)));
    assert_eq!(unicode.format, Some(AudioFormat::Wav));
    assert_eq!(unicode.bit_depth, Some(24));
    assert_eq!(unicode.sample_rate, Some(96_000));
    assert_eq!(unicode.bitrate, Some(4_608));
    assert_eq!(unicode.album_artist.as_deref(), Some("Various Artists"));
    assert_eq!(unicode.compilation, Some(true));
    assert_eq!(unicode.genre.as_deref(), Some("Game Soundtrack"));

    let soundtrack = by_path("Main Menu.flac");
    assert_eq!(soundtrack.replay_gain.track_gain_centidb, Some(-775));
    assert_eq!(
        library
            .computed_replay_gain(&soundtrack.path)
            .track_gain_centidb,
        Some(412),
        "the measurement v6 stored is still fresh for the same file"
    );
    assert_eq!(
        library
            .known_files()
            .values()
            .filter(|known| known.stamp.is_some())
            .count(),
        5,
        "every v8 stamp survives, so the next scan is still incremental"
    );

    // The v8 root record survives whole: every row still names its folder, and
    // the folder still knows when it was last scanned.
    assert_eq!(library.unrooted_tracks(), 0);
    assert_eq!(library.root_stats(Path::new("/m")).tracks, 5);
    assert_eq!(
        library.root_stats(Path::new("/m")).last_scan_ns,
        Some(V8_LAST_SCAN_NS),
    );

    // The first-seen timestamps v8 wrote are untouched — the one column a
    // migration must never move (ADR-0019).
    for album in library.albums() {
        assert!(
            album
                .first_seen_ns
                .is_some_and(|seen| (V8_FIRST_SEEN_NS..V8_FIRST_SEEN_NS + 5).contains(&seen)),
            "the upgrade must not restamp when an album arrived"
        );
    }

    // The new table exists and is **empty**, which is the whole truth about it:
    // it records acts a listener has not performed, and there has been none.
    // This is the only migration in the ladder with no backfill to argue about.
    assert_eq!(library.forgotten_paths(), 0);
    let forgotten: i64 = conn
        .query_row("SELECT count(*) FROM forgotten", [], |row| row.get(0))
        .expect("the forgotten table exists");
    assert_eq!(forgotten, 0);

    // Grouping is *exactly* the pre-v9 behaviour — the upgrade adds a table and
    // never changes what the shelf shows.
    let albums = library.albums();
    let passage = albums
        .iter()
        .find(|a| a.title == Some("Northwest Passage"))
        .expect("the double rip");
    assert_eq!(passage.artist, AlbumArtist::Named("Stan Rogers"));
    assert_eq!(passage.editions.len(), 2);
    assert_eq!(
        library.shelves(GroupKey::Genre).len(),
        2,
        "Folk and the OST"
    );
}

/// The upgrade is **prospective**, and the test says so rather than the ADR
/// alone: a v8 library that had already lost a first-seen cannot have it back
/// (nothing recorded it), but the very first forget after the upgrade is
/// remembered and restored.
#[test]
fn a_v9_upgrade_cannot_undo_an_old_loss_and_prevents_the_next_one() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("library.db");
    write_v8_database(&db);

    let mut library = Library::open(&db).expect("open migrates to v9");
    assert_eq!(
        library.forgotten_paths(),
        0,
        "the upgrade invents no memory of a folder somebody removed last year",
    );

    let rows: Vec<PathBuf> = library.tracks().map(|meta| meta.path.clone()).collect();
    assert_eq!(library.forget_root(Path::new("/m")).expect("forget"), 5);
    assert_eq!(library.forgotten_paths(), 5);
    for path in &rows {
        assert!(
            library
                .forgotten_first_seen(path)
                .is_some_and(|seen| (V8_FIRST_SEEN_NS..V8_FIRST_SEEN_NS + 5).contains(&seen)),
            "{} kept the first-seen v8 recorded for it",
            path.display(),
        );
    }
}

/// **The case the whole design is insurance against**, walked through with real
/// files: a listener looks at a record whose share is merely unmounted, decides
/// it is gone, and says so. baz does exactly what it was told — and the
/// remount costs them nothing, because the one fact a rescan could not
/// rediscover was kept.
///
/// This is why a listener-initiated forget is offerable at all. ADR-0010
/// refused to *guess* that an unreachable folder is a deleted one, and it was
/// right; what it could not offer was a way for a person to say it, because
/// saying it wrongly was unrecoverable. It no longer is.
#[test]
fn forgetting_a_record_that_was_only_unmounted_costs_nothing_when_it_returns() {
    use baz_core::library::{ScanEntry, scan};

    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("library.db");
    let root = dir.path().join("NAS");
    real_wav(&root.join("Artist/Album/01.wav"));
    real_wav(&root.join("Artist/Album/02.wav"));

    let walk = |root: &Path| -> Vec<TrackMeta> {
        scan(root)
            .expect("walk")
            .filter_map(|entry| match entry {
                ScanEntry::Track(meta) => Some(meta),
                _ => None,
            })
            .collect()
    };

    let mut library = Library::open(&db).expect("open");
    library
        .add_tracks_under(Some(&root), walk(&root))
        .expect("first scan");
    library.record_scan(&root, 1_000).expect("record");
    let arrived = library.albums()[0].first_seen_ns.expect("stamped");
    let held = library.known_files();

    // The share goes away. Every path beneath it now answers `NotFound` —
    // which is indistinguishable from the album having been deleted, and is
    // exactly why baz does not decide this for itself.
    let parked = dir.path().join("parked");
    std::fs::rename(&root, &parked).expect("unmount");
    assert!(
        scan(&root).is_err(),
        "an absent folder refuses to be walked"
    );

    // The listener asserts it anyway, at record scale. baz obeys: the rows go
    // and the wall is empty.
    let gone: Vec<PathBuf> = library.tracks().map(|meta| meta.path.clone()).collect();
    assert_eq!(library.forget_paths(gone).expect("forget"), 2);
    assert_eq!(library.len(), 0);
    assert_eq!(library.forgotten_paths(), 2);
    assert_eq!(
        library.root_stats(&root).last_scan_ns,
        Some(1_000),
        "forgetting records is not forgetting the folder",
    );

    // The share comes back. The next ordinary pass finds the files, and the
    // library is *the same library* — not a fresh import wearing its name.
    std::fs::rename(&parked, &root).expect("remount");
    library
        .add_tracks_under(Some(&root), walk(&root))
        .expect("rescan");
    assert_eq!(library.len(), 2, "no duplicates");
    assert_eq!(
        library.known_files(),
        held,
        "same paths, same stamps, same recorded root",
    );
    assert_eq!(
        library.albums()[0].first_seen_ns,
        Some(arrived),
        "and the record files under the date it really arrived",
    );
    assert_eq!(library.forgotten_paths(), 0);
}

/// **What else survives a forget, checked rather than assumed.** The design
/// keeps exactly one fact in the index — but the index is not the only store,
/// and the scope of this ADR was decided by looking at the others.
///
/// The play ledger is a separate append-only file keyed by path
/// (ADR-0018), and nothing in baz deletes from it, so PLAYED already survives a
/// folder being removed and added back — with no tombstone, no widening, and
/// nothing to build. That is worth pinning: a later change that made forgetting
/// prune the ledger would silently take a fact this design decided it did not
/// need to keep.
#[test]
fn forgetting_and_restoring_a_folder_leaves_the_play_ledger_alone() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("library.db");
    let mut library = Library::open(&db).expect("open");
    library
        .add_tracks_under(
            Some(Path::new("/m")),
            vec![
                track("/m/1.flac", "Talk Talk", "Laughing Stock", "Myrrhman", 1),
                track("/m/2.flac", "Bark Psychosis", "Hex", "The Loom", 1),
            ],
        )
        .expect("add");

    let now = now_unix_s();
    let history = history_of(dir.path(), &[("/m/1.flac", now - 3_600)]);
    let played = |library: &Library| -> Vec<String> {
        library
            .shelves_with_history(GroupKey::Played, Some(&history))
            .iter()
            .map(|shelf| shelf.header.label())
            .collect()
    };
    assert_eq!(played(&library), ["This evening", "Never played"]);

    library.forget_root(Path::new("/m")).expect("forget");
    library
        .add_tracks_under(
            Some(Path::new("/m")),
            vec![
                track("/m/1.flac", "Talk Talk", "Laughing Stock", "Myrrhman", 1),
                track("/m/2.flac", "Bark Psychosis", "Hex", "The Loom", 1),
            ],
        )
        .expect("re-add");

    assert_eq!(
        played(&library),
        ["This evening", "Never played"],
        "the ledger is another store and the round trip never touched it",
    );
}
