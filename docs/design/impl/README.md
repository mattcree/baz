# Increments 1–5 — rendered evidence

Pixel evidence for the first five increments of
[`../01-ux-audit-and-ia.md`](../01-ux-audit-and-ia.md) §5. Every image is the
real binary, captured the way [§0.2](../01-ux-audit-and-ia.md#02-how-the-screenshots-were-made)
specifies and nothing was touched that this work did not start:

- a private `Xvfb :141` at 1400×1000, `env -u WAYLAND_DISPLAY
  WINIT_UNIX_BACKEND=x11`; the app opens at its shipped 1280×860 and there is
  no window manager to move it;
- scratch `HOME` **and** scratch `XDG_DATA_HOME` / `XDG_CONFIG_HOME` /
  `XDG_CACHE_HOME` / `XDG_STATE_HOME`, so the maintainer's library database and
  config were never opened;
- a throwaway 18-album / 109-track fixture of generated covers and **digitally
  silent** FLAC, never `~/Music`;
- captures targeted at *this* process's window by pid, never "the active
  window";
- the build carries `device-output` — the transport is what three of the five
  increments are about — so the private `HOME` also carries an `.asoundrc`
  routing ALSA's default PCM to the `null` device. Two independent guarantees
  of silence: every sample is a zero, and the sink discards it.

One consequence of the null sink is worth stating because it is visible in the
frames: it accepts writes as fast as they arrive, so playback free-wheels at
roughly ten times real time. That is why the elapsed timestamps advance faster
than the interval between captures, and why a queue left alone walks forward
through its tracks. Nothing about the marking depends on it — the dot follows
`TrackStarted` either way.

## The frames

| Image | What it shows |
|---|---|
| [`01-shelf.png`](01-shelf.png) | **Increment 2 and 5.** Five columns at 1280 px. Every artist line sits on one baseline: *Selected Ambient Works 85-92* and *Music Has the Right to Children* clip at one line instead of pushing their artist line down. The bar carries `⏮ ⏵ ⏭` with Previous inert, because nothing is playing. |
| [`02-column-hold-during.png`](02-column-hold-during.png) | **Increment 3, mid-gesture.** One press on a tile; the inspector is up and the grid is *still laid out at five columns*. |
| [`03-column-hold-after.png`](03-column-hold-after.png) | The same click 400 ms later: the reflow to three columns lands. Deferred, never cancelled. |
| [`04-doubleclick-plays.png`](04-doubleclick-plays.png) | **Increment 3, the repair.** A double-click on column 3 of row 0 — the case the audit filmed failing ([`12-doubleclick-reflow.png`](../audit/12-doubleclick-reflow.png)) — now plays. The tile is haloed and dotted, the inspector marks the sounding track, the bar names it. |
| [`05-inspector-playing.png`](05-inspector-playing.png) | **Increments 1 and 4.** A click on the fifth track row started the album there (`SetQueue` + `JumpTo`, since the engine held nothing). The lamp dot sits in the 24 px number column, the row is carded, and the title takes the medium weight the bar gives the same string. |
| [`06-inspector-jumped.png`](06-inspector-jumped.png) | **Increment 4, the case worth having.** A click on row 1 while row 5 played: the engine already held this album, so this is `JumpTo` alone. The engine trace shows `queue #4` then `queue #0` with nothing between — a jump, not five skips. |
| [`07-shelf-1000.png`](07-shelf-1000.png) | The second width: 1000×760, two columns, the block still anchored to its columns rather than to its contents. |
| [`08-bar-stopped.png`](08-bar-stopped.png) · [`09-bar-playing.png`](09-bar-playing.png) · [`10-bar-paused.png`](10-bar-paused.png) · [`11-bar-seek-hover.png`](11-bar-seek-hover.png) | **Increment 5.** The bottom bar in four of its states, cropped to the same 1280×104 region of the window. |
| [`12-bar-states-stacked.png`](12-bar-states-stacked.png) | The four stacked, for reading the transport row down a single column. |
| [`13-bar-diff-playing-vs-paused.png`](13-bar-diff-playing-vs-paused.png) · [`14-bar-diff-stopped-vs-playing.png`](14-bar-diff-stopped-vs-playing.png) | Difference images, auto-levelled. Everything that lights up is *inside* a slot that was reserved for it. |

## What increment 3 does *not* fix, stated rather than implied

The audit's frame was a double-click on the **fifth** tile of row 0. Holding
the column count does not rescue that particular tile at 1280 px, and no
arithmetic in the grid could: five columns put it at x 1000–1240, and the
inspector the first press opens occupies x 940–1280. The tile does not move —
it is *covered*. A second press there lands on the panel because the panel is
there.

What the hold does fix is every tile that remains in the shelf's own width, and
that is where the failure actually lived: without it, column 3 regrouped to row
1 column 0, so the second press selected a different album and nothing played.
[`04-doubleclick-plays.png`](04-doubleclick-plays.png) is that case, working.

One residual is visible if you compare [`01-shelf.png`](01-shelf.png) with
[`02-column-hold-during.png`](02-column-hold-during.png): the block shifts left
by 16 px during the hold, because the grid is *centred* in the shelf viewport
and the viewport narrowed. Sixteen pixels is well inside a 240 px tile, so it
breaks no gesture — and it disappears when the rail does, in increments 6–8.

## The pixel-stability measurement

The bar gained a control, so its promise — *nothing moves as the music moves* —
was re-measured rather than assumed. Two readings, over six states (stopped,
playing, after Previous, paused, seek-hovered, resumed):

**The transport row's ink occupies exactly the same pixels in every state.**
Thresholded bounding box of the row, in the same crop:

```
bar-1-stopped            112x32+2+6
bar-2-playing            112x32+2+6
bar-3-after-previous     112x32+2+6
bar-4-paused             112x32+2+6
bar-5-seek-hover         112x32+2+6
bar-6-resumed            112x32+2+6
```

112 = 3 × `TRANSPORT_HIT` 32 + 2 × `GAP_SM` 8 — the three buttons and the two
gaps, to the pixel, whether the middle glyph is play or pause and whether the
outer two are live or inert.

**The bar's top edge does not move.** Sampled at x = 300 (a column with no
content in any state), the hairline is at row 2 of the crop and `RECESS` begins
at row 3 — in all six. A bar whose height varied with the transport would slide
that hairline.

