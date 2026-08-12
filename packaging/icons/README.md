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

## Window and executable icons

`app.rs` decodes the 64 px PNG into `window::Settings::icon`, which supplies
the red mark on Windows and X11. Wayland does not support per-window icons;
there the desktop entry resolves the hicolor icon above. `build.rs` uses the
multi-resolution ICO to embed the same mark in Windows `baz.exe` release
builds, including the release workflow's native Windows artifact.

[freedesktop icon theme]: https://specifications.freedesktop.org/icon-theme-spec/latest/
