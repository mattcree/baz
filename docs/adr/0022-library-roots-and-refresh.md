# ADR-0022: Several music folders, roots as first-class, and three refreshes

**Status**: accepted (2026-08-08) · **replaces gate 2 of ADR-0010's removal
policy** · closes `docs/BACKLOG.md`'s *"The index has no notion of which root a
row came from"* · takes *watch folders* out of `VISION.md`'s bigger chapters and
answers it with a **no**

## Context

baz held **one** music folder. `config.rs` carried `music_dir:
Option<PathBuf>`, `app.rs` scanned it once at launch, and nothing happened
afterwards. The owner asked for two things:

> *"is there a way to point it at a different library(s)? we should allow
> selection"*
> *"I assume we index that stuff and have some sort of periodic refresh in the
> backlog (with force sync option)"*

The first is a feature. The second is three features that are easy to say in
one breath and must not be built as one thing.

There is also a debt underneath both. ADR-0010 §3 protects the index from being
destroyed by a scan with four gates, and the second of them is *"the path is
under the root just scanned"* — a `starts_with` on the path. That gate is
correct **only** while baz holds one folder. Its own ADR said so, in the
deferrals:

> *A per-root record. Gate 2 uses the root currently being scanned. An index
> that remembered which root each row came from could be stricter still, and
> would want a `roots` table — worth it when baz supports more than one music
> folder, not before.*

This is that moment. Adding folders without fixing the gate would ship a
removal rule that is wrong the first time somebody holds `~/Music` and
`~/Music/Live`, or a folder and a symlink into it — because then *both* roots'
`starts_with` answers "yes" for rows neither of them put there.

## Decision

### 1. Schema v8: every row records the root it was found under

`SCHEMA_VERSION` moves 7 → 8, adding one nullable column and one table by
`ALTER TABLE` / `CREATE TABLE` inside a single transaction with the
`user_version` bump — the same discipline as v2 – v7, so an interrupted upgrade
leaves a v7 database the next open migrates again.

```sql
ALTER TABLE tracks ADD COLUMN root BLOB;   -- OS-native bytes, like `path`
CREATE TABLE roots (
    path         BLOB PRIMARY KEY,          -- OS-native bytes
    last_scan_ns INTEGER                    -- ns since the epoch; NULL = never
) STRICT;
```

`root` is a blob in the same platform-native encoding as `tracks.path`, for the
same reason: a root is a path, and paths are not UTF-8.

**Two facts, two homes.** `roots` records what baz *knows about* a folder —
when a scan of it last completed — and **not** which folders the listener has
chosen. That stays in `config.toml`, which is the front end's file. The index
never has an opinion about which folders somebody wants; it only reports what it
has seen.

`root` is in the upsert's insert list *and* its update list, which is the
opposite of `first_seen_ns` (v7) and right for the opposite reason: a
first-seen is a fact about the past that a rescan must never disturb, and a root
is a fact about *now*. A listener who removes one folder and adds another
containing the same tree has re-homed those tracks, and the row should say so
the moment a walk reads it again. The update is `COALESCE`d so that a caller
which names no root leaves the recorded one alone — **saying nothing about a
row's root is not the same as clearing it**.

The write API is `Library::add_tracks_under(root, tracks)`. The root belongs to
the *batch*, not to each `TrackMeta`, and that is not a shortcut: a scan walks
one root at a time, so every entry it emits came from the root being walked,
and a per-track field would repeat one path a hundred thousand times. It is
also the field a scan is *entitled* to set — `TrackMeta` is what reading a
file's tags yields, and no file carries the name of a folder somebody pointed
baz at. In RAM the root sits on `IndexedTrack` beside `computed` and
`first_seen`, as an `Arc<Path>`: one allocation per distinct root for the whole
library.

### 2. The backfill, and why it is honest where ADR-0019's three were not

Existing rows get `NULL`. `NULL` means "no root recorded", and it is a **safe**
value rather than a gap: no root's scan may prune a row that belongs to none.
A migration that got the direction wrong here would delete libraries; this one
can only decline to prune.

It is then filled — properly, not guessed — by `Library::adopt_root(root)`,
which the front end calls at launch for each folder it is configured to hold.
A row is claimed only if it names **no** root *and* lies under **this** root.

