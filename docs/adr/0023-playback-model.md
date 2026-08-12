# ADR-0023: The playback model — one list with a cursor, and the queue as a record of a choice

> **Start-confirmation amendment (2026-08-12).** A deliberate album Play now
> carries the listener to Now Playing, but the place follows playback truth,
> not command optimism. Explicit `Play album`, a search album's Play, Enter's
> album answer and the shared album double-click all call one
> `start_and_show`: an accepted `SetQueue` plus `Play` arms the requested run,
> and only a `TrackStarted` whose path belongs to that run spends the
> destination. An empty queue never arms it; refused commands and engine
> closure stay put; a wholly failed run reaches `QueueEnded` and cancels it.
> `Resume` remains immediate because it names the validated run the engine is
> already holding, rather than claiming a fresh run has begun.

> **Interaction amendment (2026-08-12).** A playable content row no longer
> spends its playback command on an ordinary single click. The first click
> selects/highlights; a double click activates through the exact existing
> `play_from`/`JumpTo` path described below. Album, playlist and implicit-list
> tiles use the same rule. Labelled Play controls and menu verbs remain direct.
> This changes only how intent reaches the model: the list, cursor, queue and
> engine protocol are untouched. ADR-0022's 2026-08-12 amendment owns the
> shared selection state and timing.

> **Amendment (2026-08-10) — shuffle is a property of the player, and it is a
> property of the *walk* rather than of the list.**
>
> The owner, twice in one day. First: *"can you make shuffle a property of the
> player i.e. toggle on/off."* Then, on seeing what that shipped as: *"I think
> shuffle as a concept is more about going to an unknown next track rather than
> actually mutating the track list if that makes sense."*
>
> **This is one decision and is written as one.** The first version of the
> amendment made shuffle a *permutation*: turning it on re-ordered the queue
> ahead of the cursor, turning it off restored a retained `Vec<PathBuf>`, and
> most of the text was rules about when that retained order was still valid. The
> owner's second sentence says that was the wrong object. What follows replaces
> it rather than correcting it, because a record that reads as a decision plus a
> correction teaches the correction instead of the decision.
>
> **1. The model.** A run is a list and a cursor (§1). **Shuffle changes how the
> cursor moves; it never changes the list.** The queue keeps the order the
> gesture that built it laid out — always, in both positions of the control, and
> whatever else happens to it — and what shuffle decides is which entry the
> cursor goes to next. `baz_core::traversal` holds the type and the argument.
>
> **2. The selection rule, in one sentence.** *With shuffle on a run plays a
> **bag**: one deterministic shuffled pass over the run's entries, in which no
> entry repeats until every entry has played, and when the bag is spent the run
> ends.*
>
> A bag rather than a uniform draw, and the difference is not fussiness. A
> uniform draw can play the same track twice running and can leave a track
> unheard across a whole album, and a listener has no way to tell an unlucky run
> from a broken one. A bag is what the word means to people: everything gets
> played, nothing gets played twice, the order is the surprise. Concretely a bag
> **is** a permutation of the run's positions, computed once from a seed — which
> is not an implementation convenience but the thing §4 depends on.
>
> **3. Where the decision lives: in the engine.** This is the one place this
> amendment changes something the original §"Consequences" was proud of, and it
> is stated rather than slipped in. The engine learns **one** standing property,
> the order it walks its queue in (`Command::SetTraversal`,
> `Event::TraversalChanged`). It gains no repeat flag, no continuation policy and
> nothing that refills; a bag is finite and §5's silence is untouched.
>
> **Because baz is gapless.** Gapless means the engine knows the next track
> *before the current one ends* — it decodes one ahead on a prefetch thread and
> splices into the same ring (ADR-0004, ADR-0009). A shuffle that chose its next
> track when the current one ended would be choosing after the moment the
> decision was needed. And a front end cannot supply the answer either: the only
> way to say "this plays next" over this protocol is to send a queue, and
> ADR-0014's `UpdateQueue` costs the following boundary its sample-accurate
> splice. One edit, one boundary is a fair price for an edit; a mode that charged
> it at **every** boundary of a shuffled run is not. The acceptance test is
> `a_shuffled_run_is_gapless_and_bit_identical`, which plays a two-track queue
> under a reversing traversal and compares the delivered stream sample for sample
> against the reference decodes concatenated in the bag's order.
>
> **4. baz says what is next.** The order is decided in advance, so baz knows it,
> and this product does not conceal what it knows: the run column marks the row
> that plays next and dims the entries the pass is already past, and the bar's
> continuation counts the bag's remainder rather than the list's tail. *Unknown*
> in "an unknown next track" describes how the choice is made, not something
> withheld from the listener. This is why `Traversal::play_order` is a pure
> public function: the front end computes the identical pass from the identical
> seed, so the row marked on screen is the row that plays.
>
> **5. When the bag empties.** The run ends — `QueueEnded`, exactly as an
> unshuffled run ends at its last track. Nothing refills and nothing re-rolls; a
> fresh pass comes from a fresh gesture, which is where every other list in baz
> comes from. §5's silence is unchanged, and there is nowhere in the engine for a
> refill to live.
>
> **6. What a manual jump does to it.** It moves the cursor **within** the bag
> and does not re-roll it: jump to a track and the pass continues from that
> track's place in the order, so entries earlier in the bag are passed over and
> entries later in it still come. Re-rolling on every jump would mean the order
> on screen changed each time the listener touched a row, which contradicts §4.
> `Next` and `Previous` step along the bag for the same reason — a skip must land
> where the run was going, or the interface lied about where that was.
>
> **7. What a new run does.** A fresh seed, per run. The same seed over a
> re-played record would be the same shuffle twice, which is the one thing about
> a shuffle a listener notices immediately. The **mode** is persisted in
> `config.toml`; the **seed** is not, so two launches with shuffle on are two
> different passes.
>
> **8. Turning the mode on or off never stops the music.** The sounding track is
> delivered to its end and the run continues on the new plan after it — that one
> boundary is a fresh decode rather than a splice, which is ADR-0014's existing
> bargain at its existing price. Nothing behind the cursor is disturbed and
> nothing in the list moves at all.
>
> **9. What was deleted, and why the deletion is the point.** Turning shuffle off
> is now trivial because nothing was ever changed. Gone with the permutation:
> the retained `Vec<PathBuf>` and its two invalidation rules; the restore walk
> and the three consequences it had to define (a deleted row staying deleted, an
> appended row staying appended, a repeated file being put back twice); the "a
> run restored from a snapshot has no retained order" case and the message it
> needed; the hand-reorder rule that dropped the retained order; the hoist that
> made a track click's clicked row lead a permuted body; and the whole of the
> front end's `shuffle` module. **Every one of those was a rule about keeping two
> orders in step.** There is one order now, and it is the run's.
>
> **10. §"Consequences" is corrected.** It recorded *"shuffle or repeat in the
> engine (unchanged from ADR-0014's deferral — both remain front-end expressions
> over `SetQueue`/`UpdateQueue`)"*. **That is no longer true of shuffle**, and §3
> says what replaced it and why the alternative was a gap at every boundary.
> Repeat is unchanged: still deferred, still nothing in the engine.

