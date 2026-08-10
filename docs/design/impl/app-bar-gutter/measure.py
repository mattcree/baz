#!/usr/bin/env python3
"""Read the window's right-hand alignment edge off a baz frame.

The owner's complaint of 2026-08-10 — *"the settings cog is padded in quite a
bit and does not align with the rail"* — is a claim about pixels, so it is
answered with pixels rather than with a look. For each of three surfaces this
finds the **rightmost column that carries ink**, in window coordinates:

  * the app bar's trailing control (the gear, or the close button when
    ``app::owns_chrome`` is true);
  * the index rail's letters, which ``crate::spine`` draws flush to
    ``bounds.width - theme::HANG``;
  * the bottom bar's volume groove.

The last two are the control: they are two independent surfaces that already
agree, which is what makes their shared x *the* edge rather than one surface's
opinion. If the app bar's trailing ink lands on it too, law L1 holds on the
window's right edge; if it does not, the difference is the defect, in pixels.

"Ink" is any pixel whose furthest RGB channel stands more than ``THRESH`` from
the crop's own modal colour. The mode is taken per crop rather than from a
theme token because the app bar's ground (``theme::bar``) and the wall's are
different planes, and a single hard-coded background would find ink in the
plane change.
"""

import re
import subprocess
import sys
from collections import defaultdict

#: How far a channel must stand from the crop's modal colour to count as ink.
#: 10/255 clears the wall/recess plane change (6 levels) and the hairline, and
#: still catches the faintest thing measured here — a `paper_muted` rail letter
#: at roughly 100 levels over the wall.
THRESH = 10


def ink_span(png, x0, y0, w, h):
    """Return ``(first, last)`` inked column of a crop, in window x, or None."""
    out = subprocess.run(
        ["magick", png, "-crop", f"{w}x{h}+{x0}+{y0}", "+repage", "-depth", "8", "txt:-"],
        capture_output=True,
        text=True,
        check=True,
    ).stdout
    px = {}
    for line in out.splitlines()[1:]:
        m = re.match(r"(\d+),(\d+): \((\d+),(\d+),(\d+)", line)
        if m:
            x, y, r, g, b = (int(v) for v in m.groups())
            px[(x, y)] = (r, g, b)
    counts = defaultdict(int)
    for v in px.values():
        counts[v] += 1
    base = max(counts, key=counts.get)
    xs = sorted(
        {
            x
            for (x, _y), v in px.items()
            if max(abs(v[i] - base[i]) for i in range(3)) > THRESH
        }
    )
    return (x0 + xs[0], x0 + xs[-1]) if xs else None


def main():
    png, w = sys.argv[1], int(sys.argv[2])
    h = int(sys.argv[3]) if len(sys.argv) > 3 else 0
    # The app bar's control row: y 4..36, `APP_BAR_PAD_V` inside a 41 px band.
    # x from `W - 100`, which holds the trailing control's box and the gutter
    # and **nothing else** in any of the four states measured: the display
    # options' slot ends at `W - 104` at its furthest out (the pre-fix state,
    # where the phantom seam pushed the whole cluster in), so the span printed
    # is one control's rather than a cluster's.
    # The rail is measured **first** and everything else is reported against
    # it: it is the surface the owner named, and the one the other two are
    # either on or off.
    regions = {
        "index rail letters": (w - 60, 170, 56, 480),
        "app bar, trailing control": (w - 100, 4, 100, 33),
        "app bar, zone 1": (0, 4, 120, 33),
    }
    if h:
        # The bottom bar's volume block, whose groove ends on the same gutter.
        regions["bottom bar, volume groove"] = (w - 200, h - 60, 200, 40)
    edge = None
    for label, box in regions.items():
        span = ink_span(png, *box)
        if span is None:
            print(f"  {label:28s} no ink")
            continue
        first, last = span
        if label == "index rail letters":
            edge = last
        note = ""
        if edge is not None and label != "index rail letters":
            note = f"   delta vs rail {last - edge:+d}"
        print(f"  {label:28s} ink x {first}..{last}   from right {w - last:3d}{note}")


if __name__ == "__main__":
    main()
