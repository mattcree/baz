//! The M3U line format: read any playlist file the wild produces, write the
//! strict common subset back.
//!
//! The format is described once, normatively, in [`crate::playlist`]'s
//! module docs; this module is its implementation and its round-trip tests.
//! Two properties carry everything:
//!
//! - **Nothing here can panic and nothing here touches the filesystem.**
//!   Parsing is a pure function of bytes (path *resolution* is string work —
//!   joining and `~`-expansion — never a `stat`), which is what makes it
//!   fuzzable and what keeps "read" incapable of side effects.
//! - **`parse` → `render` → `parse` is a fixed point.** The first parse may
//!   normalise (a relative path becomes absolute, `#extinf` becomes
//!   `#EXTINF`, a fractional length is floored) — but what it produces
//!   re-reads to exactly itself, forever. The fuzz target
//!   (`fuzz/fuzz_targets/playlist_m3u.rs`) holds this on arbitrary bytes;
//!   the tests below hold it on the adversarial files a music folder
//!   actually contains.

use std::borrow::Cow;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use super::{Entry, ExtInf, Item, Note};

/// The `#EXTM3U` header. Optional on read (a bare list of paths is a
/// playlist); always written first.
pub(crate) const HEADER_LINE: &str = "#EXTM3U";

/// The one directive this module interprets: `#EXTINF:seconds,title`,
/// applying to the next path line.
const EXTINF_PREFIX: &str = "#EXTINF:";

/// The comment written above a path that is not valid UTF-8 (ADR-0024 §2).
///
/// baz's own, not the user's: it is skipped on read and regenerated on
/// write, so rewrites cannot multiply it — the one deliberate exception to
/// "comments are preserved verbatim". (A hand-written copy of this exact
/// line is therefore also absorbed.)
pub(crate) const NON_UTF8_WARNING: &str =
    "# baz: the path on the next line is not valid UTF-8 and is written byte-for-byte";

/// The UTF-8 byte-order mark, tolerated at the start of the file.
const BOM: &[u8] = b"\xef\xbb\xbf";

