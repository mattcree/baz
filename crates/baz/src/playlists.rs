//! The playlist surfaces' state: the panel, the open page, and the acts on
//! the shelf of files (ADR-0024 §4–§6).
//!
//! [`baz_core::playlist`] is the storage layer — the format, the folder, the
//! honesty clause that nothing writes a playlist but the user's own edit.
//! This module is the *shell's* half: whether the panel is open, what a pick
//! in flight is holding, what the open page's rows resolve to against the
//! library, and the guarded edits the page's controls mean. It is
//! ADR-0006 layer 1 — pure of iced, unit-tested — and every engine effect
//! (playing, queueing, the picker's Queue row) stays in `app.rs`, exactly as
//! it does for the album page.
//!
//! There is no collecting mode here. The armed layer (ADR-0024 §6 layer 2)
//! shipped and was removed the next day on the owner's own observation — it
//! was a second list-building grammar and a mode
//! (`docs/design/09-implicit-playlists.md` §9). What remains is the one
//! transfer gesture: a `+` or `Add to playlist…` opens the panel as the
//! picker, and the pick lands where it is aimed — the Queue first, then the
//! lists.
//!
//! # The fingerprint discipline
//!
//! Every edit goes through [`Playlists::edit_open`], which asks the storage
//! layer's [`Playlist::externally_edited`] **before** applying anything. A
//! file that changed under baz — vim, a sync tool, another baz — is re-read
//! and the press is dropped rather than applied: an index computed against a
//! stale picture removes a different track, which is the exact failure
//! ADR-0014 chose whole-queue commands to rule out, and last-writer-wins
//! (ADR-0024 §2) is about *files*, not about aiming a stale row number at
//! fresh contents. The next press acts on what is actually in the file, which
//! is by then on screen.
//!
//! # What "playable" means
//!
//! The storage layer holds no opinion ([`Playlist::partition`] takes the
//! caller's verdict), and the verdict here is ADR-0024 §3's: **an entry the
//! index knows is playable; an entry it does not know plays anyway if the
//! file exists** — refusing a file the user explicitly listed because the
//! cache lacks a row would invert the cache/source-of-truth order — and an
//! entry whose path resolves to nothing is missing: it stays in the file,
//! renders dimmed from the path's stem, and is sent nowhere. The one `stat`
//! per unindexed entry happens at load, not per frame.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use baz_core::history::Recency;
use baz_core::index::{AlbumArtist, GroupKey, Initial, Library};
use baz_core::playlist::{Entry, ExtInf, Folder, Item, Note, Playlist, PlaylistError};

use crate::vm::{self, GroupHeaderVm, QueueItemVm, QueueVm, RunSource, TrackVm};

/// What the record page's transfer affordances need to know, bundled so the
/// view signatures stay readable: whether playlists can exist at all, and
/// whether the panel is standing beside the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Collecting {
    /// Whether a playlists folder exists on this system.
    pub(crate) available: bool,
    /// Whether the panel is open — the state that draws the track rows' `+`
    /// at rest (the task's own furniture, not permanent chrome; when the
    /// panel is closed the `+` is hover-revealed, with the record page's
    /// `Add to playlist…` as the always-visible route to the same picker).
    pub(crate) panel_open: bool,
}

/// A playlist's session identity: FNV-1a 64 over the *name's* exact bytes.
///
/// The filename is the name (ADR-0024 §2), so the name is the identity, and
/// this is to a playlist what [`vm::album_id`] is to an album: a `Copy`
/// handle [`crate::place::Place::Playlist`] can carry, resolved against the
/// folder's listing whenever it is spent. Case-sensitive because filesystems
/// are; a rename mints a new id and the shell moves the place with it.
#[must_use]
pub(crate) fn playlist_id(name: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &byte in name.as_bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
    }
    hash
}

/// Reserved identity for the built-in list. It is not derived from a
/// filename because no listener-owned file exists behind it.
pub(crate) const FAVOURITES_ID: u64 = 0xFABA_0000_0000_0001;

fn empty_favourites_row() -> PanelRow {
    PanelRow {
        id: FAVOURITES_ID,
        name: "Favourites".to_owned(),
        entries: 0,
        seconds: None,
        playable: 0,
        created_unix_s: None,
        touched_unix_s: None,
        art: Vec::new(),
        // The built-in has no file, so there is nothing to put a picture
        // beside; it wears its heart instead (`views::default_playlist_mark`).
        image: None,
    }
}

fn favourites_row(library: Option<&Library>) -> PanelRow {
    let Some(library) = library else {
        return empty_favourites_row();
    };
    let tracks = library.favourite_tracks();
    let mut row = empty_favourites_row();
    row.entries = tracks.len() + library.missing_favourites();
    row.playable = tracks.len();
    let mut seconds = 0_u64;
    let mut timed = false;
    for meta in tracks {
        if let Some(duration) = meta.duration {
            seconds = seconds.saturating_add(duration.as_secs());
            timed = true;
        }
        let album = vm::album_id(AlbumArtist::of(meta), library.record_title(meta));
        if row.art.len() < 4 && !row.art.contains(&album) {
            row.art.push(album);
        }
    }
    row.seconds = timed.then_some(seconds);
    row
}

/// One playlist as the panel lists it: identity, name, and the two facts a
/// row states (`12 · 42:10`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PanelRow {
    /// [`playlist_id`] of the name.
    pub(crate) id: u64,
    /// The name — the file's stem, exactly.
    pub(crate) name: String,
    /// How many entries the file holds, duplicates and all.
    pub(crate) entries: usize,
    /// Total declared time, over the entries whose `#EXTINF` declared one.
    /// `None` when none did (a bare imported path list), so the row does not
    /// claim `0:00` about music it has not measured.
    pub(crate) seconds: Option<u64>,
    /// Entries baz can currently hand to the engine.
    pub(crate) playable: usize,
    /// When the playlist file was created, in Unix seconds, when the
    /// filesystem exposes that fact. This is the Playlists place's creation
    /// ordering key and is deliberately not the mtime below: editing a list
    /// must not make it newly created.
    pub(crate) created_unix_s: Option<u64>,
    /// **When the file was last written**, in seconds since the Unix epoch —
    /// the returns lane's order key for a list (ADR-0030 §1: *a playlist is
    /// touched when it is played, or when its file is written by the user's
    /// own edit*).
    ///
    /// The mtime, from the fingerprint this module already reads for the
    /// external-edit check, so the lane costs the folder no extra `stat`.
    /// `None` on a filesystem that reports no usable stamp: the list is still
    /// in the lane, at the end of the touched rows, because *"every playlist,
    /// always"* is the membership rule and a missing timestamp is not a
    /// reason to hide a thing somebody made.
    ///
    /// **Playing a list does not move it**, and that is honest rather than
    /// incomplete: the ledger records tracks, so a play of a list is
    /// indistinguishable from a play of the records in it. Recording a
    /// separate played-at per list would be a second history to keep true.
    pub(crate) touched_unix_s: Option<u64>,
    /// The sleeve's quotations (ADR-0024 §A1): the first four *distinct*
    /// records the library resolves, in playlist order — four for the 2 × 2
    /// collage, fewer meaning "draw the first full-bleed", none meaning the
    /// rest tile.
    pub(crate) art: Vec<u64>,
    /// The **authored** sleeve, where the listener has set one: the sibling
    /// picture beside the `.m3u8` (`baz_core::playlist::IMAGE_EXTENSIONS`).
    /// A list that has one draws it instead of its collage at every size, and
    /// removing it gives the collage back — the collage is what a playlist's
    /// sleeve *is* when nobody has said otherwise.
    pub(crate) image: Option<PathBuf>,
}

/// How the full Playlists place arranges its tiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum PlaylistOrder {
    /// Names, case-insensitive, with original spelling as the stable tie-break.
    #[default]
    Alphabetical,
    /// Newest-created first; files whose filesystem cannot report creation
    /// time follow the dated files, alphabetically.
    Created,
    /// Most recently played first; lists with no attributed play follow the
    /// played lists, alphabetically.
    Played,
}

impl PlaylistOrder {
    pub(crate) const ALL: [Self; 3] = [Self::Alphabetical, Self::Created, Self::Played];

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Alphabetical => "A–Z",
            Self::Created => "Date created",
            Self::Played => "Played",
        }
    }

    /// The Library group key this ordering projects onto, which is what the
    /// shared index rail is built from. `A–Z` is the alphabet; the two
    /// chronological orderings are the Library's own elapsed buckets.
    pub(crate) const fn key(self) -> GroupKey {
        match self {
            Self::Alphabetical => GroupKey::Alphabet,
            Self::Created => GroupKey::Added,
            Self::Played => GroupKey::Played,
        }
    }
}

impl PanelRow {
    /// The row's line under the name: `Playlist · 12 · 42:10`, or
    /// `Playlist · 12` when no time is known.
    ///
    /// **The line under a name declares its kind in its first token**
    /// (ADR-0024 §A3.1). A found thing's line is an artist's name; a made
    /// thing's is this; an implicit one's is a scale statement, which
    /// [`crate::implicit`]'s `All songs` already gives
    /// (`1284 records · 9902 songs · 84:12:07`). The rule is spent here
    /// because this one string is what the returns lane's rows
    /// (`crate::app::App::sync_lane`) and the panel's rows
    /// (`crate::views::playlist_panel`) both draw — and any tile a playlist
    /// ever reaches will draw it too.
    ///
    /// Before this the string was a **bare integer**: `14`, at
    /// [`theme::SIZE_META`](crate::theme::SIZE_META) 12 in `paper_faint`, in
    /// the exact slot where a record prints `Anne-Marie Puig`. It did not read
    /// as a count — it read as a name truncated to nothing, which is design
    /// 14 §3.1's finding.
    ///
    /// **No new widget and no geometry change**: the same `SIZE_META` text at
    /// the same leading, a different string. At the lane's 146 px of measure
    /// ([`crate::theme::SIDEBAR_ROW_TEXT_W`]) the longest form this can take is well
    /// inside the measure — measured with the real face in the row test.
    #[must_use]
    pub(crate) fn counts(&self) -> String {
        match self.seconds {
            Some(seconds) => format!(
                "Playlist · {} · {}",
                self.entries,
                vm::format_duration(Duration::from_secs(seconds))
            ),
            None => format!("Playlist · {}", self.entries),
        }
    }
}

/// One cell on the saved-playlist wall.
///
/// The create affordance is a *cell* rather than a control in the strip
/// (the owner: *"the new playlist should be like a ghost playlist with a + in
/// the middle called 'New Playlist' on the playlist page, not a button"*), so
/// the wall's layout has to be able to say "a tile that is not a list" — which
/// is the whole of this type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Cell<'a> {
    /// The ghost tile: press it to open the creation place.
    New,
    /// A saved playlist, or the built-in `Favourites` row.
    List(&'a PanelRow),
}

/// **The saved-playlist wall's layout, derived once.**
///
/// Built by [`Playlists::wall`]; laid out by [`crate::shelf::Shelves`], which
/// is the Library's own layout engine and takes exactly this — a count per
/// run. The view draws it and `App::request_playlist_art` reads the same
/// projection to decide which collages to decode, so the two cannot disagree
/// about which tile is in view.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Wall<'a> {
    /// Every cell, in wall order, across all runs.
    pub(crate) cells: Vec<Cell<'a>>,
    /// One entry per run. `None` is the unlabelled lead run.
    pub(crate) headers: Vec<Option<GroupHeaderVm>>,
    /// How many cells each run holds, in run order — [`crate::shelf::Shelves`]'
    /// own input.
    pub(crate) counts: Vec<usize>,
    /// The group key the rail is drawn from — the active ordering's
    /// projection ([`PlaylistOrder::key`]).
    pub(crate) key: GroupKey,
}

impl Wall<'_> {
    /// The headers the index rail indexes: every labelled run, in order.
    ///
    /// The lead run is skipped, so a rail entry's index is one less than its
    /// run's — [`Self::run_of`] is the way back, and the only way back.
    pub(crate) fn rail_headers(&self) -> Vec<GroupHeaderVm> {
        self.headers.iter().flatten().cloned().collect()
    }

    /// The run a rail entry jumps to: the lead run holds no heading, so every
    /// indexed run is one past it.
    pub(crate) const fn run_of(rail_entry: usize) -> usize {
        rail_entry + 1
    }

    /// **Whether a run's heading may be pinned to the top of the viewport.**
    ///
    /// The lead run's may not: it has no heading, and the pinned layer paints
    /// an opaque band, so pinning nothing would draw a blank strip over the
    /// covers scrolling under it.
    pub(crate) fn pinned(&self, run: usize) -> bool {
        self.headers.get(run).is_some_and(Option::is_some)
    }
}

/// Map a filesystem or session timestamp to the Library's honest age buckets.
///
/// `Unrecorded` describes an unavailable creation stamp; `Never` describes a
/// playlist that has not been played in this run. They are deliberately not
/// collapsed into the same quiet label.
fn recency(timestamp: Option<u64>, created: bool) -> Recency {
    let Some(timestamp) = timestamp else {
        return if created {
            Recency::Unrecorded
        } else {
            Recency::Never
        };
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = now.saturating_sub(timestamp) / 86_400;
    match days {
        0 => Recency::Today,
        1..=6 => Recency::ThisWeek,
        7..=30 => Recency::ThisMonth,
        31..=364 => Recency::MonthsAgo(u32::try_from((days / 30).max(1)).unwrap_or(u32::MAX)),
        _ => Recency::YearsAgo(u32::try_from((days / 365).max(1)).unwrap_or(u32::MAX)),
    }
}

/// An inline name entry — the panel's `New playlist`, the page's rename, the
/// queue place's `Save as playlist`. The roots field's anatomy: text, and the
/// storage layer's refusal surfaced plainly under it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct NameEntry {
    /// What has been typed.
    pub(crate) text: String,
    /// Why the last submission did not go through, in the storage layer's own
    /// words, if it did not.
    pub(crate) error: Option<String>,
}

/// What a pick is waiting to add: the record (or the one track) that was
/// pointed at, held while the panel serves as the picker (09 §8.1 — one
/// transfer gesture, every destination).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Pending {
    /// What the panel's hint names — `Add "Geogaddi"`, `Add "Amo Bishop
    /// Roden"`.
    pub(crate) label: String,
    /// The entries a pick appends to a *file*, in order, with their
    /// `#EXTINF` metadata.
    pub(crate) entries: Vec<Entry>,
    /// The same music as queue items, for a pick that lands on the picker's
    /// **Queue** row instead of a file — carried from the gesture so the
    /// append keeps the record-group facts (album, filed-under) the queue
    /// place's headers need, which a path-and-`#EXTINF` entry no longer
    /// holds.
    pub(crate) items: Vec<QueueItemVm>,
}

/// One row of the open playlist's page, resolved and render-ready.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PageRow {
    /// Position as the page numbers it, from 1 — over every entry, missing
    /// ones included, because the file holds them and the page shows the
    /// file.
    pub(crate) position: usize,
    /// Display title: the index's, else the `#EXTINF`'s, else the path's
    /// stem (which is all a missing entry has).
    pub(crate) title: String,
    /// The track's artist, when something said one.
    pub(crate) artist: Option<String>,
    /// The record title shown in the playlist table's Album column. Unlike
    /// the retired group heading, this travels with every row.
    pub(crate) album: Option<String>,
    /// The shelf identity for this row's artwork, when the library resolved
    /// it to a record.
    pub(crate) album_id: Option<u64>,
    /// `m:ss`, or empty when nothing declared a length.
    pub(crate) duration: String,
    /// Whether the path resolved to nothing: drawn dimmed, unplayable, and
    /// left in the file (ADR-0024 §3).
    pub(crate) missing: bool,
    /// This row's position in the *playable subset* — the index `JumpTo`
    /// speaks — or `None` for a missing row.
    pub(crate) playable_position: Option<usize>,
    /// The referenced file, for the missing row's one-glance-away path.
    pub(crate) path: PathBuf,
}

