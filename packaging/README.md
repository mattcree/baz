# Packaging

Files a distribution or an installer needs, kept in the repository so they are
reviewed like code rather than invented per-packager.

The normal Baz build includes Vibe and its local audio analysis. The
`--no-default-features` build exists only as a development check that the
optional analysis dependency remains isolated; it is not a separately named or
distributed product.

## `flatpak/`

The Flatpak manifest and the AppStream metadata, plus the submission path to
Flathub. See [`flatpak/README.md`](flatpak/README.md).

## `macos/`

The macOS application bundle: `Info.plist.in` and the `bundle.sh` that
assembles `baz.app` around a built binary. macOS keeps an application's icon,
name and permission strings in the bundle rather than in the executable, so a
bare Mach-O has nowhere to put them. See [`macos/README.md`](macos/README.md),
which also says why the bundle is unsigned and what signing it would take.

## `icons/`

The application icon: an SVG master and the PNG ladder, in the freedesktop
hicolor layout. See [`icons/README.md`](icons/README.md) for what the mark is,
where every colour in it comes from, and how to regenerate the PNGs.

## `io.github.mattcree.baz.desktop`

The freedesktop [desktop entry] for the application. Install it as:

```
/usr/share/applications/io.github.mattcree.baz.desktop        # system-wide
~/.local/share/applications/io.github.mattcree.baz.desktop    # per-user
```

The basename **must** stay `io.github.mattcree.baz.desktop`. Five things agree
on that one string and a desktop matches them against each other:

| Where | What it is |
|---|---|
| `packaging/io.github.mattcree.baz.desktop` | the file name |
| `DesktopEntry` on `org.mpris.MediaPlayer2` | `DESKTOP_ENTRY` (`crates/baz/src/mpris/mod.rs`) |
| the window's Wayland `app_id` / X11 `WM_CLASS` | the same constant (`app::window_settings`) |
| `<id>` in the AppStream metainfo | `packaging/flatpak/` |
| `id:` in the Flatpak manifest | `packaging/flatpak/` |
| the icon files' basename | `packaging/icons/hicolor/*/apps/`, `packaging/icons/*.icns` |
| `CFBundleIdentifier` and `CFBundleIconFile` | `packaging/macos/Info.plist.in`, via `bundle.sh`'s one literal |

That is how GNOME's and KDE's media widgets find the player's name and icon
from an MPRIS connection, and how a launcher knows the running window belongs
to the entry it launched. Change one and all five change together — CI's
`packaging` job fails the build if they disagree.

It is reverse-DNS rather than the bare `baz` it once was because Flatpak
requires that of an application id, and there is no version of this that works
with two different names. The MPRIS **bus** name is unaffected: it is the
spec's, and remains `org.mpris.MediaPlayer2.baz`.

`Exec=baz` assumes the binary is on `PATH`; a packager installing elsewhere
should write the absolute path. The optional `baz [MUSIC_DIR]` argument is
deliberately not exposed here — the entry launches the app, which remembers
the folder itself.

`Icon=io.github.mattcree.baz` is a **bare name, not a path**, which is what the
desktop entry spec wants: the launcher resolves it against the icon theme and
picks the size it needs. The files it resolves to are `packaging/icons/`
(`icons/README.md`), and the Flatpak, the release tarball and a by-hand install
each put them in the hicolor tree. Note that `crates/baz/src/icon.rs` is
unrelated — that is the in-UI transport glyph sheet, drawn in code.

### What the entry deliberately omits

- **`MimeType=`** — baz implements no `OpenUri` and registers no file
  associations, so claiming to handle `audio/flac` would put it in "Open
  with…" menus where it would do nothing. Same reasoning as the empty
  `SupportedUriSchemes`/`SupportedMimeTypes` MPRIS properties; see
  `crates/baz/src/mpris/mod.rs`.

Validate with `desktop-file-validate packaging/io.github.mattcree.baz.desktop`
after any edit; CI does the same on every pull request.

[desktop entry]: https://specifications.freedesktop.org/desktop-entry-spec/latest/
