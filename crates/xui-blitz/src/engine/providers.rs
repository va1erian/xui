//! The engine's answers to a Blitz document's calls out: redraws, the
//! pointer, the clipboard and navigation. Blitz calls them on the engine
//! thread, in the middle of its own work, so each just queues a command for
//! the engine's loop.

use std::sync::Mutex;
use std::sync::mpsc::Sender;

use blitz_traits::navigation::{NavigationOptions, NavigationProvider};
use blitz_traits::shell::{ClipboardError, ShellProvider};
use cursor_icon::CursorIcon;

use super::Command;

/// The queue a document's providers post to. `Sender` is `Send` but the
/// providers must be `Sync` too, hence the lock.
pub(crate) struct Post(Mutex<Sender<Command>>);

impl Post {
    pub(crate) fn new(tx: Sender<Command>) -> Post {
        Post(Mutex::new(tx))
    }

    fn send(&self, command: Command) {
        let tx = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let _ = tx.send(command);
    }
}

impl ShellProvider for Post {
    fn request_redraw(&self) {
        self.send(Command::Redraw);
    }

    fn set_cursor(&self, icon: Option<CursorIcon>) {
        self.send(Command::Cursor(icon));
    }

    fn set_clipboard_text(&self, text: String) -> Result<(), ClipboardError> {
        self.send(Command::Copy(text));
        Ok(())
    }
}

impl NavigationProvider for Post {
    fn navigate_to(&self, options: NavigationOptions) {
        self.send(Command::Link(Box::new(options)));
    }
}
