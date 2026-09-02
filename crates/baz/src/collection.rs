//! **The collection the shell holds** — the loaded library, everything drawn
//! from it, and the artwork caches that stand behind both.
//!
//! ADR-0006 layer 2 is the application shell, and this is the half of it that
//! is not the application: [`Shelf`] is what a scan produced and every place
//! reads, while `app.rs` is `Message`, `update`, the subscriptions and the
//! composition. The two were one file until 2026-09-02, when `app.rs` stood at
//! **13 106 lines of shipped code** with a 7 553-line `impl App` and 2 889
//! lines of this beside it.
//!
//! # Why this was the first cut and not the largest one
//!
//! `docs/BACKLOG.md`'s proposal for `app.rs` is three ordered steps, and this
//! is step 1 because it is the only one that removes an **ambiguity** rather
//! than a line count. [`Shelf`] and [`crate::shelf`] are unrelated: the module
//! is the wall's grid arithmetic — [`crate::shelf::Grid`],
//! [`crate::shelf::Shelves`], the density detents, the virtualisation — and
//! the type is the library, its view models, its scan state, its health log
//! and its two tiers of decoded artwork. A reader meeting `shelf` in this
//! crate had to work out which one was meant, on every page, and the answer
//! was never in the name.
//!
//! Nothing here changed on the way across. The move is a move; the type keeps
//! its fields, its methods, its documentation and its name, and what it is
//! called is a separate question with a separate commit.
//!
//! # What is in here
//!
//! - [`Shelf`] itself: the [`baz_core::index::Library`], the album and artist
//!   view models the wall draws, the shelves those are broken into, the scan's
//!   live state, the health log, the play ledger, and the selection.
//! - **The two artwork tiers.** [`ThumbCache`] is the wall's bounded LRU with
//!   its resident handle tier for what is on screen (`WORK.md` item 5);
//!   [`Hero`] is one record decoded at the Now playing place's size, with the
//!   field derived from the same pixels.
//! - **The transition between two heroes** — [`Change`] and [`Showing`] — which
//!   is a rule about pictures rather than about a window, and is tested
//!   without either.

use std::collections::{HashMap, HashSet, VecDeque};
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::{Duration, Instant};

use baz_core::history::History;
use baz_core::index::{GroupKey, Library};
use iced::widget::column;
use iced::widget::image as iced_image;
use iced::widget::scrollable::AbsoluteOffset;
use iced::{Element, Size, Task};
use lru::LruCache;

use crate::app::{
    Blockage, CoverAction, Message, WINDOW, blur_search, check_folder, expand_tilde,
    folder_refusal, now_ns, persist, persist_density, persist_group_key, pick_folder, read_history,
    scroll_id, search_id, shifted_index,
};
use crate::motion::{Keyed, Tween};
use crate::player::PlayerState;
use crate::scan::ScanUpdate;
use crate::selection::Content;
use crate::{art, config, motion, scan, shelf, theme, views, vm};

/// One shelf of the wall, as the shell holds it: its header and the slice of
/// [`Shelf::albums`] under it.
///
/// The albums themselves stay in one flat vector so that a selection, a
/// thumbnail and a playing album are all still just an index — re-arranging
/// the wall must not re-key the caches (see [`Shelf::rebuild_shelves`]).
pub(crate) struct GroupVm {
    /// What the shelf's header draws, and what the rail projects.
    pub(crate) header: vm::GroupHeaderVm,
    /// One past its last album in [`Shelf::albums`].
    ///
    /// The end alone, because the shelves are contiguous and in order: a
    /// shelf begins where the one before it ended, and carrying both would be
    /// two numbers that have to agree.
    pub(crate) end: usize,
}

/// **One record decoded at the Now playing place's tier** — the artwork, the
/// number that bounds it, and the field derived from it (doc 12 §5.2, §5.3).
///
/// All three come out of **one** decode on **one** worker call, because all
/// three are readings of the same pixels and a second pass over them would be
/// a second chance to disagree.
#[derive(Debug, Clone)]
pub(crate) struct Hero {
    /// The cover at up to [`art::HERO_PX`] per edge.
    pub(crate) handle: iced_image::Handle,
    /// A real rear insert, when the files or tags carry one. `None` asks the
    /// jewel case to typeset the album's track list instead.
    pub(crate) back: Option<iced_image::Handle>,
    /// `min(width, height)` of what the decode actually returned — **the
    /// source's own pixels**, and the third term of the Now playing place's
    /// `art_edge`. Not [`art::HERO_PX`], which is only the decoder's ceiling:
    /// a 500 px cover yields 500 here and is drawn at 500.
    pub(crate) px: f32,
    /// The record's ambient field, or `None` when the cover carries no hue
    /// worth reading — a monochrome sleeve gets the room (story S7).
    pub(crate) field: Option<crate::field::Field>,
}

/// **What a newly-committed picture asks of the Now playing place** — the whole
/// of the crossfade's predicate, as a function of the two pictures and of
/// nothing else.
///
/// A free decision rather than three lines inside [`Shelf::settle_art`],
/// because *when a transition may run* is the part of this feature that is
/// worth being able to state and to test without a window, a library or a
/// player (ADR-0006 layer 1's habit, applied to a rule that could not quite
/// live in [`crate::motion`] — it is about an `iced` handle).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Change {
    /// **Draw the new picture and keep no clock.** Everything that is not two
    /// distinct covers: the first record of a session, a record with no art at
    /// either end, and — the case the owner would notice — a picture that has
    /// not actually changed.
    Cut,
    /// **Dissolve, over [`motion::DISSOLVE`].** Two decoded covers that are not
    /// the same picture.
    Dissolve,
}

impl Change {
    /// The rule: **two pictures, and they must differ.**
    ///
    /// `None` is *a record with no art*, which draws the wall's deterministic
    /// gradient — a stand-in rather than artwork, and dissolving a stand-in is
    /// decoration (ADR-0020 §3).
    ///
    /// Distinctness is `Handle`'s own equality, which for a decoded image is
    /// its allocation id and its pixels: **the handle being drawn**, not the
    /// track and not the record. Two clones of one decode are equal, so a
    /// surface redrawing what it already had can never start a flight. Two
    /// *separate* decodes of byte-identical covers — one record ripped twice —
    /// are not equal and would run a dissolve, which is invisible by
    /// construction (`X · (1 − t) + X · t` is `X` at every `t`) and costs the
    /// 200 ms of clock a rarity may cost.
    fn between(from: Option<&Hero>, to: Option<&Hero>) -> Self {
        match (from, to) {
            (Some(from), Some(to)) if from.handle != to.handle => Self::Dissolve,
            _ => Self::Cut,
        }
    }
}

/// **The Now playing place's artwork, mid-change and at rest** — what to draw,
/// what to draw it over, and how far between them the surface stands.
///
/// One value rather than three readings, for the reason
/// `views::now_playing::work`'s caller needs: the cover and the field are two
/// readings of one decode, and a surface that fetched them separately could
/// draw one record's picture over another record's room for a frame.
pub(crate) struct Showing<'a> {
    /// The picture the surface has committed to. `None` before the first hero
    /// of a session lands, and for a record with no art — both of which fall
    /// through to the thumbnail and then the gradient.
    pub(crate) hero: Option<&'a Hero>,
    /// The picture it is dissolving away from, while it is dissolving.
    pub(crate) from: Option<&'a Hero>,
    /// [`Self::hero`]'s own opacity over [`Self::from`], in `[0, 1]`. **`1.0`
    /// at rest**, which is one picture at full strength and no clock.
    pub(crate) t: f32,
}

/// `min(w, h)` as the `f32` a layout wants.
///
/// Named because both tiers spend it and because what it *means* is the same
/// sentence in both: **the largest square that may be drawn from this decode
/// without inventing a pixel.**
fn shortest_edge(w: u32, h: u32) -> f32 {
    f32::from(u16::try_from(w.min(h)).unwrap_or(u16::MAX))
}

/// A finished thumbnail decode, as the message carries it: its shortest edge
/// and its handle.
///
/// One function because three call sites build it — the wall's visible range,
/// the surfaces beside the wall, and the playlist collages — and three copies
/// of *decode, measure, wrap* is three places for the measurement to drift
/// away from the pixels it describes.
fn decoded((w, h, rgba): (u32, u32, Vec<u8>)) -> (f32, usize, iced_image::Handle) {
    let bytes = rgba.len();
    (
        shortest_edge(w, h),
        bytes,
        iced_image::Handle::from_rgba(w, h, rgba),
    )
}

pub(crate) struct ThumbEntry {
    handle: iced_image::Handle,
    decoded_bytes: usize,
}

/// The thumbnail cache with an un-evictable resident tier for what the current
/// frame can show.
///
/// The old single LRU could evict a visible sleeve when lane, playlist or
/// artist work filled the same 64 slots. Worse, the wall's unchanged-range
/// guard then declined to request it again. Prepared disk art made the reload
/// cheap but did not prevent the visible blank. This keeps the 64-entry LRU
/// for everything off screen and moves current targets into a separate map;
/// leaving the target returns a handle to the LRU immediately.
pub(crate) struct ThumbCache {
    recent: LruCache<u64, ThumbEntry>,
    resident: HashMap<u64, ThumbEntry>,
    /// Handles that have actually reached a visible target in this process.
    ///
    /// Moving away must not turn an already-present sleeve back into a
    /// gradient. Retaining only entries that were resident (rather than every
    /// speculative completion) makes the cost proportional to artwork the
    /// listener has visited.
    ///
    /// **It is an LRU now, and it was a `HashMap`.** "Bounded above by the
    /// indexed collection" was the whole of its bound, which is to say it had
    /// none: a large library retained every cover it ever showed, and the
    /// figures this project published were measurements of what that came to
    /// on the owner's 393 albums rather than a limit anything enforced. It is
    /// ordered so that [`art::THUMB_BUDGET_BYTES`] can be enforced against the
    /// **least recently visited** art, which is the only ordering under which
    /// trimming is not arbitrary.
    ///
    /// Its capacity is the byte budget at the *smallest* entry the tier can
    /// hold, so the count never binds before the bytes do — the bound that
    /// matters is [`ThumbCache::trim_to_budget`]'s, and this one exists only
    /// because `LruCache` requires a capacity.
    retained: LruCache<u64, ThumbEntry>,
    wall: HashSet<u64>,
    chrome: HashSet<u64>,
    page: HashSet<u64>,
}

impl ThumbCache {
    fn new(capacity: NonZeroUsize) -> Self {
        Self {
            recent: LruCache::new(capacity),
            resident: HashMap::new(),
            retained: LruCache::new(art::retained_capacity()),
            wall: HashSet::new(),
            chrome: HashSet::new(),
            page: HashSet::new(),
        }
    }

    fn peek(&self, id: u64) -> Option<&iced_image::Handle> {
        self.resident
            .get(&id)
            .or_else(|| self.retained.peek(&id))
            .or_else(|| self.recent.peek(&id))
            .map(|entry| &entry.handle)
    }

    /// Is this id's art already decoded — and, if it is, say so **and mark it
    /// used**.
    ///
    /// The promotion is the point and is why this takes `&mut`: it is called
    /// on every target of every re-aim, so "recently used" means "recently on
    /// screen", which is exactly the order [`Self::trim_to_budget`] has to
    /// trim against. `retained` moved from a `HashMap` to an LRU for this
    /// reason as much as for the popping.
    fn touch(&mut self, id: u64) -> bool {
        self.resident.contains_key(&id)
            || self.retained.get(&id).is_some()
            || self.recent.get(&id).is_some()
    }

    fn put(&mut self, id: u64, handle: iced_image::Handle, decoded_bytes: usize) {
        let entry = ThumbEntry {
            handle,
            decoded_bytes,
        };
        if self.is_pinned(id) {
            self.recent.pop(&id);
            self.retained.pop(&id);
            self.resident.insert(id, entry);
        } else {
            self.resident.remove(&id);
            self.retained.pop(&id);
            self.recent.put(id, entry);
        }
        self.trim_to_budget();
    }

    /// **Hold the stated budget** ([`art::THUMB_BUDGET_BYTES`]), by dropping
    /// the least valuable decoded artwork until the total fits.
    ///
    /// The order is the tiering argument, spent: **speculative first** — art a
    /// decode completed for that no surface ever displayed — and then the
    /// **least recently visited retained** art. Nothing the current frame can
    /// draw is ever dropped; the resident tier is exempt, because a visible
    /// sleeve turning back into a gradient is the defect this whole tier
    /// exists to prevent (item 20), and the loop stops rather than reaching
    /// for it. `the_visible_wall_can_never_exhaust_the_art_budget` is what
    /// makes that exemption safe to state.
    ///
    /// The running total is carried rather than recomputed: the sum is a walk
    /// of every entry, and recomputing it inside the loop would make trimming
    /// a large overflow quadratic in the size of the cache.
    fn trim_to_budget(&mut self) {
        let mut held = self.decoded_bytes();
        while held > art::THUMB_BUDGET_BYTES {
            let dropped = self
                .recent
                .pop_lru()
                .or_else(|| self.retained.pop_lru())
                .map(|(_, entry)| entry.decoded_bytes);
            let Some(dropped) = dropped else {
                // Only the resident tier is left, and it is not ours to take.
                break;
            };
            held = held.saturating_sub(dropped);
        }
    }

    fn clear_handles(&mut self) {
        self.recent.clear();
        self.resident.clear();
        self.retained.clear();
    }

    fn len(&self) -> usize {
        self.recent.len() + self.resident.len() + self.retained.len()
    }

    fn resident_len(&self) -> usize {
        self.resident.len()
    }

    fn retained_len(&self) -> usize {
        self.retained.len()
    }

    fn decoded_bytes(&self) -> usize {
        self.resident
            .values()
            .chain(self.retained.iter().map(|(_, entry)| entry))
            .chain(self.recent.iter().map(|(_, entry)| entry))
            .map(|entry| entry.decoded_bytes)
            .sum()
    }

    pub(crate) fn focus_wall(&mut self, ids: impl IntoIterator<Item = u64>) {
        self.wall = ids.into_iter().collect();
        self.reconcile();
    }

    fn focus_chrome(&mut self, ids: impl IntoIterator<Item = u64>) {
        self.chrome = ids.into_iter().collect();
        self.reconcile();
    }

    pub(crate) fn focus_page(&mut self, ids: impl IntoIterator<Item = u64>) {
        self.page = ids.into_iter().collect();
        self.reconcile();
    }

    fn is_pinned(&self, id: u64) -> bool {
        self.wall.contains(&id) || self.chrome.contains(&id) || self.page.contains(&id)
    }

    /// One ordered snapshot of every target the current composition can draw.
    /// Wall and page work lead the resident chrome, but no category replaces
    /// another category's queue.
    fn targets(&self) -> Vec<u64> {
        let mut seen = HashSet::new();
        self.wall
            .iter()
            .chain(&self.page)
            .chain(&self.chrome)
            .copied()
            .filter(|id| seen.insert(*id))
            .collect()
    }

    fn reconcile(&mut self) {
        let wanted: HashSet<u64> = self
            .wall
            .iter()
            .chain(&self.chrome)
            .chain(&self.page)
            .copied()
            .collect();
        let leaving: Vec<u64> = self
            .resident
            .keys()
            .filter(|id| !wanted.contains(id))
            .copied()
            .collect();
        for id in leaving {
            if let Some(entry) = self.resident.remove(&id) {
                self.retained.put(id, entry);
            }
        }
        for id in wanted {
            if self.resident.contains_key(&id) {
                continue;
            }
            if let Some(entry) = self.retained.pop(&id) {
                self.resident.insert(id, entry);
            } else if let Some(entry) = self.recent.pop(&id) {
                self.resident.insert(id, entry);
            }
        }
        // A composition change can only ever move art *into* the resident
        // tier or out of it, never decode more — but art arriving from the
        // speculative tier stops being trimmable when it does, so the budget
        // is re-checked here as well as after a decode.
        self.trim_to_budget();
    }
}

/// The bounded thumbnail work list.
///
/// Foreground requests replace one another: after a fast scroll, covers from
/// the old viewport must not stand ahead of the viewport now on screen.
/// The visible lane has its own queue behind the page, so its album covers and
/// playlist collages are not crowded out by a fast page scroll. The two
/// in-flight jobs are never
/// cancelled because image decoders are blocking; bounding their count makes
/// letting them finish cheaper and safer than pretending cancellation could
/// stop the underlying work.
#[derive(Debug, Default)]
pub(crate) struct ThumbJobs {
    foreground: VecDeque<u64>,
    queued: HashSet<u64>,
    in_flight: HashSet<u64>,
    started: u64,
    completed: u64,
    peak: usize,
}

impl ThumbJobs {
    fn focus(&mut self, ids: impl IntoIterator<Item = u64>) {
        for id in self.foreground.drain(..) {
            self.queued.remove(&id);
        }
        for id in ids {
            if self.in_flight.contains(&id) {
                continue;
            }
            if self.queued.insert(id) {
                self.foreground.push_back(id);
            }
        }
    }

    fn retry(&mut self, id: u64) {
        if !self.in_flight.contains(&id) && self.queued.insert(id) {
            self.foreground.push_front(id);
        }
    }

    fn pop(&mut self) -> Option<u64> {
        let id = self.foreground.pop_front()?;
        self.queued.remove(&id);
        Some(id)
    }

    fn started(&mut self, id: u64) {
        self.in_flight.insert(id);
        self.started += 1;
        self.peak = self.peak.max(self.in_flight.len());
    }

    fn finished(&mut self, id: u64) {
        self.in_flight.remove(&id);
        self.completed += 1;
    }
}

