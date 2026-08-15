#!/usr/bin/env bash
# **A fixture with something to measure.**
#
# `docs/design/composition/tools/mkfixture.sh` builds a wall of *digital
# silence*, deliberately: every sample is a zero, which is one of the two
# independent guarantees that a headless run is inaudible. That fixture is
# right for photographing layout and wrong for photographing the contour —
# bliss extracts identical features from every silent file, so every track
# lands at the same place on a collection-relative axis and the dots draw a
# flat row whatever the line asks for. The picture would be honest and would
# demonstrate nothing.
#
# So this builds 24 short tracks whose **loudness and pulse rate genuinely
# vary**: a click train at a stated tempo under a quiet tone, at a stated
# amplitude. That is enough for tempo, loudness and loudness-variation — the
# three features `Dimension::Energy` is made of — to spread across a real
# range, so a drawn line has something to follow and a reader can see whether
# it did.
#
# It stays inaudible in practice by the same discipline as the rest: the peak
# is -30 dBFS, the capture never presses play, and the scratch `HOME` routes
# ALSA's default PCM to `null` regardless.
#
#   toolbox run -c baz-dev docs/design/impl/contour/mkfixture-varied.sh /tmp/baz-varied
set -euo pipefail

FIX=${1:-/tmp/baz-varied}
RATE=44100
SECONDS_PER=24

rm -rf "$FIX"; mkdir -p "$FIX"
TMP=$(mktemp -d); trap 'rm -rf "$TMP"' EXIT

# One WAV per (tempo, amplitude) pair, written by hand: a click train at
# `bpm` over a sine at 220 Hz, both scaled by `amp`.
python3 - "$FIX" "$TMP" "$RATE" "$SECONDS_PER" <<'PY'
import math, struct, subprocess, sys, wave
from pathlib import Path

fix, tmp, rate, secs = Path(sys.argv[1]), Path(sys.argv[2]), int(sys.argv[3]), int(sys.argv[4])
peak = 0.032  # about -30 dBFS: measurable, inaudible at any sane volume

# Six "records" of four tracks, walking tempo and loudness together so the
# collection has a real spread and a real middle rather than two clumps.
records = [
    ("Nocturne Machine", "Ini Kovac", 1995, 62, 0.18),
    ("Low Tide", "Sonja Aalto", 2001, 78, 0.34),
    ("Middle Distance", "Peel & Marsh", 2008, 96, 0.52),
    ("Signal Hill", "Kesh", 2014, 118, 0.70),
    ("Overdrive", "Corvin", 2019, 140, 0.86),
    ("Terminal Velocity", "Studio Hain", 2023, 168, 1.00),
]

for index, (album, artist, year, bpm, loud) in enumerate(records, start=1):
    folder = fix / f"{index:02d} - {artist} - {album}"
    folder.mkdir(parents=True)
    for track in range(1, 5):
        # Within a record the four tracks vary a little, so no two files are
        # identical and the axis has fine structure as well as a spread.
        track_bpm = bpm + (track - 2) * 4
        track_loud = loud * (0.86 + 0.07 * track)
        frames = rate * secs
        period = max(1, int(rate * 60 / track_bpm))
        samples = bytearray()
        for n in range(frames):
            phase = n / rate
            tone = math.sin(2 * math.pi * 220.0 * phase) * 0.35
            # A short exponential click on every beat: the onset bliss reads.
            since = n % period
            click = math.exp(-since / (rate * 0.012)) if since < rate * 0.05 else 0.0
            value = (tone + click) * track_loud * peak
            packed = struct.pack("<h", max(-32767, min(32767, int(value * 32767))))
            samples += packed + packed
        wav = tmp / f"{index}-{track}.wav"
        with wave.open(str(wav), "wb") as out:
            out.setnchannels(2)
            out.setsampwidth(2)
            out.setframerate(rate)
            out.writeframes(bytes(samples))
        flac = folder / f"{track:02d} Part {track}.flac"
        subprocess.run(
            ["flac", "--totally-silent", "-5", "-f", "-o", str(flac), str(wav)],
            check=True,
        )
        subprocess.run(
            [
                "metaflac",
                "--remove-all-tags",
                f"--set-tag=ALBUM={album}",
                f"--set-tag=ARTIST={artist}",
                f"--set-tag=ALBUMARTIST={artist}",
                f"--set-tag=DATE={year}",
                "--set-tag=GENRE=Electronic",
                f"--set-tag=TRACKNUMBER={track}",
                f"--set-tag=TITLE=Part {track}",
                str(flac),
            ],
            check=True,
        )
        wav.unlink()
print("built", len(records) * 4, "tracks across", len(records), "records")
PY

echo "tracks: $(find "$FIX" -name '*.flac' | wc -l)  records: $(find "$FIX" -mindepth 1 -maxdepth 1 -type d | wc -l)"
du -sh "$FIX"
