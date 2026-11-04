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
  geometry, gradients, layer clips, bitmaps (premultiplied, cached by image id within a 96 MiB budget per window, oldest evicted first).
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
| size-optimised exe | 2,798,592 B (2.67 MiB) | **501,760 B (0.48 MiB)** |
| default `--release` exe | 4,531,712 B | **929,792 B** |
| crates in the dependency tree | 105 | `xui-core` (14) plus `windows` |
| backend source (no tests) | ~8.6k lines | ~4.1k lines |

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
- **`capture`, pointer lock, window icon, `GlWidget`.**
- **Damage tracking is Windows' update region only.** The canvas backend's
  scene damage list is not reused, so every `WM_PAINT` replans all nodes.
  Sharing `scene.rs`, `compositor.rs` and the damage code needs them pulled out
  of `xui-canvas` (they have no `tiny-skia` use, except that the compositor
  draws into its `Surface`).
- **Per-draw allocations in the paint path (avoidable, deferred):** each `draw_text`
  builds a DirectWrite layout (plus a UTF-16 `Vec` and a format-cache key); each
  gradient fill creates a stop collection and a brush; `fill_polygon` and
  non-uniform rounded rectangles build a path geometry; each rounded clip creates
  a layer. A layout cache keyed on text, font and box, and cached layers and
  geometries, would remove them. `AGENTS.md` asks that these be called out; they
  are here and in the PR description.
- **Text metrics differ from `cosmic-text`**, so layouts differ by a pixel or
  two between backends. Headless snapshots still use the software backend.
- Not measured: frame time. No light/dark screenshot pair beyond one light
  capture of the cookbook (the dark theme path is the same `Themed` code).

## Verified

`cargo fmt --all --check`, `cargo check --all-targets` and `cargo clippy --all-targets -- -D warnings` are clean. The 34 tests of `xui-d2d` pass in Windows Sandbox with the default parallelism: 18 unit tests (scene, input order, capture loss, Tab, dialog filters), 5 DirectWrite tests (no window), 4 clipboard tests, 4 native file-dialog tests, and 3 real-window tests (paints and timers, a modal window, and the pixel test of the reach clip and of a leaked clip, which reads the screen and is `#[ignore]`d so a headless CI runner skips it; run it in the sandbox with `-TestArgs --include-ignored`). The cookbook example and the miniscent editor rendered correctly there. The rest of the workspace ran in the sandbox too: the only failing binaries (`deps`, `cookbook_doc`, `examples_use_layouts`, `files`, `cli`, `icons` of `xui-core`) read source files by path and fail there for want of a checkout; they pass on the host, and none involves this crate.

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

## Review

An independent review (a fresh agent, given the diff and the claims to verify, not
the author's verdict) found one Critical issue and several Major ones; all were
fixed: a possible double free of the window handler when `CreateWindowExW` fails
after `WM_NCCREATE`; a doctest that did not compile; a mouse capture Windows
could take away without the widget being told (`WM_CAPTURECHANGED` is now
reported); an unbounded clipboard read and leaked clipboard blocks; a file filter
with no extensions (now `*.*`) and a default extension applied to an open; and
missing tests (now: pointer, focus, capture and Tab order; capture loss; clipboard;
modal loop; a pixel test of the reach clip and of a leaked clip; a layout used from
another thread). Running the clipboard test in parallel found one more real bug the review had not: `OpenClipboard(NULL)` lets several threads open the clipboard at once, so one thread's `EmptyClipboard` freed data another was reading (heap corruption). It now opens with a hidden message-only owner window per thread. Also fixed from the Minor list: an unbalanced layer pop, the key
repeat count, the first baseline of wrapped text, windows destroyed by Windows
itself, the primary-window flag, and a missing-ancestor clip.

Still open, by design: the per-draw allocations above, a test that a missing file
is refused when it must exist (the refusal is the Common Item Dialog's own modal
message box, which a helper thread cannot dismiss reliably), a test of the image
cache budget (needs a Direct2D target without a window), `WM_SYSCHAR` (unverified
beeping on Alt+letter), and a panic in app code unwinding through the window
procedure, which aborts.
