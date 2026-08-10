# baz

**A fast, open-source music player for people who own their music.**
foo, bar… baz — a spiritual successor to foobar2000: instant, correct, no
commercial agenda; with the beauty and convenience of the paid players (Roon,
Plexamp, Audirvana) and none of their clouds, accounts, or subscriptions.

> **Status: pre-alpha.** It scans a music folder, shows your albums and plays
> them gaplessly; it is not a finished player. **v0.1.0 is the first release**
> — see the [releases page](https://github.com/mattcree/baz/releases), or
> build it from source ([`docs/INSTALL.md`](docs/INSTALL.md)). What has
> actually landed is in
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

**Start typing.** Any letter, anywhere, filters the wall — there is no field to
click into first, and the search well fills in as you type so you can see what
you asked for. That is why every letter shortcut in the table below wears a
modifier: the letters belong to the query now.

| Key | Does |
|---|---|
| any printable character | filter the wall by it, from wherever you are — the search well takes the caret with the first keystroke |
| <kbd>Enter</kbd> | play the best match for what you typed; with no query, play the selected album |
| <kbd>Esc</kbd> | leave the search field, then peel one layer: the place you are in, else the settings, else the query, else the shuffle's marks |
| <kbd>Space</kbd> | play / pause |
| <kbd>←</kbd> <kbd>→</kbd> | seek 5 s back / forward |
| <kbd>Shift</kbd>+<kbd>←</kbd> <kbd>→</kbd> | seek 30 s back / forward |
| <kbd>Ctrl</kbd>+<kbd>→</kbd> | next track |
| <kbd>Ctrl</kbd>+<kbd>←</kbd> | previous track — or restart this one, if you are more than 3 s in |
| <kbd>↑</kbd> <kbd>↓</kbd> | volume up / down, one step (~1 dB at the top of the fader) |
| <kbd>Ctrl</kbd>+<kbd>M</kbd> | mute / unmute |
| <kbd>Ctrl</kbd>+<kbd>-</kbd> <kbd>Ctrl</kbd>+<kbd>=</kbd>, or <kbd>Ctrl</kbd>+scroll | hang the wall closer or wider — **spacious**, **balanced**, **dense** |
| <kbd>1</kbd> … <kbd>6</kbd> | arrange the wall by **A–Z**, **artist**, **year**, **genre**, **added** or **played** — the same six words in the top bar. **A–Z** breaks the wall into letter shelves; **artist** breaks the same order finer, one shelf per person, and pressing their name opens their page |
| <kbd>/</kbd>, or <kbd>Ctrl</kbd>+<kbd>F</kbd> | put the caret in the search field without typing anything |
| <kbd>Ctrl</kbd>+<kbd>U</kbd> | go to **Now playing** with the run beside it — what is playing, and what is **up next**, on one surface |
| <kbd>Ctrl</kbd>+<kbd>,</kbd> | go to the settings, or come back |
| <kbd>Ctrl</kbd>+<kbd>B</kbd> | hide the album inspector, or bring it back — the selection is kept |
| <kbd>Ctrl</kbd>+<kbd>R</kbd> | **the pull**: one record, weighted toward what you have not heard in a long time. Nothing plays — it is offered, and pressing again offers a different one |

**Shuffle** has no key, only the word in the top bar. The rule runs one way —
every action needs a visible control, not every control needs a key — and a
shuffle is a decision made once an evening, not a reflex.

Media keys (play/pause, previous, next, stop) work too — on Linux they usually arrive
over MPRIS rather than as key presses, which is the same thing by a different
road. The *volume* media keys are deliberately left alone: on every desktop
they mean the system's volume, and baz's fader is baz's own.

**The number row is the one place a bare key is not the query.** `1`–`6` pick
the arrangement and the rest of the row does nothing, because a row where `1`
rearranges the wall and `7` types a `7` would be two rules wearing one shape.
The cost, stated rather than discovered: you cannot start a search with a digit
from the wall. Press <kbd>/</kbd> first and every digit types, including the
first.

**While the search field has focus, every key belongs to the field**: Space
types a space, the arrows move the caret, `n` types an `n`. That is deliberate
and it is not a heuristic — baz asks the toolkit whether a widget consumed the
key and never second-guesses the answer. It is also why exactly one keystroke
per search reaches the shortcut table: the first one, which hands the caret to
the field, after which the field has them all. **Nothing has focus at startup**
— it used to be the search field, which cost <kbd>Space</kbd> its meaning until
you pressed <kbd>Esc</kbd> — because typing no longer needs a focused field.

**The wall's density is a gesture, not a setting.** There is no appearance
panel, no grid-size picker and no zoom slider: <kbd>Ctrl</kbd>+scroll on the
wall, or the two zoom keys, and baz remembers where you left it. Three named
steps rather than a free zoom, so that every screenshot of baz is one of three
walls and a layout bug is reproducible.

Every one of those keys goes on working on the now-playing surface. The run is
not a layer over anything — it is half of that place, drawn beside the record,
and the `Run` word in the place's top-right is what stands it down. <kbd>Esc</kbd>
leaves the place, as it leaves every place; it does not hide the run, because a
peel that removed half a place would make one key mean three things.

## Accessibility — read this before you install

**baz has no screen-reader support.** Not partial, not planned-for-soon: none.
The toolkit it is built on ([iced](https://iced.rs) 0.13) publishes no
accessibility tree, and its buttons take no keyboard focus. If you use a screen
reader, baz will not work for you today, and we would rather you learn that here
than after installing it.

That choice was made openly — ADR-0005 chose iced knowing it, with
"AccessKit-dependent accessibility" written into its accepted costs — and
[ADR-0017 §4](docs/adr/0017-design-direction.md) publishes it rather than
letting it be inherited quietly.

What baz does guarantee, because these are the guarantees still available and
being unable to make the big one is a reason to be strict about the rest:

- **Every action has a visible, pointer-reachable control.** No action is
  keyboard-only, and no control's only affordance is hover. This is a
  [standing refusal](docs/the product's standing rules), not an aspiration — it has already
  cost design proposals that were prettier without it.
- **Contrast floors are tested**, every ink against every surface, with opacity
  composited before measuring rather than assumed away.
- **Hit targets have a floor** and it is asserted in the test suite.
- **No state is signalled by colour alone.**

If AccessKit support lands in iced, baz's side of it is small — every control
is already a labelled, focusable-in-principle widget rather than a bare
positional mark. That was a deliberate constraint on the design, not luck.

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

**v0.1.0 is the first release**, on the
[releases page](https://github.com/mattcree/baz/releases). Building from source
is the other way, and it is one command plus (on Linux) one system package:

```sh
cargo build --release --locked -p baz --features device-output
./target/release/baz [MUSIC_DIR]
```

`device-output` is what makes sound come out; it is off by default because
building it needs the platform's audio headers (`libasound2-dev` /
`alsa-lib-devel` on Linux, nothing on macOS or Windows). Without it baz builds
and runs everywhere and hides the playback controls.

There are three ways in — **Flatpak** (the intended one on Linux, once the
Flathub submission lands), **signed-by-nobody release binaries** for Linux,
Windows and macOS, and source. All three, the checksum step, where baz keeps
its config and library, and the honest state of each platform:
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
