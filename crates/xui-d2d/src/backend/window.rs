#![forbid(unsafe_code)]

//! Opening and closing top-level windows.

use std::rc::Rc;

use xui_core::Theme;
use xui_core::backend::{BackendError, PlatformSpec, Result, WindowIcon, WindowId};

use super::{Shared, WindowState};
use crate::sys::{icon, settings, window};

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
                icon: None,
                icons: None,
            },
        );
        // A window whose spec has an icon gets it from the runtime right
        // away; loading the default first would only be thrown away.
        if spec.icon.is_none() {
            self.show_icon(id, None);
        }
        Ok(id)
    }

    /// Sets the icon `window` shows instead of the executable's default. An
    /// icon that fails to load (a missing resource, no images) is ignored, so
    /// the window keeps the icon it shows and follows DPI changes with it.
    pub(super) fn set_icon(&self, window: WindowId, icon: WindowIcon) {
        self.show_icon(window, Some(icon));
    }

    /// Reloads `window`'s icon at its current DPI, as after a DPI change.
    pub(super) fn reload_icon(&self, window: WindowId) {
        let icon = self
            .windows
            .borrow()
            .get(&window.raw())
            .and_then(|state| state.icon.clone());
        self.show_icon(window, icon);
    }

    /// Loads `icon` (the executable's default when `None`) at `window`'s
    /// current DPI, shows it and remembers it. The class icon is loaded once
    /// at the system DPI; this keeps the title bar and task bar icons sharp
    /// on a monitor at another DPI, and after a DPI change.
    fn show_icon(&self, window: WindowId, icon: Option<WindowIcon>) {
        let Some((hwnd, dpi)) = self
            .windows
            .borrow()
            .get(&window.raw())
            .map(|state| (state.hwnd, state.dpi))
        else {
            return;
        };
        let Some(icons) = icon::load(icon.as_ref(), dpi) else {
            return;
        };
        // `WM_SETICON` is sent to this window's own procedure, so no state is
        // borrowed while it runs.
        icon::apply(hwnd, &icons);
        // The icons shown until now are released only once the window no
        // longer shows them.
        let replaced = self
            .windows
            .borrow_mut()
            .get_mut(&window.raw())
            .map(|state| {
                state.icon = icon;
                state.icons.replace(icons)
            });
        drop(replaced);
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
        self.forget_frames(window);
        self.windows.borrow_mut().remove(&window.raw())
    }

    pub(super) fn close(&self, window: WindowId) {
        if let Some(state) = self.release(window) {
            window::destroy(state.hwnd);
        }
        if self.windows.borrow().is_empty() {
            // `Shared` is not reliably dropped before the process exits (a probe
            // in its `Drop` never fired), so the cached Direct2D objects are
            // released here, not left to thread-local destructors at teardown.
            crate::sys::gfx::release_thread_resources();
        }
    }
}
