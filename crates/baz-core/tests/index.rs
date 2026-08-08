//! Integration tests for `baz_core::index`: persistence roundtrip through a
//! real database file, incremental adds mid-scan, search semantics, and
//! album grouping — all through the public `Library` API.

use std::path::PathBuf;
use std::time::Duration;

use baz_core::index::{AlbumArtist, IndexError, Library};
use baz_core::library::{AudioFormat, FileStamp, TrackMeta};

/// A fully-`None` track except for its path — the shape a tagless file with
/// an uninformative folder layout produces.
fn bare(path: &str) -> TrackMeta {
    TrackMeta {
        path: PathBuf::from(path),
        artist: None,
        album_artist: None,
        compilation: None,
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

    // The schema really did move — and all the way, v1 → v2 → v3 → v4,
    // because migrations chain rather than jumping.
    let conn = rusqlite::Connection::open(&db).expect("raw open");
    let version: i64 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("user_version");
    assert_eq!(version, 4);

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
    assert_eq!(version, 4);

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
    assert_eq!(version, 4);

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
        library.known_files().values().all(Option::is_none),
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
    assert!(known.values().all(|s| *s == Some(stamp)));
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
    assert_eq!(known[&PathBuf::from("/m/stamped.flac")], Some(stamp));
    assert_eq!(
        known[&PathBuf::from("/m/unstamped.flac")],
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
