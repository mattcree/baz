# 08 — Playback and playlists: the model, honestly stated

> **Amended 2026-08-12 — deliberate album starts land on playback truth.**
> Explicit Play and album activation now share `start_and_show`. Command
> acceptance arms Now Playing; a matching engine `TrackStarted` opens it.
> Empty/refused/dead or wholly failed runs do not navigate. This changes the
> destination after the gesture, not the list-and-cursor model below.

> **Amended 2026-08-12 — one content grammar supersedes the click counts in
> this study.** One click now selects/highlights every playable tile and row;
> double click activates it. Album/list tiles play, while track rows
> needle-drop or jump through the paths this document specifies. Explicit
> labelled Play controls remain direct. The queue/playback model below is
> unchanged; statements that tile clicks navigate or row single-clicks play
> describe the implementation studied on 2026-08-09 and are historical after
> ADR-0022's interaction amendment.

> The owner's questions, in full, because the whole document is an answer to
> them:
>
> *"I think we just need to delve into how playback works. if I just click a
> track from any album, it should start playing that album, but is it enqueuing
> that album and starting from that song? we need to model playlists honestly.
> cos we want to be able to generate a playlist based on user sentiment? the
> underlying model of this isn't clear to me so maybe we should give it a real
> deep dive in terms of HCI and UX."*
>
> *"I think we need a complete deep dive on how users interact to play, create
> playlists, edit playlists."*
>
> *"I think a sidebar/collapsible panel for playlist is okay. we need a way to
> see playlists, and possibly a section. it should be really easy to drag a
> song into a playlist."*
>
> A design specification, not an implementation. Written 2026-08-09 against
> `4c47dd8`. Every claim about today's behaviour is read off the code and cited
> `file:line`; every prior-art claim carries a named source, either directly or
> through [`03-interface-prior-art.md`](03-interface-prior-art.md)'s sourced
> findings. The decisions this document reaches are extracted into
> [ADR-0023](../adr/0023-playback-model.md) (the playback model) and
> [ADR-0024](../adr/0024-playlists.md) (playlists), both **Proposed**.

---

## 0. What this document is judged against

1. **[the product's standing rules](../the product's standing rules), which binds.** No autoplay, no
   radio, silence is a feature (the product's standing rule). No invisible shuffle
   pools (`:28`). No auto-generated playlists (`:32`). No side surfaces
   (`:92–96`) — an entry §5 must engage rather than sneak past, because the
   ledger's editing rule says a refusal leaves only by an ADR that beats its
   argument. And the visible-control rule (`:112–115`): every action has a
   visible, pointer-reachable control; no action is keyboard-only; no control's
   only affordance is hover.
2. **The friction budget.** *Intent → sound in one press from anywhere sound
   can be meant; add-to-playlist in two gestures or fewer.* Stated in §7 and
   checked flow by flow, including the flows that miss it.
3. **The places model** (ADR-0022): one place at a time, the bar in every one
   of them, nothing else on screen. A playlist surface has to earn its place in
   that model, not beside it.
4. **The prior-art evidence** (`03-interface-prior-art.md`): twenty-one
   workflows ranked into five frequency bands, and the queue-placement study
   whose findings — one list with a cursor, transient must not mean
   unverifiable, model the queue as what you chose — this document extends from
   the queue to the playlist.

---

## 1. Today's playback model, read off the code

### 1.1 What the queue *is*

The engine holds exactly one structure: **a list of paths and a cursor**.
`Control` in `crates/baz-core/src/engine.rs:1180–1182` carries `queue:
Vec<PathBuf>` and `position: usize`, and that is the entire state. There is no
shuffle flag, no repeat flag, no continuation policy, no second lane, no
"context" object behind the list (ADR-0014 "Deliberately left out" records
shuffle and repeat as queue *policies* a front end expresses by sending the
order it wants). A playing session snapshots the queue as `Arc<[PathBuf]>`
(`engine.rs:2377`), which is what makes an edit unable to disturb a delivered
sample: the audio in flight was cut from a value, not a view.

The protocol over it (`crates/baz-core/src/protocol.rs:87–265`):

| Command | What it does | Where defined |
|---|---|---|
| `SetQueue { paths }` | **The reset.** Replace the queue; any playback stops. "Forget what you were doing and hold this instead." | `protocol.rs:94` |
| `UpdateQueue { paths }` | **The edit.** Same payload, opposite intent: the music keeps playing; an edit that misses the playing track disturbs no delivered sample (ADR-0014). | `protocol.rs:160` |
| `Play` / `Pause` / `Stop` | Transport over the current position. | `protocol.rs:165–170` |
| `Next` / `Previous` | Relative steps; `Previous` restarts past 3 s (`PREVIOUS_RESTART_MS`). | `protocol.rs:173, 197` |
| `Seek { position_ms }` | Absolute, inside the sounding track. | `protocol.rs:214` |
| `JumpTo { position }` | Absolute, inside the queue — "`Play` aimed at a chosen entry". | `protocol.rs:260` |

The engine validates nothing, deduplicates nothing, reorders nothing
(ADR-0014 Consequences). It has no opinion about *what* is queued. Everything
that follows in this document — playlists included — is expressible over these
six commands, and §8 confirms that nothing here changes the engine.

### 1.2 The vocabulary, precisely

Three concepts recur in every product studied, and naming them is what makes
the rest of the document sayable:

- **The playing context**: the thing the listener chose — *this record*, *this
  playlist*, *a shuffle of what the wall showed*. In some products it is a live
  object the player keeps consulting (Spotify's context, Plex's
  `playQueueSourceURI`); in others it exists only for the moment of the
  gesture.
- **The queue**: the concrete ordered list of tracks that will sound, and the
  cursor's position in it.
- **History**: what actually sounded, which is not the same list — jumps,
  skips and edits make them diverge.

baz's model, stated in these terms: **the context is reified into the queue at
the moment of the gesture, and then discarded.** `Play album` reads the
record's selected edition and sends its paths (`app.rs:1403–1428` via
`vm::album_queue`, `vm.rs:733`); from that moment the engine holds *a list*,
not *an album*. The queue is **a record of a choice, not a live view of a
source**: re-tagging the album mid-play moves nothing, choosing a different
edition on its page moves nothing (`player.rs:1744–1761` deliberately marks no
row when the listed edition is not byte-for-byte the queue), and no policy can
reach into the list after the fact. History, separately, is the ledger
(ADR-0018) — the queue's rows *behind* the cursor are drawn faint
(`QueueRowState::Played`, `views/queue.rs:239–241`) but the ledger is the only
record of what was actually heard.

The one-list-with-a-cursor shape is MusicBee's, adopted by name (ADR-0016; the
queue place's module docs restate it, `views/queue.rs:30–38`): history behind
the cursor, queue ahead, one surface, and the summary reads what is *left*
(`3 of 12 · 38:12 left`).

