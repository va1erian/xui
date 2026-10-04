# Plan: developer and agent productivity (xui × LazyRAD × lazyOS)

Status: **draft** · Owners: xui, LazyRAD, lazyOS · 2026-10-04

## Goal

Make a typical lazyOS app take a few dozen lines instead of a few hundred.
The same layout should work at any size and DPI, and an agent should get from
a prompt to a verified screenshot in seconds rather than minutes. The plan
works on four levers:

1. **Less boilerplate.** Remove the code every app repeats: backend connect,
   theme, `Quit`, keep-alive fields, `unwrap` per widget.
2. **Fewer setup lines.** One declarative description of the UI, used by Rust
   apps, `.lfm` forms and Rhai alike.
3. **Layout instead of rects.** Ship a complete layout system, then remove
   `Widget::new(ui, Rect, …)`.
4. **Faster generation.** Need fewer tokens to write an app and fewer
   compiles to check it. Prefer interpreted forms and Rhai where they fit.
   Verify headlessly.

## Where we are (baseline)

| Fact | Evidence |
|---|---|
| Absolute positioning is the default. Every `::new` takes a device-pixel `Rect`. | `docs/widgets.md`; about 45 xui examples, all 36 `controls/*` demos |
| `arrange` (row/column, `Fixed`/`Min`/`Fill`) exists, but only 14 widgets implement `::auto`/`Placeable`. Natural sizes are hard-coded. | `xui-core/src/arrange/`, `widget/placeable.rs` |
| `arrange` has no alignment, max size, grid, wrap, scrolling or absolute/anchored children. It has no measure/arrange protocol and does not relayout when text changes. | `arrange/mod.rs`, `layout/tree.rs` |
| A minimal xui example is 60 LOC plus a 136-LOC `support.rs`. | `examples/controls/label.rs` |
| lazyOS has about 20 xui programs, and about 15 of them place widgets by hand. Some ignore resize; three hand-roll a `layout()` function; only Editor and Writer use `arrange`. | `xui-app/src/bin/*`, `crates/{paint,explorer}`, `web/src/layout.rs` |
| Every lazyOS bin repeats: `LazyOSBackend::connect`, serial `BIND/UP/RUN` markers, `run_themed`, a `Close` msg, `_keep` fields, `unbind`. Counter is 116 LOC for two widgets. | `xui-app/src/bin/counter.rs`, `network.rs` |
| `hidpi::rect` exists only because apps use pixel constants. | `xui-app/src/hidpi.rs` |
| LazyRAD forms are data: `.lfm` TOML with absolute `left/top/width/height` plus 12-value anchors, no layout containers. The calculator form is 159 lines for 17 controls. There are no control arrays. | `lazyrad/crates/xui-form`, `examples/calculator` |
| `xui-form` and `xui-rhai` were written to move into xui. Rhai apps need no Rust compile; F5 spawns `lazyrad-player`. | LazyRAD `PLAN.md`, `crates/xui-rhai` |
| A new lazyOS app also needs a `build.rs` switch, a `run_demo.py` flag, a launcher test and a manifest with traced permissions. | `lazyos/AGENTS.md` |
| Build cost: the xui CI job takes about 16 min, and the xui-app musl build about 5.5 min. Three repos pin xui at different revs. | `lazyos/docs/perf/xui-ci-runner-plan.md` |

## Target shape

The same small dialog in each of the three ways an app can be written.

**Rust (xui):**

```rust
use xui::prelude::*;

#[derive(Clone)]
enum Msg { Greet }

struct Hello { name: Handle<Edit<Msg>>, out: Handle<Label<Msg>> }

impl App for Hello {
    type Msg = Msg;
    fn update(&mut self, Msg::Greet: Msg, ui: &mut Ui<Msg>) {
        let text = format!("Hello, {}!", ui[&self.name].text());
        ui[&self.out].set_text(&text);
    }
}

fn main() -> xui::Result<()> {
    xui::app("Hello").size(dip(360.0), dip(160.0)).run(|ui| {
        let name = ui.handle(Edit::new().placeholder("Your name"));
        let out = ui.handle(Label::new(""));
        ui.root(column().padding(16).gap(8).children((
            row().gap(8).children((&name, Button::new("Greet").on_click(Msg::Greet))),
            &out,
        )))?;
        Ok(Hello { name, out })
    })
}
```

