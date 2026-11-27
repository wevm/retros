//! Scores: medians across rounds, list roll-ups, Lighthouse's log-normal curve, the weighted
//! mean of a category, and the opportunities that would raise it.
//!
//! Each measurement is scored against its own target: the one the baseline file sets for it, else
//! the first pattern that matches its item, else the target its line sets, else the default. A
//! value at its target scores 90, and a value at the metric's `poor`, twice the target by default,
//! scores 50. A list collapses to the item furthest past its own target at the configured rank, so
//! the p90 of a list is the item with at most a tenth of the list further past its target. Scores
//! follow Lighthouse's arithmetic: a metric's score is a whole number from 0 to 100, from
//! `Util.computeLogNormalScore`, and a category's score is the weighted mean of its metric scores
//! as `ReportScoring.arithmeticMean` computes it, rounded.

use crate::baseline;
use crate::config::{Better, Category, Metric, Rollup};
use crate::lines::Line;
use crate::value::{Kind, Value};

/// How many times a category's script runs when its first run writes a time or a metric that
/// varies. Those items take their median across the runs.
pub const ROUNDS: usize = 5;

/// The smallest gain, in category points, that counts as an opportunity. Smaller gains round to
/// zero.
const MIN_GAIN: f64 = 0.5;

/// The result of one category run.
#[derive(Clone, Debug, PartialEq)]
pub struct CategoryScore {
    /// Configured metrics in the order `uphill.yaml` declares them.
    pub metrics: Vec<MetricScore>,
    /// How many rounds the category's times and values that vary take their median of: the runs
    /// of its script, or 1 when it has neither.
    pub rounds: usize,
    /// The weighted mean of the metric scores, or `None` when no metric with weight has a score.
    pub score: Option<u8>,
}

/// One configured metric's result.
#[derive(Clone, Debug, PartialEq)]
pub struct MetricScore {
    /// The item the roll-up selected from a list, whose value and target the metric reports.
    /// `None` for a metric without items, a check, or a `sum`.
    pub at: Option<String>,
    /// Each item, worst first. Empty for a metric written without items.
    pub items: Vec<Item>,
    /// The kind of value the script wrote.
    pub kind: Kind,
    /// The target on the line of a metric written without items.
    pub line_target: Option<Value>,
    /// The metric's configuration.
    pub metric: Metric,
    /// The note the line sets, for a metric written without items.
    pub note: Option<String>,
    /// The own target of a metric written without items: from the baseline file, or else from
    /// its line.
    pub own_target: Option<Value>,
    /// The score from 0 to 100. A check has one unless every item skips, and other metrics have
    /// one when every measurement has a target.
    pub score: Option<u8>,
    /// The target the metric is scored against: the selected item's target, or the total of the
    /// targets for a `sum`.
    pub target: Option<Value>,
    /// The collapsed value in base units. For a check, the share of items that pass among those
    /// that do not skip, from 0 to 1, or 1 when every item skips.
    pub value: f64,
}

impl MetricScore {
    /// Returns the value of a metric written without items: `pass`, `fail`, `skip`, or its
    /// collapsed amount.
    pub fn single(&self) -> Value {
        match self.kind {
            Kind::Check if self.skipped() => Value::Skip,
            Kind::Check => Value::Check(self.value >= 1.0),
            kind => Value::of(kind, self.value),
        }
    }

    /// Returns whether the metric is a check where every item, or its one line, skips.
    pub fn skipped(&self) -> bool {
        self.kind == Kind::Check && self.score.is_none()
    }
}

/// One item of a list metric.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    /// The target on the item's line.
    pub line_target: Option<Value>,
    /// The item name, such as `viem/chains`.
    pub name: String,
    /// Why the item is what it is, from its line in the first round.
    pub note: Option<String>,
    /// The item's own target when it differs from the metric's default: from the baseline file,
    /// or else from the first pattern that matches it, or else from its line.
    pub target: Option<Value>,
    /// The item's value. A timing item holds its median across rounds.
    pub value: Value,
}

