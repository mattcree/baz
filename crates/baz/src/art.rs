//! Album-art resolution and thumbnail decoding.
//!
//! Everything here is designed to run on a blocking worker (the shelf calls
//! [`load_thumb`] via `tokio::task::spawn_blocking`): file I/O, tag parsing
//! and image decoding never touch the UI thread.
//!
//! # Resolution order (tested in this module)
//!
//! 1. An **embedded picture** in the album's first track (front cover
//!    preferred, else the first picture the tag carries), read with `lofty`.
//! 2. A **cover file** in the album's directory: `cover.jpg`, `cover.jpeg`,
//!    `cover.png`, then `folder.jpg`, matched case-insensitively.
//!
//! JPEG and PNG are the decodable formats (the `image` crate features we
//! enable); art in rarer formats (webp, bmp) falls back to the deterministic
//! gradient placeholder rather than an error.
//!
//! # Memory budget
//!
//! Thumbnails are decoded to at most [`THUMB_PX`]² RGBA and cached in an LRU
//! keyed by album id. The spike's 800-entry cache reached 400–500 MiB RSS;
//! v0.1 budgets **~150 MiB** for decoded thumbnails, and the entry count is
//! derived from the real worst-case entry size rather than guessed:
//! 256 × 256 × 4 B = 256 KiB per thumbnail, so 150 MiB / 256 KiB = **600
//! entries** ([`THUMB_CACHE_ENTRIES`]). GPU-side copies held by `iced`'s
//! image cache are bounded by the same entry count.

use std::path::{Path, PathBuf};

use lofty::picture::PictureType;
use lofty::prelude::*;

/// Max thumbnail edge in pixels; decoded art is downscaled to fit.
pub const THUMB_PX: u32 = 256;

/// Thumbnail LRU capacity. Derivation (do not hand-tune without redoing it):
/// budget 150 MiB ÷ (256 × 256 px × 4 B/px = 256 KiB worst case) = 600.
pub const THUMB_CACHE_ENTRIES: usize = 600;

/// Where an album's art comes from, per the resolution order above.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArtSource {
    /// Picture bytes embedded in the first track's tags (already extracted —
    /// resolution and decoding happen on the same worker call, so the file
    /// is not parsed twice).
    Embedded(Vec<u8>),
    /// A cover image file sitting next to the music.
    File(PathBuf),
}

/// Cover-file names tried in the album directory, in priority order.
/// Matched case-insensitively (`Cover.JPG` counts).
const COVER_FILE_NAMES: [&str; 4] = ["cover.jpg", "cover.jpeg", "cover.png", "folder.jpg"];

/// Resolve the art source for an album given its first track's path.
/// `None` means "no art found" — the shelf renders the gradient placeholder.
#[must_use]
pub fn resolve(first_track: &Path) -> Option<ArtSource> {
    if let Some(bytes) = embedded_picture(first_track) {
        return Some(ArtSource::Embedded(bytes));
    }
    let dir = first_track.parent()?;
    cover_file(dir).map(ArtSource::File)
}

/// The cover file sitting beside `first_track`, if the album has one — step 2
/// of the resolution order on its own, with no tag parsing.
///
/// Separate from [`resolve`] because a *path* is useful where decoded bytes
/// are not: MPRIS's `mpris:artUrl` can only carry a URL, so a cover file that
/// genuinely exists is the only art baz can honestly advertise to the desktop
/// (see [`crate::mpris`]). One `read_dir` of the album folder, cheap enough
/// to run once per track change.
#[must_use]
pub fn cover_file_beside(first_track: &Path) -> Option<PathBuf> {
    cover_file(first_track.parent()?)
}

/// The embedded picture bytes of `track`, if its tags carry any.
/// Front-cover typed pictures win over other types.
fn embedded_picture(track: &Path) -> Option<Vec<u8>> {
    let file = lofty::read_from_path(track).ok()?;
    let tag = file.primary_tag().or_else(|| file.first_tag())?;
    let pictures = tag.pictures();
    let picture = pictures
        .iter()
        .find(|p| p.pic_type() == PictureType::CoverFront)
        .or_else(|| pictures.first())?;
    Some(picture.data().to_vec())
}

/// The best cover file in `dir` per [`COVER_FILE_NAMES`], case-insensitive.
fn cover_file(dir: &Path) -> Option<PathBuf> {
    let entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .map(|e| e.path())
        .collect();
    for candidate in COVER_FILE_NAMES {
        let found = entries.iter().find(|path| {
            path.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.eq_ignore_ascii_case(candidate))
        });
        if let Some(path) = found {
            return Some(path.clone());
        }
    }
    None
}

