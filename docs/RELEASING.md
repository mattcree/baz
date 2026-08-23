# Releasing baz

> This is the machinery and the checklist. The first public beta, `v0.1.0`,
> shipped through it on 2026-08-14 after explicit owner authority. Future tags
> remain external release boundaries and require the same explicit authority.

`docs/ENGINEERING.md`: *"Releases are built, signed, and
reproducible-where-possible from CI only — no artifacts from developer
machines."* Two of those three are implemented; the third is stated honestly
below rather than implied.

## The shape of it

`.github/workflows/release.yml` runs on a `v*` tag, and on `workflow_dispatch`
for a dry run that builds and checksums everything and publishes nothing.

```
version ──┬──> build (linux-x86_64, windows-x86_64 + .msi, macos-universal + .dmg) ──┐
gate ─────┴──> flatpak (single-file .flatpak bundle) ────────────────────────────────┴──> publish
                                                          (SHA256SUMS + draft GitHub Release)
```

- **`version`** reads `[workspace.package] version` from `Cargo.toml` and, on a
  tag, refuses to continue unless the tag matches it. A tag that disagrees with
  the workspace costs seconds, not a full matrix build.
- **`gate`** is `uses: ./.github/workflows/ci.yml` — the *whole* PR suite
  (rustfmt, clippy `--all-features` with warnings denied, tests on all three
  operating systems, rustdoc, `cargo-deny`, MSRV, coverage floor, packaging
  metadata), run again against the tagged tree. `build` has `needs: [version,
  gate]`, so there is no path from a tag to an artifact that skips it. It is
  the same workflow file PRs are held to, called rather than copied, so the two
  definitions of "green" cannot drift apart.
- **`build`** produces one Baz archive per platform, and on two of them an
  installer beside it: `cargo-wix` builds the `.msi`, `packaging/macos/dmg.sh`
  the `.dmg`. Both are the *same* binary the archive holds, packaged again —
  not a second build.
- **`flatpak`** builds the Linux bundle. It has the same `needs: [version,
  gate]` as `build` and no `if:`, so a dry run covers it. What it does *not*
  do is publish the OSTree repository it builds through, which is why a
  `.flatpak` bundle cannot update itself — see ADR-0045.
- **`publish`** computes every SHA-256 in one place, verifies the sums file
  against the files it describes, and creates the release **as a draft** —
  ENGINEERING's "a human owns the trunk" applies at least as much to what
  strangers download. The maintainer looks at the artifacts and presses
  publish.

### Platforms and features

| Artifact | Runner | Target(s) |
|---|---|---|
| `linux-x86_64` | `ubuntu-latest` | `x86_64-unknown-linux-gnu` |
| `windows-x86_64` | `windows-latest` | `x86_64-pc-windows-msvc` |
| `macos-universal` | `macos-latest` (arm64) | `aarch64-apple-darwin` + `x86_64-apple-darwin`, combined with `lipo` |
| `flatpak` | `ubuntu-latest` | `x86_64`, built in the Freedesktop SDK |

Each of the first three also carries `install.sh` (Linux), and Windows and
macOS additionally emit an installer: `baz-X.Y.Z-windows-x86_64.msi` and
`baz-X.Y.Z-macos-universal.dmg`. The installers place `baz-boot` beside `baz`
and point the Start-menu entry and app icon at it; ADR-0043 says why.

macOS ships as one universal binary rather than two downloads because an
ordinary person should not have to know which Mac they own; the arm64 runner
cross-builds the Intel slice with the same Xcode toolchain and no extra linker.

Device output is part of every GUI build. Building `cpal` needs platform audio
headers, which the primary development host lacks outside its toolbox. The
Linux runner installs `libasound2-dev`, the same package and the same reason as
CI's test job.

Vibe is explicitly selected in the Flatpak and release build commands alongside
device output. The dependency-minimal `--no-default-features` build is only a
CI/development boundary check and is never emitted by a release workflow.
`ort` is pinned to 2.0.0-rc.10 because its ONNX Runtime 1.22 distribution is
the last verified release that supplies both Intel and Apple Silicon macOS
binaries. Its native-TLS dependency is confined to the build-time downloader;
the Baz binary does not link OpenSSL. Revisit that pin when a rustls-backed
release again covers both slices of the universal archive.

