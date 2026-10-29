#!/usr/bin/env python3
"""Roughly find layout problems in rola's `--help` copy.

This reads the i18n YAML and every `#[help(buffer)]` key, then reports, per
language and block:

- the right-column starts and whether they agree;
- the widest left cell and the gap before the right column;
- lines wider than 80 display columns.

It is a discovery tool, not an assertion. A help block may hold several tables,
and a prose line may contain 2+ spaces that look like a column gap. Open the
cited YAML line and check the real output before calling anything a defect.

Width follows Unicode East Asian Width (W/F = 2), matching the program's own
`ansi::display_width`, which uses `unicode_width`.
"""

from __future__ import annotations

import os
import re
import sys
import unicodedata
from pathlib import Path

if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8")

ENTRY = re.compile(r"^(\s*)(\S.*?)(\s{2,})(\S.*)$")
HELP_HANDLER = re.compile(r"#\[help\(buffer\)\]")


def project_root() -> Path:
    """The repository root, found by walking up to `app/cli/rola/i18n`."""
    here = Path(__file__).resolve()
    for parent in here.parents:
        if (parent / "app" / "cli" / "rola" / "i18n").is_dir():
            return parent
    return Path.cwd()


def display_width(text: str) -> int:
    total = 0
    for ch in text:
        if unicodedata.combining(ch) or unicodedata.category(ch) in ("Mn", "Me", "Cf"):
            continue
        total += 2 if unicodedata.east_asian_width(ch) in ("W", "F") else 1
    return total


def render_line(line: str) -> str:
    """Approximate `trd!`: remove paired emphasis markers, keep code and flags."""
    for delimiter in ("***", "**", "__", "~~"):
        line = re.sub(re.escape(delimiter) + r"(.+?)" + re.escape(delimiter), r"\1", line)
    return re.sub(r"(?<!\*)\*(?!\*)(.+?)(?<!\*)\*(?!\*)", r"\1", line)


def help_keys(source_root: Path) -> dict[str, Path]:
    """Every `t!("...help")` called from a `#[help(buffer)]` handler."""
    found: dict[str, Path] = {}
    for path in sorted(source_root.rglob("*.rs")):
        text = path.read_text(encoding="utf-8")
        for handler in HELP_HANDLER.finditer(text):
            tail = text[handler.end() : handler.end() + 1200]
            key = re.search(r't!\("([^"]+help)"\)', tail)
            if key:
                found[key.group(1)] = path
    return found


def block_scalars(path: Path) -> dict[str, dict[str, list[str]]]:
    """Read the `en` / `zh-CN` block scalars, keyed by their YAML path."""
    lines = path.read_text(encoding="utf-8").splitlines()
    blocks: dict[str, dict[str, list[str]]] = {}
    stack: list[tuple[int, str]] = []
    i = 0
    while i < len(lines):
        raw = lines[i]
        stripped = raw.strip()
        if not stripped or stripped.startswith("#"):
            i += 1
            continue
        indent = len(raw) - len(raw.lstrip(" "))
        while stack and stack[-1][0] >= indent:
            stack.pop()
        match = re.match(r"^\s*([^:\s][^:]*?):\s*(.*)$", raw)
        if not match:
            i += 1
            continue
        key, value = match.group(1).strip(), match.group(2).strip()
        dotted = ".".join([k for _, k in stack] + [key])
        if value in ("|", "|-", "|+", ">", ">-"):
            body: list[str] = []
            j = i + 1
            content_indent = None
            while j < len(lines):
                if not lines[j].strip():
                    body.append("")
                    j += 1
                    continue
                ind = len(lines[j]) - len(lines[j].lstrip(" "))
                if ind <= indent:
                    break
                if content_indent is None:
                    content_indent = ind
                if ind < content_indent:
                    break
                body.append(lines[j][content_indent:])
                j += 1
            blocks[dotted] = blocks.get(dotted, {})
            blocks[dotted]["__body__"] = body
            blocks[dotted]["__path__"] = [str(path)]
            i = j
            continue
        stack.append((indent, key))
        i += 1
    return blocks


def two_column_rows(body: list[str]) -> list[tuple[int, int, str, str]]:
    rows = []
    for line in body:
        rendered = render_line(line)
        match = ENTRY.match(rendered)
        if not match:
            continue
        indent, left, _, right = match.groups()
        if left.startswith(("[", "#")):
            continue
        rows.append(
            (
                display_width(indent) + display_width(left),
                display_width(rendered[: match.start(4)]),
                left,
                right,
            )
        )
    return rows


def main() -> int:
    root = project_root()
    source_root = root / "app" / "cli" / "rola" / "src"
    i18n_root = root / "app" / "cli" / "rola" / "i18n"

    wanted = help_keys(source_root)
    if not wanted:
        print(f"no #[help(buffer)] handlers found under {source_root}", file=sys.stderr)
        return 2

    by_key: dict[str, dict[str, object]] = {}
    for yml in sorted(i18n_root.rglob("*.yml")):
        for dotted, values in block_scalars(yml).items():
            if dotted.endswith(".en") or dotted.endswith(".zh-CN"):
                key = dotted.rsplit(".", 1)[0]
                lang = dotted.rsplit(".", 1)[1]
                by_key.setdefault(key, {})[lang] = (values["__body__"], values["__path__"][0])

    print("key                            lang   rows   right@      widest  gap  aligned  widest_line  >80")
    findings = 0
    for key in sorted(wanted):
        if key not in by_key:
            print(f"{key:30} --     NOT FOUND")
            continue
        for lang in ("en", "zh-CN"):
            if lang not in by_key[key]:
                continue
            body, _ = by_key[key][lang]
            rows = two_column_rows(body)
            if not rows:
                widest = max((display_width(render_line(line)) for line in body if line.strip()), default=0)
                over = sum(1 for line in body if line.strip() and display_width(render_line(line)) > 80)
                print(f"{key:30} {lang:6} 0      -           -      -    -        {widest:>11}  {over:>3}  (no table)")
                continue
            starts = sorted({right for _, right, _, _ in rows})
            widest_left = max(left for left, _, _, _ in rows)
            gap = starts[0] - widest_left
            aligned = len(starts) == 1
            shown = [display_width(render_line(line)) for line in body if line.strip()]
            widest_line = max(shown, default=0)
            over = sum(1 for width in shown if width > 80)
            if not aligned or gap != 4:
                findings += 1
            print(
                f"{key:30} {lang:6} {len(rows):>4}  {','.join(map(str, starts)):<10} "
                f"{widest_left:>6} {gap:>4}  {str(aligned):<7} {widest_line:>11}  {over:>3}"
                + ("" if aligned and gap == 4 else "  <--")
            )
            if not aligned:
                for left, right, cell, _ in rows:
                    print(f"    left_end={left:3} right@{right:3} gap={right - left:2}  {cell!r}")

    print()
    print("Heuristic only. Open the YAML line and compare the real --help output before reporting.")
    print(f"blocks with a table that needs a look: {findings}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
