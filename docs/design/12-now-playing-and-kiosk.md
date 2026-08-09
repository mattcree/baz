# 12 — The now-playing screen: a surface for the far field

> The owner, verbatim, on what this surface is to become:
>
> *"now playing does not need the play pause controls. it would be nice if the
> album art was somehow more prominent, like it takes up the background and has
> some nice VU meter stuff over it in a stylised way, maybe somewhat ambient…
> I also like the idea of just seeing related stuff appearing in like a feed of
> random facts. I think this kinda stuff would naturally be toggle-able"*
>
> Earlier, on the same surface: *"a kiosk like look in the sense that you can
> just leave it full screen on your other monitor"*, and *"maybe we could have
> a visualizer mode at some point, but also VU options"*.
>
> And the ruling that settles §7, given while this draft was being finished:
> **"ambient motion is fine as long as the performance remains top tier."**
>
> A design study, not an implementation. Written 2026-08-09 against `ee96016`.
> Every claim about shipped behaviour is cited `file:line`; every prior-art
> claim carries a named source; every performance claim is a measurement with a
> method attached, or it is **labelled an estimate**. Its decisions are proposed
> as [ADR-0029](../adr/0029-the-ambient-surface.md).
>
> **What changed since this document was started.** `Place::NowPlaying`
> shipped (`views/now_playing.rs`), and it shipped with the composition this
> study had specified: artwork large, identity under it, the needle, every
> measure derived from the viewport and swept 400–4000 px by its own test
> (`now_playing.rs:218–234`). So the kiosk is not a second design; it is this
> surface with a bigger number in it, and that is now a property of the
> arithmetic rather than a plan. What this revision does is take the surface
> from *correct* to *worth leaving on*.
>
> The short version, decision by decision.
>
> 1. **The transport comes off the surface.** It is drawn twice today — the
>    place calls the bar's own `transport()` (`now_playing.rs:168`) while the
>    bar draws it 24 px below (`app.rs:3744–3752`). Same function, same state,
>    two copies, one screen. §6.
> 2. **The artwork becomes the field, and the refusal it argues with is already
>    broken.** `NOW_PLAYING_MAX` is 720 px (`now_playing.rs:81`) and the cache
>    it draws from is 320 px (`art.rs:48`) — the shipped surface upscales
>    2.25× at 1920×1080 today, which is *no artwork is ever drawn larger than
>    its source* being false in the one place the ledger never checked. §5
>    resolves it with a **derived ambient field**: not the artwork, a *reading*
>    of it, in the same sense the ledger already calls an art-derived lamp
>    **data**. The refusal is rewritten to say what it always meant.
> 3. **The meter is a real measurement or it is furniture.** It is **not a VU**;
>    it is a **momentary-loudness meter to EBU R128 / ITU-R BS.1770-4** with a
>    sample-peak overlay, because baz already owns that filter, derived and
>    vector-tested (`loudness.rs:1`, ADR-0015 §1). Ballistics are specified,
>    the standard is named, and *"VU options"* is answered with three
>    ballistics rather than one. The tap is **read-only by type** — `&[f32]`,
>    never `&mut` — so bit-exactness is not promised, it is unspoiled by
>    construction. §9.
> 4. **The feed is excellent with the network unplugged**, because the ledger,
>    the tags, the measured loudness and the signal path are already on disk.
>    Its rotation rule is one sentence, and it falls on the permitted side of
>    *history records, it never performs* for a reason §8.4 states rather than
>    assumes. Enrichment is a strictly additive opt-in layer the composition
>    must not have a hole in when it is off.
> 5. **Ambient motion is the thing, not the concession.** The owner asked for
>    it; ADR-0020 gains the class rather than being violated by it. The bar is
>    performance, and §7 pays it in numbers with a stated method. **The rest of
>    the product's idle is untouched structurally** — the subscription is a
>    function of state (`app.rs:3900–3908`), so with the place off screen there
>    is no timer to be careful about. **Burn-in stops being an argument** and
>    becomes the one hazard ambient motion happens to fix.
> 6. **Kiosk mode is one drag and one key**, because iced 0.13 cannot put a
>    window on a monitor you name (§0.3, verified against the installed
>    source). §11.

---

## 0. What decides this

### 0.1 The constraint set already in force

This study does not get to invent freely — but four of the entries that used
to bind it hardest are entries **the owner has now reversed**, and the ledger's
own preamble settles what that means:

> *"Contributors and agents — not the owner. **The owner's decision is
> sufficient on its own**; an entry he reverses gets rewritten to say what was
> decided and why, and that is the whole of the process. Nobody argues with a
> document to change their own product."*
> (`REFUSALS.md:6–14`)

So the table below is in two halves, and the split is the honest one. Nothing
here is an argument against him; the second half is a record of decisions, and
§14 carries the rewritten entries in the ledger's own voice.

**Still binding, and they shape every answer below:**

| Entry | Where | What it holds to on this surface |
|---|---|---|
| **Every action has a visible, pointer-reachable control; no control's only affordance is hover** | `REFUSALS.md:249–250` | No kiosk whose controls appear on mouse-move. This is the mitigation for a toolkit that publishes no accessibility tree, and ADR-0028 re-confirmed it outranks a quietness preference (`REFUSALS.md:88–98`). It is what makes §6's *drop the transport* legal — the bar keeps it, visibly, in this place as in every other |
| **No state is signalled by colour alone** | `REFUSALS.md:259–261` | A meter that is only a colour change; a fact whose "new" is only a tint |
| **Amber is never an opaque fill**, and states playback truth only | `REFUSALS.md:271–274` | The accent on anything that is not *which record, which track, where the playhead is*. §9.6 is why the meter is **not** amber |
| **No snake oil** | `REFUSALS.md:348–350` | Any signal-path claim the path cannot demonstrate. This binds the meter hardest of all: a meter that contradicts `bit-perfect` would be exactly the unearned claim this entry exists to stop |
| **No engagement stats. History records; it never performs** | `REFUSALS.md:68–73` | Streaks, charts, top-artists, listening-time totals. §8.4 states which side the rotating fact falls on, and why, rather than assuming it |
| **No cloud dependency; internet features individually opt-in** | `README.md:22–24`, `VISION.md` pillar 3 | A screen whose feed is empty without a network. §8 is built local-first for this reason and not merely in deference to it |
| **The bar's ratchet** | `REFUSALS.md:137–142` | Removing a bar slot for tidiness. Replacing a slot with *a better statement of the same fact* is the one permitted move — and §6 is the mirror case: the *place* drops what the bar already says |
| **No borders on artwork**; **no shadows** except the playing halo | `REFUSALS.md:279–282` | A frame or drop-shadow to separate the cover from the field behind it. §5.4 solves that separation with light, which is the one thing permitted |

