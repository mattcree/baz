//! **Is there a newer baz, and is it any of our business to say so?**
//!
//! ADR-0043 §3. The update *mechanism* is the platform's package manager —
//! Flathub, winget, Homebrew — and none of it is code baz owns. What is left
//! over is the person who took a tarball, a zip, or dragged the `.app` across:
//! no store knows they exist, and nothing will ever tell them a new version
//! shipped.
//!
//! # It hands off; it does not overwrite
//!
//! The owner, 2026-08-20: *"ideally we want to be able to update easily. as in,
//! the user just clicks something and the app updates"*. So baz downloads and
//! **verifies**, and then hands the verified file to the thing that already
//! knows how to install it — `msiexec` on Windows, the disk image on macOS.
//!
//! That is not timidity, it is the shape that is both safer *and* more
//! familiar. A self-replacing binary fights the lock on the running
//! executable on Windows and breaks a bundle's signature on macOS; an
//! installer does neither, and it is what a listener on those platforms
//! expects a download to do anyway. The one step baz refuses to skip is the
//! **checksum**: nothing is opened, run or handed anywhere until its SHA-256
//! matches the `SHA256SUMS` published beside it.
//!
//! # What it cannot do, and does not pretend to
//!
//! **Inside a Flatpak there is no update button**, because `/app` is read
//! only and the store already updates baz without being asked. Drawing a
//! button that could not work — or that told somebody to go and download
//! something — would be worse than drawing nothing. [`Route`] decides this,
//! and it is read from the filesystem rather than compiled in.
//!
//! **Not on by default.** baz makes no network request in its life — that is a
//! property of a local music player, not an oversight — and it will not start
//! making one because a developer thought it would be handy. The setting is
//! off until a listener turns it on, and the words next to it say what it does.
//!
//! # It has to know how it was installed
//!
//! Telling a Flatpak user *a new version is available, go and download it* is
//! telling them to break their own installation: their store already has it
//! and will offer it. So [`Route`] is read from the filesystem, and the
//! sentence changes with it. Getting this wrong is worse than saying nothing.

/// How this copy of baz got onto the machine, as far as it can tell.
///
/// Detected rather than compiled in, because one binary is shipped several
/// ways: the same `baz` inside a Flatpak is also the one inside the tarball.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Route {
    /// Inside a Flatpak sandbox. The store owns the update.
    Flatpak,
    /// Anywhere else: a tarball, a zip, an MSI, a dragged bundle.
    ///
    /// The MSI and the DMG are updated by their own package managers where a
    /// listener used one, and by hand where they did not — and baz cannot tell
    /// those apart, so it says the thing that is true either way: *a new
    /// version exists, here is where it lives*.
    Standalone,
}

impl Route {
    /// Read the route from the running process.
    ///
    /// `/.flatpak-info` exists in every Flatpak sandbox and nowhere else; it
    /// is the check `flatpak` itself documents for exactly this question.
    pub(crate) fn detect() -> Self {
        if std::path::Path::new("/.flatpak-info").exists() {
            Self::Flatpak
        } else {
            Self::Standalone
        }
    }

    /// What to tell a listener, given a newer version.
    pub(crate) fn sentence(self, newer: &str) -> String {
        match self {
            Self::Flatpak => format!(
                "baz {newer} has been released. Your software centre will \
                 offer it."
            ),
            Self::Standalone => format!("baz {newer} has been released."),
        }
    }

    /// **Whether baz can install the update itself.**
    ///
    /// Inside a Flatpak it cannot and must not offer to: `/app` is read only,
    /// and the store updates baz without being asked. A button that could not
    /// work is worse than no button.
    pub(crate) const fn can_install(self) -> bool {
        matches!(self, Self::Standalone)
    }
}

/// **Which published file this platform installs from.**
///
/// The suffix, not the whole name, because the name carries a version this
/// code has just learned and should not have to rebuild. The release publishes
/// an archive *and* an installer for every platform; this names the installer,
/// because handing a listener a `.tar.gz` is handing them the problem back.
#[must_use]
pub(crate) const fn asset_suffix() -> &'static str {
    if cfg!(target_os = "windows") {
        ".msi"
    } else if cfg!(target_os = "macos") {
        ".dmg"
    } else {
        // A Linux listener who is *not* in a Flatpak took the archive, and the
        // archive is what they can be given again. `Route::can_install` has
        // already excluded the sandbox by the time this is asked.
        ".tar.gz"
    }
}

