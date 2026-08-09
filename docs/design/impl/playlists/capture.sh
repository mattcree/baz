#!/usr/bin/env bash
# Render the playlist surfaces (ADR-0024 §4–§6) headless, on a private Xvfb,
# with all six XDG redirections from docs/DEVELOPMENT.md. Nothing touches the
# owner's session; the run's `[mpris] no session bus` line is the receipt that
# it did not, and this script prints it.
#
# What it proves against the real binary, at 1280×860 and 1920×1080:
#
#   1. **The wall does not reflow when the panel opens.** The before/after
#      pair is diffed with the panel's own region blanked on both frames —
#      the remainder must be pixel-identical (magick compare AE == 0), which
#      is "no press re-hangs the collection" as an assertion rather than a
#      promise. (Run the app with the wgpu renderer, never tiny-skia: partial
#      repaints are not run-to-run deterministic.)
#   2. The panel: New playlist, the rows with their counts, the armed state's
#      surface step, and the collecting `+` on every wall label.
#   3. The playlist page: hero name, counts (`4 of 5 · 1 missing` on the
#      seeded list), Play/Queue/Rename/Delete, record-group headers, the
#      dimmed missing row with its path.
#   4. The queue place's `Save as playlist`.
#
# Build the binary **inside the toolbox** (a host-built release binary links
# a newer glibc than the container has and dies before it draws):
#
#   toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
#     cargo build --release -p baz --features device-output
#   toolbox run -c baz-dev docs/design/composition/tools/mkfixture.sh /tmp/baz-playlists-fix
#   toolbox run -c baz-dev docs/design/impl/playlists/capture.sh
set -uo pipefail

REPO=${REPO:-$(git rev-parse --show-toplevel)}
BIN=${BIN:-$REPO/target/tb/release/baz}
FIX=${FIX:-/tmp/baz-playlists-fix}
OUT=${OUT:-$REPO/docs/design/impl/playlists}
DISP=${DISP:-:193}
S=/tmp/baz-playlists-scratch

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

# Seed the playlists folder the way a migrating listener would: files. One
# list spanning four records (the 2 × 2 collage, ADR-0024 §A1), one carrying
# a single record and a missing entry (the full-bleed single, and the page's
# `N of M · K missing` arithmetic), and one empty (the rest tile).
PL="$S/data/baz/playlists"
mkdir -p "$PL"
mapfile -t AMBER < <(find "$FIX" -path '*Amber Room*' -name '*.flac' | sort)
mapfile -t ORBITS < <(find "$FIX" -path '*Orbits*' -name '*.flac' | sort | head -2)
mapfile -t BASALT < <(find "$FIX" -path '*Basalt*' -name '*.flac' | sort | head -1)
mapfile -t WERK   < <(find "$FIX" -path '*Werkbund*' -name '*.flac' | sort | head -2)
{
  echo "#EXTM3U"
  echo "# made with baz on 2026-08-09"
  printf '%s\n' "${AMBER[@]:0:2}"
  printf '%s\n' "${ORBITS[@]}"
  printf '%s\n' "${BASALT[@]}"
  printf '%s\n' "${WERK[@]}"
} > "$PL/Late Shift.m3u8"
{
  echo "#EXTM3U"
  printf '%s\n' "${AMBER[@]:0:4}"
  echo "/gone/nowhere/dust.flac"
} > "$PL/Worn Tape.m3u8"
printf '#EXTM3U\n# made with baz on 2026-08-09\n' > "$PL/Sketches.m3u8"

Xvfb "$DISP" -screen 0 1280x860x24 -nolisten tcp &
XPID=$!
sleep 1

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

shot() { sleep 0.9; magick import -window root "$OUT/$1.png"; echo "  shot $1"; }
klick(){ xdotool mousemove "$1" "$2"; sleep 0.3; xdotool click 1; sleep 0.9; }
key()  { xdotool key "$@"; sleep 0.6; }
park() { xdotool mousemove $((W - 6)) 200; }

