//! [`app`]: a window on the right backend in one call, with headless
//! screenshots for free.

use std::rc::Rc;

use xui_canvas::snapshot::{Gallery, SnapshotError};
use xui_canvas::{OffscreenBackend, WinitBackend};
use xui_core::app::{App, Ui};
use xui_core::backend::{Backend, PlatformSpec, Result};
use xui_core::{Dip, Theme};

/// Starts describing an app window titled `title`, on the backend
/// [`default_backend`] picks:
///
/// ```no_run
/// use xui::prelude::*;
///
/// struct Hello;
///
/// impl App for Hello {
///     type Msg = ();
///     fn update(&mut self, _msg: (), _ui: &mut Ui<()>) {}
/// }
///
/// fn main() -> Result<()> {
///     xui::app("Hello").size(320, 120).run(|ui| {
///         ui.root(column().padding(16).child(label("Hello, world")))?;
///         Ok(Hello)
///     })
/// }
/// ```
pub fn app(title: impl Into<String>) -> Launch {
    Launch {
        inner: xui_core::app(title),
        backend: None,
    }
}

/// An app window about to run; see [`app`].
pub struct Launch {
    inner: xui_core::Launch,
    backend: Option<Rc<dyn Backend>>,
}

impl Launch {
    /// The window's initial client size, in design units.
    pub fn size(mut self, width: impl Into<Dip>, height: impl Into<Dip>) -> Launch {
        self.inner = self.inner.size(width, height);
        self
    }

    /// Adjusts the rest of the window's [`PlatformSpec`].
    pub fn spec(mut self, f: impl FnOnce(PlatformSpec) -> PlatformSpec) -> Launch {
        self.inner = self.inner.spec(f);
        self
    }

    /// Runs on `backend` instead of the default one.
    pub fn backend(mut self, backend: Rc<dyn Backend>) -> Launch {
        self.backend = Some(backend);
        self
    }

    /// Opens the window, builds the app with `make` and runs it, as
    /// [`xui_core::Launch::run`] does.
    ///
    /// With `XUI_SNAPSHOT=<dir>` the app instead renders headlessly, saves
    /// `<exe>-light.png` and `<exe>-dark.png` into `<dir>` and ends; with
    /// `XUI_BACKEND=offscreen` it runs headlessly without saving.
    pub fn run<A, F>(self, make: F) -> Result<()>
    where
        A: App,
        F: FnOnce(&mut Ui<A::Msg>) -> Result<A>,
    {
        let gallery = Gallery::from_env();
        let (backend, offscreen) = match self.backend {
            Some(backend) => (backend, None),
            None if gallery.offscreen() => {
                let offscreen = Rc::new(OffscreenBackend::new());
                (Rc::clone(&offscreen) as Rc<dyn Backend>, Some(offscreen))
            }
            None => (default_backend(), None),
        };
        self.inner.backend(backend).run(move |ui| {
            let app = make(ui)?;
            if let Some(offscreen) = offscreen {
                save_snapshots(&offscreen, ui, gallery);
            }
            Ok(app)
        })
    }
}

/// The backend an app runs on by default: the native Win32 backend where it
/// is built in (the `d2d` feature on Windows) unless `XUI_BACKEND=canvas`,
/// otherwise the portable software backend.
pub fn default_backend() -> Rc<dyn Backend> {
    #[cfg(all(feature = "d2d", windows))]
    {
        if std::env::var("XUI_BACKEND").as_deref() != Ok("canvas") {
            return Rc::new(xui_win32::Win32Backend::new());
        }
    }
    Rc::new(WinitBackend::new())
}

/// Arranges for the offscreen run to save a light and a dark screenshot named
/// after the executable once the app is built, then end.
fn save_snapshots<M: 'static>(backend: &OffscreenBackend, ui: &Ui<M>, gallery: Gallery) {
    if gallery.dir().is_none() {
        return;
    }
    let ui = ui.clone();
    let name = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "app".to_string());
    backend.set_run_hook(move || {
        for (variant, theme) in [("light", Theme::light()), ("dark", Theme::dark())] {
            ui.set_theme(theme);
            let saved = ui
                .capture()
                .map_err(SnapshotError::from)
                .and_then(|image| gallery.save(&name, variant, &image));
            if let Err(error) = saved {
                eprintln!("{name}-{variant}: {error}");
                std::process::exit(1);
            }
        }
    });
}
