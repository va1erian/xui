//! Downloads: what the view cannot show, saved by a host-supplied
//! [`Downloader`].
//!
//! A navigation becomes a download when the response is not something the
//! view displays (an archive, a PDF, a `Content-Disposition: attachment`),
//! and when the application asks for one with
//! [`BlitzView::download`](crate::BlitzView::download). Each is offered to the
//! [`Downloader`] set with [`set_downloader`], which decides where its bytes
//! go by returning a [`DownloadSink`] (or refuses it). The sink runs on the
//! fetch thread, so it should only write; the view that started the download
//! reports its progress as [`BlitzViewEvent`](crate::BlitzViewEvent)s.

use std::collections::HashMap;
use std::io;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError, RwLock};

use crate::engine::Reporter;
use crate::fetch::Response;
use crate::net::{Head, header};
use crate::view::BlitzViewEvent;

/// The fewest new bytes between two progress reports, so a fast download does
/// not wake the UI for every piece.
const PROGRESS_STEP: u64 = 64 * 1024;

/// Names one download in the view's events and in
/// [`BlitzView::cancel_download`](crate::BlitzView::cancel_download).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DownloadId(pub u64);

/// A download as it starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadInfo {
    /// Names the download.
    pub id: DownloadId,
    /// Where it comes from.
    pub url: String,
    /// A file name for it: the `Content-Disposition` file name, else the URL's
    /// last path segment. It is untrusted (a server chose it): sanitise it
    /// before using it as a path.
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

/// Decides where downloads go. Called on a fetch thread.
pub trait Downloader: Send + Sync {
    /// The sink for `info`, or `None` to refuse the download.
    fn start(&self, info: &DownloadInfo) -> Option<Box<dyn DownloadSink>>;
}

static DOWNLOADER: RwLock<Option<Arc<dyn Downloader>>> = RwLock::new(None);

/// Sets the [`Downloader`] every view's downloads are offered to. Without
/// one, nothing is downloaded.
pub fn set_downloader(downloader: Arc<dyn Downloader>) {
    *DOWNLOADER.write().unwrap_or_else(PoisonError::into_inner) = Some(downloader);
}

fn downloader() -> Option<Arc<dyn Downloader>> {
    DOWNLOADER
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
}

/// One download under way. It sits behind a lock shared with [`CANCELS`], so
/// a cancel and the fetch's own end race for it and only one finishes it.
struct Active {
    id: DownloadId,
    sink: Box<dyn DownloadSink>,
    view: Reporter,
    received: u64,
    reported: u64,
}

type Shared = Arc<Mutex<Option<Active>>>;

/// The downloads under way, by id, for [`cancel`].
static CANCELS: Mutex<Option<HashMap<u64, Shared>>> = Mutex::new(None);

fn registry<R>(f: impl FnOnce(&mut HashMap<u64, Shared>) -> R) -> R {
    f(CANCELS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get_or_insert_with(HashMap::new))
}

impl Active {
    fn end(mut self, error: Option<String>) {
        registry(|r| r.remove(&self.id.0));
        if self.received != self.reported {
            self.reported = self.received;
            self.view.event(BlitzViewEvent::DownloadProgress {
                id: self.id,
                received: self.received,
            });
        }
        self.view.event(BlitzViewEvent::DownloadFinished {
            id: self.id,
            error: error.clone(),
        });
        self.sink.finish(error.map_or(Ok(()), Err));
    }
}

/// Stops download `id`: it ends now with an error, and whatever the fetch
/// still delivers is dropped.
pub(crate) fn cancel(id: DownloadId) {
    let Some(shared) = registry(|r| r.remove(&id.0)) else {
        return;
    };
    let active = shared.lock().unwrap_or_else(PoisonError::into_inner).take();
    if let Some(active) = active {
        active.end(Some("cancelled".to_string()));
    }
}

/// The stream a page response is diverted to when it becomes a download.
struct Stream(Shared);

impl Stream {
    fn with(&self, f: impl FnOnce(&mut Active) -> Result<(), String>) {
        let mut guard = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(active) = guard.as_mut()
            && let Err(e) = f(active)
            && let Some(active) = guard.take()
        {
            active.end(Some(e));
        }
    }
}

impl Response for Stream {
    fn status(&mut self, _code: u16) {}

    fn header(&mut self, _name: &str, _value: &str) {}

    fn data(&mut self, bytes: &[u8]) {
        self.with(|a| {
            a.sink.write(bytes).map_err(|e| e.to_string())?;
            a.received += bytes.len() as u64;
            if a.received - a.reported >= PROGRESS_STEP {
                a.reported = a.received;
                a.view.event(BlitzViewEvent::DownloadProgress {
                    id: a.id,
                    received: a.received,
                });
            }
            Ok(())
        });
    }

