# xui-explorer

A small, **spatial** file explorer built on xui's portable widget layer: one
window is one open folder, with no tree, address bar or in-window navigation.
It is written as a portable core with every OS-specific operation behind two
small traits, so it can be embedded by an operating system with its own
filesystem and shell — in particular **LazyOS**.

- `xui-core` only (no platform dependency, no `unsafe`).
- Folders open their own window (or reuse the one already showing them); files
  go to the OS default handler.
- `IconView` listing (folders first, then files), a `StatusBar` summary, a
  context `Menu`, Delete/Properties `TaskDialog`/`Dialog` actions and
  `Delete` / `Alt+Enter` / `F5` shortcuts.

## The seam

Everything OS-specific is in [`src/platform.rs`](src/platform.rs):

```rust
trait Platform {
    fn list(&self, dir: &Path) -> io::Result<Vec<RawEntry>>;
    fn metadata(&self, path: &Path) -> io::Result<Meta>;
    fn remove(&self, path: &Path, recursive: bool) -> io::Result<()>;
    fn home(&self) -> Option<PathBuf>;
}

trait Launcher {
    fn open(&self, path: &Path) -> io::Result<()>;
}
```

The trait signatures mention only `Path`/`PathBuf`, `OsString`, `io::Result`
and `SystemTime` — no `xui`, no `std::fs`, no `cfg`. The rest of the crate is
portable:

- `model/` is pure logic: sorting, `Listing`, the `IconModel` view, status
  summaries, the Properties rows, size and UTC time formatting and path
  helpers. It assumes nothing about drive letters or separators.
- `window.rs` is the per-window `App`: it owns the widgets and reacts to their
  messages; it never touches the filesystem directly.
- `shell.rs` holds the `Platform`, the `Launcher` and the path-to-window
  registry that makes an already-open folder a no-op and lets a delete close
  the windows below it.

`testing::MemPlatform` is an in-memory `Platform` used by the tests, so no test
touches the real disk.

## Porting to LazyOS

Implement three things and nothing else:

1. **`Platform`** over LazyOS' filesystem. `metadata` must be symlink-aware (use
   the equivalent of `symlink_metadata`) and `remove` must delete a symlink as a
   link; `home` may return `None`.
2. **`Launcher`**, or a no-op if LazyOS has no default handler yet. The explorer
   reports a launcher error in the status bar, so a no-op that returns
   `io::ErrorKind::Unsupported` is honest.
3. **A `xui_core::backend::Backend`** (LazyOS' own), and start the app with
   `xui_core::run_app`.

Then build windows with `Explorer::new(platform, launcher).open_root(ui, path)`
and the portable crate is unchanged. The `std-platform` feature (on by default)
is the only part that uses `std::fs` and `#[cfg(...)]`; turn it off for a
no-std-ish target.

## Known limits (v1)

- **No filesystem watching.** Refresh happens on `F5`, the context menu, and
  after the explorer's own delete. A change made outside the explorer shows only
  after a refresh.
- **Listing runs on the UI thread.** A very large or slow directory blocks the
  window while it loads.
- **No raise/focus API.** Opening a folder that is already open reports
  `"already open"` in the status bar instead of bringing that window forward.
- Out of scope: rename, copy/move, drag and drop, new folder, recycle bin,
  hidden-file toggle, sorting options, search, navigating up.

## Checks

```bash
cargo fmt --all --check
cargo clippy -p xui-explorer --all-targets -- -D warnings
cargo test -p xui-explorer
```

The snapshot test writes `target/snapshots/xui-explorer-{light,dark}.png`
headlessly, with no window.
