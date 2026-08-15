# The 2026-08-15 review pass — items 43–51

Ten asks, logged in one run on 2026-08-14 while the owner read a running build,
recorded before any of them was reproduced, and built the next day. This is the
evidence: what each frame shows, what was measured rather than eyeballed, and
the three places where a first fix was wrong and the render said so.

Every frame is the real release binary on a private Xvfb with all six XDG
redirections (`capture.sh`; `docs/DEVELOPMENT.md`'s recipe). Both runs printed
`[mpris] no session bus`, which is the receipt that nothing touched the owner's
session.

| frame | what it is for |
|---|---|
| `01-library-at-rest-1280x860` | the baseline every other frame is read against |
| `02-playlists-wall-1280x860` | items 43–45: no place name, no tally, ghost tile, `Favourites`, then A–Z runs |
| `03-playlists-wall-pinned-heading-1280x860` | item 44: a heading pinned at the viewport edge, by the Library's own `Shelves::sticky` |
| `04-new-playlist-fork-1280x860` | item 45: the ghost tile opens the creation place |
| `05-vibe-form-1280x860` | item 50: describe → shape → compose, consent above the press |
| `06-record-page-two-column-1280x860` | item 46: the cover fixed, its tail scrolling, the sleeve keeping its 320 |
| `07-record-page-short-window-1280x760` | item 46: the tail overrunning a short window, the cover whole |
| `08-record-page-aside-scrolled-1280x760` | item 46: `DETAILS` reached, the bar beside the tail and never the artwork |
| `09-app-bar-clusters-3x` | items 47–48: two seams, and the bell at its neighbours' width |
| `10-bottom-cluster-2x` | item 47: no hole before the mute button |

## Item 43 — the strip says what the Library's says

The owner: *"the playlists page does not need the word 'playlists' at the top"*
and *"no need for the playlist count and another noise."*

It is a **divergence rather than a preference**, which is what made it easy to
settle: the Library's strip (`views::top_bar`) carries arrangement keys and
transient scan status — no place name, no tally — while the playlist wall led
with `place_name("Playlists")` and closed with `13 playlists`. That tally sat
in `place_header_led`'s `note`, whose own doc says it is *"for a statement about
the place, never a keyboard hint"*, with Settings' *"Kept in config.toml…"*
named as its only customer. A count of the tiles in front of you is neither.

Three things went, and one deliberately stayed:

- the word — the place is named by the lit lane destination and by every
  playlist page's `Playlists › Name` breadcrumb;
- the tally;
- the per-tile `Playlist · 12 · 41:03` caption, which spent a line saying
  *playlist* under every tile on a wall of nothing but playlists.
- **The note slot survives for the deletion confirmation** (`Delete “Zed”?`),
  which is a statement about the place: something is pending in it.

`PanelRow::counts` is untouched. It is ADR-0024 §A3.1's rule — *the line under
a name declares its kind in its first token* — and it earns that in the returns
lane and the picker panel, where a made thing's line sits beside a found
thing's and must not be mistaken for an artist's name. On this wall there is
nothing to be mistaken for.

The caption lane itself stays, empty: `theme::CAPTION_H` is the grid's, and a
tile one line shorter than the Library's would break the pitch the two walls
share.

## Item 44 — the wall groups, with the Library's own machinery

*"a-z playlists should group alphabetically — use the exact same pattern as the
library please."*

**Half of it already existed and none of it was visible.** `views::playlists`'
rail computed `GroupHeaderVm::Initial` runs and handed the shared `Spine` each
group's first row — so the rail could jump to boundaries that were drawn
nowhere, over a flat virtualized grid under one `section_rule("All playlists")`.

*"The exact same pattern"* is taken literally: the layout engine is
`shelf::Shelves` (the record wall's own), the heading band and its pinned copy
are `views::shelf::group_band` / `pinned_band` — extracted from the Library's
private ones in this change, so there is **one** band rather than two that
agree — and the projection is `playlists::Playlists::wall`, which both the view
and the artwork scheduler read.

Two things had to be decided rather than assumed:

- **The lead run has no heading and holds two cells**: the create tile and
  `Favourites`. Neither belongs in a letter — one is a control, the other a
  built-in with no creation stamp and no alphabetical place among the
  listener's own lists. An unlabelled leading run is how a wall says so without
  inventing a category with one member. `Wall::pinned` refuses to pin it: the
  pinned layer paints an opaque band, and a blank one would be a strip drawn
  over the covers passing under it.
- **All three orderings group**, because the rail already projected all three.
  `Date created` and `Played` get the Library's elapsed buckets in the flow,
  not just in the rail.

`App::request_playlist_art` reads the same `Wall`. That is not tidiness: a
grouped wall's visible tiles are no longer `scroll / row_h`, because a band
stands between every run, and the flat arithmetic would have decoded the
collages of tiles a screen away while the ones on screen stayed gradients.

## Item 45 — the create affordance is a tile

*"the new playlist should be like a ghost playlist with a + in the middle
called 'New Playlist' on the playlist page, not a button."*

It was a word button in the strip — a control about *making* a thing, filed in
the row that says how the collection is *arranged*. It is now the wall's first
cell, at the same edge, mat, caption block and state-rule lane as a real tile,
so nothing moves when the ghost becomes a list. That is the panel's own ghost
row (`views::playlist_panel`) at wall scale, and it keeps that row's two rules:
the sleeve is `theme::ghost_sleeve` with the drawn `Glyph::Plus` and never
anything resembling artwork, and it answers the pointer like its neighbours.

The mark is drawn at `theme::GHOST_MARK_PX` = `ICON_PX × 2` — the sprite's own
raster edge, so it is pixel-exact rather than an upscale. `icon.rs`'s
`the_raster_size_follows_its_token` pins the equality.

Frame `04` is the tile pressed: the creation place opens.

## Item 46 — the aside could not scroll, and then it took three goes

*"the details on the album view is not scrollable."*

**Two-column form only**, which is the diagnosis: `views::page::view`'s desktop
branch scrolls the track table alone — deliberately, and the reason `TRACKS` is
a sticky head — while the column beside it was a plain container in a
`Fill`-height row. No scroller, no clip of its own. Frame `07` is that column
overrunning a 760 px window; frame `08` is the same page after three wheel
clicks over the tail, with `DETAILS` — album artist, released, genre, tracks,
format, depth, sample rate, bitrate, size, ReplayGain, added, folder — reached.
Before this change none of that was reachable at that height by any gesture.

**The render then caught three things the first fixes got wrong**, and they are
worth recording because none of them is visible in a passing test:

1. **iced clips a scrollable's content at the viewport edge**, rather than
   painting the bar over it as the note in the first draft claimed. At the
   aside's own 320 the sleeve lost nine pixels. `theme::ALBUM_ASIDE_LANE` now
   declares the column's lane — the aside, `ALBUM_ASIDE_INSET` 2, and the bar —
   and the *measure beside it* yields, which costs nothing at any width where
   the list has already reached `LIST_MEASURE`.
2. **A `Length::Fill` child in a `Shrink` column resolves against the parent**,
   not against its siblings. The aside column was `Shrink`, so `Play album`
   stretched past the 320 px sleeve to the viewport's edge and lost its right
   border to the clip — three sides of a rounded rectangle, on the record
   page's one commitment. The column states its width now. Measured at
   x = 591 in `06`: amber, inside the clip.
3. **The bar ran down the artwork**, because that fix scrolled the column
   whole. The owner: *"scroll bar being on the whole section for album image
   and details in the album view looks bad. the image should not scroll."*
   The cover is fixed now and everything under it scrolls.

   **The fixed half is the cover alone**, and that is arithmetic rather than
   taste: the cover, `Play album` and the acts come to 424 px, while a 620 px
   window gives the composed row 361 — so a fixed head of all three put the
   commitment past the body's edge with nothing able to scroll it, which is
   item 46's own defect one block higher. The cover alone is 320. Both figures
   are asserted, so the day they change the reason is on screen rather than in
   someone's memory. One consequence, stated: at very short windows the tail's
   viewport is a sliver — about 60 px at 620 — so its reachability is real but
   tight; at 760 (frames `07`/`08`) it is 170 px and reads normally.

## The heart on the built-in list

*"can we create a default heart image on the playlists."* `Favourites` drew
the rest tile — the surface step with the list's **name** in it — whenever it
held no records to quote. That is the honest thing for a list the listener
made (there is nothing else true about it yet) and the wrong thing for a
built-in whose subject is known before it holds anything. It wears the heart
now, in the same slot at every size and in every surface that draws a list's
sleeve: the wall tile, the returns lane, the picker panel.
`views::default_playlist_mark` is the one decision, so the three cannot drift.
Frame `02`.

## Item 47 — the justification was already right; the gap was a reserved slot

*"can you make sure the player controls on the bottom are right justified.
there seems to be a gap between controls and the mute button. the top bar has
weird spacing as well for icons/controls."*

The bottom cluster **was** right-aligned (`align_x(Right)` against a `Fill`
identity zone, hanging on `BAR_EDGE_PAD` 14). What he was seeing was
`signal_path` returning a `Space` of `SIGNAL_W` **96** whenever the chain is
direct — which is every ordinary run — standing *between* `Shuffle` and the
mute button.

**The reservation is right and the position was wrong.** A note that appeared
mid-run and shoved the volume sideways is movement on the one surface ADR-0020
forbids it on. At the cluster's *leading* edge the same reservation abuts the
identity zone's `Length::Fill`, which is empty space anyway: invisible while
empty, and still moving nothing when it fills. Frame `10` is the result.

`BAR_TRAILING_W` is unchanged at 636 — the `GAP_SM` the signal path gave back
to the volume is exactly what pairing `Repeat` with `Shuffle` on the cluster
seam saves — so `bar_title_lane_w` and every figure derived from it stand.

**The app bar had three rhythms for one kind of object**: `GAP_XS` 4 inside the
history pair and the window buttons, `GAP_LG` 16 between the bell and the gear
— the *between*-clusters number spent inside one cluster. The bottom bar had
been on 8-inside/16-between all along, so that is the rule, and it is now a
token: `theme::CONTROL_CLUSTER_GAP`. A detent run (`density_marks`,
`visualizer::marks`) still touches, because it is one control with several
states rather than a cluster of controls. Frame `09` at 3×.

The budget moved with the geometry rather than being renumbered:
`APP_BAR_HISTORY_W` 84 → 88, `APP_BAR_BUTTONS_W` 128 → 136,
`APP_BAR_FURNITURE_W` 272 → 264, `APP_BAR_LINE` 850 → **854**, and
`WINDOW_FLOOR_W` 860 → **864** by its own derivation, which keeps the stated
10 px of slack exactly.

**What was *not* done, and why.** The app bar's own empty reserved slot —
`APP_BAR_MARKS_W` **160**, held in every place that hangs no works — was left
alone. It already abuts the drag gap's `Length::Fill`, so an empty slot there
is invisible for the same reason the bottom bar's now is; frame `04` is a place
with no display options and no visible hole. Collapsing it would make the bar's
right cluster slide 160 px as you walk between places, which is the failure the
reservation exists to prevent.

## Item 48 — the bell was the narrowest mark in its cluster

*"the bell icon is a little bit narrow/skinny."*

Measured off the sheet rather than judged: `BELL`'s widest point was its rim at
`0.160 → 0.840` = **0.68** of the em box, against `GEAR` 0.84 and `HOME` /
`NOW_PLAYING` 0.88 in the identical `ICON_PX` 20 box — about 13.6 px of ink
where the gear one seam away lays 16.8.

The mouth is **0.78** now, by a 1.147 scale about the vertical axis: the dome,
the flare's shoulders and the rim keep their proportions to each other exactly,
because the profile was right and only its width was wrong. The height is
untouched at 0.79, so the bell stays no wider than it is tall.
`the_bell_is_as_wide_as_the_cluster_it_stands_in` holds it in the neighbours'
range from below and against its own height from above.

## Item 49 — the bar drew nothing where the page draws words

*"some albums do not show the album details in the bottom bar now playing even
though the album page shows it."*

Two readings, and the first is not a bug: **the bar has never drawn an album
title**. `bottom_bar::now_playing_line` is three lanes — title, artist,
continuation — so a record reaches the bar as its sleeve and as *"then 2
albums"* and in no other way. Naming the album there is a composition change on
the one surface whose geometry may not move, and it is left for the owner to
ask for.

The second reading had a mechanism, and it is fixed. `AlbumArtistVm::name()` is
`None` for a compilation (`Various`) and for an untagged record (`Unknown`),
and the bar and the Now playing placard both stopped there and drew an empty
lane — while the album page, the wall tile and the picker panel all draw
`label()`, which always says something. So a compilation whose file carried no
artist tag lost its artist line **in the bar only**: exactly *"the album page
shows it, the bar doesn't"*.

`NowPlaying::artist_line` is the one answer now — the track's own tag, then the
album's artist, then who the record is filed under — and both surfaces call it.
`NowPlaying::artist` is untouched, because MPRIS publishes it as `albumArtist`
and baz's placeholder words are not an artist's name; the test asserts that
boundary as well as the fix.

## Item 50 — the Vibe flow

*"we need to examine the flow for the vibe playlist. the ux is terrible and it
makes no sense right now."*

Six faults were visible in the source before anything was run, and each is
answered in `views::new_playlist`'s module docs. The two that mattered most:

- **The order was inverted.** `Shape the journey` — the energy shape and the
  waypoints, which exist to inform the request — stood *below* the button that
  spends the request, and `Save playlist` stood *above* the name field it
  needs. Frame `05` is the order now: describe, shape, compose; then review,
  name, save.
- **The consent gate stood in the middle of the flow**: prompt → `Create mix` →
  a paragraph → a second, differently named button. The engine never needed two
  presses — `Message::VibeCreate` already starts the analysis and composes when
  it lands, through `awaiting_create` — so the paragraph moved *above* the
  press, where consent belongs, and the second button is gone. `VibeCancel` had
  no sender left and went with it: a message no control sends is the
  visible-control rule failing in the direction nobody checks.

Also: one vocabulary (`Make a mix` / `Create mix` / `Another version` are gone;
the place makes a **playlist**, the route **composes**), Manual and Vibe drawing
the same `draft_row` (the shared track row, the favourite slot and the icon
slots — Manual's bare `Up | Down | Remove` word buttons are gone), the composer
moved out of `views::home`, and the analysis-failure note in the room's **alert**
ink rather than its accent, which it had been riding into that file on Home's
permit for the `CONTINUE` needle.

## Item 51 — the consistency pass

`docs/design/06-composition-audit.md` §9 carries the inventory this pass
produced and a verdict per divergence, including the three it did not close.
