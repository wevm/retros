//! The `uphill` command line.
//!
//! The command line is a thin adapter around the engine. incur parses the arguments and prints
//! help, the version, the `--llms` manifest, and skills. Each command finds the configuration,
//! runs measurement scripts, reads the files they write, prints what the engine reports, and
//! writes the report files. It exits with status 0 on success, 1 when a category fails, and 2
//! when it cannot run, such as for an unknown argument or an invalid configuration.

mod check;
mod dashboard;
mod fetch;
mod history;
mod init;
mod measure;
mod packs;
mod project;
mod rules;
mod score;
mod script;
mod worktree;

use std::io::Write;
use std::process::ExitCode;
use std::sync::{Arc, Mutex};

use engine::error::Error;
use incur::{Io, Parser};

/// Score metrics against their targets and compare them with their baselines
#[derive(Parser)]
#[incur(name = "uphill", version)]
enum Command {
    /// Pin a pack to a commit and list it in the configuration: owner/repo/path on GitHub, an https URL followed by //path, or a local ./path, with an optional @ref
    Add {
        /// The pack, such as wevm/uphill/packs/rust@main
        #[incur(arg)]
        pack: String,
        /// The pack's categories to run, or every category
        #[incur(arg)]
        categories: Option<Vec<String>>,
        /// Take a commit less than a day old
        allow_fresh: bool,
        /// Write without asking
        yes: bool,
    },
    /// Name the commit that last moved a metric: from its baselines, or by measuring commits since <ref>, or every commit, in temporary checkouts and bisecting
    Blame {
        /// The category of the metric
        #[incur(arg)]
        category: String,
        /// The metric to trace
        #[incur(arg)]
        metric: String,
        /// One item of a list metric
        #[incur(arg)]
        item: Option<String>,
        /// Bisect only the commits since this revision
        since: Option<String>,
        /// The subject of the metric, when a category's lines have subjects
        subject: Option<String>,
    },
    /// Run each script once and check the metric lines it writes
    Check,
    /// Write .uphill/dashboard.html from the latest noted runs of each repository: a Git URL, a local path, or owner/name on GitHub
    Dashboard {
        /// A Git URL, a local path, or owner/name on GitHub
        #[incur(arg)]
        repositories: Vec<String>,
    },
    /// Print a brief for an agent to set up Uphill in this project
    Init,
    /// List the commits that changed a metric's baseline, or the runs noted for it
    Log {
        /// The category of the metric
        #[incur(arg)]
        category: String,
        /// The metric to trace
        #[incur(arg)]
        metric: String,
        /// One item of a list metric
        #[incur(arg)]
        item: Option<String>,
        /// The subject of the metric, when a category's lines have subjects
        subject: Option<String>,
    },
    /// Remove a pack from the configuration
    Remove {
        /// The pack, by its directory name, such as rust
        #[incur(arg)]
        pack: String,
    },
    /// Print each metric's rule and targets, for an agent to follow while it writes code
    Rules {
        /// Print only this category's rules
        #[incur(arg)]
        category: Option<String>,
        /// Write the rules to .agents/uphill/rules.md, and @-mention that file from AGENTS.md
        write: bool,
    },
    /// Move each remote pack, or the named ones, to the newest commit of the ref it follows
    Update {
        /// The packs to update, by their directory names, or every remote pack
        #[incur(arg)]
        packs: Option<Vec<String>>,
        /// Take a commit less than a day old
        allow_fresh: bool,
        /// Write without asking
        yes: bool,
    },
}

fn main() -> ExitCode {
    // `uphill [category]` runs inside incur, which reports only an exit code for it.
    let scored = Arc::new(Mutex::new(None));
    let record = scored.clone();

    let cli = Command::cli()
        // Update notices suggest `--update`, which writes baselines here.
        .update(false)
        .handle(move |options: score::Options, _| {
            let code = score(&options);
            if let Ok(mut scored) = record.lock() {
                *scored = Some(code);
            }
        });

    // incur reads `--version` only, and `-V` stays the short form. incur also reads a leading
    // `--update` as the `update` command, so the flag takes its explicit value.
    let argv: Vec<String> = std::env::args()
        .skip(1)
        .map(|arg| match arg.as_str() {
            "--update" => "--update=true".to_owned(),
            "-V" => "--version".to_owned(),
            _ => arg,
        })
        .collect();
    let stdout = std::io::stdout();
    let mut out = Vec::new();
    let parsed = cli.parse_with::<Command>(
        &argv,
        &mut Io {
            env: &|key| std::env::var(key).ok(),
            stderr: &mut std::io::stderr(),
            // `parse_with` serves no MCP, so nothing reads standard input.
            stdin: &mut futures_util::stream::empty(),
            stdout: &mut out,
            tty: std::io::IsTerminal::is_terminal(&stdout),
        },
    );

    // Commands write their own output. Without a command, incur wrote help, the version, or a
    // built-in's output, or rejected the arguments, which goes to standard error.
    let code = match parsed {
        Ok(command) => run(command),
        Err(code) => match scored.lock().ok().and_then(|scored| *scored) {
            Some(scored) => scored,
            None if code == 0 => ExitCode::SUCCESS,
            None => {
                let _ = std::io::stderr().write_all(&out);
                return ExitCode::from(2);
            }
        },
    };
    let _ = stdout.lock().write_all(&out);
    code
}

/// Runs `uphill [category]`.
fn score(options: &score::Options) -> ExitCode {
    if options
        .compare
        .as_ref()
        .is_some_and(|reference| reference.starts_with('-'))
    {
        return usage("--compare needs a revision, such as --compare main");
    }
    with_project(|project| score::run(project, options))
}

/// Runs a command other than `uphill [category]`.
fn run(command: Command) -> ExitCode {
    match command {
        Command::Add {
            allow_fresh,
            categories,
            pack,
            yes,
        } => report(packs::add(
            &pack,
            &categories.unwrap_or_default(),
            packs::Flags { allow_fresh, yes },
        )),
        Command::Blame {
            category,
            item,
            metric,
            since,
            subject,
        } => {
            let selector = history::Selector {
                category,
                item,
                metric,
                subject,
            };
            with_project(|project| history::blame(project, &selector, since.as_deref()))
        }
        Command::Check => with_project(check::run),
        Command::Dashboard { repositories } => report(dashboard::run(&repositories)),
        Command::Init => init::run(),
        Command::Log {
            category,
            item,
            metric,
            subject,
        } => {
            let selector = history::Selector {
                category,
                item,
                metric,
                subject,
            };
            with_project(|project| history::log(project, &selector))
        }
        Command::Remove { pack } => report(packs::remove(&pack)),
        Command::Rules { category, write } => {
            with_project(|project| rules::run(project, category.as_deref(), write))
        }
        Command::Update {
            allow_fresh,
            packs,
            yes,
        } => report(packs::update(
            &packs.unwrap_or_default(),
            packs::Flags { allow_fresh, yes },
        )),
    }
}

/// Returns a command's exit code, or writes its message to standard error with exit status 2.
fn report(result: Result<ExitCode, String>) -> ExitCode {
    result.unwrap_or_else(|message| {
        let _ = writeln!(std::io::stderr(), "{message}");
        ExitCode::from(2)
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

/// Writes an error to standard error, and returns the usage exit code.
#[cold]
fn usage(message: &str) -> ExitCode {
    let _ = writeln!(std::io::stderr(), "error: {message}");
    ExitCode::from(2)
}
