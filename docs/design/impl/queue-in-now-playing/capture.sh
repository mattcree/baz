#!/usr/bin/env bash
# Render the **two places this study merges** — `Place::NowPlaying` and
# `Place::Queue` — side by side at the two sizes the study measures, against
# the real binary, headless, on a private Xvfb, with all six XDG
# redirections from docs/DEVELOPMENT.md §"Headless UI verification".
#
# Nothing touches the owner's session and nothing is audible: the scratch
# HOME routes ALSA's default PCM to null and the fixture's samples are all
# zero. The run's `[mpris] no session bus` line is the receipt that the
# isolation held, and this script prints it.
#
# These are frames of **today**, not of the design. They exist so the
# composition argument in `docs/design/12-now-playing-and-kiosk.md` §5.5a is
# made against measured pixels rather than against a recollection of them:
# what the two surfaces spend their width on, and what each one is missing
# that the other has.
#
#   toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
#     cargo build --release -p baz --features device-output
#   toolbox run -c baz-dev docs/design/composition/tools/mkfixture.sh /tmp/baz-qp-fix
#   toolbox run -c baz-dev docs/design/impl/queue-in-now-playing/capture.sh
set -uo pipefail

REPO=${REPO:-$(git rev-parse --show-toplevel)}
BIN=${BIN:-$REPO/target/tb/release/baz}
FIX=${FIX:-/tmp/baz-qp-fix}
OUT=${OUT:-$REPO/docs/design/impl/queue-in-now-playing}
DISP=${DISP:-:197}
S=/tmp/baz-qnp-scratch

mkdir -p "$OUT"
rm -rf "$S"; mkdir -p "$S"/{home,data,config,cache,run}; chmod 700 "$S/run"
cat > "$S/home/.asoundrc" <<'EOF'
pcm.!default { type null }
ctl.!default { type null }
EOF
mkdir -p "$S/config/baz"
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
park()  { xdotool mousemove $((W - 6)) 300; }

pair() { # prefix tile_x tile_y
  # A double-click on a sleeve plays that record whole (ADR-0023 §2's
  # needle-drop, at the album level), so the run below is an ordinary
  # album — the anonymous case this study's model is about. Space pauses
  # immediately so the cursor holds still for the stills; the null sink
  # otherwise races through silent tracks.
  xdotool mousemove "$2" "$3"; sleep 0.4
  xdotool click --repeat 2 --delay 120 1; sleep 1.2
  key space
  # The two places, back to back, lane open, with nothing else changed.
  key ctrl+u
  park; shot "${1}a-queue-open-${W}x${H}"
  key Escape
  sleep 0.4
  click 100 184                       # the lane head's third row: Now playing
  park; shot "${1}b-now-playing-open-${W}x${H}"
  # …and the same pair with the lane collapsed (Ctrl+B), which is the
  # width the study's kiosk rows assume.
  key ctrl+b
  park; shot "${1}c-now-playing-collapsed-${W}x${H}"
  key ctrl+u
  park; shot "${1}d-queue-collapsed-${W}x${H}"
  key ctrl+b                          # leave the lane as we found it
}

for size in "1280 860" "1920 1080"; do
  set -- $size; W=$1; H=$2
  Xvfb "$DISP" -screen 0 ${W}x${H}x24 -nolisten tcp & XPID=$!
  sleep 1
  launch "$W" "$H"
  case $W in 1280) pair 01 340 250 ;; *) pair 02 340 250 ;; esac
  kill "$APID" 2>/dev/null; wait "$APID" 2>/dev/null
  kill "$XPID" 2>/dev/null; wait "$XPID" 2>/dev/null
done

echo "--- the isolation receipt ---"
grep -E "^\[mpris\]" "$S/app.log" || echo "(no mpris line — look at $S/app.log)"
echo "done — $OUT"
