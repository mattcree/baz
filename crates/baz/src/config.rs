//! Persistent configuration: the music folders, and the settings a listener
//! sets once and expects to find again.
//!
//! The file is `$XDG_CONFIG_HOME/baz/config.toml` (via the `dirs` crate). It
//! currently carries three things — the music folders baz holds, the
//! arrangement of the wall, and the ReplayGain setting (ADR-0013) — and it is
//! written by baz and documented as safe to edit by hand.
//!
//! # `music_dirs`, and the `music_dir` it replaced
//!
//! baz held exactly one folder until ADR-0022, under the key `music_dir`. It
//! now holds an **ordered list**, under `music_dirs`, and the order is the
//! listener's: it is the order the Settings place lists them in, the order they
//! are scanned in, and the order a nested pair is resolved in.
//!
//! A config written by the old baz is read and **migrated silently**: a file
//! with `music_dir` and no `music_dirs` yields exactly that one folder, and the
//! next write replaces the key. Nothing is asked of the listener, and nothing
//! is lost — losing somebody's library to a change in a file format would be a
//! self-inflicted version of the failure ADR-0010's removal gates exist to
//! prevent.
//!
//! The fallback is per key rather than per file, like everything else here: a
//! `music_dirs` that is present but unreadable (not an array, or an array of
//! numbers) falls back to `music_dir` too, because the *most conservative*
//! reading of a damaged file is the one that keeps a folder baz can still see.
//!
//! # Why the `toml` crate now, and not before
//!
//! v0.1 hand-rolled a single-key writer and said so in as many words: *"if the
//! config ever grows past a couple of keys, switching to the `toml` crate is
//! the plan of record"*, which `docs/BACKLOG.md` repeated. ReplayGain is the
//! growth that was being waited for — one key becomes five, one of them a
//! table — so this is that switch, taken on the terms it was promised on
//! rather than deferred again.
//!
//! The cost was measured before it was paid. `toml` adds **three** crates to
//! the lock file (`toml`, `serde_spanned`, `toml_writer`); its parser
//! (`toml_parser`, `toml_datetime`, `winnow`) and `serde` were already in the
//! graph, and every one of them is MIT OR Apache-2.0, which `deny.toml`
//! already allows. What the three buy is the half of TOML a hand-rolled
//! reader silently gets wrong: a trailing comment after a value, a literal
//! `'single-quoted'` string, `1_000`, and the escape sequences the old
//! `escape`/`unescape` pair had to keep in step with the specification by
//! hand. A file baz *invites* people to edit must not quietly ignore a valid
//! edit, and "quietly ignore" is precisely the failure mode of a parser that
//! only recognises the subset it writes.
//!
//! # Nothing here fails; everything degrades
//!
//! [`load`] returns a [`Config`], never an error and never an `Option`. A
//! missing file, an unreadable one, one that is not TOML at all, and one whose
//! `mode` says `"loudest"` all resolve the same way: **each key that cannot be
//! read takes its default, and the keys around it are unaffected**. That is
//! deliberately per-key rather than per-document, and it is the reason the
//! reader walks a [`toml::Table`] by hand instead of deriving
//! `Deserialize` — a `#[derive]` fails the *whole* document on one bad value,
//! which would lose a listener their music folder because they mistyped a
//! pre-amp. The defaults are `baz-core`'s own
//! ([`ReplayGainSettings::default`] — off, no pre-amp, clipping prevention
//! armed), so a config baz has never written and a config baz cannot
//! understand both mean exactly what a fresh install means.
//!
//! # Units: centidecibels, as everywhere else
//!
//! Both pre-amps are stored as integer **centidecibels** — `preamp_centidb =
//! -350` is −3.50 dB — which is ADR-0013 §1's argument for the third time in
//! this workspace: one canonical integer encoding, so the round-trip test
//! below tests the config rather than a float formatter. The key names carry
//! the unit, and the written file carries a comment saying so.
//!
//! The `mode` spelling is not hand-maintained either: it is round-tripped
//! through the same `serde` implementation the protocol's JSON uses, so the
//! word in `config.toml` is the word on the wire by construction rather than
//! by two lists agreeing.
//!
//! # Limitation: UTF-8 paths only
//!
//! TOML strings are UTF-8, so a music directory whose path is not valid UTF-8
//! cannot be persisted. Such a directory still works for the session (paths
//! are handled as `PathBuf` throughout); it is simply left out of the
//! `music_dirs` array — **and the rest of the array, and the rest of the
//! document, are still written**, which is the one behaviour change from v0.1's
//! writer. Losing an unrelated setting, or an unrelated folder, because one
//! path is unrepresentable would be a second failure caused by the first.

use std::io;
use std::path::{Path, PathBuf};

