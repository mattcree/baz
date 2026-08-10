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
use std::time::{Duration, SystemTime};

use baz_core::library::{
    AudioFormat, FileStamp, KnownFile, KnownFiles, ScanEntry, ScanError, TrackMeta,
    is_confirmed_gone, scan, scan_incremental,
};
use baz_core::replaygain::ReplayGainTags;
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

/// Ogg's page checksum: CRC-32 with polynomial `0x04c1_1db7`, no input or
/// output reflection, zero initial value and no final XOR (RFC 3533 §6).
/// Deliberately not any of the CRC crates — a five-line loop beats a
/// dependency for one test fixture.
fn ogg_crc(data: &[u8]) -> u32 {
    let mut crc: u32 = 0;
    for &byte in data {
        crc ^= u32::from(byte) << 24;
        for _ in 0..8 {
            crc = if crc & 0x8000_0000 == 0 {
                crc << 1
            } else {
                (crc << 1) ^ 0x04c1_1db7
            };
        }
    }
    crc
}

/// One Ogg page carrying whole packets (RFC 3533 §6). `header_type` is the
/// bitfield: 0x02 begins the logical stream, 0x04 ends it.
fn ogg_page(header_type: u8, granule: u64, seq: u32, packets: &[&[u8]]) -> Vec<u8> {
    // Segment table: each packet becomes ⌊len/255⌋ lacing values of 255
    // followed by len % 255, which is how a reader knows where it ended.
    let mut segments = Vec::new();
    for packet in packets {
        let mut remaining = packet.len();
        while remaining >= 255 {
            segments.push(255_u8);
            remaining -= 255;
        }
        segments.push(u8::try_from(remaining).expect("< 255 by construction"));
    }
    assert!(segments.len() <= 255, "fixture pages hold one packet each");

    let mut page = Vec::new();
    page.extend_from_slice(b"OggS");
    page.push(0); // stream structure version
    page.push(header_type);
    page.extend_from_slice(&granule.to_le_bytes());
    page.extend_from_slice(&0xBA25_F00D_u32.to_le_bytes()); // bitstream serial
    page.extend_from_slice(&seq.to_le_bytes());
    let crc_at = page.len();
    page.extend_from_slice(&0_u32.to_le_bytes()); // CRC placeholder
    page.push(u8::try_from(segments.len()).expect("checked above"));
    page.extend_from_slice(&segments);
    for packet in packets {
        page.extend_from_slice(packet);
    }
    let crc = ogg_crc(&page);
    page[crc_at..crc_at + 4].copy_from_slice(&crc.to_le_bytes());
    page
}

/// A comment header body in the Vorbis-comment format: a vendor string and an
/// empty user-comment list. Shared by Ogg Opus (`OpusTags`) and Ogg Vorbis.
fn vorbis_comment_body() -> Vec<u8> {
    const VENDOR: &[u8] = b"baz test fixture";
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&u32::try_from(VENDOR.len()).expect("short").to_le_bytes());
    bytes.extend_from_slice(VENDOR);
    bytes.extend_from_slice(&0_u32.to_le_bytes()); // no user comments
    bytes
}

/// A structurally valid **Ogg Opus** stream: an `OpusHead` identification
/// packet, an `OpusTags` comment packet, and one audio page whose granule
/// position gives the stream a duration (RFC 7845 §5).
///
/// It carries no decodable audio because nothing decodes it — that is the
/// whole point. What it has to be is *identifiable*: the scanner's job is to
/// recognise Opus and decline to list it, and a fixture that lofty could not
/// identify would prove nothing.
fn write_ogg_opus(root: &Path, relative: &str) -> PathBuf {
    const PRE_SKIP: u16 = 312;
    const RATE: u32 = 48_000;
    let path = root.join(relative);
    make_dirs(&path);

    let mut head = Vec::new();
    head.extend_from_slice(b"OpusHead");
    head.push(1); // encapsulation version
    head.push(1); // channel count
    head.extend_from_slice(&PRE_SKIP.to_le_bytes());
    head.extend_from_slice(&RATE.to_le_bytes()); // original input rate
    head.extend_from_slice(&0_i16.to_le_bytes()); // output gain
    head.push(0); // channel mapping family 0 (mono/stereo)

    let mut tags = Vec::new();
    tags.extend_from_slice(b"OpusTags");
    tags.extend_from_slice(&vorbis_comment_body());

    // One 20 ms SILK-NB frame's worth of TOC byte plus filler. The granule
    // position is in 48 kHz samples and includes the pre-skip.
    let audio = [0x08_u8, 0x00, 0x00, 0x00];
    let granule = u64::from(PRE_SKIP) + u64::from(RATE); // one second of audio

    let mut bytes = Vec::new();
    bytes.extend_from_slice(&ogg_page(0x02, 0, 0, &[&head]));
    bytes.extend_from_slice(&ogg_page(0x00, 0, 1, &[&tags]));
    bytes.extend_from_slice(&ogg_page(0x04, granule, 2, &[&audio]));
    fs::write(&path, bytes).expect("write ogg opus fixture");
    path
}

