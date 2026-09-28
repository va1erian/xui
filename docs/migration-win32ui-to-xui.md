# Migrating from `win32ui` to `xui`

`win32ui` became **xui**, a cross-platform toolkit with a backend-agnostic front
layer. There was no compatibility shim; this was a hard rename (see the
[epic](https://github.com/va1erian/xui/issues/1)).

## The Win32-native widget layer has been removed

The original `win32ui` API — native common controls (`ListView`, `TreeView`,
`ComboBox`, …), the `column!`/`row!`/`tabs!` layout tree, `xui_win32::run_app(
WindowSpec, make)` and the Windows-only chrome built on them (strip menu,
material status/top bars, `TitleBar::Extended`) — was deleted in
[#173](https://github.com/va1erian/xui/pull/173) (epic
[#161](https://github.com/va1erian/xui/issues/161)). The last commit that still
has it is tagged `pre-win32ui-controls-removal`.

What remains is one widget layer, the portable one in `xui-core`, running on any
backend:

```toml
[dependencies]
xui = "0.1"                        # the portable widgets + the canvas backend
xui = { version = "0.1", features = ["d2d"] }   # also the Windows Direct2D backend
```

`xui-win32` is now only a `Backend` implementation (`Win32Backend`) plus the
low-level platform layer it is built on.

## Moving app code to the portable layer

- **Widgets are constructed with a `Rect`, not added to a layout tree.**
  `Label::new(ui, rect, text)`, `Button::new(ui, rect, text)`, … A container
  (`Panel`, `ScrollView`, `Split`, `Tabs`) owns its children, and `Dock`/`Stack`
  are available for manual arithmetic.
- **Events map to `Msg` the same way** — `on_click`, `on_select`, `on_change`,
  `on_toggle`, and `App::update` is never re-entered.
- **Some widgets differ slightly.** `TreeView`/`ListView`/`GridView` are
  model-driven; `ComboBox`/`RadioGroup` return indices; `Dialog` is an in-window
  modal. See [Widgets](widgets.md).
- **The window entry point changes.** `xui_win32::run_app(WindowSpec, make)`
  becomes `xui_core::run_app(backend, PlatformSpec, make)`. See
  [Getting started](getting-started.md) and [Backends](backends.md).

## Not carried over

- Windows UI Automation for controls (the portable `Win32Backend` has none yet,
  issue [#169](https://github.com/va1erian/xui/issues/169)).
- The strip menu and the material status/top bars; the portable `TopBar` and
  `MaterialStatusBar` widgets are the replacements.
- `xui-litehtml` is out of the workspace until it is ported
  ([#168](https://github.com/va1erian/xui/issues/168)).
