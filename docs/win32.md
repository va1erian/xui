# The Win32 layer

`xui-win32` is more than a backend. It contains a **mature Win32-native widget
layer** — the original crate, kept for Windows-only apps that want the deepest
integration — plus the lower platform layer that both this layer and the
portable `Win32Backend` sit on.

If you write cross-platform code, use the portable widgets and
[`Win32Backend`](backends.md). Read this chapter when you are building a Windows
application and want native common controls, the declarative layout tree, or the
Windows window features.

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
  DpiChanged, KeyDown, Char, Mouse*, Command, Notify, DrawItem, …}` replace raw
  `(u32, WPARAM, LPARAM)` triples.
- **RAII GDI.** `gdi::{Font, Brush, Pen, Bitmap}` and a double-buffered
  `gdi::Paint` / `Canvas`, so there is no manual `DeleteObject`.
- **Raw hook.** `WindowHandler::raw_message` (and `Ui::on_raw_message`) sees
  every message before it is decoded; the pointer is valid only for the call.
- **`Hwnd`.** An opaque, `Copy` handle (`raw()`, `is_alive()`) for OS interop,
  with no `windows` type in the public API.
- **`looper`.** `run`, `quit`, `run_modal`, and `xui_win32::init()` (per-monitor
  v2 DPI + common-control class registration, idempotent).
- `unsafe` is confined to `src/sys/`; every other module forbids it.

## The native widget layer

`xui_win32::run_app(WindowSpec, make)` builds a window and calls `make` to build
the app. Widgets own native controls and map their events to your `Msg`; the
same never-re-entered `App::update` model applies.

```rust
use xui_win32::prelude::*;

enum Msg { Search(String), Open(usize), Delete }

struct MailWindow { list: ListView<Row>, search: Edit }

impl App for MailWindow {
    type Msg = Msg;
    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Search(q) => self.list.set_model(filter(&q)),
            Msg::Open(i) => { /* ... */ }
            Msg::Delete => { /* ... */ }
        }
    }
}

