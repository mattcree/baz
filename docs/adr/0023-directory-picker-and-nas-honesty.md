# ADR-0023: A directory picker, and the NAS as an ordinary folder

**Status**: accepted (2026-08-09) · **supersedes the no-dialog clause of
ADR-0022 §8** · closes `docs/BACKLOG.md`'s *"Music folders are typed, not
picked"*

## Context

The owner asked:

> *"we need a dir picker for the library option and we should allow a NAS to
> be the target."*

Two asks, and they are smaller than they look — because ADR-0022 built most of
the second one, and the argument that blocked the first one has since
dissolved upstream.

**The picker was refused once, on stated grounds.** ADR-0022 §8: *"baz takes
no dialog dependency (`rfd` and the portal behind it are not in the graph)"* —
and when that sentence was written it named a real cost. `rfd` 0.15's portal
backend rode on `ashpd`, which chose between `async-std` (a runtime baz does
not run, since declared unmaintained) and a `tokio` feature whose zbus
coupling could have re-plumbed the MPRIS connection's executor underneath it.
The alternative backend was `gtk3`, i.e. `gtk-sys`, i.e. the end of the
zero-system-deps Linux build.

**That cost was re-measured, not re-asserted.** `rfd` 0.17 dropped `ashpd`
entirely: the `xdg-portal` backend is now rfd's own small D-Bus client, driven
by `pollster::block_on`, with no runtime coupling and no zbus in its tree. The
measured graph cost on Linux, with `default-features = false, features =
["xdg-portal"]`:

```
rfd v0.17.2
├── libc, log, percent-encoding, raw-window-handle   (already in the lock)
└── pollster v0.4.0                                   (new; MIT/Apache-2.0)
```

Four new packages across the whole six-target lock (`rfd`, `pollster`, and the
macOS pair `block2`/`objc2-app-kit`), all on the existing licence allowlist.
`cargo deny check` passes with **no** policy change: advisories ok, bans ok,
licenses ok, sources ok. `gtk-sys` is nowhere; MPRIS's zbus 4.4 is untouched.
The refusal's grounds are gone, so the refusal goes with them — by ADR, which
is what the ledger's editing rule demands.

## Decision

### 1. `Browse…` beside the well — a second door, not a replacement

The Settings place's add-a-folder row gains one `Browse…` control (the
ellipsis is the convention meaning "a dialog follows"). It opens the desktop's
own directory picker: on Linux, the XDG desktop portal — GNOME's dialog on
GNOME, KDE's on KDE, and the only mechanism a future Flatpak would be
*permitted* to use.

**The typed path stays, and not out of caution.** A dialog can only offer what
the filesystem shows it, and the folder most worth typing is exactly the one a
dialog cannot show: the share a listener knows by heart but has not mounted
today. The refusals ledger's rule — every act a visible pointer target — is
met by each door on its own, so a desktop with no portal service (a bare
window manager, a broken `xdg-desktop-portal`) loses a convenience and no
capability: the dialog answers `None`, which lands as a dismissal, and the
well is still there.

**The event loop never waits on it.** `FileDialog::pick_folder` blocks its
thread until the dialog closes, so it runs on tokio's blocking pool via the
same `Task::perform` + `spawn_blocking` shape the thumbnail decodes use, and
returns as a message (`MusicFolderPicked`). Dismissal decides nothing and
therefore changes nothing — not even the text already typed in the well. The
`rfd` call is one small function (`app::pick_folder`), deliberately the only
uncovered line of the feature: everything before it is message plumbing and
everything after it is the acceptance path both doors share, and both of those
are pinned by test.

### 2. The NAS is a path, and the work is honesty, not protocol

No smb/nfs client code. A mounted share **is** a folder (`/mnt/nas`, a gvfs
path, an autofs point), and what a network folder actually demands is that baz
tell the truth in the three moments a local folder never has:

- **When it is gone.** Already held, by construction: ADR-0022's removal gates
  mean an unavailable root's walk refuses (`RootUnavailable`), refusal hands
  the removal pass nothing, and *nothing is pruned from any root* — while the
  Settings place says, per folder, that nothing was removed from it. This ADR
  adds no mechanism there; it adds the **tests** that pin the whole lifecycle,
  which existed only in parts:
  - worker-level (`baz::scan`): scanned present → unmounted (the fixture tree
    renamed away) → rescan removes nothing and reports the root unavailable →
    remounted → rescan reports every row *unchanged* — nothing re-read,
    nothing re-added, so duplicates are structurally impossible;
  - library-level (`baz-core/tests/index.rs`): across the same outage the
    rows, their stamps, their recorded root, the folder's last-scan record and
    every album's `first_seen_ns` are identical before and after — returning
    is not arriving.
- **When it is slow or dead.** The scan already runs wholly on the `baz-scan`
  worker; a hung `readdir` on a stale mount costs a pass, never a frame. The
  one filesystem wait left on the UI thread was ADR-0022's own `dir.is_dir()`
  in the add-a-folder path — one `stat`, which against a dead hard mount does
  not fail but *waits*, for as long as the mount options say. It moves to the
  blocking pool (`check_folder`), returning as a message with the same words
  on refusal. A folder arriving from the picker skips the re-stat entirely:
  the dialog walked the real filesystem to offer it, and re-checking would put
  the wait back.
- **In the Settings place.** Unchanged, verified against the visual language:
  the unavailable line ("Not reachable right now — N tracks kept, nothing
  removed.") is quiet ink, not `ALERT`, per the §5 rule that alert is for
  problems and never for the merely unusual — the same info-not-warning stance
  the resampling indicator set.

### 3. What was deliberately not taken

- **`rfd`'s `wayland` feature** (dialog parenting via `raw-window-handle`).
  It pulls `wayland-client`/`wayland-protocols` into rfd's own tree and needs
  the window handle threaded from iced into the call; the portal centres an
  unparented dialog acceptably. Worth revisiting if a compositor misplaces it.
- **The `gtk3` fallback**, permanently: `gtk-sys` is the cost ADR-0022 §8 was
  right about.
- **The picker on the first-run screen.** One question, one field is that
  screen's whole design, and its validation still stats on the UI thread — at
  first run there is no configured NAS to hang on. If the picker earns its
  place there, it earns a look of its own.
- **A "reconnect" or mount-triggering control.** baz does not mount
  filesystems; it reports them. The periodic rescan (ADR-0022 §6) already
  notices a returned share within five minutes, and a force sync notices it on
  a press.

## Consequences

- Adding a folder is a dialog *or* a typed path; both land in one acceptance
  path (`accept_folder`) that dedupes, persists, adopts and scans.
- A dead mount can stall a scan pass and a pool thread, and nothing else. The
  UI thread performs no filesystem waits on any library-management path.
- The unavailable-root lifecycle is pinned end to end by tests at both layers,
  so the "a scan that couldn't see a root must never prune it" guarantee can
  no longer regress silently.
- `rfd` 0.17 (portal-only) is in the graph: one new crate on Linux, four in
  the lock, `cargo deny` green with no policy change. ADR-0022 §8's no-dialog
  clause is superseded; its typed-path affordance is load-bearing and stays.
