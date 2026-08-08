# Steps 9 and 10 — the needle, and the bar at 58 px

Rendered evidence and measurements for
[ADR-0017](../../../adr/0017-design-direction.md) §1.1 and its build plan's
steps 9 and 10: a 2 px seek line flush on the window's bottom edge, segmented by
the queue's real entry lengths, and a now-playing bar re-laid around its
absence of a seek row.

## How these were made

The real release build, `--all-features` (the transport only exists with
`device-output`), rendered on a private `Xvfb :181` with **all six**
redirections from [`docs/DEVELOPMENT.md`](../../../DEVELOPMENT.md#headless-ui-verification).
The script is [`capture.sh`](capture.sh) beside these images. Every run's log
carries the receipt:

```
[mpris] no session bus; desktop media controls unavailable (I/O error: No such file or directory (os error 2))
```

The fixture is the composition audit's own: a throwaway 25-album / 206-track
library of **digitally silent** FLAC (every sample a zero), never `~/Music`,
built by [`../../composition/tools/mkfixture.sh`](../../composition/tools/mkfixture.sh).
The scratch `HOME` carries an `.asoundrc` routing ALSA's default PCM to `null`,
so the transport is real and nothing was audible; `BAZ_DEVICE_TESTS` was never
set. Album 7 — *Closing Time* — opens with **one hour** of silence, which is
what holds a stable playing state while the frames are taken (the null sink
free-runs at ~90×, which is also why the elapsed stamp advances between shots).

The ruler is the committed one,
[`../../composition/tools/ruler.py`](../../composition/tools/ruler.py): a
standard-library PNG decoder, so every number below is read off a committed
image rather than asserted.

## The frames

| | |
|---|---|
| [`01-bar-stopped`](01-bar-stopped-1280x860.png) | Nothing playing. The needle draws its **unfilled track, whole** — a line that came and went with the music would be movement in the one place ADR-0020 forbids it. |
| [`02-bar-playing`](02-bar-playing-1280x860.png) | *Closing Time*, 9 tracks queued. Nine segments, eight 2 px track gaps, the fill running into the first. |
| [`03-needle-segment-hovered`](03-needle-segment-hovered-1280x860.png) | The pointer on a segment that is **not** sounding: a click there jumps, so the tip names the record — `Marginalia 5`. |
| [`04-needle-playing-entry-hovered`](04-needle-playing-entry-hovered-1280x860.png) | The pointer **inside** the sounding entry: a click there seeks, so the tip is a timestamp — `7:26`. |
| [`05-bar-paused`](05-bar-paused-1280x860.png) | Paused. Every edge in the bar and every segment boundary is identical to `02`; only the fill and the glyph moved. |
| [`06-wall-playing`](06-wall-playing-1280x860.png) | The wall the whole thing is for. |
| [`07-what-the-wall-gained`](07-what-the-wall-gained-1280x860.png) | The same 180 px strip of the same frame, before (top) and after (bottom). The before is the committed [`../composition/shots/wall-playing-1280x860.png`](../composition/shots/wall-playing-1280x860.png) — the bar as the composition audit left it. |

## The bar's arithmetic, measured

Read off `02-bar-playing-1280x860.png`, bottom-up:

| rows | what | px |
|---|---|---:|
| 801 | the hairline, `#1b1c1c` | 1 |
| 802–857 | the band, `RECESS` `#060708` | **56** |
| 858–859 | the needle | **2** |
| | **the window's bottom edge, total** | **59** |

Before, from the same fixture and the same window: **105** (1 + 104). The
collection gets **46 px** back, and its share of an 860 px window goes from
`(860 − 49 − 105)/860` = **82.1 %** to `(860 − 49 − 59)/860` = **87.4 %**.

### The one-pixel correction to ADR-0017

The ADR wrote this column as `1 rule + 12 + 32 + 12` and totalled it **58**.
The parts are right and the total is one out — 1 + 12 + 32 + 12 = **57** — and
58 is not reachable at all: a bar is `2a + 2ℓ + TRANSPORT_HIT + 1` for a
symmetric padding `a` and lead `ℓ`, which is **odd** for every integer pair,
because the hairline is odd and everything else is doubled. So the parts are
held and the total is corrected. Two consequences, both in our favour and both
stated rather than quietly banked:

- the ADR predicted "recover 44" against a 102 px bar it inherited from before
  the composition audit re-derived the band; measured against the 105 that is
  actually there, the wall gets **46**;
- the ADR conceded "28 px against the critique's ~32 px of bottom furniture";
  ours is 59, so the concession is **27**.

## Law L4 — one centre line per bar, at 58 px

The same instrument [`06-composition-audit.md`](../../06-composition-audit.md)
§5.1 used: the contrast-weighted ink centre of each zone's primary mark, against
the band's own mid-line. The band is 802–857, so the mid-line is **830.0**.

| mark | ink rows | ink centre | Δ |
|---|---|---:|---:|
| Previous glyph | 824–835 | 830.00 | **0.00** |
| Play/Pause glyph | 824–835 | 830.00 | **0.00** |
| Next glyph | 824–835 | 830.00 | **0.00** |
| mute glyph | 824–835 | 830.00 | **0.00** |
| volume rail (rail only) | 828–831 | 830.00 | **0.00** |
| `Queue` readout | 827–834 | 830.44 | +0.44 |
| elapsed stamp | 827–834 | 831.03 | +1.03 |
| total stamp | 827–834 | 831.08 | +1.08 |
| left zone, middle lane (artist) | 826–834 | 831.45 | +1.45 |
| signal note | 826–836 | 831.53 | +1.53 |
| `Queue` label | 827–836 | 831.93 | +1.93 |

**Spread 1.93 px**, against the law's 2 px ceiling and the audit's measured
**50 px** across seven lines in a 102 px band. The bar's own mid-line is not
merely one of them — five marks are on it *exactly*.

The five zeroes are the structural claims, and they are structural rather than
nudged: the band is `2 × BAR_LEAD + TRANSPORT_HIT`, so its mid-line **is** the
transport's centre; the volume block is one control height with the fader's hit
band centred in it, so centring the block centres the rail. The sub-2 px
residues are the ink's own mass distribution inside a line box whose *geometric*
centre is 830.0 exactly — lowercase type at 12 px carries its mass below the
box's centre, and the audit's own table shows the same signature (its `Queue`
label read +1.5 for the same reason).

## Law L1 — one gutter, and what the needle does about it

| | left edge | right edge |
|---|---:|---:|
| the bar's content | **40** | **1240** |
| the needle | 0 | 1280 |

The bar hangs from `HANG` on both sides, as every window-edge surface does. The
needle does not, and that is a decision rather than an oversight: it is a
**rule**, and every rule in baz spans its container edge to edge — the bar's own
hairline at `y = 801` runs 0–1280 in the same frame. L1 governs content lanes;
a 2 px line whose meaning *is* the window's bottom edge has no lane. Inset to
`HANG` it would also stop being a screen-edge target, which is half of what
makes a 2 px control aimable at all.

## Law L7 — one control height, and the needle's named third

| control | drawn height |
|---|---:|
| Previous · Play/Pause · Next | 32 |
| mute | 32 |
| `Queue` | 32 |
| the volume fader's hit band (`RAIL_HIT` family) | 28 in a 32 block |
| **the needle's aiming band** | **12** |

The needle is a third pointer height and it is named rather than smuggled.
ADR-0017 specified `NEEDLE_HIT` 22; 22 would reach 8 px into the transport
row's boxes, so it is amended to 12 — `GAP_MD`, on the 4 px lattice, and
**exactly the bar's bottom lead**, which is empty recess. The bound is the
safety property and it is asserted in code rather than argued here:
`NEEDLE_HIT <= BAR_LEAD`, so a press aimed at Next can never be taken by a line
at the bottom of the window. The band is claimed *upward and out of layout*,
which is the only way a 2 px control can be aimed at without charging the
collection for the aiming.

## The needle's geometry, measured

From `02-bar-playing-1280x860.png`, row 858 — every transition along the line:

```
0:#e3a14e  70:#161617  726:#0c0d0e 728:#161617  787:#0c0d0e 789:#161617
886:#0c0d0e 888:#161617  933:#0c0d0e 935:#161617  1016:#0c0d0e 1018:#161617
1049:#0c0d0e 1051:#161617  1118:#0c0d0e 1120:#161617  1226:#0c0d0e 1228:#161617
```

Nine segments, eight gaps, each exactly **2 px** (`SEGMENT_GAP`). The fill is
`#e3a14e` — the lamp — and the unfilled track is `#161617`, the room's hairline
over `RECESS`, which ADR-0017 §1.6's exemption list named as a mark that is
located and never read *before it existed*.

The first segment runs 0–726. *Closing Time* is 1:42:00 and its opener is
1:00:00, so it is 58.8 % of the music; with 8 × 2 px of gaps and 9 × 4 px of
floor taken out first, `3600/6120 × 1228 + 4` = **726**. The fill reaches 70 px
at 5:44 of 60:00 — `344/3600 × 726` = **69.4**. Both agree with
`player::needle_spans` to the pixel, because the widget and the hit test call
the same function.

## What is *not* here, and why

**The album-boundary gap is not in a screenshot.** `ALBUM_GAP` is real, tested
and drawn — but baz cannot yet *produce* a queue holding two records: the only
thing that queues anything is "play this album" (`app::play_album` →
`vm::album_queue`), and shift-click-to-stack is step **13** of ADR-0017's plan.
Faking a frame for it would be exactly the dishonesty the rest of this file is
written against, so the boundary is proven where it can be:

- `player::tests::a_record_ending_is_a_wider_gap_than_a_track_ending` — the gap
  at a record's end is `ALBUM_GAP` and every other gap is `SEGMENT_GAP`, with
  the four equal entries still equal (the wider gap comes out of the line, not
  out of a segment);
- `player::tests::album_boundaries_come_from_the_queue_the_engine_was_sent` —
  the boundaries are computed from the queue the view model built, not from a
  flag a test set by hand.

The frame lands with step 13.

## Reproducing this

```sh
bash docs/design/composition/tools/mkfixture.sh /tmp/baz-comp-fixture
toolbox run -c baz-dev cargo build --release --all-features
toolbox run -c baz-dev bash docs/design/impl/needle/capture.sh
```

Nothing was left running: the script kills its own app and its own `Xvfb`, and
`ps` was checked afterwards.
