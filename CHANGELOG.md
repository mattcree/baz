# Changelog

All notable changes to baz are recorded here, in the format of
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## Versioning

baz follows [Semantic Versioning](https://semver.org/). It is **pre-1.0**, and
what that means here is specific rather than decorative:

- **`0.y.z` promises nothing about compatibility.** The on-disk library
  database, the `baz-core` command/event protocol, the configuration file and
  the user interface may all change in a `0.y+1.0` release. Schema migrations
  are still written and tested — a database from an older baz is upgraded, not
  discarded — but the shape of things is not yet settled.
- **`0.y.z` → `0.y.z+1`** is bug fixes and additions that break nothing.
- **`0.y.z` → `0.y+1.0`** is anything else.
- **1.0.0** is not a quality claim about the code; it is the point at which the
  library format and the protocol become promises. It arrives when they are
  worth promising, not on a date.

Every release is built from a tag by CI, gated on the full test suite — see
[`docs/RELEASING.md`](docs/RELEASING.md). Nothing below has been tagged yet.

## [Unreleased]

Everything so far. baz has never been released; this section is the state of
`main`, and it becomes the first version's section when a tag is cut.

**Status: pre-alpha.** It scans a music folder, shows the albums and plays
them. It is not a finished player, and nothing here is a promise about the
next commit.

### Added

**Library**

- Directory scanner reading tags with `lofty`, falling back to folder-structure
  inference (artist/album/track) where tags are absent or unusable.
- Persistent library index in SQLite (bundled, so no system library is
  required), with an in-RAM corpus for search. Schema versioning via
  `user_version` with migrations tested against databases built by older schema
  SQL rather than by baz itself; currently at v5.
- Search across the whole library as you type, over a memchr-backed substring
  scan.
- Albums grouped by **album artist**, not track artist, so a soundtrack or
  compilation is one album rather than one per credited performer. The album
  artist resolves from the `ALBUMARTIST` tag, else the compilation flag, else
  the track artist; where no signal exists baz declines to merge rather than
  guess (ADR-0008, schema v3).
- **Album editions**: one album that exists in several formats — a lossless rip
  and an MP3 copy — is one shelf entry with an edition per codec, keyed on the
  codec read from file headers rather than on folder names. The default edition
  is ranked lossless before lossy, then by track count, then by mean bitrate
  (ADR-0007, schema v2).
- **A multi-CD album is one record** (ADR-0038). Most already were: the
  grouping key reads no path, so discs that share an `ALBUM` tag have always
  been one shelf entry whether they sit in one folder or two, ordered by
  (disc, track). What shattered was the rip that puts the disc *in the title* —
  `… (Disc 2)`, `… CD2`, `… [Disc 2]`. A closed-list marker rule takes it off:
  three words (`disc`, `disk`, `cd`), one or two digits, at the end, on a
  bracket or whitespace boundary, never a fuzzy distance. It fires **only when
  a sibling exists** — two spellings of one base title under one album artist —
  so a listener who owns only disc 1 sees `Bitches Brew CD1` unchanged. The
  marker also supplies the disc number where `DISCNUMBER` is absent (tags win
  where both exist), which is what makes a `CD1`/`CD2` rip play in disc order
  and gives its page the `DISC 1` / `DISC 2` breaks. Discs compose with
  editions rather than colliding: a two-disc set owned in FLAC and MP3 is one
  record, two editions, two discs each. No file is written and the index keeps
  every tag verbatim, so the merge is reversible by deleting the rule.
- **Incremental scanning**: a file whose (mtime, size) stamp is unchanged is not
  reopened. Measured over 10 000 synthetic tagged files, scan 61.2 ms → 10.3 ms
  and a whole warm launch 83.4 ms → 11.6 ms (ADR-0010, schema v4).
- **Removal by positive confirmation only.** A row is deleted only when the
  walk saw something, the row names a root whose walk produced something this
  pass (ADR-0022 replaced the old path-prefix gate), no ancestor directory
  failed to be read, and the filesystem confirms the file is gone. The stated
  price: deleting a whole album *folder* leaves its rows, because from below
  that is indistinguishable from an unmounted share.
- **A native folder picker, and the NAS as a first-class folder** (ADR-0025).
  The Settings place's add-a-folder row gains `Browse…` — the desktop's own
  directory dialog via the XDG portal on Linux (`rfd` 0.17, portal-only: no
  gtk, one new crate) — beside the typed path, which stays: a dialog cannot
  name an unmounted share. A typed path is now statted off the UI thread, so
  a dead network mount can no longer stall the event loop at the moment of
  adding it; and the unavailable-root lifecycle (unmount → nothing removed →
  remount → same rows, same stamps, same first-seen, no duplicates) is pinned
  by tests at both the worker and the library layer.

**Playback**

- Gapless playback engine ported from the Phase 1 spike, with sample-level
  continuity asserted against synthesized ground truth rather than against
  baz's own output.
- Formats: FLAC (including FLAC-in-Ogg), WAV, MP3, M4A/MP4 (AAC and ALAC) and
  Ogg Vorbis, all through pure-Rust `symphonia` — no C library and no system
  dependency anywhere in the decode path. Per-format gapless behaviour is
  documented and tested, including what MP4 does not trim.
- **The output follows the source sample rate** and never resamples silently.
  A session opens the device at the native rate of the track that starts it; a
  track at a different rate drains the sink and reopens. Where a device offers
  no mode at the source rate the track still plays, converted, and the
  conversion is reported. Measured on a 24/48 file: play-to-first-sample
  2 224 ms → 12.5 ms on a rate change, 0.7 ms when the device is already there
  (ADR-0009).
- Seeking, with playback position reported from the engine's own knowledge and
  never extrapolated between reports.
- **Volume**, as engine state: a cubic 60 dB fader law shared by every front
  end, applied as software gain in the one place every sample passes. At
  exactly unity the samples reach the sink with no copy and no arithmetic, so
  bit-exactness at full volume is a property of the control flow rather than of
  floating-point luck. Reported honestly through `VolumePath` (ADR-0011).
- **ReplayGain**, read from the tags files already carry — `REPLAYGAIN_*` in
  Vorbis comments, `ID3v2` `TXXX` frames, MP4 freeform atoms and APE items, plus
  the Opus-style `R128_*` integer form — in **off / track / album** modes with a
  pre-amp, a separate pre-amp for files that carry none, and clipping prevention
  from the declared peaks. Album mode falls back to a track's own gain when the
  file declares no album value, and an untagged file is played exactly as stored
  by default, so switching ReplayGain on cannot alter a library nothing has
  scanned. It shares the volume's gain stage — one multiply per sample for both
  — and changes on the track boundary's own first sample. Reported through the
  same `VolumePath` the volume uses, because there is one gain stage and one
  answer to "is this bit-exact" (ADR-0013, schema v5).
- **ReplayGain *analysis*** — baz computes the figures for files that carry
  none (ADR-0015, schema v6). An EBU R128 / ITU-R BS.1770-4 **gated integrated
  loudness** meter and a sample-peak meter, written in `baz-core` with no new
  dependency and validated against the **EBU Tech 3341 compliance signals** at
  48 kHz and 44.1 kHz: every case lands inside the ±0.1 LU the specification
  states, worst measured error **0.0241 LU**, and the K-weighting coefficients
  are asserted against BS.1770-4's own published 48 kHz table to 1e-12. A
  background pass measures the library album edition by album edition —
  cancellable within one decode block, resumable (a cancelled pass keeps what
  it measured), skipping anything a tag already answers, and structurally
  unable to fight the incremental scanner because a scan writes the tag columns
  and a pass writes its own. Computed figures are distinguishable from tagged
  ones everywhere it matters: separate columns, a file stamp that makes a
  measurement stale when the file changes, and three new `ReplayGainSource`
  variants (`computed_track`, `computed_album`, `computed_track_fallback`).
  **Tags still win, field by field**, so measuring a library can never change
  how an already-tagged track sounds. Driven by a second command vocabulary,
  `AnalysisCommand`, because it is a second service — the player has no
  database and the analyser has no sink.
- **Queue editing that does not stop the music.** `JumpTo { position }` plays
  the entry it names — the queue-relative sibling of `Seek` — from wherever the
  transport is, including stopped. `UpdateQueue { paths }` removes, reorders,
  inserts and appends by sending the queue as it should now be: an edit that
  does not touch the playing track leaves the delivered stream bit-identical to
  an unedited run, because the running session plays its track out and only then
  hands over to the edited queue. What survives an edit is the playing *track*,
  never its index, so the engine re-derives the position by identity and reports
  it on `QueueChanged { len, position }`; removing the playing track continues
  at the entry that took its place (ADR-0014).
- Command/event protocol between the engine and any front end, with the wire
  format pinned by test.

**History**

- **An append-only play-history ledger** at `$XDG_DATA_HOME/baz/history.tsv`,
  beside the library and in plain text (ADR-0018). One tab-separated line per
  play — `started_utc`, `played`/`skipped`, `listened_ms`, `track_ms`, path —
  documented by a comment header inside the file itself, so it is greppable,
  `awk`-able, backup-able and deletable without any tool knowing what baz is.
  The file is **never rewritten**: no compaction, no rotation, no in-place
  fixes. An interrupted append is closed off with a marker that cannot read
  back as a record, and a reader stops at the last complete line, so a
  truncated tail costs exactly itself and reading is safe while the engine
  writes.
- A play is **half the track, or four minutes, whichever comes first** — the
  scrobbling convention, with two deliberate departures: no minimum track
  length (that rule exists for public leaderboards, which baz does not have),
  and skips are recorded as their own outcome rather than discarded. What is
  counted is audio delivered, so pausing adds nothing and a seek inside a track
  continues one play rather than starting another.
- Written by **the engine**, never a front end, so a crashed front end loses
  nothing and a second one cannot double-write; the append and its `fsync` run
  on the ledger's own thread, never on the pump path. A front end's whole
  involvement is `EngineHandle::set_history`, and the default is no ledger, so
  nothing is written until one is opened.
- `Event::PlayRecorded`, emitted **after** the line is in the file, and a typed
  read API for the three surfaces the design names: per-track play counts and
  dates, recency buckets (`THIS EVENING` → months → `NEVER`), and
  least-recently-played weighting. Nothing else — history records, it never
  performs. Scrobbling is out of scope and attaches downstream, as a consumer
  of the event, never as a dependency of the ledger.
- **The ledger is a list of runs, each holding its plays** (ADR-0034, amending
  ADR-0018). A comment line — `# baz run <started_utc> <kind:key:name>`, or `-`
  for a run that came from no list — opens a run, and `Command::SetQueue`
  carries the origin that fills it in. The owner: *"when I play a song from a
  playlist it should only bump the recency of that playlist, not the underlying
  albums please"*. Playing a list already credited the list *within* a session;
  the ledger was per **path** and the engine was never told a run's provenance,
  so a relaunch re-derived the albums and put them back at the head of the
  lane. It does not any more.
  - **The line format did not change.** Five fields, four tabs, every
    byte-exact test unmodified — because `decode` rejects a six-column line
    outright and the file is never rewritten, so the sixth field this was
    first specified as would have left a permanently mixed file that every
    older baz read as partly corrupt. A `#` line was already skipped and
    already not damage. **No migration, no downgrade hazard**: an older baz
    reads a new ledger whole, and this baz reads an old one exactly as it did.
  - **No pinned wire byte moved either.** The command's `origin` field is
    omitted when there is nothing to say, so a sender with no origin produces
    the bytes it always produced.
  - **An album's run still credits the album.** A fixed list is not a
    playlist; what stops crediting the records is a run that named a *list*.
    A track heard only inside a list is not a record you put on, and the lane
    says nothing about it — while the play counts, the inspector card and the
    `PLAYED` group key still see every one of those plays. *When did I last
    play this track* and *when did I last put this record on* are different
    questions.
  - **The lamp dot follows the run's origin too.** The owner, on the same
    surface: *"I still see albums specifically appearing as if they are playing
    rather than the playlist … it is showing next to the album rather than the
    playlist"*. Order and mark now come out of one function
    (`lane::sounding_subject`), so they cannot drift; a list's run marks the
    list and none of the records it quotes, and at most one row is ever marked
    because a run has one origin. A **finished** run marks nothing, which took
    saying out loud: a run's origin outlives the run, where the sounding record
    the mark used to read went to `None` when the music stopped and was
    carrying the liveness by accident.
  - `docs/design/impl/ledger-remembers-the-list/` has the owner's own check —
    play a list, quit, relaunch — in frames.

**Playlists**

- **Playlists as files the user owns** (ADR-0024): one `.m3u8` per playlist in
  `$XDG_DATA_HOME/baz/playlists/`, beside the library and the ledger — no
  database table, no export step. `baz_core::playlist` reads liberally
  (headerless files, bare path lists, CRLF, BOM, relative paths and `~`,
  unknown `#EXT` directives preserved verbatim) and writes the strict common
  subset (`#EXTM3U`, one `#EXTINF` per entry, absolute paths, UTF-8, atomic
  whole-file rewrites). The migration story for a foobar2000/MusicBee refugee
  is `cp *.m3u8` into one directory; legacy `.m3u` files are listed and read,
  never minted. Nothing writes a playlist file but the user's own edit —
  enforced by the module's shape, not by discipline — and external edits are
  honoured by fingerprint on read: last writer wins, per file, no prompt.
- **A playlist's page** (`Place::Playlist`, the record page's sibling): the
  name at hero scale, `Play`, `Queue`, `Rename`, `Delete` (confirmed in the
  roots ADR's voice — *"The file goes; your music stays"*), and rows in the
  queue place's anatomy: record-group headers over consecutive same-record
  runs, a reserved ✕ to remove and reserved ▲▼ steppers to reorder (the
  no-drag pointer route the visible-control rule requires), a row click
  playing the list from there through the same `play_from` rule every list
  surface uses, and the lamp dot only when the queue is exactly this list. A
  missing entry **stays in the file**, drawn dimmed from its path's stem with
  the path on the row, unplayable; `Play` sends the playable subset and the
  page says so: `38 of 40 · 2 missing`.
- **The playlist panel** — the one summoned, single-tenant side surface, and
  the amendment of the refusals ledger's side-surfaces entry (by ADR-0024
  under the ledger's own editing rule; the entry names the panel and closes
  the slot). A labelled `Playlists` door in the Library strip (`Ctrl+P`
  beside it) floats it over the wall's right edge by ADR-0016's verified
  mechanics — `stack` + `opaque`, no scrim, wheel passing through — so the
  wall does not reflow by a pixel (`docs/design/impl/playlists/` holds the
  before/after diff). Present in Library, Album and Queue places, absent in
  Settings; `Esc` peels its layers one per press. Contents (doc 09 §8.1):
  the **Queue's row at the head** — the unnamed, sounding list, a readout at
  rest (its door stays the bar's labelled `Queue`) and the picker's first
  destination while a pick stands — then one row per playlist, sleeve and
  name, one control (the door to its page), then `New playlist` (an inline
  name field validated by the storage layer's rule, its refusals surfaced in
  its own words).
- **One transfer gesture, every destination** (doc 09 §8.1, the accepted
  amendments to ADR-0023/-0024): a track row's reserved-slot `+` and the
  record page's `Add to…` (relabelled from `Add to playlist` — the ellipsis
  promises the second press) open the panel as the picker. Its rows, in one
  order: **Queue** first — append to the run via `UpdateQueue`, the music
  keeps playing, and appending to an empty stopped engine loads the queue
  without starting it — the **playing list hoisted second** while provenance
  stands (marked *playing*; its pick appends to the *file* only, never the
  sounding run — the both-at-once gesture is refused, doc 09 §6), the named
  lists, `New playlist`. Additions append; duplicates are allowed and
  unmarked — the gesture did what it said. ADR-0023 §3's dedicated `Queue
  album` control is withdrawn before being built (two controls, one message —
  L8.6); the picker's Queue row is the queue-append.
- **Queue-place edit parity** (doc 09 §8.2, step 5 of its §13; ADR-0024's
  amendment item 3, accepted): the queue's rows carry the playlist page's
  whole reserved edit set — the ▲▼ steppers in the page's exact slots,
  swapping an entry with its neighbour through the pure `queue_edit::shifted`
  and sent as the whole-list `UpdateQueue` (the music keeps playing,
  ADR-0014; the cursor follows its track through the reorder, found by path
  until the engine's `QueueChanged` confirms), the existing ✕, and the
  transfer `+` toward the picker, on the sounding row too. The queue place
  and the playlist page are now **the same editor**, differing only in their
  header blocks — the builder's audible workbench (S9a) is complete.
- **`Play all`** (doc 09 §7.1, S6; step 6 of its §13; ADR-0023's amendment
  item 4, accepted): one word in the Library strip, leading `Shuffle` and
  `Pull` — the wall's current scope reified into the queue, whole records in
  the arrangement's own order, playing from the top. **The scope is the
  wall, always**: a query plays the matches, a YEAR arrangement plays the
  collection chronologically, "everything" is the empty query, and an empty
  wall does nothing and claims nothing. No pool is claimed and no
  confirmation interposes at any scale, because **the queue place is now
  virtualized** (`queue_window`, §7.1's named gate — the wall's
  spacer-window discipline at list scale, pinned by a test that a
  40 000-row queue builds a bounded slice): February's
  select-all-into-a-playlist is one press, and the result is an ordinary
  queue — readable, jumpable, editable, saveable, ending in silence.
- **The context menu, as a mirror layer** (doc 09 §5.2, step 4 of its §13 —
  the last step before the drag; ADR-0024's amendment item 4, accepted):
  right-click opens a float of short verbs **at the pointer**, flipped to
  stay inside the window at its edges, on four objects and no more — track
  rows wherever they appear (album page, Songs section, playlist page:
  `Play · Queue · Add to "{current}" · Add to playlist…`), queue rows
  (`Play · Add to "{current}" · Add to playlist… · Remove`), album tiles
  (`Open · Play album · Queue album · Add to playlist…`), and the bar's
  now-playing block (`Go to record · Add to "{current}" · Add to
  playlist…`). Governed exactly as the keyboard is: **every item sends only
  messages some visible on-screen control also sends** — pinned by
  `every_menu_item_is_a_press_some_control_also_makes`, the keyboard
  sweep's exact shape — so the menu is an accelerator layer, never the only
  route, and no bar slot, no new key binding and no submenu arrives with
  it. `Add to "{current}"` appears exactly while playing provenance names a
  file that still exists (absent, not disabled, otherwise) and appends to
  the **file only**, the run untouched — which closes S4: the sounding
  song reaches the current playlist in two gestures from anywhere,
  right-click the bar and press the item. One menu at a time by
  construction (the overlay state is a single `Option`); Esc peels it
  first, a press outside puts it down, an item press closes and fires; the
  float is the panel's ADR-0016 mechanics (stack + `opaque`, no scrim,
  surface step + hairline, nothing reflows by a pixel). With the mirror
  in place the playlist page's rows gain the **transfer `+`** in the queue
  row's exact outer slot — the last piece of §8.2's "same editor" claim,
  and the visible twin the page rows' menu items mirror. Captures at
  `docs/design/impl/context-menus/`.
- **Shift-click queues the record** (doc 09 §13 step 7; ADR-0023 §3's
  append accelerator): shift held, the press that would open a record's
  page appends it to the run instead — `UpdateQueue` through the picker
  Queue row's exact shape, nothing sounding unasked, the record joining
  the tail as its own headed group. The visible on-screen route to the
  same act is the picker's Queue row (`Add to…` → `Queue`), which the
  gesture accelerates the way a key binding accelerates a button.
- **Drag to reorder, drag to add** (doc 09 §13 step 8 — the last of its
  steps; ADR-0024 §6 layer 3, resequenced by doc 11 P5): press any row of
  the two list editors — the queue place or a playlist's page — move past
  an 8 px threshold, and the row is in the hand: a quiet ghost card names
  it at the pointer, an insertion line rides the boundaries between rows,
  and release commits **one** whole-list edit — one `UpdateQueue` for the
  run (the music keeps playing, the cursor follows its track by path), one
  atomic file save for the artefact. Carried over the standing panel's
  playlist rows instead, the drop appends the track to that file — the
  picker row's own append, made direct. A sub-threshold press is still the
  row's ordinary click; <kbd>Esc</kbd> discards the gesture; the pointer
  leaving the window or the window losing focus commits it where the line
  was (the groove's own capture lessons, inherited whole — iced 0.13 still
  has no pointer grab, and `crates/baz/src/drag.rs` documents what is done
  about it). **Sugar only**: the ▲▼ steppers, the ✕, the `+`, the picker
  and the context menu all remain exactly as shipped — the drag is
  pointer-only by nature, and the visible controls stay the accessible
  route the visible-control rule requires. Captures at
  `docs/design/impl/drag/`.
- **Playing provenance** (doc 09 §6): a queue reified from a playlist file —
  its `Play`, or a click on its rows — records the file's *name* on the
  request-side queue record. Origin, never a live link: it survives every
  queue edit and `QueueEnded`, and is replaced only when a different play
  gesture replaces the run. The Queue place's summary leads with it — `Road
  Trip · 3 of 12 · 38:12 left` — answering *"what list is this run from?"*,
  previously unanswerable.
- **The armed collecting mode is removed** (was ADR-0024 §6 layer 2; doc 09
  §9). Shipped one day, removed on the owner's own observation: it was a
  second list-building grammar and a mode — the one press in the product
  whose meaning depended on what was armed earlier. With it go the panel
  rows' receive `+`, the wall labels' collecting mark and tile-press
  override, the record page's relabelling, and the armed layer of the `Esc`
  peel. Its one-press economy passes to the context menu (doc 09 §5.2,
  pending) and the drag (layer 3, unchanged).
- **`Save as playlist` on the queue place**: tonight's run frozen into a new
  file — a new artefact and nothing else; the queue is not linked to the
  playlist it seeded, and editing either never reaches the other.
- **A playlist has a sleeve** (ADR-0024 §A1, on the owner's *"similar to
  Spotify a playlist would appear like a cd does"*): a generated collage of
  quotations — the first four distinct records' artwork as a 2 × 2, the
  first record's face full-bleed below four, and a designed rest tile (the
  surface step, the name in ink) for a list with nothing to quote. Cells
  come from the wall's own thumbnail cache with no new decode path and
  degrade to the wall's own gradient placeholder mid-decode; the composition
  re-derives whenever the playlist re-reads, so an edit changes the sleeve
  with the rows. It hangs at 40 px on the panel's rows — which turns the
  index from a list of names into a shelf of objects — and at 320 in the
  hero position of the playlist's page, which now wears the record page's
  own two-column arrangement: the work over `Play` in the aside, the name
  over the rows in the main column. Whether playlists also join the wall is
  deliberately deferred to the implicit-playlists design study (doc 09).
- **A record is a work you found; a playlist is a label you made**
  ([ADR-0024](docs/adr/0024-playlists.md) §A3–§A5, design doc 14, tier 1). The
  owner reported two things in one breath — *"we do not have the playlist name
  really prominent… the information heirarchy isn't great to be able to tell
  the difference between an album and a playlist"* and *"'save as playlist'
  really makes no sense on the playlist page for a CD"* — and they were one
  defect: `Save as playlist` over a CD wrote a one-record playlist, whose
  sleeve is byte-for-byte the widget a record's own row builds, which then
  landed in `RECENT` above the record it came from wearing its face. The
  control manufactured the confusion.
  - **The line under a name declares its kind in its first token.** A
    playlist's `14` becomes `Playlist · 14 · 42:10`, in the exact slot where a
    record prints its album artist. One string, and it reaches the returns
    lane's rows, the playlist panel's rows and any tile a playlist ever gets:
    the same `SIZE_META` text at the same leading, so **no geometry changes
    anywhere**. A bare integer in that slot did not read as a count — it read
    as a name truncated to nothing.
  - **The playlist page gets back the byline the record page always had.** The
    name was never small: it is the album title's own `SIZE_HERO` 28 /
    `SEMIBOLD` and always was. What was missing was the 19 px line of support
    under it, so the block stopped after 52 px against a record's 80. The
    playlist page was the album page *with the byline deleted*. The slot now
    holds the word `Playlist`, the two identity blocks are one 80 px shape, and
    the difference lives in what the middle line **says** — an artist for a
    found thing, its kind for a made one. Not *"Made by you"*: a `.m3u8`
    dropped into the folder was not made here and records no author.
  - **The run column's strip names its subject.** With no provenance it read
    `1 of 24 · 1:56:19 left` — a reading with no noun — beside a word offering
    to save it and 57 px above the record's own title. Both branches now open
    with a subject: `Run · 1 of 24 · 1:56:19 left`, or the list's name when the
    run came from one.
  - **`Save as playlist` says what it is saving, and offers only when it can
    act.** Over a run reified from a file and unedited since, the word is the
    readout `Saved as "Road Trip"` — the run *is* that file, whose name the
    same strip is already printing. One edit and the live word returns as
    **`Save as new playlist`**. `Save changes to "…"` is refused: provenance is
    an origin, never a live link, and a run that wrote itself back would be the
    two-structure confusion returning. Nothing is removed — freezing a
    transient into a file is still one press, which is what a shuffle, a
    `Play all` and an edited run genuinely want. The precedent is eleven lines
    up in the same file: `Undo` is drawn only while there is an edit to take
    back.
  - **The collage is demoted from *the* signal to *a* signal**, and the two
    prose sites that said otherwise are corrected (`views/lane.rs`,
    [ADR-0030](docs/adr/0030-the-returns-lane-and-the-home-band.md)'s fifth
    amendment). ADR-0024 §A1 is unchanged in every rule; what was false was the
    load on it — below four distinct records a playlist's sleeve *is* a
    record's cover, from the same cache at the same edge, which is every list
    `Save as playlist` makes from a CD and every list on its way to four.
  - No badge, no glyph, no rounded corner: a mark over a sleeve breaches
    *"nothing is ever drawn on top of a sleeve"*, and a rounded one means
    *different* rather than *made*. Frames at
    `docs/design/impl/records-and-lists/`.
- **…and the two pages now say it in the type**
  ([ADR-0024](docs/adr/0024-playlists.md) §A4.4, design doc 14 tier 2). Tier 1
  stated the distinction in words; this states it in the face, which is the
  axis that costs no pixels and holds at every size.
  - **A record's page sets its title in IBM Plex Serif Italic**
    (`theme::WORK_TITLE`), joining Home's `CONTINUE` placard — and **a
    playlist's page deliberately does not**. Same `SIZE_HERO` 28, same ink,
    same slot, same composition to the pixel: three ink bands, 71 px of ink, a
    35 px pitch to the byline and 27 px to the facts, identical on both pages
    at 1280 and at 1920. Only the face differs, and it differs because a
    record's title is a **work's** — published by someone else, set the way a
    museum placard sets one — while a playlist's name is a **label the owner
    typed**, like the search query, the rename field and the folder path, all
    of which are already sans.
  - **The serif's boundary stopped being a count and became a rule.** It used
    to be *"there is one placard in the product"*, which cannot say whether the
    next string may have the face. It is now: **the serif italic sets an
    album's title, on the surface whose subject that album is** — not a
    track's, not an artist's, not a playlist's name, and not an album's title
    standing as a fact about something else, which is why `Now playing` stays
    wholly in the sans and now has a sharper reason to than it had.
  - **The test stays an enumeration**, never a `contains`:
    `the_serif_is_the_work_titles_and_nothing_else` names two views and fails
    the build on a third, and nothing may name the serif family directly — so
    reverting the whole experiment is still one token. Whether the face should
    reach the wall's tile captions and the returns lane's rows is the owner's
    open question and is untouched.
  - **The silent-fallback hole is closed mechanically.** `Font::with_name` is a
    string match: a family spelling that drifts by one character resolves
    against whatever the *host* owns, which looks right on the machine that
    shipped it and wrong on a fresh one. Two new assertions compare the family
    strings baz asks for against the names the bundled bytes spell for
    themselves, check the serif declares the italic style the token requests,
    and require the face to carry every Latin-1 letter and every punctuation
    mark an album title arrives with — a codepoint it lacked would fall back
    *per glyph*, setting half a title in a host font. **Found writing them**:
    the family a matcher reads is `name` record 16, not record 1; record 1 is
    the legacy family, which holds four styles at most, so Plex Sans Medium's
    reads `IBM Plex Sans Medm`.
  - **The playlist's byline states its composition**: `Playlist` →
    `Playlist · 12 records`, which also explains the collage beside it. **Not**
    from the sleeve's quotation list, as design 14 costed it — that list stops
    at four, so a fourteen-record playlist would have read `Playlist · 4
    records` over a page listing fourteen. The distinct set is walked to its
    end instead, and a list nothing in the library resolves says `Playlist` and
    claims no count.
  - **Tier 2's third item is declined on a frame rather than adopted.** The
    save label naming its subject — `Save these 24 as a playlist` — was
    conditioned on tier 1's `Run · ` prefix proving insufficient. It did not:
    the strip reads `Run · 2 of 12 · 55:00 left … Save as playlist`, subject
    first, with the run's own cursor between the noun and the word. Against
    that, a variable-length label in the 440 px strip is the one measurement
    doc 14 §6.3 flagged as wanting a frame. Frames, measurements and the
    reasoning at `docs/design/impl/serif-titles/`.

**Interface**

- **`A–Z` is a group key again, first in the row**
  ([ADR-0035](docs/adr/0035-the-wall-has-a-subject.md), third amendment). The
  owner, on the wall that shipped an hour earlier: *"also, we have removed the
  a-z option from grouping? that feels like it should go back and honestly it's
  the first option, followed by artist"*. The strip reads **A–Z · ARTIST ·
  YEAR · GENRE · ADDED · PLAYED**, and <kbd>1</kbd>…<kbd>6</kbd> select them in
  that order.
  - **Two densities of one order, on purpose.** `A–Z` breaks the wall into 27
    letter shelves; `ARTIST` breaks the same traversal finer, one shelf per
    person. The entry below deleted `A–Z` on the ground that the two are the
    same traversal, which is true and is now the reason both exist: the owner
    uses them differently, and the coarser wall is not a caption for the finer
    one at the sizes a real library reaches.
  - **A new code, `"alphabet"`.** `GroupKey::code` is on-disk config and a code
    is never repurposed — so the restored key does **not** take `"artist"`
    back. That word already changed meaning once without saying so (it named
    the initial grouping before ADR-0035 and the artist grouping after), which
    is now recorded on `GroupKey::code` itself; giving it away again would make
    one code name three arrangements. Every `config.toml` on disk keeps the
    arrangement it currently resolves to.
  - **The rail gained no branch.** One function serves both keys: the first
    shelf of each initial's run, which is a letter per artist-run under
    `ARTIST` and the identity under `A–Z`. `Initial` is unchanged and is now
    both a wall header and a rail letter, one mapping asked of `baz-core` in
    both places. `A–Z`'s headers are inert text — a letter is not a place —
    while `ARTIST`'s header is still the door to the artist's page.
  - **The strip's budget, re-derived rather than reused.** The row has carried
    a sixth word before and it was `ARTISTS`, at 77.49 px; `A–Z` costs
    **44.92**, so the figures are not the earlier costing's. Measured row
    357.91, `KEYS_W` 314 → **360**, `LIBRARY_LINE` 506 → **552**,
    `TOP_BAR_SPLIT` 778 → **824**, `SINGLE_LINE_NO_WELL` 554 → **600** against
    an unmoved `WIDEST_LANE_STRIP` 720. **The window's own minimum did not have
    to move** — the library line sits 48 px under the 600 floor — and the
    single-line-with-well band survives at 824…904.

- **`ARTIST` groups the wall by artist**
  ([ADR-0035](docs/adr/0035-the-wall-has-a-subject.md)). The owner, on the
  artists wall that shipped earlier the same day: *"artists should be grouping
  stuff by artist not just alphabetically"*.
  - **One shelf per artist, headed by their name** — unknowns first, then names
    case-folded alphabetically, then unnamed compilations, with each artist's
    records alphabetical under them. It broke records on the artist's *initial*
    before, which is what made a key called `ARTIST` a key whose word was false
    and what made it collide with the Artist **place**.
  - **The header is the door to that place**, in the record page breadcrumb's
    own paint and on the word's own box. The type is unchanged, so the band is
    still pixel-identical pinned and unpinned.
  - **The index rail is still the alphabet.** With a shelf per artist there are
    far more headers than letters, so a letter jumps to the **first artist
    filed under it** — the shape `rail::genre` already had. `Initial` is
    unchanged; it stopped being the wall's header and became the rail's letter.
  - **It is an ordinary group key**, because grouping albums under their album
    artist shows every album exactly once, which is ADR-0019 §1's promise
    verbatim. `shelves(GroupKey::Artist)` is still `albums()` with its breaks
    named, element for element — the finer headers name breaks that were
    already in the list.
  - **So `A–Z` and `ARTISTS` are both gone**, and the strip is five words
    again. `A–Z` was the same traversal under coarser headers, and the
    jump-to-letter it was good for lives in the rail. `GroupKey::code()` is
    still `"artist"` — nothing was retired, so every `config.toml` baz has ever
    written resolves, now to the arrangement its word always claimed. A
    `wall_subject` line from the release that had one is read by nothing and
    dropped on the next save.
  - **Deleted with them**: `vm::WallSubject`, `ArtistVm`, `ArtistShelfVm`,
    `build_artists`, `visible_artists`, four parallel `artist_*` fields on
    `Shelf`, `show_subject`, five `wall_*` accessors, the artist tile,
    `views::SLEEVE_CELLS`, `top_bar::subject_word`, the `wall_counts` /
    `wall_noun` readout split in both wells, `WallSubjectSelected`, the
    <kbd>6</kbd> accelerator, the `wall_subject` config key, and the artists'
    own art prefetch. Net −700 lines across `crates/`, tests included.
  - **The strip's budget goes back**: `KEYS_W` 368 → **314**, `LIBRARY_LINE`
    560 → **506**, `TOP_BAR_SPLIT` 832 → **778**, `SINGLE_LINE_NO_WELL` 608 →
    **554** against an unmoved `WIDEST_LANE_STRIP` 720. The window's own
    minimum does not move. The single-line-with-well band is 778…904, wider
    than it has ever been.

- **The returns lane**: a resident surface at the window's left edge
  (ADR-0030), on the room's `recess` so it reads as cut into the room rather
  than stuck onto it. 280 px open, 96 collapsed, `Ctrl`+`B` or two marks at
  its foot, and the state is remembered in `config.toml` beside the density
  step. Its head holds three fixed destinations — `Home`, `Library`,
  `Now playing` — always all three, the current one in full paper ink, and
  **`Now playing` carries the lamp dot when something is sounding**, stacked
  on the glyph so it survives the collapse. Below a hairline, two sections:
  **`PLAYLISTS`**, every list, and **`RECENT`**, the last 24 records played —
  each last touched first, ties by name. No queue, no sort, no filter, no
  pinning — each of those is one of the five findings that killed the last
  resident column (ADR-0024 §5), read here as engineering lessons.
  - **The lists have a section of their own**, and it reverses the brief the
    lane was built from. The owner asked for *"recent albums and playlists
    mixed based on some order"*, that is what shipped, and then he read it:
    *"I guess we need to add playlists into their own section under library"*.
    So it is a **split, not an addition** — `RECENT` holds no list at all now,
    because a list in both sections is one door drawn twice, and it costs
    nothing to obey because `PLAYLISTS` is *every* list. Both sentences are on
    the record: ADR-0030 is amended (sixth), not rewritten.
    - **The order is untouched.** Last touched first, in each section — a list
      played this morning moved section, not rank. Alphabetical was the other
      honest answer for a section holding all of them and was declined: the
      ask is for a section, not a second ordering, and it would have spent the
      recency he already had.
    - **Under the head, not inside it.** *"Under library"* read positionally;
      a section between `Library` and `Now playing` would split the closed
      triple of destinations. The lane still has exactly **one** seam — the
      sections are named by headings, not divided by a second rule.
    - **One scroller over both sections**, which is the load-bearing part.
      `PLAYLISTS` has no cap, so two scrollers would have given the surface
      two scroll positions to arbitrate between and a fixed-height first
      section would have pushed `RECENT` off the bottom of the window at about
      a dozen lists. Proved at thirty lists, expanded and collapsed, at 1280
      and 1920 (`docs/design/impl/playlists-section/`).
    - **A section with nothing in it is absent, not empty**, for both sections:
      a first run gets no headings over nothing, where `RECENT` used to draw
      its word over an empty column. **Collapsed, a heading is nothing** —
      `RECENT`'s own answer at 96 px, taken rather than re-invented; the two
      runs of sleeves are separated by the sections' own `GAP_MD` and every
      row keeps its tooltip.
  - **The collapse is a hard cut, one frame, no tween.** It is the one press
    in the product whose subject is the collection's width, and it is safe
    because it lands outside the wall: no wall gesture can be in flight. The
    wall keeps the *shelf* that was at the top of the viewport, not its pixel
    offset. The 1 px width sweep from 300 to 2560 now runs in both lane states
    over every density step.
  - At two of three shipped widths the column count does not change and the
    covers simply get bigger — 243 → 304 at 1280, 258 → 294 at 1920 — so the
    gesture reads as *zoom* rather than as reflow, which is also what it means.
  - **The strip's `Playlists` door is gone**: the lane is the resident,
    complete index of lists. 88 px come back to the strip and the single-line
    floor falls from 960 to 872. The two-line split still earns its keep — it
    exists for the *library* line, whose tenants still need 600. The window's
    declared minimum rises to 696, the strip's floor plus the lane's rail.
  - **The search well is in the lane's head**, under the three destinations —
    the owner's decision: *"the design does not match properly… the search
    should really be in the sidebar"*. A **field**, not a `Search`
    destination: baz has type-anywhere, so the query is open before you decide
    to search, and a destination row would say *go somewhere first*. Its mark
    stands on the destinations' glyph vertical and its text on their word
    vertical, and it is **one control tall**.
    - **The match count is inside the field**, right-aligned in a reserved
      72 px slot: `3 / 25`, which is the pair the strip's own well has always
      drawn. The slot is the lane's own rather than the strip's 88, which is
      what makes it fit — 232 less the 44 px text inset, a `GAP_MD` and 72
      leaves the query **104 px**, more than a reserved 88 would have. Fixed
      and right-aligned, so the figures change in place; reserved only while a
      query stands, and on the *right*, so nothing moves as the first
      character lands.
    - **The collection's counts are not in the lane at all** — they are the
      Home place's `COLLECTION` footer (below). The well's placeholder is free
      to say `Search`.
    - **Collapsed**, the well is the magnifier in the destinations' anatomy,
      tooltipped `Search`, taking the lit ink while a query stands. Pressing
      it, `/`, `Ctrl`+`F` and the first key of a query all open the lane onto
      the caret — one frame, no tween.
    - **Every road to the query now goes to the Library first.** Typing from
      `Home` or a record's page used to fill a field that was not on screen and
      narrow a wall that was not either; the lane put the field in every place,
      and this puts the wall back under it.
    - **Below `SIDEBAR_FLOOR` the strip takes the well back** in its old form,
      because a 96 px rail cannot hold a field and cannot open. One home per
      regime, never two — the breakpoint is the lane's own floor.
    - **The strip is now states, acts and the gear.** `TOP_BAR_SPLIT` is 872
      exactly rather than a rounded 960; the well's 80 px fluid range is
      deleted, because no strip that draws the well is ever wide enough to
      climb it, so the split is the whole of the collapse order. Above
      `SIDEBAR_FLOOR` the strip is one line at every width in either lane
      state — 648 wanted against a narrowest possible 720, asserted.
- **`Place::Home`** — the interrupted run and what is new, as a place rather
  than as a band at the head of the wall (the owner's choice between
  ADR-0030 §3.2 and §9.4). `CONTINUE` is a 132 px sleeve beside a placard: the
  artist in letterspaced caps, the work's title, `1999 · FLAC · 16-bit ·
  44.1 kHz`, then **the needle** — 2 px at exactly the sleeve's width, amber to
  the elapsed fraction with a 1 px tick at the position — then `Resume ·
  Anhydrous 11 · 3:12 of 4:34`. **The placard carries the needle and nothing is
  drawn on the artwork.** `RECENTLY ADDED` is one row of the wall's own tiles by
  first-seen, with the wall's own hover options. `COLLECTION` closes the page
  with four figures — `25 ALBUMS`, `9 ARTISTS`, `206 TRACKS`,
  `14 hours OF MUSIC` — a figure at the emphasis size over a tracked word, on a
  96 px lattice, nothing pressable and nothing in colour. Every band is absent,
  not empty.
  - **It is a footer because you come to Home to get back into music**, not to
    read numbers: `CONTINUE` is the one thing on the page you press and an
    inventory must not push it down. Cut from the four and each for a reason —
    *when the collection was last added to* (`RECENTLY ADDED` says it one
    section above, with covers); *how many records have never been played* (a
    fact about the listener, and ADR-0030 §6 refuses engagement statistics);
    *the size on disk* (a filesystem fact, and the record page's `Details`
    block is where bytes belong).
  - The four are counted where the wall's view model is built — one pass over
    every track per rebuild, never per frame, which is ADR-0030 §4's contract.
    Artists are **named album artists, case-folded**, so a library that spells
    one band two ways is not told it has two.
- **`Place::Artist`** — an artist's page: their name, their counts, and their
  records in **the wall's own tile**, so a record behaves the same there as it
  does on the wall. Reached from the record page's new breadcrumb. It carries
  `vm::artist_id`'s hash rather than a name — `album_id`'s own first half, the
  same marker bytes and the same case-folding, so a nameless compilation and a
  band called "Various Artists" stay two artists exactly as they stay two
  albums. An artist the library no longer holds answers with the wall, as a
  vanished record's page does. Deliberately absent: a biography, an artist
  image, play counts, and a flat list of every track (network, engagement
  statistics, and ADR-0017 §1.7 respectively).
- **`Artist › Album` in the record page's header.** The strip used to lead with
  the word `Album` — the *kind* of page you were on, when the page is entirely
  made of the answer. It now names where the record sits, and the artist half
  is a door. Frames and the measured strip height:
  `docs/design/impl/artist-page/`.
- **`Place::NowPlaying`** — the sounding record at the size it deserves:
  artwork, identity, the same needle at the work's width, the bar's own
  transport. Every measure is derived from the viewport, so the kiosk
  full-screen mode is this surface at a larger size rather than a second one.
  No visualizer and no VU.
- **The queue and Now playing are one surface, and the bar's `Queue` door is
  gone.** The owner: *"the queue and the now playing need integrated in some
  way so we can remove the queue option from the bottom bar"*. **A run is a
  list and a cursor** — `Place::NowPlaying` was drawing the cursor and
  `Place::Queue` was drawing the list, each missing the half the other held —
  so the place enum goes from eight members to seven and the queue place's
  whole body becomes the merged surface's **run column**. Frames and the
  measured composition: `docs/design/impl/queue-merged/`;
  the argument is `docs/design/12-now-playing-and-kiosk.md` §3.4 and §5.5a.
  - **Fourteen of the queue's fifteen affordances survive**: row click to
    jump, the per-row ✕, the ▲▼ steppers, the transfer `+`, drag-to-reorder,
    `Save as playlist` and its field, `Undo` and `Ctrl+Z`, the
    provenance-led summary, the right-press mirror menu, row hover tracking,
    the virtual window, the column's own scroll, and the album group headers.
    The one that goes is the `Queue` header strip — the merged place wears
    none, because the lane is the route and the run's head states the list.
    The two empty states become one: the run's wins, because it names the
    gestures that fill the list.
  - ~~**Two densities, chosen and remembered.**~~ **Reversed by the owner the
    same day** — see *The `Run` word is gone* under Removed. The run column
    stands whenever there is a run. It is `RUN_MEASURE` 440 — half the measure
    a list that owns its surface gets — and below a `SPLIT_FLOOR` 784 body the
    two columns re-stack into one with the record as the run's head block.
  - **The run costs the record nothing** wherever the record is height-bound,
    which is every window above the tightest one this product draws. At
    1280 × 860 with the returns lane open it costs 113 px, and the remedy is
    already on screen and already keyed (`Ctrl+B`). Recorded as a cost paid.
  - **The bar does not move a pixel**, and the door's 152 px goes to the
    title: the title lane at 1280 is **248 → 408**, and a track title that
    clipped before clips 160 px later.
  - `Ctrl+U` is now the lane's `Now playing` row plus the `Run` word, made
    for you. It stops toggling, because a destination never closes itself;
    `Esc` is the way out, and always was.
- **The now-playing sleeve is 32 px larger at every height-bound size.**
  ADR-0029's first step removed the transport this place was drawing a second
  time, and the 32 px it had reserved stayed in `art_edge`'s arithmetic. It is
  gone with it.
- **The artwork stops at the file, and the room takes the record's colour.**
  The owner, at full screen: *"also fullscreen the now playing looks weird"*.
  He was right, and about two things at once (ADR-0029 §2, doc 12 §5.2–§5.4;
  frames, the sampled field and the measured edges at
  `docs/design/impl/artwork-at-size/`).
  - **`NOW_PLAYING_MAX` 720 is deleted.** It was a constant standing in for a
    fact about the decode, and it was wrong in both directions at once: it let
    a **320 px thumbnail be drawn at 2.25×** on any panel 1080 px tall or
    better — *no artwork is ever drawn larger than its source*, false in the
    one place nobody had a test for — while capping a 2560 px panel's work at
    720 in a 2280 px body. Measured off the frames, the sleeve goes
    **720 → 1024 at 2560 × 1440** and **720 → 773 at 1920 × 1080**; a
    collection ripped with 300 px covers now draws them at **300**, honestly
    small, rather than at 720.
  - **A second decode tier, of one record**: `art::load_hero` at `HERO_PX`
    **1024**, two entries, **8 MiB** against the thumbnail cache's 150. Same
    resolution order, same downscale-only decode, so a record's hero can never
    disagree with its thumbnail about which file the art came from. `art_edge`
    gains a third term and it is **the source's own pixels**.
  - **The ambient field.** Three hue angles read off the decoded cover, hung on
    the room's own lightness ladder at a ceiling of **oklch L 0.22** with
    chroma pinned at **0.024**, drawn as a three-stop wash under the whole
    place. It is `Palette::lamp`'s own rule — *hue read from the record,
    lightness and chroma pinned* — with three colours instead of one and a
    large area instead of a small one. **Not the artwork**: three angles cannot
    reconstruct an image, and a gradient has no resolution, which is why it can
    fill a 4K panel where a 1000 px cover cannot. **Not a scrim**: it is under
    everything, laid over nothing, and it dims no artwork.
  - **Under the run column the ceiling is lower**, clamped flat to the room's
    own `wall` — measured at L 0.155–0.160 against 0.158 — so a scrolling list
    is read over the same ground every other list in the product is read over
    and no new contrast number enters the design. Below `SPLIT_FLOOR` the whole
    body is the list, so the whole field is that ground.
  - **A record with no hue gets the room**, not a grey wash. A monochrome
    sleeve — under 2 % of its pixels carrying chroma — has nothing to derive
    from, and the honest answer is `#0C0D0E`, measured exactly.
