#!/usr/bin/env bash
# ADR-0042's evidence: a folder removed and added back keeps its records' ADDED.
#
#   toolbox run -c baz-dev env CARGO_TARGET_DIR=<somewhere> \
#     cargo build --release -p baz --features device-output
#   toolbox run -c baz-dev docs/design/impl/forget-and-remember/harness.sh
#
# # What it proves and what it refuses to prove
#
# **The value from before, not merely a value.** A fixture scanned five minutes
# ago and re-scanned five minutes later would show `This evening` at both ends
# and prove nothing: today equals today. So the library is *aged* before the
# round trip — its rows' `first_seen_ns` written straight into the scratch
# database at four years, one year and three months old — and the assertion is
# that the same three shelves, with the same records on them, are on the wall
# after the folder has been forgotten and added back.
#
# Writing those timestamps from outside is not arranging the result. It is the
# one fact in the schema that **no press can reach**: `UPSERT_TRACK` names
# `first_seen_ns` in its INSERT list and omits it from its update list
# (ADR-0019 §5), so there is no route through the interface that could age a
# library, and a harness that waited four years is not a harness. Everything
# the feature does — the forgetting, the re-adding, the restoring — happens
# through the product, by presses, with nothing set from the outside.
#
# # The route is a listener's route
#
# Every state below is reached by pressing what a listener would press: the
# gear in the app bar, `Remove` and then `Forget` on the folder's own row,
# the add-a-folder well and its word. Nothing is deep-linked and nothing is
# written into the config that a press could not reach. A capture that arrives
# at its frame by a route nobody takes has produced a false picture on this
# project six times.
#
# # Headless, and isolated six ways
#
# Xvfb, and all six XDG redirections from docs/DEVELOPMENT.md, so nothing
# reaches the owner's library, settings or session bus. The run's
# `[mpris] no session bus` line is the receipt and this script prints it.
# Every sample in the fixture is a zero and the scratch HOME routes ALSA's
# default PCM to null. `BAZ_DEVICE_TESTS` stays unset.
set -uo pipefail

REPO=${REPO:-$(git rev-parse --show-toplevel)}
BIN=${BIN:?set BIN to the release binary built for this branch}
FIX=${FIX:-/tmp/baz-forget-fixture}
OUT=${OUT:-$REPO/docs/design/impl/forget-and-remember}
DISP=${DISP:-:196}
S=${S:-/tmp/baz-forget-scratch}
W=1600; H=900

# Four years, one year and three months, in days — the three ADDED shelves the
# wall must still draw after the round trip.
AGES=(1460 365 90)

mkdir -p "$OUT"

# ------------------------------------------------------------------- fixture
if [[ ! -d $FIX ]]; then
  "$REPO/docs/design/composition/tools/mkfixture.sh" "$FIX"
fi

# ------------------------------------------------------------------- scratch
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
group_key = "added"
density = "dense"
sidebar_open = true
EOF

DB="$S/data/baz/library.db"

# ------------------------------------------------------------------- display
Xvfb "$DISP" -screen 0 ${W}x${H}x24 -nolisten tcp >/dev/null 2>&1 &
XPID=$!
export DISPLAY=$DISP
# Wait for the server rather than sleeping at it. A fixed sleep is why this
# script once photographed nothing: a previous run's Xvfb had not released the
# display number yet, the new one exited, and the app died on `XOpenDisplay`
# twenty seconds later with its frames already named. `xdotool` rather than
# `xdpyinfo`, which the toolbox image does not carry.
for _ in $(seq 1 40); do
  DISPLAY="$DISP" xdotool getdisplaygeometry >/dev/null 2>&1 && break
  sleep 0.5
done
DISPLAY="$DISP" xdotool getdisplaygeometry >/dev/null 2>&1 \
  || { echo "no X server on $DISP"; exit 1; }
