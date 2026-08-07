# Spike B — Gapless Engine Core: Results

> Throwaway spike (per NEXT-STEPS.md Phase 1). It answers questions; it does not become the codebase.
> All verification is offline: alsa-lib-devel is missing on this machine, so device output is behind the
> non-default `device-output` cargo feature (confirmed: enabling it fails at `alsa-sys` / missing `alsa.pc`).
> Run: `cargo test` (default features, headless). Numbers below from `cargo test --release -- --nocapture`,
> Linux, Rust 1.92.

## Architecture proven

```
                    +--------------------+
 track N   file --> |  AudioSource       |            decode (worker) thread
                    |  (Symphonia probe/ |
                    |   decode -> f32)   |
                    +---------+----------+
                              | interleaved f32 blocks
                              v
                    [ rate strategy at boundary:            +--------------------+
                      same rate -> splice                   | prefetch thread    |
                      Resample  -> rubato -> stream rate    | AudioSource(N+1)   |
                      Reopen    -> drain, notional          | -> Vec<f32> + progress
                                   stream reconfigure ]     |    atomics         |
                              |                             +---------+----------+
                              v                                       |
                    +--------------------+      buffer handed to decode thread
                    |  rtrb SPSC ring    |<-----------------------------+
                    |  (lock-free,       |   at the track boundary (join)
                    |   8192 frames)     |
                    +---------+----------+
                              | wait-free read_chunk (no alloc/lock/IO)
                              v
                    +--------------------+
                    |  Sink trait        |   consumer loop = stand-in for the
                    |  OfflineSink (test)|   audio callback; pull path upholds
                    |  [cpal callback    |   realtime rules by construction
                    |   behind feature]  |
                    +--------------------+
```

- `#![forbid(unsafe_code)]` holds — `rtrb` provides the lock-free SPSC ring, no hand-rolled unsafe.
- Splice is plain sample-accurate concatenation through the ring; track boundaries are bookkeeping, not audio events.
- `cargo clippy --all-targets` is clean.

## Test results (6/6 green, debug and release)

| Test | Asserts | Result / key numbers |
|---|---|---|
| `split_wavs_reconstruct_reference` | the two 5 s halves (split at frame 220 513, a non-zero-crossing: sin = 0.73·A) concatenate to the 10 s reference, decode-level | exact |
| `engine_gapless_wav_exact` | engine([part1, part2]) output == single-file reference decode, sample for sample | exact, 882 000/882 000 samples; boundary max adjacent-sample jump **0.050143** = the theoretical continuous-sine bound 2·A·sin(π·440/44100) = 0.050143 — no click, no gap, no overlap |
| `engine_gapless_flac_exact` | same over FLAC (encoded with **ffmpeg**; no `flac` CLI on this machine); plus FLAC-vs-i16-WAV losslessness cross-check | exact; boundary jump 0.050171 (bound + i16 quantization 2/32768 ≈ 6.1e-5) |
| `decode_ahead_overlaps_playback` | track 2 decoded before track 1 finished draining | track 2 **100 % decoded (220 487/220 487 frames) at 1.6 ms**; track 1 drained at 61.2  ms (consumer paced ~92× realtime) — ~39× margin. WAV decode of 5 s took 1.5 ms ≈ 3400× realtime |
| `rate_change_reopen_measures_gap` | strategy (a): both segments bit-exact vs their own decodes; one reconfigure event with measured cost | buffered at boundary: **7508 frames = 170.3 ms of audio** (ring nearly full, 8192-frame ring). A hard flush discards it; drain-then-reconfigure waited **2.1 ms** at test pace (would be the full 170 ms at 1× realtime) |
| `rate_change_resample_is_continuous` | strategy (b): 48 kHz track resampled to 44.1 kHz stream rate, spliced seamlessly | output exactly 10 s at 44.1 kHz; track-1 region bit-exact; sine continues through boundary and entire tail with max error **4.25e-3** (−45.5 dB re. amplitude 0.8); resample of 5 s stereo took **33 ms ≈ 152× realtime** (release; 1.76 s ≈ 3× realtime in debug) |

Ground truth is synthesized (continuous 440 Hz sine) or reference single-file decode — never the engine's own recorded output (ENGINEERING.md rule).

## Decode-ahead evidence

Prefetch of track N+1 runs on its own thread, publishing progress via atomics; the consumer snapshots that
progress at the instant it drains track N's last sample. Measured: decode of track 2 finished at **1.6 ms**
into the run; track 1 finished draining at **61.2 ms**. The prefetch thread never touches the ring — the
consumer's guarantees are structurally unaffected by decode-ahead.

## Sample-rate change at a boundary — ADR-0004 input

Measured comparison (44 100 Hz → 48 000 Hz boundary):

