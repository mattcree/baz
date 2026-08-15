# The long version

The README is written for someone deciding whether to install a music player.
This page is the same feature set with the engineering shown — the claims baz
makes about audio, and how each one is checked. It exists so the README can
stay short without any of this being lost.

Everything here is current behaviour, not intent.

## Playback

**Gapless, and it is measured rather than claimed.** WAV, FLAC and ALAC cross a
track boundary **bit-exact** — the test suite asserts the joined decode
sample-for-sample against the continuous one. MP3 is trimmed from its LAME
header and Ogg Vorbis from its granule positions, both to a bounded error the
tests pin. A track boundary in baz is bookkeeping, not an audio event: the next
track is already decoded and waiting in a lock-free ring.

**The output follows the music.** baz opens the device at the rate of what you
are playing and resamples nothing; a track at a different rate reopens the
device rather than being converted to fit. The one case where it *must*
convert — hardware with no mode for the material — is reported on screen
instead of done quietly. The bottom bar's `bit-perfect` readout means three
things at once and all of them are checked: baz converted nothing, the fader is
at unity, and the file was not folded down from more than two channels. On
Linux an optional `exclusive-output` build takes the ALSA device outright,
`hw:` only, with resampling switched off in the driver.

**Multichannel folds to stereo** — 3.0, 4.0, 5.0 and 5.1, by the ITU-R BS.775
matrix, with the speaker layout *read from the container* rather than guessed
from the channel count (Vorbis and WAVE order 5.1 differently, and getting that
wrong puts the centre channel in one ear). 7.1 and 6.1 are still refused, and
say which layout they found.

**ReplayGain, both halves.** baz honours the `REPLAYGAIN_*` and R128 tags
foobar2000, rsgain and loudgain wrote, and it can measure your library itself:
a full EBU R128 / ITU-R BS.1770-4 pass, gated integrated loudness and true
peak, verified against the EBU Tech 3341 compliance signals. It is per album
edition and resumable, it skips what is already known, and it stores what it
finds in its own index rather than in your files. Off by default, because
`Off` means no gain arithmetic at all.

## Library

**Your folders, unchanged.** Name as many as you like, including a mounted
NAS. baz reads them and never writes to them: no tags rewritten, no files
moved, no `.baz` directories left behind. A rescan is incremental — one `stat`
per unchanged file, so a 10 000-track library re-checks in about 10 ms — and a
share that is not mounted is reported, not pruned. Nothing leaves the index
until four separate checks agree the folder is really gone.

**A wall you can arrange.** Six arrangements — A–Z, artist, year, genre, added,
played — and four densities, changed with <kbd>Ctrl</kbd>+scroll or the four
marks at the top right. One click selects and highlights a record; double-click
plays it. The same selection-first rule applies to playlists and track rows,
while labelled Play controls act immediately. baz remembers where you left it.

**Search as you type, for tracks and records.** No field to click into first:
any letter, anywhere, opens the app-bar chooser over the place you are already
using. Ranked Tracks and Albums share one scroll surface. Arrow keys select a
result and choose `Play` or `Enqueue`; Enter confirms, while Esc or a click
outside clears the chooser and returns to the unchanged place.

**Playlists are files you own.** One UTF-8 `.m3u8` per list, in baz's own data
folder, written as plain text any other player can read. Edit one in a text
editor and baz notices; the reader never writes back, so a line baz did not
understand is still there afterwards, byte for byte. Undo is eight deep per
list, and `Delete` sends the file to the desktop's trash — never `unlink` — so
the recovery is your file manager's own `Restore`. Deleting a list never
touches the music.

**Playlists by feel.** baz can listen to your library itself — locally, with no
network — and build a list that follows a shape you draw: start quiet, climb,
hold, come down. The analysis runs on your machine, the models ship with the
optional `vibe` build, and hovering a row in the result shows where that track
landed on your line. `docs/design/17-contour.md` is the design; ADR-0034 is the
decision.

## The desktop

