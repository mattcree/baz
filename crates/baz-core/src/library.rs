//! Library scanning: walk a music folder, extract per-track metadata.
//!
//! This is the front half of the "point it at a directory and play within a
//! minute" pillar: [`scan`] walks a root with `walkdir`, picks out audio
//! files by extension ([`AUDIO_EXTENSIONS`]), reads their tags with `lofty`,
//! and fills any holes from the folder structure via [`inference`] (tags win
//! over inference, field by field). Persistence, watching, and search
//! indexing are deliberately *not* here — they consume this module's output
//! in later stages.
//!
//! # Why an iterator
//!
//! [`Scan`] is a pull-based [`Iterator`] rather than a callback API: the
//! future UI populates the shelf live during a scan, and an iterator lets the
//! consumer decide the pacing — drain it on a worker thread into a channel,
//! batch entries per frame, or stop early — without this module dictating a
//! threading model or inverting control. Each `next()` does the I/O for at
//! most one directory entry, so consumption is incremental by construction.
//!
//! # Resilience
//!
//! A scan of a real library will meet unreadable directories, permission
//! holes, and corrupt files. None of these abort the scan: per-file problems
//! are *data*, reported as [`ScanEntry::Failed`] so the UI can show them,
//! while the iterator keeps going. Only failure to start at all — a missing
//! or non-directory root — is a [`ScanError`].

use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use lofty::file::{FileType, TaggedFile};
use lofty::prelude::*;
use lofty::tag::{ItemKey, Tag};

pub mod inference;

/// File extensions (ASCII case-insensitive, without the dot) the scanner
/// treats as audio. This is *the* place to extend format support.
pub const AUDIO_EXTENSIONS: &[&str] = &["flac", "mp3", "ogg", "opus", "m4a", "mp4", "wav"];

/// The codec a track's samples are stored in.
///
/// This is the axis along which a multi-format collection splits into
/// *editions* (`docs/adr/0007-album-editions.md`): a collector who keeps a
/// lossless archive and a lossy copy for the phone has one album in two
/// codecs, and the shelf must say so.
///
/// Deliberately the **codec**, never the container or the folder name. A
/// library may be filed as `FLAC/…` and `MP3/…`, or may not be filed by
/// format at all; only what is inside the file is trustworthy. Every variant
/// here is a codec that one of [`AUDIO_EXTENSIONS`] actually carries —
/// anything else reads back as `None` (see [`TrackMeta::format`]) rather
/// than being guessed at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AudioFormat {
    /// Free Lossless Audio Codec — `.flac`, or FLAC in an Ogg/MP4 container.
    Flac,
    /// Apple Lossless — `.m4a` / `.mp4`.
    Alac,
    /// Linear PCM in a RIFF WAVE container — `.wav`.
    Wav,
    /// MPEG-1/2 Audio Layer III — `.mp3`.
    Mp3,
    /// Advanced Audio Coding — `.m4a` / `.mp4`.
    Aac,
    /// Ogg Vorbis — `.ogg`.
    Vorbis,
    /// Opus — `.opus`, or Opus in an Ogg container.
    Opus,
}

impl AudioFormat {
    /// Whether the codec reconstructs its source samples bit-exactly.
    ///
    /// This is the primary key of the default-edition ranking
    /// ([`crate::index::Album::editions`]): a lossless edition is preferred
    /// over a lossy one whatever their bitrates say, because "lossless" is a
    /// fact about the decoded samples and "bitrate" is a fact about the
    /// file.
    #[must_use]
    pub fn is_lossless(self) -> bool {
        matches!(self, Self::Flac | Self::Alac | Self::Wav)
    }

    /// The stable lowercase code used to persist this format in the index
    /// (`tracks.format`, schema v2). Never change an existing code: it is
    /// on-disk data, and [`AudioFormat::from_code`] is its only reader.
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::Flac => "flac",
            Self::Alac => "alac",
            Self::Wav => "wav",
            Self::Mp3 => "mp3",
            Self::Aac => "aac",
            Self::Vorbis => "vorbis",
            Self::Opus => "opus",
        }
    }

    /// Parse a [`AudioFormat::code`] back. An unrecognized code yields
    /// `None` — the same "unknown format" a never-scanned row carries — so a
    /// database holding a code this build does not know degrades to an
    /// unnamed edition instead of failing the open.
    #[must_use]
    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "flac" => Some(Self::Flac),
            "alac" => Some(Self::Alac),
            "wav" => Some(Self::Wav),
            "mp3" => Some(Self::Mp3),
            "aac" => Some(Self::Aac),
            "vorbis" => Some(Self::Vorbis),
            "opus" => Some(Self::Opus),
            _ => None,
        }
    }

    /// The name a listener would recognize, for edition labels.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Flac => "FLAC",
            Self::Alac => "ALAC",
            Self::Wav => "WAV",
            Self::Mp3 => "MP3",
            Self::Aac => "AAC",
            Self::Vorbis => "Vorbis",
            Self::Opus => "Opus",
        }
    }
}

