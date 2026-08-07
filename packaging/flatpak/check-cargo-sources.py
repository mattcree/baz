#!/usr/bin/env python3
"""Check packaging/flatpak/cargo-sources.json against Cargo.lock.

A Flathub build has no network, so every crate baz depends on is listed in
`cargo-sources.json` as a URL and a SHA-256. That file is generated from
Cargo.lock, which means it can fall out of step with it — and the symptom
would be a Flathub build failing long after the dependency change that broke
it. This turns that into a CI failure on the pull request instead.

It verifies, not regenerates: the check is that the set of
(name, version, sha256) triples in Cargo.lock is exactly the set the manifest
vendors. Regeneration stays the job of the upstream tool, which is the one
that knows the vendoring layout:

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
    entries = json.loads(SOURCES.read_text(encoding="utf-8"))
    have = set()
    for entry in entries:
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


def main() -> int:
    wanted, have = from_lock(), from_sources()
    if wanted == have:
        print(f"cargo-sources.json matches Cargo.lock ({len(wanted)} crates)")
        return 0

    for label, diff in (("missing from", wanted - have), ("stale in", have - wanted)):
        for name, version, _ in sorted(diff):
            print(f"{label} cargo-sources.json: {name} {version}", file=sys.stderr)
    print(
        "\ncargo-sources.json is out of step with Cargo.lock. Regenerate it "
        "with flatpak-cargo-generator.py (command in this script's docstring) "
        "and commit the result alongside the lockfile change.",
        file=sys.stderr,
    )
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
