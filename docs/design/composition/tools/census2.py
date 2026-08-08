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
p(f"############ {TAG}   body y[53,{RULE2})   bar y[{BAR},{H})  bar h {H - BAR + 1}")

# ============================================================ TOP BAR
p("\n### TOP BAR — element boxes (surface + ink), rows 0..52, ground wall")
for lo, hi in xr(im, 0, 52, 0, W, WALL, thr=3, gap=6):
    ys = yr(im, lo, hi, 0, 52, WALL, thr=3, gap=1)
    box = ink_box(im, lo, 0, hi, 52, WALL, 3)
    p(f"   x {lo:5d}..{hi:5d} (w {hi - lo:4d})   ink-y {ys}   box {box}")

p("\n### TOP BAR — the two right-hand labels, ink boxes")
for name, a, b in (("counts", 1000, 1180), ("Settings", 1185, 1265)):
    box = ink_box(im, a, 0, b, 52, WALL, 20)
    p(f"   {name:9s} ink box {box}   ink-y centre {(box[1] + box[3]) / 2:.1f}")

# the search well: exact surface extent
p("\n### TOP BAR — search well surface extent (recess against wall)")
col = [y for y in range(0, 55) if dist(im.rgb(200, y), WALL) > 2]
p(f"   well rows {min(col)}..{max(col) + 1}  h {max(col) + 1 - min(col)}")
row = [x for x in range(0, 500) if dist(im.rgb(x, 26), WALL) > 2]
p(f"   well cols {min(row)}..{max(row) + 1}  w {max(row) + 1 - min(row)}")
ph = ink_box(im, 20, 12, 372, 40, RECESS, 14)
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

# ============================================================ transport glyphs
p("\n### transport glyph optical centring (16 px icon in a 32 px hit box)")
imp = load("wall-playing")
row_c = BAR + 12 + 16  # padding GAP_MD then half of TRANSPORT_HIT
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
for s, e in yr(im, art_lo, art_hi, 53, RULE2, WALL, thr=3, gap=0):
    p(f"   y {s:5d}..{e:5d}  h {e - s:4d}")

# ============================================================ INSPECTOR
p("\n### INSPECTOR")
imi = load("inspector")
panel_x = next(x for x in range(W - 1, 0, -1) if dist(imi.rgb(x, 700), PLINTH) > 2) + 1
p(f"   panel surface x {panel_x}..{W}  (w {W - panel_x})")
shelf_right = next(x for x in range(panel_x - 1, 0, -1) if dist(imi.rgb(x, 700), WALL) <= 2)
p(f"   last wall pixel before the panel: x={shelf_right}  ->  gap {panel_x - shelf_right - 1} px")
p(f"   colour at the seam: " + " ".join(hexs(imi.rgb(x, 700)) for x in range(panel_x - 3, panel_x + 2)))
p("   panel content, row bands:")
for s, e in yr(imi, panel_x, W, 53, RULE2, PLINTH, thr=3, gap=1):
    box = ink_box(imi, panel_x, s, W, e, PLINTH, 6)
    p(f"      y {s:5d}..{e:5d}  h {e - s:4d}   x-extent {box[0] if box else '-'}..{box[2] if box else '-'}")
p("   panel elements, x edges by band:")
for s, e in yr(imi, panel_x, W, 53, RULE2, PLINTH, thr=3, gap=1):
    rs = xr(imi, s, e, panel_x, W, PLINTH, thr=3, gap=8)
    p(f"      y[{s},{e}) -> " + ", ".join(f"{a}..{b}" for a, b in rs))

# ============================================================ SETTINGS
p("\n### SETTINGS place")
ims = load("settings")
p("   ground probe:", hexs(ims.rgb(600, 400)), hexs(ims.rgb(30, 400)), hexs(ims.rgb(1200, 400)))
for s, e in yr(ims, 0, W, 53, RULE2, WALL, thr=3, gap=1):
    rs = xr(ims, s, e, 0, W, WALL, thr=3, gap=8)
    p(f"   y[{s:4d},{e:4d}) h{e - s:3d} -> " + ", ".join(f"{a}..{b}" for a, b in rs))
p("   top bar:")
for s, e in yr(ims, 0, W, 0, 52, WALL, thr=3, gap=1):
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
xs = [x for x in range(W) if dist(imq.rgb(x, 740), LIT) <= 3]
ys = [y for y in range(53, RULE2) if dist(imq.rgb(1000, y), LIT) <= 3]
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
bb = ink_box(ime, 0, 60, W, RULE2 - 5, WALL, 3)
p(f"   block box {bb}  centre ({(bb[0] + bb[2]) / 2:.1f},{(bb[1] + bb[3]) / 2:.1f})"
  f"   shelf area centre ({W / 2:.1f},{(53 + RULE2) / 2:.1f})")

# ============================================================ DENSITY
p("\n### INK DENSITY (share of pixels differing from the region's ground by >8)")
for name, imx, box, g in (
    ("top bar", im, (0, 0, W, 52), WALL),
    ("bottom bar idle", im, (0, BAR, W, H), RECESS),
    ("bottom bar playing", imp, (0, BAR, W, H), RECESS),
    ("wall (whole body)", im, (0, 53, W, RULE2), WALL),
    ("inspector panel", imi, (panel_x, 53, W, RULE2), PLINTH),
    ("settings body", ims, (0, 53, W, RULE2), WALL),
    ("first run", imf, (0, 0, W, H), WALL),
):
    cov, cx, cy, mass = ink_mass(imx, box[0], box[1], box[2], box[3], g, 8)
    area = (box[2] - box[0]) * (box[3] - box[1])
    p(f"   {name:20s} coverage {cov:7.2%}   centroid ({cx:7.1f},{cy:7.1f})  area {area}")
