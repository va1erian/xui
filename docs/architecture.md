# Architecture

xui is split so that as little as possible knows about a platform. This chapter
describes the portable front layer, the contract every backend implements, how a
widget event becomes an `App::update` call, and the invariants that keep the
seams clean.

## The layers

```text
application
   │  owns widget values, implements App::update(&mut self, Msg, &mut Ui)
   ▼
xui-core  (portable; #![forbid(unsafe_code)], no platform dependency)
   │  widgets · App/Ui runtime · router · theme · units · layout arithmetic
   │  accessibility model · Backend trait · WindowId/WidgetId · Event/Canvas
   ▼
a backend (xui-win32 · xui-canvas · xui-canvas::OffscreenBackend)
   │  decodes native input → Event, creates nodes, paints, shapes text
   ▼
the platform (Win32 · winit + softbuffer/tiny-skia · a headless surface)
```

- **`xui-core`** is the only crate an application *needs* to depend on. It has
  no platform dependency and no `unsafe`, so it compiles everywhere.
- **A backend** is one value implementing [`xui_core::backend::Backend`]. It
  owns the event loop, creates windows and nodes, delivers events, paints and
  measures text.
- The application never names a platform handle; it refers to a window by
  `WindowId` and a widget by `WidgetId`, both assigned by the backend.

### The two widget layers

`xui-win32` contains **two** stacks, and it matters which one you are using.

| | Portable widget layer | Win32-native widget layer |
|---|---|---|
| Lives in | `xui-core` (`widget`, `app`, `backend`) | `xui-win32` (`app`, `controls`, `window`, `gdi`, `d2d`, `gl`) |
| Runs on | every backend | Win32 only |
| Entry point | `xui_core::run_app(backend, PlatformSpec, make)` | `xui_win32::run_app(WindowSpec, make)` |
| Hosts widgets as | painted child windows (native `EDIT` for `Edit`) | native common controls (`ListView`, `TreeView`, …) |
| Layout | widgets are positioned explicitly; containers arrange their children | `column!`/`row!`/`tabs!` layout tree |
| Best for | code that should build and run on every platform | Windows-only apps that want the deepest native integration |

The `xui` umbrella crate selects between them:

- With the default `win32` feature **on Windows**, `pub use xui_win32::*`
  re-exports the **Win32-native** names, so `xui::run_app`, `xui::Ui`,
  `xui::Label`, `xui::column!` are the native layer. The portable layer stays
  reachable as `xui::xui_core` (and `xui::Win32Backend`).
- With `canvas` (or on a target where `xui-win32` is empty),
  `pub use xui_core::*` makes the umbrella's bare names the **portable**
  widgets.

When you write cross-platform code, depend on `xui-core` directly (or use the
umbrella with `--no-default-features --features canvas`) and pass a backend to
`xui_core::run_app`. See [Backends](backends.md) and
[Getting started](getting-started.md).

## The `Backend` contract

`xui_core::backend::Backend` is object-safe: the runtime holds one value behind
`Rc<dyn Backend>` and you can choose it at run time. Its operations group into:

- **Lifecycle** — `init` (idempotent, default no-op), `run` (pump until `quit`),
  `run_with` (build the app once the window is live, then pump; see below),
  `quit`, `wake`/`waker` (a `Send + Sync` cross-thread wake), `set_event_sink`.
- **Windows** — `open_window`, `close_window`, `set_window_title`,
  `set_window_enabled` (make a modal owner inert), `native_window`,
  `capture`, `run_modal`, `minimize`, `toggle_maximize`, `is_maximized`,
  `caption_inset`.
- **Nodes** — `create`/`destroy` (destroy cascades), `apply_moves` (batched so a
  relayout does not flicker), `set_visible`/`set_enabled`, `raise`,
  `set_drag_region`, `set_cursor`, `set_clip`, `set_capture`/`release_capture`,
  `focus`, `set_text`/`text`, `bounds`, `invalidate`/`invalidate_rect`,
  `set_painter`.
- **Text** — `measure_text`, `text_shaper`/`layout_text` (a `Send + Sync` shaper
  a worker can use), `dpi`, `client_rect`.
- **Runtime** — `set_theme`, `set_timer`/`kill_timer`, and
  `supports(NodeKind) -> ImplKind`.

