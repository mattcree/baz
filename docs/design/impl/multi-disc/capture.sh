#!/usr/bin/env bash
# Render the multi-disc shelf — headless, on a private Xvfb, with all six XDG
# redirections from docs/DEVELOPMENT.md. Nothing touches the owner's session;
# the run's `[mpris] no session bus` line is the receipt, and this script
# prints it.
#
# The owner: *"it would be good if multi CD albums were a single item"*. The
# frames are the answer to that, and the honest half of the answer is that
# three of the four shapes were already single items — so the wall is captured
# **before and after**, over one fixture holding all of them:
#
#   1. **The wall before.** Twelve tiles. `Bitches Brew (Disc 1)` sits beside
#      `(Disc 2)`, `Tusk CD1` beside `CD2`, `The Beatles [Disc 1]` beside
#      `[Disc 2]`, `Spirit of Eden` beside `Spirit of Eden - Disc 2` — while
#      Prince, The Clash and Genesis are already one tile each, because their
#      discs share an `ALBUM` tag.
#   2. **The wall after.** Eight tiles. The four shattered records are one
#      record each, under the name without the marker. `Wu-Tang Forever CD1`
#      is deliberately still called that: no sibling, no rename (ADR-0038 §3).
#   3. **A merged record's page**, `Bitches Brew` — one sleeve, eight tracks,
#      `DISC 1` and `DISC 2` breaking the run where the two discs meet.
#   4. **The record whose tagger never wrote `DISCNUMBER`**, `Tusk` — the
#      breaks come from the `CD1`/`CD2` marker in the title, and the record
#      also carries **two editions**, which is ADR-0007's axis meeting this
#      one: one record, two editions, two discs each.
#   5. **The asymmetric rip**, `Spirit of Eden` — half of it names a disc and
#      half does not. The unnamed half leads with no header over it, because
#      no header was earned, and `DISC 2` breaks where the marked half starts.
#
# Build both binaries **inside the toolbox** (a host-built release binary links
# a newer glibc than the container has and dies before it draws):
#
#   toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
#     cargo build --release -p baz --features device-output
#   docs/design/impl/multi-disc/mkfixture.sh /tmp/baz-multidisc-fixture
#   toolbox run -c baz-dev docs/design/impl/multi-disc/capture.sh
#
# BEFORE is optional: a binary built from the commit this branch left, used for
# frame 1 only. Without it that frame is skipped and the rest still render.
set -uo pipefail

REPO=${REPO:-$(git rev-parse --show-toplevel)}
BIN=${BIN:-$REPO/target/tb/release/baz}
BEFORE=${BEFORE:-}
FIX=${FIX:-/tmp/baz-multidisc-fixture}
OUT=${OUT:-$REPO/docs/design/impl/multi-disc}
DISP=${DISP:-:197}
S=/tmp/baz-multidisc-scratch

mkdir -p "$OUT"

# A clean scratch per launch: the library database must not carry a merge
# decision made by the *other* binary into this one's frame.
scratch() {
  rm -rf "$S"
  mkdir -p "$S"/{home,data,config,cache,run}
  chmod 700 "$S/run"
  # Two independent guarantees that nothing is audible: the sink discards every
  # sample, and the fixture's samples are all zero (docs/DEVELOPMENT.md).
  cat > "$S/home/.asoundrc" <<'EOF'
pcm.!default { type null }
ctl.!default { type null }
EOF
  mkdir -p "$S/config/baz"
  cat > "$S/config/baz/config.toml" <<EOF
music_dirs = [
    "$FIX",
]
# ADDED puts the whole fixture in one shelf, so the count the ask is about —
# how many tiles this collection *is* — is one frame and not a scroll. Dense
# and no lane for the same reason.
group_key = "added"
density = "dense"
sidebar_open = false
EOF
}

launch() { # BINARY W H
  env -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS DISPLAY="$DISP" \
      WINIT_UNIX_BACKEND=x11 HOME="$S/home" XDG_DATA_HOME="$S/data" \
      XDG_CONFIG_HOME="$S/config" XDG_CACHE_HOME="$S/cache" \
      XDG_RUNTIME_DIR="$S/run" BAZ_ROOM=closing-time \
      "$1" >> "$S/app.log" 2>&1 &
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
  xdotool windowsize "$WID" "$2" "$3"
  xdotool windowfocus --sync "$WID"
  sleep 9   # let the launch scan land, and every thumbnail decode
}

stop()  { kill "$APID" 2>/dev/null; wait "$APID" 2>/dev/null; sleep 0.6; }
shot()  { sleep 1.6; magick import -window root -crop "${W}x${H}+0+0" +repage \
            "$OUT/$1.png"; echo "  shot $1"; }
park()  { xdotool mousemove 40 $((H - 200)); sleep 0.6; }
hover() { xdotool mousemove "$1" "$2"; sleep 0.8; }
click() { xdotool mousemove "$1" "$2"; sleep 0.3; xdotool click 1; sleep 1.2; }
key()   { xdotool key "$@"; sleep 1.0; }
# A tile opens by hovering it and pressing its *lower* half: the hover options
# cover the top, and a press landing in the same instant the pointer arrives is
# swallowed (the pattern `lane-and-home/capture.sh` uses).
open_tile() { hover "$1" "$2"; click "$1" $(( $2 + 60 )); park; }

W=1360; H=860
Xvfb "$DISP" -screen 0 1920x1080x24 -nolisten tcp &
XPID=$!
sleep 1

# 1. The wall as it was.
if [[ -n $BEFORE ]]; then
  scratch
  launch "$BEFORE" $W $H
  park
  shot 01-the-wall-before
  stop
else
  echo "  (no BEFORE binary; skipping 01-the-wall-before)"
fi

# 2..5 — the wall as it is, and three of its merged records.
scratch
launch "$BIN" $W $H
park
shot 02-the-wall-after

# The wall is grouped by artist, ascending: Fleetwood Mac, Genesis, Miles
# Davis, Prince, Talk Talk, The Beatles, The Clash, Wu-Tang Clan.
# Tile centres on the dense ADDED wall, row one: Tusk, The Lamb…, Bitches
# Brew, Sign o' the Times, Spirit of Eden.
open_tile 693 205   # Miles Davis — Bitches Brew
shot 03-bitches-brew-breaks-into-two-discs
key Escape          # back up to the wall
park

open_tile 208 205   # Fleetwood Mac — Tusk (two editions, two discs)
shot 04-tusk-two-editions-two-discs
key Escape
park

open_tile 1155 205  # Talk Talk — Spirit of Eden (the asymmetric rip)
shot 05-spirit-of-eden-one-disc-unnumbered
stop

kill "$XPID" 2>/dev/null

echo
echo "--- the isolation receipt (docs/DEVELOPMENT.md) ---"
grep -m3 '^\[mpris\]' "$S/app.log" || echo "NO MPRIS LINE — check the log"
