#!/usr/bin/env python3
"""List the Contributing section of every README.

The script reads the `# Contributing` section of each README under the areas this tree is made of,
and prints it with the path it belongs to. It is the first command an agent runs when the user asks
what can be contributed, or what a machine needs before it can be: every area states both beside the
code, so the answer comes from the tree rather than from a list that goes stale.

Usage:
    python dev/ask-contributing/list.py
    python dev/ask-contributing/list.py app modules
    python dev/ask-contributing/list.py --json
    python dev/ask-contributing/list.py --strict
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
AREAS = ("app", "modules", "utils", "dev", "tests", "book")
HEADING = "# Contributing"

# Trees that are not this project's own prose: build output, the history, the third-party clones
# and the packages an install leaves behind. `build` is skipped at the root only, because `dev/build`
# is a crate of this tree.
SKIPPED_ROOTS = {".git", ".cache", "build", "THIRD_PARTY"}
SKIPPED_ANYWHERE = {".cache", "node_modules", "vendor"}


def readmes(area: str) -> list[Path]:
    """Every README under `area`, sorted by path."""
    base = ROOT / area
    if not base.is_dir():
        return []

    found = []
    for path in base.rglob("README.md"):
        parts = path.relative_to(ROOT).parts
        if parts[0] in SKIPPED_ROOTS:
            continue
        if any(part in SKIPPED_ANYWHERE for part in parts):
            continue
        found.append(path)

    return sorted(found)


def contributing(path: Path) -> str | None:
    """The `# Contributing` section, heading included, or None when the README has none."""
    lines = path.read_text(encoding="utf-8").splitlines()
    try:
        start = next(i for i, line in enumerate(lines) if line.strip() == HEADING)
    except StopIteration:
        return None

    body = [lines[start]]
    fenced = False
    for line in lines[start + 1 :]:
        stripped = line.strip()
        if stripped.startswith("```"):
            fenced = not fenced
        elif not fenced and line.startswith("# "):
            break
        body.append(line)

    text = "\n".join(body).strip()
    return text or None


def collect(areas: tuple[str, ...]) -> list[dict[str, str]]:
    """What every README in `areas` states, with the path relative to the repository root."""
    found: list[dict[str, str]] = []
    for area in areas:
        area_readme = ROOT / area / "README.md"
        for path in readmes(area):
            relative = path.relative_to(ROOT).as_posix()
            text = contributing(path)
            if text is None:
                if path == area_readme:
                    print(f"warning: {relative} has no {HEADING}", file=sys.stderr)
                continue
            found.append(
                {
                    "area": area,
                    "path": relative,
                    "contributing": text,
                }
            )
    return found


def render_text(found: list[dict[str, str]]) -> None:
    """Print the sections grouped by area, one path and its section under each."""
    current = None
    for entry in found:
        if entry["area"] != current:
            if current is not None:
                print()
            current = entry["area"]
            print(current)
        print(f"  {entry['path']}")
        print()
        for line in entry["contributing"].splitlines():
            print(f"    {line}" if line else "")
        print()


def missing(areas: tuple[str, ...]) -> list[str]:
    """The areas whose own README states no Contributing section."""
    absent = []
    for area in areas:
        path = ROOT / area / "README.md"
        if not path.is_file() or contributing(path) is None:
            absent.append(area)
    return absent


def main() -> int:
    parser = argparse.ArgumentParser(
        description="List the Contributing section of every README under the tree's areas.",
    )
    parser.add_argument(
        "areas",
        nargs="*",
        choices=AREAS,
        metavar="AREA",
        help="one or more of: app, modules, utils, dev, tests, book. The default is all six.",
    )
    parser.add_argument(
        "--json",
        action="store_true",
        help="print the result as JSON instead of text.",
    )
    parser.add_argument(
        "--strict",
        action="store_true",
        help="exit 1 when an area's own README states no Contributing section.",
    )
    args = parser.parse_args()

    areas = tuple(args.areas) if args.areas else AREAS
    found = collect(areas)
    if args.json:
        json.dump(found, sys.stdout, ensure_ascii=False, indent=2)
        print()
    else:
        render_text(found)

    if args.strict and missing(areas):
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
