# ADR-0037: The artist's page states what you own — and the first network request is the owner's to authorise

**Status**: proposed (2026-08-10) · **§1–§4 accepted-on-adoption** (they need
no other decision reopened) · **§6 is a decision put to the owner, not taken
here** · reads
[ADR-0030](0030-the-returns-lane-and-the-home-band.md) §6's refusal of
engagement statistics as binding and stays on its side of the line · puts
**one question** to [ADR-0018](0018-play-history-ledger.md) §6, which names
*"no totals-by-artist"* in as many words · the study, with every measurement,
is [`docs/design/15-the-artist-page.md`](../design/15-the-artist-page.md)

## Context

The owner, looking at the shipped Artist place:

> *"ideally the by artist page could have more info, maybe just the wikipedia
> for the band or something?"*

`crates/baz/src/views/artist.rs` draws a header strip with the artist's name
and `6 records · 74 tracks` (`artist.rs:106-112`, `artist.rs:156-170`), one
`section_rule("Records")`, and the wall's tiles (`artist.rs:114-136`). That
is the whole page: **a filtered wall with a title on it**, and nothing
between the rule and the tiles or after them.

The sentence contains two asks with nothing in common but a page, and the
whole of this ADR's structure is the separation:

1. **More information on the page.** Free. baz already holds every fact
   needed, on the listener's own disk.
2. **Wikipedia.** **baz's first network request**, and the precedent binds
   every enrichment after it.

### What the second one is measured against

- **There is no network code in baz.** `std::net`, `TcpStream`, `UdpSocket`,
  `reqwest`, `ureq` — **zero occurrences** across `crates/*/src` at
  `eae1b90`.
