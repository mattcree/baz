#!/usr/bin/env bash
# Render doc 09 §13 step 4 — the context menu, as a mirror layer (§5.2) —
# headless, on a private Xvfb, with all six XDG redirections from
# docs/DEVELOPMENT.md. Nothing touches the owner's session; the run's
# `[mpris] no session bus` line is the receipt that it did not, and this
# script prints it.
#
# What it shows against the real binary, at 1280×860 and 1920×1080:
#
#   1. **The four menus of §5.2's table**, each opened by a right press and
#      drawn at the pointer: an album tile (`Open · Play album · Queue album
#      · Add to playlist…`), a track row on the record's page and in the
#      Songs section (`Play · Queue · [Add to "{current}"] · Add to
#      playlist…`), a playlist page row (the same, spending the page's own
#      messages — and the row's new transfer `+` in the queue row's outer
#      slot), a queue row (`Play · Add to "{current}" · Add to playlist… ·
#      Remove`), and the bar's now-playing block (`Go to record · Add to
#      "{current}" · Add to playlist…`).
#   2. **`Add to "{current}"` appears exactly while provenance stands**: the
#      first menus open before any playlist plays and carry no such item;
#      after `Road Trip` plays, every menu names it.
#   3. **The flip at the window's edges**: the bar's menu opens *upward*
#      (the pointer is at the bottom edge), and a Songs row clicked near the
#      right edge opens its card to the pointer's *left* — the card never
#      leaves the window.
#   4. **S4, two gestures from anywhere**: right-click the bar, press
#      `Add to "Road Trip"` — then the playlist's page shows the grown file
#      while the bar's continuation line shows the run untouched.
#   5. **No reflow**: the wall at rest and the wall with a menu open differ
#      by nothing outside the card's own region (`magick compare` AE=0 with
#      the card masked), printed at the end.
#
# Build the binary **inside the toolbox** (a host-built release binary links
# a newer glibc than the container has and dies before it draws):
#
#   toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
#     cargo build --release -p baz --features device-output
#   toolbox run -c baz-dev docs/design/composition/tools/mkfixture.sh /tmp/baz-cm-fix
#   toolbox run -c baz-dev env FIX=/tmp/baz-cm-fix docs/design/impl/context-menus/capture.sh
set -uo pipefail

REPO=${REPO:-$(git rev-parse --show-toplevel)}
BIN=${BIN:-$REPO/target/tb/release/baz}
FIX=${FIX:-/tmp/baz-cm-fix}
OUT=${OUT:-$REPO/docs/design/impl/context-menus}
DISP=${DISP:-:198}
S=/tmp/baz-cm-scratch

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
EOF

# The current playlist every `Add to "{current}"` item names: a real file in
# the scratch data dir, holding three fixture tracks by absolute path.
mkdir -p "$S/data/baz/playlists"
{
  echo "#EXTM3U"
  find "$FIX" -name "*.flac" | sort | head -3
} > "$S/data/baz/playlists/Road Trip.m3u8"

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
  sleep 4   # let the launch scan land
}

shot()   { sleep 0.9; magick import -window root "$OUT/$1.png"; echo "  shot $1"; }
key()    { xdotool key "$@"; sleep 0.5; }
click()  { xdotool mousemove "$1" "$2"; sleep 0.3; xdotool click 1; sleep 0.6; }
rclick() { xdotool mousemove "$1" "$2"; sleep 0.4; xdotool click 3; sleep 0.6; }

# ---- 1280 × 860 -----------------------------------------------------------
W=1280; H=860
Xvfb "$DISP" -screen 0 ${W}x${H}x24 -nolisten tcp &
XPID=$!
sleep 1
launch $W $H

# Coordinates at 1280 (measured off the rendered frame):
TILE2_X=444; TILE2_Y=250          # Violet Ledger's sleeve on the wall
TRACK_X=600; TRACK_Y=268          # track row 2 on the record's page
PANEL_ROW_X=1060; PANEL_ROW_Y=125 # the panel's Road Trip row
PLAY_X=200; PLAY_Y=437            # the playlist page's Play
PLROW_X=600; PLROW_Y=282          # playlist page row 2
BAR_X=90; BAR_Y=810               # the bar's now-playing block
BAR_ITEM_X=150; BAR_ITEM_Y=758    # its menu's `Add to "Road Trip"` (flipped up)
QROW_X=600; QROW_Y=236            # queue place row 2
SONG_X=1120; SONG_Y=135           # Songs row 1, near the right edge

