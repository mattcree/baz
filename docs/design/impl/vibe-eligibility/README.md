# What "eligible" means, measured — plan 22 items 0.2 and 0.3

Design 21 §3 says the words decide **which** songs are eligible. The shipped
engine draws no such line: selection is one blended cost over every analysed
track. Before a match count, an eligible cloud or a why-line can be honest,
the line has to be drawn somewhere, and plan 22 §0.2 says that is a sweep
rather than a taste call.

This is the sweep. It is also, in one respect, a correction to design 21: the
vocabulary row that document is most confident about is the one the numbers
refuse.

## How it was measured

`crates/baz-vibe/src/bin/vibe-eligibility.rs`, run against a **real 5 076-track
analysed library** — not a fixture, and not the 72-track consented corpus,
which is too small for a distribution to have a shape. Each track's semantic
vector already sits in the analysis store; each track's genre comes from baz's
own library database and is used only as a **weak relevance judgement**.

```sh
# both databases are copies; nothing here opens baz's live data directory
toolbox run -c baz-dev ./target/release/vibe-eligibility \
  <copy-of>/vibe.db <copy-of>/library.db \
  tools/vibe-eval/requests.json sweep.json
```

18 prompts: the 12 committed requests plus design 21 §4's six starting points.
5 076 analysed tracks, 4 958 of them carrying a genre. Raw output in
`sweep.json`.

**What a genre can and cannot judge.** Six of the eighteen prompts get an
expected-genre set, and only where a genre is a defensible proxy — *gentle
acoustic jazz trio* has one, *wistful but not tragic, like remembering
somewhere fondly* does not. The other twelve are measured for their
distributions and excluded from the label scoring, because averaging in a
number that is partly meaningless would hide exactly the thing being decided.

## Finding 1 — the distribution moves wholesale with the phrase

This is the whole reason a fixed cosine floor cannot work, and it is larger
than expected:

| request | mean | sd | p99 | max |
|---|---|---|---|---|
| `start:workout` | **+0.331** | 0.194 | 0.619 | 0.675 |
| `bright-rock` | +0.246 | 0.164 | 0.505 | 0.559 |
| `gentle-jazz` | +0.167 | 0.106 | 0.414 | 0.464 |
| `slow-burn` | +0.059 | 0.099 | 0.318 | 0.405 |
| `industrial` | **−0.089** | 0.093 | 0.173 | **0.216** |

The whole library is further from *aggressive industrial percussion* than the
**mean** track is from *fast loud driving music with a hard pulse*. A floor at
0.20 therefore keeps 3 749 tracks for one phrase and **one** for the other. The
sweep says so directly:

| policy | mean kept | variation | smallest | largest | mean lift | unfillable |
|---|---|---|---|---|---|---|
| floor 0.10 | 3 562 | 0.31 | 242 | 4 578 | 1.41 | 0 |
| floor 0.20 | 2 345 | 0.47 | **1** | 3 749 | 1.24 | 1 |
| floor 0.30 | 1 173 | 0.75 | **0** | 3 167 | 1.37 | 1 |

*Lift* is the share of the kept set carrying an expected genre, over that
genre's share of the whole library. 1.0 is no concentration at all.
*Unfillable* counts prompts where the policy kept fewer than 24 tracks.

**A fixed cosine floor is out.** Not marginal — it produces empty pools.

## Finding 2 — a distribution-relative floor is out too, and less obviously

The natural repair is to make the floor relative to the request's own
distribution: keep everything *k* standard deviations above this phrase's mean.
It fails for the opposite reason.

| policy | mean kept | variation | smallest | largest | mean lift | unfillable |
|---|---|---|---|---|---|---|
| mean+1.0sd | 779 | 0.10 | 593 | 882 | 1.77 | 0 |
| mean+1.5sd | 290 | 0.38 | 40 | 437 | 1.85 | 0 |
| mean+2.0sd | 101 | 0.71 | **1** | 248 | 1.97 | 3 |
| mean+2.5sd | 41 | 1.55 | **1** | 216 | 2.13 | **11** |

The cosine distributions are left-skewed with short right tails. For
`bright-rock`, `start:workout` and `start:party` the *maximum* similarity in
the entire library sits under two standard deviations above the mean, so
mean+2.0sd keeps a single track. Eleven of eighteen prompts are unfillable at
2.5sd. A policy that empties the pool for the three most ordinary requests on
the page is not a policy.

## Finding 3 — largest-gap is a trap, and the first sweep fell into it

Plan 22 §0.2 names "largest-gap (elbow)" as a candidate. Implemented literally
— walk the ranked similarities, cut at the biggest single step — it pinned
**every one of the eighteen prompts to the smallest pool it was allowed**. The
reason is arithmetic rather than musical: a decaying curve's largest single
step is almost always at its head, so "largest gap" reliably answers "the
top", whatever the phrase.

The rule that asks the intended question is the **knee**: the ranked curve's
furthest point below the chord joining the two ends of the search window. It is
invariant to how steep the head happens to be. That is what `elbow_cut` does
now, and the difference between the two runs is the whole finding.

## Finding 4 — the recommendation

| policy | mean kept | variation | smallest | largest | mean lift | unfillable |
|---|---|---|---|---|---|---|
| top 0.5% | 25 | 0.00 | 25 | 25 | 2.01 | 0 |
| top 1.0% | 51 | 0.00 | 51 | 51 | **2.29** | 0 |
| top 2.0% | 102 | 0.00 | 102 | 102 | 2.17 | 0 |
| top 5.0% | 254 | 0.00 | 254 | 254 | 1.94 | 0 |
| **knee** | **235** | **0.29** | **146** | **410** | **1.94** | **0** |

Read the last two rows together, because they are the decision:

**At matched pool size, the knee's relevance is identical to top-K per cent's
— 1.94 against 1.94, at 235 tracks against 254 — and it is the only policy
whose size responds to the phrase at all.** Top-K per cent's variation is zero
by construction: it would answer *"matches 254 songs"* for every phrase anybody
ever typed, which makes design 21 §6's live count a decoration that cannot
teach what it exists to teach. The knee keeps 146 for *warm intimate acoustic
music* and 410 for *aggressive industrial percussion*, and those two numbers
are the readout doing its job.

Top 1.0% has the higher headline lift (2.29), and that is what a smaller set
should score: at 51 tracks it is a quarter the size. It is not a fair
comparison and it is not offered as one.

**Recommended policy — the knee, floored and bounded:**

- rank every analysed track by cosine against the request embedding;
- search the first **25%** of that ranking; cut at the point furthest below
  the chord joining its two ends;
- never cut above **24** tracks, so a pool always has room for a playlist and
  its diversity rules;
- with **fewer than 96 analysed tracks**, or **no words at all**, the pool is
  everything analysed, and the count says so.

### Its failure modes, stated

1. **A pool is never proof of relevance.** The knee always finds a bend, so a
   phrase the tower understands nothing of still yields ~200 songs that are
   merely the least-bad. Nothing in the count can distinguish that case. This
   is precisely why plan 22 §2.2 adds the closest-three readout: a count says
   how many, never how well, and the three titles are the cheapest possible
   answer to *does baz understand my phrase*.
2. **The weak judgements are weak.** `gentle-jazz` scores a lift near zero
   under **every** policy, including the ones that plainly retrieve jazz — the
   library tags those records `Vocal`, `Easy Listening` and `Retrospective
   Pop`. That is a limit of genre-as-judgement, not a finding about retrieval,
   and it is why item 0.1's blind listening remains the gate it is.
3. **Small libraries have no distribution.** Under ~96 analysed tracks the
   floor and the horizon decide the answer between them, so the policy
   degrades to *everything analysed* by rule rather than by accident.

## Finding 5 — the tick buckets

Plan 22 §1.5 wants three ticks of match strength per row, with boundaries from
this sweep. Absolute boundaries are unusable for the same reason a fixed floor
is: at any pair of fixed cosines, every row of a `start:workout` list shows
three ticks and every row of an `industrial` list shows one.

**Recommended: the pool's own terciles, computed per request.** A tick then
reads as *strongly / moderately / weakly matched for this request*, which is
the only claim the numbers support. The boundaries this produces are in
`sweep.json` per prompt; they range from `[0.079, 0.173]` for `industrial` to
`[0.484, 0.529]` for `calm-piano`, which is the point.

## Finding 6 — the vocabulary, and the row that did not earn its place

Design 21 §4 asks for twelve chips in three rows — *made of*, *feels like*,
*moves like* — each appending to the request with a comma, and says the twelve
are chosen by measurement rather than by taste. Twenty-seven candidates were
scored on three numbers:

- **discrimination** — how far the chip's own best 2% sits above the library
  mean, in library standard deviations. A word the tower has no opinion about
  scores near zero.
- **pull** — how much appending the chip to a real request moves that
  request's pool *towards the chip's own meaning*, averaged over the five
  starting-point phrases. This is the number that matters: it is what a
  vocabulary is for.
- **displacement** — how much appending it changes the pool at all.

Every one of the twenty-seven discriminates (2.0–3.3 sd). Every one displaces
heavily (0.45–0.83). **Pull is where they separate, and it separates them by
row:**

| row | best pull | median pull | at or below zero |
|---|---|---|---|
| made of | **0.142** | 0.061 | 1 of 9 |
| feels like | 0.099 | 0.026 | 0 of 9 |
| moves like | **0.046** | 0.009 | 2 of 9 |

The finding is uncomfortable and worth stating plainly: **high displacement
with near-zero pull means appending an adjective to a five-word request
scrambles the embedding rather than steering it.** The pool changes a great
deal; it does not change towards what the listener pressed. Instrumentation
words survive this — they name something the audio tower can hear — and the
*moves like* row does not: its best chip pulls 0.046, two of its nine are at or
below zero, and it duplicates the question the curve already asks in the
control directly beneath it.

**Recommended: twelve chips in two rows, not three.**

| made of | pull | lift | | feels like | pull |
|---|---|---|---|---|---|
| acoustic guitar | 0.142 | 1.53 | | hopeful | 0.099 |
| synthesizers | 0.092 | **4.06** | | warm | 0.061 |
| piano | 0.091 | **3.57** | | dark | 0.048 |
| strings | 0.078 | **3.66** | | melancholy | 0.033 |
| electric guitars | 0.061 | 1.09 | | dreamy | 0.026 |
| female vocals | 0.025 | — | | tense | 0.022 |

The three strongest *made of* chips also concentrate genre by 3.5–4.1×, which
is independent corroboration from a different measurement: the pull metric and
the genre lift agree about which words the tower actually hears.

*Moves like* is dropped, and this document is where that is recorded rather
than it quietly not appearing on the page. If it is wanted back, the thing to
change is not the chip list: it is that movement words should steer the
**curve** — press *driving*, get a shape — rather than be appended to a
sentence that then means something else.

## What this decides, and what it does not

**Decides** — plan 22 §8 decision 5 (the eligibility policy), the tick-bucket
boundaries, and design 21 §4's twelve chips.

**Does not decide** — whether the retrieval is any good. Every number here is
relative: which policy concentrates labels *better than another*. Item 0.1's
blind listening against a diversity-matched random control is still the only
thing that answers *does this work*, it still needs the owner's ears, and
finding 4's first failure mode is the reason it cannot be skipped.
