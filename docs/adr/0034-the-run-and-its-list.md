# ADR-0034: The run and its list — `Origin`, and a history that remembers which list you were in

**Status**: accepted (2026-08-10) · **§2–§5 shipped**; §1's `QueueVm::origin` is not built yet — see *What shipped* below · records the owner's model, given while the
queue/now-playing merge was being designed · **amends [ADR-0023](0023-playback-model.md) §1 and §4** (provenance
generalised from *a playlist's name* to *the list's identity*) · **amends
[ADR-0018](0018-play-history-ledger.md)** (the ledger gains a run marker; the
line format is untouched) · **amends [ADR-0024](0024-playlists.md) §1** (states
what an implicit list has that a playlist does not, without widening the
definition) · adds one optional field to `Command::SetQueue` with **no change to
any pinned wire byte** · closes `docs/BACKLOG.md:9–25`, the owner's
attribution defect · the surface that spends it is
[`docs/design/12-now-playing-and-kiosk.md`](../design/12-now-playing-and-kiosk.md)
and [ADR-0029 §8](0029-the-ambient-surface.md)

## What shipped, 2026-08-10

The owner asked for the defect, not the model: *"when I play a song from a
playlist it should only bump the recency of that playlist, not the underlying
albums please"*. So the **ledger half** was built whole and §1's queue-side
refactor was not. What is on `main`:

| | |
|---|---|
| **§2** the origin on `SetQueue` | **shipped.** `command_wire_format_is_stable` is *unmodified* — `protocol.rs` gained 92 lines and deleted none. The `Some` arm is pinned in its own test, beside one that reads an older `set_queue` back. |
| **§3** the `kind:key:display` encoding | **shipped**, as `crate::origin::Origin::{encode,decode}`, all six kinds, with one deviation below. |
| **§4** the run marker | **shipped**, less rule 4 — see ADR-0018's amendment for why the second header block on an *older* file is refused. Rules 1–3 hold, and the marker is written in the **same `write_all`** as the run's first play, so it can never be orphaned by a crash between the two. |
| **§5** `History::runs()` | **shipped**, with `History::last_played_unlisted()` beside it — the per-track reading the lane folds. `TrackHistory` gained nothing. |
| **§1** `Origin` as a type | **shipped as the type**, promoted out of `implicit` exactly as §1.4 asks, with `file()`, `is_destination()` and `name()`. |
| **§1** `QueueVm::origin` | **not built.** `QueueVm::provenance` is still `Option<String>`, so the product constructs `Origin::Playlist` and nothing else. |

**That last row is the honest boundary, and it is not a shortfall against the
owner's ask.** A run with no origin writes a marker naming no list, and its
plays fold onto their records exactly as an unmarked ledger's always did — so
an album's run still credits the album, which is the other half of what he
said. What §1 would add is a *subject* for the five other kinds: the queue
summary reading `Ochre · 2 of 9` instead of `2 of 9`, and a draw crediting
nothing where today it credits the records it quoted. Neither is the defect.

