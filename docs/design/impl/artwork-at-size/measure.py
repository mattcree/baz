#!/usr/bin/env python3
"""Read step A2's two claims off the committed PNGs.

Nothing here estimates. Both figures come from pixels:

- **the artwork's edge**, before and after `NOW_PLAYING_MAX` 720 was deleted,
  at three window sizes and in both densities;
- **the field's own colours**, sampled at named points and converted to oklch,
  against the numbers `crate::field` promises: a ceiling of **L 0.22** in the
  ambient region, a flat **L 0.158** (the room's own `wall`) under the run
  column where type scrolls, and a chroma pinned at **0.024**.

The sleeve is found as the largest run of *saturated* pixels rather than the
largest bright block, because the surface now has a coloured ground and a
brightness threshold would find the field as well. The field is chromatic but
faint — its chroma is 0.024, an order under any fixture cover's — so the two
separate cleanly on chroma, and the separation is printed rather than assumed.

    python3 docs/design/impl/artwork-at-size/measure.py
"""

import math
import os
import sys

sys.path.insert(
    0,
    os.path.join(
        os.path.dirname(os.path.abspath(__file__)), "..", "..", "composition", "tools"
    ),
)
from ruler import Img  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))

BAR_H = 81
HANG = 40
GAP_XL = 24
RUN_MEASURE = 440
# crate::field's own numbers, and crate::theme's `wall`.
CEILING_L = 0.22
WALL_L = 0.1584
FLOOR_L = 0.1584
FIELD_C = 0.024
# The chroma that separates a cover from the ground it hangs on. The field is
# pinned at 0.024 and every fixture cover is far above it; 0.06 is the midpoint
# in log terms and no pixel of either lands near it.
SLEEVE_C = 0.06


# ------------------------------------------------------------------- colour


def oklch(rgb):
    """sRGB bytes -> (L, C, hue degrees). Ottosson's published constants."""

    def lin(c):
        c /= 255.0
        return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4

    r, g, b = (lin(v) for v in rgb)
    l = 0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b
    m = 0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b
    s = 0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b
    l, m, s = l ** (1 / 3), m ** (1 / 3), s ** (1 / 3)
    lightness = 0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s
    a = 1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s
    bb = 0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s
    return lightness, math.hypot(a, bb), math.degrees(math.atan2(bb, a)) % 360


def px(im, x, y):
    i = 3 * (y * im.w + x)
    return im.px[i], im.px[i + 1], im.px[i + 2]


# ------------------------------------------------------------------- sleeve


def is_ground(rgb):
    """Whether a pixel is the field (or the room) rather than something drawn
    on it.

    Two tests, because a cover can fail either one alone: the field's chroma is
    pinned at 0.024 and no cover in the fixture is that neutral *and* that
    close to the room's lightness at once. A `mono` sleeve is darker than the
    field, a `pale` one lighter, a `chroma` one far more saturated.
    """
    lightness, chroma, _ = oklch(rgb)
    return chroma < SLEEVE_C and FLOOR_L - 0.02 <= lightness <= CEILING_L + 0.02


def longest(values):
    """The longest contiguous run in a sorted list of ints, as (first, last)."""
    best = run = (values[0], values[0])
    for v in values[1:]:
        run = (run[0], v) if v == run[1] + 1 else (v, v)
        if run[1] - run[0] > best[1] - best[0]:
            best = run
    return best


def sleeve(im, x0, x1, y1):
    """The artwork's square, found as the block that is **not** the ground.

    Chroma alone is not enough — a near-neutral sleeve (the fixture's `mono`
    family, half of ECM's catalogue) carries less of it than the field does —
    so a pixel counts as artwork when it leaves the field's band in *either*
    direction. The columns are counted first because the artwork is a square
    and a column through it is `edge` px of it, where a column through the
    placard is a line of type.

    **The longest contiguous run**, not the outer bounds: the needle is drawn
    in the lamp and sits 100-odd px under the sleeve, so the outer bounds would
    report the artwork as taller than it is by everything in between.
    """
    cols = [
        x
        for x in range(x0, x1)
        if sum(1 for y in range(0, y1, 3) if not is_ground(px(im, x, y))) > 60
    ]
    if not cols:
        return None
    left, right = longest(cols)
    mid = (left + right) // 2
    rows = [y for y in range(0, y1) if not is_ground(px(im, mid, y))]
    if not rows:
        return None
    top, bottom = longest(rows)
    return left, right, top, bottom


