//! Standalone benchmark of the Rust search index over the generated dataset.
//! Runs today with zero system deps (no gtk/webkit needed).
//!
//! Usage: cargo run --release --bin bench [-- <path/to/albums.jsonl>]

use shelf_index::Index;
use std::path::PathBuf;
use std::time::Instant;

const QUERIES: &[&str] = &["a", "ar", "art", "artist 1", "artist 19", "track 07", "größenwahn"];
const WARMUP: usize = 25;
const ITERS: usize = 300;
const WINDOW: usize = 60; // typical visible grid window the UI would request

fn main() {
    let path: PathBuf = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "dataset/albums.jsonl".into())
        .into();

    let t0 = Instant::now();
    let idx = Index::from_jsonl_path(&path).unwrap_or_else(|e| {
        eprintln!("failed to load {}: {e}\n(run gen-dataset first)", path.display());
        std::process::exit(1);
    });
    let load = t0.elapsed();
    println!(
        "loaded+indexed {} albums / {} tracks from {} in {:.1?}\n",
        idx.album_count(),
        idx.track_count(),
        path.display(),
        load
    );

    println!(
        "{:<14} {:>8} {:>10} {:>10} {:>10} {:>10}",
        "query", "matches", "p50", "p95", "p99", "max"
    );
    for q in QUERIES {
        for _ in 0..WARMUP {
            std::hint::black_box(idx.search(q, 0, WINDOW));
        }
        let mut samples = Vec::with_capacity(ITERS);
        let mut total = 0usize;
        for _ in 0..ITERS {
            let t = Instant::now();
            let w = std::hint::black_box(idx.search(q, 0, WINDOW));
            samples.push(t.elapsed().as_nanos() as u64);
            total = w.total;
        }
        samples.sort_unstable();
        println!(
            "{:<14} {:>8} {:>10} {:>10} {:>10} {:>10}",
            format!("{q:?}"),
            total,
            fmt_ns(pct(&samples, 50.0)),
            fmt_ns(pct(&samples, 95.0)),
            fmt_ns(pct(&samples, 99.0)),
            fmt_ns(*samples.last().unwrap()),
        );
    }
}

fn pct(sorted: &[u64], p: f64) -> u64 {
    let idx = ((p / 100.0) * (sorted.len() - 1) as f64).round() as usize;
    sorted[idx]
}

fn fmt_ns(ns: u64) -> String {
    if ns >= 1_000_000 {
        format!("{:.2}ms", ns as f64 / 1e6)
    } else {
        format!("{:.1}µs", ns as f64 / 1e3)
    }
}