- **Zero system dependencies, defended in writing, four times.**
  `Cargo.toml:33-36` (SQLite `bundled`, *"no system library or -dev package
  […] on any platform"*), `Cargo.toml:41-62` (*"the whole decode path is
  pure Rust, and staying that way is a build-system property worth
  keeping"*), `Cargo.toml:81-88` (zbus: *"pure Rust — no C, no system
  library, no pkg-config"*), `Cargo.toml:89-95` (`rfd` without gtk-sys).
- **The precedent.** `docs/BACKLOG.md:271-330` refuses Opus because libopus
  bindings cost *"a **C library and a `cmake` build dependency on every
  platform**"* and *"spending that property on one lossy format is not a
  trade worth making **unprompted**"*. This request is prompted. That
  changes **who decides**; it does not change the arithmetic.
- **`deny.toml`** allows twelve licences and calls extending the list *"a
  reviewed decision"* (`deny.toml:45-75`).
- **`packaging/flatpak/io.github.mattcree.baz.yml:26-50`** lists the
  sandbox's `finish-args` in full and **contains no `--share=network`**. The
  shipped Flatpak cannot open a socket.

## Decision

### 1. The page states **what you own**, and that is what makes it worth visiting

One line of type under the header strip, at `SIZE_META` 12 / `LINE_META` 16
in `paper_faint` — the record page's catalogue-line voice
(`views/album.rs:567-570`), which is also the ink the header strip's own
note takes (`views/mod.rs:365-372`), so the page has one quiet-fact voice
rather than two. No new size, no new ink, no new token:

```
4 hours 12 minutes · 1988–1991 · FLAC, MP3 · In your library since 2019
```

- **It reads as a sentence, not a table.** That is the test that picked
  `COLLECTION`'s four figures
  (`docs/design/impl/home-stats/README.md:151-153`) and it is the test that
  picks these.
- **Not a `STAT_W` lattice.** `COLLECTION`'s four cells
  (`theme.rs:1264-1275`, `views/home.rs:838-860`) are the right shape for a
  footer you consult and the wrong shape above a wall of covers, where they
  would read as a dashboard header. The room does not have those.
- **Each term is absent, not empty** — ADR-0030 §6's rule as Home keeps it
  (`views/home.rs:76-83`). No years, no year term. **No terms at all, no
  band**, and the page is exactly today's page.
- **One line, forever.** `LINE_META` 16 + `GAP_XL` 24 = **40 px** of wall
  pushed down. A second line is a paragraph and the page acquires a
  preamble. (Whether it may be two is design 15 §9 tier 3 #10 — his eye, on
  a frame.)
- **The header strip does not move.** `artist.rs:106-112` fixes its lead to
  `TRANSPORT_HIT` 32 because the album page's breadcrumb is one press away
  at the same height; a height change across that press is a jump. Nothing
  here touches it, and nothing lengthens the right-edge note, which is
  `Wrapping::None` and would clip at `TOP_BAR_FLOOR` 600
  (`views/mod.rs:365-372`, `theme.rs:2017`).

### 2. `ALSO ON` — records they guest on, as records

A second `section_rule` under the wall, in the wall's own tiles: albums
filed under *someone else* that carry this artist on one or more tracks,
folded on `vm::artist_id(TrackVm::artist)` (`vm.rs:303-306`).

- **This is not what `artist.rs:32-35` refused.** That refusal is of *"a
  flat list of every track they appear on"*, which would break ADR-0017
  §1.7's *"albums listed as albums, never flattened"*. `ALSO ON` lists
  **records**, in the tile — the rule kept, not broken. The album page it
  doors to already draws the per-track artist column that explains the tile,
  because `AlbumVm::track_artists_vary` exists for exactly this case
  (`vm.rs:75-85`).
- **Never a record already under `RECORDS`.** One fact drawn twice is doc 07
  L8.6's test, and the same record under two headings on one page is the
  most visible way to fail it.
- **Absent when empty**, which is most artists in most libraries.
- **Cost, and where it is paid.** The fold is a walk over every track, and
  ADR-0030 §4 forbids paying a walk per frame. It goes exactly where
  `vm::Collection` went — built in `Shelf::rebuild_shelves`, held on the
  shelf (`app.rs:4533-4541`): **one pass per rebuild, zero per frame.** The
  held value is bounded by the number of guest credits, not by the library.

### 3. The ledger stays off this page, and ADR-0018 §6 is not reopened here

`COLLECTION` got past ADR-0030 §6 on one sentence, and it is a test rather
than a slogan (`views/home.rs:71-76`):

> it describes **what you own**, not what you do with it, and **every figure
> in it would be identical if the application had never been opened**.

§1's five terms and §2's records pass it unchanged. *First heard*, *last
heard*, *play counts* and *records never played* fail it — and ADR-0018 §6
does not merely decline a fourth read surface, it declines **this one**, by
name:

> There are now **two**, and there was never going to be a fourth. **No
> totals-by-artist**, no listening-time-per-month, no top-N. Those would be
> *built from this data*, so the way not to build them is not to provide the
> surface that makes them easy […]

*First heard, folded by artist* **is** totals-by-artist. So it is not built
here.

**This is not a prohibition, and the distinction matters.** `docs/REFUSALS.md`
was deleted because it had become law over the owner, and the repo's one
rule is that what he asks for goes in the app. He asked for *more
information*; he did not ask for play counts. What is refused is **an agent
reversing a written decision in two ADRs as a side effect of a page
redesign**. Design 15 §9 tier 3 #7–#9 put it to him as three concrete
questions, including the smallest admissible form — `First heard 2019 ·
Last played 3 months ago`, in `Recency::label()`'s own vocabulary
(`baz-core/src/history/read.rs:104-118`), **no counts, no ranking, no
comparison to another artist**, which is history as a *door* rather than as
a figure, the way the returns lane already reads it (ADR-0030 §1). One
sentence from him adopts it.

### 4. `Look up` — the encyclopaedia, at zero cost, now

One quiet word-button in §1's line, opening the artist on Wikipedia **in the
listener's own browser**:

- Linux: `org.freedesktop.portal.OpenURI` over D-Bus. baz already speaks
  D-Bus with `zbus::blocking::Connection`
  (`crates/baz/src/mpris/server.rs:33`) and zbus is already a direct
  dependency (`Cargo.toml:81-88`). **Zero new crates.**
- Windows and macOS: one `std::process::Command`.
- **No `--share=network`** — the portal opens the link on the host, outside
  the sandbox. baz's Flathub permission list does not change by one word.
- No TLS, no cache, no rate limit, no User-Agent obligation, no CC BY-SA
  obligation (baz displays no Wikipedia text), no offline failure mode, and
  no layout that can jump.

It does not put the paragraph on the page, which is what was asked. It is
adopted **as what the page has while §6 is open**, and explicitly not as a
substitute for it. It may turn out to be all he wanted.

### 5. If the request is ever made, this is its shape — specified now so the decision in §6 is made against a real design

**5.1 Three hops.** `musicbrainz.org/ws/2/artist/?query=…&fmt=json&limit=5`
(name → candidate MBIDs, with `inc=url-rels` folding the Wikidata link into
the same response) → `wikidata.org/wiki/Special:EntityData/<QID>.json?props=sitelinks`
(→ `sitelinks.enwiki.title`; **unrestricted, that document is ~150 kB for
one string**) → `<lang>.wikipedia.org/api/rest_v1/page/summary/<title>`
(→ `extract`, `description`, `content_urls`, `revision`, `timestamp`).

**5.2 A wrong match is corrected by the listener, never by a heuristic.**
One strong candidate resolves silently. Two or more and **the block is
absent**, with one line in its place — `Two artists match "Nadir". Choose
which` — opening ADR-0031's picker at the pointer, listing each candidate as
MusicBrainz describes it (`disambiguation`, `type`, `life-span`, which exist
for exactly this). A match already showing is re-openable from the
attribution line itself: **no settings page, no rescan, no "refresh
metadata" verb.** `None of these` is a row, and choosing it is remembered.

**5.3 The cache.** `$XDG_DATA_HOME/baz/artists/<mbid>.json`, beside
`library.db` and `history.tsv` — ADR-0018 §1's argument for where baz's own
records live, and ADR-0018 §3's argument for why this is a file and not a
table. One plain file per artist, carrying the MBID, the QID, the article
title, language, URL, revision and fetch instant, plus the extract. **No
expiry, no background refresh, no watcher.** Deleting a file is the whole of
"refresh"; deleting the directory is the whole of "forget". **Negative
results are cached too** — no match, listener said none, service said 503 —
or every visit to an unmatched artist is a fresh chain.

**5.4 The obligations, met exactly.**

- **MusicBrainz** requires a descriptive User-Agent with contact information
  and rate-limits to **1 request/second per source IP**, declining the rest
  with 503. This was verified rather than assumed: an anonymous-agent
  request to the artist search **returned 503 on the first attempt**. baz
  sends `baz/<version> ( https://github.com/mattcree/baz )`, serialises
  through one worker with a ≥ 1 s floor, and makes at most one chain per
  page opened by a person.
- **Wikimedia**'s User-Agent policy warns that non-compliant clients *"may
  be blocked without notice"*; baz sends
  `baz/<version> ( https://github.com/mattcree/baz ) ureq/<version>`.
- **Wikipedia text is CC BY-SA 4.0.** Attribution is satisfied by a
  hyperlink to the article, and a notice stating the licence is also
  required. So `From Wikipedia · CC BY-SA 4.0` is drawn under **every**
  extract, always, both halves doors — never behind a hover, a menu or a
  tooltip. **The extract is reproduced verbatim and never edited,
  truncated, summarised or merged**, because modifying it attaches
  share-alike to the modification; not modifying it is free.
- **Share-alike does not reach baz's source.** baz is GPL-3.0-or-later
  (`Cargo.toml:10`); the extract is a separately-licensed work *displayed*
  by the program and cached beside it — the relationship the program already
  has to the listener's FLAC files. The cache file carries its own licence
  and article URL so a copy travelling alone travels with them.
- **`deny.toml` is untouched by CC BY-SA.** It walks the Cargo graph, and
  this is data, not a crate — the distinction the file already draws for the
  bundled IBM Plex typeface at `deny.toml:49-56`.

**5.5 Privacy, stated rather than buried.** A request tells someone else's
server an artist name, an IP and a timestamp, and that *is* what you are
listening to. VISION.md's third pillar is *sovereignty by default* and
ADR-0018 §7 is a page about there being *"no identifier, no machine ID, no
session key, no hash of anything"*. Therefore:

1. **Off by default**, with **no first-run prompt** — a prompt at first run
   is a dark pattern wearing consent.
2. **One switch, in a third Settings section** (`views/settings.rs:148` has
   two, `Playback` and `Library`; this is neither), in that file's
   established shape of a name and one present-tense sentence
   (`views/settings.rs:698-724`):

   > **Artist information** — *When you open an artist's page, baz asks
   > MusicBrainz and Wikipedia who they are. Nothing else on your machine is
   > sent, nothing is sent while music plays, and each artist is asked about
   > once.*
   >
   > Switch: **`Look artists up online`**. Off.

   Under it, in the readout ink, **where the answers are kept and how to
   remove them** — a promise about local storage that does not say where is
   not a promise.
3. **Never triggered by playback.** Not by `TrackStarted`, not by a scan,
   not by a queue change, not by MPRIS, not by the lane rebuilding. **Only
   by a person opening an artist's page.** This is what makes the log on
   somebody else's server a record of what you *looked up*, which you chose,
   rather than what you *played*, which you did not.
4. **Once per artist, ever** — including negatively.
5. **No identifier of any kind**: no install id, no session, no cookies, no
   `Referer`. The User-Agent identifies **baz**, as both services require,
   and nothing about the person running it.
6. **The switch stops asking; deleting the directory forgets.** Two acts,
   never conflated, or the switch would silently destroy data.

**5.6 Offline is the normal case, and the composition is what guarantees
it.** The block is **the last thing on the page** — `RECORDS`, `ALSO ON`,
then `ABOUT` — so a late arrival grows the scrollable and **nothing already
drawn can move**. That is the no-jump requirement solved by ordering rather
than by reserving space. **Absent, not empty**: with the setting off, with
no network, with no match, with a 503, there is **no `ABOUT` rule on the
page at all** — no box, no skeleton, no `Loading…`, no spinner. A listener
who never turns it on never sees a trace of the feature, including its
absence. Nothing is drawn optimistically, the same rule
`views/album.rs:55-56` already keeps, and a chain is abandoned on
navigation.

### 6. The dependency decision is the owner's, and here is what it costs

Measured 2026-08-10 by resolving each candidate in a scratch crate outside
this repo and intersecting `cargo tree -e normal --target
x86_64-unknown-linux-gnu` against baz's `Cargo.lock` (558 entries, 497
distinct names).

