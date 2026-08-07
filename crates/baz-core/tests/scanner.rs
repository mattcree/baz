//! Integration tests for `baz_core::library`: real files in a tempdir, read
//! back through the public `scan` API.
//!
//! Fixture provenance: WAV audio is generated with `hound`; FLAC, MP3 and
//! MP4 containers are assembled here from their published byte layouts (see
//! [`write_flac`], [`write_mp3`], [`write_m4a`]) so that the album-artist
//! tests exercise the *real* per-container tag mappings — Vorbis
//! `ALBUMARTIST`, `ID3v2` `TPE2`, MP4 `aART` — and not one mapping three
//! times. Tags are written with `lofty`, the same crate the scanner reads
//! with; that makes these round-trip tests, not external-reference tests.
//! Externally produced fixture files (per `docs/ENGINEERING.md`, "tests to
//! specification") come in a later PR alongside the golden-file audio tests.

use std::fs;
use std::path::{Path, PathBuf};

use baz_core::library::{AudioFormat, ScanEntry, ScanError, TrackMeta, scan};
use lofty::config::WriteOptions;
use lofty::prelude::*;
use lofty::tag::{ItemKey, ItemValue, Tag, TagItem, TagType};

/// One second of 8 kHz mono ramp — a small but genuine WAV file.
fn write_wav(root: &Path, relative: &str) -> PathBuf {
    let path = root.join(relative);
    make_dirs(&path);
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

/// Create the parent directories of `path`.
fn make_dirs(path: &Path) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create fixture dirs");
    }
}

/// A minimal but structurally valid FLAC file: the `fLaC` marker, a
/// STREAMINFO block describing one second of 16-bit 44.1 kHz mono, and a
/// PADDING block. No audio frames — the scanner reads headers and tags, and
/// never decodes — so this stays under a kilobyte.
///
/// It exists so the album-artist test for **Vorbis comments** runs against a
/// real Vorbis-comment container rather than being a second `ID3v2` test in
/// disguise.
fn write_flac(root: &Path, relative: &str) -> PathBuf {
    const SAMPLE_RATE: u64 = 44_100;
    let path = root.join(relative);
    make_dirs(&path);

    let mut stream_info = Vec::with_capacity(34);
    stream_info.extend_from_slice(&4096_u16.to_be_bytes()); // min block size
    stream_info.extend_from_slice(&4096_u16.to_be_bytes()); // max block size
    stream_info.extend_from_slice(&[0; 3]); // min frame size: unknown
    stream_info.extend_from_slice(&[0; 3]); // max frame size: unknown
    // 20 bits sample rate, 3 bits (channels - 1), 5 bits (depth - 1),
    // 36 bits total samples.
    // Channels - 1 is zero (mono), so its three bits contribute nothing.
    let packed: u64 = (SAMPLE_RATE << 44) | (15 << 36) | SAMPLE_RATE;
    stream_info.extend_from_slice(&packed.to_be_bytes());
    stream_info.extend_from_slice(&[0; 16]); // MD5 of the (absent) audio

    // Metadata block header: last-block flag | 7-bit type, then a 24-bit
    // content length. STREAMINFO (type 0) is followed by a PADDING block
    // (type 1) flagged last, exactly as `flac(1)` lays a real file out — a
    // reference encoder always leaves room for tags to be written in place.
    let block = |last: bool, ty: u8, content: &[u8]| {
        let mut bytes = vec![if last { 0x80 | ty } else { ty }];
        let len = u32::try_from(content.len()).expect("fixture blocks are small");
        bytes.extend_from_slice(&len.to_be_bytes()[1..]);
        bytes.extend_from_slice(content);
        bytes
    };

    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"fLaC");
    bytes.extend_from_slice(&block(false, 0, &stream_info));
    bytes.extend_from_slice(&block(true, 1, &[0; 512]));

    fs::write(&path, bytes).expect("write flac fixture");
    path
}

