# 14 — Records and lists: telling a found thing from a made one

> **The owner, 2026-08-10**, two sentences in one breath:
>
> 1. *"we do not have the playlist name really prominent. basically the
>    information heirarchy isn't great to be able to tell the difference
>    between an album and a playlist"*
> 2. *"'save as playlist' really makes no sense on the playlist page for a
>    CD"*
>
> Logged in `docs/BACKLOG.md`, *What the owner asked for*, as **designing** — *"the family was made
> deliberately; the cost is now visible"*.

**Status**: design study · **tier 1 shipped 2026-08-10** (§9) · **tier 2
shipped 2026-08-10** — #6 and #7 adopted, **#8 declined on a frame** (§9) ·
tier 3 is three questions waiting on the owner ·
**§6 amended 2026-08-10** — *one page, two subjects*: the arrangement this
study said the two pages *share* was two copies of it, and making it one
found two divergences §3.4's table could not see (the quiet act's lane, and
a whole page 12 px out because its strip's lead was a word rather than a
control). Proposes an
amendment to
[ADR-0024](../adr/0024-playlists.md) §A1–§A2, which is where the sleeve and
the shared page arrangement were decided, and touches
[ADR-0030](../adr/0030-the-returns-lane-and-the-home-band.md) §2, which is
where the lane's refusal to mark the two kinds was decided. Tiered
proposals in §9. Every claim about today's UI carries a `file:line` or a
frame.

---

## 0. The finding, in one sentence

The two sentences are one bug.

