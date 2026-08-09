# 12 — The now-playing screen: a surface for the far field

> The owner, verbatim:
>
> *"we would like a nigh playing screen, which is just dedicated to a very
> nice, almost, like, kiosk like look in the sense that you can just leave it
> full screen on your other monitor, and it will just be great. For example,
> if it could show the album and the track that's playing and also details
> about that that are found on… well, wherever you think is a good source that
> really can give context. Or that's just an example of a thought, but I still
> think that kiosk mode could be good."*
>
> And, in a second brief: *"maybe we could have a visualizer mode at some
> point, but also VU options"* — with the instruction that the two are to be
> **separate proposals**.
>
> (Voice-dictated; *"nigh playing"* is now playing.)
>
> A design study, not an implementation. Written 2026-08-09 against `b795a06`
> (the merge that retired the icon backlog entry). Every claim about shipped
> behaviour is cited `file:line`; every prior-art claim carries a named
> source; every performance claim is a measurement with a method attached, or
> it is labelled an estimate. Its first decision is proposed as
> [ADR-0029](../adr/0029-now-playing-and-kiosk.md). **Its second and third
> decisions — level metering, and the visualizer — are deliberately not in
> that ADR**, for the reason §9.0 gives: each one needs a refusal overturned,
> and a refusal overturned inside an ADR about something else is a refusal
> deleted rather than beaten.
>
> The short version. **The screen is a place, not a second window** — iced
> 0.13 cannot put a window on a monitor you name (§0.3, verified against the
> installed source), so *"full screen on your other monitor"* is delivered by
> the gesture that already works on both display servers: drag the window
> there, press one key. **Nothing on it moves that is not data arriving**, and
> the redraw rate while music plays is the engine's existing 4 Hz
> (`engine.rs:562`) — so ADR-0020's 0.0 % idle claim is not merely survived
> here, it is *tested* here, for eight hours at a time. **Burn-in is answered
> by the palette rather than by motion**: the room is `#0C0D0E`, the bright
> region is the artwork, and the artwork changes every few minutes on its
> own. **The screen is excellent with the network unplugged**, because the
> five things worth saying about a record are already on disk — and one of
> them, the signal path, is already computed and currently dead code
> (`player.rs:2016–2027`). Enrichment is a strictly additive opt-in layer that
> the composition must not have a hole in when it is off. **VU is admitted as
> data and refused as furniture.** **The visualizer is deferred with a stated
> price**, because it is the one thing in this document that genuinely costs a
> redraw while the window is otherwise idle.

---

## 0. What decides this

### 0.1 The constraint set already in force

This study does not get to invent freely. The refusals ledger, nine
composition laws (`.interface-design/system.md` §13), and ADR-0020's motion
law bound every answer below. The entries that bind hardest, and what each
one forbids here:

| Entry | Where | What it forbids on this surface |
|---|---|---|
| **No motion that costs anything when nothing is moving** | `REFUSALS.md:209–210`, ADR-0020 | A drifting gradient, a pulsing glow, a rotating record, a spectrum analyser at rest. The clause the rest hangs on: *"anything requiring a redraw while the window is idle"* (`REFUSALS.md:243–245`) |
| **Nothing is ever drawn on top of a sleeve** | `REFUSALS.md:89–91` | Text over the artwork, a scrim to make text legible over it, a play overlay, a duration chip. *"The only thing that touches artwork is light around it"* |
| **No artwork is ever drawn larger than its source** | `REFUSALS.md:92–93`, `art.rs:44–48` | Upscaling a 320 px thumbnail to fill a 1440 px screen. `ART_MAX == THUMB_PX` is asserted in code (`theme.rs:5549–5550`) |
| **No scrim, ever** | `REFUSALS.md:94–95` | The blurred-cover-as-background pattern every streaming client ships |
| **Amber is never an opaque fill**, and states playback truth only | `REFUSALS.md:196–199`, system §5 | A meter in the accent; a large amber field; the accent on anything that is not *which record, which track, where the playhead is* |
| **No engagement stats. History records; it never performs** | `REFUSALS.md:55–61` | Streaks, charts, top-artists, listening-time totals. What is permitted is enumerated: the PLAYED key, the card's *"PLAYED — N times since YYYY"* with its date stamps, and the pull's weighting |
| **Every action has a visible, pointer-reachable control; no control's only affordance is hover** | `REFUSALS.md:174–175` | A kiosk whose transport appears on mouse-move. This is the mitigation for a toolkit with no accessibility tree, and ADR-0028 has just re-confirmed that it outranks a quietness preference |
| **Skeuomorphism: the record supplies vocabulary, never surface** | `REFUSALS.md:257–261` | Named in the ban list: **VU meters**, tonearms, wear, patina, *"any circle pretending to be a record"*. §9 engages this directly |
| **No snake oil** | `REFUSALS.md:274–276` | Any signal-path claim the path cannot demonstrate. *"The condition report is an archivist's note, never a sales pitch"* |
| **No cloud dependency; internet features individually opt-in** | `README.md:22–24`, `VISION.md` pillar 3 | A screen whose context section is empty without a network |
| **The bar's ratchet** | `REFUSALS.md:100–105` | Removing a bar slot for tidiness. Replacing a slot with *a better statement of the same fact* is the one permitted move |

