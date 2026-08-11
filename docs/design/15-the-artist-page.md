# 15 — The artist's page: what it can say for nothing, and what a first request would cost

> **The owner, 2026-08-10**, one sentence:
>
> *"ideally the by artist page could have more info, maybe just the wikipedia
> for the band or something?"*
>
> Logged in `docs/BACKLOG.md`, *What the owner asked for*, as **designing**.

**Status**: design study · proposes an amendment to
[ADR-0030](../adr/0030-the-returns-lane-and-the-home-band.md) §6 (which is
where every engagement statistic was refused, and whose line this study has
to stay on the right side of) and puts one question to
[ADR-0018](../adr/0018-play-history-ledger.md) §6 (which named
*"no totals-by-artist"* in as many words) · the network half is proposed as
[ADR-0037](../adr/0037-the-artist-page-and-the-first-request.md), where the
dependency decision is recorded as **the owner's**. Tiered proposals in §9.
Every claim about today's code carries a `file:line`, taken at `eae1b90`.

---

## 0. The finding, in one sentence

**These are two asks wearing one sentence, and they must not be built
together.**

The first — *the artist page could have more info* — is free. baz already
holds, on the listener's own disk, every fact needed to make that page worth
opening: how many records, how long they run, the years they span, the
formats they are owned in, when the first of them arrived, and which records
filed under somebody else this artist is *on*. None of that costs a crate, a
byte of network, or a line of `deny.toml`. It is §§1–4 of this study and it
is ranked first.

The second — *maybe just the wikipedia* — is **baz's first network
request**, and the precedent binds every enrichment after it. Priced
honestly it comes to fourteen new crates, one new licence on the allowlist,
a C-and-assembly TLS core in a path that parses hostile input off the wire,
a **new Flatpak permission that every user on Flathub can see**, and a
standing obligation to somebody else's servers. It is §§5–8, it is
ADR-0037, and **the dependency decision is put to the owner rather than
taken here**, because the costing is close enough that taste decides it and
taste is his.

The acceptance test for the whole feature is stated once, at the top, and
every proposal below is measured against it:

> **The page must be good with the network off.** Not *tolerable* — good.
> Offline is the normal case, not the error case.

---

## 1. What the page is today

`crates/baz/src/views/artist.rs` is 182 lines and draws three things
(`artist.rs:92-149`):

1. a header strip whose lead is the artist's name at `SIZE_EMPHASIS` 15,
   boxed to `TRANSPORT_HIT` 32 so the strip is the same height as the album
   page's breadcrumb (`artist.rs:106-112` — the reason is stated there: the
   two places are joined by one press, and a strip that changed height
   between them would make that press a jump);
2. a quiet note at the strip's right edge, `6 records · 74 tracks`
   (`artist.rs:139`, built by `counts` at `artist.rs:156-170`);
3. one `section_rule("Records")` and the wall's own tiles
   (`artist.rs:114-136`).

That is the whole page. **Between the rule and the tiles there is nothing,
and after the tiles there is nothing.** It is a filtered wall with a title
on it, and the owner is right that it is not worth visiting.

The module's own prose already anticipated this conversation
(`artist.rs:28-35`), and it is worth quoting because §5 has to argue with
half of it:

> **Deliberately not here yet**, and each for a reason rather than for want
> of room: a biography or any critic metadata (it would come off the
> network, and **nothing in baz goes to the network**); an artist image
> (same); play counts and every other engagement statistic (ADR-0030 §6
> refused those from Home and the argument does not change with the
> surface); and a flat list of every track they appear on […]

Two of those four are network. One is a rule about statistics. **The fourth
is a rule about *tracks*, not about records** — and that gap is where §4's
best proposal lives.

### 1.1 Found on the way: the tiles were not the wall's to the pixel

The version studied here claimed the records were drawn *"in **the wall's own tile**
— the same `views::shelf::tile` with the wall's own `Grid` — so the sleeve,
the caption, the playing mark and the hover options are the wall's to the
pixel."*

The **widget** was the wall's. The **geometry** was not, and it is worth
recording before anything is added above it, because a band above the wall
makes the eye compare this page's covers against the Library it just came
from.

| | width fed to `Grid::new` | where |
|---|---|---|
| the Library wall | `window − sidebar − INDEX_LANE_W 108 − WALL_SCROLLBAR_W 4` | `app.rs:5095-5101` |
| the Artist page | `body_width − 2 × HANG 40` | `artist.rs:114-117` |
| Home's rows | `body_width − 2 × HANG 40` | `home.rs:110-118` |

Resolved through `Grid::new` (`shelf.rs:351-374`) at `Density::Balanced`
(`ART_TARGET` 272, `ART_MIN` 240, `ART_MAX` 320, hang 40):

| window | lane | wall: cols × art | artist: cols × art |
|---|---|---|---|
| 1280 | open 280 | 3 × **242.67** | 3 × **253.33** |
| 1280 | rail 96 | 3 × 304.00 | 3 × 314.67 |
| 1920 | open 280 | 5 × 257.60 | 5 × 264.00 |
| 1920 | **rail 96** | **5** × 294.40 | **6** × 244.00 |
| 2560 | open 280 | 7 × 264.00 | 7 × 268.57 |

So the covers are 4–11 px wider on the artist's page at every size, and at
**1920 with the lane collapsed the artist page draws six columns where the
wall draws five** — the wall's `wanted` is 6 and its `ceiling` is 5, so it
clamps; the artist page's width is 32 px larger and does not.

**Resolved after this measurement.** The shell now resolves one
`Shelf::grid` and hands it to the Library, Home and Artist places; the artist
view no longer constructs a second grid. The table above is the before-state
that justified the change, not a claim about the current page. The regression
test `every_place_that_hangs_works_hangs_them_on_one_grid` prevents a view from
growing its own answer again.

---

## 2. The inventory: everything baz knows about an artist, offline

`records(shelf, artist)` (`artist.rs:61-67`) already hands the page every
`AlbumVm` filed under this artist. Everything below is reachable from that
vector plus the two things `Shelf` already holds. **Nothing here needs a new
schema column, a new scan, or a network byte.**

### 2.1 From the records themselves

| fact | where it lives | cost |
|---|---|---|
| record count | `records.len()` | free — already drawn (`artist.rs:157`) |
| track count | `AlbumVm::all_tracks` | free — already drawn (`artist.rs:157`) |
| **total playing time** | Σ `TrackVm::duration` (`vm.rs:308`) | one pass over the artist's tracks; the identical arithmetic `Collection::count` does at `vm.rs:551-575` |
| **earliest and latest year** | min/max `AlbumVm::year` (`vm.rs:87`) | free |
| **formats owned** | distinct `EditionVm::key` labels (`vm.rs:206-208`) | free |
| **encoding**, per record | `EditionVm::detail` — `24-bit · 96 kHz`, `320 kbps` (`vm.rs:209-213`) | free, already computed |
| bit depth / sample rate / bitrate | `vm.rs:214-221` | free |
| **genres**, verbatim | `AlbumVm::genre` (`vm.rs:88-92`) | free, with the honesty caveat in §4.4 |
| **when the first record arrived** | min `AlbumVm::first_seen_ns` (`vm.rs:93-97`) | free; `vm::details` already renders one at `vm.rs:1167` |
| size on disk | Σ `TrackVm::bytes` (`vm.rs:311-314`) | free |
| ReplayGain coverage | `vm.rs:222-229` | free |
| **records filed under someone else that this artist is on** | `TrackVm::artist` (`vm.rs:303-306`), keyed by `vm::artist_id` | **one pass over every track per rebuild** — see §4.3 |
| **an artist picture from the listener's own disk** | `album.first_track.parent().parent()`, the shape `art.rs:136-148` already uses for covers | a scanner/decoder question, not a network one — §4.5 |

