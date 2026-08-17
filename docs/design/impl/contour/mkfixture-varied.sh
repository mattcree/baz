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
# So this builds 48 tracks whose **loudness and pulse rate genuinely
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
SECONDS_PER=96

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
# **Twelve records, so a request can actually be filled.** It was six, and the
# diversity rule that refuses the same artist twice running capped every
# composed list at twelve tracks — which at 24 s a track is under five minutes
# against a shortest request of half an hour, so every list came with a
# shortfall note attached. Twelve artists and longer tracks between them clear
# it, and the tempo/loudness walk still runs end to end.
records = [
    ("Nocturne Machine", "Ini Kovac", 1995, 62, 0.18),
    ("Low Tide", "Sonja Aalto", 2001, 78, 0.34),
    ("Paper Mill", "The Ardent", 2004, 84, 0.40),
    ("Middle Distance", "Peel & Marsh", 2008, 96, 0.52),
    ("Chalk Downs", "Edith Rowan Quartet", 2010, 104, 0.58),
    ("Signal Hill", "Kesh", 2014, 118, 0.70),
    ("Amber Room", "Sotto", 2016, 126, 0.74),
    ("Violet Ledger", "Anne-Marie Puig", 2018, 132, 0.80),
    ("Overdrive", "Corvin", 2019, 140, 0.86),
    ("Meadowgrass", "Sonja Aalto", 2021, 150, 0.92),
    ("Cyan Handbook", "Nils Odden", 2022, 158, 0.96),
    ("Terminal Velocity", "Studio Hain", 2023, 168, 1.00),
]

# **Titles, because this fixture is photographed too.** It was `Part 1` … `Part
# 4`, which is fine for a contour receipt and gives a store page away instantly
# — `docs/screenshots/capture.sh` takes its smart-playlist frame here, since
# the silent fixture analyses to one vector for every track and would
# photograph the feature not working.
TITLES = [
    "Slow Return", "Winter Ferry", "Halogen", "A Careful Distance",
    "Saltmarsh", "Every Little Light", "The Wire Fence", "Nine Bells",
    "Marginalia", "Signal Fire", "Coastal Path", "Lantern",
    "Bell Foundry", "Overcast", "Stray Current", "Sea Fret",
    "Meridian", "Rushlight", "The Turning Year", "Copperplate",
    "Northerly", "Gasworks", "Understory", "The Last Post",
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
        # **One beat, then repeated.** The signal is periodic by construction —
        # a sine under a click train — so writing it sample by sample for the
        # whole track is the same arithmetic over and over. Building one beat
        # and tiling it is what makes a fixture long enough to fill a real
        # request affordable: the tracks were 24 s each because 24 s was what
        # a per-sample loop could be asked for, and 24 s tracks cannot fill
        # the shortest length baz offers, so every frame taken here carried a
        # shortfall note.
        #
        # The seam is a phase jump in the 220 Hz tone at each beat boundary,
        # which is inaudible at −30 dBFS and identical across every track, so
        # it moves no track relative to another — which is the only thing this
        # fixture is for.
        beat = bytearray()
        for n in range(period):
            phase = n / rate
            tone = math.sin(2 * math.pi * 220.0 * phase) * 0.35
            # A short exponential click on the beat: the onset bliss reads.
            click = math.exp(-n / (rate * 0.012)) if n < rate * 0.05 else 0.0
            value = (tone + click) * track_loud * peak
            packed = struct.pack("<h", max(-32767, min(32767, int(value * 32767))))
            beat += packed + packed
        samples = (beat * (frames // period + 1))[: frames * 4]
        wav = tmp / f"{index}-{track}.wav"
        with wave.open(str(wav), "wb") as out:
            out.setnchannels(2)
            out.setsampwidth(2)
            out.setframerate(rate)
            out.writeframes(bytes(samples))
        flac = folder / f"{track:02d} {TITLES[(index * 7 + track) % len(TITLES)]}.flac"
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
                f"--set-tag=TITLE={TITLES[(index * 7 + track) % len(TITLES)]}",
                str(flac),
            ],
            check=True,
        )
        wav.unlink()
print("built", len(records) * 4, "tracks across", len(records), "records")
PY

# **Sleeves**, because the frame that uses this fixture has a lane and a bottom
# bar in it and both draw the record's cover. Six two-tone fields with the name
# on them: nothing here is measured, so they only have to look like records.
i=0
for dir in "$FIX"/*/; do
  i=$((i + 1))
  hue=$(( (i * 57) % 360 ))
  name=$(basename "$dir")
  rest=${name#* - }
  artist=${rest%% - *}
  album=${rest#* - }
  magick -size 600x600 "xc:hsl(${hue},34%,26%)" \
    -fill "hsl($(( (hue + 30) % 360 )),52%,58%)" -draw "polygon 0,600 600,0 600,600" \
    -fill "hsl(${hue},10%,94%)" -pointsize 30 -gravity northwest -annotate +44+44 "$artist" \
    -fill "hsl(${hue},14%,80%)" -pointsize 24 -gravity northwest -annotate +44+86 "$album" \
    -attenuate 0.28 +noise Gaussian -quality 92 "$dir/cover.jpg"
done

echo "tracks: $(find "$FIX" -name '*.flac' | wc -l)  records: $(find "$FIX" -mindepth 1 -maxdepth 1 -type d | wc -l)"
du -sh "$FIX"