**Full MPRIS2** — GNOME's and KDE's media controls, the lock screen,
`playerctl` and the hardware media keys all drive baz and show the title,
artist, album and cover. Volume is readable and settable from there, through
the same fader curve the on-screen control uses, and the position it reports is
the engine's own knowledge rather than a clock running alongside it. With no
session bus baz prints one line and runs exactly as before.

## Keyboard, in full

**Start typing.** Any letter, anywhere, opens the app-wide Tracks and Albums
chooser over the place you are on — there is no field to click into first, and
the app-bar search well fills in as you type. Up/Down choose a result;
Left/Right choose a track's Play or Enqueue action; Enter confirms. That is why
every letter shortcut wears a modifier: the letters belong to the query now.
Nothing has focus when baz starts, so <kbd>Space</kbd> means play/pause on the
very first frame. A successful album Play carries you to Now Playing only after
the audio engine confirms that a track began.

| Key | Does |
|---|---|
| any printable character | open and filter the app-wide Tracks and Albums chooser; the search well takes the caret with the first keystroke |
| <kbd>Enter</kbd> | confirm the selected search result; with no query, activate the selected album, playlist or track |
| <kbd>Esc</kbd> | peel one layer, top down: a drag in flight, then a menu, then the playlists panel, then a field, then the place you are in, then the query |
| <kbd>F11</kbd> | fill the window's current monitor, or return to the previous window size |
| <kbd>Space</kbd> | play / pause |
| <kbd>←</kbd> <kbd>→</kbd> | seek 5 s back / forward |
| <kbd>Shift</kbd>+<kbd>←</kbd> <kbd>→</kbd> | seek 30 s back / forward |
| <kbd>Ctrl</kbd>+<kbd>→</kbd> | next track |
| <kbd>Ctrl</kbd>+<kbd>←</kbd> | previous track — or restart this one, if you are more than 3 s in |
| <kbd>↑</kbd> <kbd>↓</kbd> | volume up / down, one step (~1 dB at the top of the fader) |
| <kbd>Ctrl</kbd>+<kbd>M</kbd> | mute / unmute |
| <kbd>Ctrl</kbd>+<kbd>-</kbd> <kbd>Ctrl</kbd>+<kbd>=</kbd>, or <kbd>Ctrl</kbd>+scroll | hang the wall closer or wider — **spacious**, **balanced**, **compact**, **dense** |
| <kbd>1</kbd> … <kbd>6</kbd> | arrange the wall by **A–Z**, **artist**, **year**, **genre**, **added** or **played** |
| <kbd>/</kbd>, or <kbd>Ctrl</kbd>+<kbd>F</kbd> | put the caret in the search field without typing anything |
| <kbd>Ctrl</kbd>+<kbd>U</kbd> | go to **Now playing** — what is playing, and what is **up next**, on one surface |
| <kbd>Ctrl</kbd>+<kbd>P</kbd> | open the playlists panel, or close it |
| <kbd>Ctrl</kbd>+<kbd>B</kbd> | collapse the left-hand lane, or bring it back |
| <kbd>Ctrl</kbd>+<kbd>,</kbd> | go to the settings, or come back |
| <kbd>Ctrl</kbd>+<kbd>Z</kbd> | undo the last edit to the list you are looking at (eight deep, no redo) |

Media keys — play/pause, previous, next, stop — work too. On Linux they usually
arrive over MPRIS rather than as key presses, which is the same thing by a
different road. The *volume* media keys are deliberately left alone: on every
desktop they mean the system's volume, and baz's fader is baz's own. Wheel or
trackpad travel directly over that fader adjusts baz's volume in the same
bounded steps as Up/Down without scrolling the page underneath it.

**Shuffle has no key**, only the control in the bottom bar. The rule runs one
way — every action needs a visible control, not every control needs a key — and
shuffle is a standing choice made once an evening rather than a reflex.

**The number row is the one place a bare key is not the query.** `1`–`6` pick
the arrangement and the rest of the row does nothing, because a row where `1`
rearranges the wall and `7` types a `7` would be two rules wearing one shape.
The cost, stated rather than discovered: you cannot start a search with a digit
from the wall. Press <kbd>/</kbd> first and every digit types.