Methods a backend cannot meaningfully provide have defaults: `capture` and
`run_modal` return `BackendError::Unsupported`, `native_window` returns `None`,
and the rest are no-ops. This is why the portable core can run on a minimal
backend (the offscreen one overrides `run` to return immediately) without
special-casing it.

`run_app` builds the app through `run_with`: the runtime installs its event sink
first, then the backend invokes the `make` closure once its platform window
exists and its DPI is known, and only then pumps the loop. A backend that
creates its window synchronously (Win32) builds the app before pumping; a
backend whose window is created lazily by the loop (`winit`) builds it from
inside the loop, so widgets lay out at the real scale factor from the start
instead of being built at 96 DPI and rescaled afterwards. The default
implementation calls `on_ready` and then `run`.

`NodeKind` is the portable widget vocabulary (`Label`, `Button`, `Edit`,
`ListView`, `ScrollView`, `Panel`, `Custom`, …). `supports` tells the front
layer whether the backend provides a real control for a kind (`ImplKind::Native`)
or expects the front layer to paint it (`ImplKind::Painted`). On Win32 today
only `Edit` is native; `WinitBackend` and `OffscreenBackend` report `Painted`
for everything.

### Identity

- `WindowId` and `WidgetId` are opaque, backend-assigned `u64` handles
  (`NONE`, `from_raw`, `raw`, `is_none`). They are never platform handles.
- `NodeId` is different: it is the app-defined `usize` key of a tree row, not a
  backend id.
- `ParentRef` says what a node is parented to — the window, or a container node
  (`Panel`, `ScrollView`, `Tabs`).

## Events, routing and the message model

A backend decodes native input into a portable `Event` (`MouseDown`,
`MouseMove`, `KeyDown`, `Char`, `TextChanged`, `Resize`, `Paint`, `Timer`,
`DpiChanged`, `Close`, `Wake`, …) and delivers it to the `WidgetId` it targets.
Window-level events that belong to no widget target `WidgetId::NONE`.

The flow, end to end:

1. The backend calls the installed `WidgetHost` sink with `(target, event)`.
2. `Runtime::deliver` handles `WidgetId::NONE` window events first — `Wake`
   collects worker-thread messages and drains the queue, `Close` consults the
   close mapper, `Timer`/`DpiChanged`/`DisplayChange` consult theirs — then
   routes everything else through the `Router`.
3. The router invokes the closures a widget registered for its node. Those
   closures map the event to the application's own `Msg` (`on_click`,
   `on_select`, `on_change`, …) and enqueue it.
4. Enqueuing on the empty→non-empty edge calls `Backend::wake`, which the
   backend turns back into `Event::Wake`.
5. The runtime drains the queue into `App::update(&mut self, msg, ui)`.

**`App::update` is never re-entered.** The app is borrowed for the whole call; a
message raised *while* `update` runs (a widget feedback, a send from inside
`update`) is queued and delivered after it returns. This is what makes native
callbacks — which run synchronously inside a `SendMessage`, a modal menu loop or
a paint — safe to bridge without `Rc<RefCell<…>>` panics. The design is the
Elm/relm4 shape applied to retained widgets.

### Worker threads

`Ui::proxy()` returns a `Proxy<Msg>` that is `Send + Sync` when `Msg: Send`.
A worker calls `proxy.send(msg)`; a burst of sends coalesces into a single wake.
After the window closes the send fails, so a worker cannot queue messages nobody
will drain.

### Secondary and modal windows

`Ui::open_window` opens a non-modal child window with its own `App`/`Msg` and
returns a `WindowHandle` (`send`, `close`, `set_title`, `native`, `capture`).
`Ui::open_modal` disables the opener, blocks on `Backend::run_modal`, and
returns the value the child passed to `Ui::close_with_result`.

## Layout

`xui-core`'s layout module is **pure arithmetic, not an engine**: `Insets`,
`Anchor`/`anchored`, `Dock`/`DockLayout` (carve fixed strips) and
`Stack`/`StackSlot` (tile a row or column, largest-remainder rounding). Every
value is resolved to device pixels once, from `Dip` through the window's DPI.