# Both the app and the display, on every exit path. Anchored on the full path,
# never a bare name: a bare `baz` also matches the owner's own running copy.
cleanup() {
  [[ -n ${APID:-} ]] && kill "$APID" 2>/dev/null
  local pid
  pid=$(pgrep -x -f "$BIN" || true)
  [[ -n $pid ]] && kill $pid 2>/dev/null
  kill "$XPID" 2>/dev/null
  return 0
}
trap cleanup EXIT INT TERM

launch() {
  env -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS DISPLAY="$DISP" \
      WINIT_UNIX_BACKEND=x11 HOME="$S/home" XDG_DATA_HOME="$S/data" \
      XDG_CONFIG_HOME="$S/config" XDG_CACHE_HOME="$S/cache" \
      XDG_RUNTIME_DIR="$S/run" BAZ_ROOM=closing-time \
      "$BIN" >> "$S/app.log" 2>&1 &
  APID=$!
  WID=""
  for _ in $(seq 1 80); do
    WID=$(timeout 3 xdotool search --sync --onlyvisible --class baz 2>/dev/null | head -1)
    [[ -n $WID ]] && break
    sleep 0.25
  done
  if [[ -z $WID ]]; then echo "NO WINDOW"; tail -30 "$S/app.log"; exit 1; fi
  xdotool windowmove "$WID" 0 0
  xdotool windowsize "$WID" "$W" "$H"
  xdotool windowfocus --sync "$WID"
  # Park the pointer on dead ground *before* the wall first draws. Xvfb starts
  # the pointer at the centre of the screen, which is over a tile, and a tile
  # under the pointer raises its four verbs — so a frame taken after a launch
  # would be a picture of the pointer rather than of the wall.
  xdotool mousemove 140 520
  sleep "${1:-25}"
}

stop() {
  kill "$APID" 2>/dev/null
  for _ in $(seq 1 40); do kill -0 "$APID" 2>/dev/null || break; sleep 0.25; done
  APID=""
}

shot()  { sleep 1.0; magick import -window root -crop "${W}x${H}+0+0" +repage \
            "$OUT/$1.png"; echo "  shot $1"; }
click() { xdotool mousemove "$1" "$2"; sleep 0.4; xdotool click 1; sleep 1.4; }
# Dead ground in the lane, below its rows: the wall's tiles reveal their four
# verbs under the pointer, and a picture of the wall must not be a picture of
# the pointer.
# Two moves, not one: a single jump can land without the toolkit having
# produced a motion event over the ground in between, which leaves the tile the
# pointer *used* to be over still showing its four verbs.
park()  { xdotool mousemove 700 520; sleep 0.4; xdotool mousemove 140 520; sleep 1.2; }

# What the wall is grouped under, straight out of the database — the same three
# facts the shelves are drawn from, printed so a frame can be checked against a
# number rather than against a memory of an earlier frame.
added() {
  sqlite3 "$DB" \
    "SELECT first_seen_ns, count(*) FROM tracks GROUP BY first_seen_ns ORDER BY 1"
}
forgotten() { sqlite3 "$DB" "SELECT count(*) FROM forgotten"; }

# --------------------------------------------------------------- 0 · the scan
echo "== first launch: the library arrives"
launch 30
stop

echo "-- schema version: $(sqlite3 "$DB" 'PRAGMA user_version')"
echo "-- tracks: $(sqlite3 "$DB" 'SELECT count(*) FROM tracks')"

# ------------------------------------------------------------------- 1 · age
# The passage of time, which no press can perform. Albums are dealt round-robin
# into the three bands so every shelf holds several records rather than one.
echo "== aging the library into three ADDED shelves"
NOW=$(date +%s%N)
python3 - "$DB" "$NOW" "${AGES[@]}" <<'AGE'
import sqlite3, sys
db, now = sys.argv[1], int(sys.argv[2])
ages = [int(a) for a in sys.argv[3:]]
conn = sqlite3.connect(db)
albums = sorted({r[0] for r in conn.execute("SELECT album FROM tracks")})
for i, album in enumerate(albums):
    stamp = now - ages[i % len(ages)] * 86_400_000_000_000
    conn.execute("UPDATE tracks SET first_seen_ns = ? WHERE album = ?", (stamp, album))
