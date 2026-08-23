//! **Where a downloaded update waits for the next launch.**
//!
//! ADR-0043 §5. The download and the offer happen in different processes,
//! minutes or days apart, so between them the update has to live somewhere on
//! disk with enough beside it to be trusted when it is picked up again.
//!
//! # Why the digest is written down and checked twice
//!
//! [`crate::fetch_verified`] already proved these bytes are the ones published
//! beside the release's own `SHA256SUMS`, and then it put them in a
//! **user-writable cache directory** and let a session end. So the digest
//! travels in the marker and [`ready`] hashes the file again before it is
//! offered. Hashing 190 MB costs a fraction of a second and happens only on
//! the one launch where an update is actually waiting.
//!
//! # What the second check proves, and what it does not
//!
//! **It is an integrity check, not a provenance one**, and this module said
//! otherwise until 2026-08-23 — *"anything on the machine could have touched
//! the file in between"*, as though re-hashing answered that. It does not.
//! The digest is read back out of **the same user-writable directory as the
//! payload**, so whatever could rewrite the installer could rewrite the marker
//! beside it, and the pair would still agree. `ready` would offer it, and on
//! Windows the launcher hands it to `msiexec` with a per-machine scope and an
//! elevation prompt.
//!
//! What it does catch is every way a stage goes bad on its own: a download
//! truncated by a session that was killed mid-write, a cache a disk corrupted,
//! a partial copy. Those are the realistic failures and it is worth having for
//! them — `a_payload_that_no_longer_matches_its_marker_is_refused`.
//!
//! **The gap is pinned as a test**
//! (`a_rewritten_pair_is_offered_because_the_digest_has_no_provenance`) rather
//! than described here, so it stays true. Closing it needs one of two things
//! that cost money or privilege: a signed installer, so `msiexec` checks a
//! publisher this file cannot forge, or a staging directory an unprivileged
//! process cannot write. ADR-0043 §4 defers signing on cost; until one of them
//! lands, nothing here may tell a listener the file has been *verified* at the
//! moment it is offered — only that it was verified when it was downloaded.
//!
//! That second check is also what makes a *stale* stage harmless rather than
//! dangerous: a truncated download from a session that was killed mid-write
//! fails the digest and is discarded, and the next session stages it again.
//!
//! # Why the cache directory rather than a temporary one
//!
//! It has to survive a reboot, because *the next launch* is the whole point
//! and a listener may well shut the machine down in between. It also has to
//! be somewhere the system may reclaim without breaking anything, because an
//! update nobody accepted should not sit in a backup forever. The cache
//! directory is the one place with both properties.

use std::path::{Path, PathBuf};

/// The marker file's name, beside the installer it describes.
const MARKER: &str = "pending.toml";

/// **An update that has been downloaded, verified and left for the launcher.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pending {
    /// The version, as the release tag names it.
    pub version: String,
    /// The installer's file name, inside [`dir`].
    pub file_name: String,
    /// Its SHA-256, as it was proved at download time.
    pub sha256: String,
}

/// **Where a staged update lives**, or `None` on a platform with no cache
/// directory — where the answer is simply that nothing is ever staged.
#[must_use]
pub fn dir() -> Option<PathBuf> {
    Some(dirs::cache_dir()?.join("baz").join("update"))
}

/// **The marker, as it is written.**
///
/// Its own function so the reader below has something to be tested against
/// rather than a format described in a comment.
#[must_use]
pub fn marker(pending: &Pending) -> String {
    let mut table = toml::Table::new();
    table.insert("version".into(), pending.version.clone().into());
    table.insert("file".into(), pending.file_name.clone().into());
    table.insert("sha256".into(), pending.sha256.clone().into());
    format!(
        "# Written by baz. An update it has already downloaded and checked,\n\
         # waiting for the next launch to offer it. Deleting this file and the\n\
         # installer beside it cancels the offer and costs nothing.\n{table}"
    )
}

/// **The marker, as it is read.**
///
/// Every field or nothing: a marker missing one of them describes an update
/// that cannot be verified or cannot be found, and a half-read stage is worse
/// than no stage.
#[must_use]
pub fn parse(text: &str) -> Option<Pending> {
    let table: toml::Table = text.parse().ok()?;
    let field = |key: &str| table.get(key)?.as_str().map(str::to_owned);
    let pending = Pending {
        version: field("version")?,
        file_name: field("file")?,
        sha256: field("sha256")?,
    };
    // **A file name is a name, never a path.** It is joined onto a directory
    // and then handed to an installer, so a separator or a `..` in it would
    // be this code being told where to point `msiexec`.
    if pending.file_name.is_empty()
        || pending.file_name.contains('/')
        || pending.file_name.contains('\\')
        || pending.file_name.contains("..")
    {
        return None;
    }
    if pending.sha256.len() != 64 || !pending.sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    Some(pending)
}