Portable widgets are **positioned explicitly**: each takes a `Rect` at
construction, in device pixels. Container widgets own their children and arrange
them:

- `Panel` scopes child creation through `panel.ui()`; the caller (or a `Stack`)
  positions them in the panel's own coordinates.
- `Split` lays two `WidgetId` panes out with a draggable divider.
- `Tabs` docks a strip and shows only the selected page's children.
- `ScrollView` stacks registered rows, clips descendants and scrolls.
- `Dialog`, `Menu`, `ComboBox` and `Tooltip` share the popup elevation helper.

The widget layer uses no `set_layout` call and no layout tree of its own; if you
want the declarative `column!`/`row!` tree, that is the Win32-native layer (see
[The Win32 layer](win32.md)).

## Units, colour, theme

- **Units are types.** `Dip` is a design value; `Px` is a device pixel. `Rect`,
  `Point` and `Size` are device pixels. A backend converts `Dip` to `Px` once at
  the boundary, so DPI scaling cannot be applied twice.
- **`Color`** is an opaque RGB value with `ColorRef` conversions, `lerp` and a
  WCAG `luminance`/`contrast_ratio`.
- **`Theme`** is a small semantic palette (`background`, `surface`, `text`,
  `accent`, `selection`, `border`, …) with `light()`/`dark()` constructors.
  Widgets read the window's live theme; see [Theming](theming.md).

## Accessibility

`xui_core::accessibility` is a plain model — `Role`, `Action`, `RangeValue` and
a `Node` tree — with no backend hook. A backend that can expose it (Win32 UI
Automation) bridges the tree to the platform; a painted widget describes itself
through the same model.

## Source layout

```text
crates/xui-core/src/
  lib.rs            public surface + prelude
  backend/          the Backend trait and its vocabulary
    mod.rs          Backend, Painter, Waker, BackendError, Result
    ids.rs          WindowId / WidgetId
    node.rs         NodeKind, NodeSpec, ParentRef, ImplKind
    event.rs        Event, TimerId
    canvas.rs       the Canvas draw trait and text styles
    paint.rs        Rgba, gradients, strokes, corners
    text.rs         FontSpec, TextLayout, TextShaper
    spec.rs         PlatformSpec, Decorations, Backdrop
    native.rs       NativeWindowHandle
    cursor.rs       Cursor
    headless/       a test-only recording backend (#[cfg(test)])
  app/              App, Ui, run_app, Proxy, WindowHandle
  router.rs         maps an Event to the widget owning a node
  widget/           the portable widgets (one module per widget)
  layout.rs         Dock/Stack/Anchor/Insets arithmetic
  geometry.rs       Point / Size / Rect (device pixels)
  units.rs          Dip / Px
  color.rs          Color
  theme/            Theme tokens and the Themed trait
  message/          Key, Modifiers, MouseButton, HitTest
  accessibility/    Role, Action, RangeValue, Node
  image.rs          portable RGBA images and PNG/JPEG decode
  property.rs       the Properties/Property/Value surface

crates/xui-win32/src/     the Win32 backend + the Win32-native widget layer
crates/xui-canvas/src/    WinitBackend, SkiaCanvas, OffscreenBackend, GL seam
crates/xui-gpu/src/       the shared OpenGL surface seam (no platform code)
crates/xui-litehtml/src/  the litehtml HTML view (Windows)
crates/xui/src/           the umbrella crate
```

## Invariants

These are non-negotiable and are part of `AGENTS.md`.

- **`unsafe` only in a `sys/` module.** Every other module starts with
  `#![forbid(unsafe_code)]`. Each `unsafe` block carries a `// SAFETY:` comment.
- **No platform types in the public API.** A platform handle may be a private
  field; callers only see xui types (`Hwnd` and `NativeWindowHandle` are opaque
  wrappers, not raw handles).
- **Widgets own their nodes and destroy them in `Drop`.**
- **Small files.** Aim under 300 lines, hard limit 400; split along a real seam
  before crossing it.
- **Platform constants come from the platform binding or the SDK header**, never
  from memory.

## Related

- [Backends](backends.md) — the available implementations and how to combine
  them.
- [Development](development.md) — the checks and how to add a widget.
- [The Win32 layer](win32.md) — the native controls and Windows-only features.
