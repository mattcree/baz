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

icon="$here/../icons/$app_id.icns"
[ -f "$icon" ] || { echo "bundle.sh: no icon at $icon — run packaging/icons/render.sh" >&2; exit 1; }

app="$out/baz.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"

# The plist's two substitutions. `@APP_ID@` rather than a literal so the one
# string stays one string; `@VERSION@` from the caller so the Finder's Get
# Info panel agrees with the tag.
sed -e "s/@APP_ID@/$app_id/g" -e "s/@VERSION@/$version/g" \
  "$here/Info.plist.in" > "$app/Contents/Info.plist"

cp "$icon" "$app/Contents/Resources/$app_id.icns"
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
    ("CFBundleIconFile", app_id),
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
