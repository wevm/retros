//! `retros check`: runs each category's script once and checks the metric lines it writes.
//! Categories that share a `run` command share one run.
//!
//! Each category prints one line as its script finishes, followed by the end of standard error
//! for a failed script, or by the errors and warnings its metric lines produced. A summary line
//! ends the output. The command exits with status 0 when every category passes, even with
//! warnings, and with status 1 when any category fails.

use std::fmt::Write as _;
use std::io::{self, Write as _};
use std::process::ExitCode;

use crate::measure::{self, count};
use crate::project::Project;

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

    let categories: Vec<_> = project.config.categories.iter().collect();
    for group in measure::groups(&categories) {
        let results = measure::group(project, &project.root, &group, false)?;
        let mut block = String::new();

        for (category, result) in group.iter().zip(results) {
            let name = &category.name;
            match result {
                Ok(measured) => {
                    passed += 1;
                    warnings += measured.warnings.len();

                    let mut summary = count(measured.metrics, "metric");
                    if !measured.warnings.is_empty() {
                        let _ = write!(summary, ", {}", count(measured.warnings.len(), "warning"));
                    }
                    let _ = writeln!(block, "✓ {name:width$}  {summary}");
                    indent(&mut block, &measured.warnings.join("\n"));
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
