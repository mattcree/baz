# ADR-0014: Editing the queue without silencing it — `JumpTo`, `UpdateQueue`, and identity over index

**Status**: accepted (2026-08-08) · extends the ADR-0003 command/event protocol; changes no existing message and no existing behaviour · no schema change (the queue is session state, not library state) · unblocks the interactive queue panel that shipped read-only

## Context

baz has a queue panel. It shows what was handed to the engine, in play order,
with the playing row marked — and it is text. Clicking a row does nothing,
rows cannot be dragged, and nothing can be removed. That was not a design
preference; `crates/baz/src/player.rs` names the reason precisely, and it is a
protocol fact:

- **Click a row to jump to it** needs a command that names a queue position.
  The protocol had `Next` and `Previous` and nothing else, so reaching row 9
  meant eight `Next`s — eight sessions, eight `SignalPath` reports, and eight
  tracks of audio briefly reaching the output. That is not a jump.
- **Remove a track** and **reorder** need a queue replacement that *keeps
  playing*. `SetQueue` is documented to stop ("any playback in progress stops"),
  so the obvious implementation — re-send the queue minus one track — would
  silence the music to delete a track the listener was not listening to.

Three ordinary interactions, missing for want of two commands. The front end
did the honest thing and drew no affordance rather than a fake one; this ADR
is the other half of that bargain.

## Decision

### 1. `Command::JumpTo { position }` — the queue-relative sibling of `Seek`

`Seek` names a position **inside a track**; `JumpTo` names a position **in the
queue**. It plays that entry from its beginning, and it is implemented as the
same drain-and-restart machinery `Next`, `Previous` and `Seek` already share,
aimed at an arbitrary index — `Next` is now literally `JumpTo(current + 1)` and
the code says so. A third way to select a queue entry would have meant a third
set of answers about what is discarded, what happens to pause, and what happens
past the end.

| state | `JumpTo` |
|---|---|
| playing | abandons the session (its buffered audio is discarded, as `Next` does) and starts at `position` |
| paused | **moves and resumes** |
| stopped | **starts playing at `position`** |
| out of range, or any position of an empty queue | `QueueEnded`; a later `Play` starts from the top |

Two of those deserve their reason stated.

**Paused moves and resumes** because `Next` and `Previous` do, and the engine's
command table states it for both. Three transport commands that select a queue
entry must not disagree about whether pressing them starts the music.

**Stopped starts playing**, which is the one place it parts company with `Next`
and `Previous`. They are no-ops while stopped because they are *relative* and
there is nothing to be relative to; an absolute position has no such difficulty.
A listener clicking a row of a stopped queue means "play this", and `JumpTo` is
`Play` aimed at a chosen entry.

**Out of range ends the run** rather than clamping to the last entry or
erroring. Clamping would play a track the listener did not point at, which is a
worse answer than stopping. Erroring has nowhere to go: this protocol has no
error channel, and a queue that shrank under a click is an ordinary race rather
than a fault. Ending the run is exactly what `Next` past the last track already
did, so there is one answer to "past the end", not two.

### 2. `Command::UpdateQueue { paths }` — the edit

A second whole-queue command, identical in payload to `SetQueue` and opposite
in intent: **the music keeps playing.** The guarantee is stated as a testable
claim rather than an aspiration:

> An edit that does not touch the playing track does not disturb one delivered
> sample. Offline, the delivered stream either side of an edit is bit-identical
> to an unedited run.

`SetQueue` is untouched. It remains the reset — "forget what you were doing and
hold this instead" — and its wire format, its `Stopped`, and its documented
behaviour are exactly what they were. Two names for two intents beats one
command with a `keep_playing: bool`, which would have made every call site read
as a question about a flag rather than a statement of what the user did.