**That backfill is knowable and correct**, and the contrast with the three
ADR-0019 refused for `first_seen_ns` is the whole argument. Those three (*now*,
`mtime_ns`, `id` order) each had to *invent* a fact nobody had ever recorded —
when a track entered somebody's collection. This one reads **two recorded
facts**: a pre-v8 baz held exactly one music folder, so every row it wrote came
from that folder; the config file still says which folder it was, and the row's
own path still confirms it is under it. Nothing is fabricated.

The one thing `baz-core` cannot do is make the claim itself, because the folder
is named in `config.toml` — the front end's file, which `baz-core` has never
read. So the migration adds the column and the code that *holds* the fact
states it, in one call, at the moment it also states which roots to scan.

A row under **none** of the configured folders stays rootless and is therefore
permanently unprunable. That is the honest answer for a file baz was pointed at
once and is not pointed at now — it is the stray-fixture case ADR-0010 §3
named — and it is reported rather than hidden: `Library::unrooted_tracks()`
counts it and the Settings place says how many there are and what to do about
them.

### 3. Removal's gate 2, replaced

ADR-0010's other three gates are correct and unchanged: **a scan that produced
no entry prunes nothing**, **no ancestor the walk reported `Failed`**, and
**positive `is_confirmed_gone`**. Gate 2 becomes:

> **The row names a root whose walk produced something this pass.**

Gates 1 and 2 fuse into one lookup, and gate 1 becomes *per root* rather than
per pass — which is the grain the danger actually has. With several folders
configured, one of them being an empty mount point must cost that folder its
pruning and cost the others nothing.

What the replacement buys, stated as the cases that used to be wrong:

| case | old gate 2 | new gate 2 |
|---|---|---|
| two roots, one nested in the other | scanning the inner root would prune the outer root's rows, since they pass `starts_with(inner)` when they are inside it | only rows recorded under the inner root are prunable |
| a root that is a symlink onto another's tree | the mirror's walk never sees the recorded path, and the prefix test says the row is its to judge | the mirror recorded nothing, so it may prune nothing |
| a row from a folder baz no longer holds | immortal, with no way to clear it | still unprunable by a scan, but the folder can be added back (adopting it) or its rows forgotten outright |
| an absent root among several | one missing folder ended the whole scan | the pass continues; nothing is pruned from any root, including the absent one |

The mechanics stay where ADR-0010 put them: `Library::remove_tracks` is still
deliberately dumb, every judgement is still in `scan::vanished`, and the worker
can still only ever nominate paths the caller handed it in
`Library::known_files` — which now carries the recorded root alongside the
stamp (`library::KnownFile`).

### 4. Removing a folder forgets its tracks

`Library::forget_root(root)` deletes every row **recorded under** that root, and
the root's own record. Keyed on the record, never on a path prefix, so a nested
folder the listener kept does not lose the rows it holds.

The alternative — keep the rows until something confirms the files are gone —
was rejected because it is the trap the ask warned about. A folder baz no longer
holds is a folder baz can no longer *refresh*: its albums would sit on the wall
forever, never updating when their tags change, never disappearing when their
files do, and with no surface anywhere that could explain why. "Nothing
happened" is a worse answer than "they went".

**The price, stated rather than hidden**: those rows' `first_seen_ns` goes with
them, so a folder removed and added back files its albums under ADDED = today.
That is a real loss of a fact ADR-0019 built a whole column to protect, and it
is the cost of the reversal being otherwise free. It is not silent — removing is
two presses and the confirming one says *"Forget N tracks? The files stay on
disk; baz stops holding them."*

Nothing on disk is touched. baz has never modified or deleted a music file and
this does not start.

### 5. The config grows a list, and migrates the old key silently

`music_dir` (one string) becomes `music_dirs` (an ordered array). The order is
data: it is the order the folders are scanned in, listed in, and — since a
rootless row can be claimed by only one root — the order a nested pair is
resolved in.

Reading stays **defensive per key**, as `config.rs` already was:

- An entry that is not a string, or is blank, is skipped; the folders around it
  survive. One mistyped line must not cost a listener their other three.
