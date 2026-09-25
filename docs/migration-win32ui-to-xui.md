# Migrating from `win32ui` to `xui`

`win32ui` is being turned into **xui**, a cross-platform toolkit with a
backend-agnostic front layer. The old crate is now `xui-win32` (the Win32
backend), and the umbrella crate is `xui`. There is no compatibility shim; this
is a hard rename (see the [epic](https://github.com/va1erian/xui/issues/1)).

This document covers the move for an existing `win32ui` app. It is updated as
the migration proceeds; the widget API itself is unchanged so far.

## Cargo.toml

```toml
# before
[dependencies]
win32ui = "0.1"

# after
[dependencies]
xui = { version = "0.1", features = ["win32"] }   # win32 is the default
```

If you also used the low-level platform layer (raw `Window`/`Message`/
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
`xui_win32` (or `xui::xui_win32` from the umbrella). That layer is intentionally
not portable and will stay Win32-specific.

## Small API notes

- `Theme::system()` still works, but it is now an extension-trait method
  (`SystemTheme`) provided by the Win32 backend. With `use xui::prelude::*;`
  nothing changes; code that called it without the prelude must add
  `use xui::SystemTheme;`.
- Window class names (`win32ui.*`) are unchanged for now.

## What is portable today

The front layer is being decoupled in stages. As of the workspace split these
are already in `xui-core` and shared by every backend:

- `Point`/`Size`/`Rect`, `Dip`/`Px`, `Color`
- the pure layout arithmetic (`Dock`, `Stack`, `Insets`)
- the semantic `Theme` tokens and the `Themed` trait
- the input vocabulary (`Key`, `Modifiers`, `MouseButton`, `HitTest`)
- the accessibility tree model (`Node`, `Role`, `Action`)

The widget layer (windows, controls, menus, the widget message model) is still
`xui-win32` only. It moves to the shared front layer in the later milestones;
until then, an app written against `xui` runs on the Win32 backend.

## Windows-only features

`Backdrop`, `TitleBar::Extended`, the strip menu, the material status/top bars,
monitor and placement APIs, and WGC capture remain Win32-specific. They will
move behind a `Win32Ext` extension trait with defined fallbacks so `xui-core`
stays free of them.
