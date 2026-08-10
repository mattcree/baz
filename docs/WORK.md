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
2. **Doc 14's Tier 2 — the serif on the two pages.** Held back tonight only
   because `views/now_playing.rs` argues in prose *against* the serif and must
   be amended in the same commit, and that file was another agent's. Do it
   next, and amend the prose with it or the code argues with the ADR.
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
- **Doc 14 Tier 1** — the kind in the line under a name (`Playlist · 14 ·
  42:10`), the byline that makes the two identity blocks the same height, and
  the `Save as playlist` fixes: the run strip gains its noun, and the word
  becomes the readout `Saved as "Road Trip"` while provenance stands unedited.

## Waiting on the owner

- **Borderless window chrome.** Wayland already draws that title bar inside
  baz's own process, so turning it off is one field — but **iced 0.13 exposes
  no edge-drag resize anywhere in `window::Action`**, so going borderless today
  loses pointer resizing. The route is a ~30-line upstream-shaped iced patch,
  which means a forked dependency. *Needs: yes or no to the fork.*
- **Doc 14's Tier 3**, three questions rather than tasks: should the serif
  reach the *wall and lane* as well as the pages (sixty italic captions is an
  aesthetics call, and aesthetics is the owner's own rule)? Should a playlist
  of fewer than four records draw the designed rest tile instead of one
  record's cover full-bleed? And on `Save as playlist` — did he mean **remove
  it** rather than make it make sense?
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
