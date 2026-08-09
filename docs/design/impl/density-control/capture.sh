#!/usr/bin/env bash
# Render ADR-0028's density detents headless, on a private Xvfb, with all six
# XDG redirections from docs/DEVELOPMENT.md. Nothing touches the owner's
# session; the run's `[mpris] no session bus` line is the receipt that it did
# not, and this script prints it. Processes are stopped by pid, never by name.
#
# What it shows against the real binary, at 1280×860:
#
#   1. **The marks at rest** — three detents at the foot of the index rail's
#      lane, `Balanced` (the default) at full glyph ink, the wall at
#      4 columns.
#   2. **A press on the top mark** — the real click, not a seeded config:
#      the wall re-hangs at `Spacious` (3 × 320) and the full-ink mark moves
#      to the top. The marks themselves do not move: the lane's geometry is
#      constant across steps.
#   3. **A press on the bottom mark from Spacious** — a two-notch jump in
#      one press, which is the mirror delta (`Density::steps_to`) at work:
#      the wall re-hangs at `Dense` (5 columns), the full-ink mark moves to
#      the bottom.
#   4. **A mark's tooltip** — the icon-only law's accessible name, `Spacious`
#      beside the hovered mark.
#
# A cropped strip of the lane's foot across the three steps is composed at
# the end (`05-lane-strip.png`), so the moving active mark can be read at a
# glance.
#
# Build the binary **inside the toolbox** (a host-built release binary links
# a newer glibc than the container has and dies before it draws):
#
#   toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
#     cargo build --release -p baz --features device-output
#   toolbox run -c baz-dev docs/design/composition/tools/mkfixture.sh /tmp/baz-density-fix
#   toolbox run -c baz-dev docs/design/impl/density-control/capture.sh
set -uo pipefail

REPO=${REPO:-$(git rev-parse --show-toplevel)}
BIN=${BIN:-$REPO/target/tb/release/baz}
FIX=${FIX:-/tmp/baz-density-fix}
OUT=${OUT:-$REPO/docs/design/impl/density-control}
DISP=${DISP:-:199}
S=/tmp/baz-density-scratch

mkdir -p "$OUT"
rm -rf "$S"
mkdir -p "$S"/{home,data,config,cache,run}
chmod 700 "$S/run"
# Two independent guarantees that nothing is audible: the sink discards every
# sample, and the fixture's samples are all zero (docs/DEVELOPMENT.md).
cat > "$S/home/.asoundrc" <<'EOF'
pcm.!default { type null }
ctl.!default { type null }
EOF

Xvfb "$DISP" -screen 0 1280x860x24 -nolisten tcp &
XPID=$!
sleep 1

launch() {
  env -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS DISPLAY="$DISP" \
      WINIT_UNIX_BACKEND=x11 HOME="$S/home" XDG_DATA_HOME="$S/data" \
      XDG_CONFIG_HOME="$S/config" XDG_CACHE_HOME="$S/cache" \
      XDG_RUNTIME_DIR="$S/run" BAZ_ROOM=closing-time \
      "$BIN" "$@" >> "$S/app.log" 2>&1 &
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
  xdotool windowsize "$WID" 1280 860
  xdotool windowfocus --sync "$WID"
  sleep 2
}

stop_app() { kill "$APID" 2>/dev/null; wait "$APID" 2>/dev/null; }
shot()  { sleep 0.9; magick import -window root "$OUT/$1.png"; echo "  shot $1"; }
click() { xdotool mousemove "$1" "$2"; sleep 0.4; xdotool click 1; sleep 0.9; }
# Park in the lane's dead foot — below the marks, above the bar — a spot
# that hovers nothing at any density (parking on the wall put a hover rule
# under whichever cover grew beneath the pointer).
park()  { xdotool mousemove 1232 766; sleep 0.4; }

# The marks' geometry, derived from the tokens rather than eyeballed:
# x — the lane's foot is right-aligned so the sprite's ink lands on
#     W − HANG; the STEPPER_HIT 24 box is centred on
#     1280 − (HANG − MARK_INSET) − STEPPER_HIT/2 = 1280 − 36 − 12 = 1232.
# y — the body ends at 860 − BAR_CONTENT_H 80 = 780; the marks keep one
#     HANG 40 above the bar; three 24 px boxes bottom-up from 740:
#     Dense centres at 728, Balanced 704, Spacious 680.
MX=1232
Y_SPACIOUS=680
Y_DENSE=728

launch "$FIX"
# ---- 1: the marks at rest, Balanced lit --------------------------------
park; shot 01-marks-balanced
# ---- 2: the real press on the top mark → Spacious ----------------------
click $MX $Y_SPACIOUS
park; shot 02-marks-spacious
# ---- 3: the bottom mark from Spacious — a two-notch mirror delta -------
click $MX $Y_DENSE
park; shot 03-marks-dense
# ---- 4: the accessible name --------------------------------------------
xdotool mousemove $MX $Y_SPACIOUS
sleep 1.2
shot 04-mark-tooltip
stop_app

kill "$XPID" 2>/dev/null
wait "$XPID" 2>/dev/null

# ---- 5: the lane's foot, three steps side by side ----------------------
magick "$OUT/01-marks-balanced.png" -crop 140x140+1140+640 "$S/lane-balanced.png"
magick "$OUT/02-marks-spacious.png" -crop 140x140+1140+640 "$S/lane-spacious.png"
magick "$OUT/03-marks-dense.png"    -crop 140x140+1140+640 "$S/lane-dense.png"
magick "$S/lane-spacious.png" "$S/lane-balanced.png" "$S/lane-dense.png" \
  +append "$OUT/05-lane-strip.png"
echo "  composed 05-lane-strip (spacious · balanced · dense)"

echo "--- receipt ---"
grep -m1 "\[mpris\]" "$S/app.log" || echo "NO RECEIPT IN LOG"
echo "done."
