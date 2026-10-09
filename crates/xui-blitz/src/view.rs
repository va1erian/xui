//! The public [`BlitzView`]: a page rendered by its engine thread, hosted as
//! a custom-painted portable node.

use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;

use xui_core::Color;
use xui_core::app::Ui;
use xui_core::backend::{NodeKind, NodeSpec, Result, WidgetId};
use xui_core::geometry::Rect;
use xui_core::widget::Control;
use xui_core::widget::Orientation;
use xui_core::widget::scrollbar::{self, ScrollBar, ThumbState};

use crate::download::{self, DownloadId, DownloadInfo};
use crate::engine::{self, Command, OnEvent, Options, Reporter, Wake};
use crate::widget::Widget;

/// What a [`BlitzView`] reports, mapped to the app's message by the
/// `on_event` closure given to [`BlitzView::builder`]. The closure runs on
/// the view's engine thread or a fetch thread, so it should only build the
/// message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlitzViewEvent {
    /// The page's title changed (empty when it has none).
    TitleChanged(String),
    /// The view moved to a new URL (a followed link, a redirect, a new page).
    UrlChanged(String),
    /// A load started (`true`) or finished, its style sheets, fonts and
    /// images included (`false`).
    LoadingChanged(bool),
    /// A link or form was followed in a view that does not follow links
    /// itself (see [`BlitzViewBuilder::follow_links`]); the host decides
    /// what to do with it.
    LinkClicked(String),
    /// The user pressed Ctrl+C with text selected; carries the text. The
    /// crate has no clipboard of its own, so the host puts it on the
    /// platform clipboard.
    CopyRequested(String),
    /// Something could not be done: `navigate` was given something that is
    /// not a URL, or a download could not start.
    Failed(String),
    /// The page could not be fetched; the view shows an error page instead.
    /// (A failed style sheet or image is not reported: the page shows
    /// without it.)
    FetchFailed {
        /// The URL that could not be fetched.
        url: String,
        /// Why.
        message: String,
    },
    /// The status line text: the link under the pointer, or empty.
    StatusChanged(String),
    /// A link to a URL the view cannot open itself (`mailto:`, or `http:`
    /// with no [`Fetcher`](crate::Fetcher)), for the application to hand to
    /// the system. `by_user` is whether the user's click or key, or the app,
    /// asked for it (there are no scripts, so today it always is).
    LaunchUrl {
        /// The URL to hand on.
        url: String,
        /// Whether the user's click or key, or the app, asked for it.
        by_user: bool,
    },
    /// The user right-clicked the page. `x` and `y` are in device pixels
    /// from the view's top-left corner; `link` and `image` are the absolute
    /// URLs of the link and picture under the pointer, if any. The host
    /// decides what to offer (a menu with Save Image As, say; see
    /// [`BlitzView::download`]).
    ContextMenu {
        /// Pointer position, device pixels from the view's left edge.
        x: i32,
        /// Pointer position, device pixels from the view's top edge.
        y: i32,
        /// The link under the pointer.
        link: Option<String>,
        /// The picture under the pointer.
        image: Option<String>,
    },
    /// A download started; its bytes go to the
    /// [`Downloader`](crate::Downloader)'s sink.
    DownloadStarted(DownloadInfo),
    /// `received` bytes of a download have arrived (reported every so
    /// often, and once more at its end).
    DownloadProgress {
        /// The download.
        id: DownloadId,
        /// Bytes so far.
        received: u64,
    },
    /// A download ended: `error` is `None` when every byte arrived.
    DownloadFinished {
        /// The download.
        id: DownloadId,
        /// Why it did not complete.
        error: Option<String>,
    },
}

/// What a view shows first.
enum Start {
    Html { html: String, base_url: String },
    Url(String),
}

/// Configures a [`BlitzView`]; made by [`BlitzView::builder`].
pub struct BlitzViewBuilder<M> {
    on_frame: Arc<dyn Fn() -> M + Send + Sync>,
    on_event: Arc<dyn Fn(BlitzViewEvent) -> Option<M> + Send + Sync>,
    start: Start,
    follow_links: bool,
    background: Color,
}

