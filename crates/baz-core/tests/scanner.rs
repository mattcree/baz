//! Integration tests for `baz_core::library`: real files in a tempdir, read
//! back through the public `scan` API.
//!
//! Fixture provenance: WAV audio is generated with `hound`; tags are written
//! with `lofty` — the same crate the scanner reads with. That makes these
//! round-trip tests, not external-reference tests; externally produced
//! fixture files (per `docs/ENGINEERING.md`, "tests to specification") come
//! in a later PR alongside the golden-file audio tests.

use std::fs;
use std::path::{Path, PathBuf};

use baz_core::library::{AudioFormat, ScanEntry, ScanError, TrackMeta, scan};
use lofty::config::WriteOptions;
use lofty::prelude::*;
use lofty::tag::{Tag, TagType};

/// One second of 8 kHz mono ramp — a small but genuine WAV file.
fn write_wav(root: &Path, relative: &str) -> PathBuf {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create fixture dirs");
    }
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 8000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(&path, spec).expect("create wav");
    for i in 0..8000_i16 {
        writer.write_sample(i).expect("write sample");
    }
    writer.finalize().expect("finalize wav");
    path
}

/// Tag fields to apply to a fixture; `None` leaves the field unset.
#[derive(Default)]
struct Tags<'a> {
    artist: Option<&'a str>,
    album: Option<&'a str>,
    title: Option<&'a str>,
    track: Option<u32>,
    disc: Option<u32>,
    year: Option<u32>,
}

fn write_tags(path: &Path, tags: &Tags<'_>) {
    let mut tag = Tag::new(TagType::Id3v2);
    if let Some(artist) = tags.artist {
        tag.set_artist(artist.to_owned());
    }
    if let Some(album) = tags.album {
        tag.set_album(album.to_owned());
    }
    if let Some(title) = tags.title {
        tag.set_title(title.to_owned());
    }
    if let Some(track) = tags.track {
        tag.set_track(track);
    }
    if let Some(disc) = tags.disc {
        tag.set_disk(disc);
    }
    if let Some(year) = tags.year {
        tag.set_year(year);
    }
    tag.save_to_path(path, WriteOptions::default())
        .expect("write fixture tags");
}

/// Run a scan to completion, panicking on any `Failed` entry.
fn scan_tracks(root: &Path) -> Vec<TrackMeta> {
    scan(root)
        .expect("scan starts")
        .map(|entry| match entry {
            ScanEntry::Track(meta) => meta,
            ScanEntry::Failed { path, reason } => {
                panic!("unexpected failure for {}: {reason}", path.display())
            }
        })
        .collect()
}

#[test]
fn fully_tagged_file_tags_win_over_path() {
    let dir = tempfile::tempdir().expect("tempdir");
    // Path deliberately disagrees with the tags on every inferable field.
    let path = write_wav(dir.path(), "Wrong Artist/Wrong Album/09 - Wrong Title.wav");
    write_tags(
        &path,
        &Tags {
            artist: Some("Big Star"),
            album: Some("Radio City"),
            title: Some("September Gurls"),
            track: Some(11),
            disc: Some(1),
            year: Some(1974),
        },
    );

    let tracks = scan_tracks(dir.path());
    assert_eq!(tracks.len(), 1);
    let t = &tracks[0];
    assert_eq!(t.path, path);
    assert_eq!(t.artist.as_deref(), Some("Big Star"));
    assert_eq!(t.album.as_deref(), Some("Radio City"));
    assert_eq!(t.title.as_deref(), Some("September Gurls"));
    assert_eq!(t.track, Some(11));
    assert_eq!(t.disc, Some(1));
    assert_eq!(t.year, Some(1974));
    // 8000 samples at 8 kHz: about a second, known cheaply from the header.
    let duration = t.duration.expect("wav header exposes duration");
    assert!(
        (900..=1100).contains(&duration.as_millis()),
        "duration was {duration:?}"
    );
}

#[test]
fn encoding_properties_come_from_the_file_not_the_folder_name() {
    let dir = tempfile::tempdir().expect("tempdir");
    // A folder named for a *different* format: only the file may be believed.
    let path = write_wav(dir.path(), "MP3/Big Star/Radio City/01 - track.wav");
    write_tags(
        &path,
        &Tags {
            artist: Some("Big Star"),
            album: Some("Radio City"),
            ..Tags::default()
        },
    );

    let tracks = scan_tracks(dir.path());
    assert_eq!(tracks.len(), 1);
    let t = &tracks[0];
    assert_eq!(
        t.format,
        Some(AudioFormat::Wav),
        "the codec is read from the file, never inferred from the path"
    );
    assert!(t.format.is_some_and(AudioFormat::is_lossless));
    // The fixture is 16-bit mono at 8 kHz (see `write_wav`).
    assert_eq!(t.bit_depth, Some(16));
    assert_eq!(t.sample_rate, Some(8_000));
    assert_eq!(t.bitrate, Some(128), "8000 Hz * 16 bit = 128 kbit/s");
}