use baz_core::index::GroupKey;
use baz_core::protocol::ReplayGainMode;
use baz_core::replaygain::ReplayGainSettings;

use crate::shelf::Density;

/// The `[replaygain]` table's name in the document.
const REPLAY_GAIN_TABLE: &str = "replaygain";

/// The key the active group key is written under.
const GROUP_KEY: &str = "group_key";

/// The key the music folders are written under (ADR-0022).
const MUSIC_DIRS: &str = "music_dirs";

/// The key baz wrote its single music folder under before ADR-0022. Read, never
/// written: a file carrying it is migrated to [`MUSIC_DIRS`] on the next save.
const LEGACY_MUSIC_DIR: &str = "music_dir";

/// The key the density step is written under.
const DENSITY: &str = "density";

/// The key the returns lane's open/closed state is written under.
const SIDEBAR_OPEN: &str = "sidebar_open";

/// Application configuration. See the [module docs](self) for scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// The music folders baz scans and shelves, **in the listener's order**
    /// (ADR-0022). Empty before a first run has chosen one.
    ///
    /// The order is data, not presentation: it is the order the folders are
    /// scanned in, the order the Settings place lists them in, and — because a
    /// pre-v8 row can be claimed by only one root — the order a nested pair is
    /// resolved in. Duplicates are dropped on the way in, since a folder listed
    /// twice would be walked twice for one set of rows.
    pub music_dirs: Vec<PathBuf>,
    /// How ReplayGain is configured (ADR-0013). Engine state, persisted here
    /// because it is a listener's standing decision rather than a property of
    /// a session: unlike panel visibility (`crate::panels`, deliberately not
    /// persisted) it has something to say on the first frame of every launch.
    pub replay_gain: ReplayGainSettings,
    /// How the wall is arranged — ARTIST / YEAR / GENRE / ADDED / PLAYED
    /// (ADR-0019).
    ///
    /// **View state, persisted**, which is the distinction ADR-0017 §1.3 draws
    /// and the reason there is no Settings row for it: a listener presses a
    /// word in the top bar once and expects the wall to be arranged that way
    /// the next time baz opens. Density lands here on the same terms.
    ///
    /// Written as [`GroupKey::code`] — a stable lowercase word, not an index —
    /// so the file stays legible and a key added or reordered in `baz-core`
    /// cannot silently re-arrange somebody's wall.
    pub group_key: GroupKey,
    /// How closely the wall hangs its works — Spacious / Balanced / Dense
    /// (ADR-0017 step 6, `.interface-design/system.md` §7.1).
    ///
    /// **View state, persisted, and deliberately not a setting.** ADR-0017
    /// §1.3 takes the critique's better half — *Settings must never be the
    /// answer to a view question* — and supersedes `02` §2.7's Settings →
    /// Appearance row: the control is the three detent marks at the foot of
    /// the index rail's lane (ADR-0028), with <kbd>Ctrl</kbd>+<kbd>-</kbd> /
    /// <kbd>Ctrl</kbd>+<kbd>=</kbd> and <kbd>Ctrl</kbd>+scroll as its
    /// accelerators, and this key is where the press's *result* is
    /// remembered, exactly as `group_key` remembers the press on a word in
    /// the top bar. There is no density row anywhere in the Settings place,
    /// and the product's standing rules (as ADR-0028 narrowed it) still refuses the
    /// view-options menu that would be the other way to spell it.
    ///
    /// Written as [`Density::code`], for `group_key`'s reason.
    pub density: Density,
    /// Whether the returns lane stands open (ADR-0030 §3).
    ///
    /// **View state, persisted, and deliberately not a setting** — `density`'s
    /// argument exactly, and ADR-0030 restates ADR-0017 §1.3 to say so: the
    /// control is the pair of marks at the lane's foot with
    /// <kbd>Ctrl</kbd>+<kbd>B</kbd> as its accelerator, and there is no
    /// Settings row anywhere for it.
    ///
    /// A fresh baz opens with the lane **open**: it is the surface the owner
    /// asked for, and a resident index that arrives collapsed would have to be
    /// discovered before it could be used.
    pub sidebar_open: bool,
}

impl Default for Config {
    /// No music folders, `baz-core`'s own ReplayGain defaults, the wall
    /// arranged by artist and hung at the default density — the state a fresh
    /// install is in, and the state an unreadable config resolves to.
    fn default() -> Self {
        Self {
            music_dirs: Vec::new(),
            replay_gain: ReplayGainSettings::default(),
            group_key: GroupKey::Artist,
            density: Density::Balanced,
            sidebar_open: true,
        }
    }
}

