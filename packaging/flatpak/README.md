# Flatpak

Flatpak is the intended way an ordinary person installs baz on Linux: one
package that works on every distribution, sandboxed, with the desktop
integration wired up.

**baz is not on Flathub.** Nothing here has been submitted. This directory is
the machinery, kept in the repository so it is reviewed like code.

| File | What it is |
|---|---|
| `io.github.mattcree.baz.yml` | the manifest — runtime, permissions, build commands |
| `io.github.mattcree.baz.metainfo.xml` | AppStream metadata: the store listing, and what GNOME Software and KDE Discover read |
| `cargo-sources.json` | every crate in `Cargo.lock` as a URL and a SHA-256; **generated** |
| `check-cargo-sources.py` | proves the above still matches `Cargo.lock`; run by CI |

The desktop entry lives one directory up, in `packaging/`, because it is not
Flatpak-specific.

## The application id

**`io.github.mattcree.baz`**, from the repository home
`github.com/mattcree/baz` — the form ADR-0002 anticipated once the home was
fixed. Flathub requires a reverse-DNS id it can attribute to something you
demonstrably control, and `io.github.<owner>.<repo>` is the standard proof for
a project hosted on GitHub.

That one string is five things at once, and a desktop matches them against each
other:

| Where | What |
|---|---|
| this manifest | `id:` |
| `io.github.mattcree.baz.metainfo.xml` | `<id>` |
| `packaging/io.github.mattcree.baz.desktop` | the file's basename |
| `crates/baz/src/mpris/mod.rs` | `DESKTOP_ENTRY`, which MPRIS advertises |
| `crates/baz/src/app.rs` | the window's Wayland `app_id` / X11 `WM_CLASS` |

Change one and all five change together. CI's `packaging` job asserts they are
equal on every pull request, so drift fails the build rather than the store
listing.

The MPRIS **bus** name is a separate thing and stays `org.mpris.MediaPlayer2.baz`
— that name belongs to the MPRIS spec, not to the desktop.

## Regenerating `cargo-sources.json`

A Flathub build has no network, so every dependency must be declared. The list
is generated from `Cargo.lock` by flatpak-builder-tools and must be
regenerated in the same commit as any `Cargo.lock` change:

```sh
python3 -m venv /tmp/fcg && /tmp/fcg/bin/pip install tomlkit aiohttp
curl -sLO https://raw.githubusercontent.com/flatpak/flatpak-builder-tools/master/cargo/flatpak-cargo-generator.py
/tmp/fcg/bin/python flatpak-cargo-generator.py Cargo.lock \
  -o packaging/flatpak/cargo-sources.json
python3 packaging/flatpak/check-cargo-sources.py
```

Every crate baz depends on comes from crates.io and carries a checksum in
`Cargo.lock`, so the generator computes the list without downloading anything.

## Validating locally

```sh
appstreamcli validate --no-net io.github.mattcree.baz.metainfo.xml
desktop-file-validate ../io.github.mattcree.baz.desktop
flatpak-builder --show-deps io.github.mattcree.baz.yml
python3 check-cargo-sources.py
```

CI runs the first, second and fourth on every pull request. `--show-deps` is
not run in CI: it needs the flatpak toolchain on the runner and adds no signal
the YAML parse does not already give.

## Building it

The manifest builds a **tagged** commit, which is what Flathub requires and
which means it does not build your working tree. To build the checkout you
have, replace the `git` source with a `dir` source:

```yaml
- type: dir
  path: ../..
```

and drop `--locked` if you have uncommitted lockfile changes. Then:

```sh
flatpak install flathub org.freedesktop.Platform//25.08 org.freedesktop.Sdk//25.08 \
                        org.freedesktop.Sdk.Extension.rust-stable//25.08
flatpak-builder --user --install --force-clean build-dir io.github.mattcree.baz.yml
flatpak run io.github.mattcree.baz
```

This has not been done yet: no Flatpak of baz has been built, and the build
commands in the manifest are unexercised. Expect the first build to find
something.

## Permissions, and why each one

| Permission | Why |
|---|---|
| `--socket=wayland`, `--socket=fallback-x11`, `--share=ipc`, `--device=dri` | iced renders through wgpu and needs the GPU node; X11 is the fallback for sessions with no compositor |
| `--socket=pulseaudio` | cpal opens ALSA's `default` PCM, which the runtime's ALSA configuration routes to this socket |
| `--filesystem=xdg-music:ro` | the music. Read-only, because baz never writes to a listener's files — the sandbox is the place to make that structural rather than promised |
| `--own-name=org.mpris.MediaPlayer2.baz` and `.baz.*` | the MPRIS name, and the per-instance fallback a second copy claims |

No network, no home directory, no device access beyond the GPU node.

The narrow `xdg-music` grant is a real limitation: baz's first-run screen takes
a **typed path** rather than opening a portal file chooser, so a collection
outside `~/Music` needs an explicit override:

```sh
flatpak override --user --filesystem=/srv/music:ro io.github.mattcree.baz
```

A portal-based folder chooser would remove the need for any filesystem
permission at all and is the right fix; it is not built.

## Submitting to Flathub

Not done, and not to be done without the maintainer's decision. The path, for
when it is:

1. **Make the placeholders real.** The manifest's `tag` and `commit` must name
   an actual release; the metainfo's `<releases>` entry must be that release
   with its real date; and the screenshot must exist. Flathub rejects a
   submission whose screenshot URL does not resolve, and a store listing with
   no picture of the application is not a listing anyone installs from. That
   means committing at least one screenshot to the repository — 16:9-ish, the
   default light theme, an actual library rather than an empty state — and
   pointing `<image>` at its raw URL.
2. **Check it the way Flathub will.** Run `flatpak run
   org.flathub.flatpak-external-data-checker` if any source ever gains an
   external URL, and build the manifest clean (`--force-clean`, no network) on
   a machine that has never built baz before.
3. **Open the submission.** Fork `flathub/flathub`, create a branch named
   `io.github.mattcree.baz`, add the manifest, the metainfo and
   `cargo-sources.json` at the repository root, and open a pull request against
   the `new-pr` branch. The Flathub build bot builds it and comments; a
   reviewer then looks at the permissions and the metadata.
4. **Expect questions about permissions.** `--filesystem=xdg-music:ro` is
   modest and should pass without argument. If a reviewer asks why there is no
   file chooser, the answer is the one above: there is no portal chooser yet,
   and the grant is read-only and narrow because of it.
5. **After acceptance**, the flathub repository becomes the place the manifest
   is edited for packaging changes, and this copy stays the upstream source of
   truth. Keep them in step deliberately; nothing enforces it across
   repositories.

Flathub's own documentation is at <https://docs.flathub.org/docs/for-app-authors/submission>.
