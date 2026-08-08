//! The ledger's line format: encode one [`PlayRecord`] to one line, and read
//! one back.
//!
//! The format is described once, normatively, in [`crate::history`]'s module
//! docs; this module is its implementation and its round-trip tests. Nothing
//! here allocates outside the string it is building, and nothing here can
//! panic on hostile input — a ledger is a file the user may have edited by
//! hand, and a hand-edited line is an ordinary thing to meet, not a fault.

use std::borrow::Cow;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::history::PlayRecord;
use crate::protocol::PlayOutcome;

/// The field separator: one tab, so that `awk -F'\t'` addresses fields by
/// number and no field can be confused with the spaces inside a filename.
pub(crate) const SEPARATOR: char = '\t';

/// How many tab-separated fields a record line has.
#[cfg(test)]
pub(crate) const FIELDS: usize = 5;

/// What a record line writes for a track whose length the file never declared.
///
/// A dash rather than `0`: a reader must be able to tell "this container
/// declares no length" from "this track is zero seconds long", the same
/// distinction [`Event::Progress`](crate::protocol::Event::Progress) makes
/// with `null`.
pub(crate) const UNKNOWN: &str = "-";

/// The wire word for [`PlayOutcome::Played`] — a word rather than a flag,
/// because `grep played` is the whole point.
pub(crate) const PLAYED: &str = "played";

/// The wire word for [`PlayOutcome::Skipped`].
pub(crate) const SKIPPED: &str = "skipped";

/// The comment marker. A reader skips these lines; a human reads them.
pub(crate) const COMMENT: char = '#';

/// The header written when the ledger file is created, so that the format is
/// documented inside the file rather than only in this repository.
///
/// It is written **once**, when the file is empty, and is never rewritten —
/// see [`crate::history`] on why nothing here ever rewrites anything.
pub(crate) const HEADER: &str = concat!(
    "# baz play history. One line per play, appended, never rewritten.\n",
    "# Fields, tab-separated: started_utc, outcome, listened_ms, track_ms, path\n",
    "# started_utc  ISO-8601 UTC, when the track's first audio was heard\n",
    "# outcome      played | skipped (played = half the track, or 4 minutes)\n",
    "# listened_ms  milliseconds of this track's audio actually delivered\n",
    "# track_ms     the track's own length, or - when the file declares none\n",
    "# path         escapes: \\\\ \\t \\n \\r and \\xHH for non-UTF-8 bytes\n",
    "# Lines starting with # are comments. This is yours: grep it, back it up,\n",
    "# or delete it. Nothing here is sent anywhere. Format v1.\n",
);

/// What is appended to close off a line an interrupted write left unfinished.
///
/// A bare newline would not do, and the reason is the whole point of this
/// constant: `…\tplayed\t99\t100\t/music/Talk Ta` plus a newline is a *valid*
/// record naming a file that does not exist. So the terminator carries two
/// extra separators and a comment, which makes the line unparseable under every
/// truncation point — too many fields when the cut fell late, an empty
/// `track_ms` when it fell in the middle, too few fields when it fell early —
/// and legible to a human reading the file about what happened.
///
/// It is still an append. Not one byte that was already in the file changes.
pub(crate) const TRUNCATED: &str = "\t\t# incomplete line, closed off by baz\n";

/// Seconds in a minute.
const MINUTE: u64 = 60;
/// Seconds in an hour.
const HOUR: u64 = 60 * MINUTE;
/// Seconds in a day.
pub(crate) const DAY: u64 = 24 * HOUR;

/// Encode `record` as one ledger line, newline included.
pub(crate) fn encode(record: &PlayRecord) -> String {
    let outcome = match record.outcome {
        PlayOutcome::Skipped => SKIPPED,
        // `PlayOutcome` is `#[non_exhaustive]`; a variant added later is
        // recorded under its own name rather than silently as a play.
        _ => PLAYED,
    };
    let track_ms = match record.track_ms {
        Some(ms) => ms.to_string(),
        None => UNKNOWN.to_string(),
    };
    let mut line = String::with_capacity(64);
    line.push_str(&format_timestamp(record.started_unix_s));
    line.push(SEPARATOR);
    line.push_str(outcome);
    line.push(SEPARATOR);
    line.push_str(&record.listened_ms.to_string());
    line.push(SEPARATOR);
    line.push_str(&track_ms);
    line.push(SEPARATOR);
    escape_path(&record.path, &mut line);
    line.push('\n');
    line
}

