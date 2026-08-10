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

### Audible tests are opt-in: `BAZ_DEVICE_TESTS=1`

A routine `cargo test --workspace --all-features` **makes no sound**, and that
is a rule rather than an accident. The device-gated tests still open the real
default output, write to it, reopen it at another rate, discard its ring and
tear it down — they just write silence while doing it, because every assertion
they make is about the transport (frames moved, ring emptied, stream still
alive, no xruns) and a driver clocks out silence exactly as it clocks out a
tone. Silence costs the coverage nothing and buys back a quiet machine.

What silence *cannot* stand in for is a human hearing that baz plays music, so
the tests that drive a full engine session through real hardware — the
`device_engine_*` tests in `crates/baz-core/tests/engine.rs` — are behind a
variable and skip with a notice when it is unset:

```sh
toolbox run -c baz-dev env BAZ_DEVICE_TESTS=1 \
  cargo test -p baz-core --all-features -- --nocapture
```

Set it to any non-empty value except `0`. It changes nothing else: the same
tests, the same assertions, run on demand instead of on every build. CI leaves
it unset, so CI is silent too — which costs nothing there, since no runner has
an audio device in the first place.

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

They write silence rather than tones (see above), so they are quiet; they skip
with a notice when every device is busy.

## Two meters, both off by default

`BAZ_FRAME_LOG=1` prints a timestamp every time the shell draws. It is how the
0-frames-when-idle claim in `docs/design/04-fluidity.md` §1.4 is checked, and
how a *"nothing is moving and it still redraws"* report is settled.

`BAZ_MSG_LOG=1` prints one line a second naming every message variant that
arrived in it, busiest first, and nothing at all in a second where nothing
arrived:

```
$ BAZ_MSG_LOG=1 baz 2>&1 | grep '^\[msg\]'
[msg] 87/s  Scrolled 58  ·  WindowResized 29
```

That line is a real measurement, taken while dragging a window edge: **every
resize step delivers three messages, not one** — `WindowResized` with its
estimated grid, `Scrolled` when the scrollable measures its real bounds, and
`Scrolled` again when the grid that changed underneath it changed the content's
height (iced republishes a viewport whose `content_bounds` moved,
`iced_widget-0.13.4/src/scrollable.rs:1249`). Two of the three ask
`request_visible_thumbs` for exactly what the first asked for, which is why
that function keeps a range guard.

Both meters are free when off — one relaxed atomic load, resolved once from the
environment — and neither formats anything it does not print. Reach for the
message meter first whenever a report sounds like *"something is firing a
lot"*: it turns a hypothesis into a histogram in ten seconds, and the shell's
messages come from six subscriptions, a scrollable that republishes on every
layout change and a window that reconfigures on every drag step, which is not a
thing to reason about from the source.

## The local gate is not CI, and twice today that mattered

Run the whole gate after your **last** edit, not your last interesting one:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo doc --workspace --no-deps          # warnings are errors
cargo deny check
python3 packaging/flatpak/check-cargo-sources.py
```

`main` went red on 2026-08-10 from running everything in that list except the
first, after a `clippy` fix shortened a line enough for `rustfmt` to want it
joined.

**Then watch CI, because two of the three platforms exist only there.** The
same day, two merges went red on `macos-latest` while every one of the commands
above was green locally, and the cause was not portability trivia — a timing
assertion held on Linux and failed at **5× the budget** on macOS, because a
giant allocation is a lazy mapping on one and real pages on the other. That is
a *finding*, and it was sitting in a job nobody had looked at.

Windows has taught this project three of these the hard way — drive-less
fixture paths, UTF-16LE stored paths, FILETIME stamps — and macOS has now
taught it one. A green local gate is necessary and is not sufficient.

## Headless UI verification

Agents (and you) can render the real binary without touching your desktop
session. Always redirect the XDG dirs — the app writes its library on launch,
and an earlier run polluted the maintainer's real database by relying on
backup-and-restore, which races the app's own writes:

```sh
toolbox run -c baz-dev env -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS \
  WINIT_UNIX_BACKEND=x11 HOME=/tmp/scratch/home \
  XDG_DATA_HOME=/tmp/scratch/data XDG_CONFIG_HOME=/tmp/scratch/config \
  XDG_CACHE_HOME=/tmp/scratch/cache XDG_RUNTIME_DIR=/tmp/scratch/run \
  xvfb-run -s '-screen 0 1400x1000x24' cargo run --release -p baz -- /tmp/fixture-music
