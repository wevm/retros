//! `uphill rules`: prints the rules of the project's metrics, or writes them to the rules file,
//! `.agents/uphill/rules.md`, and @-mentions that file from the project's AGENTS.md. `uphill add`,
//! `uphill update`, and `uphill remove` write the file too, and `uphill check` fails when it is out
//! of date.

use std::io::{self, Write as _};
use std::process::ExitCode;

use engine::config::Category;
use engine::rules;

use crate::project::Project;

/// Guidance files that can hold the mention, in the order agents read them. Without either,
/// Uphill writes AGENTS.md.
const GUIDANCE: [&str; 2] = ["AGENTS.md", "CLAUDE.md"];

/// Prints the rules of every category, or of `category`, or with `write`, writes the rules file.
///
/// # Errors
///
/// Returns a message for the terminal when the category is unknown, when `write` names a
/// category, or when a file cannot be read or written.
pub(crate) fn run(
    project: &Project,
    category: Option<&str>,
    write: bool,
) -> Result<ExitCode, String> {
    if write {
        if category.is_some() {
            return Err(
                "error: --write writes the rules of every category, so it takes no category"
                    .to_owned(),
            );
        }
        let wrote = self::write(project)?;
        let message = match wrote_message(project, &wrote) {
            message if message.is_empty() => {
                format!("{}{} is up to date", project.prefix, rules::FILE)
            }
            message => message,
        };
        let _ = writeln!(io::stdout(), "{message}");
        return Ok(ExitCode::SUCCESS);
    }

    let categories = match category {
        Some(name) => vec![project.category(name)?],
        None => project.config.categories.iter().collect(),
    };
    let _ = io::stdout().write_all(rules::markdown(&categories).as_bytes());
    Ok(ExitCode::SUCCESS)
}

/// Writes the rules file and adds the mention to the guidance file when either changes. Returns the
/// paths it wrote, relative to the project root.
///
/// # Errors
///
/// Returns a message for the terminal when a file cannot be read or written.
pub(crate) fn write(project: &Project) -> Result<Vec<&'static str>, String> {
    let mut wrote = Vec::new();

    let text = markdown(project);
    let path = project.root.join(rules::FILE);
    if std::fs::read_to_string(&path).ok().as_deref() != Some(text.as_str()) {
        path.parent()
            .map(std::fs::create_dir_all)
            .transpose()
            .and_then(|_| std::fs::write(&path, &text))
            .map_err(|error| format!("error: could not write {}: {error}", rules::FILE))?;
        wrote.push(rules::FILE);
    }

    let guidance = GUIDANCE
        .into_iter()
        .find(|name| project.root.join(name).is_file())
        .unwrap_or("AGENTS.md");
    let path = project.root.join(guidance);
    let current = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(format!("error: could not read {guidance}: {error}")),
    };
    if let Some(text) = rules::mention(&current) {
        std::fs::write(&path, text)
            .map_err(|error| format!("error: could not write {guidance}: {error}"))?;
        wrote.push(guidance);
    }

    Ok(wrote)
}

/// Returns whether the rules file exists and differs from the rules of the configuration.
pub(crate) fn stale(project: &Project) -> bool {
    std::fs::read_to_string(project.root.join(rules::FILE))
        .is_ok_and(|text| text != markdown(project))
}

/// Writes the rules file after a command changed the configuration in the current directory, and
/// prints what it wrote. A project that cannot load prints a warning, since the command itself
/// succeeded.
pub(crate) fn refresh() {
    let result = std::env::current_dir()
        .map_err(|error| format!("could not read the current directory: {error}"))
        .and_then(|cwd| crate::project::find(&cwd))
        .and_then(|project| write(&project).map(|wrote| wrote_message(&project, &wrote)));

    let _ = match result {
        Ok(message) if message.is_empty() => Ok(()),
        Ok(message) => writeln!(io::stdout(), "{message}"),
        Err(message) => writeln!(
            io::stderr(),
            "warning: {} is out of date: {}",
            rules::FILE,
            message.trim_start_matches("error: ")
        ),
    };
}

/// Returns the rules of every category of `project`.
fn markdown(project: &Project) -> String {
    let categories: Vec<&Category> = project.config.categories.iter().collect();
    rules::markdown(&categories)
}

/// Returns the line that names the files `write` wrote, relative to the current directory, or an
/// empty line when it wrote nothing.
fn wrote_message(project: &Project, wrote: &[&str]) -> String {
    if wrote.is_empty() {
        return String::new();
    }
    let names: Vec<String> = wrote
        .iter()
        .map(|name| format!("{}{name}", project.prefix))
        .collect();
    format!("Wrote {}", names.join(", "))
}
