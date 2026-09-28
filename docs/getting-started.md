# Getting started

This tutorial builds a small but complete xui application, then explains how to
choose — and switch — the backend it runs on.

## 1. Prerequisites

- A recent stable Rust toolchain (the crates are edition 2024).
- On Linux/macOS, nothing extra for the software backend: `winit`, `softbuffer`
  and `tiny-skia` are pure Rust and dlopen the system graphics libraries.
- On Windows, the Visual Studio build tools as usual; the Win32 backend links
  the system libraries.

## 2. Create the project

```text
cargo new counter
```

Add `xui-core` and one backend. For a Windows-native build use `xui-win32`:

```toml
# Cargo.toml
[dependencies]
xui-core = { path = "../xui/crates/xui-core" }   # or a crates.io version
xui-win32 = { path = "../xui/crates/xui-win32" }
```

For a cross-platform build use `xui-canvas` instead:

```toml
[dependencies]
xui-core = { path = "../xui/crates/xui-core" }
xui-canvas = { path = "../xui/crates/xui-canvas" }
```

You can depend on both and choose at run time — see step 6.

## 3. Write the app

An xui app is a struct implementing `App`, plus a `run_app` call that builds the
window and the widgets. Widget events map to your own `Msg` through closures
fixed when the widget is built; the runtime delivers those messages to
`App::update`, which is never re-entered.

```rust
// src/main.rs
use std::rc::Rc;

use xui_core::app::{App, Ui, run_app};
use xui_core::backend::{Backend, PlatformSpec};
use xui_core::widget::{Button, Edit, Label};
use xui_core::{Dip, Rect};

enum Msg {
    Text(String),
    Bump,
}

struct Counter {
    // Widgets are values the app owns and keeps alive.
    echo: Label<Msg>,
    _edit: Edit<Msg>,
    _button: Button<Msg>,
    edits: u32,
    clicks: i32,
}

impl App for Counter {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Text(text) => {
                self.edits += 1;
                self.echo.set_text(&format!("{text} ({})", self.edits));
            }
            Msg::Bump => {
                self.clicks += 1;
                self.echo.set_text(&format!("{} clicks", self.clicks));
            }
        }
        let _ = ui; // use `ui` to open windows, set a theme, start timers, ...
    }
}

fn main() -> xui_core::backend::Result<()> {
    // Pick a backend. This is the only line that changes between targets.
    let backend: Rc<dyn Backend> = Rc::new(xui_win32::Win32Backend::new());

    run_app(
        backend,
        PlatformSpec::new("Counter").size(Dip(360.0), Dip(200.0)),
        |ui| {
            let echo = Label::new(ui, Rect::new(16, 16, 344, 48), "type something").unwrap();
            let edit = Edit::new(ui, Rect::new(16, 56, 344, 96), "")
                .unwrap()
                .on_change(|text| Some(Msg::Text(text.to_string())));
            let button = Button::new(ui, Rect::new(16, 112, 160, 152), "Count")
                .unwrap()
                .on_click(|| Some(Msg::Bump));

            // Keep every widget alive by returning it in the app.
            Counter {
                echo,
                edits: 0,
                clicks: 0,
                // `edit` and `button` are used only through the closures above.
                _edit: edit,
                _button: button,
            }
        },
    )
}
```

A few things to notice:

- **Bounds are `Rect` in device pixels.** Sizes you design are `Dip` and are
  converted once, at the window's DPI. Read a design value's pixels with
  `Dip::to_px(ui.dpi())`.
- **Widgets are retained values.** Hold the ones you mutate later; dropping a
  widget destroys its node.
