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
cargo test --all-features
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

`.github/workflows/ci.yml` runs six jobs:

| Job | Runner | What it does |
|---|---|---|
| `check` | windows-latest | fmt, check, clippy and test on the default features; the `canvas` feature and `xui-canvas` tests; the GL test (`XUI_CANVAS_GL_TEST=1`, skips cleanly with no driver); `cargo test --all-features`; and a smoke run of the `widgets`, `gl`, `demo` and `umbrella` examples with an auto-close |
| `check-wgc` | windows-latest | the capture feature: check, clippy and test with `--features wgc` |
| `cross-check` | ubuntu-latest | `cargo check --all-targets`, clippy on `xui-core`, and `xui-core`/`xui-canvas` tests — proving the front layer has no Win32 dependency and the empty backends compile |
| `cross-check-macos` | macos-latest | check, `xui-core`/`xui-canvas` tests, and the canvas gallery example |
| `coverage` | windows-latest | `cargo llvm-cov --workspace --all-features`, uploading LCOV to Codecov and as a `coverage-lcov` artifact |
| `snapshots` | ubuntu-latest | `scripts/snapshots.sh`: every example rendered headlessly in light and dark, uploaded as the `snapshots` artifact (not a required check) |

## Coverage

Coverage is tracked with
[`cargo-llvm-cov`](https://github.com/taiki-e/cargo-llvm-cov), which instruments
the crates and runs the suite. The `coverage` job uploads `lcov.info` to
[Codecov](https://codecov.io/gh/va1erian/xui) and keeps it as an artifact.
Locally:

```powershell
rustup component add llvm-tools-preview
cargo install cargo-llvm-cov
scripts\coverage.ps1          # writes target\coverage\lcov.info + html\
```

`codecov.yml` keeps the project and patch statuses *informational* for now, so
coverage never blocks a PR; tighten them once the numbers are stable.

## Property tests

`crates/xui-core/tests/properties.rs` uses
[`proptest`](https://github.com/proptest-rs/proptest) to check the invariants the
pure arithmetic promises — half-open rectangles, rigid offsets, non-inverting
insets, exact docking and stacks, anchors that stay inside their parent. They
need no window, so they are fast and deterministic. Add a property whenever you
add a pure function with a stated invariant.

## Widget snapshots

`crates/xui-canvas/tests/widget_snapshots.rs` builds one of every portable
widget in an offscreen window, renders it light and dark, and asserts the
gallery painted on each theme and that the two images differ. It writes
`target/ui/widgets-{light,dark}.png` for eyeballing a suspected visual change.

## Headless screenshots

`scripts/snapshots.ps1` (or `scripts/snapshots.sh`) runs every `control_*`
example and the larger demos on the offscreen backend and writes light and dark
PNGs to `target/snapshots/`, with no window and no sandbox. CI uploads them as
the `snapshots` artifact. See [headless-screenshots.md](headless-screenshots.md),
which also covers the `xui_canvas::snapshot` API for tests in your own app.

## Mutation testing

`cargo-mutants` finds tests that do not really assert. It is scoped to
`xui-core` (`.cargo/mutants.toml`) so it never opens a window:

```powershell
cargo install cargo-mutants
scripts\mutants.ps1
```

Treat a surviving mutant as a prompt to strengthen a test, not a per-PR gate.

## Examples as smoke tests

Every example is compiled by `cargo check --all-targets`. The `check` job also
*runs* the interactive ones with an auto-close
(`XUI_DEMO_AUTOCLOSE_MS` / `WIN32UI_DEMO_AUTOCLOSE_MS`) so a panic on startup or
a paint regression fails CI. When you add an example, give it the same
auto-close hook — the per-control demos under `crates/xui/examples/controls/`
are the smallest form.

## Adding a portable widget

The portable widget layer is what most new controls should target, because it
runs on every backend. A widget:

1. Lives in `crates/xui-core/src/widget/<name>.rs` and is re-exported from
   `widget/mod.rs` and `lib.rs`.
2. Holds a `Control<M>` (which owns its node), creates one or more nodes through
   `Ui`, registers a painter and an event mapper, and destructs its nodes in
   `Drop`. A widget that animates itself starts its own timer with
   `Control::set_timer(millis, on_tick)`; the control stops it on drop.
3. Maps events to the app's `Msg` through closures fixed at construction
   (`on_click`, `on_select`, …) — never a numeric control id.
4. Paints only from `Theme` tokens, reading the live theme; owner-draw any part
   a backend leaves to the front layer.
5. Implements `HasText` and/or `Properties` where they fit.
6. Is exercised by the cross-backend gallery
   (`crates/xui/examples/widgets.rs`), gets a per-control demo under
   `crates/xui/examples/controls/`, and appears in the offscreen light/dark
   snapshot (`crates/xui-canvas/tests/widget_snapshots.rs`). Add a `proptest`
   property for any pure helper it introduces, and attach one light and one
   dark screenshot to a UI change.

Keep `xui-core` free of platform dependencies and `unsafe`; a new `NodeKind` may
need adding to `backend/node.rs` if the backend must be aware of it.

## File size

Aim under 300 lines per file, hard limit 400. Split along a real seam — a new
`sys/<name>.rs` for a backend's raw helpers, separate paint/state/model modules,
etc. — before crossing the limit.

## Screenshots

For portable widgets, render them headlessly first
([headless-screenshots.md](headless-screenshots.md)). For a real window, never raise, activate, foreground a window or move the pointer to screenshot on a
shared desktop. Use the composited capture (`--features wgc`):

```text
cargo run -p xui-win32 --features wgc --example capture -- --title "My App" --out shot.png
```

See [Windows-only window features → Capture](win32-windows.md#capture) and
[sandbox-testing.md](sandbox-testing.md).
