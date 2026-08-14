# A stated memory budget for artwork, and the scheduler repair beside it

WORK.md item 37. Two halves, in one change because they are one subject: how
much decoded artwork baz holds, and why the artwork it is holding does not
reach the screen.

## The repair: a delta handed to a replacing queue

`ThumbJobs::focus` **drains the whole foreground queue and re-adds only its
argument**. That is what "re-aim" means and it is right — a wall that has
scrolled past a record should stop waiting to decode it.

`App::request_target_thumbs` was handing it a **delta**: the targets that were
neither cached nor *already queued*. So a re-aim over an unchanged viewport
passed the empty set, and the replace threw the whole queue away and put
nothing back.

On an untouched cold start that happens twice — iced emits `Scrolled` once the
scrollable measures its real bounds, and `WindowResized` when the first resize
lands — and there is no third event to re-queue anything.

### Measured, before and after

Fresh 25-album library, 1280 × 860, Balanced, isolated Xvfb, **no interaction
at all** (`coldstart.sh`; the pointer is parked outside the window and never
moves again):

| | decodes completed in 15 s | frames at 6, 9, 12, 15 s |
|---|---|---|
| before | **2** | pixel-identical (`AE=0`), every cover a gradient |
| after | **8** — the whole visible wall | pixel-identical, every cover drawn |

`cold-start-after.png` is the 15 s frame.

The fix is to pass the **complete snapshot** rather than the delta — drop the
`thumb_jobs.contains` exclusion and keep the other two. It is safe because
`focus` already skips in-flight ids and `queued.insert` is idempotent, so
drain-then-re-add now *preserves* queued work while still dropping whatever left
the target set. The two exclusions that stay are facts about the id rather than
about the queue: `touch` says it is already decoded, `no_art` says there is
nothing on disk.

`request_thumbs` (a page) and `request_thumbs_for` (chrome) come through the
same function and get the same repair. `ThumbJobs::retry` — the density-grew
retry — prepends without draining and is untouched, which is item 30's contract.

### Warm resize

`resize.sh`: settle at 1280 × 860, then widen to 1900 × 1100.
`warm-resize-mid.png` is taken 0.6 s in — **no gradient flash**; every cover
already decoded is still drawn, and only the two records the wider window newly
reveals are pending. `completed` goes 8 → 10: nothing is re-decoded.

## The budget: a decision, where there had been a measurement

The owner: the tiered art machinery *"was introduced to try to keep RAM usage
down but we never specified a sensible limit."*

He is right, and the gap was specific. The **retained** tier — art that has
actually reached a visible target, kept so that scrolling away and back cannot
turn a sleeve back into a gradient — was a plain `HashMap` with **no bound at
all** except the size of the indexed collection. Every figure this project
published (60.0 / 124.3 / 153.5 MiB at Dense/Balanced/Spacious) was a
measurement of what that came to on the owner's 393 albums. On a 5,000-album
library the same code retains something over two gigabytes, and nothing in the
product would have said so.

### The numbers

- **`THUMB_BUDGET_BYTES` = 160 MiB** — the thumbnail tier's whole decoded RGBA.
  Chosen against the collection the feature exists for: the owner's 393 albums
  at Spacious's 320 px ceiling is 153.5 MiB, so *his whole collection stays
  retained at every density*. It is the smallest 32 MiB step that clears it,
  which keeps the headroom from being a second undeclared decision.
- **`SPECULATIVE_BUDGET_BYTES` = 25 MiB** — the share art nobody has seen may
  hold. Not a new number: it is exactly what the tier's long-standing 64
  entries cost at the largest edge, which this project already published as its
  worst case. **`THUMB_CACHE_ENTRIES` is derived from it** and comes back to
  64, so stating the decision in bytes changed no behaviour and moved it to the
  side of the equation that can be argued with.
- **170 MiB is all of baz's decoded artwork**, with the hero tier's 8 and the
  artist tier's 2. That is the figure worth quoting because it is the one a
  process monitor shows — and Settings → Debug now reports the resident set
  beside it, so the claim is checkable inside the running app.

### How it is held

`retained` is an **LRU** now rather than a `HashMap`, so "least recently
visited" is a real ordering; `touch` promotes, and `touch` is called on every
target of every re-aim, so it means "least recently *on screen*".

`ThumbCache::trim_to_budget` drops **speculative art first, then the least
recently visited retained art**, until the total fits. The running total is
carried rather than recomputed, so trimming a large overflow is linear.

### The one hole, stated rather than assumed

The **resident tier is exempt**. A visible sleeve turning back into a gradient
is the defect the whole tier exists to prevent (item 20), so the trim stops
rather than reaching for it — which means a window whose visible wall alone
exceeded the budget would exceed it.

It cannot, and the margin is nearer than it looks. At 3840 × 2160 with no room
taken by the bars or captions, each density against **its own** decode ceiling:

| density | tiles | edge | resident |
|---|---|---|---|
| Spacious | 112 | 320 px | 43 MiB |
| Balanced | 144 | 288 px | 45 MiB |
| Compact | 220 | 240 px | 48 MiB |
| **Dense** | **336** | **200 px** | **51 MiB** |

A third of the budget, not a hundredth. `the_visible_wall_can_never_exhaust_the_art_budget`
asserts a 2× margin against the worst of them; an earlier draft of this claimed
two orders of magnitude and was simply wrong, which is why the figure is a
table now.

Note the pairing: Dense hangs the most tiles *and* decodes the smallest, so the
two do not compound. Costing every density at `THUMB_PX` would be a worst case
the product cannot reach.

## Isolation

Both harnesses take all six XDG redirections; `[mpris] no session bus` is in
each log as the receipt. The fixture is `mkfixture.sh`'s silent FLACs.