That is about 25 lines. The 60-line label demo today has the same shape.
Widgets are built without a `ui` or a rect, and the tree owns them. `Handle`
is a typed id, so the app holds no keep-alive fields. `on_click(Msg::Greet)`
works when `Msg: Clone`, and the closure form stays available. This is the
*only* construction path: there is no rect constructor and no `Rc` child to
fall back on.

**Form (`.lfm` v2 in RON, shared by LazyRAD and `xui::form`):**

```ron
Form(
    title: "Hello",
    size: (360, 160),
    root: Column(padding: 16, gap: 8, children: [
        Row(gap: 8, children: [
            Edit(name: "name_edit", placeholder: "Your name", fill: 1),
            Button(name: "greet_button", text: "Greet"),
        ]),
        Label(name: "result_label"),
    ]),
)
```

**Script (Rhai, unchanged binding convention):**

```rhai
fn greet_button_click() { result_label.text = `Hello, ${name_edit.text}!`; }
```

**lazyOS:** `lazyos_app::main!(Hello)` covers connect, theme, the serial
markers, close and unbind. A one-line `xui-app new <name>` generates the
manifest, the `build.rs` switch, the `run_demo` flag and the launcher test.

## Workstreams

Each item is one issue-sized PR. **[xui]**, **[rad]** and **[os]** name the
repo that owns it.

### W1: Layout engine v2 [xui]

The foundation. Everything else builds on it.

1. **Measure/arrange protocol.** Replace `Placeable::natural_size` with
   `measure(&self, ui, Constraints) -> Size` and `arrange(rect)`.
   `Constraints` carries min and max per axis. Natural sizes come from the
   theme's metrics and text measurement, not from hard-coded constants.
   `natural_size` is removed in the same PR, with no shim.
2. **Alignment and size limits.** Add `Align::{Start, Center, End, Stretch}`
   on both axes, per child and as a container default. Add `max_width` and
   `max_height`, and an `aspect` ratio.
3. **`grid()`.** Tracks are `Auto`/`Fixed`/`Fill(w)`, with row and column
   span. This is the layout forms need: label/field pairs, calculator keypads,
   settings pages.
4. **`wrap()`.** Flow layout for toolbars, chips and icon grids.
5. **`scroll()`.** A layout node that wraps `ScrollView` and measures its
   content unbounded on the scroll axis.
6. **`stack()` / `overlay()` and `absolute()`.**
   - `stack()`/`overlay()` layers children on z.
   - `absolute()` is a container whose children carry `at(x, y, w, h)` plus
     an `Anchor`. It is the one sanctioned home for free positioning: the
     designer surface, `.lfm` v1 import, and custom painters' hit regions.
7. **Every widget is placeable.** Panel, GroupBox, Tabs, Split, TreeView,
   GridView, IconView, Toolbar, TopBar, ScrollView and the HTML/code/rich-text
   views get `measure`, and the containers take a layout as content.
   `development.md` → "Adding a portable widget" gains a required step:
   implement `measure`.
8. **Automatic relayout.** Setters that change natural size (`set_text`,
   `set_items`, `set_visible`, font, theme) mark the node dirty. Relayout runs
   once per frame before paint, so `ui.relayout()` becomes unnecessary.
9. **Layout debugging for agents.**
   - `Snapshot::with_layout_overlay()` draws bounds and baselines.
   - `ui.layout_report()` returns a text dump: the tree, the rects, and
     warnings for clipping, zero size, text truncation and overlap. Overlap
     is reported only inside containers whose children must be disjoint, so
     `stack()`, `overlay()` and `absolute()` are exempt.
   - An agent can read the report instead of a screenshot. CI can assert
     "no warnings" over every example.

