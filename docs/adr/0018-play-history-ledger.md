# ADR-0018: The play-history ledger — a plain append-only file, written by the engine, that nothing sends anywhere

**Status**: accepted (2026-08-08) · first unit of the `docs/design/critique` build order, which puts history **before everything else** · introduces `baz_core::history` and `Event::PlayRecorded` · **not** a schema change: the ledger is deliberately not in `library.db` (see §3)

## Context

`docs/design/critique/03-build-guide.md` opens its build order with a single
instruction and a single reason:

> **History ledger first.** Append-only file, written from the first beta even
> with zero UI — history cannot be backfilled; PLAYED, the card, and the pull
> feed on it.

That reason is the whole of the urgency. Every other cache in baz rebuilds: a
missing thumbnail redraws, a missing ReplayGain figure re-measures (ADR-0015), a
deleted `library.db` rescans (ADR-0010). A day of listening that nobody wrote
down is gone, and no amount of later engineering recovers it. Shipping the
surfaces first and the ledger later would mean shipping surfaces with nothing to
show.

`02-surfaces.md` says what it is and — as importantly — what it is not:

> Append-only ledger in a plain local file — one line per play; user's to grep /
> back up / burn. Last.fm scrobbling = optional output, never a dependency.
> Surfaces: a PLAYED group key, inspector card stamps, and weighting for "the
> pull". Nothing else. **History records; it never performs** — no charts, no
> streaks, no Wrapped.

And `docs/VISION.md`'s third pillar is the standing constraint: *sovereignty by
default — offline-first, no account, no telemetry, all app data in open
formats.* A listening history is the single most personal thing a music player
holds. If baz is going to keep one, the way it keeps it has to be the argument
for trusting it with anything else.

## Decision

### 1. The file, and the line

`$XDG_DATA_HOME/baz/history.tsv` (and the platform equivalents `dirs::data_dir`
reports) — **beside `library.db`**, because it is the same kind of thing: baz's
own record of the user's collection, in the user's own space.

One line per play. Five tab-separated fields. UTF-8. The first line of a new
file is a comment block documenting the format inside the file itself.

```
2026-08-06T07:06:40Z	played	231480	245013	/home/matt/Music/Talk Talk/01 Myrrhman.flac
2026-08-06T07:10:51Z	skipped	9200	402000	/home/matt/Music/Talk Talk/02 Ascension Day.flac
```

| # | field | meaning |
|---|---|---|
| 1 | `started_utc` | ISO-8601 UTC, seconds, `Z` — when the track's **first audio** was heard |
| 2 | `outcome` | `played` or `skipped` (§2) |
| 3 | `listened_ms` | milliseconds of this track's audio actually delivered to the output |
| 4 | `track_ms` | the track's own length, or `-` when the file declares none |
| 5 | `path` | the file, escaped (below) |

Each choice earns its place:

- **Timestamp first, UTC, no offset.** Lexicographic order is chronological
  order, so `sort` works on the file and `grep '^2026-08'` is "August" — with no
  tool on the machine needing to know what a date is. A local-time-with-offset
  format would have sorted wrongly across a DST boundary; a local-time-without-
  offset one would have been ambiguous for one hour a year, permanently.
- **Seconds resolution.** A play is a human-scale event. Milliseconds in the
  timestamp would be four characters of noise per line.
- **Tab, not comma or space.** Filenames contain spaces and commas constantly
  and tabs almost never; `awk -F'\t' '$2=="played" {print $5}'` is the whole
  query language this file needs.
- **Words, not codes.** `played` / `skipped` rather than `1` / `0`, because
  `grep played` is the point.
- **Integer milliseconds** for both durations — the argument
  `crates/baz-core/src/protocol.rs` already makes: one canonical encoding, and
  no float is ever printed into a file that gets compared byte for byte.
- **`-` for an undeclared track length**, never `0`: a reader must be able to
  tell "this container declares no length" from "this track is zero seconds",
  the same distinction `Event::Progress` makes with `null`.
- **Path last**, because it is the long, variable field, and a human scanning
  the file reads down a ragged right edge rather than a ragged middle.

#### Escaping

The path — and only the path — is escaped, to the minimum that keeps the format
line-oriented, tab-delimited, reversible and valid UTF-8:

