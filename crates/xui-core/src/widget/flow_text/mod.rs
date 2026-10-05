#![forbid(unsafe_code)]

//! [`FlowText`]: a wrapped line of styled, optionally clickable runs.
//!
//! Runs are laid out left to right as one wrapped block, so a word wrap crosses
//! run boundaries. Each run is styled from theme tokens: normal, weak
//! (de-emphasised) or link. A link is accent-coloured, gets the hand cursor on
//! hover, is underlined while hovered, and emits its own mapped `Msg` on click
//! through the same per-window queue as every other widget.
//!
//! The wrapped layout is measured through
//! [`Ui::measure_text`](crate::app::Ui::measure_text) and painted with
//! [`Canvas::draw_text`](crate::backend::Canvas::draw_text), so it needs no
//! backend-specific text primitive; it is cached and only rebuilt when the
//! widget's bounds change or a run changes (a move that resizes the node
//! re-wraps even on a backend that sends no `Resize`), so a paint re-measures
//! nothing and input hit-tests the boxes the paint used.
//!
//! ```ignore
//! FlowText::new(ui, Rect::new(0, 0, 400, 60))?
//!     .run(Run::link(&artist).on_click(|| Some(Msg::GoToArtist(artist.clone()))))
//!     .separator(" · ")
//!     .run(Run::link(&album).on_click(|| Some(Msg::GoToAlbum(album.clone()))))
//!     .separator(" · ")
//!     .run(Run::weak(format!("({year})")))
//! ```

mod layout;
mod place;
mod render;
#[cfg(test)]
mod tests;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::control::Control;
use crate::app::Ui;
use crate::backend::{Cursor, Event, NodeKind, NodeSpec, Result, TextStyle};
use crate::geometry::Rect;
use crate::message::MouseButton;
use crate::theme::Theme;
use crate::units::{Dip, Px};

use layout::{Layout as Block, Span};

/// How a [`Run`] is styled from the theme.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunStyle {
    /// Primary text ([`Theme::text`]).
    Normal,
    /// De-emphasised text ([`Theme::text_secondary`]).
    Weak,
    /// A clickable link ([`Theme::accent`], hand cursor, underlined on hover).
    Link,
}

/// One run of a [`FlowText`] line.
///
/// Build it with [`Run::normal`], [`Run::weak`] or [`Run::link`], and map a
/// click with [`Run::on_click`] (only meaningful for a link).
pub struct Run<M> {
    text: String,
    style: RunStyle,
    weight: u16,
    italic: bool,
    size_dip: Option<f32>,
    on_click: Option<Box<dyn Fn() -> Option<M>>>,
}

impl<M> Run<M> {
    fn new(text: impl Into<String>, style: RunStyle) -> Run<M> {
        Run {
            text: text.into(),
            style,
            weight: 400,
            italic: false,
            size_dip: None,
            on_click: None,
        }
    }

    /// A primary-text run.
    pub fn normal(text: impl Into<String>) -> Run<M> {
        Run::new(text, RunStyle::Normal)
    }

    /// A de-emphasised run.
    pub fn weak(text: impl Into<String>) -> Run<M> {
        Run::new(text, RunStyle::Weak)
    }

    /// A clickable link run.
    pub fn link(text: impl Into<String>) -> Run<M> {
        Run::new(text, RunStyle::Link)
    }

    /// Sets the font weight (100-900).
    pub fn weight(mut self, weight: u16) -> Run<M> {
        self.weight = weight;
        self
    }

    /// Sets italic.
    pub fn italic(mut self, italic: bool) -> Run<M> {
        self.italic = italic;
        self
    }

    /// Sets the em size in design units. Runs of different sizes share one
    /// baseline.
    pub fn size(mut self, size_dip: f32) -> Run<M> {
        self.size_dip = Some(size_dip);
        self
    }

    /// Maps a click on this run to the app's message.
    pub fn on_click(mut self, click: impl Fn() -> Option<M> + 'static) -> Run<M> {
        self.on_click = Some(Box::new(click));
        self
    }
}

/// The widget's mutable state, shared with its painter and event mapper.
struct FlowInner<M> {
    runs: RefCell<Vec<Run<M>>>,
    /// The cached layout, rebuilt only on a resize or a run change.
    layout: RefCell<Option<Block>>,
    /// The last node size in device pixels.
    size: Cell<(i32, i32)>,
    enabled: Cell<bool>,
}

/// A wrapped line of styled, optionally clickable runs.
pub struct FlowText<M: 'static> {
    control: Control<M>,
    inner: Rc<FlowInner<M>>,
    hover: Rc<Cell<Option<usize>>>,
    pressed: Rc<Cell<Option<usize>>>,
}

