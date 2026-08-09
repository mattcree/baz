# Releasing baz

> Nothing has been released. This is the machinery and the checklist; the
> maintainer decides when the first tag happens.

`docs/ENGINEERING.md`: *"Releases are built, signed, and
reproducible-where-possible from CI only — no artifacts from developer
machines."* Two of those three are implemented; the third is stated honestly
below rather than implied.

## The shape of it

`.github/workflows/release.yml` runs on a `v*` tag, and on `workflow_dispatch`
for a dry run that builds and checksums everything and publishes nothing.

```
version ──┐
          ├──> build (linux-x86_64, windows-x86_64, macos-universal)
gate ─────┘         └──> publish (SHA256SUMS + draft GitHub Release)
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
- **`build`** produces one archive per platform.
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

macOS ships as one universal binary rather than two downloads because an
ordinary person should not have to know which Mac they own; the arm64 runner
cross-builds the Intel slice with the same Xcode toolchain and no extra linker.

**The shipped feature set is `--features device-output`, and that is the whole
of it** — `baz` has exactly one feature. It is non-default only because
building `cpal` needs platform audio headers, which the primary development
host lacks; a released music player obviously needs audio output. The Linux
runner installs `libasound2-dev`, the same package and the same reason as CI's
test job.

**A consequence worth stating before you try it**: the release build command
does not run on the maintainer's own machine. Fedora Silverblue has no
`alsa-lib-devel`, so `cargo build … --features device-output` fails on the host
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
- **The artifacts are not signed.** No GPG signature, no macOS notarization, no
  Windows Authenticode. Certificates and notarization are a cost and an
  identity decision the project has not made, and pretending otherwise would be
  worse than saying so. `docs/INSTALL.md` tells users exactly this.

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

# 3. The release build, exactly as .github/workflows/release.yml runs it.
toolbox run -c baz-dev env CARGO_INCREMENTAL=0 \
  RUSTFLAGS="--remap-path-prefix=$PWD=/build/baz" \
  cargo build --release --locked --target x86_64-unknown-linux-gnu \
              -p baz --features device-output
```

That last one takes about eight minutes from cold and produces a 33 MB binary
at `target/x86_64-unknown-linux-gnu/release/baz`. It links `libasound.so.2`,
`libc`, `libm` and `libgcc_s` and nothing else — `ldd` it if you want the
receipt; `docs/INSTALL.md` states what it additionally `dlopen`s.

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

The archive comes out at about 12 MB. The last line is the workflow's own
self-check — it proves the sums file describes the files beside it — and it
passes.

**Build the Flatpak too**, which the dry run does not cover at all:
`packaging/flatpak/README.md` §"Building it". Budget fifteen minutes and 10 GB
of scratch space, and keep it off a tmpfs.

## Cutting a release

1. `main` is green, and the tree is the tree you mean to ship. The workspace
   version is `0.0.0` today, so the first release is a real edit, not a bump.
2. Move `CHANGELOG.md`'s `[Unreleased]` content into a `## [X.Y.Z] - YYYY-MM-DD`
   section; leave `[Unreleased]` empty above it.
3. Set `version` in `[workspace.package]` in `Cargo.toml`, and the
   `baz-core` entry in `[workspace.dependencies]` to match. Run `cargo check`
   so `Cargo.lock` picks it up. **This does not touch
   `packaging/flatpak/cargo-sources.json`**: that file lists only crates with a
   registry `source`, and the two workspace members have none. Re-run
   `python3 packaging/flatpak/check-cargo-sources.py` anyway — it costs a
   second and it is the only thing standing between a lockfile change and a
   broken Flathub build.
4. Add the release to `packaging/flatpak/io.github.mattcree.baz.metainfo.xml`
   with its real date, replacing the placeholder entry (which currently claims
   `0.1.0` on `2026-01-01`, `type="development"`).
5. Commit the four together — `Cargo.toml`, `Cargo.lock`, `CHANGELOG.md`,
   the metainfo. That commit is the release commit.
6. Run the release workflow by hand (`workflow_dispatch`) first. It builds
   every platform and produces the checksums without publishing anything — the
   cheapest way to find out that a runner image has changed. **`workflow_dispatch`
   runs against a branch, not an arbitrary SHA**, so the release commit has to
   be the tip of one; in practice that means push it to `main` and dispatch
   from `main`.
7. Tag it: `git tag -a vX.Y.Z -m "baz X.Y.Z"` and push the tag. The version job
   will reject it if step 3 was missed.
8. Review the draft release, then publish it.
9. Update the Flathub manifest's `tag` and `commit` — `commit` must be the full
   SHA the tag resolves to (`git rev-parse vX.Y.Z`), because Flathub requires
   both so a moved tag cannot change what is built. Regenerate
   `cargo-sources.json` if `Cargo.lock` gained or dropped a dependency;
   `packaging/flatpak/README.md` has the commands.

## What a tag will prove that nothing else can

This workflow has never run in CI. The dry run in step 6 exercises everything
except the four things that only a real tag reaches: the tag-versus-manifest
check's success path, `gh release create --verify-tag`, the release-notes
generation, and the upload of assets to a release. Expect to fix something the
first time.

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