| in the path | in the file |
|---|---|
| `\` | `\\` |
| tab | `\t` |
| newline | `\n` |
| carriage return | `\r` |
| any other C0 control, or `DEL` | `\xHH` |
| a byte that is not part of valid UTF-8 | `\xHH` |

Nothing else is touched, which is the property that matters:
`/mnt/nas/音楽/坂本龍一/01.flac` is in the file exactly as it is on disk, so
`grep 'Talk Talk' history.tsv` finds what a human expects it to find. And
because every escape is ASCII, **the file is always valid UTF-8 even when the
paths in it are not** — a byte-oriented filesystem cannot produce a ledger that
breaks `less`, an editor, or a locale-aware `grep`. (On platforms whose paths
are UTF-16, the only unrepresentable sequences are unpaired surrogates, which
are recorded with the replacement character — the trade `protocol.rs` already
makes for paths on the wire.)

### 2. What counts as a play

**Half the track's length, or four minutes, whichever comes first** —
`baz_core::history::play_threshold_ms`, with `PLAY_THRESHOLD_CAP_MS = 240_000`.
That is the convention Last.fm established and ListenBrainz kept, and it is
right for a reason worth stating: half of a two-minute pop song and half of a
twenty-minute side of *Tago Mago* are not equally strong evidence, and past
about four minutes the fraction stops mattering. Somebody four minutes into a
long piece is listening to it. A track whose container declares no length has
only the cap to go on, which is the right answer for a stream.

Two deliberate departures from the scrobbling convention, both towards recording
more truth:

**No minimum track length.** Last.fm refuses anything under thirty seconds. That
rule exists to stop people gaming a public leaderboard — an anti-abuse measure
for a scoreboard baz does not have and, per the design, must never grow. A
twelve-second track played to its end is a play. A file on the listener's own
disk is not evidence in anyone's competition.

**Skips are recorded**, as their own outcome, one line each. Three arguments, in
order of weight:

1. *It is more honest.* A threshold that decides which listening is worth
   writing down silently discards the other half of the evidence. The ledger's
   claim is "this is what happened", and half of what happened is not that.
2. *It is more useful.* "You have started this four times and never finished it"
   is the strongest single signal in the file, and both the pull's weighting and
   the inspector card want it.
3. *It is free to ignore.* `grep played` recovers the played-only view exactly,
   so a reader who does not want skips pays nothing for their being there. The
   cost on disk is one ~100-byte line per skip — a few megabytes for a listening
   lifetime.

The argument against is real and is recorded rather than waved away: a record of
what you *abandoned* feels more like surveillance than a record of what you
finished. The answer is the one that governs everything else here — the file is
local, plain, documented in its own header, and the user's to delete. A player
that could not be trusted with a skip could not be trusted with a play either.

**Nothing at all is recorded for a track that delivered no audio.** A queue entry
the listener jumped straight past was never met, and a ledger of things that did
not happen is not a ledger.

`listened_ms` counts **audio delivered to the output**, not wall-clock time and
not a position in the track. Pausing for an hour adds nothing. Hearing a passage
twice counts it twice; skipping forward past one does not count it. At a track
boundary the count stops exactly at the boundary, not at wherever the pump had
reached when the crossing was announced.

### 3. Append-only means append-only

The file is **never rewritten**. No compaction, no de-duplication, no rotation,
no in-place correction, no "tidy up on startup". The only operation
`baz_core::history` performs on it is `write(2)` at the end.

That is a guarantee rather than an implementation note, and it is what makes the
ordinary Unix habits safe: `tail -f` it while music plays, `cp` it mid-write,
back it up with `rsync --append`, split it by year with `grep`. It is also why
this is a **file and not a table in `library.db`** — a row in SQLite is a thing
the database may rewrite, VACUUM, or lose to a corrupted page, and none of
`grep`, `awk`, `sort` or a backup tool can read it. `library.db` is a cache
(VISION.md: "the database is a cache"); this is not.

`History::read` is a snapshot, and a snapshot of an append-only file can only
ever be *missing later plays* — it can never be wrong about an earlier one.
Re-reading is the whole update mechanism, and reading is safe while the engine
appends.

#### The truncated tail

A line is one `write_all` to an `O_APPEND` handle, followed by `fsync`. A
process killed mid-append can still leave a partial final line.

- **Reading** stops at the last line ending in a newline. A partial final line
  is not a record; every complete line before it is read. A line that *is*
  terminated but cannot be parsed — a hand edit, a backup concatenated by
  mistake — is skipped and counted (`History::malformed`), never fatal. One bad
  line costs exactly itself.
- **Opening for writing** checks whether the file ends in a newline and, if it
  does not, appends `\t\t# incomplete line, closed off by baz` and a newline.

  A bare newline would **not** have been enough, and this is the subtle part:
  `…\tplayed\t231480\t245013\t/home/matt/Music/Talk Ta` plus a newline is a
  *perfectly well-formed record*, naming a file that never existed. The
  terminator is chosen so the line is unparseable wherever the cut fell — too
  many fields if it fell late, an empty `track_ms` if it fell in the middle, too
  few fields if it fell early — and legible to a human about what happened. It
  is still an append: no byte already in the file changes.

