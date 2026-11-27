//! Measures categories: runs their scripts, reads the metric lines, and checks them against each
//! category's configuration. Uphill measures one unit per script of a category (see
//! [`Category::units`]) and merges the units of a category before it scores it. Units with the
//! same `run` command share one run, and each line goes to the unit that configures its metric. A run whose first round writes a time, or a
//! metric that varies, runs again until it has run [`engine::score::ROUNDS`] times, or
//! [`engine::compare::PAIRS`] times beside another revision's checkout.

use std::path::Path;
use std::time::Duration;

use engine::config::Category;
use engine::lines::Line;
use engine::report::Failure;
use engine::value::Kind;

use crate::error::Error;
use crate::project::Project;
use crate::script::{self, Outcome};

/// The metric lines of each round of a category that passed its checks.
pub(crate) struct Measured {
    /// The number of distinct metrics the first round wrote for the category.
    pub(crate) metrics: usize,
    /// The text each round wrote to `$UPHILL_METRICS`, in the order the rounds ran. A shared run
    /// holds the lines of every category that shares it.
    pub(crate) sources: Vec<String>,
    /// Rendered warnings from the first round, such as metrics `uphill.yaml` does not configure.
    pub(crate) warnings: Vec<String>,
}

/// What one unit measured at `HEAD`, and in another revision when it ran there, with its group
/// and its position in the group.
pub(crate) struct Ran<'g> {
    pub(crate) base: Option<Result<Measured, Failure>>,
    pub(crate) group: &'g [&'g Category],
    pub(crate) index: usize,
    pub(crate) result: Result<Measured, Failure>,
}

impl Ran<'_> {
    /// Returns what the unit measured in one checkout, `measured`, with its group and position.
    pub(crate) fn part<'a>(&'a self, measured: &'a Measured) -> Part<'a> {
        Part {
            group: self.group,
            index: self.index,
            measured,
        }
    }
}

/// What one unit of a category measured in one checkout, with its group and its position in the
/// group.
pub(crate) struct Part<'a> {
    pub(crate) group: &'a [&'a Category],
    pub(crate) index: usize,
    pub(crate) measured: &'a Measured,
}

/// Returns the units of every category, in the order of the categories and their scripts.
pub(crate) fn units(categories: &[Category]) -> Vec<Category> {
    categories.iter().flat_map(Category::units).collect()
}

/// Returns the lines of `category` for each round, merged from what each of its units measured:
/// the unit's group, its position in the group, and its result. The outer result fails when two
/// units write the same item.
///
/// # Errors
///
/// Returns a message for the terminal when a round's text no longer parses, which the checks
/// already ruled out.
pub(crate) fn merged<'s>(
    category: &Category,
    parts: &[Part<'s>],
) -> Result<Result<Vec<Vec<Line<'s>>>, Failure>, Error> {
    let rounds = parts
        .iter()
        .map(|part| crate::score::rounds(part.group, part.index, &part.measured.sources))
        .collect::<Result<Vec<_>, Error>>()?;
    Ok(
        engine::check::merge(category, &rounds).map_err(|error| Failure {
            details: String::new(),
            message: error.message,
        }),
    )
}

/// Groups categories that share a `run` command, in the order of each group's first category.
pub(crate) fn groups<'c>(categories: &[&'c Category]) -> Vec<Vec<&'c Category>> {
    let mut groups: Vec<Vec<&'c Category>> = Vec::new();
    for &category in categories {
        match groups.iter_mut().find(|group| {
            group
                .first()
                .is_some_and(|first| first.run() == category.run())
        }) {
            Some(group) => group.push(category),
            None => groups.push(vec![category]),
        }
    }
    groups
}

/// Runs the script that the categories of `group` share once in the checkout at `root`, or
/// [`engine::score::ROUNDS`] times when `repeat` is set and the first run writes a time or a
/// metric that varies. Returns each category's result, in the order of `group`.
///
/// # Errors
///
/// Returns a message for the terminal when Uphill cannot run the script at all. A script that
/// fails fails every category in the group. Lines that fail one category's checks fail only that
/// category.
pub(crate) fn group(
    project: &Project,
    root: &Path,
    group: &[&Category],
    repeat: bool,
) -> Result<Vec<Result<Measured, Failure>>, Error> {
    let mut run = Run::new(project, root, group);
    run.round()?;
    if repeat && run.timed {
        while run.rounds < engine::score::ROUNDS {
            run.round()?;
        }
    }
    Ok(run.finish())
}

/// Runs the script that the categories of `group` share in the checkout at `head`, and the script
/// of the same categories in `base_group` in the checkout at `base`. When the first run at `head`
/// writes a time or a metric that varies, both run
/// [`engine::compare::PAIRS`] times in interleaved pairs, and the side that runs first alternates
/// from pair to pair. Otherwise `base` runs once, or not at all unless `exact` asks for its
/// sizes, counts, and checks. Returns each category's results at `head` and at `base`.
///
/// # Errors
///
/// Returns a message for the terminal when Uphill cannot run the script at all.
pub(crate) fn pairs(
    project: &Project,
    head: &Path,
    base: &Path,
    group: &[&Category],
    base_group: &[&Category],
    exact: bool,
) -> Result<Compared, Error> {
    let mut head = Run::new(project, head, group);
    head.round()?;
    if !head.timed && !exact {
        return Ok((head.finish(), None));
    }

    let mut base = Run::new(project, base, base_group);
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

/// Each category's results at `head`, and at `base` when it ran there.
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
    /// The text each round wrote to `$UPHILL_METRICS`.
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
                .map(|category| category.timeout())
                .max()
                .unwrap_or(Duration::ZERO),
        }
    }

    /// Runs the script once more and checks its lines, unless it already failed.
    fn round(&mut self) -> Result<(), Error> {
        self.rounds += 1;
        let (Some(first), false) = (self.group.first(), self.stopped) else {
            return Ok(());
        };
        let (group, project, round) = (self.group, self.project, self.rounds);

        let run = script::run(first.run(), self.root, self.timeout)
            .map_err(|error| format!("error: could not run `{}`: {error}", first.run()))?;
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
                engine::check::Source::Baselines => crate::diagnostic(
                    severity,
                    &finding.error,
                    &project.baselines_name,
                    project.baselines_source.as_deref().unwrap_or_default(),
                ),
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
            // A check that varies records no baseline, but it has no median to repeat for.
            self.timed = lines.iter().any(|line| match line.value.kind() {
                Kind::Check => false,
                Kind::Time => true,
                _ => varies(line.metric),
            });
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
