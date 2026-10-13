#![forbid(unsafe_code)]

//! `xui-form render`: the form in light and dark, as PNGs, with no window.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use xui_canvas::snapshot::{Snapshot, Stage, render_with};
use xui_core::app::{App, Ui};
use xui_core::backend::BackendError;
use xui_core::{Image, Theme};
use xui_form::{Factories, Form, Handlers, LiveForm};
use xui_script::Msg;
use xui_script::form::{ScriptForm, ScriptSource};

use crate::args::{self, Args, Failure, Outcome};

/// A plain form, kept alive while it is captured.
struct Plain {
    _form: LiveForm<()>,
}

impl App for Plain {
    type Msg = ();

    fn update(&mut self, _msg: (), _ui: &mut Ui<()>) {}
}

/// A scripted form, kept alive while it is captured.
struct Scripted {
    _form: ScriptForm,
}

impl App for Scripted {
    type Msg = Msg;

    fn update(&mut self, _msg: Msg, _ui: &mut Ui<Msg>) {}
}

/// What to render and how.
struct Options {
    out: PathBuf,
    script: Option<(String, String)>,
    dpi: u32,
    overlay: bool,
    report: bool,
}

/// Renders the file named to `<out>/<name>-light.png` and `-dark.png`.
pub(crate) fn run(mut args: Args) -> Outcome {
    let out = PathBuf::from(args.option("out")?.unwrap_or_else(|| ".".to_owned()));
    let script = match args.option("script")? {
        Some(path) => Some((path.clone(), args::read(path.as_ref())?)),
        None => None,
    };
    let dpi = match args.option("dpi")? {
        Some(dpi) => dpi
            .parse()
            .map_err(|_| Failure::Usage(format!("`--dpi {dpi}` is not a number")))?,
        None => 96,
    };
    let options = Options {
        out,
        script,
        dpi,
        overlay: args.flag("overlay"),
        report: args.flag("report"),
    };
    let Some(file) = args.next_positional() else {
        return Err(Failure::Usage("`render` needs a file".to_owned()));
    };
    args.finish()?;
    let file = PathBuf::from(file);
    let form = args::load(&file)?;
    let stem = file
        .file_stem()
        .map_or_else(|| "form".into(), |stem| stem.to_string_lossy());
    for (variant, theme) in [("light", Theme::light()), ("dark", Theme::dark())] {
        // The report is the same in both themes; print it once.
        let report = options.report && variant == "light";
        let image = capture(&form, theme, &options, report)
            .map_err(|error| Failure::Fatal(format!("{}: {error}", file.display())))?;
        let path = options.out.join(format!("{stem}-{variant}.png"));
        save(&image, &path)?;
        println!("{}", path.display());
    }
    Ok(ExitCode::SUCCESS)
}

/// The form rendered in `theme`, its script run first when there is one.
fn capture(form: &Form, theme: Theme, options: &Options, report: bool) -> Result<Image, String> {
    let mut snapshot = Snapshot::new(form.size.0.dip(), form.size.1.dip())
        .theme(theme)
        .dpi(options.dpi)
        .title(form.title.clone());
    if options.overlay {
        snapshot = snapshot.with_layout_overlay();
    }
    let failed = |error: &dyn std::fmt::Display| BackendError::Other(error.to_string());
    let image = match &options.script {
        None => render_with(
            snapshot,
            |ui| {
                let live = xui_form::build(ui, form, &Factories::xui(), &Handlers::new())
                    .map_err(|error| failed(&error))?;
                Ok(Plain { _form: live })
            },
            move |stage: &Stage<'_, ()>| print_report(report, stage.ui()),
        ),
        Some((file, code)) => render_with(
            snapshot,
            |ui| {
                let source = ScriptSource {
                    name: &form.name,
                    code,
                    file,
                };
                let scripted = ScriptForm::build(ui, form, source, (), |_| Ok(()))
                    .map_err(|error| failed(&error))?;
                Ok(Scripted { _form: scripted })
            },
            move |stage: &Stage<'_, Msg>| print_report(report, stage.ui()),
        ),
    };
    image.map_err(|error| error.to_string())
}

/// Prints the layout report: every widget's rectangle, and warnings for
/// clipped, empty, truncated or overlapping ones.
fn print_report<M: 'static>(report: bool, ui: &Ui<M>) {
    if report {
        print!("{}", ui.layout_report());
    }
}

/// Saves `image` to `path`, creating its directory.
fn save(image: &Image, path: &Path) -> Result<(), Failure> {
    let fail =
        |error: &dyn std::fmt::Display| Failure::Fatal(format!("{}: {error}", path.display()));
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|error| fail(&error))?;
    }
    image.save_png(path).map_err(|error| fail(&error))
}