/// A structurally valid **Ogg Vorbis** stream: the three Vorbis headers
/// (identification, comment, setup) followed by an audio page whose granule
/// position gives the stream a duration.
///
/// The control for [`write_ogg_opus`]: `.ogg` is a container, not a codec, and
/// dropping Opus must not drop Vorbis with it.
fn write_ogg_vorbis(root: &Path, relative: &str) -> PathBuf {
    const RATE: u32 = 44_100;
    let path = root.join(relative);
    make_dirs(&path);

    // Identification header (Vorbis I §4.2.2).
    let mut ident = vec![1_u8];
    ident.extend_from_slice(b"vorbis");
    ident.extend_from_slice(&0_u32.to_le_bytes()); // version
    ident.push(2); // channels
    ident.extend_from_slice(&RATE.to_le_bytes());
    ident.extend_from_slice(&0_i32.to_le_bytes()); // bitrate maximum
    ident.extend_from_slice(&192_000_i32.to_le_bytes()); // bitrate nominal
    ident.extend_from_slice(&0_i32.to_le_bytes()); // bitrate minimum
    ident.push(0xB8); // blocksize_0 = 2^8, blocksize_1 = 2^11
    ident.push(0x01); // framing bit

    let mut comment = vec![3_u8];
    comment.extend_from_slice(b"vorbis");
    comment.extend_from_slice(&vorbis_comment_body());
    comment.push(0x01); // framing bit

    // Setup header: never parsed by a tag reader (only by a decoder), so its
    // body is filler. It must exist, because the header packet count is what
    // tells a reader where the audio starts.
    let mut setup = vec![5_u8];
    setup.extend_from_slice(b"vorbis");
    setup.extend_from_slice(&[0; 64]);

    let audio = [0_u8; 16];
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&ogg_page(0x02, 0, 0, &[&ident]));
    bytes.extend_from_slice(&ogg_page(0x00, 0, 1, &[&comment, &setup]));
    bytes.extend_from_slice(&ogg_page(0x04, u64::from(RATE), 2, &[&audio]));
    fs::write(&path, bytes).expect("write ogg vorbis fixture");
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

/// An iTunes **freeform** metadata item: the `----` atom, carrying a `mean`
/// (the namespace, always `com.apple.iTunes` in practice), a `name` (the key)
/// and a `data` payload.
///
/// This is where MP4 keeps ReplayGain — there is no well-known atom for it —
/// so writing the boxes out by hand is what makes the MP4 ReplayGain test a
/// real container test rather than a second Vorbis test in disguise.
fn ilst_freeform(name: &str, text: &str) -> Vec<u8> {
    let full = |fourcc: [u8; 4], body: &[u8]| {
        let mut payload = 0_u32.to_be_bytes().to_vec(); // version 0, no flags
        payload.extend_from_slice(body);
        atom(fourcc, &payload)
    };
    let mut payload = full(*b"mean", b"com.apple.iTunes");
    payload.extend_from_slice(&full(*b"name", name.as_bytes()));
    let mut data = 1_u32.to_be_bytes().to_vec(); // type set 0, type 1: UTF-8
    data.extend_from_slice(&0_u32.to_be_bytes()); // locale
    data.extend_from_slice(text.as_bytes());
    payload.extend_from_slice(&atom(*b"data", &data));
    atom(*b"----", &payload)
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
    let items: Vec<Vec<u8>> = tags
        .iter()
        .map(|(fourcc, value)| ilst_text(*fourcc, value))
        .collect();
    write_m4a_items(root, relative, &items)
}