/// The open playlist: the storage value, and everything the page draws
/// resolved from it against the library.
#[derive(Debug)]
pub(crate) struct OpenPlaylist {
    /// [`playlist_id`] of the name, which is what the place carries.
    pub(crate) id: u64,
    /// The storage value; [`Playlist::save`] is the only door to the disk.
    playlist: Playlist,
    /// The page's rows, in file order.
    pub(crate) rows: Vec<PageRow>,
    /// The playable subset as track values, for
    /// [`PlayerState::play_from`](crate::player::PlayerState::play_from) and
    /// [`playing_row_in`](crate::player::PlayerState::playing_row_in) — the
    /// same rule every list surface spends, unchanged.
    pub(crate) tracks: Vec<TrackVm>,
    /// What `Play` sends and what the rows were counted from: the playable
    /// subset as a queue record, one value for both so the paths sent and the
    /// rows drawn cannot describe different music.
    pub(crate) queue: QueueVm,
    /// How many entries did not resolve.
    pub(crate) missing: usize,
    /// The sleeve's quotations, by [`PanelRow::art`]'s rule — recomputed
    /// with every re-read, so an edit that changes the first records changes
    /// the sleeve with the rows.
    pub(crate) art: Vec<u64>,
    /// **How many distinct records this list is drawn from**, uncapped — what
    /// the page's byline states (`Playlist · 4 records`, ADR-0024 §A4.3).
    ///
    /// It is *not* `art.len()`. [`art`](Self::art) stops at four because four
    /// is all the 2 × 2 collage can quote, so a fourteen-record list has an
    /// `art` of 4 — and a byline reading `Playlist · 4 records` over it would
    /// be a false statement about the object, which is the one thing this
    /// whole change is for. Design 14 §5.4 costed the byline as free from the
    /// sleeve's list; the sleeve's list cannot pay for it, and this counts
    /// the walk out to its end.
    ///
    /// Entries the library cannot resolve contribute nothing — an unindexed
    /// path names no record — so a list of nothing but missing entries states
    /// `Playlist` and no count, which is all it can prove.
    pub(crate) records: usize,
    /// The **authored** sleeve beside this list's file, if the listener set
    /// one. Held on the open page as well as on the row because the page's
    /// acts read it — `Set image…` or `Change image…`, and `Remove image`
    /// only where there is one to remove.
    pub(crate) image: Option<PathBuf>,
    /// The rename field, while renaming.
    pub(crate) renaming: Option<NameEntry>,
    /// Whether Delete has been pressed once and is waiting for the explicit
    /// `Move to Trash` confirmation on the page.
    pub(crate) confirming_delete: bool,
}

impl OpenPlaylist {
    /// The playlist's name.
    #[must_use]
    pub(crate) fn name(&self) -> &str {
        self.playlist.name()
    }

    /// The header's counts line: `40 tracks · 1:12:40`, with `38 of 40 · 2
    /// missing` in front of it when entries are missing (ADR-0024 §3 — the
    /// page says what the queue and the file disagree about, before the music
    /// starts).
    #[must_use]
    pub(crate) fn counts_line(&self) -> String {
        let total = self.rows.len();
        let mut parts: Vec<String> = Vec::new();
        if self.missing > 0 {
            parts.push(format!(
                "{} of {} · {} missing",
                total - self.missing,
                total,
                self.missing
            ));
        } else {
            parts.push(match total {
                1 => "1 track".to_owned(),
                n => format!("{n} tracks"),
            });
        }
        let time = self.queue.total_time();
        if time > Duration::ZERO {
            parts.push(vm::format_duration(time));
        }
        parts.join(" · ")
    }
}

/// How the initial entries of an unsaved playlist draft are supplied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CreationMode {
    Manual,
    Vibe,
}

/// Session-only state for the shallow, resumable creation flow.
#[derive(Debug)]
pub(crate) struct CreationDraft {
    pub(crate) mode: Option<CreationMode>,
    pub(crate) name: String,
    pub(crate) name_is_suggested: bool,
    pub(crate) items: Vec<QueueItemVm>,
    pub(crate) error: Option<String>,
    pub(crate) saved: bool,
    /// Which draft row the pointer is on, so the row's card can be drawn
    /// behind its editing controls as well as behind its body (item 53).
    /// Session state about a pointer: nothing decides anything from it.
    pub(crate) hovered_row: Option<usize>,
}

impl Default for CreationDraft {
    fn default() -> Self {
        Self {
            mode: None,
            name: String::new(),
            name_is_suggested: true,
            items: Vec::new(),
            error: None,
            saved: false,
            hovered_row: None,
        }
    }
}

/// A deterministic, visible starting name derived from the listener's first
/// semantic phrase. Storage validation and collision suffixing still happen
/// before Save, in the editable field.
#[must_use]
pub(crate) fn suggested_name(prompt: &str) -> String {
    let first = prompt
        .split([',', ';', '\n'])
        .next()
        .unwrap_or_default()
        .trim();
    let structural = ["start with ", "start ", "begin with ", "begin "];
    let mut phrase = first;
    let lower = first.to_lowercase();
    for prefix in structural {
        if lower.starts_with(prefix) {
            phrase = first.get(prefix.len()..).unwrap_or(first).trim();
            break;
        }
    }
    let clean: String = phrase
        .chars()
        .map(|ch| {
            if matches!(ch, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                ' '
            } else {
                ch
            }
        })
        .collect();
    let clean = clean.split_whitespace().collect::<Vec<_>>().join(" ");
    if clean.is_empty() {
        return "Vibe playlist".to_owned();
    }
    if clean.chars().count() <= 48 {
        return clean;
    }
    let mut short = String::new();
    for word in clean.split_whitespace() {
        let extra = usize::from(!short.is_empty()) + word.chars().count();
        if short.chars().count() + extra > 48 {
            break;
        }
        if !short.is_empty() {
            short.push(' ');
        }
        short.push_str(word);
    }
    if short.is_empty() {
        "Vibe playlist".to_owned()
    } else {
        short
    }
}

/// The playlist surfaces' whole state, held by the shell beside the player.
#[derive(Debug)]
pub(crate) struct Playlists {
    /// The folder, or the logged reason there is none (a platform with no
    /// data directory). Every act checks; the panel says so in words.
    folder: Option<Folder>,
    /// The panel's index: every playlist, sorted as the folder lists them.
    pub(crate) rows: Vec<PanelRow>,
    /// The pinned built-in row, refreshed from durable library membership.
    pub(crate) favourite: PanelRow,
    /// The full Playlists place's session ordering.
    pub(crate) order: PlaylistOrder,
    /// The saved-playlist tile currently under the pointer.
    pub(crate) hovered: Option<u64>,
    /// Saved-playlist overview row awaiting an explicit trash confirmation.
    pub(crate) confirming_overview_delete: Option<u64>,
    /// Whether the panel is summoned. Session state, not config — which
    /// surface you were last collecting into is not a standing decision, the
    /// same argument that keeps `settings_section` out of `config.toml`.
    pub(crate) panel_open: bool,
    /// A pick in flight: the panel is serving as the picker.
    pub(crate) pending: Option<Pending>,
    /// The panel's `New playlist` field, while it is a field.
    pub(crate) naming: Option<NameEntry>,
    /// The queue place's `Save as playlist` field, while it is a field.
    pub(crate) saving_queue: Option<NameEntry>,
    /// The resumable, unsaved draft behind the canonical `New playlist`
    /// place. No file exists until its explicit Save action succeeds.
    pub(crate) creation: CreationDraft,
    /// The playlist whose page is open, if one is.
    pub(crate) open: Option<OpenPlaylist>,
    /// The open page's edit history: whole-item-list snapshots, newest last
    /// (doc 11 §5 P2). Keyed to [`Self::undo_for`] and cleared the moment
    /// the page it describes is no longer the page on screen — a snapshot
    /// applied to a different file would be the stale-index failure the
    /// fingerprint discipline exists to refuse.
    undo: crate::undo::History<Vec<Item>>,
    /// Which page id [`Self::undo`]'s snapshots belong to.
    undo_for: Option<u64>,
    /// Bumped on every [`Self::refresh`] and every [`Self::note_played`] — the
    /// shell's cue that the lane's lists half has moved, without diffing two
    /// vectors of strings.
    stamp: u64,
    /// **When each list was last played**, by [`playlist_id`], for as long as
    /// this process runs.
    ///
    /// Beside [`PanelRow::touched_unix_s`] rather than folded into it, because
    /// the two are different facts with different lifetimes: the mtime is the
    /// file's and survives a quit, and this is the run's and does not. The lane
    /// takes whichever is later ([`Self::touched`]).
    ///
    /// **It is not persisted, and that is a stated shortfall rather than an
    /// oversight** — see [`Self::note_played`].
    played: std::collections::HashMap<u64, u64>,
    /// How [`Self::delete_id`] removes the file: the **platform trash** in
    /// the product ([`Folder::delete_to_trash`], doc 11 §5 P2), a plain
    /// unlink under the test constructor's tempdir fixtures — where the real
    /// trash would mean a test writing outside its own directory, the XDG-isolation
    /// rule at test scale. The trash behaviour itself is pinned by the
    /// storage layer's isolated `tests/trash.rs`, and the wiring here is
    /// pinned by `the_product_deletes_to_the_trash_and_the_tests_do_not`.
    delete: fn(&Folder, &str) -> Result<(), PlaylistError>,
}

impl Playlists {
    /// The listener-owned playlist directory, when this platform supplied it.
    pub(crate) fn folder_path(&self) -> Option<&Path> {
        self.folder.as_ref().map(Folder::dir)
    }

    /// Open the surfaces over the user's own folder.
    pub(crate) fn start() -> Self {
        let folder = match Folder::open_default() {
            Ok(folder) => {
                crate::baz_log!("[playlists] folder: {}", folder.dir().display());
                Some(folder)
            }
            Err(error) => {
                crate::baz_log!("[playlists] unavailable: {error}");
                None
            }
        };
        let mut playlists = Self {
            folder,
            rows: Vec::new(),
            favourite: empty_favourites_row(),
            order: PlaylistOrder::default(),
            hovered: None,
            confirming_overview_delete: None,
            panel_open: false,
            pending: None,
            naming: None,
            saving_queue: None,
            creation: CreationDraft::default(),
            open: None,
            undo: crate::undo::History::new(),
            undo_for: None,
            stamp: 0,
            played: std::collections::HashMap::new(),
            delete: Folder::delete_to_trash,
        };
        playlists.refresh(None);
        playlists
    }

    /// Start (or resume) the canonical creation flow at its chooser.
    pub(crate) fn begin_creation(&mut self) {
        if self.creation.saved {
            self.creation = CreationDraft::default();
        }
        self.creation.saved = false;
    }

    /// Put a collision-safe prompt suggestion into the visible name field,
    /// unless the listener has already replaced that suggestion themselves.
    pub(crate) fn suggest_creation_name(&mut self, prompt: &str) {
        if !self.creation.name_is_suggested {
            return;
        }
        let base = suggested_name(prompt);
        let mut candidate = base.clone();
        let mut suffix = 2_usize;
        while self.holds(&candidate) {
            candidate = format!("{base} {suffix}");
            suffix = suffix.saturating_add(1);
        }
        self.creation.name = candidate;
        self.creation.error = None;
    }

    /// The storage-layer refusal for the visible draft name.
    #[must_use]
    pub(crate) fn creation_refusal(&self) -> Option<String> {
        if let Some(error) = &self.creation.error {
            return Some(error.clone());
        }
        let name = self.creation.name.trim();
        if name.is_empty() {
            return None;
        }
        if let Err(error) = baz_core::playlist::validate_name(name) {
            return Some(error.to_string());
        }
        self.holds(name)
            .then(|| format!("There is already a playlist called {name:?}."))
    }

    #[must_use]
    pub(crate) fn creation_can_save(&self, has_items: bool) -> bool {
        self.creation.mode.is_some()
            && !self.creation.name.trim().is_empty()
            && self.creation_refusal().is_none()
            && (self.creation.mode == Some(CreationMode::Manual) || has_items)
    }

    /// Save the current draft as one ordinary playlist. Manual and Vibe
    /// differ only in how their initial entries arrived; the resulting file
    /// has the same format and lifecycle.
    pub(crate) fn save_creation(
        &mut self,
        generated: Option<&crate::vibe::Generated>,
        library: &Library,
    ) -> Option<u64> {
        let has_items = generated.is_some_and(|draft| !draft.items.is_empty());
        if !self.creation_can_save(has_items) {
            return None;
        }
        let name = self.creation.name.trim().to_owned();
        let folder = self.folder.as_ref()?;
        let mut playlist = match folder.create(&name) {
            Ok(playlist) => playlist,
            Err(error) => {
                self.creation.error = Some(error.to_string());
                return None;
            }
        };
        if let Some(request) = generated {
            playlist
                .items_mut()
                .push(Item::Note(Note::from_text(&format!(
                    "# made by baz · {} · {}",
                    request.description,
                    request.pool_note().to_lowercase()
                ))));
            playlist
                .items_mut()
                .extend(request.items.iter().map(entry_for).map(Item::Entry));
        } else {
            playlist
                .items_mut()
                .extend(self.creation.items.iter().map(entry_for).map(Item::Entry));
        }
        if let Err(error) = playlist.save() {
            self.creation.error = Some(error.to_string());
            return None;
        }
        let id = playlist_id(playlist.name());
        self.creation.saved = true;
        self.refresh(Some(library));
        Some(id)
    }

    /// A surfaces value over an explicit folder — the test seam, exactly as
    /// [`Folder::open`] is the storage layer's.
    #[cfg(test)]
    fn over(folder: Folder) -> Self {
        let mut playlists = Self {
            folder: Some(folder),
            rows: Vec::new(),
            favourite: empty_favourites_row(),
            order: PlaylistOrder::default(),
            hovered: None,
            confirming_overview_delete: None,
            panel_open: false,
            pending: None,
            naming: None,
            saving_queue: None,
            creation: CreationDraft::default(),
            open: None,
            undo: crate::undo::History::new(),
            undo_for: None,
            stamp: 0,
            played: std::collections::HashMap::new(),
            delete: Folder::delete,
        };
        playlists.refresh(None);
        playlists
    }