**A consequence worth stating before you try it**: the release build command
does not run on the maintainer's own machine. Fedora Silverblue has no
`alsa-lib-devel`, so `cargo build -p baz` fails on the host
and every local rehearsal below runs inside the `baz-dev` toolbox
(`scripts/toolbox-setup.sh`, `docs/DEVELOPMENT.md`). The Linux release artifact
has therefore never been produced outside a container, which is fine — CI's
runner is one too.

## What is pinned, and what "reproducible" honestly means

Pinned, so two builds of the same tag see the same inputs:

- **Compiler**: `rust-toolchain.toml` (1.92.0), which `rustup` honours on the
  runners, so `rustup target add` extends *that* toolchain and not a floating
  stable.
- **Dependencies**: `--locked`, against the committed `Cargo.lock` — the same
  lockfile the CI gate proved and the same one the Flatpak vendors.
- **Features**: stated above, not inherited from a default.
- **Build path**: `--remap-path-prefix` keeps the runner's absolute checkout
  path out of the binary.
- **Incremental compilation**: off, and no build cache — unlike CI, the
  release job compiles from cold, so a cached artifact from an earlier run is
  never an unexamined input.

Not reproducible, and not claimed to be:

- **Bit-identical rebuilds are unverified.** Nobody has rebuilt a tag and
  compared hashes, and the release job does not attempt to. GitHub's runner
  images move underneath us (linker, system libraries, Xcode and MSVC
  versions), the archives carry the mtimes of the moment they were created, and
  thin LTO's scheduling is not something this project has proven deterministic.
  Treat `SHA256SUMS` as "this is the file that CI produced", published beside
  the public log of the run that produced it — not as "you can rebuild this
  byte for byte".
- **Nothing published here is signed.** The archives, the `.msi` and the
  `.dmg` all ship unsigned, with published SHA-256 checksums beside them.
  Windows SmartScreen and macOS Gatekeeper will both say so, in those words,
  to every listener who runs an installer.

  **Baz has updated itself since v0.5.0**, so the sentence that used to sit
  here — "listeners manually verify, replace and relaunch" — stopped being
  true and is corrected rather than quietly deleted. Settings → Updates
  downloads the next release, checks it against the `SHA256SUMS` published
  beside it, and installs it: in place on Linux, via `msiexec` on Windows.
  ADR-0043 is the design.

  That check is done over the network at download time, against a file served
  by GitHub over TLS, and it is worth exactly what that is worth. It is *not*
  a signature. Two consequences follow, and neither is fixed:

  1. Nothing proves the release came from this project rather than from
     whoever could serve those bytes. A signing key is the only thing that
     would, and ADR-0043 §4 defers acquiring one on cost.
  2. The staged installer is re-checked before it is offered, but against a
     digest stored beside it in the same user-writable directory, so that
     second check proves the download is intact and not that it is ours
     (`crates/baz-update/src/stage.rs`, and the backlog item it points at).

  Treat `SHA256SUMS` as "this is the file CI produced", published next to the
  public log of the run that produced it. Presenting stronger provenance
  needs the signing and distribution design ADR-0043 §4 still defers.

## Rehearsing it locally, before any of the above

Every command in this section has been run on the maintainer's machine and does
what it says. None of them writes anything outside the working tree, and none
of them can publish. Do this first; it is twenty minutes and it is where the
surprises are.

