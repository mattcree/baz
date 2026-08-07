# Spike A (Tauri flavor) — 100k-track shelf: results so far

Status date: 2026-08-07. Machine: i5-12600K (16 threads), 32 GiB, Fedora (kernel 7.0.11-200.fc44), Rust 1.92, Node 26, npm 11.

**Question this spike answers:** can Tauri 2 + Solid deliver a snappy virtualized album shelf over 100k synthetic tracks (10k albums with art), with the search index living in Rust behind IPC?

## What was built

```
spikes/shelf-tauri/
├── crates/shelf-index/     Plain Rust crate, zero system deps. The search index
│                           (case-insensitive substring over title/artist/tracks,
│                           lowercase haystacks, windowed results) + 3 bins:
│                           gen-dataset, bench, gen-icons. 2 unit tests pass.
├── dataset/                Generated: albums.jsonl (10,000 albums / 100,000
│                           tracks, 3.3 MB) + art/{id}.png (10,000 × 512×512
│                           diagonal-gradient PNGs, ~1.1 GB). Gitignored.
├── src/                    Solid + TS UI, shared by both run modes:
│   ├── App.tsx             hand-rolled virtualized grid (absolute-positioned
│   │                       cells over a spacer, overscan 3 rows), search box,
│   │                       lazy <img> art, F1 FPS overlay, footer HUD
│   ├── backend.ts          Backend interface: TauriBackend (invoke + custom
│   │                       art protocol) / WorkerBackend (Web Worker) — chosen
│   │                       at runtime by __TAURI_INTERNALS__ detection
│   ├── worker/search.worker.ts  browser-mode index (same window discipline)
│   └── metrics.ts          keystroke p50/p95 recorder, rAF FPS meter
├── src-tauri/              Tauri 2 shell (own workspace, excluded from the
│                           root workspace so everything else builds today).
│                           Commands: stats, search(query, offset, limit) →
│                           only the visible window, never the full library.
│                           Art via register_uri_scheme_protocol("shelfart") —
│                           bytes over the webview network stack, no IPC base64.
└── dist/                   vite production build — GREEN, TypeScript clean.
```

### IPC discipline (the point of this spike)

- `search` returns `{total, offset, items: AlbumHit[], index_us}` where `items`
  is at most the requested window (UI asks for visible rows + overscan, ~60–150
  albums ≈ 5–15 KB JSON). `limit` is clamped server-side at 1,000. Track lists
  are never serialized. The UI re-requests a window when scrolling drifts
  outside the cached one (rAF-throttled, stale-response guarded).
- Album art never crosses IPC: `shelfart://localhost/{id}.png` (Linux) is
  handled by a custom protocol reading from `dataset/art/`.
- `index_us` (Rust-side time) rides along in every response, so the frontend
  logs IPC overhead = RTT − index time, per query.

### Dataset spec (deterministic, seed 42 — must match the iced spike)

- ids `"00001"…"10000"`; titles `"Album {i:05}"`, every 100th (i % 100 == 0)
  `"Ålbum № {i} — Ethereal Größenwahn: Живопись"`; artists `"Artist {0001..2000}"`
  round-robin (`(i-1) % 2000 + 1`); tracks `"Track {01..10} of Album {i:05}"`.
- Year 1960..=2025: `1960 + splitmix64((42 << 32) + i) % 66` (SplitMix64
  finalizer; see `crates/shelf-index/src/lib.rs::spec`).
- Art colors: FNV-1a 64 of `id` and of `id + ":b"`, 3 bytes each from bits
  40/24/8; 512×512 diagonal gradient `t = (x+y)/(2·511)`, linear RGB lerp,
  rounded. JSONL field order: id, title, artist, year, tracks (serde_json,
  one object per line).
- Regenerate: `cargo run --release --features gen --bin gen-dataset` (spike
  root; ~4 s with rayon). Verified: 10,000 lines, 100,000 tracks, 10,000 PNGs,
  spot-checked pixels and unicode titles.

## Measurements so far

### Rust index benchmark (this machine, release, `cargo run --release --bin bench`)

Load + index 10,000 albums / 100,000 tracks from JSONL: **8.4 ms**.

Scripted queries, 300 iterations each after 25 warmup, window = 60:

| query        | matches | p50     | p95     | p99     | max     |
|--------------|--------:|--------:|--------:|--------:|--------:|
| "a"          | 10,000  | 32.2 µs | 35.1 µs | 48.8 µs | 83.0 µs |
| "ar"         | 10,000  | 59.6 µs | 63.7 µs | 66.9 µs | 70.3 µs |
| "art"        | 10,000  | 65.9 µs | 68.1 µs | 70.2 µs | 76.4 µs |
| "artist 1"   |  5,000  | 75.8 µs | 78.2 µs | 79.5 µs | 86.4 µs |
| "artist 19"  |    500  | 91.8 µs | 94.5 µs | 98.4 µs | 116.2 µs |
| "track 07"   | 10,000  | 88.0 µs | 91.7 µs | 105.9 µs | 150.9 µs |
| "größenwahn" |    100  | 75.9 µs | 77.9 µs | 78.6 µs | 78.9 µs |

**Takeaway:** worst case ~0.15 ms — the index is ~100× under the 16 ms
keystroke budget. Whatever latency the full app shows will come from IPC
serialization and the webview render path, not from search. That is exactly
what the Tauri-mode `[ipc]` console log is instrumented to isolate.

### Frontend

- `npm run build` (tsc --noEmit + vite build): **green**, 204 ms, main chunk
  17.4 kB (7.2 kB gz).
- `cargo test -p shelf-index`: 2/2 pass (spec determinism, search windowing +
  unicode + offset).
