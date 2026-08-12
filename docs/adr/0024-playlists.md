# ADR-0024: Playlists — files the user owns, a page, and one summoned panel

> **Interaction amendment (2026-08-12).** Saved-playlist collection tiles and
> playable rows now join ADR-0022's product-wide content grammar: one click
> selects/highlights, double click plays or needle-drops. Queue rows use the
> same selection state and double click to jump. The page's labelled `Play`,
> edit/transfer controls, panel destination rows and context-menu verbs remain
> direct because they are explicit commands, not ordinary content activation.

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
above; doc 09 §9) and its **layer 3 (the drag) shipped** (2026-08-09, the
shared pointer-capture widget `crates/baz/src/drag.rs` — doc 11 P5's
resequencing, doc 09 §13 step 8), and §3's repair surface (`Locate…`) not
yet built;
§7 remains ground rules for a feature that does not exist ·
**amended 2026-08-09** — the playlist's sleeve, §A1–§A2 ·
**amended 2026-08-10** — telling a found thing from a made one, §A3–§A6
(design 14), on the owner's *"the information heirarchy isn't great to be
able to tell the difference between an album and a playlist"* and
*"'save as playlist' really makes no sense on the playlist page for a CD"*:
the kind stated in words, the page's hierarchy restated per kind, the
byline line restored, and the run's own control made to say what it saves.
**§A3.1, §A3.2, §A4.2, §A4.3 and §A5.1–§A5.3 are shipped** (2026-08-10, doc
14 tier 1; captures at
[`docs/design/impl/records-and-lists/`](../design/impl/records-and-lists/README.md)).
**§A4.4 — the serif on the record page's hero — is shipped** (2026-08-10, doc
14 tier 2; captures at
[`docs/design/impl/serif-titles/`](../design/impl/serif-titles/README.md)),
with `views/now_playing.rs`'s prose amended in the same change and §A4.3's
byline extended to state its composition.
**§A2's arrangement is one function since 2026-08-10** — `views/page.rs`, on
the owner's *"right now they are different but for no good reason"*; §A4.5
is the strip's lead, and the frames are at
[`docs/design/impl/one-page-two-subjects/`](../design/impl/one-page-two-subjects/README.md).
**And the row inside it, later the same day** — the owner's *"ensure our
playlist view in the now playing and the playlist view/album view are the same
thing"*: a record's track, a playlist's entry and the **run column's** row were
three literal copies of one anatomy, the record head was two, and
`views/queue.rs` held four more copies of the reserved icon slot this ADR had
already shared for the two pages. `views::page::track_row` and `icon_slot`
ended those copies, moving no pixels. At that point the one-arrangement rule
explicitly stopped at the composition: the run column was not drawn through
`page::view`. **A7 supersedes that boundary.** What may still differ is the
owner's `DETAILS`, the next-track ring and trailing slot sets; they are
capabilities or row content inside the shared composition, not permission for
a second page. Historical frames are at
[`docs/design/impl/one-list-drawn-once/`](../design/impl/one-list-drawn-once/README.md).
§A3's closing paragraph and §A4.4's last sentence are the two questions
**left to the owner** ·
extracts the decisions of
[`docs/design/08-playback-and-playlists.md`](../design/08-playback-and-playlists.md)
§4–§6 · changes no engine command — playing a playlist is `SetQueue` ·
introduces `baz_core::playlist` and `Place::Playlist` · **amends two
the product's standing rules entries under the ledger's own editing rule** (§5, §6) ·
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

*Amended 2026-08-10 to record the owner's decision* — *"the 'all songs' should
be an implicit playlist."* **All songs** is a named, ordered list of track
references that no person made and no file stores, so it is not a playlist
under this definition, and the definition is not widened to swallow it. It is
the other kind of list this product has always had and only recently named:
doc 09 §2's *implicit* playlist, of which the queue is the other resident
example.