/// The shelf screen: library, scan state, and grid/panel view state.
///
/// Fields the view layer reads are `pub(crate)`; the ones the update loop
/// owns alone (in-flight decodes, the scan channel, click timing) stay
/// private — [`crate::views`] draws this state, it never steers it.
#[expect(
    clippy::struct_excessive_bools,
    reason = "the booleans are independent UI facts (scan, hover, lane, and \
              chooser visibility), not variants of one state machine"
)]
pub(crate) struct Shelf {
    /// The open library: the search index the counts and the query run over.
    pub(crate) library: Library,
    /// How the wall is arranged (ADR-0019). Persisted in `config.toml`; the
    /// top bar's row of words and `1`–`6` are the two ways to change it.
    pub(crate) group_key: GroupKey,
    /// How closely the wall hangs (ADR-0017 step 6). Persisted in
    /// `config.toml`; <kbd>Ctrl</kbd>+<kbd>-</kbd> / <kbd>Ctrl</kbd>+<kbd>=</kbd>
    /// and <kbd>Ctrl</kbd>+scroll are the two ways to change it, and there is
    /// no third way anywhere in the Settings place.
    pub(crate) density: shelf::Density,
    /// **What shape the collection is hung in** ([`shelf::Layout`]) — the wall
    /// of covers, or one record per row. Orthogonal to `density`, which says
    /// how big rather than what shape.
    pub(crate) layout: shelf::Layout,
    /// The play ledger, read once at open — what [`GroupKey::Played`] shelves
    /// on, and the returns lane's order key for a record.
    ///
    /// It had a second consumer, the pull's weighting; that went with the pull
    /// on 2026-08-10 (ADR-0018's amended third surface).
    ///
    /// A **snapshot**, not a live view: the file is append-only, so a snapshot
    /// can only ever be missing the last few minutes and can never be wrong
    /// about an earlier play (`baz_core::history::History`). `None` is a
    /// correct answer rather than a broken one — PLAYED then draws one
    /// `Never played` shelf holding the library, which is a true statement
    /// about a library baz has no record of.
    pub(crate) history: Option<History>,
    /// Owned view model of every album, in the active key's shelf order —
    /// the shelves flattened, so an album is still one index.
    pub(crate) albums: Vec<vm::AlbumVm>,
    /// One entry per shelf: what its header says and which slice of `albums`
    /// it holds. Contiguous and in wall order, so a shelf is a range rather
    /// than a per-album lookup.
    pub(crate) groups: Vec<GroupVm>,
    /// Indices into `albums` drawn by the wall, in wall order. App-bar search
    /// covers the current place instead of filtering this collection.
    pub(crate) visible: Vec<usize>,
    /// How many of each shelf's albums survived it, in `groups` order — what
    /// [`shelf::Shelves`] lays the wall out from.
    visible_counts: Vec<usize>,
    /// The live search text.
    pub(crate) query: String,
    /// Relevance-ordered track answers for the live app-bar query. The result
    /// surface virtualizes this complete bounded set instead of truncating it
    /// to the old Library Songs section.
    pub(crate) songs: Vec<vm::SongVm>,
    /// Relevance-ordered album answers for the dropover, as stable wall ids.
    pub(crate) search_albums: Vec<u64>,
    /// **Every playlist baz holds, by id and name** — the corpus the query is
    /// matched against, kept here because the shelf is where searching
    /// happens and the folder is the app's.
    ///
    /// Written by [`crate::app::App`] after each `playlists.refresh`, which is the only
    /// thing that can change it, so there is one writer and no clock.
    pub(crate) playlist_names: Vec<(u64, String)>,
    /// The playlists this query matches, in the panel's own order.
    pub(crate) search_playlists: Vec<u64>,
    /// Whether the non-empty query's dropover is currently exposed.
    pub(crate) search_open: bool,
    /// The selected track row's inline action. Albums keep their established
    /// activation and explicit Open grammar instead.
    pub(crate) search_action: crate::search::Action,
    /// Keyboard-selected action on the selected album cover.
    pub(crate) cover_action: CoverAction,
    /// Search's own selection/activation clock. It is separate from the place
    /// underneath so dismissing the dropover exposes that unchanged mark.
    pub(crate) search_selection: crate::selection::State,
    /// Absolute offset and measured height of the dropover's sole scroller.
    pub(crate) search_scroll_offset: f32,
    pub(crate) search_viewport_h: f32,
    /// **The record the wall was last left for**, if any. This remains the
    /// wall's navigation anchor; [`Self::selection`] separately owns the
    /// visible/actionable selection restored by ADR-0022's 2026-08-12
    /// amendment. Explicitly opening a record updates both facts, while
    /// selecting one without opening updates only the latter.
    ///
    /// Session-scoped, like everything else about where the wall is standing.
    pub(crate) opened: Option<u64>,
    /// The one selection/activation machine shared by playable tiles and rows.
    pub(crate) selection: crate::selection::State,
    /// Which format of an album the user picked, for albums where they
    /// picked one. Absent = the ranked-best edition (see
    /// [`vm::selected_edition`]).
    ///
    /// Session-scoped by choice: the persistent config is a hand-rolled
    /// single-key TOML file (see `config.rs`), so persisting a per-album map
    /// would mean adopting a real TOML parser for a preference whose proper
    /// home is a column in the library database anyway. Deferred in
    /// ADR-0007 rather than bolted on here.
    pub(crate) edition_choice: HashMap<u64, vm::EditionKey>,
    /// Decoded thumbnails: current-frame residents plus the bounded off-screen
    /// LRU. Access goes through [`Self::thumb`] so a view cannot accidentally
    /// bypass the residency guarantee.
    pub(crate) thumbs: ThumbCache,
    /// Small, bounded cache of local artist portraits visited this session.
    artist_images: LruCache<u64, iced_image::Handle>,
    /// Artist portrait decodes currently running off the UI thread.
    artist_image_pending: HashSet<u64>,
    /// Artists already found not to carry a local portrait.
    no_artist_image: HashSet<u64>,
    /// **The shortest edge of each decoded thumbnail**, in pixels.
    ///
    /// [`art::load_thumb`] downscales only, so this is `min(w, h)` of a
    /// picture that is either the source itself (a cover smaller than
    /// [`art::THUMB_PX`]) or a faithful reduction of it — **in both cases a
    /// true upper bound on what may be drawn from this handle**. It is what
    /// keeps *no artwork is ever drawn larger than its source* true on the
    /// Now playing place for the frames between arriving and the hero landing,
    /// rather than only afterwards.
    ///
    /// A plain map rather than an entry in [`Self::thumbs`]: the six surfaces
    /// that draw a thumbnail want the handle and nothing else, and widening
    /// the LRU's value would have touched all six to serve one. Four bytes per
    /// album the process has ever decoded — the same unbounded-by-design
    /// shape, and two orders of magnitude smaller than, [`Self::no_art`].
    thumb_px: HashMap<u64, f32>,
    /// **Decoded-hero LRU** — the Now playing place's own decode tier
    /// ([`art::HERO_CACHE_ENTRIES`] entries, 8 MiB worst case).
    heroes: LruCache<u64, Hero>,
    /// The record [`Self::request_hero`] has a decode in flight for.
    hero_pending: Option<u64>,
    /// The album range [`Shelf::request_visible_thumbs`] last asked about, so
    /// the two redundant requests every resize step delivers cost a
    /// comparison instead of a pass over the library. `None` until the first
    /// ask, and reset by anything that changes *which* albums the range names
    /// rather than where it sits — see [`Shelf::forget_requested`].
    last_requested: Option<(usize, usize)>,
    /// Visibility-first, bounded thumbnail scheduler.
    thumb_jobs: ThumbJobs,
    /// Albums known to have no (decodable) art — render the gradient and
    /// stop asking. Cleared once when the scan finishes, since late tracks
    /// or cover files may have arrived for early albums.
    no_art: HashSet<u64>,
    /// **Authored playlist sleeves**, decoded: playlist id → the picture the
    /// listener put beside that list's `.m3u8`.
    ///
    /// Held here rather than in [`crate::playlists::Playlists`] because this
    /// is where every surface that draws a sleeve already looks — the wall's
    /// tiles, the lane's rows and the page all reach a `Shelf` — and one
    /// answer cannot drift from another. One decode at
    /// [`art::THUMB_PX`] serves all three: it is exactly
    /// [`theme::ART_MAX`], the largest a playlist sleeve is ever drawn.
    ///
    /// Bounded by the number of lists that *have* a picture, which is a number
    /// the listener chose one file at a time; entries for lists that are gone
    /// are dropped whenever the folder is re-read.
    playlist_images: HashMap<u64, iced_image::Handle>,
    /// Playlist ids whose sleeve decode is in flight, so a re-render does not
    /// start it again. A picture that fails to decode stays out of
    /// [`Self::playlist_images`] and the list draws its collage, which is the
    /// same honest reading a record's tile gives art it cannot decode.
    playlist_image_jobs: HashSet<u64>,
    scan_rx: Option<Receiver<ScanUpdate>>,
    /// The music folders baz is holding, in the listener's order (ADR-0022).
    ///
    /// The shell's copy of `config.music_dirs`: the config file is the durable
    /// record and this is what is scanned, listed and removed from. They are
    /// kept in step by writing the config every time this moves.
    pub(crate) roots: Vec<PathBuf>,
    /// The folders the most recent pass could not walk at all. Cleared at the
    /// start of each pass, so it always describes the latest attempt rather
    /// than accumulating every share that was ever offline.
    pub(crate) unavailable: HashSet<PathBuf>,
    /// Bounded session history shown by the bottom-right status control.
    pub(crate) health: crate::health::Log,
    /// The periodic-refresh clock (ADR-0022 §3).
    refresh: scan::Refresh,
    /// What has been typed into the Settings place's add-a-folder field.
    folder_input: String,
    /// Why the last folder submitted was not added, if it was not.
    folder_error: Option<String>,
    /// Which folder's Remove is armed and waiting for its confirming press.
    folder_pending_removal: Option<usize>,
    /// Paths under successfully scanned roots whose own parent directories
    /// are absent. They require explicit confirmation and are never automatic.
    prunable: Vec<PathBuf>,
    /// Whether Settings is showing the exact bulk-prune consequence.
    prune_pending: bool,
    /// Whether Settings is showing the exact rootless-row consequence.
    unrooted_pending: bool,
    /// Whether the scan worker is still running.
    pub(crate) scanning: bool,
    /// Files the scan could not read.
    pub(crate) files_skipped: usize,
    /// A fatal-ish problem worth a status-line mention (scan could not
    /// start, or a library write failed). Never a modal.
    pub(crate) problem: Option<String>,
    /// Where the shelf is scrolled to (logical px from the top).
    pub(crate) scroll_offset: f32,
    /// The grid viewport's size, for the virtualization math.
    pub(crate) grid_size: Size,
    last_scan_log: Instant,
    /// Which album's tile the pointer is on, if any.
    ///
    /// The shelf's hover mark is a **rule drawn under the wall label**
    /// (ADR-0017 step 14), not a card behind the sleeve — and a rule under the
    /// label is a *sibling* of the button, not the button. iced 0.13 tells a
    /// widget its own hover status inside a style function and tells its
    /// siblings nothing, so the tile reports its own crossings with a
    /// `mouse_area` and the shelf holds the one answer. Exactly the pattern
    /// [`crate::app::App::hovered_queue_row`] already uses, and for exactly the same
    /// toolkit reason.
    ///
    /// The rule's lane is reserved whatever this says, so it changes what is
    /// drawn in it and never the geometry around it.
    pub(crate) hovered_album: Option<u64>,
    /// Whether the pointer is on Home's **All songs** tile.
    ///
    /// [`Self::hovered_album`]'s mechanism for the one tile that is not a
    /// record — a `bool` because there is exactly one of it, where the wall has
    /// hundreds and needs an id. It carries no tween for the same reason: the
    /// wall's keyed tween exists so that crossing a gutter *hands the mark over*
    /// from one sleeve to the next, and a lone tile has nothing to hand it to.
    /// The hover options themselves were always a boolean reveal rather than a
    /// tween (`views::shelf`'s `hover_options`), so this tile's layer appears
    /// exactly as the wall's does.
    pub(crate) hovered_all_songs: bool,
    /// How far the hovered tile's mark has travelled (ADR-0020 §2.3).
    ///
    /// **One tween for the whole wall, keyed by the hovered id — never one per
    /// tile.** The shelf draws hundreds of tiles and at most one of them is
    /// under the pointer, so a tween per tile would be state allocated for a
    /// condition all but one of them is never in; and crossing the gutter from
    /// one sleeve to the next hands the mark over rather than restarting it
    /// (see [`crate::motion::Keyed`]).
    pub(crate) tile_hover: Keyed<u64>,
    /// Home's explicit, local metadata-playlist request composer.
    pub(crate) vibe: crate::vibe::State,
    /// **What the Now playing place has committed to drawing of the record** —
    /// the record's id, and its hero when the answer was a picture.
    ///
    /// The word is *committed* rather than *sounding*, and the difference is
    /// the whole of [`Self::settle_art`]: a record whose hero has not finished
    /// decoding has no answer yet, so this still names the record before it and
    /// the surface goes on drawing what it was drawing. `Some((id, None))` is
    /// a record the decode has answered *no art* for — the gradient
    /// placeholder, and nothing to dissolve.
    ///
    /// **It costs no memory.** The `Hero` is a clone whose handle is an `Arc`
    /// over the same decoded pixels, and the record it names is always the
    /// freshest entry of the two-entry hero LRU — [`Self::request_hero`] `get`s
    /// the sounding record on every message, which is what keeps it there.
    art_shown: Option<(u64, Hero)>,
    /// **The picture the hero is dissolving away from**, for as long as
    /// [`Self::art_dissolve`] is live, and `None` at every other instant.
    ///
    /// The second entry of the hero LRU is what makes this free: the record
    /// that just stopped is still decoded, so both pictures are alive at once
    /// and the crossfade needs no cache of its own. Checked rather than
    /// assumed — see [`Self::settle_art`].
    art_prior: Option<Hero>,
    /// **The incoming hero's opacity**, `0` → `1` over [`motion::DISSOLVE`],
    /// linear (ADR-0020's third amendment).
    ///
    /// Settled at `1` at rest, which is the surface drawing one picture at full
    /// strength and keeping no clock.
    art_dissolve: Tween,
    /// The width of the window the shelf is laid out in.
    ///
    /// **The wall's width, full stop.** It used to be the window's less
    /// whatever the inspector was taking at this instant — a number that
    /// changed nine times over 150 ms — and with no side surface left there is
    /// nothing to subtract but the index rail's lane (see
    /// [`Shelf::grid_width`]).
    ///
    /// Crate-visible because the view layer resolves the app bar, lane and
    /// virtualized surfaces from this same measurement.
    pub(crate) window_w: f32,
    /// **Whether the returns lane stands open** (ADR-0030 §3), as the config
    /// remembers it.
    ///
    /// It lives here rather than on the shell because [`Shelf::grid_width`]
    /// reads it: the lane's width is a term in the wall's, and the wall's
    /// width is resolved in exactly one place.
    pub(crate) lane_open: bool,
    /// **When each record was last played**, in seconds since the Unix epoch —
    /// the ledger folded onto records, once.
    ///
    /// Built at launch from the [`History`] snapshot in one pass over the
    /// library, and thereafter maintained by *events*: a `TrackStarted`
    /// updates one entry. That is ADR-0030 §4's responsiveness contract made
    /// literal — **never a per-frame file read, and no watcher**.
    lane_played: HashMap<u64, u64>,
    /// The lane's records half, resolved and trimmed to
    /// [`crate::lane::RECENT_ALBUMS`]: what the lane draws, less the lists.
    ///
    /// Cached rather than derived per frame because deriving it walks every
    /// album; the lists are merged in at view time, which is O(playlists) and
    /// independent of the library's size.
    pub(crate) lane_recent: Vec<crate::lane::Touched>,
    /// Bumped whenever [`Self::lane_recent`] is rebuilt — the shell's cue to
    /// re-merge, without comparing two vectors of strings.
    pub(crate) lane_stamp: u64,
    /// **The collection's four figures**, for the Home place's `COLLECTION`
    /// footer ([`vm::Collection`]).
    ///
    /// Cached here for ADR-0030 §4's reason: three of the four are a pass over
    /// every track, and the contract forbids paying that per frame. It is
    /// rebuilt exactly where the albums it counts are —
    /// [`Self::rebuild_shelves`] — so it cannot describe a library that is no
    /// longer on the shelf.
    pub(crate) collection: vm::Collection,
    /// Offline facts for each artist, rebuilt with the album wall and read by
    /// the artist page without walking tracks during a frame.
    pub(crate) artist_facts: HashMap<u64, vm::ArtistFacts>,
    /// Album indices for records on which an artist is credited but which are
    /// filed under somebody else. See [`vm::artist_inventory`].
    artist_also_on: HashMap<u64, Vec<usize>>,
}

impl Shelf {
    /// **Is this file on a drive that is not connected?** — the reading every
    /// track surface takes.
    ///
    /// From [`Self::unavailable`], which the scan already maintains: it is
    /// cleared at the start of every pass and filled by the folders that pass
    /// could not walk, so it describes the latest attempt and clears itself
    /// when the drive comes back. The backlog entry that asked for this said
    /// *"the hard part is not the badge, it is knowing"* and worried about a
    /// `stat` per visible row per frame. The knowing was already here; the
    /// first attempt at this shipped a second probe on the same clock before
    /// noticing.
    pub(crate) fn offline(&self, path: &std::path::Path) -> bool {
        crate::reach::unreachable(&self.unavailable, path)
    }

    /// Current play-ledger snapshot for the local Now Playing fact feed.
    pub(crate) fn history(&self) -> Option<&History> {
        self.history.as_ref()
    }
}

