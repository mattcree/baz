#!/bin/sh
# **Hand a Mac user something they know what to do with.**
#
# What shipped before this was a `.zip` of a `.app`, which is a download that
# assumes the person on the other end knows an application belongs in
# `/Applications` and that dragging it there is how it gets installed. Plenty
# do. The ones who do not end up running baz out of `~/Downloads`, where the
# first system update that clears the folder takes their music player with it.
#
# A disk image with the bundle on the left and a symlink to `/Applications` on
# the right is the convention every Mac application has used for twenty years,
# and it teaches the gesture by showing it.
#
# `hdiutil` only, no `create-dmg` and no AppleScript window dressing: the
# background image and the icon positions those buy need a mounted, scripted
# Finder window, which does not work on a headless runner and is the usual
# reason a DMG step is flaky. What is here is the layout, which is the part
# that matters.
set -eu

usage() {
    echo "usage: dmg.sh <path/to/baz.app> <output.dmg> <volume name>" >&2
    exit 2
}

[ $# -eq 3 ] || usage
bundle="$1"
output="$2"
volume="$3"

[ -d "$bundle" ] || { echo "dmg.sh: no bundle at $bundle" >&2; exit 1; }

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

cp -R "$bundle" "$work/"
ln -s /Applications "$work/Applications"

# `UDZO` is the compressed read-only format every distributed DMG uses.
# `-quiet` because hdiutil narrates its own progress bar into CI logs.
rm -f "$output"
hdiutil create \
    -volname "$volume" \
    -srcfolder "$work" \
    -ov \
    -format UDZO \
    -quiet \
    "$output"

# Prove it mounts and contains what we think, rather than trusting that a
# zero exit means a usable image: a DMG that fails to attach fails on the
# listener's machine, which is the worst place to find out.
mount=$(mktemp -d)
hdiutil attach "$output" -mountpoint "$mount" -nobrowse -quiet
[ -d "$mount/$(basename "$bundle")" ] || {
    hdiutil detach "$mount" -quiet || true
    echo "dmg.sh: the image mounted without the bundle in it" >&2
    exit 1
}
[ -L "$mount/Applications" ] || {
    hdiutil detach "$mount" -quiet || true
    echo "dmg.sh: the image has no /Applications to drag onto" >&2
    exit 1
}
hdiutil detach "$mount" -quiet
rmdir "$mount"

echo "dmg.sh: wrote $output"
