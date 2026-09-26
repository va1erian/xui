# xui-canvas

The cross-platform, software-rendered backend for [xui](../xui). It draws the
portable widgets with [tiny-skia](https://github.com/linebender/tiny-skia) into
an RGBA buffer, so it runs (and can be snapshot-tested) without a platform UI
toolkit.

A `WinitBackend` window can also hand its client area to a GPU renderer. Install
a `GlWidget` with `WinitBackend::set_gl_content(window, widget)` and the backend
presents `glow` OpenGL frames through a `GlSurface` (a `glutin` context on the
winit window) instead of the software copy:

```rust
impl GlWidget for Visualizer {
    fn paint(&self, canvas: &mut dyn Canvas, bounds: Rect, theme: &Theme) {
        // software fallback: runs when no GL context can be created
    }
    fn paint_gl(&self, gl: &glow::Context, bounds: Rect, theme: &Theme) {
        // issue GL calls; the backend presents the frame
    }
    fn gl_teardown(&self, gl: &glow::Context) {
        // free GPU resources while the context is current
    }
}
```

The `unsafe` GL context creation and the `glow` loader live in `src/sys/gl/`;
every other module forbids `unsafe`.

## Constraints

- **GL content takes over the window.** It must be the window's sole content:
  CPU nodes are not composited into a GL frame. (Lifting the seam into
  `xui-core` and compositing CPU nodes as a texture is a follow-up.)
- **Fallback.** When no GL display/config/context can be created (a headless
  session, no driver) the backend paints `GlWidget::paint` on the software
  surface for good, so the window stays usable. The offscreen backend never has
  a GL context and always uses this fallback.
- **Platforms.** `glutin`'s default backends (EGL/GLX/X11/Wayland/WGL) are
  dlopen-based, so the Linux cross-check needs no system graphics headers beyond
  what winit already needs. Context requests are core-profile 3.3.

`cargo run -p xui-canvas --example gl` opens a window with an animated,
GPU-rendered triangle; set `XUI_DEMO_AUTOCLOSE_MS` to have it quit itself.

See the [workspace README](../../README.md) and the
[epic](https://github.com/va1erian/xui/issues/1).
