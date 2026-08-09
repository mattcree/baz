# ADR-0029: The ambient surface — the field, the spectrum, the meter, the feed, and the class that admits them

> ## Amendment (2026-08-09) — the spectrum analyser, promoted
>
> After this record was written, the owner asked what the meter should be:
> *"is it a spectrum analyzer or graphic thing with the bars going up and
> down… that would be nice"*.
>
> **This collapses a deferral.** The earlier brief asked for *"a visualizer mode
> at some point, but also VU options"*, and §Consequences below deferred the
> visualizer. **The bars are that visualizer, and *"at some point"* is now.**
> Doc 12 §10, which was the deferral, is now the design; doc 12 §13's D1 is
> retired.
>
> **The R128 meter of §3 stays, in full** — the bars are a *visual* and the
> meter is a *reading*. What changes is which is the default and how they are
> held together. §3a is the new decision; §3 is unchanged except that its
> instrument register now defaults to off.

**Status**: proposed (2026-08-09), **amended 2026-08-09 (§3a: the spectrum analyser, promoted)** · extracts the decisions of `docs/design/12-now-playing-and-kiosk.md` · **amends [ADR-0020](0020-motion.md)** (adds §7, user-started ambient content) · **amends [ADR-0015](0015-replaygain-analysis.md) §3** (its reversal clause is triggered and re-decided) · rewrites four `docs/REFUSALS.md` entries on the owner's decision

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

### 3a. The spectrum analyser is the surface's primary visual

**The FFT is `realfft` 3.5.0, and it costs zero new crates.** Not a judgement
call: `rubato` — baz's windowed-sinc resampler, a non-optional `baz-core`
dependency (`crates/baz-core/Cargo.toml:25`) — already depends on `realfft`,
which depends on `rustfft`. Both are in `Cargo.lock` and compiled into every
build today. Licences are `MIT` and `MIT OR Apache-2.0`, both already on
`deny.toml`'s allowlist, so **no reviewed extension of the licence policy is
required**; none of the five crates has a `build.rs`, a `links` key, or is a
`-sys` crate, so **`docs/BACKLOG.md:122–131`'s "pure Rust, zero system
dependencies" property is not spent.** That note refused libopus for costing *"a
C library and a `cmake` build dependency on every platform"*; nothing of the
kind is being paid here.

**Hand-rolling is declined**, and the distinction from ADR-0015 is stated rather
than glossed: that decision hand-rolled the K-weighting because a skeptic must
be able to read it *against a published standard in one sitting*. **An FFT has
no standard to audit** — only an answer, checkable mechanically against a naive
DFT — so the argument that carried ADR-0015 does not reach it, and
`ENGINEERING.md`'s *prefer proven crates* applies unopposed. The FFT is also not
a parser in front of hostile input, which is the other half of the BACKLOG
note's reasoning.

**Where it runs: not the audio path, and never a queue.** The audio callback is
untouched. The engine thread's tap gains one line beside §3's meter call, on the
same `a`/`b` slices at the same instant, pre-gain: a `(l+r)*0.5` downmix written
into a **16 384-sample overwriting ring** (371 ms at 44.1 kHz). The UI thread
takes the newest 2048 samples once a frame and transforms them. **The writer
never blocks and there is no backpressure path**: a slow UI analyses a more
recent window next time, and the ring cannot grow. At 60 fps the reader is ~22×
inside the overwrite window. A torn read is possible, harmless, and accepted
deliberately — one frame of a visual, at 60 fps, is not worth a seqlock.

The meter deliberately does **not** read the ring: K-weighting is a stateful IIR
filter that must see every sample in order, and a consumer permitted to drop
samples cannot host one. The FFT is stateless per window and can.

**The transform**: 2048-point real, Hann-windowed, once per drawn frame,
normalised so a full-scale sine reads 0 dBFS. *Estimate: 20–60 µs per frame,
≈ 0.1–0.4 % of a 16.7 ms budget* — labelled an estimate, and gated below.

**The banding**: **32 Hz – 16 kHz**, nine octaves, geometric band edges, each
bar the **sum of power** of the bins in its band. **Bar count is derived from
width and the kiosk scale** — `round(body_width / (24 · kiosk_scale)).clamp(24, 64)`
— giving 49 bars at 1280, 64 at 1920, and 62 at 60 px pitch on 4K, because at
three metres bars must get *chunkier*, not merely more numerous. The bottom
octave holds ~1.5 bins, so the lowest bars share them and move together; **that
is stated rather than interpolated**, and a 4096-point transform for the bottom
octave is ranked as deferred rather than pre-built.

