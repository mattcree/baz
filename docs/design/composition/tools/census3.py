#!/usr/bin/env python3
"""Optical centring, panel geometry and proportion. One window size per run."""
import sys

sys.path.insert(0, __import__("os").path.dirname(__import__("os").path.abspath(__file__)))
from ruler import Img, hexs, dist, ink_box, ink_mass  # noqa: E402
from bands import xruns, yruns  # noqa: E402

SHOTS = "/tmp/baz-comp-shots"
W, H = (int(v) for v in sys.argv[1].split("x"))
TAG = f"{W}x{H}"
WALL = (0x0C, 0x0D, 0x0E)
RECESS = (0x06, 0x07, 0x08)
PLINTH = (0x14, 0x15, 0x17)
LIT = (0x1C, 0x1D, 0x20)


def load(n):
    return Img(f"{SHOTS}/{n}-{TAG}.png")


def xr(im, y0, y1, x0, x1, g, thr=3, gap=6):
    return xruns(im, y0, y1, x0, x1, thr=thr, gap=gap, ground=g)[1]


def yr(im, x0, x1, y0, y1, g, thr=3, gap=1):
    return yruns(im, x0, x1, y0, y1, thr=thr, gap=gap, ground=g)[1]


im = load("wall-rest")
BAR = next(y + 1 for y in range(H - 1, H - 200, -1) if dist(im.rgb(5, y), RECESS) > 2)
RULE2 = BAR - 1
# The top strip's own hairline, found rather than assumed (see census2).
TOPB = next(y for y in range(12, 120) if dist(im.rgb(5, y), WALL) > 2)
BODY = TOPB + 1
print(f"############ {TAG}  top bar h {TOPB + 1}  body y[{BODY},{RULE2})  bar y[{BAR},{H})")

# ---------------------------------------------------------- the search well
print("\n### search well, exact")
col = [y for y in range(0, 50) if dist(im.rgb(200, y), WALL) > 2]
print(f"   recess rows {min(col)}..{max(col) + 1}  h {max(col) + 1 - min(col)}")
print("   column 200 colours:", " ".join(f"{y}:{hexs(im.rgb(200, y))}" for y in range(8, 46)))

# ---------------------------------------------------------- transport glyphs
print("\n### transport glyphs — ink centroid against the hit box")
BOXES_MID = W / 2
# hit boxes: three 32 px squares, GAP_SM 8 apart, centred on the bar's centre
for i, off in enumerate((-40, 0, 40)):
    cx0 = BOXES_MID + off - 16
    for imx, tag in ((im, "idle"), (load("wall-playing"), "playing")):
        box = ink_box(imx, int(cx0), BAR, int(cx0) + 32, H, RECESS, 10)
        cov, ix, iy, _ = ink_mass(imx, int(cx0), BAR, int(cx0) + 32, H, RECESS, 10)
        if box:
            print(
                f"   glyph {i} {tag:8s} hitbox x[{cx0:.0f},{cx0 + 32:.0f}) "
                f"ink box {box}  bbox-cx {(box[0] + box[2]) / 2:7.1f} "
                f"centroid-cx {ix:7.1f}  box-cx {cx0 + 16:7.1f}  "
                f"d_bbox {(box[0] + box[2]) / 2 - (cx0 + 16):+5.1f} "
                f"d_centroid {ix - (cx0 + 16):+5.1f}"
            )

# ---------------------------------------------------------- Settings button
print("\n### top-bar right cluster: the two labels on one row")
for name, a, b in (("counts", 1000, W - 108), ("Settings", W - 100, W - 10)):
    box = ink_box(im, a, 0, b, 50, WALL, 20)
    print(f"   {name:9s} ink box {box}  ink-y centre {(box[1] + box[3]) / 2:.1f}")
print("   (the row's controls are TRANSPORT_HIT 32 tall, top-padded 10 -> y 10..42,")
print("    so a centred 12/1.35 line box has its ink centre at y ~25.9)")

# ---------------------------------------------------------- INSPECTOR
print("\n### INSPECTOR")
imi = load("inspector")
probe_y = RULE2 - 6
panel_x = None
for x in range(W - 1, 0, -1):
    if dist(imi.rgb(x, probe_y), PLINTH) > 2:
        panel_x = x + 1
        break
print(f"   panel x {panel_x}..{W}  w {W - panel_x}  (probe row {probe_y})")
print("   seam colours:", " ".join(f"{x}:{hexs(imi.rgb(x, probe_y))}" for x in range(panel_x - 4, panel_x + 2)))
print("   panel rows:")
for s, e in yr(imi, panel_x, W, BODY, RULE2, PLINTH, thr=3, gap=1):
    rs = xr(imi, s, e, panel_x, W, PLINTH, thr=3, gap=10)
    print(f"      y[{s:4d},{e:4d}) h{e - s:4d} -> " + ", ".join(f"{a}..{b}" for a, b in rs))
