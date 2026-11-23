//! Past runs, kept as Git notes on the commits they measured. `uphill --note` writes one, and the
//! HTML report and `uphill log` read them.
//!
//! A note holds one section for each category and subject: a header with the score the run
//! reported, such as `[footprint 93]` or `[zc: speed 85]`, then the value of each item, or of a
//! metric without items, as metric lines. A timing holds its median. A category that failed has a
//! header without a score and no lines. Past lines are scored again with today's configuration,
//! so a metric's roll-up follows the configuration and the note stays small.

use std::fmt::Write as _;

use crate::baseline;
use crate::config::Metric;
use crate::lines;
use crate::report::Report;
use crate::score::{self, MetricScore};

/// A past run, read from the note on the commit it measured.
#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    /// Each category and subject the run measured, in the order it reported them.
    pub categories: Vec<Ran>,
    /// The commit as reports name it, such as `3e0053b`.
    pub commit: String,
    /// The commit's date, such as `2026-11-23`.
    pub date: String,
    /// The commit's title, the first line of its message.
    pub title: String,
}

impl Run {
    /// Returns the category of the run with this name and subject, if the run measured it.
    pub fn category(&self, name: &str, subject: Option<&str>) -> Option<&Ran> {
        self.categories
            .iter()
            .find(|ran| ran.name == name && ran.subject.as_deref() == subject)
    }
}

/// One category of a past run, for one subject.
#[derive(Clone, Debug, PartialEq)]
pub struct Ran {
    /// The category name.
    pub name: String,
    /// The category score the run reported, or `None` when it had none or failed.
    pub score: Option<u8>,
    /// The metric lines, without a subject.
    pub source: String,
    /// The subject, or `None` when the category's lines name none.
    pub subject: Option<String>,
}

impl Ran {
    /// Scores one metric of the run again with its configuration today. Returns `None` when the
    /// run did not measure it.
    pub fn metric(&self, metric: &Metric) -> Option<MetricScore> {
        let lines = lines::parse(&self.source).ok()?;
        score::metric(metric, &[lines])
    }
}

/// Writes the note for a run: each category's score and the value of each of its items.
pub fn note(report: &Report) -> String {
    let mut out = String::new();
    for category in &report.categories {
        out.push('[');
        if let Some(subject) = &category.subject {
            let _ = write!(out, "{subject}: ");
        }
        out.push_str(&category.name);
        let Ok(scored) = &category.result else {
            out.push_str("]\n");
            continue;
        };
        if let Some(score) = scored.score {
            let _ = write!(out, " {score}");
        }
        out.push_str("]\n");

        for metric in &scored.metrics {
            let name = &metric.metric.name;
            if metric.items.is_empty() {
                let _ = writeln!(out, "{name} {}", baseline::exact(metric.single()));
            }
            for item in &metric.items {
                let quoted = item.name.replace('\\', "\\\\").replace('"', "\\\"");
                let _ = writeln!(out, "{name} \"{quoted}\" {}", baseline::exact(item.value));
            }
        }
    }
    out
}

/// Reads the categories of a note. Text before the first header is ignored.
pub fn parse(note: &str) -> Vec<Ran> {
    let mut categories: Vec<Ran> = Vec::new();
    for line in note.lines() {
        if let Some(header) = line
            .trim()
            .strip_prefix('[')
            .and_then(|rest| rest.strip_suffix(']'))
        {
            let (subject, rest) = match header.split_once(": ") {
                Some((subject, rest)) => (Some(subject.to_owned()), rest),
                None => (None, header),
            };
            let (name, score) = match rest.split_once(' ') {
                Some((name, score)) => (name, score.parse().ok()),
                None => (rest, None),
            };
            categories.push(Ran {
                name: name.to_owned(),
                score,
                source: String::new(),
                subject,
            });
        } else if let Some(ran) = categories.last_mut() {
            ran.source.push_str(line);
            ran.source.push('\n');
        }
    }
    categories
}
