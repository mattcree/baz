//! Integration tests for `baz_core::index`: persistence roundtrip through a
//! real database file, incremental adds mid-scan, search semantics, and
//! album grouping — all through the public `Library` API.

use std::path::PathBuf;
use std::time::Duration;

use baz_core::index::{IndexError, Library};
use baz_core::library::{AudioFormat, TrackMeta};

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
        format: None,
        bit_depth: None,
        sample_rate: None,
        bitrate: None,
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
    assert_eq!(albums[0].editions.len(), 1);
    assert_eq!(albums[0].editions[0].tracks.len(), 2);
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
                row.path.as_bytes(),
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

    // The schema really did move.
    let conn = rusqlite::Connection::open(&db).expect("raw open");
    let version: i64 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("user_version");
    assert_eq!(version, 2);

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
