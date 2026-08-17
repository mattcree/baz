//! **Is the model of the songs sound?** — `docs/WORK.md` item 79's first
//! step, and the one that has to come before any fix.
//!
//! The owner: *"the most important is that our model of the songs that is
//! used in the smart playlists needs to be rock solid."* Every reading on the
//! composing page and every line on *what Baz heard* is a claim about the
//! per-track features. If those are shaky the page is confidently wrong
//! rather than merely limited, which design note 23 §4 names as the worst
//! thing this feature can be.
//!
//! So: one offline pass over a real store, reporting what can be checked
//! without ears.
//!
//! 1. **Per feature** — minimum, p05, median, p95, maximum, and the three
//!    counts that mean something has gone wrong rather than something is
//!    unusual: exact zeros, non-finite values, and values pinned at the
//!    normaliser's ±1 boundary. A feature that saturates is a feature whose
//!    range was chosen for other music.
//! 2. **The tempo histogram**, in BPM. A healthy 5 000-track library should
//!    be roughly unimodal somewhere around 100–130. A second hump up near 180
//!    is the signature of **octave errors** — beat trackers reporting double
//!    the felt tempo — which is what the owner caught by ear when *what Baz
//!    heard* named a Renaissance madrigal as one of his fastest records.
//! 3. **Duplicate vectors**, because two different files with byte-identical
//!    features means the analyser returned something generic rather than
//!    something about the music.
//!
//! ```text
//! vibe-audit ~/.local/share/baz/vibe.db [~/.local/share/baz/library.db]
//! ```
//!
//! **Give it the library too.** `baz_vibe::prepare` walks the *library's*
//! paths and pulls a stored vector for each, so a row whose file the library
//! no longer holds is inert — it is in the store and it is not in the model.
//! Auditing the store alone therefore overstates the problem, sometimes
//! badly: the owner's store carries 206 rows from a test fixture of digital
//! silence that has not existed since it was analysed, and every one of them
//! reads as the quietest, slowest, darkest thing ever recorded.
//!
//! Reads only. It opens the store read-only and writes nothing anywhere.

use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::path::Path;

use rusqlite::{Connection, OpenFlags};

/// The store's feature vectors, and how many rows the library no longer holds
/// (`None` when no library was given to check against).
type Audited = (Vec<Vec<f32>>, Option<usize>);

/// bliss' own feature order, as far as the analysis names it. Past these the
/// vector is chroma, which nothing in baz reads and which is reported by
/// index alone.
const NAMED: [&str; 10] = [
    "tempo",
    "zero crossings",
    "centroid mean",
    "centroid sd",
    "rolloff mean",
    "rolloff sd",
    "flatness mean",
    "flatness sd",
    "loudness mean",
    "loudness sd",
];

/// bliss normalises to this range, so a value sitting exactly on an end is a
/// clamp rather than a reading.
const LIMIT: f32 = 1.0;
/// How near an end counts as pinned to it.
const PINNED: f32 = 0.001;

fn main() {
    if let Err(error) = run() {
        eprintln!("vibe audit: {error}");
        std::process::exit(1);
    }
}

