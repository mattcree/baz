//! **Is there a newer baz, and is it any of our business to say so?**
//!
//! ADR-0043 §3 and §5. The update *mechanism* is the platform's package
//! manager — Flathub, winget, Homebrew — and none of it is code baz owns.
//! What is left over is the person who took a tarball, a zip, or dragged the
//! `.app` across: no store knows they exist, and nothing will ever tell them
//! a new version shipped.
//!
//! # Two processes, and the split is the whole design
//!
//! The owner, 2026-08-20: *"honestly we don't need to show that a new version
//! in the app… we could just have a separate boot up script essentially, a
//! smaller exe which checks for updates and prompts to either install or not,
//! based on their config flag"*.
//!
//! So there are two callers and they are deliberately never the same process:
//!
//! - **`baz`, while somebody is listening**, uses [`check`] and
//!   [`fetch_verified`] to *stage* an update into [`stage`]'s directory. It
//!   draws nothing. A 186 MB disk image cannot be downloaded while a listener
//!   waits to hear music, so it is downloaded while they are already hearing
//!   it.
//! - **`baz-boot`, before baz starts**, finds that staged file and *offers*
//!   it. The offer is instant, because the download already happened, and
//!   accepting it works because **baz is not running**: an installer cannot
//!   replace a file the running application holds open, which is the whole
//!   reason a launcher exists rather than a button.
//!
//! # It hands off; it does not overwrite
//!
//! The owner, the same day: *"ideally we want to be able to update easily. as
//! in, the user just clicks something and the app updates"*. So baz downloads
//! and **verifies**, and then hands the verified file to the thing that
//! already knows how to install it — `msiexec` on Windows, the disk image on
//! macOS.
//!
//! That is not timidity, it is the shape that is both safer *and* more
//! familiar. A self-replacing binary fights the lock on the running
//! executable on Windows and breaks a bundle's signature on macOS; an
//! installer does neither, and it is what a listener on those platforms
//! expects a download to do anyway. The one step baz refuses to skip is the
//! **checksum**: nothing is opened, run, staged or handed anywhere until its
//! SHA-256 matches the `SHA256SUMS` published beside it.
//!
//! # What it cannot do, and does not pretend to
//!
//! **Inside a Flatpak nothing here runs at all**, because `/app` is read only
//! and the store already updates baz without being asked. The Flatpak ships
//! no launcher and its manifest grants no network. [`Route`] decides this,
//! and it is read from the filesystem rather than compiled in.
//!
//! **One flag, and it governs both halves.** `check_for_updates` in
//! `config.toml` is what the staging task reads and what the launcher reads,
//! and turning it off means neither runs. It is on by default, which is a
//! stated departure from *baz makes no network request unasked* rather than a
//! hidden one — the words beside the box say what it does, and the owner's
//! *"this could then be unchecked for anyone that doesn't want to"* is why
//! the box is there at all.
//!
//! # It has to know how it was installed
//!
//! Telling a Flatpak user *a new version is available, go and download it* is
//! telling them to break their own installation: their store already has it
//! and will offer it. So [`Route`] is read from the filesystem, and the
//! sentence changes with it. Getting this wrong is worse than saying nothing.

pub mod stage;

