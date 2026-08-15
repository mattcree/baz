# 22 — Vibe: the implementation plan

> **This is the build plan for [design 21](21-vibe-the-design.md), amended by
> the 2026-08-15 engine audit.** Design 21 remains the design: the one-request
> model, the two panes, the nine states, the language table all stand. What
> changed is the ground under §3 and §6 — the audit read
> `crates/baz-vibe/src/lib.rs` against the design's claims and found that the
> engine does not do what the design says it does. This plan makes the engine
> match the promise rather than weakening the promise to match the engine, and
> it orders the work so that nothing legible is built on top of anything
> unmeasured.
>
> **Carried out on 2026-08-15, except for §0.1.** Phases 0.2–4 are built and
> in the tree; what each one found is recorded where the work is, and the
> departures from design 21 are collected in that document's own header note.
> Six of §8's seven decisions were taken as proposed — the seventh is 0.1's
> verdict, which cannot be taken until 0.1 is done.
>
> **§0.1 is the one item no agent can do**, and it is the gate note 16 set
> rather than a formality: the harness, the consented corpus, the four systems
> and the materialized candidates are all ready, and the ratings need the
> owner's ears. If the semantic system does not beat the diversity-matched
> random control, the next work is engine quality — window policy, hybrid
> conventional constraints — and not more interface. Nothing built below
> changes that, because every measurement below is *comparative*: which policy
> concentrates the right songs better than another, never whether any of them
> is good.
>
> Where a phase's answer differed from what this plan expected, the plan is
> **not** edited to match — the difference is the finding:
>
> - **§0.2's largest-gap candidate is a trap.** Implemented literally it pinned
>   all eighteen swept prompts to the smallest pool allowed. The knee of the
>   ranked curve is what was meant.
> - **§0.3 could not fill three rows.** Design 21 §4's *moves like* row
>   measured as displacement without pull, and the sweep refused it.
> - **§2.1's two options were both wrong.** Neither page-open nor first-press:
>   the text tower loads on the **first settled phrase**, so nothing is paid
>   for until the listener has actually written something, and by the time they
>   stop typing the count is there.
> - **§1.5's tick boundaries cannot be absolute.** They are the eligible pool's
>   own terciles, per request.

## 1. What the audit found

Design 21 §3 claims its which/where/how-many table "is a description of the
implementation rather than a metaphor for it." As of the audit it is a
metaphor. Three findings, each with the code that proves it:

1. **There is no eligible set.** Selection is one soft cost —
   `0.45 × relevance + 0.30 × curve-fit + 0.20 × continuity` when words and a
   shape are both present (`lib.rs` `Weights::for_request`) — over *every*
   analysed track. No track is ever let in or kept out by the words; a poor
   word-match can win a slot by sitting at the right height. The match count,
   the eligible cloud and the why-line all presuppose a set that does not
   exist, and whatever threshold the surface invented, the composer would not
   respect it.
