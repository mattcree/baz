#!/usr/bin/env bash
# Render **the owner's Now playing batch of 2026-08-10** — the `Run` word gone,
# the place showing whatever the bar names, the empty state's inset, the three
# kinds of list, the continuous field and the run that follows the music —
# headless, on a private Xvfb, with all six XDG redirections from
# docs/DEVELOPMENT.md §"Headless UI verification".
#
# Nothing touches the owner's session and nothing is audible: the scratch HOME
# routes ALSA's default PCM to null and the fixture's samples are all zero. The
# run's `[mpris] no session bus` line is the receipt that the isolation held,
# and this script prints it. BAZ_DEVICE_TESTS is not set and nothing here sets
# it.
#
# Build the binary first:
#   toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
#     cargo build --release -p baz --features device-output
#
# The fixture is `docs/design/composition/tools/mkfixture.sh`'s, with its covers
# re-drawn at 1400 px by `impl/artwork-at-size/capture.sh` — the same records
# wearing the same colours as every other frame in this folder's siblings.
#
#   toolbox run -c baz-dev docs/design/impl/now-playing-shows-the-run/capture.sh
set -uo pipefail

REPO=${REPO:-$(git rev-parse --show-toplevel)}
BIN=${BIN:-$REPO/target/tb/release/baz}
FIX=${FIX:-/tmp/baz-hero-fix}
OUT=${OUT:-$REPO/docs/design/impl/now-playing-shows-the-run}
DISP=${DISP:-:198}
S=/tmp/baz-run-scratch

mkdir -p "$OUT"

scratch() { # extra config lines
  rm -rf "$S"; mkdir -p "$S"/{home,data,config,cache,run}; chmod 700 "$S/run"
  cat > "$S/home/.asoundrc" <<'EOF'
pcm.!default { type null }
ctl.!default { type null }
EOF
  mkdir -p "$S/config/baz"
  # **`run_column` is deliberately written as `false`** in the stale-config
  # pass: it is the key a listener upgrading from yesterday's build still has,
  # and the frame proves it costs them nothing — the run column stands anyway.
  { printf 'music_dirs = [\n    "%s",\n]\ngroup_key = "artist"\nsidebar_open = true\n' "$FIX"
    [[ -n ${1:-} ]] && printf '%s\n' "$1"
  } > "$S/config/baz/config.toml"
}

launch() { # w h
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
  if [[ -z $WID ]]; then echo "NO WINDOW"; tail -30 "$S/app.log"; kill "$APID" "$XPID" 2>/dev/null; exit 1; fi
  export DISPLAY=$DISP
  xdotool windowmove "$WID" 0 0
  xdotool windowsize "$WID" "$1" "$2"
  xdotool windowfocus --sync "$WID"
  for _ in $(seq 1 60); do
    grep -q "^\[scan\] done:" "$S/app.log" && break
    sleep 0.5
  done
  sleep 4
}

shot() { sleep 0.9; magick import -window root "$OUT/$1.png"; echo "  shot $1"; }
key()  { xdotool key "$@"; sleep 0.5; }
park() { xdotool mousemove $((W - 6)) $((H - 200)); }

start_x11() { # w h
  W=$1; H=$2
  Xvfb "$DISP" -screen 0 ${W}x${H}x24 -nolisten tcp & XPID=$!
  sleep 1
}
stop_x11() {
  kill "$APID" 2>/dev/null; wait "$APID" 2>/dev/null
  kill "$XPID" 2>/dev/null; wait "$XPID" 2>/dev/null
}

# --- 01/02/03 · a record's run: sounding, paused, and then assembled --------
# **The play gesture is a query and Enter**, not a double-click on a sleeve.
# That matters for what these frames are evidence of: a double-click lands on
# the wall's hover options and *appends*, which makes the run an edited one —
# `RunSource::Fixed` + an edit is `RunOrigin::Assembled`, and the save word
# correctly appears. Enter on a query is `play_first_match` → `play_album` →
# one `SetQueue`, which is a **pristine fixed run** and is the exact case the
# owner reported: *"I still see save as playlist on the queue when playing a
# CD"*.
#
# So 01 and 02 must show **no save word**, and 03 — the same run after one
# append — must show one. The pair is the rule.
record_run() { # prefix size
  xdotool type --delay 60 "Ochre"; sleep 1.2
  key Return
  sleep 2.0
  # Esc leaves the well (and clears the query with it) — the chord layer is
  # not bound while the field has the caret, which is `keys::Focus`'s whole
  # job. The run is unaffected: a query is not a list.
  key Escape
  sleep 0.6
  key ctrl+u
  sleep 2.0
  park; shot "${1}-01-sounding-${2}"
  key space
  sleep 1.0
  park; shot "${1}-02-paused-${2}"
}