Two further postures, not refusals but settled:

- **Places are the whole surface model.** *"The window holds one place at a
  time, and the now-playing bar is in every one of them"*
  (`0022-places-and-nothing-else.md:74–75`, `place.rs:6–7`). Five members
  today; adding a sixth is cheap, and adding a *kind* is not.
- **Nothing in the bar changes size as playback moves**
  (`bottom_bar.rs:74–86`) — the reserved-slot promise, tested. Whatever this
  screen states, it must state in slots that are the same width when the
  track changes.

### 0.2 What baz already knows, without asking anyone

The sovereignty constraint (§8) is not a limitation to be worked around here;
it is the design's advantage, because the inventory is much richer than the
bar has room for. Everything below is on the user's disk today, needs no
network, and needs no new engine work unless the last column says so.

| Fact | Where it lives now | On screen today? |
|---|---|---|
| Title, artist, album, album artist, year, track/disc no., genre | the scan's tag read (`lofty`), folded into `vm::AlbumVm` / the index | bar and record page |
| Embedded artwork, or `cover.jpg`/`folder.jpg` beside the music | `art.rs:55–68` — `ArtSource::Embedded(Vec<u8>)` or `File(PathBuf)`, decoded to `THUMB_PX` 320 (`art.rs:48`) | wall, record page |
| Format, bit depth, sample rate, duration | the condition report (`FLAC · 16-bit · 44.1 kHz`), system §9 | record page |
| **The signal path** — source rate, output rate, whether baz converts and why, whether it holds the device exclusively, and whether any gain stage touches the samples | `Event::SignalPath` → `PlayerState::signal` (`player.rs:1034–1041`); `SignalChain` (`protocol.rs:931`) is `Direct` / `Converting{reason}` / `Exclusive{conversion}`; `VolumePath` (`protocol.rs:1048`) is `Unity` / `SoftwareGain` / `DeviceAttenuator` | **Partly.** `signal_note()` (`player.rs:2048`) renders the converting case and the word `bit-perfect`. **`signal_path()` — the whole reading — is dead code in non-test builds** (`player.rs:2016–2027`), kept deliberately, with the reason in the annotation: *"a diagnostics readout is ADR-0009's next step"* |
| ReplayGain, track and album, tag-read or baz-measured | ADR-0013 / ADR-0015; `replaygain.rs` | Settings, record page |
| **The history ledger** — plays, skips, first played, last played, total delivered | `TrackHistory` (`history/read.rs:140–159`), folded from `history.tsv` by `History::read` | **Only as the PLAYED group key.** The other permitted surface — the card's *"PLAYED — N times since YYYY"* — **lost its home when ADR-0022 deleted the inspector** |
| The queue as proportional segments with record breaks, and where the playhead is in it | `NeedleBar` / `NeedleEntry` (`player.rs:516–552`), already hit-tested by the bar's needle | bar, at 2 px |
| What this run came from | `queue_provenance()` (`player.rs:1722`) — *"from Sunday Morning.m3u"* | Queue place summary |
| What follows this track | `continuation_note()` (`player.rs:1695`) — *"then 2 albums · 1:58:00 left"* | bar, third line |

