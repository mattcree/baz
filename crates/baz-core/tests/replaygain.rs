//! Integration tests for `baz_core::replaygain`: the tag parser and the gain
//! selection rule, both as pure functions.
//!
//! Everything here is a table. That is deliberate — ADR-0013's whole claim is
//! that ReplayGain's arithmetic is decidable without an audio device, a
//! decoder, or a file, so the arithmetic is tested without one. What the
//! *engine* does with the answer is `tests/engine.rs`, asserted on samples;
//! what a real container yields is `tests/scanner.rs`, asserted on files.
//!
//! The parser half doubles as the specification the fuzz target
//! (`fuzz/fuzz_targets/replaygain_tags.rs`) checks invariants against: tags are
//! untrusted input, and "never panics, never returns a value outside its
//! documented range" is the property, not "handles the inputs we thought of".

use baz_core::protocol::{ReplayGainMode, ReplayGainSource};
use baz_core::replaygain::{
    MAX_APPLIED_CENTIDB, MAX_PREAMP_CENTIDB, MAX_TAG_GAIN_CENTIDB, MAX_TAG_PEAK_MICRO, PEAK_UNITY,
    R128_REFERENCE_OFFSET_CENTIDB, ReplayGainDecision, ReplayGainField, ReplayGainSettings,
    ReplayGainTags, field_of_key, parse_gain, parse_peak, parse_r128_gain,
};

// ---------------------------------------------------------------------------
// Key recognition
// ---------------------------------------------------------------------------

/// Every spelling the four containers baz reads actually put on disk resolves
/// to the same field. One test, because the whole point of `field_of_key` is
/// that there is one answer rather than one per container.
#[test]
fn every_container_spelling_names_the_same_field() {
    let cases: &[(&str, ReplayGainField)] = &[
        // Vorbis comments (FLAC, Ogg) — the canonical spelling.
        ("REPLAYGAIN_TRACK_GAIN", ReplayGainField::TrackGain),
        ("REPLAYGAIN_TRACK_PEAK", ReplayGainField::TrackPeak),
        ("REPLAYGAIN_ALBUM_GAIN", ReplayGainField::AlbumGain),
        ("REPLAYGAIN_ALBUM_PEAK", ReplayGainField::AlbumPeak),
        // Vorbis comments are case-insensitive by specification, and taggers
        // have written every casing there is.
        ("replaygain_track_gain", ReplayGainField::TrackGain),
        ("ReplayGain_Album_Peak", ReplayGainField::AlbumPeak),
        // A space where the convention has an underscore.
        ("REPLAYGAIN TRACK GAIN", ReplayGainField::TrackGain),
        // MP4 freeform atoms: lofty's spelling and Symphonia's, which differ
        // only in whether the `----` box name is kept.
        (
            "----:com.apple.iTunes:replaygain_track_gain",
            ReplayGainField::TrackGain,
        ),
        (
            "com.apple.iTunes:replaygain_album_peak",
            ReplayGainField::AlbumPeak,
        ),
        (
            "----:com.apple.iTunes:REPLAYGAIN_ALBUM_GAIN",
            ReplayGainField::AlbumGain,
        ),
        // Symphonia names an ID3v2 user-defined frame after its description.
        ("TXXX:REPLAYGAIN_TRACK_GAIN", ReplayGainField::TrackGain),
        ("TXXX:replaygain_album_gain", ReplayGainField::AlbumGain),
        // The Opus-style integer form, which turns up in Vorbis comments on
        // files that are not Opus.
        ("R128_TRACK_GAIN", ReplayGainField::R128TrackGain),
        ("r128_album_gain", ReplayGainField::R128AlbumGain),
    ];
    for (key, want) in cases {
        assert_eq!(field_of_key(key), Some(*want), "{key}");
    }
}

