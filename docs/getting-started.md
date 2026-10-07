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

Add the umbrella crate. Its default `canvas` feature is the cross-platform
software backend; `d2d` adds the Windows-native Direct2D backend:

```toml
# Cargo.toml
[dependencies]
xui = { path = "../xui/crates/xui" }   # or a crates.io version
```

## 3. Write the app

An xui app is a struct implementing `App`, plus one `xui::app(..)` call that
opens the window and mounts its layout. Widgets are described with builders
(`label`, `edit`, `button`, ...) inside rows, columns and grids; the layout
creates them, sizes each from its content, and re-flows on resize, DPI change
and `Ui::set_visible`. Events map to your own `Msg` when the widget is
described; the runtime delivers those messages to `App::update`, which is
never re-entered.

```rust
// src/main.rs
use xui::prelude::*;

#[derive(Clone)]
enum Msg {
    Text(String),
    Bump,
}

#[derive(Default)]
struct Counter {
    echo: Handle<Label<Msg>>, // the one widget the app changes
    clicks: i32,
}

impl App for Counter {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _ui: &mut Ui<Msg>) {
        let text = match msg {
            Msg::Text(text) => text,
            Msg::Bump => {
                self.clicks += 1;
                format!("{} clicks", self.clicks)
            }
        };
        self.echo.get().set_text(&text);
    }
}

fn main() -> Result<()> {
    xui::app("Counter").size(360, 200).run(|ui| {
        let app = Counter::default();
        ui.root(column().padding(16).gap(8).children((
            label("type something").bind(&app.echo),
            edit().placeholder("text").on_change(Msg::Text),
            row().justify(Align::End).child(button("Count").on_click(Msg::Bump)),
        )))?;
        Ok(app)
    })
}
```

A few things to notice:

- **No coordinates and no `ui` per widget.** A builder becomes a widget when
  the layout is mounted, parented to the container it is mounted in. The first
  widget that fails to build surfaces from `ui.root(..)?`.
- **Handles for what you change.** `bind(&handle)` fills a `Handle` when the
  widget is created; `handle.get()` reaches it from `update`. `ui.root` keeps
  everything else alive as long as the window.
- **Sizing and placement.** `.fill(weight)`, `.fixed(n)`, `.min(n)`,
  `.width(n)`, `.height(n)`, `.max_width(n)`, `.align(Align::Center)` and, in a
  `grid([Track::Auto, Track::Fill(1)])`, `.span(columns)`. Numbers are design
  units (`Dip`). `spacer()` is an empty flexible gap; `group(title, layout)`
  frames a layout and `tabs().page(title, layout)` pages them.
- **Headless screenshots.** `XUI_SNAPSHOT=<dir> cargo run` renders the app in
  light and dark into `<dir>` without opening a window;
  `XUI_DEMO_AUTOCLOSE_MS=4000` makes it quit on its own.

A larger runnable version is `crates/xui/examples/layout.rs`.

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
| An HTML page or a web browser view, on any backend | `xui-blitz`'s `BlitzView` |
| An HTML page laid out by litehtml (being replaced by `xui-blitz`) | `xui-litehtml`'s `HtmlView` |

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

`xui::app` picks the backend: the Direct2D one where the `d2d` feature is on
(on Windows) unless `XUI_BACKEND=canvas`, otherwise the software backend.
`.backend(..)` overrides it with any `Rc<dyn Backend>`, and
`xui_core::app(..)` and `run_app` are the same entry points for an app that
depends on `xui-core` and a backend crate directly.

```toml
[dependencies]
xui = { version = "0.1", features = ["d2d"] }      # also the Direct2D backend on Windows
```

## 7. Explore the samples

- `crates/xui/examples/widgets.rs` — every portable widget, switchable backend.
- `crates/xui/examples/listview.rs` and `gridview.rs` — virtualized models.
- `crates/xui/examples/controls/` — one small example per portable widget.
- `crates/xui-canvas/examples/gl.rs` — a `GlWidget`.

Read next: [Cookbook](cookbook.md), [Widgets](widgets.md), [Theming](theming.md),
[Architecture](architecture.md).