/// How this copy of baz got onto the machine, as far as it can tell.
///
/// Detected rather than compiled in, because one binary is shipped several
/// ways: the same `baz` inside a Flatpak is also the one inside the tarball.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
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
    pub fn detect() -> Self {
        if std::path::Path::new("/.flatpak-info").exists() {
            Self::Flatpak
        } else {
            Self::Standalone
        }
    }

    /// What to tell a listener, given a newer version.
    pub fn sentence(self, newer: &str) -> String {
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
    pub const fn can_install(self) -> bool {
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
pub const fn asset_suffix() -> &'static str {
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

/// **Can this platform's installer replace baz without being supervised?**
///
/// Windows and macOS publish an installer — an `.msi` and a `.dmg` — that a
/// listener already expects to double-click, and a launcher can hand either
/// one over with baz not running. That is the whole staged-update path.
///
/// **A Linux archive cannot be installed by anyone but the person holding
/// it.** Unpacking a `.tar.gz` over an existing installation is a decision
/// about a directory only they know the shape of, so there is nothing for a
/// launcher to do and none is shipped in that download. The blessed Linux
/// route is the Flatpak, which updates itself and needs none of this either.
///
/// This is the one predicate: it decides whether `baz` stages anything,
/// whether `baz-boot` is shipped at all, and which sentence Settings shows.
#[must_use]
pub const fn installs_itself() -> bool {
    cfg!(target_os = "windows") || cfg!(target_os = "macos")
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

/// **The releases endpoint.** Public, unauthenticated, and rate limited far
/// above once a day.
///
/// **Not `/releases/latest`, and the difference is the whole feature.** That
/// endpoint answers with the newest release GitHub considers *stable*, and it
/// excludes every prerelease and every draft. baz is pre-1.0 and
/// `.github/workflows/release.yml` marks every `0.*` tag a prerelease by
/// rule, so for the whole life of the project `/releases/latest` has answered
/// `404` — which the fetch below reads as *this repository has no releases* and
/// [`check`] reports as *you have the newest*. Every shipped baz has
/// therefore told its listener it was up to date however far behind it was,
/// and would have gone on doing so until 1.0.0. Proved by installing v0.4.0
/// and pressing the button: *"You have baz 0.4.0, which is the newest"*, with
/// v0.4.1 published.
///
/// The list endpoint holds no opinion about stability — prereleases are
/// simply in it. `per_page` bounds a document baz reads four fields out of;
/// it is not a page anybody will turn.
pub const ENDPOINT: &str = "https://api.github.com/repos/mattcree/baz/releases?per_page=20";

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
pub fn is_newer(candidate: &str, running: &str) -> bool {
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

/// **Every top-level release object in the array, as its own slice.**
///
/// The list endpoint returns many releases where the old one returned a
/// single object, and that changes what the small readers below are allowed
/// to see. [`tag_of`] and [`asset_url`] scan forwards for the first field
/// that matches; pointed at the whole array they would happily read the tag
/// of one release and the assets of another. So the array is cut into
/// elements first, and every field is read from inside exactly one of them.
///
/// Brace depth, with strings and their escapes respected, because a release
/// body is free text a listener wrote and may contain any bracket it likes.
/// Anything unbalanced simply yields fewer elements, which is silence.
fn releases(json: &str) -> Vec<&str> {
    let mut found = Vec::new();
    let mut depth = 0_usize;
    let mut start = 0_usize;
    let mut in_string = false;
    let mut escaped = false;
    for (at, byte) in json.bytes().enumerate() {
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'{' => {
                if depth == 0 {
                    start = at;
                }
                depth += 1;
            }
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    found.push(&json[start..=at]);
                }
            }
            _ => {}
        }
    }
    found
}

/// **Is this release a draft?**
///
/// An unauthenticated caller is not shown drafts at all, so in practice this
/// never fires — which is exactly why it is here. baz cannot see the auth
/// state of the request that produced a document, and *the endpoint would
/// never send me one* is an assumption about somebody else's server. A
/// release whose `draft` field is missing or is not a plain boolean is
/// **treated as a draft and skipped**, on the same rule the rest of this file
/// follows: every doubt resolves to silence.
fn is_draft(release: &str) -> bool {
    let Some(at) = release.find("\"draft\"") else {
        return true;
    };
    let rest = &release[at + "\"draft\"".len()..];
    let Some(colon) = rest.find(':') else {
        return true;
    };
    let rest = rest[colon + 1..].trim_start();
    !rest.starts_with("false")
}

/// **The newest published release in the array, and the tag that names it.**
///
/// *Newest* is the greatest version, not the most recently created. GitHub
/// returns the list in creation order, and creation order is not version
/// order the moment a patch is backported onto an older line — publishing
/// 0.4.2 after 0.5.0 would otherwise offer every listener a downgrade. The
/// comparison is the same version parse [`is_newer`] uses, so a tag this
/// cannot read is not a candidate.
#[must_use]
pub fn newest_published(json: &str) -> Option<(String, &str)> {
    releases(json)
        .into_iter()
        .filter(|release| !is_draft(release))
        .filter_map(|release| {
            let tag = tag_of(release)?;
            let version = parse(&tag)?;
            Some((version, tag, release))
        })
        .max_by_key(|(version, _, _)| *version)
        .map(|(_, tag, release)| (tag, release))
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
pub fn tag_of(json: &str) -> Option<String> {
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
pub fn asset_url(json: &str, suffix: &str) -> Option<String> {
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
pub fn published_sum(sums: &str, file_name: &str) -> Option<String> {
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
pub fn digest_matches(bytes: &[u8], published: &str) -> bool {
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
pub struct Update {
    /// The version, as the tag names it.
    pub version: String,
    /// Where this platform's installer lives.
    pub asset: String,
    /// Its file name, which is also the key into `SHA256SUMS`.
    pub file_name: String,
    /// Where the release's `SHA256SUMS` lives.
    pub sums: String,
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

/// **The tag names the release; a sentence names a version.**
///
/// Tags are written `v0.4.1`, and every line this reaches sets it beside a
/// bare `CARGO_PKG_VERSION`: *You have baz 0.4.0* directly above *baz v0.4.1
/// has been released* reads as two different kinds of thing. Only the
/// rendering changes — nothing downstream matches this against a tag, because
/// the asset and checksum URLs come out of the release document and the
/// staged marker only records what to say at the next start.
fn display_version(tag: &str) -> String {
    tag.strip_prefix('v').unwrap_or(tag).to_owned()
}

/// **Ask whether there is a newer baz, and where its installer is.**
///
/// Blocking, and meant for a worker thread. `Ok(None)` is the ordinary
/// answer — up to date, or a release whose shape this does not read.
///
/// # Errors
///
/// The network, or a response that is not what the endpoint documents.
pub fn check() -> Result<Option<Update>, String> {
    // **A repository with no release is up to date, not broken.** This is the
    // state baz itself is in until its first tag, and it is the state a fork
    // is in permanently.
    let Ok(body) = get_maybe(ENDPOINT)? else {
        return Ok(None);
    };
    let json = String::from_utf8_lossy(&body);
    // **One release is chosen before a single field is read out of it.** The
    // endpoint returns the list, and the assets named in it belong to whichever
    // release they sit inside; reading across the whole document would let baz
    // pair one release's tag with another release's installer.
    let Some((tag, release)) = newest_published(&json) else {
        return Ok(None);
    };
    if !is_newer(&tag, env!("CARGO_PKG_VERSION")) {
        return Ok(None);
    }
    let Some(asset) = asset_url(release, asset_suffix()) else {
        // A release with no installer for this platform is not an error and
        // not an update: it is a release we cannot install, and the honest
        // answer is silence.
        return Ok(None);
    };
    let Some(sums) = asset_url(release, "SHA256SUMS") else {
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
        version: display_version(&tag),
        asset,
        file_name,
        sums,
    }))
}

/// **Download the installer and prove it is the published one.**
///
/// Returns the path it was written to, which is [`stage`]'s directory: the
/// digest is compared before the file is written anywhere a listener could
/// run it, so a mismatch leaves nothing behind to be found later and mistaken
/// for a download, and what does get written is written with the marker that
/// lets the launcher prove it again at the next start.
///
/// # Errors
///
/// The network, a missing or malformed `SHA256SUMS` entry, a digest that does
/// not match, or a filesystem that will not take the file.
pub fn fetch_verified(update: &Update) -> Result<std::path::PathBuf, String> {
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

    stage::hold(
        &stage::Pending {
            version: update.version.clone(),
            file_name: update.file_name.clone(),
            sha256: published,
        },
        &bytes,
    )
}

/// **Does this listener want baz to look for updates at all?**
///
/// One key out of `config.toml`, read here rather than in `baz` because the
/// launcher needs the same answer and is not allowed to link the application
/// to get it. `crates/baz/src/config.rs` holds a test that the two readers
/// agree; the shape of that agreement is the point, not either copy.
///
/// **A config file that cannot be read means yes**, matching the default the
/// application itself writes. A listener who has never opened Settings has no
/// file, and *no file* must not mean *never tell me about a fix*.
#[must_use]
pub fn wanted() -> bool {
    let Some(path) = config_path() else {
        return true;
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return true;
    };
    wanted_in(&text)
}

/// **Where `config.toml` is**, by the same rule `crates/baz/src/config.rs`
/// uses — and pinned to it by a test over there, because two processes
/// disagreeing about which file holds the setting would be a tick-box that
/// does nothing.
#[must_use]
pub fn config_path() -> Option<std::path::PathBuf> {
    Some(dirs::config_dir()?.join("baz").join("config.toml"))
}

/// [`wanted`] over a `config.toml` already in hand.
///
/// Separated so it can be held against the application's own reader without
/// either of them touching an environment variable — see
/// `crates/baz/src/config.rs`.
#[must_use]
pub fn wanted_in(text: &str) -> bool {
    const KEY: &str = "check_for_updates";
    let Ok(table) = text.parse::<toml::Table>() else {
        return true;
    };
    table
        .get(KEY)
        .and_then(toml::Value::as_bool)
        .unwrap_or(true)
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
pub fn hand_off(path: &std::path::Path) -> Result<(), String> {
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
    // **On Linux baz applies the update itself** ([`applies_in_place`]), which
    // is the whole of the owner's ask: a button in Settings that updates baz,
    // not one that hands back a `.tar.gz`.
    if applies_in_place() {
        return install_in_place(path);
    }
    let (program, args): (&str, Vec<&std::ffi::OsStr>) = if cfg!(target_os = "windows") {
        ("msiexec", vec![std::ffi::OsStr::new("/i"), path.as_ref()])
    } else {
        ("open", vec![path.as_ref()])
    };
    std::process::Command::new(program)
        .args(args)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("could not start {program}: {error}"))
}

/// **Can baz apply an update to itself, here, now?**
///
/// Linux, and only Linux, because it is the one platform where a running
/// program can be replaced without ceremony: `rename(2)` over a busy
/// executable succeeds, the running process keeps the inode it started from,
/// and the new file takes the name. Windows locks the image and macOS wants a
/// bundle signature left intact, which is why both hand off to an installer at
/// the next start instead ([`installs_itself`]).
///
/// **This reverses ADR-0043's Linux answer, at the owner's instruction**
/// (2026-08-23): *"I can't update via the menu. I don't want that. I just want
/// to be able to update in there… I just want to update like the other apps."*
/// The ADR argued that unpacking an archive over an existing installation is a
/// decision about a directory only its owner knows the shape of. That was true
/// of a bare tarball and is not true of this one: the archive carries
/// `install.sh`, which knows the layout, writes a manifest and is the same
/// script that put baz there in the first place.
#[must_use]
pub const fn applies_in_place() -> bool {
    cfg!(target_os = "linux")
}

/// **Where a standalone Linux baz is installed**, derived from the running
/// binary.
///
/// `install.sh` places the binary at `<prefix>/bin/baz`, so the prefix is the
/// grandparent of the running executable. Anything that does not have that
/// shape — a binary run out of `target/release`, say — has no prefix baz may
/// write to, and the caller reports that rather than guessing at `~/.local`
/// and installing a second copy somewhere the listener is not running from.
fn install_prefix() -> Result<std::path::PathBuf, String> {
    let exe = std::env::current_exe().map_err(|why| format!("cannot find myself: {why}"))?;
    let exe = exe.canonicalize().unwrap_or(exe);
    let prefix = exe
        .parent()
        .filter(|bin| bin.file_name().is_some_and(|name| name == "bin"))
        .and_then(std::path::Path::parent)
        .ok_or_else(|| {
            format!(
                "baz is running from {}, which is not an installed copy — \
                 install it with the archive's install.sh first",
                exe.display()
            )
        })?;
    Ok(prefix.to_path_buf())
}

/// **Unpack the verified archive and run its own installer over this copy.**
///
/// The bytes have already been proved to be the published ones
/// ([`fetch_verified`]) before anything here runs; nothing is unpacked, and no
/// script is executed, until that has happened.
///
/// **The archive's `install.sh` does the placing, not this function.** It
/// knows the four directories a desktop looks in, it keeps the manifest
/// `uninstall.sh` reads, and it is the same script that made the installation
/// being replaced. Re-implementing that here would be a second layout to keep
/// in step with the first, and the two would drift on the first icon size
/// anybody added.
///
/// `tar` rather than a crate: it is on every Linux that can run baz, and the
/// alternative is two dependencies in a graph `deny.toml` keeps short on
/// purpose.
///
/// # Errors
///
/// A prefix that cannot be derived, an archive `tar` will not read, an
/// installer that is missing or fails, or a directory this process may not
/// write to.
fn install_in_place(archive: &std::path::Path) -> Result<(), String> {
    let prefix = install_prefix()?;
    let staging = archive.with_extension("unpacked");
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging)
        .map_err(|why| format!("could not make room to unpack: {why}"))?;
    let untar = std::process::Command::new("tar")
        .arg("-xzf")
        .arg(archive)
        .arg("-C")
        .arg(&staging)
        .status()
        .map_err(|why| format!("could not run tar: {why}"))?;
    if !untar.success() {
        return Err("the download could not be unpacked".to_owned());
    }
    // The archive holds exactly one top-level directory, named for the
    // release. Finding it rather than composing it means the version does not
    // have to be spelled twice.
    let root = std::fs::read_dir(&staging)
        .map_err(|why| format!("could not read the unpacked archive: {why}"))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| path.is_dir())
        .ok_or_else(|| "the download did not contain an installation".to_owned())?;
    let installer = root.join("install.sh");
    if !installer.exists() {
        return Err("the download carries no installer".to_owned());
    }
    let ran = std::process::Command::new("sh")
        .arg(&installer)
        .arg("--prefix")
        .arg(&prefix)
        .current_dir(&root)
        .status()
        .map_err(|why| format!("could not run the installer: {why}"))?;
    // Tidy up whether or not it worked: the archive is large and a failed
    // install leaves nothing behind that a retry would want.
    let _ = std::fs::remove_dir_all(&staging);
    if !ran.success() {
        return Err(format!(
            "the installer refused to write to {} — check the permissions there",
            prefix.display()
        ));
    }
    let _ = std::fs::remove_file(archive);
    Ok(())
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
pub fn handed_off_note() -> &'static str {
    if cfg!(target_os = "windows") {
        "The installer is running. Follow it, and quit baz when it asks — baz \
         will not close itself."
    } else if cfg!(target_os = "macos") {
        "The disk image is open. Drag baz onto Applications to replace this \
         version, then quit and reopen it. Gatekeeper will not object: the \
         download was checked against its published checksum and its \
         quarantine flag cleared."
    } else {
        "baz has been updated, and the download was checked against its \
         published checksum first. Quit and reopen to run the new version."
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

    /// A release list in GitHub's shape, newest first, every entry a
    /// prerelease — which is what this repository actually returns.
    const LIST: &str = r#"[
      {"tag_name":"v0.4.1","draft":false,"prerelease":true,"body":"a note with a { brace and a \\"quote\\"","assets":[
        {"browser_download_url":"https://github.com/mattcree/baz/releases/download/v0.4.1/baz-0.4.1-linux-x86_64.tar.gz"},
        {"browser_download_url":"https://github.com/mattcree/baz/releases/download/v0.4.1/SHA256SUMS"}]},
      {"tag_name":"v0.4.0","draft":false,"prerelease":true,"assets":[
        {"browser_download_url":"https://github.com/mattcree/baz/releases/download/v0.4.0/baz-0.4.0-linux-x86_64.tar.gz"},
        {"browser_download_url":"https://github.com/mattcree/baz/releases/download/v0.4.0/SHA256SUMS"}]}
    ]"#;

    /// **A prerelease is still a release, and this is the bug that shipped.**
    ///
    /// Every baz tag is `0.*`, and `release.yml` marks every `0.*` a
    /// prerelease, so `/releases/latest` answered `404` and a listener on
    /// v0.4.0 was told they had the newest while v0.4.1 sat published. The
    /// list endpoint has no such opinion. If this test ever goes quiet again,
    /// the update feature is dead and nothing else will say so.
    #[test]
    fn a_prerelease_is_still_a_release() {
        let (tag, release) = super::newest_published(LIST).expect("a published release");
        assert_eq!(tag, "v0.4.1");
        assert!(super::is_newer(&tag, "0.4.0"), "0.4.1 is newer than 0.4.0");
        assert!(
            super::asset_url(release, ".tar.gz")
                .is_some_and(|url| url.ends_with("baz-0.4.1-linux-x86_64.tar.gz")),
            "the newest release's own archive"
        );
    }

    /// **Every field comes out of one release**, never read across the array.
    ///
    /// The readers scan forwards for the first field that matches, so pointed
    /// at the whole document they would pair v0.4.1's tag with whichever
    /// asset appeared first. Handing a listener an installer from a different
    /// release than the one they were told about is the failure this guards.
    #[test]
    fn fields_are_read_out_of_one_release_and_not_across_the_array() {
        let (_, release) = super::newest_published(LIST).expect("a published release");
        for suffix in [".tar.gz", "SHA256SUMS"] {
            let url = super::asset_url(release, suffix).expect("an asset");
            assert!(
                url.contains("/download/v0.4.1/"),
                "{url} came from another release"
            );
        }
    }

    /// **Newest means the greatest version, not the most recent publish.**
    ///
    /// A patch backported onto an older line is created *after* the newer
    /// minor and GitHub lists it first. Taking the list's order would offer
    /// every 0.5.0 listener a downgrade to 0.4.2 and call it an update.
    #[test]
    fn creation_order_is_not_version_order() {
        let out_of_order = r#"[
          {"tag_name":"v0.4.2","draft":false,"prerelease":true,"assets":[]},
          {"tag_name":"v0.5.0","draft":false,"prerelease":true,"assets":[]}
        ]"#;
        let (tag, _) = super::newest_published(out_of_order).expect("a published release");
        assert_eq!(tag, "v0.5.0");
    }

    /// **A draft is not published, and doubt about that is also a draft.**
    ///
    /// An unauthenticated caller is never shown drafts, so this is a guard
    /// against an assumption rather than an observation — and a release whose
    /// `draft` field is missing or malformed is skipped on the same rule.
    #[test]
    fn a_draft_is_never_offered() {
        let with_draft = r#"[
          {"tag_name":"v0.9.0","draft":true,"prerelease":true,"assets":[]},
          {"tag_name":"v0.8.0","assets":[]},
          {"tag_name":"v0.4.1","draft":false,"prerelease":true,"assets":[]}
        ]"#;
        let (tag, _) = super::newest_published(with_draft).expect("a published release");
        assert_eq!(
            tag, "v0.4.1",
            "a draft or an unreadable draft field was taken"
        );
    }

    /// **Nothing published is silence, not an error.**
    ///
    /// A repository before its first tag, and a fork permanently. The list
    /// endpoint answers `[]` with a `200` where the old one answered `404`,
    /// so this is the case that used to arrive as [`super::NotFound`].
    #[test]
    fn an_empty_list_offers_nothing() {
        assert!(super::newest_published("[]").is_none());
        assert!(super::newest_published("").is_none());
    }

    /// **A listener is shown a version, never a tag.**
    ///
    /// The two sentences sit one above the other in Settings, and the running
    /// version arrives from `CARGO_PKG_VERSION` with no `v` on it.
    #[test]
    fn what_is_shown_is_a_version_and_not_a_tag() {
        assert_eq!(super::display_version("v0.4.1"), "0.4.1");
        assert_eq!(super::display_version("0.4.1"), "0.4.1");
    }

    /// **The one test that would have caught the bug that shipped.**
    ///
    /// Every fixture above passes against `/releases/latest` too — the defect
    /// was never in the reading, it was in which document was asked for, and
    /// only a real call can see that. `#[ignore]`d because CI does not go to
    /// the network; run it by hand after touching [`super::ENDPOINT`]:
    ///
    /// ```text
    /// cargo test -p baz-update -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "goes to the network; run by hand after touching ENDPOINT"]
    fn the_live_endpoint_still_answers_with_a_release() {
        let body = super::get_maybe(super::ENDPOINT)
            .expect("the releases endpoint")
            .unwrap_or_else(|super::NotFound| panic!("{} answered 404", super::ENDPOINT));
        let json = String::from_utf8_lossy(&body);
        let (tag, release) =
            super::newest_published(&json).expect("a published release on the real repository");
        println!("live: newest published is {tag}");
        assert!(
            super::is_newer(&tag, "0.4.0"),
            "{tag} should be newer than the v0.4.0 that was told it was current"
        );
        assert!(
            super::asset_url(release, "SHA256SUMS").is_some(),
            "a release baz would refuse for having no checksums"
        );
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
        // that baz is still running and that they have something to do. Since
        // 2026-08-23 Linux says *Quit and reopen* rather than *Unpack it*,
        // because baz now installs the update itself
        // ([`super::applies_in_place`]) — the act changed, so the words did.
        assert!(
            note.contains("uit") || note.contains("Unpack"),
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
        let source = include_str!("lib.rs").replace("\r\n", "\n");
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
