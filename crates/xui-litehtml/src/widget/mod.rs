//! The owner-drawn [`HtmlWidget`] behind the public [`HtmlView`](crate::HtmlView)
//! (which lives in `view.rs`): an xui-win32 [`CustomWidget`] that owns a render
//! worker and paints its latest frame with Direct2D.
//!
//! Interaction (links, selection, copy, keyboard) is resolved here on the UI
//! thread from the frame's [`LinkTable`](crate::LinkTable) and
//! [`TextRunTable`](crate::TextRunTable), exactly like `egui-litehtml-webview`:
//! a click is reported by the same paint/input that saw it, no re-layout per
//! pointer move. The character boundary under the pointer, and the highlight
//! boxes, come from DirectWrite's own hit-testing/selection rects (see
//! [`Painter`](crate::Painter)) rather than the table's left-to-right `offsets`,
//! so right-to-left and complex text select accurately. Input dispatch lives in
//! `input.rs`.

use std::cell::{Cell, RefCell};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::time::{Duration, Instant};

use xui_win32::d2d::{D2dCanvas, RectF, TextSystem};
use xui_win32::gdi::Canvas;
use xui_win32::{Color, CustomWidget, Hwnd, Input, Rect as PxRect, Renderer, Theme, WidgetCx};

use crate::geom::{Point, Rect};
use crate::list::Frame;
use crate::paint::Painter;
use crate::selection::{Selection, TextPos};
use crate::worker::{Job, Output, RenderJob};

mod input;

/// How many device-independent pixels one wheel notch scrolls.
pub(super) const WHEEL_LINE_DIP: f32 = 60.0;
/// How many device-independent pixels one arrow-key press scrolls.
pub(super) const KEY_LINE_DIP: f32 = 40.0;
/// How many device-independent pixels a pointer must move to count as a drag.
pub(super) const DRAG_SLOP_DIP: f32 = 3.0;
/// Longest gap between clicks that still extends a multi-click.
const MULTI_CLICK: Duration = Duration::from_millis(500);
/// The translucent highlight fill over the page.
const SELECTION_FILL: xui_win32::d2d::Rgba =
    xui_win32::d2d::Rgba::with_alpha(0x33, 0x99, 0xFF, 0x80);

fn to_rectf(r: Rect) -> RectF {
    RectF::new(r.left, r.top, r.right, r.bottom)
}

/// The owner-drawn widget behind an [`HtmlView`](crate::HtmlView). All mutable
/// state lives in `Cell`/`RefCell` fields because the xui-win32 `CustomWidget`
/// trait hands the widget `&self`.
pub struct HtmlWidget {
    painter: RefCell<Painter>,
    tx: Sender<Job>,
    rx: Receiver<Output>,
    latest_id: Arc<AtomicU64>,
    submitted_id: Cell<u64>,
    requested_width: Cell<f32>,
    reset_images: Cell<bool>,
    dirty: Cell<bool>,
    failed: Cell<bool>,
    html: RefCell<String>,
    frame: RefCell<Option<Frame>>,
    /// What shows behind (and around) the document; white unless the host
    /// themes it.
    background: Cell<Color>,
    /// Vertical scroll offset in device-independent pixels.
    scroll: Cell<f32>,
    /// The viewport height as of the last paint, for page-scroll clamping.
    viewport_height: Cell<f32>,
    /// The selected text, as carets into the frame's run table.
    selection: Cell<Option<Selection>>,
    /// A primary-button selection drag is in progress.
    dragging: Cell<bool>,
    /// The drag moved past [`DRAG_SLOP_DIP`], so it is not a click.
    moved: Cell<bool>,
    /// Where the last primary button went down, in device-independent pixels.
    press: Cell<Option<Point>>,
    /// Multi-click state: 1, 2 or 3.
    click_count: Cell<u8>,
    /// When and where the last click registered, to extend a multi-click.
    last_click: Cell<Option<(Instant, Point)>>,
    /// Shift is held (tracked from key events, for Shift+click extend).
    shift_held: Cell<bool>,
    /// The window's scale from device to independent pixels (dpi / 96).
    scale: Cell<f32>,
    /// The top-level window, for clipboard ownership while copying.
    hwnd: Hwnd,
}