The differing-pixel counts, playing versus each state, land where they should:

```
bar-3-after-previous       429 px  seek fill only (the position moved)
bar-4-paused               562 px  the toggle's glyph box + the seek fill
bar-5-seek-hover         1 687 px  the reserved preview lane + the fill
bar-1-stopped            4 241 px  title, timestamps and groove appear at all
```

---

# Increment 6 — Up next becomes a popover

The first surface to leave the rail (ADR-0016). Captured the same way as
everything above — a private `Xvfb :147` at 1400×1000, scratch `HOME` **and**
scratch `XDG_*` (including `XDG_RUNTIME_DIR`, so the maintainer's session bus
was never joined: the log reads `[mpris] no session bus`), an 18-album /
183-track fixture of generated covers and digitally silent FLAC, and a
`--features device-output` release build whose scratch `HOME` carries an
`.asoundrc` routing ALSA's default PCM to `null`. `BAZ_DEVICE_TESTS` was never
set. Captures are targeted at this process's window by pid.

| Image | What it shows |
|---|---|
| [`15-up-next-popover.png`](15-up-next-popover.png) | **Up next**, open. 360 px, anchored 16 px above the bar and 16 px in from the right edge, over the shelf and over the album inspector — **and not over the transport**, which keeps every reserved pixel and stays live. No scrim: the covers behind it are at full brightness. The playing row is carded and dotted, `2 of 12 · 43:54 left` summarises what is *ahead*, and the bar's `Up next  2 / 12` control is in its raised "open" state — the anchor the toolkit will not let the popover draw as a notch. |
| [`19-up-next-affordance-at-rest.png`](19-up-next-affordance-at-rest.png) | The same control with the popover shut, at 250%: **a labelled, always-visible door**, and no chrome at all until you reach for it. This is the prior-art study's R1 — *transient must not mean unverifiable* — and it is the one place this increment departs from the design spec, which offered only `Q` and an unlabelled press on the now-playing block. |
| [`16-up-next-esc-peels-one-layer.png`](16-up-next-esc-peels-one-layer.png) | **One <kbd>Esc</kbd>, one layer.** The popover is gone and the album inspector beside it is untouched — still open, still marking the playing track. A second press would take the inspector. Verified with an instrumented build that counted the messages: one key press, one `EscapePressed`. |
| [`17-up-next-click-outside.png`](17-up-next-click-outside.png) | **Click-outside, and only that.** The press landed on a shelf tile: the popover closed and the tile did not change selection, because the `mouse_area` layer captured the press before the place underneath could see it. This is the dismissal iced 0.13 gives no other route to, working. |
| [`18-bar-diff-popover-open-vs-closed.png`](18-bar-diff-popover-open-vs-closed.png) | The bottom bar, popover-closed against popover-open, auto-levelled. Everything that changed is inside the 152 px control. |