impl Config {
    /// Serialize to the document this module writes.
    ///
    /// Assembled rather than derived so the comments survive: this file is
    /// meant to be opened and understood, and a serializer's output explains
    /// nothing. Values are still rendered by `toml`, so the quoting and
    /// escaping are the specification's rather than ours.
    #[must_use]
    pub fn to_toml(&self) -> String {
        use std::fmt::Write as _;
        // Writing into a `String` cannot fail, so every `write!` here is
        // infallible; the results are dropped rather than handled.
        let mut out = String::from("# baz configuration — written by baz, safe to edit\n");
        // Written on one line per folder rather than as an inline array, so a
        // list of four is readable and a hand edit is one line. An empty list
        // omits the key entirely: a fresh install's file should not carry an
        // empty array asking to be filled in.
        let dirs: Vec<&str> = self
            .music_dirs
            .iter()
            .filter_map(|dir| dir.to_str())
            .collect();
        if !dirs.is_empty() {
            let _ = writeln!(out, "# the folders baz holds, scanned in this order");
            let _ = writeln!(out, "{MUSIC_DIRS} = [");
            for dir in dirs {
                let _ = writeln!(out, "    {},", toml_string(dir));
            }
            let _ = writeln!(out, "]");
        }
        let _ = writeln!(
            out,
            "# how the wall is arranged: \"artist\", \"year\", \"genre\", \
             \"added\" or \"played\"\n{GROUP_KEY} = {}",
            toml_string(self.group_key.code()),
        );
        let _ = writeln!(
            out,
            "# how closely it hangs: \"spacious\", \"balanced\" or \"dense\" \
             (Ctrl+- / Ctrl+= / Ctrl+scroll)\n{DENSITY} = {}",
            toml_string(self.density.code()),
        );
        let _ = writeln!(
            out,
            "# whether the returns lane stands open (Ctrl+B, or the two \
             marks at its foot)\n{SIDEBAR_OPEN} = {}",
            self.sidebar_open,
        );
        let _ = write!(
            out,
            "\n[{REPLAY_GAIN_TABLE}]\n\
             # mode: \"off\" (the default — baz changes nothing), \"track\" or \"album\"\n\
             mode = {}\n\
             # gains are centidecibels, hundredths of a decibel: -350 is -3.50 dB\n\
             preamp_centidb = {}\n\
             no_tag_preamp_centidb = {}\n\
             prevent_clipping = {}\n",
            toml_string(mode_key(self.replay_gain.mode)),
            self.replay_gain.preamp_centidb,
            self.replay_gain.no_tag_preamp_centidb,
            self.replay_gain.prevent_clipping,
        );
        out
    }

    /// Read a config document. Never fails: see the module's degradation note.
    #[must_use]
    pub fn from_toml(text: &str) -> Self {
        let Ok(table) = text.parse::<toml::Table>() else {
            return Self::default();
        };
        let music_dirs = read_music_dirs(&table);
        let replay_gain = table
            .get(REPLAY_GAIN_TABLE)
            .and_then(toml::Value::as_table)
            .map_or_else(ReplayGainSettings::default, read_replay_gain);
        // The same per-key degradation every value here gets: a key spelled
        // by a newer baz, or by a hand that guessed, is the default arrangement
        // and costs nothing around it. `GroupKey::from_code` is documented for
        // exactly this.
        let group_key = table
            .get(GROUP_KEY)
            .and_then(toml::Value::as_str)
            .and_then(GroupKey::from_code)
            .unwrap_or(GroupKey::Artist);
        // The same per-key degradation, for the same reason: a step this build
        // cannot name must cost the wall its zoom and nothing else. It
        // degrades to Balanced rather than to the nearest step, because there
        // is no nearest step to an unreadable word.
        let density = table
            .get(DENSITY)
            .and_then(toml::Value::as_str)
            .and_then(Density::from_code)
            .unwrap_or(Density::Balanced);
        // A bool degrades to the default the same way a word does: a value
        // that is not a bool at all — a hand edit, a newer baz's spelling —
        // costs the lane its remembered state and nothing around it.
        let sidebar_open = table
            .get(SIDEBAR_OPEN)
            .and_then(toml::Value::as_bool)
            .unwrap_or(true);
        Self {
            music_dirs,
            replay_gain,
            group_key,
            density,
            sidebar_open,
        }
    }
}

