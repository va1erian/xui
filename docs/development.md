# Development

How the workspace is built, tested and extended. The non-negotiable rules live
in [AGENTS.md](../AGENTS.md); the design invariants are repeated in
[Architecture](architecture.md#invariants).

## Toolchain

- Stable Rust, edition 2024. There is no pinned `rust-toolchain` file and no
  declared MSRV; CI uses `stable`.
- The workspace members include `xui-core`, `xui-win32`, `xui-canvas`,
  `xui`, `xui-litehtml`, `xui-blitz`, `xui-code-editor` (`Cargo.toml`, resolver 3).

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
| `lint` | windows-latest | `cargo fmt --all --check`, then clippy with `-D warnings` on `--all-targets` and again with `--all-features` (the d2d, canvas and wgc backends) |
| `test` | windows-latest | `cargo test`; `xui-code-editor`'s tests with the `rhai-syntax` feature; the opt-in GL test (`XUI_CANVAS_GL_TEST=1`, skips cleanly with no driver); and a run of the `widgets` example with an auto-close. The all-features test pass runs in `coverage` |
| `cross-check` | ubuntu-latest | `cargo check --all-targets`, clippy on `xui-core`, the `xui-core`, `xui-code-editor` (`rhai-syntax`) and `xui-canvas` tests — proving the front layer has no Win32 dependency and the empty backends compile — and a `xui-blitz` check for `x86_64-unknown-linux-musl` |
| `cross-check-macos` | macos-latest | `cargo check --all-targets`, the `xui-core` and `xui-canvas` tests, and a check of the canvas `widgets` example |
| `snapshots` | ubuntu-latest | `scripts/snapshots.sh`: every example rendered headlessly in light and dark, uploaded as the `snapshots` artifact (not a required check) |
| `coverage` | windows-latest | `cargo llvm-cov --workspace --all-features`, uploading LCOV to Codecov and as a `coverage-lcov` artifact |

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

`crates/xui-canvas/tests/damage_differential/` and `damage_widgets.rs` are the
differential tests of the canvas backend's damage-driven repaint: two offscreen
backends get the same random (seeded) operations or the same scripted session,
one repaints from damage only and the other repaints the whole window before
each frame, and every frame must be equal pixel for pixel, on several themes and
at several DPIs. `XUI_DAMAGE_SEEDS`
(default 25) and `XUI_REGION_SEEDS` (default 96, for the region repaint of
random curved scenes in `region_tests.rs`) raise the number of seeds for a
longer run; a failure names the seed and the step. See
[Repainting only what changed](backends.md#repainting-only-what-changed).

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

Every example is compiled by `cargo check --all-targets`. The `test` job also
*runs* the `widgets` example with an auto-close (`XUI_DEMO_AUTOCLOSE_MS`;
`WIN32UI_DEMO_AUTOCLOSE_MS` for the Win32 ones) so a panic on startup or a
paint regression fails CI, and `snapshots` renders every example headlessly.
When you add an example, give it the same auto-close hook — the per-control
demos under `crates/xui/examples/controls/` are the smallest form.

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
   a backend leaves to the front layer. Paint inside the node's bounds, or
   within the overdraw margin around them (8 design units): the canvas
   backends clip every painter to that reach, so a shadow or a label that
   overflows needs bounds that cover it. A stroke is centred on its
   rectangle's edge, so a frame stroked under a `push_clip` of that same
   rectangle loses its outer half: stroke the frame outside the clip. Invalidate
   the node (`Control::invalidate`) whenever a change alters how it looks,
   including a focus change: the canvas backends repaint only what was marked.
5. Implements `Placeable` so layouts can hold it: `measure(ui, constraints)`
   returns its natural size from its content and the theme's design values
   (never its current bounds, or a widget squeezed once stays small), reading
   `constraints.max_width` when its height depends on its width; `placed`
   re-lays any satellite node (a scrollbar) after the layout moves it, and
   `layout_text` returns the text of a widget sized by its text. Add a
   builder function in `arrange` (`radio_group(..)`, `toolbar()`) with
   by-value message mapping (`.on_select(Msg::Pick)`), and a test that
   mounts it in a layout. A setter that changes the natural size calls
   `Control::invalidate_layout`.
6. Implements `HasText` and/or `Properties` where they fit.
7. Is exercised by the cross-backend gallery
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
