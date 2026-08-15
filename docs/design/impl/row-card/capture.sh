#!/usr/bin/env bash
# **Item 53 — the card reaches the row's controls.**
#
# The owner, 2026-08-15: *"can we make sure the playlist row controls are
# inside the highlighted row as well."* A track row's highlight was painted by
# the row's *button*, and every surface that has one hangs its heart, its `+`
# and its ▲▼✕ off the side of that button — so a hovered row lit up to the
# duration and then stopped, with two or four unlit controls sitting on bare
# wall beside it.
#
# The frames below are the whole argument, and each one is the pointer
# **resting on a row** rather than pressing it:
#
#   1  A record's page, pointer on track 3. The lit card now runs past the
#      duration, under the heart and the transfer `+`, to the content lane's
#      own right edge (law L5).
#   2  A saved playlist's page, pointer on a row. The same card, now under
#      four controls — heart, ▲, ▼, ✕ — which is the surface the owner was
#      looking at when he said it.
#   3  Favourites, pointer on a row. The place had no hover answer at all
#      before this (its rows took the button's own `Status`), so it gained
#      `hovered_favourite_row` with the frame to show for it.
#
# Headless, on a private Xvfb, with all six XDG redirections from
# docs/DEVELOPMENT.md; the `[mpris] no session bus` line printed at the end is
# the receipt that the owner's session was never touched.
#
#   toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
#     cargo build --release -p baz --features device-output
#   toolbox run -c baz-dev docs/design/composition/tools/mkfixture.sh /tmp/baz-row-fix
#   toolbox run -c baz-dev env FIX=/tmp/baz-row-fix \
#     docs/design/impl/row-card/capture.sh
set -uo pipefail

REPO=${REPO:-$(git rev-parse --show-toplevel)}
BIN=${BIN:-$REPO/target/tb/release/baz}
FIX=${FIX:-/tmp/baz-row-fix}
OUT=${OUT:-$REPO/docs/design/impl/row-card}
DISP=${DISP:-:203}
S=/tmp/baz-row-scratch
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

# One saved list to open. Favourites is **not** a file to seed — membership is
# durable library state — so the run marks three of them by pressing hearts,
# which is also the shortest proof that the heart under the new card still
# presses.
mkdir -p "$S/data/baz/playlists"
{ echo "#EXTM3U"; find "$FIX" -name '*.flac' | sort | head -8; } \
  > "$S/data/baz/playlists/Road Trip.m3u8"

Xvfb "$DISP" -screen 0 "${W}x${H}x24" -nolisten tcp &
XPID=$!
sleep 1
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
xdotool windowsize "$WID" "$W" "$H"
xdotool windowfocus --sync "$WID"
sleep 6

shot()  { sleep 1.2; magick import -window root "$OUT/$1.png"; echo "  shot $1"; }
click() { xdotool mousemove "$1" "$2"; sleep 0.3; xdotool click 1; sleep 1.2; }
# **Settle, then rest.** A page's row artwork arrives after the page does, and
# a late reflow moves the row out from under a *stationary* pointer: iced
# re-evaluates a `mouse_area`'s bounds on cursor movement, so the row publishes
# its exit and never its re-entry. So park off the list, let the thumbnails
# land, and only then move onto the row.
rest()  { xdotool mousemove 900 620; sleep 1.6; xdotool mousemove "$1" "$2"; sleep 1.0; }

# The lane's destinations, by the same arithmetic as the review pass's script:
# APP_BAR_H 49 + SIDEBAR_PAD 8 + n × 52 + 24, at the head glyph's x of 32.
LANE_X=32
LIB_Y=133 PLAYLISTS_Y=185
# The lane's lists half, under the four destinations: the one saved list this
# fixture seeds. There is no Favourites *destination* — the place is reached
# from its pinned tile on the Playlists wall, which is the wall's second cell.
LIST_X=115 LIST_Y=310
# The wall's first cell, and the labelled `Open` in its hover veil. The
# Playlists wall's first cell is the ghost `New Playlist` tile, so Favourites
# is the second: cells are 252 wide on a 24 gutter from x 276.
CELL1_X=400 CELL1_SLEEVE_Y=315 CELL1_OPEN_X=330 CELL1_OPEN_Y=410
# The veil's own labelled `Open`, measured off the wall frame: cell 2 spans
# 572–827, and the veil's four option rows put `Open` at 0.87 of the sleeve.
# Pressing the sleeve's middle instead lands on `Play`, which starts music.
CELL2_X=652 CELL2_OPEN_X=628 CELL2_OPEN_Y=378
# A row on the opened page, in the body column rather than over a control, and
# the heart's own lane at the row's right.
ROW_X=700 ROW3_Y=430
HEART_X=1178 ROW1_Y=303 ROW2_Y=355 ROW3_HEART_Y=407

click $LANE_X $LIB_Y
sleep 1
xdotool mousemove $CELL1_X $CELL1_SLEEVE_Y
sleep 0.6
click $CELL1_OPEN_X $CELL1_OPEN_Y
rest $ROW_X $ROW3_Y
shot "01-record-row-hovered-${W}x${H}"

# Three hearts, pressed through the card that now runs under them.
click $HEART_X $ROW1_Y
click $HEART_X $ROW2_Y
click $HEART_X $ROW3_HEART_Y

click $LIST_X $LIST_Y
sleep 1.5
rest $ROW_X $ROW3_Y
shot "02-playlist-row-hovered-${W}x${H}"

click $LANE_X $PLAYLISTS_Y
sleep 1
xdotool mousemove $CELL2_X $CELL1_SLEEVE_Y
sleep 0.6
click $CELL2_OPEN_X $CELL2_OPEN_Y
sleep 1.5
# Favourites holds exactly the three tracks the hearts above marked, so the
# pointer rests on row 2 rather than on the record page's third row.
rest $ROW_X $ROW2_Y
shot "03-favourites-row-hovered-${W}x${H}"

kill "$APID" 2>/dev/null; wait "$APID" 2>/dev/null
kill "$XPID" 2>/dev/null; wait "$XPID" 2>/dev/null
echo "--- isolation receipt ---"
grep -m1 mpris "$S/app.log" || echo "NO MPRIS LINE — the isolation is unproven"
