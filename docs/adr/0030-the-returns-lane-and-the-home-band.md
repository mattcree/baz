# ADR-0030: The returns lane, and the home band

> ## Amendment (2026-08-09) — the owner's decisions, recorded not argued
>
> the product's preamble: *"The owner's decision is sufficient on its
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
>
> ## Second amendment (2026-08-09) — the search well moves into the lane
>
> The owner, looking at the shipped frames: *"the design does not match
> properly… the search should really be in the sidebar"*. **The well is the
> lane's now**, and §1's *"nothing else is admitted, ever"* is amended a second
> time. The concession is recorded rather than smoothed over, in the same shape
> as the first: the head is now a closed set of **three destinations and one
> field**, above the hairline, with the list below it still holding one subject
> and one order. **A fourth destination is still the nav rail L8.4 refused**,
> and the well is not one — see below.
>
> **1. A field, not a `Search` destination.** The two were genuinely different
> and the choice was made on baz's own terms rather than on Spotify's. Spotify
> makes `Search` a place you navigate to; baz has **type-anywhere** (ADR-0017
> §1.2), so any printable key opens the query from anywhere, and a destination
> row would say *go somewhere first* — the opposite of what the product does.
> The well is also as much a *readout* as an input, and a readout of the frame's
> state belongs in the frame's resident surface. It does not navigate and it
> holds no place; it wears the destination anatomy only when collapsed, because
> a 96 px rail has exactly one anatomy.
>
> **2. What it repairs.** The strip and the lane both carried the frame's
> identity, so the eye had two places to start. With the well moved the strip
> carries none: it is the wall's arrangement (five state words) and the wall's
> verbs (three act words), with the application's gear in the corner.
>
> **3. The counts and the match count are re-homed**, and the reason is
> arithmetic. The lane's measure is `SIDEBAR_MEASURE` 232 against the strip's
> 280; the in-well `MATCH_W` 88 slot plus the 44 px text inset would leave the
> query 88 px. Both figures go onto **one always-drawn line under the field** —
> `25 albums · 206 tracks` at rest, `12 of 25 albums` while narrowing — which is
> the lane row's own two-line anatomy and therefore the surface's own. Always
> drawn so a keystroke pushes no `RECENT` row down; left-aligned so the figures
> change in place.
>
> **4. Collapsed, and the one new re-hang.** At `SIDEBAR_RAIL_W` the well is the
> magnifier, tooltipped, lit while a query stands. Pressing it — and <kbd>/</kbd>,
> <kbd>Ctrl</kbd>+<kbd>F</kbd>, and the **first key of a type-anywhere query** —
> opens the lane onto the caret. §3's rule gains a clause, stated rather than
> hidden: *the collection may also be re-hung by the keystroke that opens the
> query*, which is safe for §3's own reason — it lands outside the wall, no
> pointer gesture is in flight, and the wall's contents are being replaced by
> the matches in the same frame. Below `SIDEBAR_FLOOR` the lane cannot open, so
> no mark is drawn there and the strip keeps the well in doc 10 §4.1's form.
> **One home per regime, never two.**
>
> **5. Every road to the query goes to the Library.** The well searches the
> collection, so the collection is what is on screen while you type into it.
> Before this, typing from `Home` or a record's page filled a field that was not
> drawn and narrowed a wall that was not either — a real defect the resident
> well made visible and then fixed.
>
> **What the strip's budget becomes**: `TOP_BAR_SPLIT` 960 → **872**, exact
> rather than rounded; the well's 80 px fluid range deleted as unreachable, so
> the split is the whole of the collapse order; and above `SIDEBAR_FLOOR` the
> strip is one line at every width in either lane state (648 wanted, 720
> narrowest). Frames and the full arithmetic:
> [`docs/design/impl/search-in-lane/`](../design/impl/search-in-lane/README.md).
>
> ## Third amendment (2026-08-09) — `CONTINUE` is the question you ask in the silence
>
> The owner, looking at the shipped band: *"when you click 'continue' and on
> the home thing it does not update to show what is currently playing"*. §6
> specified the band as a reading of the snapshot, and a snapshot is a record
> of where you **were** — so pressing `Resume` left a frozen placard on screen
> while something else was sounding. The first answer drafted for this was a
> band with **two readings**, `CONTINUE` and `NOW PLAYING`, swapping on the
> engine at identical geometry. The owner replaced it with a better one:
> *"in fact, keep it simple with the continue part… once you select resume, it
> just disappears"*, *"or takes you to now playing"*, *"it just reappears when
> you stop the player"*.
>
> **§6's rule for `CONTINUE` is replaced by one predicate:**
>
> > **The band stands whenever there is a run to carry on with and nothing is
> > sounding.** Start anything, anywhere in the product, and it is gone; stop,
> > and it is back, describing where you now are.
>
> §6's *inventory* is untouched for the third time — the two facts are still
> the two facts, and the five refusals still stand. What changes is when one of
> them is on screen and where its content comes from.
>
> **1. It is a predicate, not a lifecycle**, and one function answers it
> (`views::home::standing`). Sounding: no band. **Paused**: the band, describing
> **what you paused** at the engine's own confirmed position — not the launch
> snapshot, which by then names the start of that same track. **A run that
> ended**: no band; this is the one case the word *stopped* does not settle on
> its own, and it goes the other way from a pause, because a run played to its
> end has no *where you stopped* and the product's standing rules states the silence at
> the end of a run as a feature. **Nothing sounded yet**: the launch snapshot,
> which is the only state in which it is read at all.
>
> **2. Why this is better than the two-reading band, and not merely smaller.**
> It has no path where the band is wrongly absent; it is useful after *every*
> stop rather than only after a launch (pause an album halfway, come to Home,
> the way back in is there); and it **deletes an idle cost rather than
> budgeting for one** — a `NOW PLAYING` reading would have wanted a live
> position while the music ran, and a band that is *absent* while the music
> runs wants nothing, so Home carries no subscription and no clock, and the
> needle it draws is one the engine has stopped moving. §6 also has a standing
> reason to prefer it: what is sounding is the bar's job in every place, and
> `Now playing` is a place of its own one row up in the head. A Home band that
> described the sounding track was the same fact in three places at once.
>
> **3. `Resume` starts the run *and* goes to `Now playing`** — the one play
> gesture in the product that navigates, and the exception is deliberate.
> `Play` on a tile, a record's page or a playlist says *play this*, and
> answering it by leaving the surface you are choosing from would be the
> interface taking the wheel; `Resume` says *pick up where I left off*, and the
> place that describes where you are is the answer to it rather than a side
> effect. It is also what makes the disappearance coherent: you are not left
> standing on Home watching a placard go.
>
> **4. Two consequences below the surface**, both repairs. `Now playing` no
> longer answers a start-in-flight with *"Nothing playing."* — a sentence that
> appears and vanishes is read, where a blank that fills is not. And the guard
> that stops a restored run overwriting the interrupted point is now **one pure
> function shared by both writers** (`app::next_snapshot`), stated as *has
> anything sounded* rather than *is a row playing*. That closes two holes the
> narrower reading left: the **exit path**, which wrote unconditionally, so
> opening baz and closing it again without pressing anything spent the position
> anyway; and an **unmounted library**, whose unresolvable snapshot produced no
> queue and was then deleted outright.
>
> Frames — the band present, the band gone, and the same band back on a pause:
> [`docs/design/impl/home-continue/`](../design/impl/home-continue/README.md).
>
> ## Fourth amendment (2026-08-10) — the well's second line, split in two
>
> The owner, looking at the shipped well: *"the album and track count below the
> search bar doesn't look good… maybe this should go into the home as some
> basic stats?"*. The **second amendment's §3 is replaced**, and §6's inventory
> gains its third fact.
>
> The line carried two strings — `25 albums · 206 tracks` at rest and
> `12 of 25 albums` while narrowing — and the second amendment treated them as
> one readout in two states. They are not. **They have different subjects and
> therefore different homes:**
>
> - **The resting counts are a statistic about the collection.** Nothing is
>   being searched while they are on screen, and they were standing in the
>   lane's most valuable space — the block directly above `RECENT`, which is
>   the surface's whole point. A fact about the whole library belongs on the
>   surface whose subject is the whole library.
> - **The match count is feedback about the query**, so it stays with the field
>   that answers it. It does not go to Home, and the reason is not taste: while
>   you are narrowing the collection, Home is not on screen.
>
> **1. The match count goes back inside the field**, right-aligned in a
> reserved `SIDEBAR_MATCH_W` **72** — the lane's own slot, not the strip's
> `MATCH_W` 88. That is what makes it fit where §3 said it would not:
>
> ```text
>   SIDEBAR_MEASURE 232 − SIDEBAR_HEAD_TEXT_X 44 − GAP_MD 12 − 72 = 104 px
>                                                        (with MATCH_W: 88)
> ```
>
> 72 holds `9999 / 9999` measured in the bundled face — a collection ten times
> the owner's — and 104 px is more room than the arrangement §3 rejected would
> have left, which is the whole argument. §3's discipline is kept rather than
> dropped: the slot is fixed and right-aligned so the figures change **in
> place**; it is reserved on the *right* and the query sets from the left, so
> the caret and the first character it lands never move; and the well's block
> is still a fixed height, so no `RECENT` row is pushed down by a keystroke.
>
> **2. `SIDEBAR_WELL_H` falls from 52 to 32** — one control, nothing under it —
> and the list below gets the 20 px back. **Measured** off two runs of the same
> capture script against the binary either side of the change, rather than
> predicted: **11 `RECENT` rows at 1920 × 1080 where the second amendment left
> 10**; **7 either way at 1280 × 860**, where the 20 px buys three eighths of a
> row and no whole one. The second amendment's own note — and doc 13 §9.2's,
> which repeated it — said the well cost a row at 860 and would give that one
> back; the frames say otherwise, and the frames win. With the counts gone the
> placeholder is free to say what the field is for, so it says `Search`.
>
> **3. §6's inventory gains `COLLECTION`, and it is a *footer*.** Four figures:
> albums, artists, tracks, and total playing time in one unit. It is the last
> section on the page, under `RECENTLY ADDED`, because Home's job is to put you
> back into music — `CONTINUE` is the one thing on the page you press, and an
> inventory must not push it down. It is also the only section here that is
> pure statement, and leading with the part you cannot use would be the wrong
> way round.
>
> **This does not reopen §6's refusal of engagement statistics.** The line it
> draws holds, and `COLLECTION` is on the other side of it: every figure
> describes **what you own**, and every one would be identical if the
> application had never been opened. Three candidates were cut *by* that line
> and by L8.6:
>
> - **When the collection was last added to** — `RECENTLY ADDED` is drawn one
>   section above and says it with covers. One fact drawn twice is L8.6's test.
> - **Records never played** — read out of the play ledger, and it changes
>   while you sit looking at it. An engagement statistic wearing an inventory's
>   clothes, which is why it is named here rather than quietly omitted.
> - **Size on disk** — a filesystem fact, and nothing a listener would act on.
>   The record page's `Details` block is where bytes belong.
>
> **4. §4's responsiveness contract is honoured, not excepted.** Three of the
> four figures are a walk over every track, so they are counted where the view
> model is built (`Shelf::rebuild_shelves`) and held as four scalars —
> **one pass per rebuild, zero per frame**, the same shape as the ledger fold
> §4 already licenses.
>
> Frames and the measured row count:
> [`docs/design/impl/home-stats/`](../design/impl/home-stats/README.md).
>
> ## Fifth amendment (2026-08-10) — §2's reason for marking nothing was false
>
> The owner: *"the information heirarchy isn't great to be able to tell the
> difference between an album and a playlist"*. From
> [`docs/design/14-records-and-lists.md`](../design/14-records-and-lists.md)
> §4.1, and adopted as [ADR-0024](0024-playlists.md) §A3.
>
> **§2's *conclusion* stands and its *premise* does not.** The conclusion —
> *one anatomy for both kinds of row, and nothing drawn on a row to mark its
> kind* — is untouched: no badge, no glyph, no corner, no second sleeve shape,
> and `RECENT` still mixes the two kinds in one touch-ordered list rather than
> segregating them (which is what §8.3 of doc 14 records baz as having declined
> on purpose). The premise was *"the sleeve already does"*, and it is **false
> for every playlist of one to three distinct records**: below four,
> `views::playlist_sleeve` draws the first record's cover full-bleed through
> byte-for-byte the widget a record's own row builds, from the same cache at
> the same edge (ADR-0024 §A1 rule 2). That is every playlist
> `Save as playlist` makes from a CD — one record, by construction — and every
> list on its way to four.
>
> **What carries the distinction instead is the line already under the name.**
> A record's row prints its album artist there; a playlist's printed a bare
> integer, `14`, at `SIZE_META` 12 in `paper_faint` — the weakest string in the
> product, sitting in the exact slot where the disambiguation should happen.
> It now prints `Playlist · 14 · 42:10` (ADR-0024 §A3.1). Same widget, same
> size, same ink, same 64 px row: **no geometry changes here at all**, which is
> why this is an amendment to a *reason* rather than to a layout.
>
> Frames — the lane with both kinds in it:
> [`docs/design/impl/records-and-lists/`](../design/impl/records-and-lists/README.md).

