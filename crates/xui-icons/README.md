# xui-icons

A multi-colour vector icon set for [xui](../xui-core): 36 icons on a 32×32
grid in the "Global Village" style: 90s world-music flat colour, chunky ink
outlines and woven zig-zag trim, recolourable with a `Palette`. (The glossy
"Aero" style lives on the `attic` branch.) Enable the crate from the umbrella
with the `icons` feature, or depend on `xui-icons` directly.

| Category | Icons |
|---|---|
| Hardware | computer, monitor, keyboard, mouse, hard-disk, floppy, cd-rom, printer, modem, network, server, speaker |
| Files | folder, folder-open, document, image, music, archive |
| System | trash, trash-full, settings, search, home, clock, mail, lock, help, warning, info, error |
| Toolkit | rpc, pub-sub, theme, widget, terminal, window |

The crate depends on `xui-core` only and has no `unsafe`.

```rust
use xui_icons::{Icon, Palette, draw};

// In a custom painter, with a `&mut dyn Canvas` and a `Rect`:
draw(canvas, Icon::Network, rect, &Palette::GLOBAL_VILLAGE);
```

`draw` fills the smaller side of `rect`, centred, through `Canvas::fill_path`
and `Canvas::stroke_path`. That is the portable path API: `xui-canvas`
rasterises it with `tiny-skia`.

## Colour and dark mode

Shapes name a `Tone` (ink, cream, teal, rose, amber, …) and a
`Palette` maps tones to colours. `Palette::GLOBAL_VILLAGE` is the default. The
dark ink outline suits mid and light surfaces; on a very dark one, retint it:

```rust
use xui_core::backend::Rgba;
use xui_icons::{Palette, Tone};

let on_black = Palette::GLOBAL_VILLAGE.with(Tone::Ink, Rgba::rgb(0x3A, 0x31, 0x90));
```

## Gallery

```sh
cargo run -p xui-icons --example gallery
```

renders `icons-village-dark.png` and `icons-village-light.png` into
`target/snapshots/` headlessly.

## Assets

```text
assets/village/svg/         source of truth: one standalone 32×32 SVG per icon
assets/village/png/<size>/  alpha PNG exports at 16, 24, 32, 48, 64, 128 and 256 px
assets/village/ico/         Windows .ico per icon (16, 24, 32, 48, 64 and 256 px)
assets/generate.py            compiles assets/village/svg into src/village
assets/svgpath.py             its SVG path and shape geometry (standard library only)
```

Edit an SVG, then run `python crates/xui-icons/assets/generate.py` and commit
the regenerated `src/village/`. Supported SVG: `rect`, `circle`,
`ellipse` and `path` (`M L H V C S A Z`, absolute or relative); files are
styled with the classes in their `<style>`. The PNG and `.ico` files are
exports and are not read by the build.