    /// Re-list the folder and re-read every file's counts.
    ///
    /// Called when the panel opens and after every act on the shelf — never
    /// per frame. External edits are honoured here by re-reading (the mtime
    /// check on read is the whole mechanism; ADR-0024 refuses a watcher), so
    /// a file dropped into the folder appears the next time the panel is
    /// summoned: last writer wins, no prompt.
    pub(crate) fn refresh(&mut self, library: Option<&Library>) {
        self.stamp = self.stamp.wrapping_add(1);
        self.favourite = favourites_row(library);
        let Some(folder) = &self.folder else {
            return;
        };
        let listed = match folder.list() {
            Ok(listed) => listed,
            Err(error) => {
                crate::baz_log!("[playlists] cannot list the folder: {error}");
                return;
            }
        };
        let readings: Vec<(String, Playlist, Option<u64>)> = listed
            .iter()
            // The built-in owns this identity. A same-named imported file is
            // left untouched on disk but can neither replace nor duplicate it
            // in Baz's collection.
            .filter(|file| !file.name.eq_ignore_ascii_case("Favourites"))
            .filter_map(|file| {
                file.read().ok().map(|playlist| {
                    let created = std::fs::metadata(&file.path)
                        .and_then(|metadata| metadata.created())
                        .ok()
                        .and_then(|created| created.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|duration| duration.as_secs());
                    (file.name.clone(), playlist, created)
                })
            })
            .collect();
        // One walk of the index for every sleeve at once: each entry path
        // resolves to the id of the record it belongs to — the same identity
        // the wall's thumbnail cache is keyed by, which is what makes the
        // collage a read of that cache rather than a second pipeline
        // (ADR-0024 §A1).
        let records: HashMap<&Path, u64> = match library {
            Some(library) => {
                let wanted: std::collections::HashSet<&Path> = readings
                    .iter()
                    .flat_map(|(_, playlist, _)| {
                        playlist.entries().map(|entry| entry.path.as_path())
                    })
                    .collect();
                library
                    .tracks()
                    .filter(|meta| wanted.contains(meta.path.as_path()))
                    .map(|meta| {
                        (
                            meta.path.as_path(),
                            vm::album_id(AlbumArtist::of(meta), library.record_title(meta)),
                        )
                    })
                    .collect()
            }
            // No library yet (first construction): the rows list without
            // sleeves, and the first refresh with one fills them in.
            None => HashMap::new(),
        };
        self.rows = readings
            .iter()
            .map(|(name, playlist, created_unix_s)| {
                let mut entries = 0usize;
                let mut playable = 0usize;
                let mut seconds: Option<u64> = None;
                let mut art: Vec<u64> = Vec::new();
                for entry in playlist.entries() {
                    entries += 1;
                    if records.contains_key(entry.path.as_path()) || entry.path.is_file() {
                        playable += 1;
                    }
                    if let Some(declared) = entry.extinf.as_ref().and_then(|extinf| extinf.seconds)
                    {
                        seconds = Some(seconds.unwrap_or(0) + declared);
                    }
                    if art.len() < 4
                        && let Some(&record) = records.get(entry.path.as_path())
                        && !art.contains(&record)
                    {
                        art.push(record);
                    }
                }
                PanelRow {
                    id: playlist_id(name),
                    name: name.clone(),
                    entries,
                    seconds,
                    playable,
                    created_unix_s: *created_unix_s,
                    // The lane's order key, from the fingerprint the
                    // external-edit check already read: no extra `stat`, and
                    // nothing new is written to learn it.
                    touched_unix_s: playlist
                        .fingerprint()
                        .and_then(|stamp| u64::try_from(stamp.mtime_ns / 1_000_000_000).ok()),
                    art,
                    // Four `is_file` calls per list, on paths baz computes —
                    // the folder was already read for the list itself, and
                    // this is the same order of work as the `created` stat
                    // above.
                    image: folder.image_of(name),
                }
            })
            .collect();
    }

    /// Every saved playlist in the full page's selected order, **without** the
    /// pinned built-in row.
    ///
    /// This is the half the wall groups. `Favourites` is deliberately not in
    /// it: it is not alphabetically placed, it has no creation stamp, and
    /// filing it under `F` would put a built-in row inside a run of the
    /// listener's own lists (see [`Self::wall`]).
    pub(crate) fn ordered_saved(&self) -> Vec<&PanelRow> {
        let mut saved: Vec<&PanelRow> = self.rows.iter().collect();
        let by_name = |a: &&PanelRow, b: &&PanelRow| {
            (a.name.to_lowercase(), a.name.as_str()).cmp(&(b.name.to_lowercase(), b.name.as_str()))
        };
        match self.order {
            PlaylistOrder::Alphabetical => saved.sort_by(by_name),
            PlaylistOrder::Created => saved.sort_by(|a, b| {
                match (a.created_unix_s, b.created_unix_s) {
                    (Some(a), Some(b)) => b.cmp(&a),
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => std::cmp::Ordering::Equal,
                }
                .then_with(|| by_name(a, b))
            }),
            PlaylistOrder::Played => saved.sort_by(|a, b| {
                match (
                    self.played.get(&a.id).copied(),
                    self.played.get(&b.id).copied(),
                ) {
                    (Some(a), Some(b)) => b.cmp(&a),
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => std::cmp::Ordering::Equal,
                }
                .then_with(|| by_name(a, b))
            }),
        }
        saved
    }

