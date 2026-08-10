#!/usr/bin/env python3
"""Measure the wall's scrollbar and the index rail off the rendered frames.

The defect was found with a ruler on a screenshot rather than in the source,
and it is checked the same way. Two x-ranges per frame, read out of the
right-hand 200 px of the window:

1. **the bar** — [`theme::WALL_SCROLLBAR_W`] 4 px wide and, at rest near the
   top of a wall of 25 records, a hundred-odd pixels of *contiguous* ink;
2. **the rail's letters** — [`theme::INDEX_W`] 60 px of lane whose ink ends on
   `W − theme::HANG`, each glyph at most a line tall.

**The longest contiguous inked run in a column is what tells them apart**, and
it is the only discriminator that works: a column of the rail accumulates a lot
of ink over the whole strip (twenty-seven letters), but never more than one
line of it at a stretch, while the scroller is one unbroken block. Counting
inked *rows* instead was the first attempt and it cannot separate them.

The claim the numbers make:

- **before** the move the bar is at the wall's right edge, *inboard* of the
  rail's ink, with `INDEX_LANE_W` 108 of window outboard of it — the owner's
  *"scroll bar is in a strange location… it seems to have padding on the
  right"*;
- **after** it is `WALL_SCROLLBAR_W` from the window's edge, outboard of the
  rail, and **the rail's ink has not moved by a pixel**.

Usage: measure.py <frames-dir>
"""

import sys
from collections import Counter
from pathlib import Path

try:
    from PIL import Image
except ImportError:  # pragma: no cover - the harness installs Pillow
    print("  (Pillow not present; skipping the measured checks)")
    sys.exit(0)

# `crate::theme`, at the values these frames were rendered with.
HANG = 40
INDEX_W = 60
INDEX_LANE_W = 108
BAR_W = 4

# name, moved
FRAMES = [
    ("01-lane-open-before-1280", False),
    ("02-lane-open-after-1280", True),
    ("01-lane-shut-before-1280", False),
    ("02-lane-shut-after-1280", True),
    ("05-lane-open-before-1920", False),
    ("06-lane-open-after-1920", True),
    ("05-lane-shut-before-1920", False),
    ("06-lane-shut-after-1920", True),
]

# The wall's body, clear of the top bar and of the now-playing bar. The density
# detents at the lane's foot are inside it and are short marks, so they read as
# ink rather than as a bar, which is what we want.
TOP, FOOT = 55, 95
STRIP = 200
# A scroller is never this short; a line of type never this tall.
BLOCK = 40


def runs(xs):
    """Contiguous runs in a sorted list of x values, as (first, last)."""
    out = []
    for x in sorted(xs):
        if out and x == out[-1][1] + 1:
            out[-1][1] = x
        else:
            out.append([x, x])
    return [(a, b) for a, b in out]


def measure(im):
    """`(bar runs, ink runs)` in the window's right-hand strip."""
    w, h = im.size
    px = im.load()
    y0, y1, x0 = TOP, h - FOOT, w - STRIP
    ground = Counter(
        px[x, y] for x in range(x0, w) for y in range(y0, y1)
    ).most_common(1)[0][0]

    longest = {}
    for x in range(x0, w):
        run = current = 0
        for y in range(y0, y1):
            if sum(abs(a - b) for a, b in zip(px[x, y], ground)) > 8:
                current += 1
                run = max(run, current)
            else:
                current = 0
        longest[x] = run
    return (
        runs(x for x, r in longest.items() if r >= BLOCK),
        runs(x for x, r in longest.items() if 0 < r < BLOCK),
    )


def main(out):
    frames = Path(out)
    ok = True
    seen = 0
    for name, moved in FRAMES:
        path = frames / f"{name}.png"
        if not path.exists():
            continue
        seen += 1
        im = Image.open(path).convert("RGB")
        w, h = im.size
        bar, ink = measure(im)
        print(f"  {name}  ({w}×{h})")
        if not bar:
            print("    FAIL no bar in the window's right-hand strip")
            ok = False
            continue
        if len(bar) != 1:
            print(f"    FAIL {len(bar)} bar-shaped runs: {bar}")
            ok = False
            continue
        left, right = bar[0]
        # Where the bar should be: `WALL_SCROLLBAR_W` off the window's edge
        # after the move, and off the *wall's* — the window less the rail's
        # lane and the bar's own — before it.
        want = (w - BAR_W) if moved else (w - INDEX_LANE_W - BAR_W)
        good = left == want and right - left + 1 == BAR_W
        print(
            f"    {'ok  ' if good else 'FAIL'} the bar spans x {left}–{right} "
            f"(want {want}–{want + BAR_W - 1})"
        )
        ok &= good

        ink_left = min(a for a, _ in ink) if ink else None
        ink_right = max(b for _, b in ink) if ink else None
        # The rail's ink hangs from `W − HANG` and is at most `INDEX_W` wide —
        # law L1's one window gutter, which the move was not allowed to touch.
        placed = (
            ink_right is not None
            and abs(ink_right - (w - HANG - 1)) <= 1
            and ink_left >= w - HANG - INDEX_W
        )
        print(
            f"    {'ok  ' if placed else 'FAIL'} the rail's ink spans x "
            f"{ink_left}–{ink_right} (lane {w - HANG - INDEX_W}–{w - HANG - 1})"
        )
        ok &= placed

        outboard = left >= (ink_right or 0)
        print(
            f"    {'ok  ' if outboard == moved else 'FAIL'} the bar is "
            f"{'outboard' if outboard else 'inboard'} of the rail's ink"
            + ("" if moved else "   ← the defect, as reported")
        )
        ok &= outboard == moved
        if not moved:
            print(f"    ..   {w - right - 1} px of window outboard of the bar")
    if seen == 0:
        print(f"  (no frames at {frames}; run capture.sh first)")
        return 0
    print("  measured:", "as designed" if ok else "SOMETHING MOVED")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))