### 1.3 What each gesture does today, gesture by gesture

Every row read off the code, not the docs:

| Gesture | What is sent | Result | Where |
|---|---|---|---|
| **Click / double-click a tile on the wall** | first click: nothing; double-click: record queue + `Play` | The first click selects/highlights. The second matching click plays the record and opens Now Playing when the engine confirms a track began. `Open` remains an explicit navigation control. | `selection.rs`; `app.rs::activate_content`; `app.rs::start_and_show` |
| **`Play album` on a record's page** | `SetQueue{album's edition, whole, in order}` then `Play` | The record replaces whatever was queued, from track 1; matching `TrackStarted` opens Now Playing. Failure stays on the record. | `app.rs::play_album`; `app.rs::start_and_show`; the control in `views/album.rs` |
| **Double-click a track row on a record's page** | *decision*: engine already holds exactly this list → `JumpTo{row}`; else `SetQueue{album}` then `JumpTo{row}` | **The album is enqueued whole and playback starts at that song.** The first click only selects; activation still spends `PlayerState::play_from`. | `selection.rs`; `app.rs::play_track` |
| **`Enter`** (with a query) | as `Play album`, for the top-ranked match on the wall | ADR-0021's ranking, filtered through what is visible; with no query, the record last opened. | `app.rs:759–760, 2255–2273` |
| **Double-click a row in the Queue place** | `JumpTo{row}` | First click selects; the second jumps. No queue decision is needed because this list *is* what the engine holds. | `selection.rs`; `app.rs::jump_to_queued` |
| **A queue row's ✕** | `UpdateQueue{list minus that row}` | The music keeps playing (ADR-0014). | `app.rs:1649+`; `views/queue.rs:328–361` |
| **`Shuffle`** | `SetQueue{drawn records, whole, in whole-album order}` then `Play` | A *finite* queue of `shuffle::SLEEVES` records drawn from the visible pool; the pool is marked on the wall; the run **ends**. No mode, no flag, nothing to turn off. | `app.rs:1517–1574`; `vm::stacked_queue`, `vm.rs:780` |
| **`Pull`** | **nothing** | One record offered on its page; accepting it is its ordinary `Play album`. | `app.rs:1576–1616` |
| **The album ends** | — | `QueueEnded`; the bar reads *Nothing playing*; the queue place says so in words: *"When a queue ends, baz stops."* | `views/queue.rs:177–204` |

Three absences, stated because §3–§5 exist to fill or defend them:

- **"Add to queue" does not exist.** `docs/BACKLOG.md:313–316` says it
  plainly: *"There is one way to queue anything — play an album, which
  replaces the queue wholesale. 'Add to queue' / 'play next' are now
  expressible (`UpdateQueue` with the entry inserted where it belongs) and
  want a shelf-side gesture to send them."* ADR-0017 step 13 designed the
  gesture (the stack, shift-click) and it has not shipped.
- **Playlists do not exist.** Not in the config, not in the schema, not on
  any surface. The only `playlist` in the tree is
  `playback::engine::run_playlist`'s name for its list-of-files argument
  (`crates/baz-core/src/playback/engine.rs:214`) — the low-level pipeline,
  not a user concept.
- **The queue does not survive a quit.** `Config` persists `music_dirs`,
  `replay_gain`, `group_key`, `density` and nothing else
  (`crates/baz/src/config.rs:114–156`). Prior art's W2 (*resume what I was
  playing*) is unimplemented, and `03` R14 already recommended it.

### 1.4 The direct answer to the owner's question, for today's build

*"If I just click a track from any album… is it enqueuing that album and
starting from that song?"*

**Yes — exactly that, by design, and the design is right.** A click on track 7
of a record's page sends the record's selected edition, whole and in order, as
the queue, and drops the cursor on row 7 (`app.rs:1449–1501`). What plays next
is track 8, then the rest of the record, then silence. Tracks 1–6 sit *behind*
the cursor — reachable by `Previous` and by clicking their rows, drawn faint in
the queue place — and will not sound unasked. Whether this is the *final*
answer, and what happens on the second click, is §3.

---

## 2. How everyone else models it

The prior-art study (`03-interface-prior-art.md`) covered queue *placement*
exhaustively. This section covers the queue and playlist *model* — what the
structures are, not where they are drawn — because that is the half the
owner's question is about. Three families cover every product examined.

### 2.1 Family one: the playlist *is* the playback model

**foobar2000.** Playback runs through the active playlist; there is no
separate "queue" surface at all — the bolt-on playback queue has had **no
built-in view in twenty-four years**, is flushed by default when you pick a
track by hand (the `foo_keep_queue` component exists solely to prevent this),
and the wiki FAQ answers "what plays next?" with *"This is not possible in
foobar2000 since v0.9.5.3"* (`03` §5.2(b), sourced to the Hydrogenaudio wiki
FAQ and the foo_keep_queue component page). The library is a catalogue;
playing anything means first materializing it into a playlist. That is the
tradition's deepest structural choice, and the one baz most needs to refuse:
fooyin, its heir, opens on an **empty playlist** occupying 80 % of the window,
and double-clicking an album *appends it to a playlist and starts nothing*
(`03` §3 W1, first-hand render `prior-art/fooyin-06`).

**Winamp** had no queue at all in 2.x — the playlist window was the queue
(`03` §5.1). **MPD** is the purest statement of the family: one structure,
the queue, which *"used to be called 'current playlist' or just 'playlist',
but that was deemed confusing, because 'playlists' are also files containing a
sequence of songs"* — stored playlists are files that `load` copies **into**
the queue, and consume mode eats the queue as it plays
([MPD protocol documentation](https://mpd.readthedocs.io/en/latest/protocol.html)).
MPD also solved a problem ADR-0014 solved the other way: it addresses queue
entries by *stable id* as well as by position, where baz sends the whole list
and re-derives the cursor by path identity.

**cmus** runs the family's most elegant refinement: library or playlist as the
*source*, plus a priority queue that overrides it — queued tracks *"are played
before anything else (i.e. the playlist or library)"*, and *"once the queue is
empty, playback will resume from the last position in the library"*
([cmus manual, `Doc/cmus.txt`](https://raw.githubusercontent.com/cmus/cmus/master/Doc/cmus.txt)).
That fall-through — queue empties, source resumes — is precisely a
continuation policy, and baz has refused continuation policies.

**What the family gets right**: one visible structure; what you see is what
plays; playlists are durable artefacts distinct from the transient run.
**What it gets wrong**: playing anything destroys your list (or demands the
ceremony of building one first), and browsing is decoupled from playing — the
`VISION.md` pitch line *"no playlist ceremony"* is a refusal of exactly this.

### 2.2 Family two: context plus queue, two structures

**Apple Music** runs a playing context plus **Up Next as a stack of
contexts**: *"if you're listening to a playlist, you can choose an album to
switch to after the song currently playing finishes. When the album finishes,
Music resumes playing the playlist"*
([Apple support](https://support.apple.com/guide/music/queue-your-songs-musb1e6d1c76/mac);
`03` §4.1.3). The interposed album is a unit; the suspended context survives
underneath. It is the most sophisticated model in the field and nearly nobody
copies it correctly.

**Spotify** runs the same two structures and documents neither: a context lane
and a manual-queue lane with different lifetimes, where playing anything new
wipes the manual queue. The best statement of the failure is a user's
(`03` §4.4.5, [kroltan on HN](https://news.ycombinator.com/item?id=34259776)):

> *"Playing an album plays the first song in the album, and puts the rest in
> the 'up next' part of the queue, but queueing an album queues all its songs
> in the 'queue' part of the queue. 'up next' goes after 'queue', so this
> means I will hear song A1, then B1, B2, […], then A2, A3."*

Spotify additionally appends a **radio tail** — autoplay continues with
similar tracks when the context ends — which is the piece the product's standing rules
forbids outright. **Plexamp** models the context explicitly and well:
its Play Queue is a server object carrying `playQueueSourceURI`, *"the
original request that created the queue"*, so the system knows you are playing
*In Rainbows* rather than ten tracks that happen to be from it (`03` §5.2(h),
sourced to [Plex — Play Queues](https://support.plex.tv/articles/202188298-play-queues/)
and [python-plexapi](https://python-plexapi.readthedocs.io/en/latest/modules/playqueue.html));
its `Autoplay` is a user-selectable continuation policy. **Roon**'s queue is a
view opened from the transport with the playing track pinned at its head and a
*"Jump to now playing"* control
([Roon KB — The Queue](https://help.roonlabs.com/portal/en/kb/articles/the-queue)).

**What the family gets right**: "play this now" and "hear this later" are both
one gesture, and a chosen unit survives as a unit. **What it gets wrong,
repeatedly**: two structures with different lifetimes that the surface renders
as one list, insert semantics illegible at the moment of the gesture
(Plexamp's *"Add to Queue, despite the icon, adds the track Next"* complaint
stream, `03` §5.2(e)), and — Spotify — a queue that is not yours because a
recommender shares the pen.

### 2.3 Family three: one list with a cursor, reified

**MusicBee**: one Now Playing list; history behind the cursor, queue ahead;
three insert positions into the same list (`NowPlayingList_PlayNow`,
`_QueueNext`, `_QueueLast` in its own plugin API header, `03` §5.2(c)); saved
playlists a wholly separate API family. **Albums (iOS)** ships the album-first
refinement: a **two-level queue** — albums as rows, expandable to tracks,
swipe to remove a track (`03` §5.2(g)). **Longplay** queues whole albums, and
its 1.0 → 2.0 history is the standing warning about the album boundary
(`03` §5.2(d)): 1.0 stopped dead at the end of every album and the developer
reversed it — not to a radio, but to an explicit choice between Infinite Album
Shuffle and a manual Album Queue.

**baz is in this family**, and the study's R5 already recommended saying so
explicitly. What baz adds to it is the *reification rule* of §1.2: the list is
a value the front end constructed and the engine holds verbatim, so nothing —
no source, no policy, no service — can write into it except a user gesture.

### 2.4 What the survey decides

1. **Stay in family three.** Family one's ceremony is refused by the vision;
   family two's dual structure is the best-documented confusion in consumer
   audio. One visible list, a cursor, whole-queue edits: the model already
   shipped is the right one and every decision below builds on it rather than
   beside it.
2. **Take family two's one good idea at the boundary where it is honest.**
   "Hear this later" (§3.3) inserts *whole records at record boundaries* into
   the one list — Apple's stack-of-contexts expressed inside MusicBee's single
   structure, which is exactly the synthesis Albums (iOS) shipped.
3. **Playlists are family one's durable artefact, kept out of the playback
   path.** A playlist is a file; playing it copies it into the queue (MPD's
   `load`, exactly); the queue never writes back. §4.
4. **The context is worth *recording*, not *keeping live*.** Plexamp's
   `playQueueSourceURI` knows what you asked for; baz's queue place already
   renders provenance (album headers over runs of rows, `views/queue.rs:143`)
   from the request-side record (`vm::QueueVm`, `vm.rs:586`). Recording what
   was chosen is honest; a live context that keeps acting after the gesture is
   a policy, and policies are what the refusals exist to keep out of the
   engine.

---

## 3. The playback model, designed — the answer with reasons

This section gives the owner's question its definitive answer, in four parts:
the click, the end, the second click, and "add to queue" beside all three.
Extracted as [ADR-0023](../adr/0023-playback-model.md).

### 3.1 The click: a track click queues the record and drops the needle

**Decision: what ships today is confirmed as the model, and named.** Clicking
a track on a record's page enqueues **that record, whole, in order** (the
selected edition), and starts playback **at that track**. The vocabulary is
the record's own, and the skeuomorphism rule permits exactly this use of it
(the product's standing rules, Skeuomorphism: physics, structure and vocabulary — never
surface): the gesture is **dropping the needle**. You did not extract track 7
from the record; you put the record on and set the needle down at track 7.

Why this and not the alternatives:

- **Not "play only this track."** The album is the unit (`VISION.md` pillar
  1); a click that played one track and stopped would make the product's most
  natural gesture produce three minutes of music and a dead stop, which is the
  album-boundary failure at track scale. Spotify needed Adele to make even
  *Play* mean the album in order (`03` §3 W1); baz gets it by construction.
- **Not "play from here to the end only."** The tracks before the click go
  *behind* the cursor, not into the void: `Previous` reaches them, their rows
  are clickable, and the queue place draws them faint as the history side of
  the one list. A listener who dropped the needle mid-record can lift it back.
  Discarding them would make the queue lie about what record is on.
- **Not "start the album from track 1."** The listener pointed at a track. A
  click that plays a different track than the one under the pointer fails the
  same test ADR-0014 applied to out-of-range jumps: playing something the
  listener did not point at is the worst answer available.

One refinement over today, at zero model cost: **the same click inside the
record that is already sounding is a jump, never a re-queue** — already true
(`player.rs:1782–1795`: `holds_exactly` → `JumpTo` alone), stated here so it
is a rule rather than an implementation detail. Moving around inside the
record you are listening to never resets the run, never interrupts delivery,
and never disturbs a shuffle's marks (`app.rs:1466–1471`).

### 3.2 The end: silence, reaffirmed against the strongest counter-evidence

When the queue's last track ends, **nothing happens**. the product's standing rule is
already binding — *the queue empties and there is silence; silence is a
feature* — and this document re-tested it against the strongest evidence on
the other side before repeating it, because Longplay 1.0 shipped exactly this
and reversed it within one major version (`03` §5.4).

The reversal does not bind baz, for the reason ADR-0017 §2 gave and this
document extends: **Longplay had no way to ask for more before the silence,
and baz has three.** *Hear this later* (§3.3) is a standing answer given in
advance; `Shuffle` is an explicit continuation from a visible pool; the
playlist (§4) is a continuation the listener authored. The refusal was never
"you may not continue" — it is "the software will not decide to continue for
you." What the silence buys is the integrity of everything else: a queue that
can grow a tail nobody asked for is a queue whose end state is illegible, and
every "what did I just do" complaint stream in the study (Plexamp's, Spotify's)
starts at a structure the user does not fully own.

The queue place already spends its empty state saying this in words
(`views/queue.rs:190–196`: *"When a queue ends, baz stops."*) — the decision
is on screen at the exact moment every other player would have started
something.

### 3.3 The second click, and "hear this later"

**A play gesture aimed at a different record replaces the queue.** Clicking a
track (or `Play album`) on a record the engine is not holding sends `SetQueue`
— the run in progress ends because the listener superseded it. This is kept,
and it is the load-bearing half of the model: *play means now.* The moment
"play" sometimes means "append" the product has re-created fooyin's defining
failure (double-click appends, twice, and nothing plays — `03` §3 W1) or
Spotify's A1-B1-B2-A2 interleave. One gesture, one meaning, everywhere.

"Hear this later" is therefore **its own gesture with its own name**, and it
is the stack ADR-0017 step 13 already adopted and sequenced — this section
supplies the semantics it left implicit:

1. **Queueing is at record granularity by default.** Shift-click a sleeve, or
   a record page's `Queue album` control (§3.5), appends **the whole record**
   to the end of the queue as its own group. Albums are listed as albums,
   never flattened (`views/queue.rs:138–143`; the refusal by way of the
   critique's stack).
2. **A queued track is its own one-row group.** Shift-click a track row
   appends that track alone. It does not smuggle its album in — the listener
   pointed at a track — and the queue place heads it with its record's name
   like any other group, so provenance survives.
3. **Insertion is append-only for now.** MusicBee has three insert positions;
   Apple inserts after the current song. baz ships **`Queue last` only**, and
   defers `Queue next`, for two reasons stated rather than discovered later.
   First, *next* is ambiguous at the album boundary — after the current
   *track* splits the record you are inside (breaking never-flattened); after
   the current *record* is probably what an album-first listener means but is
   precisely the kind of insert-semantics illegibility Plexamp's complaint
   stream is made of (`03` §5.2(e)). Second, the queue place already gives the
   deliberate listener the general tool: append, then drag the group where you
   want it (once reorder ships) — or jump. When `Queue next` arrives it means
   **after the sounding record**, at the album boundary, and the ADR records
   that in advance so it is not re-litigated at the keyboard.
4. **Mechanically it is `UpdateQueue`** with the new rows appended — the music
   keeps playing, per ADR-0014's guarantee. No new engine surface.

### 3.4 How the queue place displays the distinction

It doesn't — **and that is the decision, not an omission.** There is one list.
A record you put on and a record you queued behind it look identical: a header
naming the record, its rows beneath, in play order. The provenance the display
carries is *which record each run of rows belongs to* (`views/queue.rs:143`),
never *which gesture put it there* — because the moment the surface renders
two classes of entry it has re-created Spotify's two lanes, and the user must
be told which lane wins. baz's answer to "what will play next?" is: **read the
list downward from the dot.** Nothing else is true, so nothing else is shown.

The summary stays MusicBee's remaining-time reading (`3 of 12 · 38:12 left`),
and the bar's ambient continuation (`then 2 albums · 1:58:00 left`,
`player.rs:1680–1710`) keeps stating the tail without costing the wall —
which is what makes a queue place you rarely visit an acceptable home for
queue *editing* (ADR-0022's own bargain).

### 3.5 What this adds to the surfaces, and what it does not

- The record page gains **`Queue album`** beside `Play album` — quiet (no
  accent; the lamp stays spent on playback truth alone), visible,
  pointer-reachable, and the no-modifier route the visible-control rule
  requires shift-click to have. Placement is L8-clean: it reads the selected
  album, so it lives with the album (`07-control-placement.md` §2, L8.1).
- Track rows gain the shift-click append; the pointer route for single-track
  queueing is the same row's reserved-slot control described with the playlist
  panel in §5.6, because the two collectors share one anatomy.
- **The transport gains nothing**, the bar loses nothing, and the engine
  changes not at all.
- **The queue survives a quit.** Decided here because it is the last honest
  gap in "the queue is a record of a choice": a record of a choice that
  evaporates at midnight is a poor record. On exit baz persists the queue's
  paths, the cursor, and the elapsed position; on launch it restores them
  **paused** — never sounding unasked (the same principle as the pull:
  nothing plays until the listener says so). Silent, no prompt: `03` R14's
  finding is that a single-user local player should never ask seven questions
  about queue lifecycle (Feishin needs a discard dialog only because it syncs
  to a server). W2 closes.

---

## 4. Playlists, modeled honestly

Extracted as [ADR-0024](../adr/0024-playlists.md).

### 4.1 What a playlist is, and is not

**A playlist is a named, ordered list of track references, made by a person,
stored in a file that person owns.** Every clause is load-bearing:

- **Named**: it persists under a name the user chose, unlike the queue.
- **Ordered**: the order is the user's utterance. baz never re-sorts it,
  deduplicates it, or "cleans it up".
- **Track references**: paths, not copies and not database ids — the same
  identity the queue, the ledger and the engine already speak.
- **Made by a person**: the product's standing rule. §6 says exactly what this permits a
  generator.
- **A file the person owns**: §4.3.

What a playlist is *not*, against each neighbouring concept:

| | Playlist | Queue | Album |
|---|---|---|---|
| Lifetime | durable, named | transient, one run | permanent, derived |
| Author | the user | the last play gesture | the artist (read from tags) |
| Ordered by | the user, arbitrarily | play order of the choice | disc/track numbers |
| Editable | yes, and edits persist | yes, and edits die with the run | no — fix the tags |
| Stored | a file the user owns | engine state (+ §3.5's resume snapshot) | rows in the cache |
| Plays by | copying into the queue | being the queue | being read into the queue |

The queue/playlist boundary is MPD's, adopted deliberately: a stored playlist
is **loaded into** the queue, and from that instant they are decoupled.
Editing the playlist mid-run does not reach into the sounding queue; editing
the queue does not write back into the file. Anything else makes one of the
two surfaces lie — either the file mutates while you watch a different
surface, or the queue changes under the needle because someone edited a file.
One copy operation, at one visible moment, in one direction.

**The honesty clauses**, which are the owner's word "honestly" made
mechanical:

1. **The playlist a user edits is exactly what plays.** `Play` sends its
   entries, in its order, verbatim. No shuffle-on-play, no dedup, no
   "unavailable items were skipped" silence (§4.5 says what happens instead).
2. **Nothing edits a playlist but the user.** Not playback (no
   most-recently-played reshuffling), not the scanner, not a generator after
   the fact. baz writes a playlist file only as the direct result of a user's
   edit to that playlist.
3. **No smart lists pretending to be lists.** A rule that materializes tracks
   at read time ("all 5-star jazz added this year") is a *query*, and baz's
   queries live on the wall (group keys, search). If saved queries ever ship
   they ship as saved queries — a lens on the library, visibly live — never
   wearing a playlist's name. A playlist is frozen ground truth; the moment
   its contents can change without an edit, clause 1 is unstatable.

### 4.2 The word

The surfaces say **Playlists**. The vinyl register offered *mixtape* (already
in the product's standing rule's gloss) and it was considered and declined for the
surface: the skeuomorphism rule says the record supplies vocabulary where the
vocabulary carries structure — *drop the needle* names a mechanism — but
"mixtape" would rename a concept every listener already owns under its
universal name, which is a legibility tax with no structural payoff (and a
cassette word in a vinyl room besides). *Crate* stays reserved for its
critique meaning — a grouping of records, a future group key — which is a
different thing from an ordered track list and must not share a name with it.

### 4.3 Storage: `.m3u8` files, one per playlist

**Decision: a playlist is an `.m3u8` file in `$XDG_DATA_HOME/baz/playlists/`
(and the platform equivalents), one file per playlist, filename = playlist
name. No database table, no export step, no second copy.**

The argument, in the order it decides:

1. **Sovereignty.** `VISION.md` pillar 3: all app data in open formats; files
   are the source of truth; the database is a cache. A playlist is the single
   most *authored* artefact a music player holds — more the user's than the
   history ledger ADR-0018 already put in a plain file, and for the same
   reasons: a SQLite row is a thing the database may rewrite, VACUUM or lose
   to a corrupt page, and none of `grep`, `git`, a text editor or a backup
   tool can read it. A user should be able to version-control their playlists,
   sync them with Syncthing, and edit one in vim. With files, all three are
   free.
2. **Interop.** M3U is *"a de facto standard"* supported by effectively every
   player — VLC, iTunes, Winamp, foobar2000, MusicBee, MPD
   ([Wikipedia — M3U](https://en.wikipedia.org/wiki/M3U)). baz's audience
   arrives from foobar2000 and MusicBee with folders of these files;
   **reading them is the migration story**, and writing them means a playlist
   made in baz plays in the car. A DB-with-export gets the same interop only
   for users who perform the export, and the exported copy is stale the
   moment it exists — two sources of truth is exactly the dishonesty §4.1
   forbids.
3. **The format's known weaknesses, met head-on rather than discovered:**
   - *No formal spec.* baz writes the strict common subset: `#EXTM3U` header,
     one `#EXTINF:seconds,Artist - Title` line per entry (so the file is
     legible in a text editor and in players with no library), one path per
     line. Reading is liberal: `#EXTM3U`-less files, bare path lists, CRLF,
     BOM, and unknown `#EXT` lines all read fine (unknown directives are
     preserved, not stripped, on rewrite — a file baz didn't fully understand
     is not a file baz truncates).
   - *Encoding.* `.m3u8`'s UTF-8 mandate is the reason it is the chosen
     extension (plain `.m3u`'s locale-dependent encoding is the documented
     ambiguity — same Wikipedia source). Files are written UTF-8. The
     platform's rare non-UTF-8 path is written byte-verbatim with a warning
     comment above it — the file then honestly mirrors the filesystem that
     produced it, baz round-trips it exactly, and no player on earth handles
     that path better. (The ADR-0018 escaping scheme was considered and
     declined: escapes are baz-private, and a private dialect of a public
     format forfeits reason 2.)
   - *Paths.* baz **writes absolute paths** — the same identity the index,
     the queue and the ledger use, and the only kind that stays valid when
     the playlist folder itself is what moves. It **reads** absolute paths,
     `~`, and paths relative to the playlist file's own directory (the
     de facto behaviour everywhere), so imported files work unedited. The
     known cost: a library moved wholesale breaks absolute references — met
     by §4.5's missing-entry surface, not by silent guessing.
