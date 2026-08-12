# 09 — Implicit playlists: one kind of list, one grammar

> The owner, after using the shipped playlist surfaces, verbatim:
>
> *"I am really struggling to find up with a simple and satisfying information
> heirarchy here. I don't want people to have to context switch constantly,
> but also dont want a poor sub standard ux."* — *"making playlists should be
> easy and just jumping out of the playlist should just be a case of picking
> another song to play."* — *"we are thinking there are implicit playlists
> everywhere."*
>
> And, in a second brief, the scenarios themselves (lightly trimmed of
> transcription artifacts): search should return **songs**, not only albums —
> *"people are really searching for songs in most cases… but if it finds
> albums, it should show them as well… they should be separate"*; a
> right-click that can *"send to a new playlist"* and — *"at any time you're
> playing a playlist"* — *"send to current playlist"*; a place to *"see these
> playlists and manage them"*; *"what happens when you shuffle the library?
> Has that created an implicit playlist?"*; *"one of the ways that I play
> music is to be able to play everything… in February I would have just
> selected the entire library and sent it to a playlist, which seems like a
> really silly and hacky way to do this"*; and the standing question — *"when
> do we create a playlist that lives longer than just the time that it's
> played for?"* — with the instruction to *"use industry standard ways of
> describing these interaction patterns and come up with the user stories
> required to do this."*
>
> A design study, not an implementation. Written 2026-08-09 against `75818b4`
> (the merge that shipped ADR-0024 §4–§6: the playlist page, the summoned
> panel, the armed collecting mode, `Save as playlist`). Successor to
> [`08-playback-and-playlists.md`](08-playback-and-playlists.md); its
> conclusions are carried into **proposed amendments** to
> [ADR-0023](../adr/0023-playback-model.md) and
> [ADR-0024](../adr/0024-playlists.md). Every claim about shipped behaviour
> is cited `file:line`; every prior-art claim carries a named source. The
> spine of the document is §4 — the user stories, in the industry's own
> artifacts (stories, task flows, Given/When/Then acceptance criteria),
> written to be implemented and tested as stated.
>
> The short version: the owner's observation is correct, it is already latent
> in ADR-0023's own language, and the shipped creation UX contradicts it.
> **One kind of list; one of them sounding and unnamed; one transfer gesture;
> the queue admitted to the panel as the unnamed list at its head; search
> that answers in songs; a context-menu mirror layer; a defined "current
> playlist"; play-everything and shuffle specified — and the armed collecting
> mode removed.** It shipped yesterday; sunk cost is zero; the owner's
> discomfort is data.

---

## 0. What is on screen today

The shipped inventory this document judges, read off the code:

- **The queue place** — one list with a cursor, record-group headers,
  click-to-jump, a per-row ✕ (`views/queue.rs:308`), and **`Save as
  playlist`** becoming a name field (`views/queue.rs:154–196`,
  `playlists.rs:527`). No reorder: the queue place's rows carry no ▲▼.
- **The playlist page** (`Place::Playlist`) — the queue place's anatomy plus
  the durable artefact's acts: `Play`, `Queue` (append to the live run via
  `UpdateQueue`, `app.rs:1347–1375`), `Rename`, `Delete`
  (`views/playlist.rs:118–154`), and per-row ✕ **and** ▲▼ steppers
  (`views/playlist.rs:412–475`).
- **The panel** — summoned, floating, single-tenant
  (`views/playlist_panel.rs:1–37`). Each row carries **two controls**: the
  name (a door to the page) and a `+` that **arms** the list to receive
  (`views/playlist_panel.rs:188–275`, `playlists.rs:409–415`).
- **The adds** — layer 1, the pick: `Add to playlist` on the record's page
  (`views/album.rs:352`) and a reserved `+` slot on its track rows
  (`views/album.rs:695`) hold what was pointed at and summon the panel as a
  picker (`playlists.rs:419–438`). Layer 2, arming: with a playlist armed the
  same presses append outright, one press each (`app.rs:1283–1287`), the
  record page's control relabels *"+ Add to the open playlist"*
  (`views/album.rs:356–360`), and the track rows' `+` is drawn at rest
  (`views/album.rs:680`).
- **Search** — type-anywhere filters the wall to **albums**. The index has
  always searched *tracks* (`Library::search`,
  `crates/baz-core/src/index.rs:1119`, ranked by ADR-0021) and the front end
  folds the matching tracks onto their albums for the wall (the fold is
  pinned by `vm.rs:1810`'s test, `search_filter_maps_tracks_to_albums…`).
  **The song answers exist and are thrown away at the surface.**
- **Shuffle** — a finite draw of `SLEEVES` = 8 records
  (`crates/baz/src/shuffle.rs:71`) from the wall's visible pool, sent as an
  ordinary `SetQueue` of whole records (`app.rs:1532–1574`); the pool dims on
  the wall and the next `RINGED` = 2 draws carry rings (`shuffle.rs:78`).
- **Not shipped**: ADR-0023 §3's `Queue album` control (no such message
  exists — the only queue-append in the product is the playlist page's
  `Queue`), the drag (layer 3), `Locate…`, any context menu, any song-level
  search surface, any play-everything gesture.

## 1. The diagnosis: two grammars for one operation

Every one of those surfaces manipulates the same kind of thing — an ordered
list of tracks — and the product ships **two unrelated grammars** for
building one:

| | The queue grammar | The collecting grammar |
|---|---|---|
| You stand at | the list itself | the source, aiming at a destination elsewhere |
| The gestures | play, drop the needle, jump, ✕, (reorder — playlist page only) | open panel, arm a target, press scattered `+` marks |
| Feedback | the list changes under your eyes; you can *hear* it | a count ticks in a side panel you are not looking at |
| Mode | none | one — the armed list, a state the whole wall wears |
| Where "what am I building" lives | the list surface | the armed row's counts, off to the side |

The first grammar is baz's own: direct, spatial, audible, and already proven
on three surfaces that share one row anatomy (the album page, the queue
place, the playlist page — `views/playlist.rs:5–15` says so deliberately).
The second is the streaming world's add-to-playlist side-channel — Spotify's
context-menu add, with the destination made visible. The owner's *"context
switch constantly"* is the seam between the two grammars, and the
*"struggling with the hierarchy"* is the fact that lists live in two
unrelated homes: the sounding one behind the bar's `Queue` door, the kept
ones behind the strip's `Playlists` door, with a mode bridging them.

The fix is not a better bridge. It is the owner's own sentence: *there are
implicit playlists everywhere* — so stop treating the kept ones as a
different kind of thing.

