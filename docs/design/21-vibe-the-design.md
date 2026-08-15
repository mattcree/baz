# 21 — Composing a playlist: the design as it stands

**This is the current thinking, written as one thing.** It supersedes the
proposals scattered through `18`–`20` and the three review notes for the
purpose of building: where they disagree, this document is what we mean.

How it got here is worth knowing but is not this document.
`docs/design/19-vibe-next-phase.md` is the review that produced it — the
owner's complaints, the eight defects, the three layouts, and the quorum in
`docs/design/quorum/2026-08-15-vibe.jsonl` that found nine things the first
draft had missed. Every claim below that came from somewhere is attributed at
the end.

**Nothing here is built.** Three decisions remain open (§11) and one of them is
consent rather than design.

---

## 1. What the feature is

You describe the music you want and draw how it should move. baz builds a
playlist out of your own files, on your own machine, and saves an ordinary
`.m3u8`.

It is the most distinctive thing in the product and the least finished. What
ships today works — the engine is measured and correct — behind a page that
asks for five decisions before it shows a single track.

## 2. The model, in one line

**There is one request, and it is a sentence about sound.** Everything else is
either a way of writing that sentence or a separate question about movement.

That sentence is load-bearing, because the previous design drew *mood* and
*words* as two inputs of equal weight, which implied they combine. They cannot:
there is one text embedding, so a mood and a phrase would have to be
concatenated — which is exactly what filling the field already does, only
hidden. A mood is not an input. It is a shortcut that writes into the only
input there is.

So the page asks **two questions**, in the listener's own words:

- **What do you want to hear?**
- **How should it move?**

And nothing on the surface says *vibe*, *contour*, *dimension* or *recipe*.

## 3. What each control decides

This is a description of the implementation rather than a metaphor for it.

| the control | decides | mechanically |
|---|---|---|
| what you want to hear | **which** songs are eligible | one text embedding, compared against every track baz has heard |
| how it should move | **where** each one goes | positions along the line; each position takes the nearest eligible song at that height |
| how long | **how many** positions there are | the length, adjusted for the songs' real durations |

Stating this on screen is what makes the feature legible. Every question a
listener has becomes answerable without anybody understanding an embedding:

- *Why is this song here?* The words let it in; the line put it fourth.
- *Why did my change do nothing?* You moved the line, which reorders rather
  than re-selects.
- *Why is it short?* Only eleven songs were eligible that far up.

## 4. What you want to hear

One band. The **field is at the head of it**, with one sentence underneath:
*this is exactly what Baz searches for.* That sentence says there is no hidden
state, nothing accumulating out of sight, and what you can read is what will
happen.

Beneath the field, two ways to write it:

- **Starting points** — six named moods (`Sunday morning`, `Late-night drive`,
  `Focus`, `Workout`, `Wind down`, `Party`). Pressing one **replaces the line**
  with its words, visibly, and you can edit them immediately.
- **A vocabulary** — twelve chips in three rows: what it is made of, what it
  feels like, how it moves. Each **appends** to the line with a comma.

There is no language model here. The text tower answers *descriptive phrases
about sound*: "slow sparse piano, melancholy" retrieves; "songs about my ex"
retrieves noise. The vocabulary is the answer to that, and it is a route rather
than a rule — telling somebody to "describe the sound, not the story" without
giving them the words is a scold.

**The twelve chips are chosen by a scored run against the baseline corpus**
(`crates/baz-vibe/src/bin/vibe-baseline.rs`), not by taste. That harness has
been in the tree unused since the feature shipped; this is its first customer,
and it is the same rule backlog item 59 already states: a prompt change that
cannot be measured is a superstition.

### The rules, which are behaviours rather than modes

| the listener | then |
|---|---|
| presses a starting point | its words replace the line, visibly, editable at once |
| edits the line | the starting point's light goes out — not a mode switching off, a label ceasing to be true |
| presses a second starting point | it replaces the line; there is no way to have two, because there is one line. Undo restores |
| presses a vocabulary chip | it appends to the line, with a comma |

And the effect that used to be silent is bounded: **a starting point sets the
shape and the length only while the listener has not set them themselves.**
Drag a point once and they are yours; from then on a mood changes the words and
nothing else. Invisible when it is right.

## 5. How it should move

One line by default, and it is **a blend** — the owner's instruction — with one
correction from the room: the blend is a **weighted** mean with energy
dominant, not a plain average. Each dimension is a rank within the collection,
so an unweighted mean puts loud-and-slow in the same place as quiet-and-fast,
and a line drawn through the middle would be satisfied by tracks that sound
nothing alike. That is the *"the dots aren't following my line"* failure, and
it would have come back wearing a different hat.

Seeding each dimension from the blend stays exactly consistent under weighting
— set every dimension to the same curve and the weighted mean is that curve,
whatever the weights — so the instruction survives the correction intact. This
is the sort of thing that gets "simplified" later by somebody who does not know
why it held, so it is written down here.

The control carries seven things:

1. **One line**, drawn over the axis.
2. **The axis labelled in words** — *loud, fast, busy* at the top, *quiet,
   slow, sparse* at the bottom. No legend, no key.
