# ADR-0005: GUI toolkit — iced

**Status**: accepted (2026-08-07) · decided by Spike A head-to-head (`git show dc13d7e` — spikes/shelf-iced, spikes/shelf-tauri)

## Context

ADR-0003 deferred the GUI choice to an empirical spike: the same 100k-track / 10k-album virtualized shelf with search-as-you-type, built in both iced 0.13 and Tauri 2 + Solid, measured on Linux — the deciding platform per the project's Linux-first-class stance.

## Evidence

**Both candidates' index/search layers are equally instant** (identical-spec Rust either way): iced filter p99 0.08–0.17 ms; Tauri Rust-side index p99 49–106 µs over 100k tracks. Search performance did not differentiate them.

**What differentiated them:**

| | iced | Tauri 2 + Solid |
|---|---|---|
| Linux system deps | **none** — pure Rust (winit/wgpu/cosmic-text), built on a machine with zero GUI dev headers | full gtk3/webkit2gtk4.1/dbus dev stack required; build impossible until a container was provisioned |
| Startup → interactive | 168 ms incl. full index hydration | vite 290 ms + shell (dev mode; not directly comparable) |
| Fling-scroll under a free-spinning wheel (Logitech MX hyperscroll) | **stable FPS, felt snappy** (user-observed) | **FPS drops, felt laggier** (user-observed) — matching the WebKitGTK image-decode jank risk flagged in `docs/research/04-tech-stack.md` |
| Widget effort | virtualized grid hand-rolled (viewport/overscan state) — the predicted tax, paid and acceptable | virtualization ergonomic (~350-line UI), excellent DX |
| Binary/deploy | single static 20.3 MiB binary (15.6 stripped) | 9.7 MiB shell + webview variance per platform |

The user's hands-on verdict on the deciding platform: iced snappier and FPS-stable; Tauri visibly degraded under scroll stress. WebKitGTK jank was Tauri's known primary risk; it materialized on first contact.

## Decision

**iced** is baz's GUI toolkit. The Tauri spike is retired.

## Consequences

- **Accepted costs**: custom widgets are built in-house (the spike's virtualized grid is the pattern); accessibility depends on iced/AccessKit maturing — track it, contribute if needed; the web-frontend contributor funnel is forgone in favor of an all-Rust codebase (one language, one toolchain, simpler CI — a real win for the ENGINEERING.md gate philosophy).
- **Retained upside**: zero system dependencies on Linux keeps packaging trivial (a `dnf install`-free build was demonstrated); single static binary; pixel-identical rendering across platforms; COSMIC-proven foundation.
- **Reversibility**: the headless-core-first architecture (ADR-0003) keeps this decision revisable — the GUI is a thin client over the `baz-core` API. If iced stalls or AccessKit integration proves inadequate, a replacement shell is a bounded rewrite, not a project rewrite.
- Thumbnail memory (spike showed ~400–500 MiB with an 800-entry 256px RGBA LRU) needs a real budget policy in production (smaller thumbs, GPU-resident textures, tighter LRU) — noted for v0.1.
