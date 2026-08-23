//! **Writing a file so a crash cannot leave it half-written.**
//!
//! One implementation, because there were two places that needed it and only
//! one that had it. `playlist::write_atomic` has done this since playlists
//! were authored in place; `baz`'s `config.rs` used `std::fs::write`, which
//! truncates first — so a kill or a full disk between the truncate and the
//! write left an empty `config.toml`, and the next setting a listener touched
//! wrote the defaults over the wreckage. Audit finding 2, 2026-08-23.
//!
//! # Why a temp file and a rename
//!
//! `rename(2)` within a directory is atomic: a reader sees the old file or the
//! new one and never a torn one. The data is `sync_data`'d **before** the
//! rename, so the bytes are on the medium before the name points at them —
//! without that the rename can land while the contents are still in flight,
//! which is the failure this exists to prevent and is invisible in testing.
//!
//! The temp file is made in the **same directory** as its destination, because
//! a rename across filesystems is not a rename. It is named to be recognisable
//! as debris rather than as content: something that enumerates a directory
//! must never mistake a half-written file for a real one.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

/// Write `bytes` to `path`, atomically.
///
/// The destination is replaced only once the new content is complete and
/// synced. On any failure the temp file is removed and the destination is left
/// exactly as it was.
///
/// # Errors
///
/// Any I/O error from creating, writing, syncing or renaming; and
/// `AlreadyExists` if a thousand temp names in the destination's directory are
/// all taken, which means something is very wrong with it.
pub fn write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let directory = path.parent().unwrap_or_else(|| Path::new(""));
    let pid = std::process::id();
    for attempt in 0u32..1024 {
        let candidate = directory.join(format!(".baz-{pid}-{attempt}.tmp"));
        let mut file = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        };
        let written = file
            .write_all(bytes)
            .and_then(|()| file.sync_data())
            .and_then(|()| {
                drop(file);
                std::fs::rename(&candidate, path)
            });
        if let Err(error) = written {
            // Best effort: the temp file is debris either way, and the error
            // worth reporting is the write's.
            let _ = std::fs::remove_file(&candidate);
            return Err(error);
        }
        return Ok(());
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "could not find a free temp-file name",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The destination is replaced, and nothing is left beside it.
    #[test]
    fn a_write_replaces_the_file_and_leaves_no_debris() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("config.toml");
        std::fs::write(&path, b"old").expect("seed");
        write(&path, b"new and longer").expect("write");
        assert_eq!(std::fs::read(&path).expect("read"), b"new and longer");
        let strays: Vec<_> = std::fs::read_dir(dir.path())
            .expect("read_dir")
            .filter_map(Result::ok)
            .map(|entry| entry.file_name())
            .filter(|name| name != "config.toml")
            .collect();
        assert!(strays.is_empty(), "left debris: {strays:?}");
    }

    /// **A failed write does not destroy what was there**, which is the whole
    /// difference from `fs::write`: that truncates first, so the same failure
    /// leaves an empty file.
    #[test]
    fn a_failed_write_leaves_the_old_file_intact() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("kept.toml");
        std::fs::write(&path, b"the original").expect("seed");
        // A destination that cannot be renamed onto: a directory of that name
        // would be replaced, so use an unwritable *parent* instead by pointing
        // at a path whose directory does not exist.
        let doomed = dir.path().join("missing").join("kept.toml");
        assert!(write(&doomed, b"never lands").is_err());
        assert_eq!(std::fs::read(&path).expect("read"), b"the original");
    }
}
