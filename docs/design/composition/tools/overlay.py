#!/usr/bin/env python3
"""Draw the rulers onto the frames. Emits and runs ImageMagick draw commands."""
import subprocess
import sys
import os

SHOTS = "/tmp/baz-comp-shots"
OUT = sys.argv[1]
os.makedirs(OUT, exist_ok=True)

SHARED = "#5FA8D3"  # an edge two or more elements sit on
LONE = "#E3776B"  # an edge nothing else shares
UNIT = "#E3A14E"  # a rhythm / centre line
NOTE = "#E8E4DB"


def draw(src, dst, cmds, crop=None, scale=None):
    args = ["magick", f"{SHOTS}/{src}.png"]
    args += ["-font", "DejaVu-Sans", "-pointsize", "11"]
    args += cmds
    if crop:
        args += ["-crop", crop, "+repage"]
    if scale:
        args += ["-resize", scale]
    args += [f"{OUT}/{dst}.png"]
    subprocess.run(args, check=True)


def vline(x, y0, y1, colour, label=None, ly=None, anchor="start"):
    out = [["-stroke", colour, "-strokewidth", "1", "-fill", "none",
            "-draw", f"line {x},{y0} {x},{y1}"]]
    if label:
        lx = x + 3 if anchor == "start" else x - 3
        out.append(["-stroke", "none", "-fill", colour,
                    "-draw", f"text {lx},{ly or y0 + 12} '{label}'"])
    return out


def hline(y, x0, x1, colour, label=None, lx=None):
    out = [["-stroke", colour, "-strokewidth", "1", "-fill", "none",
            "-draw", f"line {x0},{y} {x1},{y}"]]
    if label:
        out.append(["-stroke", "none", "-fill", colour,
                    "-draw", f"text {lx or x0 + 4},{y - 4} '{label}'"])
    return out


def caption(x, y, text, colour=NOTE, size=13):
    return [["-stroke", "none", "-fill", colour, "-pointsize", str(size),
             "-draw", f"text {x},{y} '{text}'"], ["-pointsize", "11"]]


def flat(groups):
    return [c for g in groups for c in g]


# ------------------------------------------------------- 1. wall, 1280
cmds = []
for x, lab, col in [
    (16, "16 search well", LONE),
    (40, "40 HANG", SHARED),
    (310, "310", SHARED),
    (350, "350", SHARED),
    (620, "620", SHARED),
    (660, "660", SHARED),
    (930, "930", SHARED),
    (970, "970", SHARED),
    (1240, "1240", SHARED),
    (1270, "1270 scrollbar", LONE),
    (1048, "1048 counts", LONE),
    (1200, "1200 Settings", LONE),
    (1264, "1264", SHARED),
    (376, "376", LONE),
]:
    cmds += vline(x, 0, 758, col, lab, ly=(70 if col == LONE else 84))
cmds += hline(52, 0, 1280, SHARED, "y 52  top-bar rule")
cmds += hline(93, 0, 1280, SHARED, "y 93  first work")
cmds += hline(758, 0, 1280, SHARED, "y 758  bar rule")
cmds += caption(20, 800, "wall 1280x860 - 14 distinct x-edges above the bar; 6 are shared by the hang, 8 are singletons")
draw("wall-rest-1280x860", "01-wall-edges-1280", flat(cmds))

# ------------------------------------------------------- 2. bottom bar, 1280
cmds = []
for y, lab, col, lx in [
    (787, "y 787  transport glyphs  -22.5", LONE, 700),
    (810, "y 810.5  left-zone text  +1.0", LONE, 150),
    (816, "y 816  volume + mute  +6.5", LONE, 950),
    (837, "y 837  seek groove  +27.5", LONE, 560),
    (809, "y 809.5  the bar mid-line", UNIT, 380),
]:
    cmds += hline(int(y), 0, 1280, col, lab, lx=lx)
cmds += vline(16, 759, 860, SHARED, "16")
cmds += vline(290, 759, 860, LONE, "290 Queue", ly=852)
cmds += vline(434, 759, 860, SHARED, "434 zone edge", ly=770)
cmds += vline(450, 759, 860, SHARED, "450")
cmds += vline(830, 759, 860, SHARED, "830")
cmds += vline(846, 759, 860, SHARED, "846 zone edge", ly=770)
cmds += vline(1264, 759, 860, SHARED, "1264", anchor="end")
cmds += caption(16, 772, "the bottom bar, playing: four mark-lines in one 102 px band, and none of them is the mid-line")
draw("wall-playing-1280x860", "02-bar-centrelines-1280", flat(cmds),
     crop="1280x114+0+752")

# ------------------------------------------------------- 3. top bar, 1280
cmds = []
cmds += hline(30, 900, 1280, SHARED, "y 30  counts baseline", lx=905)
cmds += hline(22, 900, 1280, LONE, "y 22  Settings baseline  -8", lx=905)
cmds += hline(26, 0, 900, UNIT, "y 26  the row centre line", lx=420)
cmds += vline(16, 0, 52, SHARED, "16")
cmds += vline(376, 0, 52, LONE, "376")
cmds += vline(1048, 0, 52, LONE, "1048")
cmds += vline(1200, 0, 52, LONE, "1200")
cmds += vline(1264, 0, 52, SHARED, "1264", anchor="end")
cmds += caption(16, 48, "the top bar: the well is 30 px where every other control is 32; Settings sits 8 px above the counts it shares a row with")
draw("wall-rest-1280x860", "03-topbar-baselines-1280", flat(cmds), crop="1280x60+0+0")