impl fmt::Display for AudioFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Metadata for one audio file, as the indexer and shelf UI will consume it.
///
/// Every descriptive field is optional: files are the source of truth, and
/// when neither tags nor folder structure provide a value we report "unknown"
/// rather than invent one. Presentation choices ("Unknown Artist", filename
/// fallbacks) belong to the consumer.
///
/// The last four fields describe the *encoding* rather than the work. They
/// all come from the audio-property header lofty parses during the same read
/// as the tags — no extra I/O, no decoding — and they exist for one purpose:
/// splitting an album into editions and ranking them (see [`AudioFormat`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackMeta {
    /// Absolute path of the audio file (the file's identity; never inferred).
    pub path: PathBuf,
    /// Track artist, from tags or else the grandparent directory.
    pub artist: Option<String>,
    /// Album artist — who the *album* is filed under, as distinct from who
    /// performs an individual track. This is the tag that keeps a
    /// soundtrack or a compilation from shattering into one shelf entry per
    /// composer (`docs/adr/0008-album-artist-grouping.md`), read from
    /// whichever spelling the container uses: Vorbis `ALBUMARTIST` (and the
    /// non-standard `ALBUM ARTIST`), `ID3v2` `TPE2`, MP4 `aART`, APE
    /// `Album Artist`.
    ///
    /// `None` means the file did not say. It is **not** a claim that the
    /// album artist equals the track artist — that fallback is a grouping
    /// decision ([`crate::index::AlbumArtist`]), made where the album's
    /// whole track list is visible, not guessed at here.
    ///
    /// Folder inference fills this exactly when it fills
    /// [`TrackMeta::artist`] — from the grandparent directory of an
    /// `Artist/Album/track` layout — so a folder-organised library with no
    /// tags at all still groups correctly.
    pub album_artist: Option<String>,
    /// The file's compilation flag: `ID3v2` `TCMP`, MP4 `cpil`, Vorbis
    /// `COMPILATION`, APE `Compilation`.
    ///
    /// It earns its place by making one grouping case decidable that
    /// nothing else can decide: tracks by *different* artists that belong
    /// to one album and carry no album-artist tag. Without a signal, two
    /// artists who each released a "Greatest Hits" would merge into one
    /// shelf entry; with it, a flagged compilation groups under
    /// [`crate::index::AlbumArtist::Various`] instead.
    ///
    /// `None` is "the file said nothing", distinct from `Some(false)`.
    pub compilation: Option<bool>,
    /// Album title, from tags or else the parent directory.
    pub album: Option<String>,
    /// Track title, from tags or else the filename.
    pub title: Option<String>,
    /// Track number within the disc, from tags or else the filename.
    pub track: Option<u32>,
    /// Disc number, from tags only (folder layouts rarely encode it reliably).
    pub disc: Option<u32>,
    /// Release year, from tags only.
    pub year: Option<u32>,
    /// Playing time, when the format headers make it cheaply available
    /// during the tag read (no decoding is performed).
    pub duration: Option<Duration>,
    /// Codec the samples are stored in — the edition axis.
    ///
    /// `None` means "not known": a codec [`AudioFormat`] does not name, or an
    /// index row written before schema v2 that no rescan has refreshed yet.
    /// It never means "no format".
    pub format: Option<AudioFormat>,
    /// Bits per sample. Lossless codecs declare one; MP3/AAC/Vorbis/Opus do
    /// not — their internal precision is not a sample width — so `None` there
    /// is the truth, not a gap.
    pub bit_depth: Option<u8>,
    /// Sample rate in Hz.
    pub sample_rate: Option<u32>,
    /// Audio bitrate in kbit/s as declared or derived by the container; an
    /// average for VBR encodings.
    pub bitrate: Option<u32>,
}

