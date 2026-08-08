# ADR-0024: Playlists — files the user owns, a page, and one summoned panel

**Status**: proposed (2026-08-09) · extracts the decisions of
[`docs/design/08-playback-and-playlists.md`](../design/08-playback-and-playlists.md)
§4–§6 · changes no engine command — playing a playlist is `SetQueue` ·
introduces `baz_core::playlist` and `Place::Playlist` · **amends two
`docs/REFUSALS.md` entries under the ledger's own editing rule** (§5, §6) ·
sibling of [ADR-0023](0023-playback-model.md) · the owner's brief: *"we need a
way to see playlists, and possibly a section. it should be really easy to
drag a song into a playlist"* — and *"we need to model playlists honestly"*

## Context

baz has no playlists — not in code, config or schema. The audience arrives
from foobar2000 and MusicBee with folders of `.m3u` files and a habit of
ordered lists; the vision commits to *"no playlist ceremony"* (playing music
must never require building a list first) while `VISION.md` pillar 3 commits
every piece of app data to open formats the user owns. The owner additionally
wants sentiment-generated playlists eventually, which makes the model's
honesty guarantees load-bearing before any generator exists. The design study
(`docs/design/08` §2) places the decision against prior art: MPD's
stored-playlists-loaded-into-the-queue boundary, foobar2000's
playlist-as-playback-model ceremony, and the self-mutating "smart" lists the
honesty requirement rules out.

## Decision

### 1. What a playlist is

> **A named, ordered list of track references, made by a person, stored in a
> file that person owns.**