# A shift-click on a sleeve appends the record to the run (doc 09 §13 step 7),
# which is the listener assembling a list out of what was a record's own.
# **The wall's hover `Queue` option**, which is the listener assembling a list
# out of records (doc 09 §13 step 7's gesture, reached by its named control).
# The query is cleared first or the wall holds one tile and there is nothing
# to add.
append_a_record() {
  key Escape          # out of the place
  key Escape          # …and out of the query, so the whole wall is back
  sleep 0.8
  xdotool mousemove 340 250; sleep 0.6
  xdotool click --repeat 2 --delay 120 1; sleep 1.5
  key ctrl+u
  sleep 1.5
}

for size in "1280 860" "1920 1080"; do
  set -- $size
  echo "=== ${1}x${2} · a record's run ==="
  start_x11 "$1" "$2"; scratch ""; launch "$1" "$2"
  record_run "$(printf '%02d' $((${1} / 100)))" "${1}x${2}"
  append_a_record
  park; shot "$(printf '%02d' $((${1} / 100)))-03-assembled-${1}x${2}"
  stop_x11
done

# --- 10 · the genuinely-empty place ----------------------------------------
# No run at all and nothing sounding, which since this branch is the *only*
# state that reaches the empty text. The frame is for its **inset**: the block
# stands on the place's own gutter and at the rows' own measure, where before
# it was flush against the window (*"hugging the left with no padding"*).
echo "=== 1280x860 · the empty place ==="
start_x11 1280 860; scratch ""; launch 1280 860
key ctrl+u
sleep 1.5
park; shot "10-empty-1280x860"
stop_x11

# --- 20 · a stale config, and the run column standing anyway ---------------
# `run_column = false` is what a listener who turned the density off still has
# in `config.toml`. The key is read without harm, is not written back, and
# cannot stand the column down.
echo "=== 1280x860 · a stale run_column = false ==="
start_x11 1280 860; scratch "run_column = false"; launch 1280 860
record_run "20-stale-config" "1280x860"
echo "  config after the run:"; cat "$S/config/baz/config.toml" | sed 's/^/    /'
stop_x11

# --- 30 · a long run: the scrollbar, and the follow ------------------------
# Several records appended into one run — *"playlists can be long"* — so the
# column overflows and the thumb is genuinely small. Three frames: the head of
# the run, the same column after the music has moved on, and the whole surface
# at 1280 where the run takes `RUN_MEASURE` inside a narrower body.
echo "=== 1920x1080 · a long run ==="
start_x11 1920 1080; scratch ""; launch 1920 1080
xdotool type --delay 60 "Ochre"; sleep 1.2
key Return
sleep 2.0
key Escape         # out of the well
key Escape         # …and out of the query
sleep 0.8
# Several records appended through the wall's hover `Queue`, left to right
# along the first shelf.
for x in 340 540 740 940 1140 1340; do
  xdotool mousemove $x 250; sleep 0.5
  xdotool click --repeat 2 --delay 120 1; sleep 1.0
done
key ctrl+u
sleep 2.0
park; shot "30-long-run-head-1920x1080"
# **The follow, driven by the engine.** `Next` is a confirmed track change, so
# the column moves exactly when `TrackStarted` lands — and only when the row it
# is moving to was not already on screen.
# Ctrl+Right is the bar's Next button, exactly (`keys.rs:390`).
for _ in $(seq 1 30); do key ctrl+Right; done
sleep 2.0
park; shot "31-long-run-followed-1920x1080"
stop_x11

echo "--- isolation receipt ---"
grep -m2 -e "no session bus" -e "^\[mpris\]" "$S/app.log" || echo "  (no mpris line — check the log)"