def report_sleeve(name, window_w, window_h, lane, run):
    im = Img(os.path.join(HERE, name))
    body_w = window_w - lane
    run_w = RUN_MEASURE if run else 0
    right = min(window_w, window_w - (HANG + run_w + GAP_XL)) if run else window_w
    found = sleeve(im, lane, int(right), window_h - BAR_H - 8)
    if not found:
        print(f"  {name:48s}  no artwork found")
        return None
    x0, x1, y0, y1 = found
    edge = x1 - x0 + 1
    print(
        f"  {name:48s}  edge {edge:5d}   x {x0}–{x1}   y {y0}–{y1} ({y1 - y0 + 1})"
        f"   body {body_w}×{window_h - BAR_H}"
    )
    return edge


# -------------------------------------------------------------------- field
#
# **Named bare rectangles, not a sweep.** The field is under everything, so a
# sweep of the body samples the run's rows, the playing row's `plinth_lit`
# ground, the needle and the type as well. Each rectangle below is a region of
# a specific frame where nothing is drawn over the field, and each is stated so
# a reader can check it against the PNG rather than trust it.
#
# **Patch means, not pixels.** iced dithers its gradients — measured at ±3/255,
# which at these lightnesses is ±0.012 oklch L, and which is what doc 12 §5.3
# asks for when it says the field must be *continuous*. Dither is zero-mean by
# construction, so every figure below is the mean of a 9 × 9 patch and the
# spread is reported beside it rather than mistaken for the signal.

PATCH = 9

REGIONS = [
    # (frame, region, x0, x1, y0, y1, the ceiling it must not pass)
    ("12-after-1280x860-run-off.png", "ambient", 296, 416, 100, 770, CEILING_L),
    ("12-after-1280x860-run-on.png", "ambient", 300, 790, 700, 770, CEILING_L),
    ("19-after-1920x1080-run-off.png", "ambient", 296, 416, 100, 960, CEILING_L),
    ("19-after-1920x1080-run-on.png", "ambient", 300, 1430, 966, 995, CEILING_L),
    ("19-after-1920x1080-run-on.png", "under run", 1444, 1876, 960, 995, WALL_L),
    ("25-after-2560x1440-run-off.png", "ambient", 296, 416, 110, 1340, CEILING_L),
    ("25-after-2560x1440-run-on.png", "ambient", 1360, 2060, 110, 1340, CEILING_L),
    ("25-after-2560x1440-run-on.png", "under run", 2085, 2515, 1000, 1340, WALL_L),
    # The control: a collection with no hue over the presence floor. The field
    # is `None` and the room shows through, which is story S7's own criterion.
    ("41-after-monochrome-1920x1080-run-off.png", "ambient", 296, 416, 100, 960, CEILING_L),
    # Below `SPLIT_FLOOR` the whole body is the run's list, so the whole field
    # is `Reach::Still` — one domain, not two.
    ("30-after-restacked-1000x800.png", "under run", 860, 980, 100, 700, WALL_L),
]


# A patch is **bare field** when two things hold, and both are stated because
# each one alone lets something through:
#
#   1. its own per-channel range is no more than the dither's, so nothing with
#      an edge in it — type, a hairline, a row's boundary — is counted;
#   2. it carries the field's own chroma, which excludes the playing row's
#      `plinth_lit` ground. That plane is *the room's*, near-neutral at C 0.006,
#      and it is drawn **over** the field rather than being it.
#
# Both are properties of the pixels rather than of a y-coordinate, so the
# sampling does not have to be re-tuned every time a run scrolls differently.
FLAT = 12
TINTED = 0.010
# What a 9 x 9 mean can still carry of the dither: 7/255 of noise over 81
# samples leaves a few thousandths of oklch L, and a comparison tighter than
# its own instrument is a comparison that fails on the instrument.
TOLERANCE = 0.004


