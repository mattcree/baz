# ADR-0020: Motion — bounded transitions, because the premise was wrong

**Status**: accepted (2026-08-08) · **amends [ADR-0017](0017-design-direction.md)** (whose "0 ms motion" row is reversed) and supersedes `docs/design/02-visual-language.md` §7's prohibition

## Context

Every design document baz has written specifies **hard cuts everywhere**. The
reasoning, from `02-visual-language.md` §7, was that a transition

> "means driving state from a `window::frames()` subscription, **which redraws
> whether or not anything is moving**"

— so motion would cost the startup and memory promises. ADR-0017 upheld it.

That sentence is true of an *unconditional* subscription and false of a
**bounded** one. Worse, baz already ships the bounded pattern twice in
`app.rs`: `ColumnHold`'s `time::every` guard, and a `window::frames()`
subscription dropped once startup is logged. The mechanism was never missing;
the specification simply mis-stated what it would cost, and three documents
inherited the error.

The owner's verdict on the result — *"subtly clunky"*, *"honestly just all
quite clunky"* — is the symptom this decision produced.

## Measurements

Spiked on iced 0.13.1 with baz's own feature set, 1280×860, 120 tiles per
frame, on a real GPU (`docs/design/04-fluidity.md` carries the full table and
the llvmpipe control):

| driver | idle after motion | 20 × 150 ms burst | continuous |
|---|---|---|---|
| **bounded `time::every`** | **0.0 % CPU**, 8 frames / 4 s | 5.1 % | 4.0 % @ 60 fps |
| unconditional `frames()` | 2.0 %, **60 fps forever** | 1.9 % | 2.0 % |

The decisive line: *frames after the last tween settled: 1, over the 3.9 s
since*. **The clock stops.** Startup-to-interactive does not move — the 74 ms
spread between two runs of the *unmodified* binary swamps any difference —
and `size_of::<Tween>()` is 48 bytes.

## Decision

1. **Bounded motion is permitted.** A `Tween` shaped like `ColumnHold` — told
   the time rather than asking for it, pure and unit-testable — drives
   transitions from a subscription that is **active only while something is
   moving**. "No redraw while idle" stops being a prohibition on motion and
   becomes a boolean the subscription reads, and a test asserts it.
2. **The five transitions that ship**, ranked by clunkiness removed per unit of
   work: icon-button ink fade (90 ms) · queue popover fade + 8 px slide
   (140 ms) · shelf tile hover (90 ms, one tween keyed by hovered id, never one
   per tile) · inspector width (150 ms, interacts with `ColumnHold`) · lamp
   warming on track change (200 ms linear).
3. **Still forbidden, and these are refusals**: grid stagger, thumbnail
   fade-in, album-art crossfade, and any animation of bar geometry. Motion
   states what changed; it never decorates, and it never moves the transport.
4. Every other §7 prohibition survives unchanged.

## Consequences

- `02-visual-language.md` §7 and ADR-0017's motion row are superseded; both
  gain a pointer here rather than being edited into silence, because the
  mistake is instructive: a constraint was asserted rather than measured, and
  three documents inherited it without anyone checking.
- A transition that cannot be expressed as a bounded tween does not ship.
- The idle-cost claim is a **test**, not a promise: the suite asserts the
  subscription is inactive when no tween is running.
