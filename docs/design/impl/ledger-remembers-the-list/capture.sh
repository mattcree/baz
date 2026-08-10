#!/usr/bin/env bash
# **Play a list, quit, come back** — the owner's own check, rendered against the
# real binary, headless, on a private Xvfb, with all six XDG redirections from
# docs/DEVELOPMENT.md §"Headless UI verification".
#
# The owner: *"when I play a song from a playlist it should only bump the
# recency of that playlist, not the underlying albums please"*. The live half
# of that already worked; what he was seeing was the **cross-quit** half, and
# these two frames are it.
#
# The scenario, in one session and two relaunches:
#
#   1. Play the record `Closing Time`. An ordinary album run — no list.
#   2. Play the list `Road Trip`, which quotes two tracks of `Closing Time`
#      and two of `Paper Mill`. The list's file is given a 30-day-old mtime,
#      so the only thing that can raise it in the lane is the *play*.
#   3. Quit.
#   4. **Relaunch** — `02-after-…`: the lane folded out of a v1.1 ledger.
#   5. Relaunch again over the **same ledger with its `# baz run` markers
#      deleted** — `01-before-…`. That is byte-for-byte the file an older baz
#      would have written, so the "before" frame is this build reading a v1
#      ledger rather than a different build reading anything.
#
# Step 5 is doing double duty on purpose: it is the before/after contrast
# *and* the proof that an old ledger still folds exactly as it did.
#
# Nothing touches the owner's session and nothing is audible: the scratch HOME
# routes ALSA's default PCM to null and every fixture sample is a zero.
# `BAZ_DEVICE_TESTS` is unset. The run's `[mpris] no session bus` line is the
# receipt that the isolation held, and this script prints it.
#
#   toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
#     cargo build --release -p baz --features device-output
#   toolbox run -c baz-dev docs/design/composition/tools/mkfixture.sh /tmp/baz-ledger-fix
#   toolbox run -c baz-dev docs/design/impl/ledger-remembers-the-list/capture.sh
set -uo pipefail

REPO=${REPO:-$(git rev-parse --show-toplevel)}
BIN=${BIN:-$REPO/target/tb/release/baz}
FIX=${FIX:-/tmp/baz-ledger-fix}
OUT=${OUT:-$REPO/docs/design/impl/ledger-remembers-the-list}
DISP=${DISP:-:194}
S=/tmp/baz-lrl-scratch
W=1400
H=1000

ALBUM="$FIX/01 - Halvard Sten - Closing Time"
OTHER="$FIX/02 - The Ardent - Paper Mill"

mkdir -p "$OUT"
rm -rf "$S"; mkdir -p "$S"/{home,data,config,cache,run}; chmod 700 "$S/run"
cat > "$S/home/.asoundrc" <<'EOF'
pcm.!default { type null }
ctl.!default { type null }
EOF
mkdir -p "$S/config/baz" "$S/data/baz/playlists"
printf 'music_dirs = [\n    "%s",\n]\ngroup_key = "artist"\nsidebar_open = true\n' \
  "$FIX" > "$S/config/baz/config.toml"

