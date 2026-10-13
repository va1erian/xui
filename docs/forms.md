# Forms and scripts

A form is a window's content described as data, in a `.lfm` file, instead of
Rust code. `xui::form` (feature `form`) loads, validates and builds one;
`xui::script` (feature `rhai`) runs a [Rhai](https://rhai.rs) script against
it, so a small app needs no Rust event code at all. Both crates (`xui-form`,
`xui-script`) came from LazyRAD, the RAD IDE built on xui.

## The `.lfm` format 2

A form is [RON](https://github.com/ron-rs/ron): a tree of layouts and
widgets, each node written `Kind(field: value, …)`.

```ron
Form(title: "Hello", size: (360, 160), root: Column(gap: 8, padding: 16, children: [
    Row(gap: 8, children: [
        Edit(name: "name_edit", placeholder: "Your name", fill: 1),
        Button(name: "greet_button", text: "Greet"),
    ]),
    Label(name: "result_label"),
]))
```

- **The form:** `name`, `title`, `size` (design units, default
  `(320, 200)`), `resizable` and `root`.
- **Layouts:** `Row`, `Column` and `Wrap` (`gap`, `padding`, `align_items`,
  `justify`, `children`), `Grid` (`columns: [Auto, Fixed(80), Fill(1)]`,
  `gap`, `padding`, `align_items`, `children`) and `Absolute` (`size`, the
  size its positions were designed at, `padding`, `children`).
- **Containers:** `Panel(content: …)`, `Group(text: "Options", content: …)`
  and `Tabs(pages: [Page(title: "General", content: …)])`. A `content` that
  is a widget is laid out as a column of one.
- **Widgets:** `Label`, `Button`, `Hyperlink`, `CheckBox`, `ToggleButton`,
  `Edit`, `MultilineEdit`, `NumberField`, `Slider`, `ProgressBar`,
  `RadioGroup`, `ComboBox`, `ListView` and `Separator`. Each has `name`, its
  properties, `visible` and `enabled`.
- **Layout fields**, on every node: `fill` (a share of the leftover space),
  `width`, `height`, `max_width`, `max_height`, `align` (`Start`, `Center`,
  `End`, `Stretch`), `span` (in a `Grid`), and `at: (x, y, width, height)`
  with `anchor` (`TopLeft`, …, `BottomRight`, `StretchHorizontal`,
  `StretchVertical`, `Fill`) in an `Absolute` layout.
- **Defaults are omitted.** Every field has a default and a field equal to
  it is never written. `Form::to_ron` writes one canonical layout (a fixed
  `PrettyConfig`: a node per line, its children below it), so a saved file is
  byte-stable and `xui-form fmt` of a formatted file changes nothing.
  Comments are not kept.
- **Control arrays.** `Button(name: "digit", array: 10, text: "{index}")` is
  ten buttons, `digit[0]` to `digit[9]`; `{index}` in a text becomes the
  element's index. Elements placed apart are written one by one with
  `index: 7`. All elements share one handler, which receives the index.

The node types are plain serde types in `xui::form::model`, declared once by
macros that also produce the [`Catalog`](#the-schema): the format and the
schema cannot drift. `xui::form::load` reports a misspelt field or kind with
its line, column and a "did you mean" (`txt` → `text`, `Colum` → `Column`).
`Form::validate` reports the rest at once: names that are not identifiers or
are used twice, clashing control arrays, values out of range (`selected:
-1`), and layout fields the parent ignores (`at` outside an `Absolute`, `span`
outside a `Grid`).

A format-1 file (the flat TOML of the first LazyRAD releases) is not loaded
at runtime: convert it once with `xui-form migrate` (or
`xui_form::migrate::from_v1`, feature `migrate`). Each container's children
become an `Absolute` layout with their old rectangles and anchors, or a
`Grid` when the rectangles line up in rows and columns with one gap and none
is anchored. A `GroupBox` becomes a `Group`, `cue` becomes `placeholder`, and
`tab_index` is dropped: the tab order is the tree order.

## Building a form

```rust
use xui::form::{build, load, Factories, Handlers};

let form = load(include_str!("main.lfm"))?;
let handlers = Handlers::new().on("greet_button", "Click", Msg::Greet);
let live = build(ui, &form, &Factories::xui(), &handlers)?;
live.set("result_label", "text", &Value::Text("Ready".into()))?;
```

`build` turns the tree into the same `arrange` builders a Rust app writes (a
`Row` is `row()`, a `Button` is `button(..)`) and mounts them as the
window's content; the `LiveForm` reads and writes each named widget's
properties. `describe` stops before mounting and returns the `Layout` and the
`Controls`, to place a form inside a larger Rust layout (call
`Controls::ready` once it is mounted).

Events map to the app's messages through a `Binder`: `Handlers` attaches
closures by widget name and event (`on_with` gets a control array's index),
and any other `Binder` can decide per event. `BuildOptions::design_mode`
wires no events, for a designer's preview, and `BuildOptions::container`
builds inside a container node.

`left`, `top`, `width` and `height` are runtime properties of a widget in an
`Absolute` layout: writing one moves the widget for good (the layout reads a
shared `layout::Placement`). Elsewhere the layout places the widget and they
are read-only.

To add a widget kind, register a `WidgetFactory` in the `Factories`: it
returns `Created::new(entry, live)`, the `arrange` builder that creates the
widget at mount time, bound to a `Handle`, and the `LiveWidget` that reads
and writes it afterwards. New kinds in the file format belong in
`xui-form`'s model.

## The schema

`Catalog::xui()` is every kind's spec: each widget's runtime properties (name,
type, default, category, access) and events (name, arguments), every node's
file fields with their RON types and defaults, the layout fields and the
form's fields. It is `Serialize`; `xui-form schema --json` prints it.

## The `xui-form` tool

`crates/xui-form-cli` builds the `xui-form` binary (`cargo install --path
crates/xui-form-cli`, or `cargo run -p xui-form-cli --`). It works on forms
without compiling an app, so an agent or a designer can iterate in seconds:

| Command | What it does |
|---|---|
| `xui-form check FILE...` | Loads and validates each form: `file:line:col: error: … (did you mean `text`?)` for a load error, `file: error: `node`: …` or `warning:` for each diagnostic, `file: ok` otherwise. Exits 1 when any form has an error. |
| `xui-form render FILE [--out DIR] [--script F.rhai] [--dpi N] [--overlay] [--report]` | Renders `<name>-light.png` and `<name>-dark.png` headlessly. `--script` runs the script's top-level code and `form_load` first; `--overlay` draws the layout's bounds; `--report` prints `Ui::layout_report` (every rect, with warnings). |
| `xui-form fmt FILE... [--check]` | Rewrites each form canonically; `--check` only lists the ones that are not, and exits 1 when there are any. |
| `xui-form migrate FILE [--out F]` | Converts a format-1 (TOML) form to format 2, printing what it dropped to standard error. |
| `xui-form schema --json` | Prints the `Catalog`: every widget with its properties, events and fields, the layouts, and the common, layout, page and form fields. |

A misspelt command gets a "did you mean" too. Usage errors exit with
status 2.

## Scripting a form with Rhai

```rhai
fn form_load() { clear_click(); }

fn digit_click(index) {
    form.state.entry = `${form.state.entry}${index}`;
    display.text = form.state.entry;
}
```

- **Handlers** are `fn <control>_<event>(args…)`: the control's name and the
  event's name in snake_case (`greet_button_click`, `name_edit_change`,
  `agree_check_toggle`, `items_list_activate`). A control array's handler
  takes the element's index first (`digit_click(index)`). The form's own
  events use the `form` prefix: `form_load`, `form_close`. An event is wired
  only when its handler exists; a handler may declare fewer parameters than
  the event carries, or more (they are `()`).
- **Controls** are in scope by name in every function, through an
  `Engine::on_var` resolver that *pushes* the control into the scope, so
  `label.text = …` writes through. A control array is a Rhai array:
  `digit[3].text`. A property is any of the catalog's names for the control's
  kind; a wrong one is a runtime error with a "did you mean".
- **`form`** has `title`, `state` (an object map that lives as long as the
  form, for values kept between events: top-level `let`s are not visible in
  functions) and `show()`/`hide()`.
- **Limits.** Each handler run has an operation budget
  (`EngineHost::set_operation_budget`, 1,000,000 by default), and
  `EngineHost::stop` ends a running script at its next progress check.
- Errors are `ScriptError`s located in the script file (`calc.rhai:3:5: …`).

`ScriptForm::build(ui, &form, source, setup, configure)` builds the widgets,
compiles the script, runs its top-level code once and calls `form_load`; the
app routes each `script::Msg::Event` back with `ScriptForm::run`. `setup` (an
`EngineSetup`) installs a host's standard library; `()` installs none. Rhai
is pinned to `=1.26.1` (no default features; `std` and `debugging`), the
version LazyRAD and lazyOS link.

See `examples/form.rs` (with `forms/login.lfm`) and `examples/script.rs`
(the calculator: `forms/calculator.lfm` and `calculator.rhai`).
