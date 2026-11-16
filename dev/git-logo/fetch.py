#!/usr/bin/env python3
"""Fetches the Git logomark and draws the picture the icon library holds.

The SVG is what the address serves, committed beside this script, and the PNG is what
`rorolala-dev-icons --render` makes of it, so what travels with the plugin is drawn from the same
artwork a person can look up. Run it from anywhere:

    python dev/git-logo/fetch.py

`curl` does the fetching rather than Python's own client: the certificate store a Python build
ships with is not the machine's, and what is fetched here is a file from a public site rather than
a secret.
"""

from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[2]
VENDOR = ROOT / "app/desktop/Plugins/GitVCSPlugin/vendor/git"
URL = "https://git-scm.com/images/logos/downloads/Git-Icon-White.svg"
SVG = VENDOR / "Git-Icon-White.svg"
PNG = VENDOR / "Git-Icon-White.png"
SIZE = "64"


def main() -> None:
    VENDOR.mkdir(parents=True, exist_ok=True)

    subprocess.run(["curl", "-fsSL", URL, "-o", str(SVG)], check=True)
    print(f"==> {SVG}")

    subprocess.run(
        [
            "cargo",
            "run",
            "--quiet",
            "-p",
            "rorolala-dev-icons",
            "--",
            "--render",
            str(SVG),
            str(PNG),
            SIZE,
        ],
        cwd=ROOT,
        check=True,
    )


if __name__ == "__main__":
    main()
