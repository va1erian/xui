#![forbid(unsafe_code)]

//! Worker-thread access to the app: [`Proxy`].
//!
//! Background work feeds the UI through a [`Proxy`]: [`Ui::proxy`] returns one,
//! worker threads share it (`Clone`, and `Send + Sync` when the message type
//! is), and [`Proxy::send`] pushes onto a thread-safe [`Inbox`]. The UI thread
//! moves the inbox into the window's own queue ahead of the drain, so the
//! messages are delivered by the same drain and [`App::update`] is never
//! re-entered. Sending is coalesced: a burst before the UI thread collects
//! posts a single wake.
//!
//! [`App::update`]: super::App::update
//! [`Ui::proxy`]: super::Ui::proxy

use std::collections::VecDeque;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

use crate::backend::Waker;

/// The messages a worker has handed back, plus the coalescing flag that keeps a
/// burst to one wake.
pub(crate) struct Inbox<M> {
    queue: Mutex<VecDeque<M>>,
    woken: AtomicBool,
    closed: AtomicBool,
}

impl<M> Inbox<M> {
    pub(crate) fn new() -> Arc<Inbox<M>> {
        Arc::new(Inbox {
            queue: Mutex::new(VecDeque::new()),
            woken: AtomicBool::new(false),
            closed: AtomicBool::new(false),
        })
    }

    /// Marks the inbox closed: later sends fail rather than queue forever.
    pub(crate) fn close(&self) {
        self.closed.store(true, Ordering::SeqCst);
    }

    fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }

    /// Pushes `msg`, reporting whether the caller must post a wake. Only the
    /// first push since the last [`Inbox::collect`] reports `true`.
    fn push(&self, msg: M) -> bool {
        self.queue
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .push_back(msg);
        !self.woken.swap(true, Ordering::SeqCst)
    }

    /// Takes every waiting message, clearing the coalescing flag.
    pub(crate) fn collect(&self) -> Vec<M> {
        self.woken.store(false, Ordering::SeqCst);
        let waiting: Vec<M> = self
            .queue
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .drain(..)
            .collect();
        waiting
    }
}

/// A thread-safe handle for sending an app's messages from worker threads.
///
/// Returned by [`Ui::proxy`](super::Ui::proxy). Cheap to clone: every clone
/// feeds the same window. `Proxy<M>` is `Send + Sync` whenever `M: Send`.
pub struct Proxy<M> {
    inbox: Arc<Inbox<M>>,
    wake: Arc<Waker>,
}

impl<M: Send> Proxy<M> {
    pub(crate) fn new(inbox: Arc<Inbox<M>>, wake: Waker) -> Proxy<M> {
        Proxy {
            inbox,
            wake: Arc::from(wake),
        }
    }

    /// Sends `msg` to the app for delivery to
    /// [`App::update`](super::App::update), from any thread.
    ///
    /// A burst of sends before the UI thread collects posts a single wake no
    /// matter how many messages it holds. Once the window is gone the message
    /// is handed back as `Err(msg)`; this never panics.
    pub fn send(&self, msg: M) -> Result<(), M> {
        if self.inbox.is_closed() {
            return Err(msg);
        }
        if self.inbox.push(msg) {
            (self.wake)();
        }
        Ok(())
    }
}

impl<M> Clone for Proxy<M> {
    fn clone(&self) -> Proxy<M> {
        Proxy {
            inbox: Arc::clone(&self.inbox),
            wake: Arc::clone(&self.wake),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicUsize;

    use super::*;

    struct Counters {
        wakes: AtomicUsize,
    }

    fn proxy() -> (Proxy<String>, Arc<Inbox<String>>, Arc<Counters>) {
        let inbox = Inbox::new();
        let counters = Arc::new(Counters {
            wakes: AtomicUsize::new(0),
        });
        let weak = Arc::clone(&counters);
        let proxy = Proxy::new(
            Arc::clone(&inbox),
            Box::new(move || {
                weak.wakes.fetch_add(1, Ordering::SeqCst);
            }),
        );
        (proxy, inbox, counters)
    }

    #[test]
    fn a_burst_posts_one_wake_and_keeps_every_message() {
        let (proxy, inbox, counters) = proxy();
        for i in 0..1000 {
            proxy.send(format!("{i}")).unwrap();
        }
        assert_eq!(counters.wakes.load(Ordering::SeqCst), 1);
        assert_eq!(inbox.collect().len(), 1000);
    }

    #[test]
    fn a_new_burst_wakes_again_after_collect() {
        let (proxy, inbox, counters) = proxy();
        proxy.send("a".to_string()).unwrap();
        proxy.send("b".to_string()).unwrap();
        assert_eq!(counters.wakes.load(Ordering::SeqCst), 1);
        assert_eq!(inbox.collect(), vec!["a".to_string(), "b".to_string()]);
        proxy.send("c".to_string()).unwrap();
        assert_eq!(counters.wakes.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn concurrent_bursts_post_one_wake_and_lose_nothing() {
        let (proxy, inbox, counters) = proxy();
        let mut handles = Vec::new();
        for _ in 0..8 {
            let proxy = proxy.clone();
            handles.push(std::thread::spawn(move || {
                for i in 0..500 {
                    proxy.send(format!("{i}")).unwrap();
                }
            }));
        }
        for handle in handles {
            handle.join().expect("worker panicked");
        }
        assert_eq!(counters.wakes.load(Ordering::SeqCst), 1);
        assert_eq!(inbox.collect().len(), 4000);
    }

    #[test]
    fn a_send_after_close_hands_the_message_back() {
        let inbox = Inbox::new();
        let proxy = Proxy::new(Arc::clone(&inbox), Box::new(|| {}));
        inbox.close();
        assert_eq!(proxy.send("late".to_string()), Err("late".to_string()));
    }

    #[test]
    fn proxy_is_clone_send_sync() {
        fn assert_clone_send_sync<T: Clone + Send + Sync>() {}
        assert_clone_send_sync::<Proxy<String>>();
    }
}
