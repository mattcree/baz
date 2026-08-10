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
//! baz budgets **~150 MiB** for decoded thumbnails, and the entry count is
//! derived from the real worst-case entry size rather than guessed:
//! 320 × 320 × 4 B = 400 KiB per thumbnail, so 150 MiB / 400 KiB = **384
//! entries** ([`THUMB_CACHE_ENTRIES`]). GPU-side copies held by `iced`'s
//! image cache are bounded by the same entry count.
//!
//! **The edge went 256 → 320 with the hang** (ADR-0017 step 5/7): the wall now
//! draws a sleeve at up to [`crate::theme::ART_MAX`] 320 px, and a cache that
//! stayed at 256 would have made *no artwork is ever drawn larger than its
//! source* false at every width above ~1120 px. The budget did not move; the
//! entry count absorbed it, which is 36 % fewer entries for 56 % more pixels
//! each. That is still ~8× the live widget count at any window size, so the
//! only thing it costs is a scroll further back through the wall before a
//! thumbnail has to be decoded again.
//!
//! # Two tiers, because two surfaces want different things
//!
//! The wall draws up to 120 records at 320 px. **The Now playing place draws
//! one record at whatever the viewport allows**, and 320 is nowhere near
//! enough for that — it is why that surface shipped clamped at a flat 720 and
//! was upscaling a 320 px thumbnail 2.25× to reach it (ADR-0029 §2, doc 12
//! §0.4 b). So there is a second decode of **one** record, [`load_hero`], at
//! [`HERO_PX`] 1024, in a [`HERO_CACHE_ENTRIES`]-entry cache:
//!
//! | Tier | Edge | Entries | Worst case | For |
//! |---|---|---|---|---|
//! | [`load_thumb`] | [`THUMB_PX`] 320 | [`THUMB_CACHE_ENTRIES`] 384 | ~150 MiB | the wall, the lane, every collage |
//! | [`load_hero`] | [`HERO_PX`] 1024 | [`HERO_CACHE_ENTRIES`] 2 | **8 MiB** | the Now playing place's one work |
//!
//! **The hero tier is 5.3 % more art memory** for the surface the owner wants
//! to leave running, and it is what makes *no artwork is ever drawn larger
//! than its source* true on that surface for the first time: the edge stops
//! being a chosen constant and becomes `min(viewport, the source's own
//! pixels)`, where the second term is what the decode actually returned.
//!
//! Both tiers call the **same** resolution order and the same decode, so a
//! record's hero can never disagree with its thumbnail about which file the
//! art came from.
//!
//! # A defect this found: the decode was enlarging small covers
//!
//! This module said *downscale-only* and was not.
//! [`image::DynamicImage::thumbnail`] scales **to fit** — it forwards to
//! `resize_dimensions(.., fill: false)`, whose ratio is not clamped at 1
//! (`image-0.24.9/src/dynimage.rs:716–719`) — so a 120 px cover was decoded to
//! 320 × 320, and the RGBA in the cache carried 6.8× more pixels than the file
//! had. Nothing caught it: the tests all used sources **larger** than
//! [`THUMB_PX`], which is the ordinary case and therefore the only one anybody
//! wrote.
//!
//! It never showed on the wall, because a 320 px handle in a 320 px tile is
//! 1 : 1 either way — the enlargement happened in the decoder instead of in
//! the GPU. It shows immediately on a surface that reads the decode's size and
//! **believes it**, which is what step A2's `art_edge` now does. [`decode`]
//! guards the call; `the_hero_is_the_same_art_decoded_larger_and_never_upscaled`
//! pins both tiers at both ends.

use std::path::{Path, PathBuf};

use lofty::picture::PictureType;
use lofty::prelude::*;

/// Max thumbnail edge in pixels; decoded art is downscaled to fit.
///
/// **Exactly [`crate::theme::ART_MAX`]**, which is the refusal *no artwork is
/// ever drawn larger than its source* as an equation;
/// `the_wall_never_draws_art_larger_than_its_source` in [`crate::shelf`]
/// asserts the two are one number.
pub const THUMB_PX: u32 = 320;

