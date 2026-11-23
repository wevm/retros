//! Baselines: each metric's `baseline` key records the sizes, counts, and checks that
//! `retros --update` accepted, and the targets scripts set on their lines. Each run reports what
//! changed since.
//!
//! A metric without items records one value. A metric with items records each item, sorted by
//! name. A category whose lines name subjects records each subject first, in the order the run
//! wrote them.
//!
//! A line that sets its own target records it after the value, as metric lines write it. A script
//! that loosens its own target then changes the baseline like a regression does. Timings and
//! metrics that vary record only their line targets, because their values differ between machines
//! and runs. Updating twice from the same code writes the same file.
//!
//! ```yaml
//! footprint:
//!   run: bun metrics/footprint.ts
//!   bundle: { target: 30kB, baseline: 27.5kB }
//!   modules: { baseline: 412 }
//!   export-size:
//!     target: 5kB
//!     baseline:
//!       viem: 1kB
//!       viem/chains: 61kB target 80kB
//!
//! speed:
//!   run: bun metrics/speed.ts
//!   import-time: { target: 30ms, baseline: { viem: target 20ms } }
//! ```

use crate::config::{self, Better, Config, Format, Metric};
use crate::document::{Kind as NodeKind, Node};
use crate::edit;
use crate::error::{Code, Error, Location};
use crate::format::{SIZES, TIMES};
use crate::report::{CategoryReport, Report};
use crate::score::MetricScore;
use crate::value::{self, Kind, Value};

/// A metric's `baseline`: one record, or records by item or by subject.
#[derive(Clone, Debug, PartialEq)]
pub enum Baseline {
    /// Records by item, sorted by name, or by subject, in the order the run wrote them.
    Map(Vec<(String, Baseline)>),
    /// The record of a metric without items.
    Record(Record),
}

/// What a baseline records for one measurement.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Record {
    /// The target the measurement's line sets, unless a pattern in `targets` overrides it.
    pub target: Option<Value>,
    /// The size, count, or check, or `None` for a timing.
    pub value: Option<Value>,
}

/// One difference between a run and the baselines.
#[derive(Clone, Debug, PartialEq)]
pub struct Change {
    /// The value in this run, or `None` for a metric or item that is gone. For a target change,
    /// the target in this run.
    pub after: Option<Value>,
    /// The recorded value, or `None` for a new metric or item. For a target change, the recorded
    /// target.
    pub before: Option<Value>,
    /// The category name.
    pub category: String,
    /// What changed: a value, a target, or whether the metric or item exists.
    pub direction: Direction,
    /// The item that changed, or `None` for the whole metric.
    pub item: Option<String>,
    /// The metric name.
    pub metric: String,
    /// The subject of the category, or `None` when its lines name no subject.
    pub subject: Option<String>,
}

/// The kind of a change.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction {
    /// The value moved in its better direction, or a check now passes.
    Better,
    /// The metric or item has a baseline but is not in this run.
    Gone,
    /// The target is easier to meet than the recorded one, or was removed.
    Looser,
    /// The metric or item is in this run but has no baseline.
    New,
    /// The target is harder to meet than the recorded one, or is new.
    Tighter,
    /// The value moved in its worse direction, or a check now fails.
    Worse,
}

impl Direction {
    /// Returns the word reports use for the change, such as `worse`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Better => "better",
            Self::Gone => "gone",
            Self::Looser => "looser",
            Self::New => "new",
            Self::Tighter => "tighter",
            Self::Worse => "worse",
        }
    }

    /// Returns whether the change is to a target rather than a value.
    pub fn is_target(self) -> bool {
        matches!(self, Self::Looser | Self::Tighter)
    }
}

/// A metric's records in one category or subject: one record, or each item's record.
enum Entry {
    /// Each item's record, sorted by name.
    Items(Vec<(String, Record)>),
    /// The record of a metric without items.
    Single(Record),
}

impl Entry {
    /// Returns the entry as the configuration writes it.
    fn baseline(self) -> Baseline {
        match self {
            Self::Items(items) => Baseline::Map(
                items
                    .into_iter()
                    .map(|(item, record)| (item, Baseline::Record(record)))
                    .collect(),
            ),
            Self::Single(record) => Baseline::Record(record),
        }
    }
}