### 2.2 From the ledger

Held on `Shelf` already: the `History` snapshot (`app.rs:4337-4349`) and
`lane_played`, a per-record last-played map maintained by *events* rather
than by re-reading the file (`app.rs:4515-4522`). Folded per track,
`TrackHistory` carries (`baz-core/src/history/read.rs:129-146`):

`plays` · `skips` · `first_played_unix_s` · `last_played_unix_s` ·
`last_touched_unix_s` · `listened_ms`

and `History::recency` buckets a path into `ThisEvening → Today → ThisWeek →
ThisMonth → MonthsAgo(n) → YearsAgo(n) → Never`, with `Recency::label()`
giving the string the PLAYED shelf headers already draw
(`history/read.rs:66-119`).

So *"you first played them in 2019"* is computable, exactly, today, for
nothing. **§3 is about whether it may be drawn.**

---

## 3. The line, and which side each fact falls on

### 3.1 Where the line was drawn, and by whom

ADR-0030 §6 refused *"every engagement statistic, which is not close"* from
the home band. When the collection's counts later moved onto Home as the
`COLLECTION` footer, that refusal had to be argued past, and the argument
that got them through is the sharpest sentence the project has on this
subject (`views/home.rs:71-76`):

> `COLLECTION` is not one of those, and the line is worth stating plainly:
> it describes **what you own**, not what you do with it, and **every figure
> in it would be identical if the application had never been opened**.

That last clause is a *test*, not a slogan, and it is the one this study
uses. `docs/design/impl/home-stats/README.md:126-190` runs the same test
over the three figures it cut, and one of the three is exactly the fact this
page is tempted by:

> **Records never played.** A figure about the *listener*, read out of the
> play ledger, and it changes while you sit looking at it. ADR-0030 §6
> refuses every engagement statistic; this is the one on the list that is
> easy to mistake for an inventory fact, so it is named here rather than
> quietly dropped.

### 3.2 The ledger's own ADR names this page's temptation in as many words

ADR-0018 §6 does not merely decline a fourth read surface; it declines this
one, by name:

> There are now **two**, and there was never going to be a fourth. **No
> totals-by-artist**, no listening-time-per-month, no top-N. Those would be
> *built from this data*, so the way not to build them is not to provide the
> surface that makes them easy […]

*First heard, last heard, and play counts, folded by artist* **is
totals-by-artist**. Not a cousin of it — the thing itself, spelled out in
the decision record, with a stated mechanism for staying out of it.

### 3.3 So the inventory sorts cleanly, and the sort is the design

| fact | *identical if baz had never been opened?* | side |
|---|---|---|
| records, tracks, playing time | yes | **own** |
| years spanned | yes | **own** |
| formats, encodings, ReplayGain, size | yes | **own** |
| genres | yes | **own** |
| when the first record arrived in the library | yes — it is a scan fact, and `vm::details` already draws it per record (`vm.rs:1167`) | **own** |
| records this artist appears on under another name | yes | **own** |
| first heard / last heard | **no** | do |
| play counts, listened time | **no** | do |
| records never played | **no** | do |

Eleven facts on the *own* side, three on the *do* side. **The eleven are
tier 1 and need no ADR reopened.** The three are tier 3 — §9 puts them to
the owner as one concrete string rather than as a principle, because
reversing a written decision in two ADRs is his call and not a design
agent's.

### 3.4 What this study does **not** say

It does not say the ledger must never reach this page. The repo's one rule
is that what the owner asks for goes in the app, and he has not asked for
this — he asked for *more info*. What is being avoided is an agent quietly
overturning ADR-0018 §6 as a side effect of a page redesign, which is the
failure mode `docs/WORK.md`'s preamble exists to prevent.

There is also a **shape** that would satisfy the ADR if he wants the
listening half: history as a **door**, not a figure. The returns lane
already draws history and performs nothing — it is `recency`'s data read as
an order (ADR-0030 §1) rather than as a scoreboard. The artist-page form of
that is not `Played 47 times`; it is `Last played 3 months ago` in
`Recency::label()`'s own vocabulary, one string, no count, no ranking, no
comparison to another artist. §9 tier 3 #12 is that string, drawn, so he can
say yes or no to something rather than to an argument.

---

## 4. The design, offline

### 4.1 The constraint that decides the composition

The header strip **must not grow**. `artist.rs:106-112` fixes its lead to
`TRANSPORT_HIT` 32 for a stated reason — the album page's breadcrumb is one
press away at the same height, and a height change across that press is a
jump. `place_header_led` (`views/mod.rs:356-378`) then adds
`pad(TOP_BAR_PAD_V 8, HANG 40)` and a 1 px hairline, so the strip is 48 px
plus its rule at every width, in every place. **Nothing this study proposes
touches it.**

The right edge of that strip carries one right-aligned, `Wrapping::None`
note (`views/mod.rs:365-372`). It cannot grow either: an unwrapped string at
the right edge of a 600 px body — `TOP_BAR_FLOOR`, the narrowest body baz
can draw (`theme.rs:2017`, and the const-assert at `theme.rs:1277-1285`
which uses exactly that figure as *the narrowest place body*) — clips.

So the facts go **in the body**, and they go **above the wall**, because
below sixty tiles is where facts go to die. `COLLECTION` chose the foot for
the opposite reason and said so (`views/home.rs:759-769`): Home's job is to
put you back into music, `CONTINUE` is the thing you press, and an inventory
must not push it down. **On the artist's page the inventory is not competing
with a control — it *is* the thing the page was missing**, and the wall
below it is reached by a scroll either way.

### 4.2 The band: one line, one voice, no new lattice

```
Talk Talk                                        6 records · 74 tracks   ← unchanged strip
────────────────────────────────────────────────────────────────────────
4 hours 12 minutes · 1988–1991 · FLAC, MP3 · In your library since 2019  ← the band (new)

RECORDS
────────
[  ][  ][  ]
```

- **`SIZE_META` 12 / `LINE_META` 16 in `paper_faint`** — the record page's
  catalogue line's own voice (`album.rs:567-570`), which is also the ink the
  header strip's note already takes (`views/mod.rs:365-372`), so the page
  has **one** quiet-fact voice rather than two. No new size, no new ink, no
  new token. (`paper_dim`, the `Details` table's value ink at
  `album.rs:440-476`, is one step louder and is the fallback if a frame says
  12 px faint under a hairline is too quiet to be the only type above the
  wall.)
- **One line, `·`-separated, and it reads as a sentence** — *four hours
  twelve, 1988 to 1991, on FLAC and MP3, in your library since 2019.* That
  is the test `COLLECTION` used to pick its four
  (`docs/design/impl/home-stats/README.md:151-153`) and it is the test that
  picks these.
- **Not a stat row.** `COLLECTION`'s four cells on the `STAT_W` 96 lattice
  (`theme.rs:1264-1275`, `views/home.rs:838-860`) are the right shape for a
  *footer you consult*, and the wrong shape here: four figures at
  `SIZE_EMPHASIS` above a wall of covers is a dashboard header, and the room
  does not have those. One quiet line is the room's own answer to *"say some
  facts about the subject"* and it is already used on the surface one press
  away.
