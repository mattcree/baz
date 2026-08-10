# `ARTIST` groups by artist

Frames from the real binary, rendered headless on a private Xvfb with all six
XDG redirections from [`docs/DEVELOPMENT.md`](../../../DEVELOPMENT.md). Nothing
touched the owner's session; `capture.sh` prints the run's own
`[mpris] no session bus` line as the receipt, quoted at the foot of this page.

Reproduce with [`capture.sh`](capture.sh); the header comment carries the
toolbox build line. The decision is
[ADR-0035](../../../adr/0035-the-wall-has-a-subject.md), which these frames
replace [`artists-wall/`](../artists-wall/) as the record of.

## The finding, in the owner's words

> *"artists should be grouping stuff by artist not just alphabetically"*

The wall's first key was called `ARTIST` and grouped records by their album
artist's **initial** — one shelf per letter, everyone whose name starts with an
S sharing it. That is what made its word collide with the Artist *place*, and
the release before this one answered the collision by renaming the key `A–Z`
and adding a sixth word, `ARTISTS`, for a wall of artist tiles. This answers it
by fixing the key instead.

## What changed

| | frame |
|---|---|
| **One shelf per artist, headed by their name**, in the library's own order — unknowns first, names case-folded, unnamed compilations last — with each artist's records alphabetical under them. | [`00`](00-artist-wall-1280.png) · [`02`](02-shelf-headers-1280.png) · [`20`](20-artist-wall-1920.png) |
| **The strip is five words again**, and the first is `ARTIST`. `A–Z` is gone, `ARTISTS` is gone; there is nothing left in the row that is not a group key. | [`01`](01-arrangement-row-1280.png) |
| **The header is a door to `Place::Artist`** — the same place the record page's `Artist ›` breadcrumb opens, by the same `vm::artist_id`, in the same `theme::word_button` paint. The ground is the **word's** box, not the shelf's width. | [`03`](03-the-header-under-the-pointer-1280.png) · [`04`](04-the-door-at-rest-and-hovered-1280.png) · [`05`](05-the-artist-place-1280.png) |
| **The rail is still the alphabet**, over a wall with far more headers than letters: a letter lands on the **first artist filed under it**. `K` is one press and puts Kesh at the top. | [`06`](06-rail-jumped-to-k-1280.png) |
| **A shelf is the wall's ordinary shelf.** Kesh holds six records and gets two rows of the same grid at the same pitch; nothing about the virtualizer, the sticky header or the density knows an artist from a decade. | [`07`](07-a-shelf-with-two-rows-1280.png) |
| **The readouts count records**, because every tile on the wall is a record again: `16 / 25`, and `25 albums · 206 tracks` as the well's placeholder. The `wall_counts` / `wall_noun` split the subject needed is deleted. | [`08`](08-mid-query-1280.png) · [`09`](09-match-count-1280.png) · [`21`](21-mid-query-1920.png) |
| **The other four keys are untouched**, which is what makes this an ordinary key rather than a mode. | [`10`](10-year-wall-1280.png) · [`22`](22-year-wall-1920.png) |

## The door is quiet on purpose

[`04`](04-the-door-at-rest-and-hovered-1280.png) is the header at rest above
the header under the pointer, both at 4×. The type does not change — same face,
same size, same tracking, same `paper_faint` ink, same line box — because the
band has to be pixel-identical pinned and unpinned, which is what makes pinning
a *position* rather than a *state*. What it gains is `theme::word_button`'s
`ink_wash` ground, which is what the record page's breadcrumb already wears:
two doors to one place should not be two kinds of control.

`magick compare -metric AE` puts the difference at **910 pixels**, all of them
inside the word's own box. It stops at the `G`; a band-wide ground would light
the whole wall on a mouse-over.

## The migration, photographed

`capture.sh` seeds a `config.toml` a baz from **before** this change wrote,
`wall_subject = "artists"` and all, and prints the document baz writes back:

```toml
# how the wall is arranged: "artist", "year", "genre", "added" or "played"
group_key = "artist"
# how closely it hangs: "spacious", "balanced" or "dense" (Ctrl+- / Ctrl+= / Ctrl+scroll)
density = "balanced"
```

Two facts in one file. **`wall_subject` is gone** — every value in `config.rs`
is read by name, so a key nothing reads is not read, and the next save drops
the line rather than carrying a setting nothing honours. And **`group_key`
needed no migration at all**: the code is unchanged and names the same key,
which is the reason nothing was retired. A config written by any baz ever
released resolves, and resolves to the arrangement its word always claimed.

## The strip, un-re-measured

The sixth word cost the arrangement row 54 px. Removing it gives all 54 back,
so `KEYS_W` is 368 → **314** and every figure derived from it returns to what
it was before ADR-0035 — asserted as const arithmetic in `theme.rs`, with
`KEYS_SPENT` kept at **0** rather than deleted, because what a word costs the
strip is the number the next one is argued against.

| | with the sixth word | now |
|---|---:|---:|
| `KEYS_W` | 368 | **314** |
| `LIBRARY_LINE` | 560 | **506** |
| `SINGLE_LINE` = `TOP_BAR_SPLIT` | 832 | **778** |
| `SINGLE_LINE_NO_WELL` | 608 | **554** |
| `WIDEST_LANE_STRIP` | 720 | 720 (unmoved) |
| `TOP_BAR_FLOOR`, and the window's own minimum | 600 / 696 | 600 / 696 (unmoved) |

The single-line-with-well band is therefore **778…904** rather than 832…904,
and [`12`](12-strip-band-874.png) is its new left edge: a 874 px window leaves
the strip exactly 778, the narrowest window at which it is still one line. It
was 928 an hour ago. [`14`](14-strip-two-lines-696.png) is the window's own
minimum, where the strip is two lines and nothing hides or overflows — the
94 px of slack the library line now sits under the floor by.

## The isolation receipt

```
[mpris] no session bus; desktop media controls unavailable (I/O error: No such file or directory (os error 2))
```

Three launches, three lines, no name published to any session bus. Nothing was
audible: the scratch `HOME` routes ALSA's default PCM to `null` and the
fixture's samples are all zero, and `BAZ_DEVICE_TESTS` was never set.
