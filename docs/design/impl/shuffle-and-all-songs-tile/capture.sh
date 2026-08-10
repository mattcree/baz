#!/usr/bin/env bash
# Render the two things the owner asked for on 2026-08-10 after seeing what
# that morning's work shipped as — shuffle that does not mutate the run, and
# `All songs` as a tile on Home — headless, on a private Xvfb, with all six XDG
# redirections from docs/DEVELOPMENT.md. Nothing touches the owner's session;
# the run's `[mpris] no session bus` line is the receipt that it did not, and
# this script prints it.
#
# The direct descendant of docs/design/impl/shuffle-and-all-songs/capture.sh,
# whose central experiment no longer describes the product: that harness
# photographed the run *before* shuffle, the run *shuffled*, and the run
# *restored*, and compared the first against the third at zero differing pixels.
# There is nothing to restore now. The experiment inverts, and the inversion is
# the whole claim of this feature:
#
#   **the rows must be identical in all three frames — off, on, and off
#   again — because the list is never touched.** What differs is which row
#   carries the mark.
#
# The fixture is docs/design/composition/tools/mkfixture.sh — 25 albums of
# digital silence, which is enough that a run is visibly several records and
# that `All songs` has a collage to draw.
#
#   docs/design/composition/tools/mkfixture.sh /tmp/baz-tile-fixture
#   cargo build --release -p baz --features device-output
#   docs/design/impl/shuffle-and-all-songs-tile/capture.sh
set -uo pipefail

REPO=${REPO:-$(git rev-parse --show-toplevel)}
BIN=$REPO/target/release/baz
FIX=${FIX:-/tmp/baz-tile-fixture}
OUT=${OUT:-$REPO/docs/design/impl/shuffle-and-all-songs-tile}
W=${W:-1280}
H=${H:-860}
DISP=${DISP:-:177}
S=/tmp/baz-tile-scratch

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

PARK_X=$((W - 6)); PARK_Y=$((H / 2))

# **Everything here is driven by pointer**, for its predecessor's reason: the
# search well holds focus from launch and lives in the returns lane, so iced's
# `text_input` consumes bare letters wherever the pointer is. Every control
# below has a visible, pointer-reachable form — the product's own rule — so the
# harness uses those and never a bare letter.
#
# Coordinates are read off the frames at this exact geometry (1280 x 860, lane
# open); change W or H and they must be re-read.
HOME_X=100;  HOME_Y=84    # the lane's `Home` destination
LIB_X=100;   LIB_Y=124    # the lane's `Library` destination
NOW_X=100;   NOW_Y=164    # the lane's `Now playing` destination
TILE_X=446;  TILE_Y=166   # Home's `All songs` tile, on the sleeve
CAP_X=446;   CAP_Y=310    # the same tile's caption — inside the button, outside the veil
PAUSE_X=640; PAUSE_Y=818  # the bar's play/pause
SHUF_X=966;  SHUF_Y=818   # the bar's crossed arrows

# ---------------------------------------------------------------------------
# 1. Home, with the tile on it.
#
# Nothing has played, so `CONTINUE` is absent and the tile is the first thing
# on the page — which is the ordinary state of Home and the case the placement
# argument turns on. It stands on the same lattice as `RECENTLY ADDED` below:
# one column of the wall's own grid, at the wall's own density.
# ---------------------------------------------------------------------------
klick $HOME_X $HOME_Y
mv $PARK_X $PARK_Y;                     shot 01-home-with-the-all-songs-tile

# The hover layer: the wall's own veil, with two options where a record has
# four. `Add to…` is absent by construction — there is no file behind the list.
mv $TILE_X $TILE_Y;                     shot 02-the-tiles-hover-options

# ---------------------------------------------------------------------------
# 2. The tile plays everything.
#
# Pressed on its **caption**, which is inside the tile's button and outside the
# veil's band — so this is the tile's own press rather than the veil's `Play`,
# and the frame proves the whole tile is the control.
# ---------------------------------------------------------------------------
klick $CAP_X $CAP_Y
sleep 3
# **Pause before photographing the run**, and the reason is the comparison
# below rather than tidiness. The fixture is silent FLACs against a null ALSA
# device, so the engine drains a track as fast as it can decode one and the
# cursor walks the run while the camera is loading. Paused, the only thing that
# can differ between the frames below is what this feature does.
klick $PAUSE_X $PAUSE_Y
sleep 1

# **The run, with shuffle off.** The merged now-playing place: the record on
# the left, the run column on the right. The sounding row carries the filled
# lamp dot; the row under it carries the open ring — *what plays next*, stated
# in both modes because it is true in both.
klick $NOW_X $NOW_Y
mv $PARK_X $PARK_Y;                     shot 03-the-run-shuffle-off

