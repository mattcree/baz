//! Spike B verification suite. All headless: engine output goes into an
//! OfflineSink and is compared against synthesized/reference-decoded ground
//! truth — never against the engine's own recorded output.

use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

use baz_spike_audio_gapless::engine::{run_playlist, EngineConfig, RateStrategy};
use baz_spike_audio_gapless::fixtures::{
    self, FixtureSet, AMP, FREQ, RATE, RATE_HI, RATE_PAIR_FRAMES_44K, RATE_PAIR_FRAMES_48K,
    SPLIT_FRAME, TOTAL_FRAMES,
};
use baz_spike_audio_gapless::signal;
use baz_spike_audio_gapless::sink::OfflineSink;
use baz_spike_audio_gapless::source::AudioSource;

fn fixtures() -> &'static FixtureSet {
    static F: OnceLock<FixtureSet> = OnceLock::new();
    F.get_or_init(|| {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("spike-b-fixtures");
        fixtures::generate(&dir).expect("fixture generation")
    })
}

fn test_config() -> EngineConfig {
    EngineConfig {
        ring_frames: 8192,
        consumer_chunk_frames: 2048,
        consumer_pace: Duration::from_micros(500),
        strategy: RateStrategy::Resample,
    }
}

/// Exact sample equality with a useful failure message.
fn assert_samples_eq(got: &[f32], want: &[f32], what: &str) {
    assert_eq!(got.len(), want.len(), "{what}: length mismatch");
    if let Some(i) = (0..got.len()).find(|&i| got[i] != want[i]) {
        panic!(
            "{what}: first mismatch at interleaved sample {i} (frame {}): got {} want {}",
            i / 2,
            got[i],
            want[i]
        );
    }
}

/// Continuity check on channel 0 around an output frame boundary: the largest
/// adjacent-sample jump must be consistent with a continuous sine (no click,
/// no dropped/duplicated samples).
fn assert_boundary_continuous(interleaved: &[f32], boundary_frame: usize, what: &str) -> f32 {
    let ch0 = signal::channel(interleaved, 2, 0);
    let lo = boundary_frame.saturating_sub(1000);
    let hi = (boundary_frame + 1000).min(ch0.len());
    let max_delta = signal::max_adjacent_delta(&ch0[lo..hi]);
    let bound = signal::sine_adjacent_bound(FREQ, AMP, RATE) * 1.05;
    assert!(
        max_delta <= bound,
        "{what}: adjacent-sample jump {max_delta} at boundary exceeds continuous-sine bound {bound}"
    );
    max_delta
}

/// Sanity: the two split WAVs concatenated decode to exactly the reference.
/// (Pure source-level check, no engine involved.)
#[test]
fn split_wavs_reconstruct_reference() {
    let f = fixtures();
    let (reference, rate, ch) = AudioSource::decode_all(&f.ref_f32).unwrap();
    assert_eq!((rate, ch), (RATE, 2));
    assert_eq!(reference.len(), TOTAL_FRAMES * 2);
    let (p1, _, _) = AudioSource::decode_all(&f.part1_f32).unwrap();
    let (p2, _, _) = AudioSource::decode_all(&f.part2_f32).unwrap();
    assert_eq!(p1.len(), SPLIT_FRAME * 2);
    let mut joined = p1;
    joined.extend_from_slice(&p2);
    assert_samples_eq(&joined, &reference, "split WAV concatenation");
}

/// Deliverable 3a (WAV): engine output over [part1, part2] is
/// sample-for-sample identical to the single-file reference decode, and the
/// splice region is click-free.
#[test]
fn engine_gapless_wav_exact() {
    let f = fixtures();
    let (reference, _, _) = AudioSource::decode_all(&f.ref_f32).unwrap();
    let mut sink = OfflineSink::with_capacity(reference.len());
    let report = run_playlist(
        &[f.part1_f32.clone(), f.part2_f32.clone()],
        test_config(),
        &mut sink,
    )
    .unwrap();

    assert_eq!(report.stream_rate, RATE);
    assert_eq!(report.track_start_frames, vec![0, SPLIT_FRAME]);
    assert_samples_eq(sink.samples(), &reference, "gapless WAV output");
    let max_delta = assert_boundary_continuous(sink.samples(), SPLIT_FRAME, "gapless WAV");
    println!(
        "[gapless-wav] output={} samples, boundary max adjacent delta={:.6} (bound {:.6})",
        sink.samples().len(),
        max_delta,
        signal::sine_adjacent_bound(FREQ, AMP, RATE)
    );
}