| | (a) Reopen (flush + stream reconfigure) | (b) Resample to stream rate (rubato sinc) |
|---|---|---|
| Continuity | **Gap guaranteed.** 170 ms of audio buffered at the boundary must be drained (audible silence while the device reopens) or discarded (truncates track ending). Real device reopen adds unmeasured latency on top (needs ALSA; typically tens of ms). | **Seamless.** Sine continues through the splice; worst-case sample error 4.25e-3 (−45.5 dB), max adjacent-sample jump within continuous-signal bound + ripple. |
| Bit-exactness | Bit-exact per segment (verified). | Not bit-exact for the resampled track (by definition). |
| CPU cost | ~0 | 33 ms per 5 s of stereo (152× realtime, release) — trivially absorbed by the prefetch thread before the splice. |
| Complexity | Device lifecycle churn at boundaries; error paths (device busy, rate unsupported) land mid-playlist. | One well-tested DSP component; needs careful splice alignment (see gotcha below). |

**Recommendation for ADR-0004**: resample to a fixed/negotiated stream rate by default; reopen the stream
only in an explicit bit-perfect/exclusive mode where the user has opted into gaps at rate boundaries. The
measurements support it: reopen's cost is an unavoidable audible discontinuity (≥ the buffered 170 ms drained,
plus real reopen latency), while resample's costs are 33 ms of worker-thread CPU and −45 dB worst-case error —
below 16-bit audibility concerns and zero on same-rate boundaries (the overwhelmingly common case).
Caveat honestly noted: real device reopen latency was **not** measurable here; the comparison's reopen column
is a lower bound on its cost.

### Rubato gotcha (worth keeping)

`SincFixedIn::output_delay()` reports 117 frames, but the output is **already time-aligned** with input frame 0
(`last_index` starts at `-sinc_len/2`; verified with an impulse test — see `examples/lag.rs`: impulse at input
frame 48 000@48 kHz lands at output frame 44 100 exactly). Trimming `output_delay()` frames — the obvious
reading of the docs — shifts the splice by ~2.7 ms and produced a 0.8-amplitude discontinuity. The alignment
cost is instead an onset/tail transient (zero history in the sinc window), which the spike removes by
anti-reflective padding sized as a multiple of `from/gcd(from,to)` so the pad trims to an integer number of
output frames (no sub-sample phase error). The production engine must own this alignment logic and test it,
exactly as done here.

## Unproven until alsa-lib-devel (or another backend) is available

- Real device timing: callback cadence, buffer sizes, underrun behavior, actual drain latency.
- Measured device reopen latency for the Reopen strategy (the ADR-0004 table's missing number).
- Exclusive mode (ALSA `hw:` / WASAPI exclusive) and its bit-exactness sanity check.
- That the cpal glue in `src/device.rs` (feature `device-output`) even compiles — it is written to the same
  realtime rules but has never been built here.
- Scheduler behavior of the pull path under realtime priority (the offline consumer proves structure, not timing).

## Symphonia ergonomics — honest notes

- End-of-stream is signaled as `Error::IoError(UnexpectedEof)` from `next_packet()`, not a dedicated variant or
  `Option` — every consumer writes the same fragile match on `io::ErrorKind`.
- `decode()` returns an `AudioBufferRef` borrowing the decoder; getting interleaved f32 means maintaining a
  `SampleBuffer` sized from `decoded.capacity()` and re-copying per packet (and re-allocating if a later packet
  is larger). Workable, but boilerplate every integrator re-derives.
- `codec_params.sample_rate` / `channels` are `Option`s even for formats where they are mandatory; error handling
  is on the caller for cases that should be unrepresentable.
- WAV and FLAC carry exact sample counts, so gapless is pure concatenation (this spike). For MP3/AAC the engine
  must apply encoder delay/padding trim itself; Symphonia exposes some of this but the spike did not exercise
  it — flagged as future work for the real engine, with the same synthesized-signal tests.
- On the plus side: probe → format → decoder worked identically for WAV-f32, WAV-i16, and ffmpeg-encoded FLAC,
  and FLAC decode was bit-exact against the i16 WAV ground truth on the first try.

## Files

- `src/engine.rs` — playlist engine (decode thread, prefetch thread, ring, consumer, both rate strategies)
- `src/source.rs` — Symphonia wrapper; `src/sink.rs` — `Sink` + `OfflineSink`; `src/resample.rs` — aligned rubato wrapper
- `src/signal.rs`, `src/fixtures.rs`, `src/bin/gen_signals.rs` — synthesized ground truth (WAV via hound, FLAC via ffmpeg)
- `tests/engine.rs` — the six verification tests
- `examples/lag.rs` — impulse/sine alignment evidence for the rubato gotcha