    /// **The wall, as it is laid out** — the projection both the view and the
    /// artwork scheduler read, so neither can disagree with the other about
    /// which tile is where.
    ///
    /// The shape is the Library's, deliberately (the owner: *"a-z playlists
    /// should group alphabetically — use the exact same pattern as the
    /// library please"*): a list of cells in wall order plus a count per run,
    /// which is exactly what [`crate::shelf::Shelves`] lays out for the record
    /// wall. What differs is only what a cell and a header *mean*.
    ///
    /// **The lead run has no heading and holds two cells**: the create tile
    /// and `Favourites`. Neither belongs in a letter — one is a control and
    /// the other is a built-in — and an unlabelled leading run is how a wall
    /// says so without inventing a heading (`BUILT-IN` over one tile names a
    /// category with one member forever). It is the run the pinned layer must
    /// never draw: see [`Wall::pinned`].
    pub(crate) fn wall(&self) -> Wall<'_> {
        let mut cells = vec![Cell::New, Cell::List(&self.favourite)];
        let mut headers: Vec<Option<GroupHeaderVm>> = vec![None];
        let mut counts = vec![cells.len()];
        for playlist in self.ordered_saved() {
            let header = self.header_of(playlist);
            if headers.last() != Some(&Some(header.clone())) {
                headers.push(Some(header));
                counts.push(0);
            }
            if let Some(count) = counts.last_mut() {
                *count += 1;
            }
            cells.push(Cell::List(playlist));
        }
        Wall {
            cells,
            headers,
            counts,
            key: self.order.key(),
        }
    }

    /// Which group a saved playlist falls in, under the active ordering.
    ///
    /// The vocabulary is the Library's own [`GroupHeaderVm`], so one rail, one
    /// header band and one set of labels serve both collections — an A–Z rail
    /// is never painted over a date-sorted wall, and a heading always names
    /// the run it stands on.
    fn header_of(&self, playlist: &PanelRow) -> GroupHeaderVm {
        match self.order {
            PlaylistOrder::Alphabetical => {
                GroupHeaderVm::Initial(Initial::of(AlbumArtist::Named(&playlist.name)))
            }
            PlaylistOrder::Created => {
                GroupHeaderVm::Recency(recency(playlist.created_unix_s, true))
            }
            PlaylistOrder::Played => {
                GroupHeaderVm::Recency(recency(self.played_at(playlist.id), false))
            }
        }
    }

    /// The timestamp that defines the active `Played` ordering. Kept beside
    /// the ordering rather than inferred from the file's mtime: playback is a
    /// session fact and must not pretend an edit was a listen.
    #[must_use]
    pub(crate) fn played_at(&self, id: u64) -> Option<u64> {
        self.played.get(&id).copied()
    }

    /// The counter the returns lane watches: see [`Self::stamp`]'s field.
    #[must_use]
    pub(crate) fn stamp(&self) -> u64 {
        self.stamp
    }

    /// **A run reified from this list has started a track**: the list is now
    /// the most recently touched thing in the lane.
    ///
    /// The owner's defect, from the other side — see [`crate::lane::played_list`]
    /// for why the play attributes here rather than to the records the list
    /// quotes. Reports whether anything moved, so the caller does not re-sort
    /// the lane once per track of a run that is already at its head.
    ///
    /// **This is not persisted, and the shortfall is stated rather than
    /// hidden.** Across a quit a list falls back to its file's mtime, because
    /// the only thing baz writes about what was played is `baz-core`'s play
    /// ledger — which is per *path*, is appended by the engine, and is never
    /// told a run's provenance (the engine receives `SetQueue { paths }` and
    /// nothing else). Recording it would be a protocol field and a ledger
    /// format change, which is ADR-0018's decision to reopen and not a
    /// bug-fix's. `docs/BACKLOG.md` carries it.
    pub(crate) fn note_played(&mut self, id: u64, at: u64) -> bool {
        if self.played.get(&id) == Some(&at) {
            return false;
        }
        self.played.insert(id, at);
        self.stamp = self.stamp.wrapping_add(1);
        true
    }

    /// When a list was last **touched**: the later of its file's mtime and the
    /// last time a run reified from it started a track.
    ///
    /// The later of the two rather than one preferred over the other, because
    /// they are both true and the lane's order is *last touched*: editing a
    /// list you played an hour ago must move it, and playing a list you edited
    /// an hour ago must move it too.
    pub(crate) fn touched(&self, row: &PanelRow) -> Option<u64> {
        match (row.touched_unix_s, self.played.get(&row.id).copied()) {
            (Some(mtime), Some(played)) => Some(mtime.max(played)),
            (mtime, played) => mtime.or(played),
        }
    }

    /// Whether anything playlist-shaped can happen at all.
    #[must_use]
    pub(crate) fn available(&self) -> bool {
        self.folder.is_some()
    }

    /// The record page's reading of this state ([`Collecting`]).
    #[must_use]
    pub(crate) fn collecting(&self) -> Collecting {
        Collecting {
            available: self.available(),
            panel_open: self.panel_open,
        }
    }

    /// The panel row answering to `id`, if the folder still holds it.
    #[must_use]
    pub(crate) fn row(&self, id: u64) -> Option<&PanelRow> {
        self.rows.iter().find(|row| row.id == id)
    }

    /// Whether a list by this name stands in the folder **right now** — the
    /// folder's own answer, not [`Self::rows`]', because the rows are only
    /// refreshed while the panel is being used and the context menu asks at
    /// a right-press, panel or no panel (doc 09 §6: a rename or delete
    /// under the run withdraws the verb rather than letting it dangle).
    #[must_use]
    pub(crate) fn holds(&self, name: &str) -> bool {
        if name.eq_ignore_ascii_case("Favourites") {
            return true;
        }
        self.folder
            .as_ref()
            .and_then(|folder| folder.list().ok())
            .is_some_and(|listed| listed.iter().any(|file| file.name == name))
    }

    /// The `Playlists` door, and <kbd>Ctrl</kbd>+<kbd>P</kbd>: summon the
    /// panel, or close it. Closing puts down everything the panel was holding
    /// — a pick, a half-typed name — because a closed panel holding a pick
    /// would be an invisible mode.
    pub(crate) fn toggle_panel(&mut self, library: Option<&Library>) {
        if self.panel_open {
            self.close_panel();
        } else {
            self.refresh(library);
            self.panel_open = true;
        }
    }

    /// Close the panel and everything it was holding.
    pub(crate) fn close_panel(&mut self) {
        self.panel_open = false;
        self.pending = None;
        self.naming = None;
    }

    /// <kbd>Esc</kbd>'s share of the peel, panel layers only: the name field,
    /// then a pick in flight, then the panel itself. One layer per press,
    /// topmost first — the field is the newest thing on screen, the pick is
    /// the task the panel was opened for, and the panel is the surface both
    /// live on. Re-derived after the armed layer's removal (09 §9): the peel
    /// order is unchanged for what remains, one layer shorter. Reports
    /// whether a layer came off.
    pub(crate) fn peel(&mut self) -> bool {
        if !self.panel_open {
            return false;
        }
        if self.naming.take().is_some() {
            return true;
        }
        if self.pending.take().is_some() {
            return true;
        }
        self.close_panel();
        true
    }

    /// Begin a pick: hold what was pointed at and summon the panel to serve
    /// as the picker.
    pub(crate) fn begin_pick(
        &mut self,
        library: Option<&Library>,
        label: String,
        entries: Vec<Entry>,
        items: Vec<QueueItemVm>,
    ) {
        if entries.is_empty() {
            return;
        }
        self.pending = Some(Pending {
            label,
            entries,
            items,
        });
        if !self.panel_open {
            self.refresh(library);
            self.panel_open = true;
        }
    }

    /// Complete a pick on the picker's **Queue** row: put the pick down and
    /// hand its music back for `app.rs` to append to the run — the engine
    /// effect stays in the shell, exactly as `Play` and `Queue` do
    /// (09 §8.1). The panel stays open, as [`Self::pick`] leaves it.
    pub(crate) fn pick_queue(&mut self) -> Option<Pending> {
        self.pending.take()
    }

    /// The picker's named rows, in the picker's order (09 §8.1): the current
    /// playlist — the one `playing` names, while it still exists — hoisted
    /// first, every other list in the folder's own order after it. The
    /// **Queue** row above them and `New playlist` below are the view's; this
    /// is the ordering of the *files*, kept here so it is a tested fact
    /// rather than a rendering accident.
    #[must_use]
    pub(crate) fn picker_order(&self, playing: Option<u64>) -> Vec<&PanelRow> {
        let mut ordered: Vec<&PanelRow> = Vec::with_capacity(self.rows.len());
        ordered.extend(self.rows.iter().filter(|row| playing == Some(row.id)));
        ordered.extend(self.rows.iter().filter(|row| playing != Some(row.id)));
        ordered
    }

    /// Complete a pick on the row `id`: append the held entries and put the
    /// pick down. The panel stays open — the counts it shows have just
    /// changed, and closing it would hide the effect of the press.
    pub(crate) fn pick(&mut self, id: u64, library: &Library) {
        let Some(pending) = self.pending.take() else {
            return;
        };
        self.append(id, pending.entries, library);
    }

    /// Append `entries` to the playlist `id` — the one write every add layer
    /// ends in. Duplicates are allowed and unmarked (ADR-0024 §3): the
    /// gesture did what it said.
    pub(crate) fn append(&mut self, id: u64, entries: Vec<Entry>, library: &Library) {
        if entries.is_empty() {
            return;
        }
        let Some(folder) = &self.folder else {
            return;
        };
        let Some(row) = self.row(id) else {
            return;
        };
        let listed = match folder.list() {
            Ok(listed) => listed,
            Err(error) => {
                crate::baz_log!("[playlists] cannot list the folder: {error}");
                return;
            }
        };
        let Some(file) = listed.iter().find(|file| file.name == row.name) else {
            // Deleted under the panel; the refresh below takes the row off.
            self.refresh(Some(library));
            return;
        };
        // Freshly read, so the append lands on what the file holds *now* —
        // an external edit since the panel last looked is simply the base
        // this edit goes on top of (last writer wins, per file).
        let mut playlist = match file.read() {
            Ok(playlist) => playlist,
            Err(error) => {
                crate::baz_log!("[playlists] cannot read {}: {error}", file.path.display());
                return;
            }
        };
        let added = entries.len();
        // The list the append lands on, kept for the open page's history
        // when this file *is* the open page (doc 11 §5 P2 — an append is an
        // edit a hand can take back). Snapshotted from the fresh read, so
        // undo restores exactly what the file held the moment before this
        // write, external edits included.
        let before = playlist.items().to_vec();
        playlist
            .items_mut()
            .extend(entries.into_iter().map(Item::Entry));
        match playlist.save() {
            Ok(()) => crate::baz_log!(
                "[playlists] {added} added to {:?} ({} entries)",
                playlist.name(),
                playlist.entries().count()
            ),
            Err(error) => {
                crate::baz_log!("[playlists] could not save {:?}: {error}", playlist.name());
                return;
            }
        }
        if self.open.as_ref().is_some_and(|open| open.id == id) {
            self.record_undo(id, before);
        }
        self.refresh(Some(library));
        if self.open.as_ref().is_some_and(|open| open.id == id) {
            self.reload_open(library);
        }
    }

    /// **Why the name being typed cannot be saved yet**, in the storage
    /// layer's own words — or `None` when it can.
    ///
    /// The ghost row's `Save` control reads this and is inert while it is
    /// `Some`, which is the visible-control rule's other half: *a control
    /// that cannot act must not pretend it can*. The refusal is the one
    /// `Folder::create` would give, asked before the press rather than after
    /// it, so the words a listener reads are the words the act would have
    /// produced.
    ///
    /// An **empty** field is not a refusal — it is the field at rest, and a
    /// row that shouted at you for not having typed yet would be worse than
    /// one that waited. It reports `None` here and `false` from
    /// [`Self::naming_can_save`], which is exactly the difference between
    /// *nothing to say* and *nothing to do*.
    #[must_use]
    pub(crate) fn naming_refusal(&self) -> Option<String> {
        let naming = self.naming.as_ref()?;
        // The last submission's own refusal outranks anything derived: it is
        // what actually happened, and it may name something no check here can
        // see (a permission, a full disk).
        if let Some(error) = &naming.error {
            return Some(error.clone());
        }
        let name = naming.text.trim();
        if name.is_empty() {
            return None;
        }
        if let Err(error) = baz_core::playlist::validate_name(name) {
            return Some(error.to_string());
        }
        if self.holds(name) {
            return Some(format!("There is already a playlist called {name:?}."));
        }
        None
    }

    /// Whether the ghost row's `Save` may act: something typed, and nothing
    /// refusing it.
    #[must_use]
    pub(crate) fn naming_can_save(&self) -> bool {
        self.naming
            .as_ref()
            .is_some_and(|naming| !naming.text.trim().is_empty())
            && self.naming_refusal().is_none()
    }

    /// The panel's `New playlist` was submitted: create the file, and when a
    /// pick was in flight, complete it into the new list (create-from-a-record
    /// is two gestures, ADR-0024 §6).
    ///
    /// On refusal the storage layer's words land in the field's error line —
    /// surfaced plainly, not translated.
    pub(crate) fn submit_new(&mut self, library: &Library) {
        // A press that cannot act does nothing at all — the `Save` control is
        // already inert, and Enter is its accelerator, so the two must refuse
        // the same name in the same way (the mirror rule).
        if !self.naming_can_save() {
            return;
        }
        let Some(naming) = &mut self.naming else {
            return;
        };
        let name = naming.text.trim().to_owned();
        let Some(folder) = &self.folder else {
            return;
        };
        match folder.create(&name) {
            Ok(playlist) => {
                crate::baz_log!("[playlists] created {:?}", playlist.name());
                let id = playlist_id(playlist.name());
                self.naming = None;
                self.refresh(Some(library));
                if self.pending.is_some() {
                    self.pick(id, library);
                }
            }
            Err(error) => naming.error = Some(error.to_string()),
        }
    }

    /// The queue place's `Save as playlist` was submitted: tonight's run
    /// frozen into a new file, and nothing else — the queue is not linked to
    /// the playlist, and the file is not linked to the run (ADR-0024 §4).
    pub(crate) fn submit_queue_save(&mut self, queue: &QueueVm, library: Option<&Library>) {
        let Some(saving) = &mut self.saving_queue else {
            return;
        };
        let name = saving.text.trim().to_owned();
        let Some(folder) = &self.folder else {
            return;
        };
        match folder.create(&name) {
            Ok(mut playlist) => {
                playlist
                    .items_mut()
                    .extend(queue.items.iter().map(|item| Item::Entry(entry_for(item))));
                match playlist.save() {
                    Ok(()) => {
                        crate::baz_log!(
                            "[playlists] queue saved as {:?} ({} entries)",
                            playlist.name(),
                            queue.items.len()
                        );
                        self.saving_queue = None;
                        self.refresh(library);
                    }
                    Err(error) => saving.error = Some(error.to_string()),
                }
            }
            Err(error) => saving.error = Some(error.to_string()),
        }
    }

    /// Open the playlist `id`'s page: read the file and resolve its rows.
    /// Reports whether there is now a page to show.
    pub(crate) fn open_page(&mut self, id: u64, library: &Library) -> bool {
        // Re-list first: the id in hand may have been minted frames ago.
        self.refresh(Some(library));
        let Some(folder) = &self.folder else {
            return false;
        };
        let Some(row) = self.rows.iter().find(|row| row.id == id) else {
            return false;
        };
        let Ok(listed) = folder.list() else {
            return false;
        };
        let Some(file) = listed.iter().find(|file| file.name == row.name) else {
            return false;
        };
        match file.read() {
            Ok(playlist) => {
                // A history keyed to any other page does not survive the
                // swap. (Leaving the place already cleared it — P2's word
                // stands "until the next edit, a navigation, or the run
                // ending" — so this guard is the module keeping its own
                // invariant rather than trusting the shell to.)
                if self.undo_for != Some(id) {
                    self.clear_undo();
                }
                self.open = Some(resolve(id, playlist, library));
                self.sync_open_image();
                true
            }
            Err(error) => {
                crate::baz_log!("[playlists] cannot read {}: {error}", file.path.display());
                false
            }
        }
    }

    /// Re-read the open playlist from disk and resolve it again — the answer
    /// to any external edit, and to every saved one.
    pub(crate) fn reload_open(&mut self, library: &Library) {
        let Some(open) = &self.open else {
            return;
        };
        let (id, path) = (open.id, open.playlist.path().to_path_buf());
        match Playlist::read(&path) {
            Ok(playlist) => {
                self.open = Some(resolve(id, playlist, library));
                self.sync_open_image();
            }
            Err(error) => {
                // Deleted under the page. The shell draws the wall when the
                // place stops resolving; nothing to hold here — the edit
                // history included, which described a file that is gone.
                crate::baz_log!("[playlists] cannot read {}: {error}", path.display());
                self.open = None;
                self.clear_undo();
                self.refresh(Some(library));
            }
        }
    }

    /// Whether the open page still answers to `id` — the shell's per-frame
    /// question, mirroring [`crate::app::Shelf::album`].
    #[must_use]
    pub(crate) fn page(&self, id: u64) -> Option<&OpenPlaylist> {
        self.open.as_ref().filter(|open| open.id == id)
    }

    /// One guarded edit to the open playlist: fingerprint first, then the
    /// edit, then the atomic save, then a re-resolve (module docs). `edit`
    /// reports whether it changed anything; an untouched value is not saved.
    ///
    /// Every edit that goes through here records the list it replaced in the
    /// page's history (doc 11 §5 P2) — *after* the fingerprint check, so a
    /// snapshot is only ever of a state this process itself wrote or read
    /// whole, and only when the edit actually changed something.
    fn edit_open(&mut self, library: &Library, edit: impl FnOnce(&mut Playlist) -> bool) {
        self.edit_open_recording(true, library, edit);
    }

    /// [`Self::edit_open`], with the history push optional — `false` is the
    /// undo itself, which must not record the state it is removing (an undo
    /// that pushed would make <kbd>Ctrl</kbd>+<kbd>Z</kbd> a two-state
    /// toggle rather than a walk back through the edits).
    fn edit_open_recording(
        &mut self,
        record: bool,
        library: &Library,
        edit: impl FnOnce(&mut Playlist) -> bool,
    ) {
        let Some(open) = &mut self.open else {
            return;
        };
        if open.playlist.externally_edited() {
            // The press was aimed at rows the file no longer holds: re-read,
            // apply nothing (module docs — last writer wins is about files,
            // not about stale indices). The history goes with the stale
            // picture: its snapshots describe a lineage the disk has left.
            crate::baz_log!("[playlists] {:?} changed on disk; re-reading", open.name());
            self.clear_undo();
            self.reload_open(library);
            return;
        }
        let before = open.playlist.items().to_vec();
        let id = open.id;
        if !edit(&mut open.playlist) {
            return;
        }
        if let Err(error) = open.playlist.save() {
            crate::baz_log!("[playlists] could not save {:?}: {error}", open.name());
        } else if record {
            self.record_undo(id, before);
        }
        self.reload_open(library);
        self.refresh(Some(library));
    }

    /// Keep `before` as the open page's newest undo snapshot.
    fn record_undo(&mut self, id: u64, before: Vec<Item>) {
        if self.undo_for != Some(id) {
            self.undo.clear();
            self.undo_for = Some(id);
        }
        self.undo.push(before);
    }

    /// Forget the open page's edit history — the page was left, renamed,
    /// deleted, or overtaken by an external edit. Crate-visible because
    /// leaving the Playlist *place* is the shell's knowledge, not this
    /// module's.
    pub(crate) fn clear_undo(&mut self) {
        self.undo.clear();
        self.undo_for = None;
    }

    /// Whether the open page has an edit to take back — what decides if its
    /// `Undo` word is drawn.
    #[must_use]
    pub(crate) fn can_undo_open(&self) -> bool {
        self.open
            .as_ref()
            .is_some_and(|open| self.undo_for == Some(open.id))
            && self.undo.can_undo()
    }

    /// The page's `Undo` (and <kbd>Ctrl</kbd>+<kbd>Z</kbd> over it): put the
    /// file back as it stood before the last recorded edit — one atomic
    /// whole-file rewrite, through the same fingerprint guard as the edit it
    /// reverses. A file that changed on disk since refuses the restore and
    /// drops the whole history instead: the snapshots describe a lineage
    /// the disk has left, and "last writer wins" is about files, not about
    /// baz overwriting somebody's edit with its own memory.
    pub(crate) fn undo_open(&mut self, library: &Library) {
        if !self.can_undo_open() {
            return;
        }
        let Some(before) = self.undo.pop() else {
            return;
        };
        self.edit_open_recording(false, library, move |playlist| {
            *playlist.items_mut() = before;
            true
        });
    }

    /// The page's per-row ✕: take the entry at display row `row` out of the
    /// file. Missing entries have the control too — removing a dead
    /// reference is an edit the user makes, not one baz makes for them.
    pub(crate) fn remove_entry(&mut self, row: usize, library: &Library) {
        self.edit_open(library, |playlist| {
            let Some(at) = entry_indices(playlist).get(row).copied() else {
                return false;
            };
            playlist.items_mut().remove(at);
            true
        });
    }

    /// The page's ▲▼ steppers: move the entry at display row `row` one step
    /// up (`-1`) or down (`+1`), swapping with its neighbouring *entry* —
    /// notes keep their positions, because a rewrite never moves what it did
    /// not understand.
    pub(crate) fn shift_entry(&mut self, row: usize, delta: i32, library: &Library) {
        self.edit_open(library, |playlist| {
            let entries = entry_indices(playlist);
            let Some(&from) = entries.get(row) else {
                return false;
            };
            let neighbour = match delta {
                d if d < 0 => row.checked_sub(1),
                d if d > 0 => Some(row + 1),
                _ => None,
            };
            let Some(&to) = neighbour.and_then(|at| entries.get(at)) else {
                return false;
            };
            playlist.items_mut().swap(from, to);
            true
        });
    }

    /// The page's reorder **drag**, committed: take the entry at display row
    /// `from` out and put it back so it displays at row `to` — one edit, one
    /// atomic save (doc 09 §13 step 8; [`crate::drag`] holds the gesture,
    /// [`Self::shift_entry`] remains the steppers' route).
    ///
    /// Entries move; notes keep their positions relative to the entries
    /// around them exactly as [`Self::shift_entry`]'s swap leaves them —
    /// a rewrite never moves what it did not understand — and the insertion
    /// point is re-read *after* the removal, so a note between two entries
    /// cannot displace the landing.
    pub(crate) fn move_entry(&mut self, from: usize, to: usize, library: &Library) {
        if from == to {
            return;
        }
        self.edit_open(library, |playlist| {
            let entries = entry_indices(playlist);
            let Some(&lift) = entries.get(from) else {
                return false;
            };
            if to >= entries.len() {
                return false;
            }
            let item = playlist.items_mut().remove(lift);
            // Where the moved entry must sit to display at row `to`: before
            // the entry now occupying that display row, or at the very end.
            let remaining = entry_indices(playlist);
            let at = remaining
                .get(to)
                .copied()
                .unwrap_or_else(|| playlist.items().len());
            playlist.items_mut().insert(at, item);
            true
        });
    }

    /// The page's rename, submitted: a filesystem rename keeping the
    /// extension, refused by the storage layer's own rule, its words in the
    /// field. Returns the new id when it went through, so the place can move
    /// with the name.
    pub(crate) fn submit_rename(&mut self, library: &Library) -> Option<u64> {
        let open = self.open.as_mut()?;
        let renaming = open.renaming.as_mut()?;
        let to = renaming.text.trim().to_owned();
        let from = open.playlist.name().to_owned();
        if to == from {
            open.renaming = None;
            return None;
        }
        let folder = self.folder.as_ref()?;
        match folder.rename(&from, &to) {
            Ok(file) => {
                crate::baz_log!("[playlists] renamed {from:?} to {to:?}");
                // The id is the name, hashed, so a rename mints a new page
                // identity — the old id's history does not follow it.
                self.clear_undo();
                let id = playlist_id(&file.name);
                match file.read() {
                    Ok(playlist) => {
                        self.open = Some(resolve(id, playlist, library));
                        self.sync_open_image();
                    }
                    Err(_) => self.open = None,
                }
                self.refresh(Some(library));
                Some(id)
            }
            Err(error) => {
                renaming.error = Some(error.to_string());
                None
            }
        }
    }

    /// Re-read which picture the open page's list wears. Called wherever the
    /// page is installed or the folder's pictures change; two borrows rather
    /// than one because the answer comes from the folder and the question from
    /// the page.
    fn sync_open_image(&mut self) {
        let name = self.open.as_ref().map(|open| open.name().to_owned());
        let image = match (&self.folder, name) {
            (Some(folder), Some(name)) => folder.image_of(&name),
            _ => None,
        };
        if let Some(open) = &mut self.open {
            open.image = image;
        }
    }

    /// **Put a picture on a list**: copy `source` beside the `.m3u8` as its
    /// sleeve. The owner: *"lets allow setting an image/removing the image for
    /// a playlist."*
    ///
    /// Answers the path it landed at, or the reason it did not, in the words
    /// the surface shows. The refresh that follows is what makes the tile,
    /// the lane row and the page agree — every one of them reads the rows.
    pub(crate) fn set_image(
        &mut self,
        id: u64,
        source: &Path,
        library: Option<&Library>,
    ) -> Result<PathBuf, String> {
        let name = self
            .rows
            .iter()
            .find(|row| row.id == id)
            .map(|row| row.name.clone())
            .ok_or_else(|| "That playlist is no longer here.".to_owned())?;
        let folder = self
            .folder
            .as_ref()
            .ok_or_else(|| "Baz has no playlists folder to write to.".to_owned())?;
        match folder.set_image(&name, source) {
            Ok(path) => {
                crate::baz_log!("[playlists] {name:?} wears {}", path.display());
                self.refresh(library);
                self.sync_open_image();
                Ok(path)
            }
            Err(error) => {
                crate::baz_log!("[playlists] could not set an image on {name:?}: {error}");
                Err(error.to_string())
            }
        }
    }

    /// **Take the picture off again**, to the trash. The collage comes back,
    /// because the collage is what a playlist's sleeve is by default.
    ///
    /// `false` means there was nothing to take or the trash refused; both are
    /// logged, and neither is a state the interface has to say anything about
    /// beyond the sleeve it draws next.
    pub(crate) fn remove_image(&mut self, id: u64, library: Option<&Library>) -> bool {
        let Some(name) = self
            .rows
            .iter()
            .find(|row| row.id == id)
            .map(|row| row.name.clone())
        else {
            return false;
        };
        let Some(folder) = &self.folder else {
            return false;
        };
        match folder.remove_image(&name) {
            Ok(removed) => {
                if removed {
                    crate::baz_log!("[playlists] {name:?} is back to its collage");
                    self.refresh(library);
                    self.sync_open_image();
                }
                removed
            }
            Err(error) => {
                crate::baz_log!("[playlists] could not remove {name:?}'s image: {error}");
                false
            }
        }
    }

    /// Delete one saved playlist from either of its two doors. The file moves
    /// to the platform trash; the music stays.
    pub(crate) fn delete_id(&mut self, id: u64, library: Option<&Library>) -> bool {
        let Some(name) = self
            .rows
            .iter()
            .find(|row| row.id == id)
            .map(|row| row.name.clone())
        else {
            self.confirming_overview_delete = None;
            return false;
        };
        let Some(folder) = &self.folder else {
            self.confirming_overview_delete = None;
            return false;
        };
        match (self.delete)(folder, &name) {
            Ok(()) => {
                crate::baz_log!(
                    "[playlists] {name:?} moved to the trash — the file; the music stays"
                );
                if self.open.as_ref().is_some_and(|open| open.id == id) {
                    self.open = None;
                }
                self.confirming_overview_delete = None;
                self.hovered = None;
                self.clear_undo();
                self.refresh(library);
                true
            }
            Err(error) => {
                crate::baz_log!("[playlists] could not delete {name:?}: {error}");
                false
            }
        }
    }

    /// Compatibility door for the detail page and its existing focused tests.
    ///
    /// The shell exposes this only after the page's explicit confirmation.
    /// The operation remains reversible through the desktop trash; a refusal
    /// from that layer leaves the file exactly where it was and the page
    /// standing — nothing falls back to unlinking.
    #[cfg(test)]
    pub(crate) fn delete_open(&mut self, library: Option<&Library>) -> bool {
        self.open
            .as_ref()
            .map(|open| open.id)
            .is_some_and(|id| self.delete_id(id, library))
    }
}

