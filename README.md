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

## Keyboard

| Key | Does |
|---|---|
| <kbd>Space</kbd> | play / pause |
| <kbd>←</kbd> <kbd>→</kbd> | seek 5 s back / forward |
| <kbd>Shift</kbd>+<kbd>←</kbd> <kbd>→</kbd> | seek 30 s back / forward |
| <kbd>N</kbd>, or <kbd>Ctrl</kbd>+<kbd>→</kbd> | next track |
| <kbd>/</kbd>, or <kbd>Ctrl</kbd>+<kbd>F</kbd> | focus the search field |
| <kbd>Esc</kbd> | clear the search, else close the album panel |

Media keys (play/pause, next, stop) work too — on Linux they usually arrive
over MPRIS rather than as key presses, which is the same thing by a different
road.

**While the search field has focus, every key belongs to the field**: Space
types a space, the arrows move the caret, `N` types an `N`. That is deliberate
and it is not a heuristic — baz asks the toolkit whether a widget consumed the
key and never second-guesses the answer. The search field takes focus at
startup, so the first <kbd>Esc</kbd> hands the keyboard back to the transport.

## Desktop integration (Linux)

baz implements [MPRIS2], so GNOME's and KDE's media controls, the lock screen,
`playerctl`, and hardware media keys all drive it, showing the current track's
title, artist, album and cover. What they show comes only from what the
playback engine confirmed — the position is baz's real knowledge, not a clock
run alongside it.

It is an enhancement and never a requirement: with no D-Bus session bus, baz
prints one line and runs exactly as before. Packagers should install
[`packaging/baz.desktop`](packaging/README.md).

[MPRIS2]: https://specifications.freedesktop.org/mpris-spec/latest/

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
