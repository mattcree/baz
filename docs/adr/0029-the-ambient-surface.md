# ADR-0029: The ambient surface — the field, the meter, the feed, and the class that admits them

**Status**: proposed (2026-08-09) · extracts the decisions of `docs/design/12-now-playing-and-kiosk.md` · **amends [ADR-0020](0020-motion.md)** (adds §7, user-started ambient content) · **amends [ADR-0015](0015-replaygain-analysis.md) §3** (its reversal clause is triggered and re-decided) · rewrites four `docs/REFUSALS.md` entries on the owner's decision

## Context

`Place::NowPlaying` shipped (`crates/baz/src/views/now_playing.rs`, routed at
`app.rs:3670–3676`): the sounding record's art, its identity, the needle, the
transport. Its own module doc calls it *"a first version"* and says what it was
built to become — *"the kiosk full-screen mode is this same surface at a larger
size, and that is a property of the composition rather than a plan"*
(`now_playing.rs:17–30`).

The owner then said what it should become:

> *"now playing does not need the play pause controls. it would be nice if the
> album art was somehow more prominent, like it takes up the background and has
> some nice VU meter stuff over it in a stylised way, maybe somewhat ambient…
> I also like the idea of just seeing related stuff appearing in like a feed of
> random facts. I think this kinda stuff would naturally be toggle-able"*

and, on the constraint that governs all of it:

> *"ambient motion is fine as long as the performance remains top tier."*

**Four of this product's own entries stood in the way**, and `REFUSALS.md`'s
preamble settles the process: the ledger *"binds contributors and agents — not
the owner… an entry he reverses gets rewritten to say what was decided and why,
and that is the whole of the process."* So this record does not argue. It
records, and it specifies the constraints that replace the blanket refusals.

**Two defects were found in the shipped surface while specifying it**, and they
matter because they change what the decisions are *about*:

1. **The transport is drawn twice.** `now_playing.rs:168` calls
   `bottom_bar::transport(player, ink)` — the bar's own function — while
   `app.rs:3744–3752` appends the bar under every place unconditionally. The
   owner's *"does not need the play pause controls"* is a bug report.
2. **The surface upscales artwork, and the ledger says it never does.** The
   handle comes from the 320 px thumbnail cache (`art.rs:48`,
   `now_playing.rs:106`) and is drawn at up to `NOW_PLAYING_MAX` 720
   (`now_playing.rs:81`) — **2.25× at 1920 × 1080 and above**. *"No artwork is
   ever drawn larger than its source"* was already false here, in the one place
   nobody had a test for.

## Decision

### 1. The transport comes off the surface

`now_playing.rs:168`'s call is deleted; nothing replaces it. The bar carries
play/pause, previous, next, the needle and the fader in this place as in every
other, visibly and at rest, which is all `REFUSALS.md`'s accessibility entry
requires. This is the same reasoning that removed `‹ Library` from the place
headers in `9a7e9a5` — **a place that repeats what a resident surface already
carries is making one statement twice, and the copy is what goes**. It also
returns 32 px (`TRANSPORT_HIT`) to the arithmetic that decides how large the
artwork can be.

### 2. The background is a derived field; the artwork is drawn at its true size

**Not full-bleed artwork.** A 1000 px cover on a 3840 px monitor is a 3.8×
upscale — a visibly soft image, on a surface whose subject is a piece of visual
art — and it crops, because a sleeve is square and a monitor is not.

**The field** is three colours sampled from the cover, lightness and chroma
clamped into the room's range, composited as a wash over `#0C0D0E` at a ceiling
of **L 0.22**. It is *not the artwork*: it is not invertible, it carries no
resolution, and it dims nothing. It is the rule this ledger already applies to
the art-derived lamp — *"hue read from the record, lightness and chroma pinned…
**data**, not a preference"* — at a larger size. Amberol ships the honest
version of this; Apple Music and YouTube Music ship the blurred-copy version,
which is the one that duplicates the work.

**The artwork** gets a second decode of one record — `load_hero` at
`HERO_PX` 1024, 2 entries, ~8 MiB against the thumbnail cache's 150 MiB — and
`art_edge` gains a third term bounding it by **the source's own pixels**.
`NOW_PLAYING_MAX` is deleted: it was a constant standing in for a fact about the
decode, and it is what gave a 4K panel a 720 px cover in a 3744 px body.

**Nothing is drawn on the sleeve. Everything ambient is drawn on the field.**
That sentence reconciles *"VU meter stuff over it"* with the entry forbidding
marks on artwork outside a wall tile: what *"takes up the background"* is the
field, and the meter is over the field.

### 3. The meter is a momentary-loudness meter, and it says which standard

**Default: EBU R128 / ITU-R BS.1770-4 momentary** — K-weighted mean square over
a 400 ms sliding window, 100 ms hop — with a sample-peak indicator. Chosen
because **baz already owns that filter, derived and vector-tested** (ADR-0015 §1
asserts all ten coefficients against BS.1770-4's published tables and matches
five compliance vectors to 0.025 LU), because it is the same scale as the
record's stored integrated loudness so the two are comparable, and because
`BLOCK_MS = 400` with `STEPS_PER_BLOCK = 4` is already exactly this window.

