#!/usr/bin/env bash
# **The background visualisation, away from the place that owns it** — item 84.
#
# The owner, 2026-08-22: *"can you make sure when we switch to other screens and
# the visualizer stays in the background that it continues animating, but it
# should be heavily blurred or opaque? … almost that liquid glass look, then I
# might like it."*
#
# He is deciding whether he wants the background visualiser at all, so the
# deliverable is a picture rather than a passing test. This takes four: the
# unveiled Now playing that is the control, and three places where the veiled
# ground has to stay a ground.
#
#   toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb cargo build --release -p baz
#   toolbox run -c baz-dev docs/design/impl/liquid-glass/capture.sh
#
# Headless and isolated six ways, exactly as `docs/screenshots/capture.sh` is
# and for the same reasons — the `[mpris] no session bus` line it prints is the
# receipt that nothing reached the owner's desktop, library or session bus.
set -uo pipefail

REPO=${REPO:-$(git rev-parse --show-toplevel)}
BIN=${BIN:-$REPO/target/tb/release/baz}
OUT=${OUT:-$REPO/docs/design/impl/liquid-glass}
DISP=${DISP:-:197}
S=${S:-/tmp/baz-glass-scratch}
W=1600; H=900

FIX=${FIX:-/tmp/baz-shot-fix}

# **The fixture, not his own library** — and that is a finding rather than a
# preference. `docs/screenshots/capture.sh` photographs his real collection,
# and it can: it only needs the *index* and the *art cache*, both of which are
# files on this machine. This capture needs something to actually **play**, and
# his music is on SMB shares reached through gvfs. An isolated run has no gvfs,
# so the first attempt photographed a wall of grey placeholders under
# `Nothing playing` and `3 folders are not reachable`. A visualisation of
# silence is not a picture of anything.
STAMP=$(sha256sum "$REPO/docs/design/composition/tools/mkfixture.sh" | cut -c1-16)
if [[ ! -d $FIX || $(cat "$FIX/.generator" 2>/dev/null) != "$STAMP" ]]; then
  rm -rf "$FIX"
  LONG_TITLE="Nightjar" "$REPO/docs/design/composition/tools/mkfixture.sh" "$FIX"
  echo "$STAMP" > "$FIX/.generator"
fi


rm -rf "$S"; mkdir -p "$S"/{home,data,config,cache,run}; chmod 700 "$S/run"
cat > "$S/home/.asoundrc" <<'EOF'
pcm.!default { type null }
ctl.!default { type null }
EOF
mkdir -p "$S/config/baz" "$S/data/baz" "$S/cache/baz"
# **And the fixture is silent, which is the second finding.**
#
# `mkfixture.sh` writes zeroes on purpose — a capture must be inaudible twice
# over — and a visualisation of zeroes is a flat line, so the first run of this
# script photographed a Now playing with nothing moving on it.
#
# So the fixture is copied aside and the album this script presses `Play` on is
# given a signal: a four-tone chord with a slow vibrato on two of its voices
# and a swell over the whole thing, so every band has something in it and the
# field moves. Nothing about the safety changes — the scratch `HOME` still
# routes ALSA's default PCM to `null`, and `BAZ_DEVICE_TESTS` stays unset.
#
# **The tracks are ten minutes each, and that is not arbitrary.** A `null` PCM
# accepts samples as fast as they are written, so a run consumes a record far
# faster than its running time — the same property the Flatpak's own ALSA
# routing had to fix. Thirty-second tracks were raced to the end of the album
# before the third frame and every shot after the first read `Nothing playing`.
MUSIC="$S/music"
rm -rf "$MUSIC"; cp -r "$FIX" "$MUSIC"
# The tile this script presses is the second of the first shelf on a
# `year`-grouped wall, and that is Werkbund. Named rather than derived, and
# checked against the engine's own log at the end.
SOUNDING="$MUSIC/06 - Studio Hain - Werkbund"
[[ -d $SOUNDING ]] || { echo "the fixture no longer holds $SOUNDING"; exit 1; }
python3 - "$S/minute.raw" <<'PYEOF'
import math, struct, sys
rate, seconds = 44100, 60
out = open(sys.argv[1], "wb")
chunk = bytearray()
for frame in range(rate * seconds):
    t = frame / rate
    value = (
        0.30 * math.sin(2 * math.pi * 110 * t)
        + 0.24 * math.sin(2 * math.pi * 440 * t * (1 + 0.08 * math.sin(t / 2.7)))
        + 0.16 * math.sin(2 * math.pi * 2200 * t * (1 + 0.05 * math.sin(t / 1.9)))
        + 0.10 * math.sin(2 * math.pi * 7000 * t)
    ) * (0.55 + 0.45 * math.sin(t / 4.3))
    sample = int(max(-1.0, min(1.0, value)) * 22000)
    chunk += struct.pack("<hh", sample, sample)
    if len(chunk) >= 1 << 20:
        out.write(chunk); chunk = bytearray()