Two honest absences, stated here so no section below quietly assumes them:

- **baz does not read embedded lyrics.** Nothing in the scan asks for them.
  The brief's *"embedded lyrics if present"* is therefore a **new capability**,
  not an inventory item; `lofty` can read the frames, and §8.3 prices it.
- **baz holds no MusicBrainz IDs.** Every enrichment source worth having is
  MBID-keyed (`VISION.md` pillar 5), so the opt-in layer's first step is an
  identifier baz does not currently store — which is the strongest single
  argument for the *local* screen being finished before the layer is designed.

**The finding this inventory produces**: three of baz's best facts are
currently homeless or nearly so — the full signal path (dead code awaiting a
readout), the ledger's card (its surface deleted), and the needle's shape of
the run (2 px tall). A now-playing screen is not a new appetite for data. It
is the room those three have been waiting for.

### 0.3 What the toolkit will actually do

Verified against the installed source in `~/.cargo/registry`, not from
memory. The four answers that decide §3:

1. **More than one window in one process: yes, but only through
   `iced::daemon`.** `iced::application` can open a second window, but every
   window renders the same `view`; a daemon's `view` takes a `window::Id` and
   can answer differently per window. baz is an `application` today. The
   migration is real but shallow — the entry point, the `view`/`title`
   signatures, and a window-id-keyed piece of state.
2. **Choosing which monitor a window opens on: no.** iced 0.13 exposes no
   monitor enumeration and no monitor handle. `window::Settings::position`
   offers `Position::Specific`, which is an X11-only pixel offset and inert on
   Wayland. Underneath, winit *does* model it (`Fullscreen::Borderless(Option<MonitorHandle>)`),
   so this is a gap in iced's surface, not in the platform — but it is not
   reachable from baz today without patching the toolkit.
3. **Fullscreen: yes, borderless.** `window::change_mode(id, Mode::Fullscreen)`
   — and it lands on **the monitor the window is currently on**. That single
   fact is what makes §3's answer both possible and cheap.
4. **Redraw discipline: the event loop parks.** iced 0.13's winit loop uses
   `ControlFlow::Wait` when no window has requested a redraw, so a window with
   no animation and no events costs no wakeups — which is the mechanism behind
   ADR-0020's measured 0.0 %. One caveat that matters for §10: control flow is
   **coalesced across windows**, so in a two-window program one animating
   window keeps the whole loop awake.

And one platform note that costs nothing: **baz already speaks D-Bus.** MPRIS2
ships (`README.md:127–137`) over `zbus`, on its own session-bus thread, with
the exact posture §11 needs — *"with no D-Bus session bus, baz prints one line
and runs exactly as before."* Screensaver inhibition is the same bus, the same
optionality, and the same failure mode.

---

## 1. The case for the surface, and where it was already anticipated

The owner's brief reads as a new appetite. It is not: it is the third time
this product's own documents have circled the same shape without building it.

**Doc 03 wrote the charter, in the Winamp section**
(`03-interface-prior-art.md:610–618`). On the windowshade — 275 × 14 px
retaining a working transport, which *"nobody has copied"*:

> baz's now-playing bar is already this shape; the idea worth borrowing is
> that **the player can become the *whole* window when the collection is not
> what you need.**

That sentence is this document's thesis, written eleven months of design ago
and never cashed. The windowshade is the player at its *smallest*; the kiosk
is the same idea at its largest, and both rest on the same observation — that
browsing and listening are different activities and the second one does not
need the wall.

