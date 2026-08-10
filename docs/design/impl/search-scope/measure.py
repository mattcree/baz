#!/usr/bin/env python3
"""Measure the well's two edges off the rendered frames, rather than asserting it.

ADR-0036 §4's whole case for putting the `×` on the left is geometric, and the
geometry is the kind a unit test cannot see. Three claims, checked here against
real pixels:

1. **The mark's box does not move when the query lands.** The magnifier at rest
   and the cross under a query have the same centre x, which is the
   destinations' own glyph vertical (`SIDEBAR_HEAD_GLYPH_X` 20 from a head
   row's left edge, so 44 from the window's, the lane being inset `GAP_XL` 24).
2. **The query's own room is unchanged on the left**, which follows from (1):
   the field's text inset is `SIDEBAR_HEAD_TEXT_X` 44 in both states, so the
   query's first glyph starts at 24 + 44 = 68.
3. **The count still sits in its reserved slot at the right**, ending one
   `GAP_MD` in from the field's right edge: 24 + 232 − 12 = 244.

And one that is about the decision rather than the geometry:

4. **The well's placeholder is on screen at rest and gone under a query**, so
   the scope word occupies the lane it is supposed to and nothing else.

Usage: measure.py <frames-dir>
"""

import sys
from pathlib import Path

try:
    from PIL import Image
except ImportError:  # pragma: no cover - the harness installs Pillow
    print("  (Pillow not present; skipping the measured checks)")
    sys.exit(0)


def box_of(im, x0, x1, y0, y1, predicate):
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


def report(name, got, want, tol=2):
    ok = got is not None and abs(got - want) <= tol
    print(f"  {'ok  ' if ok else 'FAIL'} {name}: {got} (want {want})")
    return ok


def main(out):
    frames = Path(out)
    rest = frames / "01-well-at-rest-library-1280.png"
    query = frames / "02-well-mid-query-the-x-and-the-count-1280.png"
    if not rest.exists() or not query.exists():
        print(f"  (no frames at {frames}; run capture.sh first)")
        return 0

    at_rest = Image.open(rest).convert("RGB")
    under = Image.open(query).convert("RGB")
    # The recess the well is drawn on, sampled from inside the field itself,
    # well clear of both the mark and the count.
    ground = at_rest.load()[150, 40]

    def ink(p):
        return sum(p) - sum(ground) > 60

    # The well's own band, *inside* its border. The head is GAP_XL 24 down and
    # the block is TRANSPORT_HIT 32 tall, so the field runs y24–y56 and its
    # focus ring is the first and last row of that — which is ink right across
    # the field and would swamp every column window below.
    Y0, Y1 = 28, 53

    ok = True
    # 1. The mark's box, in both states, in a window that starts inside the
    #    field's own left border — the mid-query frame is focused, and a focus
    #    ring is a vertical line of ink at the field's edge — and stops well
    #    short of the query's first glyph at 68.
    at = box_of(at_rest, 28, 62, Y0, Y1, ink)
    on = box_of(under, 28, 62, Y0, Y1, ink)
    ok &= report(
        "magnifier centre x (at rest)", None if at is None else (at[0] + at[1]) // 2, 44
    )
    ok &= report(
        "clear mark centre x (under a query)",
        None if on is None else (on[0] + on[1]) // 2,
        44,
    )

    # 2. The query's first glyph, against the placeholder's — both set at the
    #    field's own SIDEBAR_HEAD_TEXT_X 44 inset from the lane's 24.
    placeholder = box_of(at_rest, 62, 240, Y0, Y1, ink)
    typed = box_of(under, 62, 160, Y0, Y1, ink)
    ok &= report(
        "placeholder first glyph x", None if placeholder is None else placeholder[0], 68
    )
    ok &= report("query first glyph x", None if typed is None else typed[0], 68)

    # 3. The count's slot at the other edge — right-aligned, ending a GAP_MD in
    #    from the field's right edge. Searched right of where a two-letter
    #    query can reach.
    count = box_of(under, 170, 250, Y0, Y1, ink)
    ok &= report("match count right edge x", None if count is None else count[1], 244)

    # 4. And the placeholder is *gone* under a query: no ink between the query's
    #    own two letters and the count's slot.
    gap = box_of(under, 110, 168, Y0, Y1, ink)
    print(
        f"  {'ok  ' if gap is None else 'FAIL'} the placeholder is not drawn "
        f"beside a query: {'clear' if gap is None else gap}"
    )
    ok &= gap is None

    print("  measured:", "all good" if ok else "SOMETHING MOVED")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))
