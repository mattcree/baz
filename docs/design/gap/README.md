# The visual gap — rendered evidence

Every image here supports
[`../05-toolkit-and-visual-gap.md`](../05-toolkit-and-visual-gap.md). Nothing
was touched that this work did not start.

## How the shipped frames were made

The real release binary, `--features device-output`, on a private `Xvfb`, with
**all six** redirections from
[`docs/DEVELOPMENT.md`](../../DEVELOPMENT.md#headless-ui-verification) — scratch
`HOME`, `XDG_DATA_HOME`, `XDG_CONFIG_HOME`, `XDG_CACHE_HOME`,
`XDG_RUNTIME_DIR`, and `env -u DBUS_SESSION_BUS_ADDRESS`. Every run's log
carries the receipt:

```
[mpris] no session bus; desktop media controls unavailable (I/O error: No such file or directory (os error 2))
```

so the maintainer's library, config, thumbnails and session bus were never
opened. No window manager, so each window is exactly the size it was asked for;
captures are cropped to the window's own geometry.

A throwaway 25-album / 181-track fixture of **digitally silent** FLAC — every
sample a zero, verified by decoding each file and checking the byte histogram
has one bucket — with generated cover art in six visual families, never
`~/Music`. The scratch `HOME` carries an `.asoundrc` routing ALSA's default PCM
to `null`, so the transport is real and nothing was audible: the sink discards
every sample, and every sample is already zero. `BAZ_DEVICE_TESTS` was never
set. The Xvfb displays, the fixture and the scratch trees were removed
afterwards.

Because ALSA's `null` PCM free-runs, a 30-second track is consumed in
milliseconds and the queue ends before a screenshot can be taken. The fixture
therefore carries one **one-hour** silent track, which holds a stable playing
state for about thirty seconds of wall clock.

## How the target frames were made

`../critique/baz critique.dc.html` opened in headless Chrome with a throwaway
profile — the maintainer's browser session was never touched. The handoff's
authoring runtime (`support.js`, `doc-page.js`) was never shipped with the
package, so a local shim reimplemented the three constructs the board uses
(`<helmet>`, `<sc-for>`, `<sc-if>`, plus `{{ }}` interpolation) over a
deterministic placeholder dataset, and substituted the three IBM Plex Sans faces
baz already bundles for the board's Google Fonts link. **The shim is a viewing
aid, not part of the design package**: it draws the board's own markup with its
own colours, sizes and spacing, and invents only the placeholder sleeve colours
and track names the board's data bindings expect.

## The plates

| Image | What it shows |
|---|---|
| [`00-hairline-gamma.png`](00-hairline-gamma.png) | Defect D1. The four alpha-expressed tokens, blended in sRGB (what CSS and `theme.rs`'s test do) and in linear light (what iced's renderer does). The right column matches the measured pixels to the byte. |
| [`01-wall.png`](01-wall.png) | Board 1b's wall against the shipped wall at 1280. Same cover size, same 40 px hang; different everything else. |
| [`02-playback.png`](02-playback.png) | Board 5a's needle — 2 px flush on the window edge — against the shipped 102 px bar. |
| [`03-inspector.png`](03-inspector.png) | Board 9b against the shipped 340 px inspector. Note the opaque amber **Play album** fill (D2). |
| [`04-queue.png`](04-queue.png) | Board 13a's stack against the shipped queue popover. |
| [`05-playing.png`](05-playing.png) | Playing state: halo only, against halo + ring + caption band + panel (D4). |
| [`06-rooms.png`](06-rooms.png) | Closing Time and Reading Room, both shipped, both from the real binary. |

## The frames

`shipped/` — the real binary at 1280 × 860 and 1920 × 1080 in Closing Time, and
at 1280 in Reading Room (via `BAZ_ROOM`, which resolves the room whose tokens
are defined but not yet selectable).

`board/` — individual mocks off the Claude Design board, by id: `1b` the wall,
`4a` type-to-filter, `5a` the needle, `8a` the four rooms, `9a` the groove, `9b`
sides in the inspector, `11a` the index rail, `13a` the stack.

> It is called `board/` and not `target/` because `.gitignore`'s `target/` rule
> — meant for Cargo's build directory — is unanchored and matches at any depth,
> so a `docs/design/gap/target/` full of design mocks is silently dropped by
> `git add -A` with no warning. Worth knowing before naming a docs directory.
