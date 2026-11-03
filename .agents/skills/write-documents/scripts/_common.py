"""Shared reading for the write-documents checkers.

Both checkers look for candidates in user-facing copy. This module resolves the project root,
finds the i18n files, and yields each line with its language and with code spans, fenced blocks
and URLs masked out. Nothing here decides whether a candidate is a defect.
"""

from __future__ import annotations

import re
from collections.abc import Iterable, Iterator
from dataclasses import dataclass
from pathlib import Path

YAML_SUFFIXES = {".yml", ".yaml"}
TEXT_SUFFIXES = YAML_SUFFIXES | {".md", ".txt"}

DEFAULT_GLOBS = (
    "app/cli/*/i18n/**/*.yml",
    "app/desktop/**/i18n/**/*.yml",
)

LANG_KEY = re.compile(
    r"^(?P<indent>\s*)(?P<key>en|zh-CN|zh_Cn|zh_CN|zh)\s*:\s*(?P<rest>.*)$"
)
BLOCK_VALUE = re.compile(r"^[|>][+-]?\s*$")
FENCE = re.compile(r"^\s*(?:```|~~~)")
ORDERED_MARKER = re.compile(r"^\s*\d+[.)](?=\s)")
CJK = re.compile(r"[\u3400-\u4dbf\u4e00-\u9fff\uf900-\ufaff]")
URL = re.compile(r"(?:https?://|www\.)\S+", re.IGNORECASE)


@dataclass(frozen=True)
class Line:
    """One line of user-facing copy, ready to scan."""

    path: Path
    number: int
    text: str
    language: str
    offset: int = 0


@dataclass(frozen=True)
class Finding:
    """One candidate the checkers report, without a verdict."""

    path: Path
    line: int
    column: int
    symbol: str
    reason: str
    excerpt: str


def project_root() -> Path:
    """Return the repository root by looking for the CLI i18n directory."""
    here = Path(__file__).resolve()
    for parent in here.parents:
        if (parent / "app" / "cli" / "rola" / "i18n").is_dir():
            return parent
    return Path.cwd()


def default_files() -> list[Path]:
    """Return the user-facing i18n files under the project root."""
    root = project_root()
    seen: set[Path] = set()
    files: list[Path] = []
    for pattern in DEFAULT_GLOBS:
        for path in sorted(root.glob(pattern)):
            if path.is_file() and path not in seen:
                seen.add(path)
                files.append(path)
    return files


def expand(paths: Iterable[str]) -> list[Path]:
    """Turn files and directories into a list of files."""
    files: list[Path] = []
    for raw in paths:
        path = Path(raw)
        if path.is_dir():
            for found in sorted(path.rglob("*")):
                if found.is_file() and found.suffix.lower() in TEXT_SUFFIXES:
                    files.append(found)
        elif path.is_file():
            files.append(path)
        else:
            raise SystemExit(f"no such file or directory: {raw}")
    return files


def visible_lines(path: Path, include_code: bool) -> Iterator[Line]:
    """Yield each line of copy with code, URLs and structural syntax removed."""
    try:
        text = path.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError) as error:
        raise SystemExit(f"cannot read {path}: {error}") from error

    is_yaml = path.suffix.lower() in YAML_SUFFIXES
    is_markdown = path.suffix.lower() == ".md"
    language = ""
    block_indent: int | None = None
    in_fence = False

    for number, raw in enumerate(text.splitlines(), 1):
        if is_yaml:
            if block_indent is not None:
                stripped = raw.lstrip()
                if stripped and len(raw) - len(stripped) <= block_indent:
                    block_indent = None
                else:
                    yield Line(
                        path, number, _mask(raw, include_code), language or _guess(raw)
                    )
                    continue
            if raw.lstrip().startswith("#"):
                continue
            match = LANG_KEY.match(raw)
            if match is None:
                continue
            language = "zh" if match.group("key").lower().startswith("zh") else "en"
            rest = match.group("rest")
            if BLOCK_VALUE.match(rest.strip()):
                block_indent = len(match.group("indent"))
                continue
            value = _strip_comment(rest).rstrip()
            start = match.start("rest")
            if len(value) >= 2 and value[0] == value[-1] and value[0] in ('"', "'"):
                value = value[1:-1]
                start += 1
            if value.strip():
                yield Line(
                    path,
                    number,
                    _mask(value, include_code),
                    language,
                    start,
                )
            continue

        if FENCE.match(raw):
            in_fence = not in_fence
            continue
        if in_fence:
            continue
        if is_markdown:
            raw = ORDERED_MARKER.sub(lambda match: " " * len(match.group(0)), raw)
        yield Line(path, number, _mask(raw, include_code), _guess(raw))


def report(
    findings: list[Finding], files: list[Path], *, strict: bool, title: str
) -> int:
    """Print candidates and a summary; return the process exit code."""
    for finding in findings:
        print(
            f"{_display(finding.path)}:{finding.line}:{finding.column}: "
            f"{finding.reason}: {finding.symbol!r}"
        )
        if finding.excerpt:
            print(f"    {finding.excerpt}")
    if not findings:
        print(f"{title}: no candidates in {len(files)} file(s)")
        return 0
    print()
    print(f"{title}: {len(findings)} candidate(s) in {len(files)} file(s)")
    counts: dict[str, int] = {}
    for finding in findings:
        counts[finding.reason] = counts.get(finding.reason, 0) + 1
    for reason, count in sorted(counts.items(), key=lambda item: (-item[1], item[0])):
        print(f"  {count:>3}  {reason}")
    return 1 if strict else 0


def excerpt(text: str, index: int) -> str:
    """Return a short window around a match, for the report."""
    start = max(0, index - 20)
    end = min(len(text), index + 21)
    return text[start:end].strip()


def _guess(text: str) -> str:
    return "zh" if CJK.search(text) else "en"


def _mask(text: str, include_code: bool) -> str:
    if not include_code:
        text = _mask_inline_code(text)
    return URL.sub(lambda match: " " * len(match.group(0)), text)


def _mask_inline_code(text: str) -> str:
    masked = list(text)
    index = 0
    while index < len(text):
        if text[index] != "`":
            index += 1
            continue
        end = index
        while end < len(text) and text[end] == "`":
            end += 1
        run = end - index
        close = text.find("`" * run, end)
        if close < 0:
            index = end
            continue
        for position in range(index, close + run):
            masked[position] = " "
        index = close + run
    return "".join(masked)


def _strip_comment(value: str) -> str:
    head = value.lstrip()
    if head.startswith('"'):
        index = 1
        while index < len(head):
            if head[index] == "\\":
                index += 2
                continue
            if head[index] == '"':
                return head[: index + 1]
            index += 1
        return head
    if head.startswith("'"):
        index = 1
        while index < len(head):
            if head[index] == "'":
                if index + 1 < len(head) and head[index + 1] == "'":
                    index += 2
                    continue
                return head[: index + 1]
            index += 1
        return head
    index = head.find(" #")
    return head[:index] if index >= 0 else head


def _display(path: Path) -> str:
    try:
        return path.resolve().relative_to(project_root()).as_posix()
    except ValueError:
        return str(path)