## The bar did not move

The bar gained a control, so its promise — *nothing moves as the music moves* —
was re-measured rather than assumed. The transport was **paused** first, so the
only variable across the three captures is the popover's state.

**Closed, then open, then closed again is pixel-identical to the first frame:**

```
magick compare -metric AE  crop-bar-a-closed.png  crop-bar-c-closed-again.png
0
```

**Nothing outside the control changed at all.** The bounding box of every
differing pixel between closed and open, in a 1280 × 104 bar:

```
152x25+282+41         the Up next control, and nothing else
                      (3 696 differing pixels, every one of them its card)
```

152 is `UP_NEXT_W` exactly.

**The three things that must not move, in all three states:**

```
                        transport row     label + readout ink   title+artist ink
bar-a-closed            110x28+45+0       119x11+0+3            94x28+1+1
bar-b-open              110x28+45+0       119x11+0+3            94x28+1+1
bar-c-closed-again      110x28+45+0       119x11+0+3            94x28+1+1
```

110 = 3 × `TRANSPORT_HIT` 32 + 2 × `GAP_SM` 8, less the two glyph boxes' own
transparent margins — the same measurement increments 1–5 made, unchanged. The
`Up next` label and its `2 / 12` readout occupy the same 119 × 11 px of ink
whether the popover is open or shut, inside the control's 152 px slot; and the
track title beside them is identical to the pixel, which is the thing a border
appearing on hover would have broken.

**And the bar's top edge is where it was.** Sampled at x = 470, a column with no
content in any state:

```
y0=srgb(124,51,34)  y1=srgb(19,17,16)  y2=srgb(77,73,70)  y3=srgb(13,11,10)
```

— identical in all three. The hairline is at row 2 and `RECESS` begins at row 3,
so the bar's 102 px height did not change when the popover opened.

## What the captures also settle

Two behaviours were driven rather than argued, because both depend on
`iced_widget` primitives the design spec verified by reading:

- **`opaque` really does capture presses inside its bounds.** The popover's ✕
  and its rows take clicks; the press that lands beside it does not reach the
  shelf ([`17`](17-up-next-click-outside.png)).
- **The popover really is not modal.** With it open, the transport below is
  uncovered and clickable, the shelf still scrolls, and every keyboard binding
  still fires — <kbd>Q</kbd> closed it in one run, <kbd>Esc</kbd> in another,
  and a press on the shelf in a third.

---

# Increment 7 — the queue's rows become controls

Where the popover earns its existence: ADR-0014 shipped `JumpTo` and
`UpdateQueue` and nothing spent them. Captured on the same private display,
with the same scratch `HOME`, scratch `XDG_*` and null ALSA sink as increment 6,
and with the transport **paused** wherever a measurement is being made.

