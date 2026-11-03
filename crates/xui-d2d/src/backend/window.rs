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
        let primary = self.windows.borrow().is_empty();
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

    pub(super) fn close(&self, window: WindowId) {
        if self.captured.get().and_then(|id| self.window_of(id)) == Some(window) {
            self.captured.set(None);
        }
        self.scene.borrow_mut().remove_window(window);
        self.timers
            .borrow_mut()
            .retain(|_, owner| *owner != window.raw());
        let state = self.windows.borrow_mut().remove(&window.raw());
        if let Some(state) = state {
            window::destroy(state.hwnd);
        }
    }
}