***"VU options"* is answered with three ballistics**, each naming its standard:
**Loudness** (R128 momentary, default), **VU** (IEC 60268-17: 300 ms to 99 %,
1–1.5 % overshoot), **PPM** (IEC 60268-10 Type II: 10 ms rise, 2.8 s per 24 dB).
Each ships with compliance tests against the published document, on ADR-0015's
own precedent that a measurement without vectors is a guess wearing a standard's
name.

**The tap reads `a` and `b` in `Session::pump` (`engine.rs:2759–2777`) — the
ring's own content, pre-gain, in both branches.** `settle_volume`
(`engine.rs:1859`) folds ReplayGain and volume into one number applied in one
place, so the ring holds the decoded file untouched. Consequences: the meter
**cannot contradict `bit-perfect`, because it never observes the gain stage**;
one tap has one meaning in both branches; it measures the record rather than the
volume knob; and it is comparable to the stored figure. That it reads before
ReplayGain is **labelled on the instrument**, or it would be snake oil.

**Metering cannot alter a sample, by type rather than by promise**: the tap is
`fn observe(&mut self, samples: &[f32])` — never `&mut` — so ADR-0009's
bit-exactness is defended by the borrow checker and its existing tests pass
unmodified.

**Cost when off is zero, not small**: the meter is an `Option<LiveMeter>` owned
by the session and swapped by a `Command`. When `None` there is no filter state,
no memory, no arithmetic, and **no atomic is loaded** — the pump does one null
check on thread-local state per block, the same class of check it already
performs.

**Levels cross on the pattern the codebase already uses twice**: two
`AtomicU32`s carrying f32 bit patterns, single writer, plain
`store(Ordering::Release)`, published **10 times a second** on step close — the
discipline of `DeviceSink`'s callback counters (`device.rs:385–387`) and
`SharedVolume`'s gain (`volume.rs:244–249`). `LoudnessMeter` itself is **not**
reused: `close_step` pushes to a `Vec`, which may allocate. `KWeighting` is.

### 4. The feed is local-first, and its rotation rule is one sentence

> **The feed shows one fact at a time, cycling in a fixed order through exactly
> the facts this record has, advancing every 20 seconds, on every track change,
> and whenever you press it.**

The pool is **the record**, so it is inspectable by exhaustion — you can press
through all of it in under a minute, which is the strongest form of the ledger's
no-invisible-pool rule. The order is **fixed, not random**, despite the brief's
*"random facts"*: a random draw means a fact you saw once and cannot get back.
Variety comes from records differing in which facts they have.

Eleven facts, all already on disk: the ledger's play count and dates, the
record's position in the collection, the measured loudness and peak, the full
signal path (**currently dead code**, `player.rs:2016–2027`), the technical
truth, the release year, the queue's provenance.

**On the engagement-stats entry**: *"Played N times since YYYY"* is not near the
line — it is **the permitted item, verbatim**, and it lost its home when
ADR-0022 deleted the inspector. Refused and kept refused: `listened_ms` as a
total, any cross-track aggregation, and any sentence with an opinion about the
listener. *History records; it never performs.*

**Network enrichment is not in this decision.** It is blocked on an identifier
baz does not store, and the surface must be excellent without it — if the
network layer's failure mode is *"the feed is slightly shorter"*, the local
design was right.

### 5. ADR-0020 gains a sixth class, and the toggles are first-class

ADR-0020 already admitted a class rather than stretching a list once, for the
fisheye, *"because shipping it under §2's list silently would make the list a
fiction"*. The same move:

> **§7. User-started ambient content is permitted**, distinct from the bounded
> tween and from pointer-derived deformation:
> - only on a surface whose purpose is to be looked at — today
>   `Place::NowPlaying` and nothing else; a second needs an argument that beats
>   this one;
> - **a thing you start, never a thing that starts itself** — the ledger's own
>   sentence about shuffle;
> - **exactly nothing when off or off-screen**, structurally;
> - a **stated frame budget and a measured cost**, re-measured when it changes;
> - it **states nothing**, so it may never be the only carrier of a fact.

**Three toggles**, because they are three subsystems: **T1** field
(still/drifting/unavailable), **T2** meter, **T3** feed. **All default on**, and
T1 defaults to *drifting* — that one on hardware-protection grounds (§7.6),
since the drift *is* the burn-in mitigation. Controls live **both** on the
surface (an `Ambient` word-door, visible at rest — never hover-revealed) and in
Settings.

**The structural zero.** The ambient subscription is one arm in `app.rs`'s
`Vec<Subscription>` under `self.place == Place::NowPlaying &&
self.ambient.animating()`. iced 0.13 rebuilds subscriptions after every update
and drops the ones that went away (`app.rs:3900–3908`), so navigating away or
toggling off **removes the timer** — there is no teardown to forget, because
there is no handle to hold.
`the_ambient_clock_is_absent_outside_its_place` asserts it over every place ×
every toggle combination.