/// Read one ledger line back, or `None` if it is not one.
///
/// `None` covers every kind of damage a line can suffer — a truncated tail, a
/// hand edit, a field that is not a number, an escape that does not exist — and
/// the caller's response to all of them is the same: skip this line and keep
/// the rest of the file. That is the whole corrupt-line policy, and it lives
/// here so that no caller has to invent one.
///
/// The trailing newline may be present or absent; `\r\n` is tolerated so a
/// ledger copied through a Windows editor still reads.
pub(crate) fn decode(line: &str) -> Option<PlayRecord> {
    let line = line.strip_suffix('\n').unwrap_or(line);
    let line = line.strip_suffix('\r').unwrap_or(line);
    if line.is_empty() || line.starts_with(COMMENT) {
        return None;
    }
    let mut fields = line.split(SEPARATOR);
    let started = fields.next()?;
    let outcome = fields.next()?;
    let listened = fields.next()?;
    let track = fields.next()?;
    let path = fields.next()?;
    if fields.next().is_some() {
        // More than five fields means a raw tab reached the file, which the
        // escaper cannot produce: the line was not written by baz and its
        // field boundaries are not knowable.
        return None;
    }
    let outcome = match outcome {
        PLAYED => PlayOutcome::Played,
        SKIPPED => PlayOutcome::Skipped,
        _ => return None,
    };
    let track_ms = if track == UNKNOWN {
        None
    } else {
        Some(track.parse().ok()?)
    };
    Some(PlayRecord {
        started_unix_s: parse_timestamp(started)?,
        outcome,
        listened_ms: listened.parse().ok()?,
        track_ms,
        path: unescape_path(path)?,
    })
}

/// `1786_000_000` → `"2026-08-08T19:06:40Z"`.
///
/// Seconds resolution, `Z`, no offset, no sub-second part: the ledger's
/// timestamps sort lexicographically in exactly the order they happened, which
/// is what makes `sort` and `grep '^2026-08'` work on the file without any tool
/// knowing what a date is.
pub(crate) fn format_timestamp(unix_s: u64) -> String {
    let (year, month, day) = civil_from_days(unix_s / DAY);
    let secs = unix_s % DAY;
    let (hour, minute, second) = (secs / HOUR, (secs % HOUR) / MINUTE, secs % MINUTE);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

/// `"2026-08-08T19:06:40Z"` → `1786_000_000`, or `None` if it is not exactly
/// that shape.
///
/// Strict on purpose. The check is a round trip — parse, re-format, compare —
/// which rejects `2026-02-30`, `2026-13-01` and `2026-08-08T25:00:00Z` without
/// a calendar table, because none of them is what
/// [`format_timestamp`] would have written.
pub(crate) fn parse_timestamp(text: &str) -> Option<u64> {
    let bytes = text.as_bytes();
    // YYYY-MM-DDTHH:MM:SSZ — twenty characters, all ASCII.
    if bytes.len() < 20 || !text.is_ascii() {
        return None;
    }
    // `MM-DDTHH:MM:SSZ` is fifteen characters; everything before it is the
    // year and its separator, so a five-digit year still parses.
    let (date, rest) = text.split_at(text.len() - 15);
    let year: u64 = date.strip_suffix('-')?.parse().ok()?;
    let month: u64 = rest.get(0..2)?.parse().ok()?;
    let day: u64 = rest.get(3..5)?.parse().ok()?;
    let hour: u64 = rest.get(6..8)?.parse().ok()?;
    let minute: u64 = rest.get(9..11)?.parse().ok()?;
    let second: u64 = rest.get(12..14)?.parse().ok()?;
    if rest.as_bytes().get(2) != Some(&b'-')
        || rest.as_bytes().get(5) != Some(&b'T')
        || rest.as_bytes().get(8) != Some(&b':')
        || rest.as_bytes().get(11) != Some(&b':')
        || rest.as_bytes().get(14) != Some(&b'Z')
        || rest.len() != 15
        || month == 0
        || month > 12
        || day == 0
        || day > 31
    {
        return None;
    }
    let unix_s = days_from_civil(year, month, day)?
        .checked_mul(DAY)?
        .checked_add(hour * HOUR + minute * MINUTE + second)?;
    // The round trip is the validator: anything the formatter would not have
    // written is not a timestamp this ledger can have contained.
    (format_timestamp(unix_s) == text).then_some(unix_s)
}

/// Days since the Unix epoch → the civil `(year, month, day)` they name.
///
/// Howard Hinnant's `civil_from_days`, which is exact for every date the
/// proleptic Gregorian calendar has and needs no table and no leap-year special
/// case. Unsigned throughout because the ledger's timestamps are `u64` seconds
/// since 1970 and so are never before the epoch.
fn civil_from_days(days: u64) -> (u64, u64, u64) {
    // Shift the epoch to 0000-03-01, which puts the leap day at the end of the
    // year and makes the month arithmetic below a single linear formula.
    let z = days + 719_468;
    let era = z / 146_097;
    let day_of_era = z - era * 146_097; // [0, 146_096]
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153; // [0, 11], March = 0
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    (if month <= 2 { year + 1 } else { year }, month, day)
}

/// The inverse of [`civil_from_days`]. `None` for a date before the epoch,
/// which this ledger cannot hold.
fn days_from_civil(year: u64, month: u64, day: u64) -> Option<u64> {
    let year = if month <= 2 {
        year.checked_sub(1)?
    } else {
        year
    };
    let era = year / 400;
    let year_of_era = year - era * 400;
    let shifted_month = if month > 2 { month - 3 } else { month + 9 };
    let day_of_year = (153 * shifted_month + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    (era * 146_097 + day_of_era).checked_sub(719_468)
}

/// Append `path`, escaped, to `out`.
///
/// The escape set is the smallest one that keeps the file line-oriented,
/// tab-delimited, valid UTF-8 and reversible: backslash (so the escape is
/// itself escapable), tab (the separator), CR and LF (the record separator),
/// the other C0 controls (which would otherwise make `less` unreadable), and
/// `\xHH` for any byte that is not part of valid UTF-8.
///
/// Everything else — every ordinary path, in every language — travels
/// verbatim, which is the property that makes `grep 'Talk Talk'` find what a
/// human expects it to find.
pub(crate) fn escape_path(path: &Path, out: &mut String) {
    let bytes = path_bytes(path);
    let mut rest: &[u8] = &bytes;
    loop {
        match std::str::from_utf8(rest) {
            Ok(text) => {
                escape_str(text, out);
                return;
            }
            Err(error) => {
                let (good, bad) = rest.split_at(error.valid_up_to());
                if let Ok(text) = std::str::from_utf8(good) {
                    escape_str(text, out);
                }
                // `None` means the input ended mid-sequence: every remaining
                // byte is unusable, so escape them all.
                let broken = error.error_len().unwrap_or(bad.len());
                for byte in &bad[..broken] {
                    let _ = write!(out, "\\x{byte:02X}");
                }
                rest = &bad[broken..];
                if rest.is_empty() {
                    return;
                }
            }
        }
    }
}

/// The UTF-8 half of [`escape_path`].
fn escape_str(text: &str, out: &mut String) {
    for ch in text.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                let _ = write!(out, "\\x{:02X}", c as u32);
            }
            c => out.push(c),
        }
    }
}

