#!/usr/bin/env bash
# Cold start with NO interaction at all: does the visible wall decode itself?
set -euo pipefail
W=1280; H=860
OUT=${OUT:-/tmp/scratch/cold}
DISP=:95
rm -rf "$OUT" /tmp/scratch/data /tmp/scratch/config /tmp/scratch/cache
mkdir -p "$OUT" /tmp/scratch/{data,config,cache}
Xvfb "$DISP" -screen 0 ${W}x${H}x24 -nolisten tcp >/dev/null 2>&1 &
XVFB=$!
sleep 1
export DISPLAY=$DISP

env -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS DISPLAY="$DISP" \
  WINIT_UNIX_BACKEND=x11 HOME=/tmp/scratch/home \
  XDG_DATA_HOME=/tmp/scratch/data XDG_CONFIG_HOME=/tmp/scratch/config \
  XDG_CACHE_HOME=/tmp/scratch/cache XDG_RUNTIME_DIR=/tmp/scratch/run \
  BAZ_MSG_LOG=1 BAZ_PERF_LOG=1 \
  ./target/debug/baz /tmp/scratch/music >"$OUT/run.log" 2>&1 &
APP=$!
trap 'kill $APP $XVFB 2>/dev/null || true' EXIT

WID=""
for _ in $(seq 1 60); do
  WID=$(timeout 3 xdotool search --sync --onlyvisible --class baz 2>/dev/null | head -1) || true
  [ -n "$WID" ] && break
  sleep 0.5
done
xdotool windowmove "$WID" 0 0
xdotool windowsize "$WID" "$W" "$H"
# **No focus, no pointer, no click.** The pointer is parked off the window at
# the start and never moves again; nothing below touches the app.
xdotool mousemove 5 855

for t in 3 6 9 12 15; do
  sleep 3
  magick import -window root -crop "${W}x${H}+0+0" +repage "$OUT/t${t}.png"
done

echo "--- decodes completed, over the run:"
grep -o "completed=[0-9]*" "$OUT/run.log" | tail -1 || echo "(no [art] thumb lines at all)"
grep -c "\[art\] thumb" "$OUT/run.log" || true
echo "--- frames identical from t=6 on?"
for t in 9 12 15; do
  printf "t6 vs t%s: " "$t"
  magick compare -metric AE "$OUT/t6.png" "$OUT/t${t}.png" null: 2>&1 || true
  echo
done
