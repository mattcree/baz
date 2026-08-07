//! Pure folder-structure and filename inference for untagged files.
//!
//! A large share of real libraries is messy: untagged rips, half-tagged
//! downloads, WAVs with no tag container at all. The vision doc commits us to
//! being *excellent with messy libraries*, so when tags are absent the scanner
//! falls back to the one structure almost every collection shares:
//! `Artist/Album/NN - Title.ext`. Tags always win over inference, field by
//! field — inference only ever fills holes (that merge lives in
//! [`super::scan`]'s track builder; this module is the pure parsing half).
//!
//! Everything here is pure string/path manipulation with no I/O, which is what
//! makes it independently unit-testable and fuzzable
//! (`fuzz/fuzz_targets/scanner_inference.rs`). Invariants the fuzz target
//! asserts: no panics on any input, returned strings are trimmed and
//! non-empty, and any inferred track number is in `1..=999`.

use std::ffi::OsStr;
use std::path::{Component, Path};

/// A track number needs at most this many leading digits; anything longer
/// ("1999 - Party", catalogue numbers, timestamps) is almost never a track
/// number, so we refuse to guess.
const MAX_TRACK_DIGITS: usize = 3;

/// Metadata inferred from a file's path relative to the scan root.
///
/// Every field is optional: inference never invents data it cannot see.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InferredTrack {
    /// Artist, taken from the grandparent directory (`Artist/Album/file`).
    pub artist: Option<String>,
    /// Album, taken from the parent directory. When the file sits only one
    /// directory below the scan root, that directory is treated as the album
    /// (the shelf is album-oriented) and the artist is left unknown.
    pub album: Option<String>,
    /// Title parsed from the filename (see [`parse_filename`]).
    pub title: Option<String>,
    /// Track number parsed from the filename (see [`parse_filename`]).
    pub track: Option<u32>,
}

/// The result of parsing a filename stem: `"01 - Title"` → track 1, "Title".
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParsedFilename {
    /// Leading track number, if the stem matches a track-number pattern.
    /// Always in `1..=999`: `0` means "unknown" in the wild and long runs of
    /// digits are more likely years or IDs, so neither is guessed at.
    pub track: Option<u32>,
    /// The title portion of the stem, trimmed. `None` only when nothing
    /// usable remains (empty stem, or a bare `"01 - "`).
    pub title: Option<String>,
}

/// Infer artist/album/title/track from a path *relative to the scan root*.
///
/// The path must be relative so that directories above the scanned folder
/// (`/home`, `Music`, …) can never leak into the metadata: only components
/// the user chose to scan participate. The last two directory components are
/// read as `Artist/Album` (just `Album` if there is only one); the filename
/// stem goes through [`parse_filename`].
///
/// Non-UTF-8 path components are converted lossily (U+FFFD replacement):
/// inferred fields are display metadata, and a mangled-but-visible name beats
/// dropping the track. The file's [`path`](super::TrackMeta::path) itself is
/// never touched by inference.
#[must_use]
pub fn infer_from_relative_path(relative: &Path) -> InferredTrack {
    let mut dirs: Vec<&OsStr> = relative
        .components()
        .filter_map(|c| match c {
            Component::Normal(os) => Some(os),
            _ => None,
        })
        .collect();

    let Some(file_name) = dirs.pop() else {
        return InferredTrack::default();
    };

    // A dotfile like ".flac" has no extension: `file_stem` keeps it whole.
    let stem = Path::new(file_name).file_stem().unwrap_or(file_name);
    let parsed = parse_filename(&stem.to_string_lossy());

    let album = dirs.pop().and_then(component_name);
    let artist = dirs.pop().and_then(component_name);

    InferredTrack {
        artist,
        album,
        title: parsed.title,
        track: parsed.track,
    }
}

/// Parse a filename stem (extension already removed) into an optional track
/// number and title.
///
/// Recognized patterns, in spirit: `"01 - Title"`, `"01. Title"`,
/// `"01_Title"`, `"1 Title"` — up to three leading digits, followed by
/// either an explicit separator (`-`, `.`, `_`, with optional whitespace) or
/// plain whitespace, then the title.
///
/// Deliberate refusals, because guessing wrong is worse than not guessing:
///
/// - Track `0` ("unknown" by convention) → no split, whole stem is the title.
/// - More than three digits (`"1999 - Party"`, `"99999 - x"`) → likelier a
///   year or an ID; whole stem is the title.
/// - Digits not followed by a separator or space (`"1st song"`, bare `"01"`)
///   → no split.
///
/// When no pattern matches, the whole trimmed stem becomes the title (or
/// `None` if the stem is blank). Returned strings are always trimmed and
/// non-empty.
#[must_use]
pub fn parse_filename(stem: &str) -> ParsedFilename {
    let stem = stem.trim();
    let fallback = ParsedFilename {
        track: None,
        title: non_empty(stem),
    };

    let digit_end = stem
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(stem.len());
    let digits = &stem[..digit_end];
    let rest = &stem[digit_end..];

    if digits.is_empty() || digits.len() > MAX_TRACK_DIGITS {
        return fallback;
    }
    // ≤ 3 ASCII digits always fit in u32.
    let Ok(number) = digits.parse::<u32>() else {
        return fallback;
    };

    let after_ws = rest.trim_start();
    let title_part = if let Some(after_sep) = after_ws.strip_prefix(['-', '.', '_']) {
        after_sep
    } else if rest.starts_with(char::is_whitespace) {
        // "1 Title": no separator, but the digits end at a word boundary.
        after_ws
    } else {
        return fallback;
    };

    if number == 0 {
        return fallback;
    }

    ParsedFilename {
        track: Some(number),
        title: non_empty(title_part),
    }
}