// **There is no interval, because there is no automatic check.**
//
// An earlier draft had one — once a day, behind a setting. The press *is* the
// consent, and it is unambiguous in a way a checkbox somebody ticked months
// ago is not: baz reaches the network when a listener asks it to and at no
// other moment. That also means there is no clock to guard, no setting to
// explain, and no state to persist, which is a smaller product for the same
// two clicks.
//
// If an automatic check is ever wanted, this is where the interval goes, and
// the setting to go with it.

/// The releases endpoint. Public, unauthenticated, and rate limited far above
/// once a day.
pub(crate) const ENDPOINT: &str = "https://api.github.com/repos/mattcree/baz/releases/latest";

/// **Is `candidate` newer than `running`?**
///
/// A three-part numeric compare over `MAJOR.MINOR.PATCH`, with a leading `v`
/// tolerated because that is how the tags are written and the API echoes them.
///
/// **Anything it cannot parse is not newer.** A pre-release suffix, a fourth
/// component, a tag somebody typed by hand — every one of those returns
/// `false`, because the only cost of missing a release is that a listener
/// hears about it a version later, and the cost of a false positive is baz
/// telling somebody their current version is out of date when it is not.
#[must_use]
pub(crate) fn is_newer(candidate: &str, running: &str) -> bool {
    let Some(candidate) = parse(candidate) else {
        return false;
    };
    let Some(running) = parse(running) else {
        return false;
    };
    candidate > running
}

/// `MAJOR.MINOR.PATCH` as three numbers, or `None`.
fn parse(version: &str) -> Option<(u32, u32, u32)> {
    let version = version.trim();
    let version = version.strip_prefix('v').unwrap_or(version);
    let mut parts = version.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    // A fourth part, or a suffix the split left behind, means this is not the
    // shape we know. Refuse rather than guess.
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

/// The `tag_name` out of GitHub's release JSON, without a JSON parser.
///
/// One string field out of a document baz never otherwise looks at, from an
/// endpoint whose shape is stable and public. Pulling `serde_json`'s full
/// derive machinery across the wire for `"tag_name": "v0.4.0"` would be a
/// dependency in the graph for one line — and `deny.toml` is a file this
/// project keeps short on purpose.
///
/// It is deliberately strict: a field it does not find exactly is `None`, and
/// `None` is silence.
#[must_use]
pub(crate) fn tag_of(json: &str) -> Option<String> {
    let at = json.find("\"tag_name\"")?;
    let rest = &json[at + "\"tag_name\"".len()..];
    let colon = rest.find(':')?;
    let rest = &rest[colon + 1..];
    let open = rest.find('"')?;
    let rest = &rest[open + 1..];
    let close = rest.find('"')?;
    let tag = &rest[..close];
    // A tag with an escape in it is not a version number; a tag longer than a
    // version number is not one either. Both are refused rather than shown.
    if tag.is_empty() || tag.len() > 32 || tag.contains('\\') {
        return None;
    }
    Some(tag.to_owned())
}

/// **The download URL for this platform's installer**, out of the release
/// document.
///
/// Read with the same deliberately small reader [`tag_of`] uses, and held to
/// the same rule: anything it is not certain of is `None`, and `None` is a
/// button that does not appear.
///
/// It matches on the *suffix* and on the host it came from. A release asset
/// URL that does not live on `github.com` is refused outright — the document
/// is fetched over TLS from GitHub, but a field inside a document is data, and
/// data that names where to send a listener is exactly the field an attacker
/// would want to control.
#[must_use]
pub(crate) fn asset_url(json: &str, suffix: &str) -> Option<String> {
    const HOSTS: [&str; 2] = [
        "https://github.com/mattcree/baz/releases/download/",
        "https://objects.githubusercontent.com/",
    ];
    let mut rest = json;
    while let Some(at) = rest.find("\"browser_download_url\"") {
        rest = &rest[at + "\"browser_download_url\"".len()..];
        let Some(colon) = rest.find(':') else { break };
        let after = &rest[colon + 1..];
        let Some(open) = after.find('"') else { break };
        let after = &after[open + 1..];
        let Some(close) = after.find('"') else { break };
        let url = &after[..close];
        rest = &after[close..];
        if url.ends_with(suffix) && HOSTS.iter().any(|host| url.starts_with(host)) {
            return Some(url.to_owned());
        }
    }
    None
}

/// **The published SHA-256 for one file**, out of the release's `SHA256SUMS`.
///
/// The format is `sha256sum`'s own: sixty-four hex characters, two spaces, the
/// file name. Nothing else is accepted — not a short digest, not upper case
/// mixed with lower, not a line whose name merely *contains* the one asked
/// for. A checksum reader that is generous is a checksum reader that can be
/// talked into agreeing.
#[must_use]
pub(crate) fn published_sum(sums: &str, file_name: &str) -> Option<String> {
    for line in sums.lines() {
        let line = line.trim();
        let Some((digest, name)) = line.split_once("  ") else {
            continue;
        };
        if name != file_name {
            continue;
        }
        let digest = digest.trim();
        if digest.len() == 64 && digest.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Some(digest.to_ascii_lowercase());
        }
    }
    None
}