/// Deliverable 3a (FLAC): same assertion over FLAC decode via Symphonia.
/// Also cross-checks FLAC losslessness against the i16 WAV ground truth.
#[test]
fn engine_gapless_flac_exact() {
    let f = fixtures();
    let Some(flac) = &f.flac else {
        eprintln!("SKIP: no ffmpeg or flac CLI available; FLAC fixtures not generated");
        return;
    };
    // FLAC must decode bit-identically to the i16 WAV it was encoded from.
    let (reference_flac, rate, ch) = AudioSource::decode_all(&flac.full).unwrap();
    let (reference_wav, _, _) = AudioSource::decode_all(&f.ref_i16).unwrap();
    assert_eq!((rate, ch), (RATE, 2));
    assert_samples_eq(&reference_flac, &reference_wav, "FLAC vs i16 WAV reference");

    let mut sink = OfflineSink::with_capacity(reference_flac.len());
    let report = run_playlist(
        &[flac.part1.clone(), flac.part2.clone()],
        test_config(),
        &mut sink,
    )
    .unwrap();
    assert_eq!(report.track_start_frames, vec![0, SPLIT_FRAME]);
    assert_samples_eq(sink.samples(), &reference_flac, "gapless FLAC output");
    let max_delta = assert_boundary_continuous(sink.samples(), SPLIT_FRAME, "gapless FLAC");
    println!(
        "[gapless-flac] encoder={}, output={} samples, boundary max adjacent delta={:.6}",
        flac.encoder,
        sink.samples().len(),
        max_delta
    );
}

/// Deliverable 3b: decode-ahead of track N+1 demonstrably overlaps playback
/// of track N.
#[test]
fn decode_ahead_overlaps_playback() {
    let f = fixtures();
    let mut sink = OfflineSink::with_capacity(TOTAL_FRAMES * 2);
    let report = run_playlist(
        &[f.part1_f32.clone(), f.part2_f32.clone()],
        test_config(),
        &mut sink,
    )
    .unwrap();
    let p = &report.prefetch;
    println!(
        "[decode-ahead] track2 decode finished at {:.2} ms; track1 drained at {:.2} ms; \
         track2 frames decoded at drain: {}/{} ({:.1}%); track2 decode took {:.2} ms",
        p.next_decode_done_ms_from_start,
        p.prev_drain_ms_from_start,
        p.next_frames_decoded_when_prev_drained,
        p.next_track_frames_total,
        100.0 * p.next_frames_decoded_when_prev_drained as f64 / p.next_track_frames_total as f64,
        p.next_decode_ms,
    );
    assert_eq!(p.next_track_frames_total, TOTAL_FRAMES - SPLIT_FRAME);
    assert!(
        p.next_frames_decoded_when_prev_drained > 0,
        "no overlap: track 2 had not started decoding when track 1 drained"
    );
    assert!(
        p.next_decode_finished_before_prev_drained,
        "track 2 decode did not finish before track 1 finished draining \
         (decoded {}/{} frames at drain)",
        p.next_frames_decoded_when_prev_drained, p.next_track_frames_total
    );
    assert!(p.next_decode_done_ms_from_start < p.prev_drain_ms_from_start);
}

