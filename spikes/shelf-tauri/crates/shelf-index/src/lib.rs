//! shelf-index: the search index for baz Spike A (Tauri flavor).
//!
//! Plain Rust, zero system deps. The Tauri shell (`src-tauri`) depends on this
//! crate; the `bench` bin exercises it standalone so index performance can be
//! measured today, before webkit2gtk/gtk devel packages are installed.

use serde::{Deserialize, Serialize};
use std::io::BufRead;
use std::path::Path;

/// One album as stored in `dataset/albums.jsonl` (field order matters for the
/// generated file, which must be byte-identical to the shared spec).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Album {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub year: u16,
    pub tracks: Vec<String>,
}

/// What the UI receives per visible album. Deliberately excludes `tracks`:
/// the IPC discipline of this spike is "never serialize the full library".
#[derive(Debug, Clone, Serialize)]
pub struct AlbumHit {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub year: u16,
}

/// A visible window of search results plus the total match count.
#[derive(Debug, Serialize)]
pub struct SearchWindow {
    pub total: usize,
    pub offset: usize,
    pub items: Vec<AlbumHit>,
    /// Time the Rust-side search took, in microseconds (lets the frontend
    /// separate index time from IPC overhead).
    pub index_us: u64,
}

pub struct Index {
    albums: Vec<Album>,
    /// Per-album lowercase haystack: "title\nartist\ntrack1\n...\ntrack10".
    hay: Vec<String>,
    track_count: usize,
}

impl Index {
    pub fn from_albums(albums: Vec<Album>) -> Self {
        let mut track_count = 0;
        let hay = albums
            .iter()
            .map(|a| {
                track_count += a.tracks.len();
                let mut h = String::with_capacity(
                    a.title.len() + a.artist.len() + a.tracks.iter().map(|t| t.len() + 1).sum::<usize>() + 2,
                );
                h.push_str(&a.title);
                h.push('\n');
                h.push_str(&a.artist);
                for t in &a.tracks {
                    h.push('\n');
                    h.push_str(t);
                }
                h.to_lowercase()
            })
            .collect();
        Self { albums, hay, track_count }
    }

    pub fn from_jsonl_path(path: &Path) -> std::io::Result<Self> {
        let file = std::fs::File::open(path)?;
        let reader = std::io::BufReader::new(file);
        let mut albums = Vec::with_capacity(10_000);
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let album: Album = serde_json::from_str(&line)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
            albums.push(album);
        }
        Ok(Self::from_albums(albums))
    }

    pub fn album_count(&self) -> usize {
        self.albums.len()
    }

    pub fn track_count(&self) -> usize {
        self.track_count
    }

    /// Case-insensitive substring search over title + artist + all track
    /// titles. Returns only the `[offset, offset+limit)` window of matches,
    /// never the full result set.
    pub fn search(&self, query: &str, offset: usize, limit: usize) -> SearchWindow {
        let t0 = std::time::Instant::now();
        let q = query.trim().to_lowercase();
        let mut total = 0usize;
        let mut items = Vec::with_capacity(limit.min(256));
        if q.is_empty() {
            total = self.albums.len();
            for a in self.albums.iter().skip(offset).take(limit) {
                items.push(hit(a));
            }
        } else {
            for (i, h) in self.hay.iter().enumerate() {
                if h.contains(&q) {
                    if total >= offset && items.len() < limit {
                        items.push(hit(&self.albums[i]));
                    }
                    total += 1;
                }
            }
        }
        SearchWindow {
            total,
            offset,
            items,
            index_us: t0.elapsed().as_micros() as u64,
        }
    }
}

fn hit(a: &Album) -> AlbumHit {
    AlbumHit {
        id: a.id.clone(),
        title: a.title.clone(),
        artist: a.artist.clone(),
        year: a.year,
    }
}

/// The deterministic dataset spec (seed 42). Shared by the generator bin and
/// tests; the competing iced spike generates from the identical spec.
pub mod spec {
    use super::Album;

