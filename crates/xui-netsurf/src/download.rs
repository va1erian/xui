#![forbid(unsafe_code)]

//! Downloads: what NetSurf cannot show, saved by a host-supplied
//! [`Downloader`].
//!
//! NetSurf turns a navigation into a download when the response is not
//! something it displays (an archive, a PDF, a `Content-Disposition:
//! attachment`), and when the application asks for one with
//! [`NetSurfView::download`](crate::NetSurfView::download). Each download is
//! offered to the [`Downloader`] set with [`set_downloader`], which decides
//! where its bytes go by returning a [`DownloadSink`] (or refuses it). The
//! sink runs on the engine thread, so it should only write; the view that
//! started the download reports its progress as
//! [`NetSurfViewEvent`](crate::NetSurfViewEvent)s.

use std::cell::RefCell;
use std::collections::HashMap;
use std::io;
use std::sync::{Arc, PoisonError, RwLock};

use crate::engine::{Output, Reporter};

/// The fewest new bytes between two progress reports, so a fast download does
/// not wake the UI for every piece.
const PROGRESS_STEP: u64 = 64 * 1024;

/// Names one download in the view's events and in
/// [`NetSurfView::cancel_download`](crate::NetSurfView::cancel_download).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DownloadId(pub(crate) u64);

/// A download as NetSurf starts it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadInfo {
    /// Names the download.
    pub id: DownloadId,
    /// Where it comes from.
    pub url: String,
    /// NetSurf's file name for it: the `Content-Disposition` file name, else
    /// the URL's last path segment. It is untrusted (a server chose it):
    /// sanitise it before using it as a path.
    pub filename: String,
    /// The MIME type the server gave.
    pub mime: String,
    /// The size the server announced, if it did.
    pub total: Option<u64>,
}

/// Where one download's bytes go.
pub trait DownloadSink: Send {
    /// Writes the next bytes. An error stops the download, which then ends
    /// with that error.
    fn write(&mut self, data: &[u8]) -> io::Result<()>;

    /// The download ended: `Ok` when every byte arrived, else why not (a
    /// failed fetch, a cancel, a write error). A sink should remove a partial
    /// file on `Err`.
    fn finish(self: Box<Self>, result: Result<(), String>);
}

/// Decides where downloads go. Called on the engine thread.
pub trait Downloader: Send + Sync {
    /// The sink for `info`, or `None` to refuse the download.
    fn start(&self, info: &DownloadInfo) -> Option<Box<dyn DownloadSink>>;
}

static DOWNLOADER: RwLock<Option<Arc<dyn Downloader>>> = RwLock::new(None);

/// Sets the [`Downloader`] every view's downloads are offered to. Without
/// one, NetSurf refuses to download.
pub fn set_downloader(downloader: Arc<dyn Downloader>) {
    *DOWNLOADER.write().unwrap_or_else(PoisonError::into_inner) = Some(downloader);
}

fn downloader() -> Option<Arc<dyn Downloader>> {
    DOWNLOADER
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
}

/// What a view hears about one of its downloads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DownloadNews {
    Started(DownloadInfo),
    Progress {
        id: DownloadId,
        received: u64,
    },
    Finished {
        id: DownloadId,
        error: Option<String>,
    },
}

/// One download under way.
struct Active {
    sink: Box<dyn DownloadSink>,
    /// The view that started it, if it is still known.
    view: Option<Reporter>,
    received: u64,
    reported: u64,
}

impl Active {
    fn report(&self, news: DownloadNews) {
        if let Some(view) = &self.view {
            view.send(Output::Download(news));
        }
    }
}

/// The downloads under way, kept by the engine (on its thread).
#[derive(Default)]
pub(crate) struct Downloads {
    active: RefCell<HashMap<u64, Active>>,
}

impl Downloads {
    /// A download starts; whether the host takes it.
    pub(crate) fn start(&self, info: DownloadInfo, view: Option<Reporter>) -> bool {
        let Some(sink) = downloader().and_then(|d| d.start(&info)) else {
            log::info!("xui-netsurf: download of {} refused", info.url);
            return false;
        };
        let id = info.id.0;
        let active = Active {
            sink,
            view,
            received: 0,
            reported: 0,
        };
        active.report(DownloadNews::Started(info));
        self.active.borrow_mut().insert(id, active);
        true
    }

    /// The next bytes of download `id`; whether it goes on.
    pub(crate) fn data(&self, id: u64, data: &[u8]) -> bool {
        let mut active = self.active.borrow_mut();
        let Some(download) = active.get_mut(&id) else {
            return false;
        };
        if let Err(e) = download.sink.write(data) {
            log::warn!("xui-netsurf: download {id} could not be written: {e}");
            return false;
        }
        download.received += data.len() as u64;
        if download.received - download.reported >= PROGRESS_STEP {
            download.reported = download.received;
            download.report(DownloadNews::Progress {
                id: DownloadId(id),
                received: download.received,
            });
        }
        true
    }

    /// Download `id` ended, with `error` unless it completed.
    pub(crate) fn end(&self, id: u64, error: Option<String>) {
        let Some(download) = self.active.borrow_mut().remove(&id) else {
            return;
        };
        let id = DownloadId(id);
        if download.received != download.reported {
            download.report(DownloadNews::Progress {
                id,
                received: download.received,
            });
        }
        download.report(DownloadNews::Finished {
            id,
            error: error.clone(),
        });
        download.sink.finish(error.map_or(Ok(()), Err));
    }
}
