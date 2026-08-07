//! Cold vs warm scan over a synthetic 10 000-file library.
//!
//! This is the `docs/ENGINEERING.md` benchmark for scan throughput, and the
//! evidence behind spending a schema version on incremental scanning. Four
//! measurements over one synthetic tree, two of the scan alone and two of
//! what a launch actually costs (scan **plus** the index writes it causes):
//!
//! - **cold**: [`scan`] opens and tag-parses every file — what every baz
//!   launch did before schema v4.
//! - **warm**: [`scan_incremental`] with the stamps the previous scan
//!   recorded; every file is `stat`ed, none is opened, and nothing is
//!   written to the database.
//!
//! The fixtures are 10 000 WAVs of ~8 KB each carrying a real tag set
//! (artist, album, title, track number, year) written by the same crate the
//! scanner reads with, laid out `Artist NNNN/Album NNNN/NN Track.wav`.
//!
//! # Measured
//!
//! Development host (Fedora 44, ext4 on a local SSD, warm page cache, release
//! build). The **ratio** is the durable part; the absolute times are this
//! machine's.
//!
//! | measurement | 10 000 files | per file |
//! |---|---|---|
//! | `scan/cold_10k` | 61.2 ms | 6.1 µs |
//! | `scan/warm_10k` | 10.3 ms | 1.0 µs |
//! | `scan/launch_cold_10k` (scan + index) | 83.4 ms | 8.3 µs |
//! | `scan/launch_warm_10k` (scan + index) | 11.6 ms | 1.2 µs |
//!
//! The scan itself is **5.9×** cheaper warm; the launch as a whole — the
//! number a listener feels — is **7.2×** cheaper, because a warm pass also
//! writes no rows where a cold one upserts every track it read.
//!
//! Both figures are **lower bounds** on a real library, and deliberately
//! reported as such. These fixtures carry no embedded cover art and every
//! byte of them is in the page cache; real FLACs and MP4s hold a JPEG inside
//! the tag block that lofty parses, and a 100k-track library fits in nobody's
//! cache. The `stat` side of the comparison grows with none of that, so the
//! gap on a real collection is wider than the table — how much wider is a
//! question only a real 100k library answers, and this bench does not
//! pretend to.

use std::hint::black_box;
use std::path::{Path, PathBuf};

use baz_core::index::Library;
use baz_core::library::{KnownFiles, ScanEntry, TrackMeta, scan, scan_incremental};
use criterion::{Criterion, criterion_group, criterion_main};
use lofty::config::WriteOptions;
use lofty::prelude::*;
use lofty::tag::{Tag, TagType};

/// Files in the synthetic library. Large enough for per-file costs to
/// dominate the walk, small enough to build in a few seconds.
const FILES: usize = 10_000;

/// Files per album directory, so the walk crosses realistic directory
/// boundaries rather than reading one enormous flat folder.
const PER_ALBUM: usize = 10;

/// Build the synthetic library under `root`: [`FILES`] tagged WAVs laid out
/// `Artist NNNN/Album NNNN/NN Track.wav`.
fn build_library(root: &Path) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 8_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    for index in 0..FILES {
        let album = index / PER_ALBUM;
        let artist = album / 10;
        let dir = root.join(format!("Artist {artist:04}/Album {album:04}"));
        std::fs::create_dir_all(&dir).expect("create fixture dirs");
        let path = dir.join(format!("{:02} Track.wav", index % PER_ALBUM));

        let mut writer = hound::WavWriter::create(&path, spec).expect("create wav");
        for sample in 0..4_000_i16 {
            writer.write_sample(sample).expect("write sample");
        }
        writer.finalize().expect("finalize wav");

        // A real file carries tags, and reading them is the cost the warm
        // pass avoids. An untagged fixture would flatter the comparison.
        let mut tag = Tag::new(TagType::RiffInfo);
        tag.set_artist(format!("Artist {artist:04}"));
        tag.set_album(format!("Album {album:04}"));
        tag.set_title(format!("Track {index:05}"));
        tag.set_track(u32::try_from(index % PER_ALBUM).unwrap_or(0) + 1);
        tag.set_year(1960 + u32::try_from(artist % 60).unwrap_or(0));
        tag.save_to_path(&path, WriteOptions::default())
            .expect("write fixture tags");
    }
}

/// The stamps a completed scan leaves in the index — the warm pass's input.
fn stamps(root: &Path) -> KnownFiles {
    scan(root)
        .expect("scan starts")
        .filter_map(|entry| match entry {
            ScanEntry::Track(meta) => Some((meta.path, meta.stamp)),
            _ => None,
        })
        .collect()
}

/// One launch's worth of work: drain the scan and apply it to a library,
/// exactly as `crates/baz/src/app.rs` does. Returns the tracks written.
fn absorb(entries: impl Iterator<Item = ScanEntry>, library: &mut Library) -> usize {
    let fresh: Vec<TrackMeta> = entries
        .filter_map(|entry| match entry {
            ScanEntry::Track(meta) => Some(meta),
            _ => None,
        })
        .collect();
    library.add_tracks(fresh).expect("index write")
}

fn bench_scan(c: &mut Criterion) {
    let dir = tempfile::tempdir().expect("tempdir");
    build_library(dir.path());
    let root: PathBuf = dir.path().to_path_buf();
    let known = stamps(&root);
    assert_eq!(known.len(), FILES, "the fixture library must be complete");

    let mut group = c.benchmark_group("scan");
    // One sample is a whole pass over 10 000 files; the default 100 would
    // spend minutes re-measuring a very stable number.
    group.sample_size(10);

    group.bench_function("cold_10k", |b| {
        b.iter(|| {
            let count = scan(black_box(&root)).expect("scan starts").count();
            assert_eq!(count, FILES);
            black_box(count)
        });
    });

    group.bench_function("warm_10k", |b| {
        b.iter(|| {
            let skipped = scan_incremental(black_box(&root), black_box(&known))
                .expect("scan starts")
                .filter(|entry| matches!(entry, ScanEntry::Unchanged { .. }))
                .count();
            assert_eq!(skipped, FILES, "a warm pass must open nothing");
            black_box(skipped)
        });
    });

    // What a launch costs end to end. The warm pass wins twice: it parses
    // no files *and* it writes no rows.
    group.bench_function("launch_cold_10k", |b| {
        b.iter(|| {
            let mut library = Library::open_in_memory().expect("open");
            let written = absorb(scan(black_box(&root)).expect("scan starts"), &mut library);
            assert_eq!(written, FILES);
            black_box(written)
        });
    });

    group.bench_function("launch_warm_10k", |b| {
        b.iter(|| {
            let mut library = Library::open_in_memory().expect("open");
            let written = absorb(
                scan_incremental(black_box(&root), black_box(&known)).expect("scan starts"),
                &mut library,
            );
            assert_eq!(written, 0, "a warm pass writes nothing");
            black_box(written)
        });
    });

    group.finish();
}

criterion_group!(benches, bench_scan);
criterion_main!(benches);
