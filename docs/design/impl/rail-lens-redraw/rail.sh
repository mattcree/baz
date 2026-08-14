set -euo pipefail
W=1280; H=860
OUT=/tmp/scratch/rail
DISP=:93
rm -rf "$OUT" /tmp/scratch/data /tmp/scratch/config /tmp/scratch/cache
mkdir -p "$OUT" /tmp/scratch/{data,config,cache}
Xvfb "$DISP" -screen 0 ${W}x${H}x24 -nolisten tcp >/dev/null 2>&1 & XVFB=$!
sleep 1; export DISPLAY=$DISP
env -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS DISPLAY="$DISP" \
  WINIT_UNIX_BACKEND=x11 HOME=/tmp/scratch/home \
  XDG_DATA_HOME=/tmp/scratch/data XDG_CONFIG_HOME=/tmp/scratch/config \
  XDG_CACHE_HOME=/tmp/scratch/cache XDG_RUNTIME_DIR=/tmp/scratch/run \
  ./target/debug/baz /tmp/scratch/music >"$OUT/run.log" 2>&1 & APP=$!
trap 'kill $APP $XVFB 2>/dev/null || true' EXIT
WID=""; for _ in $(seq 1 60); do WID=$(timeout 3 xdotool search --sync --onlyvisible --class baz 2>/dev/null | head -1) || true; [ -n "$WID" ] && break; sleep 0.5; done
xdotool windowmove "$WID" 0 0; xdotool windowsize "$WID" $W $H
# Park far from the rail and let everything settle.
xdotool mousemove 600 400; sleep 8
magick import -window root -crop "60x700+1220+120" +repage "$OUT/rest.png"
# One single move onto the rail. Nothing else happens afterwards.
xdotool mousemove 1245 400
sleep 2
magick import -window root -crop "60x700+1220+120" +repage "$OUT/hover.png"
# Nudge one pixel: a second motion event, still nothing else.
xdotool mousemove 1245 401
sleep 2
magick import -window root -crop "60x700+1220+120" +repage "$OUT/hover2.png"
printf "rest vs hover : "; magick compare -metric AE "$OUT/rest.png" "$OUT/hover.png" null: 2>&1 || true; echo
printf "rest vs hover2: "; magick compare -metric AE "$OUT/rest.png" "$OUT/hover2.png" null: 2>&1 || true; echo
printf "hover vs hover2: "; magick compare -metric AE "$OUT/hover.png" "$OUT/hover2.png" null: 2>&1 || true; echo
# Now a long slow sweep down the rail, sampling as we go — the real gesture.
for y in 200 240 280 320 360 400 440 480; do
  xdotool mousemove 1245 $y; sleep 0.8
  magick import -window root -crop "60x700+1220+120" +repage "$OUT/sweep-$y.png"
done
echo "--- consecutive sweep frames that are IDENTICAL (the lens did not follow):"
prev=""
for y in 200 240 280 320 360 400 440 480; do
  if [ -n "$prev" ]; then
    d=$(magick compare -metric AE "$OUT/sweep-$prev.png" "$OUT/sweep-$y.png" null: 2>&1 || true)
    echo "  $prev -> $y : $d"
  fi
  prev=$y
done
# The snap back: leave the lane entirely and confirm the rest frame returns.
xdotool mousemove 600 400; sleep 2
magick import -window root -crop "60x700+1220+120" +repage "$OUT/left.png"
printf "rest vs left (0 = snapped back): "; magick compare -metric AE "$OUT/rest.png" "$OUT/left.png" null: 2>&1 || true; echo
magick "$OUT/sweep-280.png" -filter point -resize 300% "$OUT/zoom.png"
