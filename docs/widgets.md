# Widgets

The portable widget layer lives in `xui_core::widget`. Every widget is a value
that owns one node in the window. You describe it with a **builder** from
`xui_core::arrange` (all in `xui::prelude`), chain options and event mappings,
and place it in a layout; the widget is created when the layout is mounted, and
the layout owns it. A [`Handle`](../crates/xui-core/src/arrange/build.rs)
reaches a widget the app changes later. There is no `Rect` and no `ui` per
widget: layouts size widgets from their content (see
[Architecture → Layout](architecture.md#layout)).

```rust
use xui::prelude::*;

let greeting: Handle<Label<Msg>> = Handle::new();
ui.root(column().padding(16).gap(8).children((
    edit().placeholder("your name").on_change(Msg::Name),
    label("Hello").bind(&greeting),
)))?;
// later, in `update`:
greeting.get().set_text("Hello, you");
```

Mounting returns the first constructor error. `absolute()` with
`.at(x, y, w, h)` is the one container for free positions (a designer
surface, a form imported from coordinates). Dialogs (`Dialog::message`,
`TaskDialog::new`, `FileDialog`), `Menu::context` and `Tooltip::attach` are
created directly, since they are not placed in a layout.

## Common shape

- **No control ids.** Widgets are values; notifications are routed internally by
  `WidgetId`. There is no base trait — each widget has the methods it needs.
- **Events map to your `Msg`.** `on_click`, `on_select`, `on_change`,
  `on_toggle`, `on_activate`, … take a closure returning `Option<Msg>`. The
  runtime queues the message and calls `App::update`, never re-entered.
- **Design values are `Dip`.** Sizes and gaps given to layouts (`.width(120)`,
  `.gap(8)`) are design units, scaled to the window's DPI.
- **`Properties`.** Most widgets expose a `Properties` surface
  (`property(name)`, `set_property(name, value)`) for generic tooling; it is not
  the primary API.
- **Models are borrowed.** `ListModel`, `TreeModel` and `GridModel` return
  borrowed data, so painting allocates nothing per visible item.

## Catalogue

### Text and display

| Widget | Builder | Notable API |
|---|---|---|
| `Label<M>` | `label(text)`, `.title()`, `.caption()` | `HasText` (`text`/`set_text`), `set_selected` |
| `Hyperlink<M>` | `hyperlink(text)` | `on_click(Fn() -> Option<M>)` |
| `FlowText<M>` | `flow_text().run(..)` | `run(Run<M>)`, `separator(&str)`, `preferred_height(Dip)`; runs are `Run::normal/weak/link`, with per-run `on_click` |
| `Icon` | enum | `draw_icon(canvas, Icon, rect, color, dpi)`; `Icon::{Plus, Minus, Close, Check, ChevronDown, ChevronUp, Search, More}` |
| `Separator<M>` | `separator()`, `vertical_separator()` | `orientation()` |
| `GroupBox<M>` | `group(title, layout)` | non-interactive frame around a layout; bind it to hide the frame and its content as one |
| `ProgressBar<M>` | `progress(max)`, `.value(v)` | `value`/`set_value`, `set_max` |

### Buttons and choices

| Widget | Builder | Events |
|---|---|---|
| `Button<M>` | `button(text)`, `.icon(..)`, `.tooltip(..)`, `.primary()` | `on_click(msg)` |
| `CheckBox<M>` | `checkbox(text)`, `.checked(b)` | `on_toggle(bool)`; `is_checked`/`set_checked` |
| `ToggleButton<M>` | `toggle_button(text)`, `.icon(..)`, `.tooltip(..)` | `on_toggle(bool)` |
| `RadioGroup<M>` | `radio_group(&[labels])`, `.selected(i)` | `on_select(usize)`; `selected`/`select` |
| `ColorPicker<M>` | `color_picker(&[Color])` | `columns(n)`, `selected(Color)`, `on_select(Color)` |
| `ColorPanel<M>` | `color_panel()`, `.color(c)` | tabbed picker: `color`, `set_color`, `select_tab`, `on_change(Color)`, `on_commit(Color)`; `color_field(hsv)`/`hue_slider(h)` are its reusable parts |

### Colour

`ColorPanel<M>` is a tabbed colour picker drawn entirely by xui, so it runs on
every backend (no `ChooseColor`, GTK or portal). Its **Simple** tab is the
32-colour `BASIC_COLORS` grid built on [`ColorPicker`](#buttons-and-choices);
its **Full** tab has a flat preview, an SV `ColorField`, a `HueSlider`, and
editable HEX, RGB, CMYK, HSV and HSL boxes. The panel keeps an `Hsv` triple as
its source of truth, so dragging through black or white preserves the hue.

```rust
let panel: Handle<ColorPanel<Msg>> = Handle::new();
ui.root(column().child(
    color_panel()
        .color(Color::rgb(0xEB, 0x40, 0x34))
        // Every change, including drag ticks.
        .on_change(Msg::Preview)
        // A mouse-up, Enter, a swatch pick or a valid text commit.
        .then(|panel| panel.on_commit(|color| Some(Msg::Apply(color))))
        .bind(&panel),
))?;

panel.get().set_color(Color::rgb(0, 0x78, 0xD4)); // programmatic; fires nothing
```

The colour maths (RGB/HSV/HSL/CMYK, hex parse/format) is a pure module; the
`#eb4034` reference is `235, 64, 52` / `0%, 73%, 78%, 8%` / `4°, 78%, 92%` /
`4°, 82%, 56%`. A text box parses its own format on Enter or focus loss and
reverts invalid input, firing `on_commit` only for a valid commit.

### Text entry and ranges

| Widget | Builder | Events |
|---|---|---|
| `Edit<M>` | `edit()`, `.text(..)`, `.placeholder(..)`, `.password()` | `on_change(&str)`; native `EDIT` on Win32, painted elsewhere |
| `MultilineEdit<M>` | `multiline_edit()` | `on_change(&str)` |
| `NumberField<M>` | `number_field(min, max, step)` | `on_change(f64)`, `on_commit(f64)` |
| `Slider<M>` | `slider(min, max)` | `on_change(f64)`, `on_commit(f64)`; `set_range` |
| `ComboBox<M>` | `combo_box(&[items])` | `on_select(usize)`; `item_icon(index, impl Into<IconRef>)` / `set_item_icon(index, Option<IconRef>)` add a leading 16 DIP icon to an item, in the list and in the closed box while selected (`icon(index)` reads it back) |

### Rich text (optional)

`xui-rich-text` provides `RichTextEditor`, an editable rich-text widget with
character and paragraph formatting, lists, tables and images that text flows
around. It is a custom-painted node like `xui-code-editor`'s editor, so it runs
on every backend and follows the theme. It is opt-in: depend on the crate directly or
enable the `xui` crate's `rich-text` feature. Documents save as JSON and export
to Markdown. See its [README](../crates/xui-rich-text/README.md).

### Collections

`ListView<M>` is a virtualized list. Build it from words or from a model:

```rust
let list: Handle<ListView<Msg>> = Handle::new();
ui.root(column().child(
    list()
        .column("Name", Fill)
        .on_select(Msg::Open)
        .on_activate(Msg::Open)
        .bind(&list)
        .fill(1),
))?;

// Later: swap the data, selection and scroll survive.
list.get().set_items(&["A", "B", "C"]);
```

- Builders: `column(title, width)`, `column_right`, `add_column(Column)`,
  `selection_mode(SelectionMode)`, `multi_select(bool)`, `on_select`,
  `on_selection(&[usize])`, `on_activate`, `on_context`, `on_sort`.
- Runtime: `set_model`/`set_items`, `selected`, `selection`, `select`,
  `set_selection`, `ensure_visible`, `set_sort_indicator`, `cell_text`,
  `column_count`.
- `set_model(impl ListModel)` (or `refresh_model` for live data) takes a
  custom model; a `Vec<String>` and `Vec<Vec<String>>` implement `ListModel`
  already.
- `ListModel::icon(row) -> Option<IconRef>` (default `None`) gives a row a
  leading 16 DIP icon before its first column's text, e.g.
  `(row < errors).then(|| Lucide::CircleX.into())`. It is drawn in the row's
  text colour (on-accent when selected, disabled colour when disabled); a row
  without one keeps its text at the ordinary inset.

`TreeView<M>` is keyed and lazy, with checkboxes optional and indent guides
behind `indent_guides(bool)` (on by default):

```rust
use xui_core::widget::{Glyph, TreeRow};

tree_view()
    .rows(vec![
        TreeRow::new("Inbox", 0).expandable(true).expanded(true).icon(Glyph::Folder),
        TreeRow::new("Work", 1).icon(Glyph::Tag),
    ])
    .on_select(Msg::Open)
    .then(|tree| tree.on_toggle(|id, expanded| Some(Msg::Fold(id, expanded))))
```

- `on_select(NodeId)`, `on_toggle(NodeId, bool)`, `on_check(NodeId, CheckState)`,
  `checkboxes(bool)`, `tri_state(bool)`.
- `icon(impl Into<RowIcon>)` puts a leading icon before a row's label:
  `RowIcon::Glyph(Glyph::Folder)` draws a themed vector shape (pass a `Glyph`
  directly), and `RowIcon::Image(Image)` draws a decoded bitmap in its own
  colours — uploaded once per image and cached by the backend (keyed by the
  image's identity), so per-row art stays cheap on every repaint.
- Indent guides are selection-aware: they blend into the selected or hovered
  row's fill instead of crossing it with a high-contrast `border` line.
- Runtime: `selected`, `select`, `checked`, `set_checked`, `set_rows`,
  `set_model`.
- `tree_view_with(impl TreeModel)`: only roots are read up front; a
  branch's children are fetched on first expansion.

`GridView<M>` is a virtualized tile grid:

```rust
grid_view_with(model)
    .on_activate(Msg::Open)
    .then(|grid| {
        grid.tile_size(TileSize::new(Dip(120.0), Dip(120.0)).gap(Dip(8.0)))
            .on_paint_tile(|canvas, tile| { /* draw thumbnail + text */ })
    })
```

- `Tile<'a>` is the model's per-tile data (text and optional `Image`);
  `TilePaint<'a>` is the paint context; `TileSize` accepts a `Dip` or
  `(Dip, Dip)`.

`IconView<M>` is a virtualized Windows XP-style icon view: each item is a tile
with an icon on the left and up to three lines of text on the right, flowed left
to right and wrapped. It uses only the portable `Backend` contract, so it is the
same on every backend (there is no native `ListView`):

```rust
use xui_core::widget::{IconModel, IconSize, IconView};

let view: Handle<IconView<Msg>> = Handle::new();
// (in a layout)
icon_view_with(model)
    .on_select(Msg::Select)
    .on_activate(Msg::Open)
    .then(|view| view.on_context(|item, at| Some(Msg::Context(item, at))))
    .bind(&view)
    .fill(1);
// Small (16), Medium (32) or Large (48, the default); reflows and repaints.
view.get().set_icon_size(IconSize::Large);
```

- `IconModel` supplies each tile lazily: `items()`, `icon(item) -> Option<IconRef>`
  and `line(item, line) -> Option<&str>` for lines `0..3` (line 1 is the name in
  the normal text token, lines 2 and 3 are secondary details in the muted token).
  A `Vec<String>` and a `Vec<Vec<String>>` implement it already. Only visible
  tiles are laid out and painted, so a model of 100 000 items costs the same as
  ten. A model that draws its own multi-colour icon implements
  `paint_icon(item, canvas, rect, theme, dpi)`, called before `icon` for each
  visible tile; returning `true` means the `IconRef` fallback is not used.
  `IconView::invalidate` repaints after a live appearance change.
- A tile's width, height and text metrics derive from the icon size. A line that
  does not fit is end-ellipsised; a missing line is simply not drawn. The small
  tile is one line tall, so it shows the name alone.
- States come from semantic tokens: hover, an active selection (accent fill on
  the text block plus an accent-tinted icon), an inactive selection
  (`selection_unfocused`), a dotted focus rectangle on the focused tile and a
  disabled colour. Icons follow the tile's text colour.
- Interaction: a left click selects (Ctrl toggles, Shift extends the range in
  `SelectionMode::Multi`), empty space clears, a double click or Return
  activates, a right click selects an unselected tile and reports its item and
  pointer position through `on_context`, and the arrow keys, Home/End and
  PageUp/PageDown move the focus with the focused tile kept in view.
- Builders: `selection_mode(SelectionMode)`, `multi_select(bool)`, `on_select`,
  `on_selection(&[usize])`, `on_activate`, `on_context`. Runtime: `set_model` /
  `set_items`, `set_icon_size` / `icon_size`, `selected`, `selection`, `select`,
  `set_selection`, `focused`, `ensure_visible`, `set_enabled`, `item_data`,
  `len`, `id`.

### Menus

```rust
use xui_core::widget::{Menu, MenuId};

// A menu bar is a builder like any widget; it is one row tall.
menu_bar(|m| {
    m.submenu(MenuId::new(0), "&File", |f| {
        f.item(MenuId::new(1), "&Open");
        f.separator();
        f.check(MenuId::new(2), "Auto &save", true);
    });
})
.on_select(Msg::Command)

// A context menu has no bar:
let context = Menu::context(ui)
    .on_select(|id| Some(Msg::Command(id)))
    .build(|m| { m.item(MenuId::new(10), "Cu&t"); m.item(MenuId::new(11), "&Copy"); });
context.show_context(x, y);
```

`MenuScope` offers `item`, `separator`, `check`, `radio`, `submenu`, and
`icon(impl Into<IconRef>)`, which gives the entry just appended a leading 16 DIP
icon (`m.item(id, "&Copy").icon(Lucide::Copy)`). A popup with any icon reserves
an icon column so labels stay aligned (it replaces the mark column unless a
check or radio entry needs that too); popups without icons are laid out as
before. Icons follow the entry's text colour (dimmed when disabled) and show in
bar dropdowns and context menus on every backend, since menus are always
painted by the portable popup (there is no native `HMENU` path). Runtime:
`set_enabled`/`is_enabled`, `set_checked`/`is_checked`, `open`/`close`.

### Containers

Containers hold layouts: their children are nodes of the container (clipped
to it, destroyed with it) and laid out in its own coordinates.

| Container | Builder | API |
|---|---|---|
| `Panel<M>` | `panel(layout)`, `.plain()` | `set_layout(layout)`; `set_design_mode(bool)` makes the subtree ignore input (form designers; also on `ScrollView`, `Split`, `Tabs`) |
| `ScrollView<M>` | `scroll(layout)` | the layout is measured at the view's width with no height bound and scrolled as one; `scroll_to(Px)`, `on_scroll(Fn(Px))`, `content_height()` |
| `Split<M>` | `split(a, b)`, `.stacked()`, `.position(d)`, `.min(a, b)` | `set_layouts(a, b)`, `set_position(Dip)`, `on_moved(Fn(Dip))` |
| `Tabs<M>` | `tabs().page(title, layout)` | `add_layout_page`, `remove_page(index)`/`rename_page(index, title)`, `select(index)`/`selected`, `on_change(Fn(usize))` |

```rust
ui.root(column().padding(16).child(
    split(
        column().child(tree_view().rows(rows).fill(1)),
        scroll(column().gap(4).children(items)),
    )
    .position(200)
    .fill(1),
))?;
```

### Status and chrome

| Widget | Constructor | Notes |
|---|---|---|
| `Toolbar<M>` | `text_toolbar(&[labels])` or `toolbar()` + `.item`/`.item_with_text`/`.separator` | `on_click(usize)` counts items, not separators; buttons are content-sized and left-packed, `fill()` splits the width equally; items that do not fit are clipped (no overflow menu yet) |
| `StatusBar<M>` | `status_bar(&[parts])` | `set_text(part, text)`, `set_parts` |
| `MaterialStatusBar<M>` | `material_status_bar(&[parts])` | status bar drawn on a material band |
| `TopBar<M>` | `top_bar()`, items through `.then(..)` | `icon`/`toggle`/`label`/`slider`/`spacer`, keyed by `TopBarId`; `width`/`expand` resize an item; `on_click`/`on_toggle`/`on_change`. `Glyph` icons are vendored [Lucide](https://lucide.dev) outlines (ISC; see the README credits) and include the transport set (`Play`, `Pause`, `Stop`, `Previous`, `Next`, `Repeat`, `Shuffle`) and the navigation set (`Audio`, `Album`, `People`, `Tag`, `Folder`, `Star`, `StarFilled`, `History`, `Monitor`, `Settings`) |

### Dialogs and tooltips

`Dialog<M>` is an in-window modal (a scrim plus a centred card), not an OS
dialog:

```rust
use xui_core::widget::{Dialog, DialogAction};

let dialog = Dialog::confirm(ui, "Save changes?", "Your edits will be lost.")
    .unwrap()
    .accept_label("Save")
    .on_action(|action| Some(Msg::Dialog(action)));
dialog.open();
```

`Dialog::message`, `Dialog::confirm` and `Dialog::prompt` are the entry points;
`DialogAction` is `Accept(String)` or `Cancel`.

`TaskDialog<M>` is the richer sibling: the same scrim and card, plus an
`icon`, any number of app-defined command buttons and an optional verification
checkbox. Add commands with `command`, then read the checkbox with
`is_checked` after the action:

```rust
use xui_core::widget::{TaskDialog, TaskDialogAction, TaskDialogIcon};

let dialog = TaskDialog::new(ui, "Delete 3 files?", "Deleted files cannot be recovered.")
    .unwrap()
    .icon(TaskDialogIcon::Warning)
    .command("Delete").unwrap()
    .command("Keep").unwrap()
    .verification("Don't ask me again").unwrap()
    .on_action(|action| Some(Msg::TaskDialog(action)));
dialog.open();
```

`TaskDialogAction` is `Command(usize)` (the index a command was added with) or
`Cancel`; `TaskDialogIcon` is `None` (the default), `Info`, `Warning`, `Error`
or `Shield`. Enter picks the first command, Escape cancels.

`FileDialog<M>` is the portable file open/save picker: a modal card with a path
bar, a directory list, a filename field, an extension filter and overwrite
confirmation. It never writes; `on_accept` receives the chosen `PathBuf`.
`open()` asks the backend for a native picker first and falls back to the card
(see [Backends](backends.md#file-dialogs-and-the-filesystem-seam)), so the app
code is the same either way.

```rust
use xui_core::widget::FileDialog;

let dialog = FileDialog::save_file(ui, "Save As").unwrap()
    .suggested_name("untitled.txt")
    .filter("Text files", &["txt", "md"])
    .on_accept(|path| Some(Msg::SaveAs(path)))
    .on_cancel(|| None);
dialog.open();
```

`Tooltip<M>` attaches to any widget by id and is owned by one hidden node per
window:

```rust
use xui_core::widget::Tooltip;

let tip = Tooltip::attach(ui, button.id(), "Open the docs").unwrap();
tip.show(); // e.g. for a screenshot; otherwise it appears on hover
```

## Text

Text is shaped by the backend through `Backend::text_shaper()`, a `Send + Sync`
handle a worker can use. The `WinitBackend` and `Win32Backend` both shape and
hit-test; `Canvas::draw_layout` draws a layout that came back from a worker.
`FlowText` supports styled runs (normal/weak/link), rich ranges, links and
wrapping. See [Backends](backends.md) for the per-backend text stack.

## Related

- [Theming](theming.md) — how widgets get their colours.
- [Architecture](architecture.md) — the node/event model behind the builders.
- [Backends](backends.md) — what a widget becomes on each backend.
- [The Win32 layer](win32.md) — the native controls, if you need deeper Windows
  integration than the portable widgets give.