- A duplicate is dropped, keeping the first mention.
- A `music_dirs` that is absent, not an array, or empty of anything usable falls
  back to the pre-v8 `music_dir` string.

That last rule *is* the migration, and it is silent: a config written by the old
baz yields exactly its one folder, and the next save writes `music_dirs` and
drops `music_dir`. Nothing is asked of the listener and nothing is lost —
**losing somebody's library to a change in a file format would be a
self-inflicted version of the failure ADR-0010's gates exist to prevent.**

`baz DIR` on the command line now **adds** its folder to the front of the
remembered list rather than replacing it. Pointing baz at a folder for an
afternoon must not silently forget the other three.

### 6. Three refresh mechanisms, and they are three different things

| | when | what it does | who starts it |
|---|---|---|---|
| **At launch** | once, at `Shelf::open` | incremental: an unchanged stamp is never opened | baz |
| **While running** | every `REFRESH_INTERVAL` (5 min), measured from the previous pass *finishing* | the same incremental pass | baz |
| **Force sync** | on a press | re-reads **every** file, whatever its stamp says | the listener |

All three run on the `baz-scan` worker thread and hand the UI thread batches at
10 Hz. None of them touches the engine, which is a separate thread with its own
queue and its own ring buffer. **The index cannot make a sample late**, and that
is structural rather than careful.

`scan::Refresh` is the clock and it is pure state, tested without sleeping. Two
rules: nothing is due while a scan is running, and the clock restarts when a
pass *finishes*. So a library that takes six minutes to walk is rescanned every
eleven rather than continuously, and a slow pass can never build a backlog.

**Force sync is a different act from a rescan, not a more thorough one.** A
rescan asks "what has changed?". A force sync says "assume nothing about what I
already believe" — which is the only answer for the case the stamp structurally
cannot see (a file rewritten in place to exactly its old length with its mtime
restored, ADR-0010 §1), and for a listener who suspects the index rather than
the disk. It is `ScanMode::Force`, one branch in the worker choosing
`library::scan` over `library::scan_incremental`; everything downstream — the
batching, the counts, the removal pass — is identical, which is what makes it a
mode rather than a second pipeline.

### 7. `notify` was evaluated and **rejected**

`docs/research/04-tech-stack.md` named `notify` as the intended watch-folders
mechanism and it was never built. It is still not, and this is the reasoning
rather than a deferral.

**What it would cost.** The licence is *not* the objection: `notify` is
CC0-1.0 (dual-licensed with Artistic-2.0 in recent releases), and `CC0-1.0` is
already on `deny.toml`'s allowlist — it entered transitively via
`iced → wgpu → naga → hexf-parse` — so `cargo deny` would pass without the
policy being widened. (The exact expression would still be confirmed at the
version taken; that is what the gate is for.)

The cost is crates and platform surface. On Linux `notify` pulls `inotify` and
`mio`; on macOS `fsevent-sys`/`kqueue`; on Windows it drives
`ReadDirectoryChangesW`. That is three per-platform event backends and a
long-lived background thread sitting on the path that reads the user's own
filesystem, which `ENGINEERING.md` treats as hostile-input territory — against a
dependency graph whose whole decode path is currently pure Rust with zero system
dependencies.

**What it would buy, honestly.** Less than it looks like, because a watcher's
coverage is *silently* partial in exactly the libraries baz is built for:

- **inotify is per-directory and capped.** `fs.inotify.max_user_watches`
  defaults to 8 192 on many distributions. A recursive watch on Marta's 40 000
  tracks (`docs/research/05-personas.md`) needs one watch per *directory* —
  thousands — shared with every other application on the desktop. Exhausting it
  fails the registration, and the failure is per-directory: parts of the tree
  are watched and parts are not, with nothing on screen to say which.
- **Network mounts do not generate events at all.** NFS and SMB report nothing
  through inotify for changes made on the server. The NAS is precisely the case
  every gate in ADR-0010 was written for, and it is the case a watcher cannot
  serve.
- **The other platforms have their own holes.** FSEvents coalesces and can
  report a directory rather than a file; `ReadDirectoryChangesW` drops events
  when its buffer overflows during a bulk copy — the exact moment a listener
  most wants the wall to update.