/// **Does what we downloaded match what was published?**
///
/// The one step that is never skipped. baz opens, runs and hands off nothing
/// whose digest it has not compared — and the comparison is
/// constant-time-irrelevant but case-insensitive, because `sha256sum` and
/// GitHub disagree about case and neither is wrong.
#[must_use]
pub(crate) fn digest_matches(bytes: &[u8], published: &str) -> bool {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let got = hasher.finalize();
    let got = got.iter().fold(String::with_capacity(64), |mut acc, byte| {
        use std::fmt::Write as _;
        let _ = write!(acc, "{byte:02x}");
        acc
    });
    got == published.trim().to_ascii_lowercase()
}

/// The most baz will read from the network, in bytes.
///
/// The largest thing a release publishes is a Flatpak bundle at around 150 MB
/// and the installers are a tenth of that, so 512 MB is far above anything
/// legitimate and far below anything that would fill a disk while a listener
/// watched a spinner. A cap is not paranoia here: the length a server declares
/// is a claim, and this is what happens when the claim is a lie.
const CEILING: u64 = 512 * 1024 * 1024;

/// What a completed check found.
#[derive(Debug, Clone)]
pub(crate) struct Update {
    /// The version, as the tag names it.
    pub(crate) version: String,
    /// Where this platform's installer lives.
    pub(crate) asset: String,
    /// Its file name, which is also the key into `SHA256SUMS`.
    pub(crate) file_name: String,
    /// Where the release's `SHA256SUMS` lives.
    pub(crate) sums: String,
}

/// A GET that answered `404`.
///
/// Its own error because on the releases endpoint it is **not a failure**: it
/// is what GitHub says about a repository with no published release, which is
/// exactly the state baz is in before its first one. Reporting that in the
/// alert ink as *"could not reach …: http status: 404"* is baz telling a
/// listener something went wrong when nothing did — found by pressing the
/// button, which is the only way it would ever have been found.
#[derive(Debug)]
struct NotFound;

/// One GET, with the two headers GitHub wants and a cap on what comes back.
fn get_maybe(url: &str) -> Result<Result<Vec<u8>, NotFound>, String> {
    let agent = ureq::Agent::new_with_defaults();
    let response = agent
        .get(url)
        // GitHub refuses an unidentified client, and an honest agent string
        // is also what lets them see baz in their logs and rate-limit it as
        // one thing rather than as anonymous noise.
        .header("User-Agent", concat!("baz/", env!("CARGO_PKG_VERSION")))
        .header("X-GitHub-Api-Version", "2022-11-28")
        .call();
    let mut response = match response {
        Ok(response) => response,
        Err(ureq::Error::StatusCode(404)) => return Ok(Err(NotFound)),
        Err(error) => return Err(format!("could not reach {url}: {error}")),
    };
    response
        .body_mut()
        .with_config()
        .limit(CEILING)
        .read_to_vec()
        .map(Ok)
        .map_err(|error| format!("could not read {url}: {error}"))
}

/// [`get_maybe`], where a missing file *is* a failure — the asset and the
/// checksums, both of which the release document has just promised exist.
fn get(url: &str) -> Result<Vec<u8>, String> {
    get_maybe(url)?.map_err(|NotFound| format!("{url} is not there"))
}

