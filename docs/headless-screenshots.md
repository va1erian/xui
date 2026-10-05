# Headless screenshots

`xui_canvas::snapshot` renders an app's window to an `Image` with the software
renderer: no window, no display, no GPU and no Windows Sandbox. It works on
Windows, Linux and macOS, so it runs in `cargo test` and on CI.

It shows what **xui paints**. Native window chrome, DWM materials and the
Win32 backend's own rendering are not part of it; see
[Headless versus real windows](#headless-versus-real-windows).

## Rendering a window in a test

Give `render` the closure you would give `run_app`:

```rust
use xui_canvas::snapshot::{Snapshot, render};
use xui_core::{Dip, Theme};

let image = render(
    Snapshot::new(Dip(720.0), Dip(300.0)).theme(Theme::dark()).dpi(144),
    |ui| MyApp::build(ui), // returns your `App`
)?;
image.save_png("target/snapshots/toolbar-dark.png")?;
```

- `Snapshot` sets the size in design units, the theme (light by default), the
  DPI (96 by default; the image is `dpi / 96` times larger) and the title.
- The capture happens after the build closure has returned and the runtime has
  drained the messages queued while building, so a layout driven by those
  messages has settled.
- `save_png` needs the directory to exist; `Image::encode_png` gives the bytes.
- `try_render` takes a build closure that returns `Result<App, BackendError>`
  (widget constructors are fallible) and returns the error as
  `SnapshotError::Backend`. A zero, negative, non-finite or over-large size
  (more than 8192 px a side) is `SnapshotError::Size`. Nothing panics.

To snapshot a single widget, build an app whose only content is that widget in
a small window.

### Hover, click and open states

`render_with` adds a step closure that runs after the app is built and before
the capture. Its `Stage` sends messages and injects input; every call is
processed (widgets update, messages reach `update`) before it returns:

```rust
use xui_canvas::snapshot::render_with;

let image = render_with(
    Snapshot::new(Dip(320.0), Dip(200.0)),
    |ui| Ok(MyApp::build(ui)?),
    |stage| {
        stage.hover(40, 36);          // client pixels
        stage.click(30, 78);          // left press + release
        stage.emit(Msg::OpenMenu);    // straight to `update`
    },
)?;
```

Positions are device pixels. The step must be `'static`; share results with the
test through `Rc<Cell<_>>` or `Rc<RefCell<_>>`.

### Determinism

Two renders of the same app in one process, and across runs on one machine, are
byte-identical: the offscreen backend has no timers, so nothing blinks or
animates, and each render uses a fresh backend, so no theme, DPI or window is
carried over.

Text is shaped from the machine's installed fonts (xui does not bundle any), so
the bytes can differ between machines that have different fonts. That is why
golden images should be compared with a tolerance, as below.

## Checking visuals without golden files

Assert on pixels instead of comparing whole images. These helpers are plain
functions over `Image::pixel`:

```rust
use xui_core::image::Image;

/// Whether any pixel inside `rect` (x, y, width, height) differs from `background`.
fn painted_inside(image: &Image, (x, y, w, h): (u32, u32, u32, u32), background: [u8; 4]) -> bool {
    (y..y + h).any(|row| (x..x + w).any(|col| image.pixel(col, row) != Some(background)))
}

/// Whether every pixel outside `rect` is `background` (nothing leaked).
fn clean_outside(image: &Image, (x, y, w, h): (u32, u32, u32, u32), background: [u8; 4]) -> bool {
    (0..image.height()).all(|row| {
        (0..image.width()).all(|col| {
            let inside = (x..x + w).contains(&col) && (y..y + h).contains(&row);
            inside || image.pixel(col, row) == Some(background)
        })
    })
}

let light = render(Snapshot::new(Dip(200.0), Dip(80.0)), build)?;
let bg = light.pixel(199, 79).unwrap(); // the window background
assert!(painted_inside(&light, (20, 20, 120, 32), bg), "the button painted");
assert!(clean_outside(&light, (20, 20, 120, 32), bg), "nothing outside it");
```

Also useful: render light and dark and assert the two differ, or render a state
with `render_with` and assert it differs from the resting state.

## Golden images with a tolerance

When a picture is the clearest assertion, keep a PNG in the repository and
compare with a tolerance, so font differences between machines do not fail the
test:

```rust
fn matches_golden(image: &Image, golden: &Image, per_channel: u8, max_bad_fraction: f32) -> bool {
    if image.size() != golden.size() {
        return false;
    }
    let bad = image
        .pixels()
        .chunks_exact(4)
        .zip(golden.pixels().chunks_exact(4))
        .filter(|(a, b)| a.iter().zip(*b).any(|(x, y)| x.abs_diff(*y) > per_channel))
        .count();
    (bad as f32) <= max_bad_fraction * (image.width() * image.height()) as f32
}
```

Load goldens with `Image::decode(&std::fs::read(path)?)`. Regenerate them with
`image.save_png(path)` when a visual change is intended (for example behind an
`UPDATE_GOLDENS=1` environment variable in your test), look at the new images,
and commit them with the change that caused them. Prefer pixel assertions for
anything that must hold; goldens are for "does this still look right".

## Using it from another app: LazyRAD

Any app with an `App` type and a `build(ui)` can snapshot itself. LazyRAD has
`LazyRad` (its `App`) and `LazyRad::build(ui)`. Add `xui-canvas` as a
dev-dependency and write `tests/snapshots.rs`:

```rust
use xui_canvas::snapshot::{Snapshot, try_render};
use xui_core::{Dip, Theme};

fn snapshot(name: &str, theme: Theme, width: f32, height: f32) {
    // An app's build usually returns `Result<_, BackendError>` because widget
    // constructors can fail, so use `try_render` rather than `render`.
    let image = try_render(Snapshot::new(Dip(width), Dip(height)).theme(theme), |ui| {
        LazyRad::build(ui)
    })
    .expect("render");
    std::fs::create_dir_all("target/snapshots").unwrap();
    image.save_png(format!("target/snapshots/{name}.png")).unwrap();
}

#[test]
fn ide_window_light_and_dark() {
    snapshot("ide-light", Theme::light(), 1280.0, 800.0);
    snapshot("ide-dark", Theme::dark(), 1280.0, 800.0);
}
```

Add a second test that opens a sample project (send its "open" message with
`render_with`) and renders it at another size. The tests run under plain
`cargo test`, on any OS. Upload the folder from CI:

```yaml
- run: cargo test --test snapshots
- uses: actions/upload-artifact@v4
  with:
    name: snapshots
    path: target/snapshots/
```

If building the app can fail, return the `Result` and use `try_render`.

## Attaching images to a PR

UI changes in xui attach a light and a dark screenshot ([AGENTS.md](../AGENTS.md)).
Headless images are enough for portable widgets. For the whole gallery run
`scripts/snapshots.ps1` (Windows) or `scripts/snapshots.sh`: every
`control_*` example and the larger demos (`widgets`, `listview`, `gridview`,
`top_bar`, `layout`, `containers`, `absolute`) are rendered into `target/snapshots/` as
`<example>-light.png` and `<example>-dark.png`. CI runs the same script and
uploads the folder as the `snapshots` artifact of every PR. Drag the images
that show your change into the PR description.

A demo joins the gallery by starting with `xui::app(..).run(..)`:
`XUI_SNAPSHOT=<dir>` saves the light and dark images and exits, and
`XUI_BACKEND=offscreen` runs headless without saving. `xui_canvas::snapshot::Gallery` parses the variables. With
neither set, a demo behaves exactly as before.

## Headless versus real windows

Headless snapshots use the software renderer, so they do **not** show native
window chrome, caption buttons, DWM backdrop materials, native menus or dialogs,
or anything only the Win32 (Direct2D) backend draws. For those, capture a real
window: `Windows.Graphics.Capture` through the `capture` example or
`Window::capture_composited`, run inside Windows Sandbox as described in
[sandbox-testing.md](sandbox-testing.md). Use headless for "what does this
widget look like, and did it clip"; use a real window for "does it look right
in the OS".
