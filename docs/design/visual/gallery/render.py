#!/usr/bin/env python3
"""Draw the gallery-direction mockups from the tokens themselves.

Not application code — this lives under `docs/` and emits the SVGs beside it.
Every colour, size and gap below is copied from `.interface-design/system.md`
§2–§9, so the mockups are a *build target*: if the shipped app disagrees with
one of these pictures, one of the two is wrong and the token says which.

    python3 docs/design/visual/gallery/render.py

The grid arithmetic in `hang()` is the specification's, verbatim, so the column
counts and art sizes in the pictures are computed rather than drawn by eye.
"""

import itertools
import os

OUT = os.path.dirname(os.path.abspath(__file__))
_ids = itertools.count()          # deterministic element ids, run to run

# --------------------------------------------------------------------------
# Tokens (.interface-design/system.md §2, §3, §4, §6, §7, §8)
# --------------------------------------------------------------------------
RECESS, WALL, PLINTH, PLINTH_LIT = '#060708', '#0C0D0E', '#141517', '#1C1D20'
PAPER, PAPER_DIM, PAPER_FAINT, PAPER_MUTED = '#E8E4DB', '#ABA8A1', '#888680', '#6C6A66'
LAMP, LAMP_BRIGHT, LAMP_INK, ALERT = '#E3A14E', '#F1B362', '#1B140B', '#D9776B'
HAIRLINE_A, HAIRLINE_STRONG_A = 0.07, 0.15
LAMP_GLOW_A = 0.45
HALO_BLUR = 24          # was 16: it is now the only shadow in the product

GAP_XXS, GAP_XS, GAP_SM, GAP_MD, GAP_LG, GAP_XL = 2, 4, 8, 12, 16, 24
# The density control (03-interface-prior-art.md R7). Three named steps; the
# hang's four numbers are all a function of the step, so the whole grid
# parameterises rather than the user overriding a designer's constant.
#   step -> (HANG, ART_MIN, ART_TARGET, ART_MAX)
DENSITY = {
    'Spacious': (48.0, 288.0, 320.0, 320.0),
    'Balanced': (40.0, 240.0, 272.0, 320.0),      # the default
    'Dense':    (28.0, 176.0, 200.0, 240.0),
}
HANG, ART_MIN, ART_TARGET, ART_MAX = DENSITY['Balanced']
THUMB_PX = 320.0        # == max ART_MAX over every step: nothing upscales, ever
INDEX_W = 20.0          # the spine index rail (R8)

SIZE_CAPTION, SIZE_META, SIZE_BODY = 11, 12, 13
SIZE_EMPHASIS, SIZE_TITLE, SIZE_HERO = 15, 22, 32
LABEL_LINE_H = SIZE_BODY * 1.40          # 18.2
LABEL_H = 2 * LABEL_LINE_H               # 36.4

RADIUS_SEGMENT, RADIUS_CHIP, RADIUS_CTRL = 3, 3, 4
DOT = 6
RAIL, KNOB, KNOB_ACTIVE = 4, 5, 7
HIT_SLOP = 9
RAIL_HIT = RAIL + 2 * HIT_SLOP           # 22
PREVIEW_H = 15
TRANSPORT_HIT, ICON_PX = 32, 16
STAMP_W, SEEK_W = 52, 260
SEEK_ROW_W = SEEK_W + 2 * (STAMP_W + GAP_SM)   # 380
SEEK_ROW_H = PREVIEW_H + RAIL_HIT              # 37
SIGNAL_W, LEVEL_W, PREVIEW_W = 96, 48, 48
VOLUME_W = 96
VOLUME_BLOCK_W = TRANSPORT_HIT + GAP_SM + VOLUME_W   # 136
POSITION_W = 56
TRACK_NO_W = 24
SCROLLBAR_W = 10

TOP_BAR_H = 56
BAR_H = 102
INSPECTOR_MIN_W, INSPECTOR_MAX_W = 340, 420

SANS = 'IBM Plex Sans'