/// Returns whether any metric in `config` has a baseline.
pub fn recorded(config: &Config) -> bool {
    config
        .categories
        .iter()
        .flat_map(|category| &category.metrics)
        .any(|metric| metric.baseline.is_some())
}

/// Returns every difference between the categories that ran and their baselines, in report order.
pub fn compare(config: &Config, report: &Report) -> Vec<Change> {
    let mut changes = Vec::new();

    for category in &report.categories {
        let Ok(scored) = &category.result else {
            continue;
        };
        let configured = config
            .categories
            .iter()
            .find(|configured| configured.name == category.name)
            .map_or(&[][..], |configured| configured.metrics.as_slice());
        let subject = category.subject.as_deref();
        let mut push = |metric: &str, item: Option<&str>, before, after, direction| {
            changes.push(Change {
                after,
                before,
                category: category.name.clone(),
                direction,
                item: item.map(str::to_owned),
                metric: metric.to_owned(),
                subject: category.subject.clone(),
            });
        };

        for metric in &scored.metrics {
            let name = metric.metric.name.as_str();
            let before = configured
                .iter()
                .find(|configured| configured.name == name)
                .and_then(|configured| recorded_entry(configured, subject));
            let after = entry(metric);

            // A timing has no recorded value, so a missing record means no line target.
            if varies(metric) {
                let none = Entry::Items(Vec::new());
                let records = pair(
                    before.as_ref().unwrap_or(&none),
                    after.as_ref().unwrap_or(&none),
                );
                for (item, before, after) in records {
                    let (before, after) = (before.unwrap_or_default(), after.unwrap_or_default());
                    if let Some((before, after, direction)) =
                        target_change(&metric.metric, item, before.target, after.target)
                    {
                        push(name, item, before, after, direction);
                    }
                }
                continue;
            }

            let (Some(before), Some(after)) = (before, after) else {
                push(name, None, None, None, Direction::New);
                continue;
            };
            if matches!(before, Entry::Single(_)) != matches!(after, Entry::Single(_)) {
                push(name, None, None, None, Direction::New);
                continue;
            }

            for (item, before, after) in pair(&before, &after) {
                match (before, after) {
                    (Some(before), Some(after)) => {
                        if let (Some(old), Some(new)) = (before.value, after.value)
                            && let Some(direction) = direction(old, new, metric.metric.better)
                        {
                            push(name, item, Some(old), Some(new), direction);
                        }
                        if let Some((old, new, direction)) =
                            target_change(&metric.metric, item, before.target, after.target)
                        {
                            push(name, item, old, new, direction);
                        }
                    }
                    (None, Some(after)) => push(name, item, None, after.value, Direction::New),
                    (Some(before), None) => push(name, item, before.value, None, Direction::Gone),
                    (None, None) => {}
                }
            }
        }

        for metric in configured {
            let written = scored
                .metrics
                .iter()
                .any(|scored| scored.metric.name == metric.name);
            if !written && recorded_entry(metric, subject).is_some() {
                push(&metric.name, None, None, None, Direction::Gone);
            }
        }
    }

    // A subject with a baseline in a category that ran, but that this run did not write.
    for configured in &config.categories {
        let ran: Vec<&CategoryReport> = report
            .categories
            .iter()
            .filter(|category| category.name == configured.name && category.result.is_ok())
            .collect();
        if !ran.iter().any(|category| category.subject.is_some()) {
            continue;
        }
        for metric in &configured.metrics {
            let Some(Baseline::Map(subjects)) = &metric.baseline else {
                continue;
            };
            for (subject, _) in subjects {
                if ran
                    .iter()
                    .any(|category| category.subject.as_ref() == Some(subject))
                {
                    continue;
                }
                changes.push(Change {
                    after: None,
                    before: None,
                    category: configured.name.clone(),
                    direction: Direction::Gone,
                    item: None,
                    metric: metric.name.clone(),
                    subject: Some(subject.clone()),
                });
            }
        }
    }

    changes
}

