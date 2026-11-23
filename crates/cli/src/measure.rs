//! Measures categories: runs their script, reads the metric lines, and checks them against each
//! category's configuration. Categories with the same `run` command share one run, and each line
//! goes to the category that configures its metric. A run whose first round writes a time, or a
//! metric that varies, runs again until it has run [`engine::score::ROUNDS`] times, or
//! [`engine::compare::PAIRS`] times beside another revision's checkout.

use std::path::Path;
use std::time::Duration;

use engine::config::Category;
use engine::report::Failure;
use engine::value::Kind;

use crate::project::Project;
use crate::script::{self, Outcome};

/// The metric lines of each round of a category that passed its checks.
pub(crate) struct Measured {
    /// The number of distinct metrics the first round wrote for the category.
    pub(crate) metrics: usize,
    /// The text each round wrote to `$RETROS_METRICS`, in the order the rounds ran. A shared run
    /// holds the lines of every category that shares it.
    pub(crate) sources: Vec<String>,
    /// Rendered warnings from the first round, such as metrics `retros.yaml` does not configure.
    pub(crate) warnings: Vec<String>,
}

/// Groups categories that share a `run` command, in the order of each group's first category.
pub(crate) fn groups<'c>(categories: &[&'c Category]) -> Vec<Vec<&'c Category>> {
    let mut groups: Vec<Vec<&'c Category>> = Vec::new();
    for &category in categories {
        match groups
            .iter_mut()
            .find(|group| group.first().is_some_and(|first| first.run == category.run))
        {
            Some(group) => group.push(category),
            None => groups.push(vec![category]),
        }
    }
    groups
}

/// Runs the script that the categories of `group` share once, or [`engine::score::ROUNDS`] times
/// when `repeat` is set and the first run writes a time or a metric that varies. Returns each
/// category's result, in the order of `group`.
///
/// # Errors
///
/// Returns a message for the terminal when Retros cannot run the script at all. A script that
/// fails fails every category in the group. Lines that fail one category's checks fail only that
/// category.
pub(crate) fn group(
    project: &Project,
    group: &[&Category],
    repeat: bool,
) -> Result<Vec<Result<Measured, Failure>>, String> {
    let mut run = Run::new(project, &project.root, group);
    run.round()?;
    if repeat && run.timed {
        while run.rounds < engine::score::ROUNDS {
            run.round()?;
        }
    }
    Ok(run.finish())
}

/// Runs the script that the categories of `group` share in this checkout and in the checkout at
/// `base`. When the first run here writes a time or a metric that varies, both run
/// [`engine::compare::PAIRS`] times in interleaved pairs, and the side that runs first alternates
/// from pair to pair. Otherwise `base` runs once, or not at all unless `exact` asks for its
/// sizes, counts, and checks. Returns each category's results here and in `base`.
///
/// # Errors
///
/// Returns a message for the terminal when Retros cannot run the script at all.
pub(crate) fn pairs(
    project: &Project,
    base: &Path,
    group: &[&Category],
    exact: bool,
) -> Result<Compared, String> {
    let mut head = Run::new(project, &project.root, group);
    head.round()?;
    if !head.timed && !exact {
        return Ok((head.finish(), None));
    }

    let mut base = Run::new(project, base, group);
    base.round()?;
    if head.timed {
        for pair in 1..engine::compare::PAIRS {
            if pair % 2 == 1 {
                base.round()?;
                head.round()?;
            } else {
                head.round()?;
                base.round()?;
            }
        }
    }
    Ok((head.finish(), Some(base.finish())))
}

/// Each category's results in this checkout, and in the other revision when it ran there.
pub(crate) type Compared = (
    Vec<Result<Measured, Failure>>,
    Option<Vec<Result<Measured, Failure>>>,
);

