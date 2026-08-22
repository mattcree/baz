#!/usr/bin/env bash
# **Selecting more than one thing, and the strip that spends it** — item 62.
#
# `docs/design/18-feature-parity.md` §4 calls multi-select the floor every
# other list feature stands on, and the part worth photographing is the part
# that is *new to look at*: a set of rows lit at once, and the strip at the
# foot of the place naming how many and what can be done with them.
#
#   toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb cargo build --release -p baz
#   toolbox run -c baz-dev docs/design/impl/multi-select/capture.sh
#
# Headless and isolated six ways, exactly as `docs/screenshots/capture.sh` is
# and for the same reasons; it prints the `[mpris] no session bus` receipt.
set -uo pipefail

REPO=${REPO:-$(git rev-parse --show-toplevel)}
BIN=${BIN:-$REPO/target/tb/release/baz}
OUT=${OUT:-$REPO/docs/design/impl/multi-select}
DISP=${DISP:-:196}
S=${S:-/tmp/baz-marks-scratch}
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
    # **And the two channels differ**, or the stereo image (item 85) is a
    # vertical line — the correct reading for a mono file and a useless
    # picture of the visualisation. The 2200 Hz voice is panned by a slowly
    # turning law and the 7 kHz one is inverted between the channels, which
    # gives the goniometer a cloud with both a middle and a width.
    pan = 0.5 + 0.45 * math.sin(t / 6.1)
    wide = 0.16 * math.sin(2 * math.pi * 2200 * t * (1 + 0.05 * math.sin(t / 1.9)))
    edge = 0.10 * math.sin(2 * math.pi * 7000 * t)
    common = value - wide - edge
    left = common + wide * pan + edge
    right = common + wide * (1.0 - pan) - edge
    chunk += struct.pack(
        "<hh",
        int(max(-1.0, min(1.0, left)) * 22000),
        int(max(-1.0, min(1.0, right)) * 22000),
    )
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

# **Open a record**, through the tile's own `Open` — the fourth of the four
# choices its hover raises, at +182 from the tile's top
# (`docs/screenshots/capture.sh` derived these from a frame; the wall here is
# the same fixture at the same size).
rest 648 255
click 596 354
sleep 2
park
probe album-page

# **Ctrl over three rows that are not adjacent**, which is the half a range
# cannot do. `$ROW_1`/`$ROW_PITCH` are read off the probe.
ROW_1=${ROW_1:-301}; ROW_PITCH=${ROW_PITCH:-52}; ROW_X=${ROW_X:-700}
for row in 0 2 4; do
  xdotool mousemove $ROW_X $((ROW_1 + row * ROW_PITCH))
  sleep 0.3
  xdotool keydown ctrl; xdotool click 1; xdotool keyup ctrl
  sleep 0.8
done
park
shot ctrl-picked

# **And Shift over a range**, from the anchor the last press left.
xdotool mousemove $ROW_X $((ROW_1 + 8 * ROW_PITCH))
sleep 0.3
xdotool keydown shift; xdotool click 1; xdotool keyup shift
sleep 1.0
park
shot shift-range

# **Esc puts it down**, which is the way out that is not a word.
xdotool key --clearmodifiers Escape
sleep 1.0
park
shot cleared

echo "== receipts"
grep -m1 'mpris' "$S/app.log" || echo "  (no mpris line — check the isolation)"
