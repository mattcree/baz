# 13 — The everyday flow: a home, a returns lane, and putting things somewhere

> **The owner's first brief**, verbatim (voice-dictated, lightly punctuated):
>
> *"the way that playlists work as a sidebar, I don't hate it, but I don't
> really love it either. I wonder if there are just simply better ways to
> deal with the flow of, firstly, starting to play an album. I feel like
> that does should not need two clicks. I wonder if when we mouse over, we
> can just show two options somehow. […] I mean, I wonder should it be
> there's options that appear above the icon that you're hovering on? For
> example, send all to current playlist. or play now, which just starts to
> play the album, or just view details, which might take you into the
> screen that we have now. However, I think we should consider the way that
> information hierarchy works and use some elements of nesting. For example,
> when there's a master details view, often you can pop back up a level
> quite easily because you can already see the depth that you've went into
> with the options on the left. […] I think iDaily things like managing
> playlists, they just need to be more clear how it works. For example, when
> you click to add something to a playlist currently, it shows a playlist
> thing, and there's a very minor tip at the very top of the playlist window
> that indicates you need to click on a playlist to add it to it. I mean, it
> makes sense to some degree. But is there a better way to do it? these are
> the things that I think will finally decide how good this is."*
>
> **And his second, which arrived while this document was being written:**
>
> *"let's do the ground work for adding a home page and left hand side bar.
> the side bar will have recent albums and playlists mixed based on some
> order. we can collapse it into only an icon list. similar to Spotify"*
>
> **And the note that sets the bar:** *"listen I am the authority. we didn't
> have to have hard rules. hard rules to me are mostly about responsiveness
> and a nice aesthetic etc"*
>
> A design study, not an implementation. Written 2026-08-09 against `b795a06`
> (`c7e0f8c` for everything in `crates/`). Every claim about shipped
> behaviour is cited `file:line` or to a frame in
> [`impl/everyday-flow/`](impl/everyday-flow/), captured for this study on
> the real binary; every prior-art claim carries a named source. Its
> decisions are proposed as
> [ADR-0030](../adr/0030-the-returns-lane-and-the-home-band.md),
> [ADR-0031](../adr/0031-the-picker-at-the-pointer.md) and
> [ADR-0032](../adr/0032-the-walls-verbs-and-the-records-position.md).
>
> **The short version.** The two briefs are one request, and reading them
> together is the study's main finding: *"you can already see the depth that
> you've went into with the options on the left"* is a description of the
> sidebar the second brief asks for. So the spine is the **returns lane** —
> one resident left surface with one subject, *things you have touched* —
> and the **home band** it cannot carry: what you were in the middle of, and
> what is new. The lane kills the playlist panel outright (it does the
> panel's index job better and its drop-target job continuously), it answers
> the depth question by *being* the master pane, and it makes the record
> page's position readout the last thing depth needs. Three further
> decisions fall out: the picker leaves the panel for a **card at the
> pointer** (the owner's *"very minor tip"* is not a copy defect — it is a
> 682 px trip whose length is set by the window's width); the tile's context
> menu gains the one verb it lacks, **album-scope `Add to "{current}"`**;
> and hover-revealed verbs on the wall are declined, on geometry rather
> than on principle. The hardest engineering problem in the request is that
> a collapsing lane re-hangs the wall; §2.4 solves it, and the answer is
> that the collapse is the one press in the product that *cannot* break a
> gesture, because it is the only one that lands outside the wall.

---

## 0. What decides this

### 0.1 The bar the owner set

*"hard rules to me are mostly about responsiveness and a nice aesthetic
etc."* That is the standard this document is written to, and it is
narrower and sharper than the corpus's usual one. Concretely, for this
work:

1. **The lane costs nothing when nothing is happening.** No subscription,
   no tween, no per-frame file read, no watcher. ADR-0020's real clause —
   *anything requiring a redraw while the window is idle* — is the
   engineering form of "responsive", and §2.5 pays it in arithmetic.
2. **The collapse is immediate.** One frame, no wait, no animation to sit
   through. §2.4.
3. **The wall does not jank when it happens**, and no gesture in flight can
   be broken by it. §2.4 — this is the hardest part of the request and it
   has a structural answer rather than a mitigation.
4. **It looks excellent in both states, at every width.** Type, spacing and
   sleeves: §2.6, with the measured layouts in §9.

### 0.2 The craft tools, kept because they produce the bar

The composition laws, the 4 px lattice, the contrast floors and the motion
budget are kept in this document **as instruments, not as authorities**.
They are here because a lane whose rows land off the lattice looks wrong,
because a glyph under the contrast floor cannot be seen, and because a
motion budget is how you get a product that does not heat a laptop while
idle. Where one of them would produce a worse result it is named and
overruled in the open — §2.6 does this once, for the seam.

The ones that carry real weight below: **L1** (one window gutter, and a
surface's own content hangs `GAP_XL` from its own edge — the panel's
precedent, `views/playlist_panel.rs:175–177`); **L5** (each surface
declares its alignment edges); **L7** (one control height, `TRANSPORT_HIT`
32, with the named secondary `STEPPER_HIT` 24); **L8.1** (a control's home
is the surface whose subject it shares, where subject is what it must
consult to know what to do); **L8.6** (facts may be restated everywhere;
controls may not).

### 0.3 The reversal, recorded once

`docs/REFUSALS.md` says *"baz has no resident side surfaces, and no surface
that is a slot"*, and `11-jobs-era-critique.md` P10 declined to restore a
persistent left column partly because the owner had rejected sidebars
twice. **He has now asked for one, and that decision is his to make.** This
paragraph is the whole of the ceremony.

What is different this time, stated so the next reader knows it was
thought about rather than forgotten: the surface that was rejected twice
was a **slot** — a 340 px column that showed the selected album, then the
queue, then Settings, arbitrating between three unrelated tenants and
re-hanging the wall when a tile was pressed. The surface proposed here has
**one subject and no arbitration**: it always shows the same list, of one
kind of thing, in one order. The ledger entry is superseded by ADR-0030
rather than argued with; the five findings that killed the rail
(ADR-0024 §5) are used below as **engineering lessons**, which is now the
whole of their value:

| What killed the rail | How this lane avoids it |
|---|---|
| Three unrelated tenants | One subject — *things you have touched* — and one list (§2.1) |
| A paragraph of dismissal | One control, two states, one key (§2.4) |
| The wrong tenant paying resident width | Its tenants are the two most frequent workflows in the corpus: browse (W4, band A) and get back to what you were in (W12, band A) — `03` §1.2 |
| A gesture-breaking reflow | The collapse is the only press that lands outside the wall, so no wall gesture can be in flight (§2.4) |
| Arbitration state | None: there is nothing to arbitrate between (§2.1) |

**ADR-0022's foundational sentence changes**, and that is stated rather
than slipped in. *"The window holds one place at a time"* becomes:

> **The window holds one place at a time, with the returns lane to its left
> in every place but Settings, the index rail at the wall's right edge in
> Library as always, and the now-playing bar under all of them.**

### 0.4 What is on screen today

Read off the code and confirmed on the four frames captured for this study
([`impl/everyday-flow/`](impl/everyday-flow/), isolation receipt in its
README):

- **The wall's tile**: one object, one press — `AlbumClicked(album.id)`
  (`views/shelf.rs:1021`), shift held it queues instead
  (`app.rs:1054–1061`). Its whole hover vocabulary is a 1 px → 2 px rule
  under the label and one rung of ink on the artist line
  (`shelf.rs:972–976`), visible whole in frame `01`.
- **The tile's menu**: `Open · Play album · Queue album · Add to playlist…`
  (`menu.rs:233–264`), a `MENU_W` 232 card whose top-left corner is the
  pointer, edge-flipped (`menu.rs:359–372`). Frame `02`.
- **The playlist panel**: `PANEL_W` 340 plus a 1 px seam, right-aligned in
  a `stack` over the place, `opaque`, no scrim, wheel passing through
  (`app.rs:3198–3223`). It is three things at once — a directory of lists,
  the destination picker, and the drag's drop target. Frame `03`.
- **The record's page**: sleeve at `ART_MAX` 320, `Play album` a 320 × 32
  lamp-outlined glyph-and-word, `Add to playlist…` a bare word, and
  `‹ Prev` / `Next ›` in the header stepping `vm::neighbours` over the
  wall's visible order (`album.rs:156–191`, `vm.rs:1095`). Frame `04`.
- **The wall's width algebra**, which §2.4 turns on:
  `Shelf::grid_width` is `window_w − INDEX_LANE_W` and nothing else
  (`app.rs:4108–4110`, pinned by a source test at `app.rs:5083–5088`);
  `Grid::new` resolves columns, art, gutter and margin from that one number
  (`shelf.rs:355–385`). `INDEX_LANE_W` is 108 (`theme.rs:993`).
- **Not shipped, and load-bearing for §3**: ADR-0023 §6's queue snapshot.
  The ADR specifies that *"on exit the front end persists the queue's
  paths, the cursor and the elapsed position; on launch it restores them
  **paused** — nothing sounds unasked, and one press resumes"*, and closes
  prior art's W2. Nothing in `crates/` implements it. It is the single best
  thing a home surface could hold, and it does not exist yet.

---

## 1. The two briefs are one request

| # | The owner's words | What it turns out to be |
|---|---|---|
| **1** | *"starting to play an album… should not need two clicks"* · *"options that appear above the icon"* | A request for the record's verbs at the pointer. They already float there — that is the tile menu (frame `02`) — short one verb and one teacher. §4 |
| **2** | *"use some elements of nesting… you can already see the depth that you've went into with the options on the left"* | **This is the sidebar.** baz's navigation tree is one level deep everywhere, so there is no depth to draw; what he is describing is the master pane's other job, which the second brief asks for by name. §2, §5 |
| **3** | *"a very minor tip at the very top of the playlist window"* | Not a copy defect: a 682 px trip whose length is set by the window's width. §6 |
| **4** | *"playlists work as a sidebar, I don't hate it, but I don't really love it either"* | Answered by the second brief: the lane does the panel's jobs better, so the panel goes. §7 |
| **5** | *"a home page and left hand side bar… recent albums and playlists mixed… collapse it into only an icon list"* | The spine. §2, §3 |

Problem 2 and the second brief are the same sentence twice. That is why
this document leads with the lane instead of appending it.

---

## 2. The returns lane

### 2.1 One subject, and what that decides

> **The lane's subject is *things you have touched*: records you have
> played, and lists you have made or edited. Its order is when you last
> touched them. Nothing else is admitted, ever.**

The subject is what closes the slot. The dead rail could hold anything
because its subject was *"whatever needs a side"*; this lane's membership
is a predicate over the user's own actions, so there is no arbitration to
write and nothing to decide per frame. It also decides the questions the
brief asks:

- **Recent albums and playlists mixed?** Yes, and the mixing is not a
  compromise — it is the subject. A record you played on Tuesday and a list
  you edited on Wednesday are the same kind of thing under this predicate,
  and interleaving them by time is the only order that treats them as one
  kind.
- **Does the queue belong in it?** **No.** The queue's subject is
  *playback*, not *things you have touched* (doc 07 L8.1's four subjects).
  Admitting it would put a second, unrelated tenant in the lane on day one
  — finding 1, reproduced exactly. The queue keeps its labelled door in the
  bar, its ambient continuation line, and its place. Stated as a rule the
  lane inherits: **the lane never holds anything that is not an object you
  can open.**
- **Does Settings, or a nav list of places, belong in it?** No, for the
  same reason, and doc 07 L8.4 already refused a nav rail: three of baz's
  places have a subject you can only mean while looking at something
  specific.

### 2.2 What is in it, and the order

**Membership.**

- **Every playlist**, always. The lane is the *complete* index of lists —
  this is what lets the panel die (§7) without any list becoming
  unreachable.
- **The last `RECENT_ALBUMS` = 24 records you played**, newest first. 24 is
  chosen to be a scroll and a bit at the shipped window (§9 measures ten
  rows visible at 860 px), so the list is readable to its end — the same
  instinct as shuffle's bounded draw: *a run you can read to the end is a
  run you own.*

**Order: last touched, newest first.** "Touched" is defined exactly, and
every event in it is one the person caused:

| Row kind | Touched when | Read from |
|---|---|---|
| A record | a play of any of its tracks is recorded | the history ledger's append (ADR-0018) |
| A playlist | it is played, or its file is written by the user's own edit | the ledger, and the file's mtime (`playlists.rs`'s existing mtime discipline) |

Ties break by name ascending, so the order is **total and reproducible** —
two launches over the same data draw the same lane. There is no score, no
decay, no weighting and no blend: this is the anti-invisible-pool rule
(`REFUSALS.md`) applied to an ordering rather than to a pool, and it is the
difference between this lane and the surface Spotify puts in the same
pixels (§10.1).

**No pinning, at v1.** Spotify has it (§10.1) and it is genuinely useful,
and it is refused here for one reason: a pinned set is a second ordering
the list must arbitrate against the first, which is finding 5 arriving
through a feature request. Named in §11 so a re-proposal meets the reason.

**No sort control and no filter row.** The order *is* the design, and the
lane is short enough to read. `REFUSALS.md`'s view-options entry stands
here undisturbed: the alternative is a sort dropdown, which is the one
thing it names.

**What falls off the end**: the twenty-fifth-most-recent record. Nothing is
lost, because every record is on the wall and the wall is one press away —
which is the property that makes a bounded list honest.

### 2.3 The two widths

```
EXPANDED — SIDEBAR_W 280                    COLLAPSED — SIDEBAR_RAIL_W 96

┌──────────────────────────────┐            ┌──────────┐
│                              │  GAP_XL 24 │          │  GAP_XL 24
│  Recent                      │  20        │          │
│                              │  GAP_SM  8 │          │
│  ┌────┐                      │            │  ┌────┐  │
│  │ 48 │ Violet Ledger        │  64        │  │ 48 │  │  64
│  └────┘ Anne-Marie Puig      │            │  └────┘  │
│  ┌────┐                      │            │  ┌────┐  │
│  │ ▨▨ │ Road Trip            │  64        │  │ ▨▨ │  │  64
│  └────┘ 14 · 51:08           │            │  └────┘  │
│  ┌────┐                      │            │  ┌────┐  │
│  │ 48 │ ● Ochre              │  64        │  │ 48 │  │  64
│  └────┘ Anne-Marie Puig      │            │  └────┘  │
│                              │            │          │
│              ⋮               │            │    ⋮     │
│                              │            │          │
│  ▮▯                          │  24        │    ▮▯    │  24  ← the two marks
└──────────────────────────────┘  GAP_XL 24 └──────────┘  GAP_XL 24
 24 │ 48 │ 8 │ ── 176 ── │ 24               24 │ 48 │ 24
```

| Token | Value | Derivation |
|---|---:|---|
| `SIDEBAR_W` | **280** | `GAP_XL` 24 + lane 232 + `GAP_XL` 24. The content lane is `MENU_W` 232 — the product's existing float measure |
| `SIDEBAR_RAIL_W` | **96** | `GAP_XL` 24 + `SIDEBAR_SLEEVE` 48 + `GAP_XL` 24 |
| `SIDEBAR_SLEEVE` | **48** | one step above `PANEL_SLEEVE` 40, because in the collapsed state the sleeve is the *only* thing identifying the row |
| `SIDEBAR_ROW_H` | **64** | 48 + 2 × `GAP_SM`. Above L7's floor, and the two-line block (`LINE_BODY` 20 + `LINE_META` 16 = 36) sits centred in it |
| `SIDEBAR_FLOOR` | **1000** | the smallest window at which the expanded lane still leaves the wall two columns at or above `ART_MIN` 240 (988, rounded onto the lattice). Below it the lane is collapsed and the expand mark is inert |

The row's name column is 176 px, which holds about 24 characters at
`SIZE_BODY` 13 and clips with `Wrapping::None` — the wall label's own rule
(`shelf.rs:962–968`), so a long title fails the way it already fails
everywhere else in the product rather than in a new way.

**Both kinds of row have one anatomy**: a sleeve, a name, and one quiet
line under it. For a record the quiet line is the album artist; for a
playlist it is the counts (`14 · 51:08`, `PanelRow::counts`,
`playlists.rs:108–118`). **Nothing marks which kind a row is** — no badge,
no icon, no section header — because the sleeve already says it: a record
wears its cover, a playlist wears the 2 × 2 collage of the records it
quotes (ADR-0024 §A1). That is the aesthetic answer to *"mixed"*, and it is
the reason the mixing reads as one list rather than as two lists sharing a
column.

### 2.4 The collapse, and the reflow — the hard part

**The problem, stated exactly.** `Shelf::grid_width` is
`window_w − INDEX_LANE_W` (`app.rs:4108–4110`). A lane that takes width
makes it `window_w − SIDEBAR − INDEX_LANE_W`, so **collapsing re-hangs the
grid**, and re-hanging the grid is precisely the gesture-breaking reflow
that was one of the five findings. ADR-0022 celebrated deleting the
machinery that existed to survive it — *"the reflow, the width tween, the
panel's lagging album, the grid hold, the double-click detector and the
`ColumnHoldTick` subscription all existed to make a re-hang survivable;
none of them has anything left to do."*

**Three candidate answers, and why the third is right.**

**(a) The lane overlays the wall; the wall keeps its full width.** This is
what the panel does today and it is why the panel never re-hangs anything
(frames `01`/`03`: *Ochre* at x 41–281 in both, to the pixel). It fails for
a *resident* surface by arithmetic: at 1280 the wall would be hung for
1076 px with a 40 px margin, so the first column of covers begins at
x 136 — and an expanded 280 px lane would cover 60 % of it. A surface that
permanently hides content is worse than one that re-lays it.

**(b) Make the hang invariant to the lane's two states.** It cannot be
done. The column count is `clamp(round((w + hang)/(art_target + hang)), 1,
floor((w − hang)/(art_min + hang)))` (`shelf.rs:363–366`), and any width
delta can cross a column boundary at some window size. §9 shows it
crossing at 1440 and not at 1280 or 1920 — which is exactly the point:
you cannot choose widths that avoid it everywhere.

**(c) Accept the re-hang, and make it unable to break anything.** Adopted.
The rule that replaces *"no press re-hangs the collection"* is:

> **No press re-hangs the collection except the one press whose subject is
> the collection's width — and that press lands outside the wall, so no
> gesture on the wall can be in flight when it fires.**

This is a structural property, not a mitigation, and it is worth being
precise about why. The failure the old rule was written against was
specific and documented: a press *on a tile* opened the inspector, which
re-laid the grid **under the pointer**, so the second press of a
double-click landed on a different record
(`impl/04-doubleclick-plays.png`, and the `ColumnHoldTick` machinery built
to survive it). The collapse control is in the lane's foot. Nothing on the
wall is mid-gesture when it is pressed, because the press is not on the
wall. The double-click it could break does not exist any more anyway
(ADR-0022 removed it structurally), and the one wall gesture that spans
frames — the drag — starts on rows, not tiles (`drag.rs`).

**Three details make it feel controlled rather than random**, and they are
where the responsiveness bar is actually met:

1. **Hard cut, one frame.** No width tween. ADR-0020 §2.4's 150 ms
   inspector-width tween is not forbidden — it simply has no surface, and
   *"if either surface ever returns, its number returns with it"* — but it
   should **not** return here: tweening the lane's width would re-resolve
   `Grid::new` on every frame of the tween, so the wall would re-hang nine
   times and pop columns mid-slide. One frame is both cheaper and better,
   and it is what *"the collapse must feel immediate"* means.
2. **The wall keeps its shelf, not its pixel offset.** After the re-hang
   the wall scrolls so that the shelf that was at the top of the viewport
   is still at the top. The machinery exists: `Shelves::run_at` already
   maps a scroll offset to a run, and `Shelves` recomputes run tops from
   the new grid. Without this the wall lands at an arbitrary place and the
   collapse feels like a page reload; with it, the collapse feels like the
   covers changed size, which is what actually happened.
3. **The last-opened record's rule survives it.** The 2 px rule under the
   record you last opened (`shelf.rs:950–956`) is drawn from data, not
   geometry, so it is still on the right tile after the re-hang — which is
   the anchor the eye actually uses.

**The control.** Two marks at the foot of the lane, in the density
detents' exact anatomy (ADR-0028): each a `STEPPER_HIT` 24 box, the
current state's mark at full glyph ink and **inert** (it is the fact), the
other at the resting `GLYPH_OPACITY` and pressable (it is the control),
each tooltipped with its state's name — `Expanded`, `Collapsed`. The
glyphs are self-depicting the way the density marks are: a rectangle with
a wide left band, and one with a narrow left band.

This puts the two view controls in the product at the feet of the two
lanes, one on each side of the wall — which is a composition statement
worth having rather than a coincidence, and it means the lane introduces
**no new control vocabulary at all**.

`Ctrl+B` returns as the accelerator. Doc 07 §5.3 deleted it because *"its
subject was a sidebar that no longer exists"*, and ADR-0022 left it unbound
deliberately — *"a key that survives a redesign pointing at a new meaning
is worse than one that stops."* Its subject has returned, and its meaning
is the one it always had, so the key returns with it and the reflex every
editor of this decade has trained is worth something again.

**Persistence.** The state is one bool in `config.toml`, beside the density
step and the group key — ADR-0028's precedent verbatim (*"the step persists
exactly as it did — as state in `config.toml`, the way the group key
does"*). No Settings row: this is a view question, and ADR-0017 §1.3 stands.

**Below `SIDEBAR_FLOOR` 1000** the lane is collapsed and the `Expanded`
mark is inert — the density control's own rule for a step it cannot take,
and the L9-shaped regime the strip already declares. One breakpoint, one
assertion, no cascade.

### 2.5 Responsiveness, paid in arithmetic

The lane's cost, per frame and at rest:

| Cost | Answer |
|---|---|
| Idle CPU | **Zero.** No subscription, no tween, no clock. The lane is a pure projection of a list held in memory, exactly as the index rail is a pure projection of the shelves (`shelf.rs:540–545`) |
| Per-frame work | Ten to thirteen rows of `container`/`text`/`image` at the shipped window (§9). The wall draws four to six *hundred* fewer pixels of cover than it did, so the frame gets cheaper, not dearer |
| Reading the ledger | **Once, at launch.** The returns list is built once and then maintained by events: a `TrackStarted` updates one entry's timestamp and re-sorts a 24-entry list; a playlist write updates one entry. **Never a file read per frame, and no watcher** — ADR-0024's own argument against watching the playlists folder applies unchanged |
| Thumbnails | The wall's existing cache, the same decode path, the same LRU, the same deterministic gradient placeholder (ADR-0024 §A1's rule). At `SIDEBAR_SLEEVE` 48 the lane asks for thumbnails the wall has usually already decoded |
| The re-hang | One `Grid::new` — *"six multiplications and a floor"* (`shelf.rs:308–310`) — plus one scroll fix-up. One frame |

The one thing that would break this bar is a lane that recomputed its
order from the ledger every frame, and the design above forbids it in the
only place it matters: **the ordering is state, updated by the events that
change it.**

### 2.6 Aesthetics

- **Type.** The name at `SIZE_BODY` 13 Medium in `paper`; the quiet line at
  `SIZE_META` 12 in `paper_faint`. That is the wall label's own pair
  (`shelf.rs:942–986`), so a row in the lane and a caption under a cover
  are the same two lines at the same two sizes — which is what makes the
  lane read as part of the same room rather than as a docked widget.
- **The playing record** takes the lamp dot before its name and the halo
  around its sleeve — the wall's exact vocabulary (`shelf.rs:916–941`),
  amber only for playback truth, never for selection.
- **No seam, no ground, no shadow.** This is the one place a shipped law
  gets overruled in the open. The panel draws a 1 px seam down its left
  edge (`playlist_panel.rs:169–173`) because it *floats* over the wall and
  needs to say where it ends. A resident lane does not float, and the index
  rail — the surface it is most like — deliberately has *"no ground, no
  edge, no chips, no rule between the lane and the wall"* (`shelf.rs:548`).
  The lane follows the rail, and the separation is real rather than drawn:
  **the wall's block is centred in its grid width with `margin ≥ hang`**,
  which is not a hope but a consequence — when the art is uncapped the
  gutter is exactly `hang` and the margin is exactly `hang`; when the art
  is capped the block is smaller and the margin is larger
  (`shelf.rs:368–380`). So there are **never fewer than 40 px of clear wall
  between the lane's rows and the nearest cover**, at any width, in either
  state. A drawn line would be adding ink to a gap that is already there.
- **The collapsed lane is a column of covers**, which is the product's best
  argument for itself: at 96 px it is the wall's own vocabulary at a
  smaller size, and it holds the two things the eye is fastest at — colour
  and image. The tooltip carries the name (doc 10 §3.1's icon-only rule),
  which is also what makes the collapsed state legal rather than a hover
  puzzle.
- **The heading.** One word, `Recent`, at `SIZE_META` caps-tracked in
  `paper_faint` — the state-row vocabulary, the same voice as a shelf
  header. It names the ordering, which is the one thing every row in a
  mixed list shares. In the collapsed state the heading is absent, not
  abbreviated: 96 px cannot hold a tracked word, and a clipped label is
  worse than none when every row below it carries a tooltip.

### 2.7 What the lane replaces: the panel dies

The playlist panel has three jobs. The lane takes two and §6 takes the
third:

| The panel's job | After |
|---|---|
| **The index of lists** | The lane holds *every* playlist, resident, in every place but Settings — strictly more available than a surface you had to summon |
| **The destination picker** | §6's card at the pointer |
| **The drag's drop target** | The lane, which is *better*: the panel had to be open before you started dragging, and the lane is always there |

Zero jobs remain. **The panel goes** — `views/playlist_panel.rs` in full,
`PANEL_W`, the `panel_open` state, the `Playlists` strip door, `Ctrl+P`,
and `Playlists::peel`'s layers. `REFUSALS.md`'s *"one summoned,
single-tenant panel exists"* sentence goes with it, replaced by the lane's
own entry in ADR-0030.

This is the honest answer to the owner's *"I don't hate it, but I don't
really love it either"*: the panel was carrying three jobs because there
was nowhere else to put them, and once a resident lane exists there is a
better home for every one.

Two things the panel did that must not be lost, and where they land:

- **`New playlist`** — the creation door. It moves into the card (§6.3),
  where creation happens at the moment you need a list that does not exist,
  and to the lane's foot beside the collapse marks for the deliberate case.
- **`Save as playlist`** on the queue place is untouched — it was never the
  panel's.

### 2.8 What the lane does *not* do

- **It is not a nav rail.** It holds objects, not destinations. There is no
  `Library` row, no `Settings` row, no `Queue` row (§2.1).
- **It does not follow selection.** It is not an inspector; nothing in it
  changes because you clicked something on the wall. That is the property
  the dead column lacked, and it is why this lane has no arbitration state.
- **It does not scroll the wall.** Pressing a row opens a page — the same
  message a tile sends (`AlbumClicked`) or the panel's row sent
  (`OpenPlaylist`). Getting *back to what is playing* remains the bar's
  now-playing block, which is a different question with a different owner
  (ADR-0022).

---

## 3. The home band

### 3.1 What is honestly available, inventoried

Spotify's home is a recommendation surface, and every mechanism it runs on
is refused here by name: no engagement stats, no radio, no invisible pools,
no auto-generated anything, and *history records; it never performs*. So
the question is not *what does a home page usually hold* but **what does
baz already know that is worth showing, and did the person cause it?**

| Fact | Where it lives | Did the person cause it? | Verdict |
|---|---|---|---|
| **The interrupted run** — what was playing, which track, how far in | ADR-0023 §6's snapshot — **specified, not shipped** (§0.4) | Yes, entirely | **Earns a place.** The single best thing on the list |
| **Recently added records** | `first_seen_ns`, written once when a row is created (`baz-core/src/index.rs:100, 244`); already the `ADDED` group key | Yes — they added the files | **Earns a place** |
| **Recently played records** | the ledger (ADR-0018) | Yes | **Refused here** — it is the lane's content, and one fact drawn twice is L8.6's own test |
| **Playlists** | the folder | Yes | **Refused here** — the lane's content |
| **The pull** | the ledger's weighting | **No** — the machine chooses | **Refused.** The pull is an act you press, and an unbidden offer on a home surface is generation without a request |
| Play counts, streaks, totals, "top artists" | — | — | Refused outright, and not close |
| Collection counts | already the search well's placeholder (ADR-0026 §4) | — | Already stated; L8.6 |

Two of the seven survive, and the survivors are exactly the two facts the
lane cannot carry: **a position** (the run you were in the middle of) and
**an acquisition** (what arrived, which is not something you *touched*).
That is a useful check — it means the home band and the lane are not two
drawings of one fact.

### 3.2 Home is the wall's own head

> **The Library place's body leads with the home band when the query is
> empty. There is no fifth place.**

Why not a `Place::Home`:

1. **The wall is the product.** *"The library is the interface"* is a
   `VISION.md` pillar, and `03` §2.3's content share at rest — 73–100 %
   against a tradition managing 0–26 % — is the number the product is
   positioned on. A launch destination that is not the collection spends it.
2. **A home place would need a way back to the wall, and that is a nav
   rail.** The lane holds objects, not destinations (§2.8); adding a
   `Library` row to it would be the third tenant and the first destination,
   which is finding 1 arriving on day two. Doc 07 L8.4 refused a nav rail
   already.
3. **You are always already there.** The band is at the head of the body,
   so arriving at baz *is* arriving at home, and scrolling is leaving it.
   There is no navigation to design, no door to label, and no `‹ Library`
   ambiguity to resolve.
4. **The precedent is shipped.** Under a non-empty query the wall's body
   grows a `Songs` section above it (doc 09 §5, `shelf.rs:151–159`). The
   home band is the same move for the empty query, in the same slot, with
   the same absent-not-empty rule.

**One rule governs the slot**: the body's head is the **home band** when
the query is empty, and the **Songs section** when it is not. Never both,
never neither.

The alternative — a real `Place::Home` as the launch destination — is drawn
in §9.4 with its costs, because the owner asked for a *page* and should be
able to overrule this with the design already done rather than as a
sketch.

### 3.3 What the band holds, and the rules

```
─── CONTINUE ────────────────────────────────────────────────────────
  ┌────┐  Anhydrous 2                                    [ ▶ Resume ]
  │ 64 │  Violet Ledger · Anne-Marie Puig · 3:12 of 6:27
  └────┘

─── RECENTLY ADDED ──────────────────────────────────────────────────
  ┌──────────┐  ┌──────────┐  ┌──────────┐        one wall row, the
  │  cover   │  │  cover   │  │  cover   │        wall's own tile at
  └──────────┘  └──────────┘  └──────────┘        the current density
  Teal          Red Shift      Ochre

─── ARTIST · A ──────────────────────────────────────────────────────
  the wall
```

- **`CONTINUE`** — one row: the track, the record and artist, the position
  in figures, and a `Resume` control that is the ordinary `Play` (glyph +
  word, no accent — the strip's `Play all` anatomy, not the page's lamp).
  It restores the snapshot **paused** and one press sounds, which is
  ADR-0023 §6 verbatim. Nothing about it plays by itself.
- **`RECENTLY ADDED`** — exactly one row of the wall's own tiles at the
  current density, newest `first_seen_ns` first. Not a new object, not a
  new size, not a carousel: one row of the same tiles, so the band costs no
  new vocabulary and inherits hover, the menu, shift-click and the halo for
  free.
- **The section rules** are `section_rule`'s caps-tracked words, the same
  furniture the `Songs` and `Albums` sections already use
  (`views/mod.rs`, `shelf.rs:216–219`).

**The rules that keep it honest:**

- **A band is absent, not empty** — doc 09 §5's rule for the Songs section.
  No "nothing here yet" placeholder inside a band; the band simply is not
  there.
- `CONTINUE` is absent when there is no snapshot, or the snapshot's files
  no longer resolve.
- `RECENTLY ADDED` is absent when the library holds fewer than
  `2 × columns` records — so the band never shows you a row you can already
  see whole on the wall below it. It is also absent when nothing has been
  added since the first scan created every row at once, because then every
  record is equally new and the band would be an arbitrary slice.
- **Both are absent under a query**, where the slot belongs to `Songs`.
- **Both scroll away.** They are at the head of the body, not pinned. The
  wall at rest, one scroll down, is exactly the wall of today.

**What the two library sizes see.** A first run with 4 albums: no
`CONTINUE` (nothing has played), no `RECENTLY ADDED` (4 < 2 × 3 columns),
so the body is the wall — which is the right first frame and is also
today's first frame, unchanged. A library of 40 000: both bands, each a
fixed height the virtualizer adds as a constant to its estimate, and the
wall below them virtualized exactly as now.

### 3.4 What the home band unblocks

ADR-0023 §6 is specified, argued, costed at *"one new persisted snapshot,
zero engine changes"*, and unbuilt. It closes prior art's **W2**, the one
band-C workflow *"whose absence is felt every launch"*. It has never
shipped because nothing on screen wanted it: a queue that restores paused,
with no surface saying so, is indistinguishable from a queue that did not
restore.

The home band is the surface that wants it. **They unblock each other**,
and that is the strongest argument in §3: the best thing a baz home page
can hold is a feature the product already decided to build and never found
a reason to.
