#!/usr/bin/env bash
# Render the two walls the owner asked to have side by side — headless, on a
# private Xvfb, with all six XDG redirections from docs/DEVELOPMENT.md. Nothing
# touches the owner's session; the run's `[mpris] no session bus` line is the
# receipt that it did not, and this script prints it.
#
# The change (ADR-0035's third amendment) is one key restored and one word
# added to the strip:
#
#   1. **`A–Z` is a group key again**, breaking records on the album artist's
#      initial — 27 letter shelves and the two anonymous ends. The owner:
#      *"also, we have removed the a-z option from grouping? that feels like it
#      should go back and honestly it's the first option, followed by artist"*.
#   2. **It is first in the row**: `A–Z · ARTIST · YEAR · GENRE · ADDED ·
#      PLAYED`, and the number row is `1`–`6`.
#   3. **Its code is `"alphabet"`, not `"artist"`.** `GroupKey::code` is on-disk
#      config and may never be repurposed — and `"artist"` already was, once,
#      silently. The frames carry that as a migration: the config below is the
#      document a baz from *before* ADR-0035 wrote, and `group_key = "artist"`
#      still resolves to the artist grouping rather than being quietly handed
#      back to the letters.
#
# What the frames have to earn is that the two walls are *different at a
# glance* while being the same order underneath, which is the whole of the
# decision. So each is shot at both sizes with the pointer parked, and each has
# its rail cropped out beside it: the same 27 letters over 10 letter shelves
# and over 13 artist shelves, with `S` landing on Sonja Aalto either way.
#
# The last pair are the *strip's* frames, because the sixth word costs 46 px:
# the split is 824 rather than 778, so the single-line-with-well band is
# 824…904. `10-strip-single-line-with-well-920.png` is that band photographed
# above its new left edge, and the 696 frame is the window's own minimum, which
# did **not** have to move.
#
# The fixture is `mkfixture.sh`'s own, unmodified — it files two records under
# each of thirteen artists whose initials collide on `S` three times and on `A`
# twice, which is exactly the contrast the two walls exist to show.
#
# Build the binary **inside the toolbox** (a host-built release binary links a
# newer glibc than the container has and dies before it draws):
#
#   toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
#     cargo build --release -p baz --features device-output
#   toolbox run -c baz-dev docs/design/composition/tools/mkfixture.sh /tmp/baz-az-fix
#   toolbox run -c baz-dev docs/design/impl/az-and-artist/capture.sh
set -uo pipefail

REPO=${REPO:-$(git rev-parse --show-toplevel)}
BIN=${BIN:-$REPO/target/tb/release/baz}
FIX=${FIX:-/tmp/baz-az-fix}
OUT=${OUT:-$REPO/docs/design/impl/az-and-artist}
DISP=${DISP:-:197}
S=/tmp/baz-az-scratch

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

if [[ ! -d $FIX ]]; then
  echo "no fixture at $FIX — run mkfixture.sh first" >&2; exit 1
fi

# **The config a baz from before ADR-0035 wrote**, verbatim — the retired
# `wall_subject` key included. It is the code decision, photographed:
# `group_key = "artist"` keeps the meaning it has had since ADR-0035 and is
# *not* handed back to the restored letter key, which spells itself
# `"alphabet"`. So this launch opens on the ARTIST wall, and `1` is what
# reaches the letters.
mkdir -p "$S/config/baz"
cat > "$S/config/baz/config.toml" <<EOF
music_dirs = [
    "$FIX",
]
group_key = "artist"
wall_subject = "artists"
density = "balanced"
sidebar_open = true
EOF

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
  sleep 5   # let the launch scan land
}

stop()  { kill "$APID" 2>/dev/null; wait "$APID" 2>/dev/null; sleep 0.6; }
shot()  { sleep 1.2; magick import -window root -crop "${W}x${H}+0+0" +repage \
            "$OUT/$1.png"; echo "  shot $1"; }
crop()  { magick "$OUT/$1.png" -crop "$3" +repage "$OUT/$2.png"; echo "  shot $2"; }
# Park the pointer where it states nothing: the lane's own empty middle, below
# the last row and above the marks.
park()  { xdotool mousemove 40 $((H - 220)); sleep 0.7; }
key()   { xdotool key "$@"; sleep 0.9; }

