# Packaging

Files a distribution or an installer needs, kept in the repository so they are
reviewed like code rather than invented per-packager.

## `baz.desktop`

The freedesktop [desktop entry] for the application. Install it as:

```
/usr/share/applications/baz.desktop          # system-wide
~/.local/share/applications/baz.desktop      # per-user
```

The basename **must** stay `baz.desktop`. Three things agree on it and a
desktop matches them against each other:

| Where | What it is |
|---|---|
| `packaging/baz.desktop` | the file name |
| `DesktopEntry` on `org.mpris.MediaPlayer2` | `"baz"` (`crates/baz/src/mpris/`) |
| the window's Wayland `app_id` / X11 `WM_CLASS` | `"baz"` (`app::window_settings`) |

That is how GNOME's and KDE's media widgets find the player's name and icon
from an MPRIS connection, and how a launcher knows the running window belongs
to the entry it launched. Change one and all three change together.

`Exec=baz` assumes the binary is on `PATH`; a packager installing elsewhere
should write the absolute path. The optional `baz [MUSIC_DIR]` argument is
deliberately not exposed here — the entry launches the app, which remembers
the folder itself.

### What the entry deliberately omits

- **`Icon=`** — baz ships no application icon yet (`crates/baz/src/icon.rs` is
  the in-UI transport glyph sheet, drawn in code, not an app icon). An `Icon=`
  key naming a file no package installs is worse than none: launchers show a
  broken image rather than their own placeholder. Tracked in
  `docs/BACKLOG.md`; add the key in the same change that adds the artwork.
- **`MimeType=`** — baz implements no `OpenUri` and registers no file
  associations, so claiming to handle `audio/flac` would put it in "Open
  with…" menus where it would do nothing. Same reasoning as the empty
  `SupportedUriSchemes`/`SupportedMimeTypes` MPRIS properties; see
  `crates/baz/src/mpris/mod.rs`.

Validate with `desktop-file-validate packaging/baz.desktop` after any edit.

[desktop entry]: https://specifications.freedesktop.org/desktop-entry-spec/latest/
