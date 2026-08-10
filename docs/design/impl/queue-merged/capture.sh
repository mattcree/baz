#!/usr/bin/env bash
# Render **the merged surface** — `Place::NowPlaying` with the run standing
# beside the record and with it stood down — at the two sizes doc 12 §5.5a
# measures, against the real binary, headless, on a private Xvfb, with all six
# XDG redirections from docs/DEVELOPMENT.md §"Headless UI verification".
#
# Nothing touches the owner's session and nothing is audible: the scratch
# HOME routes ALSA's default PCM to null and the fixture's samples are all
# zero. The run's `[mpris] no session bus` line is the receipt that the
# isolation held, and this script prints it.
#
# These are frames of **what shipped in M1 and M2** — the pair
# `docs/design/impl/queue-in-now-playing/` took of the two places this merge
# replaced is the before, and this is the after. Read them together: 01a there
# is the queue place, 01b there is the unmerged now-playing place, and 01a here
# is the one surface that took over from both.
#
#   toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
#     cargo build --release -p baz --features device-output
#   toolbox run -c baz-dev docs/design/composition/tools/mkfixture.sh /tmp/baz-qm-fix
#   toolbox run -c baz-dev docs/design/impl/queue-merged/capture.sh
set -uo pipefail

REPO=${REPO:-$(git rev-parse --show-toplevel)}
BIN=${BIN:-$REPO/target/tb/release/baz}
FIX=${FIX:-/tmp/baz-qm-fix}
OUT=${OUT:-$REPO/docs/design/impl/queue-merged}
DISP=${DISP:-:198}
S=/tmp/baz-qm-scratch

mkdir -p "$OUT"
rm -rf "$S"; mkdir -p "$S"/{home,data,config,cache,run}; chmod 700 "$S/run"
cat > "$S/home/.asoundrc" <<'EOF'
pcm.!default { type null }
ctl.!default { type null }
EOF
mkdir -p "$S/config/baz"
# `run_column` is left unwritten on purpose: a fresh baz opens with the run
# standing, and the first frame is what a listener actually sees.
printf 'music_dirs = [\n    "%s",\n]\ngroup_key = "artist"\nsidebar_open = true\n' "$FIX" > "$S/config/baz/config.toml"

launch() {
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
  sleep 4
}
shot()  { sleep 0.9; magick import -window root "$OUT/$1.png"; echo "  shot $1"; }
key()   { xdotool key "$@"; sleep 0.5; }
click() { xdotool mousemove "$1" "$2"; sleep 0.3; xdotool click 1; sleep 0.5; }
park()  { xdotool mousemove $((W - 6)) $((H - 200)); }

# The `Run` word sits in the place's top-right, one HANG in from the body's
# right edge and one HANG down from its top. The body's right edge is the
# window's, so the word is at roughly (W - HANG - half its width, HANG + 16).
run_word() { xdotool mousemove $((W - 74)) 58; sleep 0.3; xdotool click 1; sleep 0.6; }

pair() { # prefix
  # A double-click on a sleeve plays that record whole (ADR-0023 §2's
  # needle-drop, at the album level), so the run below is an ordinary
  # album — the anonymous case the model is about. Space pauses immediately
  # so the cursor holds still for the stills; the null sink otherwise races
  # through silent tracks.
  xdotool mousemove 340 250; sleep 0.4
  xdotool click --repeat 2 --delay 120 1; sleep 1.2
  key space
  # Ctrl+U: the lane's `Now playing` row plus the `Run` word, made for you.
  key ctrl+u
  park; shot "${1}a-run-on-${W}x${H}"
  # The second density: the same place with the list stood down. `F11` is
  # deliberately not what does this (§3.4.2) — the word is.
  run_word
  park; shot "${1}b-run-off-${W}x${H}"
  # …and back on, with the lane collapsed: the case §5.5a says costs the
  # record nothing, and the one the kiosk rows assume.
  run_word
  key ctrl+b
  park; shot "${1}c-run-on-collapsed-${W}x${H}"
  # The sleeve at its corrected size: the run stood down and the lane
  # collapsed is the widest the record ever gets, and it is height-bound
  # there — which is exactly where A1's unspent 32 px was being lost.
  run_word
  park; shot "${1}d-sleeve-collapsed-${W}x${H}"
  key ctrl+b                          # leave the lane as we found it
  run_word                            # …and the run as we found it
}

for size in "1280 860" "1920 1080"; do
  set -- $size; W=$1; H=$2
  Xvfb "$DISP" -screen 0 ${W}x${H}x24 -nolisten tcp & XPID=$!
  sleep 1
  launch "$W" "$H"
  case $W in 1280) pair 01 ;; *) pair 02 ;; esac
  kill "$APID" 2>/dev/null; wait "$APID" 2>/dev/null
  kill "$XPID" 2>/dev/null; wait "$XPID" 2>/dev/null
done

echo "--- the isolation receipt ---"
grep -E "^\[mpris\]" "$S/app.log" || echo "(no mpris line — look at $S/app.log)"
echo "done — $OUT"