impl<M: 'static> FlowText<M> {
    /// Creates an empty flow line at `bounds`, adopting `ui`'s theme.
    pub(crate) fn new(ui: &Ui<M>, bounds: Rect) -> Result<FlowText<M>> {
        let control = Control::new(ui, &NodeSpec::new(NodeKind::Label, bounds))?;
        let inner = Rc::new(FlowInner {
            runs: RefCell::new(Vec::new()),
            layout: RefCell::new(None),
            size: Cell::new((bounds.width(), bounds.height())),
            enabled: Cell::new(true),
        });
        let hover = Rc::new(Cell::new(None));
        let pressed = Rc::new(Cell::new(None));

        {
            let inner = Rc::clone(&inner);
            let hover = Rc::clone(&hover);
            let theme = ui.theme_handle();
            let selected = control.selected_handle();
            let ui = ui.clone();
            control.set_painter(Rc::new(move |canvas| {
                render::paint(&ui, &inner, &hover, &theme.get(), canvas, selected.get());
            }));
        }

        {
            let inner = Rc::clone(&inner);
            let hover = Rc::clone(&hover);
            let pressed = Rc::clone(&pressed);
            let ui = ui.clone();
            let id = control.id();
            control.on_events(move |event| {
                // In design mode the editor handles input, not the widget.
                if ui.is_design_mode() && event.is_input() {
                    return None;
                }
                if !inner.enabled.get() {
                    return None;
                }
                match event {
                    Event::Resize { width, height } => {
                        inner.size.set((*width, *height));
                        render::relayout(&ui, &inner);
                        ui.invalidate(id);
                    }
                    Event::MouseMove { x, y, .. } => {
                        let link = render::link_at(&inner, *x, *y);
                        if hover.replace(link) != link {
                            ui.invalidate(id);
                        }
                        ui.set_cursor(
                            id,
                            if link.is_some() {
                                Cursor::Hand
                            } else {
                                Cursor::Default
                            },
                        );
                    }
                    Event::MouseLeave | Event::CaptureChanged => {
                        pressed.set(None);
                        if hover.replace(None).is_some() {
                            ui.invalidate(id);
                        }
                    }
                    Event::MouseDown {
                        x,
                        y,
                        button: MouseButton::Left,
                        ..
                    } => {
                        pressed.set(render::link_at(&inner, *x, *y));
                    }
                    Event::MouseUp {
                        x,
                        y,
                        button: MouseButton::Left,
                        ..
                    } => {
                        let released = render::link_at(&inner, *x, *y);
                        if pressed.replace(None) == released
                            && let Some(run) = released
                        {
                            let message = {
                                let runs = inner.runs.borrow();
                                runs.get(run)
                                    .and_then(|run| run.on_click.as_ref())
                                    .and_then(|click| click())
                            };
                            return message;
                        }
                    }
                    _ => {}
                }
                None
            });
        }

        render::relayout(ui, &inner);
        Ok(FlowText {
            control,
            inner,
            hover,
            pressed,
        })
    }

    /// Appends `run` to the line.
    pub fn run(self, run: Run<M>) -> FlowText<M> {
        self.inner.runs.borrow_mut().push(run);
        render::relayout(self.control.ui(), &self.inner);
        self.control.invalidate();
        self
    }

    /// Appends a weak separator run (for example `" · "`).
    pub fn separator(self, text: &str) -> FlowText<M> {
        self.run(Run::weak(text))
    }

    /// The height the runs wrap to at `width`, in design units, so a layout can
    /// size the widget to its wrapped content.
    pub fn preferred_height(&self, width: Dip) -> Dip {
        let ui = self.control.ui();
        let dpi = ui.dpi();
        let block = self.wrapped(ui, dpi, width.to_px(dpi).value());
        Px(block.height.max(1)).to_dip(dpi)
    }

    /// The runs wrapped at `width` device pixels.
    fn wrapped(&self, ui: &Ui<M>, dpi: u32, width: i32) -> Block {
        let runs = self.inner.runs.borrow();
        let spans: Vec<Span> = runs
            .iter()
            .enumerate()
            .map(|(index, run)| Span {
                run: index,
                text: &run.text,
                style: render::text_style(run, &Theme::light()),
            })
            .collect();
        let mut measure = |text: &str, style: &TextStyle| ui.measure_text(text, style, dpi);
        layout::layout(&spans, width, i32::MAX, &mut measure)
    }

    /// The line's node identity.
    pub fn id(&self) -> crate::backend::WidgetId {
        self.control.id()
    }

    /// Enables or disables the line. A disabled line is dimmed and ignores
    /// input.
    pub fn set_enabled(&self, enabled: bool) {
        self.inner.enabled.set(enabled);
        if !enabled {
            self.hover.set(None);
            self.pressed.set(None);
        }
        self.control.set_enabled(enabled);
        self.control.invalidate();
    }

    /// Marks the line selected, so its painter draws an outline (a form
    /// editor's selection).
    pub fn set_selected(&self, selected: bool) {
        self.control.set_selected(selected);
    }
}