`Save as playlist`, pressed while a CD is playing, writes a playlist whose
only member is that record; a one-record playlist's sleeve is
[the record's own cover, full-bleed](#41-the-sleeve-is-not-the-signal-it-was-designed-to-be)
(`views/mod.rs:221-223`); and that new thing then lands in the returns lane
**above the record it was made from, wearing its face, in the same type, at
the same size** (`views/lane.rs:557-671` — one code path, no branch). The
control the owner called nonsense is a machine for manufacturing the
confusion he reported in the other sentence.

So the fix is not a badge. It is to decide **what a made thing looks like**,
and then to make the control that makes made things say what it is doing.

---

## 1. The defect: `Save as playlist`

### 1.1 Where it is actually drawn

It is **not** on `Place::Playlist`. `views/playlist.rs` never builds it — the
only draw site in the product is `save_control` at
**`crates/baz/src/views/queue.rs:337-353`**, pushed into the run column's
summary strip at **`queue.rs:246`**:

```rust
row![
    text(list.summary) …,
    undo_control(can_undo),
    Space::with_width(Length::Fill),
    save_control(saving.is_none()),
]
```

`run_column` has exactly one caller — `views/now_playing.rs`, asserted by
`every_queue_affordance_survives_the_merge` (`now_playing.rs:826-838`). So
the word appears on **`Place::NowPlaying`**, the surface the queue merged
into this afternoon.

The owner called that surface *"the playlist page"*, and he is not
misreading it. The module says so itself
(`queue.rs:49-56`):

> *"the queue place and the playlist page are **the same editor** (09 §8.2),
> differing only in their header blocks"*

The right half of Now playing **is** the playlist editor. Its rows are the
playlist page's rows; its ▲▼, ✕ and transfer `+` are the playlist page's
edit set. What is missing is any statement that the thing being edited is a
run rather than a file.

### 1.2 What the frame shows

`docs/design/impl/queue-merged/01a-run-on-1280x860.png` (1280 × 860, lane
open, run standing, playing the record *Ochre* by Anne-Marie Puig):

```
  x=800                                              x=1240
   ┌────────────────────────────────────────────────────┐
y=105│ 1 of 24 · 1:56:19 left   Undo        Save as playlist │   ← the strip
   ├────────────────────────────────────────────────────┤
y=162│ Ochre                                              │   ← the record's
y=182│ Anne-Marie Puig                                    │     own title
   ├────────────────────────────────────────────────────┤
y=208│ ●  Undertow 1                              3:23    │
      …
```

**`Save as playlist` sits 57 px above the record's own title.** Nothing
between them says the column is a run. The strip's left end reads
`1 of 24 · 1:56:19 left`, which is a run statement, but it is 340 px away
and hard against the opposite edge; the word is optically a control *of the
column*, and the column is showing a CD.

The same reading at 1920 × 1080
(`docs/design/impl/queue-merged/02a-run-on-1920x1080.png`): strip at
y = 105, `Save as playlist` right-aligned at x ≈ 1880, `Ochre /
Anne-Marie Puig` at y = 162/182.

### 1.3 Why it is a category error, in two cases

**Case A — the run is a record, unedited.** The thing is already saved: it
is an album on disk with a cover, a title and an artist. Pressing the word
produces a second artefact whose contents are that record, whose name is
whatever you typed, and whose sleeve — because it resolves to one distinct
record — is *that record's cover full-bleed* (`views/mod.rs:221-223`). Two
objects, one face, adjacent in `RECENT`. This is §0's loop.

**Case B — the run came from a playlist.** `PlayerState::queue_provenance`
(`player.rs:1800`) holds the file's name while that origin stands, and
`queue_summary` (`player.rs:2267-2270`) prints it at the **head of the very
strip the word sits in**:

```rust
match &queue.provenance {
    Some(name) => format!("{name} · {reading}"),
    None => reading,
}
```

so the strip literally reads

```
Road Trip · 1 of 14 · 52:31 left   Undo              Save as playlist
```

— an offer to save a thing whose name is printed two inches to the left.
Pinned by `the_summary_leads_with_provenance_until_a_new_run_replaces_it`
(`player.rs:5183`).

`save_control` is conditioned on **nothing** except whether its own name
field is already open (`save_control(saving.is_none())`, `queue.rs:246`). It
does not read provenance, it does not read whether the run has been edited,
and it does not read whether the run is one record in its own order.

### 1.4 The fix

The one rule of this repo is that anything the owner asks for goes in the
app, so **nothing here removes the control**. The act it performs —
freezing a transient into a file — is real, is ADR-0024 §4's *creation act*,
and is genuinely wanted for a shuffle, a `Play all`, or a run you have
edited. What is wrong is that it never says what it is saving, and that it
claims to be able to act when it cannot usefully act.

**The precedent is in the same file, eleven lines up.** `undo_control`
(`queue.rs:305-318`):

> *"Drawn only while there is an edit to take back: a standing `Undo` over a
> list nobody has edited would be **a control that cannot act pretending it
> can**."*

`Save as playlist` over a run that is already a named file is exactly that
defect. Three changes, all of which the existing state can already answer:

**F1 · The strip names its subject.** Today the strip's left end says
`1 of 24 · 1:56:19 left` when there is no provenance — a reading with no
noun. Give it the noun the provenance branch already supplies:

| run | strip reads today | strip reads after |
|---|---|---|
| from a playlist | `Road Trip · 1 of 14 · 52:31 left` | unchanged |
| from a record, or shuffled, or `Play all` | `1 of 24 · 1:56:19 left` | **`Run · 1 of 24 · 1:56:19 left`** |

One word, in the string `queue_summary` already builds, in the branch that
already exists. It costs no widget and no height, and it is what makes the
word beside it unambiguous — `Save as playlist` is now visibly an act on
*the run*, not on the record whose title is 57 px below.

**F2 · When the run is already a saved file, the control states that
instead of offering it.** Provenance standing and no edit since the run was
reified ⇒ the word becomes an inert statement in the same slot, same size,
same quiet ink:

```
Road Trip · 1 of 14 · 52:31 left   Undo                Saved as “Road Trip”
```

Not a disabled button and not a removal — a readout, which is what the
panel's `Queue` row already is at rest (ADR-0024 amendment item 2: *"At rest
the panel's Queue row is a readout, not a door"*). The moment the run is
edited — a ✕, a stepper, a drag, an append — provenance is *origin, never a
live link* (ADR-0023 §3; `player.rs:2243-2244`), the run has diverged from
the file, and the live word returns as **`Save as new playlist`**.

> **What F2 must not become.** `Save changes to "Road Trip"` — writing the
> run back into the file it came from — is refused, and not on taste:
> ADR-0024 amendment item 6 keeps the run *tonight's snapshot* and holds
> §1's decoupling **in both directions**, and ADR-0023 §3 makes provenance
> an origin rather than a link. A run that wrote itself back would be the
> two-structure confusion returning.

**F3 · Case A gets an honest label, not a removal.** A record's run has no
provenance, so under F2 the word stays live — correctly, because a record's
run genuinely is not a playlist file. What changes is that under F1 it is
now unmistakably about the run. Optionally (tier 2, §9) the label states its
subject: **`Save these 24 as a playlist`**, which is the one wording under
which Case A stops reading as *"save this CD, which is saved"*.

**Cost**: `save_control` gains one argument (the run's saved-ness), which
`PlayerState` can already answer from `queue_provenance()` plus the undo
history the strip already consults for `can_undo`. No new state.

---

## 2. What they are, which is where an honest signal comes from

| | a record | a playlist |
|---|---|---|
| where it came from | **found** — derived from files someone else authored | **made** — by you |
| can you change it | no | yes; it is the point |
| its order | the artist's | yours |
| its cover | one image, authored | a collage of quotations, generated |
| its name | the **work's title** | a **label you typed** |
| what it is made of | tracks | **references to tracks, which may go missing** |

A signal that expresses *made by you* is honest. A signal that just means
*different* is decoration, and this product has a standing law against
decoration in exactly this place — *"an archive is rectilinear and a sleeve
has square corners… **Artwork is radius 0 always**"* (`theme.rs:1481-1484`),
with `RADIUS_TILE` **deleted** (`theme.rs:2449-2451`).

The last row of that table is the one nobody has spent yet, and it is the
richest: **a playlist's name is a string a person typed, and every other
string a person typed in this product is already set in the sans.** The
search query, the rename field, the folder path. A record's title is a
*work's* title, and the product has a face reserved for exactly that
(§5.2).

---

## 3. Where the confusion bites: an inventory, with frames

### 3.1 The returns lane's `RECENT` — records and playlists interleaved

The two kinds are **deliberately mixed** in one column, sorted by touch:
`lane.rs:155-177`, pinned by
`the_lane_is_last_touched_first_and_mixes_the_two_kinds` (`lane.rs:219-235`).
They are carried in **one struct** — `lane::Touched { subject, name, under,
at }` (`lane.rs:129-140`) — so there is no field a view could branch on but
`Subject::Record(u64) | Subject::Playlist(u64)` (`lane.rs:83-89`).

`lane_row` (`views/lane.rs:557-671`) branches **twice** in 114 lines:

- `:574-588` — which sleeve source (thumb vs `playlist_sleeve`)
- `:589-592` — which message (`AlbumClicked` vs `OpenPlaylist`)

Everything else is one code path: `SIDEBAR_SLEEVE` 48 (`:572`, `:586`),
`SIZE_BODY` 13 / `LEADING_BODY` / `theme::MEDIUM` / `room.paper` for the name
(`:613-618`), `SIZE_META` 12 / `room.paper_faint` for the line under
(`:620-624`), `GAP_XXS` 2 between them (`:626`), `GAP_SM` 8 sleeve-to-text
(`:631`), `SIDEBAR_ROW_H` 64 (`:648`), one hover style (`:653`).

The **one** mark that differs is the lamp dot, and it is reserved to records
by construction (`views/lane.rs:138`) — *"A list is never 'the sounding
record' however many of its tracks are in the run"*. It marks *what is
sounding*, not *what kind this is*, so it is unavailable as a signal.

**The two lines under the name, verbatim:**

| | string | built at |
|---|---|---|
| record | `Anne-Marie Puig` — the album-artist label, bare | `app.rs:5160-5171` |
| playlist | `14`, or `12 · 42:10` when `#EXTINF` declared times | `app.rs:2530-2537` → `playlists.rs:125-134` |

**A bare integer.** At `SIZE_META` 12 in `paper_faint`, 176 px from the
sleeve, `14` does not read as a count — it reads as an artist's name that
has been truncated to nothing. This is the weakest string in the product and
it is sitting in the exact slot where the disambiguation should happen.

**Frame** — `docs/design/impl/home-stats/01-lane-at-rest-1280.png`:

```
 x=24                              x=252
  ┌──────────────────────────────────┐
  │ RECENT                           │  y=214
  ├──────────────────────────────────┤
  │ [cover ] ● Ochre                 │  y=244  ← record
  │ [ 48×48] Anne-Marie Puig         │  y=264
  │ [cover ]   Werkbund              │  y=308  ← record
  │ [      ]   Studio Hain           │  y=328
  │ [cover ]   Basalt                │  y=372  ← record
  │ [      ]   Ini Kovac             │  y=392
  │ [cover ]   Verdigris             │  y=436  ← record
  │ [      ]   Sonja Aalto           │  y=456
  │ [2 × 2 ]   Road Trip             │  y=500  ← playlist
  │ [collag]   14                    │  y=520
  │ [2 × 2 ]   Sunday Morning        │  y=564  ← playlist
  │ [collag]   7                     │  y=584
  │ [2 × 2 ]   Late Shift            │  y=628  ← playlist
  │ [collag]   22                    │  y=648
  │ [2 × 2 ]   Long Drive            │  y=692  ← playlist
  └──────────────────────────────────┘
```

In *this* frame the collage happens to work: the fixture's playlists all
hold four or more distinct records, so the 2 × 2 reads. §4.1 is about the
case where it does not — which is the case `Save as playlist` creates.

And in `docs/design/impl/lane-and-home/04-album-page-1280.png`, taken on a
machine whose thumbnails had not decoded, the three playlists show the rest
tile and the record shows its cover — so the kinds are distinguishable there
**by accident of missing artwork**, which is not a design.

### 3.2 Home

`views/home.rs` has three sections — `CONTINUE` (`:260-369`),
`RECENTLY ADDED` (`:525-550`), `COLLECTION` (`:603-622`) — and **playlists
are explicitly refused from all three** (`views/home.rs:59-61`): *"Refused
from the page and still refused: recently played and playlists, which are
the returns lane's content one column to the left."*

So Home is not confused **today**. It is about to be. Two things are in
flight:

- the owner's ask *"I wanted the Play all to be more like a tile on the home
  screen, a special 'playlist'"* (`BACKLOG.md`, **building**) — an
  **implicit** list rendered as a tile, in a section of records;
- `RECENTLY ADDED` already draws **the wall's own tile function verbatim**
  (`views/home.rs:541-543` → `views/shelf.rs:890-1042`), so anything that
  joins it inherits an album tile exactly.

And the wall already has a precedent that proves the collage is not a
kind-signal: **an artist's tile wears a playlist's sleeve.**
`views/shelf.rs:1085-1090` calls `crate::views::playlist_sleeve(…)` inside
the album tile's exact geometry, and its own doc says *"The sleeve is the
collage, and it is **the** collage"* (`views/shelf.rs:1055-1062`). Three
different kinds of thing — record, artist, playlist — already share one
sleeve vocabulary on one wall.

### 3.3 The playlist panel

`views/playlist_panel.rs`, `PANEL_W` 340. Three row shapes, one anatomy:

| row | sleeve | name | line under | at |
|---|---|---|---|---|
| `All songs` (implicit) | collage, `PANEL_SLEEVE` 40 | `SIZE_BODY`/`MEDIUM` | `1284 records · 9902 songs · 84:12:07` | `:228-268` |
| `Queue` (implicit) | **none** | `SIZE_BODY`/`MEDIUM`/`paper_dim` | `62 · 5:05:39` | `:280-332` |
| a playlist | collage, 40 | `SIZE_BODY`/`MEDIUM` | `entry.counts()` → `14` | `:501-591` |

Note that `All songs` — the one list here that is neither found nor made —
is the only row whose line under **says what it is made of in words**. It is
the rule the other rows want.

### 3.4 The two pages, side by side

`docs/design/impl/drag/07-playlist-page-1280x860.png` and
`docs/design/impl/artist-page/01-album-page-with-the-breadcrumb.png`, both
1280 × 860:

```
        A PLAYLIST'S PAGE                    A RECORD'S PAGE
 ┌───────────────────────────┐        ┌───────────────────────────┐
 │ Playlist                  │ y=25   │ Anne-Marie Puig › Ochre   │ y=25
 ├───────────────────────────┤ y=48   ├───────────────────────────┤ y=48
 │ ┌───────┐  Road Trip      │ y=106  │ ┌───────┐  Ochre          │ y=106
 │ │       │  2 tracks·9:52  │ y=134  │ │       │  Anne-Marie Puig│ y=137
 │ │  320  │  ───────────────│ y=157  │ │  320  │  1999·12 tracks·│ y=161
 │ │       │  TRACKS         │ y=181  │ │       │  ───────────────│ y=189
 │ │       │  Orbits         │ y=204  │ │       │  TRACKS         │ y=208
 │ │       │  Kesh           │ y=224  │ │       │  1 Undertow 1   │ y=236
 │ └───────┘  1 Anhydrous 2  │ y=250  │ └───────┘  2 Marginalia 2 │ y=268
 │ [   Play   ]              │ y=436  │ [ Play album ]            │ y=436
 │  Queue Rename Delete      │ y=482  │  Add to playlist…         │ y=482
 └───────────────────────────┘        └───────────────────────────┘
```

They are the same page. Same aside width, same sleeve edge, same primary
button at the sleeve's width, same hero size, same section rule, same rows.

**And the difference is a subtraction, which is the finding of this
section.** The record's identity block is three lines; the playlist's is
two. Measured from the source:

| | album page | playlist page |
|---|---|---|
| line 1 | title, `SIZE_HERO` 28 / `SEMIBOLD`, clipped at `2 × LINE_HERO` — `album.rs:526-534` | name, **identical** — `playlist.rs:222-230` |
| line 2 | **artist, `SIZE_TITLE` 19 / `paper_dim`** — `album.rs:535-538` | *(absent)* |
| line 3 | `1999 · 12 tracks · 59:18 · FLAC · 16-bit · 44.1 kHz`, `SIZE_META` 12 — `album.rs:539-542` | `2 tracks · 9:52`, `SIZE_META` 12 — `playlist.rs:207-213` |
| block height | 32 + 4 + 24 + 4 + 16 = **80 px** | 32 + 4 + 16 = **52 px** |

**This is the whole of *"the playlist name isn't really prominent"*.** The
name is already the same 28 px `SEMIBOLD` as an album title. It does not
*read* as prominent because (a) its neighbour is a 320 px image, and (b)
unlike the record's, it is followed immediately by a 12 px count line, so it
terminates after 52 px and reads as a stub rather than a placard. The
record's title is given a 19 px line of support; the playlist's is given
none.

The slot the playlist page is missing is the **byline** — and the byline is
precisely where *made by you* belongs.

Note also that the playlist page's counts line and the album page's meta
line share a vocabulary exactly: `2 tracks · 9:52` is
`1999 · 12 tracks · 59:18 · FLAC · 16/44.1` with the found-thing facts
removed. Same noun, same separator, same ink. The playlist's line is a
**subset** of the record's, which is the page-level statement of the same
disease.

### 3.5 The place header already carries a kind word, in the wrong place

`views/playlist.rs:155` — `place_header("Playlist")`, drawn at
`SIZE_EMPHASIS` 15 / `MEDIUM` in the chrome strip (`views/mod.rs:287-294`).
The album page does **not** have a matching word: it leads with the
breadcrumb `Anne-Marie Puig › Ochre` (`album.rs:126`).

So a kind word exists, but it is (a) 58 px above the name, in the chrome,
where every other place puts *where you are*; (b) unmatched on the record's
side, so it reads as a route rather than a contrast; and (c) invisible at
the moment the eye is actually deciding, which is when it lands on the
28 px name.

> *Settled 2026-08-10 (one page, two subjects; ADR-0024 §A4.5).* Tier 1 gave
> the kind its home in the byline, and the chrome's copy then said the same
> thing twice, 58 px up and 4 px smaller. The strip leads with the **list's
> name** now, which is `views::place_header_led`'s own rule for a place whose
> subject changes and which the Album and Artist places already followed. The
> lead was also 12 px shorter than a record's, because a bare word is not a
> control — see §6's amendment.

---

## 4. Why they were built alike, and which premise has failed

This sameness was designed, twice, on purpose. Both decisions must be
engaged rather than overwritten.

**ADR-0024 §A1** built the collage: *"A playlist should be a visual object
with a sleeve, the way a record is"*, on the owner's own words *"I think
similar to Spotify a playlist would appear like a cd does."*

**ADR-0024 §A2** gave the playlist page the record page's two-column
arrangement and, crucially, the record page's **declared hierarchy**:

> *"the page… now holds the same declared hierarchy — **the work ≫ `Play` →
> the name → the rows** (law L6)"*

and closed with the sentence this study is here to amend:

> *"a playlist that looks like a record wherever a playlist appears"*

**ADR-0030 §2 / `views/lane.rs:550-556`** refused a mark in the lane:

> *"**Nothing marks which kind a row is**, because the sleeve already does:
> a record wears its cover, a playlist wears the 2 × 2 collage of the
> records it quotes (ADR-0024 §A1). That is what makes a mixed list read as
> one list rather than as two lists sharing a column."*

Every one of these was right about something and the family they made is
worth keeping. But the lane's refusal rests on a **premise about the code**,
and the premise is false in the commonest case.

### 4.1 The sleeve is not the signal it was designed to be

`views/mod.rs:176-224`, the deciding match:

```rust
match art {
    [] => { /* the rest tile */ }
    [a, b, c, d, ..] => { /* 2 × 2, each cell edge / 2.0 */ }
    // Below four distinct records the first one's face is the sleeve —
    // one rule at every size, and the tiling question never opens.
    [first, ..] => sleeve_cell(shelf, *first, edge),
}
```

| distinct member records | what is drawn |
|---|---|
| 0 | the rest tile (plinth + name) |
| **1, 2, 3** | **one record's cover, full-bleed at `edge`** |
| 4+ | four cells at `edge / 2` |

And `sleeve_cell` (`views/mod.rs:230-238`) is *byte-for-byte the widget a
record's own row builds* (`views/lane.rs:574-579`): same `thumbs.peek`, same
`iced_image` at the same edge, same `gradient_block` fallback. So for one to
three records, a playlist's sleeve is not *similar to* a record's cover — it
**is** a record's cover, from the same cache, at the same size.

ADR-0030 §2's *"the sleeve already does"* is therefore true for playlists of
four or more records and false for the rest — and the rest includes:

- every playlist made by `Save as playlist` from a CD (§1.3 Case A): exactly
  one record, by construction;
- every playlist a person is part-way through building, which passes through
  one, two and three on its way to four;
- `Road Trip` in the shipped frame `docs/design/impl/drag/07-playlist-page-1280x860.png`
  — two tracks from one record, `Orbits` by `Kesh`, whose 320 px page hero
  is that record's cover.

§A1's argument for the full-bleed single is good and survives — every 2- or
3-way tiling *is* a layout with an opinion, drawn differently at 40 px and
320 px. What does not survive is the **load** placed on it. The sleeve can
stay exactly as designed; it just cannot be the only thing carrying the
distinction.

### 4.2 What §A2 imported that does not transfer

*"the work ≫ `Play` → the name → the rows"* is the album page's hierarchy
and it is correct there: **the sleeve is the work**, and the title is its
caption. On a made list it does not transfer, for a reason that is about the
objects rather than about taste:

> **A record's cover is about the record. A playlist's collage is about its
> contents.**

The collage is a picture of the rows — four quotations from things further
down the page. It is *evidence for* the list, not an image *of* it. Ranking
it above the name declares that a made thing's subject is its contents,
which is the opposite of true: the contents change constantly (that is the
point, §2 row 2) and the **name is the only stable fact about it**.

So the two pages should share the **arrangement** and not the
**hierarchy**. §6 settles that.

---

## 5. The axes, weighed

### 5.1 The collage — keep, demote

Retain §A1 unchanged (including the full-bleed single, for §A1's own good
reason), and demote it from *the* signal to *a* signal. Recorded rather than
assumed, because ADR-0030 §2 currently assumes the opposite.

One variant is worth putting to the owner rather than deciding here (§9,
tier 3): **below four distinct records, draw the rest tile instead of the
record's face.** It makes the sleeve honest at every count — a playlist
either quotes visibly or says its own name on a plinth, and never borrows a
cover it does not own. The cost is real and aesthetic: a two-record playlist
loses the prettiest sleeve available to it, at 320 px on its own page. The
owner's hard rules are responsiveness and aesthetics, so this is his call,
not mine.

### 5.2 Typography — the strongest axis, and it is half-built

`theme::WORK_TITLE` (`theme.rs:1202-1219`) is IBM Plex Serif **Italic**,
chosen as *"the museum-placard convention, where the work's title is italic
and every fact around it (the artist, the date, the medium) is not"*, marked
*"The typographic risk, seen and approved by the owner (2026-08-09)"*.

It is spent on **one string**: the album title in Home's `CONTINUE` placard,
`views/home.rs:337`. Locked there by `the_serif_is_the_work_titles_and_nothing_else`
(`theme.rs:4182-4238`), whose assertion is an **equality**, not a
`contains`:

```rust
assert_eq!(users, ["views/home.rs"], …);
```

so adding the token to any other view fails the build today. `views/now_playing.rs:64-70`
argues in prose that the serif stays on the placard.

> *Shipped 2026-08-10 (tier 2 #6).* The assertion is now an enumeration of
> **two** — `views/album.rs` and `views/home.rs`, sorted — and it stays an
> enumeration: the risk the token carries is not that the serif is used, it is
> that it *spreads*, one surface at a time, with no single change big enough to
> argue about. `now_playing.rs`'s prose was amended with it; what its argument
> got right is §5.2's own worry, and what it got wrong was drawing the boundary
> as a **count of placards** rather than as a rule about strings.

**Why this is the honest axis and not decoration.** The distinction print
has drawn for two centuries is exactly ours:

- **a record's title is a work's title** → italic, because it names a
  published work someone else authored;
- **a playlist's name is a label you typed** → roman sans, the same face as
  every other user-authored string in this product — the search query, the
  rename field, the folder path in `DETAILS`.

It maps onto the honest axis (§2, last row) with nothing left over. It costs
zero pixels, zero chrome, zero new tokens, and it survives at every size:
`Road Trip` in sans beside `Ochre` in serif italic is legible as *two kinds
of string* at 48 px in the lane and at 28 px on the page.

**Where the line has to be drawn, precisely.** *"A work's title"* cannot
mean *"any title"*, or the run column's track rows take the serif and the
axis is destroyed. The defensible rule, and the one this study proposes:

> **The serif italic sets an album's title, and only an album's title.** Not
> a track's, not an artist's, not a playlist's, not a place's. baz is an
> album-oriented product (`VISION.md`) — the album is the work here; a track
> is a part of one, an artist is a person, a playlist is a label.

*Amended in the building (tier 2 #6).* That is not quite enough on its own:
`views/now_playing.rs` prints the sounding track's album under it, and that is
an album's title. The clause the code needed, and the rule as shipped:

> **…on the surface whose subject that album is.** Where an album's title
> appears as a *fact about something else* — the line under the sounding
> track, a group header in a run — it stays roman, because a placard sets the
> title in italic and every fact around it in roman.

Enumerable, and it stays one token. Three risks, named:

1. **Sixty italic serif captions on the wall** is a real aesthetic risk and
   the owner's own hard rule. Tiered accordingly (§9): pages first, wall to
   him.
2. **Legibility at `SIZE_BODY` 13** in the lane row — an italic serif at
   13 px in `paper` on `recess` is a genuine question, answerable only from
   a frame.
3. **The revert property weakens**: the token's own doc promises *"one line
   to revert"*, which an enumerated allow-list of four call sites does not
   quite keep. Mitigated by keeping it one token and making the test an
   enumerated list rather than an equality on a single file — the promise
   becomes *one token to revert*, which is still the thing that matters.

### 5.3 A kind word in the line under the name — the cheapest relief

The line under the name is **already** the disambiguating slot and it is
already carrying different *sorts* of statement — a proper noun for a
record, a quantity for a playlist. It is just not saying so. Give it the
noun:

| | today | proposed |
|---|---|---|
| record, lane | `Anne-Marie Puig` | *unchanged* |
| playlist, lane | `14` | **`Playlist · 14 · 42:10`** |
| playlist, panel | `14` | **`Playlist · 14 · 42:10`** |
| implicit (`All songs`) | `1284 records · 9902 songs · 84:12:07` | *unchanged* — already says what it is |
| record, wall tile | `Anne-Marie Puig · 1999` | *unchanged* |

Stated as one rule rather than three cases:

> **The line under a name declares its kind in its first token** — an
> artist's name for a found thing, the word `Playlist` for a made one, a
> scale statement for an implicit one.

Zero geometry change: it is a different string in the same `SIZE_META` text
widget, and at 176 px of measure in the lane
(`SIDEBAR_MEASURE` 232 − `SIDEBAR_SLEEVE` 48 − `GAP_SM` 8) a 21-character
line at 12 px fits with room. The change is four lines in
`playlists.rs:125-134`.

This is what §9 puts first, because it is the only proposal that reaches
**every** confused surface — lane, panel, and any future tile — at once,
with no typographic risk and no new widget.

### 5.4 The byline line on the page — what fixes *"not prominent"*

Restore the line the playlist page is missing (§3.4), in the album page's
artist slot, at the album page's artist size:

```
Road Trip                    ← SIZE_HERO 28 / SEMIBOLD / paper
Playlist · 4 records         ← SIZE_TITLE 19 / paper_dim   ★ new
14 tracks · 52:31            ← SIZE_META 12 / paper_faint
```

against

```
Ochre                        ← SIZE_HERO 28 / WORK_TITLE (§5.2)
Anne-Marie Puig              ← SIZE_TITLE 19 / paper_dim
1999 · 12 tracks · 59:18 · FLAC · 16-bit · 44.1 kHz
```

Three consequences, all good:

1. The identity block goes 52 px → **80 px, exactly the record's**. The two
   pages become geometrically identical and differ *in what the middle line
   says* — which is the correct place for the difference to live.
2. The name stops reading as a stub. This is the answer to *"not really
   prominent"* and it does not require a type size above `SIZE_HERO`, which
   the ramp does not have and which would be a real ramp change
   (`theme.rs:845-849`).
3. `Playlist · 4 records` also **explains the collage above it** — it tells
   you the picture is quotations from the things below.

   *Corrected in the building (tier 2 #7).* This paragraph originally read
   *"the picture is made of four things, at the moment you are looking at four
   things"*, which is where the mistake in §9's costing came from: it treats
   the byline's number as **the collage's** count. It is not — it is the
   list's. The two coincide only for a playlist of exactly four distinct
   records; for `Road Trip` (fourteen tracks, twelve records) the collage
   quotes four of twelve, and the byline says twelve.

**Wording, and why not *"Made by you"*.** ADR-0024 §4 and the panel's own
empty state admit `.m3u8` files *dropped into the playlists folder*, which
this product did not author. `Made by you` would be a lie for those, and the
file records no author. `Playlist · 4 records` claims only what the file can
prove. (`Playlist` alone is the minimal form and is what tier 1 ships;
`· 4 records` is tier 2.)

### 5.5 Shape and corner — declined, with the reason

Rounding a playlist's sleeve, or matting it, or giving it a distinct
silhouette, contradicts a stated law — *"Artwork is radius 0 always"*
(`theme.rs:1481-1484`), `RADIUS_TILE` deleted (`theme.rs:2449-2451`), `fn
tile` returning `radius: 0.0` (`theme.rs:2453-2464`) — and it is decoration
by §2's test: a rounded corner means *different*, not *made*. Declined here
so that it is on the record as considered rather than missed.

### 5.6 An icon or badge on the tile — declined

A glyph over a sleeve breaches *"nothing is ever drawn on top of a sleeve"*,
which ADR-0024 §A1 went out of its way to preserve when it built the collage
*out of* quotations rather than *over* them. Declined for the same reason,
and because a badge is the answer that would have made the owner's own
sentence about hierarchy true without fixing it — a mark you must learn is
not a hierarchy.

---

## 6. The pages: share the arrangement, not the hierarchy

**Should the two pages share an anatomy at all? Yes — the arrangement. No —
the hierarchy.**

**Keep, from ADR-0024 §A2** — the two-column arrangement, the aside fixed at
`ALBUM_ASIDE_W` 320 so the blocks share one lane (law L5,
`album.rs:208-212`), `Play` under the object at the sleeve's whole width,
the stack below `ALBUM_BREAKPOINT` 744. This was right: two arrangements for
two nearly identical jobs would be two vocabularies, and the family the
owner asked for in the first place is worth keeping.

> **Amended 2026-08-10 — *one page, two subjects*.** This section said *share
> the arrangement*, and §3.4 measured the two pages against each other and
> found them the same. Both were right about the design and wrong about the
> code: the arrangement was **two copies**, written weeks apart, and this
> study compared them by reading them rather than by making them one. The
> owner named it — *"right now they are different but for no good reason"* —
> and the arrangement is now `crates/baz/src/views/page.rs`, one function the
> two pages hand what differs (ADR-0024 §A2 as amended; §A4.5 for the strip's
> lead).
>
> **Two divergences this study missed, both found in a frame** rather than in
> the source, at [`impl/one-page-two-subjects/`](impl/one-page-two-subjects/README.md):
>
> 1. **The quiet act hung from two lanes.** §3.4's table compared the identity
>    blocks and the rows and never looked at the aside below `Play`. A
>    record's `Add to playlist…` was a *centred, full-width* box at
>    `paper_dim`; a playlist's three acts were *natural-width* words at
>    `paper`. Leftmost ink x = 115 against x = 12.
> 2. **A playlist's whole page rode 12 px higher than a record's.**
>    `TOP_BAR_H` is `2 · TOP_BAR_PAD_V + TRANSPORT_HIT + 1` = 49, but the
>    shared strip lays out whatever lead it is handed, and §3.5's *"a kind
>    word exists, but it is in the wrong place"* was reading a lead that was a
>    bare 20 px word where the record's is a 32 px control. Sleeve top y = 88
>    against y = 77.
>
>    **Tier 1's own frames could not have shown it**, and this is the
>    methodological finding: §5.4 and tier 1 cropped each identity block out
>    of *its own* page and stacked the crops, which compares **shapes**. The
>    80 px claim was true and is still true. Two blocks of one shape at two
>    positions look identical in that pair of crops and are 12 px apart on
>    screen. A crop taken at the *same window coordinates* is what turns a
>    shape into a position, and it is what this branch's harness does.

**Change** — §A2's imported declared hierarchy. Restated per kind:

| | a record's page | a playlist's page |
|---|---|---|
| declared hierarchy | **the work ≫ `Play` → the title → the rows** (unchanged) | **the name ≫ `Play` → the collage → the rows** |
| why | the sleeve *is* the work; the title captions it | the collage is *evidence about the rows*; the name is the only stable fact |
| the hero | the album's title, `SIZE_HERO`, **serif italic** (§5.2) | the list's name, `SIZE_HERO`, **sans SEMIBOLD** |
| the byline slot | the artist | **`Playlist · 4 records`** (§5.4) |

The collage stays at 320 in the aside. Demoting it in the *declaration* does
not mean shrinking it in the *layout*: at 744–1280 the aside's width is what
lets the aside's three blocks share an x-edge, and shrinking the sleeve
inside a 320 lane would break law L5 for no gain. What the demotion buys is
the byline line and the type change, which is where the reading actually
happens.

### 6.1 Drawn: the playlist page at 1280 × 860, lane open

Derived from `views/playlist.rs:236-247`, `views/mod.rs:347-354` and
`theme.rs`: body = 1280 − `SIDEBAR_W` 280 = **1000**; content = 1000 −
2·`HANG` 40 − `SCROLLBAR_LANE` 10 = **910**; `side_by_side` since 1000 ≥
`ALBUM_BREAKPOINT` 744; measure = (910 − `ALBUM_ASIDE_W` 320 − `GAP_XL` 24)
clamped to `LIST_MEASURE` 880 = **566**.

```
 0        280                                                    1280
 ├─ lane ──┤                                                       │
 │         │◄─40─►┌───────────────┐◄24►┌───────────────────────┐◄─50─►│
 │         │      │               │    │                       │      │
 │  RECENT │ y=48 ├───────────────┤    ├───────────────────────┤      │
 │         │      │               │    │ Road Trip             │ 28   │  y=88
 │         │      │   collage     │    │ ───────────────────── │      │  y=120
 │         │      │   320 × 320   │    │ Playlist · 4 records  │ 19 ★ │  y=124
 │         │      │               │    │ 14 tracks · 52:31     │ 12   │  y=152
 │         │      │               │    ├───────────────────────┤      │  y=168
 │         │      │               │    │ ─────── hairline ──── │      │  y=192
 │         │      │               │    │ TRACKS                │ 10   │  y=216
 │         │      └───────────────┘    │  1  Anhydrous 2       │      │  y=240
 │         │ y=408      ▲              │  2  Nightwatch 3      │      │
 │         │ y=420 ┌────┴──────────┐   │                       │      │
 │         │       │    ▶  Play    │32 │                       │      │
 │         │ y=452 └───────────────┘   │                       │      │
 │         │ y=464  Queue Rename Delete│                       │      │
 │         │                           │                       │      │
 └─────────┴──── x=320 ────── x=640 ───┴ x=664 ───────── x=1230 ┴──────┘
                  │◄─── 320 ────►│◄24►│◄────── 566 ─────────►│
```

★ is the one added widget. Everything else is today's geometry: the
identity block grows 52 → 80 px and the section rule below it moves down
28 px. Compare the record's page at the same size, which is unchanged except
that its hero is set in `WORK_TITLE` (tier 2):

```
 │         │      │   cover       │    │ Ochre                 │ 28 serif italic
 │         │      │   320 × 320   │    │ Anne-Marie Puig       │ 19
 │         │      │               │    │ 1999 · 12 tracks · …  │ 12
```

### 6.2 Drawn: the playlist page at 1920 × 1080, lane open

body = 1920 − 280 = **1640**; content = 1640 − 80 − 10 = **1550**;
measure = (1550 − 320 − 24) = 1206 → **clamped to `LIST_MEASURE` 880**; page
width = 320 + 24 + 880 = **1224**, centred in the 1550 available
(`align_x(Center)`, `playlist.rs:158-161`), so 163 px of air either side.

```
 0        280      443            763  787                  1667      1920
 ├─ lane ──┤◄─40─►│◄─163 air──►│      │                       │◄163►│◄50►│
 │         │      ┌────────────┐ ◄24► ┌───────────────────────┐      │
 │  RECENT │      │  collage   │      │ Road Trip             │ 28   │
 │         │      │  320 × 320 │      │ Playlist · 4 records  │ 19 ★ │
 │         │      │            │      │ 14 tracks · 52:31     │ 12   │
 │         │      │            │      │ ───── hairline ────── │      │
 │         │      │            │      │ TRACKS                │      │
 │         │      └────────────┘      │  1  Anhydrous 2       │      │
 │         │      [    Play    ]      │  2  Nightwatch 3      │      │
 │         │       Queue Rename Delete│                       │      │
 └─────────┴──────┴────────────┴──────┴───────────────────────┴──────┴────┘
                  │◄── 320 ───►│◄24►│◄──────── 880 ─────────►│
```

The measure clamps at 880 and the page centres, so **the identity block is
the only thing that changes with width** — the drawing at 1920 is the
drawing at 1280 with more air, which is the property that makes the added
line safe at every size.

### 6.3 Drawn: the run column's strip after the `Save as playlist` fix

At 1280 × 860, lane open, run standing: body = 1000, `run_w` = 440 since
1000 ≥ `SPLIT_FLOOR` 784 (`now_playing.rs:98-105`), record column =
1000 − 80 − (440 + 24) = 456. Verified against
`docs/design/impl/queue-merged/01a-run-on-1280x860.png`: sleeve x 320 → 776
= 456, run column x 800 → 1240 = 440.

```
                                        x=800                       x=1240
 A · a record's run, unsaved              ┌────────────────────────────┐
                                    y=105 │ Run · 1 of 24 · 1:56:19    │
                                          │ left    Undo   Save as     │
                                          │                 playlist   │
                                          ├────────────────────────────┤
                                    y=162 │ Ochre                      │
                                    y=182 │ Anne-Marie Puig            │

 B · a run reified from a file, unedited  ┌────────────────────────────┐
                                    y=105 │ Road Trip · 1 of 14 ·      │
                                          │ 52:31 left       Saved as  │
                                          │                “Road Trip” │  ← readout
                                          ├────────────────────────────┤

 C · the same run, after one edit         ┌────────────────────────────┐
                                    y=105 │ Road Trip · 1 of 14 ·      │
                                          │ 52:31 left  Undo  Save as  │
                                          │              new playlist  │  ← live
                                          ├────────────────────────────┤
```

At 440 px of measure the strip is tight — `1 of 24 · 1:56:19 left` plus
`Undo` plus `Save as playlist` already nearly fills it in the shipped frame.
Adding `Run · ` costs ~34 px at `SIZE_META`; `Save as new playlist` costs
~48 px more than `Save as playlist`. Both fit at 440 with the `Space::Fill`
between them absorbing the difference, but this is the one measurement in
this study that wants a frame before it ships, and the note is here so it is
not discovered late.

---

## 7. The lane and the tiles, drawn

`SIDEBAR_W` 280 = `GAP_XL` 24 + `SIDEBAR_MEASURE` 232 + `GAP_XL` 24
(`theme.rs:1058-1063`). Row = `SIDEBAR_ROW_H` 64 = `SIDEBAR_SLEEVE` 48 +
2·`GAP_SM` 8 (`theme.rs:1082`). Text block = 232 − 48 − 8 = **176**; name
`LINE_BODY` 20 + `GAP_XXS` 2 + under `LINE_META` 16 = 38, centred in 64.

```
 x=24                                              x=252
  ├────────────────────── 232 ──────────────────────┤
  │ RECENT                                          │  SIZE_HEADING 10
  ├─────────────────────────────────────────────────┤
  │ ┌──────┐ ◄8► ●  Ochre                           │  13 / MEDIUM  ← serif
  │ │ 48×48│         Anne-Marie Puig                │  12 / faint     italic
  │ └──────┘                                        │                 (tier 3)
  ├─────────────────────────────────────────────────┤  64 px pitch
  │ ┌──────┐        Road Trip                       │  13 / MEDIUM  sans
  │ │collag│        Playlist · 14 · 42:10           │  12 / faint   ★
  │ └──────┘                                        │
  ├─────────────────────────────────────────────────┤
  │ ┌──────┐        All songs                       │  13 / MEDIUM  sans
  │ │collag│        1284 records · 9902 songs       │  12 / faint
  │ └──────┘                                        │
  └─────────────────────────────────────────────────┘
        │◄─48─►│◄8►│◄────────── 176 ──────────────►│
```

★ is the whole of tier 1 at this surface: one string, no geometry. The three
rows now read as three kinds by their second line alone — a person, a made
list, a scale — which holds when the collage does not (§4.1) and holds when
the thumbnails have not decoded.

The same rule on a wall/Home tile, where the caption has two reserved lanes
of `CAPTION_LINE_H` 20 inside `CAPTION_H` 40 (`views/shelf.rs:977-1004`):

```
 ┌───────────────────┐   ┌───────────────────┐   ┌───────────────────┐
 │                   │   │                   │   │                   │
 │   cover, 272 sq   │   │  collage, 272 sq  │   │  collage, 272 sq  │
 │                   │   │                   │   │                   │
 └───────────────────┘   └───────────────────┘   └───────────────────┘
   Ochre                   Road Trip               All songs
   Anne-Marie Puig · 1999  Playlist · 14 · 42:10   1284 records · 9902 songs
   └ found ┘               └ made ┘                └ implicit ┘
```

At `Density::Balanced` the art runs 240–320 with `HANG` 40
(`shelf.rs:174-176`); 272 is `ART_TARGET`. The tile is unchanged — only the
second caption lane's string differs, which is what makes this rule
survivable when the All-songs tile lands.

---

## 8. Prior art: three products, three answers, one of them bad

All three face exactly this: a library that mixes found things and made
things in one grid, with generated covers on the made ones.

### 8.1 Spotify — the byline, and a shape that cannot be a sleeve

Spotify is baz's acknowledged source for the collage: ADR-0024 §A1 adopted
*"a 2 × 2 collage of the first distinct cover arts, falling back to a single
cover below four"* from it explicitly, on the owner's *"similar to Spotify a
playlist would appear like a cd does."*

What baz took the picture from and **not** the sentence under it. Spotify's
detail page runs a kicker (`Album` / `Playlist`), then the name, then a
**byline row that names a person**: a circular avatar and an owner name,
then the counts. The circle is the load-bearing part — it is the one shape
in Spotify's vocabulary that is never a sleeve, so *made by someone* is
readable before you read a word. The repo's own survey already records this
column: `docs/design/03-interface-prior-art.md` §2.2 lists Spotify's
settings/identity affordance as **avatar**, against baz's own row in the
same table.

**What baz can take**: the byline slot (§5.4). **What baz cannot**: the
avatar, because baz has no accounts and no other user — a circle would be
decoration with nothing behind it, and §2's test refuses that. baz's byline
names the *kind and the composition* instead, which is what its files can
actually prove.

### 8.2 Apple Music — the one that solves it badly, and it is baz's exact disease

Apple's library grids — Recently Added above all — interleave albums and
playlists as **identical squares with two lines of caption**, where the only
difference is that an album's second line is an artist's name and a
playlist's is a curator, or nothing. Generated playlist art is a mosaic of
member covers, which collapses to a single cover for small playlists. There
is no kicker, no shape difference, and no type difference at grid scale;
the kind is only recoverable on the detail page.

That is, line for line, `views/lane.rs:557-671` plus `views/mod.rs:221-223`.
baz has independently reinvented the treatment whose failure mode the owner
just reported, and Apple's version is worse only in that its grids are
bigger. It is worth showing precisely because it is the *default* answer —
what you get when a good tile design is reused for a second kind of thing
without anyone deciding to.

### 8.3 Plexamp — segregation, which baz has already declined

As the survey records it (`03` §2.1), Plexamp's home is *"shelves of
recommendations"* — kind-labelled horizontal strips, with the cover wall one
level down under Library. The disambiguation is structural: a strip is
titled with what it contains, so a playlist is never adjacent to an album
without a heading between them.

**baz has already declined this**, on purpose and with a good reason:
ADR-0030 mixes the two kinds in `RECENT` because the lane's subject is
*what you touched*, and sorting by kind would make it two lists sharing a
column — the exact thing `views/lane.rs:550-556` says it is avoiding. The
segregation answer is cheap and it works, and it is the wrong trade here.
Recorded so the option is on file as weighed rather than missed; the
consequence of declining it is that the per-row signal has to do the work,
which is §5.3.

---

## 9. Proposals, ordered by relief

### Tier 1 — adopt · **shipped 2026-08-10**

All five, on `main`. Frames, the harness and the measured strip at
[`docs/design/impl/records-and-lists/`](impl/records-and-lists/README.md);
recorded in [ADR-0024](../adr/0024-playlists.md) §A3–§A5 and
[ADR-0030](../adr/0030-the-returns-lane-and-the-home-band.md)'s fifth
amendment.

**Two things this study got wrong, found in the building:**

1. **#1's *"zero geometry change"* claim is true, and #4's *"no new state"*
   claim is not.** §1.4 costed the save fix as answerable from
   `queue_provenance()` *"plus the undo history the strip already consults for
   `can_undo`"*. It is not: `App::queue_undo` is cleared when the place is
   left (`App::note_place_left`), when the run column stands down
   (`App::set_run`) and when the run ends (`Event::QueueEnded`) — all three
   are doc 11 §5 P2's ends for an *edit history*, and none of them un-edits a
   run. Reading saved-ness off it would have made an edited run claim to be
   its source file again after one navigation, which is the exact lie §A5 is
   removing. Divergence is a fact about the queue record, so it is kept there:
   one bool on `PlayerState`, written by the two calls (`note_queue_sent`,
   `note_queue_edited`) that already draw ADR-0014's line between a new run
   and an edit to the one sounding, and read by one method, `run_origin()`.
2. **#3's noun goes on the whole no-provenance branch**, not only on the
   cursor reading: before a run starts the strip says `Run · 24 tracks ·
   1:58:00` as well. The branch is the branch.

| # | change | where | relief |
|---|---|---|---|
| **1** | **The line under a name declares its kind in its first token.** `14` → `Playlist · 14 · 42:10`. | `playlists.rs:125-134` (`counts()`), reaching `views/lane.rs:620-624` and `views/playlist_panel.rs:534-538` unchanged | Fixes the lane and the panel at once. The only proposal that reaches every confused surface. Zero geometry, zero risk, ~4 lines. |
| **2** | **The playlist page gets its byline line**, `Playlist`, at `SIZE_TITLE` 19 / `paper_dim`, in the album page's artist slot. | `views/playlist.rs:216-234` (`identity_block`) | Identity block 52 → 80 px, matching the record's. This *is* the answer to *"the name isn't prominent"*. One widget. |
| **3** | **`Save as playlist` F1** — the run's strip names its subject: `Run · 1 of 24 · 1:56:19 left`. | `player.rs:2245-2271` (`queue_summary`), the `None` branch | The word beside it stops reading as a control of the record below it. One `format!`. |
| **4** | **`Save as playlist` F2** — provenance standing and no edit ⇒ the control is the readout `Saved as "Road Trip"`; after an edit ⇒ live, as `Save as new playlist`. | `views/queue.rs:337-353` + one argument from `queue.rs:246` | Kills the *"offering to save a thing whose name you are printing"* case. Follows `undo_control`'s own precedent (`queue.rs:311-318`). |
| **5** | **Record in ADR-0024 that the collage is no longer the sole signal**, and correct ADR-0030 §2 / `views/lane.rs:550-556`, whose stated premise is false for one-to-three-record playlists. | the amendment; `views/lane.rs:550-556` prose | Stops the next agent re-deriving *"the sleeve already says it"* from a comment. |

### Tier 2 — adopt with modification · **#6 and #7 shipped 2026-08-10; #8 declined**

Frames, the harness and the measured blocks at
[`docs/design/impl/serif-titles/`](impl/serif-titles/README.md); recorded in
[ADR-0024](../adr/0024-playlists.md) §A4.4 and §A4.3.

| # | change | modification, and why |
|---|---|---|
| **6** | **The serif italic on the pages**: the album page's hero title takes `theme::WORK_TITLE`, joining Home's placard. | Restrict to **pages and placards** — two call sites, `views/home.rs:337` and `views/album.rs:526-534`. Not the wall, not the lane (tier 3). The test `the_serif_is_the_work_titles_and_nothing_else` (`theme.rs:4223-4231`) changes from `assert_eq!(users, ["views/home.rs"])` to an enumerated list, and `views/now_playing.rs:64-70`'s prose must be amended in the same commit or the code argues with the ADR. |
| **7** | **The byline states the composition**: `Playlist` → `Playlist · 4 records`. | Needs the distinct-record count, which `playlists.rs:1208-1222` already computes for the sleeve — the same list, published. Ship after #2 so the added line is proven at both window sizes first. |
| **8** | **`Save as playlist` F3** — the label names its subject: `Save these 24 as a playlist`. | Only in the no-provenance case, and only if #3's `Run · ` prefix proves insufficient in a frame. A variable-length label in a 440 px strip is the risk (§6.3); measure before adopting. |

**Three things this study got wrong, found in the building:**

1. **#6's rule needed one more clause.** *"The serif sets an album's title and
   only an album's title"* (§5.2) does not exclude the `Ochre` that
   `views/now_playing.rs` prints under the sounding track — that *is* an
   album's title. The rule shipped as **the serif italic sets an album's
   title, on the surface whose subject that album is**. Now playing's subject
   is a moment in a **track**; the album under it is a *fact about it*, and
   the placard convention this whole idea comes from sets the title in italic
   and the facts around it in roman.

2. **#7's count cannot come from the sleeve's list.** §5.4 costed it as free
   from *"the distinct-record list `playlists.rs` already computes"*. That
   list stops at four, because four is all a 2 × 2 can quote — so the shipped
   frame's `Road Trip`, fourteen tracks from **twelve** distinct records,
   would have read `Playlist · 4 records` over a page listing twelve. A false
   byline in the slot this study exists to make honest. The distinct set is
   walked to its end instead (`OpenPlaylist::records`), and a list nothing
   resolves states `Playlist` and claims no count.

3. **#8 is declined, from the frame it was conditioned on.**
   `docs/design/impl/serif-titles/0d-strip-unfiled-1280x860.png` reads
   `Run · 2 of 12 · 55:00 left … Save as playlist`: the strip leads with the
   noun, and the run's own cursor — a reading no file has — sits between the
   subject and the word. Tier 2 also adds a second, independent statement that
   the record below is a different sort of thing, since its title is now set
   in a face no label can wear. Against that, §6.3 named the 440 px strip as
   *"the one measurement in this study that wants a frame before it ships"*,
   and `Save these 12 as a playlist` is **variable-length** — a label that fits
   at twelve tracks and elides at 1284. Weighed and declined, not missed.

### Tier 3 — present to the owner

| # | question | why it is his |
|---|---|---|
| **9** | **Should a record's title be set in serif italic everywhere it is named — the wall's tile captions and the lane's rows included?** | This is the strongest possible answer to his question: it makes every record in the product typographically a *work* and every playlist a *label*, at every size, with no badge. It is also sixty italic serif captions on a wall of covers, and his two hard rules are responsiveness and **aesthetics**. He approved the serif once, for one string; this is a different magnitude and needs his eye on a frame, not an argument. |
| **10** | **Should a playlist of one to three records show the rest tile instead of that record's cover?** (ADR-0024 §A1 rule 2) | It is the only change that makes the sleeve honest at every count, and it is the direct cure for the §0 loop. It also costs a two-record playlist the best sleeve it could have, at 320 px on its own page. A genuine aesthetic trade with no right answer from the code. |
| **11** | **Should `Save as playlist` offer at all when the run is exactly one record in its own order?** | Under tier 1 it stays, correctly labelled — the repo's rule is that what he asks for goes in the app, and this study does not propose a prohibition. But he is the one who noticed it, and if his intent was *"this should not be here"* rather than *"this should make sense here"*, that is a sentence only he can write. |

### Not proposed, and why

- **A badge or glyph on the sleeve** — breaches *"nothing is ever drawn on
  top of a sleeve"*, which §A1 preserved deliberately (§5.6).
- **A rounded or matted sleeve for playlists** — contradicts *"artwork is
  radius 0 always"* (`theme.rs:1481-1484`) and means *different* rather than
  *made* (§5.5).
- **Sorting `RECENT` by kind** — Plexamp's answer, declined by ADR-0030 for
  a reason that still holds (§8.3).
- **A type size above `SIZE_HERO` 28 for the playlist name** — the ramp ends
  there (`theme.rs:845-849`); the prominence problem is a missing line, not
  a small number (§3.4).
- **Writing an edited run back to its source file** — refused by ADR-0024
  amendment item 6 and ADR-0023 §3 (§1.4).

---

## 10. What this costs in tests

| test | file | what changes |
|---|---|---|
| `the_serif_is_the_work_titles_and_nothing_else` | `theme.rs:4182-4238` | `assert_eq!(users, ["views/home.rs"])` → an enumerated list. Tier 2/3 only. The second assertion — that nothing names `font::SERIF` directly — **stands unchanged**, which is what keeps the revert to one token. *Shipped as `["views/album.rs", "views/home.rs"]`, sorted, since the source walk is in filesystem order.* |
| `the_sounding_record_is_the_marked_row` | `views/lane.rs:961-979` | unaffected — the lamp is playback truth, not kind. |
| `the_lane_is_last_touched_first_and_mixes_the_two_kinds` | `lane.rs:219-235` | unaffected — the mixing stays; only the row's second line changes. |
| `the_summary_leads_with_provenance_until_a_new_run_replaces_it` | `player.rs:5183` | gains the `Run · ` prefix in its no-provenance assertions (tier 1 #3). |
| `the_summary_counts_down_what_is_left_rather_than_up_what_exists` | `player.rs:5313` | same — four `assert_eq!`s on the summary string. |
| `every_queue_affordance_survives_the_merge` | `views/now_playing.rs:766-838` | `Message::SaveQueueStart` must still be spent by `queue.rs` — tier 1 #4 keeps it, so this passes unchanged. **It would fail on a removal**, which is the guard that makes #4 the right shape of fix. |
| `the_sleeve_quotes_the_first_four_distinct_records_in_order` | `playlists.rs:1915-1960` | unaffected by tier 1; rewritten only if tier 3 #10 is taken. |
| `the_playlist_sleeve_sizes_hold_the_artwork_laws` | `theme.rs:6732-6741` | unaffected. |

New tests the changes want:

- **the line under a name declares its kind** — a source or unit assertion
  that `counts()` leads with `Playlist`, and that the record arm
  (`app.rs:5160-5171`) leads with the artist label. The property is *the two
  strings are never the same shape*, and it is the one thing a screenshot
  cannot check.
- **the identity block is the same height on both pages** — the geometric
  claim of §5.4, asserted over the two `column!` compositions the way
  `now_playing.rs`'s own `art_edge` tests are swept.
- **`Save as playlist` does not offer over a run that is a saved file** —
  the §1 defect, as a unit test over the predicate rather than the widget.

And two that tier 2 turned out to want, neither of them foreseen here, both
about the thing a frame **cannot** check — that the face in the frame is the
bundled one. `Font::with_name` is a string match, so a family spelling that
drifts by one character resolves silently against the host's fonts and looks
correct on the machine that shipped it:

- **the family names baz asks for are the names the faces spell** —
  `font.rs`, comparing `SANS` and `SERIF` against `name` record 16 of the
  bundled bytes, plus the serif's italic style bit. (Record 16, not record 1:
  record 1 is the legacy family and holds four styles, so Plex Sans Medium's
  reads `IBM Plex Sans Medm`.)
- **the serif face carries every letter an album title arrives with** — a
  title is other people's text, and a codepoint the face lacks falls back
  *per glyph*, setting half a title in a host font.

---

## 11. The one-line summary for `CHANGELOG`

> A record is a work you found; a playlist is a label you made. The line
> under a name now says which, `Save as playlist` says what it is saving,
> and a made list's page gets the byline line a found one always had.

---

## 12. 2026-08-12 amendment — one playlist, two persistence states

The distinction this study established still holds in words: a saved file is
durable and a run is transient. It no longer permits separate page anatomy.
The owner's same-size review found that the run's private composition had
drifted even though both states shared collage, identity and row primitives.

The saved detail page is now the reference. Both states enter
`views::playlist_page`, which owns the collage, 320 px sleeve/aside, playlist
breakpoint, responsive document, three-line identity, section rule, empty
state, scroller and fixed-pitch artwork/Album rows. The semantic distinctions
occupy slots: durable Play/Rename/Delete/counts versus transient
Save/provenance/cursor/remaining-time. The next ring remains because a run has
a cursor; it changes the marker's content, not the row's composition.

The before/after frames and drift inventory are
`docs/design/impl/one-playlist-page/`. This supersedes the earlier acceptance
of separate top-level compositions while preserving the data-model separation
and no-write-back rule.