**ADR-0009 wrote the requirement, and left the code in place.** The
`signal_path()` annotation (`player.rs:2016–2027`) does not say *unused*; it
says the full reading is kept *"because the state is the honest thing to hold
and a diagnostics readout is ADR-0009's next step."* Karl — the persona whose
indispensable list is *"a status readout proving the chain (source rate →
output rate, no resampling)"* (`research/05-personas.md:35`) — has been
served a two-word note in a 96 px slot while the whole answer sat in memory
with a `dead_code` annotation on it.

**ADR-0022 created the vacancy.** The refusals ledger permits the history
ledger exactly three surfaces, and one of them is *"the inspector card
('PLAYED — N times since YYYY', plus a column of date stamps)"*
(`REFUSALS.md:58–60`). ADR-0022 deleted the inspector. The permission
survived its surface; the fact has had nowhere to stand since.

So the surface is not being invented to satisfy a brief. **It is the room
three existing, argued, already-permitted facts have been waiting for** — and
the brief is what makes building it now the obvious move rather than a
speculative one.

### 1.1 The workflow, and its band

`03` §1.2 ranks by frequency, and this surface sits oddly on that ladder
because it is not a workflow at all — it is a **session-long posture**. The
honest ranking is by *what it is for*, and there are three uses in the brief
and its neighbourhood:

| Use | Who | Frequency | What it needs |
|---|---|---|---|
| Leave it running on a second display for an evening | the brief, verbatim | entered once, held for hours | Legible **across a room**; costs nothing while it sits; survives eight hours on an OLED |
| Look at what is playing, properly, on the machine you are working at | W12 *get back to what is playing* — band A (`03` §1.2) | 10–100× a session | One key in, one key out, no loss of place |
| Verify the chain | W16, band D; Karl | ~weekly | The whole signal path, stated flatly, no boast |

The first is the one the design must be **optimised** for, because it is the
one with a hazard attached (§7) and the one no other surface in baz can be
retargeted to serve. The second and third fall out of it for free — which is
the test of whether the composition is right, and §5 is built to meet it.

### 1.2 The two viewing distances, which is the whole design

Everything difficult about this surface resolves once one observation is made
explicit:

> **This screen is read at two distances that do not overlap, and it is
> operated at only one of them.**

At **3 m** — the sofa, the other desk, across the room — the eye can resolve
the artwork, a large title, and a handful of lines set for the far field.
`SIZE_BODY` 13 is not merely small at that distance; it is *not there*. At
**60 cm** — sitting at the machine, reaching for the transport — everything
in the room is legible, including the bar.

This is not a compromise to be split. It is two audiences for one frame, and
they want opposite things: the far field wants very few, very large
statements; the near field wants controls and detail. A single composition
serves both if, and only if, the near-field material is **already in the
window and already learned** — which it is, because the bar is in every place
(ADR-0022) and the listener has used it forty times a session.

So the resolution §5 and §6 are built on:

> **The bar is the near-field surface. The place above it is the far-field
> surface. They are not competing for one reader, because at 3 m the bar is
> invisible and at 60 cm the far-field type is merely large.**

That is what lets this screen be a kiosk *and* keep every visible control the
accessibility refusal requires — and it costs exactly nothing, because both
halves already exist.

---

## 3. The screen: a place, and two acts

### 3.1 `Place::NowPlaying` — the sixth member

The surface is a **place**: a sixth member of `place.rs`'s enum, with the door
behaviour every other member already has (`place.rs:109–152`'s shape — a door
that closes itself and nothing else), and `back()` returning `Library` like
all of them.

The alternative — a second window — is refused on evidence rather than taste,
and the evidence is §0.3(2): **iced 0.13 cannot open a window on a monitor you
name.** A second window would appear wherever the compositor decides, which on
Wayland is "wherever it likes" and on X11 is a pixel offset that means nothing
on a multi-monitor desktop with a heterogeneous layout. So a second window does
not deliver the brief's *"your other monitor"* — it delivers *a second window
you must then drag anyway*, plus a `daemon` migration, plus §0.3(4)'s coalesced
control flow, plus a second surface to hold in the places model that ADR-0022
spent a whole decision reducing to one.

**What actually delivers the brief is one drag and one key**, and it works
today on both display servers:

> Put the window on the monitor you want it on — the gesture every desktop
> already has — press the door, press `F11`. The place fills that monitor,
> because `Mode::Fullscreen` lands on **the monitor the window is currently
> on** (§0.3(3)).

That is not a workaround; it is the shortest path to the stated goal, and it
composes with the platform instead of fighting it. Two notes that make it
better than it sounds:

- **The desktop can automate it.** Both major Linux compositors match window
  rules on `app_id`, and baz already ships a desktop entry and an `app_id`
  (`packaging/`). A user who wants baz to open fullscreen on `DP-2` every time
  writes three lines of compositor config. baz does not need — and should not
  grow — a monitor picker to serve that; the window manager is where that
  decision has always lived, and it is the one place that knows what the
  monitors are called.
- **The cost of the refused alternative is recorded, not hidden.** If iced ever
  exposes winit's `Fullscreen::Borderless(Option<MonitorHandle>)`, a *second
  window on a named monitor* becomes a twenty-line change on top of a `daemon`
  migration. §13 records that as the one thing that would reopen this decision,
  so re-proposing it means citing a toolkit change rather than re-arguing taste.

### 3.2 The two acts, kept separate

The brief bundles two things — *a dedicated now-playing screen* and *full
screen* — and the temptation is to build one control that does both. Doc 10
§3.1's second clause is exactly the rule against it: a control may not wear a
symbol whose convention promises one scope while acting on another. They are
two acts with two subjects, and they get two controls:

| Act | Subject | Control | Accelerator |
|---|---|---|---|
| Go to the now-playing screen | what is sounding → **the bar** (L8) | a word-door, `Now playing`, beside `Queue · N` | — |
| Make the window fill the display | this window → **the place's own body** (L8) | one glyph, the universal expand mark, in the place's top-right | `F11` |

Both are ordinary and both are visible, which is what the accessibility refusal
requires (`REFUSALS.md:174–175`). Three consequences, each argued:

- **The bar gains a slot, which the ratchet explicitly permits** — *"A slot may
  be added to the now-playing bar. None may be removed for tidiness"*
  (`REFUSALS.md:100–101`). It is a word and not a glyph because L8.4's door rule
  stands and the enumerated symbol list is closed at two, the gear and the
  magnifier (`system.md:876–879`); no universal symbol distinguishes *this
  screen* from *queue* from *playlist*, which is the same finding that kept the
  `Queue` door a word (doc 10 §3.4).
- **The door is offered even when nothing is sounding**, which is a deliberate
  departure from the now-playing block's rule (*"not offered when nothing is
  sounding, because a control that cannot act must not pretend it can"*,
  `0022-places-and-nothing-else.md:136–137`). The distinction is real: `Go to
  record` has no record to go to, whereas this place has something true to say
  with nothing playing (§5.5) — and setting the screen up *before* starting the
  music is the natural order for the brief's own use case.
- **Fullscreen is a window act, not a place act.** `F11` works in every place;
  it is not the kiosk's private key. This matters because the alternative —
  fullscreen that only exists inside one place — would make leaving the place
  and leaving fullscreen the same gesture, which is the micro-mode the ledger
  refuses in another form (`REFUSALS.md:113–118`).

### 3.3 Leaving: `Esc` peels, exactly as it already does

baz's `Esc` is already a peeling rule rather than a back button — *"leave the
search field, then peel one layer: the queue, else the settings, else the pull,
else the query…"* (`README.md:43`). Fullscreen and the place are two layers, so
they peel in the order they were put on:

1. **`Esc` in the fullscreen kiosk** → leaves fullscreen, stays in the place.
   This is the convention every browser and video player has taught, and
   breaking it here would strand a user who cannot find their window
   decorations.
2. **`Esc` again** → `Place::back()` → the Library, with the wall's scroll,
   query and arrangement untouched, exactly as leaving any other place.
3. The door and `‹ Library` do what they do in every place.

No new rule, no new key, and nothing about `Esc` changes anywhere else.

---

## 4. The user stories

