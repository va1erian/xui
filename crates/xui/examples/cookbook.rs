//! The recipes of `docs/cookbook.md`, one tab each, compiled and rendered so
//! the snippets there stay true.
//!
//! ```text
//! cargo run -p xui --example cookbook
//! XUI_SNAPSHOT=target/snapshots cargo run -p xui --example cookbook
//! ```

use xui::prelude::*;

#[derive(Clone)]
enum Msg {
    Submit,
    Pick(usize),
    Draft(String),
    Add,
    Remove,
    Tick,
}

/// The people the master/detail recipe lists.
const PEOPLE: [(&str, &str, &str); 3] = [
    ("Ada Lovelace", "Analyst", "London"),
    ("Grace Hopper", "Admiral", "Arlington"),
    ("Alan Turing", "Mathematician", "Wilmslow"),
];

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

// [form]
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
// [/form]

// [master_detail]
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
// [/master_detail]

// [editable_list]
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
// [/editable_list]

// [settings]
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
// [/settings]

// [live]
/// Live values: a gauge a periodic message refreshes (see `main`).
fn live(app: &Cookbook) -> Layout<Msg> {
    column().padding(16).gap(8).children((
        label("Load"),
        progress(100).value(0).bind(&app.load),
        label("Refreshed every 500 ms by Ui::every"),
    ))
}
// [/live]

impl App for Cookbook {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _ui: &mut Ui<Msg>) {
        match msg {
            Msg::Submit => {
                let (name, email) = (self.name.get().text(), self.email.get().text());
                self.status
                    .get()
                    .set_text(0, &format!("Submitted {name} <{email}>"));
            }
            Msg::Pick(row) => {
                let (_, role, city) = PEOPLE[row.min(PEOPLE.len() - 1)];
                self.role.get().set_text(role);
                self.city.get().set_text(city);
            }
            Msg::Draft(text) => self.draft = text,
            Msg::Add => {
                if !self.draft.is_empty() {
                    self.entries.push(self.draft.clone());
                    self.show_entries();
                }
            }
            Msg::Remove => {
                if let Some(row) = self.items.get().selected() {
                    self.entries.remove(row);
                    self.show_entries();
                }
            }
            Msg::Tick => {
                self.ticks = (self.ticks + 7) % 100;
                self.load.get().set_value(self.ticks);
            }
        }
    }
}

impl Cookbook {
    /// Shows the entries, keeping the list's scroll and selection.
    fn show_entries(&self) {
        let rows: Vec<Vec<String>> = self.entries.iter().map(|e| vec![e.clone()]).collect();
        self.items.get().refresh_model(rows);
    }
}

// [shell]
/// The window: tab pages that fill, a status bar under them, a timer.
fn main() -> Result<()> {
    xui::app("Cookbook").size(640, 420).run(|ui| {
        let app = Cookbook::default();
        ui.root(
            column().children((
                tabs()
                    .page("Form", form(&app))
                    .page("Master/detail", master_detail(&app))
                    .page("List", editable_list(&app))
                    .page("Settings", settings())
                    .page("Live", live(&app))
                    .fill(1),
                status_bar(&["Ready"]).bind(&app.status),
            )),
        )?;
        ui.every(500, Msg::Tick);
        Ok(app)
    })
}
// [/shell]