> **Amendment (2026-08-09), from
> [`docs/design/09-implicit-playlists.md`](../design/09-implicit-playlists.md)**
> — the implicit-playlist study, commissioned on the owner's *"we are
> thinking there are implicit playlists everywhere."* Four changes to this
> record, none touching the engine. **Status: all four items accepted and
> shipped, whole** (2026-08-09 — items 1–2 as doc 09 §13 steps 1–2, the
> picker's Queue row and playing provenance; item 3 as step 3, the songs
> section; item 4 as steps 5–6, `Play all` over the virtualized queue
> place, with shuffle's contract already shipped behaviour; item 1's
> shift-click accelerator as step 7 and its context-menu accelerator as
> step 4, the mirror layer):
>
> 1. **§3's dedicated `Queue album` control is withdrawn before being
>    built** (it never shipped). The queue-append lives in the unified
>    transfer gesture instead — `Add to…` / a row's `+` / the context menu
>    opens the picker, whose **first row is the Queue** — because a
>    dedicated control beside a picker containing the queue would be two
>    controls sending one message, which L8.6 forbids. Shift-click and the
>    menu's `Queue` item are the one-press accelerators, resolving to the
>    picker's Queue row as their on-screen control (09 §8.1). `Queue
>    next`'s deferral and its fixed album-boundary semantics are unchanged.
>    *Shift-click shipped as step 7 (2026-08-09)*: shift held, the press
>    that would open a record's page appends it to the run instead —
>    `UpdateQueue`, nothing sounding unasked — resolved against the
>    hand-kept modifier state because iced 0.13 reports a button's press
>    without one. *The menu's `Queue` item shipped as step 4 (2026-08-09,
>    the mirror layer)*: its presses are the row's `+` then the picker's
>    Queue row, made for you — two messages visible controls also send,
>    which is the accelerator resolving to its on-screen control by
>    construction (`every_menu_item_is_a_press_some_control_also_makes`).
> 2. **Playing provenance is defined** (09 §6), completing §1's "the
>    context is recorded, not kept live": a queue reified from a named
>    playlist carries the source's name on the request-side record; it
>    survives edits and `QueueEnded`, is replaced only by a replacing
>    `SetQueue`, and powers the Queue place's summary and the *"add to the
>    current playlist"* verb. Origin, never a live link — Plexamp's
>    `playQueueSourceURI`, adopted by name.
> 3. **A song search result's press is a needle-drop** (09 §5): §2's rule
>    extended verbatim to the new `Songs` section — the song's record is
>    queued whole, the cursor on the song, and `Enter` plays the
>    top-ranked song the same way (superseding the album-level answer).
>    *Accepted (2026-08-09), shipped as doc 09 §13 step 3*: the section
>    renders over the filtered wall, its press and `Enter` both resolve
>    through the record page's `play_track`/`play_from` path, and the
>    captures are at `docs/design/impl/songs-search/`.
> 4. **Play-everything and shuffle are specified as this model's cases**
>    (09 §7): `Play all` reifies the wall — every visible record, in the
>    wall's arrangement order — into the queue in one press; a shuffle
>    draw *is* an implicit playlist (readable, editable, saveable,
>    ending); §5's silence is unchanged by both.
>    *Accepted (2026-08-09), shipped as doc 09 §13 steps 5–6*: `Play all`
>    leads the Library strip's acts, its scope exactly the wall's visible
>    set, no pool claimed and no confirmation at any scale — §7.1's
>    virtualization gate is met by the queue place's spacer window
>    (`queue_window`, pinned bounded at 40 000 rows) — and the draw's
>    "editable like any queue" criterion is completed by queue-place
>    reorder (step 5). Captures at `docs/design/impl/queue-parity/`.

**Status**: proposed (2026-08-09) · extracts the decisions of
[`docs/design/08-playback-and-playlists.md`](../design/08-playback-and-playlists.md)
§1–§3 · changes no engine command and no protocol message — every gesture here
compiles to ADR-0014's existing vocabulary · confirms and names semantics that
shipped piecemeal across ADR-0014, ADR-0016 and ADR-0022 · answers the owner's
question *"if I just click a track from any album… is it enqueuing that album
and starting from that song?"* on the record · sibling of
[ADR-0024](0024-playlists.md)

## Context

The engine's queue is a list of paths and a cursor
(`crates/baz-core/src/engine.rs:1180–1182`) with no shuffle flag, no repeat
flag and no continuation policy, edited whole (`UpdateQueue`) and addressed
absolutely (`JumpTo`). The front end already spends it: `Play album` sends the
record whole (`app.rs:1403–1428`), a track click sends the record whole and
jumps (`app.rs:1449–1501` via `PlayerState::play_from`,
`player.rs:1782–1795`), and the queue place draws one list with the playing
row dotted. All of that behaviour exists; none of it was ever stated as *the
model*, and the owner's question is evidence that an unstated model is a model
nobody can trust. The design study surveyed how sixteen products model the
same ground (`docs/design/08` §2) and found three families; this ADR commits
baz to one of them, names the semantics, and adds the two gestures the model
was missing.

## Decision

### 1. The model, named

> **baz's queue is one list with a cursor, and the list is a record of a
> choice.** The playing context — this record, this playlist, this draw — is
> reified into the queue at the moment of the gesture and then discarded.
> Nothing writes into the queue but a user gesture; no source, no policy and
> no service holds a pen.

The one-list-with-a-cursor half is MusicBee's model, already adopted by name
in ADR-0016; the reification half is what distinguishes baz from every
context-plus-queue product (Apple, Spotify, Plexamp): there is no live
context object that keeps acting after the gesture. Provenance is *recorded*
(the queue place heads each record's run of rows with its name) but never
*consulted*.

### 2. A track click drops the needle

Clicking a track on a record's page **enqueues that record — the selected
edition, whole, in order — and starts playback at that track.** The tracks
before the clicked one sit behind the cursor: reachable by `Previous` and by
their rows, drawn faint as history in the queue place, never sounding
unasked. The gesture's name is the record's own vocabulary: **drop the
needle**.

Rejected alternatives, recorded: *play only this track* (three minutes and a
dead stop; the album is the unit), *discard the earlier tracks* (the queue
would lie about what record is on), *start from track 1* (plays something
the listener did not point at — the same test ADR-0014 applied to
out-of-range jumps).

**Within the sounding record, the same click is a jump, never a re-queue** —
`play_from`'s `holds_exactly` branch, promoted from implementation detail to
rule: moving around inside the record you are listening to never resets the
run and never disturbs a shuffle's marks.

### 3. Play means now; a second gesture means later

A play gesture aimed at a different record **replaces the queue** (`SetQueue`
— the run ends because the listener superseded it). This is load-bearing:
the moment "play" sometimes appends, the product has rebuilt fooyin's
defining failure (double-click appends and nothing plays) or Spotify's
two-lane interleave (`docs/design/03-interface-prior-art.md` §3 W1,
§4.4.5). One gesture, one meaning, everywhere.

"Hear this later" is its own gesture — the stack ADR-0017 step 13 adopted —
with these semantics:

- **Record granularity by default.** `Queue album` (a visible control on the
  record's page, beside `Play album`, no accent) and shift-click a sleeve
  append the whole record to the end of the queue as its own headed group.
  Albums are listed as albums, never flattened.
- **A queued track is its own one-row group**, headed by its record's name.
  It does not smuggle its album in.
- **Append-only for now.** `Queue next` is deferred, and when it ships it
  means *after the sounding record* — at the album boundary — recorded here
  so the insert semantics are decided before the control exists rather than
  discovered in a complaint stream (Plexamp's, per the study).
- Mechanically: `UpdateQueue` with rows appended. The music keeps playing
  (ADR-0014's guarantee), and the engine learns nothing new.

### 4. The queue displays no gesture-provenance, deliberately

One list. A record you put on and a record you queued look identical: header,
rows, play order. The display's provenance is *which record a row belongs
to*, never *which gesture added it* — two visible classes of entry is
Spotify's two lanes, and the reader must then be told which lane wins. The
answer to "what plays next" is: read the list downward from the dot. Nothing
else is true, so nothing else is shown. The summary stays the remaining-time
reading (`3 of 12 · 38:12 left`); the bar's continuation lane stays the free
ambient reading of the tail.

### 5. The end is silence, re-tested and reaffirmed

the product's entry stands: the queue empties and there is silence. The
strongest counter-evidence — Longplay 1.0 shipped a dead stop and reversed it
within one major version — does not bind, because Longplay had no way to ask
for more *before* the silence and baz now has three: `Queue album` standing
as an answer given in advance, `Shuffle` from a visible pool, and the
playlist (ADR-0024). The refusal was never "you may not continue"; it is
"the software will not decide to continue for you." The queue place's empty
state keeps saying so in words.

### 6. The queue survives a quit

On exit the front end persists the queue's paths, the cursor and the elapsed
position; on launch it restores them **paused** — nothing sounds unasked, and
one press resumes. Silent, no prompt (the study's R14: a single-user local
player never needs a queue-lifecycle dialog). Front-end state beside the
config; on launch it is an ordinary `SetQueue` + `Seek`, so the engine is
untouched. This closes prior art's W2, the one band-C workflow whose absence
is felt every launch.

## Consequences

- The owner's question has a one-sentence answer that is also the
  implementation: *a track click puts the record on and drops the needle
  there; the rest of the record follows; then silence; later is `Queue
  album`; a different record replaces the run.*
- Two new visible controls (`Queue album`; later `Queue next` under §3's
  pre-decided semantics), one new persisted snapshot, zero engine changes.
- The shift-click stack gains its pointer-reachable sibling before it ships,
  so the visible-control rule is met on arrival rather than retrofitted.
- `docs/BACKLOG.md`'s "The queue cannot be built" closes when §3 ships.
- What is deliberately not done: `Queue next` (deferred, semantics fixed);
  any continuation policy (refused, again); any second queue lane (refused);
  **repeat** in the engine (unchanged from ADR-0014's deferral). *Shuffle* was
  on that list until 2026-08-10 and is not any more — see the amendment's §3
  for the one property the engine gained and the gapless argument that put it
  there.
