# baz — the request log

> **What the owner asked for, and where it got to.** One line per ask, in his
> own words where they were short enough to keep, with a state and a pointer.
>
> This file exists because of a specific failure, and naming it is the point:
> on 2026-08-09 three asks — remove `Pull`, make shuffle a player mode, make
> *all songs* an implicit playlist — were requested more than once, mapped in
> `BACKLOG.md`, and then reported back to him as *"decisions waiting on the
> owner"* rather than built. They were not decisions. They were instructions,
> and they lived only in conversation, which scrolls away. His verdict:
> *"again we seem to be losing these things. I've mentioned them multiple
> times."*
>
> **The rule this file exists to enforce:** an ask from the owner is written
> here the moment it is made, and it leaves only as **shipped**, or as
> **declined with his agreement**. Nothing else removes a line. If an ask needs
> a decision from him before it can be built, that is a *note on the line*, not
> a reason to drop it.
>
> **This file is the record of asks; [`WORK.md`](WORK.md) is the ordered queue.**
> An ask logged here without an item there is half-tracked.
>
> Related: `CHANGELOG.md` is what landed, `BACKLOG.md` is what was deliberately
> deferred with reasons, `NEXT-STEPS.md` is the ordered plan. This is the one
> that answers *"did you do the thing I asked?"*

## Open

| Ask | State | Where |
|---|---|---|
| The ambient Now playing — cover as the background, stylised VU over it, a feed of facts, all toggle-able; *"a spectrum analyzer or graphic thing with the bars going up and down"* | **designed, unbuilt** | ADR-0029, design 12 — the merge (M1, M2) shipped; A2 · A3 · A4 · A5 · A6 · A7 · A8 · A9 remain, or A2→A6→A8 for the bars |
| Window chrome: buttons right, gear left, borderless | **blocked on a decision** | iced 0.13 exposes no edge-drag resize; needs a forked dependency |
| Kiosk mode — full screen on a second monitor | **designed, unbuilt** | design 12; single window, iced has no monitor enumeration |
| Vibe- or prompt-generated playlists | **designed, unmerged** | `design/dynamic-playlists`: a rule you can say out loud, drawn into the queue |
| *"shuffle... is more about going to an unknown next track rather than actually mutating the track list"* | **building** | traversal, not permutation; gapless is the constraint |
| *"I wanted the Play all to be more like a tile on the home screen, a special 'playlist'"* | **building** | asked twice before it was built |
| *"fullscreen the now playing looks weird"* | **building** | the art is clamped at 720; step A2 deletes the cap |
| *"the information heirarchy isn't great to be able to tell the difference between an album and a playlist"* | **designed, unbuilt** | design 14, ADR-0024 §A3–§A6 — the kind stated in words, the byline line restored; the serif and the sleeve-below-four are his call |
| *"'save as playlist' really makes no sense on the playlist page for a CD"* | **designed, unbuilt** | design 14 §1, ADR-0024 §A5 — it is drawn on the merged Now playing, not the playlist page, and never says it is saving *the run*; nothing is removed |
| *"the album and track count below the search bar doesn't look good... maybe this should go into the home as some basic stats?"* | **building** | resting counts to Home; match count into the field |
| *"every album has a playlist implicitly... which playlist and which track"* | **designing** | everything playing is a list and a cursor; reopens ADR-0018 for the ledger |

## Shipped

Newest first. Each was asked for in conversation and is now in the product.

| Ask | Landed as |
|---|---|
| *"artists should be grouping stuff by artist not just alphabetically"* | `ARTIST` groups albums under their artist; `A–Z`, `WallSubject` and 700 lines went with it |
| *"artists should be grouping stuff by artist not just alphabetically"* | `ARTIST` shelves one artist per shelf, the header a door to their place; `A–Z` and the `ARTISTS` word both gone (ADR-0035) |
| The `ARTIST` group key and the `Artist` place are both called artist | the key groups by artist now, so the word is true and the two are one thing (ADR-0035) |
| *"integrate the queue with now playing so we can remove the queue option from the bottom bar"* | `Place::Queue` deleted, its whole body the merged surface's run column; the bar's door off, its 152 px to the title |
| *"remove pull since it doesn't make sense here"* | gone, with `History::pull_weight` — its only consumer |
| *"shuffle as a concept is more about going to an unknown next track rather than actually mutating the track list"* | a traversal in the engine, not a permutation: the run keeps its own order and the walk is a bag. `crates/baz/src/shuffle.rs` deleted with it |
| *"again I wanted the Play all, to be more like a tile on the home screen, a special 'playlist'"* | an `All songs` tile on Home, second on the page, in the wall's tile anatomy with a list's collage sleeve |
| *"make shuffle a property of the player i.e. toggle on/off"* | player state in the bar, persisted; a mode rather than an act |
| *"the 'all songs' should be an implicit playlist"* | `implicit::ImplicitList` with an `Origin` kind; `Play all` is its `Play` |
| A breadcrumb instead of Prev/Next, and Artists alongside the group keys | `Place::Artist`, `Artist › Album`; the stepper withdrawn — it walked an order you cannot see. *Alongside* the keys became *one of* them (ADR-0035) |
| *"the recent bit shows albums popping up even though it was the playlist which was played"* | a run reified from a list credits the **list** (across a quit still falls back — needs ADR-0018 reopened) |
| *"search belongs at the top"* | the well leads the lane |
| *"the search should really be in the sidebar"* | the well moved out of the strip |
| Remove the nav controls from the playlist and album views | place headers lost `‹ Library` and the Esc hint |
| The lane's scrollbar at its edge; no rectangle round the collapsed `Now playing` | the gutter moved onto the lane's contents; the mark is the glyph's own box |
| *"now playing does not need the play pause controls"* | it was drawn twice; the duplicate and its wrapper are gone |
| `CONTINUE` disappears on resume, takes you to Now playing, returns when you stop | one predicate: the band stands when there is a run and nothing sounds |
| A home page and a left sidebar with recents, collapsible to icons | `Place::Home`, the returns lane, ADR-0030 |
| A Now playing page beside Home and Library | `Place::NowPlaying` |
| Clearer click affordance on playlist rows; `New playlist` as a ghost row with `Save` | the row-hover family, fixed as a system |
| One press to play from the wall, via options over the cover | the hover options; retired the two-press cost |
| *"a very minimal scroll bar because otherwise it's hard to just jump to the end"* | the wall's 4 px bar in its own lane |
| The album art beside the bar's now-playing block | 52 px cover, one control with the text |
| Dock-style magnification on the index rail | the fisheye, 2.5× with displacement |
| A directory picker, and a NAS as a library folder | ADR-0025 |
| Playlists, modelled honestly | ADR-0024 — `.m3u8` files you own |
| *"how users interact to play, create playlists, edit playlists"* | design 08, 09; the whole implicit-playlist epic |
| Rethink control layout and iconography | ADR-0026, design 10 |
| A Jobs-era adversarial critique | design 11 — P1–P6, P8 shipped |