- **A work's own title is set in IBM Plex Serif Italic** — the museum-placard
  convention, on the strings in the product that are a work's name standing
  beside its own facts. It shipped on one, the Home placard, and doc 14 tier 2
  added the second, the record page's hero. The owner saw the typographic risk
  and approved it; it is one token, pinned by test to an **enumerated** list of
  consumers, so reverting is one line. The face is bundled complete under
  OFL-1.1 (the same licence file, byte for byte, as the bundled sans).
- **Every row-shaped control answers the pointer on the ground it actually
  stands on.** A hover is now one surface step up from the row's own ground
  (`Palette::step_up`) rather than the fixed `plinth`, which was right for
  rows on the wall and silent everywhere else: the **playlist panel's rows**
  and the **context menu's items** both stood on `plinth` and painted the
  colour already under them. The owner named it — *"a more clear indicator
  that something is a click area… right now it's a bit… unresponsive"*. On the
  wall the values are the shipped ones to the bit.
- **`New playlist` is a ghost row** at the head of the panel's list, in a real
  row's exact geometry with the drawn `+` in a recessed sleeve slot. Pressing
  it turns the label into a field in place; `Save` sits at the row's right end
  and is inert while the name is empty or refused, with the storage layer's own
  words under it. `Enter` commits, `Esc` cancels, and the ghost returns after
  a save so the affordance is never consumed.
