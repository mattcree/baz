#!/usr/bin/env bash
# **Item 60 — where the memory goes, measured rather than reasoned about.**
#
# The owner, 2026-08-15: *"figure out why we are using so much memory… I see
# 1.8GB."* The diagnosis in `docs/WORK.md` item 60 was made from the source and
# one idle reading: `baz-vibe` held **both** ONNX towers per worker thread, the
# text tower is 126 MB on disk, the audio tower 34 MB, and a scan runs eight
# workers — so a first analysis could materialise 1.28 GB of weights that a
# 260 MB idle baseline then sits on top of.
#
# The first repair shipped with 0.2.0: each tower is opened **where it is
# used**, so the workers hold audio weights and one thread holds the text
# tower. This script is the receipt, and the answer to the question the entry
# left open — *does it come back down when the scan ends?* The worker threads
# are tokio's blocking pool, which retires idle threads on its own; whether the
# sessions go with them is a fact about the runtime, not something to assume.
#
# It samples this process's own RSS from `/proc` every two seconds across:
# idle → composing (the analysis, at eight workers) → two minutes of quiet.
#
#   toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
#     cargo build --release -p baz --features device-output
#   toolbox run -c baz-dev docs/design/impl/vibe-memory/measure.sh
set -uo pipefail

REPO=${REPO:-$(git rev-parse --show-toplevel)}
BIN=${BIN:-$REPO/target/tb/release/baz}
FIX=${FIX:-/tmp/baz-varied}
OUT=${OUT:-$REPO/docs/design/impl/vibe-memory}
DISP=${DISP:-:20${BAZ_VIBE_WORKERS:-7}}
S=/tmp/baz-memory-scratch-${BAZ_VIBE_WORKERS:-default}
W=1280
H=860

mkdir -p "$OUT"
[[ -d $FIX ]] || { echo "no fixture at $FIX — run docs/design/impl/contour/mkfixture-varied.sh"; exit 1; }
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

Xvfb "$DISP" -screen 0 "${W}x${H}x24" -nolisten tcp &
XPID=$!
sleep 1
env -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS DISPLAY="$DISP" \
    WINIT_UNIX_BACKEND=x11 HOME="$S/home" XDG_DATA_HOME="$S/data" \
    XDG_CONFIG_HOME="$S/config" XDG_CACHE_HOME="$S/cache" \
    XDG_RUNTIME_DIR="$S/run" BAZ_ROOM=closing-time \
    BAZ_VIBE_MODEL_DIR="$REPO/models/vibe" \
    ${BAZ_VIBE_WORKERS:+BAZ_VIBE_WORKERS="$BAZ_VIBE_WORKERS"} \
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
sleep 8

rss() { awk '/VmRSS/ {print $2}' "/proc/$APID/status" 2>/dev/null; }

click() { xdotool mousemove "$1" "$2"; sleep 0.3; xdotool click 1; sleep 1.4; }

log="$OUT/rss-${BAZ_VIBE_WORKERS:-default}.tsv"
: > "$log"
sample() { # seconds  label
  local until=$((SECONDS + $1))
  while (( SECONDS < until )); do
    printf '%d\t%s\t%s\n' "$SECONDS" "$(rss)" "$2" >> "$log"
    sleep 2
  done
}

sample 10 idle

# Home → the vibe route. `New vibe playlist` is Home's own door to it, which
# is a listener's route rather than a deep link.
click 32 81
click 327 515
sleep 1
# The request field, then Compose — the form's own `on_submit`, so the press
# does not depend on where the button falls.
click 640 468
xdotool type --clearmodifiers --delay 40 "a slow build into something loud"
xdotool key --clearmodifiers Return
sample 90 composing
sample 120 after

kill "$APID" 2>/dev/null; wait "$APID" 2>/dev/null
kill "$XPID" 2>/dev/null; wait "$XPID" 2>/dev/null

echo "--- peak by phase (MiB) ---"
awk -F'\t' '{ mb=$2/1024; if (mb > peak[$3]) peak[$3]=mb; last[$3]=mb }
            END { for (phase in peak) printf "%-10s peak %7.1f   last %7.1f\n", phase, peak[phase], last[phase] }' "$log"
echo "--- isolation receipt ---"
grep -m1 mpris "$S/app.log" || echo "NO MPRIS LINE — the isolation is unproven"
grep -m3 -i 'vibe\|analys' "$S/app.log" || true