| Image | What it shows |
|---|---|
| [`20-up-next-row-hover-remove.png`](20-up-next-row-hover-remove.png) | The pointer on row 9. The row lifts, and its ✕ appears — **only** its ✕: twelve permanent crosses would be twelve invitations to destroy something in a surface built for a glance. Every other row is exactly where it was, and so is every duration, because the slot the ✕ appears *in* is reserved whether or not it is in it. |
| [`21-up-next-row-removed.png`](21-up-next-row-removed.png) | The same queue one press later. *Beware the Friendly Stranger* is gone, the rows below it have renumbered, the summary has fallen from `1 of 12 · 50:25 left` to `1 of 11 · 48:14 left`, and the bar reads `1 / 11`. **The transport has not moved**: still paused, still 2:18 into *Ready Lets Go*. That is ADR-0014's guarantee — an edit that does not touch the playing track does not disturb one delivered sample — on camera. |
| [`22-up-next-jump-to-row.png`](22-up-next-jump-to-row.png) | A click on row 3. `3 of 11 · 37:26 left`, the dot on *In the Annexe*, rows 1–2 fallen to the history ink behind the cursor, and the bar naming it. The engine trace is `queue #0` then `queue #2` with nothing in between — one jump, not two skips. |

## The ✕ appears without anything moving

The popover was captured with the pointer off the window, then on row 9, then
off again. Paused throughout, so the pointer is the only variable.

```
pointer off  vs  pointer off again        0 differing pixels
pointer off  vs  row 9 hovered            bbox 317x25+17+315
```

A 25-pixel band in a 360 × 436 popover: row 9, and nothing else. The duration
column — the one most at risk, because it sits against the same right edge the
✕ arrives at — occupies the identical box in both:

```
pointer off      34x308+0+0
row 9 hovered    34x308+0+0
```

## Two notes on how it is built

**The edit is a whole list, computed by a pure function.** `queue_edit::without`
takes the `QueueVm` record and returns the queue as it should now be; the
payload sent is that value's own `paths()`. ADR-0014 rejected index deltas
because *an index applied against a stale picture removes a different track and
neither side can tell*, and working on the record rather than on a bare
`Vec<PathBuf>` means the list the engine is handed and the list on screen cannot
come apart. Ten unit tests, no iced.

**Hover is tracked with a row's identity, not a flag.** iced 0.13 tells a widget
its own hover status inside a style function and nothing about its siblings, so
each row reports its own crossings. Both messages are published from the same
`CursorMoved` in widget order — so dragging the pointer *up* a list delivers the
new row's entry before the old row's exit. An exit that meant "nothing is
hovered" would undo the entry that had just arrived; naming the row in the exit
makes the order stop mattering. Found by reasoning about the widget tree rather
than by watching a ✕ flicker, and the message's own doc comment says so.

---

# Increment 8 — Settings becomes a place, and the rail ceases to exist

The move completed. The settings leave the right-hand column for a place of
their own, the column's last roommate goes with them, and `panels.rs` — the
state machine that arbitrated three tenants — becomes `selection.rs`, which
answers one question with two fields.

| Image | What it shows |
|---|---|
| [`23-settings-place-1280.png`](23-settings-place-1280.png) | **The Settings place** at the shipped window. A section list on the left (`SETTINGS_NAV_W` 200), the form beside it capped at `SETTINGS_CONTENT_W` 640 and left-aligned, and a header carrying the way back. The ReplayGain content is the rail panel's, moved **verbatim** — the same segmented control, the same reserved note slot, the same `SETTING_VALUE_W` steppers, all of which were built for a 292 px column and are still fine at 640. And the now-playing bar is underneath, unchanged, still playing: **the bar is in every place**. |
| [`24-settings-place-960.png`](24-settings-place-960.png) | The same place below `SETTINGS_BREAKPOINT`. One column, the section name as a heading over its content. One branch, and it is the branch the album inspector will need at its own breakpoint. |
| [`25-library-restored-by-esc.png`](25-library-restored-by-esc.png) | <kbd>Esc</kbd> from the Settings, and the Library is **exactly as it was left** — same scroll offset, same selection, same inspector, same playing row, same paused position. That is what "a place is free to reverse" means: the Library's state lives in one struct that the Settings place does not touch. |
| [`26-library-1280-inspector.png`](26-library-1280-inspector.png) · [`27-library-960-inspector.png`](27-library-960-inspector.png) | The Library with the inspector at two window widths — three columns at 1280, two at 960. The column is the same 340 px in both, and it is the only thing that can be there now. |

## What the rail cost, and what replaced it

`panels.rs` carried three fields and a four-way question. `selection.rs`
carries two fields and a one-line answer:

```rust
pub fn inspecting(self) -> Option<u64> {
    if self.hidden { None } else { self.selected }
}
```

`showing_album` is now literally that expression compared to an id, which is
why the exhaustive walk over every path of four moves — *no reachable state
shows an inspector without an album* — got shorter rather than being deleted.
It is still walked, because "obviously true" is exactly what the rail's rule
looked like from the inside as well.

Two rules died with it, and both were symptoms rather than bugs:

- **Un-hiding an empty rail opened the queue.** A key whose entire job is to
  give the shelf its width back was creating a panel, because it had to invent
  content and the queue was the one tenant always meaningful to ask for.
  <kbd>Ctrl</kbd>+<kbd>B</kbd> now offers the album that is *playing* — a fact
  rather than an invention — and otherwise does nothing.
- **`Esc` had to choose between unrelated things.** It now peels one layer, top
  down: popover, then a place that is not home, then the search query, then the
  inspector. Four `if`s, in the order the layers stack.

## One thing the pixels caught that the code did not

The form's 640 px cap was first written as `container(...).max_width(640)`, and
the render shows what that produced: a segmented control **998 px** wide. A
`max_width` bounds the *limits* a child is laid out in, and a `Fill` child
inside a `Shrink` container resolves against what the row actually handed it.
The width is now arithmetic the view does from the window's own width, and the
measurement that caught it is in the function's doc comment so the next person
does not re-derive it.

---

# Visual language — implementation evidence, pass 1

> The three foundation items of `docs/design/02-visual-language.md`: the
> bundled typeface (§2.2), the two contrast corrections (§2.1.2), and the
> accent cut-back (§2.1.1). Everything here is the **real binary** rendered on a
> private headless display, not a mockup.

## How these were made

A private `Xvfb :91` at 1400×1000, a scratch `HOME` and scratch `XDG_*` dirs, and
a throwaway library of **18 albums / 141 tracks** — silent FLACs with generated
cover art in six visual idioms, two albums deliberately artless. Nothing touched
the maintainer's session, library or config.

The screenshots come from a `--features device-output` release build so the
now-playing bar is visible at all (without the feature `app.rs` hides it, which
is why `docs/design/visual/` has no bottom-bar screenshot). **No sound was
produced**: the scratch `HOME` carries an `.asoundrc` routing the default PCM to
`null`, `BAZ_DEVICE_TESTS` was never set, and the fixture files are digital
silence in the first place. The engine really ran — `engine ready (device opened
at 44100 Hz)` — and really moved the transport; it moved it into `/dev/null`.

Before and after are the same commit's parent and the branch, built the same way
in the same container, driven through the same script.

| File | What it shows |
|---|---|
| `00-first-run-before.png` / `-after.png` | the first-run question |
| `01-shelf-before.png` / `-after.png` | the shelf, nothing playing |
| `02-album-panel-before.png` / `-after.png` | the album panel |
| `03-shelf-playing-after.png` | the four permitted accent uses, all on screen at once |
| `04-bottom-bar-states-after.png` | the bar in six states, 102 px in every one |
| `05-bottom-bar-geometry-diff.png` | before-vs-after of the bar with nothing playing |

## 1. The typeface

`00-first-run-before.png` is the finding §1.1(1) opens with: `Font::DEFAULT` is
the generic `Family::SansSerif`, `SIZE_HERO` asks it for Semibold, and the
platform's fallback chain answers with a **monospace** — the product's one line
of copy, *Where's your music?*, set in a typewriter face. `01-shelf-before.png`
is the same defect fifteen times over: every tile title monospaced, every artist
line beneath it proportional.

`-after.png` is IBM Plex Sans at a real Medium and a real SemiBold, with IBM
Plex Mono on the counts line, the durations and the signal note. The two faces
are drawn together, which is the point: `18 albums · 141 tracks` beside
`Étude for Empty Halls` now reads as one typeface with two settings.

## 2. The contrast corrections

Look at `05-bottom-bar-geometry-diff.png`: the volume rail is one of only two
things that changed. That rail is `PAPER_FAINT`, which moved from `#726D66`
(3.4 : 1 on the panel — below the AA floor, and it carries every duration,
count, hint and signal note in the product) to `#8A857C` (4.5–5.4 : 1).