/// A minimal but structurally valid MP3: ten MPEG-1 Layer III frames,
/// 128 kbit/s mono at 44.1 kHz, with zeroed payloads. Only the frame headers
/// are ever parsed, and lofty prepends its own `ID3v2` tag on write — which is
/// the point: this is what makes the `TPE2` test a genuine `ID3v2` test.
fn write_mp3(root: &Path, relative: &str) -> PathBuf {
    // 0xFF 0xFB: sync, MPEG-1, Layer III, no CRC.
    // 0x90:      bitrate index 9 (128 kbit/s), sample rate index 0 (44.1 kHz).
    // 0xC4:      mono, not copyrighted, original.
    const FRAME_HEADER: [u8; 4] = [0xFF, 0xFB, 0x90, 0xC4];
    // 144 * 128000 / 44100, truncated — the MPEG-1 Layer III frame length.
    const FRAME_LEN: usize = 417;

    let path = root.join(relative);
    make_dirs(&path);
    let mut bytes = Vec::with_capacity(FRAME_LEN * 10);
    for _ in 0..10 {
        bytes.extend_from_slice(&FRAME_HEADER);
        bytes.resize(bytes.len() + FRAME_LEN - FRAME_HEADER.len(), 0);
    }
    fs::write(&path, bytes).expect("write mp3 fixture");
    path
}

/// One MP4 atom: a big-endian length covering the header, the four-character
/// identifier, and the payload.
fn atom(fourcc: [u8; 4], payload: &[u8]) -> Vec<u8> {
    let len = u32::try_from(payload.len() + 8).expect("fixture atoms are small");
    let mut bytes = Vec::with_capacity(payload.len() + 8);
    bytes.extend_from_slice(&len.to_be_bytes());
    bytes.extend_from_slice(&fourcc);
    bytes.extend_from_slice(payload);
    bytes
}

/// An iTunes metadata item: a named atom wrapping one `data` atom carrying a
/// well-known type indicator (1 = UTF-8), a locale, and the value.
fn ilst_text(fourcc: [u8; 4], text: &str) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(&1_u32.to_be_bytes()); // type set 0, type 1: UTF-8
    payload.extend_from_slice(&0_u32.to_be_bytes()); // locale
    payload.extend_from_slice(text.as_bytes());
    atom(fourcc, &atom(*b"data", &payload))
}

/// A minimal but structurally valid `.m4a`: `ftyp`, a sound `trak` with the
/// `mdhd` timing lofty needs, and a `udta/meta/ilst` tag block. Written so
/// the MP4 side of album-artist reading — the `aART` atom — is tested
/// against a real MP4 atom tree rather than a stand-in.
///
/// No `stsd`, so no codec-specific sample description exists and lofty
/// reports no bit depth — which is exactly how the scanner classifies an
/// `.m4a` as AAC rather than ALAC (`docs/adr/0007-album-editions.md`).
fn write_m4a(root: &Path, relative: &str, tags: &[([u8; 4], &str)]) -> PathBuf {
    const TIMESCALE: u32 = 44_100;
    let path = root.join(relative);
    make_dirs(&path);

    let mut ftyp = Vec::new();
    ftyp.extend_from_slice(b"M4A "); // major brand
    ftyp.extend_from_slice(&512_u32.to_be_bytes()); // minor version
    ftyp.extend_from_slice(b"M4A "); // compatible brands

    let mut mdhd = Vec::new();
    mdhd.extend_from_slice(&0_u32.to_be_bytes()); // version 0, no flags
    mdhd.extend_from_slice(&0_u32.to_be_bytes()); // creation time
    mdhd.extend_from_slice(&0_u32.to_be_bytes()); // modification time
    mdhd.extend_from_slice(&TIMESCALE.to_be_bytes());
    mdhd.extend_from_slice(&TIMESCALE.to_be_bytes()); // duration: one second
    mdhd.extend_from_slice(&0x55C4_u16.to_be_bytes()); // language: "und"
    mdhd.extend_from_slice(&0_u16.to_be_bytes()); // pre-defined

    let mut hdlr = Vec::new();
    hdlr.extend_from_slice(&[0; 8]); // version/flags, pre-defined
    hdlr.extend_from_slice(b"soun"); // this is the audio track

    let mut mdia = atom(*b"mdhd", &mdhd);
    mdia.extend_from_slice(&atom(*b"hdlr", &hdlr));
    let trak = atom(*b"trak", &atom(*b"mdia", &mdia));

    let mut ilst = Vec::new();
    for (fourcc, value) in tags {
        ilst.extend_from_slice(&ilst_text(*fourcc, value));
    }
    let mut meta = Vec::new();
    meta.extend_from_slice(&0_u32.to_be_bytes()); // full-atom version/flags
    meta.extend_from_slice(&atom(*b"ilst", &ilst));
    let udta = atom(*b"udta", &atom(*b"meta", &meta));

    let mut moov_payload = trak;
    moov_payload.extend_from_slice(&udta);

    let mut bytes = atom(*b"ftyp", &ftyp);
    bytes.extend_from_slice(&atom(*b"moov", &moov_payload));
    fs::write(&path, bytes).expect("write m4a fixture");
    path
}

