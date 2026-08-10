# ADR-0027: Undo for list edits, and the trash for playlist deletion

**Status**: accepted (2026-08-09) · ships doc 11 §5 P2 (adopt-modified) ·
overturns no standing rule entry — forgiveness was absent, not refused · extends
ADR-0014's and ADR-0024's edit surfaces without changing an engine command,
a protocol message, or a play gesture · retires the playlist delete confirm
of ADR-0024 §4 · adds one vetted crate (`trash`), priced below the way
ADR-0025 priced `rfd`.

## Context

The Jobs-era critique's largest single finding (doc 11 §2.11): **the product
contained no reversal of any kind.** No undo message, no undo stack, no
trash, no restore. Its two destructive-edit surfaces — the queue and the
playlist page — shipped instant irreversible edits; its one deletion shipped
a confirmation. The 1992 HIG's forgiveness principle ranks the mechanisms
the other way: reversibility first, and a warning only for the case undo
cannot reach.

What made the repair cheap is that the architecture had already paid for it:
every queue edit is a whole-list `UpdateQueue` computed by pure functions
from the previous list (ADR-0014), and every playlist edit is an atomic
whole-file rewrite (ADR-0024 §2). The previous state was a value the code
held in its hand and dropped on every edit.

## Decision

### 1. A bounded edit history per list surface

`crates/baz/src/undo.rs` — a bounded (depth 8) stack of whole-list
snapshots, one history per surface, never a global system:

- **The Queue place** keeps the run as it stood before each remove, reorder
  and append. `Undo` restores the snapshot as `UpdateQueue` — ADR-0014's
  no-sample-disturbed edit — with no `Play`, no `SetQueue`, no `JumpTo`
  anywhere on the path: **the list comes back, never the playback
  position, and nothing ever sounds because of an undo** (pinned by
  `an_undo_restores_the_list_and_never_sounds`). Provenance travels in the
  snapshot like every other field.
- **The open playlist page** keeps the file's item list before each remove,
  reorder, and append that lands in it. `Undo` is one atomic whole-file
  rewrite through the **same fingerprint guard as the edit it reverses**: a
  file edited under baz refuses the restore, re-reads the disk's truth, and
  drops the whole stale history — baz's memory never overwrites somebody
  else's edit.

The visible control is a transient word — `Undo` — beside the Queue place's
summary and the playlist page's counts, present exactly while there is an
edit to take back: no toast, no popover, no timer, a word in a strip in the
product's own grammar. <kbd>Ctrl</kbd>+<kbd>Z</kbd> is its accelerator,
legal because the visible twin exists (doc 09 §5.2's construction; the
keyboard sweep names it). A history ends three ways (P2's clause): the next
edit replaces its top, leaving the surface clears it, and the run ending
clears the queue's.

P2 prescribed one-deep; this ships the same design with a small bound
(eight) because the mechanism is identical and a second mis-press is as
real as a first. **Not undone, by P2's own scope**: playback acts (play,
seek, volume are not destructive — the era never undid Play either), renames
(a rename mints a new page identity), and deletion (the trash is its undo).

### 2. Playlist `Delete` moves the file to the platform trash

`Folder::delete_to_trash` (baz-core) moves the `.m3u8` to the freedesktop
trash — `$XDG_DATA_HOME/Trash`, with the spec's `.trashinfo` beside it, so
every desktop file manager can Restore it. The page's `Delete` is now **one
press**, and the two-press confirm with its genuinely excellent sentence —
*"The file goes; your music stays."* — is retired with honour: the trash
keeps the promise the sentence made. A refusal from the trash layer leaves
the file exactly where it was; nothing falls back to unlinking. The plain
`Folder::delete` survives as library API for tools and tests that mean
*remove, now, here*.

**The dependency, priced** (the `rfd` discipline): `trash` 5.2.6, MIT
OR Apache-2.0. On the compiled Linux graph it adds `chrono`, `urlencoding`
and `scopeguard`; the remaining lock growth (fourteen entries total) is
Windows/macOS target-gated (`windows`, duplicate `windows-core` versions —
`multiple-versions = "warn"` stands). `cargo deny check` green with no
policy change; `packaging/flatpak/cargo-sources.json` regenerated
additively, `check-cargo-sources.py` green.

**The test isolation rule extends to the trash.** The behaviour is pinned by
`crates/baz-core/tests/trash.rs`, a one-test binary that redirects
`XDG_DATA_HOME`/`HOME` into its tempdir before any thread exists — the
six-variable isolation of `docs/DEVELOPMENT.md`, at test scale. The shell's
`delete_open` reaches the trash through an injected seam so tempdir fixtures
unlink instead of writing outside their own directory; the wiring is pinned
by `the_product_deletes_to_the_trash_and_the_tests_do_not`.

## Consequences

- Every destructive list edit in the product is reversible: two surfaces
  gain undo, the one deletion gains the desktop's own restore, and the
  product's last routine confirmation dialog is gone — the alert-box count
  the 1992 HIG treats as a design smell is now zero.
- Esc's peel loses a layer (the armed delete no longer exists to peel).
- L8/L9 clearance (P2's own analysis): the word reads the place's own edit
  history — subject = the place, resident while band-B-frequent — and one
  short word lands in strips that were near-empty.
- The queue undo's clear-on-run-end means a history can never resurrect a
  run the engine has finished; the playlist undo's clear-on-leave means a
  snapshot can never be applied to a page that is not on screen.
