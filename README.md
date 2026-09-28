# xui

[![CI](https://github.com/va1erian/xui/actions/workflows/ci.yml/badge.svg)](https://github.com/va1erian/xui/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/va1erian/xui/branch/main/graph/badge.svg)](https://codecov.io/gh/va1erian/xui)

Cross-platform UI for Rust: **small, fast, idiomatic and properly themed**.

xui is a portable widget layer (`xui-core`) plus interchangeable **backends**.
An application writes its widgets once, in device-independent units, and picks
how they are hosted and drawn — native Win32 common controls on Windows, a
`winit` + `tiny-skia` software surface everywhere, or an offscreen surface for
snapshots and tests. Dark mode is first-class on every backend.

```text
                ┌──────────────────────────────────────────┐
  your app ───▶    xui-core: App / Ui, widgets, layout,      
                    theme, input, accessibility, Backend    
                └───────────────┬──────────────────────────┘
                                │ Backend trait
          ┌─────────────────────┼───────────────────────┬─────────────┐
          ▼                     ▼                       ▼             ▼
   xui-win32            xui-canvas (winit)      xui-canvas       xui-litehtml
   Win32Backend         WinitBackend            OffscreenBackend  HTML view
   (native Edit,        (tiny-skia software,    (headless         (Windows,
    GDI/D2D paint)       optional glow/GL)       snapshots)        Direct2D)
```

## Crates

| Crate | What it is |
|---|---|
| `xui-core` | The portable front layer: geometry, units, colour, pure layout arithmetic, semantic theme tokens, the input vocabulary, the accessibility model, the **widget layer**, the `App`/`Ui` runtime and the `Backend` contract. No platform dependency, no `unsafe`. |
| `xui-win32` | The optional Windows backend: `Win32Backend` (an implementation of `xui-core`'s contract) with native window chrome and Direct2D/DirectWrite painting (GDI fallback), backdrop materials, a custom caption, monitors and capture, plus the low-level platform layer it is built on. No widget API of its own. |
| `xui-canvas` | The cross-platform software backend: a `winit` window compositing with `tiny-skia` and presenting through `softbuffer`, an optional GPU path (`glow` OpenGL through `glutin`), and an `OffscreenBackend` that renders the same widgets headlessly. |
| `xui-gpu` | The shared OpenGL seam behind both backends' GL widgets: the opaque `GlContext` handle, the `GlSurface` frame/offscreen lifecycle and the offscreen readback. Keeps `glow` out of `xui-core`. |
| `xui` | The umbrella crate most applications depend on. Selects a backend by feature (`canvas` by default, `d2d` for the Windows Direct2D backend) and re-exports the portable front layer. |
| `xui-litehtml` | An HTML view built on `litehtml` and Direct2D. A portable custom-painted node; Windows-only today because copy uses the Win32 clipboard. |

## Documentation

- **[Getting started](docs/getting-started.md)** — set up a basic app and pick a
  backend.
- **[Architecture](docs/architecture.md)** — the layers, the `Backend`
  contract, the message model and the invariants.
- **[Backends](docs/backends.md)** — what each backend supports, and how to use
  or combine them.
- **[Widgets](docs/widgets.md)** — the portable widget catalogue and models.
- **[Theming](docs/theming.md)** — tokens, dark mode and live switching.
- **[The Win32 layer](docs/win32.md)** — the Windows backend, its platform layer
  and the Windows-only window features.
- **[Platform integration](docs/platform-integration.md)** — where OS services
  (media keys, taskbar, notifications) belong.
- **[Migration from `win32ui`](docs/migration-win32ui-to-xui.md)**.
- **[macOS](docs/macos.md)** and **[sandboxed testing](docs/sandbox-testing.md)**.
- **[Development](docs/development.md)** — checks, CI and how to add a widget.

## Quick start

An app depends on `xui-core` plus the backend it wants, builds an `App`, and
hands a backend to `run_app`:

```rust
use std::rc::Rc;

use xui_core::app::{App, Ui, run_app};
use xui_core::backend::{Backend, PlatformSpec};
use xui_core::widget::{Button, Label};
use xui_core::{Dip, Rect};

enum Msg {
    Bump,
}

struct Counter {
    label: Label<Msg>,
    count: i32,
}

impl App for Counter {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Bump => {
                self.count += 1;
                self.label.set_text(&format!("{} clicks", self.count));
            }
        }
    }
}

fn main() -> xui_core::backend::Result<()> {
    let backend: Rc<dyn Backend> = Rc::new(xui_canvas::WinitBackend::new());
    run_app(
        backend,
        PlatformSpec::new("Counter").size(Dip(320.0), Dip(160.0)),
        |ui| {
            let label = Label::new(ui, Rect::new(16, 16, 304, 48), "0 clicks").unwrap();
            let button = Button::new(ui, Rect::new(16, 64, 304, 104), "Click me")
                .unwrap()
                .on_click(|| Some(Msg::Bump));
            Counter { label, count: 0 }
        },
    )
}
```

See **[Getting started](docs/getting-started.md)** for the full walk-through,
including how to choose and switch the backend.

## Goals

- **Portable first.** Widgets, layout, theming and input live in `xui-core` and
  are shared by every backend. A widget never names a platform handle.
- **Fast where it counts.** The default backend is portable software rendering;
  on Windows an opt-in backend paints the same widgets through Direct2D and hosts
  a real native `EDIT` where that is best (today, `Edit`), so IME and selection
  come for free.
- **Themed, dark mode included.** Controls draw only from semantic theme
  tokens; the app picks a `Theme` (or follows the system) and never handles
  `NM_CUSTOMDRAW`, `WM_CTLCOLOR*` or `SetWindowTheme`.
- **Idiomatic Rust, without a class hierarchy.** Events map to the app's own
  message type through closures fixed at construction; shared behaviour comes
  from traits; there are no numeric control ids.
- **Testable offscreen.** The same widgets render headlessly for deterministic
  snapshots and unit tests.

## Status

The portable core, the widget layer, the `Win32Backend` and the `xui-canvas`
software backend all exist and are exercised by tests and examples. The
decoupling from the original Win32-only crate is still in progress; see the
[epic](https://github.com/va1erian/xui/issues/1) for milestones.

The `xui` umbrella's default `canvas` feature is the cross-platform backend; the
`d2d` feature adds the Windows Direct2D backend. Its bare names are always the
portable `xui-core` widgets. The original Win32-native widget layer was removed
(see [Migration](docs/migration-win32ui-to-xui.md)).

## License

MIT. See [LICENSE](LICENSE).