#[expect(
    clippy::cast_precision_loss,
    clippy::too_many_lines,
    reason = "library counts printed as shares; three reports that read as one pass"
)]
fn run() -> Result<(), Box<dyn Error>> {
    let arguments: Vec<_> = std::env::args_os().collect();
    if !(2..=3).contains(&arguments.len()) {
        return Err("usage: vibe-audit STORE [LIBRARY]".into());
    }
    let store = Path::new(&arguments[1]);
    let held = arguments
        .get(2)
        .map(|path| library(Path::new(path)))
        .transpose()?;
    let (tracks, skipped) = read(store, held.as_ref())?;
    let count = tracks.len();
    if let Some(skipped) = skipped {
        println!(
            "{skipped} stored vectors ignored: the library no longer holds those files, \
             so nothing loads them"
        );
    }
    let width = tracks.iter().map(Vec::len).max().unwrap_or(0);
    println!(
        "{count} tracks, {width} features each, from {}",
        store.display()
    );

    // ── 1. Per feature ──────────────────────────────────────────────────
    println!();
    println!(
        "{:<16} {:>7} {:>7} {:>7} {:>7} {:>7} {:>6} {:>6} {:>6}",
        "feature", "min", "p05", "p50", "p95", "max", "zero", "pinned", "bad"
    );
    for index in 0..width {
        let mut values: Vec<f32> = Vec::with_capacity(count);
        let (mut zeros, mut pinned, mut bad) = (0_usize, 0_usize, 0_usize);
        for track in &tracks {
            let Some(value) = track.get(index).copied() else {
                continue;
            };
            if !value.is_finite() {
                bad += 1;
                continue;
            }
            if value == 0.0 {
                zeros += 1;
            }
            if (value.abs() - LIMIT).abs() < PINNED {
                pinned += 1;
            }
            values.push(value);
        }
        if values.is_empty() {
            continue;
        }
        values.sort_by(f32::total_cmp);
        let at = |percent: usize| values[(values.len() - 1) * percent / 100];
        let name = NAMED.get(index).map_or_else(
            || format!("chroma {}", index - NAMED.len() + 1),
            |named| (*named).to_owned(),
        );
        println!(
            "{name:<16} {:>7.3} {:>7.3} {:>7.3} {:>7.3} {:>7.3} {zeros:>6} {pinned:>6} {bad:>6}",
            values[0],
            at(5),
            at(50),
            at(95),
            values[values.len() - 1],
        );
    }

    // ── 2. The tempo histogram, in the unit a listener reads ────────────
    println!();
    println!("tempo, in BPM — a healthy library is one hump around 100–130");
    let mut buckets = [0_usize; 12];
    let mut zero_tempo = 0_usize;
    for track in &tracks {
        let Some(raw) = track.first().copied().filter(|value| value.is_finite()) else {
            continue;
        };
        let bpm = (raw + 1.0) * 103.0;
        if bpm < 1.0 {
            zero_tempo += 1;
            continue;
        }
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a bounded BPM into one of twelve buckets"
        )]
        let bucket = ((bpm / 20.0) as usize).min(buckets.len() - 1);
        buckets[bucket] += 1;
    }
    for (bucket, held) in buckets.iter().enumerate() {
        let share = *held as f32 / count as f32;
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a share of a library, as a bar of hashes"
        )]
        let bar = "#".repeat((share * 200.0).round() as usize);
        println!(
            "{:>4}–{:<4} {held:>6}  {bar}",
            bucket * 20,
            bucket * 20 + 20
        );
    }
    println!("{:>9} {zero_tempo:>6}  (no tempo found at all)", "0");

    // ── 3. Vectors that are not about the music ─────────────────────────
    let mut seen: HashMap<Vec<u32>, usize> = HashMap::new();
    for track in &tracks {
        *seen
            .entry(track.iter().map(|v| v.to_bits()).collect())
            .or_default() += 1;
    }
    let repeated: usize = seen.values().filter(|held| **held > 1).sum();
    println!();
    println!(
        "{repeated} tracks share a vector with another ({} distinct vectors)",
        seen.len()
    );
    Ok(())
}

/// Every path the library holds, as the store spells them.
fn library(path: &Path) -> Result<HashSet<Vec<u8>>, Box<dyn Error>> {
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let mut statement = connection.prepare("SELECT path FROM tracks")?;
    let rows = statement.query_map([], |row| row.get::<_, Vec<u8>>(0))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

fn read(store: &Path, held: Option<&HashSet<Vec<u8>>>) -> Result<Audited, Box<dyn Error>> {
    let connection = Connection::open_with_flags(store, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let mut statement = connection.prepare("SELECT path, values_blob FROM features")?;
    let rows = statement.query_map([], |row| {
        Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, Vec<u8>>(1)?))
    })?;
    let (mut tracks, mut skipped) = (Vec::new(), 0_usize);
    for row in rows {
        let (path, blob) = row?;
        if held.is_some_and(|held| !held.contains(&path)) {
            skipped += 1;
            continue;
        }
        let values: Vec<f32> = blob
            .chunks_exact(4)
            .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect();
        if values.is_empty() {
            continue;
        }
        tracks.push(values);
    }
    if tracks.is_empty() {
        return Err("the store holds no feature vectors for this library".into());
    }
    Ok((tracks, held.map(|_| skipped)))
}
