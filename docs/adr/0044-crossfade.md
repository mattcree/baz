# ADR-0044: Crossfade — the producer mixes it, the front end says where it may happen

**Status**: proposed (2026-08-22) · answers the owner's *"another backlog item: crossfade"* … *"enable disable as a control and a setting for how long"* · amends [ADR-0003](0003-the-playback-engine.md)'s boundary policy and [ADR-0013](0013-replaygain.md)'s single gain stage

## Context

The surface the owner asked for is settled and small: **a switch and a
duration**, in Settings → Playback beside the gapless and ReplayGain controls,
because what it reads is the player.

**The engine half is the work**, and `docs/BACKLOG.md` named three questions
that had to be answered before a line of it was written. This record answers
them, and adds two the investigation found.

baz's boundary policy today is **drain-and-restart** at a track edge, and a
crossfade is the opposite shape: two tracks sounding at once. The obvious
implementation — two decoders alive, their outputs summed in the pump — is the
expensive one: `Session::pump` is the realtime path this project has been most
careful with, it caps every block at a track boundary so a per-track ReplayGain
lands on the *right sample*, and it has a transparent short-circuit that hands
the ring's own slices to the sink untouched. Growing it a second source and a
mix stage puts all of that at risk for a feature nobody asked to be
sample-exact.

## Decision

### 1. The **producer** mixes the overlap; the pump does not change at all

The producer thread already decodes one track ahead, and every track after the
anchor is decoded **whole**, into a `Vec<f32>`, before it is pushed
(`Session::produce`). So the overlap can be made where the samples already sit
in memory:

- hold back the last *n* samples of the outgoing track rather than pushing them;
- when the incoming track's samples are ready, **sum the held tail into its
  head** under a pair of ramps;
- push the mixed block, then the rest.

The ring still carries **one** stream, the pump still reads one chunk and writes
it, and not a line of the realtime path moves. The cost is memory — one fade's
worth of samples, 8 s at 48 kHz stereo is about 3 MB — and it is paid only while
a fade is configured.

**The ramps are equal-power** (`sin`/`cos`), not linear. Two uncorrelated
signals summed under linear ramps dip about 3 dB in the middle, which is
audible as a hole; equal-power holds the sum flat, which is what every
crossfading mixer has done since tape.

### 2. **The front end says where a fade is allowed**, because the engine cannot know

*"What happens at a gapless boundary?"* — the first of the backlog's questions,
and the one whose answer changed the shape of the whole feature.

A crossfade across an album's own seam destroys exactly the thing this project
spends a chapter protecting. So a fade must be **skipped between gapless
neighbours** — and **`baz-core` cannot decide that**. The engine plays a list of
paths; *album* is a library fact it has never had and should not grow. Two
consecutive files in one folder are not necessarily one record, and one record's
tracks are not necessarily consecutive in a queue.

So the queue carries the answer. Each queued item gains **`fade_into_next`**,
set by the front end that built the run:

| Run | `fade_into_next` |
|---|---|
| an album, played front to back | `false` at every internal seam — this is the case gapless exists for |
| a playlist, an assembled run, all songs, shuffle | `true`, except where two adjacent items are consecutive tracks of one album |

Which makes the setting mean **between records** rather than *between tracks*,
without the engine learning what a record is.

### 3. What it does to ReplayGain, and the one thing that is stated rather than solved

*"What does it do to ReplayGain and the equaliser?"* — the second question.

**The equaliser is easy and needs no decision.** It is a property of the
*listener*, applied after the sum, exactly where it already is.

**ReplayGain is a property of the track**, and during an overlap there are two
tracks and one downstream gain. The resolution:

- The next track's `TrackBound.start_sample` is placed at the **end** of the
  overlap, so the whole fade plays under the *outgoing* track's gain.
- The producer **pre-scales the incoming head** by the ratio of the two tracks'
  tagged gains, so that after the outgoing gain is applied downstream it lands
  where the incoming track's own gain would have put it.

That is exact in track mode with tagged files, and a no-op in album mode within
one album (both tracks carry the same album gain, so the ratio is 1) — which is
also the case a fade is skipped in anyway.

**Where it is not exact**, and this is stated rather than hidden: the producer
knows the *tags*, and the engine resolves the *applied* gain from the tags, the
mode, the pre-amps and anything baz measured itself (ADR-0015). Where those
disagree — an untagged file playing at the untagged pre-amp, or a computed gain
standing in for a missing tag — the pre-scale is off by that difference for the
length of the fade. The exact fix is a per-source gain in the pump, which is the
thing §1 exists to avoid; it can be revisited if anybody can hear it.

### 4. A skip does not crossfade

*"Does a skip crossfade?"* — the third question, and the answer is **no**.

A manual `Next` is *take me to the next thing now*, and a listener who presses
it and then waits six seconds has been told their button is slow. It also falls
out of the architecture for free: a skip tears the session down and starts a new
one (drain-and-restart), so there is no held tail to mix and nothing to write.
A fade happens where a track **ends on its own**, and nowhere else.