The industry artifacts doc 09 §4 established: each scenario as a **user
story**, its **task flow** from where the listener actually is, and
**acceptance criteria** in Given/When/Then, written to be implemented and
tested as stated. Personas are `research/05-personas.md`'s.

### S1 — Leave it running on the other monitor

> As a listener with a spare display, I want to put baz's now-playing screen on
> it full screen and leave it there for the evening, so that the room has
> something worth looking at and my main screen is free for work.

**Task flow**: ① drag the window to the second monitor; ② press `Now playing`
in the bar; ③ press `F11`. Then nothing, for eight hours.

**Acceptance criteria**

- Given the window is on a given monitor, when `F11` is pressed, then the
  window fills **that** monitor and no other, and the place fills the window.
- Given the kiosk is showing and a track is playing, when nothing is
  interacted with for an hour, then the process's UI redraw rate is exactly the
  engine's `Event::Progress` cadence — 4 Hz (`engine.rs:562`) — and no clock,
  tween, or subscription in the front end is running that was not running in the
  Library place (§7.1).
- Given the kiosk is showing and playback is **paused or stopped**, when nothing
  is interacted with, then **no frame is drawn at all** beyond the one that
  stated the change: `Progress` is not emitted while paused or stopped
  (`protocol.rs:435–438`, enforced structurally by the pump's pause gate at
  `engine.rs:1518–1523`), and the front end installs no timer of its own here.
  The measured idle claim of ADR-0020 holds on this surface unmodified.
- Given eight hours have passed with music playing, when the display is
  examined, then the only pixels that held one colour throughout are the room
  itself and the bar's furniture, both at or near `#0C0D0E` (§7.2).
- Given the track changes, when the new record's artwork and type arrive, then
  they replace the old ones as a **hard cut** — no crossfade, which is refused
  by name (`REFUSALS.md:241–243`, ADR-0020 §3).

### S2 — Look at what is playing, properly

> As a listener at the machine, I want one press to see the record I am
> hearing at a size worth looking at, so that "what is this?" is answered
> without leaving what I was doing for long.

**Task flow**: ① `Now playing` in the bar; ② look; ③ `Esc`.

**Acceptance criteria**

