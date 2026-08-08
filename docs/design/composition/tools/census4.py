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
# The two chrome bands, found rather than assumed — their heights are tokens
# now, so the ruler reads them off the frame instead of remembering 53 and 102.
TOPB = next(y for y in range(12, 120) if dist(im.rgb(5, y), (0x0C, 0x0D, 0x0E)) > 2)
BODY = TOPB + 1
print(f"############ {TAG}   top bar h {TOPB + 1}   bar h {H - BAR + 1}")

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
    rs = yruns(imx, x0, x1, BODY, RULE2, thr=3, gap=0, ground=WALL)[1]
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
edges |= {52, BODY, RULE2, BAR}
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
ie = chrome_yedges(imi, panel_x, W, BODY, RULE2, PLINTH)
print("\n   inspector y-edges:", sorted(ie))
for r, u, ph in best_unit(sorted(ie), 2, 24)[:6]:
    print(f"      {r:6.3f}  unit {u:3d}  phase {ph:3d}")

ims = load("settings")
se = chrome_yedges(ims, 0, W, BODY, RULE2, WALL)
print("\n   settings y-edges:", sorted(se))
for r, u, ph in best_unit(sorted(se), 2, 24)[:6]:
    print(f"      {r:6.3f}  unit {u:3d}  phase {ph:3d}")

imq = load("queue-playing")
# The popover's surface, found over the whole body rather than at a row a
# fixed bar height used to put it at.
_qy = [y for y in range(BODY, RULE2) if dist(imq.rgb(W - 60, y), LIT) <= 3]
_qm = (min(_qy) + max(_qy)) // 2 if _qy else RULE2 - 40
xs = [x for x in range(W) if dist(imq.rgb(x, _qm), LIT) <= 3]
qe = chrome_yedges(imq, min(xs), max(xs) + 1, BODY, RULE2, LIT)
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
print(f"   top bar {TOPB + 1} / H = {(TOPB + 1) / H:.4f}")
print(f"   bottom bar {H - RULE2} / H = {(H - RULE2) / H:.4f}")
print(f"   body {RULE2 - BODY} / H = {(RULE2 - BODY) / H:.4f}")
print(f"   collection share (body) = {(RULE2 - BODY) / H:.4f}")
print(f"   inspector {W - panel_x} / W = {(W - panel_x) / W:.4f}   "
      f"shelf {panel_x} / W = {panel_x / W:.4f}")
# The sleeve, measured: it is the tallest recess band in the panel's upper
# half, and it is square. It was a hardcoded 292 — the panel minus its two
# paddings — which is precisely the number the audit's defect 5 is about.
_sb = [(a, b) for a, b in yruns(imi, panel_x, W, BODY, RULE2, thr=3, gap=1, ground=PLINTH)[1]
       if b - a > 40]
art = (_sb[0][1] - _sb[0][0]) if _sb else 0
print(f"   inspector: sleeve {art} / panel {W - panel_x} = {art / (W - panel_x):.4f}")
imx = load("wall-rest")
x1 = next(x for x in range(41, W) if dist(imx.rgb(x, 200), WALL) <= 2)
a = x1 - 40
# The label block, measured: the tile's row pitch, less its art, less the gap
# above the label and the row's trailing hang. It was a hardcoded 36.4 and it is
# 40 now — one `HANG` — because the body's line box is on the lattice (§2.1).
_tb = yruns(imx, 40, x1, BODY, RULE2, thr=3, gap=0, ground=WALL)[1]
_arts = [(s0, e0) for s0, e0 in _tb if e0 - s0 > a - 8]
_pitch = (_arts[1][0] - _arts[0][0]) if len(_arts) > 1 else 0
LABEL_H = _pitch - a - 16 - 40 if _pitch else 0
# The fixture's first two rows can straddle a shelf break, in which case the
# pitch carries the header's band as well. One `HANG` of it, and it is the only
# thing that can sit between two rows of covers.
_broke = LABEL_H > 60
if _broke:
    LABEL_H -= 40
print("   tile: art / (art + GAP_LG + LABEL_H) = ", end="")
print(f"{a / (a + 16 + LABEL_H):.4f}  (art {a}, row pitch {_pitch}"
      f"{' incl. a shelf break' if _broke else ''}, label block {LABEL_H})")
# The form's right edge, measured off the widest control in the place rather
# than remembered as 878 — which was the whole of the audit's defect 9.
_bands = yruns(ims, 0, W, BODY, RULE2, thr=3, gap=1, ground=WALL)[1]
_edges = []
for _s, _e in _bands:
    for _a, _b in xruns(ims, _s, _e, 0, W, thr=3, gap=8, ground=WALL)[1]:
        _edges.append(_b)
_right = max(_edges) if _edges else 0
print(f"   settings content right edge {_right} / W = {_right / W:.4f}")
_pys = [y for y in range(BODY, RULE2) if dist(imq.rgb(W - 60, y), LIT) <= 3]
_pw = max(xs) + 1 - min(xs) if xs else 0
_ph = max(_pys) + 1 - min(_pys) if _pys else 0
print(f"   queue popover {_pw}x{_ph};  {_ph} / body {RULE2 - BODY} = "
      f"{_ph / max(RULE2 - BODY, 1):.4f}")

# ------------------------------------------------------------- SYMMETRY
print("\n### SYMMETRY — bottom bar zone widths (playing)")
# zones: left fill | centre SEEK_ROW_W | right fill, GAP_LG apart, pad GAP_LG
inner_l, inner_r = 40, W - 40  # the one window gutter (law L1)
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
