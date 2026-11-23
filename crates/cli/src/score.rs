//! `retros [category]`: runs each category's script, scores its metrics for each subject, prints
//! each category as it finishes, then the opportunities and the changes since the baselines.
//! Categories that share a `run` command share one run. It writes `.retros/report.json`,
//! `.retros/report.md`, and `.retros/report.html` in the project root. With `--update`, it writes
//! this run's sizes, counts, checks, and line targets to each metric's `baseline` in the
//! configuration. With `--compare <ref>`, it also checks out `<ref>` in a temporary worktree,
//! runs timings in interleaved pairs there and here, and compares sizes, counts, and checks with
//! the revision's baselines, or with its own run when it has none.
//!
//! The command exits with status 0 when every category ran and passed its checks, and with
//! status 1 when any category failed. With `--require-baseline`, it also exits with status 1 when
//! anything differs from the baselines, or when no metric has one. With `--require-targets`, it
//! exits with status 1 when a metric misses its target or a script fails. Scores do not change
//! the exit status.

use std::fmt::Write as _;
use std::io::{self, Write as _};
use std::process::ExitCode;

use engine::baseline;
use engine::compare::Comparison;
use engine::config::{self, Category, Config};
use engine::lines::Line;
use engine::report::{CategoryReport, Report};

use crate::measure;
use crate::project::Project;
use crate::script::{self, Outcome};
use crate::worktree::{self, Worktree};

/// What `retros [category]` was asked to do.
#[derive(Default)]
pub(crate) struct Options {
    /// The category to run, or `None` for every category.
    pub(crate) category: Option<String>,
    /// The revision to compare with, or `None` without `--compare`.
    pub(crate) compare: Option<String>,
    /// Whether any difference from the baselines fails the run.
    pub(crate) require_baseline: bool,
    /// Whether a missed target or a failed script fails the run.
    pub(crate) require_targets: bool,
    /// Whether to write this run's values to the baselines.
    pub(crate) update: bool,
}

