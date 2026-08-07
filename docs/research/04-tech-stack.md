# Tech Stack Evaluation

> Research groundwork for baz, 2026-08-07. Language, audio, GUI, index, and architecture options for a snappy cross-platform player; ends with a recommendation.

## 1. Core language

**Rust — clear winner.** Realtime-safe audio threads without GC, fearless concurrency for indexer/watcher/decoder pipelines, and the exact ecosystem this app needs already exists (Symphonia, cpal, rusqlite, notify, Tauri). Contributor appeal is decisive for a new OSS project: Rust consistently tops "most admired" surveys and new media-tooling contributors overwhelmingly land there.

- **C++**: the incumbent (foobar2000, fooyin, Strawberry) and Qt is superb, but new OSS C++ projects struggle to attract contributors, and memory-safety bugs in decoder/tag-parsing code (hostile input!) are a real liability.
- **Zig**: still pre-1.0 in 2026, per-release churn, essentially no audio/GUI ecosystem — reject for now.
- **Go**: GC in audio callbacks is manageable but not ideal; cgo required for every audio API; GUI story (Fyne — see Supersonic) produces mediocre-feeling apps — reject.

## 2. Audio stack

- **Symphonia** (pure Rust demux/decode: FLAC, ALAC, AAC, MP3, Vorbis, WAV…): explicitly supports **gapless** (padding/delay handling), decode perf within ±15% of FFmpeg, 100% safe Rust. Caveat: slow release cadence — plan to vendor/patch. Use as the decode layer.
- **Output**: **cpal** covers shared-mode WASAPI/CoreAudio/ALSA well, but **WASAPI exclusive mode is an open issue (#459) — not implemented**. For bit-perfect exclusive output write thin per-platform backends: the `wasapi` crate (exclusive + event-driven), CoreAudio hog-mode via `coreaudio-rs`, ALSA direct `hw:` via the `alsa` crate. Gapless is then *your* mixer's job: one continuous device stream fed by a ring buffer, decode-ahead of track N+1, sample-accurate splice — the architecture foobar2000 uses.
- **miniaudio** (C): good low-level device layer incl. WASAPI exclusive; viable via FFI but duplicates cpal + platform crates, weaker decoders than Symphonia. Skip.
- **GStreamer**: gapless via playbin `about-to-finish` has documented bugs/hangs; heavy runtime dependency to ship on Windows/macOS. Wrong for a lean cross-platform core.
- **BASS**: **proprietary; free only non-commercial, paid commercial licenses** — incompatible with an open-source project. Reject outright.
- **FFmpeg**: broadest coverage, but C API, LGPL/GPL build-matrix headaches, binary bloat. Keep as an *optional* fallback decoder plugin later; not the core.

## 3. GUI

- **iced**: Elm-style, MIT, GPU-rendered; **proven at scale by System76's COSMIC desktop**, plus Halloy/Sniffnet. Best "serious native Rust" bet. Weaknesses: custom widgets are on you; accessibility (AccessKit) partial; text stack good but behind browsers.
- **egui**: fastest to prototype, but immediate-mode "tool" look — wrong for a beautiful consumer player.
- **Slint**: polished, shipped in audio products; but dual-licensing friction and DSL lock-in for an OSS community project.
- **gpui** (Zed): gorgeous performance but Zed-shaped, pre-1.0, frequent breaking changes. Too risky as a foundation in 2026.
- **Xilem/Vello**: still alpha. Watch, don't adopt.
- **Tauri 2**: production-ready; ~8MB installs, far lower RAM than Electron; real music players ship on it (**Museeks migrated Electron→Tauri**, Moosync, rustmusic with DSD/exclusive output). Best-in-class text rendering, accessibility (real DOM = screen readers work), unmatched custom-design freedom (album shelves, WebGL/WebGPU visualizers, CSS-driven panel layouts). Risks: per-platform webview variance — **WebKitGTK on Linux is the weak leg** — and IPC discipline (raw/custom-protocol responses, never JSON-serialize 500k rows).
- **Electron**: reject — ≈280MB/2.5s startup vs Tauri's 45MB/0.3s in 2026 benchmarks directly contradicts "exceptionally snappy," even though Plexamp/Feishin prove it can ship.
- **Qt/QML**: technically the safest route to a great player (fooyin, Strawberry, Elisa) but drags you to C++ or to cxx-qt (early, changing API); LGPL linking adds packaging friction.
- **Flutter**: desktop audio via media_kit/libmpv; gapless is "highly experimental." Reject for a bit-perfect player.

## 4. Library index

**SQLite (+FTS5) as durable store, in-memory index for search.** FTS5 with `prefix='2 3 4'` gives 10–30ms prefix queries — good, but the foobar2000/MusicBee "instant" feel comes from holding the entire library metadata **in RAM** (500k tracks ≈ 100–300MB interned strings) and matching lowercase-folded haystacks per keystroke; a parallel linear scan or trigram map returns in low single-digit ms with zero I/O. So: rusqlite + FTS5 for persistence/cold start/complex queries; hydrate an in-memory search structure at launch; incremental updates via the **notify** crate (inotify/FSEvents/ReadDirectoryChangesW/kqueue under one API) with debouncing, plus periodic full rescans for network mounts where watching is unreliable.

## 5. Architecture

Build **headless-core-first**: a `baz-core` crate (playback engine, decode graph, library DB + index, watcher, playlist/queue model) exposing an async command/event API, with the GUI as a thin client in the same process. This is MPD's model (decoupled protocol) and Roon/Plexamp's core/remote split, without paying the daemon tax on day one. Later, a `baz-served` binary wraps the same crate behind HTTP and speaks **OpenSubsonic** — the de-facto open standard in 2026 — which instantly gives baz's future server mode dozens of free clients. Design the core API as serde-serializable messages from the start so in-process vs. remote is a transport swap.

## Recommendation

**Primary: Rust workspace** — `baz-core` (Symphonia decode → custom gapless ring-buffer engine → cpal shared-mode + native `wasapi`-exclusive/CoreAudio-hog/ALSA-`hw:` backends; rusqlite + FTS5 + in-memory search index; notify watcher) **with a Tauri 2 shell and a Solid (or Svelte) frontend** using virtualized lists. All hot paths (search, decode, index) live in Rust; the webview only paints. This maximizes design freedom (shelves, visualizers, future foobar-style layout engine in DOM/CSS), gets accessibility and world-class text rendering for free, and doubles the contributor funnel (Rust systems folks + web UI folks — the Museeks migration shows this works for exactly this app category).

**Runner-up: the identical `baz-core` with an iced front end** — single static binary, pixel-identical on all platforms, COSMIC-proven, no webview variance.

**Key trade-off:** Tauri buys UI velocity, aesthetics, and accessibility at the cost of platform webview inconsistency (chiefly WebKitGTK on Linux) and strict IPC discipline to keep 500k-track views instant; iced buys total rendering control and a lean single binary at the cost of hand-building every widget, weaker accessibility/text infrastructure, and a smaller UI-contributor pool. **Because both share the same headless core, the GUI bet is reversible — which is itself a reason to structure the project this way.**

## Sources

- https://github.com/pdeljanov/Symphonia
- https://github.com/RustAudio/cpal/issues/459 (WASAPI exclusive unimplemented)
- https://crates.io/crates/wasapi
- https://www.un4seen.com/bass.html (BASS licensing)
- https://gitlab.freedesktop.org/gstreamer/gstreamer/-/issues/915
- https://www.phoronix.com/news/COSMIC-Desktop-Iced-Toolkit
- https://wrenlearnsrust.com/posts/2026-03-11-rust-gui-landscape-2026.html
- https://github.com/zed-industries/zed/discussions/13694 ; https://www.gpui.rs/
- https://linebender.org/blog/tmil-25/
- https://madewithslint.com/
- https://tech-insider.org/tauri-vs-electron-2026/
- https://github.com/martpie/museeks (Electron→Tauri music player)
- https://github.com/larevuegeek/rustmusic (Tauri 2 audiophile player)
- https://news.ycombinator.com/item?id=46776564 (Linux music players 2026 discussion)
- https://www.sqlite.org/fts5.html
- https://github.com/notify-rs/notify
- https://opensubsonic.netlify.app/docs/
- https://pub.dev/packages/just_audio_media_kit (Flutter gapless experimental)
- https://blog.jetbrains.com/blog/2026/06/05/why-zig-isn-t-1-0-yet/
- https://github.com/KDAB/cxx-qt
