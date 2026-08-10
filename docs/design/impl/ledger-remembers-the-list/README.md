# The ledger remembers the list

> *"when I play a song from a playlist it should only bump the recency of that
> playlist, not the underlying albums please"* — the owner, 2026-08-10

Frames of the real binary, headless, over the six-variable XDG isolation in
`docs/DEVELOPMENT.md` §"Headless UI verification". `capture.sh` is the whole
scenario; `[mpris] no session bus` appeared three times in its output, once per
session, which is the receipt that nothing reached the owner's desktop. Nothing
was audible: the scratch `HOME` routes ALSA's default PCM to `null`, every
fixture sample is a zero, and `BAZ_DEVICE_TESTS` was unset.

## What was done

One session did the listening, and two relaunches read it back:

1. **`Closing Time` was played as a record.** An ordinary album run, from no
   list.
2. **`Road Trip` was played.** A list quoting two tracks of `Closing Time` and
   two of `Paper Mill`. Its file was given a **30-day-old mtime**, so the only
   thing that could raise it in the lane is the play itself.
3. **Quit.**
4. **Relaunch**, over the ledger as written — frame `03`.
5. **Relaunch again**, over the *same ledger with its `# baz run` markers
   deleted* — frame `02`. That is byte for byte the file an older baz would
   have written, so the "before" frame is this build reading a v1 ledger rather
   than a different build reading anything. It is the before/after contrast and
   the old-ledger compatibility check in one run.

## The file the frames were folded from

```text
# baz run 2026-08-10T11:07:08Z -
2026-08-10T11:07:08Z	skipped	8684	387000	…/Closing Time/05 Marginalia 5.flac
2026-08-10T11:07:08Z	played	331906	387000	…/Closing Time/05 Marginalia 5.flac
# baz run 2026-08-10T11:07:13Z playlist:80f603b54a9209ce:Road Trip
2026-08-10T11:07:13Z	played	3600000	3600000	…/Closing Time/01 The Long Lie Down 1.flac
2026-08-10T11:08:00Z	played	274000	274000	…/Closing Time/02 Cassette Weather 2.flac
2026-08-10T11:08:04Z	played	349000	349000	…/Paper Mill/01 Undertow 1.flac
2026-08-10T11:08:08Z	played	97000	97000	…/Paper Mill/02 Marginalia 2.flac
```

Two runs, each opened by a marker. The album's names no list and writes `-`;
the playlist's names one. **Every play line is unchanged** — five fields, four
tabs, the format ADR-0018 pinned. The grain of the file changed; the grammar of
a line did not.

## The frames

| | |
|---|---|
| [`00-the-list-before-it-is-played.png`](00-the-list-before-it-is-played.png) | The list's page, with the lane holding `Closing Time` above `Road Trip` — the record was played, the list has not been. |
| [`01-the-live-half-in-the-same-session.png`](01-the-live-half-in-the-same-session.png) | The same session after playing the list. `Road Trip` is at the head: **the live half has worked since `lane::played_list`**, and is not what this change is about. |
| [`02-before-the-records-jump-the-list.png`](02-before-the-records-jump-the-list.png) | **Before.** The relaunch over a v1 ledger. `Paper Mill` and `Closing Time` are at the head and **`Road Trip` is last** — the lane re-derived the run from its play lines, so the two records the list quoted jumped over the list that was actually played. This is exactly what the owner reported. |
| [`03-after-the-list-is-at-the-head.png`](03-after-the-list-is-at-the-head.png) | **After.** The relaunch over the v1.1 ledger. `Road Trip` is at the head. |

### What frame `03` gets right that is easy to miss

**`Closing Time` is still in the lane, one row down.** It was played as a
record in its own run, and an album's run still credits the album — a fixed
list is not a playlist, and the marker's job is to stop a *playlist's* run
crediting its albums, not to stop albums being credited when an album is what
played.

**`Paper Mill` is gone entirely.** It was only ever heard inside `Road Trip`,
so there is no moment at which the listener put that record on. Saying nothing
about it is the honest answer, and it is the one the lane now gives.

Both readings come from the same file, and the ledger still counts every one of
those plays for the track — `History::track` is untouched, so the inspector
card and the `PLAYED` group key see all six. *When did I last play this track*
and *when did I last put this record on* are different questions, and this
change is the point at which baz stopped answering the second with the first.

## Reproducing

```sh
toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
  cargo build --release -p baz --features device-output
toolbox run -c baz-dev docs/design/composition/tools/mkfixture.sh /tmp/baz-ledger-fix
toolbox run -c baz-dev docs/design/impl/ledger-remembers-the-list/capture.sh
```

The script waits on the **ledger's own line count** rather than on a sleep, so
"the run was recorded" is a fact it checks rather than a duration it hopes for.