/// The inverse of [`escape_path`]. `None` for an escape that does not exist,
/// which is a line this ledger did not write.
pub(crate) fn unescape_path(field: &str) -> Option<PathBuf> {
    let mut bytes = Vec::with_capacity(field.len());
    let mut chars = field.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            let mut buf = [0u8; 4];
            bytes.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
            continue;
        }
        match chars.next()? {
            '\\' => bytes.push(b'\\'),
            't' => bytes.push(b'\t'),
            'n' => bytes.push(b'\n'),
            'r' => bytes.push(b'\r'),
            'x' => {
                let hi = chars.next()?.to_digit(16)?;
                let lo = chars.next()?.to_digit(16)?;
                // Both digits are one nibble, so the product is a byte.
                #[allow(clippy::cast_possible_truncation)]
                bytes.push((hi * 16 + lo) as u8);
            }
            _ => return None,
        }
    }
    Some(path_from_bytes(bytes))
}

/// A path's bytes, on a platform whose paths *are* bytes.
#[cfg(unix)]
fn path_bytes(path: &Path) -> Cow<'_, [u8]> {
    use std::os::unix::ffi::OsStrExt;
    Cow::Borrowed(path.as_os_str().as_bytes())
}

/// A path's bytes elsewhere.
///
/// Windows paths are UTF-16, and the only sequences that do not convert are
/// unpaired surrogates — which no filesystem API produces from a real name and
/// which the [`crate::protocol`] already refuses to put on the wire. Such a
/// path is recorded with the replacement character rather than refused, which
/// is documented in [`crate::history`] and is the same trade the protocol
/// makes.
#[cfg(not(unix))]
fn path_bytes(path: &Path) -> Cow<'_, [u8]> {
    Cow::Owned(path.to_string_lossy().into_owned().into_bytes())
}

