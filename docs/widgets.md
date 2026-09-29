# Widgets

The portable widget layer lives in `xui_core::widget`. Every widget is a value
that owns one node in the window; you create it with a `&Ui<Msg>`, a `Rect`
(device pixels) and whatever it needs, chain `on_*` builders to map events to
your `Msg`, and keep it alive by storing it in your app. Dropping a widget
destroys its node.

```rust
use xui_core::widget::{Edit, Label};
use xui_core::Rect;

let name = Edit::new(ui, Rect::new(16, 16, 336, 48), "your name")
    .unwrap()
    .on_change(|text| Some(Msg::Name(text.to_string())));
let label = Label::new(ui, Rect::new(16, 56, 336, 88), "Hello").unwrap();
```

All constructors return `Result<Self, BackendError>` except `Menu::context`.
Event builders return `Self`, so they chain at construction.

## Common shape

- **No control ids.** Widgets are values; notifications are routed internally by
  `WidgetId`. There is no base trait — each widget has the methods it needs.
- **Events map to your `Msg`.** `on_click`, `on_select`, `on_change`,
  `on_toggle`, `on_activate`, … take a closure returning `Option<Msg>`. The
  runtime queues the message and calls `App::update`, never re-entered.
- **Design values are `Dip`.** Bounds you pass are device-pixel `Rect`s; convert
  design values with `Dip::to_px(ui.dpi())`.
- **`Properties`.** Most widgets expose a `Properties` surface
  (`property(name)`, `set_property(name, value)`) for generic tooling; it is not
  the primary API.
- **Models are borrowed.** `ListModel`, `TreeModel` and `GridModel` return
  borrowed data, so painting allocates nothing per visible item.

## Catalogue

### Text and display

| Widget | Constructor | Notable API |
|---|---|---|
| `Label<M>` | `Label::new(ui, rect, text)` | `HasText` (`text`/`set_text`), `set_selected` |
| `Hyperlink<M>` | `Hyperlink::new(ui, rect, text)` | `on_click(Fn() -> Option<M>)` |
| `FlowText<M>` | `FlowText::new(ui, rect)` | `run(Run<M>)`, `separator(&str)`, `preferred_height(Dip)`; runs are `Run::normal/weak/link`, with per-run `on_click` |
| `Icon` | enum | `draw_icon(canvas, Icon, rect, color, dpi)`; `Icon::{Plus, Minus, Close, Check, ChevronDown, ChevronUp, Search, More}` |
| `Separator<M>` | `Separator::new(ui, rect)`, `Separator::vertical(ui, rect)` | `orientation()` |
| `GroupBox<M>` | `GroupBox::new(ui, rect, title)` | non-interactive frame; place children as siblings |
| `ProgressBar<M>` | `ProgressBar::new(ui, rect, max)` | `value`/`set_value`, `set_max` |

### Buttons and choices

| Widget | Constructor | Events |
|---|---|---|
| `Button<M>` | `Button::new(ui, rect, text)` | `on_click`, `icon(Icon)` |
| `CheckBox<M>` | `CheckBox::new(ui, rect, text)` | `on_toggle(bool)`; `is_checked`/`set_checked` |
| `ToggleButton<M>` | `ToggleButton::new(ui, rect, text)` | `on_toggle(bool)` |
| `RadioGroup<M>` | `RadioGroup::new(ui, rect, &[labels])` | `on_select(usize)`; `selected`/`select` |
| `ColorPicker<M>` | `ColorPicker::new(ui, rect, &[Color])` | `columns(n)`, `selected(Color)`, `on_select(Color)` |

### Text entry and ranges

| Widget | Constructor | Events |
|---|---|---|
| `Edit<M>` | `Edit::new(ui, rect, text)` | `on_change(&str)`; native `EDIT` on Win32, painted elsewhere |
| `MultilineEdit<M>` | `MultilineEdit::new(ui, rect, text)` | `on_change(&str)` |
| `NumberField<M>` | `NumberField::new(ui, rect, min, max, step)` | `on_change(f64)`, `on_commit(f64)` |
| `Slider<M>` | `Slider::new(ui, rect, min, max)` | `on_change(f64)`, `on_commit(f64)`; `set_range` |
| `ComboBox<M>` | `ComboBox::new(ui, rect, &[items])` | `on_select(usize)`; `item_icon(index, impl Into<IconRef>)` / `set_item_icon(index, Option<IconRef>)` add a leading 16 DIP icon to an item, in the list and in the closed box while selected (`icon(index)` reads it back) |

### Collections

`ListView<M>` is a virtualized list. Build it from words or from a model:

```rust
use xui_core::widget::{ListView, ListModel};

let list = ListView::new(ui, rect, &["Inbox", "Sent", "Drafts"])
    .unwrap()
    .on_select(|row| Some(Msg::Open(row)))
    .on_activate(|row| Some(Msg::Open(row)));

// Later: swap the data, selection and scroll survive.
list.set_items(&["A", "B", "C"]);
```

- Builders: `column(title, width)`, `column_right`, `add_column(Column)`,
  `selection_mode(SelectionMode)`, `multi_select(bool)`, `on_select`,
  `on_selection(&[usize])`, `on_activate`, `on_context`, `on_sort`.
- Runtime: `set_model`/`set_items`, `selected`, `selection`, `select`,
  `set_selection`, `ensure_visible`, `set_sort_indicator`, `cell_text`,
  `column_count`.
