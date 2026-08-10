#!/usr/bin/env python3
"""Read doc 14 tier 1's two load-bearing figures off the committed frames,
rather than deriving them from tokens.

Both claims are about pixels, and both were made in a document before any of
this was drawn:

1. **The identity blocks are the same height.** ADR-0024 §A4.3 says a record's
   is 80 px (32 + 4 + 24 + 4 + 16) and a playlist's was 52, and that restoring
   the byline makes them one shape. So: measure the ink in the two identity
   crops — first ink row to last — and compare. The unit test in
   `views/playlist.rs` asserts the *arithmetic*; this asserts the render.

2. **The strip still fits at `RUN_MEASURE` 440.** Design 14 §6.3 called this
   *"the one measurement in this study that wants a frame before it ships"* —
   `Run · ` costs ~34 px and `Save as new playlist` ~48 px more than the word
   it replaces, and the tightest state has provenance, a reading, `Undo` and
   the longer word all in one 440 px line. So: find the ink columns in each
   strip crop, report where the reading ends and the word begins, and fail if
   they touch.

Usage: measure.py <frames-dir>
"""

import sys
from pathlib import Path

try:
    from PIL import Image
except ImportError:  # pragma: no cover - the harness installs Pillow
    print("  (Pillow not present; skipping the measured checks)")
    sys.exit(0)

# The frames are drawn in `closing-time`, whose wall is near-black. Anything
# meaningfully above it is ink; the threshold is deliberately generous so that
# `paper_faint` at 12 px counts and JPEG-free PNG noise does not.
INK = 40
# The crop the capture takes is 10 px wider than the run column on the left,
# so the column's own right edge is this far in.
STRIP_PAD = 10


def ink_rows(image):
    """Row indices holding ink, top to bottom."""
    w, h = image.size
    px = image.convert("L").load()
    return [y for y in range(h) if any(px[x, y] > INK for x in range(w))]


def ink_cols(image):
    """Column indices holding ink, left to right."""
    w, h = image.size
    px = image.convert("L").load()
    return [x for x in range(w) if any(px[x, y] > INK for y in range(h))]


def runs(values):
    """Contiguous runs in a sorted index list, as (first, last) pairs."""
    out = []
    for value in values:
        if out and value == out[-1][1] + 1:
            out[-1][1] = value
        else:
            out.append([value, value])
    return [tuple(pair) for pair in out]


def main(root: Path) -> int:
    failures = 0

    print("--- the identity blocks, measured (ADR-0024 §A4.3) ---")
    for size in ("1280x860", "1920x1080"):
        heights = {}
        for kind, name in (("made", "identity-made"), ("found", "identity-found")):
            frame = root / f"{'0' if size.startswith('1280') else '1'}{'4' if kind == 'made' else '6'}-{name}-{size}.png"
            if not frame.exists():
                print(f"  {frame.name}: missing")
                failures += 1
                continue
            rows = ink_rows(Image.open(frame))
            heights[kind] = rows[-1] - rows[0] + 1 if rows else 0
            print(f"  {size} {kind:<5} block ink spans {heights[kind]} px")
        if len(heights) == 2:
            # The ink's extent is shorter than the 80 px of *boxes* — the hero's
            # cap height starts below its line box and the meta line's baseline
            # sits above its own bottom — so what is asserted is that the two
            # pages agree, which is the claim. A 2 px tolerance covers the two
            # faces' differing descenders.
            delta = abs(heights["made"] - heights["found"])
            ok = delta <= 2
            print(f"  {size} difference: {delta} px  {'ok' if ok else 'FAIL'}")
            failures += 0 if ok else 1

    print()
    print("--- the run strip at RUN_MEASURE 440 (design 14 §6.3) ---")
    for prefix, size in (("0", "1280x860"), ("1", "1920x1080")):
        for tag, state in ((f"{prefix}8", "unfiled"), (f"{prefix}a", "saved"), (f"{prefix}c", "diverged")):
            frame = root / f"{tag}-strip-{state}-{size}.png"
            if not frame.exists():
                print(f"  {frame.name}: missing")
                failures += 1
                continue
            cols = ink_cols(Image.open(frame))
            if not cols:
                print(f"  {frame.name}: no ink")
                failures += 1
                continue
            blocks = runs(cols)
            # The widest gap is the `Space::Fill` between the reading (with
            # `Undo` when there is one) and the save word.
            gaps = [
                (blocks[i + 1][0] - blocks[i][1] - 1, blocks[i][1], blocks[i + 1][0])
                for i in range(len(blocks) - 1)
            ]
            gap, ends, begins = max(gaps) if gaps else (0, 0, 0)
            right = cols[-1] - STRIP_PAD
            ok = gap > 0 and right <= 440
            print(
                f"  {size} {state:<8} reading ends x={ends - STRIP_PAD:>3}, "
                f"word begins x={begins - STRIP_PAD:>3}, "
                f"air between {gap:>3} px, right edge {right:>3}/440  "
                f"{'ok' if ok else 'FAIL'}"
            )
            failures += 0 if ok else 1

    print()
    print("all measured claims hold" if not failures else f"{failures} FAILED")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main(Path(sys.argv[1] if len(sys.argv) > 1 else ".")))
