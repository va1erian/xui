# Agent conventions

Read this and [README.md](README.md) before writing any code. The docs'
[Architecture](docs/architecture.md) invariants and
[Development](docs/development.md) ("Adding a portable widget") sections are
part of these rules.

## Invariants (non-negotiable)

- **`unsafe` only in `src/sys/`.** Every other module starts with
  `#![forbid(unsafe_code)]`. Each `unsafe` block carries a `// SAFETY:` comment
  explaining why its contract holds.
- **No `windows` types in the public API.** They may be private fields; callers
  only ever see xui types.
- **Controls own their child `HWND`** and destroy it (and unregister from
  `controls::registry`) in `Drop`.
- **Small files.** Aim under 300 lines, hard limit 500. If a change would push a
  file past that, split it along a real seam first. New raw-Win32 helpers for a
  new control go in their own `src/sys/<name>.rs` rather than growing
  `sys/control.rs`.
- **Win32 constants come from the `windows` crate or the SDK header**, never from
  memory. If you must mirror one as a literal (as `gdi/paint.rs` does for `DT_*`),
  say which header it comes from.

## Quality bar

- Idiomatic Rust: builders over long argument lists, enums over raw codes,
  RAII over manual cleanup. Match the existing module's naming and style.
- No allocation or GDI-object creation per item in paint/notification hot paths
  (`NM_CUSTOMDRAW`, `LVN_GETDISPINFO`, `WM_PAINT`) unless unavoidable; say so if it is.
- Public items get doc comments. Comments elsewhere only explain the non-obvious *why*.
- No dead code, no commented-out code. Nothing emusic-specific in new public API:
  this is a general-purpose library.
- Every new control or message is exercised in `examples/demo/` and has a
  test in `tests/` or a unit test.
- **Widgets follow the widget layer** (docs → *Architecture*): events map to
  the app's `Msg` through closures given at construction, there are no numeric
  control ids in the public API, shared behaviour comes from capability traits
  (never a base type, `Deref` or downcasting), and design values are `Dip`.
  Widgets live in `xui-core`; backends (`xui-canvas`, `xui-win32`) implement
  the `Backend` contract and expose no widget API of their own.
- **Dark mode is first-class**: widgets implement `Themed`, use semantic theme
  tokens only, and owner-draw any native part that ignores dark mode (documented
  APIs only). UI PRs attach one light and one dark screenshot.
- Tests that enter the message loop use a watchdog so they fail instead of
  hanging. Never create ad-hoc debug windows.

## Checks (all must pass before opening a PR)

```bash
cargo fmt --all --check
cargo check --all-targets
cargo clippy --all-targets -- -D warnings
cargo test
```

## Review (before opening a PR)

Always review before submitting a PR. Every PR is reviewed before it is opened,
whatever its base branch (stacked PRs included), except a typo-only or comment-only PR,
which needs a self-check. No review bot is part of this: do not wait for CodeRabbit
and do not request it. If a bot comments anyway, treat its comments like any other
finding.

Run the checks above first (`cargo test` on a Windows desktop goes through the sandbox
script under "Operational rules"), then review the diff that passed them. Re-review
any change made after the review, other than a fix the reviewer has re-checked.

- **Who reviews.** Not the session that wrote or planned the change: a new session
  started on the branch (for example `/code-review` in Claude Code) or the owner.
  Give the reviewer the diff, the sources it relies on, and the draft PR description's
  claims as a list to verify, not the author's verdict. Re-reading your own diff is a
  self-check, not a review.
- **What it covers.** The whole diff against the sources it relies on: correctness
  and edge cases, failure paths and tests, the invariants and quality bar above, and
  every claim the PR description makes.
- **Findings.** *Critical*: data loss, a crash, a security issue, a broken invariant
  above. *Major*: wrong behaviour on realistic input, a false claim in the
  description, a missing failure-path test. Fix every Critical finding before
  opening. Fix every Major finding too, unless the owner agrees, in the PR
  description, to defer it to a linked issue. The author does not dismiss a finding
  alone: one the author believes is wrong goes back to the reviewer or the owner, with
  evidence. Minor findings may be deferred with a reason.
- **Record it.** The PR description has a `Review` section (the PR template has
  one): who reviewed, what they covered, and what changed because of it. A typo-only
  or comment-only PR may say "self-checked".
- **A review is not permission to merge.** See "Do not merge your own PR unless
  requested" below.

## Operational rules

- **Running tests on a Windows desktop:** use `scripts\sandbox\run.ps1` (see the
  `sandbox-tests` skill in `.claude/skills/` and `docs/sandbox-testing.md`). It
  runs `cargo test`, or a single test binary, inside Windows Sandbox, so windows
  and synthetic input never reach the shared desktop. Plain `cargo test` on the
  host is only acceptable in CI or where the sandbox is unavailable.
- Running the demo: always set `WIN32UI_DEMO_AUTOCLOSE_MS=4000` so it exits on
  its own, and never leave a demo process running when you finish.
- **Screenshots.** For portable widgets, take them headlessly:
  `scripts/snapshots.ps1` (or `.sh`) renders every example, light and dark, into
  `target/snapshots/`, and `xui_canvas::snapshot::render` does the same from a
  test (`docs/headless-screenshots.md`). No window opens, so no sandbox is
  needed. Use a real-window capture only for what headless cannot show: native
  chrome, DWM materials and Win32-backend rendering. For a real window, never raise, activate or foreground a window and never move
  the pointer to take a screenshot; other agents are testing on the same
  desktop. Use `cargo run --features wgc --example capture -- --hwnd/--title/
  --pid … --out shot.png` (or, in tests, `Window::capture_composited` /
  `xui_win32::capture::capture_hwnd`): `Windows.Graphics.Capture` captures any
  top-level window, including one under another, with the DWM frame, caption
  buttons and backdrop material. Prefer it over `capture_screen`, which only
  works for an unoccluded window and has been the reason windows were raised.
  `Window::capture` (`PrintWindow`) stays for a cheap capture without DWM
  chrome. Window lookup by title/pid is only for the capture tool's own opt-in
  flags on windows you started.
- A dependency's source lives under `~/.cargo/registry/src/*/<crate>-<version>/`.
  Never scan the filesystem (`find /`, `Get-ChildItem -Recurse C:\`) for it.
- Touch only the files your issue names; if you need to change a file another
  open issue owns, say so in the PR description instead of working around it.
- Do not merge your own PR unless requested.