### 4. Written by the engine, on the engine's terms

**The playback engine writes the ledger. A front end never does.** The engine is
the only thing that knows what is reaching the output and for how long; a ledger
written by a front end would lose an album to a crash and would be written
*twice* by two front ends attached to one engine. A front end's whole
involvement is one call — `EngineHandle::set_history(Some(ledger))` — which is
the same seam ADR-0015 built for computed ReplayGain figures, for the same
reason.

The default is `None`: an engine nobody has handed a ledger to writes nothing
anywhere. That is what keeps the workspace's test suite, and any embedder that
has not opted in, off the user's disk.

**No file I/O on the pump path.** The engine thread's entire cost is one integer
compare per report call ("did the delivered track change?") and — once per
finished play, at most once per track, between pump iterations where event
emission already happens — one mutex read of the ledger slot and one channel
send. The `write` and the `fsync` happen on the ledger's own `baz-history`
thread. `Session::pump` is untouched: the same ring read and sink write it has
always been (`docs/ENGINEERING.md`, "the audio thread is sacred").

A play spans the time a track is *the track being delivered*: it opens at
`Event::TrackStarted` and closes when anything displaces it — the next track,
`Next`, `Previous`, `JumpTo`, `Stop`, the end of the queue, a queue edit that
moves the transport (ADR-0014), a sample-rate handover (ADR-0009), or the engine
shutting down. **A seek is the one exception**: it rebuilds the session, but the
listener is still inside the same track, so the play carries across rather than
being filed and started again. Four drags of the needle file one line, not five.

### 5. `Event::PlayRecorded`, and what it promises

```json
{"event":"play_recorded","path":"/music/a.flac","started_unix_s":1786000000,
 "listened_ms":231480,"track_ms":245013,"outcome":"played"}
```

The wire conventions are unchanged: internally tagged, `snake_case`,
`#[non_exhaustive]`, `Eq`, integers only, byte-pinned by
`play_recorded_wire_format_is_stable`. The timestamp is whole **seconds** rather
than the milliseconds every other time in the protocol carries, because it is a
wall-clock instant rather than a duration — it is the number in the file, and a
front end renders it as a date.

It is emitted **by the writer thread, after the line is in the file and
synced**. That is the state-before-event contract ADR-0015 states, applied to
the one piece of state that is not in memory: a front end that reacts to this by
re-reading the ledger always finds the play it was just told about. A record
that could not be written emits nothing — there is no line to be news about —
and the failure is counted (`HistoryLedger::write_failures`) rather than taking
the music down.

It is deliberately **not** ordered against the transport events: a play ends
when the next one begins, so it typically arrives just after the `TrackStarted`
that displaced it. A front end must not infer what is playing from it.

### 6. Reading it back: three surfaces, and no fourth

`baz_core::history::History` folds the file per track and answers exactly the
three questions `02-surfaces.md` names:

1. `History::track(path) -> TrackHistory` — plays, skips, first/last played,
   total listened. **The inspector card's stamp.** Counts and the two bracketing
   dates rather than every play's timestamp: a card says "played 14 times, last
   on Tuesday", and holding a timestamp per play per track would keep a
   100 000-track library's whole listening life in memory to render a line of
   text. The file still has every play, and `grep` still finds them.