/// The music folders a document names, in order, with the legacy single key as
/// the fallback (module docs).
///
/// Three degradations, each on its own and none of them fatal:
///
/// - An entry that is not a string, or is blank, is **skipped**; the folders
///   around it survive. One mistyped line must not cost a listener their other
///   three folders.
/// - A duplicate is dropped, keeping the first mention. A folder listed twice
///   would be walked twice for one set of rows, and the second walk would
///   re-home them to the same place it found them.
/// - A `music_dirs` that is absent, not an array, or an array with nothing
///   usable in it falls back to the pre-ADR-0022 `music_dir` string. That is
///   the silent migration, and it is also the most conservative reading of a
///   damaged file.
fn read_music_dirs(table: &toml::Table) -> Vec<PathBuf> {
    let listed: Vec<PathBuf> = table
        .get(MUSIC_DIRS)
        .and_then(toml::Value::as_array)
        .map(|array| {
            let mut dirs: Vec<PathBuf> = Vec::with_capacity(array.len());
            for value in array {
                let Some(dir) = value.as_str().filter(|dir| !dir.is_empty()) else {
                    continue;
                };
                let dir = PathBuf::from(dir);
                if !dirs.contains(&dir) {
                    dirs.push(dir);
                }
            }
            dirs
        })
        .unwrap_or_default();
    if !listed.is_empty() {
        return listed;
    }
    table
        .get(LEGACY_MUSIC_DIR)
        .and_then(toml::Value::as_str)
        .filter(|dir| !dir.is_empty())
        .map(|dir| vec![PathBuf::from(dir)])
        .unwrap_or_default()
}

/// Read the `[replaygain]` table, key by key, defaulting each miss on its own.
///
/// `ReplayGainSettings::new` clamps both pre-amps into
/// ±[`MAX_PREAMP_CENTIDB`](baz_core::replaygain::MAX_PREAMP_CENTIDB), so a
/// hand-edited `preamp_centidb = 99999` is the most rather than an error — the
/// same answer the engine gives the same number, which is what keeps the file
/// and the engine from disagreeing about what was asked for.
fn read_replay_gain(table: &toml::Table) -> ReplayGainSettings {
    let defaults = ReplayGainSettings::default();
    let mode = table
        .get("mode")
        .cloned()
        .and_then(|value| value.try_into::<ReplayGainMode>().ok())
        .unwrap_or(defaults.mode);
    ReplayGainSettings::new(
        mode,
        centidb(table, "preamp_centidb").unwrap_or(defaults.preamp_centidb),
        centidb(table, "no_tag_preamp_centidb").unwrap_or(defaults.no_tag_preamp_centidb),
        table
            .get("prevent_clipping")
            .and_then(toml::Value::as_bool)
            .unwrap_or(defaults.prevent_clipping),
    )
}

/// A centidecibel figure from `key`, or `None` when the file does not carry an
/// integer that fits one.
///
/// Out-of-`i16` values read as absent rather than saturating, for the reason
/// `baz_core::replaygain`'s tag parser gives: a number nobody could have meant
/// is not a number to guess the intent of.
fn centidb(table: &toml::Table, key: &str) -> Option<i16> {
    i16::try_from(table.get(key).and_then(toml::Value::as_integer)?).ok()
}

/// The document's spelling of `mode`, taken from the protocol's own `serde`
/// implementation so the two cannot drift.
///
/// Falls back to the default mode's spelling for a variant this build does not
/// know how to name — `ReplayGainMode` is `#[non_exhaustive]`, and writing a
/// mode baz could not read back would be worse than writing the default.
fn mode_key(mode: ReplayGainMode) -> &'static str {
    match mode {
        ReplayGainMode::Track => "track",
        ReplayGainMode::Album => "album",
        // `Off`, and — since `ReplayGainMode` is `#[non_exhaustive]` — any
        // mode this build cannot name. Off is the default, and it is the one
        // answer that is safe to write for a mode nobody here understands.
        _ => "off",
    }
}

/// `value` as a TOML basic string, quoted and escaped by `toml` itself.
fn toml_string(value: &str) -> String {
    toml::Value::String(value.to_owned()).to_string()
}

/// `$XDG_CONFIG_HOME/baz/config.toml`, or `None` on a platform where no
/// config directory can be determined.
pub fn config_file() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("baz").join("config.toml"))
}

/// `$XDG_DATA_HOME/baz/library.db` — the SQLite library index location
/// (see `baz_core::index`); `None` when no data directory exists.
pub fn library_db_file() -> Option<PathBuf> {
    Some(dirs::data_dir()?.join("baz").join("library.db"))
}

/// Load the config from `path`. A missing, unreadable or unparsable file is
/// the default config — never an error dialog, and never a lost setting the
/// file *did* state correctly (module docs).
pub fn load(path: &Path) -> Config {
    std::fs::read_to_string(path)
        .map(|text| Config::from_toml(&text))
        .unwrap_or_default()
}