3. **The eligible songs drawn behind it** (§6).
4. **A sentence stating the shape**, updating as it is dragged: *"starts quiet,
   climbs to a peak two-thirds through, comes down."*
5. **Presets as chips underneath** — `Any`, `Steady`, `Slow build`, `Peak &
   fall`, `Wind down`. Underneath rather than as thumbnails above, because they
   are the press-instead-of-drag route to the same outcome.
6. **A ring on the focused point**, arrow keys to nudge it, grab regions at
   twice the drawn radius.
7. **An expander** — *tune each thing Baz listens for* — a labelled control,
   not a bare triangle, whose curves open holding this line's own points.

The `−`/`+` stepper that mints a curve is deleted. Nothing else in baz creates
a control with a stepper.

## 6. The relationship, made visible

Four readouts. Three are free; the fourth is the cheapest thing in this
document and the most valuable.

**A live match count under the field** — *"matches 340 songs of the 9 412 Baz
has heard"* — one text embedding, debounced ~400 ms after typing stops, then a
comparison against vectors already in memory. Tens of milliseconds. This is the
difference between a text box and a control somebody can learn.

**The cloud behind the line becomes the eligible songs**, not the whole
library. Free, given the count above — the same numbers drawn instead of
counted — and it is the clearest picture of cause and effect in the feature:
narrow the phrase and watch the cloud thin out under your curve; draw the line
where the cloud is not and you know what will happen before pressing anything.

**Match strength per row**, three ticks filled by strength, never a colour.
Retrieval already computes this to choose the song and currently discards it.
Three buckets, so drift in the underlying numbers never changes the picture. A
weak tick at position five is not a failure to hide: it says the line asked for
something the words did not have much of, which is true and useful.

**New / kept after a recompose, with one sentence naming the cause** — *"adding
`strings` narrowed the pool from 340 to 291 and changed 6 of the 18. The order
is the same because you did not move the line."* Keep the previous list,
compare by path. One use teaches the entire model, and nobody has to be taught
it.

### One deliberate refusal

**The list does not update while the line is dragged.** It is affordable —
retrieval over an analysed library is sub-second — and it is still wrong: a
result that changes under your hand cannot be read, and you would be tuning
against a moving target. Everything *about* the answer updates live; the answer
waits to be asked for.

## 7. The states

Nine, of which the shipping build designs one. The first two are where a new
listener spends their entire first session.

**1 · Never listened.** The ask pane is fully drawn and fully pressable — set
up the request while it works — and the commitment says what it needs:
`Compose · needs listening first`. The result pane is the invitation, with the
cost stated (*9 412 tracks · roughly two hours · stop and resume any time*) and
a labelled *Listen to my library*. A page you cannot touch for two hours reads
as broken.

**2 · Listening.** A real reading, not a spinner: how many, how long left, a
pause control, and the tracks appearing as they are heard. The commitment
changes its own words to what it can do now — `Compose from 1 240 so far` —
which is true at every point on the bar.

**3 · Ready.** Nothing asked yet. The default shape and no words is a perfectly
good request: `Compose · about an hour` works, and gives you an hour of your
library shaped like a gentle arc. Nobody has to type anything to get a result.

**4 · Asked.** A mood press fills the line — and, if the listener has not
touched them, the shape and the length — and composes immediately.

**5 · Composing.** The rows it is about to fill, drawn as skeletons, with
*looking through 9 412 tracks…*

**6 · A list.** The result: the curve with a dot per song, the rows with their
match ticks, and a why-line when a row is selected. Selecting row four marks
its dot, drops a tick to the axis, and writes the two-part sentence — *your
words let it in; your line put it fourth.* Three cues, none of them a colour.

**7 · Edited.** Ordinary rows with the ordinary controls: reorder, remove, undo.
*Compose again* states what it will replace, and only once there is something
to lose.

**8 · Saved.** The ask pane becomes the naming pane for one press. The name is
proposed from the starting point and is editable in place. What lands on disk
is the same `.m3u8` as every other list in baz.

**Aside · the shape cannot be filled.** Warned twice — before the press against
the eligible cloud, and on the result in plain numbers — with *lower the line*
offered as a control. Nothing is padded to reach the asked-for length.

**Aside · tuning each dimension.** The expander, closed by default, its curves
seeded from the blend.

## 8. The layout

**Wide (≥ ~1180 px): two panes.** The ask on the left at a bounded width, the
result on the right. The list is on screen the whole time you are tuning, and
the curve sits over it so a line and its result are one picture.

**Narrow: the same three blocks stacked** — ask, curve, list — with **Compose
pinned** to the foot of the ask block so it is always in reach, and the page
landing on the list after a compose rather than at the top of the form. Under
700 px of height the curve collapses to its sentence and its presets, which is
its accessible form anyway.

**Nothing is hidden behind a tab at any width**, and the order of the two
questions, the words on every control and the visibility of the commitment are
the same on both. Somebody who learns this page on a laptop should not have to
learn it again on a desktop.

The row lane takes a **maximum measure** rather than the window; that is a
product-wide rule proposed in design note 20 §1, and this page is one of its
five customers.