**One deviation from §3, found by building it.** §1's `Album { id, name,
artist }` cannot round-trip: the encoding has **one** display field, and a
second name would need a fourth. `Album` and `Artist` therefore carry `id` and
`name` only. The lane already resolves a record's artist from the index when it
draws the row, so nothing reads a name this loses.

**One rule the implementation adds**, because marking a run *excludes* its
plays from the records they quoted: **a kind `lane::subject_of` answers `None`
for must not be written as a marker until the lane can credit it**, or the
touch is lost rather than moved. `origin.rs`'s
`no_kind_is_written_that_the_lane_cannot_credit` is that rule as a test, and it
is what §1 has to satisfy when it lands.

Frames, and the owner's own check end to end:
[`docs/design/impl/ledger-remembers-the-list/`](../design/impl/ledger-remembers-the-list/README.md).

## Context

The owner, on what the player should be tracking:

> *"the now playing needs to take into account that we could have been playing a
> song as part of a playlist… probably the basic model is that every album has a
> playlist implicitly… and so when we track the state of what is playing now or
> what our recent plays were… it should be basically which playlist and which
> track"*

**The claim is that everything that plays is a *list* and a *cursor*, and that an
album is implicitly a list.** So is a playlist, so is `All songs`, so is the wall
in its arrangement. *What is playing* is therefore always answerable as **which
list, and which track in it** — and so is every line of the play history.

### baz already believes half of this

ADR-0023 §1 says it and stops one step short:

> *"baz's queue is one list with a cursor, and the list is a record of a choice.
> The playing context — this record, this playlist, this draw — is reified into
> the queue at the moment of the gesture and then discarded."*

`docs/design/09-implicit-playlists.md:150–151` says it harder:

> *"baz has one kind of list. One of them is sounding and has no name; the rest
> are named and silent. Making a playlist is listening plus naming."*

**What is reified is the *contents*. What is discarded is the *identity*.** That
is the whole of the gap, and it is one field wide:
`QueueVm::provenance: Option<String>` (`vm.rs:714`) holds the *name of a playlist
file* and nothing else. It is set in exactly one place in the product —
`playlists.rs:1334`, `provenance: Some(playlist.name().to_owned())` — and every
other construction hard-codes `None`:

| Gesture | Where the queue is built | Origin recorded |
|---|---|---|
| A playlist's `Play`, or a click on its rows | `app.rs:2018`, `app.rs:2129` | **the file's name** |
| `Play album` | `vm::album_queue`, `vm.rs:859` | `None` |
| A track click / a song-search hit | `app.rs:3178` via `vm::album_queue` | `None` |
| Shuffle | `app.rs:3262` via `vm::stacked_queue`, `vm.rs:984` | `None` |
| `Play all` | `app.rs:3321` via `vm::stacked_queue` | `None` |
| Anything appended to a run | `app.rs:2093`, `addition.provenance = None` | cleared |

The frame at
[`docs/design/impl/queue-in-now-playing/01a-queue-open-1280x860.png`](../design/impl/queue-in-now-playing/README.md)
is that table rendered: an album run's summary reads `1 of 24 · 1:56:18 left`,
with the subject missing, because `queue_summary` prepends a name only when
provenance is `Some` (`player.rs:2189–2192`). A playlist run gets `Road Trip · 3
of 12 · 38:12 left` from the same three lines of code. **One sentence, and half
its subjects are unrepresentable.**

### And the engine has never been told at all

`Command::SetQueue { paths }` (`protocol.rs:94–97`) is the whole of what the
engine learns about a run, and `Event::TrackStarted { path, position }`
(`protocol.rs:364–369`) is the whole of what comes back. The engine writes the
play ledger, so the ledger cannot record what the engine was never told:
`PlayRecord` is five fields — `started_unix_s`, `outcome`, `listened_ms`,
`track_ms`, `path` (`history.rs:275–289`) — and `History` is keyed by
`PathBuf` alone (`read.rs:194`).

That is why the lane's attribution **works within a session and dies at a quit**,
which `docs/BACKLOG.md:9–25` already records as the owner's defect and marks
**Owner decision**:

> *"the recent bit shows albums popping up even though it was the playlist which
> was played" … the fix cannot reach across a quit, and the reason is structural
> rather than lazy … Closing it properly means **a provenance field on the queue
> command and a sixth field in the ledger line** (format v1 → v2 …). That reopens
> **ADR-0018**, which is the owner's decision and not a bug-fix's.*

He has now made it. This record is what he decided.

### Half of the answer landed on `main` while this was being written

`cad9f5a`, *"Make the implicit list a kind, not an All-songs-shaped thing"*,
merged in `db73cd3` — reshaped from the same sentence of the owner's, and it
says in its own module doc exactly where it stops:

> *"So **everything that plays is a list and a cursor**, and lists differ only in
> what they are made of and what identity they have. A named playlist has a
> *file*; an album's implicit list would have an *album id*; a draw's list has
> nothing durable at all; **All songs** has only a name… **Only one origin is
> built here**… the full model — including the harder half, where the play ledger
> records one line per *track path* and the engine is never told a run's
> provenance — **is a separate piece of design work and is not decided here**."*
> (`implicit.rs:15–28`)

**This record is that separate piece**, and §1.4 says how the two types become
one rather than two things called `Origin`.

`design/dynamic-playlists`, still unmerged, proposes rules-not-lists and refuses
provenance for a draw. It is not contradicted either, and §1.3 is why: a richer
origin that could name a fileless list would re-open `docs/BACKLOG.md:681–683`'s
Trap 1 — so the type carries a *destination* bit and the picker reads that
instead of `is_some()`, which reproduces today's behaviour by construction.

## Decision

### 1. `Origin` — a run's list, named

> **Every run carries the identity of the list it was reified from. The identity
> is a property of the *run*, not of the list object.**

That second sentence is the reconciliation with `AllSongs`: nothing is added to
the list types. `AllSongs` keeps no id, no path and no `save`; the queue built
*from* it carries an `Origin` that says so.

```rust
/// The list this run is a reification of — origin, never a live link.
/// `implicit::Origin` (shipped in `cad9f5a`) grown the three kinds that
/// have an identity of their own; see §1.4.
pub(crate) enum Origin {
    /// The implicit list every record is. `vm::album_id`.
    Album { id: u64, name: String, artist: String },
    /// One artist's records, in the artist page's order. `vm::artist_id`.
    Artist { id: u64, name: String },
    /// A playlist file — **the only kind with a file**, and so the only
    /// destination. `playlists::playlist_id` over the name, which *is*
    /// the filename (ADR-0024 §2).
    Playlist { id: u64, name: String },
    /// The wall, in its arrangement. Its identity is its name and nothing
    /// else, because there is only ever one of it (`implicit.rs:116–121`).
    AllSongs,
    /// A shuffle draw: an order, not a place. Nothing durable.
    Draw,
    /// Assembled one transfer at a time. There was no list.
    Hand { was: Option<String> },
}
```

**Three properties, each load-bearing:**

- **It is inert.** ADR-0023 §1 refuses *a context object that keeps acting*.
  Nothing reads `Origin` to decide what plays next; it is read to *say* things
  and to *file* things. It stands through jumps, seeks, pause, `QueueEnded` and
  every `UpdateQueue` edit including appends, and is replaced only by a
  replacing `SetQueue` — ADR-0023 §4's rule verbatim, now applying to six kinds
  instead of one.
- **The name is stored, not resolved.** A `key` alone would print nothing after a
  rescan renamed the record, and the history's whole job is to be readable years
  later. The key is for joining; the name is for reading; neither is derived from
  the other at read time.
- **`Hand` is a real answer, not a null.** A run you built by transferring rows
  one at a time came from no list, and saying so is different from saying nothing.
  `Origin` is still `Option` on a queue that predates this record or was restored
  from an old snapshot — `None` means *we do not know*, and `Hand` means *there
  was nothing to know*.

#### 1.1 Why not `Vec<Origin>`, and why an append clears nothing

Appending a record to a playlist run makes a run that is no longer that playlist.
Today `app.rs:2093` answers by nulling provenance. **That answer is kept, and
restated**: an append moves the run to `Origin::Hand` carrying the name of what
it was —

> `Hand`, `"from Road Trip"` — *you started here and then made it your own.*

A list of origins was considered and refused: it would make the head a
comma-separated sentence that grows without bound, and it would make *which list
am I in* a question with several answers, which is the thing the model exists to
prevent.

#### 1.2 Why an order is not an identity

`feat/shuffle-and-all-songs` retains a `shuffle::SourceOrder = Vec<PathBuf>` —
*"the order a run would play in with shuffle off"* — as a second run-level fact.
**It is not folded into `Origin` and should not be.** `Origin` answers *which
list*; `SourceOrder` answers *in what order it would play unshuffled*. Toggling
shuffle changes the second and must not change the first, because you are still
in `Road Trip`. Two facts, two fields, one run.

#### 1.3 The destination bit, which is what stops Trap 1

```rust
impl Origin {
    /// The playlist file this list is stored in — `implicit::Origin::file`
    /// (`implicit.rs:147–151`), grown the one variant that answers `Some`.
    pub fn file(&self) -> Option<&str> {
        match self {
            Self::Playlist { name, .. } => Some(name),
            _ => None,
        }
    }