```

**Redirect all six, not just the three obvious ones.** Each omission has cost
us something real:

- `XDG_DATA_HOME` — the library database. An early run relied on
  backup-and-restore instead and **polluted the maintainer's real library**,
  because a restore races the app's own writes.
- `XDG_CONFIG_HOME` — the remembered music folder and settings.
- `XDG_CACHE_HOME` — thumbnails.
- `HOME` — everything that resolves through it when the above are unset, and
  the `.asoundrc` you may want (see below).
- `XDG_RUNTIME_DIR` **and** `DBUS_SESSION_BUS_ADDRESS` — zbus falls back to
  `$XDG_RUNTIME_DIR/bus`, so a run that leaves these alone **joins the
  maintainer's session bus and publishes an MPRIS name onto his desktop**. That
  happened. A correctly isolated run logs `[mpris] no session bus`; treat that
  line as the receipt that the isolation held.

**Silence, when you build with `device-output`.** The transport only exists in
that build, so rendering the bottom bar needs it — but nothing should be
audible. Put an `.asoundrc` in the scratch `HOME` routing ALSA's default PCM to
`null`, and use silent fixtures. Two independent guarantees: the sink discards
every sample, and every sample is a zero. `BAZ_DEVICE_TESTS` stays unset.

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

**Build the binary where you will run it.** The host toolchain links against
the host's glibc, which is newer than the toolbox's, so a host-built
`target/release/baz` dies inside the container with `GLIBC_… not found` — and a
capture script waiting for a window will simply hang. Build with
`CARGO_TARGET_DIR=target/tb` inside the toolbox and point the harness at it
(`capture.sh` takes `BIN`).

`toolbox run` does **not** forward your shell's environment: pass variables
through `env`, as `toolbox run -c baz-dev env OUT=… SCEN=… bash …`. A run whose
variables were silently dropped renders the default scenario at the default
size, which looks like a successful capture of the wrong thing.

### Two `baz` binaries, and how to tell which one you are running

The consequence of the paragraph above is that this checkout usually holds
**two release binaries with the same name**:

| path | built by | runs on |
|---|---|---|
| `target/release/baz` | the host toolchain | the host only |
| `target/tb/release/baz` | `CARGO_TARGET_DIR=target/tb` inside the toolbox | the toolbox, and the host |

They are indistinguishable to anyone looking for an executable, and the
**obvious** one — `target/release/` — is the one that goes stale, because
day-to-day work builds through the toolbox. That has cost real time twice:

- a capture measured the wrong build, because a script found the wrong file;
- the owner launched `target/release/baz` for a look at the product, got a
  two-day-old build, and reported two defects that had both been fixed. (One
  of them turned out to be a real failure *mode* underneath — see ADR-0041 —
  but the report started with a stale binary.)

So: **check the timestamps before you conclude anything from a binary's
behaviour.** `ls -l target/release/baz target/tb/release/baz`, and prefer
`sha256sum` when two builds are being compared — `baz --version` prints the
crate version, which does not move between commits and cannot tell them apart.
A capture script comparing two builds must copy each to its **own filename**
before running either.

Screenshot and diff with ImageMagick (`magick compare -metric AE`). Use the
wgpu renderer, not tiny-skia: tiny-skia does damage-based partial repaints and
is not run-to-run deterministic, so it cannot prove a refactor changed nothing.
Read the count **in parentheses** and parse it with `awk`, not `sed`: the
leading figure is formatted with `%g`, so a large difference arrives as
`1.86447e+08 (2845)` and a shell integer comparison on it reads *one*.

> The Phase 1 spikes referenced here previously were deleted when Phase 1
> closed; they remain recoverable at `git show dc13d7e`.