/// The item indices of the entries, in order — what a display row number
/// means against a file that also holds notes.
fn entry_indices(playlist: &Playlist) -> Vec<usize> {
    playlist
        .items()
        .iter()
        .enumerate()
        .filter(|(_, item)| item.as_entry().is_some())
        .map(|(index, _)| index)
        .collect()
}

/// An M3U entry for a queue item: the path, and `#EXTINF` metadata built the
/// conventional way (`Artist - Title`) so the file reads well in a text
/// editor and in players with no library.
fn entry_for(item: &QueueItemVm) -> Entry {
    let artist = item
        .artist
        .as_deref()
        .or(item.album_artist.as_deref())
        .unwrap_or_default();
    let title = if artist.is_empty() {
        item.title.clone()
    } else {
        format!("{artist} - {}", item.title)
    };
    Entry {
        path: item.path.clone(),
        extinf: Some(ExtInf {
            seconds: item.duration.map(|duration| duration.as_secs()),
            title,
        }),
    }
}

/// The entries a run of **queue items** contributes — what the queue
/// place's `+` holds toward the picker (doc 09 §8.2): the same
/// `Artist - Title` `#EXTINF` convention as [`entries_for_tracks`], built
/// from the request-side record the rows themselves are drawn from.
#[must_use]
pub(crate) fn entries_for_items(items: &[QueueItemVm]) -> Vec<Entry> {
    items.iter().map(entry_for).collect()
}

/// The entries a record contributes: the selected edition's tracks, in the
/// page's own order, each with its `#EXTINF` line.
#[must_use]
pub(crate) fn entries_for_tracks(tracks: &[TrackVm], album_artist: &str) -> Vec<Entry> {
    tracks
        .iter()
        .map(|track| {
            let artist = track.artist.as_deref().unwrap_or(album_artist);
            let title = if artist.is_empty() {
                track.title.clone()
            } else {
                format!("{artist} - {}", track.title)
            };
            Entry {
                path: track.path.clone(),
                extinf: Some(ExtInf {
                    seconds: track.duration.map(|duration| duration.as_secs()),
                    title,
                }),
            }
        })
        .collect()
}

/// Resolve a playlist against the library: the page's rows, the playable
/// subset, and the queue `Play` sends — one pass, one value each.
#[expect(
    clippy::too_many_lines,
    reason = "one pass building the page's four readings together is what \
              guarantees they cannot disagree; splitting it would trade a \
              length lint for a second walk that could"
)]
fn resolve(id: u64, playlist: Playlist, library: &Library) -> OpenPlaylist {
    // One walk of the index rather than a query per entry: the entry set is
    // small and the walk is the sub-ms in-RAM corpus every keystroke already
    // sweeps.
    let wanted: std::collections::HashSet<&Path> = playlist
        .entries()
        .map(|entry| entry.path.as_path())
        .collect();
    let mut indexed: HashMap<&Path, &baz_core::library::TrackMeta> = HashMap::new();
    for meta in library.tracks() {
        if wanted.contains(meta.path.as_path()) {
            indexed.insert(meta.path.as_path(), meta);
        }
    }
    let mut rows: Vec<PageRow> = Vec::new();
    let mut tracks: Vec<TrackVm> = Vec::new();
    let mut items: Vec<QueueItemVm> = Vec::new();
    let mut missing = 0usize;
    // The sleeve's quotations: the first four distinct records, in order
    // (ADR-0024 §A1) — the same identity the wall's thumbnail cache is keyed
    // by, so the page's hero and the panel's tile read the cache the tiles
    // already fill. Beside it the *whole* distinct set, which the byline
    // states and which `art` deliberately cannot answer (see
    // `OpenPlaylist::records`): the walk is the same walk, and one `insert`
    // per resolved entry over a list of tens is not a cost worth a second
    // pass to avoid.
    let mut art: Vec<u64> = Vec::new();
    let mut records: std::collections::HashSet<u64> = std::collections::HashSet::new();
    for (position, entry) in playlist.entries().enumerate() {
        let stem = || {
            entry.path.file_stem().map_or_else(
                || entry.path.display().to_string(),
                |stem| stem.to_string_lossy().into_owned(),
            )
        };
        let meta = indexed.get(entry.path.as_path());
        if let Some(meta) = meta {
            let record = vm::album_id(AlbumArtist::of(meta), library.record_title(meta));
            if art.len() < 4 && !art.contains(&record) {
                art.push(record);
            }
            records.insert(record);
        }
        // The verdict (module docs): indexed, or on disk. The one `stat` per
        // unindexed entry happens here, at load.
        let playable = meta.is_some() || entry.path.is_file();
        let extinf_title = entry
            .extinf
            .as_ref()
            .map(|extinf| extinf.title.as_str())
            .filter(|title| !title.is_empty());
        // `Artist - Title` is the format's convention for the display title;
        // split it so an unindexed entry still reads as a track rather than
        // as one long dash-joined string.
        let (extinf_artist, extinf_track) = match extinf_title.and_then(|t| t.split_once(" - ")) {
            Some((artist, track)) => (Some(artist.to_owned()), Some(track.to_owned())),
            None => (None, extinf_title.map(str::to_owned)),
        };
        let title = meta
            .and_then(|meta| meta.title.clone())
            .or(extinf_track)
            .unwrap_or_else(stem);
        let artist = meta.and_then(|meta| meta.artist.clone()).or(extinf_artist);
        let duration = meta.and_then(|meta| meta.duration).or_else(|| {
            entry
                .extinf
                .as_ref()
                .and_then(|extinf| extinf.seconds)
                .map(Duration::from_secs)
        });
        let record = meta.and_then(|meta| {
            // The record's title, not the file's raw album tag, so a merged
            // multi-disc set has one stable identity and one sleeve.
            library.record_title(meta).map(|album| {
                (
                    vm::album_id(AlbumArtist::of(meta), Some(album)),
                    album.to_owned(),
                    meta.album_artist
                        .clone()
                        .or_else(|| meta.artist.clone())
                        .unwrap_or_default(),
                )
            })
        });
        let playable_position = playable.then_some(tracks.len());
        // The row's own artist line is carried only when the record's header
        // does not already cover it — the album page's and the queue's shared
        // rule, so a run of one record reads as one record rather than as its
        // artist stated once per row.
        let own_artist = artist.clone().filter(|artist| {
            record
                .as_ref()
                .is_none_or(|(_, _, album_artist)| album_artist != artist)
        });
        if playable {
            tracks.push(TrackVm {
                disc: None,
                number: None,
                title: title.clone(),
                artist: artist.clone(),
                duration,
                path: entry.path.clone(),
                bytes: None,
            });
            items.push(QueueItemVm {
                title: title.clone(),
                artist: own_artist.clone(),
                album: record.as_ref().map(|(_, album, _)| album.clone()),
                album_artist: record.as_ref().map(|(_, _, artist)| artist.clone()),
                duration,
                path: entry.path.clone(),
            });
        } else {
            missing += 1;
        }
        rows.push(PageRow {
            position: position + 1,
            title,
            // A flat playlist row carries its artist itself. Fall back to the
            // record artist when the track tag does not repeat it.
            artist: artist.or_else(|| {
                record
                    .as_ref()
                    .map(|(_, _, artist)| artist.clone())
                    .filter(|artist| !artist.is_empty())
            }),
            album: record.as_ref().map(|(_, album, _)| album.clone()),
            album_id: record.as_ref().map(|(id, _, _)| *id),
            duration: duration.map(vm::format_duration).unwrap_or_default(),
            missing: !playable,
            playable_position,
            path: entry.path.clone(),
        });
    }
    // The queue's own header names the first record, exactly as a stacked
    // queue's does; a playlist opening on loose tracks is headed by the
    // playlist's name, which is the truest thing there is to say about it.
    let (album, artist) = items
        .first()
        .map(|item| (item.album.clone(), item.album_artist.clone()))
        .unwrap_or_default();
    let queue = QueueVm {
        album,
        artist: artist.unwrap_or_else(|| playlist.name().to_owned()),
        items,
        origin: Some(crate::origin::Origin::playlist(playlist.name())),
        // Playing provenance (09 §6): a queue reified from this *file*
        // carries the file's name — origin, never a live link. Set here, in
        // the one place the playable subset becomes a queue record, so
        // `Play` and a row click cannot disagree about where the run is
        // from.
        //
        // **It is a `named` list**, and named lists offer no save word: the
        // owner, 2026-08-10, *"adding more stuff to an existing playlist is
        // fine, that does not need a save -- it's a low bar to edit a
        // playlist"*. Editing this run still never touches the file
        // (ADR-0024 §1); the page is the route to that.
        source: RunSource::Playlist(playlist.name().to_owned()),
    };
    OpenPlaylist {
        id,
        playlist,
        rows,
        tracks,
        queue,
        missing,
        art,
        records: records.len(),
        // Filled in by `Playlists::sync_open_image` the moment this is
        // installed: `resolve` reads the library, and where the picture lives
        // is the folder's business.
        image: None,
        renaming: None,
        confirming_delete: false,
    }
}

#[cfg(test)]
mod tests {
    use baz_core::playlist::Note;
    use baz_core::replaygain::ReplayGainTags;

    use super::*;

    fn folder() -> (tempfile::TempDir, Folder) {
        let dir = tempfile::tempdir().expect("tempdir");
        let folder = Folder::open(dir.path().join("playlists")).expect("open");
        (dir, folder)
    }

    /// **The line under a name declares its kind in its first token**
    /// (ADR-0024 §A3.1) — and the property being pinned is not the wording,
    /// it is that a made thing's line and a found thing's line are **never
    /// the same shape**.
    ///
    /// That is the one thing a screenshot cannot check. Before this, a
    /// playlist's line was `14` and a record's was `Anne-Marie Puig`: two
    /// strings in one slot, at one size, in one ink, with nothing but their
    /// content to tell a reader which kind of object he was looking at — and
    /// `14` did not read as a count, it read as a name truncated to nothing.
    /// The record arm is asserted here beside it, from the same source the
    /// lane builds it from (`app::App::sync_lane`'s record arm takes
    /// `album.artist.label()`), because *neither string alone* is the
    /// property.
    #[test]
    fn the_line_under_a_name_declares_its_kind_in_its_first_token() {
        let timed = PanelRow {
            id: playlist_id("Road Trip"),
            name: "Road Trip".to_owned(),
            entries: 14,
            seconds: Some(2530),
            playable: 14,
            created_unix_s: None,
            touched_unix_s: None,
            art: Vec::new(),
            image: None,
        };
        assert_eq!(timed.counts(), "Playlist · 14 · 42:10");

        // A bare imported path list declares no times, so the row says what it
        // knows and does not claim `0:00` about music it has not measured —
        // but it still leads with the noun, because the noun is the point.
        let untimed = PanelRow {
            seconds: None,
            ..timed.clone()
        };
        assert_eq!(untimed.counts(), "Playlist · 14");

        // The shapes, against each other. A found thing's line is a person's
        // name; a made thing's opens with the kind and then counts. No made
        // line can be mistaken for a found one at a glance, at any count.
        let found = "Anne-Marie Puig";
        for made in [timed.counts(), untimed.counts()] {
            assert!(
                made.starts_with("Playlist · "),
                "a made thing names its kind first: {made}"
            );
            assert!(
                !found.starts_with("Playlist · "),
                "…and a found thing never can, because its line is an artist"
            );
            assert_ne!(made, found);
        }

        // **And it costs no geometry.** The tightest surface the string
        // reaches is the lane's row: its text lane is
        // [`theme::SIDEBAR_ROW_TEXT_W`] 146 px (232 − the rail's two pads and
        // sleeve, both seams and the lamp slot), set at SIZE_META 12 with
        // `Wrapping::None` inside a container that clips. Measured with the
        // real bundled face — the same `font::ttf` reader `font.rs`'s own
        // slot tests use — the ordinary form sets in with room, which is what
        // the rule was costed at (design 14 §5.3).
        //
        // A list of four or five figures would run past 146 and be **clipped**
        // — never reflowed, never taller, never pushing a row down: the
        // container's `clip(true)` and the row's fixed SIDEBAR_ROW_H see to
        // that, and it is the same truncation a long artist name has always
        // taken in the same slot. So the claim under test is *the ordinary
        // line fits*, and the guarantee under every line is *the row does not
        // move*.
        let sans = crate::font::tests::ttf::Face::parse(crate::font::SANS_REGULAR);
        let measure = crate::theme::SIDEBAR_ROW_TEXT_W;
        for line in [timed.counts(), untimed.counts()] {
            let width = sans.width(&line, crate::theme::SIZE_META);
            assert!(
                width < measure,
                "`{line}` sets in {width} px against the lane's {measure}"
            );
        }
    }

