#!/usr/bin/env bash
# Render the Songs section (design doc 09 §5, step 3 of §13) headless, on a
# private Xvfb, with all six XDG redirections from docs/DEVELOPMENT.md.
# Nothing touches the owner's session; the run's `[mpris] no session bus`
# line is the receipt that it did not, and this script prints it.
#
# What it shows against the real binary, at 1280×860 and 1920×1080:
#
#   1. **Two sections, separate**: a query with matching tracks renders a
#      ranked `Songs` section — up to eight rows — above an `Albums` rule
#      and the wall, filtered as today, both on the wall's own block ruler.
#   2. **Enter needle-drops the top song**: the record queued whole, the
#      lamp dot following `TrackStarted` into the section's row.
#   3. **An album-name query** puts that record's opening track on top
#      (ADR-0021's field ranking), so the sound of Enter is unchanged there.
#   4. **A narrow query** leaves a short section — the section is the ranked
#      head, never padded.
#   5. **No matching tracks**: the section is absent (not empty) and the
#      wall's own empty state stands alone.
#
# Build the binary **inside the toolbox** (a host-built release binary links
# a newer glibc than the container has and dies before it draws):
#
#   toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
#     cargo build --release -p baz --features device-output
#   toolbox run -c baz-dev docs/design/composition/tools/mkfixture.sh /tmp/baz-songs-fix
#   toolbox run -c baz-dev docs/design/impl/songs-search/capture.sh
set -uo pipefail

REPO=${REPO:-$(git rev-parse --show-toplevel)}
BIN=${BIN:-$REPO/target/tb/release/baz}
FIX=${FIX:-/tmp/baz-songs-fix}
OUT=${OUT:-$REPO/docs/design/impl/songs-search}
DISP=${DISP:-:197}
S=/tmp/baz-songs-scratch

mkdir -p "$OUT"
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
group_key = "artist"
EOF

Xvfb "$DISP" -screen 0 1280x860x24 -nolisten tcp &
XPID=$!
sleep 1

launch() { # W H
  env -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS DISPLAY="$DISP" \
      WINIT_UNIX_BACKEND=x11 HOME="$S/home" XDG_DATA_HOME="$S/data" \
      XDG_CONFIG_HOME="$S/config" XDG_CACHE_HOME="$S/cache" \
      XDG_RUNTIME_DIR="$S/run" BAZ_ROOM=closing-time \
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
  xdotool windowsize "$WID" "$1" "$2"
  xdotool windowfocus --sync "$WID"
  sleep 4   # let the launch scan land
}

shot()  { sleep 0.9; magick import -window root "$OUT/$1.png"; echo "  shot $1"; }
key()   { xdotool key "$@"; sleep 0.5; }
qtype() { xdotool type --delay 90 "$1"; sleep 0.9; }
clearq(){ key Escape; key Escape; }  # blur the well, then peel the query
park()  { xdotool mousemove $((W - 6)) 300; }

# ---- 1280 × 860 -----------------------------------------------------------
W=1280; H=860
launch $W $H

# 1. The wall at rest — no query, no section, the composition untouched.
park; shot 01-wall-before-1280x860
# 2. Type-anywhere, mixed results: the first keystroke lands in the well and
#    the section grows with the wall's filter — eight ranked track rows,
#    every row `title  artist · record  duration`, the wall below under its
#    Albums rule.
qtype "night"
park; shot 02-songs-mixed-1280x860
# 3. Enter: the top-ranked song sounds — its record queued whole, the cursor
#    on the song, the dot following TrackStarted into the first row.
key Return
sleep 1.2
park; shot 03-songs-enter-playing-1280x860
# 4. An album-name query: ADR-0021's field ranking puts that record's own
#    tracks in the section, opening track on top.
clearq
qtype "orbits"
park; shot 04-songs-album-query-1280x860
# 5. A narrow query: the section is the ranked head, never padded — two
#    matching songs are two rows.
clearq
qtype "nightwatch 9"
park; shot 05-songs-narrow-1280x860
# 6. No matching tracks: no section, the wall's own empty state.
clearq
qtype "zzzq"
park; shot 06-songs-none-1280x860
clearq

kill "$APID" 2>/dev/null; wait "$APID" 2>/dev/null
kill "$XPID" 2>/dev/null; wait "$XPID" 2>/dev/null

# ---- 1920 × 1080 ----------------------------------------------------------
W=1920; H=1080
Xvfb "$DISP" -screen 0 ${W}x${H}x24 -nolisten tcp &
XPID=$!
sleep 1
launch $W $H
qtype "night"
park; shot 07-songs-mixed-1920x1080
clearq
qtype "nightwatch 9"
park; shot 08-songs-narrow-1920x1080
clearq
qtype "zzzq"
park; shot 09-songs-none-1920x1080
clearq
kill "$APID" 2>/dev/null; wait "$APID" 2>/dev/null
kill "$XPID" 2>/dev/null; wait "$XPID" 2>/dev/null

echo "--- the isolation receipt ---"
grep -E "^\[mpris\]|^\[startup\] room" "$S/app.log" || echo "(no mpris line — look at $S/app.log)"
echo "done — $OUT"