**The scale**: dBFS, height linear in decibels, **floor −72 dBFS** — below any
16-bit noise floor, above the level where dither makes bars twitch. **At digital
silence every bar is exactly zero, not near it**, because an all-zero window
produces exactly-zero bins. *Silence is a feature*, drawn rather than honoured.
Stopped or paused, the surface holds no bars at all, as it holds no artwork.

**Ballistics**: instantaneous attack, exponential decay, with peak-hold caps.
The **Ballistics** selector governs both instruments so the surface has one
speed setting — but the meter's column is **standardised and tested against
published documents** while the bars' column is **conventional**, chosen because
it looks right, and the code must not launder the second into the authority of
the first.

**The look**: drawn **inside the field's own shader**, so N bars cost a
64-float uniform upload rather than N quads. Colours are sampled from the cover,
capped at **L ≤ 0.38** — above the field's 0.22 so they read against it, below
the sleeve so **the artwork stays the brightest object**. **Never amber**: the
accent states playback truth and a spectrum is a property of the audio. The bars
are full-bleed and their opacity is **masked to zero over the centred column**,
softly, so type is never read over moving light — which costs **no layout at
all**, and is the discipline the ledger already blessed for the hover veil. Under
`tiny-skia`, where the shader renders nothing, the bars fall back to `Canvas`
geometry: a still field and working bars, never a hole.

**Defaults**: **the spectrum is on; the R128 instrument readout is off.** The
bars are what the owner asked to see, they read from three metres, and they are
the surface's primary motion; the meter is a precise number for a specific
question at 60 cm, and a kiosk that opens covered in decibel figures is an
instrument panel rather than something you leave running.

**They cannot disagree**, and this is asserted rather than argued. Both read the
same samples, at the same instant, through the same absent gain stage. The only
difference is that the meter is **K-weighted** and the bars are not — a
published curve, a stated transformation, not a discrepancy. Three tests hold
it: `the_meter_and_the_bars_agree_on_silence`,
`the_meter_and_the_bars_agree_on_a_full_scale_sine`, and
`neither_instrument_moves_with_the_volume`.

**Off is structurally zero**, by §3's mechanism exactly: an
`Option<SpectrumRing>` owned by the session and swapped by a `Command`. When
`None` there is no buffer, no downmix and no write — and with the toggle off
there is no `window::frames()` arm, so the loop parks.

**The gate gains three metrics**: FFT + banding **< 1 ms** per frame; ring write
**< 5 µs** per block on the engine thread; and the existing frame-time
thresholds **unchanged** at 8 ms (1080p) and 12 ms (4K) — the bars must fit
inside the budget already set, not enlarge it.

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

**Four toggles**, because they are four subsystems: **T1** field
(still/drifting/unavailable), **T2** spectrum, **T3** meter, **T4** feed — plus
one **Ballistics** selector governing both instruments. **T1, T2 and T4 default
on; T3 defaults off** (§3a). T1 defaults to *drifting* on hardware-protection
grounds (§7.6), since the drift *is* the burn-in mitigation. Controls live **both** on the
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
- **The visualizer is no longer deferred** — §3a is it, and the question *what
  would baz's own be* is answered: a real transform, drawn as light in the
  record's own colours, inside the field's shader, masked away from type. What
  remains deferred is a 4096-point transform for the lowest octave, if the bass
  ever reads as visibly ganged.
- **The plan is nine shippable steps**, ordered highest-relief-first: delete the
  duplicate transport · the hero decode · the static field · the kiosk type
  scale · the feed · the toggles · the drift (gated) · **the tap and the
  spectrum** (gated) · the meter. A release may stop after any of them, and doc
  12 §12 gives the shorter path — 1 → 2 → 6 → 8 — if the bars are wanted first.

## What would reverse this

- **The measurement gate failing.** If the field cannot hold 12 ms at 4K on real
  hardware it does not ship drifting, and T1's default becomes *still*; if the
  spectrum cannot, T2's default becomes off and the bars ship as an opt-in. The
  owner's condition — *"as long as the performance remains top tier"* — is
  explicit, and it is the one this decision is subordinate to.
- **iced exposing `MonitorHandle` *and* an answer to the global control-flow
  coupling** — both, not either — which would reopen the second window.
- **Evidence of OLED ghosting on a drifting field**, which would revive the
  periodic pixel nudge (priced at one frame per 60 s, exactly `REFRESH_TICK`'s
  existing bill).