2. **Moving the line re-selects, not just reorders.** A shaped request
   retrieves per position (`walk`'s sampled shortlist), so dragging the curve
   changes which tracks are even in the room. Design 21 §3's answer — *"you
   moved the line, which reorders rather than re-selects"* — is false as
   shipped, and the new/kept diff would expose the contradiction on first use.
3. **Hidden state the design forswears.** §4 promises *"no hidden state,
   nothing accumulating out of sight."* The engine applies a +2.0 freshness
   penalty to recently-offered tracks — against weights summing to under 1.0,
   a ban, not a tiebreak — and `Vibe::create` (`crates/baz/src/vibe.rs`)
   increments the variation seed on every press. Compose twice with an
   identical request and the list changes, while the diff sentence would have
   to say *"you changed nothing."* Meanwhile the visible **Another version**
   press that carries this exact power already exists in code (`another()`)
   and was dropped from design 21's state 7.

And one finding about the ground itself: **nobody has ever verified the
retrieval**. Note 16's acceptance item 10 — blind listening must beat a
diversity-matched random control — was never exercised; the harness, corpus
and 36 anonymous candidate lists exist and the ratings remain unfilled. The
owner's first ask is results that make sense; that is a retrieval-quality
question before it is a UI question.

## 2. The shape of the plan

Five phases. Phase 0 is measurement and runs first because three later items
are superstition without it (the eligibility threshold, the vocabulary chips,
every duration in the copy). Phase 1 rebuilds the engine's selection so the
design's model becomes literally true, and states the invariants as tests.
Phase 2 builds the services the readouts need. Phase 3 is design 21 §13's
page, re-ordered onto the new ground. Phase 4 is the ship gate.

The dependency rule: **no readout is built before the engine fact it reads
exists, and no copy states a number before the measurement that backs it.**

```
Phase 0  measure ──┬─ 0.2 threshold ──► Phase 1 engine ──► Phase 2 services ──► Phase 3.4 readouts
                   ├─ 0.3 chips ─────────────────────────────────────────────► Phase 3.2 band
                   ├─ 0.4 rate ──────────────────────────────────────────────► Phase 3.6 first run copy
                   └─ 0.1 ballot ────────────────────────────────────────────► Phase 4 ship gate
Phase 3.1 panes (structural, no engine dependency — may start immediately)
```

## 3. Phase 0 — Measure before building

### 0.1 Fill the blind ballot — **waiting: the owner's ears**

The one item no agent can do. The harness (`tools/vibe-eval/`), the 72-track
consented corpus and the materialized `.m3u8` candidates under ignored
`local/` are ready; note 16 records that the deterministic random control was
added last, so **regenerate the ballot with all four systems present before
listening**. The scorer refuses an incomplete ballot by design.

*Acceptance:* every candidate rated on every request with the forced
preference recorded; `score` run; one paragraph in note 16 stating whether the
semantic system reliably beat the random control.

*If it does not win:* Phases 1–3 pause at the end of Phase 1 and the next
work is engine quality (window policy, hybrid conventional constraints), not
UI. This is note 16's own gate, restored.

### 0.2 Choose the eligibility policy, by sweep

Extend the harness with an `eligibility` probe: for each committed request,
embed the prompt, rank the corpus, and report the similarity distribution plus
what each candidate policy keeps — fixed cosine floor, top-K per cent, and a
largest-gap (elbow) cut. Score each policy against the corpus's genre/tag
labels as weak relevance judgements, and against the ballot's ratings once
0.1 lands. CLAP similarity distributions vary by phrase, which is exactly why
this is a measurement and not a taste call.

*Deliverable:* a table in `docs/design/impl/vibe-eligibility/README.md` and
one recommended policy with its failure modes stated. The same sweep fixes
the three tick-bucket boundaries (§5, item 1.5). **Waiting:** the owner signs
off the policy, because it decides what "matches 340 songs" means forever.

### 0.3 Score the twelve vocabulary chips

Design 21 §4's rule, executed: candidate chips per row (made-of / feels-like /
moves-like) are scored by the harness against the corpus — a chip earns its
place by measurably narrowing retrieval toward tracks carrying the matching
labels. `vibe-baseline`'s first customer, as backlog item 59 demands.
*Deliverable:* the twelve chips with their scores, same `impl/` directory.

### 0.4 Measure a real per-track analysis rate

Run analysis over a real several-thousand-track library (the owner's, in the
`baz-dev` toolbox, or a synthetic one decoded from real formats) and record
tracks/hour at the shipping four workers. Every duration in Phase 3.6's copy
— *"roughly two hours"* — cites this number. *Deliverable:* one row added to
`docs/design/impl/vibe-memory/README.md`.

## 4. Phase 1 — Engine: make §3's table true

All in `crates/baz-vibe`. The public API changes; `crates/baz/src/vibe.rs`
`generate()` is the one caller and moves in the same commit.

### 1.1 Two-stage selection

New first stage: `eligible(prompt_embedding, candidates) -> Pool`, applying
0.2's policy. The pool carries, per track, its similarity and its tick bucket.
Second stage: the walk runs **within the pool only**. Curve-fit, continuity
and the diversity rules (artist ≤ 2, never adjacent, fresh-album preference)
keep their jobs; relevance leaves the blended cost and becomes a within-pool
tiebreak. `Weights` loses its three-way trade — the comment explaining why
relevance and fit no longer trade against each other goes where the old
weights table sits, because that trade is what made a lullaby eligible for a
workout.