```sh
# 1. The gate, as the PR suite runs it. In the toolbox, because
#    --all-features reaches device-output and so needs the ALSA headers.
#    CI sets RUSTFLAGS=-D warnings for the whole workflow, so do the same
#    here or clippy is a different check locally than it is on the runner.
export RUSTFLAGS="-D warnings"
toolbox run -c baz-dev env RUSTFLAGS="$RUSTFLAGS" cargo fmt --all --check
toolbox run -c baz-dev env RUSTFLAGS="$RUSTFLAGS" cargo clippy --workspace --all-targets --all-features
toolbox run -c baz-dev env RUSTFLAGS="$RUSTFLAGS" cargo test --workspace --all-features --no-fail-fast
toolbox run -c baz-dev env RUSTFLAGS="$RUSTFLAGS" RUSTDOCFLAGS="-D warnings" \
  cargo doc --workspace --no-deps --all-features
cargo deny check

# 2. The packaging metadata, exactly as CI's `packaging` job checks it.
desktop-file-validate packaging/io.github.mattcree.baz.desktop
appstreamcli validate --no-net packaging/flatpak/io.github.mattcree.baz.metainfo.xml
python3 -c 'import yaml,sys; yaml.safe_load(open(sys.argv[1]))' \
  packaging/flatpak/io.github.mattcree.baz.yml
python3 packaging/flatpak/check-cargo-sources.py
python3 packaging/flatpak/check-manifest-pin.py

# 3. The release build, exactly as .github/workflows/release.yml runs it.
toolbox run -c baz-dev env CARGO_INCREMENTAL=0 \
  RUSTFLAGS="--remap-path-prefix=$PWD=/build/baz" \
  cargo build --release --locked --target x86_64-unknown-linux-gnu \
              -p baz --features device-output,vibe-analysis
```

That last one takes about eight minutes from cold and currently produces a
76 MB binary at `target/x86_64-unknown-linux-gnu/release/baz`. It links
`libstdc++`, `libasound`, `libc`, `libm` and `libgcc_s`; `ldd` it if you want
the receipt, and `docs/INSTALL.md` states what it additionally `dlopen`s.

Then the staging and checksum steps, which are the ones nobody had ever run:

```sh
version="$(grep -m1 '^version = ' Cargo.toml | sed 's/.*"\(.*\)".*/\1/')"
stage="staging/baz-${version}-linux-x86_64"
mkdir -p "$stage" dist
cp target/x86_64-unknown-linux-gnu/release/baz "$stage/"
cp LICENSE README.md CHANGELOG.md "$stage/"
cp packaging/io.github.mattcree.baz.desktop "$stage/"
cp packaging/flatpak/io.github.mattcree.baz.metainfo.xml "$stage/"
cp -r packaging/icons/hicolor "$stage/icons"
tar -czf "dist/baz-${version}-linux-x86_64.tar.gz" -C staging "baz-${version}-linux-x86_64"

cd dist && sha256sum -- * | tee SHA256SUMS
grep -v ' SHA256SUMS$' SHA256SUMS | sha256sum --check --strict
```

With the pinned 156 MB Vibe model set included, the Linux archive currently
comes out at about 154 MB. The last line is the workflow's own self-check — it
proves the sums file describes the files beside it — and it passes.

**The dry run covers the Flatpak** — the `flatpak` job has no `if:` — so you
no longer have to build it locally to know it builds. Do it anyway when you
have touched the manifest, the metainfo or `cargo-sources.json`, because the
job proves the bundle *builds* and not that it *runs*:
`packaging/flatpak/README.md` §"Building it". Budget fifteen minutes and 10 GB
of scratch space, and keep it off a tmpfs.

## Cutting a release

1. `main` is green, and the tree is the tree you mean to ship.
2. Move `CHANGELOG.md`'s `[Unreleased]` content into a `## [X.Y.Z] - YYYY-MM-DD`
   section; leave `[Unreleased]` empty above it, and add the two link
   references at the foot of the file (`[Unreleased]` becomes a `compare/`
   link against the new tag).
3. Set `version` in `[workspace.package]` in `Cargo.toml`, and the
   `baz-core` entry in `[workspace.dependencies]` to match. Run `cargo check`
   so `Cargo.lock` picks it up. **This does not touch
   `packaging/flatpak/cargo-sources.json`**: that file lists only crates with a
   registry `source`, and the two workspace members have none. Re-run
   `python3 packaging/flatpak/check-cargo-sources.py` anyway — it costs a
   second and it is the only thing standing between a lockfile change and a
   broken Flathub build.
4. Add the release to `packaging/flatpak/io.github.mattcree.baz.metainfo.xml`
   with its real date. `type` there is the release *channel* and not a maturity
   claim — see the comment above the `<releases>` block.
5. Re-run `docs/screenshots/capture.sh` if the interface has moved since the
   last release, and check the frames. Flathub's page is whatever is committed
   in `docs/screenshots/`, served from `main`; a store listing showing a baz
   nobody can install any more is worse than an old version number.
