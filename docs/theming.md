# Theming

Dark mode is first-class: widgets paint only from semantic theme tokens, so the
application picks a `Theme` and everything follows. Every backend shares the same token set.

## The palette

`xui_core::Theme` is a small, complete semantic palette. It is `Copy` and
`PartialEq`, so it is cheap to store and compare.

| Token | Meaning |
|---|---|
| `is_dark` | Whether this is the dark variant |
| `background`, `surface`, `raised` | Window, card and elevated surfaces |
| `text`, `text_secondary`, `text_disabled`, `text_on_accent` | Foreground levels |
| `accent` | The system accent for selection and emphasis |
| `warning`, `danger` | Semantic status colours |
| `selection`, `selection_unfocused` | Selected rows and items |
| `hover`, `pressed` | Interaction states |
| `border`, `border_focused` | Separators and focus rings |
| `shadow` | Popup elevation |
| `input_background` | Text fields and other inputs |
| `input_border` | Border around inputs (fields, check boxes) |
| `scrollbar`, `scrollbar_track` | Scrollbars |
| `scrim` | The translucent modal backdrop a `Dialog`/`TaskDialog` paints over the window |

`Theme::light()` and `Theme::dark()` sample Windows 11 Explorer/Settings/WinUI
values; each field documents its source. `Theme::default()` is light.

```rust
use xui_core::theme::Theme;

let theme = Theme::dark();
```

## Applying a theme

For a portable app, call `Ui::set_theme` with the live theme:

```rust
fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
    match msg {
        Msg::Dark => ui.set_theme(Theme::dark()),
        Msg::Light => ui.set_theme(Theme::light()),
    }
}
```

`set_theme` stores the theme on the window and calls `Backend::set_theme`, so
the backend can update native parts (title bar, scrollbars, controls). Widgets
that were already built adopt the new theme on their next paint: they read the
window's live theme through a shared handle captured at construction, so nothing
is rebuilt and no event is needed.

To start a window dark, call `ui.set_theme(Theme::dark())` inside `run_app`'s
`make` closure, before creating widgets.

## Painting from tokens

A custom/painted widget implements `Themed` (or reads the theme handle) and
draws only from tokens:

```rust
use xui_core::theme::{Theme, Themed};

impl Themed for Swatch {
    fn apply_theme(&self, theme: &Theme) {
        self.background.set(theme.surface);
        self.border.set(theme.border);
        self.text.set(theme.text);
    }
}
```

`Theme` is exposed on the widget-facing `Canvas` APIs and on `Ui::theme()` /
`Ui::theme_handle()`. `Themed::apply_theme` is the hook a backend calls when the
window's theme changes; bundled portable widgets generally read the live theme
in their painter instead.

## Decoration and Midnight

Besides colours, a theme carries decoration tokens: `background_end` and
`surface_end` (the bottoms of the window and card gradients), `bevel` (a 1px
top highlight), `shade` (how far a control face darkens toward its bottom),
`corner_radius` (cards and group boxes) and `glow` (the accent glow around a
checked or selected indicator). `Theme::light()` and `Theme::dark()` leave them
all off, so they paint flat exactly as before; `Theme::midnight()` is a navy
dark theme that turns them on.

`xui_core::theme::look` paints them, so a custom widget looks like the bundled
ones: `paint_background` (the window gradient), `backdrop`, `card`, `band`,
`face`, `field`, `row`/`selected_row` and `glow`.

A widget starts its painter with `look::backdrop(canvas, theme.background)`
instead of `canvas.clear(...)`. A compositing backend (`xui-canvas`, and an
embedder that paints nodes in creation order) reports
`Canvas::composites_parents()`: the widget's container is already on the
surface, so the backdrop does nothing and the widget sits on its card or the
window gradient. The Win32 backend repaints a widget alone into its own
buffer, so there the backdrop fills with the colour given.

## Follow the system (Windows)

The Win32 backend adds a `SystemTheme` extension trait over `Theme`:

```rust
use xui_win32::SystemTheme;

let theme = Theme::system();      // reads the registry / DWM / high-contrast
```

It reads the user's light/dark preference, the accent colour and the high-contrast
flag through documented APIs. `is_theme_change(message)` identifies the window
message that means the preference changed, and a Win32 window can
`follow_system_theme`. See [The Win32 layer](win32.md).

## Dark-mode notes

- A backend that owns native parts re-themes them centrally; the app never calls
  `SetWindowTheme` or handles `WM_CTLCOLOR*`.
- Where a native part ignores dark mode, a backend owner-draws it from the same
  tokens. The Win32 details (list headers, scrollbars, tooltips) are in
  [The Win32 layer](win32.md).
- The canvas backend paints everything itself, so it is dark-mode-complete by
  construction.

## Related

- [Widgets](widgets.md) — the widgets that consume the tokens.
- [The Win32 layer](win32.md) — system theme, backdrop materials and the
  extended title bar.
