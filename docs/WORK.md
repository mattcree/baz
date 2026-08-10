# baz — the work queue

> **This file answers one question: what is next?** Read the top of the *Next*
> list below and start. If you are an agent picking this project up cold, this
> is the only file you need to begin; everything else explains *why*.
>
> **The rule.** Every item here is one of four states, and an item leaves only
> by being **done** or by the owner saying it should go. "Blocked on a
> decision" is a *note on the item*, never a reason to delete it — that failure
> has happened, and this file exists because of it.
>
> | state | means |
> |---|---|
> | **next** | ready to start, in the order listed |
> | **doing** | an agent has it right now |
> | **waiting** | needs a decision from the owner, named on the item |
> | **done** | shipped and on `main` |
>
> **Every agent updates this file in the commit that lands its work** — moving
> its item to done and adding anything it discovered. A branch that changes the
> product and not this file is incomplete.
>
> Where the other documents fit: `CHANGELOG.md` is what shipped, `BACKLOG.md`
> is what was deliberately *not* done and why, `NEXT-STEPS.md` is the shape of
> the project, `REQUESTS.md` is the owner's asks verbatim with their fate.
> **This is the ordered queue.** If they disagree, this one is wrong — fix it.

## Next

1. **Doc 12 step A4 — `RUN_MEASURE` scaled by `kiosk_scale`.** Visible in a
   committed frame: at 2560 with the run standing, ~700 px of field sits
   between the sleeve and the run column, because the record column hangs left
   and the run stays 440 wide. A4 takes it to ~1100 at that size.
2. **Cut v0.1.** Nothing is installable. The icon, the release rehearsal and
   the Flatpak build are all done; what is left is a screenshot for the
   metainfo, the version edit from `0.0.0`, a `workflow_dispatch` dry run,
   then the tag. **The tag is the owner's to cut** — the workflow produces a
   draft.
3. **Rewrite the README as the project's public face**, with the icon and real
   screenshots of the wall, Home, Now playing and a playlist. Deliberately
   last, so it describes what actually ships. Its keyboard table is still
   stale: `Pull` is gone, `Q` never opened the queue, shuffle is a mode,
   `Ctrl+B` exists. (The group-key row itself is current again — the six words
   and `1`–`6` were corrected when `A–Z` came back.)

## Doing

Nothing. The queue above is the next thing.

## Waiting on the owner

- **Borderless window chrome.** Wayland already draws that title bar inside
  baz's own process, so turning it off is one field — but **iced 0.13 exposes
  no edge-drag resize anywhere in `window::Action`**, so going borderless today
  loses pointer resizing. The route is a ~30-line upstream-shaped iced patch,
  which means a forked dependency. *Needs: yes or no to the fork.*
