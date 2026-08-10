# Serif titles — a record's title is a work, a playlist's name is a label

Frames of **doc 14 tier 2**
([`docs/design/14-records-and-lists.md`](../../14-records-and-lists.md) §5.2,
§9), recorded as [ADR-0024](../../../adr/0024-playlists.md) §A4.4. Tier 1's
frames — the same fixture, the same harness, the words rather than the type —
are one directory over at
[`docs/design/impl/records-and-lists/`](../records-and-lists/README.md).

Tier 1 answered the owner's *"the information heirarchy isn't great to be able
to tell the difference between an album and a playlist"* **in words**: the line
under a name now leads with its kind, and the playlist page got back the byline
line the record page always had. Tier 2 says the same thing **in the type**,
which is the axis with no pixel cost:

> a record's title is a **work's** title, published by someone else → serif
> italic, the museum placard's own convention
>
> a playlist's name is a **label the owner typed** → the sans every other typed
> string in this product is set in — the search query, the rename field, the
> folder path

The two page heroes are the same size, the same ink, the same slot, in the same
composition. **The asymmetry is the design**, and the frames below exist so it
can be looked at rather than argued about.

Rendered by [`capture.sh`](capture.sh) against the release binary, headless on
a private Xvfb, with all six XDG redirections from `docs/DEVELOPMENT.md`
§"Headless UI verification". Nothing touched the owner's session and nothing
was audible — the scratch `HOME` routes ALSA's default PCM to null and every
fixture sample is a zero. The receipt the run printed:

```
[mpris] no session bus; desktop media controls unavailable (I/O error: No such file or directory (os error 2))
```

Fixture: `/tmp/baz-lane-fix`, 25 records / 206 tracks, tier 1's own — plus the
same four `.m3u8` lists at staggered mtimes with `#EXTINF` durations.

## The frames

`0…` is 1280 × 860, `1…` is 1920 × 1080. Every pair is the same gesture at the
two sizes. The `-2x` frames are **point-sampled**, not interpolated: they show
the rendered pixels four times larger and invent nothing, which is the only way
a magnified letterform is evidence.

| | What it shows |
|---|---|
| `01` / `11` `-record-page` | A found thing's page. The hero `Ochre` in `theme::WORK_TITLE` |
| `02` / `12` `-playlist-page` | A made thing's page. The hero `Road Trip` in the sans, and the byline now stating its composition |
| `03` / `13` `-hero-found` | The record's identity block, cropped |
| `04` / `14` `-hero-made` | The playlist's identity block, at the same crop |
| `05` / `15` `-hero-found-2x` | The record's block at 2× — the letterforms |
| `06` / `16` `-hero-made-2x` | The playlist's block at 2× |
| `0a` / `1a` `-heroes-together` | **The two blocks stacked at the same crop** — the whole claim of the tier in one image |
| `0b` / `1b` `-heroes-together-2x` | The same at 2×. This is the frame to look at |
| `07` / `17` `-record-long-title` | The fixture's box-set-length title, which is the serif's real risk rather than `Ochre` |
| `08` / `18` `-hero-long-title` | Its identity block: two lines of 28 px italic, clipped where `max_height(2.0 * LINE_HERO)` puts the cut |
| `09` / `19` `-hero-long-title-2x` | The same at 2× |
| `0c` / `1c` `-run-of-a-record` | `Now playing` over a record's run — **unchanged by this tier**, re-shot to judge tier 2 #8 from a frame |
| `0d` / `1d` `-strip-unfiled` | Its summary strip, cropped |
| `0e` / `1e` `-strip-unfiled-2x` | The same at 2× |

## What the pixels say

[`measure.py`](measure.py) reads the ink out of the two identity crops. Its
output at both window sizes, identical to the pixel:

```
  a record — 03-hero-found-1280x860.png
      hero    top y= 27  ink height=21   first letter leans +3 px
      byline  top y= 62  ink height=18
      facts   top y= 89  ink height= 9
      block  first ink to last = 71 px   pitch hero→byline=35  byline→facts=27
  a playlist — 04-hero-made-1280x860.png
      hero    top y= 15  ink height=26   first letter leans +0 px
      byline  top y= 50  ink height=18
      facts   top y= 77  ink height= 9
      block  first ink to last = 71 px   pitch hero→byline=35  byline→facts=27
```

