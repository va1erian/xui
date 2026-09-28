# Vendored Lucide icons

The SVGs in this directory are copied unmodified from
[Lucide](https://lucide.dev) (`lucide-static` 1.48.0), which is licensed under
the ISC License; see [LICENSE](LICENSE). Copyright (c) Lucide Icons and
Contributors.

`generate.py` converts them into `../../src/widget/lucide/data.rs` (absolute move/line/cubic
segments on the 24x24 design grid) so they can be drawn with the portable
`Canvas::fill_path`/`stroke_path`. To add an icon, drop its SVG here, re-run the
script and reference the new constant from `src/widget/lucide/mod.rs`.
