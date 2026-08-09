#!/usr/bin/env python3
"""Check packaging/flatpak/cargo-sources.json against Cargo.lock.

A Flathub build has no network, so every crate baz depends on is listed in
`cargo-sources.json` as a URL and a SHA-256. That file is generated from
Cargo.lock, which means it can fall out of step with it — and the symptom
would be a Flathub build failing long after the dependency change that broke
it. This turns that into a CI failure on the pull request instead.

It verifies, not regenerates. Two things are checked, and the second exists
because the first was not enough:

1. the set of (name, version, sha256) triples in Cargo.lock is exactly the set
   the manifest vendors;
2. every vendored crate also gets its `.cargo-checksum.json`, and the vendor
   config is present.

**Why (2).** A vendored crate directory that cargo will accept is *two* entries
in this file: the `archive` that unpacks the `.crate`, and an `inline` that
writes `.cargo-checksum.json` beside it. Cargo reads the second for every
crate in the directory before it resolves anything, so one missing inline
fails the whole build with an error naming a crate that has nothing to do with
what is being compiled. That is exactly what the committed file did — seven
archives had no checksum beside them, check (1) passed because it only looked
at archives, and the first Flatpak build anyone ran died on
`failed to load checksum '.cargo-checksum.json' of block2 v0.6.2` while
resolving `cpal`. The generator produces both halves; a hand-edit or a partial
regeneration produces one.

Regeneration stays the job of the upstream tool, which is the one that knows
the vendoring layout:

    pip install tomlkit aiohttp        # in a virtualenv
    curl -sLO https://raw.githubusercontent.com/flatpak/flatpak-builder-tools/master/cargo/flatpak-cargo-generator.py
    python3 flatpak-cargo-generator.py Cargo.lock \\
        -o packaging/flatpak/cargo-sources.json

Standard library only (tomllib needs Python 3.11+): CI installs nothing for it.
"""

from __future__ import annotations

import json
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
LOCK = ROOT / "Cargo.lock"
SOURCES = ROOT / "packaging" / "flatpak" / "cargo-sources.json"

REGISTRY = "registry+https://github.com/rust-lang/crates.io-index"
CRATE_URL = "https://static.crates.io/crates/{name}/{name}-{version}.crate"


def from_lock() -> set[tuple[str, str, str]]:
    """The published crates Cargo.lock pins, as (name, version, sha256)."""
    lock = tomllib.loads(LOCK.read_text(encoding="utf-8"))
    wanted = set()
    for pkg in lock["package"]:
        source = pkg.get("source")
        if source is None:
            continue  # a workspace member: it comes from the git source
        if source != REGISTRY:
            sys.exit(
                f"{pkg['name']} {pkg['version']} comes from {source!r}, not "
                f"crates.io. This check only understands registry crates; a "
                f"git or alternate-registry dependency needs the manifest and "
                f"this script extended together."
            )
        wanted.add((pkg["name"], pkg["version"], pkg["checksum"]))
    return wanted


def from_sources() -> set[tuple[str, str, str]]:
    """The crates cargo-sources.json vendors, as (name, version, sha256)."""
    have = set()
    for entry in _entries():
        if entry.get("type") != "archive":
            continue  # inline .cargo-checksum.json files and the vendor config
        url = entry["url"]
        prefix = "https://static.crates.io/crates/"
        if not url.startswith(prefix) or not url.endswith(".crate"):
            sys.exit(f"unexpected archive url in cargo-sources.json: {url}")
        name, filename = url[len(prefix) : -len(".crate")].split("/", 1)
        version = filename[len(name) + 1 :]
        have.add((name, version, entry["sha256"]))
    return have


def _entries() -> list[dict]:
    return json.loads(SOURCES.read_text(encoding="utf-8"))


def vendor_layout_is_complete() -> list[str]:
    """Complaints about the vendor directory the sources lay down, if any.

    Cargo scans every directory under the replaced source before it resolves a
    single dependency, so a crate unpacked without its `.cargo-checksum.json`
    fails the build — with an error that names the crate missing the file
    rather than anything to do with what was being compiled.
    """
    entries = _entries()
    unpacked = {e["dest"] for e in entries if e.get("type") == "archive"}
    checksummed = {
        e["dest"] for e in entries if e.get("dest-filename") == ".cargo-checksum.json"
    }
    config = {
        e.get("dest-filename")
        for e in entries
        if e.get("dest") == "cargo" and str(e.get("dest-filename", "")).startswith("config")
    }

    problems = []
    for dest in sorted(unpacked - checksummed):
        problems.append(f"{dest} is unpacked with no .cargo-checksum.json beside it")
    for dest in sorted(checksummed - unpacked):
        problems.append(f"{dest} has a .cargo-checksum.json but nothing is unpacked there")
    if not config:
        problems.append(
            "no cargo/config(.toml) entry: nothing tells cargo to replace crates-io "
            "with the vendored directory"
        )
    return problems


def main() -> int:
    wanted, have = from_lock(), from_sources()
    ok = True

    if wanted == have:
        print(f"cargo-sources.json matches Cargo.lock ({len(wanted)} crates)")
    else:
        ok = False
        for label, diff in (("missing from", wanted - have), ("stale in", have - wanted)):
            for name, version, _ in sorted(diff):
                print(f"{label} cargo-sources.json: {name} {version}", file=sys.stderr)

    problems = vendor_layout_is_complete()
    if problems:
        ok = False
        for problem in problems:
            print(f"vendor layout: {problem}", file=sys.stderr)
    else:
        print(f"every vendored crate carries its checksum ({len(have)} of {len(have)})")

    if ok:
        return 0
    print(
        "\ncargo-sources.json is out of step with Cargo.lock, or is not a "
        "complete vendor directory. Regenerate it with "
        "flatpak-cargo-generator.py (command in this script's docstring) and "
        "commit the result alongside the lockfile change — do not hand-edit it.",
        file=sys.stderr,
    )
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
