# Running the canvas gallery on macOS

On macOS the only backend compiled is the cross-platform software backend
`xui-canvas`: a `winit` window, a `softbuffer` surface and `tiny-skia` for
rasterising the portable `xui-core` widgets. The Win32 backend is
`#![cfg(windows)]`, so the gallery's "native" renderer silently resolves to the
canvas backend there (`backend_for` in `examples/widgets.rs`).

## Toolchain

Rust dependencies are pure Rust (winit uses `objc2`, no Homebrew or Xcode
frameworks are needed beyond the system SDK). Install:

```bash
xcode-select --install          # system linker and SDK
rustup toolchain install stable # any recent stable
```

No extra Rust targets are required for a native build; for an Apple-silicon
cross-check from another host, `rustup target add aarch64-apple-darwin` is
enough to `cargo check` (no linker).

## Build and run the gallery

```bash
cargo run -p xui --features canvas --example widgets
```

`XUI_BACKEND=canvas` selects the software backend; on macOS it is already the
only backend, so the variable is optional:

```bash
XUI_BACKEND=canvas cargo run -p xui --features canvas --example widgets
```

To have it quit on its own (handy for a smoke run), set
`XUI_DEMO_AUTOCLOSE_MS`:

```bash
XUI_DEMO_AUTOCLOSE_MS=4000 cargo run -p xui --features canvas --example widgets
```

Other environment variables the gallery reads: `XUI_GALLERY_THEME=dark` starts
dark, `XUI_GALLERY_MENU` opens the menu bar, `XUI_GALLERY_CONTEXT` opens the
context menu. `XUI_AUTOSWITCH_MS` (the Renderer button) is Windows-only: the
gallery can only switch backends where both are compiled in.

## Lifecycle

The gallery installs no close handler, so clicking the window's red close
button delivers `CloseRequested`; the runtime closes the window and the event
loop exits. `winit` installs the standard application menu (About, Hide,
Quit); **Cmd-Q** quits the process.

## Retina and backing scale

`WinitBackend` derives its dots-per-inch from the window's `scale_factor`:
`round(scale_factor * 96)`, so a Retina display reports 192 DPI and the
software surface is the window's physical (backing) pixel size.

`run_app` builds the widgets before the real window exists, so they are first
laid out at the default 96 DPI. As soon as the window reports its real scale
factor the backend lifts every node's bounds to the backing pixels (and does
so again if the window moves between monitors of different scale), so the
layout fills the window and text is rasterised at the display scale rather than
upscaled.

The residual ordering issue — a widget that captures `ui.dpi()` at
construction time for its own internal measurements still sees 96 while its
canvas paints at the real DPI — is tracked as
[#58](https://github.com/va1erian/xui/issues/58). The clean fix is to create
the window before the app is built, which needs a change in `xui-core`'s
`run_app`, not in this backend.

## Continuous integration

The `cross-check-macos` job builds the workspace, runs the headless core and
canvas tests, and checks the gallery:

```bash
cargo check -p xui --features canvas --example widgets
```