/// [`write_m4a`] with the `ilst` items supplied whole, for the freeform atoms
/// ([`ilst_freeform`]) that [`write_m4a`]'s four-character-code shorthand
/// cannot express.
fn write_m4a_items(root: &Path, relative: &str, items: &[Vec<u8>]) -> PathBuf {
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
    for item in items {
        ilst.extend_from_slice(item);
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
            ScanEntry::Unchanged { path } => {
                panic!(
                    "a full scan never skips a file, but skipped {}",
                    path.display()
                )
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

/// **The shelf never advertises what the engine cannot play — extension half.**
///
/// `.opus` is not in `AUDIO_EXTENSIONS` (Symphonia ships no Opus decoder in
/// any released version — see `AudioFormat::is_decodable`), so an `.opus`
/// file is not audio as far as a scan is concerned: not a track, and not a
/// [`ScanEntry::Failed`] either. The bytes are deliberately garbage, because
/// the point is that nothing ever opens them.
#[test]
fn opus_files_are_not_scanned_at_all() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_wav(dir.path(), "Artist/Album/01 - Keeper.wav");
    fs::write(
        dir.path().join("Artist/Album/02 - Unplayable.opus"),
        b"never opened",
    )
    .expect("opus file");

    let entries: Vec<ScanEntry> = scan(dir.path()).expect("scan starts").collect();
    assert_eq!(
        entries.len(),
        1,
        ".opus must not produce an entry of any kind, got: {entries:?}"
    );
    match &entries[0] {
        ScanEntry::Track(meta) => assert_eq!(meta.title.as_deref(), Some("Keeper")),
        other => panic!("expected only the wav track, got {other:?}"),
    }
}

/// **The shelf never advertises what the engine cannot play — codec half.**
///
/// Extension is not enough: `.ogg` is a container and it can hold Opus. Such
/// a file is dropped on the codec lofty read out of it, not listed and then
/// skipped at playback time. The Vorbis file beside it — same extension, same
/// container, decodable codec — must still be listed, or the fix would have
/// cured the disease by killing the patient.
#[test]
fn ogg_opus_is_dropped_but_ogg_vorbis_is_kept() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_ogg_vorbis(dir.path(), "Artist/Album/01 - Vorbis.ogg");
    write_ogg_opus(dir.path(), "Artist/Album/02 - Opus.ogg");

    let entries: Vec<ScanEntry> = scan(dir.path()).expect("scan starts").collect();
    assert_eq!(
        entries.len(),
        1,
        "the Opus file must be dropped silently — not listed, and not reported \
         as a failure. Entries: {entries:?}"
    );
    match &entries[0] {
        ScanEntry::Track(meta) => {
            assert_eq!(meta.title.as_deref(), Some("Vorbis"));
            assert_eq!(
                meta.format,
                Some(AudioFormat::Vorbis),
                "the surviving .ogg must be the Vorbis one"
            );
        }
        other => panic!("expected the Vorbis track, got {other:?}"),
    }
}

/// The decoder set and the advertised extension list agree on Opus, from the
/// library side. (`every_advertised_extension_decodes` in `tests/playback.rs`
/// is the other half: every extension that *is* advertised really decodes.)
#[test]
fn opus_is_the_only_undecodable_format() {
    let undecodable: Vec<AudioFormat> = [
        AudioFormat::Flac,
        AudioFormat::Alac,
        AudioFormat::Wav,
        AudioFormat::Mp3,
        AudioFormat::Aac,
        AudioFormat::Vorbis,
        AudioFormat::Opus,
    ]
    .into_iter()
    .filter(|f| !f.is_decodable())
    .collect();
    assert_eq!(
        undecodable,
        vec![AudioFormat::Opus],
        "if this changed, AUDIO_EXTENSIONS and docs/BACKLOG.md's Opus entry \
         need to change with it"
    );
}

/// A file is identified by its **content**, not its extension.
///
/// `lofty::read_from_path` picks a parser from the extension alone, which is
/// how an Ogg Opus file named `.ogg` used to arrive as "Vorbis: File missing
/// magic signature". The scanner sniffs instead, and the side benefit is
/// tested here: a FLAC that someone named `.mp3` — real libraries have
/// these — is read as the FLAC it is, rather than failing to parse.
#[test]
fn a_mislabelled_file_is_read_by_its_content() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_flac(dir.path(), "Artist/Album/01 - Actually FLAC.mp3");

    let entries: Vec<ScanEntry> = scan(dir.path()).expect("scan starts").collect();
    assert_eq!(entries.len(), 1);
    match &entries[0] {
        ScanEntry::Track(meta) => {
            assert_eq!(meta.title.as_deref(), Some("Actually FLAC"));
            assert_eq!(
                meta.format,
                Some(AudioFormat::Flac),
                "the codec must come from the bytes, not from `.mp3`"
            );
        }
        other => panic!("a mislabelled file must still be read, got {other:?}"),
    }
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
            ScanEntry::Failed { .. } | ScanEntry::Unchanged { .. } => None,
        })
        .collect();
    let failures: Vec<(&PathBuf, &String)> = entries
        .iter()
        .filter_map(|e| match e {
            ScanEntry::Failed { path, reason } => Some((path, reason)),
            ScanEntry::Track(_) | ScanEntry::Unchanged { .. } => None,
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

/// The genre is read, and read **verbatim** (ADR-0019): a messy tag reaches
/// the shelf exactly as the file spells it — no normalisation, no mapping
/// table, no splitting on the separator a tagger happened to use. The GENRE
/// group key exists to show a listener what their files actually say.
#[test]
fn genre_is_read_verbatim_from_the_tag() {
    for messy in [
        "Post-Rock",
        "post rock",
        "Rock; Instrumental",
        "Drum & Bass / Jungle",
        "(17)",
        "Chill\u{e5}",
    ] {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = write_flac(dir.path(), "Artist/Album/01 - Track.flac");
        let mut tag = Tag::new(TagType::VorbisComments);
        tag.set_artist("Someone".to_owned());
        tag.insert_text(ItemKey::Genre, messy.to_owned());
        tag.save_to_path(&path, WriteOptions::default())
            .expect("write vorbis comments");

        assert_eq!(
            only_track(dir.path()).genre.as_deref(),
            Some(messy),
            "`{messy}` must arrive on the shelf as `{messy}`"
        );
    }
}

/// A file with no genre tag says nothing, and the folder it sits in is not
/// consulted: a directory name is evidence about artist and album, and
/// evidence about nothing else.
#[test]
fn a_missing_genre_is_never_inferred_from_the_folder() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_flac(dir.path(), "Jazz/Kind of Blue/01 - So What.flac");
    let mut tag = Tag::new(TagType::VorbisComments);
    tag.set_artist("Miles Davis".to_owned());
    // A blank genre is not a genre either — the same hygiene every other tag
    // field gets.
    tag.insert_text(ItemKey::Genre, "   ".to_owned());
    tag.save_to_path(&path, WriteOptions::default())
        .expect("write vorbis comments");

    let t = only_track(dir.path());
    assert_eq!(t.genre, None);
    assert_eq!(t.artist.as_deref(), Some("Miles Davis"));
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

// ---------------------------------------------------------------------------
// Incremental scanning: the stamp check, and what counts as proof of absence.
// ---------------------------------------------------------------------------

/// The stamps a full scan of `root` produces, keyed by path — what the index
/// would hold after that scan (`Library::known_files`).
fn stamps_from_full_scan(root: &Path) -> KnownFiles {
    scan_tracks(root)
        .into_iter()
        .map(|meta| {
            assert!(
                meta.stamp.is_some(),
                "every track a scan reads must carry the stamp the next scan compares: {}",
                meta.path.display()
            );
            (meta.path, KnownFile::stamped(meta.stamp))
        })
        .collect()
}

/// Replace a file's contents with `len` bytes of garbage and put its
/// modification time back, so its stamp is unchanged but its bytes are not.
/// Anything that actually *opens* it afterwards fails to parse it.
fn gut_but_keep_the_stamp(path: &Path, stamp: FileStamp) {
    let len = usize::try_from(stamp.size).expect("fixtures are small");
    fs::write(path, vec![0xABu8; len]).expect("overwrite");
    fs::File::options()
        .write(true)
        .open(path)
        .expect("reopen")
        .set_modified(stamp.modified())
        .expect("restore mtime");
    assert_eq!(
        FileStamp::of_path(path),
        Some(stamp),
        "the fixture must be indistinguishable by size and mtime"
    );
}

/// The core claim of incremental scanning: an unchanged file is **not read**.
///
/// Proved without trusting a timer or a counter. The file's bytes are
/// replaced with garbage no parser accepts while its size and mtime are
/// restored, so a scan that opened it could only report
/// [`ScanEntry::Failed`]. It reports `Unchanged` instead.
#[test]
fn an_unchanged_file_is_reported_without_being_read() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_wav(dir.path(), "Artist/Album/01 - Quiet.wav");
    write_tags(
        &path,
        &Tags {
            title: Some("Quiet"),
            ..Tags::default()
        },
    );
    let known = stamps_from_full_scan(dir.path());
    let stamp = known[&path].stamp.expect("a stamp");
    gut_but_keep_the_stamp(&path, stamp);

    let entries: Vec<ScanEntry> = scan_incremental(dir.path(), &known)
        .expect("scan starts")
        .collect();
    assert_eq!(
        entries,
        vec![ScanEntry::Unchanged { path }],
        "the tags must have been taken from the index, not from the file"
    );
}

#[test]
fn a_touched_file_is_read_again() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_wav(dir.path(), "Artist/Album/01 - Moved.wav");
    write_tags(
        &path,
        &Tags {
            title: Some("Before"),
            ..Tags::default()
        },
    );
    let known = stamps_from_full_scan(dir.path());

    // Same bytes, later mtime — a `touch`, or a tagger that rewrote in place.
    fs::File::options()
        .write(true)
        .open(&path)
        .expect("reopen")
        .set_modified(SystemTime::now() + Duration::from_secs(600))
        .expect("touch");

    let entries: Vec<ScanEntry> = scan_incremental(dir.path(), &known)
        .expect("scan starts")
        .collect();
    match entries.as_slice() {
        [ScanEntry::Track(meta)] => {
            assert_eq!(meta.title.as_deref(), Some("Before"));
            assert_ne!(
                meta.stamp, known[&path].stamp,
                "the row must carry the *new* stamp, or every scan re-reads it"
            );
        }
        other => panic!("a touched file must be re-read, got {other:?}"),
    }
}

