# Development

How the workspace is built, tested and extended. The non-negotiable rules live
in [AGENTS.md](../AGENTS.md); the design invariants are repeated in
[Architecture](architecture.md#invariants).

## Toolchain

- Stable Rust, edition 2024. There is no pinned `rust-toolchain` file and no
  declared MSRV; CI uses `stable`.
- The workspace has five members: `xui-core`, `xui-win32`, `xui-canvas`,
  `xui`, `xui-litehtml` (`Cargo.toml`, resolver 3).

## Checks

All of these must pass before a PR:

```bash
cargo fmt --all --check
cargo check --all-targets
cargo clippy --all-targets -- -D warnings
cargo test
```

Feature-specific checks:

```bash
cargo check -p xui --features canvas --all-targets
cargo clippy -p xui --features canvas --all-targets -- -D warnings
cargo test -p xui-canvas

cargo check --all-targets --features wgc
cargo clippy --all-targets --features wgc -- -D warnings
cargo test --features wgc
```

## Running UI tests safely

The integration tests create real top-level windows, move focus and send
synthetic input. On a shared desktop that steals focus and makes tests flaky, so
run them in **Windows Sandbox**:

```powershell
scripts\sandbox\run.ps1                        # every test
scripts\sandbox\run.ps1 -CargoArgs '--test','slider' -TestArgs 'drag'
```

See [sandbox-testing.md](sandbox-testing.md) and the
`sandbox-tests` skill. Plain `cargo test` on the host is only for CI or where the
sandbox is unavailable. Tests that enter a message loop install a watchdog so
they fail instead of hanging, and never raise a window or move the pointer to
take a screenshot.

## CI

`.github/workflows/ci.yml` runs four jobs:

| Job | Runner | What it does |
|---|---|---|
| `check` | windows-latest | fmt, check, clippy and test on the default features; then the `canvas` feature and `xui-canvas` tests; then the GL test (`XUI_CANVAS_GL_TEST=1`, skips cleanly with no driver) |
| `check-wgc` | windows-latest | the same with `--features wgc` |
| `cross-check` | ubuntu-latest | `cargo check --all-targets`, clippy on `xui-core`, and `xui-core`/`xui-canvas` tests — proving the front layer has no Win32 dependency and the empty backends compile |
| `cross-check-macos` | macos-latest | check, `xui-core`/`xui-canvas` tests, and the canvas gallery example |

## Adding a portable widget

The portable widget layer is what most new controls should target, because it
runs on every backend. A widget:

1. Lives in `crates/xui-core/src/widget/<name>.rs` and is re-exported from
   `widget/mod.rs` and `lib.rs`.
2. Holds a `Control<M>` (which owns its node), creates one or more nodes through
   `Ui`, registers a painter and an event mapper, and destructs its nodes in
   `Drop`.
3. Maps events to the app's `Msg` through closures fixed at construction
   (`on_click`, `on_select`, …) — never a numeric control id.
4. Paints only from `Theme` tokens, reading the live theme; owner-draw any part
   a backend leaves to the front layer.
5. Implements `HasText` and/or `Properties` where they fit.
6. Is exercised by the cross-backend gallery
   (`crates/xui/examples/widgets.rs`) and has a unit test (an offscreen
   snapshot in `xui-canvas`'s tests, or a `xui-core` unit test). A UI change
   attaches one light and one dark screenshot.

Keep `xui-core` free of platform dependencies and `unsafe`; a new `NodeKind` may
need adding to `backend/node.rs` if the backend must be aware of it.

## Adding a Win32-native control

For the native-compatible layer, follow the widget-layer rules in
[AGENTS.md](../AGENTS.md) and [The Win32 layer](win32.md):

1. Add a `sys::control` helper for the raw messages you need; keep it safe and
   document every `unsafe` block.
2. Add `controls/<name>.rs`: a struct holding a `Control`, an inner state
   implementing `registry::ControlEvents` if it needs owner-data/custom-draw, and
   a `Drop` that unregisters.
3. Decode application-level notifications into a `…Event` enum and add a
   `Notify::…` variant in `message.rs` + `sys::message::decode_notify`.
4. Map those events to the app's `Msg` with a `registry::register_app_events`
   mapper and expose builder methods.
5. Implement `Themed`, re-deriving colours with `<Control>Theme::from_theme`,
   updating native parts via `sys::apply_native_theme`, invalidating, and
   registering with `theme::register_themed`.
6. Re-export from `lib.rs` (and `prelude`), exercise it in
   `examples/demo/`, and add a test in `tests/`.

## File size

Aim under 300 lines per file, hard limit 400. Split along a real seam — a new
`sys/<name>.rs` for a control's raw helpers, separate paint/state/model modules,
etc. — before crossing the limit.

## Screenshots

Never raise, activate, foreground a window or move the pointer to screenshot on a
shared desktop. Use the composited capture (`--features wgc`):

```text
cargo run -p xui-win32 --features wgc --example capture -- --title "My App" --out shot.png
```

See [Windows-only window features → Capture](win32-windows.md#capture) and
[sandbox-testing.md](sandbox-testing.md).