    #[test]
    fn the_full_page_orders_by_name_creation_date_or_last_played() {
        let (_keep, folder) = folder();
        let mut playlists = Playlists::over(folder);
        playlists.creation.mode = Some(CreationMode::Vibe);
        playlists.creation.name = "ambient music that slowly gathers momentum".to_owned();
        let row = |name: &str, created_unix_s| PanelRow {
            id: playlist_id(name),
            name: name.to_owned(),
            entries: 0,
            seconds: None,
            playable: 0,
            created_unix_s,
            touched_unix_s: None,
            art: Vec::new(),
            image: None,
        };
        playlists.rows = vec![
            row("beta", Some(20)),
            row("Alpha", Some(10)),
            row("Imported", None),
        ];

        // The wall's own reading, which is what the page draws: the create
        // tile, then the pinned built-in, then the ordering.
        let names = |playlists: &Playlists| {
            playlists
                .wall()
                .cells
                .iter()
                .map(|cell| match cell {
                    Cell::New => "+".to_owned(),
                    Cell::List(row) => row.name.clone(),
                })
                .collect::<Vec<_>>()
        };

        playlists.order = PlaylistOrder::Alphabetical;
        assert_eq!(
            names(&playlists),
            ["+", "Favourites", "Alpha", "beta", "Imported"]
        );

        playlists.order = PlaylistOrder::Created;
        assert_eq!(
            names(&playlists),
            ["+", "Favourites", "beta", "Alpha", "Imported"],
            "creation order is newest first, with unknown dates last"
        );

        playlists.note_played(playlist_id("Alpha"), 10);
        playlists.note_played(playlist_id("beta"), 20);
        playlists.order = PlaylistOrder::Played;
        assert_eq!(
            names(&playlists),
            ["+", "Favourites", "beta", "Alpha", "Imported"],
            "played order is most recent first, with never-played lists last"
        );
    }

    /// **The wall groups like the Library's**, and its lead run is the one
    /// exception, stated: the create tile and the built-in row stand together
    /// under no heading, and every other run is a letter or a bucket with its
    /// own.
    #[test]
    fn the_wall_groups_alphabetically_under_a_leading_unheaded_run() {
        let (_keep, folder) = folder();
        let mut playlists = Playlists::over(folder);
        let row = |name: &str| PanelRow {
            id: playlist_id(name),
            name: name.to_owned(),
            entries: 0,
            seconds: None,
            playable: 0,
            created_unix_s: None,
            touched_unix_s: None,
            art: Vec::new(),
            image: None,
        };
        playlists.rows = vec![row("Aubade"), row("apples"), row("Bricolage"), row("Zed")];
        playlists.order = PlaylistOrder::Alphabetical;
        let wall = playlists.wall();

        // Runs: [+ , Favourites] · A(2) · B(1) · Z(1).
        assert_eq!(wall.counts, vec![2, 2, 1, 1]);
        assert_eq!(wall.cells.len(), wall.counts.iter().sum::<usize>());
        assert_eq!(wall.headers[0], None, "the lead run carries no heading");
        assert!(
            !wall.pinned(0),
            "and therefore may never be pinned — an opaque band with nothing in \
             it is a blank strip over the covers under it"
        );
        assert_eq!(
            wall.rail_headers()
                .iter()
                .map(GroupHeaderVm::label)
                .collect::<Vec<_>>(),
            ["A", "B", "Z"],
            "the rail indexes the lettered runs and not the lead"
        );
        for (entry, run) in [(0, 1), (1, 2), (2, 3)] {
            assert_eq!(
                Wall::run_of(entry),
                run,
                "a rail entry names the run one past the lead"
            );
            assert!(wall.pinned(run));
        }

        // Favourites is **not** filed under F. It is a built-in with no
        // creation stamp and no alphabetical place among the listener's own
        // lists, and the lead run is where the wall says so.
        assert!(matches!(wall.cells[1], Cell::List(row) if row.name == "Favourites"));
        assert!(
            !wall
                .rail_headers()
                .iter()
                .any(|header| header.label() == "F"),
            "no letter run exists for a list that is not in one"
        );
    }

    /// The two unavailable-timestamp states stay distinct, because one is
    /// "the filesystem could not say" and the other is "you have not played
    /// it", and a wall that collapsed them would claim the first about the
    /// second.
    #[test]
    fn unavailable_timestamps_keep_created_and_played_honest() {
        assert_eq!(recency(None, true), Recency::Unrecorded);
        assert_eq!(recency(None, false), Recency::Never);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        assert_eq!(recency(Some(now), true), Recency::Today);
    }

    /// An absolute fixture path by the platform's own rule — the same lesson
    /// the storage layer's tests carry.
    fn track(path: &str) -> PathBuf {
        if cfg!(windows) {
            PathBuf::from(format!("C:{path}"))
        } else {
            PathBuf::from(path)
        }
    }

    fn meta(artist: &str, album: &str, title: &str, path: &str) -> baz_core::library::TrackMeta {
        baz_core::library::TrackMeta {
            path: track(path),
            artist: Some(artist.to_owned()),
            album_artist: Some(artist.to_owned()),
            compilation: None,
            genre: None,
            album: Some(album.to_owned()),
            title: Some(title.to_owned()),
            track: Some(1),
            disc: None,
            year: None,
            duration: Some(Duration::from_secs(100)),
            format: None,
            bit_depth: None,
            sample_rate: None,
            bitrate: None,
            stamp: None,
            replay_gain: ReplayGainTags::default(),
        }
    }

    fn library() -> Library {
        let mut library = Library::open_in_memory().expect("library");
        library
            .add_tracks(vec![
                meta(
                    "Low",
                    "Things We Lost",
                    "Sunflower",
                    "/m/low/sunflower.flac",
                ),
                meta(
                    "Low",
                    "Things We Lost",
                    "Dinosaur Act",
                    "/m/low/dinosaur.flac",
                ),
                meta("Eno", "Apollo", "An Ending", "/m/eno/ascent.flac"),
            ])
            .expect("add");
        library
    }

    /// A queue item for a pick's hand, minimal on purpose: the picker tests
    /// care about what travels, not about catalogue completeness.
    fn item(title: &str, path: &str) -> QueueItemVm {
        QueueItemVm {
            title: title.to_owned(),
            artist: None,
            album: Some("Apollo".to_owned()),
            album_artist: Some("Eno".to_owned()),
            duration: Some(Duration::from_secs(260)),
            path: track(path),
        }
    }

    fn write_list(folder: &Folder, name: &str, entries: &[&str]) -> u64 {
        let mut playlist = folder.create(name).expect("create");
        for path in entries {
            playlist
                .items_mut()
                .push(Item::Entry(Entry::new(track(path))));
        }
        playlist.save().expect("save");
        playlist_id(name)
    }

    /// **A chosen picture reaches every surface that draws the list**, which
    /// is the only thing about item 52 that could quietly not work: the row
    /// feeds the wall's tiles, the panel and the lane, and the open page reads
    /// its own copy for the acts (`Set image…` becomes `Change image…`, and
    /// `Remove image` appears at all).
    ///
    /// The *removal* is not asserted here because it spends the platform
    /// trash, which a test process has no claim on; `Folder::remove_image` is
    /// the one line it is, and replacing a picture goes through it in
    /// `baz_core`'s own test.
    #[test]
    fn a_chosen_picture_reaches_the_row_and_the_open_page() {
        let (dir, folder) = folder();
        let library = library();
        write_list(&folder, "Road Trip", &["/m/eno/ascent.flac"]);
        let mut playlists = Playlists::over(folder);
        playlists.refresh(Some(&library));
        let id = playlist_id("Road Trip");
        assert!(
            playlists.rows.iter().all(|row| row.image.is_none()),
            "a list draws its collage until somebody says otherwise"
        );

        let chosen = dir.path().join("sleeve.png");
        std::fs::write(&chosen, b"bytes, never decoded in this test").expect("write");
        let landed = playlists
            .set_image(id, &chosen, Some(&library))
            .expect("set the image");
        assert!(landed.ends_with("Road Trip.png"));
        assert_eq!(
            playlists
                .rows
                .iter()
                .find(|row| row.id == id)
                .and_then(|row| row.image.clone()),
            Some(landed.clone()),
            "the row the tiles, the panel and the lane all read"
        );

        assert!(playlists.open_page(id, &library), "the page opens");
        assert_eq!(
            playlists.open.as_ref().and_then(|open| open.image.clone()),
            Some(landed),
            "and the page, whose acts depend on it"
        );
    }

    #[test]
    fn generated_playlist_is_an_ordinary_file_with_inert_local_provenance() {
        let (_dir, folder) = folder();
        let library = library();
        let mut playlists = Playlists::over(folder);
        playlists.creation.mode = Some(CreationMode::Vibe);
        playlists.creation.name = "ambient music that slowly gathers momentum".to_owned();
        let generated = crate::vibe::Generated {
            description: "ambient music that slowly gathers momentum · local semantic model"
                .to_owned(),
            request: "ambient music that slowly gathers momentum".to_owned(),
            items: vec![item("An Ending", "/m/eno/ascent.flac")],
            levels: Vec::new(),
            pool_tracks: 1,
            analyzed_tracks: 1,
            tempo_span: Some((72.0, 72.0)),
            target_minutes: 60,
        };
        let id = playlists
            .save_creation(Some(&generated), &library)
            .expect("a request with tracks creates a playlist");
        let row = playlists.row(id).expect("the ordinary listing includes it");
        assert_eq!(row.name, "ambient music that slowly gathers momentum");
        let file = playlists
            .folder
            .as_ref()
            .expect("folder")
            .list()
            .expect("list")
            .into_iter()
            .find(|file| file.name == row.name)
            .expect("file");
        let playlist = file.read().expect("read generated file");
        assert_eq!(playlist.entries().count(), 1);
        assert!(playlist.items().iter().any(|item| {
            matches!(item, Item::Note(note) if note.text().contains("ambient music that slowly gathers momentum · local semantic model"))
        }));
    }

    #[test]
    fn prompt_names_are_visible_safe_bounded_and_use_the_first_phrase() {
        assert_eq!(
            suggested_name(
                "Start sparse and nocturnal, build into restless electronic music, then finish warm"
            ),
            "sparse and nocturnal"
        );
        assert_eq!(
            suggested_name("dreamy shoegaze for a rainy evening"),
            "dreamy shoegaze for a rainy evening"
        );
        assert_eq!(suggested_name("///"), "Vibe playlist");
        assert!(suggested_name(&"word ".repeat(30)).chars().count() <= 48);
    }

    #[test]
    fn creation_does_not_write_until_the_explicit_save_boundary() {
        let (_dir, folder) = folder();
        let library = library();
        let mut playlists = Playlists::over(folder);
        playlists.begin_creation();
        playlists.creation.mode = Some(CreationMode::Manual);
        playlists.creation.name = "Road notes".to_owned();
        playlists
            .creation
            .items
            .push(item("An Ending", "/m/eno/ascent.flac"));
        assert!(playlists.rows.is_empty());
        let id = playlists
            .save_creation(None, &library)
            .expect("explicit save writes the draft");
        assert_eq!(
            playlists.row(id).map(|row| row.name.as_str()),
            Some("Road notes")
        );
    }

    #[test]
    fn the_id_is_the_name_and_nothing_else() {
        assert_eq!(playlist_id("Driving"), playlist_id("Driving"));
        assert_ne!(playlist_id("Driving"), playlist_id("driving"));
        assert_ne!(playlist_id("Driving"), playlist_id("Quiet"));
    }

    #[test]
    fn the_page_resolves_indexed_unindexed_and_missing_entries() {
        let (keep, folder) = folder();
        // An unindexed file that exists: plays anyway (ADR-0024 §3).
        let loose = keep.path().join("loose.flac");
        std::fs::write(&loose, b"\0").expect("write");
        let library = library();
        let mut playlists = Playlists::over(folder);
        let id = {
            let folder = playlists.folder.as_ref().expect("folder");
            let mut playlist = folder.create("Mix").expect("create");
            for path in [
                track("/m/low/sunflower.flac"),
                track("/m/low/dinosaur.flac"),
                loose.clone(),
                track("/gone/nowhere.flac"),
            ] {
                playlist.items_mut().push(Item::Entry(Entry::new(path)));
            }
            playlist.save().expect("save");
            playlist_id("Mix")
        };
        playlists.refresh(Some(&library));
        assert!(playlists.open_page(id, &library));
        let open = playlists.page(id).expect("open");
        assert_eq!(open.rows.len(), 4);
        assert_eq!(open.missing, 1);
        assert_eq!(open.tracks.len(), 3, "the playable subset");
        assert_eq!(open.queue.paths().len(), 3);
        // Indexed entries read from the index. A flat playlist row carries
        // its own artist, album and artwork identity rather than relying on a
        // group heading above it.
        assert_eq!(open.rows[0].title, "Sunflower");
        assert_eq!(open.rows[0].artist.as_deref(), Some("Low"));
        assert_eq!(open.rows[0].album.as_deref(), Some("Things We Lost"));
        assert_eq!(
            open.rows[0].album_id,
            Some(vm::album_id(
                AlbumArtist::Named("Low"),
                Some("Things We Lost")
            ))
        );
        assert!(!open.rows[0].missing);
        // The unindexed-but-present file plays, named from its stem.
        assert!(!open.rows[2].missing);
        assert_eq!(open.rows[2].title, "loose");
        // The missing entry stays, dimmed from its stem, out of the subset.
        assert!(open.rows[3].missing);
        assert_eq!(open.rows[3].title, "nowhere");
        assert_eq!(open.rows[3].playable_position, None);
        // `38 of 40 · 2 missing`, at this size.
        assert!(open.counts_line().starts_with("3 of 4 · 1 missing"));
    }

    #[test]
    fn reordering_swaps_entries_and_leaves_notes_where_they_were() {
        let (_keep, folder) = folder();
        let library = library();
        let mut playlists = Playlists::over(folder);
        let id = {
            let folder = playlists.folder.as_ref().expect("folder");
            let mut playlist = folder.create("Mix").expect("create");
            playlist
                .items_mut()
                .push(Item::Entry(Entry::new(track("/m/a.flac"))));
            playlist
                .items_mut()
                .push(Item::Note(Note::from_text("a comment between entries")));
            playlist
                .items_mut()
                .push(Item::Entry(Entry::new(track("/m/b.flac"))));
            playlist.save().expect("save");
            playlist_id("Mix")
        };
        playlists.refresh(Some(&library));
        assert!(playlists.open_page(id, &library));
        playlists.shift_entry(1, -1, &library);
        let open = playlists.page(id).expect("open");
        assert!(open.rows[0].path.ends_with("b.flac"));
        assert!(open.rows[1].path.ends_with("a.flac"));
        // The note is still on its own line in the file.
        let text = std::fs::read_to_string(open.playlist.path()).expect("read");
        assert!(text.contains("# a comment between entries"), "{text:?}");
        // The ends do not wrap.
        playlists.shift_entry(0, -1, &library);
        let open = playlists.page(id).expect("open");
        assert!(open.rows[0].path.ends_with("b.flac"), "the top stays put");
    }