The muted fader in the last row of `04-bottom-bar-states-after.png` is
`PAPER_MUTED`, which moved from `#4A4743` (**1.9 : 1** — below even the 3 : 1
non-text floor, i.e. the position the listener chose was effectively invisible
while muted, which is the one thing mute exists to preserve) to `#6E6A62`
(3.1–3.6 : 1). It is still plainly quieter than the live fader two rows above
it, which is the other half of the requirement.

`theme.rs`'s `every_ink_clears_its_contrast_floor_on_every_surface_it_lands_on`
computes every ink-on-surface pairing the room can produce, and is what stops
either drifting back.

## 3. The accent cut-back

In `01-shelf-before.png` the brightest thing on screen is an **amber ring around
the search field** — the field takes focus at launch, so the first frame baz
ever drew was a lit lamp with no music playing. In `00-first-run-before.png` the
wordmark is amber for the same non-reason. Both are paper in the `-after`
shots: `PAPER_RING` on the focus ring, `PAPER_FAINT` on the wordmark. The
scanning note moved from mono `LAMP` to sans `PAPER_DIM` (visible during a scan
rather than in a resting shot).

`03-shelf-playing-after.png` is what the accent is *for*, and it is the whole of
what it is for: the halo on the playing sleeve (twice — tile and panel), the
seek groove's elapsed fill and knob, and the Play album button. Nothing else on
that screen is amber.

## 4. Do the reserved slots still hold?

The risk §4.6 names: a different face has different figure widths, and every
fixed slot in the pixel-stable bar was sized against the old one. Measured off
the rendered pixels — ink extent inside each slot, against the slot's width:

```
state               slot                   before ink  after ink   slot   verdict
01-playing          STAMP_W elapsed                28         27     52   holds (25 px spare)
01-playing          STAMP_W total                  50         50     52   holds (2 px spare)
01-playing          SIGNAL_W                       78         79    120   holds (41 px spare)
02-seek-hover       STAMP_W elapsed                36         36     52   holds (16 px spare)
02-seek-hover       PREVIEW_W seek tip             58         58     58   the tip chip is PREVIEW_W by construction
03-paused           STAMP_W total                  50         50     52   holds (2 px spare)
04-volume-hover     LEVEL_W volume tip             62         62     62   the tip chip is LEVEL_W by construction
05-muted            STAMP_W elapsed                36         36     52   holds (16 px spare)
```

The `STAMP_W total` row is the worst case in the product and it is real, not
simulated: one fixture album's tracks run past the hour, so the total reads
`1:01:22` — seven monospace figures, the `h:mm:ss` shape the token exists for.
Fifty pixels of ink in a fifty-two pixel slot.

**And the geometry did not move.** `05-bottom-bar-geometry-diff.png` is the bar
with nothing playing, before against after: **928 differing pixels out of
131 840 (0.70%)**, and every one of them is ink — the two words *Nothing
playing* re-set in the new face, and the volume rail's corrected value. The
transport buttons, the hairline rule, the mute target, the fader's knob and the
bar's 102 px height are pixel-identical.

One honest note on the risk itself. The old build's monospace resolved to
Liberation Mono, which advances 0.6 em like Plex Mono does, so the *rendered*
widths barely moved (78 → 79 px on the signal note). What was actually wrong was
the arithmetic in `theme.rs`'s assertions, which checked the slots against a
0.5 em guess and therefore claimed 20% more headroom than the slots ever had.
That guess is now `MONO_EM = 0.6`, and the claim it stands for is measured
string by string against the shipped bytes in
`crate::font`'s `every_reserved_slot_holds_its_worst_case_in_the_bundled_face`.

## What is deliberately absent

The shadow and halo changes, the flexible shelf grid, the art-hue lamp, the
quieter gradient placeholder with its letterform, the caption year drop, and
`SIZE_TITLE` / `SIZE_HERO` with the serif are all specified and none of them are
in this pass — which is why the shelf here still has its 240 px centred grid,
its dead gutters and its stock scrollbar. `theme::SERIF` is parked at the end of
`theme.rs` with the face bundled, ready for the surface that spends it.
