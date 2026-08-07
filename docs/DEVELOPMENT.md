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
| `alsa-lib-devel` | cpal / device audio output |
| `libxkbcommon-devel`, `libxkbcommon-x11` | iced/winit window creation (the X11 one is required even for headless Xvfb runs) |
| `xorg-x11-server-Xvfb`, `ImageMagick` | headless render verification — screenshot the real UI on a private display and diff it |
| `flac`, (`ffmpeg` if present) | encoding test fixtures for the audio golden tests |

baz itself needs **no GUI system libraries** to build on Linux: iced is pure
Rust (ADR-0005) and SQLite is bundled. Everything above serves the toolchain or
the test harness, not the binary.

Rust itself is **not** installed in the container — it comes from your rustup install in `$HOME` (pinning via `rust-toolchain.toml` once the workspace exists in Phase 2).

## Alternatives

- **Classic Fedora (no container)**: `sudo dnf install -y` the package list from `scripts/toolbox-setup.sh`.
- **Dev container**: `.devcontainer/` carries the same environment as a Containerfile for VS Code / Claude Code / cloud agents. Keep its package list in sync with `scripts/toolbox-setup.sh` (single source of truth: the script).
- **Debian/Ubuntu contributors**: the equivalent of the build-critical one is `libasound2-dev` (what CI installs on its Ubuntu runners); add `libxkbcommon-x11-0 xvfb imagemagick` for the render harness.

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

### Exclusive-mode output (Linux, ADR-0012)

Shared mode is the default. To have baz hold an ALSA `hw:` device outright —
no system mixer between the decoder and the converter — build with the
`exclusive-output` feature and name a device:

```sh
toolbox run -c baz-dev env BAZ_OUTPUT=exclusive BAZ_OUTPUT_DEVICE=hw:3,0 \
  cargo run --release -p baz --features device-output,baz-core/exclusive-output
```

`BAZ_OUTPUT` is `shared` (the default) or `exclusive`; anything else is an
error rather than a silent fall back. `BAZ_OUTPUT_DEVICE` is an ALSA
`hw:CARD,DEV` name — never `plughw:`, which converts without saying so and is
refused. With several devices and none named, the open fails with a message
listing them; with exactly one, it is used.

**Expect `DeviceBusy` on a desktop.** The sound server usually holds the card
the desktop is routed to, and exclusive mode cannot share it: pick a device
`PipeWire` is not using, or release the one it is. The failure is immediate
(~50 µs) and named, never a hang.

The device-gated tests honour the same variable, which is how to verify the
claims on a particular DAC rather than on whatever enumerates first:

```sh
toolbox run -c baz-dev env BAZ_OUTPUT_DEVICE=hw:3,0 \
  cargo test -p baz-core --features exclusive-output --test playback exclusive -- --nocapture
```

They play short tones on real hardware, so they are audible; they skip with a
notice when every device is busy.

## Headless UI verification

Agents (and you) can render the real binary without touching your desktop
session. Always redirect the XDG dirs — the app writes its library on launch,
and an earlier run polluted the maintainer's real database by relying on
backup-and-restore, which races the app's own writes:

```sh
toolbox run -c baz-dev env -u WAYLAND_DISPLAY WINIT_UNIX_BACKEND=x11 \
  XDG_DATA_HOME=/tmp/scratch/data XDG_CONFIG_HOME=/tmp/scratch/config \
  XDG_CACHE_HOME=/tmp/scratch/cache \
  xvfb-run -s '-screen 0 1400x1000x24' cargo run --release -p baz -- /tmp/fixture-music
```

### Verifying MPRIS without touching the owner's desktop

The desktop integration talks to a D-Bus **session** bus, and the owner has one
running. Never test against it — start a private one, point only the test
instance at it, and inspect that:

```sh
dbus-daemon --session --fork --print-address=3 3>/tmp/scratch/addr \
                              --print-pid=4 4>/tmp/scratch/pid
export DBUS_SESSION_BUS_ADDRESS="$(cat /tmp/scratch/addr)"
# ...launch baz with the XDG redirection above, plus this address...
busctl --user introspect org.mpris.MediaPlayer2.baz /org/mpris/MediaPlayer2
busctl --user call org.mpris.MediaPlayer2.baz /org/mpris/MediaPlayer2 \
       org.mpris.MediaPlayer2.Player PlayPause
kill "$(cat /tmp/scratch/pid)"
```

`XDG_RUNTIME_DIR` should be short: zbus falls back to `$XDG_RUNTIME_DIR/bus`,
and a Unix socket path over ~100 bytes fails with `SUN_LEN` rather than the
error you were testing for. To check graceful degradation, unset
`DBUS_SESSION_BUS_ADDRESS` and point `XDG_RUNTIME_DIR` at a directory with no
`bus` socket: the app must print one `[mpris]` line and run normally.

Screenshot and diff with ImageMagick (`magick compare -metric AE`). Use the
wgpu renderer, not tiny-skia: tiny-skia does damage-based partial repaints and
is not run-to-run deterministic, so it cannot prove a refactor changed nothing.

> The Phase 1 spikes referenced here previously were deleted when Phase 1
> closed; they remain recoverable at `git show dc13d7e`.
