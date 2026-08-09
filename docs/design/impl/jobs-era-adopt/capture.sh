#!/usr/bin/env bash
# Render doc 11 §5's adopt tier (P1, P2, P3, P4, P6) headless, on a private
# Xvfb, with all six XDG redirections from docs/DEVELOPMENT.md. Nothing
# touches the owner's session; the run's `[mpris] no session bus` line is
# the receipt that it did not, and this script prints it. Processes are
# stopped by pid, never by name.
#
# What it shows against the real binary, at 1280×860:
#
#   1. **P1** — the first-run screen with `Browse…` beside the typed field,
#      the human placeholder, and the CLI-free footnote; the off-thread
#      check's refusal in place.
#   2. **P3** — the Album place's header carrying `‹ Prev` / `Next ›`, and a
#      step along the wall's own order.
#   3. **P4** — "Esc returns to Library" in the same frame.
#   4. **P2** — the queue's transient `Undo` beside the summary after a
#      shift-click append, and the run restored after Ctrl+Z; the playlist
#      page's one-press `Delete` (no confirm below the acts row).
#   5. **P6** — the tile menu printing `Shift-click` beside `Queue album`, the
#      Shuffle/Pull tooltips, the queue's completed empty-state line, and
#      the Songs rule's "Enter plays the first match."
#
# Build the binary **inside the toolbox** (a host-built release binary links
# a newer glibc than the container has and dies before it draws):
#
#   toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
#     cargo build --release -p baz --features device-output
#   toolbox run -c baz-dev docs/design/composition/tools/mkfixture.sh /tmp/baz-jea-fix
#   toolbox run -c baz-dev docs/design/impl/jobs-era-adopt/capture.sh
set -uo pipefail

REPO=${REPO:-$(git rev-parse --show-toplevel)}
BIN=${BIN:-$REPO/target/tb/release/baz}
FIX=${FIX:-/tmp/baz-jea-fix}
OUT=${OUT:-$REPO/docs/design/impl/jobs-era-adopt}
DISP=${DISP:-:198}
S=/tmp/baz-jea-scratch

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

# A pre-seeded playlist, so the page's one-press Delete can be shown without
# a click-choreographed save: two real fixture files, one file the user owns.
seed_playlist() {
  mkdir -p "$S/data/baz/playlists"
  {
    echo "#EXTM3U"
    find "$FIX" -name '*.flac' | sort | head -8
  } > "$S/data/baz/playlists/Road Trip.m3u8"
}

Xvfb "$DISP" -screen 0 1280x860x24 -nolisten tcp &
XPID=$!
sleep 1

launch() { # extra args...
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
key()   { xdotool key "$@"; sleep 0.6; }
click() { xdotool mousemove "$1" "$2"; sleep 0.3; xdotool click 1; sleep 0.7; }
park()  { xdotool mousemove 1274 500; sleep 0.3; }

# ---- P1: first run ---------------------------------------------------------
launch
park; shot 01-first-run
# The typed door's refusal, decided off-thread and said in place. (The
# field takes a click first: the setup screen has no type-anywhere.)
click 600 370
xdotool type --delay 60 "/nowhere/particular"
key Return
sleep 1.0
park; shot 02-first-run-refusal
stop_app

# ---- The library, for everything after ------------------------------------
seed_playlist
launch "$FIX"
sleep 4   # let the launch scan land
park; shot 03-wall

# P6.4: the Songs rule teaches Enter, at the moment a query stands.
xdotool type --delay 80 "night"
sleep 0.9
park; shot 04-songs-rule-note
key Escape; key Escape

# P6.1: the tile menu prints the queueing gesture beside its verb.
xdotool mousemove 160 260; sleep 0.3; xdotool click 3; sleep 0.8
shot 05-tile-menu-accelerator
key Escape

# P3 + P4: the record's page — ‹ Prev / Next › in the header, "Esc returns
# to Library" across the strip.
click 160 260
sleep 0.8
park; shot 06-album-prev-next
# One step along the wall's own order.
NEXT_X=${NEXT_X:-252}
click "$NEXT_X" 32
sleep 0.8
park; shot 07-album-stepped
key Escape

# P2: an append arms the queue's Undo; Ctrl+Z takes it back.
key Return          # Enter plays the top of the wall — the run to edit
sleep 1.2
xdotool keydown shift; sleep 0.2
click 420 260       # shift-click a second record: queued, not opened
xdotool keyup shift; sleep 0.5
key ctrl+u
sleep 0.8
park; shot 08-queue-undo-armed
key ctrl+z
sleep 0.8
park; shot 09-queue-undone
key Escape

# P2: the playlist page's Delete is one press — no armed sentence, no
# second button below the acts row.
key ctrl+p
sleep 0.8
PANEL_ROW_Y=${PANEL_ROW_Y:-127}
click 1050 "$PANEL_ROW_Y"
sleep 0.9
park; shot 10-playlist-one-press-delete
# …and the press itself: one click on Delete, no confirm — the file is in
# the trash and the page leaves for the Library.
DELETE_X=${DELETE_X:-225}
DELETE_Y=${DELETE_Y:-485}
click "$DELETE_X" "$DELETE_Y"
sleep 0.9
park; shot 11-playlist-deleted-to-trash
ls "$S/data/Trash/files" >> "$S/app.log" 2>&1 || true
key Escape

# P6.3: the queue's empty state states the refusal with its answers.
# (A fresh launch: nothing has played, the queue is empty.)
stop_app
launch "$FIX"
sleep 3
key ctrl+u
sleep 0.8
park; shot 12-queue-empty-taught

# P6.2: the draw words teach on hover.
key Escape
SHUFFLE_X=${SHUFFLE_X:-800}
xdotool mousemove "$SHUFFLE_X" 32; sleep 1.4
shot 13-shuffle-tooltip
PULL_X=${PULL_X:-844}
xdotool mousemove "$PULL_X" 32; sleep 1.4
shot 14-pull-tooltip

stop_app
kill "$XPID" 2>/dev/null; wait "$XPID" 2>/dev/null

echo "--- the isolation receipt ---"
grep -E "^\[mpris\]|^\[startup\] room" "$S/app.log" || echo "(no mpris line — look at $S/app.log)"
echo "done — $OUT"
