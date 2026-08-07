# ADR-0001: License — GPL-3.0-or-later

**Status**: accepted (2026-08-07) · revisable until first public release

## Context

baz is an open-source, community-oriented music player. The license question is between GPL-3.0-or-later (copyleft; fooyin, Navidrome, Strawberry, Tauon precedent) and MPL-2.0 (file-level copyleft, friendlier to embedding `baz-core` in other software).

## Decision

**GPL-3.0-or-later** for the application and all crates in the workspace.

Rationale:

- The project's positioning is explicitly "a free alternative to paid, closed products, minus the vendor cloud" (see `docs/research/06-paid-product-teardown.md`). Copyleft protects that positioning: nobody repackages baz as a proprietary paid player.
- Every successful open-source peer in this exact niche chose GPL-3 (fooyin, Strawberry, Tauon, Navidrome, Feishin). Contributors in this space expect it.
- The embedding argument for MPL is weak here: `baz-core` is a player engine, not a general-purpose library; plugin/extension boundaries can be given a linking exception later if ever needed.
- While the project has a single copyright holder, relicensing remains trivially possible before external contributions arrive; this decision hardens with the first outside PR.

## Consequences

- Dependency license compatibility enforced by `cargo-deny` allowlist (GPL-3-compatible only: MIT/Apache-2.0/BSD/MPL-2.0/zlib etc.).
- BASS-style proprietary audio libraries are doubly excluded (already rejected on principle in `docs/research/04-tech-stack.md`).
- Contributor policy (DCO vs CLA) deferred to Phase 2 scaffolding; default inclination is DCO (no CLA), matching the project's trust-through-openness stance.
