//! Scores: medians across rounds, list roll-ups, Lighthouse's log-normal curve, and the weighted
//! mean of a category.
//!
//! A value at its target scores 90, a value twice the target scores 50, and a value four times
//! the target scores 10. Scores follow Lighthouse's rounding. A metric's score is a whole number from 0 to 100, and a
//! category's score is the weighted mean of its metric scores, rounded.

use crate::config::{Better, Category, Metric, Rollup};
use crate::lines::Line;
use crate::value::{Kind, Value};

/// The result of one category run.
#[derive(Clone, Debug, PartialEq)]
pub struct CategoryScore {
    /// Configured metrics in the order `retros.toml` declares them.
    pub metrics: Vec<MetricScore>,
    /// How many times the script ran. Timing metrics take the median of each item across rounds.
    pub rounds: usize,
    /// The weighted mean of the metric scores, or `None` when no metric with weight has a score.
    pub score: Option<u8>,
}

/// One configured metric's result.
#[derive(Clone, Debug, PartialEq)]
pub struct MetricScore {
    /// Each item and its value, worst first. Empty for a metric written without items.
    pub items: Vec<Item>,
    /// The kind of value the script wrote.
    pub kind: Kind,
    /// The metric's configuration.
    pub metric: Metric,
    /// The score from 0 to 100. A check always has one, and other metrics have one when they set
    /// a target.
    pub score: Option<u8>,
    /// The collapsed value in base units. For a check, the share of items that pass, from 0 to 1.
    pub value: f64,
}

/// One item of a list metric.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    /// The item name, such as `viem/chains`.
    pub name: String,
    /// The item's value. A timing item holds its median across rounds.
    pub value: Value,
}