impl Shelf {
    /// Open the library DB, hydrate the shelf, persist the chosen folders, and
    /// kick off the scan worker.
    ///
    /// **The error is a [`Blockage`] and not a sentence**, which is the whole
    /// of ADR-0041 at this seam: every failure here used to arrive at the
    /// first-run screen as a string, and a string cannot be routed. A caller
    /// that knows *which* failure it has can draw the newer-baz statement, and
    /// can decide that `Try again` means something here and nothing there.
    ///
    /// **Nothing on the failing path writes.** The directory is created —
    /// which a genuine first run needs and which costs an empty folder at
    /// worst — and then `Library::open` reads `user_version` before it sets a
    /// pragma. `adopt_roots`, `persist_roots` and the scan all sit *after* the
    /// open, so a refused library leaves the config file, the database and the
    /// listener's folders exactly as they were.
    #[expect(
        clippy::too_many_lines,
        reason = "opening the shelf initializes its complete session view model in one auditable place"
    )]
    pub(crate) fn open(
        roots: Vec<PathBuf>,
        group_key: GroupKey,
        density: shelf::Density,
        layout: shelf::Layout,
        lane_open: bool,
    ) -> Result<(Self, Task<Message>), Blockage> {
        let t0 = Instant::now();
        let db_path = config::library_db_file().ok_or_else(|| Blockage::Nowhere {
            detail: "this system offers no data directory for baz to keep an index in".to_owned(),
        })?;
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Blockage::Nowhere {
                detail: format!("cannot create {}: {e}", parent.display()),
            })?;
        }
        let mut library = Library::open(&db_path).map_err(|e| Blockage::of(&e))?;
        // Schema v8's backfill, and the one place that can make it (ADR-0022):
        // `baz-core` cannot know which folder a pre-v8 row came from, and this
        // is the code that reads the config file that does. Rows already naming
        // a root are untouched, and a row under none of these folders stays
        // rootless — which means unprunable, the safe direction.
        adopt_roots(&mut library, &roots);
        // The ledger, read once. A missing file is an empty history and not an
        // error; an unreadable one costs the PLAYED key its detail and nothing
        // else, so it is a note rather than a `problem`.
        let history = read_history();

        persist_roots(&roots);
        // The snapshot is what makes the scan incremental — and the only
        // rows it is ever allowed to prune (see `scan::vanished`).
        let scan_rx = scan::spawn(
            roots.clone(),
            library.known_files(),
            scan::ScanMode::Incremental,
        );

        let mut shelf = Self {
            library,
            group_key,
            density,
            layout,
            history,
            albums: Vec::new(),
            groups: Vec::new(),
            visible: Vec::new(),
            visible_counts: Vec::new(),
            query: String::new(),
            songs: Vec::new(),
            search_albums: Vec::new(),
            playlist_names: Vec::new(),
            search_playlists: Vec::new(),
            search_open: false,
            search_action: crate::search::Action::Play,
            cover_action: CoverAction::Play,
            search_selection: crate::selection::State::default(),
            search_scroll_offset: 0.0,
            search_viewport_h: 0.0,
            opened: None,
            selection: crate::selection::State::default(),
            edition_choice: HashMap::new(),
            thumbs: ThumbCache::new(
                NonZeroUsize::new(art::THUMB_CACHE_ENTRIES).unwrap_or(NonZeroUsize::MIN),
            ),
            artist_images: LruCache::new(
                NonZeroUsize::new(art::ARTIST_CACHE_ENTRIES).unwrap_or(NonZeroUsize::MIN),
            ),
            artist_image_pending: HashSet::new(),
            no_artist_image: HashSet::new(),
            thumb_px: HashMap::new(),
            heroes: LruCache::new(
                NonZeroUsize::new(art::HERO_CACHE_ENTRIES).unwrap_or(NonZeroUsize::MIN),
            ),
            hero_pending: None,
            last_requested: None,
            thumb_jobs: ThumbJobs::default(),
            no_art: HashSet::new(),
            playlist_images: HashMap::new(),
            playlist_image_jobs: HashSet::new(),
            scan_rx: Some(scan_rx),
            roots,
            unavailable: HashSet::new(),
            health: crate::health::Log::default(),
            refresh: scan::Refresh::new(scan::REFRESH_INTERVAL, Instant::now()),
            folder_input: String::new(),
            folder_error: None,
            folder_pending_removal: None,
            prunable: Vec::new(),
            prune_pending: false,
            unrooted_pending: false,
            scanning: true,
            files_skipped: 0,
            problem: None,
            scroll_offset: 0.0,
            grid_size: Size::new(
                WINDOW.width - theme::INDEX_LANE_W,
                WINDOW.height - theme::APP_BAR_H - theme::top_bar_h(WINDOW.width, lane_open),
            ),
            last_scan_log: Instant::now(),
            hovered_album: None,
            hovered_all_songs: false,
            tile_hover: Keyed::new(),
            vibe: crate::vibe::State::default(),
            art_shown: None,
            art_prior: None,
            art_dissolve: Tween::settled(1.0).with_curve(motion::Curve::Linear),
            window_w: WINDOW.width,
            lane_open,
            lane_played: HashMap::new(),
            lane_recent: Vec::new(),
            lane_stamp: 0,
            collection: vm::Collection::default(),
            artist_facts: HashMap::new(),
            artist_also_on: HashMap::new(),
        };
        shelf.health.record(
            crate::health::Level::Working,
            "Library scan started",
            format!("Checking {} configured folders", shelf.roots.len()),
        );
        // `rebuild_shelves` folds the ledger onto the records it has just
        // built (ADR-0030 §4): once, here, and never again from the file.
        shelf.rebuild_shelves();
        let shelf_task = shelf.request_visible_thumbs();
        crate::baz_log!(
            "[startup] library open + hydrate: {:.1} ms ({} albums / {} shelves by {} at {} / {} tracks) from {}",
            t0.elapsed().as_secs_f64() * 1e3,
            shelf.albums.len(),
            shelf.groups.len(),
            group_key.code(),
            density.label(),
            shelf.library.len(),
            db_path.display()
        );
        // **The well does not take focus at startup any more**, and that is
        // step 11's doing rather than a tidy-up. It used to, so that a listener
        // could type immediately — which was the right trade while typing
        // needed a focused field, and which cost <kbd>Space</kbd> its meaning
        // until the first <kbd>Esc</kbd>, a wart the README had to document.
        // Type-anywhere pays for the typing without the focus: the first letter
        // reaches the query from the wall (`crate::keys`), so the caret can
        // start where the transport is and the keyboard means what the key
        // table says on the first frame.
        Ok((shelf, shelf_task))
    }

    pub(crate) fn update(&mut self, message: Message) -> Task<Message> {
        // The Library section's own small machine, answered first and
        // separately — six messages that all resolve to "change which folders
        // baz holds, then rescan", exactly as the volume's nine are answered
        // apart from the shell's own arms.
        if let Some(task) = self.update_library(&message) {
            return task;
        }
        match message {
            Message::SearchChanged(query) => {
                self.query = query;
                self.search_selection.clear();
                self.refilter();
                self.search_open = !self.query.trim().is_empty();
                self.search_scroll_offset = 0.0;
                iced::widget::operation::scroll_to(
                    views::search::scroll_id(),
                    AbsoluteOffset { x: 0.0, y: 0.0 },
                )
            }
            // **The `×`, which is `Esc`'s pointer route** — the identical
            // function, so the two cannot drift (ADR-0036 §4).
            Message::ClearSearch => self.clear_query(),
            Message::SearchScrolled(viewport) => {
                self.search_scroll_offset = viewport.absolute_offset().y;
                self.search_viewport_h = viewport.bounds().height;
                Task::none()
            }
            // **Type anywhere** has no arm here any more. Its message is
            // answered by the shell (`App::type_anywhere`), which reaches the
            // well — the Library, and the lane opened if the well is in it —
            // before handing the text down to [`Self::type_into_query`]. The
            // shelf cannot do that half: the place and the lane are the
            // shell's state, and the owner's move put the field in the lane.
            Message::EscapePressed => self.peel(),
            Message::GroupKeySelected(key) => self.arrange_by(key),
            Message::RailJumped(run) => self.jump_to_shelf(run),
            Message::Scrolled(viewport) => {
                self.scroll_offset = viewport.absolute_offset().y;
                let bounds = viewport.bounds();
                // The scrollable's *outer* bounds. The rows are laid out inside
                // the lanes it reserves, so the grid is told what the rows
                // actually get — otherwise the estimate and the measurement
                // disagree, and at a boundary width that is one column too
                // many. The reservation is [`theme::WALL_RESERVE`]: the bar's
                // 4 px **and** the index rail's 108, because the scrollable now
                // takes the whole body width so its bar can be drawn on the
                // window's edge and the rail is stacked under it
                // (`views::shelf::view`). It was the bar's width alone while
                // the rail was a `row!` sibling that took its lane first.
                // A scrollable leaving the tree can briefly report an empty
                // viewport. That is not a new one-column layout: keep the last
                // real width until either another real viewport or a window
                // resize supplies its replacement. This matters to the other
                // collection places, which share this grid but have no Library
                // wall of their own to measure it again.
                if bounds.width > theme::WALL_RESERVE {
                    self.grid_size.width = bounds.width - theme::WALL_RESERVE;
                }
                if bounds.height > 0.0 {
                    self.grid_size.height = bounds.height;
                }
                self.request_visible_thumbs()
            }
            Message::WindowResized(size) => {
                self.window_w = size.width;
                // Estimate until the next scroll event reports real bounds.
                // The rail's lane comes off here too, because the scrollable
                // the next `Scrolled` will measure has already given it up.
                // The strip's height is *resolved* against the window **and
                // the lane's state** — below the split the strip is two lines,
                // and an estimate that assumed one would mis-virtualize 40 px
                // of shelf. It took only the width until now, while the strip
                // itself was drawn at `App::body_width`; between a 1000 and a
                // 1056 px window with the lane open the two disagreed by
                // exactly those 40 px. One function, both facts.
                self.grid_size = Size::new(
                    self.grid_width(),
                    (size.height - theme::APP_BAR_H - theme::top_bar_h(size.width, self.lane_open))
                        .max(100.0),
                );
                self.request_visible_thumbs()
            }
            Message::TileEntered(id) => {
                self.hovered_album = Some(id);
                self.tile_hover.enter(id, motion::TILE, Instant::now());
                Task::none()
            }
            // Only if it is still the tile that left: both messages are
            // published from one `CursorMoved` in widget order, so crossing the
            // wall delivers the new tile's entry before the old tile's exit,
            // and an exit meaning "nothing is hovered" would undo it.
            Message::TileLeft(id) => {
                if self.hovered_album == Some(id) {
                    self.hovered_album = None;
                }
                self.tile_hover.leave(id, motion::TILE, Instant::now());
                Task::none()
            }
            Message::EditionSelected(id, key) => {
                // Pure view state: the track list and the *next* queue follow
                // it, but nothing already playing is disturbed.
                self.edition_choice.insert(id, key);
                Task::none()
            }
            Message::ThumbLoaded(id, edge, elapsed, decoded) => {
                self.finish_thumb(id, edge, elapsed, decoded)
            }
            // The hero tier's answer, in the thumbnail tier's own shape: a
            // decode that found nothing is recorded in the **same**
            // known-absent set, because both tiers ran the same resolution
            // order and a second ask would be a question already answered.
            Message::HeroLoaded(id, hero) => {
                if self.hero_pending == Some(id) {
                    self.hero_pending = None;
                }
                match hero {
                    Some(hero) => {
                        self.heroes.put(id, hero);
                    }
                    None => {
                        self.no_art.insert(id);
                    }
                }
                Task::none()
            }
            Message::ArtistImageLoaded(id, image) => {
                self.artist_image_pending.remove(&id);
                match image {
                    Some(image) => {
                        self.artist_images.put(id, image);
                    }
                    None => {
                        self.no_artist_image.insert(id);
                    }
                }
                Task::none()
            }
            Message::ScanTick => self.drain_scan(),
            _ => Task::none(),
        }
    }

    /// **<kbd>Enter</kbd>'s defensive album-level fall-through** outside the
    /// open chooser: the record the wall was last left for when no query
    /// stands, else the top-ranked matching album. A normal standing query has
    /// the chooser open and never reaches this older compatibility path.
    ///
    /// The order is ADR-0017 §1.2's table read left to right — *play the
    /// top-ranked match; play the selected album* — and the fall-through is
    /// what makes it one key rather than two: with a query you are choosing
    /// from what you typed, and without one you are choosing what you last
    /// opened ([`Shelf::opened`] — the mark the wall carries where a selection
    /// used to be, ADR-0022).
    ///
    /// The ranked answer is [`vm::top_match`] (ADR-0021), **filtered through
    /// the wall**: an album is only played if it is on screen. In practice the
    /// two always agree — both come from the same query against the same
    /// library — and the check is here so that "Enter plays the first match"
    /// stays a statement about the wall a listener is looking at rather than
    /// about a search index they cannot see. If the ranked album is somehow
    /// not on the wall, the wall's own first survivor is played instead, which
    /// is the record under the top-left corner of the collection.
    /// **What the defensive query fallback needle-drops**: the Songs section's
    /// own first row — the top-ranked matching track (ADR-0021),
    /// as (its record's wall id, its row in the selected edition) for
    /// [`crate::app::App::play_track`] to spend (doc 09 §5; ADR-0023 §2's amendment).
    ///
    /// `None` with no query, and `None` when nothing matches — <kbd>Enter</kbd>
    /// then falls through to [`Self::enter_plays`], whose empty-query answer
    /// (the record the wall was last left for) is unchanged. The row is
    /// resolved by [`vm::song_row`], so what <kbd>Enter</kbd> plays is
    /// exactly what a press on the section's first row plays — one answer,
    /// two routes.
    pub(crate) fn enter_drops_needle(&self) -> Option<(u64, usize)> {
        if self.query.trim().is_empty() {
            return None;
        }
        let song = self.songs.first()?;
        let album = self.albums.iter().find(|album| album.id == song.album_id)?;
        let chosen = self.edition_choice.get(&album.id).copied();
        let row = vm::song_row(album, chosen, song)?;
        Some((album.id, row))
    }

    /// The selected search row, only while it still belongs to the current
    /// ranked result set. A query edit may make an old content key stale; Enter
    /// must never activate a row no longer on screen.
    pub(crate) fn selected_search_track(&self) -> Option<Content> {
        let Content::SearchTrack { album, row } = self.search_selection.selected()? else {
            return None;
        };
        self.songs
            .iter()
            .any(|song| {
                if song.album_id != album {
                    return false;
                }
                let Some(record) = self.albums.iter().find(|record| record.id == album) else {
                    return false;
                };
                let chosen = self.edition_choice.get(&album).copied();
                vm::song_row(record, chosen, song) == Some(row)
            })
            .then_some(Content::SearchTrack { album, row })
    }

    pub(crate) fn search_result_count(&self) -> usize {
        self.songs.len() + self.search_albums.len() + self.search_playlists.len()
    }

    pub(crate) fn search_result_content(&self, index: usize) -> Option<Content> {
        if let Some(song) = self.songs.get(index) {
            let album = self.albums.iter().find(|album| album.id == song.album_id)?;
            let chosen = self.edition_choice.get(&album.id).copied();
            let row = vm::song_row(album, chosen, song)?;
            return Some(Content::SearchTrack {
                album: album.id,
                row,
            });
        }
        let after_songs = index.checked_sub(self.songs.len())?;
        if let Some(album) = self.search_albums.get(after_songs) {
            return Some(Content::Album(*album));
        }
        self.search_playlists
            .get(after_songs.checked_sub(self.search_albums.len())?)
            .copied()
            .map(Content::Playlist)
    }

    pub(crate) fn search_result_index(&self, content: Content) -> Option<usize> {
        (0..self.search_result_count())
            .find(|index| self.search_result_content(*index) == Some(content))
    }

    pub(crate) fn move_search_selection(&mut self, delta: i32) -> Task<Message> {
        let selected = self
            .search_selection
            .selected()
            .and_then(|content| self.search_result_index(content));
        let Some(index) = crate::search::moved_index(selected, self.search_result_count(), delta)
        else {
            return Task::none();
        };
        let Some(content) = self.search_result_content(index) else {
            return Task::none();
        };
        self.search_selection.select(content);
        self.search_action = crate::search::Action::Play;

        let top = crate::search::result_top(index, self.songs.len(), self.search_albums.len());
        let bottom = top + crate::search::ROW_H;
        let viewport = self.search_viewport_h;
        if viewport <= 0.0 {
            return blur_search();
        }
        let target = if top < self.search_scroll_offset {
            Some(top)
        } else if bottom > self.search_scroll_offset + viewport {
            Some((bottom - viewport).max(0.0))
        } else {
            None
        };
        Task::batch([
            blur_search(),
            target.map_or_else(Task::none, |y| {
                iced::widget::operation::scroll_to(
                    views::search::scroll_id(),
                    AbsoluteOffset { x: 0.0, y },
                )
            }),
        ])
    }

    pub(crate) fn enter_plays(&self) -> Option<u64> {
        if self.query.trim().is_empty() {
            return self.opened;
        }
        let on_the_wall = |id: u64| {
            self.visible
                .iter()
                .filter_map(|index| self.albums.get(*index))
                .any(|album| album.id == id)
        };
        vm::top_match(&self.library, &self.query)
            .filter(|id| on_the_wall(*id))
            .or_else(|| {
                self.visible
                    .first()
                    .and_then(|index| self.albums.get(*index))
                    .map(|album| album.id)
            })
    }

    /// **Type anywhere**: append what a key produced to the query, filter, and
    /// put the caret in the well (ADR-0017 §1.2, [`Message::QueryTyped`]).
    ///
    /// The text is *appended*, never assigned: a listener who has already
    /// typed and clicked away mid-query continues it rather than restarting
    /// it, which is what the well itself would do if the caret were still in
    /// it. In practice this runs once and then the well has focus, so it is
    /// the empty-query case almost every time.
    ///
    /// The caret lands at the end of what was typed — `text_input`'s own
    /// `focus` moves the cursor there — so the next keystroke, which the
    /// *field* will handle, continues the word instead of inserting before it.
    pub(crate) fn type_into_query(&mut self, text: &str) -> Task<Message> {
        self.query.push_str(text);
        self.search_selection.clear();
        self.refilter();
        self.search_open = true;
        self.search_scroll_offset = 0.0;
        Task::batch([
            iced::widget::operation::focus(search_id()),
            iced::widget::operation::scroll_to(
                views::search::scroll_id(),
                AbsoluteOffset { x: 0.0, y: 0.0 },
            ),
        ])
    }

    /// <kbd>Esc</kbd> on a non-empty query: give the wall back.
    ///
    /// **Cleared *and* blurred**, which is new with type-anywhere and is the
    /// point of it. Escape used to put the caret back in the well, because a
    /// well you had clicked into was a place you meant to be. Now that any
    /// letter reopens the query from anywhere, holding focus after a clear
    /// would leave the keyboard in an empty field — where <kbd>Space</kbd>
    /// types a space rather than pausing the music — and a listener who
    /// abandoned a search wants the transport back.
    pub(crate) fn clear_query(&mut self) -> Task<Message> {
        self.query.clear();
        self.search_selection.clear();
        self.refilter();
        self.search_open = false;
        self.search_scroll_offset = 0.0;
        Task::batch([
            blur_search(),
            iced::widget::operation::scroll_to(
                views::search::scroll_id(),
                AbsoluteOffset { x: 0.0, y: 0.0 },
            ),
        ])
    }

    /// **Hang the wall at `density`** — the zoom's one effect on the shelf
    /// (ADR-0017 step 6).
    ///
    /// Everything except the geometry survives it, and for the same reason a
    /// re-arrangement is cheap: the density changes how the works are *laid
    /// out* and nothing about which works there are, so the query, the
    /// selection, the edition choices, the thumbnail cache and what is playing
    /// are all untouched. The shelves are not even rebuilt — [`Shelf::shelves`]
    /// derives them from the grid on every call, so the next frame lays out at
    /// the new step by itself.
    ///
    /// **The scroll is anchored rather than reset**, which is the one place
    /// this differs from a key change. A zoom is a request to look at the same
    /// part of the collection more or less closely, so the offset is scaled by
    /// the wall's new height over its old: the record you were looking at is
    /// still under the pointer. (A re-arrangement moves the records themselves
    /// and therefore *must* go back to the top; see [`Self::arrange_by`].)
    ///
    /// The re-anchor is also what makes <kbd>Ctrl</kbd>+scroll behave: iced
    /// 0.13's `scrollable` has no modifier awareness and scrolls whatever the
    /// wheel says, so the notch that asked for a zoom also moves the wall.
    /// Scrolling back to the anchor overrides it in the same frame.
    pub(crate) fn set_density(&mut self, density: shelf::Density) -> Task<Message> {
        if self.density == density {
            return Task::none();
        }
        let was = self.shelves().height();
        let needs_larger_art = density.art_max() > self.density.art_max();
        self.density = density;
        if needs_larger_art {
            // A tighter density deliberately decodes fewer pixels. Moving
            // back to a looser one must not stretch those smaller handles;
            // the prepared disk cache makes refilling this bounded LRU cheap.
            self.thumbs.clear_handles();
            self.thumb_px.clear();
            self.last_requested = None;
        }
        let now = self.shelves().height();
        let anchored = if was > 0.0 {
            (self.scroll_offset * now / was).max(0.0)
        } else {
            0.0
        };
        self.scroll_offset = anchored;
        persist_density(density);
        Task::batch([
            iced::widget::operation::scroll_to(
                scroll_id(),
                AbsoluteOffset {
                    x: 0.0,
                    y: anchored,
                },
            ),
            self.request_visible_thumbs(),
        ])
    }

    /// **Escape, on the wall: peel one layer, top down.**
    ///
    /// The tail of [`crate::app::App::escape`]'s peel — everything under the popover and
    /// under the Settings place is this screen's, and this is the order it goes
    /// in. Each press takes exactly one thing off, and each early return is one
    /// press.
    ///
    /// The **query**, and since 2026-08-10 that is the whole of it. The layer
    /// under it was the shuffle pool's marks, and it went when shuffle stopped
    /// being a draw from the wall and became a property of the player: there is
    /// no pool on the wall to peel. Escape never stopped the music before and
    /// still does not.
    ///
    /// The query step **clears and blurs**, which is type-anywhere's doing
    /// (ADR-0017 step 11) — see [`Self::clear_query`] for why holding the caret
    /// stopped being right once any letter could reopen the query.
    fn peel(&mut self) -> Task<Message> {
        if !self.query.is_empty() {
            return self.clear_query();
        }
        Task::none()
    }

    /// **Arrange the wall by `key`** — the top bar's row of words and `1`–`6`
    /// both land here.
    ///
    /// Re-arranging is a *projection*, never a filter: every album is still
    /// there, in a different order under different headers (ADR-0019 §1). So
    /// the query, the selection, the edition choices, the thumbnail cache and
    /// what is playing are all untouched — an album's id does not depend on the
    /// key, which is what makes this cheap and what makes it safe.
    ///
    /// The wall does go back to the top, and that is the one thing that is
    /// *not* preserved. It is a deliberate choice rather than an omission:
    /// after a re-arrangement the record you were looking at is somewhere else
    /// entirely, so holding the scroll offset would drop you into an unrelated
    /// part of the collection while claiming nothing had moved.
    fn arrange_by(&mut self, key: GroupKey) -> Task<Message> {
        if self.group_key == key {
            return Task::none();
        }
        self.group_key = key;
        self.rebuild_shelves();
        self.scroll_offset = 0.0;
        persist_group_key(key);
        Task::batch([
            iced::widget::operation::scroll_to(scroll_id(), AbsoluteOffset { x: 0.0, y: 0.0 }),
            self.request_visible_thumbs(),
        ])
    }

    /// **The All songs list, resolved from this wall** (`crate::implicit`).
    ///
    /// Built on demand rather than held, because the list *is* the wall and the
    /// wall is recomputed: an implicit playlist that cached itself would be a
    /// snapshot claiming to be a view, and would go stale the moment a query
    /// was typed. It costs one pass over `visible`, and is asked for only when
    /// something is about to be played or drawn.
    pub(crate) fn all_songs(&self) -> crate::implicit::ImplicitList {
        crate::implicit::ImplicitList::all_songs(&self.albums, &self.visible, |id| {
            self.edition_choice.get(&id).copied()
        })
    }

    /// **All songs over the whole library**, whatever the wall is filtered to —
    /// what Home's tile draws and plays (`crate::implicit`).
    ///
    /// [`Self::all_songs`]'s sibling and not its variant: same origin, same
    /// name, same sleeve, different scope. `ImplicitList::everything` carries
    /// the argument for why Home's scope is the collection rather than the
    /// wall's, and it is short — Home shows no wall and no query, so a filter
    /// set on another page has nothing on screen to be read from.
    pub(crate) fn everything(&self) -> crate::implicit::ImplicitList {
        crate::implicit::ImplicitList::everything(&self.albums, |id| {
            self.edition_choice.get(&id).copied()
        })
    }

    /// One artist's chronological implicit `All songs` list, resolved from
    /// the same records and edition choices their page draws.
    pub(crate) fn artist_songs(&self, artist: u64) -> Option<crate::implicit::ImplicitList> {
        let name = crate::views::artist::label(self, artist)?;
        Some(crate::implicit::ImplicitList::artist(
            &self.albums,
            artist,
            name,
            |id| self.edition_choice.get(&id).copied(),
        ))
    }

    /// Records carrying this artist as a track credit while filed under a
    /// different album artist, in the wall's current order.
    pub(crate) fn artist_also_on(&self, artist: u64) -> Vec<&vm::AlbumVm> {
        self.artist_also_on
            .get(&artist)
            .into_iter()
            .flatten()
            .filter_map(|index| self.albums.get(*index))
            .collect()
    }

    /// Put a shelf at the top of the wall — what an index-rail entry does.
    ///
    /// It jumps to the shelf's **header band**, not to its first row: landing
    /// on a shelf has to land you on the thing that names it, and one `HANG`
    /// of clear wall above the covers is the difference between arriving
    /// somewhere and arriving mid-shelf.
    fn jump_to_shelf(&mut self, run: usize) -> Task<Message> {
        let shelves = self.shelves();
        let Some(target) = shelves.runs().get(run) else {
            return Task::none();
        };
        self.scroll_offset = target.top;
        Task::batch([
            iced::widget::operation::scroll_to(
                scroll_id(),
                AbsoluteOffset {
                    x: 0.0,
                    y: target.top,
                },
            ),
            self.request_visible_thumbs(),
        ])
    }

    /// **Re-hang the wall after the lane changed width** — the one re-hang
    /// the product permits, and the whole of what makes it safe.
    ///
    /// Two things happen, in this order. The viewport estimate is corrected
    /// (the next `Scrolled` will report the real bounds), and then **the wall
    /// scrolls so the shelf that was at the top of the viewport is still at
    /// the top**. Not the pixel offset: the columns changed, so every row
    /// moved, and a preserved offset would land on a different shelf. The
    /// machinery is [`shelf::Shelves::run_at`], which already maps an offset onto the
    /// run it is inside, and [`Self::jump_to_shelf`], which is what the index
    /// rail spends.
    ///
    /// The last-opened record's 2 px rule is drawn from data rather than from
    /// geometry, so it is still on the right tile afterwards — which is the
    /// anchor the eye actually uses.
    pub(crate) fn rehang(&mut self) -> Task<Message> {
        let here = self.shelves().run_at(self.scroll_offset);
        self.grid_size = Size::new(self.grid_width(), self.grid_size.height);
        if let Some(run) = here {
            return self.jump_to_shelf(run);
        }
        // Above the first shelf — the top of the wall stays the top.
        self.scroll_offset = 0.0;
        Task::batch([
            iced::widget::operation::scroll_to(scroll_id(), AbsoluteOffset { x: 0.0, y: 0.0 }),
            self.request_visible_thumbs(),
        ])
    }

    /// **The grid's width**: the window's, less the returns lane and the index
    /// rail's lane.
    ///
    /// The two are different numbers and the difference is the rail's
    /// ([`theme::INDEX_LANE_W`]): the wall is what the shelf column occupies and
    /// the grid is what the covers hang in.
    ///
    /// It used to have a third term — whatever the album inspector was taking
    /// at this instant — and losing it is the plainest thing ADR-0022 did to
    /// this file: **the wall's width is now a property of the window and
    /// nothing else**, so no press anywhere in the product can re-hang the
    /// collection. The `reflow`, the width tween, the panel's lagging album and
    /// the double-click's grid hold all existed to make a re-hang survivable;
    /// none of them has anything left to do.
    fn grid_width(&self) -> f32 {
        (self.window_w
            - theme::sidebar_w(self.window_w, self.lane_open)
            - theme::INDEX_LANE_W
            - theme::WALL_SCROLLBAR_W)
            .max(0.0)
    }

    /// **The width the place's own body gets**: the window, less the returns
    /// lane.
    ///
    /// The strip, the place headers and every breakpoint inside a place read
    /// this rather than the window: the lane is a *column*, so a body that
    /// resolved its two-line split against the window would split at the wrong
    /// moment and hang its content off a line that is no longer there.
    pub(crate) fn body_width(&self) -> f32 {
        (self.window_w - theme::sidebar_w(self.window_w, self.lane_open)).max(0.0)
    }

    /// Advance the shelf's own transitions.
    pub(crate) fn tick_motion(&mut self, now: Instant) -> Task<Message> {
        self.tile_hover.tick(now);
        if !self.art_dissolve.tick(now) {
            // **A settled dissolve holds no picture.** The same rule
            // [`Keyed::tick`] follows when it drops its key: a transition at
            // rest must not keep a reference to the thing it moved, or the
            // outgoing hero's 4 MiB would outlive the 200 ms that needed it and
            // the LRU's budget would be a fiction.
            self.art_prior = None;
        }
        Task::none()
    }

    /// Whether the shelf still needs a clock (see [`crate::app::App::moving`]).
    pub(crate) fn moving(&self) -> bool {
        self.tile_hover.live() || self.art_dissolve.live()
    }

    /// **Commit what the Now playing place draws of the record, and start the
    /// dissolve when the picture — not the track — has changed** (ADR-0020's
    /// third amendment; the owner, 2026-08-10: *"when changing track there
    /// isn't any kind of nice visual transition for album art in now playing.
    /// we should have something a bit nicer, like a quick fade"*).
    ///
    /// Called after **every** message, for [`crate::app::App::request_hero`]'s reason and
    /// at its cost: the two moments that can change this surface's artwork are
    /// the engine naming another record and a hero decode landing, and asking
    /// on both by asking always is one call site instead of a list that has to
    /// stay complete. At rest it is an `Option` compare.
    ///
    /// # The three rules, and each is a defect avoided
    ///
    /// 1. **The picture, never the track.** Consecutive tracks on one record
    ///    share a cover, and the first line out of this function is the record
    ///    the surface is already committed to — so a twelve-track album is
    ///    *twelve* track changes and **no** transition, no clock and no frame.
    ///    Where two records genuinely differ, the predicate is still the
    ///    picture: [`Change::between`] compares the handles being drawn.
    /// 2. **The new art, not the new track.** A record whose hero is still
    ///    decoding has **no answer**, and this returns without touching
    ///    anything — the surface goes on drawing the picture it has. Starting
    ///    on `TrackStarted` instead would dissolve to whatever was ready, which
    ///    is a 320 px thumbnail or nothing at all, and then pop when the hero
    ///    landed: worse than the cut it replaces, twice over.
    /// 3. **Two pictures, or no transition.** A record with no art draws the
    ///    wall's deterministic gradient, which is a *stand-in* rather than
    ///    artwork; dissolving one is decoration, and ADR-0020 §3 forbids that.
    ///    So art → no art, and no art → art, stay the hard cuts they are today.
    /// 4. **`watching` — the surface is on screen.** The commitment is
    ///    unconditional, so opening the place finds the right picture whenever
    ///    the record changed; the *tween* is not, because a clock easing a hero
    ///    nobody is looking at would redraw whatever place is on screen a dozen
    ///    times for nothing. See [`crate::app::App::settle_art`] for why this differs from
    ///    [`Self::request_hero`], which is ungated on purpose.
    ///
    /// # The second LRU entry, checked rather than trusted
    ///
    /// This needs both pictures alive at once and adds no cache to get them.
    /// [`art::HERO_CACHE_ENTRIES`] is 2 and [`Self::request_hero`] `get`s the
    /// *sounding* record — so when the incoming hero is `put`, the entry it
    /// would evict is the third-oldest and there is no third: the record that
    /// just stopped is still decoded, and `art_prior`'s handle is an `Arc` onto
    /// those same pixels rather than a copy of them. Asserted rather than
    /// assumed, because the entry that makes it true was written for a prefetch
    /// this product does not have yet:
    /// `the_hero_lru_holds_both_records_a_dissolve_needs`.
    pub(crate) fn settle_art(&mut self, sounding: Option<u64>, watching: bool, now: Instant) {
        let Some(id) = sounding else {
            // Nothing is sounding: there is no record on this surface to draw,
            // so there is nothing to dissolve *to* and the transition is
            // abandoned rather than run out. The light goes out with the music
            // and so does the picture ([`crate::app::App::warm_lamp`]'s own rule).
            self.art_shown = None;
            self.art_prior = None;
            self.art_dissolve.set(1.0);
            return;
        };
        if self
            .art_shown
            .as_ref()
            .is_some_and(|(shown, _)| *shown == id)
        {
            return;
        }
        // **The answer, or nothing at all** — rule 2. `None` here is "the
        // decode has not come back", which is not the same as "there is no
        // art"; the second is an answer and lives in `no_art`.
        let answer = if let Some(hero) = self.hero(id) {
            Some(hero.clone())
        } else if self.no_art.contains(&id) {
            None
        } else {
            return;
        };
        let prior = self.art_shown.take().map(|(_, hero)| hero);
        let change = Change::between(prior.as_ref(), answer.as_ref());
        self.art_shown = answer.map(|hero| (id, hero));
        match change {
            // **Committed, but cut** — the picture changed while the place was
            // not on screen. The surface is correct the moment it is opened and
            // no clock was spent easing something nobody saw.
            Change::Dissolve if !watching => {
                self.art_prior = None;
                self.art_dissolve.set(1.0);
            }
            Change::Dissolve => {
                self.art_prior = prior;
                self.art_dissolve.set(0.0);
                self.art_dissolve.go(1.0, motion::DISSOLVE, now);
            }
            Change::Cut => {
                self.art_prior = None;
                self.art_dissolve.set(1.0);
            }
        }
    }

    /// **What the Now playing place draws of the record, and how far through a
    /// change it is** — one answer, so the cover and the field derived from it
    /// cannot disagree about which record they are of.
    pub(crate) fn showing(&self) -> Showing<'_> {
        Showing {
            hero: self.art_shown.as_ref().map(|(_, hero)| hero),
            from: self.art_prior.as_ref(),
            t: self.art_dissolve.value(),
        }
    }

    /// The hang the grid lays out with: resolved for what the viewport
    /// measures, **less the index rail's lane**.
    ///
    /// One answer, read by the view that draws the rows and by the thumbnail
    /// prefetch that decides which of them to decode art for — a prefetch
    /// working from a different grid than the one on screen would request the
    /// wrong tiles.
    ///
    /// The rail's lane is already off `grid_size`: the wall and the rail are
    /// siblings in one row, so what the scrollable *measures* is the grid's
    /// width and the subtraction happens once, in the layout, rather than at
    /// each reader. `the_hang_holds_with_the_index_rail_taken_off_the_wall`
    /// asserts the hang survives that subtraction at every width in the band.
    pub(crate) fn grid(&self) -> shelf::Grid {
        match self.layout {
            shelf::Layout::Wall => shelf::Grid::new(self.grid_size.width, self.density),
            shelf::Layout::List => shelf::Grid::list(self.grid_size.width, self.density),
        }
    }

    /// How the wall is broken into shelves, for the current filter and grid.
    ///
    /// Rebuilt per call rather than cached: it is one pass over a few dozen
    /// counts, it has to follow the grid (which follows the window, the
    /// inspector and the double-click hold), and a cache of it would be a
    /// fourth thing that could disagree with the other three.
    pub(crate) fn shelves(&self) -> shelf::Shelves {
        shelf::Shelves::new(self.grid(), &self.visible_counts)
    }

    /// Re-ask the library for the wall under the active key, and re-derive
    /// everything that hangs off it.
    ///
    /// **The album ids do not change**, which is what makes re-arranging cheap
    /// and safe: an id is a hash of the (artist, album) pair
    /// ([`vm::album_id`]), so the thumbnail cache, the selection, the playing
    /// album and the edition choices all survive a key change untouched. Only
    /// the order and the breaks are new.
    fn rebuild_shelves(&mut self) {
        let shelves = vm::build_shelves(&self.library, self.group_key, self.history.as_ref());
        self.albums.clear();
        self.groups.clear();
        for shelf in shelves {
            self.albums.extend(shelf.albums);
            self.groups.push(GroupVm {
                header: shelf.header,
                end: self.albums.len(),
            });
        }
        // **Home's figures, counted here and nowhere else.** One pass over the
        // tracks that were just rebuilt, on the same schedule the rebuild runs
        // on — which is what keeps the `COLLECTION` footer off the per-frame
        // path (ADR-0030 §4). The arrangement does not change any of the four,
        // but re-counting is cheaper than reasoning about which caller changed
        // what.
        self.collection = vm::Collection::count(&self.albums, self.library.len());
        (self.artist_facts, self.artist_also_on) = vm::artist_inventory(&self.albums);
        // **The compose field's example, made of their music.** One pass, on
        // the same schedule and for the same reason as the two counts above:
        // it is derived from the tracks that were just rebuilt, and this is
        // the one place they are known to be settled.
        self.vibe.rebuild_example(&self.albums);
        // Rebuilding changes the album behind every virtual position even
        // though app-bar search itself no longer changes the wall.
        self.forget_requested();
        self.refilter();
        // The album ids survive a re-arrangement (see above), so the fold does
        // too — but a *rescan* can add and remove records, and the lane must
        // not go on naming one that is gone.
        if !self.lane_played.is_empty() || self.history.is_some() {
            self.fold_history_onto_records();
        }
    }

    /// **The ledger, folded onto records** — the whole of the lane's reading
    /// of history, and it happens twice in a process: at launch, and whenever
    /// the library itself is rebuilt.
    ///
    /// One pass over every track. That is the cost ADR-0030 §4 budgets and it
    /// is paid where the file is already being read; what the contract forbids
    /// is paying it *per frame*, which is why the result lives in
    /// [`Self::lane_played`] and is thereafter maintained by events.
    fn fold_history_onto_records(&mut self) {
        self.lane_played = match self.history.as_ref() {
            Some(history) => crate::lane::by_record(
                self.albums.iter().flat_map(|album| {
                    album
                        .editions
                        .iter()
                        .flat_map(|edition| edition.tracks.iter())
                        .map(move |track| (album.id, track.path.as_path()))
                }),
                // **The plays that were a *record* being put on**, which is
                // not every play of its tracks (ADR-0034). A run reified from
                // a list touched the list; re-deriving the records from those
                // play lines is exactly the attribution the live fix removes,
                // and doing it here is what made a list played last week come
                // back as its albums. `last_played_unlisted` is
                // `last_played_unix_s` minus those plays — and for a ledger
                // with no markers, which is every ledger written before this
                // shipped, it *is* `last_played_unix_s`.
                |path| history.last_played_unlisted(path),
            ),
            None => HashMap::new(),
        };
        self.rebuild_lane_recent();
    }

    /// The lane's records half, re-resolved from [`Self::lane_played`].
    ///
    /// O(albums) — a sort of the touched ones and a truncation to 24. Called
    /// when the fold is rebuilt and when one play moves one record, which is
    /// exactly the *"a `TrackStarted` updates one entry and re-sorts 24"* the
    /// contract promises.
    fn rebuild_lane_recent(&mut self) {
        let touched: Vec<crate::lane::Touched> = self
            .albums
            .iter()
            .filter_map(|album| {
                let at = self.lane_played.get(&album.id).copied()?;
                Some(crate::lane::Touched {
                    subject: crate::lane::Subject::Record(album.id),
                    // The wall's own two lines, verbatim — a record must not
                    // be named one thing on a tile and another in the lane.
                    name: album
                        .title
                        .clone()
                        .unwrap_or_else(|| "Unknown Album".to_owned()),
                    under: album.artist.label().to_owned(),
                    at: Some(at),
                })
            })
            .collect();
        self.lane_recent = crate::lane::recent(touched);
        self.lane_stamp = self.lane_stamp.wrapping_add(1);
    }

    /// A play was recorded: the record it belongs to is now the most recently
    /// touched thing in the lane.
    ///
    /// The moment is *now* rather than the ledger's, because the ledger is a
    /// snapshot read at launch and re-reading it here would be the per-frame
    /// file read the contract refuses. The two agree to within the length of
    /// the play.
    pub(crate) fn record_played(&mut self, path: &std::path::Path, at: u64) {
        let Some(album) = self.albums.iter().find(|album| {
            album
                .editions
                .iter()
                .any(|edition| edition.tracks.iter().any(|track| track.path == path))
        }) else {
            return;
        };
        let id = album.id;
        if self.lane_played.insert(id, at) == Some(at) {
            return;
        }
        self.rebuild_lane_recent();
    }

    /// Keep the wall's complete projection and rebuild the two relevance-
    /// ordered app-bar result sets for the current query.
    pub(crate) fn refilter(&mut self) {
        // Search no longer filters or replaces the Library body: the current
        // place remains unchanged under the app-wide dropover. The wall's
        // projection is therefore always the complete arranged collection.
        self.visible = (0..self.albums.len()).collect();
        // The dropover's two relevance-ordered projections. SEARCH_LIMIT is a
        // work bound, not an eight-row presentation cap; the view virtualizes
        // this result set into one scroll surface.
        self.songs = vm::song_hits(&self.library, &self.query, vm::SEARCH_LIMIT);
        self.search_albums = vm::album_hits(&self.library, &self.query, vm::SEARCH_LIMIT);
        // **Playlists are searchable** (the owner, 2026-08-18: *"playlist
        // filter could simply be the search being updated to show playlists
        // if it doesn't"*). They were deliberately outside the corpus —
        // ADR-0024 §A2 deferred wall membership, rail sorting and
        // search-corpus membership together — which meant a listener with
        // forty lists had no way to find one by name.
        //
        // Matched with `baz_core::index::search_fold` rather than a plain
        // lowercase, so a list called `Bells & Whistles` answers *bells and*
        // for the same reason a track does.
        self.search_playlists = if self.query.trim().is_empty() {
            Vec::new()
        } else {
            let needle = baz_core::index::search_fold(self.query.trim());
            self.playlist_names
                .iter()
                .filter(|(_, name)| baz_core::index::search_fold(name).contains(&needle))
                .map(|(id, _)| *id)
                .take(vm::SEARCH_LIMIT)
                .collect()
        };
        self.search_action = crate::search::Action::Play;
        // The shelves are contiguous slices of `albums` and `visible` is in
        // the same order, so each shelf's surviving count is one walk of the
        // two lists together rather than a second filter that could disagree
        // with the first.
        self.visible_counts = surviving_per_shelf(&self.visible, &self.groups);
    }

    /// Answer a message that only the folders baz holds care about, reporting
    /// whether it was one (ADR-0022).
    ///
    /// Every one of them ends in the same two acts — the list moves, and a scan
    /// starts or does not — so they are one machine rather than six arms
    /// scattered through the shelf's own.
    fn update_library(&mut self, message: &Message) -> Option<Task<Message>> {
        match message {
            // The periodic refresh. The clock says whether it is due; a pass
            // already running always says no.
            Message::RefreshTick => {
                if self.refresh.due(Instant::now(), self.scanning) {
                    crate::baz_log!("[scan] periodic refresh");
                    self.start_scan(scan::ScanMode::Incremental);
                }
            }
            Message::MusicFolderInput(value) => {
                self.folder_input.clone_from(value);
                self.folder_error = None;
            }
            Message::AddMusicFolder => return Some(self.submit_folder_input()),
            Message::PickMusicFolder => return Some(pick_folder()),
            Message::MusicFolderPicked(choice) => {
                return Some(self.folder_picked(choice.clone()));
            }
            Message::MusicFolderChecked(result) => {
                return Some(self.folder_checked(result.clone()));
            }
            // The first press arms; the second acts. See
            // `views::settings::folder_block` for why it is two.
            Message::ConfirmRemoveMusicFolder(index) => {
                self.folder_pending_removal = Some(*index);
            }
            Message::CancelRemoveMusicFolder => self.folder_pending_removal = None,
            Message::RemoveMusicFolder(index) => return Some(self.remove_root(*index)),
            Message::MoveMusicFolderUp(index) => self.move_root(*index, -1),
            Message::MoveMusicFolderDown(index) => self.move_root(*index, 1),
            Message::ConfirmPruneMissing => self.prune_pending = true,
            Message::CancelPruneMissing => self.prune_pending = false,
            Message::PruneMissing => return Some(self.prune_missing()),
            Message::ConfirmPruneUnrooted => self.unrooted_pending = true,
            Message::CancelPruneUnrooted => self.unrooted_pending = false,
            Message::PruneUnrooted => return Some(self.prune_unrooted()),
            Message::ForceSync => {
                if !self.scanning {
                    crate::baz_log!("[scan] force sync requested");
                    self.start_scan(scan::ScanMode::Force);
                }
            }
            _ => return None,
        }
        Some(Task::none())
    }

    /// What the Settings place's Library section draws (ADR-0022).
    ///
    /// A projection built here rather than in the view, because it is the join
    /// of two things this struct holds: the folders the shell is scanning, and
    /// what the index records under each of them.
    pub(crate) fn library_view<'a>(
        &'a self,
        playlists: Option<&'a std::path::Path>,
    ) -> views::settings::LibraryView<'a> {
        views::settings::LibraryView {
            folders: self
                .roots
                .iter()
                .map(|root| {
                    let stats = self.library.root_stats(root);
                    views::settings::FolderRow {
                        path: root.clone(),
                        tracks: stats.tracks,
                        last_scan_ns: stats.last_scan_ns,
                        unavailable: self.unavailable.contains(root),
                    }
                })
                .collect(),
            input: &self.folder_input,
            error: self.folder_error.as_deref(),
            pending_removal: self.folder_pending_removal,
            scanning: self.scanning,
            unrooted: self.library.unrooted_paths(),
            unrooted_pending: self.unrooted_pending,
            playlists,
            prunable: &self.prunable,
            prune_pending: self.prune_pending,
            now_ns: now_ns(),
        }
    }

    /// Send the typed path off to be looked at, coming back as
    /// [`Message::MusicFolderChecked`].
    ///
    /// The look itself — one `stat` — happens on the blocking pool, because the
    /// paths people type here are exactly the ones a dialog cannot offer: the
    /// share that is configured but not mounted, the drive that is sometimes
    /// plugged in. Against a dead hard mount that `stat` can sit for minutes,
    /// and it used to sit on the UI thread.
    fn submit_folder_input(&mut self) -> Task<Message> {
        let dir = expand_tilde(self.folder_input.trim());
        if dir.as_os_str().is_empty() {
            return Task::none();
        }
        Task::perform(check_folder(dir), Message::MusicFolderChecked)
    }

    /// What the folder picker's closing means: a chosen folder joins the
    /// list; a dismissal is not a decision and touches nothing.
    ///
    /// A picked folder skips the `stat` the typed door needs — the dialog
    /// walked the real filesystem to offer it, which is better evidence than a
    /// fresh stat, and re-checking would put an avoidable filesystem wait back
    /// on this thread.
    fn folder_picked(&mut self, choice: Option<PathBuf>) -> Task<Message> {
        match choice {
            None => Task::none(),
            Some(dir) => self.accept_folder(dir),
        }
    }

    /// What the off-thread look at a typed path came back to: the same words
    /// the first-run screen uses when the path is not a directory, or the
    /// acceptance every added folder goes through.
    fn folder_checked(&mut self, result: Result<PathBuf, String>) -> Task<Message> {
        match result {
            Ok(dir) => {
                let task = self.accept_folder(dir);
                // The field empties only when its path was taken. A refused
                // path (already here) stays put to be corrected, rather than
                // making somebody retype the long half of a NAS path.
                if self.folder_error.is_none() {
                    self.folder_input.clear();
                }
                task
            }
            Err(reason) => {
                self.folder_error = Some(reason);
                Task::none()
            }
        }
    }

    /// Hold `dir`, remember it, and scan it — the one acceptance path both
    /// doors (the typed path and the picker) land in.
    ///
    /// A folder already held is refused rather than added twice — it would be
    /// walked twice for one set of rows ([`folder_refusal`] holds the words).
    fn accept_folder(&mut self, dir: PathBuf) -> Task<Message> {
        if let Some(refusal) = folder_refusal(&self.roots, &dir) {
            self.folder_error = Some(refusal);
            return Task::none();
        }
        self.folder_error = None;
        self.folder_pending_removal = None;
        // A folder added now may hold rows an older baz left rootless — the
        // pre-v8 population this is the only cure for. Claim them before the
        // scan, so the walk that follows can prune them if they are gone.
        adopt_roots(&mut self.library, std::slice::from_ref(&dir));
        crate::baz_log!("[config] holding {}", dir.display());
        self.roots.push(dir);
        persist_roots(&self.roots);
        // Incremental, not forced: a folder that overlaps one baz already holds
        // must not cost a re-read of every file in it.
        self.start_scan(scan::ScanMode::Incremental);
        Task::none()
    }

    fn move_root(&mut self, index: usize, delta: i8) {
        if self.folder_pending_removal.is_some() {
            return;
        }
        let Some(to) = shifted_index(self.roots.len(), index, delta) else {
            return;
        };
        self.roots.swap(index, to);
        persist_roots(&self.roots);
        crate::baz_log!(
            "[config] music folder moved from {} to {}",
            index + 1,
            to + 1
        );
        self.retarget_scan();
    }

    /// **A scan in flight is walking the old list**, so an edit to the list
    /// has to hand it the new one.
    ///
    /// The owner, 2026-08-22: *"I can't seem to remove a music library… or
    /// rather remove a drive."* Every folder control used to be dead while a
    /// scan ran, and the folder somebody most wants gone — a drive that has
    /// gone away — is exactly the one that keeps a scan up. So the controls
    /// stay live and the scan is re-aimed instead.
    ///
    /// Restarting is the whole mechanism: [`Self::start_scan`] replaces
    /// `scan_rx`, and the walker treats the dropped receiver as `Walk::Stopped`
    /// — *"UI hung up; prune nothing"* — so the old worker retires at its next
    /// send rather than racing the new one into the index. `Incremental` is
    /// the mode because nothing about the files changed; only which folders
    /// baz holds did, which is the same reason adding one starts this scan and
    /// not a force sync.
    ///
    /// **Only when one was already running.** Editing the list in a quiet
    /// moment must not start a scan nobody asked for.
    fn retarget_scan(&mut self) {
        if self.scanning {
            crate::baz_log!("[scan] folder list edited; re-aiming the scan in flight");
            self.start_scan(scan::ScanMode::Incremental);
        }
    }

    /// Stop holding a folder, and **forget its tracks** (ADR-0022 §4).
    ///
    /// Nothing on disk is touched. What goes is the index's record of the
    /// folder: its rows, and its scan time. The argument for forgetting rather
    /// than keeping is in the ADR and in `Library::forget_root` — in short, a
    /// folder baz no longer holds is one baz can no longer refresh, so keeping
    /// its albums would leave a listener with rows nothing can ever correct or
    /// remove.
    fn remove_root(&mut self, index: usize) -> Task<Message> {
        self.folder_pending_removal = None;
        if index >= self.roots.len() {
            return Task::none();
        }
        let root = self.roots.remove(index);
        self.unavailable.remove(&root);
        persist_roots(&self.roots);
        match self.library.forget_root(&root) {
            Ok(count) => {
                crate::baz_log!("[index] {count} tracks forgotten with {}", root.display());
            }
            Err(error) => {
                crate::baz_log!("[index] could not forget {}: {error}", root.display());
                self.problem = Some(format!("could not forget that folder: {error}"));
            }
        }
        // The wall's mark and the art caches are keyed by album id, and the
        // albums a forgotten folder held are gone — so the rebuild has to be
        // followed by the same clean-up a finished scan does.
        self.opened = None;
        self.no_art.clear();
        self.no_artist_image.clear();
        self.rebuild_shelves();
        self.retarget_scan();
        self.request_visible_thumbs()
    }

    /// Forget the exact missing-path preview the last completed scan produced.
    /// `forget_paths` is one transactional source of truth with folder removal
    /// and preserves first-seen tombstones, so a mistaken confirmation is
    /// repaired by bringing the files back and scanning again.
    fn prune_missing(&mut self) -> Task<Message> {
        self.prune_pending = false;
        if self.prunable.is_empty() {
            return Task::none();
        }
        match self.library.forget_paths(&self.prunable) {
            Ok(count) => {
                crate::baz_log!("[index] {count} confirmed missing tracks forgotten");
                self.health.record(
                    crate::health::Level::Ready,
                    "Missing albums pruned",
                    format!(
                        "{count} index entries removed. Audio, playlists and listening history were untouched."
                    ),
                );
                self.prunable.clear();
                self.opened = None;
                self.no_art.clear();
                self.no_artist_image.clear();
                self.rebuild_shelves();
                self.request_visible_thumbs()
            }
            Err(error) => {
                self.problem = Some(format!("could not prune missing albums: {error}"));
                self.health.record(
                    crate::health::Level::Error,
                    "Could not prune missing albums",
                    error.to_string(),
                );
                Task::none()
            }
        }
    }

    /// Forget rootless legacy rows after showing every path. Unlike a scan,
    /// this is an explicit listener decision, so it can safely address rows
    /// no configured root is able to prove absent.
    fn prune_unrooted(&mut self) -> Task<Message> {
        self.unrooted_pending = false;
        let paths = self.library.unrooted_paths();
        if paths.is_empty() {
            return Task::none();
        }
        match self.library.forget_paths(&paths) {
            Ok(count) => {
                crate::baz_log!("[index] {count} rootless legacy tracks forgotten");
                self.health.record(
                    crate::health::Level::Ready,
                    "Unheld tracks removed from the index",
                    format!(
                        "{count} index entries removed. Audio, playlists and listening history were untouched."
                    ),
                );
                self.opened = None;
                self.no_art.clear();
                self.no_artist_image.clear();
                self.rebuild_shelves();
                self.request_visible_thumbs()
            }
            Err(error) => {
                self.problem = Some(format!("could not prune unheld tracks: {error}"));
                self.health.record(
                    crate::health::Level::Error,
                    "Could not remove unheld tracks",
                    error.to_string(),
                );
                Task::none()
            }
        }
    }

    /// Start a scan of every folder baz holds, in `mode`, replacing whatever
    /// pass was running.
    ///
    /// The refresh clock is restarted here rather than only on completion, so
    /// that a force sync or a newly added folder also pushes the automatic
    /// rescan out — a listener who has just refreshed does not need baz to do
    /// it again in ten seconds.
    pub(crate) fn start_scan(&mut self, mode: scan::ScanMode) {
        self.unavailable.clear();
        if self
            .problem
            .as_deref()
            .is_some_and(|problem| problem.contains("not reachable"))
        {
            self.problem = None;
        }
        self.files_skipped = 0;
        self.refresh.restarted(Instant::now());
        if self.roots.is_empty() {
            self.scan_rx = None;
            self.scanning = false;
            return;
        }
        self.health.record(
            crate::health::Level::Working,
            "Library scan started",
            format!("Checking {} configured folders", self.roots.len()),
        );
        self.scan_rx = Some(scan::spawn(
            self.roots.clone(),
            self.library.known_files(),
            mode,
        ));
        self.scanning = true;
    }

    /// Apply every pending scan update: one `add_tracks` + one view-model
    /// rebuild per tick regardless of how many batches arrived.
    fn drain_scan(&mut self) -> Task<Message> {
        let Some(drained) = self.collect_scan() else {
            return Task::none();
        };
        let Drained {
            fresh_tracks,
            vanished,
            prunable,
            scanned,
            missing,
            finished,
        } = drained;
        self.apply_scan(
            fresh_tracks,
            &vanished,
            prunable,
            scanned,
            missing,
            finished,
        )
    }

    /// Take everything the worker has said since the last tick, without
    /// touching the index — the receiving half of [`Shelf::drain_scan`].
    #[expect(
        clippy::too_many_lines,
        reason = "one receiver drain deliberately keeps every scan-worker message's state transition together"
    )]
    fn collect_scan(&mut self) -> Option<Drained> {
        let rx = self.scan_rx.as_ref()?;
        // Batches are kept per root, because the root is what makes the write
        // an `add_tracks_under`: it is the fact removal's second gate will read
        // back. A tick usually holds one root's worth; a small library can hold
        // several, and the order is the order they arrived in.
        let mut fresh_tracks: Vec<(PathBuf, Vec<baz_core::library::TrackMeta>)> = Vec::new();
        let mut vanished: Vec<std::path::PathBuf> = Vec::new();
        let mut prunable: Vec<std::path::PathBuf> = Vec::new();
        let mut scanned: Vec<(PathBuf, i64)> = Vec::new();
        let mut missing: Vec<(PathBuf, String)> = Vec::new();
        let mut finished = false;
        loop {
            match rx.try_recv() {
                Ok(ScanUpdate::Batch {
                    root,
                    tracks,
                    failed,
                    failures,
                }) => {
                    self.files_skipped += failed;
                    for (path, reason) in failures {
                        self.health.record(
                            crate::health::Level::Warning,
                            "File skipped",
                            format!("{}\n{reason}", path.display()),
                        );
                    }
                    match fresh_tracks.last_mut() {
                        Some((held, batch)) if *held == root => batch.extend(tracks),
                        _ => fresh_tracks.push((root, tracks)),
                    }
                }
                Ok(ScanUpdate::Removed { paths }) => vanished.extend(paths),
                Ok(ScanUpdate::Prunable { paths }) => prunable.extend(paths),
                Ok(ScanUpdate::RootDone {
                    root,
                    at_ns,
                    added,
                    updated,
                    unchanged,
                    failed,
                }) => {
                    record_root_scan(&mut self.health, &root, [added, updated, unchanged, failed]);
                    scanned.push((root, at_ns));
                }
                Ok(ScanUpdate::RootUnavailable { root, reason }) => missing.push((root, reason)),
                Ok(ScanUpdate::Done {
                    added,
                    updated,
                    unchanged,
                    removed,
                    failed,
                    unavailable,
                    elapsed,
                }) => {
                    let secs = elapsed.as_secs_f64();
                    let read = added + updated;
                    #[expect(
                        clippy::cast_precision_loss,
                        reason = "track counts are far below f64's exact-integer range"
                    )]
                    let rate = if secs > 0.0 { read as f64 / secs } else { 0.0 };
                    crate::baz_log!(
                        "[scan] done: {added} added, {updated} updated, {unchanged} unchanged, \
                         {removed} removed, {failed} files skipped, \
                         {unavailable} folders unavailable, {secs:.1} s ({rate:.0} tracks/s)"
                    );
                    self.health.record(
                        if failed > 0 || unavailable > 0 {
                            crate::health::Level::Warning
                        } else {
                            crate::health::Level::Ready
                        },
                        "Library scan complete",
                        format!(
                            "{added} added · {updated} updated · {removed} removed · \
                             {failed} files skipped · {unavailable} folders unavailable"
                        ),
                    );
                    finished = true;
                    break;
                }
                Ok(ScanUpdate::Error(error)) => {
                    crate::baz_log!("[scan] failed to start: {error}");
                    self.problem = Some(format!("scan failed: {error}"));
                    self.health
                        .record(crate::health::Level::Error, "Library scan failed", error);
                    finished = true;
                    break;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.health.record(
                        crate::health::Level::Error,
                        "Library scan stopped",
                        "The scan worker disconnected before reporting completion",
                    );
                    self.problem = Some("scan stopped unexpectedly".to_owned());
                    finished = true;
                    break;
                }
            }
        }
        Some(Drained {
            fresh_tracks,
            vanished,
            prunable,
            scanned,
            missing,
            finished,
        })
    }

    /// Write what one tick's worth of scan updates said, rebuild the shelf, and
    /// report the folders that were not there — the applying half of
    /// [`Shelf::drain_scan`].
    fn apply_scan(
        &mut self,
        fresh_tracks: Vec<(PathBuf, Vec<baz_core::library::TrackMeta>)>,
        vanished: &[PathBuf],
        prunable: Vec<PathBuf>,
        scanned: Vec<(PathBuf, i64)>,
        missing: Vec<(PathBuf, String)>,
        finished: bool,
    ) -> Task<Message> {
        if !prunable.is_empty() || finished {
            self.prunable = prunable;
            self.prune_pending = false;
        }
        // A folder that is not reachable right now: never a scan failure — the
        // pass carried on and pruned nothing from it (ADR-0022 §2).
        //
        // The status line gets a **count**, not a path, and that is a frame
        // constraint rather than terseness: the top bar's note is a single
        // unwrapped line sharing its row with the counts and `Settings`, and a
        // message carrying `/mnt/nas/Music/Archive` wraps it to two and pushes
        // `Settings` off the strip. Which folder it was, and that nothing was
        // removed from it, is said per folder in the Settings place — where
        // there is room to say it properly.
        let absent = missing.len();
        for (root, reason) in missing {
            crate::baz_log!("[scan] {} is unavailable: {reason}", root.display());
            self.health.record(
                crate::health::Level::Warning,
                "Folder unavailable",
                format!(
                    "{}\n{reason}\nThe existing library entries were kept.",
                    root.display()
                ),
            );
            self.unavailable.insert(root);
        }
        if absent == 1 {
            self.problem = Some("1 folder is not reachable".to_owned());
        } else if absent > 1 {
            self.problem = Some(format!("{absent} folders are not reachable"));
        }
        // When a folder's walk finished, so the Settings place can say when baz
        // last looked at it.
        for (root, at_ns) in scanned {
            if let Err(error) = self.library.record_scan(&root, at_ns) {
                crate::baz_log!(
                    "[index] could not record the scan of {}: {error}",
                    root.display()
                );
                self.health.record(
                    crate::health::Level::Error,
                    "Could not record scan time",
                    format!("{}\n{error}", root.display()),
                );
            }
        }

        let mut task = Task::none();
        if !fresh_tracks.is_empty() || !vanished.is_empty() {
            for (root, tracks) in fresh_tracks {
                if let Err(error) = self.library.add_tracks_under(Some(&root), tracks) {
                    crate::baz_log!("[index] write failed: {error}");
                    self.problem = Some(format!("library write failed: {error}"));
                    self.health.record(
                        crate::health::Level::Error,
                        "Library write failed",
                        error.to_string(),
                    );
                }
            }
            if !vanished.is_empty() {
                match self.library.remove_tracks(vanished) {
                    Ok(count) => crate::baz_log!("[index] {count} vanished tracks removed"),
                    Err(error) => {
                        crate::baz_log!("[index] removal failed: {error}");
                        self.problem = Some(format!("library removal failed: {error}"));
                        self.health.record(
                            crate::health::Level::Error,
                            "Library removal failed",
                            error.to_string(),
                        );
                    }
                }
            }
            self.rebuild_shelves();
            if self.last_scan_log.elapsed() > Duration::from_secs(2) {
                self.last_scan_log = Instant::now();
                crate::baz_log!(
                    "[scan] {} tracks / {} albums so far…",
                    self.library.len(),
                    self.albums.len()
                );
            }
            task = self.request_visible_thumbs();
        }
        if finished {
            self.scanning = false;
            self.scan_rx = None;
            // The periodic refresh is a gap *between* passes: the clock starts
            // when this one finishes, not when the next one is wanted.
            self.refresh.restarted(Instant::now());
            // Early albums may have gained art (late tracks, cover files
            // written mid-scan): allow one clean retry pass.
            self.no_art.clear();
            self.no_artist_image.clear();
            task = Task::batch([task, self.request_visible_thumbs()]);
        }
        task
    }

    /// Drop the range guard in [`Shelf::request_visible_thumbs`].
    ///
    /// Called wherever *which* albums a range names has changed — a new
    /// filter, a new arrangement, a scan that added records. The guard is an
    /// answer cached against a question about positions, and these are the
    /// events that change what a position means.
    pub(crate) fn forget_requested(&mut self) {
        self.last_requested = None;
    }

    /// Kick off off-thread decodes for every visible tile whose thumbnail is
    /// neither cached, in flight, nor known-absent. Ported from the spike;
    /// `get` (not `peek`) refreshes LRU recency for visible entries.
    /// **Decode art for records that are on screen but not on the wall** —
    /// the returns lane's rows and the Home place's `RECENTLY ADDED` row.
    ///
    /// The wall's own prefetch is a range over the *visible slice of the
    /// wall*, which is the right guard for the wall and answers nothing about
    /// a record drawn beside it: a recently-added record two thousand rows
    /// down, or a lane row for something played last week, is on screen with
    /// its decode never asked for, and falls back to the gradient forever.
    ///
    /// The same decode path, the same cache, the same in-flight and
    /// known-absent sets — so a record's sleeve is one decode however many
    /// surfaces are drawing it, and asking twice costs one set lookup.
    /// **Decode the sounding record at [`art::HERO_PX`]**, once, and derive its
    /// field while the pixels are already in hand (doc 12 §5.2, §5.3).
    ///
    /// Everything expensive happens on the blocking worker: the decode, and
    /// [`crate::field::Field::derive`]'s one pass over the sampled pixels. What
    /// crosses back is a handle, one `f32`, and three hue angles — **the UI
    /// thread never sees a pixel of a cover**, which is what keeps the field's
    /// per-frame cost at three colour conversions.
    ///
    /// `no_art` is shared with the thumbnail tier deliberately: the two tiers
    /// run the *same* resolution order and the same decode, so a record with no
    /// decodable art has none in both and asking twice would be asking a
    /// question already answered.
    ///
    /// # The successor is not prefetched, and cannot be yet
    ///
    /// Doc 12 §5.2 budgets the two entries as *"the sounding record and the one
    /// after it"*. **The one after it cannot be named from here.** The UI's
    /// record of the run is [`vm::QueueVm`], whose rows carry a title, an
    /// artist and an album *string* and **no path and no album id** — the
    /// engine holds the paths. Resolving the next record would mean matching
    /// two strings against the wall and hoping no listener owns two editions of
    /// the same record, which is a worse answer than not having one.
    ///
    /// So the second entry is spent on **the record that was sounding a moment
    /// ago**, which the LRU gives for free and which a `Prev` press or a jump
    /// back up the run collects. Naming the successor is
    /// [ADR-0034](../../docs/adr/0034-the-run-and-its-list.md)'s `Origin` work
    /// — step M3, which is what puts identity on a run's rows — and it is one
    /// line here once that lands.
    pub(crate) fn request_hero(&mut self, sounding: Option<u64>) -> Task<Message> {
        let Some(id) = sounding else {
            return Task::none();
        };
        // `get`, not `peek`: asking for the sounding record's hero is what
        // keeps it the freshest of the two entries, so the one the LRU drops
        // is always the one that stopped playing.
        if self.heroes.get(&id).is_some()
            || self.hero_pending == Some(id)
            || self.no_art.contains(&id)
        {
            return Task::none();
        }
        let Some(album) = self.albums.iter().find(|album| album.id == id) else {
            return Task::none();
        };
        self.hero_pending = Some(id);
        let path = album.first_track.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let (w, h, rgba) = art::load_hero_cached(&path)?;
                    let back = art::load_back(&path)
                        .map(|(w, h, rgba)| iced_image::Handle::from_rgba(w, h, rgba));
                    Some(Hero {
                        field: crate::field::Field::derive(w, h, &rgba),
                        px: shortest_edge(w, h),
                        handle: iced_image::Handle::from_rgba(w, h, rgba),
                        back,
                    })
                })
                .await
                .ok()
                .flatten()
            },
            move |hero| Message::HeroLoaded(id, hero),
        )
    }

    pub(crate) fn request_artist_image(&mut self, artist: u64) -> Task<Message> {
        if self.artist_images.get(&artist).is_some()
            || self.artist_image_pending.contains(&artist)
            || self.no_artist_image.contains(&artist)
        {
            return Task::none();
        }
        let Some(album) = self
            .albums
            .iter()
            .find(|album| vm::artist_id(&album.artist) == artist)
        else {
            return Task::none();
        };
        self.artist_image_pending.insert(artist);
        let path = album.first_track.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    art::load_artist(&path)
                        .map(|(w, h, rgba)| iced_image::Handle::from_rgba(w, h, rgba))
                })
                .await
                .ok()
                .flatten()
            },
            move |image| Message::ArtistImageLoaded(artist, image),
        )
    }

    pub(crate) fn artist_image(&self, artist: u64) -> Option<&iced_image::Handle> {
        self.artist_images.peek(&artist)
    }

    /// The sounding record's hero, when one is decoded — the Now playing
    /// place's artwork and the source of its field.
    pub(crate) fn hero(&self, id: u64) -> Option<&Hero> {
        self.heroes.peek(&id)
    }

    /// The shortest edge of `id`'s decoded **thumbnail**, in pixels.
    ///
    /// What the Now playing place clamps its artwork against for the frames
    /// between arriving and its hero landing. See [`Self::thumb_px`]'s field
    /// for why this is a true bound rather than a guess.
    pub(crate) fn thumb_edge(&self, id: u64) -> Option<f32> {
        self.thumb_px.get(&id).copied()
    }

    /// A decoded thumbnail from either the resident tier or the bounded
    /// off-screen LRU. Views are read-only, so observation never changes
    /// eviction order; residency is updated from measured viewport events.
    pub(crate) fn thumb(&self, id: u64) -> Option<&iced_image::Handle> {
        self.thumbs.peek(id)
    }

    /// The decoded authored sleeve for the playlist `id`, if it has one and it
    /// has arrived. `None` means *draw the collage* — either because the
    /// listener never set a picture, or because its decode is still in flight,
    /// and a tile that flickers from collage to picture once is better than a
    /// blank one that waits.
    pub(crate) fn playlist_image(&self, id: u64) -> Option<&iced_image::Handle> {
        self.playlist_images.get(&id)
    }

    /// Start the decodes for the authored sleeves in `wanted` that are neither
    /// cached nor in flight, and forget any that no longer belong to a list.
    ///
    /// `wanted` is the whole set the folder currently holds, not a delta, so
    /// this is also where a removed picture leaves the cache.
    pub(crate) fn request_playlist_images(&mut self, wanted: &[(u64, PathBuf)]) -> Task<Message> {
        let live: HashSet<u64> = wanted.iter().map(|(id, _)| *id).collect();
        self.playlist_images.retain(|id, _| live.contains(id));
        let mut tasks = Vec::new();
        for (id, path) in wanted {
            let (id, path) = (*id, path.clone());
            if self.playlist_images.contains_key(&id) || !self.playlist_image_jobs.insert(id) {
                continue;
            }
            tasks.push(Task::perform(
                async move {
                    tokio::task::spawn_blocking(move || {
                        art::load_picture(&path, art::THUMB_PX).map(decoded)
                    })
                    .await
                    .ok()
                    .flatten()
                },
                move |decoded| {
                    Message::PlaylistImageLoaded(id, decoded.map(|(_, _, handle)| handle))
                },
            ));
        }
        Task::batch(tasks)
    }

    /// One authored sleeve came back — or did not, in which case the list
    /// keeps its collage and nothing is retried until the folder is re-read.
    pub(crate) fn finish_playlist_image(&mut self, id: u64, handle: Option<iced_image::Handle>) {
        self.playlist_image_jobs.remove(&id);
        if let Some(handle) = handle {
            self.playlist_images.insert(id, handle);
        }
    }

    /// Drop what is cached and in flight for `id`, so the next request decodes
    /// the file that is there **now**. Spent when a listener sets or removes a
    /// picture: the path can be the same and the bytes different.
    pub(crate) fn forget_playlist_image(&mut self, id: u64) {
        self.playlist_images.remove(&id);
        self.playlist_image_jobs.remove(&id);
    }

    pub(crate) fn request_thumbs_for(&mut self, ids: &[u64]) -> Task<Message> {
        self.thumbs.focus_chrome(ids.iter().copied());
        self.request_target_thumbs()
    }

    /// Re-aim the scheduler from one complete target snapshot. Updating the
    /// wall, a page or resident chrome can no longer discard still-visible
    /// work nominated by either of the other two.
    ///
    /// # `focus` replaces, so it has to be given everything
    ///
    /// [`ThumbJobs::focus`] **drains the whole foreground queue and re-adds
    /// only its argument** — that is what "re-aim" means, and it is right: a
    /// wall that has scrolled past a record should stop waiting to decode it.
    /// But this function was handing it a **delta** — the targets that were
    /// neither cached nor *already queued* — so every re-aim threw away every
    /// job that was merely waiting its turn, and re-added nothing in its place.
    ///
    /// **That is the cold start, exactly.** iced emits `Scrolled` once the
    /// scrollable measures its real bounds (and `WindowResized` when the first
    /// resize lands); each handler recomputes the visible range and calls this;
    /// and the last one flushes the batch the scan drain had just queued,
    /// before two workers could consume more than two of it. Nothing else
    /// happens on an untouched window, so the wall sits on gradients until a
    /// scroll re-aims a range whose ids are now missing again and re-queues
    /// them. Measured on a fresh 25-album library at 1280 × 860 with no
    /// interaction at all: **two** decodes completed, and frames at 6, 9, 12
    /// and 15 seconds pixel-identical.
    ///
    /// The repair is to pass the **complete snapshot** rather than the delta —
    /// drop the `thumb_jobs.contains` exclusion and keep the other two. It is
    /// safe because `focus` already skips in-flight ids and `queued.insert` is
    /// idempotent, so drain-then-re-add now *preserves* queued work instead of
    /// discarding it, while still dropping whatever left the target set. The
    /// two exclusions that stay are the ones that are facts about the id rather
    /// than about the queue: `touch` says it is already decoded (and marks it
    /// recently used, which is why it must still be called on every target),
    /// and `no_art` says there is nothing on disk to decode.
    ///
    /// `request_thumbs` (a page) and `request_thumbs_for` (resident chrome)
    /// come through here too and get the same repair. `request_visible_thumbs`
    /// keeps its `last_requested` range guard, which is the dedupe for
    /// *identical* re-aims and is a separate concern from this one.
    /// [`ThumbJobs::retry`] — the density-grew retry — pushes to the front
    /// without draining and is untouched, which is item 30's shipped contract.
    fn request_target_thumbs(&mut self) -> Task<Message> {
        let mut wanted = Vec::new();
        for id in self.thumbs.targets() {
            if self.thumbs.touch(id) || self.no_art.contains(&id) {
                continue;
            }
            wanted.push(id);
        }
        self.thumb_jobs.focus(wanted);
        self.start_queued_thumbs()
    }

    /// Every thumbnail Home can draw: the All songs collage plus the visible
    /// newest-record row. Lane rows are resolved separately by the shell from
    /// the lane's exact mixed viewport.
    pub(crate) fn home_art(&self) -> Vec<u64> {
        let mut ids = self.everything().art;
        ids.extend(
            crate::views::home::newest(self, self.grid())
                .iter()
                .map(|album| album.id),
        );
        ids.sort_unstable();
        ids.dedup();
        ids
    }

    pub(crate) fn request_visible_thumbs(&mut self) -> Task<Message> {
        let (start, end) = self
            .shelves()
            .visible_albums(self.scroll_offset, self.grid_size.height);
        let tiles = self.visible.len();
        let (start, end) = (start.min(tiles), end.min(tiles));
        let visible_ids: Vec<u64> = self.visible[start..end]
            .iter()
            .filter_map(|&album_index| self.albums.get(album_index).map(|album| album.id))
            .collect();
        self.thumbs.focus_wall(visible_ids.iter().copied());
        // **Nothing new is on screen, so there is nothing to ask for.**
        //
        // Every resize step delivers *three* of these — `WindowResized` with
        // its estimate, then `Scrolled` when the scrollable measures its real
        // bounds, then `Scrolled` again when the grid that changed underneath
        // it changed the content's height (iced republishes a viewport whose
        // `content_bounds` moved, `iced_widget-0.13.4/src/scrollable.rs:1249`).
        // Measured at 87 messages a second under a dragged edge, and the work
        // behind each is a pass over every group in the library plus a walk of
        // the visible slice. Two of the three ask for exactly what the first
        // asked for.
        //
        // The guard is the *answer*, not the question: the range of albums on
        // screen. A drag that reveals no new record now costs one comparison.
        if self.last_requested == Some((start, end)) {
            return Task::none();
        }
        self.last_requested = Some((start, end));
        self.request_target_thumbs()
    }

    /// Kick off off-thread decodes for the albums in `ids` whose thumbnail is
    /// neither cached, in flight, nor known-absent — the playlist sleeves'
    /// supply line (ADR-0024 §A1), and deliberately nothing but a re-aim of
    /// [`Self::request_visible_thumbs`]'s pipeline: same cache, same decode
    /// path, same placeholder while it runs. An id the wall no longer holds
    /// is skipped; the collage cell keeps its gradient, which is the same
    /// honest reading a tile gives art that cannot be decoded.
    pub(crate) fn request_thumbs(&mut self, ids: &[u64]) -> Task<Message> {
        self.thumbs.focus_page(ids.iter().copied());
        self.request_target_thumbs()
    }

    fn finish_thumb(
        &mut self,
        id: u64,
        requested_edge: u32,
        elapsed: Duration,
        decoded: Option<(f32, usize, iced_image::Handle)>,
    ) -> Task<Message> {
        self.thumb_jobs.finished(id);
        match decoded {
            Some((px, bytes, handle)) if requested_edge >= self.density.art_max_px() => {
                self.thumb_px.insert(id, px);
                self.thumbs.put(id, handle, bytes);
            }
            Some(_) => {
                // Density grew while this blocking decode was in flight. The
                // smaller result is correct data but no longer enough pixels
                // for the active layout, so immediately replace the one job.
                self.thumb_jobs.retry(id);
            }
            None => {
                self.no_art.insert(id);
            }
        }
        let task = self.start_queued_thumbs();
        if std::env::var_os("BAZ_PERF_LOG").is_some() {
            let decoded_bytes = self.thumbs.decoded_bytes();
            let decoded_mib = decoded_bytes / (1024 * 1024);
            let decoded_tenths = (decoded_bytes % (1024 * 1024)) * 10 / (1024 * 1024);
            crate::baz_log!(
                "[art] thumb {id} in {:.1} ms; cache={} resident={} retained={} decoded={decoded_mib}.{decoded_tenths} MiB queued={} in-flight={} completed={} peak={}",
                elapsed.as_secs_f64() * 1e3,
                self.thumbs.len(),
                self.thumbs.resident_len(),
                self.thumbs.retained_len(),
                self.thumb_jobs.queued.len(),
                self.thumb_jobs.in_flight.len(),
                self.thumb_jobs.completed,
                self.thumb_jobs.peak,
            );
        }
        task
    }

    /// Fill the two decoder slots from the current page first, then from the
    /// visible returns lane. Importantly, blocking
    /// jobs are spawned only after a slot is acquired; putting a semaphore
    /// *inside* hundreds of already-spawned jobs would still grow Tokio's
    /// blocking pool and retain every queued task allocation.
    fn start_queued_thumbs(&mut self) -> Task<Message> {
        let mut tasks = Vec::new();
        let thumb_edge = self.density.art_max_px().min(art::THUMB_PX);
        while self.thumb_jobs.in_flight.len() < art::THUMB_DECODE_CONCURRENCY {
            let Some(id) = self.thumb_jobs.pop() else {
                break;
            };
            if self.thumbs.peek(id).is_some() || self.no_art.contains(&id) {
                continue;
            }
            let Some(path) = self
                .albums
                .iter()
                .find(|album| album.id == id)
                .map(|album| album.first_track.clone())
            else {
                continue;
            };
            self.thumb_jobs.started(id);
            tasks.push(Task::perform(
                async move {
                    let started = Instant::now();
                    let decoded = tokio::task::spawn_blocking(move || {
                        art::load_thumb_cached(&path, thumb_edge).map(decoded)
                    })
                    .await
                    .ok()
                    .flatten();
                    (started.elapsed(), decoded)
                },
                move |(elapsed, handle)| Message::ThumbLoaded(id, thumb_edge, elapsed, handle),
            ));
        }
        Task::batch(tasks)
    }

    /// The **Library place**: the top bar over the grid. Composition only —
    /// the surfaces themselves are [`crate::views`].
    ///
    /// Two elements in one column, and that is the whole of it. It held a
    /// three-way `row!` — the wall at an explicit width, a hairline, and the
    /// inspector behind a reveal viewport — because the grid had to survive a
    /// column arriving beside it over 150 ms. ADR-0022 deleted the column, so
    /// the wall takes the window and nothing is beside it.
    pub(crate) fn view<'a>(
        &'a self,
        player: &'a PlayerState,
        lamp: f32,
        collecting: crate::playlists::Collecting,
        over_weather: bool,
    ) -> Element<'a, Message> {
        column![
            views::top_bar::view(self, self.body_width()),
            views::shelf::view(self, player, lamp, collecting, over_weather)
        ]
        .into()
    }

    /// The album `id`'s view model, if the wall still holds it.
    ///
    /// `None` after a rescan has taken the record away while its page was open,
    /// which the shell answers by drawing the wall instead.
    pub(crate) fn album(&self, id: u64) -> Option<&vm::AlbumVm> {
        self.albums.iter().find(|album| album.id == id)
    }
}