    /// Whether this list is somewhere a track can be *added*.
    pub fn is_destination(&self) -> bool { self.file().is_some() }
}
```

Every consumer that today asks `provenance.is_some()` asks `is_destination()`
instead, and gets **the identical answer for every state reachable today** — the
picker's hoisted playing row (`views/playlist_panel.rs:99`), the context menu's
`Add to "{current}"` (`app.rs:1677`). So `menu.rs`'s
`no_menu_anywhere_offers_to_add_to_the_implicit_list` and `AllSongs`' own
absence tests keep passing **because the bit reproduces them**, not because
nobody noticed.

The rule stated once, so a seventh kind cannot get it wrong:

> **A list you can be *in* is not the same as a list you can add *to*. Only a
> file is a destination.**

#### 1.4 There is one `Origin`, and it is `implicit::Origin` grown up

`cad9f5a` shipped `implicit::Origin` — today a one-variant enum (`AllSongs`)
with `const fn name() -> &'static str` and `const fn file() -> Option<&str>`,
the latter written as *"the one method that makes the module's load-bearing
property a fact about the type rather than a convention"* (`implicit.rs:137–151`).

**That is this record's type, one kind short of finished.** Two enums both
called `Origin`, one naming fileless lists and one naming runs' lists, would be
the worst possible outcome of two people answering the same sentence. So:

