#!/usr/bin/env python3
"""Read the veil off a rendered frame and check it against the design.

The claim under test is the one `theme::veil_alpha` makes: the stops were
written as an **sRGB** composite, iced blends in **linear light**, and the
numbers handed to the renderer are re-solved so that what lands on screen is
the sRGB result the design asked for.

Nothing here trusts the arithmetic in `theme.rs`. It reads two captures — the
same wall at rest and with one tile's caption under the pointer — and for every
pixel of the hovered sleeve recovers the opacity that would have produced it
*in sRGB*:

    a_eff = (rest − drawn) / (rest − recess)

per channel, then takes the median down the sleeve's height so that the option
glyphs and labels (a small minority of any column) cannot move the answer. The
caption is hovered rather than a row, so no row's hover wash is in the frame.

    python3 sample.py            # both widths, markdown table on stdout
"""

from pathlib import Path
import statistics

import numpy as np
from PIL import Image

HERE = Path(__file__).parent
# theme::CLOSING_TIME.recess, #060708, as the 8-bit bytes the surface holds.
RECESS = np.array([6.0, 7.0, 8.0])
# theme::VEIL_SPEC — the design's own stops, in the model they were written in.
SPEC = [(0.00, 0.92), (0.38, 0.86), (0.55, 0.66), (0.68, 0.30), (0.82, 0.05), (1.00, 0.00)]
# The tile the capture hovers, measured off the rest frame (see capture.sh).
# The tile the capture hovers: its top-left and the sleeve's edge in device
# pixels, measured off the rest frame. The edge is `Grid::art − 2 × POOL_RING`
# — 243 − 4 at a 1172 px wall, 255.33 − 4 at an 1812 px one — which is what
# the two numbers below are, and finding them by measurement rather than by
# arithmetic is the point of reading the frame at all.
SLEEVES = {"1280x860": (42, 131, 239), "1920x1080": (42, 131, 251)}


def spec_at(x: float) -> float:
    """The design's opacity at fraction `x` across the sleeve."""
    for (x0, a0), (x1, a1) in zip(SPEC, SPEC[1:]):
        if x0 <= x <= x1:
            return a0 if x1 == x0 else a0 + (x - x0) * (a1 - a0) / (x1 - x0)
    return SPEC[-1][1]


def measure(suffix: str) -> list[tuple[float, float, float]]:
    x0, y0, edge = SLEEVES[suffix]
    rest = np.asarray(
        Image.open(HERE / f"01-wall-at-rest-{suffix}.png").convert("RGB"), dtype=float
    )[y0 : y0 + edge, x0 : x0 + edge]
    drawn = np.asarray(
        Image.open(HERE / f"02-options-bright-sleeve-{suffix}.png").convert("RGB"), dtype=float
    )[y0 : y0 + edge, x0 : x0 + edge]

    rows = []
    for offset, _ in SPEC:
        column = min(int(round(offset * (edge - 1))), edge - 1)
        samples = []
        for y in range(edge):
            for channel in range(3):
                ground = rest[y, column, channel]
                span = ground - RECESS[channel]
                # A pixel already at the room's own value carries no signal.
                if abs(span) < 20.0:
                    continue
                samples.append((ground - drawn[y, column, channel]) / span)
        if not samples:
            continue
        rows.append((offset, spec_at(offset), statistics.median(samples)))
    return rows


def main() -> None:
    for suffix in SLEEVES:
        print(f"\n### {suffix}\n")
        print("| offset across the sleeve | design (sRGB) | measured on the frame | delta |")
        print("|---|---|---|---|")
        worst = 0.0
        for offset, want, got in measure(suffix):
            worst = max(worst, abs(got - want))
            print(f"| {offset:.2f} | {want:.3f} | {got:.3f} | {got - want:+.3f} |")
        print(f"\nWorst deviation: **{worst:.3f}** of an opacity.")


if __name__ == "__main__":
    main()