impl HtmlWidget {
    pub(crate) fn new(
        text: TextSystem,
        tx: Sender<Job>,
        rx: Receiver<Output>,
        latest_id: Arc<AtomicU64>,
        html: String,
        scale: f32,
        hwnd: Hwnd,
    ) -> Self {
        Self {
            painter: RefCell::new(Painter::new(text)),
            tx,
            rx,
            latest_id,
            submitted_id: Cell::new(0),
            requested_width: Cell::new(0.0),
            reset_images: Cell::new(false),
            dirty: Cell::new(true),
            failed: Cell::new(false),
            html: RefCell::new(html),
            frame: RefCell::new(None),
            background: Cell::new(Color::rgb(255, 255, 255)),
            scroll: Cell::new(0.0),
            viewport_height: Cell::new(0.0),
            selection: Cell::new(None),
            dragging: Cell::new(false),
            moved: Cell::new(false),
            press: Cell::new(None),
            click_count: Cell::new(0),
            last_click: Cell::new(None),
            shift_held: Cell::new(false),
            scale: Cell::new(scale),
            hwnd,
        }
    }

    /// Sets the colour shown behind the document.
    pub fn set_background(&self, color: Color) {
        self.background.set(color);
    }

    /// Load a new page, dropping the current frame immediately so a slow render
    /// does not show the previous page.
    pub fn load(&self, html: String) {
        *self.html.borrow_mut() = html;
        self.frame.borrow_mut().take();
        self.selection.set(None);
        self.scroll.set(0.0);
        self.failed.set(false);
        self.reset_images.set(true);
        self.dirty.set(true);
    }

    /// Whether the newest render has finished and its frame is available.
    pub fn is_ready(&self) -> bool {
        self.frame
            .borrow()
            .as_ref()
            .is_some_and(|f| f.id == self.submitted_id.get())
    }

    /// Sets the vertical scroll offset (device-independent pixels), clamped on
    /// the next paint.
    pub fn set_scroll(&self, y: f32) {
        self.scroll.set(y.max(0.0));
    }

    /// The selected text, as a copy should read, or `None` if nothing is
    /// selected.
    pub fn selected_text(&self) -> Option<String> {
        let frame = self.frame.borrow();
        let frame = frame.as_ref()?;
        let sel = self.selection.get().filter(|s| !s.is_empty())?;
        Some(frame.runs.selection_text(&sel)).filter(|t| !t.is_empty())
    }

    /// Whether any text is selected.
    pub fn has_selection(&self) -> bool {
        self.selected_text().is_some()
    }

    /// Select all the text of the page.
    pub fn select_all(&self) {
        let frame = self.frame.borrow();
        if let Some(frame) = frame.as_ref() {
            self.selection.set(frame.runs.select_all());
        }
    }

    /// Drop the selection.
    pub fn clear_selection(&self) {
        self.selection.set(None);
    }

    fn submit(&self, width: f32) {
        self.submitted_id.set(self.submitted_id.get() + 1);
        self.latest_id
            .store(self.submitted_id.get(), Ordering::SeqCst);
        let job = RenderJob {
            id: self.submitted_id.get(),
            html: Arc::from(self.html.borrow().clone()),
            width,
            reset_images: self.reset_images.take(),
        };
        if self.tx.send(Job::Render(job)).is_err() {
            self.failed.set(true);
        }
        self.requested_width.set(width);
        self.dirty.set(false);
    }

