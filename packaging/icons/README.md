# The application icon

baz uses the supplied **red transparent-circle** mark everywhere an
application identity is needed.

```
crates/baz/assets/icons/logo-transparent-circle-*.svg
    supplied colour variants, retained as source artwork
crates/baz/assets/icons/logo-transparent-circle-red.svg
    canonical artwork
crates/baz/assets/icons/logo-transparent-circle-red.png
    embedded by the running app for its app-bar and window icon
crates/baz/assets/icons/logo-transparent-circle-red.ico
    embedded in native Windows baz.exe files by crates/baz/build.rs
hicolor/scalable/apps/io.github.mattcree.baz.svg
    installable copy of the canonical artwork
hicolor/{16,24,32,48,64,128,256,512}x…/apps/…baz.png
    committed hicolor raster ladder
io.github.mattcree.baz.icns
    the macOS bundle's icon, ten sizes in Apple's container
```

The hicolor basename is the application id. It lets the desktop entry say
`Icon=io.github.mattcree.baz` and lets Linux launchers, Flatpak, and AppStream
resolve the right asset.

## Regenerating the install assets

```sh
toolbox run -c baz-dev packaging/icons/render.sh
```

The script copies the canonical red SVG into the hicolor scalable path and
renders every committed PNG rung. It requires ImageMagick with librsvg; it
refuses ImageMagick's incomplete MSVG delegate. PNGs are committed so package
builds do not need a rasterizer or network access.

## The macOS `.icns`

`render.sh` writes it from the same master, and it is **committed** for the
same reason the PNG ladder is: a release runner should not need a rasterizer,
and Apple's `iconutil` exists only on macOS — generating it there would mean
the artwork was produced by a tool nobody can run while reviewing the change.

The container is written directly. `.icns` is small and documented — an `icns`
magic, a big-endian total length, then typed chunks whose payloads are
ordinary PNGs — and the script emits the ten types `iconutil` produces from a
complete `.iconset` (`icp4` `ic11` `icp5` `ic12` `ic07` `ic13` `ic08` `ic14`
`ic09` `ic10`), so a Mac reads exactly what it would have read from Apple's
own tool.

It then parses its own output back and checks each chunk's **actual pixel
size**, from the PNG's own IHDR, against the type that claims it. A 512-type
chunk holding a 128 px payload is a file that opens, validates by name, and
draws a blurry icon — which is precisely the class of silent failure this
whole file exists to avoid.

## Window and executable icons

`app.rs` decodes the 64 px PNG into `window::Settings::icon`, which supplies
the red mark on Windows and X11. Wayland does not support per-window icons;
there the desktop entry resolves the hicolor icon above. `build.rs` uses the
multi-resolution ICO to embed the same mark in Windows `baz.exe` release
builds, including the release workflow's native Windows artifact.

[freedesktop icon theme]: https://specifications.freedesktop.org/icon-theme-spec/latest/