/// Thumbnail LRU capacity. Derivation (do not hand-tune without redoing it):
/// budget 150 MiB ÷ (320 × 320 px × 4 B/px = 400 KiB worst case) = 384.
pub const THUMB_CACHE_ENTRIES: usize = 384;

/// Max **hero** edge in pixels: **1024** — the Now playing place's own decode
/// tier (doc 12 §5.2).
///
/// Chosen, and the choice is a measurement rather than a preference: it is the
/// largest edge that is smaller than the shortest dimension of every panel
/// this surface targets (1080 is the smallest kiosk height), **so the cover is
/// never the thing limiting the layout — the viewport is**. A ceiling above
/// that would buy pixels no window could show and would cost them per entry.
///
/// It is a **ceiling, not a size**: [`load_hero`] downscales only, so a 500 px
/// cover comes back 500 px and is drawn at 500. `HERO_PX` is what the decoder
/// will not exceed; what the *surface* clamps against is what the decode
/// actually returned.
pub const HERO_PX: u32 = 1024;

/// Hero LRU capacity: **2**. Derivation, in the shape [`THUMB_CACHE_ENTRIES`]
/// uses — 1024 × 1024 px × 4 B/px = **4 MiB** per entry, × 2 = **8 MiB**,
/// against the thumbnail tier's 150 MiB budget (doc 12 §5.2's gate).
///
/// Two rather than one so that **the record that was sounding a moment ago is
/// still decoded**: a `Prev` press, or a jump back up the run, finds its hero
/// in hand rather than watching the sleeve grow into place. Doc 12 §5.2 asks
/// for the *successor* instead, and it cannot have it yet — see
/// [`crate::app::Shelf::request_hero`], which records why and what would give
/// it one.
pub const HERO_CACHE_ENTRIES: usize = 2;

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
    decode(first_track, THUMB_PX)
}

/// Resolve and decode an album's art into an RGBA image no larger than
/// [`HERO_PX`] per edge — **the Now playing place's tier** (doc 12 §5.2).
///
/// [`load_thumb`]'s function with one number changed, deliberately: the same
/// resolution order, the same downscale-only [`image::DynamicImage::thumbnail`],
/// the same `(w, h, rgba)` answer, the same blocking contract. What differs is
/// only what it is *for* — one record drawn as large as a viewport allows,
/// rather than 120 drawn at a tile's size.
///
/// **Downscale-only is the load-bearing property.** A 500 px cover comes back
/// 500 × 500, and `min(w, h)` of that is the number the surface clamps its
/// artwork against, so a small source is drawn at its own size, centred, with
/// the field around it (doc 12 §5.2, story S7). The refusal *no artwork is
/// ever drawn larger than its source* becomes arithmetic on this return value
/// rather than a constant that happened to be small enough.
#[must_use]
pub fn load_hero(first_track: &Path) -> Option<(u32, u32, Vec<u8>)> {
    decode(first_track, HERO_PX)
}

