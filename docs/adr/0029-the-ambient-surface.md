# ADR-0029: The ambient surface — the field, the spectrum, the meter, the feed, the run, and the class that admits them

> ## Amendment (2026-08-10) — the queue is this surface's other half
>
> The owner: *"we need to work on the now playing since I think the queue and
> the now playing need integrated in some way so we can remove the queue option
> from the bottom bar"*.
>
> **`Place::Queue` is deleted and `Place::NowPlaying` absorbs it whole**, and
> the bar's `Queue` door goes with it. §8 below is the decision; doc 12 §3.4 is
> the argument, §5.5a the measured composition, §6.4 the fate of every affordance
> and of the bar.
>
> **This does not supersede anything in §1–§7.** The ambient surface is
> unchanged in kind; what changes is that it now has a second column, so §2's
> field, §3a's spectrum and §5's toggles acquire one new constraint each — all
> three collected in §8.4 rather than scattered. The one thing that *is*
> superseded is **§Consequences' "nine shippable steps"**, which becomes twelve
> and is re-ordered: doc 12 §12.0 is the new table, and it names the disposition
> of all nine.
>
> **A separate record carries the model this surface now reads from.** The owner,
> in the same conversation: *"probably the basic model is that every album has a
> playlist implicitly… it should be basically which playlist and which track"*.
> That reaches the protocol and the play ledger, which this record does not, so
> it is [ADR-0034](0034-the-run-and-its-list.md) and is cited from here rather
> than folded in.

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

