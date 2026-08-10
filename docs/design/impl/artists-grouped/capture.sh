#!/usr/bin/env bash
# Render the wall grouped by artist — headless, on a private Xvfb, with all six
# XDG redirections from docs/DEVELOPMENT.md. Nothing touches the owner's
# session; the run's `[mpris] no session bus` line is the receipt that it did
# not, and this script prints it.
#
# The change (ADR-0035, as amended) is one key and one deletion:
#
#   1. **`ARTIST` shelves one artist per shelf**, headed by their name, in the
#      library's own order — unknowns first, names case-folded, unnamed
#      compilations last — with each artist's records alphabetical under them.
#      It broke records on the artist's *initial* before, which is the owner's
#      finding: *"artists should be grouping stuff by artist not just
#      alphabetically"*.
#   2. **The header is the door to the Artist place**, in the record page
#      breadcrumb's own paint. That is how the artist tiles' press survives.
#   3. **The sixth word is gone**, and so is `A–Z`. Grouping albums by their
#      artist satisfies ADR-0019 §1 exactly, so it is an ordinary group key,
#      and the strip is five words again.
#
# The frames also carry the two claims that are easy to assert and hard to
# believe without a picture: the **rail is still the alphabet** over a wall
# with far more headers than letters (a letter lands on the first artist under
# it), and the shelf a six-record artist gets is the wall's ordinary shelf with
# two rows in it.
#
# The last pair are the *strip's* frames, because the sixth word cost the strip
# 54 px and this returns them: the split is 778 again, against a
# widest-strip-with-well of 904, so the single-line-with-well band is 778…904
# rather than 832…904. `10-strip-single-line-with-well-874.png` is that band's
# new left edge, photographed.
#
# The fixture is `mkfixture.sh`'s own with **four records re-filed onto one
# artist**, because a shelf that needs two rows cannot be shown by a fixture
# whose artists all hold two records.
#
# Build the binary **inside the toolbox** (a host-built release binary links a
# newer glibc than the container has and dies before it draws):
#
#   toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
#     cargo build --release -p baz --features device-output
#   toolbox run -c baz-dev docs/design/composition/tools/mkfixture.sh /tmp/baz-grouped-fix
#   toolbox run -c baz-dev docs/design/impl/artists-grouped/capture.sh
set -uo pipefail

REPO=${REPO:-$(git rev-parse --show-toplevel)}
BIN=${BIN:-$REPO/target/tb/release/baz}
FIX=${FIX:-/tmp/baz-grouped-fix}
OUT=${OUT:-$REPO/docs/design/impl/artists-grouped}
DISP=${DISP:-:198}
S=/tmp/baz-grouped-scratch

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
# **Give one artist enough records to need two rows.** `mkfixture.sh` files two
# records under each of twelve artists, so every shelf would be one short row
# and the frame could not show that a shelf is the wall's ordinary shelf. Four
# of Kesh's label-mates are re-filed under Kesh, keeping their own covers.
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

# **The config a baz from before this change wrote**, verbatim — including the
# `wall_subject` key that no longer exists. It is the migration, photographed:
# the arrangement beside it still resolves, `group_key = "artist"` now means
# the artist grouping its word always claimed, and the next save drops the
# retired line.
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
key()   { xdotool key "$@"; sleep 0.7; }
typein(){ xdotool type --delay 45 "$1"; sleep 1.2; }

Xvfb "$DISP" -screen 0 1920x1080x24 -nolisten tcp &
XPID=$!
sleep 1

# ---------------------------------------------------------------- 1280 × 860
W=1280; H=860
launch $W $H

# **The wall, grouped by artist.** One shelf per person, headed by their name;
# the strip's row is five words again and the first of them is `ARTIST`.
park
shot 00-artist-wall-1280
crop 00-artist-wall-1280 01-arrangement-row-1280 "420x50+300+0"
crop 00-artist-wall-1280 02-shelf-headers-1280 "700x430+300+55"