/// Neighbouring keys are *not* ReplayGain, and near-misses are not rounded to
/// the nearest match. A file carrying `REPLAYGAIN_REFERENCE_LOUDNESS` — which
/// every scanner writes — must not have it read as a gain.
#[test]
fn keys_that_are_not_replay_gain_are_rejected() {
    for key in [
        "REPLAYGAIN_REFERENCE_LOUDNESS",
        "REPLAYGAIN_TRACK_GAIN_",
        "REPLAYGAIN_TRACK",
        "TRACK_GAIN",
        "ALBUM_GAIN",
        "R128_GAIN",
        "R128_TRACK_PEAK",
        "ARTIST",
        "",
        ":",
        "----:com.apple.iTunes:iTunNORM",
    ] {
        assert_eq!(field_of_key(key), None, "{key} is not a ReplayGain key");
    }
}

// ---------------------------------------------------------------------------
// Value parsing
// ---------------------------------------------------------------------------

/// The conventional spellings a gain is written in, and the one number they
/// all mean.
#[test]
fn gains_parse_from_every_spelling_in_the_wild() {
    for text in [
        "-7.75 dB",
        "-7.75dB",
        "-7.75 DB",
        "-7.75",
        " -7.75 dB ",
        "-7.750000 dB",
    ] {
        assert_eq!(parse_gain(text), Some(-775), "{text:?}");
    }
    assert_eq!(parse_gain("+2.34 dB"), Some(234));
    assert_eq!(parse_gain("0.00 dB"), Some(0));
    assert_eq!(parse_gain("-0.00 dB"), Some(0), "negative zero is zero");
    assert_eq!(parse_gain("12 dB"), Some(1_200));
    // Rounded to the nearest centidecibel, which is finer than the convention
    // writes.
    assert_eq!(parse_gain("-3.615 dB"), Some(-362));
    assert_eq!(parse_gain("-3.614 dB"), Some(-361));
}

/// Malformed and hostile gains are `None` — never a saturated number that
/// would then be applied to somebody's speakers.
#[test]
fn malformed_gains_are_declined_rather_than_guessed_at() {
    for text in [
        "",
        " ",
        "dB",
        " dB",
        "loud",
        "-7.75 dBFS",
        "--7.75",
        "7,75 dB",
        "NaN",
        "nan dB",
        "inf",
        "-inf dB",
        "infinity",
        "1e30",
        "-1e30 dB",
        "1e400",
        "0x10",
        "\u{2212}7.75 dB", // a Unicode minus is not an ASCII one
    ] {
        assert_eq!(parse_gain(text), None, "{text:?} must not parse");
    }
    // Absurd but finite: beyond the accepted range, so still "the file did
    // not say".
    let over = f64::from(MAX_TAG_GAIN_CENTIDB) / 100.0 + 0.01;
    assert_eq!(parse_gain(&format!("{over} dB")), None);
    assert_eq!(parse_gain(&format!("-{over} dB")), None);
    // The edge itself is accepted.
    let edge = f64::from(MAX_TAG_GAIN_CENTIDB) / 100.0;
    assert_eq!(
        parse_gain(&format!("{edge} dB")),
        Some(MAX_TAG_GAIN_CENTIDB)
    );
}

#[test]
fn peaks_parse_as_linear_amplitudes() {
    assert_eq!(parse_peak("0.988525"), Some(988_525));
    assert_eq!(parse_peak("1.000000"), Some(PEAK_UNITY));
    assert_eq!(parse_peak("1"), Some(PEAK_UNITY));
    assert_eq!(parse_peak(" 0.5 "), Some(500_000));
    assert_eq!(parse_peak("0"), Some(0), "digital silence is a real peak");
    // Above full scale is ordinary for lossy material and is kept.
    assert_eq!(parse_peak("1.234567"), Some(1_234_567));
    // Rounded to the nearest micro-unit.
    assert_eq!(parse_peak("0.9999995"), Some(1_000_000));
    assert_eq!(parse_peak("0.9999994"), Some(999_999));
}

#[test]
fn malformed_peaks_are_declined() {
    for text in [
        "", " ", "-0.5", "-1", "loud", "1.0 dB", "NaN", "inf", "-inf", "1e400",
    ] {
        assert_eq!(parse_peak(text), None, "{text:?} must not parse");
    }
    // A hundred times full scale is not a peak, it is somebody's parse error.
    assert_eq!(parse_peak("101"), None);
    assert_eq!(parse_peak("100"), Some(MAX_TAG_PEAK_MICRO));
}