**Status**: proposed (2026-08-09), **amended 2026-08-09 (§3a: the spectrum analyser, promoted)**, **amended 2026-08-10 (§8: the queue merged in, and the bar's door removed)** · extracts the decisions of `docs/design/12-now-playing-and-kiosk.md` · **amends [ADR-0020](0020-motion.md)** (adds §7, user-started ambient content) · **amends [ADR-0015](0015-replaygain-analysis.md) §3** (its reversal clause is triggered and re-decided) · **amends [ADR-0022](0022-places-and-nothing-else.md)** (`Place::Queue` is deleted; the model loses a member and gains nothing) · **spends [ADR-0034](0034-the-run-and-its-list.md)** (the merged surface's head is that record's `Origin`) · rewrites four the product's standing rules entries on the owner's decision

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

**Four of this product's own entries stood in the way**, and the product's
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
   **(Half fixed since, 2026-08-10.** The call is gone and
   `now_playing.rs:178–189` carries the argument where it stood — but the 32 px
   the widget reserved is **still summed into `art_edge`'s `below`**
   (`now_playing.rs:62–67`), so the artwork is 32 px smaller than it should be at
   every height-bound size. §8's step M1 takes it out.)
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
other, visibly and at rest, which is all the product's accessibility entry
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

### 8. `Place::Queue` is deleted, and this surface holds the run

> **A run is a list and a cursor. This place was the cursor; `Place::Queue` was
> the list. They are two readings of one object and they become one surface.**

That sentence, and not adjacency, is why the merge is right — the two places
were each drawing half of a thing, and each one's own module doc admits it
(`queue.rs:30–38` calls the queue *"one list with a cursor"* and draws the
cursor as a 6 px dot; `bottom_bar.rs:16–22` records that the queue place's
reading half was delegated to the bar). Doc 12 §3.4 carries the full argument
and three independent confirmations.

#### 8.1 One place, two columns, one stated control

The record and the run stand **side by side** at a body ≥ **784 px**
(`SPLIT_FLOOR`), and re-stack into one scrolling column below it with the record
as the run's head. The run's column is `RUN_MEASURE` = `LIST_MEASURE / 2` =
**440**, scaled by `kiosk_scale`. Doc 12 §5.5a is the arithmetic and its table.

**The measurement that decides it**: at 1920 × 1080 the artwork is bound by
*height*, not width — 720 today because `NOW_PLAYING_MAX` clamps it, 729 after
§2 deletes the clamp — with **1104 px of body width unused**, measured off a
rendered frame (`docs/design/impl/queue-in-now-playing/`). The run column asks
for 464. **The merge costs the artwork nothing at 1920, nothing at 4K, and 53 px
at 1280 with the lane expanded**, where `Ctrl+B` is the remedy already on screen.

**Whether the run is shown is a stated, remembered control** — a `Run` word-door
beside `Ambient` in the place's top-right, on by default — **and not a function
of full-screen.** The shape offered was *full-screen stands the list down*, and
the drawing broke it on a toolkit fact this record already established in §6:
**iced 0.13 exposes no monitor enumeration**, so baz cannot tell a second-display
full-screen from an only-display one, and the single-display listener would lose
the editor to a window act. It would also make `F11` decide a place's contents,
which is the micro-mode doc 12 §3.2 refuses by name.

#### 8.2 The bar loses its door and does not move

`queue_button` (`bottom_bar.rs:348–389`), `Message::ToggleQueue`,
`theme::UP_NEXT_W` and `theme::POSITION_W` are deleted. The bar's three zones
are `Fill · TRANSPORT_W · Fill` (`bottom_bar.rs:106–117`), so **the transport
column does not move a pixel** and the now-playing block gains the 160 px the
door was holding — 288 → 448 at a 1280 window.

**The ratchet, both halves, answered separately**: the door's *readout* (the
queue's size) is replaced by a better statement of the same fact, the merged
head's `2 of 24` — the permitted move, and the one `bottom_bar.rs:328–338`
already made once. The door's *route* is removed because the owner asked, and
what is traded is recorded rather than smoothed: the press count to an editable
run is unchanged at one, the muscle memory is not.

**The continuation line stays**, and earns its place harder than before: with the
door gone it is the only reading of the run outside this place. It does **not**
gain the list's name — doc 12 §3.5 ③ refuses that on a width measurement.

#### 8.3 The doors in

`Ctrl+U` re-aims at `Place::go(Destination::NowPlaying)` **with `Run` on** — two
presses of visible controls, made for you, which is the accelerator construction
ADR-0023's amendment already blessed for the context menu's `Queue` item. **It
stops toggling**, because it is now the accelerator of a *destination* and
`place.rs:248–257` settles that a destination does not close itself. `Esc` is the
way out and always was.

**`Q` was not bound to the queue**, contrary to the brief that commissioned this
amendment: `keys.rs:820` asserts bare `q` is query text, because ADR-0017 §1.2
took every bare letter. Two doc comments still claim otherwise — `place.rs:158`
and `bottom_bar.rs:347` — and die with the code they document.

**`Resume` is re-checked and reads better, not worse** (`app.rs:2242–2253`). Its
subject is a run — a list and a position in it — and it has always navigated to
a surface that could draw only the position. Nothing about the gesture changes;
the destination can finally show what was resumed.

#### 8.4 What §2, §3a and §5 owe the second column

Three new constraints, and all three are shader arithmetic or a boolean — none
is a layout, a pass, or a scrim:

1. **The spectrum's mask widens from one centred column to the *type box***
   (the placard column ∪ the run column). Same uniform, two edges instead of
   one. §3a's *"masked to zero over the centred column, softly, so type is never
   read over moving light"* is unchanged in intent and wider in domain.
2. **Under the run column the field is clamped to `room.wall`'s lightness.**
   This introduces no new contrast number: `room.wall` is the ground every other
   list in the product is read over, so every pairing on a run row is the pairing
   that ships today, and the test is one sentence.
3. **Under the run column the field does not drift.** Clamping lightness alone
   leaves a hue drift under scrolling type. The ambient owns the rest of the
   surface and stops at the run's edge.

Stated once so it is not mistaken for §5.3's refused scrim: **the field is one
object with one ceiling function, and the ceiling is lower where type is.** A
scrim is a second object interposed; this is the same object's own value,
reduced.

**§7's gate gains one condition**: the frame-time thresholds are measured **with
a five-figure run on screen**, not with a twelve-track album. The spectrum's cost
is per frame and the run's is per visible row, and a gate over the easy half of
the composition is not a gate. `queue_window`'s virtualization
(`queue.rs:130–196`) is what makes that affordable and is now load-bearing at
kiosk scale.

**§5's four toggles stay four.** `Run` is not a fifth: T1–T4 decide how the
surface looks, `Run` decides what it is about, and a list that is not drawn
costs a `Vec` that is not built rather than a subsystem's *off*.

### 8.5 Amendment, 2026-08-10: the `Run` word is removed, and the density with it

**The owner's decision, recorded as a decision rather than argued with.**
*"remove the run button from the now playing"*, confirmed when asked which
control he meant: *"run button is what I'm referring to; just to be clear"*.

§8.1's *"whether the run is shown is a stated, remembered control"* is
**reversed**. The run column stands whenever there is a run, and nothing else
decides it. §8.1's other half — that the density must **not** be a function of
full-screen — is not reversed and did not need a word to be true: `F11` is a
window act, iced 0.13 still publishes no monitor enumeration, and there is now
no mode left to bind.

What went: the word, `Message::ToggleRun`, `App::run_column`, `set_run`, the
`run_column` config key, the place's `run: bool` parameter, `theme::now_playing`
and the run column's 48 px clearance strip. What stayed: the run column and
every one of §6.4.4's fifteen affordances.

§8.3's `Ctrl+U` construction simplifies. It was *the lane's row plus the place's
`Run` word, made for you* — legal under ADR-0023's amendment as an accelerator
sending the two messages its visible controls send. With the word gone the chord
resolves to `Message::ShowNowPlaying`, which is the message the lane's row **and**
the bar's now-playing block both send: one message, two visible twins, and no
construction needed.

#### What this costs step A6, and it is not nothing

**A6 must design its own control from scratch.** §5 and doc 12 §7.2 both place
the `Ambient` word-door *beside* `Run` in the place's top-right, and §3.4.3
argues the pair as a pair: *"it is a peer of `Ambient`, not a fifth row inside
it"*. That argument is now one-sided — there is no peer, and the top-right
corner is empty.

Three things follow, and they are recorded here so A6 does not arrive expecting
a partner that has gone:

1. **The corner is unclaimed, not reserved.** The clearance the run column left
   for the word is deleted, because height held for a control that does not
   exist is the defect this surface's arithmetic refuses everywhere else. A6
   brings its own clearance back with it and measures it against what it draws.
2. **`Ambient` can no longer justify its placement by symmetry.** *A second word
   beside the first* was half the argument for putting a door there at all; A6
   must make the whole argument on its own, and the alternative it should
   consider first is that the four toggles live in Settings and nowhere else —
   which §5's *"both, and they are different controls"* rejected partly because
   the surface already had a word-door to sit beside.
3. **The precedent the removal sets is about *the surface's subject*, not about
   ambience.** `Run` governed what the place *is about*; T1–T4 govern how it
   *looks*. The owner removed a control of the first kind. That says nothing
   either way about a control of the second kind, and A6 should not read it as
   a verdict it is not.

#### What it does not change

`Save as playlist`'s narrowing (below) and §8.4's three shader constraints are
independent of this and stand. §5's structural zero is untouched: the ambient
subscription's condition was `self.place == Place::NowPlaying && …` and never
mentioned the density.

### 8.6 Amendment, 2026-08-10: three kinds of list, and one save word

**The owner:** *"I still see save as playlist on the queue when playing a CD...
we should only be showing that in a situation where there isn't an existing
playlist. it seems to me underlying we should have playlists which are like
'fixed' i.e. a CD's track listing and some which are unnamed i.e. when we just
'add to queue' and some which are named i.e. we saved it already. the only one
which has any kind of indicator to save is the 2nd case."* And, the same
afternoon, narrowing it: *"nah I think adding more stuff to an existing playlist
is fine, that does not need a save -- it's a low bar to edit a playlist"*.

> **The save word appears only for a run the listener assembled from nothing.**

`QueueVm::provenance: Option<String>` becomes `QueueVm::source: RunSource` with
his three kinds — `Fixed`, `Playlist(name)`, `Assembled`. ADR-0024 §A5.2's
`RunOrigin` gains the distinction it was missing: *has a file* and *did the
listener assemble this* are different questions, and only the second one decides
a creation act. `Diverged` keeps the file's name and becomes a readout —
`From "Road Trip"` — because it may neither claim to *be* a file it has diverged
from nor offer a second route to a thing whose own page is a cheap route.

**A fixed run that has been edited is `Assembled`.** That asymmetry with
`Diverged` is the owner's own reasoning applied where his reason does not reach:
a named run has a cheap page to go and edit, and a fixed one has none, so what
the edit produced exists nowhere else and the creation act is the only route
left.

**ADR-0024 §1 and ADR-0023 §3 are untouched.** Nothing writes back, in any of
the four states. *"A low bar to edit a playlist"* is an argument about the
playlist page's reachability, not licence for a queue edit made for tonight to
rewrite a file somebody owns.

## Consequences

- **`Place::Queue` is the first member this product has deleted.** The enum goes
  from eight to seven, `Place::queue()` and its toggle test go with it, and the
  exhaustive walk (`place.rs:446–520`) keeps its property with one fewer arm.
  Adding a member is cheap and adding a *kind* is not; removing one is cheapest
  of all, which is the surface model working rather than bending.
- **Two empty states become one.** *"Nothing playing."*
  (`now_playing.rs:111–118`) is deleted in favour of the queue's *"Nothing
  queued"* block (`queue.rs:382–414`), which names the gestures that fill it and
  carries the silence-is-a-feature sentence. The `transport_pending` branch above
  it survives untouched — a start in flight is still not silence.
- **Every other queue affordance survives**, fourteen of fifteen, each named with
  its fate in doc 12 §6.4.4. The only casualty is `place_header("Queue")`, which
  had nothing left to head.
- **`Undo`'s scope follows the word.** `note_place_left`'s `Place::Queue` becomes
  `Place::NowPlaying`, **and turning `Run` off clears the history too**, because
  an accelerator whose visible twin is off screen is not legal.
- **The plan is twelve steps, not nine.** Doc 12 §12.0 carries the table and the
  disposition of all nine; the short version is that the merge goes first because
  every later step lays out against the composition it changes, and that **step 1
  is half shipped** — the duplicate widget is gone but its 32 px is still in
  `art_edge`'s `below` (`now_playing.rs:62–67`).
- **Four the product's standing rules entries are rewritten** — artwork-vs-source, the scrim,
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
- ~~**The plan is nine shippable steps**, ordered highest-relief-first~~ —
  **superseded by §8**. Twelve, in doc 12 §12.0's order:
  **M1 · M2 · A2 · M3 · A3 · A4 · A5 · M4 · A6 · A7 · A8 · A9**. A release may
  stop after any of them, and the shorter path to the bars is
  **M1 → M2 → A2 → A6 → A8**.

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
- **The run column proving unreadable over the field on real hardware.** §8.4
  answers it with an argument — the ground under the run is `room.wall`, which is
  what every other list is read over — and that argument is checkable in one
  capture. If it fails, the honest fix is that the field stops behind the run
  entirely (the room, `#0C0D0E`, as the ground) rather than a scrim over it.
- **Listeners finding the merged surface's editor where they wanted a kiosk**,
  which would not restore `Place::Queue` — it would make `Run` default *off* at
  first run above some body width, and that is a one-line default, deliberately.
