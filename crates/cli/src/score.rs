//! `uphill [category]`: runs each category's script, scores its metrics for each subject, prints
//! each category as it finishes, then the opportunities and the changes since the baselines.
//! Categories that share a `run` command share one run. It writes `.uphill/report.json`,
//! `.uphill/report.md`, `.uphill/report.html`, and `.uphill/comment.md` in the project root. With `--update`, it writes
//! this run's sizes, counts, checks, and line targets to the baseline file beside the
//! configuration, and moves an older configuration's `baseline` keys there. With `--compare <ref>`,
//! it also checks out `<ref>` in a temporary worktree, runs timings in interleaved pairs there and
//! here, and compares sizes, counts, and checks with the revision's baselines, or with its own run
//! when it has none.
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
use engine::compare::{Comparison, Score};
use engine::config::{self, Category, Config};
use engine::lines::Line;
use engine::report::{CategoryReport, Failure, Report};
use incur::Parser;

use crate::measure::Ran;
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
    hint = "Runs each category's script, scores its metrics, lists the opportunities that would raise the scores most, and compares sizes, counts, checks, and line targets with each metric's baseline. Reads uphill.yaml or uphill.toml, at the root, in .agents, or in .agents/uphill, from this directory or the nearest parent. Writes .uphill/report.json, report.md, report.html, and comment.md."
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
    /// Write this run's sizes, counts, checks, and line targets to the baseline file
    pub(crate) update: bool,
}