    pub const SEED: u64 = 42;
    pub const ALBUM_COUNT: usize = 10_000;
    pub const ARTIST_COUNT: usize = 2_000;
    pub const TRACKS_PER_ALBUM: usize = 10;

    /// SplitMix64 finalizer — the only PRNG in the spec.
    pub fn splitmix64(x: u64) -> u64 {
        let mut z = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Year for album i (1-based): 1960..=2025, keyed on (SEED, i).
    pub fn year_for(i: usize) -> u16 {
        1960 + (splitmix64(SEED.wrapping_shl(32).wrapping_add(i as u64)) % 66) as u16
    }

    /// FNV-1a 64-bit — used to derive art colors from the album id.
    pub fn fnv1a64(bytes: &[u8]) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for &b in bytes {
            h ^= b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01B3);
        }
        h
    }

    /// The two gradient colors for an album id.
    pub fn art_colors(id: &str) -> ([u8; 3], [u8; 3]) {
        let h1 = fnv1a64(id.as_bytes());
        let h2 = fnv1a64(format!("{id}:b").as_bytes());
        (
            [(h1 >> 40) as u8, (h1 >> 24) as u8, (h1 >> 8) as u8],
            [(h2 >> 40) as u8, (h2 >> 24) as u8, (h2 >> 8) as u8],
        )
    }

    /// Album i, 1-based (1..=10_000).
    pub fn album(i: usize) -> Album {
        assert!((1..=ALBUM_COUNT).contains(&i));
        let id = format!("{i:05}");
        let title = if i % 100 == 0 {
            format!("Ålbum № {i} — Ethereal Größenwahn: Живопись")
        } else {
            format!("Album {i:05}")
        };
        let artist = format!("Artist {:04}", (i - 1) % ARTIST_COUNT + 1);
        let tracks = (1..=TRACKS_PER_ALBUM)
            .map(|t| format!("Track {t:02} of Album {i:05}"))
            .collect();
        Album { id, title, artist, year: year_for(i), tracks }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_is_deterministic_and_matches_shape() {
        let a1 = spec::album(1);
        assert_eq!(a1.id, "00001");
        assert_eq!(a1.title, "Album 00001");
        assert_eq!(a1.artist, "Artist 0001");
        assert_eq!(a1.tracks.len(), 10);
        assert_eq!(a1.tracks[0], "Track 01 of Album 00001");
        assert_eq!(a1.tracks[9], "Track 10 of Album 00001");

        let a100 = spec::album(100);
        assert!(a100.title.starts_with("Ålbum № 100 — Ethereal Größenwahn"));

        let a2001 = spec::album(2001);
        assert_eq!(a2001.artist, "Artist 0001"); // round-robin wraps

        // Deterministic across calls.
        assert_eq!(spec::album(1234).year, spec::album(1234).year);
        assert!((1960..=2025).contains(&spec::album(9999).year));
    }

    #[test]
    fn search_windows_and_unicode() {
        let albums: Vec<Album> = (1..=spec::ALBUM_COUNT).map(spec::album).collect();
        let idx = Index::from_albums(albums);
        assert_eq!(idx.album_count(), 10_000);
        assert_eq!(idx.track_count(), 100_000);

        // Unicode, case-insensitive: 100 long-title albums.
        let w = idx.search("größenwahn", 0, 50);
        assert_eq!(w.total, 100);
        assert_eq!(w.items.len(), 50);

        // Window discipline: never more than `limit` items.
        let w = idx.search("a", 0, 60);
        assert_eq!(w.total, 10_000);
        assert_eq!(w.items.len(), 60);

        // Track-level match: every album has exactly one "track 07".
        let w = idx.search("track 07", 0, 10);
        assert_eq!(w.total, 10_000);

        // Offset works.
        let w0 = idx.search("artist 19", 0, 5);
        let w5 = idx.search("artist 19", 5, 5);
        assert_ne!(w0.items[0].id, w5.items[0].id);
        assert_eq!(w0.total, w5.total);

        // Empty query = browse-all.
        let w = idx.search("", 9_990, 60);
        assert_eq!(w.total, 10_000);
        assert_eq!(w.items.len(), 10);
    }
}
