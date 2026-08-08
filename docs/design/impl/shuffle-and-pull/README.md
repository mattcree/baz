# Shuffle from what the wall shows, and the pull

ADR-0017 steps **17** and **19**, rendered from the real binary on a private
Xvfb with all six XDG redirections from `docs/DEVELOPMENT.md`. The run's receipt
that it never touched the owner's session:

```
[mpris] no session bus; desktop media controls unavailable (I/O error: No such file or directory (os error 2))
```

Rebuild the set with:

```sh
docs/design/composition/tools/mkfixture.sh /tmp/baz-pool-fixture   # 25 silent albums
cargo build --release -p baz --features device-output
docs/design/impl/shuffle-and-pull/capture.sh
```

The fixture is 25 records of digital silence with generated covers — enough that
a pool is visibly a subset of the wall, which is the whole point of the marks.
Every sample is a zero *and* the scratch `HOME` routes ALSA's default PCM to
`null`, so the run is silent twice over.

## The frames

| | |
|---|---|
| `01-wall-filtered-before-shuffle` | the query `so` narrows 25 records to 4 — `4 of 25 albums` in the top bar. This is the wall the pool will be. |
| `02-shuffle-pool-filtered` | `Shuffle` pressed. Nothing dims, correctly: the pool **is** what the wall shows. Two of the four carry a ring — the next two draws. The playing record has the lamp halo and its dot. |
| `03-pool-visible-on-the-whole-wall` | the query cleared, the wall back to 25. **The money shot.** Row 1 is the pool at full strength (`Meadowgrass` playing, `Verdigris` and `Ultraviolet Notes` ringed); row 2 is Studio Hain's two records, outside the pool, dimmed. |
| `04-shuffle-queue` | what the shuffle queued: 32 tracks, **listed as records** — `Amber Room · Sotto`, then `Meadowgrass · Sonja Aalto`, each whole and in its own order. Editable like any queue. |
| `05-esc-peels-the-pools-marks` | Escape, and the marks come off. The music does not stop: the bar is still playing the run. |
| `06-the-pull` | `Ctrl+R`. `Wheatfield · The Ardent`, `The pull — Never played`, and **nothing plays**: the bar is still on the shuffle's record. The control that accepts it is the panel's own `Play album`. |
| `07-the-pull-again` | `Ctrl+R` again: a different record, never the one it was already showing. |
| `08-esc-returns-the-pull` | Escape puts it back — the offer and the column it was made in are one layer. |

## Measurements taken off these frames

| what | measured | expected |
|---|---|---|
| the ring's ink | `#888680` | `Palette::paper_faint` in Closing Time — `(136, 134, 128)` |
| the ring's width | 2 px, both edges of a ringed tile | `theme::POOL_RING` |
| tiles ringed | exactly 2 (`x` 323–324/564–565 and 889–890/1130–1131 at `y` 210) | `shuffle::RINGED` |
| a non-ringed tile's lane | `#0C0D0E`, the wall exactly | invisible at rest |
| a dimmed sleeve | `#6A5C2D` → `#41381B` | `theme::POOL_DIM` in linear light; see that token for why the sRGB ratio is 0.61 |

## What is not here

The **Marquee** lens (ADR-0017 step 18), which is where the pull is designed to
live. It is not built, so frames 06–08 show the pull in the album inspector —
the surface that exists — with the note as one line above `Play album`. The seam
is `app::Pull` and `views::side_panel::pull_note`; both name what Marquee
replaces and what must not be carried across.