/// Returns the configuration source with this run's sizes, counts, checks, and line targets as
/// each metric's `baseline`. A category that did not run or failed keeps its baselines, and a
/// metric whose baseline is unchanged keeps its text, so updating twice writes the same file.
///
/// # Errors
///
/// Returns an error when a metric's baseline must change but its configuration has a form
/// Retros cannot edit, such as a TOML metric defined with dotted keys.
pub fn write(
    source: &str,
    format: Format,
    config: &Config,
    report: &Report,
) -> Result<String, Error> {
    let mut changes = Vec::new();

    for category in &config.categories {
        let ran: Vec<&CategoryReport> = report
            .categories
            .iter()
            .filter(|ran| ran.name == category.name)
            .collect();
        let Some(first) = ran.first() else {
            continue;
        };
        if ran.iter().any(|ran| ran.result.is_err()) {
            continue;
        }

        for metric in &category.metrics {
            let mut entries = ran.iter().filter_map(|ran| {
                let scored = ran.result.as_ref().ok()?;
                let scored = scored
                    .metrics
                    .iter()
                    .find(|m| m.metric.name == metric.name)?;
                Some((ran.subject.clone().unwrap_or_default(), entry(scored)?))
            });
            let baseline = if first.subject.is_some() {
                let subjects: Vec<(String, Baseline)> = entries
                    .map(|(subject, entry)| (subject, entry.baseline()))
                    .collect();
                (!subjects.is_empty()).then_some(Baseline::Map(subjects))
            } else {
                entries.next().map(|(_, entry)| entry.baseline())
            };

            if baseline != metric.baseline {
                changes.push((category.name.as_str(), metric.name.as_str(), baseline));
            }
        }
    }

    if changes.is_empty() {
        return Ok(source.to_owned());
    }
    let updated = edit::baselines(source, format, &changes);

    // The editor reads only part of the syntax, so the file it writes must parse to the same
    // configuration with the new baselines.
    let mut expected = config.clone();
    for category in &mut expected.categories {
        for metric in &mut category.metrics {
            let change = changes
                .iter()
                .find(|(c, m, _)| *c == category.name && *m == metric.name);
            if let Some((_, _, baseline)) = change {
                metric.baseline.clone_from(baseline);
            }
        }
    }
    let written = config::parse(&updated, format).ok().map(without_locations);
    if written.as_ref() == Some(&without_locations(expected)) {
        return Ok(updated);
    }

    // Blames the first metric whose baseline did not come through.
    let blamed = changes
        .iter()
        .find(|(category, metric, baseline)| {
            let written = written.as_ref().and_then(|w| find(w, category, metric));
            written.is_none_or(|written| written.baseline != *baseline)
        })
        .or(changes.first());
    let (category, metric) = blamed.map_or(("", ""), |(category, metric, _)| (*category, *metric));
    let location = find(config, category, metric).map(|metric| metric.location);
    Err(cannot(format, metric, location))
}

/// Returns a metric of `config` by category and metric name.
fn find<'c>(config: &'c Config, category: &str, metric: &str) -> Option<&'c Metric> {
    config
        .categories
        .iter()
        .find(|configured| configured.name == category)?
        .metrics
        .iter()
        .find(|configured| configured.name == metric)
}

/// Returns `config` with every location cleared, to compare it with the same configuration after
/// lines moved.
fn without_locations(mut config: Config) -> Config {
    let nowhere = Location { column: 0, line: 0 };
    for category in &mut config.categories {
        category.location = nowhere;
        for metric in &mut category.metrics {
            metric.location = nowhere;
            for pattern in &mut metric.targets {
                pattern.location = nowhere;
            }
        }
    }
    config
}

/// Parses a metric's `baseline`: a record, a mapping of items or subjects to records, or a
/// mapping of subjects to mappings of items.
pub(crate) fn parse(source: &str, node: &Node) -> Result<Baseline, Error> {
    fn parse_at(source: &str, node: &Node, depth: usize) -> Result<Baseline, Error> {
        match &node.kind {
            NodeKind::Mapping(pairs) if depth < 2 && !pairs.is_empty() => pairs
                .iter()
                .map(|(key, value)| {
                    let key = key.text().ok_or_else(|| invalid(source, key.offset))?;
                    Ok((key.to_owned(), parse_at(source, value, depth + 1)?))
                })
                .collect::<Result<_, Error>>()
                .map(Baseline::Map),
            NodeKind::Scalar { text, .. } if !node.is_null() => record(text)
                .map(Baseline::Record)
                .ok_or_else(|| invalid(source, node.offset)),
            _ => Err(invalid(source, node.offset)),
        }
    }

    parse_at(source, node, 0)
}

