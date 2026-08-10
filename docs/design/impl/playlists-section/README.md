# The lane's `PLAYLISTS` section

> The owner, 2026-08-10: *"I guess we need to add playlists into their own
> section under library"*.
>
> ADR-0030's **sixth amendment**. Frames from the real binary on a private
> Xvfb, at 1280 × 860 and 1920 × 1080, with the six XDG redirections
> `docs/DEVELOPMENT.md` requires. `capture.sh` is the script that took them and
> it prints its own isolation receipt.

## What changed

The lane's list body was **one** touch-ordered list holding every playlist and
the last 24 records (ADR-0030 §1). It is now **two sections** — `PLAYLISTS`,
every list, then `RECENT`, the records — under the head, above `Collapse`.

**The reversal is the point and it is his.** When the lane was designed he
asked for exactly the mixing that shipped: *"the side bar will have recent
albums and playlists mixed based on some order"*. He has looked at the built
thing and asked for the split. Both sentences are in the ADR; neither was
edited out.

Three decisions were taken inside the ask, and the frames are what each rests
on:

| Decision | Why | Frames |
|---|---|---|
| **`RECENT` loses the lists entirely** | A list in both sections is one door drawn twice — L8.6. And nothing is lost by it: `PLAYLISTS` is *every* list, so the section that would have carried it is the section it is in | `05`, `15` |
| **Both sections stay last-touched-first** | The split is of *membership*, not of order. Alphabetical was the other honest answer for a section holding all of them, and it was refused because it would spend the recency the mixed list gave him to buy the heading — a list played this morning is still the top row | `05`, `09` |
| **Under the head, not inside it** | *"Under library"* read positionally. The three destinations are *always all three and always in that order* (ADR-0030's first amendment); a section between `Library` and `Now playing` would split the one triple this surface is not allowed to grow | `04`, `14` |

## The frames

`0…` is 1280 × 860, `1…` is 1920 × 1080. The `-crop` frames are the lane's own
column at the same scale, so two states can be compared without the wall in the
way.

| Frame | What it shows |
|---|---|
| `00`, `01` / `10`, `11` | **Both sections absent.** No lists on disk, nothing played: the lane below the hairline is bare. A section with no rows is *absent, not empty* — ADR-0030 §6's rule, applied to `RECENT` as well as to the new section, so a first run gets no words over nothing |
| `02`, `03` / `12`, `13` | **`PLAYLISTS` alone**, four lists, nothing played yet. The lists sit directly under the head and `RECENT` is not drawn |
| `04`, `05` / `14`, `15` | **Both sections.** Four lists, then four records, each last touched first. The lamp is on `Ochre` — a record's run, marked in `RECENT`, exactly as before |
| `06`, `07` / `16`, `17` | **Collapsed.** No heading either side: at 96 px there is no measure for a tracked word, which is the answer `RECENT` has always given, and `PLAYLISTS` takes it rather than inventing a second one. What separates the two runs of sleeves is the sections' own `GAP_MD`, and every row still carries its name as a tooltip |
| `08`, `09` / `18`, `19` | **A list sounding.** `Road Trip` opened from the lane and played from its own page: the lamp is on the **list**, and the records it quotes are unmarked and unmoved in `RECENT` — ADR-0034's rule, unchanged by the split and visibly so now that the two are in different sections |
| `0a` / `1a` | **Thirty lists, at the top.** The section has no cap; the scrollbar's thumb at the lane's right edge is the readout of how much there is |
| `0b`, `0c` / `1b`, `1c` | **…and `RECENT` is still reachable**, wheel-scrolled to the foot. All four records, one scroller, one bar. **This is the frame the change most needed**: a second scroller, or a fixed-height `PLAYLISTS` above a scrolling `RECENT`, would have put the records off the bottom of the window at this list count |
| `0d` / `1d` | Back at the top — **byte-for-byte identical** to `0a` / `1a` (`md5sum`), which is one scroll position moving rather than two surfaces disagreeing |
| `0e` / `1e` | **Thirty lists, collapsed, at the foot.** The rail scrolls the same one column, and the records are reachable there too |

## How the frames were reached

Every state is reached by a route a listener takes. A capture that arrives at a
state by some other road can produce a frame that looks like a pass and is not,
and this directory has a precedent for exactly that (`docs/design/impl` — a
script that double-clicked sleeves and silently appended to the run).

- **Records enter `RECENT`** by <kbd>/</kbd>, a query, <kbd>Enter</kbd> — the
  play-the-top-match of ADR-0017 §1.2. One gesture, no pointer arithmetic over
  the wall, and the ledger is written by the player rather than by the script.
- **The list is played** by clicking its row in `PLAYLISTS` and pressing `Play`
  on the page that opens. That is the only route that gives the run a
  provenance, which is what puts the lamp on the list rather than on a record.
- **The scroll** is the wheel, with the pointer in the lane's own left gutter —
  inside the scroller, outside every row's hit area, so nothing is hovered and
  no frame is taken with a row lit for a reason the reader cannot see.
- **Nothing is audible.** The fixture's samples are all zero *and* the scratch
  `HOME` carries an `.asoundrc` whose default sink discards everything.

## Running it

```sh
toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
  cargo build --release -p baz --features device-output
toolbox run -c baz-dev docs/design/composition/tools/mkfixture.sh /tmp/baz-lists-fix
toolbox run -c baz-dev env FIX=/tmp/baz-lists-fix \
  docs/design/impl/playlists-section/capture.sh
```

The last thing it prints is the `[mpris] no session bus` line. That line is the
receipt that the run never reached the owner's session bus; a run without it
should be assumed to have, and its frames thrown away.