/// **Write the installer and its marker**, replacing whatever was there.
///
/// The installer is written first and the marker last, because the marker is
/// what makes the stage real: a session killed between them leaves bytes with
/// nothing pointing at them, which the next download overwrites.
///
/// # Errors
///
/// A platform with no cache directory, or one that will not take the file.
pub fn hold(pending: &Pending, bytes: &[u8]) -> Result<PathBuf, String> {
    let dir = dir().ok_or_else(|| "this platform has no cache directory".to_owned())?;
    hold_in(&dir, pending, bytes)
}

/// [`hold`], into a directory the caller names.
///
/// The seam the round trip is tested through. Every function here reached the
/// staging directory by calling [`dir`], which is `dirs::cache_dir()` — so the
/// whole hold → pending → ready → discard cycle could only be exercised by
/// writing into the real user's cache, and it therefore was not exercised at
/// all: `hold`, `pending`, `ready` and `discard` had no test callers between
/// them (audit finding 6, 2026-08-23).
///
/// # Errors
///
/// Any I/O error creating the directory or writing the payload or the marker.
pub fn hold_in(dir: &Path, pending: &Pending, bytes: &[u8]) -> Result<PathBuf, String> {
    // An older stage is not merged with a newer one — it is replaced, so a
    // superseded installer never sits beside a current one waiting to be
    // picked by a name comparison.
    discard_in(dir);
    std::fs::create_dir_all(dir).map_err(|error| format!("{}: {error}", dir.display()))?;
    let path = dir.join(&pending.file_name);
    std::fs::write(&path, bytes).map_err(|error| format!("{}: {error}", path.display()))?;
    let marker = dir.join(MARKER);
    std::fs::write(&marker, marker_bytes(pending))
        .map_err(|error| format!("{}: {error}", marker.display()))?;
    Ok(path)
}

/// [`marker`] as bytes, which is the only shape [`hold`] wants it in.
fn marker_bytes(pending: &Pending) -> Vec<u8> {
    marker(pending).into_bytes()
}

/// **What is waiting, if anything.**
#[must_use]
pub fn pending() -> Option<Pending> {
    pending_in(&dir()?)
}

/// [`pending`], from a directory the caller names.
#[must_use]
pub fn pending_in(dir: &Path) -> Option<Pending> {
    parse(&std::fs::read_to_string(dir.join(MARKER)).ok()?)
}

/// **The staged installer, if it is still exactly what was staged.**
///
/// `None` covers every way a stage can have gone bad — the file is missing,
/// it was truncated by a session that died mid-write, something rewrote it —
/// and every one of them means *do not offer this*.
#[must_use]
pub fn ready(pending: &Pending) -> Option<PathBuf> {
    ready_in(&dir()?, pending)
}

/// [`ready`], in a directory the caller names.
#[must_use]
pub fn ready_in(dir: &Path, pending: &Pending) -> Option<PathBuf> {
    let path = dir.join(&pending.file_name);
    let bytes = std::fs::read(&path).ok()?;
    crate::digest_matches(&bytes, &pending.sha256).then_some(path)
}

/// **Throw the stage away.**
///
/// Best effort by design: this is called when an offer is declined and when
/// the staged version is no longer newer than the running one, and a cache
/// directory that will not delete is not a thing to tell a listener about.
pub fn discard() {
    if let Some(dir) = dir() {
        discard_in(&dir);
    }
}

/// [`discard`], for a directory the caller names.
pub fn discard_in(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
}

#[cfg(test)]
mod tests {
    use super::{Pending, discard_in, hold_in, marker, parse, pending_in, ready_in};

    fn sample() -> Pending {
        Pending {
            version: "v0.4.2".into(),
            file_name: "baz-0.4.2-windows-x86_64.msi".into(),
            sha256: "d2a84f4b8b650937ec8f73cd8be2c74add5a911ba64df27458ed8229da804a26".into(),
        }
    }

    /// **What is written is what is read**, which is the only promise the two
    /// halves of the stage make to each other across a reboot.
    #[test]
    fn a_marker_survives_being_written_and_read_back() {
        let pending = sample();
        assert_eq!(parse(&marker(&pending)).as_ref(), Some(&pending));
    }

    /// **A file name is a name.** It is joined onto a directory and handed to
    /// an installer, so anything that could point that installer somewhere
    /// else is refused outright rather than sanitised — a sanitiser is a
    /// thing to be outwitted, and there is no legitimate marker this rejects.
    #[test]
    fn a_marker_naming_a_path_is_not_a_marker() {
        for name in [
            "../../../etc/cron.d/baz",
            "sub/baz.msi",
            "sub\\baz.msi",
            "..",
            "",
        ] {
            let mut pending = sample();
            pending.file_name = name.into();
            assert_eq!(parse(&marker(&pending)), None, "{name} was accepted");
        }
    }

