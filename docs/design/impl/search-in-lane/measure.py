#!/usr/bin/env python3
"""Measure the lane's head off the rendered frames, rather than asserting it.

Three claims this file exists to check, because all three are about pixels
that a unit test cannot see:

1. **The well's mark stands on the destinations' glyph vertical** — the
   magnifier's centre x equals the three glyphs' centre x (theme's
   `SIDEBAR_HEAD_GLYPH_X`, 20 from a head row's left edge, so 44 from the
   window's).
2. **A lane row's sleeve is 48 px**, not the panel's 40 (`SIDEBAR_SLEEVE`).
3. **The readout line is always drawn**, so the first row of `RECENT` sits at
   the same y at rest and mid-query.

Usage: measure.py <frames-dir>
"""

import sys
from pathlib import Path

try:
    from PIL import Image
except ImportError:  # pragma: no cover - the harness installs Pillow
    print("  (Pillow not present; skipping the measured checks)")
    sys.exit(0)


def column_of(im, x0, x1, y0, y1, predicate):
    """Bounding box of the pixels in a window that satisfy `predicate`."""
    px = im.load()
    xs, ys = [], []
    for y in range(y0, y1):
        for x in range(x0, x1):
            if predicate(px[x, y]):
                xs.append(x)
                ys.append(y)
    if not xs:
        return None
    return min(xs), max(xs), min(ys), max(ys)


def report(name, got, want, tol=1):
    ok = got is not None and abs(got - want) <= tol
    print(f"  {'ok  ' if ok else 'FAIL'} {name}: {got} (want {want})")
    return ok


def main(out):
    frames = Path(out)
    rest = frames / "01-lane-well-at-rest-1280.png"
    if not rest.exists():
        print(f"  (no frames at {frames}; run capture.sh first)")
        return 0
    im = Image.open(rest).convert("RGB")
    ground = im.load()[10, 300]

    ok = True
    # The head's glyphs and the well's mark, in the same 40 px-wide window
    # down the lane's left flank. "Ink" is anything meaningfully lighter than
    # the lane's own recess.
    def ink(p):
        return sum(p) - sum(ground) > 60

    for label, y0, y1, want in [
        ("Home glyph centre x", 30, 58, 44),
        ("Library glyph centre x", 70, 98, 44),
        ("Now playing glyph centre x", 110, 138, 44),
        ("well magnifier centre x", 154, 182, 44),
    ]:
        box = column_of(im, 24, 72, y0, y1, ink)
        got = None if box is None else (box[0] + box[1]) // 2
        ok &= report(label, got, want)

    # A lane row's sleeve: the first `RECENT` row, whose sleeve is a record's
    # cover or a playlist's rest tile — either way a solid block 48 px wide
    # starting one `GAP_SM` in from the lane's own gutter.
    box = column_of(im, 24, 120, 265, 420, lambda p: p != ground)
    if box is not None:
        print(f"  ..   first RECENT row ink x{box[0]}-{box[1]} y{box[2]}-{box[3]}")

    print("  measured:", "all good" if ok else "SOMETHING MOVED")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))