| | **`ureq` 3.4.0** (rustls + ring + webpki-roots) | **`reqwest` 0.13.4** (`rustls`, `blocking`) |
|---|---|---|
| crates in the graph | 27 | 82 |
| **net new to baz** | **14** | **57** |
| new build tools | **none** — `cc`, `shlex`, `find-msvc-tools` are already in the lock (`Cargo.lock:561`) | **`cmake` + `bindgen` (libclang)**, and NASM on Windows x86/x64 |
| native-code crate | `ring`: `build = "build.rs"`, `links = "ring_core_0_17_14_"`, `build-dependencies.cc` — C and per-arch assembly, **vendored, nothing downloads** | `aws-lc-sys`: `build = "builder/main.rs"`, `links = "aws_lc_0_44_0"`, build-deps `cmake` + `bindgen` |
| new `deny.toml` licences | **one**: `CDLA-Permissive-2.0` (`webpki-roots`) | at least two, incl. `MIT-0` inside a seven-way `AND` |
| async runtime | **none** — blocking, which fits `tokio` being present *"for the iced shell only"* (`Cargo.toml:63`) | hyper, tower ×4, mio, socket2, and the whole ICU4X chain for `idna` |
| Flathub vendoring | 556 archives → 570; compiles in the SDK sandbox as-is | plus cmake and libclang in the build environment |

