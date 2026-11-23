//! The `retros` command line.
//!
//! The command line is a thin adapter around the engine. It finds the configuration, runs measurement
//! scripts, reads the files they write, prints what the engine reports, and writes the report
//! files. It exits with status 0 on success, 1 when a category fails, and 2 when it cannot run,
//! such as for an unknown argument or an invalid configuration.

mod check;
mod history;
mod init;
mod measure;
mod project;
mod score;
mod script;
mod worktree;

use std::io::Write;
use std::process::ExitCode;

use engine::error::Error;

const USAGE: &str = "\
Usage: retros [category] [--compare <ref>] [--compact] [--note] [--update]
              [--require-baseline] [--require-targets]
       retros check
       retros init
       retros log <category> <metric> [item] [--subject <name>]
       retros blame <category> <metric> [item] [--subject <name>] [--since <ref>]

Run each category's script, score its metrics, list the opportunities that would raise the
scores most, and compare sizes, counts, checks, and line targets with each metric's baseline.
Retros reads retros.yaml or retros.toml, at the root or in .agents, from this directory or the
nearest parent. Reports go to .retros/report.json, report.md, and report.html. Name a category
to run only that one.

Commands:
  blame  Name the commit that last moved a metric: from its baselines, or by measuring commits
         since <ref>, or every commit, in temporary checkouts and bisecting
  check  Run each script once and check the metric lines it writes
  init   Print a brief for an agent to set up Retros in this project
  log    List the commits that changed a metric's baseline, or the runs noted for it

Options:
  --compare <ref>     Compare with another revision: timings in interleaved pairs, and sizes,
                      counts, and checks with its baselines
  --compact           Print the scores, missed targets, and opportunities in a few lines
  --note              Keep this run as a Git note on HEAD, for trends and `retros log`
  --update            Write this run's sizes, counts, checks, and line targets as baselines
  --require-baseline  Exit with status 1 when anything differs from the baselines
  --require-targets   Exit with status 1 when a metric misses its target or a script fails
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
        [command] if command == "init" => init::run(),
        [command, arg, ..] if command == "init" => usage(&format!("unknown argument '{arg}'")),
        [command, args @ ..] if command == "log" || command == "blame" => history(command, args),
        _ => {
            let mut options = score::Options::default();
            let mut args = args.iter();

            while let Some(arg) = args.next() {
                match arg.as_str() {
                    "--compare" => match args.next() {
                        Some(reference) if !reference.starts_with('-') => {
                            options.compare = Some(reference.clone());
                        }
                        _ => return usage("--compare needs a revision, such as --compare main"),
                    },
                    "--compact" => options.compact = true,
                    "--note" => options.note = true,
                    "--update" => options.update = true,
                    "--require-baseline" => options.require_baseline = true,
                    "--require-targets" => options.require_targets = true,
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

/// Runs `retros log` or `retros blame` with the arguments after the command.
fn history(command: &str, args: &[String]) -> ExitCode {
    let mut names = Vec::new();
    let (mut since, mut subject) = (None, None);
    let mut args = args.iter();

    while let Some(arg) = args.next() {
        let value = match arg.as_str() {
            "--since" if command == "blame" => &mut since,
            "--subject" => &mut subject,
            flag if flag.starts_with('-') => return usage(&format!("unknown argument '{flag}'")),
            name => {
                names.push(name.to_owned());
                continue;
            }
        };
        match args.next() {
            Some(next) if !next.starts_with('-') => *value = Some(next.clone()),
            _ => return usage(&format!("{arg} needs a value")),
        }
    }

    let mut names = names.into_iter();
    let (Some(category), Some(metric), item, None) =
        (names.next(), names.next(), names.next(), names.next())
    else {
        return usage(&format!(
            "{command} needs a category, a metric, and optionally an item, such as `retros {command} footprint bundle`"
        ));
    };
    let selector = history::Selector {
        category,
        item,
        metric,
        subject,
    };

    with_project(|project| {
        if command == "log" {
            history::log(project, &selector)
        } else {
            history::blame(project, &selector, since.as_deref())
        }
    })
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