**Whole list, not operations.** `RemoveAt { index }`, `MoveTo { from, to }` and
`EnqueueAfter { index, paths }` were the alternative and are rejected for the
reason `Seek`, `SetMute` and `SetReplayGain` are absolute: *an index-based delta
applied against a stale picture removes a different track, and neither side can
tell.* A front end's list can go stale between the click and the send — a track
can fail and be skipped, a rate change can hand over — and a delta has no way to
notice. The whole queue cannot desynchronize, expresses every edit including
multi-selection removal and drag-reorder in one message, and costs the sender
nothing: it is holding the list it just edited.

The payload argument against it is real and answered: a queue is a play queue,
not a library, and `SetQueue` already sends exactly this list on every album
click. An edit is a user gesture, not a cadence, so the traffic is bounded by
how fast somebody can drag rows.

### 3. Identity, not index

**The thing that survives an edit is the playing track. Its position is
whatever the new list makes it.** Remove two tracks above it and it is
renumbered; a front end that assumed otherwise would mark the wrong row and a
subsequent `Next` would skip the wrong track.

So the engine re-derives the position from the *path* it is delivering:

1. the old index, if the new queue holds that path there — an edit that did not
   disturb the playing track must not renumber it;
2. otherwise the first occurrence of that path — a queue may legitimately repeat
   a file, and "first" is the same answer front ends already give when
   reconciling `TrackStarted`;
3. otherwise the track is gone, which is case 4 below.

From that moment every queue-relative command — `Next`, `Previous`, `Seek`,
`JumpTo` — is answered in the **new** queue's terms, even while the session
underneath is still playing from the snapshot it was started with. That
translation lives in exactly one place (`Control::playing_index`), so a
session's index space being its own is a fact one function knows.

### 4. Removing the playing track: the one case where the index wins

That edit *does* touch the playing track, so the no-interruption guarantee
explicitly does not cover it, and the engine says what happens instead:
**playback moves to the entry that took its place — the same index in the new
queue — from its start, exactly as `JumpTo` would.** Past the end of a shortened
queue (or into an emptied one) the run ends.

For the ordinary gesture — remove the track I am listening to — that is the
track which follows it, which is what a listener expects. Index is the right
answer in precisely this case *because identity did not survive*: there is no
path left to look for. The alternatives were considered: continuing to play a
file the queue no longer contains leaves the reported position meaning nothing,
and stopping outright is a bigger reaction than the gesture asked for.

### 5. How the audio survives: cut at the boundary, then hand over

A session snapshots its queue at start, so nothing about an edit reaches the
producer, the ring or the sink — that is what keeps the audio in flight
untouched. What the edit does is mark the session to deliver **the track it is
on and not one sample further**. The pump already refuses to read across a track
boundary (it must, so a per-track ReplayGain lands on the right sample), so the
cut costs one comparison and lands exactly on the boundary. The engine then
hands the rest of the run to a fresh session at the edited queue's next
position, **draining** the sink rather than discarding it: a session that played
its track out is owed its tail, and only an *abandoned* session has audio nobody
wants to hear. That is the same rule ADR-0009's rate-change handover follows.

Consequences, stated rather than hidden:

- **The delivered stream is unchanged.** The cut is on the boundary, so nothing
  is lost and nothing repeats.
- **The boundary out of the edited-over track is not gapless.** It becomes
  `Next`'s fresh decode (first audio in milliseconds for local files) rather
  than a sample-accurate splice. One edit costs one boundary, and the gapless
  path keeps its one guarantee: adjacent tracks playing to completion.
- **One decode-ahead is wasted** — the superseded session had already prefetched
  what it thought was next. Bounded to one track, discarded with the session.
- **The superseded session announces nothing further.** It is cut before the
  next track's first sample, and a position from its own index space would be a
  lie about the new queue.
- **An edit before the first sample is a plain restart.** With nothing delivered
  there is no audio to protect, so the session is rebuilt on the new queue — and
  it cannot be heard, because a track is reported started in the same engine
  iteration its first samples are pumped.

### 6. What the engine announces: `Event::QueueChanged { len, position }`

