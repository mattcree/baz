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