    fn end(self: Box<Self>, result: Result<(), String>) {
        let active = self.0.lock().unwrap_or_else(PoisonError::into_inner).take();
        if let Some(active) = active {
            active.end(result.err());
        }
    }
}

/// Offers the response `head` describes to the downloader; the stream its
/// body should go to, or `None` (and a log line) when it is refused.
pub(crate) fn begin(head: &Head<'_>, view: Reporter) -> Option<Box<dyn Response>> {
    static NEXT_ID: AtomicU64 = AtomicU64::new(1);
    let info = DownloadInfo {
        id: DownloadId(NEXT_ID.fetch_add(1, Ordering::Relaxed)),
        url: head.url.to_string(),
        filename: filename(head.url, header(head.headers, "Content-Disposition")),
        mime: header(head.headers, "Content-Type")
            .map(|t| t.split(';').next().unwrap_or("").trim().to_string())
            .unwrap_or_else(|| "application/octet-stream".to_string()),
        total: header(head.headers, "Content-Length").and_then(|v| v.trim().parse().ok()),
    };
    let Some(sink) = downloader().and_then(|d| d.start(&info)) else {
        log::info!("xui-blitz: download of {} refused", info.url);
        return None;
    };
    let id = info.id;
    view.event(BlitzViewEvent::DownloadStarted(info));
    let shared: Shared = Arc::new(Mutex::new(Some(Active {
        id,
        sink,
        view,
        received: 0,
        reported: 0,
    })));
    registry(|r| r.insert(id.0, Arc::clone(&shared)));
    Some(Box::new(Stream(shared)))
}

/// Whether a response with these headers is one to save rather than show:
/// an attachment, or a type the view does not display.
pub(crate) fn is_download(headers: &[(String, String)]) -> bool {
    let attachment = header(headers, "Content-Disposition")
        .is_some_and(|d| d.trim_start().to_ascii_lowercase().starts_with("attachment"));
    let mime = header(headers, "Content-Type")
        .map(|t| t.split(';').next().unwrap_or("").trim().to_ascii_lowercase())
        .unwrap_or_default();
    attachment || !crate::engine::page::is_displayable(&mime)
}

/// The file name for a download: `Content-Disposition`'s, else the URL's
/// last path segment, else `download`.
fn filename(url: &str, disposition: Option<&str>) -> String {
    let from_header = disposition.and_then(|d| {
        d.split(';').find_map(|part| {
            let (key, value) = part.split_once('=')?;
            let key = key.trim().to_ascii_lowercase();
            let value = value.trim();
            match key.as_str() {
                // RFC 5987: charset'lang'percent-encoded.
                "filename*" => {
                    let encoded = value.rsplit('\'').next()?;
                    url::form_urlencoded::parse(format!("x={encoded}").as_bytes())
                        .next()
                        .map(|(_, v)| v.into_owned())
                }
                "filename" => Some(value.trim_matches('"').to_string()),
                _ => None,
            }
        })
    });
    from_header
        .or_else(|| {
            let parsed = url::Url::parse(url).ok()?;
            let last = parsed.path_segments()?.next_back()?.to_string();
            Some(
                url::form_urlencoded::parse(format!("x={last}").as_bytes())
                    .next()?
                    .1
                    .into_owned(),
            )
        })
        .filter(|f| !f.is_empty())
        .unwrap_or_else(|| "download".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_file_name_comes_from_the_header_then_the_url() {
        assert_eq!(
            filename("http://x/a/b.zip", Some("attachment; filename=\"r e.pdf\"")),
            "r e.pdf"
        );
        assert_eq!(
            filename("http://x/a/b.zip", Some("attachment; filename*=UTF-8''na%C3%AFve.txt")),
            "naïve.txt"
        );
        assert_eq!(filename("http://x/a/my%20file.zip", None), "my file.zip");
        assert_eq!(filename("http://x/", None), "download");
    }

    #[test]
    fn attachments_and_unshown_types_download() {
        let h = |pairs: &[(&str, &str)]| -> Vec<(String, String)> {
            pairs.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect()
        };
        assert!(is_download(&h(&[("Content-Type", "application/zip")])));
        assert!(is_download(&h(&[
            ("Content-Type", "text/html"),
            ("Content-Disposition", "attachment"),
        ])));
        assert!(!is_download(&h(&[("Content-Type", "text/html; charset=utf-8")])));
        assert!(!is_download(&h(&[("Content-Type", "image/png")])));
    }
}