/// **Ask whether there is a newer baz, and where its installer is.**
///
/// Blocking, and meant for a worker thread. `Ok(None)` is the ordinary
/// answer — up to date, or a release whose shape this does not read.
///
/// # Errors
///
/// The network, or a response that is not what the endpoint documents.
pub(crate) fn check() -> Result<Option<Update>, String> {
    // **A repository with no release is up to date, not broken.** This is the
    // state baz itself is in until its first tag, and it is the state a fork
    // is in permanently.
    let Ok(body) = get_maybe(ENDPOINT)? else {
        return Ok(None);
    };
    let json = String::from_utf8_lossy(&body);
    let Some(tag) = tag_of(&json) else {
        return Ok(None);
    };
    if !is_newer(&tag, env!("CARGO_PKG_VERSION")) {
        return Ok(None);
    }
    let Some(asset) = asset_url(&json, asset_suffix()) else {
        // A release with no installer for this platform is not an error and
        // not an update: it is a release we cannot install, and the honest
        // answer is silence.
        return Ok(None);
    };
    let Some(sums) = asset_url(&json, "SHA256SUMS") else {
        // **No checksums, no update.** The verification is not a nicety that
        // degrades to a warning; without it there is nothing to verify
        // against and baz will not hand an unverified file to an installer.
        return Ok(None);
    };
    let file_name = asset.rsplit('/').next().unwrap_or_default().to_owned();
    if file_name.is_empty() {
        return Ok(None);
    }
    Ok(Some(Update {
        version: tag,
        asset,
        file_name,
        sums,
    }))
}

/// **Download the installer and prove it is the published one.**
///
/// Returns the path it was written to. The digest is compared before the file
/// is written anywhere a listener could run it, so a mismatch leaves nothing
/// behind to be found later and mistaken for a download.
///
/// # Errors
///
/// The network, a missing or malformed `SHA256SUMS` entry, a digest that does
/// not match, or a filesystem that will not take the file.
pub(crate) fn fetch_verified(update: &Update) -> Result<std::path::PathBuf, String> {
    let sums = get(&update.sums)?;
    let sums = String::from_utf8_lossy(&sums);
    let published = published_sum(&sums, &update.file_name)
        .ok_or_else(|| format!("{} is not listed in SHA256SUMS", update.file_name))?;

    let bytes = get(&update.asset)?;
    if !digest_matches(&bytes, &published) {
        return Err(format!(
            "{} did not match its published checksum and was discarded",
            update.file_name
        ));
    }

    let dir = std::env::temp_dir().join("baz-update");
    std::fs::create_dir_all(&dir).map_err(|error| format!("{}: {error}", dir.display()))?;
    let path = dir.join(&update.file_name);
    std::fs::write(&path, &bytes).map_err(|error| format!("{}: {error}", path.display()))?;
    Ok(path)
}

/// **Hand the verified file to the thing that installs it.**
///
/// Not a self-replacement: `msiexec` on Windows and the desktop's own opener
/// elsewhere, because an installer does not fight the lock on a running
/// executable and does not break a bundle's signature — and on those
/// platforms it is what a listener expects a download to do anyway.
///
/// # Errors
///
/// A platform tool that will not start.
pub(crate) fn hand_off(path: &std::path::Path) -> Result<(), String> {
    // **macOS: take the quarantine flag off first, and only here.**
    //
    // Gatekeeper attaches `com.apple.quarantine` to anything a *browser*
    // downloads, and it propagates from a disk image to whatever is dragged
    // out of it — so an unsigned baz dragged from a downloaded DMG refuses to
    // open with *"baz is damaged and can't be opened"*, which is macOS'
    // message for this case and not a statement about the file.
    // `docs/INSTALL.md` currently asks a listener to clear it by hand. This is
    // the same act, done for them.
    //
    // It is defensible **only because of the line above it**: these bytes have
    // already been proved to be the ones published beside the release's own
    // checksums. Stripping quarantine from an unverified download would be
    // taking off the one guard macOS supplies; stripping it from a verified
    // one is completing a check macOS cannot perform because baz is not
    // signed (ADR-0043 §4). If baz is ever signed and notarised, this comes
    // out — Gatekeeper will pass it on its own and the flag is then doing its
    // job rather than blocking one.
    if cfg!(target_os = "macos") {
        // Best effort: an image with no such attribute is the ordinary case
        // for a file baz wrote itself, and `xattr` reports that as a failure.
        let _ = std::process::Command::new("xattr")
            .arg("-dr")
            .arg("com.apple.quarantine")
            .arg(path)
            .status();
    }
    let (program, args): (&str, Vec<&std::ffi::OsStr>) = if cfg!(target_os = "windows") {
        ("msiexec", vec![std::ffi::OsStr::new("/i"), path.as_ref()])
    } else if cfg!(target_os = "macos") {
        ("open", vec![path.as_ref()])
    } else {
        ("xdg-open", vec![path.as_ref()])
    };
    std::process::Command::new(program)
        .args(args)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("could not start {program}: {error}"))
}