/// Scores one category from the metric lines of each round, in the order the rounds ran.
///
/// Each round must already pass [`crate::check::category`]. Sizes, counts, and checks come from
/// the first round. A metric that `retros.toml` does not configure is left out.
pub fn category(category: &Category, rounds: &[Vec<Line<'_>>]) -> CategoryScore {
    let metrics: Vec<MetricScore> = category
        .metrics
        .iter()
        .filter_map(|metric| self::metric(metric, rounds))
        .collect();

    let mut total = 0.0;
    let mut weights = 0.0;
    for metric in &metrics {
        if let Some(score) = metric.score
            && metric.metric.weight > 0.0
        {
            total += f64::from(score) * metric.metric.weight;
            weights += metric.metric.weight;
        }
    }

    CategoryScore {
        metrics,
        rounds: rounds.len(),
        score: (weights > 0.0).then(|| whole(total / weights)),
    }
}

/// Scores a value where lower is better on Lighthouse's `getLogNormalScore`, with `target` as its
/// p10 control point and twice the target as its median. A value at `target` scores 0.9, a value
/// at twice the target scores 0.5, and each side of a control point is clamped to its band.
/// `target` must be greater than zero.
pub fn curve(target: f64, value: f64) -> f64 {
    /// The closest double to `erfc⁻¹(1/5)`.
    const INVERSE_ERFC_ONE_FIFTH: f64 = 0.906_193_802_436_823_2;
    /// The largest scores below 0.9 and 0.5.
    const MAX_AVERAGE: f64 = 0.899_999_999_999_999_9;
    const MAX_POOR: f64 = 0.499_999_999_999_999_94;

    debug_assert!(target > 0.0);

    let median = 2.0 * target;

    if value <= 0.0 {
        return 1.0;
    }

    // Lighthouse guards both logarithms with JavaScript's `Number.MIN_VALUE`.
    let smallest = f64::from_bits(1);
    let x_log_ratio = (value / median).max(smallest).ln();
    let target_log_ratio = -(target / median).max(smallest).ln();
    let standardized = x_log_ratio * INVERSE_ERFC_ONE_FIFTH / target_log_ratio;
    let complementary = (1.0 - erf(standardized)) / 2.0;

    if value <= target {
        complementary.clamp(0.9, 1.0)
    } else if value <= median {
        complementary.clamp(0.5, MAX_AVERAGE)
    } else {
        complementary.clamp(0.0, MAX_POOR)
    }
}

/// Lighthouse's `erf`: Abramowitz and Stegun, formula 7.1.26.
fn erf(x: f64) -> f64 {
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs();

    let t = 1.0 / (1.0 + 0.327_591_1 * x);
    let y = t
        * (0.254_829_592
            + t * (-0.284_496_736
                + t * (1.421_413_741 + t * (-1.453_152_027 + t * 1.061_405_429))));

    sign * (1.0 - y * (-x * x).exp())
}

fn metric(metric: &Metric, rounds: &[Vec<Line<'_>>]) -> Option<MetricScore> {
    let lines: Vec<&Line<'_>> = rounds
        .first()?
        .iter()
        .filter(|line| line.metric == metric.name)
        .collect();
    let kind = lines.first()?.value.kind();

    let mut items: Vec<(Option<&str>, Value)> = lines
        .iter()
        .map(|line| {
            let value = if kind == Kind::Time && rounds.len() > 1 {
                let mut samples: Vec<f64> = rounds
                    .iter()
                    .flatten()
                    .filter(|other| other.metric == metric.name && other.item == line.item)
                    .filter_map(|other| other.value.amount())
                    .collect();
                Value::Time(median(&mut samples))
            } else {
                line.value
            };
            (line.item.as_deref(), value)
        })
        .collect();

    let (value, score) = if kind == Kind::Check {
        let passed = items
            .iter()
            .filter(|(_, value)| *value == Value::Check(true))
            .count();
        let share = count(passed) / count(items.len());
        (share, Some(whole(share * 100.0)))
    } else {
        let mut amounts: Vec<f64> = items
            .iter()
            .filter_map(|(_, value)| value.amount())
            .collect();
        let value = collapse(&mut amounts, metric.score);
        let score = metric
            .target
            .and_then(Value::amount)
            .map(|target| whole(scaled(target, metric.better, value) * 100.0));
        (value, score)
    };

    // Worst first: failed checks, then the values furthest in the worse direction.
    items.sort_by(|(_, a), (_, b)| {
        if let (Value::Check(a), Value::Check(b)) = (a, b) {
            return a.cmp(b);
        }

        let order = a
            .amount()
            .unwrap_or_default()
            .total_cmp(&b.amount().unwrap_or_default());
        match metric.better {
            Better::Higher => order,
            Better::Lower => order.reverse(),
        }
    });

    Some(MetricScore {
        items: items
            .into_iter()
            .filter_map(|(name, value)| {
                name.map(|name| Item {
                    name: name.to_owned(),
                    value,
                })
            })
            .collect(),
        kind,
        metric: metric.clone(),
        score,
        value,
    })
}

/// Scores a value on the curve. A metric where higher is better scores the reciprocals, so half
/// its target scores 50.
fn scaled(target: f64, better: Better, value: f64) -> f64 {
    match better {
        Better::Lower => curve(target, value),
        Better::Higher if value <= 0.0 => 0.0,
        Better::Higher => curve(1.0 / target, 1.0 / value),
    }
}

/// Collapses a list to one value. Percentiles take the nearest rank in the worse direction, so
/// the p90 is a written value with at most a tenth of the list above it, and the p10 mirrors it.
fn collapse(values: &mut [f64], rollup: Rollup) -> f64 {
    values.sort_by(f64::total_cmp);
    let n = values.len();
    let rank = (n * 9).div_ceil(10).max(1);

    let index = match rollup {
        Rollup::Max => n.saturating_sub(1),
        Rollup::Median => return median(values),
        Rollup::Min => 0,
        Rollup::P10 => n - rank,
        Rollup::P90 => rank - 1,
        Rollup::Sum => return values.iter().sum(),
    };

    values.get(index).copied().unwrap_or_default()
}

/// Returns the middle value, or the mean of the two middle values of an even list.
fn median(values: &mut [f64]) -> f64 {
    values.sort_by(f64::total_cmp);
    let middle = values.len() / 2;

    match (values.get(middle.wrapping_sub(1)), values.get(middle)) {
        (Some(low), Some(high)) if values.len().is_multiple_of(2) => f64::midpoint(*low, *high),
        (_, Some(value)) => *value,
        _ => 0.0,
    }
}

/// Converts a count to a float. Lists never reach 2^53 items.
#[expect(clippy::cast_precision_loss, reason = "lists never reach 2^53 items")]
fn count(n: usize) -> f64 {
    n as f64
}

/// Rounds a score from 0 to 100 to a whole number.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the clamped value is between 0 and 100"
)]
fn whole(score: f64) -> u8 {
    score.round().clamp(0.0, 100.0) as u8
}
