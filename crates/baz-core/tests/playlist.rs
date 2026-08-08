//! The playlist storage layer through its public API alone (ADR-0024 §1–§3):
//! the migration story (drop foreign `.m3u` files into the folder and they
//! read), the whole shelf lifecycle, and the honesty clause — nothing here
//! ever writes a playlist file except the explicit save of an edit.

use std::path::PathBuf;

use baz_core::playlist::{Entry, Folder, Item, Playlist, PlaylistError};

/// An absolute fixture path by the platform's own rule: `/music/a.flac` is
/// drive-less — and therefore relative — on Windows, where the parser would
/// resolve it and `save` would refuse it. See the twin helper in
/// `playlist::tests`.
fn track(path: &str) -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(format!("C:{path}"))
    } else {
        PathBuf::from(path)
    }
}

/// The file a foobar2000/MusicBee refugee actually brings: CRLF, BOM,
/// Windows-flavoured metadata, relative paths, a directive baz has never
/// heard of. `cp *.m3u8` into the folder is the whole migration.
#[test]
fn a_foreign_playlist_reads_unedited_and_survives_a_baz_edit() {
    let dir = tempfile::tempdir().expect("tempdir");
    let folder = Folder::open(dir.path()).expect("open");
    let elsewhere = track("/music/elsewhere.flac");
    let source: Vec<u8> = format!(
        "\u{feff}#EXTM3U\r\n\
        #PLAYLIST:Sunday\r\n\
        #EXTINF:245,Talk Talk - Myrrhman\r\n\
        Talk Talk/01 Myrrhman.flac\r\n\
        #EXTINF:-1,Unknown Length\r\n\
        {}\r\n",
        elsewhere.display()
    )
    .into_bytes();
    std::fs::write(dir.path().join("Sunday.m3u8"), &source).expect("write");

    let listed = folder.list().expect("list");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].name, "Sunday");
    let mut playlist = listed[0].read().expect("read");

    // Read liberally: both entries, the relative one resolved against the
    // playlist's own directory, the foreign directive kept.
    let entries: Vec<&Entry> = playlist.entries().collect();
    assert_eq!(entries.len(), 2);
    assert_eq!(
        entries[0].path,
        dir.path().join("Talk Talk/01 Myrrhman.flac")
    );
    assert_eq!(
        entries[0].extinf.as_ref().map(|extinf| extinf.seconds),
        Some(Some(245))
    );
    assert_eq!(entries[1].path, elsewhere);

    // Reading changed nothing on disk.
    assert_eq!(std::fs::read(playlist.path()).expect("read"), source);

    // A user edit, saved: the strict subset is written, and the foreign
    // #PLAYLIST directive is still there — a rewrite never strips what it
    // did not understand.
    playlist
        .items_mut()
        .push(Item::Entry(Entry::new(track("/music/added.flac"))));
    playlist.save().expect("save");
    let text = std::fs::read_to_string(playlist.path()).expect("read");
    assert!(text.starts_with("#EXTM3U\n"), "{text:?}");
    assert!(text.contains("#PLAYLIST:Sunday\n"), "{text:?}");
    assert!(
        text.contains("#EXTINF:245,Talk Talk - Myrrhman\n"),
        "{text:?}"
    );
    assert!(text.ends_with("/music/added.flac\n"), "{text:?}");
    assert!(!text.contains('\r'), "the strict subset is LF-only");

    // And the rewrite is a fixed point: saving again changes nothing.
    let mut reread = Playlist::read(playlist.path()).expect("reread");
    let after_first_save = std::fs::read(playlist.path()).expect("read");
    reread.save().expect("save again");
    assert_eq!(
        std::fs::read(reread.path()).expect("read"),
        after_first_save,
        "an idempotent rewrite wrote different bytes"
    );
}

#[test]
fn the_shelf_lifecycle_end_to_end() {
    let dir = tempfile::tempdir().expect("tempdir");
    let folder = Folder::open(dir.path()).expect("open");

    // Create, refuse a duplicate, refuse a name no filesystem would keep.
    let mut evening = folder.create("Evening").expect("create");
    assert!(matches!(
        folder.create("Evening"),
        Err(PlaylistError::AlreadyExists { .. })
    ));
    assert!(matches!(
        folder.create("a/b"),
        Err(PlaylistError::InvalidName { .. })
    ));

    // Build tonight's list and save it — the one write there is.
    for path in ["/music/a.flac", "/music/b.flac", "/music/a.flac"] {
        evening
            .items_mut()
            .push(Item::Entry(Entry::new(track(path))));
    }
    evening.save().expect("save");

    // Duplicates are the maker's business and survive the trip.
    let evening = Playlist::read(evening.path()).expect("reread");
    assert_eq!(evening.entries().count(), 3);

    // "38 of 40 · 2 missing": the caller judges, the partition reports.
    let missing = track("/music/b.flac");
    let verdict = evening.partition(|path| path != missing.as_path());
    assert_eq!(verdict.playable.len(), 2);
    assert_eq!(verdict.missing.len(), 1);

    // The fingerprint notices the vim edit; re-reading honours it.
    let mut on_disk = std::fs::read(evening.path()).expect("read");
    on_disk.extend_from_slice(b"/music/added by hand.flac\n");
    std::fs::write(evening.path(), &on_disk).expect("write");
    assert!(evening.externally_edited());
    assert_eq!(
        Playlist::read(evening.path())
            .expect("reread")
            .entries()
            .count(),
        4
    );

    // Rename refuses to overwrite; delete removes the file and nothing else.
    folder.create("Morning").expect("create");
    assert!(matches!(
        folder.rename("Morning", "Evening"),
        Err(PlaylistError::AlreadyExists { .. })
    ));
    folder.rename("Morning", "Early").expect("rename");
    folder.delete("Early").expect("delete");
    let names: Vec<String> = folder
        .list()
        .expect("list")
        .into_iter()
        .map(|file| file.name)
        .collect();
    assert_eq!(names, ["Evening"]);
}
