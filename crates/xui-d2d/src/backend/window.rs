#![forbid(unsafe_code)]

//! Opening and closing top-level windows.

use std::rc::Rc;

use xui_core::Theme;
use xui_core::backend::{BackendError, PlatformSpec, Result, WindowId};

use super::{Shared, WindowState};
use crate::sys::{settings, window};

/// Daylight or Nightfall, after the user's dark-mode setting.
pub(super) fn system_theme() -> Theme {
    if settings::prefers_dark().unwrap_or(false) {
        Theme::nightfall()
    } else {
        Theme::daylight()
    }
}

impl Shared {
    pub(super) fn open(&self, spec: &PlatformSpec) -> Result<WindowId> {
        window::init();
        let id = WindowId::from_raw(Shared::allocate(&self.next_window));
        let this = self.this.clone();
        let handler: window::Handler = Rc::new(move |hwnd, msg| {
            this.upgrade()
                .and_then(|shared| shared.handle(id, hwnd, msg))
        });
        let hwnd = window::create(
            &spec.title,
            spec.width,
            spec.height,
            spec.resizable,
            handler,
        )
        .ok_or(BackendError::CreateFailed("window"))?;
        let theme = system_theme();
        window::set_dark_title_bar(hwnd, theme.is_dark);
        // The first window ever opened is the primary one, whatever has closed since.
        let primary = id.raw() == 1;
        self.windows.borrow_mut().insert(
            id.raw(),
            WindowState {
                hwnd,
                sink: None,
                target: None,
                theme,
                dpi: window::dpi(hwnd),
                size: window::client_size(hwnd),
                hover: None,
                focused: None,
                tracking: false,
                primary,
            },
        );
        Ok(id)
    }

    /// Forgets `window`: its nodes, timers and capture, and its entry. Returns
    /// the entry, if the window was still known.
    pub(super) fn release(&self, window: WindowId) -> Option<WindowState> {
        self.unlock_window(window, false);
        if self.captured.get().and_then(|id| self.window_of(id)) == Some(window) {
            self.captured.set(None);
        }
        let removed = self.scene.borrow_mut().remove_window(window);
        // The child windows go with their parent.
        for id in removed {
            self.drop_surface(id, window, true);
        }
        self.orphans_gone(window);
        self.timers
            .borrow_mut()
            .retain(|_, owner| *owner != window.raw());
        self.windows.borrow_mut().remove(&window.raw())
    }

    pub(super) fn close(&self, window: WindowId) {
        if let Some(state) = self.release(window) {
            window::destroy(state.hwnd);
        }
    }
}