**Reversed by the owner, and rewritten in §14** — each of these is the entry
this study would otherwise have had to break:

| Entry as it stood | Where | What the owner decided |
|---|---|---|
| **No artwork is ever drawn larger than its source.** `ART_MAX == THUMB_PX`, asserted in code | `REFUSALS.md:123–124`, `art.rs:44–48` | *"the album art was somehow more prominent, like it takes up the background"*. §5 draws the true-size cover **and** a derived field behind it; §14.1 rewrites the entry around the distinction the code already needs. Note this entry is **already false on this surface today** (§0.4) |
| **No scrim, ever** | `REFUSALS.md:126–132` | The field is not a scrim — §5.3 makes the argument the ledger's own hover-veil amendment already made once (`REFUSALS.md:113–121`), and §14.2 records it |
| **Skeuomorphism: banned — vinyl discs, wood grain, tonearms, VU meters, wear, patina** | `REFUSALS.md:330–335` | *"some nice VU meter stuff over it in a stylised way"*. §9 distinguishes the **instrument** (banned surface: beige panel, glass face, swinging needle) from the **measurement** (permitted data), which is the entry's own *"physics, structure and vocabulary… never surface"* applied rather than overridden. §14.3 |
| **No motion that costs anything when nothing is moving**; *anything requiring a redraw while the window is idle* | `REFUSALS.md:284–320`, ADR-0020 | *"ambient motion is fine as long as the performance remains top tier"*. ADR-0020 **gains a sixth class** rather than losing its rule: user-started ambient content, on a surface whose whole purpose is to be looked at. §7, and §14.4 |

Two further postures, not refusals but settled, and both survive intact:

- **Places are the whole surface model.** *"The window holds one place at a
  time, with the returns lane to its left in every place but Settings, and the
  now-playing bar under all of them"* (`place.rs:5–7`). Seven members today,
  `NowPlaying` among them.
- **Nothing in the bar changes size as playback moves**
  (`bottom_bar.rs:74–86`) — the reserved-slot promise, tested. Whatever this
  screen states, it must state in slots that are the same width when the
  track changes.

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

Verified against the installed source in `~/.cargo/registry`, not from memory.
baz resolves to **iced 0.13.1** (iced_winit 0.13.0, iced_widget 0.13.4,
iced_graphics 0.13.0), with `features = ["advanced", "image", "tokio"]` and
default features on (`Cargo.toml:26`). Seven answers, and the last three decide
§5, §7 and §9 rather than §11:

1. **More than one window in one process: yes, and `iced::daemon` is not
   feature-gated** (`iced/src/lib.rs:490`, `:629`). `iced::application` can
   already open a second window — the shell has a full `WindowManager` — but
   `Program::view` **discards** the window id for an application
   (`application.rs:116–122`) and forwards it for a daemon (`daemon.rs:66–72`),
   so every window of an application renders the same `view`. The one runtime
   difference is `run(settings, Some(window))` vs `run(settings, None)`
   (`application.rs:167`, `daemon.rs:115`), read as `is_daemon` at
   `iced_winit/src/program.rs:205`. *(The `multi-window` cargo feature is
   vestigial for this purpose: it gates only a legacy trait module, and there
   are zero `cfg(feature = "multi-window")` in iced_winit.)*
2. **Choosing which monitor a window opens on: no.** `window::Settings`
   (`iced_core/src/window/settings.rs:33–76`) has no monitor field, and there
   is **no monitor enumeration in the public API at all** — `MonitorHandle`,
   `available_monitors` and `primary_monitor` appear nowhere in `iced`,
   `iced_core` or `iced_runtime`, only in iced_winit's internals
   (`program.rs:510`, `window_manager.rs:104`). winit's
   `Fullscreen::Borderless(Option<MonitorHandle>)` *is* used
   (`conversion.rs:398`) but never re-exported; the `Option` is always iced's
   choice, never yours. `Position::Specific(Point)` exists and converts to a
   bare `LogicalPosition` with **no monitor offset applied**
   (`conversion.rs:326–331`) — i.e. global desktop coordinates — so it would
   work *if* you knew the target monitor's origin, and iced gives you no way to
   learn it. Nor do the escape hatches: `window::run_with_handle` yields a
   `raw_window_handle::WindowHandle`, not winit's `Window`
   (`iced_winit/src/program.rs:1404–1413`), so there is no `current_monitor()`
   to ask.
3. **Fullscreen: yes, borderless, and always on the monitor the window is
   already on.** `window::change_mode(id, Mode::Fullscreen)`
   (`iced_runtime/src/window.rs:323`) reaches
   `window.raw.set_fullscreen(conversion::fullscreen(window.raw.current_monitor(), mode))`
   (`iced_winit/src/program.rs:1331–1338`). `Mode`'s own doc says it:
   *"the whole screen of its **current monitor**"* (`iced_core/src/window/mode.rs:7`).
   That single fact is what makes §11's answer both possible and cheap.
4. **Redraw discipline: the event loop parks in `Wait`.** After each redraw,
   the flow is set from the UI state — `Wait` unless a `RedrawRequest` is
   outstanding (`iced_winit/src/program.rs:830–846`) — and `ControlFlow::Poll`
   appears only on the wasm boot path (`program.rs:358–359`). This is the
   mechanism behind ADR-0020's measured 0.0 %. Two caveats that matter below:
   the flow is **one global setting, not per-window**, with explicit coalescing
   arms (`program.rs:471–488`); and after any message batch **every** window is
   redraw-requested (`program.rs:1089–1097`), so a timer subscription paces the
   whole process, not one surface.