conn.commit()
print(f"  aged {len(albums)} albums across {len(ages)} bands")
AGE
BEFORE=$(added)
echo "-- ADDED before, (first_seen_ns, tracks):"; echo "$BEFORE" | sed 's/^/     /'

# ------------------------------------------------------- 2 · the wall, before
echo "== the wall, grouped by ADDED"
launch 30
park
shot 01-added-before

# ------------------------------------------ 3 · Settings, and the two presses
echo "== the Settings place, and the folder's own Remove"
click 1528 20      # the gear in the app bar, at its trailing edge
click 352 180      # the Settings place's spine: Library
park
shot 02-settings-library
click 1159 193     # the folder's own Remove
park
shot 03-remove-armed
click 1109 193     # the confirming press: Forget
park
shot 04-folder-forgotten
echo "-- tracks after the forget: $(sqlite3 "$DB" 'SELECT count(*) FROM tracks')"
echo "-- tombstones: $(forgotten)"

# ----------------------------------------------------- 4 · the empty wall
echo "== what the forget did, on the wall"
click 88 165       # the lane's own Library row
park
shot 05-wall-empty

# --------------------------------------------------- 5 · adding the folder back
echo "== adding the folder back, by the well and its word"
click 1528 20      # the gear again
click 352 180      # Library
click 798 216      # the add-a-folder well, one row higher with no folder above it
xdotool type --delay 12 "$FIX"
sleep 0.5
park
shot 06-adding-it-back
click 1087 217     # its word: Add
sleep 40
park
shot 07-settings-rescanned
echo "-- tombstones after the rescan: $(forgotten)"

# ------------------------------------------------------- 6 · the wall, after
click 88 165       # the lane's own Library row
park
shot 08-added-after
stop

AFTER=$(added)
echo "-- ADDED after, (first_seen_ns, tracks):"; echo "$AFTER" | sed 's/^/     /'

# ------------------------------------------- 7 · the sentence at a narrow window
# The confirming sentence grew by half its length, and a string that overflows
# its place is a defect a unit test cannot see. 960 is the narrowest width the
# Settings place has ever been photographed at (`impl/.../24-settings-place-960`).
echo "== the same press at 960 wide"
W=960 H=760
launch 30
xdotool windowsize "$WID" $W $H; sleep 2
click 888 20       # the gear, where 960 puts it
click 249 146      # the spine, which lies down when the place is narrow
click 731 237      # Remove
park
sleep 1.0; magick import -window root -crop "${W}x${H}+0+0" +repage \
  "$OUT/09-remove-armed-960.png"; echo "  shot 09-remove-armed-960"
stop
W=1600 H=900

echo
echo "== the assertion"
if [[ "$BEFORE" == "$AFTER" ]]; then
  echo "  PASS — every record's first-seen is the value it had before the round trip"
else
  echo "  FAIL — first-seen moved:"
  diff <(echo "$BEFORE") <(echo "$AFTER") | sed 's/^/     /'
fi

# The same claim as a picture, because a listener does not read the database.
# The ADDED wall either came back or it did not, and the difference between two
# frames of it is a number.
PIX=$(magick compare -metric AE "$OUT/01-added-before.png" "$OUT/08-added-after.png" \
        null: 2>&1 | tr -d '()' | awk '{print $1}')
if [[ "$PIX" == "0" ]]; then
  echo "  PASS — the wall after the round trip is the wall before it, to the pixel"
else
  echo "  FAIL — the wall differs in $PIX pixels"
fi

echo "== isolation receipt"
grep -m2 '^\[mpris\]' "$S/app.log" || echo "  NO MPRIS LINE — check $S/app.log"
echo "done — $OUT"
