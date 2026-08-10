#!/usr/bin/env bash
# Render the wall's second subject — headless, on a private Xvfb, with all six
# XDG redirections from docs/DEVELOPMENT.md. Nothing touches the owner's
# session; the run's `[mpris] no session bus` line is the receipt that it did
# not, and this script prints it.
#
# The change (ADR-0035) is two words and one wall:
#
#   1. **The first group key's word is `A–Z`**, not `ARTIST`. It breaks records
#      on the album artist's initial, and the product also has an Artist
#      *place*, so the word now names what the key produces.
#   2. **`ARTISTS` is the sixth word in the same row** — the wall's *subject*,
#      held beside the arrangement rather than among it.
#   3. **The artists wall**: one tile per person, shelved by initial, wearing
#      the collage a playlist's sleeve wears, indexed by the alphabet rail
#      verbatim.
#
# The frames also carry the two claims that are easy to assert and hard to
# believe without a picture: the readouts follow the subject (`13 / 16`
# artists, not `25` albums), and the arrangement survives a trip through the
# artists and back (leave on YEAR, visit ARTISTS, press YEAR — the decades are
# where they were).
#
# The last pair are the *strip's* frames, because the sixth word cost the strip
# 54 px and the costing kept in `docs/BACKLOG.md` predicted that would delete
# the single-line-with-well band outright. It did not: `Pull` and `Shuffle`
# left the acts cluster in between and paid for the word twice over, so the
# split is 832 against a widest-strip-with-well of 904 and the band is real.
# `10-strip-single-line-with-well-928.png` is that band, photographed.
#
# The fixture is `mkfixture.sh`'s own with **four extra records for one
# artist**, because the collage's headline rule — four or more distinct
# records make a 2 × 2 — cannot be shown by a fixture whose artists all hold
# two.
#
# Build the binary **inside the toolbox** (a host-built release binary links a
# newer glibc than the container has and dies before it draws):
#
#   toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
#     cargo build --release -p baz --features device-output
#   toolbox run -c baz-dev docs/design/composition/tools/mkfixture.sh /tmp/baz-artists-fix
#   toolbox run -c baz-dev docs/design/impl/artists-wall/capture.sh
set -uo pipefail

REPO=${REPO:-$(git rev-parse --show-toplevel)}
BIN=${BIN:-$REPO/target/tb/release/baz}
FIX=${FIX:-/tmp/baz-artists-fix}
OUT=${OUT:-$REPO/docs/design/impl/artists-wall}
DISP=${DISP:-:197}
S=/tmp/baz-artists-scratch

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

# **Give one artist enough records to be a collage.** `mkfixture.sh` files two
# records under each of twelve artists, which draws the one-to-three rule (the
# first record's sleeve, full bleed) and never the 2 × 2. Four of Kesh's
# label-mates are re-filed under Kesh, keeping their own covers, so the wall
# holds one six-record artist among the twos and the frame shows both rules at
# once.
if [[ ! -d $FIX ]]; then
  echo "no fixture at $FIX — run mkfixture.sh first" >&2; exit 1
