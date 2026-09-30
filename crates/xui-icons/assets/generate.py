#!/usr/bin/env python3
"""Regenerates `src/village/` and `src/aero/` from the vendored SVGs.

Run from anywhere: `python crates/xui-icons/assets/generate.py`.

Each SVG under `assets/<style>/svg/` is a flat list of `rect`/`circle`/`ellipse`/
`path` elements. This script resolves every element to one filled and/or
stroked path of `PathSeg`s (rects, circles and SVG arcs become cubic Béziers)
and maps its paints to Rust, so the crate needs no SVG parser at run time:

- `village`: paints are `Tone` roles chosen by the file's CSS classes.
- `aero`: paints are literal colours, `linearGradient`s from the file's
  `<defs>` (emitted once as `src/aero/gradients.rs`) and per-element opacity;
  the drop shadow filter is applied by the crate at draw time.

Standard library only; SVG geometry lives in `svgpath.py`.
"""

import re
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from svgpath import shape_path  # noqa: E402

HERE = Path(__file__).resolve().parent
SRC = HERE.parent / "src"

# Category -> icons, in gallery order.
CATEGORIES = {
    "hardware": ["computer", "monitor", "keyboard", "mouse", "hard-disk", "floppy",
                 "cd-rom", "printer", "modem", "network", "server", "speaker"],
    "files": ["folder", "folder-open", "document", "image", "music", "archive"],
    "system": ["trash", "trash-full", "settings", "search", "home", "clock", "mail",
               "lock", "help", "warning", "info", "error"],
    "toolkit": ["rpc", "pub-sub", "theme", "widget", "terminal", "window"],
}

FILL_CLASSES = {"c": "Cream", "t": "Teal", "m": "Rose", "a": "Amber", "b": "Cobalt",
                "r": "Clay", "g": "Lavender", "k": "Ink", "s": "Screen", "l": "Mint"}
STROKE_CLASSES = {"o": ("Ink", 1.5), "h": ("Shine", 1.2), "h2": ("Ink", 1.0),
                  "zz": ("Rose", 1.6), "zt": ("Teal", 1.6), "zc": ("Cream", 1.6)}
HEX_TONES = {"#0b0822": "Ink", "#f1e3bd": "Cream", "#19b5a5": "Teal", "#e2366f": "Rose",
             "#f4a81d": "Amber", "#4162e0": "Cobalt", "#d4562f": "Clay",
             "#9a93d0": "Lavender", "#1b1552": "Screen", "#9fe6dd": "Mint",
             "#221a66": "Night", "#fff6dc": "Shine", "#fff2d0": "Chalk"}
INHERITED = ("fill", "stroke", "stroke-width", "fill-opacity", "stroke-opacity")


def local(el):
    return el.tag.split("}")[1]


# ----------------------------------------------------------------- village

def village_tone(value):
    return None if value in (None, "none") else f"Paint::Tone(Tone::{HEX_TONES[value.lower()]})"


def village_style(el, props):
    classes = set(props.get("class", "").split())
    fill = props.get("fill")
    fill = village_tone(fill) if fill else None
    for c in classes:
        if c in FILL_CLASSES:
            fill = f"Paint::Tone(Tone::{FILL_CLASSES[c]})"
    if "n" in classes:
        fill = None
    stroke = None
    for c in classes:
        if c in STROKE_CLASSES:
            tone, width = STROKE_CLASSES[c]
            stroke = (f"Paint::Tone(Tone::{tone})", width)
    if props.get("stroke") not in (None, "none"):
        width = float(props.get("stroke-width", stroke[1] if stroke else 1.0))
        stroke = (village_tone(props["stroke"]), width)
    return fill, stroke, 255


# -------------------------------------------------------------------- aero

def parse_css(root):
    """`.name{prop:value;...}` rules of the file's <style> -> {name: {prop: value}}."""
    rules = {}
    for el in root.iter():
        if local(el) == "style":
            for name, body in re.findall(r"\.([\w-]+)\s*\{([^}]*)\}", el.text or ""):
                rules[name] = dict(p.split(":") for p in body.split(";") if ":" in p)
    return rules


def aero_paint(value, opacity):
    if value in (None, "none"):
        return None
    if value.startswith("url(#"):
        return f"Paint::Gradient(&gradients::{value[5:-1].lstrip('v').upper()})"
    value = value.lstrip("#")
    if len(value) == 3:
        value = "".join(c * 2 for c in value)
    r, g, b = (int(value[i:i + 2], 16) for i in (0, 2, 4))
    a = round(float(opacity) * 255)
    return f"Paint::Color(Rgba::rgb({r}, {g}, {b}))" if a == 255 else \
        f"Paint::Color(Rgba::with_alpha({r}, {g}, {b}, {a}))"


def aero_style(el, props, css):
    # CSS rules beat presentation attributes, as in a browser.
    merged = dict(props)
    for c in props.get("class", "").split():
        merged.update(css.get(c, {}))
    fill = aero_paint(merged.get("fill"), merged.get("fill-opacity", 1))
    stroke = aero_paint(merged.get("stroke"), merged.get("stroke-opacity", 1))
    width = float(merged.get("stroke-width", 1.0))
    opacity = round(float(el.get("opacity", 1)) * 255)
    return fill, (stroke, width) if stroke else None, opacity


