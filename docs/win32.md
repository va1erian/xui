# The Win32 layer

`xui-win32` is the Windows-only backend: [`Win32Backend`](backends.md) runs the
portable `xui-core` widgets with native window chrome and Direct2D/DirectWrite
painting. Underneath it sits a small, safe platform layer over Win32 that is also
reachable directly for interop. There is no widget API of its own; widgets are
`xui-core`'s.

If you are writing a normal application, use the portable widgets and
[`Win32Backend`](backends.md) (or the default canvas backend). Read this chapter
when you need the Windows escape hatches: raw messages, the `HWND`, GDI or
Direct2D drawing.

## The platform layer

A safe, honest model of Win32. [`Window`] wraps an `HWND` with RAII, a
[`WindowClass`] registers and unregisters itself, and a [`WindowHandler`]
receives typed messages:

```rust
use xui_win32::prelude::*;

struct Main;

impl WindowHandler for Main {
    fn message(&self, window: &Window, message: Message) -> Option<LResult> {
        if let Message::Close = message {
            window.destroy();
            xui_win32::quit(0);
            return Some(0);
        }
        None
    }
}
```

- **Typed messages.** `Message::{Create, Close, Paint, Size, Timer, Activate,
  DpiChanged, KeyDown, Char, Mouse*, Command, Notify, …}` replace raw
  `(u32, WPARAM, LPARAM)` triples.
- **RAII GDI.** `gdi::{Font, Brush, Pen, Bitmap}` and a double-buffered
  `gdi::Paint` / `Canvas`, so there is no manual `DeleteObject`.
- **Direct2D/DirectWrite.** `d2d` is the layer `Win32Backend` paints portable
  widgets through: one `BeginDraw`/`EndDraw` per node, text through DirectWrite,
  with a GDI fallback if a Direct2D device cannot be created.
- **Raw hook.** `WindowHandler::raw_message` sees every message before it is
  decoded; the pointer is valid only for the call.
- **`Hwnd`.** An opaque, `Copy` handle (`raw()`, `is_alive()`) for OS interop,
  with no `windows` type in the public API.
- **`looper`.** `run`, `quit`, `run_modal`, and `xui_win32::init()` (per-monitor
  v2 DPI, idempotent).
- `unsafe` is confined to `src/sys/`; every other module forbids it.

## How messages are dispatched

`Window::create` boxes the caller's `WindowHandler` into a thin
`*mut Box<dyn WindowHandler>` and stashes it in `GWLP_USERDATA` on `WM_NCCREATE`;
the shared `window_proc` (`sys::dispatch`) decodes each raw message into a typed
`Message` and calls the handler. The handler is shared (`&self`), so a
synchronous message that arrives while the handler is already on the stack — a
`WM_SIZE` from a call inside the handler, a modal loop — is still delivered
rather than dropped; mutable state lives in `Cell`/`RefCell` fields. The box is
reclaimed once, on `WM_NCDESTROY`; if the window was destroyed from inside its own
handler, the free is deferred until the outermost dispatch for that window
returns.

## Fonts

DirectWrite resolves `system-ui` to the system message font
(`SystemParametersInfoForDpi(SPI_GETNONCLIENTMETRICS).lfMessageFont`, Segoe UI
9 pt on stock Windows), scaled to the window's DPI. `gdi::Font::system_ui` builds
the same face for the GDI fallback and for the native `Edit` node.

## Theming on Win32

- The window stores its `Theme` and answers `WM_CTLCOLOREDIT/STATIC/BTN/LISTBOX/
  DLG` with cached brushes for the native `EDIT` child.
  `Backend::set_theme` also sets the DWM dark title bar
  (`DWMWA_USE_IMMERSIVE_DARK_MODE`) and updates the class background (no white
  flashes).
- `Theme::system()` / `is_theme_change` read and track the user's preference
  (see [Theming](theming.md)).

## Interop with the portable layer

The Win32 backend exposes the handles behind its windows and nodes for
interop — `Win32Backend::window_hwnd(window)` and
`Win32Backend::node_hwnd(widget)` — and the portable `Ui::native_window()`
returns an opaque `NativeWindowHandle`. An OS integration can therefore be
written without naming a `windows` type. See
[Platform integration](platform-integration.md).

## Related

- [Windows-only window features](win32-windows.md) — backdrop, custom caption,
  monitors and capture.
- [Theming](theming.md) — the shared token set.
- [Architecture](architecture.md) — the portable layer and the `Backend`
  contract.
- [Platform integration](platform-integration.md) — the escape hatches
  (`Hwnd`, raw messages).