## 9. Language

Every control says what it does, in the words a listener uses.

| never on screen | what is written instead |
|---|---|
| Vibe | *New playlist › From a mood* |
| contour, curve | *how should it move?* |
| dimension | *what Baz listens for* |
| recipe, preset | *starting point* |
| energy, valence | *loud, fast, busy* ↔ *quiet, slow, sparse* |
| tracks (as a unit of length) | *about an hour* |
| analysis | *Baz listening to your music* |

*Vibe* stays the feature's name in the changelog and in conversation. It is not
a word anybody should have to understand in order to use it.

## 10. What it costs

Honest numbers, measured where they exist
(`docs/design/impl/vibe-memory/`):

- **A compose peaks at ~1.13 GiB** at the current four workers, against a
  ~252 MiB idle baseline. Each worker costs about **145 MiB** of ONNX Runtime
  arena — not the 34 MB its weights file holds.
- **The text tower is roughly 350 MiB**, paid once. The live match count
  (§6) **loads it when the page opens** rather than at the first compose. That
  is this design's one real new cost, and it can be deferred to the first chip
  press if it is too much.
- **Nobody has measured a per-track analysis rate on a real library.** A
  24-track fixture completes inside a 90-second window at four workers. Order
  of magnitude, a nine-thousand-track library is hours — and every piece of
  copy that states a duration needs that measurement first.
- **Retrieval over an analysed library is sub-second**, which is what makes
  §6's live readouts affordable and §6's refusal a choice rather than a
  limitation.

## 11. What is still open

Three, and only one of them is design.

1. **May baz listen to a library before it is asked to?** Quietly, at first
   launch, so the feature is instant when somebody finds it — against hours of
   CPU and a laptop's battery, spent on something they have not asked for.
   *Proposed: no. Offer it here, state the cost, let it be a decision.* This is
   consent, and it is the owner's.
2. **Does a mood compose immediately?** One press to a list is the strongest
   moment in the design, and it removes the beat where you tune before
   committing. Only applies once there is something to compose from.
   *Proposed: yes.*
3. **Do the per-dimension curves ship on day one?** The blend is the default
   either way; this is only whether the expander exists now, seeded from the
   blend, or waits to be asked for. *Proposed: ships, closed.*

## 12. What this does not touch

- **The engine.** The contour still steers position against a
  collection-relative rank axis, and retrieval still runs per position. That
  half is measured and works.
- **Local only.** No network, no account, no model that is not already in the
  build.
- **The end state.** What is saved is an ordinary `.m3u8`, editable in a text
  editor, exactly like every other list in baz.
- **No padding.** A request the library cannot fill returns fewer songs and
  says why.

## 13. Build order

1. **The two panes and the measure.** Structural; everything else hangs off it,
   and the narrow stack falls out of the same breakpoint the album page has.
2. **The one-question band.** Field at the head, starting points and vocabulary
   beneath it, and the rules in §4. Deletes the two-band layout that caused the
   confusion.
3. **The shape control.** Blend weighting, axis labels, the sentence, keys and
   hit targets, presets as chips, the expander. The widget exists; this is a
   pass over it rather than a rewrite.
4. **The readouts.** Match count, the eligible cloud, per-row ticks, and the
   new/kept diff. Start with the diff — it is the cheapest and teaches the
   most.
5. **The result.** Ordinary rows, the why-line, save-with-a-name, compose-again
   with its warning, and a different draw.
6. **The first run.** Listening progress, partial composing, pause and resume.
   Gated on decision 1.
7. **The fork's removal.** One New playlist page — *start from a mood* or
   *start with an empty list* — last, so it lands on a page that is already
   right. This deletes the strip sentence the owner asked to remove by deleting
   its reason.

## 14. Where each decision came from

| what | from |
|---|---|
| two panes; list beside the controls | the owner, on the shipped page |
| the strip sentence goes | the owner, verbatim |
| one blended line, expanding to each dimension, seeded from the blend | the owner, verbatim |
| don't give them all these options | the owner, verbatim |
| one request, not two inputs | the owner spotting the model defect on the drawings |
| a very clear relationship between the controls and the list | the owner; §3 and §6 are the answer |
| the cold start is the primary state | the quorum (R1), from Lena's objection that a mood press cannot compose on an unlistened library |
| the blend must be weighted | the quorum (R2), from Marcus on rank axes |
| a vocabulary, not a rule | the quorum (R3), Jonas and Marcus |
| keys, targets, a sentence, presets as the accessible route | the quorum (R4), Wren |
| no pairing by hue | the quorum (R5), Wren, and the standing rule in this project |
| length in minutes on the commitment | the quorum (R6), Jonas and Toby |
| the fork dies | the quorum (R7), Sam and Ines |
| the result is a playlist, not a receipt | the quorum (R8), Ines |
| a row explains itself as a rank, never a score | the quorum (R9), Toby, Priya and Marcus |
| a request the library cannot fill says so | the quorum (R10), Marcus |
| narrow pins Compose and lands on the list | the quorum (R11), Priya |
| every number in §10 | measurement, `docs/design/impl/vibe-memory/` |