So a watcher would need a periodic rescan behind it anyway, for the NAS, for the
overflow, and for the watches that could not be registered. **The fallback is
the whole feature**, and adding a watcher in front of it buys latency on local
folders in exchange for three platform backends and a mechanism that works on
some listeners' libraries and quietly does not on others.

**What makes the trade lopsided is that the rescan is nearly free.** ADR-0010
measured the warm pass at 10.3 ms per 10 000 files — one `stat` each — so
~100 ms for the 100k library the search index is built for, on a worker thread.
Five minutes is chosen against what a listener notices rather than what the
machine can afford: a minute would spend that a hundred times an hour to notice
a rip they already know they made; an hour leaves a fresh import off the wall
for most of an afternoon. Five minutes is under the time it takes to rip a CD,
which is the shortest interval at which a new record actually appears.

**What would change the decision**: a measurement showing the periodic pass is
too slow on a real large library (in which case the watcher is an *optimisation*
with a stated fallback, which is a much better shape than a feature), or a
listener who needs sub-second latency for a workflow baz does not have yet.

### 8. The Settings surface

The Settings place gains a second section, **Library**, and it cost exactly what
the place promised a second section would cost: an entry in `SECTIONS`, a block
in the same scroll, and an `on_press` to make the one-vertebra spine a real
control. No new arrangement, no new widths, no new heading treatment.

Each folder is two lines: its path on a `TRANSPORT_HIT` row with its Remove
control, and one quiet `SIZE_META` line saying how many tracks are recorded
under it and when a scan of it last finished — or, if the last pass could not
reach it, that **nothing was removed from it**, because that is the guarantee
and a listener seeing a NAS greyed out needs it. Below the folders: a well and
an `Add`, then the force-sync block.

The composition laws in `.interface-design/system.md` §13 hold by construction:
every control is `TRANSPORT_HIT` (L7), every gap is a `theme::GAP_*` token (L2),
no row introduces a horizontal edge of its own (L5), and every fixed box states
how its content is centred (L3).

**Accessibility.** The accessibility refusal binds: nothing here is
keyboard-only. Every act is a pointer target — and the add-a-folder field also
takes `Enter`, which is the same affordance the first-run screen has had since
v0.1. Folders are typed rather than picked from a system dialog, because baz
takes no dialog dependency (`rfd` and the portal behind it are not in the
graph); the first-run screen already asks for a typed path, and the two look the
same for the same reason.

> **Superseded in part by ADR-0025** (2026-08-09): `rfd` 0.17 dropped the
> dependency weight this clause priced in — its portal backend costs one new
> crate on Linux and no gtk — so the row gained a `Browse…` beside the well.
> The typed path stays, now load-bearing for the folder a dialog cannot show
> (an unmounted share); everything else in this section is unchanged.

## Consequences

- The index knows which folder each track came from, and removal proves
  ownership instead of inferring it from a path.
- Several folders can be held, listed, added and removed, each with its own
  track count and last-scan time.
- baz notices a change while it is running, within five minutes, without
  watching the filesystem.
- A listener can force a full re-read without deleting their database.
- `KnownFiles` changes shape (`Option<FileStamp>` → `KnownFile`), `Scan` and
  `ScanUpdate` gain roots and a mode, and `ScanUpdate::Error` no longer means "a
  folder is missing". All are breaking changes to pre-1.0 internal APIs.
- Removing a folder loses its tracks' `first_seen_ns` (§4).

### Deliberately deferred

- **Reordering folders in the interface.** The order is data and the config file
  is editable; a drag handle is a control with its own design.
- **Per-folder settings** — one folder watched, another not; one excluded from
  shuffle. Nothing asks for it yet.
- **Removing rows under a directory that itself vanished.** Unchanged from
  ADR-0010: it needs a signal the filesystem does not offer. Removing the
  *folder* is now a way out that did not exist before, but it is a different
  act.
- **A user-facing prune** for the rootless population. It is now *counted* and
  explained, which is the honest half; the "these 412 rows point at files I
  cannot find — remove them?" surface is still unbuilt.
- **A file watcher.** §7, with what would reverse it.