/// The Opus-style form is Q7.8 fixed point against the EBU R128 reference, so
/// it is both scaled *and* shifted onto ReplayGain's own reference.
#[test]
fn r128_gains_are_scaled_and_shifted_onto_the_replay_gain_reference() {
    // 0/256 dB against −23 LUFS is exactly the +5 dB offset against −18.
    assert_eq!(parse_r128_gain("0"), Some(R128_REFERENCE_OFFSET_CENTIDB));
    // −2321/256 = −9.066 dB, +5 dB = −4.07 dB.
    assert_eq!(parse_r128_gain("-2321"), Some(-407));
    // 256/256 = +1 dB, +5 dB = +6 dB.
    assert_eq!(parse_r128_gain("256"), Some(600));
    assert_eq!(parse_r128_gain(" -512 "), Some(300));
    // A fractional value is accepted (some taggers write one) and rounded.
    assert_eq!(parse_r128_gain("-2321.4"), Some(-407));
    for text in ["", "loud", "NaN", "inf", "1e30", "-1e30"] {
        assert_eq!(parse_r128_gain(text), None, "{text:?}");
    }
}

// ---------------------------------------------------------------------------
// Assembling a file's tags
// ---------------------------------------------------------------------------

/// The four figures, read out of the pairs a Vorbis comment block yields.
#[test]
fn a_fully_tagged_file_reads_back_all_four_figures() {
    let tags = ReplayGainTags::from_pairs([
        ("REPLAYGAIN_TRACK_GAIN", "-7.75 dB"),
        ("REPLAYGAIN_TRACK_PEAK", "0.988525"),
        ("REPLAYGAIN_ALBUM_GAIN", "-9.20 dB"),
        ("REPLAYGAIN_ALBUM_PEAK", "1.001221"),
        ("REPLAYGAIN_REFERENCE_LOUDNESS", "89.0 dB"),
        ("ARTIST", "Stan Rogers"),
    ]);
    assert_eq!(
        tags,
        ReplayGainTags {
            track_gain_centidb: Some(-775),
            track_peak_micro: Some(988_525),
            album_gain_centidb: Some(-920),
            album_peak_micro: Some(1_001_221),
        }
    );
    assert!(!tags.is_empty());
}

#[test]
fn a_file_with_no_replay_gain_reads_back_empty() {
    let tags = ReplayGainTags::from_pairs([("ARTIST", "Stan Rogers"), ("ALBUM", "Northwest")]);
    assert!(tags.is_empty());
    assert_eq!(tags, ReplayGainTags::default());
    assert!(ReplayGainTags::from_pairs::<_, &str, &str>([]).is_empty());
}

/// A malformed value does not poison the read: the other three figures survive
/// and the broken one reads as absent. A scan meeting one bad tag in a library
/// must not lose the library.
#[test]
fn one_malformed_value_does_not_take_the_others_with_it() {
    let tags = ReplayGainTags::from_pairs([
        ("REPLAYGAIN_TRACK_GAIN", "very loud indeed"),
        ("REPLAYGAIN_TRACK_PEAK", "0.5"),
        ("REPLAYGAIN_ALBUM_GAIN", "-9.20 dB"),
        ("REPLAYGAIN_ALBUM_PEAK", "-1"),
    ]);
    assert_eq!(
        tags,
        ReplayGainTags {
            track_gain_centidb: None,
            track_peak_micro: Some(500_000),
            album_gain_centidb: Some(-920),
            album_peak_micro: None,
        }
    );
}

/// The dB form wins over the R128 form whichever order they arrive in, and the
/// R128 form is used when it is all a file has.
#[test]
fn the_decibel_form_outranks_the_r128_form_in_either_order() {
    let want = Some(-775);
    let forward = ReplayGainTags::from_pairs([
        ("REPLAYGAIN_TRACK_GAIN", "-7.75 dB"),
        ("R128_TRACK_GAIN", "-2321"),
    ]);
    let reversed = ReplayGainTags::from_pairs([
        ("R128_TRACK_GAIN", "-2321"),
        ("REPLAYGAIN_TRACK_GAIN", "-7.75 dB"),
    ]);
    assert_eq!(forward.track_gain_centidb, want);
    assert_eq!(reversed.track_gain_centidb, want);
    assert_eq!(forward, reversed, "precedence must not depend on tag order");

    let r128_only =
        ReplayGainTags::from_pairs([("R128_TRACK_GAIN", "-2321"), ("R128_ALBUM_GAIN", "-1792")]);
    assert_eq!(r128_only.track_gain_centidb, Some(-407));
    assert_eq!(r128_only.album_gain_centidb, Some(-200));
    assert_eq!(
        r128_only.track_peak_micro, None,
        "the R128 form carries no peak, so there is nothing to clip-check"
    );
}

