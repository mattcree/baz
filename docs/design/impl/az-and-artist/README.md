# `A–Z` and `ARTIST`, side by side

Frames from the real binary, rendered headless on a private Xvfb with all six
XDG redirections from [`docs/DEVELOPMENT.md`](../../../DEVELOPMENT.md). Nothing
touched the owner's session; `capture.sh` prints the run's own
`[mpris] no session bus` line as the receipt, quoted at the foot of this page.

Reproduce with [`capture.sh`](capture.sh); the header comment carries the
toolbox build line. The decision is
[ADR-0035](../../../adr/0035-the-wall-has-a-subject.md)'s **third amendment**;
[`artists-grouped/`](../artists-grouped/) is the record of the decision it
amends, and is still correct about everything except §3.

## The ask, in the owner's words

> *"also, we have removed the a-z option from grouping? that feels like it
> should go back and honestly it's the first option, followed by artist"*

ADR-0035 §3 deleted `A–Z` on the ground that `A–Z` and `ARTIST` are the same
traversal — both are `albums()` with the breaks named, and `A–Z`'s breaks are
strictly coarser. That is true, and it is now the reason both exist rather than
the reason one does not: **the coarseness is the point.** These frames are that
claim, photographed. The pair to read first is
[`09`](09-az-rail-jumped-to-s-1280.png) against
[`10`](10-artist-rail-jumped-to-s-1280.png) — the *same six records*, in the
*same order*, as one flowing `S` grid and as three named shelves.

## What changed

| | frame |
|---|---|
| **The strip is six words, `A–Z` first**: `A–Z · ARTIST · YEAR · GENRE · ADDED · PLAYED`, and <kbd>1</kbd>…<kbd>6</kbd> select them in that order. | [`01`](01-arrangement-row-1280.png) · [`21`](21-arrangement-row-1920.png) |
| **The wall under `A–Z`** — one shelf per letter, the two anonymous buckets at the ends. | [`02`](02-az-wall-1280.png) · [`03`](03-az-headers-1280.png) · [`20`](20-az-wall-1920.png) |
| **The wall under `ARTIST`** — the same records in the same order, one shelf per person, each header still the door to their page. | [`05`](05-artist-wall-1280.png) · [`06`](06-artist-headers-1280.png) · [`22`](22-artist-wall-1920.png) |
| **The two densities, on the letter that has three artists under it.** `S` holds Sonja Aalto, Sotto and Studio Hain: six records in one grid under `A–Z`, three shelves under `ARTIST`. | [`09`](09-az-rail-jumped-to-s-1280.png) · [`10`](10-artist-rail-jumped-to-s-1280.png) |
| **The rail is the same rail under both**, 27 slots either way, because it is a pure function of the headers and both keys' headers file under `Initial`. Only the shelf each letter *jumps to* differs. | [`04`](04-az-rail-1280.png) · [`07`](07-artist-rail-1280.png) · [`08`](08-the-two-rails-1280.png) |
| **The strip's split moved to 824** and the single-line-with-well band is 824…904 — 920 is a window inside it. | [`11`](11-strip-single-line-with-well-920.png) · [`12`](12-strip-band-920.png) |
| **The window's own minimum did not move.** At 696 — `TOP_BAR_FLOOR` 600 plus the lane's rail — the strip is two lines and holds all six words plus `Play all` with 48 px to spare. | [`13`](13-strip-at-the-window-floor-696.png) · [`14`](14-strip-two-lines-696.png) |

## The rail gained no branch, and that is why this was cheap

[`08`](08-the-two-rails-1280.png) is the two rails appended: the `A–Z` wall's
on the left, the `ARTIST` wall's on the right, same library. They are the same
27 letters in the same ink, and the elision mark falls in the same slot.

One function produces both. It takes **the first shelf of each initial's run**
— which is a letter per artist-run under `ARTIST`, and the identity under
`A–Z`, where each initial already has exactly one shelf. So restoring the key
cost `rail::entries` one match arm and `GroupHeaderVm` one variant, and nothing
in the elision, the capacity arithmetic or the lens changed at all.

`Initial` itself is unchanged for the second time: ADR-0035 moved it from the
wall's header to the rail's letter, and it is now both, with one mapping asked
of `baz-core` in both places. That is what keeps `Various` and `Unknown` at the
two ends instead of filed under `V` and `U`.

## The code is `"alphabet"`, and the frames are its migration

The config `capture.sh` plants is the document a baz from **before** ADR-0035
wrote, `wall_subject` and all:

```toml
group_key = "artist"
wall_subject = "artists"
```

The launch opens on the **artist** wall, which is what those frames show: the
restored letter key did *not* take `"artist"`'s code back. It spells itself
`"alphabet"`, a word no baz has ever written.

That is not fastidiousness. `GroupKey::code` may never be repurposed, and it
already was, once, silently — `"artist"` named *group by the album artist's
initial* up to ADR-0035 and *group by the album artist* after it, so a
`config.toml` written before that day names a different arrangement than it did
when it was written. Handing the word back now would make one code name three
walls in three releases. The document baz writes at the foot of the run is the
other half: the key the last press left active, written as its **own** code.

## The strip's budget, measured rather than reused

The row has carried a sixth word before, and it was `ARTISTS`. This one is
`A–Z`, which is shorter, so every figure was re-derived:

| | five words | six with `ARTISTS` | **six with `A–Z`** |
|---|---:|---:|---:|
| the sixth word, in its box with its gap | — | 77.49 | **44.92** |
| the row, measured (`font.rs`) | 312.99 | 366.50 | **357.91** |
| `KEYS_W` | 314 | 368 | **360** |
| `LIBRARY_LINE` | 506 | 560 | **552** |
| `SINGLE_LINE` = `TOP_BAR_SPLIT` | 778 | 832 | **824** |
| `SINGLE_LINE_NO_WELL` | 554 | 608 | **600** |
| `WIDEST_LANE_STRIP` | 720 | 720 | 720 (unmoved) |
| `TOP_BAR_FLOOR`, and the window's own minimum | 600 / 696 | 600 / 696 | 600 / 696 (**unmoved**) |

`ACTS_FREED` — what `Pull`'s removal and shuffle's move to the now-playing bar
gave back — is 94, and `KEYS_SPENT` is 46 of it. The 48 px left is the slack
under the floor, and [`14`](14-strip-two-lines-696.png) is that slack
photographed: the six words and `Play all` on the library line at the smallest
window baz offers, nothing hidden, nothing overflowing. Every figure is const
arithmetic in `the_strip_holds_its_tenants_at_the_single_line_floor`, and the
two halves are asserted as *differences of the movements that produced them*,
so neither can be right by coincidence.

## The isolation receipt

```
[mpris] no session bus; desktop media controls unavailable (I/O error: No such file or directory (os error 2))
```

Three times, once per launch. `HOME`, `XDG_DATA_HOME`, `XDG_CONFIG_HOME`,
`XDG_CACHE_HOME` and `XDG_RUNTIME_DIR` all point inside `/tmp/baz-az-scratch`,
`DBUS_SESSION_BUS_ADDRESS` and `WAYLAND_DISPLAY` are unset, and the library the
frames show is `mkfixture.sh`'s 25 silent albums at `/tmp/baz-az-fix` — never
`~/Music`. Nothing was audible: the scratch `.asoundrc` routes to a null sink
and every sample in the fixture is a zero.
