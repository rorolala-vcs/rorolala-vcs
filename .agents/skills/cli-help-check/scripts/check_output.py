#!/usr/bin/env python3
"""Roughly check the real `--help` output of rola.

Runs the built binary with `--no-color` and measures the right-hand column of
each two-column table:

- do all right-hand texts start at the same column?
- does that column sit four display columns after the widest left cell?
- which lines exceed 80 display columns?

Width follows Unicode East Asian Width (W/F = 2), matching the program's own
`ansi::display_width`, which uses `unicode_width`.

This is a discovery tool. A help block may hold several tables, and a prose
line may contain 2+ spaces that look like a column gap; open the source and
compare before reporting a defect. `--strict` exits 1 on findings, but it is
not a release gate.
"""

from __future__ import annotations

import argparse
import re
import shlex
import subprocess
import sys
import unicodedata
from pathlib import Path

if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8")

ENTRY = re.compile(r"^(\s*)(\S.*?)(\s{2,})(\S.*)$")
LANGS = ("en", "zh-CN")


def project_root() -> Path:
    here = Path(__file__).resolve()
    for parent in here.parents:
        if (parent / "app" / "cli" / "rola" / "i18n").is_dir():
            return parent
    return Path.cwd()


def default_binary(root: Path) -> Path:
    for name in ("rola.exe", "rola"):
        candidate = root / "build" / "bin" / name
        if candidate.is_file():
            return candidate
    return root / "build" / "bin" / "rola.exe"


def display_width(text: str) -> int:
    total = 0
    for ch in text:
        if unicodedata.combining(ch) or unicodedata.category(ch) in ("Mn", "Me", "Cf"):
            continue
        total += 2 if unicodedata.east_asian_width(ch) in ("W", "F") else 1
    return total


def check(label: str, text: str) -> bool:
    lines = text.splitlines()
    rows = []
    for line in lines:
        match = ENTRY.match(line)
        if not match:
            continue
        indent, left, _, right = match.groups()
        if left.startswith(("[", "#")):
            continue
        rows.append(
            (
                display_width(indent) + display_width(left),
                display_width(line[: match.start(4)]),
                left,
                right,
            )
        )

    print("=" * 72)
    if not rows:
        print(f"{label}: no two-column lines")
        return True

    starts = sorted({right for _, right, _, _ in rows})
    widest_left = max(left for left, _, _, _ in rows)
    gap = starts[0] - widest_left
    aligned = len(starts) == 1
    ok = aligned and gap == 4

    print(
        f"{label}: rows={len(rows)} right_starts={starts} "
        f"widest_left_end={widest_left} gap_after_widest={gap} "
        f"expected={widest_left + 4} aligned={aligned} ok={ok}"
    )
    if not aligned:
        for left, right, cell, _ in rows:
            print(f"  left_end={left:3} right@{right:3} gap={right - left:2}  {cell!r}")
    else:
        for left, right, cell, _ in rows:
            if left == widest_left:
                print(f"  widest row: left_end={left:3} right@{right:3} gap={right - left:2}  {cell!r}")

    shown = [(display_width(line), line) for line in lines if line.strip()]
    widest = max(width for width, _ in shown)
    over = sum(1 for width, _ in shown if width > 80)
    print(f"  widest_line={widest} over80={over}")
    for width, line in shown:
        if width > 80:
            print(f"    {width:3} {line!r}")
    return ok


def run(binary: Path, parts: list[str], lang: str) -> str:
    argv = [str(binary), "--no-color", "--lang", lang, *parts, "--help"]
    result = subprocess.run(
        argv,
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        check=False,
    )
    # Help is written to stderr; a run that reports a usage error writes there too.
    return (result.stderr or "") + (result.stdout or "")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("commands", nargs="*", help="command paths, e.g. checkin or 'storage ls-remote-manifests'")
    parser.add_argument("--command", action="append", default=[], help="a command path containing spaces")
    parser.add_argument("--file", action="append", default=[], help="a captured --help output (repeatable)")
    parser.add_argument("--bin", help="the rola binary; defaults to build/bin/rola[.exe]")
    parser.add_argument("--strict", action="store_true", help="exit 1 on findings; not a release gate")
    args = parser.parse_args()

    root = project_root()
    binary = Path(args.bin) if args.bin else default_binary(root)
    commands = [shlex.split(command) for command in args.commands + args.command]
    if not commands and not args.file:
        commands = [[]]

    results: list[bool] = []
    if not binary.is_file():
        print(f"binary not found: {binary}", file=sys.stderr)
        return 2

    for command in commands:
        label_command = " ".join(command) or "<top-level>"
        for lang in LANGS:
            results.append(check(f"{label_command} --lang {lang}", run(binary, command, lang)))

    for path in args.file:
        results.append(check(str(path), Path(path).read_text(encoding="utf-8")))

    if not results:
        print("nothing to check", file=sys.stderr)
        return 2
    if not all(results):
        print()
        print("One or more blocks need a look. This is a heuristic; open the YAML line and compare.")
        return 1 if args.strict else 0
    return 0


if __name__ == "__main__":
    sys.exit(main())
