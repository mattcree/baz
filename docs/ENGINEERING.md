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
- **Unsafe**: `#![forbid(unsafe_code)]` in every crate except the platform audio backends (`wasapi`/CoreAudio/ALSA FFI) and any realtime primitives. Each `unsafe` block carries a `// SAFETY:` comment stating the invariant. Unsafe-bearing crates get Miri (where runnable) and extra review.
- **Errors**: `thiserror` for library errors, no `unwrap`/`expect` in library code (clippy-enforced); panics are a bug except in tests.
- **Public API**: rustdoc on everything public; `#[deny(missing_docs)]` on `baz-core`; broken intra-doc links fail CI.
- **Dependencies**: minimal and reviewed. `cargo-deny` enforces a license allowlist, bans duplicate major versions where avoidable, and fails on RUSTSEC advisories. A new dependency is a reviewed decision, not a reflex.

## Testing

- **Unit + integration tests** for all of `baz-core`; the GUI layer keeps logic thin enough that core tests carry the weight.
- **Golden-file audio tests**: decode known inputs and compare output hashes against reference decoders (e.g. `flac -d`, ffmpeg) — bit-exactness is asserted, not assumed.
- **Gapless boundary tests**: synthesized signals (continuous sine split across two files) played through the engine; assert sample-level continuity — no gap, no overlap, no discontinuity — including across sample-rate changes.
- **Loudness/ReplayGain**: validated against reference implementations (EBU R128 test vectors).
- **Property-based tests** (`proptest`) for parsers, tag handling, and library queries.
- **Fuzzing** (`cargo-fuzz`): every parser that touches file bytes (tags, cues, playlists, decoder wrappers) has a fuzz target; fuzzing runs on a CI schedule, not just ad hoc. Media parsers process hostile input; we treat them accordingly.
- **Benchmarks** (`criterion`): the hot paths — search latency, scan throughput, decode throughput — with results tracked over time; PRs touching them get a comparison, and regressions need a stated reason.
- **Coverage** (`cargo-llvm-cov`): measured and reported on every PR. Coverage is a lens, not a target — but unexplained drops block merge.

## CI pipeline

Every PR runs, on a **Linux + macOS + Windows matrix**:

1. `rustfmt --check`
2. `clippy -D warnings` (all targets, all features)
3. `cargo test` (unit + integration)
4. `cargo doc` with warnings denied
5. `cargo-deny check` (licenses, advisories, bans)
6. MSRV build check
7. Coverage report
8. Criterion benchmark comparison when core paths are touched

Scheduled jobs: fuzzing corpus runs, `cargo-audit`, dependency-update checks. Releases (later) are built, signed, and reproducible-where-possible from CI only — no artifacts from developer machines.

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
