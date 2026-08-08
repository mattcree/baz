#!/usr/bin/env python3
"""Vertical rhythm, tile state marks, proportion."""
import sys

sys.path.insert(0, __import__("os").path.dirname(__import__("os").path.abspath(__file__)))
from ruler import Img, hexs, dist, ink_box, ink_mass, best_unit  # noqa: E402
from bands import xruns, yruns  # noqa: E402

SHOTS = "/tmp/baz-comp-shots"
W, H = (int(v) for v in sys.argv[1].split("x"))
TAG = f"{W}x{H}"
WALL = (0x0C, 0x0D, 0x0E)
RECESS = (0x06, 0x07, 0x08)
PLINTH = (0x14, 0x15, 0x17)
LIT = (0x1C, 0x1D, 0x20)
load = lambda n: Img(f"{SHOTS}/{n}-{TAG}.png")  # noqa: E731

im = load("wall-rest")
BAR = next(y + 1 for y in range(H - 1, H - 200, -1) if dist(im.rgb(5, y), RECESS) > 2)
RULE2 = BAR - 1
print(f"############ {TAG}")

# ------------------------------------------------------------- TILE geometry
print("\n### TILE — one column's vertical structure (rest / hover / selected / playing)")
for nm, shot, col in (
    ("rest", "wall-rest", 40),
    ("hover", "wall-hover", 40),
    ("selected", "wall-selected", 40),
    ("playing", "wall-playing", None),
):
    imx = load(shot)
    # find column-1 art extent on the top row
    x0 = 40
    x1 = next(x for x in range(41, W) if dist(imx.rgb(x, 200), WALL) <= 2)
    rs = yruns(imx, x0, x1, 53, RULE2, thr=3, gap=0, ground=WALL)[1]
    print(f"   {nm:9s} art x {x0}..{x1} (w {x1 - x0}) ->")
    for s, e in rs[:8]:
        print(f"        y {s:5d}..{e:5d}  h {e - s:4d}")

print("\n### TILE — hover / selected rule, measured")
for nm, shot in (("rest", "wall-rest"), ("hover", "wall-hover"), ("selected", "wall-selected")):
    imx = load(shot)
    band = [
        (y, hexs(imx.rgb(100, y)))
        for y in range(410, 430)
        if dist(imx.rgb(100, y), WALL) > 2
    ]
    print(f"   {nm:9s} rule rows {band}")

# ------------------------------------------------------------- VERTICAL RHYTHM
print("\n### VERTICAL RHYTHM — every chrome y-edge in the frame (artwork excluded)")


def chrome_yedges(imx, x0, x1, y0, y1, g):
    out = set()
    for s, e in yruns(imx, x0, x1, y0, y1, thr=3, gap=0, ground=g)[1]:
        out.add(s)
        out.add(e)
    return out


edges = set()
edges |= chrome_yedges(im, 0, W, 0, 53, WALL)  # top bar
edges |= {52, 53, RULE2, BAR}
edges |= chrome_yedges(im, 0, W, BAR, H, RECESS)  # bar idle
imp = load("wall-playing")
edges |= chrome_yedges(imp, 0, W, BAR, H, RECESS)
print("   top bar + bottom bar y-edges:", sorted(edges))
fits = best_unit(sorted(edges), 2, 24)
print("   best-fit vertical units (mean |residual| px, unit, phase):")
for r, u, ph in fits[:8]:
    print(f"      {r:6.3f}  unit {u:3d}  phase {ph:3d}")

imi = load("inspector")
panel_x = next(x for x in range(W - 1, 0, -1) if dist(imi.rgb(x, RULE2 - 6), PLINTH) > 2) + 1
ie = chrome_yedges(imi, panel_x, W, 53, RULE2, PLINTH)
print("\n   inspector y-edges:", sorted(ie))
for r, u, ph in best_unit(sorted(ie), 2, 24)[:6]:
    print(f"      {r:6.3f}  unit {u:3d}  phase {ph:3d}")

ims = load("settings")
se = chrome_yedges(ims, 0, W, 53, RULE2, WALL)
print("\n   settings y-edges:", sorted(se))
for r, u, ph in best_unit(sorted(se), 2, 24)[:6]:
    print(f"      {r:6.3f}  unit {u:3d}  phase {ph:3d}")

