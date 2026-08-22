//! **`baz-boot` — the thing your Start-menu entry actually points at.**
//!
//! ADR-0043 §5. The owner, 2026-08-20: *"honestly we don't need to show that a
//! new version in the app… we could just have a separate boot up script
//! essentially, a smaller exe which checks for updates and prompts to either
//! install or not, based on their config flag"*.
//!
//! # Why a second executable is not an indulgence
//!
//! **An installer cannot replace a file the running application holds open.**
//! That one sentence is the whole argument. baz's own update button downloads
//! and verifies perfectly well and then reaches the sentence *"quit baz when
//! the installer asks"* — a chore, handed to somebody whose only crime was
//! pressing a button that said it would update their music player.
//!
//! Nothing in this process is baz. So when the answer is yes, `msiexec` gets
//! a field with nothing standing in it, replaces the application, and the
//! listener starts baz again from the same Start-menu entry they always use.
//! When the answer is no — or there is nothing to ask about, which is almost
//! every launch — this process replaces itself with baz and is gone before
//! the window appears.
//!
//! # What it does not do
//!
//! **It does not touch the network.** The download already happened, in the
//! previous session, while somebody was listening to something
//! (`crate::stage`). A launcher that fetched 190 MB would be a music player
//! that takes a minute to start on the day a release lands, which is the
//! worst possible day for it.
//!
//! **It does not decide anything on its own.** `check_for_updates` in
//! `config.toml` governs both halves: off means baz stages nothing and this
//! offers nothing.
//!
//! **It draws nothing of its own.** The one dialogue is the platform's — the
//! task dialog on Windows, the alert on macOS, `zenity` under a desktop that
//! has it. A launcher carrying a GUI toolkit to ask one question would be
//! most of a second application, and it would be the wrong one: this is the
//! operating system talking about installing software, which is exactly the
//! voice it should be in. Where there is no such tool the question is not
//! asked and baz simply starts, because a music player that will not open
//! because it could not find `zenity` is a far worse failure than a missed
//! update.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use baz_update::{Route, is_newer, stage};

/// The application this launcher exists to launch.
const BAZ: &str = if cfg!(target_os = "windows") {
    "baz.exe"
} else {
    "baz"
};

fn main() -> ExitCode {
    let Some(player) = beside_me(BAZ) else {
        return complain("baz-boot could not work out where it is installed.");
    };
    if !player.exists() {
        return complain(&format!(
            "baz is missing from {}. Reinstall baz to repair it.",
            player.parent().unwrap_or(&player).display()
        ));
    }
    if offer_is_wanted()
        && let Some((version, installer)) = waiting()
        && accepted(&version)
    {
        // **And this process stops here.** baz is deliberately not started:
        // the installer is about to replace it, and starting the version
        // being replaced is how a listener ends up with an installer
        // complaining about a file in use — the exact failure this whole
        // executable exists to remove.
        return match baz_update::hand_off(&installer) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => complain(&format!("The update could not be started.\n\n{error}")),
        };
    }
    launch(&player)
}

/// **Is anyone interested in being asked?**
///
/// Three answers that are all *no* long before the disk is touched: a listener
/// who unticked the box; a Flatpak — where `/app` is read only, the store owns
/// the update, and the manifest grants no network for the staging half to have
/// used in the first place; and a platform whose download is an archive rather
/// than an installer, which ships no launcher at all and where this would open
/// a file manager and call it an update.
fn offer_is_wanted() -> bool {
    baz_update::installs_itself() && Route::detect().can_install() && baz_update::wanted()
}

/// **What is staged, if it is newer than us and still intact.**
///
/// Returns the version and the installer's path.
///
/// Each `None` below is a different way of having nothing to offer and they
/// are all silent: nothing staged, something staged that this baz already
/// *is* (the ordinary state on the first launch after an update landed), and
/// something staged that no longer matches the digest it was staged with.
/// The last is the interesting one — a truncated download from a session that
/// was killed mid-write — and it is thrown away rather than offered.
fn waiting() -> Option<(String, PathBuf)> {
    let pending = stage::pending()?;
    if !is_newer(&pending.version, env!("CARGO_PKG_VERSION")) {
        // Already installed, or staged by a newer baz that has since been
        // replaced by an older one. Either way it is finished with.
        stage::discard();
        return None;
    }
    let Some(path) = stage::ready(&pending) else {
        stage::discard();
        return None;
    };
    Some((pending.version, path))
}

/// **Ask, in the operating system's own voice.**
///
/// Only [`rfd::MessageDialogResult::Yes`] is consent. A closed window, a
/// missing `zenity`, a desktop that answered something this does not
/// recognise — every one of them means *start my music player*, because that
/// is what the person double-clicking the icon asked for, and an update is
/// the thing they did not ask for.
///
/// **"Not now" keeps the file.** It is offered again at the next launch and
/// nothing is downloaded twice; a listener who never wants to be asked has a
/// box to untick, and re-asking is what *not now* means in every other
/// application that says it.
fn accepted(version: &str) -> bool {
    let running = env!("CARGO_PKG_VERSION");
    let version = version.strip_prefix('v').unwrap_or(version);
    rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Info)
        .set_title("baz")
        .set_description(format!(
            "baz {version} is ready to install.\n\n\
             It has already been downloaded and checked against its published \
             checksum. You are running {running}.\n\n\
             Install it now? baz will start once the installer has finished."
        ))
        .set_buttons(rfd::MessageButtons::YesNo)
        .show()
        == rfd::MessageDialogResult::Yes
}

/// **A sibling of this executable**, which is where every one of the three
/// packagings puts baz: `Program Files\baz\`, `Contents/MacOS/`, and whatever
/// directory an unpacked archive landed in.
fn beside_me(name: &str) -> Option<PathBuf> {
    Some(std::env::current_exe().ok()?.parent()?.join(name))
}

/// **Become baz**, carrying every argument through.
///
/// Every argument, because a launcher that ate them would silently break
/// `baz /path/to/album` and every desktop file that passes one.
fn command_for(player: &Path) -> Command {
    let mut command = Command::new(player);
    command.args(std::env::args_os().skip(1));
    command
}

/// On Unix the launcher *replaces* itself with baz.
///
/// No parent left sitting in the process table, no second entry in a task
/// manager, and no extra layer between a desktop's launch and the window it
/// is waiting for. `exec` only ever returns a failure — on success this
/// process has already ceased to exist.
#[cfg(unix)]
fn launch(player: &Path) -> ExitCode {
    use std::os::unix::process::CommandExt as _;
    let error = command_for(player).exec();
    complain(&format!("baz could not be started.\n\n{error}"))
}

/// Windows has no `exec`, so there the launcher spawns baz and exits, which
/// is the same thing a moment later.
#[cfg(not(unix))]
fn launch(player: &Path) -> ExitCode {
    match command_for(player).spawn() {
        Ok(_) => ExitCode::SUCCESS,
        Err(error) => complain(&format!("baz could not be started.\n\n{error}")),
    }
}

/// **Say something went wrong, to somebody who has no terminal.**
///
/// A launcher failing silently is a double-click that does nothing, which is
/// the least diagnosable failure a desktop application has. It goes to
/// `stderr` as well, because whoever is debugging this does have a terminal.
fn complain(what: &str) -> ExitCode {
    eprintln!("baz-boot: {what}");
    rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Error)
        .set_title("baz")
        .set_description(what)
        .set_buttons(rfd::MessageButtons::Ok)
        .show();
    ExitCode::FAILURE
}
