# ADR-0033: Dynamic playlists — a rule you can say out loud

**Status**: proposed (2026-08-10) · extracts the decisions of
[`docs/design/14-dynamic-playlists.md`](../design/14-dynamic-playlists.md)
§2, §3, §5, §6 and §7 · **rewrites `docs/REFUSALS.md`'s
no-auto-generated-playlists entry** and **amends ADR-0030 §6's inventory and
its refusal of the pull from Home** · **answers `docs/BACKLOG.md` §3's
*"`Play all` → an implicit playlist"*** and closes doc 11 P9 (*"`Pull`:
explain it or rename it"*) by re-homing it · adds one `baz-core` module, one
section on one page, **no new place, no new surface, no new type, no protocol
message, no persisted state and no crate in `Cargo.lock`** · the owner's
brief, verbatim: *"we had documented intentions to create a dynamic set of
playlists. the exact amount and nature wasn't settled. but we wanted to use
existing technologies such as ML etc. -- this could be a good place to do
this? and/or something which has some limited local NLP?"*

## Context

The intention is documented — `docs/design/09-implicit-playlists.md` §2 lists
implicit playlists as a taxonomy and §S10 sets the ground rules any generator
inherits — and the type does not exist: `grep -rn "implicit playlist" crates/`
returns one comment. What was never settled is **how many, what they are
called, and what each one's rule is**. This record settles that, and answers
the two technology questions the owner asked by name.

Three things make this decidable now rather than at v0.3. The ledger has been
shipping since ADR-0018 and its `pull_weight` is already a weighting over it.
The Home place exists (ADR-0030) and has room for exactly the fact none of
baz's other surfaces carry. And `docs/BACKLOG.md` records a live instruction —
*"the pull option will just disappear… The play all thing also does not need
to exist. That should be existing as a kind of playlist that is implicit"* —
which is the same question wearing a different hat and is reconciled here
rather than left to collide later.

The bar this record is written to is the owner's own: *"hard rules to me are
mostly about responsiveness and a nice aesthetic"*. §6 is the responsiveness
contract and it is arithmetic.

## Decision

### 1. What a dynamic playlist is

> **A dynamic playlist is a rule over the library that produces an ordered
> list of records when a person presses it. The rule is durable; the list is
> not.**

It is **not a new kind of thing**. Doc 09 §2's taxonomy already holds it: it is
the shuffle-draw row with a different author — where a draw is written by
chance from a pool you can see, a dynamic draw is written by a stated rule from
the pool its sentence describes. Everything after the draw is unchanged: it
reifies into the queue, is readable to its end, editable row by row, saveable
by `Save as playlist`, and it ends in silence.

**baz still has one kind of list.** No `DynamicPlaylist` type, no second store,
no place of its own, no schema change.

### 2. The sentence rule

> **Every rule states itself in one sentence, and the sentence is what the
> listener presses.**

This is the whole governing constraint, and it is executable:
`Rule::sentence` *is* the row, and `every_rule_states_itself_in_one_sentence`
holds the set to it. A rule whose sentence cannot be written is a rule baz does
not get to have. If a sentence needs a clause beginning *"based on"*, it has
failed.

### 3. The set: three rules, closed

| Rule | The sentence | Order |
|---|---|---|
| `NeverPlayed` | *"12 records you have never played"* | library order |
| `NotForAYear` | *"34 records you have not played in over a year"* | longest unheard first |
| `PartlyHeard` | *"9 records you have only heard part of"* | library order |

`Rule::ALL` is an array of three and a test asserts its length, so a fourth
arrives through §4 rather than through a plausible pull request.

Two rules of presence, both ADR-0030 §6's *a section is absent, not empty*
applied one level down: **an empty draw draws no row**, and **a draw that is
the whole library draws no row either** (on a fresh scan, `NeverPlayed` would
otherwise be a row reading *"your library"* — which is the clause ADR-0030 §6
already gives `RECENTLY ADDED`).

`NotForAYear` **is `History::pull_weight`'s arithmetic** with the control
deleted and the fact promoted to a sentence. A skip is not a play, exactly as
it is not one for `History::recency`. A record's last play is the most recent
play of any of its tracks across every edition, which is
`shuffle::album_weight`'s judgement kept verbatim.

Refused, each against a named test, with the full table in the study's §3.4:
recently played and playlists (the lane's subject; L8.6), most-played and any
chart (`REFUSALS.md`'s engagement-stats entry), anything built on skips
(*history records, it never performs* — and `pull_weight`'s own doc refuses
skips for this reason), by-genre and by-decade (the group keys already are
these, better), "more like this" (needs a signal baz does not have — §7), and
**anything blending two signals into a score**.

### 4. How a fourth rule gets in

All four, or it does not: **a true sentence** in the room's voice naming its
own pool size; **a signal that exists**, cited to a field; **a question no
other surface answers** (ADR-0030 §6's L8.6 test); and **a measured cost** in
§6's shape. And one prohibition not negotiable by argument: **a rule may not be
a condition.** If it wants to run when nobody pressed it, it is refused.

### 5. The interaction

> **A row's press draws the rule into the queue and takes you to the Queue
> place. Nothing sounds.**

- **Nothing sounds** — doc 09 §S10's binding criterion; mechanically already
  shipped, because appending to an empty stopped engine loads the queue
  without starting it (`app.rs:1363–1366`).
- **It goes to the Queue place** because the pool must be visible before it
  plays. This is the no-invisible-pools rule met the only way a surface with no
  wall to dim can meet it: the list itself, all of it, in the place built for
  reading a run to its end.
- **No new surface.** The Queue place already has group headers,
  click-to-play-from-here, the per-row ✕ and `Save as playlist`.
- **No provenance.** A draw has no file, so it sets none, and the picker
  offers no `Add to "…"` that cannot act. This is `docs/BACKLOG.md` §3's first
  trap dissolved rather than handled.
- **Saving** is `Save as playlist`, unchanged: an ordinary `.m3u8`, filename is
  the name (ADR-0024 §2), no link back to the rule, no refresh control, and
  **not even an inert provenance comment** at v1 — a comment naming a rule is
  an invitation to build the control that re-runs it.

**The heading is `DRAWS`**, in Home's existing caps section rule. It is the
product's own word — doc 09 §S7 calls a shuffle result a draw, and
`docs/REFUSALS.md` already says *"a draw is a thing you start, never a thing
that starts itself"*, which is this section's governing rule written down by a
document that was not thinking about it. `FOR YOU` and `SUGGESTIONS` are
refused as claims this design does not make. This is the one decision here with
no argument strong enough to survive the owner disliking the word; if he wants
another, the word changes and nothing else does.

The friction budget's *intent → sound in one press* is deliberately not met
here, and that is the trade: `Play all` and `Shuffle` have their scope on the
screen you press them from and a dynamic draw does not. One extra press buys
the proof.

### 6. The responsiveness contract

| Cost | Answer |
|---|---|
| Idle CPU | **Zero.** No subscription, no clock, no watcher, no thread — a pure function of state the app already holds |
| Per frame | **Nothing.** Three strings and three counts, computed elsewhere |
| When it computes | On entering Home, and on `TrackStarted` while Home is the place. Nowhere else |
| Worst measured case | **3.3 ms per rule** at 100 000 tracks against a 30 000-play ledger, release, best of five; ~10 ms for all three, once, on a navigation that is already a hard cut |
| The ledger | Read once at launch, as ADR-0030 §4 already requires. **This adds no file read** |
| Startup | **Nothing.** `Place::Library` is the launch frame, so a cold start never draws a rule |
| Binary size | **Zero bytes of dependency.** ~200 lines of `baz-core`; `Cargo.lock` unchanged |

**A rule may never be evaluated in a frame.** 3.3 ms is a fifth of one.

### 7. ML: the verdict

> **A model earns its place in baz exactly when it produces a fact baz does
> not have. It never earns its place ranking facts baz already has.**

**Not here.** The regime is one user (no accounts, no network, by
constitution), 29 plays on the owner's machine today, and six categorical tag
fields. Collaborative filtering needs many users and degenerates to play count
with one; k-means over one-hot categorical tags is a group-by, and the group-by
already ships as a group key, instantly and explainably.

Priced 2026-08-10 against baz's 556 vendored crates, 32.6 MB binary and C++-free
graph, with `deny.toml` copied into each scratch crate:

| Candidate | Crates | Clean release build | `cargo deny` | Verdict |
|---|---:|---:|---|---|
| `linfa` + `linfa-clustering` 0.8.1 | 52 | 13.2 s | pass | cheap; its k-means is a group-by |
| `tract-onnx` 0.23.4 | 111 | 67.0 s | pass | clean, pure Rust — and nothing to run |
| `candle-core` 0.11 | 120 | — | pass | **pulls C (`onig_sys`) and C++ (`esaxx-rs`) via a non-optional `tokenizers`** |
| `ort` 2.0.0-rc.13 | 14 | — | **FAILS** | `download-binaries` is a default feature; `ureq` → `native-tls` → **`openssl-sys`, banned by name in `deny.toml`** |

Three independent facts finish `ort`: the ban above; that
`packaging/flatpak/check-cargo-sources.py` exists because *"a Flathub build has
no network"*, and a build script that downloads a binary cannot be vendored
with a checksum; and that it has been a release candidate for years.

**A model download at first run is a network dependency** and is refused
(`REFUSALS.md`, VISION pillar 3) — which is what `fastembed` and `rust-bert`
both do. **A shipped model is bytes**: the lightest credible audio model baz's
own research cites (distilled CLAP, ~7 M params) is ~7 MB int8 to ~28 MB fp32
against a 32.6 MB binary.

**Where ML does earn its place**: the one fact baz lacks is *what the music
sounds like*, and that is `VISION.md` pillar 4 at v0.3 — a background,
incremental measuring pass in the shape `analysis` (ADR-0015) already has.
**When it exists, a mood-steered dynamic playlist is one more `Rule` variant
and one more sentence.** This decision is what makes that chapter cheap.

### 8. Local NLP: what it may and may not do

**May**: query-time equivalence over genre strings (case folding, Unicode
normalisation, splitting on `;` and `/`) — applied to the *question*, never
written back to the index, because `library.rs`'s verbatim-genre decision
stands. Field extraction from a typed question. **May not**: mood from titles
(*Blue Monday* is not sad); genre clustering (a group-by); and **"quiet"**,
which looks answerable from `ReplayGainTags` and is not — ReplayGain measures
the master, not the music, and a row promising quiet things while delivering
quietly-mastered things is the snake oil `REFUSALS.md` forbids. Quiet needs
§7's analysis pass.

**Fuzzy matching is refused** for baz's search: ADR-0021's six ordered fit
tiers are explainable by construction (*"no score, no arithmetic and nothing to
tune"*) and an edit distance would replace that with a number nobody can read.

**If a query well is built it is a parser, not a model** — no dependency, no
download, no session, no startup cost — and it inherits one rule:

> **The well echoes back the rule it understood, as a sentence, before it
> draws — and it says what it did not understand rather than dropping it.**

It is **its own study and its own ADR**, and whether it is wanted *instead of*
§3's rows is the owner's call. It is strictly downstream: `Rule` is what a
parser would emit, so §3 is the well's back end shipped early with three
hard-coded queries.

### 9. `docs/REFUSALS.md` is rewritten, not argued with

The ledger refused *generation without a request*, and ADR-0030 §6 refused the
pull from Home as *an unbidden offer*. The owner's ask puts generated lists on
Home; **his decision is sufficient on its own** (the ledger's preamble), so the
entries are rewritten to record what was decided. The shape of it, so a
contributor can be held to something:

> A row that **states its rule and does nothing until it is pressed** is not an
> unbidden offer. It is a door with its rule written on it. The request is the
> press; the pool is the sentence; the proof is the queue.

Every clause of the old entry survives that reading: *asked for by a person*
(the press), *owned thereafter* (`Save as playlist`, and nothing else ever
writes it), *no mutation without an edit* (nothing rewrites anything), *no pool
the person cannot see* (the sentence names its size, the queue shows its
contents). What the entry was written against — a list that already exists,
made for you, whose pool you cannot inspect — is untouched and still refused.

### 10. The argument ADR-0030 §6 demands for a third section

ADR-0030 required that a third home section beat the L8.6 test the other five
failed. Each surface, and the one fact it draws:

| Surface | The fact |
|---|---|
| The returns lane | what you have **touched** |
| `RECENTLY ADDED` | what has **arrived** |
| `CONTINUE` | where you **stopped** |
| **This section** | what you own and have **not heard** |

No surface in baz draws the fourth, and it is the only one about the
*unvisited* part of a collection — which is the part an album-first product
exists to get you into. Every candidate that would have duplicated the lane was
refused for exactly that reason (§3).

## Consequences

- **One new `baz-core` module**, `dynamic`, ~200 lines, pure, no I/O, no
  dependency. `Cargo.lock` is unchanged and `cargo deny` is untouched.
- **The ledger's three-surface rule is not breached.** `history/read.rs`'s
  *"There is deliberately no fourth"* stands: everything here is built on the
  public `History::track`, and no method is added to `History`.
- **`pull_weight` and `PULL_NEVER_WEIGHT` stop being dead code** the moment the
  strip's `Pull` goes (`docs/BACKLOG.md` §1 lists them as becoming dead). Doc
  11 P9's open question — *explain `Pull` or rename it* — is closed by
  re-homing it as a sentence.
- **`docs/REFUSALS.md` gains a rewritten entry** (§9) and ADR-0030 §6 gains an
  amendment (§10).
- **Home gains one section rule and up to three rows.** No sleeve, no collage,
  no artwork, so no artwork clause is engaged. The strip, the bar, the lane and
  the rail are untouched.
- **A merge note**: `feat/home-now-playing` is reworking `CONTINUE`'s
  predicate. Nothing here touches it — the new section is additive and its
  presence rule is its own.

## Deliberately not done

- **No `Everything` row yet.** Specified (*"All 412 records, in the library's
  own order"*) and **gated on Queue-place virtualization**: `views/queue.rs`
  draws every row in an unvirtualized column, and doc 09 §7.1 already names
  this as `Play all`'s implementation gate. A 40 000-row draw into it is a
  stalled frame.
- **No stored dynamic list, no refresh control, no provenance link** from a
  saved `.m3u8` back to its rule.
- **No dependency of any kind**, and no model, shipped or downloaded.
- **No query well.** Its own study, its own ADR (§8).
- **No answer to shuffle-as-a-mode.** `docs/BACKLOG.md` §2 lists three
  questions only the owner can answer, and names ADR-0024 §1's
  *no shuffle-on-play* clause as a direct blocker. A dynamic draw is not a way
  round any of it.
- **No co-occurrence rule.** *"Records you play in the same sitting"* is cheap,
  offline and dependency-free, and it is left out for an honesty reason rather
  than a cost one: its sentence rests on a threshold the listener cannot see.
  The study's §6.7 puts it to the owner as the one open technique.

## Open questions for the owner

Both are flagged rather than decided, because both are his:

1. **Is the query well what you actually want** — one field instead of three
   rows? It is bigger, probably better, and strictly downstream of this
   record, so nothing here is wasted either way (study §8.5).
2. **Do you want the co-occurrence rule** despite its sentence being hard to
   keep true (study §6.7)?