The same reasoning covers seek, a queue edit over the boundary, and stop.

### 5. A fade never crosses a rate change, and it costs bit-perfect

Two found in the writing, both of which would otherwise have been discovered as
defects:

**A rate change ends a session.** Under the bit-perfect default, the producer
stops one track short when the next is stored at another rate, and the engine
reopens the output. There is nothing to mix into, because the incoming track's
samples do not exist in this session — so a fade is skipped there, exactly as at
a gapless seam and for a more absolute reason.

**Mixing is not bit-perfect, and the signal path must say so.** baz's headline
claim is that the samples reaching the device are the file's own. Inside an
overlap they are the sum of two files under two ramps, which is a
transformation — so while a fade is configured, `Event::SignalPath` reports the
mix stage rather than `bit-perfect`. A player that quietly kept the badge while
mixing would be lying about the one thing it says loudest.

### 6. The surface

Settings → Playback, under the gapless and ReplayGain controls: **a switch, and
a duration** — the owner's own two. Off by default, because an album-first
player's default boundary is the one the record was mastered with. The duration
is a small set of choices rather than a free field, on the sleep timer's
argument: six numbers a listener picks from without reading beat a text box that
can hold `0.5`.

The words beside it say what §2 decided, because it is the thing that will
otherwise be read as a bug: **it does not fade between the tracks of a record.**

## Alternatives rejected

- **Two sessions summed in the pump.** The obvious shape and the expensive one
  — see Context. It is what §3's inexactness would buy, and the price is the
  realtime path.
- **A fade at every boundary, with gapless turned off while it is on.** One
  setting silently disabling another is the kind of coupling a listener
  discovers by hearing their favourite record wrong.
- **Fading on skip as well.** §4.
- **A free-text duration.** §6.

## Consequences

- One new protocol field (`fade_into_next` on a queued item) and one new
  command (the duration), both of which the front end owns.
- The realtime pump is untouched, and every bit-exactness fixture keeps
  passing unchanged — with a fade configured, the gapless fixtures must
  *still* pass, which is §2 stated as a test.
- `bit-perfect` becomes conditional on the setting, and says so.
- One stated inexactness in ReplayGain during an overlap (§3), with the exact
  fix named.

## Status of the work

**Built 2026-08-22**, in the shape this record describes and with the test it
named.

- **The producer mixes it.** `HeldTail` withholds the outgoing tail — a rolling
  window for the streamed anchor, a slice for every track after it, which are
  decoded whole — and `mix_overlap` sums it into the incoming head under
  `sin`/`cos` ramps. The realtime pump is not touched.
- **The front end says where.** `vm::fade_seams` reads the wall and marks every
  seam that is *not* two consecutive tracks of one edition; the flags travel on
  `SetQueue`/`UpdateQueue`/`UpdateQueueNext` as `fade_into_next`, additive and
  wire-stable, and `command_wire_format_is_stable` passes unchanged.
- **The surface** is an off and five lengths in Settings → Playback, with the
  sentence §6 asked for: it does not fade between the tracks of a record.
- **`bit-perfect` is conditional and says so** through
  `ConversionReason::Crossfade`. The warning that names a device to change
  stays silent for it — a fade is a choice, not a fault — while the readout
  reports *Mixing*.

**The test §2 asked for is `a_record_is_bit_exact_with_a_crossfade_configured`**
(`crates/baz-core/tests/engine.rs`): an album's own seam, with an eight-second
fade switched on, is sample-for-sample what it was.
`a_fade_between_records_overlaps_them_by_the_configured_length` is its
complement, and `a_zero_crossfade_leaves_the_run_untouched` pins that off means
off.

**Three edges the writing found**, each of which would otherwise have been a
defect: a held tail is still owed to the listener when the queue runs out, when
a rate change ends the session, and when the incoming track fails to decode.
All three release it unmixed rather than swallowing the end of the music.

## Amendment, 2026-09-03 — every seam

The owner, with a crossfade configured and a record playing: *"does crossfade
even work? I have it enabled and it doesn't seem to do anything … surely it
should take effect between all tracks."* It was working exactly as §2
decided — between records, never inside one — which is why an album played
front to back gave him nothing to hear.

**His call, and it stands: a crossfade the listener switched on crosses every
seam.** `vm::fade_seams` answers `true` at every seam but the last; the wall
is no longer consulted. §2's mechanism is unchanged — the front end still
says, per seam, and the engine still knows nothing of records — so a finer
rule (a record's own seams closed, or only its continuous ones) can return
through the same flag without touching `baz-core`. What is paid: a live
record's continuous seams are faded too while the setting is on, and
switching it off is the way to keep them. The setting's own sentence in
Settings → Playback now says so.

He also asked for the control *"somewhere near controls"*. The bottom bar's
transport has no mark for it in the sprite sheet yet; that is recorded in
`BACKLOG.md` beside the other drawing decisions rather than guessed.