/// Two comments for one field: the first parseable value wins, which is how
/// every other player reads a duplicated Vorbis comment.
#[test]
fn the_first_parseable_value_for_a_field_wins() {
    let tags = ReplayGainTags::from_pairs([
        ("REPLAYGAIN_TRACK_GAIN", "-7.75 dB"),
        ("REPLAYGAIN_TRACK_GAIN", "-3.00 dB"),
    ]);
    assert_eq!(tags.track_gain_centidb, Some(-775));
    // And an unparseable first value does not block a good second one.
    let recovered = ReplayGainTags::from_pairs([
        ("REPLAYGAIN_TRACK_GAIN", "???"),
        ("REPLAYGAIN_TRACK_GAIN", "-3.00 dB"),
    ]);
    assert_eq!(recovered.track_gain_centidb, Some(-300));
}

// ---------------------------------------------------------------------------
// Gain selection (ADR-0013's rule, as a table)
// ---------------------------------------------------------------------------

/// A well-tagged album track: both gains, both peaks.
fn album_track() -> ReplayGainTags {
    ReplayGainTags {
        track_gain_centidb: Some(-775),
        track_peak_micro: Some(988_525),
        album_gain_centidb: Some(-920),
        album_peak_micro: Some(1_001_221),
    }
}

/// A single downloaded track: track figures only, no album to be relative to.
fn single_track() -> ReplayGainTags {
    ReplayGainTags {
        track_gain_centidb: Some(233),
        track_peak_micro: Some(500_000),
        album_gain_centidb: None,
        album_peak_micro: None,
    }
}

fn settings(mode: ReplayGainMode) -> ReplayGainSettings {
    ReplayGainSettings {
        mode,
        ..ReplayGainSettings::default()
    }
}

/// One row of the selection table: what it demonstrates, the settings, the
/// file's tags, and the decision those two must produce.
type SelectionCase = (
    &'static str,
    ReplayGainSettings,
    ReplayGainTags,
    ReplayGainSource,
    i16,
    bool,
);

/// The selection rule as one table: mode, tags, and the decision.
///
/// Written as data rather than as a dozen tests because the rule *is* a table —
/// ADR-0013 states it as one — and because a reader checking the code against
/// the ADR should be able to read the two side by side. This half is the mode
/// and fallback rules; [`preamp_and_clipping_cases`] is the other half.
fn mode_cases() -> Vec<SelectionCase> {
    vec![
        (
            "off ignores everything, including a pre-amp",
            ReplayGainSettings {
                mode: ReplayGainMode::Off,
                preamp_centidb: 600,
                no_tag_preamp_centidb: -600,
                prevent_clipping: true,
            },
            album_track(),
            ReplayGainSource::Disabled,
            0,
            false,
        ),
        (
            "track mode takes the track gain",
            settings(ReplayGainMode::Track),
            album_track(),
            ReplayGainSource::Track,
            -775,
            false,
        ),
        (
            "album mode takes the album gain",
            settings(ReplayGainMode::Album),
            album_track(),
            ReplayGainSource::Album,
            -920,
            false,
        ),
        (
            "album mode falls back to the track gain when there is no album gain",
            settings(ReplayGainMode::Album),
            single_track(),
            ReplayGainSource::TrackFallback,
            233,
            false,
        ),
        (
            "track mode does NOT fall back to the album gain",
            settings(ReplayGainMode::Track),
            ReplayGainTags {
                track_gain_centidb: None,
                album_gain_centidb: Some(-920),
                ..album_track()
            },
            ReplayGainSource::NoTag,
            0,
            false,
        ),
    ]
}

