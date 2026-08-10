# ADR-0042: What baz remembers about music it no longer holds

**Status**: accepted (2026-08-10) · **withdraws the price ADR-0022 §4 accepted**
(that removing a folder destroys its tracks' `first_seen_ns`); **extends
[ADR-0019](0019-group-keys.md) §5's structural guarantee** without weakening a
word of it; **completes the first item of [ADR-0010](0010-incremental-scanning-and-removal.md)
§3's deferred list** (a user-initiated removal for the cases automation declines)
· `SCHEMA_VERSION` 8 → 9, one table, no backfill · **overturns nothing in
ADR-0010's four gates** — the refusal to guess about mounts stands exactly as
written · closes `docs/WORK.md` beta blocker 3 and builds the mechanism once
proposed for blocker 2 · zero new dependencies

> ## Amendment (2026-08-10) — the record-scale control is rejected
>
> The owner did not ask for `Forget this record` and rejected it when the
> proposed control was shown: if someone wants a record removed, they delete or
> move its files out of the held library, and baz prunes the index. The control
> was removed. `Library::forget_paths` remains library machinery and test
> coverage for the tombstone invariant; it is not a product workflow. Section
> 8 remains below as the proposal's history, not an implementation instruction.
> `docs/BACKLOG.md` retains the actual gap: safe automatic pruning beneath a
> reachable root without weakening ADR-0010's unavailable-root guarantee.

## Context

Two defects, reported from opposite ends, and the reason to answer them together
is that they are one question asked twice.

> **2. A deleted folder's records never leave the library.** `rm -rf` an album
> directory and its eight rows stay on the wall for good.
>
> **3. Removing a music folder destroys `first_seen_ns`.** Remove a root, add it
> back, and every album files under ADDED = *today*.

One forgets too little. One forgets too much. Answered separately they would
produce two mechanisms with two opinions about the same act, and the second one
written would be the one nobody remembered to keep in step.

**The existing refusal on (2) is correct and is not being overturned.** ADR-0010
§3's fourth gate requires a file's *parent directory* to be present before a row
may be deleted, because from the filesystem's side a deleted directory and an
unmounted share are the identical `NotFound` for every path beneath. Wiping a
present listener's library to tidy a stale row is the worse failure, and the
maintainer's own collection lives on two `gvfs` SMB mounts (ADR-0025), so the
unmount case is his everyday case rather than a hypothetical. Nothing below
introduces a heuristic about whether a path is "really" gone. There is no such
heuristic in this ADR, and there must not be one.

What ADR-0010 left undone was the other half, and it named it in its own
deferrals: *a user-facing prune*. A person saying *"this record is gone"* needs
no mount detection, because they are asserting the fact rather than inferring it.

**And that is precisely where (3) turned out to be the blocker.** A listener's
assertion can be wrong — the share was merely unmounted, the folder came back off
a backup — and until now being wrong was **unrecoverable**, because forgetting
destroyed the one fact nothing could rediscover. That is why (2) could not be
offered and why (3) is the more urgent of the two: it is not merely a lost date,
it is the missing guarantee that makes a destructive act safe enough to put in
front of somebody.

`docs/BACKLOG.md` named the fix for (3) — a **tombstone** — and called it *"its
own small design"*, which is this.

## Decision

### 1. The answer, in one sentence

> **baz remembers, about music it was told to stop holding, exactly one thing:
> when it first saw it. Nothing else, because nothing else is unrecoverable.**

Everything below falls out of that sentence, and both defects fall out with it.

### 2. Two doors out of the library, and only one leaves a trace

Rows leave the index two ways, and they mean opposite things:

| | who decides | on what | leaves a tombstone |
|---|---|---|---|
| `Library::remove_tracks` | **baz** | ADR-0010's four gates: the walk saw something, the path is under a root it recorded, no ancestor failed, and `symlink_metadata` says `NotFound` with the parent present | **no** |
| `Library::forget_root` / `Library::forget_paths` | **the listener** | they said so | **yes** |

The line between them is not scale and is not danger. It is **evidence versus
decision**. The gates are evidence baz gathered itself; evidence needs no
reversal, and that path runs on every scan forever, so putting a write on it
would be paying insurance against a call that was never a guess. An assertion is
a decision, decisions are sometimes wrong, and a wrong decision has to cost
nothing.

This is also why the answer is not "remember everything that ever leaves". A
library that remembered every file the listener has ever deleted would be
carrying a permanent record of their churn to insure a call nobody disputed.

### 3. The unit of forgetting is a set of paths, at whatever scale the listener
pointed at

The owner's report is about a **record** on his wall. The data loss is about a
**root**. They are not the same object and they do not need to be: they are the
same *act* at two scales, and the design makes that literal rather than
analogical.

- `Library::forget_root(root)` — every row **recorded under** that root, keyed on
  the recorded `root` column and never a path prefix (ADR-0022 §4's rule, which
  stands), plus the root's own `roots` record.
- `Library::forget_paths(paths)` — exactly the rows named.

Both write the same tombstone with the same SQL: `REMEMBER_TRACKS_UNDER_ROOT`
and `REMEMBER_TRACK` are one statement differing only in a `WHERE` clause. That
is the guard against the two halves of this design drifting into two mechanisms
that disagree, and it is asserted rather than asserted about —
`forgetting_a_root_and_forgetting_its_paths_leave_the_same_memory` forgets the
same library both ways and compares what is left.

A third scale — "forget this record" — is `forget_paths` over the album's tracks
and needs no new machinery. See §8 for what did and did not land of its control.

### 4. Schema v9: one table, and the migration with nothing to argue about

```sql
CREATE TABLE forgotten (
    path          BLOB PRIMARY KEY, -- OS-native bytes, like `tracks.path`
    first_seen_ns INTEGER NOT NULL  -- ns since the epoch
) STRICT;
```

`SCHEMA_VERSION` moves 8 → 9 by `CREATE TABLE` inside one transaction with the
`user_version` bump — the same discipline as v2 – v8, so an interrupted upgrade
leaves a v8 database the next open migrates again.

**It is the one migration in the ladder that touches no existing row.** There is
no column to fill, no value to derive and no backfill to defend, because the
table records acts a listener has not performed yet. It starts empty, and empty
is the whole truth about it.

**What the upgrade cannot repair, stated rather than hidden**: a folder removed
*before* v9 took its first-seen with it and left nothing behind. There is no
evidence anywhere to reconstruct it — the same wall ADR-0019 §5 hit when it
refused three backfills — so a pre-v9 loss stays lost and the fix is
prospective. It is the last such loss.

**Restoration does not weaken ADR-0019 §5 by one word.** `UPSERT_TRACK` still
names `first_seen_ns` in its `INSERT` list and still omits it from its
`ON CONFLICT DO UPDATE` list; a row's first-seen is still written exactly once,
when the row is created, and no rescan can reach it. What changed is what the
insert is *given*: for a path a tombstone remembers, `add_tracks_under` binds the
kept value instead of the clock. The structural guarantee is unchanged; the value
it protects is now the true one rather than today's.

The resolution happens **once, in Rust, and the same value is handed to both the
database and the in-RAM index** — deliberately not as a subquery inside the
upsert, which would have left the two halves as two writers agreeing to be
careful about a fact ADR-0019 spent a schema shape avoiding exactly that for.

### 5. Where a tombstone dies, and what bounds it

A tombstone that never expires is a leak; one that expires too early is the bug
it was built to fix. Four bounds, none of them a clock:

1. **`path` is the primary key.** A path forgotten ten times leaves one row, not
   ten. The table can never exceed the number of distinct paths the library has
   ever held — a bound stated in the listener's own data rather than an invented
   number. On conflict it keeps the **earliest** claim, which is the rule an
   album's own ADDED already follows (ADR-0019 §5: an album is dated by its
   earliest track).
