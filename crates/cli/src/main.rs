//! The `retros` command line.
//!
//! The command line is a thin adapter around the engine. It finds `retros.toml`, runs measurement
//! scripts, reads the files they write, prints what the engine reports, and writes the report
//! files. It exits with status 0 on success, 1 when a category fails, and 2 when it cannot run,
//! such as for an unknown argument or an invalid `retros.toml`.

mod check;
mod measure;
mod project;
mod score;
mod script;

use std::io::Write;
use std::process::ExitCode;

use engine::error::Error;

const USAGE: &str = "\
Usage: retros [category] [--update] [--require-baseline]
       retros check

Run each category's script, score its metrics, list the opportunities that would raise the
scores most, and compare sizes, counts, checks, and line targets with retros.lock. Reports go
to .retros/report.json and .retros/report.md. Name a category to run only that one.

Commands:
  check  Run each script once and check the metric lines it writes

Options:
  --update            Record this run's sizes, counts, checks, and line targets in retros.lock
  --require-baseline  Exit with status 1 when anything differs from retros.lock
  -h, --help          Print help
  -V, --version       Print version
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    match args.as_slice() {
        [flag] if flag == "--help" || flag == "-h" => {
            let _ = std::io::stdout().write_all(USAGE.as_bytes());
            ExitCode::SUCCESS
        }
        [flag] if flag == "--version" || flag == "-V" => {
            let _ = writeln!(std::io::stdout(), "retros {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        [command] if command == "check" => with_project(check::run),
        [command, arg, ..] if command == "check" => usage(&format!("unknown argument '{arg}'")),
        _ => {
            let mut options = score::Options::default();

            for arg in &args {
                match arg.as_str() {
                    "--update" => options.update = true,
                    "--require-baseline" => options.require_baseline = true,
                    flag if flag.starts_with('-') => {
                        return usage(&format!("unknown argument '{flag}'"));
                    }
                    name if options.category.is_none() => options.category = Some(name.to_owned()),
                    extra => return usage(&format!("unexpected argument '{extra}'")),
                }
            }

            with_project(|project| score::run(project, &options))
        }
    }
}

/// Finds the project and runs `command` on it. A message from either goes to standard error with
/// exit status 2.
fn with_project(command: impl FnOnce(&project::Project) -> Result<ExitCode, String>) -> ExitCode {
    let result = std::env::current_dir()
        .map_err(|error| format!("error: could not read the current directory: {error}"))
        .and_then(|cwd| project::find(&cwd))
        .and_then(|project| command(&project));

    result.unwrap_or_else(|message| {
        let _ = writeln!(std::io::stderr(), "{message}");
        ExitCode::from(2)
    })
}

/// Renders an engine error with its severity and code, such as
/// `error[unknown-unit] footprint metrics:3:18: unknown unit ...`, followed by the source line.
fn diagnostic(severity: &str, error: &Error, name: &str, source: &str) -> String {
    format!(
        "{severity}[{}] {}",
        error.code.as_str(),
        error.render(name, source)
    )
}

/// Writes an error and the usage to standard error, and returns the usage exit code.
#[cold]
fn usage(message: &str) -> ExitCode {
    let _ = write!(std::io::stderr(), "error: {message}\n\n{USAGE}");
    ExitCode::from(2)
}
