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
//! **[`THUMB_BUDGET_BYTES`] 160 MiB is the limit**, and it is a decision the
//! owner can disagree with rather than an emergent property of an LRU's
//! capacity argument — see that constant for where 160 comes from. Everything
//! below is how it is spent and held.
//!
//! Thumbnails are decoded to the active density's real maximum (200 px in
//! Dense, up to [`THUMB_PX`] in Spacious). Handles needed by the current wall
//! viewport, current page and resident chrome are pinned in a resident tier.
//! Once a handle has reached one of those targets it is retained, so scrolling
//! away and back cannot replace it with a gradient. A decode that finishes
//! after its target has left competes in a
//! [`THUMB_CACHE_ENTRIES`]-entry recent LRU, itself derived from
//! [`SPECULATIVE_BUDGET_BYTES`]. Prepared PNGs live in the local XDG cache for
//! the next launch.
//!
//! When the total would exceed the budget, `ThumbCache::trim_to_budget` drops
//! **speculative art first, then the least recently visited retained art**.
//! The resident tier is exempt: a visible sleeve turning back into a gradient
//! is the defect the whole tier exists to prevent. That exemption is safe
//! because the resident tier is bounded by the window — the widest window baz
//! supports pins **51 MiB** at its worst density (Dense: 336 tiles at the
//! 200 px ceiling), a little under a third of the budget, which
//! `the_visible_wall_can_never_exhaust_the_art_budget` asserts rather than
//! assumes.
//!
//! **The ceiling remains 320** (ADR-0017 step 5/7), because Spacious really
//! can draw a sleeve that large. Tighter densities now ask for their smaller
//! ceiling instead of paying for pixels they cannot display. Resident memory
//! is deliberately proportional to what the interface currently promises to
//! keep drawn: `N * edge * edge * 4` bytes, about **0.316 MiB per record** at
//! Balanced's 288 px ceiling. An 80-record Artist-page stress run therefore
//! held about **25.3 MiB** of decoded resident art. Ordinary virtualized walls
//! pin only the visible range and overscan; the current non-virtual Artist
//! page pins its whole discography until that page is left. Presentation
//! stability wins that measured memory trade: a sleeve shown once in this
//! process cannot be evicted back into a placeholder.
//!
//! The 2026-08-14 all-consumer audit closed three supply gaps around that
//! policy: Queue row pins were cleared later in the same update, Home omitted
//! the visible All songs collage, and the floating playlist panel never
//! nominated its collages. All now enter the same wall/chrome/page resident
//! union. The follow-up transition test walks 810 displayed ids, returns to the
//! first viewport and finds every handle still present; a separate stale-
//! density regression keeps the rest of the target queue intact.
//!
//! The owner's real 8,602-track index resolves to 393 albums. At Dense's 200 px
//! ceiling, retaining every square cover is at most **60.0 MiB** of CPU RGBA;
//! the measured first 180 real decodes occupied 27.3 MiB. Balanced (288 px)
//! and Spacious (320 px) worst cases are 124.3 and 153.5 MiB, and the last of
//! those is what [`THUMB_BUDGET_BYTES`] was chosen to clear — his whole
//! collection stays retained at every density.
//!
//! **These figures used to be the budget, and that was the defect.** They are
//! measurements of one collection on one machine; the 800-album synthetic
//! ceiling is 122.1 / 253.1 / 312.5 MiB, and nothing above that was bounded at
//! all. They are still worth keeping, because a limit chosen against a real
//! collection is a better limit than a round number — but they are now inputs
//! to the decision rather than a substitute for one.
//!
//! iced's wgpu raster cache trims device allocations to handles hit by the
//! current renderer pass, while retained RGBA handles make a return upload
//! synchronous; renderer residency therefore follows the current target set,
//! not the whole session store.
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
//! | [`load_thumb_cached`] | density-aware, ≤ [`THUMB_PX`] 320 | visited this session + [`THUMB_CACHE_ENTRIES`] 64 speculative/recent | **[`THUMB_BUDGET_BYTES`] 160 MiB**, enforced | the wall, the lane, every collage |
//! | [`load_hero`] | [`HERO_PX`] 1024 | [`HERO_CACHE_ENTRIES`] 2 | **8 MiB** | the Now playing place's one work |
//! | [`load_artist`] | [`ARTIST_PX`] 256 | [`ARTIST_CACHE_ENTRIES`] 8 | **2 MiB** | an artist page's portrait |
//!
//! **170 MiB is therefore the whole of baz's decoded artwork**, which is the
//! figure worth quoting because it is the one a process monitor shows —
//! and Settings → Debug now shows the resident set beside it
//! ([`crate::resource`]), so the claim is checkable inside the running app
//! rather than only in this comment.
//!
//! **The hero tier is 16 % more art memory** for the surface the owner wants
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

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::UNIX_EPOCH;

