//! Checks the metric lines of one category run against the category's configuration.

use std::collections::BTreeSet;

use crate::config::Category;
use crate::error::{Code, Error};
use crate::lines::Line;

/// The text a finding points into.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Source {
    /// `retros.yaml`.
    Config,
    /// The metric lines the category's script wrote.
    Metrics,
}

/// One problem that a check found.
#[derive(Clone, Debug, PartialEq)]
pub struct Finding {
    /// The problem.
    pub error: Error,
    /// The text that the error's location points into.
    pub source: Source,
}

/// The result of checking one category run.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Report {
    /// Problems that make the run unusable.
    pub errors: Vec<Finding>,
    /// The number of distinct metrics the script wrote.
    pub metrics: usize,
    /// Problems that leave the run usable, such as metrics the configuration does not declare.
    pub warnings: Vec<Finding>,
}

impl Report {
    /// Returns whether the run has no errors.
    pub fn passed(&self) -> bool {
        self.errors.is_empty()
    }
}

/// Splits the lines of one run among the categories that share its `run` command: each line goes
/// to the category that configures its metric. A line that none of them configures goes to the
/// first, which warns about it.
pub fn route<'a>(categories: &[&Category], lines: &[Line<'a>]) -> Vec<Vec<Line<'a>>> {
    let mut routed: Vec<Vec<Line<'a>>> = categories.iter().map(|_| Vec::new()).collect();

    for line in lines {
        let at = categories
            .iter()
            .position(|category| {
                category
                    .metrics
                    .iter()
                    .any(|metric| metric.name == line.metric)
            })
            .unwrap_or(0);
        if let Some(lines) = routed.get_mut(at) {
            lines.push(line.clone());
        }
    }

    routed
}

/// Checks that a category's script wrote every configured metric, that each metric uses one kind
/// of unit and matches its target, that no metric or item is written twice for one subject, and
/// that the lines either all name a subject or none do. A metric that only some subjects write is
/// fine: the others have no score for it.
pub fn category(category: &Category, lines: &[Line<'_>]) -> Report {
    let mut groups: Vec<(&str, Vec<&Line<'_>>)> = Vec::new();
    for line in lines {
        match groups.iter_mut().find(|(name, _)| *name == line.metric) {
            Some((_, group)) => group.push(line),
            None => groups.push((line.metric, vec![line])),
        }
    }

    let mut report = Report {
        metrics: groups.len(),
        ..Report::default()
    };

    if let Some(first) = lines.first()
        && let Some(line) = lines
            .iter()
            .find(|line| line.subject.is_some() != first.subject.is_some())
    {
        report.errors.push(Finding {
            error: Error {
                code: Code::MixedSubjects,
                location: Some(line.location),
                message: format!(
                    "`{}` mixes lines with and without subjects. Line {} {}",
                    category.name,
                    first.location.line,
                    if first.subject.is_some() {
                        "names a subject"
                    } else {
                        "names none"
                    }
                ),
            },
            source: Source::Metrics,
        });
    }

    for (name, group) in &groups {
        check_group(category, name, group, &mut report);
    }

    for metric in &category.metrics {
        if !groups.iter().any(|(name, _)| *name == metric.name) {
            report.errors.push(Finding {
                error: Error {
                    code: Code::MetricMissing,
                    location: Some(metric.location),
                    message: format!(
                        "metric `{}` is configured, but the script did not write it",
                        metric.name
                    ),
                },
                source: Source::Config,
            });
        }
    }

    report
}

fn check_group(category: &Category, name: &str, group: &[&Line<'_>], report: &mut Report) {
    let Some(first) = group.first() else {
        return;
    };
    let kind = first.value.kind();
    let error = |code, line: &Line<'_>, message| Finding {
        error: Error {
            code,
            location: Some(line.location),
            message,
        },
        source: Source::Metrics,
    };

    if let Some(line) = group.iter().find(|line| line.value.kind() != kind) {
        report.errors.push(error(
            Code::MixedUnits,
            line,
            format!(
                "metric `{name}` mixes {} and {}. Line {} writes {}",
                kind.plural(),
                line.value.kind().plural(),
                first.location.line,
                kind.plural()
            ),
        ));
    }

    if let Some(line) = group
        .iter()
        .find(|line| line.item.is_some() != first.item.is_some())
    {
        report.errors.push(error(
            Code::MixedItems,
            line,
            format!("metric `{name}` mixes lines with and without items"),
        ));
    } else {
        // Each subject writes a metric once, or each of its items once.
        let mut seen = BTreeSet::new();
        if let Some(line) = group.iter().find(|line| {
            !seen.insert((
                line.subject.unwrap_or_default(),
                line.item.as_deref().unwrap_or_default(),
            ))
        }) {
            let subject = line
                .subject
                .map(|subject| format!(" for subject `{subject}`"))
                .unwrap_or_default();
            let message = match line.item.as_deref() {
                None => format!(
                    "metric `{name}` is written more than once{subject}. Give each line an item to write a list"
                ),
                Some(item) => {
                    format!("item `{item}` of metric `{name}` is written more than once{subject}")
                }
            };
            report.errors.push(error(Code::Duplicate, line, message));
        }
    }

    match category.metrics.iter().find(|metric| metric.name == name) {
        None => report.warnings.push(error(
            Code::UnconfiguredMetric,
            first,
            format!(
                "metric `{name}` is written, but `{}` does not configure it",
                category.name
            ),
        )),
        Some(metric) => {
            // An item has its own target from a pattern in `targets` or from its line.
            let own = |line: &Line<'_>| {
                line.target.is_some()
                    || line
                        .item
                        .as_deref()
                        .is_some_and(|item| metric.pattern(item).is_some())
            };
            if metric.target.is_none()
                && group.iter().any(|line| own(line))
                && let Some(line) = group.iter().find(|line| !own(line))
            {
                report.errors.push(error(
                    Code::MixedTargets,
                    line,
                    format!(
                        "metric `{name}` sets a target on some items but not this one. Set a target on every line, or a default `target` in retros.yaml"
                    ),
                ));
            }

            if let Some(target) = metric.target
                && target.kind() != kind
            {
                report.errors.push(error(
                    Code::TargetUnit,
                    first,
                    format!(
                        "metric `{name}` is written as {}, but its `target` in retros.yaml is {}",
                        kind.plural(),
                        target.kind().singular()
                    ),
                ));
            }

            for pattern in &metric.targets {
                let finding = |code, message| Finding {
                    error: Error {
                        code,
                        location: Some(pattern.location),
                        message,
                    },
                    source: Source::Config,
                };

                if pattern.target.kind() != kind {
                    report.errors.push(finding(
                        Code::TargetUnit,
                        format!(
                            "metric `{name}` is written as {}, but its target for `{}` is {}",
                            kind.plural(),
                            pattern.pattern,
                            pattern.target.kind().singular()
                        ),
                    ));
                } else if !group.iter().any(|line| {
                    line.item
                        .as_deref()
                        .is_some_and(|item| pattern.matches(item))
                }) {
                    report.warnings.push(finding(
                        Code::UnmatchedPattern,
                        format!(
                            "pattern `{}` in the `targets` of metric `{name}` matches no item",
                            pattern.pattern
                        ),
                    ));
                }
            }
        }
    }
}
