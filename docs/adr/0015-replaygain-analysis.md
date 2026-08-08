# ADR-0015: ReplayGain analysis — measure it ourselves, against the standard's own numbers

**Status**: accepted (2026-08-08) · **completes the unit [ADR-0013](0013-replaygain.md) explicitly deferred** ("Compute the tags… is a separate unit and stays in `docs/BACKLOG.md`"); every decision in ADR-0013 stands, and what changes is that a second *source* of figures now feeds the selection rule it states · schema v6, following the v2–v5 discipline of [ADR-0007](0007-album-editions.md), [ADR-0008](0008-album-artist-grouping.md), [ADR-0010](0010-incremental-scanning-and-removal.md) and ADR-0013 · advances the `v0.2 "it respects"` line in `docs/VISION.md`

> **ADR number**: 0014 was the highest in `docs/adr/` when this was written, so
> this took **0015**. A parallel unit is writing an ADR at the same time; if it
> also landed on 0015, the later of the two renumbers.

## Context

ADR-0013 split "ReplayGain" into two units and shipped the first:

> 1. **Honour the tags files already carry.** […] Using them is a parser, a
>    selection rule and one multiply.
> 2. **Compute the tags.** An EBU R128 analysis pass over every track in a
>    library: a loudness meter, a true-peak meter, a scan UI, progress and
>    cancellation, tag *writing* […] and validation against the EBU test
>    vectors that `docs/ENGINEERING.md` names.

**This ADR is the second**, minus tag writing — see "What this deliberately
does not do".

The gap it closes is not a small one. Karl (P4 in `docs/research/05-personas.md`)
expects a player to be *able* to compute the figures; Marta's 40 000 files were
not all ripped by a tool that wrote them. Until now baz's answer to an
unscanned library was ADR-0013's `no_tag` pre-amp of 0 dB — honest, and
honestly nothing. A listener with a partly-tagged library got normalisation on
some tracks and not others, which is the one arrangement worse than none at
all, because it is *audible as inconsistency* rather than as a feature nobody
switched on.

## Decision

### 1. The measurement is EBU R128 / ITU-R BS.1770-4 gated integrated loudness

`baz_core::loudness` implements it: K-weight each channel, mean-square over
400 ms blocks overlapping by 75 %, sum the channels at their BS.1770 weights,
drop blocks below the **absolute** gate of −70 LUFS, drop blocks more than
**10 LU** below the mean of what remains, and report the loudness of the mean
of the survivors. The gain is `−18 LUFS − measured`, ReplayGain 2.0's
reference — the same five decibels from EBU R128's −23 that ADR-0013's
`R128_REFERENCE_OFFSET_CENTIDB` already adds when reading an `R128_*` tag, so
the two directions cannot drift.

**The filter is derived, not tabulated.** BS.1770-4 publishes the K-weighting
coefficients at 48 kHz only, and a music library is not at 48 kHz. baz builds
both sections from the stage parameters the standard's design is expressed in
and evaluates them at the file's own rate.

### 2. Verification is the deliverable, and here are the numbers