fi
refile() { # dir new-artist
  local dir="$FIX/$1" who="$2"
  [[ -d $dir ]] || return 0
  for f in "$dir"/*.flac; do
    metaflac --remove-tag=ARTIST --remove-tag=ALBUMARTIST \
             --set-tag="ARTIST=$who" --set-tag="ALBUMARTIST=$who" "$f"
  done
}
refile "03 - Nils Odden - Cyan Handbook"  "Kesh"
refile "15 - Nils Odden - Green Line"     "Kesh"
refile "11 - Corvin - Red Shift"          "Kesh"
refile "23 - Corvin - Teal"               "Kesh"

mkdir -p "$S/config/baz"
cat > "$S/config/baz/config.toml" <<EOF
music_dirs = [
    "$FIX",
]
group_key = "artist"
wall_subject = "records"
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
key()   { xdotool key "$@"; sleep 0.7; }
typein(){ xdotool type --delay 45 "$1"; sleep 1.2; }

Xvfb "$DISP" -screen 0 1920x1080x24 -nolisten tcp &
XPID=$!
sleep 1

# ---------------------------------------------------------------- 1280 × 860
W=1280; H=860
launch $W $H

# **Before**: the wall of records, under the renamed first word. `A–Z` leads
# the row and `ARTISTS` closes it, and neither is what the shipped build said.
park
shot 00-records-wall-1280
crop 00-records-wall-1280 01-arrangement-row-1280 "420x50+300+0"

# **The artists wall, from the word itself** — the pointer route, so the frame
# is of a control being used rather than of a key being pressed. The row hangs
# from `HANG` inside the *strip*, and the strip begins after the lane's 280,
# so the sixth word sits at roughly x = 650 at both window widths (the lane's
# width does not change with the window).
click_artists() { xdotool mousemove 650 24; sleep 0.4; xdotool click 1; sleep 1.5; }
click_artists
park
shot 02-artists-wall-1280
crop 02-artists-wall-1280 03-arrangement-row-artists-1280 "420x50+300+0"

# **The collage's two rules, on one screen.** The alphabet rail indexes this
# wall verbatim — it is the same `rail::artist` the records wall gets — so `K`
# is one press, and `K` is where the six-record artist is. Beside them the
# two-record artists take the full-bleed single, which is the same rule a
# playlist of three records follows.
xdotool mousemove 1237 337; sleep 0.4; xdotool click 1; sleep 1.2
park
shot 04-artist-collage-1280
crop 04-artist-collage-1280 05-artist-tiles-1280 "620x420+300+60"

# **Mid-query, and the readout follows the subject.** `an` narrows records; the
# artists standing are the ones with a record that survived, and the well's
# figure counts *them*.
key slash
typein "an"
park
shot 06-artists-mid-query-1280
crop 06-artists-mid-query-1280 07-artists-match-count-1280 "280x120+0+0"
key Escape
key Escape

# **The round trip.** Arrange the records by YEAR, go to the artists, come
# back to YEAR: the decades are where they were left, because the subject is
# held beside the arrangement rather than inside it.
key 2
park
shot 08-year-wall-before-1280
key 6
sleep 1
key 2
park
shot 09-year-wall-after-the-artists-1280
key 1
stop

# ------------------------------------------------- the strip, at its own band
# **The single-line-with-well band, photographed.** Below `SIDEBAR_FLOOR` the
# lane is a rail that cannot open, so the strip carries the well; a window of
# 928 leaves the strip exactly `TOP_BAR_SPLIT` = 832, the widest window at
# which the strip is still one line. The costing in `docs/BACKLOG.md` said
# this band would not exist after a sixth word.
W=928; H=700
launch $W $H
key 6
park
shot 10-strip-single-line-with-well-928
crop 10-strip-single-line-with-well-928 11-strip-band-928 "928x50+0+0"
stop

# **And at the window's own minimum** — `TOP_BAR_FLOOR` 600 plus the lane's
# rail 96 — where the strip is two lines: the frame's furniture above, the
# library's six words and its one act below. Nothing hides and nothing
# overflows; that is what the 40 px of slack under the floor is for.
W=696; H=620
launch $W $H
key 6
park
shot 12-strip-at-the-window-floor-696
crop 12-strip-at-the-window-floor-696 13-strip-two-lines-696 "696x95+0+0"
stop

# --------------------------------------------------------------- 1920 × 1080
W=1920; H=1080
launch $W $H
# **The subject is remembered**, so this window opens on the artists — the two
# strip runs above left it there. Put the records back first, or `20` would be
# a second copy of `21` rather than the frame it is labelled as. (That the
# config carried the subject across three launches is itself the persistence
# claim, arrived at by accident and kept on purpose.)
key 1
park
shot 20-records-wall-1920
click_artists
park
shot 21-artists-wall-1920
key slash
typein "an"
park
shot 22-artists-mid-query-1920
key Escape
key Escape
stop

kill "$XPID" 2>/dev/null

echo
echo "--- the isolation receipt (docs/DEVELOPMENT.md) ---"
grep -m3 '^\[mpris\]' "$S/app.log" || echo "NO MPRIS LINE — check the log"
echo
echo "--- the round trip, measured ---"
# **Pixel-identical, or the subject is not held beside the arrangement.**
# `08` is a YEAR wall; `09` is the same YEAR wall after a trip through the
# artists and back. Any difference at all would mean the sixth word had moved
# something that belongs to the five.
printf '  YEAR before vs after the artists: '
magick compare -metric AE "$OUT/08-year-wall-before-1280.png" \
                          "$OUT/09-year-wall-after-the-artists-1280.png" null: 2>&1
echo " differing pixels (0 is the claim)"
echo
echo "--- what the wall reported ---"
grep -m4 '^\[startup\]' "$S/app.log"