/// The inverse of [`path_bytes`].
#[cfg(unix)]
fn path_from_bytes(bytes: Vec<u8>) -> PathBuf {
    use std::os::unix::ffi::OsStringExt;
    PathBuf::from(std::ffi::OsString::from_vec(bytes))
}

/// The inverse of [`path_bytes`], elsewhere.
#[cfg(not(unix))]
fn path_from_bytes(bytes: Vec<u8>) -> PathBuf {
    PathBuf::from(String::from_utf8_lossy(&bytes).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(path: &str) -> PlayRecord {
        PlayRecord {
            started_unix_s: 1_786_000_000,
            outcome: PlayOutcome::Played,
            listened_ms: 231_480,
            track_ms: Some(245_013),
            path: PathBuf::from(path),
        }
    }

    /// The example the module docs and the ADR both quote, byte for byte. If
    /// this changes, the documented format changed, and that is a decision
    /// rather than a refactor.
    #[test]
    fn the_documented_line_is_the_line_that_is_written() {
        let line = encode(&record("/home/matt/Music/Talk Talk/01 Myrrhman.flac"));
        assert_eq!(
            line,
            "2026-08-06T07:06:40Z\tplayed\t231480\t245013\t\
             /home/matt/Music/Talk Talk/01 Myrrhman.flac\n"
        );
    }

    #[test]
    fn an_unknown_track_length_is_a_dash_not_a_zero() {
        let mut rec = record("/a.mp3");
        rec.track_ms = None;
        let line = encode(&rec);
        assert!(line.contains("\t-\t"), "{line}");
        assert_eq!(decode(&line).expect("decodes").track_ms, None);
    }

    #[test]
    fn a_skip_says_so_in_a_word_a_human_can_grep() {
        let mut rec = record("/a.mp3");
        rec.outcome = PlayOutcome::Skipped;
        assert!(encode(&rec).contains("\tskipped\t"));
    }

    /// The separator, the record separator and the escape character itself,
    /// all inside one path — the three characters that would otherwise make
    /// the format ambiguous.
    #[test]
    fn a_path_holding_the_separator_a_newline_and_a_backslash_round_trips() {
        for path in [
            "/music/tab\there.flac",
            "/music/new\nline.flac",
            "/music/carriage\rreturn.flac",
            "/music/back\\slash.flac",
            "/music/all\t\n\r\\ of them.flac",
            "/music/\u{1}control.flac",
        ] {
            let line = encode(&record(path));
            assert_eq!(line.matches('\t').count(), FIELDS - 1, "{line:?}");
            assert_eq!(line.matches('\n').count(), 1, "{line:?}");
            let back = decode(&line).expect("decodes");
            assert_eq!(back.path, PathBuf::from(path));
        }
    }

    /// Ordinary paths are not mangled: the file stays greppable by eye.
    #[test]
    fn an_ordinary_path_travels_verbatim() {
        for path in [
            "/home/matt/Music/Sigur Rós/Ágætis byrjun/01 Svefn-g-englar.flac",
            "/mnt/nas/音楽/坂本龍一/01.flac",
            "/music/it's a 'quoted' \"name\".flac",
        ] {
            let line = encode(&record(path));
            assert!(line.contains(path), "{line}");
            assert_eq!(decode(&line).expect("decodes").path, PathBuf::from(path));
        }
    }

    /// A path that is not valid UTF-8 at all — the case a byte-oriented
    /// filesystem allows and a text format has to answer for.
    #[cfg(unix)]
    #[test]
    fn a_non_utf8_path_round_trips_through_hex_escapes() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;

        for bytes in [
            vec![b'/', 0xff, 0xfe, b'.', b'f', b'l', b'a', b'c'],
            vec![b'/', b'a', 0x80, b'/', b'b', 0xc3, b'.', b'w', b'a', b'v'],
            // A truncated multi-byte sequence at the very end.
            vec![b'/', b'x', 0xe2, 0x82],
        ] {
            let path = PathBuf::from(OsString::from_vec(bytes.clone()));
            let mut rec = record("/placeholder");
            rec.path = path.clone();
            let line = encode(&rec);
            assert!(line.is_ascii() || std::str::from_utf8(line.as_bytes()).is_ok());
            let back = decode(&line).expect("decodes");
            assert_eq!(back.path, path);
        }
    }

    /// The escaped form is always valid UTF-8, whatever the path was — which
    /// is what keeps `grep`, `less` and every editor working on the file.
    #[cfg(unix)]
    #[test]
    fn the_encoded_line_is_always_valid_utf8() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;

        let path = PathBuf::from(OsString::from_vec(vec![0xff, 0xfe, 0xfd, 0x80]));
        let mut rec = record("/x");
        rec.path = path;
        let line = encode(&rec);
        assert!(std::str::from_utf8(line.as_bytes()).is_ok());
    }

    #[test]
    fn a_damaged_line_is_none_rather_than_a_panic() {
        for line in [
            "",
            "\n",
            "# a comment",
            "2026-08-06T07:06:40Z",
            "2026-08-06T07:06:40Z\tplayed\t1\t2",
            "2026-08-06T07:06:40Z\tplayed\t1\t2\t/a\textra",
            "2026-08-06T07:06:40Z\tlistened\t1\t2\t/a",
            "2026-08-06T07:06:40Z\tplayed\tnot-a-number\t2\t/a",
            "2026-08-06T07:06:40Z\tplayed\t1\tnot-a-number\t/a",
            "not-a-date\tplayed\t1\t2\t/a",
            "2026-13-08T02:26:40Z\tplayed\t1\t2\t/a",
            "2026-02-30T02:26:40Z\tplayed\t1\t2\t/a",
            "2026-08-08T25:26:40Z\tplayed\t1\t2\t/a",
            // An escape that does not exist.
            "2026-08-06T07:06:40Z\tplayed\t1\t2\t/a\\q",
            // A hex escape cut in half.
            "2026-08-06T07:06:40Z\tplayed\t1\t2\t/a\\x4",
        ] {
            assert!(decode(line).is_none(), "{line:?}");
        }
    }

    /// A ledger that has been through a Windows editor still reads.
    #[test]
    fn a_crlf_line_ending_is_tolerated() {
        let line = encode(&record("/a.flac")).replace('\n', "\r\n");
        assert_eq!(
            decode(&line).expect("decodes").path,
            PathBuf::from("/a.flac")
        );
    }

    /// Against dates computed by hand, not against this code's own output.
    #[test]
    fn timestamps_match_known_instants() {
        for (unix_s, text) in [
            (0, "1970-01-01T00:00:00Z"),
            (1, "1970-01-01T00:00:01Z"),
            (86_399, "1970-01-01T23:59:59Z"),
            (86_400, "1970-01-02T00:00:00Z"),
            // 2000 is a leap year (divisible by 400): 29 February exists.
            (951_782_400, "2000-02-29T00:00:00Z"),
            // 2100 will not be — the century rule. The day after 28 February
            // 2100 is 1 March, with no 29th between them.
            (4_107_542_400, "2100-03-01T00:00:00Z"),
            (1_000_000_000, "2001-09-09T01:46:40Z"),
            (4_107_456_000, "2100-02-28T00:00:00Z"),
            (1_234_567_890, "2009-02-13T23:31:30Z"),
            // The 32-bit signed epoch rollover, which this format does not have.
            (2_147_483_647, "2038-01-19T03:14:07Z"),
            (1_786_000_000, "2026-08-06T07:06:40Z"),
        ] {
            assert_eq!(format_timestamp(unix_s), text, "{unix_s}");
            assert_eq!(parse_timestamp(text), Some(unix_s), "{text}");
        }
    }

    /// Every second of a leap year and the years either side of it, formatted
    /// and read back — the exhaustive version of the table above.
    #[test]
    fn every_day_of_four_years_round_trips() {
        // 2024-01-01 through 2027-12-31, one sample per day.
        let start = 1_704_067_200;
        for day in 0..(4 * 366) {
            let unix_s = start + day * DAY + 12 * HOUR + 34 * MINUTE + 56;
            let text = format_timestamp(unix_s);
            assert_eq!(parse_timestamp(&text), Some(unix_s), "{text}");
        }
    }

    /// Lexicographic order is chronological order — the property that makes
    /// `sort` and `grep '^2026-08'` work on the file itself.
    #[test]
    fn timestamps_sort_in_the_order_they_happened() {
        let mut stamps: Vec<String> = [1_786_000_000u64, 0, 1_000_000_000, 2_147_483_647]
            .iter()
            .map(|s| format_timestamp(*s))
            .collect();
        let chronological = {
            let mut secs = [1_786_000_000u64, 0, 1_000_000_000, 2_147_483_647];
            secs.sort_unstable();
            secs.iter()
                .map(|s| format_timestamp(*s))
                .collect::<Vec<_>>()
        };
        stamps.sort();
        assert_eq!(stamps, chronological);
    }

    #[test]
    fn the_header_is_all_comments() {
        for line in HEADER.lines() {
            assert!(line.starts_with(COMMENT), "{line:?}");
            assert!(decode(line).is_none(), "{line:?}");
        }
    }
}