/// One result from a running scan: a successfully read track, or a per-file
/// failure reported as data (see the module docs on resilience).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScanEntry {
    /// An audio file that was read successfully.
    Track(TrackMeta),
    /// A file or directory the scanner could not process. The scan continues
    /// past it; the consumer decides whether and how to surface it.
    Failed {
        /// The file or directory that failed.
        path: PathBuf,
        /// Human-readable cause (I/O error, unparsable container, …). Kept as
        /// a string because it exists to be shown and logged, not matched on.
        reason: String,
    },
}

/// A scan could not start. Per-file problems are never this — they are
/// [`ScanEntry::Failed`] items in the stream.
#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    /// The scan root does not exist.
    #[error("scan root `{}` does not exist", path.display())]
    RootNotFound {
        /// The root that was requested.
        path: PathBuf,
    },
    /// The scan root exists but is not a directory.
    #[error("scan root `{}` is not a directory", path.display())]
    RootNotDirectory {
        /// The root that was requested.
        path: PathBuf,
    },
}

/// Start scanning `root` for audio files.
///
/// Returns an iterator over [`ScanEntry`] — see the [module docs](self) for
/// why the API is shaped this way. Directory traversal order is unspecified.
///
/// # Errors
///
/// [`ScanError::RootNotFound`] if `root` does not exist, and
/// [`ScanError::RootNotDirectory`] if it is not a directory. Everything after
/// that is reported in-stream as [`ScanEntry::Failed`].
pub fn scan(root: impl AsRef<Path>) -> Result<Scan, ScanError> {
    let root = root.as_ref().to_path_buf();
    if !root.exists() {
        return Err(ScanError::RootNotFound { path: root });
    }
    if !root.is_dir() {
        return Err(ScanError::RootNotDirectory { path: root });
    }
    let walker = walkdir::WalkDir::new(&root).into_iter();
    Ok(Scan { root, walker })
}

/// A running scan; see [`scan`].
#[derive(Debug)]
pub struct Scan {
    root: PathBuf,
    walker: walkdir::IntoIter,
}

impl Iterator for Scan {
    type Item = ScanEntry;

    fn next(&mut self) -> Option<ScanEntry> {
        loop {
            let entry = match self.walker.next()? {
                Ok(entry) => entry,
                Err(err) => {
                    // Unreadable directory or similar: report it and keep
                    // walking the rest of the tree.
                    let path = err
                        .path()
                        .map_or_else(|| self.root.clone(), Path::to_path_buf);
                    return Some(ScanEntry::Failed {
                        path,
                        reason: err.to_string(),
                    });
                }
            };
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.into_path();
            if !has_audio_extension(&path) {
                continue;
            }
            return Some(read_track(&self.root, path));
        }
    }
}

/// Does the path carry one of [`AUDIO_EXTENSIONS`] (ASCII case-insensitive)?
fn has_audio_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            AUDIO_EXTENSIONS
                .iter()
                .any(|known| known.eq_ignore_ascii_case(ext))
        })
}

/// Read one audio file: tags via lofty, holes filled by path inference.
fn read_track(root: &Path, path: PathBuf) -> ScanEntry {
    match lofty::read_from_path(&path) {
        Ok(file) => ScanEntry::Track(build_meta(root, path, &file)),
        Err(err) => ScanEntry::Failed {
            path,
            reason: err.to_string(),
        },
    }
}

/// Merge tag data with folder-structure inference. Tags win field by field;
/// inference only fills what tags leave blank.
fn build_meta(root: &Path, path: PathBuf, file: &TaggedFile) -> TrackMeta {
    // Inference must only ever see the part of the path below the scan root,
    // so directories the user did not choose to scan cannot leak in.
    let relative = path.strip_prefix(root).unwrap_or(&path);
    let inferred = inference::infer_from_relative_path(relative);

    let tag = file.primary_tag().or_else(|| file.first_tag());
    let (artist, album_artist, compilation, album, title, track, disc, year) = match tag {
        Some(tag) => (
            tag.artist().as_deref().and_then(clean_str),
            album_artist(tag),
            compilation_flag(tag),
            tag.album().as_deref().and_then(clean_str),
            tag.title().as_deref().and_then(clean_str),
            nonzero(tag.track()),
            nonzero(tag.disk()),
            nonzero(tag.year()),
        ),
        None => (None, None, None, None, None, None, None, None),
    };

    let properties = file.properties();
    let duration = properties.duration();

    // Inference fills the album artist on exactly the terms it fills the
    // track artist: only when the tags left *both* blank. A file that names
    // its artist but not its album artist is trusted over its folder — the
    // "Beatles/" directory must not overrule an `ARTIST=The Beatles` tag —
    // and the album-artist fallback chain (`crate::index::AlbumArtist`)
    // reaches that tag anyway.
    let inferred_artist = inferred.artist;
    let album_artist = album_artist.or_else(|| {
        if artist.is_none() {
            inferred_artist.clone()
        } else {
            None
        }
    });

    TrackMeta {
        artist: artist.or(inferred_artist),
        album_artist,
        compilation,
        album: album.or(inferred.album),
        title: title.or(inferred.title),
        track: track.or(inferred.track),
        disc,
        year,
        duration: (!duration.is_zero()).then_some(duration),
        format: detect_format(file),
        bit_depth: properties.bit_depth(),
        sample_rate: nonzero(properties.sample_rate()),
        bitrate: nonzero(properties.audio_bitrate()),
        path,
    }
}