> ## Fifth amendment (2026-08-10) — Home gains a third section: the **All songs** tile
>
> The owner: *"again I wanted the Play all, to be more like a tile on the home
> screen, a special 'playlist'"* — and the *again* is recorded, because it had
> been asked for before and not built (`docs/REQUESTS.md` exists for exactly
> that failure).
>
> **§6's inventory gains a third item, and the addition is tested against §6's
> own rule rather than waved through.** §6 admitted a fact to this surface only
> if it was *true, stable and about your music rather than about your
> behaviour*. `All songs` passes on all three: it is the collection, it changes
> only when the library does, and there is nothing in it about what you played
> or how often. It is also the one thing on the page that is **always** there —
> `CONTINUE` is absent most of the time and `RECENTLY ADDED` needs a row's worth
> of records — so it is what makes Home a door rather than a page that is
> sometimes nearly empty.
>
> **It is a tile in the wall's own anatomy**, with a list's own collage sleeve
> (ADR-0024 §A1), and it sits **second**: under `CONTINUE`, above
> `RECENTLY ADDED`, ordered by how *particular* each offer is. The full argument
> — including why the collage rather than a designed face, and why no section
> rule over one tile — is at `crate::views::home`'s `all_songs_tile`.
>
> **The five refusals are untouched.** It is not a suggestion (it is your
> collection, listed), not recently-played, not a playlist row duplicating the
> lane, and carries no engagement statistic. §9.4's `YOUR LISTS` band is still
> not built and this does not revive it: one tile for one list that has no file
> is not an index of the lists that do.
>
> **The strip's `Play all` stays.** The two differ in **scope**, and each states
> its own: `Play all` sits beside the query and the arrangement that decide the
> wall and plays exactly what the wall shows; the tile is on a page that shows
> no wall, so it plays the collection whole. `ACTS_W` is therefore untouched —
> nothing left the strip, so the acts lane's budget does not move a third time
> today.

