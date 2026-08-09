#!/usr/bin/env bash
# Render the Album place's `Artist › Album` breadcrumb and the Artist place it
# opens — headless, on a private Xvfb, with all six XDG redirections from
# docs/DEVELOPMENT.md. Nothing touches the owner's session; the run's
# `[mpris] no session bus` line is the receipt, and this script prints it.
#
# The owner: *"previous and next on albums doesn't make sense on the album
# view. we could add an Artist > album breadcrumb though. and have an artist
# page."* The frames are that sentence:
#
#   1. **A record's page, with the breadcrumb** where `Album` and the
#      `‹ Prev` / `Next ›` pair used to be.
#   2. **The breadcrumb's artist half under the pointer** — it is a door, and
#      it says so the way every other word-button in the product does.
#   3. **The Artist place**: their name, `6 records · 74 tracks`, and their
#      records in the wall's own tile.
#   4. **A second record, reached from the artist page** — the round trip the
#      withdrawn stepper was trying to buy, except every record you step to is
#      one you saw before you chose it.
#   5. **And back up to the artist**, from the second record's own breadcrumb.
#      Ochre → artist → Violet Ledger → artist is four presses and no press
#      lands anywhere you could not see.
#
# `measure.py` then checks the one geometric claim: the header strip is the
# same height on the two places the breadcrumb joins, so the press is not a
# jump.
#
# Build the binary **inside the toolbox** (a host-built release binary links a
# newer glibc than the container has and dies before it draws):
#
#   toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
#     cargo build --release -p baz --features device-output
#   toolbox run -c baz-dev docs/design/composition/tools/mkfixture.sh /tmp/baz-artist-fix
#   toolbox run -c baz-dev env FIX=/tmp/baz-artist-fix \
#     docs/design/impl/artist-page/capture.sh
set -uo pipefail

REPO=${REPO:-$(git rev-parse --show-toplevel)}
BIN=${BIN:-$REPO/target/tb/release/baz}
FIX=${FIX:-/tmp/baz-artist-fix}
OUT=${OUT:-$REPO/docs/design/impl/artist-page}
DISP=${DISP:-:198}
S=/tmp/baz-artist-scratch

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

mkdir -p "$S/config/baz"
cat > "$S/config/baz/config.toml" <<EOF
music_dirs = [
    "$FIX",
]
group_key = "artist"
density = "balanced"
sidebar_open = true
EOF

launch() { # W H
  env -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS DISPLAY="$DISP" \
      WINIT_UNIX_BACKEND=x11 HOME="$S/home" XDG_DATA_HOME="$S/data" \
      XDG_CONFIG_HOME="$S/config" XDG_CACHE_HOME="$S/cache" \
      XDG_RUNTIME_DIR="$S/run" BAZ_ROOM=closing-time \
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
  xdotool windowsize "$WID" "$1" "$2"
  xdotool windowfocus --sync "$WID"
  sleep 5   # let the launch scan land
}

stop()  { kill "$APID" 2>/dev/null; wait "$APID" 2>/dev/null; sleep 0.6; }
shot()  { sleep 0.9; magick import -window root -crop "${W}x${H}+0+0" +repage \
            "$OUT/$1.png"; echo "  shot $1"; }
park()  { xdotool mousemove 40 $((H - 200)); sleep 0.6; }
key()   { xdotool key "$@"; sleep 0.6; }
click() { xdotool mousemove "$1" "$2"; sleep 0.3; xdotool click 1; sleep 1.2; }
hover() { xdotool mousemove "$1" "$2"; sleep 0.8; }

W=1280; H=860
Xvfb "$DISP" -screen 0 1920x1080x24 -nolisten tcp &
XPID=$!
sleep 1

launch $W $H

# The wall's first tile, and the breadcrumb's artist half in the header strip
# that follows: GAP_SM 8 down from the top, HANG 40 in from the body's left
# edge — which is the lane's width plus 40.
TILE_X=440; TILE_Y=250
# A tile is opened by hovering it and then pressing its *lower* half: the
# hover options cover the top, and a press landing in the same instant the
# pointer arrives is swallowed (the pattern `lane-and-home/capture.sh` uses).
OPEN_X=400; OPEN_Y=340
CRUMB_X=330; CRUMB_Y=30

# 1. A record's page. The breadcrumb leads the strip where `Album` used to.
hover $TILE_X $TILE_Y
click $OPEN_X $OPEN_Y
park
shot 01-album-page-with-the-breadcrumb

# 2. The artist half answers the pointer — it is a door.
hover $CRUMB_X $CRUMB_Y
shot 02-the-artist-half-is-a-door

# 3. The Artist place.
click $CRUMB_X $CRUMB_Y
park
shot 03-the-artist-place

# 4. A second record by the same artist, chosen from a page that showed it
#    first — the comparison the withdrawn stepper was trying to buy. Opened
#    the wall's own way, because these are the wall's own tiles.
hover 740 250
click 700 340
park
shot 04-a-second-record-from-the-artist-page

# 5. …and back up to the artist from *that* record's breadcrumb, which is the
#    round trip closed.
click $CRUMB_X $CRUMB_Y
park
shot 05-and-back-up-to-the-artist
stop

kill "$XPID" 2>/dev/null

echo
echo "--- the header strip, on the two places the breadcrumb joins ---"
"$OUT/measure.py"

echo
echo "--- the isolation receipt (docs/DEVELOPMENT.md) ---"
grep -m3 '^\[mpris\]' "$S/app.log" || echo "NO MPRIS LINE — check the log"
