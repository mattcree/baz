# The bundled typeface — IBM Plex

baz ships its own faces and sets them as the application default. The reason is
in `docs/design/02-visual-language.md` §1.1(1) and §2.2.1: `iced::Font::DEFAULT`
is `Family::SansSerif`, a *generic* family each platform resolves for itself,
and baz then asks that unknown family for Medium and Semibold. When the resolved
family has no such face the fallback lands somewhere else entirely — on the
design audit's machine it landed on a **monospace**, so tile titles were
typewritten while the artist lines beside them were proportional. The same
argument `src/icon.rs` used to reject system glyphs ("a player should look the
same everywhere") applies to the whole interface's voice.

These bytes are compiled into the binary by `src/font.rs` (`include_bytes!`) and
handed to `iced::application(…).font(…)`, with `.default_font(…)` naming the
family. No new crate, no runtime file lookup, no install step.

## Provenance

| File | Face | Version | Bytes | SHA-256 |
|---|---|---|---|---|
| `IBMPlexSans-Regular.ttf` | IBM Plex Sans Regular | 3.005 | 200 500 | `975dcda37d80f038dcd143c22e33ca2d97a0cc5a929aace1c749153b0fe1afa5` |
| `IBMPlexSans-Medium.ttf` | IBM Plex Sans Medium | 3.005 | 202 460 | `331c8639d7598b2cde62a911a71db195e30cb655cd6bdf2e324a7e984955f907` |
| `IBMPlexSans-SemiBold.ttf` | IBM Plex Sans SemiBold | 3.005 | 202 632 | `a20caf8286023a6a7a85e40b1d2a4ae9fc3e3b1f9eda8f4c542dd4986af67bb1` |

Total: **605 592 bytes** — one family at three weights.

Two faces were deleted, taking **395 928 bytes (39.5 %)** with them
(`.interface-design/system.md` §8):

| File | Bytes | Why it went |
|---|---|---|
| `IBMPlexMono-Regular.ttf` | 173 052 | baz has no monospace: Plex Sans's figures are already tabular. See "The metrics this asset is accountable for" below. |
| `IBMPlexSerif-SemiBold.ttf` | 222 876 | baz has no display face: the room supplies nothing and the work supplies everything. The album title is Sans SemiBold at 22. |

Fetched 2026-08-08 from <https://github.com/IBM/plex>, branch `master` at commit
`bf260093582f04622aacc1e9f9ca604d7ccd0c42`, from
`packages/plex-sans/fonts/complete/ttf/` (the two deleted faces came from
`plex-mono` and `plex-serif` beside it). `OFL.txt` is that repository's
`LICENSE.txt`, unaltered.

## Subsetting: none, deliberately

**These are the upstream files byte for byte.** Re-downloading them from the
commit above and comparing the hashes in the table is the whole verification.

Subsetting was measured before it was rejected: `hb-subset` down to Basic Latin
+ Latin-1 + Latin Extended-A + the punctuation baz uses (`·` `—` `→` `−` `…`
`“” ‘’`) produced 335 556 bytes across the five faces it was measured on,
saving ~666 KB. It was not taken, for two reasons.

1. **The Reserved Font Name.** OFL-1.1 §3 forbids a *Modified Version* of the
   font software from using the Reserved Font Name — here, "Plex". A subset is a
   Modified Version. Shipping a subset still called "IBM Plex Sans" is at odds
   with the licence we are relying on to ship at all; renaming the family to
   dodge that would hide the provenance this file exists to record and would
   make `Font::with_name` name a face nobody can look up. Verbatim
   redistribution is unambiguously permitted and needs no argument.
2. **baz renders other people's tags.** Album and artist strings come out of the
   user's files, not out of this repository. The complete faces carry Latin (incl.
   Extended-A), Greek and Cyrillic — 1 019 glyphs in Sans. A Latin-only subset
   would push a Cyrillic or Greek album title back onto
   whatever the host machine happens to have, reintroducing for a real part of a
   real collection exactly the "different product on every machine" problem
   bundling is here to fix.

The honest cost is stated rather than avoided: ~666 KB of binary that a subset
would have saved, against a licence question that would have needed answering
and a script that would silently fall back.

**Coverage is still not universal.** CJK, Hebrew, Arabic, Devanagari and the
rest are not in these faces and are not going to be — cosmic-text falls back to
the platform's fonts for any codepoint Plex does not carry, exactly as it does
today. What bundling guarantees is that every glyph baz *itself* draws, and the
Latin/Greek/Cyrillic bulk of Western tag data, is the same everywhere.

## Licence

SIL Open Font License 1.1 — see `OFL.txt`. GPL-compatible (FSF licence list), so
it may ship inside a GPL-3.0-or-later binary. Copyright © 2017 IBM Corp. with
Reserved Font Name "Plex".

The OFL's obligations, and how they are met:

- *Ship the licence.* `OFL.txt`, verbatim, beside the fonts, and this file names
  it. The compiled binary is a bundled work rather than a distribution of the
  font software on its own; source releases carry `OFL.txt` directly.
- *Do not sell the fonts on their own.* baz does not.
- *Do not use the Reserved Font Name on a modified copy.* Nothing here is
  modified — see above.

`cargo deny` does **not** need an `OFL-1.1` entry in `deny.toml`: it walks the
Cargo dependency graph, and a checked-in asset is not a crate. Verified by
running `cargo deny check` with these files present.

## Which faces, and why these

`.interface-design/system.md` §8 and `docs/design/02-visual-language.md` §3 ask
for exactly:

- **Sans Regular / Medium / SemiBold** — the interface voice, the album title,
  *and every figure baz draws*. All three exist as real drawn faces, so nothing
  is synthesised and nothing falls back.

That is the whole list. **No monospace**, for a reason that is measured rather
than tasteful (below). **No serif**: a display face is the room supplying
personality, and this room supplies nothing. No italics: baz sets none.

## The metrics this asset is accountable for

**IBM Plex Sans ships tabular figures by default.** Every digit advances
600/1000 em in Regular, Medium *and* SemiBold — the same advance the deleted
Plex Mono gave — with no kerning between digits and no default-on substitution
that touches them. `0:00:00` and `9:59:59` therefore measure 43.008 px each at
`SIZE_META`, to 0.000 px: a timestamp ticking cannot move its neighbour, and a
column of durations stays a column.

That is the whole licence for shipping one family instead of two, and it is
asserted against these very bytes by `src/font.rs`'s
`the_sans_carries_baz_s_tabular_figures_in_every_weight_it_sets_them_in`.

Every fixed-width slot in the pixel-stable bottom bar is sized against a face's
real advances, so `src/font.rs`'s tests parse these files (`head`, `hhea`,
`hmtx`, `cmap`) and measure the real advance width of each worst-case string
against the token reserving it. Changing a file here without re-running
`cargo test -p baz` is how a duration silently clips.

Deleting the mono *fixed* a clip rather than risking one: `STAMP_W` is 52 px and
`10:00:00` measures 57.60 px in Plex Mono, so the shipped build could not draw a
ten-hour track's timestamp. In Plex Sans it is 50.21 px.
