# ADR-0041: A library that will not open is a statement, not a first run

**Status**: accepted (2026-08-10) · **narrows [ADR-0025](0025-directory-picker-and-nas-honesty.md)'s first-run screen** to the case it was designed for · keeps [ADR-0028](0028-density-detents.md)'s *absent, not disabled* · keeps [ADR-0040](0040-the-app-bar.md)'s app bar out of a surface that is not a place · frames in `docs/design/impl/blocked-library/`

## Context

The owner, 2026-08-10, having run a two-day-old binary out of
`target/release/` against his current library:

> *"it shows me 'where's your music' which has no browse function and it also
> tells me the schema version is version 8 if I pick any directory"*

Both symptoms belonged to that stale build. `Browse…` shipped with ADR-0025
*after* it, and its `SCHEMA_VERSION` predated 8. **Nothing in the current
product was broken.** The *failure mode* is the defect, and it is in the
current product exactly as it was in that one.

**What happened underneath was correct.** `baz_core::index` refuses a database
whose `PRAGMA user_version` is ahead of the build's rather than migrating it,
which is the only thing that protects the newer install's data — a "migration"
downwards is a guess about a schema this code has never seen.

**What happened above it was one line.** `app.rs` answered *every* failure to
open the library the same way:

```rust
Err(error) => (Screen::Setup(Setup::fresh(Some(error))), Task::none()),
```

So a listener whose collection was exactly where they left it was shown
**"Where's your music?"** — a question they had already answered, on a screen
whose every control leads back into the identical refusal. That is why he saw
the version number *"if I pick any directory"*: each folder he named went
straight back through `Library::open` against the same file.

**Why this is a beta blocker.** The beta's bar is the owner's:

> *"you can find your music, play it, make lists of it, and **nothing baz does
> loses or corrupts anything**"*

A tester who installs a release and then runs an older build is not an edge
case — that is the shape of trying something and going back. They hit this and
reasonably conclude baz ate their library. It is the most alarming thing the
application can say, and it says it in the one case where nothing is wrong with
their data at all.

## The load-bearing fact, established first

**A refused open does not write a byte.** This had to be settled before any
words were chosen, because it is the difference between a presentation defect
and a data-loss one — and because the screen is about to tell a listener their
music and their playlists are untouched, which may not be a hope.

It is now true by construction and asserted by test:

