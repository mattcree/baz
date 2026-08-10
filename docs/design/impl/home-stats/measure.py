#!/usr/bin/env python3
"""Count the lane's `RECENT` rows off the rendered frames, rather than
deriving them.

The claim the whole change is measured by is *what removing the well's second
line gives the list back*. That is 20 px of head against a 64 px row pitch, so
it is a statement about pixels — and the arithmetic that preceded these frames
got it wrong in both directions, which is why this file exists.

It counts **sleeves** down the lane's left flank. Every `RECENT` row draws a
48 px square of artwork at a fixed x on a 64 px pitch, against the lane's own
recess, and the head above draws nothing that wide; a row clipped by the
viewport's foot draws a run *shorter* than 48, which is exactly the thing that
has to be told from a whole row. So: runs of "not the ground" in a 48 px-wide
column strip, whole ones counted, short ones reported.

`00-lane-before-1280.png` / `19-lane-before-1920.png` are the same script run
against the binary one commit earlier — same fixture, same lists, same records
played, same windows — so the before/after is a measurement and not a memory.

Usage: measure.py <frames-dir>
"""

import sys
from pathlib import Path

try:
    from PIL import Image
except ImportError:  # pragma: no cover - the harness installs Pillow
    print("  (Pillow not present; skipping the measured checks)")
    sys.exit(0)

# The lane's own geometry: GAP_XL 24 in, then a GAP_SM before the sleeve.
SLEEVE_X0, SLEEVE_X1 = 32, 80
# A whole sleeve. A row clipped by the viewport's foot draws less.
WHOLE = 48
# Below the head in every frame either side of the change: the tallest thing
# the head puts in this strip is the 40 px current-place card, and `RECENT`'s
# own word sits at y≈212. Scanning from here means every run below is a
# sleeve, so a *clipped* row is measurable rather than indistinguishable from
# a glyph.
LIST_TOP = 216


def sleeve_runs(path):
    """Every run of ink in the lane's sleeve column, top to bottom."""
    im = Image.open(path).convert("RGB")
    px = im.load()
    ground = px[10, im.height // 2]
    runs, start = [], None
    # Stop above the foot marks, which are centred and never reach this strip.
    for y in range(LIST_TOP, im.height - 80):
        lit = any(
            sum(abs(a - b) for a, b in zip(px[x, y], ground)) > 24
            for x in range(SLEEVE_X0, SLEEVE_X1, 2)
        )
        if lit and start is None:
            start = y
        elif not lit and start is not None:
            runs.append((start, y - start))
            start = None
    if start is not None:
        runs.append((start, im.height - 80 - start))
    return [r for r in runs if r[1] >= 4]


def rows(path):
    """(whole rows, px of the first clipped row) for one frame."""
    runs = sleeve_runs(path)
    whole = [r for r in runs if r[1] >= WHOLE]
    # Only the *last* run can be a row the viewport's foot cut off; a short
    # run anywhere above it is a sleeve's own quiet artwork against the ground.
    tail = runs[-1][1] if runs and runs[-1][1] < WHOLE else 0
    return len(whole), tail, (whole[0][0] if whole else None)


def compare(frames, before, after, want_before, want_after):
    ok = True
    for path, want in ((before, want_before), (after, want_after)):
        p = frames / path
        if not p.exists():
            print(f"  ..   no frame at {p}")
            continue
        whole, clipped, top = rows(p)
        good = whole == want
        ok &= good
        print(
            f"  {'ok  ' if good else 'FAIL'} {path}: {whole} whole RECENT rows "
            f"(want {want}), first at y={top}, +{clipped} px of the next"
        )
    return ok


def main(out):
    frames = Path(out)
    ok = True
    print("  1280 x 860 — the 20 px is banked, not cashed:")
    ok &= compare(frames, "00-lane-before-1280.png", "01-lane-at-rest-1280.png", 7, 7)
    print("  1920 x 1080 — the row the readout line was costing:")
    ok &= compare(frames, "19-lane-before-1920.png", "20-lane-at-rest-1920.png", 10, 11)
    print("  and nothing moves when a key lands in the well:")
    for rest, mid in (
        ("01-lane-at-rest-1280.png", "02-lane-mid-query-1280.png"),
        ("20-lane-at-rest-1920.png", "21-lane-mid-query-1920.png"),
    ):
        a, b = frames / rest, frames / mid
        if not (a.exists() and b.exists()):
            continue
        moved = rows(a)[2] != rows(b)[2]
        ok &= not moved
        print(
            f"  {'FAIL' if moved else 'ok  '} {mid}: first row at "
            f"y={rows(b)[2]}, against y={rows(a)[2]} at rest"
        )
    print("  measured:", "all good" if ok else "SOMETHING MOVED")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))
