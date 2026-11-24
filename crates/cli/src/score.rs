//! `uphill [category]`: runs each category's script, scores its metrics for each subject, prints
//! each category as it finishes, then the opportunities and the changes since the baselines.
//! Categories that share a `run` command share one run. It writes `.uphill/report.json`,
//! `.uphill/report.md`, and `.uphill/report.html` in the project root. With `--update`, it writes
//! this run's sizes, counts, checks, and line targets to each metric's `baseline` in the
//! configuration. With `--compare <ref>`, it also checks out `<ref>` in a temporary worktree,
//! runs timings in interleaved pairs there and here, and compares sizes, counts, and checks with
//! the revision's baselines, or with its own run when it has none.
//!
//! With `--compact`, for agent loops, it prints the scores on one line and each missed target
//! with its note instead of each category's metrics. With `--note`, it keeps the run as a Git note
//! on `HEAD`, and `report.html` shows the trend of up to 99 noted runs before this one.
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
use incur::Parser;

use crate::project::Project;
use crate::script::{self, Outcome};
use crate::worktree::{self, Worktree};
use crate::{history, measure};

/// How many past runs the HTML report's trends show, with this one, and how many noted runs the
/// dashboard's trends show.
pub(crate) const TRENDS: usize = 99;

/// What `uphill [category]` was asked to do. incur parses it from the command line as the root
/// command, and each field's documentation is its help text.
#[derive(Parser)]
#[incur(
    hint = "Runs each category's script, scores its metrics, lists the opportunities that would raise the scores most, and compares sizes, counts, checks, and line targets with each metric's baseline. Reads uphill.yaml or uphill.toml, at the root or in .agents, from this directory or the nearest parent. Writes .uphill/report.json, report.md, and report.html."
)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each bool is an independent command-line flag"
)]
pub(crate) struct Options {
    /// Run only this category
    #[incur(arg)]
    pub(crate) category: Option<String>,
    /// Print the scores, missed targets, and opportunities in a few lines
    pub(crate) compact: bool,
    /// Compare with another revision: timings in interleaved pairs, and sizes, counts, and checks with its baselines
    pub(crate) compare: Option<String>,
    /// Keep this run as a Git note on HEAD, for trends and `uphill log`
    pub(crate) note: bool,
    /// Exit with status 1 when anything differs from the baselines
    pub(crate) require_baseline: bool,
    /// Exit with status 1 when a metric misses its target or a script fails
    pub(crate) require_targets: bool,
    /// Write this run's sizes, counts, checks, and line targets as baselines
    pub(crate) update: bool,
}

/// Runs every category of `project`, or only the category `options` names.
///
/// # Errors
///
/// Returns a message for the terminal when the category is unknown, when Uphill cannot run a
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

    if options.note {
        history::clean(project)?;
    }

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
        history: Vec::new(),
    };
    // The other revision's own scores, for comparing exact values when it records no baselines.
    let mut base_report = Report {
        categories: Vec::new(),
        changes: None,
        comparison: None,
        history: Vec::new(),
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
            Some(worktree) => measure::pairs(
                project,
                &project.root,
                &worktree.root,
                &group,
                &group,
                baselines.is_none(),
            )?,
            None => (measure::group(project, &project.root, &group, true)?, None),
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
            if options.compact {
                report.categories.push(entry);
                continue;
            }
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
    let misses = engine::report::misses(&report);
    if options.compact {
        let mut out = engine::report::scores(&report);
        if misses.is_empty() {
            out.push_str("\nNo missed targets\n");
        } else {
            out.push_str(&engine::report::missed(&misses));
        }
        let _ = io::stdout().write_all(out.as_bytes());
    }
    let _ = io::stdout().write_all(engine::report::summary(&report).as_bytes());

    report.history = history::runs(&project.root, TRENDS, true);
    let dir = project.root.join(".uphill");
    let write = |file: &str, contents: String| {
        std::fs::write(dir.join(file), contents)
            .map_err(|error| format!("error: could not write .uphill/{file}: {error}"))
    };
    std::fs::create_dir_all(&dir)
        .map_err(|error| format!("error: could not create .uphill: {error}"))?;
    let json = engine::report::json(&report);
    write("report.json", json.clone())?;
    write("report.md", engine::report::markdown(&report))?;
    write("report.html", engine::html::html(&report))?;

    let mut written = format!(
        "{prefix}.uphill/report.json, {prefix}.uphill/report.md, {prefix}.uphill/report.html"
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
    if options.note {
        let commit = history::note(project, history::NOTES, &engine::history::note(&report))?;
        history::note(project, history::REPORTS, &json)?;
        let _ = write!(written, ", a note on {commit}");
    }
    let _ = writeln!(io::stdout(), "\nWrote {written}");

    let mut failed = report
        .categories
        .iter()
        .any(|category| category.result.is_err());

    // Compares values, not rounded scores, so a value just past its target never passes as 90.
    if options.require_targets && !misses.is_empty() {
        // A compact run already listed the misses.
        let missed = if options.compact {
            String::new()
        } else {
            engine::report::missed(&misses)
        };
        let count = measure::count(misses.len(), "target");
        let _ = write!(io::stdout(), "{missed}\n{count} missed\n");
        failed = true;
    }

    // Like a snapshot test, the baseline gate fails on any difference, so improvements are
    // recorded and become the new floor.
    if options.require_baseline && !options.update {
        let message = match &report.changes {
            None => Some(
                "--require-baseline needs a baseline. Run `uphill --update` to record one"
                    .to_owned(),
            ),
            Some(changes) if !changes.is_empty() => Some(format!(
                "{} since the baseline. Run `uphill --update` to accept {}",
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

    setup(project, worktree)?;
    Ok(config.filter(baseline::recorded))
}

/// Runs the configuration's `setup` in the checkout of another revision, if it has one.
///
/// # Errors
///
/// Returns a message for the terminal when `setup` cannot run or fails.
pub(crate) fn setup(project: &Project, worktree: &Worktree) -> Result<(), String> {
    let Some(setup) = &project.config.setup else {
        return Ok(());
    };
    let run = script::run(&setup.run, &worktree.root, setup.timeout)
        .map_err(|error| format!("error: could not run `{}`: {error}", setup.run))?;
    let message = match run.outcome {
        Outcome::Exited(0) => return Ok(()),
        Outcome::Exited(code) => format!("exited with status {code}"),
        Outcome::Signaled(signal) => format!("was stopped by signal {signal}"),
        Outcome::TimedOut => format!(
            "timed out after {}",
            engine::format::duration(setup.timeout)
        ),
    };
    Err(format!(
        "error: setup in {} {message}\n{}",
        worktree.label, run.stderr
    )
    .trim_end()
    .to_owned())
}

/// Returns the lines of one category of a group from each round's metric text.
///
/// # Errors
///
/// Returns a message for the terminal when a round's text no longer parses, which the checks
/// already ruled out.
pub(crate) fn rounds<'s>(
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
