//! EBU R128 compliance: baz's loudness meter against the reference material.
//!
//! `docs/ENGINEERING.md` names this explicitly — *"Loudness/ReplayGain:
//! validated against reference implementations (EBU R128 test vectors)"* — and
//! ADR-0015 makes it the deliverable rather than a nicety: a loudness number
//! nobody checked is worse than no number, because it reaches a listener's
//! speakers wearing the authority of a measurement.
//!
//! # What is asserted
//!
//! **EBU Tech 3341's compliance signals for integrated loudness**, generated
//! here from the specification's own description (a 1 kHz sine at a stated
//! dBFS amplitude, in the stated segments) and measured with
//! [`baz_core::loudness`]. The tolerance is the **±0.1 LU the specification
//! states**, not one chosen to fit the result — and the measured margins are
//! recorded in each assertion's message so a regression that stays inside the
//! tolerance is still visible.
//!
//! The cases are 1–5. Case 6 is 5.0-channel material and is **not** here for a
//! stated reason: baz decodes everything to stereo
//! ([`baz_core::playback::CHANNELS`]), so there is no path by which five
//! channels could reach this meter, and a five-channel test would be testing a
//! capability the player does not have. Cases 7–9 pin the *momentary* and
//! *short-term* meters, which this unit does not implement (ADR-0015 says so,
//! and says why: ReplayGain needs the integrated figure and nothing else).
//!
//! Every case is run at **48 kHz and at 44.1 kHz**. The standard tabulates its
//! filter at 48 kHz only, so a library at 44.1 — which is most libraries — is
//! measured by coefficients baz derived, and the derivation is exactly the
//! thing that could be wrong.

use baz_core::loudness::{Loudness, LoudnessMeter, album_lufs};

/// The tolerance EBU Tech 3341 states for an integrated-loudness measurement,
/// in LU. Not a tolerance chosen to accommodate this implementation: it is the
/// specification's own, and the measured margins below are an order of
/// magnitude inside it.
const TOLERANCE_LU: f64 = 0.1;

/// The rates every case is measured at: the one the standard tabulates, and
/// the one CDs and most libraries actually use.
const RATES: [u32; 2] = [48_000, 44_100];

/// One segment of a test signal: how long, and at what amplitude.
///
/// The amplitude is the sine's **peak** in dBFS, which is how EBU Tech 3341
/// specifies its signals — a 1 kHz sine of peak amplitude −23 dBFS in both
/// channels is the signal that must measure −23.0 LUFS, and getting peak and
/// RMS the wrong way round here would show up as a 3.01 LU error.
struct Segment {
    seconds: f64,
    dbfs: f64,
}

/// Generate an interleaved stereo 1 kHz sine from `segments`, at `rate`.
///
/// The phase runs continuously across segment boundaries so the signal is a
/// tone that changes level, not a sequence of tones with clicks between them —
/// a discontinuity would put broadband energy into the K-weighted high shelf
/// and move the answer.
fn tone(rate: u32, segments: &[Segment]) -> Vec<f32> {
    let mut samples = Vec::new();
    let mut frame: u64 = 0;
    for segment in segments {
        let amplitude = 10f64.powf(segment.dbfs / 20.0);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // seconds x rate
        let count = (segment.seconds * f64::from(rate)).round() as u64;
        for _ in 0..count {
            #[expect(clippy::cast_precision_loss, reason = "frame counts are small")]
            let t = frame as f64 / f64::from(rate);
            #[allow(clippy::cast_possible_truncation)] // f64 sine -> f32 sample
            let value = (amplitude * (std::f64::consts::TAU * 1_000.0 * t).sin()) as f32;
            samples.push(value);
            samples.push(value);
            frame += 1;
        }
    }
    samples
}

/// Measure an interleaved stereo buffer.
fn measure(rate: u32, interleaved: &[f32]) -> Loudness {
    let mut meter = LoudnessMeter::new(rate, 2).expect("a stereo meter at a real rate");
    meter.push(interleaved);
    meter.finish()
}

