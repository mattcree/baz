#!/usr/bin/env bash
# Render **what the well says and what it offers** — ADR-0036 — headless, on a
# private Xvfb, with all six XDG redirections from docs/DEVELOPMENT.md. Nothing
# touches the owner's session; the run's `[mpris] no session bus` line is the
# receipt that it did not, and this script prints it.
#
# The owner's brief, verbatim: *"how the search works when we're not on the
# library needs to be decided. should it just pop to the library view when you
# start typing? or should it search whatever page you are on? maybe worth
# deciding as both makes sense to me. maybe a little x or esc to clear would
# make sense too"*.
#
# What it shows against the real binary:
#
#   1. **The well at rest**, reading `Search library` — the placeholder naming
#      its subject, in the field's resting width, on the Library and on a
#      playlist page alike. The second of those is the frame the question was
#      about: the field says what it searches while the window is showing
#      `Road Trip`.
#   2. **Mid-query**: the `×` standing where the magnifier was, the match count
#      unmoved in its reserved slot, and the query's own room unchanged.
#   3. **The `×` pressed** — the query gone, the caret out of the field, the
#      wall back. Which is `Esc`'s own function.
#   4. **The decision that was already shipping**: type a letter on the playlist
#      page and the Library comes back under the query. Before and after.
#   5. **The two edges of the swap, cropped and stacked** — the well at rest
#      over the well mid-query, so the claim "nothing moves" is one image.
#   6. **The narrow regime**, where the lane cannot hold the well and the strip
#      takes it back: the same mark, the same swap, at 900.
#
# Build the binary **inside the toolbox** (a host-built release binary links a
# newer glibc than the container has and dies before it draws):
#
#   toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
#     cargo build --release -p baz --features device-output
#   toolbox run -c baz-dev docs/design/composition/tools/mkfixture.sh /tmp/baz-scope-fix
#   FIX=/tmp/baz-scope-fix docs/design/impl/search-scope/capture.sh
set -uo pipefail

REPO=${REPO:-$(git rev-parse --show-toplevel)}
BIN=${BIN:-$REPO/target/tb/release/baz}
FIX=${FIX:-/tmp/baz-scope-fix}
OUT=${OUT:-$REPO/docs/design/impl/search-scope}
DISP=${DISP:-:198}
S=/tmp/baz-scope-scratch

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
density = "balanced"
sidebar_open = true
EOF

# One long list, because the question is about a page with enough rows to want
# filtering. Every third fixture track, which is also what gives the sleeve a
# collage of four distinct records rather than one cover full-bleed.
mkdir -p "$S/data/baz/playlists"
mklist() { # name age-minutes offset n
  { echo "#EXTM3U"
    find "$FIX" -name "*.flac" | sort | awk -v o="$3" 'NR % 3 == o' | head -"$4"
  } > "$S/data/baz/playlists/$1.m3u8"
  touch -d "-$2 minutes" "$S/data/baz/playlists/$1.m3u8"
}
mklist "Road Trip" 5 1 60
mklist "Sunday Morning" 90 2 9

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

resize() { xdotool windowsize "$WID" "$1" "$2"; sleep 1.2; }
stop()  { kill "$APID" 2>/dev/null; wait "$APID" 2>/dev/null; sleep 0.6; }
shot()  { sleep 0.9; magick import -window root -crop "${W}x${H}+0+0" +repage \
            "$OUT/$1.png"; echo "  shot $1"; }
# Park the pointer where it states nothing: the lane's own empty middle, below
# the last row and above the marks.
park()  { xdotool mousemove 40 $((H - 200)); sleep 0.6; }
key()   { xdotool key "$@"; sleep 0.6; }
click() { xdotool mousemove "$1" "$2"; sleep 0.3; xdotool click 1; sleep 0.8; }
hover() { xdotool mousemove "$1" "$2"; sleep 0.8; }
typein(){ xdotool type --delay 45 "$1"; sleep 1.0; }