/// **What just happened, in the words of the platform it happened on.**
///
/// The three hand-offs do genuinely different things and a single sentence
/// could only be right about one of them: Windows starts an installer that
/// asks questions, macOS opens a window a listener drags from, and a Linux
/// archive is handed to whatever the desktop opens archives with. Telling a
/// Mac listener "the installer has been handed the download" when what
/// appeared is a Finder window is baz describing something they cannot see.
#[must_use]
pub(crate) fn handed_off_note() -> &'static str {
    if cfg!(target_os = "windows") {
        "The installer is running. Follow it, and quit baz when it asks — baz \
         will not close itself."
    } else if cfg!(target_os = "macos") {
        "The disk image is open. Drag baz onto Applications to replace this \
         version, then quit and reopen it. Gatekeeper will not object: the \
         download was checked against its published checksum and its \
         quarantine flag cleared."
    } else {
        "The archive has been downloaded and checked against its published \
         checksum. Unpack it over your existing baz, then quit and reopen."
    }
}

#[cfg(test)]
mod tests {
    use super::{Route, is_newer, tag_of};

    /// **Newer means newer, and everything else means silence.**
    ///
    /// The asymmetry is the decision: missing a release costs a listener
    /// nothing but a version's delay, and a false positive tells somebody
    /// their up-to-date player is out of date. So every doubt resolves to
    /// `false`.
    #[test]
    fn only_a_plainly_greater_version_is_newer() {
        for (candidate, running) in [
            ("0.4.0", "0.3.0"),
            ("v0.4.0", "0.3.0"),
            ("0.3.1", "0.3.0"),
            ("1.0.0", "0.99.99"),
            ("0.10.0", "0.9.0"),
        ] {
            assert!(is_newer(candidate, running), "{candidate} over {running}");
        }
        for (candidate, running) in [
            ("0.3.0", "0.3.0"),
            ("0.2.9", "0.3.0"),
            ("v0.3.0", "v0.3.0"),
            // Shapes we do not read. A pre-release, a fourth part, a word.
            ("0.4.0-rc1", "0.3.0"),
            ("0.4.0.1", "0.3.0"),
            ("nightly", "0.3.0"),
            ("", "0.3.0"),
            ("0.4", "0.3.0"),
        ] {
            assert!(
                !is_newer(candidate, running),
                "{candidate} was called newer than {running}"
            );
        }
    }

    /// **The version compare is numeric, not lexical.**
    ///
    /// The bug this exists to prevent is the one every hand-rolled version
    /// check ships with: `"0.10.0" < "0.9.0"` as strings, so the tenth minor
    /// release is silently older than the ninth and nobody is ever told again.
    #[test]
    fn ten_is_greater_than_nine() {
        assert!(is_newer("0.10.0", "0.9.0"));
        assert!(is_newer("0.3.10", "0.3.9"));
        assert!(is_newer("10.0.0", "9.0.0"));
        assert!(!is_newer("0.9.0", "0.10.0"));
    }