/// A directory component as display metadata: lossy UTF-8, trimmed, blank
/// components rejected.
fn component_name(os: &OsStr) -> Option<String> {
    non_empty(&os.to_string_lossy())
}

/// Trim `s` and return it as an owned string, or `None` if blank.
fn non_empty(s: &str) -> Option<String> {
    let trimmed = s.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(track: Option<u32>, title: Option<&str>) -> ParsedFilename {
        ParsedFilename {
            track,
            title: title.map(str::to_owned),
        }
    }

    #[test]
    fn common_track_number_patterns() {
        assert_eq!(parse_filename("01 - Title"), parsed(Some(1), Some("Title")));
        assert_eq!(parse_filename("01. Title"), parsed(Some(1), Some("Title")));
        assert_eq!(parse_filename("1 Title"), parsed(Some(1), Some("Title")));
        assert_eq!(
            parse_filename("07_Track Name"),
            parsed(Some(7), Some("Track Name"))
        );
        assert_eq!(parse_filename("10-fold"), parsed(Some(10), Some("fold")));
        assert_eq!(
            parse_filename("117 - Deep Cut"),
            parsed(Some(117), Some("Deep Cut"))
        );
    }

    #[test]
    fn plain_titles_pass_through() {
        assert_eq!(
            parse_filename("Title Only"),
            parsed(None, Some("Title Only"))
        );
        assert_eq!(
            parse_filename("  padded title  "),
            parsed(None, Some("padded title"))
        );
    }

    #[test]
    fn unicode_titles() {
        assert_eq!(
            parse_filename("12 - Größenwahn"),
            parsed(Some(12), Some("Größenwahn"))
        );
        assert_eq!(
            parse_filename("03. 春の歌"),
            parsed(Some(3), Some("春の歌"))
        );
        assert_eq!(
            parse_filename("Ólafur Arnalds"),
            parsed(None, Some("Ólafur Arnalds"))
        );
    }

    #[test]
    fn pathological_inputs() {
        assert_eq!(parse_filename(""), parsed(None, None));
        assert_eq!(parse_filename("   "), parsed(None, None));
        assert_eq!(parse_filename("..."), parsed(None, Some("...")));
        // Track 0 means "unknown"; refuse to split.
        assert_eq!(parse_filename("0 degrees"), parsed(None, Some("0 degrees")));
        assert_eq!(
            parse_filename("00 - Intro"),
            parsed(None, Some("00 - Intro"))
        );
        // Too many digits: likelier an ID or year than a track number.
        assert_eq!(parse_filename("99999 - x"), parsed(None, Some("99999 - x")));
        assert_eq!(
            parse_filename("1999 - Party"),
            parsed(None, Some("1999 - Party"))
        );
        // Digits fused to letters are not a track number.
        assert_eq!(parse_filename("1st song"), parsed(None, Some("1st song")));
        // A bare number is a title, not a trackless split.
        assert_eq!(parse_filename("01"), parsed(None, Some("01")));
        // Track number with nothing after it: keep the number, no title.
        assert_eq!(parse_filename("01 - "), parsed(Some(1), None));
    }

    #[test]
    fn full_layout_inference() {
        let inferred =
            infer_from_relative_path(Path::new("Big Star/Radio City/03 - Back of a Car.wav"));
        assert_eq!(
            inferred,
            InferredTrack {
                artist: Some("Big Star".to_owned()),
                album: Some("Radio City".to_owned()),
                title: Some("Back of a Car".to_owned()),
                track: Some(3),
            }
        );
    }

    #[test]
    fn single_directory_is_album_only() {
        let inferred = infer_from_relative_path(Path::new("Radio City/song.flac"));
        assert_eq!(inferred.artist, None);
        assert_eq!(inferred.album, Some("Radio City".to_owned()));
        assert_eq!(inferred.title, Some("song".to_owned()));
    }

    #[test]
    fn bare_file_and_empty_paths() {
        let inferred = infer_from_relative_path(Path::new("song.flac"));
        assert_eq!(inferred.artist, None);
        assert_eq!(inferred.album, None);
        assert_eq!(inferred.title, Some("song".to_owned()));

        assert_eq!(
            infer_from_relative_path(Path::new("")),
            InferredTrack::default()
        );
    }

    #[test]
    fn deep_paths_use_only_last_two_directories() {
        let inferred =
            infer_from_relative_path(Path::new("lossless/rips/Artist/Album/02 - Song.flac"));
        assert_eq!(inferred.artist, Some("Artist".to_owned()));
        assert_eq!(inferred.album, Some("Album".to_owned()));
        assert_eq!(inferred.track, Some(2));
    }
}
