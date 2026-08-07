//! Fuzz the playback decoder wrapper: arbitrary bytes fed through the
//! Symphonia probe/decode path must produce `Ok` or `Err`, never a panic.
//! Sibling of `protocol_deserialize.rs` per docs/ENGINEERING.md: every
//! parser that touches external file bytes gets a fuzz target — media
//! parsers process hostile input.
#![no_main]

use baz_core::playback::AudioSource;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(mut src) = AudioSource::open_bytes(data.to_vec()) else {
        return;
    };
    // Bound the work per input: enough blocks to exercise mid-stream decode
    // and the trim window without letting a crafted header demand unbounded
    // output.
    for _ in 0..64 {
        match src.next_block() {
            Ok(Some(_)) => {}
            Ok(None) | Err(_) => break,
        }
    }
});