def aero_gradients(root):
    """The `<defs>` linear gradients -> Rust statics, and their source text."""
    out = []
    for el in root.iter():
        if local(el) != "linearGradient":
            continue
        stops = []
        for stop in el:
            r, g, b = (int(stop.get("stop-color")[i:i + 2], 16) for i in (1, 3, 5))
            a = round(float(stop.get("stop-opacity", 1)) * 255)
            stops.append(f"GradientStop::new({num(float(stop.get('offset')))}, "
                         f"Rgba::with_alpha({r}, {g}, {b}, {a}))")
        p = [num(float(el.get(k, d))) for k, d in (("x1", 0), ("y1", 0), ("x2", 0), ("y2", 1))]
        name = el.get("id").lstrip("v").upper()
        out.append(f"pub(super) static {name}: Gradient = Gradient::new(({p[0]}, {p[1]}), "
                   f"({p[2]}, {p[3]}), &[{', '.join(stops)}]);\n")
    return out


# ----------------------------------------------------------------- shared

def collect(node, resolve, props=None):
    props = props or {}
    for el in node:
        tag = local(el)
        if tag in ("style", "defs"):
            continue
        inherited = dict(props)
        inherited.update({k: el.get(k) for k in INHERITED if el.get(k) is not None})
        # A group's classes apply to its children, then the child's own.
        inherited["class"] = f"{props.get('class', '')} {el.get('class') or ''}".strip()
        if tag == "g":
            yield from collect(el, resolve, inherited)
            continue
        fill, stroke, opacity = resolve(el, inherited)
        yield shape_path(el), fill, stroke, opacity


def num(v):
    s = f"{v:.2f}".rstrip("0").rstrip(".")
    if s in ("-0", ""):
        s = "0"
    return s if "." in s else s + ".0"


def rust_path(segs):
    parts = []
    for s in segs:
        if s[0] == "Z":
            parts.append("PathSeg::Close")
        else:
            name = {"M": "MoveTo", "L": "LineTo", "C": "CubicTo"}[s[0]]
            parts.append(f"PathSeg::{name}({', '.join(num(v) for v in s[1:])})")
    return "&[" + ", ".join(parts) + "]"


def variant(name):
    return "".join(p.capitalize() for p in name.split("-"))


def const_name(name):
    return name.upper().replace("-", "_")


def emit_shape(path, fill, stroke, opacity):
    p = rust_path(path)
    if fill and stroke:
        shape = f"Shape::both({p}, {fill}, {stroke[0]}, {num(stroke[1])})"
    elif fill:
        shape = f"Shape::fill({p}, {fill})"
    elif stroke:
        shape = f"Shape::line({p}, {stroke[0]}, {num(stroke[1])})"
    else:
        raise ValueError("shape with neither fill nor stroke")
    return shape + (f".opacity({opacity})" if opacity < 255 else "")


HEADER = ("// @generated by crates/xui-icons/assets/generate.py from assets/{style}/svg/*.svg.\n"
          "// Do not edit by hand.\n\n")
IMPORTS = {
    "village": "use xui_core::backend::PathSeg;\n\nuse crate::shape::{Paint, Shape};\n"
               "use crate::tone::Tone;\n\n",
    "aero": "#[allow(unused_imports)] // not every category draws a translucent colour\n"
            "use xui_core::backend::{PathSeg, Rgba};\n\nuse super::gradients;\n"
            "use crate::shape::{Paint, Shape};\n\n",
}


def generate(style):
    svgs = HERE / style / "svg"
    out_dir = SRC / style
    out_dir.mkdir(parents=True, exist_ok=True)
    header = HEADER.format(style=style)
    gradient_text = None
    for category, names in CATEGORIES.items():
        out = [header, IMPORTS[style]]
        for name in names:
            root = ET.parse(svgs / f"{name}.svg").getroot()
            if style == "aero":
                css = parse_css(root)
                resolve = lambda el, props, css=css: aero_style(el, props, css)
                text = "".join(aero_gradients(root))
                assert gradient_text in (None, text), f"{name}: <defs> differ from the others"
                gradient_text = text
            else:
                resolve = lambda el, props: village_style(el, props)
            out.append(f"pub(super) const {const_name(name)}: &[Shape] = &[\n")
            for shape in collect(root, resolve):
                out.append(f"    {emit_shape(*shape)},\n")
            out.append("];\n\n")
        (out_dir / f"{category}.rs").write_text("".join(out).rstrip("\n") + "\n", encoding="utf-8")

    mods = sorted(CATEGORIES) + (["gradients"] if style == "aero" else [])
    mod = [header]
    for m in sorted(mods):
        mod.append("#[rustfmt::skip]\n#[allow(clippy::approx_constant)] // generated coordinates, "
                   f"not maths\nmod {m};\n")
    mod.append("\nuse crate::icon::Icon;\nuse crate::shape::Shape;\n\n"
               "/// The shapes of `icon` in this style.\n"
               "pub(crate) fn shapes(icon: Icon) -> &'static [Shape] {\n    match icon {\n")
    for category, names in CATEGORIES.items():
        for name in names:
            mod.append(f"        Icon::{variant(name)} => {category}::{const_name(name)},\n")
    mod.append("    }\n}\n")
    (out_dir / "mod.rs").write_text("".join(mod), encoding="utf-8")

    if style == "aero":
        (out_dir / "gradients.rs").write_text(
            header + "use xui_core::backend::{GradientStop, Rgba};\n\n"
            "use crate::gradient::Gradient;\n\n" + gradient_text, encoding="utf-8")


if __name__ == "__main__":
    for style in ("village", "aero"):
        generate(style)
