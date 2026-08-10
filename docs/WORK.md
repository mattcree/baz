# baz — the work queue

> **This file answers one question: what is next?** Read the top of `## Next`
> and start. If you are an agent picking this project up cold, this is the only
> file you need to begin; everything else explains *why*.
>
> **The rule.** Every item here is one of four states, and an item leaves only
> by being **done** or by the owner saying it should go. "Blocked on a decision"
> is a *note on the item*, never a reason to delete it — that failure has
> happened, and this file exists because of it.
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
> Where the other documents fit: `CHANGELOG.md` is what shipped, `BACKLOG.md` is
> what was deliberately *not* done and why, `NEXT-STEPS.md` is the shape of the
> project, `REQUESTS.md` is the owner's asks verbatim with their fate. **This is
> the ordered queue.** If they disagree, this one is wrong — fix it.

## Next

1. **The lane's scrollbar looks inset on the right.** *"scroll bar is in a
   strange location… it seems to have padding on the right"*. The lane's gutter
   was moved onto its contents so the bar could reach the edge; it evidently
   still does not. Measure it on a rendered frame rather than reasoning from
   the code — `docs/design/impl/search-in-lane/` has the harness.
2. **Doc 14's Tier 2 — the serif on the two pages.** Still held back for the
   reason it always was: `views/now_playing.rs:64-70` argues in prose *against*
   the serif and must be amended in the same commit, and that file belonged to
   the artwork-at-size agent while Tier 1 was built. Three things go with it —
   `the_serif_is_the_work_titles_and_nothing_else` (`theme.rs`) changes from
   `assert_eq!(users, ["views/home.rs"])` to an enumerated list of two; the
   album page's hero takes `theme::WORK_TITLE`; and the playlist page's hero
   deliberately does **not** (that is the axis). Tier 2's other two items are
   smaller and can ride with it: the byline stating its composition
   (`Playlist` → `Playlist · 4 records`, from the distinct-record list
   `playlists.rs` already computes for the sleeve), and — only if a frame says
   `Run · ` was not enough — the save label naming its subject.
3. **Cut v0.1.** Nothing is installable. The icon, the release rehearsal and the
   Flatpak build are all done; what is left is a screenshot for the metainfo,
   the version edit from `0.0.0`, a `workflow_dispatch` dry run, then the tag.
   **The tag is the owner's to cut** — the workflow produces a draft.
4. **Rewrite the README as the project's public face**, with the icon and real
   screenshots of the wall, Home, Now playing and a playlist. Deliberately near
   last, so it describes what actually ships. Its keyboard table is badly stale
   today: `Pull` is gone, `Q` never opened the queue, shuffle is a mode, the
   group keys changed, `Ctrl+B` exists.

## Doing

- **The artists wall groups by artist**, not alphabetically — may delete
  `WallSubject` and the `A–Z` key with it.
- **Shuffle stops permuting the run**, and `Play all` becomes a tile on Home.
  The constraint that decides the design: baz is gapless, so the next track
  must be chosen *before* the current one ends.
- **The artwork at full size** (doc 12 step A2) — deletes the 720 px clamp that
  makes full-screen *"look weird"*, adds the 1024 px hero decode and the
  cover-derived field.

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
