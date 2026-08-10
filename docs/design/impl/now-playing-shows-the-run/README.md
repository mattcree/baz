# Now playing shows the run — the owner's batch of 2026-08-10

Seven asks on one surface in one afternoon, and the frames that check them.
Every shot is a real binary on a private Xvfb with the six XDG redirections
from `docs/DEVELOPMENT.md` §"Headless UI verification" — nothing touched the
owner's session and nothing was audible. The receipt:

```
[mpris] no session bus; desktop media controls unavailable (I/O error: No such file or directory (os error 2))
```

Reproduce with `capture.sh` beside this file (build the binary first; the
header comment has the command). The fixture is
`docs/design/composition/tools/mkfixture.sh`'s, with covers re-drawn at 1400 px
by `impl/artwork-at-size/capture.sh` — the same records wearing the same
colours as the frames in that folder, so a before/after across the two is a
fair comparison.

## The asks, and which frame answers each

| # | The owner | Answered by |
|---|---|---|
| 1 | *"remove the run button from the now playing"* | every frame — the place's top-right corner is empty |
| 2 | *"it should probably just show whatever the now playing is indicating, just not playing"* | [`19-02`](19-02-paused-1920x1080.png) vs [`19-01`](19-01-sounding-1920x1080.png) |
| 3 | *"the nothing queued thing is hugging the left with no padding"* | [`10`](10-empty-1280x860.png) |
| 4 | *"I still see save as playlist on the queue when playing a CD"* | [`19-01`](19-01-sounding-1920x1080.png) (absent) vs [`19-03`](19-03-assembled-1920x1080.png) (present) |
| 5 | *"the background fade... should continue under the playlist area too"* | [`19-01`](19-01-sounding-1920x1080.png) — no vertical seam beside the column |
| 6 | *"ideally the currently playing item in the playlist is where our scroll goes to"* | [`30`](30-long-run-head-1920x1080.png) → [`31`](31-long-run-followed-1920x1080.png) |
| 7 | *"that needs a scrollbar as well since playlists can be long"* | [`31`](31-long-run-followed-1920x1080.png) — the thumb at the column's right edge |

## The frames

| Frame | What it shows |
|---|---|
| `12-01-sounding` · `19-01-sounding` | **A record's run, sounding**, at 1280 × 860 and 1920 × 1080. Three things at once: no `Run` word in the top-right; **no `Save as playlist`** — this is a `RunSource::Fixed` list and the strip reads only `Run · 2 of 12 · 51:24 left`; and the field crossing under the run column with **no seam**. |
| `12-02-paused` · `19-02-paused` | **The same run, paused.** The owner's *"just not playing"*: the record, the title, the position and the run column are all still drawn, and the only difference from the frame above is the transport's glyph. |
| `12-03-assembled` · `19-03-assembled` | **The same run after one append**, which makes it a list the listener assembled. `Save as playlist` appears, and `Undo` with it. The pair 01/03 *is* the rule — a fixed list offers nothing, an assembled one offers the creation act. |
| `10-empty-1280x860` | **The genuinely-empty place** — no run and nothing sounding, which since this branch is the only state that reaches the empty text. The block stands on the place's own gutter at the rows' own measure; before, it was flush against the body's edge. |
| `20-stale-config-01/02` | **A `config.toml` that still says `run_column = false`** — what a listener who had the density off is holding. The key is read without harm and cannot stand the column down. |
| `30-long-run-head-1920x1080` | **A long run**, several records appended: 36 rows against a 27-row viewport. The scrollbar's thumb is at the run column's own right edge. |
| `31-long-run-followed-1920x1080` | **The same column after the music moved on.** Thirty confirmed track changes later the view has followed: the summary strip has scrolled away, the rows are in the thirties, and the sounding row is on screen. The thumb has travelled with it. |

## What the frames are evidence *against*

Three claims that would have been easy to make and are checkable here instead.

**The empty state's inset is a fix, not a re-centring.** In `10` the block's
left edge is ~56 px inside the body, which is the place's `HANG` plus the
centring of `measure` inside the body — the same left edge the run's rows
take. It is not centred in the body, and it should not be: it stands where the
list it replaces would stand.

**The save word's absence in `01` is conditional, not a removal.** `03` is the
same run one append later with the word back. If a future change made
`save_control` unconditional again, `01` would be the frame that shows it.

**The field's continuity is not bought with legibility.** The seam is gone in
`19-01`, and the run's rows are read over the field's own colours from
`x ≈ 1440` rightwards. That the rows still clear their contrast floors is
**measured, not judged from this picture** —
`field::every_run_row_is_legible_over_the_brightest_field` sweeps every room ×
every hue × every ink against the field's brightest stop. The binding case is
`paper_faint` at **4.71 : 1** against a 4.5 floor; the full table is in
`crate::field`'s module docs.

## Two things the frames record honestly rather than flatteringly

**The follow lands the row wherever the end of the list allows.** In `31` the
sounding row is near the *bottom* of the column, not two rows from the top,
because the run is nearly over and the scrollable has clamped at its end. That
is correct — there is nothing below to scroll — but it means `31` shows the
follow's *effect* rather than its intended resting position. A frame taken
mid-list would show the row two rows down.

**The capture's play gesture is a query and Enter, not a double-click.** A
double-click on a sleeve lands on the wall's hover options and *appends*, which
makes the run an edited one — and an edited fixed run is `Assembled`, so the
save word correctly appears. The first version of this script did exactly that
and produced a frame that looked like the defect it was meant to disprove. The
gesture is named in `capture.sh` for that reason: **what these frames prove
depends on how the run was built**, which is the whole subject of ask 4.
