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
//! Given further arguments it also **embeds each of them and reports what the
//! eligibility policy keeps**, which answers a different question that came
//! up while building design note 24 §7 item 2 — *can a mood say when this
//! library cannot answer it?* The pool size is the only candidate signal, and
//! [`baz_vibe::eligible_count`] is floored at `KNEE_FLOOR`, so this is how
//! you find out whether an unanswerable request is distinguishable from an
//! answerable one at all.
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
    if arguments.len() < 2 {
        return Err("usage: vibe-spread STORE [REQUEST...]".into());
    }
    let store = Path::new(&arguments[1]);
    let (tracks, semantics) = read(store)?;
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

    if arguments.len() > 2 {
        println!();
        println!("{:<40} {:>8} {:>8}", "request", "pool", "top cos");
        for request in arguments.iter().skip(2) {
            let words = request.to_string_lossy();
            let embedding = baz_vibe::embed_request(&words)?;
            let mut ranked: Vec<f32> = semantics
                .iter()
                .map(|features| features.similarity(&embedding))
                .collect();
            ranked.sort_by(|left, right| right.total_cmp(left));
            let pool = baz_vibe::eligible_count(&ranked);
            println!("{words:<40} {pool:>8} {:>8.3}", ranked[0]);
        }
    }
    Ok(())
}

/// Every track twice: once for the drawn dimensions, and — where the store
/// holds one — once carrying its semantic vector, which is what a request is
/// scored against.
fn read(store: &Path) -> Result<(Vec<Features>, Vec<Features>), Box<dyn Error>> {
    let connection = Connection::open_with_flags(store, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let mut statement = connection.prepare("SELECT values_blob, semantic_blob FROM features")?;
    let rows = statement.query_map([], |row| {
        Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, Option<Vec<u8>>>(1)?))
    })?;
    let floats = |blob: &[u8]| -> Vec<f32> {
        blob.chunks_exact(4)
            .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect()
    };
    let (mut tracks, mut semantics) = (Vec::new(), Vec::new());
    for row in rows {
        let (values, semantic) = row?;
        let values = floats(&values);
        if values.len() < 12 {
            continue;
        }
        if let Some(semantic) = semantic {
            let semantic = floats(&semantic);
            if !semantic.is_empty() {
                semantics.push(Features::from_values(values.clone(), semantic));
            }
        }
        tracks.push(Features::from_values(values, Vec::new()));
    }
    if tracks.is_empty() {
        return Err("the store holds no feature vectors".into());
    }
    Ok((tracks, semantics))
}
