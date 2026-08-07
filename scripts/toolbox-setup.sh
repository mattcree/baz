#!/usr/bin/env bash
# One-command dev environment for baz on Fedora (Silverblue/ostree or classic).
# Creates a rootless `baz-dev` toolbox with the full native build stack.
# Usage: ./scripts/toolbox-setup.sh
set -euo pipefail

PACKAGES=(
  gcc gcc-c++ make git pkgconf-pkg-config
  # Tauri / GTK webview
  webkit2gtk4.1-devel gtk3-devel dbus-devel openssl-devel
  librsvg2-devel libappindicator-gtk3-devel
  # Audio
  alsa-lib-devel flac
  # Frontend toolchain (containers may not see host-managed node)
  nodejs npm
)

if ! toolbox list --containers 2>/dev/null | grep -q '\bbaz-dev\b'; then
  toolbox create -y baz-dev
fi

toolbox run -c baz-dev sudo dnf install -y "${PACKAGES[@]}"

echo
echo "baz-dev ready. Rust comes from your rustup install (shared \$HOME)."
echo "  enter:   toolbox enter baz-dev"
echo "  one-off: toolbox run -c baz-dev cargo build"
