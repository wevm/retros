//! `retros [category]`: runs each category's script, scores its metrics, prints each category as
//! it finishes, then the opportunities and the changes since `retros.lock`. It writes
//! `.retros/report.json` and `.retros/report.md` beside `retros.toml`, and with `--update`
//! records this run's sizes, counts, and checks in `retros.lock`.
//!
//! The command exits with status 0 when every category ran and passed its checks, and with
//! status 1 when any category failed. With `--require-baseline`, it also exits with status 1 when
//! a size, count, or check is worse than `retros.lock`. Scores do not change the exit status.

use std::io::{self, Write as _};
use std::process::ExitCode;

use engine::config::Category;
use engine::lock::{self, Direction};
use engine::report::{CategoryReport, Report};

use crate::measure;
use crate::project::Project;

/// What `retros [category]` was asked to do.
#[derive(Default)]
pub(crate) struct Options {
    /// The category to run, or `None` for every category.
    pub(crate) category: Option<String>,
    /// Whether a size, count, or check worse than `retros.lock` fails the run.
    pub(crate) require_baseline: bool,
    /// Whether to record this run in `retros.lock`.
    pub(crate) update: bool,
}

/// Runs every category of `project`, or only the category `options` names.
///
/// # Errors
///
/// Returns a message for the terminal when the category is unknown, when `retros.lock` is
/// invalid, when Retros cannot run a script at all, or when it cannot write a file.
pub(crate) fn run(project: &Project, options: &Options) -> Result<ExitCode, String> {
    let prefix = project.name.trim_end_matches("retros.toml");
    let lock_name = format!("{prefix}retros.lock");
    let lock_path = project.root.join("retros.lock");
    let previous = match std::fs::read_to_string(&lock_path) {
        Ok(source) => Some(
            lock::parse(&source)
                .map_err(|error| crate::diagnostic("error", &error, &lock_name, &source))?,
        ),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(format!("error: could not read {lock_name}: {error}")),
    };

    let categories: Vec<&Category> = match options.category.as_deref() {
        None => project.config.categories.iter().collect(),
        Some(name) => {
            let category = project
                .config
                .categories
                .iter()
                .find(|category| category.name == name)
                .ok_or_else(|| {
                    let names: Vec<&str> = project
                        .config
                        .categories
                        .iter()
                        .map(|category| category.name.as_str())
                        .collect();
                    format!(
                        "error: unknown category '{name}'. {} declares {}",
                        project.name,
                        names.join(", ")
                    )
                })?;
            vec![category]
        }
    };

    let mut report = Report {
        categories: Vec::new(),
        changes: None,
    };

    for category in categories {
        let mut warnings = Vec::new();

        let result = match measure::category(project, category, true)? {
            Err(failure) => Err(failure),
            Ok(measured) => {
                let rounds = measured
                    .sources
                    .iter()
                    .map(|source| engine::lines::parse(source))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|error| format!("error: {}", error.message))?;

                warnings = measured.warnings;
                Ok(engine::score::category(category, &rounds))
            }
        };

        let entry = CategoryReport {
            name: category.name.clone(),
            result,
        };

        let mut block = if report.categories.is_empty() {
            String::new()
        } else {
            "\n".to_owned()
        };
        block.push_str(&engine::report::terminal(&entry));
        for line in warnings.iter().flat_map(|warning| warning.lines()) {
            block.push_str("    ");
            block.push_str(line);
            block.push('\n');
        }

        let mut stdout = io::stdout().lock();
        let _ = stdout.write_all(block.as_bytes());
        let _ = stdout.flush();

        report.categories.push(entry);
    }

    // A first `--update` compares with an empty lock, so it lists every metric it records as new.
    report.changes = match (&previous, options.update) {
        (Some(lock), _) => Some(lock::compare(lock, &report)),
        (None, true) => Some(lock::compare(&lock::Lock::default(), &report)),
        (None, false) => None,
    };
    let _ = io::stdout().write_all(engine::report::summary(&report).as_bytes());

    let dir = project.root.join(".retros");
    let write = |file: &str, contents: String| {
        std::fs::write(dir.join(file), contents)
            .map_err(|error| format!("error: could not write .retros/{file}: {error}"))
    };
    std::fs::create_dir_all(&dir)
        .map_err(|error| format!("error: could not create .retros: {error}"))?;
    write("report.json", engine::report::json(&report))?;
    write("report.md", engine::report::markdown(&report))?;

    let mut written = format!("{prefix}.retros/report.json, {prefix}.retros/report.md");
    if options.update {
        let updated = lock::update(previous.as_ref(), &project.config, &report);
        std::fs::write(&lock_path, lock::render(&updated))
            .map_err(|error| format!("error: could not write {lock_name}: {error}"))?;
        written.push_str(", ");
        written.push_str(&lock_name);
    }
    let _ = writeln!(io::stdout(), "\nWrote {written}");

    let failed = report
        .categories
        .iter()
        .any(|category| category.result.is_err());
    let worse = report
        .changes
        .iter()
        .flatten()
        .filter(|change| change.direction == Direction::Worse)
        .count();

    if options.require_baseline && !options.update && worse > 0 {
        let _ = writeln!(
            io::stdout(),
            "\n{} worse than {lock_name}",
            measure::count(worse, "value")
        );
        return Ok(ExitCode::FAILURE);
    }

    Ok(if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}