- `Library::open` reads `user_version` and returns `SchemaTooNew` **before it
  sets a pragma**. `journal_mode` is persistent — on a database in some other
  mode it rewrites the file header — and a build that has decided not to touch
  a newer baz's library should not have changed it on the way to saying so.
  (Before this ADR the pragmas ran first. Nothing was lost by it, because a
  baz-written database is already in WAL, but "already in the mode we were
  about to set" is not a guarantee.)
- `a_too_new_database_is_refused_without_a_byte_being_written` opens a stamped
  database **three times**, which is the retry a listener performs by typing
  folders at a first-run screen, and compares the file's bytes each time. It
  then restores the version and reads every row back.
- `Shelf::open` creates the data directory and then opens the library. Its
  writes — `adopt_roots`, `persist_roots`, the scan — are all *after* the
  open, so a refused library also leaves `config.toml` alone.
- The session snapshot is not rewritten either: `next_snapshot` returns `None`
  until something has sounded, and nothing sounds on a screen with no library.
- The play-history ledger is opened and appended to, and that is safe by its
  own design: it is a line-oriented TSV that `HistoryLedger::open` only ever
  appends to, never rewrites.

## Decision

### 1. A distinct state — `Screen::Blocked`

The first-run screen keeps the case it was designed for: **a listener who has
not said where their music is.** Everything else gets a screen that makes a
statement instead of asking a question.

| `Screen::Setup` | `Screen::Blocked` |
|---|---|
| *Where's your music?* — a **question** | *Here is what happened* — a **statement** |
| The listener has not answered yet | The listener answered, and the answer is fine |
| Naming a folder is the fix | Naming a folder cannot help |

The seam that carries it is `Shelf::open`, whose error type changes from
`String` to `Blockage`. **A string cannot be routed**; that is the whole
reason the old line collapsed every failure into one screen.

### 2. One screen, three reasons — not four screens

`Blockage` has three variants and `views::blocked` has one layout. The
alternative — a screen each for the downgrade, the corrupt index and the
machine with nowhere to keep one — would be three surfaces obliged to keep
agreeing about a sentence all three must say identically.

| reason | what happened | disposition |
|---|---|---|
| `NewerBaz` | the database was written by a newer baz | **do not repair it**; put the newer baz back |
| `Unreadable` | permissions, a corrupt page, a truncated write, a full disk | a new index **is** the repair |
| `Nowhere` | no data directory, or one that cannot be created | nothing here can fix it; the environment can |

Every one of them says, in the same place and the same words, **"Your music
and your playlists are untouched"** — because in every one of them it is true,
and it is the sentence that stops somebody panicking. It is pinned by
`all_three_reasons_say_the_music_and_the_playlists_are_untouched`.

What differs is the rest of the words, and **which controls exist**.

### 3. The controls: absent, not disabled

ADR-0028's rule, kept, and here it decides both controls without appeal to
taste.

- **`Try again`** appears only where trying again could give a different
  answer. A schema version is the same number on the second read, so the
  downgrade does not get it; a permission, a lock or a missing directory can
  all be fixed from another window while this screen is up, so those do.
- **`Set this library aside…`** appears only where there is a file to move.

There is **no `theme::primary`** on this screen. That style is the lamp
outline and the lamp is reserved for playback truth; an amber control on the
one surface where nothing can play would spend a reserved signal on an apology.
Both controls are word buttons in `Browse…`'s anatomy.

### 4. Setting the library aside — the escape hatch, and its three fences

A listener who cannot get the newer baz back would otherwise have an
application that will not start. So there is a way out, and it is fenced:

1. **It never discards anything.** `baz_core::index::set_aside` *renames*
   `library.db` to `library.db.set-aside-1` — with its write-ahead log and
   shared-memory file, database first, so the worst interruption leaves no
   `library.db` at all rather than a fresh one wearing another library's log.
   Renaming it back restores it exactly, which
   `setting_a_library_aside_moves_it_whole_and_is_reversible` asserts as a
   round trip.
2. **It is never the default and never the only control.** It is the quiet
   word, and on `Unreadable` it stands second.
3. **The first press reveals; it does not act.** It shows a paragraph naming
   what a new index costs — *the ADDED dates: every record files under today* —
   and only then a word that acts, beside a way back. In `views::blocked::acts`
   the message that moves the file is **not in the returned list at all** until
   the cost is showing, so this is a property of the code and not of the
   layout.

On the downgrade the revealed paragraph opens by saying **"This is not the
fix"**, because it is not: the newer baz opens that library as it stands. A
screen that offered the two cases identically would be recommending the wrong
one half the time.

`first_seen_ns` is the reason the cost has to be named. It is the one column in
the schema that cannot be recovered by re-reading the files, `BACKLOG.md`
already carries it as a known loss, and a blithe *"start fresh"* would spend
the whole collection's worth of it on one press.

### 5. No app bar, and why

ADR-0040's bar is resident in all seven **places**. This is not a place and it
does not wear one — exactly as the first-run screen does not. The bar's zone 3
is the display options, which need a wall of records; its zone 4 is the door to
Settings, which is a place inside a library that has not opened. **A bar with
two dead zones states less than no bar**, and the window keeps the system's own
decorations to be closed by. The screen carries the wordmark, unlit, so it
still reads as baz rather than as a crash dialogue.

This is the honest reading of ADR-0040's own admission rule rather than an
exception to it: the rule admits a control that applies in *every place*, and
says nothing about a surface that is not one.

## What was considered and refused

- **A better sentence on the setup screen.** Refused. The screen's controls are
  the problem, not its copy: three doors that all lead back to the same
  refusal. The owner met that loop personally.
- **Offering nothing but "put the newer baz back".** Considered seriously, and
  it is the *correct* advice — but a listener who has lost the newer build
  would be left with an application that will not start and no way forward
  inside it. The set-aside exists for them, fenced as above, and says plainly
  that it is not the repair.
- **Deleting the database on "start over".** Refused outright. Renaming costs
  nothing, is reversible, and lets the screen say *nothing is deleted* and mean
  it.
- **A `Quit` button.** Refused. The window has the desktop's own close control
  and a Quit word on an error screen is a crash-dialogue tell — the thing the
  owner's aesthetics rule most directly forbids.
- **A separate screen for the corrupt index.** Refused; see §2. It is the same
  shape with different words and a different disposition, and one surface
  carries both without either drifting.
- **Doing anything about a music folder that has gone away.** Refused as
  out-of-scope, having been checked: it never reaches either screen. The
  library opens, the scan reports `RootUnavailable`, and the shelf says *"1
  folder is not reachable"* in the strip with every record still on the wall
  (ADR-0011's choice). That case is already answered.

## Consequences

- **`SCHEMA_VERSION` is public.** A front end has to be able to say which
  version this build speaks; reading it out of an error's `Display` string
  would be a front end parsing prose.
- **`Shelf::open` returns a type.** Any future failure worth distinguishing has
  somewhere to be distinguished, instead of being formatted into a sentence at
  the point where the information still exists.
- **`Blocked::new` is crate-visible rather than test-gated**, because several
  tests in `views` read `app.rs`'s source and stop at its first test attribute.
  A gated helper there silently truncates what they can see — which is not
  hypothetical; adding one blinded
  `every_place_that_hangs_works_hangs_them_on_one_grid`, and it failed rather
  than passing vacuously.
- **A downgrade is now survivable in both directions**, which is what the
  beta's promise required: the older build states the situation and changes
  nothing, and the newer build opens the same library it wrote.
