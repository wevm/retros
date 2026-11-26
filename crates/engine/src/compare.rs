//! Comparisons with another revision, for `uphill --compare <ref>`: exact changes against the
//! revision's baselines, and timings measured in interleaved pairs of runs on one machine.
//!
//! Each pair runs both revisions back to back, and the order alternates between pairs, so a
//! machine that slows down or warms up affects both sides alike. A timing changes only when the
//! medians of the two sides differ by at least 5% and at least 7 of 8 pairs agree on the
//! direction. Anything else is within noise.

use crate::baseline::{Change, Direction};
use crate::config::{Better, Category};
use crate::lines::Line;
use crate::score;
use crate::value::{Kind, Value};

/// How many interleaved pairs of runs a comparison takes when a script writes a time or a metric
/// that varies.
pub const PAIRS: usize = 8;

/// The smallest difference between the medians that counts as a change, as a share of the
/// revision's median.
const MIN_CHANGE: f64 = 0.05;

/// How many pairs out of [`PAIRS`] must agree on the direction of a change.
const MIN_AGREE: usize = 7;

/// What changed since another revision.
#[derive(Clone, Debug, PartialEq)]
pub struct Comparison {
    /// Sizes, counts, checks, and line targets that differ from the revision's baselines, or from
    /// the revision's own run when it has no baselines.
    pub changes: Vec<Change>,
    /// The revision as reports name it, such as `main (3e0053b)`.
    pub reference: String,
    /// Each category's score in the revision, in the order the revision measured them. A category
    /// that the revision does not declare has none.
    pub scores: Vec<Score>,
    /// Each time and value that varies, in report order, whether or not it changed.
    pub timings: Vec<Timing>,
}

/// One category's score in the revision of a comparison.
#[derive(Clone, Debug, PartialEq)]
pub struct Score {
    /// The category name.
    pub category: String,
    /// Why the category's script produced no scores in the revision, or `None` when it ran.
    pub error: Option<String>,
    /// The category's score, or `None` when it failed or no metric with weight has a score.
    pub score: Option<u8>,
    /// The subject the score belongs to, or `None` when the lines name no subject.
    pub subject: Option<String>,
}

/// One time or value that varies, measured in both revisions.
#[derive(Clone, Debug, PartialEq)]
pub struct Timing {
    /// The median in this run.
    pub after: Value,
    /// How many pairs moved in the direction of the change between the medians.
    pub agree: usize,
    /// The median in the revision.
    pub before: Value,
    /// The category name.
    pub category: String,
    /// `Better` or `Worse`, or `None` when the change is within noise.
    pub direction: Option<Direction>,
    /// The item, or `None` for a metric without items.
    pub item: Option<String>,
    /// The metric name.
    pub metric: String,
    /// How many pairs measured the item in both revisions.
    pub pairs: usize,
    /// The subject, or `None` when the category's lines name no subject.
    pub subject: Option<String>,
}

impl Timing {
    /// Returns the change between the medians as a share of the revision's median, such as 0.25
    /// for 25% more.
    pub fn ratio(&self) -> f64 {
        let (before, after) = (
            self.before.amount().unwrap_or_default(),
            self.after.amount().unwrap_or_default(),
        );
        if before == 0.0 {
            return if after == 0.0 { 0.0 } else { f64::INFINITY };
        }
        (after - before) / before
    }
}

/// Compares each time and value that varies between the rounds of two revisions, paired by
/// position: `base[i]` and `head[i]` ran back to back. Each round holds the category's lines.
/// An item that only one revision wrote is left out.
pub fn timings(category: &Category, base: &[Vec<Line<'_>>], head: &[Vec<Line<'_>>]) -> Vec<Timing> {
    let mut found: Vec<Timing> = Vec::new();

    for line in head.first().into_iter().flatten() {
        let Some(metric) = category.metrics.iter().find(|m| m.name == line.metric) else {
            continue;
        };
        let kind = line.value.kind();
        if kind != Kind::Time && !(metric.varies && kind != Kind::Check) {
            continue;
        }
        let item = line.item.as_deref();
        if found.iter().any(|timing| {
            timing.subject.as_deref() == line.subject
                && timing.metric == line.metric
                && timing.item.as_deref() == item
        }) {
            continue;
        }

        let value = |round: &[Line<'_>]| {
            round
                .iter()
                .find(|other| {
                    other.subject == line.subject
                        && other.metric == line.metric
                        && other.item.as_deref() == item
                })
                .and_then(|other| other.value.amount())
        };
        let mut before: Vec<f64> = base.iter().filter_map(|round| value(round)).collect();
        let mut after: Vec<f64> = head.iter().filter_map(|round| value(round)).collect();
        let pairs: Vec<(f64, f64)> = base
            .iter()
            .zip(head)
            .filter_map(|(b, h)| Some((value(b)?, value(h)?)))
            .collect();
        if pairs.is_empty() {
            continue;
        }

        let (before, after) = (score::median(&mut before), score::median(&mut after));
        let rose = after > before;
        let agree = pairs
            .iter()
            .filter(|(b, h)| if rose { h > b } else { h < b })
            .count();

        let mut timing = Timing {
            after: Value::of(kind, after),
            agree,
            before: Value::of(kind, before),
            category: category.name.clone(),
            direction: None,
            item: item.map(str::to_owned),
            metric: line.metric.to_owned(),
            pairs: pairs.len(),
            subject: line.subject.map(str::to_owned),
        };
        // At least 7 of 8 pairs, or the same share of fewer.
        if timing.ratio().abs() >= MIN_CHANGE && agree * PAIRS >= MIN_AGREE * pairs.len() {
            let improved = match metric.better {
                Better::Higher => after > before,
                Better::Lower => after < before,
            };
            timing.direction = Some(if improved {
                Direction::Better
            } else {
                Direction::Worse
            });
        }
        found.push(timing);
    }

    found
}
