//! **How long listening to a real library actually takes** — plan 22 item 0.4.
//!
//! Design 21 §10 admits that nobody has measured a per-track analysis rate on
//! a real library, and design 21 §7's first-run copy quotes one: *9 412 tracks
//! · roughly two hours*. That sentence cannot ship until this number exists,
//! because a wrong estimate on the one screen a new listener cannot skip is
//! worse than no estimate at all.
//!
//! It runs the shipping analysis path — decode, bliss features, CLAP audio
//! embedding, store — at the shipping worker count, over tracks drawn from a
//! real library database, into a throwaway store. It reports wall-clock,
//! tracks per hour, and the spread of per-track times, because a mean alone
//! would hide the long tail that decides whether a progress reading can be
//! trusted.

use std::error::Error;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Instant;

use rusqlite::Connection;

fn main() {
    if let Err(error) = run() {
        eprintln!("vibe rate: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let arguments: Vec<_> = std::env::args_os().collect();
    if arguments.len() != 5 {
        return Err("usage: vibe-rate LIBRARY STORE COUNT WORKERS".into());
    }
    let library = Path::new(&arguments[1]);
    let store = PathBuf::from(&arguments[2]);
    let count: usize = arguments[3].to_string_lossy().parse()?;
    let workers: usize = arguments[4].to_string_lossy().parse()?;

    let paths = read_paths(library, count)?;
    eprintln!(
        "measuring {} tracks at {workers} workers into {}",
        paths.len(),
        store.display()
    );

    let queue = Mutex::new(paths.into_iter());
    let timings: Mutex<Vec<f64>> = Mutex::new(Vec::new());
    let failures = Mutex::new(0_usize);
    let started = Instant::now();
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                loop {
                    let Some(path) = queue.lock().expect("queue").next() else {
                        break;
                    };
                    let one = Instant::now();
                    match baz_vibe::analyze_and_store(&store, path.clone()) {
                        Ok(_) => timings
                            .lock()
                            .expect("timings")
                            .push(one.elapsed().as_secs_f64()),
                        Err(error) => {
                            eprintln!("skipped {}: {error}", path.display());
                            *failures.lock().expect("failures") += 1;
                        }
                    }
                }
            });
        }
    });
    let elapsed = started.elapsed().as_secs_f64();

    let mut timings = timings.into_inner().expect("timings");
    timings.sort_by(f64::total_cmp);
    let failures = failures.into_inner().expect("failures");
    if timings.is_empty() {
        return Err("every track failed to analyse".into());
    }
    #[expect(clippy::cast_precision_loss, reason = "bounded track counts")]
    let done = timings.len() as f64;
    let at = |quantile: f64| {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a bounded quantile of a bounded count"
        )]
        let index = ((done - 1.0) * quantile) as usize;
        timings[index.min(timings.len() - 1)]
    };
    println!("analysed        {} tracks ({failures} skipped)", timings.len());
    println!("workers         {workers}");
    println!("wall clock      {elapsed:.1} s");
    println!("tracks / hour   {:.0}", done * 3_600.0 / elapsed);
    println!("seconds / track {:.2} wall, {:.2} cpu-side median", elapsed / done, at(0.5));
    println!("per-track p10   {:.2} s", at(0.10));
    println!("per-track p90   {:.2} s", at(0.90));
    println!("per-track max   {:.2} s", timings[timings.len() - 1]);
    Ok(())
}

/// Paths from a library database, longest-established first so the sample is
/// ordinary music rather than whatever was imported last.
fn read_paths(library: &Path, count: usize) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let connection = Connection::open(library)?;
    let mut statement = connection.prepare("SELECT path FROM tracks ORDER BY id LIMIT ?1")?;
    let rows = statement.query_map([i64::try_from(count).unwrap_or(i64::MAX)], |row| {
        row.get::<_, Vec<u8>>(0)
    })?;
    let mut paths = Vec::new();
    for row in rows {
        paths.push(bytes_to_path(&row?));
    }
    Ok(paths)
}

#[cfg(unix)]
fn bytes_to_path(bytes: &[u8]) -> PathBuf {
    use std::os::unix::ffi::OsStrExt;
    PathBuf::from(std::ffi::OsStr::from_bytes(bytes))
}

#[cfg(not(unix))]
fn bytes_to_path(bytes: &[u8]) -> PathBuf {
    PathBuf::from(String::from_utf8_lossy(bytes).into_owned())
}