4. **Atomicity, not append-only.** Unlike the ledger, a playlist is *meant*
   to be rewritten — by its owner. Each user edit writes whole-file to a
   temp sibling and renames over; a crash mid-edit costs at most the edit,
   never the playlist. External edits are honoured: the file's mtime is
   checked on read, and a playlist changed under baz is re-read, not
   clobbered — last writer wins per file, which for a human-scale artefact
   edited in one place at a time is the honest rule.
5. **What the files decision costs, accepted:** name-is-filename means names
   are sanitized to what the filesystem allows (the page shows the name; the
   file carries it); ten-thousand-entry lists re-parse on open (bounded,
   milliseconds, and cached against mtime like every other cache in baz);
   there is no transactional multi-playlist operation (none is designed).

The playlists *folder* is listed in Settings → Library beside the roots, with
an "open folder" affordance — the same sovereignty surface, so the user learns
where their artefacts live the same way they learn where their music does.

### 4.4 Membership beyond the library

An entry is a path, and a path need not be under a library root — an imported
`.m3u8` may reference anything. Decision: **it plays anyway.** The engine
takes paths and has never cared where they came from (`ADR-0014`
Consequences: a queue of paths that do not exist is a queue of tracks that
fail one by one — and one that does exist plays). Display metadata comes from
the index when the path is indexed and from the filename when it is not,
marked as such. Refusing to play a file the user explicitly listed, because a
cache has no row for it, would invert the cache/source-of-truth order.

