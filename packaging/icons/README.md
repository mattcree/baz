# The application icon

baz's mark, in the [freedesktop icon theme] layout every Linux launcher, the
Flatpak and the AppStream metadata read from.

```
io.github.mattcree.baz-small.svg          source for 16, 24 and 32 px only
render.sh                                 regenerates every PNG below
hicolor/scalable/apps/…baz.svg            the master, and an installed icon
hicolor/{16,24,32,48,64,128,256,512}x…/apps/…baz.png
```

The basename is the application id, which is what lets the desktop entry say
`Icon=io.github.mattcree.baz` with no path and every launcher find the right
size for itself. `packaging/README.md` lists the other five places that string
appears; the icon is now the sixth.

## What the mark is

**A work hanging on the gallery wall, lit by the one picture light, with its
wall label beneath it.** That is baz's signature as
`.interface-design/system.md` §1.1 defines it — not an illustration of music,
and deliberately not a note, a disc, a waveform or a play triangle, all of
which describe the category rather than this product.

Everything in it is drawn from the shipped design system, so the icon and the
running application are the same object:

| Part | Where it comes from |
|---|---|
| the wall, `#0C0D0E` falling to `#060708` | `WALL` and `RECESS`, and the shadow gap between them (visual language §4.2) |
| the work, hard-square, no radius | "nothing is ever drawn on top of a sleeve"; the shelf has no radii (§1.2 of the system) |
| its colour, `#4A3A55` → `#26202E` | inside §4.3's missing-artwork gamut: S 0.10–0.28, L 0.16–0.30 |
| the letterform | §4.3's placeholder again — the title's first character, in the bundled IBM Plex Sans SemiBold. For baz that character is `b` |
| the halo | `LAMP_GLOW`: `LAMP` `#E3A14E` at 45 %, blurred |
| the label, two lines, left-aligned under the work | the wall label (§1.2), `PAPER` over `PAPER_DIM` |
| the lamp dot on line one | the playing dot — the work is sounding |

The accent appears exactly twice, and both are playback truth. That is the
accent discipline (§5.3) holding in the one picture of baz a person sees before
they have ever run it.

**Two deliberate departures from the spec**, because an icon is not a shelf
tile and pretending otherwise would produce an unreadable one:

- **The letterform is `PAPER` at 50 %, not 12 %.** On the shelf the placeholder
  whispers because it is surrounded by real covers that should be louder. An
  icon has nothing around it; at 12 % the mark is a plain rectangle at every
  size below 128 px.
- **The canvas has a 26 px corner radius** though nothing else in baz has a
  radius at all. A launcher grid needs a silhouette. The work inside it stays
  hard-square, and the contrast between the two is the point rather than an
  inconsistency.

## Two sources, and where they cross over

`hicolor/scalable/apps/io.github.mattcree.baz.svg` is the master and renders 48
px and up. `io.github.mattcree.baz-small.svg` renders 16, 24 and 32.

The small source changes two things and only two: the work grows (152 → 176 in
a 256 viewBox), and the wall label loses its second line while the first
thickens. Below about 48 px the label's two lanes are each under one device
pixel and composite into a single grey smudge that says less than one
deliberate line does — so the small source says the one line properly instead.
Size-specific artwork is what the icon theme spec's per-size directories are
*for*; this is not a workaround.

## Regenerating

```sh
packaging/icons/render.sh
```

Needs ImageMagick **built against librsvg** — ImageMagick's own MSVG renderer
draws none of the gradients or the blur and would silently emit flat
rectangles, so `render.sh` refuses to run without it. Check with
`magick -list format | grep SVG`; on Fedora the package is `librsvg2-tools`.
No other tooling is required: `rsvg-convert` need not be on `PATH`, and nothing
here needs Inkscape.

The PNGs are **committed**, not generated at install time. A packager should
not need a working rasterizer, and a Flathub build has no network and no
librsvg in the freedesktop SDK. The script strips timestamps and colour
profiles, so a regeneration that changed nothing produces no diff — which is
what makes "did the SVG actually change?" answerable by `git status`.

The letterform is a real glyph outline, not a redrawn shape: it was extracted
once from `crates/baz/assets/fonts/IBMPlexSans-SemiBold.ttf` with `fontTools`,
scaled to 0.62 (master) and 0.66 (small) of the work's edge, and optically
centred on its ink box rather than on its advance width. It is inlined as a
`<path>` so the SVG has no font dependency. The face is OFL-1.1 and already
committed with its licence and provenance
(`crates/baz/assets/fonts/README.md`); OFL permits embedding the outline, and
this is not a new licence question.

## Installing it

The Flatpak manifest installs the whole ladder; see the `build-commands` in
`../flatpak/io.github.mattcree.baz.yml`. By hand, or from a release tarball:

```sh
for s in 16 24 32 48 64 128 256 512; do
  install -Dm644 "packaging/icons/hicolor/${s}x${s}/apps/io.github.mattcree.baz.png" \
    "$HOME/.local/share/icons/hicolor/${s}x${s}/apps/io.github.mattcree.baz.png"
done
install -Dm644 packaging/icons/hicolor/scalable/apps/io.github.mattcree.baz.svg \
  ~/.local/share/icons/hicolor/scalable/apps/io.github.mattcree.baz.svg
gtk-update-icon-cache -f -t ~/.local/share/icons/hicolor 2>/dev/null || true
```

`gtk-update-icon-cache` is optional: without it the icon still resolves, just
after a slower directory scan.

## The window icon, and why the binary does not carry one

iced 0.13 has `window::Settings::icon`, and it takes an `iced::window::Icon`
built by `iced::window::icon::from_rgba` — decoded RGBA, so wiring it means
embedding a PNG in the binary and decoding it at startup. That cost is small:
`image` with the `png` feature is already compiled in for album art, the 64 px
PNG here is 2.8 KB, and the decode is one 64 × 64 image.

It is **not** wired, for a reason that is about reach rather than cost. iced
0.13 is on winit 0.30, whose `Window::set_window_icon` documents
*"iOS / Android / Web / **Wayland** / **macOS** / Orbital: Unsupported"*. It
sets a window icon on **Windows and X11 only**. Wayland has no per-window icon
protocol that winit 0.30 implements, so on the platform baz is developed and
used on, the icon a person sees comes from the `.desktop` entry's `Icon=` key
resolving against this directory — which is exactly what was wired here — and
from nothing else. macOS takes it from an app bundle baz does not build.

So the window icon buys Windows and X11 sessions, and Windows ships no desktop
entry, which makes it the only place a Windows user would ever see the mark.
That is a real gap and it is recorded as one; it is deliberately not closed in
the same change as the artwork, because it is application code with its own
test and its own review, and the icon files it would embed have to exist first.
They now do. The patch is three lines in `crates/baz/src/app.rs`'s
`window_settings`, against a 64 px PNG copied into `crates/baz/assets/`:

```rust
settings.icon = image::load_from_memory(include_bytes!("../assets/window-icon.png"))
    .ok()
    .map(|img| img.into_rgba8())
    .and_then(|rgba| {
        let (w, h) = rgba.dimensions();
        iced::window::icon::from_rgba(rgba.into_raw(), w, h).ok()
    });
```

[freedesktop icon theme]: https://specifications.freedesktop.org/icon-theme-spec/latest/
