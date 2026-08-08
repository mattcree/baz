# baz

**A fast, open-source music player for people who own their music.**
foo, bar… baz — a spiritual successor to foobar2000: instant, correct, no
commercial agenda; with the beauty and convenience of the paid players (Roon,
Plexamp, Audirvana) and none of their clouds, accounts, or subscriptions.

> **Status: pre-alpha, and nothing has been released.** It scans a music
> folder, shows your albums and plays them gaplessly; it is not a finished
> player, and there is nothing to download yet — build it from source
> ([`docs/INSTALL.md`](docs/INSTALL.md)). What has actually landed is in
> [`CHANGELOG.md`](CHANGELOG.md); what is deliberately deferred is in
> [`docs/BACKLOG.md`](docs/BACKLOG.md); where things stand is in
> `docs/NEXT-STEPS.md`.

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
| <kbd>Ctrl</kbd>+<kbd>←</kbd> | previous track — or restart this one, if you are more than 3 s in |
| <kbd>↑</kbd> <kbd>↓</kbd> | volume up / down, one step (~1 dB at the top of the fader) |
| <kbd>M</kbd> | mute / unmute |
| <kbd>/</kbd>, or <kbd>Ctrl</kbd>+<kbd>F</kbd> | focus the search field |
| <kbd>Q</kbd> | show / hide **Up next** — the queue, over the now-playing bar |
| <kbd>Ctrl</kbd>+<kbd>,</kbd> | go to the settings, or come back |
| <kbd>Ctrl</kbd>+<kbd>B</kbd> | hide the album inspector, or bring it back — the selection is kept |
| <kbd>Esc</kbd> | peel one layer: **Up next**, else the settings, else the search, else the inspector |

Media keys (play/pause, previous, next, stop) work too — on Linux they usually arrive
over MPRIS rather than as key presses, which is the same thing by a different
road. The *volume* media keys are deliberately left alone: on every desktop
they mean the system's volume, and baz's fader is baz's own.

**While the search field has focus, every key belongs to the field**: Space
types a space, the arrows move the caret, `N` types an `N`. That is deliberate
and it is not a heuristic — baz asks the toolkit whether a widget consumed the
key and never second-guesses the answer. The search field takes focus at
startup, so the first <kbd>Esc</kbd> hands the keyboard back to the transport.

Every one of those keys goes on working while **Up next** is open. The popover
is deliberately not modal — iced 0.13 offers no focus containment and no
accessibility tree, so imitating a modal would be a claim the toolkit cannot
back — and <kbd>Esc</kbd>, a second <kbd>Q</kbd>, or a press anywhere outside
it all put it away.

## Desktop integration (Linux)

baz implements [MPRIS2], so GNOME's and KDE's media controls, the lock screen,
`playerctl`, and hardware media keys all drive it, showing the current track's
title, artist, album and cover. Volume is readable and settable from there
too, mapped through the same fader curve the on-screen control uses so the
two cannot disagree. What they show comes only from what the playback engine
confirmed — the position is baz's real knowledge, not a clock run alongside
it.

It is an enhancement and never a requirement: with no D-Bus session bus, baz
prints one line and runs exactly as before. Packagers should install
[`packaging/io.github.mattcree.baz.desktop`](packaging/README.md).

[MPRIS2]: https://specifications.freedesktop.org/mpris-spec/latest/

## Architecture

Rust workspace: `baz-core` is a headless engine (playback, library, protocol);
the GUI — [iced](https://iced.rs), chosen by measured spike, ADR-0005 — is a
thin client over its command/event protocol. Decisions are recorded in
[`docs/adr/`](docs/adr/).

## Installing

**There is no released version yet** — nothing has been tagged, so the releases
page is empty. Building from source is the only way to run baz today, and it is
one command plus (on Linux) one system package:

```sh
cargo build --release --locked -p baz --features device-output
./target/release/baz [MUSIC_DIR]
```

`device-output` is what makes sound come out; it is off by default because
building it needs the platform's audio headers (`libasound2-dev` /
`alsa-lib-devel` on Linux, nothing on macOS or Windows). Without it baz builds
and runs everywhere and hides the playback controls.

When releases do exist there will be three ways in — **Flatpak** (the intended
one on Linux), **signed-by-nobody release binaries** for Linux, Windows and
macOS, and source. All three, the checksum step, where baz keeps its config and
library, and the honest state of each platform:
[`docs/INSTALL.md`](docs/INSTALL.md).

The short version of "honest state": baz is pre-alpha, Linux is the platform it
is developed and used on, and the Windows and macOS binaries are built and
tested by CI on every change but have never been used by a human.

Dev environment details (including the one-command Fedora toolbox):
[`docs/DEVELOPMENT.md`](docs/DEVELOPMENT.md). How a release is cut, and what
"reproducible" does and does not mean here:
[`docs/RELEASING.md`](docs/RELEASING.md).

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