> **`implicit::Origin` is promoted out of `implicit` and gains the file-backed
> kinds. `ImplicitList` keeps its exact meaning, defined as
> `origin.file().is_none()` — which is what `implicit.rs:38–46` already says it
> is.**

Three properties survive the promotion and one is spent, stated so the
promotion is not mistaken for a rewrite:

- **`file()` survives, and becomes `is_destination()`'s implementation.** It
  keeps answering `None` for every fileless kind — by construction, per variant
  — and `menu.rs`'s sweep keeps asserting it.
- **`name()` survives** and stays `&str`. `AllSongs` and `Draw` keep their
  `'static` names; `Playlist`, `Album` and `Artist` carry theirs in the variant,
  which is where an identity that differs per instance belongs.
- **`ImplicitList` keeps holding no state of its own.** Nothing is added to it.
  The run's origin lives on the run.
- **`Copy` and `const` are spent**, and that is the real cost: three kinds carry
  a `String`, so the enum is `Clone` rather than `Copy` and `name()` is a plain
  `fn`. Worth it — the alternative is resolving a name against the library at
  every read, which is exactly what makes a history unreadable after a rescan
  (§1's second property).

**The taxonomy in `docs/design/09-implicit-playlists.md` §2 gets rewritten once,
around §6's axes**, rather than twice by two branches pulling its rows in
different directions.

### 2. The protocol: one optional field, and not one pinned byte moves

```rust
Command::SetQueue {
    paths: Vec<PathBuf>,
    /// The list this run is a reification of, encoded by §3.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    origin: Option<String>,
}
```

**`command_wire_format_is_stable` (`protocol.rs:1586`) is unchanged**, and this
is a fact about `serde`, not a hope: the pinned bytes are
`{"cmd":"set_queue","paths":["/music/a.flac"]}`, `skip_serializing_if` omits the
key when it is `None`, and `default` accepts its absence. A sender that predates
this field and a sender that has nothing to say produce **the same bytes they
produce today**. The test gains one new case for the `Some` arm rather than
having its old one rewritten.

**`UpdateQueue` does not gain the field**, and the asymmetry is the model:
`SetQueue` is *a new choice*, `UpdateQueue` is *an edit to the choice you made*
(ADR-0014 §2). An edit that could restate the origin would make provenance
something an edit can lie about.

**`TrackStarted` does not gain it either.** A front end knows what it sent; an
event that echoed it back would be the engine repeating the sender's own
sentence, which is exactly the argument ADR-0014 §6 already made for not echoing
the paths.

The engine still holds no opinion about what is queued. It carries the string to
the ledger writer and reads nothing in it — one field, one consumer.

### 3. The encoding: one line a human can grep

```
<kind>:<key>:<display>
```

Split on the **first two** colons only, so a display name containing a colon
survives untouched. `key` is lowercase hex, empty where the kind has none.
Tabs, newlines and carriage returns in the name are escaped exactly as
`history::format::escape_path` already escapes them (`format.rs:95–102`), which
is the one escaping vocabulary this product has.

```
album:9c4f1a02bb37e5d1:Ochre
playlist:3b1f00c2a49d7e60:Road Trip
artist:57ea9b1103cc2fd4:Talk Talk
all::All songs
draw::Shuffle
hand::from Road Trip
```

An unknown `kind` word is read as `None` — *we do not know* — rather than
rejected, so a ledger written by a later baz stays readable by this one.

### 4. The ledger: a run marker, and **the line format does not change**

`docs/BACKLOG.md:20` anticipated *"a sixth field in the ledger line (format
v1 → v2)"*. **Specifying it found that the sixth field is the wrong answer, and
that a strictly better one costs nothing.** The finding, with the code that
produces it:

