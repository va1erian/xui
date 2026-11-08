# xui

[![CI](https://github.com/va1erian/xui/actions/workflows/ci.yml/badge.svg)](https://github.com/va1erian/xui/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/va1erian/xui/branch/main/graph/badge.svg)](https://codecov.io/gh/va1erian/xui)

Cross-platform UI for Rust: **small, fast, idiomatic and properly themed**.

xui is a portable widget layer (`xui-core`) plus interchangeable **backends**.
An application writes its widgets once, in device-independent units, and picks
how they are hosted and drawn — a `winit` + `tiny-skia` software surface
everywhere, or an offscreen surface for snapshots and tests. Dark mode is first-class on every backend.

```text
                ┌──────────────────────────────────────────┐
  your app ───▶    xui-core: App / Ui, widgets, layout,      
                    theme, input, accessibility, Backend    
                └───────────────┬──────────────────────────┘
                                │ Backend trait
          ┌─────────────────────┴───────────────────────┐
          ▼                                             ▼
   xui-canvas (winit)                              xui-canvas
   WinitBackend                                    OffscreenBackend
   (tiny-skia software,                            (headless
    optional glow/GL)                               snapshots)
```

## Crates

| Crate | What it is |
|---|---|
| `xui-core` | The portable front layer: geometry, units, colour, pure layout arithmetic, semantic theme tokens, the input vocabulary, the accessibility model, the **widget layer**, the `App`/`Ui` runtime and the `Backend` contract. No platform dependency, no `unsafe`. |
| `xui-canvas` | The cross-platform software backend: a `winit` window compositing with `tiny-skia` and presenting through `softbuffer`, an optional GPU path (`glow` OpenGL through `glutin`), and an `OffscreenBackend` that renders the same widgets headlessly. |
| `xui-d2d` | The Windows-only lightweight backend: one Win32 window per xui window, drawn with Direct2D and DirectWrite, with native file dialogs, native child surfaces for a GPU API and the pointer lock. About a fifth of the size of `xui-canvas`; no windowing dependencies. Not covered: IME, UI Automation, borderless windows. |
| `xui-gpu` | The shared OpenGL seam behind both backends' GL widgets: the opaque `GlContext` handle, the `GlSurface` frame/offscreen lifecycle and the offscreen readback. Keeps `glow` out of `xui-core`. |
| `xui` | The umbrella crate most applications depend on. Selects a backend by feature (`canvas`, the default) and re-exports the portable front layer. |
| `xui-code-editor` | A code-editor widget: rope buffer, monospace view, find/replace, pluggable syntax highlighting. |
| `xui-rich-text` | An opt-in editable rich-text widget for a basic word processor: styled runs, paragraph formatting, lists, inline and floating images that text wraps around, undo, JSON save and Markdown export. Portable: lays out through the backend `TextShaper`. Enable with `xui`'s `rich-text` feature. |
| `xui-markdown` | An opt-in read-only Markdown view: CommonMark with GFM tables, strike-through and task lists, links, scrolling. Portable: lays out through the backend `TextShaper`, visible blocks first. Enable with `xui`'s `markdown` feature. |
| `xui-icons` | An optional, multi-colour vector icon set (36 icons) in the flat "Global Village" style, drawn through the portable `Canvas` path API, so the software backend rasterises it with `tiny-skia`. Depends on `xui-core` only. |
| `xui-blitz` | A web view on the [Blitz](https://github.com/DioxusLabs/blitz) engine (Stylo, Taffy, Parley): CSS grid, flexbox, floats, `var()`, SVG; links reported or followed, fetching through the app's `Fetcher`, downloads, selection and copy. A portable custom-painted node. |

## Documentation

- **[Getting started](docs/getting-started.md)** — set up a basic app and pick a
  backend.
- **[Architecture](docs/architecture.md)** — the layers, the `Backend`
  contract, the message model and the invariants.
- **[Backends](docs/backends.md)** — what each backend supports, and how to use
  or combine them.
- **[Widgets](docs/widgets.md)** — the portable widget catalogue and models.
- **[Theming](docs/theming.md)** — tokens, dark mode and live switching.
- **[macOS](docs/macos.md)** and **[sandboxed testing](docs/sandbox-testing.md)**.
- **[Headless screenshots](docs/headless-screenshots.md)** — render a window or
  the whole example gallery to PNG with no window, and use it from your own app.
- **[Development](docs/development.md)** — checks, CI and how to add a widget.
- **[The `attic` branch](#the-attic-branch)** — crates and features removed from `main`.

## Quick start

An app depends on `xui` (the umbrella crate, which picks a backend), builds an
`App`, and describes its window as a layout of widget builders:

```rust
use xui::prelude::*;

#[derive(Clone)]
enum Msg {
    Bump,
}

struct Counter {
    label: Handle<Label<Msg>>,
    count: i32,
}

impl App for Counter {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _ui: &mut Ui<Msg>) {
        match msg {
            Msg::Bump => {
                self.count += 1;
                self.label.get().set_text(&format!("{} clicks", self.count));
            }
        }
    }
}

fn main() -> Result<()> {
    xui::app("Counter").size(320, 160).run(|ui| {
        let clicks = Handle::new();
        ui.root(column().padding(16).gap(8).children((
            label("0 clicks").bind(&clicks),
            button("Click me").on_click(Msg::Bump),
        )))?;
        Ok(Counter { label: clicks, count: 0 })
    })
}
```

See **[Getting started](docs/getting-started.md)** for the full walk-through,
including how to choose and switch the backend.

## Goals

- **Portable first.** Widgets, layout, theming and input live in `xui-core` and
  are shared by every backend. A widget never names a platform handle.
- **Fast where it counts.** The default backend is portable software rendering,
  with an optional GPU path.
- **Themed, dark mode included.** Controls draw only from semantic theme
  tokens; the app picks a `Theme` (or follows the system) and never handles
  `NM_CUSTOMDRAW`, `WM_CTLCOLOR*` or `SetWindowTheme`.
- **Idiomatic Rust, without a class hierarchy.** Events map to the app's own
  message type through closures fixed at construction; shared behaviour comes
  from traits; there are no numeric control ids.
- **Testable offscreen.** The same widgets render headlessly for deterministic
  snapshots and unit tests.

## Status

The portable core, the widget layer and the `xui-canvas` software backend all
exist and are exercised by tests and examples. The `xui` umbrella's default
`canvas` feature is the cross-platform backend; its bare names are the portable
`xui-core` widgets.

## The `attic` branch

Code that no known consumer uses is removed from `main` but kept, with its
history and tests, on the [`attic`](https://github.com/va1erian/xui/tree/attic)
branch. It holds the last state of:

- `xui-litehtml`, `xui-netsurf` (replaced by `xui-blitz`), `xui-win32` (the
  Direct2D backend and its `d2d` feature), `xui-explorer` and `xui-paint`;
- `xui-form`, `xui-script` and `xui-form-cli` (`.lfm` forms and Rhai scripting,
  with the `form` and `rhai` features of `xui` and `docs/forms.md`);
- the "Aero" icon style of `xui-icons` (the `aero` and `icons-aero` features).

To bring one back, check it out from the branch (for example
`git checkout attic -- crates/xui-form`) and restore its workspace entry.

## License

MIT. See [LICENSE](LICENSE).

## Credits

The built-in icons are outlines from [Lucide](https://lucide.dev), vendored
under [`crates/xui-core/assets/lucide/`](crates/xui-core/assets/lucide/) and
used under the ISC License (copyright Lucide Icons and Contributors; see the
[license text](crates/xui-core/assets/lucide/LICENSE)). The generator turns them
into `xui_core::icon::Lucide`, one variant per SVG; `Icon` and `Glyph` are the
older, smaller vocabularies kept for compatibility and both draw the same
shapes through `xui_core::icon::draw_icon`.