The distinction is load-bearing rather than pedantic, and it is exactly the
one that keeps §1's honesty clauses meaningful. **A playlist is a
destination**; an implicit list is not, because there is no file to write to —
which is why the picker offers every named list and never offers this one.
**A playlist is frozen ground truth** (clause 3); an implicit list is a view of
a live thing, and All songs says so in its own counts line rather than
pretending otherwise. What the two share is everything about *being played*:
both reify into the queue at the moment of the gesture, both decouple from it
instantly, and both are one `Save as playlist` from becoming the other kind.

So the surface word **Playlists** still means files a person owns, and the
panel that carries it now has one row at its head that is not one — labelled,
sleeved and legible as a list, with no `Add` beside it in any state.

Distinct from the queue (transient, one run), from an album (derived from
tags, not editable), and from any future saved query (visibly live, never
wearing a playlist's name). The boundary with the queue is MPD's, adopted
deliberately: playing a playlist **copies** it into the queue, and from that
instant the two are decoupled — editing the playlist does not reach into the
sounding run, and editing the queue never writes back into the file.

**The honesty clauses:**

1. The playlist a user edits is exactly what plays — entries, order,
   verbatim; no dedup, no silent skipping.

   *Amended 2026-08-10 to record the owner's decision* — *"can you make
   shuffle a property of the player i.e. toggle on/off."* This clause said
   **no shuffle-on-play**, and it was the direct blocker: with shuffle on, a
   playlist's `Play` sounds its tracks in a shuffled order.

   **What the clause was protecting is untouched, and it is worth naming what
   that was.** It was written against a *list* that quietly differed from the
   list on screen — a player that reorders the file, or drops what it thinks
   are duplicates, or skips what it cannot decode without saying so. None of
   that changed: the file is still byte-verbatim, still exactly what the page
   lists, still edited by nobody but the user (clause 2), and still what the
   run is copied from at the moment of the gesture.

   **And shuffle does not re-order even the copy.** ADR-0023's amendment was
   revised the same day, on the owner's *"shuffle as a concept is more about
   going to an unknown next track rather than actually mutating the track
   list"*: shuffle is a property of the **walk**, so the run holds the
   playlist's own order — visibly, in a queue the listener can open and read row
   by row — and what changes is which row is chosen next, which the run column
   marks.

   The distinction the clause now draws is therefore sharper than the one it
   drew before: **a list is not a play order.** The playlist is ground truth
   about *what*; the run is a record of *what, in what order, this time*; and
   with shuffle on, even the run's order is the file's — only the path through
   it differs, and it is drawn. A mode a listener set, can see lit, and can
   unset is not the silent divergence this clause exists to forbid.
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

the product's standing rules — *"baz has no side surfaces"* — is amended, not deleted,
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
   *Shipped (2026-08-09), resequenced by doc 11 P5 and closing doc 09 §13
   step 8 — the last of its steps*: `crates/baz/src/drag.rs`, one per-row
   wrapper paying all three surfaces. Press a row of either editor past an
   8 px threshold and the row is in the hand — a ghost card names it at
   the pointer, an insertion line rides the row boundaries (measured by
   the rows themselves, exact under the queue's virtual window) — and
   release commits **one** edit: a whole-list `UpdateQueue`
   (`queue_edit::moved`; the music keeps playing), one atomic file save
   (`Playlists::move_entry`), or, dropped on a standing panel's playlist
   row, that file's append — the picker row's own act, made direct.
   Sub-threshold stays the row's click; Esc discards;
   `CursorLeft`/`Unfocused` commit at the line (the groove's capture
   lessons, inherited and pinned by tests). The steppers, the `+`, the
   picker and the menus all remain — the drag is sugar, exactly as
   ordered here. Captures at `docs/design/impl/drag/`.

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
- Two standing rules amended by this ADR's argument, none deleted; the
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
   *Amended 2026-08-10 (§A3)*: this rule **stands**, and its argument still
   holds — but it is the reason the sleeve **cannot be the kind signal**.
   For one to three records `views/mod.rs:221-223` returns
   `sleeve_cell(first, edge)`, which is byte-for-byte the widget a record's
   own row builds (`views/lane.rs:574-579`): not *similar to* a record's
   cover but the same cover, from the same cache, at the same size. Read
   §A3 before citing this rule as a distinction.
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
  *Amended 2026-08-10 (§A4)*: **the arrangement stands; the declared
  hierarchy does not.** A record's cover is about the record, so it is the
  work and the title captions it. A playlist's collage is about its
  *contents* — four quotations from things further down the same page — so
  it is evidence rather than subject, and the only stable fact about a made
  list is its name. A playlist's page declares **the name ≫ `Play` → the
  collage → the rows**. The two columns, the 320 px aside and the
  `ALBUM_BREAKPOINT` stack are unchanged.

  *Amended 2026-08-10 (one page, two subjects)*: **the arrangement is now
  one function**, `crates/baz/src/views/page.rs`, and the two pages hand it
  what differs. The owner: *"can we reuse the basic layout and view of the
  playlist for the album view and the playlist view accessed via clicking
  into info — right now they are different but for no good reason."* He is
  reading a real thing: this bullet said *the record page's own two-column
  arrangement*, and what shipped was **a second copy of it**, written weeks
  later. Two copies of the breakpoint arithmetic, two of the scroll, two
  identity blocks held level only by a test that read both files' tokens,
  two spellings of the quiet act, two `Play` buttons, two lamp dots, four
  copies of one reserved icon slot.

  What the two pages now supply is only what is *about their subject*: the
  strip's lead, the sleeve, the commitment's label, the acts, the aside's
  tail, the hero's face, the byline, the facts and their optional `Undo`,
  the rows and their edit slots, and the empty state. What was drift is
  gone: the quiet act had two alignments and two inks, a record's page had
  no empty state at all, its `Play album` was hidden in a no-engine build
  while a playlist's `Play` stood there dead, `DISC 1` took air under the
  `TRACKS` rule where a first record head does not, and the strip led with
  the *kind* on one page and the *subject* on the other — see §A4.5.

  **And the frames found a divergence three studies had missed.** A
  playlist's whole page rode **12 px higher** than a record's:
  `theme::TOP_BAR_H` is `2 · TOP_BAR_PAD_V + TRANSPORT_HIT + 1` = 49, but
  `views::place_header_led` lays out whatever lead it is handed — a record's
  breadcrumb is a *control* and declares 32, a playlist's name was a bare
  `LINE_EMPHASIS` 20, so that strip came to 37. Tiers 1 and 2 could not have
  seen it: each cropped its two identity blocks out of *its own* page and
  compared their **shapes**, which was true and is still true. The
  composition boxes its lead at the control height now. **Queue, Settings
  and the Artist place still carry the same 12 px** — moving four places is
  a change to the frame, and it is logged in `docs/WORK.md` with its
  measurement rather than taken in passing.
  Captures and the before/after readings at
  [`docs/design/impl/one-page-two-subjects/`](../design/impl/one-page-two-subjects/README.md).
- **Whether playlists join the wall is deliberately not decided here.** The
  owner has opened the deeper information-hierarchy question — *"I am really
  struggling to come up with a simple and satisfying information hierarchy
  here… we are thinking there are implicit playlists everywhere"* — and a
  design deep dive on it is running as its own study (design doc 09). This
  amendment must not pre-empt it, so wall membership, a playlist's rail
  sorting, and search-corpus membership are all **deferred to that study**.
  What ships here is the vocabulary every outcome needs: a playlist that
  looks like a record wherever a playlist appears, which today is the panel
  and the page.
  *Amended 2026-08-10 (§A3)*: **that sentence is the one this ADR now
  qualifies.** A playlist should carry a record's *vocabulary* — the sleeve,
  the tile, the page's arrangement — and must not be *indistinguishable*
  from one. The family stays; what it costs is now paid explicitly, by §A3's
  kind token rather than by a badge.
  Recorded as *input* to the study, not as a decision: a
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

## Amendment — telling a found thing from a made one (2026-08-10)

The owner, after living with the family §A1–§A2 built: *"we do not have the
playlist name really prominent. basically the information heirarchy isn't
great to be able to tell the difference between an album and a playlist"* —
and, in the same breath, *"'save as playlist' really makes no sense on the
playlist page for a CD"*.

From [`docs/design/14-records-and-lists.md`](../design/14-records-and-lists.md).
**Both of his sentences are one defect**, and naming the loop is what makes
this an amendment rather than a badge: `Save as playlist` over a CD writes a
playlist whose only member is that record; a one-record playlist's sleeve is
that record's cover full-bleed (§A1 rule 2); and the result lands in the
returns lane above the record it was made from, wearing its face, in the
same type at the same size. The control makes the confusion the other
sentence reports.

Nothing here is a prohibition and nothing is removed. §A1 stands whole; §A2's
arrangement stands whole.

### A3. The kind is stated in words, because the sleeve cannot state it

**The axis is *found* against *made*, and the only honest signals are the
ones that express it.** A record is derived from files someone else authored;
its order is the artist's, its cover is one authored image, its name is a
**work's title**. A playlist is made by you; its order is yours, its cover is
generated from quotations, and its name is a **label you typed**. A signal
that expresses *made by you* is honest; a signal that only means *different*
is decoration, and this ADR is in a product whose artwork is radius 0 always.

1. **The line under a name declares its kind in its first token.** An
   artist's name for a found thing (unchanged); the word **`Playlist`** for
   a made one — `14` becomes `Playlist · 14 · 42:10`; a scale statement for
   an implicit one, which `All songs` already gives
   (`1284 records · 9902 songs · 84:12:07`). One rule, spent in the slot
   that already exists at every confused surface — the returns lane's rows,
   the panel's rows, and any tile a playlist ever reaches. **No new widget
   and no geometry change**: the same `SIZE_META` text, a different string.

2. **The collage is demoted from *the* signal to *a* signal.** §A1 is
   unchanged in every rule, including the full-bleed single and its
   argument. What changes is the load: ADR-0030 §2 and the comment at
   `views/lane.rs:550-556` state that *"nothing marks which kind a row is,
   because the sleeve already does"*, and that premise is **false for every
   playlist of one to three distinct records** — which includes every
   playlist `Save as playlist` creates from a record, and every list on its
   way to four. Those two prose sites are corrected in the same change; a
   comment that survives its own reason is how the next reader re-derives a
   retired argument.

3. **No badge, no glyph, no corner.** A mark over a sleeve breaches
   *"nothing is ever drawn on top of a sleeve"*, which §A1 went out of its
   way to preserve by building the collage *out of* quotations rather than
   *over* them. A rounded or matted playlist sleeve contradicts *"artwork is
   radius 0 always"*. Both were weighed and declined; recorded so they read
   as considered rather than missed.

**Deliberately left to the owner** (design 14 §9, tier 3): whether a
one-to-three-record playlist should draw the rest tile instead of borrowing
a record's face. It is the only change that makes the sleeve honest at every
count, and it costs a two-record list the best sleeve available to it at
320 px. A genuine aesthetic trade, and aesthetics is his rule.

### A4. A made thing's page: the same arrangement, its own hierarchy

§A2 gave the playlist page the record page's arrangement **and** its declared
hierarchy. The first was right; the second was an import that does not
transfer, and it is why the name does not read as prominent.

1. **The arrangement stands**: two columns, the aside fixed at
   `ALBUM_ASIDE_W` 320 so its blocks share one x-edge (law L5), `Play` under
   the object at the sleeve's whole width, stacking below
   `ALBUM_BREAKPOINT` 744. Two arrangements for two nearly identical jobs
   would be two vocabularies, which is the family being thrown away rather
   than paid for.

2. **The hierarchy is restated per kind.** A record's page keeps **the work
   ≫ `Play` → the title → the rows**. A playlist's page declares **the name
   ≫ `Play` → the collage → the rows**, because the collage is about the
   contents and the name is the only stable fact about the list.

3. **The byline line is restored, and it is what fixes *"not prominent"*.**
   The record's identity block is three lines — title `SIZE_HERO` 28, artist
   `SIZE_TITLE` 19, catalogue `SIZE_META` 12, **80 px**. The playlist's is
   two: name, then counts, **52 px**. The playlist page is the album page
   *with the byline deleted*, and the byline slot is exactly where *made by
   you* belongs. It gains a line in that slot at that size:

   ```
   Road Trip                 ← SIZE_HERO 28 / SEMIBOLD
   Playlist · 4 records      ← SIZE_TITLE 19 / paper_dim     ★ restored
   14 tracks · 52:31         ← SIZE_META 12 / paper_faint
   ```

   The two identity blocks become **geometrically identical at 80 px**, and
   the difference moves into what the middle line says — which is where a
   difference between two kinds of thing belongs. The name is not made
   larger: `SIZE_HERO` is the top of the ramp, and the prominence problem
   was a missing line, not a small number.

   **Not *"Made by you"***: §4 admits `.m3u8` files dropped into the
   playlists folder, which this product did not author and whose author no
   file records. `Playlist · 4 records` claims only what can be proved, and
   it explains the collage above it at the moment you are looking at it.

   *Shipped in two steps* — `Playlist` alone with tier 1, the composition
   with tier 2. **The count is not the sleeve's.** Design 14 §5.4 costed it
   as free from the quotation list `playlists.rs` already builds; that list
   stops at four, because four is all a 2 × 2 can quote, so a fourteen-record
   list would have carried `Playlist · 4 records` over a page listing
   fourteen — a false statement in the slot this section exists to make
   honest. The distinct set is walked to its end
   (`playlists::OpenPlaylist::records`). A list nothing in the library
   resolves states `Playlist` and claims no count, which is all it can prove.

4. **Typography is the axis with no pixel cost, and it is half-built.**
   `theme::WORK_TITLE` — IBM Plex Serif Italic, the museum-placard
   convention, the owner's approved risk of 2026-08-09 — is spent on one
   string, Home's `CONTINUE` placard, and locked there by an equality
   assertion. The distinction print has drawn for two centuries is ours
   exactly: **a work's title is italic; a label is not.** Extending the
   token to a record's page hero, and only there, makes the two page heroes
   different *kinds of string* at the same size for nothing.
   The rule, if it is taken, is enumerable and must stay so: **the serif
   sets an album's title and only an album's title** — not a track's, not an
   artist's, not a playlist's — or the run column's rows take it and the
   axis is gone. Whether it should also reach the wall's captions and the
   lane's rows is **the owner's call** (design 14 §9, tier 3): it is the
   strongest possible answer to his question and it is also sixty italic
   serif captions on a wall of covers.

   *Taken, 2026-08-10* (doc 14 tier 2), with three things settled in the
   building:

   a. **The rule needed one more clause, and `views/now_playing.rs` is why.**
      *"An album's title and only an album's title"* does not by itself
      exclude the `Ochre` printed under the sounding track on Now playing,
      which is an album's title. The clause: **on the surface whose subject
      that album is.** A record's page and Home's `CONTINUE` placard label
      the album; Now playing labels a moment in a **track**, with the album
      under it as a *fact about it* — and the placard convention this whole
      idea comes from sets the title in italic and every fact around it in
      roman. Italicising that line would leave the smallest string on the
      surface the only italic one, inverting the convention rather than
      applying it.

   b. **`views/now_playing.rs`'s prose argued against the serif, and its
      argument survived — as a concern, not as a boundary.** It said the
      serif must not become a display face arriving one surface at a time.
      That is right and is kept verbatim in the amended text. What did not
      survive is the boundary it drew, *"there is one placard in the
      product"*: a **quantity** cannot say whether the next string may have
      the face, which is exactly how a face arrives one surface at a time. A
      rule can. The guard that carries the concern is mechanical —
      `the_serif_is_the_work_titles_and_nothing_else` is an **enumeration**
      of two views and stays one, and nothing may name the serif family
      directly, so the revert is still one token.

   c. **A frame cannot prove the bundled face rendered, so tests do.**
      `Font::with_name` is a string match against a face's `name` table; a
      spelling that drifts resolves silently against the *host's* fonts and
      looks correct on the machine that shipped it. Closed by
      `font::the_family_names_baz_asks_for_are_the_names_the_faces_spell`
      (the family strings against what the bundled bytes spell, plus the
      italic style bit) and
      `font::the_serif_face_carries_every_letter_an_album_title_arrives_with`
      (a title is other people's text, and a missing codepoint falls back per
      glyph). *Found writing the first*: the family a matcher reads is `name`
      record **16** — record 1 is the legacy family and holds four styles, so
      Plex Sans Medium's record 1 reads `IBM Plex Sans Medm`.

   The wall and the lane are untouched and the question above stays the
   owner's.

5. **The strip names the subject on both pages, and the kind word is not
   drawn twice.** *Added 2026-08-10, one page, two subjects.*

   `views::place_header_led`'s own rule: four places lead with their name and
   nothing else, and the two whose **subject changes** lead with the subject
   — the Album place with `Anne-Marie Puig › Ochre`, the Artist place with a
   runtime string. A playlist's page is the third of those and led with the
   word `Playlist`, because it predates the breadcrumb by weeks.

   It leads with the list's name now. **This subtracts a statement, not the
   statement**: design 14 §3.5 had already found the chrome the wrong home
   for the kind — *"58 px above the name… invisible at the moment the eye is
   actually deciding"* — and §A4.3 above put it in the byline, at
   `SIZE_TITLE` 19 directly under the name instead of `SIZE_EMPHASIS` 15 in
   the strip. One rule now covers both pages: **the strip names what you are
   looking at; the byline says what kind of thing it is.**

   **No eyebrow, and that is answered from a frame rather than argued.** The
   owner asked for *"some sort of title/subtitle telling us if it's an Album
   or a Playlist"*, and
   [`0d-identities-together-after-1280x860.png`](../design/impl/one-page-two-subjects/README.md)
   shows the two blocks at one crop: a serif italic title over a person's
   name against a sans name over `Playlist · 12 records`. A word above the
   name would state a second time what that frame states plainly, which is
   §A3.3's badge wearing a word. If a later frame shows otherwise, the
   eyebrow is the candidate and it goes on **both** pages or neither.

### A5. `Save as playlist` belongs to the run, and must say so

The control is drawn in exactly one place — `views/queue.rs:337-353`, in the
run column's summary strip — and reaches the screen through
`Place::NowPlaying`, which absorbed the queue place. It is **not** on
`Place::Playlist`. The owner called that surface *"the playlist page"* and he
is reading it correctly: `views/queue.rs:49-56` says the queue place and the
playlist page are *the same editor*. What the surface never says is that the
thing being edited is a **run** and not a file — so the word sits 57 px above
the record's own title with nothing between them
(`docs/design/impl/queue-merged/01a-run-on-1280x860.png`).

Two category errors follow, and the second is the sharper: `queue_summary`
prints the run's provenance at the **head of the very strip the word sits
in**, so a run reified from `Road Trip` reads
`Road Trip · 1 of 14 · 52:31 left … Save as playlist` — an offer to save a
thing whose name is printed two inches to the left. `save_control` is
conditioned on nothing but whether its own name field is open.

**The act is real and it stays.** Freezing a transient into a file is §4's
creation act and is genuinely wanted for a shuffle, a `Play all`, or an
edited run. Three changes, all answerable from state the shell already
holds:

1. **The strip names its subject.** The no-provenance branch of
   `queue_summary` gains the noun the provenance branch already supplies:
   `1 of 24 · 1:56:19 left` → **`Run · 1 of 24 · 1:56:19 left`**. One word,
   in a string that is already built, in a branch that already exists — and
   it is what makes the word beside it unambiguous.

2. **A control that cannot usefully act says so instead of offering.**
   Provenance standing and no edit since the run was reified ⇒ the word
   becomes an inert readout in the same slot, **`Saved as "Road Trip"`** —
   which is what the panel's `Queue` row already is at rest. The moment the
   run is edited it has diverged from the file, and the live word returns as
   **`Save as new playlist`**. The precedent is eleven lines up in the same
   file: `undo_control` is drawn only while there is an edit to take back,
   because *"a standing `Undo` over a list nobody has edited would be a
   control that cannot act pretending it can"*. This is that defect, and
   that cure.

3. **What this must never become.** `Save changes to "Road Trip"` — writing
   the run back into the file it came from — is **refused**, and not on
   taste: the 2026-08-09 amendment's item 6 keeps the run tonight's snapshot
   and holds §1's decoupling *in both directions*, and ADR-0023 §3 makes
   provenance an origin rather than a link. A run that wrote itself back is
   the two-structure confusion returning.

`every_queue_affordance_survives_the_merge` still requires
`Message::SaveQueueStart` to be spent by the run column, so it passes
unchanged under these three and **would fail on a removal** — which is the
guard that makes this the right shape of fix.

### A6. Prior art, since all three of the obvious references face this

- **Spotify** is where §A1's collage came from, and baz took the picture
  without the sentence under it: Spotify's detail page runs a kicker, the
  name, and then a byline that **names a person** behind a circular avatar —
  the one shape in its vocabulary that is never a sleeve. baz takes the
  byline slot (§A4.3) and not the avatar: there are no accounts here, so a
  circle would be decoration with nothing behind it.
- **Apple Music** is the one that solves it badly, and it is worth showing
  because it is baz's exact disease: library grids interleave albums and
  playlists as identical squares whose only difference is what the second
  caption line happens to hold, with mosaic playlist art that collapses to a
  single cover when the list is small. That is `views/lane.rs:557-671` plus
  `views/mod.rs:221-223`, arrived at independently — which is what happens
  when a good tile design is reused for a second kind of thing without
  anyone deciding to.
- **Plexamp** segregates: kind-labelled shelves, so a playlist is never
  adjacent to an album without a heading between them. **baz has already
  declined this** — ADR-0030's lane mixes the two kinds because its subject
  is *what you touched*, and sorting by kind would make it two lists sharing
  a column. Declining it is what puts the work on the per-row signal, which
  is §A3.1.

### A7. 2026-08-12 — saved and unsaved detail are one component

The owner's eye found the limit the earlier row merge had documented and left
in place: saved and unsaved playlists shared primitives but retained two
top-level compositions. The saved page used `views::page`; the run owned a
different breakpoint, scroll document, summary strip, empty state and grouped
row presentation. Calling them “the same editor” did not keep them looking or
behaving like one.

The saved page's established fixed-aside/table anatomy is the reference.
`views::playlist_page` is now the only playlist-specific caller of
`views::page` and owns the collage, sleeve size, responsive form, identity,
`TRACKS`/empty block, scroller and row-space mapping. Both states use the saved
page's fixed row pitch, artwork and Album context.

Persistence remains a capability, never a second layout. A saved file supplies
Play, Rename/Delete, durable counts and file Undo. The transient run supplies a
reserved commitment slot, Save/provenance readout, live cursor/remaining time,
run Undo and the next-track ring. Same-viewport evidence and the complete drift
inventory are in `docs/design/impl/one-playlist-page/`; a source guard rejects
a direct page, sleeve, breakpoint, padding or scroller in either state module.
