---
name: xui-app
description: Start or extend a Rust GUI app on xui (this repo): a window with xui::app, layouts instead of rects, typed handles and headless verification. Use when asked to write, scaffold or change an xui app, a dialog, a settings pane or a widget demo, or to check one without opening a window.
---

# Writing an xui app

The shape every app has (see `crates/xui/examples/controls/label.rs`):

```rust
use xui::prelude::*;

#[derive(Clone)]
enum Msg { Save }

#[derive(Default)]
struct App { name: Handle<Edit<Msg>>, status: Handle<Label<Msg>> }

impl xui::App for App {
    type Msg = Msg;
    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Save => {
                let name = self.name.get().text();
                self.status.get().set_text(&format!("Saved {name}"));
            }
        }
    }
}

fn main() -> Result<()> {
    xui::app("Demo").size(360, 200).run(|ui| {
        let app = App::default();
        ui.root(
            column().padding(16).gap(8).children((
                label("Name").caption(),
                edit().bind(&app.name).fill(1),
                row().justify(Align::End).child(
                    button("Save").on_click(Msg::Save).primary(),
                ),
                label("").bind(&app.status),
            )),
        )?;
        Ok(app)
    })
}
```

## Rules

- **Describe, don't place.** Builders (`label`, `button`, `edit`, `list`, …)
  become widgets when `ui.root`/`ui.mount` mounts the layout. No `Rect`, no
  `ui` argument, no per-widget `unwrap`; the first constructor error surfaces
  from `root`/`mount`.
- **Bind what you change.** A field `Handle<W>`, `.bind(&handle)` on the
  builder, `handle.get()` in `update`. Everything else needs no field.
- **Map events by value.** `Msg: Clone`; `on_click(Msg::Save)`,
  `on_toggle(Msg::On)`, `on_select(Msg::Pick)`, and for events that carry a
  value pass the enum constructor: `on_change(Msg::Name)` where
  `Msg::Name(String)`. The closure form (`on_click_with(|| ...)`) stays
  available.
- **Size only what must not be natural.** `fill(weight)` takes the leftover
  space, `fixed(n)`/`width(n)`/`height(n)` pin a size, `max_width(n)`/
  `max_height(n)` cap one, `aspect(16.0 / 9.0)` keeps a width-to-height ratio.
  Numbers are design units (`Dip`), scaled by DPI.
- **Containers:** `row()`, `column()`, `grid([Track::Auto, Track::Fill(1)])`
  (with `.span(n)`), `wrap()`, `overlay()`/`stack()` (layered),
  `scroll(layout)`, `panel(layout)`, `group(title, layout)`,
  `tabs().page(title, layout)`, `split(a, b)`. `absolute()` with `.at(x, y, w, h)`
  and `.anchor(..)` is the one free-placement container: a designer surface or
  a custom painter's hit regions.
- **Themed by default.** Use semantic tokens only; owner-draw with the
  `Canvas` API in a `build(|ui| ...)` widget if a native part ignores dark
  mode. Check both themes with `XUI_SNAPSHOT` (light and dark PNGs).

## Verifying without a window

```text
XUI_SNAPSHOT=target/shots XUI_DEMO_AUTOCLOSE_MS=4000 cargo run --example demo
XUI_LAYOUT_REPORT=1 XUI_SNAPSHOT=target/shots cargo run --example demo
```

- `XUI_SNAPSHOT` renders the app on the offscreen backend and writes
  `<name>-light.png` and `<name>-dark.png`; no window opens, so no sandbox is
  needed.
- `XUI_DEMO_AUTOCLOSE_MS=4000` makes a windowed run quit on its own; never
  leave a demo running.
- `XUI_LAYOUT_REPORT=1` prints `ui.layout_report()` and a last line
  `XUI_LAYOUT_WARNINGS:<n>`. **Read the `!` lines**: they name a clipped,
  zero-size, overlapping or text-truncated node. A clean layout has none;
  `scripts/snapshots.ps1` (or `.sh`) renders every example and fails on one.
- `xui-form check|render file.lfm` validates or renders a `.lfm` form
  headlessly; `--report` prints its layout report.

## Where to look

- `docs/cookbook.md` — about 15 copy-ready recipes (form, master/detail,
  toolbar+status, settings, list add/remove, timer monitor), compiled in
  `crates/xui/examples/cookbook.rs`.
- `docs/getting-started.md` — the app shape, backend choice, snapshots.
- `docs/widgets.md` — the widget catalogue with each builder's API.
- `docs/forms.md` + `docs/` — `.lfm` forms and `xui::script` (Rhai) when the
  app is data, not Rust.
- `crates/xui/examples/controls/*.rs` — one runnable demo per widget.
