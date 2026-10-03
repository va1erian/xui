# Vendored Lucide icons

The SVGs in this directory are copied unmodified from
[Lucide](https://lucide.dev) (`lucide-static` 1.48.0), which is licensed under
the ISC License; see [LICENSE](LICENSE). Copyright (c) Lucide Icons and
Contributors.

`generate.py` converts them into the generated modules under
`../../src/widget/lucide/`: `data.rs` holds the public `Lucide` enum and the
`Lucide::ALL` list, `paths.rs` the variant-to-path mapping, and one `data_<letter>.rs`
per first letter holds the path constants (absolute move/line/cubic segments on
the 24x24 design grid). The paths are drawn with the portable
`Canvas::fill_path`/`stroke_path` through `xui_core::icon::draw_icon`.

To add an icon, drop its SVG here and re-run:

```text
pip install svgelements
python generate.py
```

The new file name becomes the `Lucide` variant (`folder-open.svg` →
`Lucide::FolderOpen`).