# The list: two tracks from each of two records, so a run of it quotes both.
LIST="$S/data/baz/playlists/Road Trip.m3u8"
{ echo "#EXTM3U"; ls "$ALBUM"/*.flac | head -2; ls "$OTHER"/*.flac | head -2; } > "$LIST"
# Old, so that the *play* is the only thing that can move it in the lane.
touch -d '30 days ago' "$LIST"

LEDGER="$S/data/baz/history.tsv"

launch() { # data_dir log
  env -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS DISPLAY="$DISP" \
      WINIT_UNIX_BACKEND=x11 HOME="$S/home" XDG_DATA_HOME="$1" \
      XDG_CONFIG_HOME="$S/config" XDG_CACHE_HOME="$S/cache" \
      XDG_RUNTIME_DIR="$S/run" "$BIN" >> "$2" 2>&1 &
  APID=$!
  WID=""
  for _ in $(seq 1 40); do
    WID=$(DISPLAY=$DISP timeout 3 xdotool search --sync --onlyvisible --class baz 2>/dev/null | head -1)
    [[ -n $WID ]] && break
    sleep 0.25
  done
  if [[ -z $WID ]]; then echo "NO WINDOW"; tail -30 "$2"; kill "$APID" "$XPID" 2>/dev/null; exit 1; fi
  export DISPLAY=$DISP
  xdotool windowmove "$WID" 0 0
  xdotool windowsize "$WID" "$W" "$H"
  xdotool windowfocus --sync "$WID"
  sleep 5
}
quit()  { kill "$APID" 2>/dev/null; wait "$APID" 2>/dev/null; sleep 1; }
shot()  { sleep 1.2; magick import -window root "$OUT/$1.png"; echo "  shot $1"; }
click() { xdotool mousemove "$1" "$2"; sleep 0.4; xdotool click 1; sleep 0.8; }
park()  { xdotool mousemove $((W - 6)) 300; sleep 0.4; }

# Wait until the ledger holds `$1` play lines — the run has actually been
# recorded, rather than a sleep long enough to *probably* have been.
plays() { grep -c '^2' "$LEDGER" 2>/dev/null || echo 0; }
await() {
  for _ in $(seq 1 120); do
    [[ $(plays) -ge $1 ]] && return 0
    sleep 0.5
  done
  echo "  !! only $(plays) plays after waiting for $1"
}

Xvfb "$DISP" -screen 0 ${W}x${H}x24 -nolisten tcp & XPID=$!
sleep 1

# ---- 1. the session that does the listening --------------------------------
echo "session 1: an album, then the list"
launch "$S/data" "$S/app-1.log"
# The record, found by name so the click does not depend on where the wall
# happens to have hung it.
click 140 40
xdotool type --delay 40 "Closing Time"; sleep 1.5
xdotool mousemove 461 270; sleep 0.4; xdotool click --repeat 2 --delay 120 1
await 1
echo "  the album run is in the ledger"
xdotool key Escape; sleep 1
# The list, from the lane — which now holds the record above it.
click 140 317
park; shot "00-the-list-before-it-is-played"
click 480 425
# All four of the list's tracks, so the run quotes *both* records — which is
# what makes the two frames differ by two rows rather than by one.
await 6
echo "  the list's run is in the ledger"
park; shot "01-the-live-half-in-the-same-session"
quit

# ---- 2. the relaunch, over the ledger as written ---------------------------
echo "session 2: the relaunch"
launch "$S/data" "$S/app-2.log"
park; shot "03-after-the-list-is-at-the-head"
quit

# ---- 3. the same ledger, with the markers deleted --------------------------
# Byte for byte the file an older baz would have written, so this is the
# *before* frame and the old-ledger compatibility check in one run.
echo "session 3: the same ledger, markers deleted"
cp -a "$S/data" "$S/data-v1"
grep -v '^# baz run' "$S/data/baz/history.tsv" > "$S/data-v1/baz/history.tsv"
launch "$S/data-v1" "$S/app-3.log"
park; shot "02-before-the-records-jump-the-list"
quit

kill "$XPID" 2>/dev/null; wait "$XPID" 2>/dev/null

echo
echo "--- the isolation receipt ---"
grep -hE "^\[mpris\]" "$S"/app-*.log || echo "(no mpris line — look at $S/app-1.log)"
echo
echo "--- the ledger the frames were folded from ---"
grep -vE '^# (baz play|Fields|started_utc|outcome|listened_ms|track_ms|path|A line|after it|the run|Lines|or delete|a line)' "$LEDGER"
echo
echo "--- what each session credited ---"
grep -hE "^\[history\]" "$S"/app-2.log "$S"/app-3.log
echo "done — $OUT"
