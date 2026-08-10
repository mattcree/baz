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

1. **Doc 14's Tier 2 — the serif on the two pages.** Unblocked now:
   `views/now_playing.rs` is free, and it argues in prose *against* the serif,
   so amend that prose in the same commit or the code contradicts the ADR.
   Three things go with it: `the_serif_is_the_work_titles_and_nothing_else`
   (`theme.rs`) changes from `assert_eq!(users, ["views/home.rs"])` to an
   enumerated list of two; the album page's hero takes `theme::WORK_TITLE`;
   and the playlist page's hero deliberately does **not** — that is the axis.
   Two smaller items can ride with it: the byline stating its composition
   (`Playlist` → `Playlist · 4 records`, from the distinct-record list
   `playlists.rs` already computes for the sleeve), and — only if a frame says
   `Run · ` was not enough — the save label naming its subject.
2. **Doc 12 step A4 — `RUN_MEASURE` scaled by `kiosk_scale`.** Visible in a
   committed frame: at 2560 with the run standing, ~700 px of field sits
   between the sleeve and the run column, because the record column hangs left
   and the run stays 440 wide. A4 takes it to ~1100 at that size.
3. **Cut v0.1.** Nothing is installable. The icon, the release rehearsal and
   the Flatpak build are all done; what is left is a screenshot for the
   metainfo, the version edit from `0.0.0`, a `workflow_dispatch` dry run,
   then the tag. **The tag is the owner's to cut** — the workflow produces a
   draft.
4. **Rewrite the README as the project's public face**, with the icon and real
   screenshots of the wall, Home, Now playing and a playlist. Deliberately
   last, so it describes what actually ships. Its keyboard table is badly
   stale: `Pull` is gone, `Q` never opened the queue, shuffle is a mode, the
   group keys changed, `Ctrl+B` exists.

## Doing

Nothing. The queue above is the next thing.

## Waiting on the owner

- **Borderless window chrome.** Wayland already draws that title bar inside
  baz's own process, so turning it off is one field — but **iced 0.13 exposes
  no edge-drag resize anywhere in `window::Action`**, so going borderless today
  loses pointer resizing. The route is a ~30-line upstream-shaped iced patch,
  which means a forked dependency. *Needs: yes or no to the fork.*
- **Doc 14's Tier 3**, three questions rather than tasks. Tier 1 shipped
  without touching any of them, and each needs one sentence from him:
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
