#!/usr/bin/env bash
# **The Vibe page as it stands, at two widths** — the evidence for
# `docs/design/19-vibe-next-phase.md`.
#
# The owner, 2026-08-15: *"the ui layout for the vibe playlist isn't great…
# it's just not well optimised for a wide screen. we have to scroll to see the
# playlist. it should work on both wide and narrow layouts."*
#
#   1600 × 900 — a desktop window, where the right half of the page is empty
#                and the list is still below the fold
#   1000 × 700 — a narrow window, where the same column is the whole page
#
# Two frames each: the form cold, and the form after a recipe fills it in.
#
#   toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
#     cargo build --release -p baz --features device-output
#   toolbox run -c baz-dev env FIX=/tmp/baz-varied \
#     docs/design/impl/vibe-next-phase/capture.sh
set -uo pipefail

REPO=${REPO:-$(git rev-parse --show-toplevel)}
BIN=${BIN:-$REPO/target/tb/release/baz}
FIX=${FIX:-/tmp/baz-varied}
OUT=${OUT:-$REPO/docs/design/impl/vibe-next-phase}
DISP=${DISP:-:208}
S=/tmp/baz-vibeui-scratch

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

shoot() { # W H
  local W=$1 H=$2
  Xvfb "$DISP" -screen 0 "${W}x${H}x24" -nolisten tcp &
  local XPID=$!
  sleep 1
  env -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS DISPLAY="$DISP" \
      WINIT_UNIX_BACKEND=x11 HOME="$S/home" XDG_DATA_HOME="$S/data" \
      XDG_CONFIG_HOME="$S/config" XDG_CACHE_HOME="$S/cache" \
      XDG_RUNTIME_DIR="$S/run" BAZ_ROOM=closing-time \
      BAZ_VIBE_MODEL_DIR="$REPO/models/vibe" \
      "$BIN" >> "$S/app-$W.log" 2>&1 &
  local APID=$!
  local WID=""
  for _ in $(seq 1 40); do
    WID=$(DISPLAY=$DISP timeout 3 xdotool search --sync --onlyvisible --class baz 2>/dev/null | head -1)
    [[ -n $WID ]] && break
    sleep 0.25
  done
  if [[ -z $WID ]]; then echo "NO WINDOW at ${W}x${H}"; cat "$S/app-$W.log"; kill "$APID" "$XPID" 2>/dev/null; return 1; fi
  export DISPLAY=$DISP
  xdotool windowmove "$WID" 0 0
  xdotool windowsize "$WID" "$W" "$H"
  xdotool windowfocus --sync "$WID"
  sleep 6

  shot()  { sleep 1.2; magick import -window root "$OUT/$1.png"; echo "  shot $1"; }
  click() { xdotool mousemove "$1" "$2"; sleep 0.3; xdotool click 1; sleep 1.4; }
  park()  { xdotool mousemove $((W - 120)) $((H - 160)); sleep 0.5; xdotool mousemove $((W - 118)) $((H - 158)); sleep 0.9; }

  # Home → its own `New vibe playlist` door. The lane's rows are arithmetic
  # (APP_BAR_H 49 + SIDEBAR_PAD 8 + n × 52 + 24); the door is photographed.
  click 32 81
  click 327 515
  park
  shot "01-cold-${W}x${H}"
  # `Sunday morning`, the second mood, which fills words + shape + length.
  click 436 364
  park
  shot "02-filled-${W}x${H}"

  kill "$APID" 2>/dev/null; wait "$APID" 2>/dev/null
  kill "$XPID" 2>/dev/null; wait "$XPID" 2>/dev/null
  echo "--- isolation receipt (${W}x${H}) ---"
  grep -m1 mpris "$S/app-$W.log" || echo "NO MPRIS LINE — the isolation is unproven"
}

shoot 1600 900
shoot 1000 700
