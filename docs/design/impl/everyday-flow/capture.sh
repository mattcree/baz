#!/usr/bin/env bash
# Render the four frames `docs/design/13-everyday-flow.md` argues from —
# headless, on a private Xvfb, with all six XDG redirections from
# docs/DEVELOPMENT.md. Nothing touches the owner's session; the run's
# `[mpris] no session bus` line is the receipt that it did not, and this
# script prints it.
#
# What it captures against the real binary at 1280×860:
#
#   1. The wall at rest — the surface problem 1 is about, and the baseline
#      any hover-group proposal is measured against.
#   2. The tile's context menu, at the pointer: `Open · Play album ·
#      Queue album (Shift-click) · Add to playlist…` — the four verbs
#      §2.4 asks whether a hover group would merely re-draw.
#   3. **The picker, and the owner's complaint in one frame**: press
#      `Add to playlist…` and the panel arrives with `Add "…" — pick a
#      destination` set at `SIZE_META` in `paper_dim`, a line quieter than
#      the panel's own `Playlists` heading and level with `Esc closes`.
#      That line is the "very minor tip at the very top" of the brief.
#   4. The record's page, whose header carries `‹ Prev` / `Next ›`
#      (ADR-0022's comparison debt, paid by doc 11 P3) — the shipped depth
#      affordance §3 diagnoses against.
#
# Build the binary **inside the toolbox** (a host-built release binary links
# a newer glibc than the container has and dies before it draws):
#
#   toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
#     cargo build --release -p baz --features device-output
#   toolbox run -c baz-dev docs/design/composition/tools/mkfixture.sh /tmp/baz-ef-fix
#   toolbox run -c baz-dev env FIX=/tmp/baz-ef-fix \
#     docs/design/impl/everyday-flow/capture.sh
set -uo pipefail

REPO=${REPO:-$(git rev-parse --show-toplevel)}
BIN=${BIN:-$REPO/target/tb/release/baz}
FIX=${FIX:-/tmp/baz-ef-fix}
OUT=${OUT:-$REPO/docs/design/impl/everyday-flow}
DISP=${DISP:-:197}
S=/tmp/baz-ef-scratch

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

# Two kept lists, so the picker has a shelf to be a picker over.
mkdir -p "$S/data/baz/playlists"
{ echo "#EXTM3U"; find "$FIX" -name "*.flac" | sort | head -3; } \
  > "$S/data/baz/playlists/Road Trip.m3u8"
{ echo "#EXTM3U"; find "$FIX" -name "*.flac" | sort | tail -5; } \
  > "$S/data/baz/playlists/Late Nights.m3u8"

W=1280; H=860
Xvfb "$DISP" -screen 0 ${W}x${H}x24 -nolisten tcp &
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
xdotool windowsize "$WID" $W $H
xdotool windowfocus --sync "$WID"
sleep 4

shot()   { sleep 0.9; magick import -window root "$OUT/$1.png"; echo "  shot $1"; }
key()    { xdotool key "$@"; sleep 0.5; }
click()  { xdotool mousemove "$1" "$2"; sleep 0.3; xdotool click 1; sleep 0.6; }
rclick() { xdotool mousemove "$1" "$2"; sleep 0.4; xdotool click 3; sleep 0.6; }

TILE_X=444; TILE_Y=250     # the second tile's sleeve on the wall

xdotool mousemove $TILE_X $TILE_Y; sleep 0.6
shot 01-wall-tile-hovered-1280x860
rclick $TILE_X $TILE_Y
shot 02-tile-menu-1280x860
# `Add to playlist…` is the menu's fourth item: the card's top-left is the
# pointer, each item TRANSPORT_HIT 32 tall over GAP_XS 4 of card air.
click $((TILE_X + 60)) $((TILE_Y + 4 + 3 * 32 + 16))
shot 03-picker-hint-1280x860
key Escape
key Escape
click $TILE_X $TILE_Y
shot 04-record-page-header-1280x860

kill "$APID" 2>/dev/null; wait "$APID" 2>/dev/null
kill "$XPID" 2>/dev/null; wait "$XPID" 2>/dev/null

echo
echo "--- the isolation receipt ---"
grep -m1 mpris "$S/app.log" || echo "NO MPRIS LINE — the isolation is unproven"