impl<M: Send + 'static> BlitzViewBuilder<M> {
    /// Starts with `html`, its relative links resolved against `about:blank`
    /// (see [`base_url`](Self::base_url)).
    pub fn html(mut self, html: impl Into<String>) -> Self {
        self.start = Start::Html {
            html: html.into(),
            base_url: "about:blank".to_string(),
        };
        self
    }

    /// The URL the starting HTML's relative links, style sheets and images
    /// resolve against. Only meaningful after [`html`](Self::html).
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        if let Start::Html { base_url, .. } = &mut self.start {
            *base_url = url.into();
        }
        self
    }

    /// Starts by opening `url`.
    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.start = Start::Url(url.into());
        self
    }

    /// Whether the view opens the links and forms the user follows itself, as
    /// a browser does (`true`), or reports them as
    /// [`BlitzViewEvent::LinkClicked`] for the host to handle, as a mail or
    /// help reader does (`false`, the default).
    pub fn follow_links(mut self, follow: bool) -> Self {
        self.follow_links = follow;
        self
    }

    /// The colour shown behind the page and wherever it paints nothing
    /// (white by default).
    pub fn background(mut self, color: Color) -> Self {
        self.background = color;
        self
    }

    /// Creates the view at `bounds` (device pixels) and starts its engine.
    pub fn build(self, ui: &Ui<M>, bounds: Rect) -> Result<BlitzView<M>> {
        let scale = ui.dpi() as f32 / 96.0;
        let size = (bounds.width().max(1) as u32, bounds.height().max(1) as u32);
        let (out_tx, out_rx) = mpsc::channel();
        let pending = Arc::new(AtomicBool::new(false));

        let proxy = ui.proxy();
        let on_frame = self.on_frame;
        let wake: Wake = Arc::new(move || {
            let _ = proxy.send(on_frame());
        });
        let proxy = ui.proxy();
        let map = self.on_event;
        let on_event: OnEvent = Arc::new(move |event| {
            if let Some(msg) = map(event) {
                let _ = proxy.send(msg);
            }
        });
        let report = Reporter::new(out_tx, wake, Arc::clone(&pending), on_event);
        let options = Options {
            follow_links: self.follow_links,
        };
        let tx = engine::spawn(options, report, size, scale);

        let control = Control::new(ui, &NodeSpec::new(NodeKind::Custom, bounds))?;
        let widget = Rc::new(Widget::new(tx, out_rx, pending, size, scale));
        widget.set_background(self.background);
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
                let fx = widget.handle_input(event);
                if fx.focus {
                    ui.focus(node);
                }
                if fx.capture {
                    ui.set_capture(node);
                }
                if fx.release_capture {
                    ui.release_capture();
                }
                None
            });
        }
        // A sibling of the page's node, not a child: a custom node is not
        // a container on every backend, and a child of it is not placed
        // relative to it there.
        let bar_node = Control::new(ui, &NodeSpec::new(NodeKind::Container, Rect::default()))?;
        let bar = Rc::new(ScrollBar::new(bar_node.id()));
        {
            let widget = Rc::clone(&widget);
            let bar = Rc::clone(&bar);
            let theme = ui.theme_handle();
            bar_node.set_painter(Rc::new(move |canvas| {
                let b = canvas.bounds();
                let state = if bar.is_dragging() {
                    ThumbState::Pressed
                } else {
                    ThumbState::Normal
                };
                scrollbar::paint_state(
                    canvas,
                    Rect::new(0, 0, b.width(), b.height()),
                    widget.scroll_metrics(),
                    Orientation::Vertical,
                    theme.get(),
                    state,
                );
            }));
        }
        {
            let widget = Rc::clone(&widget);
            let bar = Rc::clone(&bar);
            let ui = ui.clone();
            bar_node.on_events(move |event| {
                let metrics = widget.scroll_metrics();
                bar.handle(&ui, metrics, |target| widget.scroll_to_px(target), event);
                None
            });
        }
        let view = BlitzView {
            ui: ui.clone(),
            control,
            widget,
            bar_node,
            bar,
        };
        view.place_bar(bounds);
        match self.start {
            Start::Html { html, base_url } => view.load_html(html, base_url),
            Start::Url(url) => view.navigate(&url),
        }
        Ok(view)
    }
}

/// A web page laid out and drawn by Blitz on the view's own engine thread,
/// shown through the portable canvas.
///
/// `on_frame` is the app message that says the engine drew a new frame
/// (posted through a `Proxy`, so the UI thread never blocks); the app answers
/// it by calling [`BlitzView::update`]. Events reach the app through the
/// builder's `on_event`.
pub struct BlitzView<M: 'static> {
    ui: Ui<M>,
    control: Control<M>,
    widget: Rc<Widget>,
    /// The vertical scrollbar along the view's right edge.
    bar_node: Control<M>,
    bar: Rc<ScrollBar>,
}

