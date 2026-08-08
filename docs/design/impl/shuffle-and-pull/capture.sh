#!/usr/bin/env bash
# Render the shuffle pool and the pull, headless, on a private Xvfb, with all
# six XDG redirections from docs/DEVELOPMENT.md. Nothing touches the owner's
# session; the run's `[mpris] no session bus` line is the receipt that it did
# not, and this script prints it.
#
# A sibling of docs/design/composition/tools/capture.sh, kept separately so a
# parallel pass rewriting that ruler and this one cannot collide. The fixture is
# that directory's own mkfixture.sh — 25 albums of digital silence, which is
# enough that a pool is visibly a subset of the wall.
#
#   docs/design/composition/tools/mkfixture.sh /tmp/baz-pool-fixture
#   docs/design/impl/shuffle-and-pull/capture.sh
set -uo pipefail

REPO=${REPO:-$(git rev-parse --show-toplevel)}
BIN=$REPO/target/release/baz
FIX=${FIX:-/tmp/baz-pool-fixture}
OUT=${OUT:-$REPO/docs/design/impl/shuffle-and-pull}
W=${W:-1280}
H=${H:-860}
DISP=${DISP:-:173}
S=/tmp/baz-pool-scratch

mkdir -p "$OUT"
rm -rf "$S"
mkdir -p "$S"/{home,data,config,cache,run}
chmod 700 "$S/run"
# Two independent guarantees of silence: every sample in the fixture is a zero,
# and the default PCM discards them.
cat > "$S/home/.asoundrc" <<'EOF'
pcm.!default { type null }
ctl.!default { type null }
EOF

Xvfb "$DISP" -screen 0 "${W}x${H}x24" -nolisten tcp &
XPID=$!
sleep 1

env -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS DISPLAY="$DISP" \
    WINIT_UNIX_BACKEND=x11 HOME="$S/home" XDG_DATA_HOME="$S/data" \
    XDG_CONFIG_HOME="$S/config" XDG_CACHE_HOME="$S/cache" \
    XDG_RUNTIME_DIR="$S/run" BAZ_ROOM=closing-time \
    "$BIN" "$FIX" > "$S/app.log" 2>&1 &
APID=$!

WID=""
for _ in $(seq 1 80); do
  WID=$(DISPLAY=$DISP xdotool search --sync --onlyvisible --class baz 2>/dev/null | head -1)
  [[ -n $WID ]] && break
  sleep 0.25
done
if [[ -z $WID ]]; then echo "NO WINDOW"; cat "$S/app.log"; kill $APID $XPID 2>/dev/null; exit 1; fi
export DISPLAY=$DISP
xdotool windowmove "$WID" 0 0
xdotool windowsize "$WID" "$W" "$H"
xdotool windowfocus --sync "$WID"
sleep 4   # let the scan land

shot() { sleep 0.8; magick import -window root "$OUT/$1.png"; echo "  shot $1"; }
mv()   { xdotool mousemove "$1" "$2"; sleep 0.4; }
klick(){ xdotool mousemove "$1" "$2"; sleep 0.3; xdotool click 1; sleep 0.7; }
key()  { xdotool key "$@"; sleep 0.6; }
typ()  { xdotool type --delay 40 "$1"; sleep 1.0; }

PARK_X=$((W - 6)); PARK_Y=$((H / 2))

# ---------------------------------------------------------------------------
# 1. Shuffle a *filtered* wall, then clear the filter.
#
# The pool is what the wall shows, so with no query the pool is everything and
# nothing dims — correctly. Narrowing first and widening after is what makes the
# pool visibly a subset, which is the state the two marks exist for.
# ---------------------------------------------------------------------------
# `so` narrows 25 records to the four filed under Sotto and Sonja Aalto. Four is
# the useful number here: a shuffle draws all of them, so two carry rings and two
# do not, and the wall ends up showing all three states of the mark at once.
BLUR_X=950; BLUR_Y=24    # empty top-bar wall, between `Pull` and the counts

typ "so"
mv $PARK_X $PARK_Y;                     shot 01-wall-filtered-before-shuffle

# The search well has focus from launch; the words in the bar are pointer
# targets, so the pool is started by pointer exactly as a listener would.
klick 785 24                                          # `Shuffle`
mv $PARK_X $PARK_Y;                     shot 02-shuffle-pool-filtered

# Clear the query. The wall widens back to 25 records; the pool is still the
# four it was drawn from, and now says so. Clearing the query hands focus back
# to the well, so the blur has to be re-taken before any further key.
klick $BLUR_X $BLUR_Y
key Escape
klick $BLUR_X $BLUR_Y
# The index rail's `S`, so the frame holds both states of the mark at once: the
# S shelf carries the four records the shuffle is drawing from *and* the two
# filed under Studio Hain, which it is not.
klick 1236 498
mv $PARK_X $PARK_Y;                     shot 03-pool-visible-on-the-whole-wall

# The queue the shuffle built: whole records, each named where it begins.
key q
mv $PARK_X $PARK_Y;                     shot 04-shuffle-queue
key Escape                                            # closes the popover

# ---------------------------------------------------------------------------
# 2. The pull.
# ---------------------------------------------------------------------------
key Escape                                            # peels the pool's marks
mv $PARK_X $PARK_Y;                     shot 05-esc-peels-the-pools-marks
key ctrl+r; sleep 1.2
mv $PARK_X $PARK_Y;                     shot 06-the-pull
key ctrl+r; sleep 1.2
mv $PARK_X $PARK_Y;                     shot 07-the-pull-again
key Escape; sleep 1.0
mv $PARK_X $PARK_Y;                     shot 08-esc-returns-the-pull

grep -m1 "mpris" "$S/app.log"
grep -m1 "shuffle" "$S/app.log"
grep -m2 "pull" "$S/app.log"
kill $APID 2>/dev/null; sleep 0.6; kill -9 $APID 2>/dev/null
kill $XPID 2>/dev/null; sleep 0.3; kill -9 $XPID 2>/dev/null
wait 2>/dev/null
echo "done"
