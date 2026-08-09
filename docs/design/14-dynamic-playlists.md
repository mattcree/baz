# 14 — Dynamic playlists: a rule you can say out loud

> The owner, 2026-08-10, verbatim:
>
> *"we had documented intentions to create a dynamic set of playlists. the
> exact amount and nature wasn't settled. but we wanted to use existing
> technologies such as ML etc. -- this could be a good place to do this?
> and/or something which has some limited local NLP?"*
>
> *"This place"* is **Home** (`Place::Home`), which today holds two sections:
> `CONTINUE` and `RECENTLY ADDED`.
>
> A design study, not an implementation. Written 2026-08-10 against `0896502`
> (ADR-0030's third amendment: *the band stands whenever there is a run to
> carry on with and nothing is sounding*). It settles the two things the
> owner says were never settled — **the amount and the nature** — and it
> answers the two technology questions he asked by name, in numbers rather
> than in taste. Its conclusions are carried into
> [ADR-0033](../adr/0033-dynamic-playlists.md), which decides.
>
> Every claim about shipped behaviour is cited `file:line` or by module; every
> dependency is priced by a command anyone can re-run; every prior-art claim
> carries a source, **including the prior art that argues against this
> document**. §6 is the part to read if you read one part: it is the ML
> verdict, and it is not the reflexive no.
>
> The short version: **a dynamic playlist in baz is a rule over the library
> that draws an ordered list of records when a person presses it — and the
> rule, not the list, is the durable thing.** There are three of them, each
> stated in one sentence on the row you press, each computed from the play
> ledger by arithmetic that fits on a screen. **ML earns its place exactly
> once in this product and it is not here**: it belongs in the analysis
> pipeline, as a producer of *facts baz does not have*, never as a ranker over
> facts it does. **Local NLP's real prize is not a playlist at all** — it is a
> query well, and it is a bigger and better feature than this one, and it is
> the owner's to call.

---

## 0. What is already decided, and what this document may not touch

Five documents constrain this one. They are listed first so that nothing below
reads as a fresh idea when it is an inheritance.

| Source | The constraint | Where it bites |
|---|---|---|
| `docs/design/09-implicit-playlists.md` §2 | **baz has one kind of list.** Everything that plays is an ordered list of tracks; one of them is sounding and unnamed. | §2 — dynamic playlists add **no new type** |
| `09` §S10 (via ADR-0024 §7) | A generation's output is an ordinary `.m3u8`; it arrives **silent**; **generation is an act, never a condition**; the candidate pool is statable in a sentence | §4, §7 — these are already binding criteria, not proposals |
| `docs/REFUSALS.md` | *No auto-generated playlists* · *no invisible shuffle pools* · *no engagement stats* · *history records, it never performs* · *no cloud dependency* | §3.4, §7.1, §8 |
| `docs/adr/0030-…` §6 | Home's **honest inventory**: two facts, five refusals, and *"a third [band] needs an argument that beats the L8.6 test the other five failed"* | §7.2 — the argument is owed and §7.2 is it |
| `docs/BACKLOG.md` §"The strip demolition" | The owner, 2026-08-09: *"the pull option will just disappear, and so will the shuffle… The play all thing also does not need to exist. That should be existing as a kind of playlist that is implicit."* | §2.3, §7.4 — a **live instruction adjacent to this one**, reconciled rather than ignored |

And one that binds the author personally. `docs/REFUSALS.md`'s preamble:
*"The owner's decision is sufficient on its own; an entry he reverses gets
rewritten to say what was decided and why, and that is the whole of the
process."* The ledger currently refuses **the pull on the home surface**, on
the grounds that *"an unbidden offer is generation without a request"*. The
owner's ask reverses that. §7.1 rewrites the entry; it does not argue with
him, and §7.1's reasoning exists so that the reversal has a **shape** —
something a future contributor can be held to — not so that it has a defence.

---

## 1. What is actually on the shelf: the signal inventory

Everything below is a design over these facts and no others. It is worth being
brutal about it up front, because most of §6's answer is decided here rather
than in any argument about models.

### 1.1 The play ledger (ADR-0018, `crates/baz-core/src/history/`)

Append-only, tab-separated, one line per play. Five fields, and the format is
written into the file's own header:

```
started_utc  outcome  listened_ms  track_ms  path
```

Folded per track (`history/read.rs:140–159`), that yields exactly six numbers:
`plays`, `skips`, `first_played_unix_s`, `last_played_unix_s`,
`last_touched_unix_s`, `listened_ms`. `read.rs`'s module docs then draw a
fence around it — *"There is deliberately no fourth [question]… the way to not
build [charts] is to not provide the surface that makes them easy"*. **This
study does not breach that fence**: everything in §3 is built from the public
`History::track`, and the spike adds no method to `History` at all.

**The scale, measured on the owner's own machine, 2026-08-10.** This is the
number that decides §6, so it is quoted rather than estimated:

```
$ grep -vc '^#' ~/.local/share/baz/history.tsv
29
$ grep -v '^#' ~/.local/share/baz/history.tsv | cut -f5 | sort -u | wc -l
23
$ grep -v '^#' ~/.local/share/baz/history.tsv | cut -f2 | sort | uniq -c
     12 played
     17 skipped
```

