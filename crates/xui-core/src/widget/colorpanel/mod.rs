#![forbid(unsafe_code)]

//! [`ColorPanel`]: a tabbed colour picker — a **Simple** basic-colour grid and a
//! **Full** HSV picker with editable HEX/RGB/CMYK/HSV/HSL boxes.
//!
//! The panel keeps an [`Hsv`] triple as its source of truth, so hue survives a
//! trip through black or white, and emits the chosen [`Color`] through
//! [`ColorPanel::on_change`] (every change) and [`ColorPanel::on_commit`] (a
//! mouse-up, Enter, a swatch pick or a valid text commit).
//!
//! ```no_run
//! # use xui_core::app::Ui;
//! # use xui_core::geometry::Rect;
//! # use xui_core::widget::ColorPanel;
//! # use xui_core::{Color, Dip};
//! # let ui: Ui<()> = unimplemented!();
//! let panel = ColorPanel::new(&ui, Rect::default())
//!     .unwrap()
//!     .with_color(Color::rgb(0xEB, 0x40, 0x34))
//!     .on_change(|_color| None)
//!     .on_commit(|_color| None);
//! ```

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::button::Button;
use super::colorpicker::ColorPicker;
use super::control::Control;
use super::edit::Edit;
use super::groupbox::GroupBox;
use super::panel::Panel;
use super::tabs::Tabs;
use crate::Color;
use crate::app::Ui;
use crate::backend::{Canvas, NodeKind, NodeSpec, Result, WidgetId};
use crate::geometry::Rect;
use crate::theme::{Theme, Themed};

mod field;
mod hue;
mod layout;
mod model;
mod state;
mod text;

pub use field::ColorField;
pub use hue::HueSlider;
pub use model::{BASIC_COLORS, Hsv};

use crate::theme::look::backdrop;
use model::{Hsv as HsvModel, format_hex};
use state::{Shared, end_drags, set_hue, sync};
use text::Field;

/// A tabbed colour picker.
pub struct ColorPanel<M: 'static> {
    control: Control<M>,
    ui: Ui<M>,
    shared: Rc<Shared<M>>,
    tabs: Tabs<M>,
    swatches: ColorPicker<M>,
    simple: Panel<M>,
    panel: Panel<M>,
    field: ColorField<M>,
    hue: HueSlider<M>,
    preview: Control<M>,
    boxes: Vec<(GroupBox<M>, Rc<Edit<M>>)>,
    copy: Button<M>,
    nodes: Vec<WidgetId>,
}