Constraints: each piece is its own file under `layout/` or `arrange/`, kept
under 300 lines. Each gets a proptest that children never escape the
container, and a light and a dark snapshot. Disjoint containers (row,
column, grid, wrap) also assert that children never overlap. Layered
containers (`stack()`, `overlay()`) assert z-order instead.

### W2: Construction API without `ui` or rects [xui]

1. **Widget builders.** `Button::new("OK")` returns a description. The
   widget is realised when it is mounted. This removes the `ui` argument, the
   `Rect` and the per-widget `Result`. Errors surface once, from `root`/`mount`.
2. **`Handle<W>`.** `ui.handle(builder)` returns a typed, `Copy` handle.
   Access goes through `ui[&h]` or `ui.get(&h)`. The `Ui` owns the widget, so
   `Drop` and registry rules still hold inside xui, and apps stop carrying
   `_keep` fields.
3. **Message shorthand.** `on_click(msg)` where `Msg: Clone`, alongside the
   existing closures. Add `on_change_map(Msg::Name)` for events that carry a
   value.
4. **`xui::app(title)` builder.**
   - Picks the backend: `XUI_BACKEND`, then the platform default.
   - Applies the system theme, so lazyOS's `run_themed` workaround (#542)
     is no longer needed.
   - Makes close mean quit by default.
   - Honours `XUI_DEMO_AUTOCLOSE_MS` and `XUI_SNAPSHOT` natively.
   - This folds `examples/controls/support.rs` into the library.
5. **`xui::prelude`**, with one import for the common case.
6. **Remove the old paths; don't deprecate them.** The goal is that the clean
   way is the only way, so there is no multi-release window:
   - `Rc<Widget>` children in `arrange` are replaced by handles in the same PR
     that introduces `Handle` (W2.2).
   - `Widget::new(ui, Rect, …)` and the `::auto` constructors are deleted in
     the PR that migrates the last xui example, and the lazyOS and LazyRAD
     migrations land against that xui rev. Each lands as a breaking change in
     `CHANGELOG.md`.
   - `set_bounds` becomes `pub(crate)`, used only by `absolute()` and the
     layout engine.
   - A CI check fails on `Rect::new` in `examples/` outside the `absolute()`
     demo.

### W3: One declarative form format [xui + rad]

1. **Move `xui-form` into xui** as `xui::form`, behind feature `form`. This
   is what LazyRAD designed it for. LazyRAD then depends on it.
2. **`.lfm` format 2, in RON.**
   - A nested tree of layout nodes using the W1 containers. Each node is an
     enum variant: `Button(name: "ok", text: "OK")`.
   - The form types are plain serde types in `xui::form`. The `Catalog`
     schema and the file format come from one definition, so they cannot
     drift.
   - Defaults are omitted, through `#[serde(default)]` plus
     `skip_serializing_if`.
   - The designer writes canonical, byte-stable output through a fixed
     `PrettyConfig`. Comments are not preserved across a designer save; the
     TOML writer doesn't preserve them either.
   - **Control arrays:** `Button(name: "digit", array: 10)` produces
     `digit_click(index)`. The calculator form goes from 159 to about 40
     lines and loses 10 duplicate handlers.
   - **No v1 loader at runtime.** `xui-form migrate` converts v1 TOML to v2
     RON once: anchored rects become an `absolute()` container, or a
     `grid()` when the rects line up. The LazyRAD samples and lazyOS
     projects are converted in the same PR that adds the tool.
3. **The form builds the same tree as Rust.** `form::build` emits W2
   builders. A Rust app can `form::load(include_str!("main.lfm"))` and
   attach closures by name. One schema (`Catalog`) drives LazyRAD's property
   grid, Rhai's properties and validation.
4. **`Properties::Value` widening** (LazyRAD gap G6): add Color, Font, Enum,
   Dip, Image and a property schema, so per-control overrides (G17) have a
   typed home.
5. **`xui-form` CLI.** `xui-form check|render|fmt|migrate file.lfm` validates
   a form, renders it headlessly (light and dark), formats it and converts v1
   forms. Agents can iterate on a form without compiling anything.
