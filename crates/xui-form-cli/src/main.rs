#![forbid(unsafe_code)]

//! `xui-form`: work on `.lfm` forms without compiling anything.
//!
//! ```text
//! xui-form check FILE...                     validate; exit 1 on an error
//! xui-form render FILE [--out DIR] [--script FILE.rhai] [--dpi N]
//!                      [--overlay] [--report]
//!                                            light and dark PNGs, headlessly
//! xui-form fmt FILE... [--check]             rewrite canonically
//! xui-form migrate FILE [--out FILE]         format 1 (TOML) to format 2
//! xui-form schema --json                     every kind, property and event
//! ```

use std::process::ExitCode;

mod args;
mod check;
mod fmt;
mod migrate;
mod render;
mod schema;

use args::Args;

/// The usage text, printed for `--help` and after a usage error.
const USAGE: &str = "\
usage: xui-form <command> [options]

commands:
  check FILE...            validate forms; errors exit with status 1
  render FILE              render light and dark PNGs headlessly
      --out DIR            where to write them (default: the current directory)
      --script FILE.rhai   run the form's script (form_load) first
      --dpi N              render at N dots per inch (default: 96)
      --overlay            draw the layout's bounds over the form
      --report             print the layout report (rects and warnings)
  fmt FILE... [--check]    rewrite forms canonically; --check only reports
  migrate FILE [--out F]   convert a format-1 (TOML) form to format 2
  schema --json            print the schema: kinds, properties, events, fields
";

/// The subcommands, for the "did you mean" of a misspelt one.
const COMMANDS: [&str; 5] = ["check", "render", "fmt", "migrate", "schema"];

fn main() -> ExitCode {
    let mut args = Args::from_env();
    let Some(command) = args.next_positional() else {
        eprint!("{USAGE}");
        return ExitCode::from(2);
    };
    let result = match command.as_str() {
        "check" => check::run(args),
        "render" => render::run(args),
        "fmt" => fmt::run(args),
        "migrate" => migrate::run(args),
        "schema" => schema::run(args),
        "help" | "--help" | "-h" => {
            print!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        other => Err(args::Failure::Usage(
            match xui_form::closest(other, COMMANDS) {
                Some(command) => format!("unknown command `{other}` (did you mean `{command}`?)"),
                None => format!("unknown command `{other}`"),
            },
        )),
    };
    match result {
        Ok(code) => code,
        Err(args::Failure::Usage(message)) => {
            eprintln!("xui-form: {message}\n\n{USAGE}");
            ExitCode::from(2)
        }
        Err(args::Failure::Fatal(message)) => {
            eprintln!("xui-form: {message}");
            ExitCode::FAILURE
        }
    }
}