/// Size alone is enough: an edit that lands in the same second on a
/// coarse-timestamp filesystem still changes the length.
#[test]
fn a_file_that_changed_size_is_read_again() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_wav(dir.path(), "Artist/Album/01 - Grew.wav");
    let known = stamps_from_full_scan(dir.path());
    let stamp = known[&path].stamp.expect("a stamp");

    // Longer file, original mtime restored: only the size gives it away.
    let mut bytes = fs::read(&path).expect("read");
    bytes.extend_from_slice(&[0u8; 64]);
    fs::write(&path, &bytes).expect("grow");
    fs::File::options()
        .write(true)
        .open(&path)
        .expect("reopen")
        .set_modified(stamp.modified())
        .expect("restore mtime");

    let entries: Vec<ScanEntry> = scan_incremental(dir.path(), &known)
        .expect("scan starts")
        .collect();
    assert!(
        matches!(entries.as_slice(), [ScanEntry::Track(_)]),
        "a size change must force a re-read, got {entries:?}"
    );
}

#[test]
fn a_new_file_is_read_even_when_its_neighbours_are_cached() {
    let dir = tempfile::tempdir().expect("tempdir");
    let old = write_wav(dir.path(), "Artist/Album/01 - Old.wav");
    let known = stamps_from_full_scan(dir.path());
    let fresh = write_wav(dir.path(), "Artist/Album/02 - New.wav");
    write_tags(
        &fresh,
        &Tags {
            title: Some("New"),
            ..Tags::default()
        },
    );

    let entries: Vec<ScanEntry> = scan_incremental(dir.path(), &known)
        .expect("scan starts")
        .collect();
    assert_eq!(entries.len(), 2);
    assert!(
        entries.contains(&ScanEntry::Unchanged { path: old }),
        "the known file is skipped: {entries:?}"
    );
    let read: Vec<&TrackMeta> = entries
        .iter()
        .filter_map(|e| match e {
            ScanEntry::Track(meta) => Some(meta),
            _ => None,
        })
        .collect();
    assert_eq!(read.len(), 1);
    assert_eq!(read[0].path, fresh);
    assert_eq!(read[0].title.as_deref(), Some("New"));
}

