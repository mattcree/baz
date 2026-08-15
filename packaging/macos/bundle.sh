#!/usr/bin/env bash
# **Assemble `baz.app` from a built binary.**
#
# A bare Mach-O executable cannot carry an application icon, a name, or a
# permission string: on macOS all three are properties of a *bundle*, which is
# a directory with a documented shape. baz shipped a bare binary in a tarball,
# so Finder, the Dock, Launchpad and Spotlight had nothing to draw but the
# generic application mark. This builds the directory.
#
#     packaging/macos/bundle.sh BINARY VERSION OUTPUT_DIR
#
# It runs on any Unix — deliberately. Nothing here is `iconutil`, `plutil` or
# `codesign`: the icon is committed (`packaging/icons/`, rendered by
# `render.sh`), the plist is a template with two substitutions, and the
# structure is `mkdir` and `cp`. That means the bundle can be built and
# inspected on the machine baz is developed on rather than only on the release
# runner, which is the difference between a reviewable change and a change
# nobody can look at.
#
# **What it deliberately does not do: sign or notarize.** See
# `packaging/macos/README.md` — that needs an Apple Developer account, which
# is an external boundary and the owner's to cross.
set -euo pipefail

if [ "$#" -ne 3 ]; then
  echo "usage: bundle.sh BINARY VERSION OUTPUT_DIR" >&2
  exit 2
fi

binary=$1
version=$2
out=$3
here=$(cd "$(dirname "$0")" && pwd)
app_id=io.github.mattcree.baz

[ -f "$binary" ] || { echo "bundle.sh: no binary at $binary" >&2; exit 1; }

iconset="$here/../icons/baz.iconset"
fallback="$here/../icons/$app_id.icns"
[ -d "$iconset" ] || { echo "bundle.sh: no iconset at $iconset — run packaging/icons/render.sh" >&2; exit 1; }

app="$out/baz.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"

# The plist's two substitutions. `@APP_ID@` rather than a literal so the one
# string stays one string; `@VERSION@` from the caller so the Finder's Get
# Info panel agrees with the tag.
sed -e "s/@APP_ID@/$app_id/g" -e "s/@VERSION@/$version/g" \
  "$here/Info.plist.in" > "$app/Contents/Info.plist"

# **Apple's own tool builds the container that ships.**
#
# The first version of this shipped a hand-written `.icns` and macOS drew the
# generic icon over it — while an independent reader opened the file and found
# all ten sizes, so whatever IconServices wanted was something a general
# parser does not check. Guessing at that one release at a time, with no Mac
# in the loop, is the wrong way to spend attention: `iconutil` is on every
# macOS runner and is the tool the format belongs to.
#
# The fallback is used only where `iconutil` does not exist, which is CI's
# Linux run checking this script's *shape*. It is never what a person
# downloads.
if command -v iconutil >/dev/null 2>&1; then
  iconutil -c icns "$iconset" -o "$app/Contents/Resources/$app_id.icns"
  echo "  icon: built by iconutil from $(basename "$iconset")"
else
  [ -f "$fallback" ] || {
    echo "bundle.sh: no iconutil and no fallback at $fallback" >&2
    exit 1
  }
  cp "$fallback" "$app/Contents/Resources/$app_id.icns"
  echo "  icon: committed fallback (no iconutil on this machine)"
fi
cp "$binary" "$app/Contents/MacOS/baz"
chmod +x "$app/Contents/MacOS/baz"

# The models Vibe needs. `Contents/Resources` is where a bundle's data lives.
# `baz_vibe::semantic::model_directory` walks the executable's ancestors and
# now also tries `Resources/models/vibe` at each rung — `Contents/Resources`
# is one directory *across* from `Contents/MacOS`, not above it, so ancestors
# alone would never have reached it. That was checked against the source
# rather than assumed, and the source gained a line.
if [ -d "$here/../../models/vibe" ]; then
  mkdir -p "$app/Contents/Resources/models/vibe"
  cp "$here/../../models/vibe/audio_model_quantized.onnx" \
     "$here/../../models/vibe/text_model_quantized.onnx" \
     "$here/../../models/vibe/tokenizer.json" \
     "$here/../../models/vibe/LICENSE" \
     "$here/../../models/vibe/MODEL_CARD.md" \
     "$here/../../models/vibe/README.md" \
     "$app/Contents/Resources/models/vibe/"
fi

# **Check the shape rather than trusting the script**, because every failure
# mode here is silent: macOS answers a malformed bundle with the generic icon
# and no diagnostic at all.
python3 - "$app" "$app_id" "$version" <<'EOF'
import plistlib
import struct
import sys

app, app_id, version = sys.argv[1], sys.argv[2], sys.argv[3]

with open(f"{app}/Contents/Info.plist", "rb") as handle:
    plist = plistlib.load(handle)
for key, want in [
    ("CFBundleIdentifier", app_id),
    ("CFBundleIconFile", f"{app_id}.icns"),
    ("CFBundleExecutable", "baz"),
    ("CFBundleShortVersionString", version),
    ("CFBundlePackageType", "APPL"),
]:
    if plist.get(key) != want:
        raise SystemExit(f"Info.plist {key} is {plist.get(key)!r}, expected {want!r}")
if plist.get("NSHighResolutionCapable") is not True:
    raise SystemExit("NSHighResolutionCapable must be true or the icon draws soft")
if plist.get("LSUIElement") is not False:
    raise SystemExit("LSUIElement must be false or the Dock icon is hidden")

# The icon is the point of the exercise; read its container rather than its
# name. `@VERSION@` left unsubstituted would also land here as a plist that
# parses and a version nobody can compare.
with open(f"{app}/Contents/Resources/{app_id}.icns", "rb") as handle:
    icon = handle.read()
if icon[:4] != b"icns" or struct.unpack(">I", icon[4:8])[0] != len(icon):
    raise SystemExit("the bundled icns header does not describe the file it is in")
if "@" in version:
    raise SystemExit(f"version {version!r} was never substituted")

print(f"  bundle: {app_id} {version}, icon {len(icon)} bytes")
EOF

echo "built $app"
