# Cookbook

Copy-ready layouts for the windows apps usually need, on the declarative API
(builders, layouts, handles; see [Getting started](getting-started.md)). Each
recipe is a function from `crates/xui/examples/cookbook.rs`, which compiles
them and renders them, so the code here is known to build and lay out:

```text
cargo run -p xui --example cookbook
XUI_SNAPSHOT=target/snapshots cargo run -p xui --example cookbook   # light + dark PNGs
```

Every recipe follows the same few rules:

- **Describe, don't place.** A builder (`label`, `edit`, `button`, `list`, …)
  becomes a widget when the layout is mounted; no `Rect`, no `ui` argument.
- **Bind what you change.** A field `Handle<W>` in the app, `bind(&handle)` on
  the builder, `handle.get()` in `update`. Everything else needs no field.
- **Map events by value.** `on_click(Msg::Save)`, `on_change(Msg::Name)`,
  `on_select(Msg::Pick)`; `Msg` derives `Clone`.
- **Size only what must not be natural.** `fill(1)` takes the leftover space,
  `width`/`height`/`fixed` pin a size, `max_width` caps one; a nested row or
  column is as tall (or wide) as its content unless it fills.

The recipes share this app struct and message enum:

```rust
#[derive(Clone)]
enum Msg { Submit, Pick(usize), Draft(String), Add, Remove, Tick }

#[derive(Default)]
struct Cookbook {
    name: Handle<Edit<Msg>>,
    email: Handle<Edit<Msg>>,
    role: Handle<Label<Msg>>,
    city: Handle<Label<Msg>>,
    items: Handle<ListView<Msg>>,
    entries: Vec<String>,
    draft: String,
    load: Handle<ProgressBar<Msg>>,
    ticks: i32,
    status: Handle<StatusBar<Msg>>,
}
```

## The window shell

Pages that fill the window, a status bar under them, and a periodic message.
`ui.root` keeps the layout for the window's lifetime.

```rust
/// The window: tab pages that fill, a status bar under them, a timer.
fn main() -> Result<()> {
    xui::app("Cookbook").size(640, 420).run(|ui| {
        let app = Cookbook::default();
        ui.root(column().children((
            tabs()
                .page("Form", form(&app))
                .page("Master/detail", master_detail(&app))
                .page("List", editable_list(&app))
                .page("Settings", settings())
                .page("Live", live(&app))
                .fill(1),
            status_bar(&["Ready"]).bind(&app.status),
        )))?;
        ui.every(500, Msg::Tick);
        Ok(app)
    })
}
```

## A form

A two-column grid (`Track::Auto` for the labels, `Track::Fill(1)` for the
fields) keeps labels aligned however long they are; `justify(Align::End)` puts
the action at the right.

```rust
/// A form: labels and fields in a two-column grid, the action right-aligned
/// under it.
fn form(app: &Cookbook) -> Layout<Msg> {
    column().padding(16).gap(12).children((
        grid([Track::Auto, Track::Fill(1)]).gap(8).children((
            label("Name").align(Align::Center),
            edit().placeholder("Ada Lovelace").bind(&app.name),
            label("Email").align(Align::Center),
            edit().placeholder("ada@example.com").bind(&app.email),
        )),
        row()
            .justify(Align::End)
            .child(button("Submit").on_click(Msg::Submit)),
    ))
}
```

## Master/detail

A list of fixed width beside a group that fills the rest. `on_select` carries
the row; `update` fills the detail labels through their handles.

```rust
/// Master/detail: a list on the left, its selection in a group on the right.
fn master_detail(app: &Cookbook) -> Layout<Msg> {
    let names: Vec<Vec<String>> = PEOPLE.iter().map(|p| vec![p.0.to_string()]).collect();
    row().padding(16).gap(12).children((
        list()
            .column("Name", Fill)
            .on_select(Msg::Pick)
            .then(move |list| {
                list.set_model(names);
                list
            })
            .width(200),
        group(
            "Details",
            grid([Track::Auto, Track::Fill(1)]).gap(6).children((
                label("Role"),
                label("").bind(&app.role),
                label("City"),
                label("").bind(&app.city),
            )),
        )
        .fill(1),
    ))
}
```

## A list with add and remove

The entry row is as tall as its widgets; the list fills the rest. Refresh a
list whose data changes with `refresh_model`, which keeps its scroll and
selection (`set_model` starts over).

```rust
/// A list with add and remove: an entry row over the list.
fn editable_list(app: &Cookbook) -> Layout<Msg> {
    column().padding(16).gap(8).children((
        row().gap(8).children((
            edit().placeholder("New item").on_change(Msg::Draft).fill(1),
            button("Add").on_click(Msg::Add),
            button("Remove").on_click(Msg::Remove),
        )),
        list().column("Item", Fill).bind(&app.items).fill(1),
    ))
}
```

## A settings page

`group(title, layout)` frames a layout; each group is as tall as its content,
so the page reads top to bottom. `max_width` keeps a picker from stretching.

```rust
/// A settings page: grouped options, each group as tall as its content.
fn settings() -> Layout<Msg> {
    column().padding(16).gap(12).children((
        group(
            "Appearance",
            column()
                .gap(6)
                .children((checkbox("Dark mode"), checkbox("Large text"))),
        ),
        group(
            "Updates",
            column().gap(6).children((
                checkbox("Check automatically").checked(true),
                combo_box(&["Daily", "Weekly", "Monthly"]).max_width(160),
            )),
        ),
    ))
}
```

## Live values

`ui.every(500, Msg::Tick)` raises a message periodically (see the shell); the
handler sets the bar through its handle.

```rust
/// Live values: a gauge a periodic message refreshes (see `main`).
fn live(app: &Cookbook) -> Layout<Msg> {
    column().padding(16).gap(8).children((
        label("Load"),
        progress(100).value(0).bind(&app.load),
        label("Refreshed every 500 ms by Ui::every"),
    ))
}
```