use lofty::picture::PictureType;
use lofty::prelude::*;

/// Max thumbnail edge in pixels; decoded art is downscaled to fit.
///
/// **Exactly [`crate::theme::ART_MAX`]**, which is the refusal *no artwork is
/// ever drawn larger than its source* as an equation;
/// `the_wall_never_draws_art_larger_than_its_source` in [`crate::shelf`]
/// asserts the two are one number.
pub const THUMB_PX: u32 = 320;

/// Bytes of decoded RGBA per pixel. Named because every budget below is this
/// times an area times a count, and a `4` written out four times is four
/// chances to write a `3`.
const RGBA: usize = 4;

/// **The thumbnail tier's whole decoded-RGBA budget: 160 MiB.**
///
/// # It is a decision now, and it was not one before
///
/// The owner, 2026-08-14: the tiered art machinery *"was introduced to try to
/// keep RAM usage down but we never specified a sensible limit."* He is right,
/// and the gap was specific — the **retained** tier had no bound at all except
/// the size of the indexed collection. Every figure this module published
/// (60.0 / 124.3 / 153.5 MiB at Dense/Balanced/Spacious on his 393 albums) was
/// a *measurement of what that shape happened to cost on his machine*, not a
/// limit the shape was built to meet. On a 5,000-album library the same code
/// retains something over two gigabytes and nothing in the product would have
/// said so.
///
/// # Where 160 comes from
///
/// It is chosen against the collection the feature exists for, and stated so
/// the next person can disagree with a number rather than with an emergent
/// property of `lru`'s capacity argument:
///
/// - The owner's 8,602-track index resolves to **393 albums**, whose worst
///   case — every cover square, at Spacious's 320 px ceiling — is
///   **153.5 MiB**. 160 clears it, so *his whole collection stays retained at
///   every density*, which is the case item 30 was built to serve.
/// - It is the smallest 32 MiB step that does, which keeps the headroom an
///   accident of rounding rather than a second undeclared decision.
/// - With the hero tier's 8 MiB ([`HERO_CACHE_ENTRIES`]) and the artist tier's
///   2 MiB ([`ARTIST_CACHE_ENTRIES`]), **all decoded artwork in the process is
///   under 170 MiB**, which is the number worth quoting because it is the one
///   a listener's process monitor shows. Settings → Debug now shows the
///   resident set beside it (`crate::resource`), so the claim is checkable
///   inside the running app.
///
/// # What it bounds, and the one thing it cannot
///
/// The cache trims **speculative first, then the least-recently-visited
/// retained art**, until the total fits. The **resident tier is exempt**, and
/// that exemption is the budget's one honest hole: the current frame's
/// artwork is un-evictable by construction (item 20 — a visible sleeve turning
/// back into a gradient is the defect this whole tier exists to prevent), so a
/// window large enough that its visible wall alone exceeded this figure would
/// exceed it. It cannot in practice — the widest supported window at the
/// largest edge holds well under a tenth of this — and
/// `the_visible_wall_can_never_exhaust_the_art_budget` asserts the margin
/// rather than leaving it to be believed.
pub const THUMB_BUDGET_BYTES: usize = 160 * 1024 * 1024;