Distinct from the queue (transient, one run), from an album (derived from
tags, not editable), and from any future saved query (visibly live, never
wearing a playlist's name). The boundary with the queue is MPD's, adopted
deliberately: playing a playlist **copies** it into the queue, and from that
instant the two are decoupled — editing the playlist does not reach into the
sounding run, and editing the queue never writes back into the file.

**The honesty clauses:**

1. The playlist a user edits is exactly what plays — entries, order,
   verbatim; no shuffle-on-play, no dedup, no silent skipping.
2. Nothing edits a playlist but the user. baz writes a playlist file only as
   the direct result of a user's edit to that playlist — never on play, never
   on scan.
3. No smart lists pretending to be lists. A playlist is frozen ground truth;
   contents that can change without an edit make clause 1 unstatable.

The surface word is **Playlists** — *mixtape* considered and declined (renames
a universally-owned concept for no structural payoff); *crate* stays reserved
for the album-set grouping it means in the critique.

### 2. Storage: one `.m3u8` file per playlist

`$XDG_DATA_HOME/baz/playlists/` (platform equivalents via the same `dirs`
seam as ADR-0018), filename = playlist name. **No database table, no export
step.** Sovereignty (pillar 3; the ledger's precedent — a SQLite row can be
rewritten or lost and `grep` cannot read it), and interop (M3U is the de
facto standard every player reads and the audience's migration format —
[Wikipedia — M3U](https://en.wikipedia.org/wiki/M3U)). DB-with-export is
rejected because the exported copy is stale the moment it exists: two sources
of truth is the dishonesty §1 forbids.

Format decisions, met in advance:

- **Write the strict common subset**: `#EXTM3U`, one
  `#EXTINF:seconds,Artist - Title` per entry, one path per line, UTF-8
  (`.m3u8`'s mandate is why it is the extension — plain `.m3u`'s locale
  encoding is the documented ambiguity). **Read liberally**: headerless
  files, bare path lists, CRLF, BOM, relative paths and `~`; unknown `#EXT`
  directives are preserved on rewrite, never stripped.
- **Absolute paths written**; relative resolved on read against the file's
  own directory. A rare non-UTF-8 path is written byte-verbatim with a
  warning comment — the file honestly mirrors the filesystem, baz
  round-trips it, and a baz-private escape dialect would forfeit interop.
- **Atomic whole-file rewrites** (temp + rename) on user edits — unlike the
  ledger this file is *meant* to be rewritten, by its owner. External edits
  honoured via mtime; last writer wins per file.
- Provenance as comments (`# made with baz on …`; generators add one line
  naming themselves and their input). Inert, legible, never consulted for
  behaviour.
- The folder is shown in Settings → Library beside the roots.

### 3. Missing and foreign entries

The library-roots posture (ADR-0022 *Several music folders*), applied to
playlists: **counted and surfaced, never silently pruned.**

- An entry whose path no longer resolves **stays in the file**, renders
  dimmed from the path's stem, unplayable, the path one glance away.
- **Play sends the playable subset**, and the page says so: `38 of 40 ·
  2 missing`.
- Repair is **offered, never automatic**: candidate matches (same filename
  under a current root) proposed per entry, confirmed by the user; the
  confirmation is the only thing that writes the file.
- An entry outside every library root **plays anyway** — the engine takes
  paths; refusing a file the user explicitly listed because the cache lacks
  a row would invert the cache/source-of-truth order. Metadata from the
  index when present, from the filename when not.
- **Duplicates allowed, unmarked** — a queue may legitimately repeat a file
  (ADR-0014 §3) and a list that plays its theme twice is its maker's
  business.

### 4. The surfaces: a place and a panel

- **`Place::Playlist(name)`** — the playlist's page, sibling of
  `Place::Album`: header (name, counts, missing count), `Play`, `Queue`,
  `Rename`, `Delete` (confirm: *"The file goes; your music stays"*), rows
  with the queue place's anatomy — reserved-slot ✕ to remove, reserved-slot
  ▲▼ steppers to reorder (the no-drag pointer route the visible-control rule
  requires; drag-to-reorder arrives with the shared pointer-capture widget),
  record-group headers over consecutive same-record runs, row click =
  `SetQueue` + `JumpTo` through the same `play_from` rule every list surface
  uses, lamp dot only when the queue is exactly this list.
- **The queue place gains `Save as playlist`** — the transient frozen into an
  artefact (prior art's W19), a new file and nothing else.
- **The playlist panel** — §5.
- **No Playlists place**: the panel is the index; a full-window list of
  twelve names would be the settings-rail emptiness at window scale.
- **No keyboard-only anything**: the panel's door is `Ctrl+P` beside its
  labelled control; every act has a visible pointer route.

### 5. The panel, and the refusal it amends

`docs/REFUSALS.md` — *"baz has no side surfaces"* — is amended, not deleted,
under the ledger's editing rule, and this ADR is the required argument. The
rail died of five findings: three unrelated tenants, a paragraph of
dismissal, the wrong tenant paying resident width, a gesture-breaking reflow,
arbitration state. The playlist panel has none by construction, and one thing
no place can have:

1. **One tenant, forever** — playlists; the amended entry names it and closes
   the slot. The junk-drawer disease requires vacancy.
2. **Summoned, not resident** — opened by a labelled `Playlists` door in the
   Library strip, closed by `Esc` or the door; the wall keeps 100 % at rest.
3. **It exists for simultaneity** — collecting is two-surface work (source
   and destination on screen at once), the one job ADR-0022's model cannot
   express. It *receives*; it does not display a selection, which is what the
   dead column did and what places do better.
4. **It overlays and never re-hangs the wall** — ADR-0016's verified float
   mechanics (`stack` + `opaque` + `mouse_area`, no scrim, wheel passes
   through), so "no press re-hangs the collection" survives.
5. Present in Library, Album and Queue places; absent in Settings.

Amended entry text: *"baz has no **resident** side surfaces, and no surface
that is a slot. One summoned, single-tenant panel exists: the playlist panel
(ADR-0024) — opened by its labelled door for the duration of a collecting
task, overlaying without reflow, closed at rest. It may never gain a second
tenant."* The owner has blessed this surface explicitly; the entry records
the argument so the blessing is not a precedent for panels generally.

Panel contents: `New playlist`, then one row per playlist — the name (door to
its page) and a receive-target. Rename and delete live on the page, where the
contents are visible at the moment of the decision.

### 6. Adding — three layers, budget ≤ 2 gestures

1. **Two-press add, ships first**: `Add to playlist` on the record's page →
   pick a playlist (or `New playlist`). A track row's reserved-slot `+` does
   the same for one track. No drag, no modifier, toolkit-safe today.
2. **The open playlist**: arm a playlist in the panel (a visible, reversible
   state — surface step and hairline, never the accent) and every tile label
   and track row gains a rest-drawn quiet `+`; one press per addition for a
   collecting session. `Esc` or the armed row disarms.
3. **The drag**: iced 0.13 has no pointer capture, so the drag needs the
   hand-built widget (`groove.rs` precedent) that also unlocks queue and
   playlist reorder — one investment, three surfaces. It ships last and is
   sugar over routes that already work; *"really easy to drag"* must not
   mean *"waiting on the hardest widget in the plan"*.

### 7. Generated playlists: the guarantees any generator inherits

The sentiment feature is not designed here; its ground is. Owed by the model
to any generator:

1. **Output is an ordinary playlist** — same folder, same rights, same
   format; no second species.
2. **Generation is an act, not a condition** — a person asks, a file appears;
   no standing rule refreshes it; *regenerate* is a press on the artefact.
3. **Provenance recorded, inert** — a comment line and, on the page, the
   pull-note's voice: facts, never a score, never a "because you liked".
4. **Nothing plays until the person says so** — the pull's own discipline
   (`app.rs:1584–1588`): the output is a page to read, and its `Play` is the
   ordinary one.
5. **Only the user's own data, and no hidden pool** — ledger, library, local
   analysis; the candidate set statable in a sentence, as shuffle's visible
   pool already is.

**Second refusal amended**: *"No auto-generated playlists"* keeps its force
against the thing it was written against — playlists that generate themselves
unbidden — and its gloss is tightened so it cannot be read to forbid the
owner's stated goal: *"Every playlist is asked for by a person and owned by
them thereafter. Refused: generation without a request, mutation without an
edit, and any candidate pool the person cannot see."*

## Consequences

- Playlists exist as files before any chrome does: `baz_core::playlist`
  (pure, iced-free, beside `history` for the same second-front-end reason)
  plus a page reading whatever the user drops into the folder — the
  migration story for a foobar2000/MusicBee refugee is `cp *.m3u8` into one
  directory.
- One new place member, one new panel, two new controls on the record's
  page; the engine is untouched, per ADR-0017 §5's amended cost honesty this
  is a product change in `baz-core` and is priced as one.
- Two REFUSALS entries amended by this ADR's argument, none deleted; the
  panel's exception is named and closed to tenants.
- Deliberately not done: `Queue next` (ADR-0023 fixes its future semantics),
  saved queries / smart lists (a different species, refused a playlist's
  name), playlist folders or nesting (nothing asks for it), watching the
  playlists folder (the mtime check on read is the whole mechanism — the
  ADR-0022 §7 argument against watchers applies with less force and the
  same conclusion), and any sync or share surface.
