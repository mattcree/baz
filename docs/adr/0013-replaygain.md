# ADR-0013: ReplayGain — read the tags people already have, share the volume's gain stage, and say so

**Status**: accepted (2026-08-08) · **amends the guarantee as restated in [ADR-0011](0011-volume-control.md)** (which itself amended [ADR-0009](0009-follow-the-source-rate.md); every earlier decision stands, and what changes is that the software gain stage now has a second input) · schema v5, following the v2–v4 discipline of [ADR-0007](0007-album-editions.md), [ADR-0008](0008-album-artist-grouping.md) and [ADR-0010](0010-incremental-scanning-and-removal.md) · advances the `v0.2 "it respects"` line in `docs/VISION.md`

## Context

`docs/VISION.md` lists **"correct ReplayGain"** among the non-negotiable
inherited properties, and `docs/research/01-foobar2000.md` puts it on the
day-one list for the refugee this player is for. Until now baz had none: it
played every file at whatever level it was mastered at, and a shuffled queue
crossing from a 1980s CD rip to a 2010s loudness-war master moved twenty
decibels between one track and the next.

There are two separable units behind the word "ReplayGain", and they are not
the same size:

1. **Honour the tags files already carry.** Every serious library has been
   through foobar2000, `rsgain`, `loudgain` or `metaflac` at some point, and the
   numbers are sitting in the files. Using them is a parser, a selection rule
   and one multiply.
2. **Compute the tags.** An EBU R128 analysis pass over every track in a
   library: a loudness meter, a true-peak meter, a scan UI, progress and
   cancellation, tag *writing* (baz has never written to a music file), and
   validation against the EBU test vectors that `docs/ENGINEERING.md` names.

**This ADR is the first only.** The second is a separate unit and stays in
`docs/BACKLOG.md` under "bigger chapters". The split is worth stating because
the first is what a foobar2000 refugee notices on day one — their library is
already tagged — and the second is what an untagged library needs. Shipping
one without the other is honest as long as the untagged case is *not made
worse*, which is what the no-ReplayGain pre-amp's default of zero guarantees
below.

## Decision

### 1. Units on the wire and in the database are integers

Gains are **centidecibels** (`i16`, hundredths of a dB, 0 = unity); peaks are
**micro-units** (`u32`, millionths of full scale, 1 000 000 = 1.0).

