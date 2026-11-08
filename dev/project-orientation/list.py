#!/usr/bin/env python3
"""List the Introduction of every module README.

The script reads the `## Introduction` section of each README under `app/`, `modules/`, `utils/`
and `dev/`, and prints it with the path of the module it belongs to. It is the first command an
agent runs when it has to learn what a part of the tree is for: the text is short, the set is
complete, and the source remains the README beside the code.

Usage:
    python dev/project-orientation/list.py
    python dev/project-orientation/list.py modules utils
    python dev/project-orientation/list.py --json
    python dev/project-orientation/list.py --strict
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
AREAS = ("app", "modules", "utils", "dev")
INTRO_HEADING = "## Introduction"


def readmes(area: str) -> list[Path]:
    """Every README a module of `area` owns, sorted by path."""
    base = ROOT / area
    if not base.is_dir():
        return []

    paths = list(base.glob("*/README.md"))
    if area == "app":
        paths.extend((base / "cli").glob("*/README.md"))
    if area == "dev":
        # The packaging assets are not a crate, so they are the one directory below a direct
        # child that carries a README of its own.
        paths.append(base / "packaging" / "linux" / "README.md")

    return sorted(path for path in paths if path.is_file())


def introduction(path: Path) -> str | None:
    """The text under `## Introduction`, joined into one line."""
    lines = path.read_text(encoding="utf-8").splitlines()
    try:
        start = next(i for i, line in enumerate(lines) if line.strip() == INTRO_HEADING)
    except StopIteration:
        return None

    body: list[str] = []
    for line in lines[start + 1 :]:
        if line.startswith("## "):
            break
        body.append(line.strip())

    text = " ".join(part for part in body if part)
    return text or None


def collect(areas: tuple[str, ...]) -> list[dict[str, str]]:
    """What every selected area states, with the path relative to the repository root."""
    found: list[dict[str, str]] = []
    for area in areas:
        for path in readmes(area):
            relative = path.parent.relative_to(ROOT).as_posix()
            text = introduction(path)
            if text is None:
                print(
                    f"warning: {relative}/README.md has no {INTRO_HEADING}",
                    file=sys.stderr,
                )
                continue
            found.append(
                {
                    "area": area,
                    "path": relative,
                    "introduction": text,
                }
            )
    return found


def render_text(found: list[dict[str, str]]) -> None:
    """Print the modules grouped by area, one introduction under each path."""
    current = None
    for entry in found:
        if entry["area"] != current:
            if current is not None:
                print()
            current = entry["area"]
            print(current)
        print(f"  {entry['path']}")
        print(f"    {entry['introduction']}")


def main() -> int:
    parser = argparse.ArgumentParser(
        description="List the Introduction of every app, modules, utils and dev README.",
    )
    parser.add_argument(
        "areas",
        nargs="*",
        choices=AREAS,
        metavar="AREA",
        help="one or more of: app, modules, utils, dev. The default is all four.",
    )
    parser.add_argument(
        "--json",
        action="store_true",
        help="print the result as JSON instead of text.",
    )
    parser.add_argument(
        "--strict",
        action="store_true",
        help="exit 1 when a selected README has no Introduction.",
    )
    args = parser.parse_args()

    areas = tuple(args.areas) if args.areas else AREAS
    found = collect(areas)
    if args.json:
        json.dump(found, sys.stdout, ensure_ascii=False, indent=2)
        print()
    else:
        render_text(found)

    if args.strict and len(found) != sum(len(readmes(area)) for area in areas):
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