/// Resolve and decode an album's art into an RGBA thumbnail no larger than
/// [`THUMB_PX`] per edge: `(width, height, rgba_bytes)`. `None` when there
/// is no art or it cannot be decoded. Blocking; call off the UI thread.
#[must_use]
pub fn load_thumb(first_track: &Path) -> Option<(u32, u32, Vec<u8>)> {
    let image = match resolve(first_track)? {
        ArtSource::Embedded(bytes) => image::load_from_memory(&bytes).ok()?,
        ArtSource::File(path) => image::open(path).ok()?,
    };
    let thumb = image.thumbnail(THUMB_PX, THUMB_PX).into_rgba8();
    let (w, h) = thumb.dimensions();
    Some((w, h, thumb.into_raw()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use lofty::config::WriteOptions;
    use lofty::picture::{MimeType, Picture};
    use lofty::tag::{Tag, TagType};
    use std::io::Cursor;

    /// A tiny valid PNG (solid color, `w`×`h`) as encoded bytes.
    fn png_bytes(w: u32, h: u32) -> Vec<u8> {
        let img = image::RgbaImage::from_pixel(w, h, image::Rgba([200, 40, 40, 255]));
        let mut out = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut out, image::ImageFormat::Png)
            .expect("encode png");
        out.into_inner()
    }

    /// Write a minimal WAV at `path` (hound), optionally embedding a front
    /// cover picture via an `ID3v2` tag (lofty).
    fn write_wav(path: &Path, with_picture: Option<Vec<u8>>) {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 8_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(path, spec).expect("wav");
        for _ in 0..64 {
            writer.write_sample(0i16).expect("sample");
        }
        writer.finalize().expect("finalize");

        if let Some(bytes) = with_picture {
            let mut tag = Tag::new(TagType::Id3v2);
            tag.push_picture(Picture::new_unchecked(
                PictureType::CoverFront,
                Some(MimeType::Png),
                None,
                bytes,
            ));
            tag.save_to_path(path, WriteOptions::default())
                .expect("embed picture");
        }
    }

    #[test]
    fn embedded_picture_wins_over_cover_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let track = dir.path().join("01 song.wav");
        let png = png_bytes(4, 4);
        write_wav(&track, Some(png.clone()));
        std::fs::write(dir.path().join("cover.jpg"), b"decoy").expect("write");

        assert_eq!(resolve(&track), Some(ArtSource::Embedded(png)));
    }

    #[test]
    fn cover_file_fallback_respects_priority_order() {
        let dir = tempfile::tempdir().expect("tempdir");
        let track = dir.path().join("01 song.wav");
        write_wav(&track, None);

        // folder.jpg alone is found…
        std::fs::write(dir.path().join("folder.jpg"), b"x").expect("write");
        assert_eq!(
            resolve(&track),
            Some(ArtSource::File(dir.path().join("folder.jpg")))
        );
        // …but any cover.* beats it, and cover.jpg beats cover.png.
        std::fs::write(dir.path().join("cover.png"), b"x").expect("write");
        assert_eq!(
            resolve(&track),
            Some(ArtSource::File(dir.path().join("cover.png")))
        );
        std::fs::write(dir.path().join("cover.jpg"), b"x").expect("write");
        assert_eq!(
            resolve(&track),
            Some(ArtSource::File(dir.path().join("cover.jpg")))
        );
    }

    #[test]
    fn cover_file_match_is_case_insensitive() {
        let dir = tempfile::tempdir().expect("tempdir");
        let track = dir.path().join("01 song.wav");
        write_wav(&track, None);
        std::fs::write(dir.path().join("Cover.JPG"), b"x").expect("write");
        assert_eq!(
            resolve(&track),
            Some(ArtSource::File(dir.path().join("Cover.JPG")))
        );
    }

    #[test]
    fn no_art_resolves_to_none() {
        let dir = tempfile::tempdir().expect("tempdir");
        let track = dir.path().join("01 song.wav");
        write_wav(&track, None);
        std::fs::write(dir.path().join("notes.txt"), b"x").expect("write");
        assert_eq!(resolve(&track), None);
        assert_eq!(load_thumb(&track), None);
    }

    #[test]
    fn load_thumb_decodes_and_downscales() {
        let dir = tempfile::tempdir().expect("tempdir");
        let track = dir.path().join("01 song.wav");
        write_wav(&track, None);
        // A cover larger than THUMB_PX must be downscaled to the budgeted
        // size; the cache-entry math in the module docs depends on this.
        std::fs::write(dir.path().join("cover.png"), png_bytes(600, 300)).expect("write");
        let (w, h, rgba) = load_thumb(&track).expect("thumb");
        assert!(w <= THUMB_PX && h <= THUMB_PX, "got {w}x{h}");
        assert_eq!(w, 256, "aspect ratio preserved, long edge = THUMB_PX");
        assert_eq!(h, 128);
        assert_eq!(rgba.len(), (w * h * 4) as usize);

        // An undecodable "cover" is no art, not an error.
        std::fs::write(dir.path().join("cover.png"), b"not an image").expect("write");
        std::fs::remove_file(dir.path().join("cover.jpg")).ok();
        assert_eq!(load_thumb(&track), None);
    }

    #[test]
    fn embedded_thumb_round_trip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let track = dir.path().join("01 song.wav");
        write_wav(&track, Some(png_bytes(300, 300)));
        let (w, h, _) = load_thumb(&track).expect("thumb");
        assert_eq!((w, h), (256, 256));
    }
}
