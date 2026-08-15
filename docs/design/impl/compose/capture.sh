#!/usr/bin/env bash
# **The composing place, state by state** — plan 22 §7's ship gate.
#
# Design 21 §7 draws nine states and the shipping build designed one. This
# exercises them in the real binary against a real analysed fixture, because
# every claim the page makes — the live count, the eligible cloud thinning
# under the curve, the ticks, the diff naming its cause — needs an analysed
# collection to be *true* rather than asserted.
#
# Headless, on a private Xvfb, with all six XDG redirections from
# docs/DEVELOPMENT.md; the run's `[mpris] no session bus` line is the receipt
# that nothing touched the owner's session or library.
#
#   toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
#     cargo build --release -p baz --features device-output
#   toolbox run -c baz-dev docs/design/impl/contour/mkfixture-varied.sh /tmp/baz-varied
#   toolbox run -c baz-dev docs/design/impl/compose/capture.sh
set -uo pipefail
REPO=${REPO:-$(git rev-parse --show-toplevel)}
BIN=${BIN:-$REPO/target/tb/release/baz}
FIX=${FIX:-/tmp/baz-varied}
OUT=${OUT:-$REPO/docs/design/impl/compose}
DISP=${DISP:-:197}
S=/tmp/baz-compose-scratch
# Wide enough for the two panes (COMPOSE_BREAKPOINT is 1440) and tall enough
# for the curve (COMPOSE_SHORT_H is 700).
W=${W:-1600}; H=${H:-980}
TAG=${TAG:-}

mkdir -p "$OUT"
[[ -d $FIX ]] || { echo "no fixture at $FIX"; exit 1; }
rm -rf "$S"; mkdir -p "$S"/{home,data,config,cache,run}; chmod 700 "$S/run"
cat > "$S/home/.asoundrc" <<'AEOF'
pcm.!default { type null }
ctl.!default { type null }
AEOF
mkdir -p "$S/config/baz"
cat > "$S/config/baz/config.toml" <<CEOF
music_dirs = ["$FIX"]
group_key = "alphabet"
CEOF

Xvfb "$DISP" -screen 0 "${W}x${H}x24" -nolisten tcp & XPID=$!
sleep 1
env -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS DISPLAY="$DISP" \
    WINIT_UNIX_BACKEND=x11 HOME="$S/home" XDG_DATA_HOME="$S/data" \
    XDG_CONFIG_HOME="$S/config" XDG_CACHE_HOME="$S/cache" \
    XDG_RUNTIME_DIR="$S/run" BAZ_ROOM=closing-time BAZ_VIBE_WORKERS=8 \
    BAZ_VIBE_MODEL_DIR="$REPO/models/vibe" \
    "$BIN" >> "$S/app.log" 2>&1 & APID=$!
WID=""
for _ in $(seq 1 60); do
  WID=$(DISPLAY=$DISP timeout 3 xdotool search --sync --onlyvisible --class baz 2>/dev/null | head -1)
  [[ -n $WID ]] && break
  sleep 0.25
done
if [[ -z $WID ]]; then echo "NO WINDOW"; tail -20 "$S/app.log"; kill $APID $XPID 2>/dev/null; exit 1; fi
export DISPLAY=$DISP
xdotool windowmove "$WID" 0 0; xdotool windowsize "$WID" $W $H; xdotool windowfocus --sync "$WID"
sleep 6
shot(){ sleep 1.2; magick import -window root "$OUT/$1${TAG}.png"; echo "  shot $1${TAG}"; }
click(){ xdotool mousemove "$1" "$2"; sleep 0.3; xdotool click 1; sleep 1.0; }
move(){ xdotool mousemove "$1" "$2"; sleep 0.8; }

# Home → New vibe playlist. A listener's own route in, not a deep link.
click 32 81
click 327 515
move 1400 940
shot "01-never-listened"

# **1 → 2.** The ask pane is fully live while nothing has been heard, which is
# the whole of what makes the first state not read as broken. Type before
# pressing anything.
click 470 194
xdotool type --clearmodifiers --delay 30 "warm late-night listening"
sleep 1
move 1400 940
shot "02-ask-live-while-cold"

# Listen to my music — the offer, stated with its cost, rather than something
# baz did without being asked. It is the one accent-weight control on this
# state; `Compose` beside it says what it needs and waits.
click 1120 626
sleep 6
move 1400 940
shot "03-listening"

# **Wait by watching the store stop growing**, rather than by guessing.
DB="$S/data/baz/vibe.db"
last=-1; still=0
for _ in $(seq 1 120); do
  sleep 5
  size=$(stat -c %s "$DB" 2>/dev/null || echo 0)
  if [[ "$size" == "$last" && "$size" != "0" ]]; then
    still=$((still + 1)); [[ $still -ge 3 ]] && break
  else still=0; fi
  last=$size
done
sleep 8
move 1400 940
shot "04-ready"

# **4 · Asked.** A starting point writes into the one input there is.
click 323 307
sleep 3
move 1400 940
shot "05-started-from-a-mood"

# Compose.
click 470 194
xdotool key --clearmodifiers Return
sleep 6
move 1400 940
shot "06-a-list"

# **6 · A row explains itself** — three cues, none of them a colour.
click 1100 640
move 1400 940
shot "07-why-this-song"

# **The diff.** Narrow the words and compose again: the sentence has to name
# the words as the cause and give both counts.
click 470 194
xdotool key --clearmodifiers End
xdotool type --clearmodifiers --delay 30 ", piano"
sleep 2
xdotool key --clearmodifiers Return
sleep 6
move 1400 940
shot "08-diff-after-narrowing"

# …and *another version*, which is the visible press that carries the
# variation the engine used to take invisibly on every compose.
click 340 700
sleep 5
move 1400 940
shot "09-another-version"

# **The expander** reveals the per-dimension lines rather than seeding them.
click 790 470
sleep 2
move 1400 940
shot "10-each-thing-baz-listens-for"

kill "$APID" 2>/dev/null; wait "$APID" 2>/dev/null
kill "$XPID" 2>/dev/null; wait "$XPID" 2>/dev/null
echo "--- isolation receipt ---"
grep -m1 mpris "$S/app.log" || echo "NO MPRIS LINE — the isolation is unproven"
