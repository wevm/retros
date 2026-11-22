//! Checks the metric lines of one category run against the category's configuration.

use std::collections::BTreeSet;

use crate::config::Category;
use crate::error::{Code, Error};
use crate::lines::Line;

/// The text a finding points into.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Source {
    /// `retros.toml`.
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

/// Checks that a category's script wrote every configured metric, that each metric uses one kind
/// of unit and matches its target, and that no metric or item is written twice.
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
    } else if first.item.is_none() {
        if let Some(line) = group.get(1) {
            report.errors.push(error(
                Code::Duplicate,
                line,
                format!(
                    "metric `{name}` is written more than once. Give each line an item to write a list"
                ),
            ));
        }
    } else {
        let mut seen = BTreeSet::new();
        if let Some(line) = group
            .iter()
            .find(|line| !seen.insert(line.item.as_deref().unwrap_or_default()))
        {
            report.errors.push(error(
                Code::Duplicate,
                line,
                format!(
                    "item `{}` of metric `{name}` is written more than once",
                    line.item.as_deref().unwrap_or_default()
                ),
            ));
        }
    }

    match category.metrics.iter().find(|metric| metric.name == name) {
        None => report.warnings.push(error(
            Code::UnconfiguredMetric,
            first,
            format!(
                "metric `{name}` is written, but `[{}]` does not configure it",
                category.name
            ),
        )),
        Some(metric) => {
            if metric.target.is_none()
                && group.iter().any(|line| line.target.is_some())
                && let Some(line) = group.iter().find(|line| line.target.is_none())
            {
                report.errors.push(error(
                    Code::MixedTargets,
                    line,
                    format!(
                        "metric `{name}` sets a target on some lines but not this one. Set a target on every line, or a default `target` in retros.toml"
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
                        "metric `{name}` is written as {}, but its `target` in retros.toml is {}",
                        kind.plural(),
                        target.kind().singular()
                    ),
                ));
            }
        }
    }
}
