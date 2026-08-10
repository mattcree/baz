# A library from a newer baz is a statement, not a first run

> *"it shows me 'where's your music' which has no browse function and it also
> tells me the schema version is version 8 if I pick any directory"*
> — the owner, 2026-08-10

He had run a two-day-old binary against his current library. Both of the
symptoms he names belonged to that stale build — `Browse…` shipped after it,
and its `SCHEMA_VERSION` predated 8 — so nothing in the shipped product was
broken. **The failure mode is the defect**, and it was in the shipped product
exactly as it was in that one: baz refused a database written by a newer build
(correctly, and without touching it) and reported that by drawing the
**first-run screen**.

To a listener whose collection is exactly where they left it, that is the
application saying *your library does not exist* — the most alarming thing baz
can say, in the one case where nothing is wrong with their data at all.

ADR-0041 is the decision. This is the evidence.

## How these were made

`capture.sh`, headless on a private `Xvfb :197`, with all six XDG redirections
from `docs/DEVELOPMENT.md`. The `[mpris] no session bus` line from **every**
run is printed at the end of the script and is the receipt that nothing touched
the owner's session. The owner's real library at
`~/.local/share/baz/library.db` was never opened by anything here, and neither
was `~/Music`: the script builds its own 206-track library by scanning the
silent generated fixture from `docs/design/composition/tools/mkfixture.sh`, and
then stamps that database with `user_version = 10`.

**Two binaries, two filenames**, because a filename collision has measured the
wrong build on this project before:

| binary | sha256 (first 16) | what it is |
|---|---|---|
| `baz-before` | `02d4516d61b5a5d8` | `da6a547`, the merge of `feat/app-bar` |
| `baz-after` | `90af38b7b4054770` | this branch |

Both built inside the `baz-dev` toolbox with `--features device-output`, into
this worktree's own `target/tb` — a host-built binary links a newer glibc than
the container has and dies before it draws.

**Every press is placed from the frame that is on screen at the time**, never
from a constant: `ink_box` trims the block, `click_the_well` puts the pointer
95 px below its top, and `press_word` trims the bottom 32 px band on its own to
find the words' real left edge and width. Every transition is then *measured*
— `changed` demands more than 500 differing pixels, `same` demands fewer than
50 — and the pointer is parked at 20, 20 before each shot so that nothing is
photographed under a hover or mid-press.

Three false frames were produced and thrown away while writing this, all three
caught by those gates rather than by eye:

1. a `ctrl+a` before typing left a modifier latched and swallowed the whole
   path — two frames that differed by **one pixel** and claimed to show a
   folder being typed;
2. `magick import -window root` takes the X keyboard focus, so a `Return` sent
   after a screenshot went to the root window and submitted nothing — two
   **byte-identical** frames claiming to show a submission;
3. the gate itself read `1.86447e+08 (2845)` with `sed` and compared the
   leading field as an integer, getting *one* — the frames were right and the
   ruler was wrong, which threw away good evidence for a whole run.

## The receipts

**The database is not written to.** Its SHA-256 after the old build has been
run against it, and after the new one has been run and had its library set
aside and moved back, against the fingerprint taken the moment it was stamped:

```
  stamped from the future : 0ec9ac8f9fa05ba39408f0951273b9ad31caa350e6ae0d1551df61aa41142df8
  after the BEFORE binary : 0ec9ac8f9fa05ba39408f0951273b9ad31caa350e6ae0d1551df61aa41142df8
  after the AFTER binary  : 0ec9ac8f9fa05ba39408f0951273b9ad31caa350e6ae0d1551df61aa41142df8
```

That is *"your music and your playlists are untouched"* as a measurement. The
same fact is held by
`a_too_new_database_is_refused_without_a_byte_being_written` in
`crates/baz-core/tests/index.rs`, which opens a stamped database three times —
the retry a listener performs by typing folders at a first-run screen — and
compares the bytes after each.

**Nothing is deleted by setting the library aside.** The data directory after
frame 09:

```
history.tsv
library.db                  ← the new, empty index
library.db.set-aside-1      ← the newer baz's library, renamed and intact
library.db-shm
library.db-wal
playlists
[library] set aside to …/data/baz/library.db.set-aside-1
```

## The frames

| file | what it shows |
|---|---|
| `01-before-where-is-your-music.png` | **The defect.** The old build, launched with the folders it has always had, on a library that is right there: *"Where's your music?"*, and the schema version underneath it in alert ink. |
| `02-before-a-genuine-first-run.png` | The same build from a config naming no folder — the screen doing its actual job, with no error on it. This is what makes 04 evidence rather than a still life. |
| `03-before-a-folder-typed.png` | The fixture typed into its well. Still no error. |
| `04-before-any-directory-says-the-version.png` | **Submitted — and the version message arrives.** This is the *"it tells me the schema version if I pick any directory"* half: the folder was taken, went straight back into the same refusal, and the screen asked its question again. |
| `05-after-a-newer-baz.png` | **The state this branch adds.** What happened, what is safe, what to do, the two version numbers, where the file is, and one quiet word. No question mark on the screen. |
| `06-after-a-newer-baz-1920.png` | The same at 1920 × 1080 — a block centred in a window has to survive being given more window. |
| `07-after-what-starting-over-costs.png` | The second door **opened, not taken**. The first press reveals a paragraph that names what a new index costs (the ADDED dates) and says plainly that this is *not* the fix for a downgrade, and only then a word that acts — beside a way back. |
| `08-after-keeping-it.png` | `Keep it`. **Zero pixels** different from 05: the way back lands where it started. |
| `09-after-set-aside-the-wall-opens.png` | `Set aside and start over`, taken. The wall, over the same folders, with the old library renamed rather than deleted — and the app bar back, because this is a place again. |
| `10-after-an-unreadable-index.png` | The sibling case: 64 KB of `/dev/urandom` where the index was. Same surface, different words, and `Try again` present — because here, unlike the downgrade, trying again can give a different answer. |

## What the frames are asserting about the design

- **05 has no question mark and no folder field.** Naming a folder cannot help,
  so the screen does not offer to take one. That is the whole difference
  between it and 01.
- **05 and 10 say the same sentence in the same place**: *Your music and your
  playlists are untouched.* Pinned by
  `all_three_reasons_say_the_music_and_the_playlists_are_untouched`.
- **10 has two words and 05 has one.** *Absent, not disabled*: `Try again` is a
  control that could change the answer on an unreadable index and could not on
  a schema version.
- **07 exists at all** because nothing on this screen may rewrite a database on
  one press. `views::blocked::acts` does not contain the acting message until
  the cost is showing, so this is a fact about the code rather than about the
  layout.
- **Neither 05 nor 10 wears the app bar.** The bar is resident in all seven
  places (ADR-0040) and this is not a place — its display options need a wall
  and its gear opens a place inside a library that has not opened. 09 has the
  bar, because 09 *is* a place.