/// A change that would raise a category's score: one item, one metric, or every item of a list
/// meeting its target.
#[derive(Clone, Debug, PartialEq)]
pub struct Opportunity {
    /// How many items must meet their targets: 1 for one item or a metric without items.
    pub count: usize,
    /// The category points the change would add, before rounding.
    pub gain: f64,
    /// The item to change, or `None` for the whole metric.
    pub item: Option<String>,
    /// The metric name.
    pub metric: String,
    /// The value to reach. For a check, `pass`.
    pub target: Value,
    /// The current value.
    pub value: Value,
}

/// One measurement as scoring sees it: its value and the target it is scored against.
#[derive(Clone, Debug)]
struct Measure {
    line: Option<Value>,
    name: Option<String>,
    note: Option<String>,
    own: Option<Value>,
    target: Option<f64>,
    value: Value,
}

/// A metric's collapsed result.
struct Evaluated {
    at: Option<usize>,
    score: Option<u8>,
    target: Option<f64>,
    value: f64,
}

/// Scores one category for each subject its lines name, in the order the subjects first appear.
/// Lines without subjects score once, with `None` as their subject.
///
/// Each round must already pass [`crate::check::category`].
pub fn subjects(
    category: &Category,
    rounds: &[Vec<Line<'_>>],
) -> Vec<(Option<String>, CategoryScore)> {
    let mut subjects: Vec<Option<&str>> = Vec::new();
    for line in rounds.first().into_iter().flatten() {
        if !subjects.contains(&line.subject) {
            subjects.push(line.subject);
        }
    }
    if subjects.is_empty() {
        subjects.push(None);
    }

    subjects
        .into_iter()
        .map(|subject| {
            let rounds: Vec<Vec<Line<'_>>> = rounds
                .iter()
                .map(|round| {
                    round
                        .iter()
                        .filter(|line| line.subject == subject)
                        .cloned()
                        .collect()
                })
                .collect();
            (
                subject.map(str::to_owned),
                self::category(category, &rounds),
            )
        })
        .collect()
}

/// Scores one category from the metric lines of each round, in the order the rounds ran.
///
/// Each round must already pass [`crate::check::category`], and name one subject or none. Sizes,
/// counts, checks, and targets come from the first round, except metrics that vary. A metric that
/// `uphill.yaml` does not configure is left out.
pub fn category(category: &Category, rounds: &[Vec<Line<'_>>]) -> CategoryScore {
    let metrics: Vec<MetricScore> = category
        .metrics
        .iter()
        .filter_map(|metric| self::metric(metric, rounds))
        .collect();

    let repeated = metrics.iter().any(|metric| {
        metric.kind == Kind::Time || (metric.metric.varies && metric.kind != Kind::Check)
    });

    CategoryScore {
        score: mean(&metrics).map(whole),
        metrics,
        rounds: if repeated { rounds.len() } else { 1 },
    }
}

/// Returns the changes that would raise the category's score, largest gain first.
///
/// Each item that misses its target, and each metric without items that misses its target, is
/// one opportunity. When no single item of a list moves the score, the list as a whole is one
/// opportunity: every item that misses its target meeting it.
pub fn opportunities(scored: &CategoryScore) -> Vec<Opportunity> {
    let Some(base) = mean(&scored.metrics) else {
        return Vec::new();
    };
    let mut found = Vec::new();

    for (index, metric) in scored.metrics.iter().enumerate() {
        if metric.score.is_none() || metric.metric.weight <= 0.0 {
            continue;
        }

        let measures = measures(metric);
        let gain = |changed: &[Measure]| {
            let score = evaluate(&metric.metric, metric.kind, changed).score;
            let mut metrics = scored.metrics.clone();
            if let Some(changed) = metrics.get_mut(index) {
                changed.score = score;
            }
            mean(&metrics).unwrap_or(base) - base
        };

        let misses: Vec<usize> = (0..measures.len())
            .filter(|&at| {
                measures
                    .get(at)
                    .is_some_and(|m| misses(m, metric.metric.better))
            })
            .collect();
        let before = found.len();

        for &at in &misses {
            let mut changed = measures.clone();
            let Some(measure) = changed.get_mut(at) else {
                continue;
            };
            let current = measure.value;
            let target = met(measure, metric.kind);
            measure.value = target;

            let gain = gain(&changed);
            if gain >= MIN_GAIN {
                found.push(Opportunity {
                    count: 1,
                    gain,
                    item: measures.get(at).and_then(|m| m.name.clone()),
                    metric: metric.metric.name.clone(),
                    target,
                    value: current,
                });
            }
        }

        if found.len() == before && misses.len() > 1 {
            let mut changed = measures.clone();
            for &at in &misses {
                if let Some(measure) = changed.get_mut(at) {
                    measure.value = met(measure, metric.kind);
                }
            }

            let gain = gain(&changed);
            if gain >= MIN_GAIN {
                // A check list's value is the share of items that pass, but every item this
                // opportunity names fails.
                let value = match metric.kind {
                    Kind::Check => Value::Check(false),
                    kind => Value::of(kind, metric.value),
                };
                found.push(Opportunity {
                    count: misses.len(),
                    gain,
                    item: None,
                    metric: metric.metric.name.clone(),
                    target: metric.target.unwrap_or(Value::Check(true)),
                    value,
                });
            }
        }
    }

    found.sort_by(|a, b| b.gain.total_cmp(&a.gain));
    found
}

/// Scores a value where lower is better on Lighthouse's `getLogNormalScore`, with `target` as its
/// p10 control point and `poor` as its median. A value at `target` scores 0.9, a value at `poor`
/// scores 0.5, and each side of a control point is clamped to its band. `target` must be greater
/// than zero and less than `poor`.
pub fn curve(target: f64, poor: f64, value: f64) -> f64 {
    /// The closest double to `erfc⁻¹(1/5)`.
    const INVERSE_ERFC_ONE_FIFTH: f64 = 0.906_193_802_436_823_2;
    /// The largest scores below 0.9 and 0.5.
    const MAX_AVERAGE: f64 = 0.899_999_999_999_999_9;
    const MAX_POOR: f64 = 0.499_999_999_999_999_94;

    debug_assert!(target > 0.0);
    debug_assert!(target < poor);

    let median = poor;

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

/// Returns a score from 0 to 100 for a value on the curve, as Lighthouse's
/// `Util.computeLogNormalScore` does: scores above 0.9 gain up to half a point more, and the
/// result rounds down to a whole number.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the clamped value is between 0 and 100"
)]
fn points(score: f64) -> u8 {
    let boosted = if score > 0.9 {
        score + 0.05 * (score - 0.9)
    } else {
        score
    };
    (boosted * 100.0).floor().clamp(0.0, 100.0) as u8
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

/// Scores one metric from the metric lines of each round, or returns `None` when no line of the
/// first round measures it.
pub(crate) fn metric(metric: &Metric, rounds: &[Vec<Line<'_>>]) -> Option<MetricScore> {
    let lines: Vec<&Line<'_>> = rounds
        .first()?
        .iter()
        .filter(|line| line.metric == metric.name)
        .collect();
    let kind = lines.first()?.value.kind();
    let default = metric.target.and_then(Value::amount);

    let mut measures: Vec<Measure> = lines
        .iter()
        .map(|line| {
            let repeated = kind == Kind::Time || (metric.varies && kind != Kind::Check);
            let value = if repeated && rounds.len() > 1 {
                let mut samples: Vec<f64> = rounds
                    .iter()
                    .flatten()
                    .filter(|other| other.metric == metric.name && other.item == line.item)
                    .filter_map(|other| other.value.amount())
                    .collect();
                Value::of(kind, median(&mut samples))
            } else {
                line.value
            };

            // The baseline file's own target has the final say, then a pattern, then the line's.
            let own = baseline::target(metric, line.subject, line.item.as_deref())
                .or_else(|| {
                    line.item
                        .as_deref()
                        .and_then(|item| metric.pattern(item))
                        .map(|pattern| pattern.target)
                })
                .or(line.target);

            Measure {
                line: line.target,
                name: line.item.as_deref().map(str::to_owned),
                note: line.note.as_deref().map(str::to_owned),
                own,
                target: own.and_then(Value::amount).or(default),
                value,
            }
        })
        .collect();
    let unlisted = lines.first().filter(|line| line.item.is_none());
    let own_target =
        unlisted.and_then(|line| baseline::target(metric, line.subject, None).or(line.target));
    let note = unlisted.and_then(|line| line.note.as_deref().map(str::to_owned));

    let evaluated = evaluate(metric, kind, &measures);
    let at = evaluated
        .at
        .and_then(|at| measures.get(at))
        .and_then(|measure| measure.name.clone());

    // Worst first: failed checks, then passed checks, then skipped checks, or the values
    // furthest past their targets.
    measures.sort_by(|a, b| {
        if kind == Kind::Check {
            let rank = |measure: &Measure| match measure.value {
                Value::Check(passed) => u8::from(passed),
                _ => 2,
            };
            return rank(a).cmp(&rank(b));
        }

        let order = ratio(a).total_cmp(&ratio(b));
        match metric.better {
            Better::Higher => order,
            Better::Lower => order.reverse(),
        }
    });

    Some(MetricScore {
        at,
        items: measures
            .into_iter()
            .filter_map(|measure| {
                Some(Item {
                    line_target: measure.line,
                    name: measure.name?,
                    note: measure.note,
                    target: measure.own.filter(|own| Some(*own) != metric.target),
                    value: measure.value,
                })
            })
            .collect(),
        kind,
        line_target: unlisted.and_then(|line| line.target),
        metric: metric.clone(),
        own_target,
        note,
        score: evaluated.score,
        target: evaluated.target.map(|target| Value::of(kind, target)),
        value: evaluated.value,
    })
}

/// Collapses a metric's measurements and scores the result.
fn evaluate(metric: &Metric, kind: Kind, measures: &[Measure]) -> Evaluated {
    if kind == Kind::Check {
        let passed = measures
            .iter()
            .filter(|measure| measure.value == Value::Check(true))
            .count();
        let judged = measures
            .iter()
            .filter(|measure| measure.value != Value::Skip)
            .count();

        // A check where every item skips does not apply, so it has no score, like an audit
        // Lighthouse marks not applicable.
        return Evaluated {
            at: None,
            score: (judged > 0).then(|| whole(count(passed) / count(judged) * 100.0)),
            target: None,
            value: if judged > 0 {
                count(passed) / count(judged)
            } else {
                1.0
            },
        };
    }

    let targets: Option<Vec<f64>> = measures.iter().map(|measure| measure.target).collect();
    let amount = |measure: &Measure| measure.value.amount().unwrap_or_default();

    let (at, value, target) = if metric.score == Rollup::Sum {
        let value = measures.iter().map(amount).sum();
        (
            None,
            value,
            targets.as_ref().map(|targets| targets.iter().sum()),
        )
    } else {
        // Rank by the value relative to its target, so each item is judged by its own target.
        let mut order: Vec<usize> = (0..measures.len()).collect();
        order.sort_by(|&a, &b| {
            let ratio = |at: usize| measures.get(at).map_or(0.0, ratio);
            ratio(a).total_cmp(&ratio(b))
        });

        let n = order.len();
        let rank = (n * 9).div_ceil(10).max(1);
        let index = match metric.score {
            Rollup::Max => n.saturating_sub(1),
            Rollup::Median => n / 2,
            Rollup::Min => 0,
            Rollup::P10 => n.saturating_sub(rank),
            Rollup::P90 | Rollup::Sum => rank - 1,
        };

        let at = order.get(index).copied();
        let selected = at.and_then(|at| measures.get(at));
        let named = measures.iter().any(|measure| measure.name.is_some());

        (
            at.filter(|_| named),
            selected.map(amount).unwrap_or_default(),
            selected.and_then(|measure| measure.target),
        )
    };

    let target = target.filter(|_| targets.is_some());
    Evaluated {
        at,
        score: target.map(|target| points(scaled(metric, target, value))),
        target,
        value,
    }
}

/// Returns the measurements of a scored metric, so a changed copy can be scored again.
fn measures(metric: &MetricScore) -> Vec<Measure> {
    let default = metric.metric.target.and_then(Value::amount);

    if metric.items.is_empty() {
        let value = metric.single();
        return vec![Measure {
            line: metric.line_target,
            name: None,
            note: None,
            own: metric.own_target,
            target: metric.own_target.and_then(Value::amount).or(default),
            value,
        }];
    }

    metric
        .items
        .iter()
        .map(|item| Measure {
            line: item.line_target,
            name: Some(item.name.clone()),
            note: None,
            own: item.target,
            target: item.target.and_then(Value::amount).or(default),
            value: item.value,
        })
        .collect()
}

/// Returns whether a measurement misses its target: a failed check, or a value past its target.
fn misses(measure: &Measure, better: Better) -> bool {
    match (measure.value, measure.target) {
        (Value::Check(passed), _) => !passed,
        (Value::Skip, _) | (_, None) => false,
        (value, Some(target)) => {
            let amount = value.amount().unwrap_or_default();
            match better {
                Better::Higher => amount < target,
                Better::Lower => amount > target,
            }
        }
    }
}

/// Returns the value a measurement has when it meets its target.
fn met(measure: &Measure, kind: Kind) -> Value {
    match (kind, measure.target) {
        (Kind::Check, _) | (_, None) => Value::Check(true),
        (kind, Some(target)) => Value::of(kind, target),
    }
}

/// Returns a measurement's value relative to its target, or its value without a target.
fn ratio(measure: &Measure) -> f64 {
    measure.value.amount().unwrap_or_default() / measure.target.unwrap_or(1.0)
}

/// Returns the weighted mean of the metric scores in points, before rounding, or `None` when no
/// metric with weight has a score. Like Lighthouse's `ReportScoring.arithmeticMean`, it sums each
/// score as a fraction of 1 times its weight, in the order `uphill.yaml` declares the metrics, so
/// a mean that falls on half a point rounds the way Lighthouse's does.
fn mean(metrics: &[MetricScore]) -> Option<f64> {
    let mut total = 0.0;
    let mut weights = 0.0;

    for metric in metrics {
        if let Some(score) = metric.score
            && metric.metric.weight > 0.0
        {
            total += f64::from(score) / 100.0 * metric.metric.weight;
            weights += metric.metric.weight;
        }
    }

    (weights > 0.0).then(|| total / weights * 100.0)
}

/// Scores a value on the curve against its target and the metric's `poor` for that target. A
/// metric where higher is better scores the reciprocals, so half its target scores 50 by default.
fn scaled(metric: &Metric, target: f64, value: f64) -> f64 {
    let poor = metric.poor(target);
    match metric.better {
        Better::Lower => curve(target, poor, value),
        Better::Higher if value <= 0.0 => 0.0,
        Better::Higher => curve(1.0 / target, 1.0 / poor, 1.0 / value),
    }
}

/// Returns the middle value, or the mean of the two middle values of an even list.
pub(crate) fn median(values: &mut [f64]) -> f64 {
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
