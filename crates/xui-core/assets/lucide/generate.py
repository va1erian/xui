"""Converts the vendored Lucide SVGs into the generated icon modules.

Every shape becomes a list of absolute move/line/cubic/close segments on the
24x24 design grid (arcs and quadratics are converted to cubics), which the
canvas draws natively.

The generator writes `src/widget/lucide/data.rs` (the public `Lucide` enum and
the `Lucide::ALL` list), `paths.rs` (the variant-to-path mapping) and one
`data_<letter>.rs` per first letter, so no generated file crosses the line
limit. Adding an icon is dropping its SVG here and re-running the script.

Usage: pip install svgelements; python generate.py
"""
import glob
import os
import re

from svgelements import (Arc, Circle, Close, CubicBezier, Line, Move,
                         Path, QuadraticBezier, Rect)

HERE = os.path.dirname(os.path.abspath(__file__))
OUT_DIR = os.path.join(HERE, '..', '..', 'src', 'widget', 'lucide')

HEADER = [
    "// Generated from the vendored Lucide SVGs in `assets/lucide/` (see",
    "// `assets/lucide/README.md`) by `assets/lucide/generate.py`. Do not edit.",
]


def fmt(v):
    text = ('%.3f' % v).rstrip('0')
    return text + '0' if text.endswith('.') else text


def cubic(seg):
    a, b, c, d = seg.start, seg.control1, seg.control2, seg.end
    return f"CubicTo({fmt(b.x)}, {fmt(b.y)}, {fmt(c.x)}, {fmt(c.y)}, {fmt(d.x)}, {fmt(d.y)})"


def segments(path):
    out = []
    for seg in path:
        if isinstance(seg, Move):
            out.append(f"MoveTo({fmt(seg.end.x)}, {fmt(seg.end.y)})")
        elif isinstance(seg, Close):
            out.append("Close")
        elif isinstance(seg, Line):
            out.append(f"LineTo({fmt(seg.end.x)}, {fmt(seg.end.y)})")
        elif isinstance(seg, CubicBezier):
            out.append(cubic(seg))
        elif isinstance(seg, QuadraticBezier):
            p0, q, p1 = seg.start, seg.control, seg.end
            c1 = (p0.x + 2 / 3 * (q.x - p0.x), p0.y + 2 / 3 * (q.y - p0.y))
            c2 = (p1.x + 2 / 3 * (q.x - p1.x), p1.y + 2 / 3 * (q.y - p1.y))
            out.append(f"CubicTo({fmt(c1[0])}, {fmt(c1[1])}, {fmt(c2[0])}, {fmt(c2[1])}, {fmt(p1.x)}, {fmt(p1.y)})")
        elif isinstance(seg, Arc):
            out.extend(cubic(c) for c in seg.as_cubic_curves())
        else:
            raise ValueError(type(seg))
    return out


def shape_path(tag, a):
    if tag == 'path':
        return Path(a['d'])
    if tag == 'circle':
        return Path(Circle(float(a['cx']), float(a['cy']), float(a['r'])))
    if tag == 'rect':
        rx = float(a.get('rx', 0))
        return Path(Rect(float(a.get('x', 0)), float(a.get('y', 0)),
                         float(a['width']), float(a['height']),
                         rx, float(a.get('ry', rx))))
    if tag in ('polyline', 'polygon'):
        points = [float(v) for v in re.findall(r'-?\d+(?:\.\d+)?', a['points'])]
        pairs = list(zip(points[0::2], points[1::2]))
        d = ''.join(f"{'M' if i == 0 else 'L'}{x} {y}" for i, (x, y) in enumerate(pairs))
        return Path(d + ('Z' if tag == 'polygon' else ''))
    return Path(f"M{a['x1']} {a['y1']}L{a['x2']} {a['y2']}")


def const_name(stem):
    return stem.upper().replace('-', '_').replace(' ', '_')