/// Persist `config` to `path`, creating parent directories as needed.
///
/// # Errors
///
/// Any filesystem error from creating the directory or writing the file. A
/// non-UTF-8 music folder is **not** an error: it is omitted from the array and
/// the rest of the document is written (module docs).
pub fn store(path: &Path, config: &Config) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, config.to_toml())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(
        mode: ReplayGainMode,
        preamp: i16,
        no_tag: i16,
        prevent_clipping: bool,
    ) -> ReplayGainSettings {
        ReplayGainSettings::new(mode, preamp, no_tag, prevent_clipping)
    }

    #[test]
    fn round_trips_plain_and_awkward_paths() {
        for dir in [
            "/home/user/Music",
            "/home/user/My \"Music\"",
            "/home/user/back\\slash",
            "/home/ünï çödé/曲",
            "/home/user/line\nbreak\tand\rreturn",
            "/home/user/ctrl\u{1}char",
            "/home/user/# not a comment",
        ] {
            let config = Config {
                music_dirs: vec![PathBuf::from(dir)],
                ..Config::default()
            };
            let back = Config::from_toml(&config.to_toml());
            assert_eq!(back, config, "round-trip failed for {dir:?}");
        }
    }

    /// The persisted setting, every mode and both signs of both pre-amps,
    /// through the document and back unchanged.
    #[test]
    fn round_trips_every_replay_gain_setting() {
        let cases = [
            settings(ReplayGainMode::Off, 0, 0, true),
            settings(ReplayGainMode::Track, -350, 0, true),
            settings(ReplayGainMode::Album, 600, -500, false),
            settings(ReplayGainMode::Track, 2000, -2000, true),
        ];
        for replay_gain in cases {
            let config = Config {
                music_dirs: vec![PathBuf::from("/m")],
                replay_gain,
                group_key: GroupKey::Year,
                density: Density::Dense,
                sidebar_open: true,
            };
            let back = Config::from_toml(&config.to_toml());
            assert_eq!(back, config, "round-trip failed for {replay_gain:?}");
        }
    }

    /// The document's `mode` word is the protocol's word. Pinned as bytes so
    /// that a config written by this build stays readable by a build whose
    /// `serde` naming somebody changed — and so the file is legible.
    #[test]
    fn the_mode_is_spelled_the_way_the_protocol_spells_it() {
        for (mode, word) in [
            (ReplayGainMode::Off, "off"),
            (ReplayGainMode::Track, "track"),
            (ReplayGainMode::Album, "album"),
        ] {
            assert_eq!(mode_key(mode), word);
            let config = Config {
                replay_gain: settings(mode, 0, 0, true),
                ..Config::default()
            };
            assert!(
                config.to_toml().contains(&format!("mode = \"{word}\"")),
                "{mode:?} was not written as {word:?}:\n{}",
                config.to_toml()
            );
            // And the same word read back through `serde` is the same mode,
            // which is what makes the two directions one decision.
            assert_eq!(Config::from_toml(&config.to_toml()).replay_gain.mode, mode);
        }
    }

    /// The active group key survives a restart, in every arrangement — and it
    /// is written as the word `baz-core` spells it, so the file is legible and
    /// a reordered enum cannot re-arrange somebody's wall.
    #[test]
    fn round_trips_every_group_key_as_its_own_word() {
        for key in GroupKey::ALL {
            let config = Config {
                music_dirs: vec![PathBuf::from("/m")],
                group_key: key,
                ..Config::default()
            };
            let text = config.to_toml();
            assert!(
                text.contains(&format!("group_key = \"{}\"", key.code())),
                "{key:?} was not written as its code:\n{text}"
            );
            assert_eq!(Config::from_toml(&text), config, "{key:?} did not survive");
        }
    }

    /// **The density step survives a restart**, in every step, written as the
    /// word `shelf.rs` spells it — the zoom is a gesture, but where it landed
    /// is remembered like any other view state (ADR-0017 §1.3).
    #[test]
    fn round_trips_every_density_step_as_its_own_word() {
        for density in Density::ALL {
            let config = Config {
                music_dirs: vec![PathBuf::from("/m")],
                density,
                ..Config::default()
            };
            let text = config.to_toml();
            assert!(
                text.contains(&format!("density = \"{}\"", density.code())),
                "{density:?} was not written as its code:\n{text}"
            );
            assert_eq!(
                Config::from_toml(&text),
                config,
                "{density:?} did not survive"
            );
        }
    }

    /// A density step baz cannot read degrades **alone**, to Balanced — the
    /// same per-key rule everything else in the document gets, applied to the
    /// one value that would otherwise re-hang a whole wall.
    #[test]
    fn an_unreadable_density_degrades_to_balanced_alone() {
        for spelling in [
            "\"compact\"",
            "\"DENSE\"",
            "3",
            "true",
            "\"\"",
            "\"dense \"",
        ] {
            let text = format!(
                "music_dir = \"/m\"\ngroup_key = \"year\"\ndensity = {spelling}\n\
                 [replaygain]\nmode = \"album\"\n"
            );
            let config = Config::from_toml(&text);
            assert_eq!(config.density, Density::Balanced, "{spelling}");
            assert_eq!(config.group_key, GroupKey::Year, "{spelling}");
            assert_eq!(config.music_dirs, vec![PathBuf::from("/m")], "{spelling}");
            assert_eq!(config.replay_gain.mode, ReplayGainMode::Album, "{spelling}");
        }
        // Absent entirely — every config written before this key existed, and
        // every config written by a baz that has never been zoomed.
        assert_eq!(
            Config::from_toml("music_dir = \"/m\"\n").density,
            Density::Balanced
        );
    }

    /// A group key baz cannot read degrades **alone**, to ARTIST — the same
    /// per-key rule the pre-amps get, applied to the one setting whose loss
    /// would rearrange a whole wall.
    #[test]
    fn an_unreadable_group_key_degrades_to_artist_alone() {
        for spelling in ["\"crates\"", "\"ARTIST\"", "7", "true", "\"\""] {
            let text = format!(
                "music_dir = \"/m\"\ngroup_key = {spelling}\ndensity = \"dense\"\n\
                 [replaygain]\nmode = \"album\"\n"
            );
            let config = Config::from_toml(&text);
            assert_eq!(config.group_key, GroupKey::Artist, "{spelling}");
            assert_eq!(config.density, Density::Dense, "{spelling}");
            assert_eq!(config.music_dirs, vec![PathBuf::from("/m")], "{spelling}");
            assert_eq!(config.replay_gain.mode, ReplayGainMode::Album, "{spelling}");
        }
        // Absent entirely — every config written before this key existed.
        assert_eq!(
            Config::from_toml("music_dir = \"/m\"\n").group_key,
            GroupKey::Artist
        );
    }

    #[test]
    fn parse_tolerates_comments_and_whitespace_and_unknown_keys() {
        let text = "# a comment\n\n  music_dir   =   \"/m\"  # trailing\n\
                    future_key = 3\n\n[replaygain]\nmode = 'album'\n\
                    preamp_centidb = -3_50\n[future_table]\nx = 1\n";
        let config = Config::from_toml(text);
        assert_eq!(config.music_dirs, vec![PathBuf::from("/m")]);
        assert_eq!(config.replay_gain.mode, ReplayGainMode::Album);
        assert_eq!(config.replay_gain.preamp_centidb, -350);
        // Absent keys inside a table that *is* present still default.
        assert!(config.replay_gain.prevent_clipping);
        assert_eq!(config.replay_gain.no_tag_preamp_centidb, 0);
    }

    /// The degradation rule, key by key: nothing here is an error, and no bad
    /// value takes a good one down with it.
    #[test]
    fn a_corrupt_or_absent_value_degrades_to_the_default_alone() {
        let default = ReplayGainSettings::default();

        // No file at all — the caller's `load` path, and a fresh install.
        assert_eq!(Config::from_toml(""), Config::default());
        // Not TOML in the slightest.
        assert_eq!(Config::from_toml("}{ not toml ["), Config::default());
        // A whole document of the wrong shape where the table should be.
        assert_eq!(
            Config::from_toml("replaygain = 7\nmusic_dir = \"/m\"\n").replay_gain,
            default
        );

        // One spoiled key at a time, with good ones around it that all have to
        // survive. Each document is written whole rather than by appending a
        // second copy of the key — two `mode =` lines would be a *duplicate
        // key*, which is not TOML at all and is the separate case below.
        let good = ("\"album\"", "-350", "0", "false");
        let spoil = |mode: &str, preamp: &str, no_tag: &str, clipping: &str| {
            format!(
                "music_dir = \"/m\"\n[replaygain]\nmode = {mode}\n\
                 preamp_centidb = {preamp}\nno_tag_preamp_centidb = {no_tag}\n\
                 prevent_clipping = {clipping}\n"
            )
        };
        for (text, description, expected) in [
            (
                spoil("\"loudest\"", good.1, good.2, good.3),
                "a mode that does not exist",
                settings(default.mode, -350, 0, false),
            ),
            (
                spoil("3", good.1, good.2, good.3),
                "a mode of the wrong type",
                settings(default.mode, -350, 0, false),
            ),
            (
                spoil(good.0, "\"loud\"", good.2, good.3),
                "a pre-amp that is not a number",
                settings(ReplayGainMode::Album, default.preamp_centidb, 0, false),
            ),
            (
                spoil(good.0, "1e9", good.2, good.3),
                "a pre-amp beyond i16",
                settings(ReplayGainMode::Album, default.preamp_centidb, 0, false),
            ),
            (
                spoil(good.0, "-99999999", good.2, good.3),
                "a pre-amp far below i16",
                settings(ReplayGainMode::Album, default.preamp_centidb, 0, false),
            ),
            (
                spoil(good.0, good.1, good.2, "\"yes\""),
                "a flag that is not a bool",
                settings(ReplayGainMode::Album, -350, 0, default.prevent_clipping),
            ),
        ] {
            let config = Config::from_toml(&text);
            assert_eq!(
                config.music_dirs,
                vec![PathBuf::from("/m")],
                "{description} lost the music folder"
            );
            assert_eq!(
                config.replay_gain, expected,
                "{description} did not degrade alone"
            );
        }
    }

    /// A duplicate key is not TOML at all, so the *document* is refused —
    /// which is the whole document degrading, and is the one case where that
    /// is right: there is no way to know which of the two values was meant.
    #[test]
    fn a_document_that_is_not_toml_is_the_default_config() {
        assert_eq!(
            Config::from_toml("music_dir = \"/a\"\nmusic_dir = \"/b\"\n"),
            Config::default()
        );
    }

    /// A pre-amp past the engine's limit is clamped on the way in, so the file
    /// and the engine agree about what a sloppy hand edit asked for.
    #[test]
    fn an_out_of_range_preamp_is_clamped_the_way_the_engine_clamps_it() {
        let config = Config::from_toml(
            "[replaygain]\npreamp_centidb = 30000\nno_tag_preamp_centidb = -30000\n",
        );
        assert_eq!(
            config.replay_gain.preamp_centidb,
            baz_core::replaygain::MAX_PREAMP_CENTIDB
        );
        assert_eq!(
            config.replay_gain.no_tag_preamp_centidb,
            -baz_core::replaygain::MAX_PREAMP_CENTIDB
        );
    }

    #[test]
    fn parse_rejects_a_missing_or_empty_music_dir_without_losing_the_rest() {
        assert!(Config::from_toml("music_dir = \"\"").music_dirs.is_empty());
        assert!(Config::from_toml("other = \"x\"").music_dirs.is_empty());
        let config = Config::from_toml("music_dir = \"\"\n[replaygain]\nmode = \"track\"\n");
        assert!(config.music_dirs.is_empty());
        assert_eq!(config.replay_gain.mode, ReplayGainMode::Track);
    }

    /// v0.1 refused to write the file at all for an unrepresentable path.
    /// Now the path is omitted and everything else is kept: one limitation
    /// must not become two.
    #[cfg(unix)]
    #[test]
    fn a_non_utf8_music_dir_is_omitted_rather_than_losing_the_document() {
        use std::os::unix::ffi::OsStringExt as _;
        let raw = std::ffi::OsString::from_vec(b"/music/\xFF\xFE".to_vec());
        let config = Config {
            music_dirs: vec![PathBuf::from(raw)],
            replay_gain: settings(ReplayGainMode::Album, -300, 0, false),
            group_key: GroupKey::Genre,
            density: Density::Spacious,
            sidebar_open: true,
        };
        let text = config.to_toml();
        assert!(!text.contains(MUSIC_DIRS), "{text}");
        let back = Config::from_toml(&text);
        assert!(back.music_dirs.is_empty());
        assert_eq!(back.replay_gain, config.replay_gain);
    }

    #[test]
    fn store_and_load_round_trip_on_disk() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("nested").join("config.toml");
        let config = Config {
            music_dirs: vec![PathBuf::from("/home/user/Music")],
            replay_gain: settings(ReplayGainMode::Album, -350, 250, false),
            group_key: GroupKey::Played,
            density: Density::Spacious,
            sidebar_open: true,
        };
        store(&path, &config).expect("store creates parents and writes");
        assert_eq!(load(&path), config);
        // An absent file is the default config, not an error.
        assert_eq!(load(&dir.path().join("absent.toml")), Config::default());
    }

    /// Several folders survive a restart, **in order**, which is the whole
    /// point of the list: the order is scanned in, listed in, and resolved in.
    #[test]
    fn round_trips_several_music_folders_in_order() {
        let dirs = vec![
            PathBuf::from("/home/user/Music"),
            PathBuf::from("/mnt/nas/Archive"),
            PathBuf::from("/home/ünï çödé/曲"),
            PathBuf::from("/home/user/My \"Music\""),
        ];
        let config = Config {
            music_dirs: dirs.clone(),
            ..Config::default()
        };
        let text = config.to_toml();
        let back = Config::from_toml(&text);
        assert_eq!(
            back.music_dirs, dirs,
            "order and contents must both survive"
        );
        assert_eq!(back, config);
        // Reversing the list is a different config, or the order is not data.
        let reversed = Config {
            music_dirs: dirs.into_iter().rev().collect(),
            ..Config::default()
        };
        assert_ne!(Config::from_toml(&reversed.to_toml()), config);
    }

    /// **The silent migration.** A config written by a baz that held one folder
    /// is read as a one-folder list, and the next save writes the new key. A
    /// listener must not lose their library to a change in a file format.
    #[test]
    fn a_legacy_single_music_dir_migrates_silently_to_the_list() {
        let old = "music_dir = \"/home/user/Music\"\ngroup_key = \"year\"\n\
                   [replaygain]\nmode = \"album\"\npreamp_centidb = -350\n";
        let config = Config::from_toml(old);
        assert_eq!(config.music_dirs, vec![PathBuf::from("/home/user/Music")]);
        // Nothing else moved on the way through.
        assert_eq!(config.group_key, GroupKey::Year);
        assert_eq!(config.replay_gain.mode, ReplayGainMode::Album);
        assert_eq!(config.replay_gain.preamp_centidb, -350);

        // And the file baz writes back carries the new key and not the old one,
        // while naming the same folder.
        let text = config.to_toml();
        assert!(text.contains(MUSIC_DIRS), "{text}");
        let table: toml::Table = text.parse().expect("valid TOML");
        assert!(
            !table.contains_key(LEGACY_MUSIC_DIR),
            "the legacy key is not written back: {text}"
        );
        assert_eq!(Config::from_toml(&text).music_dirs, config.music_dirs);
    }

    /// The list wins where both keys are present — a file edited by hand, or
    /// one an older baz re-wrote beside a newer one's list.
    #[test]
    fn the_list_wins_over_the_legacy_key_when_a_document_carries_both() {
        let config =
            Config::from_toml("music_dir = \"/old\"\nmusic_dirs = [\"/new\", \"/newer\"]\n");
        assert_eq!(
            config.music_dirs,
            vec![PathBuf::from("/new"), PathBuf::from("/newer")]
        );
    }

    /// Per-key degradation, applied inside the list: one unusable entry costs
    /// itself and nothing around it, and a whole unusable list falls back to the
    /// legacy key rather than to nothing.
    #[test]
    fn an_unusable_folder_entry_degrades_alone() {
        // A number and a blank among three good folders.
        let config = Config::from_toml(
            "music_dirs = [\"/a\", 7, \"\", \"/b\", true, \"/c\"]\ngroup_key = \"genre\"\n",
        );
        assert_eq!(
            config.music_dirs,
            vec![
                PathBuf::from("/a"),
                PathBuf::from("/b"),
                PathBuf::from("/c")
            ]
        );
        assert_eq!(config.group_key, GroupKey::Genre);

        // A duplicate is dropped, keeping the first mention: one folder listed
        // twice would be walked twice for one set of rows.
        assert_eq!(
            Config::from_toml("music_dirs = [\"/a\", \"/b\", \"/a\"]\n").music_dirs,
            vec![PathBuf::from("/a"), PathBuf::from("/b")]
        );

        // A list that is not a list, and a list with nothing usable in it, both
        // fall back to the legacy key — the most conservative reading of a
        // damaged file is the one that keeps a folder baz can still see.
        for spoiled in [
            "music_dirs = 7",
            "music_dirs = []",
            "music_dirs = [3, \"\"]",
        ] {
            let text = format!("{spoiled}\nmusic_dir = \"/legacy\"\n");
            assert_eq!(
                Config::from_toml(&text).music_dirs,
                vec![PathBuf::from("/legacy")],
                "{spoiled}"
            );
        }
        // With no legacy key either, the answer is simply no folders.
        assert!(Config::from_toml("music_dirs = 7\n").music_dirs.is_empty());
    }

    /// One unrepresentable path costs itself and not the folders beside it —
    /// the array's version of the per-key rule the document already follows.
    #[cfg(unix)]
    #[test]
    fn a_non_utf8_folder_is_omitted_without_losing_the_others() {
        use std::os::unix::ffi::OsStringExt as _;
        let raw = std::ffi::OsString::from_vec(b"/music/\xFF\xFE".to_vec());
        let config = Config {
            music_dirs: vec![
                PathBuf::from("/first"),
                PathBuf::from(raw),
                PathBuf::from("/third"),
            ],
            ..Config::default()
        };
        let back = Config::from_toml(&config.to_toml());
        assert_eq!(
            back.music_dirs,
            vec![PathBuf::from("/first"), PathBuf::from("/third")]
        );
    }

    /// The written document is valid TOML by the crate's own reckoning, not
    /// merely by ours — the assembled-with-comments writer's standing check.
    #[test]
    fn the_written_document_parses_as_toml() {
        let config = Config {
            music_dirs: vec![PathBuf::from("/home/user/My \"Music\"")],
            replay_gain: settings(ReplayGainMode::Track, -1234, 567, false),
            group_key: GroupKey::Added,
            density: Density::Dense,
            sidebar_open: true,
        };
        let table: toml::Table = config.to_toml().parse().expect("baz writes valid TOML");
        assert!(table.contains_key(MUSIC_DIRS));
        assert!(table.contains_key(REPLAY_GAIN_TABLE));
        assert!(table.contains_key(GROUP_KEY));
        assert!(table.contains_key(DENSITY));
    }
}
