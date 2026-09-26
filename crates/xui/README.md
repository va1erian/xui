# xui

Cross-platform UI for Rust: small, fast, idiomatic and properly themed.

`xui` is the umbrella crate. It selects a backend by feature (`win32` by
default, `canvas` for the cross-platform software backend) and re-exports the
shared front layer from [xui-core](../xui-core), so an application can write
`use xui::prelude::*;` and never name a backend.

On Windows with the default `win32` feature the bare names are the
[Win32-native widget layer](../xui-win32); the portable front layer is always
available as `xui::xui_core`. See the
[workspace README](../../README.md) and [Getting started](../../docs/getting-started.md).