    /// **A digest that is not a digest cancels the stage**, because the whole
    /// value of the marker is the thing it lets the launcher prove.
    #[test]
    fn a_marker_without_a_full_digest_is_refused() {
        for digest in ["", "dead", &"0".repeat(63), &format!("{}z", "0".repeat(63))] {
            let mut pending = sample();
            pending.sha256 = digest.into();
            assert_eq!(parse(&marker(&pending)), None, "{digest} was accepted");
        }
    }

    /// **Every field or nothing.** A marker missing one describes an update
    /// that cannot be found or cannot be verified.
    #[test]
    fn a_half_written_marker_is_no_marker() {
        for text in [
            "",
            "version = \"v0.4.2\"",
            "file = \"baz.msi\"\nsha256 = \"\"",
            "not toml at all {{{",
        ] {
            assert_eq!(parse(text), None, "{text:?} was accepted");
        }
    }

    /// **The whole cycle, in a directory of its own.** Audit finding 6.
    ///
    /// `hold` → `pending` → `ready` → `discard` had no test callers between
    /// them, because every one of them found its directory through
    /// `dirs::cache_dir()` and exercising them meant writing into the real
    /// user's cache. This is the round trip that could not be written.
    #[test]
    fn a_stage_survives_being_written_and_picked_up_again() {
        let dir = tempfile::tempdir().expect("tempdir");
        let bytes = b"an installer, near enough".as_slice();
        let mut waiting = sample();
        waiting.sha256 = sha256_of(bytes);

        let path = hold_in(dir.path(), &waiting, bytes).expect("hold");
        assert!(path.exists(), "the payload is where hold said it is");

        let read_back = pending_in(dir.path()).expect("a marker is waiting");
        assert_eq!(read_back.version, waiting.version);
        assert_eq!(read_back.sha256, waiting.sha256);
        assert_eq!(
            ready_in(dir.path(), &read_back).as_deref(),
            Some(path.as_path()),
            "an untouched stage is offered"
        );

        discard_in(dir.path());
        assert!(
            pending_in(dir.path()).is_none(),
            "and discard leaves nothing"
        );
    }

    /// **A payload that changed under us is not offered.**
    ///
    /// This is what the second digest check is *for*: a download truncated by
    /// a session that died mid-write, or a cache a disk corrupted.
    #[test]
    fn a_payload_that_no_longer_matches_its_marker_is_refused() {
        let dir = tempfile::tempdir().expect("tempdir");
        let bytes = b"the whole installer".as_slice();
        let mut waiting = sample();
        waiting.sha256 = sha256_of(bytes);
        let path = hold_in(dir.path(), &waiting, bytes).expect("hold");

        std::fs::write(&path, b"the whole install").expect("truncate it");
        assert!(
            ready_in(dir.path(), &waiting).is_none(),
            "a truncated payload must not be offered"
        );
    }

    /// **And what the second check does *not* prove.**
    ///
    /// The digest is read back out of the same user-writable directory as the
    /// payload, so anything able to rewrite one can rewrite the other and the
    /// pair still agrees. That is provenance the check does not give, the
    /// module said it did until 2026-08-23, and the fix is a signature or a
    /// directory the user cannot write — neither of which is free. Pinned as
    /// a test so the limitation is a fact in the suite rather than a sentence
    /// in a comment.
    #[test]
    fn a_rewritten_pair_is_offered_because_the_digest_has_no_provenance() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut waiting = sample();
        waiting.sha256 = sha256_of(b"the published installer");
        hold_in(dir.path(), &waiting, b"the published installer").expect("hold");

        // Stand in for anything with write access to the cache.
        let substituted = b"something else entirely".as_slice();
        std::fs::write(dir.path().join(&waiting.file_name), substituted).expect("swap");
        let mut forged = waiting.clone();
        forged.sha256 = sha256_of(substituted);
        std::fs::write(dir.path().join(super::MARKER), marker(&forged)).expect("forge");

        let read_back = pending_in(dir.path()).expect("a marker is waiting");
        assert!(
            ready_in(dir.path(), &read_back).is_some(),
            "the pair agrees, so it is offered — this is the gap, not a bug in the test"
        );
    }

    fn sha256_of(bytes: &[u8]) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        hasher
            .finalize()
            .iter()
            .fold(String::new(), |mut acc, byte| {
                use std::fmt::Write as _;
                let _ = write!(acc, "{byte:02x}");
                acc
            })
    }
}
