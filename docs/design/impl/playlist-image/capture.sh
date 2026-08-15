#!/usr/bin/env bash
# **Item 52 — a playlist wears a picture the listener chose.**
#
# The owner, 2026-08-15: *"lets allow setting an image/removing the image for a
# playlist."*
#
# A playlist's sleeve is otherwise a collage of quotations from the records it
# holds (ADR-0024 §A1). This shows one list wearing an authored picture instead
# — in the **wall's tile**, in the **returns lane**, and on the list's **own
# page**, where the acts read `Change image…` and `Remove image` — beside a
# second list still wearing its collage, so the two states are in one frame.
#
# **What this can and cannot photograph.** Setting a picture opens the
# platform's file dialog, which on Linux is a D-Bus portal a headless run has
# no claim on: `rfd` returns `None` at once and the act is a dismissal. So the
# run seeds the sibling file the dialog would have produced — `<name>.png`
# beside `<name>.m3u8`, which is exactly what `Folder::set_image` writes — and
# photographs everything downstream of it, which is the half that could
# quietly not work. The copy itself is covered by
# `baz_core::playlist::tests::a_playlist_wears_one_authored_sleeve_and_keeps_it_through_a_rename`
# and `playlists::tests::a_chosen_picture_reaches_the_row_and_the_open_page`.
#
#   toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
#     cargo build --release -p baz --features device-output
#   toolbox run -c baz-dev env FIX=/tmp/baz-review-fix \
#     docs/design/impl/playlist-image/capture.sh
set -uo pipefail

REPO=${REPO:-$(git rev-parse --show-toplevel)}
BIN=${BIN:-$REPO/target/tb/release/baz}
FIX=${FIX:-/tmp/baz-review-fix}
OUT=${OUT:-$REPO/docs/design/impl/playlist-image}
DISP=${DISP:-:205}
S=/tmp/baz-image-scratch
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

mkdir -p "$S/data/baz/playlists"
{ echo "#EXTM3U"; find "$FIX" -name '*.flac' | sort | head -8; } \
  > "$S/data/baz/playlists/Road Trip.m3u8"
{ echo "#EXTM3U"; find "$FIX" -name '*.flac' | sort | tail -6; } \
  > "$S/data/baz/playlists/Sunday Morning.m3u8"

# The picture the dialog would have copied here. Deliberately **not** square
# and not baz-coloured: a listener's photograph is neither, and the sleeve has
# to crop it to the square hole every surface draws a sleeve in rather than
# letterbox the room's ground into the middle of a shelf.
magick -size 900x600 gradient:'#c8552e-#2b3a55' \
  -fill white -pointsize 96 -gravity center -annotate 0 'ROAD\nTRIP' \
  "$S/data/baz/playlists/Road Trip.png"

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
park()  { xdotool mousemove 1100 700; sleep 0.5; xdotool mousemove 1102 702; sleep 0.9; }

LANE_X=32
PLAYLISTS_Y=185
# The wall's second run: `Road Trip` under `R`, `Sunday Morning` under `S`.
# Read off the frame, like every other coordinate in this repo's captures.
# `Road Trip` is the first row of the lane's lists half, under the four
# destinations: SIDEBAR_DEST_H 48 on a SIDEBAR_ROW_GAP 4 seam, then the rule.
LANE_LIST_X=115 LANE_LIST_Y=310

click $LANE_X $PLAYLISTS_Y
park
# Both runs in one frame: `Road Trip` under `R` wearing its picture and
# `Sunday Morning` under `S` wearing its collage, which is the comparison.
xdotool mousemove 640 500
for _ in 1 2; do xdotool click 5; sleep 0.25; done
park
shot "01-wall-one-picture-one-collage-${W}x${H}"

# The lane's own row for the list, which is one press and does not depend on
# where the wall happens to be scrolled.
click $LANE_LIST_X $LANE_LIST_Y
park
shot "02-page-with-its-acts-${W}x${H}"

kill "$APID" 2>/dev/null; wait "$APID" 2>/dev/null
kill "$XPID" 2>/dev/null; wait "$XPID" 2>/dev/null
echo "--- isolation receipt ---"
grep -m1 mpris "$S/app.log" || echo "NO MPRIS LINE — the isolation is unproven"