- **Ellipsis, never wrap.** `Wrapping::None` and `.clip(true)`, the
  treatment `views/home.rs:855` gives the stat cells. At 600 px body the
  content lane is 600 − 2×`HANG` 40 − `SCROLLBAR_LANE` 10 = **510 px**; the
  string above measures ≈ 68 characters ≈ 428 px at `SIZE_META` in the
  bundled Plex Sans, so it fits the floor. A pathological case (six formats,
  a long date) clips at the right rather than reflowing the page.
- **Each term is absent, not empty** — ADR-0030 §6's rule, kept verbatim by
  Home (`views/home.rs:76-83`). No years on the records? No year term, not
  `— · —`. One record, one format, no `first_seen_ns` (which is permanently
  `None` for rows predating schema v7, `vm.rs:93-97`)? Then the line is
  `41 minutes · FLAC` and that is a true sentence. **An artist with none of
  the five terms gets no band at all**, and the page is exactly today's
  page.

#### The geometry, measured

At 1280 with the lane open, `Density::Balanced`:

```
 0        280                                                        1280
 ├─ lane ──┤                                                           │
 │         │                                                           │
 │         │  Talk Talk                       6 records · 74 tracks    │  y=8   strip
 │         │                                                           │  y=40
 │  RECENT ├───────────────────────── hairline ────────────────────────┤  y=48
 │         │                                                           │
 │         │◄─40─►                                                     │  y=49  place_pad top = HANG 40
 │         │      4 hours 12 min · 1988–1991 · FLAC, MP3 · since 2019  │  y=89  ★ 16 px, SIZE_META
 │         │                                                           │  y=105
 │         │      ─────────────────── hairline ───────────────────     │  y=129 GAP_XL 24
 │         │      RECORDS                                              │  y=138 SIZE_HEADING 10
 │         │                                                           │  y=150
 │         │      ┌──────────┐ ┌──────────┐ ┌──────────┐               │  y=166 GAP_LG 16
 │         │      │  253.33  │ │  253.33  │ │  253.33  │               │
 │         │      └──────────┘ └──────────┘ └──────────┘               │
 └─────────┴──── x=320 ──── x=613 ──── x=907 ──────────────── x=1230 ──┘
                 │◄─ 253.33 ─►│◄ 40 ►│                        gutter 40
```

★ is the whole of the addition. **Cost to the wall: 16 px of type +
`GAP_XL` 24 = 40 px**, one row of tiles pushed down by 40 of the ~293 px
that row occupies. Everything else on the page is today's geometry to the
pixel.

Restated as the rule: *the band is one `LINE_META` and one `GAP_XL`,
forever.* A second line would be a paragraph and the page would have a
preamble.

### 4.3 `ALSO ON` — the fact that makes the page worth visiting

The best local fact on this page is not a figure. It is: **which records
filed under somebody else does this artist appear on?**