# **The header is a door.** Pressing the artist's name on the wall opens their
# place — the same `Place::Artist` the record page's `Artist ›` breadcrumb
# opens, reached by the same `vm::artist_id`. The pointer route, so the frame is
# of a control being used: the ground under the word is `theme::word_button`'s,
# the breadcrumb's own, and it is the word's box rather than the shelf's width.
xdotool mousemove 370 95; sleep 0.8
shot 03-the-header-under-the-pointer-1280
# The affordance is deliberately quiet — `ink_wash`, the same value the
# breadcrumb wears — so it is stated at 4× against the resting header above it
# rather than left for the reader to find. The ground is exactly the word's
# box: it stops at the `G`, not at the shelf's right edge.
magick \
  \( "$OUT/00-artist-wall-1280.png" -crop 170x26+312+83 +repage \) \
  \( "$OUT/03-the-header-under-the-pointer-1280.png" -crop 170x26+312+83 +repage \) \
  -append -filter point -resize 400% "$OUT/04-the-door-at-rest-and-hovered-1280.png"
echo "  shot 04-the-door-at-rest-and-hovered-1280"
xdotool click 1; sleep 1.8
park
shot 05-the-artist-place-1280
key Escape
sleep 1.2

# **The rail is still the alphabet, and a letter lands on the first artist
# under it.** `K` is one press, and it puts Kesh at the top — the six-record
# artist, whose shelf is the wall's ordinary shelf with two rows in it. That is
# the claim a rail over twelve headers instead of five letters has to earn.
xdotool mousemove 1237 337; sleep 0.4; xdotool click 1; sleep 1.4
park
shot 06-rail-jumped-to-k-1280
crop 06-rail-jumped-to-k-1280 07-a-shelf-with-two-rows-1280 "700x470+300+40"

# **Mid-query.** The wall narrows to the records that matched and the shelves
# that keep one; the well's figure counts records, because every tile is a
# record again.
key slash
typein "an"
park
shot 08-mid-query-1280
crop 08-mid-query-1280 09-match-count-1280 "280x120+0+0"
key Escape
key Escape

# **The other four keys are untouched.** YEAR is the same wall it was, which is
# what makes ARTIST an ordinary sixth-of-five rather than a mode.
key 2
park
shot 10-year-wall-1280
key 1
stop

# ------------------------------------------------- the strip, at its own band
# **The single-line-with-well band, at its new left edge.** Below
# `SIDEBAR_FLOOR` the lane is a rail that cannot open, so the strip carries the
# well; a window of 874 leaves the strip exactly `TOP_BAR_SPLIT` = 778, the
# narrowest window at which the strip is still one line. It was 928 while the
# row had six words.
W=874; H=700
launch $W $H
park
shot 11-strip-single-line-with-well-874
crop 11-strip-single-line-with-well-874 12-strip-band-874 "874x50+0+0"
stop

# **And at the window's own minimum** — `TOP_BAR_FLOOR` 600 plus the lane's
# rail 96 — where the strip is two lines: the frame's furniture above, the
# library's five words and its one act below. Nothing hides and nothing
# overflows; that is what the 94 px of slack under the floor is for.
W=696; H=620
launch $W $H
park
shot 13-strip-at-the-window-floor-696
crop 13-strip-at-the-window-floor-696 14-strip-two-lines-696 "696x95+0+0"
stop

# --------------------------------------------------------------- 1920 × 1080
W=1920; H=1080
launch $W $H
park
shot 20-artist-wall-1920
key slash
typein "an"
park
shot 21-mid-query-1920
key Escape
key Escape
key 2
park
shot 22-year-wall-1920
key 1
stop

kill "$XPID" 2>/dev/null

echo
echo "--- the isolation receipt (docs/DEVELOPMENT.md) ---"
grep -m3 '^\[mpris\]' "$S/app.log" || echo "NO MPRIS LINE — check the log"
echo
echo "--- the retired config key, migrated ---"
# The document baz wrote back. `wall_subject` was read by nothing, so it is
# gone; `group_key = "artist"` is exactly what it was, and now names the
# arrangement its word always claimed.
cat "$S/config/baz/config.toml"
echo
echo "--- what the wall reported ---"
grep -m4 '^\[startup\]' "$S/app.log"
