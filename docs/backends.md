# Backends

A **backend** implements [`xui_core::backend::Backend`](architecture.md#the-backend-contract):
it owns the event loop, creates windows and nodes, decodes native input into
portable `Event`s, paints and measures text. The application chooses one and
hands it to `xui_core::run_app`.

| Backend | Crate | Window | Painting | Native handle | Modal |
|---|---|---|---|---|---|
| `Win32Backend` | `xui-win32` | Win32 top-level | GDI + Direct2D + DirectWrite | yes (`HWND`) | yes |
| `WinitBackend` | `xui-canvas` | `winit` | `tiny-skia` → `softbuffer`, or GL | no | no |
| `OffscreenBackend` | `xui-canvas` | none | `tiny-skia` into an RGBA surface | no | no |
| `GlWidget` host | `xui-canvas` | `winit` | `glow`/OpenGL (whole client area) | no | no |

Every backend runs the **same portable widgets**. This is the point of the
split: an app written against `xui-core` works on Win32, on Linux/macOS through
winit, and headlessly in tests.

## `Win32Backend` — native Windows

`crates/xui-win32/src/backend/`. This is the backend that makes the portable
widgets integrate with Windows: most nodes are painted child windows, but a kind
with a good native control is hosted natively. Today `NodeKind::Edit` is a real
`EDIT`, so text entry gets real IME, selection, clipboard and accessibility;
`supports` reports `ImplKind::Native` for it and `Painted` for everything else.

```rust
use std::rc::Rc;
use xui_core::backend::Backend;
use xui_win32::Win32Backend;

let backend: Rc<dyn Backend> = Rc::new(Win32Backend::new());
```

- Windows, explicit DPI handling, `WM_DPICHANGED` suggestions, per-monitor-v2.
- Window chrome is mapped from `PlatformSpec`: `Backdrop::{Opaque,Acrylic,Mica}`
  and a custom caption (`Decorations::None` + `caption_inset`).
- DirectWrite text with a cached GDI fallback.
- `capture` uses `Windows.Graphics.Capture` when the `wgc` feature is on,
  otherwise `PrintWindow`.
- `run_modal` runs a nested Win32 loop.
- The platform layer underneath (`Window`, `Message`, `Hwnd`, `raw_message`) is
  reachable for interop; see [The Win32 layer](win32.md).

Because the crate is `#![cfg(windows)]`, on other targets `xui-win32` compiles
to an empty crate, so a cross-platform workspace still checks.

## `WinitBackend` — portable, software-rendered

`crates/xui-canvas/src/backend/`. A `winit` event loop composites the widget
nodes into an RGBA buffer with `tiny-skia` and presents it through `softbuffer`.
It is the backend to use on Linux and macOS, and a useful reference renderer on
Windows.

```rust
use std::rc::Rc;
use xui_core::backend::Backend;
use xui_canvas::WinitBackend;

// Must be built on the process's main thread; the loop is consumed by run().
let backend: Rc<dyn Backend> = Rc::new(WinitBackend::new());
```

- `supports` always returns `ImplKind::Painted`; there are no native controls.
- Nodes are kept in creation order (later draws on top; `raise` moves a node
  last). Input is hit-tested and translated into the node's own coordinates.
- DPI comes from the window's `scale_factor` (`round(scale × 96)`). The app is
  built only after the real window exists (`Backend::run_with`), so widgets lay
  out at that scale from the start; when the scale changes later the backend
  rescales node bounds to the backing pixels.
- Text is shaped with `cosmic-text`, which also gives hit-testing and selection
  geometry through the `TextShaper`/`TextLayout` seam.
- `capture` composites into an offscreen surface.
- No `native_window`, and no nested modal loop (`run_modal` reports
  `Unsupported`, so `Ui::open_modal` is unavailable on this backend).
  Non-modal secondary windows (`Ui::open_window`), multiple top-level windows
  and timers all work.
- **One event loop per process.** `WinitBackend::new` must run on the main
  thread; you cannot tear the loop down and build another in the same process.

## `OffscreenBackend` — headless snapshots and tests

`crates/xui-canvas/src/offscreen/`. A full software `Backend` with no OS window,
so it runs on any platform (including headless CI). It renders with the same
node/clip/cull logic as the windowed backend.

```rust
use std::rc::Rc;
use xui_canvas::OffscreenBackend;

let backend = Rc::new(OffscreenBackend::new());
// ... run_app(backend.clone(), spec, |ui| { ... }) ...
let image = backend.render(window).unwrap();      // RgbaImage
let bytes = image.pixel(x, y);
```

- `open_window_at(spec, dpi)` opens a window at an arbitrary DPI, for
  high-DPI layout tests; `Backend::open_window` uses 96.
- `inject(window, event)` delivers a synthetic event to the topmost node under
  its position, translating coordinates.
- `set_gl_content`/`clear_gl_content` mirror the windowed GL seam so a
  `GlWidget`'s software fallback can be tested. The offscreen backend never
  creates a GL context.
- `render(window)` returns an `RgbaImage { width, height, pixels }`.

This is how the `xui-core` and `xui-canvas` unit tests render deterministic
snapshots without a desktop.

## GPU rendering on the canvas backend

A `WinitBackend` window can host a GPU renderer. Install a `GlWidget` and the
backend renders `glow` OpenGL frames through a `GlSurface` (a `glutin`
core-profile 3.3 context on the winit window) into a texture, reads them back and
composites them through the software painter model, so GL content is one painter
among many — the whole client area with `set_gl_content`, or one pane with
`set_gl_content_on`:

```rust
use xui_canvas::{GlWidget, glow};
use xui_canvas::glow::HasContext;

impl GlWidget for Visualizer {
    // Software fallback: runs when no GL context can be created.
    fn paint(&self, canvas: &mut dyn xui_core::backend::Canvas, bounds: Rect, theme: &Theme) { /* ... */ }

    fn paint_gl(&self, gl: &glow::Context, bounds: Rect, theme: &Theme) { /* issue GL calls */ }

    fn gl_teardown(&self, gl: &glow::Context) { /* free GPU resources while current */ }
}

// From inside run_app's `make`, with an Rc<WinitBackend> in hand:
backend.set_gl_content(ui.window(), Triangle::new());

// Or fill one node's pane, laid out like any other widget:
backend.set_gl_content_on(visualizer_node, Triangle::new());
```

```text
cargo run -p xui-canvas --example gl
```

Constraints worth knowing:

- **Composited, not a takeover.** A GL frame is rendered into an offscreen
  texture, read back as pixels and composited through the same software surface
  as CPU nodes: window-level content is the base layer and node-level content
  sits at its node's bounds, so a GL visualizer can share a window with ordinary
  widgets.
- **Fallback is permanent.** If no display/config/context can be created, or a
  frame cannot be rendered, the backend switches to the software `paint` path
  for good, so the window stays usable. `gl_teardown` runs with the context
  still current.
- The GL context and `glow` loader are the crate's only `unsafe`
  (`src/sys/gl/`); every other module forbids it.
- `glow` is re-exported as `xui_canvas::glow` so an implementor names the exact
  version the crate loads.

## `xui-litehtml` — an HTML view

`crates/xui-litehtml` lays a page out with `litehtml` on a worker thread and
paints it with Direct2D/DirectWrite through `xui-win32`. It is a Windows-only
`CustomWidget` host today (`HtmlView<M>`), with links, scrolling, text selection
and copy. The middle is backend-neutral — the worker emits a `DisplayList` of
neutral draw commands — so portability to `xui-core`/`xui-canvas` is a follow-up
(issues #51, #52, #35). It is not part of the umbrella crate.

## Combining backends

**Same app, either backend.** `run_app` takes `Rc<dyn Backend>`, so the choice
can be made at run time:

```rust
fn backend() -> Rc<dyn Backend> {
    #[cfg(all(feature = "win32", windows))]
    if std::env::var("XUI_BACKEND").as_deref() != Ok("canvas") {
        return Rc::new(xui_win32::Win32Backend::new());
    }
    Rc::new(xui_canvas::WinitBackend::new())
}
```

`crates/xui/examples/widgets.rs` does exactly this and adds a **Renderer**
button that relaunches the process on the other backend — a new process because
`winit` allows a single event loop per process.

**Compile-time selection with the umbrella.** Depend on `xui` with a feature:

```toml
[dependencies]
xui = { version = "0.1", default-features = false, features = ["canvas"] }
```

`win32` (default) and `canvas` can also be enabled together; the examples gate
the switch on `cfg!(all(feature = "win32", windows))`. Remember the umbrella's
bare-name rule from [Architecture](architecture.md#the-two-widget-layers): with
`win32` on Windows the bare names are the native layer, so portable code should
spell the core path (`xui::xui_core::…`) or depend on `xui-core` directly.

**Things you cannot mix.** A window belongs to exactly one backend: its nodes,
painters and event sink all live there, and a `WidgetId` is meaningless to
another backend. The offscreen backend is a separate world used for rendering,
not a window you show. GL content is the exception inside a canvas window, but it
is composited like any other node and can cover the whole window or one pane.

**Choosing.** Use `Win32Backend` for Windows apps that want native text fields
and the Windows window features; use `WinitBackend` when the app must also run
on Linux/macOS or be snapshot-tested; use `OffscreenBackend` in tests and build
tooling. [Getting started](getting-started.md) walks through the decision.