This is the third time this workspace has made this choice and it is the same
argument each time (`protocol`'s "Time on the wire", ADR-0011 §1): one
canonical JSON encoding so `wire_format_is_stable` tests the protocol rather
than a float formatter, and the types keep their `Eq`. The third reason is
specific to ReplayGain: **0.01 dB is finer than the tag convention writes.**
`"-7.75 dB"` carries two decimals; a `f32` would have stored precision the file
never had.

The `Eq` half is load-bearing rather than decorative here. `TrackMeta` gains a
ReplayGain field, `TrackMeta` is embedded in `Album`/`Edition`, and the whole
workspace compares all three with `assert_eq!`. A float field would have
deleted `Eq` from three public types to gain nothing.

### 2. Three modes, and **off** is the default and is *structurally* off

`ReplayGainMode` is `Off` | `Track` | `Album`.

`Off` resolves to `ReplayGainDecision::UNITY`, whose `amplitude()` returns
exactly `1.0` **from an early return** rather than from `10⁰` happening to be
representable. The engine multiplies the volume by that, recognises the product
as exactly `1.0`, and takes the same no-copy, no-multiply branch it took before
ReplayGain existed. So "ReplayGain off" and "a baz without ReplayGain" are the
same stream bit for bit, and that is a property of the control flow rather than
of floating-point identity — the same argument ADR-0011 §5 makes for unity
volume, for the same reason.

Off is the *default* for the reason ADR-0009 makes the bit-perfect path the
default: **a player that has not been told to change the samples must not
change the samples.**

### 3. The selection rule

Stated once, implemented once (`ReplayGainSettings::resolve`), and tested as a
table (`tests/replaygain.rs`):

1. **Off** → unity, source `Disabled`. No pre-amp, no clip check.
2. **Track** → the track gain, clip-checked against the track peak. It does
   **not** fall back to the album gain: album-relative levels are the one thing
   track mode exists to remove, so supplying them under the name "track" would
   be answering a different question. A file with no track gain takes rule 4.
3. **Album** → the album gain, clip-checked against the **album** peak (falling
   back to the track peak if the file gives no album peak). When the file
   declares no album gain, the **track gain** is used and the source is reported
   as `TrackFallback` — a single downloaded track has no album to be relative
   to, and playing it unnormalised would be worse than playing it as its own
   album of one.
4. **No usable gain** → the *no-ReplayGain pre-amp*, source `NoTag`, no clip
   check (there is no peak to check against).
5. Otherwise the pre-amp is added and the total is clamped into
   `MIN_APPLIED_CENTIDB..=MAX_APPLIED_CENTIDB` (−90 dB … +20 dB) — a
   total-function guard on untrusted input, never reached by a real tag.
6. **Clipping prevention** (rule 7 below).

**Why album mode clip-checks against the album peak.** The album peak is the
loudest sample anywhere in the album, so it is at least this track's own peak.
Checking against it means every track of the album is reduced by *the same
amount*, which is exactly the property album mode exists to preserve. Checking
each track against its own peak would reduce them by different amounts and
reintroduce the level differences the album gain was carrying —
`album_mode_clip_checks_against_the_album_peak` pins this.

### 4. The clipping rule, exactly

When clipping prevention is armed (**on by default**) and a non-zero peak is
known:

> **applied = min(requested, ⌊−20·log₁₀(peak) × 100⌋ centidecibels)**

Three things are deliberate:

- **`min`, so it only ever reduces.** A peak below full scale does not license
  extra gain: a peak is a bound on this file, not a target. A quiet track with a
  peak of 0.1 still gets exactly the gain its tags asked for.
- **Floor, not round-to-nearest.** Rounding to the nearest centidecibel could
  round a limit up by half a centidecibel and put the result 0.005 dB over full
  scale — which is the single outcome the check exists to prevent.
  `the_clipping_ceiling_never_rounds_upward` asserts, for a sweep of peaks, that
  `amplitude × peak ≤ 1.0` after the conversion back.
- **A peak of exactly zero is treated as no peak.** Digital silence cannot
  clip, and `1/0` is not a gain.

When no peak is declared the gain is applied **in full**, and the readout says
which figure it came from. That is the honest answer: baz has nothing to
check against and will not invent a peak.

### 5. Fallback when a file has no ReplayGain at all: **a configurable pre-amp, defaulting to 0 dB**

foobar2000 makes this configurable, and baz follows it there. What baz does not
follow is defaulting it to anything but unity. Zero means:

- **Switching ReplayGain on cannot quieten a library that has never been
  scanned.** Every file resolves to 0 dB, the combined gain is exactly `1.0`,
  and the engine takes the transparent branch — so ADR-0009's guarantee is
  intact for that listener. `an_untagged_queue_is_untouched_in_every_mode`
  pins it against the existing gapless bit-exactness fixture.
- **A mixed library behaves the way the tags describe.** Tagged tracks are
  normalised, untagged ones play as stored, and the readout says which is which
  per track (`ReplayGainSource::NoTag`).

A non-zero default would have been guessing at the loudness of files nobody has
measured, which is the kind of silent alteration ADR-0009 exists to rule out.

### 6. It shares the volume's gain stage; it does not stand beside it

The resolved ReplayGain is **multiplied by the volume**, and the *product* is
published as the one gain the pump path reads (`SharedVolume::set_gain`). There
is one `Fader`, one multiply per sample, one slew.

Consequences, all deliberate:

- **The cost of ReplayGain on the realtime path is zero.** The pump does the
  same acquire load, the same branch and the same multiply it already did.
- **Both gains compose exactly once.** `volume_and_replay_gain_compose_into_exactly_one_multiply`
  asserts `sample × (v·g)` sample-for-sample **and proves the fixture can tell
  that apart from `(sample × v) × g`**, so "once" is measured rather than
  asserted about the source.
- **The device attenuator is only ever offered the volume.** A downstream
  attenuator cannot carry a per-track ReplayGain, so when a sink takes the
  volume the ReplayGain is still applied in software — and the path is reported
  as `SoftwareGain`, not `DeviceAttenuator`, because baz is multiplying.
- **ReplayGain may exceed unity, and the volume still may not.** ADR-0011's
  consequence "no gain above unity … makeup gain is a ReplayGain-shaped
  question and belongs with ReplayGain" is answered here: a quiet track with a
  `+6 dB` tag is amplified, because that is what ReplayGain is for, and
  clipping prevention (armed by default) is what keeps it safe. The *volume
  control* still only attenuates.

### 7. The gain changes at the track boundary, on the boundary's own sample

ReplayGain is per track and the engine can only change a gain between pump
calls — so `Session::pump` **caps every read at the next known track
boundary**. One comparison and one `min` per block; the samples delivered and
their order are unchanged (which is why the bit-exactness fixtures are
unaffected), and the first sample of a new track is the first sample at its own
gain rather than up to a block — 46 ms at the app's chunk size — late.

The change is then slewed over `RAMP_MS` (20 ms) like any other gain change, so
a gapless splice carries a short ramp rather than a step discontinuity. **In
album mode there is nothing to ramp**: every track of an album shares one album
gain, so the gain does not change at the boundary at all
(`album_mode_holds_one_gain_across_the_whole_album`).

The engine reads the tags from the file it is about to play, not from the
library index: `AudioSource` lifts them out of the metadata Symphonia already
parsed during the header probe, at **no extra I/O**, and they travel to the
engine thread on the `TrackBound` that already carries the track's rate and
depth. The engine is given paths and nothing else, so a queue the library has
never seen still plays at the right level.

### 8. Honesty: one gain stage, one readout — the existing one

**ReplayGain is a software gain. When it is active and not unity, the path is
not bit-exact.** That is reported through `Event::VolumeChanged`'s
`VolumePath`, unchanged, because baz has one gain stage and `VolumePath`
describes it. `VolumePath::is_transparent()` remains the whole question, and a
front end written before ReplayGain existed keeps getting the right answer.

`Event::ReplayGainChanged` carries the ReplayGain-specific facts — mode, both
pre-amps, whether clipping prevention is armed, which figure the gain came from
(`ReplayGainSource`), the applied centidecibels, and whether clipping
prevention bit — and **deliberately carries no fidelity flag of its own**. Two
answers to one question is how two answers come to disagree.

**Tone is part of the decision**, exactly as in ADR-0009 §5 and ADR-0011 §8.
This is information, never a warning. ReplayGain is a correctness feature the
listener asked for and describes a better listening experience, not a degraded
one; the unacceptable version is the silent one, where baz scales the stream
while claiming it does not.

### 9. Schema v5, no backfill

Four nullable `INTEGER` columns (`rg_track_gain_centidb`,
`rg_track_peak_micro`, `rg_album_gain_centidb`, `rg_album_peak_micro`), added
by `ALTER TABLE` inside one transaction with the `user_version` bump — v2's,
v3's and v4's discipline exactly, so an interrupted upgrade leaves a v4
database the next open migrates again.

`NULL` for every existing row, and the only honest value: a ReplayGain figure
lives in a file's tags and nowhere else, nothing already in the database
implies one, and computing one means the analysis pass this ADR explicitly does
not contain. The v2 backfill had a file extension to read; there is no
equivalent here.

`NULL` is self-healing, as v2's, v3's and v4's gaps were: the existing rescan
fills them. Note the interaction with v4's incremental scan, which is
**correct rather than a gap**: an unchanged file is not re-read and keeps its
NULLs — but an unchanged file is one whose tags have not moved, and a listener
who runs a ReplayGain scanner over their library rewrites those files, which
moves their stamps, which is what makes baz re-read them.

## Consequences

- **A listener who wants bit-perfect leaves ReplayGain off**, and the control
  says so. The same honest trade the volume makes, on the same channel.
- **An unscanned library is unaffected in every mode.** ReplayGain can be
  switched on over it and nothing changes — asserted against the gapless
  bit-exactness fixture, not a new one.
- **baz still does not write to music files.** Nothing here tags anything.
- **The R128 form is read but shifted.** `R128_TRACK_GAIN`/`R128_ALBUM_GAIN`
  are Q7.8 fixed point against EBU R128's −23 LUFS, while ReplayGain 2.0 aims
  at −18 LUFS, so `+5 dB` is added on the way in
  (`R128_REFERENCE_OFFSET_CENTIDB`, stated as a named constant so the
  assumption is visible and testable). Opus files themselves are still not
  scanned — Symphonia ships no Opus decoder (`docs/BACKLOG.md`) — but the tag
  form turns up in Vorbis comments on FLAC and Ogg files written by R128-era
  tools, which is why it is read.
- **A `REPLAYGAIN_*` value outranks an `R128_*` one for the same field**,
  whichever order they arrive in; within one family the first parseable value
  wins, which is how every other player reads a duplicated Vorbis comment.
- **Parsing is defensive and returns `None` rather than saturating.** A gain of
  `1e30`, a negative peak, `NaN`, `inf`, a Unicode minus sign — all read as "the
  file did not say", which is a state the selection rules already handle. A
  malformed tag can neither panic nor poison the other three figures on the same
  file, and it never reaches a speaker as a number nobody chose.
  `fuzz/fuzz_targets/replaygain_tags.rs` is the standing check.
- **True-peak is not implemented.** The `_PEAK` tags are *sample* peaks, which
  is what ReplayGain 2.0 scanners write, and inter-sample overshoot after
  reconstruction is not modelled. Saying so is better than implying a
  guarantee the tags cannot support; a true-peak limiter belongs with the
  scanning work.
- **No limiter, no dynamic compression.** The gain is a constant multiply per
  track. If clipping prevention has to cut, it cuts the whole track's gain —
  it does not ride it.
- **The GUI is deliberately not built here**, as ADR-0011's was not. The
  protocol, the parser, the engine, the index and the readout are; the control
  is a parallel unit. What a front end needs is below.

## What a front end needs

- **Send** `Command::SetReplayGain { mode, preamp_centidb, no_tag_preamp_centidb, prevent_clipping }`.
  It is absolute and idempotent — send the whole setting, not a delta. `mode` is
  `off` | `track` | `album`; both pre-amps are hundredths of a decibel and clamp
  to ±`baz_core::replaygain::MAX_PREAMP_CENTIDB` (±20 dB); `prevent_clipping`
  should default to `true`. Redundant commands emit nothing, so an event is
  always news.
- **Observe** `Event::ReplayGainChanged { mode, preamp_centidb, no_tag_preamp_centidb, prevent_clipping, source, applied_centidb, clipping_prevented }`
  and follow it rather than your own optimistic copy, so two front ends on one
  engine agree. It arrives on an accepted command *and* at a track boundary
  where the resolved figure changes — an album in album mode states it once.
- **Read** `EngineHandle::replay_gain() -> ReplayGainState` once at start-up,
  for the state before anybody changes anything.
- **Render** `applied_centidb` as `applied_centidb / 100.0` dB if you show a
  number, and `source` as the explanation: `no_tag` means *this file has no
  ReplayGain*, which is a fact about the file and not a failure —
  it is the expected reading for a library that has never been through a
  scanner, and `disabled` is a different fact and must look different.
  `clipping_prevented` is worth surfacing: it is the difference between "your
  +6 dB was applied" and "it was cut to +2.1 dB to stay below full scale".
- **The fidelity indicator does not change.** It is still
  `path.is_transparent()` from `Event::VolumeChanged` combined with
  `SignalChain::Direct` from `Event::SignalPath`. An active ReplayGain will move
  `path` to `software_gain` with the volume still at unity — that is correct,
  and it should be rendered the way ADR-0009 §5 asks: small, neutral,
  informational, never a warning.
