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
a backend (xui-canvas · xui-win32 · xui-canvas::OffscreenBackend)
   │  decodes native input → Event, creates nodes, paints, shapes text
   ▼
the platform (winit + softbuffer/tiny-skia · Win32 · a headless surface)
```

- **`xui-core`** is the only crate an application *needs* to depend on. It has
  no platform dependency and no `unsafe`, so it compiles everywhere. It is also
  the only widget layer: every backend runs the same portable widgets, and the
  `xui` umbrella crate's bare names are always this layer.
- **A backend** is one value implementing [`xui_core::backend::Backend`]. It
  owns the event loop, creates windows and nodes, delivers events, paints and
  measures text. `xui-canvas` (default, every platform) and `xui-win32`
  (opt-in `d2d` feature, Windows-only, Direct2D-accelerated) are peers — pick
  one by feature or construct either at runtime and pass it to
  `xui_core::run_app`. See [Backends](backends.md).
- The application never names a platform handle; it refers to a window by
  `WindowId` and a widget by `WidgetId`, both assigned by the backend.

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

Below the declarative layer, a widget's node is placed by a `Rect` in device
pixels; only layouts (and, inside `xui-core`, composite widgets) do that. The
rect constructors are crate-private, so apps describe layouts.
Container widgets own their children and arrange them:

- `Panel` holds a layout in its own coordinates (`panel(layout)`).
- `Split` lays two pane layouts out with a draggable divider (`split(a, b)`).
- `Tabs` docks a strip and shows only the selected page's layout.
- `ScrollView` measures its layout at its width, clips descendants and
  scrolls (`scroll(layout)`).
- `Dialog`, `Menu`, `ComboBox` and `Tooltip` share the popup elevation helper.

On top of that arithmetic, `xui_core::arrange` is how apps lay windows out:
nest `row()`/`column()`/`grid()`/`wrap()`/`overlay()`/`stack()` layouts of
widget **builders**, wrap a layout in `scroll(..)`, `group(title, ..)` or
`tabs().page(..)`, then `ui.root(layout)` creates the widgets and keeps them
placed. `absolute()` is the one container for free positions (entries at
`.at(x, y, w, h)` with an `.anchor(..)`): the designer surface and forms
imported from coordinates. The pieces:

- `layout::Group`/`Item` are a pure tree (leaves are opaque keys) with rows,
  columns, grids (`Track::Auto`/`Fixed`/`Fill` columns, spans), wraps,
  layered groups and absolute groups, per-item `Align` and max sizes, and
  framed items (a leaf with a group inside its content insets). Leaves are
  measured through a callback with `Constraints` (a column bounds its
  children's width, so wrapping content reports its height for that width),
  so the tree is testable with no backend. A nested group takes its natural
  size unless it is sized to fill. Rows, columns, grids and wraps keep their
  items disjoint; layered and absolute groups overlap on purpose; no
  container places an item outside itself (property-tested in
  `xui-core/tests/layout_containers.rs`). `Group::trace` returns every node
  with its rectangle and warnings.
- `widget::Placeable` is the capability a layout needs from a widget: its
  node, `measure(ui, constraints)` from its text and the design tokens, the
  `content_insets` of a frame (`GroupBox`), a `placed` hook for satellite
  nodes and containers (`Tabs` re-lays its pages there, `ScrollView` its
  content), and the `layout_text` a report prints.
- `arrange::Build` describes a widget; it is created at mount time with the
  `Ui` of the container the layout is mounted in, so builders take no `ui` and
  no `Rect`. `Handle<W>` is the app's typed reference to a widget it changes
  later (`bind` fills it at mount). Events map to `Msg` by value
  (`on_click(Msg::Ok)`, `on_change(Msg::Name)`), so `Msg: Clone` is the norm.
- `Ui::root` keeps the mounted layout for the window's lifetime (the runtime
  releases it when the window goes); `Ui::mount` returns a `Mounted` to drop
  for content that is replaced; `Ui::mount_in(container, layout)` places a
  layout inside a container node. A frame and its content are one unit:
  hiding a bound `group` collapses its content too.
- `app(title)...run(make)` (and the umbrella's `xui::app`, which also picks the
  backend and handles `XUI_SNAPSHOT`) opens the window, applies the backend's
  `system_theme`, builds the app and honours `XUI_DEMO_AUTOCLOSE_MS`.

**Relayout is automatic.** Window resizes and DPI changes re-flow at once. A
change that can alter a natural size (a widget's text, icon or title, its
visibility, a list's items, the theme) only marks the window's layouts dirty
(`Ui::invalidate_layout`, which custom widgets call too); the runtime runs one
layout pass after it has handled the event that caused it, before the paint
the change scheduled, and `Ui::capture` flushes it first. `Ui::relayout()`
stays for reading new bounds within the same handler. `Ui::layout_report()`
prints the trees, rectangles and warnings (clipping, zero size, truncated
text, overlap in disjoint containers) for an agent or a test to read instead
of a screenshot; `Snapshot::with_layout_overlay()` outlines the same
rectangles on a headless capture.

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

crates/xui-canvas/src/    WinitBackend, SkiaCanvas, OffscreenBackend, GL seam
crates/xui-win32/src/     Win32Backend: Direct2D/DirectWrite painting (Windows)
crates/xui-gpu/src/       the shared OpenGL surface seam (no platform code)
crates/xui-code-editor/src/  the code-editor widget (a portable custom-painted node)
crates/xui-icons/src/     the optional Global Village icon set (vector shapes drawn through Canvas)
crates/xui-litehtml/src/  the litehtml HTML view (a portable custom-painted node)
crates/xui-blitz/src/     the Blitz web view (a portable custom-painted node; replaces litehtml and NetSurf)
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
- [The Win32 layer](win32.md) — the low-level platform layer `Win32Backend` is
  built on, for interop.
