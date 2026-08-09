//! `Folder::delete_to_trash` moves the file to the XDG trash — it never
//! unlinks (doc 11 §5 P2: forgiveness is reversibility first, and the trash
//! is the mechanism that let the confirm dialog retire).
//!
//! # Why this test lives in its own binary
//!
//! The freedesktop trash resolves through the environment —
//! `$XDG_DATA_HOME/Trash` — and the *only* way to keep this test off the
//! developer's real trash is to redirect that variable. Environment writes
//! are process-global, so the test gets a test binary to itself: one
//! `#[test]`, the variables set before any other code runs, nothing racing.
//! This is the six-variable isolation rule of `docs/DEVELOPMENT.md`
//! §"Headless UI verification", applied at test scale — the same rule that
//! exists because an unisolated run once polluted the maintainer's real
//! library.
//!
//! Everything — the playlists folder, `HOME`, `XDG_DATA_HOME` — sits inside
//! one tempdir, which also keeps file and trash on one filesystem, so the
//! spec's home-trash branch (not a mount's `.Trash-$uid`) is the one
//! exercised.

#![expect(
    unsafe_code,
    reason = "redirecting XDG_DATA_HOME is the whole point of the test — the \
              alternative is a test that writes into the developer's real \
              trash. One test, one binary, set before any thread exists; \
              ENGINEERING.md's unsafe rule names explicit opt-outs like this."
)]

use baz_core::playlist::{Folder, PlaylistError};

#[test]
fn delete_to_trash_moves_the_file_into_the_xdg_trash_and_never_unlinks() {
    let scratch = tempfile::tempdir().expect("tempdir");
    let data_home = scratch.path().join("data");
    let home = scratch.path().join("home");
    std::fs::create_dir_all(&data_home).expect("data home");
    std::fs::create_dir_all(&home).expect("home");
    // SAFETY: this test binary holds exactly one test, so these writes
    // happen before any thread the process will ever have — the reason the
    // file holds one `#[test]` and must keep holding one.
    unsafe {
        std::env::set_var("XDG_DATA_HOME", &data_home);
        std::env::set_var("HOME", &home);
    }

    let folder = Folder::open(scratch.path().join("playlists")).expect("open folder");
    let playlist = folder.create("Road Trip").expect("create");
    let kept_path = playlist.path().to_path_buf();
    assert!(kept_path.is_file());

    folder.delete_to_trash("Road Trip").expect("trash");

    // The folder no longer holds it…
    assert!(!kept_path.exists(), "the file must leave the folder");
    assert!(matches!(
        folder.delete_to_trash("Road Trip"),
        Err(PlaylistError::NotFound { .. })
    ));

    // …and the XDG trash does: the file itself under `Trash/files`, and the
    // spec's `.trashinfo` beside it under `Trash/info`, naming where it came
    // from — which is what makes the desktop's Restore work.
    let files: Vec<_> = std::fs::read_dir(data_home.join("Trash").join("files"))
        .expect("Trash/files exists — the delete moved, it did not unlink")
        .map(|entry| entry.expect("entry").file_name())
        .collect();
    assert_eq!(files.len(), 1, "one deleted playlist, one trashed file");
    let trashed = files[0].to_string_lossy().into_owned();
    assert!(
        trashed.starts_with("Road Trip"),
        "the trashed file keeps its name: {trashed}"
    );
    let info = data_home
        .join("Trash")
        .join("info")
        .join(format!("{trashed}.trashinfo"));
    let written = std::fs::read_to_string(&info).expect(".trashinfo beside the file");
    assert!(
        written.contains("Path="),
        ".trashinfo records the origin: {written}"
    );

    // The reversal the whole change exists for: the trashed bytes are the
    // playlist, recoverable by hand or by any file manager.
    let recovered = std::fs::read_to_string(data_home.join("Trash").join("files").join(&trashed))
        .expect("the trashed file is readable");
    assert!(recovered.starts_with("#EXTM3U"));
}
