# ADR-0006: UI layering — presentation must be trivially replaceable

**Status**: accepted (2026-08-07)

## Context

The owner's directive after the first design pass: the current look is "functional and good enough for now," but the *exact* interface will be redesigned later with serious UX expertise (possibly guided by vetted community design skills). The codebase must make a visual/layout shift cheap — changing the UI should be fairly trivial, never a rewrite.

## Decision

Three-layer contract inside `crates/baz`, on top of the already-UI-agnostic `baz-core` (ADR-0003):

1. **Pure state & logic (iced-free, unit-tested)**: `vm` (view models, album grouping/queueing), `player` (event-driven playback state machine), `shelf` (grid geometry math), `config`, `scan` batching, `art` resolution. These modules must never import iced types. A UI replacement reuses them wholesale.
2. **Design tokens (`theme`)**: every color, spacing, radius, type size, and widget style lives here; view code references tokens only. A reskin = editing one module. Hardcoded visual values in view code are a review-blocking defect.
3. **View composition**: iced-specific `view` functions and the `update` loop. This is the *only* disposable layer — a layout shift rewrites these functions and nothing else. View functions stay small, named per surface (`tile`, `side_panel`, `bottom_bar`, …), and may compute derived display strings but never mutate state.

Enforcement: layer-1 modules keep zero iced imports (checkable by grep/review); new visual constants go to `theme` or don't merge; behavior changes and visual changes land as separate commits whenever practical.

## Consequences

- The first design pass already conforms (verified 2026-08-07: `app.rs` holds all iced composition; `vm`/`player`/`shelf` are pure and tested; `theme` holds all tokens). When `app.rs` grows past comfort, views split into a `views/` module tree — mandated at next substantial UI change, not as churn now.
- A future UX-expert redesign — or even a second front end (ADR-0003's server/remote story) — costs layer 3 only.
- Community design skills used for future passes are treated like dependencies: specific, provenance-checked, owner-approved before installation.