    fn poll(&self) {
        loop {
            match self.rx.try_recv() {
                Ok(Output::Frame(frame)) if frame.id == self.submitted_id.get() => {
                    // A new layout of the same text keeps the selection (carets
                    // are run indexes); a different page cannot.
                    let changed = match &*self.frame.borrow() {
                        Some(prev) => prev.runs.text_signature() != frame.runs.text_signature(),
                        None => true,
                    };
                    if changed {
                        self.selection.set(None);
                    }
                    *self.frame.borrow_mut() = Some(frame);
                }
                Ok(Output::Failed { id }) if id == self.submitted_id.get() => self.failed.set(true),
                // A frame or failure for a superseded job.
                Ok(_) => {}
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.failed.set(true);
                    break;
                }
            }
        }
    }

    fn content_height(&self) -> f32 {
        self.frame.borrow().as_ref().map_or(0.0, |f| f.list.size.1)
    }

    fn max_scroll(&self) -> f32 {
        (self.content_height() - self.viewport_height.get()).max(0.0)
    }

    fn scroll_by(&self, delta: f32) {
        let next = (self.scroll.get() + delta).clamp(0.0, self.max_scroll());
        self.scroll.set(next);
    }

    /// Converts a client-coordinate device pixel to a document point
    /// (device-independent pixel, scrolled into the page's space).
    fn doc_point(&self, p: Point) -> Point {
        Point::new(p.x, p.y + self.scroll.get())
    }

    /// The caret nearest `doc`, from DirectWrite hit-testing (see
    /// [`Painter::caret_at`]).
    fn caret_at(&self, doc: Point) -> Option<TextPos> {
        let frame = self.frame.borrow();
        let frame = frame.as_ref()?;
        self.painter
            .borrow_mut()
            .caret_at(&frame.list, &frame.runs, doc)
    }

    /// Extends a multi-click and returns the new click count (1, 2 or 3).
    fn register_click(&self, p: Point) -> u8 {
        let now = Instant::now();
        let same = self.last_click.get().is_some_and(|(t, last)| {
            now.duration_since(t) < MULTI_CLICK
                && (last.x - p.x).abs() < 4.0
                && (last.y - p.y).abs() < 4.0
        });
        let count = if same {
            self.click_count.get() % 3 + 1
        } else {
            1
        };
        self.click_count.set(count);
        self.last_click.set(Some((now, p)));
        count
    }

    /// Paints the selection highlight over the page, in document coordinates
    /// (the canvas is already translated by the scroll offset).
    fn paint_selection(&self, canvas: &mut D2dCanvas) {
        let Some(sel) = self.selection.get().filter(|s| !s.is_empty()) else {
            return;
        };
        let frame = self.frame.borrow();
        let Some(frame) = frame.as_ref() else {
            return;
        };
        let rects = self
            .painter
            .borrow_mut()
            .selection_rects(&frame.list, &frame.runs, &sel);
        for r in &rects {
            canvas.fill_rect_rgba(to_rectf(*r), SELECTION_FILL);
        }
    }

    /// Copies the selection to the clipboard.
    fn copy_selection(&self) {
        if let Some(text) = self.selected_text() {
            let _ = xui_win32::clipboard::set_text(self.hwnd, &text);
        }
    }
}

impl CustomWidget for HtmlWidget {
    type Event = crate::view::HtmlViewEvent;

    // This widget is Direct2D-only; the GDI fallback (used only when Direct2D
    // cannot create a surface) leaves the page blank.
    fn renderer(&self) -> Renderer {
        Renderer::Direct2D
    }

    fn paint(&self, _canvas: &Canvas, _bounds: PxRect, _theme: &Theme) {}

    fn paint_d2d(&self, canvas: &mut D2dCanvas, bounds: RectF, _theme: &Theme) {
        let width = bounds.width().max(1.0);
        let height = bounds.height().max(1.0);
        self.viewport_height.set(height);

        if self.dirty.get() || (width - self.requested_width.get()).abs() > 0.5 {
            self.submit(width);
        }
        self.poll();

        let scroll = self.scroll.get().clamp(0.0, self.max_scroll());
        self.scroll.set(scroll);

        let viewport = Rect::new(0.0, 0.0, width, height);
        match self.frame.borrow().as_ref() {
            Some(frame) => {
                self.painter.borrow_mut().paint(
                    &frame.list,
                    canvas,
                    viewport,
                    scroll,
                    self.background.get(),
                );
                self.paint_selection(canvas);
            }
            None => canvas.clear(self.background.get()),
        }
    }

    fn input(&self, input: Input, cx: &mut WidgetCx<crate::view::HtmlViewEvent>) {
        self.handle_input(input, cx);
    }

    fn preferred_size(&self, _dpi: u32) -> Option<xui_win32::Size> {
        None
    }
}
