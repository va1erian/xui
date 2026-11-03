# xui-win32

The Windows backend for [xui](../xui): `Win32Backend`, the Win32 implementation of
[xui-core](../xui-core)'s `Backend` contract. It runs the portable widgets with
native window chrome, Direct2D/DirectWrite painting (GDI fallback), and a real
native `EDIT` for text entry. It also contains the low-level platform layer it is
built on (`Window`, `WindowHandler`, `Hwnd`, `gdi`, `d2d`), reachable for interop.
There is no widget API of its own.

Most applications depend on the `xui` umbrella crate instead (feature `d2d`). See
the [workspace README](../../README.md),
[The Win32 layer](../../docs/win32.md) and
[Windows-only window features](../../docs/win32-windows.md).