# 1. The wall at rest, the pointer already resting on the tile — the
#    baseline the no-reflow diff is measured against.
xdotool mousemove $TILE2_X $TILE2_Y; sleep 0.6
shot 01-wall-before-1280x860
# 2. Right press the tile: §5.2's tile menu, at the pointer. No provenance
#    stands yet, so no `Add to "{current}"` anywhere — absent, not disabled.
rclick $TILE2_X $TILE2_Y
shot 02-tile-menu-1280x860
# 3. Esc peels the menu (one layer, one press); the tile press opens the
#    record; a track row's right press: the track menu.
key Escape
click $TILE2_X $TILE2_Y
rclick $TRACK_X $TRACK_Y
shot 03-track-menu-1280x860
# Provenance: play "Road Trip" from its page (panel → row → Play), pause so
# the stills hold, close the panel again.
key Escape                        # the menu
key ctrl+p                        # the panel
click $PANEL_ROW_X $PANEL_ROW_Y   # its page
click $PLAY_X $PLAY_Y             # provenance stands
sleep 0.8
key space                         # hold still
key Escape                        # the panel down
# 4. A playlist page row's menu — the page's own messages, the current list
#    named — with the row hovered so its slots (the new transfer `+` at the
#    outer edge) are in frame.
xdotool mousemove $PLROW_X $PLROW_Y; sleep 0.6
rclick $PLROW_X $PLROW_Y
shot 04-playlist-row-menu-1280x860
# 5. S4's first gesture, from anywhere: back on the wall, right press the
#    bar's now-playing block. The card opens *upward* — the bottom-edge
#    flip — and carries `Add to "Road Trip"`; the pointer rests on it.
key Escape                        # the menu
key Escape                        # the page
rclick $BAR_X $BAR_Y
xdotool mousemove $BAR_ITEM_X $BAR_ITEM_Y; sleep 0.4
shot 05-bar-menu-s4-1280x860
# 6. The queue place: a row's menu, `Remove` included, the summary leading
#    with the provenance the menu names.
key Escape                        # the menu
key ctrl+u                        # the queue place
rclick $QROW_X $QROW_Y
shot 06-queue-row-menu-1280x860
# 7. The right-edge flip: a query brings the Songs section; its first row
#    right-pressed near the window's right edge opens the card to the
#    pointer's *left*, whole and on screen.
key Escape                        # the menu
key Escape                        # the place
key q                             # type-anywhere: the query "q"
sleep 0.6
rclick $SONG_X $SONG_Y
shot 07-songs-row-edge-flip-1280x860
# 8. S4's second gesture, and the proof of both halves: press the item, then
#    open the file's page — it has grown by the sounding track, while the
#    bar's continuation line still counts the run it always did.
key Escape                        # the menu
key Escape                        # the query
rclick $BAR_X $BAR_Y
click $BAR_ITEM_X $BAR_ITEM_Y     # Add to "Road Trip" — the file, not the run
key ctrl+p
click $PANEL_ROW_X $PANEL_ROW_Y
shot 08-road-trip-after-s4-1280x860

kill "$APID" 2>/dev/null; wait "$APID" 2>/dev/null
kill "$XPID" 2>/dev/null; wait "$XPID" 2>/dev/null

# ---- 1920 × 1080 ----------------------------------------------------------
W=1920; H=1080
Xvfb "$DISP" -screen 0 ${W}x${H}x24 -nolisten tcp &
XPID=$!
sleep 1
launch $W $H

TILE2_X=462; TILE2_Y=255
PANEL_ROW_X=1700; PANEL_ROW_Y=125
PLAY_X=502; PLAY_Y=437
BAR_X=90; BAR_Y=1030
BAR_ITEM_X=150; BAR_ITEM_Y=978

# Provenance first, so both 1920 stills carry the S4 item.
key ctrl+p
click $PANEL_ROW_X $PANEL_ROW_Y
click $PLAY_X $PLAY_Y
sleep 0.8
key space
key Escape                        # the panel
key Escape                        # the page
# 9. The tile menu at 1920, `Add to "Road Trip"` included.
rclick $TILE2_X $TILE2_Y
shot 09-tile-menu-1920x1080
# 10. The bar's menu at 1920: the bottom-edge flip, the S4 item under the
#     pointer.
key Escape
rclick $BAR_X $BAR_Y
xdotool mousemove $BAR_ITEM_X $BAR_ITEM_Y; sleep 0.4
shot 10-bar-menu-s4-1920x1080

kill "$APID" 2>/dev/null; wait "$APID" 2>/dev/null
kill "$XPID" 2>/dev/null; wait "$XPID" 2>/dev/null

# ---- the no-reflow proof --------------------------------------------------
# The menu is a layer, never a column: outside the card's own region the
# wall with a menu open is the wall at rest, pixel for pixel. The card in
# 02 sits at the pointer (444,250), 232 px wide, 4 items tall; mask a
# rectangle just larger than it in both stills and require AE = 0.
mask() { magick "$1" -fill black -draw "rectangle 440,246 680,392" "$2"; }
mask "$OUT/01-wall-before-1280x860.png" "$S/before-masked.png"
mask "$OUT/02-tile-menu-1280x860.png"   "$S/after-masked.png"
AE=$(magick compare -metric AE "$S/before-masked.png" "$S/after-masked.png" null: 2>&1)
echo "--- no-reflow: AE outside the card's region = $AE (must be 0) ---"

echo "--- the isolation receipt ---"
grep -E "^\[mpris\]|^\[startup\] room|^\[playlists\]" "$S/app.log" || echo "(no mpris line — look at $S/app.log)"
echo "done — $OUT"