- **Sound from the wall is one press.** Hovering a record on the wall reveals
  four options laid over its sleeve — `Play`, `Queue`, `Add to…`, `Open` — and
  `Play` sounds the record without opening its page. The owner's approved
  design, and the reversal of the product's *sound from the wall is two
  presses* and *nothing is ever drawn on top of a sleeve*, and of ADR-0032 §2's
  *no hover-revealed verb group*; all three are rewritten to record what was
  decided. The `Ctrl`-click accelerator ADR-0032 §4 left open for the owner is
  **not** taken — *"burying things in modifier keys is not great"*.
  - **The veil is a gradient, never a panel**: it gathers at the sleeve's left
    edge and dissolves to nothing before the right one, so the right of every
    cover stays as painted and the record stays recognisable while you choose.
    Its stops were specified as an sRGB composite and are re-solved for a
    renderer that blends in linear light (`theme::veil_alpha`) — a correction
    that runs the *opposite* way to the 3.7× overdraw `Palette::ink_over`
    documents, because the veil is dark ink over lighter artwork rather than
    light ink on a dark ground. Verified against sampled pixels from real
    frames, not against the arithmetic: worst deviation 0.021 of an opacity at
    both 1280 × 860 and 1920 × 1080
    (`docs/design/impl/hover-options/README.md`).
  - **Four options on one left edge**, glyph then label, each taking a quarter
    of the sleeve's height as its hit band — 47 px at the tightest density baz
    draws, against law L7's 32 px floor. The ink lane ends at the veil's `0.55`
    stop and the hit band at its `0.68` stop, both read out of the veil's own
    stops rather than declared: type stops where the veil still carries it over
    a paper-white sleeve, and the band stops short of the right edge so a
    press on the sleeve **outside** an option still opens the record's page.
    Shift-click still queues; the tile's right-press menu is unchanged and
    remains the pointer-reachable twin of all four options, so nothing is
    reachable by hover alone.
  - **The reveal costs nothing**: it is the `+` slot's own boolean, not a
    tween. No new motion class, no clock, no subscription — 0 frames drawn in
    10 s whether or not a tile is hovered. ADR-0020's five transitions are
    untouched.
  - `Play` wears the accent under the licence `theme::primary` holds — it is
    the control that creates playback truth, and at most one tile is hovered.
    `Queue` is **paper**, one departure from the approved mockup, taken under
    the brief's own licence because the ledger's amber entry says *not what is
    queued* in those words.
  - Two glyphs join the sheet: `Queue` (three bars, the last short — a list
    that runs out, which is what keeps it off the hamburger) and `Open` (the
    disclosure chevron). Options are wall tiles' alone — not the Songs rows,
    not the lane.