/// Runs every category of `project`, or only the category `options` names.
///
/// # Errors
///
/// Returns a message for the terminal when the category is unknown, when Uphill cannot run a
/// script at all, when it cannot check out or set up the revision to compare with, when it cannot
/// move an older configuration's baselines into the baseline file, or when it cannot write a file.
pub(crate) fn run(project: &Project, options: &Options) -> Result<ExitCode, String> {
    crate::project::install(&project.root, &project.installs)?;
    let prefix = &project.prefix;

    let categories: Vec<&Category> = match options.category.as_deref() {
        None => project.config.categories.iter().collect(),
        Some(name) => vec![project.category(name)?],
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

    let units = measure::units(&project.config.categories);
    let all: Vec<&Category> = units.iter().collect();
    let groups = measure::groups(&all);
    let chosen = |name: &str| categories.iter().any(|chosen| chosen.name == name);
    // What each unit measured, in the order of `all`. A category scores once all its units ran.
    let mut ran: Vec<Option<Ran>> = all.iter().map(|_| None).collect();
    let mut scored: Vec<bool> = categories.iter().map(|_| false).collect();

    for (number, group) in groups.iter().enumerate() {
        if !group.iter().any(|unit| chosen(&unit.name)) {
            continue;
        }

        let (results, base) = match &worktree {
            Some(worktree) => {
                measure::pairs(project, &project.root, &worktree.root, group, group, true)?
            }
            None => (measure::group(project, &project.root, group, true)?, None),
        };
        let mut base = base.map(Vec::into_iter);
        for (index, (unit, result)) in group.iter().zip(results).enumerate() {
            let base = base.as_mut().and_then(Iterator::next);
            if let Some(at) = all.iter().position(|other| std::ptr::eq(*other, *unit)) {
                ran[at] = Some(Ran {
                    base,
                    group: number,
                    index,
                    result,
                });
            }
        }

        // Each category's entries, one per subject, with the warnings its lines produced.
        let mut entries: Vec<(CategoryReport, Vec<String>)> = Vec::new();
        for (category, scored) in categories.iter().zip(&mut scored) {
            let parts: Vec<&Ran> = all
                .iter()
                .zip(&ran)
                .filter(|(unit, _)| unit.name == category.name)
                .filter_map(|(_, ran)| ran.as_ref())
                .collect();
            if *scored {
                continue;
            }
            // Categories print in order, so one that waits for a later script holds back the rest.
            if parts.len() < category.scripts.len() {
                break;
            }
            *scored = true;

            let entry = |subject, result| CategoryReport {
                description: category.description.clone(),
                name: category.name.clone(),
                result,
                subject,
                title: category.title(),
            };
            let unit = |ran: &Ran, measured| (groups[ran.group].as_slice(), ran.index, measured);

            let mut warnings = Vec::new();
            let base_failure = parts.iter().find_map(|ran| match &ran.base {
                Some(Err(failure)) => Some(failure),
                _ => None,
            });
            if let (Some(failure), Some(worktree)) = (base_failure, &worktree) {
                let mut warning = format!(
                    "warning: {} failed in {}: {}",
                    category.name, worktree.label, failure.message
                );
                if !failure.details.is_empty() {
                    let _ = write!(warning, "\n{}", failure.details);
                }
                warnings.push(warning);
            }

            let head: Result<Vec<_>, &Failure> = parts
                .iter()
                .map(|ran| ran.result.as_ref().map(|measured| unit(ran, measured)))
                .collect();
            let rounds = match head {
                Err(failure) => Err(failure.clone()),
                Ok(head) => measure::merged(category, &head)?,
            };
            // The revision's scores come from its own run, whether or not this run passed.
            let base: Option<Vec<_>> = parts
                .iter()
                .map(|ran| match &ran.base {
                    Some(Ok(measured)) => Some(unit(ran, measured)),
                    _ => None,
                })
                .collect();
            let base_rounds = match base {
                Some(base) => Some(measure::merged(category, &base)?),
                None => None,
            };
            match (&base_rounds, base_failure) {
                (Some(Ok(base_rounds)), _) => {
                    for (subject, scored) in engine::score::subjects(category, base_rounds) {
                        base_report.categories.push(entry(subject, Ok(scored)));
                    }
                }
                (Some(Err(failure)), _) | (None, Some(failure)) => {
                    base_report
                        .categories
                        .push(entry(None, Err(failure.clone())));
                }
                (None, None) => {}
            }

            match rounds {
                Err(failure) => entries.push((entry(None, Err(failure)), warnings)),
                Ok(rounds) => {
                    if let Some(Ok(base_rounds)) = &base_rounds {
                        timings.extend(engine::compare::timings(category, base_rounds, &rounds));
                    }

                    let mut warnings: Vec<String> = parts
                        .iter()
                        .filter_map(|ran| ran.result.as_ref().ok())
                        .flat_map(|measured| measured.warnings.iter().cloned())
                        .chain(warnings)
                        .collect();
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
                    .any(|ran| ran.name == change.category && ran.result.is_ok())
            });
            changes
        };
        let scores = base_report
            .categories
            .iter()
            .map(|ran| Score {
                category: ran.name.clone(),
                error: ran
                    .result
                    .as_ref()
                    .err()
                    .map(|failure| failure.message.clone()),
                score: ran.result.as_ref().ok().and_then(|scored| scored.score),
                subject: ran.subject.clone(),
            })
            .collect();
        report.comparison = Some(Comparison {
            changes,
            reference: worktree.label.clone(),
            scores,
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
    write("comment.md", engine::comment::markdown(&report))?;

    // `comment.md` is the short form of `report.md` for a pull request comment, so the line names
    // only the three reports.
    let mut written = format!(
        "{prefix}.uphill/report.json, {prefix}.uphill/report.md, {prefix}.uphill/report.html"
    );
    if options.update {
        // An older configuration's `target`, `targets`, and `baseline` keys move into the
        // baseline file.
        let name = &project.name;
        let stripped = baseline::strip(&project.source, project.format, &project.packs)
            .map_err(|error| crate::diagnostic("error", &error, name, &project.source))?;
        let updated = baseline::write(&project.config, &report, project.format);
        let sidecar = &project.baselines_name;
        // A run that records nothing, without a baseline file, has nothing to write or remove.
        if updated != project.baselines_source.as_deref().unwrap_or_default() {
            let result = if updated.is_empty() {
                std::fs::remove_file(&project.baselines)
            } else {
                std::fs::write(&project.baselines, &updated)
            };
            result.map_err(|error| format!("error: could not write {sidecar}: {error}"))?;
            if !updated.is_empty() {
                let _ = write!(written, ", {sidecar}");
            }
        }
        if stripped != project.source {
            std::fs::write(&project.path, stripped)
                .map_err(|error| format!("error: could not write {name}: {error}"))?;
            let _ = write!(written, ", {name}");
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

    let relative =
        |path: &std::path::Path| path.strip_prefix(&project.root).unwrap_or(path).to_owned();
    let baselines = std::fs::read_to_string(worktree.root.join(relative(&project.baselines))).ok();
    let config = std::fs::read_to_string(worktree.root.join(relative(&project.path)))
        .ok()
        .and_then(|source| {
            crate::project::load(
                &[&worktree.root, &project.root],
                &source,
                baselines.as_deref(),
                project.format,
            )
        });
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
        .flat_map(|category| &category.scripts)
        .flat_map(|script| script.run.split_whitespace())
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

/// Runs the configuration's `setup` in the checkout of another revision, if it has one, and
/// installs the dependencies of the packs the project approves.
///
/// # Errors
///
/// Returns a message for the terminal when `setup` cannot run or fails, or an install fails.
pub(crate) fn setup(project: &Project, worktree: &Worktree) -> Result<(), String> {
    crate::project::install(&worktree.root, &project.installs)?;
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