    /// **One field out of the release document, and nothing else read.**
    #[test]
    fn the_tag_is_read_out_of_the_release_document() {
        assert_eq!(
            tag_of(r#"{"url":"…","tag_name":"v0.4.0","name":"baz 0.4.0"}"#).as_deref(),
            Some("v0.4.0")
        );
        assert_eq!(
            tag_of("{ \"tag_name\" :  \"0.4.0\" }").as_deref(),
            Some("0.4.0")
        );
        for refused in [
            "{}",
            r#"{"name":"v0.4.0"}"#,
            r#"{"tag_name":""}"#,
            r#"{"tag_name":"v0.4.0\u0000"}"#,
            &format!(r#"{{"tag_name":"{}"}}"#, "9".repeat(40)),
        ] {
            assert_eq!(tag_of(refused), None, "{refused} was read as a tag");
        }
    }

    /// **A download URL is data, and data does not get to say where we go.**
    ///
    /// The release document arrives over TLS from GitHub, which authenticates
    /// *the document* and says nothing about a field inside it. The field that
    /// names where to send a listener is precisely the one an attacker would
    /// want, so it is matched against the hosts a GitHub release asset can
    /// actually live on, and anything else is refused — which means no button.
    #[test]
    fn an_asset_url_must_be_a_github_release_asset() {
        let good = r#"{"assets":[
          {"browser_download_url":"https://github.com/mattcree/baz/releases/download/v0.4.0/baz-0.4.0-windows-x86_64.msi"},
          {"browser_download_url":"https://github.com/mattcree/baz/releases/download/v0.4.0/baz-0.4.0-linux-x86_64.tar.gz"}
        ]}"#;
        assert!(
            super::asset_url(good, ".msi").is_some_and(|url| url.ends_with("windows-x86_64.msi"))
        );
        assert!(
            super::asset_url(good, ".tar.gz")
                .is_some_and(|url| url.ends_with("linux-x86_64.tar.gz"))
        );
        assert_eq!(
            super::asset_url(good, ".dmg"),
            None,
            "a suffix with no asset"
        );

        for hostile in [
            r#"{"browser_download_url":"https://evil.example/baz.msi"}"#,
            r#"{"browser_download_url":"http://github.com/mattcree/baz/releases/download/v1/x.msi"}"#,
            r#"{"browser_download_url":"https://github.com.evil.example/mattcree/baz/releases/download/v1/x.msi"}"#,
            r#"{"browser_download_url":"file:///etc/passwd.msi"}"#,
        ] {
            assert_eq!(
                super::asset_url(hostile, ".msi"),
                None,
                "followed a URL off GitHub: {hostile}"
            );
        }
    }

    /// **A checksum reader that is generous can be talked into agreeing.**
    ///
    /// So this one is not: `sha256sum`'s exact format, a full-length hex
    /// digest, and an *equal* file name rather than one that merely contains
    /// what was asked for — otherwise `baz-0.4.0-linux-x86_64.tar.gz.sig`
    /// answers for `baz-0.4.0-linux-x86_64.tar.gz`.
    #[test]
    fn a_published_sum_is_read_exactly_or_not_at_all() {
        let sums = "\
d2a84f4b8b650937ec8f73cd8be2c74add5a911ba64df27458ed8229da804a26  baz-0.4.0-linux-x86_64.tar.gz\n\
0000000000000000000000000000000000000000000000000000000000000000  baz-0.4.0-windows-x86_64.msi\n";
        assert_eq!(
            super::published_sum(sums, "baz-0.4.0-linux-x86_64.tar.gz").as_deref(),
            Some("d2a84f4b8b650937ec8f73cd8be2c74add5a911ba64df27458ed8229da804a26")
        );
        for absent in [
            "baz-0.4.0-linux-x86_64.tar",    // a prefix
            "0.4.0-linux-x86_64.tar.gz",     // a suffix
            "baz-0.5.0-linux-x86_64.tar.gz", // another version
        ] {
            assert_eq!(super::published_sum(sums, absent), None, "{absent} matched");
        }
        // A short digest is not a digest.
        assert_eq!(super::published_sum("dead  a.msi", "a.msi"), None);
        // Nor is one with a non-hex character in it.
        let nearly = format!("{}z  a.msi", "0".repeat(63));
        assert_eq!(super::published_sum(&nearly, "a.msi"), None);
    }

    /// **Nothing is opened, run or handed on whose digest did not match.**
    ///
    /// The known-answer test is the empty string's SHA-256, which is the one
    /// digest worth hard-coding: if this ever disagrees, the hasher is wrong
    /// rather than the test.
    #[test]
    fn a_download_is_compared_against_what_was_published() {
        const EMPTY: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        assert!(super::digest_matches(b"", EMPTY));
        assert!(super::digest_matches(b"", &EMPTY.to_ascii_uppercase()));
        assert!(!super::digest_matches(b"x", EMPTY));
        assert!(!super::digest_matches(b"", ""));
        assert!(!super::digest_matches(b"", &EMPTY[..63]));
    }

