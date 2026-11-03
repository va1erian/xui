# xui-gpu

The shared OpenGL seam between xui's backends.

`xui-win32` creates its context through WGL and `xui-canvas` through `glutin`,
but both drive the same lifecycle: resize the framebuffer, track DPI, begin a
frame (make the context current, set the viewport, clear), present it, and — for
a GL widget composited into a software surface — render into a texture and read
it back. This crate owns that lifecycle in terms of an opaque
[`GlContext`](src/lib.rs) handle, so each backend only supplies the platform
context creation.

`glow` lives here rather than in `xui-core`, which stays free of GPU
dependencies.