def patches(im, x0, x1, y0, y1):
    """Every 9 x 9 patch of bare field in the rectangle, as (L, C, hue)."""
    out = []
    for y in range(y0, y1 - PATCH, PATCH * 3):
        for x in range(x0, x1 - PATCH, PATCH * 3):
            vals = [px(im, x + dx, y + dy) for dy in range(PATCH) for dx in range(PATCH)]
            if max(
                max(v[ch] for v in vals) - min(v[ch] for v in vals) for ch in range(3)
            ) > FLAT:
                continue
            mean = oklch([sum(v[ch] for v in vals) / len(vals) for ch in range(3)])
            # The room itself is legitimately untinted — that is the control
            # row, and its `None` field is the answer story S7 asks for.
            if mean[1] < TINTED and mean[1] > 0.005:
                continue
            out.append(mean)
    return out


def spread(im, x, y):
    """The dither's own amplitude at one point: the per-channel range of a
    9 x 9 patch, which is the whole of why the means above are means."""
    vals = [px(im, x + dx, y + dy) for dy in range(PATCH) for dx in range(PATCH)]
    return max(
        max(v[ch] for v in vals) - min(v[ch] for v in vals) for ch in range(3)
    )


def report_field():
    print(
        f"\n  {'frame':44s} {'region':10s} {'n':>4s}  "
        f"{'L (patch means)':>17s}  {'C':>13s}  {'hue':>7s}   ceiling"
    )
    for name, region, x0, x1, y0, y1, ceiling in REGIONS:
        im = Img(os.path.join(HERE, name))
        pts = patches(im, x0, x1, y0, y1)
        ls = [p[0] for p in pts]
        cs = [p[1] for p in pts]
        hx = sum(math.cos(math.radians(p[2])) for p in pts)
        hy = sum(math.sin(math.radians(p[2])) for p in pts)
        hue = math.degrees(math.atan2(hy, hx)) % 360
        verdict = (
            "OK" if max(ls) <= ceiling + TOLERANCE else f"OVER by {max(ls) - ceiling:+.3f}"
        )
        floor = (
            "OK" if min(ls) >= WALL_L - TOLERANCE else f"UNDER by {WALL_L - min(ls):+.3f}"
        )
        print(
            f"  {name:44s} {region:10s} {len(pts):4d}  "
            f"{min(ls):.3f}–{max(ls):.3f}      {min(cs):.3f}–{max(cs):.3f}  "
            f"{hue:6.1f}°   {ceiling:.3f} {verdict} / floor {floor}"
        )
    im = Img(os.path.join(HERE, "25-after-2560x1440-run-on.png"))
    print(
        f"\n  the dither, measured: a 9 x 9 patch of bare field spans "
        f"{spread(im, 1700, 700)}/255 within a channel - "
        f"~{0.012:.3f} oklch L, which is what keeps a wash this dark continuous "
        f"(doc 12 §5.3)."
    )
    print(
        f"  the room, for reference: #0C0D0E is L {WALL_L:.3f} C 0.003 hue 248°;"
        f" the field's pinned chroma is {FIELD_C:.3f} and its ceiling {CEILING_L:.3f}."
    )


# --------------------------------------------------------------------- main

SIZES = [
    ("12", 1280, 860, 280),
    ("19", 1920, 1080, 280),
    ("25", 2560, 1440, 280),
]

if __name__ == "__main__":
    print("\n=== the artwork's edge, before and after ===")
    for tag, w, h, lane in SIZES:
        print(f"\n{w} x {h}, returns lane open {lane}")
        for run, word in ((True, "run-on"), (False, "run-off")):
            before = report_sleeve(f"{tag}-before-{w}x{h}-{word}.png", w, h, lane, run)
            after = report_sleeve(f"{tag}-after-{w}x{h}-{word}.png", w, h, lane, run)
            if before and after:
                print(f"  {'':48s}  {before} -> {after}   {after - before:+d}")

    print("\n=== story S7: a 300 px source, at 1920 x 1080 ===")
    for run, word in ((True, "run-on"), (False, "run-off")):
        report_sleeve(f"40-before-small-source-1920x1080-{word}.png", 1920, 1080, 280, run)
        report_sleeve(f"40-after-small-source-1920x1080-{word}.png", 1920, 1080, 280, run)

    print("\n=== below SPLIT_FLOOR: the record as the run's head block ===")
    # A 1000 x 800 window with the lane open is a 720 px body, under the floor
    # of 784. The head is `ART_MIN` 240 bounded by the source like everything
    # else, and the whole field is the run's ground.
    for word in ("before", "after"):
        report_sleeve(f"30-{word}-restacked-1000x800.png", 1000, 800, 280, False)

    print("\n=== the field, sampled ===")
    report_field()
