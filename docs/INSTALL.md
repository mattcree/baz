# Installing baz

> **Read this first.** baz is a **public beta**. The core loop is finished —
> you can find your music, play it, make lists of it, and nothing baz does
> loses or corrupts anything — but it is pre-1.0, it has rough edges, and the
> ones we know about are in [`../README.md`](../README.md)'s *Known
> limitations* rather than left to be discovered. It reads your files and never
> writes to them. Expect the library database to be rebuilt by a future
> version; that costs a rescan and nothing else.
>
> **Linux is the platform baz is developed and used on.** The Windows and macOS
> binaries are built and tested by CI on every change — the same test suite, on
> all three operating systems — but no human has sat in front of baz on Windows
> or macOS. They are honest builds, not a supported experience. Reports
> welcome; surprises likely.
>
> **v0.1.0 is the first release, and it has not been tagged yet.** Until it is,
> the [releases page](https://github.com/mattcree/baz/releases) is empty and
> **building from source is the only way in** — it works on every platform and
> it is described below.

## Which file to take

Everything a release publishes is the same build. They differ in what happens
afterwards — and *updating* is the difference that matters, because only one
column here happens without you (ADR-0043).

| You are on | Take | Updates by |
|---|---|---|
| Linux | `.flatpak` | your software centre, automatically |
| Windows | `.msi` | **itself** — see *How baz updates itself* below |
| macOS | `.dmg`, then drag baz to Applications | **itself**, the same way |
| anything | `.tar.gz` / `.zip` | nothing; you come back to the releases page |

The archives are not second-class — they are what a packager, a reviewer, or
somebody on a distribution we do not target actually wants. They just do not
update themselves, and nothing in baz will tell you when they are stale.

**Unsigned, for now.** Windows SmartScreen and macOS Gatekeeper will both say
so, because a signing certificate is bought rather than built and baz has not
bought one. The Flatpak needs no such thing, which is a second reason it is
the recommended route on Linux and not merely a convenient one.

## How baz updates itself

**Only the `.msi` and the `.dmg` do this**, because they are the only two
downloads that install anything. A Flatpak is updated by the store that owns
it, and nobody but you can unpack an archive over your own installation.

Two things happen, in two different sessions, and neither of them interrupts
anything (ADR-0043 §5):

1. **While you are listening**, baz asks GitHub once — after the window is
   already up, once per launch — whether there is a newer release. If there
   is, it downloads it in the background and checks it against the SHA-256
   published beside it. It draws nothing at all. A file that fails that check
   is thrown away.
2. **The next time you start baz**, a small program called `baz-boot` runs
   first. If nothing is waiting — which is almost every launch — it starts baz
   and is gone. If something is waiting, it checks it again and asks you, once,
   whether to install it. Say no and it starts baz; it will ask again next
   time. Say yes and the installer runs with baz not yet started, which is the
   only moment it can replace baz without asking you to close anything.

`baz-boot` is what your Start-menu entry and your application icon point at.
That is the whole of it: it has no window of its own and it never asks you to
quit your music player.

**To turn it off**, untick Settings → Updates → *Download new versions in the
background*. That stops both halves: nothing is downloaded and nothing is
offered, and baz makes no network request at all.

**To bring it forward**, Settings → Updates → *Check for updates* asks now
rather than waiting for the background pass, and *Download it now* fetches and
checks the new version straight away. It is still offered at the next start
rather than installed on the spot, because that is the moment it can succeed.

## Flatpak (Linux, the intended way)

**From a release**, without waiting for Flathub — this is a full install, with
the desktop entry, the icons and the automatic updates once it *is* on
Flathub:

```sh
flatpak install ./baz-<version>-linux-x86_64.flatpak
flatpak run io.github.mattcree.baz
```

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

**Until the first tag exists, that command cannot work as written**: the
manifest's `git` source names `tag: v0.1.0` and a placeholder commit, and
nothing has been tagged. Swap in a `dir` source pointing at your checkout —
`packaging/flatpak/README.md` §"Building it" has the two lines — and it builds.
That has been done and it works; the build takes about fifteen minutes from
cold and wants roughly 10 GB of scratch space, which is more than a `/tmp` on
tmpfs is likely to have. Put `build-dir` and flatpak-builder's `--state-dir`
somewhere on real disk.

## Release binaries

GitHub Release archives are the supported beta distribution path, and they are
the one route that does **not** update itself: when you choose to update,
download the new archive and its `SHA256SUMS`, verify it, quit baz, replace the
old application files and relaunch. Your configuration, library database and
playlists live outside the archive and are preserved. Keep the previous archive
until the new one has opened successfully if you want a simple manual rollback.

Each tagged release attaches these to its GitHub release page, together with a
`SHA256SUMS` file:

| File | For | Rust target |
|---|---|---|
| `baz-<version>-linux-x86_64.tar.gz` | 64-bit Intel/AMD Linux | `x86_64-unknown-linux-gnu` |
| `baz-<version>-macos-universal.tar.gz` | any Mac, Apple silicon or Intel | `aarch64-` and `x86_64-apple-darwin`, combined into `baz.app` |
| `baz-<version>-windows-x86_64.zip` | 64-bit Windows | `x86_64-pc-windows-msvc` |

Verify what you downloaded before you run it:

```sh
sha256sum --check --ignore-missing SHA256SUMS
```

Then unpack it and run the installer:

```sh
tar xf baz-<version>-linux-x86_64.tar.gz
cd baz-<version>-linux-x86_64
./install.sh
```

That puts the binary in `~/.local/bin`, the menu entry in
`~/.local/share/applications`, the AppStream metadata in
`~/.local/share/metainfo` and the whole icon ladder in
`~/.local/share/icons/hicolor` — so baz appears in your launcher with its own
mark and the media keys work. It needs no privilege, touches nothing outside
`~/.local`, and rewrites the entry's `Exec=` to the binary it actually
installed. `./install.sh --system` writes to `/usr/local` instead (with
`sudo`), and `--prefix DIR` writes anywhere.

It keeps a manifest of every file it wrote, so:

```sh
./uninstall.sh          # or --system / --prefix DIR, matching the install
```

removes exactly those and nothing else. Your library, playlists and settings
live in `~/.local/share/baz` and `~/.config/baz` and are never touched by
either script.

### By hand

If you would rather place the files yourself, the archive is laid out so you
can:

```sh
install -Dm755 baz ~/.local/bin/baz
```

The Linux archive also carries the desktop entry, the AppStream metadata and
the icons, for a menu entry with baz's own artwork and working media-key
integration:

```sh
install -Dm644 io.github.mattcree.baz.desktop \
  ~/.local/share/applications/io.github.mattcree.baz.desktop
install -Dm644 io.github.mattcree.baz.metainfo.xml \
  ~/.local/share/metainfo/io.github.mattcree.baz.metainfo.xml
cp -r icons/. ~/.local/share/icons/hicolor/
gtk-update-icon-cache -f -t ~/.local/share/icons/hicolor 2>/dev/null || true
```

The `icons/` directory in the archive is already in the hicolor layout, so the
copy is the whole install; the entry's `Icon=io.github.mattcree.baz` then
resolves at whatever size the launcher asks for.
`gtk-update-icon-cache` only makes the lookup faster — the icon resolves
without it.

`Exec=baz` in that entry assumes the binary is on `PATH`; edit it to an
absolute path if you put it elsewhere.

### macOS

**Take the `.dmg`.** Open it and drag baz onto the Applications shortcut
beside it — the gesture the window is laid out to teach. The `.zip` below is
the same bundle for anyone who would rather place it themselves.

The macOS download contains **`baz.app`**, an ordinary application bundle.
Drag it to `/Applications` — or run it from wherever you unpacked it — and it
will carry baz's own mark in Finder, the Dock and Launchpad. The local Vibe
models travel inside the bundle, so there is nothing to place beside it.

**It is not signed or notarized**, and macOS will say so in a way that reads
like a fault. Gatekeeper attaches a quarantine flag to anything a browser
downloads, and for an unsigned app it refuses to open it with *"baz is damaged
and can't be opened."* **The app is not damaged**; that is the message macOS
uses for this case. Two ways through it:

- **Right-click the app and choose Open**, then confirm. macOS remembers the
  choice, so this is once per download.
- Or clear the flag yourself: `xattr -dr com.apple.quarantine
  /Applications/baz.app`.

**Updates do not have this problem.** baz downloads the next disk image
itself, compares its SHA-256 against the checksums published beside the
release, and clears the quarantine flag before opening it — so what you drag
out is not quarantined and Gatekeeper does not object. baz only does that
because it has just proved the bytes are the published ones; it is completing a
check macOS cannot perform for an unsigned application, not skipping one. If
baz is ever signed and notarised this stops happening, because Gatekeeper will
pass it on its own.

Verify the download's SHA-256 against `SHA256SUMS` first if you would rather
not take either on trust.

### Windows

**Take the `.msi`.** It installs baz into `Program Files`, adds a Start-menu
entry, and appears in Add/Remove Programs so it can be uninstalled the
ordinary way. Installing a newer `.msi` over an older one **replaces** it
rather than installing beside it, which is the whole of updating on Windows;
`winget upgrade` does the same thing once baz is listed there. The `.zip` is
the bare executable, for anyone who wants no installer at all.

SmartScreen will warn, for the same reason: the installer is unsigned. *More
info* → *Run anyway*.

**Neither is a formality being skipped.** Signing needs a paid Apple Developer
account and a Windows code-signing certificate — a cost and an identity
decision the project has not made. The sums are produced by the same public CI
run that built the binaries, from a tag, and the workflow that did it is
`.github/workflows/release.yml`.

**Linux runtime requirements**, read off the shipped binary rather than
assumed. It links four libraries — `libasound.so.2`, `libc`, `libm`,
`libgcc_s` — so glibc (the artifact is a `gnu` target, not `musl`) and ALSA are
the hard requirements. It has **no build-time dependency on any GUI library**:
nothing GTK, Qt or webview is involved, SQLite is compiled in, and the toolkit
is Rust.

That is not the same as needing no GUI libraries at runtime. winit opens the
display by `dlopen`, so the binary also wants, at the moment it opens a window:
`libxkbcommon.so.0` always, plus `libwayland-client.so.0` on a Wayland session
or `libX11.so.6` and `libxcb.so.1` on X11, and `libEGL.so.1` and a working
Vulkan or GL driver for wgpu to render through. Every desktop Linux install
already has all of these — a headless server does not, and neither does a
minimal container.

## From source

This is the only way to run baz today, and it is not hard — one system package
on Linux, and nothing at all on macOS or Windows.

```sh
git clone https://github.com/mattcree/baz
cd baz
cargo build --release --locked -p baz
./target/release/baz [MUSIC_DIR]
```

The toolchain is pinned in `rust-toolchain.toml`, so `rustup` installs the
right compiler for you; the minimum supported version is in `Cargo.toml`.

Audio output is an unconditional part of the GUI binary. Building it needs
the platform's audio headers, which the project's primary development host
does not have outside the `baz-dev` toolbox. There is no library-only GUI
variant with its playback controls removed.

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

Nothing is written next to your music. baz uses the platform's own locations —
`dirs::config_dir()` and `dirs::data_dir()`, each with a `baz` folder inside:

| | Linux | macOS | Windows |
|---|---|---|---|
| Config | `~/.config/baz/config.toml` | `~/Library/Application Support/baz/config.toml` | `%APPDATA%\baz\config.toml` |
| Library database | `~/.local/share/baz/library.db` | `~/Library/Application Support/baz/library.db` | `%APPDATA%\baz\library.db` |
| Playlists | `~/.local/share/baz/playlists/` | `~/Library/Application Support/baz/playlists/` | `%APPDATA%\baz\playlists\` |
| Play history | `~/.local/share/baz/history.tsv` | `~/Library/Application Support/baz/history.tsv` | `%APPDATA%\baz\history.tsv` |

`%APPDATA%` is the **roaming** one (`FOLDERID_RoamingAppData`,
`C:\Users\<you>\AppData\Roaming`), for both columns. On Linux the two respect
`$XDG_CONFIG_HOME` and `$XDG_DATA_HOME` when they are set. macOS has one
directory where the other platforms have two, which is Apple's arrangement, not
baz's — config and data share `Application Support` there.

**Your playlists are ordinary files.** `playlists/` holds one UTF-8 `.m3u8` per
playlist, written by baz and readable by anything (ADR-0024); `.m3u` files
dropped in beside them are read but never rewritten. That folder is not
configurable today. Back it up like any other folder of documents — nothing
else in this table is worth backing up.

Under Flatpak all of it lives under `~/.var/app/io.github.mattcree.baz/`, in
`config/baz/` and `data/baz/` respectively. Deleting the database costs you one
rescan and nothing else — your files are the source of truth, and baz never
edits them. Deleting `playlists/` costs you your playlists, because they are
the only copy.

## How baz draws, and how to make it draw differently

**baz is GPU-accelerated already, and it falls back on its own.** The shell is
iced 0.14 with both renderers compiled in — `wgpu` (Vulkan, Metal, DX12 or GL)
and `tiny-skia` (CPU) — and iced's fallback compositor tries the GPU first and
the software path second when no usable adapter answers
(`iced_renderer-0.13.0/src/fallback.rs:214–262`). There is nothing to switch on.

There is no setting for it, on purpose: the automatic fallback already covers
the case a setting would exist for, and a renderer picker is exactly the kind
of tenant a Settings place accretes. What is offered instead is an **escape
hatch by environment variable**, for the one case the fallback cannot detect —
a GPU path that is present and *bad* (a driver that tears, a hybrid laptop
whose discrete card spins up for a music player):

```sh
ICED_BACKEND=tiny-skia baz   # force the CPU renderer
ICED_BACKEND=wgpu baz        # force the GPU renderer, and fail loudly if it cannot
WGPU_BACKEND=gl baz          # keep the GPU path but pick the API yourself
                             # (vulkan · metal · dx12 · gl)
```

`ICED_BACKEND` takes a comma-separated list and tries each in turn. Both are
read once, when the window opens; there is no way to change renderer without
restarting, which is also why this is not a toggle in Settings.
