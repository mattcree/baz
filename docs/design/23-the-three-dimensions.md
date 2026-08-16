# 23 — Three dimensions, and whether the first one earns its place

**Nothing here proposes a change.** The owner, 2026-08-16: *"don't take a
direct course based on what I've said here. this needs to be thought about. it
needs to be pored over. this is a hard problem to solve without this feature
just being a complete joke and gimmick."* This is the poring.

---

## 1. The framing, accepted

His words: *"we have three basic dimensions i.e. which songs are in the
initial set, the line is the contour or flow of the energy essentially (and
this can be broken down further into different subdimensions) and the length
of the experience."*

That is design 21 §3's table said better, and the page now states it in a line.
It is the right frame. But it hides an asymmetry that his own next question
lands on, and that nothing in the design has ever said out loud:

| dimension | rests on | can it be wrong? |
|---|---|---|
| **which songs** | a neural model's opinion about the meaning of a phrase | **yes, and silently** |
| **the flow** | tempo, loudness, loudness variance, spectral centroid, rolloff, zero crossings, flatness | no — these are measurements |
| **how long** | arithmetic over durations | no |

Two of the three are **measurement**. One is **inference**. They are drawn on
the same page in the same voice, and the interface gives no sign which is
which.

## 2. What the first step actually delivers, per request

`docs/design/impl/vibe-eligibility/` reports a mean genre lift of **1.94×** for
the shipping policy. **That average conceals the finding.** Read per request,
over the owner's real 5 076-track library:

| request | share of library carrying the expected genre | share of the pool | lift |
|---|---|---|---|
| `calm-piano` | 13.3% | 52% | **3.94×** |
| `bright-rock` | 33.5% | 77% | **2.29×** |
| `focus` | 13.9% | 29% | 2.10× |
| `industrial` | 6.9% | 13% | 1.84× |
| `dream-pop` | 11.1% | 11% | **0.99× — chance** |
| `gentle-jazz` | 10.3% | 4.7% | **0.46× — worse than chance** |

Two work well, two are mediocre, and **two are at or below random selection**.
A mean of 1.94 is not a description of this; it is a way of not looking at it.

The caveat, stated because it cuts both ways: genre is a **weak judge**. The
library tags a lot of jazz-adjacent music `Vocal`, `Easy Listening` and
`Retrospective Pop`, so `gentle-jazz`'s 0.46 may be a labelling artefact
rather than a retrieval failure. But `dream-pop` at 0.99 against a broad
`pop / alternative / indie` label is harder to explain away.

**And nothing here answers the real question.** Every number is comparative —
which policy concentrates labels better than another. Whether the model's idea
of *warm hypnotic music for driving at night* is anyone's has never been
tested, and plan 22 §0.1's blind ballot against a diversity-matched random
control is still unfilled.

## 3. What the model demonstrably *can* hear

The same sweep scored 27 candidate words on whether pressing one moves a real
request's pool toward that word's meaning:

- **Instruments and texture move it.** `acoustic guitar` 0.142, `synthesizers`
  0.092, `piano` 0.091, `strings` 0.078 — and the last three concentrate the
  matching genre by **3.5–4.1×**, independent corroboration from a different
  measurement.
- **Moods do not.** Best 0.099, median 0.026, several at or below zero, while
  *displacing* the pool by 0.45–0.83. Pressing `dreamy` changes a great deal
  and steers almost nothing.

This is the sharpest thing we know, and it maps exactly onto the owner's
instinct: *"I like the idea of having a way to specify stuff like instruments,
etc."* The evidence says instruments are the part that works.

**Which is also the answer to the other half of his question** — *"I wonder if
the other stuff can at least be understood better?"* Probably not, and the
reason is uncomfortable: an interface can only explain something that is
happening. If pressing `dreamy` does not reliably steer the result, no amount
of copy, readout or diagram makes it comprehensible — it would be explaining a
mechanism that is not operating. **Better explanation cannot rescue a control
that does not work; it can only disguise it more expensively.**

## 4. The gimmick test

What would make this a gimmick:

1. **The output is indistinguishable from shuffle with a shape.** Unknown, and
   only the blind ballot answers it.
2. **The interface claims precision it does not have.** *"Matches 211 songs of
   the 5 076 Baz has heard"* is a confident, specific sentence over a selection
   that for two of six test requests was no better than chance. This is the
   one we are currently failing, and it is the worst kind, because it is a
   dishonesty rather than a limitation.
3. **A listener cannot predict what a change will do.** True of the words —
   the cloud thins, the count moves, and the direction is not reliable.

What makes it real, today:

- **The flow half is provable and now proves itself.** The rank axes are
  measurements, collection-relative by construction, and since 2026-08-16
  every row states what it is — `loud · fast · swinging`. Draw a rising line,
  read the list downward, watch the words travel. That is a claim anybody can
  check by ear.

So: **the feature's credibility rests on the half that is measurement, and is
put at risk by the half that is inference.**

## 5. The page's hierarchy is backwards

Simple mode leads with the prose field. It is the primary control, at the head,
in the largest voice. The curve is in the *advanced* pane.

That is precisely inverted relative to what can be trusted. The page gives
pride of place to the dimension that is sometimes no better than chance, and
files the one that is verifiable under a tab.

Nobody decided this. It follows from design 21 §2's *"there is one request, and
it is a sentence about sound"* — which was a good answer to a **different**
problem (mood and words reading as two inputs) and quietly became a claim about
importance.

## 6. Three ways it could go

