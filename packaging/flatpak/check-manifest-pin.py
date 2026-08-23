#!/usr/bin/env python3
"""Check that the Flathub manifest's ``commit`` really is its ``tag``'s commit.

Flathub wants a git source pinned by both ``tag`` and ``commit`` so that moving
a tag cannot change what gets built. That protection is only worth anything if
the two agree, and there is a specific, easy way for them not to.

Every release tag in this repository is annotated. ``git rev-parse v0.4.0``
therefore prints the SHA of the *tag object* — a real, 40-character, entirely
valid-looking hash that is not the commit. Flathub will check out the tag, find
a different commit than the manifest names, and fail the build. Nothing about
the wrong hash looks wrong; you cannot catch it by reading.

``docs/RELEASING.md`` step 10 says to use ``git rev-parse vX.Y.Z^{commit}``.
This is that sentence as a check, so that getting it wrong fails in CI on the
release commit rather than in review on a submission.
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

MANIFEST = Path(__file__).with_name("io.github.mattcree.baz.yml")


def git(*args: str) -> tuple[int, str]:
    done = subprocess.run(
        ["git", *args], capture_output=True, text=True, cwd=Path(__file__).parents[2]
    )
    return done.returncode, done.stdout.strip()


def pinned_source(manifest: dict) -> dict:
    """The manifest's own git source — the one pointing at this repository."""
    for module in manifest.get("modules", []):
        if not isinstance(module, dict):
            continue
        for source in module.get("sources", []):
            if not isinstance(source, dict):
                continue
            if source.get("type") == "git" and "mattcree/baz" in source.get("url", ""):
                return source
    sys.exit("check-manifest-pin: no git source for mattcree/baz in the manifest")


def main() -> None:
    try:
        import yaml
    except ImportError:
        sys.exit("check-manifest-pin: needs PyYAML (python3-yaml)")

    source = pinned_source(yaml.safe_load(MANIFEST.read_text()))
    tag, commit = source.get("tag"), source.get("commit")
    if not tag or not commit:
        sys.exit(
            "check-manifest-pin: the git source needs both `tag` and `commit`; "
            f"got tag={tag!r} commit={commit!r}"
        )

    code, want = git("rev-parse", "--verify", "--quiet", f"{tag}^{{commit}}")
    if code != 0:
        # A shallow checkout without tags cannot answer this. Say so loudly
        # rather than passing quietly: a check that skips without a reason is
        # indistinguishable from a check that ran.
        print(
            f"check-manifest-pin: SKIPPED — tag {tag} is not in this checkout, "
            "so the pin could not be verified. Fetch tags to check it.",
            file=sys.stderr,
        )
        return

    if commit != want:
        _, tag_object = git("rev-parse", tag)
        hint = ""
        if commit == tag_object:
            hint = (
                f"\n\n  That is the annotated *tag object* for {tag}, not its commit.\n"
                f"  It is what `git rev-parse {tag}` prints. Use\n"
                f"      git rev-parse {tag}^{{commit}}\n"
                "  which dereferences the tag. See docs/RELEASING.md step 10."
            )
        sys.exit(
            f"check-manifest-pin: {MANIFEST.name} pins tag {tag} to the wrong commit.\n"
            f"  manifest says: {commit}\n"
            f"  {tag} is:      {want}{hint}"
        )

    print(f"check-manifest-pin: {tag} correctly pinned to {want}")


if __name__ == "__main__":
    main()