**Three findings are recorded as findings, not as options:**

1. **`reqwest` is `docs/BACKLOG.md:293-302`'s Opus refusal word for word** —
   a C library plus a `cmake` build dependency on every platform — for one
   paragraph of text, where the refused version was for *a music format*.
   **It is not a close call and this ADR does not present it as one.**
2. **`ureq`'s TLS is not pure Rust.** `ring` is C and assembly with a
   `links` key. Mitigating: a C compiler is **already** required
   (`rusqlite` `bundled`, `Cargo.toml:36`), so this is a new *C surface*,
   not a new *build requirement*, and nothing downloads at build time.
   Aggravating: it would be **the first C in baz parsing hostile input off
   the wire** — `libsqlite3-sys` parses a file baz wrote, and `symphonia` is
   pure Rust and fuzzed. The pure-Rust rustls providers are young, which is
   the same judgement `docs/BACKLOG.md:303-310` made about the young Opus
   decoders, pointing the same way.
3. **The most expensive line is not technical.** Adding `--share=network` to
   the Flatpak manifest puts **"Network access"** on baz's Flathub page,
   permanently and visibly, for an offline-first music player. There is no
   engineering answer to that; it is what the product says about itself.

> **The question, in one sentence: may baz spend its zero-network-dependency
> property, plus a visible Flathub network permission, on one paragraph of
> encyclopaedia text per artist — given that §4 already puts that paragraph
> one press away in the listener's own browser for nothing?**

**If the answer is no, that is a complete and legitimate outcome.** §1–§4
ship, the page is good offline, and this ADR stands as the record of what
was priced. If the answer is yes, §5 is the whole build and nothing in it is
open.

## Consequences

- **`views/artist.rs` gains one line of type and one section**, and loses
  nothing. Its `counts` note and its header geometry are untouched.
- **`Shelf` gains one cached fold** for §2, in `rebuild_shelves`, beside
  `vm::Collection` — the pattern `app.rs:4533-4541` already establishes, and
  the same responsiveness contract (ADR-0030 §4) satisfied the same way.
- **`views/artist.rs:28-35`'s prose must be amended in the commit that lands
  §2 or the code argues with this ADR.** Its refusal of *"a flat list of
  every track they appear on"* stands and is quoted approvingly; its
  sentence *"nothing in baz goes to the network"* stays true under §1–§4 and
  becomes false the day §6 is answered yes.