- `history::format::decode` **rejects any line with a sixth field outright** —
  `if fields.next().is_some() { return None }` (`format.rs:128–133`) — and the
  reader tallies the rejection into `History::malformed()` (`read.rs:270–273`).
  Since ADR-0018 §3 guarantees the file is **never rewritten**, a sixth column
  produces a **permanently mixed file that every older baz reads as partly
  corrupt**, silently losing those plays from the play counts, the PLAYED group
  key and the lane.
- The `Format v1.` marker is prose inside a comment written once at file creation
  (`format.rs:57`), never rewritten, and **never parsed** — so there is no
  version to switch on, and adding one would require the rewrite §3 forbids.
- Four separate places hard-pin four tabs: `format.rs:23`, `format.rs:426`,
  `tests/history.rs:205`, and `fuzz/fuzz_targets/history_line.rs:24`, plus the
  byte-exact line at `format.rs:387` and the byte-exact header at
  `history.rs:669`.

And the thing that makes the better answer free:

- **`#` lines are skipped and are *not* damage.** `read.rs:266–269` skips them
  before `decode` is reached; `format.rs:119–121` returns `None` for them without
  the caller counting it. Every reader baz has ever shipped already ignores them.

> **The ledger gains a run marker: a comment line that opens a run, followed by
> that run's plays. The five-field line format is untouched.**

```
# baz run 2026-08-06T07:06:40Z album:9c4f1a02bb37e5d1:Ochre
2026-08-06T07:06:40Z	played	231480	245013	/music/Ochre/01 Undertow.flac
2026-08-06T07:10:31Z	played	387000	387412	/music/Ochre/02 Marginalia.flac
# baz run 2026-08-06T08:02:11Z playlist:3b1f00c2a49d7e60:Road Trip
2026-08-06T08:02:11Z	played	198000	201004	/music/…/Kid A.flac
```

**The grain of the file changes; the grammar of a line does not.** A ledger was a
list of plays; it is now a list of runs, each holding its plays — which is the
owner's model applied to the history, written in the one syntax every existing
reader was already told to ignore.

Call it **format v1.1**, and mean it: `v1` describes a *line*, and no line
changed.

**What each reader sees:**

| | reads a v1 file | reads a v1.1 file |
|---|---|---|
| **a v1 baz** | as today | as today, exactly — the markers are comments, `malformed()` stays **0** |
| **a v1.1 baz** | every run `None` — *we do not know* — which is precisely today's behaviour | full attribution |

There is **no downgrade hazard and no migration**, which is the whole reason to
prefer it. A user who installs this baz, dislikes it, and goes back loses
nothing at all.

**Four rules the marker obeys:**

1. **Written by the same writer, on the same handle, before the run's first
   play line.** One writer, so no interleaving; the reader is already sequential
   (`read.rs:243–281`).
2. **A marker with no plays after it means nothing** — a run started and skipped
   through leaves a dangling marker, which readers ignore. Better than
   suppressing it, which would need the writer to know the future.
3. **A play line with no marker before it is `Origin: None`.** That is every line
   in every existing ledger, and it is the honest reading.
4. **A second header block is appended once**, the first time a v1.1 writer opens
   an older file, documenting the marker in the file's own voice — the same act
   as `format::TRUNCATED` (`format.rs:71`), and an *append*, so ADR-0018 §3's
   never-rewritten guarantee is untouched.

**`Event::PlayRecorded` is unchanged.** It carries what was written; the origin
is a property of the run the front end already holds.

### 5. `TrackHistory` gains nothing, and a second reading arrives beside it

`History::by_path` (`read.rs:194`) stays keyed by path and `TrackHistory`
(`read.rs:140–159`) keeps its six fields, because *how many times did I play this
track* is a question about a track and must not become a question about a list.

Beside it, a second fold: **`History::runs() -> Vec<PlayedRun>`** — `origin`,
`started_unix_s`, `plays` — which is what the lane reads at launch. Two folds,
two questions, one pass.

