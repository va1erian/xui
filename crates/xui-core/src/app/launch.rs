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
                layout_report(ui);
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

/// Prints the layout report and its warning count when `XUI_LAYOUT_REPORT` is
/// set, so a headless run (a snapshot, a CI step) can assert that no layout
/// warns. The count is the last line, `XUI_LAYOUT_WARNINGS:<n>`.
fn layout_report<M: 'static>(ui: &Ui<M>) {
    if std::env::var_os("XUI_LAYOUT_REPORT").is_none() {
        return;
    }
    let report = ui.layout_report();
    let warnings = layout_warnings(&report);
    print!("{report}");
    if !report.is_empty() && !report.ends_with('\n') {
        println!();
    }
    println!("XUI_LAYOUT_WARNINGS:{warnings}");
}

/// The number of warnings in a layout report: its `! ` lines, whatever their
/// indentation (a node's own line may hold a `!` in its text).
fn layout_warnings(report: &str) -> usize {
    report
        .lines()
        .filter(|line| line.trim_start().starts_with("! "))
        .count()
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

#[cfg(test)]
mod tests {
    use super::layout_warnings;

    #[test]
    fn warnings_are_the_bang_lines_of_a_report() {
        let report = "\
layout in the window, at 0,0 size 400x300
  column at 0,0 size 400x300
    Label \"! loud\" at 8,8 size 40x28
    Button \"OK\" at 8,36 size 0x0
      ! zero size
    Label \"x\" at 8,64 size 10x28
      ! text truncated: needs 90px wide
";
        assert_eq!(layout_warnings(report), 2);
        assert_eq!(layout_warnings("layout in the window\n  column\n"), 0);
        assert_eq!(layout_warnings(""), 0);
    }
}
