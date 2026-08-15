#!/usr/bin/env bash
# **The contour, end to end.** Analyse the fixture library for real, compose
# against a drawn shape, and hover a row of the result — because everything
# this control claims (the library's own distribution behind the line, a dot
# per chosen track, the thread between them, the lit dot for a hovered row)
# needs an analysed collection to be true rather than asserted.
#
# Headless, on a private Xvfb, with all six XDG redirections from
# docs/DEVELOPMENT.md; the run's `[mpris] no session bus` line is the receipt
# that nothing touched the owner's session.
#
#   toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
#     cargo build --release -p baz --features device-output
#   toolbox run -c baz-dev docs/design/composition/tools/mkfixture.sh /tmp/baz-review-fix
#   toolbox run -c baz-dev env FIX=/tmp/baz-review-fix \
#     docs/design/impl/contour/capture.sh
set -uo pipefail
REPO=${REPO:-$(git rev-parse --show-toplevel)}
BIN=${BIN:-$REPO/target/tb/release/baz}
FIX=${FIX:-/tmp/baz-review-fix}
OUT=${OUT:-$REPO/docs/design/impl/contour}
DISP=${DISP:-:196}
S=/tmp/baz-contour-scratch
W=1280; H=980

mkdir -p "$OUT"
rm -rf "$S"; mkdir -p "$S"/{home,data,config,cache,run}; chmod 700 "$S/run"
cat > "$S/home/.asoundrc" <<'AEOF'
pcm.!default { type null }
ctl.!default { type null }
AEOF
mkdir -p "$S/config/baz"
cat > "$S/config/baz/config.toml" <<CEOF
music_dirs = ["$FIX"]
group_key = "alphabet"
CEOF

Xvfb "$DISP" -screen 0 "${W}x${H}x24" -nolisten tcp & XPID=$!
sleep 1
env -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS DISPLAY="$DISP" \
    WINIT_UNIX_BACKEND=x11 HOME="$S/home" XDG_DATA_HOME="$S/data" \
    XDG_CONFIG_HOME="$S/config" XDG_CACHE_HOME="$S/cache" \
    XDG_RUNTIME_DIR="$S/run" BAZ_ROOM=closing-time BAZ_VIBE_WORKERS=8 \
    "$BIN" >> "$S/app.log" 2>&1 & APID=$!
WID=""
for _ in $(seq 1 60); do
  WID=$(DISPLAY=$DISP timeout 3 xdotool search --sync --onlyvisible --class baz 2>/dev/null | head -1)
  [[ -n $WID ]] && break
  sleep 0.25
done
if [[ -z $WID ]]; then echo "NO WINDOW"; tail -5 "$S/app.log"; kill $APID $XPID 2>/dev/null; exit 1; fi
export DISPLAY=$DISP
xdotool windowmove "$WID" 0 0; xdotool windowsize "$WID" $W $H; xdotool windowfocus --sync "$WID"
sleep 6
shot(){ sleep 1.0; magick import -window root "$OUT/$1.png"; echo "  shot $1"; }
click(){ xdotool mousemove "$1" "$2"; sleep 0.3; xdotool click 1; sleep 1.0; }
move(){ xdotool mousemove "$1" "$2"; sleep 0.7; }

# Playlists → the ghost tile → Vibe.
click 32 185
click 400 315
click 640 380
move 1000 900
shot "01-vibe-form-cold"

# The request, then the one press. On a cold index that press reads the
# library once and composes as soon as it can — which is the whole of what
# the old two-button consent gate was doing in two.
# The well itself, then Return — `on_submit` is `VibeCreate`, so the one press
# needs no coordinate and cannot drift with the layout. (It did: the redesign
# moved `Compose` below the fold, and a click at its old place did nothing at
# all.)
click 750 371
xdotool type --delay 40 "warm late-night listening"
sleep 0.5
move 1000 950
shot "02-request-typed"
click 750 371
xdotool key --clearmodifiers Return
sleep 4
move 1000 950
shot "03-analysing"

# **Wait for the analysis by watching its own cache grow**, rather than by
# guessing a duration: the store stops growing when there is nothing left to
# read.
DB="$S/data/baz/vibe.db"
last=-1; still=0
for _ in $(seq 1 120); do
  sleep 10
  size=$(stat -c %s "$DB" 2>/dev/null || echo 0)
  if [[ "$size" == "$last" && "$size" != "0" ]]; then
    still=$((still + 1))
    [[ $still -ge 3 ]] && break
  else
    still=0
  fi
  last=$size
done
sleep 20
move 1000 950
shot "04-composed"

# The list is below the shape, so the page is turned before its rows can be
# pointed at.
xdotool mousemove 750 700
for _ in $(seq 1 6); do xdotool click 5; sleep 0.2; done
sleep 1
move 1000 950
shot "05-list"

# **Hover one row of the result**, which is what makes the picture answerable:
# the dot for that track grows, stands on a guide to the floor, and the line
# under the shape names it in words.
move 700 700
shot "06-row-hovered"

kill "$APID" 2>/dev/null; wait "$APID" 2>/dev/null
kill "$XPID" 2>/dev/null; wait "$XPID" 2>/dev/null
echo "--- isolation receipt ---"
grep -m1 mpris "$S/app.log" || echo "NO MPRIS LINE — the isolation is unproven"
