#!/usr/bin/env python3
"""The composition laws, measured off the frames ADR-0022 ships.

Uses the rulers committed at `docs/design/composition/tools/`. Four tables:

  L1  one gutter per window edge  — the x of each surface's first and last ink
  L4  one centre line per bar     — every mark's centre, and the spread
  L5  the permitted alignment edges, per surface — the x-edge census
  L6  declared-then-measured hierarchy — contrast-weighted ink mass per region

Run from the repo root, after `capture.sh`:

    toolbox run -c baz-dev python3 docs/design/impl/places/measure.py
"""
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "..", "composition", "tools"))
from ruler import Img, col_ink, row_ink, runs, ink_mass, dist  # noqa: E402

WALL = (0x0C, 0x0D, 0x0E)
RECESS = (0x06, 0x07, 0x08)
SHOTS = HERE


def load(name):
    return Img(os.path.join(SHOTS, f"{name}.png"))


def bar_top(im):
    """The y at which the bottom bar's recess band starts — found, not assumed.

    The scan starts above the needle: the needle is flush on the window's own
    bottom edge and is *not* recess where a segment is filled, so a ruler that
    started at the last row would find the line rather than the band.
    """
    return next(
        y + 2
        for y in range(im.h - 1 - 2, im.h - 240, -1)
        if dist(im.rgb(5, y), RECESS) > 2
    )


def top_bar_h(im):
    """The y at which the top strip's hairline sits."""
    return next(y for y in range(12, 160) if dist(im.rgb(5, y), WALL) > 2)


def xedges(im, y0, y1, ground, thr=8):
    """The x of every vertical run of ink between y0 and y1."""
    flags = col_ink(im, y0, y1, 0, im.w, ground, thr)
    return [s for s, _ in runs(flags, gap=3)]


def xspan(im, y0, y1, ground, thr=8):
    e = xedges(im, y0, y1, ground, thr)
    if not e:
        return None
    flags = col_ink(im, y0, y1, 0, im.w, ground, thr)
    r = runs(flags, gap=3)
    return e[0], r[-1][1]


def ycentres(im, x0, x1, y0, y1, ground, thr=8, gap=2):
    """The centre y of every horizontal run of ink in a column slice."""
    flags = row_ink(im, x0, x1, y0, y1, ground, thr)
    return [(s + e) / 2.0 for s, e in runs(flags, origin=y0, gap=gap)]


def table(title, rows):
    print(f"\n### {title}")
    width = max(len(str(r[0])) for r in rows)
    for name, value in rows:
        print(f"  {str(name).ljust(width)}   {value}")


def law1(name, shot):
    im = load(shot)
    hang = 40
    tb = top_bar_h(im)
    bar = bar_top(im)
    rows = []
    # thr 4, not 8: the search well is a *recess* box on the wall, and the two
    # surfaces are 6 apart in linear bytes by design (the palette's whole
    # thesis). A ruler that could not see it would report the well's ink rather
    # than the well.
    span = xspan(im, 0, tb, WALL, thr=4)
    rows.append(("top strip", f"{span}   want ({hang}, {im.w - hang})"))
    span = xspan(im, tb + 2, bar - 2, WALL)
    rows.append(("body", f"{span}"))
    span = xspan(im, bar + 2, im.h - 3, RECESS)
    rows.append(("bottom bar", f"{span}   want ({hang}, {im.w - hang})"))
    rows.append(("top strip h", tb + 1))
    rows.append(("bottom furniture", im.h - bar + 2))
    table(f"L1 — {name} ({im.w}×{im.h})", rows)


