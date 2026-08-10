# The artwork at size — frames of what step A2 shipped

The owner, looking at the merged Now playing surface: *"also fullscreen the now
playing looks weird"*.

He was right, and the cause was already written down. `views/now_playing.rs`
clamped the artwork at `NOW_PLAYING_MAX` **720**, so at 2560 × 1440 a 2280 px
body held a 720 px square and 1560 px of `#0C0D0E`. Step A2
([`docs/design/12-now-playing-and-kiosk.md`](../../12-now-playing-and-kiosk.md)
§5.2, §5.3, §12; [ADR-0029](../../../adr/0029-the-ambient-surface.md) §2)
deletes the constant, gives the surface its own decode tier, and fills what the
artwork honestly cannot with **the record's own colours**.

Rendered by [`capture.sh`](capture.sh) against **two** release binaries — the
commit A2 landed on, and this branch — headless on a private Xvfb, with all six
XDG redirections from `docs/DEVELOPMENT.md` §"Headless UI verification".
Nothing touched the owner's session and nothing was audible: the scratch `HOME`
routes ALSA's default PCM to null and every fixture sample is a zero. The
receipt the run printed:

```
[mpris] no session bus; desktop media controls unavailable (I/O error: No such file or directory (os error 2))
```

**The fixture's covers are re-drawn at 1400 px** (`mkfixture.sh` ships 600).
600 is under the hero tier's own ceiling, and the whole subject of this step is
what happens when the source is the binding term and when it is not — so the
fixture has to be able to be both. A second pass re-draws them at 300 px for
story S7, and a third at flat grey for the field's fallback.

## The frames

| | What it shows |
|---|---|
| `12-{before,after}-1280x860-{run-on,run-off}.png` | The desktop case. The record is **width**-bound with the run standing, so A2 changes its size not at all — what changes is the ground it stands on |
| `19-{before,after}-1920x1080-{run-on,run-off}.png` | **1080p.** 720 → 773. Not the headline number, and the headline is the field |
| `25-{before,after}-2560x1440-{run-on,run-off}.png` | **The complaint.** 720 → **1024**, source-bound, and a room that is no longer an empty one |
| `30-{before,after}-restacked-1000x800.png` | Below `SPLIT_FLOOR`: a 720 px body, the record re-hung as the run's head block, and the **whole** field clamped to `wall` because the whole body is the list |
| `40-{before,after}-small-source-1920x1080-*.png` | **Story S7.** A collection ripped with 300 px covers. Before: upscaled to 720 from a 320 px thumbnail. After: drawn at 300, centred, on a field |
| `41-after-monochrome-1920x1080-*.png` | **The control.** Every cover neutral grey. No hue over the presence floor, so there is no field and the room shows through — measured below as `#0C0D0E` exactly |

## What the pixels say

[`measure.py`](measure.py) reads both load-bearing figures off the committed
PNGs. Nothing below is computed from tokens alone.

### The artwork's edge

The sleeve is found as the block that is **not the ground** — a pixel is
artwork when it leaves the field's lightness band in either direction, or
carries more chroma than the field's pinned 0.024. Chroma alone would miss a
`mono` sleeve, which is less saturated than the field it hangs on.

| Window | Density | Before | After | | Bound by, after |
|---|---|---|---|---|---|
| 1280 × 860, lane open | run on | 456 | **456** | ±0 | width — `1000 − 2·HANG − (440 + GAP_XL)` |
| 1280 × 860, lane open | run off | 568 | **552** | −16 | height — and the −16 is the `below` correction, not the clamp |
| 1920 × 1080, lane open | run on | 720 | **773** | +53 | height — `999 − 2·HANG − below 146` |
| 1920 × 1080, lane open | run off | 720 | **772** | +52 | height (one px of antialiasing off the 773) |
| **2560 × 1440**, lane open | run on | 720 | **1024** | **+304** | **the source** — `HERO_PX`, the decode's own ceiling |
| **2560 × 1440**, lane open | run off | 720 | **1024** | **+304** | **the source** |
| 1920 × 1080, **300 px covers** | either | 720 | **300** | −420 | **the source** — and the 720 was a 2.25× upscale of a 320 px thumbnail |
| 1000 × 800 (below `SPLIT_FLOOR`) | — | 240 | **240** | ±0 | `ART_MIN`, as the run's head block |

Three readings, and all three are the step working rather than three separate
results:

- **At 1280 nothing moves**, because the record is width-bound there and the
  clamp was never what was binding it. The frames at that size are about the
  field and nothing else.
- **At 2560 the source becomes the binding term**, which is doc 12 §5.5's
  *"the screen rewards a well-kept collection"* arriving. A 1400 px cover
  decodes to `HERO_PX` 1024 and is drawn at 1024; the same window with 300 px
  covers draws 300.
- **The 300 px row is the one that shows what the old number was.** Before, a
  300 px file was drawn at 720 — from a **320 px thumbnail**, so 2.25× of
  invented pixels. The refusal *no artwork is ever drawn larger than its
  source* was false there, and it is now arithmetic.

### The field's colours, sampled

Named bare rectangles, not a sweep: the field is *under* everything, so a sweep
of the body samples the run's rows, the playing row's ground, the needle and
the type as well. Each rectangle is stated in [`measure.py`](measure.py), and a
patch inside one counts as bare field only when its own per-channel range is no
more than the dither's **and** it carries the field's own chroma — the second
condition is what excludes the playing row's `plinth_lit`, which is the *room's*
plane drawn over the field rather than the field itself.

