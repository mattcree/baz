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

## 3. The screen: a place that already exists, and one act left

### 3.1 `Place::NowPlaying` — shipped, and what that settles

The surface is a **place**, and it is no longer a proposal: `Place::NowPlaying`
is a member of `place.rs`'s enum, routed at `app.rs:3670–3676`, reached from the
returns lane's head. `place.rs:19–27` records how it got there and on whose
authority:

> *"ADR-0030 recommended a home **band** at the head of the Library's body and
> recorded `Place::Home` under 'deliberately not done'. **The owner overruled
> both**, and `docs/REFUSALS.md`'s preamble says his decision is sufficient on
> its own: home is a real place, and a `Now playing` place stands beside it."*

Three things that decision settles, so this study does not re-open them:

- **The route in is the lane, not a bar door.** The head's three destinations —
  Home, Library, Now playing — are *"a closed set of three"* and *"a fourth is
  the refused thing"* (`REFUSALS.md:200–202`). The `Now playing` row carries the
  amber lamp when something sounds. **The bar gains no slot**, and the first
  draft of this document's proposal to add one is withdrawn: the ratchet permits
  it, but the lane already delivers it and a second route would be the
  duplication §6.0 exists to remove.
- **The place is offered when nothing is sounding**, and the shipped view
  answers with *"Nothing playing."* (`now_playing.rs:92–104`) — silence stated
  rather than an empty frame, which is S5 and which §7.6 discovered is also the
  burn-in answer.
- **`back()` returns `Library`** like every other member, because *"`Esc` means
  put this down, not go to the home page"* (`place.rs:29–33`).

What is left to decide is therefore not *whether* the screen is a place. It is
what the place should contain (§5–§9) and how it fills a monitor (§11).

### 3.2 The one act that still needs a control

The brief bundled two things — *a dedicated now-playing screen* and *full
screen*. The first now ships. The second does not, and it stays a separate act
with a separate subject, per doc 10 §3.1's rule that a control may not wear a
symbol whose convention promises one scope while acting on another:

| Act | Subject | Control | Status |
|---|---|---|---|
| Go to the now-playing screen | what is sounding → **the lane's head** | the `Now playing` row, with its lamp | **Shipped** |
| Make the window fill the display | this window → **the place's own body** | the expand glyph, place top-right | `F11`, §11 |
| Choose what is ambient | this surface → **the place's own body** | the `Ambient` word-door, place top-right | §7.2 |

So the place's top-right carries **two** controls and no more: one glyph
(expand, tooltipped per the icon-only law) and one word (`Ambient`). Both are
visible at rest and pointer-reachable, which `REFUSALS.md:249–250` requires; the
alternative — revealing them on mouse-move, as most kiosks do — is refused
outright, and ADR-0028 re-confirmed that entry outranks a quietness preference.

**Fullscreen is a window act, not a place act.** `F11` works in every place; it
is not the kiosk's private key. The alternative — fullscreen that exists only
inside one place — would make leaving the place and leaving fullscreen the same
gesture, which is a micro-mode the ledger refuses in another form.

### 3.3 Leaving: `Esc` peels, exactly as it already does

baz's `Esc` is already a peeling rule rather than a back button — *"leave the
search field, then peel one layer: the queue, else the settings, else the pull,
else the query…"* (`README.md:43`). Fullscreen and the place are two layers, so
they peel in the order they were put on:

1. **`Esc` in the fullscreen kiosk** → leaves fullscreen, stays in the place.
   Every browser and video player has taught this, and breaking it would strand
   a user who cannot find their window decorations.
2. **`Esc` again** → `Place::back()` → the Library, with the wall's scroll,
   query and arrangement untouched, exactly as leaving any other place.
3. The lane's rows do what they do in every place.

No new rule, no new key, and nothing about `Esc` changes anywhere else. The
`Ambient` menu, when open, peels first — it is a summoned layer, and it goes
before the fullscreen does.

---

## 4. The user stories

The industry artifacts doc 09 §4 established: each scenario as a **user story**,
its **task flow** from where the listener actually is, and **acceptance
criteria** in Given/When/Then, written to be implemented and tested as stated.
Personas are `research/05-personas.md`'s.

### S1 — Leave it running on the other monitor

> As a listener with a spare display, I want to put baz's now-playing screen on
> it full screen and leave it there for the evening, so that the room has
> something worth looking at and my main screen is free for work.

**Task flow**: ① drag the window to the second monitor; ② `Now playing` in the
lane; ③ `F11`. Then nothing, for eight hours.

**Acceptance criteria**

- Given the window is on a given monitor, when `F11` is pressed, then the window
  fills **that** monitor and no other, and the place fills the window.
- Given the kiosk is showing at 3840 × 2160 with the default toggles, when it has
  been running for an hour, then the 99th-percentile frame time is **under
  12 ms** and process CPU is **at or under 5 %** of one core (§7.4's gate,
  measured on a real GPU).
- Given the kiosk is showing and playback is **stopped**, when nothing is
  interacted with, then the surface states silence in words
  (`now_playing.rs:92–104`), draws no field, and holds a frame that is
  overwhelmingly `#0C0D0E`.
- Given eight hours have passed with music playing, when the display is
  examined, then no region has held both a high luminance and a constant value
  throughout: the artwork changes every 3–5 minutes and the field drifts
  continuously (§7.6).
- Given the track changes, when the new record's artwork and type arrive, then
  they replace the old ones as a **hard cut** — no crossfade, refused by name
  (ADR-0020 §3). **The field is the one exception**: it interpolates between the
  outgoing and incoming palettes over ≤ 400 ms, because a hard cut of the whole
  room's colour is a flash, and the field is ambient content rather than a
  statement (§7.1).

### S2 — Look at what is playing, properly

> As a listener at the machine, I want one press to see the record I am hearing
> at a size worth looking at, so that "what is this?" is answered without
> leaving what I was doing for long.

**Task flow**: ① `Now playing` in the lane; ② look; ③ `Esc`.

**Acceptance criteria**

- Given any place, when `Now playing` is pressed, then the kiosk replaces it as
  an ordinary place change — a **hard cut**.
- Given the kiosk, when `Esc` is pressed and the window is not fullscreen, then
  the Library returns with its scroll, query and arrangement untouched
  (`place.rs:41–46`).
- Given the kiosk is showing, when a track change occurs, then every readout on
  it updates from the same `PlayerState` the bar reads, so the two surfaces
  cannot disagree.
- Given the kiosk is showing, when the pointer rests anywhere on it, then **no
  control appears that was not already visible** (§3.2).

### S3 — Pause it from the chair

> As the listener who just left the room, I want to stop the music from the
> screen I am looking at, without a transport drawn twice on it.

**Acceptance criteria**

- Given the kiosk is showing, when the listener reaches for play/pause, then the
  bar's transport is present, unchanged, in this place as in every other
  (`app.rs:3744–3752`), at its usual position and size.
- Given the kiosk is showing, when the place's own body is inspected, then it
  **contains no transport** (§6.0) — and no other control that the bar already
  carries.
- Given the kiosk, when the listener presses the needle at kiosk width, then the
  seek happens exactly as it does in the bar, because it is the same widget with
  the same hit test (`player.rs:527–531`).

### S4 — Be told something you did not know

> As Devon, I want the screen to tell me something about this record I would not
> otherwise have thought about, so that leaving it on is rewarding rather than
> merely decorative.

**Task flow**: ① `Now playing`; ② read; ③ press the line for the next fact.

**Acceptance criteria**

- Given the ledger has records for the sounding track, when the kiosk renders,
  then it states the permitted card — *"Played N times since YYYY"* — in the
  ledger's own permitted form (`REFUSALS.md:71–73`), read from `TrackHistory`
  (`history/read.rs:140–159`).
- Given the ledger has **no** record for this track, when the kiosk renders,
  then it says so as a positive statement — *"never played before"* — which is
  what the ledger actually knows (`Recency::Never`, distinct from `Unrecorded`).
- Given any state, when the feed renders, then it shows **no totals, no streaks,
  no charts, no listening-time sum and no cross-track aggregation** —
  `TrackHistory::listened_ms` exists and is deliberately not rendered (§8.4).
- Given the feed is showing fact *n*, when the line is pressed, then fact *n+1*
  appears; and cycling through the whole rotation returns to fact *n* — **the
  pool is exhaustible, which is how it is visible** (§8.2).
- Given a play is recorded while the kiosk is showing, when
  `Event::PlayRecorded` arrives, then the count is current. Today the history
  snapshot is read once at open (`app.rs:4212`, read via `read_history()` at `app.rs:5757–5773`) and `PlayRecorded` has no
  consumer in `crates/baz` at all; §12 step 5 is that wiring.

### S5 — Verify the chain

> As Karl, I want the whole signal path stated in one place, so that I can
> confirm the claim rather than trust it.

**Acceptance criteria**

- Given a `SignalPath` event has been received, when the feed reaches F4, then
  it states the **whole** reading from `PlayerState::signal_path()`
  (`player.rs:2016–2027`) — source rate, output rate, the chain's state, the
  conversion reason when there is one, and the volume path — not the bar's
  two-word summary.
- Given the chain is `Exclusive { conversion: None }`, when the reading renders,
  then it says so — **which the bar today does not**, because `bit_exact()`
  compares `chain == SignalChain::Direct` exactly and an exclusive chain
  therefore renders no note at all. §12 step 5 records this as a defect this
  study found, with the protocol's own guidance as the fix (`protocol.rs:924–927`:
  ask through `is_exclusive()` and `conversion_reason()`, do not enumerate
  variants).
- Given no `SignalPath` has been received, when the feed renders, then F4 is
  **absent from the cycle, not empty**. A fact that says nothing is not filled
  with a dash.
- Given any state, when the reading renders, then its words are flat: no
  "degraded", no "fallback", no boast, no badge, no colour carrying a verdict
  (ADR-0009 §5, `REFUSALS.md:348–350`).
- Given the meter is on, when the listener changes the volume, then **the meter
  does not move**, because it reads pre-gain (§9.2) — and the instrument says so
  in its own caption, so the behaviour is explained on screen rather than
  surprising.

### S6 — Turn it all off

> As a listener who finds movement distracting, I want a still screen, and I
> want the product to cost nothing when I am not looking at it.

**Acceptance criteria**

- Given the kiosk is showing, when the `Ambient` door is pressed, then three
  labelled switches appear — field, meter, feed — and the same three are in
  Settings (§7.2).
- Given all three toggles are off, when the kiosk is showing and music is
  playing, then the surface draws only on data arriving, and `view()` is not
  called on any clock of its own.
- Given all three toggles are off, when the surface is idle, then process CPU is
  **0.0 %** and the frame count after settling is **zero** — bit-for-bit the
  figure ADR-0020 shipped.
- Given any toggle state, when the listener navigates to any other place, then
  **no ambient subscription exists**, asserted over every place × every toggle
  combination (`the_ambient_clock_is_absent_outside_its_place`, §7.3).
- Given the renderer has fallen back to tiny-skia, when the field is on, then it
  draws the **static** wash rather than nothing (§7.5) — a still field, not a
  hole.

### S7 — The artwork is missing, or small

> As Marta, whose older rips have no embedded cover and whose newer ones have a
> 300 px one, I want the screen composed rather than broken.

**Acceptance criteria**

- Given no `ArtSource` resolves (`art.rs:69–78` returns `None`), when the kiosk
  renders, then it draws the wall's own deterministic gradient placeholder at
  this scale — the same object a tile shows (`now_playing.rs:114`) — and the
  field falls back to the room, because there is no palette to read.
- Given the source artwork is **smaller** than the viewport allows, when it
  renders, then it is drawn **at its own pixel size**, centred, never scaled up
  — enforced by `art_edge`'s third term (§5.2) and asserted by
  `the_now_playing_surface_never_draws_art_larger_than_its_source`.
- Given the artwork is missing, when the composition renders, then **no lane
  moves**: the placard sits where it sits whether or not there is a picture,
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
  postage stamp in a void. §11.2 is the type scale that goes with it.

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

## 6. Input: the transport comes off, because it is already there

*A kiosk that cannot be paused is a design failure; a kiosk covered in chrome
is not a kiosk.* Both halves are true, and the tension dissolves against
§1.2's two distances rather than being split.

### 6.0 The decision: delete the place's transport, keep the bar's

> **`now_playing.rs:168`'s call to `bottom_bar::transport` is deleted. Nothing
> replaces it.**

The owner's *"now playing does not need the play pause controls"* is, on this
surface, a bug report. §0.4 (a) has the evidence: the place calls the bar's own
`transport(player, ink)` and the bar draws the same function under it
unconditionally (`app.rs:3744–3752`), separated by one `GAP_XL` 24 and the
place's own bottom padding. **The same three glyphs, from the same function,
driven by the same `PlayerState`, twice on one screen.**

