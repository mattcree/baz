//! Fuzz the history ledger's line reader (ADR-0016): arbitrary bytes must
//! produce `Some` or `None`, never a panic, and anything accepted must
//! round-trip back to the exact same line.
//!
//! The ledger is a file a user may edit, concatenate from backups, or have
//! truncated by a crash, so its reader parses external bytes and gets a target
//! like every other one (docs/ENGINEERING.md).
#![no_main]

use baz_core::history::PlayRecord;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(line) = std::str::from_utf8(data) else {
        return;
    };
    if let Some(record) = PlayRecord::from_line(line) {
        let encoded = record.to_line();
        let back = PlayRecord::from_line(&encoded).expect("re-encoded line must parse");
        assert_eq!(record, back);
        // A record is exactly one line: nothing a path can contain may add a
        // separator or a line break to the file.
        assert_eq!(encoded.matches('\n').count(), 1);
        assert_eq!(encoded.matches('\t').count(), 4);
    }
});
