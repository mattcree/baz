#!/usr/bin/env python3
"""What the returns lane's collapse does to the wall, in figures.

`Grid::new`'s own arithmetic (`crates/baz/src/shelf.rs`) at Balanced, over
`window − sidebar − INDEX_LANE_W − WALL_SCROLLBAR_W` — which is exactly
`Shelf::grid_width`. Printed by `capture.sh` and copied into README.md, so the
table in the doc is generated rather than transcribed.
"""

HANG, ART_MIN, ART_TARGET, ART_MAX, ART_FLOOR = 40.0, 240.0, 272.0, 320.0, 24.0
INDEX_LANE_W, WALL_SCROLLBAR_W = 108.0, 4.0
SIDEBAR_W, SIDEBAR_RAIL_W, SIDEBAR_FLOOR = 280.0, 96.0, 1000.0


def sidebar(window: float, stored_open: bool) -> float:
    return SIDEBAR_W if (stored_open and window >= SIDEBAR_FLOOR) else SIDEBAR_RAIL_W


def grid(width: float):
    width = max(width, 0.0)
    wanted = (width + HANG) / (ART_TARGET + HANG)
    wanted = int(wanted + 0.5)                       # round_half_up
    ceiling = max(int((width - HANG) // (ART_MIN + HANG)), 1)
    columns = max(min(max(wanted, 1), ceiling), 1)
    art = min(max((width - (columns + 1) * HANG) / columns, ART_FLOOR), ART_MAX)
    gutter = min(max((width - 2 * HANG - columns * art) / (columns - 1), 0.0),
                 2 * HANG) if columns > 1 else 0.0
    block = columns * art + (columns - 1) * gutter
    return columns, art, gutter, max((width - block) / 2.0, 0.0)


print(f"| {'Window':>6} | {'Lane':<20} | {'Grid width':>10} | {'Columns':>7} "
      f"| {'Art':>6} | {'Gutter':>6} | {'Margin':>6} |")
print(f"|{'-'*8}|{'-'*22}|{'-'*12}|{'-'*9}|{'-'*8}|{'-'*8}|{'-'*8}|")
for window in (1280.0, 1440.0, 1920.0):
    rows = [
        ("no lane (before)", 0.0),
        ("expanded, 280", sidebar(window, True)),
        ("collapsed, 96", sidebar(window, False)),
    ]
    for label, lane in rows:
        width = max(window - lane - INDEX_LANE_W - WALL_SCROLLBAR_W, 0.0)
        columns, art, gutter, margin = grid(width)
        print(f"| {window:>6.0f} | {label:<20} | {width:>10.0f} | {columns:>7} "
              f"| {art:>6.0f} | {gutter:>6.0f} | {margin:>6.0f} |")