2. **It is consumed by the return.** The moment its path is inserted into
   `tracks` again, the tombstone is deleted — in the same transaction, so the
   restored row and the spent memory land together or not at all.
3. **It is swept at open.** `DELETE FROM forgotten WHERE path IN (SELECT path
   FROM tracks)`, one set-based statement per open, which answers the crash
   between (2)'s two writes and makes the invariant *no path is ever both held
   and tombstoned* true at every launch rather than merely usually.
4. **A row with no first-seen leaves no tombstone.** `first_seen_ns` is
   `NOT NULL`: a memory holding no fact would be a leak buying nothing. A pre-v7
   row reads `Not recorded` before the act and after it.

**No expiry, and that is the decision rather than the omission.** The defect this
exists to fix is a folder removed in one year and added back in another, so any
age limit short enough to be a bound would reintroduce the bug at its own
boundary — with the added cruelty of working in testing and failing in a life.

**What it costs on the hot path: nothing measurable.** The scan's per-file work
gains one hash probe of a map that is empty in the ordinary library, and the
consume statement is guarded by that map being non-empty, so an ordinary scan
executes it zero times. `scan/launch_cold_10k` measures **81.0 ms** on the
development host against the **83.4 ms** ADR-0010 recorded for the same
benchmark; the addition does not appear above the difference between two runs.

