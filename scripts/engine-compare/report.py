"""Builds the comparison from run.sh's and reference.py's output: a side-by-side
PNG per fixture (browser | Blitz | NetSurf | litehtml), a similarity score
against the browser and a summary in JSON for the report page.

Usage: python report.py [out-dir]   (default target/engine-compare)
Needs Pillow and NumPy.

The score is how alike two renders look at a glance, not a pass/fail: both
images are scaled to 1/4, blurred, and compared pixel by pixel; a pixel
"matches" when every channel is within 24 levels. Fonts differ between the
browser (Windows) and the probes (Linux), so text never matches exactly; what
the score catches is layout, colour and missing content.
"""

import csv
import json
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw, ImageFilter, ImageFont

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
COLUMNS = ["browser", "blitz", "netsurf", "litehtml"]
LABELS = {"browser": "Chrome (reference)", "blitz": "Blitz", "netsurf": "NetSurf", "litehtml": "litehtml"}


def load(path: Path, size):
    if not path.exists():
        return None
    image = Image.open(path).convert("RGB")
    if size and image.size != size:
        canvas = Image.new("RGB", size, "white")
        canvas.paste(image.crop((0, 0, *size)), (0, 0))
        image = canvas
    return image


def similarity(a: Image.Image, b: Image.Image) -> float:
    small = (a.width // 4, a.height // 4)
    blur = ImageFilter.GaussianBlur(1)
    x = np.asarray(a.resize(small).filter(blur), dtype=np.int16)
    y = np.asarray(b.resize(small).filter(blur), dtype=np.int16)
    close = (np.abs(x - y) <= 24).all(axis=2)
    return float(close.mean())


#: The size of a column when nothing rendered at all.
PLACEHOLDER = (1000, 1400)


def composite(images, out: Path):
    scale = 0.5
    sizes = [i.size for i in images.values() if i] or [PLACEHOLDER]
    w = max(width for width, _ in sizes) * scale
    h = max(height for _, height in sizes) * scale
    w, h, head, gap = int(w), int(h), 28, 8
    sheet = Image.new("RGB", (len(COLUMNS) * (w + gap) - gap, h + head), (230, 232, 236))
    draw = ImageDraw.Draw(sheet)
    try:
        font = ImageFont.truetype("arial.ttf", 18)
    except OSError:
        font = ImageFont.load_default()
    for i, col in enumerate(COLUMNS):
        x = i * (w + gap)
        draw.text((x + 6, 4), LABELS[col], fill=(20, 20, 20), font=font)
        image = images.get(col)
        if image is None:
            draw.text((x + 6, head + 10), "(no render)", fill=(160, 0, 0), font=font)
            continue
        sheet.paste(image.resize((int(image.width * scale), int(image.height * scale))), (x, head))
    sheet.save(out, optimize=True)


def main() -> None:
    out = Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT / "target" / "engine-compare"
    shots = out / "shots"
    sheets = out / "sheets"
    sheets.mkdir(exist_ok=True)

    timings = {}
    with open(out / "timings.csv", newline="") as f:
        for row in csv.DictReader(f):
            timings[(row["fixture"], row["engine"])] = row
    sizes = {}
    with open(out / "sizes.csv", newline="") as f:
        for row in csv.DictReader(f):
            sizes.setdefault(row["profile"], {})[row["engine"]] = int(row["bytes"])

    fixtures = sorted({name for name, _ in timings})
    summary = {"fixtures": [], "sizes": sizes}
    for name in fixtures:
        ref = load(shots / f"{name}.browser.png", None)
        size = ref.size if ref else None
        images = {"browser": ref}
        for engine in COLUMNS[1:]:
            images[engine] = load(shots / f"{name}.{engine}.png", size)
        scores = {
            e: round(similarity(ref, images[e]), 3) if ref and images[e] else None
            for e in COLUMNS[1:]
        }
        composite(images, sheets / f"{name}.png")
        summary["fixtures"].append(
            {
                "name": name,
                "similarity": scores,
                "ms": {e: int(timings[(name, e)]["ms"]) for e in COLUMNS[1:] if (name, e) in timings},
                "ready": {e: timings[(name, e)]["ready"] for e in COLUMNS[1:] if (name, e) in timings},
            }
        )
        print(name, scores)
    (out / "summary.json").write_text(json.dumps(summary, indent=2))
    print(json.dumps(sizes, indent=2))


if __name__ == "__main__":
    main()
