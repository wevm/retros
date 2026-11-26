//! `uphill check`: runs each category's scripts once and checks the metric lines they write.
//! Categories that share a `run` command share one run.
//!
//! Each category prints one line once all its scripts finish, followed by the end of standard error
//! for a failed script, or by the errors and warnings its metric lines produced. A summary line
//! ends the output. The command exits with status 0 when every category passes, even with
//! warnings, and with status 1 when any category fails or the rules file is out of date.

use std::fmt::Write as _;
use std::io::{self, Write as _};
use std::process::ExitCode;

use engine::config::Category;
use engine::report::Failure;

use crate::measure::{self, Ran, count};
use crate::project::Project;

/// Runs `uphill check` for `project`.
///
/// # Errors
///
/// Returns a message for the terminal when Uphill cannot run a script at all, such as when `sh`
/// is missing. A script that fails is reported in the output, not returned.
pub(crate) fn run(project: &Project) -> Result<ExitCode, String> {
    crate::project::install(&project.root, &project.installs)?;
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

    let units = measure::units(&project.config.categories);
    let all: Vec<&Category> = units.iter().collect();
    let groups = measure::groups(&all);
    // What each unit measured, in the order of `all`, with its group and position in the group.
    let mut ran: Vec<Option<Ran>> = all.iter().map(|_| None).collect();
    let mut done: Vec<bool> = project.config.categories.iter().map(|_| false).collect();

    for (number, group) in groups.iter().enumerate() {
        let results = measure::group(project, &project.root, group, false)?;
        for (index, (unit, result)) in group.iter().zip(results).enumerate() {
            if let Some(at) = all.iter().position(|other| std::ptr::eq(*other, *unit)) {
                ran[at] = Some(Ran {
                    base: None,
                    group: number,
                    index,
                    result,
                });
            }
        }
        let mut block = String::new();

        for (category, done) in project.config.categories.iter().zip(&mut done) {
            let parts: Vec<_> = all
                .iter()
                .zip(&ran)
                .filter(|(unit, _)| unit.name == category.name)
                .filter_map(|(_, ran)| ran.as_ref())
                .collect();
            if *done {
                continue;
            }
            // Categories print in order, so one that waits for a later script holds back the rest.
            if parts.len() < category.scripts.len() {
                break;
            }
            *done = true;

            let name = &category.name;
            let head: Result<Vec<_>, &Failure> = parts
                .iter()
                .map(|ran| {
                    ran.result
                        .as_ref()
                        .map(|measured| (groups[ran.group].as_slice(), ran.index, measured))
                })
                .collect();
            let result = match head {
                Err(failure) => Err(failure.clone()),
                Ok(head) => measure::merged(category, &head)?.map(|rounds| {
                    // A metric that two scripts write counts once.
                    let mut metrics: Vec<&str> = Vec::new();
                    for line in rounds.first().into_iter().flatten() {
                        if !metrics.contains(&line.metric) {
                            metrics.push(line.metric);
                        }
                    }
                    let warnings: Vec<String> = head
                        .iter()
                        .flat_map(|(_, _, measured)| measured.warnings.iter().cloned())
                        .collect();
                    (metrics.len(), warnings)
                }),
            };
            match result {
                Ok((metrics, measured_warnings)) => {
                    passed += 1;
                    warnings += measured_warnings.len();

                    let mut summary = count(metrics, "metric");
                    if !measured_warnings.is_empty() {
                        let _ = write!(summary, ", {}", count(measured_warnings.len(), "warning"));
                    }
                    let _ = writeln!(block, "✓ {name:width$}  {summary}");
                    indent(&mut block, &measured_warnings.join("\n"));
                }
                Err(failure) => {
                    failed += 1;

                    let _ = writeln!(block, "✗ {name:width$}  {}", failure.message);
                    indent(&mut block, &failure.details);
                }
            }
        }

        let mut stdout = io::stdout().lock();
        let _ = stdout.write_all(block.as_bytes());
        let _ = stdout.flush();
    }

    // An outdated rules file fails the check, so agents never read rules the metrics dropped.
    if crate::rules::stale(project) {
        failed += 1;
        let _ = writeln!(
            io::stdout(),
            "✗ {}{} is out of date. Run `uphill rules --write`",
            project.prefix,
            engine::rules::FILE
        );
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

/// Appends each line of `text`, indented under its category.
fn indent(block: &mut String, text: &str) {
    for line in text.lines() {
        let _ = writeln!(block, "    {line}");
    }
}