No prompt (shape-only request): the pool is the whole analysed library, count
shown as such. No shape: ordering is relevance + continuity within the pool,
as `select_semantic` behaves today.

### 1.2 The invariants, as tests

These are §3's table made executable, and they are the point of the phase:

- **I1 — the words let it in:** every chosen track is in `eligible(words)`.
- **I2 — determinism:** identical request + identical seed → identical list.
- **I3 — the line does not re-select the pool:** words fixed, curve moved →
  pool identical (count and cloud stable under dragging).
- **I4 — a small pool is honest:** when pool ≤ positions, moving the line
  changes order only, never membership — which is when "reorders rather than
  re-selects" must be literally true.
- **I5 — no padding:** result length never exceeds what the pool supports.

Property-style where cheap, fixture-based otherwise. They live beside the
walk and run in CI.

### 1.3 Axis relativity — **waiting: an owner decision**

Rank axes are currently built over the whole pool passed in
(`Axes::over(..., candidates)`), i.e. library-relative. Two coherent options:

- **Pool-relative (proposed):** *"how should it move"* means *how should the
  music you asked for move*. The line is always fillable; the axis words stay
  true within the request; the cannot-be-filled state nearly disappears. Cost:
  the same line means different absolute loudness for different phrases — but
  the axis was already collection-relative, never absolute.
- **Library-relative:** stable axis across requests; narrow phrases leave the
  top of the curve unreachable, handled by design 21's warned-twice state.

The cloud and the sentence under the curve are drawn from whichever wins, so
this lands before Phase 2.

### 1.4 Visible variation, deterministic compose

- Remove the freshness penalty from the compose path, and `create()` stops
  auto-incrementing the seed: **Compose with an unchanged request returns the
  identical list**, which is what makes the new/kept diff's causal sentence
  always true (I2).
- **Another version** stays a distinct, visible press (the code already has
  `another()`); it advances the seed and the diff names it as the cause:
  *"another version: same request, different draw — changed 7 of 18."*
- The `remember_offered` store and `RECENTLY_OFFERED_CAP` queue either go, or
  survive only inside Another version's draw; either way nothing invisible
  biases a compose. Migration note: the on-disk offered table becomes inert;
  `prepare()` stops returning it.

### 1.5 `Selection` carries the explanation

