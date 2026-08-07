# Spike A (iced) — 100k-track album shelf: results

Throwaway spike per `docs/NEXT-STEPS.md` Phase 1. Question: can **iced** (latest
stable, 0.13.1) deliver a snappy virtualized album shelf over 10k albums /
100k tracks with lazy art and search-as-you-type?

Machine: i5-12600K (16 threads), Fedora Linux 44, Rust 1.92.0 stable.
No gtk3/alsa dev headers installed — **not needed**: iced's stack
(winit + wgpu + cosmic-text + pure-Rust x11rb/wayland-client) built clean with
zero system dev packages. No blockers hit.

## What was built

- `src/lib.rs` — shared dataset model, deterministic (seed 42, splitmix64)
  generation helpers, jsonl loader, case-folded substring index (one lowercase
  blob per album covering artist + title + 10 track titles).
- `src/bin/gen_dataset.rs` — writes `dataset/albums.jsonl` (10k albums, 100k
  tracks, spec-exact naming incl. every-100th unicode titles) and
  `dataset/art/{id}.png` (10k × 512×512 two-color diagonal gradients, colors
  hash-derived from album id via HSL; rayon-parallel, `image` crate).
- `src/bin/bench_search.rs` — headless filter benchmark, scripted query
  sequence, p50/p95/p99 over 200 iterations per query.
- `src/main.rs` — the iced demo app:
  - **Virtualized grid**: only visible rows (+2 overscan) are materialized;
    `Space` spacers above/below keep the scrollbar honest. Geometry tracked
    from `scrollable::on_scroll` viewport + window resize events.
  - **Lazy art**: visible-cell PNGs decoded off-thread (`spawn_blocking`),
    downscaled to ≤256px, uploaded via `image::Handle::from_rgba`, kept in an
    800-entry LRU (recency refreshed by visibility).
  - **Search box**: per-keystroke case-folded substring filter over all 100k
    tracks; view snaps to top on each keystroke.
  - **Instrumentation**: startup-to-first-frame printed on first presented
    frame (index is hydrated before the window, so it doubles as
    startup-to-interactive); per-keystroke filter time and time-to-view-commit
    (keystroke → next presented frame); F1 FPS overlay (frame subscription is
    only active when needed so idle stays idle); RSS from `/proc/self/status`
    every 5 s.

## Headless measurements

| Metric | Value |
|---|---|
| `cargo build --release` (clean, incl. all deps) | 46.2 s |
| `cargo build` / `cargo clippy` | clean, 0 warnings |
| Dataset generation (jsonl + 10k PNGs, 16 threads) | 2.95 s (jsonl alone 0.01 s); 1.1 GiB on disk |
| albums.jsonl load (index hydration) | 7.4–8.8 ms |
| Case-fold index build | 2.5 ms |
| RSS after hydration (pre-window) | 17.4 MiB |
| Binary size (release) | 20.3 MiB (15.6 MiB stripped) |

Search filter over the 100k-track index (200 iters/query):

| Query | Matches (albums) | p50 (ms) | p95 (ms) | p99 (ms) |
|---|---:|---:|---:|---:|
| `a` | 10000 | 0.043 | 0.053 | 0.079 |
| `ar` | 10000 | 0.073 | 0.091 | 0.135 |
| `art` | 10000 | 0.076 | 0.087 | 0.109 |
| `artist 1` | 5000 | 0.102 | 0.116 | 0.167 |
| `artist 19` | 500 | 0.123 | 0.136 | 0.145 |
| `track 07` | 10000 | 0.108 | 0.121 | 0.128 |
| `größenwahn` | 100 | 0.098 | 0.111 | 0.124 |

The filter is ~2 orders of magnitude under the 16 ms keystroke budget; the
in-app `time-to-view-commit` log (filter + rebuild + render) is the number
that matters interactively — watch it while typing.

Headless smoke test: with no `DISPLAY`/`WAYLAND_DISPLAY` the app prints its
hydration instrumentation then fails only at winit event-loop creation, as
expected. GUI not launched during this session per spike rules.

## How to run the interactive demo

```sh
cd spikes/shelf-iced
cargo run --release --bin gen_dataset   # once; ~3 s, writes ./dataset (1.1 GiB)
cargo run --release --bin shelf-iced    # the demo (watch stdout)
cargo run --release --bin bench_search  # headless filter benchmark
```

What to look for:
1. Stdout at launch: `[startup]` lines — jsonl hydration ms and
   startup-to-first-frame ms.
2. Fling the scrollbar / mouse-wheel hard across the 10k-album grid. Press
   **F1** for the FPS overlay; it should hold your refresh rate. Art tiles
   appear as decodes land (placeholder `…` first) — check for jank while a
   burst of thumbnails uploads.
3. Type in the search box (`artist 19`, `größenwahn`, `track 07`). Every
   keystroke logs `[search] filter … ms` and `time-to-view-commit … ms`;
   commit should stay well under 16 ms.
4. `[rss]` lines every 5 s — idle vs. after heavy scrolling (LRU capped at
   800 decoded thumbnails ≈ 200 MiB worst case; tune `LRU_CAPACITY`).

## Caveats

- Grid viewport size is inferred from scroll events plus a window-resize
  approximation (search-bar height hardcoded); a resize before the first
  scroll can mis-guess a row until the next scroll event. Fine for a spike;
  a real app would use `responsive` or layout introspection.
- Thumbnail requests are issued from `update` (scroll/search/resize), not
  from `view`, so an odd resize path could show a placeholder until the next
  scroll tick.
- FPS counter counts presented-frame callbacks over 1 s; iced only redraws
  when it has reason to, so the overlay itself forces continuous redraw while
  visible (that is what makes it a meaningful scroll-smoothness probe).
- Uses `iced::time::every` + tokio feature; no attempt to trim binary size.

## Preliminary assessment of iced for baz

**Positives**
- **Zero system-dependency build** on Linux. This is the direct answer to the
  WebKitGTK wound: nothing to apt/dnf install, one static-ish binary.
- **Performance headroom is obvious**: sub-ms filtering, ~10 ms hydration of
  100k tracks, 17 MiB pre-window RSS, and a GPU-drawn grid that only
  materializes ~40 cells regardless of library size.
- **Text rendering** (cosmic-text) handled the unicode torture titles
  (`Ålbum № … Größenwahn: Живопись`) with no work on our side.
- **Image ergonomics are good**: `Handle::from_rgba` + off-thread decode +
  LRU was ~40 lines; no texture-atlas or lifetime fights.
- The Elm loop made the instrumentation trivial and honest (keystroke →
  presented frame is directly observable via `window::frames()`).

**Costs / risks**
- **No built-in virtualized list/grid.** The spacer-plus-slice trick works but
  is hand-rolled state (offset, viewport size, overscan) that a real app must
  own carefully — this is the largest widget-effort line item vs. a DOM/CSS
  world where the platform gives you more.
- View is rebuilt per update; fine here because virtualization keeps the
  widget count tiny, but it is a discipline the codebase must keep.
- Styling/theming is programmatic (fine for us), and some layout information
  (widget's own size) is awkward to get outside `responsive`.
- 0.13 → 0.14 API churn is real; expect mechanical migration work.

**Verdict (Linux leg)**: iced comfortably clears every headless target and the
architecture gives no reason to expect scroll or keystroke misses — pending
the interactive scroll/FPS/RSS check on this machine and the macOS/Windows
legs. Compare against Spike A-tauri's WebKitGTK numbers before the ADR.
