# xui-icons

A multi-colour vector icon set for [xui](../xui-core): 36 icons on a 32×32
grid in two looks, chosen at compile time so only one is built in.

| Style | Build | Look |
|---|---|---|
| Global Village | default | 90s world-music: flat colour, chunky ink outlines, woven zig-zag trim. Recolourable with a `Palette`. |
| Aero | `--features aero` | Vista-era glass: glossy gradients, white specular highlights, soft shadows. |

`xui_icons::STYLE` says which one this build has. Enable the crate from the
umbrella with the `icons` feature (`icons-aero` for the glossy set), or depend
on `xui-icons` directly.

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

`draw` fills the smaller side of `rect`, centred, through `Canvas::fill_path`,
`Canvas::fill_path_linear` and `Canvas::stroke_path`. That is the portable path
API: `xui-canvas` rasterises it with `tiny-skia`. The Aero style needs
`fill_path_linear` for its gradients; a backend that does not override it fills
those shapes with their middle colour (`xui-canvas` overrides it).

## Colour and dark mode

Global Village shapes name a `Tone` (ink, cream, teal, rose, amber, …) and a
`Palette` maps tones to colours. `Palette::GLOBAL_VILLAGE` is the default. The
dark ink outline suits mid and light surfaces; on a very dark one, retint it:

```rust
use xui_core::backend::Rgba;
use xui_icons::{Palette, Tone};

let on_black = Palette::GLOBAL_VILLAGE.with(Tone::Ink, Rgba::rgb(0x3A, 0x31, 0x90));
```

Aero uses fixed colours and gradients (both suit dark and light surfaces), so
the palette does not reach it.

## Gallery

```sh
cargo run -p xui-icons --example gallery                  # Global Village
cargo run -p xui-icons --example gallery --features aero  # Aero
```

renders `icons-<style>-dark.png` and `icons-<style>-light.png` into
`target/snapshots/` headlessly.

## Assets

```text
assets/<style>/svg/         source of truth: one standalone 32×32 SVG per icon
assets/<style>/png/<size>/  alpha PNG exports at 16, 24, 32, 48, 64, 128 and 256 px
assets/<style>/ico/         Windows .ico per icon (16, 24, 32, 48, 64 and 256 px)
assets/generate.py          compiles assets/*/svg into src/village and src/aero
assets/svgpath.py           its SVG path and shape geometry (standard library only)
```

Edit an SVG, then run `python crates/xui-icons/assets/generate.py` and commit
the regenerated `src/village/` and `src/aero/`. Supported SVG: `rect`, `circle`,
`ellipse` and `path` (`M L H V C S A Z`, absolute or relative); Global Village
files are styled with the classes in their `<style>`, Aero files with literal
colours, `linearGradient`s in `<defs>`, per-element `opacity` and the CSS rules
in their `<style>`. The Aero drop-shadow filter is not read: the crate draws a
one-unit soft shadow under opaque shapes itself. The PNG and `.ico` files are
exports and are not read by the build.
