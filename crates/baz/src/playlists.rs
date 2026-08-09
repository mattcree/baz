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
//! transfer gesture: a `+` or `Add to…` opens the panel as the picker, and
//! the pick lands where it is aimed — the Queue first, then the lists.
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
use std::time::Duration;

use baz_core::index::{AlbumArtist, Library};
use baz_core::playlist::{Entry, ExtInf, Folder, Item, Playlist};

use crate::vm::{self, QueueItemVm, QueueVm, TrackVm};

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
    /// `Add to…` as the always-visible route to the same picker).
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
    /// The sleeve's quotations (ADR-0024 §A1): the first four *distinct*
    /// records the library resolves, in playlist order — four for the 2 × 2
    /// collage, fewer meaning "draw the first full-bleed", none meaning the
    /// rest tile.
    pub(crate) art: Vec<u64>,
}

impl PanelRow {
    /// The row's counts line: `12 · 42:10`, or `12` when no time is known.
    #[must_use]
    pub(crate) fn counts(&self) -> String {
        match self.seconds {
            Some(seconds) => format!(
                "{} · {}",
                self.entries,
                vm::format_duration(Duration::from_secs(seconds))
            ),
            None => self.entries.to_string(),
        }
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
    /// `m:ss`, or empty when nothing declared a length.
    pub(crate) duration: String,
    /// Whether the path resolved to nothing: drawn dimmed, unplayable, and
    /// left in the file (ADR-0024 §3).
    pub(crate) missing: bool,
    /// The record this row opens a run of, when it is the first row of one —
    /// the queue place's group-header rule, over consecutive same-record
    /// runs.
    pub(crate) head: Option<(String, String)>,
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
    /// Whether `Delete` has been pressed once and is waiting for the
    /// confirming press (the Settings folder-Remove shape).
    pub(crate) delete_armed: bool,
    /// The rename field, while renaming.
    pub(crate) renaming: Option<NameEntry>,
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

/// The playlist surfaces' whole state, held by the shell beside the player.
#[derive(Debug)]
pub(crate) struct Playlists {
    /// The folder, or the logged reason there is none (a platform with no
    /// data directory). Every act checks; the panel says so in words.
    folder: Option<Folder>,
    /// The panel's index: every playlist, sorted as the folder lists them.
    pub(crate) rows: Vec<PanelRow>,
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
    /// The playlist whose page is open, if one is.
    pub(crate) open: Option<OpenPlaylist>,
}

impl Playlists {
    /// Open the surfaces over the user's own folder.
    pub(crate) fn start() -> Self {
        let folder = match Folder::open_default() {
            Ok(folder) => {
                println!("[playlists] folder: {}", folder.dir().display());
                Some(folder)
            }
            Err(error) => {
                println!("[playlists] unavailable: {error}");
                None
            }
        };
        let mut playlists = Self {
            folder,
            rows: Vec::new(),
            panel_open: false,
            pending: None,
            naming: None,
            saving_queue: None,
            open: None,
        };
        playlists.refresh(None);
        playlists
    }

    /// A surfaces value over an explicit folder — the test seam, exactly as
    /// [`Folder::open`] is the storage layer's.
    #[cfg(test)]
    fn over(folder: Folder) -> Self {
        let mut playlists = Self {
            folder: Some(folder),
            rows: Vec::new(),
            panel_open: false,
            pending: None,
            naming: None,
            saving_queue: None,
            open: None,
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
        let Some(folder) = &self.folder else {
            return;
        };
        let listed = match folder.list() {
            Ok(listed) => listed,
            Err(error) => {
                println!("[playlists] cannot list the folder: {error}");
                return;
            }
        };
        let readings: Vec<(String, Playlist)> = listed
            .iter()
            .filter_map(|file| {
                file.read()
                    .ok()
                    .map(|playlist| (file.name.clone(), playlist))
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
                    .flat_map(|(_, playlist)| playlist.entries().map(|entry| entry.path.as_path()))
                    .collect();
                library
                    .tracks()
                    .filter(|meta| wanted.contains(meta.path.as_path()))
                    .map(|meta| {
                        (
                            meta.path.as_path(),
                            vm::album_id(AlbumArtist::of(meta), meta.album.as_deref()),
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
            .map(|(name, playlist)| {
                let mut entries = 0usize;
                let mut seconds: Option<u64> = None;
                let mut art: Vec<u64> = Vec::new();
                for entry in playlist.entries() {
                    entries += 1;
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
                    art,
                }
            })
            .collect();
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
                println!("[playlists] cannot list the folder: {error}");
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
                println!("[playlists] cannot read {}: {error}", file.path.display());
                return;
            }
        };
        let added = entries.len();
        playlist
            .items_mut()
            .extend(entries.into_iter().map(Item::Entry));
        match playlist.save() {
            Ok(()) => println!(
                "[playlists] {added} added to {:?} ({} entries)",
                playlist.name(),
                playlist.entries().count()
            ),
            Err(error) => {
                println!("[playlists] could not save {:?}: {error}", playlist.name());
                return;
            }
        }
        self.refresh(Some(library));
        if self.open.as_ref().is_some_and(|open| open.id == id) {
            self.reload_open(library);
        }
    }

    /// The panel's `New playlist` was submitted: create the file, and when a
    /// pick was in flight, complete it into the new list (create-from-a-record
    /// is two gestures, ADR-0024 §6).
    ///
    /// On refusal the storage layer's words land in the field's error line —
    /// surfaced plainly, not translated.
    pub(crate) fn submit_new(&mut self, library: &Library) {
        let Some(naming) = &mut self.naming else {
            return;
        };
        let name = naming.text.trim().to_owned();
        let Some(folder) = &self.folder else {
            return;
        };
        match folder.create(&name) {
            Ok(playlist) => {
                println!("[playlists] created {:?}", playlist.name());
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
                        println!(
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
                self.open = Some(resolve(id, playlist, library));
                true
            }
            Err(error) => {
                println!("[playlists] cannot read {}: {error}", file.path.display());
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
            Ok(playlist) => self.open = Some(resolve(id, playlist, library)),
            Err(error) => {
                // Deleted under the page. The shell draws the wall when the
                // place stops resolving; nothing to hold here.
                println!("[playlists] cannot read {}: {error}", path.display());
                self.open = None;
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
    fn edit_open(&mut self, library: &Library, edit: impl FnOnce(&mut Playlist) -> bool) {
        let Some(open) = &mut self.open else {
            return;
        };
        if open.playlist.externally_edited() {
            // The press was aimed at rows the file no longer holds: re-read,
            // apply nothing (module docs — last writer wins is about files,
            // not about stale indices).
            println!("[playlists] {:?} changed on disk; re-reading", open.name());
            self.reload_open(library);
            return;
        }
        if !edit(&mut open.playlist) {
            return;
        }
        if let Err(error) = open.playlist.save() {
            println!("[playlists] could not save {:?}: {error}", open.name());
        }
        self.reload_open(library);
        self.refresh(Some(library));
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
                println!("[playlists] renamed {from:?} to {to:?}");
                let id = playlist_id(&file.name);
                match file.read() {
                    Ok(playlist) => self.open = Some(resolve(id, playlist, library)),
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

    /// The page's `Delete`, confirmed: the file goes; the music stays.
    /// Reports whether it went, so the shell can leave the page it was for.
    pub(crate) fn delete_open(&mut self, library: Option<&Library>) -> bool {
        let Some(open) = &self.open else {
            return false;
        };
        let Some(folder) = &self.folder else {
            return false;
        };
        let name = open.playlist.name().to_owned();
        match folder.delete(&name) {
            Ok(()) => {
                println!("[playlists] deleted {name:?} — the file; the music stays");
                self.open = None;
                self.refresh(library);
                true
            }
            Err(error) => {
                println!("[playlists] could not delete {name:?}: {error}");
                false
            }
        }
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
    let mut previous_record: Option<(String, String)> = None;
    // The sleeve's quotations: the first four distinct records, in order
    // (ADR-0024 §A1) — the same identity the wall's thumbnail cache is keyed
    // by, so the page's hero and the panel's tile read the cache the tiles
    // already fill.
    let mut art: Vec<u64> = Vec::new();
    for (position, entry) in playlist.entries().enumerate() {
        let stem = || {
            entry.path.file_stem().map_or_else(
                || entry.path.display().to_string(),
                |stem| stem.to_string_lossy().into_owned(),
            )
        };
        let meta = indexed.get(entry.path.as_path());
        if art.len() < 4
            && let Some(meta) = meta
        {
            let record = vm::album_id(AlbumArtist::of(meta), meta.album.as_deref());
            if !art.contains(&record) {
                art.push(record);
            }
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
            meta.album.as_ref().map(|album| {
                (
                    album.clone(),
                    meta.album_artist
                        .clone()
                        .or_else(|| meta.artist.clone())
                        .unwrap_or_default(),
                )
            })
        });
        // A record's name where its run begins — the first row of the page
        // included, because unlike the queue place this page's own header
        // names the playlist, not a record.
        let head = record.clone().filter(|this| {
            previous_record
                .as_ref()
                .is_none_or(|previous| previous != this)
        });
        let playable_position = playable.then_some(tracks.len());
        // The row's own artist line is carried only when the record's header
        // does not already cover it — the album page's and the queue's shared
        // rule, so a run of one record reads as one record rather than as its
        // artist stated once per row.
        let own_artist = artist.clone().filter(|artist| {
            record
                .as_ref()
                .is_none_or(|(_, album_artist)| album_artist != artist)
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
                album: record.as_ref().map(|(album, _)| album.clone()),
                album_artist: record.as_ref().map(|(_, artist)| artist.clone()),
                duration,
                path: entry.path.clone(),
            });
        } else {
            missing += 1;
        }
        previous_record = record;
        rows.push(PageRow {
            position: position + 1,
            title,
            artist: own_artist,
            duration: duration.map(vm::format_duration).unwrap_or_default(),
            missing: !playable,
            head,
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
        // Playing provenance (09 §6): a queue reified from this *file*
        // carries the file's name — origin, never a live link. Set here, in
        // the one place the playable subset becomes a queue record, so
        // `Play` and a row click cannot disagree about where the run is
        // from. Every other queue construction leaves it `None`.
        provenance: Some(playlist.name().to_owned()),
    };
    OpenPlaylist {
        id,
        playlist,
        rows,
        tracks,
        queue,
        missing,
        art,
        delete_armed: false,
        renaming: None,
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
        // Indexed entries read from the index — and the row carries no artist
        // of its own when the record's header already says it.
        assert_eq!(open.rows[0].title, "Sunflower");
        assert_eq!(open.rows[0].artist, None);
        assert!(!open.rows[0].missing);
        // The record's run is headed once, at its first row.
        assert_eq!(
            open.rows[0].head,
            Some(("Things We Lost".to_owned(), "Low".to_owned()))
        );
        assert_eq!(open.rows[1].head, None, "a run is headed once");
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

    #[test]
    fn the_name_rules_refusal_lands_in_the_fields_own_words() {
        let (_keep, folder) = folder();
        let library = library();
        let mut playlists = Playlists::over(folder);
        playlists.naming = Some(NameEntry {
            text: "a/b".to_owned(),
            error: None,
        });
        playlists.submit_new(&library);
        let naming = playlists.naming.as_ref().expect("the field survives");
        let error = naming.error.as_deref().expect("the refusal is surfaced");
        assert!(error.contains("path separator"), "{error:?}");
        // A taken name is refused in the same lane.
        playlists.naming = Some(NameEntry {
            text: "Mix".to_owned(),
            error: None,
        });
        playlists.submit_new(&library);
        assert!(playlists.naming.is_none(), "a good name goes through");
        playlists.naming = Some(NameEntry {
            text: "Mix".to_owned(),
            error: None,
        });
        playlists.submit_new(&library);
        let error = playlists
            .naming
            .as_ref()
            .and_then(|naming| naming.error.as_deref())
            .expect("the collision is surfaced");
        assert!(error.contains("already exists"), "{error:?}");
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
            provenance: None,
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
        assert_eq!(playlists.row(bare).expect("row").art, []);
        // Without a library there is nothing to resolve against, and the
        // rows say so rather than guessing.
        playlists.refresh(None);
        assert_eq!(playlists.row(id).expect("row").art, []);
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
            playlists
                .page(id)
                .expect("open")
                .queue
                .provenance
                .as_deref(),
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
}