- **The sounding record's sleeve is in the now-playing bar**: 52 px square,
  left of the track and artist, *inside* the block's existing hit target so
  the cover and the type are one control that goes one place. It fits the
  bar's existing 80 px band and its named 12 px lead with nothing re-derived —
  52 is the largest square on the 4 px lattice inside the 56 px the tallest
  zone already reserved. No artwork, and the block renders exactly as it did
  before.
- **The wall has a scrollbar, on the window's right edge** — 4 px, no trough,
  the room's own hairline, reserving a lane inside the scrollable so no cover
  is ever drawn under it. The owner's decision (*"just a very minimal scroll
  bar because otherwise, it's hard to just jump to the end"*); the product's
  *two vertical strips may not do one job* entry is rewritten to record it. The
  wall was the only scrolling surface in baz without one — every list already
  had `list_scrollbar` — and the rail is untouched: it still says where you are
  and still names the shelf it jumps to. What the bar adds is the gesture the
  rail has no rung for, because *the end* is not a group key. `INDEX_LANE_W`
  and the rail's width algebra are unchanged; the 4 px comes out of the wall's
  own measure, which the grid absorbs.

  It shipped at the right edge of the wall's *scrollable*, with the index
  rail's 108 px lane standing outboard of it — *"scroll bar is in a strange
  location… it seems to have padding on the right"*. Measured on a 1280 × 860
  frame: the bar at **x 1168–1171**, the rail's letters at x 1226–1239, the
  window's edge at 1280. It now sits at **x 1276–1279**, outboard of the rail,
  on the window's edge; at 1920 × 1080, x 1808–1811 → x 1916–1919. The fix is
  the one the returns lane already made in the owner's words (*"the scrollbar
  should be at the edge of it"*): **the content keeps its inset, only the bar
  reaches the edge.** The scrollable takes the whole body width and reserves
  both lanes (`theme::WALL_RESERVE` 112 = the bar's 4 + the rail's 108), and
  the rail is stacked *under* it — under, because iced hands the topmost layer
  the pointer first and a rail over the bar would be a bar nobody can grab.
  **Nothing else moved**: the bounding box of every differing pixel between the
  before and after frames is the two bars' own columns. The price is stated
  rather than hidden — the rail's press band ran to the window's edge, which
  made an unaimed fling at the edge always hit it, and it now stops 4 px short;
  the band is 104 px wide and what the edge hits instead is the other scroll
  affordance for the same wall. Before/after captures, the ruler and the
  argument at `docs/design/impl/wall-scrollbar/`.
