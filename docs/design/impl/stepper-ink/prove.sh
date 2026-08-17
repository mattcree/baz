#!/usr/bin/env bash
# **The settings steppers under a pointer.**
#
# The item is about ink, so an assertion about which function the source calls
# is not the proof — the proof is the mark getting brighter. This opens
# Settings → Playback, photographs the ReplayGain stepper rows with the pointer
# parked away from them, then parks it on the Pre-amp `+` and photographs them
# again. The difference between the two frames is the whole of the fix.
#
# Private Xvfb, all six XDG variables redirected into a scratch tree.
set -euo pipefail

ROOT=$(git rev-parse --show-toplevel)
OUT="$ROOT/docs/design/impl/stepper-ink"
S=$(mktemp -d)
DISP=":${DISPLAY_NUM:-96}"
W=1400; H=900
BIN="$ROOT/target/release/baz"
FIX=${FIX:-$ROOT/target/tmp/escape-fixture}
# The Workers `+`, measured off 01-vibe-section.
PLUS_X=${PLUS_X:-1110}
PLUS_Y=${PLUS_Y:-212}

mkdir -p "$S"/{home,data,config,cache,run} "$S/config/baz" "$OUT"
chmod 700 "$S/run"
cat > "$S/config/baz/config.toml" <<EOF
music_dirs = [
    "$FIX",
]
group_key = "alphabet"
density = "compact"
sidebar_open = true
EOF

Xvfb "$DISP" -screen 0 ${W}x${H}x24 -nolisten tcp >/dev/null 2>&1 &
XPID=$!
sleep 2
export DISPLAY=$DISP
cleanup() {
  [[ -n ${APID:-} ]] && kill "$APID" 2>/dev/null
  local pid
  pid=$(pgrep -x -f "$BIN" || true)
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
  sleep 0.5
done
[[ -z $WID ]] && { echo "baz never opened a window; see $S/app.log" >&2; exit 1; }
xdotool windowmove "$WID" 0 0
xdotool windowsize "$WID" "$W" "$H"
xdotool windowfocus --sync "$WID"
sleep 4

shot() { import -window "$WID" "$OUT/$1.png"; }
# A nudge after the move: the fade is driven by frames, and a pointer that
# arrives and never moves again can leave the last frame unrequested.
park() { xdotool mousemove "$1" "$2"; sleep 0.4; xdotool mousemove $(($1 + 1)) $(($2 + 1)); sleep 1.2; }

# The gear opens Settings on Playback, where both ReplayGain stepper rows are.
park 1222 24; xdotool click 1; sleep 2
shot 00-settings-open

# **The Vibe section, not the ReplayGain rows.** The ladder's first rule is
# that a dead control stays dead, and a machine with no sound card — which is
# every Xvfb — has no engine, so both ReplayGain steppers are disabled and
# correctly ignore a pointer entirely. Measuring them proves nothing about the
# hover. `Workers` depends on the config alone and is live anywhere.
park 297 292; xdotool click 1; sleep 2
shot 01-vibe-section

# Parked well away: the pair at rest.
park 700 690
shot 02-marks-at-rest

# On the `+`. Its mark brightens; the `−` beside it does not.
park "$PLUS_X" "$PLUS_Y"
shot 03-pointer-on-plus

echo "wrote $OUT/{00-settings-open,01-vibe-section,02-marks-at-rest,03-pointer-on-plus}.png"
