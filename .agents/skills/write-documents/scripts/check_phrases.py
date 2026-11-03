#!/usr/bin/env python3
"""Report negation-then-affirmation and template phrasing in user-facing copy.

Scope and masking match `check_symbols.py`: the user-facing i18n files by default, with inline
code, fenced blocks and URLs skipped unless `--include-code` is given. Findings are candidates;
a person decides.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from _common import (
    Finding,
    Line,
    default_files,
    excerpt,
    expand,
    report,
    visible_lines,
)

PATTERNS = (
    (
        "zh",
        re.compile(r"不是[^。！？\n]{1,30}?而是"),
        "negation then affirmation (不是 X 而是 Y)",
    ),
    (
        "zh",
        re.compile(r"并非[^。！？\n]{1,30}?而是"),
        "negation then affirmation (并非 X 而是 Y)",
    ),
    (
        "zh",
        re.compile(r"不仅仅是?[^。！？\n]{1,30}?而是"),
        "negation then affirmation (不仅 X 而是 Y)",
    ),
    (
        "zh",
        re.compile(r"不，只是"),
        "negation then affirmation (不，只是)",
    ),
    (
        "zh",
        re.compile(r"由[^。！？\n]{1,30}?等一系列"),
        "template setup (由 X 等一系列)",
    ),
    (
        "en",
        re.compile(r"\bnot\s+(?!only\b)[^.!?\n]{1,60}?\bbut\b", re.IGNORECASE),
        "negation then affirmation (not X but Y)",
    ),
    (
        "en",
        re.compile(
            r"\bno,\s+(?:it|that|this)\s+is\s+(?:just|merely|simply)\b",
            re.IGNORECASE,
        ),
        "negation then affirmation (no, it is just X)",
    ),
    (
        "en",
        re.compile(r"\bthrough\s+a\s+series\s+of\b", re.IGNORECASE),
        "template setup (through a series of X)",
    ),
)


def scan(line: Line) -> list[Finding]:
    """Return every candidate in one line of copy."""
    findings: list[Finding] = []
    for language, pattern, reason in PATTERNS:
        if language != line.language:
            continue
        for match in pattern.finditer(line.text):
            findings.append(
                Finding(
                    path=line.path,
                    line=line.number,
                    column=line.offset + match.start() + 1,
                    symbol=match.group(0),
                    reason=reason,
                    excerpt=excerpt(line.text, match.start()),
                )
            )
    return findings


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description=(
            "Report negation-then-affirmation and template phrasing in user-facing copy."
        )
    )
    parser.add_argument(
        "paths",
        nargs="*",
        help="files or directories to scan; default: the user-facing i18n files under app/",
    )
    parser.add_argument(
        "--include-code",
        action="store_true",
        help="scan inline code and fenced blocks too",
    )
    parser.add_argument(
        "--strict",
        action="store_true",
        help="exit 1 when there are candidates",
    )
    args = parser.parse_args(argv)

    files = expand(args.paths) if args.paths else default_files()
    findings: list[Finding] = []
    for path in files:
        for line in visible_lines(path, args.include_code):
            findings.extend(scan(line))
    return report(findings, files, strict=args.strict, title="check_phrases")


if __name__ == "__main__":
    sys.exit(main())
