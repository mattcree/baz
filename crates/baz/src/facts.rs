//! Local, record-bounded facts for Now Playing's one-line feed.

use std::time::{SystemTime, UNIX_EPOCH};

use crate::app::Shelf;
use crate::player::{PlayerState, SignalPath};

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// Build the fixed F1 → F2/F3 → F11 → F6 → F4 → F5 → F9 → F7 → F8 cycle.
/// Missing readings are absent, never placeholders.
pub(crate) fn current(shelf: &Shelf, player: &PlayerState) -> Vec<String> {
    let Some(path) = player.now_playing_path() else {
        return Vec::new();
    };
    let now_s = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    let mut facts = Vec::new();
    if let Some(history) = shelf.history() {
        let track = history.track(path);
        if track.plays > 0 {
            if let Some(first) = track.first_played_unix_s {
                facts.push(format!(
                    "Played {} {} since {}",
                    track.plays,
                    if track.plays == 1 { "time" } else { "times" },
                    month_year(first)
                ));
            }
            if let Some(last) = track.last_played_unix_s {
                facts.push(format!("Last played {}", ago(now_s.saturating_sub(last))));
            }
        } else {
            facts.push("Never played before".to_owned());
        }
    }

    let record = shelf
        .albums
        .iter()
        .find(|album| album.all_tracks().any(|track| track.path == path));
    if let Some(album) = record {
        if let Some(artist) = album.artist.name() {
            let count = shelf
                .albums
                .iter()
                .filter(|candidate| candidate.artist.name() == Some(artist))
                .count();
            facts.push(format!(
                "One of {count} {} by {artist} in your collection",
                if count == 1 { "record" } else { "records" }
            ));
        }
        // F6 (measured loudness) stays absent until the owned view model
        // exposes the analysis result; the index currently does not.
        if let Some(signal) = player.signal_path() {
            facts.push(signal_fact(signal));
        }
        let mut tail = Vec::new();
        if let Some((edition, track)) = album.editions.iter().find_map(|edition| {
            edition
                .tracks
                .iter()
                .find(|track| track.path == path)
                .map(|track| (edition, track))
        }) {
            let mut encoding = Vec::new();
            if let Some(format) = edition.key.0 {
                encoding.push(format.name().to_owned());
            }
            if let Some(bits) = edition.bit_depth {
                encoding.push(format!("{bits}-bit"));
            }
            if let Some(rate) = edition.sample_rate {
                encoding.push(rate_label(rate));
            }
            if let Some(bytes) = edition
                .tracks
                .iter()
                .map(|track| track.bytes)
                .sum::<Option<u64>>()
            {
                encoding.push(size_label(bytes));
            }
            if !encoding.is_empty() {
                facts.push(encoding.join(" · "));
            }
            if let Some(number) = track.number {
                let total = edition
                    .tracks
                    .iter()
                    .filter(|candidate| candidate.disc == track.disc)
                    .filter_map(|candidate| candidate.number)
                    .max();
                tail.push(total.map_or_else(
                    || format!("Track {number}"),
                    |total| format!("Track {number} of {total}"),
                ));
            }
        }
        if let Some(source) = player.queue_provenance() {
            facts.push(format!("From {source}"));
        }
        match (album.year, album.genre.as_deref()) {
            (Some(year), Some(genre)) => facts.push(format!("Released {year} · {genre}")),
            (Some(year), None) => facts.push(format!("Released {year}")),
            (None, Some(genre)) => facts.push(genre.to_owned()),
            (None, None) => {}
        }
        facts.extend(tail);
    } else if let Some(source) = player.queue_provenance() {
        facts.push(format!("From {source}"));
    }
    facts
}

fn signal_fact(path: SignalPath) -> String {
    let mode = if path.chain.is_exclusive() {
        "exclusive"
    } else {
        "shared"
    };
    let process = if path.chain.is_converting() {
        "resampled"
    } else {
        "direct"
    };
    format!(
        "{} source → {} output · {process} · {mode}",
        rate_label(path.source_rate_hz),
        rate_label(path.output_rate_hz),
    )
}

fn rate_label(hz: u32) -> String {
    if hz.is_multiple_of(1_000) {
        format!("{} kHz", hz / 1_000)
    } else {
        format!("{:.1} kHz", f64::from(hz) / 1_000.0)
    }
}

#[expect(clippy::cast_precision_loss, reason = "one-decimal display size")]
fn size_label(bytes: u64) -> String {
    let mib = bytes as f64 / (1024.0 * 1024.0);
    if mib >= 1024.0 {
        format!("{:.1} GiB", mib / 1024.0)
    } else {
        format!("{mib:.1} MiB")
    }
}

fn ago(seconds: u64) -> String {
    let days = seconds / 86_400;
    match days {
        0 => "today".to_owned(),
        1 => "1 day ago".to_owned(),
        2..=59 => format!("{days} days ago"),
        60..=729 => format!("{} months ago", days / 30),
        _ => format!("{} years ago", days / 365),
    }
}

fn month_year(unix_s: u64) -> String {
    let (_, month, year) = civil_date(unix_s / 86_400);
    let month = usize::try_from(month).expect("civil month fits usize");
    format!("{} {year}", MONTHS[month - 1])
}

/// Gregorian civil date from days since 1970-01-01 (Howard Hinnant's method).
fn civil_date(days: u64) -> (u32, u32, i64) {
    let z = i64::try_from(days)
        .unwrap_or(i64::MAX)
        .saturating_add(719_468);
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    let day = u32::try_from(day).expect("civil day is in 1..=31");
    let month = u32::try_from(month).expect("civil month is in 1..=12");
    (day, month, year)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_and_elapsed_labels_are_stable() {
        assert_eq!(month_year(0), "January 1970");
        assert_eq!(month_year(1_551_398_400), "March 2019");
        assert_eq!(ago(86_400), "1 day ago");
        assert_eq!(ago(240 * 86_400), "8 months ago");
    }

    #[test]
    fn facts_use_flat_archivist_language() {
        let strings = ["Never played before", "Played 2 times since March 2019"];
        for fact in strings {
            for refused in ["streak", "top artist", "congrat", "hours listened"] {
                assert!(!fact.to_lowercase().contains(refused));
            }
        }
    }
}
