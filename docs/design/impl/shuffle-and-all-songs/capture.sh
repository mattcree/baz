#!/usr/bin/env bash
# Render the strip without `Pull`, the shuffle toggle in both states, and the
# All songs row — headless, on a private Xvfb, with all six XDG redirections
# from docs/DEVELOPMENT.md. Nothing touches the owner's session; the run's
# `[mpris] no session bus` line is the receipt that it did not, and this script
# prints it.
#
# The direct descendant of docs/design/impl/shuffle-and-pull/capture.sh, whose
# subjects no longer exist: `Pull` was removed on 2026-08-10 and shuffle stopped
# being a draw from the wall on the same day, so the pool's dimming and rings
# have nothing left to photograph. This harness photographs what replaced them.
#
# The fixture is docs/design/composition/tools/mkfixture.sh — 25 albums of
# digital silence, which is enough that a queue is visibly several records and
# that `All songs` has a collage to draw.
#
#   docs/design/composition/tools/mkfixture.sh /tmp/baz-shuffle-fixture
#   docs/design/impl/shuffle-and-all-songs/capture.sh
set -uo pipefail

REPO=${REPO:-$(git rev-parse --show-toplevel)}
BIN=$REPO/target/release/baz
FIX=${FIX:-/tmp/baz-shuffle-fixture}
OUT=${OUT:-$REPO/docs/design/impl/shuffle-and-all-songs}
W=${W:-1280}
H=${H:-860}
DISP=${DISP:-:175}
S=/tmp/baz-shuffle-scratch

mkdir -p "$OUT"
rm -rf "$S"
mkdir -p "$S"/{home,data,config,cache,run}
chmod 700 "$S/run"
# Two independent guarantees of silence: every sample in the fixture is a zero,
# and the default PCM discards them. BAZ_DEVICE_TESTS stays unset.
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

PARK_X=$((W - 6)); PARK_Y=$((H / 2))

# **Everything here is driven by pointer, and that is not a stylistic choice.**
# The search well holds focus from launch and it now lives in the returns lane
# (ADR-0030's search amendment), so iced's `text_input` consumes bare letters
# wherever the pointer is: an earlier version of this script pressed `q` for the
# Queue door and photographed a search for "qq" instead. Every control below has
# a visible, pointer-reachable form — which is the product's own rule — so the
# harness uses those and never a bare letter.
#
# Coordinates are read off 01 and 02 at this exact geometry (1280 x 860, lane
# open); change W or H and they must be re-read.
QUEUE_X=441; QUEUE_Y=819   # the bar's labelled `Queue` door
PAUSE_X=640; PAUSE_Y=819   # the bar's play/pause
SHUF_X=965;  SHUF_Y=819    # the bar's crossed arrows

# The Queue place is a *door*: the same press opens and closes it.
queue_shot() { klick $QUEUE_X $QUEUE_Y; mv $PARK_X $PARK_Y; shot "$1"; klick $QUEUE_X $QUEUE_Y; }

# ---------------------------------------------------------------------------
# 1. The strip, with `Pull` gone.
#
# What is left in the acts cluster is `Play all` and nothing else — the whole
# of ACTS_W's fall from 182 to 88, photographed. The lane holds the well and
# the counts; the five group keys are unchanged.
# ---------------------------------------------------------------------------
mv $PARK_X $PARK_Y;                     shot 01-the-strip-without-pull

# ---------------------------------------------------------------------------
# 2. The shuffle toggle, off and on, on the now-playing bar.
#
# Off first, and with something sounding, so that the frame carries the state
# the control is *about*. The toggle is the crossed arrows at the head of the
# bar's right-hand zone — the properties zone, beside the volume.
# ---------------------------------------------------------------------------
klick 700 24                                          # `Play all`
sleep 3
# **Pause before photographing the run**, and the reason is the comparison
# below rather than tidiness. The fixture is silent FLACs against a null ALSA
# device, so the engine drains a track as fast as it can decode one and the
# cursor walks the queue while the camera is loading: an earlier version of
# this script compared two frames taken eight seconds apart and measured the
# playhead moving, not the order changing. Paused, the only thing that can
# differ between 03 and 08 is what this feature does to the order.
klick $PAUSE_X $PAUSE_Y
sleep 1
mv $PARK_X $PARK_Y;                     shot 02-shuffle-off-with-a-run

# **The run before shuffle touches it** — the order `Play all` built, which is
# the wall's own arrangement. This frame is the control in the experiment below.
queue_shot 03-the-run-before-shuffle

# The bar's own control. Lit is the accent; the tooltip names which way the
# next press goes, which is the second channel the state is carried on.
klick $SHUF_X $SHUF_Y
mv $PARK_X $PARK_Y;                     shot 04-shuffle-on
mv $SHUF_X $SHUF_Y;                     shot 05-shuffle-on-tooltip

# The run, re-ordered forward of the needle and nothing behind it moved.
queue_shot 06-the-run-shuffled

# Off again, and the run goes back into the order `Play all` built it in.
klick $SHUF_X $SHUF_Y
mv $PARK_X $PARK_Y;                     shot 07-shuffle-off-again
queue_shot 08-the-run-restored

# ---------------------------------------------------------------------------
# **The claim this whole feature is judged on, as an image comparison.**
#
# 03 is the run before shuffle; 08 is the run after on-then-off. If turning
# shuffle off restores the unshuffled order, the *rows* are the same picture.
# 06 is the shuffled run in between, which must differ — otherwise the first
# comparison would be satisfied by a shuffle that never shuffled.
#
# The comparison is over a **crop of the rows column**, not the whole frame, and
# that is the honest form of it: the bar's elapsed figure and the needle move
# between any two shots taken seconds apart, so a whole-frame `AE` counts the
# clock as a difference and answers a question nobody asked.
# ---------------------------------------------------------------------------
ROWS="760x560+300+110"
rows() { magick "$OUT/$1.png" -crop "$ROWS" +repage "$S/rows-$1.png"; }
rows 03-the-run-before-shuffle
rows 06-the-run-shuffled
rows 08-the-run-restored

echo "  the rows: restored vs. before  (must be 0):"
magick compare -metric AE "$S/rows-03-the-run-before-shuffle.png" \
  "$S/rows-08-the-run-restored.png" "$OUT/09-diff-restored-vs-before.png" 2>&1 | tail -1
echo ""
echo "  the rows: shuffled vs. before  (must not be 0):"
magick compare -metric AE "$S/rows-03-the-run-before-shuffle.png" \
  "$S/rows-06-the-run-shuffled.png" "$OUT/10-diff-shuffled-vs-before.png" 2>&1 | tail -1
echo ""

# ---------------------------------------------------------------------------
# 3. All songs, in the playlist panel.
#
# The first row of the directory: name, counts, collage sleeve. Then the same
# panel with a pick in flight, which is the frame that proves the negative —
# every other row says `Add` and this one does not, because there is no file
# behind it to write to.
# ---------------------------------------------------------------------------
key ctrl+p
mv $PARK_X $PARK_Y;                     shot 11-all-songs-in-the-panel

grep -m1 "mpris" "$S/app.log"
grep -m2 "all-songs" "$S/app.log"
grep -m2 "shuffle" "$S/app.log"
kill $APID 2>/dev/null; sleep 0.6; kill -9 $APID 2>/dev/null
kill $XPID 2>/dev/null; sleep 0.3; kill -9 $XPID 2>/dev/null
wait 2>/dev/null
echo "done"
