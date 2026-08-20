# ADR-0043: Installing and updating — the platform's package manager owns the update, and baz tells the people it cannot reach

**Status**: proposed (2026-08-20) · answers the owner's *"we need to get on our backlog INSTALLERS for all platforms… and we need to solve updating"* · extends [ADR-0025](0025-picking-a-folder.md)'s desktop integration and the release workflow the archives already come from

## Context

What a release produces today, and what each one leaves a listener holding:

| Platform | Ships | What it does not do |
|---|---|---|
| Linux | `tar.gz` with `install.sh`/`uninstall.sh`, icons, desktop entry, AppStream | never updates; the Flatpak manifest is **validated** by CI and **never built** |
| Windows | `zip` with a bare `baz.exe` | no installer, no Start-menu entry, no uninstall, no updates |
| macOS | `zip` with a universal `baz.app` | no `.dmg`, unsigned, no updates |

So there are three gaps and they are not the same shape. Windows has no
installer at all. Linux has the *machinery* for the good answer and does not
run it. macOS has a bundle and no way to hand it over.

**And nothing anywhere updates.** baz has never made a network request in its
life — that is a property, not an oversight — so "solve updating" is not one
decision but two: *how does a new version reach a machine*, and *what is baz
allowed to do about it*.

## Decision

### 1. The package manager owns the update. Everywhere it exists.

| Platform | Route | Updates by |
|---|---|---|
| Linux | **Flathub** | `flatpak update`, GNOME Software, KDE Discover — automatically |
| Windows | **winget** (from the MSI) | `winget upgrade` |
| macOS | **Homebrew cask** (from the DMG) | `brew upgrade --cask` |
| Any | tarball / zip / drag-installed `.app` | nothing — see §3 |

This is the whole of the update story for anyone who installs the ordinary
way, and it is **no code in baz at all**. Three reasons it is not a close call:

- **A self-updater cannot work in a Flatpak.** `/app` is read only, by design.
  The one Linux route we most want people to take is the one an in-app
  updater is structurally unable to serve.
- **A self-updater on Windows fights the file lock.** Replacing a running
  `.exe` means a helper process, a scheduled swap, and a class of failure that
  leaves a listener with no player.
- **A self-updater is a permanent remote-code-execution path** that we would
  own, sign for, and be responsible for the day it is compromised. baz plays
  local files. The cost is wrong for what it buys.

### 2. Build the installers in CI, three of them

- **Flatpak** — `flatpak-builder` in the release job over the manifest that
  already exists, producing a single-file `.flatpak` bundle attached to the
  release. That gives a working install *today*, on any distribution, without
  waiting on Flathub review; the Flathub submission is then a human step whose
  artefact CI already proves builds.
- **Windows MSI** — `cargo-wix` (WiX Toolset), which is the Windows-native
  path and the format `winget` prefers. It brings the Start-menu entry, the
  Add/Remove Programs entry, and the uninstall that a zip cannot.
- **macOS DMG** — `hdiutil` over the bundle that is already built, with the
  drag-to-Applications layout, because a `.zip` of a `.app` is a download an
  ordinary person is expected to know what to do with and does not.

The tarball and the zip **stay**. They are what a reviewer, a packager and a
person on a distribution we do not target actually want, and they cost one
line each now that the staging exists.

### 3. baz tells the people no package manager can reach — decided, and **not built yet**, because it costs a dependency

*The design below is settled. What is not settled is whether it is worth what
it costs, and that is the owner's call rather than one to make quietly.*

**The cost.** baz has no HTTP client. `ureq` is in `Cargo.lock` today only as a
**build**-dependency of `ort-sys`, so it is compiled and never linked; making
it a runtime dependency puts it and a TLS stack into every shipped binary, on
every platform, and through `deny.toml`'s licence review. That is a
meaningful addition to a local music player in exchange for one request a day.

**What it buys.** Only the tarball, zip and dragged-bundle listeners — who are
also the most technically capable group and the ones most able to watch a
repository. Everybody who installs the ordinary way is already served by §1
with no code at all.

**The one option that costs nothing** is worth recording because it is not
obviously silly: the check is *only ever wanted where a package manager is
absent*, and that is exactly where `curl` is present — macOS, Windows 10 and
later, and every Linux that is not inside a sandbox. Inside a Flatpak baz
would not check at all. So a shell-out has no coverage gap; what it has is the
ugliness of a GUI process spawning a subprocess for network I/O, which is a
matter of taste rather than of correctness.

