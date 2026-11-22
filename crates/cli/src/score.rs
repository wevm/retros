//! `retros [category]`: runs each category's script, scores its metrics, prints each category as
//! it finishes, and writes `.retros/report.json` and `.retros/report.md` beside `retros.toml`.
//!
//! The command exits with status 0 when every category ran and passed its checks, and with
//! status 1 when any category failed. Scores do not change the exit status.

use std::io::{self, Write as _};
use std::process::ExitCode;

use engine::config::Category;
use engine::report::{CategoryReport, Report};

use crate::measure;
use crate::project::Project;

/// Runs every category of `project`, or only the category named `only`.
///
/// # Errors
///
/// Returns a message for the terminal when `only` names no category, when Retros cannot run a
/// script at all, or when it cannot write the reports.
pub(crate) fn run(project: &Project, only: Option<&str>) -> Result<ExitCode, String> {
    let categories: Vec<&Category> = match only {
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

    let dir = project.root.join(".retros");
    let write = |file: &str, contents: String| {
        std::fs::write(dir.join(file), contents)
            .map_err(|error| format!("error: could not write .retros/{file}: {error}"))
    };
    std::fs::create_dir_all(&dir)
        .map_err(|error| format!("error: could not create .retros: {error}"))?;
    write("report.json", engine::report::json(&report))?;
    write("report.md", engine::report::markdown(&report))?;

    let prefix = project.name.trim_end_matches("retros.toml");
    let _ = writeln!(
        io::stdout(),
        "\nReports: {prefix}.retros/report.json, {prefix}.retros/report.md"
    );

    let failed = report
        .categories
        .iter()
        .any(|category| category.result.is_err());
    Ok(if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}
