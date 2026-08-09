//! **The queue survives a quit** — ADR-0023 §6, built at last because
//! something on screen finally wants it.
//!
//! ADR-0006 layer 1: pure, iced-free, unit-tested. The snapshot is the run as
//! it stood when baz was closed — the paths, where the cursor was in them, how
//! far into that track it had got, and where the run came from — written
//! beside the config and read once at launch.
//!
//! # What it is not
//!
//! Not a history, not a session log, not a second library. One snapshot,
//! overwritten; a file that cannot be read is *no snapshot*, which is a state
//! the interface already has to draw (a fresh install has none).
//!
//! # Restoring, and the one place this parts company with ADR-0023 §6
//!
//! §6 says the run is restored **paused**. The engine's command table
//! (`baz_core::engine`) makes that unrepresentable at a non-zero cursor
//! without changing the engine, which §6 costed at *"zero engine changes"*:
//! `SetQueue` replaces the queue and starts nothing, a later `Play` starts at
//! the queue **top**, and every command that selects a position — `JumpTo`,
//! `Next`, `Previous` — starts the music by design, because three transport
//! commands that pick a position must not disagree about whether pressing them
//! sounds.
//!
//! So what ships is: at launch the queue is **loaded and silent**, and the
//! interrupted point is held here and spent by one press — `Resume` on the
//! Home place's `CONTINUE` placard, which is `JumpTo` at the cursor then
//! `Seek` to the position. The clause that matters is kept exactly — *nothing
//! sounds unasked* — the queue does survive the quit, and it is still one
//! press to carry on. What is lost against the letter of §6 is that the
//! transport reads *stopped* rather than *paused* until that press, which is
//! the truth about the engine rather than a shortfall in it.
//!
//! # When it is written
//!
//! On the run changing (a track starts, the queue is replaced or edited) and
//! again on exit, where the elapsed position is picked up. Never per frame and
//! never per second: between those two moments the position on disk is the
//! start of the current track, which is the correct place to resume from if
//! baz is killed rather than closed.

use std::path::{Path, PathBuf};

/// The interrupted run.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Snapshot {
    /// The queue's paths, in play order — the `SetQueue` payload, verbatim.
    pub paths: Vec<PathBuf>,
    /// Which position the cursor was on.
    pub cursor: usize,
    /// How far into that track playback had got, in milliseconds.
    pub position_ms: u64,
    /// Playing provenance (ADR-0023's amendment): the name of the playlist
    /// file this run was reified from, when one was. Origin, never a link.
    pub provenance: Option<String>,
}

impl Snapshot {
    /// Whether there is a run here at all.
    ///
    /// An empty snapshot and a missing file are the same state and must draw
    /// the same: `CONTINUE` is **absent, not empty** (ADR-0030 §6's rule).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    /// The path the cursor is on, if the cursor is still inside the run.
    ///
    /// A cursor past the end is not an error — a snapshot written by a later
    /// baz, or a file edited by hand, is an ordinary thing to meet — and it
    /// reads as *no interrupted track*, which makes the band absent.
    #[must_use]
    pub fn current(&self) -> Option<&Path> {
        self.paths.get(self.cursor).map(PathBuf::as_path)
    }

    /// Serialize to the document this module writes.
    ///
    /// One path per line rather than an inline array, for
    /// [`crate::config::Config::to_toml`]'s reason: a run of forty is
    /// readable, and a hand edit is one line.
    #[must_use]
    pub fn to_toml(&self) -> String {
        use std::fmt::Write as _;
        let mut out =
            String::from("# baz's interrupted run — written by baz on exit, safe to delete\n");
        let _ = writeln!(out, "cursor = {}", self.cursor);
        let _ = writeln!(out, "position_ms = {}", self.position_ms);
        if let Some(provenance) = &self.provenance {
            let _ = writeln!(out, "provenance = {}", toml_string(provenance));
        }
        let _ = writeln!(out, "paths = [");
        for path in self.paths.iter().filter_map(|path| path.to_str()) {
            let _ = writeln!(out, "    {},", toml_string(path));
        }
        let _ = writeln!(out, "]");
        out
    }

    /// Read a snapshot document. **Never fails**: an unreadable or partial
    /// file degrades per key, exactly as the config does, and the worst case
    /// is a run baz does not offer to continue.
    #[must_use]
    pub fn from_toml(text: &str) -> Self {
        let Ok(table) = text.parse::<toml::Table>() else {
            return Self::default();
        };
        let paths: Vec<PathBuf> = table
            .get("paths")
            .and_then(toml::Value::as_array)
            .map(|listed| {
                listed
                    .iter()
                    .filter_map(toml::Value::as_str)
                    .filter(|path| !path.is_empty())
                    .map(PathBuf::from)
                    .collect()
            })
            .unwrap_or_default();
        let read = |key: &str| {
            table
                .get(key)
                .and_then(toml::Value::as_integer)
                .and_then(|value| u64::try_from(value).ok())
                .unwrap_or(0)
        };
        Self {
            cursor: usize::try_from(read("cursor")).unwrap_or(0),
            position_ms: read("position_ms"),
            provenance: table
                .get("provenance")
                .and_then(toml::Value::as_str)
                .map(str::to_owned),
            paths,
        }
    }
}