2. `History::recency(path, now) -> Recency` — **the PLAYED group key**:
   `ThisEvening` → `Today` → `ThisWeek` → `ThisMonth` → `MonthsAgo(n)` →
   `YearsAgo(n)` → `Never`, ordered so a front end sorts by the type and gets
   the design's group order. Elapsed-time bands, not calendar arithmetic — a
   band whose width depended on which month it was would make the key jump about
   for no reason a listener could see. `ThisEvening` is defined as the last six
   hours, which is what a type holding UTC seconds and no timezone database can
   honestly promise. Buckets on the last **play**, never a skip: starting
   something and abandoning it is not having heard it.
3. `History::pull_weight(path, now) -> u32` — **the pull's weighting**. One per
   day since last played, capped at a year (366 at the cap), and 367 for a
   record never played, so what nobody has ever put on is drawn in preference to
   what was heard a year ago. Nothing is ever weighted to zero: the pull is a
   bias, not a filter. It deliberately ignores skips — down-weighting what you
   skipped would make the pull start having opinions about your taste, which is
   the "performing" the design rules out. `TrackHistory` carries the skip count
   for a caller that decides otherwise, in the open.

There is no fourth. No totals-by-artist, no listening-time-per-month, no top-N.
Those would be *built from this data*, so the way not to build them is not to
provide the surface that makes them easy.

### 7. Privacy, and the scrobbling seam

There is nothing in this file that is not already on the user's disk: paths they
chose, times their own clock reported, durations their own files declare. **No
identifier, no machine ID, no session key, no hash of anything.** It is written
to their data directory, documented in its own header, and nothing sends it
anywhere — there is no network code in this path and no account to attach it to.

Scrobbling is **out of scope for this unit**, and the design's phrasing is the
reason: *optional output, never a dependency.* The seam it attaches to is
`Event::PlayRecorded` — a scrobbler is a *consumer* of that event, and of
`History` over the file for catching up after being offline. It sits downstream
of the ledger rather than beside it, which means concretely: the ledger is
complete whether or not a scrobbler exists, whether or not it is configured, and
whether or not the network is up. A scrobbler that fails, or is removed, or was
never written changes nothing about what was recorded. No code in
`baz_core::history` knows what a scrobbler is, and that is the property to
preserve when one is added.

## Consequences

- **History starts accumulating at the first run that wires the ledger**, which
  is the point of doing this first. The wiring in `crates/baz` — one
  `HistoryLedger::open_default()` and one `set_history` at start-up — is the
  remaining step, and it is one line of intent in each place.
- **One new dependency line, no new crate in the workspace lock.** `baz-core`
  gains `dirs`, which is already in `Cargo.lock` and already linked into every
  baz binary through `crates/baz`. Stated precisely, because the fuzz crate has
  its own lock and never saw `crates/baz`: `fuzz/Cargo.lock` does grow, by
  `dirs` and its four transitive crates. No fuzz target links them. The
  alternative — spelling the XDG spec out with `std::env` — would have been
  right on Linux and wrong on Windows and macOS. Date formatting needed **no**
  dependency: ISO-8601 UTC from a `u64` is Howard Hinnant's `civil_from_days`
  and its inverse, about forty lines, tested against hand-computed instants and
  exhaustively round-tripped across a leap year.
- **The file grows without bound**, by design. At one line per track played or
  skipped, a heavy listener writes on the order of a megabyte a year. Compaction
  is exactly the thing that was ruled out; a user who wants a smaller file has
  `grep`, `split`, and `rm`.
- **`fsync` per play.** Once per track is free at this cadence and buys real
  durability for a file that cannot be rebuilt. It is on the ledger's thread, so
  a slow filesystem delays a line, never a sample.
- **A cheap clock produces a cheap timestamp.** The ledger records what
  `SystemTime::now()` says; a machine whose clock was wrong records the wrong
  time, and correcting the clock later does not correct the line (nothing is
  ever rewritten). Recency treats a future timestamp as the most recent bucket
  rather than underflowing.
- **The wire grew one event and one enum**, both `#[non_exhaustive]`, so a front
  end written before this ADR keeps working and simply never hears about plays.
- **`Recency::ThisEvening` is six hours, not an evening.** The honest
  approximation, stated in the type's docs so a front end knows what it is
  rendering. A real local-evening bucket needs a timezone database, which is a
  dependency this decision did not think the label was worth.
