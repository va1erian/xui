#![forbid(unsafe_code)]

//! The painting and bounds-refresh half of [`FlowText`](super::FlowText):
//! measuring the runs into a wrapped block and drawing the cached fragments.

use std::cell::Cell;

use super::layout::{Fragment, Span};
use super::{FlowInner, Run, RunStyle};
use crate::app::Ui;
use crate::backend::{Canvas, TextStyle};
use crate::geometry::{Point, Rect};
use crate::theme::Theme;
use crate::theme::look::backdrop;
use crate::units::Dip;

/// The default em size of a run without an explicit size.
const DEFAULT_SIZE: Dip = Dip(14.0);
/// The width of the hover underline, in device pixels.
const UNDERLINE: f32 = 1.0;

/// The [`TextStyle`] for `run` with `theme`'s resolved colours.
pub(super) fn text_style<M>(run: &Run<M>, theme: &Theme) -> TextStyle {
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
pub(super) fn relayout<M: 'static>(ui: &Ui<M>, inner: &FlowInner<M>) {
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
    let block = super::layout::layout(&spans, width, height, &mut measure);
    *inner.layout.borrow_mut() = Some(block);
}

/// The run of the link under `(x, y)`, if any.
pub(super) fn link_at<M>(inner: &FlowInner<M>, x: i32, y: i32) -> Option<usize> {
    let layout = inner.layout.borrow();
    let layout = layout.as_ref()?;
    let runs = inner.runs.borrow();
    layout.fragment_at(x, y).and_then(|fragment| {
        let run = fragment.run?;
        (runs.get(run)?.style == RunStyle::Link).then_some(run)
    })
}

/// Paints the cached layout, with the hovered link underlined.
///
/// A container positions a widget with `apply_moves` and, on a backend whose
/// children are not real windows, sends no `Resize`; so re-wrap here whenever
/// the canvas reports a size the cached layout was not measured for.
pub(super) fn paint<M: 'static>(
    ui: &Ui<M>,
    inner: &FlowInner<M>,
    hover: &Cell<Option<usize>>,
    theme: &Theme,
    canvas: &mut dyn Canvas,
    selected: bool,
) {
    let bounds = canvas.bounds();
    let size = (bounds.width(), bounds.height());
    if inner.size.get() != size {
        inner.size.set(size);
        relayout(ui, inner);
    }
    backdrop(canvas, theme.background);
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