### 4.5 When a referenced file moves or vanishes

The library-roots work is the precedent (ADR-0022 *Several music folders*):
rows that can no longer be accounted for are **counted and surfaced, never
silently pruned** (`Library::unrooted_tracks`, and the gates that forbid a
scan from destroying what it cannot prove gone). The same posture, applied to
playlists:

- **The entry stays in the file.** baz never rewrites a playlist because a
  path stopped resolving — the file may be about to come back (a NAS mount, a
  USB disk), and an entry deleted by helpfulness is a deletion the user
  never asked for. This is ADR-0010's lesson (*a scan that produced no entry
  prunes nothing*) at playlist scale.
- **The entry is shown, marked missing**: its row renders from the path's
  stem, dimmed, unplayable, with the path itself one glance away. Not
  hidden — a 40-track playlist showing 38 rows with no explanation is the
  "nothing happened" answer ADR-0022 §4 called worse than "they went".
- **Play sends the playable subset**, in order, and says so: the playlist
  page's summary reads `38 of 40 · 2 missing`, so what the queue holds and
  what the file holds differ *legibly*. Sending known-missing paths to fail
  one by one was considered — it is arguably more literal — and declined
  because each failure costs a session spin-up and an error the listener can
  do nothing about mid-run; the count on the page states the same truth
  before the music starts.
