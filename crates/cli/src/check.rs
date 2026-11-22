//! `retros check`: runs each category's script once and checks the metric lines it writes.
//!
//! Each category prints one line as its script finishes, followed by the end of standard error
//! for a failed script, or by the errors and warnings its metric lines produced. A summary line
//! ends the output. The command exits with status 0 when every category passes, even with
//! warnings, and with status 1 when any category fails.

use std::fmt::Write as _;
use std::io::{self, Write as _};
use std::process::ExitCode;

use engine::check::{Report, Source};
use engine::config::Category;

use crate::project::Project;
use crate::script::{self, Outcome};

/// Runs `retros check` for `project`.
///
/// # Errors
///
/// Returns a message for the terminal when Retros cannot run a script at all, such as when `sh`
/// is missing. A script that fails is reported in the output, not returned.
pub(crate) fn run(project: &Project) -> Result<ExitCode, String> {
    let width = project
        .config
        .categories
        .iter()
        .map(|category| category.name.len())
        .max()
        .unwrap_or_default();
    let mut failed = 0;
    let mut passed = 0;
    let mut warnings = 0;

    for category in &project.config.categories {
        let run = script::run(&category.run, &project.root, category.timeout)
            .map_err(|error| format!("error: could not run `{}`: {error}", category.run))?;
        let (block, result) = check(project, category, &run, width);

        match &result {
            Some(report) if report.passed() => passed += 1,
            _ => failed += 1,
        }
        warnings += result.map_or(0, |report| report.warnings.len());

        let mut stdout = io::stdout().lock();
        let _ = stdout.write_all(block.as_bytes());
        let _ = stdout.flush();
    }

    let mut summary = format!("\n{passed} passed");
    if failed > 0 {
        let _ = write!(summary, ", {failed} failed");
    }
    if warnings > 0 {
        let _ = write!(summary, ", {}", count(warnings, "warning"));
    }
    summary.push('\n');
    let _ = io::stdout().write_all(summary.as_bytes());

    Ok(if failed > 0 {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}

/// Checks one run and renders its block of output. Returns the check report when the script
/// exited with status 0 and wrote valid lines.
fn check(
    project: &Project,
    category: &Category,
    run: &script::Run,
    width: usize,
) -> (String, Option<Report>) {
    let name = &category.name;
    let failure = |summary: String, details: &str| {
        let mut block = format!("✗ {name:width$}  {summary}\n");
        for line in details.lines() {
            let _ = writeln!(block, "    {line}");
        }
        (block, None)
    };

    match run.outcome {
        Outcome::Exited(0) => {}
        Outcome::Exited(code) => {
            return failure(format!("script exited with status {code}"), &run.stderr);
        }
        Outcome::Signaled(signal) => {
            return failure(
                format!("script was stopped by signal {signal}"),
                &run.stderr,
            );
        }
        Outcome::TimedOut => {
            return failure(
                format!(
                    "script timed out after {}",
                    engine::format::duration(category.timeout)
                ),
                &run.stderr,
            );
        }
    }

    let label = format!("{name} metrics");
    let Ok(metrics) = std::str::from_utf8(&run.metrics) else {
        return failure("metric lines are not valid UTF-8".to_owned(), "");
    };
    let lines = match engine::lines::parse(metrics) {
        Ok(lines) => lines,
        Err(error) => {
            return failure(
                "invalid metric line".to_owned(),
                &crate::diagnostic("error", &error, &label, metrics),
            );
        }
    };

    let report = engine::check::category(category, &lines);
    let mut summary = count(report.metrics, "metric");
    if !report.errors.is_empty() {
        let _ = write!(summary, ", {}", count(report.errors.len(), "error"));
    }
    if !report.warnings.is_empty() {
        let _ = write!(summary, ", {}", count(report.warnings.len(), "warning"));
    }

    let mark = if report.passed() { '✓' } else { '✗' };
    let mut block = format!("{mark} {name:width$}  {summary}\n");
    let findings = report
        .errors
        .iter()
        .map(|finding| ("error", finding))
        .chain(report.warnings.iter().map(|finding| ("warning", finding)));

    for (severity, finding) in findings {
        let (file, source) = match finding.source {
            Source::Config => (project.name.as_str(), project.source.as_str()),
            Source::Metrics => (label.as_str(), metrics),
        };
        for line in crate::diagnostic(severity, &finding.error, file, source).lines() {
            let _ = writeln!(block, "  {line}");
        }
    }

    (block, Some(report))
}

/// Formats a count with its noun, such as `1 metric` or `3 metrics`.
fn count(n: usize, noun: &str) -> String {
    if n == 1 {
        format!("1 {noun}")
    } else {
        format!("{n} {noun}s")
    }
}
