//! The public [`HtmlView`]: an HTML view hosted as a custom-painted portable
//! node, rendered on a worker thread with litehtml (see the crate docs and
//! `widget.rs`'s `HtmlWidget`).

use std::rc::Rc;
use std::sync::atomic::AtomicU64;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

use xui_core::Color;
use xui_core::app::Ui;
use xui_core::backend::{NodeKind, NodeSpec, Result};
use xui_core::geometry::Rect;
use xui_core::widget::Control;

use crate::text::TextSystem;
use crate::widget::HtmlWidget;
use crate::worker::{ImageFetcher, ImageSource, Worker};

/// An event raised by an [`HtmlView`], mapped to the app's message type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HtmlViewEvent {
    /// The user clicked a link (an `<a href>`). litehtml has no navigation
    /// concept of its own, so the host decides what to do with it.
    LinkClicked(String),
    /// The user pressed Ctrl+C with text selected; carries the text as a copy
    /// should read. The crate has no clipboard of its own, so the host puts it
    /// on the platform clipboard.
    CopyRequested(String),
}

/// An HTML view hosted as a custom-painted node, rendered on a worker thread
/// with litehtml and drawn through the portable canvas.
///
/// The worker is created here and lives for the view's lifetime; `on_frame` is
/// the app message that tells the host a new frame is ready (mapped through a
/// `Proxy`, so the UI thread is never blocked; the app answers it by calling
/// [`HtmlView::invalidate`]), and `on_event` maps the view's
/// [`HtmlViewEvent`]s to the app's `Msg`.
pub struct HtmlView<M: 'static> {
    control: Control<M>,
    widget: Rc<HtmlWidget>,
    images: ImageSource,
}

impl<M: Send + 'static> HtmlView<M> {
    /// Creates the view at `bounds` (device pixels), loading `html`, and starts
    /// its worker thread.
    pub fn new(
        ui: &Ui<M>,
        bounds: Rect,
        html: String,
        on_frame: impl Fn() -> M + Send + Sync + 'static,
        on_event: impl Fn(HtmlViewEvent) -> Option<M> + 'static,
    ) -> Result<HtmlView<M>> {
        let text = TextSystem::new(ui.text_shaper());
        let (job_tx, job_rx) = mpsc::channel();
        let (out_tx, out_rx) = mpsc::channel();
        let latest_id = Arc::new(AtomicU64::new(0));
        let scale = ui.dpi() as f32 / 96.0;
        let images: ImageSource = Arc::new(Mutex::new(None));
        let worker_images = Arc::clone(&images);

        let control = Control::new(ui, &NodeSpec::new(NodeKind::Custom, bounds))?;
        let widget = Rc::new(HtmlWidget::new(
            text.clone(),
            job_tx.clone(),
            out_rx,
            latest_id.clone(),
            html,
            scale,
        ));

        {
            let widget = Rc::clone(&widget);
            let theme = ui.theme_handle();
            control.set_painter(Rc::new(move |canvas| widget.paint(canvas, theme.get())));
        }
        {
            let widget = Rc::clone(&widget);
            let ui = ui.clone();
            let id = control.id();
            control.on_events(move |event| {
                if !event.is_input() {
                    return None;
                }
                let effects = widget.handle_input(event);
                if effects.focus {
                    ui.focus(id);
                }
                if effects.capture {
                    ui.set_capture(id);
                }
                if effects.release_capture {
                    ui.release_capture();
                }
                if let Some(cursor) = effects.cursor {
                    ui.set_cursor(id, cursor);
                }
                if effects.invalidate {
                    ui.invalidate(id);
                }
                effects.emit.and_then(&on_event)
            });
        }

        // Worker -> UI wakeup: a `Proxy` post mapped through `on_frame`.
        let proxy = ui.proxy();
        let on_frame = Arc::new(on_frame);
        let wake: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
            let _ = proxy.send(on_frame());
        });
        let spawned = std::thread::Builder::new()
            .name("litehtml-worker".to_string())
            .spawn(move || {
                let worker = Worker::new(text, out_tx, latest_id, wake, worker_images);
                worker.run(job_rx);
            });
        if let Err(e) = &spawned {
            log::error!("xui-litehtml: could not start the render thread: {e}");
        }

        Ok(HtmlView {
            control,
            widget,
            images,
        })
    }

    /// Sets how `http(s)` images are fetched, or `None` (the default) to leave
    /// them unloaded. The fetcher runs on the render thread and may block. It
    /// applies to the next [`load`](Self::load).
    pub fn set_image_fetcher(&self, fetcher: Option<ImageFetcher>) {
        *self
            .images
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = fetcher;
    }

    /// Loads a new page, replacing whatever is currently shown.
    pub fn load(&self, html: String) {
        self.widget.load(html);
        self.control.invalidate();
    }

    /// Sets the colour shown behind the document and wherever it paints
    /// nothing (white by default). Pair it with a stylesheet that sets the
    /// text colour, so unstyled messages follow the app's theme.
    pub fn set_background(&self, color: Color) {
        self.widget.set_background(color);
        self.control.invalidate();
    }

    /// Whether the newest render has finished.
    pub fn is_ready(&self) -> bool {
        self.widget.is_ready()
    }

    /// Sets the vertical scroll offset (device-independent pixels).
    pub fn set_scroll(&self, y: f32) {
        self.widget.set_scroll(y);
        self.control.invalidate();
    }

    /// The selected text, as a copy should read, or `None` if nothing is
    /// selected.
    pub fn selected_text(&self) -> Option<String> {
        self.widget.selected_text()
    }

    /// Whether any text is selected.
    pub fn has_selection(&self) -> bool {
        self.widget.has_selection()
    }

    /// Select all the text of the page.
    pub fn select_all(&self) {
        self.widget.select_all();
        self.control.invalidate();
    }

    /// Drop the selection.
    pub fn clear_selection(&self) {
        self.widget.clear_selection();
        self.control.invalidate();
    }

    /// Moves and resizes the view (device pixels).
    pub fn set_bounds(&self, bounds: Rect) {
        self.control.set_bounds(bounds);
    }

    /// Repaints the view (used when the worker reports a frame is ready).
    pub fn invalidate(&self) {
        self.control.invalidate();
    }
}