/// **The share of [`THUMB_BUDGET_BYTES`] speculative work may hold: 25 MiB.**
///
/// Speculative art is what a decode completed for but no surface ever
/// displayed — the tail of a fast scroll, mostly. It is worth keeping (the
/// scroll usually comes back) and it is worth keeping *least*, so it is the
/// first thing trimmed and it gets a sub-budget of its own rather than
/// competing freely with art the listener has actually looked at.
///
/// 25 MiB is not a new number: it is exactly what the tier's long-standing 64
/// entries cost at the largest edge, which this module already published as
/// its worst case. Stating it as the budget and **deriving the entry count**
/// changes no behaviour and moves the decision to the side of the equation
/// where it belongs — a count of entries cannot be argued with, and a number
/// of megabytes can.
pub const SPECULATIVE_BUDGET_BYTES: usize = 25 * 1024 * 1024;

/// Off-screen thumbnail LRU capacity for decodes that never reached a visible
/// target: **64**, derived — [`SPECULATIVE_BUDGET_BYTES`] divided by one entry
/// at [`THUMB_PX`], which is `320 × 320 × 4` = 400 KiB.
///
/// A count rather than a byte cap because the LRU is a count, and because at
/// the *smaller* density ceilings the same 64 entries cost less (9.8 MiB at
/// Dense's 200 px) — so this is the tier's ceiling and not its size.
///
/// Current wall, page and chrome targets live in a separate resident tier and
/// do not count against this cap; see [`THUMB_BUDGET_BYTES`].
pub const THUMB_CACHE_ENTRIES: usize =
    SPECULATIVE_BUDGET_BYTES / (THUMB_PX as usize * THUMB_PX as usize * RGBA);

/// The retained tier's entry capacity — [`THUMB_BUDGET_BYTES`] at the
/// **smallest** entry it can hold, so the count can never bind before the
/// bytes do.
///
/// The bound that matters is the byte budget, enforced by
/// `ThumbCache::trim_to_budget`. This exists only because `LruCache` requires
/// a capacity, and giving it one that could bite first would be a second,
/// undeclared limit — exactly the thing this whole item is undoing. The
/// smallest entry is one square pixel of RGBA, which is what a 1 × 1 cover
/// decodes to and what the cache's own tests use.
#[must_use]
pub fn retained_capacity() -> std::num::NonZeroUsize {
    std::num::NonZeroUsize::new(THUMB_BUDGET_BYTES / RGBA).unwrap_or(std::num::NonZeroUsize::MIN)
}

/// Maximum thumbnail decodes allowed to run at once.
///
/// Two keeps image work bounded on low-end machines while still allowing a
/// slow file read to overlap one decode. Visible work is prioritised by the
/// scheduler in `app`, so a larger pool would mostly increase peak CPU and
/// allocation pressure rather than make the artwork in front of the listener
/// arrive sooner.
pub const THUMB_DECODE_CONCURRENCY: usize = 2;

/// Prepared artwork is derived and replaceable, but it still gets a disk
/// budget. Pruning runs once per process and removes oldest entries first.
const PREPARED_CACHE_BYTES: u64 = 256 * 1024 * 1024;
static PREPARED_CACHE_PRUNED: OnceLock<()> = OnceLock::new();

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
/// against the thumbnail tier's 9.8–25 MiB budget.
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

/// Artist portraits placed above album folders by common library managers.
const ARTIST_FILE_NAMES: [&str; 4] = ["artist.jpg", "artist.jpeg", "artist.png", "folder.jpg"];

/// Artist-page portraits are small supporting images, not Now Playing heroes.
pub const ARTIST_PX: u32 = 256;
/// Eight visited artists cost at most 2 MiB of decoded RGBA.
pub const ARTIST_CACHE_ENTRIES: usize = 8;