- Given any place, when `Now playing` is pressed, then the kiosk replaces it as
  an ordinary place change — a **hard cut**, per ADR-0022's rule 4 and
  `REFUSALS.md:229–231` (*"the surfaces either side of a navigation share no
  element to move"*).
- Given the kiosk, when `Esc` or `‹ Library` is pressed and the window is not
  fullscreen, then the Library returns with its scroll, query and arrangement
  untouched (`place.rs:41–46`).
- Given the kiosk is showing, when a track change occurs, then every readout on
  it updates from the same `PlayerState` the bar reads, so the two surfaces
  cannot disagree — the property `bottom_bar.rs:85–86` already asserts for the
  bar's own zones.
- Given the kiosk is showing, when the pointer rests anywhere on it, then no
  control appears that was not already visible (§6).

### S3 — Verify the chain

> As Karl, I want the whole signal path stated in one place — what the file is,
> what the device is running at, whether anything converted it, and whether any
> gain stage touched the samples — so that I can confirm the claim rather than
> trust it.

**Task flow**: ① `Now playing`; ② read the signal register.

**Acceptance criteria**

- Given a `SignalPath` event has been received, when the kiosk renders, then it
  states the **whole** reading from `PlayerState::signal_path()`
  (`player.rs:2026–2028`) — source rate, output rate, the chain's state, the
  conversion reason when there is one, and the volume path — not the bar's
  two-word summary.
- Given the chain is `Exclusive { conversion: None }`, when the kiosk renders,
  then it says so — **which the bar today does not**, because `bit_exact()`
  compares `chain == SignalChain::Direct` exactly (`player.rs:1541–1545`) and an
  exclusive chain therefore renders no note at all (`player.rs:2051–2053`).
  §12 step 3 records this as a defect found by this study, with the protocol's
  own guidance as the fix (`protocol.rs:924–927`: ask through `is_exclusive()`
  and `conversion_reason()`, do not enumerate variants).
- Given no `SignalPath` has been received — nothing has played this session —
  when the kiosk renders, then the signal register is **absent, not empty**, and
  states nothing. A slot that says nothing is not filled with a dash.
- Given any state, when the register renders, then its words are flat: no
  "degraded", no "fallback", no boast, no badge, no colour carrying a verdict
  (ADR-0009 §5, and `REFUSALS.md:274–276`).

### S4 — Be told something you did not know

> As Devon, I want the screen to tell me something about this record that I
> would not otherwise have thought about, so that leaving it on is rewarding
> rather than merely decorative.

**Task flow**: ① `Now playing`; ② read.

**Acceptance criteria**

- Given the ledger has records for the sounding track, when the kiosk renders,
  then it states the permitted card — *"PLAYED — N times since YYYY"* — in the
  ledger's own permitted form (`REFUSALS.md:58–60`), read from
  `TrackHistory` (`history/read.rs:140–159`).
- Given the ledger has **no** record for this track, when the kiosk renders,
  then it says so as a positive statement — *"not played before"* — which is
  what the ledger actually knows (`Recency::Never` is the positive statement,
  distinct from `Unrecorded`, `0018-play-history-ledger.md:5–11`).
- Given any state, when the ledger register renders, then it shows **no totals,
  no streaks, no charts, no listening-time sum, and no cross-track
  aggregation** — `TrackHistory::listened_ms` exists and is deliberately not
  rendered (§8.2), because a listening-time total is refused by name
  (`REFUSALS.md:55–57`).
- Given a play is recorded while the kiosk is showing, when
  `Event::PlayRecorded` arrives, then the ledger register re-reads and the count
  is current. Today the history snapshot is read **once at open**
  (`app.rs:3422–3431`) and `PlayRecorded` has no consumer in `crates/baz` at
  all; §12 step 4 is that wiring, and it is the only place in this document
  where a kiosk readout needs new plumbing rather than a new view.

### S5 — Nothing is playing

> As anyone who walks past the screen, I want it to be honestly quiet when the
> music has stopped, so that the machine is not pretending.

**Acceptance criteria**

- Given the queue has ended or playback is stopped, when the kiosk renders,
  then it states silence in words, holds no artwork, and draws no transport
  state that is not true — the queue place's own posture
  (`views/queue.rs:190–196`) at room scale, and the ledger's *"silence is a
  feature"* (`REFUSALS.md:19–21`) drawn rather than merely honoured.
- Given the stopped kiosk, when it has been stopped for any length of time,
  then it is drawing **no frames** and the surface is overwhelmingly the room's
  own `#0C0D0E` — which is simultaneously the honest empty state and the whole
  of the burn-in answer for the static case (§7.2).
- Given the engine was never built with `device-output`, when the kiosk is
  opened, then it says what the bar says — the availability note
  (`player.rs:2005–2012`) — rather than an empty frame.

### S6 — The artwork is missing

> As Marta, whose older rips have no embedded cover, I want the screen to be
> composed rather than broken when there is no picture.

**Acceptance criteria**

- Given no `ArtSource` resolves for the record (`art.rs:68–70` returns `None`),
  when the kiosk renders, then it draws the wall's own deterministic gradient
  placeholder at kiosk scale — the same object the shelf already draws, not a
  new empty-state illustration.
- Given the source artwork is **smaller** than the kiosk's target size, when it
  renders, then it is drawn **at its own pixel size**, centred, and never scaled
  up — the refusal *no artwork is ever drawn larger than its source*
  (`REFUSALS.md:92–93`) applied to a surface whose target is much larger than
  the wall's, and asserted by the kiosk's own test (§12 step 2), the way
  `the_wall_never_draws_art_larger_than_its_source` (`shelf.rs:1513`) asserts it
  for the wall.
- Given the artwork is missing, when the composition renders, then **no lane
  moves**: the type block sits where it sits whether or not there is a picture,
  because reserved slots are the bar's promise (`bottom_bar.rs:74–86`) and this
  surface inherits it.
