# The macOS application bundle

baz shipped a bare universal binary in a `.tar.gz`. On macOS that is not an
application: an icon, a display name and a permission string are all properties
of a **bundle**, which is a directory with a documented shape. A loose Mach-O
executable has nowhere to put them, so Finder, the Dock, Launchpad and
Spotlight had nothing to draw but the generic application mark — and
double-clicking it opened Terminal rather than baz.

```
baz.app/
└── Contents/
    ├── Info.plist                  from Info.plist.in, two substitutions
    ├── MacOS/
    │   └── baz                     the universal binary, from `lipo`
    └── Resources/
        ├── io.github.mattcree.baz.icns
        └── models/vibe/            the local CLAP towers
```

## Building one

```sh
packaging/macos/bundle.sh BINARY VERSION OUTPUT_DIR
```

**It runs on any Unix, deliberately.** Nothing in it is `iconutil`, `plutil` or
`codesign`: the icon is committed and rendered by `packaging/icons/render.sh`,
the plist is a template with two substitutions, and the structure is `mkdir`
and `cp`. A packaging change nobody can build until a release runs is a
packaging change nobody reviews, so CI assembles one on Linux over a stub
executable on every push and runs the script's own structural checks.

Those checks matter more than they look, because **every failure here is
silent**. macOS answers a malformed bundle by drawing the generic icon and
saying nothing at all. So the script reads back what it wrote: the plist
parses, `CFBundleIdentifier`, `CFBundleIconFile`, `CFBundleExecutable` and
`CFBundleShortVersionString` say what they must, `NSHighResolutionCapable` is
true, `LSUIElement` is false, the `.icns` header describes the file it is in,
and `@VERSION@` was actually substituted.

## The icon

`packaging/icons/io.github.mattcree.baz.icns`, written by `render.sh` from the
same red-circle SVG every other platform uses, and **committed** for the same
reason the hicolor PNG ladder is: a release runner should not need a
rasterizer, and `iconutil` exists only on macOS — generating it there would
mean the artwork was produced by a tool nobody can run while reviewing the
change.

`render.sh` writes the container directly. `.icns` is a small documented
format — an `icns` magic, a big-endian total length, then typed chunks whose
payloads are ordinary PNGs — and it emits the ten types `iconutil` produces
from a complete `.iconset`, so a Mac reads exactly what it would have read
from Apple's own tool. It then parses its own output back and checks each
chunk's **actual pixel size** against the type that claims it, because a
512-type chunk holding a 128 px PNG is invisible until a Mac draws a blurry
icon.

## What this does not do: signing and notarization

**The bundle is unsigned and un-notarized**, and that is a decision waiting on
the owner rather than an oversight.

macOS attaches `com.apple.quarantine` to anything downloaded from a browser.
Gatekeeper then refuses to open an unsigned quarantined app, with a message
that says the application *"is damaged and can't be opened"* — which is false,
and is the single most confusing thing a first-time macOS user of an
independent application meets. `docs/INSTALL.md` documents the two ways
through it.

Fixing it properly needs:

- an **Apple Developer Program** membership (a paid annual account),
- a **Developer ID Application** certificate, kept as a CI secret,
- `codesign --options runtime` over the bundle, then `notarytool submit
  --wait`, then `stapler staple` so the ticket travels with the download.

All of that crosses an external boundary — an account, a payment, and
credentials in this repository's CI — which `AGENTS.md` reserves to the owner.
The machinery above is arranged so that adding it later is two steps in the
release workflow and no change to the bundle's shape.

## What is still unverified

**Nobody has opened this on a Mac.** Everything above is checked structurally
on Linux, which proves the bundle is well-formed and proves nothing about what
Finder draws. The acceptance in `docs/BACKLOG.md` is the mark visible on the
`.app` in Finder, the Dock and Launchpad on a real machine, with a screenshot
in `docs/design/impl/`. That is the owner's step, for the same reason the
packaged `.exe` launch in `WORK.md` item 12 is.
