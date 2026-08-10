# Frames: the two places this study merges

Eight frames of the **real binary**, headless, at the two sizes
`docs/design/12-now-playing-and-kiosk.md` §5.5a measures — captured before
any of the merge is built, because the argument for merging
`Place::Queue` into `Place::NowPlaying` is an argument about **measured
width**, and it should be made against pixels rather than against a
recollection of them.

Rendered by [`capture.sh`](capture.sh) against `c768035`, on a private
Xvfb, with all six XDG redirections from `docs/DEVELOPMENT.md`
§"Headless UI verification". Nothing touched the owner's session and
nothing was audible — the scratch `HOME` routes ALSA's default PCM to
`null` and the fixture's samples are all zero. **The isolation receipt**,
printed by the script and reproduced here verbatim:

```
[mpris] no session bus; desktop media controls unavailable (I/O error: No such file or directory (os error 2))
[mpris] no session bus; desktop media controls unavailable (I/O error: No such file or directory (os error 2))
```

The run is an ordinary album, started by a double-click on a sleeve —
deliberately the **anonymous** case, because that is the case the owner's
list model is about.

| Frame | What it is for |
|---|---|
| `01a-queue-open-1280x860.png` | The queue place, lane open. **The summary is anonymous** |
| `01b-now-playing-open-1280x860.png` | Now playing, same run, same size — the margin the run column moves into |
| `01c-now-playing-collapsed-1280x860.png` | The same, lane collapsed (`Ctrl+B`) |
| `01d-queue-collapsed-1280x860.png` | The queue place, lane collapsed |
| `02a-queue-open-1920x1080.png` | The queue place at the size the brief describes |
| `02b-now-playing-open-1920x1080.png` | Now playing, lane open |
| `02c-now-playing-collapsed-1920x1080.png` | **The decisive frame** — see below |
| `02d-queue-collapsed-1920x1080.png` | The queue place, lane collapsed |

## The three findings these frames carry

**1. The now-playing surface is already leaving the run's column empty.**
Measured off the frames rather than computed:

| Frame | Body | Work, drawn | Left margin | Right margin | Slack |
|---|---|---|---|---|---|
| `01b` 1280 × 860, lane open 280 | x 280 → 1280 = **1000** | x 512 → 1048 = **536** | 232 | 232 | **464** |
| `02c` 1920 × 1080, lane collapsed 96 | x 96 → 1920 = **1824** | x 648 → 1368 = **720** | 552 | 552 | **1104** |

`537` and `720` are exactly what `now_playing::art_edge` computes for
those bodies (`now_playing.rs:59–73`; the 720 is `NOW_PLAYING_MAX`
clamping, `now_playing.rs:82`), so the frames confirm the arithmetic
rather than merely illustrating it. The run column the merge asks for is
`RUN_MEASURE` **440** plus one `GAP_XL` **24** = **464** — which is the
1280 slack to the pixel, and **less than half** the 1920 slack.

**2. At 1920 × 1080 the merge costs the artwork nothing at all.** In
`02c` the work is *clamp*-bound, not width-bound: it is 720 because
`NOW_PLAYING_MAX` stops it, with 1104 px of body width unused. Taking 464
of that for the run leaves 1360 px of record column against a work that
wants at most 729 even after §5.2 deletes the clamp. **This is the frame
the merge rests on.**

**3. The queue's summary is anonymous, and that is the defect the
owner named.** `01a` reads

```
1 of 24 · 1:56:18 left        Undo                    Save as playlist
```

with no subject. It should read `Ochre · 1 of 24 · 1:56:18 left`, and it
does not, because `vm::album_queue` hard-codes `provenance: None`
(`vm.rs:859`) and `queue_summary` only prepends a name when provenance is
`Some` (`player.rs:2189–2192`). A playlist run *does* get its name there.
Same sentence, one subject present and one absent — which is exactly the
gap [ADR-0034](../../../adr/0034-the-run-and-its-list.md) closes.

Note also, in `01b`'s lane, that `RECENT` credits **Ochre the record**.
Under the list model that is not a fallback and not a bug: an album's
implicit list *is* the record, so the lane is already list-shaped and the
model explains it rather than changing it.
