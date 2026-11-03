# A lightweight Direct2D backend (`xui-d2d`)

An investigation, with a working prototype in `crates/xui-d2d`. The question:
can `xui-core` run on Windows by borrowing only the **window**, the **message
loop and input**, and **drawing and text** from the OS (Direct2D and
DirectWrite), and keep everything else in xui, instead of handing each widget
to a native control as the removed `xui-win32` did?

**Yes.** The prototype opens one `HWND` per xui window, keeps xui's own scene,
and runs the unmodified cookbook demo on it.

## Design

- **One `HWND` per xui window.** Every widget is a painted node of a scene xui
  keeps (`backend/scene.rs`): bounds, visibility, clip, paint order, hit
  testing. `supports` answers `Painted` for every kind, as the canvas backend
  does. No widget owns a window handle, so the "controls own their `HWND`"
  machinery of the old backend disappears.
- **Input** is decoded in the window procedure (`backend/handler.rs`,
  `input.rs`, `keys.rs`). Windows' virtual-key codes are already xui's `Key`
  codes. `CS_DBLCLKS` gives `MouseDoubleClick`, `WM_CHAR` gives `Char`,
  `SetTimer` and `PostMessage` give timers and `wake`. Hover, focus, capture and
  Tab traversal follow the offscreen backend's order of events.
- **Drawing** is an `ID2D1HwndRenderTarget` at 96 DPI (one unit is one pixel),
  retaining contents so a partial repaint only draws the update rectangle. The
  `Canvas` trait maps almost one-to-one: rounded rects, ellipses, path
  geometry, gradients, layer clips, bitmaps (premultiplied, cached by image id).
  Rectangles are rounded to whole pixels as `SkiaCanvas` does.
- **Text** is DirectWrite. `DwShaper`/`DwLayout` implement `TextShaper` and
  `TextLayout` (hit testing and selection rectangles, with the UTF-8/UTF-16
  offset maps). Layouts are `Send + Sync` through a `Mutex` around the COM
  object and an `unsafe impl` in `sys`.
- **`unsafe`** lives only in `src/sys/` (1.8k lines). Every Direct2D and
  DirectWrite call in `windows` 0.62 is `unsafe`, so the whole renderer sits
  there behind a safe API; `backend/` is `forbid(unsafe_code)`.

## Size

Cookbook example, same profile for both (`opt-level=s`, LTO, one codegen unit,
strip, `panic=abort`), release, Windows x64:

| | `xui-canvas` | `xui-d2d` |
|---|---|---|
| size-optimised exe | 2,798,592 B (2.67 MiB) | **489,472 B (0.47 MiB)** |
| default `--release` exe | 4,531,712 B | **905,216 B** |
| crates in the dependency tree | 105 | `xui-core` (14) plus `windows` |
| backend source (no tests) | ~8.6k lines | ~3.8k lines |

`xui-core` plus the cookbook widgets with a stub backend is 389,632 B, so the
Direct2D backend adds about 0.1 MB, against about 2.4 MB for the canvas stack
(`winit`, `tiny-skia`, `cosmic-text`, `softbuffer`, `glow`/`glutin`, `arboard`).
The earlier projection of 0.7-0.9 MB was pessimistic. Direct2D and DirectWrite
are OS DLLs: they cost no bytes in the exe but load at run time.

## What the prototype covers, and what it does not

Covered: window, title, enable, minimize/maximize, DPI (per-monitor v2) and
`DpiChanged`, resize, mouse (buttons, double click, wheel, leave, capture),
keyboard, Tab traversal, timers, `wake`, clipboard text, system dark mode and
reduced motion, dark title bar, cursors, the full `Canvas` trait, text shaping.

Not covered (each is a follow-up, not a blocker):

- **Borderless windows, custom caption, drag regions, Mica/Acrylic**
  (`Decorations::None`, `set_drag_region`, `caption_inset`, `backdrop`): the
  window always has a system frame.
- **IME composition** (`WM_IME_*`) and **UI Automation**: CJK input and screen
  readers.
- **Native file dialog, modal loop, `capture`, pointer lock, window icon,
  `GlWidget`.** The portable file dialog already covers the first.
- **Damage tracking is Windows' update region only.** The canvas backend's
  scene damage list is not reused, so every `WM_PAINT` replans all nodes.
  Sharing `scene.rs`, `compositor.rs` and the damage code needs them pulled out
  of `xui-canvas` (they have no `tiny-skia` use, except that the compositor
  draws into its `Surface`).
- **Per-draw allocations:** each `draw_text` builds a DirectWrite layout; gradient
  brushes and non-uniform rounded-rect geometries are created per draw. A layout
  cache keyed on text, font and box would remove the first.
- **Text metrics differ from `cosmic-text`**, so layouts differ by a pixel or
  two between backends. Headless snapshots still use the software backend.
- Not measured: frame time. No light/dark screenshot pair beyond one light
  capture of the cookbook (the dark theme path is the same `Themed` code).

## Verified

`cargo fmt`, `cargo clippy --all-targets -- -D warnings` clean for `xui-d2d`;
6 scene unit tests and 3 DirectWrite tests pass on the host; the real-window
test (`tests/window.rs`: paints a node, fires timers, ends on its own timer)
passes in Windows Sandbox, as does the cookbook example, which rendered
correctly there. The rest of the workspace was not re-run.

## Recommendation

Worth pursuing as a Windows-native option next to `xui-canvas`: about a fifth of
the size, a third of the source, and no windowing dependencies. Order of work:
borderless/caption, IME, UI Automation, then extract the shared scene and damage
code so both backends use one implementation.

## Update: the miniscent editor on this backend

The miniscent level editor (`E:\repos\miniscent`, branch `d2d-backend` of its
worktree `E:\repos\miniscent-d2d`) runs on `xui-d2d` behind a `d2d` cargo
feature. In Windows Sandbox: it starts, draws its menus, toolbars, viewports
and status bar; Ctrl+S opens the **native Save dialog** (suggested `level.msl`)
and writes the file (12,893 bytes); a fresh editor's Ctrl+O opens the **native
Open dialog**, loads that file, and shows "26 brushes, 12 entities".

Backend additions for it: `Backend::file_dialog` over the Common Item Dialog,
`run_modal`, secondary windows shown once their sink is set, and a byte-bounded
image cache (the 3D view streams a new image per frame). IME was deferred.

Release size of `mscent-editor` (miniscent's own profile: `opt-level = "s"`,
fat LTO, one codegen unit):

| | canvas (default) | `d2d` |
|---|---|---|
| exe | 20,817,920 B | 18,671,616 B |
| crates in the tree | 409 | 369 |

The 2.1 MB saved is the same ~2.4 MB the cookbook showed for the winit/tiny-skia
stack; the editor itself is dominated by `wgpu`, Blitz/Stylo and its own code,
so the relative saving is 10%, not 83%.