def law4(shot):
    im = load(shot)
    bar = bar_top(im)
    band = im.h - 2 - bar  # the needle takes the last 2
    mid = bar + band / 2.0
    zones = {
        "now-playing block": (40, 300),
        "elapsed / total": (318, 400),
        "Queue door": (417, 570),
        "transport": (584, 700),
        "signal note": (1020, 1100),
        "volume": (1105, 1245),
    }
    rows = []
    marks = []
    for zname, (x0, x1) in zones.items():
        # `gap=0` for the volume: the unity detent sits `DETENT_GAP` 2 px above
        # the rail, so a run-joiner that tolerates 2 merges the two marks into
        # one block whose centre is neither.
        cs = ycentres(
            im,
            x0,
            min(x1, im.w - 1),
            bar,
            im.h - 3,
            RECESS,
            gap=0 if zname == "volume" else 2,
        )
        if not cs:
            continue
        # A zone taller than one line hangs its extra lines symmetrically about
        # the centre line, so the *block's* centre is the mark to compare — with
        # one exception the law names: the volume's mark is its **rail**, and
        # the unity detent is a deliberate 5 px mark *above* it
        # (`theme::DETENT_GAP`), so a block centre would average the two.
        mark = max(cs) if zname == "volume" else (min(cs) + max(cs)) / 2.0
        marks.append(mark)
        rows.append((zname, f"centre {mark:7.2f}   lines {len(cs)}   Δ {mark - mid:+.2f}"))
    spread = max(marks) - min(marks) if marks else 0.0
    rows.append(("band", f"{bar}…{im.h - 3}   h {band}   mid {mid:.1f}"))
    rows.append(("SPREAD", f"{spread:.2f} px   (law L4 ceiling: 2)"))
    table(f"L4 — one centre line per bar ({im.w}×{im.h})", rows)


def law5(name, shot, y0f, y1f, ground=WALL):
    im = load(shot)
    tb = top_bar_h(im)
    bar = bar_top(im)
    y0 = y0f(tb, bar)
    y1 = y1f(tb, bar)
    e = xedges(im, y0, y1, ground)
    table(
        f"L5 — {name} ({im.w}×{im.h})",
        [("x-edges", len(e)), ("first…last", f"{e[0]}…{e[-1]}" if e else "—"), ("all", e)],
    )


def law6(shot, regions, ground=WALL):
    """Contrast-weighted ink mass per region, **and per line of it**.

    Two columns because the law needs both and they answer different questions.
    *Mass* is what §13 says and it ranks a twelve-row list above a one-word
    title, correctly: there is simply more ink in twelve lines. *Mass per line*
    is loudness, and it is the half of the audit's defect 5 that was about the
    album's name — the finding was never "the title has less ink than the track
    list", it was "the title is not the loudest thing you read".
    """
    im = load(shot)
    total = 0.0
    got = []
    for rname, (x0, y0, x1, y1, lines) in regions.items():
        _, _, _, m = ink_mass(im, x0, y0, x1, y1, ground)
        got.append((rname, m, m / max(lines, 1)))
        total += m
    by_mass = sorted(got, key=lambda kv: -kv[1])
    by_line = sorted(got, key=lambda kv: -kv[2])
    table(
        f"L6 — measured hierarchy, by mass ({im.w}×{im.h})",
        [
            (f"{i + 1}. {n}", f"{m:12.0f}   {100.0 * m / total:5.1f} %")
            for i, (n, m, _) in enumerate(by_mass)
        ],
    )
    table(
        "L6 — measured hierarchy, by mass **per line** (loudness)",
        [(f"{i + 1}. {n}", f"{k:12.0f}") for i, (n, _, k) in enumerate(by_line)],
    )


if __name__ == "__main__":
    law1("the wall", "01-wall-no-scrollbar")
    law1("the record's page", "02-album-place")
    law1("the queue place", "05-queue-place")
    law1("the record's page at 1920", "11-album-place-1920")

    law4("05-queue-place")

    law5("the record's page — the aside", "02-album-place", lambda t, b: t + 41, lambda t, b: 700)
    law5(
        "the queue place — the rows",
        "05-queue-place",
        lambda t, b: t + 60,
        lambda t, b: t + 500,
    )

    # The page's declared order: the work → the title → `Play album` → the
    # track list → the condition.
    law6(
        "02-album-place",
        {
            "the work (sleeve)": (40, 89, 360, 409, 1),
            "the title": (384, 90, 1230, 125, 1),
            "`Play album`": (40, 421, 360, 453, 1),
            "the track list": (384, 220, 1230, 600, 12),
            "the condition (Details)": (40, 470, 360, 690, 13),
            "the artist": (384, 126, 1230, 152, 1),
            "the catalogue line": (384, 152, 1230, 172, 1),
        },
    )