/// Formats a record as metric lines write it, such as `61kB target 80kB`, `412`, or
/// `target 20ms`. Returns whether it is a number, which TOML writes without quotes.
pub(crate) fn text(record: Record) -> (String, bool) {
    let parts: Vec<String> = [
        record.value.map(exact),
        record
            .target
            .map(|target| format!("target {}", exact(target))),
    ]
    .into_iter()
    .flatten()
    .collect();
    let number = matches!(
        record,
        Record {
            target: None,
            value: Some(Value::Count(_))
        }
    );
    (parts.join(" "), number)
}

/// Formats a value so it parses back to the same amount, in the largest unit that keeps the number
/// at 1 or more with at most one decimal place, such as `27.5kB`, `300µs`, or `2523728B`.
fn exact(value: Value) -> String {
    let (amount, units) = match value {
        Value::Check(true) => return "pass".to_owned(),
        Value::Check(false) => return "fail".to_owned(),
        Value::Count(n) => return format!("{n}"),
        Value::Size(n) => (n, &SIZES),
        Value::Time(n) => (n, &TIMES),
    };

    units
        .iter()
        .rev()
        .filter(|(_, factor)| amount >= *factor)
        .map(|(unit, factor)| format!("{}{unit}", (amount / factor * 10.0).round() / 10.0))
        .find(|text| value::parse(text).ok() == Some(value))
        .unwrap_or_else(|| format!("{amount}{}", units.first().map_or("", |(unit, _)| *unit)))
}

/// Returns whether a metric's values differ between runs: a time, or a size or count that the
/// configuration marks with `varies`. Its baseline records only line targets.
fn varies(metric: &MetricScore) -> bool {
    metric.kind == Kind::Time || (metric.metric.varies && metric.kind != Kind::Check)
}

/// Returns a metric's recorded entry for a category's subject, or `None` when it has none.
fn recorded_entry(metric: &Metric, subject: Option<&str>) -> Option<Entry> {
    let baseline = match (subject, metric.baseline.as_ref()?) {
        (None, baseline) => baseline,
        (Some(subject), Baseline::Map(subjects)) => {
            &subjects.iter().find(|(name, _)| name == subject)?.1
        }
        (Some(_), Baseline::Record(_)) => return None,
    };

    match baseline {
        Baseline::Record(record) => Some(Entry::Single(*record)),
        Baseline::Map(items) => items
            .iter()
            .map(|(item, recorded)| match recorded {
                Baseline::Record(record) => Some((item.clone(), *record)),
                Baseline::Map(_) => None,
            })
            .collect::<Option<_>>()
            .map(Entry::Items),
    }
}

/// Pairs the records of two entries by item: each item in either, with its record on each side.
/// Two entries without items pair as one record with no item.
fn pair<'a>(
    before: &'a Entry,
    after: &'a Entry,
) -> Vec<(Option<&'a str>, Option<Record>, Option<Record>)> {
    match (before, after) {
        (Entry::Single(before), Entry::Single(after)) => vec![(None, Some(*before), Some(*after))],
        (Entry::Items(before), Entry::Items(after)) => {
            let find = |items: &[(String, Record)], name: &str| {
                items.iter().find(|(item, _)| item == name).map(|(_, r)| *r)
            };
            let mut pairs: Vec<_> = after
                .iter()
                .map(|(item, record)| (Some(item.as_str()), find(before, item), Some(*record)))
                .collect();
            pairs.extend(
                before
                    .iter()
                    .filter(|(item, _)| find(after, item).is_none())
                    .map(|(item, record)| (Some(item.as_str()), Some(*record), None)),
            );
            pairs
        }
        (Entry::Single(before), Entry::Items(_)) => vec![(None, Some(*before), None)],
        (Entry::Items(_), Entry::Single(after)) => vec![(None, None, Some(*after))],
    }
}

