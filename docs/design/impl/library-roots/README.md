# Several music folders, on the real binary

Rendered from `target/release/baz` by `capture.sh` — headless, on a private
Xvfb, with all six XDG redirections from `docs/DEVELOPMENT.md`. Nothing touched
the maintainer's session; the run's receipt is the line the script prints:

```
[mpris] no session bus; desktop media controls unavailable (I/O error: No such file or directory (os error 2))
```

The fixture is `docs/design/composition/tools/mkfixture.sh` — 25 albums of
digital silence with generated covers — split across three real directories, so
every folder in these frames is a genuinely separate tree.

## What each frame shows

| frame | what it is |
|---|---|
| `01-wall-two-folders.png` | the wall, holding two folders' records at once |
| `02-library-section.png` | Settings → **Library**: each folder with its track count and last scan |
| `03-third-folder-added.png` | a third folder typed in and scanned, the other two untouched |
| `04-folder-unavailable-after-force-sync.png` | the second folder deleted from disk, then a force sync — it reads *"Not reachable right now — 81 tracks kept, nothing removed"* |
| `05-removal-armed.png` | the first press of Remove: the row names what the second press will do |
| `06-folder-forgotten.png` | the confirming press; the folder and its rows are gone |
| `07-wall-after-forgetting.png` | the wall, 64 tracks lighter, with the unreachable folder still counted |

## The run's own log, which is the actual evidence

```
[scan] /tmp/baz-roots-a: 64 added, 0 updated, 0 unchanged, 0 skipped
[scan] /tmp/baz-roots-b: 81 added, 0 updated, 0 unchanged, 0 skipped
[scan] done: 145 added, 0 updated, 0 unchanged, 0 removed, 0 files skipped, 0 folders unavailable

[config] holding /tmp/baz-roots-c
[scan] /tmp/baz-roots-a: 0 added, 0 updated, 64 unchanged, 0 skipped
[scan] /tmp/baz-roots-b: 0 added, 0 updated, 81 unchanged, 0 skipped
[scan] /tmp/baz-roots-c: 61 added, 0 updated, 0 unchanged, 0 skipped

[scan] force sync requested
[scan] /tmp/baz-roots-a: 0 added, 64 updated, 0 unchanged, 0 skipped
[scan] /tmp/baz-roots-c: 0 added, 61 updated, 0 unchanged, 0 skipped
[scan] done: 0 added, 125 updated, 0 unchanged, 0 removed, 0 files skipped, 1 folders unavailable
[scan] /tmp/baz-roots-b is unavailable: scan root `/tmp/baz-roots-b` does not exist
[scan] 206 tracks / 25 albums so far…

[index] 64 tracks forgotten with /tmp/baz-roots-a
```

Five claims, each visible in those lines or in the frames:

1. **Counts are per root.** Every pass reports each folder separately.
2. **Adding a folder is incremental.** The two folders baz already held came
   back `145 unchanged` — not one file was re-opened for them.
3. **A force sync ignores stamps.** The same files came back `125 updated,
   0 unchanged`.
4. **An absent folder prunes nothing, anywhere.** `/tmp/baz-roots-b` was deleted
   from disk between the two passes: `0 removed`, and the library still held
   `206 tracks` afterwards — its 81 rows intact.
5. **The status line stays on one line.** An unreachable folder is reported in
   the top bar as a fixed-width count (`1 folder is not reachable`) rather than
   by path: the strip is a single unwrapped row shared with the counts and
   `Settings`, and a message carrying `/mnt/nas/Music/Archive` wrapped it to two
   and pushed `Settings` off the frame. *Which* folder, and that nothing was
   removed from it, is said per folder in the Settings place instead. Frame 07
   is the check.

The first step of the run is <kbd>Ctrl</kbd>+<kbd>,</kbd>, and that is also a
check: before the type-anywhere work removed the launch focus, that chord was
typed into the search well instead of reaching the subscription. It now opens
the place.