/// The body both tiers share: resolve, decode, downscale to fit `edge`.
///
/// One function because the two tiers must never disagree about **which file
/// the art came from** — a hero resolved by a different order than the
/// thumbnail would be a record whose sleeve changed when it started playing.
fn decode(first_track: &Path, edge: u32) -> Option<(u32, u32, Vec<u8>)> {
    let image = match resolve(first_track)? {
        ArtSource::Embedded(bytes) => image::load_from_memory(&bytes).ok()?,
        ArtSource::File(path) => image::open(path).ok()?,
    };
    // **The guard is what makes this downscale-only**, and it is not
    // decoration: `DynamicImage::thumbnail` scales *to fit*, in both
    // directions — it forwards to `resize_dimensions(.., fill: false)`, whose
    // ratio is not clamped at 1 (`image-0.24.9/src/dynimage.rs:716–719`). So
    // the call this module has always made was quietly **enlarging** any cover
    // smaller than its tier, and both this file's own prose and ADR-0029 §5.2
    // described it as downscale-only. Found by step A2, whose whole subject is
    // artwork that must never exceed its source; see the module docs.
    let scaled = if image.width() > edge || image.height() > edge {
        image.thumbnail(edge, edge).into_rgba8()
    } else {
        image.into_rgba8()
    };
    let (w, h) = scaled.dimensions();
    Some((w, h, scaled.into_raw()))
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
        std::fs::write(dir.path().join("cover.png"), png_bytes(800, 400)).expect("write");
        let (w, h, rgba) = load_thumb(&track).expect("thumb");
        assert!(w <= THUMB_PX && h <= THUMB_PX, "got {w}x{h}");
        assert_eq!(w, THUMB_PX, "aspect ratio preserved, long edge = THUMB_PX");
        assert_eq!(h, THUMB_PX / 2);
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
        write_wav(&track, Some(png_bytes(400, 400)));
        let (w, h, _) = load_thumb(&track).expect("thumb");
        assert_eq!((w, h), (THUMB_PX, THUMB_PX));
    }

    /// **The hero tier reads the same file the thumbnail does, and never
    /// invents a pixel.** Both halves matter: a hero resolved by a different
    /// order would be a record whose sleeve changed when it started playing,
    /// and a hero that upscaled would put the Now playing surface back where
    /// ADR-0029 §2 found it.
    #[test]
    fn the_hero_is_the_same_art_decoded_larger_and_never_upscaled() {
        let dir = tempfile::tempdir().expect("tempdir");
        let track = dir.path().join("01 song.wav");
        write_wav(&track, None);

        // A source between the two tiers: the thumbnail clamps it, the hero
        // does not, and neither exceeds the file.
        std::fs::write(dir.path().join("cover.png"), png_bytes(600, 600)).expect("write");
        assert_eq!(load_thumb(&track).map(|(w, h, _)| (w, h)), Some((320, 320)));
        assert_eq!(load_hero(&track).map(|(w, h, _)| (w, h)), Some((600, 600)));

        // A source **smaller than both** stays its own size in both tiers.
        // This is what makes `art_edge`'s third term honest for a listener
        // whose older rips carry 120 px covers (story S7).
        std::fs::write(dir.path().join("cover.png"), png_bytes(120, 120)).expect("write");
        assert_eq!(load_thumb(&track).map(|(w, h, _)| (w, h)), Some((120, 120)));
        assert_eq!(load_hero(&track).map(|(w, h, _)| (w, h)), Some((120, 120)));

        // A source **larger than the hero's ceiling** is clamped to it, so the
        // 8 MiB derivation in [`HERO_CACHE_ENTRIES`] is a worst case rather
        // than a hope.
        std::fs::write(dir.path().join("cover.png"), png_bytes(2400, 2400)).expect("write");
        let (w, h, rgba) = load_hero(&track).expect("hero");
        assert_eq!((w, h), (HERO_PX, HERO_PX));
        assert_eq!(rgba.len(), (w * h * 4) as usize);
        assert_eq!(
            rgba.len() * HERO_CACHE_ENTRIES,
            8 * 1024 * 1024,
            "the hero cache's stated 8 MiB is this number"
        );

        // The **embedded** picture wins for the hero exactly as it does for
        // the thumbnail — one resolution order, asserted rather than assumed.
        write_wav(&track, Some(png_bytes(700, 700)));
        assert_eq!(load_hero(&track).map(|(w, h, _)| (w, h)), Some((700, 700)));

        // And art that cannot be decoded is no art in both tiers, not an error.
        std::fs::remove_file(dir.path().join("cover.png")).expect("remove");
        let bare = dir.path().join("02 song.wav");
        write_wav(&bare, None);
        std::fs::write(dir.path().join("cover.png"), b"not an image").expect("write");
        assert_eq!(load_hero(&bare), None);
        assert_eq!(load_thumb(&bare), None);
    }
}