## 2. The model: everything that plays is a playlist

ADR-0023 §1 already says it without drawing the conclusion: *"the playing
context — this record, this playlist, this draw — is reified into the queue
at the moment of the gesture."* Reified into **an ordered list of tracks**.
The taxonomy, complete — including the two the owner's second brief asks
about by name:

| The implicit playlist | Who wrote it | Mutable? | Named? | Where it shows |
|---|---|---|---|---|
| An album's track list | the artist (read from tags) | no — fix the tags | by the artist | the record's page |
| **The wall, in its arrangement — *All songs*** | the group key and the filter | by arranging | **yes, since 2026-08-10** | the wall itself; its handle is the playlist panel's first row |
| ~~A shuffle draw~~ | ~~chance, from the wall's visible pool~~ | — | no | — |
| ~~The pull's offer~~ | ~~the ledger's weighting~~ | — | no | — |
| **The queue** | **the listener's gestures, tonight** | **yes** | **no** | the Queue place |
| A saved playlist | the listener, deliberately | yes | yes | its page |
| A generator's output (future) | a person's explicit ask | yes | yes | its page |

So the owner's question — *"what happens when you shuffle the library? has
that created an implicit playlist?"* — has a one-word answer: **yes.** The
run reifies into the queue, which is the implicit playlist: readable to its
end, editable row by row, endable, and one `Save as playlist` from being
kept. And the wall row is the key to play-everything (§7): the arrangement a
listener is looking at *is already an ordered list of records* — playing it
needs no selection ceremony, because the scope is on screen.

