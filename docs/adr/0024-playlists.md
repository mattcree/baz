# ADR-0024: Playlists — files the user owns, a page, and one summoned panel

> **Amendment (2026-08-09), from
> [`docs/design/09-implicit-playlists.md`](../design/09-implicit-playlists.md)**
> — the implicit-playlist study, on the owner's report that the shipped
> collecting UX splits the hierarchy (*"I don't want people to have to
> context switch constantly… we are thinking there are implicit playlists
> everywhere"*). Six changes; §1–§3 (the storage model and every honesty
> clause) and §7 (the generator guarantees) are untouched. **Status: items
> 1, 2 and 6 accepted and shipped (2026-08-09, doc 09 §13 steps 1–2 — the
> armed mode removed, the picker's Queue row and hoisted playing list, the
> file-only append); item 5 accepted and shipped (2026-08-09, step 3 — the
> songs section, captures at `docs/design/impl/songs-search/`); item 3
> accepted and shipped (2026-08-09, step 5 — queue-place edit parity,
> captures at `docs/design/impl/queue-parity/`); item 4 accepted and
> shipped (2026-08-09, step 4 — the context menu as a mirror layer,
> captures at `docs/design/impl/context-menus/`). All six stand**:
>
> 1. **§6 layer 2 — the armed collecting mode — is withdrawn.** Shipped
>    2026-08-09, removed on the owner's own observation (09 §9): it was a
>    second list-building grammar and a mode. The panel's per-row receive
>    `+` goes with it; panel rows carry one control (the door), plus
>    whole-row as target while picking. The drag remains the future
>    one-press form; the two-press pick remains the modeless floor.
> 2. **The picker gains the Queue as its first row** and hoists the
>    current playlist (ADR-0023's amended provenance) second, marked
>    *playing* (09 §8.1). At rest the panel's Queue row is a readout, not
>    a door. **The single-tenant clause is restated, not weakened**: the
>    panel's tenant is *ordered lists of tracks* — the unnamed one
>    included, which is the unification's claim that there is one kind —
>    and it still may never hold anything of another subject.
> 3. **The queue place reaches edit parity** (09 §8.2): the playlist
>    page's ▲▼ slots and the transfer `+` join its rows, making the queue
>    and playlist pages one editor; `Save as playlist` is named as *the*
>    creation act — a playlist outlives its playing when it is named, and
>    not before (09 §8.3).
>    *Accepted (2026-08-09), shipped as doc 09 §13 step 5*: the ▲▼ go out
>    as whole-list `UpdateQueue` through the pure `queue_edit::shifted` —
>    the music keeps playing, the cursor follows its track — and the `+`
>    (the sounding row's included) holds the row's track toward the
>    picker. The rows are drawn through the place's new virtual window
>    (`queue_window`), which is also `Play all`'s §7.1 gate.
> 4. **A context-menu mirror layer is introduced** (09 §5.2): right-click
>    menus on track rows, queue rows, tiles, and the bar's now-playing
>    block, governed as the keyboard is — every item sends a message some
>    visible control also sends, pinned by the same shape of test — which
>    is what admits *"send to current playlist"* in two gestures from
>    anywhere without breaching the visible-control rule.
>    *Accepted (2026-08-09), shipped as doc 09 §13 step 4*: the four menus
>    of §5.2's table exactly, a float at the pointer on ADR-0016's
>    mechanics (one at a time by construction — the overlay state is a
>    single `Option`; Esc peels it first; flipped inside the window at the
>    edges), the mirror pinned by
>    `every_menu_item_is_a_press_some_control_also_makes`, and the
>    playlist page's rows gaining the transfer `+` — §8.2's "same editor"
>    anatomy completed — as the visible twin their menu items mirror.
>    Captures at `docs/design/impl/context-menus/`.
> 5. **Search answers in songs** (09 §5): a ranked `Songs` section above
>    the filtered wall, from track results `Library::search` already
>    returns; a result's press is a needle-drop; the sections stay
>    separate, per the owner's brief.
> 6. **`Add to "{current}"` appends to the file only** (09 §6): the run
>    stays tonight's snapshot — §1's decoupling holds in both directions,
>    and the both-at-once gesture is refused as the two-structure
>    confusion returning.

**Status**: accepted (2026-08-09) — §1–§3 shipped as `baz_core::playlist`;
§4–§6 shipped as `Place::Playlist`, the panel, the add layers 1–2, and
`Save as playlist`, with §6's **layer 2 since withdrawn** (the amendment
above; doc 09 §9) and its **layer 3 (the drag) pending** on the shared
pointer-capture widget, and §3's repair surface (`Locate…`) not yet built;
§7 remains ground rules for a feature that does not exist ·
**amended 2026-08-09** — the playlist's sleeve, §A1–§A2 ·
extracts the decisions of
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

Panel contents (as amended by doc 09 §8.1, §9): the Queue's row at the head —
a readout at rest, the picker's first destination while a pick stands — then
one row per playlist (the name and sleeve, a door to its page; the
receive-target died with layer 2), then `New playlist`. Rename and delete
live on the page, where the contents are visible at the moment of the
decision.

### 6. Adding — three layers, budget ≤ 2 gestures

1. **Two-press add, ships first**: `Add to playlist` on the record's page →
   pick a playlist (or `New playlist`). A track row's reserved-slot `+` does
   the same for one track. No drag, no modifier, toolkit-safe today.
2. ~~**The open playlist**: arm a playlist in the panel (a visible, reversible
   state — surface step and hairline, never the accent) and every tile label
   and track row gains a rest-drawn quiet `+`; one press per addition for a
   collecting session. `Esc` or the armed row disarms.~~ **Withdrawn
   2026-08-09** — superseded by
   [doc 09](../design/09-implicit-playlists.md) §9 (the amendment above,
   item 1): shipped one day, removed on the owner's own observation as a
   second list-building grammar and a mode. Its one-press economy passes to
   the context menu (09 §5.2, pending) and the drag (layer 3, unchanged).
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

## Amendment — the sleeve (2026-08-09)

The owner, after using the shipped surfaces: *"I think similar to Spotify a
playlist would appear like a cd does."* A playlist should be a visual object
with a sleeve, the way a record is — not only a name in the panel and a page.
This amendment designs that sleeve and decides where it hangs.

### A1. The sleeve is a collage of quotations

A playlist has no artwork of its own; its sleeve is **constructed from the
records it quotes**. The canonical prior art is Spotify's generated cover —
a 2 × 2 collage of the first distinct cover arts, falling back to a single
cover below four — and baz adopts that read with its rules stated rather than
inherited:

1. **Four or more distinct records** (resolved through the library, in
   playlist order): a 2 × 2 collage of the first four records' sleeves, each
   cell half the tile's edge, no gaps, no frames.
2. **One to three distinct records**: the **first** record's sleeve,
   full-bleed. A 2- or 3-way tiling was considered and declined: every
   candidate (halves, an L, a dominant-plus-strip) is a layout with an
   opinion, drawn differently at 40 px and 320 px, where "the first record's
   face" is one rule at every size — and it is what Spotify itself shipped
   for years below four.
3. **No resolvable records** (an empty list, or nothing the library knows):
   a designed rest tile — the room's [`plinth`] surface step with a hairline
   edge, carrying the playlist's name in the display face (the name whole at
   page scale, its initial at panel scale). No random colours and no
   generated gradients: a made thing with no contents yet is *quiet*, not
   decorated.