- **Doc 14's Tier 3**, three questions rather than tasks. Tiers 1 and 2 both
  shipped without touching any of them, and each needs one sentence from him.
  The first has got **sharper** rather than softer now that tier 2 has landed:
  the serif is on a record's page, so he can see the face at 28 px in the
  product before answering whether it should also be on sixty tile captions at
  13 px — `docs/design/impl/serif-titles/` has it magnified. Nothing in tier 2
  presumes an answer; the wall and the lane are untouched.
  - **Should a record's title be set in serif italic everywhere it is named —
    the wall's tile captions and the lane's rows included?** *Needs: his eye on
    a frame, not an argument.* It is the strongest possible answer to his own
    question — every record typographically a *work*, every playlist a
    *label*, at every size, with no badge — and it is also sixty italic serif
    captions on a wall of covers. He approved the serif once, for one string
    (Home's `CONTINUE` placard); this is a different magnitude. The 13 px
    legibility of an italic serif in a lane row is answerable only from a
    rendered frame.
  - **Should a playlist of one to three distinct records draw the rest tile
    instead of that record's cover full-bleed?** *Needs: yes or no.* It is the
    only change that makes the sleeve honest at every count and the direct cure
    for the loop doc 14 §0 names — but it costs a two-record list the best
    sleeve available to it, at 320 px on its own page. A genuine aesthetic
    trade with no right answer from the code, and aesthetics is his rule.
    (ADR-0024 §A1 rule 2; `views/mod.rs`'s deciding match.)
  - **On `Save as playlist` — did he mean *remove it* rather than make it make
    sense?** *Needs: his intent.* Tier 1 kept it and labelled it honestly,
    because the repo's one rule is that what he asks for goes in the app and
    the act is real for a shuffle, a `Play all` or an edited run. But he is the
    one who noticed it, and if the intent was *"this should not be here"* that
    is a sentence only he can write.
- **Resizing is still slow**, reported twice. Two commands separate the toolkit
  from us and have never been run on the machine that has the bug — Xvfb has no
  GPU and cannot reproduce it:
  ```sh
  ICED_BACKEND=tiny-skia baz        # smooth ⇒ wgpu surface reconfiguration
  ICED_PRESENT_MODE=immediate baz   # smooth ⇒ vsync/swapchain
  BAZ_MSG_LOG=1 baz                 # names what actually fires while dragging
  ```
  *Needs: one run on the owner's machine.*

## Recently done

Newest first. Fuller detail in `CHANGELOG.md`.

- **A multi-CD album is one record** — the owner's *"it would be good if multi
  CD albums were a single item"*. ADR-0038; fixture, before/after frames and
  the shape table at `docs/design/impl/multi-disc/`.
  - **Three of the four shapes already were one item**, and that was
    established with a fixture of real tagged files before anything was
    changed. The grouping key is (album artist, album title) and reads no path,
    so discs sharing an `ALBUM` tag were always one record whether they sat in
    one folder or two — and `disc` has always been the third field of the
    library's sort key, so a merged set has never played its two track-ones
    interleaved.
  - **The shatter was the disc in the *title*** — `… (Disc 2)`, `… CD2`,
    `… [Disc 2]`, which is how a great many rips arrive. `split_disc_marker`
    takes it off: three words, one or two digits, at the end, on a bracket or
    whitespace boundary. A closed list, never a distance.
  - **It fires only when a sibling exists.** A lone `Bitches Brew CD1` keeps
    its name; the rule can never rename a record it did not merge. That is the
    ADR-0008 posture held as far as it can be held, and the ADR is explicit
    about what it costs where it is let go.
  - **The marker also supplies the missing disc number**, which is the
    correctness half: a `CD1`/`CD2` rip that never wrote `DISCNUMBER` now plays
    in disc order and its page draws the breaks. Tags still win where both
    exist.
  - **Left unmerged deliberately**: two folders with no disc signal at all
    (shape 4) still interleave, because folder names are evidence about nothing
    and inventing an order from them is the guess this project does not make.
- **`A–Z` is a group key again, first in the row** — the owner's *"that feels
  like it should go back and honestly it's the first option, followed by
  artist"*. The strip is `A–Z · ARTIST · YEAR · GENRE · ADDED · PLAYED` and the
  number row is `1`–`6`. ADR-0035's third amendment; frames at
  `docs/design/impl/az-and-artist/`.
  - **The new key does not take `"artist"`'s code back.** It is `"alphabet"`,
    because `"artist"` was already repurposed once without saying so — it named
    the initial grouping before ADR-0035 and the artist grouping after, so a
    `config.toml` written before that day quietly changed meaning. That is now
    a paragraph on `GroupKey::code` itself, where the never-repurpose rule
    lives, rather than folklore.
  - **The budget was re-measured, not reused.** The last sixth word was
    `ARTISTS` at 77.49 px; `A–Z` costs 44.92, so the row is 357.91 and
    `KEYS_W` is **360** rather than the earlier costing's 368. Downstream:
    `LIBRARY_LINE` 552, `TOP_BAR_SPLIT` 824, `SINGLE_LINE_NO_WELL` 600.
    **Nothing forced the window's minimum**, which was the thing to confirm —
    the library line sits 48 px under the 600 floor, and the
    single-line-with-well band survives at 824…904.
  - **Found on the way**: `views::top_bar::group_key`'s doc still carried a
    paragraph about *"none of the five is current while the artists are on the
    wall"*, describing a wall deleted the same day. Corrected.
- **Search off the Library, decided and built** — ADR-0036, the owner's *"how
  the search works when we're not on the library needs to be decided… maybe a
  little x or esc to clear would make sense too"*. The first half was **already
  half-answered**: every road to the query has gone to the Library first since
  the well moved into the lane (`App::reach_the_well`). What was missing is that
  the field never said so, so the placeholder now names its subject —
  **`Search library`**, in every place, in the field's resting 176 px, which
  costs nothing because a placeholder and the count's slot are never on screen
  together. **Contextual search is refused** on one hard constraint rather than
  on taste: type-anywhere is a promise about the collection, and a scoped well
  would revoke it on exactly the pages a scope applies to. And the **`×`** ships
  in the mark's own box — the magnifier at rest, the cross while a query stands
  — because the field's right edge is full and the swap costs the query none of
  its 104 px. It runs `Esc`'s own function. Frames at
  `docs/design/impl/search-scope/`.
  - **The one thing this declines and does not dismiss**: a filter for a long
    playlist's rows. Costed in `BACKLOG.md` as a *second control on the
    playlist page* — its state beside `renaming`, peeled by
    `peel_place_states`, and needing a key of its own because `/` and `Ctrl`+`F`
    belong to the well. One surface earns it; that is the owner's call to make.

- Doc 14 Tier 2 — **the distinction moves into the type**. A record's page sets
  its title in the serif italic; a playlist's page deliberately keeps the sans,
  and that asymmetry *is* the design. The two identity blocks did not move a
  pixel: three ink bands, 71 px of ink, a 35 px pitch to the byline and 27 px
  to the facts, identical on both pages at 1280 and 1920
  (`docs/design/impl/serif-titles/measure.py`). The byline also gained its
  composition, `Playlist · 12 records`. Frames at
  `docs/design/impl/serif-titles/`.
  - **`now_playing.rs`'s prose argued against the serif, and it was half
    right.** Its concern — *a display face arriving one surface at a time* — is
    kept verbatim and is exactly why the test stays an **enumeration** rather
    than becoming a `contains`. Its **boundary** was wrong: *"there is one
    placard in the product"* is a quantity, and a quantity cannot say whether
    the next string may have the face. The rule that replaced it is *the serif
    sets an album's title, on the surface whose subject that album is* — under
    which Now playing stays sans **more firmly** than before, since its hero is
    a **track's** title and the album under it is a fact about that track.
  - **Found on the way, twice.** (a) Doc 14 costed the byline's count as free
    from the sleeve's quotation list; that list stops at four, so `Road Trip` —
    fourteen tracks, twelve records — would have read `Playlist · 4 records`
    over a page listing twelve. The distinct set is walked to its end now. (b)
    A frame cannot prove the *bundled* serif rendered rather than a host serif
    iced silently fell back to, so two `font.rs` tests do: the family strings
    against what the bytes spell, and every Latin-1 letter an album title can
    arrive with. Writing the first turned up that the family a matcher reads is
    `name` record **16** — record 1 is the legacy family, and Plex Sans
    Medium's reads `IBM Plex Sans Medm`.
  - **Tier 2 #8 was declined from its own frame**, not skipped: the strip reads
    `Run · 2 of 12 · 55:00 left … Save as playlist`, subject first, and a
    variable-length `Save these N as a playlist` in the 440 px strip is the one
    measurement doc 14 §6.3 flagged as wanting a frame before it ships.
- **The artwork stops at the file, and the room takes the record's colour**
  (doc 12 A2 **and A3** — A2 alone did not answer the complaint: at 1920 the
  record is height-bound, so deleting the 720 px clamp bought 53 px and left
  the same empty room. The clamp made the square small; the *absent field*
  made the room empty). A 1024 px hero decode, the sleeve now source-bound
  (1024 at 2560; **300** for a 300 px cover, where it used to be a 2.25×
  upscale of a 320 px thumb), and a three-hue field on the room's own
  lightness ladder.
- **Shuffle is a property of the walk.** The run keeps its order; the engine
  gained one standing `traversal` and nothing else. `shuffle.rs` deleted whole,
  along with two invalidation rules, the restore walk and the snapshot case.
  Gapless survives by handing a `Session` an itinerary plus a slot→position
  plan, so the decode-ahead producer is unchanged to the line — every existing
  gapless test passes untouched. The rule is a **bag**: one shuffled pass, no
  repeat until everything has played, and the next row is *shown* (an open ring
  beside the sounding row's dot) rather than hidden.
- **`All songs` has a tile on Home**, second under `CONTINUE`. The strip's
  `Play all` stays: it plays what the *wall* shows, which is the only way to
  play seven search results; the tile plays the collection whole.
- The wall's scrollbar moved to the **window's** right edge. It was the wall's
  bar, not the lane's — this file's item said "the lane's", and a rendered
  frame said otherwise: the lane draws no bar at all with a short list, and the
  wall's sat at x 1168–1171 in a 1280 px window with the rail's 108 px lane
  outboard of it. Now x 1276–1279, with the rail, its letters and the density
  detents at exactly the x they had. It costs the rail 4 px of the press band
  that ran to the screen edge; taken on purpose, and argued at
  `docs/design/impl/wall-scrollbar/`.
- `ARTIST` groups albums under their artist. It turned out to be an ordinary
  group key rather than a subject beside one — `shelves(Artist)` is `albums()`
  with its breaks named — and that identity retired `A–Z` too, since both are
  `albums()` differing only in where the headers fall. **−700 lines**, no
  migration.
- Doc 14 Tier 1 — a record is a work you found, a playlist is a label you made.
  The line under a name declares its kind first; the playlist page gets back the
  byline the record page always had (52 → 80 px, the record's own block); the
  run strip names its subject; and `Save as playlist` becomes the readout
  `Saved as "…"` while the run *is* that file. Frames at
  `docs/design/impl/records-and-lists/`.
  - **Found on the way**: doc 14 §1.4 costed the save fix at *"no new state"*,
    reading saved-ness off `can_undo`. That is wrong — `App::queue_undo` is
    cleared by leaving the place, by standing the run column down and by the
    run ending, none of which un-edits a run, so an edited run would have
    claimed to be its source file again after one navigation. Divergence is now
    a flag beside the queue record (`PlayerState::queue_edited`, one bool, two
    writers).
  - `views/queue.rs` had `save_control`'s doc comment attached to
    `undo_control` — two blocks run together, so the save word carried none.
    Repaired with the change.
- Settings wears the lane — it had neither lane nor door, so `Esc` was the only
  way out of a place you reach with the pointer.
- The bar's now-playing block leads to `Now playing` rather than to the record.
- The artists wall, and `A–Z` naming what it breaks on.
- The queue merged into `Now playing`; the bar's `Queue` door removed.
- The collection's counts moved from the lane to Home as a `COLLECTION` footer.
- `Pull` removed; shuffle became a player property; `All songs` became an
  implicit list.
- Design doc 14 — records versus lists; found the two complaints were one
  defect, and that a one-to-three-record playlist's sleeve is byte-for-byte the
  widget a record's own row builds.
- The refusals ledger deleted — it had become law over the owner.