/// Assert an integrated-loudness reading against Tech 3341's target, at both
/// rates, reporting the margin so a drift inside the tolerance is still
/// visible in the output.
fn assert_case(name: &str, target: f64, segments: &[Segment]) {
    for rate in RATES {
        let measured = measure(rate, &tone(rate, segments))
            .integrated_lufs()
            .unwrap_or_else(|| panic!("{name} at {rate} Hz measured nothing at all"));
        let error = (measured - target).abs();
        assert!(
            error <= TOLERANCE_LU,
            "{name} at {rate} Hz: measured {measured:.4} LUFS, \
             EBU Tech 3341 requires {target:.1} ±{TOLERANCE_LU} LU (error {error:.4} LU)"
        );
        println!("{name} at {rate} Hz: {measured:.4} LUFS (error {error:.4} LU)");
    }
}

/// **EBU Tech 3341 case 1**: a 1 kHz stereo sine at −23 dBFS for 20 s must
/// measure −23.0 LUFS.
///
/// The single most load-bearing vector in the file. It checks, in one number,
/// the K-weighting's gain at 1 kHz, the −0.691 LU offset that cancels it, the
/// channel summation (two unity-weighted channels are +3.01 dB over one), the
/// peak-versus-RMS reading of the specification, and the block machinery.
#[test]
fn tech_3341_case_1_a_tone_reads_its_own_level() {
    assert_case(
        "case 1 (−23 dBFS, 20 s)",
        -23.0,
        &[Segment {
            seconds: 20.0,
            dbfs: -23.0,
        }],
    );
}

/// **EBU Tech 3341 case 2**: the same tone ten decibels quieter must measure
/// ten LU quieter. Linearity, which a gate applied at the wrong point would
/// break.
#[test]
fn tech_3341_case_2_the_measurement_is_linear_in_level() {
    assert_case(
        "case 2 (−33 dBFS, 20 s)",
        -33.0,
        &[Segment {
            seconds: 20.0,
            dbfs: -33.0,
        }],
    );
}

/// **EBU Tech 3341 case 3**: −36 dBFS for 10 s, −23 dBFS for 60 s, −36 dBFS
/// for 10 s must measure −23.0 LUFS.
///
/// This is the **relative gate**: the −36 dBFS passages are 13 LU below the
/// programme, so the gate removes them. Without it the answer would be pulled
/// roughly half a decibel low by twenty seconds of quiet.
#[test]
fn tech_3341_case_3_the_relative_gate_removes_quiet_passages() {
    assert_case(
        "case 3 (−36/−23/−36 dBFS)",
        -23.0,
        &[
            Segment {
                seconds: 10.0,
                dbfs: -36.0,
            },
            Segment {
                seconds: 60.0,
                dbfs: -23.0,
            },
            Segment {
                seconds: 10.0,
                dbfs: -36.0,
            },
        ],
    );
}

/// **EBU Tech 3341 case 4**: −72 dBFS for 10 s, −23 dBFS for 60 s, −72 dBFS
/// for 10 s must measure −23.0 LUFS.
///
/// The **absolute gate**: the quiet passages are below −70 LUFS, so they are
/// removed before the relative threshold is even computed. The distinction
/// from case 3 matters — an implementation that applied only the relative gate
/// would pass case 3 and this one, and one that applied only the absolute gate
/// would pass this one and fail case 3.
#[test]
fn tech_3341_case_4_the_absolute_gate_removes_near_silence() {
    assert_case(
        "case 4 (−72/−23/−72 dBFS)",
        -23.0,
        &[
            Segment {
                seconds: 10.0,
                dbfs: -72.0,
            },
            Segment {
                seconds: 60.0,
                dbfs: -23.0,
            },
            Segment {
                seconds: 10.0,
                dbfs: -72.0,
            },
        ],
    );
}

/// **EBU Tech 3341 case 5**: −26 dBFS for 20 s, −20 dBFS for 20.1 s, −26 dBFS
/// for 20 s must measure −23.0 LUFS.
///
/// Nothing is gated here — every passage is within 10 LU of the mean — so this
/// is the case that checks the *averaging* is over power rather than over
/// decibels. Averaging the block loudnesses instead of their mean squares
/// gives −23.9, nearly a decibel out, and passes every other case in this
/// file.
#[test]
fn tech_3341_case_5_the_average_is_over_power_not_decibels() {
    assert_case(
        "case 5 (−26/−20/−26 dBFS)",
        -23.0,
        &[
            Segment {
                seconds: 20.0,
                dbfs: -26.0,
            },
            Segment {
                seconds: 20.1,
                dbfs: -20.0,
            },
            Segment {
                seconds: 20.0,
                dbfs: -26.0,
            },
        ],
    );
}

