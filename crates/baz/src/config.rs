//! Minimal persistent configuration: the music directory, and nothing else.
//!
//! v0.1 persists exactly one thing — the last successfully opened music
//! folder — so a returning user lands straight on their shelf. The file is
//! `$XDG_CONFIG_HOME/baz/config.toml` (via the `dirs` crate), containing a
//! single `music_dir = "..."` key.
//!
//! # Why hand-rolled instead of a TOML crate
//!
//! The format is a strict subset of TOML (one key, basic-string value with
//! standard escapes), written and read by the tested functions below. Pulling
//! a full TOML parser for one key would be a dependency out of proportion to
//! the job; if the config ever grows past a couple of keys, switching to the
//! `toml` crate is the plan of record.
//!
//! # Limitation: UTF-8 paths only
//!
//! TOML strings are UTF-8, so a music directory whose path is not valid
//! UTF-8 cannot be persisted. Such a directory still works for the session
//! (paths are handled as `PathBuf` throughout); it just will not be
//! remembered across restarts. [`Config::to_toml`] returns `None` in that
//! case and the caller skips the write with a log line.

use std::io;
use std::path::{Path, PathBuf};

/// Application configuration. See the [module docs](self) for scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// The music folder baz scans and shelves on startup.
    pub music_dir: PathBuf,
}

impl Config {
    /// Serialize to the single-key TOML document this module writes.
    ///
    /// Returns `None` when `music_dir` is not valid UTF-8 (not representable
    /// in TOML — see the module docs).
    pub fn to_toml(&self) -> Option<String> {
        let dir = self.music_dir.to_str()?;
        Some(format!(
            "# baz configuration — written by baz, safe to edit\nmusic_dir = \"{}\"\n",
            escape(dir)
        ))
    }

    /// Parse a config document produced by [`Config::to_toml`] (or edited by
    /// hand within the same single-key subset). Unknown lines are ignored so
    /// a hand-added comment does not break loading. Returns `None` when no
    /// valid `music_dir` key is present.
    pub fn from_toml(text: &str) -> Option<Self> {
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            if key.trim() != "music_dir" {
                continue;
            }
            let value = value.trim();
            let inner = value.strip_prefix('"')?.strip_suffix('"')?;
            let dir = unescape(inner)?;
            if dir.is_empty() {
                return None;
            }
            return Some(Self {
                music_dir: PathBuf::from(dir),
            });
        }
        None
    }
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

/// Load the config from `path`; `None` if the file is missing or unparsable
/// (both mean "first run" to the caller — never an error dialog).
pub fn load(path: &Path) -> Option<Config> {
    let text = std::fs::read_to_string(path).ok()?;
    Config::from_toml(&text)
}

/// Persist `config` to `path`, creating parent directories as needed.
///
/// # Errors
///
/// Any filesystem error from creating the directory or writing the file.
/// A non-UTF-8 `music_dir` is reported as [`io::ErrorKind::InvalidData`]
/// (see the module docs on the UTF-8 limitation).
pub fn store(path: &Path, config: &Config) -> io::Result<()> {
    let text = config.to_toml().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "music_dir is not valid UTF-8; not persistable in config.toml",
        )
    })?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, text)
}

/// Escape a string for a TOML basic string (`"…"`).
fn escape(s: &str) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04X}", c as u32);
            }
            c => out.push(c),
        }
    }
    out
}

/// Inverse of [`escape`]; `None` on any malformed escape sequence.
fn unescape(s: &str) -> Option<String> {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next()? {
            '\\' => out.push('\\'),
            '"' => out.push('"'),
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            't' => out.push('\t'),
            'u' => {
                let hex: String = chars.by_ref().take(4).collect();
                if hex.len() != 4 {
                    return None;
                }
                let code = u32::from_str_radix(&hex, 16).ok()?;
                out.push(char::from_u32(code)?);
            }
            _ => return None,
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_plain_and_awkward_paths() {
        for dir in [
            "/home/user/Music",
            "/home/user/My \"Music\"",
            "/home/user/back\\slash",
            "/home/ünï çödé/曲",
            "/home/user/line\nbreak\tand\rreturn",
            "/home/user/ctrl\u{1}char",
        ] {
            let config = Config {
                music_dir: PathBuf::from(dir),
            };
            let text = config.to_toml().expect("UTF-8 path serializes");
            let back = Config::from_toml(&text).expect("parses back");
            assert_eq!(back, config, "round-trip failed for {dir:?}");
        }
    }

    #[test]
    fn parse_tolerates_comments_and_whitespace() {
        let text = "# a comment\n\n  music_dir   =   \"/m\"  \nfuture_key = 3\n";
        let config = Config::from_toml(text).expect("parses");
        assert_eq!(config.music_dir, PathBuf::from("/m"));
    }

    #[test]
    fn parse_rejects_garbage() {
        assert_eq!(Config::from_toml(""), None);
        assert_eq!(Config::from_toml("music_dir = unquoted"), None);
        assert_eq!(Config::from_toml("music_dir = \"\""), None);
        assert_eq!(Config::from_toml("music_dir = \"bad\\escape\\q\""), None);
        assert_eq!(Config::from_toml("other = \"x\""), None);
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_dir_is_not_serializable() {
        use std::os::unix::ffi::OsStringExt;
        let raw = std::ffi::OsString::from_vec(b"/music/\xFF\xFE".to_vec());
        let config = Config {
            music_dir: PathBuf::from(raw),
        };
        assert_eq!(config.to_toml(), None);
    }

    #[test]
    fn store_and_load_round_trip_on_disk() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("nested").join("config.toml");
        let config = Config {
            music_dir: PathBuf::from("/home/user/Music"),
        };
        store(&path, &config).expect("store creates parents and writes");
        assert_eq!(load(&path), Some(config));
        assert_eq!(load(&dir.path().join("absent.toml")), None);
    }
}