/// Runs every category of `project`, or only the category `options` names.
///
/// # Errors
///
/// Returns a message for the terminal when the category is unknown, when Retros cannot run a
/// script at all, when it cannot check out or set up the revision to compare with, when it cannot
/// write a baseline into the configuration, or when it cannot write a file.
pub(crate) fn run(project: &Project, options: &Options) -> Result<ExitCode, String> {
    let prefix = &project.prefix;

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

    // Dropping the worktree removes it, so it goes away on every return from here.
    let worktree = options
        .compare
        .as_deref()
        .map(|reference| worktree::add(&project.root, reference))
        .transpose()?;
    let baselines = match &worktree {
        Some(worktree) => prepare(project, worktree, &categories)?,
        None => None,
    };

    let mut report = Report {
        categories: Vec::new(),
        changes: None,
        comparison: None,
    };
    // The other revision's own scores, for comparing exact values when it records no baselines.
    let mut base_report = Report {
        categories: Vec::new(),
        changes: None,
        comparison: None,
    };
    let mut timings = Vec::new();

    let all: Vec<&Category> = project.config.categories.iter().collect();
    for group in measure::groups(&all) {
        let chosen =
            |category: &Category| categories.iter().any(|chosen| chosen.name == category.name);
        if !group.iter().any(|category| chosen(category)) {
            continue;
        }

        // Each category's entries, one per subject, with the warnings its lines produced.
        let mut entries: Vec<(CategoryReport, Vec<String>)> = Vec::new();
        let (results, base) = match &worktree {
            Some(worktree) => measure::pairs(project, &worktree.root, &group, baselines.is_none())?,
            None => (measure::group(project, &group, true)?, None),
        };
        let mut base = base.map(Vec::into_iter);

        for (index, (category, result)) in group.iter().zip(results).enumerate() {
            let base = base.as_mut().and_then(Iterator::next);
            if !chosen(category) {
                continue;
            }
            let entry = |subject, result| CategoryReport {
                description: category.description.clone(),
                name: category.name.clone(),
                result,
                subject,
                title: category.title(),
            };

            let mut warnings = Vec::new();
            if let (Some(Err(failure)), Some(worktree)) = (&base, &worktree) {
                let mut warning = format!(
                    "warning: {} failed in {}: {}",
                    category.name, worktree.label, failure.message
                );
                if !failure.details.is_empty() {
                    let _ = write!(warning, "\n{}", failure.details);
                }
                warnings.push(warning);
            }

            match result {
                Err(failure) => entries.push((entry(None, Err(failure)), warnings)),
                Ok(measured) => {
                    let rounds = rounds(&group, index, &measured.sources)?;
                    if let Some(Ok(base)) = &base {
                        let base_rounds = self::rounds(&group, index, &base.sources)?;
                        timings.extend(engine::compare::timings(category, &base_rounds, &rounds));
                        for (subject, scored) in engine::score::subjects(category, &base_rounds) {
                            base_report.categories.push(entry(subject, Ok(scored)));
                        }
                    }

                    let mut warnings = [measured.warnings, warnings].concat();
                    for (subject, scored) in engine::score::subjects(category, &rounds) {
                        entries.push((entry(subject, Ok(scored)), std::mem::take(&mut warnings)));
                    }
                }
            }
        }

        // Subjects print together, each with its categories in order.
        let mut subjects: Vec<Option<String>> = Vec::new();
        for (entry, _) in &entries {
            if !subjects.contains(&entry.subject) {
                subjects.push(entry.subject.clone());
            }
        }
        entries.sort_by_key(|(entry, _)| subjects.iter().position(|s| *s == entry.subject));

        for (entry, warnings) in entries {
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
    }

    // A first `--update` lists every metric it records as new.
    report.changes = (baseline::recorded(&project.config) || options.update)
        .then(|| baseline::compare(&project.config, &report));
    if let Some(worktree) = &worktree {
        let changes = if let Some(baselines) = &baselines {
            baseline::compare(baselines, &report)
        } else {
            let accepted = baseline::accepted(&project.config, &base_report);
            let mut changes = baseline::compare(&accepted, &report);
            // A category that failed in the other revision has nothing to compare with.
            changes.retain(|change| {
                base_report
                    .categories
                    .iter()
                    .any(|ran| ran.name == change.category)
            });
            changes
        };
        report.comparison = Some(Comparison {
            changes,
            reference: worktree.label.clone(),
            timings,
        });
    }
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
    write("report.html", engine::html::html(&report))?;

    let mut written = format!(
        "{prefix}.retros/report.json, {prefix}.retros/report.md, {prefix}.retros/report.html"
    );
    if options.update {
        let name = &project.name;
        let updated = baseline::write(&project.source, project.format, &project.config, &report)
            .map_err(|error| crate::diagnostic("error", &error, name, &project.source))?;
        if updated != project.source {
            std::fs::write(&project.path, updated)
                .map_err(|error| format!("error: could not write {name}: {error}"))?;
            written.push_str(", ");
            written.push_str(name);
        }
    }
    let _ = writeln!(io::stdout(), "\nWrote {written}");

    let mut failed = report
        .categories
        .iter()
        .any(|category| category.result.is_err());

    // Compares values, not rounded scores, so a value just past its target never passes as 90.
    if options.require_targets {
        let misses = engine::report::misses(&report);
        if !misses.is_empty() {
            let missed = engine::report::missed(&misses);
            let count = measure::count(misses.len(), "target");
            let _ = write!(io::stdout(), "{missed}\n{count} missed\n");
            failed = true;
        }
    }

    // Like a snapshot test, the baseline gate fails on any difference, so improvements are
    // recorded and become the new floor.
    if options.require_baseline && !options.update {
        let message = match &report.changes {
            None => Some(
                "--require-baseline needs a baseline. Run `retros --update` to record one"
                    .to_owned(),
            ),
            Some(changes) if !changes.is_empty() => Some(format!(
                "{} since the baseline. Run `retros --update` to accept {}",
                measure::count(changes.len(), "change"),
                if changes.len() == 1 { "it" } else { "them" }
            )),
            Some(_) => None,
        };
        if let Some(message) = message {
            let _ = writeln!(io::stdout(), "\n{message}");
            return Ok(ExitCode::FAILURE);
        }
    }

    Ok(if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}

/// Prepares the checkout of the revision to compare with. It warns when the revision measures
/// differently, runs the configuration's `setup` there, and returns the revision's configuration
/// when it records baselines.
///
/// # Errors
///
/// Returns a message for the terminal when `setup` cannot run or fails.
fn prepare(
    project: &Project,
    worktree: &Worktree,
    categories: &[&Category],
) -> Result<Option<Config>, String> {
    let label = &worktree.label;
    let mut out = format!("Comparing with {label}\n");

    let place = project
        .path
        .strip_prefix(&project.root)
        .unwrap_or(&project.path);
    let config = std::fs::read_to_string(worktree.root.join(place))
        .ok()
        .and_then(|source| config::parse(&source, project.format).ok());
    if !config
        .as_ref()
        .is_some_and(|config| config::alike(config, &project.config))
    {
        let _ = writeln!(
            out,
            "warning: {} differs in {label}. Both revisions run the categories and commands of this checkout",
            project.name
        );
    }

    // Files that a `run` command names, such as `metrics/speed.ts` in `bun metrics/speed.ts`.
    let mut files: Vec<&str> = Vec::new();
    for word in categories
        .iter()
        .flat_map(|category| category.run.split_whitespace())
    {
        let word = word.trim_matches(['"', '\'']);
        if !files.contains(&word) && project.root.join(word).is_file() {
            files.push(word);
        }
    }
    for file in files {
        if std::fs::read(project.root.join(file)).ok()
            != std::fs::read(worktree.root.join(file)).ok()
        {
            let _ = writeln!(
                out,
                "warning: {}{file} differs in {label}, so its changes may come from the script",
                project.prefix
            );
        }
    }

    out.push('\n');
    let mut stdout = io::stdout().lock();
    let _ = stdout.write_all(out.as_bytes());
    let _ = stdout.flush();
    drop(stdout);

    if let Some(setup) = &project.config.setup {
        let run = script::run(&setup.run, &worktree.root, setup.timeout)
            .map_err(|error| format!("error: could not run `{}`: {error}", setup.run))?;
        let message = match run.outcome {
            Outcome::Exited(0) => None,
            Outcome::Exited(code) => Some(format!("exited with status {code}")),
            Outcome::Signaled(signal) => Some(format!("was stopped by signal {signal}")),
            Outcome::TimedOut => Some(format!(
                "timed out after {}",
                engine::format::duration(setup.timeout)
            )),
        };
        if let Some(message) = message {
            return Err(format!("error: setup in {label} {message}\n{}", run.stderr)
                .trim_end()
                .to_owned());
        }
    }

    Ok(config.filter(baseline::recorded))
}

/// Returns the lines of one category of a group from each round's metric text.
///
/// # Errors
///
/// Returns a message for the terminal when a round's text no longer parses, which the checks
/// already ruled out.
fn rounds<'s>(
    group: &[&Category],
    index: usize,
    sources: &'s [String],
) -> Result<Vec<Vec<Line<'s>>>, String> {
    sources
        .iter()
        .map(|source| {
            let lines = engine::lines::parse(source)?;
            Ok(engine::check::route(group, &lines).swap_remove(index))
        })
        .collect::<Result<Vec<_>, engine::error::Error>>()
        .map_err(|error| format!("error: {}", error.message))
}
