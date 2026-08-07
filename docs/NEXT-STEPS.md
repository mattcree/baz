# baz — Next Steps

> Concrete, ordered, with acceptance criteria. Standards in `ENGINEERING.md`; vision in `VISION.md`. Updated 2026-08-07.
>
> **Status**: Phase 0 ✅ (ADRs 0001–0003) · Phase 1 ✅ (spikes complete and deleted per standing rule; results in ADR-0004/0005 and at `git show dc13d7e`) · **Phase 2 is next.** GUI = iced (ADR-0005). Dev environment: `docs/DEVELOPMENT.md`.

## Phase 0 — Decisions (hours, not days)

| # | Decision | Options / notes | Done when |
|---|---|---|---|
| 0.1 | License | GPL-3 (fooyin/Navidrome precedent, protects against proprietary forks) vs MPL-2 (embedding-friendly) | ADR-0001 written |
| 0.2 | Name check | "baz" on crates.io, GitHub, Flathub, winget, Homebrew; decide binary name (`baz`? `bazplayer`?) | ADR-0002 written |
| 0.3 | Ratify stack | Rust workspace + headless `baz-core`; GUI decided by the Phase 1 spike, not by taste | ADR-0003 written |
| 0.4 | Repo home | GitHub org/user, issue tracker conventions | repo exists (see Phase 2) |

## Phase 1 — The two spikes (throwaway code, real answers)

Both spikes are explicitly disposable: they answer questions, they do not become the codebase.

### Spike A — GUI: the 100k-track shelf
The go/no-go on Tauri vs iced. Build the same brutal demo twice:
a virtualized album-shelf grid over 100k synthetic tracks (10k albums with art), with search-as-you-type filtering.

**Measure** (all three OSes, worst-case Linux/WebKitGTK is the decider):
- cold start to interactive
- keystroke → filtered-view latency (target: < 16 ms perceived, instrument it)
- scroll smoothness at speed over the art grid (dropped frames)
- RSS memory at idle and after heavy scrolling

**Accept**: the winner hits targets on all platforms; if Tauri fails only on WebKitGTK, that is a fail (Linux is not a second-class citizen — that's the wound we're healing).

### Spike B — Audio: the gapless engine core
Symphonia decode → ring buffer → cpal output, playing two FLACs gaplessly.

**Prove**:
- sample-level continuity across the track boundary (synthesized-sine test from `ENGINEERING.md`, verified by loopback capture or output-buffer inspection)
- decode-ahead of track N+1 while N plays, without touching the audio thread's guarantees
- behavior across a sample-rate change at a track boundary (documented strategy: reopen stream vs resample — measure the gap)
- one platform exclusive-mode proof (ALSA `hw:` or WASAPI exclusive via the `wasapi` crate) with bit-exactness sanity check

**Accept**: gapless verified by test, not by ear; a written note on the sample-rate-change strategy becomes ADR-0004.

## Phase 2 — Scaffold: repo + CI before features (milestone zero)

1. `git init`, workspace layout: `crates/baz-core`, `crates/baz-ui` (winner of Spike A), `xtask` for automation.
2. Install the full CI pipeline from `ENGINEERING.md` — fmt, clippy `-D warnings`, tests, doc build, cargo-deny, MSRV, coverage, 3-OS matrix — **green on the empty workspace**.
3. `README.md` (vision one-pager + development-model statement incl. the AI policy), `CONTRIBUTING.md` (points at `ENGINEERING.md`), `LICENSE`, ADRs 0001–0004, CI badges.
4. First fuzz target wired (even if it fuzzes a trivial parser) so the scheduled-fuzzing lane exists from day one.

**Accept**: a stranger cloning the repo sees a project that takes itself seriously before it does anything.

## Phase 3 — v0.1 "it plays": one vertical slice

Scope (from `VISION.md`, deliberately minimal):
- scan a directory → SQLite + in-RAM index (tags via lofty or equivalent; folder-structure inference for untagged files)
- shelf view with album art, populating live during scan
- click album → gapless playback, front to back
- search-as-you-type over the whole library
- fixed layout with hideable panels; media keys + MPRIS on Linux

**Accept**: the `research/05` first-run target — under 60 seconds and two decisions (pick folder, click album) from launch to music on a messy real-world library — plus all engine behavior covered by the test suite, benchmarks recorded as the baseline.

## Phase 4 and beyond (sketch, revisit after v0.1)

v0.2 correctness depth (ReplayGain, cues, watch folders, batch tagging, exclusive outputs) → v0.3 flow (bliss analysis, steered shuffle) → v0.4 context (enrichment pane, scrobbling, palette theming) → paid-parity extensions per the `research/06` hit-list. Order is re-derived from real usage after v0.1, not locked today.

## Standing rules while executing

- No feature lands before Phase 2's pipeline exists. No exceptions — this is the credibility mechanism.
- Spikes are deleted, not "cleaned up later."
- Every stack-level choice becomes a short ADR at the moment it's made.