/// The rounds of one group's script in one checkout, measured one round at a time.
struct Run<'a> {
    group: &'a [&'a Category],
    project: &'a Project,
    /// Each category's result so far, in the order of `group`.
    results: Vec<Result<Measured, Failure>>,
    root: &'a Path,
    /// How many rounds have run, or would have run after the script failed.
    rounds: usize,
    /// The text each round wrote to `$RETROS_METRICS`.
    sources: Vec<String>,
    /// Whether the script failed, which fails every category and ends the run.
    stopped: bool,
    /// Whether the first round wrote a time or a metric that varies.
    timed: bool,
    timeout: Duration,
}

impl<'a> Run<'a> {
    fn new(project: &'a Project, root: &'a Path, group: &'a [&'a Category]) -> Self {
        Run {
            group,
            project,
            results: group
                .iter()
                .map(|_| {
                    Ok(Measured {
                        metrics: 0,
                        sources: Vec::new(),
                        warnings: Vec::new(),
                    })
                })
                .collect(),
            root,
            rounds: 0,
            sources: Vec::new(),
            stopped: false,
            timed: false,
            timeout: group
                .iter()
                .map(|category| category.timeout)
                .max()
                .unwrap_or(Duration::ZERO),
        }
    }

    /// Runs the script once more and checks its lines, unless it already failed.
    fn round(&mut self) -> Result<(), String> {
        self.rounds += 1;
        let (Some(first), false) = (self.group.first(), self.stopped) else {
            return Ok(());
        };
        let (group, project, round) = (self.group, self.project, self.rounds);

        let run = script::run(&first.run, self.root, self.timeout)
            .map_err(|error| format!("error: could not run `{}`: {error}", first.run))?;
        let mut fail_all = |details: String, message: String| {
            self.results = group
                .iter()
                .map(|_| {
                    Err(Failure {
                        details: details.clone(),
                        message: in_round(message.clone(), round),
                    })
                })
                .collect();
            self.stopped = true;
        };

        let message = match run.outcome {
            Outcome::Exited(0) => None,
            Outcome::Exited(code) => Some(format!("script exited with status {code}")),
            Outcome::Signaled(signal) => Some(format!("script was stopped by signal {signal}")),
            Outcome::TimedOut => Some(format!(
                "script timed out after {}",
                engine::format::duration(self.timeout)
            )),
        };
        if let Some(message) = message {
            fail_all(run.stderr, message);
            return Ok(());
        }

        let Ok(text) = String::from_utf8(run.metrics) else {
            fail_all(String::new(), "metric lines are not valid UTF-8".to_owned());
            return Ok(());
        };
        let label = in_round(format!("{} metrics", first.name), round);

        let lines = match engine::lines::parse(&text) {
            Ok(lines) => lines,
            Err(error) => {
                fail_all(
                    crate::diagnostic("error", &error, &label, &text),
                    "invalid metric line".to_owned(),
                );
                return Ok(());
            }
        };

        let routed = engine::check::route(group, &lines);
        for ((category, lines), result) in group.iter().zip(&routed).zip(&mut self.results) {
            let Ok(measured) = result else {
                continue;
            };

            let report = engine::check::category(category, lines);
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

                *result = Err(Failure {
                    details: details.join("\n"),
                    message: in_round(summary.join(", "), round),
                });
                continue;
            }

            if round == 1 {
                measured.metrics = report.metrics;
                measured.warnings = report
                    .warnings
                    .iter()
                    .map(|finding| render("warning", finding))
                    .collect();
            }
        }

        if round == 1 {
            let varies = |name: &str| {
                group
                    .iter()
                    .flat_map(|category| &category.metrics)
                    .any(|metric| metric.name == name && metric.varies)
            };
            self.timed = lines
                .iter()
                .any(|line| line.value.kind() == Kind::Time || varies(line.metric));
        }

        drop(lines);
        self.sources.push(text);
        Ok(())
    }

    /// Returns each category's result with the text of every round.
    fn finish(mut self) -> Vec<Result<Measured, Failure>> {
        for measured in self.results.iter_mut().flatten() {
            measured.sources.clone_from(&self.sources);
        }
        self.results
    }
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
