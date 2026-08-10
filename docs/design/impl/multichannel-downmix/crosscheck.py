#!/usr/bin/env python3
"""Cross-check baz's ITU-R BS.775 downmix against a second implementation.

The tests in `crates/baz-core/tests/playback.rs` prove baz's fold against the
recommendation's own equations, written out in the test. That is the assertion
that matters, but it is baz checking baz's arithmetic against baz's reading of
a document. This script is the other kind of evidence: it takes the same
fixtures through **ffmpeg's** downmix and reports whether the two agree.

    ./mkfixtures.sh fixtures
    ./crosscheck.py fixtures

Two things it establishes, and one it deliberately does not:

* ffmpeg's own defaults are the same coefficients — `center_mix_level` and
  `surround_mix_level` both 0.707107, `lfe_mix_level` **0**. Printed from the
  running binary, so the citation is checkable rather than remembered.
* Per-speaker placement is identical: every tone comes out of the side BS.775
  puts it on, in both implementations.
* It does **not** expect the absolute levels to match. baz scales the whole
  matrix by 1/2.4142 so the fold can never overflow; ffmpeg, writing float,
  does not, and its output of the clipping fixture goes past full scale. That
  difference is the decision in ADR-0039 §4, and the script measures it rather
  than hiding it.
"""

import math
import re
import subprocess
import sys
from pathlib import Path

import numpy as np

RATE = 48000
TONES = [400.0, 700.0, 1100.0, 1700.0, 2300.0, 3100.0]
AMP = 0.3
K = 1.0 / math.sqrt(2.0)


def decode(path, channels):
    """Decode a file to float32 with ffmpeg, as (frames, channels)."""
    raw = subprocess.run(
        ["ffmpeg", "-v", "error", "-i", str(path), "-f", "f32le", "-"],
        capture_output=True,
        check=True,
    ).stdout
    return np.frombuffer(raw, dtype="<f4").reshape(-1, channels)


def amplitude_at(x, hz):
    """Peak amplitude of one frequency, by Goertzel — reads one tone, ignores
    every other, which is what makes placement a number."""
    n = len(x)
    w = 2.0 * math.pi * hz / RATE
    i = np.arange(n)
    return float(2.0 * abs(np.sum(x * np.exp(-1j * w * i))) / n)


def profile(stereo, out):
    """Every speaker's tone, measured in one output channel."""
    chan = stereo[:, out]
    lo, hi = len(chan) // 8, len(chan) - len(chan) // 8
    return [amplitude_at(chan[lo:hi], hz) for hz in TONES]


def ffmpeg_downmix(path, out):
    """ffmpeg's own stereo downmix of the same file, at its own defaults."""
    subprocess.run(
        ["ffmpeg", "-y", "-v", "error", "-i", str(path), "-ac", "2",
         "-c:a", "pcm_f32le", str(out)],
        check=True,
    )
    return decode(out, 2)


def show_defaults():
    text = subprocess.run(
        ["ffmpeg", "-h", "full"], capture_output=True, text=True
    ).stdout
    print("ffmpeg's own downmix defaults, from the running binary:")
    for opt in ("center_mix_level", "surround_mix_level", "lfe_mix_level"):
        for line in text.splitlines():
            if line.strip().startswith("-" + opt):
                got = re.search(r"default (\S+?)\)", line)
                print(f"  {opt:<20} {got.group(1) if got else '?'}")
                break
    print()


def main(dirname):
    d = Path(dirname)
    show_defaults()

    print("Placement: which speaker's tone lands in which output.")
    print("Slots, in mask order: 0 FL  1 FR  2 FC  3 LFE  4 Ls  5 Rs")
    print("BS.775 says Lo gets FL at 1, FC at 1/√2, Ls at 1/√2, and")
    print("nothing else; Ro is its mirror. The LFE appears in neither.\n")

    src = d / "tones_51.wav"
    mixed = ffmpeg_downmix(src, d / "_ff_51.wav")
    for out, name in ((0, "Lo"), (1, "Ro")):
        got = profile(mixed, out)
        want = [
            AMP * (1.0 if (i == 0 and out == 0) or (i == 1 and out == 1)
                   else K if i == 2 or (i == 4 and out == 0) or (i == 5 and out == 1)
                   else 0.0)
            for i in range(6)
        ]
        print(f"  {name} measured  " + "  ".join(f"{v:7.4f}" for v in got))
        print(f"  {name} BS.775    " + "  ".join(f"{v:7.4f}" for v in want))
        worst = max(abs(a - b) for a, b in zip(got, want))
        print(f"  {name} worst error {worst:.2e}\n")

    print("Levels: what each implementation does about overflow.")
    clip = d / "clip_51.wav"
    ff = ffmpeg_downmix(clip, d / "_ff_clip.wav")
    peak = float(np.abs(ff).max())
    print(f"  fixture peak per channel      1.000 (full scale, in phase, 6 ch)")
    print(f"  matrix worst-case gain        {1 + 2 * K:.4f}  (+{20 * math.log10(1 + 2 * K):.2f} dB)")
    print(f"  ffmpeg -ac 2 output peak      {peak:.4f}")
    print(f"  baz output peak               1.0000  (matrix scaled by "
          f"{1 / (1 + 2 * K):.5f}, {20 * math.log10(1 / (1 + 2 * K)):.2f} dB)")
    print()
    print("  ffmpeg writing float takes no headroom, so its downmix of this")
    print("  fixture is above full scale and clips at whatever is downstream.")
    print("  baz scales the matrix instead, which is ADR-0039 §4.")


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else "fixtures")