- **`docs/design/impl/artist-page/`** gets the frames when §1–§2 build, in
  the shape doc 14's tiers established.
- **A `file:line` correction lands with this ADR**: `artist.rs:19-26` claims
  the tiles are *"the wall's to the pixel"*, and they are not — the wall
  feeds `Grid::new` `window − sidebar − INDEX_LANE_W 108 − 4`
  (`app.rs:5095-5101`) where this page feeds it `body_width − 2 × HANG 40`
  (`artist.rs:114-117`). Covers are 4–11 px wider here at every size, and at
  **1920 with the lane collapsed this page draws six columns where the wall
  draws five**. The prose is corrected unconditionally; whether the geometry
  is reconciled is design 15 §9 tier 3 #11, and it wants a frame.
- **Nothing in §6 is started before it is answered.** No branch, no
  `Cargo.toml` line, no `deny.toml` entry, no client behind a feature flag.
  A dependency added *"in case"* is a dependency added.

## Deliberately not done

- **No prohibition on anything.** `docs/REFUSALS.md` was deleted for
  becoming law over the owner. Every refusal above is rationale for a
  choice, sited in an ADR and a design doc, and every one of them is
  reversible by one sentence from him.
- **No artist image off the network.** Wikipedia's `thumbnail` is in hop
  3's payload and is a *different licence per file* — Commons images are
  individually licensed and some are non-free fair-use. That is a copyright
  problem, not a design problem. The only artist image proposed is
  design 15 §4.5's: `artist.jpg` / `folder.jpg` from the **parent of the
  album directories**, through `art.rs:136-148`'s existing lookup and the
  same downscale-only decode — the listener's own file, no crate, no
  request.
- **No chart, no timeline, no decade histogram.** ADR-0030 §6's posture is
  that history records and does not perform, and a chart is performance
  whatever it is made of. `1988–1991` is the fact.
- **No second arrangement control for the records** — `artist.rs:54-60`
  already argues it out.
- **No size on disk.** Cut once from `COLLECTION` as *"a fact about a
  filesystem, and nothing you would do differently having read it"*
  (`views/home.rs:794-797`). The record page's `Details` block is where
  bytes belong.
- **No scrobbling.** Out of scope as in ADR-0018 §7 and for its reason:
  *optional output, never a dependency*, attaching to `Event::PlayRecorded`
  downstream of everything here.
- **No "refresh metadata" verb.** Deleting a file in `artists/` is the whole
  of it, and a verb that exists to undo a cache is a confession that the
  cache is wrong.
- **No hand-rolled HTTP without TLS.** MusicBrainz and Wikipedia are
  HTTPS-only and HSTS-preloaded; there is no plaintext endpoint, and a
  hand-written TLS 1.3 is not a thing this project will contain. The
  "no new crates *and* an in-app fetch" option does not exist, and saying so
  is part of the costing.

## Considered and rejected

- **A `STAT_W` stat row on the artist page**, mirroring `COLLECTION`. Right
  shape for a footer you consult, wrong shape above a wall of covers, where
  four figures at `SIZE_EMPHASIS` read as a dashboard header.
- **The facts in the header strip's right-edge note.** It is
  `Wrapping::None` and would clip at `TOP_BAR_FLOOR` 600, and the strip's
  height is load-bearing across the breadcrumb press.
- **The facts at the foot of the page**, where `COLLECTION` sits. Home put
  its figures last so an inventory could not push down the thing you press;
  here the figures *are* what the page was missing, and below sixty tiles is
  where facts go to die.
- **Wikipedia searched directly by name**, skipping MusicBrainz and
  Wikidata. One request instead of three, and wrong for the reason the chain
  exists: *Nadir* the band, *Nadir* the album, *Nadir* the point below your
  feet — and the page would state the astronomical one confidently.
- **MusicBrainz's `wikipedia` URL relation**, skipping Wikidata. Deprecated
  upstream in favour of the Wikidata relation, and it is the one that goes
  stale when an article is renamed. Wikidata is the indirection that
  survives a rename; that is what it is for.
- **A server or proxy of baz's own** between the listener and the two
  services, which is how Roon and Plex solve this. It is an account, a
  running cost and a company that knows what everyone looked up — the exact
  model VISION.md's third pillar refuses. It also has a documented failure
  mode: MusicBee lost Last.fm as a source when Last.fm's API changed, and
  Plex's Last.fm agent was retired under its users.
- **Fetching on scan**, as Plex's server does. It is fast and it is the one
  thing §5.5 clause 3 forbids: a scan-time fetch tells somebody else's
  server your entire collection in one sitting, unasked.
