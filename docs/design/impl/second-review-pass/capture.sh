#!/usr/bin/env bash
# Render the frames the 2026-08-15 review pass (items 43–51) is argued from —
# headless, on a private Xvfb, with all six XDG redirections from
# docs/DEVELOPMENT.md. Nothing touches the owner's session; the run's
# `[mpris] no session bus` line is the receipt that it did not, and this script
# prints it.
#
# What it captures against the real binary:
#
#   1280×860 — the ordinary desktop window
#     01  Library at rest — the app bar's clusters and the bottom bar's, in the
#         state every other frame is read against.
#     02  The saved-playlist wall: no place name, no tally, the ghost `New
#         Playlist` tile first, `Favourites` beside it under no heading, then
#         A–Z runs with the Library's own heading bands (items 43–45).
#     03  The same wall scrolled — a heading pinned at the viewport's top edge,
#         which is the Library's `Shelves::sticky` doing it (item 44).
#     04  The New playlist fork, opened from the ghost tile (item 45).
#     05  The Vibe form: describe, shape, compose — in that order, with the
#         first-run consent above the press it consents to (item 50).
#     06  A record's page in its two-column form (item 46's baseline).
#
#   1280×760 — a window short enough that the aside's tail overruns the body
#     07  The record's page: the aside's foot cut by the body's edge.
#     08  The same page after three wheel clicks **over the aside**: it scrolls
#         now, which is the whole of item 46. Before this it could not.
#
# Build the binary **inside the toolbox** (a host-built release binary links a
# newer glibc than the container has and dies before it draws):
#
#   toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
#     cargo build --release -p baz --features device-output
#   toolbox run -c baz-dev docs/design/composition/tools/mkfixture.sh /tmp/baz-review-fix
#   toolbox run -c baz-dev env FIX=/tmp/baz-review-fix \
#     docs/design/impl/second-review-pass/capture.sh
set -uo pipefail

REPO=${REPO:-$(git rev-parse --show-toplevel)}
BIN=${BIN:-$REPO/target/tb/release/baz}
FIX=${FIX:-/tmp/baz-review-fix}
OUT=${OUT:-$REPO/docs/design/impl/second-review-pass}
DISP=${DISP:-:197}
S=/tmp/baz-review-scratch

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
group_key = "alphabet"
EOF

# Saved lists whose initials span the alphabet, so the wall has runs to group
# and the rail has letters to jump to. Deliberately including a lower-case
# initial (`apples`), which must file under `A` beside `Aubade`.
mkdir -p "$S/data/baz/playlists"
seed() { # name  first-n-tracks
  { echo "#EXTM3U"; find "$FIX" -name '*.flac' | sort | head -"$2"; } \
    > "$S/data/baz/playlists/$1.m3u8"
}
seed "Aubade" 4
seed "apples" 3
seed "Bricolage" 6
seed "Morning Coffee" 5
seed "Zed" 2