- **The wall's density has a visible control** (ADR-0028; doc 11 §5 P8, the
  owner choosing the visible handle): three detent marks at the foot of the
  index rail's lane — each the wall itself at its hang, one, four, nine works
  in one shared glyph field — named `Spacious` / `Balanced` / `Dense` by their
  tooltips. The current step reads at full glyph ink (never the accent —
  density is not playback truth) and is inert; a press on either other mark
  sends the exact `DensityStep` delta the zoom gesture would spend, so
  <kbd>Ctrl</kbd>+scroll and <kbd>Ctrl</kbd>+<kbd>-</kbd> /
  <kbd>Ctrl</kbd>+<kbd>=</kbd> are now accelerators of a visible control
  rather than the action's only route — closing the law-contradiction doc 11
  named (*no action may be gesture-only*). The view-options rule is
  narrowed under the ledger's editing rule, not deleted: menus, choosers, free
  zoom sliders and Settings rows stay refused, the three named steps stay
  three, the step still persists as state, and the wall's width algebra is
  untouched at every step (the marks live inside the lane's constant 108 px).
  Captures at `docs/design/impl/density-control/`.
- **Search answers in songs** (design doc 09 §5, step 3 of its §13; ADR-0023
  §2's amendment, accepted): while a query is live the Library place's body
  opens with a ranked **Songs** section — up to eight track rows, ADR-0021's
  track ranking surfaced instead of thrown away at the album fold — above an
  `Albums` rule and the wall, filtered as today. Two sections, separate, as
  the owner asked. Each row is a list row: title, `artist · record` with the
  record's name a door to its album page, a right-aligned duration, the
  reserved `+` slot toward a playlist, and the lamp dot following
  `TrackStarted` when its file is the one sounding. **A song row's press is a
  needle-drop**: the record queued whole (the selected edition — a song found
  in one rip resolves into the rip the page would play) with the cursor on
  the song, through the record page's own `play_track`/`play_from` path —
  rows play, tiles navigate, and no third grammar arrives. **Enter retargets
  to the top-ranked song** (was: the best album; the album-level answer
  survives only as the fall-through), so Enter is exactly a press on the
  section's first row. Type-anywhere is unchanged in shape: the first
  keystroke both filters the wall and grows the section in the same frame,
  and `Esc` peels the query as before. The section sits on the wall's own
  ruler — the block width, the shared lanes, the one control height — pinned
  by test; headless captures in `docs/design/impl/songs-search/`.
- **Shuffle, drawing only from what the wall shows** (ADR-0017 step 17). One
  rule and no options: the pool is whatever the active group key, the current
  search query and the current shelf leave visible, and it is **visible** —
  while a shuffle is running, sleeves outside its pool are drawn at 35 % and its
  next two draws carry a faint ink ring. The ring's lane is reserved on every
  tile in every state, so the mark costs no geometry and moves no cover when it
  arrives. What shuffle produces is an ordinary queue of **whole records**,
  eight of them, sent with the same `SetQueue` a double-clicked sleeve sends —
  inspectable and editable in the popover, which now names each record where it
  begins rather than printing forty titles under one album's name. It **ends**,
  in silence, because a shuffle that refilled itself would be the radio
  the product's standing rules rules out. There is no shuffle *mode*, nothing to turn off,
  and no "vibe shuffle": a mood is a group key or a query, so a future `MOOD`
  key needs no new code here and no new control.

  *Superseded 2026-08-10 — see "Shuffle is a property of the player" under
  Changed.* There **is** a mode now, on the owner's decision, and the draw and
  its pool are gone; the "no vibe shuffle" half is untouched. Left standing
  here because this section records what shipped, and a changelog that edited
  its own history would be the one document in the repo you could not use to
  reconstruct one.
- **The pull** (ADR-0017 step 19) — `Ctrl+R`, or the word in the top bar. One
  record from the same pool, weighted by the ledger's own
  `History::pull_weight` (one per day since it was last heard, capped at a
  year, heaviest for one never played, never zero), offered with `Last played
  3 years ago` beside it. **Nothing plays**: the pull selects a sleeve, opens
  its inspector and prints one line; accepting it is pressing the same `Play
  album` any record gets. Pressing again offers a different one; `Esc` puts it
  back. It is presented in the album inspector because the Marquee lens
  (step 18) is not built yet, and the seam it will take over is one struct and
  one function.
- **Motion — five bounded transitions, and a clock that stops** (ADR-0020).
  Every design document baz had written specified hard cuts everywhere, on a
  premise that did not survive measurement: a transition was said to need a
  `window::frames()` subscription "which redraws whether or not anything is
  moving". That is true of an unconditional subscription and false of a bounded
  one, and baz already shipped the bounded pattern twice. `crates/baz/src/motion.rs`
  adds a 48-byte `Tween` — pure, iced-free, told the time rather than asking for
  it, shaped like `shelf::GridHold` — and the shell subscribes to a timer **only
  while something is moving**, which is asserted by a test rather than promised.
  What moves: the icon-button ink fade (90 ms), the queue popover's arrival —
  opacity and an 8 px rise (140 ms), the shelf tile's hover rule (90 ms, one
  tween keyed by the hovered id, never one per tile), the album inspector's
  width (150 ms), and the lamp warming when the light moves to another record
  (200 ms linear). Nothing else: grid stagger, thumbnail fade-in, album-art
  crossfades and any animation of the bar's geometry stay refused. The bottom
  bar's pixel stability is unchanged and is now asserted *during* a transition
  as well as at rest.
- **Icon buttons answer the pointer.** Hovering a transport glyph used to change
  the box around it and leave the mark byte-identical, because the glyph is a
  rasterised sprite and a `button` style's `text_color` never reaches one — so
  the hover and press arms of the transport style were dead code for all six
  icon buttons in the product. The shell now holds which control the pointer is
  on, and the ink ladder completes: **0.57 resting, 1.00 hovered, 0.75 held,
  0.28 dead**, with 90 ms between the rungs.
- **The wall has five arrangements, and it says where the breaks are.**
  ARTIST · YEAR · GENRE · ADDED · PLAYED as one row of words in the top bar —
  no menu, no dropdown, no chip around the live one; the active key is full
  paper in the Medium face and the other four are quiet. <kbd>1</kbd>–<kbd>5</kbd>
  select the same five. The choice is remembered in `config.toml`
  (ADR-0017 step 8, ADR-0019).
- **Shelf headers, pinned.** The wall renders shelves rather than a flat grid:
  each group breaks to a new row under a 10 px tracked-caps header, and the
  header of the shelf you are looking at stays at the top of the viewport while
  its covers pass beneath it. The band is exactly one `HANG`, which is what
  makes the hand-over exact — a header stops being pinned at the same pixel its
  shelf's last row leaves the top.
- **An index rail** down the wall's right-hand edge: a pure projection of the
  active key, holding no state of its own — A–Z for artist, decades for year,
  genre names verbatim, recency buckets for added and played. Clicking jumps.
  Values the collection has nothing under are **drawn**, quieter and inert,
  because an index that hides its gaps lies about the collection; long sets
  elide to the ends plus a window around where you are.
- **The play ledger is wired.** ADR-0018 built it and nothing called it, so
  nothing was being recorded; baz now opens it at start-up and hands it to the
  engine, which is what gives the PLAYED key something to sort by.
- The iced GUI (ADR-0005): first-run screen, album shelf with virtualized
  scrolling and album art, side panel with the track list and an edition
  selector, and a bottom bar with transport, seek groove and now-playing.
- A visual design pass — the "listening room" theme — and a seek groove with a
  click-versus-drag threshold, hover preview and an honest cursor.
- **baz has a typeface.** The IBM Plex superfamily — Sans at Regular, Medium and
  SemiBold, Mono, and Serif — ships inside the binary and is installed as the
  application default. Previously baz asked the *generic* `sans-serif` for
  weights it might not have, and the platform's fallback answered with whatever
  it liked: on one machine every tile title and the first-run question rendered
  in a monospace. The faces are unmodified upstream files under the OFL, with
  provenance and hashes in `crates/baz/assets/fonts/README.md`, and the mono's
  advance width is measured against every reserved slot in the bottom bar so a
  font change can never silently clip a duration.
- **Two inks that failed contrast were corrected.** `PAPER_FAINT`, which carries
  every duration, count, hint and signal note, was 3.4 : 1 on the panel — below
  the WCAG AA floor; `PAPER_MUTED`, the muted fader, was 1.9 : 1, which made the
  position mute exists to restore effectively invisible. Both now clear their
  floors on every surface, and a test computes every ink-on-surface pairing the room can produce, so
  neither can drift back.
- **The lamp is reserved again.** The amber accent means playback truth and was
  being spent on input focus — and the search field takes focus at launch, so
  baz's first frame was an amber ring with no music — on the scanning note, and
  on the first-run wordmark. Focus and text selection are now paper, the scanning
  note is a dim sans sentence rather than an amber figure, and the accent is left
  to the four things that are the playhead plus the one control that creates it.
  A test asserts the accent appears in no other style.
- A **volume control** in the bottom bar: a mute affordance and a fader on the
  same custom groove widget the seek bar was built on, so it inherits that bar's
  cursor, its hover preview (in dB) and its click-versus-drag threshold — and
  the needle that replaced the seek bar inherits all three from the same place. Unity — the
  position at which baz touches not one sample — is reachable by a four-pixel
  snap at the top of the travel and marked by a detent that lights when the
  handle is on it. Drawn in paper ink rather than the accent, because a volume
  is a setting and the lamp means playback truth.
- A signal-path readout in a fixed-width slot beside the fader: the chain
  (`48 → 44.1 kHz`) when the engine is converting, `bit-perfect` when the path
  is literally untouched — a direct chain *and* a transparent volume, which is
  the conjunction ADR-0011 made of ADR-0009's guarantee — and nothing at all in
  between. Same faint ink as the rest of the secondary text, no icon, no fault
  vocabulary, and no layout shift when it appears (ADR-0009 §5).
- **The needle.** A 2 px seek line flush on the window's bottom edge,
  **segmented by the queue's real entry lengths** — so it states position *and*
  structure in the same two pixels: you can see that you are three minutes into
  a nine-minute closer, and where the record you are on ends. The fill is the
  lamp, the unplayed track the room's faintest mark, 2 px of gap between two
  tracks and 8 px where one record ends and the next begins. **Clicking a
  segment plays it** (`JumpTo`); clicking *inside* the entry that is already
  sounding moves the playhead within it (`Seek`) — one gesture, two commands,
  and the segment you pointed at is what says which you meant. Hovering names
  what a click would ask for: a record's title, or a timestamp. A 2 px mark
  would be a 2 px target, so the needle claims its aiming band upward out of
  layout, bounded by the empty lane the bar keeps under its transport, and
  asserted never to reach a control.
- **The now-playing bar is 57 px, where it was 105**, and the collection gets
  **46 px** back — its share of an 860 px window goes from 82.1 % to 87.4 %.
  The needle took the seek row's whole job, so the 260 px groove and its hit
  band are gone; the two timestamps are not, and moved into the bar's left zone
  beside the wall label. **Nothing else was removed.** Previous · Play/Pause ·
  Next stay, on evidence rather than taste: three vendors in baz's own prior-art
  study bought "visual calm" by removing skip and all three reversed inside two
  years, and the hover-reveal that would have replaced them needs the playing
  cover to be on screen, which after a filter or a long scroll it is not. The
  bar's one centre line survived the re-lay — every mark in it now sits within
  1.93 px of the band's mid-line, where the composition audit measured seven
  lines spanning 50 px, and five of them are on it exactly.
- **What is coming next, without opening anything.** The now-playing bar states
  the queue's continuation on a third line under the artist: `then 19 more ·
  57:38 left` while a record is still running, `then Kid A` when one is stacked
  behind it, `then 2 albums · 1:58:00 left` when several are, `then Windowlicker`
  for a single loose song and `then 3 tracks` for a handful — records counted as
  records, never flattened into their tracks. On the last track of a queue it
  says **nothing at all**: silence after a queue is a feature, and announcing it
  would be the announcement rather than the silence. The **Queue** control keeps
  its label and its press and now reads the size of what it opens rather than
  the position in it, because the ambient line states that better. The
  remaining-time figure is the same computation the popover's summary uses, so
  the two cannot disagree; the line's lane is reserved whether or not it says
  anything, so nothing in the bar moves as the queue advances.
- **A visible play queue.** What baz handed the engine, in play order, with the
  playing track marked by the same amber lamp dot the shelf gives the playing
  album, the tracks behind it dimmed, and a `3 of 12 · 51:20` count. It shares
  the right-hand rail with the album panel rather than adding a second one —
  the shelf is the interface, and one panel width is the whole budget for
  chrome beside it — so switching between the two reflows nothing.
  Deliberately a *view*: reordering, removal and click-to-jump each need an
  engine command that does not exist, and `player.rs` names exactly which
  rather than faking any of them.
- **Hideable panels**, the half of the v0.1 sketch that was never built. Both
  rail panels carry a ✕, Escape closes whichever is showing, `Q` toggles the
  queue and Ctrl+B dismisses the rail outright and brings back what was
  dismissed. The shelf reflows to the reclaimed width and re-virtualizes at it
  — five columns to three and back, in the shipped window.
- **A settings panel, and baz's first settings surface.** It is a third panel
  in the same right-hand rail as the album panel and the queue — not a popover
  — so it cannot cover the covers or the transport, it inherits the ✕, Escape
  and Ctrl+B that already dismiss a panel, and adding the next setting is a
  section in one scroll rather than a second surface. The rail is the "one
  deliberate layer down" the vision's progressive-disclosure pillar names, and
  this is now the pattern every future setting follows.
- **ReplayGain controls** in it: the three modes as a segmented control, a
  pre-amp and a separate pre-amp for untagged files (half-decibel steps,
  stopping exactly at the engine's ±20 dB), and clipping prevention. Every
  press sends the whole absolute setting and changes nothing on screen until
  `ReplayGainChanged` confirms it, so a clamp or a second front end is
  rendered as the engine's answer rather than as the request. Underneath, what
  the setting came to for the track playing: `-7.75 dB · from this track's
  ReplayGain tag`, and for an unscanned file `0.00 dB · this file carries no
  ReplayGain, so it plays exactly as stored` — a fact, said differently from
  "off", which states no figure at all because the engine performs no
  arithmetic in that mode. Clipping prevention says what it cut and why. No
  alarm colours, no fault vocabulary, and no amber: the lamp stays reserved for
  playback truth (ADR-0013 §8). **The fidelity indicator is unchanged** —
  it is still `VolumePath` plus `SignalChain`, one gain stage and one answer.
- **The ReplayGain setting is remembered** across restarts, in
  `config.toml`'s new `[replaygain]` table. Written from what the engine
  *confirmed*, never from what was asked for.
- **Keyboard control**: space to play/pause, arrows to seek (shifted for 30 s),
  up/down for volume and `M` for mute, `N` or Ctrl+Right for next, `/` or
  Ctrl+F for search, `Q` for the queue, Ctrl+`,` for the settings, Ctrl+B for
  the panels, Escape to back out.
  While the search field has focus no binding is live — baz asks the toolkit
  whether the widget consumed the key and never second-guesses the answer.
- Presentation split into a `views/` module tree, verified pixel-identical
  across six screens before and after the move (ADR-0006).
- **Controls and iconography, by rule** (design doc 10, ADR-0026 —
  accepted). One rule now decides every control's form: icon-only where the
  symbol is universal *and* the semantics are exactly baz's, word-only where
  the act is baz's own, glyph-plus-word where the act is conventional and
  the scope is not. What that means on screen: the Settings door is **the
  gear** (a 32 px glyph button with a tooltip, in the transport's own
  hover-ink anatomy); the search well wears **the magnifier** and carries
  the collection counts as its placeholder, with the match count
  (`7 / 1284`) in a reserved slot beside the caret; `Play all` leads its
  cluster with the play triangle (no accent — the lamp stays the pages'
  one commitment); and every row slot draws one mark technology — `+`, ↑,
  ↓ and the settings steppers' −/+ are drawn glyphs now, matching the ✕
  beside them in stroke and ink, each named by its tooltip. `Shuffle` and
  `Pull` deliberately stay words: the crossed-arrows convention promises a
  mode baz refuses to have, and the pull has no convention at all.
- **The Library strip has a charter and a budget law (L9)**, asserted in
  code: every tenant declares a reserved width, the sum must fit the
  declared single-line floor, and below 960 px the strip **splits into two
  lines** (frame line: well and doors; library line: arrangement and acts)
  instead of overflowing — which it previously did at the shipped window
  whenever a scan with skipped files was running. The well is fluid
  (280 → 200), 600 px is the strip's floor and the window's declared
  minimum width, and the app's layout estimate reads the resolved strip
  height so the virtualizer cannot be told the wrong regime. The bottom
  bar is deliberately untouched — examined against the form rule and
  passed as shipped, verified by pixel diff.
- The Settings place's header is `place_header` now — the frame is one
  function in five places rather than two that could drift.

**Desktop integration (Linux)**

- **An application icon**, so baz has a face in the launcher, the dock and the
  window list: an SVG master and the hicolor PNG ladder in `packaging/icons/`,
  named by the desktop entry and installed by both the Flatpak and the Linux
  tarball. The mark is a work on the gallery wall under its picture light and
  its wall label — the placeholder tile baz itself draws for a coverless
  album — in the visual language's own tokens, with the accent spent only
  where it means playback truth.
- **MPRIS2**: both interfaces on the session bus, so GNOME's and KDE's media
  controls, the lock screen, `playerctl` and hardware media keys drive baz and
  show title, artist, album and cover art. `Volume` is readable and writable,
  mapped through `baz-core`'s taper in both directions so a lock-screen slider
  and the fader in the window mean the same sound. Position, playback status
  and volume come from engine events only. With no session bus baz prints one
  line and runs exactly as before.
- A desktop entry, and the window's Wayland `app_id` / X11 `WM_CLASS`, so a
  launcher can associate the running window with the entry that started it.

**Forgiveness and the first minute** (design doc 11 §5, the adopt tier)

- **Undo for list edits** (ADR-0027): a bounded history of whole-list
  snapshots per surface. `Ctrl+Z` — or the transient `Undo` word that
  appears beside the Queue place's summary and the playlist page's counts —
  takes back a remove, a reorder or an append. A queue undo restores the
  *list* through `UpdateQueue`, never the playback position: nothing ever
  sounds because of an undo. A playlist undo is one atomic whole-file
  rewrite through the same external-edit fingerprint guard as the edit it
  reverses.
- **Playlist `Delete` moves the file to the platform trash** instead of
  unlinking it (the `trash` crate; freedesktop trash on Linux, so any file
  manager can Restore). One press — the two-press confirm and its sentence
  are retired, because the act is reversible now.
- **The first-run screen gains `Browse…`** — the desktop's own folder
  picker beside the typed path (ADR-0025's two-door shape, arriving at the
  screen it was first deferred from) — and the typed path's check moved off
  the UI thread onto the blocking pool. The window also accepts a dropped
  folder where the toolkit delivers drops (X11; winit 0.30 has no Wayland
  file-drop support), without advertising it where it cannot. The
  `baz DIR` teaching moved to `--help`.
- **`‹ Prev` / `Next ›` in the Album place's header** (doc 07 §3.2's own
  prescription, unpaid until now): step between records in the wall's
  current arrangement — same order, same filtered set — with `Ctrl+[` /
  `Ctrl+]` as the accelerators. Comparing two releases is one press per
  release again.
- **Teaching at the moment of relevance**: the tile menu's `Queue album`
  prints its `Shift-click` accelerator (a word, not `⇧` — the face has no
  arrow and doc 10 §3.6 bans borrowed characters); `Shuffle` and `Pull` carry tooltips
  saying what a press does; the queue's empty state states the refusal
  *with* its answers ("Shuffle draws again; Play all plays the Library.");
  the Songs rule notes "Enter plays the first match."

**Distribution**

- Release workflow building Linux x86\_64, Windows x86\_64 and a universal
  macOS binary from a version tag, gated on the full CI suite, with SHA-256
  checksums.
- Flatpak manifest and AppStream metadata under `packaging/flatpak/`, and
  packaging metadata validated on every pull request.
- [`docs/INSTALL.md`](docs/INSTALL.md) and
  [`docs/RELEASING.md`](docs/RELEASING.md).

**Design records, not yet built**

- **The queue and Now playing are one surface** — the owner: *"the queue and
  the now playing need integrated in some way so we can remove the queue option
  from the bottom bar"*. `Place::Queue` is deleted and its every affordance
  moves into a run column beside the record, in the margin the surface is
  already leaving empty: measured off rendered frames, the merge costs the
  artwork **nothing at 1920 × 1080, nothing at 4K, and 53 px at 1280 with the
  lane expanded**. The bar's `Queue` door comes off and the transport does not
  move a pixel; the 160 px it held goes to the track title. Whether the run is
  shown is a stated, remembered `Run` word — deliberately **not** a function of
  full-screen, because iced 0.13 exposes no monitor enumeration and so cannot
  tell a second-display kiosk from an only-display one.
  [ADR-0029 §8](docs/adr/0029-the-ambient-surface.md),
  [doc 12 §3.4/§5.5a/§6.4](docs/design/12-now-playing-and-kiosk.md), frames at
  [`docs/design/impl/queue-in-now-playing/`](docs/design/impl/queue-in-now-playing/README.md).
- **Every run carries the identity of the list it came from** — the owner:
  *"probably the basic model is that every album has a playlist implicitly… it
  should be basically which playlist and which track"*. `Origin` names the list
  for six kinds where today only a playlist file is named, so
  `Road Trip · 3 of 12` and `Ochre · 2 of 9` become one sentence; a destination
  bit keeps `Add to "{name}"` unrepresentable for a list with no file. Two
  findings changed the shape: `SetQueue` can carry it **without moving one
  pinned wire byte**, and the sixth ledger column the backlog anticipated turns
  out to be actively harmful — the reader rejects a six-field line outright, so
  the ledger gains a **`# baz run` comment marker** instead and no older baz
  reads a newer file as damaged. Closes the owner's attribution defect at
  `docs/BACKLOG.md:9–25`.
  [ADR-0034](docs/adr/0034-the-run-and-its-list.md).

### Changed

- **The well has one meaning and now says so, and it has an `×`**
  ([ADR-0036](docs/adr/0036-the-wells-one-meaning.md)). The owner: *"how the
  search works when we're not on the library needs to be decided… maybe a
  little x or esc to clear would make sense too"*.
  - **It searches the collection, from every place** — unchanged behaviour,
    now a decision. Every road to the query already went to the Library first
    (`App::reach_the_well`): a printable key from anywhere, `/`, `Ctrl`+`F`,
    the collapsed magnifier. What was missing is that the field never said so.
  - **The placeholder names the scope: `Search library`**, permanently, in
    every place — the noun on the destination row two below it, which is where
    the query lands. It costs nothing, because a placeholder is drawn exactly
    when the query is empty and the count's 72 px slot is reserved exactly when
    it is not: the word sets in the field's resting **176 px**, not in the 104
    a query gets. Swept past two longer candidates so a later edit cannot clip.
  - **Contextual search is refused, and type-anywhere is the reason.** A well
    scoped to the page would make the collection unreachable *by typing* on
    exactly the pages a scope applies to — the distinctive gesture revoked
    where the feature is most wanted. It would also mean two live queries or a
    field that empties as you navigate, an `Esc` peel whose length depends on
    where you stand, and — for the honest version, a separate per-page filter —
    a second key, since `/` and `Ctrl`+`F` are spoken for. Exactly one surface
    would have earned one (a long playlist's rows; a record's tracks and an
    artist's records are short enough that a filter is noise, and the run is
    the one list you reorder by dragging). Recorded in `docs/BACKLOG.md` with
    its shape.
  - **The `×` is in the mark's own box, on the left**, because the field's
    right edge is full: `GAP_MD` + `SIDEBAR_MATCH_W` 72 is sized for
    `1284 / 1284`, and a glyph box beside it would take the query's room from
    104 px to 80 — below the 88 the design measured and rejected. The mark's
    box is already `SIDEBAR_GLYPH_BOX` 24 (which is `STEPPER_HIT`) on the
    destinations' glyph vertical, so the swap moves nothing on either edge:
    **the magnifier at rest, the cross while a query stands.** A field with
    text and a count in it does not need to be told it is a search field.
  - **The `×` is `Esc`'s pointer route** — the identical function
    (`Shelf::clear_query`), so the query goes, the caret leaves the field and
    the transport gets the keyboard back. Drawn exactly while a query stands,
    which is exactly when the key has that layer to peel; tooltipped
    `Clear the search (Esc)`. Both wells draw it from one function, so the
    pointer route exists at every width the keyboard route does.
- **The search well's second line is gone; its two figures went to two
  different places** (ADR-0030's fourth amendment). The owner, reading the
  shipped lane: *"the album and track count below the search bar doesn't look
  good… maybe this should go into the home as some basic stats?"*. The line
  carried `25 albums · 206 tracks` at rest and `12 of 25 albums` while
  narrowing, and those were never one readout: the resting counts are a
  **statistic about the collection**, standing in the lane's most valuable
  space with nothing being searched, and they are the Home place's
  `COLLECTION` footer now; the match count is **feedback about the query**, so
  it moved *inside* the field it answers, right-aligned in a 72 px slot, where
  it costs no line at all. The well's block falls from 52 px to 32, and the
  `RECENT` list gets the difference: **11 rows at 1920 × 1080 where there were
  10**, measured off the frames rather than predicted — at 1280 × 860 the
  20 px buys three eighths of a row and the count stays at 7. The strip's own
  well, drawn only below `SIDEBAR_FLOOR`, is unchanged — a strip is one control
  tall and never had a second line to lose.
- **Home's `CONTINUE` band is the question you ask in the silence** (ADR-0030's
  third amendment). It used to be a reading of the launch snapshot, so pressing
  `Resume` left a frozen placard describing where you *were* on screen while
  something else was sounding. It is now one predicate: **the band stands
  whenever there is a run to carry on with and nothing is sounding.** Start
  anything, anywhere in the product, and it is gone; stop, and it is back,
  describing where you now are. Paused, it shows **what you paused** at the
  engine's own position; a run played to its *end* leaves no band, because a
  finished run has no "where you stopped" and the silence at the end of a run is
  a feature; only before anything has sounded is the snapshot read at all. It
  costs nothing at rest — a band that is absent while the music runs wants no
  position, so Home has no clock and no subscription, and the needle it draws is
  one the engine has stopped moving.
- **`Resume` starts the run and takes you to `Now playing`** — the one play
  gesture in baz that navigates, and deliberately: `Play` says *play this* and
  leaves you where you are choosing from, where `Resume` says *pick up where I
  left off*, which the place describing where you are is the answer to. It has
  two shapes now, matching the band's two: a paused run is a plain `Play`
  (seeking it back to the snapshot's cursor would restart the track you are
  halfway through), and the interrupted run is `JumpTo` then `Seek` as before.
- **The queue survives a quit** (ADR-0023 §6, unbuilt until something wanted
  it). `session.toml` beside the config holds the paths, the cursor, the
  elapsed position and the provenance, written when the run moves and again on
  the way out. On launch the run is handed back to the engine **loaded and
  silent** and the interrupted point is one press away on Home's `CONTINUE`.
  One deviation from §6's letter, argued in `session.rs`: the engine's command
  table makes *loaded and paused at a non-zero cursor* unrepresentable without
  changing the engine, which §6 costed at zero. The clause that matters is
  kept — nothing sounds unasked.
- **A place's body is sized against the window less the returns lane**, in both
  axes (`Shelf::body_width`, `App::body_height`). Every in-place breakpoint —
  the strip's two-line split, the album page's two columns, the Settings
  measure — resolves against the body rather than the window, because a body
  that split against the window would split at the wrong moment the instant a
  column stood beside it.
- **`Ctrl`+`B` returns.** Doc 07 §5.3 retired it when ADR-0022 left no sidebar;
  its subject is back and its meaning is unchanged, which is the only condition
  on which a retired reflex may be revived.
- **`Place::is_home` is `Place::is_library`.** It always meant *the collection
  is on screen*; the name only became wrong when a place was actually called
  Home. `Place::Library` is still the launch frame and still what `Esc`
  returns to.
- **One vocabulary in the shipping copy** (doc 11 §5 P4): every place
  header now says *"Esc returns to Library"* — "the wall" was the design
  corpus's own name leaking on screen — the record page's `Add to…` names
  its object as `Add to playlist…`, and a sweep test pins the whole
  room-vocabulary list out of user-facing strings (it caught two further
  leaks on arrival: the first-run scan line and the empty library's
  heading).
- **`config.toml` is now read and written with the `toml` crate**, replacing
  the hand-rolled single-key writer whose own documentation named this as the
  plan of record once the configuration grew past a couple of keys. It grew
  today. Three crates enter the lock file (`toml`, `serde_spanned`,
  `toml_writer`; the parser and `serde` were already there), all MIT OR
  Apache-2.0. Reading is per-key and defensive rather than derived: a value
  baz cannot understand takes its own default and its neighbours are
  untouched, so a mistyped pre-amp cannot cost anybody their music folder.
  A non-UTF-8 music directory now omits its key instead of preventing the
  whole file from being written.
- Application id is now the reverse-DNS `io.github.mattcree.baz` rather than
  the bare `baz`: the desktop entry's basename, the AppStream component id, the
  Flatpak id, the window's `app_id` and MPRIS's `DesktopEntry` property are one
  string, and Flatpak requires that string to be reverse-DNS. The MPRIS *bus*
  name is unaffected and remains `org.mpris.MediaPlayer2.baz`.

### Added

- **`All songs` — the implicit playlist, given a type.** The owner: *"the
  'all songs' should be an implicit playlist."* `docs/design/09-implicit-playlists.md`
  §2 had listed *"the wall, in its arrangement"* among the implicit playlists
  since the study was written, but the vocabulary was design language and not a
  thing in the code — `grep -rn "implicit playlist" crates/` returned one
  comment. It is `crate::implicit::ImplicitList` now: a name, a counts line, a
  collage sleeve, and a row at the head of the playlist panel, above the
  unnamed sounding list it is a sibling of.

  **The type is the kind, not the instance**, on the owner's steer that *"the
  basic model is that every album has a playlist implicitly… it should be
  basically which playlist and which track"*. So it is an `Origin` with a
  variant rather than a bespoke `AllSongs` struct, and each origin carries the
  identity that kind actually has: a named playlist has a **file**, an album's
  list would have an **album id**, a draw has **nothing durable**, and
  All songs has only a **name**. Only the All-songs origin is built — the
  others are recorded in `Origin`'s own docs with what each would carry, so
  adding one is a variant and a constructor. The state-tracking half of that
  model (what "recent plays" means when the ledger records one line per track
  path and the engine is never told a run's origin) is separate design work
  and is deliberately not decided here.

  Two things fall out of building it as a kind. `Origin::file()` answers
  `None` for every origin, and the panel *reads* that to decide the row cannot
  be a pick destination — so a later origin inherits the refusal instead of
  needing to be remembered about. And `narrowed_from` is an `Option`: All songs
  is a view of the library and prints `7 of 25 records` under a query, where a
  list whose extent is fixed by what it is (an album's tracks) has no whole to
  be part of and must not invent one.

  **`Play all` is its `Play`** — one concept where there were two. The strip's
  word no longer builds a queue of its own; it resolves the list and plays it.

  **It is ordered by the wall's arrangement and the wall's filter**, and it says
  so rather than pretending to be a snapshot: playing it twice with the wall
  unchanged is the same list, and what changes it is a control the listener
  pressed. Under a query the counts read `7 of 1284 records` rather than
  letting the name claim otherwise, because a list called *All songs* holding
  seven of twelve hundred would be lying.

  **It is playable and viewable, never a destination.** There is no file behind
  it, so the picker never offers `Add to "All songs"` — swept in `menu.rs` over
  every target and every reachable set of facts, and closed at its source in
  `implicit.rs`, which pins that the list's run carries no provenance (giving
  the wall's run provenance is exactly what would put the name into a transfer
  verb).

  **Where you look at it is the wall.** Doc 09 §2's own table already answered
  that — *"the wall itself"* — and a second page listing the same music as text
  would be doc 07 L8.6's one fact drawn twice, drawn worse and without the art.
  The panel's row is the handle; its press goes to the Library.

  ADR-0024 §1's definition of a playlist (*"made by a person, stored in a file
  that person owns"*) is amended to say why All songs is deliberately **not**
  one, and why the definition is not widened to swallow it.
- **`All songs` has a face: a tile on Home.** The owner: *"again I wanted the
  Play all, to be more like a tile on the home screen, a special 'playlist'"* —
  and the *again* is the point, because it had been asked for before and not
  built.

  It is the wall's own tile, to the token: the grid's art edge, the sleeve
  inside its mat, the two-lane caption box, the state rule, and the wall's own
  hover veil — built by the wall's own function rather than by a second one that
  looks like it. Two options where a record has four, and the two it does not
  have are the two an implicit list cannot answer: `Add to…` is refused by
  construction (there is no file), and `Queue` would append a library to a run.

  **It wears a list's collage sleeve** rather than a designed face. The
  objection to the collage is real — four arbitrary covers claim to characterise
  a list whose definition is *no selection at all* — and it loses to a larger
  one: the playlist panel's `All songs` row already draws exactly this collage,
  and a second face for one list is worse than a restless one. A typographic
  face fails twice over, because the figures it would carry are the `COLLECTION`
  footer's own, three sections down the same page.

  **Second on the page**: under `CONTINUE`, above `RECENTLY ADDED`, ordered by
  how *particular* each offer is. `CONTINUE` is your own interrupted run and is
  absent most of the time; with it absent — the page's ordinary state — the tile
  is the first thing on Home, which is right for a door. No section rule over
  it: a rule names a set, and this is one thing that names itself in its own
  caption.

  **It plays the collection whole**, not whatever the wall is filtered to, and
  it says which in its counts line. Home shows no wall and no query, so a tile
  there that applied a filter set on another page would be acting on state the
  listener cannot see or clear from where they are standing.

  **The strip's `Play all` stays**, and the two are not one control at two
  sizes: `Play all` lives beside the query and the arrangement that decide the
  wall and plays exactly what the wall shows — the only way to play seven search
  results — where the tile is the way into all of it. One list, one origin, one
  sleeve, one `Play`, two scopes, each stating its own. `ACTS_W` is untouched:
  nothing left the strip, so the acts lane's budget does not move a third time
  in one day.

  `Origin::file()` still answers `None`, so the picker still refuses the list as
  a destination by construction. Recorded in ADR-0030's fifth amendment, which
  tests the addition against §6's own inventory rule and confirms the five
  refusals are untouched.

### Changed

- **Shuffle is a property of the player, and of the *walk* rather than of the
  list.** The owner, twice in one day: *"can you make shuffle a property of the
  player i.e. toggle on/off"*, and then, on seeing what that shipped as, *"I
  think shuffle as a concept is more about going to an unknown next track
  rather than actually mutating the track list if that makes sense."*

  It was one press in the Library strip that drew eight records out of the wall
  and started them, with nothing to turn off. It is now a **toggle** — the
  crossed arrows on the now-playing bar, lit in the accent while it is on,
  remembered in `config.toml` beside the other standing decisions — and what it
  changes is which track is chosen next. **The queue is never permuted.** It
  keeps the order the gesture that built it laid out, in both positions of the
  control and whatever else happens to it.

  **The selection rule, in one sentence:** with shuffle on a run plays a
  **bag** — one deterministic shuffled pass over the run's entries, in which no
  entry repeats until every entry has played, and when the bag is spent the run
  ends. A uniform draw can play the same track twice running and leave another
  unheard for a whole album; a bag is what the word means to people. The bag is
  **not** re-rolled when it empties (a fresh pass comes from a fresh gesture)
  and **not** re-rolled by a jump — jumping moves the cursor within the bag, so
  `Next` and `Previous` land where the run was actually going. A fresh seed per
  run, so the same record played twice is two different shuffles.

  **The decision lives in the engine, because baz is gapless.** `baz-core` gains
  one standing property — `traversal`, set by `Command::SetTraversal` and
  answered by `Event::TraversalChanged` — and nothing else: no repeat flag, no
  continuation policy, nothing that refills. It has to be the engine: gapless
  means the next track is decoded *while the current one plays*, and the only
  way a front end can name the next track is by sending a queue, which ADR-0014
  documents as costing the following boundary its sample-accurate splice. One
  edit, one boundary is a fair price for an edit; a mode that charged it at
  every boundary of a shuffled run is not. Internally a session is handed an
  **itinerary** — the entries it will play, in the order it will play them —
  so the decode-ahead loop is untouched to the line, and every existing gapless
  test passes unchanged. The new one,
  `a_shuffled_run_is_gapless_and_bit_identical`, plays a two-track queue under a
  reversing traversal and compares the delivered stream sample for sample
  against the reference decodes concatenated in the bag's order.

  **baz says what is next.** The order is decided in advance, so baz knows it,
  and the run column says so: the row that plays next carries an **open ring**
  where the sounding row carries the filled lamp dot, and the entries the pass
  is already past are dimmed. The bar's continuation counts the bag's
  remainder rather than the list's tail, and the popover's `3 of 12` counts
  how far through the pass you are rather than which row you are on. The mark is
  drawn with shuffle **off** too, where it is simply the row below: a fact that
  is true in both modes, and an interface that only marked what is next when it
  was surprising would be one that had decided when you are allowed to know.

  **Turning it off never stops the music, and never touches the run.** The
  sounding track is delivered to its end and the run continues on the new plan
  after it — ADR-0014's existing handover at its existing price of one
  boundary.

  **Every play gesture agrees**, structurally rather than by convention: press
  `Play` on a record with shuffle on and the record plays shuffled, and
  `Play all`, a playlist's `Play` and a track click all hand the list their
  gesture means to the same function. A **track click** needs no special case
  any more — starting at a row and continuing by the plan is exactly what
  `JumpTo` does.

  **The crossed-arrows glyph, taken.** `docs/design/10-controls-and-iconography.md`
  §3.2 refused it *only* because the symbol promises a mode with a lit state
  and baz's shuffle was an act — a conditional argument that named its own
  condition. It is a mode now, so the clause is rewritten and the symbol is
  honest. It went to the **bar** rather than staying in the strip because a
  control goes where what it reads is: what a mode reads is the player, and the
  player's surface is under every place where the strip is under one.

  **What went with the act**: the wall's shuffle pool and its two marks — the
  35 % dimming of every record outside the draw, and the ring on the next two.
  Both existed to answer one question about a draw whose source was only
  implied: *what can this shuffle play?* A mode has no source of its own, so
  **the pool is the run** — the queue, a place you can open and read row by
  row — and the question answers itself. The ring's reserved lane stays as the
  sleeve's mat; only the ink went.

  Recorded as **one decision** in ADR-0023's amendment (rewritten in place
  rather than corrected), in ADR-0024 §1's first honesty clause, and in doc 10
  §3.2 where the crossed-arrows clause lives. Captures:
  `docs/design/impl/shuffle-and-all-songs-tile/`.

### Removed

- **The `Run` word, and the two densities it chose between.** The owner:
  *"remove the run button from the now playing"*, and, when asked to be sure
  which control was meant, *"run button is what I'm referring to; just to be
  clear"*.

  **The run column is not what went — it is what stands.** The list, the rows,
  the steppers, the ✕, the transfer `+`, drag-to-reorder, `Undo`, the
  provenance-led summary and the virtual window are all untouched
  (`every_queue_affordance_survives_the_merge` is unchanged and still green).
  What went is the *word* in the place's top-right and everything that existed
  only to remember which reading it had last chosen: `Message::ToggleRun`,
  `App::run_column`, `App::toggle_run`, `App::set_run`, `persist_run_column`,
  the `run_column` key in `config.toml`, the `run: bool` parameter of the
  place's `view`, `theme::now_playing` (its last consumer was that word), and
  the `clippy::struct_excessive_bools` expectation on `App`, which fell silent
  by itself once the flag went.

  **The run column now stands whenever there is a run, and nothing else decides
  it.** That is a reversal of M1's *"the density is a stated control"*,
  recorded as the owner's decision rather than argued with: a surface whose own
  argument is *a run is a list and a cursor* was offering a control that hid
  the list. It also removed a lie — with the density off and a stopped run
  loaded, the place printed *"Nothing queued"* over a queue it was holding.

  Two consequences worth naming. `Message::ShowTheRun` folded into
  `Message::ShowNowPlaying`, because with no density to set it was that message
  with a longer name; <kbd>Ctrl</kbd>+<kbd>U</kbd> now sends the message two
  visible controls already send — the lane's `Now playing` row and the bar's
  now-playing block — which is a simpler legality than the two-message
  construction it replaces. And the run column lost its 48 px `clearance`
  strip, which was air reserved for the word; step A6's `Ambient` door brings
  its own back if it claims that corner.

  A `config.toml` carrying `run_column = false` is read without harm and the
  key is not written back — a listener who had the density off upgrades into
  the surface with *more* on it, never into a blank half
  (`the_retired_density_key_is_neither_written_nor_honoured`).

- **`Pull`, and everything that existed only for it.** The owner: *"please can
  we remove pull since it doesn't make sense here."* Gone: the Library strip's
  `Pull` word and its tooltip, <kbd>Ctrl</kbd>+<kbd>R</kbd>, the record page's
  `The pull · Last played 3 years ago` line, the draw itself (~125 lines of
  `shuffle.rs`), and `baz-core`'s `History::pull_weight` with `PULL_DAY_CAP`
  and `PULL_NEVER_WEIGHT` — the weighting had exactly one consumer, and a
  weighted draw nothing draws from is a recommendation engine's foundations
  left in the ground. **Shuffle was untouched** at the time: the two shared one
  function, `shuffle::Pool::from_wall`, and shuffle owned it — until the pool
  went with the draw later the same day, and then the module went with the
  permutation (below).

  This is also the third answer to `docs/design/11-jobs-era-critique.md` **P9**
  (*"`Pull`: explain it or rename it"*), which was an open question addressed
  to the owner: he removed the control instead. Recorded there, in
  ADR-0018 §6, whose third read surface is struck with its behaviour preserved
  in the text so it stays findable. Two knock-ons worth naming: the copy
  sweep's licence list held `Pull` and `The pull` and nothing else, so **P4's
  one-vocabulary rule is now total**; and the strip's acts cluster fell from
  182 px to 144, taking the two-line split seam from 872 to 834.
- **The whole of the front end's `shuffle` module, and the machinery that
  existed to keep two orders in step.** Shuffle became a property of the walk
  rather than of the list (above), so turning it off is trivial: nothing was
  ever changed, and there is nothing to put back.

  Gone with the permutation: `crates/baz/src/shuffle.rs` entire —
  `SourceOrder`, `source_order`, `arranged`, `restored`, `leading`, and the
  `SplitMix64` and Fisher–Yates it carried (those two moved to
  `baz_core::traversal`, where the engine and the front end share one function
  and therefore one answer). Gone from `PlayerState`: the retained
  `Option<Vec<PathBuf>>` and its four methods (`source_order`,
  `note_shuffled_run`, `retain_source_order`, `forget_source_order`), and the
  `bool` that made the struct need an `#[expect(clippy::struct_excessive_bools)]`
  — the allowance went with the flag. Gone from `App`: the branch in `send_run`
  that permuted a run and the one in `toggle_shuffle` that un-permuted it, and
  the two `forget_source_order` calls the reorder handlers made.

  Gone from the *rules*, which is the larger part: the retained order's two
  invalidation conditions; the restore walk and the three consequences it had to
  define (a deleted row staying deleted, an appended row staying appended, a
  repeated file being put back twice); the "a run restored from a snapshot has
  no retained order" case and the stdout line that explained it; and the hoist
  that made a track click's clicked row lead a permuted body. Every one of them
  was a rule about keeping two orders in step. There is one order now, and it is
  the run's.
- **The playlist delete confirmation.** *"Delete "{name}"? The file goes;
  your music stays."* was the correct fallback while deletion was
  irreversible; the trash makes it reversible, and the 1992 HIG's ranking —
  reversibility first, warnings only where undo cannot reach — then retires
  the dialog (ADR-0027). The product now ships zero routine confirmation
  dialogs.
- `.opus` no longer appears in the library. Symphonia ships no Opus decoder in
  any released version, and the alternatives cost either a C library on every
  platform or an unmaintained parser on a path that reads hostile input.
  Advertising a file baz cannot play is worse than not listing it; the three
  things that would reverse this are recorded in `docs/BACKLOG.md`.

### Fixed

- **A shuffled run's continuation counts records, not visits.** The owner: *"the
  album count in the bottom bar when in shuffle mode is weird... way too many
  albums shown"*. The bar's ambient line groups the queue's remainder so that a
  stacked record reads as one thing rather than eleven, and it did that by
  folding *adjacent* items sharing an album title. Under a shuffled pass the
  items of one record are no longer adjacent, so every return to a record
  started a new entry: a three-record run read `then 10 albums` on the first
  seed tried.

  The grouping is now conditional, because the old rule is right about the case
  it was written for. Adjacency encodes a fact about **the listener's own
  order** — a record stacked twice with something between it really is two
  things, and the run being broken is the listener's doing. A shuffled walk has
  no such order to break; it simply returns. So albums fold by title under
  shuffle and by adjacency without it, and
  `a_record_stacked_twice_is_two_entries` keeps its deliberate reading
  untouched. Loose songs are distinct things either way and were left alone.

  Pinned twice over: one test asserting both readings of the same five items,
  since the difference between them *is* the fix, and one sweeping 32 seeds of
  the shuffle the player actually performs, because a single permutation proves
  only that one permutation is safe and this defect does not appear until the
  walk happens to return.

- **`Now playing` shows what the bar names, whether or not it is sounding.**
  The owner: *"it should probably just show whatever the now playing is
  indicating, just not playing"*. The two halves are now read from the two
  questions the bar under the place already answers and from nothing else — the
  record column draws when `PlayerState::now_playing` answers, the run column
  when `queue_list` does — so the place cannot contradict the bar beneath it. A
  paused run, and a run restored from `session.toml` at launch, both draw.
  The empty state is reachable in exactly one case: no record and no run.

  **The record's column is now drawn even when there is no record**, which
  fixed a composition that disagreed with itself: with a run standing and
  nothing sounding, the body dropped to the re-stacked single column while the
  field still painted `Ground::Split`, so a scrolling list sat under ambient
  light at a full-width window. The field was right and the layout was the half
  that was wrong. It also means a loaded run becoming a sounding one moves not
  one pixel of the list.

- **The `Nothing queued` state is inset like the rows it replaces.** The owner:
  *"the nothing queued thing is hugging the left with no padding"*. It was
  handed to a centring container at `width(Fill)`, which defeats the centring
  and lands the block flush against the window's edge; it is now drawn in the
  run column's own frame — the place's gutter, and the measure the rows are set
  at. The other two empty states were checked and are correct: the wall's is a
  shrink-width block genuinely centred, and the playlist page's sits inside
  `place_pad()` on the same heading lane as the `Tracks` rule above it.

- **The ambient field runs under the run column, continuously.** The owner:
  *"the background fade behind the album art seems to abruptly end beside the
  track list which looks bad -- the fade should continue under the playlist
  area too"*.

  It was two washes drawn side by side — the ambient one, and a second clamped
  flat to `wall`'s lightness under the rows (doc 12 §5.4 term 2, ADR-0029 §8.4)
  — and that is worse than the lightness step it was designed as: **two
  gradients do not step at their join, the second restarts the ramp**, so the
  seam was a hard vertical edge announcing the layout. `field::Reach` and
  `now_playing::Ground` are both deleted; there is one wash over the whole
  body.

  **The clamp existed for a real reason and the answer to it is a
  measurement**, not nerve. `every_run_row_is_legible_over_the_brightest_field`
  sweeps every room × every hue × every ink the run column draws against the
  field's own brightest stop, at the floors each ink's use implies. The field
  costs every ink about an eighth of its ratio and no ink its floor — in
  Closing Time, `paper` 15.33 → 13.54, `paper_dim` 8.20 → 7.24, `paper_faint`
  5.34 → **4.71** against a 4.5 floor, `paper_muted` 3.61 → 3.19 against 3.0,
  `alert` 6.30 → 5.57; Reading Room within 0.02 at the binding inks. The
  binding case is `paper_faint` at 4.7 % of margin, and it is the number to
  re-check before `field::CEILING_L` is ever raised.

  `the_composition_holds_across_the_restack` lost a third of its subject and is
  better for it: it existed to prove the field's domain and the columns' split
  turned at the same width, and there is now one number where there were two.

- **The run column follows the music.** The owner: *"ideally the currently
  playing item in the playlist is where our scroll goes to i.e. it should be
  visible when we change track"*.

  Three rules make it bearable rather than annoying, and all three are in
  `views::now_playing::follow`. It moves **only on the engine's own
  confirmation** — `track_seq` changing, which is a new track and never a seek
  and never a clock. It moves **only when the sounding row is not already
  visible**, which is most track changes inside one record, so the ordinary
  experience of a twelve-track album is no movement at all. And it **does not
  fight a manual scroll**: nothing notices that the listener has scrolled away,
  because the next track change is the only boundary at which they are not
  mid-gesture. The row lands two rows' worth of list down rather than flush at
  the top, so what you have already heard stays visible behind the cursor.

  **Arriving at the place is the same computation**, which supersedes the
  merge's `queue_scroll = 0.0`: opening the place on the top of a run you are
  forty tracks into is the same defect reached by a different door.

  `queue_window::row_box` is the new arithmetic, and it is deliberately the
  same walk over the same pitches `queue_window::window` makes — otherwise a
  follow to a row outside the built slice would scroll into a spacer.
  `a_follow_lands_on_its_row_inside_the_built_slice` sweeps a 4 000-row run at
  three viewports and asserts the target is both built *and* on screen.

  **The playlist page and the record's page deliberately do not do this.** They
  mark the sounding row with the same lamp dot, so the argument nearly carries
  — but they are documents you are reading, where the lamp is an annotation,
  and the run column is the list you are hearing, where the cursor is the
  subject. Moving a document under its reader is the defect, not the fix.

- **The run column's scroll offset is reset on every route into the place.**
  `queue_scroll`'s own note says it must be zero when the place is entered,
  because iced 0.13 keys widget state by tree position and a remembered offset
  windows rows the widget is not showing. Only `Message::ShowTheRun` wrote it;
  the lane's `Now playing` row and the bar's now-playing block did not, so
  those two arrived with a stale offset. Deleting that message forced the
  question, and the reset moved to `note_place_left`, where every route passes.

- **`Save as playlist` is offered only for a run the listener assembled.** The
  owner: *"I still see save as playlist on the queue when playing a CD... we
  should only be showing that in a situation where there isn't an existing
  playlist"*, and then, narrowing it: *"nah I think adding more stuff to an
  existing playlist is fine, that does not need a save -- it's a low bar to
  edit a playlist"*.

  `QueueVm::provenance: Option<String>` — the playlist file's name, or nothing
  — becomes `QueueVm::source: RunSource`, the **three kinds of list** he named:
  `Fixed` (a record's track listing, `All songs`, `Play all`), `Playlist(name)`
  (reified from a file), `Assembled` (built by hand). The old reading could not
  tell *a list that exists without a file* from *a list that does not exist at
  all*, so a CD's run and a hand-built queue were the same `Unfiled` and both
  were offered the creation act. The predicate wanted was never *has a file*
  but **did the listener assemble this**, so it is spelled as a kind: a later
  origin lands on `Fixed` and inherits *offer nothing*, which is the safe
  direction.

  The strip, in four states: `Fixed` says nothing (in a reserved slot of the
  strip's own height, so `rows_top` stays true); `Saved as "Road Trip"`
  unedited; **`From "Road Trip"`** once edited — it may not keep claiming to
  *be* the file, which is the lie ADR-0024 §A5.2 removed, and it may not offer
  a new one either; `Save as playlist`, live, for an assembled run **and for a
  fixed run that has been edited**, because that has become something that
  exists nowhere else and there is no file to go and edit instead.

  **Nothing writes back.** ADR-0024 §1's decoupling and ADR-0023 §3's
  origin-never-a-link are untouched: *"a low bar to edit a playlist"* is an
  argument about how easy the playlist's page is to reach, not licence for the
  queue to edit files somebody owns. The run's kind survives a quit
  (`session.toml` gains `assembled`), so the strip offers the same word
  tomorrow that it offers tonight; the *edit* flag is session state and
  deliberately does not.


- **The decode was enlarging covers smaller than its own tier.**
  `image::DynamicImage::thumbnail` scales **to fit**, in both directions, and
  `art.rs` has called it *downscale-only* since v0.1: a 120 px cover was decoded
  to 320 × 320 and cached with 6.8× more pixels than the file had. It never
  showed on the wall, where a 320 px handle in a 320 px tile is 1 : 1 either
  way, and it shows immediately on a surface that reads the decode's size and
  **believes it** — which is what step A2's `art_edge` now does. Both tiers are
  guarded.
- **The now-playing placard reserved 16 px less than it draws**, so the two
  timestamps under the needle were dropped off the bottom at every
  height-bound size with the run standing. The old expression rolled the
  placard's five `GAP_XS` and the needle's own tick into `4 · GAP_LG`;
  `NOW_PLAYING_MAX` 720 hid the shortfall by leaving slack, and deleting the
  clamp spent it. The reservation is now spelled term by term as the layout it
  describes.
- **The re-stacked head block reserved the record column's height**, 108 px
  more than its own — and that number is the offset the run's virtual window
  measures its slice from, so a scrolled restacked surface could draw the wrong
  rows. It reserves its own.
- **A record's hover options no longer open on another place.** The hovered
  tile is remembered by the shelf and cleared by the pointer *leaving* it — so
  navigating out from under the pointer (a tile's own press, or a keyboard
  door) left the mark set. Invisible while the wall was the only surface
  drawing tiles; Home's `RECENTLY ADDED` row made it possible and the Artist
  place made it plain, offering a record's `Play` / `Queue` / `Add to…` /
  `Open` for a record the pointer was nowhere near. The hovered tile is now
  cleared where the open menu and the in-flight drag already were.
- **Playing a list no longer fills the returns lane with the records it
  quotes.** The owner: *"the recent bit shows albums popping up even though it
  was the playlist which was played"*. The lane's `RECENT` half is folded out
  of the play ledger, which is per *track path*, so a run reified from a list
  marked every album the list touched — several unrelated records jumping to
  the head, one per track, while the list that was actually played sat where
  its file's mtime had left it. **A play now attributes to the run's
  provenance**: a run reified from a list touches the *list*, and every other
  origin (a record, `Play all`, a shuffle draw, a stacked queue) touches the
  record exactly as before. A list is touched by being played as well as by
  being edited, and the lane takes whichever is later. One shortfall stands and
  is recorded in `docs/BACKLOG.md`: across a quit the attribution falls back to
  the ledger's, because the engine is never told a run's provenance and fixing
  that reopens ADR-0018's ledger format.
- **Opening baz and closing it again no longer costs you your place.** The
  guard that stops a restored run overwriting the interrupted point protected
  the *"the run moved"* writer and not the *exit* writer, which writes
  unconditionally — so launching and quitting without pressing anything wrote a
  cursor of 0 and a position of 0 over the snapshot. Both writers now share one
  pure function (`app::next_snapshot`), stated as **has anything sounded**
  rather than *is a row playing*, which is also the only fact that separates a
  run restored at launch from a run that has just ended.
- **A library that is not mounted yet no longer deletes the interrupted run.**
  A snapshot whose files do not resolve produces no queue, and the old *no
  queue ⇒ write an empty snapshot* arm then cleared the file outright. A NAS
  that was not up when baz opened is an ordinary thing to meet.
- **The `Now playing` place no longer flashes "Nothing playing." on the way
  in.** `Resume` navigates there in the same press that asks the engine to
  begin, and the engine's confirmation is a frame or two behind it. While a
  transport command awaits its confirming event the surface stays bare instead:
  a sentence that appears and vanishes is read, and a blank that fills is not.
- **A run played to its end is written away** rather than left on disk, so the
  band cannot come back after a relaunch offering to replay something you
  finished — the same judgement the band makes on screen, made once on disk so
  the two cannot disagree.
- **A returns-lane row's sleeve is 48 px, not 40.** The expanded lane drew the
  *playlist panel's* `PANEL_SLEEVE`, which left `SIDEBAR_ROW_H`'s own
  derivation — 48 with one `GAP_SM` above and below — describing a row nothing
  was drawing, against doc 13 §9.2's 48. Measured off the shipped frame at
  40 × 40.
- **The returns lane marks the record that is sounding.** Doc 13 §2.6 promised
  the lamp dot before its name; every row drew the row style with `playing`
  hard-coded false, so the surface whose subject is *things you have touched*
  could not say which of them was on. The dot and the row's card now — the
  vocabulary the queue and a playlist's page already use — and the card is what
  survives the collapse, where there is no name to set a dot before. A list is
  never marked, however many of its tracks are in the run.
- **A playlist's sleeve gets real artwork rather than getting it by luck.** The
  collage is read out of the wall's thumbnail cache and nothing was putting the
  records a list quotes *into* that cache: the lane's own art request yields its
  **records** and skipped its **lists**, so a list wore the deterministic
  gradient until one of the records it quotes happened to scroll onto the wall.
  Four ids per list are now asked for by name, on the same guard, through the
  same cache.
- **The strip's height and the strip's width agreed about the split.** The strip
  is drawn at the window less the returns lane, but `theme::top_bar_h` was still
  being handed the *window*, so between a 1000 and a 1056 px window with the
  lane open the strip drew two lines while the virtualizer's viewport estimate
  assumed one — 40 px of mis-virtualized shelf. It takes the window and the
  lane's state now, which is what the composition takes.
- **The reorder steppers now render.** The playlist page's ▲▼ shipped as
  U+25B4/U+25BE literals, but IBM Plex Sans — the product's one face —
  carries no triangle glyphs at any code point, so the steppers rasterised
  as tofu boxes; no capture had ever hovered a row to see it. Both editors
  (the playlist page, and the queue place that joined it at doc 09 §13
  step 5) now use ↑/↓, which are in the face. Caught by the queue-parity
  captures' first run (`docs/design/impl/queue-parity/`).
- **The empty playlist page no longer recommends the removed armed mode.**
  Its words now name the transfer gesture — a row's `+`, or the record
  page's `Add to…`, then the list in the picker.
- **The rail panels' scrollbar no longer covers the duration column.** iced
  draws a `scrollable`'s bar over the right edge of its contents rather than
  beside them, so a track list long enough to scroll clipped every duration by
  its last character — `1:15` read as `1:1`. Both lists now keep a lane clear
  for the bar, reserved whether or not the list currently overflows so that a
  twelfth track does not shunt eleven durations sideways, and the lane and the
  bar are one token so they cannot drift apart. Nothing else about either
  panel moved.
- `.m4a`/`.mp4` files the library listed but could not play.
- `.ogg` files the library listed but could not play (Vorbis, above).
- Format detection now probes file *content* rather than trusting the
  extension, so an Ogg Opus named `.ogg` is identified as Opus instead of
  failing with a complaint about the file, and a FLAC named `.mp3` reads
  correctly. `every_advertised_extension_decodes` asserts that every extension
  the shelf advertises actually decodes real audio, so this class of bug fails
  the build rather than reaching a listener.
- Albums no longer shatter into one shelf entry per credited artist (album
  artist grouping, above).
- Duplicate interleaved track lists for an album held in two formats (editions,
  above).
- The device ring buffer is discarded when a playback session is abandoned,
  instead of leaking the previous session's audio into the next.
- `cargo test --workspace --all-features` no longer plays tones out of the
  developer's speakers. The device-gated tests still open, feed, reopen and
  tear down the real output — they write silence, which every assertion they
  make is indifferent to. The tests that can only be judged by ear (a full
  engine session through real hardware) are now opt-in behind
  `BAZ_DEVICE_TESTS=1`; see `docs/DEVELOPMENT.md`.
- **Windows: opening the audio output a second time in one process no longer
  crashes it** (`STATUS_ACCESS_VIOLATION`). cpal's WASAPI backend caches a
  process-global device enumerator inside the COM apartment of whichever thread
  touched it first, but initialises COM per-thread and calls `CoUninitialize()`
  from a thread-local destructor — so the first thread to exit tore the
  apartment down underneath the still-published global, and the next device
  open dereferenced freed state. baz opens its device on the engine thread
  (cpal streams are not `Send`), so anything that spawns a second engine — an
  output-mode change, a retry after a device error, a front end restarting
  playback — was affected. baz now makes the process's first cpal call from a
  dedicated thread that never exits; see `playback::device`'s "Why cpal is
  first touched from a thread that never exits". This is what had the Windows
  CI job dying part-way through the `baz-core` integration suite.
- The seek bar (now the needle) and the volume fader no longer stay stuck to the pointer after
  a drag that ends outside the window. If the pointer leaves the window, or
  the window loses focus, mid-drag, the gesture now ends there and commits at
  the last position it saw — the release that would normally end it is being
  delivered to somebody else, and neither iced 0.13 nor `winit` offers a
  pointer grab a widget could hold instead.

### Known limitations

- Seeking within Ogg Vorbis loses one lapped block — measured at 1024 frames,
  23.2 ms at 44.1 kHz — because symphonia's Vorbis decoder returns an empty
  buffer for the first packet after a reset. Pinned by a test and backlogged;
  fixing it means changing a seek path five formats share.
- Deleting an entire album folder leaves its rows in the index (see Removal,
  above).
- **baz does not write ReplayGain into music files.** It reads the tags a file
  has and computes what it lacks (ADR-0015), but the figures it computes live
  in baz's own index — another player will not see them.
- **ReplayGain's clipping check uses a *sample* peak** — the declared one where
  a file has it, baz's own measurement where it does not — which is what
  ReplayGain 2.0 scanners write. Inter-sample (true-peak) overshoot after
  reconstruction is not modelled, and there is no limiter: if the gain has to
  be cut, the whole track's gain is cut rather than ridden.
- **No UI for the analysis pass yet.** It is reachable through
  `AnalysisCommand` and reports progress on the event stream; the control is a
  parallel unit, exactly as the volume slider and the ReplayGain mode selector
  were before them.
- **The drag commits at the window's edge on X11.** iced 0.13 has no pointer
  capture, so a reorder drag that crosses the window edge ends and commits
  there (the groove's own documented price); on Wayland the compositor holds
  the implicit grab and the drag survives the crossing. The missing-entry
  repair surface (`Locate…`, ADR-0024 §3) is designed and not yet built.
- No cue sheets, no watch folders, no tag editing, no application
  icon, and no exclusive-mode output (which is also what puts hardware volume
  out of reach). `docs/BACKLOG.md` is the honest list.

[Unreleased]: https://github.com/mattcree/baz/commits/main