Emitted on an accepted `SetQueue` or `UpdateQueue` that actually changed
something, and never otherwise. `position` is the engine's own re-derived answer
(`None` when nothing is playing) and a front end should prefer it to its own
computation: the two differ exactly when an edit races a track boundary, and the
engine's is the one the audio agrees with.

**The paths are deliberately not echoed, and there is no `EngineHandle::queue()`
accessor.** The engine applies what it is given verbatim — no filtering, no
validation, no de-duplication — so a sender's copy of its own list is exact by
construction, and repeating an album back on every edit would be churn to state
a fact the receiver already has. What a front end genuinely cannot compute is
the re-derived position, and that is the field this event exists to carry, with
the length beside it as a cheap check that the two sides hold the same number of
entries. A pull accessor was costed and left out: to honour the engine's
state-before-event ordering contract it would have to publish under a lock
before every `TrackStarted`, which is real machinery to answer a question the
events already answer. It becomes worth adding the day a *second* front end
attaches to a running engine — the same day the single-consumer event receiver
needs fan-out — and not before.

## Consequences

- Two new commands and one new event. No existing message changes shape or
  meaning; both enums stay `#[non_exhaustive]` and `Eq`, and every new variant's
  bytes are pinned by `wire_format_is_stable` tests.
- `QueueChanged` now rides along with the `SetQueue` every front end already
  sends. Nothing breaks — the event enum is `#[non_exhaustive]`, so a front end
  that ignores it is correct — but tests that assert "silence" after a
  `SetQueue` had to be told what the news is.
- Editing while playing costs one non-gapless boundary and one wasted
  decode-ahead. A shift-only edit (removing an already-played track, say) could
  in principle keep its gapless splice, because the rest of the run is unchanged
  and only the numbering moved. That optimisation needs a second index space
  inside the session's reporting, and one mechanism that is always right beats
  two mechanisms of which one is faster; it is recorded here as deliberately not
  done rather than not noticed.
- The engine still holds no opinion about *what* is queued: no validation, no
  de-duplication, no reordering of its own. A queue of paths that do not exist
  is a queue of tracks that will fail one by one, exactly as before.

## Deliberately left out

- **`EngineHandle::queue()`** and a paths-carrying event — §6.
- **Shuffle and repeat.** Both are queue *policies*, and both are expressible on
  top of what this ADR adds: a front end shuffles by sending the order it wants.
  Whether the engine should own them is a separate decision with its own
  questions (does shuffle survive an edit? what does `Previous` mean under it?),
  and it stays in `docs/BACKLOG.md`.
- **A gapless edit.** See Consequences.
- **Per-entry identity.** Entries are paths, so a queue that lists one file
  twice has two entries the engine cannot tell apart; the reconciliation rule
  above answers with the first. Opaque entry ids would fix that and would change
  every message that carries a path, which is not a price this gesture is worth.

## What a front end needs

- **Click a row** → send `Command::JumpTo { position }` with the row's
  zero-based index. It works whatever the transport is doing, including stopped.
  Expect `TrackStarted { path, position }` (or `QueueEnded` if the index is past
  the end), and mark the row from *that*, not optimistically.
- **Remove / reorder / append** → send `Command::UpdateQueue { paths }` with the
  queue as it should now be, in play order. Never send `SetQueue` for an edit:
  it stops the music, and that is what it is for.
- **Observe `Event::QueueChanged { len, position }`** and take `position` as the
  playing row — it is the engine's re-derived answer and it beats yours. Check
  `len` against the length you sent; a mismatch means your picture is stale, and
  the fix is to re-send the queue you intend rather than to patch it.
- **Expect no `Stopped` from an edit.** If the edit removed the playing track,
  expect `TrackStarted` for the entry that took its place (or `QueueEnded`), and
  nothing else.
- **Nothing else changes.** `TrackStarted`'s position remains the authority
  per track, the existing reconciliation of remembered paths against engine
  positions keeps working untouched, and an edit while paused leaves the
  transport paused.