    /// Doc 09 §13 step 8 — the drag's commit on the artefact: one lift, one
    /// landing, **one** saved file, the notes untouched on their own lines.
    #[test]
    fn a_drag_commit_repositions_one_entry_and_keeps_the_notes() {
        let (_keep, folder) = folder();
        let library = library();
        let mut playlists = Playlists::over(folder);
        let id = {
            let folder = playlists.folder.as_ref().expect("folder");
            let mut playlist = folder.create("Mix").expect("create");
            playlist
                .items_mut()
                .push(Item::Entry(Entry::new(track("/m/a.flac"))));
            playlist
                .items_mut()
                .push(Item::Note(Note::from_text("a comment between entries")));
            playlist
                .items_mut()
                .push(Item::Entry(Entry::new(track("/m/b.flac"))));
            playlist
                .items_mut()
                .push(Item::Entry(Entry::new(track("/m/c.flac"))));
            playlist.save().expect("save");
            playlist_id("Mix")
        };
        playlists.refresh(Some(&library));
        assert!(playlists.open_page(id, &library));
        // The head row, dragged past the tail: displays last, one edit.
        playlists.move_entry(0, 2, &library);
        let open = playlists.page(id).expect("open");
        assert!(open.rows[0].path.ends_with("b.flac"));
        assert!(open.rows[1].path.ends_with("c.flac"));
        assert!(open.rows[2].path.ends_with("a.flac"));
        let text = std::fs::read_to_string(open.playlist.path()).expect("read");
        assert!(text.contains("# a comment between entries"), "{text:?}");
        // Back up to the head — the landing is re-read after the removal,
        // so the note cannot displace it.
        playlists.move_entry(2, 0, &library);
        let open = playlists.page(id).expect("open");
        assert!(open.rows[0].path.ends_with("a.flac"));
        assert!(open.rows[1].path.ends_with("b.flac"));
        // A drop where the row already is, or from a row the page does not
        // have, asks for nothing.
        playlists.move_entry(1, 1, &library);
        playlists.move_entry(9, 0, &library);
        let open = playlists.page(id).expect("open");
        assert!(open.rows[0].path.ends_with("a.flac"));
        assert_eq!(open.rows.len(), 3);
    }

    #[test]
    fn a_stale_press_is_dropped_and_the_file_rereads() {
        let (_keep, folder) = folder();
        let library = library();
        let mut playlists = Playlists::over(folder);
        let id = {
            let folder = playlists.folder.as_ref().expect("folder");
            write_list(folder, "Mix", &["/m/a.flac", "/m/b.flac"])
        };
        playlists.refresh(Some(&library));
        assert!(playlists.open_page(id, &library));
        // vim adds a track under the open page: size changes, stamp changes.
        let path = playlists
            .page(id)
            .expect("open")
            .playlist
            .path()
            .to_path_buf();
        let mut bytes = std::fs::read(&path).expect("read");
        bytes.extend_from_slice(track("/m/c.flac").display().to_string().as_bytes());
        bytes.push(b'\n');
        std::fs::write(&path, &bytes).expect("write");
        // The press lands on a stale picture: nothing is removed, the page
        // re-reads, and the file still holds all three entries.
        playlists.remove_entry(0, &library);
        let open = playlists.page(id).expect("open");
        assert_eq!(open.rows.len(), 3, "the stale press removed nothing");
        // The next press acts on what is on screen.
        playlists.remove_entry(0, &library);
        assert_eq!(playlists.page(id).expect("open").rows.len(), 2);
    }

    /// **The name rules refuse before the press, not after it** — the ghost
    /// row's `Save` reads [`Playlists::naming_refusal`] and is inert while it
    /// speaks, so the refusal a listener reads is the one the act would have
    /// produced rather than the one it did.
    ///
    /// The words are the storage layer's, unchanged: baz does not translate a
    /// refusal into a friendlier lie about what happened.
    #[test]
    fn the_name_rules_refuse_before_the_press_in_the_storage_layers_words() {
        let (_keep, folder) = folder();
        let library = library();
        let mut playlists = Playlists::over(folder);

        // Nothing typed: nothing to say, and nothing to do. The two readings
        // are different on purpose — a row that shouted at you for not having
        // typed yet would be worse than one that waited.
        playlists.naming = Some(NameEntry::default());
        assert_eq!(playlists.naming_refusal(), None);
        assert!(!playlists.naming_can_save());

        // A broken rule, named before the press — and the press does nothing.
        playlists.naming = Some(NameEntry {
            text: "a/b".to_owned(),
            error: None,
        });
        let refusal = playlists.naming_refusal().expect("the refusal is surfaced");
        assert!(refusal.contains("path separator"), "{refusal:?}");
        assert!(!playlists.naming_can_save());
        playlists.submit_new(&library);
        assert!(
            playlists.naming.is_some(),
            "an inert control's accelerator must be inert too"
        );

        // A good name goes through, and the ghost returns.
        playlists.naming = Some(NameEntry {
            text: "Mix".to_owned(),
            error: None,
        });
        assert!(playlists.naming_can_save());
        playlists.submit_new(&library);
        assert!(playlists.naming.is_none(), "a good name goes through");

        // …and the collision it just created is refused in the same lane,
        // before the press, by name.
        playlists.naming = Some(NameEntry {
            text: "Mix".to_owned(),
            error: None,
        });
        let refusal = playlists
            .naming_refusal()
            .expect("the collision is surfaced");
        assert!(refusal.contains("already"), "{refusal:?}");
        assert!(!playlists.naming_can_save());
    }

    #[test]
    fn a_pick_appends_where_it_was_aimed_and_new_playlist_completes_it() {
        let (_keep, folder) = folder();
        let library = library();
        let mut playlists = Playlists::over(folder);
        let id = {
            let folder = playlists.folder.as_ref().expect("folder");
            write_list(folder, "Mix", &["/m/a.flac"])
        };
        playlists.refresh(Some(&library));
        playlists.begin_pick(
            Some(&library),
            "Add \u{201c}Apollo\u{201d}".to_owned(),
            vec![Entry::new(track("/m/eno/ascent.flac"))],
            vec![item("An Ending", "/m/eno/ascent.flac")],
        );
        assert!(playlists.panel_open, "the panel serves as the picker");
        playlists.pick(id, &library);
        assert!(playlists.pending.is_none());
        let row = playlists.row(id).expect("row");
        assert_eq!(row.entries, 2, "the record was appended");
        // A pick into a new playlist: two gestures, one file.
        playlists.begin_pick(
            Some(&library),
            "Add \u{201c}Apollo\u{201d}".to_owned(),
            vec![Entry::new(track("/m/eno/ascent.flac"))],
            vec![item("An Ending", "/m/eno/ascent.flac")],
        );
        playlists.naming = Some(NameEntry {
            text: "Fresh".to_owned(),
            error: None,
        });
        playlists.submit_new(&library);
        let fresh = playlists.row(playlist_id("Fresh")).expect("created");
        assert_eq!(fresh.entries, 1, "the pick completed into the new list");
    }