`AlbumVm::track_artists_vary` exists precisely because per-track artists are
real and are lost by the album grouping (`vm.rs:75-85` — *"a soundtrack
filed under one label with a different composer per cue, or a compilation.
[…] Marta's per-composer credits are the reason this exists"*). Every
`TrackVm` keeps its own artist verbatim even when it equals the album's
(`vm.rs:303-306`), so the fold is available and exact.

Drawn as a second `section_rule` under the wall, in the wall's own tiles:

```
RECORDS
────────
[  ][  ][  ]
[  ][  ][  ]

ALSO ON
────────
[  ][  ]
```

**This is not the thing `artist.rs:32-35` refused.** That refusal is
specific and correct: *"a flat list of every track they appear on […] would
be ADR-0017 §1.7's 'albums listed as albums, never flattened' broken on a
page whose whole subject is records."* `ALSO ON` lists **records**, in the
tile, as records — it is that rule *kept*, not broken. The listener presses
one and lands on the album page, where `track_artists_vary` already draws
the per-track artist column that explains why the tile was there.

**What it costs, honestly.** The fold is `vm::artist_id(track.artist)` over
every track in the library — a walk, and ADR-0030 §4's responsiveness
contract forbids paying a walk per frame. It goes exactly where
`vm::Collection` went: built in `Shelf::rebuild_shelves` and held on the
shelf (`app.rs:4533-4541` states this pattern and its reason), one pass per
rebuild, zero per frame. The held value is a `HashMap<u64, Vec<usize>>` —
artist id → indices into `Shelf::albums` — which is bounded by the number of
*guest credits*, not by the library, and is empty for the ordinary
well-tagged collection.

**Absent, not empty**: an artist who guests on nothing gets no `ALSO ON`
rule. Most will.

**One exclusion, stated**: a record already in `RECORDS` never appears in
`ALSO ON`. One fact drawn twice is doc 07 L8.6's test, and the same record
under two headings on one page is the most visible way to fail it.

### 4.4 Genres — proposed, with the caveat printed

`AlbumVm::genre` is *verbatim from the tags* and the view model says so
loudly (`vm.rs:88-92`): a library carrying `Post-Rock`, `post rock` and
`Rock; Instrumental` has **three** genres and baz shows three, because it
*has* three genre tags. On a record's page that is fine — it is one string
about one record, and the `Details` table's job is to report what the file
says (`vm.rs:1144`).

Folded over an artist, three spellings of one genre become a list that looks
like sloppiness on baz's part rather than on the tagger's. So genres are
**tier 2, not tier 1**, and if adopted they take the same case-folded
de-duplication `Collection::count` uses for artists (`vm.rs:551-560`), with
the *first spelling seen* kept — ADR-0019 §4's rule, which is available here
because the fold walks the artist's records in shelf order, unlike
`artist::label`'s problem at `artist.rs:70-83`.

Cap at three, `+2 more` never — a truncated list with no way to see the rest
is a control that isn't one. Three, then stop.

### 4.5 An artist picture, off the listener's own disk

`artist.rs:30` refuses an artist image on the grounds that it *"would come
off the network."* That is true of the *usual* artist image and false of the
one already sitting in most collections:

```
/music/Talk Talk/           ← artist.jpg, folder.jpg, or artist.png here
/music/Talk Talk/Laughing Stock/   ← cover.jpg here; art.rs:136 already finds it
```

`art.rs:136-148` is `COVER_FILE_NAMES` matched case-insensitively in
`first_track.parent()`. The artist-level equivalent is
`first_track.parent()?.parent()` and a four-name list, reusing `cover_file`
almost verbatim, decoded through the same downscale-only path
(`art.rs:213-240`) into the same `lru` thumbnail cache.

This is the cheapest thing in this study that makes the page *feel* like a
page rather than a filtered wall, and it costs **no crate, no network, and
no new decoder**. It is tier 2 rather than tier 1 only because it needs a
composition decision — where the picture sits, and what the page does at the
overwhelming majority of libraries that have no such file (answer: absent,
not empty; the band and the wall are the page, exactly as in §4.2).

**One honesty note it must carry**: this is *the listener's own file*, not a
picture baz went and got. If it is wrong, it is wrong on their disk, and the
fix is a file manager. That property is the entire reason it is admissible
where §5's is a decision.

### 4.6 Refused from the offline page, each for a reason

- **A timeline, a bar chart, a decade histogram of the release years.** The
  data supports it and the room does not have charts. ADR-0030 §6's posture
  is *history records, it does not perform*, and a chart is performance
  whatever it is made of. `1988–1991` is the fact; the shape of the
  distribution is a graphic.
- **`Play everything by this artist`.** A control, not information, and the
  strip's `Play all` plus the wall's own hover options already reach every
  record on the page. If he wants it, it is one message and it belongs in
  the playlist epic, not in a study about what a page can *say*.
- **A second arrangement control for the records** — `artist.rs:54-60`
  already argues this out: the wall's order is the order the listener asked
  for, and a chronological toggle here would be a second answer to *"in what
  order do this artist's records go"* with nothing on screen to explain it.
- **Size on disk.** True, cheap, dull, and already cut once from
  `COLLECTION` for exactly that (`views/home.rs:794-797`): *"a fact about a
  filesystem, and nothing you would do differently having read it."* The
  record page's `Details` block is where bytes belong.
- **A `Details`-style table.** The record page's condition report is a
  reference table you scan beside the object (`album.rs:424-476`). An artist
  is not an object with a condition; a table of eleven right-aligned labels
  above a wall of covers would be a form.

---

## 5. Prior art: three products, three sources, three attributions

### 5.1 Roon — a paid provider, plus Wikipedia, and it costs an account

Roon's own metadata page states it plainly: *"Roon now supports multiple
sources of album reviews and performer biographies, including Wikipedia
articles in 20 languages"* ([roon.app/en/music/data]). Underneath that,
Roon's editorial metadata is a **licensed commercial feed** — the
TiVo/Rovi/AllMusic database, which TiVo sells as a product and which also
supplies AllMusic, Apple Music and Spotify ([TiVo music metadata]). Roon's
own help pages describe biographies and reviews as coming from *"Roon"* as a
source, without naming the upstream ([Roon metadata model]).

**What baz should take from it:** Roon's answer to *"where does the bio come
from"* is *"from us, and you are paying us"* — a subscription, an account,
and a cloud service that every library is identified to. That is precisely
the model VISION.md's third pillar exists to refuse, and it is the reason
Roon's approach is not portable to baz however good the result looks.

**What baz should take from it anyway:** Roon puts the biography on a page
that is *already complete without it*. The artist page in Roon is a
discography first.

### 5.2 Plexamp / Plex — a server-side agent, and the fragility of a middleman

Plex's music metadata has moved from a **Last.fm agent** to Plex's own
*"Plex Music"* provider ([Plex metadata agents]); the Last.fm agent is
deprecated, and users report Last.fm-derived text still arriving through the
new provider ([Plex forum, music metadata sources]). Plexamp is a client of
the server's cache, so the request is made once, by the server, on scan —
**not by the phone in your hand when you open a page.**

**What baz should take from it:** two things, one good and one bad. The good
one is the architecture: **fetch once, cache, serve from the cache**, so the
listener's browsing is not a stream of requests. The bad one is the failure
mode: a product whose bios come through a middleman gets its bios changed,
or removed, by the middleman. Plex's own users experienced exactly that when
the Last.fm agent was retired. baz has no server to put in the middle, which
means the *cache is on the listener's disk* — better for privacy and better
for durability, and it is what §7.4 proposes.

### 5.3 MusicBee — the closest model, and the one baz is actually near

MusicBee's Music Explorer node shows artist biographies, discographies and
similar artists, historically sourced from Last.fm ([MusicBee wiki,
Navigator]), and — the detail that matters — **MusicBee stopped using
Last.fm as a source for tags and artwork after Last.fm's API changed**,
keeping only scrobbling ([MusicBee wiki, Last.fm]).

**What baz should take from it:** MusicBee is a local-first Windows player
with an optional network panel, which is structurally the same product baz
is. And its history is the strongest available argument for §7.6's
**absent, not broken** rule: a third-party text source *will* change its
terms, and the design has to survive that as *"the block is not there"*
rather than as *"the page is broken"*.

### 5.4 The one nobody does well, and baz can

None of the three lets you **correct a wrong match**. Roon has an identity
editor buried in album/artist settings; Plex's is a server admin function;
MusicBee's panel simply shows whatever the name matched. Every one of them
resolves an artist name by a heuristic and then argues with you about it.
§7.3 proposes the opposite, and it is the part of this design that is
actually novel: **a wrong match is corrected by the listener, in one press,
and the correction is remembered.**

---

## 6. What a network request costs baz, measured

### 6.1 The standard it has to clear

- **Zero network code today.** `grep -rn "std::net\|TcpStream\|UdpSocket\|
  reqwest\|ureq" crates/*/src/` returns **0 lines** at `eae1b90`. There is
  no HTTP client, no socket, and no URL in the product.
- **Zero system dependencies, defended repeatedly and in writing.**
  `Cargo.toml:33-36` bundles SQLite from source *"so no system library or
  -dev package is needed on any platform"*; `Cargo.toml:41-62` records that
  *"the whole decode path is pure Rust, and staying that way is a
  build-system property worth keeping"*; `Cargo.toml:81-88` takes zbus
  because it is *"pure Rust — no C, no system library, no pkg-config — so
  the Linux build stays system-dep-free"*; `Cargo.toml:89-95` takes `rfd`
  with *"**no** gtk-sys and no async-runtime coupling"*.
- **The precedent that decides the standard.** `docs/BACKLOG.md:271-330`
  refuses Opus, and the operative sentence is
  (`docs/BACKLOG.md:293-302`):

  > The cost is a **C library and a `cmake` build dependency on every
  > platform** […] baz's decode path is pure Rust with **zero system
  > dependencies** today (even SQLite is `bundled`); spending that property
  > on one lossy format is not a trade worth making unprompted.

  Note *unprompted*. This one is prompted. That changes who decides; it does
  not change the arithmetic.
- **`deny.toml`** allows 12 licences and calls extending the list *"a
  reviewed decision"* (`deny.toml:45-75`), denies `openssl-sys` outright
  with *"prefer rustls"* (`deny.toml:84`), and denies unknown registries and
  git sources (`deny.toml:87-89`).

### 6.2 The measured cost, three ways

Measured on 2026-08-10 by resolving each candidate in a scratch crate
outside this repo and intersecting its `cargo tree -e normal --target
x86_64-unknown-linux-gnu` against baz's own `Cargo.lock` (558 package
entries, 497 distinct names).