/// The second part of the same table: what the two pre-amps do to a figure
/// the rows above already showed being chosen. Split from [`mode_cases`] only
/// to keep each function readable — the contract is one table, and
/// [`the_selection_rule_is_the_documented_table`] runs all of it.
fn preamp_cases() -> Vec<SelectionCase> {
    vec![
        (
            "an untagged file gets the no-ReplayGain pre-amp, which is unity by default",
            settings(ReplayGainMode::Track),
            ReplayGainTags::default(),
            ReplayGainSource::NoTag,
            0,
            false,
        ),
        (
            "an untagged file gets whatever the no-ReplayGain pre-amp is set to",
            ReplayGainSettings {
                mode: ReplayGainMode::Album,
                no_tag_preamp_centidb: -350,
                ..ReplayGainSettings::default()
            },
            ReplayGainTags::default(),
            ReplayGainSource::NoTag,
            -350,
            false,
        ),
        (
            "the pre-amp adds to a tagged gain",
            ReplayGainSettings {
                mode: ReplayGainMode::Track,
                preamp_centidb: 300,
                ..ReplayGainSettings::default()
            },
            album_track(),
            ReplayGainSource::Track,
            -475,
            false,
        ),
        (
            "the pre-amp does not apply to an untagged file",
            ReplayGainSettings {
                mode: ReplayGainMode::Track,
                preamp_centidb: 300,
                ..ReplayGainSettings::default()
            },
            ReplayGainTags::default(),
            ReplayGainSource::NoTag,
            0,
            false,
        ),
    ]
}

/// The third part of the same table: the clipping rule. Split from
/// [`preamp_and_clipping_cases`] for readability only; all three run together
/// in [`the_selection_rule_is_the_documented_table`].
fn clipping_cases() -> Vec<SelectionCase> {
    vec![
        (
            "clipping prevention cuts a boost the peak has no room for",
            ReplayGainSettings {
                mode: ReplayGainMode::Track,
                preamp_centidb: 600,
                ..ReplayGainSettings::default()
            },
            // peak 0.5 leaves exactly 6.02 dB of headroom; +2.33 +6.00 = +8.33
            single_track(),
            ReplayGainSource::Track,
            602,
            true,
        ),
        (
            "clipping prevention off lets the full figure through",
            ReplayGainSettings {
                mode: ReplayGainMode::Track,
                preamp_centidb: 600,
                prevent_clipping: false,
                ..ReplayGainSettings::default()
            },
            single_track(),
            ReplayGainSource::Track,
            833,
            false,
        ),
        (
            "clipping prevention never *raises* a gain the peak leaves room for",
            settings(ReplayGainMode::Track),
            single_track(),
            ReplayGainSource::Track,
            233,
            false,
        ),
        (
            "a file with a gain but no peak is applied in full: nothing to check against",
            ReplayGainSettings {
                mode: ReplayGainMode::Track,
                preamp_centidb: 600,
                ..ReplayGainSettings::default()
            },
            ReplayGainTags {
                track_gain_centidb: Some(233),
                track_peak_micro: None,
                ..ReplayGainTags::default()
            },
            ReplayGainSource::Track,
            833,
            false,
        ),
        (
            "a peak of zero is digital silence: nothing to clip",
            ReplayGainSettings {
                mode: ReplayGainMode::Track,
                preamp_centidb: 600,
                ..ReplayGainSettings::default()
            },
            ReplayGainTags {
                track_gain_centidb: Some(233),
                track_peak_micro: Some(0),
                ..ReplayGainTags::default()
            },
            ReplayGainSource::Track,
            833,
            false,
        ),
        (
            "a file already over full scale is attenuated even at a zero gain",
            ReplayGainSettings {
                mode: ReplayGainMode::Track,
                ..ReplayGainSettings::default()
            },
            ReplayGainTags {
                track_gain_centidb: Some(0),
                track_peak_micro: Some(1_122_018), // +1.00 dB
                ..ReplayGainTags::default()
            },
            ReplayGainSource::Track,
            -100,
            true,
        ),
    ]
}

#[test]
fn the_selection_rule_is_the_documented_table() {
    let cases = mode_cases()
        .into_iter()
        .chain(preamp_cases())
        .chain(clipping_cases());
    for (what, settings, tags, source, gain_centidb, clipping_prevented) in cases {
        assert_eq!(
            settings.resolve(tags),
            ReplayGainDecision {
                source,
                gain_centidb,
                clipping_prevented,
            },
            "{what}"
        );
    }
}