/// Returns the entry a scored metric records, or `None` for a timing without line targets.
fn entry(metric: &MetricScore) -> Option<Entry> {
    let timing = varies(metric);

    if metric.items.is_empty() {
        let value = match metric.kind {
            Kind::Check => Some(Value::Check(metric.value >= 1.0)),
            _ if timing => None,
            kind => Some(Value::of(kind, metric.value)),
        };
        if value.is_none() && metric.line_target.is_none() {
            return None;
        }
        return Some(Entry::Single(Record {
            target: metric.line_target,
            value,
        }));
    }

    // An item's own target is its line's, unless a pattern in `targets` overrides it.
    let mut items: Vec<(String, Record)> = metric
        .items
        .iter()
        .map(|item| {
            let line = item
                .target
                .filter(|_| metric.metric.pattern(&item.name).is_none());
            let record = Record {
                target: line,
                value: (!timing).then_some(item.value),
            };
            (item.name.clone(), record)
        })
        .filter(|(_, record)| record.value.is_some() || record.target.is_some())
        .collect();
    if items.is_empty() {
        return None;
    }
    items.sort_by(|(a, _), (b, _)| a.cmp(b));
    Some(Entry::Items(items))
}

/// Returns the direction of a change between two values, or `None` when they are equal.
fn direction(before: Value, after: Value, better: Better) -> Option<Direction> {
    if let (Value::Check(before), Value::Check(after)) = (before, after) {
        return match (before, after) {
            (true, false) => Some(Direction::Worse),
            (false, true) => Some(Direction::Better),
            _ => None,
        };
    }

    let (before, after) = (before.amount()?, after.amount()?);
    if same(before, after) {
        return None;
    }

    let improved = match better {
        Better::Higher => after > before,
        Better::Lower => after < before,
    };
    Some(if improved {
        Direction::Better
    } else {
        Direction::Worse
    })
}

/// Returns the change between a recorded line target and this run's, as the targets the item is
/// scored against. Without a line target, an item falls back to the first matching pattern in
/// `targets`, then the metric's `target`, so removing a line target that equals its fallback
/// changes nothing.
fn target_change(
    metric: &Metric,
    item: Option<&str>,
    before: Option<Value>,
    after: Option<Value>,
) -> Option<(Option<Value>, Option<Value>, Direction)> {
    if before == after {
        return None;
    }

    let fallback = item
        .and_then(|item| metric.pattern(item))
        .map(|pattern| pattern.target)
        .or(metric.target);
    let (before, after) = (before.or(fallback), after.or(fallback));

    let direction = match (
        before.and_then(Value::amount),
        after.and_then(Value::amount),
    ) {
        (Some(old), Some(new)) if same(old, new) => return None,
        (Some(old), Some(new)) => {
            let easier = match metric.better {
                Better::Higher => new < old,
                Better::Lower => new > old,
            };
            if easier {
                Direction::Looser
            } else {
                Direction::Tighter
            }
        }
        (Some(_), None) => Direction::Looser,
        (None, Some(_)) => Direction::Tighter,
        (None, None) => return None,
    };

    Some((before, after, direction))
}

/// Returns whether two amounts are equal within rounding.
fn same(a: f64, b: f64) -> bool {
    (a - b).abs() <= f64::EPSILON * a.abs().max(b.abs())
}

/// Parses a record: a value, a value followed by `target <value>`, or `target <value>` alone for a
/// timing.
fn record(text: &str) -> Option<Record> {
    let (recorded, target) = match text.strip_prefix("target ") {
        Some(target) => (None, Some(target)),
        None => match text.split_once(" target ") {
            Some((recorded, target)) => (Some(recorded), Some(target)),
            None => (Some(text), None),
        },
    };

    let value = match recorded {
        Some(recorded) => Some(
            value::parse(recorded)
                .ok()
                .filter(|value| value.kind() != Kind::Time)?,
        ),
        None => None,
    };
    let target = match target {
        Some(target) => Some(
            value::parse(target)
                .ok()
                .filter(|target| target.amount().is_some())?,
        ),
        None => None,
    };

    Some(Record { target, value })
}

#[cold]
fn cannot(format: Format, metric: &str, location: Option<Location>) -> Error {
    let example = format.pick(
        format!("a mapping, such as `{metric}: {{ target: 5kB }}`"),
        format!("an inline table, such as `{metric} = {{ target = \"5kB\" }}`"),
    );
    Error {
        code: Code::InvalidType,
        location,
        message: format!(
            "Retros cannot write the baseline of metric `{metric}` in this form. Write the metric as {example}, then run `retros --update` again"
        ),
    }
}

#[cold]
fn invalid(source: &str, offset: usize) -> Error {
    Error::at(
        Code::InvalidValue,
        source,
        offset,
        "expected a size, count, or check, optionally followed by `target <value>`, or a mapping of items or subjects to them. Run `retros --update` to rewrite the baseline".to_owned(),
    )
}