# --------------------------------------------------------------------------
# The hang (system.md §7) — the specification's arithmetic, verbatim
# --------------------------------------------------------------------------
def hang(w, density='Balanced'):
    """The grid, from the *grid width* — see `grid_width()`, which is not the
    window width. `floor(x + 0.5)`, never a language's `round`: Python's
    banker's rounding would send 5.5 columns to 6 and 4.5 to 4."""
    hg, amin, atgt, amax = DENSITY[density]
    cap = max(1, int((w - hg) // (amin + hg)))
    n = max(1, min(int((w + hg) / (atgt + hg) + 0.5), cap))
    art = min(amax, (w - (n + 1) * hg) / n)
    gut = (w - 2 * hg - n * art) / (n - 1) if n > 1 else 0.0
    margin = hg
    if gut > 2 * hg:
        gut = 2 * hg
        margin = (w - (n * art + (n - 1) * gut)) / 2
    return n, art, gut, margin, art + GAP_LG + LABEL_H + hg


def grid_width(window_w, with_inspector):
    """What the hang actually lays out in: the content area less the two lanes
    the shelf keeps clear on its right — the scrollbar's and the index's."""
    return (window_w - (inspector_w(window_w) if with_inspector else 0)
            - SCROLLBAR_W - INDEX_W)


def inspector_w(window_w):
    return max(INSPECTOR_MIN_W, min(INSPECTOR_MAX_W, int(0.28 * window_w + 0.5)))


# --------------------------------------------------------------------------
# SVG helpers
# --------------------------------------------------------------------------
class Svg:
    def __init__(self, w, h, bg=WALL, title=''):
        self.w, self.h = w, h
        self.parts = []
        self.defs = []
        self.title = title
        self.rect(0, 0, w, h, bg)

    def raw(self, s):
        self.parts.append(s)

    def rect(self, x, y, w, h, fill, r=0, opacity=None, stroke=None, sw=1):
        o = f' fill-opacity="{opacity}"' if opacity is not None else ''
        s = f' stroke="{stroke}" stroke-width="{sw}"' if stroke else ''
        rr = f' rx="{r}"' if r else ''
        self.parts.append(
            f'<rect x="{x:.2f}" y="{y:.2f}" width="{w:.2f}" height="{h:.2f}"'
            f' fill="{fill}"{o}{rr}{s}/>')

    def circle(self, cx, cy, r, fill, opacity=None):
        o = f' fill-opacity="{opacity}"' if opacity is not None else ''
        self.parts.append(
            f'<circle cx="{cx:.2f}" cy="{cy:.2f}" r="{r:.2f}" fill="{fill}"{o}/>')

    def text(self, x, y, s, size=SIZE_BODY, fill=PAPER, weight=400,
             anchor='start', family=SANS, opacity=None, spacing=None):
        esc = (s.replace('&', '&amp;').replace('<', '&lt;').replace('>', '&gt;'))
        o = f' fill-opacity="{opacity}"' if opacity is not None else ''
        a = f' text-anchor="{anchor}"' if anchor != 'start' else ''
        ls = f' letter-spacing="{spacing}"' if spacing else ''
        self.parts.append(
            f'<text x="{x:.2f}" y="{y:.2f}" font-family="{family}"'
            f' font-size="{size}" font-weight="{weight}" fill="{fill}"'
            f'{o}{a}{ls}>{esc}</text>')

    def line(self, x1, y1, x2, y2, stroke, sw=1, opacity=None, dash=None):
        o = f' stroke-opacity="{opacity}"' if opacity is not None else ''
        d = f' stroke-dasharray="{dash}"' if dash else ''
        self.parts.append(
            f'<line x1="{x1:.2f}" y1="{y1:.2f}" x2="{x2:.2f}" y2="{y2:.2f}"'
            f' stroke="{stroke}" stroke-width="{sw}"{o}{d}/>')

    def group(self, s):
        self.parts.append(s)

    def save(self, name):
        defs = ('<defs>' + ''.join(self.defs) + '</defs>') if self.defs else ''
        doc = (
            f'<svg xmlns="http://www.w3.org/2000/svg" width="{self.w}"'
            f' height="{self.h}" viewBox="0 0 {self.w} {self.h}">\n'
            f'<title>{self.title}</title>\n{defs}\n' + '\n'.join(self.parts) + '\n</svg>\n')
        path = os.path.join(OUT, name)
        with open(path, 'w') as f:
            f.write(doc)
        print(f'wrote {name}  ({self.w}x{self.h})')


def hairline(svg, x1, y1, x2, y2, strong=False):
    svg.line(x1, y1, x2, y2, PAPER, 1,
             opacity=HAIRLINE_STRONG_A if strong else HAIRLINE_A)


# --------------------------------------------------------------------------
# Procedural sleeves — deterministic per title, six idioms, no external assets
# --------------------------------------------------------------------------
ALBUMS = [
    ('Music Has the Right to Children', 'Boards of Canada'),
    ('Selected Ambient Works 85–92', 'Aphex Twin'),
    ('Spirit of Eden', 'Talk Talk'),
    ('In Rainbows', 'Radiohead'),
    ('Untrue', 'Burial'),
    ('Loveless', 'My Bloody Valentine'),
    ('The Disintegration Loops', 'William Basinski'),
    ('Endtroducing.....', 'DJ Shadow'),
    ('Homogenic', 'Björk'),
    ('Blue Lines', 'Massive Attack'),
    ('Ágætis byrjun', 'Sigur Rós'),
    ('For Alto', 'Anthony Braxton'),
    ('Rings of Saturn', 'Nala Sinephro'),
    ('A Love Supreme', 'John Coltrane'),
    ('Talkie Walkie', 'Air'),
    ('Geogaddi', 'Boards of Canada'),
    ('Kid A', 'Radiohead'),
    ('Vespertine', 'Björk'),
    ('Third', 'Portishead'),
    ('Mezzanine', 'Massive Attack'),
    ('The K&D Sessions', 'Kruder & Dorfmeister'),
    ('Music for Airports', 'Brian Eno'),
    ('Substrata', 'Biosphere'),
    ('76:14', 'Global Communication'),
]

SLEEVE_PALETTES = [
    ('#12303F', '#2F7E9E', '#EDE0C4'),      # cold blue
    ('#3A0F18', '#B02B3C', '#F2D9C6'),      # oxblood
    ('#0D2019', '#2E8A5B', '#DCF0D6'),      # deep green
    ('#1B1030', '#6B44C8', '#EFE6FF'),      # violet
    ('#3A2A05', '#D0961A', '#FFF3D2'),      # ochre
    ('#08090B', '#191B20', '#3A3E46'),      # near-black — merges with the wall
    ('#5A2408', '#E0692A', '#FFE3C4'),      # burnt orange
    ('#E4E0D6', '#B9B2A4', '#2A2723'),      # paper-pale — the opposite case
    ('#1A0B12', '#8E2E6B', '#F6DCEF'),      # magenta
    ('#0A1A1E', '#3FB0AD', '#E6FBF7'),      # teal
]

# The first row of every shelf mock is curated rather than hashed, so the
# picture always contains the two cases the wall has to survive: a near-black
# cover (index 5's palette) and a paper-pale one (index 7's).
PAL_OVERRIDE = {0: 0, 1: 4, 2: 9, 3: 5, 4: 2, 5: 8, 6: 6, 7: 7, 15: 1}
IDIOM_OVERRIDE = {1: 2, 3: 1, 5: 0, 7: 1, 15: 2}


def h32(s):
    v = 2166136261
    for ch in s:
        v = ((v ^ ord(ch)) * 16777619) & 0xFFFFFFFF
    return v


def sleeve(svg, x, y, size, title, artist, idx):
    """A deterministic procedural cover: the wall has to be judged, not imagined."""
    seed = h32(title + artist)
    pal = SLEEVE_PALETTES[PAL_OVERRIDE.get(idx, seed % len(SLEEVE_PALETTES))]
    idiom = IDIOM_OVERRIDE.get(idx, (seed >> 8) % 6)
    gid = f'clip{next(_ids)}'          # unique: `idx` selects a palette, not an id
    svg.defs.append(
        f'<clipPath id="{gid}"><rect x="{x:.2f}" y="{y:.2f}"'
        f' width="{size:.2f}" height="{size:.2f}"/></clipPath>')
    svg.raw(f'<g clip-path="url(#{gid})">')
    svg.rect(x, y, size, size, pal[0])
    r = lambda n: ((seed >> (n * 3)) % 1000) / 1000.0

    uid = next(_ids)
    if idiom == 0:                                    # concentric rings
        cx, cy = x + size * (0.3 + 0.4 * r(1)), y + size * (0.3 + 0.4 * r(2))
        for k in range(9, 0, -1):
            svg.circle(cx, cy, size * 0.09 * k, pal[1 if k % 2 else 2],
                       opacity=0.10 + 0.06 * (9 - k))
    elif idiom == 1:                                  # horizontal bands
        n = 5 + int(6 * r(3))
        for k in range(n):
            hh = size / n
            svg.rect(x, y + k * hh, size, hh,
                     pal[1] if (k + seed) % 3 else pal[2],
                     opacity=0.15 + 0.55 * ((k * 37 + seed) % 100) / 100.0)
    elif idiom == 2:                                  # one large form, offset
        svg.rect(x, y, size, size, pal[1], opacity=0.35)
        cx, cy = x + size * (0.25 + 0.5 * r(4)), y + size * (0.25 + 0.5 * r(5))
        svg.circle(cx, cy, size * (0.18 + 0.16 * r(6)), pal[2], opacity=0.88)
    elif idiom == 3:                                  # dot field
        n = 6 + int(5 * r(7))
        step = size / n
        for a in range(n):
            for b in range(n):
                if (a * 7 + b * 13 + seed) % 5 == 0:
                    continue
                svg.circle(x + step * (a + 0.5), y + step * (b + 0.5),
                           step * 0.22, pal[2],
                           opacity=0.12 + 0.5 * ((a * b + seed) % 10) / 10.0)
    elif idiom == 4:                                  # diagonal split
        svg.raw(f'<path d="M{x:.2f},{y + size:.2f} L{x + size:.2f},{y:.2f}'
                f' L{x + size:.2f},{y + size:.2f} Z" fill="{pal[1]}"/>')
        svg.raw(f'<path d="M{x:.2f},{y:.2f} L{x + size * 0.62:.2f},{y:.2f}'
                f' L{x:.2f},{y + size * 0.62:.2f} Z" fill="{pal[2]}"'
                f' fill-opacity="0.55"/>')
    else:                                             # grain field + block
        gd = f'grad{uid}'
        svg.defs.append(
            f'<linearGradient id="{gd}" x1="0" y1="0" x2="0.6" y2="1">'
            f'<stop offset="0" stop-color="{pal[1]}"/>'
            f'<stop offset="1" stop-color="{pal[0]}"/></linearGradient>')
        svg.rect(x, y, size, size, f'url(#{gd})')
        svg.rect(x + size * 0.12, y + size * 0.60, size * 0.5, size * 0.02, pal[2],
                 opacity=0.8)
        svg.rect(x + size * 0.12, y + size * 0.66, size * 0.33, size * 0.02, pal[2],
                 opacity=0.5)
    svg.raw('</g>')


def wall_label(svg, x, y, width, title, artist, playing=False, hovered=False,
               selected=False):
    """The signature, at shelf scale. Two one-line lanes, never one two-line box."""
    tx = x
    if playing:
        svg.circle(x + DOT / 2, y + LABEL_LINE_H * 0.62 - DOT / 2 + 1, DOT / 2, LAMP)
        tx = x + DOT + GAP_XS
    baseline1 = y + LABEL_LINE_H * 0.72
    baseline2 = y + LABEL_LINE_H + LABEL_LINE_H * 0.70
    # `Wrapping::None` clips at the lane's width; the clip is predictable
    # because the lane is fixed.
    cid = f'lab{next(_ids)}'
    svg.defs.append(f'<clipPath id="{cid}"><rect x="{x:.2f}" y="{y:.2f}"'
                    f' width="{width:.2f}" height="{LABEL_H:.2f}"/></clipPath>')
    svg.raw(f'<g clip-path="url(#{cid})">')
    svg.text(tx, baseline1, title, SIZE_BODY, PAPER, 500)
    svg.text(x, baseline2, artist, SIZE_META,
             PAPER_DIM if hovered else PAPER_FAINT, 400)
    svg.raw('</g>')
    if selected:
        svg.rect(x, y + LABEL_H + GAP_XS, width, 2, PAPER_FAINT)
    elif hovered:
        svg.line(x, y + LABEL_H + GAP_XS + 0.5, x + width, y + LABEL_H + GAP_XS + 0.5,
                 PAPER, 1, opacity=HAIRLINE_STRONG_A)


INDEX_KEYS = '#ABCDEFGHIJKLMNOPQRSTUVWXYZ'
# Which initials the fixture library actually has an album under. The rest are
# drawn, but at PAPER_MUTED — an index that hides its gaps is lying about the
# collection.
INDEX_PRESENT = set('#ABEFGHKLMRSTUV')


def spine_index(svg, x, y, h, current=None):
    """The spine index (03-interface-prior-art.md R8): jump-to-letter.

    Type, not chrome — so the shelf still contains only artwork and type. Never
    the accent: an index is navigation, not playback truth. When 27 keys do not
    fit the height, the run subsamples and elided keys render as a 2 px dot,
    the pattern every phone contact list uses.
    """
    step = SIZE_CAPTION * 1.45
    fits = int(h // step)
    keys = list(INDEX_KEYS)
    if fits < len(keys):
        stride = len(keys) / fits
        keys = [(INDEX_KEYS[int(i * stride)], i) for i in range(fits)]
        keys = [(k, True) for k, _ in keys]
    else:
        keys = [(k, True) for k in keys]
    top = y + (h - len(keys) * step) / 2
    for i, (k, _) in enumerate(keys):
        ky = top + i * step + SIZE_CAPTION * 0.8
        if k == current:
            ink, weight = PAPER, 500
        elif k in INDEX_PRESENT:
            ink, weight = PAPER_FAINT, 400
        else:
            ink, weight = PAPER_MUTED, 400
        svg.text(x + INDEX_W / 2, ky, k, SIZE_CAPTION, ink, weight,
                 anchor='middle')


def halo(svg, x, y, size, idx):
    """The one shadow primitive in the product, and it is light, not elevation.

    `Shadow { color: LAMP_GLOW, offset: 0, blur_radius: HALO_BLUR }`. iced's
    blur radius is about twice a Gaussian sigma, so 24 renders as sigma 12.
    """
    fid = f'halo{next(_ids)}'
    svg.defs.append(
        f'<filter id="{fid}" x="-60%" y="-60%" width="220%" height="220%">'
        f'<feGaussianBlur stdDeviation="{HALO_BLUR / 2:.1f}"/></filter>')
    svg.raw(f'<rect x="{x:.2f}" y="{y:.2f}" width="{size:.2f}" height="{size:.2f}"'
            f' fill="{LAMP}" fill-opacity="{LAMP_GLOW_A}" filter="url(#{fid})"/>')


# --------------------------------------------------------------------------
# Chrome
# --------------------------------------------------------------------------
def top_bar(svg, w, counts='24 albums · 287 tracks', query=None):
    svg.rect(0, 0, w, TOP_BAR_H, WALL)
    hairline(svg, 0, TOP_BAR_H + 0.5, w, TOP_BAR_H + 0.5)
    fw, fh = 360, 28
    fx, fy = GAP_XL, (TOP_BAR_H - fh) / 2
    svg.rect(fx, fy, fw, fh, RECESS, RADIUS_CTRL)
    svg.line(fx + 0.5, fy + 0.5, fx + 0.5, fy + fh - 0.5, PAPER, 1, opacity=HAIRLINE_A)
    svg.raw(f'<rect x="{fx:.2f}" y="{fy:.2f}" width="{fw}" height="{fh}"'
            f' rx="{RADIUS_CTRL}" fill="none" stroke="{PAPER}"'
            f' stroke-opacity="{HAIRLINE_A}" stroke-width="1"/>')
    svg.text(fx + GAP_MD, fy + fh / 2 + 4,
             query if query else 'Search your collection',
             SIZE_BODY, PAPER if query else PAPER_FAINT)
    svg.text(w - GAP_XL, TOP_BAR_H / 2 + 4, 'Settings', SIZE_META, PAPER_DIM,
             anchor='end')
    svg.text(w - GAP_XL - 56 - GAP_XL, TOP_BAR_H / 2 + 4, counts, SIZE_META,
             PAPER_FAINT, anchor='end')


def transport_glyph(svg, cx, cy, kind, ink=PAPER, opacity=1.0):
    s = ICON_PX / 2
    if kind == 'play':
        svg.raw(f'<path d="M{cx - s * 0.55:.2f},{cy - s * 0.8:.2f}'
                f' L{cx + s * 0.75:.2f},{cy:.2f}'
                f' L{cx - s * 0.55:.2f},{cy + s * 0.8:.2f} Z"'
                f' fill="{ink}" fill-opacity="{opacity}"/>')
    elif kind == 'pause':
        svg.rect(cx - s * 0.72, cy - s * 0.8, s * 0.5, s * 1.6, ink, opacity=opacity)
        svg.rect(cx + s * 0.22, cy - s * 0.8, s * 0.5, s * 1.6, ink, opacity=opacity)
    elif kind in ('next', 'prev'):
        d = 1 if kind == 'next' else -1
        for k in (0, 1):
            ox = cx + d * (k * s * 0.62 - s * 0.62)
            svg.raw(f'<path d="M{ox - d * s * 0.3:.2f},{cy - s * 0.7:.2f}'
                    f' L{ox + d * s * 0.42:.2f},{cy:.2f}'
                    f' L{ox - d * s * 0.3:.2f},{cy + s * 0.7:.2f} Z"'
                    f' fill="{ink}" fill-opacity="{opacity}"/>')
        svg.rect(cx + d * s * 0.72, cy - s * 0.7, s * 0.22, s * 1.4, ink,
                 opacity=opacity)


def transport_button(svg, x, y, kind, hovered=False):
    svg.rect(x, y, TRANSPORT_HIT, TRANSPORT_HIT,
             PLINTH_LIT if hovered else PLINTH, RADIUS_CTRL)
    svg.raw(f'<rect x="{x:.2f}" y="{y:.2f}" width="{TRANSPORT_HIT}"'
            f' height="{TRANSPORT_HIT}" rx="{RADIUS_CTRL}" fill="none"'
            f' stroke="{PAPER}" stroke-opacity='
            f'"{HAIRLINE_STRONG_A if hovered else HAIRLINE_A}" stroke-width="1"/>')
    transport_glyph(svg, x + TRANSPORT_HIT / 2, y + TRANSPORT_HIT / 2, kind)


def now_playing_bar(svg, w, y, title='Roygbiv', artist='Boards of Canada',
                    position='3 / 12', elapsed='2:31', total='7:04',
                    frac=0.36, signal='bit-perfect', preview=None):
    svg.rect(0, y, w, BAR_H, RECESS)
    hairline(svg, 0, y + 0.5, w, y + 0.5)
    inner_y = y + GAP_MD

    # ---- centre: the fixed SEEK_ROW_W column, optically centred
    cx0 = (w - SEEK_ROW_W) / 2
    row_y = inner_y
    bx = cx0 + (SEEK_ROW_W - (3 * TRANSPORT_HIT + 2 * GAP_SM)) / 2
    transport_button(svg, bx, row_y, 'prev')
    transport_button(svg, bx + TRANSPORT_HIT + GAP_SM, row_y, 'pause')
    transport_button(svg, bx + 2 * (TRANSPORT_HIT + GAP_SM), row_y, 'next')

    seek_top = row_y + TRANSPORT_HIT + GAP_SM
    groove_y = seek_top + PREVIEW_H + RAIL_HIT / 2
    gx = cx0 + STAMP_W + GAP_SM
    svg.rect(gx, groove_y - RAIL / 2, SEEK_W, RAIL, PLINTH, RAIL / 2)
    svg.rect(gx, groove_y - RAIL / 2, SEEK_W * frac, RAIL, LAMP, RAIL / 2)
    svg.circle(gx + SEEK_W * frac, groove_y, KNOB, LAMP)
    # timestamps sit in fixed STAMP_W slots, right/left aligned to the groove
    svg.text(gx - GAP_SM, groove_y + 4, elapsed, SIZE_META, PAPER_DIM, anchor='end')
    svg.text(gx + SEEK_W + GAP_SM, groove_y + 4, total, SIZE_META, PAPER_FAINT)
    # the PREVIEW_H lane is reserved whether or not anything hovers
    if preview:
        px_, label = preview
        tipx = gx + SEEK_W * px_ - PREVIEW_W / 2
        svg.rect(tipx, seek_top, PREVIEW_W, PREVIEW_H, PLINTH_LIT, RADIUS_CHIP)
        svg.raw(f'<rect x="{tipx:.2f}" y="{seek_top:.2f}" width="{PREVIEW_W}"'
                f' height="{PREVIEW_H}" rx="{RADIUS_CHIP}" fill="none"'
                f' stroke="{PAPER}" stroke-opacity="{HAIRLINE_STRONG_A}"/>')
        svg.text(tipx + PREVIEW_W / 2, seek_top + 11, label, SIZE_CAPTION,
                 PAPER_DIM, anchor='middle')

    # ---- left: the wall label at bar scale + the fixed position slot,
    #      the whole block optically centred in the bar's height
    lx = GAP_XL
    block_h = LABEL_LINE_H * 2 + SIZE_META * 1.35 + GAP_XS
    top = y + (BAR_H - block_h) / 2
    svg.text(lx, top + 13, title, SIZE_BODY, PAPER, 500)
    svg.text(lx, top + 13 + LABEL_LINE_H, artist, SIZE_META, PAPER_DIM)
    svg.text(lx, top + 13 + 2 * LABEL_LINE_H + GAP_XS, position, SIZE_META,
             PAPER_FAINT)

    # ---- right: signal note then the volume block, right-aligned
    rx = w - GAP_XL
    svg.text(rx, inner_y + 14, signal, SIZE_META, PAPER_FAINT, anchor='end')
    vx = rx - VOLUME_BLOCK_W
    vy = inner_y + 34
    svg.rect(vx, vy, TRANSPORT_HIT, TRANSPORT_HIT, PLINTH, RADIUS_CTRL)
    svg.raw(f'<rect x="{vx:.2f}" y="{vy:.2f}" width="{TRANSPORT_HIT}"'
            f' height="{TRANSPORT_HIT}" rx="{RADIUS_CTRL}" fill="none"'
            f' stroke="{PAPER}" stroke-opacity="{HAIRLINE_A}"/>')
    svg.raw(f'<path d="M{vx + 10:.2f},{vy + 13:.2f} L{vx + 14:.2f},{vy + 13:.2f}'
            f' L{vx + 19:.2f},{vy + 8:.2f} L{vx + 19:.2f},{vy + 24:.2f}'
            f' L{vx + 14:.2f},{vy + 19:.2f} L{vx + 10:.2f},{vy + 19:.2f} Z"'
            f' fill="{PAPER}"/>')
    fx = vx + TRANSPORT_HIT + GAP_SM
    fy = vy + TRANSPORT_HIT / 2
    svg.rect(fx, fy - RAIL / 2, VOLUME_W, RAIL, PLINTH, RAIL / 2)
    svg.rect(fx, fy - RAIL / 2, VOLUME_W * 0.78, RAIL, PAPER_FAINT, RAIL / 2)
    svg.circle(fx + VOLUME_W * 0.78, fy, KNOB, PAPER_FAINT)
    svg.rect(fx + VOLUME_W - 1, fy - RAIL / 2 - 2 - 5, 2, 5, PAPER, opacity=HAIRLINE_A * 3)


# --------------------------------------------------------------------------
# 01 / 02 — the shelf at two window widths
# --------------------------------------------------------------------------
def shelf(window_w, window_h, name, with_inspector=False, playing_index=1,
          hover_index=6, selected_index=None, labels=None, albums=None,
          density='Balanced'):
    svg = Svg(window_w, window_h, WALL, f'baz shelf @ {window_w}px')
    insp = inspector_w(window_w) if with_inspector else 0
    content_w = window_w - insp
    shelf_w = grid_width(window_w, with_inspector)
    n, art, gut, margin, row_h = hang(shelf_w, density)

    top_bar(svg, window_w)
    shelf_top = TOP_BAR_H + 1
    shelf_bottom = window_h - BAR_H

    # clip the shelf region so partial rows read as scrollable, like the real one
    cid = f'shelfclip{next(_ids)}'
    svg.defs.append(f'<clipPath id="{cid}"><rect x="0" y="{shelf_top}"'
                    f' width="{shelf_w}" height="{shelf_bottom - shelf_top}"/></clipPath>')
    svg.raw(f'<g clip-path="url(#{cid})">')

    order = albums or ALBUMS
    y = shelf_top + HANG
    i = 0
    row = 0
    while y < shelf_bottom + row_h:
        for c in range(n):
            if i >= len(order):
                break
            x = margin + c * (art + gut)
            title, artist = order[i]
            playing = (i == playing_index)
            hovered = (i == hover_index)
            selected = (selected_index is not None and i == selected_index)
            if playing:
                halo(svg, x, y, art, i)
            sleeve(svg, x, y, art, title, artist, i)
            wall_label(svg, x, y + art + GAP_LG, art, title, artist,
                       playing=playing, hovered=hovered, selected=selected)
            i += 1
        y += row_h
        row += 1
    svg.raw('</g>')

    # the two lanes the shelf keeps clear on its right: the scrollbar's, and the
    # spine index — the run of letters down the edge of a card-catalogue drawer
    svg.rect(content_w - INDEX_W - SCROLLBAR_W + 3, shelf_top + 40,
             SCROLLBAR_W - 4, 180, PAPER, r=3, opacity=HAIRLINE_A)
    spine_index(svg, content_w - INDEX_W, shelf_top, shelf_bottom - shelf_top,
                current='B')

    if with_inspector:
        inspector(svg, window_w - insp, TOP_BAR_H + 1, insp,
                  shelf_bottom - TOP_BAR_H - 1)

    now_playing_bar(svg, window_w, shelf_bottom,
                    preview=(0.62, '4:23') if not with_inspector else None)

    # the picture is a spec: say what the hang computed, and name the states
    ann_y = shelf_top + HANG - 13
    svg.text(margin, ann_y,
             f'{density} · window {window_w:.0f} → grid {shelf_w:.0f} px  →  '
             f'{n} × {art:.0f} px · gutter {gut:.0f} · margin {margin:.0f}'
             f' · row pitch {row_h:.1f}  ·  dead gutter 0',
             SIZE_CAPTION, PAPER_MUTED)
    if labels:
        for c, word in labels.items():
            if c < n:
                lx = margin + c * (art + gut)
                svg.text(lx, shelf_top + HANG + art + GAP_LG + LABEL_H + 24,
                         word, SIZE_CAPTION, PAPER_MUTED)
    svg.save(name)
    return n, art, gut, margin, row_h


# --------------------------------------------------------------------------
# 03 — the album inspector
# --------------------------------------------------------------------------
TRACKS = [
    ('Ready Lets Go', '0:57', False),
    ('Music Is Math', '5:21', False),
    ('Beware the Friendly Stranger', '0:38', True),
    ('Gyroscope', '3:35', False),
    ('Dandelion', '1:00', False),
    ('Sunshine Recorder', '6:14', False),
    ('In the Annexe', '1:22', False),
    ('Julie and Candy', '6:12', False),
    ('The Smallest Weird Number', '1:16', False),
    ('1969', '4:21', False),
    ('Energy Warning', '0:35', False),
    ('The Beach at Redpoint', '4:20', False),
]


FIELD_LABEL_W = 96.0

# What a metadata-rich release actually looks like (R6): the tradition baz
# succeeds shows ~20 fields for free, and four lines is a regression for the
# cataloguer personas. Every row is present only when the scan read one.
DETAILS = [
    ('Album artist', 'Boards of Canada'),
    ('Released', '18 February 2002'),
    ('Label', 'Warp Records'),
    ('Catalogue', 'WARPCD101'),
    ('Genre', 'Electronic · IDM'),
    ('Discs', '1 of 1'),
    ('Format', 'FLAC · 16-bit · 44.1 kHz · stereo'),
    ('Bitrate', '921 kbps average'),
    ('Size', '236.4 MB'),
    ('ReplayGain', 'album −7.24 dB · peak 0.988'),
    ('MusicBrainz', 'e8e9c0f4…'),
    ('Added', '14 March 2024'),
    ('Path', '~/Music/Boards of Canada/Geogaddi'),
]


def inspector(svg, x, y, w, h):
    svg.rect(x, y, w, h, PLINTH)
    hairline(svg, x + 0.5, y, x + 0.5, y + h)
    px = x + GAP_XL
    inner = w - 2 * GAP_XL
    art = min(inner, ART_MAX)
    ay = y + GAP_XL
    # No halo: this album is *selected*, not playing. Selection and playback are
    # different facts and the inspector must be able to show one without the
    # other — which is the whole reason the shelf marks them differently.
    sleeve(svg, px, ay, art, 'Geogaddi', 'Boards of Canada', 15)

    ty = ay + art + GAP_LG + SIZE_TITLE
    svg.text(px, ty, 'Geogaddi', SIZE_TITLE, PAPER, 600)
    svg.text(px, ty + 22, 'Boards of Canada', SIZE_EMPHASIS, PAPER_DIM)
    svg.text(px, ty + 42, '2002 · 12 tracks · 35:51', SIZE_META, PAPER_FAINT)
    svg.text(px, ty + 58, 'FLAC · 16-bit · 44.1 kHz', SIZE_META, PAPER_FAINT)

    # Play album — the control that *creates* playback truth, so it carries the
    # accent; but as a line and a mark, never a fill. Amber is never an opaque
    # rectangle in baz.
    by = ty + 74
    bh = TRANSPORT_HIT
    svg.rect(px, by, inner, bh, LAMP, RADIUS_CTRL, opacity=0.10)
    svg.raw(f'<rect x="{px:.2f}" y="{by:.2f}" width="{inner:.2f}" height="{bh}"'
            f' rx="{RADIUS_CTRL}" fill="none" stroke="{LAMP}" stroke-width="1"/>')
    transport_glyph(svg, px + 20, by + bh / 2, 'play', LAMP)
    svg.text(px + 36, by + bh / 2 + 5, 'Play album', SIZE_BODY, PAPER, 600)

    ly = by + bh + GAP_LG
    listed = 0
    for i, (name, dur, playing) in enumerate(TRACKS):
        rh = 22
        ry = ly + i * rh
        if ry + rh > y + h - GAP_XL:
            break
        listed = i + 1
        if playing:
            svg.rect(px - GAP_XS, ry, inner + 2 * GAP_XS, rh, PLINTH_LIT,
                     RADIUS_SEGMENT)
            svg.raw(f'<rect x="{px - GAP_XS:.2f}" y="{ry:.2f}"'
                    f' width="{inner + 2 * GAP_XS:.2f}" height="{rh}"'
                    f' rx="{RADIUS_SEGMENT}" fill="none" stroke="{PAPER}"'
                    f' stroke-opacity="{HAIRLINE_STRONG_A}"/>')
            svg.circle(px + TRACK_NO_W - DOT / 2 - 2, ry + rh / 2, DOT / 2, LAMP)
        else:
            svg.text(px + TRACK_NO_W, ry + rh / 2 + 4, str(i + 1), SIZE_META,
                     PAPER_FAINT, anchor='end')
        ink = PAPER if not playing else PAPER
        weight = 500 if playing else 400
        if i < 2:
            ink = PAPER_FAINT          # played rows fall back
        svg.text(px + TRACK_NO_W + GAP_SM, ry + rh / 2 + 4, name, SIZE_BODY,
                 ink, weight)
        svg.text(px + inner - SCROLLBAR_W, ry + rh / 2 + 4, dur, SIZE_META,
                 PAPER_FAINT, anchor='end')
    # the reserved scrollbar lane the list keeps clear, whether or not it scrolls
    svg.rect(px + inner - SCROLLBAR_W + 2, ly + 2, SCROLLBAR_W - 4,
             max(24.0, (listed / len(TRACKS)) * (listed * 22 - 4)), PAPER, r=3,
             opacity=HAIRLINE_A)
    # the condition report, in full, below the track list (R6). Drawn faded at
    # the panel's foot because in the real surface it is below the fold.
    dy = ly + listed * 22 + GAP_LG
    if dy < y + h - 40:
        hairline(svg, px, dy + 0.5, px + inner, dy + 0.5)
        svg.text(px, dy + 18, 'Details', SIZE_META, PAPER_MUTED, 500)
        for i, (k, v) in enumerate(DETAILS):
            fy = dy + 38 + i * 17
            if fy > y + h - GAP_XL:
                break
            svg.text(px + FIELD_LABEL_W, fy, k, SIZE_META, PAPER_MUTED,
                     anchor='end')
            svg.text(px + FIELD_LABEL_W + GAP_MD, fy, v, SIZE_META, PAPER_DIM)


def album_inspector_sheet():
    # Geogaddi is what the inspector is showing, so it must be the album the
    # shelf marks as selected: selection and the inspector are one fact.
    order = [ALBUMS[0], ALBUMS[1], ALBUMS[15]] + [
        a for i, a in enumerate(ALBUMS) if i not in (0, 1, 15)]
    return shelf(1280, 820, '03-album-inspector.svg', with_inspector=True,
                 playing_index=0, hover_index=1, selected_index=2, albums=order,
                 labels={0: 'playing — halo + dot',
                         1: 'hovered — artist lifts, 1 px hairline rule',
                         2: 'selected — 2 px PAPER_FAINT rule; the inspector is this'})


# --------------------------------------------------------------------------
# 04 — the now-playing bar at 2x, with its reserved slots called out
# --------------------------------------------------------------------------
def bar_sheet():
    W, S = 1280, 2
    rows = [
        ('POSITION_W', 56, '199 / 240', 53.46, 'new — the queue readout the IA adds'),
        ('STAMP_W', 52, '10:00:00', 50.21, 'unchanged — and the mono face could not hold this at 52'),
        ('SEEK_W', 260, '—', 0, 'unchanged — a map of the track, not a gauge'),
        ('SEEK_ROW_W', 380, '—', 0, 'SEEK_W + 2 × (STAMP_W + GAP_SM)'),
        ('PREVIEW_H', 15, '—', 0, 'the hover lane, reserved whether or not anything hovers'),
        ('SIGNAL_W', 96, '192 → 176.4 kHz', 92.38, 'was 120'),
        ('LEVEL_W', 48, '-18.1 dB', 43.34, 'was 62'),
        ('PREVIEW_W', 48, '0:00:00 + 2 × GAP_XS', 47.42, 'was 58'),
        ('VOLUME_BLOCK_W', 136, '—', 0, 'TRANSPORT_HIT + GAP_SM + VOLUME_W'),
        ('BAR_H', 102, '—', 0, '1 + 12 + 32 + 8 + 15 + 22 + 12 — identical in every state'),
    ]
    H = BAR_H * S + 96 + len(rows) * 28
    svg = Svg(W * S, H, WALL, 'baz now-playing bar @2x, with its reserved slots')
    svg.rect(0, 0, W * S, H, WALL)
    svg.raw(f'<g transform="scale({S})">')
    now_playing_bar(svg, W, 0, preview=(0.62, '4:23'))
    svg.raw('</g>')

    x = 40
    y = BAR_H * S + 44
    svg.text(x, y, 'Nothing in the bar is sized to its content. Every slot is a '
                   'token wide enough for its worst case, measured in IBM Plex Sans.',
             SIZE_EMPHASIS + 3, PAPER_DIM)
    y += 34
    cols = (x, x + 300, x + 380, x + 800, x + 900)
    svg.text(cols[0], y, 'TOKEN', SIZE_BODY, PAPER_MUTED, 500)
    svg.text(cols[1], y, 'RESERVED', SIZE_BODY, PAPER_MUTED, 500, anchor='end')
    svg.text(cols[2], y, 'WORST CASE', SIZE_BODY, PAPER_MUTED, 500)
    svg.text(cols[3], y, 'MEASURED', SIZE_BODY, PAPER_MUTED, 500, anchor='end')
    svg.text(cols[4], y, 'NOTE', SIZE_BODY, PAPER_MUTED, 500)
    hairline(svg, x, y + 10.5, W * S - x, y + 10.5)
    for i, (tok, res, worst, meas, note) in enumerate(rows):
        ry = y + 32 + i * 26
        svg.text(cols[0], ry, tok, SIZE_EMPHASIS, PAPER, 500)
        svg.text(cols[1], ry, f'{res} px', SIZE_EMPHASIS, PAPER_DIM, anchor='end')
        svg.text(cols[2], ry, worst, SIZE_EMPHASIS, PAPER_DIM)
        svg.text(cols[3], ry, f'{meas:.2f} px' if meas else '—',
                 SIZE_EMPHASIS, PAPER_FAINT, anchor='end')
        svg.text(cols[4], ry, note, SIZE_EMPHASIS, PAPER_FAINT)
    svg.save('04-now-playing-bar.svg')
    return H


# --------------------------------------------------------------------------
# 05 — the figures specimen: the proof that removing the mono costs nothing
# --------------------------------------------------------------------------
STAMPS = ['0:00', '1:07', '2:31', '3:36', '9:41', '10:24', '47:21', '1:03:45']
DURS = ['0:57', '5:21', '0:38', '3:35', '1:00', '6:14', '1:22', '6:12']


def figures_sheet():
    W, H = 1280, 1000
    svg = Svg(W, H, WALL, 'baz figures specimen — no monospace')
    x = 48
    svg.text(x, 60, 'Figures without a monospace', SIZE_HERO, PAPER, 600)
    svg.text(x, 88, 'IBM Plex Sans ships tabular figures by default. Measured with '
                    'HarfBuzz, default features on — what cosmic-text will draw.',
             SIZE_EMPHASIS, PAPER_DIM)

    # --- 1. the ten digits, each in its 0.600 em box
    y = 132
    svg.text(x, y, '1 · EVERY DIGIT ADVANCES 600/1000 EM, IN ALL THREE WEIGHTS',
             SIZE_META, PAPER_MUTED, 500)
    hairline(svg, x, y + 8.5, W - 48, y + 8.5)
    y += 34
    for wi, (label, weight) in enumerate([('Regular', 400), ('Medium', 500),
                                          ('SemiBold', 600)]):
        size = 34
        adv = 0.600 * size
        svg.text(x, y + wi * 58 + 24, label, SIZE_META, PAPER_FAINT)
        bx = x + 90
        for d in range(10):
            svg.rect(bx + d * adv, y + wi * 58 - 6, adv, 44, PLINTH)
            svg.line(bx + d * adv + 0.5, y + wi * 58 - 6,
                     bx + d * adv + 0.5, y + wi * 58 + 38, PAPER, 1,
                     opacity=HAIRLINE_STRONG_A)
            svg.text(bx + d * adv + adv / 2, y + wi * 58 + 26, str(d), size, PAPER,
                     weight, anchor='middle')
        svg.line(bx + 10 * adv + 0.5, y + wi * 58 - 6, bx + 10 * adv + 0.5,
                 y + wi * 58 + 38, PAPER, 1, opacity=HAIRLINE_STRONG_A)
        svg.text(bx + 10 * adv + GAP_MD, y + wi * 58 + 24,
                 f'{adv:.1f} px at {size} px · identical for 0–9',
                 SIZE_META, PAPER_FAINT)

    # --- 2. stacked timestamps: the columns line up because the advances do
    y = 342
    svg.text(x, y, '2 · THE COLUMN THE MONO EXISTED FOR', SIZE_META, PAPER_MUTED, 500)
    hairline(svg, x, y + 8.5, W - 48, y + 8.5)
    y += 30
    svg.text(x, y, 'a real duration column, right-aligned', SIZE_META, PAPER_FAINT)
    col_r = x + 260
    for i, d in enumerate(DURS):
        ry = y + 24 + i * 19
        svg.text(x + 20, ry, TRACKS[i][0][:28], SIZE_BODY, PAPER_DIM)
        svg.text(col_r, ry, d, SIZE_META, PAPER, anchor='end')
    svg.line(col_r + 1.5, y + 12, col_r + 1.5, y + 24 + len(DURS) * 19 - 10, LAMP,
             1, opacity=0.45)
    svg.text(x + 20, y + 24 + len(DURS) * 19 + 6, 'one edge, every row — the '
             'pinned side is the one the eye follows', SIZE_CAPTION, PAPER_MUTED)

    svg.text(x + 400, y, 'timestamps stacked — every one 43.008 px wide',
             SIZE_META, PAPER_FAINT)
    for i, s in enumerate(STAMPS):
        ry = y + 24 + i * 19
        svg.text(x + 400, ry, s, SIZE_META, PAPER)
    svg.line(x + 399.5, y + 12, x + 399.5, y + 24 + len(STAMPS) * 19 - 10,
             PAPER, 1, opacity=HAIRLINE_STRONG_A)

    pairs = [('0:00:00', '9:59:59'), ('1:23:45', '8:07:02'),
             ('12 / 32 albums', '11 / 11 albums'), ('999', '111'),
             ('-18.1 dB', '-60.0 dB'), ('3:36', '9:41')]
    svg.text(x + 620, y, 'same shape ⇒ same width, measured', SIZE_META, PAPER_FAINT)
    for i, (a, b) in enumerate(pairs):
        ry = y + 24 + i * 19
        svg.text(x + 620, ry, a, SIZE_META, PAPER)
        svg.text(x + 760, ry, b, SIZE_META, PAPER)
        svg.text(x + 900, ry, 'Δ 0.000 px', SIZE_META, PAPER_MUTED)

    # --- 3. the one jiggle, named
    y = 590
    svg.text(x, y, '3 · THE ONE PLACE A PROPORTIONAL FACE CAN STILL MOVE A FIGURE',
             SIZE_META, PAPER_MUTED, 500)
    hairline(svg, x, y + 8.5, W - 48, y + 8.5)
    y += 30
    jig = [('-20.00 dB', 'hyphen-minus advances 0.399 em', 54.48, ALERT),
           ('+20.00 dB', 'plus advances 0.600 em', 56.89, PAPER_DIM),
           ('−20.00 dB', 'U+2212 also advances 0.600 em — the fix', 56.89, PAPER),
           ('0.00 dB', 'unsigned: 7.2 px narrower, one point in the travel, accepted',
            49.69, PAPER_FAINT)]
    slot_l, slot_w = x + 20, 60.0
    for i, (s, note, wpx, ink) in enumerate(jig):
        ry = y + 22 + i * 28
        svg.rect(slot_l, ry - 14, slot_w, 22, PLINTH)
        svg.text(slot_l + slot_w - 3, ry + 2, s, SIZE_META, ink, anchor='end')
        # the slot's pinned right edge, and where this string's left edge lands
        svg.line(slot_l + slot_w - 2.5, ry - 16, slot_l + slot_w - 2.5, ry + 10,
                 LAMP, 1, opacity=0.6)
        svg.line(slot_l + slot_w - 3 - wpx, ry - 16, slot_l + slot_w - 3 - wpx,
                 ry + 10, PAPER, 1, opacity=0.35, dash='2 2')
        svg.text(slot_l + slot_w + 24, ry + 2, f'{wpx:.2f} px — {note}',
                 SIZE_META, PAPER_FAINT)
    svg.text(x + 20, y + 140,
             'Never acceptable for anything that ticks with playback — elapsed, '
             'remaining, preview, level, position. Those are fixed-digit-count '
             'strings and are exact to 0.000 px.', SIZE_META, PAPER_DIM)

    # --- 4. the verdict
    y = 770
    svg.text(x, y, '4 · VERDICT', SIZE_META, PAPER_MUTED, 500)
    hairline(svg, x, y + 8.5, W - 48, y + 8.5)
    y += 32
    for line in [
        'MONO is deleted. Nothing in baz needs it: the alignment it was standing in for '
        'is a property of the Sans.',
        'Every reserved slot is re-derived from the real advances. Four of them shrink; '
        'STAMP_W keeps 52 px and gains a ten-hour track.',
        'Rule: figures are right-aligned, in fixed slots, at fixed digit counts. '
        'Ragged-left reads fine editorially.',
        'Bundle drops from five faces (1 001 520 B) to three (605 592 B). '
        'SERIF goes with it — the room supplies nothing, the work supplies everything.',
    ]:
        svg.text(x, y, '—  ' + line, SIZE_BODY, PAPER_DIM)
        y += 24
    svg.save('05-figures-specimen.svg')


def inspector_sheet_full():
    """The inspector at `INSPECTOR_MAX_W`, unscrolled, so the whole column can
    be read at once — including the Details block, which in the real surface is
    below the fold (03-interface-prior-art.md R6)."""
    w, h = 900, 1180
    svg = Svg(w, h, WALL, 'baz album inspector, full column')
    svg.text(40, 44, 'The album inspector, unscrolled', SIZE_HERO - 8, PAPER, 600)
    svg.text(40, 68, 'At INSPECTOR_MAX_W 420. Four lines above the fold for '
                     'Devon; the whole condition report below it for Marta and Karl.',
             SIZE_META, PAPER_DIM)
    inspector(svg, 40, 88, INSPECTOR_MAX_W, h - 128)
    ax = 40 + INSPECTOR_MAX_W + 48
    notes = [
        ('the sleeve', 'min(column − 2 × GAP_XL, ART_MAX) = 320, left-aligned'),
        ('the wall label', 'title 22 SemiBold over artist 15 PAPER_DIM'),
        ('the catalogue line', 'the selected edition, not the album'),
        ('the condition report', 'only when the scan read one'),
        ('Play album', 'LAMP outlined — never a fill'),
        ('the track list', 'right-aligned durations; the playing row dotted'),
        ('Details', 'every field the scan has, no disclosure, below the fold'),
    ]
    ny = 150
    for k, v in notes:
        svg.text(ax, ny, k, SIZE_BODY, PAPER, 500)
        svg.text(ax, ny + 17, v, SIZE_META, PAPER_FAINT)
        ny += 48
    svg.text(ax, ny + 24, 'fooyin shows ~20 fields for free.', SIZE_META, PAPER_DIM)
    svg.text(ax, ny + 42, 'Four lines was a regression; this is', SIZE_META, PAPER_DIM)
    svg.text(ax, ny + 60, 'the answer, and it costs zero clicks.', SIZE_META, PAPER_DIM)
    svg.save('07-inspector-full.svg')
    return h


def density_sheet():
    """The three density steps at one window width, so R7 is visible."""
    W = 1280
    panel_h = 320 + GAP_LG + LABEL_H + 12          # the tallest step's cell
    svg = Svg(W, 96 + 3 * (panel_h + 56), WALL, 'baz shelf density steps @1280')
    svg.text(40, 52, 'Density is a user control, not a designer’s constant',
             SIZE_HERO - 6, PAPER, 600)
    svg.text(40, 78, 'Three named steps, all four hang numbers a function of the '
                     'step. Settings → Appearance. Shown at a 1280 px window.',
             SIZE_EMPHASIS, PAPER_DIM)
    y = 108
    for step in ('Spacious', 'Balanced', 'Dense'):
        gw = W - SCROLLBAR_W - INDEX_W
        n, art, gut, margin, row_h = hang(gw, step)
        hg = DENSITY[step][0]
        note = ' (default)' if step == 'Balanced' else ''
        svg.text(40, y, f'{step}{note}', SIZE_EMPHASIS, PAPER, 600)
        svg.text(220, y, f'HANG {hg:.0f} · ART_MIN {DENSITY[step][1]:.0f} · '
                         f'ART_TARGET {DENSITY[step][2]:.0f} · '
                         f'ART_MAX {DENSITY[step][3]:.0f}   →   '
                         f'{n} × {art:.0f} px, gutter {gut:.0f}, margin {margin:.0f}',
                 SIZE_META, PAPER_FAINT)
        cid = f'den{next(_ids)}'
        svg.defs.append(f'<clipPath id="{cid}"><rect x="0" y="{y + 12}"'
                        f' width="{W}" height="{panel_h}"/></clipPath>')
        svg.raw(f'<g clip-path="url(#{cid})">')
        for c in range(n):
            ax = margin + c * (art + gut)
            title, artist = ALBUMS[c % len(ALBUMS)]
            sleeve(svg, ax, y + 24, art, title, artist, c)
            wall_label(svg, ax, y + 24 + art + GAP_LG, art, title, artist)
        svg.raw('</g>')
        y += panel_h + 56
    svg.save('06-density.svg')
    return svg.h


if __name__ == '__main__':
    shelf(1280, 820, '01-shelf-1280.svg', playing_index=1, hover_index=2,
          selected_index=None,
          labels={0: 'rest — nothing behind the work at all',
                  1: 'playing — halo + dot; the only light in the room',
                  2: 'hovered — artist lifts, 1 px hairline rule under the label',
                  3: 'a paper-pale sleeve, for the other extreme'})
    shelf(1920, 1080, '02-shelf-1920.svg', playing_index=4, hover_index=1,
          selected_index=None)
    album_inspector_sheet()
    bar_sheet()
    figures_sheet()
    density_sheet()
    inspector_sheet_full()
    print()
    for step in DENSITY:
        print(f'  --- {step}')
        for w in (640, 892, 1090, 1250, 1470, 1890, 2530):
            n, a, g, m, r = hang(w, step)
            print(f'    grid {w:>4} = {n} cols x {a:6.1f} px, gutter {g:4.1f}, '
                  f'margin {m:5.1f}, row {r:6.1f}')
