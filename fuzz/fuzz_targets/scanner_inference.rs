//! Fuzz the pure filename/path inference parser: arbitrary strings must
//! never panic, and the documented invariants must hold — inferred strings
//! are trimmed and non-empty, inferred track numbers are in 1..=999.
//! Sibling of `protocol_deserialize.rs` per docs/ENGINEERING.md: every
//! parser that touches external input gets a fuzz target.
#![no_main]

use std::path::Path;

use baz_core::library::inference::{infer_from_relative_path, parse_filename};
use libfuzzer_sys::fuzz_target;

fn assert_clean(field: Option<&str>) {
    if let Some(s) = field {
        assert!(!s.is_empty(), "inferred strings must be non-empty");
        assert_eq!(s, s.trim(), "inferred strings must be trimmed");
    }
}

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };

    let parsed = parse_filename(text);
    if let Some(track) = parsed.track {
        assert!((1..=999).contains(&track), "track out of range: {track}");
    }
    assert_clean(parsed.title.as_deref());

    let inferred = infer_from_relative_path(Path::new(text));
    if let Some(track) = inferred.track {
        assert!((1..=999).contains(&track), "track out of range: {track}");
    }
    assert_clean(inferred.artist.as_deref());
    assert_clean(inferred.album.as_deref());
    assert_clean(inferred.title.as_deref());
});