6. **Move `xui-rhai` into xui** as `xui::script`, behind feature `rhai`.
   - Rhai is pinned once, in xui, at the version lazyOS's `rhai-lazy`
     uses.
   - `lazyos/rhai-host` can then open forms with handlers directly, without
     going through LazyRAD.
   - LazyRAD keeps only what an IDE needs: its stdlib, the `FsPolicy` and
     the player.
   - The `on_var` resolver, the naming-convention binder and the operation
     budget move as they are.
   - `rhai` stays out of `xui-core`'s default build, so Rust-only apps don't
     compile it.

### W4: LazyRAD designer on layouts [rad]

1. The designer surface edits a layout tree, not only coordinates.
   - Drop onto a container to insert a child; drag to reorder.
   - The property grid shows the layout props (`fill`, `align`, `span`).
2. The `absolute()` container stays the default for "VB6 mode", so users who
   want free placement keep it. The other option is "Convert to grid/column",
   which infers rows and columns from aligned rects.
3. Tab order follows tree order (M6 item).
4. **Hot reload in the player.**
   - On `.lfm` change: rebuild the form and keep `form.state`.
   - On `.rhai` change: recompile and swap handlers between events (PLAN
     §7 stretch).
   - This is cheap because both formats are interpreted, and it is the
     largest win for iteration time.

### W5: lazyOS app boilerplate [os]

1. **`lazyos_app` crate (in `xui-app`).** `lazyos_app::main!(App)` or
   `lazyos_app::run(spec, init)` wraps connect, theme, the first-frame marker,
   run, unbind, exit codes and the `XXX:*:PASS/FAIL` markers. The markers are
   derived from the package name.
2. **Scaffolder.** `tools/xui/new_app.py <short> --kind rust|rhai`. It
   writes the bin or `.lrp`, the manifest, the `build.rs` switch, the
   `run_demo.py` flag, the `test_catalog.py` entry and a headless snapshot
   test. It replaces the AGENTS.md checklist with one command.
3. **Migrate the 15 hand-placed apps** to `arrange`/`grid`, one PR per app,
   in this order: Counter, Network, NetTools, Settings pages, Config,
   Widget, then the custom-layout apps (Paint, Explorer, Web) with their
   `layout()` functions replaced. Then delete `hidpi::rect` and
   `design_rect`; `design_point` stays for custom painters until W1.9 covers
   hit regions.
4. **Rhai-first small apps.** New utility-class apps (dialogs, settings
   panes, monitors) default to `.lfm` plus `.rhai` packaged as `.lzp` with
   `lrplay`. Rust is for apps that need it (editors, Paint, browsers).
   Document this decision rule in AGENTS.md.
5. **One xui pin.**
   - The canonical revision lives in one file, `lazyos/xui-rev.toml`.
     lazyOS is the integration point that ships all three.
   - A workspace-level `[patch]` only affects the workspace that declares
     it, so each consumer pins explicitly:
     - xui-app and lazyrad-os: their `Cargo.toml` `rev` is rewritten from
       the file by `tools/xui/bump.py`, the same script that bumps the file.
     - LazyRAD: the bump PR is opened in LazyRAD with the same rev, and its
       transitive xui must resolve to that rev too.
   - CI in each repo reads the resolved xui rev from `Cargo.lock` and fails
     when it differs from `xui-rev.toml`. LazyRAD fetches the file from
     lazyOS `main`.

### W6: Generation and iteration speed [all]

1. **Verify headlessly first.**
   - Agents verify layout with `xui-form render` or
     `xui_canvas::snapshot` plus `layout_report()` on the host, in seconds,
     before QEMU.
   - QEMU runs only for integration evidence.
   - Add this order to the lazyOS and xui AGENTS.md files.
2. **Compile time.**
   - Feature-gate heavy optional parts of `xui-core`: colour picker, file
     dialog, HTML.
   - Keep the `opt-level` overrides for tiny-skia only, not the whole
     dependency graph.
   - Use a shared `CARGO_TARGET_DIR` per repo for agent worktrees, with
     sccache.
   - Land the CI runner plan's caching. Target: the xui-app incremental
     rebuild of one bin in under 20 s.