# ---- 1280 × 860 -----------------------------------------------------------
W=1280; H=860
launch $W $H

# 1. The wall at rest — the "before" of the no-reflow diff.
park; shot 01-wall-before
# 2. Ctrl+P: the panel floats over the wall's right edge. The "after".
key ctrl+p
park; shot 02-wall-panel-open
# 3. Arm the first playlist (the row's receive target, right edge of the
#    row): the surface step and hairline on the row, the quiet `+` on every
#    wall label. The rows now open with their sleeves — the 2 × 2 collage,
#    the rest tile, the full-bleed single, top to bottom.
klick $((W - 36)) 116
park; shot 03-panel-armed
# 4. A record's page beside the open panel: `Add to playlist` under the play
#    control. (Disarm first, so the tile press below navigates rather than
#    adds.)
klick $((W - 36)) 116
klick 160 250
park; shot 04-album-page-with-panel
# 5. The playlist page, by the panel row's name: the collage in the hero
#    position, the record page's own two-column arrangement.
klick $((W - 250)) 116
park; shot 05-playlist-page
# 6. The single-record list with a missing entry, straight from the panel
#    (its rows are doors from any place it stands over): the full-bleed
#    sleeve, the dimmed row with its path, the counted arithmetic.
klick $((W - 250)) 220
park; shot 06-playlist-page-missing
# 7. Play from its first row: the playable subset queues (4 of 5), the lamp
#    lands on the page's row.
klick 450 250
sleep 2
park; shot 07-playlist-playing
# 8. The queue place: exactly the playable subset a playlist's Play sent,
#    grouped under its records' names, and `Save as playlist` by the summary.
key Escape
key ctrl+u
park; shot 08-queue-save-control
# 9. The save field, open.
klick 1024 106
park; shot 09-queue-save-field
key Escape

kill "$APID" 2>/dev/null; wait "$APID" 2>/dev/null
kill "$XPID" 2>/dev/null; wait "$XPID" 2>/dev/null

# ---- 1920 × 1080 ----------------------------------------------------------
W=1920; H=1080
Xvfb "$DISP" -screen 0 ${W}x${H}x24 -nolisten tcp &
XPID=$!
sleep 1
launch $W $H
park; shot 10-wall-before-1920
key ctrl+p
park; shot 11-wall-panel-open-1920
# 12. The page at 1920: the hero collage beside a list at its measure, both
#     clear of the panel.
klick $((W - 250)) 116
park; shot 12-playlist-page-1920
key Escape
kill "$APID" 2>/dev/null; wait "$APID" 2>/dev/null
kill "$XPID" 2>/dev/null; wait "$XPID" 2>/dev/null

# ---- the no-reflow assertion ---------------------------------------------
# Blank the panel's own region (its 340 px + the 1 px seam, full height above
# the bar) on both frames of each pair; what remains must be identical.
assert_no_reflow() { # before after W label
  local before=$1 after=$2 width=$3 label=$4
  local lane=341
  local ae
  magick "$OUT/$before.png" -fill black \
    -draw "rectangle $((width - lane)),0 $width,9999" "$S/masked-before.png"
  magick "$OUT/$after.png" -fill black \
    -draw "rectangle $((width - lane)),0 $width,9999" "$S/masked-after.png"
  ae=$(magick compare -metric AE "$S/masked-before.png" "$S/masked-after.png" null: 2>&1)
  ae=${ae%% *} # AE prints `N (n)`; the absolute count is the claim
  echo "no-reflow @$label: AE=$ae outside the panel's region"
  [[ "$ae" == "0" ]] || { echo "REFLOW DETECTED at $label"; exit 1; }
}
assert_no_reflow 01-wall-before 02-wall-panel-open 1280 "1280x860"
assert_no_reflow 10-wall-before-1920 11-wall-panel-open-1920 1920 "1920x1080"

echo "--- the isolation receipt ---"
grep -E "^\[mpris\]|^\[startup\] room|^\[playlists\]" "$S/app.log" || echo "(no mpris line — look at $S/app.log)"
echo "done — $OUT"
