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

## Cutting a release

1. `main` is green, and the tree is the tree you mean to ship.
2. Move `CHANGELOG.md`'s `[Unreleased]` content into a `## [X.Y.Z] - YYYY-MM-DD`
   section; leave `[Unreleased]` empty above it.
3. Set `version` in `[workspace.package]` in `Cargo.toml`, and the
   `baz-core` entry in `[workspace.dependencies]` to match. Run `cargo check`
   so `Cargo.lock` picks it up.
4. Add the release to `packaging/flatpak/io.github.mattcree.baz.metainfo.xml`
   with its real date, replacing the placeholder entry.
5. Commit the four together. That commit is the release commit.
6. Run the release workflow by hand (`workflow_dispatch`) from that commit
   first. It builds every platform and produces the checksums without
   publishing anything — the cheapest way to find out that a runner image has
   changed.
7. Tag it: `git tag -a vX.Y.Z -m "baz X.Y.Z"` and push the tag. The version job
   will reject it if step 3 was missed.
8. Review the draft release, then publish it.
9. Update the Flathub manifest's `tag` and `commit`, and regenerate
   `cargo-sources.json` if `Cargo.lock` changed — `packaging/flatpak/README.md`
   has the commands.

## What a tag will prove that nothing else can

This workflow has never run. The dry run in step 6 exercises everything except
the four things that only a real tag reaches: the tag-versus-manifest check's
success path, `gh release create --verify-tag`, the release-notes generation,
and the upload of assets to a release. Expect to fix something the first time.