**Is the bar present in this place? Yes, and unconditionally.** The composition
at `app.rs:3744–3752` appends `bottom_bar::view` under *every* place, gated on
exactly one thing — `Availability::NotBuilt`, a build with no audio output at
all, where hiding playback UI entirely is the honest answer. There is no
per-place branch. `Place::NowPlaying` is not special-cased and does not need to
be.

**This is the same reasoning that just removed `‹ Library` from the place
headers** (`9a7e9a5`, *"The place headers lose their way back, because the lane
is one"*): a place that repeats what a resident surface already carries is
making the same statement twice, and the second copy is the one that goes. The
lane made the header's back-link redundant; the bar makes the place's transport
redundant. One precedent, applied twice, four commits apart.

**What it costs: nothing, and this is checkable.** Every function of the deleted
control survives at full size, visibly, one surface down — play/pause, previous,
next, the needle, the fader, the doors. The accessibility refusal
(`REFUSALS.md:249–250`) is satisfied by *a* visible pointer-reachable control,
not by two; and it is satisfied by the control the listener has used forty times
a session rather than by a copy of it that exists in one place.

**What it buys**, and why it is not merely tidiness:

- **32 px of `below`** (`TRANSPORT_HIT`), which §5.5 spends on the meter and the
  feed. The transport was the single largest term in the arithmetic that decides
  how big the artwork can be — so the control the owner does not want was
  literally making the thing he does want smaller.
- **The composition stops arguing with itself.** L6's hierarchy rule is the one
  §6.2 has to defend for the *title*; for the transport there is no defence to
  make, because unlike the title it is not a statement at a different scale to a
  different reader. It is the identical widget at the identical size.
- **A surface with no controls on it but the needle** is what makes §7's
  ambient motion legible as *content* rather than as chrome that moves.

### 6.1 The bar itself: in it, unchanged

ADR-0022's second rule — *"the bar is in every place, unchanged, and it is the
only thing that is"* (`0022-places-and-nothing-else.md:91`) — **is not amended
here.** The kiosk is a place; the bar is in it; the transport, the needle, the
volume and the doors all work exactly as they do everywhere else.

The alternatives were weighed and each fails on a named rule:

| Alternative | Fails on |
|---|---|
| **Hover-revealed transport** | Refused outright: *"no control's only affordance is hover"* (`REFUSALS.md:249–250`). ADR-0028 has just re-confirmed that this entry outranks a quietness preference, and it is the mitigation for a toolkit with no accessibility tree — which is precisely the wrong thing to trade for a tidier picture. Doc 10 §6.3 already lists it as refused rather than merely rejected |
| **A bar re-laid for this place** (drop the wall label, keep the transport) | A bar that changes shape per place is a bar you cannot learn. Doc 10 §4.4's finding stands: *"the transport a listener uses forty times a session does not move a pixel under this study."* The ratchet's permitted move — replacing a slot with a better statement of the same fact — is about the bar's own evolution, not about the bar becoming five bars |
| **No bar, plus a kiosk-local transport** | Two transports in one product, and the second one unlearned. It also re-opens every arithmetic ADR-0022 settled |
| **No bar, no transport** | S1's listener cannot pause the music from the screen they are looking at |

### 6.2 The one duplication that survives, and why it is not the transport's

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

**Four things, and the list is closed.** Every one is visible at rest, which is
the whole of what `REFUSALS.md:249–250` asks:

- **The needle**, at the work's width (§5.5). Already interactive, already
  hit-tested through the same module that draws it (`player.rs:527–531` — *"the
  line that is drawn and the line that is clicked can never be two different
  lines"*), and already carrying its own hover preview. Nothing is invented; it
  is the same widget given room.
- **The fullscreen glyph** (§3.2), one `TRANSPORT_HIT` 32 box, at rest, in the
  place's top-right corner, tooltipped per the icon-only law.
- **The `Ambient` word-door** (§7.2), beside it, opening the three toggles.
- **The feed line** (§8.2), which advances on a press — which is what makes the
  rotation a control rather than a performance.

And one thing it deliberately refuses: **the artwork is not a control.** A
click on the sleeve is a gesture with no visible affordance, and the ledger
forbids both halves of that — nothing may be drawn on a sleeve to advertise it
(`REFUSALS.md:107–111`), and no action may be gesture-only (`REFUSALS.md:249–250`,
doc 09 §5.2's reading). The route to the record's page is the bar's
now-playing block, in this place as in every other.

---


## 7. Ambient motion: the class, the toggles, and the bill

### 7.0 The ruling, and what it changes

> **"Ambient motion is fine as long as the performance remains top tier."**
> — the owner, 2026-08-09

This reverses the posture the first draft of this document was written in, and
it is worth being precise about what it does and does not reverse.

**What it does not touch.** ADR-0020's measurement stands, its five transitions
stand, and its idle claim stands *for the product at rest*. That claim was
earned by refusing decorative motion **nobody asked for** — a drifting gradient
on the wall, a pulse on the bar, a stagger on the grid — and every one of those
is still refused, because nobody has asked for them and they would still be
decoration.

**What it changes.** This surface is different in kind. The owner has asked for
ambient motion *here*, explicitly, on a screen whose entire purpose is to be
looked at. **Motion that is the content is not decoration.** So ADR-0020 does
not get violated by this surface; it **gains a class** that this surface fits
into, bounded by the same discipline every other class carries.

**What replaces the old bar.** The constraint is no longer *"does it redraw
while idle"* — of course it does, that is what ambient means. The constraint is
the owner's own, and it is harder because it is quantitative: **top tier**, in
numbers, with a method. §7.4 is that bill, and §12 gates the work on it.

### 7.1 ADR-0020's sixth class: user-started ambient content

ADR-0020 names five bounded transitions (§2) and, in its amendment, a sixth
thing that is *not* a transition — pointer-derived deformation (§5), admitted as
its own class *precisely because* shipping it under the transitions list *"would
make the list a fiction"*. That is the precedent, and it is exactly the move
this needs:

> **7. User-started ambient content is permitted**, as a class distinct from
> both the bounded tween and pointer-derived deformation, under its own
> discipline:
>
> - it exists **only on a surface whose purpose is to be looked at** — today
>   that is `Place::NowPlaying` and nothing else, and a second such surface
>   needs an argument that beats this one;
> - it is **a thing you start, never a thing that starts itself** — the same
>   sentence the ledger already applies to shuffle (`REFUSALS.md:37–38`), and
>   the reason the toggles in §7.2 are first-class rather than a preference
>   panel;
> - it costs **exactly nothing when it is off or when its surface is not on
>   screen**, guaranteed by the subscription being a function of state (§7.3)
>   rather than by anyone remembering to stop a timer;
> - it carries **a stated frame budget and a measured cost**, re-measured when
>   it changes, because *"top tier"* is a number or it is nothing;
> - it **states nothing**. This is the line that keeps the class from becoming
>   a loophole: a bounded tween exists to say *something changed*, and ambient
>   content exists to be pleasant. Because it makes no claim, it may never be
>   the only carrier of one — no state is signalled by the field's motion, and
>   `REFUSALS.md:259–261` is why.

The last clause is what stops this class swallowing the other two. A drifting
field cannot tell you the track changed; the hard cut does that, and it stays a
hard cut.

### 7.2 What is toggle-able, where, and what the defaults are

Three independent switches, because they have genuinely different costs and
genuinely different audiences:

| # | Toggle | What it controls | Default | Cost when on |
|---|---|---|---|---|
| **T1** | **Ambient field** | The derived wash (§5.3), and whether it **drifts** or is still | **On, drifting** | §7.4 |
| **T2** | **Meter** | The two registers of §9 — the field's response and the instrument | **On** | §7.4 |
| **T3** | **Feed** | The rotating fact line (§8) | **On** | ~0 (§8.5) |

**Why these three and not one.** A single "ambient mode" switch would bundle a
GPU cost, an audio-thread cost and a disk read into one control, so a listener
who wants the facts but not the meter would have to take both. Three switches,
three subsystems, and each one's *off* is a real structural saving rather than a
skipped draw call.

**Why the defaults are on.** Under the old posture these defaults would have
been defensive; under the ruling they are aesthetic, and the aesthetic answer is
that **a surface the owner asked to be ambient should be ambient the first time
he opens it.** A default of *off* would make the feature a thing you have to
find, which for a screen whose whole value is being left running is the wrong
failure. T1's *drift* sub-state is the one genuine judgement call, and it
defaults on for the burn-in reason §7.6 gives — it is the mitigation, so making
it opt-in would ship the hazard by default.

**Where the controls live: both, and they are different controls.**

- **On the surface** — a single **`Ambient`** word-door in the place's
  top-right, opening a small menu with the three switches. It is a labelled,
  pointer-reachable control at rest, which `REFUSALS.md:249–250` requires and
  which a hover-revealed gear would violate. It is a *word* and not a glyph for
  doc 10 §3.4's reason: the enumerated symbol list is closed at two, the gear
  and the magnifier (`system.md:876–879`), and no universal symbol means
  *ambient*.
- **In Settings** — the same three switches, in the playback section, because
  Settings is where a person looks for a setting and because a listener who
  turned the field off from a chair needs somewhere to find it again that is not
  the surface they turned it off on.

Both write the same three booleans. There is no third state and no per-place
override; this is one surface, so the setting is global by construction.

**The fourth switch that is not here.** *Reduce motion* is not a baz setting —
ADR-0020 §2 already specifies that every transition *"degrades to a hard cut by
passing a zero duration, which is how a reduce motion setting will be
implemented"*. T1's still/drifting sub-state is that mechanism for this class,
and if a global reduce-motion preference is ever read from the desktop it sets
T1 to still and leaves T2 and T3 alone — a meter is not motion for motion's
sake, and a rotating fact is not an animation at all.

### 7.3 The structural zero, and why it is not a promise

> **With this surface off screen, or with its toggles off, idle is bit-for-bit
> the 0.0 % that shipped — because there is no timer to be careful about.**

The mechanism is already in the codebase and already commented, at
`app.rs:3900–3908`:

> *"**Only while something is moving, and never otherwise** — the whole of
> ADR-0020's cost argument… A subscription in iced 0.13 is a function of state:
> it is rebuilt after every update and the ones that went away are dropped, so
> the last tick of the last tween removes this timer and the event loop parks."*

So the ambient subscription is one more arm in the same `Vec<Subscription>`,
under a guard that names both conditions:

```rust
// The ambient surface's clock. Both terms matter: the place must be on
// screen AND something must be asked to move. Off screen, this arm does
// not exist, so there is no timer, no wakeup, and nothing to stop.
if self.place == Place::NowPlaying && self.ambient.animating() {
    subs.push(window::frames().map(Message::AmbientFrame));
}
```

Three properties follow, and each is structural rather than careful:

1. **Navigating away removes the clock.** `self.place` is read on every rebuild.
   A place change is an update; the arm evaluates false; the subscription is
   dropped; the loop parks in `Wait` (`iced_winit/src/program.rs:830–846`). No
   teardown code exists to be forgotten, because there is no handle to hold.
2. **Toggling off removes the clock**, by the identical mechanism —
   `ambient.animating()` is `T1.drifting || T2.on`. T3 is deliberately *not* in
   that term because the feed is not animated: it advances on a 20 s dwell, so
   it gets its own far slower arm under the same guard, and §8.5 prices it
   honestly at three wakeups a minute rather than claiming it is free.
3. **The rest of the product is untouched, provably.** ADR-0020's idle
   assertion is *a test* (`0020-motion.md`: *"the suite asserts the subscription
   is inactive when no tween is running"*). This adds one assertion of the same
   shape, and §12 step 6 makes it a gate:

```
the_ambient_clock_is_absent_outside_its_place
  for place in every Place except NowPlaying:
    for toggles in every combination:
      assert!(!subscription_contains_ambient_frames(place, toggles))
```

**One caveat, stated because it is real and easy to miss.** §0.3(4) found that
iced's control flow is **global, not per-window**, and §0.3(5) that after any
message batch *every* window is redraw-requested (`program.rs:1089–1097`). Today
baz is a single-window `application`, so this changes nothing. It is recorded
because it is the precise reason §11 does **not** recommend a second window: an
ambient clock in a kiosk window would pace the main window too, and the 0.0 %
idle claim would become false for the *whole product* the moment the kiosk was
opened. **The single-window design is what keeps §7.3's guarantee true**, which
makes §11's toolkit finding a design constraint rather than a limitation.

### 7.4 The bill

**What is already known, and it is the most relevant datum in the project.**
Doc 04 §1.4 measured baz's exact toolkit and feature set under **continuous**
animation on a real GPU (AMD RX 7700/7800 XT, Wayland, vsync, 1280 × 860,
**120 tiles per frame**):

| driver | continuous animation |
|---|---|
| bounded `time::every` | 242 frames (60 fps) · **4.0 % CPU** |
| unconditional `frames()` | 241 frames · **2.0 % CPU** |

**4.0 % of one core at 60 fps, drawing 120 album tiles.** This surface draws
**one** image, six text runs, a needle and a meter — an order of magnitude fewer
widgets. That is the strongest available evidence that the CPU side of this
design is cheap, and it is a measurement rather than a hope.

**What is not known, and this study will not pretend otherwise.** Three numbers
the owner's bar requires do not exist yet, because the feature does not exist
yet:

1. Frame time at 60 Hz for **this** composition, with the field's shader.
2. **GPU** cost — doc 04 measured process CPU from `/proc/self/stat` only, which
   for a fragment-shader-bound design is the wrong instrument.
3. Both of the above **at 3840 × 2160**, which is the case the owner described
   and which is **4.0× the pixels** of doc 04's 1280 × 860 window.

**The estimates, labelled as estimates.** The field is a full-screen quad with a
few dozen ALU operations per pixel and no texture sampling. At 4K60 that is
≈ 0.5 Gpixel/s of fill; integrated GPUs of the last five years sustain several
times that, so the field should be **GPU-bound but far from GPU-limited**, and
its CPU cost should be **one uniform write per frame**. The meter's geometry is
tens of vertices. *These are engineering estimates from the shape of the work,
not measurements, and §12 step 7 refuses to ship on them.*

**The measurement protocol** — an extension of doc 04 §1.3's harness rather than
a new one, so the numbers are comparable to the ones already in the project:

| | |
|---|---|
| **Harness** | doc 04 §1.3's standalone binary, pinned to iced 0.13.1 with baz's exact feature set, extended with a fourth driver: `ambient` (field shader + meter geometry + feed, no tiles) |
| **Instruments** | process CPU from `/proc/self/stat` (as doc 04); `view()` call count in a `Cell` inside `view` (passive, as doc 04); **frame time from `RedrawRequested` deltas**; **GPU busy % and VRAM from `amdgpu_top`/`radeontop`**, which doc 04 lacked |
| **Windows** | 1280 × 800 · 1920 × 1080 · **3840 × 2160**, all lane-collapsed |
| **Phases** | 3 s warm-up · 60 s ambient with music playing · toggles off · 60 s idle |
| **Control** | the same binary with all three toggles off, which must reproduce doc 04's `off` driver exactly |

**The acceptance gate — the numbers that define "top tier".** §12 step 7 does
not pass unless all four hold on the real GPU:

| Metric | Threshold | Why this number |
|---|---|---|
| Frame time, 1920 × 1080, 99th pct | **< 8 ms** | Half a 60 Hz frame. Leaves the compositor its own budget on a machine also doing work |
| Frame time, 3840 × 2160, 99th pct | **< 12 ms** | Still inside 16.7 ms with margin, at 4× the pixels |
| Process CPU, ambient, 4K | **≤ 5 %** of one core | Doc 04's continuous figure was 4.0 % at 120 tiles; this draws far less, so exceeding it would mean something is wrong rather than expensive |
| Process CPU, all toggles off, idle | **0.0 %**, and `view()` calls **0** after settle | §7.3's structural claim, verified rather than asserted |

**On taking the measurement.** Doc 04 recorded that its real-GPU run *"put a
window on the maintainer's display for about 50 seconds… there is no headless
path to a real-GPU number, and reporting software-rasterised CPU as if it were
the shipping cost would be a false receipt — so the intrusion was taken
deliberately rather than avoided."* The same applies and more so, because this
run needs a 4K panel and about four minutes. **The intrusion is the deliverable,
not a cost to be minimised**: a design justified by llvmpipe numbers would be
exactly the false receipt doc 04 refused to write.

### 7.5 What makes it cheap, and how it degrades

**The field is a `shader` widget, not a `Canvas`.** §0.3(6–7) settles this:

- `Canvas` is **not compiled into baz today** and its `Cache` *"will not redraw
  its geometry unless the dimensions of its layer change or it is explicitly
  cleared"* (`iced_graphics/src/geometry/cache.rs:7–10`) — so a field that drifts
  means `clear()` every frame and a **full re-tessellation** of a gradient mesh
  at 4K, on the CPU, sixty times a second. That is the expensive way to draw the
  cheap thing.
- `iced::widget::shader` **is available today with no manifest change**, because
  `wgpu` is in iced's default features (`iced/Cargo.toml:107–112`) and baz does
  not disable them. The drift becomes a **time uniform**: the CPU writes one
  float per frame and the GPU evaluates the wash per pixel, which is what GPUs
  are for.

**The meter is a `Canvas`**, and that is the one manifest change this design
needs (`canvas = ["iced_widget/canvas"]`, no new crate — §0.3(6)). Its geometry
is small, and `Cache::with_group` (`cache.rs:41–45`) is used exactly as intended:
**the static scale and labels in one cache, the moving mark in another**, so the
per-frame re-tessellation covers tens of vertices rather than the whole
instrument.

**The degradation, which is required rather than nice.** iced's defaults include
`tiny-skia` as a software fallback, and **the shader widget renders nothing
under it** (§0.3(7)). A blank rectangle where the field should be is not an
acceptable failure for the surface the owner leaves running. So:

> **T1 has three states, not two: `drifting` · `still` · `unavailable`.** When
> the wgpu backend is not in use, the field falls back to a **static** wash —
> the same three sampled colours composited once as an ordinary gradient
> `container` background, which every backend draws. It does not drift, it costs
> nothing per frame, and it looks like the still state rather than like a bug.

The detection is a startup property of the renderer, not a per-frame check, and
it sets T1's ceiling once. A listener on software rendering sees a still field
and a working meter, which is a good screen; they do not see a hole.

### 7.6 Burn-in: the hazard, and why ambient motion is now its answer

**The hazard is real and specific.** OLED wear is cumulative and per-subpixel:
an emitter dims roughly in proportion to the total light it has emitted, so a
region that is bright **and unchanging** for many hours ages relative to its
neighbours and leaves a ghost. Every shipping mitigation is one of two things —
*move the bright thing* (pixel shifting, orbiting) or *dim it* (logo-luminance
limiting, static-content detection).

**Under the old posture this was a genuine contradiction**: the mitigation is
motion, and motion was forbidden. Under the ruling it is not a contradiction at
all — **the mitigation and the feature are the same thing** — and this section
gets to stop arguing and start specifying.

**Four layers of answer, weakest hazard first:**

1. **The palette does most of the work, and it always did.** Closing Time's room
   is `#0C0D0E` (`system.md:150`), at the very bottom of the display's range,
   where a subpixel emits almost nothing and ages almost not at all. Roughly, at
   1920 × 1080 with a 729 px work:

   ```
   the room + the field (#0C0D0E → L ≤ 0.22)        ~73 % of pixels
   the work (bright, arbitrary, CHANGES per track)  ~26 % of pixels
   type + bar furniture (small, CHANGES per track)   ~1 % of pixels
   ```

   **The only large bright region is the artwork, and the artwork is the thing
   that changes every three to five minutes** — 100–160 complete content changes
   over an eight-hour evening. The hazard and the stasis are in different
   places.
2. **The field drifts, which is the textbook mitigation, for free.** T1's drift
   moves the wash's centres continuously. Every pixel of the 73 % therefore sees
   a varying — if small — luminance over any ten-minute window, which is exactly
   what pixel-orbiting exists to achieve, achieved by the feature rather than by
   a hack bolted beside it. **This is the argument for `drifting` being the
   default**, and it is a hardware-protection argument rather than an aesthetic
   one.
3. **The one genuinely static case is the stopped one, and it is already
   answered.** A kiosk left paused overnight is an unchanging frame — but a
   stopped surface holds no artwork and states silence in words
   (`now_playing.rs:92–104`, S5), so the frame it holds is overwhelmingly
   `#0C0D0E` with one dim line on it. The honest empty state and the burn-in
   answer are the same design, which is the strongest sign both are right. Note
   the field is also **absent** when nothing sounds: there is no record to derive
   it from, so there is nothing to draw.
4. **The bar's furniture is the residual**, and it is small, dim, and 81 px of a
   1080 px screen. It is the same furniture every other place shows, so this
   surface adds no hazard the product did not already have.

**What is still refused, and now for reasons rather than by inheritance:**

- **Global dimming after a timeout** — it would make the artwork a liar about
  its own colours, and the room is already dark. The hazard does not come from
  the room.
- **Replacing the content with a screensaver while music plays** — the screen's
  whole job is to state what is playing. Plexamp's screensaver is the prior art
  and it is the wrong trade for a surface that exists to answer *what is this*.
- **A periodic whole-composition pixel nudge** — the drift already moves the
  large areas continuously, and nudging the *type* would make text shimmer,
  which is worse than the hazard it treats. Recorded at its price so
  re-proposing it costs an observation rather than an argument: a 1 px shift
  once a minute is one frame per 60 s, which is exactly `REFRESH_TICK`'s
  existing bill (`app.rs:82`).

---

## 8. The feed: what baz already knows

> *"I also like the idea of just seeing related stuff appearing in like a feed
> of random facts."*

### 8.0 The premise: this is baz's strongest hand, not its weakest

The instinct with *"related stuff"* is to reach for a network, and that instinct
is wrong here — not for purity, but because **baz can say things no streaming
service can**. A service knows the record; it does not know *your* copy of it,
or that you first played it in 2019, or that it is a 24-bit rip reaching your
DAC untouched. Doc 03 reached this conclusion from the other direction and
stated it flatly:

> *"Power lives in **what the product knows about your files** — the twenty-field
> readout, the facets, the tag tools."* (`03:626–629`)

*"You have played this 34 times since 2019"* is context no streaming service can
give you, and it is already in a file on the listener's disk. So the feed is
designed **excellent with zero network first**, and §8.6's enrichment is a layer
the composition must not have a hole in when it is absent.

### 8.1 The inventory, and what each fact is worth

Everything below is on disk today. The last column is the honest one: whether it
is *interesting*, because a feed of boring true facts is worse than no feed.

| # | Fact | Source, cited | Worth reading? |
|---|---|---|---|
| F1 | **"Played 34 times since March 2019"** | `TrackHistory::plays`, `first_played_unix_s` (`history/read.rs:150–159`) | **The best one.** It is the ledger's own permitted form (`REFUSALS.md:71–73`), and it is the fact nobody else has |
| F2 | **"Last played 8 months ago"** | `TrackHistory::last_played_unix_s` | **Strong.** Re-encountering something you had forgotten is the surface's best moment |
| F3 | **"You have never played this before"** | `Recency::Never`, the ledger's *positive* statement, distinct from `Unrecorded` (`0018-play-history-ledger.md:5–11`) | **Strong**, and honest — it is a thing the ledger knows, not an absence |
| F4 | **The signal path in full** — source rate, output rate, whether anything converted, whether the device is held exclusively, whether any gain stage touches the samples | `PlayerState::signal_path()` (`player.rs:2016–2027`) — **dead code today**, kept because *"a diagnostics readout is ADR-0009's next step"* | **Strong for Karl** (`research/05-personas.md:35`), and it is already computed and currently thrown away |
| F5 | **"FLAC · 24-bit · 96 kHz · 47.2 MB"** | the condition report; `EditionVm::bit_depth` (`vm.rs:195–202`, `:1041`), `format_size` (`vm.rs:1079`) | Solid, and already rendered elsewhere |
| F6 | **"Measured −14.2 LUFS, peak −0.3 dBFS"** | ADR-0015's analysis; `Loudness::integrated_lufs` / `sample_peak` (`loudness.rs:390–397`) | **Underrated.** It is a real measurement baz performed, to a named standard, and it explains why this record is louder than the last |
| F7 | **"Released 1988 · Art rock"** | the scan's tag read, folded into `vm::AlbumVm` | Weak alone, fine in rotation |
| F8 | **"Track 6 of 6 · the last on side two"** | track/disc numbers | Pleasant; the record's own structure |
| F9 | **"From *Sunday Morning.m3u*"** | `queue_provenance()` (`player.rs:1722`) | Good — it answers *why is this playing* |
| F10 | **"Then 2 albums · 1:58:00 left"** | `continuation_note()` (`player.rs:1695`) | Already in the bar; **excluded** from the feed for that reason (§6.0's rule) |
| F11 | **"One of 47 records by this artist in your collection"** | derivable from the index, no new storage | **Strong.** It is the record's position in *your* collection, which is the sentence §5.5 wanted to be true |

**Three honest absences**, stated so nothing below quietly assumes them:

- **baz does not store when a record was added.** `FileStamp` holds `mtime_ns`
  and size (`library.rs:231`, `:246`), and a file's mtime is when it was *last
  written*, not when it entered the collection — a re-tag rewrites it. *"Added
  in 2019"* would therefore be a plausible-looking lie. It is **not shipped**;
  §13 D4 ranks the first-seen column that would make it true.
- **baz does not read embedded lyrics.** Nothing in the scan asks for them.
- **baz holds no MusicBrainz IDs**, and every enrichment source worth having is
  MBID-keyed (`VISION.md` pillar 5) — which is the strongest single argument for
  finishing the local feed before designing the network layer.

### 8.2 The rotation rule, in one sentence

The ledger's discipline here is the no-invisible-pool rule — *"a shuffle whose
source you cannot see is a recommendation engine wearing a dice icon"*
(`REFUSALS.md:41–47`). A rotating fact is close enough to that line that the
rule which picks facts must be statable in a sentence. It is:

> **The feed shows one fact at a time, cycling in a fixed order through exactly
> the facts this record has, advancing every 20 seconds, on every track change,
> and whenever you press it.**

Four properties, each deliberate:

- **The pool is the record.** Not the collection, not a recommendation set, not
  anything the listener cannot enumerate. Every fact on screen is about the
  thing currently sounding, and pressing through the cycle shows you the whole
  pool in under a minute. **The pool is visible by exhaustion**, which is the
  strongest form of the ledger's requirement.
- **The order is fixed, not random**, despite the brief's *"random facts"*. A
  fixed cycle is what makes the pool inspectable; a random draw would mean a
  fact you saw once and could not get back. The order is F1 → F2/F3 → F11 →
  F6 → F4 → F5 → F9 → F7 → F8, ranked by the *worth reading* column, so the
  best fact is the one you see on a track change. **It feels varied because
  records differ in which facts they have**, which is variety from the data
  rather than from a die.
- **Facts a record does not have are absent, not empty.** No dashes, no "unknown"
  — a record with no ReplayGain analysis simply has no F6 in its cycle, exactly
  as S3's signal register is *absent, not empty*.
- **It advances on a press**, which makes it a control rather than a
  performance, and satisfies `REFUSALS.md:249–250` — the line is a labelled,
  pointer-reachable target, not a thing that only happens *to* you.

### 8.3 Why it is one line and not a panel

The brief says *"a feed"*, and the temptation is a scrolling column. It is one
line, for §1.2's reason: **at 3 m a column of `SIZE_BODY` 13 is not small, it is
absent.** A feed you cannot read from the chair you left the screen for is
decoration. One line at `SIZE_BODY` on the placard column, at the work's own
width, is legible at 60 cm and — at kiosk scale, where §11.2 steps the type — at
3 m as well.

It also keeps the reserved-slot promise (`bottom_bar.rs:74–86`): **one line
high, always**, whether the fact is short or long, so nothing on the surface
moves when the fact changes. A fact longer than the width elides; it does not
wrap and it does not reflow the composition.

### 8.4 The engagement-stats line, and which side this falls on

`REFUSALS.md:68–73` is the entry that binds hardest here, and it is worth
quoting rather than paraphrasing:

> **No engagement stats.** No Wrapped, no streaks, no charts, no "top artists of
> the year", no listening-time totals. **History records; it never performs.**
>
> What history is allowed to surface: the PLAYED group key, the inspector card
> ("PLAYED — N times since YYYY", plus a column of date stamps), and the pull's
> weighting. Nothing else.

**The finding: F1 is not near the line — it is the permitted item, verbatim.**
The entry enumerates three permitted surfaces and one of them is *"PLAYED — N
times since YYYY"*. F1 is that string. And §1 established that this permission
**lost its home** when ADR-0022 deleted the inspector: the fact has been
permitted and homeless since. So the feed is not asking for a new licence; it is
the room an existing one has been waiting for.

**Where the line actually is, and the three things kept on the far side:**

| Refused | Why it is the refused thing |
|---|---|
| **`listened_ms` as a total** — "you have listened to 4.2 hours of this artist" | A listening-time total, refused by name. The field exists (`history/read.rs:157`) and is **deliberately not rendered** |
| **Any cross-track or cross-artist aggregation** — "your most played record this month" | This is a chart. The pool would be the collection rather than the record, which also breaks §8.2 |
| **Any framing that congratulates** — "you're on a 6-day streak", "your #1 record" | *History records; it never performs.* The tone test is that every fact reads as an **archivist's note** (`REFUSALS.md:348–350`'s posture for the condition report), stated flatly, with no second person doing anything impressive |

**The tone rule, made concrete.** *"Played 34 times since March 2019"* is a
record. *"You've played this 34 times — one of your favourites!"* is a
performance. The difference is not the number; it is whether the sentence has an
opinion about the listener. Every string in F1–F11 is written in the first form,
and that is a reviewable property rather than a matter of taste.

**F11's edge case, admitted.** *"One of 47 records by this artist in your
collection"* is an aggregation — over the collection, not over history. It is
permitted because it is a fact about **the library's contents**, which is what
the wall already displays and what group keys already count; it says nothing
about listening. Had it been *"your 3rd most-played artist"*, it would be a
chart and refused.

### 8.5 What the feed costs

Honestly, and it is not zero:

- **The data is already in memory.** The history snapshot is read once at open
  (`app.rs:4212`, read via `read_history()` at `app.rs:5757–5773`) and the tags come from the index. No disk read happens to
  render a fact.
- **The clock is `time::every(20 s)` — three wakeups a minute**, under the same
  structural guard as §7.3 (place on screen, T3 on). For scale, baz already runs
  `REFRESH_TICK` at one wakeup a minute while idle (`app.rs:82`, installed at
  `app.rs:3921`), and ADR-0020's accepted idle cost includes it. Three is
  the same order of magnitude, it is stated rather than hidden, and it is
  **gone** when the place is not showing.
- **One new subscription to `PlayRecorded`.** For the count to be current rather
  than stale, the ledger must be re-read when a play is recorded. Today
  `Event::PlayRecorded` **has no consumer in `crates/baz` at all** — §12 step 5
  is that wiring, and it is the only place in this document where a readout needs
  new plumbing rather than a new view.

### 8.6 The network layer, if it is ever built

Every constraint below is a requirement, not a preference, and none of it is in
§12's plan — it is ranked in §13 as **D2**, deliberately behind everything local.

- **Individually opt-in**, per source, off by default (`README.md:22–24`,
  `VISION.md` pillar 3). Not one "enable online features" switch.
- **Blocked on an identifier baz does not store.** MusicBrainz, Discogs and
  Wikipedia are all MBID-keyed; §8.1's third absence is the real first step, and
  it is a scan-and-schema change rather than a UI one.
- **Cached to disk, and the cache is the user's** — same posture as the ledger:
  a plain local file they can inspect, back up or delete.
- **Attributed on the fact itself.** A fact from Wikipedia says so, in the line,
  because a screen that mixes *measured from your file* with *scraped from the
  web* without marking which is which is the beginning of snake oil
  (`REFUSALS.md:348–350`).
- **Refusable and failure-silent.** No network, no spinner, no error — the
  cycle simply has fewer facts in it, exactly as a record with no ReplayGain has
  no F6. This is the property that makes the local design load-bearing: **if the
  network layer's failure mode is "the feed is slightly shorter", the surface was
  designed correctly.**

---

## 9. The meter: a real measurement, or nothing

> *"some nice VU meter stuff over it in a stylised way, maybe somewhat
> ambient"*, and earlier: *"maybe we could have a visualizer mode at some point,
> but also VU options"*.

### 9.0 What it is, named precisely

**It is not a VU.** §2.3 sets out the four instruments people call one; this is
the fourth:

> **The default is a momentary-loudness meter to EBU R128 / ITU-R BS.1770-4** —
> K-weighted mean square over a **400 ms** sliding window, hopping every 100 ms
> — **with a sample-peak indicator beside it.**

**Why that one, and it is not a close call:**

1. **baz already owns the filter, derived and vector-tested.**
   `baz_core::loudness` implements BS.1770-4 K-weighting, and ADR-0015 §1 records
   that *"the filter is derived, not tabulated"* with all ten coefficients
   asserted against the standard's published Tables 1 and 2, plus five
   compliance vectors matching to within 0.025 LU (`0015:69–73`). **A meter
   built on this is correct by inheritance**; a meter built on a fresh
   peak-follower would be a new number nobody has checked.
2. **It is the same scale as a fact the surface already shows.** F6 states the
   record's *integrated* loudness, measured offline by the same code. Putting a
   *momentary* reading of the same quantity next to it means the two numbers are
   comparable — the live mark sits above or below the record's own average, and
   that is a genuinely informative thing to watch. No other choice of instrument
   gives that for free.
3. **It is the closest of the four to what a listener means by "how loud is this
   right now"**, because K-weighting is a perceptual weighting and peak is not.
4. **The 400 ms window is already the shape of the existing code.**
   `BLOCK_MS = 400` with `STEPS_PER_BLOCK = 4` (`loudness.rs:87–91`) is exactly a
   400 ms window hopping every 100 ms. The offline analyser and the live meter
   are the same measurement at different lifetimes.

### 9.1 "VU options" — the choice, offered

The owner's *"VU options"* is read as *offer the choice*, and three ballistics
ship. Each names its standard, because a meter that does not is furniture:

| Mode | Standard | Integration (rise) | Fall | What it is for |
|---|---|---|---|---|
| **Loudness** *(default)* | EBU R128 / ITU-R BS.1770-4, momentary | 400 ms K-weighted sliding window, 100 ms hop | (sliding window — no separate ballistic) | How loud this sounds; comparable to F6 |
| **VU** | IEC 60268-17 | **300 ms to 99 %** of a steady tone, with **1–1.5 % overshoot** | symmetric, 300 ms | The classic instrument, behaving as the classic instrument. Reads average; deliberately misses transients |
| **PPM** | IEC 60268-10 Type II (BBC/EBU) | **10 ms** | **2.8 s per 24 dB** | Catching transients the other two average away |

**A sample-peak hold** sits alongside all three, in dBFS, with a 1.5 s hold and
an instant reset on track change. It is the only one of the four numbers that is
sample-accurate, and it is the one that answers *is this clipping*.

**Two disciplines carried from ADR-0015, because they are what make this
different from every "VU meter" in §2.3:**

- **Compliance vectors ship with the ballistics.** ADR-0015 asserted its filter
  against the standard's own tables and five test signals; the VU and PPM modes
  get the same treatment — a step of sine at reference level must reach 99 % in
  300 ms ± tolerance for VU, and the PPM's fall must cross 24 dB in 2.8 s.
  **A ballistic without a test is a guess with a standard's name on it.**
- **The published constants are checked against the published document before
  implementation, not from memory.** The figures in the table above are stated
  to be verified at implementation time — that is §12 step 8's gate and §13 R1's
  ranking, and it is exactly the posture ADR-0015 took when it refused to ship
  true peak *"without [its compliance vectors]"* (`0015:148–151`).

### 9.2 Where it taps, and why there

The pump is `Session::pump` (`engine.rs:2735–2782`), and its shape decides
everything:

```rust
let transparent = fader.is_transparent();
let (a, b) = chunk.as_slices();          // ← the ring: the decoded file
if transparent {
    sink.write(a);                        // bit-exact: untouched to the device
    if !b.is_empty() { sink.write(b); }
} else {
    scratch[..split].copy_from_slice(a);
    scratch[split..n].copy_from_slice(b);
    let block = &mut scratch[..n];
    fader.apply(block, rate);             // ← the ONE gain stage
    sink.write(block);
}
```

> **The tap reads `a` and `b` — the ring's own content, before the fader — in
> both branches.**

**This is pre-gain, and that is the decision.** `settle_volume`
(`engine.rs:1845–1874`) folds ReplayGain and volume into **one** number
(`applied = volume_applied * replay_gain`, `:1859`) applied in **one** place,
`Fader::apply` (`volume.rs:442–485`). So the ring holds the decoded file and
nothing has touched it yet. Four consequences, and each is a reason:

1. **The meter can never contradict `bit-perfect`, because it never observes the
   gain stage at all.** The bar's `bit_exact()` asks `VolumePath::is_transparent`
   — the engine's own answer (`player.rs:2034–2038`) — about a stage the meter
   is upstream of. There is no reading either surface can produce that disagrees
   with the other, and this is structural rather than a matter of keeping them in
   sync.
2. **One tap, one meaning, in both branches.** A post-fader tap would have to
   read `a`/`b` in the transparent branch and `block` in the scaled one — the
   same code path producing two different quantities depending on the volume
   knob. In the transparent branch pre- and post- are *bit-identical* anyway
   (the gain is exactly 1.0, `volume.rs:198–203`), so pre-gain is the reading
   that is stable across the branch rather than the one that changes with it.
3. **It measures the record, not the volume knob.** Turning the volume down does
   not move the meter — which is what a console VU does, what makes the reading
   worth looking at, and what makes it comparable to F6's stored figure (also
   measured from the decoded file, `loudness.rs`).
4. **It reads before ReplayGain too**, which must be **labelled** or it is snake
   oil (`REFUSALS.md:348–350`). The instrument's own caption says what it
   measures — *the file as decoded* — so a listener who sees the meter unmoved
   after enabling ReplayGain has the answer on screen rather than a mystery.

**Metering cannot alter a sample, and it is not promised — it is unspoiled by
type.** The tap's signature is:

```rust
fn observe(&mut self, samples: &[f32])
```

`&[f32]`, never `&mut [f32]`. `a` and `b` come from `chunk.as_slices()`, which
yields shared slices; the meter is handed those. **There is no expressible
mutation**, so ADR-0009's bit-exactness is not defended by a test here — it is
defended by the borrow checker, and the existing bit-exactness tests continue to
pass unmodified because the code they exercise is untouched.

### 9.3 Zero cost when off — structurally

The requirement is *zero, not small*. An `AtomicBool` checked per block would be
small; this is zero:

> **The meter is an `Option<LiveMeter>` owned by the session, created and dropped
> by a `Command`.** When it is `None` there is no filter state, no memory, and no
> arithmetic — the pump does one null check on engine-thread-local state per
> block, which is the same class of check as `self.next_boundary()`
> (`engine.rs:2754`) that it already performs every block.

```rust
// In pump, after the slices are obtained. Nothing here can mutate them.
if let Some(meter) = &mut self.meter {
    meter.observe(a);
    if !b.is_empty() { meter.observe(b); }
}
```

- **No atomic is loaded on the disable path.** The UI's toggle sends
  `Command::SetMetering(bool)`; the engine thread swaps the `Option` between
  commands, never inside a pump. There is no cross-thread read per block.
- **A block is 1024–8192 samples**, so this branch is evaluated tens of times a
  second, not tens of thousands.
- **When it is `None`, the K-weighting filters do not exist**, so the cost is
  not "a skipped multiply" — it is an absent object.

**And the whole chain switches off together.** T2 off ⇒ no `Command::SetMetering`
⇒ no `LiveMeter` in the engine ⇒ no atomic being written ⇒ no
`window::frames()` arm in the subscription (§7.3) ⇒ the loop parks. **Each layer's
off state is the absence of a thing rather than a guard around it**, which is
what makes §7.3's claim hold end to end.

### 9.4 How levels cross to the UI

The precedent is in the codebase twice, and this is the third instance of the
same pattern rather than a new mechanism:

- `DeviceSink`'s callback publishes its counters with a **plain store**, single
  writer, no read-modify-write: *"Callback-owned counters: the callback is their
  only writer, so it keeps them locally and publishes with a plain store — no
  read-modify-write on the realtime path"* (`device.rs:385–387`,
  `:420–421`). The seek watermark is the same idea in the other direction
  (`device.rs:337–342`, `:397`).
- `SharedVolume` holds a gain as **one `AtomicU32` carrying the f32's bit
  pattern** — *"the wait-free way"* (`volume.rs:244–249`).

So:

```rust
/// Engine → UI. One writer (the engine thread, inside the pump), any number
/// of readers. Two f32 bit patterns; no lock, no allocation, no RMW.
pub struct SharedMeter {
    momentary: AtomicU32,   // LUFS (or the selected ballistic's reading)
    peak:      AtomicU32,   // dBFS, sample peak, with hold applied
}
```

- **Published once per completed 100 ms step**, not per sample and not per
  block: the meter accumulates in engine-thread-local state and does two
  `store(Ordering::Release)` when a step closes. That is **10 stores a second,
  total**.
- **Read once per frame** by the UI with `load(Ordering::Acquire)`.
- **Torn reads are impossible and staleness is bounded** by construction: each
  value is a single 32-bit atomic, and the worst a frame can see is a reading
  100 ms old — a third of a VU's own integration time, i.e. below the
  instrument's resolution.
- **The two values are deliberately not read atomically together.** They are
  independent measurements displayed independently; a frame that shows a
  momentary reading from step *n* and a peak from step *n+1* is not wrong, and
  paying for a seqlock to prevent it would be paying for a problem that does not
  exist.

**What is *not* reused, and why.** `LoudnessMeter` (`loudness.rs:211–231`) is
the offline instrument and **must not** be used on this path: `close_step`
pushes to `self.blocks: Vec<f64>` for the album gate, and a `Vec::push` can
reallocate — which is exactly what the pump path may not do. The live meter
reuses **`KWeighting`** (`loudness.rs:153–201`, `run(&mut self, x: f64) -> f64`,
pure state, no allocation) and re-implements the step/window accumulation with a
fixed-size ring. That is a `pub(crate)` on one struct and no new dependency.

**On ADR-0015's reversal clause, engaged rather than skipped.** ADR-0015 §3
names what would reverse its hand-rolled-rather-than-crate decision: *"needing
BS.1770-4 Annex 2 true peak, **a momentary or short-term meter**, loudness
range, or more than two channels"* (`0015:132–135`). This design needs a
momentary meter, so the clause is live and must be answered rather than walked
past. The answer: the clause is about the **analysis unit's** completeness — the
point at which a general-purpose crate's generality stops being a cost — and
what this needs is not generality but **the K-weighting filter already written
and already vector-tested**, with less machinery around it rather than more (no
gating, no LRA, no album pass, two channels). Taking a dependency to get a
subset of what baz already has, on the realtime path, with its allocation
behaviour unaudited, would be the worse trade. **§12 step 8 records this as a
decision re-made rather than defended**, which is what the clause asks for.

### 9.5 The two registers, and where they are drawn

This is where the owner's *"stylised… somewhat ambient"* and §1.2's two
distances turn out to be the same requirement. **One measurement, two readouts:**

**The ambient register — the field responds.** The field's overall luminance and
the scale of its wash track the momentary reading, gently: a mapping from LUFS
to a narrow luminance band, heavily smoothed, never exceeding §5.3's L 0.22
ceiling. From 3 m you do not read a number — **the room breathes with the
music**. This is the "VU meter stuff over it" the brief asks for, drawn **over
the field, never over the sleeve** (§5.4).

Two constraints keep it honest:
- **It is bounded and slow.** The luminance band is narrow enough that a loud
  passage cannot make the field compete with the artwork, which is the one thing
  §5.3 promised.
- **It states nothing** (§7.1's last clause). Nobody can read a level off a
  breathing room, and it is not asked to carry one. It is the ambient class
  doing what the ambient class is for.

**The instrument register — the meter proper.** On the placard column, at the
work's own width, `METER_H` 24: a horizontal scale with the current reading, the
peak hold, and — the detail that makes it worth having — **a fixed mark at this
record's own integrated loudness (F6)**, so the live reading is legible as
*louder or quieter than this record's average* rather than as an abstract
number. At 60 cm it is an instrument; at 3 m it is a moving line, which is fine,
because the ambient register is what the far field is reading.

### 9.6 Why the meter is not amber, and other refusals kept

- **Not amber.** `REFUSALS.md:271–274`: the accent *"states what is true about
  playback right now and nothing else: not what is queued, not what is selected,
  not what has focus"*. A level is not *which record, which track, where the
  playhead is* — it is a measurement of the audio. The meter draws in the room's
  own inks (`paper`, `paper_dim`, `paper_faint`); the needle keeps the amber,
  because the needle is exactly what the accent is reserved for. **The one
  exception is the peak indicator crossing 0 dBFS**, which is playback truth of
  the kind the accent exists for — and it is accompanied by the numeral, because
  `REFUSALS.md:259–261` forbids state signalled by colour alone.
- **Not an instrument face.** §14.3 rewrites the skeuomorphism entry, and the
  line it draws is the entry's own: *"the record supplies physics, structure and
  vocabulary… it never supplies **surface**."* **Refused: a beige panel, a glass
  face, a printed arc scale, a pivoting needle, a bezel, a lamp behind the
  dial.** **Permitted: the measurement**, drawn in baz's own vocabulary — a line,
  a mark, a numeral, the room's inks. The owner's *"in a stylised way"* is read
  as exactly this: the instrument's *behaviour*, not its *costume*.
- **No peak-hold-forever, no session maxima, no "loudness score".** Those are
  engagement stats about audio, and `REFUSALS.md:68–73`'s tone rule applies to
  the meter as much as to the feed.
- **No headroom claim, no "audiophile" framing.** `REFUSALS.md:348–350`. The
  meter reports a number and names its standard. It does not say the number is
  good.

---

## 10. The spectrum analyser: the visualizer, promoted

> *"is it a spectrum analyzer or graphic thing with the bars going up and
> down… that would be nice"* — the owner, 2026-08-09

**What this collapses.** The earlier brief asked for *"a visualizer mode at some
point, but also VU options"*, and this document deferred the visualizer as
**D1** while designing the meter. The bars **are** that visualizer, and *"at
some point"* has become now. So §10 stops being a deferral with a price and
becomes the design; §13's D1 is retired and replaced.

**What it does not collapse.** The R128 meter of §9 stays, in full. The two are
not competing answers to one question — **the bars are a *visual* and the meter
is a *reading***, and §10.7 specifies the guarantee that they can never tell
different stories about the same instant.

### 10.1 The FFT: `realfft`, and it costs nothing

**The decision: take `realfft` 3.5.0.** Not because a hand-rolled radix-2 would
be hard, but because of a fact that settles it outright:

> **`realfft` and `rustfft` are already in `Cargo.lock`.** `rubato` — baz's
> windowed-sinc resampler, a non-optional `baz-core` dependency
> (`crates/baz-core/Cargo.toml:25`, used at `playback/resample.rs:32`) —
> depends on `realfft`, which depends on `rustfft`. Both are compiled into
> every baz build today.

So the cost of the usual objection is zero, and each part of it is checked
rather than assumed:

| Question | Answer | How known |
|---|---|---|
| New crates in the tree? | **None.** `realfft` 3.5.0 → `rustfft` 6.4.1 → `num-complex`, `num-integer`, `num-traits`, `primal-check`, `strength_reduce`, `transpose` — every one already present | `Cargo.lock` (`rubato` 0.16.2's dependency block) |
| Licences? | `realfft` **MIT**; `rustfft`, `primal-check`, `strength_reduce`, `transpose`, `num-complex` all **MIT OR Apache-2.0** | each crate's `Cargo.toml` in `~/.cargo/registry` |
| Already allowed by policy? | **Yes** — `MIT` and `Apache-2.0` are both on `deny.toml`'s allowlist, which is a *"GPL-3.0-compatible allowlist [whose] extending… is a reviewed decision"*. **No extension is needed** | `deny.toml`, `[licenses] allow` |
| A C library or build dependency? | **No.** None of the five has a `build.rs` or a `links` key, and none is a `-sys` crate | checked in the registry source |
| Does `cargo deny check` change? | **No.** The graph is unchanged — this adds a direct edge to a crate already in it | follows from the above |

**Why this matters more here than usual.** `docs/BACKLOG.md:122–131` sets the
standard, in the Opus decision: libopus bindings were refused because they cost
*"a **C library and a `cmake` build dependency on every platform**… baz's decode
path is pure Rust with **zero system dependencies** today (even SQLite is
`bundled`); spending that property on one lossy format is not a trade worth
making unprompted."* **That property is not spent here.** The FFT is pure Rust,
already present, and adds no build step on any platform.

**And the hostile-input concern does not apply.** The same BACKLOG note refuses
young pure-Rust Opus decoders because *"this is a parser sitting in front of
hostile input from the user's own filesystem"*. An FFT is not a parser: it
consumes `f32` samples the decoder has already produced and validated, and its
input length is baz's own constant. `ENGINEERING.md`'s *prefer proven crates*
points the same way it did there — and `rustfft` is the proven one.

**Why not hand-roll it, given ADR-0015 hand-rolled the K-weighting.** The
distinction is what a skeptic needs to audit. ADR-0015's reason was
verifiability against a *published standard*: *"`loudness.rs` puts the filter
derivation, the gate and the standard's published constants where a skeptic can
read them against BS.1770-4 in one sitting"* (`0015:128–131`). **An FFT has no
standard to audit** — it has an answer, and the answer is checkable
mechanically against a naive DFT. There is nothing a reader must be able to read
in one sitting, so the argument that carried ADR-0015 does not reach this, and
the ordinary preference for proven code applies unopposed.

**The API, and the allocation property that matters.** `realfft`'s
`process_with_scratch(input, output, scratch)` takes a caller-owned scratch
buffer, in contrast to `process`, which *"allocates additional scratch space as
needed"* (`realfft-3.5.0/src/lib.rs:127–145`). The planner is built once and the
buffers are owned; **the per-frame path allocates nothing.**

### 10.2 Where it runs: not the audio path, and never a queue

Three threads matter, and the transform is on none of the first two:

```
audio callback  (device.rs:390–422)   ── untouched. Nothing added, ever.
engine thread   (Session::pump)       ── the tap: downmix + one ring write
UI thread       (view, once a frame)  ── the FFT, the banding, the draw
```

**The tap**, beside §9.2's meter call, on the same `a`/`b` slices at the same
instant, pre-gain:

```rust
if let Some(meter) = &mut self.meter {
    meter.observe(a);                       // §9 — K-weighted, continuous
    if !b.is_empty() { meter.observe(b); }
}
if let Some(ring) = &self.spectrum {
    ring.write_downmixed(a);                // §10 — mono, overwriting
    if !b.is_empty() { ring.write_downmixed(b); }
}
```

Both take `&[f32]`. **Neither can mutate a sample**, for §9.2's reason: the
type does not permit it, so ADR-0009's bit-exactness is defended by the borrow
checker and its tests pass unmodified.

**The downmix is `(l + r) * 0.5`** — one add and one multiply per frame, about
88 200 flops/second at 44.1 kHz stereo, which is an order of magnitude *below*
the K-weighting the meter already does beside it. A spectrum of a stereo mix is
conventionally the mono sum, and it halves the ring.

**The ring is an overwriting ring, not a queue — this is the whole answer to
"what if the UI is slower than the audio".**

> **`SpectrumRing`: 16 384 `f32` (64 KiB), power-of-two, single writer (engine),
> single reader (UI). The writer never blocks and never fails; it overwrites the
> oldest samples and publishes a monotonically increasing write count. The
> reader takes the most recent 2048 samples and ignores everything else.**

- **Drop, never queue.** There is no backpressure path from the UI to the
  engine, and no way for a slow UI to make the ring grow. A UI that misses
  frames simply analyses a more recent window next time; the audio is never
  affected, because the writer's cost is a memcpy into a fixed buffer whatever
  the reader is doing.
- **The margin is large and stated.** 16 384 mono samples is **371 ms** at
  44.1 kHz. The reader needs the newest 2048 (46 ms). At 60 fps it returns every
  16.7 ms, so it is roughly **22× inside** the window before the data it wants is
  overwritten. Even a UI stalled to 3 fps still reads intact samples.
- **A torn read is possible and harmless, and that is a decision.** If the
  writer laps the reader mid-copy the frame shows a spectrum spliced from two
  instants — one frame, on a visual, at 60 fps. Paying for a seqlock to prevent
  something invisible is paying for a problem that does not exist; §9.4 declined
  the same trade for the same reason. This is stated so nobody later "fixes" it
  under the impression it was overlooked.
- **The meter does not read the ring**, and that is deliberate. K-weighting is a
  stateful IIR filter that must see **every** sample in order or its state is
  wrong; a consumer permitted to drop samples cannot host it. The FFT is
  stateless per window and can. So each measurement sits where its own
  arithmetic requires — and §10.7 shows this costs nothing in agreement.

### 10.3 The transform, and its constants

| Parameter | Value | Why |
|---|---|---|
| Size | **2048** real samples | 21.5 Hz bins and a 46 ms window at 44.1 kHz. 1024 gives 43 Hz bins — too coarse in the bass; 4096 gives 93 ms — visibly sluggish on attacks |
| Window | **Hann** | Standard for display analysis: −31 dB side lobes, and the spectral leakage that would otherwise smear a tone across neighbouring bars |
| Output | 1025 complex bins (0…Nyquist) | `realfft` returns `N/2 + 1` |
| Rate | once per drawn frame, from the newest 2048 samples | Overlapping windows at 60 fps; no hop bookkeeping, because the ring always holds "the most recent" |
| Normalisation | a full-scale sine reads **0 dBFS** in its own band | So the bars and §9's meter share one reference (§10.7) |

**Cost, and it is small.** A 2048-point *real* transform is roughly half the
work of a 2048-point complex one — `realfft` exists for exactly that — at about
`(N/2)·log₂(N/2) ≈ 1024 × 10 ≈ 10 240` butterflies, order **10⁵ flops per
frame**, ≈ 6 Mflop/s at 60 fps. *Engineering estimate: **20–60 µs per frame**,
i.e. **0.1–0.4 %** of a 16.7 ms budget.* Labelled an estimate; §10.8 measures it.

### 10.4 The banding: linear bins, logarithmic music

FFT bins are linear and pitch is logarithmic, so the mapping is specified rather
than left to the drawing code.

**Range: 32 Hz – 16 kHz**, nine octaves. Not 20 Hz–20 kHz, and honestly so: the
first bin above DC is 21.5 Hz, so *"20 Hz"* would be a label for a band with no
resolution behind it, and above 16 kHz there is nothing in most material and
nothing in most listeners.

**Band edges are geometric**: `f(i) = 32 · (16000/32)^(i/bars)`, so every bar
spans the same musical interval. Each bar's value is the **sum of the power** of
the bins whose centres fall in its band — energy, not maximum, so a band's value
does not depend on how many bins it happens to contain.

**Bar count is derived from width, not fixed**, because a count that looks right
at 1280 is wrong at 4K:

```
bars = round(body_width / (24 · kiosk_scale)).clamp(24, 64)
```

| Window | body width | `kiosk_scale` | bars | bar pitch |
|---|---|---|---|---|
| 1280 × 800 | 1184 | 1.0 | **49** | 24 px |
| 1920 × 1080 | 1824 | 1.0 | **64** (clamped) | 28 px |
| 3840 × 2160, 3000 px cover | 3744 | 2.5 | **62** | 60 px |

Keyed to `kiosk_scale` (§11.2) for the reason the type is: at 3 m the bars must
get **chunkier**, not merely more numerous. A 4K panel gets 62 bars at 60 px
rather than 156 at 24 px, and the ceiling of 64 keeps the count musically
sensible — about 7 bars per octave, near the 1/6-octave resolution a hardware
analyser uses.

**The bottom-octave limit, stated rather than faked.** With 21.5 Hz bins, the
32–64 Hz octave contains about **1.5 bins** while seven bars want to span it. So
the lowest bars **share bins and move together**, and that is what the data
supports — the alternative is interpolating a resolution the transform does not
have, which is drawing a number that was never measured. If the bass ever reads
as visibly ganged, the honest fix is a 4096-point transform for the lowest
octave only, and it is ranked in §13 rather than pre-built.

### 10.5 The scale, the floor, and silence

- **Scale: dBFS**, with the bar's height linear in decibels — the same
  convention as every analyser, and the one that matches hearing.
- **Floor: −72 dBFS.** Twelve bits below full scale: far under the noise floor
  of any 16-bit material (≈ −96 dBFS) yet high enough that dither and room tone
  in a quiet passage do not make the bars twitch. Displayed travel is therefore
  **72 dB**, floor to full scale.
- **Digital silence is exactly flat, and it is exact rather than approximate.**
  If every sample in the window is `0.0`, every bin's magnitude is exactly
  `0.0`, which clamps to the floor and draws zero height. **The bars are still —
  not low, not shimmering.** *Silence is a feature* (`REFUSALS.md:19–21`) is
  drawn rather than merely honoured, which is the same property §7.6 relies on
  for the stopped case.
- **Stopped or paused: the surface holds no bars at all.** When nothing is
  sounding the place already states silence in words and draws no artwork
  (`now_playing.rs:92–104`, S5) — so there is no field and no spectrum either,
  and the ring is not being written. On *pause* the bars fall to the floor under
  the decay ballistic rather than snapping, which is the honest picture of
  audio stopping.

### 10.6 Ballistics, and the shared options family

**Attack is instantaneous; decay is exponential.** A new value above the current
bar takes it immediately — an analyser that lags a transient is lying about the
transient — and a value below it decays at a fixed dB/second.

**Peak-hold caps** — the small marks that hang above each bar and fall — are the
Winamp and foobar2000 signature, they cost one float per bar, and they are what
makes a fast-decaying bar readable.

The **Ballistics** selector of §9.1 governs both instruments, which is what stops
the surface having two unrelated speed settings:

| Ballistics | Meter (§9.1) — *standardised* | Bars: decay | Cap: hold, then fall |
|---|---|---|---|
| **Loudness** *(default)* | R128 momentary, 400 ms window | 20 dB/s | 2.0 s, then 10 dB/s |
| **VU** | IEC 60268-17, 300 ms to 99 % | 30 dB/s | 1.5 s, then 15 dB/s |
| **PPM** | IEC 60268-10 Type II, 10 ms rise | 60 dB/s | 1.0 s, then 20 dB/s |

**One honesty note that must survive into the code.** The meter's column is
**standardised** and tested against published documents (§9.1, §13 R1). **The
bars' column is conventional** — these are display constants chosen because they
look right, not because a standard specifies them, and no test asserts them
against any document. Naming them in one table must not launder the second
column into the authority of the first, and the code's comment says so.

### 10.7 How the bars and the meter cannot disagree

They read the same samples, at the same instant, through the same (absent) gain
stage. The differences that remain are stated transformations, not
discrepancies:

| | Meter (§9) | Bars (§10) |
|---|---|---|
| Source | `a`/`b` in `pump`, pre-gain | the same `a`/`b`, same call site, pre-gain |
| Weighting | **K-weighted** (BS.1770-4) | **unweighted** |
| Domain | one number | 24–64 bands |
| Reference | dBFS / LUFS | dBFS, same reference |

So a bass-heavy passage reads lower on the meter than the bars' bottom bands
suggest — **because K-weighting attenuates the bass by a published amount**, and
that is the meter being correct, not the two disagreeing. Neither follows the
volume knob, because neither observes the fader.

**Three properties, asserted as tests rather than argued:**

- `the_meter_and_the_bars_agree_on_silence` — at digital silence the meter is at
  its floor and every bar is at zero height, in the **same frame**. Both derive
  from the same zero samples, so this is exact.
- `the_meter_and_the_bars_agree_on_a_full_scale_sine` — a 1 kHz full-scale sine
  puts exactly one band at 0 dBFS and the meter at the LUFS the K-weighting
  curve predicts for 1 kHz, which is a number `loudness.rs` can already compute.
- `neither_instrument_moves_with_the_volume` — sweeping the fader from unity to
  silence changes no reading on either.

### 10.8 The look

This is the part the owner weighted most — *"stylised"*, *"somewhat ambient"* —
and the thing to avoid is named: a harsh green Winamp bank.

**The bars are drawn inside the field's own shader, as light rather than as
widgets.** One full-screen quad already exists for the field (§7.5); the bar
heights arrive as a **uniform array of 64 floats — 256 bytes per frame** — and
the fragment shader decides, per pixel, whether it is inside a bar. So the
drawing cost of N bars is **not N quads**; it is a slightly longer shader on the
quad the surface already draws.

**Colour: the cover's own palette, never the accent.**

- The bars use the **brightest** of the three colours §5.3 samples from the
  artwork, so they belong to the record rather than to a theme.
- Their luminance ceiling is **L ≤ 0.38** — above the field's 0.22 so they are
  legible against it, below the sleeve so **the artwork remains the brightest
  object on the screen**, which is the promise §5.3 made and this must not
  break.
- **Not amber.** `REFUSALS.md:271–274` reserves the accent for *"what is true
  about playback right now… not what is queued, not what is selected"* — and a
  spectrum is a property of the audio, not playback truth. The needle keeps the
  amber; the bars never take it.
- A cover with no chroma yields the room's own ink, on §5.3's existing fallback.

**Shape.** Flat-topped columns with a `GAP_XXS` 2 px gutter, rising from the
bottom edge of the body, height = normalised dB × `BAND_H`, where
`BAND_H = body_height × 0.45` — so a full-scale bar reaches 45 % up the body:
dramatic from a chair, never near the top. The caps are a 2 px mark in the same
colour at higher opacity. No radius, no gloss, no gradient along the bar's
length, no mirrored reflection.

**The mask, which is what makes it work.** The bars are full-bleed and would
otherwise run behind the placard, and moving light under type is hard to read
even at safe contrast. So:

> **The bars' opacity is multiplied by a soft-edged mask that goes to zero over
> the centred column** — the work and the placard, expanded by `GAP_XL` 24, with
> the transition falling over a further `GAP_XL`.

The composition sits in a calm pocket and the spectrum surrounds it. **This
costs no layout at all** — `below` is unchanged, so the artwork keeps every
pixel §5 fought for. And it is the discipline the ledger already blessed for the
hover veil: *"a gradient that dies before the [edge], never a flat panel"*
(`REFUSALS.md:113–121`).

**1920 × 1080, lane collapsed — 64 bars, 28 px pitch:**

```
 ┌──────────────────────────────────────────────────────────────────┐
 │▓▓▓▓▓ the field, derived, L ≤ 0.22 ▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓│
 │▓ 96 ▓                                                          ▓▓│
 │▓lane▓                  ┌──────────────┐                        ▓▓│
 │▓    ▓                  │              │                        ▓▓│
 │▓Home▓                  │   the work   │  729 × 729             ▓▓│
 │▓Libr▓                  │              │                        ▓▓│
 │▓Now ▓                  └──────────────┘   ← mask: bars duck    ▓▓│
 │▓ ●  ▓                  T A L K   T A L K     behind the column ▓▓│
 │▓    ▓                  Spirit of Eden                          ▓▓│
 │▓    ▓                  Spirit of Eden · 1988                   ▓▓│
 │▓    ▓                  ├──────────────┤ needle                 ▓▓│
 │▓    ▓                  3:12              6:27                  ▓▓│
 │▓  ▁▂▓ Played 34 times since 2019           ▁▃         ▂▁       ▓▓│
 │▓ ▃█▅▂▃▁      ▂▄▃▁         ▁▂▄▅▃▂        ▂▄██▅▃    ▁▃▅█▆▄▂▁     ▓▓│
 │▓██████▅▃▂▁▂▄███▆▄▂▁▃▅▆▄▂▄███████▅▃▂▁▂▄▆████████▄▂▄████████▅▃▂▁ ▓▓│  ← BAND_H = 45 %
 ├──────────────────────────────────────────────────────────────────┤
 │ the bar — 81 px, unchanged, in every place                       │
 └──────────────────────────────────────────────────────────────────┘
    32 Hz ◄────────────── nine octaves, geometric ──────────────► 16 kHz
```

**3840 × 2160 — 62 bars at 60 px, everything at `kiosk_scale` 2.5:**

```
 ┌────────────────────────────────────────────────────────────────────────────┐
 │▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓│
 │▓ 96▓                                                                     ▓▓│
 │▓   ▓                      ┌────────────────────┐                         ▓▓│
 │▓   ▓                      │                    │                         ▓▓│
 │▓ ● ▓                      │     the work       │  1809 × 1809            ▓▓│
 │▓   ▓                      │                    │  (source-bound)         ▓▓│
 │▓   ▓                      └────────────────────┘                         ▓▓│
 │▓   ▓                                                                     ▓▓│
 │▓   ▓                      T A L K   T A L K        ← 25 px tracked caps  ▓▓│
 │▓   ▓                      Spirit of Eden           ← 70 px title         ▓▓│
 │▓   ▓                      Spirit of Eden · 1988    ← 33 px               ▓▓│
 │▓   ▓                      ├────────────────────┤   ← 5 px needle         ▓▓│
 │▓   ▓                      3:12            6:27                           ▓▓│
 │▓  ▁▓ Played 34 times since 2019  ← 33 px               ▁▃                ▓▓│
 │▓ ▄█▃▁▂    ▁▃▂        ▂▄▃▁         ▁▂▄▃▂        ▂▄█▅▃      ▃▅█▆▄▂         ▓▓│
 │▓███████▄▂▄███▅▃▂▁▃▅███████▄▂▁▂▄▆████████▅▃▂▄██████████▄▂▄██████████▅▃▂▁  ▓▓│
 ├────────────────────────────────────────────────────────────────────────────┤
 │ the bar — 81 px, unchanged                                                 │
 └────────────────────────────────────────────────────────────────────────────┘
```

**The fallback, required rather than nice.** §0.3(7) found the `shader` widget
renders nothing under `tiny-skia`. T1's field already falls back to a static
wash; the bars fall back to **`Canvas` geometry** — N flat rectangles and their
caps, in the same sampled colour, with the mask reduced to a hard rectangular
exclusion rather than a soft gradient. `Canvas` draws under every backend, the
geometry is tens of vertices, and `Cache::with_group` (§0.3(6)) keeps the
static part out of the per-frame re-tessellation. **A software-rendered baz gets
a still field and working bars, not a hole.**

### 10.9 What this costs, and the thresholds it must clear

The bars are the most expensive thing on this surface, so §7.4's gate is
extended rather than reused unchanged:

| Added metric | Threshold | Why |
|---|---|---|
| FFT + banding, per frame, CPU | **< 1 ms** | The estimate is 20–60 µs; a millisecond is over an order of magnitude of headroom, so exceeding it means something is wrong rather than expensive |
| Frame time with bars, 1920 × 1080, 99th pct | **< 8 ms** | Unchanged from §7.4 — the bars must fit inside the budget already set, not enlarge it |
| Frame time with bars, 3840 × 2160, 99th pct | **< 12 ms** | Unchanged, and this is the case the owner described |
| Ring write cost, engine thread, per block | **< 5 µs** | It is a memcpy plus a downmix; this is a guard against an accidental per-sample atomic |
| All toggles off, idle | **0.0 % CPU, 0 frames** | §7.3's structural claim, unchanged and re-asserted with the bars present |

The harness is doc 04 §1.3's, already extended in §7.4; the `ambient` driver
gains the FFT and the bar draw. **The owner's ruling is the standing condition
— *"ambient motion is fine as long as the performance remains top tier"* — so
the bars are welcome and these numbers are the price of entry.**

**Off is still structurally zero**, by §9.3's mechanism exactly: the ring is an
`Option<SpectrumRing>` owned by the session and swapped by a `Command`. When
`None` there is no buffer, no downmix and no write — and with T2 off there is no
`window::frames()` arm, so the loop parks (§7.3).

---

## 11. Kiosk mode: one drag and one key

### 11.1 The decision, and the toolkit fact behind it

> **The kiosk is `Place::NowPlaying` full-screen in baz's one window. There is
> no second window.**

§0.3(2) is the evidence, verified against the installed source rather than
recalled: **iced 0.13 has no monitor enumeration in its public API at all** —
`MonitorHandle`, `available_monitors` and `primary_monitor` appear nowhere in
`iced`, `iced_core` or `iced_runtime`. winit models it
(`Fullscreen::Borderless(Option<MonitorHandle>)`, used at
`iced_winit/src/conversion.rs:398`) but iced never exposes the `Option`; it
chooses for you. Even the escape hatch fails: `window::run_with_handle` yields a
`raw_window_handle::WindowHandle`, not winit's `Window`
(`iced_winit/src/program.rs:1404–1413`), so there is no `current_monitor()` to
ask.

**So a second window cannot be put on the monitor you name.** It would appear
wherever the compositor decides, and you would drag it — which is the gesture
the single-window answer uses directly, without the `daemon` migration.

**And §7.3 found a second, stronger reason.** iced's control flow is **global,
not per-window** (`iced_winit/src/program.rs:471–488`), and after any message
batch **every** window is redraw-requested (`program.rs:1089–1097`). In a
two-window baz, the kiosk's ambient clock would pace the main window too, and
ADR-0020's 0.0 % idle claim would become false for the whole product the moment
the kiosk opened. **The single-window design is what keeps the ambient motion
free elsewhere**, which turns a toolkit limitation into a design constraint worth
having.

**What delivers the brief**, and it works today on both display servers:

> Put the window on the monitor you want — the gesture every desktop already
> has — press `Now playing` in the lane, press `F11`. The place fills **that**
> monitor, because `Mode::Fullscreen` lands on the monitor the window currently
> occupies (`iced_winit/src/program.rs:1331–1338`; `Mode`'s own doc says *"the
> whole screen of its **current monitor**"*, `iced_core/src/window/mode.rs:7`).

Two notes that make it better than it sounds:

- **The desktop can automate it.** Both major Linux compositors match window
  rules on `app_id`, and baz ships a desktop entry and an `app_id`
  (`packaging/`). A user who wants baz to open fullscreen on `DP-2` every time
  writes three lines of compositor config. baz should not grow a monitor picker
  to serve that: the window manager is the one component that knows what the
  monitors are called.
- **The cost of reopening this is recorded.** If iced ever exposes winit's
  `Fullscreen::Borderless(Option<MonitorHandle>)`, a second window on a named
  monitor becomes a small change on top of a `daemon` migration — and §7.3's
  global-control-flow objection would still have to be answered. §13 records
  both, so re-proposing it means citing a toolkit change *and* solving the idle
  coupling, rather than re-arguing taste.

`F11` is a **window** act, not a place act: it works in every place, exactly as
`Esc` peels fullscreen before it peels the place (§3.3). The kiosk does not own
a private key.

### 11.2 What changes at kiosk size

**The bug this fixes first.** §5.5's table shows the shipped clamp giving a 4K
panel a **720 px** work in a **3744 × 2079** body — 19 % of the width. That is
not a kiosk, and it is the direct consequence of `NOW_PLAYING_MAX` being a fixed
constant standing in for a fact about the decode (§0.4 b). §5.2 deletes it, and
the work becomes as large as the viewport and **the source file** allow — 1809 px
at 4K from a 3000 px cover.

**The type scale, which is the rest of the answer.** baz's largest token is
`SIZE_HERO` 28 (`theme.rs:845`), sized for a 60 cm reading of a 1280 px window.
At 3 m on a 4K panel it is not small, it is **absent** (§1.2). So the kiosk
introduces a scale step — and it is derived, not a second token sheet:

```
kiosk_scale(edge) = (edge / 720).clamp(1.0, 2.5)
```

Keyed to `edge` — the work's own resolved size — rather than to the window,
because `edge` is already the surface's one derived measure
(`now_playing.rs:49–72`) and because it is the thing the type sits under and
aligns to. A bigger sleeve gets bigger type by construction, and the placard
never outgrows the work it labels.

| Element | Token | 1920 (edge 729) | 4K (edge 1809) |
|---|---|---|---|
| Title | `SIZE_HERO` 28 | 28 | **70** |
| Artist (tracked caps) | `SIZE_HEADING` 10 | 10 | **25** |
| Album | `SIZE_BODY` 13 | 13 | **33** |
| Feed line | `SIZE_BODY` 13 | 13 | **33** |
| Figures | `SIZE_META` 12 | 12 | **30** |
| Needle | `NEEDLE_H` 2 | 2 | **5** |

The clamp's floor of 1.0 is what keeps every window at or below 720 px of work
**pixel-identical to what ships today**, so this change cannot regress the
desktop case. The ceiling of 2.5 stops a very large source producing type that
is absurd at 60 cm on the same panel.

**What does not change at any size**, and this is the property that made the
shipped surface right in the first place: the composition. There is no separate
kiosk layout, no second view function, no mode. `now_playing.rs`'s own test —
*"the kiosk is this surface at a larger size, and it is a property of the
arithmetic rather than a plan"* (`now_playing.rs:214–234`) — is extended with
the new terms rather than replaced.

### 11.3 The screensaver, and the two things the desktop must be told

A kiosk that a screen blanker turns off after ten minutes is not one, and baz
already has the machinery: **it speaks D-Bus** for MPRIS2 over `zbus`, on its own
session-bus thread, with the exact posture this needs — *"with no D-Bus session
bus, baz prints one line and runs exactly as before"* (`README.md:127–137`).

- **Inhibit the screensaver while the kiosk is full-screen and music is
  playing**, via `org.freedesktop.ScreenSaver.Inhibit`, released the moment
  either condition stops being true. **Not while merely paused** — a paused
  kiosk holding a near-black frame is exactly when the display *should* be
  allowed to sleep, which is also §7.6's static case.
- **Failure is silent and total.** No bus, no inhibitor, no message, no
  degradation of anything else — the same failure mode MPRIS already has.

Refused: inhibiting system **sleep** (baz is not entitled to keep a machine
awake), and any inhibition while the surface is not full-screen.

---

## 12. The implementation plan

Ordered so the **highest relief per unit of work lands first**, and so that
every step is shippable on its own — a release could stop after any of them and
the surface would be better than it was, not half-built.

The engine work is deliberately **last**, not because it is least valuable but
because steps 1–6 are all front-end changes to a place that already exists,
while step 8 touches the realtime path and ADR-0009's promise. Front-loading the
riskiest work would mean the visible improvements wait on it.

---

**Step 1 — Delete the duplicate transport.** *(One line. Do this first.)*

Remove `crate::views::bottom_bar::transport(player, ink)` from
`now_playing.rs:168` and its `GAP_XL`. Drop `TRANSPORT_HIT` from `below` in
`art_edge` (`now_playing.rs:61–66`), which grows the artwork by 32 px at every
height-bound size for free.

*Ships*: the owner's stated ask, a defect fixed, and a larger sleeve.
*Test*: `the_place_draws_no_transport` — the place's element tree contains no
transport widget; the bar's is untouched. Existing `art_edge` tests updated for
the new `below`.

---

**Step 2 — The hero decode, and the refusal made true.** *(§5.2)*

`art::load_hero(first_track, HERO_PX = 1024)` beside `load_thumb`, same
resolution order, `image.thumbnail` (downscale-only). A 2-entry LRU on `Shelf`
keyed by album id, filled for the sounding record and its successor. `art_edge`
gains its third term (`hero_px`); **`NOW_PLAYING_MAX` is deleted.**

*Ships*: artwork at its real size — 1000 px at 2560, up from 720 — and
`REFUSALS.md`'s artwork entry becomes true on this surface for the first time
(§0.4 b).
*Test*: `the_now_playing_surface_never_draws_art_larger_than_its_source`, swept
over `hero_px ∈ {120, 320, 500, 1024}` × sides 400–4000, mirroring
`shelf.rs:1509–1530`.
*Gate*: memory — the 2-entry hero cache must not exceed 8 MiB.

---

**Step 3 — The field, static.** *(§5.3, no motion yet)*

Palette extraction from the decoded hero (three clamped colours), composited as
an ordinary gradient behind the place's body. **No shader, no clock, no
toggle yet** — this is the still state, which every backend draws and which
§7.5 needs as the fallback anyway.

*Ships*: the single largest visual change in this document, at zero per-frame
cost, on every renderer.
*Test*: extraction is deterministic for a given cover; the composite never
exceeds L 0.22; a cover with no chroma yields the room rather than a grey wash.

---

**Step 4 — The kiosk type scale.** *(§11.2)*

`kiosk_scale(edge)` and its application to the placard, the feed and the needle.

*Ships*: 4K becomes a kiosk rather than a postage stamp.
*Test*: `the_type_scale_is_identity_below_720` — every size token at
`edge ≤ 720` is pixel-identical to today, so the desktop case cannot regress;
and monotonicity plus the 2.5 ceiling across 400–4000.

---

**Step 5 — The feed.** *(§8)*

F1–F11 as a `Fact` enum with one formatter each, the fixed cycle (§8.2), the
20 s dwell under §7.3's guard, the press-to-advance, and the
**`Event::PlayRecorded` subscription** so the count is current — the one piece of
new plumbing, since `PlayRecorded` has no consumer in `crates/baz` today.

*Ships*: the ledger's permitted card gets the home it lost when ADR-0022 deleted
the inspector (§1), and `signal_path()` stops being dead code.
*Test*: a record with no history yields F3 and never a dash; the cycle contains
only facts the record has; **no formatter emits a total, an aggregate over
history, or a second-person congratulation** (§8.4's tone rule, as a test over
the string table).

---

**Step 6 — The toggles.** *(§7.2)*

T1/T2/T3 as three booleans, the `Ambient` word-door and its menu, the Settings
rows, persistence. T2 and T1-drift are wired to nothing yet; this step is the
control surface arriving before the things it controls.

*Ships*: nothing visible on its own — this is the step that makes 7 and 8 safe.
*Test*: **`the_ambient_clock_is_absent_outside_its_place`** (§7.3), asserted over
every place × every toggle combination. This test is what makes steps 7 and 8
unable to regress the product's idle.

---

**Step 7 — The field drifts.** *(§7.5, and the first measurement gate)*

The `shader` widget (no manifest change — `wgpu` is already default, §0.3(7)),
a time uniform, the `window::frames()` arm under §7.3's guard, and the
**`unavailable` fallback to step 3's static field** when the backend is
tiny-skia.

*Ships*: the surface becomes ambient.
***Gate — this step does not merge until §7.4's four thresholds pass on the real
GPU***, including at 3840 × 2160. The harness is doc 04 §1.3's, extended with an
`ambient` driver and GPU instrumentation. Per doc 04's own precedent this puts a
window on the maintainer's display for a few minutes; **that intrusion is the
deliverable** and must not be substituted with a software-rasterised number.

---

**Step 8 — The meter.** *(§9 — the only step that touches the realtime path)*

In `baz-core`: `pub(crate)` on `KWeighting`; a `LiveMeter` with fixed-size
accumulation and no `Vec` growth; `SharedMeter`'s two `AtomicU32`s;
`Command::SetMetering(bool)` swapping the session's `Option<LiveMeter>`; the
`observe(&[f32])` tap on `a`/`b` in `pump`. In `crates/baz`: the instrument
register, the field's response, and the three ballistics.

*Ships*: the last piece of the brief.
*Tests, and they are the point*:
- **Bit-exactness**: the existing ADR-0009 suite must pass **unmodified**. The
  tap cannot mutate — `&[f32]` — so this is a regression guard, not the proof.
- **Compliance vectors** per §9.1: VU reaches 99 % in 300 ms ± tolerance with
  1–1.5 % overshoot; PPM falls 24 dB in 2.8 s; momentary agrees with
  `loudness.rs`'s integrated figure on a steady tone. **The published constants
  are read from the standards at implementation time, not from this document.**
- **No allocation on the pump path**, asserted the way baz already asserts
  realtime contracts in `device.rs` and `volume.rs:432–437`.
- **Zero when off**: with `SetMetering(false)`, the session holds no
  `LiveMeter`.
*Gate*: §7.4's thresholds re-run with the meter live.

---

## 13. Deferred, and re-verification — ranked

Everything this study declined, in the order it should be picked up.

**Deferred work:**

| | Item | Why deferred | What would trigger it |
|---|---|---|---|
| **D1** | **The visualizer** (§10) | The marginal work is an FFT and a sample ring; the marginal *risk* is looking like every other player. The interesting question — *what would baz's own be* — is unanswered and should not be answered in a hurry | Step 8 lands the tap it needs. Then it wants its own study, not a section |
| **D2** | **Network enrichment** (§8.6) | Blocked on an identifier baz does not store: every good source is MBID-keyed and baz holds no MBIDs. That is a scan-and-schema change, not a UI one | The local feed shipping and proving the composition has no hole in it |
| **D3** | **Embedded lyrics** | A new scan capability (`lofty` can read the frames); and at 3 m a scrolling lyric column is the wrong object for the far field | A demand this study did not find |
| **D4** | **A first-seen column in the index** | Would make *"added to your collection in 2019"* true. Today only `mtime` exists and a re-tag rewrites it, so the fact would be a plausible-looking lie (§8.1) | A schema change for another reason, which this would ride along with |
| **D5** | **A second window on a named monitor** (§11.1) | Two blockers, not one: iced exposes no monitor handle, **and** global control flow would couple the kiosk's clock to the main window's idle | iced exposing `MonitorHandle` **and** an answer to the idle coupling. Both, not either |
| **D6** | **A periodic pixel nudge for burn-in** (§7.6) | The drift already moves the large areas; nudging type would make it shimmer | Real-world evidence of ghosting on a drifting field. Priced at one frame per 60 s so re-proposing costs an observation, not an argument |

**Re-verification — claims in this document that are weaker than the rest:**

| | Claim | Current standing | Before it is leaned on |
|---|---|---|---|
| **R1** | The VU and PPM ballistic constants (§9.1) | Stated from the standards by name; **not read from the published documents in this session** | Read IEC 60268-17 and IEC 60268-10 directly at step 8, exactly as ADR-0015 asserted its coefficients against BS.1770-4's own tables |
| **R2** | Roon's Display mode and Plexamp's screensaver (§2) | **Not independently verified** — doc 03 recorded that Plexamp's UI page 301s and its layout *"was not seen"* | Direct examination before any composition decision cites them. Nothing in §5–§9 currently rests on either |
| **R3** | The GPU cost estimates (§7.4) | **Labelled estimates**, from the shape of the work | Step 7's gate, which is the measurement itself |

---

## 14. The ledger entries, rewritten

Per `REFUSALS.md:6–14`, an entry the owner reverses *"gets rewritten to say what
was decided and why, and that is the whole of the process."* These are drafted
in the ledger's own voice, in the form it already uses for the hover veil
(`REFUSALS.md:113–121`) and the wall's scrollbar (`REFUSALS.md:235–243`) — the
old text quoted, the decision recorded, and **the constraint that replaced the
blanket refusal** named along with the test that holds it.

### 14.1 Artwork and its source

> **Artwork is never *upscaled*: no sleeve is drawn larger than the pixels its
> file actually contains.** The wall holds this at `ART_MAX == THUMB_PX`
> (`the_wall_never_draws_art_larger_than_its_source`); the now-playing surface
> holds it against its own decode, whose size it reads rather than assumes
> (`the_now_playing_surface_never_draws_art_larger_than_its_source`).
>
> **A field derived from a cover is not the cover.** The now-playing surface
> draws a wash built from three colours sampled out of the artwork, with
> lightness and chroma clamped into the room's range — the same rule the
> art-derived lamp already follows, at a larger size. It is not invertible, it
> carries no resolution, and *larger than its source* is not a predicate that
> applies to it. What it may not do is exceed L 0.22, because the sleeve must
> stay the brightest object on the screen.
>
> *Rewritten on the owner's decision, 2026-08-09* — *"it would be nice if the
> album art was somehow more prominent, like it takes up the background"*. This
> entry used to read **"No artwork is ever drawn larger than its source.
> `ART_MAX == THUMB_PX`, asserted in code"**, and the equation had come to stand
> for the rule: the now-playing place shipped drawing a 320 px thumbnail at
> 720 px, so the entry was **already false** in the one place nobody had
> checked. What was always meant is the first paragraph; the second records what
> the owner asked for and the constraint that keeps it honest.

### 14.2 The scrim

> **No scrim, ever.** Dimming ten thousand covers to show twelve rows is the
> exact mistake the palette exists to avoid. A scrim is a surface laid over *the
> collection* to make something else readable.
>
> Unchanged by the hover veil, and unchanged by the now-playing field, for the
> same reason in both cases: the veil is a mark on **one** object under the
> pointer that stops before that object is hidden, and the field sits **under**
> everything and dims nothing — it is the room's own colour, changed by the
> record. Neither is a layer over the collection.
>
> *Extended on the owner's decision, 2026-08-09.* The entry is not weakened; the
> distinction it already drew for the veil is drawn once more.

### 14.3 Skeuomorphism, and the meter

> The record supplies **physics, structure and vocabulary** — the stack, sides,
> groove spacing, "drop the needle". It never supplies **surface**. Banned:
> vinyl discs peeking from sleeves, wood grain, tonearms, wear, patina, and any
> circle pretending to be a record.
>
> **A meter is admitted as a measurement and refused as an object.** Permitted:
> the reading, drawn in baz's own vocabulary — a line, a mark, a numeral, the
> room's inks — with its standard named and its ballistics tested against the
> published document (EBU R128 / ITU-R BS.1770-4 momentary by default;
> IEC 60268-17 VU and IEC 60268-10 PPM offered). Refused, and these are the
> surface the entry has always been about: a beige panel, a glass face, a
> printed arc scale, a pivoting needle, a bezel, a lamp behind the dial.
>
> *Rewritten on the owner's decision, 2026-08-09* — *"some nice VU meter stuff
> over it in a stylised way"*. This entry used to name **VU meters** in the ban
> list, between wood grain and wear. What was being banned was the *instrument
> as furniture*, which is what every other item in that list is; a real
> measurement of the audio is data, and this ledger already distinguishes the
> two when it calls the art-derived lamp *data* rather than decoration. The
> entry's own sentence draws the line: *physics, structure and vocabulary —
> never surface.*

### 14.4 Motion, and the ambient class

> Appended to the motion entry, after the fisheye paragraph:
>
> **One surface is allowed to be ambient, because being looked at is its
> purpose.** `Place::NowPlaying` may run continuous, user-started ambient
> content — a field that drifts, a meter that moves — under ADR-0020 §7's
> discipline: it is a thing you start and never a thing that starts itself; it
> exists only on a surface whose purpose is to be looked at, and a second such
> surface needs an argument that beats this one; it **states nothing**, so it
> can never be the only carrier of a fact; and it costs **exactly nothing** when
> its toggles are off or its place is not on screen, because the subscription is
> a function of state and an absent arm has no timer to stop
> (`the_ambient_clock_is_absent_outside_its_place`).
>
> The rest of the product's idle is unchanged and still measured at 0.0 %. The
> clause everything else hangs on — *anything requiring a redraw while the
> window is idle* — still forbids exactly what it was written against:
> decorative motion **nobody asked for**, anywhere else in baz.
>
> *Amended on the owner's decision, 2026-08-09* — *"ambient motion is fine as
> long as the performance remains top tier."* The bar this entry sets on that
> one surface is therefore **performance rather than abstinence**, and it is a
> number rather than an adjective: `docs/design/12-now-playing-and-kiosk.md`
> §7.4 carries the thresholds and the method, and no ambient work merges without
> them.