**A — keep all three, make the first honest.** Stop asserting precision:
replace the count with something weaker, lead with the closest-three, mark
free text as best-effort. *Cheapest. Does not make the retrieval better, only
less overclaiming.*

**B — narrow the first step to what the model hears.** Instruments and texture
as the primary vocabulary, because they measure 3.5–4.1×; free prose demoted
to an escape hatch that says it is a guess. *Follows the evidence. Loses
"warm hypnotic music for driving at night" as a headline capability — which
may be the capability that sells the feature.*

**C — drop semantic retrieval; make "which" a set of facts.** *(Ruled out by
§7a: a tag filter does not reproduce what the model does, and half the
requests have no tag to filter on.)* Genre, year,
artist, folder, not-played-recently — all from tags and the ledger, all true.
The line then shapes that set. *Fully explicable, zero inference. Deletes the
350 MiB text tower and most of the per-track analysis cost. Becomes
rule-based playlists (item 66) with a contour over them, which is a
respectable feature — and a different one.*

A synthesis worth considering: **split "which" into facts and sound**, told
apart on screen by how much they can be trusted. Facts (genre, year, unplayed)
are certainties. Sound (instruments) is a good guess. Prose is a long shot,
and says so.

## 7. What would settle it

Two experiments. One needs the owner; one does not.

1. **The blind ballot** (plan 22 §0.1). Harness, consented corpus, four
   systems including a diversity-matched random control, 36 anonymous
   candidate lists — all ready, ratings unfilled. If semantic retrieval does
   not beat random, **option C wins on evidence** and the argument is over.
2. **Semantic against a genre filter** — *runnable today, no ears needed.* For
   each committed request, compare the CLAP pool against the dumb baseline
   *"tracks whose genre tag contains the obvious word"*. If a tag filter
   matches or beats the model on the requests where a tag exists, that is a
   strong argument for C on the cheap; if the model clearly wins on the
   requests tags cannot express, that is the case for A or B. Roughly an hour
   of work on the existing `vibe-eligibility` harness.

Doing (2) first is the obvious order: it is free, it is decisive in one
direction, and it sharpens what (1) would need to show.

## 7a. The free experiment, run — and it settles one thing

Ran 2026-08-16 on the owner's 5 076-track library. For each request with an
obvious genre word, the model's pool against *"tracks whose genre tag already
contains that word"*:

| request | model pool | tag set | overlap | of the tag set, found | of the pool, beyond tags | lift |
|---|---|---|---|---|---|---|
| `calm-piano` | 179 | 661 | 0.12 | 14% | 48% | 3.94 |
| `bright-rock` | 199 | 1 661 | 0.09 | 9% | 24% | 2.29 |
| `focus` | 226 | 689 | 0.08 | 9% | 71% | 2.10 |
| `industrial` | 410 | 340 | 0.07 | 15% | 88% | 1.84 |
| `dream-pop` | 185 | 548 | 0.03 | 4% | 89% | 0.99 |
| `gentle-jazz` | 319 | 511 | 0.02 | 3% | 95% | 0.46 |

**The model is not a tag filter in disguise.** The sets barely overlap
anywhere. Option C's implied hope — that a genre filter would reproduce the
model for free — is wrong, and drops out of the running as a straight
replacement.

What it is doing instead, where it works, is **selecting**. `calm-piano`
returns 179 tracks of which 93 are tagged classical or new age; the tag filter
returns 661 undifferentiated ones. Half the model's pool is on-genre and it
has chosen a fifth of the candidates — which is the behaviour wanted, because
*calm piano instrumental* is a much narrower request than *classical*.

Where it fails it does not fail towards tags, it wanders: `gentle-jazz` and
`dream-pop` put 89–95% of their pool outside the tag set **and** score at or
below chance, which is the signature of a pool assembled on something other
than what was asked for.

**And half the committed requests cannot be a tag filter at all.** Six of
twelve — *wistful but not tragic*, *dark tense music*, *tension gradually
clearing into warmth*, *a patient nocturnal electronic journey* — have no
genre word to filter on. That is the model's unique territory and it is half
the request set, which is the strongest argument for keeping it.

**So the decision this note was opened to make: keep the semantic step.** Not
because it is proven — two of six are still at chance — but because the
cheapest alternative demonstrably does not do its job, and the job is half of
what the feature is for. What remains true is §4's charge: the *count*
overclaims, and that is fixable today without waiting for anybody's ears.

## 7b. Resolved — the hierarchy, 2026-08-16

§5's charge was acted on the day this note was read. The owner: *"if we treat
words as just a kind of filter… the curves make more sense up front I think."*
The line is the page's question now and stands at both depths; the words are a
filter column that says it is optional. See
`docs/design/25-the-line-leads.md`, which also records what this note's
evidence does **not** support: prose was demoted, not deleted, because §7a
ruled out the cheapest replacement and half the committed requests have no
genre word to filter on.

§4's other charge — the count overclaiming — was fixed the same day; the
readout says *drew … to choose from* and no longer says *match*.

What remains open is §7 experiment 1, the blind ballot, which needs ears.

## 8. What this note does not do

It does not choose. The evidence supports *narrowing* the first step and
*demoting* prose; it does not yet support deleting semantic retrieval, because
the one measurement that would justify that has not been taken. Nor does it
support the status quo, which claims a precision two of six test requests did
not have.

The one thing that seems safe to say before any experiment: **the count is
overclaiming and should stop**, because that is a statement the interface
makes about its own confidence, and we know it is not warranted.