5. **`window::frames()` is vsync-paced, not a timer** — it listens for
   `RedrawRequested` (`iced_runtime/src/window.rs:164–177`), which is
   synthesized from winit's own event (`iced_winit/src/program.rs:785–787`).
   Its docs say the rate is *"the refresh rate of the first application
   window… normally managed by the graphics driver and/or the OS"*. It yields a
   bare `Instant` with the window id **ignored**, so it cannot tell you which
   window drew. `window::redraw_events()` **does not exist in 0.13**; the whole
   set is `frames`, `events`, `open_events`, `close_events`, `resize_events`,
   `close_requests`.
6. **`Canvas` exists but is not compiled into baz today.** It is gated on the
   `canvas` feature (`iced_widget/src/lib.rs:116–121`), which chains
   `iced/Cargo.toml:105` → `iced_widget/Cargo.toml:91` → `iced_renderer/geometry`
   and is **not** in `default`. Enabling it is a one-line manifest change and
   **no new crate**. The important negative: its `Cache` does *not* help a
   moving meter. `Cache` skips tessellation only while the geometry is
   unchanged — *"will not redraw its geometry unless the dimensions of its layer
   change or it is explicitly cleared"* (`iced_graphics/src/geometry/cache.rs:7–10`,
   fast path at `:76–79`) — so a value that moves means `clear()` and a full
   re-tessellation **every frame**. There is no dirty-region invalidation.
   `Cache::with_group` (`cache.rs:41–45`) is the mitigation: static chrome in
   one cache, the moving mark in another, so only the small thing re-tessellates.
7. **The `shader` widget — a real custom wgpu pipeline — is available right
   now, with no manifest change.** It is gated on the `wgpu` feature
   (`iced_widget/src/lib.rs:95–100`), and `wgpu` is in iced's `default`
   (`iced/Cargo.toml:107–112`, `:137–140`), which baz does not disable. The
   trait is `Program<Message>` with `draw(&self, state, cursor, bounds) ->
   Self::Primitive` (`iced_widget/src/shader/program.rs:13–62`), and the
   `Primitive` gets `prepare`/`render` against the live device and queue;
   `iced_widget/src/shader.rs:21` re-exports raw `wgpu` itself.
   **The caveat that shapes §7.5**: the shader widget only renders under the
   wgpu backend, and `tiny-skia` is also in iced's default features as a
   fallback. On a machine that falls back to software rendering, a shader
   widget draws nothing — so anything built on it needs a defined degradation,
   not a blank rectangle.

And one platform note that costs nothing: **baz already speaks D-Bus.** MPRIS2
ships (`README.md:127–137`) over `zbus`, on its own session-bus thread, with
the exact posture §11 needs — *"with no D-Bus session bus, baz prints one line
and runs exactly as before."* Screensaver inhibition is the same bus, the same
optionality, and the same failure mode.

### 0.4 What shipped, and the defect in it

`Place::NowPlaying` exists (`views/now_playing.rs`, 249 lines) and is routed at
`app.rs:3670–3676`. Its composition is a column of three: the work, the placard
(artist in tracked caps, title at `SIZE_HERO`, album, the needle, the two
figures), and the transport (`now_playing.rs:164–174`). Its one derived measure
is `art_edge` (`now_playing.rs:58–72`):

```
below   = LINE_HEADING 12 + LINE_HERO 32 + LINE_BODY 20
        + NEEDLE_H 2 + TRANSPORT_HIT 32 + 4 × GAP_LG 64        = 162
edge    = min(width − 2×HANG, height − 2×HANG − below)
            .clamp(ART_MIN 240, NOW_PLAYING_MAX 720)
```

**Two findings, and the second is a defect.**

**(a) The transport is on this screen twice.** `now_playing.rs:168` calls
`crate::views::bottom_bar::transport(player, ink)` — not a similar control, the
*same function* the bar composes — and the bar itself is appended under every
place unconditionally, gated only on whether the build has audio output at all
(`app.rs:3744–3752`). So the shipped surface draws play/pause, previous and
next, then draws them again one `GAP_XL` and one bar-height below. The owner's
*"now playing does not need the play pause controls"* is not a preference being
accommodated; it is a duplication being reported. §6.

**(b) The surface upscales artwork, and the ledger says it never does.** The
image handle comes from `shelf.thumbs.peek(&id)` (`now_playing.rs:106`), and
`Shelf::thumbs` is `LruCache<u64, iced_image::Handle>` (`app.rs:4034`) filled
from `art::load_thumb`, which is `image.thumbnail(THUMB_PX, THUMB_PX)` —
**320 px on the long edge, always** (`art.rs:131–139`, `art.rs:48`). It is then
drawn at `edge`, which the clamp allows up to 720.

| Window | `edge` | Source | Scale factor |
|---|---|---|---|
| 1280 × 800 | 558 | 320 | **1.74×** |
| 1920 × 1080 | 720 (at the ceiling) | 320 | **2.25×** |
| 2560 × 1440 | 720 (at the ceiling) | 320 | **2.25×** |
| 3840 × 2160 | 720 (at the ceiling) | 320 | **2.25×** |

The crossover is `edge > 320`, i.e. any window taller than 562 px and wider
than 400 px — which is every window baz is ever run in. The captures on main
show it: `docs/design/impl/lane-and-home/23-now-playing-1920.png` is a 320 px
thumbnail drawn at 720.

The code knows. `NOW_PLAYING_MAX`'s own doc comment
(`now_playing.rs:74–81`) reads *"Past this a cover stops gaining anything — the
decoded thumbnail is not that large, and a bigger square would be upscaling"* —
which concedes the thumbnail is not that large and then sets the ceiling at
720 regardless. The wall's guard (`shelf.rs:1509–1530`,
`the_wall_never_draws_art_larger_than_its_source`) asserts `ART_MAX == THUMB_PX`
for **the wall**, and there was no equivalent for a surface that had not been
written when it was.

