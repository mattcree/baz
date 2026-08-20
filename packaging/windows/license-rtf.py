#!/usr/bin/env python3
"""Render `LICENSE` as the RTF the Windows installer shows.

**Generated, never committed.** WiX wants RTF for its licence page and baz's
licence is the GPL-3 text in `LICENSE`; keeping a second copy in a second
format is how the two come to disagree. This is run by the release workflow
immediately before `cargo wix`, from the same file every other packaging route
ships, so the installer cannot show a licence the project does not have.

RTF escaping is three characters — backslash and both braces — plus a
paragraph break per line. Nothing here needs a library.
"""

import pathlib
import sys

ESCAPES = str.maketrans({"\\": r"\\", "{": r"\{", "}": r"\}"})


def main() -> int:
    root = pathlib.Path(__file__).resolve().parents[2]
    text = (root / "LICENSE").read_text(encoding="utf-8")
    body = "".join(
        f"{line.translate(ESCAPES)}\\par\n" for line in text.splitlines()
    )
    # `\fs16` is 8 pt in RTF's half-points: the licence page is a small scroll
    # box and the whole preamble should be reachable without forty pages of
    # scrolling. A monospaced face keeps the GPL's own hand-set indentation.
    rtf = (
        r"{\rtf1\ansi\ansicpg1252\deff0"
        r"{\fonttbl{\f0\fmodern\fcharset0 Consolas;}}"
        "\n\\f0\\fs16\n" + body + "}"
    )
    out = root / "packaging" / "windows" / "License.rtf"
    out.write_text(rtf, encoding="cp1252", errors="replace")
    print(f"wrote {out} ({len(rtf)} bytes)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
