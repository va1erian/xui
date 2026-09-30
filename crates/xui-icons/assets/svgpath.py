"""SVG geometry for generate.py: path data, rects and ellipses as absolute
("M"|"L"|"C"|"Z", ...) segments, with arcs turned into cubic Béziers.
Standard library only."""

import math
import re

NUMBER = re.compile(r"[-+]?(?:\d+\.?\d*|\.\d+)(?:[eE][-+]?\d+)?")
ARGS = {"M": 2, "L": 2, "H": 1, "V": 1, "C": 6, "S": 4, "A": 7, "Z": 0}


def arc_to_cubics(x1, y1, rx, ry, phi, large, sweep, x2, y2):
    """SVG arc endpoint -> centre parameterisation -> cubic segments (F.6.5)."""
    if rx == 0 or ry == 0 or (x1, y1) == (x2, y2):
        return [("L", x2, y2)]
    rx, ry = abs(rx), abs(ry)
    cp, sp = math.cos(math.radians(phi)), math.sin(math.radians(phi))
    dx, dy = (x1 - x2) / 2, (y1 - y2) / 2
    x1p, y1p = cp * dx + sp * dy, -sp * dx + cp * dy
    lam = x1p ** 2 / rx ** 2 + y1p ** 2 / ry ** 2
    if lam > 1:
        rx, ry = rx * math.sqrt(lam), ry * math.sqrt(lam)
    num = rx ** 2 * ry ** 2 - rx ** 2 * y1p ** 2 - ry ** 2 * x1p ** 2
    den = rx ** 2 * y1p ** 2 + ry ** 2 * x1p ** 2
    co = math.sqrt(max(0.0, num / den)) * (-1 if large == sweep else 1)
    cxp, cyp = co * rx * y1p / ry, -co * ry * x1p / rx
    cx = cp * cxp - sp * cyp + (x1 + x2) / 2
    cy = sp * cxp + cp * cyp + (y1 + y2) / 2

    def angle(ux, uy, vx, vy):
        return math.atan2(ux * vy - uy * vx, ux * vx + uy * vy)

    t1 = angle(1, 0, (x1p - cxp) / rx, (y1p - cyp) / ry)
    dt = angle((x1p - cxp) / rx, (y1p - cyp) / ry, (-x1p - cxp) / rx, (-y1p - cyp) / ry)
    if not sweep and dt > 0:
        dt -= 2 * math.pi
    elif sweep and dt < 0:
        dt += 2 * math.pi
    n = max(1, math.ceil(abs(dt) / (math.pi / 2) - 1e-9))
    step = dt / n
    k = 4 / 3 * math.tan(step / 4)
    out = []
    t = t1
    for _ in range(n):
        c1, s1, c2, s2 = math.cos(t), math.sin(t), math.cos(t + step), math.sin(t + step)
        pts = [(c1 - k * s1, s1 + k * c1), (c2 + k * s2, s2 - k * c2), (c2, s2)]
        vals = []
        for ex, ey in pts:
            ex, ey = ex * rx, ey * ry
            vals += [cp * ex - sp * ey + cx, sp * ex + cp * ey + cy]
        out.append(("C", *vals))
        t += step
    out[-1] = ("C", *out[-1][1:5], x2, y2)
    return out


def parse_path(d):
    """Path data -> absolute ("M"|"L"|"C"|"Z", ...) segments."""
    tokens = re.findall(r"[MmLlHhVvCcSsAaZz]|" + NUMBER.pattern, d)
    segs, i = [], 0
    cx = cy = sx = sy = 0.0
    last_c2 = None
    cmd = None
    while i < len(tokens):
        if tokens[i].isalpha():
            cmd = tokens[i]
            i += 1
            if cmd in "Zz":
                segs.append(("Z",))
                cx, cy = sx, sy
                last_c2 = None
                continue
        up, rel = cmd.upper(), cmd.islower()
        a = [float(t) for t in tokens[i:i + ARGS[up]]]
        i += ARGS[up]
        ox, oy = (cx, cy) if rel else (0.0, 0.0)
        if up == "M":
            cx, cy = a[0] + ox, a[1] + oy
            sx, sy = cx, cy
            segs.append(("M", cx, cy))
            cmd = "l" if rel else "L"
            last_c2 = None
        elif up == "L":
            cx, cy = a[0] + ox, a[1] + oy
            segs.append(("L", cx, cy))
            last_c2 = None
        elif up == "H":
            cx = a[0] + ox
            segs.append(("L", cx, cy))
            last_c2 = None
        elif up == "V":
            cy = a[0] + oy
            segs.append(("L", cx, cy))
            last_c2 = None
        elif up in "CS":
            if up == "C":
                x1, y1 = a[0] + ox, a[1] + oy
                x2, y2, x, y = a[2] + ox, a[3] + oy, a[4] + ox, a[5] + oy
            else:
                x1, y1 = (2 * cx - last_c2[0], 2 * cy - last_c2[1]) if last_c2 else (cx, cy)
                x2, y2, x, y = a[0] + ox, a[1] + oy, a[2] + ox, a[3] + oy
            segs.append(("C", x1, y1, x2, y2, x, y))
            last_c2 = (x2, y2)
            cx, cy = x, y
        elif up == "A":
            x, y = a[5] + ox, a[6] + oy
            segs += arc_to_cubics(cx, cy, a[0], a[1], a[2], int(a[3]), int(a[4]), x, y)
            cx, cy = x, y
            last_c2 = None
    return segs


def shape_path(el):
    """The path of a `rect`, `circle`, `ellipse` or `path` element."""
    tag = el.tag.split("}")[1]
    f = lambda k, default=0.0: float(el.get(k, default))
    if tag == "path":
        return parse_path(el.get("d"))
    if tag == "rect":
        x, y, w, h = f("x"), f("y"), f("width"), f("height")
        r = min(f("rx"), w / 2, h / 2)
        if r == 0:
            return parse_path(f"M{x} {y}H{x + w}V{y + h}H{x}Z")
        return parse_path(
            f"M{x + r} {y}H{x + w - r}A{r} {r} 0 0 1 {x + w} {y + r}V{y + h - r}"
            f"A{r} {r} 0 0 1 {x + w - r} {y + h}H{x + r}A{r} {r} 0 0 1 {x} {y + h - r}"
            f"V{y + r}A{r} {r} 0 0 1 {x + r} {y}Z")
    if tag in ("circle", "ellipse"):
        cx, cy = f("cx"), f("cy")
        rx, ry = (f("r"), f("r")) if tag == "circle" else (f("rx"), f("ry"))
        return parse_path(
            f"M{cx - rx} {cy}A{rx} {ry} 0 0 1 {cx + rx} {cy}A{rx} {ry} 0 0 1 {cx - rx} {cy}Z")
    raise ValueError(f"unsupported element <{tag}>")