shoot() { # W H suffix
  local W=$1 H=$2 SUF=$3
  Xvfb "$DISP" -screen 0 "${W}x${H}x24" -nolisten tcp &
  local XPID=$!
  sleep 1
  env -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS DISPLAY="$DISP" \
      WINIT_UNIX_BACKEND=x11 HOME="$S/home" XDG_DATA_HOME="$S/data" \
      XDG_CONFIG_HOME="$S/config" XDG_CACHE_HOME="$S/cache" \
      XDG_RUNTIME_DIR="$S/run" BAZ_ROOM=closing-time \
      "$BIN" >> "$S/app-$SUF.log" 2>&1 &
  local APID=$!
  local WID=""
  for _ in $(seq 1 40); do
    WID=$(DISPLAY=$DISP timeout 3 xdotool search --sync --onlyvisible --class baz 2>/dev/null | head -1)
    [[ -n $WID ]] && break
    sleep 0.25
  done
  if [[ -z $WID ]]; then echo "NO WINDOW at $SUF"; cat "$S/app-$SUF.log"; kill "$APID" "$XPID" 2>/dev/null; return 1; fi
  export DISPLAY=$DISP
  xdotool windowmove "$WID" 0 0
  xdotool windowsize "$WID" "$W" "$H"
  xdotool windowfocus --sync "$WID"
  sleep 6

  shot()  { sleep 1.0; magick import -window root "$OUT/$1.png"; echo "  shot $1"; }
  move()  { xdotool mousemove "$1" "$2"; sleep 0.5; }
  click() { xdotool mousemove "$1" "$2"; sleep 0.3; xdotool click 1; sleep 1.2; }
  wheel() { xdotool mousemove "$1" "$2"; sleep 0.2; for _ in $(seq 1 "$3"); do xdotool click 5; sleep 0.25; done; sleep 0.8; }

  # **The lane's four destinations**, computed rather than eyeballed: the app
  # bar is APP_BAR_H 49, the lane pads by SIDEBAR_PAD 8, and each row is
  # SIDEBAR_DEST_H 48 on a SIDEBAR_ROW_GAP 4 seam — so row n's centre is
  # 49 + 8 + n × 52 + 24, at the head glyph's own x of 32.
  local LANE_X=32
  local HOME_Y=81 LIB_Y=133 PLAYLISTS_Y=185
  # Empty wall to park the pointer in: over a control it would draw a tooltip
  # into every frame that follows.
  local PARK_X=$((W - 300)) PARK_Y=$((H - 260))

  # **Wall coordinates, measured off the rendered frames rather than guessed.**
  # The lane is expanded at 232, so the wall's block begins at 276; its first
  # cell is 276–528 with its caption under it. A tile's *caption* is the safe
  # press: the sleeve carries the hover veil's options, and the caption carries
  # only the tile's own select/activate.
  # The wall's hover veil puts `Open` in the last of its four option rows, at
  # 0.87 of the sleeve's height — the deterministic route to a record's page,
  # where a second press on the tile is the *select then activate* grammar and
  # depends on the first having registered.
  local CELL1_X=400 CELL1_SLEEVE_Y=315 CELL1_OPEN_X=330 CELL1_OPEN_Y=410

  if [[ $SUF == 1280x860 ]]; then
    move $PARK_X $PARK_Y
    shot "01-library-at-rest-${SUF}"
    click $LANE_X $PLAYLISTS_Y
    move $PARK_X $PARK_Y
    shot "02-playlists-wall-${SUF}"
    wheel 640 500 4
    shot "03-playlists-wall-pinned-heading-${SUF}"
    # Back to the top, then the ghost tile — the wall's first cell.
    for _ in 1 2 3 4 5 6; do xdotool click 4; sleep 0.2; done
    sleep 1
    click $CELL1_X $CELL1_SLEEVE_Y
    move $PARK_X $PARK_Y
    shot "04-new-playlist-fork-${SUF}"
    # `Vibe` is the second choice block: two full-width buttons under the
    # question, each one HANG-padded around two lines.
    click 640 380
    move $PARK_X $PARK_Y
    shot "05-vibe-form-${SUF}"
    # The contour: press a drawn shape, then take hold of a point and move it.
    # `Peak and fall` is the fourth thumbnail; the points of the line it loads
    # are then dragged, which is the whole gesture this control exists for.
    # `Peak and fall` is the fourth drawn shape; its three points then get
    # dragged, which is the gesture this control exists for. Coordinates are
    # read off frame 05 rather than guessed.
    click 688 400
    move $PARK_X $PARK_Y
    shot "11-contour-peak-and-fall-${SUF}"
    xdotool mousemove 796 455
    sleep 0.5
    xdotool mousedown 1
    sleep 0.3
    xdotool mousemove 700 500
    sleep 0.3
    xdotool mousemove 620 560
    sleep 0.5
    xdotool mouseup 1
    sleep 0.8
    move $PARK_X $PARK_Y
    shot "12-contour-dragged-${SUF}"
    click $LANE_X $LIB_Y
    move $PARK_X $PARK_Y
    sleep 1
    # A record's page, through the sleeve's own labelled `Open`.
    move $CELL1_X $CELL1_SLEEVE_Y
    click $CELL1_OPEN_X $CELL1_OPEN_Y
    move $PARK_X $PARK_Y
    shot "06-record-page-two-column-${SUF}"
  else
    click $LANE_X $LIB_Y
    move $PARK_X $PARK_Y
    sleep 1
    move $CELL1_X $CELL1_SLEEVE_Y
    click $CELL1_OPEN_X $CELL1_OPEN_Y
    move $PARK_X $PARK_Y
    shot "07-record-page-short-window-${SUF}"
    # Three wheel clicks **over the aside's tail** — the strip under the cover,
    # which is the half this pass gave a scroller of its own. The cover itself
    # does not scroll, so a wheel over the artwork does nothing, which is the
    # point.
    wheel 380 560 3
    shot "08-record-page-aside-scrolled-${SUF}"
  fi

  kill "$APID" 2>/dev/null; wait "$APID" 2>/dev/null
  kill "$XPID" 2>/dev/null; wait "$XPID" 2>/dev/null
  echo "--- isolation receipt ($SUF) ---"
  grep -m1 mpris "$S/app-$SUF.log" || echo "NO MPRIS LINE — the isolation is unproven"
}

shoot 1280 860 1280x860
shoot 1280 760 1280x760
