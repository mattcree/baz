# Installing baz

> **Read this first.** baz is **pre-alpha**. It scans a music folder, shows
> your albums and plays them; it is not a finished player. It reads your files
> and never writes to them, but expect rough edges, and expect the library
> database to be rebuilt by a future version.
>
> **Linux is the platform baz is developed and used on.** The Windows and macOS
> binaries are built and tested by CI on every change — the same test suite, on
> all three operating systems — but no human has sat in front of baz on Windows
> or macOS. They are honest builds, not a supported experience. Reports
> welcome; surprises likely.
>
> There is no released version yet. Nothing has been tagged, so there are no
> downloads on the releases page. Until then, **build from source**.

## Flatpak (Linux, the intended way)

Not yet on Flathub — see `packaging/flatpak/README.md` for where the
submission stands. When it is there:

```sh
flatpak install flathub io.github.mattcree.baz
flatpak run io.github.mattcree.baz
```

The Flatpak is granted read-only access to `~/Music` and nothing else of your
home directory. If your collection lives elsewhere, grant it explicitly:

```sh
flatpak override --user --filesystem=/srv/music:ro io.github.mattcree.baz
```

It also gets Wayland (or X11), the GPU, audio, and permission to own its MPRIS
bus name so desktop media controls work. It does not get the network, because
baz does not use it.

To build and install the Flatpak from a checkout without waiting for Flathub:

```sh
flatpak install flathub org.freedesktop.Platform//25.08 org.freedesktop.Sdk//25.08 \
                        org.freedesktop.Sdk.Extension.rust-stable//25.08
flatpak-builder --user --install --force-clean build-dir \
  packaging/flatpak/io.github.mattcree.baz.yml
```

That builds the tag named in the manifest, not your working tree — see
`packaging/flatpak/README.md` for building the checkout itself.

## Release binaries

Each tagged release attaches these to its GitHub release page, together with a
`SHA256SUMS` file:

| File | For | Rust target |
|---|---|---|
| `baz-<version>-linux-x86_64.tar.gz` | 64-bit Intel/AMD Linux | `x86_64-unknown-linux-gnu` |
| `baz-<version>-macos-universal.tar.gz` | any Mac, Apple silicon or Intel | `aarch64-` and `x86_64-apple-darwin`, combined |
| `baz-<version>-windows-x86_64.zip` | 64-bit Windows | `x86_64-pc-windows-msvc` |

Verify what you downloaded before you run it:

```sh
sha256sum --check --ignore-missing SHA256SUMS
```

Then unpack and put the binary somewhere on your `PATH`:

```sh
tar xf baz-<version>-linux-x86_64.tar.gz
cd baz-<version>-linux-x86_64
install -Dm755 baz ~/.local/bin/baz
```

The Linux archive also carries the desktop entry and AppStream metadata, for a
menu entry and working media-key integration:

```sh
install -Dm644 io.github.mattcree.baz.desktop \
  ~/.local/share/applications/io.github.mattcree.baz.desktop
install -Dm644 io.github.mattcree.baz.metainfo.xml \
  ~/.local/share/metainfo/io.github.mattcree.baz.metainfo.xml
```

`Exec=baz` in that entry assumes the binary is on `PATH`; edit it to an
absolute path if you put it elsewhere.

**macOS and Windows**: the binaries are **not signed or notarized**. macOS will
refuse to run an unnotarized download until you clear the quarantine attribute
(`xattr -d com.apple.quarantine baz`), and Windows SmartScreen will warn. That
is not a formality being skipped — code-signing certificates are a cost and an
identity decision the project has not made. Check the SHA-256 against
`SHA256SUMS` and decide for yourself; the sums are produced by the same public
CI run that built the binaries, from a tag, and the workflow that did it is
`.github/workflows/release.yml`.

**Linux runtime requirements**: glibc (the artifact is a `gnu` target, not
`musl`) and ALSA at runtime, which every desktop Linux has. baz needs no GUI
system libraries: the toolkit is pure Rust and SQLite is compiled in.

## From source

This is the only way to run baz today, and it is not hard — one system package
on Linux, and nothing at all on macOS or Windows.

```sh
git clone https://github.com/mattcree/baz
cd baz
cargo build --release --locked -p baz --features device-output
./target/release/baz [MUSIC_DIR]
```

The toolchain is pinned in `rust-toolchain.toml`, so `rustup` installs the
right compiler for you; the minimum supported version is in `Cargo.toml`.

**`--features device-output` is what makes sound come out.** It is off by
default because building it needs the platform's audio headers, which the
project's primary development host does not have. Without it baz still builds
and runs everywhere, shows your whole library, and hides the playback controls
— useful for working on the interface, useless as a music player. Released
binaries and the Flatpak are always built with it.

The audio headers, if your Linux distribution has not already installed them:

| Distribution | Package |
|---|---|
| Debian, Ubuntu | `libasound2-dev` |
| Fedora, RHEL | `alsa-lib-devel` |
| Arch | `alsa-lib` |

macOS and Windows need nothing extra — CoreAudio and WASAPI come with the
system.

Fedora users, including Silverblue: `./scripts/toolbox-setup.sh` builds a
container with everything, and `docs/DEVELOPMENT.md` covers the rest.

## Where baz keeps its things

Nothing is written next to your music. baz uses the platform's own locations:

| | Linux | macOS | Windows |
|---|---|---|---|
| Config | `~/.config/baz/config.toml` | `~/Library/Application Support/baz/` | `%APPDATA%\baz\` |
| Library database | `~/.local/share/baz/library.db` | `~/Library/Application Support/baz/` | `%APPDATA%\baz\` |

Under Flatpak these live in `~/.var/app/io.github.mattcree.baz/`. Deleting the
database costs you one rescan and nothing else — your files are the source of
truth, and baz never edits them.