imq = load("queue-playing")
xs = [x for x in range(W) if dist(imq.rgb(x, RULE2 - 20), LIT) <= 3]
qe = chrome_yedges(imq, min(xs), max(xs) + 1, 53, RULE2, LIT)
print("\n   queue popover y-edges:", sorted(qe))
for r, u, ph in best_unit(sorted(qe), 2, 24)[:6]:
    print(f"      {r:6.3f}  unit {u:3d}  phase {ph:3d}")

allx = sorted(edges | ie | se | qe)
print("\n   ALL chrome y-edges pooled:", len(allx))
for r, u, ph in best_unit(allx, 2, 24)[:8]:
    print(f"      {r:6.3f}  unit {u:3d}  phase {ph:3d}")

# ------------------------------------------------------------- PROPORTION
print("\n### PROPORTION")
print(f"   window {W}x{H}")
print(f"   top bar 53 / H = {53 / H:.4f}")
print(f"   bottom bar {H - RULE2} / H = {(H - RULE2) / H:.4f}")
print(f"   body {RULE2 - 53} / H = {(RULE2 - 53) / H:.4f}")
print(f"   collection share (body) = {(RULE2 - 53) / H:.4f}")
print(f"   inspector {W - panel_x} / W = {(W - panel_x) / W:.4f}   "
      f"shelf {panel_x} / W = {panel_x / W:.4f}")
art = 292
print(f"   inspector: sleeve {art} / panel {W - panel_x} = {art / (W - panel_x):.4f}")
print(f"   tile: art / (art + GAP_LG + LABEL_H) = ", end="")
imx = load("wall-rest")
x1 = next(x for x in range(41, W) if dist(imx.rgb(x, 200), WALL) <= 2)
a = x1 - 40
print(f"{a / (a + 16 + 36.4):.4f}  (art {a})")
print(f"   settings content right edge 878 / W = {878 / W:.4f}")
print(f"   queue popover 358x388;  388 / body {RULE2 - 53} = {388 / (RULE2 - 53):.4f}")

# ------------------------------------------------------------- SYMMETRY
print("\n### SYMMETRY — bottom bar zone widths (playing)")
# zones: left fill | centre SEEK_ROW_W | right fill, GAP_LG apart, pad GAP_LG
inner_l, inner_r = 16, W - 16
centre_w = 380
fill = (inner_r - inner_l - centre_w - 2 * 16) / 2
print(f"   content x[{inner_l},{inner_r}) w {inner_r - inner_l}")
print(f"   left fill {fill}, centre {centre_w}, right fill {fill}")
for nm, shot in (("idle", "wall-rest"), ("playing", "wall-playing")):
    imx = load(shot)
    lb = ink_box(imx, inner_l, BAR, int(inner_l + fill), H, RECESS, 10)
    cb = ink_box(imx, int(inner_l + fill + 16), BAR, int(inner_l + fill + 16 + centre_w), H, RECESS, 10)
    rb = ink_box(imx, int(inner_r - fill), BAR, inner_r, H, RECESS, 10)
    print(f"   {nm:8s} left ink {lb}  centre ink {cb}  right ink {rb}")
    if lb and rb:
        print(f"            left  ink starts {lb[0] - inner_l:+4d} from the inner edge, "
              f"right ink ends {inner_r - rb[2]:+4d} from it")

# ------------------------------------------------------------- DENSITY
print("\n### DENSITY")
for name, imx, box, g in (
    ("top bar", im, (0, 0, W, 52), WALL),
    ("bottom bar idle", im, (0, BAR, W, H), RECESS),
    ("bottom bar playing", imp, (0, BAR, W, H), RECESS),
    ("wall body", im, (0, 53, W, RULE2), WALL),
    ("wall body, chrome only", im, (0, 53, W, RULE2), WALL),
    ("inspector panel", imi, (panel_x, 53, W, RULE2), PLINTH),
    ("settings body", ims, (0, 53, W, RULE2), WALL),
    ("queue popover", imq, (min(xs), 353 if H < 1000 else 573, max(xs) + 1,
                            741 if H < 1000 else 961), LIT),
    ("first run", load("first-run"), (0, 0, W, H), WALL),
    ("empty library", load("empty-library"), (0, 53, W, RULE2), WALL),
):
    cov, cx, cy, mass = ink_mass(imx, *box, g, 8)
    print(f"   {name:24s} coverage {cov:7.2%}  centroid "
          f"({cx if cx is None else round(cx, 1)},{cy if cy is None else round(cy, 1)})")
