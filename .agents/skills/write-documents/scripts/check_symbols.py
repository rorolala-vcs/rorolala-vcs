#!/usr/bin/env python3
"""Report banned symbols and sentence punctuation outside the project set.

By default the script scans user-facing copy: the i18n files under `app/cli/` and `app/desktop/`.
Pass paths to check comments, documentation or any other text. Inline code, fenced blocks and
URLs are skipped unless `--include-code` is given. Findings are candidates; a person decides. The
script exits 0 unless `--strict` is given.
"""

from __future__ import annotations

import argparse
import sys
import unicodedata
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

BANNED = {
    "\u2014": ("U+2014 EM DASH", "banned dash"),
    "\u2013": ("U+2013 EN DASH", "banned dash"),
    "\u2012": ("U+2012 FIGURE DASH", "banned dash"),
    "\u2015": ("U+2015 HORIZONTAL BAR", "banned dash"),
    "\u2212": ("U+2212 MINUS SIGN", "banned dash"),
    "\u2e3a": ("U+2E3A TWO-EM DASH", "banned dash"),
    "\u2e3b": ("U+2E3B THREE-EM DASH", "banned dash"),
    "\u2026": ("U+2026 HORIZONTAL ELLIPSIS", "banned decorative mark"),
    "\u00b7": ("U+00B7 MIDDLE DOT", "banned decorative mark"),
    "\u2022": ("U+2022 BULLET", "banned decorative mark"),
    "\u30fb": ("U+30FB KATAKANA MIDDLE DOT", "banned decorative mark"),
    "\u301c": ("U+301C WAVE DASH", "banned decorative mark"),
    "\uff5e": ("U+FF5E FULLWIDTH TILDE", "banned decorative mark"),
    "\u007e": ("U+007E TILDE", "banned decorative mark"),
}

ALLOWED = {
    "zh": set("：，。、“”；！？"),
    "en": set(':,.";!?'),
}

LANG_NAME = {"zh": "Chinese", "en": "English"}

COMMON_EXEMPT = set("-'_/\\@#%&*()[]{}<>") | {"\u2010", "\u2011", "\u2019"}
CJK_BRACKETS = set("（）［］｛｝【】〔〕〈〉《》")

EXEMPT = {
    "en": COMMON_EXEMPT,
    "zh": COMMON_EXEMPT | CJK_BRACKETS,
}


def scan(line: Line) -> list[Finding]:
    """Return every candidate in one line of copy."""
    findings: list[Finding] = []
    text = line.text
    allowed = ALLOWED[line.language]
    exempt = EXEMPT[line.language]
    index = 0
    while index < len(text):
        char = text[index]
        if char == "\u2014" and text[index + 1 : index + 2] == "\u2014":
            findings.append(_finding(line, index, "——", "banned double dash"))
            index += 2
            continue
        if char == "\u2026" and text[index + 1 : index + 2] == "\u2026":
            findings.append(_finding(line, index, "……", "banned decorative mark (……)"))
            index += 2
            continue
        banned = BANNED.get(char)
        if banned is not None:
            name, reason = banned
            findings.append(_finding(line, index, char, f"{reason} ({name})"))
            index += 1
            continue
        if char == "-" and _spaced(text, index):
            findings.append(_finding(line, index, char, "hyphen used as a dash"))
            index += 1
            continue
        if (
            unicodedata.category(char).startswith("P")
            and char not in allowed
            and char not in exempt
        ):
            findings.append(
                _finding(
                    line,
                    index,
                    char,
                    f"outside the {LANG_NAME[line.language]} sentence set",
                )
            )
        index += 1
    return findings


def _spaced(text: str, index: int) -> bool:
    if not text[:index].strip():
        return False
    if index + 1 >= len(text):
        return False
    return text[index - 1].isspace() and text[index + 1].isspace()


def _finding(line: Line, index: int, symbol: str, reason: str) -> Finding:
    return Finding(
        path=line.path,
        line=line.number,
        column=line.offset + index + 1,
        symbol=symbol,
        reason=reason,
        excerpt=excerpt(line.text, index),
    )


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Report banned symbols and sentence punctuation outside the project set."
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
    return report(findings, files, strict=args.strict, title="check_symbols")


if __name__ == "__main__":
    sys.exit(main())
