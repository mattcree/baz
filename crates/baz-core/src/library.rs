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

use std::path::{Path, PathBuf};
use std::time::Duration;

use lofty::file::TaggedFile;
use lofty::prelude::*;

pub mod inference;

/// File extensions (ASCII case-insensitive, without the dot) the scanner
/// treats as audio. This is *the* place to extend format support.
pub const AUDIO_EXTENSIONS: &[&str] = &["flac", "mp3", "ogg", "opus", "m4a", "mp4", "wav"];

/// Metadata for one audio file, as the indexer and shelf UI will consume it.
///
/// Every descriptive field is optional: files are the source of truth, and
/// when neither tags nor folder structure provide a value we report "unknown"
/// rather than invent one. Presentation choices ("Unknown Artist", filename
/// fallbacks) belong to the consumer.
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

    let duration = file.properties().duration();

    TrackMeta {
        artist: artist.or(inferred.artist),
        album: album.or(inferred.album),
        title: title.or(inferred.title),
        track: track.or(inferred.track),
        disc,
        year,
        duration: (!duration.is_zero()).then_some(duration),
        path,
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
    fn blank_and_zero_tag_values_count_as_absent() {
        assert_eq!(clean_text(Some("  ".into())), None);
        assert_eq!(clean_text(Some(" x ".into())), Some("x".to_owned()));
        assert_eq!(clean_text(None), None);
        assert_eq!(nonzero(Some(0)), None);
        assert_eq!(nonzero(Some(7)), Some(7));
    }
}
