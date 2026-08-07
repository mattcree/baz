# ADR-0003: Core stack — Rust workspace, headless `baz-core`, GUI decided by spike

**Status**: accepted (2026-08-07) · GUI winner to be recorded in a follow-up ADR

## Context

Full analysis in `docs/research/04-tech-stack.md`. Requirements: exceptionally snappy with 100k+ track libraries, cross-platform (Windows/Linux/macOS with Linux first-class), gapless bit-perfect playback, open-source contributor appeal, and a path to an optional server/remote mode without a rewrite.

## Decision

1. **Language: Rust**, stable toolchain, single workspace.
2. **Architecture: headless-core-first.** All engine logic lives in `baz-core` (playback, decode, library DB + in-RAM search index, watcher, queue model) behind a serde-serializable async command/event API. GUIs are thin clients; a future server mode (OpenSubsonic) is a transport swap, not a rewrite.
3. **Audio**: Symphonia for demux/decode; custom gapless mixer (single continuous device stream, ring buffer, decode-ahead, sample-accurate splice); cpal for shared-mode output; thin native per-platform backends for exclusive mode (`wasapi` crate / CoreAudio hog mode / ALSA `hw:`). BASS, GStreamer, FFmpeg-as-core rejected (licensing / gapless bugs / build burden — see research/04).
4. **Persistence**: rusqlite + FTS5 as durable store; full metadata hydrated into an in-memory search index at launch; `notify` for file watching.
5. **GUI: decided empirically, not by taste.** Two candidates — Tauri 2 (+ Solid/Svelte) and iced — are built against the same 100k-track shelf benchmark (Spike A, `docs/NEXT-STEPS.md`). The winner is recorded in a follow-up ADR with the measurements attached. Linux (WebKitGTK for Tauri) is the deciding platform: failing there is failing, regardless of Windows/macOS results.

## Consequences

- The GUI bet is reversible by construction — both candidates sit on the identical core API. This hedge is a feature of the architecture, not indecision.
- Symphonia's slow release cadence is accepted; vendoring/patching is the planned mitigation.
- Exclusive-mode output requires per-platform unsafe/FFI code — the only crates exempt from `#![forbid(unsafe_code)]` (see `docs/ENGINEERING.md`).
- Spike outcomes feed ADR-0004 (audio boundary strategy: sample-rate change handling) and ADR-0005 (GUI winner).