/// Deliverable 3c, strategy (a) "reopen": flush/drain + notional stream
/// reconfigure at the rate boundary. Segments stay bit-exact; the cost (the
/// gap) is measured.
#[test]
fn rate_change_reopen_measures_gap() {
    let f = fixtures();
    let mut cfg = test_config();
    cfg.strategy = RateStrategy::Reopen;
    let expected_frames = RATE_PAIR_FRAMES_44K + RATE_PAIR_FRAMES_48K;
    let mut sink = OfflineSink::with_capacity(expected_frames * 2);
    let report = run_playlist(&[f.rate_44k.clone(), f.rate_48k.clone()], cfg, &mut sink).unwrap();

    assert_eq!(
        report.reconfigures.len(),
        1,
        "expected one reconfigure event"
    );
    let ev = &report.reconfigures[0];
    assert_eq!(ev.at_output_frame, RATE_PAIR_FRAMES_44K);
    assert_eq!((ev.from_rate, ev.to_rate), (RATE, RATE_HI));
    assert!(
        ev.buffered_frames_at_boundary > 0,
        "boundary hit with an empty ring; flush-cost measurement is vacuous"
    );

    // Each segment is bit-exact against its own file's decode; the gap between
    // them is the price of this strategy, not a decode error.
    let (a, _, _) = AudioSource::decode_all(&f.rate_44k).unwrap();
    let (b, _, _) = AudioSource::decode_all(&f.rate_48k).unwrap();
    let out = sink.samples();
    assert_eq!(out.len(), expected_frames * 2);
    assert_samples_eq(&out[..a.len()], &a, "reopen segment 1");
    assert_samples_eq(&out[a.len()..], &b, "reopen segment 2");

    println!(
        "[reopen] buffered at boundary: {} frames = {:.2} ms of audio \
         (a hard flush discards this; a drain waits it out); drain wait measured: {:.2} ms; \
         real device reopen latency NOT measurable without ALSA (typically tens of ms, TBD)",
        ev.buffered_frames_at_boundary, ev.buffered_ms_at_boundary, ev.drain_wait_ms
    );
}

/// Deliverable 3c, strategy (b) "resample": track 2 (48 kHz) is converted to
/// the 44.1 kHz stream rate with rubato and spliced seamlessly. The sine must
/// continue through the boundary within a small tolerance.
#[test]
fn rate_change_resample_is_continuous() {
    let f = fixtures();
    let cfg = test_config(); // Resample strategy
    let expected_frames = RATE_PAIR_FRAMES_44K + RATE_PAIR_FRAMES_44K; // 48k half resamples to 220500
    let mut sink = OfflineSink::with_capacity(expected_frames * 2);
    let report = run_playlist(&[f.rate_44k.clone(), f.rate_48k.clone()], cfg, &mut sink).unwrap();

    assert!(
        report.reconfigures.is_empty(),
        "resample path must not reconfigure"
    );
    assert_eq!(report.stream_rate, RATE);
    let out = sink.samples();
    assert_eq!(
        out.len(),
        expected_frames * 2,
        "resampled output length (delay-compensated) should be exactly 10 s at 44.1 kHz"
    );

    let ch0 = signal::channel(out, 2, 0);
    let boundary = RATE_PAIR_FRAMES_44K;

    // Track 1 region passes through untouched: exact.
    let ideal_track1 = signal::max_error_vs_sine(&ch0, FREQ, AMP, RATE, 0..boundary);
    assert_eq!(
        ideal_track1, 0.0,
        "track 1 must be bit-exact (no resampling applied)"
    );

    // Boundary region and the whole resampled tail: the sine continues within
    // a small tolerance (sinc passband error + edge effects).
    let err_boundary = signal::max_error_vs_sine(
        &ch0,
        FREQ,
        AMP,
        RATE,
        boundary.saturating_sub(200)..boundary + 2000,
    );
    let err_tail = signal::max_error_vs_sine(&ch0, FREQ, AMP, RATE, boundary..ch0.len());
    let max_delta = signal::max_adjacent_delta(&ch0);
    // In the resampled region the sinc passband ripple (up to the 0.01
    // continuity tolerance, measured ~4e-3) rides on top of the ideal sine
    // slope, so adjacent samples may legitimately differ by up to
    // bound + 2*ripple. A splice error would blow far past this.
    let bound = signal::sine_adjacent_bound(FREQ, AMP, RATE) + 2.0 * 0.01;

    println!(
        "[resample] resample time: {:.2} ms for 5 s of 48 kHz stereo ({}x realtime); \
         max |error| vs ideal sine: boundary region {:.2e}, entire resampled tail {:.2e}; \
         max adjacent delta whole output {:.6} (continuous-sine bound {:.6})",
        report.resample_ms.unwrap_or(f64::NAN),
        (5000.0 / report.resample_ms.unwrap_or(f64::NAN)).round(),
        err_boundary,
        err_tail,
        max_delta,
        signal::sine_adjacent_bound(FREQ, AMP, RATE)
    );

    assert!(
        err_boundary < 0.01,
        "splice not continuous: boundary error {err_boundary} vs ideal sine"
    );
    assert!(
        err_tail < 0.01,
        "resampled tail deviates from ideal sine by {err_tail}"
    );
    assert!(
        max_delta <= bound,
        "click detected: adjacent delta {max_delta} exceeds bound {bound}"
    );
}