impl<M: Send + 'static> BlitzView<M> {
    /// Starts configuring a view: `on_frame` builds the message that asks for
    /// [`update`](Self::update), `on_event` maps the view's events to
    /// messages (or `None` to ignore one).
    pub fn builder(
        on_frame: impl Fn() -> M + Send + Sync + 'static,
        on_event: impl Fn(BlitzViewEvent) -> Option<M> + Send + Sync + 'static,
    ) -> BlitzViewBuilder<M> {
        BlitzViewBuilder {
            on_frame: Arc::new(on_frame),
            on_event: Arc::new(on_event),
            start: Start::Html {
                html: String::new(),
                base_url: "about:blank".to_string(),
            },
            follow_links: false,
            background: Color::rgb(255, 255, 255),
        }
    }

    /// Takes the engine's newest frame and repaints. Call it when
    /// `on_frame`'s message arrives.
    pub fn update(&self) {
        if let Some(cursor) = self.widget.drain() {
            self.ui.set_cursor(self.control.id(), cursor);
        }
        self.control.invalidate();
        self.bar_node.invalidate();
    }

    /// Lays the scrollbar along the right edge of a view at `bounds` (in
    /// the view's parent, where the bar sits beside the view's node).
    fn place_bar(&self, bounds: Rect) {
        let width = scrollbar::THICKNESS.to_px(self.ui.dpi()).value();
        self.bar.set_track(width, bounds.height());
        let track = Rect::new(
            bounds.right - width,
            bounds.top,
            bounds.right,
            bounds.bottom,
        );
        self.ui.apply_moves(&[(self.bar.id(), track)]);
        self.ui.raise(self.bar.id());
    }

    /// The view's node, for placing popups at its pointer positions
    /// ([`BlitzViewEvent::ContextMenu`]).
    pub fn id(&self) -> WidgetId {
        self.control.id()
    }

    /// Shows `html`, resolving its links, style sheets and images against
    /// `base_url`.
    pub fn load_html(&self, html: impl Into<String>, base_url: impl Into<String>) {
        self.widget.loading();
        self.widget.send(Command::Html {
            html: html.into(),
            base_url: base_url.into(),
        });
    }

    /// Opens `url` (`file:`, `data:` and `about:blank`, and `http:` and
    /// `https:` once the application has called
    /// [`set_fetcher`](crate::set_fetcher)); one the view cannot open is
    /// reported as [`BlitzViewEvent::LaunchUrl`].
    pub fn navigate(&self, url: &str) {
        self.widget.loading();
        self.widget.send(Command::Navigate(url.to_string()));
    }

    /// Stops loading the page (what has arrived stays on show).
    pub fn stop(&self) {
        self.widget.send(Command::Stop);
    }

    /// Downloads `url` instead of showing it; the download is offered to the
    /// [`Downloader`](crate::Downloader) like any other.
    pub fn download(&self, url: &str) {
        self.widget.send(Command::Download(url.to_string()));
    }

    /// Stops download `id`; it ends with an error.
    pub fn cancel_download(&self, id: DownloadId) {
        download::cancel(id);
    }

    /// Whether the page is drawn and nothing is still loading.
    pub fn is_ready(&self) -> bool {
        self.widget.is_ready()
    }

    /// Whether the page could not be opened (an error page shows) or the
    /// engine stopped.
    pub fn has_failed(&self) -> bool {
        self.widget.has_failed()
    }

    /// Sets the colour shown wherever the page paints nothing.
    pub fn set_background(&self, color: Color) {
        self.widget.set_background(color);
    }

    /// Sets the vertical scroll offset (CSS pixels).
    pub fn set_scroll(&self, y: f32) {
        self.widget.send(Command::ScrollTo(y as f64));
    }

    /// Drops the text selection.
    pub fn clear_selection(&self) {
        self.widget.send(Command::ClearSelection);
    }

    /// Moves and resizes the view (device pixels).
    pub fn set_bounds(&self, bounds: Rect) {
        self.control.set_bounds(bounds);
        self.place_bar(bounds);
    }
}

impl<M: 'static> Drop for BlitzView<M> {
    fn drop(&mut self) {
        self.widget.send(Command::Quit);
    }
}