/// A row the index has no stamp for — every row, on the first launch after
/// the v4 upgrade — is read, not assumed current.
#[test]
fn a_known_path_without_a_stamp_is_still_read() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_wav(dir.path(), "Artist/Album/01 - Unstamped.wav");
    let known: KnownFiles = KnownFiles::from([(path.clone(), KnownFile::stamped(None))]);

    let entries: Vec<ScanEntry> = scan_incremental(dir.path(), &known)
        .expect("scan starts")
        .collect();
    match entries.as_slice() {
        [ScanEntry::Track(meta)] => assert!(meta.stamp.is_some(), "and it gains one"),
        other => panic!("an unstamped row must be re-read, got {other:?}"),
    }
}

/// The stamps in the cache are matched **per path**. A file that happens to
/// share another file's size and mtime — trivially true of two files written
/// in the same instant — must not inherit its cached row.
#[test]
fn a_stamp_belongs_to_one_path_only() {
    let dir = tempfile::tempdir().expect("tempdir");
    let one = write_wav(dir.path(), "Artist/Album/01 - One.wav");
    let two = write_wav(dir.path(), "Artist/Album/02 - Two.wav");
    // Give them genuinely identical stamps.
    let stamp = FileStamp::of_path(&one).expect("a stamp");
    fs::File::options()
        .write(true)
        .open(&two)
        .expect("reopen")
        .set_modified(stamp.modified())
        .expect("align mtime");
    assert_eq!(FileStamp::of_path(&two), Some(stamp));

    // Only `one` is in the index.
    let known: KnownFiles = KnownFiles::from([(one.clone(), KnownFile::stamped(Some(stamp)))]);
    let entries: Vec<ScanEntry> = scan_incremental(dir.path(), &known)
        .expect("scan starts")
        .collect();
    assert!(entries.contains(&ScanEntry::Unchanged { path: one }));
    assert!(
        entries
            .iter()
            .any(|e| matches!(e, ScanEntry::Track(meta) if meta.path == two)),
        "the unknown file must be read despite a matching stamp: {entries:?}"
    );
}

/// A plain [`scan`] ignores everything the index knows — it is the full pass,
/// unchanged from before v4, and still the one an empty library performs.
#[test]
fn a_full_scan_never_skips_anything() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_wav(dir.path(), "Artist/Album/01 - One.wav");
    let known = stamps_from_full_scan(dir.path());
    assert_eq!(known.len(), 1);

    let entries: Vec<ScanEntry> = scan(dir.path()).expect("scan starts").collect();
    assert!(matches!(entries.as_slice(), [ScanEntry::Track(_)]));
}

#[test]
fn a_file_that_is_really_gone_is_confirmed_gone() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_wav(dir.path(), "Artist/Album/01 - Doomed.wav");
    assert!(
        !is_confirmed_gone(&path),
        "a file that is present is never confirmed gone"
    );
    fs::remove_file(&path).expect("delete");
    assert!(is_confirmed_gone(&path));
}

/// The conservative rule: a missing *directory* proves nothing about the
/// files under it. An unplugged drive, an unmounted share and a deleted
/// folder all answer `NotFound` for every path below, so none of them may
/// authorise a delete.
#[test]
fn a_file_under_a_missing_directory_is_not_confirmed_gone() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("NAS/Artist/Album/01 - Unplugged.wav");
    assert!(!path.exists());
    assert!(
        !is_confirmed_gone(&path),
        "no parent directory means no evidence"
    );

    // Once the directory is back and the file still is not, the answer flips.
    make_dirs(&path);
    assert!(is_confirmed_gone(&path));
}