This matters to §5 beyond bookkeeping. The refusal is not a wall the owner's
brief runs into for the first time — it is a wall the product already walked
through, quietly, and the honest move is to rewrite the entry to say what it
always meant (§14.1) *and* give the surface a decode path that makes the
rewritten entry true (§5.2, §12 step 2). A ledger entry that is false in shipped
code is worse than no entry.

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

## 2. Prior art: five traditions, and what each one gets wrong

**On sourcing.** Doc 03 set this project's standard and also its warning:
*"reading a review saying 'Plexamp is beautiful' is [not enough]"*
(`03-interface-prior-art.md:38`), and it recorded an honest failure —
*"Plexamp's current layout was not seen… `plexamp.com` is a JS-rendered page
[and] the Plexamp UI now 301s to the support index"* (`03:104–106`). That
constraint has not changed. So the table below marks each claim by how it is
known, and **§13 ranks re-verifying the two weakest ones** before anything is
built on them.

| Source | Claim | Known how |
|---|---|---|
| **Apple Music** full-screen | The background is **twisted and blurred copies of the artwork in a Metal shader, not colour sampling** | doc 03's own audit, `03:235` |
| **YouTube Music** | A **blurred enlarged copy of the art behind itself**, while a 48 px thumbnail in the bar duplicates 860 px of art on the same screen | doc 03's own audit, `03:232` |
| **Amberol** | **No blur of the art at all** — the whole window is washed with a **three-gradient composite built from the cover's palette** | doc 03's own audit, `03:236` |
| **Spotify** | Page header gradient from the **dominant colour** | doc 03's own audit, `03:233` |
| **Winamp** windowshade | 275 × 14 px retaining a working transport; *"the player can become the whole window when the collection is not what you need"* | doc 03's own audit, `03:610–618` |
| **Roon** "Display" mode | A dedicated full-screen now-playing view intended for a second screen or a tablet left on a shelf | product documentation; **not independently verified here** — §13 R2 |
| **Plexamp** screensaver / visualizer | A now-playing surface that becomes ambient after idle | doc 03 recorded it could not fetch the UI (`03:104–106`); **not independently verified** — §13 R2 |
| **foobar2000 / Winamp** visualization culture | MilkDrop, AVS, and the spectrum-analyser default; 102,634 Winamp skins, *"half illegible"* | doc 03's skinning analysis, `03:592–606` |

### 2.1 The finding: two families, and only one of them is honest

The four sourced treatments of "art as background" split cleanly, and the split
is the whole of §5's decision:

- **Blur the artwork itself** — Apple Music (shader), YouTube Music (enlarged
  copy). The background *is* the cover, transformed. It is beautiful, and it is
  the thing baz's own ledger objected to under two entries at once: it draws a
  copy of the work larger than the work, and the copy competes with the original
  a few hundred pixels away. YouTube Music's version is doc 03's own example of
  the failure — *"a 48 px thumb in the bar **duplicating the 860 px art on the
  same screen**"*.