> **Two rows changed on 2026-08-10, both on the owner's decisions.**
>
> **The wall's row got a name.** *"The play all thing also does not need to
> exist. That should be existing as a kind of playlist that is implicit."* The
> vocabulary was here and the type was not — `grep -rn "implicit playlist"
> crates/` returned one comment. It is `crate::implicit::ImplicitList` now:
> **All songs**, with a name, a counts line, a collage sleeve, and a row at the
> head of the playlist panel. `Play all` is that list's own `Play` — one
> concept where there were two — and this table's "Named?" column moves from
> *no* to *yes* for it, which is the whole of what was built.
>
> **The type is this table's own column, made code.** On the owner's steer that
> *"every album has a playlist implicitly… it should be basically which
> playlist and which track"*, it is an `Origin` **kind** rather than one
> bespoke thing, and what each variant carries is exactly this table's
> *"Who wrote it"* and *"Named?"* columns resolved into an identity: a file for
> a saved playlist, an album id for a record's own list, nothing durable for a
> draw, a name alone for All songs. Only the All-songs variant is built; the
> rest of this table is where the others come from when they are.
>
> Three things it deliberately is **not**. It is not a *file*: there is nothing
> to append to, so the picker never offers `Add to "All songs"` — asserted in
> `menu.rs` over every target, and closed at its source by the list's run
> carrying no provenance, since provenance is what would put the name in a
> transfer verb (§6). It is not a *snapshot*: it follows the wall's current
> arrangement and the wall's current filter, and it says so — under a query the
> counts read `7 of 1284 records` rather than letting the name claim otherwise.
> And it is not a *page*: this table's own last column already says where the
> wall is seen — **the wall itself** — and a second surface listing the same
> music as text would be doc 07 L8.6's one fact drawn twice, drawn worse.
>
> **The shuffle-draw and pull rows are struck.** The pull was removed. Shuffle
> became a property of the player (ADR-0023's amendment), so there is no draw
> that reifies — what the mode produces is not a new list but a re-ordering of
> the queue row below, which was always the implicit playlist the answer above
> pointed at.

Read down the table and the queue and a saved playlist differ in exactly two
properties: **a name, and whether the engine is holding it.** Everything
else — rows, order, groups, the click-plays-from-here rule, the dot's
honesty condition — is already shared. The model in one sentence:

> **baz has one kind of list. One of them is sounding and has no name; the
> rest are named and silent. Making a playlist is listening plus naming.**

This is the shape prior art calls *multiple lists, one active* —
foobar2000's and Winamp's skeleton (`03-interface-prior-art.md` §5.1) —
**minus the tradition's fatal ceremony**: no play gesture in baz ever
requires naming or choosing a list first, because the unnamed one is
materialized by the gesture itself (ADR-0023 §2). fooyin's first frame
demands a list before a sound (`03` §4.3, first-hand render
`prior-art/fooyin-02`); baz's first frame is a wall you click. The owner's
February workaround — *select the entire library, send it to a playlist,
play that* — was this model performed by hand; §7 keeps its understandable
rule (what you see becomes the list, the list plays) and deletes its
ceremony (the list materializes as the queue; naming is optional and comes
last, not first).

## 3. The hypothesis, and the case that resists it

*"Making a playlist is listening plus naming"* — queue things with gestures
you already know, name the result if it deserves keeping. Its home ground is
S8 (keep what I stumbled into): the flow is already shipped end to end and
needs nothing built. The case that resists it is S9 — *build a road-trip
list from scratch, deliberately* — because building-by-queueing destroys the
current listening: play replaces the run (ADR-0023 §3), and even a
queue-append pollutes tonight's tail with next week's draft. Split it
honestly:

**S9a — building with your ears.** Sit down to assemble the list *as the
thing you are doing*: play a candidate, drop the needle to audition a
transition, queue the next record, reorder, ✕ the misfires, name the
result. This is how mixtapes were actually made — you heard the tape as you
dubbed it — and it is the strongest version of the hypothesis: the
builder's workbench is the one list you can *hear*, and every gesture on it
is a gesture playback already taught. It requires only that the queue place
reach edit parity with the playlist page (§8.2).

**S9b — building silently, while tonight's music must not stop.** Real, and
irreducible: a second list genuinely is in play. Three priced precedents,
each measured against the context-switch cost the owner names:

- **foobar2000 / Winamp — multiple lists, one active.** Build in a
  background playlist tab; playback untouched. The price is permanent:
  *every* play must name a list, the active list is a fact you must track
  (foobar answers "what plays next" with *"not possible"* — `03` §5.2(b),
  Hydrogenaudio wiki FAQ), and the first-run experience is a blank list
  demanding ceremony. The tradition pays at every session for what S9b
  needs weekly.
- **MusicBee — playing versus editing as two resident surfaces.** The Now
  Playing list docked at the side, the centre panel an editor; drag between
  them (`03` §5.2(c), MusicBee's own plugin API and wiki). Context switch
  while editing is genuinely low — both lists are on screen — and the price
  is exactly the resident side surface baz's owner has rejected three times
  (ADR-0022's history).
- **Spotify — the side-channel add.** Playlist building happens entirely
  outside playback (row menus onto a destination page you cannot see);
  playback undisturbed. The price is displaced feedback — fire-and-forget
  into an unseen list — plus the two-structure queue nobody can explain
  (`03` §4.4.5, [kroltan on HN](https://news.ycombinator.com/item?id=34259776)).
  **The shipped baz collecting mode is this pattern with the destination
  made visible** — better than its parent, and still the second grammar
  whose seam the owner is feeling.

The pricing forces the conclusion: **baz already is "multiple lists, one
sounding."** The named lists *are* the silent lists; a playlist page is
already "a list surface that is not playing." S9b does not need a new
structure — a draft mode, a second queue, a workspace — it needs the
*transfer* into a silent list to be one modeless gesture (§8.1), performed
through the context menu or the `+` (2 gestures per addition, band-D
arithmetic per `03` §1.2), with the drag as its eventual one-gesture form. A
"draft mode" of the queue place was considered and rejected by name: two
modes of one surface is *which-list-is-sounding* illegibility — foobar's
disease — arriving through the back door.

**Verdict: the hypothesis is adopted, with one amendment** — the modeless
transfer gesture survives as the side-channel for silent adds; the *mode*
does not (§9).

---

## 4. The user stories

The industry artifacts the owner asked for: each scenario as a **user
story**, its **task flow** (exact clicks, from where the listener actually
is), and **acceptance criteria** in Given/When/Then form, written to be
implemented and tested as stated. The design decisions the criteria depend
on are argued in §5–§9 and cross-referenced; nothing in a criterion is
undecided elsewhere.

A vocabulary note used throughout: **the picker** is the panel serving as a
destination list (§8.1) — its rows, in order: `Queue` (the unnamed list),
the **current playlist** when one stands (§6, marked *playing*), every named
list, `New playlist`. **Needle-drop** is ADR-0023 §2: clicking a track
enqueues its record whole and starts at that track.

### S1 — Play a specific song, via search

> As a listener who knows the song I want, I want to type its name and click
> it, so that it is playing within seconds and I have not had to think about
> albums to get there.

**Task flow** (from any place): ① type — the first keystroke focuses the
resident app-bar query and opens its Tracks/Albums dropover over the unchanged
place; ② select a track with pointer or Up/Down; ③ choose `Play` (Left/Right on
the keyboard) and confirm. The result surface is scrollable rather than a
capped preview of a separately scrolling wall.

**Acceptance criteria**

- Given any place and a non-empty query, the Tracks and Albums sections render
  in one bounded dropover and that place remains underneath; opening/dismissing
  search does not navigate or alter history.
- Given enough results to exceed the dropover, one scrollbar traverses both
  sections. The implementation may virtualize/page results, but it exposes no
  eight-track cap, nested wall scroller or unreachable tail.
- Given a track selection, Up/Down changes the selected result and keeps it in
  view; Left/Right changes `Play | Enqueue`; Enter performs the highlighted
  action. Navigation keys alone do not start sound or alter the queue.
- Given `Play`, preserve the existing needle-drop context unless the later
  interaction design explicitly changes it: queue the selected edition and
  start at that track. Given `Enqueue`, append the track without replacing or
  starting the current run—except over an existing playlist page, where the
  action is labelled `Add to playlist` and appends to that file instead.
- Given no matching tracks but matching albums, the Tracks section is absent,
  not empty, and Albums remain in the same scroller. Album activation behavior
  is completed in the implementation story rather than inferred from tracks.
- Given Esc or click outside, dismiss the dropover and expose the unchanged
  place. The clear/dismiss layering and post-action behavior are settled in the
  implementation brief in `WORK.md`.

### S2 — Browse, open a record, drop the needle

> As a listener browsing my own shelves, I want to open a record and click
> the track I feel like, so that the record plays from there — because I put
> a record on, I didn't extract a song.

**Task flow**: ① scroll the wall (or jump by the index rail); ② click a
tile → the record's page; ③ click a track row → needle-drop.

**Acceptance criteria** (shipped behaviour, restated as the contract —
ADR-0023 §2):

- Given a record's page and a stopped, paused, or otherwise-occupied
  engine, when a track row is clicked, then the record's selected edition is
  queued whole and playback starts at that row; earlier tracks sit behind
  the cursor, reachable by `Previous` and by their rows.
- Given the engine already holds exactly this record, when any of its rows
  is clicked, then the click is a bare `JumpTo` — no re-queue, no
  interruption of the run, shuffle marks undisturbed
  (`player.rs:1782–1795`, `app.rs:1466–1471`).
- Given the record ends, when the last track finishes, then there is
  silence, and the queue place says so in words (`views/queue.rs:190–196`).

### S3 — Send a song to a *new* playlist, mid-listen

> As a listener who just heard something worth keeping, I want to flick it
> into a brand-new list without stopping the music or leaving where I am,
> so that keeping a thing costs a moment, not a task.

**Task flow**: ① right-click the song's row (album page, queue place,
songs section — or the bar's now-playing block for the sounding track) →
② `Add to playlist…` → the picker opens holding the song → ③ `New
playlist` → type a name, `Enter`. Pointer-only route without the menu: the
row's `+` slot → picker → `New playlist`.

**Acceptance criteria**

- Given any track row and the music playing, when `Add to playlist…` is
  chosen (menu or `+`) and `New playlist` is picked and named, then a new
  `.m3u8` exists containing that track, not one delivered sample was
  disturbed, and no transport state changed.
- Given the picker open with a pick in flight, when a name is submitted,
  then the storage layer's refusals (duplicate name, unusable name) surface
  in the field in its own words (`playlists.rs:502–522`), and the pick is
  completed into the created list on success (`playlists.rs:516–518`).
- Given the pick lands, when the panel updates, then the new list's row
  shows its count — the feedback is in view at the moment of the press
  (`playlists.rs:430–433` keeps the panel open).

### S4 — Send the sounding song to the *current* playlist

> As a listener playing one of my playlists, I want any song I'm hearing or
> looking at to go into that playlist in one short gesture, so that the
> list I'm living in grows while I live in it.

**Task flow**: ① right-click the sounding row — or, from anywhere, the
bar's now-playing block — ② `Add to "Road Trip"`. Two gestures, from any
place. Pointer-only route: the row's `+` → the picker, where *Road Trip —
playing* sits directly under `Queue`, hoisted (§6).

**Acceptance criteria**

- Given a queue whose provenance (§6) names a playlist that still exists,
  when any track row (or the now-playing block) is right-clicked, then the
  menu carries `Add to "{name}"` naming that playlist; when no provenance
  stands or the file is gone, the item is absent — a control that cannot
  act must not pretend it can.
- Given `Add to "{name}"` is chosen, when the append lands, then the *file*
  gains the track and **the live queue is unchanged** — the run is
  tonight's snapshot; the file is the kept thing (the decoupling rule,
  ADR-0024 §1, restated in §6 with the tempting alternative refused).
- Given the picker is opened while provenance stands, when its rows render,
  then the current playlist is second (under `Queue`), marked *playing*.
- Given the current playlist's own page is open while it is playing, when
  the file-append lands, then the page re-reads and shows the new row
  (`playlists.rs:491–493`), and the lamp dot's rule is unchanged — it marks
  a row only when the queue is exactly the listed subset
  (`player.rs:1756–1761`), so a file that has grown past its snapshot marks
  nothing rather than lying.

### S5 — See, manage, and prune playlists

> As a listener with a shelf of kept lists, I want one obvious place to see
> them and ordinary gestures to fix them, so that maintenance never feels
> like filing.

**Task flow**: ① `Playlists` (Library strip, or `Ctrl+P`) → the panel:
every list, the unnamed `Queue` at its head as a readout, counts beside
each name; ② click a name → its page; ③ on the page: ✕ removes a row, ▲▼
reorders, `Rename`/`Delete` with the confirm that names the survivor.

**Acceptance criteria**

- Given the panel open, when it renders, then it lists the `Queue` row
  first (name and counts, **a readout, not a door** — §8.1) and every
  `.m3u8` in the folder below it, each row exactly one control (the door to
  its page): the arm `+` is gone (§9).
- Given a playlist's page, when a row's ✕ is pressed, then the file loses
  that entry (missing entries removable too — a dead reference is the
  user's to remove, `playlists.rs:637–648`); when ▲▼ is pressed, then the
  entry swaps with its neighbour and notes keep their places
  (`playlists.rs:654–671`).
- Given `Delete` pressed once, when the confirm renders, then it reads
  *"The file goes; your music stays"* and a second press deletes
  (`views/playlist.rs:245–260`).
- Given a file changed on disk since the page read it, when any edit is
  pressed, then the edit is dropped and the page re-reads — a stale row
  number is never applied to fresh contents (`playlists.rs:615–635`).

### S6 — Play everything

> As a listener who sometimes just wants the whole collection on, I want
> one press that plays everything I'm looking at, so that "play it all"
> costs less than it did in February (select all → send to playlist → play).

**Task flow**: ① `Play all` in the Library strip (§7.1). That is the whole
flow — one press from the resting window, and it meets the friction
budget's *intent → sound in one press* **from the wall**, which no other
gesture does.

**Acceptance criteria**

- Given the Library place with an empty query, when `Play all` is pressed,
  then the queue becomes every record the wall shows, whole records in the
  wall's current arrangement order, and the first track sounds.
- Given a query or a group-key arrangement narrowing the wall, when
  `Play all` is pressed, then the queue is exactly the visible matches in
  visible order — **the scope is the wall**, always; playing what you
  cannot see is refused for the reason shuffle's invisible pool is
  (the product's standing rule). "Everything in the library" is the empty query,
  one `Esc` away.
- Given the resulting queue, when the Queue place is opened, then it is an
  ordinary queue: readable to its end, jumpable, editable, saveable via
  `Save as playlist`, and it **ends** — no wraparound, no refill.
- Given an empty wall (no library, or a query with no matches), when
  `Play all` is pressed, then nothing happens and nothing is claimed —
  shuffle's own empty-pool rule (`app.rs:1538–1543`).

### S7 — Shuffle the library

> As a listener who wants to be surprised by my own collection, I want to
> shuffle what I'm looking at and see what I'm in for, so that surprise
> never means surrender.

**Task flow**: ① (optional) arrange or filter the wall — the pool is what
you see; ② `Shuffle` → eight records drawn, the pool dims, the next two
draws ring, the first drawn record sounds. Reading what is coming: the
bar's continuation line, or the Queue place. Another armful: press
`Shuffle` again.

**Acceptance criteria** (shipped, restated as contract, plus the model's
answer):

- Given a visible wall, when `Shuffle` is pressed, then `SLEEVES` = 8
  records (`shuffle.rs:71`) are drawn without replacement from exactly the
  visible pool, queued whole in whole-record order, and playback starts;
  the non-pool covers dim and the next `RINGED` = 2 draws carry rings.
- Given the draw is playing, when the Queue place is opened, then the whole
  run is legible — **the draw is an implicit playlist**, and `Save as
  playlist` keeps a good one (the owner's question answered in the
  affirmative: yes, shuffling created one, and it is on screen).
- Given the draw is playing, when a row of it is clicked or removed or
  reordered (§8.2), then the run edits like any queue and the pool's marks
  survive anything short of a re-queue (`app.rs:1466–1471`).
- Given the draw ends, when the last track finishes, then silence
  (the product's standing rule); another press is another draw. The bounded draw is
  kept over shuffle-the-whole-pool deliberately: a run you can read to the
  end is a run you own, and eight records is an evening — the unbounded
  alternative is honest but unreadable, and it is one `Play all` +
  future-shuffle-arrangement away if ever wanted (§7.2).

### S8 — Keep what I stumbled into

> As a listener whose evening turned out well, I want to freeze tonight's
> run under a name, so that a good accident becomes a kept thing.

**Task flow**: ① bar's `Queue` door → the Queue place, the evening on
screen; ② `Save as playlist` → type a name, `Enter`.

**Acceptance criteria** (shipped — `views/queue.rs:154–196`,
`playlists.rs:527–555` — restated as contract):

- Given a non-empty queue, when `Save as playlist` is named and submitted,
  then a new `.m3u8` holds exactly the queue's rows in order, with
  `#EXTINF` metadata; the queue is not linked to the file and keeps
  playing.
- Given the queue holds a record entered mid-way (needle-drop), when it is
  saved, then the whole record is saved — the queue records choices, and
  every row about to be frozen is on screen at the moment of saving; what
  actually *sounded* is the ledger's business (ADR-0018).
- Given a name that already exists, when submitted, then the storage
  layer's refusal lands in the field; nothing is overwritten (§8.3 records
  the iterate-on-the-page path and defers overwrite-on-confirm).

### S9 — Build a road-trip list from scratch, deliberately

> As a listener planning next week's drive, I want to assemble and audition
> a list on purpose, so that the deliberate case is as respectable as the
> accidental one.

**Task flow, audible build (S9a — the canonical form)**: ① play a first
candidate record (any play gesture); ② add candidates — `Add to…` → `Queue`
from record pages, shift-click, song-search rows' menu → `Queue`; ③ shape
it in the Queue place — jump to audition transitions, ✕, ▲▼; ④ `Save as
playlist`, name it. **Task flow, silent build (S9b)**: ① panel → `New
playlist`, name it; ② from the wall/pages/search, per addition: `+` or
right-click → `Add to playlist…` → pick it (2 gestures each; the drag will
make it one); ③ its page to audition later.

**Acceptance criteria**

- Given tracks and records added via the picker's `Queue` row, when each
  add lands, then the run is appended without one delivered sample
  disturbed (`UpdateQueue`, ADR-0014), and appending to an empty stopped
  engine loads the queue without starting it (`app.rs:1363–1366`) —
  nothing sounds unasked, so silent assembly *into the queue* is also
  possible when nothing is playing.
- Given the Queue place, when reorder ships (§8.2), then its rows carry the
  playlist page's exact ▲▼ slots and the whole-queue edit goes out as
  `UpdateQueue`.
- Given the silent build, when additions go to the named file, then
  tonight's run is untouched at every step, and the count is visible in
  the picker at each add.

### S10 — A generated playlist (vibes, or a prompt) — *future, ground rules only*

> As a listener who sometimes wants the collection to propose, I want to ask
> for a list — a vibe, a prompt — and receive an ordinary playlist, so that
> being surprised never means being managed.

No task flow is designed here — the generator is future work — but the
model it lands on is finished, and these criteria bind any implementation
(ADR-0024 §7, restated against this document's surfaces):

- Given a generation is requested, when it completes, then the output is an
  ordinary named `.m3u8` in the panel like any other — same rights, same
  page, same picker row — with its provenance as inert comment lines.
- Given the output exists, when it renders, then nothing is playing that
  was not playing before: the artefact arrives silent, and its `Play` is
  the ordinary one (the pull's discipline, `app.rs:1584–1588`).
- Given any later moment, when the generator has not been explicitly asked
  again, then the file has not changed — generation is an act, never a
  condition.
- One precision owed to the record: `baz-core`'s `analysis` module is the
  **ReplayGain measuring service** (`crates/baz-core/src/analysis.rs:1–12`,
  ADR-0015) — a background, incremental, whole-library decode-and-measure
  pipeline whose *shape* is the seam a future vibe signal (bliss-rs,
  `VISION.md` pillar 4, v0.3) would reuse. The vibe features themselves do
  not exist; nothing should cite that module as if they did.

---

## 5. Decision: search answers in songs

> **Amended 2026-08-12:** search now answers in a scrollable app-bar dropover
> over any place. “Songs” below means the dropover's **Tracks** section and the
> album wall means its **Albums** section; the cap and Library-body placement
> are superseded. The ranking/context reasoning remains.

*"People are really searching for songs in most cases."* The data already
agrees: `Library::search` returns ranked **tracks** (`index.rs:1119`,
ADR-0021 — whose entire context section is about a chosen *track* coming
out of the speakers), and the current surface folds them onto albums and
discards the track-level answer (`vm.rs:1810`'s pinned fold). The design:

- **Two sections, one scroller.** Under a non-empty query the app-bar well
  opens a dropover containing ranked `Tracks` followed by `Albums`. It appears
  over whichever place was already open, does not navigate to Library, and has
  one continuous scroll/selection model. Results may be virtualized but are not
  deliberately capped at eight tracks.
- **A track result is a selectable list row** — title, artist · album and
  duration — using the product-wide selection grammar. Playback is explicit
  through its `Play | Enqueue` action choice; merely moving or single-selecting
  the row does nothing.
- **Typing selects nothing.** The Tracks heading teaches
  `↑↓ select · ←→ action · Enter confirm`; the chooser owns those bare arrows
  even while the well still has focus, and its selection is separate from the
  unchanged place underneath.
- On an existing playlist page the second action reads **`Add to playlist`**
  and appends the searched track to that file. On every other place it remains
  **`Enqueue`** and appends to the live run. Neither route starts playback.
- **The press is a needle-drop** (ADR-0023 §2): the song's record is queued
  whole, the cursor on the song. The alternatives are rejected by the
  model: *play the song alone* is three minutes and a dead stop — the
  album-boundary failure at track scale; *queue the result list as a
  context* is Spotify's heterogeneous hidden list (`03` §4.4.5). One
  answer, already learned from S2, no third grammar. The listener who
  truly wants only the song has the ✕ on everything after it — or simply
  plays something else, because leaving is free.
- **`Enter` confirms the selected result's selected action**, rather than
  unconditionally playing the top-ranked track. Type-anywhere is untouched:
  the first bare keystroke focuses the query and opens the dropover in the same
  frame without changing place.

## 5.2 Decision: a context menu, as a mirror layer

The owner reached for right-click twice, and his audience's muscle memory
(foobar2000, MusicBee) reaches for it hourly. baz has no menus of any kind;
this is a new interaction class and gets a governing rule before it gets a
single item, because the visible-control rule (the product's standing rule)
otherwise forbids it — a right-click is a gesture, and no action may be
gesture-only.

> **The context menu is a pointer mirror, governed exactly as the keyboard
> is: every menu item sends a message some visible on-screen control also
> sends, and no action's only route is a menu.** (L8.7's clause — *the
> keyboard is the same decision, made twice* — extended: made three times.
> Proposed as an L8.7 amendment in `07-control-placement.md`'s terms, with
> the same shape of test: `every_menu_item_is_a_press_some_control_also_makes`.)

This is what reconciles menus with L8.6 (*no two controls send the same
message*): a menu item is not a second control any more than a key binding
is — it is an accelerator layer over the controls that exist, and the
binding test, not the control table, is what pins it. Mechanically the menu
is feasible today: `mouse_area` carries `on_right_press` in iced 0.13
(`iced_widget-0.13.4/src/mouse_area.rs:53`), and the menu itself is the
ADR-0016 float mechanics (stack + `opaque` + click-outside `mouse_area`, no
scrim) at the pointer's position; `Esc` and click-outside close it.

The menus, kept short — verbs only, no state, nothing that is not a mirror:

| Object | Items | Each mirrors |
|---|---|---|
| Track row (album page, songs section, playlist page) | `Play` · `Queue` · `Add to "{current}"`* · `Add to playlist…` | the row's press · the picker's Queue row · the picker's hoisted row · the row's `+` |
| Queue row | `Play` · `Add to "{current}"`* · `Add to playlist…` · `Remove` | the row's press · as above · the row's `+` · the row's ✕ |
| Album tile | `Open` · `Play album` · `Queue album` · `Add to playlist…` | the tile's press · the page's `Play album` · the picker's Queue row (record-granular) · the page's `Add to…` |
| The bar's now-playing block | `Go to record` · `Add to "{current}"`* · `Add to playlist…` | the block's press · as above · the sounding row's `+` |
| Playlist panel row | *(no menu at v1)* | its acts live on the page, where the contents are visible |

\* present only while a current playlist stands (§6); absent, not disabled,
otherwise.

The bar row is what makes S4 two gestures *from anywhere*: the sounding
track is always in the bar, so its menu is always one right-click away —
and every item on it mirrors a control that already exists, so the bar
gains no slot and the ratchet (the product's standing rule) is untouched.

## 6. Decision: the current playlist, defined

*"At any time you're playing a playlist, you should be able to… send to
current playlist."* The model must first say what "the playlist that's
currently being played" *is*, because ADR-0024 §1's boundary — playing a
playlist **copies** it into the queue; from that instant they are decoupled
— means the queue is never "a playlist being played." The definition:

> **Playing provenance**: when a queue is reified from a named playlist
> (its `Play`, or a click on its rows), the request-side record
> (`vm::QueueVm`) carries the source's name. Provenance stands through
> everything that is *this run* — jumps, seeks, pause, `QueueEnded`, and
> every `UpdateQueue` edit including appends — and is replaced only when
> the queue is replaced (a `SetQueue` from any other gesture). It is a
> statement about **origin**, never a live link: a run that has been
> edited is still "the run I started from Road Trip," which is exactly
> Plexamp's `playQueueSourceURI` — *"the original request that created the
> queue"* (`03` §5.2(h), [Plex — Play
> Queues](https://support.plex.tv/articles/202188298-play-queues/)).

What it buys, and where it shows:

- **The Queue place's summary leads with it**: `Road Trip · 3 of 12 ·
  38:12 left`. One glance answers *"what list is this run from?"* — a
  question the hierarchy sheet (§10) previously could not answer at all.
- **`Add to "{name}"`** — the context-menu item of §5.2, and the picker
  hoisting the named list to second position, marked *playing*. Available
  exactly while provenance stands and the file still exists; a rename or
  delete under the run withdraws the verb rather than letting it dangle.
- **The append goes to the file only. The run is unchanged.** The tempting
  "both" — append to the file *and* the live tail, so you hear it tonight
  — was weighed and refused: a gesture that writes two structures at once
  is the two-lane confusion coming home (`03` §4.4.5), and the honest
  statements stay separate and both available: *keep it* is `Add to
  "Road Trip"`; *hear it tonight* is `Queue`; doing both is both gestures,
  each doing exactly what it says.
- **Decoupling survives intact**: file edits during the run never move the
  needle; run edits never touch the file; the dot on the playlist's page
  keeps its exactness rule (`player.rs:1756–1761`), so the moment file and
  snapshot diverge the page marks nothing rather than guessing.

## 7. Decision: play everything, and shuffle, specified end-to-end

### 7.1 `Play all` — the wall is a list, so play it

The February workaround — select the whole library, send it to a playlist,
play that — followed a rule the owner correctly defends as understandable:
*what you see becomes the list, and the list plays.* The gesture that
beats it keeps the rule and deletes the ceremony:

**`Play all`, one word in the Library strip beside `Shuffle` and `Pull`**
(same L8.1 home: it reads the wall — the arrangement, the filter — to know
what to do, `07-control-placement.md` §2). One press: every record the
wall currently shows becomes the queue, whole records in the wall's own
order, and the first track sounds. The wall *is* the selection — scope is
always on screen (the invisible-pool refusal generalized), "everything"
is the empty query, and a filtered or re-arranged wall plays exactly what
it shows, which turns the group keys into programme builders for free
(YEAR-arranged wall → the collection in chronological order; a genre
filter → that genre, front to back).

The costs, stated: at Marta's scale this is a five-figure-track queue. The
engine is indifferent (a `Vec<PathBuf>`; ADR-0014's whole-queue edits are
bounded by gesture cadence, and the payload at 40 000 paths is a few
megabytes per edit — real, and accepted there for the desync-proofing);
**the Queue place is not** — it draws every row (`views/queue.rs:70–133`,
an unvirtualized column), so `Play all` at large scale requires the queue
place to adopt the shelf's virtualization before it ships to that
audience. Named as the implementation gate, not designed here.

### 7.2 Shuffle — already specified, now stated as one contract

Shuffle's function end-to-end (all shipped, S7 pins it): the pool is what
the wall shows and nothing else, marked by dimming and rings
(the product's standing rule, `app.rs:1532–1574`); a press draws eight records
whole and queues them as an ordinary finite queue; the draw is an implicit
playlist — readable, editable, saveable, ending in silence; another press
is another draw, and a draw is a *thing you start, never a thing that
starts itself*. What is deliberately still absent: any engine shuffle
mode, any track-granular shuffle, any refill. The bounded draw is chosen
over shuffle-everything for legibility (S7's last criterion); the steered
shuffle of `VISION.md` pillar 4 remains v0.3 and lands, when it lands, as
a different *draw*, not a different structure.

## 8. The unification, concretely

Four changes, each small, jointly one model.

### 8.1 One transfer gesture, and the queue is a destination

Every "put this listed thing into a list" act becomes **one gesture with
one anatomy**: the reserved-slot `+` on a track row (album page, songs
section, queue place), `Add to…` on the record's page (replacing `Add to
playlist`, `views/album.rs:352` — the ellipsis honestly promising a second
press), and the context menu mirroring both. All open the panel as the
picker it already knows how to be — and the picker's **first row is the
Queue**, above the current playlist (when standing), the named lists, and
`New playlist`:

```
 Playlists                    Esc closes
 Add "Amo Bishop Roden" — pick a list
 ─ Queue          8 · 32:10   ← the unnamed list, first
 ─ Road Trip — playing   14 · 51:08
 ─ Late Nights   23 · 1:40:11
 ─ New playlist
```

Picking *Queue* appends to the run — `UpdateQueue`, the music keeps
playing, and appending to an empty stopped engine loads a queue without
starting it (`app.rs:1363–1366`), so nothing sounds unasked. Picking a
named row appends to the file. Picking `New playlist` names one and
appends (`playlists.rs:516–518`). **"Hear this later" and "keep this"
become the same gesture with a different destination**, which is the
unification stated as muscle memory.

Two consequences, both forced rather than chosen:

- **ADR-0023 §3's dedicated `Queue album` control is withdrawn before
  being built.** With the queue in the picker, a dedicated append control
  beside it would be two controls sending one message — L8.6 forbids
  exactly this — and the control never shipped, so nothing regresses.
  Queue-append costs two presses (`Add to…` → `Queue`), inside W8's
  band-C budget (`03` §1.2), and the second press is always in the same
  physical place — the panel's first row — which is what makes a
  two-press gesture cheap in practice. The context menu's `Queue` item
  and shift-click are its accelerators, both mirroring the picker's Queue
  row (the mirror rule of §5.2 is what gives the gesture an on-screen
  control to resolve to).
- At rest — no pick in flight — the panel's Queue row is a **readout,
  not a door**: name and counts in the room's quieter voice, no press.
  The queue's door is the bar's labelled `Queue`, in every place, and a
  second door would be L8.6's other violation. Facts may be restated
  everywhere; controls may not.

### 8.2 The queue place reaches edit parity

The queue place gains the ▲▼ steppers in the playlist page's exact
reserved slots (`views/playlist.rs:412–440` is the anatomy; `queue_edit`
grows the pure reorder the ✕ already has for removal) and the `+` slot of
§8.1. After this, the queue place and the playlist page are **the same
editor** — position column, group headers, click-plays-from-here, ✕, ▲▼,
`+` — differing only in the header block: the playlist's name and acts
versus the queue's provenance-led summary and `Save as playlist`. One
grammar, learned once, and the builder's workbench (S9a) is complete.

### 8.3 Naming is the creation act

`Save as playlist` stays where it is and becomes what creation *is*: the
moment the unnamed list earns a name — the owner's *"when do we create a
playlist that lives longer than the time it's played for"* answered as a
gesture: **when you name it, and not before.** The panel's `New playlist`
remains for the deliberate silent start (S9b) and as a pick target;
nothing else creates. One wrinkle stated rather than hidden: after
naming, the queue and the file are decoupled (ADR-0024 §1 —
deliberately), so continuing to build and saving again under the same
name is **refused** by the storage layer's no-overwrite rule, with its
words in the field. The iteration path is the artefact's page, whose
editor is now identical to the one you were just using (§8.2). An
explicit overwrite-on-confirm is deferred until someone hits the edge;
guessing at it now would add a destructive path to the product's one
naming flow.

### 8.4 "Jumping out is picking another song" — verified

Already true, end to end, and worth stating because the owner asked for
it by name: a playlist's `Play` reifies the file into the queue
(`app.rs:1323–1345`); playing anything else replaces the run (`SetQueue`,
ADR-0023 §3); the file is untouched by any of it (`playlists.rs` module
docs: nothing writes a playlist but the user's own edit to that
playlist). There is no "playlist mode" to leave, because there is no
mode: leaving a playlist *is* picking another song, one press, wherever
that song is. The unification adds nothing here — it removes the one
thing that contradicted it, the armed state, which was a mode you could
forget you were in.

## 9. What is removed: the armed collecting mode

The verdict first: **arming (ADR-0024 §6 layer 2) is removed** — the
per-row receive `+` (`views/playlist_panel.rs:233–263`), the `armed`
state (`playlists.rs:238`), the relabelled record-page control
(`views/album.rs:356–360`), the armed arm of every add
(`app.rs:1283–1287`).

The case for it, at full strength: arming compresses a bulk silent build
from two presses per addition to one; it was designed as the record-shop
gesture (the crate on the counter, 08 §5.6); it is visible while it
stands and reversible in one press. Nothing about it is dishonest.

Why it goes anyway:

1. **It is the second grammar.** §1's whole diagnosis. Arming makes the
   wall wear a collecting state, splits "what am I building" off to a
   side surface, and is the one thing in the product that answers *"what
   does this press do"* with *"it depends what you armed earlier"* — a
   mode, the thing every dismissal-model lesson in baz's history
   (ADR-0016's rail, ADR-0022's `Ctrl+B`) was learned against.
2. **Its frequency arithmetic fails.** The workflow it optimizes — bulk,
   silent, deliberate collection — is band D at most (`03` §1.2), and
   under §3 its strongest instances re-model as audible building anyway.
   L8.2's rule is that band D may not buy resident state; a mode is
   resident *cognitive* state, dearer than pixels.
3. **The owner's discomfort is an observation, and observations beat
   priors** — the same epistemology ADR-0022 used to delete the inspector
   the prior art defended. He has used the shipped mode and named the
   feeling. It shipped yesterday; the sunk cost is zero.
4. **Its one-press economy has two successors that are not modes**: the
   context menu (§5.2 — `Add to "{current}"` is two gestures from
   anywhere, with zero setup and zero teardown, where arming was two of
   setup, one per add, and one to leave), and the drag (ADR-0024 §6
   layer 3, unchanged, still pending the pointer-capture widget) —
   modeless simultaneity, the destination appearing *because your hand is
   carrying something*, which was the owner's original ask verbatim.

What the removal costs, stated: a twenty-track silent build goes from ~22
presses to ~40 (or ~20 where the queue is an acceptable vehicle, via
shift-click), until the drag lands. Accepted, at band-D frequency, as the
price of one grammar.

Consequences for the visible-control rule: the track rows' `+` keeps its
rest-drawn state **whenever the panel is open** (`views/album.rs:680`'s
`panel_open` arm survives; only the `armed` arm dies), and when the panel
is closed the `+` is hover-revealed with the record page's `Add to…`, the
context menu, and the panel door as the always-visible routes to the same
destinations. The panel's rows drop to **one control each**, which is the
shrinking the hierarchy question wanted: the panel stops being a
workspace and becomes what the first brief asked for — *"a way to see
playlists"* — a directory of every list baz holds, the unnamed one at its
head.

## 10. The information hierarchy, in one table

The answer sheet after this study — each question, the one place that
answers it, and what answered it before:

| Question | Where you look | Before this study |
|---|---|---|
| What is playing? | the bar — every place | unchanged |
| What plays next? | the bar's continuation line; the Queue place for the whole of it | unchanged |
| What list is this run from? | the Queue place's summary — `Road Trip · 3 of 12 · 38:12 left` | **unanswerable** — provenance existed nowhere |
| What lists do I have? | the panel: every list, the unnamed sounding one first | the panel: named lists only; the queue elsewhere |
| **What am I building?** | **the Queue place — the list you can hear** (or the named page, for a silent build) | the armed row's ticking count, *or* the queue, depending which grammar you were in |
| Where is the song I'm thinking of? | type it — the `Songs` section, ranked | typing found only its album |
| How do I keep it? | `Save as playlist` on the queue; `Add to "{current}"` / `Add to playlist…` on any row | three shapes: pick, arm-then-press, and a designed-but-unshipped `Queue album` |
| How do I put *this* somewhere? | the one `+` / `Add to…` / right-click → pick the destination (Queue first, the playing list second) | as above |
| How do I play everything? | `Play all` — one press, scope = the wall | select-all-to-a-playlist by hand (February) |
| How do I leave a list? | play anything else — there is nothing to leave | unchanged, but now without a mode that outlives the thought |

The sheet is longer than §6-of-old by the rows the second brief demanded
(songs, provenance, play-everything) and shorter where it counts: "what
am I building" collapses into "what plays next," the add gestures
collapse from three shapes into one, and no row's answer is a mode.
Places stay four-plus-one (`Library · Album · Queue · Playlist ·
Settings`, the bar everywhere, one summoned panel); nothing new is
resident.

## 11. The friction budget, re-checked

*Intent → sound in one press from anywhere sound can be meant;
add-to-playlist in two gestures or fewer.* The rows that moved or are
new:

| Flow | Presses | Budget |
|---|---|---|
| Play a known song (typing aside) | 1 — click its result row | ✓ new — was 2+ via its album |
| `Enter` on a query | 1 — the top song sounds | ✓ |
| Play everything | 1 — `Play all` | ✓ **from the wall** — the budget's first wall-resident pass |
| Shuffle | 1 | ✓ unchanged |
| Send sounding song to current playlist | 2 — right-click the bar, `Add to "{name}"` | ✓ from anywhere |
| Send any row to a new playlist | 3 — menu/`+`, `New playlist`, name | ✓* naming is the third gesture and is the point |
| Add a row to a kept list | 2 — `+` or menu, pick | ✓ |
| Queue a record for later | 2 — `Add to…`, `Queue` (1 by shift-click / menu) | ✓ W8 band C; the dedicated 1-press control traded under L8.6, stated in the amendment |
| Bulk silent build, per addition | 2 (1 by drag, when it lands) | band D — conceded; was 1 under the removed mode |
| Bulk audible build, per addition | 1–2, and you hear the result | ✓ — the hypothesis's payoff |
| Name the build | 2 — `Save as playlist`, name | ✓ |
| Leave any list | 1 — play something else | ✓ |

Everything not listed is unchanged from 08 §7, including its two standing
concessions (play-a-record-from-the-wall and play-a-playlist at two
presses), which this study inherits and does not widen — and `Play all`
plus the songs section mean the two commonest *sound* intents (a specific
song; everything) now meet the one-press budget exactly where the old
sheet conceded it.

## 12. What survives untouched

- **Play means now** (ADR-0023 §3): no gesture here overloads a play
  press with an append — the transfer gesture is its own control on every
  surface, and fooyin's append-and-nothing-plays remains refused.
- **The storage model and every honesty clause** (ADR-0024 §1–§3): files,
  decoupling at play time, missing entries surfaced, nothing writes a
  file but its owner's edit. Provenance (§6) is front-end session state
  about the *queue*, not a property of the file.
- **The sentiment-generator guarantees** (ADR-0024 §7, S10): untouched
  and now concretely landed — the generated file appears in the same
  panel, edits with the same grammar, and gains the same picker row as
  every other list.
- **The engine**: nothing in this document is a protocol change. Songs
  search is a surface over data the index already returns; menus,
  provenance, `Play all`, reorder, the picker's appends — all
  `SetQueue`/`UpdateQueue`/`JumpTo`; naming is a file write.
- **the product's standing rules as amended**: the panel's single-tenant clause holds
  under the ADR-0024 amendment's restatement — the Queue row is not a
  second tenant, because the panel's subject was always *ordered lists of
  tracks* and the unification's claim is that there is one kind. The
  no-side-surfaces entry, the silence entry, the invisible-pool entry and
  the bar's slot ratchet are all load-bearing above and none is touched.

## 13. Cost and order

1. **Remove arming** — deletions in `playlists.rs`, `playlist_panel.rs`,
   `album.rs`, `app.rs`; panel rows simplify to one control. The cheapest
   step, and the one the owner's discomfort names.
2. **The picker's Queue row and the provenance hoist** — pick rendering in
   the panel; one append arm in `app.rs` reusing `queue_playlist`'s shape
   (`app.rs:1347–1375`); `QueueVm` gains the provenance name;
   `Add to playlist` relabels to `Add to…`; the Queue place's summary
   leads with the name.
3. **The songs section** — a `Songs` block over the wall reading
   `Library::search`'s existing ranked tracks; row press = the
   `play_track` path; `Enter` retargets to the top track. (ADR-0021's
   ranking is already the hard part, and it shipped.)
4. **The context menu** — the float-at-pointer widget on ADR-0016's
   verified mechanics and `mouse_area::on_right_press`; the mirror test
   (`every_menu_item_is_a_press_some_control_also_makes`); the four menus
   of §5.2's table.
5. **Queue-place parity** — ▲▼ slots on queue rows; `queue_edit` grows the
   pure reorder; the `+` slot joins the queue row's reserved set.
6. **`Play all`** — one strip control; `vm::stacked_queue` over the wall's
   visible order (`vm.rs:780` already builds exactly this shape for
   shuffle). Gated at large scale on queue-place virtualization (§7.1),
   which is its own step.
7. **Shift-click** — the accelerator, now that its on-screen control
   exists.
8. **The drag** — unchanged from ADR-0024 §6 layer 3, pending the shared
   widget; it lands on panel rows already doing the no-drag job.

Steps 1–2 alone dissolve the two-grammar seam; steps 3–4 are the second
brief's two new surfaces; each step ships whole and none waits on the one
after it.

---

## 14. Summary

The owner saw the model before the product did: there are implicit
playlists everywhere, because every play gesture already reifies an
ordered list — the album the artist wrote, the wall the arrangement
wrote, the draw chance wrote, the run tonight wrote. baz has one kind of
list; one is sounding and unnamed, the rest are named and silent. Making
a playlist is listening plus naming — the answer to *"when does a
playlist outlive its playing"* is *when you name it, and not before* —
and the one transfer gesture (any row, `+` or right-click, pick a
destination: the Queue first, the playing list second) is the whole of
the remaining ceremony. Search answers in the unit people think in.
`Play all` makes February's workaround one press. Shuffle was an implicit
playlist all along, and now the product says so. The armed mode, one day
old, goes — taking the second grammar and the split hierarchy with it —
and leaving was always free; now nothing pretends otherwise.