fn record_root_scan(health: &mut crate::health::Log, root: &std::path::Path, counts: [usize; 4]) {
    let [added, updated, unchanged, failed] = counts;
    crate::baz_log!(
        "[scan] {}: {added} added, {updated} updated, {unchanged} unchanged, {failed} skipped",
        root.display()
    );
    health.record(
        if failed > 0 {
            crate::health::Level::Warning
        } else {
            crate::health::Level::Ready
        },
        "Folder scanned",
        format!(
            "{}\n{added} added · {updated} updated · {unchanged} unchanged · {failed} skipped",
            root.display()
        ),
    );
}

/// One tick's worth of scan updates, taken off the channel and not yet applied
/// (`Shelf::collect_scan` → `Shelf::apply_scan`).
///
/// The split exists because the two halves want different borrows: receiving
/// holds the channel, and applying holds the library. Keeping them apart is
/// also what makes the "one `add_tracks_under` per root per tick" property
/// visible rather than buried in a loop.
struct Drained {
    /// Tracks read this tick, grouped by the root that produced them.
    fresh_tracks: Vec<(PathBuf, Vec<baz_core::library::TrackMeta>)>,
    /// Rows the removal pass proved are gone.
    vanished: Vec<PathBuf>,
    /// Missing paths whose absent parent makes them manual-confirmation only.
    prunable: Vec<PathBuf>,
    /// Roots whose walk finished, with the moment it did.
    scanned: Vec<(PathBuf, i64)>,
    /// Roots that could not be walked, with the reason.
    missing: Vec<(PathBuf, String)>,
    /// Whether the pass is over.
    finished: bool,
}

