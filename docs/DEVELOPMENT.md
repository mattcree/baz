# baz — Development Environment

> How to get a full native build environment. Standards live in `ENGINEERING.md`.

## TL;DR (Fedora, including Silverblue/ostree)

```sh
./scripts/toolbox-setup.sh   # creates the rootless `baz-dev` toolbox with all deps
toolbox enter baz-dev        # or: toolbox run -c baz-dev <command>
```

The toolbox shares your `$HOME`, your session (Wayland/X11, PipeWire) and your rustup toolchain, so GUI apps and audio output work from inside it, `cargo` is the same one you use on the host, and no `sudo` password is ever needed. Node inside the container comes from Fedora's `nodejs` package (host-managed Node installs like linuxbrew may not resolve in-container).

## What the environment provides

| Component | Why |
|---|---|
| `gcc`/`gcc-c++`/`make`/`pkgconf` | C toolchain for -sys crates |
| `webkit2gtk4.1-devel`, `gtk3-devel`, `dbus-devel`, `librsvg2-devel`, `libappindicator-gtk3-devel`, `openssl-devel` | Tauri 2 native shell on Linux |
| `alsa-lib-devel` | cpal / device audio output |
| `flac`, (`ffmpeg` if present) | encoding test fixtures for audio golden tests |
| `nodejs`, `npm` | frontend toolchain |

Rust itself is **not** installed in the container — it comes from your rustup install in `$HOME` (pinning via `rust-toolchain.toml` once the workspace exists in Phase 2).

## Alternatives

- **Classic Fedora (no container)**: `sudo dnf install -y` the package list from `scripts/toolbox-setup.sh`.
- **Dev container**: `.devcontainer/` carries the same environment as a Containerfile for VS Code / Claude Code / cloud agents. Keep its package list in sync with `scripts/toolbox-setup.sh` (single source of truth: the script).
- **Debian/Ubuntu contributors**: equivalents are `libwebkit2gtk-4.1-dev libgtk-3-dev libdbus-1-dev libssl-dev librsvg2-dev libasound2-dev` — to be verified when CI lands (CI runs on Ubuntu runners and is the reference for that list).

## Running baz with audio output

Device playback is behind the non-default `device-output` feature (building
cpal needs `alsa-lib-devel`, which the toolbox provides):

```sh
toolbox run -c baz-dev cargo run --release -p baz --features device-output [-- MUSIC_DIR]
```

A plain host `cargo run -p baz` builds everywhere and runs the full shelf,
but prints `built without audio output — see docs/DEVELOPMENT.md` and hides
the playback UI. With the feature but no usable output device, the app still
runs and the bottom bar reports "no audio device".

## Running the Phase 1 spikes

All spikes are throwaway (see `NEXT-STEPS.md`) but runnable:

```sh
# iced shelf — builds on host OR container (no system deps needed)
cd spikes/shelf-iced && cargo run --release --bin gen_dataset && cargo run --release --bin shelf-iced

# Tauri shelf — browser mode (host) / native (container)
cd spikes/shelf-tauri && npm run dev          # browser mode at :5173
toolbox run -c baz-dev npm run tauri dev      # native WebKitGTK window

# audio engine — tests prove gapless; device output behind a feature
cd spikes/audio-gapless && cargo test
toolbox run -c baz-dev cargo test --features device-output
```
