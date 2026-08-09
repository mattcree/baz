# ADR-0030: The returns lane, and the home band

> ## Amendment (2026-08-09) — the owner's decisions, recorded not argued
>
> `docs/REFUSALS.md`'s preamble: *"The owner's decision is sufficient on its
> own; an entry he reverses gets rewritten to say what was decided and why,
> and that is the whole of the process. Nobody argues with a document to
> change their own product."* This record was written to be decided on, and it
> was. **Four things changed, and the body below is left as it was proposed so
> that what was recommended and what was chosen can both be read.**
>
> **1. Home is a place, not a band.** §3.2 recommended the band at the head of
> the Library's body and §9.4 drew `Place::Home` as the alternative it was
> being recommended against. The owner chose §9.4. What §9.4 priced as the
> cost — *"a fifth place needs a route back to the wall, which is either a nav
> rail or a strip tenant"* — is paid by item 2, which was being built anyway,
> and it costs the strip nothing.
>
> §6's inventory is **unchanged**: `CONTINUE` and `RECENTLY ADDED`, and the
> five refusals beside them, were an argument about *facts* rather than about
> geometry, so moving the surface does not touch them. §9.4's own `YOUR LISTS`
> band is **not** built — it duplicates the lane, which is L8.6's test, and
> nothing about choosing the page revives it.
>
> **2. The lane's head holds three fixed destinations, always.** In his words:
> *"home will appear at the top of the left hand sidebar always either way and
> it will contain the top level concerns. think spotify"*, and *"as an
> extension we will want a Now playing page at the top with the Home and
> Library"*.
>
> This **reverses §1's refusal of destination rows** (*"any destination row
> (`Library`, `Settings`) — that is a nav rail, refused by doc 07 L8.4"*). The
> concession is recorded rather than smoothed over: the head is a *second
> subject* in a surface whose whole defence was that it had one. What limits
> the damage is the shape it was given — **a closed set of exactly three**,
> above a hairline, with the list below it still holding one subject and one
> order. A fourth destination is the nav rail L8.4 refused, and it is not
> admitted by this amendment.
>
> **3. `Place::NowPlaying` is a seventh member**, from the same sentence.
> `docs/design/12-now-playing-and-kiosk.md` (unfinished) argued this surface
> for its own reasons; what ships is a first version, and its measures are
> derived from the viewport so the kiosk mode is the same surface larger.
>
> **4. The playlist panel stays.** §5 removed it. It cannot go yet: ADR-0031's
> card at the pointer is not built, and the panel is still the picker for
> `Add to…`. Only its **strip door** is removed, which is what §5's argument
> actually rests on — the lane is the resident index, so a labelled door to a
> second index is two controls answering one question. Lists appearing in both
> the lane and the panel is an accepted transitional state.
>
> Then the owner looked at the panel and said it *"might be alright for
> keeps"*, and asked for two things on it: rows that visibly answer the
> pointer, and `New playlist` as a **ghost playlist row** that becomes a field
> in place with a `Save` control. Both shipped. §5's *"nothing remains"* is
> therefore wrong as written; the panel has a future, and this record no longer
> claims otherwise.
>
> **What did not change**, because he did not touch it: `Place::Library` is
> still the launch frame (`VISION.md`'s first pillar) and still what
> <kbd>Esc</kbd> returns to. §1's membership and order, §2's widths, §3's hard
> cut and §4's responsiveness contract are all built exactly as written.
>
> **Shipped 2026-08-09**, in four commits: the lane, the panel's ghost row and
> the row-hover correction, `Place::Home`, `Place::NowPlaying`. Captures and
> the measured column-count table are at
> [`docs/design/impl/lane-and-home/`](../design/impl/lane-and-home/README.md).

**Status**: accepted and shipped, as amended above (2026-08-09) · extracts the decisions of
[`docs/design/13-everyday-flow.md`](../design/13-everyday-flow.md) §2, §3,
§5 and §7 · **supersedes `docs/REFUSALS.md`'s no-resident-side-surfaces
entry and `11-jobs-era-critique.md` P10** · **restates ADR-0022's
one-place-at-a-time sentence** · **removes the playlist panel's strip door**
(the panel itself stays — see the amendment) · gives ADR-0023 §6's unbuilt
queue snapshot a surface · adds one resident surface, five tokens, two
glyphs and one persisted bool; no engine command, no protocol message ·
the owner's brief, verbatim: *"let's do the ground work for adding a home
page and left hand side bar. the side bar will have recent albums and
playlists mixed based on some order. we can collapse it into only an icon
list. similar to Spotify"*

## Context

The owner has asked for a home page and a left sidebar. That decision is
his, and this record does not argue it — it designs it. The whole of the
ceremony is the next paragraph.

`docs/REFUSALS.md` said *"baz has no resident side surfaces, and no surface
that is a slot"*, and doc 11 P10 declined to restore a persistent left
column partly because the owner had rejected sidebars twice. What was
rejected twice was a **slot**: a 340 px column that showed the selected
album, then the queue, then Settings — three unrelated tenants, arbitration
state, and a re-hang of the wall every time a tile was pressed. What is
built here has one subject, one list, one order and no arbitration. The
entry is superseded by this record; the five findings that killed the rail
(ADR-0024 §5) survive as **engineering lessons**, which is now the whole of
their value:

| What killed the rail | How this lane avoids it |
|---|---|
| Three unrelated tenants | One subject — *things you have touched* (§1) |
| A paragraph of dismissal | One control, two states, one key (§3) |
| The wrong tenant paying resident width | Its tenants are W4 *browse* and W12 *get back to what you were in* — both band A (`03` §1.2) |
| A gesture-breaking reflow | The collapse is the only press that lands outside the wall (§3) |
| Arbitration state | None: there is nothing to arbitrate between |

The bar this record is written to is the owner's own: *"hard rules to me are
mostly about responsiveness and a nice aesthetic"*. §4 is the
responsiveness contract and it is stated in arithmetic rather than in
intent.

**ADR-0022's foundational sentence changes**, and that is the largest thing
in this record:

> **The window holds one place at a time, with the returns lane to its left
> in every place but Settings, the index rail at the wall's right edge in
> Library as always, and the now-playing bar under all of them.**

## Decision

### 1. The lane's subject, membership and order

> **The lane's subject is *things you have touched*: records you have
> played, and lists you have made or edited. Its order is when you last
> touched them. Nothing else is admitted, ever.**

The subject is what closes the slot: membership is a predicate over the
user's own actions, so there is nothing to decide per frame and nothing to
arbitrate.

- **Membership**: **every playlist, always** — the lane is the complete
  index of lists, which is what lets the panel go (§5) without any list
  becoming unreachable — and **the last `RECENT_ALBUMS` = 24 records
  played**, newest first.
- **Order**: last touched, newest first. A record is touched when a play of
  any of its tracks is recorded in the ledger (ADR-0018); a playlist is
  touched when it is played, or when its file is written by the user's own
  edit. **Ties break by name ascending**, so the order is total and two
  launches over the same data draw the same lane.
- **No score, no decay, no weighting, no blend.** This is the
  anti-invisible-pool rule applied to an ordering, and it is the difference
  between this lane and the surface Spotify puts in the same pixels
  (doc 13 §10.1).
- **What falls off the end** is the twenty-fifth-most-recent record.
  Nothing is lost: every record is on the wall.

**Refused, and each for the same reason** — a second ordering or a second
subject is how the last one died: **the queue** (its subject is playback;
it keeps its labelled door in the bar, its ambient continuation line and
its place); **any destination row** (`Library`, `Settings` — that is a nav
rail, refused by doc 07 L8.4); **a sort control or filter row** (the order
*is* the design, and a sort dropdown is the form `REFUSALS.md`'s
view-options entry names); **pinning** (a second ordering to arbitrate
against the first).

### 2. The two widths

| Token | Value | Derivation |
|---|---:|---|
| `SIDEBAR_W` | **280** | `GAP_XL` 24 + lane 232 + `GAP_XL` 24; the lane is `MENU_W` 232, the product's existing float measure |
| `SIDEBAR_RAIL_W` | **96** | `GAP_XL` 24 + `SIDEBAR_SLEEVE` 48 + `GAP_XL` 24 |
| `SIDEBAR_SLEEVE` | **48** | one step above `PANEL_SLEEVE` 40: collapsed, the sleeve is the only thing identifying the row |
| `SIDEBAR_ROW_H` | **64** | 48 + 2 × `GAP_SM`; above L7's floor, and it holds `LINE_BODY` 20 over `LINE_META` 16 centred |
| `SIDEBAR_FLOOR` | **1000** | the smallest window at which the expanded lane leaves the wall two columns at or above `ART_MIN` 240 (988, on the lattice) |

Both widths were derived from baz's tokens and land on the industry's:
Material's drawer is 280 dp at its default maximum and its rail 96 dp under
the expressive update.

**One anatomy for both kinds of row** — sleeve, name, one quiet line (the
album artist; a playlist's counts) — and **nothing marks which kind a row
is**, because the sleeve already does: a record wears its cover, a playlist
wears the 2 × 2 collage of the records it quotes (ADR-0024 §A1). That is
what makes a mixed list read as one list.

### 3. The collapse, and the one press that may re-hang the wall

`Shelf::grid_width` becomes `window_w − sidebar − INDEX_LANE_W`, so
collapsing re-hangs the grid. That cannot be designed away — the column
count is a clamp over `ART_TARGET` and `ART_MIN` and any width delta can
cross a boundary at some window size. The rule that replaces *"no press
re-hangs the collection"*:

> **No press re-hangs the collection except the one press whose subject is
> the collection's width — and that press lands outside the wall, so no
> gesture on the wall can be in flight when it fires.**

This is structural, not a mitigation. The failure the old rule was written
against was a press *on a tile* re-laying the grid under the pointer, so
the second press of a double-click landed on a different record; the
collapse control is in the lane's foot, and nothing on the wall is
mid-gesture when it is pressed.

Three details carry the feel:

1. **Hard cut, one frame. No tween.** ADR-0020 §2.4's 150 ms width tween is
   not forbidden but must not return here: tweening would re-resolve
   `Grid::new` on every frame and pop columns mid-slide. One frame is
   cheaper *and* better, and immediate is what the owner asked for.
2. **The wall keeps its shelf, not its pixel offset.** After the re-hang
   the wall scrolls so the shelf that was at the top of the viewport is
   still there (`Shelves::run_at` already maps offset → run).
3. **The last-opened record's 2 px rule** is drawn from data, not geometry,
   so it is still on the right tile afterwards — which is the anchor the
   eye uses.

**The control** is two marks at the foot of the lane in the density
detents' exact anatomy (ADR-0028): `STEPPER_HIT` 24 boxes, the current
state's mark at full glyph ink and **inert** (it is the fact), the other at
the resting ink and pressable (it is the control), each tooltipped with its
state's name — `Expanded`, `Collapsed`. Two new self-depicting glyphs join
`icon.rs`: a rectangle with a wide left band, and one with a narrow left
band. The two view controls in the product now stand at the feet of the two
lanes, one either side of the wall.

**`Ctrl+B` returns** as the accelerator. Doc 07 §5.3 deleted it because
*"its subject was a sidebar that no longer exists"*; its subject has
returned and its meaning is unchanged, so the key returns with it.

**Persistence**: one bool in `config.toml`, beside the density step and the
group key. **No Settings row** — ADR-0017 §1.3 stands. **Below
`SIDEBAR_FLOOR` 1000** the lane is collapsed and the `Expanded` mark is
inert.

### 4. The responsiveness contract

| Cost | Answer |
|---|---|
| Idle CPU | **Zero.** No subscription, no tween, no clock — a pure projection, as the index rail is |
| Per frame | 9 rows at 860 px, 13 at 1080; and the wall draws fewer cover pixels than before, so the frame gets cheaper |
| The ledger | **Read once, at launch.** The list is then maintained by events: a `TrackStarted` updates one entry and re-sorts 24; a playlist write updates one. **Never a per-frame file read, and no watcher** |
| Thumbnails | The wall's existing cache, decode path, LRU and gradient placeholder (ADR-0024 §A1) |
| The re-hang | One `Grid::new` — six multiplications and a floor — plus one scroll fix-up. One frame |

### 5. The playlist panel is removed

Its three jobs go elsewhere: the **index** to the lane (resident and
complete, where the panel had to be summoned); the **drop target** to the
lane (always available, where the panel had to be open before the drag
began); the **picker** to ADR-0031's card at the pointer. Nothing remains.

`views/playlist_panel.rs`, `PANEL_W`, the `panel_open` state, the
`Playlists` strip door and `Ctrl+P` all go, and `REFUSALS.md`'s *"one
summoned, single-tenant panel exists"* sentence goes with them. `New
playlist` moves into the card and to the lane's foot; `Save as playlist` on
the queue place was never the panel's and is untouched.

### 6. The home band

**Home is the head of the Library place's body, not a fifth place.** Under
an empty query the body leads with the home band; under a query the slot
belongs to the `Songs` section (doc 09 §5). **Never both, never neither.**

Only two facts survive an honest inventory, and they are exactly the two
the lane cannot carry:

- **`CONTINUE`** — the interrupted run: the track, its record and artist,
  the position in figures, and a `Resume` control that is the ordinary
  `Play`. This requires **ADR-0023 §6's queue snapshot**, which is
  specified, costed there at *"one new persisted snapshot, zero engine
  changes"*, and unbuilt. It restores **paused**; nothing sounds unasked.
- **`RECENTLY ADDED`** — one row of the wall's own tiles at the current
  density, newest `first_seen_ns` first (`baz-core/src/index.rs`).

Refused from the band: **recently played** and **playlists** (the lane's
content — one fact drawn twice is L8.6's test); **the pull** (an act you
press; an unbidden offer is generation without a request); and every
engagement statistic, which is not close.

**The rules**: a band is **absent, not empty**; `CONTINUE` is absent with no
snapshot or unresolvable files; `RECENTLY ADDED` is absent when the library
holds fewer than `2 × columns` records, or when every row was created by
one first scan; both scroll away with the wall.

## Consequences

- **One resident surface**, present in Library, Album, Queue and Playlist,
  absent in Settings — ADR-0024 §5's clause 5, inherited verbatim.
- **`Shelf::grid_width` gains its second term.** The 1 px-step width sweeps
  (`app.rs:5980`, `theme.rs:3940`) re-run over 300–2560 **in both states**;
  that sweep is the acceptance test for the lane.
- **A new test**: *the shelf at the top of the viewport is the same shelf
  after a re-hang.*
- **Five tokens, two glyphs, one persisted bool, one keybinding restored.**
- **Deletions**: the whole of `views/playlist_panel.rs` and its state.
- **ADR-0023 §6 ships at last**, because something on screen finally wants
  it. It closes prior art's W2.
- **Content share**, re-measured honestly (doc 13 §9.3): at 1280 the wall's
  own share falls from 91.6 % to 84.1 % collapsed and 69.7 % expanded — and
  the **user's own content share does not move at all**, because every
  pixel the lane takes it gives back as covers. The lane's only chrome is
  one caps-tracked word and two 24 px marks.

## Deliberately not done

- **No `Place::Home`.** Drawn in doc 13 §9.4 with its costs, and presented
  to the owner there rather than decided here: a fifth place needs a route
  back to the wall, which is either a nav rail or a strip tenant, and the
  launch frame stops being the collection.
- **No draggable width.** A dragged width is a per-user layout — `03`
  §4.3's customisable-panel tradition — and it would make every width claim
  conditional.
- **No artists, no genres, no destinations in the lane.**
- **No lane in Settings.**
- **No second home band** beyond the two §6 admits; a third needs an
  argument that beats the L8.6 test the other five failed.

## Considered and rejected

Full list with the arguments: doc 13 §11. In brief — the lane overlaying
the wall rather than taking width (at 1280 it would cover 60 % of the first
column of covers); a width tween on the collapse; the queue in the lane; a
sort control, a filter row, or pinning; a drawn seam or a surface step
under the lane (the grid's margin is provably ≥ 40 px at every width, so
the gap is already there); "recently played" as a home band; and the pull
on the home band.