- Vite dev server smoke-tested headless: `/`, `/dataset/albums.jsonl`
  (3.4 MB, ndjson), `/dataset/art/00042.png` (image/png) all 200.
- Browser-mode render measurements (startup-to-interactive, keystroke
  p50/p95, FPS): **not yet taken** — instrumented and ready, needs a human
  (or the machine's browser) at the keyboard; see commands below.

## Blocker: native build (expected, confirmed)

`cargo build` in `src-tauri/` fails at the first native `-sys` crate. Exact
error, for the record:

```
error: failed to run custom build command for `libdbus-sys v0.2.7`
  Package dbus-1 was not found in the pkg-config search path.
  The system library `dbus-1` required by crate `libdbus-sys` was not found.
```

`pkg-config --list-all` shows **zero** gtk/webkit/glib `.pc` files, so glib-sys,
gtk-sys, soup3-sys and webkit2gtk-sys will fail identically right behind it.
This machine has no GUI-toolchain devel packages at all.

### Fedora packages to unblock

```sh
sudo dnf install webkit2gtk4.1-devel gtk3-devel dbus-devel \
    openssl-devel librsvg2-devel libappindicator-gtk3-devel \
    pkgconf-pkg-config gcc gcc-c++ make
```

(`webkit2gtk4.1-devel` + `gtk3-devel` + `dbus-devel` are the ones the build
actually demands; the rest is the standard Tauri 2 Fedora prerequisite list so
`tauri build`/bundling won't stall later. `librsvg2-devel` +
`libappindicator-gtk3-devel` are bundling/tray extras — droppable for a pure
`tauri dev` run.)

Everything else is done: `src-tauri` is fully written (commands, custom
protocol, capabilities, icons generated, tauri.conf.json), it is excluded from
the root cargo workspace so nothing else is blocked, and `dist/` is already
built for `frontendDist`. Note the Rust shell has **not compiled yet** — expect
possibly a round of trivial API fixes on first real build.

## How to run

### (a) Browser mode — works today

```sh
cd spikes/shelf-tauri
npm run dev          # then open http://localhost:5173 in Chrome
```

What to look for:
- Console: `[startup] interactive at …ms` on load.
- Type into the search box (try `artist 19`, `track 07`, `größenwahn`, and
  fast backspacing). ~1 s after you stop, console + green footer text print
  `[keystrokes n=…] filter p50/p95 | commit p50/p95 | index p50 | overhead`.
  **commit p95 is the number to judge against the 16 ms target.**
- F1 toggles the FPS overlay; flick-scroll the grid hard and watch for drops
  below ~55 (Chrome here is only a proxy for WebKitGTK).
- Placeholder-striped cells while scrolling = windowed fetch catching up;
  they should backfill within a frame or two.

### (b) Tauri mode — after installing the packages above

```sh
cd spikes/shelf-tauri
npm run tauri dev              # dev loop (vite + rust shell)
# release-feel run:
npm run build && npm run tauri build -- --no-bundle
./src-tauri/target/release/shelf-tauri
```

What to look for, beyond the browser-mode checklist:
- stderr: `[rust] indexed 10000 albums / 100000 tracks in …` (~10 ms).
- Console: `[ipc] q="…" rtt=…ms index=…ms` per keystroke/scroll-fetch — **rtt
  minus index is the true IPC tax**; footer shows it live. If rtt p95 stays
  ~1–2 ms, IPC is a non-issue; if it spikes on 150-item windows, that is the
  finding.
- Art loads through `shelfart://` — check the network tab / no base64 anywhere.
- Same FPS + keystroke drills, now on WebKitGTK — this is the decider per
  NEXT-STEPS (worst-case Linux). Also record RSS at idle and after heavy
  scrolling (`ps -o rss= -p $(pgrep -f shelf-tauri)`), and WebKit's
  `WPEWebProcess`/`WebKitWebProcess` too — the webview process holds the
  decoded images.

## Preliminary assessment (honest, pre-native-build)

- **DX:** very good so far. Solid + Vite iteration is instant; the whole UI is
  ~350 lines. The dual-backend seam (worker vs invoke) cost almost nothing and
  means render-path findings in Chrome carry over structurally. Rust-side DX
  is clean: index crate is dependency-light and testable without any GUI.
  The sharp edge is environmental: Tauri's Linux system-dep wall (this
  blocker) is real friction iced would not have.
- **Search:** a non-problem. 0.03–0.15 ms worst case in Rust; even the JS
  worker version will be single-digit ms. The 16 ms budget will be spent
  in the webview: style/layout of swapped-in cells and image decode.
- **Virtualization:** hand-rolled absolute-position grid renders ≤ ~120 cells
  regardless of library size; total-count spacer means scrollbar behavior is
  exact. Risk to watch on WebKitGTK: img decode jank during flick scrolls
  (10k × 512² PNGs; decoding="async" + lazy helps, and real thumbnails should
  be pre-scaled ~160px — 512px sources are deliberately punishing here).
- **Where IPC will bite (predictions to verify):** (1) per-keystroke +
  per-scroll-window JSON serialization — bounded by the window clamp, should
  be ~1 ms, but WebKitGTK's invoke transport is slower than Chrome's — measure
  `rtt − index`; (2) scroll-driven refetch latency showing as placeholder
  flash at high fling velocity — overscan hides some, an LRU window cache
  would hide more; (3) the custom protocol serves art sequentially per
  request — fine for PNGs on SSD, but this is where cover caching would go.
  Nothing so far suggests the architecture (index in Rust, windows over IPC)
  is wrong — it kept every payload small by construction.
- **Not yet known:** everything WebKitGTK — cold start, scroll FPS, RSS. Those
  are the accept/reject criteria in NEXT-STEPS and need the native build.