/// Persist the folders baz holds; best-effort with a log, never fatal — a
/// read-only config dir must not block listening to music.
///
/// This is also where the silent migration lands: [`persist`] reads the
/// file first, so a document that still carries the pre-ADR-0022 `music_dir` is
/// parsed into the list, replaced by it, and written back under the new key
/// with everything else in the document intact.
fn persist_roots(roots: &[PathBuf]) {
    for root in roots {
        if root.to_str().is_none() {
            crate::baz_log!(
                "[config] {} is not valid UTF-8; it cannot be written to config.toml \
                 (this session is unaffected)",
                root.display()
            );
        }
    }
    persist(|config| config.music_dirs = roots.to_vec());
}

/// Claim the index's rootless rows for the folders baz holds — schema v8's
/// backfill, made from the one place that knows both halves (ADR-0022).
///
/// Best-effort with a log: a failure leaves those rows rootless, which costs
/// them nothing but the ability to be pruned. In order, so a file under two
/// nested folders goes to the one the listener listed first.
fn adopt_roots(library: &mut Library, roots: &[PathBuf]) {
    for root in roots {
        match library.adopt_root(root) {
            Ok(0) => {}
            Ok(count) => {
                crate::baz_log!("[index] {count} rows now recorded under {}", root.display());
            }
            Err(error) => crate::baz_log!(
                "[index] could not adopt rows under {}: {error}",
                root.display()
            ),
        }
    }
}