Figures are **9 × 9 patch means**. iced dithers its gradients — measured at
**7/255 within a channel**, about 0.012 oklch L — which is exactly what doc 12
§5.3 asks for when it requires the wash to be *continuous* at these
lightnesses. Dither is zero-mean, so a mean is the right instrument and the
spread is reported rather than mistaken for signal.

| Frame | Region | n | L (patch means) | C | hue | Against |
|---|---|---|---|---|---|---|
| `12-after-…-run-off` | ambient | 125 | 0.188–0.221 | 0.024–0.025 | 127° | ceiling 0.220 ✅ |
| `12-after-…-run-on` | ambient | 54 | 0.156–0.184 | 0.023–0.024 | 100° | ceiling 0.220 ✅ |
| `19-after-…-run-off` | ambient | 160 | 0.194–0.221 | 0.024–0.025 | 127° | ceiling 0.220 ✅ |
| `19-after-…-run-on` | ambient | 42 | 0.156–0.189 | 0.023–0.024 | 103° | ceiling 0.220 ✅ |
| `19-after-…-run-on` | **under the run** | 16 | **0.156–0.159** | 0.022–0.024 | 97° | `wall` 0.158 ✅ |
| `25-after-…-run-off` | ambient | 230 | 0.193–0.221 | 0.024–0.025 | 127° | ceiling 0.220 ✅ |
| `25-after-…-run-on` | ambient | 1196 | 0.155–0.205 | 0.022–0.025 | 107° | ceiling 0.220 ✅ |
| `25-after-…-run-on` | **under the run** | 208 | **0.155–0.160** | 0.022–0.024 | 103° | `wall` 0.158 ✅ |
| `30-after-restacked` | **under the run** | 102 | **0.154–0.161** | 0.022–0.024 | 104° | `wall` 0.158 ✅ |
| `41-after-monochrome` | ambient | 160 | **0.158–0.158** | **0.003–0.003** | **248°** | **the room, exactly** |

The room, for comparison: `#0C0D0E` is **L 0.158, C 0.003, hue 248°**.

Five things that table settles, each of which was a claim in doc 12 §5.3–§5.4
before it was a number:

1. **The ceiling holds.** No patch of ambient field exceeds **L 0.220** in any
   frame at any size. The artwork stays the brightest object on the surface by
   construction rather than by luck.
2. **The floor holds.** No patch falls under the room's own `wall` L 0.158 —
   the field is a *change* to the room, never a darkening of it.
3. **Under the run column the field is flat at `wall`**, in all three frames
   that have one: 0.155–0.161 against 0.158, which is the dither's own residual.
   That is §5.4 term 2 as the one-line test it promised to be — *the run
   column's ground is never lighter than `room.wall`* — and it introduces no
   new contrast number, because `wall` is the ground every other list in this
   product is read over.
4. **The chroma is pinned.** 0.022–0.025 everywhere, against `field::CHROMA`
   **0.024**. The record supplies a hue and nothing else, which is
   `Palette::lamp`'s own rule at a larger size.
5. **The hue is the record's.** 97°–127° across the frames — the fixture's
   *Ochre* sleeve is olive and yellow-green, and that is where those hues sit
   in oklch. The monochrome control reads **248°**, which is not a derived hue
   at all: it is `#0C0D0E`'s own, because there is no field.

### What a reader should look at rather than measure

- **`25-before` against `25-after`.** The before frame is a 720 px square in a
  2280 px body with 1560 px of flat near-black around it. That is the owner's
  sentence, rendered.
- **`40-after-small-source-1920x1080-run-off`.** A 300 px sleeve, drawn at 300,
  centred on a field. It is *small*, honestly, and the composition is still
  composed — which is the whole argument for deriving a field rather than
  enlarging a picture.
- **`41-after-monochrome`.** No hue, no field, the room. The fallback is not a
  grey wash pretending to be derived from something.

## One thing the frames show that A2 does not fix

At **2560 × 1440 with the run standing** the record column is 1800 px wide and
the work is 1024 of it, left-hung, so about 700 px of field sits between the
sleeve and the run's column. That is doc 12 §5.5a's left-hang working as
written — the work and its placard share a left edge and hang off the body's
own `HANG` — and it is **step A4's** to close, not this one's: A4 scales
`RUN_MEASURE` by `kiosk_scale`, taking the run from 440 to about 1100 at this
size, which is most of that gap. Recorded here because it is visible in a
committed frame and a reader should not have to wonder whether anybody saw it.

## Reproducing

```
# the two binaries
toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
  cargo build --release -p baz --features device-output
cp target/tb/release/baz /tmp/baz-after
git checkout HEAD~1 -- crates/baz
toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
  cargo build --release -p baz --features device-output
cp target/tb/release/baz /tmp/baz-before
git checkout HEAD -- crates/baz

# the fixture, the frames, the numbers
toolbox run -c baz-dev docs/design/composition/tools/mkfixture.sh /tmp/baz-hero-fix
toolbox run -c baz-dev docs/design/impl/artwork-at-size/capture.sh
python3 docs/design/impl/artwork-at-size/measure.py
```

The fixture is silent FLAC with generated covers, so the frames are of the real
binary drawing real decoded artwork, real durations and a field derived from
real pixels — the sleeves are the fixture's own covers, not a stand-in the
place drew.
