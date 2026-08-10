# One page, two subjects — the frames

> **The owner, 2026-08-10**: *"can we reuse the basic layout and view of the
> playlist for the album view and the playlist view accessed via clicking into
> info — right now they are different but for no good reason. it would be good
> if it was clear via some sort of title/subtitle telling us if it's an Album
> or a Playlist"*

Shipped 2026-08-10. Records [ADR-0024](../../adr/0024-playlists.md) §A2's
arrangement being **made literal** — `crates/baz/src/views/page.rs`, the
composition a record's page and a made list's page both wear — and the two
divergences the frames caught that no source sweep could.

`capture.sh` shoots **two builds**, because the claim is a comparison: `BIN0`
is the commit the branch started from and `BIN` is the branch, same fixture,
same window, same gestures. `measure.py` reads the numbers back out of the
pixels and prints `agree` or `DIFFER` beside each, so this page can be checked
rather than believed.

```sh
toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
  cargo build --release -p baz --features device-output
toolbox run -c baz-dev docs/design/composition/tools/mkfixture.sh /tmp/baz-lane-fix
toolbox run -c baz-dev env BIN0=… BIN=… \
  docs/design/impl/one-page-two-subjects/capture.sh
python3 docs/design/impl/one-page-two-subjects/measure.py
```

Isolation receipt, both runs (docs/DEVELOPMENT.md §"Headless UI verification",
all six XDG redirections, scratch `HOME` routing ALSA to null, silent
fixtures, `BAZ_DEVICE_TESTS` unset):

```
[mpris] no session bus; desktop media controls unavailable (I/O error: No such file or directory (os error 2))
```

---

## The answer to the second sentence, first

**His *"title/subtitle telling us if it's an Album or a Playlist"* was already
shipped**, by design 14's tiers 1 and 2, and
[`0d-identities-together-after-1280x860.png`](0d-identities-together-after-1280x860.png)
is the frame that says so. The two identity blocks at one crop:

```
Ochre                                   ← 28 px, serif italic: a work's title
Anne-Marie Puig                         ← 19 px: a person
1999 · 12 tracks · 59:18 · FLAC · 16-bit · 44.1 kHz

Road Trip                               ← 28 px, sans: a label somebody typed
Playlist · 12 records                   ← 19 px: the kind, in the same slot
14 tracks · 2:02:56
```

The made thing says the word `Playlist` at 19 px directly under its name; the
found thing names a person there, sets its title in a face no label can wear,
and carries `Play album` and a `DETAILS` table. **Nothing was added.** An
eyebrow above the name was the obvious candidate and it is not drawn: a second
statement of a thing this frame already states plainly would be the badge
[design 14](../14-records-and-lists.md) §5.6 declined, wearing a word.

What the change did to the words is subtract one. The strip above these blocks
used to read `Playlist` on one page and `Anne-Marie Puig › Ochre` on the
other — see [`0b-strips-together-before`](0b-strips-together-before-1280x860.png)
against [`0b-strips-together-after`](0b-strips-together-after-1280x860.png).
`views::place_header_led`'s own rule is that **a place whose subject changes
leads with its subject**, which the Album place and the Artist place both do;
the Playlist place led with its *kind* because it predates the breadcrumb by
weeks. It leads with the list's name now, and the kind is where design 14 §3.5
argued it belongs — not *"58 px above the name, in the chrome… invisible at
the moment the eye is actually deciding"*.

---

## The headline frame

[`0e-pages-together-after-1280x860.png`](0e-pages-together-after-1280x860.png)
— the two pages at one crop, one above the other. Same gutter, same 320 px
aside, same sleeve edge, same accent box at the sleeve's width, same acts row
on the same lane, same identity block at the same offset, same `TRACKS` rule,
same rows, same seam to the main column.
[`0e-pages-together-before`](0e-pages-together-before-1280x860.png) is the
same crop of the same two pages before the change; the differences are the
whole of this study.

---

## What `measure.py` reads, before and after

At **both** window sizes, identically:

| reading | before | after |
|---|---|---|
| identity block — the three bands' tops | `DIFFER` (27/62/89 against 15/50/77) | **agree** |
| identity block — pitch between the bands | agree | agree |
| identity block — first ink to last | agree, 71 px | agree, 71 px |
| the commitment's top (`Play album` / `Play`) | `DIFFER` (9 against 0) | **agree** |
| the commitment's left edge | agree | agree |
| the acts row's top | `DIFFER` (65 against 53) | **agree** |
| the acts row's left edge | `DIFFER` (115 against 12) | **agree** |
| the main column's first ink, x | agree, 344 | agree, 344 |

Two findings in that table, and **neither was visible in the source**:

### 1 · The acts row hung from two different lanes

`115` against `12`. A record's single quiet act, `Add to playlist…`, was a
**centred, full-width box** in `theme::word_button`'s paint, resting at
`paper_dim`; a playlist's `Queue` · `Rename` · `Delete` were **natural-width
words** in `theme::transport`'s, resting at `paper`. One slot, two alignments,
two inks, and no file that could name the difference because neither file
could see the other. [`0c-asides-together-before`](0c-asides-together-before-1280x860.png)
against [`0c-asides-together-after`](0c-asides-together-after-1280x860.png).

They are one word now (`views::page::act`), hanging from the aside's own lane
like everything else in it — which is law L5, and which a centred box could
not do.

### 2 · The two strips were different heights, and so was the whole page

`9` against `0`, and the identity block's `27/62/89` against `15/50/77`: the
same 12 px, three times. **A playlist's whole page rode 12 px higher than a
record's.**

`theme::TOP_BAR_H` is `2 · TOP_BAR_PAD_V + TRANSPORT_HIT + 1` = 49, and
`views::place_header_led` does not hold its lead to it — it lays out whatever
it is handed. A record's breadcrumb is a **control** and declares
`TRANSPORT_HIT` 32 of its own; a playlist's name was a bare `LINE_EMPHASIS`
20, so that strip came to 37. Measured off
[`01-record-before`](01-record-before-1280x860.png) and
[`02-playlist-before`](02-playlist-before-1280x860.png): sleeve top at y = 88
against y = 77.

Tiers 1 and 2 could not have caught this. Both cropped each identity block out
of *its own* page and compared their **shapes** — three lines, one pitch,
80 px — which was true, and is still true. This study crops both at the same
window coordinates, which is what turns a shape into a position.

The composition boxes its lead at the control height now
(`views::page::the_lead_stands_at_the_height_the_frame_declares`), so the two
subject pages agree.

> **The rest of the product still has that 12 px**, and it is not fixed here.
> Queue, Settings and the Artist place all lead with a bare name, so all three
> stand at 37 against the Library's own 49 — *"the frame is the frame in every
> place — navigating may not slide the content area by a pixel"*
> (`views/mod.rs`) is false by 12 px today. Holding every lead at
> `TRANSPORT_HIT` inside `place_header_led` is one line, and it moves the
> content of four places on screen, which is a change to the **frame** rather
> than to these two pages. Logged in [`docs/WORK.md`](../../../WORK.md) with
> this measurement rather than taken in passing.

---

## The frames

Prefix `0` is 1280 × 860 and `1` is 1920 × 1080. Every name carries `-before`
or `-after`.

| | what it is |
|---|---|
| `01-record-…` | a record's page, whole |
| `02-playlist-…` | a playlist's page, whole |
| `03-strip-record-…`, `04-strip-playlist-…` | the header strip, at the body's width |
| `05-aside-record-…`, `06-aside-playlist-…` | the aside below the sleeve: the commitment and the acts |
| `07-identity-found-…`, `08-identity-made-…` | the identity block, at tier 1's own crop — these overlay [`../records-and-lists/`](../records-and-lists/README.md) and [`../serif-titles/`](../serif-titles/README.md) |
| `09-page-record-…`, `0a-page-playlist-…` | the whole two-column composition |
| `0b-strips-together-…` | the two strips stacked |
| `0c-asides-together-…` | the two asides stacked |
| `0d-identities-together-…` | the two identity blocks stacked |
| `0e-pages-together-…` | **the two pages stacked** — the claim in one look |

The `1920 × 1080` set is the `1280 × 860` set with more air and nothing else,
which is the property the clamp at `LIST_MEASURE` 880 exists to give: the page
centres, the measure stops growing, and every number in the table above is the
same at both sizes.