/// A broken symlink is a directory entry that exists. Removing its row would
/// be removing a row for something still on disk.
#[cfg(unix)]
#[test]
fn a_broken_symlink_is_not_confirmed_gone() {
    let dir = tempfile::tempdir().expect("tempdir");
    let link = dir.path().join("Artist/Album/01 - Dangling.wav");
    make_dirs(&link);
    std::os::unix::fs::symlink(dir.path().join("nowhere.wav"), &link).expect("symlink");
    assert!(fs::metadata(&link).is_err(), "the target really is missing");
    assert!(!is_confirmed_gone(&link), "the link itself is still there");
}

/// A directory that exists but cannot be read answers with a permission
/// error, not `NotFound` — and a permission error is not evidence of
/// absence. (Skipped when the test runs as root, for whom nothing is
/// unreadable.)
#[cfg(unix)]
#[test]
fn a_file_in_an_unreadable_directory_is_not_confirmed_gone() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().expect("tempdir");
    let locked = dir.path().join("Locked");
    fs::create_dir_all(&locked).expect("mkdir");
    let path = locked.join("01 - Hidden.wav");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).expect("lock");

    let unreadable = fs::symlink_metadata(&path)
        .err()
        .is_some_and(|err| err.kind() != std::io::ErrorKind::NotFound);
    if unreadable {
        assert!(
            !is_confirmed_gone(&path),
            "\"I was not allowed to look\" is not \"it is not there\""
        );
    }
    // Restore, or the tempdir cannot be cleaned up.
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).expect("unlock");
}

/// The inverse of `FileStamp::modified`, mirroring what `FileStamp::of`
/// computes from metadata. Written here rather than called from `baz-core`
/// so the round-trip is checked against an independent implementation
/// instead of the code under test agreeing with itself.
fn system_time_to_ns(t: std::time::SystemTime) -> Option<i64> {
    match t.duration_since(std::time::UNIX_EPOCH) {
        Ok(since) => i64::try_from(since.as_nanos()).ok(),
        Err(before) => i64::try_from(before.duration().as_nanos())
            .ok()
            .and_then(i64::checked_neg),
    }
}

#[test]
fn a_stamp_survives_a_round_trip_through_system_time() {
    // Our own arithmetic must be lossless in both directions — in particular
    // the pre-epoch branch, which negates rather than subtracting and is easy
    // to get wrong by a tick.
    //
    // The values are multiples of 100 ns deliberately. `SystemTime` is
    // `FILETIME`-backed on Windows and therefore cannot *hold* finer than a
    // 100-nanosecond tick; asserting on `…789` would test that platform's
    // clock resolution rather than baz's conversion, which is the mistake this
    // test made before. Timestamps baz actually handles come from the
    // filesystem, so they are already at whatever resolution the platform
    // offers, and `a_stamp_read_from_the_filesystem_is_stable` covers that
    // path end to end.
    for mtime_ns in [0, 100, 1_700_000_000_123_456_700, -86_400_000_000_100] {
        let stamp = FileStamp {
            mtime_ns,
            size: 123,
        };
        assert_eq!(
            system_time_to_ns(stamp.modified()),
            Some(mtime_ns),
            "conversion must round-trip a timestamp the platform can represent"
        );
    }
}

#[test]
fn a_stamp_read_from_the_filesystem_is_stable() {
    // What incremental scanning actually needs is that an untouched file
    // reports the *same* stamp every time — not that the filesystem preserves
    // whatever nanosecond count we handed it. It usually cannot: NTFS keeps
    // 100-nanosecond ticks since 1601, HFS+ whole seconds, FAT two of them.
    // Asserting exact fidelity tested the filesystem's granularity, not baz,
    // and failed on Windows for a reason that says nothing about our code.
    for mtime_ns in [0, 100, 1_700_000_000_123_456_700, -86_400_000_000_100] {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = write_wav(dir.path(), "a.wav");
        let requested = FileStamp {
            mtime_ns,
            size: 123,
        }
        .modified();
        if fs::File::options()
            .write(true)
            .open(&path)
            .expect("reopen")
            .set_modified(requested)
            .is_err()
        {
            // Some platforms refuse pre-epoch or out-of-range timestamps
            // outright. Refusing is fine; silently mangling is what would
            // matter, and the stability check below covers that.
            continue;
        }
        let first = FileStamp::of_path(&path).expect("stamp");
        let second = FileStamp::of_path(&path).expect("stamp again");
        assert_eq!(
            first, second,
            "an untouched file must report an identical stamp every read"
        );
        assert_eq!(
            Some(first),
            std::fs::metadata(&path)
                .ok()
                .and_then(|m| FileStamp::of(&m)),
            "of_path must agree with the metadata the platform reports"
        );
    }
}

// ---------------------------------------------------------------------------
// ReplayGain (docs/adr/0013-replaygain.md)
// ---------------------------------------------------------------------------