impl<M: 'static> ColorPanel<M> {
    /// Creates a panel at `bounds`, initially black.
    pub fn new(ui: &Ui<M>, bounds: Rect) -> Result<ColorPanel<M>> {
        let dpi = ui.dpi();
        let control = Control::new(ui, &NodeSpec::new(NodeKind::Container, bounds))?;
        {
            let theme = ui.theme_handle();
            control.set_painter(Rc::new(move |canvas: &mut dyn Canvas| {
                backdrop(canvas, theme.get().background);
            }));
        }
        let scoped = ui.with_parent(control.id());
        let local = Rect::from_size(bounds.size());

        let hsv = Rc::new(Cell::new(Hsv::new(0.0, 0.0, 0.0)));
        let hue = Rc::new(Cell::new(0.0));

        let tabs = Tabs::new(&scoped, local)?;
        // The swatch grid lives in its own panel at the panel's origin: the
        // grid's own hit-testing subtracts its parent-relative left/top, so a
        // non-zero origin would offset every click.
        let simple = Panel::new(tabs.ui(), Rect::default())?;
        let swatches = ColorPicker::new(simple.ui(), Rect::default(), &BASIC_COLORS)?;
        let panel = Panel::new(tabs.ui(), Rect::default())?;

        // `page` relayouts and moves the panels into the page area, so their
        // bounds are known before the Full tab's children are placed.
        let tabs = tabs
            .page("Simple", &[simple.id()])
            .page("Full", &[panel.id()]);
        // The Simple page is selected first, so its panel is the one Tabs has
        // already moved into the page area; its size is the page size.
        let page = ui.bounds(simple.id());
        ui.apply_moves(&[(swatches.id(), Rect::from_size(page.size()))]);
        let layout = layout::full(Rect::from_size(page.size()), dpi);

        let panel_ui = panel.ui().clone();
        let preview = Control::new(&panel_ui, &NodeSpec::new(NodeKind::Custom, layout.preview))?;
        {
            let hsv = Rc::clone(&hsv);
            let theme = ui.theme_handle();
            preview.set_painter(Rc::new(move |canvas: &mut dyn Canvas| {
                let theme = theme.get();
                let bounds = canvas.bounds();
                canvas.fill_rect(bounds, hsv.get().to_color());
                canvas.stroke_rect(bounds, theme.input_border, 1.0);
            }));
        }
        let field = ColorField::with_state(&panel_ui, layout.field, Rc::clone(&hsv))?;
        let hue_slider = HueSlider::with_state(&panel_ui, layout.hue, Rc::clone(&hue))?;
        let field_drag = field.dragging();
        let hue_drag = hue_slider.dragging();

        let order = [Field::Hex, Field::Rgb, Field::Cmyk, Field::Hsv, Field::Hsl];
        let mut boxes = Vec::with_capacity(order.len());
        for (index, kind) in order.iter().enumerate() {
            let group = GroupBox::new(&panel_ui, layout.groups[index], kind.title())?;
            let edit = Rc::new(Edit::new(
                &panel_ui,
                layout.edits[index],
                &kind.format(hsv.get()),
            )?);
            boxes.push((group, edit));
        }
        let copy = Button::new(&panel_ui, layout.copy, "Copy")?;

        let shared = Rc::new(Shared {
            hsv: Rc::clone(&hsv),
            hue: Rc::clone(&hue),
            preview: Cell::new(preview.id()),
            field: Cell::new(field.id()),
            hue_node: Cell::new(hue_slider.id()),
            texts: RefCell::new(
                order
                    .iter()
                    .zip(boxes.iter())
                    .map(|(kind, (_, edit))| (*kind, Rc::downgrade(edit)))
                    .collect(),
            ),
            on_change: RefCell::new(None),
            on_commit: RefCell::new(None),
        });

        let swatches = {
            let shared = Rc::clone(&shared);
            let ui = ui.clone();
            swatches.on_select(move |color| {
                let triple = HsvModel::from_color(color);
                shared.hsv.set(triple);
                sync(&ui, &shared, None);
                if let Some(message) = shared.change(color) {
                    ui.emit(message);
                }
                shared.commit(color)
            })
        };
        let field = {
            let ui_change = ui.clone();
            let shared_change = Rc::clone(&shared);
            let ui_commit = ui.clone();
            let shared_commit = Rc::clone(&shared);
            field
                .on_change(move |triple| {
                    shared_change.hsv.set(triple);
                    sync(&ui_change, &shared_change, None);
                    shared_change.change(triple.to_color())
                })
                .on_commit(move |triple| {
                    shared_commit.hsv.set(triple);
                    sync(&ui_commit, &shared_commit, None);
                    shared_commit.commit(triple.to_color())
                })
        };
        let hue_slider = {
            let ui_change = ui.clone();
            let shared_change = Rc::clone(&shared);
            let ui_commit = ui.clone();
            let shared_commit = Rc::clone(&shared);
            hue_slider
                .on_change(move |degrees| set_hue(&ui_change, &shared_change, degrees, false))
                .on_commit(move |degrees| set_hue(&ui_commit, &shared_commit, degrees, true))
        };
        let copy = {
            let shared = Rc::clone(&shared);
            let ui = ui.clone();
            copy.on_click(move || {
                let color = shared.hsv.get().to_color();
                ui.set_clipboard_text(&format_hex(color));
                None
            })
        };
        let tabs = {
            let ui = ui.clone();
            tabs.on_change(move |_index| {
                end_drags(&ui, &field_drag, &hue_drag);
                None
            })
        };
        for (kind, (_, edit)) in order.iter().zip(boxes.iter()) {
            let ui = ui.clone();
            let shared = Rc::clone(&shared);
            let mapper_ui = ui.clone();
            let kind = *kind;
            ui.add_events(edit.id(), move |event| {
                text::on_event(&mapper_ui, &shared, kind, event)
            });
        }

        let mut nodes = vec![
            control.id(),
            tabs.id(),
            swatches.id(),
            simple.id(),
            panel.id(),
            preview.id(),
            field.id(),
            hue_slider.id(),
            copy.id(),
        ];
        for (group, edit) in &boxes {
            nodes.push(group.id());
            nodes.push(edit.id());
        }

        Ok(ColorPanel {
            control,
            ui: ui.clone(),
            shared,
            tabs,
            swatches,
            simple,
            panel,
            field,
            hue: hue_slider,
            preview,
            boxes,
            copy,
            nodes,
        })
    }

    /// Sets the initial colour without raising a change event.
    pub fn with_color(self, color: Color) -> ColorPanel<M> {
        self.set_color(color);
        self
    }

    /// Maps every colour change (including drag ticks) to the app's message.
    pub fn on_change(self, mapper: impl Fn(Color) -> Option<M> + 'static) -> ColorPanel<M> {
        *self.shared.on_change.borrow_mut() = Some(Rc::new(mapper));
        self
    }

    /// Maps a committed colour (mouse-up, Enter, a swatch pick or a valid text
    /// commit) to the app's message.
    pub fn on_commit(self, mapper: impl Fn(Color) -> Option<M> + 'static) -> ColorPanel<M> {
        *self.shared.on_commit.borrow_mut() = Some(Rc::new(mapper));
        self
    }

    /// The chosen colour.
    pub fn color(&self) -> Color {
        self.shared.hsv.get().to_color()
    }

    /// Sets the colour from code without raising a change event, updating every
    /// view.
    pub fn set_color(&self, color: Color) {
        self.shared.hsv.set(HsvModel::from_color(color));
        sync(&self.ui, &self.shared, None);
        self.swatches.select(color);
    }

    /// The selected tab (`0` Simple, `1` Full).
    pub fn tab(&self) -> usize {
        self.tabs.selected()
    }

    /// Selects the Simple (`0`) or Full (`1`) tab.
    pub fn select_tab(&self, index: usize) {
        self.end_drags();
        self.tabs.select(index);
    }

    /// The panel's node identity.
    pub fn id(&self) -> WidgetId {
        self.control.id()
    }

    /// Moves/resizes the panel and re-lays its children out.
    pub fn set_bounds(&self, bounds: Rect) {
        self.control.set_bounds(bounds);
        self.arrange(bounds);
    }

    /// Re-lays the children for the panel's node at `bounds`.
    fn arrange(&self, bounds: Rect) {
        self.tabs.set_bounds(Rect::from_size(bounds.size()));
        let page = self.current_page();
        self.apply_full(page);
    }

    /// The page rectangle of the currently selected tab, in the tab's own
    /// coordinates.
    fn current_page(&self) -> Rect {
        if self.tabs.selected() == 0 {
            self.ui.bounds(self.simple.id())
        } else {
            self.ui.bounds(self.panel.id())
        }
    }

    /// Shows or hides the panel, ending any drag in progress.
    pub fn set_visible(&self, visible: bool) {
        if !visible {
            self.end_drags();
        }
        self.control.set_visible(visible);
        self.tabs.set_visible(visible);
        self.simple.set_visible(visible);
    }

    /// Enables or disables the panel, ending any drag in progress.
    pub fn set_enabled(&self, enabled: bool) {
        if !enabled {
            self.end_drags();
        }
        self.control.set_enabled(enabled);
        self.tabs.set_enabled(enabled);
        self.simple.set_enabled(enabled);
    }

    /// Re-lays the Full tab's children from the current page size.
    fn apply_full(&self, page: Rect) {
        let layout = layout::full(Rect::from_size(page.size()), self.ui.dpi());
        let mut moves = vec![
            (self.preview.id(), layout.preview),
            (self.field.id(), layout.field),
            (self.hue.id(), layout.hue),
            (self.copy.id(), layout.copy),
            (self.swatches.id(), Rect::from_size(page.size())),
        ];
        for (index, (group, edit)) in self.boxes.iter().enumerate() {
            moves.push((group.id(), layout.groups[index]));
            moves.push((edit.id(), layout.edits[index]));
        }
        self.ui.apply_moves(&moves);
    }

    /// Ends a drag in progress on the field or the hue slider.
    fn end_drags(&self) {
        end_drags(&self.ui, &self.field.dragging(), &self.hue.dragging());
    }
}

mod place;
#[cfg(test)]
mod test_support;

impl<M: 'static> Themed for ColorPanel<M> {
    fn apply_theme(&self, _theme: &Theme) {
        for id in &self.nodes {
            self.ui.invalidate(*id);
        }
    }
}

#[cfg(test)]
mod tests;
