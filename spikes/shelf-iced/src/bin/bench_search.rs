//! Headless benchmark of the search filter over the 100k-track index.
//! Scripted query sequence, N iterations each, p50/p95/p99 per query.

use shelf_iced::{dataset_dir, Index};
use std::time::Instant;

const QUERIES: &[&str] = &[
    "a",
    "ar",
    "art",
    "artist 1",
    "artist 19",
    "track 07",
    "größenwahn",
];
const WARMUP: usize = 5;
const ITERS: usize = 200;

fn percentile(sorted: &[f64], p: f64) -> f64 {
    let idx = ((sorted.len() as f64 - 1.0) * p / 100.0).round() as usize;
    sorted[idx]
}

fn main() -> std::io::Result<()> {
    let jsonl = dataset_dir().join("albums.jsonl");
    let index = Index::load(&jsonl)?;
    println!(
        "loaded {} albums ({} tracks): jsonl load {:.1} ms, index build {:.1} ms",
        index.albums.len(),
        index.albums.len() * 10,
        index.load_time_ms,
        index.index_time_ms
    );
    println!("iterations per query: {ITERS} (after {WARMUP} warmup)\n");
    println!(
        "{:<14} {:>8} {:>10} {:>10} {:>10}",
        "query", "matches", "p50 (ms)", "p95 (ms)", "p99 (ms)"
    );

    for q in QUERIES {
        for _ in 0..WARMUP {
            std::hint::black_box(index.filter(q));
        }
        let mut times = Vec::with_capacity(ITERS);
        let mut matches = 0;
        for _ in 0..ITERS {
            let t = Instant::now();
            let res = std::hint::black_box(index.filter(q));
            times.push(t.elapsed().as_secs_f64() * 1e3);
            matches = res.len();
        }
        times.sort_by(|a, b| a.partial_cmp(b).unwrap());
        println!(
            "{:<14} {:>8} {:>10.3} {:>10.3} {:>10.3}",
            q,
            matches,
            percentile(&times, 50.0),
            percentile(&times, 95.0),
            percentile(&times, 99.0)
        );
    }
    Ok(())
}