| | `ureq` 3.4.0 | `reqwest` 0.13.4 | no crate at all |
|---|---|---|---|
| features | default (`rustls` + `ring` + `webpki-roots` + `gzip`) | `--no-default-features --features rustls,blocking` | — |
| crates in the normal graph | **27** | **82** | 0 |
| **already in baz's lock** | 13 | 25 | — |
| **net new crates** | **14** | **57** | **0** |
| new build-dependencies | `cc`, `find-msvc-tools`, `shlex` — **all three already in the lock** (`Cargo.lock:561` for `cc`) | `cmake`, `bindgen` (libclang), NASM on Windows x86/x64 | none |
| a `-sys`-shaped crate? | **yes, one**: `ring` declares `links = "ring_core_0_17_14_"` and compiles C + per-arch assembly from `build.rs` | **yes, worse**: `aws-lc-sys` declares `links = "aws_lc_0_44_0"` and a `builder/main.rs` | no |
| a `build.rs` that downloads? | **no** — every byte is vendored in the crate | **no** — same | — |
| new licences for `deny.toml` | **one**: `CDLA-Permissive-2.0` (`webpki-roots`) | at least two: `CDLA-Permissive-2.0` and `MIT-0` (inside `aws-lc-sys`'s seven-way `AND` string) | none |
| blocking or async | **blocking** — fits `tokio` being present *"for the iced shell only"* (`Cargo.toml:63`) | async, and pulls `hyper`, `tower` ×4, `mio`, `socket2` and the whole ICU4X chain (`icu_*`, `zerovec`, `yoke`, `tinystr`, `writeable`, `litemap`) for `idna` | — |

The 14 net-new crates for `ureq`, in full:

`base64` · `http` · `httparse` · `ring` · `rustls` · `rustls-pki-types` ·
`rustls-webpki` · `subtle` · `untrusted` · `ureq` · `ureq-proto` ·
`utf8-zero` · `webpki-roots` · `zeroize`

Their licences, read from the crate manifests in the registry cache:

| crate | licence | on `deny.toml`'s allowlist? |
|---|---|---|
| `ring` | `Apache-2.0 AND ISC` | yes, both |
| `rustls`, `rustls-webpki`, `untrusted` | `ISC` | yes |
| `rustls-pki-types`, `ureq`, `ureq-proto`, `httparse`, `base64`, `utf8-zero`, `zeroize` | `MIT OR Apache-2.0` | yes |
| `http` | `MIT` | yes |
| `subtle` | `BSD-3-Clause` | yes |
| **`webpki-roots`** | **`CDLA-Permissive-2.0`** | **no — one new line** |

### 6.3 The three findings that actually matter

**1. `reqwest` is the Opus refusal, verbatim.** `aws-lc-sys` 0.44.0's
manifest declares `build = "builder/main.rs"`, `links = "aws_lc_0_44_0"`,
and build-dependencies on **`cmake`** and **`bindgen`**. That is *"a C
library and a `cmake` build dependency on every platform"*, word for word,
plus a libclang requirement `libsqlite3-sys` never imposed and NASM on two
Windows targets. Fifty-seven new crates to fetch one paragraph of text.
**`reqwest` is not a close call and this study does not present it as one.**

**2. `ureq`'s TLS is not pure Rust, and the doc comment that says so must be
written honestly.** `ring` 0.17.14 has `build = "build.rs"`, a
`build-dependencies.cc`, and `links = "ring_core_0_17_14_"`. It compiles C
and per-architecture assembly. Two mitigations and one aggravation, all
three of which belong in the ADR:

- *Mitigation.* A C compiler is **already required** to build baz —
  `rusqlite` is `bundled` (`Cargo.toml:36`), so `libsqlite3-sys` compiles
  SQLite from source through `cc` on every platform today, and `cc`,
  `shlex` and `find-msvc-tools` are all already in `Cargo.lock`. So this is
  **not a new build requirement**; it is a new *C surface in the binary*.
- *Mitigation.* Nothing downloads at build time, so Flathub's offline build
  and `deny.toml`'s `unknown-git = "deny"` are both unaffected.
- *Aggravation.* It is the first C in baz that parses **hostile input off
  the wire**. `libsqlite3-sys` parses a file baz wrote itself; `symphonia`
  parses hostile input but is pure Rust and fuzzed
  (`docs/ENGINEERING.md`'s fuzzing policy). A TLS record parser in C is a
  different class of exposure, and `ENGINEERING.md`'s *"prefer proven
  crates"* is the only thing that argues for it — `ring` is, by adoption,
  about as proven as a Rust crate gets, and 0.17.14 is past
  RUSTSEC-2025-0009. It is still C.

  The pure-Rust alternative (`rustls` with a RustCrypto provider) exists and
  is not proven — the same judgement `docs/BACKLOG.md:303-310` made about
  the young Opus decoders, and it points the same way.

**3. The Flatpak cost is a *visible permission*, not a build detail.**
`packaging/flatpak/io.github.mattcree.baz.yml:26-50` lists the sandbox's
`finish-args` in full, and **there is no `--share=network`**. The shipped
Flatpak literally cannot open a socket. Adding the biography means adding
that line, which means **"Network access" appears in the app's permission
list on Flathub and in every software centre that shows it** — for an
offline-first music player whose pitch is that your listening is nobody's
business. That is the single most expensive line item in this study and it
is not a technical one.

The mechanical Flatpak cost is trivial by comparison:
`packaging/flatpak/cargo-sources.json` is a generated list of **556 vendored
archives**; `ureq` makes it 570. `ring`'s C and assembly compile inside the
SDK sandbox with the toolchain that is already there. `reqwest` would
additionally need `cmake` and libclang in the build environment.

### 6.4 The option nobody costed, which costs nothing

**Hand the URL to the desktop and let the listener's own browser fetch it.**

- On Linux: `org.freedesktop.portal.OpenURI` over D-Bus. baz already speaks
  D-Bus with `zbus::blocking::Connection`
  (`crates/baz/src/mpris/server.rs:33`), and zbus is a **direct dependency
  already** (`Cargo.toml:81-88`). **Zero new crates.**
- On Windows and macOS: one `std::process::Command`.
- **No `--share=network`** — the portal opens the link on the host, outside
  the sandbox, which is exactly what the portal is for. The Flatpak's
  permission list does not change.
- No TLS, no cache, no HTML parsing, no rate limit, no User-Agent
  obligation, no CC BY-SA obligation (baz displays no Wikipedia text), no
  offline failure mode, and no layout that can jump.
- The privacy exposure is the one the listener chose, in their own browser,
  with their own blockers and their own session — and they chose it by
  pressing a control that says where it goes.

What it does *not* do is put the paragraph on the page, which is what the
owner asked for. That is the honest limitation and §9 states it as such:
this is proposed as **tier 1, shipping now**, and explicitly **not** as a
substitute for the decision in §7. It is what the page has while the
decision is open, and it may turn out to be all he wanted.

---

## 7. If the request is made: how, exactly

Everything in this section is conditional on §9 tier 4 and ADR-0037. It is
specified rather than sketched so that the decision is made against a real
design and not against a vague one.

### 7.1 The chain, hop by hop

An artist name is not an identifier. Three hops, in order, each with a cost
and a failure:

| # | request | what it gets | on failure |
|---|---|---|---|
| 1 | `GET musicbrainz.org/ws/2/artist/?query=artist:<name>&fmt=json&limit=5` | up to 5 candidate MBIDs with `score`, `disambiguation`, `type`, `country`, `life-span` | no block, and the page is §4's page |
| 2 | `GET wikidata.org/wiki/Special:EntityData/<QID>.json`, reached from the MusicBrainz artist's `wikidata` URL relation (`inc=url-rels` on hop 1, folding hops 1 and 2's lookup into one) | `sitelinks.enwiki.title` | no block |
| 3 | `GET <lang>.wikipedia.org/api/rest_v1/page/summary/<title>` | `extract` (the lead paragraph), `description`, `content_urls.desktop.page`, `thumbnail`, `wikibase_item` | no block |

Verified against the live services on 2026-08-10:

- Hop 3 returns exactly those keys for `Talk Talk` — `extract`,
  `extract_html`, `description` (*"English post-rock and former synth-pop
  group (1981–1991)"*), `content_urls`, `thumbnail`, `originalimage`,
  `wikibase_item: Q595705`, `pageid`, `revision`, `timestamp`, `lang` —
  and, importantly, **no licensing field at all**. §7.5's attribution is
  entirely the client's obligation; nothing in the payload reminds you.
- Hop 2 on `Q595705` carries `P434` (MusicBrainz artist ID) =
  `a74f43e4-50c4-4b19-a2ce-c05ce9bccb03` and `sitelinks.enwiki.title` =
  `"Talk Talk"`. **The entity document is ~150 kB** for one three-line
  answer, which is the single ugliest number in this design: 150 kB of JSON,
  38 language sitelinks and years of YouTube subscriber counts, to learn one
  string. `Special:EntityData/<QID>.json?props=sitelinks` trims it and must
  be used.
- Hop 1 **returned HTTP 503 on the first attempt**, from a client with a
  non-descriptive User-Agent. That is the documented behaviour, not bad
  luck: MusicBrainz throttles by User-Agent and by source IP, allows *"(on
  average) 1 request per second"* per IP, and declines the rest with 503,
  with anonymous agents throttled hardest. **The User-Agent obligation in
  §7.5 is enforced, not advisory, and this study has the receipt.**

### 7.2 Fewer hops, and why they are refused

- **Name straight to Wikipedia** (skip 1 and 2) is one request instead of
  three and is wrong for the reason the whole chain exists: *Nadir* the
  band, *Nadir* the album, *Nadir* the point below your feet. Wikipedia's
  own search would happily return the astronomical one, and the page would
  state it confidently.
- **Name straight to Wikidata** by `haswbstatement` has the same ambiguity
  and worse ranking.
- **MusicBrainz's `wikipedia` URL relation** (skipping hop 2) exists but is
  deprecated upstream in favour of the Wikidata relation, and it is the
  relation that goes stale when an article is renamed. Wikidata is the
  indirection that survives a rename; that is what it is for.

### 7.3 Two bands called Nadir — the disambiguation, and who resolves it

**The heuristic never decides. The listener does, once, and it is
remembered.**

- **Exactly one candidate above a strong score, and its `type` is `Group` or
  `Person`** → resolve, cache, draw. No prompt, no badge.
- **Two or more plausible candidates** → **the block is absent**, and one
  quiet line stands where it would have been:

  ```
  ABOUT
  ─────
  Two artists match “Nadir”.  Choose which                    ← a word-button
  ```

  Pressing it opens the picker at the pointer — ADR-0031's mechanism, which
  already exists — listing each candidate as MusicBrainz describes it:
  `Nadir — Hungarian thrash metal band, 1988–` /
  `Nadir — French electronic project`. The `disambiguation` and `life-span`
  fields exist for exactly this and are what a human needs to answer.
- **A wrong match already showing** → the `ABOUT` block's attribution line
  (§7.5) is itself the door: pressing the source name reopens the same
  picker. **No settings page, no re-scan, no "refresh metadata" verb.** This
  is the thing §5.4 says none of the three prior-art products does, and it
  is a stronger reason to build this in-app than the paragraph is.
- **The choice is stored as an MBID against the artist's `artist_id` hash**,
  in the cache of §7.4. It survives a rescan, a rename and a reinstall of
  the library DB, because it is not in the library DB.
- **`Not this artist / none of these`** is one of the picker's rows, and
  choosing it is remembered too. An artist the listener has said no about is
  never asked about again and never requested again.

### 7.4 The cache: a second visit asks nobody

`$XDG_DATA_HOME/baz/artists/<mbid>.json`, beside `library.db` and
`history.tsv` — ADR-0018 §1's argument for where baz's own records live,
applied unchanged.

- **Plain files in an open format**, one per artist, the listener's to read,
  back up, or delete. Deleting one is the whole of "refresh"; deleting the
  directory is the whole of "forget everything I looked up". No verb in the
  interface has to exist for either.
- **Not in `library.db`.** ADR-0018 §3's reasoning transfers exactly: the DB
  is a cache that a rescan may rebuild, and this is a record of *the
  listener's choices* (which artist is which) plus somebody else's text.
- **Each file carries what it needs to be honest about itself**: the MBID,
  the Wikidata QID, the article title and language, the `extract`, the
  article URL, the revision id and timestamp hop 3 returned, and the instant
  baz fetched it. The revision id is what makes the attribution in §7.5 a
  true statement rather than an approximate one.
- **No expiry, no background refresh, no watcher.** A cached paragraph never
  goes stale in a way that hurts anybody, and a player that phones home on a
  timer is the thing this whole design is avoiding. If the listener wants it
  again, they delete the file.
- **Negative results are cached too** — "no match", "listener said none of
  these", "the service said 503". Otherwise every visit to an unmatched
  artist is a fresh pair of requests, which is the rate-limit violation
  §7.5 promises not to commit.

### 7.5 The obligations, precisely

**MusicBrainz.** A descriptive `User-Agent` with contact information is
required, in the documented form
`Application name/<version> ( contact-url )`. baz's is:

```
baz/0.1.0 ( https://github.com/mattcree/baz )
```

Rate limit: **one request per second per source IP**, enforced with 503.
baz's shape makes this easy to honour and the honouring must be structural,
not aspirational: requests are made **only when a person opens an artist
page**, at most one chain per page, never for an artist already in the cache
(including negatively), and the client serialises through a single worker
with a ≥ 1 s floor between MusicBrainz requests. A human cannot open pages
fast enough to breach it; a loop could, and there is no loop.

**Wikimedia.** The User-Agent policy requires a descriptive agent with
contact information — `<client>/<version> (<contact>) <library>/<version>`
— and says non-compliant clients *"may be blocked without notice"*. baz's:

```
baz/0.1.0 ( https://github.com/mattcree/baz ) ureq/3.4
```

**The licence, and exactly what must be on screen.** Wikipedia text is
**CC BY-SA 4.0**. Attribution is satisfied by *"a hyperlink (where possible)
or URL to the page or pages you are re-using"*, and a licensing notice
stating the work is released under CC BY-SA must be included. So the block
carries, always, unconditionally, and never behind a hover or a menu:

```
ABOUT
─────
Talk Talk were an English post-rock and new wave band formed in 1981 by
Mark Hollis, Lee Harris, Paul Webb and Simon Brenner. Initially a synth-pop
group, …

From Wikipedia · CC BY-SA 4.0                              ← both are doors
```

- **`From Wikipedia`** links to `content_urls.desktop.page` — the article
  itself, which is the credit CC BY-SA asks for, and which doubles as
  §7.3's correction door.
- **`CC BY-SA 4.0`** links to the licence deed. The notice is required; a
  bare link to the article is not sufficient on its own.
- **The extract is reproduced verbatim and is never edited, truncated with
  an ellipsis, summarised, or merged with any other text.** The moment baz
  modifies it, share-alike attaches to the modification and baz must mark it
  as modified. Not modifying it is free; the alternative is not. This is a
  hard rule on the implementation, not a preference.
- **Share-alike does not reach baz's own source.** baz is GPL-3.0-or-later
  (`Cargo.toml:10`); the Wikipedia extract is a separately-licensed work
  *displayed* by the program and cached beside it, not incorporated into
  it — the same relationship the program has to the listener's FLAC files.
  The cache file must therefore carry the licence and the article URL inside
  it (§7.4), so that a copy of the cache travelling on its own travels with
  its licence.
- **`deny.toml` is untouched by any of this** — CC BY-SA governs *data*, not
  a crate, and `deny.toml` walks the Cargo graph. This is the same
  distinction the file already draws for the bundled IBM Plex typeface
  (`deny.toml:49-56`): *"a checked-in asset is not a crate, so it is outside
  this file's remit entirely."* The obligation is met on screen and in the
  cache file, and it is argued here.

### 7.6 Privacy, stated plainly rather than buried

**A request to MusicBrainz or Wikipedia tells someone else's server what you
are listening to.** Not in so many words — it tells them an artist name, an
IP address and a timestamp — but an artist name and a timestamp *is* what
you are listening to, and baz's entire pitch is that this is nobody's
business. VISION.md's third pillar is *sovereignty by default — offline-first,
no account, no telemetry*, and ADR-0018 §7 is a page about there being *"no
identifier, no machine ID, no session key, no hash of anything"* in the one
file that could carry one.

So, without hedging:

1. **Off by default.** Not "on with a first-run prompt". Off, with nothing
   asked, because a prompt at first run is a dark pattern wearing consent.
2. **One switch, in Settings.** `views/settings.rs:148` has two sections,
   `Playback` and `Library`; this needs a third, because it is neither.
   The section, in the file's own established shape — a name, then one
   present-tense sentence about what it *does* (`views/settings.rs:698-724`):

   > ### Artist information
   >
   > **When you open an artist's page, baz asks MusicBrainz and Wikipedia
   > who they are. Nothing else on your machine is sent, nothing is sent
   > while music plays, and each artist is asked about once.**

   The switch's own label: **`Look artists up online`**. Off.

   The section also states, under the switch, where the answers are kept and
   how to remove them — the path in §7.4, in the readout ink, because a
   promise about local storage that does not say *where* is not a promise.
3. **Never triggered by playback.** Not on `TrackStarted`, not on scan, not
   on a queue change, not by the MPRIS metadata refresh, not by the lane
   rebuilding. **Only by a person opening an artist's page.** This is the
   most important sentence in this section: it is what makes the request
   log on somebody else's server a record of *what you looked up*, which you
   chose, rather than *what you played*, which you did not.
4. **Once per artist, ever**, because of the cache — including the negative
   cache. A second visit asks nobody. A hundredth visit asks nobody.
5. **No identifier of any kind.** No install id, no session, no cookie jar
   (the client is configured without cookies), no `Referer`, no
   `Accept-Language` derived from the listener's locale beyond the article
   language they are already asking for. The User-Agent identifies **baz**,
   which the two services require, and nothing about the person running it.
6. **Turning it off stops requests immediately and keeps what is cached.**
   Two separate acts: the switch stops asking; deleting the directory
   forgets. Conflating them would mean the switch silently destroyed data.

### 7.7 Offline is the normal case: no spinner, no empty box, no jump

The product refuses spinners, and this design does not need one, because the
composition removes the need rather than hiding it:

- **The block is the last thing on the page.** `RECORDS`, then `ALSO ON`,
  then `ABOUT`. Everything the listener came for is above it and **nothing
  above it can move when a fetch lands** — a late arrival grows the
  scrollable's content and shifts nothing already drawn. That is the whole
  of the no-jump requirement, solved by ordering rather than by reserving
  space.
- **Absent, not empty** — the rule ADR-0030 §6 set and Home keeps
  (`views/home.rs:76-83`). No box, no placeholder, no skeleton, no
  `Loading…`, no greyed rectangle. With the setting off, with no network,
  with no match, with a 503: **there is no `ABOUT` rule on the page**, and
  the page ends where §4's page ends. A listener who never turns the setting
  on never sees a trace of this feature, including its absence.
- **The one exception is the ambiguity line** (§7.3), which is drawn only
  when baz has *more* information than it can act on — two named
  candidates — and is a sentence with a door rather than a state of waiting.
- **No optimistic drawing.** Nothing in baz marks a row optimistically
  (`views/album.rs:55-56`), and the same rule applies here: the block
  appears when the text is in the cache, never before.
- **Nothing is fetched for a page the listener leaves.** The chain is
  abandoned on navigation; a response that arrives for a page nobody is on
  is written to the cache and drawn on the next visit.

---

## 8. Prior art's verdict, applied

| | source | attribution shown | cost to the listener |
|---|---|---|---|
| Roon | TiVo/Rovi (AllMusic) + Wikipedia in 20 languages | *"Roon"* as the source in its metadata model; Wikipedia named on the marketing page | a subscription and an account; every library identified to a cloud service |
| Plexamp / Plex | Plex Music (formerly the Last.fm agent) | provider named in library settings | a server, and a middleman that can change or withdraw the text |
| MusicBee | Last.fm (retired for tags/artwork after an API change) | panel names its source | none, but the panel broke when Last.fm's terms changed |
| **baz, as proposed** | **MusicBrainz → Wikidata → Wikipedia, direct** | **`From Wikipedia · CC BY-SA 4.0` under every extract, both doors, always** | **one visible Flatpak permission, one setting, off by default** |

The row that matters is the last column. Roon and Plex both solve this by
putting a company between you and the encyclopaedia; baz cannot and should
not. Going direct is more honest, and it is also *more expensive in the one
currency baz has been saving in* — the dependency graph — which is exactly
why the decision is the owner's.

---

## 9. Proposals, ordered by relief

### Tier 1 — adopt

| # | proposal | why it is tier 1 |
|---|---|---|
| **1** | **The facts band** (§4.2): one `SIZE_META` line under the header — playing time · years · formats · in-library-since. `LINE_META` 16 + `GAP_XL` 24 = **40 px**, one voice, no new token, absent term by absent term. | Every figure passes `views/home.rs:71-76`'s test unchanged. No ADR is reopened. It is the whole of *"more info"* for zero cost, and it is the acceptance test's page. |
| **2** | **`ALSO ON`** (§4.3): a second `section_rule` under the wall, the wall's own tiles, records this artist appears on that are filed under someone else. Absent when empty, which is most artists. Never repeats a record already in `RECORDS`. | The only genuinely *new* information on the page, and it keeps ADR-0017 §1.7 rather than breaking it — records listed as records. Costs one cached fold, in the place `vm::Collection` established (`app.rs:4533-4541`). |
| **3** | **`Look up`, via the desktop** (§6.4): one quiet word-button in the band's line, opening the artist on Wikipedia in the listener's own browser through `org.freedesktop.portal.OpenURI` on the connection `mpris/server.rs:33` already makes. | **Zero new crates. No `--share=network`. No licence obligation. No cache. No failure mode.** It answers *"maybe just the wikipedia for the band"* this week, and it is explicitly **not** presented as a substitute for tier 4 — it is what the page has while tier 4 is open, and it may be all he wanted. |

### Tier 2 — adopt with modification

| # | proposal | the modification |
|---|---|---|
| **4** | **Genres in the band** (§4.4). | Case-folded de-duplication with the first spelling kept (ADR-0019 §4's rule, available here where `artist::label` could not use it), **capped at three, with no `+N more`**. Without the fold, `Post-Rock` / `post rock` reads as baz's sloppiness rather than the tagger's. |
| **5** | **An artist picture off the listener's own disk** (§4.5): `artist.jpg` / `folder.jpg` in the *parent* of the album directories, through `art.rs:136-148`'s existing case-insensitive lookup and the same downscale-only decode. | Needs a composition decision this study does not make: where it sits relative to the band, and at what edge. Absent, not empty, at the great majority of libraries that have no such file. It is the cheapest thing here that makes the page feel like a page, and it costs no crate and no request. |
| **6** | **Reconcile the artist page's grid with the wall's** (§1.1). | One line: feed `Grid::new` the same width `Shelf::grid_width` computes, or state in the prose that it deliberately does not. Visible consequence at exactly one shipped size — 1920 with the lane collapsed goes 6 columns → 5. Adopt the *prose* fix unconditionally; the *geometry* fix wants his eye on a frame, so it is also tier 3 #11. |

### Tier 3 — present to the owner

| # | question | why only he can answer it |
|---|---|---|
| **7** | **May the ledger be folded by artist at all?** ADR-0018 §6 says **"no totals-by-artist"** in those words, and gives the mechanism: *"the way not to build them is not to provide the surface that makes them easy."* | It is a written decision in two ADRs, and reversing it as a side effect of a page redesign is exactly the failure `WORK.md`'s preamble exists to prevent. *Needs: yes or no.* |
| **8** | If yes to #7, **is this the string?** `First heard 2019 · Last played 3 months ago` — two moments, in `Recency::label()`'s own vocabulary (`history/read.rs:104-118`), **no counts, no ranking, no comparison to any other artist**. | It is history as a *door*, which the returns lane already does without performing, rather than history as a *figure*. If any ledger fact belongs on this page, it is this one and in this form. *Needs: yes to this shape, or a different one.* |
| **9** | **Records never played, by this artist** — `2 of 6 never played`. | Cut once already, from `COLLECTION`, and named there as *"the one on the list that is easy to mistake for an inventory fact"* (`views/home.rs:790-793`). It is also the single most *useful* line in this whole study for a listener with a large collection. Genuinely close. *Needs: his call.* |
| **10** | **Should the band be one line or two?** One is proposed (§4.2). Two would carry genres and the ledger line without ellipsis at 600 px. | 40 px versus 56 px of wall pushed down, and it is a judgement about how much preamble a page of covers may have. *Needs: his eye on a frame.* |
| **11** | **Should the artist page's covers be the wall's size to the pixel?** (§1.1, tier 2 #6.) | Four to eleven pixels at most sizes, and one whole column at 1920 with the lane collapsed. Nobody has complained; it is either a defect or a page that is allowed to breathe. *Needs: his eye, not an argument.* |

### Tier 4 — the dependency decision, which is his and is recorded as ADR-0037

| # | question | the costing, in one line each |
|---|---|---|
| **12** | **May baz make its first network request?** | `ureq` 3.4.0: **14 net-new crates**, one new `deny.toml` licence (`CDLA-Permissive-2.0`), blocking (which fits), no new build tool — **but** `ring` is C + assembly with a `links` key, and it would be the first C in baz parsing hostile input off the wire. |
| **13** | **If no to `ureq`, is `reqwest` the answer instead?** | **No, and this study says so rather than presenting it as a choice**: 57 net-new crates, `aws-lc-sys` with `cmake` + `bindgen` + NASM. That is `docs/BACKLOG.md:293-302`'s Opus refusal word for word, and the Opus refusal was made for *a music format*. |
| **14** | **And the part that is not technical at all**: adding `--share=network` to `packaging/flatpak/io.github.mattcree.baz.yml` puts **"Network access"** on baz's Flathub page, permanently and visibly, for an offline-first player. | There is no engineering answer to this. It is what the product says about itself, and that is his. |

**If the honest answer is that the cost is too high, that is a legitimate
outcome**, and tier 1 #3 is what ships in its place: the page is good
offline, the encyclopaedia is one press away in the listener's own browser,
and baz still has zero network dependencies. This study's recommendation, on
the evidence gathered, is **tier 1 + tier 2, ship now; tier 4, his call,
unhurried** — because the local page is 90 % of the value at 0 % of the
cost, and because a first network request is the kind of decision that is
much easier to make than to unmake.

### Not proposed, and why

- **A prohibition on any of it.** `docs/REFUSALS.md` was deleted because it
  had become law over the owner. §3's argument is *rationale for a choice*
  and it lives in a design doc and an ADR, which is where reasoning belongs.
  If he wants play counts on the artist page, they go in.
- **Scrobbling.** Out of scope here as it was in ADR-0018 §7, and for the
  same reason: it is *optional output, never a dependency*, and it attaches
  to `Event::PlayRecorded` downstream of everything in this study.
- **An artist image off the network.** Wikipedia's `thumbnail` field is
  right there in hop 3's payload and it is a different licence for every
  file — Commons images are individually licensed, and some are non-free
  fair-use on en.wiki. Getting that wrong is a copyright problem, not a
  design problem. Tier 2 #5's picture comes off the listener's own disk, and
  that is the only artist image this study proposes.
- **A "refresh metadata" verb.** Deleting a file in `artists/` is the whole
  of it (§7.4), and a verb that exists to undo a cache is a confession that
  the cache is wrong.

---

## 10. What this costs in tests

| test | file | what changes |
|---|---|---|
| `the_counts_line_honours_its_singulars` | `views/artist.rs:178-181` | unchanged — the strip's note is untouched by every tier. |
| a new `the_band_states_only_what_it_can_prove` | `views/artist.rs` | each term absent when its source is `None`; an artist with no years, no durations and no `first_seen_ns` produces **no band**, not an empty one. |
| a new `also_on_never_repeats_a_record_the_page_already_shows` | `views/artist.rs` | §4.3's exclusion, asserted rather than commented. |
| `the_home_collection_cells_hold_their_figures_and_their_words` | `font.rs:743` | unchanged — the band is deliberately **not** a `STAT_W` lattice, so this test's claim stays about Home alone. |
| a new band-measure test | `font.rs` | the band's longest realistic string fits the 510 px content lane at `TOP_BAR_FLOOR`, the same claim `theme.rs:1277-1285` makes for `COLLECTION` — as a test rather than a const-assert, since the string is runtime. |
| `the_serif_is_the_work_titles_and_nothing_else` | `theme.rs:4182-4238` | **unchanged, and that is the point**: the artist's name is not a work's title, so it stays in the sans and this enumeration does not grow. Doc 14 §5.2's line — *the serif sets an album's title, on the surface whose subject that album is* — already answers this page. |
| `cargo deny check` | `deny.toml` | tier 4 only: one allowlist entry, `CDLA-Permissive-2.0`, with the reason recorded inline as the file's own convention requires. |
| a new `nothing_reaches_the_network_unless_the_listener_asked` | `crates/baz` | tier 4 only, and it is the load-bearing one: a source walk asserting the HTTP client is constructed from exactly one call site, and that the call site is the artist place's open, not any playback path. The same shape as `theme.rs`'s serif enumeration — it fails the build on a second caller. |

---

## 11. The one-line summary for `CHANGELOG`

**Tier 1 + 2:** *The artist's page says what you own — hours, years, formats
and when they arrived — lists the records they guest on, and offers the
encyclopaedia through your own browser. No network, no new crates.*

**Tier 4, if taken:** *An artist's page can fetch one paragraph from
Wikipedia, off by default, once per artist, cached on your disk, and never
while music is playing.*

---

## Sources for §5 and §7

- [MusicBrainz API rate limiting](https://musicbrainz.org/doc/MusicBrainz_API/Rate_Limiting)
- [Wikimedia Foundation User-Agent Policy](https://foundation.wikimedia.org/wiki/Policy:Wikimedia_Foundation_User-Agent_Policy)
- [Wikipedia:Copyrights](https://en.wikipedia.org/wiki/Wikipedia:Copyrights) — CC BY-SA 4.0, and what attribution requires
- [Roon — music data](https://roon.app/en/music/data) · [Roon metadata model](https://help.roonlabs.com/portal/en/kb/articles/metadata-model) · [TiVo music metadata](https://business.tivo.com/products-solutions/metadata/music-metadata)
- [Plex — metadata agents](https://support.plex.tv/articles/200241558-agents/) · [Plex forum — music metadata sources](https://forums.plex.tv/t/music-metadata-sources/936164)
- [MusicBee wiki — Navigator](https://musicbee.fandom.com/wiki/Navigator) · [MusicBee wiki — Last.fm](https://musicbee.fandom.com/wiki/Category:Last.fm)