out.write(chunk); out.close()
PYEOF
# Ten minutes, as ten copies of one — the seam every sixty seconds is nothing
# a spectrum shows, and generating it once costs a tenth of the time.
: > "$S/signal.raw"
for _ in $(seq 1 10); do cat "$S/minute.raw" >> "$S/signal.raw"; done
flac --totally-silent -0 --force-raw-format --endian=little --sign=signed \
     --channels=2 --bps=16 --sample-rate=44100 -o "$S/signal.flac" "$S/signal.raw"
rm -f "$S/signal.raw" "$S/minute.raw"
sounded=0
for track in "$SOUNDING"/*.flac; do
  metaflac --export-tags-to="$S/tags.txt" "$track"
  cp "$S/signal.flac" "$track"
  metaflac --import-tags-from="$S/tags.txt" "$track"
  sounded=$((sounded + 1))
done
rm -f "$S/signal.flac"
echo "== $sounded tracks of $(basename "$SOUNDING") carry a signal"
[[ $sounded -ge 5 ]] || { echo "the album was not sounded"; exit 1; }

cat > "$S/config/baz/config.toml" <<EOF
music_dirs = [
    "$MUSIC",
]
group_key = "year"
density = "compact"
sidebar_open = true
check_for_updates = false
EOF

Xvfb "$DISP" -screen 0 ${W}x${H}x24 -nolisten tcp >/dev/null 2>&1 &
XPID=$!
sleep 2
export DISPLAY=$DISP
cleanup() {
  [[ -n ${APID:-} ]] && kill "$APID" 2>/dev/null
  local pid; pid=$(pgrep -x -f "$BIN" || true)
  [[ -n $pid ]] && kill $pid 2>/dev/null
  kill "$XPID" 2>/dev/null
  return 0
}
trap cleanup EXIT INT TERM

env -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS DISPLAY="$DISP" \
    WINIT_UNIX_BACKEND=x11 HOME="$S/home" XDG_DATA_HOME="$S/data" \
    XDG_CONFIG_HOME="$S/config" XDG_CACHE_HOME="$S/cache" \
    XDG_RUNTIME_DIR="$S/run" BAZ_ROOM=closing-time \
    "$BIN" >> "$S/app.log" 2>&1 &
APID=$!

WID=""
for _ in $(seq 1 80); do
  WID=$(timeout 3 xdotool search --sync --onlyvisible --class baz 2>/dev/null | head -1)
  [[ -n $WID ]] && break
  sleep 0.25
done
[[ -z $WID ]] && { echo "NO WINDOW"; tail -30 "$S/app.log"; exit 1; }
xdotool windowmove "$WID" 0 0
xdotool windowsize "$WID" "$W" "$H"
xdotool windowfocus --sync "$WID"
sleep 20

shot()  { sleep 1.2; magick import -window root -crop "${W}x${H}+0+0" +repage "$OUT/$1.png"; echo "  shot $1"; }
probe() { sleep 0.8; magick import -window root -crop "${W}x${H}+0+0" +repage "$S/$1.png"; echo "  probe $1"; }
click() { xdotool mousemove "$1" "$2"; sleep 0.4; xdotool click 1; sleep 1.6; }
rest()  { xdotool mousemove "$1" "$2"; sleep 1.5; }
park()  { xdotool mousemove 1200 78; sleep 0.5; xdotool mousemove 1202 80; sleep 0.9; }

# Put a record on, through the tile's own `Play` — his wall hangs five to a
# row, columns at 370, 615, 860, 1104, 1349 (docs/screenshots/capture.sh).
rest 648 255
click 596 198
sleep 2
# Deliberate playback lands on Now playing once the engine confirms the start.
probe now-playing-raw

# The visualisation cycle lives on Now playing and is `Off` on a fresh config,
# so one press is `Spectrum`. `$CYCLE_X`/`$CYCLE_Y` are read off the probe.
CYCLE_X=${CYCLE_X:-1242}; CYCLE_Y=${CYCLE_Y:-24}
click $CYCLE_X $CYCLE_Y
park
shot now-playing

# And the three places where it has to be weather rather than an instrument.
click 105 133; park; shot library
click 105 81;  park; shot home
click 1422 24; park; shot settings

echo "== receipts"
grep -m1 'mpris' "$S/app.log" || echo "  (no mpris line — check the isolation)"
# **Say so if the pictures are of silence.** Two ways this has already gone
# wrong without failing: the album that played was not the one given a signal,
# and the album ran out before the last frame. Both produce a plausible
# screenshot of nothing happening.
if ! grep -q "track started.*$(basename "$SOUNDING")" "$S/app.log"; then
  echo "THE SOUNDED ALBUM NEVER PLAYED — the frames are of silence."
  grep -m3 'track started' "$S/app.log"
  exit 1
fi
if grep -q 'queue ended' "$S/app.log"; then
  echo "THE RECORD RAN OUT BEFORE THE LAST FRAME — lengthen the signal."
  exit 1
fi
echo "  sounding: $(grep -c 'track started' "$S/app.log") track(s) started, queue still running"
