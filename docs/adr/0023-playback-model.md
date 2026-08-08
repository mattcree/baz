# ADR-0023: The playback model — one list with a cursor, and the queue as a record of a choice

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

`REFUSALS.md`'s entry stands: the queue empties and there is silence. The
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
  shuffle or repeat in the engine (unchanged from ADR-0014's deferral — both
  remain front-end expressions over `SetQueue`/`UpdateQueue`).