/// Which codec a file carries, from what lofty already parsed.
///
/// Most containers answer this outright. MP4 is the one ambiguous case:
/// `.m4a`/`.mp4` may hold ALAC or AAC, and the format-agnostic
/// `lofty::properties::FileProperties` that a `TaggedFile` exposes has
/// dropped the container-specific `Mp4Codec` by the time we see it. The
/// surviving discriminator is the declared bit depth: lofty fills it from
/// the `alac` sample-description atom and leaves it unset for AAC, which has
/// no sample width to declare. The one known false positive is FLAC-in-MP4
/// (which also declares a depth); it lands on [`AudioFormat::Alac`] — a
/// wrong *name* but the right fidelity tier, and a combination essentially
/// absent from real libraries.
///
/// Anything else — AIFF, APE, `WavPack`, Musepack, Speex, a codec lofty grows
/// later — is reported as `None`. The scanner only walks
/// [`AUDIO_EXTENSIONS`], so those arrive only from a mislabeled file, and an
/// honest "unknown" beats a guess.
fn detect_format(file: &TaggedFile) -> Option<AudioFormat> {
    match file.file_type() {
        FileType::Flac => Some(AudioFormat::Flac),
        FileType::Wav => Some(AudioFormat::Wav),
        FileType::Mpeg => Some(AudioFormat::Mp3),
        FileType::Aac => Some(AudioFormat::Aac),
        FileType::Vorbis => Some(AudioFormat::Vorbis),
        FileType::Opus => Some(AudioFormat::Opus),
        FileType::Mp4 => Some(if file.properties().bit_depth().is_some() {
            AudioFormat::Alac
        } else {
            AudioFormat::Aac
        }),
        _ => None,
    }
}

/// The album-artist tag, whatever the container calls it.
///
/// lofty's [`ItemKey::AlbumArtist`] already covers the mappings that matter
/// — Vorbis `ALBUMARTIST`, `ID3v2` `TPE2`, MP4 `aART`, APE `Album Artist` —
/// so the common case is one lookup. The second pass exists for the one
/// spelling real files use that no standard blesses and lofty therefore
/// leaves as an unmapped key: Vorbis comments written `ALBUM ARTIST` (or
/// `Album_Artist`) by older taggers. Punctuation-insensitive matching is
/// confined to that fallback, so a mapped tag is never second-guessed.
fn album_artist(tag: &Tag) -> Option<String> {
    if let Some(name) = tag.get_string(&ItemKey::AlbumArtist).and_then(clean_str) {
        return Some(name);
    }
    tag.items()
        .filter(|item| matches!(item.key(), ItemKey::Unknown(key) if is_album_artist_key(key)))
        .find_map(|item| item.value().text().and_then(clean_str))
}

/// Whether an unmapped tag key is a spelling of "album artist" — spaces and
/// underscores ignored, ASCII case ignored.
fn is_album_artist_key(key: &str) -> bool {
    let squashed: String = key.chars().filter(|c| !matches!(c, ' ' | '_')).collect();
    squashed.eq_ignore_ascii_case("albumartist")
}

/// The compilation flag, if the file sets one. See
/// [`TrackMeta::compilation`] for why it is read at all.
fn compilation_flag(tag: &Tag) -> Option<bool> {
    tag.get_string(&ItemKey::FlagCompilation)
        .and_then(parse_flag)
}