/// The liberal read. `directory` is the base for relative paths, `home` the
/// expansion of `~` — parameters rather than lookups so tests and the fuzz
/// target are deterministic; [`crate::playlist::parse`] supplies the real
/// home.
pub(crate) fn parse(bytes: &[u8], directory: &Path, home: Option<&Path>) -> Vec<Item> {
    let mut items = Vec::new();
    let mut pending: Option<ExtInf> = None;
    // `#EXTINF` lines demoted to notes because a nearer one superseded them,
    // held until the entry they sit above rather than pushed where they were
    // read. Why they are held is on the `pending.replace` below.
    let mut superseded: Vec<Note> = Vec::new();
    let mut header_consumed = false;
    for (index, raw) in bytes.split(|byte| *byte == b'\n').enumerate() {
        let raw = if index == 0 {
            raw.strip_prefix(BOM).unwrap_or(raw)
        } else {
            raw
        };
        // *Every* trailing carriage return, not one. A note is kept as `raw`
        // rather than as the trimmed line, so that a comment's leading
        // whitespace survives a rewrite — which means whatever `raw` still
        // ends with is what `render` will write, and `render` terminates its
        // lines with a bare `\n`. Strip one CR and `#\r\r\n` parses to a note
        // of `#\r`, renders as `#\r\n`, and parses back to a note of `#`: the
        // module's round-trip law broken by a rewrite that silently edits a
        // user's comment. Found by `fuzz/fuzz_targets/playlist_m3u.rs` in
        // three bytes; pinned by `a_note_ending_in_carriage_returns_survives
        // _a_rewrite`.
        //
        // Nothing is lost that a line-oriented format could have carried: a
        // run of CRs immediately before the line break is terminator noise in
        // every dialect of M3U, and a CR *inside* a line still travels
        // verbatim (`the_round_trip_law_holds_on_awkward_input`).
        let mut raw = raw;
        while let Some(shorter) = raw.strip_suffix(b"\r") {
            raw = shorter;
        }
        let line = raw.trim_ascii();
        if line.is_empty() {
            continue;
        }
        if line.first() == Some(&b'#') {
            // The header is consumed only as the first significant line;
            // a stray one later in the file is somebody's comment.
            if !header_consumed
                && items.is_empty()
                && pending.is_none()
                && line.eq_ignore_ascii_case(HEADER_LINE.as_bytes())
            {
                header_consumed = true;
                continue;
            }
            if line == NON_UTF8_WARNING.as_bytes() {
                continue;
            }
            if let Some(extinf) = parse_extinf(line) {
                if let Some(loser) = pending.replace(extinf) {
                    // Two #EXTINF lines before one path: the nearer one wins
                    // the entry; the earlier one is kept as a note rather
                    // than stripped.
                    //
                    // **Held until the entry, not pushed here**, and the
                    // reason is the round-trip law. A demoted `#EXTINF` is
                    // still a canonical `#EXTINF` line, so `render` writes it
                    // as one and the next `parse` reads it as one — it goes
                    // back into `pending` and is demoted a second time, at
                    // the position of the *winner*, which `render` has moved
                    // down to sit immediately above its path. Pushed where it
                    // was read, it therefore hops over any comment between
                    // the two on every rewrite: `parse(render(parse(f)))`
                    // returned the same items in a different order. Holding
                    // it puts it where the rewrite will put it anyway, which
                    // makes the first parse already a fixed point. Found by
                    // `fuzz/fuzz_targets/playlist_m3u.rs`; pinned by
                    // `a_superseded_extinf_does_not_hop_over_a_comment`.
                    superseded.push(Note(extinf_line(&loser).into_bytes()));
                }
                continue;
            }
            // Everything else — comments, provenance, directives from
            // other players, an #EXTINF too mangled to read — is preserved
            // exactly, leading whitespace and non-UTF-8 bytes included.
            items.push(Item::Note(Note(raw.to_vec())));
            continue;
        }
        items.extend(superseded.drain(..).map(Item::Note));
        items.push(Item::Entry(Entry {
            path: resolve(line, directory, home),
            extinf: pending.take(),
        }));
    }
    // Anything still held describes an entry that never arrived; it is still
    // the user's line, and it keeps its order relative to the dangling one.
    items.extend(superseded.into_iter().map(Item::Note));
    if let Some(dangling) = pending.take() {
        // An #EXTINF with no path after it describes nothing, but it is
        // still the user's line: kept as a note, not stripped.
        items.push(Item::Note(Note(extinf_line(&dangling).into_bytes())));
    }
    items
}

/// The strict write: header, then each item on its line — notes verbatim,
/// entries as `#EXTINF` (when known) above the path.
pub(crate) fn render(items: &[Item]) -> Vec<u8> {
    let mut out = Vec::with_capacity(64 * (items.len() + 1));
    out.extend_from_slice(HEADER_LINE.as_bytes());
    out.push(b'\n');
    for item in items {
        match item {
            Item::Note(note) => push_line(&mut out, note.bytes()),
            Item::Entry(entry) => {
                if let Some(extinf) = &entry.extinf {
                    push_line(&mut out, extinf_line(extinf).as_bytes());
                }
                let bytes = path_bytes(&entry.path);
                if std::str::from_utf8(&bytes).is_err() {
                    push_line(&mut out, NON_UTF8_WARNING.as_bytes());
                }
                push_line(&mut out, &bytes);
            }
        }
    }
    out
}

/// One line into the buffer, newline appended.
///
/// Any `\n` inside the bytes is dropped: the format is line-oriented and
/// has no escapes, so a line break simply cannot be carried. Parsed input
/// never contains one (lines are split on it); for caller-built entries
/// [`Playlist::save`](crate::playlist::Playlist::save) refuses first with
/// an error, so this is a belt for a strap that is already buckled.
fn push_line(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend(bytes.iter().copied().filter(|byte| *byte != b'\n'));
    out.push(b'\n');
}