**No third question is admitted**, and ADR-0018 §6's refusal is inherited
verbatim: no totals by list, no most-played list, no charts. *History records; it
never performs.* A run marker makes an engagement statistic **easier** to build,
which is exactly why the refusal is restated here rather than assumed.

### 6. What ADR-0023 and ADR-0024 now say

**ADR-0023 §4** said the queue displays no *gesture*-provenance — *"the display's
provenance is which record a row belongs to, never which gesture added it"*.
Unchanged: `Origin` is not a gesture, it is a **list**. `Play album` and a track
click on the same record produce the *same* `Origin`, which is the test that this
is not gesture-provenance wearing a new coat.

**ADR-0024 §1**'s definition of a playlist is not widened. What is added is the
axis that separates the kinds, which doc 09 §2's taxonomy table should be
rewritten around:

| | has a file | is a destination | re-derivable | authored by |
|---|---|---|---|---|
| Playlist | yes | **yes** | no | a person |
| Album | no | no | yes, from tags | the record |
| Artist | no | no | yes, from the index | the index |
| All songs | no | no | yes, from the wall | the arrangement |
| Draw | no | no | no (a seed is not a list) | chance |
| Hand | no | no | no | a person, one row at a time |

**The `is a destination` column has exactly one `yes`, and that is ADR-0024 §1
restated rather than amended.**

## Consequences

- **`QueueVm::provenance: Option<String>` becomes `origin: Option<Origin>`**, and
  `PlayerState::queue_provenance()` (`player.rs:1722–1724`) becomes
  `origin().filter(Origin::is_destination).map(Origin::name)`. **Every existing
  consumer keeps its exact behaviour through that one function** —
  `player.rs:2190`, `app.rs:1677`, `app.rs:2790`, `views/playlist_panel.rs:99` —
  and only new consumers see the general answer.
- **`queue_summary` (`player.rs:2167–2193`) gains a subject for five more
  kinds**, at one changed line: `match &queue.provenance` becomes
  `match &queue.origin`. `Road Trip · 3 of 12 · 38:12 left` and
  `Ochre · 2 of 9 · 31:04 left` become the same sentence.
- **`lane::played_list` (`lane.rs:118–120`) stops being a special case.** Today
  it maps a playlist name to a `playlist_id`; it becomes a map from `Origin` to
  `lane::Subject`, and the finding is that **it already is one**:
  `Subject::Record(id)` *is* the album's implicit list. The lane's two subjects
  were list identities before anyone called them that. What changes is that
  `Origin::Draw` credits **nothing** — a draw is not somewhere you return to
  — where today it silently credits every record it quoted.
- **`docs/BACKLOG.md:9–25` closes**, and its own cheaper alternative stays
  refused for its own reason: the marker is *in the ledger*, written by the
  ledger's writer, so it is not "a second store of a fact the ledger should
  hold".
- **`session.rs`'s snapshot** (`session.rs:56`, `:91–92`, `:132–133`) carries the
  encoded string in place of the bare name. An old snapshot's plain name reads as
  `playlist::<name>` with an empty key, which resolves on first use — a
  round-trip an old baz still parses, because it is still a string.
- **A run's origin can be wrong about the world and right about the past.** A
  playlist deleted after a run was played leaves a history line that still names
  it. That is correct: the ledger records what happened, and what happened is that
  you played `Road Trip`.
- **Nothing here makes shuffle, repeat or a continuation policy expressible.**
  The engine gains one string it does not read. ADR-0014's and ADR-0023's
  deferrals stand exactly where they were.

## What would reverse this

- **Evidence that a run marker breaks a real third-party reader.** The format is
  a file the owner is invited to `grep`, and a marker that made his own tooling
  wrong would be worth the sixth column's downgrade hazard instead. `awk -F'\t'`
  over the file is unaffected; a reader that counts *lines* rather than parsing
  them is not, and that is the case to look for.
- **A second consumer of the origin inside the engine.** One string carried and
  never read is a reasonable price; a second reader would make the engine hold an
  opinion about what is queued, and the honest response would be to move the
  ledger write to the front end rather than to widen the engine's remit.
- **The owner deciding a draw *is* somewhere you return to**, which would give
  `Origin::Draw` a real key (the seed) and a lane row, and would put it back
  in tension with `design/dynamic-playlists`' refusal.
