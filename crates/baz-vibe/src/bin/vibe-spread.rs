//! **How far a real library actually spreads on each dimension** — the
//! measurement behind `crate::vibe::FLAT_AXIS` in the shell.
//!
//! Design note 24 §4 asks for one flag and refuses another. It refuses *"most
//! varied in texture, least in brightness"*, because those are raw bliss
//! features in different units and comparing them is arithmetic without
//! meaning. It asks for the one-sided flag: say when *this* collection barely
//! varies on a dimension, so a listener is not handed a line that the dots
//! will follow perfectly while nothing about the music changes.
//!
//! That flag needs a number, and a number invented at a desk would be the
//! same overclaiming design note 23 spends its length on. So: run this
//! against a real store, read the p05–p95 span each dimension actually has,
//! and set the threshold below the narrowest of them — because a false *"this
//! line will not do much"* is worse than a missing one. It would talk
//! somebody out of a control that works.
//!
//! ```text
//! vibe-spread ~/.local/share/baz/vibe.db
//! ```
//!
//! Reads only. It opens the store read-only and writes nothing anywhere.

use std::error::Error;
use std::path::Path;

use baz_vibe::{Dimension, Features};
use rusqlite::{Connection, OpenFlags};

fn main() {
    if let Err(error) = run() {
        eprintln!("vibe spread: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let arguments: Vec<_> = std::env::args_os().collect();
    if arguments.len() != 2 {
        return Err("usage: vibe-spread STORE".into());
    }
    let store = Path::new(&arguments[1]);
    let tracks = read(store)?;
    println!("{} tracks in {}", tracks.len(), store.display());
    println!();
    println!(
        "{:<12} {:>8} {:>8} {:>8} {:>8}",
        "", "p05", "p50", "p95", "span"
    );

    let mut narrowest = f32::MAX;
    for dimension in [
        Dimension::Energy,
        Dimension::Tempo,
        Dimension::Brightness,
        Dimension::Dynamics,
        Dimension::Texture,
    ] {
        let mut values: Vec<f32> = tracks
            .iter()
            .map(|features| features.value(dimension))
            .collect();
        values.sort_by(f32::total_cmp);
        let at = |percent: usize| values[(values.len() - 1) * percent / 100];
        let (low, middle, high) = (at(5), at(50), at(95));
        let span = high - low;
        narrowest = narrowest.min(span);
        println!(
            "{:<12} {low:>8.3} {middle:>8.3} {high:>8.3} {span:>8.3}",
            format!("{dimension:?}")
        );
    }
    println!();
    println!("narrowest span: {narrowest:.3}");
    println!("a threshold below this flags nothing in this library, which is the point");
    Ok(())
}

fn read(store: &Path) -> Result<Vec<Features>, Box<dyn Error>> {
    let connection = Connection::open_with_flags(store, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let mut statement = connection.prepare("SELECT values_blob FROM features")?;
    let rows = statement.query_map([], |row| row.get::<_, Vec<u8>>(0))?;
    let mut tracks = Vec::new();
    for row in rows {
        let values: Vec<f32> = row?
            .chunks_exact(4)
            .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect();
        if values.len() < 12 {
            continue;
        }
        tracks.push(Features::from_values(values, Vec::new()));
    }
    if tracks.is_empty() {
        return Err("the store holds no feature vectors".into());
    }
    Ok(tracks)
}