6. Commit them together — `Cargo.toml`, `Cargo.lock`, `CHANGELOG.md`, the
   metainfo, and any re-shot screenshots. That commit is the release commit.
7. Run the release workflow by hand (`workflow_dispatch`) first. It builds
   every platform and produces the checksums without publishing anything — the
   cheapest way to find out that a runner image has changed. **`workflow_dispatch`
   runs against a branch, not an arbitrary SHA**, so the release commit has to
   be the tip of one; in practice that means push it to `main` and dispatch
   from `main`.
8. Tag it: `git tag -a vX.Y.Z -m "baz X.Y.Z"` and push the tag. The version job
   will reject it if step 3 was missed.
9. Review the draft release, then publish it.
10. Update the Flathub manifest's `tag` and `commit` — `commit` must be the full
    SHA of the **commit**, which is `git rev-parse vX.Y.Z^{commit}` and not
    `git rev-parse vX.Y.Z`. Every tag here is annotated, so the second form
    prints the hash of the *tag object* — a different, equally valid-looking
    40-character SHA that Flathub will fail to check out. `^{commit}`
    dereferences it. Flathub wants both fields so a moved tag cannot change
    what is built, which is exactly why putting the wrong SHA in the second
    one defeats the point of having it.

    You do not have to get this right from memory:
    `python3 packaging/flatpak/check-manifest-pin.py` compares the manifest's
    `commit` against the commit its own `tag` resolves to, and if you have
    pasted the tag object's hash it says so and prints the command that gives
    the right one. CI's packaging job runs it on every push, with tags
    fetched so it cannot skip. Regenerate
    `cargo-sources.json` if `Cargo.lock` gained or dropped a dependency;
    `packaging/flatpak/README.md` has the commands.

## v0.5.0 release record

