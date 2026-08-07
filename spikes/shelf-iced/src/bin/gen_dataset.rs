//! Deterministic dataset generator (seed 42).
//! Produces dataset/albums.jsonl (10k albums / 100k tracks) and
//! dataset/art/{album_id}.png (512x512 two-color diagonal gradients).

use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::{ExtendedColorType, ImageEncoder};
use rayon::prelude::*;
use shelf_iced::{art_colors, dataset_dir, make_album, NUM_ALBUMS};
use std::io::Write;
use std::time::Instant;

const ART_SIZE: u32 = 512;

fn main() -> std::io::Result<()> {
    let out = dataset_dir();
    let art_dir = out.join("art");
    std::fs::create_dir_all(&art_dir)?;

    // --- albums.jsonl -----------------------------------------------------
    let t0 = Instant::now();
    let albums: Vec<_> = (1..=NUM_ALBUMS).map(make_album).collect();
    let mut file = std::io::BufWriter::with_capacity(
        1 << 20,
        std::fs::File::create(out.join("albums.jsonl"))?,
    );
    for album in &albums {
        serde_json::to_writer(&mut file, album)?;
        file.write_all(b"\n")?;
    }
    file.flush()?;
    let jsonl_time = t0.elapsed();
    println!(
        "albums.jsonl: {} albums / {} tracks in {:.2}s",
        albums.len(),
        albums.len() * 10,
        jsonl_time.as_secs_f64()
    );

    // --- art PNGs ---------------------------------------------------------
    let t1 = Instant::now();
    let errors: Vec<String> = (1..=NUM_ALBUMS)
        .into_par_iter()
        .filter_map(|id| {
            let (c1, c2) = art_colors(id);
            let mut buf = vec![0u8; (ART_SIZE * ART_SIZE * 3) as usize];
            let denom = (2 * (ART_SIZE - 1)) as f32;
            for y in 0..ART_SIZE {
                for x in 0..ART_SIZE {
                    let t = (x + y) as f32 / denom; // diagonal 0..1
                    let px = ((y * ART_SIZE + x) * 3) as usize;
                    for ch in 0..3 {
                        buf[px + ch] =
                            (c1[ch] as f32 + (c2[ch] as f32 - c1[ch] as f32) * t).round() as u8;
                    }
                }
            }
            let path = art_dir.join(format!("{id}.png"));
            let write = || -> Result<(), Box<dyn std::error::Error>> {
                let file = std::io::BufWriter::new(std::fs::File::create(&path)?);
                let enc = PngEncoder::new_with_quality(file, CompressionType::Fast, FilterType::Sub);
                enc.write_image(&buf, ART_SIZE, ART_SIZE, ExtendedColorType::Rgb8)?;
                Ok(())
            };
            write().err().map(|e| format!("{}: {e}", path.display()))
        })
        .collect();
    if !errors.is_empty() {
        eprintln!("{} art errors, first: {}", errors.len(), errors[0]);
        std::process::exit(1);
    }
    println!(
        "art: {} x {}x{} PNGs in {:.2}s",
        NUM_ALBUMS,
        ART_SIZE,
        ART_SIZE,
        t1.elapsed().as_secs_f64()
    );
    println!("total: {:.2}s -> {}", t0.elapsed().as_secs_f64(), out.display());
    Ok(())
}
