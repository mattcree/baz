//! Writes placeholder app icons for src-tauri (Tauri wants them at build time).
//! Usage: cargo run --release --features gen --bin gen-icons [-- <icons_dir>]

use image::{ImageBuffer, Rgb};
use std::path::PathBuf;

fn main() {
    let dir: PathBuf = std::env::args().nth(1).unwrap_or_else(|| "src-tauri/icons".into()).into();
    std::fs::create_dir_all(&dir).expect("create icons dir");
    for (name, size) in [("32x32.png", 32u32), ("128x128.png", 128), ("128x128@2x.png", 256), ("icon.png", 512)] {
        let denom = (2 * (size - 1)) as f32;
        let img = ImageBuffer::from_fn(size, size, |x, y| {
            let t = (x + y) as f32 / denom;
            Rgb([
                (20.0 + 80.0 * t) as u8,
                (40.0 + 160.0 * t) as u8,
                (120.0 + 100.0 * t) as u8,
            ])
        });
        img.save(dir.join(name)).expect("write icon");
    }
    println!("icons written to {}", dir.display());
}