4. **A cell whose thumbnail has not decoded yet** shows the same
   deterministic gradient placeholder a wall tile shows for the same record
   — the collage degrades exactly as the wall does, because it is drawn from
   the same cache.

**Against the refusals, explicitly.** *"Nothing is ever drawn on top of a
sleeve"* is not touched: the collage **constructs the playlist's own sleeve
out of quotations** — each cell is a record's artwork, whole and unmarked,
at thumbnail scale; nothing is drawn over any record's sleeve, and the
playlist's sleeve is its own object the way a record's is. *"No artwork is
ever drawn larger than its source"* holds by arithmetic: the full-bleed
single is drawn at `ART_MAX` exactly (the album page's own bound) and a
collage cell at half of it.

**Mechanically**: the cells come from the **same thumbnail cache the wall
uses** — the same decode path, the same LRU, the same placeholder — with no
new pipeline, no composited bitmap and no write anywhere near a cover file.
The composition is a list of album ids computed when the playlist is read,
so it follows the fingerprint discipline the surfaces already have: an
edited playlist re-reads, re-resolves, and its sleeve regenerates with its
rows.

### A2. Where the sleeve hangs today

- **The panel's rows** carry it at [`PANEL_SLEEVE`] 40: the rows were
  text-only, and a sleeve is what makes twelve of them scannable — and makes
  the armed row read as *a thing receiving* rather than a highlighted line.
- **The playlist's page** carries it in the hero position at `ART_MAX` 320,
  in the record page's own two-column arrangement: the object and its acts
  in the aside, the name and the rows in the main column. The page was
  already the album page's sibling; now it holds the same declared hierarchy
  — **the work ≫ `Play` → the name → the rows** (law L6).
- **Whether playlists join the wall is deliberately not decided here.** The
  owner has opened the deeper information-hierarchy question — *"I am really
  struggling to come up with a simple and satisfying information hierarchy
  here… we are thinking there are implicit playlists everywhere"* — and a
  design deep dive on it is running as its own study (design doc 09). This
  amendment must not pre-empt it, so wall membership, a playlist's rail
  sorting, and search-corpus membership are all **deferred to that study**.
  What ships here is the vocabulary every outcome needs: a playlist that
  looks like a record wherever a playlist appears, which today is the panel
  and the page. Recorded as *input* to the study, not as a decision: a
  pinned wall shelf strains ADR-0019's arrangement-as-projection grammar
  (under YEAR a playlist has no year, under ARTIST no artist; the rail would
  carry a foreign entry; `Library::search` does not hold made things), and
  the owner's sentence reads playlists as first-class visual citizens — the
  study weighs the two, and nothing built here forecloses either answer.
- **A playlist cannot be added to a playlist today**, because no surface
  offers the gesture: entries are track references (§1) and the panel's
  rows are doors and receive targets, not sources. If a future gesture ever
  picks up a playlist and drops it on another list, the honest meaning is
  *append its tracks, resolved at that moment* — noted so the semantics are
  on record before any widget, per ADR-0023 §3's precedent, and subject to
  the same deep dive.