3. **Fewer tokens per app.**
   - The W2 and W3 shapes cut a typical app by 3–5×.
   - Add `docs/cookbook.md`: about 15 copy-ready patterns (form+grid,
     master/detail, toolbar+status, settings page, list with
     add/remove, timer-driven monitor).
   - Add an `xui-app` skill in `.claude/skills/`, so agents start from a
     known-good template instead of reading 600 lines of AGENTS.md.
4. **Schema for agents.** `xui-form schema --json` dumps the `Catalog`:
   widgets, properties, events and layout props. Agents and editors get
   exact names, and `xui-form check` returns errors an agent can act on (with
   a "did you mean").

## Sequencing

```
M1  W1.1-1.3, W2.4, W2.5, W5.1, W5.5 protocol, align, grid, app builder, lazyos_app, single pin
M2  W1.4-1.9, W2.1-2.3, W5.2         complete layout, builders + handles (Rc children gone), scaffolder
M3  W2.6, W5.3                       migrate every xui example and lazyOS app, delete rect constructors
M4  W3.1-3.6, W6.1, W6.4             form + script in xui, RON lfm v2 + migrate, CLI, headless-first
M5  W4.1-4.4, W5.4, W6.2-6.3         designer on layouts, hot reload, rhai-first, build speed
```

The single pin moves to M1 because breaking changes land quickly and three
repos have to follow them together. M3 is one coordinated change: the xui PR
that deletes the rect constructors and the lazyOS PRs that migrate the last
apps land against the same rev.

## Success metrics

| Metric | Today | Target |
|---|---|---|
| Lines for the label demo | 60 (+136 support) | ≤ 25, no support file |
| lazyOS Counter | 116 | ≤ 35 |
| Calculator `.lfm` + `.rhai` | 159 + 65 | ≤ 45 + 25 |
| lazyOS apps placing widgets by hand | ~15 / 20 | 0 (custom painters use `absolute()`) |
| Apps that re-flow correctly on resize and at 2× | ~5 | all |
| Files touched to add a lazyOS app | ~6, by hand | 1 command |
| Agent loop "edit form → see result" | rebuild + QEMU, minutes | `xui-form render`, < 2 s |
| Incremental rebuild of one xui-app bin | minutes | < 20 s |

## Risks

- **Breaking churn across three pinned repos.** The plan chooses this on
  purpose: there are no shims and no deprecation window.
  - The single pin (W5.5, in M1) keeps the repos in lockstep.
  - Each removal ships with its migrations: xui examples in the same PR,
    lazyOS and LazyRAD bumps against that rev.
  - `xui-form migrate` handles forms.
- **Handles vs owned widgets.** This moves ownership into `Ui`. The
  invariant "controls own their node and destroy it in `Drop`" still holds
  inside the store. Document the change in `architecture.md`.
- **Designer UX regression for VB6-style users.** `absolute()` stays a
  first-class container and is the default for new forms in LazyRAD.
- **Layout performance in `WM_PAINT` and frame paths.** Dirty-tracking
  avoids a full relayout, and `measure` must not allocate per frame
  (AGENTS.md hot-path rule). Add a benchmark in the M2 PRs.

## Decisions

1. **`xui-rhai` moves into xui** as `xui::script` (W3.6).
2. **`.lfm` v2 is RON.**
   - A UI is a tree of typed nodes, and RON's `Variant(field: …)` maps
     straight onto Rust enums through serde. The schema and the format are
     one definition.
   - TOML forces a flat `[[node]]` list with `parent` links. TOML 1.0 doesn't
     allow multi-line inline tables, so it can't express a tree readably.
   - KDL reads well for trees and preserves comments when edited. But it has
     no serde-native mapping, it changed syntax from v1 to v2, and models
     have seen far less of it.
3. **Handles replace `Rc` children immediately, and every old path is
   removed rather than deprecated**: rect constructors, `::auto`,
   `natural_size`, the runtime v1 `.lfm` loader. The clean path is the only
   path.