The implementation, written and unit-tested against the rules below, is kept
at `docs/design/impl/release-check.rs.txt` rather than in the tree, because
code nothing calls is worse than code that does not exist.

#### The rules it is built to, whichever way it is wired

- **A version check, off by default, stated in Settings.** baz makes no
  network request today and it will not start making one because a developer
  thought it would be handy. Opt-in, in words, once.
- **At most once a day**, compared against `CARGO_PKG_VERSION`.
- **The whole effect is one line in the health log** behind the bell — the
  surface that already exists for *something you should know*. No modal, no
  badge, no download.
- **The compare is numeric, not lexical.** Every hand-rolled version check
  ships with `"0.10.0" < "0.9.0"` as strings, and the failure is silent
  forever after the tenth minor release.
- **Every doubt resolves to silence.** A pre-release suffix, a fourth
  component, a tag typed by hand: not newer. Missing a release costs a
  listener one version's delay; a false positive tells somebody their
  up-to-date player is out of date.
- **It knows how it was installed and says the right thing.**
  `/.flatpak-info` exists → *your software centre will offer it*, with no
  mention of a download. Telling a Flatpak listener to go and download
  something is telling them to break their own installation, and it is the one
  thing here that would be actively harmful to get wrong.

### 3b. What is built instead, today

The release notes now carry a table saying which file to take **and what
updates it** — including the honest row for the archives, whose answer is
*nothing; you come back here*. That is not a substitute for §3 and it is not
pretending to be; it is the part that costs nothing and removes the commonest
version of the problem, which is a listener taking the tarball without knowing
the Flatpak existed.

### 4. Signing is a decision for the owner, and the plan works without it

Neither Windows SmartScreen nor macOS Gatekeeper will be quiet about an
unsigned download. That is bought, not built:

| | Cost | What it removes |
|---|---|---|
| Apple Developer ID + notarisation | 99 USD/yr | *"baz cannot be opened because Apple cannot check it"* |
| Windows code-signing certificate | ~200–400 USD/yr (OV), more for EV | SmartScreen's *"unrecognised app"* |

**Flathub needs neither** — it signs and distributes for us, which is a second
reason it is the recommended Linux route rather than merely a convenient one.
Everything below is built to be signable later: the MSI and the DMG take a
signing step that is currently a no-op, so turning it on is a secret and a
flag rather than a rebuild of the pipeline.

## What this exposes, and it is worth knowing before Flathub

The manifest grants `--filesystem=xdg-music:ro` and nothing else. **The
owner's own library is on an SMB share reached through gvfs**, at
`/run/user/1000/gvfs/…`, which that grant does not cover — so a Flathub baz
would not, today, see the library it was developed against. The manifest says
as much and names the fix: a portal-based folder chooser, so a listener grants
the folder rather than the packager guessing it. That is now a blocker on the
Flathub submission rather than a note in a file, and it is tracked as such.

## Alternatives rejected

- **`cargo-dist`.** Actively maintained (v0.32.0, May 2026; commits this
  week) and it would generate the installers *and* a self-updater from one
  config. Rejected for the shape of the fit rather than the quality: it wants
  to own the release workflow, and ours is 398 lines of stated decisions —
  version cross-checks, a CI gate, `lipo`, a bundle script that refuses to
  ship a fallback icon. Adopting it means replacing all of that with generated
  YAML, and its updater is the thing §1 argues nobody should ship. Worth
  revisiting if the release workflow ever becomes a burden rather than an
  asset.
- **An in-app self-updater** (`self_update`, `axoupdater`). §1.
- **Snap, AppImage, `.deb`/`.rpm`.** Flatpak is the owner's stated preference
  and one Linux format done properly beats four done partly. A `.deb` is the
  plausible fifth if anyone asks.
- **Microsoft Store / Mac App Store.** Both want signing, review, and a
  sandbox story; both are a later chapter and neither is on the path to the
  other three.

## Consequences

- Three new release artefacts, and one of them (`.flatpak`) is a working
  install on every Linux distribution the week it lands.
- One new opt-in network capability, guarded by a setting, whose entire
  effect is a line of text.
- A blocker on Flathub — the portal folder chooser — promoted from a comment
  in a manifest to a tracked piece of work.
- Signing remains bought rather than built, and the pipeline is shaped so
  that buying it is a flag.
