//! Integration tests for `baz_core::index`: persistence roundtrip through a
//! real database file, incremental adds mid-scan, search semantics, and
//! album grouping — all through the public `Library` API.

use std::path::PathBuf;
use std::time::Duration;

use baz_core::index::{IndexError, Library};
use baz_core::library::TrackMeta;

/// A fully-`None` track except for its path — the shape a tagless file with
/// an uninformative folder layout produces.
fn bare(path: &str) -> TrackMeta {
    TrackMeta {
        path: PathBuf::from(path),
        artist: None,
        album: None,
        title: None,
        track: None,
        disc: None,
        year: None,
        duration: None,
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
    assert_eq!(album.artist, Some("The Band"));
    assert_eq!(album.title, Some("Live Set"));
    assert_eq!(album.tracks.len(), 3);
    let order: Vec<_> = album
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
    assert_eq!(albums[0].artist, Some("Alpha"));
    assert_eq!(albums[1].artist, Some("Beta"));
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
    assert_eq!(albums[0].artist, None);
    assert_eq!(albums[0].title, None);
    assert_eq!(albums[0].tracks.len(), 2);
    assert_eq!(albums[1].artist, Some("Zeta"));
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
