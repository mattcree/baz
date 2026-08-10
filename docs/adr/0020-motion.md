# ADR-0020: Motion — bounded transitions, because the premise was wrong

**Status**: accepted (2026-08-08), amended (2026-08-09: pointer-derived deformation) · **amends [ADR-0017](0017-design-direction.md)** (whose "0 ms motion" row is reversed) and supersedes `docs/design/02-visual-language.md` §7's prohibition

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

## Amendment (2026-08-09): pointer-derived deformation

The index rail's fisheye — the owner's ask: *"magnification style … like mac OS
dock. you move your mouse and it makes the hovered item bigger, and the
surrounding ones"* — is continuous, pointer-tracked movement, which is on
neither list above. It is also not a transition: nothing about it can be
expressed as a bounded tween, because the letter sizes are a function of
**where the pointer is**, not of how long anything has been true. Shipping it
under §2's list silently would make the list a fiction, so the class is named
here instead.

5. **Pointer-derived deformation is permitted**, as a class distinct from the
   bounded tween, under its own discipline:
   - the deformed geometry is a **pure function of the current pointer
     position** (`theme::magnify` for size, `theme::magnify_shift` — its
     integral, the dock's own mechanism — for place; both unit-tested: peak
     under the pointer, monotone symmetric falloff, exact rest beyond the
     reach, displacement that preserves order and agrees with the hit test) —
     no clock, no `Tween`, no subscription, no state, nothing to settle;
   - it costs frames only while the pointer moves, **by construction rather
     than by a guard**: iced 0.13 requests a redraw for every window event
     (`iced_winit::program`, unconditionally after event processing), the
     deformation is read at draw time from the live cursor, and there is no
     clock to stop. The §Measurements idle claim is untouched — a resting
     pointer is a resting rail;
   - it may deform only the surface it magnifies: the lane's width, the wall's
     grid and every layout outside the widget are invariant (the width-algebra
     tests hold, unmodified). Within the lane the strip scales *and spreads*,
     and both are functions of **rest** geometry — distances are measured to
     centres the deformation itself never touches, so there is no feedback
     through the deformed output and a resting pointer draws a stable frame.
     The spread is bounded by the head-room the strip really has
     (`theme::MAGNIFY_SPREAD`, which the rail's elision capacity reserves), so
     the deformation cannot push anything out of its surface.
6. **The snap back to rest on pointer-exit is a hard cut**, not a sixth tween.
   The deformation is the pointer's shadow on the strip; when the pointer
   leaves, the input is gone and the next frame is the rest frame. A
   relaxation tween here would attach a clock to the one motion class whose
   whole argument is that it has none. The first shipped cut moved only glyph
   sizes; with displacement the cut also moves positions — up to
   `MAGNIFY_SPREAD` (45 px) at the strip's far ends — and the argument is
   unchanged in kind: what eases in this product is *statements arriving*
   (five tweens, each a state that became true), and the lens is not a state —
   it is the hand's own shadow, at the window's far edge, under the pointer
   that is leaving. Easing it home would animate the departure of an input
   nobody is looking at by the time it matters; the dock eases its relaxation
   because its lens is the OS's centrepiece, and this one is an index rail.
7. **The deformation and the interaction must be one function.** Whatever the
   lens magnifies most, the press fires and the hover marks: the winning slot
   is the hit test's answer, the wash chip and the paper lift sit on exactly
   that slot, and displacement provably cannot split them
   (`|d + shift(d)|` grows with `|d|`, so the nearest displaced glyph is the
   nearest rest slot — asserted in `spine.rs`'s tests). A deformation whose
   optics and whose targets could disagree is forbidden in this class, because
   it would aim the owner's hand at a letter the click will not take.

One instance ships: the index rail (`spine.rs`). A second instance means
re-arguing this amendment, not citing it.

## Amendment (2026-08-10): the hero's dissolve, and one refusal reversed

The owner: *"when changing track there isn't any kind of nice visual transition
for album art in now playing. we should have something a bit nicer, like a
quick fade"*.

§3 above forbids **album-art crossfades** by name, and it is now the only one
of that list the owner has asked for. It comes off — narrowly, and with the
argument written down, because a refusal that is quietly dropped is a list
nobody can trust the rest of.

**Why the refusal was there, and why it does not survive contact.** §3's rule
is *motion states what changed; it never decorates*. The refusal read
album-art crossfade as decoration: a picture prettily giving way to another
picture. On the surfaces that existed when §3 was written, that was right —
the artwork was a 320 px tile on a wall of a hundred others, and a tile fading
into a tile states nothing a tile cannot state by simply being there. It is
wrong on the surface that exists now. ADR-0029 made the Now playing place's
artwork **the subject** — one work, at the size the viewport allows, with the
room lit from its own palette — and on that surface *the picture changing is
the whole of what the change is*. There is no other element that could state
it. The cut was not stating it either: it was interrupting.

8. **The hero's dissolve is permitted**, as ADR-0020 §2's sixth transition and
   under §1's discipline in full — a bounded `Tween`, a subscription that is a
   function of state, and no clock at rest. Its scope is exactly this:
   - **`motion::DISSOLVE`, which *is* `motion::LAMP`** — 200 ms, linear. It is
     an alias and not a copy: the lamp warms because the light moved to
     another record, and the hero dissolves because the picture of that record
     changed, so they are one event and must finish together
     (`the_dissolve_is_the_lamps_own_number`). No number was invented; the
     owner said *quick* and named none, and 90 ms — `INK` and `TILE` — is a
     cut with a smear on it.
   - **One surface**: the Now playing place's hero. The wall's tiles, the
     lane's rows, every collage and the bar's own small sleeve are untouched,
     and §3's neighbouring refusal — *any fade as a thumbnail decodes* —
     stands unmodified. A second consumer means re-arguing this amendment,
     not citing it.
   - **The predicate is the picture, not the track.** Consecutive tracks on
     one record share a cover; the shell holds what it has committed to
     drawing and compares the *handle*
     (`a_dissolve_needs_two_pictures_that_are_not_the_same_picture`). A
     twelve-track album is twelve track changes, no transition, and no clock.
   - **It begins when the new art is ready**, not when the track starts.
     `art::load_hero` decodes off-thread; a dissolve begun before the decode
     lands fades to nothing and then pops, which is worse than the cut it
     replaces. So the surface **holds the picture it has** until there is an
     answer for the incoming record — either a hero or *this record has no
     art* — and only then crosses.
   - **The field crosses with it.** `field::dissolve` takes the same `t` the
     cover's incoming layer is drawn at. The wash is derived from the cover;
     a cover that dissolved over a room that cut would be the seam ADR-0029's
     one-wash fix removed, back in time instead of in space.
   - **Two pictures, or no transition.** A record with no art draws the wall's
     deterministic gradient, which is a stand-in rather than artwork; fading a
     stand-in is decoration and stays forbidden. So art → no art, and no art →
     art, are hard cuts.
   - **One square, or no transition.** Both covers are drawn at
     `now_playing::art_edge`'s answer, whose third term is the decode's own
     pixels. Two decodes that resolve to different squares would make the
     dissolve a resize as well, and §3 forbids animating geometry — so that
     change stays the cut it has always been
     (`a_dissolve_is_refused_where_the_two_covers_are_not_one_square`).

**What it costs, measured rather than assumed** (`docs/design/impl/art-crossfade/`,
which films the real binary at 60 fps and reads every frame back). At rest:
nothing — one `image` widget, as before, and the `Tween` is settled so the
timer does not exist (`a_settled_surface_has_nothing_to_dissolve`,
`the_motion_clock_is_off_until_something_moves`, both extended for this).
While it runs: one extra `image` in a `stack!`, for 200 ms, once per **record**
change, and no new decode or cache — `art::HERO_CACHE_ENTRIES` is 2 and its
second entry already holds the record that just stopped, which
`the_hero_lru_holds_both_records_a_dissolve_needs` checks rather than trusts.

The film counts **twelve** distinct frames across the transition against the
`before` build's **one**, and twelve is the number
`a_200ms_transition_is_about_twelve_frames_at_60hz` derives from the tween's
arithmetic with no window anywhere near it. The cover's own fraction and the
field's never disagree by more than **0.018**. CPU at rest is flat between the
two builds.

**And one thing this amendment reveals rather than fixes.** The hold measures
the wait for `art::load_hero` — **33 ms** on a quiet machine with the fixture's
600 px covers, and 100–320 ms on a loaded one — during which the previous
record's cover stands under the new record's title. It is bounded, it is
strictly better than what it replaces (which cut to a 320 px thumbnail on a
room with no field and then popped to full size), and it goes to zero the
moment the *successor's* hero is prefetched, which `art::HERO_CACHE_ENTRIES`
already describes as one line once ADR-0034's `Origin` work can name the next
record. The crossfade is the first consumer that makes that prefetch worth
having, and a listener whose covers are 3000 px is the case that would make it
urgent.
