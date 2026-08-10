# Records and lists — telling a found thing from a made one

Frames of **doc 14 tier 1**
([`docs/design/14-records-and-lists.md`](../../14-records-and-lists.md) §9),
recorded as [ADR-0024](../../../adr/0024-playlists.md) §A3–§A5 and
[ADR-0030](../../../adr/0030-the-returns-lane-and-the-home-band.md)'s fifth
amendment.

The owner, 2026-08-10, two sentences in one breath:

> *"we do not have the playlist name really prominent. basically the
> information heirarchy isn't great to be able to tell the difference between
> an album and a playlist"*
>
> *"'save as playlist' really makes no sense on the playlist page for a CD"*

**They were one defect.** `Save as playlist`, pressed over a CD, wrote a
playlist whose only member was that record; a one-record playlist's sleeve is
byte-for-byte the widget a record's own row builds; and the result landed in
`RECENT` above the record it was made from, wearing its face, in the same type
at the same size. The control manufactured the confusion the other sentence
reported.

Rendered by [`capture.sh`](capture.sh) against the release binary, headless on
a private Xvfb, with all six XDG redirections from `docs/DEVELOPMENT.md`
§"Headless UI verification". Nothing touched the owner's session and nothing
was audible — the scratch `HOME` routes ALSA's default PCM to null and every
fixture sample is a zero. The receipt the run printed:

```
[mpris] no session bus; desktop media controls unavailable (I/O error: No such file or directory (os error 2))
```

Fixture: `/tmp/baz-lane-fix`, 25 records / 206 tracks, the lane frames' own —
plus four `.m3u8` lists at staggered mtimes, **with `#EXTINF` durations**, so
the line under a name shows its longer form.

## The frames

`0…` is 1280 × 860, `1…` is 1920 × 1080. Every pair is the same gesture at the
two sizes.

| | What it shows |
|---|---|
| `01` / `11` `-lane-both-kinds` | The Library with the returns lane holding **both kinds interleaved**, sorted by touch — four records played, four lists — which is ADR-0030 §1's mixing, kept |
| `02` / `12` `-lane-rows` | The rows themselves, cropped. One anatomy, one 64 px pitch, one sleeve vocabulary; **the second line is the whole of the difference** — `Anne-Marie Puig` against `Playlist · 14 · 2:02:56` |
| `03` / `13` `-playlist-page` | A made thing's page, with the byline line restored |
| `04` / `14` `-identity-made` | Its identity block, cropped: `Road Trip` 28 / `Playlist` 19 / `14 tracks · 2:02:56` 12 |
| `05` / `15` `-record-page` | A found thing's page, unchanged |
| `06` / `16` `-identity-found` | Its identity block at the same crop: `Ochre` 28 / `Anne-Marie Puig` 19 / `1999 · 12 tracks · 59:18 · FLAC · 16-bit · 44.1 kHz` 12 |
| `0d` / `1d` `-identities-together` | **The two blocks at the same crop, stacked** — the comparison as a look rather than an argument. Same three lines, three sizes, three inks, one 80 px block; the middle line is the only thing saying two different sorts of thing |
| `07` / `17` `-run-of-a-record` | `Now playing` with a **record's** run — the case the owner was looking at |
| `08` / `18` `-strip-unfiled` | Its summary strip, cropped: `Run · 2 of 12 · 55:03 left … Save as playlist` |
| `09` / `19` `-run-from-a-file` | The same place with a run reified from `Road Trip`, untouched |
| `0a` / `1a` `-strip-saved` | Its strip: `Road Trip · 1 of 14 · 1:59:17 left … Saved as “Road Trip”` — a readout, not an offer |
| `0b` / `1b` `-run-edited` | The same run after one ✕ |
| `0c` / `1c` `-strip-diverged` | Its strip: `Road Trip · 1 of 13 · 1:53:28 left  Undo  Save as new playlist` — live again, and a **new** file |

## What the pixels say

[`measure.py`](measure.py) reads both load-bearing claims off the committed
PNGs. Nothing below is computed from tokens.

```
--- the identity blocks, measured (ADR-0024 §A4.3) ---
  1280x860 made  block ink spans 71 px
  1280x860 found block ink spans 71 px
  1280x860 difference: 0 px  ok
  1920x1080 made  block ink spans 71 px
  1920x1080 found block ink spans 71 px
  1920x1080 difference: 0 px  ok

--- the run strip at RUN_MEASURE 440 (design 14 §6.3) ---
  1280x860 unfiled  reading ends x=130, word begins x=337, air between 206 px, right edge 421/440  ok
  1280x860 saved    reading ends x=172, word begins x=308, air between 135 px, right edge 420/440  ok
  1280x860 diverged reading ends x=217, word begins x=311, air between  93 px, right edge 421/440  ok
  1920x1080 unfiled  reading ends x=130, word begins x=337, air between 206 px, right edge 421/440  ok
  1920x1080 saved    reading ends x=172, word begins x=308, air between 135 px, right edge 420/440  ok
  1920x1080 diverged reading ends x=217, word begins x=311, air between  93 px, right edge 421/440  ok
```

**The identity blocks agree to the pixel**, at both sizes. The block was 52 px
against a record's 80; it is now the record's, and the two pages differ in
what the middle line *says* rather than in how many lines they have. (The ink
measures 71 rather than 80 because a hero's cap height starts below its line
box and the meta line's baseline sits above its own; what matters is that the
two are the same number, and they are.)

**And the strip fits at 440.** Design 14 §6.3 called this *"the one
measurement in this study that wants a frame before it ships"*: `Run · ` costs
about 34 px, `Save as new playlist` about 48 px more than the word it
replaces, and the worst case has provenance, a countdown, `Undo` **and** the
longer word in one 440 px line. That worst case is `…-strip-diverged`, and it
leaves **93 px of air** between the reading and the word, ending 19 px inside
the column. Nothing wraps, nothing clips, and the figures are identical at
both window sizes because the run column is a fixed `RUN_MEASURE` at each.

## What is not here

- **Tier 2 — the serif on the record page's hero.** Held because
  `views/now_playing.rs` argues in prose against it and must be amended in the
  same change; queued in `docs/WORK.md`.
- **Tier 3 — three questions for the owner**: the serif on the wall and the
  lane, the rest tile below four distinct records, and whether he meant
  *remove* `Save as playlist` rather than fix it. Also in `docs/WORK.md`,
  under waiting.
