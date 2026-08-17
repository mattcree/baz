//! **Which library folders are reachable right now.**
//!
//! The owner: *"we need to show beside songs when they are not available due
//! to the drive not being loaded or being removed."* baz keeps a row for every
//! track it has ever scanned, and it is right to — an unplugged disk or an
//! unmounted share is a **temporary** absence, and the scanner's
//! positive-evidence gates exist precisely so a missing root never prunes the
//! index. But the library said nothing about it: every row looked playable,
//! search returned them, a playlist containing them looked whole, and the only
//! way to find out was to press one and meet a decode failure in the health
//! log, which is the wrong end of the interaction.
//!
//! It matters more here than in most players because the owner's own library
//! lives on an SMB share reached through gvfs, so *every* track is one unmount
//! away from this state — and the same is true of anyone with an external
//! drive.
//!
//! # The hard part was never the mark, and baz already knew
//!
//! Answering *is this file there* per row per frame would be a `stat` per
//! visible track, which is exactly the per-frame filesystem work the wall must
//! never do — and on a dead network mount a single `stat` can block for
//! **seconds**. The backlog entry that asked for this said to design the
//! knowing first.
//!
//! It was already designed. `Shelf::unavailable` is the folders the most
//! recent scan could not walk: cleared at the start of every pass, filled by
//! that pass, and therefore self-correcting when the drive comes back — on the
//! five-minute rescan clock the entry itself nominated. **The first attempt at
//! this shipped a second probe on that same clock before noticing**, which is
//! the kind of duplication that reads as thoroughness and is really a failure
//! to look.
//!
//! So all that is left here is the join, and it is a prefix test rather than a
//! lookup: a row knows its path, the answer is per root, and `starts_with`
//! costs nothing. The index's own `root` column would be the tidier join and
//! would have to be carried through four view models to reach the rows that
//! need it.
//!
//! # What it is careful not to be
//!
//! **Not a prune.** `Library::forget_paths` and the Settings prune flow handle
//! the permanent case and must not be reached by this one: a share that is
//! merely unmounted has not been deleted, and a reading that quietly removed
//! rows would turn a lost password into a lost library.
//!
//! **Not a claim about a file.** A reachable root does not promise every file
//! under it opens; it promises the folder is there. The honest reading is
//! about the drive, which is what the listener actually wants to know.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// **Is this file under a folder that has gone?**
///
/// `missing` is the roots the last scan could not walk. A path under none of
/// them is reachable as far as anything here knows, which is the honest answer
/// between passes.
pub(crate) fn unreachable(missing: &HashSet<PathBuf>, path: &Path) -> bool {
    missing.iter().any(|root| path.starts_with(root))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_row_is_unreachable_only_under_a_root_that_is() {
        let missing: HashSet<PathBuf> = [PathBuf::from("/mnt/share"), PathBuf::from("/media/usb")]
            .into_iter()
            .collect();
        assert!(unreachable(&missing, Path::new("/mnt/share/Rock/a.flac")));
        assert!(unreachable(&missing, Path::new("/media/usb/b.flac")));
        assert!(!unreachable(&missing, Path::new("/home/me/Music/c.flac")));
        // **A prefix is a path prefix, not a string one.** `/mnt/shareholder`
        // is a different folder from `/mnt/share`, and `starts_with` on
        // `Path` knows it where `str::starts_with` would not.
        assert!(!unreachable(&missing, Path::new("/mnt/shareholder/d.flac")));
        // Nothing missing is nothing marked, which is the ordinary case.
        assert!(!unreachable(
            &HashSet::new(),
            Path::new("/mnt/share/Rock/a.flac")
        ));
    }
}
