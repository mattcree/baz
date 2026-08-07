//! Deterministic dataset generator (seed 42). Writes:
//!   dataset/albums.jsonl      — 10,000 albums / 100,000 tracks
//!   dataset/art/{id}.png      — 512x512 two-color diagonal gradient per album
//!
//! Usage: cargo run --release --features gen --bin gen-dataset [-- <out_dir>]
//! (out_dir defaults to "dataset" relative to cwd)

use image::{ImageBuffer, Rgb};
use rayon::prelude::*;
use shelf_index::spec;
use std::io::Write;
use std::path::PathBuf;

const ART_SIZE: u32 = 512;

fn main() {
    let out: PathBuf = std::env::args().nth(1).unwrap_or_else(|| "dataset".into()).into();
    let art_dir = out.join("art");
    std::fs::create_dir_all(&art_dir).expect("create art dir");

    let t0 = std::time::Instant::now();

    // albums.jsonl — one serde_json object per line, field order id/title/artist/year/tracks.
    let albums: Vec<_> = (1..=spec::ALBUM_COUNT).map(spec::album).collect();
    {
        let f = std::fs::File::create(out.join("albums.jsonl")).expect("create albums.jsonl");
        let mut w = std::io::BufWriter::new(f);
        for a in &albums {
            serde_json::to_writer(&mut w, a).expect("serialize album");
            w.write_all(b"\n").expect("write newline");
        }
        w.flush().expect("flush albums.jsonl");
    }
    let t_jsonl = t0.elapsed();

    // Art: 512x512 diagonal gradient, colors from FNV-1a of the album id.
    let t1 = std::time::Instant::now();
    albums.par_iter().for_each(|a| {
        let (c1, c2) = spec::art_colors(&a.id);
        let denom = (2 * (ART_SIZE - 1)) as f32;
        let img = ImageBuffer::from_fn(ART_SIZE, ART_SIZE, |x, y| {
            let t = (x + y) as f32 / denom;
            Rgb([
                lerp(c1[0], c2[0], t),
                lerp(c1[1], c2[1], t),
                lerp(c1[2], c2[2], t),
            ])
        });
        img.save(art_dir.join(format!("{}.png", a.id))).expect("write png");
    });

    let tracks: usize = albums.iter().map(|a| a.tracks.len()).sum();
    println!(
        "wrote {} albums / {} tracks to {} (jsonl {:.1?}, art {:.1?})",
        albums.len(),
        tracks,
        out.display(),
        t_jsonl,
        t1.elapsed()
    );
}

fn lerp(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t).round() as u8
}