**Nothing moved.** Three bands, 71 px of ink, a 35 px pitch from the hero to
the byline and 27 px from the byline to the facts — the same six numbers on
both pages, the same at 1280 and at 1920. That is the tier's real safety
claim: tier 1 made the two identity blocks one composition, and changing the
face did not cost a pixel of it.

The hero bands differ (21 px against 26 px) for a reason that is about the
strings and not the size: `Ochre` has no ascender above cap height and no
descender; `Road Trip` has a `d` and a `p`.

The two heroes' own y differ (27 against 15) because the record's page leads
with a breadcrumb and the playlist's with a place header — a chrome difference
that predates this study by weeks and is not touched here.

## Is that really the bundled face?

This is the failure mode worth naming, because it is the one that looks fine
here and wrong everywhere else. `Font::with_name` is a **string match**. Get
the family spelling wrong and cosmic-text does not warn, fail or draw a box —
it resolves the request against whatever serif the *host* has. The frame above
would still show a serif italic; it would just be a different one on every
machine, which is precisely what `crates/baz/src/font.rs` bundled a typeface
to end.

Frames cannot close that, so it is closed mechanically, in three assertions
that run on every build:

| assertion | what it forecloses |
|---|---|
| `font::the_family_names_baz_asks_for_are_the_names_the_faces_spell` | The family string `theme::WORK_TITLE` asks for is compared against the name the bundled bytes spell for themselves, and the face is checked to declare the **italic** style the token requests. A one-character drift silently becomes "whatever this machine owns"; now it is a red test |
| `font::the_serif_face_carries_every_letter_an_album_title_arrives_with` | A record's title is not baz's string — it is whatever the tags say. A codepoint the bundled face lacks falls back **per glyph**, so one accented letter would set half a title in a host font. Latin-1's letters and the punctuation titles arrive with are all asserted present |
| `theme::the_serif_is_the_work_titles_and_nothing_else` | The face cannot spread. `WORK_TITLE`'s consumers are an **enumerated** list of two views, and nothing may name the serif family directly, so reverting the whole experiment is still one token |

**Found writing the first of those**: the family a face spells is `name` record
**16**, not record 1. Record 1 is the legacy family and holds four styles at
most, so every weight past Regular and Bold gets a family of its own — IBM Plex
Sans Medium's record 1 reads `IBM Plex Sans Medm`, and the test's first draft
failed against the shipped, working bundle. Record 16 is what fontdb matches
on, and it is what the assertion reads.

## The long title, and why it is here

`Ochre` proves nothing hard. The serif's real risk at `SIZE_HERO` 28 is a title
that wraps, and the fixture carries one on purpose: *A Rather Considerably
Overlong Album Title That Will Clip*.
`09` / `19` `-hero-long-title-2x` shows it at two lines of italic, cut by the
`max_height(2.0 * LINE_HERO)` clip the page has always had — *"two lines is a
title; more is a paragraph"*. The italic holds its measure and the clip lands
where it did before.

## Tier 2 #8, judged from `0d` and `0e`

Doc 14 tier 2 #8 offers the save label naming its subject —
`Save these 24 as a playlist` — **only if** tier 1's `Run · ` prefix proves
insufficient in a frame. The frame says it was sufficient, so #8 is **not
taken**, and the reason is on the record rather than in a preference:

```
Run · 2 of 12 · 55:00 left                          Save as playlist
```

The strip leads with the noun. `Save as playlist` sits at the far end of a line
whose subject has already been declared, and the run's own reading — `2 of 12`,
a cursor, which no file has — is between them. Tier 2 also adds a second,
independent statement that the record below is a different sort of thing: its
title is now set in a different face from any label a playlist could wear.

Against that, #8's cost is the one doc 14 §6.3 already flagged as the single
measurement wanting a frame: the strip is `RUN_MEASURE` 440 wide, and
`Save these 12 as a playlist` is a **variable-length** label — it grows with
the digits — in the tightest strip the product draws, which also has to hold
provenance, the reading and `Undo` at the same time (`0c`/`1c` of the tier 1
set). A label that fits at 12 tracks and elides at 1284 is worse than the word
it replaced.

Recorded as weighed and declined, not missed.
