# Getting started

This tutorial builds a small but complete xui application, then explains how to
choose — and switch — the backend it runs on.

## 1. Prerequisites

- A recent stable Rust toolchain (the crates are edition 2024).
- On Linux/macOS, nothing extra for the software backend: `winit`, `softbuffer`
  and `tiny-skia` are pure Rust and dlopen the system graphics libraries.
- On Windows, the Visual Studio build tools as usual; the optional Win32
  (`d2d`) backend links the system libraries.

## 2. Create the project

```text
cargo new counter
```

Add `xui-core` and one backend. The default, cross-platform choice is
`xui-canvas`:

```toml
# Cargo.toml
[dependencies]
xui-core = { path = "../xui/crates/xui-core" }   # or a crates.io version
xui-canvas = { path = "../xui/crates/xui-canvas" }
```

For a Windows-only build with native window chrome and Direct2D painting, use
`xui-win32` instead:

```toml
[dependencies]
xui-core = { path = "../xui/crates/xui-core" }
xui-win32 = { path = "../xui/crates/xui-win32" }
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
    let backend: Rc<dyn Backend> = Rc::new(xui_canvas::WinitBackend::new());

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
- **Positions are explicit.** There is no layout tree in the portable layer; a
  container widget (`Panel`, `ScrollView`, `Split`, `Tabs`) owns and arranges
  its children, and the pure `Dock`/`Stack` arithmetic is available if you want
  to compute positions yourself.

## 4. Run it

```text
cargo run
```

The window appears at 360 × 200 and the widgets work.

## 5. Choose a backend

Use the table below; the full capability comparison is in [Backends](backends.md).

| You want… | Use |
|---|---|
| One binary that also runs on Linux and macOS, with no platform UI toolkit (the default) | `xui_canvas::WinitBackend` |
| A Windows-only app with native text input, backdrop materials, the extended title bar and Direct2D-accelerated painting | `xui_win32::Win32Backend` |
| GPU rendering inside a canvas window | a `GlWidget` installed with `WinitBackend::set_gl_content` |
| Deterministic headless snapshots in tests and tooling | `xui_canvas::OffscreenBackend` |
| An HTML page rendered with Direct2D on Windows | `xui-litehtml`'s `HtmlView` |

Two practical notes:

- `WinitBackend::new()` must be called on the main thread, and a process can have
  only one `winit` event loop. If you offer a runtime backend switch, relaunch
  the process (as `crates/xui/examples/widgets.rs` does).
- The same widget code runs on every backend, so a good pattern is to develop
  against the canvas backend (it builds everywhere and is snapshot-testable) and
  optionally ship the Win32 backend on Windows.

To run the ready-made galleries:

```text
cargo run -p xui --example widgets      # portable widgets, software backend
cargo run -p xui-canvas --example gl    # GPU triangle
```

## 6. Switching the backend at run time

Because `run_app` takes `Rc<dyn Backend>`, selection can be a runtime decision:

```rust
fn backend() -> Rc<dyn Backend> {
    #[cfg(all(feature = "d2d", windows))]
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
xui = { version = "0.1" }                          # canvas by default
xui = { version = "0.1", features = ["d2d"] }      # also the Direct2D backend on Windows
```

With the umbrella, the portable entry point is `xui::run_app` (the same as
`xui_core::run_app`) and the backends are `xui::xui_win32::Win32Backend` /
`xui::xui_canvas::WinitBackend`.

## 7. Explore the samples

- `crates/xui/examples/widgets.rs` — every portable widget, switchable backend.
- `crates/xui/examples/listview.rs` and `gridview.rs` — virtualized models.
- `crates/xui/examples/controls/` — one small example per portable widget.
- `crates/xui-canvas/examples/gl.rs` — a `GlWidget`.

Read next: [Widgets](widgets.md), [Theming](theming.md),
[Architecture](architecture.md).