Twenty-nine plays over twenty-three tracks, of which twelve met the threshold.
The library beside it is **37 files**. That is the whole behavioural corpus
baz has ever accumulated. It will grow — but it grows at the rate one person
listens to music, which is on the order of 10³ plays a year, and it is
**never** the rate a model wants.

### 1.2 The library index (`crates/baz-core/src/index.rs`, `library.rs:340`)

Per track, and every field optional because files are the source of truth:
`artist`, `album_artist`, `compilation`, `album`, `genre` (**verbatim** — no
normalisation, no mapping table, no splitting on `;`, by explicit decision),
`title`, `track`, `disc`, `year`, `duration`, `format`, `bit_depth`,
`sample_rate`, `bitrate`, `stamp`, `replay_gain`. Per album (`index.rs:1747`):
the artist key, title, year, first-declared genre, `first_seen_ns`, and the
ranked `editions`.

### 1.3 What baz does **not** have, and will not acquire

No network, ever. No last.fm, no MusicBrainz, no critic metadata, no
user-to-user anything. No audio embeddings and no audio features of any kind:
`baz-core`'s `analysis` module is the **ReplayGain measuring service**
(ADR-0015) and doc 09 §S10 already had to write a paragraph correcting people
who cite it as if it were a feature extractor. No ratings — baz has no star
control and refuses one. No "added to playlist" signal beyond the `.m3u8`
files themselves.

### 1.4 The one measurement baz has that nobody expects

`ReplayGainTags` and `Library::computed_replay_gain` hold **a loudness figure
per track** — read from tags or measured by ADR-0015's pass. This is the only
number in baz derived from the audio itself, and §8.3 shows exactly why it
does *not* answer the question everyone immediately asks it.

---

## 2. The model: a dynamic playlist is a rule, and it is not a new kind of thing

### 2.1 The definition

> **A dynamic playlist is a rule over the library that produces an ordered
> list of records when a person presses it. The rule is durable; the list is
> not.**

Read that against doc 09 §2's taxonomy and the claim it makes is small on
purpose. That table has seven rows — the album's track list, the wall in its
arrangement, a shuffle draw, the pull's offer, the queue, a saved playlist, a
generator's output. A dynamic playlist is **the third row with a different
author**: where a shuffle draw is written by chance from a pool you can see, a
dynamic draw is written by a *stated rule* from a pool the sentence describes.
Everything after the draw is identical — it reifies into the queue, it is
readable to its end, editable row by row, saveable by `Save as playlist`, and
it ends in silence.

So doc 09's model absorbs this feature without amendment: **baz still has one
kind of list.** No `DynamicPlaylist` type, no second playlist store, no place
of its own, no schema change. That is not a modest ambition; it is the reason
the feature can be built in a fortnight and understood in a sentence.

### 2.2 Why the rule is durable and the list is not

The alternative — store the result — is the design every other product picks,
and it is the one `docs/REFUSALS.md` refuses in six words: *generation is an
act, never a condition*. A stored, self-refreshing list has to answer *when
does it refresh*, and every answer is bad: on a timer (baz has no clock at
idle and ADR-0020 forbids one), on a file watch (a watcher, at rest, for a
list nobody is looking at), or on read (which is not storage at all, it is
this design with a cache in front of it).

Keeping the rule instead makes the awkward questions disappear rather than get
answered:

- *Does it change under me?* No. The list you drew is the list you have, in
  the queue, where you can see all of it.
- *Where is it stored?* Nowhere. Until you name it — at which point it is an
  ordinary `.m3u8` and the filename is the name (ADR-0024 §2), with no
  provenance link back and nothing that will ever rewrite it.
- *What is in it?* The sentence on the row said so before you pressed.

### 2.3 What this settles from the BACKLOG, which was the harder half

