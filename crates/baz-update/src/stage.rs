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
//! **user-writable cache directory** and let a session end. Anything on the
//! machine could have touched the file in between; the launcher that picks it
//! up is about to hand it to `msiexec`. So the digest travels in the marker
//! and [`ready`] hashes the file again before it is offered. Hashing 190 MB
//! costs a fraction of a second and happens only on the one launch where an
//! update is actually waiting.
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

use std::path::PathBuf;

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
    // An older stage is not merged with a newer one — it is replaced, so a
    // superseded installer never sits beside a current one waiting to be
    // picked by a name comparison.
    discard();
    std::fs::create_dir_all(&dir).map_err(|error| format!("{}: {error}", dir.display()))?;
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
    let text = std::fs::read_to_string(dir()?.join(MARKER)).ok()?;
    parse(&text)
}

/// **The staged installer, if it is still exactly what was staged.**
///
/// `None` covers every way a stage can have gone bad — the file is missing,
/// it was truncated by a session that died mid-write, something rewrote it —
/// and every one of them means *do not offer this*.
#[must_use]
pub fn ready(pending: &Pending) -> Option<PathBuf> {
    let path = dir()?.join(&pending.file_name);
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
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[cfg(test)]
mod tests {
    use super::{Pending, marker, parse};

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
}