/// Tag fields to apply to a fixture; `None` leaves the field unset.
#[derive(Default)]
struct Tags<'a> {
    artist: Option<&'a str>,
    album_artist: Option<&'a str>,
    compilation: Option<bool>,
    album: Option<&'a str>,
    title: Option<&'a str>,
    track: Option<u32>,
    disc: Option<u32>,
    year: Option<u32>,
}

fn write_tags(path: &Path, tags: &Tags<'_>) {
    write_tags_as(path, TagType::Id3v2, tags);
}

fn write_tags_as(path: &Path, tag_type: TagType, tags: &Tags<'_>) {
    let mut tag = Tag::new(tag_type);
    if let Some(artist) = tags.artist {
        tag.set_artist(artist.to_owned());
    }
    if let Some(album_artist) = tags.album_artist {
        // lofty maps this to the container's own key: Vorbis `ALBUMARTIST`,
        // `ID3v2` `TPE2`, MP4 `aART`.
        tag.insert_text(ItemKey::AlbumArtist, album_artist.to_owned());
    }
    if let Some(compilation) = tags.compilation {
        tag.insert_text(
            ItemKey::FlagCompilation,
            if compilation { "1" } else { "0" }.to_owned(),
        );
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
            ..Tags::default()
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

// ---------------------------------------------------------------------------
// Album artist (docs/adr/0008-album-artist-grouping.md)
// ---------------------------------------------------------------------------

/// The one track a fixture directory holds.
fn only_track(root: &Path) -> TrackMeta {
    let mut tracks = scan_tracks(root);
    assert_eq!(tracks.len(), 1, "fixture holds exactly one track");
    tracks.remove(0)
}

#[test]
fn album_artist_is_read_from_vorbis_comments() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_flac(dir.path(), "[GST] Cookie's Bustle/1. Main Menu.flac");
    write_tags_as(
        &path,
        TagType::VorbisComments,
        &Tags {
            artist: Some("Kouhei Okamura, Masashi Matsumoto"),
            album_artist: Some("RODIK"),
            album: Some("Cookie's Bustle OST (gamerip)"),
            ..Tags::default()
        },
    );

    let t = only_track(dir.path());
    assert_eq!(t.format, Some(AudioFormat::Flac), "a real FLAC container");
    assert_eq!(t.album_artist.as_deref(), Some("RODIK"));
    assert_eq!(
        t.artist.as_deref(),
        Some("Kouhei Okamura, Masashi Matsumoto"),
        "the track artist is kept: it is the per-track credit, not a duplicate"
    );
    assert_eq!(t.compilation, None, "the file set no flag");
}

#[test]
fn album_artist_is_read_from_the_non_standard_vorbis_spelling() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_flac(dir.path(), "Compilation/01 - Track.flac");
    // Older taggers write `ALBUM ARTIST`, with a space. No standard blesses
    // it and lofty leaves it unmapped, so the scanner has to recognize it.
    let mut tag = Tag::new(TagType::VorbisComments);
    tag.set_artist("Someone".to_owned());
    // `insert` would refuse an unmapped key; `insert_unchecked` is lofty's
    // documented door for exactly this case, and writes the comment
    // verbatim.
    tag.insert_unchecked(TagItem::new(
        ItemKey::Unknown("ALBUM ARTIST".to_owned()),
        ItemValue::Text("Hipgnosis".to_owned()),
    ));
    tag.save_to_path(&path, WriteOptions::default())
        .expect("write vorbis comments");

    assert_eq!(
        only_track(dir.path()).album_artist.as_deref(),
        Some("Hipgnosis")
    );
}