/// Album mode clip-checks against the **album** peak, so every track of an
/// album is reduced by the same amount and the level differences the album
/// gain carries survive. Checking each track against its own peak would undo
/// exactly what album mode exists to preserve.
#[test]
fn album_mode_clip_checks_against_the_album_peak() {
    let settings = ReplayGainSettings {
        mode: ReplayGainMode::Album,
        preamp_centidb: 1_000,
        ..ReplayGainSettings::default()
    };
    // Two tracks of one album: the same album gain and album peak, wildly
    // different track peaks.
    let quiet = ReplayGainTags {
        track_gain_centidb: Some(-200),
        track_peak_micro: Some(200_000),
        album_gain_centidb: Some(-100),
        album_peak_micro: Some(900_000),
    };
    let loud = ReplayGainTags {
        track_peak_micro: Some(900_000),
        ..quiet
    };
    let a = settings.resolve(quiet);
    let b = settings.resolve(loud);
    assert_eq!(a, b, "one album, one reduction");
    assert!(a.clipping_prevented);
    assert_eq!(a.source, ReplayGainSource::Album);
    // 1/0.9 is +0.915 dB, floored to a whole centidecibel.
    assert_eq!(a.gain_centidb, 91);

    // With no album peak the track peak is used instead — the honest fallback,
    // and the tracks then legitimately differ.
    let no_album_peak = |tags: ReplayGainTags| ReplayGainTags {
        album_peak_micro: None,
        ..tags
    };
    assert_ne!(
        settings.resolve(no_album_peak(quiet)),
        settings.resolve(no_album_peak(loud))
    );
}

/// Whatever a file or a front end asks for, the applied gain stays inside the
/// documented range and the transparent case stays exactly transparent.
#[test]
fn the_applied_gain_is_always_inside_its_documented_range() {
    let extremes = [
        i16::MIN,
        -MAX_TAG_GAIN_CENTIDB,
        -1,
        0,
        1,
        MAX_TAG_GAIN_CENTIDB,
        i16::MAX,
    ];
    for mode in [
        ReplayGainMode::Off,
        ReplayGainMode::Track,
        ReplayGainMode::Album,
    ] {
        for gain in extremes {
            for preamp in extremes {
                for peak in [None, Some(0), Some(1), Some(PEAK_UNITY), Some(4_000_000)] {
                    let settings =
                        ReplayGainSettings::new(mode, preamp, preamp, peak.is_some_and(|p| p > 0));
                    assert!(settings.preamp_centidb.abs() <= MAX_PREAMP_CENTIDB);
                    let decision = settings.resolve(ReplayGainTags {
                        track_gain_centidb: Some(gain),
                        track_peak_micro: peak,
                        album_gain_centidb: Some(gain),
                        album_peak_micro: peak,
                    });
                    assert!(
                        decision.gain_centidb <= MAX_APPLIED_CENTIDB,
                        "{decision:?} exceeds the applied ceiling"
                    );
                    let amplitude = decision.amplitude();
                    assert!(
                        amplitude.is_finite() && amplitude > 0.0,
                        "{decision:?} is not a usable gain: {amplitude}"
                    );
                    assert_eq!(decision.is_transparent(), decision.gain_centidb == 0);
                }
            }
        }
    }
}

/// Off is off: whatever the file says and whatever the pre-amps are set to,
/// the decision is unity — which is what lets the engine take the same
/// no-arithmetic path it took before ReplayGain existed.
#[test]
fn off_resolves_to_unity_for_every_input() {
    for tags in [ReplayGainTags::default(), album_track(), single_track()] {
        for preamp in [-MAX_PREAMP_CENTIDB, 0, MAX_PREAMP_CENTIDB] {
            let settings = ReplayGainSettings::new(ReplayGainMode::Off, preamp, preamp, preamp > 0);
            let decision = settings.resolve(tags);
            assert_eq!(decision, ReplayGainDecision::UNITY);
            assert!(decision.is_transparent());
        }
    }
}