/// **How many of each shelf's tiles survived the query**, in shelf order.
///
/// The shelves are contiguous slices of the flat list and `surviving` is in
/// the same order, so this is one walk of the two lists together rather than
/// a second filter that could disagree with the first.
fn surviving_per_shelf(surviving: &[usize], groups: &[GroupVm]) -> Vec<usize> {
    let mut seen = surviving.iter().peekable();
    groups
        .iter()
        .map(|group| {
            let mut count = 0;
            while seen.next_if(|index| **index < group.end).is_some() {
                count += 1;
            }
            count
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::num::NonZeroUsize;

    use iced::widget::image as iced_image;
    use lru::LruCache;

    use super::{Change, Hero, ThumbCache, ThumbJobs};
    use crate::app::hero_target;
    use crate::art;
    use crate::place::Place;

    #[test]
    fn none_stops_hero_work_without_costing_album_detail() {
        use crate::visualizer::Foreground;
        assert_eq!(
            hero_target(Place::Library, Some(7), Foreground::Cover),
            Some(7)
        );
        assert_eq!(
            hero_target(Place::NowPlaying, Some(7), Foreground::JewelCase),
            Some(7)
        );
        assert_eq!(
            hero_target(Place::NowPlaying, Some(7), Foreground::None),
            None
        );
        assert_eq!(
            hero_target(Place::Album(9), Some(7), Foreground::None),
            Some(9)
        );
    }

    #[test]
    fn visible_art_replaces_stale_work() {
        let mut jobs = ThumbJobs::default();
        jobs.focus([1, 2, 3]);
        jobs.focus([3, 4]);

        assert_eq!(jobs.foreground, VecDeque::from([3, 4]));
        assert_eq!(jobs.pop(), Some(3));

        jobs.focus([5]);
        assert_eq!(jobs.foreground, VecDeque::from([5]));
        assert!(!jobs.queued.contains(&4), "the old viewport was discarded");
    }

    #[test]
    fn in_flight_art_is_deduplicated_but_never_cancelled_by_a_new_viewport() {
        let mut jobs = ThumbJobs::default();
        jobs.focus([7, 8]);
        assert_eq!(jobs.pop(), Some(7));
        jobs.started(7);

        jobs.focus([7, 9]);
        assert_eq!(jobs.foreground, VecDeque::from([9]));
        assert!(jobs.in_flight.contains(&7));
        assert_eq!(jobs.peak, 1);

        jobs.finished(7);
        assert_eq!(jobs.completed, 1);
        assert!(!jobs.in_flight.contains(&7));
    }

    #[test]
    fn one_complete_target_snapshot_keeps_page_and_chrome_work() {
        let mut jobs = ThumbJobs::default();
        jobs.focus([10, 11, 20, 21]);

        assert_eq!(jobs.pop(), Some(10));
        assert_eq!(jobs.pop(), Some(11));
        assert_eq!(jobs.pop(), Some(20));
        assert_eq!(jobs.pop(), Some(21));
    }

    /// **A re-aim that changes nothing must lose nothing** — the cold start,
    /// as arithmetic.
    ///
    /// `focus` replaces, which is right: a wall that scrolled past a record
    /// should stop waiting to decode it. What was wrong was *what it was
    /// given*. `request_target_thumbs` handed it the targets that were neither
    /// cached nor **already queued**, so a re-aim over an unchanged viewport
    /// passed the empty set and the replace threw the whole queue away.
    ///
    /// On an untouched cold start that happens twice — iced emits `Scrolled`
    /// when the scrollable measures its real bounds, and `WindowResized` when
    /// the first resize lands — and there is no third event to re-queue
    /// anything, so the wall sits on gradients until someone touches it.
    /// Measured before the fix on a fresh 25-album library at 1280 × 860 with
    /// no interaction: **2** decodes completed and frames at 6, 9, 12 and 15
    /// seconds pixel-identical. After: **8**, the whole visible wall.
    ///
    /// This is the pure half of that, and it is deliberately written as the
    /// *shape of the call* rather than as a screenshot: the defect is that a
    /// caller passed a delta to a replacing queue, so what has to be pinned is
    /// that a snapshot survives the replace and a delta does not.
    #[test]
    fn re_aiming_with_the_whole_snapshot_keeps_queued_work_that_a_delta_would_drop() {
        // The snapshot: everything still wanted, including what is already
        // queued. Two workers have taken the first two; the rest are waiting.
        let mut jobs = ThumbJobs::default();
        jobs.focus([1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(jobs.pop(), Some(1));
        jobs.started(1);
        assert_eq!(jobs.pop(), Some(2));
        jobs.started(2);

        // The re-aim iced delivers on its own, over an unchanged viewport.
        jobs.focus([1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(
            jobs.foreground,
            VecDeque::from([3, 4, 5, 6, 7, 8]),
            "the six waiting decodes were discarded by a re-aim that asked for \
             exactly what was already asked for"
        );
        assert!(
            jobs.in_flight.contains(&1) && jobs.in_flight.contains(&2),
            "a re-aim must not re-queue what two workers are already decoding"
        );

        // And the delta the old caller would have computed — nothing is
        // uncached-and-unqueued, so it is empty — takes the queue with it.
        let mut delta = ThumbJobs::default();
        delta.focus([1, 2, 3, 4, 5, 6, 7, 8]);
        delta.pop();
        delta.pop();
        delta.focus(std::iter::empty());
        assert!(
            delta.foreground.is_empty(),
            "this is the defect, held here so the difference between the two \
             calls is visible in one place"
        );
    }

    /// The other half of the same rule: a re-aim still **drops** what left the
    /// target set, which is what makes `focus` a re-aim rather than an append.
    /// Passing the whole snapshot buys back the waiting work without buying
    /// back the work that scrolled away.
    #[test]
    fn a_snapshot_re_aim_still_drops_what_left_the_viewport() {
        let mut jobs = ThumbJobs::default();
        jobs.focus([1, 2, 3, 4]);
        jobs.focus([3, 4, 5, 6]);
        assert_eq!(jobs.foreground, VecDeque::from([3, 4, 5, 6]));
        assert!(!jobs.queued.contains(&1) && !jobs.queued.contains(&2));
    }

    /// **The density retry still prepends rather than replacing** — item 30's
    /// shipped contract, re-asserted beside the change that altered how the
    /// queue is filled.
    ///
    /// A decode that completed too small after the density grew re-queues one
    /// id. It must not take the rest of the visible wall with it, which is
    /// exactly what it would do if it reached for `focus`.
    #[test]
    fn the_density_retry_prepends_and_keeps_the_rest_of_the_wall() {
        let mut jobs = ThumbJobs::default();
        jobs.focus([1, 2, 3]);
        jobs.retry(9);
        assert_eq!(jobs.foreground, VecDeque::from([9, 1, 2, 3]));
    }

    /// **The stated budget is stated, derived from, and reachable** — item 37's
    /// first half, which is a decision rather than a repair.
    ///
    /// The owner: the art machinery *"was introduced to try to keep RAM usage
    /// down but we never specified a sensible limit."* Everything this asserts
    /// is the arithmetic of that limit, so the numbers in `art`'s prose cannot
    /// drift away from the constants underneath them.
    #[test]
    fn the_art_budget_is_a_stated_decision_the_tiers_derive_from() {
        const MIB: usize = 1024 * 1024;
        // The figure, and the two it is chosen against: the owner's 393-album
        // index at Spacious's 320 px ceiling, every cover square.
        const OWNERS_ALBUMS: usize = 393;
        const SPACIOUS_ENTRY: usize = 320 * 320 * 4;
        // The two smaller tiers, for the whole-process figure below.
        const HERO: usize = 1024 * 1024 * 4 * art::HERO_CACHE_ENTRIES;
        const ARTIST: usize = 256 * 256 * 4 * art::ARTIST_CACHE_ENTRIES;
        const {
            assert!(art::THUMB_BUDGET_BYTES == 160 * MIB);
            assert!(OWNERS_ALBUMS * SPACIOUS_ENTRY < art::THUMB_BUDGET_BYTES);
            // …and it is the smallest 32 MiB step that clears it, so the
            // headroom is not a second undeclared decision.
            assert!(OWNERS_ALBUMS * SPACIOUS_ENTRY > art::THUMB_BUDGET_BYTES - 32 * MIB);
        }
        // The speculative sub-budget is what the entry count is derived *from*,
        // and it comes to the count the tier has always had — so stating the
        // decision in bytes changed no behaviour.
        const {
            assert!(art::SPECULATIVE_BUDGET_BYTES == 25 * MIB);
            assert!(art::THUMB_CACHE_ENTRIES == 64);
            assert!(art::THUMB_CACHE_ENTRIES * SPACIOUS_ENTRY == art::SPECULATIVE_BUDGET_BYTES);
            assert!(art::SPECULATIVE_BUDGET_BYTES < art::THUMB_BUDGET_BYTES);
        }
        // And all decoded artwork in the process is the figure worth quoting,
        // which is the one a process monitor shows — and which Settings →
        // Debug now shows the resident set beside.
        const {
            assert!(HERO == 8 * MIB);
            assert!(ARTIST == 2 * MIB);
            assert!(art::THUMB_BUDGET_BYTES + HERO + ARTIST == 170 * MIB);
        }
    }

    /// **The resident tier's exemption is safe**, which is the budget's one
    /// hole and therefore the one thing that has to be argued rather than
    /// assumed.
    ///
    /// `trim_to_budget` will not evict art the current frame can draw — item
    /// 20's rule, and the reason this whole tier exists. That means a window
    /// whose *visible wall alone* exceeded [`art::THUMB_BUDGET_BYTES`] would
    /// exceed it. It cannot: the widest window baz supports pins **51 MiB** at
    /// its worst density, a little under a third of the budget.
    ///
    /// The bound is deliberately generous — a full 4K window, every tile at
    /// the density's *smallest* work so the count is maximal, no room taken by
    /// the two bars or the captions — and the margin it clears by is **stated
    /// rather than assumed**, because it is nearer than it looks: the worst
    /// density comes to about a third of the budget, not a hundredth. A tier
    /// that cannot be evicted is worth knowing the size of.
    ///
    /// Each density is costed against **its own** decode ceiling
    /// ([`crate::shelf::Density::art_max_px`]), which is the pairing that
    /// actually happens — Dense hangs the most tiles *and* decodes the
    /// smallest, so the two do not compound. Costing every density at
    /// [`art::THUMB_PX`] would be a worst case the product cannot reach and
    /// would fail this test for a reason that is not true.
    #[test]
    fn the_visible_wall_can_never_exhaust_the_art_budget() {
        use crate::shelf::Density;

        // Far past any window baz is dragged to, and the bars take none of it.
        let (window_w, window_h) = (3840.0_f32, 2160.0_f32);
        let worst = Density::ALL
            .iter()
            .map(|density| {
                #[expect(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "a count of tiles across a window is small and non-negative"
                )]
                let tiles = ((window_w / density.art_min()).ceil() as usize)
                    * ((window_h / density.art_min()).ceil() as usize);
                let edge = density.art_max_px() as usize;
                (tiles * edge * edge * 4, *density, tiles)
            })
            .max_by_key(|(bytes, _, _)| *bytes)
            .expect("the densities are not empty");
        let (bytes, density, tiles) = worst;
        assert!(
            bytes * 2 < art::THUMB_BUDGET_BYTES,
            "the widest supported window at {density:?} pins {tiles} tiles, \
             {} MiB, against a {} MiB budget — the resident tier is exempt from \
             the trim, so it must stay comfortably under it",
            bytes / (1024 * 1024),
            art::THUMB_BUDGET_BYTES / (1024 * 1024)
        );
    }

    /// **The budget is enforced, and it is enforced in the right order.**
    ///
    /// Speculative art — decoded for, never displayed — goes first; then the
    /// least recently *visited* retained art; and the resident tier is never
    /// touched. Written over a tiny budget so the arithmetic is legible; the
    /// tiering is what is being asserted, not the size.
    #[test]
    fn the_budget_trims_speculative_art_first_then_the_least_recently_visited() {
        // One entry per byte-budget slot: `put_pixel` writes 4 bytes.
        let mut cache = ThumbCache::new(NonZeroUsize::new(8).expect("a cache"));

        // Three covers the listener has actually looked at, in order.
        for id in [1, 2, 3] {
            cache.focus_wall([id]);
            put_pixel(&mut cache, id);
            cache.focus_wall([]);
        }
        assert_eq!(cache.retained_len(), 3);

        // One the listener is looking at now.
        cache.focus_wall([9]);
        put_pixel(&mut cache, 9);
        assert_eq!(cache.resident_len(), 1);

        // And some speculative completions behind them.
        for id in [20, 21] {
            put_pixel(&mut cache, id);
        }
        assert_eq!(cache.recent.len(), 2);

        // Now squeeze. The trim runs on `put`, so a budget this small is
        // easier to exercise directly.
        cache.trim_to_budget();
        assert_eq!(
            cache.decoded_bytes(),
            6 * 4,
            "nothing should be dropped while the whole cache is far under budget"
        );

        // Re-touching a retained id makes it the most recent, which is the
        // ordering the trim depends on and the reason `retained` is an LRU.
        assert!(cache.touch(1));
        let order: Vec<u64> = cache.retained.iter().map(|(id, _)| *id).collect();
        assert_eq!(
            order.first(),
            Some(&1),
            "visiting retained art did not make it recent, so a trim would drop \
             the art the listener just looked at"
        );

        // Resident art survives a trim that has nothing else left to take.
        cache.recent.clear();
        cache.retained.clear();
        cache.trim_to_budget();
        assert!(
            cache.peek(9).is_some(),
            "the trim reached into the current frame"
        );
    }

    /// **The trim actually drops things, and drops them in the stated order.**
    ///
    /// The test above establishes the ordering and the resident exemption; this
    /// one makes the budget *bind*, which needs entries big enough to reach it.
    /// The sizes are declared rather than decoded — `decoded_bytes` is a number
    /// the caller hands the cache from the real decode, so a 1 × 1 handle that
    /// claims a quarter of the budget exercises exactly the accounting under
    /// test without allocating 160 MiB in a unit test.
    #[test]
    fn the_budget_binds_and_takes_speculative_art_before_visited_art() {
        // Quarter-budget entries: four fit exactly, a fifth must displace one.
        let quarter = art::THUMB_BUDGET_BYTES / 4;
        let mut cache = ThumbCache::new(NonZeroUsize::new(64).expect("a cache"));
        let put = |cache: &mut ThumbCache, id: u64| {
            cache.put(id, pixel_handle(1), quarter);
        };

        // Two the listener has visited, oldest first, then two speculative.
        for id in [1, 2] {
            cache.focus_wall([id]);
            put(&mut cache, id);
            cache.focus_wall([]);
        }
        put(&mut cache, 20);
        put(&mut cache, 21);
        assert_eq!(cache.decoded_bytes(), 4 * quarter);
        assert!(cache.decoded_bytes() <= art::THUMB_BUDGET_BYTES);

        // A fifth decode. Speculative art goes first — art nobody has seen is
        // worth less than art the listener has.
        put(&mut cache, 22);
        assert!(
            cache.decoded_bytes() <= art::THUMB_BUDGET_BYTES,
            "the budget is not enforced"
        );
        assert!(
            cache.peek(1).is_some() && cache.peek(2).is_some(),
            "visited art was dropped while speculative art was still held"
        );
        assert!(
            cache.peek(20).is_none(),
            "the oldest speculative entry survived"
        );

        // With no speculative art left to absorb the overflow, the **least
        // recently visited** retained entry is what goes — and the one just
        // re-visited stays, which is the whole reason `retained` is ordered.
        cache.recent.clear();
        assert!(cache.touch(1), "1 is now the most recently visited");
        for id in [3, 4, 5] {
            cache.focus_wall([id]);
            put(&mut cache, id);
            cache.focus_wall([]);
        }
        assert!(cache.decoded_bytes() <= art::THUMB_BUDGET_BYTES);
        assert!(
            cache.peek(1).is_some(),
            "the trim dropped art the listener had looked at more recently than              art it kept"
        );
        assert!(
            cache.peek(2).is_none(),
            "the least recently visited retained art should have gone first"
        );
        assert!(
            [4, 5].iter().all(|id| cache.peek(*id).is_some()),
            "the art just visited was dropped"
        );
    }

    fn pixel_handle(red: u8) -> iced_image::Handle {
        iced_image::Handle::from_rgba(1, 1, vec![red, 0, 0, 255])
    }

    fn put_pixel(cache: &mut ThumbCache, id: u64) {
        cache.put(
            id,
            pixel_handle(u8::try_from(id % 255).expect("bounded color")),
            4,
        );
    }

    #[test]
    fn a_loaded_visible_sleeve_cannot_be_evicted_by_cache_churn() {
        let mut old = LruCache::new(NonZeroUsize::new(2).expect("a cache"));
        old.put(1, pixel_handle(1));
        old.put(2, pixel_handle(2));
        old.put(3, pixel_handle(3));
        assert!(
            old.peek(&1).is_none(),
            "reproduction: the old undifferentiated LRU evicts the visible sleeve"
        );

        let mut cache = ThumbCache::new(NonZeroUsize::new(2).expect("a cache"));
        put_pixel(&mut cache, 1);
        put_pixel(&mut cache, 2);
        cache.focus_wall([1]);

        for id in 3..20 {
            put_pixel(&mut cache, id);
        }

        assert!(cache.peek(1).is_some(), "the visible handle disappeared");
        assert_eq!(cache.resident_len(), 1);
        assert_eq!(cache.recent.len(), 2, "off-screen work stays bounded");
    }

    #[test]
    fn leaving_the_viewport_retains_art_that_was_actually_displayed() {
        let mut cache = ThumbCache::new(NonZeroUsize::new(2).expect("a cache"));
        cache.focus_wall([1]);
        cache.focus_chrome([1]);
        put_pixel(&mut cache, 1);

        cache.focus_wall([]);
        assert_eq!(
            cache.resident_len(),
            1,
            "another visible surface still quotes it"
        );
        cache.focus_chrome([]);
        assert_eq!(cache.resident_len(), 0);
        assert_eq!(cache.retained_len(), 1);
        assert!(
            cache.peek(1).is_some(),
            "unpinning does not drop the handle"
        );

        for id in 2..5 {
            put_pixel(&mut cache, id);
        }
        assert!(
            cache.peek(1).is_some(),
            "displayed art cannot become a gradient after unrelated churn"
        );
    }

    #[test]
    fn scroll_away_past_sixty_four_covers_and_return_keeps_every_shown_handle() {
        let mut cache = ThumbCache::new(
            NonZeroUsize::new(art::THUMB_CACHE_ENTRIES).expect("the production bound"),
        );
        let first = 1..=18;
        cache.focus_wall(first.clone());
        for id in first.clone() {
            put_pixel(&mut cache, id);
        }

        for page in 1..45 {
            let start = page * 18 + 1;
            let end = start + 17;
            cache.focus_wall(start..=end);
            for id in start..=end {
                put_pixel(&mut cache, id);
            }
        }

        cache.focus_wall(first.clone());
        for id in first {
            assert!(cache.peek(id).is_some(), "shown target {id} was evicted");
        }
        assert_eq!(cache.resident_len(), 18);
        assert_eq!(cache.retained_len(), 44 * 18);
        assert_eq!(cache.recent.len(), 0, "every fixture reached the viewport");
        assert_eq!(cache.decoded_bytes(), 45 * 18 * 4);
    }

    #[test]
    fn stale_density_retry_does_not_discard_other_visible_work() {
        let mut jobs = ThumbJobs::default();
        jobs.focus([10, 11, 12]);
        assert_eq!(jobs.pop(), Some(10));
        jobs.started(10);
        jobs.finished(10);
        jobs.retry(10);

        assert_eq!(jobs.foreground, VecDeque::from([10, 11, 12]));
    }

    #[test]
    fn queue_page_art_is_not_unpinned_by_the_resident_chrome_pass() {
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/app.rs"),
        )
        .expect("app source");
        assert!(
            source.contains("Place::Playlists | Place::Playlist(_) | Place::Queue"),
            "the after-message chrome pass can evict visible Queue-row sleeves"
        );
        assert!(
            source.contains("ids.extend(state.all_songs().art)")
                && source.contains("self.playlists.panel_open"),
            "the floating playlist panel has visible collages but no residency supply"
        );
        assert!(
            source.contains("self.playlists.panel_open,")
                && source.contains("crate::views::home::standing"),
            "panel-open and Home Continue must participate in the target snapshot"
        );
    }

    /// A 1 × 1 decode standing in for a cover: a distinct [`Hero`] every call,
    /// which is what makes the handle comparisons below mean anything.
    fn a_hero() -> Hero {
        Hero {
            handle: iced_image::Handle::from_rgba(1, 1, vec![0_u8; 4]),
            back: None,
            px: 1.0,
            field: None,
        }
    }

    /// **The dissolve's predicate is the picture, and it is a refusal three
    /// ways out of four** (ADR-0020's third amendment; [`Change::between`]).
    ///
    /// The case that matters most is the third: consecutive tracks on one
    /// record share a cover, and fading a picture into an identical picture is
    /// a flight, a clock and 25 wakes announcing a change nothing made.
    #[test]
    fn a_dissolve_needs_two_pictures_that_are_not_the_same_picture() {
        let (first, second) = (a_hero(), a_hero());
        assert_eq!(
            Change::between(Some(&first), Some(&second)),
            Change::Dissolve,
            "two decoded covers that differ"
        );
        // The same picture, arrived at twice — the surface redrawing what it
        // already had. A clone shares the handle, so this is an identity test
        // and not a coincidence of contents.
        assert_eq!(
            Change::between(Some(&first), Some(&first.clone())),
            Change::Cut,
            "a picture that has not changed may not start a flight"
        );
        // A stand-in is not artwork: the wall's deterministic gradient is what
        // a record with no cover draws, and dissolving one is decoration.
        assert_eq!(Change::between(None, Some(&second)), Change::Cut);
        assert_eq!(Change::between(Some(&first), None), Change::Cut);
        // The first record of a session has nothing behind it.
        assert_eq!(Change::between(None, None), Change::Cut);
    }

    /// **The transition needs both records decoded at once, and the two-entry
    /// hero LRU already holds them** — checked rather than trusted, because the
    /// entry that makes it true was written for a *prefetch* this product does
    /// not have yet ([`art::HERO_CACHE_ENTRIES`]'s own note).
    ///
    /// The discipline being reproduced is the one the shell really runs:
    /// [`Shelf::request_hero`] `get`s the **sounding** record on every message,
    /// which keeps it the freshest entry, and `HeroLoaded` `put`s the decode
    /// when it lands. Under that discipline the entry a `put` evicts is always
    /// the record *before last*, so the record that just stopped is still
    /// decoded for exactly as long as the dissolve needs it — and
    /// [`Shelf::art_prior`] is an `Arc` onto those same pixels rather than a
    /// copy of them.
    #[test]
    fn the_hero_lru_holds_both_records_a_dissolve_needs() {
        let mut heroes: LruCache<u64, Hero> = LruCache::new(
            NonZeroUsize::new(art::HERO_CACHE_ENTRIES).expect("the hero tier has entries"),
        );
        assert_eq!(art::HERO_CACHE_ENTRIES, 2, "the whole of the claim");

        // Four records in a row, which is more than the cache holds — so if
        // the second slot were spent on anything but the last record, one of
        // these rounds would find it gone.
        let mut previous: Option<u64> = None;
        for id in 1..=4 {
            // `request_hero`: ask for the sounding record, miss, decode.
            assert!(heroes.get(&id).is_none(), "record {id} was not decoded yet");
            // `HeroLoaded`: the decode lands.
            heroes.put(id, a_hero());
            // …and the dissolve asks for the record that just stopped.
            if let Some(was) = previous {
                assert!(
                    heroes.peek(&was).is_some(),
                    "record {was} was evicted before {id}'s dissolve could use it"
                );
                // Both alive at once is the whole requirement.
                assert!(heroes.peek(&id).is_some());
                assert_eq!(heroes.len(), 2);
            }
            previous = Some(id);
        }
        // And the one *before* that is gone, which is the budget holding: two
        // entries, 8 MiB, and the crossfade adds no third.
        assert!(heroes.peek(&2).is_none());
    }
}
