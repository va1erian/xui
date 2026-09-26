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
//! widget is resized or a run changes, so a paint re-measures nothing and input
//! hit-tests the boxes the paint used.
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
#[cfg(test)]
mod tests;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::control::Control;
use crate::app::Ui;
use crate::backend::{Canvas, Cursor, Event, NodeKind, NodeSpec, Result, TextStyle};
use crate::geometry::{Point, Rect};
use crate::message::MouseButton;
use crate::theme::Theme;
use crate::units::{Dip, Px};

use layout::{Fragment, Layout as Block, Span};

/// The default em size of a run without an explicit size.
const DEFAULT_SIZE: Dip = Dip(14.0);
/// The width of the hover underline, in device pixels.
const UNDERLINE: f32 = 1.0;

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
    pub fn new(ui: &Ui<M>, bounds: Rect) -> Result<FlowText<M>> {
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
            control.set_painter(Rc::new(move |canvas| {
                paint(&inner, &hover, &theme.get(), canvas, selected.get());
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
                        relayout(&ui, &inner);
                        ui.invalidate(id);
                    }
                    Event::MouseMove { x, y, .. } => {
                        let link = link_at(&inner, *x, *y);
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
                        pressed.set(link_at(&inner, *x, *y));
                    }
                    Event::MouseUp {
                        x,
                        y,
                        button: MouseButton::Left,
                        ..
                    } => {
                        let released = link_at(&inner, *x, *y);
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

        relayout(ui, &inner);
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
        relayout(self.control.ui(), &self.inner);
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
        let runs = self.inner.runs.borrow();
        let spans: Vec<Span> = runs
            .iter()
            .enumerate()
            .map(|(index, run)| Span {
                run: index,
                text: &run.text,
                style: text_style(run, &Theme::light()),
            })
            .collect();
        let mut measure = |text: &str, style: &TextStyle| ui.measure_text(text, style, dpi);
        let block = layout::layout(&spans, width.to_px(dpi).value(), i32::MAX, &mut measure);
        Px(block.height.max(1)).to_dip(dpi)
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

/// The [`TextStyle`] for `run` with `theme`'s resolved colours.
fn text_style<M>(run: &Run<M>, theme: &Theme) -> TextStyle {
    let color = match run.style {
        RunStyle::Normal => theme.text,
        RunStyle::Weak => theme.text_secondary,
        RunStyle::Link => theme.accent,
    };
    let size = run.size_dip.map_or(DEFAULT_SIZE, Dip);
    TextStyle::new(color, size)
        .weight(run.weight)
        .italic(run.italic)
}

/// Rebuilds the cached layout from the runs and the node's last size.
fn relayout<M: 'static>(ui: &Ui<M>, inner: &FlowInner<M>) {
    let (width, height) = inner.size.get();
    let dpi = ui.dpi();
    let runs = inner.runs.borrow();
    let spans: Vec<Span> = runs
        .iter()
        .enumerate()
        .map(|(index, run)| Span {
            run: index,
            text: &run.text,
            style: text_style(run, &Theme::light()),
        })
        .collect();
    let mut measure = |text: &str, style: &TextStyle| ui.measure_text(text, style, dpi);
    let block = layout::layout(&spans, width, height, &mut measure);
    *inner.layout.borrow_mut() = Some(block);
}

/// The run of the link under `(x, y)`, if any.
fn link_at<M>(inner: &FlowInner<M>, x: i32, y: i32) -> Option<usize> {
    let layout = inner.layout.borrow();
    let layout = layout.as_ref()?;
    let runs = inner.runs.borrow();
    layout.fragment_at(x, y).and_then(|fragment| {
        let run = fragment.run?;
        (runs.get(run)?.style == RunStyle::Link).then_some(run)
    })
}

/// Paints the cached layout, with the hovered link underlined.
fn paint<M: 'static>(
    inner: &FlowInner<M>,
    hover: &Cell<Option<usize>>,
    theme: &Theme,
    canvas: &mut dyn Canvas,
    selected: bool,
) {
    let bounds = canvas.bounds();
    canvas.clear(theme.background);
    let layout = inner.layout.borrow();
    let Some(layout) = layout.as_ref() else {
        return;
    };
    let runs = inner.runs.borrow();
    let place = |rect: Rect| {
        Rect::new(
            bounds.left + rect.left,
            bounds.top + rect.top,
            bounds.left + rect.right,
            bounds.top + rect.bottom,
        )
    };
    for fragment in &layout.fragments {
        let mut style = fragment_style(fragment, &runs, theme);
        if !inner.enabled.get() {
            style.color = theme.text_disabled;
        }
        canvas.draw_text(&fragment.text, place(fragment.rect), &style);
    }
    if let Some(link) = hover.get() {
        for fragment in layout.fragments.iter().filter(|f| f.run == Some(link)) {
            let rect = place(fragment.rect);
            canvas.draw_line(
                Point::new(rect.left, rect.bottom - 1),
                Point::new(rect.right, rect.bottom - 1),
                theme.accent,
                UNDERLINE,
            );
        }
    }
    if selected {
        canvas.stroke_rect(bounds, theme.accent, 2.0);
    }
}

/// The style a fragment paints with: its run's, or weak for an ellipsis.
fn fragment_style<M>(fragment: &Fragment, runs: &[Run<M>], theme: &Theme) -> TextStyle {
    match fragment.run.and_then(|run| runs.get(run)) {
        Some(run) => text_style(run, theme),
        None => TextStyle::new(theme.text_secondary, DEFAULT_SIZE),
    }
}