let _ = xui_win32::run_app(WindowSpec::new("Mail"), |ui| {
    let search = Edit::single_line(ui).cue("Search")
        .on_change(|t| Some(Msg::Search(t.into())));
    let list = ListView::<Row>::new(ui)
        .column("From", dip(180.0), |r| r.from.as_str())
        .column("Subject", Fill, |r| r.subject.as_str())
        .on_activate(|i| Some(Msg::Open(i)));
    ui.set_layout(column![search, list.fill(1)]);
    MailWindow { list, search }
});
```

The layout macros share names with std prelude macros, so import them
explicitly: `use xui_win32::{column, row};`.

### Design choices

- **Events map to your `Msg`.** A widget gets a small mapping closure at
  construction; messages are queued and delivered to `App::update`, which is
  never re-entered. Win32 calls back synchronously (a `SendMessage` inside a
  handler, a modal menu loop), so closure-heavy toolkits need `Rc<RefCell<…>>`
  state that panics under re-entry. Here a message raised while `update` runs is
  delivered after it returns.
- **Shared behaviour comes from traits, not inheritance.** Every widget holds a
  `Control`; `AsControl` plus a blanket `ControlExt` give every widget
  `set_enabled`, `set_visible`, `focus`, `set_tooltip`, and `set_font`.
  Capability traits (`HasText`, `Themed`) say what a widget *can do*. There is no
  base type, no `Deref` chain and no downcasting.
- **No control ids.** Widgets are values you hold; notifications are routed
  internally by child `HWND`.
- **Layout is a tree the window owns.** `column!`/`row!` with `fill`/`width`
  re-lay out on resize and DPI change, and the moves are batched with
  `DeferWindowPos` so a resize does not flicker. The app never handles `WM_SIZE`.
- **Typed data, not strings and indices.** `ListView<T>` has typed columns over
  a `ListModel`; `TreeView<K>` has a keyed lazy model; `ComboBox<T>` and
  `RadioGroup<T>` return values, not indices.
- **Units are types.** `Dip` for design values, `Px` for device pixels.
- **Worker threads talk to the UI through `Proxy<Msg>`**, `Send + Sync` with
  coalesced wake-ups.

### Layout tree

- `Layout::column()`, `Layout::row()`, `Layout::free(origin)`; `.spacing(Dip)`,
  `.margins(Insets)`, `.item(…)`.
- Sizing via `LayoutExt`: `.fill(n)`, `.fixed(Dip)`, `.width(…)`, `.height(…)`,
  `.min(…)`, `.anchor(Anchor)`.
- Macros: `column!`, `row!`, `split_row!`, `split_col!`, `tabs!`.
- Nodes: `Split`, `Tabs`, `Panel`, `MaterialStatusBar`, `MaterialTopBar`,
  `Fluent` (fluent spacing helper).

### Native controls

`Button`, `CheckBox`, `ColorPicker`, `ComboBox`, `Edit`, `FlowText`, `GroupBox`,
`Label`, `ListView`, `Menu`, `Panel`, `ProgressBar`, `RadioGroup`, `ScrollView`,
`StatusBar`, `TaskDialog`, `Toolbar`, `TreeView`, `GridView`, plus the layout
hosts. They are real common controls, so **accessibility, IME and system
behaviour come for free**, with the theming and owner-draw handling described
below and in [Theming](theming.md).

Decoding plumbing (owner-data requests, custom draw, lazy tree expansion) is
handled by the crate; the app sees only per-control event enums
(`ListViewEvent`, `TreeViewEvent`, …).

### Custom widgets

For a control the layer does not provide, implement `CustomWidget` and host it in
a `Custom`. It can pick a paint path — `Renderer::Gdi` (default),
`Renderer::Direct2D`, or `Renderer::Gl` (OpenGL via `glow`, re-exported as
`xui_win32::glow`) — and declare its accessibility via the portable model.

## How the native layer dispatches messages

`Window::create` boxes the caller's `WindowHandler` into a thin
`*mut Box<dyn WindowHandler>` and stashes it in `GWLP_USERDATA` on `WM_NCCREATE`;
the shared `window_proc` (`sys::dispatch`) decodes each raw message into a typed
`Message` and calls the handler. The handler is shared (`&self`), so a
synchronous message that arrives while the handler is already on the stack — a
`ListView::select()` notification, a `WM_SIZE` from a call inside the handler, a
modal loop — is still delivered rather than dropped; mutable state lives in
`Cell`/`RefCell` fields. The box is reclaimed once, on `WM_NCDESTROY`; if the
window was destroyed from inside its own handler, the free is deferred until the
outermost dispatch for that window returns.

Common controls send their "self-contained" notifications (`LVN_GETDISPINFO`,
`NM_CUSTOMDRAW`, `TVN_ITEMEXPANDING`, …) to their **parent**, not themselves.
`controls::registry` keeps a thread-local map from child `HWND` to the Rust state
that wants first refusal on those messages; `window_proc` offers each `WM_NOTIFY`
to the registry before decoding it for the application. This is why the app never
sees owner-data or custom-draw plumbing — only `ListViewEvent`/`TreeViewEvent`s.

The ListView is a real owner-drawn virtual list: `LVS_OWNERDATA` with cell text
via `LVN_GETDISPINFO`, and the whole row painted in `NM_CUSTOMDRAW` (selection
highlight, optional zebra background, app-supplied `row_style`/`row_painter`
overrides, column separators). Row height uses the documented "1×height image
list" trick (`sys::listview::lv_set_row_height`); `LVS_OWNERDRAWFIXED` +
`WM_MEASUREITEM` is an incompatible pair that virtual lists never receive
`WM_MEASUREITEM` for. Its header is a separate child control, so the ListView is
subclassed (`sys::control::HeaderSubclass`) to intercept the header's
`NM_CUSTOMDRAW` and paint it dark too.

## Fonts

Every control uses one UI font: the system message font
(`SystemParametersInfoForDpi(SPI_GETNONCLIENTMETRICS).lfMessageFont`, Segoe UI
9 pt on stock Windows), scaled to the window's DPI. `gdi::Font::system_ui`
builds it; the widget layer shares one instance per DPI so no control can hold a
deleted `HFONT`.

The policy is enforced centrally rather than per control:

- every control gets the shared font at creation;
- `sys::apply_native_theme` restores a control's font after `SetWindowTheme`
  (comctl32 otherwise resets it to the stock font);
- `WM_DPICHANGED` moves every control still on a shared font to the new DPI;
- DirectWrite resolves `system-ui` to the same face.

Override one control with `ControlExt::set_font`; a font set that way is kept
across theming and DPI changes. `tests/fonts.rs` audits one of every control with
`WM_GETFONT`.

## Theming on Win32

Controls theme themselves; the app never handles `NM_CUSTOMDRAW`,
`WM_CTLCOLOR*` or `SetWindowTheme`.

- The window stores its `Theme` and answers `WM_CTLCOLOREDIT/STATIC/BTN/LISTBOX/
  DLG` with cached brushes. `Window::set_theme`/`Ui::set_theme` also sets the DWM
  dark title bar (`DWMWA_USE_IMMERSIVE_DARK_MODE`), updates the class background
  (no white flashes) and re-themes every registered child.
- `sys/theme.rs` applies the documented `DarkMode_Explorer`/`DarkMode_CFD`
  visual styles per control kind, plus dark scrollbars.
- Where a native part ignores dark mode (list header, status bar, tabs,
  tooltips), the crate owner-draws it from theme tokens. Only documented APIs
  are used — no `uxtheme` ordinals.
- `Theme::system()` / `is_theme_change` read and track the user's preference
  (see [Theming](theming.md)).

## Interop with the portable layer

The Win32 backend exposes the handles behind its windows and nodes for
interop — `Win32Backend::window_hwnd(window)` and
`Win32Backend::node_hwnd(widget)` — and the portable `Ui::native_window()`
returns an opaque `NativeWindowHandle`. An OS integration can therefore be
written against either layer without naming a `windows` type. See
[Platform integration](platform-integration.md).

## Related

- [Windows-only window features](win32-windows.md) — backdrop, title bar, strip
  menu, material bars, monitors, capture and UI Automation.
- [Theming](theming.md) — the shared token set.
- [Architecture → The two widget layers](architecture.md#the-two-widget-layers)
  — when to use this layer versus the portable widgets.
- [Platform integration](platform-integration.md) — the escape hatches
  (`on_raw_message`, `Hwnd`, `Proxy`).
