# 12 — The now-playing screen: a surface for the far field

> **Implementation amendment (2026-08-11).** The owner removed the VU view
> after seeing it and separated the remaining visual choices into two axes:
> *"make the spectrum take up the background… a toggle like shuffle… and the
> other one is a 'radio' button essentially, allowing the art or the cd as two
> alternatives in the foreground"*. The shipped control is therefore two
> mutually exclusive foreground choices (**Cover / Jewel Case**) and one
> independent **Spectrum** toggle. The spectrum fills the Now Playing body
> behind either foreground; it no longer replaces the record object. The audio
> tap, sample snapshot and redraw clock are all absent while that toggle is off
> (except the jewel case's own rotation clock), and the VU mode and glyph are
> removed rather than retained as a hidden option. This owner decision
> supersedes the meter-specific recommendations below wherever they conflict.

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
> And then, on what the meter should actually be: **"is it a spectrum analyzer
> or graphic thing with the bars going up and down… that would be nice"** — which
> promotes the visualizer from *"at some point"* to now, and makes the bars this
> surface's primary visual (§10).
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
>    two copies, one screen. §6. **The widget half of this shipped**
>    (2026-08-10: `now_playing.rs:178–189` now carries the argument where the
>    call stood) — but the 32 px it reserved is **still summed into `art_edge`'s
>    `below`** (`now_playing.rs:62–67`), so the sleeve is still 32 px short at
>    every height-bound size. §12.0 says where that goes.
> 2. **The artwork becomes the field, and the refusal it argues with is already
>    broken.** `NOW_PLAYING_MAX` is 720 px (`now_playing.rs:81`) and the cache
>    it draws from is 320 px (`art.rs:48`) — the shipped surface upscales
>    2.25× at 1920×1080 today, which is *no artwork is ever drawn larger than
>    its source* being false in the one place the ledger never checked. §5
>    resolves it with a **derived ambient field**: not the artwork, a *reading*
>    of it, in the same sense the ledger already calls an art-derived lamp
>    **data**. The refusal is rewritten to say what it always meant.
> 3. **Two instruments, one tap, and they cannot disagree.** The **spectrum
>    analyser** is the primary visual (§10): a 2048-point real FFT over 24–64
>    geometric bands, drawn as light inside the field's own shader. It costs
>    **zero new crates** — `realfft` is already in `Cargo.lock`, because
>    `rubato`, baz's resampler, depends on it. Beside it, the **momentary-loudness
>    meter to EBU R128 / ITU-R BS.1770-4** survives as the *reading* (§9), because
>    baz already owns that filter, derived and vector-tested (ADR-0015 §1). Both
>    read the same samples at the same instant, pre-gain; §10.7 is the guarantee
>    and its tests. The tap is **read-only by type** — `&[f32]`, never `&mut` —
>    so bit-exactness is not promised, it is unspoiled by construction.
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
> 7. **The queue is not a second place. It is this surface's other half.**
>    (2026-08-10, the owner: *"the queue and the now playing need integrated in
>    some way so we can remove the queue option from the bottom bar"*.)
>    `Place::Queue` is deleted and its every affordance moves here, into a
>    **run column** that fits inside the margin this surface is already leaving
>    empty — measured, at
>    [`impl/queue-in-now-playing/`](impl/queue-in-now-playing/README.md).
>    §3.4 is the argument, §5.5a is the drawing, §6.4 is what the bar does
>    afterwards.
> 8. **What is playing is *which list, and which track in it*.**
>    (2026-08-10, the same conversation: *"probably the basic model is that
>    every album has a playlist implicitly… it should be basically which
>    playlist and which track"*.) That is the model the merged surface reads
>    from, and it is recorded on its own as
>    [ADR-0034](../adr/0034-the-run-and-its-list.md), because it reaches the
>    protocol and the play ledger and this study does not. §3.5 is what it buys
>    here.
>
> **Citation baseline for the 2026-08-10 revision.** Every `file:line` in §3.4,
> §3.5, §5.5a, §6.4 and §12.0 is read against **`c768035`**, and three other
> branches are editing `app.rs`, `views/lane.rs`, `views/home.rs`,
> `views/top_bar.rs` and `theme.rs` concurrently. Check `git log` before quoting
> a line number from those five: the *claims* are about named functions and
> constants and survive the drift; the numbers may not.

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
> (the product's standing rule)

So the table below is in two halves, and the split is the honest one. Nothing
here is an argument against him; the second half is a record of decisions, and
§14 carries the rewritten entries in the ledger's own voice.

**Still binding, and they shape every answer below:**

| Entry | Where | What it holds to on this surface |
|---|---|---|
| **Every action has a visible, pointer-reachable control; no control's only affordance is hover** | the product's standing rule | No kiosk whose controls appear on mouse-move. This is the mitigation for a toolkit that publishes no accessibility tree, and ADR-0028 re-confirmed it outranks a quietness preference (the product's standing rule). It is what makes §6's *drop the transport* legal — the bar keeps it, visibly, in this place as in every other |
| **No state is signalled by colour alone** | the product's standing rule | A meter that is only a colour change; a fact whose "new" is only a tint |
| **Amber is never an opaque fill**, and states playback truth only | the product's standing rule | The accent on anything that is not *which record, which track, where the playhead is*. §9.6 is why the meter is **not** amber |
| **No snake oil** | the product's standing rule | Any signal-path claim the path cannot demonstrate. This binds the meter hardest of all: a meter that contradicts `bit-perfect` would be exactly the unearned claim this entry exists to stop |
| **No engagement stats. History records; it never performs** | the product's standing rule | Streaks, charts, top-artists, listening-time totals. §8.4 states which side the rotating fact falls on, and why, rather than assuming it |
| **No cloud dependency; internet features individually opt-in** | `README.md:22–24`, `VISION.md` pillar 3 | A screen whose feed is empty without a network. §8 is built local-first for this reason and not merely in deference to it |
| **The bar's ratchet** | the product's standing rule | Removing a bar slot for tidiness. Replacing a slot with *a better statement of the same fact* is the one permitted move — and §6 is the mirror case: the *place* drops what the bar already says |
| **No borders on artwork**; **no shadows** except the playing halo | the product's standing rule | A frame or drop-shadow to separate the cover from the field behind it. §5.4 solves that separation with light, which is the one thing permitted |

**Reversed by the owner, and rewritten in §14** — each of these is the entry
this study would otherwise have had to break:

| Entry as it stood | Where | What the owner decided |
|---|---|---|
| **No artwork is ever drawn larger than its source.** `ART_MAX == THUMB_PX`, asserted in code | the product's standing rule, `art.rs:44–48` | *"the album art was somehow more prominent, like it takes up the background"*. §5 draws the true-size cover **and** a derived field behind it; §14.1 rewrites the entry around the distinction the code already needs. Note this entry is **already false on this surface today** (§0.4) |
| **No scrim, ever** | the product's standing rule | The field is not a scrim — §5.3 makes the argument the ledger's own hover-veil amendment already made once (the product's standing rule), and §14.2 records it |
| **Skeuomorphism: banned — vinyl discs, wood grain, tonearms, VU meters, wear, patina** | the product's standing rule | *"some nice VU meter stuff over it in a stylised way"*. §9 distinguishes the **instrument** (banned surface: beige panel, glass face, swinging needle) from the **measurement** (permitted data), which is the entry's own *"physics, structure and vocabulary… never surface"* applied rather than overridden. §14.3 |
| **No motion that costs anything when nothing is moving**; *anything requiring a redraw while the window is idle* | the product's standing rule, ADR-0020 | *"ambient motion is fine as long as the performance remains top tier"*. ADR-0020 **gains a sixth class** rather than losing its rule: user-started ambient content, on a surface whose whole purpose is to be looked at. §7, and §14.4 |

Two further postures, not refusals but settled, and both survive intact:

- **Places are the whole surface model.** *"The window holds one place at a
  time, with the returns lane to its left in every place but Settings, and the
  now-playing bar under all of them"* (`place.rs:5–7`). Eight members today,
  `NowPlaying` among them — and **seven after §3.4**, because `Queue` is the
  first member this product has deleted for being half of another one. Adding a
  member is cheap and adding a *kind* is not; removing one is cheapest of all,
  and this is the model working rather than bending.
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
(the product's standing rule). ADR-0022 deleted the inspector. The permission
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
(the product's standing rule). A field built from the cover's own palette is that
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
> both**, and the product's preamble says his decision is sufficient on
> its own: home is a real place, and a `Now playing` place stands beside it."*

Three things that decision settles, so this study does not re-open them:

- **The route in is the lane, not a bar door.** The head's three destinations —
  Home, Library, Now playing — are *"a closed set of three"* and *"a fourth is
  the refused thing"* (the product's standing rule). The `Now playing` row carries the
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
| Show or hide the run | this surface → **the place's own body** | the `Run` word, place top-right | §3.4.3 |

So the place's top-right carries **three** controls and no more: one glyph
(expand, tooltipped per the icon-only law) and two words (`Run`, `Ambient`) —
`Run` first, because it decides what the surface *is* and `Ambient` only decides
how it looks. Both words are
visible at rest and pointer-reachable, which the product's standing rule requires; the
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

**`Esc` does not hide the run** (§3.4). The run is not a layer; it is half the
place. A peel that removed it would make `Esc` mean three different things in
one place, and would make the way *back* to it a control the peeling reflex
cannot reach.

### 3.4 The queue is not a second place — it is this surface's other half

> **`Place::Queue` is deleted. `Place::NowPlaying` absorbs it whole.** The
> owner: *"we need to work on the now playing since I think the queue and the
> now playing need integrated in some way so we can remove the queue option
> from the bottom bar"*.

#### 3.4.1 Why they are one object rather than two adjacent ones

The tempting argument is that they are *related*, which is not an argument —
plenty of related things are separate places on purpose, and this document
spent §3.1 explaining why `NowPlaying` is not `Album` despite being about the
same record most of the time.

The real argument is the owner's own model, and it is stronger than adjacency:

> **A run is a list and a cursor. Now playing is the cursor. The queue is the
> list. They are two readings of one object.**

That is [ADR-0034](../adr/0034-the-run-and-its-list.md) §1, and it makes the
present arrangement read as a defect rather than a layout: *a surface about
what is playing that cannot say what it is playing **in**, beside a surface
about the list that does not show what is sounding **from** it.* Each is
missing the half the other is holding. `queue.rs:30–38` already says the queue
place is *"one list with a cursor"*; the cursor is the thing it draws at 6 px,
as a lamp dot in a number column.

**Three independent confirmations that this was already true:**

- **`Resume` navigates here** — the one play gesture in the product that
  navigates (`app.rs:2205–2253`). Its subject is *the interrupted run*, a list
  and a cursor, and it has always landed on a surface showing only the cursor.
  §3.4.5.
- **The bar's own left zone is already the merged reading**: the record, then
  the artist, then `then 23 more · 1:56:18 left`
  (`bottom_bar.rs:479–502`) — cursor and list, in three lines, in the one
  surface that is in every place. The merged place is that zone at the size it
  deserves.
- **The queue place's mitigation for its own cost was the bar.** `queue.rs:23–28`:
  *"knowing what is next used to cost nothing and now costs leaving the shelf.
  The mitigation is that the bar's own third line states the continuation
  ambiently, so this place is for **changing** the queue rather than for reading
  it."* A place whose reading half was delegated elsewhere is a place holding
  one half of a thing.

#### 3.4.2 The shape that was proposed, and where the drawing broke it

The shape offered for testing was **one place, two densities — in the window,
what is playing and what is next, editable; full-screen, the ambient kiosk with
the list stood down.**

**The two densities are right. Binding them to full-screen is wrong**, and the
reason is a toolkit fact this document already established rather than a matter
of taste:

1. **baz cannot tell the two full-screens apart.** The distinction the proposal
   needs is *full-screen on the second monitor* versus *full-screen on the only
   monitor*, and **iced 0.13 has no monitor enumeration in its public API at
   all** (§0.3(2), §11.1, verified against the installed source). A
   single-display listener who presses `F11` would lose the editor, with no way
   back that is not un-full-screening.
2. **It would make `F11` a content act.** §3.2 settled that full-screen is a
   *window* act working in every place, and §3.3 that `Esc` peels it before the
   place. A full-screen that also decided what a place contains is the
   micro-mode §3.2 refuses in as many words: *"leaving the place and leaving
   full-screen the same gesture"*.
3. **It would make the composition a function of the window manager.** A tiling
   user is permanently in one density and never learns the other exists.

So: **the second density is real and it is a stated control** — `Run`, §3.4.3 —
and **full-screen changes nothing about it**. What genuinely does change with
size is arithmetic, not mode: which axis the two columns sit on (§5.5a) and the
type scale (§11.2). That keeps `now_playing.rs:24–31`'s standing claim true —
*"the kiosk is this same surface at a larger size, and that is a property of the
composition rather than a plan"* — for the merged surface as well.

#### 3.4.3 `Run`: one word, remembered — **removed by the owner, 2026-08-10**

> **This section is superseded and kept for the argument it made.** The owner,
> hours after M1 shipped: *"remove the run button from the now playing"*, and,
> asked which control he meant, *"run button is what I'm referring to; just to
> be clear"*. **The run column stands whenever there is a run**; there is no
> density, no word, and no config key. ADR-0029 §8.5 records the decision and
> what it costs step A6 — which is that `Ambient` must now design its own
> control rather than adding a second word to an existing pair.
>
> What survives of the text below is the *other* half of its argument: the
> density was never bound to full-screen, and that is now true because there is
> no mode left to bind. What does not survive is the premise that the surface
> should offer the choice at all — a place whose argument is *a run is a list
> and a cursor* was offering a control that hid the list.

A labelled word-door in the place's top-right, beside `Ambient` (§3.2), visible
at rest, persisted like every other place-level setting. On by default.

**It is where the bar's door went**, and that is the honest description: the
owner asked for the `Queue` control to come off the bar, and the affordance did
not evaporate — it moved into the room it describes. The press count is
unchanged for the ordinary case: today, *bar → `Queue`* is one press to an
editable run; afterwards, *lane → `Now playing`* is one press to an editable
run.

**It is a peer of `Ambient`, not a fifth row inside it.** T1–T4 govern
*ambience*; `Run` governs *what the surface is about*. Folding it into that menu
would put the surface's subject behind a door labelled with its decoration.

#### 3.4.4 The doors in, and the two stale comments this found

**The lane's `Now playing` row is the single way in**, unchanged (§3.1), and it
is now the way in to both halves.

**`Ctrl+U` becomes the accelerator of that row.** `keys.rs:401` binds it to
`Message::ToggleQueue` today; it resolves to `Place::go(Destination::NowPlaying)`
instead, and `Message::ToggleQueue`, `Place::queue()` and `Place::Queue` are all
deleted. Three notes:

- **It stops toggling**, and that is the mirror rule rather than a loss. The
  key is the accelerator of a *destination*, and `place.rs:248–257` settles what
  a destination does: *"pressing the row you are already on must leave you
  there"*, asserted by `a_destination_never_closes_itself`. A key that closed
  what its visible twin does not close would be a second behaviour with no
  control. `Esc` is the way out, and always was.
- **`U` still means what it meant.** `keys.rs:206–223` spends the letter on
  *up next*, and up next is exactly the half of this surface that was the
  queue. The reflex lands on a surface that contains what it always contained.
- ~~**It also turns the run on**~~ — **moot since 2026-08-10.** It was legal by
  ADR-0023's amendment (*"two messages visible controls also send"*), and the
  removal of the `Run` word removed the second message rather than the
  legality: the chord now sends `Message::ShowNowPlaying`, which is what the
  lane's row **and** the bar's now-playing block both send. One message, two
  visible twins, no construction required — a simpler legality than the one it
  replaces.

**`Q` is not bound to the queue and has not been for some time.** The brief for
this revision said it was; the code says otherwise, and the code is right:
`keys.rs:820` asserts `bind(&ch("q"), none())` is `QueryTyped("q")`, because
ADR-0017 §1.2 took every bare letter for the query (`keys.rs:206–213`). **So
there is nothing to remove.** What the search *did* find is two doc comments
that never followed the key:

- `place.rs:158` — *"<kbd>Q</kbd>, and the bar's labelled `Queue` control"*
- `bottom_bar.rs:347` — *"It is the same message <kbd>Q</kbd> sends."*

Both die with the code they document, which is the tidy way for a stale comment
to go.

#### 3.4.5 What `Resume` does, re-checked

`resume_the_run` (`app.rs:2242–2253`) starts the run and navigates here in the
same press. **It reads better after the merge, not worse**, and this is the
clearest single piece of evidence that the two places were one:

> `Resume`'s subject is a **run** — a list, and a position in it. It has always
> navigated to a surface that could draw only the position.

Nothing about the gesture changes: still one press, still the front end's own
act without waiting on `TrackStarted` (`app.rs:2230–2234`), still nothing at all
when the track is gone. What changes is that the destination can finally show
what was resumed. With the run column on, the listener lands looking at the
thing the word named.

**One check made and passed**: landing on an editable list is not alarming,
because every edit affordance on a run row is hover-gated
(`queue.rs:572`, `let offered = live && hovered;`). A `Resume` press that ends
with the pointer parked on the transport reveals nothing.

### 3.5 What the list model buys this surface — four claims, checked

[ADR-0034](../adr/0034-the-run-and-its-list.md) gives every run an `Origin`.
Four things were proposed as following from it; two are real as stated, one is
real in a weaker form, and one is **refused on measurement**.

**① The merged surface heads the run with the list it came from. REAL.**
`queue_summary` prepends the name at exactly one line — `player.rs:2189–2192` —
and it is `None` for every run that is not a playlist. So
`Road Trip · 4 of 12 · 38:12 left` and `Ochre · 2 of 9 · 31:04 left` become the
same sentence with different subjects, at a one-line change. The frame
`impl/queue-in-now-playing/01a-queue-open-1280x860.png` is the sentence with its
subject missing.

**② The lane credits the list for every kind of run. REAL, in two stages.**
Within a session it lands as soon as `Origin` exists in the front end; across a
quit it needs the ledger's run marker (ADR-0034 §4), because the launch fold
re-derives from a per-path ledger (`app.rs:5986–6002`). The finding worth
recording is how *small* the change is: `lane::Subject` is already
`Record(u64) | Playlist(u64)` (`lane.rs:84–90`), and under the model those are
**both list identities** — an album's implicit list *is* the record. The lane was
list-shaped before anyone said so. The one behaviour that changes is that a
shuffle draw credits nothing, where today it credits every record it quoted.

**③ The bar's continuation line says what it is continuing. REFUSED, on
measurement.** The bar's left zone is one of two equal `Length::Fill` flanks
around `TRANSPORT_W` 112 with two `GAP_LG` 16 (`bottom_bar.rs:106–117`), so at a
1280 px window it is `(1280 − 144) / 2` = **568**; the block that holds the three
lines gets 568 less two `STAMP_W` 52, less `UP_NEXT_W` 152, less three `GAP_SM`
= **288**, and the cover takes `BAR_COVER` 52 and a `GAP_MD` 12 off that
(`bottom_bar.rs:440–451`), leaving about **224** for a line that already reads
`then 23 more · 1:56:18 left`. Prefixing `Road Trip · ` costs roughly 70 of
those 224. §6.4 wins 160 px back, which makes it *fit* at 1280 — and it is
still refused, because the fact has a better home: the merged surface's head
states it at `SIZE_META` in a column with room, and printing it in both places
would be the same subtraction twice, which is the exact reasoning
`bottom_bar.rs:328–338` already used to take the position out of the door's
readout.

**④ `Save as playlist` becomes *name the list you are already in*. REAL as a
default, refused as an act.** ADR-0024 §4 is explicit that saving makes *"a new
file and nothing else — the queue does not become linked to the playlist it
seeded"* (`queue.rs:198–201`), and the merged surface does not reopen it: a
control that sometimes made a file and sometimes renamed one would be two acts
under one word. What the model changes is the **prefilled name**: the field
opens holding the run's own list name — `Ochre`, `Road Trip`, `All songs` —
instead of an empty box under `Name tonight's run…`. That delivers the sentence
as a default, costs one argument to `save_field` (`queue.rs:312–338`), and keeps
ADR-0024 §4 whole.

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
  ledger's own permitted form (the product's standing rule), read from `TrackHistory`
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
  (ADR-0009 §5, the product's standing rule).
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
- Given all four toggles are off, when the kiosk is showing and music is
  playing, then the surface draws only on data arriving, and `view()` is not
  called on any clock of its own.
- Given all four toggles are off, when the surface is idle, then process CPU is
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

### S8 — See what is next without leaving what is playing

> As a listener with a long run on, I want to see what is coming while still
> looking at the record I am hearing, so that *what is this* and *what is next*
> are one glance and not two navigations.

**Task flow**: ① `Now playing` in the lane (or `Ctrl+U`). That is all.

**Acceptance criteria**

- Given a run is playing and `Run` is on, when the place renders at a body
  **≥ 784 px** wide, then the record and the run stand side by side, and the
  run's first visible row is the sounding one, without scrolling.
- Given the run holds a five-figure number of entries (`Play all`), when the
  place renders, then only the visible slice is built — `queue_window`'s two
  spacers, unchanged (`queue.rs:130–196`) — and the frame-time thresholds of
  §7.4 hold **with that run on screen**, which is a new condition on an existing
  gate (§12, step M4).
- Given the run is showing, when the pointer is not on a row, then **no edit
  control is drawn** — the ▲▼ ✕ + slots are reserved and empty
  (`queue.rs:572`, `queue.rs:633`), so the surface at rest is a reading.
- Given `Run` is turned off, when the place renders, then the record occupies
  the whole body and the composition is **pixel-identical to the unmerged
  surface at that size**.

### S9 — Edit the run from the surface that is showing it

> As a listener, I want every gesture the queue place had, in the place that
> replaced it, so that a merge is not a quiet subtraction.

**Task flow**: ① be in `Now playing`; ② hover a row; ③ act.

**Acceptance criteria**

- Given a run row is hovered and an engine is present, when the pointer is on
  it, then ▲, ▼, ✕ and `+` appear in their reserved slots and **no duration
  moves sideways** — `queue.rs:449–458`'s two fixed-slot rules, inherited whole.
- Given a row is pressed below the drag threshold, when it is released, then
  playback jumps to it (`Message::JumpToQueued`, ADR-0014's `JumpTo`); given it
  is pressed and moved past the threshold, then it lifts and reorders against
  the insertion line.
- Given an edit is made, when it lands, then `Undo` stands in the run's summary
  strip and `Ctrl+Z` is legal; given the place is left, **or `Run` is turned
  off**, then the history is cleared, because an accelerator whose visible twin
  is off screen is not legal (§6.4, `queue.rs:210–213`).
- Given the run came from a playlist, when `Save as playlist` is pressed, then
  the name field opens **prefilled with that list's name** and still writes a
  new file (§3.5 ④, ADR-0024 §4).

### S10 — Know which list you are in

> As a listener, I want the surface to tell me which list I am playing and where
> I am in it, whether that list is a playlist, a record, or everything I own.

**Acceptance criteria**

- Given a run reified from any list kind, when the merged surface renders, then
  its head reads `{list} · {n} of {N} · {t} left` — the same sentence for
  `Road Trip`, for `Ochre` and for `All songs`
  ([ADR-0034](../adr/0034-the-run-and-its-list.md) §1).
- Given a run assembled by hand, when the head renders, then it names no list
  and says so by omission — never a placeholder, never `Unknown`.
- Given the run came from a list with **no file**, when the transfer picker is
  opened, then `Add to "{name}"` is **not offered** for it — ADR-0034 §1.3's
  destination bit, which is the behaviour that ships today, preserved by
  construction rather than by memory.
- Given baz is quit and relaunched, when the lane is folded from the ledger,
  then a run played from a list credits **the list** and not the records it
  quoted (ADR-0034 §4, closing `docs/BACKLOG.md:9–25`).

### S11 — Leave it running, with the list on it or not

> As a listener at a kiosk, I want to decide once whether the run is on the
> screen, and never be asked again by the window manager.

**Acceptance criteria**

- Given the place is full-screen, when `Run` is on, then the run is on screen;
  when `Run` is off, it is not. **`F11` changes neither** (§3.4.2).
- Given `Run` was turned off, when baz is relaunched, then it is still off.
- Given the kiosk is at 3840 × 2160 with the run on, when it renders, then the
  run's type is scaled by `kiosk_scale` like every other measure (§11.2) and
  its column is `RUN_MEASURE · kiosk_scale` wide.
- Given the run is on over a drifting field, when a row is read, then **nothing
  under the run column moves and nothing under it is lighter than
  `room.wall`** (§5.4) — so every contrast pairing on a run row is the pairing
  that ships today.

### S12 — The bar, after its door goes

> As a listener anywhere in the product, I want the bar to keep telling me what
> is coming, now that it no longer has a door to what is coming.

**Acceptance criteria**

- Given anything is playing, when the bar renders, then the continuation line
  still reads `then {…} · {…} left` (`bottom_bar.rs:563–599`) — the door goes,
  the ambient reading stays (§6.4).
- Given the `Queue` control is gone, when the bar renders at any width, then
  **the transport column has not moved a pixel** and the now-playing block is
  160 px wider (§6.4's measurement).
- Given a track title that clipped before, when it renders after the change,
  then it clips 160 px later — which is the whole of what the removed slot was
  spent on.

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
           height − 2·HANG − below)
       .max(ART_MIN)
       .min(hero_px)                   ← the source's own pixels, and it wins
```

> **Corrected 2026-08-10, when A2 shipped.** This block ended `.max(ART_MIN)`
> until then, and at `hero_px` 120 that expression is **240** — which fails the
> test printed four paragraphs below it, and draws a 120 px cover at 2×. The
> order above is the shipped one and the reasoning is one sentence:
> `ART_MIN` is a **design floor** saying a work this small has stopped being a
> subject, and `hero_px` is a **fact** saying there are no more pixels. *A fact
> outranks a floor.* Story S7 asks for the small cover *"drawn at its own pixel
> size, centred, never scaled up"*, and the field is what makes that composed
> rather than broken.

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
   preference"* (the product's standing rule). The field is the lamp's own rule, with
   three colours instead of one and a large area instead of a small one. If
   hue-read-from-the-record is data at 6 px, it does not become decoration at
   1920.
4. **Amberol ships the honest version** — *"the whole window washed with a
   3-gradient composite from the palette"* (`03:236`) — and it is the one
   treatment in doc 03's table that draws no copy of the art at all.

**Why it is not a scrim.** The ledger's objection is specific and it is worth
quoting: *"a scrim is a surface laid over **the collection** to make something
else readable"* (the product's standing rule). The field is under everything, laid over
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
z3   the placard, the meter's register, the feed,
     the run's head and the run's rows                        (type)
z2   the work — true size, its halo                           (artwork)
z1.5 the spectrum — full bleed, masked under the type box     (light)
z1   the field — full bleed, derived, ambient;
     under the run column: clamped and still                  (light)
z0   the room, #0C0D0E                                        (ground)
     ─────────────────────────────────────────────────────────
     the bar, outside the place entirely (app.rs:3879–3885)
```

z1 and z1.5 are **one shader**, not two layers: the field and the bars are
evaluated in the same fragment pass (§10.8), which is why the bars cost a
uniform upload rather than N quads.

**The mask now has three terms, and all three are shader arithmetic** — no
layout, no second pass, no scrim. §10.8 already established the first for a
single centred column; the merged surface widens its domain to the **type
box**, meaning the placard column ∪ the run column:

1. **The spectrum's opacity is zero over the type box**, softly, exactly as
   §10.8 specifies. Widening the domain costs nothing: it is the same uniform
   with two edges instead of one.
2. **Under the run column the field is clamped to `room.wall`'s own
   lightness.** This is the answer to *is a spectrum behind an editable list
   beautiful or unreadable* — it is neither, because behind the list there is
   no spectrum and the field is no lighter than the ground every other list in
   this product is read over. The virtue of stating it that way is that it
   introduces **no new number and no new contrast claim**: every pairing on a
   run row is the pairing that ships today, and the test is one line —
   *the run column's ground is never lighter than `room.wall`*.
3. **Under the run column the field does not drift.** Clamping lightness is not
   enough on its own: a hue drift under a scrolling list is still motion behind
   type. The ambient owns the rest of the surface and stops at the run's edge.

> **The field is one object with one ceiling function, and the ceiling is lower
> where type is.** That is not a scrim (§5.3, §14.2) — a scrim is a *second
> object* interposed between two others, and this is the same object's own
> value, reduced.

> **Nothing is drawn on the sleeve. Everything ambient is drawn on the field.**

That sentence is what reconciles the owner's *"VU meter stuff over it"* with
the product's standing rule's *"anything on artwork anywhere but a wall tile — not the
Songs rows, not the lane, not the record's page"*. What *"takes up the
background"* is the field; the meter is over **the field**. The sleeve is the
one object on this screen with nothing on top of it, and it stays that way. The
owner's brief and the entry ask for the same composition once the field and the
work are understood as two objects.

The work keeps its halo (`theme::lamp_glow`, the product's standing rule — *"no shadows
except the playing halo, which is not elevation, it is light"*), and that halo
is now doing real work: it is what separates a sleeve from a field of a similar
colour, which is the job a border would otherwise be reached for and which
the product's standing rule forbids in as many words.

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

> **Measured 2026-08-10 (A2).** The shipped figure is **146** — this table's
> six objects, every gap the column actually lays out between them, and the
> needle's own tick — and 190 is that plus the meter's 24 and the feed's 20,
> neither of which is built. The sum above reaches the same 190 by a different
> route and was 16 px optimistic about the base; §5.5a's note carries the
> arithmetic.

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

### 5.5a The merged composition, measured — the record and the run

§5.5's table is the surface **without** the run. This section is the same
arithmetic with the run column in it, and it is the argument for §3.4: the
merge is close to free, and at the size the brief describes it is exactly free.

#### The finding that decides it: the margin is already there

From the frames, measured off the rendered pixels rather than computed
(`impl/queue-in-now-playing/README.md`):

| Frame | Body | Work, drawn | Left | Right | Unused width |
|---|---|---|---|---|---|
| 1280 × 860, lane open 280 | **1000** | **536** | 232 | 232 | **464** |
| 1920 × 1080, lane collapsed 96 | **1824** | **720** | 552 | 552 | **1104** |

The work is centred, so today that width is spent on two symmetric voids. The
run column asks for `RUN_MEASURE` **440** plus one `GAP_XL` **24** = **464** —
**the 1280 slack to the pixel, and 42 % of the 1920 slack.** At 1920 the work is
not even width-bound: it is 720 because `NOW_PLAYING_MAX` clamps it
(`now_playing.rs:82`), which is the defect §5.2 deletes, and even at the
post-§5.2 figure of 729 there are 1095 px of body it cannot use.

#### The measures, derived

```
RUN_MEASURE = LIST_MEASURE / 2 = 440
```

Derived rather than chosen: `LIST_MEASURE` **880** (`theme.rs:3191`) is the
measure this product gives a list that owns its surface, and the run owns half
of one. It clears the anatomy's own floor with slack to spare —

```
TRACK_NO_W 24 + GAP_SM 8 + title + GAP_SM 8 + DURATION_W 48
             + GAP_XS 4 + 4 × STEPPER_HIT 96 + 3 × GAP_XS 12
             + SCROLLBAR_LANE 10                     = 210 + title
```

— leaving **230 px of title lane** at `SIZE_BODY` 13, against the 224 px the
bar's own block gets for the same three-line reading (§3.5 ③). A run row is not
a cramped row; it is the row the bar has always drawn, given more room.

```
run_w    = RUN_MEASURE · kiosk_scale(by_height)      (0 when Run is off,
                                                      or body_w < SPLIT_FLOOR)
record_w = body_w − 2·HANG − (run_w + GAP_XL)
edge     = min(record_w, by_height, hero_px)
by_height = body_h − 2·HANG − below                  (below = 190, §5.5)
SPLIT_FLOOR = ART_MIN 240 + 2·HANG 80 + 440 + GAP_XL 24 = 784
```

**The circularity is broken in one term, deliberately.** `kiosk_scale` is keyed
to `by_height` — the height-bound candidate — rather than to `edge`, because
`by_height` does not depend on `run_w`. Keying it to `edge` would make the run's
width depend on the record's width which depends on the run's width, and the
fixed point would have to be iterated or fudged. One honest substitution
instead, stated here so nobody later "fixes" it.

#### The table

| Window | Lane | Body | `run_w` | `record_w` | `by_height` | `edge` | vs. §5.5 |
|---|---|---|---|---|---|---|---|
| 1280 × 860 | open 280 | 1000 × 779 | 440 | 456 | 509 | **456** (width) | −53 |
| 1280 × 860 | collapsed 96 | 1184 × 779 | 440 | 640 | 509 | **509** (height) | **0** |
| 1280 × 800 | open 280 | 1000 × 719 | 440 | 456 | 449 | **449** (height) | **0** |
| 1920 × 1080 | open 280 | 1640 × 999 | 440 | 1096 | 729 | **729** (height) | **0** |
| 1920 × 1080 | collapsed 96 | 1824 × 999 | 440 | 1280 | 729 | **729** (height) | **0** |
| 3840 × 2160 | collapsed 96, 3000 px source | 3744 × 2079 | 1100 | 2540 | 1809 | **1809** (height) | **0** |

> **Measured, 2026-08-10, after M1 shipped**
> ([`impl/queue-merged/`](impl/queue-merged/README.md)). Two corrections to the
> table above, neither of which touches its argument:
>
> 1. **`below` is 130 in the shipped build, not 190.** The 190 includes the
>    meter's 24, the feed's 20 and one `GAP_LG` — steps A9 and A5, neither
>    built, and neither may reserve height before it exists. So every
>    `by_height` here is 60 px larger today and will shrink to the printed
>    figure as those land. The 1280-lane-open row is unaffected because it is
>    **width**-bound: `edge` is 456 there at either value, and the frame reads
>    456 to the pixel.
>
>    **Re-corrected 2026-08-10, when A2 shipped: it is 146, not 130.** The 130
>    was 16 px short of what the placard column lays out — `.spacing(GAP_XS)`
>    applies between all six of its children, and the needle draws
>    `NEEDLE_H + GAP_XS` tall. `NOW_PLAYING_MAX` 720 was hiding the shortfall by
>    leaving slack; A2 spends that slack and the two timestamps go off the
>    bottom. So every `by_height` here is **44** px larger than printed today,
>    not 60, and §5.5's future figure is **146 + 24 + 20 = 190** — the same
>    number, from a base that is now correct rather than by coincidence.
> 2. **`run_w` is `RUN_MEASURE` flat**, not `RUN_MEASURE · kiosk_scale`, until
>    A4 builds `kiosk_scale`. The 3840 row's 1100 is that step's, not this one's.
>
> The properties the table exists for survive both: the record is height-bound
> above the tightest window, the run takes width the record structurally cannot
> use, and the one row where the cost is real is the one below.

**The run costs the record nothing in five of six rows**, and the one exception
is 53 px at 1280 with the lane expanded — where the record is width-bound
because 1000 px of body is the tightest case this product has. That case has a
remedy already on screen and already keyed: `Ctrl+B` collapses the lane and the
record returns to 509 (`theme.rs:1109–1115`, `sidebar_w`). Recorded as a cost
paid rather than a cost hidden.

**Why the cost vanishes everywhere else**: above 1280 the record is bound by
**height**, not width, because `below` is 190 and a 16:9 body is short before it
is narrow. The run takes width the record structurally cannot use. That is not
luck — it is the same property that made §5.5 report 27 % of a 4K body going to
the work.

#### 1920 × 1080, lane collapsed, `Run` on — the case the brief describes

```
 ┌──────────────────────────────────────────────────────────────────────────┐
 │▓▓▓▓▓ field: derived wash, drifting, L ≤ 0.22 ▓▓▓│ clamped to wall, still │
 │▓ 96▓                                            │                        │
 │▓lane                ┌──────────────┐            │ Ochre · 2 of 24        │
 │▓                    │              │            │   · 1:52:56 left       │
 │▓Home                │   the work   │            │  Undo   Save as playl. │
 │▓Libr                │   729 × 729  │            │                        │
 │▓Now ●               │              │            │ Ochre                  │
 │▓                    │              │            │ Anne-Marie Puig        │
 │▓                    └──────────────┘            │  1  Undertow 1    3:23 │
 │▓                    A N N E - M A R I E  P U I G│  ●  Marginalia 2  6:27 │
 │▓                    Marginalia 2                │  3  Sixth Street  2:14 │
 │▓                    Ochre · 1988                │  4  Blue Hour 4   5:12 │
 │▓                    ├──────────────┤            │  5  Ledger 5      8:28 │
 │▓                    4:24            6:27        │  6  Attic Tape 6  4:01 │
 │▓                    ▁▂▃▅▃▂▁  −14.2 LUFS         │  7  Ferrous 7     7:04 │
 │▓                    Played 34 times since 2019  │  8  Quiet Part 8  2:48 │
 │▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓│  9  Slow Return   5:49 │
 ├──────────────────────────────────────────────────────────────────────────┤
 │ the bar — 81 px, in every place, **minus its `Queue` door** (§6.4)        │
 └──────────────────────────────────────────────────────────────────────────┘
   │←────────── record column, 1280 ──────────→│24│←── run, 440 ──→│
```

Three things the drawing settles:

- **The record column is left-hung, not centred.** With the run taking the right
  edge, centring the work in what remains would leave the placard's left
  alignment pointing at nothing. The work and its placard share a left edge with
  each other and hang from the body's own `HANG`, which is the rule
  `now_playing.rs:123–145` already follows — applied to a column instead of to
  the body.
- **The run's head is in the run's column**, so it costs the record no height.
  `below` stays 190 and every figure in §5.5's table survives untouched. That is
  why the `vs. §5.5` column reads zero rather than "about zero".
- **The ambient stops at the run's edge**, per §5.4's three mask terms. The
  spectrum is full-bleed *behind the record* and absent behind the list.

#### As the window narrows

| Body width | What happens |
|---|---|
| ≥ 784 (`SPLIT_FLOOR`) | Two columns, as above |
| < 784 | **The run wins, and the record becomes its head** |

Below `SPLIT_FLOOR` the record cannot be *the size it deserves* in any case —
`ART_MIN` 240 in a 704 px column is a thumbnail, not a subject — and what is
left worth doing at that width is the run. So the columns re-stack into one:
the record drawn at `ART_MIN` as the run's head block, cover left with the
artist, title and album beside it, the needle under them at the head's width,
and the run's rows below in the same single scroll. **One composition
degrading, not a second layout** — the same four objects, re-hung.

**Two consequences stated rather than discovered:**

- The head block scrolls away with the list. That is correct here and would be
  wrong above `SPLIT_FLOOR`: at this width the surface has become the editor,
  and an editor whose first 300 px are a fixed hero is an editor you scroll
  past to use.
- `SPLIT_FLOOR` bites at a **1064 px window with the lane open**, or an
  **880 px window with it collapsed**. Both are below the 1280 the composition
  audits are taken at, so this regime is a real one and not a theoretical one.

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
| The **run** at `RUN_MEASURE` | The bar states the run as one clipped line and a 2 px needle. This is the list, editable | **Yes** — every gesture `Place::Queue` had (§6.4) |

Two of these seven exist today, two are being enlarged, and three are new. That
is the honest shape of the work, and §12 orders it.

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
appends `bottom_bar::view` under *every* place. The GUI always includes device
output, and an unavailable device is represented as state in that same bar;
there is no alternate silent composition and no per-place branch.
`Place::NowPlaying` is not special-cased and does not need to be.

**This is the same reasoning that just removed `‹ Library` from the place
headers** (`9a7e9a5`, *"The place headers lose their way back, because the lane
is one"*): a place that repeats what a resident surface already carries is
making the same statement twice, and the second copy is the one that goes. The
lane made the header's back-link redundant; the bar makes the place's transport
redundant. One precedent, applied twice, four commits apart.

**What it costs: nothing, and this is checkable.** Every function of the deleted
control survives at full size, visibly, one surface down — play/pause, previous,
next, the needle, the fader, the doors. The accessibility refusal
(the product's standing rule) is satisfied by *a* visible pointer-reachable control,
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
| **Hover-revealed transport** | Refused outright: *"no control's only affordance is hover"* (the product's standing rule). ADR-0028 has just re-confirmed that this entry outranks a quietness preference, and it is the mitigation for a toolkit with no accessibility tree — which is precisely the wrong thing to trade for a tidier picture. Doc 10 §6.3 already lists it as refused rather than merely rejected |
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
the whole of what the product's standing rule asks:

- **The needle**, at the work's width (§5.5). Already interactive, already
  hit-tested through the same module that draws it (`player.rs:527–531` — *"the
  line that is drawn and the line that is clicked can never be two different
  lines"*), and already carrying its own hover preview. Nothing is invented; it
  is the same widget given room.
- **The fullscreen glyph** (§3.2), one `TRANSPORT_HIT` 32 box, at rest, in the
  place's top-right corner, tooltipped per the icon-only law.
- **The `Ambient` word-door** (§7.2), beside it, opening the four toggles and
  the ballistics selector.
- **The feed line** (§8.2), which advances on a press — which is what makes the
  rotation a control rather than a performance.

And one thing it deliberately refuses: **the artwork is not a control.** A
click on the sleeve is a gesture with no visible affordance, and the ledger
forbids both halves of that — nothing may be drawn on a sleeve to advertise it
(the product's standing rule), and no action may be gesture-only (the product's standing rule,
doc 09 §5.2's reading). The route to the record's page is the bar's
now-playing block, in this place as in every other.

### 6.4 The bar after its door goes, and the fate of every queue affordance

#### 6.4.1 The door comes off, and the bar does not move a pixel

`queue_button` (`bottom_bar.rs:348–389`) is deleted, with
`Message::ToggleQueue`, `theme::UP_NEXT_W`, `theme::POSITION_W` and
`PlayerState::queue_size_note` (`player.rs:1666–1669`).

**The bar does not get narrower, and it does not re-centre**, and this is a
property of how it is already built rather than a thing to be careful about.
The bar is three zones — `Fill · TRANSPORT_W · Fill` with two `GAP_LG`
(`bottom_bar.rs:106–117`) — composed under the *whole window* rather than
inside the lane's row (`app.rs:3879–3885`). Two equal-weight fills keep the
transport optically centred whatever the left zone holds, so removing a
152 px fixed slot from inside the left zone moves nothing outside it.

What it does instead is measurable:

| Window | Left zone | Now-playing block, today | …after |
|---|---|---|---|
| 1280 | 568 | **288** | **448** (+56 %) |
| 1920 | 888 | **608** | **768** (+26 %) |

(`(W − TRANSPORT_W 112 − 2 · GAP_LG 16) / 2`, less two `STAMP_W` 52 and
`UP_NEXT_W` 152, less `GAP_SM` 8 per gap — `bottom_bar.rs:260–278`.)

**So the removed slot is spent on the title.** A track title that clips today
clips 160 px later. That is not offered as compensation for a loss; it is what
the width was doing before.

#### 6.4.2 The ratchet, honoured or paid — stated either way

The bar's standing rule is that no slot is removed for tidiness and that a slot
may be replaced by *a better statement of the same fact*. Two halves, answered
separately rather than blurred:

- **The door's *readout* — the queue's size — is replaced.** The merged
  surface's head states it as `2 of 24`, which is the same fact with the cursor
  in it. This is exactly the move `bottom_bar.rs:328–338` already made once,
  when the readout stopped being a position and became a size.
- **The door's *route* is removed, and the owner asked for it.** The ledger's
  preamble settles the process — it binds contributors and agents, not him.
  What is traded is recorded rather than smoothed over: reaching an editable run
  from the wall was one press to a door in the bar and is now one press to a
  destination in the lane. **The press count is unchanged**; the muscle memory
  is not.

#### 6.4.3 Does the continuation line still earn its place? **Yes — more than before**

`continuation_lane` (`bottom_bar.rs:563–599`) stays, unchanged, and its
justification gets *stronger* rather than weaker. It was argued for as the
ambient half of a pair: *"knowing costs nothing; opening is for changing"*
(`bottom_bar.rs:16–22`). With the door gone it is **the only statement about the
run that exists outside the merged place**. Removing it too would leave the bar
saying nothing about what is next and navigation as the only way to ask — which
is precisely the audit finding that put a queue control in the bar to begin
with (`bottom_bar.rs:5–14`).

It does **not** gain the list's name: §3.5 ③ refuses that on the measurement
above.

#### 6.4.4 Every queue affordance, and its fate

Nothing here is dropped by omission. Fifteen things `Place::Queue` had:

| | Affordance | Where it is today | Fate |
|---|---|---|---|
| 1 | Row click → jump | `queue.rs:554`, `Message::JumpToQueued` | **Survives**, unchanged |
| 2 | Per-row ✕ | `queue.rs:714–749` | **Survives**, hover-gated |
| 3 | ▲▼ steppers | `queue.rs:575–588`, `queue.rs:624–665` | **Survives**, hover-gated |
| 4 | The transfer `+` | `queue.rs:671–702` | **Survives**; the picker floats over this place as over every other (`app.rs:3857–3872`) |
| 5 | Drag-to-reorder | `queue.rs:558–571` | **Survives**; the ghost is already a whole-window layer (`app.rs:3915–3922`) and composites over the field without a new mechanism |
| 6 | `Save as playlist` + name field | `queue.rs:289–338` | **Survives**, and the field is prefilled with the run's list name (§3.5 ④) |
| 7 | `Undo`, and `Ctrl+Z` | `queue.rs:266–287`, `app.rs:3619–3651` | **Survives**. `note_place_left`'s `from == Place::Queue` (`app.rs:3661`) becomes `Place::NowPlaying`, **and turning `Run` off clears it too** — the word is in the run's summary strip, and an accelerator whose visible twin is off screen is not legal |
| 8 | The provenance-led summary | `player.rs:2167–2193` | **Survives and is promoted** to the surface's head, with a subject for six list kinds instead of one (ADR-0034) |
| 9 | Right-press mirror menu | `queue.rs:602–607`, `menu::Target::QueueRow` | **Survives**, unchanged |
| 10 | Row hover tracking | `queue.rs:604–605`, `hovered_queue_row` | **Survives**, unchanged |
| 11 | The virtual window | `queue.rs:130–196`, `queue_window.rs` | **Survives**, and becomes load-bearing at kiosk scale too — see the new gate condition in §12 M4 |
| 12 | The place's own scroll | `queue.rs:237–247`, `Message::QueueScrolled` | **Survives** as the run column's scroll |
| 13 | Album group headers | `queue.rs:348–375` | **Survives** — *albums are listed as albums, never flattened* |
| 14 | `place_header("Queue")` | `queue.rs:232` | **Dropped.** The merged place wears no header strip; the lane is the route, and the head states the list |
| 15 | The empty state | `queue.rs:382–414` | **Survives, and replaces the other one** — see below |

**The two empty states become one**, which is a real merge decision rather than
a detail. The surface would otherwise have both *"Nothing playing."*
(`now_playing.rs:111–118`) and *"Nothing queued"* with its two following lines
(`queue.rs:387–406`). The queue's wins and `now_playing.rs`'s branch is deleted,
because the queue's is strictly more informative: it names the gestures that
fill it and it carries the silence-is-a-feature sentence the product wants said
at exactly that moment. **The `transport_pending` branch above it
(`now_playing.rs:105–107`) stays** — a start in flight is still not silence, and
that reasoning is untouched by the merge.

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
>   sentence the ledger already applies to shuffle (the product's standing rule), and
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
>   the product's standing rule is why.

The last clause is what stops this class swallowing the other two. A drifting
field cannot tell you the track changed; the hard cut does that, and it stays a
hard cut.

### 7.2 What is toggle-able, where, and what the defaults are

**Four** independent switches, because they have genuinely different costs and
genuinely different audiences:

| # | Toggle | What it controls | Default | Cost when on |
|---|---|---|---|---|
| **T1** | **Ambient field** | The derived wash (§5.3), and whether it **drifts** or is still | **On, drifting** | §7.4 |
| **T2** | **Spectrum** | The bars (§10) — the surface's primary visual | **On** | §10.9 |
| **T3** | **Meter** | The R128 instrument readout (§9.5) | **Off** | §7.4 |
| **T4** | **Feed** | The rotating fact line (§8) | **On** | 3 wakes/min (§8.5) |

Plus one selector, not a switch: **Ballistics** — *Loudness · VU · PPM* —
governing **both** the meter's integration and the bars' decay, so the surface
cannot have two unrelated speed settings (§10.6).

**`Run` is not a fifth row here**, and the line is worth drawing sharply
(§3.4.3): T1–T4 decide how the surface *looks*, and `Run` decides what it is
*about*. It is a peer word beside `Ambient` in the place's top-right, with its
own persisted boolean, and it appears in Settings beside these four rather than
inside their menu. It also has no cost story to tell — a list that is not drawn
costs a `Vec` that is not built, which is the virtual window's own arithmetic
and not a subsystem's *off*.

**Why four and not one.** A single "ambient mode" switch would bundle a GPU
cost, an FFT, an engine-thread tap and a disk read into one control, so a
listener who wants the facts but not the bars would have to take both. Four
switches, four subsystems, and each one's *off* is a real structural saving
rather than a skipped draw call.

**Why T2 is on and T3 is off.** They answer different questions and only one is
a *visual*: the bars are what the owner asked to see, they read from three
metres, and they are the surface's primary motion. The R128 readout is a
**reading** — a precise number for a specific question, legible at 60 cm — and a
kiosk that opens covered in decibel figures is an instrument panel rather than
something you leave on. Both are available; the visual is the default.

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
  top-right, opening a small menu with the four switches and the ballistics
  selector. It is a labelled,
  pointer-reachable control at rest, which the product's standing rule requires and
  which a hover-revealed gear would violate. It is a *word* and not a glyph for
  doc 10 §3.4's reason: the enumerated symbol list is closed at two, the gear
  and the magnifier (`system.md:876–879`), and no universal symbol means
  *ambient*.
- **In Settings** — the same four switches and selector, in the playback section, because
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
T1 to still and leaves T2, T3 and T4 alone — a spectrum and a meter are
readings rather than motion for motion's sake, and a rotating fact is not an
animation at all.

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
   `ambient.animating()` is `T1.drifting || T2.on || T3.on`. **T4 is
   deliberately *not* in that term** because the feed is not animated: it
   advances on a 20 s dwell, so it gets its own far slower arm under the same
   guard, and §8.5 prices it honestly at three wakeups a minute rather than
   claiming it is free.
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
| **Control** | the same binary with all four toggles off, which must reproduce doc 04's `off` driver exactly |

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
| F1 | **"Played 34 times since March 2019"** | `TrackHistory::plays`, `first_played_unix_s` (`history/read.rs:150–159`) | **The best one.** It is the ledger's own permitted form (the product's standing rule), and it is the fact nobody else has |
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
(the product's standing rule). A rotating fact is close enough to that line that the
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
  performance, and satisfies the product's standing rule — the line is a labelled,
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

the product's standing rule is the entry that binds hardest here, and it is worth
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
| **Any framing that congratulates** — "you're on a 6-day streak", "your #1 record" | *History records; it never performs.* The tone test is that every fact reads as an **archivist's note** (the product's standing rule's posture for the condition report), stated flatly, with no second person doing anything impressive |

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
  structural guard as §7.3 (place on screen, T4 on). For scale, baz already runs
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
  (the product's standing rule).
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
   oil (the product's standing rule). The instrument's own caption says what it
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

**And the whole chain switches off together.** T3 off ⇒ no `Command::SetMetering`
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

§1.2's two distances want two different things from one measurement, and the
surface now has a distinct instrument for each.

**The far-field register is the spectrum (§10), and it is on by default.** The
owner's *"stylised… somewhat ambient"* and *"bars going up and down"* resolve to
the same object: light organised into columns, over the field, never over the
sleeve (§5.4), readable from three metres as movement rather than as a number.
It **states nothing** (§7.1's last clause), which is what the ambient class
requires — nobody reads a level off a bank of bars, and it is not asked to carry
one.

*(An earlier draft made the far-field register a global luminance response of
the field to momentary loudness — the room breathing. The bars are the same idea
resolved into something you can actually see, and they are what the owner asked
for, so the breathing field is **not built**. It is recorded here so that
re-proposing it is a decision rather than a rediscovery: it would be nearly free,
since the field's shader already has the loudness value as a uniform.)*

**The near-field register is the meter proper, and it is off by default.** On
the placard column, at the work's own width, `METER_H` 24: a horizontal dB scale
with the momentary reading, the peak hold, and — the detail that makes it worth
having — **a fixed mark at this record's own integrated loudness (F6)**, so the
live reading is legible as *louder or quieter than this record's average* rather
than as an abstract number.

**Why the meter is not the default, stated rather than assumed.** It is a
*reading*, and a kiosk that opens covered in decibel figures is an instrument
panel rather than something you leave running. The bars answer *what is the
music doing*; the meter answers *what level is it*, which is a question a
smaller number of people ask at a smaller distance. Both ship; the visual is
what greets you. §10.7 is the guarantee that turning both on cannot produce two
stories about the same instant.

### 9.6 Why the meter is not amber, and other refusals kept

- **Not amber.** a standing rule of the product: the accent *"states what is true about
  playback right now and nothing else: not what is queued, not what is selected,
  not what has focus"*. A level is not *which record, which track, where the
  playhead is* — it is a measurement of the audio. The meter draws in the room's
  own inks (`paper`, `paper_dim`, `paper_faint`); the needle keeps the amber,
  because the needle is exactly what the accent is reserved for. **The one
  exception is the peak indicator crossing 0 dBFS**, which is playback truth of
  the kind the accent exists for — and it is accompanied by the numeral, because
  the product's standing rule forbids state signalled by colour alone.
- **Not an instrument face.** §14.3 rewrites the skeuomorphism entry, and the
  line it draws is the entry's own: *"the record supplies physics, structure and
  vocabulary… it never supplies **surface**."* **Refused: a beige panel, a glass
  face, a printed arc scale, a pivoting needle, a bezel, a lamp behind the
  dial.** **Permitted: the measurement**, drawn in baz's own vocabulary — a line,
  a mark, a numeral, the room's inks. The owner's *"in a stylised way"* is read
  as exactly this: the instrument's *behaviour*, not its *costume*.
- **No peak-hold-forever, no session maxima, no "loudness score".** Those are
  engagement stats about audio, and the product's standing rule's tone rule applies to
  the meter as much as to the feed.
- **No headroom claim, no "audiophile" framing.** the product's standing rule. The
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
  not low, not shimmering.** *Silence is a feature* (the product's standing rule) is
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
- **Not amber.** the product's standing rule reserves the accent for *"what is true
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
(the product's standing rule).

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
desktop case.

> **Corrected 2026-08-10, when A4's run half shipped.** *"Every window at or
> below 720 px of work"* does not include 1920 × 1080, and this section was
> written as though it did. A 1920 body's work is **773 px**, because `below` is
> **146** in the build and not the 190 §11.2 assumed — the same stale input
> §5.5a's table corrects twice for other rows. So the run at 1920 is
> `440 × 1.074` = **472**, not 440.
>
> **Nothing a listener sees moves badly**, which is why it was allowed rather
> than tuned: the work at 1920 is 773 px with the run at 440 and 773 px with it
> at 472, because the run takes width the record is structurally unable to use.
> `1280 × 860` — the window every composition audit in this project is taken at
> — is genuinely untouched, and measured so: the two columns' band diffs at
> **0 pixels** between the builds. Keying the reference to make 1920 land on
> exactly 1.0 would be a constant chosen to flatter this table, and the table is
> the thing that is stale. The ceiling of 2.5 stops a very large source producing type that
is absurd at 60 cm on the same panel.

**What does not change at any size**, and this is the property that made the
shipped surface right in the first place: the composition. There is no separate
kiosk layout, no second view function, no mode. `now_playing.rs`'s own test —
*"the kiosk is this surface at a larger size, and it is a property of the
arithmetic rather than a plan"* (`now_playing.rs:214–234`) — is extended with
the new terms rather than replaced.

**The run at kiosk size**, which §3.4.2 promised to settle here:

- **Full-screen does nothing to it.** `Run` is on or off because a person said
  so, and `F11` is a window act (§3.2). The kiosk listener turns the run off
  once and it stays off; the single-display listener keeps their editor. The
  toolkit could not support the alternative in any case — with no monitor
  enumeration in iced 0.13 (§11.1), baz cannot tell a second-display
  full-screen from an only-display one.
- **Its type scales like everything else**, and its column with it:
  `run_w = RUN_MEASURE · kiosk_scale`, so 440 at 1920 and **1100** at 4K, with
  `SIZE_BODY` 13 → **33** on a run row. A run at three metres is *the next few
  tracks, large*, which is a thing worth looking at.
- **Its editor costs the kiosk nothing, structurally.** Every edit control on a
  run row is hover-gated (`queue.rs:572`), and nobody hovers a screen three
  metres away. The far field gets the reading and the near field gets the
  editor, from the same code, with no mode deciding which — which is §1.2's two
  distances resolving themselves for the second time in this document.
- **§5.5a's table shows the record loses nothing**: at 3840 × 2160 with a
  3000 px source the work is 1809 either way, because it is height-bound and the
  run takes width the record cannot use.

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
because steps 1–7 are all front-end changes to a place that already exists,
while steps 8 and 9 touch the pump path and ADR-0009's promise. Front-loading
the riskiest work would mean the visible improvements wait on it — and the
closing note under step 9 gives the shorter path if the bars are wanted first.

### 12.0 The merge re-orders this plan, and one step of it is already half done

Four new steps, **M1–M4**, and the nine below become **A1–A9**. The disposition
of every original step is stated rather than left to be inferred.

**The merge goes first, whole.** Steps A2–A9 all draw into a composition the
merge is about to change; building the hero decode, the field and the type scale
against a single-column surface and then re-laying it out is rework with a
receipt. It is also the owner's live ask.

| # | Step | Disposition |
|---|---|---|
| **M1** | The merged surface: the run column, ~~`Run`~~, `SPLIT_FLOOR` | **✅ Shipped** 2026-08-10 — **the `Run` word was removed by the owner the same day** (§3.4.3) |
| **M2** | The door comes off; `Place::Queue` is deleted | **✅ Shipped** 2026-08-10 |
| **A1** | *Delete the duplicate transport* | **✅ Shipped** — its surviving half rode in M1 |
| **A2** | The hero decode; `NOW_PLAYING_MAX` deleted | **✅ Shipped** 2026-08-10 — **and it took A3 with it**, see below |
| **M3** | `Origin`, and the head gains its subject | New — after A2, per the order below |
| **A3** | The field, static | **✅ Shipped** 2026-08-10, in A2 — the two are one visual answer |
| **A4** | The kiosk type scale | Survives, **+ the run** |
| **A5** | The feed | Survives, unchanged |
| **M4** | The run marker: the command field, ledger v1.1, the lane across a quit | New — after A5, per the order below |
| **A6** | The toggles | Survives, **and must now design its own control** — `Run` is gone, so `Ambient` has no peer to sit beside (ADR-0029 §8.5) |
| **A7** | The field drifts *(gated)* | Survives — **but §5.4 term 3 has no mechanism left**: the field's domains went with the seam the owner reported (ADR-0029 §8.7), so A7 must answer *drift under scrolling type* by slowing or stilling the whole field, not by re-introducing a region |
| **A8** | The tap and the spectrum *(gated)* | Survives, **+ a widened mask and a harder gate** |
| **A9** | The meter | Survives, unchanged |

**Nothing below is void outright, and A1 is the only one that changed shape.**

#### A1 is half done, and the half that is left is a subtraction of 32

> **Done, 2026-08-10, in M1.** `below` is now
> `LINE_HEADING + LINE_HERO + LINE_BODY + NEEDLE_H + 4 × GAP_LG` = **130**, and
> `the_placard_reserves_no_transport_it_does_not_draw` pins both that figure and
> the 162 it replaced. **The 190 below is still the *future* number**: it is 130
> plus the meter's 24, the feed's 20 and one `GAP_LG` — none of which are built,
> and none of which may reserve height before they exist. Every `by_height`
> figure in §5.5 and §5.5a is therefore **60 px larger in the shipped build**
> than the table states, and will shrink to the table's number as A5 and A9
> land. The *properties* the tables were making an argument for — the record is
> height-bound above 1280, the run takes width it cannot use — hold at both
> values, which is why they were stated as properties in the tests rather than
> as rows.


The duplicate widget is **gone**: `now_playing.rs:178–189` now carries the
argument in a comment where the call used to be, and there is no
`bottom_bar::transport` in the file. **But the 32 px it reserved is still in the
arithmetic** — `art_edge`'s `below` still sums `theme::TRANSPORT_HIT`
(`now_playing.rs:62–67`), so today's `below` is **162** and the artwork is
32 px smaller than it should be at every height-bound size. That is A1's second
half, it is one line, and it rides in M1 because M1 touches that function
anyway. §5.5's `below` of 190 is 162 − 32 + the meter's 24 + the feed's 20 +
one `GAP_LG` 16, and now says so.

---

**Step M1 — The merged surface.** *(§3.4, §5.5a — the owner's ask)* — **✅ shipped 2026-08-10**

`Place::NowPlaying` grows a second column. `views/queue.rs`'s body becomes
`views/now_playing.rs`'s run column, taking `RUN_MEASURE` and the run's own
scroll; `art_edge` gains the `run_w` term and loses `TRANSPORT_HIT` (A1's
second half); the `Run` word joins the place's top-right; `SPLIT_FLOOR` restacks
the two columns into one below 784 px of body. **`Place::Queue` still exists and
still routes** — both doors work for exactly one step, which is what makes this
reversible.

*Ships*: the integration, at both sizes, with every queue gesture intact.
*Tests*:
- `the_run_costs_the_record_nothing_above_1280` — §5.5a's table as a sweep:
  `edge` with the run equals `edge` without it for every body ≥ 1053 px wide.
- `the_two_columns_restack_below_the_split_floor`, swept 400–4000 as
  `art_edge`'s existing test is (`now_playing.rs:239–256`).
- `every_queue_affordance_survives_the_merge` — §6.4.4's table as a source
  assertion over the new module, in the shape `queue.rs:774–839` already uses:
  each of `JumpToQueued`, `ShiftQueued`, `RemoveQueued`, `AddQueuedToPlaylist`,
  `SaveQueueStart`, `Undo`, `DragLift`, `DragOverRow`, `QueueScrolled` is still
  spent, and the two spacers are still built.
- `the_run_is_virtual_at_kiosk_scale` — `queue_window`'s slice is what is built
  at 3840 × 2160 with 40 000 rows.

---

**Step M2 — The door comes off, and the place with it.** *(§3.4.4, §6.4)* — **✅ shipped 2026-08-10**

Delete `Place::Queue`, `Place::queue()`, `Message::ToggleQueue`,
`views::queue`'s place wrapper, `bottom_bar::queue_button`, `theme::UP_NEXT_W`,
`theme::POSITION_W` and `PlayerState::queue_size_note`. Re-aim `keys.rs:401` at
`Place::go(Destination::NowPlaying)` **plus `Run` on**. Merge the two empty
states (§6.4.4). Delete the two stale `Q` comments (`place.rs:158`,
`bottom_bar.rs:347`).

*Ships*: **the owner's ask, completed** — one surface, one door, nothing drawing
the same list twice.
*Tests*:
- `the_bar_does_not_move_when_the_door_goes` — the existing reserved-slot suite
  (`bottom_bar.rs:1201–1290`) re-run with `UP_NEXT_W` gone; the transport
  column's x is unchanged at 1280 and 1920.
- `ctrl_u_is_the_lane_rows_accelerator` — the chord resolves to the same place
  the lane's row does, and **pressing it twice leaves you there**
  (`place.rs:344–362`'s `a_destination_never_closes_itself`, extended).
- The place enum's exhaustive walk (`place.rs:446–520`) loses a member and
  keeps its property.

---

**Step M3 — `Origin`, and the head gains its subject.** *(ADR-0034 §1, §3.5 ①④)*

`implicit::Origin` promoted and grown its three identified kinds (ADR-0034
§1.4 — **there is one `Origin`, not two**); `QueueVm::provenance` becomes
`origin: Option<Origin>`; the six construction sites (`vm.rs:859`, `vm.rs:984`,
`app.rs:2018`, `app.rs:2062`, `app.rs:2080`, `app.rs:3262`, `app.rs:3321`) say
which list they are reifying; `queue_provenance()` becomes
`origin().filter(Origin::is_destination).map(…)`; `queue_summary`'s one line
(`player.rs:2189–2192`) reads the new field; `save_field` prefills the name;
`lane::played_list` maps `Origin` to `Subject`. **No engine, no ledger, no
protocol.**

*Ships*: `Ochre · 2 of 24 · 1:52:56 left` — the frame
`impl/queue-in-now-playing/01a` with its subject restored — and list attribution
in the lane within a session.
*Tests*:
- `every_run_names_its_list` — one case per `Origin` variant, asserting the head's
  string.
- `only_a_file_is_a_destination` — the picker offers `Add to "{name}"` for
  `Playlist` and for nothing else, which is the assertion that keeps
  `no_menu_anywhere_offers_to_add_to_the_implicit_list` true.
- `an_append_moves_the_run_to_hand` — ADR-0034 §1.1.

---

**Step M4 — The run marker.** *(ADR-0034 §2–§5 — the first step that touches
`baz-core`)*

`Command::SetQueue` gains `origin: Option<String>` with
`#[serde(default, skip_serializing_if = "Option::is_none")]`; the engine carries
it to the ledger writer and reads nothing in it;
`history::format` gains the `# baz run` marker's encoder and decoder;
`History` gains `runs()` beside `by_path`; the launch fold (`app.rs:5986–6002`)
reads it; `session.rs`'s snapshot carries the encoded string.

*Ships*: **`docs/BACKLOG.md:9–25` closes** — a list played last week is still
credited to the list after a quit.
*Tests*:
- `the_pinned_command_bytes_are_unchanged` — `command_wire_format_is_stable`
  (`protocol.rs:1586`) passes **untouched**, with a new case appended for the
  `Some` arm.
- `a_v1_reader_counts_no_damage_in_a_v1_1_file` — the decisive one: a ledger
  with markers, read by `decode` and `from_reader` **exactly as they stand
  today**, yields `malformed() == 0` and every record. The four
  four-tab pins (`format.rs:23`, `format.rs:426`, `tests/history.rs:205`,
  `fuzz/fuzz_targets/history_line.rs:24`) and the byte-exact line
  (`format.rs:387`) and header (`history.rs:669`) all pass **unmodified** — if
  any of them needs editing, the design was wrong and the sixth column is back
  on the table.
- `a_play_with_no_marker_before_it_is_unknown`, and
  `an_unknown_origin_kind_reads_as_none_rather_than_damage`.
- `a_dangling_marker_is_ignored`.

---

**Step A1 — Delete the duplicate transport.** *(**✅ shipped**; see §12.0.)*

~~Remove `crate::views::bottom_bar::transport(player, ink)` from
`now_playing.rs:168` and its `GAP_XL`.~~ **Done** — the call is gone and
`now_playing.rs:178–189` carries the argument where it stood.

~~**What is left**: drop `TRANSPORT_HIT` from `below` in `art_edge`
(`now_playing.rs:62–67`, still summing it today), which grows the artwork by
32 px at every height-bound size for free. **Folded into M1**, which touches
that function anyway.~~ **Done in M1**: `below` is 130.

*Ships*: a larger sleeve, at no cost.
*Test*: `the_place_draws_no_transport` — the place's element tree contains no
transport widget; the bar's is untouched. Existing `art_edge` tests updated for
the new `below`.

---

**Step A2 — The hero decode, and the refusal made true.** *(§5.2)* — **✅ shipped 2026-08-10, with A3**

`art::load_hero(first_track, HERO_PX = 1024)` beside `load_thumb`, same
resolution order, `image.thumbnail` (downscale-only). A 2-entry LRU on `Shelf`
keyed by album id, filled for the sounding record and its successor. `art_edge`
gains its third term (`hero_px`); **`NOW_PLAYING_MAX` is deleted.**

*Ships*: artwork at its real size — 1000 px at 2560, up from 720 — and
the product's artwork entry becomes true on this surface for the first time
(§0.4 b).
*Test*: `the_now_playing_surface_never_draws_art_larger_than_its_source`, swept
over `hero_px ∈ {120, 320, 500, 1024}` × sides 400–4000, mirroring
`shelf.rs:1509–1530`.
*Gate*: memory — the 2-entry hero cache must not exceed 8 MiB.

> ### What A2 got wrong when it met the code
>
> **Measured, 2026-08-10** ([`impl/artwork-at-size/`](impl/artwork-at-size/README.md)),
> against two release binaries at 1280 × 860, 1920 × 1080 and 2560 × 1440, in
> both densities. Five corrections, and the first is a change to the plan
> rather than to a number.
>
> **1 · A2 alone does not answer the owner, so A3 rode with it.** The ask was
> *"fullscreen the now playing looks weird"*, and the arithmetic says the
> clamp is only half of why. At 1920 the record is **height**-bound, so
> deleting `NOW_PLAYING_MAX` buys **53 px** — 720 → 773 — and leaves the same
> 1000-odd px of empty room the complaint was about. At 2560 it buys 304, and
> still leaves 1250. **The clamp was making the square small; the missing
> field was making the room empty**, and shipping the first without the second
> would have been a release that measured better and looked the same. A3 is
> one gradient behind one place and it was cheaper to draw than to defer.
>
> **2 · §5.2's formula contradicts §5.2's test.** The printed expression ends
> `.max(ART_MIN)`, and the test six lines under it asserts
> `art_edge(side, side, 120) <= 120`. At a 120 px cover the expression is
> **240** and the test fails. The test is right: `ART_MIN` is a *design* floor
> saying a work this small has stopped being a subject, and `hero_px` is a
> *fact* saying there are no more pixels. **The fact outranks the floor**, S7
> asks for the small cover *"drawn at its own pixel size, centred, never
> scaled up"*, and the shipped clamp is `.max(ART_MIN).min(source)`.
>
> **3 · The successor cannot be prefetched, and will not be until M3.** The
> two entries were budgeted as *"the sounding record and the one after it"*.
> The one after it **cannot be named from the front end**: `vm::QueueVm`'s rows
> carry a title, an artist and an album *string*, and no path and no album id —
> the engine holds the paths. Matching two strings against the wall would pick
> the wrong edition for any listener who owns two of a record. So the second
> entry holds **the record that just stopped**, which the LRU gives for free
> and which a `Prev` press collects. Naming the successor is
> [ADR-0034](../adr/0034-the-run-and-its-list.md)'s `Origin` — step M3 — and it
> is one line here once that lands.
>
> **4 · The decode was never downscale-only, in either tier.** §5.2 says
> `image.thumbnail` is *"downscale-only, exactly as `load_thumb` does"*, and
> `art.rs` has said the same since v0.1. It is not:
> `DynamicImage::thumbnail` forwards to `resize_dimensions(.., fill: false)`,
> whose ratio is not clamped at 1 (`image-0.24.9/src/dynimage.rs:716–719`), so
> a 120 px cover was decoded to 320 × 320. It never showed on the wall, where a
> 320 px handle in a 320 px tile is 1 : 1 either way; it shows the instant a
> surface reads the decode's size and believes it, which is exactly what this
> step's third term does. Both tiers are guarded now.
>
> **5 · `below` was 130 and the column lays out 146.** Not a number this
> document printed — §12.0's note recorded 130 as M1's honest figure — but a
> number this step had to spend, and it was 16 px short: `.spacing(GAP_XS)`
> applies between *all six* of the placard's children rather than only the
> artist and the title, and the needle draws `NEEDLE_H + GAP_XS` tall so its
> tick reads as a mark on the line. `NOW_PLAYING_MAX` 720 was hiding the
> shortfall by leaving 69 px of slack at 1920; A2 spends that slack and the
> two timestamps go off the bottom. **§5.5's future 190 is therefore 146 plus
> the meter's 24, the feed's 20** — the same arithmetic, from a correct base.
> A sibling defect fell out of the same reading: the re-stacked head block was
> reserving `BELOW` where its own column comes to **38**, and that figure is
> `views::queue`'s `rows_top`, so the over-reservation was not blank space —
> it was the virtual window measuring its slice from the wrong offset.
>
> **What the frames confirm.** 720 → **1024** at 2560 (source-bound), 720 →
> **773** at 1920 (height-bound), **456 → 456** at 1280 with the run standing
> (width-bound, untouched), and a 300 px collection drawn at **300** where it
> was drawn at 720. The field: no ambient patch over **L 0.220** at any size,
> none under the room's own 0.158, **0.155–0.160 under the run column** against
> `wall` 0.158, chroma **0.022–0.025** against the pinned 0.024, and a
> monochrome collection reading `#0C0D0E` **exactly**.
>
> **One thing A2 does not fix, and A4 does.** At 2560 with the run standing
> the record column is 1800 px and the work is 1024 of it, left-hung, so ~700
> px of field sits between the sleeve and the run. That is §5.5a's left-hang
> working as written; **A4** scales `RUN_MEASURE` by `kiosk_scale` — 440 → ~1100
> at this size — which is most of that gap.
>
> > **Measured and shipped 2026-08-10** — the run's half of A4, on the owner's
> > *"make sure the layout of the now playing makes sense on wider screens"*.
> > [`impl/one-list-drawn-once/`](impl/one-list-drawn-once/README.md).
> > **Three corrections to the paragraph above.**
> >
> > 1. **The gap is 1171 px at 2560 × 1440, not ~700.** The ~700 assumes a
> >    1024 px cover; the field between the columns is *everything the work
> >    cannot use*, so a smaller cover leaves more of it, and the fixture's
> >    covers are 600 px. The arithmetic was right about its own cover and the
> >    defect is worse than it read.
> > 2. **`RUN_MEASURE · kiosk_scale` is ~692 at 2560, not ~1100.** 1100 is the
> >    scale at its 2.5 **ceiling**, which is a 4K figure; 2560 × 1440 gives a
> >    1133 px `by_height` and a scale of 1.57.
> > 3. **A4 is not most of that gap, and it is not the whole fault.** It closes
> >    1171 → 919. The work at that window is bound by the **file**, so none of
> >    the rest was the run's to give back. The owner's own first telling had
> >    both halves — *"the playlist hugs right and the art hugs left"* — and the
> >    left one is `view`'s: the record's container was `width(Fill)` with no
> >    `align_x`, so every spare pixel piled up between the columns. The two
> >    columns centre as one pair now, which is `views::page::view`'s rule, and
> >    the gap is one `GAP_XL` at every size.

---

**Step A3 — The field, static.** *(§5.3, §5.4 term 2)* — **✅ shipped 2026-08-10, in A2**

Palette extraction from the decoded hero (three clamped colours), composited as
an ordinary gradient behind the place's body. **No shader, no clock, no
toggle yet** — this is the still state, which every backend draws and which
§7.5 needs as the fallback anyway.

*Ships*: the single largest visual change in this document, at zero per-frame
cost, on every renderer.
*Test*: extraction is deterministic for a given cover; the composite never
exceeds L 0.22; a cover with no chroma yields the room rather than a grey wash.

> **Shipped in A2**, and the reason is A2's own first correction above: the
> clamp made the square small and the absent field made the room empty, and the
> owner's sentence was about the room. Three things this step's text did not
> anticipate:
>
> - **Only *hue* is read from the record.** *"Lightness and chroma clamped into
>   the room's own range"* is what §5.3 says, and a clamp is not enough — most
>   covers sit far above L 0.22, so clamping collapses all three colours onto
>   the ceiling and the field is flat. What ships is the sentence §5.3 quotes
>   from the ledger instead: **hue read from the record, lightness and chroma
>   pinned**. Three hue angles, hung on the room's own ladder. That also makes
>   §5.3's first property literally true — three angles cannot reconstruct an
>   image — and `crate::field::Field` is three `f32`s for exactly that reason.
> - **The chroma is 0.024, and it is a gamut measurement.** A binary search over
>   both rooms' ladders at one-degree steps puts the largest chroma that leaves
>   sRGB **nowhere** at 0.0269; above it, cyans clip at Closing Time's floor and
>   oranges at Reading Room's wall. A clipped channel is a hue that is no longer
>   the record's, so the constant is measured and the clamp is never reached.
> - **iced 0.13 has no radial gradient.** §5.3 asks for *"a slow
>   radial-plus-linear wash"*; `iced::Gradient` has one variant, `Linear`
>   (`iced_core-0.13.2/src/gradient.rs:9–12`). The linear half ships honestly
>   rather than being faked with stacked containers, and the radial half is A7's
>   shader or nothing.
>
> And one thing it did anticipate, now measured: §5.3 requires the wash to be
> **continuous**, *"which at these lightnesses means dithering"*. iced already
> dithers its gradients — **7/255 within a channel**, about 0.012 oklch L —
> which is why the frames' figures are 9 × 9 patch means.

---

**Step 4 — The kiosk type scale.** *(§11.2)* — **the run's half shipped
2026-08-10; the type is not built**

`kiosk_scale(edge)` and its application to the placard, the feed and the needle.

> **What shipped is `kiosk_scale` itself and its one consumer, `run_w`** —
> `views::now_playing`, on the owner's *"wider screens"* report, because that
> was a queued beta blocker and the type scale is a 1.0 kiosk surface
> (`docs/WORK.md`'s scope). The placard, the feed and the needle are **not**
> scaled and no size token moved, so `the_type_scale_is_identity_below_720`
> below is still unwritten and still the gate for the rest of this step.
> `kiosk_scale` lives in `views::now_playing` rather than `theme` for the same
> reason — it has one consumer — and moves to `theme` when the type takes it.
> Frames: [`impl/one-list-drawn-once/`](impl/one-list-drawn-once/README.md).

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

T1–T4 as four booleans plus the ballistics selector, the `Ambient` word-door and
its menu, the Settings rows, persistence. T2, T3 and T1-drift are wired to
nothing yet; this step is the
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

**Step 8 — The tap, and the spectrum.** *(§10 — the headline visual, and the
first step that touches the engine)*

In `baz-core`: `SpectrumRing` (16 384 `f32`, overwriting, single writer);
`Command::SetSpectrum(bool)` swapping the session's `Option<SpectrumRing>`; the
`write_downmixed(&[f32])` tap on `a`/`b` in `pump`. In `crates/baz`: a
`realfft` planner and owned scratch buffers, Hann windowing, the geometric
banding, the attack/decay and peak-hold ballistics, and the bars in the field's
shader with the `Canvas` fallback.

*Ships*: **the thing the owner asked to see.**
*Tests*:
- **Bit-exactness**: the existing ADR-0009 suite passes **unmodified**. The tap
  takes `&[f32]`, so this is a regression guard rather than the proof.
- **`the_bars_are_still_at_digital_silence`** — an all-zero window puts every
  bar at exactly zero height, not near it (§10.5).
- **`the_bars_do_not_move_with_the_volume`** — sweeping the fader changes no
  bar, because the tap is pre-gain.
- **No allocation per frame**: `process_with_scratch` with owned buffers, and no
  allocation on the pump path, asserted as `device.rs` and `volume.rs:432–437`
  already do.
- **`a_slow_reader_never_blocks_the_writer`** — the ring's writer completes with
  no reader present and with a reader stalled.
*Gate*: §10.9's thresholds, including the FFT's < 1 ms and the 4K frame time —
**and, since the merge, measured with a five-figure run on screen** (§5.5a).
The spectrum's cost is per-frame and the run's is per-visible-row, so a gate
taken over a twelve-track album would be measuring the easy half of the
composition. The mask's domain also widens from one column to the type box
(§5.4), which is a change to a uniform rather than to the pass.

---

**Step 9 — The meter.** *(§9 — a second consumer of step 8's tap)*

In `baz-core`: `pub(crate)` on `KWeighting`; a `LiveMeter` with fixed-size
accumulation and no `Vec` growth; `SharedMeter`'s two `AtomicU32`s;
`Command::SetMetering(bool)` swapping the session's `Option<LiveMeter>`; the
`observe(&[f32])` call beside step 8's ring write, on the same slices at the
same instant. In `crates/baz`: the instrument register and the shared ballistics
selector.

*Ships*: the precise reading, for the person who wants it.
*Tests*:
- **Compliance vectors** per §9.1: VU reaches 99 % in 300 ms ± tolerance with
  1–1.5 % overshoot; PPM falls 24 dB in 2.8 s; momentary agrees with
  `loudness.rs`'s integrated figure on a steady tone. **The published constants
  are read from the standards at implementation time, not from this document**
  (§13 R1).
- **The agreement tests of §10.7**: `the_meter_and_the_bars_agree_on_silence`
  and `..._on_a_full_scale_sine`, plus
  `neither_instrument_moves_with_the_volume`.
- **Zero when off**: with `SetMetering(false)` the session holds no `LiveMeter`.
*Gate*: §7.4's thresholds re-run with both instruments live.

---

**A note on the order.** Steps A1–A7 and M1–M3 are front-end work on a place
that already exists; **M4** is the first to touch `baz-core` and A8 is the
first to touch the pump path, which is why the engine work sits behind the
visible wins rather than in front of them.

**The whole order, after §12.0's re-ordering:**

> **M1 · M2 · A2 · M3 · A3 · A4 · A5 · M4 · A6 · A7 · A8 · A9**

**A2 and A3 shipped together, 2026-08-10**, so what remains of that order is
**M3 · A4 · A5 · M4 · A6 · A7 · A8 · A9**. The pairing was not opportunism: the
clamp is what made the artwork small and the *absent field* is what made the
room empty, and the owner's sentence — *"fullscreen the now playing looks
weird"* — was about the room. A2 alone buys 53 px at 1920 and leaves the void
exactly where it was.

**If the owner wants the bars sooner than this order delivers them**, the
shortest honest path is **M1 → M2 → A2 → A6 → A8**: the merge, the door off,
the artwork at its real size, the toggles, and the spectrum — with the static
field (A3), the type scale (A4), the feed (A5) and the model's ledger half (M4)
following. That path reaches the headline visual in five steps instead of
twelve and gives up nothing structurally; it only means the bars arrive over a
plain `#0C0D0E` room rather than over the derived field for a release or two.

**M1 and M2 are not skippable in that path or any other**, and the reason is
not that they are the owner's ask (though they are): every later step lays out
against the merged composition, and a release that shipped the field, the type
scale and the spectrum into a single-column surface would have to draw all three
again.

---

## 13. Deferred, and re-verification — ranked

Everything this study declined, in the order it should be picked up.

**Deferred work:**

| | Item | Why deferred | What would trigger it |
|---|---|---|---|
| **D1** | **A 4096-point transform for the lowest octave** (§10.4) | With 21.5 Hz bins the 32–64 Hz octave holds ~1.5 bins, so the bottom bars share them and move together. That is the data being honest; a second transform is the only fix that is not interpolation | The bass reading as visibly ganged on real material, once step 8 ships |
| **D2** | **Network enrichment** (§8.6) | Blocked on an identifier baz does not store: every good source is MBID-keyed and baz holds no MBIDs. That is a scan-and-schema change, not a UI one | The local feed shipping and proving the composition has no hole in it |
| **D3** | **Embedded lyrics** | A new scan capability (`lofty` can read the frames); and at 3 m a scrolling lyric column is the wrong object for the far field | A demand this study did not find |
| **D4** | **A first-seen column in the index** | Would make *"added to your collection in 2019"* true. Today only `mtime` exists and a re-tag rewrites it, so the fact would be a plausible-looking lie (§8.1) | A schema change for another reason, which this would ride along with |
| **D5** | **A second window on a named monitor** (§11.1) | Two blockers, not one: iced exposes no monitor handle, **and** global control flow would couple the kiosk's clock to the main window's idle | iced exposing `MonitorHandle` **and** an answer to the idle coupling. Both, not either |
| **D6** | **A periodic pixel nudge for burn-in** (§7.6) | The drift already moves the large areas; nudging type would make it shimmer | Real-world evidence of ghosting on a drifting field. Priced at one frame per 60 s so re-proposing costs an observation, not an argument |
| **D7** | **A lane row for a shuffle draw** (ADR-0034 §6) | A draw is an order, not a place you return to — and `design/dynamic-playlists` refuses draw provenance for its own reasons. Crediting one would need a real key (the seed) and a row to credit | The owner deciding a draw is somewhere you go back to. It is one `Origin` variant away, deliberately |
| **D8** | **`Origin` on `Event::TrackStarted`** (ADR-0034 §2) | A front end knows what it sent; echoing it back is ADR-0014 §6's refused move. It becomes necessary the day a *second* front end attaches to a running engine — the same day `EngineHandle::queue()` does | A second front end, which is one event's fan-out away and not close |
| **D9** | **A `Run`-off composition that states what it is hiding** | With `Run` off the surface says nothing about the list at all, where the bar's continuation line still does. Adding a one-line reading would be a third density | Evidence that kiosk listeners turn the run off and then miss it |

**Re-verification — claims in this document that are weaker than the rest:**

| | Claim | Current standing | Before it is leaned on |
|---|---|---|---|
| **R1** | The VU and PPM ballistic constants (§9.1) | Stated from the standards by name; **not read from the published documents in this session** | Read IEC 60268-17 and IEC 60268-10 directly at step 9, exactly as ADR-0015 asserted its coefficients against BS.1770-4's own tables |
| **R2** | Roon's Display mode and Plexamp's screensaver (§2) | **Not independently verified** — doc 03 recorded that Plexamp's UI page 301s and its layout *"was not seen"* | Direct examination before any composition decision cites them. Nothing in §5–§10 currently rests on either |
| **R3** | The GPU cost estimates (§7.4) | **Labelled estimates**, from the shape of the work | Step 7's gate, which is the measurement itself |
| **R4** | The FFT cost estimate — 20–60 µs per frame (§10.3) | **Labelled an estimate**, from the butterfly count | Step A8's gate, which measures it directly against a < 1 ms threshold |
| **R5** | The bar's block widths — 288 → 448 at 1280 (§6.4.1) and the 224 px title lane (§3.5 ③) | **Computed from tokens, not measured on a frame.** The zone arithmetic is `bottom_bar.rs:106–117` and `:260–278` read as written; no capture at those widths measured the block's inner edges | One `magick` measurement off a rendered bar before §3.5 ③'s refusal is cited as settled. The *direction* is certain — 160 px is `UP_NEXT_W` plus a gap — but the residue is not |
| **R6** | `RUN_MEASURE` 440 as a comfortable title lane | Derived as `LIST_MEASURE / 2` and checked against the row anatomy's 210 px of fixed slots (§5.5a), but **never rendered**. 230 px of title at `SIZE_BODY` 13 is inferred, not seen | Step M1's own captures at 1280 and 1920, which are the first frames that will contain a run column at all |

---

## 14. The ledger entries, rewritten

Per the product's standing rule, an entry the owner reverses *"gets rewritten to say what
was decided and why, and that is the whole of the process."* These are drafted
in the ledger's own voice, in the form it already uses for the hover veil
(the product's standing rule) and the wall's scrollbar (the product's standing rule) — the
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
> room's inks or the cover's own palette — with its standard named and its
> ballistics tested against the published document (EBU R128 / ITU-R BS.1770-4
> momentary by default; IEC 60268-17 VU and IEC 60268-10 PPM offered). Refused,
> and these are the surface the entry has always been about: a beige panel, a
> glass face, a printed arc scale, a pivoting needle, a bezel, a lamp behind the
> dial.
>
> **The spectrum analyser is admitted on the same terms**, and it is the
> now-playing surface's primary visual: a real transform of the audio, drawn as
> light in colours sampled from the record, below the artwork's luminance so the
> sleeve stays the brightest object. Refused for the same reason the dial's
> bezel is: a harsh fixed-hue bank, a gloss or bevel on a bar, a mirrored
> reflection, and the accent — amber states playback truth, and a spectrum is a
> property of the audio rather than of playback.
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
> content — a field that drifts, a spectrum analyser, a meter that moves — under
> ADR-0020 §7's discipline: it is a thing you start and never a thing that starts itself; it
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
