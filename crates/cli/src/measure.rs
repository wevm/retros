//! Measures one category: runs its script, reads the metric lines, and checks them against the
//! category's configuration. A category whose first run writes a time runs again until it has
//! run [`engine::score::ROUNDS`] times.

use engine::config::Category;
use engine::report::Failure;
use engine::value::Kind;

use crate::project::Project;
use crate::script::{self, Outcome};

/// The metric lines of each round of a category that passed its checks.
pub(crate) struct Measured {
    /// The number of distinct metrics the first round wrote.
    pub(crate) metrics: usize,
    /// The text each round wrote to `$RETROS_METRICS`, in the order the rounds ran.
    pub(crate) sources: Vec<String>,
    /// Rendered warnings from the first round, such as metrics `retros.toml` does not configure.
    pub(crate) warnings: Vec<String>,
}

/// Runs `category` once, or [`engine::score::ROUNDS`] times when `repeat` is set and the first run
/// writes a time.
///
/// # Errors
///
/// Returns a message for the terminal when Retros cannot run the script at all. A script that
/// fails, or writes lines that fail their checks, returns `Ok(Err(failure))`.
pub(crate) fn category(
    project: &Project,
    category: &Category,
    repeat: bool,
) -> Result<Result<Measured, Failure>, String> {
    let mut measured = Measured {
        metrics: 0,
        sources: Vec::new(),
        warnings: Vec::new(),
    };
    let mut rounds = 1;

    while measured.sources.len() < rounds {
        let round = measured.sources.len() + 1;
        let run = script::run(&category.run, &project.root, category.timeout)
            .map_err(|error| format!("error: could not run `{}`: {error}", category.run))?;

        let message = match run.outcome {
            Outcome::Exited(0) => None,
            Outcome::Exited(code) => Some(format!("script exited with status {code}")),
            Outcome::Signaled(signal) => Some(format!("script was stopped by signal {signal}")),
            Outcome::TimedOut => Some(format!(
                "script timed out after {}",
                engine::format::duration(category.timeout)
            )),
        };
        if let Some(message) = message {
            return Ok(Err(Failure {
                details: run.stderr,
                message: in_round(message, round),
            }));
        }

        let Ok(text) = String::from_utf8(run.metrics) else {
            return Ok(Err(Failure {
                details: String::new(),
                message: in_round("metric lines are not valid UTF-8".to_owned(), round),
            }));
        };
        let label = in_round(format!("{} metrics", category.name), round);

        let lines = match engine::lines::parse(&text) {
            Ok(lines) => lines,
            Err(error) => {
                return Ok(Err(Failure {
                    details: crate::diagnostic("error", &error, &label, &text),
                    message: in_round("invalid metric line".to_owned(), round),
                }));
            }
        };

        let report = engine::check::category(category, &lines);
        let render = |severity, finding: &engine::check::Finding| match finding.source {
            engine::check::Source::Config => {
                crate::diagnostic(severity, &finding.error, &project.name, &project.source)
            }
            engine::check::Source::Metrics => {
                crate::diagnostic(severity, &finding.error, &label, &text)
            }
        };

        if !report.passed() {
            let mut summary = vec![count(report.metrics, "metric")];
            summary.push(count(report.errors.len(), "error"));
            if !report.warnings.is_empty() {
                summary.push(count(report.warnings.len(), "warning"));
            }

            let details: Vec<String> = report
                .errors
                .iter()
                .map(|finding| render("error", finding))
                .chain(
                    report
                        .warnings
                        .iter()
                        .map(|finding| render("warning", finding)),
                )
                .collect();

            return Ok(Err(Failure {
                details: details.join("\n"),
                message: in_round(summary.join(", "), round),
            }));
        }

        if round == 1 {
            measured.metrics = report.metrics;
            measured.warnings = report
                .warnings
                .iter()
                .map(|finding| render("warning", finding))
                .collect();

            let timed = lines.iter().any(|line| line.value.kind() == Kind::Time);
            if repeat && timed {
                rounds = engine::score::ROUNDS;
            }
        }

        drop(lines);
        measured.sources.push(text);
    }

    Ok(Ok(measured))
}

/// Formats a count with its noun, such as `1 metric` or `3 metrics`.
pub(crate) fn count(n: usize, noun: &str) -> String {
    if n == 1 {
        format!("1 {noun}")
    } else {
        format!("{n} {noun}s")
    }
}

/// Names the round in a message, after the first.
fn in_round(message: String, round: usize) -> String {
    if round == 1 {
        message
    } else {
        format!("{message} (round {round})")
    }
}
