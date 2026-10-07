"""Reference renders of the comparison fixtures by a real browser (Edge or
Chrome, headless), offline so that, like the probes, it loads nothing from the
network.

Usage: python reference.py [out-dir]   (default target/engine-compare)

Writes shots/<fixture>.browser.png next to the probes' shots.
"""

import os
import shutil
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
WIDTH, HEIGHT = int(os.environ.get("WIDTH", 1000)), int(os.environ.get("HEIGHT", 1400))

# Chrome first: headless Edge exits without a screenshot on some machines.
CANDIDATES = [
    r"C:\Program Files\Google\Chrome\Application\chrome.exe",
    r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
    "chromium",
    "google-chrome",
]


def browser() -> str:
    for c in CANDIDATES:
        if Path(c).exists() or shutil.which(c):
            return c
    sys.exit("reference.py: no Edge or Chrome found")


def pages(out: Path):
    for f in sorted((HERE / "fixtures").glob("*.html")):
        yield f.stem, f
    yield "theoldnet", HERE / "fixtures" / "theoldnet" / "index.html"
    for f in sorted((out / "live").glob("*.html")):
        yield f.stem, f


def main() -> None:
    out = Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT / "target" / "engine-compare"
    (out / "shots").mkdir(parents=True, exist_ok=True)
    exe = browser()
    profile = out / "browser-profile"
    for name, page in pages(out):
        shot = out / "shots" / f"{name}.browser.png"
        subprocess.run(
            [
                exe,
                "--headless=new",
                "--disable-gpu",
                "--hide-scrollbars",
                "--force-device-scale-factor=1",
                f"--user-data-dir={profile}",
                # No network: like the probes, only file: loads.
                "--host-resolver-rules=MAP * ~NOTFOUND",
                f"--window-size={WIDTH},{HEIGHT}",
                f"--screenshot={shot}",
                page.as_uri(),
            ],
            check=False,
            capture_output=True,
            timeout=120,
        )
        print(f"{name}: {'ok' if shot.exists() else 'FAILED'}")


if __name__ == "__main__":
    main()
