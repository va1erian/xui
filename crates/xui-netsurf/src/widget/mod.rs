#![forbid(unsafe_code)]

//! The custom-painted [`NetSurfWidget`] behind the public
//! [`NetSurfView`](crate::NetSurfView): it holds the newest frame the engine
//! sent and paints it with `xui-litehtml`'s [`Painter`], the same painter a
//! litehtml page goes through. Input lives in `input.rs`.

use std::cell::{Cell, RefCell};
use std::sync::mpsc::{Receiver, Sender, TryRecvError};

use xui_core::Color;
use xui_core::backend::{Canvas, Cursor};
use xui_core::geometry::Rect as PxRect;
use xui_core::theme::Theme;
use xui_core::widget::scrollbar::{self, Orientation, Scroll, THICKNESS, ThumbState};
use xui_litehtml::{Painter, Rect, TextSystem};

use crate::download::DownloadNews;
use crate::engine::{Command, Frame, Output};
use crate::pointer::cursor_for;
use crate::view::NetSurfViewEvent;

mod input;

/// The custom-painted widget behind a [`NetSurfView`](crate::NetSurfView).
/// Its state sits in cells because the painter and the event mapper share it
/// through an `Rc`.
pub(crate) struct NetSurfWidget {
    id: u64,
    tx: Sender<Command>,
    rx: Receiver<Output>,
    text: TextSystem,
    painter: RefCell<Painter>,
    frame: RefCell<Option<Frame>>,
    /// The epoch the painter's font and image caches belong to.
    painter_epoch: Cell<u64>,
    loading: Cell<bool>,
    failed: Cell<bool>,
    /// The viewport last reported to the engine, in CSS pixels.
    sent_size: Cell<(i32, i32)>,
    background: Cell<Color>,
    /// Vertical scroll offset in CSS pixels.
    scroll: Cell<f32>,
    viewport_height: Cell<f32>,
    /// Device pixels per CSS pixel, as of the last paint.
    scale: Cell<f32>,
    /// Where the primary button went down (document pixels), and whether the
    /// pointer has since moved far enough to be a drag.
    press: Cell<Option<(i32, i32)>>,
    moved: Cell<bool>,
    /// The cursor last handed to the view, and the one NetSurf asked for
    /// since, if it differs.
    cursor: Cell<Cursor>,
    pending_cursor: Cell<Option<Cursor>>,
}

impl NetSurfWidget {
    pub(crate) fn new(
        id: u64,
        tx: Sender<Command>,
        rx: Receiver<Output>,
        text: TextSystem,
        size: (i32, i32),
        scale: f32,
    ) -> NetSurfWidget {
        NetSurfWidget {
            id,
            tx,
            rx,
            painter: RefCell::new(Painter::new(text.clone())),
            text,
            frame: RefCell::new(None),
            painter_epoch: Cell::new(0),
            loading: Cell::new(true),
            failed: Cell::new(false),
            sent_size: Cell::new(size),
            background: Cell::new(Color::rgb(255, 255, 255)),
            scroll: Cell::new(0.0),
            viewport_height: Cell::new(size.1 as f32),
            scale: Cell::new(scale),
            press: Cell::new(None),
            moved: Cell::new(false),
            cursor: Cell::new(Cursor::Default),
            pending_cursor: Cell::new(None),
        }
    }

    pub(crate) fn send(&self, cmd: Command) {
        if self.tx.send(cmd).is_err() {
            self.failed.set(true);
        }
    }

    pub(crate) fn id(&self) -> u64 {
        self.id
    }

    pub(crate) fn set_background(&self, color: Color) {
        self.background.set(color);
    }

    pub(crate) fn set_scroll(&self, y: f32) {
        self.scroll.set(y.max(0.0));
    }

    /// Whether a page has been drawn and nothing is still loading.
    pub(crate) fn is_ready(&self) -> bool {
        !self.loading.get() && self.frame.borrow().is_some()
    }

    /// Whether the engine could not open the page or has stopped.
    pub(crate) fn has_failed(&self) -> bool {
        self.failed.get()
    }

