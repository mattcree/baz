# baz

**A fast, open-source music player for people who own their music.**
foo, bar… baz — a spiritual successor to foobar2000: instant, correct, no
commercial agenda; with the beauty and convenience of the paid players (Roon,
Plexamp, Audirvana) and none of their clouds, accounts, or subscriptions.

> **Status: pre-alpha.** The groundwork — research, architecture decisions, and
> the quality pipeline — is in place; the player itself is being built behind
> it. Nothing is usable yet. Follow `docs/NEXT-STEPS.md` for where things stand.

## What baz will be

- **The library is the interface.** Your album collection is the home screen —
  point baz at a directory, click an album, it plays front-to-back, gapless.
- **Instant.** Sub-second start; search-as-you-type across 100k+ tracks in
  microseconds (measured, not promised — see the spike results in git history).
- **Sovereign.** Offline-first, no account, no telemetry. Your files are the
  source of truth; baz never writes to them unbidden. Internet features
  (metadata, artwork, scrobbling) are individually opt-in.
- **Correct.** Gapless playback verified sample-for-sample, honest bit-perfect
  output modes, ReplayGain done right — the HydrogenAudio ethos, tested in CI.
- **Cross-platform.** Linux, Windows, macOS — Linux is first-class, not a port.

The full vision, competitive analysis, and staged scope live in
[`docs/VISION.md`](docs/VISION.md).

## Architecture

Rust workspace: `baz-core` is a headless engine (playback, library, protocol);
the GUI — [iced](https://iced.rs), chosen by measured spike, ADR-0005 — is a
thin client over its command/event protocol. Decisions are recorded in
[`docs/adr/`](docs/adr/).

## Building

```sh
cargo build --release   # workspace has no Linux system deps by design
```

Dev environment details (including the one-command Fedora toolbox):
[`docs/DEVELOPMENT.md`](docs/DEVELOPMENT.md).

## Development model — a note on AI

baz is developed with substantial AI assistance, openly: AI-assisted commits
carry co-author trailers. Trust is deliberately **not** placed in provenance —
it is placed in gates that anyone can inspect: every change passes rustfmt,
clippy with warnings denied, tests on three OSes, cargo-deny license and
advisory checks, coverage, scheduled fuzzing of every byte-facing parser, and
audio-correctness tests asserted against external references (reference
decoders, synthesized ground truth) — never against the code's own output. A
human owns every merge. The full charter:
[`docs/ENGINEERING.md`](docs/ENGINEERING.md).

## License

[GPL-3.0-or-later](LICENSE) (ADR-0001).