# Play album internals
pa = [(s, e) for s, e in yr(imi, panel_x, W, BODY, RULE2, PLINTH, thr=3, gap=1) if e - s > 25 and s > 450]
if pa:
    s, e = pa[0]
    print(f"   Play album band y[{s},{e})")
    inner = ink_box(imi, panel_x + 2, s + 3, W - 2, e - 3, PLINTH, 20)
    print(f"      label+triangle ink box {inner}")
    cov, ix, iy, _ = ink_mass(imi, panel_x + 26, s + 3, W - 26, e - 3, PLINTH, 20)
    print(f"      inner ink centroid ({ix:.1f},{iy:.1f})  button centre "
          f"({(panel_x + 24 + W - 24) / 2:.1f},{(s + e) / 2:.1f})")

# ---------------------------------------------------------- SETTINGS
print("\n### SETTINGS")
ims = load("settings")
for s, e in yr(ims, 0, W, 0, RULE2, WALL, thr=3, gap=1):
    rs = xr(ims, s, e, 0, W, WALL, thr=3, gap=10)
    print(f"   y[{s:4d},{e:4d}) h{e - s:4d} -> " + ", ".join(f"{a}..{b}" for a, b in rs))

# ---------------------------------------------------------- QUEUE
print("\n### QUEUE popover")
imq = load("queue-playing")
# The popover's surface, found over the whole body rather than at a row a
# fixed bar height used to put it at.
_qy = [y for y in range(BODY, RULE2) if dist(imq.rgb(W - 60, y), LIT) <= 3]
_qm = (min(_qy) + max(_qy)) // 2 if _qy else RULE2 - 40
xs = [x for x in range(W) if dist(imq.rgb(x, _qm), LIT) <= 3]
ys = [y for y in range(BODY, RULE2) if dist(imq.rgb(int((min(xs) + max(xs)) / 2), y), LIT) <= 3]
px0, px1, py0, py1 = min(xs), max(xs) + 1, min(ys), max(ys) + 1
print(f"   popover x {px0}..{px1} (w {px1 - px0})  y {py0}..{py1} (h {py1 - py0})")
print(f"   gap to window right {W - px1}, gap to bar {BAR - py1}")
for s, e in yr(imq, px0, px1, py0, py1, LIT, thr=3, gap=1):
    rs = xr(imq, s, e, px0, px1, LIT, thr=3, gap=10)
    print(f"   y[{s:4d},{e:4d}) h{e - s:4d} -> " + ", ".join(f"{a}..{b}" for a, b in rs))

# ---------------------------------------------------------- FIRST RUN
print("\n### FIRST RUN")
imf = load("first-run")
for s, e in yr(imf, 0, W, 0, H, WALL, thr=3, gap=2):
    b = ink_box(imf, 0, s, W, e, WALL, 3)
    print(f"   y[{s:4d},{e:4d}) h{e - s:3d}   x {b[0]}..{b[2]}  w {b[2] - b[0]}")
bb = ink_box(imf, 0, 0, W, H, WALL, 3)
cov, cx, cy, _ = ink_mass(imf, 0, 0, W, H, WALL, 8)
print(f"   block box {bb}  block centre ({(bb[0] + bb[2]) / 2:.1f},{(bb[1] + bb[3]) / 2:.1f})")
print(f"   window centre ({W / 2:.1f},{H / 2:.1f})   ink centroid ({cx:.1f},{cy:.1f})")
print(f"   block top {bb[1]} = {bb[1] / H:.3f} H;  block centre = {((bb[1] + bb[3]) / 2) / H:.3f} H")

# ---------------------------------------------------------- EMPTY / NO MATCH
for nm in ("empty-library", "search-no-match"):
    ime = load(nm)
    bb = ink_box(ime, 0, 60, W, RULE2 - 5, WALL, 3)
    print(f"\n### {nm}: block {bb}  centre "
          f"({(bb[0] + bb[2]) / 2:.1f},{(bb[1] + bb[3]) / 2:.1f})  "
          f"shelf centre ({W / 2:.1f},{(53 + RULE2) / 2:.1f})")
    for s, e in yr(ime, 0, W, 60, RULE2 - 5, WALL, thr=3, gap=2):
        b = ink_box(ime, 0, s, W, e, WALL, 3)
        print(f"      y[{s},{e}) x {b[0]}..{b[2]}")
