#![forbid(unsafe_code)]

//! [`app`]: one call from a title to a running window.

use std::cell::RefCell;
use std::rc::Rc;

use super::{App, Ui, run_app};
use crate::backend::{Backend, BackendError, PlatformSpec, Result};
use crate::units::Dip;

/// Starts describing an app window titled `title`:
///
/// ```ignore
/// xui_core::app("Hello")
///     .size(360, 160)
///     .backend(backend)
///     .run(|ui| {
///         ui.root(column().child(label("Hi")))?;
///         Ok(Hello)
///     })
/// ```
///
/// The `xui` umbrella crate's `xui::app` picks the backend itself.
pub fn app(title: impl Into<String>) -> Launch {
    Launch {
        spec: PlatformSpec::new(title),
        backend: None,
    }
}

/// An app window about to run; see [`app`].
pub struct Launch {
    spec: PlatformSpec,
    backend: Option<Rc<dyn Backend>>,
}

impl Launch {
    /// The window's initial client size, in design units.
    pub fn size(mut self, width: impl Into<Dip>, height: impl Into<Dip>) -> Launch {
        self.spec = self.spec.size(width.into(), height.into());
        self
    }

    /// Adjusts the rest of the window's [`PlatformSpec`]:
    /// `.spec(|spec| spec.resizable(false))`.
    pub fn spec(mut self, f: impl FnOnce(PlatformSpec) -> PlatformSpec) -> Launch {
        self.spec = f(self.spec);
        self
    }

    /// The backend the window runs on.
    pub fn backend(mut self, backend: Rc<dyn Backend>) -> Launch {
        self.backend = Some(backend);
        self
    }

    /// Opens the window, builds the app with `make` and runs the event loop
    /// until the window closes or the app quits. An error from `make` (the
    /// first widget that failed to build, say) closes the window and is
    /// returned.
    ///
    /// When `XUI_DEMO_AUTOCLOSE_MS` is set the app quits after that many
    /// milliseconds, so every app can be smoke-tested unattended.
    pub fn run<A, F>(self, make: F) -> Result<()>
    where
        A: App,
        F: FnOnce(&mut Ui<A::Msg>) -> Result<A>,
    {
        let backend = self
            .backend
            .ok_or(BackendError::Unsupported("an app with no backend"))?;
        let failure: Rc<RefCell<Option<BackendError>>> = Rc::new(RefCell::new(None));
        let failed = Rc::clone(&failure);
        run_app(backend, self.spec, move |ui| match make(ui) {
            Ok(app) => {
                autoclose(ui);
                Launched::Running(app)
            }
            Err(error) => {
                *failed.borrow_mut() = Some(error);
                ui.quit_with(1);
                Launched::Failed
            }
        })?;
        match failure.take() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

/// Quits after `XUI_DEMO_AUTOCLOSE_MS` milliseconds, when it is set.
fn autoclose<M: 'static>(ui: &Ui<M>) {
    let Some(millis) = std::env::var("XUI_DEMO_AUTOCLOSE_MS")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
    else {
        return;
    };
    let at = ui.set_timer(millis);
    let quit = ui.clone();
    ui.add_timer_listener(move |fired| {
        if fired == at {
            quit.quit();
        }
        None
    });
}

/// The app `make` built, or nothing when it failed.
enum Launched<A> {
    Running(A),
    Failed,
}

impl<A: App> App for Launched<A> {
    type Msg = A::Msg;

    fn update(&mut self, msg: A::Msg, ui: &mut Ui<A::Msg>) {
        if let Launched::Running(app) = self {
            app.update(msg, ui);
        }
    }
}
