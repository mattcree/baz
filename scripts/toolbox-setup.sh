#!/usr/bin/env bash
# One-command dev environment for baz on Fedora (Silverblue/ostree or classic).
# Creates a rootless `baz-dev` toolbox with the full native build stack.
# Usage: ./scripts/toolbox-setup.sh
set -euo pipefail

PACKAGES=(
  gcc gcc-c++ make git pkgconf-pkg-config
  # Audio output (cpal/ALSA) and fixture encoding for the golden-file tests
  alsa-lib-devel flac
  # iced/winit needs these to open a window; the X11 one is required even
  # for headless Xvfb runs (winit panics without it).
  libxkbcommon-devel libxkbcommon-x11
  # Headless render verification: agents screenshot the real UI on a private
  # display and diff it (that is how the views/ split was proven pixel-identical).
  xorg-x11-server-Xvfb ImageMagick
  # Release/Flatpak manifest checks documented in docs/RELEASING.md.
  python3-pyyaml desktop-file-utils appstream
)
# Note: the Tauri/WebKitGTK stack was removed after ADR-0005 chose iced —
# baz has no webview dependency, and Linux builds need no GUI system libraries.

if ! toolbox list --containers 2>/dev/null | grep -q '\bbaz-dev\b'; then
  toolbox create -y baz-dev
fi

toolbox run -c baz-dev sudo dnf install -y "${PACKAGES[@]}"

echo
echo "baz-dev ready. Rust comes from your rustup install (shared \$HOME)."
echo "  enter:   toolbox enter baz-dev"
echo "  one-off: toolbox run -c baz-dev cargo build"
