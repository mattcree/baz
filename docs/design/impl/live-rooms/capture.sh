#!/usr/bin/env bash
# **Item 54 — more rooms, and they stand the moment you press them.**
#
# The owner, 2026-08-15: *"lets create more interesting themes for the app too,
# and ideally can we apply them upon selection."*
#
# Both halves in one run, and the second half is the one that could not be
# argued in prose: the frames below are **one process**. Nothing is restarted
# between them — the room is pressed in Settings and the next frame is drawn in
# it, including the glyph sheets, which used to be rasterized once per process
# in the room's ink and were the reason the picker said *"applies on restart"*.
#
#   1  Closing Time, the room baz starts in, on the Library wall.
#   2  Settings, with the six rooms listed.
#   3  Blue Hour, pressed — the same process, the next frame.
#   4  The wall in Blue Hour: sleeves, chrome, glyphs and the amber lamp.
#   5  Sea Glass, pressed — a light room, from a dark one, with no restart.
#   6  The wall in Sea Glass.
#
#   toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
#     cargo build --release -p baz --features device-output
#   toolbox run -c baz-dev env FIX=/tmp/baz-review-fix \
#     docs/design/impl/live-rooms/capture.sh
set -uo pipefail

REPO=${REPO:-$(git rev-parse --show-toplevel)}
BIN=${BIN:-$REPO/target/tb/release/baz}
FIX=${FIX:-/tmp/baz-review-fix}
OUT=${OUT:-$REPO/docs/design/impl/live-rooms}
DISP=${DISP:-:206}
S=/tmp/baz-rooms-scratch
W=1280
H=860

mkdir -p "$OUT"
rm -rf "$S"
mkdir -p "$S"/{home,data,config,cache,run}
chmod 700 "$S/run"
cat > "$S/home/.asoundrc" <<'EOF'
pcm.!default { type null }
ctl.!default { type null }
EOF

mkdir -p "$S/config/baz"
cat > "$S/config/baz/config.toml" <<EOF
music_dirs = [
    "$FIX",
]
group_key = "alphabet"
EOF

Xvfb "$DISP" -screen 0 "${W}x${H}x24" -nolisten tcp &
XPID=$!
sleep 1
# **`BAZ_ROOM` is deliberately unset**: it is the development hatch that pins a
# room at startup, and pinning one would photograph the thing this run exists
# to disprove.
env -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS DISPLAY="$DISP" \
    WINIT_UNIX_BACKEND=x11 HOME="$S/home" XDG_DATA_HOME="$S/data" \
    XDG_CONFIG_HOME="$S/config" XDG_CACHE_HOME="$S/cache" \
    XDG_RUNTIME_DIR="$S/run" \
    "$BIN" >> "$S/app.log" 2>&1 &
APID=$!
WID=""
for _ in $(seq 1 40); do
  WID=$(DISPLAY=$DISP timeout 3 xdotool search --sync --onlyvisible --class baz 2>/dev/null | head -1)
  [[ -n $WID ]] && break
  sleep 0.25
done
if [[ -z $WID ]]; then echo "NO WINDOW"; cat "$S/app.log"; kill "$APID" "$XPID" 2>/dev/null; exit 1; fi
export DISPLAY=$DISP
xdotool windowmove "$WID" 0 0
xdotool windowsize "$WID" "$W" "$H"
xdotool windowfocus --sync "$WID"
sleep 6

shot()  { sleep 1.2; magick import -window root "$OUT/$1.png"; echo "  shot $1"; }
click() { xdotool mousemove "$1" "$2"; sleep 0.3; xdotool click 1; sleep 1.4; }
park()  { xdotool mousemove 1180 700; sleep 0.5; xdotool mousemove 1182 702; sleep 0.9; }

LANE_X=32
LIB_Y=133
# Settings is the gear in the app bar's right cluster, not a lane row, and it
# opens on **Playback**: the room picker is the third section down its own
# lane, and pressing where the rooms will be *before* choosing it lands on
# ReplayGain instead.
GEAR_X=1102 GEAR_Y=24
APPEARANCE_X=372 APPEARANCE_Y=250
# The six rooms, in the order `theme_file::BUILTINS` lists them, and each pair
# is dark-then-light so a listener reads the ladder rather than a bag: Closing
# Time · Blue Hour · Stone · Sea Glass · Plaster · Reading Room. Read off
# frame 02: the first row's centre, and the stack's 42 px pitch.
ROOM_X=810 ROOM1_Y=212 ROOM_PITCH=42

click $LANE_X $LIB_Y
park
shot "01-closing-time-wall-${W}x${H}"

click $GEAR_X $GEAR_Y
click $APPEARANCE_X $APPEARANCE_Y
park
shot "02-the-six-rooms-${W}x${H}"

# Blue Hour is the second row.
click $ROOM_X $((ROOM1_Y + ROOM_PITCH))
park
shot "03-blue-hour-standing-${W}x${H}"
click $LANE_X $LIB_Y
park
shot "04-blue-hour-wall-${W}x${H}"

# Sea Glass is the fourth.
click $GEAR_X $GEAR_Y
click $APPEARANCE_X $APPEARANCE_Y
sleep 0.6
click $ROOM_X $((ROOM1_Y + 3 * ROOM_PITCH))
park
shot "05-sea-glass-standing-${W}x${H}"
click $LANE_X $LIB_Y
park
shot "06-sea-glass-wall-${W}x${H}"

kill "$APID" 2>/dev/null; wait "$APID" 2>/dev/null
kill "$XPID" 2>/dev/null; wait "$XPID" 2>/dev/null
echo "--- isolation receipt ---"
grep -m1 mpris "$S/app.log" || echo "NO MPRIS LINE — the isolation is unproven"
