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
    let (artist, album, title, track, disc, year) = match tag {
        Some(tag) => (
            clean_text(tag.artist()),
            clean_text(tag.album()),
            clean_text(tag.title()),
            nonzero(tag.track()),
            nonzero(tag.disk()),
            nonzero(tag.year()),
        ),
        None => (None, None, None, None, None, None),
    };

    let properties = file.properties();
    let duration = properties.duration();

    TrackMeta {
        artist: artist.or(inferred.artist),
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

/// Messy-library hygiene: a tag field that is present but blank (or
/// whitespace) counts as absent, so inference can fill it.
fn clean_text(value: Option<std::borrow::Cow<'_, str>>) -> Option<String> {
    value.and_then(|s| {
        let trimmed = s.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_owned())
    })
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
        assert_eq!(clean_text(Some("  ".into())), None);
        assert_eq!(clean_text(Some(" x ".into())), Some("x".to_owned()));
        assert_eq!(clean_text(None), None);
        assert_eq!(nonzero(Some(0)), None);
        assert_eq!(nonzero(Some(7)), Some(7));
    }
}
