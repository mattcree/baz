#!/usr/bin/env python3
"""The measurements the audit tabulates. One window size per run."""
import sys

sys.path.insert(0, __import__("os").path.dirname(__import__("os").path.abspath(__file__)))
from ruler import Img, hexs, dist, runs, ink_box, ink_mass, best_unit  # noqa: E402
from bands import xruns, yruns  # noqa: E402

SHOTS = "/tmp/baz-comp-shots"
W, H = (int(v) for v in sys.argv[1].split("x"))
TAG = f"{W}x{H}"
WALL = (0x0C, 0x0D, 0x0E)
RECESS = (0x06, 0x07, 0x08)
PLINTH = (0x14, 0x15, 0x17)
LIT = (0x1C, 0x1D, 0x20)
NAMES = {WALL: "wall", RECESS: "recess", PLINTH: "plinth", LIT: "plinth-lit"}


def load(n):
    return Img(f"{SHOTS}/{n}-{TAG}.png")


def xr(im, y0, y1, x0, x1, g, thr=3, gap=6):
    return xruns(im, y0, y1, x0, x1, thr=thr, gap=gap, ground=g)[1]


def yr(im, x0, x1, y0, y1, g, thr=3, gap=1):
    return yruns(im, x0, x1, y0, y1, thr=thr, gap=gap, ground=g)[1]


def p(*a):
    print(*a)


def edges(rs):
    out = set()
    for s, e in rs:
        out.add(s)
        out.add(e)
    return out


# find the frame's bands
im = load("wall-rest")
BAR = next(y + 1 for y in range(H - 1, H - 200, -1) if dist(im.rgb(5, y), RECESS) > 2)
RULE2 = BAR - 1
# The top bar's own hairline, found rather than assumed: x = 5 is clear wall in
# every arrangement, so the first non-wall row under the frame's top is the rule.
# (It was hardcoded at 52, which was the drawn 53 px bar less its rule; the bar's
# padding is a token now, so the ruler reads it instead of remembering it.)
TOPB = next(y for y in range(12, 120) if dist(im.rgb(5, y), WALL) > 2)
BODY = TOPB + 1
p(f"############ {TAG}   top bar y[0,{TOPB}] h {TOPB + 1}   "
  f"body y[{BODY},{RULE2})   bar y[{BAR},{H})  bar h {H - BAR + 1}")

# ============================================================ TOP BAR
p(f"\n### TOP BAR — element boxes (surface + ink), rows 0..{TOPB}, ground wall")
for lo, hi in xr(im, 0, TOPB, 0, W, WALL, thr=3, gap=6):
    ys = yr(im, lo, hi, 0, TOPB, WALL, thr=3, gap=1)
    box = ink_box(im, lo, 0, hi, TOPB, WALL, 3)
    p(f"   x {lo:5d}..{hi:5d} (w {hi - lo:4d})   ink-y {ys}   box {box}")

p("\n### TOP BAR — the two right-hand labels, ink boxes")
for name, a, b in (("counts", W - 300, W - 130), ("Settings", W - 128, W)):
    box = ink_box(im, a, 0, b, TOPB, WALL, 20)
    if box:
        p(f"   {name:9s} ink box {box}   ink-y centre {(box[1] + box[3]) / 2:.1f}")
    else:
        p(f"   {name:9s} no ink in x[{a},{b})")