### 6. What is *not* remembered, and the criterion that decides it

**Remember only what nothing can recompute.** Applied to a `tracks` row, that
selects exactly one column.

| | recoverable by | in the tombstone |
|---|---|---|
| tags, `genre`, year, disc, track | reading the file | no |
| `format`, `bit_depth`, `sample_rate`, `bitrate` | reading the file | no |
| `mtime_ns`, `file_size` | `stat` | no |
| tagged `rg_*` | reading the file | no |
| measured `rg_computed_*` | an EBU R128 pass (ADR-0015) | **no** — see below |
| `root` | the scan that finds it again | no |
| **`first_seen_ns`** | **nothing, ever** | **yes** |

The `rg_computed_*` row is the only close call, and it is refused on the
criterion rather than on effort: a measurement is *expensive* to recompute and it
is *recomputable*, which makes storing it a **cache**, not a memory. A cache has
a different bound (it is invalidated by the file changing, which is why it
already carries its own stamp), a different size class (six integers per track
rather than one), and a different argument. Widening this table to hold it would
be a different feature wearing this one's name. It is noted in *Deliberately
deferred* rather than silently dropped.

**Two other stores were checked rather than assumed**, because the ask warned
against widening scope by accident:

- **The play ledger.** `history.tsv` is a separate append-only file keyed by
  path (ADR-0018) and nothing in baz deletes from it, so PLAYED already survives
  a folder being removed and added back — with no tombstone, no widening and
  nothing to build. Pinned by
  `forgetting_and_restoring_a_folder_leaves_the_play_ledger_alone`, so that a
  later change cannot quietly prune the ledger on a forget.
- **Playlists.** `.m3u8` files on disk (ADR-0024), untouched by anything here.
  A playlist naming a forgotten track keeps naming it.

So the honest answer to *"does re-adding a folder restore anything besides
`first_seen_ns`?"* is **no, and it does not need to**: the only other durable
per-track state baz keeps was never lost.

### 7. What the listener sees, and why this is neither the undo stack nor the trash