#[test]
fn album_artist_is_read_from_id3v2_tpe2() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_mp3(dir.path(), "[GST] Cookie's Bustle/# Good Ending.mp3");
    write_tags(
        &path,
        &Tags {
            artist: Some("Kouhei Okamura, Masashi Matsumoto"),
            album_artist: Some("Various Artists"),
            compilation: Some(true),
            album: Some("Cookie's Bustle"),
            ..Tags::default()
        },
    );

    let t = only_track(dir.path());
    assert_eq!(t.format, Some(AudioFormat::Mp3), "a real MPEG container");
    assert_eq!(t.album_artist.as_deref(), Some("Various Artists"));
    assert_eq!(t.compilation, Some(true));
}

#[test]
fn album_artist_is_read_from_mp4_aart() {
    let dir = tempfile::tempdir().expect("tempdir");
    // The atoms are written into the fixture directly, so this asserts the
    // `aART`/`cpil` mapping and not lofty's own writer round-tripping.
    let dir_path = dir.path();
    write_m4a(
        dir_path,
        "[GST] Cookie's Bustle/Skydiving Minigame.m4a",
        &[
            (*b"\xa9ART", "Kouhei Okamura, Masashi Matsumoto"),
            (*b"aART", "RODIK"),
            (*b"\xa9alb", "Cookie's Bustle OST (gamerip)"),
            (*b"\xa9nam", "Skydiving Minigame"),
        ],
    );

    let t = only_track(dir_path);
    assert_eq!(t.album_artist.as_deref(), Some("RODIK"));
    assert_eq!(
        t.artist.as_deref(),
        Some("Kouhei Okamura, Masashi Matsumoto")
    );
    assert_eq!(t.album.as_deref(), Some("Cookie's Bustle OST (gamerip)"));
    assert_eq!(t.title.as_deref(), Some("Skydiving Minigame"));
    // No sample description in the fixture, so no declared depth: AAC.
    assert_eq!(t.format, Some(AudioFormat::Aac));
}

#[test]
fn a_blank_album_artist_tag_counts_as_absent() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_flac(dir.path(), "Big Star/Radio City/01 - O My Soul.flac");
    write_tags_as(
        &path,
        TagType::VorbisComments,
        &Tags {
            artist: Some("Big Star"),
            album_artist: Some("   "),
            album: Some("Radio City"),
            ..Tags::default()
        },
    );

    let t = only_track(dir.path());
    assert_eq!(
        t.album_artist, None,
        "whitespace is not an album artist; the grouping fallback decides"
    );
    assert_eq!(t.artist.as_deref(), Some("Big Star"));
}

#[test]
fn folder_inference_fills_the_album_artist_when_no_tags_do() {
    let dir = tempfile::tempdir().expect("tempdir");
    // Wholly untagged, filed the way almost every collection is filed.
    write_wav(dir.path(), "Big Star/Radio City/03 - Back of a Car.wav");

    let t = only_track(dir.path());
    assert_eq!(t.artist.as_deref(), Some("Big Star"));
    assert_eq!(
        t.album_artist.as_deref(),
        Some("Big Star"),
        "the same directory that gives the artist gives the album artist"
    );
    assert_eq!(t.album.as_deref(), Some("Radio City"));
}

#[test]
fn a_tagged_artist_is_never_overruled_by_its_folder() {
    let dir = tempfile::tempdir().expect("tempdir");
    // The folder disagrees with the tag. Tags win, and inference does *not*
    // sneak the folder name in through the album-artist field — it would
    // become the shelf caption and the grouping key.
    let path = write_flac(dir.path(), "Beatles/Abbey Road/01 - Come Together.flac");
    write_tags_as(
        &path,
        TagType::VorbisComments,
        &Tags {
            artist: Some("The Beatles"),
            album: Some("Abbey Road"),
            ..Tags::default()
        },
    );

    let t = only_track(dir.path());
    assert_eq!(t.artist.as_deref(), Some("The Beatles"));
    assert_eq!(
        t.album_artist, None,
        "no album-artist tag and an artist tag present: the chain falls \
         through to the artist tag, not to the directory name"
    );
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
