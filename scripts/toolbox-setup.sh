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
  # `xdotool` drives that display: an agent presses baz's own controls rather
  # than asserting about a still. It was hand-installed in the maintainer's
  # container and missing from this list, so a rebuilt container silently lost
  # the ability to click — found on 2026-08-23 when the container vanished and
  # this script would not have brought it back.
  xorg-x11-server-Xvfb ImageMagick xdotool
  # The X client libraries winit dlopens to open a window. They arrived in the
  # maintainer's container as somebody else's dependency and were never listed,
  # so a container built from this script alone could compile baz and not run
  # it: "Create event loop: XNotSupported(libXcursor.so.1: cannot open shared
  # object file)". Found on 2026-08-23, rebuilding the container from scratch.
  libXcursor libXi libXrandr libXinerama
  # `ort`'s build-time model downloader links native-tls, so `--all-features`
  # — which the CI gate's clippy and test steps both use — needs the OpenSSL
  # headers on the *host* toolchain. The shipped binary does not link OpenSSL
  # (docs/RELEASING.md says so and it is still true); this is a build
  # dependency only. Third entry in this file's running list of packages that
  # lived in the maintainer's container by hand and not in this script: the
  # 2026-08-23 rebuild dropped it, and `cargo clippy --all-features` has failed
  # with "Could not find directory of OpenSSL installation" on any cold target
  # directory since. Found 2026-08-24.
  openssl-devel
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