#[test]
fn untagged_file_is_inferred_from_folder_layout() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_wav(dir.path(), "Big Star/Radio City/03 - Back of a Car.wav");

    let tracks = scan_tracks(dir.path());
    assert_eq!(tracks.len(), 1);
    let t = &tracks[0];
    assert_eq!(t.artist.as_deref(), Some("Big Star"));
    assert_eq!(t.album.as_deref(), Some("Radio City"));
    assert_eq!(t.title.as_deref(), Some("Back of a Car"));
    assert_eq!(t.track, Some(3));
    assert_eq!(t.disc, None, "disc is never inferred from folders");
    assert_eq!(t.year, None);
}

#[test]
fn partial_tags_win_field_by_field() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_wav(dir.path(), "Big Star/Radio City/01 - Wrong Title.wav");
    // Only the title is tagged; everything else must come from the path.
    write_tags(
        &path,
        &Tags {
            title: Some("O My Soul"),
            ..Tags::default()
        },
    );

    let tracks = scan_tracks(dir.path());
    assert_eq!(tracks.len(), 1);
    let t = &tracks[0];
    assert_eq!(t.title.as_deref(), Some("O My Soul"), "tag wins");
    assert_eq!(t.artist.as_deref(), Some("Big Star"), "inferred");
    assert_eq!(t.album.as_deref(), Some("Radio City"), "inferred");
    assert_eq!(t.track, Some(1), "inferred");
}

#[test]
fn unicode_paths_survive_inference() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_wav(dir.path(), "Größenwahn/Über Älbum/01 - Café Tacvba.wav");
    write_wav(dir.path(), "坂本龍一/async/02 - 安らぎ.wav");

    let mut tracks = scan_tracks(dir.path());
    tracks.sort_by(|a, b| a.path.cmp(&b.path));
    assert_eq!(tracks.len(), 2);

    let cjk = tracks
        .iter()
        .find(|t| t.artist.as_deref() == Some("坂本龍一"))
        .expect("CJK track present");
    assert_eq!(cjk.album.as_deref(), Some("async"));
    assert_eq!(cjk.title.as_deref(), Some("安らぎ"));
    assert_eq!(cjk.track, Some(2));

    let diacritics = tracks
        .iter()
        .find(|t| t.artist.as_deref() == Some("Größenwahn"))
        .expect("diacritics track present");
    assert_eq!(diacritics.album.as_deref(), Some("Über Älbum"));
    assert_eq!(diacritics.title.as_deref(), Some("Café Tacvba"));
}

#[test]
fn non_audio_files_and_empty_dirs_are_ignored() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_wav(dir.path(), "Artist/Album/01 - Keeper.wav");
    fs::create_dir_all(dir.path().join("Empty/Nested/Deeper")).expect("empty dirs");
    fs::write(dir.path().join("Artist/Album/cover.jpg"), b"not audio").expect("jpg");
    fs::write(dir.path().join("Artist/Album/notes.txt"), b"liner notes").expect("txt");
    fs::write(dir.path().join("Artist/Album/rip.log"), b"EAC log").expect("log");
    fs::write(dir.path().join("noextension"), b"mystery").expect("bare file");

    let tracks = scan_tracks(dir.path());
    assert_eq!(tracks.len(), 1, "only the wav is a track");
    assert_eq!(tracks[0].title.as_deref(), Some("Keeper"));
}

#[test]
fn corrupt_file_is_reported_and_scan_continues() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_wav(dir.path(), "Artist/Album/01 - Good.wav");
    let bad = dir.path().join("Artist/Album/02 - Bad.flac");
    fs::write(&bad, b"this is not a flac stream at all").expect("garbage flac");
    write_wav(dir.path(), "Artist/Album/03 - Also Good.wav");

    let entries: Vec<ScanEntry> = scan(dir.path()).expect("scan starts").collect();

    let tracks: Vec<&TrackMeta> = entries
        .iter()
        .filter_map(|e| match e {
            ScanEntry::Track(meta) => Some(meta),
            ScanEntry::Failed { .. } => None,
        })
        .collect();
    let failures: Vec<(&PathBuf, &String)> = entries
        .iter()
        .filter_map(|e| match e {
            ScanEntry::Failed { path, reason } => Some((path, reason)),
            ScanEntry::Track(_) => None,
        })
        .collect();

    assert_eq!(tracks.len(), 2, "both valid files still scanned");
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].0, &bad);
    assert!(!failures[0].1.is_empty(), "failure carries a reason");
}

#[test]
fn missing_root_is_a_scan_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let missing = dir.path().join("does-not-exist");
    match scan(&missing) {
        Err(ScanError::RootNotFound { path }) => assert_eq!(path, missing),
        other => panic!("expected RootNotFound, got {other:?}"),
    }
}

#[test]
fn file_root_is_a_scan_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file = write_wav(dir.path(), "just-a-file.wav");
    match scan(&file) {
        Err(ScanError::RootNotDirectory { path }) => assert_eq!(path, file),
        other => panic!("expected RootNotDirectory, got {other:?}"),
    }
}