/// The figures a real ReplayGain scanner writes into a FLAC's Vorbis comments,
/// read back through the public scan API.
///
/// Written with `lofty`'s own `ItemKey` mapping — the same door a scanner uses
/// — so this asserts the Vorbis-comment path end to end rather than a string
/// the test invented.
#[test]
fn replay_gain_is_read_from_vorbis_comments() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_flac(
        dir.path(),
        "Stan Rogers/Northwest Passage/01 - Passage.flac",
    );
    let mut tag = Tag::new(TagType::VorbisComments);
    tag.set_artist("Stan Rogers".to_owned());
    tag.insert_text(ItemKey::ReplayGainTrackGain, "-7.75 dB".to_owned());
    tag.insert_text(ItemKey::ReplayGainTrackPeak, "0.988525".to_owned());
    tag.insert_text(ItemKey::ReplayGainAlbumGain, "-9.20 dB".to_owned());
    tag.insert_text(ItemKey::ReplayGainAlbumPeak, "1.001221".to_owned());
    // What every scanner also writes, and what must not be mistaken for a gain.
    tag.insert_unchecked(TagItem::new(
        ItemKey::Unknown("REPLAYGAIN_REFERENCE_LOUDNESS".to_owned()),
        ItemValue::Text("89.0 dB".to_owned()),
    ));
    tag.save_to_path(&path, WriteOptions::default())
        .expect("write vorbis comments");

    let t = only_track(dir.path());
    assert_eq!(t.format, Some(AudioFormat::Flac), "a real FLAC container");
    assert_eq!(
        t.replay_gain,
        ReplayGainTags {
            track_gain_centidb: Some(-775),
            track_peak_micro: Some(988_525),
            album_gain_centidb: Some(-920),
            album_peak_micro: Some(1_001_221),
        }
    );
}

/// The `ID3v2` form: `TXXX` frames whose description is the ReplayGain key.
/// lofty writes exactly that for these `ItemKey`s on an MP3, which is what
/// makes this a genuine `ID3v2` test rather than a third Vorbis one.
#[test]
fn replay_gain_is_read_from_id3v2_txxx_frames() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_mp3(dir.path(), "Big Star/Radio City/01 - O My Soul.mp3");
    let mut tag = Tag::new(TagType::Id3v2);
    tag.set_artist("Big Star".to_owned());
    tag.insert_text(ItemKey::ReplayGainTrackGain, "+2.34 dB".to_owned());
    tag.insert_text(ItemKey::ReplayGainTrackPeak, "0.500000".to_owned());
    tag.save_to_path(&path, WriteOptions::default())
        .expect("write id3v2");

    let t = only_track(dir.path());
    assert_eq!(t.format, Some(AudioFormat::Mp3), "a real MPEG container");
    assert_eq!(
        t.replay_gain,
        ReplayGainTags {
            track_gain_centidb: Some(234),
            track_peak_micro: Some(500_000),
            album_gain_centidb: None,
            album_peak_micro: None,
        }
    );
}

/// The MP4 form: `----` freeform atoms under `com.apple.iTunes`, written into
/// the fixture as raw boxes so the atom layout itself is under test.
#[test]
fn replay_gain_is_read_from_mp4_freeform_atoms() {
    let dir = tempfile::tempdir().expect("tempdir");
    let dir_path = dir.path();
    write_m4a_items(
        dir_path,
        "[GST] Cookie\'s Bustle/As For Dreams.m4a",
        &[
            ilst_text(*b"\xa9ART", "Kouhei Okamura"),
            ilst_freeform("replaygain_track_gain", "-4.07 dB"),
            ilst_freeform("replaygain_track_peak", "0.977000"),
            ilst_freeform("replaygain_album_gain", "-5.10 dB"),
            ilst_freeform("replaygain_album_peak", "1.010000"),
        ],
    );

    let t = only_track(dir_path);
    assert_eq!(t.format, Some(AudioFormat::Aac));
    assert_eq!(
        t.replay_gain,
        ReplayGainTags {
            track_gain_centidb: Some(-407),
            track_peak_micro: Some(977_000),
            album_gain_centidb: Some(-510),
            album_peak_micro: Some(1_010_000),
        }
    );
}

/// The Opus-style integer form in Vorbis comments on a file that is not Opus —
/// which is where it actually turns up, `.opus` not being scanned at all.
/// It carries no peak, and it is shifted onto ReplayGain's own reference.
#[test]
fn the_r128_integer_form_is_read_from_vorbis_comments() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_flac(dir.path(), "R128/Album/01 - Track.flac");
    let mut tag = Tag::new(TagType::VorbisComments);
    tag.set_artist("Someone".to_owned());
    // No standard blesses these keys, so lofty leaves them unmapped and
    // `insert_unchecked` writes them verbatim — exactly how a real R128-era
    // tagger's output reaches the scanner.
    tag.insert_unchecked(TagItem::new(
        ItemKey::Unknown("R128_TRACK_GAIN".to_owned()),
        ItemValue::Text("-2321".to_owned()),
    ));
    tag.insert_unchecked(TagItem::new(
        ItemKey::Unknown("R128_ALBUM_GAIN".to_owned()),
        ItemValue::Text("-1792".to_owned()),
    ));
    tag.save_to_path(&path, WriteOptions::default())
        .expect("write vorbis comments");

    let t = only_track(dir.path());
    assert_eq!(
        t.replay_gain,
        ReplayGainTags {
            track_gain_centidb: Some(-407),
            track_peak_micro: None,
            album_gain_centidb: Some(-200),
            album_peak_micro: None,
        }
    );
}