**While the search field has focus, every key belongs to the field**: Space
types a space, the arrows move the caret, `n` types an `n`. That is not a
heuristic — baz asks the toolkit whether a widget consumed the key and never
second-guesses the answer — which is also why exactly one keystroke per search
reaches the table above: the first one, which hands the caret to the field.

## The rest of the rough edges

The README's [Known limitations](../README.md#known-limitations) carries the
ones a listener meets. These are the rest, kept because they are real.

- **AAC has no gapless trim.** MP3, Vorbis, FLAC, WAV and ALAC are all trimmed;
  AAC in MP4 is not, because symphonia's MP4 reader consults neither the edit
  list nor `iTunSMPB`. An AAC album carries about 23 ms of encoder priming at
  each track boundary.
- **Multichannel AAC does not decode at all** — upstream refuses the stream
  before a frame exists. 7.1 and 6.1 are refused in any format.
- **A folded 5.1 file plays 7.66 dB quieter than a stereo master** until you
  analyse it. That is the headroom the downmix matrix needs to be provably
  clip-free; a ReplayGain pass measures this decoder's own output and gives the
  level back exactly.
- **FLAC inside an MP4 container is labelled ALAC.** The wrong name and the
  right fidelity tier, on a combination essentially absent from real libraries.
- **Skip and seek are drain-and-restart, not sample-accurate splices.** Tens of
  milliseconds, not an audible gap in ordinary use.
- **Seeking into an Ogg Vorbis file loses one lapped block** — 1024 frames,
  23.2 ms at 44.1 kHz. Every other format seeks exactly or time-accurately.
- **Exclusive output needs the sound card to itself**, and on a desktop
  something usually has it. PipeWire held the maintainer's DAC for a whole
  session; every exclusive open refused with `DeviceBusy`. baz reports that and
  does not offer to fix it.
- **A file that needs converting is decoded whole before the first sound** when
  the device offers no mode at the source rate — about 2.6 s for a five-minute
  24/48 FLAC.
- **A file baz could not read names itself only in the terminal.** A scan
  prints one `[scan] skipped <path>: <reason>` line per failure, which is
  enough at a shell and nothing at all inside the Flatpak.
- **The wall's thumbnail cache decodes one size for several densities**, so the
  tightest steps hold covers larger than they draw.

**A defect that is not ours, and is not guarded.** A corrupt or hostile audio
file can make the decoder **reserve several gigabytes** before it reads a
byte — four such sites across FLAC, Ogg and WAV, and five more in MP4, all of
them unchecked 32-bit lengths in symphonia. On 64-bit Linux this costs nothing
measurable: the reservation is a lazy mapping, the read fails, peak memory
stays around 3 MB. **On macOS the pages are actually found**, and three test
inputs cost five seconds. Under a container limit, strict overcommit or a
32-bit build it is an abort.

baz declines to guard it, and the reason is not effort: a correct pre-check is
a second parser for four container formats, and the day baz's copy and
symphonia's original disagree about where a block ends, baz refuses a file that
plays. Real WAV files declare `0xFFFFFFFF` routinely. For a music player,
refusing a good record is the worse failure. The full argument, the reproducers
and the upstream fix that would close it are in
[ADR-0040](adr/0040-a-hostile-file.md).

## How it is built

A Rust workspace: `baz-core` is a headless engine — playback, library,
protocol — and the GUI is a thin client over its command/event protocol, so
nothing on screen can make a sample late. The toolkit is
[iced](https://iced.rs), chosen by a measured spike (ADR-0005). Every decision
of consequence is written down in [`adr/`](adr/), including the ones that went
the other way.

baz is developed with substantial AI assistance, openly: AI-assisted commits
carry co-author trailers. Trust is deliberately **not** placed in provenance —
it is placed in gates anyone can inspect. Every change passes rustfmt, clippy
with warnings denied, the whole test suite on three operating systems,
cargo-deny licence and advisory checks, and scheduled fuzzing of every
byte-facing parser; the audio-correctness tests are asserted against external
references — reference decoders, synthesized ground truth, the EBU's own
compliance signals — and never against baz's own output. A human owns every
merge. The charter is [`ENGINEERING.md`](ENGINEERING.md).