# the search well: exact surface extent
p("\n### TOP BAR — search well surface extent (recess against wall)")
# By the well's own ground (recess against wall) rather than by "not wall",
# which merges the well with the group-key row beside it.
col = [y for y in range(0, TOPB) if dist(im.rgb(200, y), RECESS) <= 2]
p(f"   well rows {min(col)}..{max(col) + 1}  h {max(col) + 1 - min(col)}")
row = [x for x in range(0, 600) if dist(im.rgb(x, (min(col) + max(col)) // 2), RECESS) <= 2]
p(f"   well cols {min(row)}..{max(row) + 1}  w {max(row) + 1 - min(row)}")
ph = ink_box(im, min(row), min(col), max(row) + 1, max(col) + 1, RECESS, 14)
p(f"   placeholder ink box {ph}  (well interior)")

# ============================================================ BOTTOM BAR idle
def bar_report(im, tag):
    p(f"\n### BOTTOM BAR — {tag}")
    rs = xr(im, BAR, H, 0, W, RECESS, thr=3, gap=6)
    for lo, hi in rs:
        ys = yr(im, lo, hi, BAR, H, RECESS, thr=3, gap=1)
        p(f"   x {lo:5d}..{hi:5d} (w {hi - lo:4d})  ink-y {ys}")
    mid = (BAR + H) / 2
    p(f"   bar mid-line y = {mid}")
    for name, a, b in (
        ("left zone", 8, 260),
        ("queue ctl", 262, 460),
        ("transport", W // 2 - 120, W // 2 + 120),
        ("seek row", W // 2 - 220, W // 2 + 220),
        ("right zone", W - 260, W),
    ):
        box = ink_box(im, a, BAR, b, H, RECESS, 10)
        if box:
            c = (box[1] + box[3]) / 2
            p(
                f"   {name:10s} ink box {box}  ink-y centre {c:7.1f}  "
                f"delta from bar mid {c - mid:+6.1f}"
            )


bar_report(im, "idle (nothing playing)")
bar_report(load("wall-playing"), "playing")


# ---------------------------------------------------------------- law L4
# One mark per zone, in a window tight enough to isolate the *mark* from the
# block it sits in. The zone windows above measure blocks — which is exactly the
# defect the law is about — so a mark that shares a window with the seek groove
# reads as the groove. Each row below is one thing a listener's eye lands on.
def marks(imx, tag):
    p(f"\n### BOTTOM BAR — the marks, against the bar's own centre line ({tag})")
    mid = (BAR + H) / 2
    windows = (
        ("transport glyphs", W // 2 - 60, W // 2 + 60, 0),
        ("seek groove", W // 2 - 140, W // 2 + 140, 1),
        ("volume rail", W - 130, W - 70, 0),
        ("mute glyph", W - 172, W - 148, 0),
        ("now-playing line 2", 40, 175, 1),
        ("queue label", 286, 340, 0),
        ("signal note", W - 245, W - 180, 0),
    )
    for name, a, b, which in windows:
        rs = yr(imx, a, b, BAR, H, RECESS, thr=3, gap=2)
        if not rs or which >= len(rs):
            p(f"   {name:17s} —")
            continue
        s0, e0 = rs[which]
        c = (s0 + e0) / 2
        p(f"   {name:17s} y {s0}..{e0}   centre {c:7.1f}   "
          f"delta from bar mid {c - mid:+6.1f}")


marks(im, "idle")
marks(load("wall-playing"), "playing")

# ============================================================ transport glyphs
p("\n### transport glyph optical centring (16 px icon in a 32 px hit box)")
imp = load("wall-playing")
row_c = (BAR + H) / 2  # the band's mid-line: the transport's centre (law L4)
for lo, hi in xr(imp, BAR, H, W // 2 - 130, W // 2 + 130, RECESS, thr=3, gap=6):
    box = ink_box(imp, lo - 12, BAR, hi + 12, H, RECESS, 10)
    p(f"   glyph x {lo}..{hi}  ink box {box}")

# ============================================================ WALL
p("\n### WALL — the hang, scanline")
for y in (200, 250):
    segs, prev = [], None
    for x in range(W):
        isw = dist(im.rgb(x, y), WALL) <= 2
        if prev is None or isw != prev:
            segs.append([x, isw])
            prev = isw
    segs.append([W, None])
    p(
        f"   y={y}: "
        + " | ".join(
            f"{'wall' if segs[i][1] else 'art'}:{segs[i + 1][0] - segs[i][0]}"
            for i in range(len(segs) - 1)
        )
    )

p("\n### WALL — one tile's vertical structure (column 1, x 40..)")
art_lo = 40
art_hi = next(x for x in range(40, W) if dist(im.rgb(x, 200), WALL) <= 2)
for s, e in yr(im, art_lo, art_hi, BODY, RULE2, WALL, thr=3, gap=0):
    p(f"   y {s:5d}..{e:5d}  h {e - s:4d}")

# ============================================================ INSPECTOR
p("\n### INSPECTOR")
imi = load("inspector")
# The panel's left edge from the *wall's* right edge plus its 1 px rule, taken
# over several rows and voted, so one row of ink cannot move it.
PROBES = [RULE2 - 20, RULE2 - 40, RULE2 - 60, (BODY + RULE2) // 2]
cands = []
for probe in PROBES:
    wall_right = max((x for x in range(W) if dist(imi.rgb(x, probe), WALL) <= 2), default=0)
    cands.append(wall_right)
shelf_right = max(set(cands), key=cands.count)
panel_x = shelf_right + 2
p(f"   panel surface x {panel_x}..{W}  (w {W - panel_x})")
p(f"   last wall pixel before the panel: x={shelf_right}  ->  gap {panel_x - shelf_right - 1} px")
p("   colour at the seam: " + " ".join(hexs(imi.rgb(x, PROBES[0])) for x in range(panel_x - 3, panel_x + 2)))
p("   panel content, row bands:")
for s, e in yr(imi, panel_x, W, BODY, RULE2, PLINTH, thr=3, gap=1):
    box = ink_box(imi, panel_x, s, W, e, PLINTH, 6)
    p(f"      y {s:5d}..{e:5d}  h {e - s:4d}   x-extent {box[0] if box else '-'}..{box[2] if box else '-'}")
p("   panel elements, x edges by band:")
for s, e in yr(imi, panel_x, W, BODY, RULE2, PLINTH, thr=3, gap=1):
    rs = xr(imi, s, e, panel_x, W, PLINTH, thr=3, gap=8)
    p(f"      y[{s},{e}) -> " + ", ".join(f"{a}..{b}" for a, b in rs))

# ============================================================ SETTINGS
p("\n### SETTINGS place")
ims = load("settings")
p("   ground probe:", hexs(ims.rgb(600, 400)), hexs(ims.rgb(30, 400)), hexs(ims.rgb(1200, 400)))
for s, e in yr(ims, 0, W, BODY, RULE2, WALL, thr=3, gap=1):
    rs = xr(ims, s, e, 0, W, WALL, thr=3, gap=8)
    p(f"   y[{s:4d},{e:4d}) h{e - s:3d} -> " + ", ".join(f"{a}..{b}" for a, b in rs))
p("   top bar:")
for s, e in yr(ims, 0, W, 0, TOPB, WALL, thr=3, gap=1):
    rs = xr(ims, s, e, 0, W, WALL, thr=3, gap=8)
    p(f"   y[{s:4d},{e:4d}) -> " + ", ".join(f"{a}..{b}" for a, b in rs))

# ============================================================ QUEUE
p("\n### QUEUE popover (playing)")
imq = load("queue-playing")
qx = None
for x in range(W - 1, 0, -1):
    if dist(imq.rgb(x, 700), LIT) <= 3:
        qx = x
        break
# find the popover box by scanning for the lit surface
# The popover's own surface, found over the whole body rather than at a row the
# bar's height used to put it at.
ys = [y for y in range(BODY, RULE2) if dist(imq.rgb(W - 60, y), LIT) <= 3]
mid = (min(ys) + max(ys)) // 2 if ys else RULE2 - 40
xs = [x for x in range(W) if dist(imq.rgb(x, mid), LIT) <= 3]
p(f"   popover surface x {min(xs)}..{max(xs) + 1} (w {max(xs) + 1 - min(xs)}), "
  f"y {min(ys)}..{max(ys) + 1} (h {max(ys) + 1 - min(ys)})")
for s, e in yr(imq, min(xs), max(xs) + 1, min(ys), max(ys) + 1, LIT, thr=3, gap=1):
    rs = xr(imq, s, e, min(xs), max(xs) + 1, LIT, thr=3, gap=8)
    p(f"   y[{s:4d},{e:4d}) h{e - s:3d} -> " + ", ".join(f"{a}..{b}" for a, b in rs))

# ============================================================ FIRST RUN
p("\n### FIRST RUN")
imf = load("first-run")
rs = xr(imf, 0, H, 0, W, WALL, thr=3, gap=10)
p("   x runs:", rs)
for s, e in yr(imf, 0, W, 0, H, WALL, thr=3, gap=2):
    b = ink_box(imf, 0, s, W, e, WALL, 3)
    p(f"   y[{s:4d},{e:4d}) h{e - s:3d}  x {b[0]}..{b[2]}")
bb = ink_box(imf, 0, 0, W, H, WALL, 3)
p(f"   block box {bb}   block centre ({(bb[0] + bb[2]) / 2:.1f}, {(bb[1] + bb[3]) / 2:.1f})"
  f"   window centre ({W / 2:.1f}, {H / 2:.1f})")
cov, cx, cy, _ = ink_mass(imf, 0, 0, W, H, WALL, 8)
p(f"   ink coverage {cov:.4%}   ink centroid ({cx:.1f}, {cy:.1f})")

# ============================================================ EMPTY
p("\n### EMPTY LIBRARY")
ime = load("empty-library")
bb = ink_box(ime, 0, BODY + 7, W, RULE2 - 5, WALL, 3)
p(f"   block box {bb}  centre ({(bb[0] + bb[2]) / 2:.1f},{(bb[1] + bb[3]) / 2:.1f})"
  f"   shelf area centre ({W / 2:.1f},{(BODY + RULE2) / 2:.1f})")

# ============================================================ DENSITY
p("\n### INK DENSITY (share of pixels differing from the region's ground by >8)")
for name, imx, box, g in (
    ("top bar", im, (0, 0, W, TOPB), WALL),
    ("bottom bar idle", im, (0, BAR, W, H), RECESS),
    ("bottom bar playing", imp, (0, BAR, W, H), RECESS),
    ("wall (whole body)", im, (0, BODY, W, RULE2), WALL),
    ("inspector panel", imi, (panel_x, BODY, W, RULE2), PLINTH),
    ("settings body", ims, (0, BODY, W, RULE2), WALL),
    ("first run", imf, (0, 0, W, H), WALL),
):
    cov, cx, cy, mass = ink_mass(imx, box[0], box[1], box[2], box[3], g, 8)
    area = (box[2] - box[0]) * (box[3] - box[1])
    p(f"   {name:20s} coverage {cov:7.2%}   centroid ({cx:7.1f},{cy:7.1f})  area {area}")
