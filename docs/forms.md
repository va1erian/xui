# Forms and scripts

A form is a window's widgets described as data, in a `.lfm` file, instead of
Rust code. `xui::form` (feature `form`) loads, validates and builds one;
`xui::script` (feature `rhai`) runs a [Rhai](https://rhai.rs) script against
it, so a small app needs no Rust event code at all. Both crates
(`xui-form`, `xui-script`) came from LazyRAD, the RAD IDE built on xui, and
keep its API.

```text
          schema                 document                live widgets
Catalog ─────────► FormDoc ─────────────► build() ───────────────► LiveForm
(what a kind      validate()   (.lfm, byte-       (Factories + Binder)
 supports)                      stable)
```

## The `.lfm` document (format 1)

```toml
format = 1

[window]
name = "login_form"
title = "Sign in"
width = 320
height = 200

[[node]]
kind = "Edit"
name = "user_edit"
left = 100
top = 16
width = 204
height = 28
anchor = "stretch_horizontal"
cue = "your name"

[[node]]
kind = "Button"
name = "ok_button"
parent = "options_group"
text = "OK"
```

- The node list is **flat**: a child names its container with `parent`.
- Every node has the common properties `left`, `top`, `width`, `height` (in
  design units; an omitted size is the kind's default), `anchor`, `visible`,
  `enabled` and `tab_index`. The rest come from the kind's spec in the
  `Catalog`.
- `anchor` is one of `top_left` (the default), `top`, `top_right`, `left`,
  `center`, `right`, `bottom_left`, `bottom`, `bottom_right`,
  `stretch_horizontal`, `stretch_vertical` and `fill`.
- Saving is canonical: a fixed key order and only non-default values, so a
  load/save cycle of a canonical file is byte-identical.

`FormDoc::from_toml` decodes each property against its type in the schema and
reports the first error with its line; `FormDoc::validate` reports every
problem at once (names, kinds, types, parents, cycles, duplicate tab indices).

## Building a form

```rust
use xui::form::{build, Catalog, Factories, FormDoc};

let catalog = Catalog::xui();
let doc = FormDoc::from_toml(include_str!("login.lfm"), &catalog)?;
let form = build(ui, &doc, &catalog, &Factories::xui(), &MyBinder)?;
form.set("status_label", "text", &Value::Text("Ready".into()))?;
```

`build` turns each container's children into an `absolute()` layout: every
node is an entry at its design rectangle, with its anchor. The form is mounted
as the window's content (or inside `BuildOptions::container`), so it
re-anchors itself whenever the window is resized; a `GroupBox` places its
children inside its frame, below its title. `LiveForm::set` of `left`, `top`,
`width`, `height` or `anchor` moves the widget for good: the layout reads each
node's position from a shared `layout::Placement`.

A `Binder` decides what each event becomes: it returns an `EventHandler` (a
closure from the event's typed arguments to the app's `Msg`) or `None` to
leave the event unwired. `BuildOptions::design_mode` wires no events, for a
designer's preview.

To add a widget kind, register a `WidgetSpec` in the `Catalog` and a
`WidgetFactory` in the `Factories`. A factory returns `Created::new(entry,
live)`: the `arrange` builder that creates the widget at mount time, bound to
a `Handle`, and the `LiveWidget` that reads and writes it afterwards.
`Catalog::alias` exposes a kind under another name.

See `examples/form.rs`.

## Scripting a form with Rhai

```rhai
fn form_load() {
    result_label.text = greeting(name_edit.text);
}

fn greet_button_click() {
    result_label.text = greeting(name_edit.text);
}
```

- **Handlers** are `fn <control>_<event>(args…)`: the control's name and the
  event's name in snake_case (`greet_button_click`, `name_edit_change`,
  `agree_check_toggle`, `items_list_activate`). The form's own events use the
  `form` prefix: `form_load`, `form_close`. An event is wired only when its
  handler exists; a handler may declare fewer parameters than the event
  carries, or more (they are `()`).
- **Controls** are in scope by name in every function, through an
  `Engine::on_var` resolver that *pushes* the control into the scope, so
  `label.text = …` writes through. A property is any of the catalog's names
  for the control's kind; a wrong one is a runtime error with a "did you
  mean" (`Text` → `text`).
- **`form`** has `title`, `state` (an object map that lives as long as the
  form, for values kept between events: top-level `let`s are not visible in
  functions) and `show()`/`hide()`.
- **Limits.** Each handler run has an operation budget
  (`EngineHost::set_operation_budget`, 1,000,000 by default), and
  `EngineHost::stop` ends a running script at its next progress check.
- Errors are `ScriptError`s located in the script file (`hello.rhai:3:5: …`).

`ScriptForm::build(ui, &doc, &catalog, source, setup, configure)` builds the
widgets, compiles the script, runs its top-level code once and calls
`form_load`; the app routes each `script::Msg::Event` back with
`ScriptForm::run`. `setup` (an `EngineSetup`) installs a host's standard
library; `()` installs none. Rhai is pinned to `=1.26.1` (no default
features; `std` and `debugging`), the version LazyRAD and lazyOS link.

See `examples/script.rs` with `examples/forms/hello.lfm` and `hello.rhai`.
