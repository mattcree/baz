# Handoff: baz — desktop music player (Rust + iced 0.13)

## Overview
Design package for **baz**, an album-wall-first desktop music player for people who own their music files. The thesis: the collection IS the interface — the app opens onto a wall of the user's covers; click one, it plays. This package contains the complete design system, per-surface specs, and build guidance.

## About the design files
The .dc.html files in this bundle are **design references created in HTML** — they show intended look and behavior; they are NOT production code. The task is to **implement these designs in the existing Rust + iced 0.13 codebase** using its established patterns. Open the HTML files in a browser to view them; the markdown files carry the same content in Claude-Code-friendly form and are the source of truth.

## Fidelity
**High-fidelity for the system, illustrative for composition.** All colors, opacities, type sizes, spacing ratios, and state treatments in the specs are exact and final. The board mocks use flat-color placeholder squares for album art and are scaled-down compositions — follow the spec numbers (e.g. covers >=160px real size), not measurements taken off the mocks.

## Read in this order
1. `01-foundations.md` — thesis, the four rooms (full color tokens), elevation/accent/type laws, the refusals ledger (things deliberately rejected — do not re-add).
2. `02-surfaces.md` — every surface and workflow: wall, find, needle, lenses, inspector, stack, shuffle, history, first run. Each spec cites its mock id on the board.
3. `03-build-guide.md` — v1 vs deferred scope, dependency-ordered build plan, iced-specific workarounds, open questions, definition of done.

## Hard constraints (real, from the codebase)
No rounded/clipped images. No OpenType feature control. Container borders are 4-sided only. No animation runtime (design assumes hard cuts). No icon set — the total glyph inventory is 4 hand-rasterised polygons (play/pause/prev/next). No accessibility tree; buttons take no keyboard focus (global shortcuts still work). Grid virtualizes 100k+ tracks; cold start budget ~200ms.

## Design tokens (summary — full tables in 01-foundations.md)
- Rooms (wall / panel+1 / float+2 / recess-1 / ink):
  - Closing Time: #0C0D0E / #181716 / #242120 / #070809 / #E8E4DB (default for OS dark)
  - Stone: #4A463F / #55504A / #605B54 / #3F3B35 / #EDE9E0
  - Plaster: #B7B0A4 / #AAA296 / #9D9589 / #C2BBAE / #211F1C
  - Reading Room: #E9E4D9 / #DFD9CC / #D5CFC2 / #F2EEE5 / #17161A (default for OS light)
- Accents (playback truth ONLY, never fills): amber oklch(0.74 0.13 75) in dark rooms (glow only in Closing Time); oxblood oklch(0.50 0.14 35) in light rooms.
- Type: IBM Plex Sans only. 9-10px caps 0.14em tracking (headers/labels), 11px working UI, 13px names, poster sizes only in Marquee + filter query. Ink opacity = hierarchy: 100/65/40/35%.
- Elevation: 4 levels max, >=0.03 oklch-L per step; hairline edges (ink 8-14%) separate, surface deltas group; no shadows anywhere; no radii; artwork square and borderless.

## Assets
No image assets — album art comes from the user's files; mocks use flat-color placeholders. Font: IBM Plex Sans (bundle it; do not depend on Google Fonts at runtime).

## Files
- `01-foundations.md`, `02-surfaces.md`, `03-build-guide.md` — the spec (source of truth)
- `baz critique.dc.html` — the visual board: all mocks, referenced by id (1a...13c) throughout the specs
- `baz 1 — foundations.dc.html`, `baz 2 — surfaces.dc.html`, `baz 3 — build guide.dc.html` — printable HTML versions of the specs
- `support.js`, `doc-page.js` — runtime for viewing the HTML files in a browser; irrelevant to implementation