/// `$XDG_CONFIG_HOME/baz/session.toml` — front-end state beside the config,
/// where ADR-0023 §6 puts it. `None` when the platform has no config
/// directory, which is the same "nothing is remembered" this build already
/// degrades to for the config itself.
#[must_use]
pub fn session_file() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("baz").join("session.toml"))
}

/// Read the snapshot at `path`. A missing file is an empty snapshot, not an
/// error: a fresh install has no interrupted run and that is an ordinary
/// state.
#[must_use]
pub fn load(path: &Path) -> Snapshot {
    std::fs::read_to_string(path)
        .map(|text| Snapshot::from_toml(&text))
        .unwrap_or_default()
}

/// Write the snapshot to `path`, creating the directory if it is missing.
///
/// # Errors
///
/// Whatever the filesystem reports. Every caller prints it and carries on: a
/// player that could not remember where it got to is a player that starts at
/// the top, not a player that stops.
pub fn store(path: &Path, snapshot: &Snapshot) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, snapshot.to_toml())
}

/// A TOML basic string. Same escaping as [`crate::config`]'s, and it is a
/// second copy on purpose: the two modules write two different documents and a
/// shared helper would make one file's format a dependency of the other's.
fn toml_string(value: &str) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04X}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run() -> Snapshot {
        Snapshot {
            paths: vec![
                PathBuf::from("/m/Anhydrous 1.flac"),
                PathBuf::from("/m/Anhydrous 2.flac"),
                PathBuf::from("/m/Anhydrous 3.flac"),
            ],
            cursor: 1,
            position_ms: 192_000,
            provenance: Some("Road Trip".to_owned()),
        }
    }

    /// **The run survives the round trip**, whole — which is the whole of
    /// what ADR-0023 §6 promises.
    #[test]
    fn the_interrupted_run_survives_a_round_trip() {
        let snapshot = run();
        assert_eq!(Snapshot::from_toml(&snapshot.to_toml()), snapshot);
        assert_eq!(
            snapshot.current(),
            Some(Path::new("/m/Anhydrous 2.flac")),
            "the cursor names the track that was interrupted"
        );
    }

    /// **A missing file, an empty file and a corrupt file are one state**, and
    /// it is the state a fresh install is in — so `CONTINUE` is absent rather
    /// than empty and nothing has to draw a run that is not there.
    #[test]
    fn nothing_to_continue_is_a_state_and_not_an_error() {
        for text in ["", "@@@ not toml", "cursor = 3\n", "paths = []\n"] {
            let snapshot = Snapshot::from_toml(text);
            assert!(snapshot.is_empty(), "{text:?}");
            assert_eq!(snapshot.current(), None, "{text:?}");
        }
        assert!(Snapshot::default().is_empty());
    }

    /// **A cursor past the end is not an error.** A snapshot written by a
    /// later baz, or edited by hand, is an ordinary thing to meet; it reads as
    /// *no interrupted track*, which makes the band absent rather than making
    /// the launch fail.
    #[test]
    fn a_cursor_outside_the_run_leaves_nothing_to_continue() {
        let mut snapshot = run();
        snapshot.cursor = 9;
        assert_eq!(snapshot.current(), None);
        assert!(!snapshot.is_empty(), "the run is still there to be shown");
    }

    /// Every value degrades on its own — the config's rule, inherited. A
    /// negative or non-integer cursor costs the *cursor* and nothing else.
    #[test]
    fn every_value_degrades_alone() {
        let snapshot = Snapshot::from_toml(
            "cursor = -4\nposition_ms = \"soon\"\nprovenance = 7\n\
             paths = [\"/m/a.flac\", 7, \"\", \"/m/b.flac\"]\n",
        );
        assert_eq!(snapshot.cursor, 0);
        assert_eq!(snapshot.position_ms, 0);
        assert_eq!(snapshot.provenance, None);
        assert_eq!(
            snapshot.paths,
            vec![PathBuf::from("/m/a.flac"), PathBuf::from("/m/b.flac")],
            "the readable paths survive their unreadable neighbours"
        );
    }

    /// A path with a quote or a backslash in it survives, because a music
    /// folder is the user's and baz does not get to say what is in it.
    #[test]
    fn an_awkward_path_survives_the_document() {
        let snapshot = Snapshot {
            paths: vec![PathBuf::from(r#"/m/He said "no"\a.flac"#)],
            cursor: 0,
            position_ms: 1,
            provenance: Some(r#"a "list""#.to_owned()),
        };
        assert_eq!(Snapshot::from_toml(&snapshot.to_toml()), snapshot);
    }
}
