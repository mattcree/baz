//! Fuzz the protocol deserializer: arbitrary bytes must produce `Ok` or `Err`,
//! never a panic, and anything accepted must round-trip. Every future parser
//! that touches external bytes gets a sibling target (docs/ENGINEERING.md).
#![no_main]

use baz_core::protocol::Command;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(cmd) = serde_json::from_slice::<Command>(data) {
        let json = serde_json::to_vec(&cmd).expect("accepted command must reserialize");
        let back: Command = serde_json::from_slice(&json).expect("round-trip must parse");
        assert_eq!(cmd, back);
    }
});