# The lane's head, at its own geometry: GAP_XL 24 in, the well's 32 px block
# first, a GAP_SM, then the three 40 px destination rows.
WELL_Y=40; HOME_Y=80; LIB_Y=120; NOW_Y=160
# The well's own two edges, in window coordinates: the lane is inset GAP_XL 24,
# so the mark's box is centred on 24 + SIDEBAR_HEAD_GLYPH_X 20 = 44, and the
# count's slot ends at 24 + SIDEBAR_MEASURE 232 − GAP_MD 12 = 244.
MARK_X=44
FIRST_LIST_Y=252   # the head list's first `RECENT` row, under the hairline

# ---------------------------------------------------------------- 1280 × 860
W=1280; H=860
Xvfb "$DISP" -screen 0 1920x1080x24 -nolisten tcp &
XPID=$!
sleep 1

launch $W $H
park

# **1. The well at rest, on the Library.** `Search library` in the placeholder
# lane, the magnifier in the mark's box.
shot 01-well-at-rest-library-1280

# **2. Mid-query.** The `×` where the magnifier was, the count in its reserved
# slot at the other edge, and the query between them.
key slash
typein "an"
park
shot 02-well-mid-query-the-x-and-the-count-1280

# The well alone, at both states, cropped to the lane's own head — the pair
# that shows the swap moves nothing on either edge.
magick "$OUT/01-well-at-rest-library-1280.png" -crop 280x60+0+16 +repage \
  "$OUT/03-mark-box-at-rest-1280.png"
magick "$OUT/02-well-mid-query-the-x-and-the-count-1280.png" \
  -crop 280x60+0+16 +repage "$OUT/04-mark-box-under-a-query-1280.png"
magick "$OUT/03-mark-box-at-rest-1280.png" "$OUT/04-mark-box-under-a-query-1280.png" \
  -append "$OUT/05-the-swap-stacked-1280.png"
echo "  shot 03/04/05 the mark's box, both states, stacked"

# **3. The `×` hovered**, so the control reads as one — the wash under the
# glyph is the transport's own hover paint.
hover "$MARK_X" "$WELL_Y"
shot 06-the-x-hovered-1280

# …and pressed: the query gone, the caret out of the field, the wall back.
click "$MARK_X" "$WELL_Y"
park
shot 07-the-x-pressed-clears-like-esc-1280

# **4. The question's own frame.** Open the longest playlist and read the well:
# it says `Search library` while the window says `Road Trip`, which is the
# whole of §2's answer.
click 120 "$FIRST_LIST_Y"
park
shot 08-a-playlist-page-the-well-names-its-scope-1280

# The well cropped from that page, beside the page's own title — one image of
# the field promising the collection over a page called something else.
magick "$OUT/08-a-playlist-page-the-well-names-its-scope-1280.png" \
  -crop 900x120+0+18 +repage "$OUT/09-scope-word-against-the-page-name-1280.png"
echo "  shot 09-scope-word-against-the-page-name-1280"

# **5. And what typing there does** — the decision that was already shipping:
# one letter and the Library is back under the query.
typein "an"
park
shot 10-typed-on-the-playlist-lands-on-the-library-1280

# Back to rest for the next regime.
key Escape
key Escape
park

# ---------------------------------------------------------------- 900 × 860
# **6. The narrow regime**: below SIDEBAR_FLOOR the lane cannot hold the well
# and the strip takes it back, with the same mark making the same swap.
resize 900 $H
W=900
park
shot 11-strip-well-at-rest-900
typein "an"
park
shot 12-strip-well-the-x-under-a-query-900
key Escape
key Escape
resize 1280 $H
W=1280
stop

kill "$XPID" 2>/dev/null

echo
echo "--- the isolation receipt (docs/DEVELOPMENT.md) ---"
grep -m3 '^\[mpris\]' "$S/app.log" || echo "NO MPRIS LINE — check the log"
echo
echo "--- the well's two edges, measured ---"
"$REPO/docs/design/impl/search-scope/measure.py" "$OUT"