Three independent checks, none of which compares the code against a recording
of its own output (`docs/ENGINEERING.md`: *"tests are written to
specification, not to implementation"*):

**a. The coefficients, against the standard's published table.**
`the_k_weighting_coefficients_match_the_published_table` derives the filter at
48 kHz and asserts all ten coefficients against BS.1770-4's Tables 1 and 2 to
within **1e-12**. They match to the last published digit.

**b. The EBU Tech 3341 compliance signals, at the tolerance the specification
states (±0.1 LU).** `tests/loudness.rs` generates each signal from the
specification's description and measures it. Every case is run at 48 kHz *and*
44.1 kHz, because 44.1 is the rate the standard does **not** tabulate and so is
the rate where a wrong derivation would show:

| EBU Tech 3341 case | target | baz @ 48 kHz | baz @ 44.1 kHz | worst error |
|---|---|---|---|---|
| 1 — 1 kHz sine, −23 dBFS, 20 s | −23.0 LUFS | −22.9933 | −22.9905 | 0.0095 LU |
| 2 — 1 kHz sine, −33 dBFS, 20 s | −33.0 LUFS | −32.9933 | −32.9905 | 0.0095 LU |
| 3 — −36 / −23 / −36 dBFS (relative gate) | −23.0 LUFS | −23.0139 | −23.0111 | 0.0139 LU |
| 4 — −72 / −23 / −72 dBFS (absolute gate) | −23.0 LUFS | −23.0150 | −23.0122 | 0.0150 LU |
| 5 — −26 / −20 / −26 dBFS (no gating) | −23.0 LUFS | −22.9787 | −22.9759 | 0.0241 LU |

**Worst error over all ten measurements: 0.0241 LU, against a permitted
0.1 LU** — a factor of four inside the specification's own tolerance.

Case 6 is 5.0-channel material and is **absent for a stated reason**: baz's
decode path is stereo everywhere (`playback::CHANNELS`), so no five-channel
signal can reach this meter and a test for one would be testing a capability
the player does not have. Cases 7–9 pin the momentary and short-term meters,
which this unit does not implement (§8).

**c. End-to-end, through real decodes.** `tests/analysis.rs` builds WAV
fixtures whose loudness is worked out in the test's own comments before the
code is asked — including the album figure, whose value (−15.60 LUFS from
three tracks at −23, −33 and −13 LUFS) is derived from the gate arithmetic in
the fixture's doc comment and is deliberately **not** the mean of the track
gains, so the fixture can tell a pooled gate from an average.

### 3. The dependency question: implemented here, and here is the costing

The obvious candidate is the [`ebur128`](https://crates.io/crates/ebur128)
crate — a pure-Rust port of libebur128 by a GStreamer maintainer. It was
checked rather than assumed:

| | |
|---|---|
| Licence | **MIT** — on `deny.toml`'s allowlist, no change needed |
| Maintenance | 0.1.10, October 2024; 175 000 recent downloads; actively used by the GStreamer Rust plugins |
| Direct deps | `bitflags ^1.0`, `dasp_frame ^0.11`, `dasp_sample ^0.11`, `smallvec ^1.0` |
| **New** crates in *this* lock | **two**: `ebur128` and `dasp_frame`. `bitflags 1.3.2`, `dasp_sample 0.11` and `smallvec 1.15` are already in `Cargo.lock` (core-graphics, cpal, and half a dozen others) |
| C / build-system cost | **none by default.** `cc` is a build dependency of the optional `capi`/`c-tests` features only, which baz would not enable — so the zero-system-dependency property `Cargo.toml` guards would survive |

So the crate is *cheap* and it is *good*. It was declined anyway, and the
reason is not "two crates":

1. **Verification is mandatory either way, and verification is what this
   project trusts.** Whichever implementation ships, `tests/loudness.rs` has to
   exist — `docs/ENGINEERING.md` names the EBU vectors as the acceptance
   criterion. The crate's marginal safety over 200 lines pinned by the same
   vectors *plus the standard's own coefficient table* is real but small, and
   `ENGINEERING.md`'s AI-trust policy is explicit that provenance is not part
   of the trust calculation — the gates are.
2. **baz would use about a seventh of it.** The crate is a streaming,
   arbitrary-channel meter with momentary, short-term, integrated, loudness
   range, peak and true-peak modes, a histogram history, mode flags and a C
   API. baz needs the integrated figure over one or two channels of `f32` it
   has already decoded. `dasp_frame`/`dasp_sample` exist to make it generic
   over sample formats baz does not have.
3. **The album figure needs the blocks, not the answer.** An album gain is the
   gated loudness of its tracks' blocks *pooled* (§5). Owning the meter means
   `Loudness` can carry its blocks and `album_lufs` can pool them in three
   lines; through the crate it means keeping a live meter state per track of an
   album and calling the multi-state API, which is more coupling to somebody
   else's lifetime model than the three lines are worth.
4. **A reader can check it.** `loudness.rs` puts the filter derivation, the
   gate and the standard's published constants where a skeptic can read them
   against BS.1770-4 in one sitting — which is the property
   `docs/ENGINEERING.md`'s closing paragraph asks for.

**What would reverse this**: needing BS.1770-4 Annex 2 true peak, a momentary
or short-term meter, loudness range, or more than two channels. Any one of
those makes the crate's generality the point rather than the cost, and the
decision should be re-made rather than defended.

`cargo deny check` is unaffected: **zero new dependencies**.

### 4. Sample peak, not true peak — and it is labelled

The `REPLAYGAIN_*_PEAK` convention is a **linear sample peak**. It is what
ReplayGain 2.0 scanners write, and it is exactly what ADR-0013 §4's clipping
rule consumes. So a sample peak is the figure this unit needs, and the one it
produces.

Inter-sample overshoot after reconstruction is not modelled — which is what
ADR-0013 already said of the tag reader, and it stays true here rather than
being quietly upgraded. True peak means BS.1770-4 Annex 2's four-times
oversampling filter *and* its own compliance vectors; shipping it without the
second would be exactly the unverified number this ADR exists to rule out.
`docs/BACKLOG.md` carries it.

### 5. Where the analysis runs: its own service, its own connection, album by album

`baz_core::analysis` is a **separate service from the playback engine**, and
that is a decision rather than a convenience. ADR-0013 §7 gives the engine
*paths and nothing else*, which is what makes a queue the library has never
seen play at the right level; the analyser is the mirror image, owning a
library and decoding audio **nobody hears**. One service would have given the
engine a database and the analyser a sink.

- **A worker thread**, spawned once, idle until told. Playback and the UI are
  untouched: it is one more thread decoding one more file.
- **Its own `Library` on the same database.** SQLite in WAL mode (which
  `Library::open` already selects) is built for one writer and concurrent
  readers. The second in-RAM index is not waste — the album grouping it
  computes **is** the plan.
- **It cannot fight the incremental scanner, structurally.** A scan writes the
  v5 tag columns; a pass writes the v6 computed columns; `UPSERT_TRACK` names
  the first group and not the second. The property is held by the schema, not
  by two writers agreeing to be careful.
- **The unit of work is the album edition** (ADR-0007/0008: album artist +
  title, split by codec), committed in one transaction. An album gain is a
  property of a *set*, so the set is what is measured and what is written.

**Cancellation** is an atomic flag checked once per decoded block plus a drain
of the command channel, so it lands within milliseconds. **Resumption is not a
separate mechanism**: a cancelled pass keeps every track figure it had
committed, and a later `StartReplayGainAnalysis` re-plans against what the
index now holds. Editions that completed are skipped entirely; the edition that
was interrupted is measured again.

**Why the resume granularity is the edition, stated plainly.** An album's
figure needs its tracks' 400 ms blocks as one set, and those blocks are not
something to keep in a database (a ten-hour set is 288 KB of them). A track
figure survives a cancel; an *album* figure of an interrupted edition does not,
and its edition is re-measured. Editions are usually ten tracks, so the worst
case a cancel wastes is nine decodes.

### 6. What is skipped, and what a partly-tagged edition does

- **A track whose file carries the figure is not measured.** Tags win in the
  selection rule (§7), so measuring one would spend a decode to produce a
  number nothing would use. `redo` does not change this: baz does not write to
  music files and will not measure what a tag already answers.
- **A track baz has already measured is not measured again**, unless the file
  changed or `redo` was asked for.
- **An edition needing an album figure is measured *whole*** — including its
  already-tagged tracks. An album gain computed from the subset that happened
  to be untagged would be a different number, and a wrong one. So a
  partly-tagged edition costs a full decode of the album and yields: computed
  figures for every track, an album figure across all of them, and *tags still
  winning per track for whichever tracks have them*.
- **An edition where every track has everything is skipped entirely**, which
  after one completed pass is every edition — so re-running the pass over an
  unchanged library costs a plan and nothing else.
- **A track that fails to decode is counted, not fatal** (the scanner's
  philosophy). The album figure is still computed from the tracks that *did*
  decode: a file baz cannot decode is a file baz cannot play, so it is not part
  of the album as anybody will hear it.

**Mono is measured as mono.** baz's decoder duplicates a mono file into both
channels, and BS.1770 sums channels at unity weight — so measuring the
decoder's output would read **3.01 LU loud** and normalise every mono track
three decibels too quiet *relative to what every other scanner writes*. The
pass builds the meter with `AudioSource::channels()`, and
`a_mono_source_is_measured_as_one_channel` pins the exact offset.

### 7. Computed figures are distinguishable from tagged ones, from the storage layer up

Three separate mechanisms, because one would not have been enough:

1. **Separate columns** (§9). The two claims coexist; neither overwrites the
   other.
2. **A stamp.** A computed figure is a claim about a file's *samples*, so it
   stops being true when the file changes. `ComputedReplayGain` carries the
   `FileStamp` it was measured at, and `figures_for` returns nothing when that
   no longer matches what the index holds. ADR-0010's rule applies unchanged:
   `None` is never a claim of freshness.
3. **The `ReplayGainSource` vocabulary**, which is where a listener's question
   is actually answered. It gains `ComputedTrack`, `ComputedAlbum` and
   `ComputedTrackFallback` beside `Track`, `Album` and `TrackFallback`, plus
   `ReplayGainSource::is_computed()` so a front end asks the property rather
   than enumerating variants.

`NoTag` **keeps its name and its wire spelling** and now means "neither the
file's tags nor an analysis provides a figure". Renaming it would have broken a
protocol a front end already reads, to restate a fact the three new variants
already carry.

**The selection rule: tags win, field by field.** `resolve_with(tags, computed)`
prefers a value the *file* carries over one baz measured, for each figure the
mode needs. `resolve(tags)` is exactly `resolve_with(tags, empty)` — asserted,
not assumed — so ADR-0013's whole table is unchanged for a file with no
measurement. Three reasons, in order of weight:

1. **The tag is what the listener's other software will use.** A track 0.3 dB
   different in baz than in foobar2000 is a difference nobody asked for.
2. **The tag may encode a decision** — a different reference, a hand edit. A
   statement about how a file should be played outranks a statement about what
   is in it.
3. **It makes the pass safe to run.** Analysing a library can never change how
   an already-tagged track sounds.

Field by field rather than whole-set, because the two are not alternatives: a
file with a track gain and no album gain takes the measured album figure for
album mode without disturbing track mode. The **peak** follows the gain's
origin, then the other origin — so a tagged gain with no tagged peak is
clip-checked against the peak baz measured, which is a strict improvement on
ADR-0013's "apply it in full, and say so".

### 8. How a measured figure reaches playback: a seam, not a command

`EngineHandle::set_computed_gains(Option<Arc<dyn ComputedGains>>)`. The engine
consults it at a track boundary, on the control thread, alongside the tags it
already lifts out of the file — **never on the pump path**, which reads the one
number `SharedVolume` publishes exactly as it did before.

It is a method and not a `Command` because the payload is a whole library's
worth of figures: a `SetComputedGains { … }` carrying forty thousand paths is a
message nobody could send twice, and an incremental one would be a second copy
of the index kept in sync by hand. `Library::computed_gains()` builds the
snapshot (fresh figures only), and a front end swaps it in at start-up and
again when `ReplayGainAnalysisFinished` arrives.

A trait rather than a concrete map, for ADR-0011 §7's reason: the branch is
reachable by a test double **today**, so the engine's half of the arrangement
is tested before a front end wires the real one in.

**The new figures take effect at the next track boundary**, deliberately: a
gain that changed under a track already playing would be a level change nobody
asked for.

### 9. Schema v6, six columns, no backfill

`rg_computed_track_gain_centidb`, `rg_computed_track_peak_micro`,
`rg_computed_album_gain_centidb`, `rg_computed_album_peak_micro`,
`rg_computed_mtime_ns`, `rg_computed_file_size` — `ALTER TABLE` inside one
transaction with the `user_version` bump, v2–v5's discipline exactly, so an
interrupted upgrade leaves a v5 database the next open migrates again.

`NULL` for every existing row, and the only honest value — for a *stronger*
reason than v5's. A computed loudness is not derivable from anything in the
database at all: it is the output of decoding every sample of every file, which
is minutes to hours of work and is precisely what the background pass exists to
do somewhere other than inside a migration. v2's backfill had a file extension
to read; there is not even a tag to read here.

`NULL` is self-healing, with a different healer from v2–v5's: the first
analysis a listener asks for fills it. Until they ask, nothing changes —
`NoTag` and the no-ReplayGain pre-amp (zero by default) are what apply, so the
upgrade cannot alter what anything sounds like.

Proof, not assurance: `a_v5_database_migrates_in_place_without_losing_anything`
builds a genuine v5 database from the v5 schema and v5 `INSERT`s with **no baz
code involved** — the double rip, the RODIK soundtrack, a real
`Various Artists` tag, non-ASCII paths and titles, real file stamps, and
ReplayGain tags on the FLAC edition only — migrates it by opening it, and
asserts that every column survives byte for byte, that `user_version` really
moved to 6, that the new columns are NULL, that the v4 stamps still make the
next scan incremental, and that grouping is *identical* to before the upgrade.
Paths are written with the `stored_path_bytes` helper, so the fixture is a
genuine database on Windows as well as on Unix.

## What this deliberately does not do

- **It does not write to music files.** baz never has. The figures live in
  baz's index, which means another player will not see them — stated plainly
  rather than implied. Tag writing is its own unit: it needs a backup story, a
  dry run, and a decision about what to do when a file is read-only or on a
  network share that lies about it.
- **No true peak** (§4), no momentary or short-term meter, no loudness range.
  ReplayGain needs the integrated figure; the others are a meter's features,
  not a normaliser's.
- **No UI.** The protocol, the meter, the pass, the schema and the readout are
  here; the control is a parallel unit, exactly as ADR-0011's slider and
  ADR-0013's mode selector were. What a front end needs is below.
- **No `Album`/`Edition` field carrying the computed figure.** The shelf does
  not need one, and adding a field to `TrackMeta` would have said that a *scan*
  produces a measurement, which it does not.

## Consequences

- **A library that has never seen a scanner can be normalised**, which is the
  point. A library that has is unaffected until a figure is missing.
- **`TrackMeta` is unchanged.** A measurement is not something reading a file's
  tags yields, so it does not live in the type that means "what reading the
  tags yielded". It lives beside it in the index, which is also what makes the
  scanner structurally unable to destroy one.
- **A file the filesystem will not stamp is re-measured on every pass**, as it
  is re-read on every scan. ADR-0010's residue, inherited rather than
  re-argued.
- **A front end's own `Library` does not see a pass's results until it
  reloads.** `Library::reload()` exists for exactly that, and the moment to
  call it is `ReplayGainAnalysisFinished`.
- **Two connections to one database.** WAL makes it legal and the column split
  makes it safe, but it is two connections, and the second hydrates a full
  in-RAM index. On a 100k library that is real memory for the duration of the
  service. Accepted, because the index *is* the plan; a lighter read-only
  accessor is a refinement, not a correction.
- **An analysis pass makes no sound.** There is no sink in `analysis`, and
  there is no path by which one could appear.
- **Album gain is only as good as the album grouping.** A mis-tagged album that
  the shelf splits in two gets two album figures. That is the same
  consequence ADR-0007 and ADR-0008 already carry, now audible.
- **Measuring is not free.** A pass decodes every file it measures, at whatever
  the decoder runs at; it is bounded by decode throughput and it is why the
  thing is cancellable and resumable at all.

## What a front end needs

- **Spawn** `baz_core::analysis::spawn(db_path)` once, keeping the
  `AnalysisHandle` and the `Receiver<Event>`. Dropping the handle cancels any
  running pass and joins the worker.
- **Send** `AnalysisCommand::StartReplayGainAnalysis { redo }` to *that handle*
  (not the engine's — they are different services and the type says so).
  `redo: false` measures what has no figure yet; `redo: true` re-measures
  what baz measured before, and never touches a tag. A start while a pass runs
  is ignored and emits nothing.
- **Send** `AnalysisCommand::CancelReplayGainAnalysis` to stop. It lands within
  one decode block, keeps what it measured, and a later start resumes.
- **Observe**, on that receiver:
  - `Event::ReplayGainAnalysisStarted { tracks, editions }` — once per accepted
    start, *after* the plan exists, so the totals are real. `tracks: 0` is the
    honest answer for a library with nothing to do.
  - `Event::ReplayGainAnalysisProgress { path, analysed, tracks, failed }` —
    one per track finished with, counts cumulative. A bar is
    `analysed / tracks`; a label is `path`.
  - `Event::ReplayGainAnalysisFinished { analysed, failed, cancelled }` — once
    per accepted start. `cancelled` is the difference between "your library is
    measured" and "as much of it as we reached is measured", and they should
    not read the same.
- **Read** `AnalysisHandle::progress() -> AnalysisProgress` for the state
  without waiting for an event. Shared state is published **before** the event
  announcing it, so a read taken after an event is never older than the news.
- **On `ReplayGainAnalysisFinished`**, do two things: `Library::reload()`, and
  `EngineHandle::set_computed_gains(Some(Arc::new(library.computed_gains())))`.
  Until the second, the figures exist and are not being applied — which is
  correct, and a front end that forgets it will wonder why nothing got louder.
- **Render `source` honestly.** `computed_track` / `computed_album` /
  `computed_track_fallback` are baz's own measurements and should say so —
  "measured by baz" reads differently from "from this track's ReplayGain tag",
  and a listener asking where a figure came from is entitled to the difference.
  `ReplayGainSource::is_computed()` is the question to ask.
- **The fidelity indicator does not change**, again. It is still
  `path.is_transparent()` from `Event::VolumeChanged` combined with
  `SignalChain::Direct` from `Event::SignalPath`. A computed ReplayGain is a
  software gain exactly as a tagged one is, reported on the same channel, and
  rendered the way ADR-0009 §5 asks: small, neutral, informational, never a
  warning.