/// Parse a boolean tag value. Containers disagree on the spelling — MP4
/// stores a real boolean that lofty surfaces as `"1"`/`"0"`, `ID3v2` `TCMP`
/// and Vorbis `COMPILATION` are free text — so the spellings actually found
/// in the wild are accepted and anything else is `None` rather than
/// silently false.
fn parse_flag(value: &str) -> Option<bool> {
    let value = value.trim();
    if value == "1" || value.eq_ignore_ascii_case("true") || value.eq_ignore_ascii_case("yes") {
        return Some(true);
    }
    if value == "0" || value.eq_ignore_ascii_case("false") || value.eq_ignore_ascii_case("no") {
        return Some(false);
    }
    None
}

/// Messy-library hygiene: a tag field that is present but blank (or
/// whitespace) counts as absent, so inference can fill it.
fn clean_str(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

/// Numeric tag hygiene: `0` conventionally means "unset" for track/disc/year.
fn nonzero(value: Option<u32>) -> Option<u32> {
    value.filter(|&n| n != 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_extension_matching_is_case_insensitive() {
        assert!(has_audio_extension(Path::new("a/b/track.FLAC")));
        assert!(has_audio_extension(Path::new("track.Mp3")));
        assert!(has_audio_extension(Path::new("track.wav")));
        assert!(!has_audio_extension(Path::new("cover.jpg")));
        assert!(!has_audio_extension(Path::new("no_extension")));
        assert!(!has_audio_extension(Path::new(".flac"))); // dotfile, no ext
    }

    #[test]
    fn format_codes_roundtrip_and_classify_fidelity() {
        let all = [
            AudioFormat::Flac,
            AudioFormat::Alac,
            AudioFormat::Wav,
            AudioFormat::Mp3,
            AudioFormat::Aac,
            AudioFormat::Vorbis,
            AudioFormat::Opus,
        ];
        for format in all {
            assert_eq!(
                AudioFormat::from_code(format.code()),
                Some(format),
                "{format} must survive a round trip through its stored code"
            );
        }
        // Codes are unique, or two formats would collide in the database.
        let mut codes: Vec<&str> = all.iter().map(|f| f.code()).collect();
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), all.len());

        assert!(AudioFormat::Flac.is_lossless());
        assert!(AudioFormat::Alac.is_lossless());
        assert!(AudioFormat::Wav.is_lossless());
        assert!(!AudioFormat::Mp3.is_lossless());
        assert!(!AudioFormat::Aac.is_lossless());
        assert!(!AudioFormat::Vorbis.is_lossless());
        assert!(!AudioFormat::Opus.is_lossless());

        // A code from a future baz degrades to "unknown", never an error.
        assert_eq!(AudioFormat::from_code("wavpack"), None);
        assert_eq!(AudioFormat::from_code(""), None);
        // Codes are case-sensitive by construction; we only ever write ours.
        assert_eq!(AudioFormat::from_code("FLAC"), None);
        assert_eq!(AudioFormat::Flac.to_string(), "FLAC");
    }

    #[test]
    fn blank_and_zero_tag_values_count_as_absent() {
        assert_eq!(clean_str("  "), None);
        assert_eq!(clean_str(" x "), Some("x".to_owned()));
        assert_eq!(clean_str(""), None);
        assert_eq!(nonzero(Some(0)), None);
        assert_eq!(nonzero(Some(7)), Some(7));
    }

    #[test]
    fn non_standard_album_artist_spellings_are_recognized() {
        for spelling in [
            "ALBUM ARTIST",
            "Album Artist",
            "album artist",
            "ALBUM_ARTIST",
            "AlbumArtist",
        ] {
            assert!(
                is_album_artist_key(spelling),
                "{spelling} is an album-artist key in the wild"
            );
        }
        // Neighbouring keys must not be swallowed.
        for other in ["ALBUMARTISTSORT", "ALBUM ARTISTS", "ARTIST", "ALBUM", ""] {
            assert!(!is_album_artist_key(other), "{other} is a different tag");
        }
    }

    #[test]
    fn compilation_flag_spellings() {
        assert_eq!(parse_flag("1"), Some(true));
        assert_eq!(parse_flag(" true "), Some(true));
        assert_eq!(parse_flag("Yes"), Some(true));
        assert_eq!(parse_flag("0"), Some(false));
        assert_eq!(parse_flag("FALSE"), Some(false));
        assert_eq!(parse_flag("no"), Some(false));
        // Anything else is "the file did not say", never a silent false.
        assert_eq!(parse_flag(""), None);
        assert_eq!(parse_flag("2"), None);
        assert_eq!(parse_flag("maybe"), None);
    }
}