/// A file with no ReplayGain reads back empty — the ordinary state of a
/// library nothing has ever scanned, and never a claim that the track needs no
/// gain.
#[test]
fn a_file_with_no_replay_gain_tags_reports_none() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_wav(dir.path(), "Big Star/Radio City/03 - Back of a Car.wav");
    assert!(only_track(dir.path()).replay_gain.is_empty());
}

/// Hostile values in a real file: the scan completes, the track is kept, the
/// unusable figures read as absent, and the usable one survives. A malformed
/// tag must never abort a scan or poison the row it is on.
#[test]
fn malformed_replay_gain_tags_neither_panic_nor_fail_the_scan() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_flac(dir.path(), "Broken/Album/01 - Track.flac");
    let mut tag = Tag::new(TagType::VorbisComments);
    tag.set_artist("Someone".to_owned());
    tag.insert_text(ItemKey::ReplayGainTrackGain, "very loud indeed".to_owned());
    tag.insert_text(ItemKey::ReplayGainTrackPeak, "-1".to_owned());
    tag.insert_text(ItemKey::ReplayGainAlbumGain, "1e30 dB".to_owned());
    tag.insert_text(ItemKey::ReplayGainAlbumPeak, "0.750000".to_owned());
    tag.save_to_path(&path, WriteOptions::default())
        .expect("write vorbis comments");

    let t = only_track(dir.path());
    assert_eq!(t.artist.as_deref(), Some("Someone"), "the row survives");
    assert_eq!(
        t.replay_gain,
        ReplayGainTags {
            track_gain_centidb: None,
            track_peak_micro: None,
            album_gain_centidb: None,
            album_peak_micro: Some(750_000),
        },
        "one good figure among four bad ones is still read"
    );
}

/// A multichannel file is on the shelf, and always was.
///
/// This is the library half of ADR-0039's question, and the answer decides
/// whether shipping the downmix needs a rescan: it does not. The scanner reads
/// headers with lofty and never decodes, and nothing in the scan path looks at
/// a channel count, so a 5.1 record has been listed with its duration, format
/// and rate since the day it was copied in — it simply refused to play when
/// clicked. Adding the fold changes what happens on the *play* side and nothing
/// on the shelf, so no library has to be rebuilt to gain it.
///
/// The fixture is `WAVE_FORMAT_EXTENSIBLE` with a real 5.1 channel mask,
/// because a plain multichannel WAV would prove only that lofty counts bytes.
#[test]
fn a_multichannel_file_is_listed_like_any_other() {
    const RATE: u32 = 48_000;
    const CHANNELS: u16 = 6;
    const MASK_51: u32 = 0x3F;
    const BLOCK_ALIGN: u16 = CHANNELS * 2;
    const DATA_BYTES: u32 = RATE * BLOCK_ALIGN as u32;

    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("Surround/Album/01 - In Five Point One.wav");
    make_dirs(&path);
    // 5.1: FL+FR+FC+LFE+BL+BR, one second of silence at 48 kHz, 16-bit.
    let mut w: Vec<u8> = Vec::new();
    w.extend_from_slice(b"RIFF");
    w.extend_from_slice(&(36u32 + 24 + DATA_BYTES).to_le_bytes());
    w.extend_from_slice(b"WAVEfmt ");
    w.extend_from_slice(&40u32.to_le_bytes()); // WAVEFORMATEXTENSIBLE
    w.extend_from_slice(&0xFFFEu16.to_le_bytes()); // WAVE_FORMAT_EXTENSIBLE
    w.extend_from_slice(&CHANNELS.to_le_bytes());
    w.extend_from_slice(&RATE.to_le_bytes());
    w.extend_from_slice(&(RATE * u32::from(BLOCK_ALIGN)).to_le_bytes());
    w.extend_from_slice(&BLOCK_ALIGN.to_le_bytes());
    w.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    w.extend_from_slice(&22u16.to_le_bytes()); // cbSize
    w.extend_from_slice(&16u16.to_le_bytes()); // wValidBitsPerSample
    w.extend_from_slice(&MASK_51.to_le_bytes()); // the declaration under test
    // KSDATAFORMAT_SUBTYPE_PCM
    w.extend_from_slice(&1u32.to_le_bytes());
    w.extend_from_slice(&0u16.to_le_bytes());
    w.extend_from_slice(&0x0010u16.to_le_bytes());
    w.extend_from_slice(&[0x80, 0x00, 0x00, 0xaa, 0x00, 0x38, 0x9b, 0x71]);
    w.extend_from_slice(b"data");
    w.extend_from_slice(&DATA_BYTES.to_le_bytes());
    w.extend_from_slice(&vec![0u8; DATA_BYTES as usize]);
    fs::write(&path, w).expect("write 5.1 fixture");

    let t = only_track(dir.path());
    assert_eq!(t.album.as_deref(), Some("Album"), "the row is on the shelf");
    assert_eq!(t.sample_rate, Some(48_000));
    assert_eq!(t.duration, Some(std::time::Duration::from_secs(1)));
}