- **Positions are explicit, or declared.** `Button::new(ui, rect, ..)` places a
  widget yourself; a container widget (`Panel`, `ScrollView`, `Split`, `Tabs`)
  owns and arranges its children. To avoid coordinates altogether, build with
  `Button::auto(ui, ..)` and lay widgets out declaratively; see
  [Layout without coordinates](#layout-without-coordinates) below.

### Layout without coordinates

The same counter, with no `Rect`s, no per-widget fields and no `unwrap` per
widget. `xui_core::arrange` owns the widgets, sizes each from its content, and
re-flows on resize, DPI change and `Ui::set_visible`:

```rust
use std::rc::Rc;
use xui_core::arrange::{LayoutExt, Mounted, column, row, spacer};
use xui_core::{Insets, dip};

struct Counter {
    echo: Rc<Label<Msg>>, // shared: the app updates it
    clicks: i32,
    _mounted: Mounted<Msg>, // owns the whole widget tree
}

fn build(ui: &Ui<Msg>) -> xui_core::backend::Result<Counter> {
    let echo = Rc::new(Label::auto(ui, "type something")?);
    let root = column()
        .margins(Insets::all(dip(16.0)))
        .spacing(dip(8.0))
        .child(&echo)
        .child(Edit::auto(ui, "").map(|e| e.on_change(|t| Some(Msg::Text(t.to_string())))))
        .child(row().child(spacer()).child(
            Button::auto(ui, "Count").map(|b| b.on_click(|| Some(Msg::Bump))),
        ));
    Ok(Counter { echo, clicks: 0, _mounted: ui.mount(root)? })
}
```

Size an entry with `.fill(weight)`, `.fixed(dip)`, `.min(dip)`, `.width(dip)` or
`.height(dip)`; `spacer()` is an empty flexible gap. A runnable version is
`crates/xui/examples/layout.rs`.

## 4. Run it

```text
cargo run
```

The window appears at 360 × 200 and the widgets work. On Windows the `Edit` is a
real native text field; on the canvas backend it is painted.

## 5. Choose a backend

Use the table below; the full capability comparison is in [Backends](backends.md).

| You want… | Use |
|---|---|
| A Windows-only app with native text input, backdrop materials, the extended title bar, monitors, UI Automation and GL/Direct2D | `xui_win32::Win32Backend` |
| One binary that also runs on Linux and macOS, with no platform UI toolkit | `xui_canvas::WinitBackend` |
| GPU rendering inside a canvas window | a `GlWidget` installed with `WinitBackend::set_gl_content` |
| Deterministic headless snapshots in tests and tooling | `xui_canvas::OffscreenBackend` |
| An HTML page rendered with Direct2D on Windows | `xui-litehtml`'s `HtmlView` |

Two practical notes:

- `WinitBackend::new()` must be called on the main thread, and a process can have
  only one `winit` event loop. If you offer a runtime backend switch, relaunch
  the process (as `crates/xui/examples/widgets.rs` does).
- The same widget code runs on every backend, so a good pattern is to develop
  against the canvas backend (it builds everywhere and is snapshot-testable) and
  ship the Win32 backend on Windows.

To run the ready-made galleries:

```text
cargo run -p xui --features canvas --example widgets   # portable widgets, software backend
cargo run -p xui-win32 --example demo                  # the Win32-native layer
cargo run -p xui-canvas --example gl                   # GPU triangle
```

## 6. Switching the backend at run time

Because `run_app` takes `Rc<dyn Backend>`, selection can be a runtime decision:

```rust
fn backend() -> Rc<dyn Backend> {
    #[cfg(all(feature = "win32", windows))]
    if std::env::var("XUI_BACKEND").as_deref() != Ok("canvas") {
        return Rc::new(xui_win32::Win32Backend::new());
    }
    Rc::new(xui_canvas::WinitBackend::new())
}

fn main() -> xui_core::backend::Result<()> {
    run_app(backend(), PlatformSpec::new("Counter"), |ui| { /* ... */ })
}
```

Compile both backends in with the umbrella crate's features:

```toml
[dependencies]
xui = { version = "0.1" }                          # win32 by default
xui = { version = "0.1", features = ["canvas"] }   # add the software backend
```

With the umbrella, the portable entry point is `xui::xui_core::run_app` and the
backends are `xui::Win32Backend` / `xui::xui_canvas::WinitBackend`. Note that
with the default `win32` feature on Windows, the umbrella's *bare* names
(`xui::run_app`, `xui::Label`) are the Win32-native layer; spell the core path
when you want the portable widgets. See [Architecture → The two widget
layers](architecture.md#the-two-widget-layers).

## 7. Explore the samples

- `crates/xui/examples/widgets.rs` — every portable widget, switchable backend.
- `crates/xui/examples/listview.rs` and `gridview.rs` — virtualized models.
- `crates/xui-win32/examples/demo/` — the Win32-native layer: backdrop, strip
  menu, tabs, tree, Direct2D documents, OpenGL cube.
- `crates/xui-canvas/examples/gl.rs` — a `GlWidget`.
- `crates/xui-win32/examples/umbrella.rs` — the smallest umbrella app.

Read next: [Widgets](widgets.md), [Theming](theming.md),
[Architecture](architecture.md).