- **Derive a palette and paint with it** — Amberol (three gradients composited
  from the cover's palette), Spotify (dominant colour). The background is **not
  the artwork**; it is a *reading* of it. Nothing is upscaled because nothing is
  copied.

**baz takes the second family, and it is not a compromise — it is the one that
matches what this product already believes.** The ledger already says colour
read from a record is data: *"the art-derived lamp is **data** — hue read from
the record, lightness and chroma pinned — not a preference"*
(`REFUSALS.md:267–269`). A field built from the cover's own palette is that
sentence at a larger size. §5.3 makes the argument in full, because it is the
one this study must make explicitly rather than assume.

### 2.2 What the visualization tradition actually teaches

MilkDrop and AVS are the strongest evidence in this document *for* the owner's
brief and the strongest *against* the way it is usually implemented. Two
findings:

- **People genuinely leave these running.** The tradition is not a footnote; it
  is why "visualizer" is a word. Doc 03's windowshade finding is the same
  observation from the other end — users kept a mode alive for two decades
  because *"letting you see what's playing… with minimal distraction"* is a real
  posture, not a niche one.
- **They are unbounded by design, and that is what makes them unshippable
  as-is.** A MilkDrop preset renders every frame at whatever cost it likes,
  because it was built for a foreground window on a machine doing nothing else.
  This surface is for a **second** monitor beside work. The owner's own bar —
  *"as long as the performance remains top tier"* — is precisely the constraint
  the tradition never had, and §7 treats it as the design's spine rather than
  its footnote.

### 2.3 The meter, where nearly everyone is wrong

Almost every "VU meter" in a music player is not one. The distinction is not
pedantry; it is the difference between a readout that means something and a
decoration that moves:

- A **true VU** is defined by its *ballistics*, not its looks: IEC 60268-17
  specifies a 300 ms integration to 99 % of a steady tone's reading, with
  1–1.5 % overshoot. It reads **average** level and deliberately misses
  transients. Its 0 VU is a reference level, not full scale.
- A **PPM** (IEC 60268-10) is the opposite instrument: ~10 ms integration to
  catch peaks, with a deliberately slow fallback (1.5–2.8 s per 20 dB,
  depending on type) so the eye can read what the ear missed.
- A **digital peak meter** is neither: it is sample-accurate with no
  integration at all, and the number it shows is dBFS.
- **Momentary loudness** (EBU R128 / ITU-R BS.1770) is a fourth thing: K-weighted
  mean square over a 400 ms window, which is the closest of the four to *how
  loud this sounds right now*.

Products that draw a beige panel with a swinging needle and drive it from a peak
sample are showing a PPM wearing a VU's clothes. §9 refuses to do that, and the
choice it makes is argued there.

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

---

## 5. The composition: a field, a work, a placard

### 5.1 The decision, first

> **The background is a *derived ambient field* — a wash built from the cover's
> own palette — and the artwork itself is drawn at its true size on top of it,
> never scaled beyond the pixels the file actually contains.**

Two things are being decided at once and they must not be confused:

1. **What fills the screen.** The field. It is *not the artwork*; §5.3 makes
   that argument explicitly, because it is the load-bearing one.
2. **How large the cover may be drawn.** As large as the viewport allows,
   bounded by **the source file's own pixels** — which means §5.2's decode tier
   is not an optimisation, it is what makes the refusal true for the first time
   (§0.4 b).

The rejected alternative — **full-bleed artwork with the entry amended** — is
rejected on three grounds, only one of which is the ledger:

- **It cannot be done honestly at 4K.** A 1000 px cover — a good one, by 2026
  standards — filling a 3840 px monitor is a 3.8× upscale. That is not a
  stylistic choice, it is a visibly soft image, and the surface's whole subject
  is a piece of visual art.
- **It is the thing doc 03 already caught being wrong.** YouTube Music's
  blurred enlarged copy sits behind a 48 px thumbnail *"duplicating the 860 px
  art on the same screen"* (`03:232`). Full-bleed plus a true-size cover is the
  same duplication; full-bleed *without* a true-size cover throws away the one
  object the surface exists to show.
- **It makes the composition unlayoutable.** A full-bleed image has an aspect
  ratio and the window has a different one, so something is always cropped —
  and cropping a record sleeve cuts the type off its own artwork.

The field has none of these problems, because a gradient has no resolution and
no aspect ratio. **It can fill a 4K panel exactly as well as a 1280 px one, and
that is not a workaround — it is the reason it is the right object.**

### 5.2 The decode tier, which makes the refusal true

baz decodes art exactly once, to 320 px (`art.rs:131–139`). That is right for a
wall drawing up to 120 tiles at `ART_MAX` 320 and wrong for a surface drawing
one work at 1000. So this surface gets **a second decode, of one record**:

```
art::load_hero(first_track) -> Option<(u32, u32, Vec<u8>)>
    the same resolution order (art.rs:69–78), decoded with
    image.thumbnail(HERO_PX, HERO_PX) — downscale-only, exactly as
    load_thumb does, so a small source stays its own size
HERO_PX = 1024
```

**Why 1024, and why a ceiling at all.** It is the largest edge that is smaller
than the shortest dimension of every panel this surface targets (1080 is the
smallest kiosk height), so the cover is never the thing limiting the layout —
the viewport is. And the memory is trivial next to the budget `art.rs:18–35`
already argues: `1024 × 1024 × 4 B = 4 MiB` per entry, **two entries** (the
sounding record and the one after it), against the thumbnail cache's 150 MiB.
That is a **5.3 % increase in baz's art memory** for the surface the owner wants
to leave running.

**The edge, restated.** The clamp gains a third term, and it is the one that
matters:

```
edge = min(width  − 2·HANG,
           height − 2·HANG − below,
           hero_px)                    ← the source's own pixels
       .max(ART_MIN)
```

`hero_px` is `min(decoded_w, decoded_h)` of what `load_hero` actually returned —
**not** `HERO_PX`, which is only the decoder's ceiling. A 500 px cover yields
`hero_px == 500` and is drawn at 500, centred, with the field around it. That is
S6's acceptance criterion and it is now enforced by arithmetic rather than by a
constant that happened to be small enough.

`NOW_PLAYING_MAX` **is deleted**. It was a fixed 720 standing in for a fact
about the decode (`now_playing.rs:74–81` says so in as many words), and once the
decode reports its own size the constant is the wrong shape of answer — it is
what made a 4K panel show a 720 px cover in a 3744 px body (§11.2).

**The test that holds it**, mirroring the wall's own
(`shelf.rs:1509–1530`):

```
the_now_playing_surface_never_draws_art_larger_than_its_source
  for hero_px in [120, 320, 500, 1024]:
    for side in (400..=4000).step_by(7):
      assert!(art_edge(side, side, hero_px) <= hero_px)
```

### 5.3 The field is not the artwork — the argument, made explicitly

This is the claim the whole section rests on, so it is made rather than assumed.

**What the field is, precisely.** Three colours sampled from the decoded cover —
not the average, which is always mud, but a small ordered palette: the most
common chroma-bearing hue, the darkest, and the lightest, each with lightness
and chroma **clamped into the room's own range** exactly as the lamp's are. They
are composited as a slow radial-plus-linear wash over `#0C0D0E`, at a ceiling of
**L ≈ 0.22** — darker than any sleeve, brighter than the room.

**Why that is data and not a copy of the work.** Four properties, each checkable:

1. **It is not invertible.** Three clamped colours cannot reconstruct an image.
   You cannot see what the record is from the field; you can only see *that the
   room changed colour*. A blurred copy, by contrast, is the work — Apple Music's
   shader background is recognisably the album from across a room, which is
   exactly why it is beautiful and exactly why it is a copy.
2. **It has no resolution**, so *"larger than its source"* is not a predicate
   that applies to it. This is the substantive difference from full-bleed, and
   it is why §14.1's rewrite is a clarification rather than a repeal: the
   refusal was always about **the work**, and the field is not the work.
3. **The ledger already ruled on exactly this object.** *"The art-derived lamp
   is **data** — hue read from the record, lightness and chroma pinned — not a
   preference"* (`REFUSALS.md:267–269`). The field is the lamp's own rule, with
   three colours instead of one and a large area instead of a small one. If
   hue-read-from-the-record is data at 6 px, it does not become decoration at
   1920.
4. **Amberol ships the honest version** — *"the whole window washed with a
   3-gradient composite from the palette"* (`03:236`) — and it is the one
   treatment in doc 03's table that draws no copy of the art at all.

**Why it is not a scrim.** The ledger's objection is specific and it is worth
quoting: *"a scrim is a surface laid over **the collection** to make something
else readable"* (`REFUSALS.md:126–132`). The field is under everything, laid over
nothing, and dims no artwork — it is the room's own colour, changed. This is the
same distinction the ledger itself drew when it admitted the hover veil, and it
is recorded in §14.2 rather than assumed here.

**The honest cost, stated.** The field is the one element on this surface with
no precedent in baz, and it is the one most able to look cheap. Two constraints
keep it from doing so, both testable: it never exceeds L 0.22 (so it cannot
compete with the sleeve, which is the brightest object by construction), and it
is **continuous** — no visible banding, which at these lightnesses means
dithering, and which §7.5 prices.

### 5.4 Where everything sits

The z-order, and the one rule that governs it:

```
z3   the placard, the meter's instrument register, the feed   (type)
z2   the work — true size, centred, its halo                  (artwork)
z1   the field — full bleed, derived, ambient                 (light)
z0   the room, #0C0D0E                                        (ground)
     ─────────────────────────────────────────────────────────
     the bar, outside the place entirely (app.rs:3744–3752)
```

> **Nothing is drawn on the sleeve. Everything ambient is drawn on the field.**

That sentence is what reconciles the owner's *"VU meter stuff over it"* with
`REFUSALS.md:107–111`'s *"anything on artwork anywhere but a wall tile — not the
Songs rows, not the lane, not the record's page"*. What *"takes up the
background"* is the field; the meter is over **the field**. The sleeve is the
one object on this screen with nothing on top of it, and it stays that way. The
owner's brief and the entry ask for the same composition once the field and the
work are understood as two objects.

The work keeps its halo (`theme::lamp_glow`, `REFUSALS.md:282` — *"no shadows
except the playing halo, which is not elevation, it is light"*), and that halo
is now doing real work: it is what separates a sleeve from a field of a similar
colour, which is the job a border would otherwise be reached for and which
`REFUSALS.md:279–280` forbids in as many words.

### 5.5 The layouts, measured

Lane open is `SIDEBAR_W` **280** and collapsed is `SIDEBAR_RAIL_W` **96**
(`theme.rs:1058`, `theme.rs:4677`); the bar is `BAR_H` **81**
(`theme.rs:3818`, `theme.rs:4224`), and it is outside the place. So
`body = (window.width − lane) × (window.height − 81)`
(`app.rs:3798–3826`).

The placard column's height below the work, with the transport gone (§6) and
the meter and feed arrived:

```
artist   LINE_HEADING  12      needle   NEEDLE_H      2
  GAP_XS               4         GAP_SM               8
title    LINE_HERO     32      figures  LINE_META    16
  GAP_XS               4         GAP_LG              16
album    LINE_BODY     20      meter    METER_H      24
  GAP_LG              16         GAP_LG              16
                                feed     LINE_BODY   20
                                ─────────────────────────
                                below              = 190
```

`below` was **162** with the transport in it and is **190** with the meter and
the feed; dropping `TRANSPORT_HIT` 32 pays for most of what arrives.

| Window | Lane | Body | `edge` (1000 px source) | Work as % of body width |
|---|---|---|---|---|
| 1280 × 800 | open 280 | 1000 × 719 | **449** (height-bound) | 45 % |
| 1280 × 800 | collapsed 96 | 1184 × 719 | **449** (height-bound) | 38 % |
| 1920 × 1080 | collapsed 96 | 1824 × 999 | **729** (height-bound) | 40 % |
| 2560 × 1440 | collapsed 96 | 2464 × 1359 | **1000** (source-bound) | 41 % |
| 3840 × 2160 | collapsed 96 | 3744 × 2079 | **1000** (source-bound) | 27 % |
| 3840 × 2160 | collapsed 96, 3000 px source | 3744 × 2079 | **1809** (height-bound) | 48 % |

Two readings of that table, and both are design findings:

- **The source becomes the binding constraint at 2560 and above**, which is the
  refusal working rather than a limitation. A listener with 300 × 300 covers
  gets a small sleeve on a large field, honestly; a listener who rips with
  1500 px art gets a wall-sized one. **The screen rewards a well-kept
  collection**, which is a sentence this product should want to be true.
- **At 4K with a modest cover the work is 27 % of the body**, and that is where
  the field stops being decoration and becomes the composition. Without it, a
  1000 px square floating in 3744 px of `#0C0D0E` is not a kiosk, it is a
  postage stamp in a void. §11.3 is the type scale that goes with it.

**1920 × 1080, lane collapsed — the case the brief describes:**

```
 ┌──────────────────────────────────────────────────────────────────┐
 │▓▓▓▓▓ the field: derived wash, full bleed, L ≤ 0.22 ▓▓▓▓▓▓▓▓▓▓▓▓▓▓│
 │▓ 96 ▓                                                          ▓▓│
 │▓lane▓                  ┌──────────────┐                        ▓▓│
 │▓    ▓                  │              │  ← halo, not a border  ▓▓│
 │▓Home▓                  │   the work   │                        ▓▓│
 │▓Libr▓                  │   729 × 729  │  true size, ≤ source   ▓▓│
 │▓Now ▓                  │              │                        ▓▓│
 │▓ ●  ▓                  └──────────────┘                        ▓▓│
 │▓    ▓                  T A L K   T A L K          12 ▲         ▓▓│
 │▓    ▓                  Spirit of Eden               32 ▲       ▓▓│
 │▓    ▓                  Spirit of Eden · 1988        20 ▲       ▓▓│
 │▓    ▓                  ├──────────────┤              2  needle ▓▓│
 │▓    ▓                  3:12              6:27       16 ▲       ▓▓│
 │▓    ▓                  ▁▂▃▅▃▂▁  −14.2 LUFS          24  meter  ▓▓│
 │▓    ▓                  Played 34 times since 2019   20  feed   ▓▓│
 │▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓│
 ├──────────────────────────────────────────────────────────────────┤
 │ the bar — 81 px, unchanged, in every place (app.rs:3744–3752)    │
 └──────────────────────────────────────────────────────────────────┘
```

Everything in the placard column is `edge` wide and left-aligned to the work's
own left edge — the wall label's rule at the far field's scale, which is what
`now_playing.rs:123–145` already does and which nothing here changes.

### 5.6 What the surface still needs of its own

With the transport gone (§6), the answer is short, and it is the test of whether
§6 was right:

| Element | Why it is not the bar's | Interactive? |
|---|---|---|
| The **work** at `edge` | The bar's cover is `BAR_COVER` 52 (`theme.rs:1723`). This is the subject; that is a thumbnail | No — §6.3 |
| The **placard** at `SIZE_HERO`+ | The bar states it at `SIZE_BODY` 13, which is not legible at 3 m (§1.2) | No |
| The **needle** at `edge` wide | The bar's is the full window at 2 px. This one is a control you can actually hit from a chair | **Yes** — already hit-tested (`player.rs:527–531`) |
| The **field** | Nothing in the bar is ambient | No |
| The **meter** | Nothing in baz measures level in real time at all (§9) | No |
| The **feed** | Nothing in baz surfaces the ledger since ADR-0022 deleted the inspector (§1) | No |

Two of these six exist today, one is being enlarged, and three are new. That is
the honest shape of the work, and §12 orders it.

---

## 6. Input: the kiosk that can be paused

*A kiosk that cannot be paused is a design failure; a kiosk covered in chrome
is not a kiosk.* Both halves are true, and the tension dissolves against
§1.2's two distances rather than being split.

### 6.1 The decision: the bar is in it, unchanged

ADR-0022's second rule — *"the bar is in every place, unchanged, and it is the
only thing that is"* (`0022-places-and-nothing-else.md:91`) — **is not amended
here.** The kiosk is a place; the bar is in it; the transport, the needle, the
volume and the doors all work exactly as they do everywhere else.

The alternatives were weighed and each fails on a named rule:

| Alternative | Fails on |
|---|---|
| **Hover-revealed transport** | Refused outright: *"no control's only affordance is hover"* (`REFUSALS.md:174–175`). ADR-0028 has just re-confirmed that this entry outranks a quietness preference, and it is the mitigation for a toolkit with no accessibility tree — which is precisely the wrong thing to trade for a tidier picture. Doc 10 §6.3 already lists it as refused rather than merely rejected |
| **A bar re-laid for this place** (drop the wall label, keep the transport) | A bar that changes shape per place is a bar you cannot learn. Doc 10 §4.4's finding stands: *"the transport a listener uses forty times a session does not move a pixel under this study."* The ratchet's permitted move — replacing a slot with a better statement of the same fact — is about the bar's own evolution, not about the bar becoming five bars |
| **No bar, plus a kiosk-local transport** | Two transports in one product, and the second one unlearned. It also re-opens every arithmetic ADR-0022 settled |
| **No bar, no transport** | S1's listener cannot pause the music from the screen they are looking at |

### 6.2 Why the duplication is acceptable, stated rather than hidden

The bar's left zone is *"the wall label at bar scale"* (`system.md:64–65`), and
this place states the same title and artist at room scale. That is a real
duplication and L6 would normally call it a hierarchy fault.

It survives on §1.2's observation, which is a fact about eyes rather than a
preference: **at 3 m, `SIZE_BODY` 13 and `SIZE_META` 12 are not small, they are
absent.** The bar does not compete with the far-field statement because it is
not legible from where the far-field statement is being read. At 60 cm both are
legible, and there the bar is not a redundant caption — it is the control
surface the hand is going to, with the transport, the needle and the fader on
it.

So the two zones are not two statements of one fact to one reader. They are one
statement each to two readers, and the product already had both.

### 6.3 What the place itself accepts

Exactly two things, both of which already exist as widgets:

- **The needle**, at kiosk scale (§5.4). It is already interactive, already
  hit-tested through the same module that draws it (`player.rs:527–531` — *"the
  line that is drawn and the line that is clicked can never be two different
  lines"*), and already carries its own hover preview. Nothing new is invented;
  it is the same widget given room.
- **The fullscreen glyph** (§3.2), one `TRANSPORT_HIT` 32 box, at rest, in the
  place's top-right corner, tooltipped per the icon-only law.

And one thing it deliberately refuses: **the artwork is not a control.** A
click on the sleeve is a gesture with no visible affordance, and the ledger
forbids both halves of that — nothing may be drawn on a sleeve to advertise it
(`REFUSALS.md:89–91`), and no action may be gesture-only (`REFUSALS.md:174–175`,
doc 09 §5.2's reading). The route to the record's page is the bar's
now-playing block, in this place as in every other.

---

## 7. Motion, idle cost, and burn-in

This is the section the brief's first two constraints live in, and they pull
against each other: **constraint 1** says a screen left running for eight hours
is the ultimate test of the 0.0 % idle claim, and **constraint 2** says a static
screen for eight hours is an OLED hazard whose standard mitigation is to move
pixels. Both are right. The resolution is that the screen is neither static nor
animated — **it is a slideshow with a four-minute frame rate**, and that fact
does the burn-in work that motion would otherwise have to do.

### 7.1 The redraw ledger

The honest accounting, in frames, with every number cited rather than
estimated:

| State | What drives a redraw | Rate |
|---|---|---|
| Library at rest, today | `REFRESH_TICK`, the periodic-rescan clock | **1 wake/minute** (`app.rs:82`) |
| Any place, a tween live | `motion::TICK`, installed **only while `moving()`** | 125 Hz, for ≤ 200 ms (`motion.rs:63`, `app.rs:3345–3353`) |
| **Kiosk, playing** | `Event::Progress`, from the engine | **4 Hz** (`engine.rs:562`) |
| **Kiosk, paused** | nothing — `Progress` is not emitted while paused | **0** (`protocol.rs:435–438`; pump pause gate `engine.rs:1518–1523`) |
| **Kiosk, stopped / queue ended** | nothing | **0** |

Three things follow, and they are the whole of this document's answer to
constraint 1:

1. **The kiosk introduces no clock of its own.** It installs no
   `window::frames()`, no `time::every`, no subscription that the Library place
   does not already have. Its redraw rate is the engine's event rate, and the
   engine's event rate is derived from *delivered audio* rather than from a
   clock (`engine.rs:557–561`), so it is 4 Hz of playing time and exactly zero
   when nothing is playing.
2. **The eight-hour test is passed by construction, not by tuning.** ADR-0020's
   claim is that the subscription is inactive when no tween is running, asserted
   by a test rather than promised. The kiosk does not weaken that assertion —
   it is the first surface on which it is *load-bearing for hours at a time*,
   which is an argument for the claim rather than against it.
3. **4 Hz is not new.** The bar has been redrawing at 4 Hz while music plays
   since the needle shipped; the kiosk redraws the same window at the same rate
   with a different picture in it. The marginal cost of this surface over the
   Library place, while playing, is one artwork quad and a few text runs per
   frame — and the Library place is drawing up to 120 tiles per frame in
   ADR-0020's own measurement setup.

**What is refused, by name.** Each of these was considered because the brief
invited it, and each breaks a property the project measured and shipped:

| Refused | Why |
|---|---|
| A drifting gradient, a slow pan/zoom over the artwork ("Ken Burns") | Requires a redraw while the window is idle — the clause everything else hangs on (`REFUSALS.md:243–245`). Also draws *on* artwork, refused separately |
| A pulsing or breathing glow on the halo | Same, plus the halo is `LAMP_GLOW` and the accent may not be animated into decoration (`REFUSALS.md:250–252`: *"motion states what changed; it never decorates"*) |
| A rotating record, a spinning disc | Refused twice over: the redraw clause, and skeuomorphism's ban on *"any circle pretending to be a record"* (`REFUSALS.md:257–261`) |
| An album-art crossfade on track change | Refused by name in ADR-0020 §3 and `REFUSALS.md:241–243`. A track change is a hard cut |
| A spectrum analyser at rest | §10. It is the one item here with a real case, and it needs its own ADR |

**What makes it feel alive instead**, since "nothing moves" is only an
acceptable answer if something else is doing the work:

- **Scale.** A 720 px sleeve is not a 320 px sleeve; the material of a record
  sleeve — grain, print texture, the photograph — is legible at kiosk size and
  invisible at wall size. The artwork is doing the aesthetic work, which is
  exactly the direction's own thesis: *"the works are lit; the room is not"*
  (`system.md:25`).
- **Type at a scale the product has never used.** `SIZE_HERO` 28 is the
  largest token baz owns and it exists for the first-run question. This surface
  needs a step beyond it (§5.2), and a title set that large is an event on a
  near-black field.
- **The needle advancing**, which was never animation: *"the two movements that
  were never animation are unchanged: the needle advancing with playback (data
  arriving) and scrolling"* (`REFUSALS.md:247–248`).
- **The track changing.** Every three to five minutes the entire surface
  becomes a different picture and a different set of words. Over an eight-hour
  evening that is on the order of **100–160 complete content changes**. A
  surface that fully repaints itself 130 times an evening is not a static
  screen, and §7.2 is where that stops being an aesthetic observation and
  becomes the engineering answer.

### 7.2 Burn-in, and why the palette is the mitigation

**The hazard is real and specific.** OLED wear is cumulative and per-subpixel:
an emitter's brightness degrades roughly in proportion to the total light it has
emitted, so a region that is bright and *unchanging* for many hours ages
relative to its neighbours and leaves a visible ghost. That is why the standard
mitigations in shipping products are all either *move the bright thing* (pixel
shifting/orbiting) or *make it dimmer* (logo-luminance limiting, static-content
detection).

**Both standard mitigations argue with §7.1**, and pixel shifting argues with it
directly: moving pixels on a timer is a redraw while the window is idle, which
is the one clause ADR-0020 kept as an absolute. So the tension the brief names
is genuine. Here is how it resolves.

**First: this room is already the mitigation.** Closing Time's wall is
`#0C0D0E` (`system.md:150`) — a near-black at the very bottom of the display's
range, where an OLED subpixel is emitting almost nothing and therefore ageing
almost not at all. The product's palette was chosen for a gallery-at-night
look, and the same choice makes it close to the least hazardous full-screen
content a display can be asked to hold. Roughly:

```
1920 × 1080 kiosk, one 720 px sleeve, the type block, the bar
────────────────────────────────────────────────────────────────
the room  (#0C0D0E, near-zero emission)          ~74 % of pixels
the artwork (bright, arbitrary, CHANGES/track)   ~25 % of pixels
ivory type + bar furniture (small, CHANGES/track) ~1 % of pixels
```

The only large bright region is the artwork, and **the artwork is the thing
that changes every three to five minutes.** The regions that are static — the
room, the bar's band, the hairline — are the regions that are near-black. The
hazard and the stasis are in different places, which is the property that makes
this safe without moving anything.

**Second: the genuinely static case is the stopped one, and S5 already answers
it.** A kiosk left paused overnight *is* an unchanging frame. But a stopped
kiosk holds no artwork and states silence in words (§5.5) — so the frame it
holds for those hours is overwhelmingly `#0C0D0E` with one quiet line on it.
The honest empty state and the burn-in answer turn out to be the same design,
which is the strongest sign that both are right.

**Third: the mitigation that costs a redraw is priced, and declined.** If
real-world evidence ever shows the above to be insufficient, the available move
is a **one-pixel nudge of the whole composition on a slow timer**. Its cost is
knowable in advance, and it is small:

> A 1 px shift once per minute is **one frame per 60 s**. baz already runs a
> 60-second timer at rest in the Library place — `REFRESH_TICK`
> (`app.rs:82`, installed at `app.rs:3366`) — and that timer is inside
> ADR-0020's accepted idle cost today. So the nudge is provably no more
> expensive than a clock the product already runs while doing nothing.

It is **not adopted**, for two reasons: the content already changes 130 times an
evening, and a mitigation with no measured problem to solve is decoration with a
technical justification. It is recorded here at its price so that re-proposing
it costs an observation rather than an argument — and so that if it is ever
adopted, nobody has to re-derive whether it breaks the motion law. It does not;
it costs one frame a minute, which is `REFRESH_TICK`'s bill exactly.

**What is refused outright**: global dimming after a timeout (it would make the
artwork a liar about its own colours, and the room is already dark), and
"screensaver" behaviour that replaces the content with something else while
music is still playing (the screen's whole job is to state what is playing).