**The visible change is one sentence**, in the one place this act has a control
(the Settings place's Library section, ADR-0022 §8). The confirming press said:

> Forget 412 tracks? The files stay on disk; baz stops holding them.

and now says:

> **Forget 412 tracks? The files stay on disk; baz stops holding them but
> remembers when they arrived.**

Three clauses in the order a hesitating listener needs them: what goes, what is
not touched, what survives the round trip. The addition is not decoration — **a
reversible act that reads as irreversible gets refused by people who would have
been fine**, and until now the reading was correct. `forget_phrase` is pinned by
test in `views/settings.rs`, where the words are decided.

**It does not join ADR-0027's undo stack.** That is a bounded stack of whole-list
snapshots per *list surface* — the queue and the open playlist page — and its
histories end when you leave the surface. Forgetting a folder is not a list edit,
it is not on a surface you leave, and its reversal is not a snapshot: it is *add
the folder back*, an act the listener already has, whose cost this ADR has just
reduced to a rescan. Modelling it as an undoable edit would mean keeping the rows
somewhere to restore — which is the wall of unrefreshable albums ADR-0022 §4
refused, rebuilt inside an undo stack.

**It does not go to the trash.** ADR-0027's trash moves a *file* to the
freedesktop trash so a file manager can Restore it. Forgetting moves no file and
touches no file. There is nothing for a file manager to hold.

**The tombstone is the forgiveness mechanism for this act**, in the 1992 HIG's
own ranking that ADR-0027 adopted: reversibility first, a warning only for what
reversal cannot reach. The warning (two presses) stays, because the act still has
a cost — a rescan, and a NAS rescan is not instant — but it is now a warning about
*time*, not about *loss*.

### 8. What landed, and what did not

**Beta blocker 3 lands whole.** Removing a folder and adding it back restores
every record's ADDED, proved through real files, real scans and a restart, with a
value planted four years in the past so that "a value exists" cannot pass for
"the value from before".

**Beta blocker 2 lands as far as this branch may reach, and no further.** Its
mechanism is built, documented and tested at both scales — `Library::forget_paths`
is the record-scale verb and
`forgetting_a_record_that_was_only_unmounted_costs_nothing_when_it_returns`
walks the exact failure it insures against. What is missing is its **control**:
a `Forget this record` item on the tile menu is one `Message` variant and one
update arm in `crates/baz/src/app.rs`, and this branch was directed to stay out
of that file while another agent works in it on the `SchemaTooNew` screens. The
design of the control is drawn here so that whoever picks it up is not
re-deciding it:

- **Where.** The tile menu (`menu.rs`'s album `Target`), last, below
  `Add to playlist…`. Not on the hover overlay, which holds the four verbs the
  owner approved and is not where a destructive act belongs.
- **What it says.** `Forget this record` — *forget*, the word this act already
  uses in Settings, and not *remove* or *delete*, which name things baz does not
  do to files.
- **Its visible twin.** `menu.rs`'s standing rule is that no action's only route
  is a menu, so the item is illegal until the same message has a named on-screen
  control. The record page's own strip is where it belongs, beside the verbs that
  are already there.
- **Its press.** `forget_paths` over the album's track paths, then the same
  clean-up `remove_root` already does — clear `opened`, clear `no_art`,
  `rebuild_shelves`.
- **What it must say when pressed.** The same three clauses as the folder's
  sentence, at record scale.

## Consequences

- Removing a music folder is **reversible at no cost**. ADR-0022 §4's stated
  price is withdrawn; that ADR is amended rather than quietly contradicted.
- `SCHEMA_VERSION` is 9. **A v9 database cannot be read by a v8 binary**, which
  is `IndexError::SchemaTooNew` — the state `docs/WORK.md`'s first beta blocker
  is being fixed to present honestly. This bump is the first real instance of
  that case reaching a listener, and the two changes belong in the same beta.
- `Library` gains `forget_paths`, `forgotten_paths` and `forgotten_first_seen`,
  and an in-RAM mirror of the new table alongside its `roots` mirror. Additive.
- `Library::remove_tracks`'s contract is now written down as *the scan's door*
  rather than *the only door*, and says why it leaves nothing behind.
- The Settings place's confirming sentence grew one clause. No record page or
  tile-menu control is added; the amendment above rejects that proposed
  workflow.
- Zero new dependencies; `cargo deny` unchanged; no Flatpak permission moves.

### The one thing that needs the owner's eye

**A tombstone holds a path.** A listener who removes a folder to stop baz knowing
about that music leaves baz holding its file paths — not its tags, not its
titles, one path and one integer — until they add it back. That is consistent
with a decision already taken (the play ledger keeps the path of everything ever
played, forever, and nothing prunes it) and it is bounded by §5, but it is a new
place the fact lives and it is the kind of thing that should be told rather than
discovered. If the answer is that a forget should be able to be *total*, that is
a second control and a different ADR; it is not smuggled in here.

### Deliberately deferred

- **A control for the record-scale forget is rejected, not deferred.** The
  owner chose filesystem removal plus automatic pruning; do not rebuild §8's
  proposal from the existence of `forget_paths`.
- **A user-initiated prune** — *"these 412 rows point at files I cannot find;
  remove them?"* — the second item of ADR-0010 §3's list, and the surface that
  answers the whole stale-row family at once rather than a record at a time. It
  wants a library-maintenance place that still does not exist, and it must show
  what it found **grouped by root and counted**, because a root whose entire
  population is unreachable is the shape an unmounted share makes and a listener
  must be able to see that before they press. Reporting a counted fact is not
  guessing; deciding from it would be.
- **Carrying `rg_computed_*` across a forget.** §6. A cache with its own bound,
  not this table.
- **Expiring or clearing tombstones by hand.** §5's bounds make it unnecessary
  and the *needs the owner's eye* note above is the only reason it would exist.
- **A tombstone for scan-confirmed removals.** §2. It would insure a call that
  was never a guess, at the price of a write on the churn path forever.