/// Read `#EXTINF:seconds,title`, or `None` when the line is not one this
/// module can claim to understand — in which case the whole line is
/// preserved as a note instead, which is the honest reading of, say, the
/// IPTV dialect's `#EXTINF:-1 tvg-id="…",Title` (attributes this module
/// cannot round-trip must not be half-read and then dropped).
fn parse_extinf(line: &[u8]) -> Option<ExtInf> {
    let prefix = EXTINF_PREFIX.len();
    if line.len() < prefix || !line[..prefix].eq_ignore_ascii_case(EXTINF_PREFIX.as_bytes()) {
        return None;
    }
    let payload = std::str::from_utf8(&line[prefix..]).ok()?;
    let (duration, title) = payload.split_once(',').unwrap_or((payload, ""));
    Some(ExtInf {
        seconds: parse_seconds(duration)?,
        // `trim_end`, because [`extinf_line`] trims the end when it writes the
        // title back and this has to be the same shape or the round-trip law
        // fails. It is `str::trim_end` on both sides rather than
        // `trim_ascii_end` on either: the line has already had *ASCII*
        // trailing whitespace taken off by the caller, so what is left to
        // disagree about is exactly the characters the two functions treat
        // differently — a vertical tab is Unicode whitespace and is not ASCII
        // whitespace, and a title ending in one used to be shortened by the
        // first save and no other. Found by
        // `fuzz/fuzz_targets/playlist_m3u.rs`; pinned by
        // `a_title_ending_in_odd_whitespace_is_already_trimmed`.
        title: title.trim_end().to_string(),
    })
}

/// The duration field: `Some(Some(n))` for a length, `Some(None)` for the
/// format's `-1` ("unknown"), `None` for something that is not a number at
/// all — the caller then keeps the whole line verbatim.
// The nesting *is* the meaning: the outer level is "was this a number",
// the inner is `ExtInf::seconds` exactly as it will be stored. A dedicated
// enum would restate `Option<u64>` under new names.
#[allow(clippy::option_option)]
fn parse_seconds(field: &str) -> Option<Option<u64>> {
    let field = field.trim();
    if field.is_empty() {
        return None;
    }
    // Whole seconds first, exactly; only then the liberal numeric forms.
    if let Ok(seconds) = field.parse::<u64>() {
        return Some(Some(seconds));
    }
    if field.parse::<i64>().is_ok() {
        // An integer u64 refused: negative. The format's "unknown".
        return Some(None);
    }
    let value = field.parse::<f64>().ok()?;
    if !value.is_finite() {
        return None;
    }
    if value < 0.0 {
        return Some(None);
    }
    // 2^64, above which a u64 saturates.
    if value >= 18_446_744_073_709_551_616.0 {
        return Some(Some(u64::MAX));
    }
    // Fractional lengths are read rounded down; the sign and range are
    // checked just above.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Some(Some(value as u64))
}

/// The canonical `#EXTINF` line for `extinf`, no newline.
///
/// The title is flattened to something the line-oriented format can carry
/// and re-read identically: line breaks become spaces and trailing
/// whitespace goes (a reader trims line ends, so it could never come back).
/// For a title that came from [`parse_extinf`] both are no-ops.
fn extinf_line(extinf: &ExtInf) -> String {
    let mut line = String::from(EXTINF_PREFIX);
    match extinf.seconds {
        Some(seconds) => {
            let _ = write!(line, "{seconds}");
        }
        None => line.push_str("-1"),
    }
    line.push(',');
    let flat: String = extinf
        .title
        .chars()
        .map(|ch| if ch == '\n' { ' ' } else { ch })
        .collect();
    line.push_str(flat.trim_end());
    line
}

