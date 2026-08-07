//! Search-latency baseline over a 100k-track synthetic library.
//!
//! This records the `docs/ENGINEERING.md` benchmark for the library-search
//! hot path: the Phase 1 spike measured p99 0.17 ms for in-RAM substring
//! search over 100k tracks, and this bench keeps the real implementation
//! honest against that bar. The query set is scripted to cover the shapes
//! that matter: a selective hit, a Unicode-folded hit, a CJK hit, a
//! high-frequency term that fills the result limit early, and a total miss
//! (the worst case — it scans every haystack).

use std::hint::black_box;
use std::path::PathBuf;
use std::time::Duration;

use baz_core::index::Library;
use baz_core::library::TrackMeta;
use criterion::{Criterion, criterion_group, criterion_main};

const ADJECTIVES: &[&str] = &[
    "Silver",
    "Broken",
    "Neon",
    "Golden",
    "Quiet",
    "Electric",
    "Crimson",
    "Hollow",
    "Wandering",
    "Midnight",
    "Paper",
    "Distant",
    "Burning",
    "Frozen",
    "Velvet",
    "Lonely",
];
const NOUNS: &[&str] = &[
    "Nightfall",
    "River",
    "Machine",
    "Garden",
    "Season",
    "Mirror",
    "Harbor",
    "Signal",
    "Empire",
    "Echo",
    "Horizon",
    "Lantern",
    "Voyage",
    "Circuit",
    "Meadow",
    "Sparrow",
];

/// Deterministic synthetic library: 2500 artists x 4 albums x 10 tracks,
/// with Unicode artists sprinkled in so folded and CJK queries have real
/// targets.
fn synthetic_tracks() -> Vec<TrackMeta> {
    let mut tracks = Vec::with_capacity(100_000);
    for artist_n in 0..2_500_u32 {
        let artist = match artist_n % 500 {
            77 => format!("Größenwahn {artist_n}"),
            250 => format!("東京事変 {artist_n}"),
            _ => format!(
                "{} {} {artist_n}",
                ADJECTIVES[(artist_n as usize) % ADJECTIVES.len()],
                NOUNS[(artist_n as usize / ADJECTIVES.len()) % NOUNS.len()],
            ),
        };
        for album_n in 0..4_u32 {
            let album = format!(
                "{} {} {album_n}",
                NOUNS[((artist_n + album_n) as usize) % NOUNS.len()],
                ADJECTIVES[(album_n as usize) % ADJECTIVES.len()],
            );
            for track_n in 1..=10_u32 {
                let title = format!(
                    "{} {} {track_n}",
                    ADJECTIVES[((artist_n + track_n) as usize) % ADJECTIVES.len()],
                    NOUNS[((album_n + track_n) as usize) % NOUNS.len()],
                );
                tracks.push(TrackMeta {
                    path: PathBuf::from(format!(
                        "/music/{artist}/{album}/{track_n:02} {title}.flac"
                    )),
                    artist: Some(artist.clone()),
                    album_artist: Some(artist.clone()),
                    compilation: None,
                    album: Some(album.clone()),
                    title: Some(title),
                    track: Some(track_n),
                    disc: Some(1),
                    year: Some(1960 + (artist_n + album_n) % 65),
                    duration: Some(Duration::from_secs(180 + u64::from(track_n))),
                    format: Some(baz_core::library::AudioFormat::Flac),
                    bit_depth: Some(16),
                    sample_rate: Some(44_100),
                    bitrate: Some(900),
                    stamp: None,
                });
            }
        }
    }
    tracks
}

fn bench_search(c: &mut Criterion) {
    let mut library = Library::open_in_memory().expect("open in-memory library");
    let tracks = synthetic_tracks();
    assert_eq!(tracks.len(), 100_000);
    library.add_tracks(tracks).expect("index synthetic tracks");

    let mut group = c.benchmark_group("search_100k");
    let queries = [
        ("selective", "velvet sparrow"),
        ("unicode_fold", "GRÖßENWAHN"),
        ("cjk", "東京"),
        ("common_fills_limit", "silver"),
        ("total_miss", "zyzzyva quartet"),
    ];
    for (name, query) in queries {
        group.bench_function(name, |b| {
            b.iter(|| black_box(library.search(black_box(query), 50)));
        });
    }
    group.finish();
}

criterion_group!(benches, bench_search);
criterion_main!(benches);