    /// **S4, headless, at the layer every route ends in** (doc 09 §4): the
    /// sounding song reaches the current playlist in two gestures from
    /// anywhere — right-click the bar, press `Add to "{name}"`.
    ///
    /// Given a queue whose provenance names a playlist that still exists;
    /// when the bar's menu is built, then the item is listed, naming the
    /// list, and its presses are exactly the sounding row's `+` and the
    /// picker's hoisted row; when those presses land here, then the *file*
    /// gains the track — its entries checked exactly, order and all, the
    /// fingerprint of the artefact — and **the run is untouched**: the
    /// queue record is bit-for-bit what it was (the decoupling rule,
    /// ADR-0024 §1, restated by 09 §6), and the shell arm the pick lands in
    /// reaches no engine at all, pinned in its source below.
    #[test]
    fn s4_the_bars_menu_sends_the_sounding_song_to_the_current_playlists_file_only() {
        use crate::app::Message;

        let (_keep, folder) = folder();
        let library = library();
        let mut playlists = Playlists::over(folder);
        let id = {
            let folder = playlists.folder.as_ref().expect("folder");
            write_list(folder, "Road Trip", &["/m/low/sunflower.flac"])
        };
        // Given: the run was reified from "Road Trip" and is sounding at
        // row 0. The record is a value; nothing below may change it.
        let run = QueueVm {
            album: Some("Apollo".to_owned()),
            artist: "Eno".to_owned(),
            items: vec![item("An Ending", "/m/eno/ascent.flac")],
            origin: Some(crate::origin::Origin::playlist("Road Trip")),
            source: RunSource::Playlist("Road Trip".to_owned()),
        };
        let before = run.clone();
        // The menu's facts, derived exactly as `App::menu_facts` derives
        // them: provenance, filtered through the folder's own listing.
        let current = run
            .provenance()
            .filter(|name| playlists.holds(name))
            .map(|name| (playlist_id(name), name.to_owned()));
        assert!(current.is_some(), "the file stands, so the verb does");
        let facts = crate::menu::Facts {
            engine_ready: true,
            collecting: playlists.available(),
            current,
            playing_album: Some(7),
            playing_queue_row: Some(0),
        };
        // When the bar is right-clicked: the item is listed, named —
        let listed = crate::menu::items(crate::menu::Target::NowPlaying, &facts);
        let add = listed
            .iter()
            .find(|entry| entry.label == "Add to \u{201c}Road Trip\u{201d}")
            .expect("S4's verb, naming the current playlist");
        // — and its presses are the sounding row's `+` then the hoisted
        // pick, in that order.
        assert!(
            matches!(
                add.presses.as_slice(),
                [Message::AddQueuedToPlaylist(0), Message::PickPlaylist(picked)]
                    if *picked == id
            ),
            "{:?}",
            add.presses
        );
        // When the presses land (their arms' own calls, at this layer):
        let sounding = &run.items[0];
        playlists.begin_pick(
            Some(&library),
            format!("Add \u{201c}{}\u{201d}", sounding.title),
            entries_for_items(std::slice::from_ref(sounding)),
            vec![sounding.clone()],
        );
        playlists.pick(id, &library);
        // Then the file gained the track — exactly, in order, and nothing
        // else changed in it.
        let written = playlists
            .folder
            .as_ref()
            .expect("folder")
            .list()
            .expect("list")
            .into_iter()
            .find(|file| file.name == "Road Trip")
            .expect("the file still stands")
            .read()
            .expect("read");
        let entries: Vec<PathBuf> = written.entries().map(|entry| entry.path.clone()).collect();
        assert_eq!(
            entries,
            vec![track("/m/low/sunflower.flac"), track("/m/eno/ascent.flac")],
            "the artefact's exact contents: what it held, then the append"
        );
        // …and the run is untouched: the record is what it was, provenance
        // included — the file is the kept thing, the run is tonight's
        // snapshot.
        assert_eq!(run, before, "the live queue is unchanged");
        // The shell arm the pick lands in reaches no engine — pinned in its
        // source, so a future "append to both" cannot arrive silently (the
        // tempting alternative 09 §6 weighed and refused).
        let shell = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/app.rs"),
        )
        .expect("the shell's source")
        .replace("\r\n", "\n");
        let arm = shell
            .split_once("Message::PickPlaylist(id) => {")
            .expect("the pick arm exists")
            .1;
        let arm = &arm[..arm.find("\n            }\n").expect("an arm ends")];
        for forbidden in ["playback.send", "UpdateQueue", "note_queue"] {
            assert!(
                !arm.contains(forbidden),
                "a file pick reached for `{forbidden}` — the append goes to \
                 the file only (09 §6)"
            );
        }
    }

    #[test]
    fn duplicates_append_unmarked_because_the_gesture_did_what_it_said() {
        let (_keep, folder) = folder();
        let library = library();
        let mut playlists = Playlists::over(folder);
        let id = {
            let folder = playlists.folder.as_ref().expect("folder");
            write_list(folder, "Mix", &["/m/low/sunflower.flac"])
        };
        playlists.refresh(Some(&library));
        playlists.append(
            id,
            vec![Entry::new(track("/m/low/sunflower.flac"))],
            &library,
        );
        assert_eq!(playlists.row(id).expect("row").entries, 2);
    }

    #[test]
    fn saving_the_queue_writes_extinf_and_refuses_bad_names_in_place() {
        let (_keep, folder) = folder();
        let mut playlists = Playlists::over(folder);
        let queue = QueueVm {
            album: Some("Apollo".to_owned()),
            artist: "Eno".to_owned(),
            items: vec![QueueItemVm {
                title: "An Ending".to_owned(),
                artist: Some("Eno".to_owned()),
                album: Some("Apollo".to_owned()),
                album_artist: Some("Eno".to_owned()),
                duration: Some(Duration::from_secs(260)),
                path: track("/m/eno/ascent.flac"),
            }],
            origin: None,
            source: RunSource::Fixed,
        };
        // Whitespace at the ends is trimmed rather than refused (the roots
        // field's own manner); what the storage layer's rule genuinely
        // refuses is surfaced in the field's words.
        playlists.saving_queue = Some(NameEntry {
            text: "night/late".to_owned(),
            error: None,
        });
        playlists.submit_queue_save(&queue, None);
        assert!(
            playlists
                .saving_queue
                .as_ref()
                .and_then(|saving| saving.error.as_deref())
                .is_some(),
            "the refusal is surfaced in the field"
        );
        playlists.saving_queue = Some(NameEntry {
            text: "Tonight".to_owned(),
            error: None,
        });
        playlists.submit_queue_save(&queue, None);
        assert!(playlists.saving_queue.is_none());
        let row = playlists.row(playlist_id("Tonight")).expect("saved");
        assert_eq!(row.entries, 1);
        assert_eq!(row.seconds, Some(260), "the EXTINF carried the length");
    }

    /// The peel: name field, then the pick, then the panel — one layer per
    /// press, and a closed panel holds nothing. Re-derived after the armed
    /// layer's removal (09 §9): what remains peels in the same order.
    #[test]
    fn escape_peels_the_panel_one_layer_at_a_time() {
        let (_keep, folder) = folder();
        let mut playlists = Playlists::over(folder);
        {
            let folder = playlists.folder.as_ref().expect("folder");
            write_list(folder, "Mix", &["/m/a.flac"]);
        }
        playlists.toggle_panel(None);
        playlists.pending = Some(Pending {
            label: String::new(),
            entries: vec![Entry::new(track("/m/a.flac"))],
            items: Vec::new(),
        });
        playlists.naming = Some(NameEntry::default());
        assert!(playlists.peel());
        assert!(playlists.naming.is_none(), "the field first");
        assert!(playlists.peel());
        assert!(playlists.pending.is_none(), "then the pick");
        assert!(playlists.peel());
        assert!(!playlists.panel_open, "then the panel itself");
        assert!(!playlists.peel(), "and a closed panel has no layers");
    }

    /// The sleeve's quotations (ADR-0024 §A1): the first four *distinct*
    /// records the library resolves, in playlist order — duplicates and
    /// later records quote nothing, unindexed entries contribute nothing,
    /// and the panel's reading and the page's are the same list.
    #[test]
    fn the_sleeve_quotes_the_first_four_distinct_records_in_order() {
        let (_keep, folder) = folder();
        let mut library = Library::open_in_memory().expect("library");
        let mut expected: Vec<u64> = Vec::new();
        let mut paths: Vec<String> = Vec::new();
        for n in 1..=5u32 {
            let path = format!("/m/band{n}/one.flac");
            let meta = meta(&format!("Band {n}"), &format!("Record {n}"), "One", &path);
            expected.push(vm::album_id(AlbumArtist::of(&meta), meta.album.as_deref()));
            paths.push(path);
            library.add_tracks(vec![meta]).expect("add");
        }
        let mut playlists = Playlists::over(folder);
        let id = {
            let folder = playlists.folder.as_ref().expect("folder");
            let mut playlist = folder.create("Mix").expect("create");
            // The first record twice (one quotation), an unindexed stranger
            // (none), then the rest — five records, four quoted.
            for path in [
                paths[0].as_str(),
                paths[0].as_str(),
                "/elsewhere/unindexed.flac",
                paths[1].as_str(),
                paths[2].as_str(),
                paths[3].as_str(),
                paths[4].as_str(),
            ] {
                playlist
                    .items_mut()
                    .push(Item::Entry(Entry::new(track(path))));
            }
            playlist.save().expect("save");
            playlist_id("Mix")
        };
        playlists.refresh(Some(&library));
        let row = playlists.row(id).expect("row");
        assert_eq!(row.art, expected[..4], "first four distinct, in order");
        assert!(playlists.open_page(id, &library));
        assert_eq!(
            playlists.page(id).expect("open").art,
            expected[..4],
            "the page quotes the same records the panel does"
        );
        // A list below four distinct records quotes what it has…
        let two = {
            let folder = playlists.folder.as_ref().expect("folder");
            let mut playlist = folder.create("Pair").expect("create");
            for path in [paths[0].as_str(), paths[1].as_str()] {
                playlist
                    .items_mut()
                    .push(Item::Entry(Entry::new(track(path))));
            }
            playlist.save().expect("save");
            playlist_id("Pair")
        };
        // …and an empty one quotes nothing at all: the rest tile's case.
        let bare = {
            let folder = playlists.folder.as_ref().expect("folder");
            folder.create("Bare").expect("create");
            playlist_id("Bare")
        };
        playlists.refresh(Some(&library));
        assert_eq!(playlists.row(two).expect("row").art, expected[..2]);
        assert_eq!(playlists.row(bare).expect("row").art, Vec::<u64>::new());
        // Without a library there is nothing to resolve against, and the
        // rows say so rather than guessing.
        playlists.refresh(None);
        assert_eq!(playlists.row(id).expect("row").art, Vec::<u64>::new());
    }

    /// The picker's ordering (09 §8.1, S4): the playing list is hoisted to
    /// the head of the named rows — second overall, under the view's Queue
    /// row — while it still exists; without provenance, or when the named
    /// file is gone, the folder's own order stands and nothing dangles.
    #[test]
    fn the_picker_hoists_the_playing_list_and_only_while_it_exists() {
        let (_keep, folder) = folder();
        let mut playlists = Playlists::over(folder);
        let (autumn, mix) = {
            let folder = playlists.folder.as_ref().expect("folder");
            (
                write_list(folder, "Autumn", &["/m/a.flac"]),
                write_list(folder, "Mix", &["/m/b.flac"]),
            )
        };
        playlists.refresh(None);
        // No provenance: the folder's own order (alphabetical listing).
        let names =
            |rows: Vec<&PanelRow>| rows.iter().map(|row| row.name.clone()).collect::<Vec<_>>();
        assert_eq!(names(playlists.picker_order(None)), ["Autumn", "Mix"]);
        // The playing list is hoisted first among the named rows…
        assert_eq!(names(playlists.picker_order(Some(mix))), ["Mix", "Autumn"]);
        // …hoisting the first row is a no-op rearrangement…
        assert_eq!(
            names(playlists.picker_order(Some(autumn))),
            ["Autumn", "Mix"]
        );
        // …and provenance naming a file that no longer exists hoists
        // nothing: a control that cannot act must not pretend it can.
        assert_eq!(
            names(playlists.picker_order(Some(playlist_id("Gone")))),
            ["Autumn", "Mix"]
        );
    }

    /// The picker's Queue row (09 §8.1): picking it hands the held music
    /// back for the shell's `UpdateQueue` append and writes **no file** —
    /// "hear this later" and "keep this" are the same gesture with a
    /// different destination, and each destination does only its own thing.
    #[test]
    fn a_queue_pick_hands_back_the_items_and_writes_no_file() {
        let (_keep, folder) = folder();
        let library = library();
        let mut playlists = Playlists::over(folder);
        let id = {
            let folder = playlists.folder.as_ref().expect("folder");
            write_list(folder, "Mix", &["/m/a.flac"])
        };
        playlists.refresh(Some(&library));
        playlists.begin_pick(
            Some(&library),
            "Add \u{201c}Apollo\u{201d}".to_owned(),
            vec![Entry::new(track("/m/eno/ascent.flac"))],
            vec![item("An Ending", "/m/eno/ascent.flac")],
        );
        let pending = playlists.pick_queue().expect("the pick in hand");
        assert_eq!(pending.items.len(), 1);
        assert_eq!(pending.items[0].title, "An Ending");
        assert!(playlists.pending.is_none(), "the pick is spent");
        assert!(playlists.panel_open, "the panel stays, as a file pick does");
        assert_eq!(
            playlists.row(id).expect("row").entries,
            1,
            "no file gained an entry from a queue pick"
        );
        assert!(playlists.pick_queue().is_none(), "nothing left to spend");
    }

    /// Playing provenance is set by reifying a playlist *file* (09 §6): the
    /// queue a page's `Play` sends carries the file's name, so the run can
    /// say what list it is from after the file and the snapshot part ways.
    #[test]
    fn a_resolved_playlist_queue_carries_the_files_name_as_provenance() {
        let (_keep, folder) = folder();
        let library = library();
        let mut playlists = Playlists::over(folder);
        let id = {
            let folder = playlists.folder.as_ref().expect("folder");
            write_list(folder, "Road Trip", &["/m/low/sunflower.flac"])
        };
        playlists.refresh(Some(&library));
        assert!(playlists.open_page(id, &library));
        assert_eq!(
            playlists.page(id).expect("open").queue.provenance(),
            Some("Road Trip")
        );
    }

    #[test]
    fn rename_moves_the_id_and_delete_reports_so_the_place_can_leave() {
        let (_keep, folder) = folder();
        let library = library();
        let mut playlists = Playlists::over(folder);
        let id = {
            let folder = playlists.folder.as_ref().expect("folder");
            write_list(folder, "Mix", &["/m/a.flac"])
        };
        playlists.refresh(Some(&library));
        assert!(playlists.open_page(id, &library));
        playlists.open.as_mut().expect("open").renaming = Some(NameEntry {
            text: "Late".to_owned(),
            error: None,
        });
        let renamed = playlists.submit_rename(&library).expect("renamed");
        assert_eq!(renamed, playlist_id("Late"));
        assert!(playlists.page(renamed).is_some());
        assert!(playlists.row(id).is_none(), "the old name is gone");
        assert!(playlists.delete_open(None));
        assert!(playlists.rows.is_empty());
    }

    #[test]
    fn overview_delete_uses_the_same_door_without_opening_or_playing_the_list() {
        let (_keep, folder) = folder();
        let mut playlists = Playlists::over(folder);
        let doomed = {
            let folder = playlists.folder.as_ref().expect("folder");
            let doomed = write_list(folder, "Doomed", &["/m/a.flac"]);
            write_list(folder, "Keep", &["/m/a.flac"]);
            doomed
        };
        playlists.refresh(None);
        assert!(playlists.open.is_none());
        playlists.confirming_overview_delete = Some(doomed);

        assert!(playlists.delete_id(doomed, None));
        assert!(playlists.row(doomed).is_none());
        assert_eq!(playlists.rows.len(), 1);
        assert!(playlists.open.is_none());
        assert_eq!(playlists.confirming_overview_delete, None);
    }

    /// The paths the open page's file holds, in order — what the undo tests
    /// compare before and after.
    fn open_paths(playlists: &Playlists) -> Vec<PathBuf> {
        playlists
            .open
            .as_ref()
            .expect("open page")
            .rows
            .iter()
            .map(|row| row.path.clone())
            .collect()
    }

    /// **Undo round-trips the page's edits** (doc 11 §5 P2): a remove and a
    /// reorder each restore the file exactly as it stood — on disk, not
    /// just on screen — and pressing `Undo` twice walks two steps back
    /// rather than toggling (an undo records nothing of its own).
    #[test]
    fn undo_round_trips_a_remove_and_a_reorder_on_disk() {
        let (_keep, folder) = folder();
        let library = library();
        let mut playlists = Playlists::over(folder);
        let id = {
            let folder = playlists.folder.as_ref().expect("folder");
            write_list(folder, "Mix", &["/m/a.flac", "/m/b.flac", "/m/c.flac"])
        };
        playlists.refresh(Some(&library));
        assert!(playlists.open_page(id, &library));
        assert!(
            !playlists.can_undo_open(),
            "an unedited page has no history"
        );
        let original = open_paths(&playlists);

        playlists.remove_entry(1, &library);
        assert_eq!(open_paths(&playlists).len(), 2);
        assert!(playlists.can_undo_open());
        playlists.shift_entry(0, 1, &library);
        let shifted = open_paths(&playlists);
        assert_ne!(shifted[0], original[0]);

        // Two edits, two steps back — newest first, no toggle.
        playlists.undo_open(&library);
        assert_eq!(open_paths(&playlists).len(), 2, "the reorder came back");
        playlists.undo_open(&library);
        assert_eq!(open_paths(&playlists), original, "the remove came back");
        assert!(!playlists.can_undo_open(), "the history is spent");

        // …and the restore reached the disk, not just the resolved rows.
        let on_disk = playlists
            .folder
            .as_ref()
            .expect("folder")
            .list()
            .expect("list")
            .into_iter()
            .find(|file| file.name == "Mix")
            .expect("the file")
            .read()
            .expect("read");
        let paths: Vec<PathBuf> = on_disk.entries().map(|entry| entry.path.clone()).collect();
        assert_eq!(paths, original);
    }

    /// **An append into the open page is undoable too** (P2's scope as
    /// adopted: remove, reorder, append) — the pick lands, `Undo` takes it
    /// back, and appends into *other* files record nothing (their pages
    /// carry no word to make the accelerator legal).
    #[test]
    fn undo_takes_back_an_append_into_the_open_page_only() {
        let (_keep, folder) = folder();
        let library = library();
        let mut playlists = Playlists::over(folder);
        let (id, other) = {
            let folder = playlists.folder.as_ref().expect("folder");
            (
                write_list(folder, "Mix", &["/m/a.flac"]),
                write_list(folder, "Other", &["/m/b.flac"]),
            )
        };
        playlists.refresh(Some(&library));
        assert!(playlists.open_page(id, &library));
        let original = open_paths(&playlists);

        // An append into a file that is not the open page: no history.
        playlists.append(
            other,
            vec![Entry::new(track("/m/eno/ascent.flac"))],
            &library,
        );
        assert!(!playlists.can_undo_open());

        // An append into the open page: one step of history.
        playlists.append(id, vec![Entry::new(track("/m/eno/ascent.flac"))], &library);
        assert_eq!(open_paths(&playlists).len(), 2);
        assert!(playlists.can_undo_open());
        playlists.undo_open(&library);
        assert_eq!(open_paths(&playlists), original);
    }

    /// **The fingerprint guard survives undo** (P2: provenance and
    /// fingerprint guards survive): a file edited under baz refuses the
    /// restore — the page re-reads the disk's truth instead — and the stale
    /// history goes with it, because its snapshots describe a lineage the
    /// disk has left.
    #[test]
    fn an_external_edit_refuses_the_undo_and_drops_the_stale_history() {
        let (_keep, folder) = folder();
        let library = library();
        let mut playlists = Playlists::over(folder);
        let id = {
            let folder = playlists.folder.as_ref().expect("folder");
            write_list(folder, "Mix", &["/m/a.flac", "/m/b.flac"])
        };
        playlists.refresh(Some(&library));
        assert!(playlists.open_page(id, &library));
        playlists.remove_entry(0, &library);
        assert!(playlists.can_undo_open());

        // Somebody else writes the file — vim, a sync tool, another baz.
        let path = playlists
            .folder
            .as_ref()
            .expect("folder")
            .list()
            .expect("list")
            .into_iter()
            .find(|file| file.name == "Mix")
            .expect("the file")
            .path;
        // A whole new list, and an mtime the fingerprint cannot mistake.
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(&path, "#EXTM3U\n/m/theirs.flac\n").expect("external edit");

        playlists.undo_open(&library);
        let after = open_paths(&playlists);
        assert_eq!(
            after,
            vec![track("/m/theirs.flac")],
            "the disk's truth stands; baz's memory does not overwrite it"
        );
        assert!(
            !playlists.can_undo_open(),
            "a history describing a lineage the disk left is dropped whole"
        );
    }

    /// **The product deletes to the trash; the fixtures do not.** The seam
    /// exists so a tempdir test never writes outside its own directory —
    /// the XDG-isolation rule at test scale — so this pin is what keeps the
    /// two wired the right way round: `start()` must hand `delete_open` the
    /// trash, and the trash behaviour itself is exercised for real in the
    /// storage layer's isolated `tests/trash.rs`.
    #[test]
    fn the_product_deletes_to_the_trash_and_the_tests_do_not() {
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/playlists.rs"),
        )
        .expect("this module's own source")
        .replace("\r\n", "\n");
        let start = source
            .split("fn start()")
            .nth(1)
            .and_then(|after| after.split("fn over(").next())
            .expect("start() precedes over()");
        assert!(
            start.contains("delete: Folder::delete_to_trash"),
            "the product's delete_open must go through the platform trash \
             (doc 11 §5 P2)"
        );
        // Assembled so the pin does not match its own text.
        let direct_call = String::from("folder.") + "delete_to_trash(";
        assert!(
            !source.contains(&direct_call),
            "delete_open reaches the trash through the seam, so the fixtures \
             can reach it with a plain unlink"
        );
    }

    /// **Playing a list touches it, and the lane takes whichever touch is
    /// later.**
    ///
    /// The two facts have different lifetimes and both are true — the mtime is
    /// the file's and survives a quit, the play is the run's and does not — so
    /// editing a list you played an hour ago moves it, and playing a list you
    /// edited an hour ago moves it too. Neither may hide the other.
    #[test]
    fn a_list_is_touched_by_being_played_as_well_as_by_being_edited() {
        let (_dir, folder) = folder();
        let mut playlists = Playlists::over(folder);
        {
            let folder = playlists.folder.as_ref().expect("folder");
            let mut playlist = folder.create("Road Trip").expect("create");
            playlist
                .items_mut()
                .push(Item::Entry(Entry::new(track("/m/a.flac"))));
            playlist.save().expect("save");
        }
        playlists.refresh(None);
        let row = playlists
            .rows
            .iter()
            .find(|r| r.name == "Road Trip")
            .cloned();
        let row = row.expect("the list is in the panel's index");
        let mtime = playlists.touched(&row);

        // A play *after* the file was written wins.
        let later = mtime.unwrap_or(0) + 3_600;
        assert!(playlists.note_played(row.id, later), "the list moved");
        assert_eq!(playlists.touched(&row), Some(later));

        // The same play again moves nothing — the lane is not re-sorted once
        // per track of a run that is already at its head.
        let before = playlists.stamp();
        assert!(!playlists.note_played(row.id, later));
        assert_eq!(
            playlists.stamp(),
            before,
            "an unchanged lane was re-stamped"
        );

        // A play *before* the file was written does not drag the row back:
        // the later of the two is the answer, whichever it is.
        let mut fresher = row.clone();
        fresher.touched_unix_s = Some(later + 60);
        assert_eq!(playlists.touched(&fresher), Some(later + 60));

        // …and a list on a filesystem with no usable mtime is still touched by
        // being played, rather than sorting as *moment unknown* forever.
        let mut stampless = row.clone();
        stampless.touched_unix_s = None;
        assert_eq!(playlists.touched(&stampless), Some(later));
    }
}