# ---------------------------------------------------------------------------
# 3. Shuffle on. **The rows do not move.**
# ---------------------------------------------------------------------------
# The tooltip first, with the pointer still on the control that was pressed —
# so that nothing moves between the two run frames the comparison is over.
klick $SHUF_X $SHUF_Y;                  shot 04-shuffle-on-tooltip
mv $PARK_X $PARK_Y;                     shot 05-the-run-shuffle-on

# Off again.
klick $SHUF_X $SHUF_Y
mv $PARK_X $PARK_Y;                     shot 06-the-run-shuffle-off-again

# ---------------------------------------------------------------------------
# **The claim this feature is judged on, as an image comparison — inverted
# from its predecessor's.**
#
# The old harness proved that turning shuffle *off* restored the order. There is
# no restoration now: the list is never touched, so 03, 04 and 06 must show the
# **same rows** — off, on, and off again. What differs is which row carries
# which mark.
#
# That inversion needs a measurement that separates *the rows* from *the marks*,
# because both are pixels in the same column. **The duration lane is that
# measurement**: `views::queue`'s row draws it in `paper_faint` unconditionally,
# where the number lane carries the dot and the ring and the title lane dims
# behind the pass. So the durations are invariant to every row state and change
# only if a row *moves* — a zero there is "the list was not permuted", and
# nothing else.
#
# Then the opposite: the **whole column** must differ between off and on, or the
# first comparison would be satisfied by a mode that did nothing at all.
#
# Both crops are of the run column only. The bar's elapsed figure and the needle
# move between any two shots taken seconds apart, so a whole-frame `AE` would
# count the clock as a difference and answer a question nobody asked.
# ---------------------------------------------------------------------------
DURATIONS="50x580+1075+195"
COLUMN="340x580+790+195"
crop() { magick "$OUT/$2.png" -crop "$1" +repage "$S/$3-$2.png"; }
for f in 03-the-run-shuffle-off 05-the-run-shuffle-on 06-the-run-shuffle-off-again; do
  crop "$DURATIONS" "$f" durations
  crop "$COLUMN" "$f" column
done

echo "  the durations: shuffle on vs. off  (must be 0 — no row moved):"
magick compare -metric AE "$S/durations-03-the-run-shuffle-off.png" \
  "$S/durations-05-the-run-shuffle-on.png" "$OUT/07-diff-durations-on-vs-off.png" 2>&1 | tail -1
echo ""
echo "  the durations: off again vs. off  (must be 0):"
magick compare -metric AE "$S/durations-03-the-run-shuffle-off.png" \
  "$S/durations-06-the-run-shuffle-off-again.png" \
  "$OUT/08-diff-durations-off-again-vs-off.png" 2>&1 | tail -1
echo ""
echo "  the whole column: on vs. off  (must NOT be 0 — the marks moved):"
magick compare -metric AE "$S/column-03-the-run-shuffle-off.png" \
  "$S/column-05-the-run-shuffle-on.png" "$OUT/09-diff-column-on-vs-off.png" 2>&1 | tail -1
echo ""

# ---------------------------------------------------------------------------
# 4. The wall, for the strip's `Play all` — which stayed, at a different scope.
# ---------------------------------------------------------------------------
klick $LIB_X $LIB_Y
mv $PARK_X $PARK_Y;                     shot 10-the-strip-keeps-play-all

# ---------------------------------------------------------------------------
# 5. **The ring, at record scale.**
#
# The frames above are a 206-track run, where the bag's next entry is usually
# outside the drawn window — honest, and no use as a photograph of the mark. So
# the same pair over **one record**, where twelve rows are the whole run and the
# ring cannot be off screen: shuffle off, the ring is the row under the dot;
# shuffle on, the rows have not moved and the ring is somewhere else.
# ---------------------------------------------------------------------------
klick $LIB_X $LIB_Y
mv 723 250                                            # hover the wall's second tile
klick 660 160                                         # its veil's `Play`
sleep 3
klick $PAUSE_X $PAUSE_Y
sleep 1
klick $NOW_X $NOW_Y
mv $PARK_X $PARK_Y;                     shot 11-a-record-shuffle-off
klick $SHUF_X $SHUF_Y
mv $PARK_X $PARK_Y;                     shot 12-a-record-shuffle-on

for f in 11-a-record-shuffle-off 12-a-record-shuffle-on; do crop "$DURATIONS" "$f" durations; done
echo "  one record, the durations: on vs. off  (must be 0 — no row moved):"
magick compare -metric AE "$S/durations-11-a-record-shuffle-off.png" \
  "$S/durations-12-a-record-shuffle-on.png" "$OUT/13-diff-durations-one-record.png" 2>&1 | tail -1
echo ""

grep -m1 "mpris" "$S/app.log"
grep -m3 "all-songs" "$S/app.log"
grep -m3 "shuffle" "$S/app.log"
kill $APID 2>/dev/null; sleep 0.6; kill -9 $APID 2>/dev/null
kill $XPID 2>/dev/null; sleep 0.3; kill -9 $XPID 2>/dev/null
wait 2>/dev/null
echo "done"