> ## Sixth amendment (2026-08-10) — the lists get a section, and this reverses his own brief
>
> The owner: *"I guess we need to add playlists into their own section under
> library"*.
>
> **This reverses the sentence this record was written from**, and the reversal
> is recorded rather than smoothed over, because pretending the mixing was
> never wanted would make the design read as an accident being corrected. His
> brief, at the top of this file and unedited: *"the side bar will have recent
> albums and playlists mixed based on some order"*. §1 built exactly that, §2
> gave it its one anatomy, and the fifth amendment defended the mixing on
> purpose — *"`RECENT` still mixes the two kinds in one touch-ordered list
> rather than segregating them"*. He has now looked at the built thing and
> asked for the segregation. He is the authority, so it is built; both
> sentences stay on the record.
>
> **§1's membership rule splits in two. Its order does not.**
>
> > **`PLAYLISTS`** — every list, always, never trimmed.
> > **`RECENT`** — the last `RECENT_ALBUMS` = 24 records played, **and no
> > lists**.
> > Both last touched first, ties by name ascending: the one total key, applied
> > twice rather than replaced.
>
> **1. `RECENT` loses the lists outright**, rather than keeping recently-played
> ones as a head. A list in both sections is one door drawn twice, which is
> L8.6's test and the rule this product applies to every control. It costs
> nothing to obey here: `PLAYLISTS` is *every* list, so the row a
> both-sections arrangement would have carried is the row that is already
> there, one section up.
>
> **2. The order is untouched, and that is a refusal.** Alphabetical is the
> other honest answer for a section that holds *all* of them rather than the
> recent ones, and it was considered and declined: the ask is for a **section**,
> not for a second ordering, and §1's whole argument against arbitration is that
> a surface with two orderings decides between them invisibly, every frame. It
> would also spend something he already had — under the mixed list a list played
> this morning stood near the top, and under last-touched-first it still does.
> It has moved **section, not rank**. If the section ever outgrows what a
> listener can scan by eye, alphabetical is the reopen, and it is named here so
> the next reader does not have to rediscover it.
>
> **3. *"Under library"* is read as under the head**, not literally between
> `Library` and `Now playing`. The first amendment's concession was a **closed
> set of exactly three, always in that order**; a section header between two of
> them would split the one triple this surface is not allowed to grow, and
> would put a list-shaped thing inside the block whose whole defence is that it
> is destinations only. So: the well, the three destinations, the hairline,
> `PLAYLISTS`, `RECENT`, `Collapse`.
>
> **4. The lane still has exactly one seam.** The sections are named by
> headings, not divided by a second rule. §1's shape holds: one cut, because
> there are two parts — the frame's concerns above it and yours below it — and
> a second rule would say the lane had three.
>
> **5. Collapsed, a section heading is nothing**, which is the answer `RECENT`
> has given since it shipped: at `SIDEBAR_RAIL_W` 96 there is no measure for a
> tracked word, and a heading over an unlabelled column of sleeves names
> nothing the eye can use. `PLAYLISTS` takes that answer rather than inventing
> a second one — a rail that grew a mark where the expanded lane has a word
> would be two answers to one question at exactly the width with no room for
> the second. What separates the two runs of sleeves on the rail is the
> sections' own `GAP_MD`, and every row keeps the tooltip that names it.
>
> **6. A section with nothing in it is absent, not empty** — §6's own rule for
> the home band, applied here for its own reason and applied to **both**
> sections, so they stay symmetric. A first run has no lists and no plays, and
> a permanent `PLAYLISTS` over a permanent gap would be chrome that never
> becomes content. `RECENT` drew its word over nothing before this and no
> longer does.
>
> **7. The unbounded section is where the defect would have been, and the
> answer is one scroller.** §1 trims the records at 24 and deliberately does
> not trim the lists, so `PLAYLISTS` can be four times the lane's height. Two
> scrollers — one per section — would give the surface two scroll positions to
> arbitrate between, which is §1's own failure mode, and would cap the lists
> after all at half a lane; a fixed-height `PLAYLISTS` above a scrolling
> `RECENT` would push the records off the bottom of the window at about a dozen
> lists. **Both sections and both headings are inside the one scroller the lane
> has always had**, so the headings scroll with their rows, every row of both
> sections is reachable at any list count, and there is still one scroll
> position and one bar at the lane's own edge. Proved at thirty lists, expanded
> and collapsed, at both widths.
>
> **What this does *not* change**: §2's widths, row height and one-anatomy rule;
> §3's hard cut and `Ctrl+B`; §4's responsiveness contract — the split is a
> partition of a list already built once per event, so it costs no per-frame
> work and no new state; ADR-0034's rule that the lamp follows the run's origin
> (the sections are disjoint, so one origin still marks exactly one row); and
> §6's home band, which still refuses a `YOUR LISTS` band because the lane is
> the index and now says so in a word.
>
> **The playlist panel is not touched, and is not made redundant by this.**
> §5 removed it, the first amendment put it back, and the reason it is still
> here is **simultaneity** — collecting needs the source and the destination on
> screen at once, which a resident lane section does not provide any more than
> a resident mixed list did. What the section *does* settle is the panel's
> third job: the lane was already the complete index and is now a **labelled**
> one. Removing the panel is a separate decision on a separate argument and is
> not taken here.
>
> Frames — both sections, collapsed, a list sounding, and thirty lists with
> `RECENT` still reachable:
> [`docs/design/impl/playlists-section/`](../design/impl/playlists-section/README.md).
>
> *(Numbering note: the two amendments above are both headed "Fifth". They are
> left as they were written — this record's rule is that history is amended,
> not rewritten — so this one is the sixth by sequence and the seventh block.)*

**Status**: accepted and shipped, as amended above (2026-08-09, 2026-08-10) · extracts the decisions of
[`docs/design/13-everyday-flow.md`](../design/13-everyday-flow.md) §2, §3,
§5 and §7 · **supersedes the product's no-resident-side-surfaces
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

the product's standing rules said *"baz has no resident side surfaces, and no surface
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
*is* the design, and a sort dropdown is the form the product's
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
`Playlists` strip door and `Ctrl+P` all go, and the product's *"one
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