# ------------------------------------------------------- 4. inspector, 1280
cmds = []
for x, lab, col in [
    (939, "939 rule", SHARED),
    (964, "964 content L", SHARED),
    (985, "985 track no.", LONE),
    (1001, "1001 title", LONE),
    (1217, "1217 duration L", LONE),
    (1242, "1242 duration R", LONE),
    (1246, "1246 scrollbar", LONE),
    (1256, "1256 content R", SHARED),
]:
    cmds += vline(x, 53, 758, col, lab, ly=(120 if col == LONE else 140))
for y, lab in [(121, "121 sleeve"), (413, "413"), (430, "430 title"),
               (456, "456 artist"), (480, "480 catalogue"), (500, "500 condition"),
               (524, "524 Play album"), (568, "568 tracks"), (722, "722 footnote")]:
    cmds += hline(y, 940, 1280, UNIT, lab, lx=944)
cmds += caption(300, 700, "the inspector: 8 distinct x-edges in a 340 px column, 5 of them singletons")
draw("inspector-1280x860", "04-inspector-edges-1280", flat(cmds))

# ------------------------------------------------------- 5. settings, 1280
cmds = []
for x, lab, col in [
    (24, "24 back + nav", SHARED),
    (93, "93 place title", LONE),
    (224, "224 nav R", LONE),
    (248, "248 content L", SHARED),
    (878, "878 content R", SHARED),
    (1264, "1264 status R", LONE),
]:
    cmds += vline(x, 0, 758, col, lab, ly=(400 if col == LONE else 420))
cmds += hline(52, 0, 1280, SHARED, "52")
cmds += caption(300, 500, "Settings at 1280: content ends at x 878 (0.686 W). At 1920 it ends at the same 878 (0.457 W).")
cmds += caption(300, 520, "The one line of type above it is right-aligned to 1264 and shares no edge with anything below.")
draw("settings-1280x860", "05-settings-edges-1280", flat(cmds))

# ------------------------------------------------------- 6. queue, 1280
cmds = []
for x, lab, col in [
    (905, "905 popover", SHARED),
    (920, "920 header", SHARED),
    (924, "924 album title", LONE),
    (941, "941 track no.", LONE),
    (1181, "1181 duration", LONE),
    (1210, "1210 row R", LONE),
    (1247, "1247 content R", SHARED),
    (1263, "1263", SHARED),
]:
    cmds += vline(x, 353, 741, col, lab, ly=(370 if col == LONE else 390))
cmds += caption(300, 500, "the queue popover: four left edges in a 358 px panel - 920, 924, 925, 941")
draw("queue-playing-1280x860", "06-queue-edges-1280", flat(cmds))

# ------------------------------------------------------- 7. first run
cmds = []
cmds += vline(640, 0, 860, UNIT, "640 window centre", ly=200)
cmds += vline(547, 0, 860, LONE, "547 ink centroid  -93", ly=230)
cmds += vline(410, 300, 560, SHARED, "410 block L")
cmds += vline(870, 300, 560, SHARED, "870 block R", anchor="end")
cmds += hline(430, 0, 1280, UNIT, "430 window centre", lx=900)
cmds += hline(417, 0, 1280, LONE, "417 ink centroid  -13", lx=900)
cmds += hline(341, 300, 1000, SHARED, "341 block top = 0.397 H", lx=880)
cmds += hline(520, 300, 1000, SHARED, "520 block bottom", lx=880)
cmds += caption(120, 620, "first run: the block is centred to the pixel; its ink is not. 93 px of the 460 px well are empty on the right.")
draw("first-run-1280x860", "07-first-run-centring-1280", flat(cmds))

# ------------------------------------------------------- 8. wall, 1920
cmds = []
for x, lab, col in [
    (40, "40", SHARED), (314, "314", SHARED), (354, "354", SHARED),
    (627, "627", SHARED), (667, "667", SHARED), (941, "941", SHARED),
    (981, "981", SHARED), (1254, "1254", SHARED), (1294, "1294", SHARED),
    (1567, "1567", SHARED), (1607, "1607", SHARED), (1880, "1880", SHARED),
    (1910, "1910 scrollbar", LONE), (16, "16", LONE), (376, "376", LONE),
]:
    cmds += vline(x, 0, 978, col, lab, ly=(70 if col == LONE else 88))
cmds += caption(20, 1010, "wall 1920x1080: the hang holds at 40 | 273 | 40 | ... ; the right margin is 30 px of wall plus a 10 px scrollbar lane")
draw("wall-rest-1920x1080", "08-wall-edges-1920", flat(cmds))

# ------------------------------------------------------- 9. inspector, 1920
cmds = []
for x, lab, col in [
    (1579, "1579 rule", SHARED), (1604, "1604 content L", SHARED),
    (1625, "1625 track no.", LONE), (1857, "1857 duration L", LONE),
    (1882, "1882 duration R", LONE), (1896, "1896 content R", SHARED),
]:
    cmds += vline(x, 53, 978, col, lab, ly=(120 if col == LONE else 140))
cmds += caption(300, 900, "the inspector does not respond to width: 340 px at 1280 and at 1920, same internal edges")
draw("inspector-1920x1080", "09-inspector-edges-1920", flat(cmds))

# ------------------------------------------------------- 10. bar, 1920
cmds = []
for y, lab, col, lx in [
    (1007, "y 1007  transport glyphs  -22.5", LONE, 1080),
    (1029, "y 1029.5  the bar mid-line", UNIT, 400),
    (1036, "y 1036  volume + mute  +6.5", LONE, 1400),
    (1057, "y 1057  seek groove  +27.5", LONE, 760),
]:
    cmds += hline(int(y), 0, 1920, col, lab, lx=lx)
cmds += caption(16, 995, "the same four lines at 1920: the bar zones are centred as blocks, so their marks are not")
draw("wall-playing-1920x1080", "10-bar-centrelines-1920", flat(cmds),
     crop="1920x114+0+972")

print("overlays written to", OUT)
