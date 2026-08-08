# The ambient continuation — rendered evidence

Pixel evidence for the bar's third line: *what the queue holds after this
track*, stated without being asked. It completes the line
[`../../critique/02-surfaces.md`](../../critique/02-surfaces.md) specifies for
this corner — *"Wall label bottom-left … **+ stack status when queued** ('then 2
sleeves · 1h 58m left')"* — of which only the wall label had shipped.

Every image is the real binary, captured per
[`docs/DEVELOPMENT.md`](../../../DEVELOPMENT.md#headless-ui-verification), and
nothing was touched that this work did not start:

- a private `Xvfb :171` at 1400×1000, `env -u WAYLAND_DISPLAY
  -u DBUS_SESSION_BUS_ADDRESS`, `WINIT_UNIX_BACKEND=x11`; no window manager, so
  the window is exactly the 1280 × 860 it asks for;
- **all six** redirections — scratch `HOME`, `XDG_DATA_HOME`, `XDG_CONFIG_HOME`,
  `XDG_CACHE_HOME`, `XDG_RUNTIME_DIR`, and no session-bus address — so the
  maintainer's library, config, thumbnails and session bus were never opened.
  The receipt, from this run's log:

  ```
  [mpris] no session bus; desktop media controls unavailable (I/O error: No such file or directory (os error 2))
  ```

- a throwaway 3-album / 26-track fixture of **digitally silent** WAV (every
  sample a zero), never `~/Music`; the build carries `device-output` so the
  transport is real, and the scratch `HOME` carries an `.asoundrc` routing
  ALSA's default PCM to `null`. `BAZ_DEVICE_TESTS` was never set;
- captures targeted at *this* process's window by pid, never "the active
  window", with the pointer parked off every control so no hover state is in a
  frame.

The fixture is untagged on purpose, so path inference decides what is an album:
`Bellwether Quartet/Long Division/*.wav` is a **record** (20 tracks), and three
files sitting at the scan root are **loose songs** — the two shapes the
continuation has to tell apart.

> The null sink accepts writes as fast as they arrive, so playback free-wheels
> at roughly **700 × real time**: one second of wall clock is eleven minutes of
> music. "Play this row" and "pause" therefore go in a single `xdotool`
> invocation, tens of milliseconds apart, and every playing frame here is a
> paused state rather than a blur.

## The frames

All crops are the same 1280 × 104 region of the window (`+0+756`).

| Image | The line, and what it is saying |
|---|---|
| [`bar-01-nothing-playing.png`](bar-01-nothing-playing.png) | **Nothing playing.** `Nothing playing`, and the **Queue** control with an empty readout: no queue, so no count — never `0`. |
| [`bar-02-more-of-this-record.png`](bar-02-more-of-this-record.png) | **More of the record now playing.** `Opening Statement` / `Bellwether Quartet` / **`then 19 more · 57:38 left`**, and `Queue 20`. The rest of a record you are already inside is counted, not named: its title is not on the bar to repeat, and a second title under the one sounding would be two titles with one of them not playing. |
| [`bar-03-popover-agrees.png`](bar-03-popover-agrees.png) · [`full-03-popover-agrees.png`](full-03-popover-agrees.png) | **One computation, two surfaces.** The popover's summary reads `1 of 20 · 57:38 left`; the bar under it reads `then 19 more · 57:38 left`. Same figure, because it is the same function (`player::left_note`) — and 1 + 19 = 20. |
| [`bar-04-loose-songs-counted.png`](bar-04-loose-songs-counted.png) | **Several loose songs.** `Battery` / **`then 2 tracks · 7:05 left`**, `Queue 3`. Songs queued one by one are counted as tracks, never as an album. |
| [`bar-05-one-loose-song-named.png`](bar-05-one-loose-song-named.png) | **One thing coming, so it is named.** `Cell` / **`then Terminal · 4:08 left`**. `then 1 track` would be the interface refusing to say the one word it knows. |
| [`bar-06-last-track-says-nothing.png`](bar-06-last-track-says-nothing.png) | **The last track says nothing at all.** `Terminal`, and the lane below it is empty. Not `up next: nothing`, not `end of queue` — [`docs/REFUSALS.md`](../../../REFUSALS.md) makes the silence after a queue a feature, and announcing it would be the announcement rather than the silence. |

### What is not here, and why

`then Kid A` and `then 2 albums · 1:58:00 left` — a *second record* stacked
behind the first — cannot be photographed yet, because no gesture in the
shipped app can put two records in one queue: shift-click-to-stack is step 13
of [ADR-0017](../../../adr/0017-design-direction.md#7-the-build-plan) §7 and the
queue is still one record or one handful of loose songs. Those wordings are
built, and they are proven by the unit tests in `player.rs`
(`one_record_behind_this_one_is_named`,
`several_records_are_counted_as_records_with_the_time_left`,
`a_mixture_names_both_kinds`, `a_record_stacked_twice_is_two_entries`) rather
than by a frame. Stated here rather than implied by a gap in the table.

## Pixel stability, measured

The left zone is the bar's most contested strip and the continuation comes and
goes with the music — it is drawn for every track of a queue but the last. Three
measurements over the six frames above, taken from the pixels rather than from
the layout code.

**1. The bar's top edge and the transport buttons do not move.** The hairline
that starts the bar, and the bounding boxes of the three transport plinths:

```
frame                            bar top   transport buttons (x0,x1,y0,y1)
01-nothing-playing                   758   (584,615,771,802) (624,655,771,802) (664,695,771,802)
02-more-of-this-record               758   (584,615,771,802) (624,655,771,802) (664,695,771,802)
03-popover-agrees                    758   (584,615,771,802) (624,655,771,802) (664,695,771,802)
04-loose-songs-counted               758   (584,615,771,802) (624,655,771,802) (664,695,771,802)
05-one-loose-song-named              758   (584,615,771,802) (624,655,771,802) (664,695,771,802)
06-last-track-says-nothing           758   (584,615,771,802) (624,655,771,802) (664,695,771,802)
```

Identical in all six. The bar is 102 px in every state, and the transport is on
the same pixels whether the continuation is absent, one word long, or a count
and a clock.

**2. Differing pixels, whole bar and transport band.** The band is x 552–728 of
the crop: the three buttons and the air around them.

```
pair                                                        whole bar   transport band
01-nothing-playing      vs 02-more-of-this-record                3403              244
02-more-of-this-record  vs 06-last-track-says-nothing            2356                0
05-one-loose-song-named vs 06-last-track-says-nothing            1387                0
02-more-of-this-record  vs 03-popover-agrees                     3549                0
04-loose-songs-counted  vs 05-one-loose-song-named               1291                0
```

**Zero** in the transport band for every pair of playing states — including the
one that matters most, *continuation present versus absent*
([`diff-05-one-loose-song-named--06-last-track-says-nothing.png`](diff-05-one-loose-song-named--06-last-track-says-nothing.png)).
The one non-zero figure is nothing-playing versus playing, and all 244 of those
pixels are **inside the three button rectangles** and 0 outside them: the
glyphs' ink opacity changes when the transport becomes live, which is a colour,
not a position.

**3. The reserved lane holds the title still.** The topmost scanline carrying
type in the left zone:

```
01-nothing-playing            805      one line: "Nothing playing"
02-more-of-this-record        786      title / artist / continuation
03-popover-agrees             786
04-loose-songs-counted        796      title / continuation  (no artist tag)
05-one-loose-song-named       795      title / continuation
06-last-track-says-nothing    795      title / *reserved empty lane*
```

**795 and 795**: the frame that has a continuation and the frame that does not
put their titles on the same scanline. Without the reservation the block would
have shrunk by a line and re-centred, dropping the title ~8 px at the moment a
listener was reading it. (The 1 px between 04 and 05 is letterform, not layout —
`Cell` and `Terminal` have an `l` reaching full ascender height and `Battery`'s
tallest is a cap `B`.)

The same three properties are asserted as arithmetic in
`theme::tests::the_left_zone_reserves_the_continuation_line_whether_or_not_it_has_one`
and
`views::bottom_bar::tests::the_left_zone_reserves_the_continuation_and_the_count_in_every_state`:
the lane is exactly one line of `SIZE_CAPTION` type, and the zone's whole height
(62.35 px) stays under the centre column's (77 px), which is what keeps the
bar's height a property of the transport.

## The Queue control

Its readout was `3 / 12` and is now the queue's size — `Queue 20`, `Queue 3`,
and empty with nothing queued. The door says *what it opens*; the position is
stated better beside it, as what is **left**. Printing both would have been the
same subtraction twice. No slot was removed from the bar: one was replaced by a
better statement of the same fact, which is the single move
[`docs/REFUSALS.md`](../../../REFUSALS.md) permits here.

## Cleanup

The private display, the app instance, the scratch `HOME`/XDG tree and the
fixture were all created by this work and all removed after the captures.
Nothing else was started, and nothing of the maintainer's was opened.
