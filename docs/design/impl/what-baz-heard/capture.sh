#!/usr/bin/env bash
# **What listening bought** — design note 24, photographed on a real run.
#
# The claims this exercises are all falsifiable against the fixture, which is
# the point of the feature: `mkfixture-varied.sh` builds 24 tracks whose tempo
# and loudness walk from 62 BPM at amplitude 0.18 to 168 BPM at 1.00, all
# tagged `GENRE=Electronic`, all a 220 Hz tone under a click train. So:
#
#   * the quietest and slowest record must be *Ini Kovac — Nocturne Machine*
#     and the loudest and fastest *Studio Hain — Terminal Velocity*;
#   * the tempo range must bracket 62–168 BPM;
#   * **brightness and texture must be flagged flat** and energy, tempo and
#     dynamics must not, because a tone at one frequency is exactly a library
#     with nothing to say on those two axes;
#   * the field's example must read `warm electronic, slow and sparse`,
#     because that is the genre the fixture carries;
#   * every mood must admit it has under 25 songs to draw from, because 24 is
#     the whole library.
#
# A ledger is seeded with two plays so the never-played line has something
# true to say. It is a fabrication, and it is labelled as one: two of the
# fixture's own paths, at a fixed date, written before the app starts.
#
# Headless, on a private Xvfb, with all six XDG redirections from
# docs/DEVELOPMENT.md; the run's `[mpris] no session bus` line is the receipt
# that nothing touched the owner's session or library.
#
#   toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
#     cargo build --release -p baz --features device-output
#   toolbox run -c baz-dev docs/design/impl/contour/mkfixture-varied.sh /tmp/baz-varied
#   toolbox run -c baz-dev docs/design/impl/what-baz-heard/capture.sh
set -uo pipefail
REPO=${REPO:-$(git rev-parse --show-toplevel)}
BIN=${BIN:-$REPO/target/tb/release/baz}
FIX=${FIX:-/tmp/baz-varied}
OUT=${OUT:-$REPO/docs/design/impl/what-baz-heard}
DISP=${DISP:-:198}
S=/tmp/baz-heard-scratch
W=${W:-1600}; H=${H:-980}
TAG=${TAG:-}

mkdir -p "$OUT"
[[ -d $FIX ]] || { echo "no fixture at $FIX"; exit 1; }
rm -rf "$S"; mkdir -p "$S"/{home,data,config,cache,run}; chmod 700 "$S/run"
cat > "$S/home/.asoundrc" <<'AEOF'
pcm.!default { type null }
ctl.!default { type null }
AEOF
mkdir -p "$S/config/baz" "$S/data/baz"
cat > "$S/config/baz/config.toml" <<CEOF
music_dirs = ["$FIX"]
group_key = "alphabet"
CEOF

# **The seeded ledger.** Two plays, so *you have never played 22 of these* is
# arithmetic over a real file rather than a constant. Tab-separated, in the
# format `baz_core::history` documents.
{
  printf '# baz play history. One line per play, appended, never rewritten.\n'
  printf '2026-08-01T20:00:00Z\tplayed\t24000\t24000\t%s\n' \
    "$FIX/01 - Ini Kovac - Nocturne Machine/01 Part 1.flac"
  printf '2026-08-01T20:00:30Z\tplayed\t24000\t24000\t%s\n' \
    "$FIX/05 - Corvin - Overdrive/02 Part 2.flac"
} > "$S/data/baz/history.tsv"

Xvfb "$DISP" -screen 0 "${W}x${H}x24" -nolisten tcp & XPID=$!
sleep 1
env -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS DISPLAY="$DISP" \
    WINIT_UNIX_BACKEND=x11 HOME="$S/home" XDG_DATA_HOME="$S/data" \
    XDG_CONFIG_HOME="$S/config" XDG_CACHE_HOME="$S/cache" \
    XDG_RUNTIME_DIR="$S/run" BAZ_ROOM=closing-time BAZ_VIBE_WORKERS=4 \
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

# Home → New vibe playlist, a listener's own route in.
click 32 81
click 327 515
move 1400 940
shot "01-before-listening"

# Listen, and wait by watching the store stop growing rather than by guessing.
click 1120 626
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
# The six mood surveys are six embeddings behind one mutex, fired at settle.
sleep 12
move 1400 940
shot "02-what-baz-heard"

# **The count is a door.** The never-played line is the one item on the
# reading that leads somewhere, so it is a press: it leaves the door with the
# filter already on. The coordinate is the chip at the foot of the block.
click 415 475
sleep 3
move 1400 940
shot "03-composing-from-what-you-forgot"

# Back to the door, and out again by the seventh way in, so the next two
# shots start from an untouched request.
click 32 81
click 327 515
sleep 2

# **The field's example, made of their music** — and the count that no longer
# says *match*. Through the door's own seventh way in, which is the route to
# the page with nothing filled in.
click 359 858
sleep 3
move 1400 940
shot "04-the-example-is-their-music"

# The advanced depth is where the readout under the field lives. The tabs sit
# at the head of the ask pane and the field directly under its question, so
# both coordinates are the pane's own and not the page's.
click 377 158
sleep 1
click 470 256
xdotool type --clearmodifiers --delay 30 "a slow warm pulse"
sleep 4
move 1400 940
shot "05-drew-rather-than-matched"

# **The axes this collection cannot answer.** A tone at one frequency has
# nothing to say about brightness or texture, and the opened lines say so.
click 822 582
sleep 3
move 1400 940
shot "06-flat-axes-admit-it"

kill "$APID" 2>/dev/null; wait "$APID" 2>/dev/null
kill "$XPID" 2>/dev/null; wait "$XPID" 2>/dev/null
echo "--- isolation receipt ---"
grep -m1 mpris "$S/app.log" || echo "NO MPRIS LINE — the isolation is unproven"