    /// **Each platform is told what actually appeared in front of it.**
    ///
    /// The three hand-offs do different things — Windows starts an installer
    /// that asks questions, macOS opens a window a listener drags from, Linux
    /// hands an archive to the desktop — and one sentence could only be right
    /// about one of them. Telling a Mac listener *the installer is running*
    /// when what appeared is a Finder window is baz describing something they
    /// cannot see.
    ///
    /// Asserted against the *running* platform rather than all three, because
    /// `cfg!` is resolved at compile time and the other two branches do not
    /// exist in this binary. What the test can hold is that this one is about
    /// the thing that will actually happen here.
    #[test]
    fn the_hand_off_describes_what_this_platform_will_show() {
        let note = super::handed_off_note();
        assert!(!note.is_empty());
        // Whatever the platform, the sentence has to leave a listener knowing
        // that baz is still running and that they have something to do.
        assert!(
            note.contains("quit") || note.contains("Unpack"),
            "the note does not say what to do next: {note}"
        );
        if cfg!(target_os = "macos") {
            assert!(note.contains("Drag"), "{note}");
            assert!(
                !note.contains("installer is running"),
                "a Mac listener is told about an installer they will not see"
            );
        }
        if cfg!(target_os = "windows") {
            assert!(note.contains("installer"), "{note}");
            assert!(!note.contains("Drag"), "{note}");
        }
    }

    /// **The quarantine flag is cleared only after the checksum matched.**
    ///
    /// Stripping `com.apple.quarantine` from an *unverified* download would
    /// take off the one guard macOS supplies for an unsigned application.
    /// Stripping it from a verified one completes a check macOS cannot
    /// perform, because baz is not signed. The order is the whole argument,
    /// so it is pinned in the source rather than left to a reader's memory.
    #[test]
    fn quarantine_is_cleared_after_verification_and_not_before() {
        let source = include_str!("release.rs").replace("\r\n", "\n");
        let shipped = source
            .split("#[cfg(test)]")
            .next()
            .expect("a source has a head");
        let verify = shipped
            .find("fn fetch_verified")
            .expect("the verification exists");
        let strip = shipped
            .find("com.apple.quarantine")
            .expect("the quarantine strip exists");
        assert!(
            verify < strip,
            "the quarantine flag is cleared before the checksum is compared"
        );
        // And it lives in the hand-off, which only runs on a verified path.
        let rest = &shipped[shipped.find("fn hand_off").expect("the hand-off")..];
        assert!(
            rest.contains("com.apple.quarantine"),
            "the strip has moved out of the hand-off"
        );
    }

    /// **Inside a Flatpak there is no update button.**
    ///
    /// `/app` is read only, so a button could not work; and the store updates
    /// baz without being asked, so it does not need to. Drawing one anyway
    /// would be an affordance that fails or a link that tells somebody to
    /// break their own installation.
    #[test]
    fn a_sandboxed_baz_does_not_offer_to_install_anything() {
        assert!(!Route::Flatpak.can_install());
        assert!(Route::Standalone.can_install());
    }

    /// **A Flatpak listener is never told to go and download something.**
    ///
    /// Their store already has it and will offer it; sending them to a
    /// releases page is sending them to break their own installation. This is
    /// the one thing in this module it would be actively harmful to get
    /// wrong, so it is asserted as an absence.
    #[test]
    fn the_sentence_matches_how_baz_was_installed() {
        let inside = Route::Flatpak.sentence("0.4.0");
        assert!(inside.contains("software centre"), "{inside}");
        assert!(
            !inside.contains("github.com") && !inside.to_lowercase().contains("download"),
            "a Flatpak listener was pointed at a download: {inside}"
        );

        // **And a standalone listener is not sent anywhere either**, because
        // there is a button. A sentence naming a download page beside a
        // control that performs the download would be two answers to one
        // question, and a listener would have to work out which is real.
        let outside = Route::Standalone.sentence("0.4.0");
        assert!(
            !outside.contains("github.com") && !outside.to_lowercase().contains("download"),
            "the update sentence sends somebody to a page instead of to the \
             button beside it: {outside}"
        );
        for sentence in [&inside, &outside] {
            assert!(
                sentence.contains("0.4.0"),
                "the sentence does not name the version: {sentence}"
            );
        }
    }
}
