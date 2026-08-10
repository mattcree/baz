# ADR-0040: A hostile file — baz takes the bound it can take, and names the one it cannot

**Status**: accepted (2026-08-10) · answers `docs/WORK.md` item 1's *"whose bound"* · the containment is a new boundary around the decoder built in [ADR-0003](0003-stack.md)'s Symphonia and does not change any decision about *what* plays · reproducers in `crates/baz-core/tests/hostile_media.rs` and, as base64, in `docs/BACKLOG.md` (`fuzz/corpus/` is `.gitignore`d, which is §3's point)

## Context

The fuzz job in `.github/workflows/ci.yml` goes on `schedule` and
`workflow_dispatch`. Neither had ever fired, so its first run anywhere was
v0.1.0's release dry run ([run 31399606796][run]) — and it went red in forty
seconds, on twenty-nine bytes:

```text
ZkxhQwYn///7/1IA/wBJAQAAABMAAAAAAAAA/z0=
```

`fLaC`, a metadata block header declaring **type 6 (PICTURE), length
0x27FFFF**, and a picture body whose 32-bit MIME-type length is **0xFF004901 —
4,278,208,769 bytes** — inside a block claiming 2.6 MB, in a file of 29. The
allocation is not checked against the block that contains it, and
`AudioSource::open_bytes` is the probe path a file on disk takes, so the
question the queue asked is the right one: **whose bound is it?**

### It is symphonia's, and that was established before anything was written

Reproduced locally first, `cargo fuzz run playback_decode` on the artifact:

```text
#18 read_picture_block   symphonia-metadata-0.5.5/src/flac.rs:53:30
#19 FlacReader::init_with_metadata
#23 Probe::format
#25 AudioSource::from_media_source   crates/baz-core/src/playback/source.rs
```

Line 53 is `let mut media_type_buf = vec![0u8; media_type_len];`, where
`media_type_len` is a `u32` read from the file three lines earlier. baz's
contribution to the frame is the call to `Probe::format`.

Four findings decided the rest:

1. **It is unfixed upstream.** symphonia **0.6.0** (2026-05-15) has the same
   read as `read_boxed_slice_exact(media_type_len)`, and `read_boxed_slice_exact`
   is `vec![0u8; len]` in both versions. Upgrading buys nothing — and
   `docs/BACKLOG.md` already prices 0.6 as a large, unrelated migration.
2. **symphonia has the knob and does not turn it.**
   `MetadataOptions { limit_metadata_bytes, limit_visual_bytes }` exists in
   `symphonia-core` 0.5.5 and is *the* API for exactly this bound. No reader in
   the released tree reads either field — the whole crate graph has zero
   references outside the definition. baz already passes
   `MetadataOptions::default()`; there is nothing to pass differently.
3. **symphonia knows the rule and applies it unevenly.** Twenty lines from the
   picture block, in the same demuxer, the stream-info block reads: *"Ensure
   the block length is correct for a stream information block before allocating
   a buffer for it"* — and does. `symphonia-format-riff`'s equivalent site is
   commented `// TODO: Apply limit.`
4. **It is a class, not a site**, which is the finding that decided everything
   below:

   | site | container | asks for | how |
   |---|---|---|---|
   | `symphonia-metadata/flac.rs:53` picture MIME type | FLAC | 4,278,208,769 | CI, then reproduced here |
   | `symphonia-metadata/flac.rs:66` picture description | FLAC | up to 2³²−1 | seven minutes of `playback_decode` |
   | `symphonia-metadata/vorbis.rs:175` comment value | FLAC, Ogg | 4,294,967,040 | the same sweep; then cut to 20 bytes by hand |
   | `symphonia-format-riff/wave/chunks.rs:538` `LIST`/`INFO` value | WAV | 2,147,483,632 | predicted from the shape, built in 56 bytes, confirmed |

   The last row is the important one: it was written *because* the first three
   suggested a class, and it worked first time — in a container the fuzzer had
   not got to. Behind them, `symphonia-format-isomp4` reserves sample tables
   from a `u32` entry count in five more places (`stsz`, `stts`, `stsc`,
   `stco`, `co64`). Bounding this from outside means shadowing four container
   parsers.

### And the severity is not what the queue assumed — it is worse in one way

The queue called it *"an allocation the machine cannot serve"*. Measured, on
a 64-bit Linux desktop with default overcommit:

```text
$ rsscheck oom-input.flac
ERR: decode error: out of bounds
Maximum resident set size (kbytes): 3376
```

`calloc` of 4.28 GB is a **lazy zero mapping**; the read that follows fails
after 29 bytes, the reservation is dropped untouched, and `open_bytes` returns
an ordinary error. Peak RSS 3.4 MB. On the platform baz ships to, **this input
is not an out-of-memory at all** — libFuzzer's `-malloc_limit_mb` is what turns
it into one, and libFuzzer is right to.

Where it bites is where the reservation cannot be made — a smaller machine,
`vm.overcommit_memory=2`, a container limit, a 32-bit build:

```text
$ ( ulimit -v 2097152; rsscheck oom-input.flac )
memory allocation of 4278208769 bytes failed
Aborted (core dumped)
```

`SIGABRT`. No unwinding, no error, no skipped file — the process is gone.

### The fuzzer found something worse while answering this

Seven minutes per target against every one of the six, from an empty corpus,
turned up **three panics in symphonia's AAC reader** — and a panic needs no
overcommit caveat. It kills the thread on every platform, in a stable release
build, today:

| site | panic | reached from |
|---|---|---|
| `adts.rs:306` | `assertion failed: step != 0` (`Iterator::step_by`) | `open` |
| `adts.rs:303` | `attempt to subtract with overflow` | `open` |
| `aac/ics/mod.rs:365` | `index out of bounds: the len is 50 but the index is 50` | `next_block` |

Twenty-nine bytes reach the first. And the extension does not save anyone —
**all six of `AUDIO_EXTENSIONS` panic through `AudioSource::open` on the same
bytes**, because a probe identifies a stream by searching its bytes and the
ADTS sync word is twelve bits. `crates/baz-core/src/engine.rs` opens sources on
the decode thread, so what this costs a listener is the music stopping, from
one corrupt file in a scanned folder, on any machine.

## Decision

### 1. The bound on an *allocation* stays symphonia's, and baz does not shadow it

No pre-probe validator. The argument is not that one would be hard; it is that
a correct one is a second parser for four containers, and an incorrect one is
worse than nothing:

- **It cannot be a header walk.** Checking that each declared block fits inside
  the file catches all four inputs above — and misses the real case, a 3 MB
  album with one corrupt length field, where the block is honest and the field
  inside it is not. To catch that, the guard has to walk block *bodies*, which
  is symphonia's parser rewritten.
- **A guard that produces no values still drifts.** The liability is not
  duplicated logic, it is a *disagreement*: the day baz's walk and symphonia's
  reader disagree about where a block ends, the guard refuses a file that
  plays. For a music player, refusing a good file is the worse failure.
- **Real files lie about their sizes.** WAV files written by streaming
  encoders routinely declare `0xFFFFFFFF`. A size-sanity rule general enough to
  cover the class would refuse them.
- **A pin buys nothing.** `Cargo.lock` already pins 0.5.5 exactly; the defect
  is in 0.5.5 and in 0.6.0.
- **A fork is the largest option, not the smallest.** `[patch.crates-io]`
  against a vendored `symphonia-metadata` is three thousand lines of somebody
  else's MPL-2.0 code to re-sync forever, and it covers one of the two crates
  the class lives in.

**What baz does instead**: every reproducer is in the repository as a test
(§3), the residue is written down in `docs/BACKLOG.md`, and the defect is
upstream's to fix. *Needs the owner's hand:* filing it there is a
GitHub account and not an agent's to do — `docs/BACKLOG.md` carries the report
text ready to paste.

### 2. The bound on an *unwind* is baz's, and it is taken

A panic crossing out of a decoder is a different thing from an allocation
failing, and baz can hold it without knowing anything about any format:

```rust
fn contain_panics<T>(doing: &'static str, call: impl FnOnce() -> Result<T, PlaybackError>)
    -> Result<T, PlaybackError>
```

`std::panic::catch_unwind` around all three doors —
`AudioSource::open`/`open_bytes` (the probe, the demuxer's metadata, the codec
registry and MP4's first packet), `next_block` (decode) and `seek` — turning
the payload into `PlaybackError::DecoderPanicked { doing }`. A hostile file now
fails exactly where a merely unreadable one already failed.

This is a boundary, not a shadow parser: it knows no container, has no opinion
about any length field, cannot refuse a file that plays, and does not grow when
symphonia grows a format. It is the whole panic class, including the ones the
fuzzer has not found yet.

**And `next_block` is the decode hot path, so it was measured rather than
assumed.** 300 seconds of 16-bit stereo WAV, 11,485 `next_block` calls, best of
fifteen, three runs of each build: **0.0485 s with the containment against
0.0483 s without** — inside the noise, and about four thousand times real time
either way. `catch_unwind` costs nothing until something unwinds.

**Three things it deliberately does not do.**

- **It does not hide the panic.** The standard hook has already put the
  message on stderr, and the error is a *named variant*, distinct from
  `Decode`: `Decode` says the bytes are not a stream, `DecoderPanicked` says a
  parser broke and there is a bug to report. Swallowing a panic into a generic
  error would have made baz's own future bugs invisible, which is the real
  objection to `catch_unwind` and is answered by the variant rather than by
  declining it.
- **It does not help with an abort.** `handle_alloc_error` is not an unwind.
  §1 is why that is left where it is.
- **It does not survive `panic = "abort"`.** No profile in `Cargo.toml` sets
  it, and `unwinding_is_what_makes_the_containment_work` fails if one ever
  does — a build that aborts cannot run the test that would have caught it, so
  the guard is on the manifest rather than on the behaviour.

### 2.5. baz probes for the formats it plays, and stops registering one it does not

The three panics were all in symphonia's **raw-ADTS** reader and the decoder
behind it, and baz has no use for either as a *format*. `.aac` is not an
`AUDIO_EXTENSIONS` member, so no raw ADTS stream is ever listed and none is
ever played; every AAC baz decodes arrives inside an MP4. Yet
`symphonia::default::get_probe()` registers `AdtsReader`, and a probe
identifies a stream by **searching its bytes for a marker** rather than by
trusting a name — so arbitrary bytes under any extension could reach it, and
did.

`playback::source::probe` is therefore `register_enabled_formats` written out,
minus that one line. Six registrations: FLAC, MPEG audio, ISO-MP4, Ogg, WAV,
and the ID3v2 metadata reader.

Two things this turned up that are worth keeping:

- **The markers overlap.** MPEG audio's frame sync is eleven set bits and
  ADTS's is twelve, so both readers claimed the same corrupt `.mp3`. Removing
  the one baz cannot use hands those bytes back to `MpaReader` — which is why
  *"no suitable format reader"* is asserted nowhere: the files are still
  claimed, by the right reader, and come back as ordinary decode errors.
- **`Id3v2Reader` is the registration that is easy to forget**, and nothing in
  the suite covered it: an MP3's ReplayGain lives in ID3v2 `TXXX` frames, so
  dropping it would have stopped MP3 ReplayGain silently with every other test
  green. `an_mp3s_replay_gain_comes_off_its_id3v2_block` now covers it, and was
  confirmed to fail with the line removed.

**What it costs, stated rather than hidden**: a raw ADTS stream misnamed
`.m4a` used to play and now does not. It was never listed by the scanner, so
reaching it meant opening it by some other route.

This is not a retreat from §2. §2 is the guard for the parsers baz *does*
register — a crafted MP4 still reaches the AAC decoder where the third panic
lived. What §2.5 removes is a parser baz was exposed to and never wanted.

#### Amendment, 2026-08-10 — §2.5 was suspected of losing an album, and is cleared

Hours after this section landed, fourteen of the owner's MP3s were found
missing from his library, and §2.5 was the first suspect for a good reason:
`file(1)` describes every one of them as *"MPEG ADTS, layer III"*, which is
precisely the shape of file the reader removed here could have claimed, and
this section itself records that MPEG audio's sync word and ADTS's overlap.

**It is not §2.5, and this was established by running it rather than by
reasoning about it.** `fbb0af7` — the commit immediately before the registry
change — and `main` were both built and both run over the same folder. Both
skipped the same fourteen files with the same sentence, and under both, all
fourteen opened and decoded through `AudioSource` without error. The cause was
a malformed ID3v2.3 `TYER` frame failing lofty's whole-file read
(`CHANGELOG.md`; the fix is `crates/baz-core/src/library.rs`).

The reason no registry change *could* have caused it is worth stating here,
because it is not obvious from this file: **the scanner and the player do not
share a parser.** `baz_core::library` reads tags with **lofty** and never
constructs a `Probe`, so what a file's row on the shelf says — and whether it
gets a row at all — is decided without Symphonia being consulted. This
section's registry is reached only when something is *played*. A file can
therefore be unlistable and perfectly playable, which is exactly what those
fourteen were, and the two halves have to be diagnosed separately.

The cost §2.5 states above stands unchanged and unrelated: a raw ADTS stream
misnamed `.m4a`, which was never listed by the scanner either.

### 3. A reproducer is a gate, not a corpus entry

The fuzz job runs on `schedule` and `workflow_dispatch`; a tag arrives as
`push`. **The gate a release is held to never fuzzes** — `docs/RELEASING.md`
says so — so a finding that lives only in `fuzz/corpus/` can regress through a
whole release before anything notices.

Every input above is therefore also a test in
`crates/baz-core/tests/hostile_media.rs`, run by `cargo test` on every push,
through both `open_bytes` **and** `open` on a real file, under **every one of
`AUDIO_EXTENSIONS`** — because the measurement in the context section is that
the extension decides nothing. They are 20 to 56 bytes and cost microseconds.

What those tests assert is *an error, not a panic, not an abort, not a hang* —
never which error, which would pin baz's suite to symphonia's internals. The
one exception is `the_aac_inputs_are_refused_rather_than_contained`, where the
*absence* of `DecoderPanicked` is the assertion: it fails the day `AdtsReader`
is registered again, and it fails with a variant that names what happened
instead of with a dead test process.

The mechanism of §2 is tested separately, in `source.rs`'s own module
(`a_panic_out_of_a_decoder_becomes_an_error`), and the reason is worth being
plain about: **§2.5 removed every input that reaches a panic**, so there is no
file left to demonstrate the containment with. It stays because the parsers
baz does register are the same shape as the one it stopped registering, and
because the fuzzer found three of these in seven minutes from an empty corpus.

### 4. CI's fuzz job measures residency, not reservation

`-rss_limit_mb` stays at 2048: memory baz's decode path actually *touches* is
baz's business, and a target that grows resident is a real defect wherever it
lives. `-malloc_limit_mb` goes to **6144**, above the largest reservation a
32-bit length field can name, with §1 named in the step — because a single
oversized reservation inside symphonia is a known, measured, upstream-owned
condition, and a job permanently red for one written-down condition is a job
nobody reads, which would cost the other five targets their audience.

The distinction is only defensible because it was measured: 3.4 MB of peak RSS
for a request of 4,278,208,769 bytes, and all three known inputs confirmed to
pass under exactly this pair of flags. Note the flag is not
`-malloc_limit_mb=0`: **zero means "use `rss_limit_mb`", not "off"** — a first
attempt at this change set it to 0 and changed nothing at all, which is the
kind of thing a verification run catches and a reading does not.

**`playback_decode` will still go red, and this is the honest version of a
claim an earlier draft of this ADR got wrong.** That draft said the job could
now go green. A verification run said otherwise within a minute: 122 bytes of
ISO-MP4 declaring a `ftyp` length of `u64::MAX` reach
`symphonia-format-isomp4`'s `atoms/mod.rs:449` and `attempt to add with
overflow`. §2 contains it — the same bytes come back as `DecoderPanicked` from
a debug build and, in release, as symphonia's own *"invalid ftyp data length"*
once the sum has wrapped — but **libfuzzer-sys installs a panic hook that
calls `process::abort()`**, so the abort happens *before* `catch_unwind` can
regain control. No containment in baz can make a panic invisible to the
fuzzer, and it should not: the fuzzer's job is to find them.

So the settled position is: the job reports upstream panics in the parsers baz
keeps, and will keep doing so until symphonia fixes them. What changed here is
that **one red target no longer hides the other five** — GitHub runs a `run:`
block under `bash -e`, so the first failing target used to abort the step. The
loop now runs every target and fails at the end with the list.

The job's **triggers are not changed here.** Fuzzing every push would put
half an hour on every commit, and the asymmetry it creates is written down in
`docs/RELEASING.md` rather than discovered. Now that the panic class is a
`cargo test` gate (§3), the thing most worth catching is caught on push
anyway.

## Consequences

- **A corrupt file in a scanned folder can no longer stop the music.** It
  fails to open, the way an empty file already did.
- **baz's error surface gains one variant**, `DecoderPanicked`, which is a
  thing to grep for in a bug report.
- **The 4 GB reservation is still made** on a machine that can serve it, and
  still aborts on one that cannot. That is the residue this ADR declines to
  fix in baz, and it is in `docs/BACKLOG.md` under *Known gaps in shipped
  features* with every reproducer.
- **Five defects that were baz's own were found by the same sweep and fixed
  outright**: a `u64` multiplication in the play ledger's date arithmetic
  (`history/format.rs`), a byte-index slice on tag text in `replaygain.rs`
  that panicked `open` on one mis-encoded gain, and three ways
  `playlist/format.rs` rewrote a file on every save. None needed a decision;
  all are in `CHANGELOG.md`. Two targets — `protocol_deserialize` and
  `scanner_inference` — came out clean.
- **If symphonia is ever forked or replaced**, §2 stays true and §1 is what
  gets revisited.

[run]: https://github.com/mattcree/baz/actions/runs/31399606796