`docs/BACKLOG.md` §3 (*"`Play all` → an implicit playlist — the vocabulary
exists, the type does not"*) names two traps for exactly this feature, and the
rule-not-list model walks out of both:

> **Trap 1** — *"Giving the wall's run provenance immediately makes the picker
> offer `Add to "Everything"`, which has no file to write to."*

Dissolved. A draw carries **no provenance**. Provenance (doc 09 §6) is a
statement about a queue's *origin* and its only consumer is the picker's
`Add to "{name}"`, which needs a **file**. A dynamic draw has no file, so it
sets no provenance, and the picker offers nothing that cannot act — which is
doc 09 §6's own rule (*"a control that cannot act must not pretend it can"*)
being obeyed rather than special-cased. The listener who wants a file presses
`Save as playlist`, and from that instant it is a file like any other.

> **Trap 2** — *"The wall's order is not stored… An implicit list that is a
> place re-derives on every visit; a snapshot is the silently-re-deriving pool
> problem in a new coat."*

Dissolved by refusing the false choice. A dynamic list is **neither** a place
nor a stored snapshot: it is a *press*. It derives exactly once, at the
moment of the gesture, into the queue — which is a snapshot the listener is
looking at. `shuffle.rs`'s `Pool` doc says the same thing about the same
problem and is worth quoting because it was written for a different feature
and lands on this one: *"It is a record of a moment… A pool that silently
re-derived itself would be a pool you could not see."*

And the owner's *"the pull option will just disappear"* is honoured while its
best part is kept. §3.2's `NotForAYear` **is** `History::pull_weight`'s
arithmetic — days since last play — with the *control* deleted and the *fact*
promoted to a sentence. The strip loses a word it could not explain (doc 11 P9
is an open question to the owner reading *"`Pull`: explain it or rename it"*);
Home gains a row that explains itself. `pull_weight` and `PULL_NEVER_WEIGHT`
stop being dead code the moment the strip control goes.

---

## 3. The amount and the nature: three rules, closed

The owner says the amount and nature were never settled. This settles them.
The test each candidate had to pass is his own product's:

> **Could the listener state this rule in a sentence — and is the sentence
> true?** A row you cannot explain to yourself is the thing this design
> refuses (ADR-0030 §6's honest inventory, applied one level down).

A second test comes from the ledger: **does the signal exist?** §1 is short
enough that most candidates die here.

### 3.1 The set

| Row | The sentence it draws | Signal it needs | Exists? |
|---|---|---|---|
| **Never played** | *"12 records you have never played"* | ledger: any play of any track | ✔ `History::track` |
| **Not for a year** | *"34 records you have not played in over a year"* | ledger: most recent play per record | ✔ `pull_weight`'s own input |
| **Only heard part of** | *"9 records you have only heard part of"* | ledger + index: which tracks of the record have been played | ✔ both |

Three. The set is **closed**, and closed is the design: `Rule::ALL` is an array
of three in `crates/baz-core/src/dynamic.rs`, and a test asserts its length so
that a fourth arrives through this document's §4 rather than through a
plausible-looking pull request.

Two shipped properties do a lot of work here and are stated as rules:

- **A row is absent, not empty** — ADR-0030 §6's clause, one level down. No
  candidates, no row.
- **A row whose draw is the whole library is also absent.** On a freshly
  scanned collection everything is unplayed, so *Never played* would be a row
  reading *"your library"* — true, useless, and a door to the place one row
  above it in the lane. This is precisely the clause ADR-0030 §6 already gives
  `RECENTLY ADDED` (*absent* when every row came from one first scan).

### 3.2 Why these three, in one paragraph each

**Never played.** The album-collector's actual anxiety, and the only question
in the product whose answer is *records you paid for and have not met*. Its
sentence is unarguable and its signal is a presence test on a `HashMap`. A
skip is **not** a play here, exactly as it is not one for `History::recency`
(`read.rs:290–305`: *"starting a track and abandoning it is not having heard
it"*), and the alternative would quietly empty the row for a listener who
auditions.

**Not for a year.** The pull, re-homed (§2.3). The record's own last play is
the *most recent* play of any of its tracks across every edition — the same
judgement `shuffle::album_weight` and `shuffle::last_played` already make and
document (*"putting side A on this morning means you have heard this record
today"*, *"hearing the FLAC rip is hearing the record"*). Its order is
**longest unheard first**, which is not a ranking over the rule but the rule's
own axis: the single fact the sentence already named. A record never played
sorts first, which is `PULL_NEVER_WEIGHT`'s judgement (*one past the day cap*)
carried over verbatim.

**Only heard part of.** The one genuinely new question, and it is new because
it is the question only an album-first product thinks to ask: which records
did I start and never finish? A track-first library has no way to phrase it. It
needs one join the others do not — the ledger against the record's track list
— and it is still a statement about the *record* (this one is unfinished),
never about the listener, which is what keeps it the right side of *history
records, it never performs*.

### 3.3 What each one costs, measured

`crates/baz-core/src/dynamic.rs` is the whole implementation. Measured in
release on the development machine, over a synthetic **10 000-record library of
10 tracks each (100 000 tracks)** against a **30 000-play ledger** (1.8 MB):

| | Time |
|---|---:|
| Read the ledger from the file (once, at launch — already paid by ADR-0030 §4's lane) | **11.3 ms** |
| `NeverPlayed` whole-library draw (best of 5) | **3.31 ms** |
| `NotForAYear` whole-library draw | **3.28 ms** |
| `PartlyHeard` whole-library draw | **3.17 ms** |

Read against ADR-0020's cost argument and doc 04's fluidity contract, that
number says one thing precisely: **3.3 ms is a fifth of a 60 Hz frame, so this
may never run in one.** The surface rule that follows (§7.3) is *draw when the
page is entered and when a play lands, never per frame* — which is not a
concession, it is the discipline ADR-0030 §4 already imposed on the returns
lane (*"Read once, at launch… never a per-frame file read, and no watcher"*).

At the owner's actual scale — 37 files, 29 ledger lines — all three draws
together are microseconds and the paragraph above is theatre. It is written
for Marta's library because that is the library the product is judged by.

The three rules share their entire per-track loop, so a fused single pass
would make three rules cost about what one does. It is deliberately **not**
written that way: 10 ms once on a navigation is not a problem anybody has, and
an optimisation without a measurement is how you get code nobody can delete.

### 3.4 What was refused, and why — the honest half of the inventory

Each of these was a serious candidate. Each fails a named test.

| Candidate | Why not |
|---|---|
| **Recently played / "lately"** | The returns lane's entire subject (ADR-0030 §1: *things you have touched*). One fact drawn twice is doc 07 L8.6's test, and ADR-0030 §6 already refused it from the home surface by name. |
| **Recently added** | It is already `RECENTLY ADDED`, one section up the same page. |
| **"You keep skipping this"** | The ledger has `skips` and offers it deliberately (`read.rs:143–148`). A row built on it is an **accusation**, and it is the clearest possible case of history performing. `pull_weight` refuses skips for the same reason in its own doc: *"down-weighting what you skipped would make the pull start having opinions about your taste."* |
| **Most played / top artists / "your year"** | `docs/REFUSALS.md`, *No engagement stats*: *"No Wrapped, no streaks, no charts, no 'top artists of the year'."* Not close. |
| **By genre / by decade / "your 90s"** | Already free, twice over: the GENRE and YEAR group keys arrange the wall, and doc 09 §7.1 observes that this *"turns the group keys into programme builders for free"*. A Home row would be a second, worse route to a shipped one. |
| **"More like this"** | Requires a similarity metric over audio. baz has none (§1.3). This is VISION pillar 4 and §6.6 is where it goes. |
| **"Deep cuts" (album tracks that are not singles)** | Requires knowing what a single is. That is critic metadata, which is network, which is a different product. |
| **Anything blending two signals into a score** | ADR-0030 §1's rule for the lane, generalised: *"No score, no decay, no weighting, no blend."* A blended row cannot be stated in a true sentence, which is this section's whole test. |

---

## 4. The rule for adding a fourth

A closed set needs a stated door, or it is closed only until the next person
with a good idea. A fourth `Rule` needs **all four**:

1. **A sentence** that is true, in the room's voice, and that names its own
   pool size. If it needs a clause beginning *"based on"*, it has failed.
2. **A signal that exists**, cited to a field in §1 — not one that could be
   made to exist.
3. **A question no other surface answers.** The lane, `RECENTLY ADDED`,
   `CONTINUE`, the group keys and the search well between them cover a great
   deal; ADR-0030 §6's L8.6 test applies unchanged.
4. **A measured cost** in the shape of §3.3's table.

And one prohibition that is not negotiable by argument: a rule may not be a
*condition*. If it wants to run when nobody pressed it, it is refused.

---

## 5. The interaction, and what it costs the surface

### 5.1 The press

> **A row's press draws the rule into the queue and takes you to the Queue
> place. Nothing sounds.**

Three claims, each inherited rather than invented:

- **Nothing sounds** — doc 09 §S10's binding criterion (*"the artefact arrives
  silent, and its `Play` is the ordinary one"*), and mechanically it is
  already shipped: appending to an empty stopped engine loads the queue
  without starting it (`app.rs:1363–1366`, cited by doc 09 §8.1).
- **It goes to the Queue place** — because the pool must be visible *before*
  it plays. This is `docs/REFUSALS.md`'s no-invisible-pools rule met in the
  one way available to a surface that has no wall to dim: the list itself, on
  screen, all of it, in the place built for reading a run to its end.
- **The Queue place is already the right surface.** It has group headers,
  click-to-play-from-here, a per-row ✕, and `Save as playlist`
  (`views/queue.rs:154–196`). Doc 09 §8.2 is bringing it to full edit parity
  with the playlist page anyway. **This feature adds no surface at all.**

The one thing given up is *intent → sound in one press*. That is deliberate
and it is the right trade here: `Play all` and `Shuffle` are gestures whose
scope is on the screen you pressed them from, and a dynamic draw's scope is
not — the sentence describes it, the queue proves it. One extra press buys the
proof. (A listener who wants sound immediately has the queue's ordinary `Play`
under their pointer when they arrive.)

### 5.2 Saving one

`Save as playlist`, unchanged, and this is where ADR-0024's world takes over
completely: the file is written, the filename is the name (ADR-0024 §2), it
appears in the returns lane and the panel like any other list, and **nothing
ever rewrites it**. There is no link back to the rule, no "refresh from rule"
control, and no provenance comment — doc 09 §S10 permits provenance as *inert
comment lines* and this study declines even that for v1, because an inert
comment that names a rule is an invitation to build the control that re-runs
it.

The listener who wants the row's answer again presses the row again. That the
answer may differ is not a bug; it is the entire meaning of the sentence, which
is in the present tense.

### 5.3 What it costs the page

Home gains **one section rule and up to three rows**. Each row is one line of
text at the room's body measure and a count inside its own sentence — no
sleeve, no collage, no artwork, so none of `docs/REFUSALS.md`'s artwork
clauses is engaged. Nothing resident is added anywhere else; the strip, the
bar, the lane and the rail are untouched.

---

## 6. Does ML earn its place? — the verdict, in numbers

The owner asked for this by name, so it gets a real answer rather than a
reflex. The answer is **not here, and exactly once elsewhere** — and the
reasons are arithmetic, licensing and build-system facts rather than taste.

### 6.1 What a model would have to beat

`History::pull_weight` — six lines — and the three rules of §3. The bar is not
"is a model better in principle"; it is *what does a model buy over a
`HashMap` lookup and a comparison, for this user, on this data?*

### 6.2 The data regime, honestly

Recommender systems come in two families and baz has neither's input.

- **Collaborative filtering** learns from *many users*. baz has **one**, by
  constitution — no accounts, no telemetry, no network. With a single user
  there is no similar user, so matrix factorisation degenerates: the user
  factor is a constant and what remains is item popularity, which is the play
  count, which is a `u32` the ledger already stores and §3.4 refuses to
  display. The only collaborative signal available within one history is
  **item–item co-occurrence** — "these two records get played in the same
  session" — and that is a `HashMap<(u64, u64), u32>` built by one pass over
  the ledger. It is a real technique, it is not machine learning in any sense
  that would justify a dependency, and §6.7 keeps it as a live candidate.
- **Content-based models** learn from item features. baz's features are ~6
  categorical tag fields (§1.2). Any embedding of them is a one-hot vector,
  and k-means over one-hot genre/artist/year is — provably, not
  rhetorically — a **group-by**, which the GENRE and YEAR group keys already
  perform, instantly, and *explainably*. The cluster labelled `3` is worse
  than the heading `Post-Rock` in every way that matters to this product.

And the corpus size (§1.1) is 29 plays. Even at a mature 10 000 plays over a
5 000-record library, the interaction matrix is 99.9 % empty with one row.
This is not a small-data regime; it is a **one-row** regime.

### 6.3 The Rust landscape in 2026, priced

Every figure below is from a command run on 2026-08-10 against a scratch crate,
with baz's own `deny.toml` copied in. baz's baseline for comparison: **556
vendored crates** (`check-cargo-sources.py`'s own count; 558 packages in
`Cargo.lock` including baz's own two), a **32.6 MB** release binary, **no C++
anywhere in the graph** (`cc` appears, `cxx`/`esaxx` do not), and one C
dependency by deliberate choice (`libsqlite3-sys`, bundled).

| Candidate | Crates added (normal) | Clean release build | `cargo deny` | Verdict |
|---|---:|---:|---|---|
| [`ndarray`](https://lib.rs/crates/ndarray) alone | 6 | — | pass | fine, buys nothing on its own |
| [`linfa` + `linfa-clustering`](https://crates.io/crates/linfa-clustering) 0.8.1 | **52** | **13.2 s** | **pass** | cheap, pure Rust — and §6.2 says its k-means is a group-by |
| [`tract-onnx`](https://lib.rs/crates/tract-onnx) 0.23.4 | **111** | **67.0 s** | **pass** | the only credible ONNX runtime here; pure Rust, no system deps |
| [`candle-core`](https://starlog.is/articles/developer-tools/huggingface-candle/) 0.11 | **120** | not built | bans pass, licences pass | **pulls C and C++** — see below |
| [`ort`](https://github.com/pykeio/ort) 2.0.0-rc.13 | 14 | not built | **FAILS** | fatal — see below |

**`candle-core` drags a C and a C++ toolchain in.** Not through an optional
feature — through its ordinary dependency graph:

```
$ cargo tree -e normal -i onig_sys
onig_sys v69.9.3          # Oniguruma, C
└── onig → tokenizers v0.22.2 → candle-core v0.11.0
$ cargo tree -e normal -i esaxx-rs
esaxx-rs v0.1.10          # suffix arrays, C++
└── tokenizers v0.22.2 → candle-core v0.11.0
```

baz's own `Cargo.toml` states the property this breaks, in a comment about
symphonia: *"no feature here pulls a C library or a system dependency: the
whole decode path is pure Rust, and staying that way is a build-system
property worth keeping."* A tokenizer for text baz will never tokenise is a
poor price for it.

**`ort` fails the gate outright, today.** Its `download-binaries` feature is
**on by default** and fetches the ONNX Runtime binary from a CDN during the
build, which drags `ureq` → `native-tls` → `openssl-sys` into build
dependencies. `deny.toml` bans that crate by name. The command and its output,
verbatim:

```
$ cargo deny check bans
error[banned]: crate 'openssl-sys = 0.9.117' is explicitly banned
   ├ openssl-sys v0.9.117
     ├── native-tls v0.2.18
     │   └── ureq v3.4.0
     │       └── (build) ort-sys v2.0.0-rc.13
```

Two more things kill it independently, so this is not a matter of turning a
feature off. First, `packaging/flatpak/check-cargo-sources.py`'s own docstring
opens with *"A Flathub build has no network"* — a build script that downloads a
binary cannot be vendored with a checksum, because it is not a crate. Second,
`ort` has been at `2.0.0-rc.*` for years and is at `rc.13`; ADR-0003's
standards do not put a release candidate on the startup path of a music
player.

### 6.4 The model itself is the larger cost, and it is a network dependency

Suppose `tract-onnx` — the one option that survives §6.3. It runs a model; the
model is a file, and the file has to come from somewhere.

- **Downloading it at first run is a network dependency.** The brief says to
  treat it as one and it is right to. This is what the ecosystem does:
  [`fastembed`](https://docs.rs/fastembed/latest/fastembed/) downloads to
  `./.fastembed_cache` on first use; [`rust-bert`](https://github.com/guillaume-be/rust-bert)
  downloads to `~/.cache/.rustbert`. Both are refused here by
  `docs/REFUSALS.md`'s *No cloud dependency for anything that works on the
  user's own files*, and by VISION pillar 3.
- **Shipping it in the binary costs its own weight.** The lightest credible
  audio model in the literature baz already cites is
  [AudioMuse-AI-DCLAP](https://github.com/NeptuneHub/AudioMuse-AI-DCLAP)'s
  distilled CLAP audio tower at **~7 M parameters** — ~28 MB at fp32, ~7 MB
  quantised to int8. Against a **32.6 MB** binary that is between a 20 % and a
  near-doubling of the artifact, and Flathub carries every byte.
- **Session init is on somebody's critical path.** VISION pillar 2 promises
  *sub-second cold start*. A model that is loaded at launch spends that budget
  on a feature nobody has pressed; a model loaded on first press spends it in
  front of the listener. Neither is free, and the arithmetic in §3.3 shows
  what it would be competing with: 3.3 ms.

### 6.5 The cheap classical alternatives, named honestly

The brief asks for these to be priced rather than waved at, because *"existing
technologies"* includes them:

| Technique | What it needs | Verdict here |
|---|---|---|
| **Ledger arithmetic** (recency, coverage) | nothing new | **This is §3. It wins.** |
| **Item–item co-occurrence** over sessions | one pass over the ledger, a `HashMap` | Genuinely promising and genuinely cheap — but see §6.7 |
| **TF-IDF over tag strings** | the index | Answers *"which records share vocabulary"*. baz already answers a better version of that with the ARTIST and GENRE group keys, exactly and without a threshold |
| **k-means over hand-built numeric features** | `linfa`, 52 crates | The features would be year, duration, bitrate. Clustering a library by *bitrate* is a sentence nobody wants to read |
| **Collaborative filtering within one history** | many users | Does not exist (§6.2) |

### 6.6 Where ML *does* earn its place — and it is already on the roadmap

The honest positive answer, and the reason this section is not a reflexive no:

> **A model earns its place in baz exactly when it produces a fact baz does not
> have. It never earns its place ranking facts baz already has.**

There is precisely one such fact: **what the music sounds like.** That is
`VISION.md` pillar 4 and `docs/research/03-modern-features.md` §1 — bliss-rs
similarity, Essentia/ONNX mood heads, distilled CLAP — scoped to **v0.3**, and
already correctly shaped as a *background, incremental, whole-library
measuring pass* whose seam is the one `analysis` (ADR-0015) already occupies.
`bliss-audio` can even be built against symphonia rather than FFmpeg
(`--no-default-features --features=aubio-static,symphonia-all`, ~5–10 % slower
per its own README), which matters because baz already ships symphonia; the
`aubio-static` half is still a C library and is the thing to price when that
day comes.

And here is the part worth writing down, because it is the whole relationship
between the two features: **when that pass exists, a mood-steered dynamic
playlist is one more `Rule` variant and one more sentence.** The model is a
*feature extractor* producing a column; the rule is arithmetic over the
column; the sentence still has to be true. §3's architecture is what makes the
ML chapter cheap when it arrives — and building the ML chapter first would
have produced a surface with nowhere to land.

### 6.7 The one thing left genuinely open, and it is the owner's

**Item–item co-occurrence within a single listener's own history** is the only
technique in §6.5 that could add something §3 cannot: *"records you play in
the same sitting."* It is cheap (one pass, a `HashMap`), it is offline, it
adds no dependency, and at 29 plays it currently has nothing to say — which is
the point: it becomes interesting after a year of use and not before.

It is not in §3's set for one reason, and it is a reason about honesty rather
than cost: **the sentence is hard to keep true.** *"Records you tend to play
alongside this one"* is a claim about a co-occurrence threshold the listener
cannot see, which is §3.4's last row applied to the author's own favourite
idea. The owner may well want it anyway, and if he does, the sentence to argue
about is the deliverable, not the arithmetic.

---

## 7. Where it lives, and the refusal that has to be rewritten

### 7.1 The refusal the owner's ask reverses

`docs/REFUSALS.md` currently reads, under *Playback*:

> **No auto-generated playlists.** Every playlist is asked for by a person and
> owned by them thereafter. Refused: generation without a request, mutation
> without an edit, and any candidate pool the person cannot see.

And ADR-0030 §6 refuses from Home, by name: *"the pull (an act you press; an
unbidden offer is generation without a request)"*.

The owner's ask puts generated lists on Home. **Per the ledger's preamble this
is not argued and the entry is rewritten to record what was decided.** The
shape of the decision, so a contributor can be held to it:

> A row that **states its rule and does nothing until it is pressed** is not an
> unbidden offer. It is a door with its rule written on it. The request is the
> press; the pool is the sentence; the proof is the queue.

Every clause of the old entry survives that reading intact — *asked for by a
person* (the press), *owned thereafter* (`Save as playlist` and nothing else
ever writes it), *no mutation without an edit* (nothing rewrites anything),
*no pool the person cannot see* (the sentence names its size, the queue shows
its contents). What the old entry was written against — a list that already
exists, made for you, whose pool you cannot inspect — is untouched and still
refused. ADR-0033 carries the rewrite.

### 7.2 The argument ADR-0030 §6 demands for a third section

ADR-0030's *Deliberately not done* says: *"No second home band beyond the two
§6 admits; a third needs an argument that beats the L8.6 test the other five
failed."* L8.6 is *no two controls send the same message* — one fact drawn
twice. The five that failed: recently played and playlists (the lane's
content), the pull (an unbidden offer), and engagement statistics.

The argument, and it is short because the test is sharp:

| Surface | The fact it draws |
|---|---|
| The returns lane | what you have **touched** |
| `RECENTLY ADDED` | what has **arrived** |
| `CONTINUE` | where you **stopped** |
| **This section** | what you own and have **not heard** |

No surface in baz draws the fourth fact, and it is the only one of the four
that is about the *unvisited* part of a collection — which is the part an
album-first product exists to get you into. §3.4 shows the work: every
candidate that *would* have duplicated the lane was refused for exactly that
reason.

### 7.3 The responsiveness contract

Priced against ADR-0020 and doc 04 explicitly, in ADR-0030 §4's own table
shape:

| Cost | Answer |
|---|---|
| Idle CPU | **Zero.** No subscription, no clock, no watcher, no thread. A pure function of state the app already holds |
| Per frame | **Nothing.** The rows are three strings and three counts, computed elsewhere |
| When it computes | On entering Home, and on `TrackStarted` while Home is the place. Nowhere else |
| Worst measured case | **3.3 ms per rule** at 100 000 tracks / 30 000 plays (§3.3); ~10 ms for all three, once, on a navigation that is already a hard cut (ADR-0030 §3) |
| The ledger | Read once at launch, as ADR-0030 §4 already requires. This feature adds **no** file read |
| Startup | **Nothing.** Home is not the launch frame — `Place::Library` is (ADR-0030's first amendment) — so a cold start that goes straight to the wall never draws a rule |
| Binary size | **Zero bytes of dependency.** `dynamic.rs` is ~200 lines of `baz-core` and adds no crate to `Cargo.lock` |

### 7.4 The `Everything` row, specified and deferred

The owner's *"the play all thing… should be existing as a kind of playlist
that is implicit"* is answered by a fourth rule — *"All 412 records, in the
library's own order"* — which is trivially statable and trivially computed.
It is **specified here and not built**, for one measured reason that is not
about this feature at all: `views/queue.rs:70–133` draws every row in an
unvirtualized column, and doc 09 §7.1 already names this as `Play all`'s
implementation gate. A draw of 40 000 tracks into a surface that renders all of
them is a stalled frame, and a stalled frame is dead on arrival.

So: **`Everything` ships when the Queue place is virtualized, and not
before.** That is one gate, it is already on the books, and it is the honest
sequencing rather than a refusal.

### 7.5 Two questions this study will not answer for the owner

Both are flagged rather than decided, because both are his:

1. **Shuffle as a playlist-level toggle.** `docs/BACKLOG.md` §2 lists three
   questions only he can answer (what toggling *off* restores; whether
   provenance becomes a live link; how a pool stays visible on a surface that
   cannot dim covers) and names ADR-0024 §1's *no shuffle-on-play* clause as a
   direct blocker. A dynamic draw is not a way round any of that, and pretending
   otherwise would smuggle a mode in through this document.
2. **Whether the query well (§8) is what he actually wants.** It is a larger
   and, this study suspects, better feature. §8.5 puts the choice to him.

---

## 8. "Limited local NLP": what it can and cannot do here

### 8.1 The corpus

The only text baz has is **tags and filenames**: artist, album artist, album,
title, genre-as-tagged. Perhaps a few hundred thousand short strings, in many
languages, with no sentences in them. There is no prose anywhere in the
product. Whatever "NLP" means here, it does not mean reading.

### 8.2 What is genuinely extractable

- **Normalisation and equivalence.** `Post-Rock`, `post rock` and
  `Rock; Instrumental` are three genres to baz today, on purpose:
  `library.rs:376–392` states it as a decision — *"the library is a cache of
  what the files say, not a place we improve them"* — so the GENRE key shows a
  listener what their tags actually are and lets them fix it in their tagger.
  That decision stands. What is available *without* touching it is a
  **query-time equivalence**: case folding, Unicode normalisation, and
  splitting on `;` and `/` applied to the *question*, never written back to
  the index and never shown as a genre. Cheap, honest, and the single best
  NLP-shaped win available.
- **Fuzzy matching.** ADR-0021 already does the hard part and does it better
  than a fuzzy matcher would: six ordered fit tiers (`Exact`, `PrefixWord`,
  `Prefix`, `Word`, `WordStart`, `Fragment`), compared lexicographically, with
  *"no score, no arithmetic and nothing to tune… any two results can be
  explained by naming the first signal that separates them."* An edit-distance
  score would replace an explainable order with an unexplainable one. **Do not
  add fuzzy matching to baz's search.**
- **Field extraction from a typed question.** "nineties" → `year in
  1990..=1999`; "flac" → `format`; "long" → `duration`. This is parsing, and it
  is the §8.4 well.

### 8.3 What is not extractable, stated plainly

- **Mood from titles.** *Blue Monday* is not sad, *Happy House* is not happy,
  and *Sunday Bloody Sunday* is not about a weekend. Any lexicon over titles
  is a random number generator with a vocabulary.
- **Genre clustering into meaningful groups.** §6.2: over one-hot categorical
  tags this is a group-by, and the group-by already ships as a key.
- **"Quiet."** This is the interesting failure, because baz very nearly has
  it. `ReplayGainTags` and `Library::computed_replay_gain` hold a real loudness
  measurement (§1.4). But ReplayGain measures **the master, not the music** — a
  loud master of a gentle song reads loud, and a quiet transfer of a
  hardcore record reads quiet — and the sign is inverted from intuition
  (a *large positive* gain means a *quiet* recording). A row promising *quiet
  things* and delivering *quietly-mastered things* is precisely the snake oil
  `docs/REFUSALS.md` forbids: *"Nothing in the interface may claim an audio
  benefit the signal path cannot demonstrate."* **Quiet needs bliss-rs.** It is
  §6.6's chapter, not this one's.
- **Anything requiring world knowledge** — "shoegaze but not the noisy ones",
  "the ones with saxophone". Network, or nothing.

### 8.4 If a natural-language well is built, this is its shape

Not built here. Specified so the option is real:

- **It is a parser, not a model.** A few hundred lines over the fields in §1:
  artist, album, genre-as-tagged, year and decades, duration, format and bit
  depth, plus the ledger's recency and played/unplayed. No dependency, no
  weights, no download, no session, no startup cost.
- **The precedent is everywhere and it is well-understood.**
  [foobar2000's autoplaylist syntax](https://wiki.hydrogenaudio.org/index.php?title=Foobar2000%3AAutoplaylist)
  (`genre IS rock AND date GREATER 1989`), iTunes Smart Playlists, MusicBee's
  Advanced Auto-Playlist,
  [beets' `smartplaylist`](https://github.com/beetbox/beets/blob/v1.5.0/docs/plugins/smartplaylist.rst)
  (which writes `.m3u` files from queries — the closest thing to baz's own
  storage model), and
  [ListenBrainz's troi/LB Radio prompts](https://troi.readthedocs.io/en/latest/lb_radio.html).
  The well is these with the operators hidden behind ordinary words.
- **And the prior art argues against half of it, which is why it is cited.**
  Every one of those is an **auto-playlist**: a stored *condition* that
  re-evaluates. `docs/REFUSALS.md` refuses conditions. So baz may take the
  grammar and must refuse the lifecycle — the well draws once, into the queue,
  exactly as §5.1 does.
- **The governing rule, and it is the same rule as §3's.**

  > **The well echoes back the rule it understood, as a sentence, before it
  > draws — and it says what it did not understand rather than dropping it.**

  A well that silently ignores *"quiet"* and returns *"things from the
  nineties"* is a machine pretending to comprehend, and pretending to
  comprehend is the failure mode this whole product is arranged against. The
  sentence is not a nicety; it is the honesty mechanism, and it is why the
  well and the three rows are the same feature wearing two faces.

### 8.5 The recommendation, and the owner's call

The well is **probably the thing he actually wants**, and it is bigger: a
field, a grammar, an error-reporting story, a home on some surface, and a
mockup before a line of code. It is also **strictly downstream** of §3 —
because §3's `Rule`, `Rule::sentence` and `Rule::draw` are exactly what a
parser would need to produce, so the three rows are the well's back end
shipped early with three hard-coded queries.

That is the sequencing recommendation: **build §3, and let the well be its own
study and its own ADR.** Whether he wants the well *instead* — and would
rather have one field than three rows — is his to say, and §9 is written so
that answer costs nothing already spent.

---

## 9. The staged plan

Each stage is independently valuable and none waits on the one after it.

1. **`baz_core::dynamic`** — the `Rule` enum, `sentence`, `stands`, `draw`,
   and thirteen tests including `every_rule_states_itself_in_one_sentence`.
   No UI, no protocol message, no dependency, no persisted state. **Built with
   this study** (`crates/baz-core/src/dynamic.rs`) so that the argument above
   can be read as code.
2. **The three rows on Home**, under one section rule; press draws into the
   queue and navigates to the Queue place; nothing sounds. `REFUSALS.md` and
   ADR-0030 §6 amended. This is the smallest shipping thing and it is the
   whole feature at v1.
3. **`Everything`** as a fourth rule — gated on Queue-place virtualization
   (§7.4), which is its own step and already on the books.
4. **The query well** — its own study, its own ADR, owner's decision (§8.5).
5. **The analysis pass** (`VISION.md` pillar 4, v0.3) — and *then* mood rules
   are one more `Rule` variant over a column that finally exists (§6.6).

Deferred by name, so nobody has to rediscover it: any stored dynamic list, any
refresh control, any provenance link from a saved file back to its rule, any
row whose rule is a blend, and the shuffle-as-a-mode question, which is the
owner's (§7.5).

---

## 10. Summary

A dynamic playlist in baz is a **rule you can say out loud**. It draws when
pressed, into the queue, in silence, and the queue is where you read it,
edit it, play it or name it — which means the feature adds a section to one
page and not one new idea to the product. Three rules pass the sentence test
and the signal test: *never played*, *not for a year*, *only heard part of*.
The first two are `pull_weight`'s arithmetic promoted from a control nobody
could explain to a row that explains itself; the third is the question only an
album-first player thinks to ask.

**ML does not earn its place here, and the numbers rather than the taste are
what say so**: one user, 29 plays, six categorical fields, and a
`HashMap` lookup that answers in 3.3 ms at Marta's scale. `ort` fails
`cargo deny` today on a banned crate reached through a build script that wants
the network; `candle` drags C and C++ into a graph that has neither; `tract` is
clean but has nothing to run; `linfa`'s k-means over categorical tags is a
group-by that already ships as a group key. ML earns its place **exactly
once** — producing the one fact baz lacks, what the music sounds like — and
that is VISION pillar 4, on the roadmap, at v0.3, and this design is what
makes it cheap when it lands.

**Local NLP can do less than hoped over tags and more than expected over
questions.** Mood from titles is a fiction, "quiet" is a ReplayGain figure
that means something else, and genre clustering is a group-by. But a query
well — a parser, not a model, no dependency and no download — is real, and it
is probably the better feature. It is the same feature as this one seen from
the other end: this study's `Rule` is what a parser would emit, and both are
governed by one sentence, which is the sentence.
