# xui-win32

The Win32 layer for [xui](../xui). It contains two things:

- `Win32Backend`, the Win32 implementation of [xui-core](../xui-core)'s
  `Backend` contract, which runs the portable widgets with painted child windows
  and a real native `EDIT` for text entry;
- a mature **Win32-native widget layer** (native common controls, GDI/Direct2D/
  OpenGL, backdrop materials, the extended title bar, the strip menu, material
  bars, monitors, capture and UI Automation) for Windows-only applications.

Most cross-platform applications depend on the `xui` umbrella crate instead. See
the [workspace README](../../README.md),
[The Win32 layer](../../docs/win32.md) and
[Windows-only window features](../../docs/win32-windows.md).