Cut on 2026-08-23 from annotated tag `v0.5.0`, resolving to commit `0908e1b`.
The dry run was
[32642628559](https://github.com/mattcree/baz/actions/runs/32642628559) and the
tagged publish run
[32643790251](https://github.com/mattcree/baz/actions/runs/32643790251). Seven
assets: three archives, `.msi`, `.dmg`, `.flatpak`, and `SHA256SUMS`.

**This is the release whose updater was proved on real hardware, which no
earlier one was.** v0.4.1 was installed on the maintainer's machine from its
own `.tar.gz` via `install.sh`; baz was then driven through Settings → Updates
by hand, and it downloaded v0.5.0, checked it, and installed over itself. The
receipt is the binary: `/usr/local/bin/baz` went from SHA-256 `fb6e6c88…` to
`ea8a0b65…`, and `ea8a0b65…` is byte-identical to the binary inside the
`v0.5.0` archive downloaded separately from the release page. The upgrade path
that every previous release record described only as machinery has now carried
a real installation forward once.

Two things had to be fixed to get there, both found by doing it rather than by
reading it:

- `crates/baz-update`'s endpoint was `/releases/latest`, which **excludes
  prereleases**. Every baz release so far is one, so the updater had been
  asking a URL that returns 404 for this project since the day it shipped. It
  now reads `/releases?per_page=20` and picks the newest published non-draft
  itself.
- `install.sh` copied over the running executable with `cp`, which truncates
  in place; the kernel refuses that for a running program (`ETXTBSY`). It now
  writes beside the target and `mv -f`s over it.

Both are in v0.5.0 — meaning **v0.4.1 and earlier cannot update themselves
in-place on Linux and never could**, and a listener on one of those has to
install v0.5.0 by hand once before the updater starts working for them. That
is a real one-time cost of shipping an updater nobody had run.

## v0.4.1 release record

Cut on 2026-08-21 from annotated tag `v0.4.1`, resolving to commit `02ef1b1`.
The dry run was
[32421656692](https://github.com/mattcree/baz/actions/runs/32421656692) and the
tagged publish run
[32429339362](https://github.com/mattcree/baz/actions/runs/32429339362).

A two-item patch on the day after v0.4.0: Updates moved into its own Settings
section, and Flatpak playback was fixed — the sandbox had no route to the
desktop sound server, so the one artifact the release notes recommended to
Linux listeners was the one that could not make a sound.

## v0.4.0 release record

Cut on 2026-08-20 from annotated tag `v0.4.0`, resolving to commit `45d5bc5`.
The tagged publish run was
[32413661268](https://github.com/mattcree/baz/actions/runs/32413661268), and
the dry run that finally went green
[32412003020](https://github.com/mattcree/baz/actions/runs/32412003020).

**It took six dispatches to get there, and the record is the point of keeping
this section.** Runs
[32406323401](https://github.com/mattcree/baz/actions/runs/32406323401),
[32407018059](https://github.com/mattcree/baz/actions/runs/32407018059),
[32408728771](https://github.com/mattcree/baz/actions/runs/32408728771) and
[32410313034](https://github.com/mattcree/baz/actions/runs/32410313034) all
died in the Windows job's new `Installer (msi)` step, on two distinct causes:

- `Error[2] (Generic): The app's version 0.4.0+dry.fc1c6b4 is a prerelease,
  but we couldn't convert the prerelease components to an integer.` A dispatch
  appends SemVer build metadata, and MSI's `ProductVersion` is four numbers.
  The step now passes `${VERSION%%+*}` as `--install-version` while the
  filename keeps the full string. **Only a dry run can hit this**, because
  only a dry run has a `+dry.SHA` version — the failure lives exactly in the
  rehearsal path, and a project that only ever tagged would have met it never.
- `main.wxs(63) : error LGHT0103 : The system cannot find the file
  'target\release\baz.exe'.` twice. `cargo wix --no-build` looks in the
  native `target/release`, and the matrix cross-builds into
  `target/x86_64-pc-windows-msvc/release`. Fixed with `--target-bin-dir`.

Both are failures of the packaging step and not of the program, which is the
kind of thing that reaches listeners as "the installer is broken" and never
appears in a test suite. The dry run has now earned its place at four
consecutive releases.

## v0.3.0 release record

Cut on 2026-08-17 from annotated tag `v0.3.0`, resolving to commit
`fc2b083`. The first dry run was
[31985077983](https://github.com/mattcree/baz/actions/runs/31985077983), the
second [31986292130](https://github.com/mattcree/baz/actions/runs/31986292130),
and the tagged publish run was
[31987176065](https://github.com/mattcree/baz/actions/runs/31987176065).

The three archives were downloaded again and **verified against the published
`SHA256SUMS`** (all three `OK`). The Linux binary reports
`baz 0.3.0 — a music player for people who own their music`; the macOS
executable is a two-slice `x86_64`/`arm64` Mach-O inside a `baz.app` whose
`Info.plist` carries `0.3.0`, and the metainfo in the Linux archive carries the
`0.3.0` release element.

**The dry run earned its place for the second release running**, and this time
on a check added since the last one. The macOS bundle step grepped the built
`.icns` for a leading `TOC ` chunk, on the reasoning that `iconutil` always
writes one and the committed fallback writer does not. The first release to
actually run that check failed it — on a 406 KB icon `iconutil` had just built
and logged building. Reading the shipped file settles it: Apple's tool wrote no
`TOC ` at all.

```text
ic12 ic07 ic13 ic08 ic04 ic14 ic09 ic05 ic10 ic11 info
```

Eleven chunks, the full modern set, and a header whose declared length matches
the file. The check now reads the line `bundle.sh` prints about which branch it
took, which is the statement itself rather than a guess about its output.

Publishing the draft is the owner's own step, as it was for `v0.1.0` and
`v0.2.0`.

**And for `v0.2.0` and `v0.3.0` it was never taken.** Both are still drafts on
the releases page today, visible only to people with push access. `v0.1.0`,
`v0.4.0`, `v0.4.1` and `v0.5.0` are published prereleases. This is not a
machinery failure — the drafts are complete and their artifacts verified — but
anything reasoning about "the last release" should mean the last *published*
one, which is what the updater's `/releases` walk now also does.

## v0.2.0 release record

Cut on 2026-08-15 from annotated tag `v0.2.0`, resolving to commit
`c96e05c`. The dry run was
[31858378333](https://github.com/mattcree/baz/actions/runs/31858378333) and the
tagged publish run was
[31860986161](https://github.com/mattcree/baz/actions/runs/31860986161); the
same tree had already passed the ordinary CI gate as
[31860783174](https://github.com/mattcree/baz/actions/runs/31860783174), and the
whole local rehearsal in this document was run before the tag.

The draft's three archives were downloaded again and **verified against the
published `SHA256SUMS`** (all three `OK`). The Linux archive holds one correctly
named root with the binary, `README.md`, `CHANGELOG.md`, `LICENSE`, the desktop
entry, the metainfo (carrying the `0.2.0` release element) and the complete Vibe
model set; the Windows zip holds the same payload around `baz.exe`. The Linux
binary reports `baz 0.2.0` when run, and the macOS executable was identified as
a two-slice `x86_64`/`arm64` Mach-O before the draft was reviewed.

**The dry run earned its place this time.** It failed first on rustdoc under
`-D warnings` — four unresolved intra-doc links introduced with the release's
own work, plus one that predated it — none of which the ordinary gate catches,
because `cargo doc` runs in the release workflow and not in the PR suite.

Publishing the draft is the owner's own step; it is a prerelease until he says
otherwise, exactly as `v0.1.0` was.

## v0.1.0 release record

The public beta shipped on 2026-08-14 from annotated tag `v0.1.0`, resolving
to commit `5f12daa4c5e26d7abcd034762916138ce38f0f40`. The final dry run was
[31790687891](https://github.com/mattcree/baz/actions/runs/31790687891); the
tagged publish run was
[31791950764](https://github.com/mattcree/baz/actions/runs/31791950764). Both
passed the complete CI gate and all three archive builds.

The draft's Linux x86-64, Windows x86-64 and universal macOS archives were
downloaded again, verified against their published `SHA256SUMS`, and inspected
for one correctly named root, expected documentation and the complete Vibe
model set. The macOS executable was independently identified as a two-slice
`x86_64`/`arm64` Mach-O before the draft was published. The public prerelease is
<https://github.com/mattcree/baz/releases/tag/v0.1.0>.

The first archive rehearsal exposed a Windows history-test ordering assumption;
the second exposed that `ort` rc.13 no longer distributed an Intel macOS
runtime. The former now waits only on the event whose contract it proves. The
latter is why the release pins rc.10, whose real bundled-model inference was
verified locally before the final matrix. Flathub remains unsubmitted; the
repository manifest now pins the released tag and commit so a later submission
cannot move underneath review.

**Why the old dry run stopped before it built anything.**

The first run —
[run 31399606796](https://github.com/mattcree/baz/actions/runs/31399606796),
against `main` at `e8dd2a2` — proved the version job and every ordinary CI job,
but the reusable CI workflow inherited the caller's `workflow_dispatch` event.
That accidentally included the scheduled/manual discovery-fuzz job in a
release rehearsal. A known Symphonia panic keeps `playback_decode` red under
libFuzzer even though baz contains it in normal builds (ADR-0040), so `gate`
failed and the artifact matrix never started.

The trigger policy is now explicit. Weekly CI and a direct manual dispatch of
the **CI** workflow run all six discovery fuzz targets. A PR, push, release dry
run, and tag use the ordinary gate; every hostile input fuzzing has already
found is a permanent test in `crates/baz-core/tests/hostile_media.rs`. Thus a
dry run and a tag have the same pre-build gate, while fuzzing continues to look
for the next input independently.

The corrected dry run and the first tag have now exercised the build matrix,
checksum self-check, tag-versus-manifest success path, verified-tag release
creation, generated notes and asset upload. Future rehearsals retain their
purpose: runner images and upstream platform distributions can still change.

The local rehearsal above has now been done once, end to end, and found two
things — both fixed in the change that added this section. The first was
cosmetic-looking and was not: `packaging/flatpak/cargo-sources.json` had seven
crates unpacked with no `.cargo-checksum.json` beside them, which cargo rejects
before it resolves anything, so the very first Flatpak build of baz died on a
crate unrelated to what it was compiling. CI's `check-cargo-sources.py` passed
throughout, because it compared only the archive list against `Cargo.lock` and
skipped the inline halves. It no longer does. The second was that the Flatpak
manifest carried the desktop entry as a `type: file` source at `../`, a path
that resolves in this repository and not in the `flathub/` one the manifest is
copied into; both it and the metainfo are now installed from the git source's
own `packaging/` tree.