- `with_model(ui, rect, impl ListModel)` takes a custom model; a `Vec<String>`
  and `Vec<Vec<String>>` implement `ListModel` already.
- `ListModel::icon(row) -> Option<IconRef>` (default `None`) gives a row a
  leading 16 DIP icon before its first column's text, e.g.
  `(row < errors).then(|| Lucide::CircleX.into())`. It is drawn in the row's
  text colour (on-accent when selected, disabled colour when disabled); a row
  without one keeps its text at the ordinary inset.

`TreeView<M>` is keyed and lazy, with checkboxes optional and indent guides
behind `indent_guides(bool)` (on by default):

```rust
use xui_core::widget::{Glyph, TreeRow, TreeView};

let tree = TreeView::new(ui, rect, &[
    TreeRow::new("Inbox", 0).expandable(true).expanded(true).icon(Glyph::Folder),
    TreeRow::new("Work", 1).icon(Glyph::Tag),
])
.unwrap()
.on_select(|id| Some(Msg::Open(id)))
.on_toggle(|id, expanded| Some(Msg::Fold(id, expanded)));
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
- `with_model(ui, rect, impl TreeModel)`: only roots are read up front; a
  branch's children are fetched on first expansion.

`GridView<M>` is a virtualized tile grid:

```rust
let grid = GridView::with_model(ui, rect, model)
    .unwrap()
    .tile_size(TileSize::new(Dip(120.0), Dip(120.0)).gap(Dip(8.0)))
    .on_activate(|i| Some(Msg::Open(i)))
    .on_paint_tile(|canvas, tile| { /* draw thumbnail + text */ });
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

let view = IconView::with_model(ui, rect, model)
    .unwrap()
    .on_select(|item| Some(Msg::Select(item)))
    .on_activate(|item| Some(Msg::Open(item)))
    .on_context(|item, at| Some(Msg::Context(item, at)));
// Small (16), Medium (32) or Large (48, the default); reflows and repaints.
view.set_icon_size(IconSize::Large);
```

- `IconModel` supplies each tile lazily: `items()`, `icon(item) -> Option<IconRef>`
  and `line(item, line) -> Option<&str>` for lines `0..3` (line 1 is the name in
  the normal text token, lines 2 and 3 are secondary details in the muted token).
  A `Vec<String>` and a `Vec<Vec<String>>` implement it already. Only visible
  tiles are laid out and painted, so a model of 100 000 items costs the same as
  ten.
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

let file = MenuId::new(1);
let menu = Menu::bar(ui, rect)
    .unwrap()
    .on_select(move |id| (id == file).then_some(Msg::Open))
    .build(|m| {
        m.submenu(MenuId::new(0), "&File", |f| {
            f.item(MenuId::new(1), "&Open");
            f.separator();
            f.check(MenuId::new(2), "Auto &save", true);
        });
    });

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

Containers own their children; create children through the container's scoped
`ui()` so they become nodes of that container (clipped to it, destroyed with it).

| Container | Constructor | API |
|---|---|---|
| `Panel<M>` | `Panel::new(ui, rect)` | `ui()` scopes children; caller positions them; `set_design_mode(bool)` makes the subtree ignore input (form designers; also on `ScrollView`, `Split`, `Tabs`) |
| `ScrollView<M>` | `ScrollView::new(ui, rect)` | `add(WidgetId, Dip)` registers a row height, `scroll_to(Px)`, `on_scroll(Fn(Px))`, `content_height()` |
| `Split<M>` | `Split::row` / `Split::column` | `pane_a(&[ids])`, `pane_b(&[ids])`, `set_position(Dip)`, `set_min`, `on_moved(Fn(Dip))` |
| `Tabs<M>` | `Tabs::new(ui, rect)` | `page(title, &[ids])`, `add_page`/`remove_page(index)`/`rename_page(index, title)`, `select(index)`/`selected`, `on_change(Fn(usize))` |

```rust
let panel = Panel::new(ui, rect).unwrap();
// Children created through `panel.ui()` are positioned in the panel's own
// coordinates and clipped to it.
let name = Edit::new(panel.ui(), Rect::new(8, 8, 300, 40), "").unwrap();
panel.set_bounds(Rect::new(0, 0, 336, 120)); // move/resize the panel node
```

### Status and chrome

| Widget | Constructor | Notes |
|---|---|---|
| `Toolbar<M>` | `Toolbar::new(ui, rect, &[labels])` or `Toolbar::empty` + `item`/`item_with_text`/`separator` | `on_click(usize)` counts items, not separators; buttons are content-sized and left-packed, `fill()` splits the width equally; items that do not fit are clipped (no overflow menu yet) |
| `StatusBar<M>` | `StatusBar::new(ui, rect, &[parts])` | `set_text(part, text)`, `set_parts` |
| `MaterialStatusBar<M>` | `MaterialStatusBar::new(ui, rect, &[parts])` | status bar drawn on a material band |
| `TopBar<M>` | `TopBar::new(ui, rect)` | `icon`/`toggle`/`label`/`slider`/`spacer`, keyed by `TopBarId`; `width`/`expand` resize an item; `on_click`/`on_toggle`/`on_change`. `Glyph` icons are vendored [Lucide](https://lucide.dev) outlines (ISC; see the README credits) and include the transport set (`Play`, `Pause`, `Stop`, `Previous`, `Next`, `Repeat`, `Shuffle`) and the navigation set (`Audio`, `Album`, `People`, `Tag`, `Folder`, `Star`, `StarFilled`, `History`, `Monitor`, `Settings`) |

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
