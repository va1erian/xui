#![forbid(unsafe_code)]

//! The public [`NetSurfView`]: a page rendered by the NetSurf engine thread,
//! hosted as a custom-painted portable node.

use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, mpsc};

use xui_core::Color;
use xui_core::app::Ui;
use xui_core::backend::{NodeKind, NodeSpec, Result};
use xui_core::geometry::Rect;
use xui_core::widget::Control;
use xui_litehtml::TextSystem;

use crate::engine::{self, Command, Wake};
use crate::widget::NetSurfWidget;

/// What a [`NetSurfView`] reports, returned by [`NetSurfView::update`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetSurfViewEvent {
    /// The page's title changed.
    TitleChanged(String),
    /// The window moved to a new URL (a followed link or a redirect).
    UrlChanged(String),
    /// A load started (`true`) or finished (`false`).
    LoadingChanged(bool),
    /// A URL could not be opened.
    Failed(String),
}

/// A web page rendered by NetSurf on its engine thread and drawn through the
/// portable canvas with `xui-litehtml`'s painter.
///
/// `on_frame` is the app message that says the engine has news (mapped through
/// a `Proxy`, so the UI thread never blocks); the app answers it by calling
/// [`NetSurfView::update`], which repaints and returns the view's events.
pub struct NetSurfView<M: 'static> {
    control: Control<M>,
    widget: Rc<NetSurfWidget>,
}

impl<M: Send + 'static> NetSurfView<M> {
    /// Creates the view at `bounds` (device pixels) and starts loading `url`
    /// (`file:`, `data:`, `about:` and `resource:` URLs; there is no network
    /// fetcher yet).
    pub fn new(
        ui: &Ui<M>,
        bounds: Rect,
        url: &str,
        on_frame: impl Fn() -> M + Send + Sync + 'static,
    ) -> Result<NetSurfView<M>> {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let tx = engine::commands(|| Arc::from(ui.text_shaper()));
        let (out, rx) = mpsc::channel();
        let scale = ui.dpi() as f32 / 96.0;
        let size = (
            ((bounds.width() as f32 / scale) as i32).max(1),
            ((bounds.height() as f32 / scale) as i32).max(1),
        );

        let control = Control::new(ui, &NodeSpec::new(NodeKind::Custom, bounds))?;
        let widget = Rc::new(NetSurfWidget::new(
            id,
            tx,
            rx,
            TextSystem::new(ui.text_shaper()),
            size,
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
            let node = control.id();
            control.on_events(move |event| {
                if !event.is_input() {
                    return None;
                }
                let effects = widget.handle_input(event);
                if effects.focus {
                    ui.focus(node);
                }
                if effects.capture {
                    ui.set_capture(node);
                }
                if effects.release_capture {
                    ui.release_capture();
                }
                if effects.invalidate {
                    ui.invalidate(node);
                }
                None
            });
        }

        let proxy = ui.proxy();
        let wake: Wake = Arc::new(move || {
            let _ = proxy.send(on_frame());
        });
        widget.send(Command::Open {
            id,
            url: url.to_string(),
            size,
            out,
            wake,
        });
        Ok(NetSurfView { control, widget })
    }

    /// Takes the engine's news: repaints with the newest frame and returns
    /// what else happened. Call it when `on_frame`'s message arrives.
    pub fn update(&self) -> Vec<NetSurfViewEvent> {
        let events = self.widget.drain();
        self.control.invalidate();
        events
    }

    /// Opens `url` in the view.
    pub fn navigate(&self, url: &str) {
        self.widget.send(Command::Navigate {
            id: self.widget.id(),
            url: url.to_string(),
        });
    }

    /// Whether a page has been drawn and nothing is still loading.
    pub fn is_ready(&self) -> bool {
        self.widget.is_ready()
    }

    /// Whether the page could not be opened or the engine stopped.
    pub fn has_failed(&self) -> bool {
        self.widget.has_failed()
    }

    /// Sets the colour shown wherever the page paints nothing (white by
    /// default).
    pub fn set_background(&self, color: Color) {
        self.widget.set_background(color);
        self.control.invalidate();
    }

    /// Sets the vertical scroll offset (CSS pixels).
    pub fn set_scroll(&self, y: f32) {
        self.widget.set_scroll(y);
        self.control.invalidate();
    }

    /// Moves and resizes the view (device pixels).
    pub fn set_bounds(&self, bounds: Rect) {
        self.control.set_bounds(bounds);
    }
}
