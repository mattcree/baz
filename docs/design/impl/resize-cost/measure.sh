#!/usr/bin/env bash
# Measure the CPU-side cost of a resize drag, headless.
#
# Run *inside* the toolbox (`toolbox run -c baz-dev measure.sh`) so the PIDs it
# starts are its own and `kill` reaches them; killing a `toolbox run` wrapper
# from the host does not reach the process inside the container.
#
# Six-variable XDG isolation, per docs/DEVELOPMENT.md. The app's
# `[mpris] no session bus` line is the receipt that no real session was touched.
set -uo pipefail

BIN=${BIN:?path to the probe-instrumented baz binary}
FIX=${FIX:?fixture music dir}
OUT=${OUT:?output log dir}
DISP=${DISP:-:191}
SCREEN=${SCREEN:-1920x1200x24}
LABEL=${LABEL:-run}
S=$(mktemp -d /tmp/baz-resize-scratch.XXXXXX)

mkdir -p "$OUT"
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
group_key = "${GROUP:-artist}"
density = "balanced"
sidebar_open = true
EOF

APID=""; XPID=""
cleanup() {
  [[ -n $APID ]] && kill "$APID" 2>/dev/null
  sleep 0.4
  # Anchored full-path reap: only this binary, never a name match, so the
  # owner's own instance on his desktop is untouchable from here.
  pkill -f "^${BIN}\$" 2>/dev/null
  [[ -n $XPID ]] && kill "$XPID" 2>/dev/null
  rm -rf "$S"
}
trap cleanup EXIT INT TERM

Xvfb "$DISP" -screen 0 "$SCREEN" -nolisten tcp &
XPID=$!
sleep 1
export DISPLAY=$DISP

env -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS DISPLAY="$DISP" \
    WINIT_UNIX_BACKEND=x11 HOME="$S/home" XDG_DATA_HOME="$S/data" \
    XDG_CONFIG_HOME="$S/config" XDG_CACHE_HOME="$S/cache" \
    XDG_RUNTIME_DIR="$S/run" BAZ_MSG_LOG=1 BAZ_PROBE=1 \
    stdbuf -oL -eL "$BIN" >> "$OUT/$LABEL.log" 2>&1 &
APID=$!

WID=""
for _ in $(seq 1 60); do
  WID=$(timeout 3 xdotool search --sync --onlyvisible --class baz 2>/dev/null | head -1)
  [[ -n $WID ]] && break
  sleep 0.25
done
if [[ -z $WID ]]; then echo "NO WINDOW"; tail -30 "$OUT/$LABEL.log"; exit 1; fi

xdotool windowmove "$WID" 0 0
xdotool windowsize "$WID" 1280 900
xdotool windowfocus --sync "$WID"
# Park the pointer off the index rail so its fisheye is not in the numbers.
xdotool mousemove 60 780
echo "### settling (scan)" >> "$OUT/$LABEL.log"
sleep "${SETTLE:-20}"

mark() { echo "### $*" >> "$OUT/$LABEL.log"; }

# A sweep: `steps` window widths delivered `pause` apart, which is the closest
# a programmatic driver gets to a dragged edge.
sweep() { # from to step pause H
  local from=$1 to=$2 step=$3 pause=$4 h=$5 w
  for ((w = from; w <= to; w += step)); do
    xdotool windowsize "$WID" "$w" "$h"
    sleep "$pause"
  done
  for ((w = to; w >= from; w -= step)); do
    xdotool windowsize "$WID" "$w" "$h"
    sleep "$pause"
  done
}

mark "IDLE 5s at 1280x900"
sleep 5

mark "SWEEP-WIDE 1000..1900 step 6 @ ~33Hz, 6 passes"
for _ in 1 2 3; do sweep 1000 1900 6 0.03 900; done

mark "IDLE 5s"
xdotool windowsize "$WID" 1280 900; sleep 5

mark "SWEEP-NARROW-1000 996..1044 step 2 @ ~33Hz (one column boundary region)"
for _ in 1 2 3 4 5 6; do sweep 996 1044 2 0.03 900; done

mark "SWEEP-NARROW-1900 1876..1924 step 2 @ ~33Hz"
xdotool windowsize "$WID" 1900 900; sleep 2
for _ in 1 2 3 4 5 6; do sweep 1876 1924 2 0.03 900; done

mark "IDLE 5s"
xdotool windowsize "$WID" 1280 900; sleep 5

# **Saturation.** No pause between steps, so the achieved rate is the app's own
# ceiling on the CPU side rather than the driver's. tiny-skia rasterises this
# window in software, so it is a *floor* on what a GPU path would manage.
mark "SATURATE 1000..1900 step 6, no pause, 6 passes"
for _ in 1 2 3; do sweep 1000 1900 6 0 900; done

mark "IDLE 5s"
xdotool windowsize "$WID" 1280 900; sleep 5

mark "SCROLL 60 wheel steps (for comparison: the other thing that redraws)"
for _ in $(seq 1 60); do xdotool click --window "$WID" 5; sleep 0.03; done
sleep 2

mark "DONE"
sleep 1
echo "wrote $OUT/$LABEL.log"