- **A moved file that the scanner re-homed** (same file, new path under a
  root) is still a broken reference — the file's path is the identity, and
  guessing "this is probably that" by tags or size is inventing a fact,
  which the roots ADR refused for backfills (*"knowable and correct"* was
  its bar). What ships instead is a **repair surface, offered not
  automatic**: the playlist page can propose candidates for a missing entry
  (same filename under a current root), and the user confirms per entry or
  per playlist. The proposal reads the index; the confirmation writes the
  file; nothing happens unbidden.

### 4.6 Duplicates

**Allowed, unmarked.** A queue may legitimately repeat a file (ADR-0014 §3
built its reconciliation rule around exactly this), and a mixtape that plays
its theme twice is its maker's business. The editor's position numbers make a
duplicate visible as two rows; a nanny-mark would be baz having an opinion
about the user's list, which clause 2 of §4.1 forbids. Adding a track already
present simply adds it again — the gesture did what it said.

---

## 5. Creating and editing — the interaction inventory, and the panel

### 5.1 Where playlists live in the places model

Two surfaces, one new place and one new kind:

- **`Place::Playlist(name)`** — a playlist's page, the sibling of
  `Place::Album(id)` (`place.rs:57–76` grows one member). Same anatomy as the
  queue place: header strip, summary, rows at `LIST_MEASURE`, one scroll —
  three list surfaces, one composition, zero new layout vocabulary.