Per chosen track: similarity bucket (three ticks, boundaries from 0.2), lane
level, and position — so the why-line (*"position 4 of 20 — louder than 78%
of this request's pool, matched 'warm analogue soul'"*), the ticks and the
dot-pairing are readings of engine output, never a second opinion computed in
the view. Plus the pool size and, for the diff, enough to name causes:
previous pool size vs new.

### 1.6 The blend, weighted

The default lane becomes the weighted mean with energy dominant (design 21
§5), in `crates/baz/src/contour.rs` where the blend is a UI concept; seeding
each dimension's curve from the blend is verified by a test that sets every
lane to one curve and asserts the blend is that curve under any weights —
the "stays exactly consistent" claim, pinned so nobody simplifies it away.

## 5. Phase 2 — Services the readouts need

### 2.1 The live match count, and the text tower's residency

A debounced (~400 ms) embed-and-count against vectors already in memory.
The cost is the ~350 MiB text tower resident from page-open.
**Waiting: owner decision** — page-open (count live immediately) or first
keystroke/chip press (deferred cost); measure both against the
`vibe-memory` baseline and decide on numbers. `semantic.rs`'s
`with_model` global session is the touch point; the audio tower must not
become co-resident (note 16's boundary).

### 2.2 Closest matches, not just a count

Under the count, the **top three matched titles, live**. Free given 2.1 —
the ranking exists in memory — and it is the cheapest possible answer to
"does Baz understand my phrase": type *slow sparse piano*, see a death-metal
track first, and you know before composing. This readout is the plan's one
addition to design 21 §6, and it earns its place because a count says how
many, never how well.

### 2.3 The diff's memory

Keep the previous result and request snapshot in `Vibe` state; compare by
path; derive the causal sentence from what actually changed (words → pool
narrowed/widened with both counts; line → order; seed → another version;
nothing → *"identical, because nothing changed"*). Engine facts from 1.5,
sentence assembled in the view model, unit-tested per cause.

## 6. Phase 3 — The page

Design 21 §13's order, on the new ground. All in `crates/baz/src/views/`
(today's single-column `new_playlist.rs` `vibe_form` splits into an ask /
shape / result module set) and `crates/baz/src/vibe.rs`.

1. **The two panes and the measure.** Ask left at bounded width, result
   right; narrow stacks ask–curve–list with Compose pinned and a post-compose
   landing on the list; under 700 px height the curve collapses to sentence +
   presets. The row lane takes the maximum measure (design 20 §1's rule).
   *Structural; can start today, in parallel with Phases 0–1.*
2. **The one-question band.** Field at the head with *"this is exactly what
   Baz searches for"*; six starting points (replace, light goes out on edit,
   undo restores); twelve chips from 0.3 (append with a comma); a starting
   point sets shape and length only while untouched. The §4 rules table
   becomes the test list for this item.
3. **The shape control.** A pass, not a rewrite: blend lane from 1.6, axis
   labelled in words at whichever relativity 1.3 decided, the live sentence,
   presets as chips underneath, focus ring + arrow keys + doubled grab
   regions, the labelled expander seeded from the blend, and the `−`/`+`
   stepper deleted.
4. **The readouts.** Diff first (teaches the most, per design 21), then
   count + closest three, then the eligible cloud behind the line, then
   per-row ticks. Each is a rendering of Phase 1/2 facts — if a readout needs
   arithmetic the engine didn't supply, that's a Phase 1 gap, not a view
   workaround. The deliberate refusal stands: the list never updates while
   the line is dragged.
5. **The result.** Ordinary rows with reorder/remove/undo; the why-line from
   1.5 (three cues — enlarged dot, axis tick, position number — never a
   colour, per the standing rule); save-in-place with the proposed name;
   **Compose again** (deterministic, states what it replaces) beside
   **Another version** (the seed press, restored from quorum R8); the
   cannot-fill state warned before the press against the cloud and on the
   result in numbers, with *lower the line* as a control.
6. **The first run.** States 1–2: the ask pane fully live while listening,
   the commitment rewriting itself (`Compose · needs listening first` →
   `Compose from 1 240 so far`), pause/resume, durations from 0.4. **Gated
   on the consent decision** (§8, decision 1).
7. **The fork's removal.** One New playlist page — *start from a mood* /
   *start with an empty list* — last, so it lands on a page that is already
   right, and the strip sentence dies with its reason.

## 7. Phase 4 — The ship gate

- All Phase 1 invariants green in CI; the walk's behaviour documented where
  the old comment block sits.
- The 0.1 ballot verdict recorded, and favourable — or the release notes say
  in plain words what is known and not known about retrieval quality.
- `vibe-memory` measurements re-run: compose peak and the text-tower
  residency decision's real numbers beside the old baseline.
- The nine states of design 21 §7 each exercised in the real binary,
  headless, frames under `docs/design/impl/` with the isolation receipt
  (six-variable XDG isolation, as always).
- Accessibility: every readout carries its meaning in shape, position or
  count — the colour-blindness rule is a hard rule, checked state by state.
- `WORK.md` items moved to done in the landing commits; `CHANGELOG.md` says
  what shipped; design 21 gets a one-line header note pointing here for the
  amendments (eligible set now real; Another version restored; closest-three
  readout added).

## 8. The decisions, collected — all **waiting**

| # | decision | proposed |
|---|---|---|
| 1 | May baz listen to a library before it is asked to? (consent, battery) | no — offer on the page, state the cost |
| 2 | Does a mood compose immediately once there is something to compose from? | yes |
| 3 | Do per-dimension curves ship on day one? | yes, closed, seeded from the blend |
| 4 | Axis relativity: pool or library? (§4, 1.3) | pool-relative |
| 5 | Eligibility policy sign-off after the 0.2 sweep | whichever the sweep recommends |
| 6 | Text tower resident from page-open, or first press? (2.1) | decide on measured numbers |
| 7 | The ballot verdict's consequence if semantic does not beat random | pause UI, do engine quality |

Decisions 1–3 are design 21 §11 unchanged; 4–7 are new, surfaced by the
audit. None blocks Phase 0 or Phase 3.1 from starting today.