/// A path line into a [`PathBuf`]: `~` expanded, relative joined to the
/// playlist's own directory, absolute taken as written. String work only —
/// no `stat`, no canonicalise, no opinion on whether it exists.
fn resolve(line: &[u8], directory: &Path, home: Option<&Path>) -> PathBuf {
    if let Some(home) = home {
        if line == b"~" {
            return home.to_path_buf();
        }
        if let Some(rest) = line.strip_prefix(b"~/") {
            return home.join(path_from_bytes(rest));
        }
        #[cfg(windows)]
        if let Some(rest) = line.strip_prefix(br"~\") {
            return home.join(path_from_bytes(rest));
        }
    }
    let path = path_from_bytes(line);
    if path.is_absolute() {
        path
    } else {
        directory.join(path)
    }
}

/// A path's bytes, on a platform whose paths *are* bytes.
#[cfg(unix)]
pub(crate) fn path_bytes(path: &Path) -> Cow<'_, [u8]> {
    use std::os::unix::ffi::OsStrExt;
    Cow::Borrowed(path.as_os_str().as_bytes())
}

/// A path's bytes elsewhere.
///
/// Windows paths are UTF-16; the only unconvertible sequences are unpaired
/// surrogates, which no real filename round-trips anywhere and which are
/// written with the replacement character — the same trade
/// [`crate::history`] and [`crate::protocol`] make. So on Windows a written
/// path line is always valid UTF-8 and the warning comment never fires.
#[cfg(not(unix))]
pub(crate) fn path_bytes(path: &Path) -> Cow<'_, [u8]> {
    Cow::Owned(path.to_string_lossy().into_owned().into_bytes())
}

/// The inverse of [`path_bytes`]: a line's bytes as a path, verbatim.
#[cfg(unix)]
fn path_from_bytes(bytes: &[u8]) -> PathBuf {
    use std::os::unix::ffi::OsStrExt;
    PathBuf::from(std::ffi::OsStr::from_bytes(bytes))
}

