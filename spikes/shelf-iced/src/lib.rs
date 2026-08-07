//! Shared code for the Spike A (iced) shelf demo: dataset model, deterministic
//! generation helpers, loading/hydration, and the search index.

use serde::{Deserialize, Serialize};
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::time::Instant;

pub const SEED: u64 = 42;
pub const NUM_ALBUMS: u32 = 10_000;
pub const NUM_ARTISTS: u32 = 2_000;
pub const TRACKS_PER_ALBUM: u32 = 10;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Album {
    pub id: u32,
    pub title: String,
    pub artist: String,
    pub year: u16,
    pub tracks: Vec<String>,
}

/// Root of the spike crate (dataset lives at `<root>/dataset`).
pub fn dataset_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("dataset")
}

// ---------------------------------------------------------------------------
// Deterministic generation (seed 42) — matches the shared spike spec.
// ---------------------------------------------------------------------------

/// splitmix64 — a tiny, well-known, language-portable PRNG step.
pub fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Deterministic per-album random stream: seed 42 mixed with album id + salt.
pub fn rng(album_id: u32, salt: u64) -> u64 {
    splitmix64(SEED ^ (album_id as u64).wrapping_mul(0x0100_0000_01B3) ^ salt.wrapping_mul(0x9E37))
}

/// Generate album `i` (1-based) per the spec.
pub fn make_album(i: u32) -> Album {
    let artist_no = ((i - 1) % NUM_ARTISTS) + 1; // round-robin
    let title = if i.is_multiple_of(100) {
        format!("Ålbum № {i} — Ethereal Größenwahn: Живопись")
    } else {
        format!("Album {i:05}")
    };
    let tracks = (1..=TRACKS_PER_ALBUM)
        .map(|t| format!("Track {t:02} of Album {i}"))
        .collect();
    Album {
        id: i,
        title,
        artist: format!("Artist {artist_no:04}"),
        year: 1960 + (rng(i, 0) % 66) as u16,
        tracks,
    }
}

/// Two deterministic RGB colors for the album's cover-art gradient (hash → HSL → RGB).
pub fn art_colors(album_id: u32) -> ([u8; 3], [u8; 3]) {
    let color = |salt: u64| -> [u8; 3] {
        let v = rng(album_id, salt);
        let h = (v % 360) as f32;
        let s = 0.45 + ((v >> 16) % 45) as f32 / 100.0;
        let l = 0.30 + ((v >> 32) % 40) as f32 / 100.0;
        hsl_to_rgb(h, s, l)
    };
    (color(1), color(2))
}

pub fn hsl_to_rgb(h: f32, s: f32, l: f32) -> [u8; 3] {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = h / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r1, g1, b1) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    [
        ((r1 + m) * 255.0).round() as u8,
        ((g1 + m) * 255.0).round() as u8,
        ((b1 + m) * 255.0).round() as u8,
    ]
}

// ---------------------------------------------------------------------------
// Loading / search index
// ---------------------------------------------------------------------------

/// The in-memory search index: one case-folded blob per album covering
/// artist + album title + all 10 track titles.
pub struct Index {
    pub albums: Vec<Album>,
    pub blobs: Vec<String>,
    pub load_time_ms: f64,
    pub index_time_ms: f64,
}

impl Index {
    /// Load albums.jsonl and build the case-folded index. Prints nothing;
    /// callers report `load_time_ms` / `index_time_ms`.
    pub fn load(jsonl_path: &Path) -> std::io::Result<Index> {
        let t0 = Instant::now();
        let file = std::fs::File::open(jsonl_path)?;
        let reader = std::io::BufReader::with_capacity(1 << 20, file);
        let mut albums = Vec::with_capacity(NUM_ALBUMS as usize);
        for line in reader.lines() {
            let line = line?;
            if line.is_empty() {
                continue;
            }
            let album: Album = serde_json::from_str(&line)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
            albums.push(album);
        }
        let load_time_ms = t0.elapsed().as_secs_f64() * 1e3;

        let t1 = Instant::now();
        let blobs = albums
            .iter()
            .map(|a| {
                let mut blob =
                    String::with_capacity(a.artist.len() + a.title.len() + 30 * a.tracks.len());
                blob.push_str(&a.artist.to_lowercase());
                blob.push('\n');
                blob.push_str(&a.title.to_lowercase());
                for t in &a.tracks {
                    blob.push('\n');
                    blob.push_str(&t.to_lowercase());
                }
                blob
            })
            .collect();
        let index_time_ms = t1.elapsed().as_secs_f64() * 1e3;

        Ok(Index {
            albums,
            blobs,
            load_time_ms,
            index_time_ms,
        })
    }

    /// Case-folded substring filter. Returns indices (into `albums`) of matches.
    /// An empty query matches everything.
    pub fn filter(&self, query: &str) -> Vec<u32> {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return (0..self.albums.len() as u32).collect();
        }
        self.blobs
            .iter()
            .enumerate()
            .filter(|(_, blob)| blob.contains(&q))
            .map(|(i, _)| i as u32)
            .collect()
    }
}

/// VmRSS in MiB from /proc/self/status (Linux only).
pub fn rss_mib() -> Option<f64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|l| l.starts_with("VmRSS:"))?;
    let kb: f64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kb / 1024.0)
}