def variant_name(stem):
    parts = re.split(r'[-_ ]+', stem)
    return ''.join(part[:1].upper() + part[1:] for part in parts if part)


def load(stem, path):
    """The path segments of one vendored SVG."""
    text = open(path).read()
    body = text[text.index('>', text.index('<svg')) + 1:]
    segs = []
    for tag, attrs in re.findall(r'<(path|circle|rect|line|polyline|polygon)\s([^>]*?)/?>', body):
        segs += segments(shape_path(tag, dict(re.findall(r'([\w-]+)="([^"]*)"', attrs))))
    return segs


def write(path, lines):
    with open(path, 'w') as handle:
        handle.write("\n".join(lines) + "\n")


# Every vendored icon, in file-name order.
icons = []
for f in sorted(glob.glob(os.path.join(HERE, '*.svg'))):
    stem = os.path.basename(f)[:-4]
    icons.append((stem, const_name(stem), variant_name(stem), load(stem, f)))

# Split the path constants by first letter so each generated file stays small.
buckets = {}
for stem, const, variant, segs in icons:
    buckets.setdefault(const[:1], []).append((const, segs))

for letter, entries in sorted(buckets.items()):
    used = {seg.split('(')[0] for _, segs in entries for seg in segs}
    variants = ", ".join(kind for kind in ("Close", "CubicTo", "LineTo", "MoveTo") if kind in used)
    lines = list(HEADER) + [
        "",
        "#![allow(clippy::approx_constant)] // generated coordinates, not maths",
        "",
        "use crate::backend::PathSeg;",
    ]
    if variants:
        lines.append(f"use PathSeg::{{{variants}}};")
    lines.append("")
    for const, segs in entries:
        lines.append(f"pub(crate) const {const}: &[PathSeg] = &[")
        row = "   "
        for seg in segs:
            if len(row) + len(seg) + 2 > 96:
                lines.append(row)
                row = "   "
            row += f" {seg},"
        lines.append(row)
        lines.append("];")
    write(os.path.join(OUT_DIR, f"data_{letter.lower()}.rs"), lines)

# The top-level generated module: the enum and ALL.
lines = list(HEADER) + [
    "",
    "use crate::backend::PathSeg;",
    "",
]
for letter in sorted(buckets):
    lines += [
        '#[path = "data_%s.rs"]' % letter.lower(),
        "mod data_%s;" % letter.lower(),
        "pub(crate) use data_%s::*;" % letter.lower(),
        "",
    ]
lines += [
    "/// One of the vendored [Lucide](https://lucide.dev) outline icons.",
    "///",
    "/// Adding an icon is dropping its SVG into `assets/lucide/` and re-running",
    "/// `assets/lucide/generate.py`; the variant is generated from the file name.",
    "#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]",
    "pub enum Lucide {",
]
for stem, const, variant, segs in icons:
    lines.append(f"    /// The `{stem}` outline.")
    lines.append(f"    {variant},")
lines += [
    "}",
    "",
    "impl Lucide {",
    "    /// Every vendored icon, in file-name order.",
    "    pub const ALL: &'static [Lucide] = &[",
]
for stem, const, variant, segs in icons:
    lines.append(f"        Lucide::{variant},")
lines += [
    "    ];",
    "}",
    "",
    '#[path = "paths.rs"]',
    "mod paths;",
    "pub(crate) use paths::path;",
]
write(os.path.join(OUT_DIR, 'data.rs'), lines)

# The variant-to-path mapping, in its own file so `data.rs` stays small.
lines = list(HEADER) + [
    "",
    "use super::*;",
    "",
    "/// The path segments that draw `icon` on the 24x24 Lucide grid.",
    "pub(crate) fn path(icon: Lucide) -> &'static [PathSeg] {",
    "    match icon {",
]
for stem, const, variant, segs in icons:
    lines.append(f"        Lucide::{variant} => {const},")
lines += [
    "    }",
    "}",
]
write(os.path.join(OUT_DIR, 'paths.rs'), lines)