    /// Takes everything the engine sent: keeps the newest frame and returns
    /// the rest as view events.
    pub(crate) fn drain(&self) -> Vec<NetSurfViewEvent> {
        let mut events = Vec::new();
        loop {
            match self.rx.try_recv() {
                Ok(Output::Frame(frame)) => {
                    if frame.epoch != self.painter_epoch.get() {
                        self.painter_epoch.set(frame.epoch);
                        self.scroll.set(0.0);
                        *self.painter.borrow_mut() = Painter::new(self.text.clone());
                    }
                    *self.frame.borrow_mut() = Some(frame);
                }
                Ok(Output::Title(t)) => events.push(NetSurfViewEvent::TitleChanged(t)),
                Ok(Output::Url(u)) => events.push(NetSurfViewEvent::UrlChanged(u)),
                Ok(Output::Loading(on)) => {
                    self.loading.set(on);
                    events.push(NetSurfViewEvent::LoadingChanged(on));
                }
                Ok(Output::Pointer(shape)) => {
                    let cursor = cursor_for(shape);
                    self.pending_cursor
                        .set((cursor != self.cursor.get()).then_some(cursor));
                }
                Ok(Output::Status(text)) => events.push(NetSurfViewEvent::StatusChanged(text)),
                Ok(Output::Launch { url, by_user }) => {
                    events.push(NetSurfViewEvent::LaunchUrl { url, by_user })
                }
                Ok(Output::Download(news)) => events.push(download_event(news)),
                Ok(Output::FetchFailed { url, message }) => {
                    events.push(NetSurfViewEvent::FetchFailed { url, message });
                }
                Ok(Output::Failed(why)) => {
                    self.failed.set(true);
                    self.loading.set(false);
                    events.push(NetSurfViewEvent::Failed(why));
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.failed.set(true);
                    break;
                }
            }
        }
        events
    }

    /// The cursor NetSurf asked for since the last call, when it differs from
    /// the one the view already shows.
    pub(crate) fn take_cursor(&self) -> Option<Cursor> {
        let next = self.pending_cursor.take()?;
        self.cursor.set(next);
        Some(next)
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

    /// Paints the newest frame, telling the engine first when the viewport
    /// changed size. A strip for the scrollbar is always reserved, as
    /// `HtmlView` does, so both engines lay out at the same width.
    pub(crate) fn paint(&self, canvas: &mut dyn Canvas, theme: Theme) {
        let bounds = canvas.bounds();
        let dpi = canvas.dpi();
        let scale = dpi as f32 / 96.0;
        self.scale.set(scale);
        let bar = THICKNESS.to_px(dpi).value();
        let track = PxRect::new(
            (bounds.right - bar).max(bounds.left),
            bounds.top,
            bounds.right,
            bounds.bottom,
        );
        let width = ((track.left - bounds.left) as f32 / scale).max(1.0);
        let height = (bounds.height() as f32 / scale).max(1.0);
        self.viewport_height.set(height);
        let size = (width.round() as i32, height.round() as i32);
        if size != self.sent_size.get() {
            self.sent_size.set(size);
            self.send(Command::Resize { id: self.id, size });
        }

        let scroll = self.scroll.get().clamp(0.0, self.max_scroll());
        self.scroll.set(scroll);
        let viewport = Rect::new(0.0, 0.0, width, height);
        match self.frame.borrow().as_ref() {
            Some(frame) => self.painter.borrow_mut().paint(
                &frame.list,
                canvas,
                viewport,
                scroll,
                self.background.get(),
            ),
            None => canvas.clear(self.background.get()),
        }
        let px = |v: f32| (v * scale).round() as i32;
        let state = Scroll {
            viewport: px(height),
            content: px(self.content_height()),
            offset: px(scroll),
        };
        scrollbar::paint_state(
            canvas,
            track,
            state,
            Orientation::Vertical,
            theme,
            ThumbState::Normal,
        );
    }
}

/// A download's news as the view reports it.
fn download_event(news: DownloadNews) -> NetSurfViewEvent {
    match news {
        DownloadNews::Started(info) => NetSurfViewEvent::DownloadStarted(info),
        DownloadNews::Progress { id, received } => {
            NetSurfViewEvent::DownloadProgress { id, received }
        }
        DownloadNews::Finished { id, error } => NetSurfViewEvent::DownloadFinished { id, error },
    }
}

impl Drop for NetSurfWidget {
    fn drop(&mut self) {
        let _ = self.tx.send(Command::Close { id: self.id });
    }
}
