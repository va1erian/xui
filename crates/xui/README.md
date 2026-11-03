# xui

Cross-platform UI for Rust: small, fast, idiomatic and properly themed.

`xui` is the umbrella crate. It re-exports the portable widget layer from
[xui-core](../xui-core) as its bare names, so an application can write
`use xui::prelude::*;` and never name a backend, and it selects a backend by
feature: `canvas` (default, the cross-platform software backend) or `d2d` (the
Windows Direct2D backend, [xui-win32](../xui-win32)). See the
[workspace README](../../README.md) and [Getting started](../../docs/getting-started.md).
