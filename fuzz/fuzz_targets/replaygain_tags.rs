//! Fuzz the pure ReplayGain tag parser and the gain selection rule: arbitrary
//! key/value pairs must never panic, and the documented invariants must hold —
//! parsed figures stay inside their declared ranges, and no resolved gain is
//! ever a value the engine could not multiply by.
//!
//! ReplayGain tags are attacker-supplied text sitting inside media files, so
//! per `docs/ENGINEERING.md` they get a target like every other parser that
//! touches file bytes. Sibling of `scanner_inference.rs`, and the reason
//! `baz_core::replaygain` is written as pure functions over `&str` with no
//! decoder, no file and no engine behind them.
#![no_main]

use baz_core::protocol::ReplayGainMode;
use baz_core::replaygain::{
    MAX_APPLIED_CENTIDB, MAX_PREAMP_CENTIDB, MAX_TAG_GAIN_CENTIDB, MAX_TAG_PEAK_MICRO,
    ReplayGainSettings, ReplayGainTags, field_of_key, parse_gain, parse_peak, parse_r128_gain,
};
use libfuzzer_sys::fuzz_target;

/// Split the input into NUL-separated fields, so one fuzz case can carry a
/// whole tag block rather than a single value.
fn fields(text: &str) -> Vec<&str> {
    text.split('\0').collect()
}

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let fields = fields(text);

    // Every value parser is total, and every value it does accept is inside
    // the range its documentation promises.
    for field in &fields {
        let _ = field_of_key(field);
        if let Some(gain) = parse_gain(field) {
            assert!(
                gain.abs() <= MAX_TAG_GAIN_CENTIDB,
                "gain {gain} is outside the accepted range"
            );
        }
        if let Some(peak) = parse_peak(field) {
            assert!(
                peak <= MAX_TAG_PEAK_MICRO,
                "peak {peak} is outside the accepted range"
            );
        }
        if let Some(gain) = parse_r128_gain(field) {
            assert!(
                gain.abs() <= MAX_TAG_GAIN_CENTIDB,
                "r128 gain {gain} is outside the accepted range"
            );
        }
    }

    // Assembling a whole tag block, keys and values interleaved, is also total.
    let pairs: Vec<(&str, &str)> = fields
        .chunks(2)
        .map(|pair| (pair[0], pair.get(1).copied().unwrap_or("")))
        .collect();
    let tags = ReplayGainTags::from_pairs(pairs.iter().copied());
    assert_eq!(tags.is_empty(), tags == ReplayGainTags::default());

    // And the selection rule is total over whatever those tags turned out to
    // be, for every mode and both pre-amps taken from the input itself.
    let preamp = i16::from_le_bytes([
        data.first().copied().unwrap_or(0),
        data.get(1).copied().unwrap_or(0),
    ]);
    for mode in [
        ReplayGainMode::Off,
        ReplayGainMode::Track,
        ReplayGainMode::Album,
    ] {
        for prevent_clipping in [false, true] {
            let settings = ReplayGainSettings::new(mode, preamp, preamp, prevent_clipping);
            assert!(settings.preamp_centidb.abs() <= MAX_PREAMP_CENTIDB);
            assert!(settings.no_tag_preamp_centidb.abs() <= MAX_PREAMP_CENTIDB);
            let decision = settings.resolve(tags);
            assert!(
                decision.gain_centidb <= MAX_APPLIED_CENTIDB,
                "{decision:?} exceeds the applied ceiling"
            );
            let amplitude = decision.amplitude();
            assert!(
                amplitude.is_finite() && amplitude > 0.0,
                "{decision:?} resolved to an unusable gain: {amplitude}"
            );
            assert_eq!(decision.is_transparent(), decision.gain_centidb == 0);
            if mode == ReplayGainMode::Off {
                assert!(decision.is_transparent(), "off must never touch a sample");
            }
        }
    }
});
