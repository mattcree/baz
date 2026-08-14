set -euo pipefail
W=1280; H=860
OUT=/tmp/scratch/resize
DISP=:94
rm -rf "$OUT" /tmp/scratch/data /tmp/scratch/config /tmp/scratch/cache
mkdir -p "$OUT" /tmp/scratch/{data,config,cache}
Xvfb "$DISP" -screen 0 1920x1200x24 -nolisten tcp >/dev/null 2>&1 & XVFB=$!
sleep 1; export DISPLAY=$DISP
env -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS DISPLAY="$DISP" \
  WINIT_UNIX_BACKEND=x11 HOME=/tmp/scratch/home \
  XDG_DATA_HOME=/tmp/scratch/data XDG_CONFIG_HOME=/tmp/scratch/config \
  XDG_CACHE_HOME=/tmp/scratch/cache XDG_RUNTIME_DIR=/tmp/scratch/run \
  BAZ_PERF_LOG=1 ./target/debug/baz /tmp/scratch/music >"$OUT/run.log" 2>&1 & APP=$!
trap 'kill $APP $XVFB 2>/dev/null || true' EXIT
WID=""; for _ in $(seq 1 60); do WID=$(timeout 3 xdotool search --sync --onlyvisible --class baz 2>/dev/null | head -1) || true; [ -n "$WID" ] && break; sleep 0.5; done
xdotool windowmove "$WID" 0 0; xdotool windowsize "$WID" $W $H; xdotool mousemove 5 1195
sleep 8
before=$(grep -o "completed=[0-9]*" "$OUT/run.log" | tail -1)
magick import -window root -crop "${W}x${H}+0+0" +repage "$OUT/settled.png"
# Warm resize, wider.
xdotool windowsize "$WID" 1900 1100
sleep 0.6
magick import -window root -crop "1900x1100+0+0" +repage "$OUT/mid.png"
sleep 5
magick import -window root -crop "1900x1100+0+0" +repage "$OUT/after.png"
after=$(grep -o "completed=[0-9]*" "$OUT/run.log" | tail -1)
echo "settled: $before   after warm resize: $after"