Xvfb "$DISP" -screen 0 1920x1080x24 -nolisten tcp &
XPID=$!
sleep 1

# ---------------------------------------------------------------- 1280 × 860
W=1280; H=860
launch $W $H

# **The strip, with six words.** `A–Z` first and current is one press away; the
# frame here is the row itself, opened on ARTIST because that is what the
# pre-existing config resolves to.
park
shot 00-artist-wall-1280
crop 00-artist-wall-1280 01-arrangement-row-1280 "470x50+300+0"

# **The wall under `A–Z`.** One press of `1`. Ten letter shelves for
# twenty-five records, with `S` holding three artists' worth in one flowing
# grid — the density the owner asked to have back.
key 1
park
shot 02-az-wall-1280
crop 02-az-wall-1280 03-az-headers-1280 "700x470+300+40"
crop 02-az-wall-1280 04-az-rail-1280 "70x600+1210+40"

# **The wall under `ARTIST`.** `2`. The same twenty-five records in the same
# order, broken finer: one shelf per person, each header a door to their page.
# Read against 02 this is the whole decision — two densities of one order.
key 2
park
shot 05-artist-wall-1280
crop 05-artist-wall-1280 06-artist-headers-1280 "700x470+300+40"
crop 05-artist-wall-1280 07-artist-rail-1280 "70x600+1210+40"

# **The rails, side by side.** 27 slots either way, because the rail is a pure
# function of the headers and both keys' headers file under `Initial`. The
# letters do not move; only the shelf each one jumps to does.
magick \( "$OUT/04-az-rail-1280.png" \) \( "$OUT/07-artist-rail-1280.png" \) \
  +append "$OUT/08-the-two-rails-1280.png"
echo "  shot 08-the-two-rails-1280"

# **A letter lands on the first shelf filed under it, under either key.** `S`
# is one press on the rail: under A–Z it is the `S` shelf itself, under ARTIST
# it is Sonja Aalto, the first of the three artists that shelf held.
key 1
xdotool mousemove 1237 493; sleep 0.4; xdotool click 1; sleep 1.4
park
shot 09-az-rail-jumped-to-s-1280
key 2
xdotool mousemove 1237 493; sleep 0.4; xdotool click 1; sleep 1.4
park
shot 10-artist-rail-jumped-to-s-1280
key 1
stop

# ------------------------------------------------- the strip, at its own band
# **The single-line-with-well band.** Below `SIDEBAR_FLOOR` the lane is a rail
# that cannot open, so the strip carries the well; the split is now 824, so a
# window of 920 leaves the strip exactly that and is the narrowest window at
# which the strip is still one line. It was 874 while the row had five words.
W=920; H=700
launch $W $H
park
shot 11-strip-single-line-with-well-920
crop 11-strip-single-line-with-well-920 12-strip-band-920 "920x50+0+0"
stop

# **And at the window's own minimum** — `TOP_BAR_FLOOR` 600 plus the lane's
# rail 96 — which did **not** have to move for the sixth word: the library line
# is 552 against a floor of 600. Two lines here, the frame's furniture above
# and the library's six words with its one act below; nothing hides and nothing
# overflows.
W=696; H=620
launch $W $H
park
shot 13-strip-at-the-window-floor-696
crop 13-strip-at-the-window-floor-696 14-strip-two-lines-696 "696x95+0+0"
stop

# --------------------------------------------------------------- 1920 × 1080
W=1920; H=1080
launch $W $H
key 1
park
shot 20-az-wall-1920
crop 20-az-wall-1920 21-arrangement-row-1920 "470x50+300+0"
key 2
park
shot 22-artist-wall-1920
key 1
stop

kill "$XPID" 2>/dev/null

echo
echo "--- the isolation receipt (docs/DEVELOPMENT.md) ---"
grep -m3 '^\[mpris\]' "$S/app.log" || echo "NO MPRIS LINE — check the log"
echo
echo "--- the config baz wrote back ---"
# `wall_subject` was read by nothing and is gone. `group_key` is the key the
# last press left active, written as its own code — which is how `"alphabet"`
# reaches the disk without `"artist"` ever changing meaning again.
cat "$S/config/baz/config.toml"
echo
echo "--- what the wall reported ---"
grep -m4 '^\[startup\]' "$S/app.log"