/// Rear-insert names used by common rippers and taggers, in priority order.
const BACK_FILE_NAMES: [&str; 6] = [
    "back.jpg",
    "back.jpeg",
    "back.png",
    "rear.jpg",
    "rear.jpeg",
    "rear.png",
];

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

/// Resolve a real rear cover for the jewel case, when the album carries one.
///
/// Unlike [`resolve`], an untyped embedded picture is not accepted: using a
/// booklet page as the rear insert is worse than Baz's generated track list.
#[must_use]
pub fn resolve_back(first_track: &Path) -> Option<ArtSource> {
    if let Some(bytes) = embedded_picture_of(first_track, PictureType::CoverBack) {
        return Some(ArtSource::Embedded(bytes));
    }
    let dir = first_track.parent()?;
    art_file(dir, &BACK_FILE_NAMES).map(ArtSource::File)
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

/// Decode a listener-provided artist portrait from the directory above an
/// album folder. Missing art is an ordinary `None`, never a placeholder.
#[must_use]
pub fn load_artist(first_track: &Path) -> Option<(u32, u32, Vec<u8>)> {
    let artist_dir = first_track.parent()?.parent()?;
    let source = art_file(artist_dir, &ARTIST_FILE_NAMES)?;
    decode_source(ArtSource::File(source), ARTIST_PX).map(into_parts)
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

fn embedded_picture_of(track: &Path, kind: PictureType) -> Option<Vec<u8>> {
    let file = lofty::read_from_path(track).ok()?;
    let tag = file.primary_tag().or_else(|| file.first_tag())?;
    tag.pictures()
        .iter()
        .find(|picture| picture.pic_type() == kind)
        .map(|picture| picture.data().to_vec())
}

/// The best cover file in `dir` per [`COVER_FILE_NAMES`], case-insensitive.
fn cover_file(dir: &Path) -> Option<PathBuf> {
    art_file(dir, &COVER_FILE_NAMES)
}

fn art_file(dir: &Path, names: &[&str]) -> Option<PathBuf> {
    let entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .map(|e| e.path())
        .collect();
    for candidate in names {
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

/// Decode a real rear cover at the Now Playing tier. Missing rear artwork is
/// not an error; the view generates a typographic insert from the track list.
#[must_use]
pub fn load_back(first_track: &Path) -> Option<(u32, u32, Vec<u8>)> {
    decode_source(resolve_back(first_track)?, HERO_PX).map(into_parts)
}

/// Resolve and decode an album's art into an RGBA thumbnail no larger than
/// [`THUMB_PX`] per edge: `(width, height, rgba_bytes)`. `None` when there
/// is no art or it cannot be decoded. Blocking; call off the UI thread.
#[must_use]
#[cfg_attr(not(test), allow(dead_code))]
pub fn load_thumb(first_track: &Path) -> Option<(u32, u32, Vec<u8>)> {
    decode(first_track, THUMB_PX)
}

/// Load a thumbnail at the edge the active shelf density actually draws.
///
/// Unlike [`load_thumb`], this path keeps a prepared PNG in the local XDG
/// cache. A warm launch therefore reads and decodes a small local image rather
/// than parsing tags and decoding the owner's (possibly remote) full-size
/// cover again. The key includes the track, adjacent cover-file metadata and
/// the requested edge, so replacing either source naturally misses the old
/// entry. Cache failures are deliberately invisible: artwork still takes the
/// ordinary source path.
#[must_use]
pub fn load_thumb_cached(first_track: &Path, edge: u32) -> Option<(u32, u32, Vec<u8>)> {
    dirs::cache_dir().map_or_else(
        || decode(first_track, edge),
        |root| load_cached(first_track, edge, &root.join("baz").join("art-v1")),
    )
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
#[cfg_attr(not(test), allow(dead_code))]
pub fn load_hero(first_track: &Path) -> Option<(u32, u32, Vec<u8>)> {
    decode(first_track, HERO_PX)
}

/// The hero tier with the same prepared local cache as thumbnails.
#[must_use]
pub fn load_hero_cached(first_track: &Path) -> Option<(u32, u32, Vec<u8>)> {
    dirs::cache_dir().map_or_else(
        || decode(first_track, HERO_PX),
        |root| load_cached(first_track, HERO_PX, &root.join("baz").join("art-v1")),
    )
}

fn load_cached(first_track: &Path, edge: u32, cache: &Path) -> Option<(u32, u32, Vec<u8>)> {
    if std::fs::create_dir_all(cache).is_ok() {
        PREPARED_CACHE_PRUNED.get_or_init(|| prune_prepared_cache(cache));
    }
    let path = cache.join(format!("{:016x}.png", cache_key(first_track, edge)));
    if let Ok(image) = image::open(&path) {
        return Some(into_parts(image.into_rgba8()));
    }

    let (w, h, rgba) = decode(first_track, edge)?;
    let image = image::RgbaImage::from_raw(w, h, rgba)?;
    if std::fs::create_dir_all(cache).is_ok() {
        let temporary = cache.join(format!(
            ".{:016x}-{}.tmp",
            cache_key(first_track, edge),
            std::process::id()
        ));
        if image
            .save_with_format(&temporary, image::ImageFormat::Png)
            .is_ok()
        {
            // A corrupt entry is only disposable derived data. Removing it
            // lets rename work on Windows too, where rename does not replace.
            if path.exists() {
                let _ = std::fs::remove_file(&path);
            }
            if std::fs::rename(&temporary, &path).is_err() {
                let _ = std::fs::remove_file(&temporary);
            }
        }
    }
    Some(into_parts(image))
}

fn prune_prepared_cache(cache: &Path) {
    let Ok(entries) = std::fs::read_dir(cache) else {
        return;
    };
    let mut files: Vec<_> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let metadata = entry.metadata().ok()?;
            metadata.is_file().then(|| {
                (
                    entry.path(),
                    metadata.len(),
                    metadata.modified().unwrap_or(UNIX_EPOCH),
                )
            })
        })
        .collect();
    let mut bytes: u64 = files.iter().map(|(_, len, _)| len).sum();
    if bytes <= PREPARED_CACHE_BYTES {
        return;
    }
    files.sort_unstable_by_key(|(_, _, modified)| *modified);
    for (path, len, _) in files {
        if bytes <= PREPARED_CACHE_BYTES {
            break;
        }
        if std::fs::remove_file(path).is_ok() {
            bytes = bytes.saturating_sub(len);
        }
    }
}

fn cache_key(first_track: &Path, edge: u32) -> u64 {
    let mut hash = DefaultHasher::new();
    "baz-art-v1".hash(&mut hash);
    first_track.hash(&mut hash);
    edge.hash(&mut hash);
    hash_metadata(first_track, &mut hash);
    // Embedded art still wins. Including an adjacent cover as an additional
    // invalidator costs one directory read but also notices the common act of
    // replacing cover.jpg without touching the audio file.
    if let Some(cover) = cover_file_beside(first_track) {
        cover.hash(&mut hash);
        hash_metadata(&cover, &mut hash);
    }
    hash.finish()
}

fn hash_metadata(path: &Path, hash: &mut impl Hasher) {
    let Ok(metadata) = std::fs::metadata(path) else {
        return;
    };
    metadata.len().hash(hash);
    let modified = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok());
    modified
        .map(|time| (time.as_secs(), time.subsec_nanos()))
        .hash(hash);
}

/// The body both tiers share: resolve, decode, downscale to fit `edge`.
///
/// One function because the two tiers must never disagree about **which file
/// the art came from** — a hero resolved by a different order than the
/// thumbnail would be a record whose sleeve changed when it started playing.
fn decode(first_track: &Path, edge: u32) -> Option<(u32, u32, Vec<u8>)> {
    decode_image(first_track, edge).map(into_parts)
}

fn decode_image(first_track: &Path, edge: u32) -> Option<image::RgbaImage> {
    decode_source(resolve(first_track)?, edge)
}

fn decode_source(source: ArtSource, edge: u32) -> Option<image::RgbaImage> {
    let image = match source {
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
    Some(scaled)
}

fn into_parts(image: image::RgbaImage) -> (u32, u32, Vec<u8>) {
    let (w, h) = image.dimensions();
    (w, h, image.into_raw())
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
    fn artist_picture_is_read_from_above_the_album_folder() {
        let root = tempfile::tempdir().expect("tempdir");
        let artist = root.path().join("Alice");
        let album = artist.join("A Record");
        std::fs::create_dir_all(&album).expect("album folder");
        let track = album.join("01 song.wav");
        write_wav(&track, None);
        std::fs::write(artist.join("Artist.PNG"), png_bytes(9, 7)).expect("artist picture");

        let (width, height, _) = load_artist(&track).expect("local artist picture");
        assert_eq!((width, height), (9, 7));
    }

    #[test]
    fn rear_cover_prefers_a_typed_embedded_picture_then_common_files() {
        let dir = tempfile::tempdir().expect("tempdir");
        let track = dir.path().join("01 song.wav");
        write_wav(&track, None);
        let front = png_bytes(3, 3);
        let back = png_bytes(5, 4);
        let mut tag = Tag::new(TagType::Id3v2);
        tag.push_picture(Picture::new_unchecked(
            PictureType::CoverFront,
            Some(MimeType::Png),
            None,
            front,
        ));
        tag.push_picture(Picture::new_unchecked(
            PictureType::CoverBack,
            Some(MimeType::Png),
            None,
            back.clone(),
        ));
        tag.save_to_path(&track, WriteOptions::default())
            .expect("embed pictures");
        std::fs::write(dir.path().join("back.jpg"), b"decoy").expect("write");
        assert_eq!(resolve_back(&track), Some(ArtSource::Embedded(back)));

        let other = dir.path().join("02 song.wav");
        write_wav(&other, None);
        std::fs::remove_file(dir.path().join("back.jpg")).expect("remove");
        std::fs::write(dir.path().join("Rear.PNG"), png_bytes(4, 4)).expect("write");
        assert_eq!(
            resolve_back(&other),
            Some(ArtSource::File(dir.path().join("Rear.PNG")))
        );
        assert!(load_back(&other).is_some());
    }

    #[test]
    fn no_art_resolves_to_none() {
        let dir = tempfile::tempdir().expect("tempdir");
        let track = dir.path().join("01 song.wav");
        write_wav(&track, None);
        std::fs::write(dir.path().join("notes.txt"), b"x").expect("write");
        assert_eq!(resolve(&track), None);
        assert_eq!(resolve_back(&track), None);
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

    #[test]
    fn prepared_cache_is_density_sized_and_source_invalidated() {
        let dir = tempfile::tempdir().expect("music tempdir");
        let cache = tempfile::tempdir().expect("cache tempdir");
        let track = dir.path().join("01 song.wav");
        write_wav(&track, None);
        let cover = dir.path().join("cover.png");
        std::fs::write(&cover, png_bytes(800, 400)).expect("large cover");

        let first = load_cached(&track, 200, cache.path()).expect("prepared thumb");
        assert_eq!((first.0, first.1), (200, 100));
        let again = load_cached(&track, 200, cache.path()).expect("warm prepared thumb");
        assert_eq!((again.0, again.1), (200, 100));
        assert_eq!(
            std::fs::read_dir(cache.path())
                .expect("cache listing")
                .count(),
            1,
            "a warm read created a duplicate prepared file"
        );

        let smaller = load_cached(&track, 100, cache.path()).expect("second density");
        assert_eq!((smaller.0, smaller.1), (100, 50));

        // A replacement whose dimensions and encoded length differ cannot
        // reuse the old source fingerprint.
        std::fs::write(&cover, png_bytes(50, 50)).expect("replacement cover");
        let replaced = load_cached(&track, 200, cache.path()).expect("replacement thumb");
        assert_eq!((replaced.0, replaced.1), (50, 50));
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
