"""Converts the vendored Lucide SVGs into src/widget/lucide/data.rs.

Every shape becomes a list of absolute move/line/cubic/close segments on the
24x24 design grid (arcs and quadratics are converted to cubics), which the
canvas draws natively.

Usage: pip install svgelements; python generate.py
"""
import glob
import os
import re

from svgelements import (Arc, Circle, Close, CubicBezier, Line, Move,
                         Path, QuadraticBezier, Rect)

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, '..', '..', 'src', 'widget', 'lucide', 'data.rs')


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
    return Path(f"M{a['x1']} {a['y1']}L{a['x2']} {a['y2']}")


lines = [
    "// Generated from the vendored Lucide SVGs in `assets/lucide/` (see",
    "// `assets/lucide/README.md`) by `assets/lucide/generate.py`. Do not edit.",
    "",
    "use crate::backend::PathSeg;",
    "use PathSeg::{Close, CubicTo, LineTo, MoveTo};",
    "",
]
for f in sorted(glob.glob(os.path.join(HERE, '*.svg'))):
    name = os.path.basename(f)[:-4].upper().replace('-', '_')
    text = open(f).read()
    body = text[text.index('>', text.index('<svg')) + 1:]
    segs = []
    for tag, attrs in re.findall(r'<(path|circle|rect|line)\s([^>]*?)/?>', body):
        segs += segments(shape_path(tag, dict(re.findall(r'([\w-]+)="([^"]*)"', attrs))))
    lines.append(f"pub(crate) const {name}: &[PathSeg] = &[")
    row = "   "
    for seg in segs:
        if len(row) + len(seg) + 2 > 96:
            lines.append(row)
            row = "   "
        row += f" {seg},"
    lines.append(row)
    lines.append("];")
open(OUT, 'w').write("\n".join(lines) + "\n")
