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

Both widths were derived from baz's own tokens and then found to land on
the industry's: Material's navigation drawer is **280 dp** at its default
maximum and its collapsed rail is **80 dp**, or **96 dp** under the
expressive update ([Material Components for
Android](https://github.com/material-components/material-components-android/blob/master/docs/components/NavigationDrawer.md),
[NavigationRail](https://github.com/material-components/material-components-android/blob/master/docs/components/NavigationRail.md)).
280 and 96 exactly. That is corroboration rather than derivation — the
numbers came from `GAP_XL` and `MENU_W` — but a surface that arrives at the
same two figures from a different direction is a surface at a sensible size.

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

**The one piece of guidance this contradicts, engaged.** Apple's HIG says
*"avoid putting critical information or actions at the bottom of a
sidebar. People often relocate a window in a way that hides its bottom
edge"*
([Apple HIG — Sidebars](https://developer.apple.com/design/human-interface-guidelines/sidebars)).
The concern is real and does not reach baz: the window's bottom edge
carries the now-playing bar in every place, so a window positioned with its
bottom edge off-screen has lost the transport, the needle and the volume
fader — a problem the collapse marks are not the worst part of. The
product has also already placed view controls exactly there once, on the
other side of the wall (ADR-0028's density detents), and putting the
sidebar's marks anywhere else would break the symmetry that makes both sets
legible as one class. Recorded rather than skipped, because a design that
only cites the guidance agreeing with it is not citing guidance.

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
  and image. The tooltip carries the name (doc 10 §3.1's icon-only rule).

  **The guidance against this is real and is worth stating.** Nielsen
  Norman is blunt about icon-only navigation: *"A text label must be
  present alongside an icon to clarify its meaning in that particular
  context"*, labels *"should be visible at all times"*, and *"don't rely on
  hover to reveal text labels: not only does it increase the interaction
  cost, but it also fails to translate well on touch devices"*
  ([NN/g](https://www.nngroup.com/articles/icon-usability/)). Three things
  make the collapsed lane survivable where an icon nav bar would not, and
  they are the conditions under which it should be kept:

  1. **These are not icons.** NN/g's finding is about glyphs standing for
     concepts — a heart meaning *favourite*, a gear meaning *settings*. A
     record's sleeve does not stand for the record; it **is** how that
     record is identified everywhere else in the product, on a wall whose
     entire premise is that a person recognises their own covers. The
     playlist collage is the same claim one level up.
  2. **Expanded is the default and it persists.** Apple's HIG says the same
     thing from the other side — *"avoid hiding the sidebar by default to
     ensure that it remains discoverable"* — and adds the behaviour §2.4's
     floor already implements: *"consider automatically hiding and
     revealing a sidebar when its container window resizes."* Collapsed is
     a state a person chose, or one a narrow window forced; it is never
     where they start.
  3. **Nothing is only in the collapsed state.** Every row's press is the
     same message in both, and the name is one press of the `Expanded` mark
     away.

  If the covers turn out not to carry it — if the owner finds himself
  hovering to read names — the honest fix is not a label crammed into 96 px
  but a wider collapsed width, and that is a token change.
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

---

## 4. The wall's verbs

### 4.1 What the owner asked for, and what exists

Three verbs revealed by hovering a tile, drawn above it: **play now**,
**send all to current playlist**, **view details**. Two of the three are
already controls (`Play album` on the record's page; `Open` is the tile's
own press). The third — *send all to current playlist* — **does not exist
at album scope anywhere in the product**: `Add to "{current}"` is offered
on track rows, queue rows, playlist-page rows and the bar's now-playing
block (`menu.rs:192–201`, `:214–223`, `:285–291`, `:313–321`) and on no
album object at all. §4.4 closes that gap.

And the *group* exists too. Frame `02` is a photograph of it: right-press a
sleeve and `Open · Play album · Queue album · Add to playlist…` floats
beside the record you pointed at, 232 px wide, its top-left corner at the
pointer, edge-flipped so it is never clipped. **Two gestures, no navigation,
the wall untouched behind it** — which is better on every axis than the
flow the owner is complaining about except one: the reveal is a right-press,
and nothing on the wall says so.

### 4.2 Why the hover reveal would be worse, measured

This is not a principle argument. The drawn design does not fit.

**The space it would have to fit in.** The wall's clear space between one
row's state rule and the next row's sleeve is the step's hang less the
rule's lane — `RULE_LANE_H = GAP_XS + SELECTION_EDGE` = 6
(`shelf.rs:1037`):

| Density | hang | clear wall between rows |
|---|---:|---:|
| Spacious | 48 | **42** |
| Balanced | 40 | **34** |
| Dense | 28 | **22** |

**What would have to go in it.** A card of three items is
`3 × TRANSPORT_HIT + 2 × GAP_XS` = **104 px**; four items **136**
(`menu::extent`, `menu.rs:344–354`). It overshoots by 62–114 px at every
step, so it must cover a neighbouring record — and laterally it is
`MENU_W` 232 against a tile pitch of 283 at the shipped window, so it
covers most of one. The flattest legal alternative, a single row of words
at L7's one control height, is 32 px: it does not fit at `Dense` at all,
and at `Balanced` it fits with 1 px of air on each side. It also cannot
carry the owner's own verbs — hanging from the art's width it would give
three items 80 px each, and `Add to "Road Trip"` is why `MENU_W` is 232.

**And it would make the wall twitch.** Today, crossing the wall changes a
1 px rule to 2 px and lifts one caption line by one rung, over a 90 ms
tween (`shelf.rs:972–976`; frame `01`). Crossing a 1280 px wall passes over
three or four tiles, so a hover group means three or four opaque cards
appearing and vanishing in one movement that was aimed at the index rail.
The usual fix is a dwell delay, which trades the twitch for a wait and
needs a clock — and the owner's own bar is *responsiveness*.

**Nobody does it this way, and the reason is geometric.** Every product
that reveals a play affordance on a cover grid draws it **inside the
object's own bounds** (§10.4: Apple Music, Plex, Spotify, Tidal all on the
artwork or its card; MusicBee and foobar2000 reveal nothing on hover at
all). An object's own bounds are the one region guaranteed to belong to no
other object, which is why a full-bleed grid has nowhere else to put it.
baz draws nothing on artwork — so the placement that makes the pattern work
is the one baz does not have.

> **Decision: no hover-revealed verb group on the wall.** Recorded in §11
> with the measurements, so a proposal that has an answer to the geometry
> meets a number rather than a wall.

### 4.3 Is the hover group just the menu, unhidden?

It is the strongest version of the ask — the card, the verbs, the geometry
and the mirror rule all exist already — and it is still no, for two reasons
specific to the menu:

1. **The card is `opaque` and captures presses** (`app.rs:3236–3249`,
   `menu.rs:449–468`), with a full-window backdrop under it whose left
   press dismisses it. A wall that grows an opaque card wherever the
   pointer rests is a wall whose next click is spent putting something down
   that you did not ask for.
2. **A menu that opens itself is not an accelerator any more.** The menu's
   whole licence is that it mirrors visible controls (`menu.rs:1–22`,
   tested at `:581`). Unhidden, it is a resident surface with a floating
   position, and it would be the second thing on the wall competing for the
   pointer with the tile itself.

What survives is the useful half: **the menu already is the adjacent group
the owner is describing.** It needs one verb and one teacher.

### 4.4 `Add to "{current}"` at album scope

With a current playlist standing — playing provenance naming a `.m3u8`
that still exists (doc 09 §6) — a record's whole selected edition is
appended to **that file**. The run is untouched: *keep it* is
`Add to "Road Trip"`, *hear it tonight* is `Queue album`, and doing both is
both gestures. The decoupling is doc 09 §6's and the both-at-once gesture
stays refused, for the reason §10.6's Plexamp entry demonstrates — two
verbs that claim different things and do the same thing are worse than one.

It costs nothing to build: the item's presses are `AddAlbumToPlaylist(id)`
then `PickPlaylist(id)`, both already in the mirror table with a named
visible twin (`menu.rs:586–615`). No new message, no new control.

```
Open                                     the tile's own press
Play album                  Ctrl-click   the page's Play album        ← §4.6
Queue album                 Shift-click  the picker's Queue row
Add to “Road Trip”                       the card's hoisted row       ← new
Add to playlist…                         the page's Add to playlist…
```

Five items is `5 × 32 + 8` = **168 px**, against the four-item 136 in frame
`02`; the edge flip already holds any height inside the window
(`menu.rs:359–372`, tested at `:761`).

It does **not** get a resident control on the record's page. The aside
holds `Play album` and `Add to playlist…`; a third word-act naming a file
that may not exist next frame is a control that comes and goes, to save one
press on a route that exists.

### 4.5 Teaching the reveal

The gesture is taught nowhere on the wall — doc 11 §2.7 found this and named
it (*"The menus mirror controls; nothing mirrors the gestures"*), and the
repair that shipped prints `Shift-click` *inside* the menu, which teaches
the accelerator to people who already found the menu.

The teacher goes where the person has **just paid the cost it would have
saved**: the record's page, reached by a tile press, whose header note lane
is one quiet meta line today.

```
‹ Library   Album   ‹ Prev · 4 of 25 · Next ›     Right-click a sleeve to play it from Library
```

One string, one lane that exists, in the voice the product already teaches
in (`Enter plays the first match.`, `Esc clears the search.`, `When a queue
ends, baz stops.`). Deliberately modest: adding a fourth kind of teaching
surface to carry one gesture is the tour doc 11 P6 refused.

### 4.6 One press to sound from the wall · **present-to-owner**

Everything above leaves sound from the wall at two presses; the owner asked
for one. The candidate that fits every constraint the product actually has
is a **modifier press on the tile meaning `Play album`** — the exact
construction already shipped for *sound-later* (shift-click queues the
record, `app.rs:1054–1061`), pointed at *sound-now*.

- It draws nothing on a sleeve and reveals nothing on hover.
- It has no timing window: the press's meaning depends on a key that is
  down or not, which is a state the hand chose rather than one the clock
  chose.
- It has a visible twin twice over — `Play album` on the page and in the
  tile's menu — so it is taught exactly where shift-click is taught, in the
  menu's accelerator column (§4.4's table).
- One arm in `Message::AlbumClicked`, one string. The existing test
  `shift_click_queues_the_record_and_nothing_sounds_unasked`
  (`app.rs:5460`) gains a sibling.

**Against it**: three meanings for one press on the product's most-pressed
object; `Ctrl`-click is the platform's add-to-selection chord, and the wall
may one day have a selection; and it is a second meaning for a press, which
is what ADR-0022 pointed away from when it aimed any return of one-press
sound at the shift-click stack.

**For it**: W1 (*put on an album*) is band A and is the product's home
intent; one-press sound-now already exists at every scope in the product
*except a single record*, which is the unit the wall is made of; and a
product with a one-press *later* and no one-press *now* has its two
gestures the wrong way round.

Presented rather than adopted, with the arm and the accelerator drawn, so
the answer is a decision rather than a design exercise.

---

## 5. Depth, and what the lane already answers

### 5.1 The tree, measured

Every navigation baz has, with its depth from home:

| From | To | Route | Depth |
|---|---|---|---|
| Library | Album | a tile's press (`shelf.rs:1021`) | 1 |
| Library | Album | a Songs row's record door (`shelf.rs:361–379`) | 1 |
| Library | Queue | the bar's `Queue` door, `Ctrl+U` | 1 |
| Library | Playlist | a lane row (was a panel row, `playlist_panel.rs:399`) | 1 |
| Library | Settings | the gear, `Ctrl+,` | 1 |
| anywhere | Album | the bar's now-playing block | 1 |
| Album | Album | `‹ Prev` / `Next ›` (`album.rs:169–191`) | 1 → 1 |

**The maximum depth of baz's navigation tree is one, from every place.**
`Place` is five members and an enum, and `Place::back` is total because
there is nothing for it to be partial about (`place.rs:171`). A breadcrumb
here would render `Library › Album` — the header's back door and the
header's title, with a separator between them (§10.7 for the guidance that
excludes exactly this case). Miller columns over a one-level tree render
one column.

### 5.2 The lane is the answer he described

*"When there's a master details view, often you can pop back up a level
quite easily because you can already see the depth that you've went into
with the options on the left."*

That is not a request for a level counter. It is a description of **a
master pane that stays on screen while you are in the detail** — and the
lane is exactly that, in every place but Settings. Open a record from the
lane and the lane is still there with the record's row in it; open a
playlist and the same. The felt problem was never depth; it was that the
window replaced everything you had, and the lane is the thing that stops
it doing that.

Two mitigations already shipped and are worth restating because they are
what the lane completes rather than replaces: the wall keeps its scroll,
query and arrangement across every navigation, and marks the record you
last opened (`shelf.rs:950–956`); and `‹ Prev` / `Next ›` step the wall's
own visible order from inside the page (doc 11 P3).

### 5.3 The one thing still missing: position

A record's page says `Album`. It does not say which of 25 records it is,
nor that the wall behind it is filtered to 7. The step pair *computes* all
of this — it is `vm::neighbours` over the wall's visible order — and states
none of it (frame `04`: two doors with no position between them).

> **The step pair states the position it already computes.**

```
‹ Library   Album   ‹ Prev  ·  4 of 25  ·  Next ›            Esc returns to Library
                                 ▲
                    the position in the wall's current arrangement — and
                    therefore the scope: a filtered wall says `2 of 7`
```

- A **readout**, not a control: no message, no press. L8.3's escape valve
  in its ordinary direction.
- It goes in the header's **existing optional-tenant slot**
  (`place_header_with`, `views/mod.rs:245–258`), so the frame stays one
  function in five places.
- Reserved `POSITION_W`, sized for `99 of 9999` at `SIZE_META` in the
  bundled Medium and asserted against its measured word, so stepping from
  `9 of 25` to `10 of 25` moves neither door.
- **Absent when there is nothing to say**: a record the wall no longer
  shows has no neighbours at all (`vm.rs:1092`, tested at `:2496–2503`),
  its doors are already inert, and it states no position rather than
  `0 of 0`.

### 5.4 What is not proposed

**No back stack.** `Place::back` stays total. There is one reachable path
where this orphans you — Queue place → the bar's now-playing block → the
record's page → `Esc` → Library rather than the Queue — and it is band D,
it is a *teleport* rather than a descent, and the standard guidance treats
Up and Back as identical inside a single task (§10.7). With the lane
resident, the case matters less again: what you left is still on screen.

---

## 6. The picking posture

### 6.1 The complaint, found in the code and photographed

The owner is describing `views/playlist_panel.rs:122–129`, verbatim:

```rust
    // A pick in flight: the panel is the picker, and it says what is in the
    // hand so the next press is legible before it is made.
    if let Some(pending) = &playlists.pending {
        body = body.push(
            text(format!("{} — pick a destination", pending.label))
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.paper_dim),
        );
    }
```

`pending.label` is `format!("Add \u{201c}{}\u{201d}", …)` (`app.rs:1718–1721`
for a record, `:1756` for a track, `:1784` for a queue row, `:1810` for a
playlist row), so the rendered line is `Add “Violet Ledger” — pick a
destination`. Frame `03` is that line on screen, and everything he says
about it is true: it is set at **`SIZE_META` 12** in **`paper_dim`**, under
a `Playlists` heading at `SIZE_EMPHASIS` 15 Medium and level with `Esc
closes`. **The panel's only statement of what it is now for is quieter than
its own title.**

But the copy is the smaller half. Three interaction defects are in the same
frame:

1. **The destination is thrown across the window.** The panel is anchored
   to the window's right edge whatever the gesture was
   (`app.rs:3216–3218`). At 1280 its rows begin at `1280 − 341 + GAP_XL` =
   **963**; the tile right-pressed in frame `03` is centred at x 444. That
   is **≈ 682 px** of pointer travel to the first destination. In the menu
   (frame `02`) the second press is **≈ 127 px** away.
2. **The distance is set by the window, not by the gesture.** At 1920 the
   same pick costs 640 px more. A gesture whose cost rises with the size of
   the display has not been designed; it has been inherited from where the
   surface happens to live.
3. **The surface is enormously larger than the task** — 340 px at full
   window height to offer three destinations — and while it stands it
   covers the index rail and the density detents.

### 6.2 The card at the pointer

> **A pick opens a card at the pointer, headed by the sentence, holding the
> destinations and nothing else.**

```
        ● pointer
        ┌─ PICKER_W 280 ────────────────────────────┐
        │                                           │ GAP_SM 8
        │ Add “Violet Ledger” to…                   │ LINE_BODY 20   SIZE_BODY, paper
        │ 9 tracks · 45:26                          │ LINE_META 16   SIZE_META, dim
        │                                           │ GAP_SM 8
        ├───────────────────────────────────────────┤ 1  hairline
        │                                           │ GAP_SM 8
        │ ▫ 48  Queue                   8 · 32:10   │ 64
        │ ▫ 48  Road Trip — playing    14 · 51:08   │ 64
        │ ▫ 48  Late Nights            23 · 1:40:11 │ 64
        │                                           │ GAP_XS 4
        │    New playlist                           │ 32
        │                                           │ GAP_SM 8
        └───────────────────────────────────────────┘
             Esc cancels · a press outside puts it down
```

**`PICKER_W` is 280 — the same number as `SIDEBAR_W`**, and the row is the
lane's row (`SIDEBAR_ROW_H` 64, `SIDEBAR_SLEEVE` 48). That is the point: a
list looks like itself wherever it appears, so the destinations on the card
are visibly the same objects as the rows in the lane. Three destinations is
`8+20+16+8+1+8 + 3×64 + 4+32+8` = **297 px**. `PICKER_MAX_H` **400** caps
it at five rows plus chrome; beyond that the rows scroll inside the card
with the heading and `New playlist` pinned outside the scroll.

**Placement** is `menu::anchor`'s rule exactly — top-left corner at the
pointer, flipped at any edge it would cross, clamped as a last resort
(`menu.rs:359–372`) — one shared function, so the two floats cannot
disagree about where an edge is.

**What each line is for:**

- **`Add “Violet Ledger” to…`** at `SIZE_BODY` 13 in full paper, Medium:
  the verb, the object, and an ellipsis promising the list below. A surface
  whose whole reason for existing is a question states the question at the
  size its own name would have taken. This is the copy fix, and it is the
  smaller half of the change by design.
- **`9 tracks · 45:26`** — what is in the hand, in figures. The shipped
  picker never states it, and it is what distinguishes *this record* from
  *this track* when the label alone is ambiguous.
- **The rows lose their `Add` word** (`playlist_panel.rs:227–232`,
  `:369–374`), which was the design compensating for a heading nobody
  read. With the heading carrying the verb, a row is a destination and says
  so by being one.

**Order** is unchanged and already pure and tested —
`playlists::picker_order` (`playlists.rs:526–531`, tested at `:1845–1856`):
the **Queue** first, the **current playlist** hoisted and marked *playing*,
then the folder's order, then `New playlist`.

**Dismissal** is the product's one model, because it is the menu's:
`Esc` peels the card before every other layer, a left press outside puts it
down and is never a spent click, a right press outside falls through.

**`New playlist`** turns the row into the name field the panel already has
(`playlist_panel.rs:273–296`) with the storage layer's refusals under it in
its own words; submitting creates and completes the pick
(`playlists.rs:516–518`). The card grows downward from its anchored corner,
or upward where the flip put it.

### 6.3 What changes, exactly

| Gesture | Today | After |
|---|---|---|
| A track row's `+` | opens the panel at the window's edge | the card, at the `+` |
| The record page's `Add to playlist…` | opens the panel | the card, at the control |
| A menu's `Add to playlist…` | opens the panel | the card, where the menu was |
| A menu's `Add to "{current}"` / `Queue` / `Queue album` | two messages, no surface shown | **unchanged** |
| Drag a row onto a playlist | onto a *standing panel's* row | onto **the lane's** row — always available |
| `Playlists` door, `Ctrl+P` | opens the panel | **gone**; the lane is resident (§2.7) |

The two-message menu items are the row that matters: they never *showed*
the picker, they made both presses (`menu.rs:189–190`, `:194–199`). They
keep working unchanged; only the surface that draws the second press moved.

---

## 7. The panel

Answered in §2.7 and recorded here so the brief's fourth question has an
answer under its own heading: **the panel does not survive.** Its three
jobs go to the lane (the index, resident and complete), the lane again (the
drop target, now always available rather than only while it was open) and
the card (the picker). Nothing is left for it to do, and the owner's *"I
don't hate it, but I don't really love it either"* is explained by the fact
that it was carrying three jobs because there was nowhere else to put them.

ADR-0024 §5's five justifications, re-run one last time: **one tenant** —
moot; **summoned, not resident** — the lane is resident and that is now the
point; **simultaneity while collecting** — the lane provides it
continuously; **overlays without reflow** — the lane re-hangs once, on
purpose, by the one press that means to (§2.4); **absent in Settings** —
inherited by the lane verbatim.

---

## 8. The user stories

Doc 09 §4's artifacts, continuing its numbering: a **user story**, its
**task flow** in exact presses, and **acceptance criteria** in
Given/When/Then, written to be implemented and tested as stated.

### S11 — Come back to what I was in the middle of

> As a listener who closed baz mid-record last night, I want the first
> thing I see to be where I was, so that resuming costs one press and no
> searching.

**Task flow**: ① launch; the `CONTINUE` band is the first thing in the
body; ② `Resume`.

- Given a queue snapshot exists from a previous session, when baz launches,
  then the queue is restored **paused** and nothing sounds (ADR-0023 §6).
- Given the restored queue, when the Library place renders with an empty
  query, then the body leads with a `CONTINUE` band naming the track, its
  record and artist, and the position as `3:12 of 6:27`.
- Given the band, when `Resume` is pressed, then playback continues from
  the stored elapsed position — an ordinary `Play`, no new engine command.
- Given no snapshot, or a snapshot whose files no longer resolve, when the
  body renders, then the band is **absent, not empty**.
- Given a non-empty query, when the body renders, then the band is absent
  and the `Songs` section holds the slot — never both.

### S12 — Get back to something I was playing yesterday

> As a listener who wants the record I had on last night, I want it on
> screen without searching for it, so that returning is recognition rather
> than recall.

**Task flow**: ① it is in the lane; ② press it → its page.

- Given a record whose track was played, when the play is recorded in the
  ledger, then that record's lane entry is timestamped and the lane
  re-sorts, newest first.
- Given more than `RECENT_ALBUMS` 24 records have been played, when the
  lane renders, then it holds the newest 24 and the rest are reachable on
  the wall.
- Given a lane row is pressed, then that record's page opens
  (`AlbumClicked`) — the same message the tile sends.
- Given two entries with the same timestamp, when the lane renders, then
  they are ordered by name ascending, so two launches over the same data
  draw the same lane.
- Given a record is playing, when its lane row renders, then it carries the
  lamp dot and its sleeve carries the halo — the wall's exact vocabulary,
  and the accent means nothing else.

### S13 — See every list I have, without summoning anything

> As a listener with a shelf of kept lists, I want them permanently in
> view, and I never want a surface to appear because I pressed something
> else.

**Task flow**: ① they are in the lane.

- Given any number of playlists, when the lane renders, then **every** one
  of them is in it — the lane is the complete index, and the 24-entry cap
  applies to records only.
- Given a playlist is played or its file is written by the user's own edit,
  then its lane entry is timestamped and the lane re-sorts.
- Given a lane playlist row is pressed, then its page opens
  (`OpenPlaylist`), and the lane is still beside it.
- Given any transfer gesture anywhere in the product, when it is made, then
  **no panel opens** — the picker is the card (§6) and the panel does not
  exist.
- Given a playlist row, when it renders, then its sleeve is the 2 × 2
  collage of the records it quotes, or the first record's face below four
  (ADR-0024 §A1), and **nothing else distinguishes it from a record row**.

### S14 — Give the wall more room

> As a listener who wants to look at covers, I want the lane out of the
> way in one press, and I want the wall to behave.

**Task flow**: ① press the `Collapsed` mark at the lane's foot, or
`Ctrl+B`.

- Given the lane is expanded, when the `Collapsed` mark is pressed, then
  the lane becomes `SIDEBAR_RAIL_W` 96, the wall re-hangs **once, in one
  frame**, and no transition runs.
- Given the re-hang, when the wall renders, then the shelf that was at the
  top of the viewport is still at the top, and the record last opened still
  carries its 2 px rule.
- Given the current state, when its own mark renders, then that mark is at
  full glyph ink and **inert**, and the other is at the resting ink and
  pressable — the density detents' rule (ADR-0028).
- Given either mark, when it renders, then it carries a tooltip naming its
  state (`Expanded`, `Collapsed`) — the icon-only law's accessible name.
- Given the state changes, when baz is relaunched, then the lane is in the
  state it was left in — one bool in `config.toml`, beside the density step.
- Given a window narrower than `SIDEBAR_FLOOR` 1000, when the place
  renders, then the lane is collapsed and the `Expanded` mark is inert.
- Given the collapsed lane, when a row renders, then it is the sleeve
  alone, carrying a tooltip with the record's or playlist's name, and its
  press is unchanged.

### S15 — Put a track somewhere, without crossing the window

> As a listener who wants to keep the song I am hearing, I want the
> destinations where my hand already is.

**Task flow**: ① a row's `+`, or right-press → `Add to playlist…`;
② press a destination on the card.

- Given any transfer control is pressed, then the card opens **at the
  pointer**.
- Given the card renders, then its heading is `Add “{label}” to…` at
  `SIZE_BODY` in full paper, with a second line stating what is held in
  figures, and **no line on the card is quieter than the sentence that says
  what the card is for**.
- Given the card renders, then its rows are, in order: `Queue`, the current
  playlist marked *playing* when provenance stands, the folder's order,
  `New playlist`.
- Given the `Queue` row is pressed, then the held music is appended to the
  run (`UpdateQueue`); appending to an empty stopped engine loads it
  without starting it (`app.rs:1363–1366`).
- Given a named row is pressed, then that **file** is appended and the card
  closes.
- Given `New playlist` is pressed, then the row becomes a name field with
  the caret in it; on submit the file is created and the pick completed,
  and the storage layer's refusals surface in the field in its own words.
- Given `Esc`, then the card closes and the pick is put down, before any
  other layer. Given a left press outside, then the card closes and the
  press does nothing else.
- Given more destinations than `PICKER_MAX_H` 400 holds, then the rows
  scroll inside the card and the heading and `New playlist` stay put.

### S16 — Send a whole record to the list I'm living in

> As a listener playing one of my playlists who has just found a record
> that belongs in it, I want the whole record in that list in one short
> gesture, without interrupting what I'm hearing.

**Task flow**: ① right-press the sleeve; ② `Add to "Road Trip"`.

- Given provenance names a playlist that still exists, when a tile is
  right-pressed, then the menu carries `Add to "{name}"` between `Queue
  album` and `Add to playlist…`; otherwise the item is **absent, not
  disabled** (`playlists::holds`, `playlists.rs:440–445`).
- Given the item is pressed, then the **file** gains the record's selected
  edition, whole, in order, with `#EXTINF` metadata, and **the live queue
  is unchanged** — not one delivered sample disturbed.
- Given the record has editions, then what is appended is the **selected**
  edition — the tracks the page lists and `Play album` would queue
  (`app.rs:1712–1717`).
- Given the append lands, then the lane's row for that playlist re-reads
  its counts and re-sorts to the head — the effect is in view.

### S17 — Start a record from the wall without leaving it

> As a listener browsing my shelves, I want to start a record I am pointing
> at without the collection disappearing.

**Task flow**: ① right-press the sleeve; ② `Play album`.

- Given the Library place and a ready engine, when a tile is right-pressed,
  then the menu opens at the pointer with `Open · Play album · Queue album
  · Add to "{current}"? · Add to playlist…` in that order, and the wall
  behind it has not moved by a pixel.
- Given `Play album` is pressed, then the record's selected edition becomes
  the queue and the first track sounds, the menu closes, and **the place is
  still Library**.
- Given a tile at the window's right or bottom edge, when it is
  right-pressed, then the card flips to the pointer's other side and is
  wholly on screen.
- Given the record's page is opened by a tile press, then its header note
  teaches the gesture that would have saved the trip.

### S18 — See what is new

> As a listener who has just added music, I want to find it without
> re-arranging the wall.

- Given the library holds at least `2 × columns` records and some were
  added after the first scan, when the Library place renders with an empty
  query, then a `RECENTLY ADDED` band holds **one row of the wall's own
  tiles**, newest `first_seen_ns` first.
- Given fewer than `2 × columns` records, or a library whose rows were all
  created by one first scan, then the band is **absent** — it never shows a
  row already visible whole below it.
- Given a band tile, then it is the wall's tile in every respect: hover,
  press, right-press menu, shift-click, halo.
- Given the wall is scrolled, then the bands scroll away with it — they are
  the head of the body, not pinned.

---

## 9. The layouts, measured

All numbers logical px on the 4 px lattice. The grid figures are
`Grid::new`'s own arithmetic at `Balanced`
(`shelf.rs:355–385`; `hang` 40, `ART_MIN` 240, `ART_TARGET` 272, `ART_MAX`
320), resolved for `window − sidebar − INDEX_LANE_W` where `INDEX_LANE_W`
is 108:

```
columns = clamp( round_half_up((w + 40) / 312),  1,  floor((w − 40) / 280) )
art     = clamp( (w − (columns+1)·40) / columns,  ART_FLOOR,  320 )
gutter  = clamp( (w − 80 − columns·art) / (columns−1),  0,  80 )
margin  = (w − block) / 2
```

### 9.1 The wall, in both states, at three widths

| Window | State | Grid width | Columns | Art | Gutter | Margin |
|---:|---|---:|---:|---:|---:|---:|
| **1280** | today (no lane) | 1172 | 4 | 243 | 40 | 40 |
| | expanded (−280) | 892 | **3** | 244 | 40 | 40 |
| | collapsed (−96) | 1076 | **3** | **305** | 40 | 40 |
| **1440** | today | 1332 | 4 | 283 | 40 | 40 |
| | expanded | 1052 | **3** | 297 | 40 | 40 |
| | collapsed | 1236 | **4** | 259 | 40 | 40 |
| **1920** | today | 1812 | 6 | 255 | 40 | 40 |
| | expanded | 1532 | **5** | 258 | 40 | 40 |
| | collapsed | 1716 | **5** | **295** | 40 | 40 |

Three things worth reading off it:

1. **At two of the three widths the collapse does not change the column
   count at all — the covers just get bigger** (1280: 244 → 305, +25 %;
   1920: 258 → 295, +14 %). That is the best possible version of this
   gesture: it reads as *zoom*, not as *reflow*, which is also what it
   means.
2. **At 1440 it does change the count** (3 ↔ 4), because the wanted count
   and the `ART_MIN` ceiling land on either side of the boundary there.
   This is unavoidable — §2.4(b) — and it is why the answer is the
   structural one about which press may re-hang, not a choice of widths.
3. **The margin is 40 in every row, and that is a proof rather than a
   coincidence.** When the art is uncapped, the gutter is exactly `hang`
   and the margin is exactly `hang`; when the art is capped the block is
   smaller and the margin is larger. So the nearest cover is **never closer
   than 40 px to the lane**, at any width, in either state — which is why
   the lane needs no drawn seam (§2.6).

### 9.2 The whole window, expanded, at 1280 × 860

```
0    24        72   96                                        1172        1280
├────┼─────────┼────┤                                          ├───────────┤
│         the returns lane, 280            │      the wall, 892       │ rail 108│
├──────────────────────────────────────────┴─────────────────────────┴─────────┤ 49  top bar
│  Recent                                   ┌──────┐ ┌──────┐ ┌──────┐         │
│  ┌──┐ Violet Ledger                       │ 244  │ │ 244  │ │ 244  │      #  │
│  │48│ Anne-Marie Puig                     └──────┘ └──────┘ └──────┘      A  │
│  └──┘                                     Teal      Red Shift  Ochre      B  │
│  ┌──┐ Road Trip                                                           C  │
│  │▨▨│ 14 · 51:08                          ┌──────┐ ┌──────┐ ┌──────┐      …  │
│  └──┘                                     │ 244  │ │ 244  │ │ 244  │         │
│   ⋮                                       └──────┘ └──────┘ └──────┘      ▫  │
│  ▮▯                                                                       ▫▫ │
├──────────────────────────────────────────────────────────────────────────────┤ 81  bar
```

Rows visible in the lane: the lane's height at 860 is
`860 − TOP_BAR_H 49 − 83` = 728; less `GAP_XL` 24 + heading 20 + `GAP_SM` 8
at the head and `STEPPER_HIT` 24 + `GAP_XL` 24 at the foot = 628, over
`SIDEBAR_ROW_H` 64 → **9 rows**, scrolling. At 1080 the same arithmetic
gives **13**.

### 9.3 Content share, both readings

`03` §2.3's number is the product's positioning claim, so it is re-measured
honestly rather than quietly:

| At 1280 | The wall | The wall + the lane |
|---|---:|---:|
| today | **91.6 %** | 91.6 % |
| collapsed | 84.1 % | **91.6 %** |
| expanded | 69.7 % | **91.6 %** |

The wall's own share falls. **The user's own content share does not move at
all**, because every pixel the lane takes from the wall it gives back as
covers — the lane's only chrome is one caps-tracked word and two 24 px
marks. That is the honest reading, and it is the reason this surface is not
the thing `03` §2.3 was warning about: the tradition it measured spends its
window on *empty containers*, and this one spends it on the user's records.

### 9.4 The alternative the owner may prefer: `Place::Home`

Drawn so that overruling §3.2 is a decision rather than a redesign.

```
┌──────────────┬───────────────────────────────────────────────┬─────┐
│ Recent       │  ─── CONTINUE ──────────────────────────────  │     │
│ ┌──┐ Violet  │   ┌──┐ Anhydrous 2            [ ▶ Resume ]     │     │
│ │48│ Ledger  │   │64│ Violet Ledger · 3:12 of 6:27            │     │
│ └──┘         │   └──┘                                         │     │
│ ┌──┐ Road    │  ─── RECENTLY ADDED ────────────────────────   │     │
│ │▨▨│ Trip    │   ▫▫▫▫  ▫▫▫▫  ▫▫▫▫  ▫▫▫▫                       │     │
│ └──┘         │                                                │     │
│  ⋮           │  ─── YOUR LISTS ────────────────────────────   │     │
│              │   ▨▨  ▨▨  ▨▨                                   │     │
│ ▮▯           │                                                │     │
└──────────────┴───────────────────────────────────────────────┴─────┘
                 and `Library` becomes a destination you navigate to
```

**What it costs, stated:** a fifth place; a route back to the wall, which
means either a `Library` row in the lane (the lane's first destination, and
its second subject) or a door in the strip (whose L9 budget is spent); the
launch frame stops being the collection, which is the `VISION.md` pillar
and `03` §2.3's number; and `YOUR LISTS` duplicates the lane, which is
L8.6's own test. **What it buys:** the owner's word, *page*, and room for
future bands that the head of the body cannot hold.

§3.2's recommendation stands, and this drawing is what it is being
recommended against.

---

## 10. Prior art

The owner asked for Spotify to be taken seriously — *"state of the art"*
for some operations while he dislikes much of its UX — so this section is
precise about which is which. Vendor documentation is preferred; community
sources are marked. `03-interface-prior-art.md`'s findings are cited by
section where they still stand.

### 10.1 Spotify's "Your Library" pane — the one the owner named

Spotify's own documentation, since this is the surface the brief points at.

- **Contents**: songs, albums, playlists, artists, podcasts and shows in one
  saved collection, in the side menu
  ([Spotify — Your Library](https://support.spotify.com/us/article/your-library/)).
  The 2023 desktop redesign is what merged them into one sidebar list
  ([Spotify Newsroom, 2023-06-20](https://newsroom.spotify.com/2023-06-20/spotify-desktop-experience-redesign-your-library-now-playing-views-customize/)).
- **Order**: a dropdown at the top of the list offering *"Recents, Recently
  added, Alphabetically, or By Creator"*, plus a desktop-only drag-and-drop
  **Custom order**; and *"the app saves your sort and filter options for
  your next sessions"*
  ([Spotify — Sort and filter](https://support.spotify.com/us/article/sort-and-filter/)).
- **Filter**: a search-within-collection field plus combinable chips —
  *"choose a filter at the top (ex. Playlists)… you can combine filters"*
  (same page).
- **Collapse**: *"click the 'Your Library' button in the top right hand
  corner to collapse the library"*, and when collapsed *"you'll see your
  playlist icons"* — a cover-art rail
  ([Newsroom](https://newsroom.spotify.com/2023-06-20/spotify-desktop-experience-redesign-your-library-now-playing-views-customize/)).
  It is documented with a keyboard shortcut: **`Alt + Shift + L`, "Toggle
  Your Library Sidebar"**
  ([Spotify — Keyboard shortcuts](https://support.spotify.com/us/article/keyboard-shortcuts/)).
- **Resize**: *"Your Library and Now Playing can both be resized to take up
  more or less of the screen"* (Newsroom). A documented numeric width range
  is **unsourced**.
- **Pinning**: documented on both platforms — *"right-click on it and select
  Pin"* (Sort and filter).

**What baz takes**: the **recents ordering** and the **collapse to covers**,
which are the two things the owner asked for by name. The ordering is the
honest one because every event in it is caused by the person, and Spotify's
own `Recents` is the same idea.

**What baz does not take, and why**:

| Spotify | baz | Reason |
|---|---|---|
| Four sort orders + a control | One order, no control | The order *is* the design (§2.2). A sort dropdown is the one form `REFUSALS.md`'s view-options entry names, and it is the answer to a pane holding four kinds and thousands of rows — baz's holds one kind and about thirty |
| A filter field and chips | Neither | Same reason: the chips exist to separate the four kinds Spotify mixed. baz's lane mixes two kinds that are the same subject, and it is short enough to read |
| Draggable width | Two states | A dragged width is a per-user layout — `03` §4.3's customisable-panel tradition — and it would make every width claim in §9 conditional |
| Pinning | Not at v1 | A pinned set is a second ordering to arbitrate against the first (§11) |

**And the warning Spotify's own redesign supplies.** The most-repeated
complaint about the 2023 sidebar was that it took the window: users
objected to having to *"give up on half of my desktop size to have an
overview"*, and to the full-window library page being discontinued in favour
of sidebar-only navigation
([Spotify Community](https://community.spotify.com/t5/Your-Library/Desktop-New-Your-Library-sidebar/td-p/5571384) [community];
[The Verge](https://www.theverge.com/2023/6/21/23768163/spotify-desktop-app-redesign-your-library-sidebar-now-playing)).
That is exactly the cost §9.3 measures, and it is why this design keeps the
wall as the place, keeps the lane collapsible to 96 px, and does not move
any existing surface into it.

### 10.2 The same surface elsewhere

- **MusicBee** — the **Navigator**, *"like a map of your library… permanently
  located in the Left Sidebar"*, whose nodes include a first-class
  **History** destination driven purely by playback
  ([MusicBee wiki — Navigator](https://musicbee.fandom.com/wiki/Navigator)).
  Nearly every node is toggleable in Layout Preferences
  ([wiki](https://musicbee.fandom.com/wiki/Layout_Preferences)); there is
  **no icon-only collapsed mode** — the panel is open, hidden, or
  auto-hidden ([forum](https://getmusicbee.com/forum/index.php?topic=30377.0)
  [community]). *The relevant finding: the product baz's audience arrives
  from already treats "what I have played" as a left-hand destination.*
- **Roon** — **no persistent left nav at all**. The twelve browsers
  (Overview, Genres, Artists, Albums, Tracks, Composers, Playlists, Tags…)
  live behind a toggled overlay opened by the navigation icon or `Tab`
  ([Roon — Browsers](https://help.roonlabs.com/portal/en/kb/articles/browsers),
  [Keyboard shortcuts](https://help.roonlabs.com/portal/en/kb/articles/keyboard-shortcuts)).
- **Apple Music** (macOS) — a **Library** heading with playlists below, and
  the library rows are **user-editable and reorderable**: *"move the pointer
  over Library in the sidebar, then choose Edit…"*
  ([Apple](https://support.apple.com/guide/music/customize-the-music-window-mus0cec331d6/mac)).
  **Recently Added** is a first-class sort over your own library
  ([Apple](https://support.apple.com/guide/music-web/sort-songs-apdmbb7c96e5/web)).
  baz declines the editability for `03` §4.3's reason — a panel the user
  configures is the tradition whose signature defect is W21 — and takes the
  recency idea.
- **foobar2000** — the **Album List** is a tree-like media-library viewer
  whose whole structure is a user-written title-formatting pattern
  ([HA wiki](https://wiki.hydrogenaudio.org/index.php?title=Foobar2000:Preferences:Album_List)),
  and its position is a user arrangement made in Layout Editing Mode
  ([HA wiki](https://wiki.hydrogenaudio.org/index.php?title=Foobar2000:Layout_Editing_Mode)).
  The left column exists; it is configuration, not design.
- **Plexamp** — a desktop sidebar is **unsourced** in vendor documentation;
  Plex's own web app documents a navigation sidebar whose sources are
  user-customisable
  ([Plex](https://support.plex.tv/articles/200484203-interface-overview/)).
  [community] reports describe Plexamp's desktop UI as a scaled mobile one
  ([Plex forums](https://forums.plex.tv/t/plexamp-navigation-and-closing/902622)).

**The pattern across the five**: the persistent left column is either a
**navigation** surface (Roon's browsers, Apple's library views) or a
**user-configured** one (MusicBee, foobar2000). baz's lane is neither — it
is a list of the user's own objects in one fixed order — which is why
almost none of the guidance about *rails* applies to it directly, and why
its closest relative in the field is the MusicBee node nobody talks about.

### 10.3 Rail and drawer guidance, and what it does and does not govern

- **Material 3** now prefers the rail to the drawer outright, and gives the
  breakpoints: compact windows get a bottom bar, medium windows a collapsed
  rail, expanded windows a rail or drawer
  ([M3 — Navigation rail](https://m3.material.io/components/navigation-rail/guidelines),
  [Navigation drawer](https://m3.material.io/components/navigation-drawer/guidelines)).
  A collapsed rail holds *"three to no more than seven"* destinations and
  *"should not be hidden"*.
- **Apple HIG — Sidebars**: *"a sidebar requires a large amount of vertical
  and horizontal space"*; *"consider letting people hide the sidebar"*;
  *"avoid hiding the sidebar by default to ensure that it remains
  discoverable"*; *"consider automatically hiding and revealing a sidebar
  when its container window resizes"*
  ([Apple](https://developer.apple.com/design/human-interface-guidelines/sidebars)).

Three of these bind and one does not, and the distinction matters:

- **Binds**: expanded by default (§2.4 — the state persists, and the
  shipped default is expanded); auto-collapse on resize (§2.4's
  `SIDEBAR_FLOOR` 1000 regime is exactly this); and the widths (§2.3).
- **Does not bind**: the *"three to seven destinations"* rule. baz's lane
  holds **no destinations at all** — it holds objects you open (§2.8). The
  guidance is about navigation rails, where each icon stands for a section
  of the app; a list of thirty records is a different thing wearing the
  same geometry, and counting it against a seven-item ceiling would be
  reading the shape rather than the content.
- **Apple offers no guidance on an icon-only collapsed sidebar** — its
  collapse model is hide/show, not shrink-to-rail — so §2.6's engagement
  with NN/g is the relevant one, not the HIG.

### 10.4 Hover-to-play on a cover grid: everybody draws it inside the object

- **Apple Music** (macOS): *"Move the pointer over any song or album, then
  click the Play button"* —
  [Apple](https://support.apple.com/guide/music/play-songs-from-your-library-mus36265ad9/mac).
  Drawn on the artwork.
- **Plex Web**: *"hover the mouse over the item poster almost anywhere and
  press the ► button that slides up"* —
  [Plex](https://support.plex.tv/articles/200392126-using-the-library-view/).
  On the poster.
- **Spotify**: the play control sits on the card, corner-anchored, and was
  **retrofitted** from a user request —
  [Spotify Community idea 6135](https://community.spotify.com/t5/Closed-Ideas/Play-button-when-you-hover-over-album-cover-on-playlist-and/idi-p/6135) [community].
- **Tidal** (web): a play icon on hover with *"3 dots … to the right of the
  play icon"* —
  [Tidal](https://support.tidal.com/hc/en-us/articles/115005843325-Web-Player-How-to-Favorite-and-Delete-Content).
- **MusicBee**: **no hover overlay.** Double-click in Album Covers view
  drills down rather than playing; playing an album from that view by
  double-click is an open request —
  [MusicBee wiki](https://musicbee.fandom.com/wiki/Album_Covers_and_Artists_Views),
  [forum 14221](https://getmusicbee.com/forum/index.php?topic=14221.0) [community].
- **foobar2000**: **no hover overlay.** Play is a double-click on a track in
  a playlist; the Album List panel's double-click defaults to
  Expand/Collapse —
  [foobar2000 FAQ](https://www.foobar2000.org/FAQ),
  [HA wiki](https://wiki.hydrogenaudio.org/index.php?title=Foobar2000%3AComponents%2FAlbum_List_Panel_%28foo_uie_albumlist%29).

**Five of five that reveal a play affordance draw it inside the object's own
bounds; the two local-library players the audience arrives from reveal
nothing on hover at all.** That is §4.2's finding, and `03` §7.3 already
priced baz's difference here as a deliberate, medium-risk break with a
load-bearing convention. What this study adds is *why* the convention is
load-bearing: an object's own bounds are the only region a full-bleed grid
can spare.

### 10.5 Timing, if a hover reveal ever were built

- **WCAG 2.1 SC 1.4.13** requires hover-triggered content to be
  **Dismissible**, **Hoverable** and **Persistent** —
  [W3C](https://www.w3.org/WAI/WCAG21/Understanding/content-on-hover-or-focus.html).
- **Nielsen Norman on timing**: feedback within **0.1 s**; reveal after
  **0.3–0.5 s** of rest; collapse after **> 0.5 s** away — and *"the more
  the revealed content obscures other elements, the longer the dwell should
  be"* —
  [NN/g](https://www.nngroup.com/articles/timing-exposing-content/).

A group that covers a whole neighbouring record sits at the long end of
that scale by NN/g's own rule, which is a half-second wait bought with a
timer — against the owner's stated bar of responsiveness.

### 10.6 Add-to-playlist, and album-scoped queueing

| Product | Picker form | Album-level queue verb |
|---|---|---|
| **Spotify** | Inline submenu **with a search field inside it**, shipped after the submenu became unusable at scale — [implemented idea 5031948](https://community.spotify.com/t5/Implemented-Ideas/All-Platforms-Playlists-Search-among-my-playlists-on-quot-add-to/idi-p/5031948) [community] | Removed in the desktop redesign, restored after complaint — [idea 4952753](https://community.spotify.com/t5/Implemented-Ideas/Desktop-Bring-back-add-album-to-queue-option/idi-p/4952753) [community] |
| **Apple Music** | Submenu, creation inside it: *"choose **Add to Playlist > New Playlist**"* — [Apple](https://support.apple.com/guide/music/create-edit-and-delete-playlists-musd5d051981/mac) | `Play Next`, `Add to Queue` on whole albums — [Apple](https://support.apple.com/guide/music/queue-your-songs-musb1e6d1c76/mac) |
| **MusicBee** | `Send To` submenu | A `Play More` submenu holding `Queue Album Next` / `Queue Album Last` — [MusicBee wiki](https://musicbee.fandom.com/wiki/Playback) |
| **YouTube Music** | *"more → **Add to playlist** → **New playlist** or an existing playlist"* — [Google](https://support.google.com/youtubemusic/answer/7205933) | — |
| **foobar2000** | `Add to`, `Insert into`, `Send to` — [HA wiki](https://wiki.hydrogenaudio.org/index.php?title=Foobar2000_Mobile%3AUI%3APlaylist) | — |
| **Plexamp** | — | `Play Next` and `Add to queue` **do the same thing** — a defect reported for years, [Plex forums 599567](https://forums.plex.tv/t/add-to-queue-does-the-same-thing-as-play-next/599567) [community] |

**Four of five put the destination list at the pointer**, one press from the
gesture that started the transfer, and **none of them throws it to the far
edge of the window**. This is where the owner is right that Spotify is
state of the art, and it is §6's whole argument.

Three lessons taken: the destination is pointer-local; creation lives
inside the picker; and **Plexamp is the warning for §4.4** — `Queue album`
(the run, tonight) and `Add to "{current}"` (the file, kept) must stay
visibly and behaviourally distinct.

Not followed: the **submenu**. Material 3 caps nesting at *"one level
deep"* and notes submenus are *"best used on large screens"*
([Material 3](https://m3.material.io/components/menus/guidelines)); Spotify's
own record shows the failure mode; and baz's `+` slots are not menus, so a
submenu would mean two destination surfaces.

**And the card is not a menu, on the era's own distinction**: *"Use an
action sheet — not a menu — to provide choices related to the action people
initiated"*, because *"people expect a menu to appear when they chose to
reveal it"*
([Apple HIG](https://developer.apple.com/design/human-interface-guidelines/action-sheets)).
A pick is exactly that. It is drawn like a menu because baz has one float
mechanism; it is governed like a control surface because its rows are
controls. Apple's *"Minimize the use of modality"*
([HIG](https://developer.apple.com/design/human-interface-guidelines/modality))
is why it takes no scrim, which the ledger refuses anyway.

### 10.7 Depth, back, and showing your place

- **Nielsen Norman on breadcrumbs**: they exist for *"making users aware of
  their current location within the hierarchical structure"* — and
  *"breadcrumbs aren't necessary (or useful) for sites with flat
  hierarchies that are only 1 or 2 levels deep"*
  ([NN/g](https://www.nngroup.com/articles/breadcrumbs/)). baz's tree is one
  level deep from every place (§5.1).
- **Miller columns** *"allow multiple levels of the hierarchy to be open at
  once, and provide a visual representation of the current location"*,
  descending from the NeXTSTEP File Viewer (1986); the 1980 Yale
  attribution is carried by
  [Wikipedia](https://en.wikipedia.org/wiki/Miller_columns) but flagged
  *citation needed* there, so it is reported as attributed rather than
  established. With one level the pattern degenerates to a list beside a
  detail — which is what the lane beside a page already is.
- **Back versus Up**: *"Within your app's task, the Up and Back buttons
  behave identically"*, and Up *"never exits your app"*
  ([Android](https://developer.android.com/guide/navigation/principles)).
  baz has one task and one start destination, so `Esc`-returns-to-Library
  is the standard's own answer (§5.4).
- **The two products the owner named have the weakest history in the
  survey.** Spotify's keyboard-shortcuts page documents **no** back or
  forward shortcut
  ([Spotify](https://support.spotify.com/us/article/keyboard-shortcuts/));
  Apple Music macOS lists none either
  ([Apple](https://support.apple.com/guide/music/keyboard-shortcuts-mus1019/mac)).
  Worth knowing before importing one.
- **Lightroom's Filmstrip** *"located at the bottom of the workspace in
  every module, displays thumbnails of the contents of the folder,
  collection, keyword set, or metadata criteria that is currently selected"*
  ([Adobe](https://helpx.adobe.com/lightroom-classic/help/workspace-basics.html)).
  This is `03` §7.2(5)'s finding and doc 11 P10's named door — and the lane
  is the same idea rotated: the peer set follows you into the detail view.

### 10.8 The evidence that cuts against this study

A prior-art section that only supports its own conclusions is a
rationalisation.

**(a) NN/g says band-A actions do not belong in a context menu**:
*"Contextual menus are not appropriate for actions users rely on
frequently. They are best used for secondary or low-priority options"*
([NN/g](https://www.nngroup.com/articles/contextual-menus-guidelines/)).
Putting on a record is band A. So §4 does **not** claim the tile menu is
the answer to one-gesture play: the primary visible route stays the page's
`Play album`, the menu is the accelerator for the hand that already knows,
and this finding is precisely why §4.6 is presented to the owner rather
than dropped. If band-A play may not live behind a right-click, may not
live on a sleeve and may not live behind a timer, then the modifier press
is the last candidate standing or the two-press price is permanent.

**(b) Icon-only navigation is measurably worse than labelled navigation.**
NN/g: *"A text label must be present alongside an icon"*, labels *"should
be visible at all times"*, and *"don't rely on hover to reveal text
labels"*
([NN/g — Icon Usability](https://www.nngroup.com/articles/icon-usability/));
and hidden navigation was used in only **27 %** of cases against 48–50 %
for visible, with desktop users *"at least 39 % slower when the navigation
was hidden"*
([NN/g — Hamburger menus](https://www.nngroup.com/articles/hamburger-menus/)).
This is the sharpest evidence against the collapsed lane, and §2.6 answers
it at length rather than here: the marks are artwork rather than symbols,
expanded is the default and persists, and nothing lives only in the
collapsed state. The measured figures are also a caution about their own
scope — they were taken over *hidden* navigation, not icon rails, and no
NN/g study isolating an icon rail was found — so they are cited as a
direction of risk rather than as a number this design has to beat.

It is also the sharpest available statement of why §6.1's *"very minor
tip"* is a real defect and not a nitpick: subtlety in the one line that
explains a surface is measured, repeatable harm.

**(c) One convergence worth recording**: NN/g's contextual-menu rule is
*"make sure the commands in contextual menus are also available from the
application's main menu"*
([NN/g](https://www.nngroup.com/articles/contextual-menus/)) — which is doc
09 §5.2's mirror rule, arrived at independently and enforced by a test
(`menu.rs:581`).

---

## 11. Considered and rejected

1. **A hover-revealed verb group on the wall.** §4.2 — 42 / 34 / 22 px of
   clear wall against a 104 px card; a wall that twitches under a crossing
   pointer; a dwell timer against a responsiveness bar. Not a principle
   objection: an answer to the geometry would reopen it.
2. **The context menu, opened on hover.** §4.3 — the card is `opaque` and
   captures presses; a menu that opens itself is no longer an accelerator.
3. **The lane overlaying the wall instead of taking width.** §2.4(a) — at
   1280 an expanded lane would cover 60 % of the first column of covers. A
   surface that permanently hides content is worse than one that re-lays it.
4. **A width tween on the collapse.** §2.4(1) — it would re-resolve
   `Grid::new` on every frame of the tween and pop columns mid-slide. One
   frame is both cheaper and better.
5. **The queue in the lane.** §2.1 — its subject is playback, not things
   you have touched; admitting it is finding 1 reproduced on day one. It
   keeps its door, its ambient continuation line and its place.
6. **A `Library` row, a `Settings` row, or any destination in the lane.**
   §2.8 — that is a nav rail, refused by doc 07 L8.4, and it is the lane's
   second subject.
7. **A sort control or a filter row in the lane.** §10.1 — the answer to a
   pane holding four kinds and thousands of rows; baz's holds one kind and
   about thirty, and a sort dropdown is named in the view-options refusal.
8. **Pinning.** §2.2 — a pinned set is a second ordering the list must
   arbitrate against the first, which is finding 5 arriving as a feature
   request. Worth revisiting if the lane's 24 records turn out to churn
   past something the owner wants held.
9. **A drawn seam or a surface step under the lane.** §2.6 — the grid's
   margin is provably ≥ 40 px at every width, so the gap is already there
   and a line would be ink added to it. The index rail's own posture.
10. **Breadcrumbs, Miller columns, a back stack.** §5.1, §5.4, §10.7.
11. **`Place::Home` as the launch destination.** §3.2, drawn in §9.4 — a
    fifth place, a route back to the wall that is either a nav rail or a
    strip tenant, a launch frame that is not the collection, and a
    `YOUR LISTS` band that duplicates the lane.
12. **The pull on the home band.** §3.1 — the pull is an act you press; an
    unbidden offer is generation without a request.
13. **"Recently played" as a home band.** §3.1 — it is the lane's content,
    and one fact drawn twice is L8.6's own test.
14. **An `Add to playlist ▸` submenu.** §10.6 — nesting guidance, Spotify's
    own failure mode, and it answers only the menu route.
15. **A modal dialog for the pick.** No dialogs in the product, no scrim by
    refusal, and Apple's own guidance is to minimise modality.
16. **Fixing only the picker's copy.** §6.1 — the fallback if the card is
    refused, not the answer: it leaves the 682 px trip, the window-width
    dependence and the 340 px surface untouched.
17. **A search field inside the card.** Spotify's answer at scale
    (§10.6). **Deferred**, not refused: the hoist plus the folder's order
    answers it until someone has more lists than `PICKER_MAX_H` holds, and
    a second text field in a float is a focus-and-dismissal problem worth
    solving when someone has it.
18. **A tooltip on every wall tile teaching the right-press.** §4.5 — a
    thousand objects wearing chrome to carry one sentence.

---

## 12. The proposals, ranked and tiered

| # | Proposal | Cost | ADR | Tier |
|---|---|---|---|---|
| **P1** | **The returns lane** — one subject, last-touched order, two widths, collapse at the foot, `Ctrl+B` (§2) | One view, one ordered list maintained by events, five tokens, two glyphs | 0030 §1–§4 | **adopt** |
| **P2** | **The panel dies** — its three jobs go to the lane and the card (§2.7, §7) | Deletions: `playlist_panel.rs`, `PANEL_W`, `panel_open`, the strip door, `Ctrl+P` | 0030 §5 | **adopt** |
| **P3** | **The picker becomes a card at the pointer** (§6) | One float on `menu::anchor`'s geometry; two tokens | 0031 §1–§2 | **adopt** |
| **P4** | **The card's sentence becomes its heading**, with what is held stated in figures (§6.2) | Strings and two type sizes | 0031 §3 | **adopt** |
| **P5** | **The home band** — `CONTINUE` and `RECENTLY ADDED` at the head of the Library body (§3) | Two bands, both absent-not-empty; **depends on P6** for the first | 0030 §6 | **adopt** |
| **P6** | **Ship ADR-0023 §6's queue snapshot** — persist on exit, restore paused (§3.4) | Already specified and costed there: *"one new persisted snapshot, zero engine changes"* | 0023 §6 | **adopt** — the home band's best content, and it closes W2 |
| **P7** | **`Add to "{current}"` at album scope** on the tile menu (§4.4) | Zero new messages, zero new controls | 0032 §1 | **adopt** |
| **P8** | **`‹ Prev · 4 of 25 · Next ›`** — the step pair states its position (§5.3) | One readout, one reserved token | 0032 §3 | **adopt** |
| **P9** | **Teach the tile menu** in the record page's header note (§4.5) | One string | 0032 §5 | **adopt-modified** — modest by design |
| **P10** | **One press to sound from the wall**, as a modifier press (§4.6) | One arm, one accelerator string | 0032 §4 | **present-to-owner** |
| **P11** | **`Place::Home`** as a real fifth place, drawn in §9.4 | A fifth place, a route back to the wall, the launch frame | 0030 *deliberately not done* | **present-to-owner** |
| **P12** | Hover-revealed verbs; the lane overlaying; the queue in the lane; sort/filter/pinning; breadcrumbs; a back stack | — | 0030 / 0032, *considered and rejected* | **rejected-with-reasons** (§11) |

P1–P9 are one coherent programme. P1+P2 are a single change seen from two
sides, and P5 is the reason to do P6 at last.

---

## 13. The implementation plan

Ordered so the highest-relief change lands first, each step whole and
shippable.

1. **The lane, expanded only.** `views/sidebar.rs`; the returns list as
   state built once at launch and maintained by `TrackStarted` and playlist
   writes; rows from the existing thumbnail cache; `SIDEBAR_W`,
   `SIDEBAR_SLEEVE`, `SIDEBAR_ROW_H` in `theme.rs` with the lattice
   assertions their neighbours carry. `Shelf::grid_width` gains its second
   term, and the 1 px-step width sweep (`app.rs:5980`, `theme.rs:3940`)
   re-runs over 300–2560 in both states — that sweep is the acceptance test
   for this step.
2. **The collapse.** `SIDEBAR_RAIL_W`, the two marks at the foot in the
   density detents' anatomy, two glyphs in `icon.rs` with the sheet's
   coverage and stroke-band tests, the `config.toml` bool, `Ctrl+B`, the
   `SIDEBAR_FLOOR` 1000 regime asserted as const arithmetic. The
   shelf-anchored scroll fix-up (§2.4(2)) lands here and is the step's
   other test: *the shelf at the top of the viewport is the same shelf
   after a re-hang.*
3. **The panel dies** (P2). Deletions only, once the lane holds every
   playlist and receives drops.
4. **The card** (P3, P4). `views/picker.rs` on `menu::anchor`/`extent`'s
   geometry and `app.rs:3236–3249`'s stacking; `PICKER_W` 280,
   `PICKER_MAX_H` 400; `App::escape` peels it first. `picker_order` moves
   unchanged. One test earns its keep: `no_transfer_gesture_opens_a_panel`,
   swept over every message that reached `begin_pick`.
5. **The queue snapshot** (P6). ADR-0023 §6 as written: persist paths,
   cursor and elapsed on exit; restore as `SetQueue` + `Seek`, paused.
6. **The home band** (P5). `CONTINUE` reading the restored snapshot;
   `RECENTLY ADDED` reading `first_seen_ns`; both absent-not-empty; the
   body's head is the band under an empty query and the `Songs` section
   otherwise.
7. **`Add to "{current}"` on the tile menu** (P7). One arm in
   `menu::items`' `Target::Album` branch; the mirror test passes with no
   new `CONTROLS` row.
8. **The position readout** (P8) and **the teacher** (P9). One token, two
   strings.
9. **P10 / P11 if the owner takes them.**

Steps 1–3 are the release; 4 is the one the owner's third complaint names;
5–6 are the home page and the feature it finally justifies.

---

## 14. What this study does not decide

- **Whether playlists join the wall.** ADR-0024 §A2 deferred wall
  membership, rail sorting and search-corpus membership; the lane gives
  playlists a resident home, which weakens the case for the wall but does
  not settle it.
- **Whether the lane ever holds artists.** It holds objects you can open;
  artists are not a place today.
- **Whether the drag ever starts on a tile.** Tiles are not drag sources
  (`drag.rs` pays the queue, the playlist page and the panel — the panel's
  share transfers to the lane).
- **The Marquee lens**, and whether it has a lane at all.
- **`Locate…`**, the playlist repair surface ADR-0024 §3 specified and
  nobody has built.

---

## 15. Summary

The owner sent two briefs and they are one request. *"You can already see
the depth that you've went into with the options on the left"* is a
description of the sidebar the second brief asks for, which is why this
study has a spine instead of four separate answers.

**The returns lane** holds one kind of thing — things you have touched:
every playlist, and the last twenty-four records you played — in one order,
last touched first, with ties broken by name so two launches draw the same
lane. It has no sort control, no filter, no pinning and no destinations,
because each of those is a second subject or a second ordering, and a
second of either is how the last side surface died. It collapses to a 96 px
column of covers and back in one frame, and the reflow problem that killed
the last one is answered structurally rather than mitigated: **the collapse
is the only press in the product that lands outside the wall**, so nothing
on the wall can be mid-gesture when it fires. At two of three measured
widths it does not even change the column count — the covers simply get
25 % bigger, which reads as zoom because that is what it is.

**The panel does not survive it**, and should not: it was carrying three
jobs because there was nowhere else to put them. The lane is a better index
(resident, complete) and a better drop target (always there, not only when
summoned), and the third job — the picker — goes to **a card at the
pointer**, which is the real answer to *"a very minor tip at the very
top"*. That complaint was never about the wording: pressing `Add to
playlist…` throws the destination 682 px across a 1280 px window and
1322 px across a 1920 one, because the surface is anchored to the window
rather than to the gesture. Four of the five products surveyed put it at
the pointer, and on this operation the owner is right that Spotify is state
of the art.

**Home is the head of the wall**, not a fifth place — you are always
already there, and the two facts that survive an honest inventory are
exactly the two the lane cannot carry: what you were in the middle of, and
what is new. The first of them makes ADR-0023 §6 worth building at last: a
queue that restores paused has never shipped because nothing on screen
wanted it, and now something does.

**And the verbs the owner wants beside a hovered sleeve already float
beside a pressed one.** The tile's menu is short one item —
`Add to "{current}"` at album scope, the only verb in his list that exists
nowhere — and one teacher, which goes on the page where the trip it would
have saved was just paid. Revealing them on hover is declined on
measurements rather than on principle: 34 px of clear wall against a 104 px
card, and a wall that would twitch under every pointer crossing it. Whether
sound from the wall should cost one press is his call, and §4.6 hands him
the one candidate that fits every constraint the product actually has,
drawn and priced.