/// A mono source must be measured as **one** channel, not as the stereo pair
/// baz's decoder duplicates it into — and the difference is exactly 3.01 LU.
///
/// BS.1770 sums the channels with unity weights, so a duplicated mono file
/// carries twice the power and reads 10·log₁₀(2) = 3.0103 LU louder. That is
/// arithmetically correct and is *not* the number to store: every other
/// scanner (`rsgain`, `loudgain`, foobar2000 — all of them libebur128 told the
/// file's own channel count) writes the one-channel figure, and a library in
/// which baz's computed mono tracks sat 3 dB away from the tagged ones would
/// be a library that jumps in level for a reason no listener could see.
///
/// This test pins both halves: the exact offset, and which side of it baz
/// takes. [`baz_core::analysis`] is the code that has to pass the source's own
/// count, and `LoudnessMeter::new` is where it says so.
#[test]
fn a_mono_source_is_measured_as_one_channel() {
    let rate = 44_100;
    let stereo = tone(
        rate,
        &[Segment {
            seconds: 5.0,
            dbfs: -23.0,
        }],
    );
    let mono: Vec<f32> = stereo.iter().step_by(2).copied().collect();

    let mut meter = LoudnessMeter::new(rate, 1).expect("mono meter");
    meter.push(&mono);
    let as_mono = meter.finish().integrated_lufs().expect("a measurement");
    let as_duplicate = measure(rate, &stereo)
        .integrated_lufs()
        .expect("a measurement");

    let doubling = 10f64 * 2f64.log10();
    assert!(
        ((as_duplicate - as_mono) - doubling).abs() < 1e-6,
        "duplicating a mono channel must be exactly {doubling} LU: \
         mono {as_mono}, duplicated {as_duplicate}"
    );
    // And the one-channel reading is the tone's own level minus that same
    // doubling — so the offset above is not two errors agreeing with each
    // other.
    assert!(
        (as_mono - (-23.0 - doubling)).abs() <= TOLERANCE_LU,
        "{as_mono}"
    );
}

/// An album's loudness is the gated loudness of its tracks' blocks **pooled**,
/// which is not the average of the tracks' own answers.
///
/// A loud track and a track with a long quiet passage: measured together, the
/// album's relative gate is computed across both and removes the quiet
/// passage; averaging the two tracks' own figures would keep it, because each
/// track's gate saw only its own material. The two answers differ by more than
/// the tolerance, which is what makes this a test rather than a restatement.
#[test]
fn an_album_figure_pools_the_blocks_rather_than_averaging_the_tracks() {
    let rate = 48_000;
    let loud = measure(
        rate,
        &tone(
            rate,
            &[Segment {
                seconds: 20.0,
                dbfs: -20.0,
            }],
        ),
    );
    let quiet = measure(
        rate,
        &tone(
            rate,
            &[Segment {
                seconds: 20.0,
                dbfs: -40.0,
            }],
        ),
    );

    let per_track: Vec<f64> = [&loud, &quiet]
        .iter()
        .map(|t| t.integrated_lufs().expect("a measurement"))
        .collect();
    let mean_of_answers = f64::midpoint(per_track[0], per_track[1]);
    let album = album_lufs([&loud, &quiet]).expect("an album measurement");

    // The quiet track is 20 LU below the loud one, so the album's relative
    // gate removes it entirely and the album measures as the loud track does.
    assert!(
        (album - per_track[0]).abs() <= TOLERANCE_LU,
        "album {album} should follow the programme ({}), not the arithmetic mean {mean_of_answers}",
        per_track[0]
    );
    assert!(
        (album - mean_of_answers).abs() > 1.0,
        "the fixture must be able to tell the two rules apart"
    );
}
