# Migrating from `win32ui` to `xui`

`win32ui` became **xui**, a cross-platform toolkit with a backend-agnostic front
layer. There is no compatibility shim; this is a hard rename (see the
[epic](https://github.com/va1erian/xui/issues/1)). The `win32ui` crate is now
`xui-win32`, and `xui` is the umbrella crate applications depend on.

## Cargo.toml

```toml
# before
[dependencies]
win32ui = "0.1"

# after — the portable front layer plus a backend, chosen by feature
[dependencies]
xui = "0.1"                     # win32 is the default feature
```

If you used the low-level platform layer (raw `Window`/`Message`/
`WindowHandler`), depend on the backend directly:

```toml
[dependencies]
xui-win32 = "0.1"
```

## Imports

| before | after |
|---|---|
| `use win32ui::prelude::*;` | `use xui::prelude::*;` |
| `use win32ui::{column, row};` | `use xui::{column, row};` |
| `win32ui::run_app(…)` | `xui::run_app(…)` |
| `win32ui::Button`, `win32ui::ListView`, … | same names under `xui::` |

The layout macros keep working: `xui::column![…]`, `xui::row![…]`,
`xui::split_row![…]`, `xui::tabs![…]`, or import them explicitly.

Low-level code that named the platform layer (`xui_win32::Window`,
`xui_win32::Message`, `xui_win32::WindowHandler`, `xui_win32::gdi`, …) now uses
`xui_win32` (or `xui::xui_win32`). That layer is intentionally Win32-specific.

## Two upgrade paths

`xui-win32` contains both a Win32-native widget layer and the Win32
implementation of the portable `Backend`. You can take either path.

### A. Stay on the Win32-native layer (smallest change)

With the umbrella's default `win32` feature, the bare names
(`xui::run_app`, `xui::Ui`, `xui::Label`, `xui::column!`) are the **same**
Win32-native API you had, so a `win32ui` app is mostly a find-and-replace of the
crate name. This is the right choice for a Windows-only app that relies on the
native controls and the window features.

### B. Move to the portable widget layer (cross-platform)

For code that should also build on Linux and macOS, use `xui-core`'s portable
widgets and pass a backend to `xui_core::run_app`. The differences to expect:

- **Widgets are constructed with a `Rect`, not added to a layout tree.**
  `Label::new(ui, rect, text)`, `Button::new(ui, rect, text)`, … There is no
  `ui.set_layout(column![…])` in the portable layer; a container (`Panel`,
  `ScrollView`, `Split`, `Tabs`) owns its children, and `Dock`/`Stack` are
  available for manual arithmetic.
- **Events map to `Msg` the same way** — `on_click`, `on_select`, `on_change`,
  `on_toggle`, and `App::update` is never re-entered.
- **Some widgets differ slightly.** `TreeView`/`ListView`/`GridView` are
  model-driven; `ComboBox`/`RadioGroup` return indices; `Dialog` is an in-window
  modal. See [Widgets](widgets.md).
- **The window entry point changes.** `xui_win32::run_app(WindowSpec, make)`
  becomes `xui_core::run_app(backend, PlatformSpec, make)`. See
  [Getting started](getting-started.md) and [Backends](backends.md).

You can mix paths at the crate level: keep a `xui_win32` dependency for the
Windows layer and add `xui-core` for portable widgets, but a single *window* uses
one runtime.

## Small API notes

- `Theme::system()` still works, but it is now an extension-trait method
  (`SystemTheme`) provided by the Win32 backend. With `use xui::prelude::*;`
  nothing changes; code that called it without the prelude must add
  `use xui::SystemTheme;`.
- Window class names in the Win32 layer are unchanged.
- `Backdrop`, `TitleBar::Extended`, the strip menu, the material status/top
  bars, monitor/placement/fullscreen and WGC capture remain Win32-specific; the
  portable `PlatformSpec` carries only `Backdrop::{Opaque, Acrylic, Mica}`,
  `Decorations` and `caption_inset`. See
  [Windows-only window features](win32-windows.md).

## What is portable

`xui-core` is the backend-agnostic front layer, shared by every backend:

- geometry, units and colour (`Point`/`Size`/`Rect`, `Dip`/`Px`, `Color`);
- the pure layout arithmetic (`Dock`, `Stack`, `Anchor`, `Insets`);
- the semantic `Theme` tokens and the `Themed` trait;
- the input vocabulary (`Key`, `Modifiers`, `MouseButton`, `HitTest`);
- the accessibility model (`Role`, `Action`, `RangeValue`, `Node`);
- the **widget layer**, the `App`/`Ui` runtime and the `Backend` contract.

`xui-win32` implements the `Backend` contract (and hosts native controls);
`xui-canvas` implements it cross-platform. See
[Architecture](architecture.md) and [Backends](backends.md).