/// The inverse of [`path_bytes`], elsewhere: foreign non-UTF-8 bytes in a
/// file read on Windows degrade to the replacement character, documented in
/// [`crate::playlist`].
#[cfg(not(unix))]
fn path_from_bytes(bytes: &[u8]) -> PathBuf {
    PathBuf::from(String::from_utf8_lossy(bytes).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIR: &str = "/music/playlists";
    const HOME: &str = "/home/matt";

    fn read(bytes: &[u8]) -> Vec<Item> {
        parse(bytes, Path::new(DIR), Some(Path::new(HOME)))
    }

    /// The round-trip law, applied to one file: the first parse may
    /// normalise, but its output re-reads to itself and re-renders to the
    /// same bytes, forever.
    fn law(source: &[u8]) {
        let first = read(source);
        let bytes = render(&first);
        let second = read(&bytes);
        assert_eq!(
            first,
            second,
            "parse(render(parse(f))) diverged for {:?}",
            String::from_utf8_lossy(source)
        );
        assert_eq!(
            render(&second),
            bytes,
            "the rewrite was not idempotent for {:?}",
            String::from_utf8_lossy(source)
        );
    }

    // ----- the round-trip law on adversarial files -------------------------

    #[test]
    fn the_round_trip_law_holds_on_the_files_the_wild_produces() {
        for source in [
            // The empty file, and the effectively empty ones.
            b"".as_slice(),
            b"\n\n\n",
            b"#EXTM3U\n",
            b"\xef\xbb\xbf#EXTM3U\r\n",
            // Headerless bare path list — the oldest dialect there is.
            b"/music/a.flac\n/music/b.flac\n",
            // No trailing newline on the last line.
            b"/music/a.flac\n/music/b.flac",
            // CRLF and a BOM together: the Windows-notepad special.
            b"\xef\xbb\xbf#EXTM3U\r\n#EXTINF:245,Talk Talk - Myrrhman\r\n/music/a.flac\r\n",
            // Blank lines everywhere, trailing ones included.
            b"\n#EXTM3U\n\n/music/a.flac\n\n\n/music/b.flac\n\n\n",
            // Foreign #EXT directives between entries, preserved in order.
            b"#EXTM3U\n#EXTGRP:driving\n/a.flac\n#EXTALB:Laughing Stock\n/b.flac\n",
            // Duplicates are allowed and must survive.
            b"/music/theme.flac\n/music/other.flac\n/music/theme.flac\n",
            // Comments, provenance, and a hand-indented comment.
            b"# made with baz on 2026-08-09\n  # a note to myself\n/a.flac\n",
            // Relative paths and ~.
            b"albums/one.flac\n../two.flac\n~/three.flac\n~\n",
            // EXTINF variants: unknown length, fractional, huge, no comma,
            // empty title, whitespace around the number, lowercase.
            b"#EXTINF:-1,Unknown Length\n/a.flac\n",
            b"#EXTINF:12.9,Fractional\n/a.flac\n",
            b"#EXTINF:99999999999999999999999,Huge\n/a.flac\n",
            b"#EXTINF:300\n/a.flac\n",
            b"#EXTINF:300,\n/a.flac\n",
            b"#EXTINF: 245 ,Spaced\n/a.flac\n",
            b"#extinf:245,lowercase\n/a.flac\n",
            // An IPTV-attribute EXTINF this module cannot claim to read:
            // preserved whole, not half-parsed.
            b"#EXTINF:-1 tvg-id=\"x\" group-title=\"y\",Channel\n/a.flac\n",
            // A mangled EXTINF: also a note, not a loss.
            b"#EXTINF:not-a-number,Broken\n/a.flac\n",
            // Two EXTINFs before one path; a dangling EXTINF at the end.
            b"#EXTINF:100,First\n#EXTINF:200,Second\n/a.flac\n",
            b"/a.flac\n#EXTINF:300,Dangling\n",
            // A directive between an EXTINF and its path.
            b"#EXTINF:245,Split\n#EXTGRP:g\n/a.flac\n",
            // A stray header mid-file is a comment, not a header.
            b"/a.flac\n#EXTM3U\n/b.flac\n",
            // Lone carriage returns inside a line survive.
            b"# odd\rcomment\n/music/odd\rname.flac\n",
        ] {
            law(source);
        }
    }

    /// A title ending in whitespace the writer would trim is trimmed on the
    /// way in, so the first save changes nothing.
    ///
    /// The third thing `playlist_m3u`'s fuzz target found, and the narrowest:
    /// `extinf_line` trims the title's end with `str::trim_end` (Unicode),
    /// while the reader kept whatever `trim_ascii` had left — and a **vertical
    /// tab** is Unicode whitespace but not ASCII whitespace, so it survived
    /// the read and died on the write.
    #[test]
    fn a_title_ending_in_odd_whitespace_is_already_trimmed() {
        for source in [
            b"#EXTINF:8,Title\x0b\n/a.flac\n".as_slice(),
            b"#EXTINF:8,Title\x0b\x0b\x0b\n/a.flac\n",
            b"#EXTINF:8,Title\xc2\xa0\n/a.flac\n", // U+00A0, also not ASCII
            b"#EXTINF:8,\x0b\n/a.flac\n",
        ] {
            law(source);
        }
        let items = read(b"#EXTINF:8,Title\x0b\n/a.flac\n");
        let Some(Item::Entry(entry)) = items.first() else {
            panic!("expected an entry, got {items:?}");
        };
        assert_eq!(
            entry.extinf.as_ref().map(|e| e.title.as_str()),
            Some("Title")
        );
        // A title with interior odd whitespace keeps all of it.
        let items = read(b"#EXTINF:8,Ti\x0btle\n/a.flac\n");
        let Some(Item::Entry(entry)) = items.first() else {
            panic!("expected an entry, got {items:?}");
        };
        assert_eq!(
            entry.extinf.as_ref().map(|e| e.title.as_str()),
            Some("Ti\x0btle")
        );
    }

    /// A superseded `#EXTINF` keeps its place relative to the comments around
    /// it, however many times the file is rewritten.
    ///
    /// The second thing `playlist_m3u`'s fuzz target found. A demoted
    /// `#EXTINF` is written back as an `#EXTINF`, so the next read demotes it
    /// again — at the winner's position, which `render` has moved down beside
    /// its path. Pushed where it was read, it hopped over the comment between
    /// them on every save.
    #[test]
    fn a_superseded_extinf_does_not_hop_over_a_comment() {
        for source in [
            b"#EXTINF:100,First\n# a comment\n#EXTINF:200,Second\n/a.flac\n".as_slice(),
            b"#EXTINF:100,First\n#EXTINF:200,Second\n# a comment\n/a.flac\n",
            b"#EXTINF:1,A\n# x\n#EXTINF:2,B\n# y\n#EXTINF:3,C\n/a.flac\n",
            // Superseded with no path at all: held to the end, still ahead of
            // the dangling one.
            b"#EXTINF:100,First\n# a comment\n#EXTINF:200,Dangling\n",
        ] {
            law(source);
        }
        // The winner is still the nearest one, which is the rule the holding
        // must not have changed.
        let items = read(b"#EXTINF:100,First\n# a comment\n#EXTINF:200,Second\n/a.flac\n");
        let Some(Item::Entry(entry)) = items.last() else {
            panic!("expected an entry last, got {items:?}");
        };
        assert_eq!(
            entry.extinf.as_ref().map(|e| e.title.as_str()),
            Some("Second")
        );
        // And the comment still leads the demoted line, in the file's order.
        assert!(
            matches!(&items[0], Item::Note(note) if note.bytes() == b"# a comment"),
            "{items:?}"
        );
    }

    /// A comment ending in carriage returns, in every arrangement — the class
    /// `playlist_m3u`'s fuzz target found, in three bytes (`#\r\r`).
    ///
    /// The note branch keeps `raw` rather than the trimmed line, so that a
    /// comment's leading whitespace survives; that made whatever trailing CRs
    /// the reader left behind part of the note's own bytes, and `render`
    /// terminates with a bare `\n`, so the next read ate one more of them.
    /// The rewrite silently shortened the user's comment, and did it again on
    /// every save until the CRs ran out.
    #[test]
    fn a_note_ending_in_carriage_returns_survives_a_rewrite() {
        for source in [
            b"#\r\r".as_slice(),
            b"#\r\r\n",
            b"#\r\r\r\r\r\n/a.flac\n",
            b"# comment\r\r\n/a.flac\n",
            b"   # indented\r\r\n",
            // Not a comment: an entry line, which is trimmed rather than kept
            // raw and so was never exposed to this — asserted, not assumed.
            b"/music/a.flac\r\r\n",
            // A CR run in the middle still travels; only the trailing run is
            // terminator noise.
            b"# before\r\rafter\r\r\n",
        ] {
            law(source);
        }
        // And the note that comes back says what it should: the interior run
        // intact, the trailing run gone.
        let items = read(b"# before\r\rafter\r\r\n");
        let Some(Item::Note(note)) = items.first() else {
            panic!("expected one note, got {items:?}");
        };
        assert_eq!(note.bytes(), b"# before\r\rafter");
    }

    #[cfg(unix)]
    #[test]
    fn the_round_trip_law_holds_on_non_utf8_bytes() {
        for source in [
            // A non-UTF-8 path, bare and with metadata.
            b"/music/\xff\xfe.flac\n".as_slice(),
            b"#EXTINF:100,Latin-1 leftovers\n/music/caf\xe9.flac\n",
            // A non-UTF-8 comment line.
            b"# \xff\xfe\n/a.flac\n",
            // A non-UTF-8 EXTINF payload: not claimable, kept verbatim.
            b"#EXTINF:100,caf\xe9\n/a.flac\n",
            // Garbage that is nothing at all.
            b"\xff\xfe\xfd\n\x80\x81\n",
        ] {
            law(source);
        }
    }

    // ----- what the liberal reader reads -----------------------------------

    #[test]
    fn a_headerless_bare_path_list_is_a_playlist() {
        let items = read(b"/music/a.flac\n/music/b.flac\n");
        assert_eq!(items.len(), 2);
        assert_eq!(
            items[0].as_entry().expect("entry").path,
            PathBuf::from("/music/a.flac")
        );
    }

    #[test]
    fn crlf_bom_and_blank_lines_do_not_reach_the_value() {
        let items = read(b"\xef\xbb\xbf#EXTM3U\r\n\r\n/music/a.flac\r\n\r\n");
        assert_eq!(items.len(), 1);
        assert_eq!(
            items[0].as_entry().expect("entry").path,
            PathBuf::from("/music/a.flac")
        );
    }

    #[test]
    fn relative_paths_resolve_against_the_files_own_directory() {
        let items = read(b"albums/one.flac\n../up.flac\n/abs.flac\n");
        let paths: Vec<&Path> = items
            .iter()
            .filter_map(Item::as_entry)
            .map(|entry| entry.path.as_path())
            .collect();
        assert_eq!(
            paths,
            [
                Path::new("/music/playlists/albums/one.flac"),
                // Joined, not normalised: this module invents no facts
                // about a filesystem it has not looked at.
                Path::new("/music/playlists/../up.flac"),
                Path::new("/abs.flac"),
            ]
        );
    }

    #[test]
    fn tilde_expands_to_home_and_survives_having_none() {
        let items = read(b"~/Music/a.flac\n~\n~oddname\n");
        let paths: Vec<&Path> = items
            .iter()
            .filter_map(Item::as_entry)
            .map(|entry| entry.path.as_path())
            .collect();
        assert_eq!(
            paths,
            [
                Path::new("/home/matt/Music/a.flac"),
                Path::new("/home/matt"),
                // `~user` expansion is a shell feature, not a path one:
                // a file literally named `~oddname` stays reachable.
                Path::new("/music/playlists/~oddname"),
            ]
        );
        // No home: the tilde is just a relative name.
        let homeless = parse(b"~/Music/a.flac\n", Path::new(DIR), None);
        assert_eq!(
            homeless[0].as_entry().expect("entry").path,
            Path::new("/music/playlists/~/Music/a.flac")
        );
    }

    #[test]
    fn extinf_metadata_attaches_to_the_next_path() {
        let items = read(b"#EXTINF:245,Talk Talk - Myrrhman\n/music/a.flac\n/music/b.flac\n");
        let with = items[0].as_entry().expect("entry");
        assert_eq!(
            with.extinf,
            Some(ExtInf {
                seconds: Some(245),
                title: "Talk Talk - Myrrhman".to_string(),
            })
        );
        assert_eq!(items[1].as_entry().expect("entry").extinf, None);
    }

    #[test]
    fn extinf_durations_read_liberally() {
        for (field, expected) in [
            (&b"#EXTINF:245,t\n/a\n"[..], Some(245)),
            (b"#EXTINF:-1,t\n/a\n", None),
            (b"#EXTINF:12.9,t\n/a\n", Some(12)),
            (b"#EXTINF:-0.5,t\n/a\n", None),
            (b"#EXTINF: 7 ,t\n/a\n", Some(7)),
            (b"#EXTINF:+3,t\n/a\n", Some(3)),
        ] {
            let items = read(field);
            let entry = items[0].as_entry().expect("entry");
            assert_eq!(
                entry.extinf.as_ref().expect("extinf").seconds,
                expected,
                "{}",
                String::from_utf8_lossy(field)
            );
        }
    }

    #[test]
    fn what_cannot_be_understood_is_kept_not_stripped() {
        let source = b"#EXTM3U\n#PLAYLIST:name\n#EXTINF:bad,x\n/a.flac\n# comment\n";
        let items = read(source);
        // Directive, mangled EXTINF, entry, comment — all present, in order.
        assert_eq!(items.len(), 4);
        assert!(matches!(&items[0], Item::Note(n) if n.bytes() == b"#PLAYLIST:name"));
        assert!(matches!(&items[1], Item::Note(n) if n.bytes() == b"#EXTINF:bad,x"));
        assert!(items[2].as_entry().is_some());
        assert!(matches!(&items[3], Item::Note(n) if n.bytes() == b"# comment"));
        // And the rewrite carries them, in their positions.
        let text = String::from_utf8(render(&items)).expect("utf8");
        assert_eq!(
            text,
            "#EXTM3U\n#PLAYLIST:name\n#EXTINF:bad,x\n/a.flac\n# comment\n"
        );
    }

    #[test]
    fn duplicates_survive_reading_and_writing() {
        let items = read(b"/theme.flac\n/other.flac\n/theme.flac\n");
        assert_eq!(items.len(), 3);
        let text = String::from_utf8(render(&items)).expect("utf8");
        assert_eq!(text.matches("/theme.flac\n").count(), 2);
    }

    #[cfg(unix)]
    #[test]
    fn a_non_utf8_path_reads_byte_verbatim() {
        use std::os::unix::ffi::OsStrExt;
        let items = read(b"/music/caf\xe9.flac\n");
        let entry = items[0].as_entry().expect("entry");
        assert_eq!(
            entry.path.as_os_str().as_bytes(),
            b"/music/caf\xe9.flac",
            "the bytes were not preserved"
        );
    }

    // ----- what the strict writer writes -----------------------------------

    #[test]
    fn the_written_file_is_the_documented_subset() {
        let items = read(b"albums/one.flac\n#EXTINF:245,Talk Talk - Myrrhman\n~/two.flac\n");
        let text = String::from_utf8(render(&items)).expect("the written file is UTF-8");
        // Header first, LF only, absolute paths, EXTINF where known. The
        // expected paths are built by the same joins the parser performs,
        // because on Windows a join writes a backslash and the assertion is
        // about resolution, not about the platform's separator.
        let one = Path::new(DIR).join("albums/one.flac");
        let two = Path::new(HOME).join("two.flac");
        assert_eq!(
            text,
            format!(
                "#EXTM3U\n{}\n#EXTINF:245,Talk Talk - Myrrhman\n{}\n",
                one.display(),
                two.display()
            )
        );
        assert!(!text.contains('\r'));
    }

    #[test]
    fn an_unknown_length_writes_as_minus_one() {
        let items = read(b"#EXTINF:-1,Unknown\n/a.flac\n");
        let text = String::from_utf8(render(&items)).expect("utf8");
        assert!(text.contains("#EXTINF:-1,Unknown\n"), "{text:?}");
    }

    #[cfg(unix)]
    #[test]
    fn a_non_utf8_path_writes_byte_verbatim_under_its_warning() {
        let items = read(b"/music/caf\xe9.flac\n");
        let bytes = render(&items);
        let expected: Vec<u8> = [
            HEADER_LINE.as_bytes(),
            b"\n",
            NON_UTF8_WARNING.as_bytes(),
            b"\n",
            b"/music/caf\xe9.flac\n",
        ]
        .concat();
        assert_eq!(bytes, expected);
        // And the warning does not multiply across rewrites.
        let again = render(&read(&bytes));
        assert_eq!(again, bytes);
    }

    #[test]
    fn a_caller_built_title_with_a_line_break_cannot_corrupt_the_file() {
        let items = vec![Item::Entry(Entry {
            path: PathBuf::from("/a.flac"),
            extinf: Some(ExtInf {
                seconds: Some(1),
                title: "broken\ntitle ".to_string(),
            }),
        })];
        let bytes = render(&items);
        let text = String::from_utf8(bytes.clone()).expect("utf8");
        assert_eq!(text, "#EXTM3U\n#EXTINF:1,broken title\n/a.flac\n");
        // Flattened once, stable thereafter.
        let reread = read(&bytes);
        assert_eq!(render(&reread), bytes);
    }

    #[test]
    fn a_note_from_text_always_reads_back_as_a_note() {
        for (text, expected) in [
            ("# already a comment", "# already a comment"),
            ("bare words", "# bare words"),
            ("with\nbreaks", "# with breaks"),
            ("  # indented", "  # indented"),
        ] {
            let note = Note::from_text(text);
            let items = vec![Item::Note(note.clone())];
            let reread = read(&render(&items));
            assert_eq!(reread, items, "{text:?}");
            assert_eq!(note.text(), expected, "{text:?}");
        }
    }
}
