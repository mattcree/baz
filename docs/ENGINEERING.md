# baz — Engineering Standards

> The quality bar for all code in this project. Written before the first line of code, deliberately. Companion to `VISION.md`; the concrete plan is in `NEXT-STEPS.md`.

## Principles

1. **Correctness over features.** A small player that is verifiably right beats a large one that is probably right. The audio path especially: bit-exactness, gapless continuity, and sample-rate handling are testable claims, and we test them.
2. **The audio thread is sacred.** No allocation, no locking, no I/O, no panics on the realtime path. Everything crossing into it goes through wait-free structures (ring buffers, atomics). This is enforced in review and, where possible, by construction (types that don't implement the tempting shortcuts).
3. **Boring reliability.** Prefer proven crates, stable toolchains, and obvious code. Cleverness needs a benchmark or a test that justifies it. HydrogenAudio's ethos — claims require evidence — applies to our engineering as much as our audio.
4. **Trust is earned by gates, not assurances.** See "AI involvement" below.

## Rust standards

- **Toolchain**: pinned stable via `rust-toolchain.toml`; MSRV declared and CI-checked.
- **Formatting**: `rustfmt` (default config), enforced in CI. No debates.
- **Linting**: `clippy` with `-D warnings`; `pedantic` and `nursery` audited and enabled per-lint (allowlist documented in `Cargo.toml`), not blanket-enabled.
- **Unsafe**: `unsafe_code = "deny"` workspace-wide, and `#![forbid(unsafe_code)]` in `baz-core`, where the audio path lives; the ALSA exclusive backend is safe Rust over the `alsa` crate. The two `unsafe` sites in the workspace are not audio at all — `env::set_var` before any thread exists in `baz`'s `main`, and `malloc_trim` in `baz-vibe` — and each is a named `#[expect]` with a `// SAFETY:` comment stating the invariant. No Miri job exists; if a third site appears, one should.
- **Errors**: `thiserror` for library errors; no `unwrap`/`expect` in library code — `clippy::unwrap_used` and `clippy::expect_used` are enabled and CI denies warnings, and the handful of `expect`s whose invariant is real carry `#[expect(clippy::expect_used, reason = ..)]` naming it. Tests are exempt. Panics are a bug except in tests.
- **Public API**: rustdoc on everything public; `missing_docs` warns workspace-wide and CI turns every warning into an error; broken intra-doc links fail CI.
- **Dependencies**: minimal and reviewed. `cargo-deny` enforces a license allowlist and fails on RUSTSEC advisories (three standing, argued exemptions are listed in `deny.toml`); duplicate versions are reported at warning level, not gated, because the GUI toolkit's graph carries them. A new dependency is a reviewed decision, not a reflex.

## Testing

- **Unit + integration tests** for all of `baz-core`; the GUI layer keeps logic thin enough that core tests carry the weight.
- **Reference-encoder audio tests**: synthesized ground truth (a known sine) is encoded by the reference encoders — `flac`, and ffmpeg's LAME, Vorbis, Opus, AAC and ALAC — and baz's decode is compared sample by sample against the signal that went in. Bit-exactness is asserted for the lossless codecs; the lossy tolerances are derived from the encoder's own measured error against the ideal signal and written down beside the test. CI installs the encoders on all three runners; a fixture a given ffmpeg cannot produce (HE-AAC without a full `libfdk_aac`) skips its one test, loudly.
- **Gapless boundary tests**: synthesized signals (continuous sine split across two files) played through the engine; assert sample-level continuity — no gap, no overlap, no discontinuity — per codec. A boundary that also changes sample rate is covered by the rate-change tests, which assert wall-clock-true elapsed time rather than sample continuity.
- **Loudness/ReplayGain**: validated against the EBU Tech 3341 compliance signals, generated from the specification's own description at the specification's own tolerance.
- **Fuzzing** (`cargo-fuzz`): every byte-facing parser in `baz-core` — play-history lines, M3U playlists, ReplayGain tags, filename inference, the command protocol, and the decoder wrapper — has a fuzz target with a committed seed corpus; the six run weekly on a schedule and on demand. The interface's theme-JSON and `config.toml` readers do not yet have targets. Media parsers process hostile input; the inputs fuzzing has found live in `tests/hostile_media.rs`, which every gate runs.
- **Benchmarks** (`criterion`): scan throughput and search latency have benches in `baz-core`. They are run by hand; CI does not yet compare them, so a regression needs a developer to notice.
- **Coverage** (`cargo-llvm-cov`): an 80 % line floor gates `baz-core` on every push; the workspace-wide figure is reported as an artifact, not gated, because GUI code needs a display to execute.

## CI pipeline

Every push and PR runs:

1. `rustfmt --check`
2. `clippy -D warnings` (all targets, all features), plus a `--no-default-features` check that the player-only build still compiles
3. `cargo test` (unit + integration), on a **Linux + macOS + Windows matrix**, with the reference encoders installed
4. `cargo doc` with warnings denied
5. `cargo-deny check` (licenses, advisories)
6. MSRV build check
7. Coverage, with the `baz-core` floor gated
8. Packaging metadata checks (desktop file, AppStream, Flatpak manifest pin)

Steps 1, 2 and 4–8 run on Linux only; the test matrix is where the three platforms are exercised. The weekly schedule runs the six fuzz targets. Releases are built from CI only, with this whole suite as a hard dependency — no artifacts from developer machines.

**The pipeline is installed before the first feature lands.** A green, meaningful CI on an empty workspace is milestone zero.

## Decisions and documentation

- **ADRs** (Architecture Decision Records) in `docs/adr/`, numbered, short: context, decision, consequences. The stack choices already made in `VISION.md` become the first ADRs when ratified.
- **CHANGELOG** kept from the first tagged version.
- Commit messages explain *why*; PRs are small and single-purpose.

## AI involvement — the trust policy

This project is developed with substantial AI assistance, openly. The fear to dispel is that AI involvement means unreviewed, plausible-looking slop. The answer is structural, not rhetorical:

1. **Provenance is disclosed, not hidden.** AI-assisted commits carry their co-author trailers. The README states the development model plainly.
2. **Provenance is also irrelevant to merge.** No code — human- or AI-written — merges without passing the full gate set above. The gates are designed so that "who wrote it" doesn't need to be part of the trust calculation. That is the point of having them.
3. **Tests are written to specification, not to implementation.** Audio-correctness tests assert against external references (reference decoders, EBU vectors, synthesized ground truth) — never against the code's own output recorded as truth.
4. **A human owns the trunk.** Pre-1.0 development is trunk-based at the maintainer's direction: work lands on `main` gated by the full CI suite, and the maintainer reviews the trunk continuously rather than per-merge — a red `main` is an all-stop until green. External contributions go through PRs with real review. In either mode, "the model said so" is never a rationale; benchmarks, tests, and ADRs are.
5. **No velocity alibi.** AI assistance raises the floor on how much rigor is affordable (more tests, more fuzzing, more docs), and that is what it will be spent on — not on shipping faster than the review bandwidth can honestly cover.

A skeptic should be able to clone the repository, read the CI config and the test suite, and conclude the quality bar is enforced by machinery they can inspect — without taking anyone's word for anything.