- **The playlist panel** — the collapsible side panel the owner has blessed,
  designed in §5.5, which is also the answer to "a way to see playlists".

There is deliberately **no Playlists place** (no full-window list-of-lists):
the panel *is* the index of playlists, and a place whose whole content is
twelve names would be the settings-panel emptiness the audit measured
(`01` §1.3(e)) at window scale. If a user someday has two hundred playlists,
the panel scrolls; the day that fails is the day a place earns proposing.

### 5.2 The inventory

Every action, its gestures, and its budget cost. **P** marks the pointer
route the visible-control rule requires; where a drag exists it is always an
*additional* route, never the only one (drag-only would violate
the product's standing rule, and §5.6 says why the drag also cannot ship first).

| Action | Pointer route (P) | Other routes | Gestures |
|---|---|---|---|
| **See playlists** | the panel's door (labelled `Playlists`, Library strip) | — | 1 |
| **Create, empty** | panel → `New playlist` row → type a name, Enter/✓ | — | 2 |
| **Create from a record** | record page → `Add to playlist` → `New playlist` | drag sleeve → `New playlist` row | 2 |
| **Create from the queue** | queue place → `Save as playlist` → name it | — | 2 |
| **Add a record** | record page → `Add to playlist` → pick the playlist | drag sleeve → playlist row; open-target `+` (§5.6) | **2** |
| **Add a track** | track row's `+` slot → pick the playlist | drag row → playlist row; open-target `+` | **2** |
| **Play a playlist** | playlist page → `Play` | — | 2 from the panel |
| **Play from track *n*** | playlist page → click row *n* | — | 2 |
| **Queue a playlist** | playlist page → `Queue` | — | 2 |
| **Rename** | playlist page header → rename control → type | edit the filename on disk | 2 |
| **Delete** | playlist page → `Delete` → confirm | delete the file on disk | 2 |
| **Remove an entry** | row ✕ (reserved slot, hover-revealed — the queue row's exact anatomy, `views/queue.rs:328–361`) | — | 1 |
| **Reorder within** | row ▲▼ steppers (reserved slot, `STEPPER_HIT` 24 — the settings steppers' size) | drag-to-reorder, when the widget exists (§5.6) | 1 per step |
| **Repair a missing entry** | the missing row's `Locate…` → confirm a candidate | edit the file | 2 |

Notes the table compresses:

- **`Save as playlist` on the queue place** is prior art's W19 (*save the
  transient* — Roon, TIDAL, YouTube Music all ship it, `03` §1.1) and it is
  the cheapest good creation flow baz can offer: the queue is already an
  authored, ordered list; naming it freezes tonight's run into an artefact.
  It writes a new file and does nothing else — the queue does not become
  "linked" to the playlist (§4.1's decoupling).
- **Delete confirms in the roots ADR's voice**: *"Delete '{name}'? The file
  goes; your music stays."* Same shape as *"Forget N tracks? The files stay
  on disk"* (ADR-0022 §4) — every destructive confirmation in baz states what
  survives.
- **Rename and delete live on the playlist's page**, not the panel: the panel
  is for reaching and receiving (§5.5); acts that destroy or redefine a thing
  belong on the thing, where its full contents are visible at the moment of
  the decision.
- **Reorder's stepper route is admittedly homely.** It exists because the
  visible-control rule demands a pointer route that is not a drag, and it is
  the same 24 px stepper the settings already draw. The drag, when the widget
  lands, becomes the *pleasant* route; the steppers remain the guaranteed
  one. (Same reserved-slot discipline as the ✕: the slots are reserved
  whether shown, so rows never shift under the pointer.)

### 5.3 The playlist page

The queue place's composition, with the differences a durable artefact earns:

- Header: the name (hero type, the album page's scale), `N tracks · 1:12:40`,
  and — when entries are missing — `38 of 40 · 2 missing` (§4.5).
- Controls: `Play` (the page's one commitment, `Play album`'s styling),
  `Queue` (append to the current run, §3.3), `Rename`, `Delete`.
- Rows: position number, title over artist, duration, the reserved ✕ and ▲▼
  slots. Consecutive tracks from one record get the record's group header,
  exactly as the queue place draws provenance (`views/queue.rs:143`) — the
  playlist stays a track list, but the surface still says where things came
  from.
- Clicking a row: `SetQueue{playlist's playable entries}` + `JumpTo{row}` —
  the same `play_from` decision the album page spends (`player.rs:1782`),
  generalized: if the engine already holds exactly this playlist, it is a
  jump; otherwise the playlist is put on and the needle drops on the clicked
  row. One rule for every list surface in the product.
- The playing row carries the lamp dot in the number column **when the queue
  is exactly this playlist** — `playing_row_in`'s existing honesty rule
  (`player.rs:1756–1761`): a surface listing something other than what the
  engine holds marks nothing.

### 5.4 Provenance in the file

A baz-written playlist opens with comment lines (`#` is the format's comment
prefix; players that don't understand them ignore them — same source as §4.3):

```
#EXTM3U
# made with baz on 2026-08-09
```

and, for §6's generated playlists, one more line naming the generator and its
input. Comments are inert, legible, grep-able, and survive round-trips. They
are provenance, not behaviour: nothing in baz reads them to decide anything.

### 5.5 The panel — justified against the model that deleted its ancestors

a standing rule of the product — *"baz has no side surfaces. No sidebar, no inspector, no
rail, no drawer, no popover, no float."* Rejected twice by the owner before it
was written down. A playlist panel is a side surface, and the same owner has
now blessed one: *"I think a sidebar/collapsible panel for playlist is okay…
it should be really easy to drag a song into a playlist."* Under the ledger's
own editing rule that entry can only be amended by an ADR that beats its
argument — so here is its argument, engaged:

The rail was deleted for five findings (`01` §1.3, ADR-0022's table): three
unrelated tenants in one slot; a dismissal model needing a paragraph; the
wrong tenant paying the width; a reflow that broke the double-click; a
paragraph of arbitration state. **The playlist panel has none of these by
construction, and one thing no place can have:**

1. **One tenant, forever.** The panel shows playlists. It is not a slot;
   nothing else may move in, and ADR-0024 says so in the amended refusal.
   The junk-drawer disease was "the only non-shelf surface, so every new idea
   becomes a tenant of it" — that failure mode requires vacancy, and this
   panel has none.
2. **Summoned for a task, closed at rest.** It opens by a labelled door
   (`Playlists`, in the Library strip — L8.4: a door goes where the hand is,
   and playlists are about the collection) and by nothing else. At rest the
   wall keeps 100 %. The rail's width tax was *resident*; a surface open only
   during the minutes you are collecting taxes nothing the rest of the year.
   Dismissal is the places model's own pair — `Esc`, or the door again — not
   a paragraph.
3. **It exists for the one thing places cannot do: simultaneity.**
   ADR-0022's "what is unreachable" list is honest that a place model cannot
   show two things at once. Every act in §5.2 except add survives that fine —
   but **collecting is inherently two-surface work**: the source (wall, page,
   queue) and the destination (the playlist) must both be on screen for a
   drag to mean anything, and for a session of many adds not to cost a
   round-trip per track. The panel is not the album column returning; the
   album column *displayed* a thing you pointed at (a reading, which a place
   does better — ADR-0022 proved it), where the panel *receives* things
   (a target, which a place cannot be, because navigating to it empties your
   hands).
4. **It overlays; it does not re-hang.** ADR-0022 bought "no press anywhere
   re-hangs the collection" and deleted the grid-hold machinery that made
   reflow survivable. The panel therefore floats over the wall's right edge
   (width ≈ `PANEL_W`'s old 340, the one dimension of the rail nobody
   faulted), no scrim (refused), wheel passing through beneath it — the
   ADR-0016 popover's verified iced mechanics (`stack` + `opaque` +
   `mouse_area`), which were deleted for want of a subject, not because they
   failed. The bar stays untouched below it.
5. **It is present in Library, Album and Queue places** — the collecting
   sources — and absent in Settings. Its open/closed state survives place
   changes while it has a job (a drag begun on the wall may end on a panel
   row after a place change never happens — the drag stays inside one
   place), so in practice: open it, collect from wherever you navigate,
   close it.

What the panel shows, top to bottom: `New playlist` (a row that becomes a
name field on press — the roots field's anatomy), then every playlist: name,
`N · 42:10`, each row **two controls** — the name (a door to
`Place::Playlist`) and the receive-target described next. Nothing else. No
per-row delete (destruction lives on the page, §5.2), no reordering of the
panel itself (alphabetical; the playlists folder is the truth).

This is also where *"possibly a section"* lands: the panel **is** the
playlists section — of the product, not of Settings. Settings gets only the
folder line (§4.3), because a playlist is content, not a standing decision.

### 5.6 "Really easy to drag a song in" — what that actually requires

Three layers, cheapest first, because the drag itself is the most expensive
gesture in this plan:

**Layer 1 — the two-press add (ships first).** The record page's
`Add to playlist` control (visible, under `Play album`/`Queue album`) slides
the panel open in *pick* mode: press a playlist row (or `New playlist`) and
the record is appended. Two gestures, no modifier, no drag — the budget met
with the toolkit baz has today. A track row's reserved-slot `+` (the queue
✕'s exact anatomy: `STEPPER_HIT`, slot always reserved, control on hover)
does the same for one track — and because hover-revealed controls need a
second route (the product's standing rule; ADR-0022 held the queue ✕ to this), the
track's second route is layer 2's open target, and until layer 2 ships the
`+` is drawn **at rest, not on hover**, on every track row of a page the
panel is open beside. A quiet mark that appears only while the user is
collecting is not permanent chrome; it is the task's own furniture.

**Layer 2 — the open playlist (the crate on the counter).** Press the
receive-target on a panel row and that playlist becomes **open for adding**
— the row visibly armed (a surface step and a hairline; never the accent,
which is playback truth). While one is open: every tile's wall label gains a
quiet `+` in its first line (the stack's numeral-chip position — nothing is
ever drawn on a sleeve), every track row's `+` slot fills at rest, and one
press adds. Press the armed row again, or `Esc`, and the wall is calm again.
This is the record-shop gesture — put the crate on the counter, pull records
into it — and it makes a twenty-track collecting session cost one press per
track instead of two, which is the difference between a feature and a chore.
The state is legible (the panel is open, the armed row is lit, the `+` marks
are on screen), reversible in one press, and touches no engine state.

**Layer 3 — the drag itself.** iced 0.13 has no pointer capture: a press
that leaves its widget is lost, which is why ADR-0016 deferred queue
drag-reorder and nothing since has changed the fact. The drag therefore
needs a hand-built widget on the `groove.rs` precedent ("we need pointer
geometry, so we wrote the widget") — this time spanning surfaces: press on a
row or tile, a ghost chip following the pointer (never a dragged sleeve —
artwork stays on the wall), panel rows lighting as legal targets while the
drag is in flight, drop to add, `Esc` to abandon. The same widget is the
missing piece for queue and playlist **reorder**, so it is one investment
paying three surfaces. It ships after layers 1–2 because "really easy" must
not mean "waiting on the hardest widget in the plan" — and because when it
lands, every drop target it needs (the panel's rows) is already on screen
doing its no-drag job.

### 5.7 Keyboard

Per L8.7, frequency chooses the layer and the screen home chooses the key:
the panel's door gets `Ctrl+P` (a door, so modified — `Q`'s own argument
after type-anywhere took the bare letters); everything else in this section
is pointer-first and earns no key at v1. No action here is keyboard-only,
and no key reaches an action without a control — the placement table
(`07` §6) grows rows, it does not grow exceptions.

---

## 6. Generated playlists — what the model guarantees any generator

The owner's stated destination: *"we want to be able to generate a playlist
based on user sentiment."* This document does not design that feature — the
analysis pipeline is v0.3's (`VISION.md` pillar 4), and the steering surface
is its own study. What it designs is the ground the feature lands on, so that
when a generator exists it cannot help but be honest. Five guarantees, owed
by the model to *any* generator — sentiment-steered, similarity-walked, or a
shell script the user wrote:

1. **A generated playlist is an ordinary playlist.** An `.m3u8` file in the
   same folder, editable, renamable, deletable, playable — indistinguishable
   in rights from one assembled by hand. The moment it plays, edits, or
   persists differently, it is a second species, and second species are how
   Spotify ended up with two queues.
2. **Generation is an act, not a condition.** A person asks; a file appears;
   the person is looking at it. There is no standing rule that regenerates,
   refreshes, or "keeps it fresh" — a playlist that changes because Tuesday
   came is self-mutating, and §4.1 clause 3 forbids it. *Regenerate* is a
   press on the artefact, and it writes a new file or visibly replaces this
   one at the user's word.
3. **Provenance is recorded, and it is inert.** The file says what made it
   (`# made by baz · pull-list · "quiet, late" · 2026-08-09` — §5.4); the
   page may show the note in the pull-note's exact voice
   (`views/album.rs:244–256`: two facts, no third, never a score, never a
   "because you liked"). Nothing reads the note to decide behaviour. History
   records; it never performs.
4. **Nothing plays until the person says so.** The generator's output arrives
   as a page to read, exactly as the pull arrives as a record's page with the
   ordinary `Play album` untouched (`app.rs:1584–1588`: *"No command is
   sent… accepting the suggestion is pressing Play album"*). A generator that
   starts sound is a radio with homework.
5. **The generator consumes only what is already the user's.** The ledger
   (ADR-0018), the library, local analysis when it exists. No cloud, no
   account (pillar 3) — and no *hidden pool*: the candidate set a generator
   draws from is statable in a sentence on the artefact ("from records on
   the wall", "from the whole library"), the same legibility rule shuffle's
   visible pool already obeys.

And one amendment owed to the ledger: the product's standing rule reads *"No
auto-generated playlists. Every crate and every mixtape is made by a
person."* The entry's argument is against **auto** — playlists that generate
themselves, unbidden, as engagement surfaces. A person explicitly asking for
a generated list, receiving an editable file, is not that; but the entry as
worded could be read to forbid it. ADR-0024 amends the gloss under the
ledger's editing rule: *made by a person* includes *asked for by a person and
owned by them thereafter*; what stays refused is generation without a
request, mutation without an edit, and any pool the person cannot see.

---

## 7. The friction budget, checked

**Amended interaction budget: explicit commands remain one press; ordinary
content activation is one double-click (two clicks); add-to-playlist remains
two gestures or fewer.** Every flow, counted from where the listener already
is:

| Flow | Presses | Budget | Notes |
|---|---|---|---|
| Play a record, from its page | 1 | ✓ | `Play album` |
| Play a record, from the wall | 2 clicks (one double-click) | ✓ | First click selects; second activates without navigating or moving the wall. |
| Drop the needle on track *n* | 2 clicks (one double-click) from the page | ✓ | First click selects; second spends §3.1's unchanged path. |
| Jump within the sounding record | 2 clicks on the row, or 1 on the needle | ✓ | The seek needle remains an explicit direct control. |
| `Enter` on a query | 1 | ✓ | top match plays |
| Shuffle the wall | 1 | ✓ | |
| The pull → sound | 2 (pull, `Play album`) | ✓* | deliberate: the second press *is* the consent (nothing plays until asked) |
| Queue a record for later | 1 from its page | ✓ | `Queue album`, §3.3 |
| Play a playlist | 2 clicks (one double-click) from its collection tile; 2 from the panel (`Open` → `Play`) | ✓ | The tile uses the shared content grammar; the page's labelled `Play` remains direct. |
| Add a record to a playlist | 2 (`Add to playlist` → pick) | ✓ | §5.6 layer 1 |
| Add a track to a playlist | 2 (row `+` → pick) | ✓ | §5.6 layer 1 |
| Add *n* tracks, collecting | 2 setup + 1 each | ✓ amortized | §5.6 layer 2 |
| Remove / reorder an entry | 1 / 1 per step | ✓ | §5.2 |
| Save the queue as a playlist | 2 | ✓ | §5.2 |
| Resume yesterday's run | 1 (`Play`) | ✓ | §3.5: restored paused, one press to sound |

The older one-click budget is deliberately superseded for ordinary content by
the owner's selection-first rule. Direct labelled commands still meet it; no
flow silently exceeds the amended budget.

---

## 8. What this costs, and what it does not touch

- **The engine: nothing.** Every gesture in this document compiles to
  `SetQueue`, `UpdateQueue`, `Play` and `JumpTo` as they ship today. No new
  command, no new event, no schema change. (The queue-resume snapshot of
  §3.5 is front-end state — a small file beside the config, written on exit,
  sent as an ordinary `SetQueue` + `Seek` on launch.)
- **`baz-core` gains one pure module**: `playlist` — read/write/parse
  `.m3u8`, the missing-entry accounting, the repair candidates (an index
  query). Pure, iced-free, exhaustively testable, beside `history` for the
  same reason `history` lives there: a second front end must not reimplement
  the format. Per ADR-0017 §5's amended claim, this is named honestly as a
  product change in core, not a redesign.
- **`crates/baz` gains**: `Place::Playlist` (one enum member and its walk),
  the panel (state + view), `views/playlist.rs` (the queue place's
  composition reused), the `Queue album` / `Add to playlist` controls, the
  open-target state, and — later, once — the pointer-capture drag widget
  that serves playlist add, playlist reorder and queue reorder alike.
- **Order**: (1) `baz_core::playlist` + the playlist page reading files
  dropped into the folder by hand — playlists exist before any chrome does;
  (2) the panel and layer-1 adds; (3) `Queue album` / the stack; (4) queue
  resume; (5) `Save as playlist`; (6) the open target; (7) the drag widget.
  Each step ships whole and none waits on the one after it.
- **the product's standing rules changes twice, by ADR-0024, under its own editing rule**:
  the side-surfaces entry gains the panel as its named, argued exception
  (§5.5), and the auto-generated-playlists entry has its gloss tightened
  (§6). Neither entry is deleted; both keep their teeth.

---

## 9. Summary

The playback model baz has is the right one; what it lacked was a statement.
**The queue is one list with a cursor, and it is a record of a choice** — a
click on a track puts the record on and drops the needle there; play means
now; later has its own gesture at the record boundary; the end is silence,
because everything that could continue the evening is a thing the listener
can ask for before the silence arrives. A playlist is the durable species of
the same idea: an ordered list a person made, in a file that person owns,
that plays exactly as written and changes only when its owner changes it. The
panel exists because collecting needs two surfaces at once — the one job the
places model cannot do — and it is one tenant, summoned, floating, and gone
at rest. And a sentiment generator, whenever it comes, inherits all of it:
its output is just a playlist, which is the entire point.