### 6. The kiosk is one window, full-screen, on the monitor it is already on

Verified against the installed iced 0.13.1 source: **there is no monitor
enumeration in iced's public API at all** — `MonitorHandle`,
`available_monitors` and `primary_monitor` appear nowhere in `iced`, `iced_core`
or `iced_runtime`; winit's `Fullscreen::Borderless(Option<MonitorHandle>)` is
used at `iced_winit/src/conversion.rs:398` but the `Option` is always iced's
choice. `Mode::Fullscreen` lands on the window's **current** monitor
(`iced_winit/src/program.rs:1331–1338`).

So: drag the window there, press `F11`. And a **second, stronger** reason not to
grow a second window — iced's control flow is **global, not per-window**
(`program.rs:471–488`) and every window is redraw-requested after any message
batch (`program.rs:1089–1097`), so a kiosk window's ambient clock would pace the
main window and make ADR-0020's idle claim false for the whole product. **The
single-window design is what keeps ambient motion free elsewhere.**

At kiosk size, `kiosk_scale(edge) = (edge / 720).clamp(1.0, 2.5)` steps the type
— keyed to the work's own resolved size, with a floor of 1.0 so **every window
at or below 720 px of work is pixel-identical to what ships today**.

### 7. "Top tier" is a number, and it gates the work

The measurement extends `docs/design/04-fluidity.md` §1.3's harness — the same
binary, the same instruments, plus frame time and GPU busy % — at 1280 × 800,
1920 × 1080 and **3840 × 2160**. No ambient work merges until:

| Metric | Threshold |
|---|---|
| Frame time, 1920 × 1080, 99th pct | **< 8 ms** |
| Frame time, 3840 × 2160, 99th pct | **< 12 ms** |
| Process CPU, ambient, 4K | **≤ 5 %** of one core |
| Process CPU + `view()` calls, all toggles off, idle | **0.0 %**, **0 frames** |

The reference point already in the project: doc 04 measured **4.0 % CPU at
60 fps continuous** on a real GPU while drawing **120 album tiles**; this surface
draws one image and a handful of text runs.

**The measurement must be taken on real hardware.** Doc 04's own note applies:
*"there is no headless path to a real-GPU number, and reporting
software-rasterised CPU as if it were the shipping cost would be a false
receipt — so the intrusion was taken deliberately rather than avoided."*

**Implementation**: the field is an `iced::widget::shader` (available today —
`wgpu` is in iced's default features), so drift is a time uniform and one float
per frame rather than CPU re-tessellation. The meter is a `Canvas` (a one-line
feature flag, no new crate) with static chrome and the moving mark in **separate
caches**, because `Cache` re-tessellates everything it holds whenever it is
cleared. **`tiny-skia` renders no shader at all**, so T1 has a third state,
`unavailable`, which falls back to the static field — a still field, never a
hole.

## Consequences

- **Four `REFUSALS.md` entries are rewritten** — artwork-vs-source, the scrim,
  skeuomorphism's VU ban, and the motion clause — each in the ledger's own
  amendment form, quoting the old text and naming the constraint that replaced
  it. Doc 12 §14 carries the drafts.
- **ADR-0015 §3's reversal clause is triggered and re-decided rather than walked
  past.** It named *"a momentary or short-term meter"* as a reason to re-make
  the hand-rolled-rather-than-crate decision. Re-made: what this needs is not
  generality but the K-weighting filter already written and vector-tested, with
  *less* machinery around it (no gating, no LRA, no album pass); taking a
  dependency to get a subset of what baz has, on the realtime path, with its
  allocation behaviour unaudited, is the worse trade.
- **A defect in the bar is recorded**: `bit_exact()` compares
  `chain == SignalChain::Direct` exactly, so an `Exclusive { conversion: None }`
  chain renders no note at all. `protocol.rs:924–927` gives the fix — ask
  through `is_exclusive()` and `conversion_reason()` rather than enumerating
  variants.
- **`Event::PlayRecorded` gains its first consumer in `crates/baz`**, and
  `signal_path()` stops being dead code.
- **The visualizer is deferred**, and now for sequence and cost rather than for
  the motion law, which no longer forbids it. The tap it needs arrives with the
  meter; what it lacks is an answer to *what baz's own visualizer would be*,
  which should not be answered inside a decision about something else.
- **The plan is eight shippable steps**, ordered highest-relief-first: delete
  the duplicate transport · the hero decode · the static field · the kiosk type
  scale · the feed · the toggles · the drift (gated on measurement) · the meter.
  A release may stop after any of them.

## What would reverse this

- **The measurement gate failing.** If the field cannot hold 12 ms at 4K on real
  hardware, it does not ship drifting, and T1's default becomes *still*. The
  owner's condition is explicit and it is the one this decision is subordinate
  to.
- **iced exposing `MonitorHandle` *and* an answer to the global control-flow
  coupling** — both, not either — which would reopen the second window.
- **Evidence of OLED ghosting on a drifting field**, which would revive the
  periodic pixel nudge (priced at one frame per 60 s, exactly `REFRESH_TICK`'s
  existing bill).
