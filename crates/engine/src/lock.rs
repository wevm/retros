//! `retros.lock`: the accepted value of every size, count, and check, the targets scripts set on
//! their lines, and the changes since.
//!
//! The lock is TOML. Each category is a table, named `zc: footprint` for a subject's scores, a
//! metric without items is a key, and a metric with items is a table of items sorted by name, so a
//! change shows as one line in a diff. A line that sets its own target records it after the value,
//! as metric lines write it, so a script that loosens its own target changes the lock like a
//! regression does. Timings and metrics that vary are left out: they differ between machines and
//! runs, and comparing them needs interleaved runs on one machine. Such an item appears only to
//! record its line's target. Updating the lock twice from the same code writes the same file.
//!
//! ```toml
//! [footprint]
//! bundle = "27500B"
//! modules = 412
//!
//! [footprint.export-size]
//! "viem" = "1000B"
//! "viem/chains" = "61000B target 80000B"
//!
//! [speed.import-time]
//! "viem" = "target 20000000ns"
//! ```

use std::borrow::Cow;
use std::fmt::Write;

use toml::Spanned;
use toml::de::{DeTable, DeValue};

use crate::config::{Better, Config, Metric};
use crate::error::{Code, Error, Location};
use crate::report::Report;
use crate::score::MetricScore;
use crate::value::{self, Kind, Value};

/// A parsed `retros.lock`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Lock {
    /// Categories with their recorded metrics, in the order of `retros.toml`. A category with a
    /// subject is named as reports label it, such as `zc: footprint`.
    pub categories: Vec<(String, Vec<(String, Entry)>)>,
}

/// One metric's recorded entry.
#[derive(Clone, Debug, PartialEq)]
pub enum Entry {
    /// Each item's record, sorted by name.
    Items(Vec<(String, Record)>),
    /// The record of a metric without items.
    Single(Record),
}

/// What the lock records for one measurement.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Record {
    /// The target the measurement's line sets, unless a pattern in `targets` overrides it.
    pub target: Option<Value>,
    /// The size, count, or check, or `None` for a timing.
    pub value: Option<Value>,
}

/// One difference between a run and the lock.
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
    /// The metric or item is in the lock but not in this run.
    Gone,
    /// The target is easier to meet than the recorded one, or was removed.
    Looser,
    /// The metric or item is in this run but not in the lock.
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

/// Parses `retros.lock` source text.
///
/// # Errors
///
/// Returns the first error: TOML syntax, a record that is not a size, count, or check with an
/// optional target, or a table where a record belongs.
pub fn parse(source: &str) -> Result<Lock, Error> {
    let document = DeTable::parse(source).map_err(|error| Error {
        code: Code::Toml,
        location: error.span().map(|span| Location::of(source, span.start)),
        message: error.message().to_owned(),
    })?;

    let mut lock = Lock::default();

    for (category, metrics) in document.get_ref() {
        let DeValue::Table(metrics) = metrics.get_ref() else {
            return Err(invalid(source, category.span().start, "a category table"));
        };

        let mut entries = Vec::new();
        for (metric, value) in metrics {
            let entry = match value.get_ref() {
                DeValue::Table(items) => {
                    let mut recorded = Vec::new();
                    for (item, value) in items {
                        recorded.push((item.get_ref().to_string(), record(source, value)?));
                    }
                    Entry::Items(recorded)
                }
                _ => Entry::Single(record(source, value)?),
            };
            entries.push((metric.get_ref().to_string(), entry));
        }

        lock.categories
            .push((category.get_ref().to_string(), entries));
    }

    Ok(lock)
}

/// Renders a lock as TOML.
pub fn render(lock: &Lock) -> String {
    let mut out = String::from(
        "# The accepted value of every size, count, and check, and the targets scripts set on their\n# lines. `retros --update` writes this file. Commit it, so each run reports what changed since.\n",
    );

    for (category, entries) in &lock.categories {
        let _ = write!(out, "\n[{}]\n", key(category));
        for (metric, entry) in entries {
            if let Entry::Single(record) = entry {
                let _ = writeln!(out, "{} = {}", key(metric), literal(*record));
            }
        }

        for (metric, entry) in entries {
            if let Entry::Items(items) = entry {
                let _ = write!(out, "\n[{}.{}]\n", key(category), key(metric));
                for (item, record) in items {
                    let _ = writeln!(out, "{} = {}", quoted(item), literal(*record));
                }
            }
        }
    }

    out
}

/// Returns the lock after a run: categories that ran record their sizes, counts, checks, and line
/// targets, and categories that did not run or failed keep their previous entries. Categories
/// follow the order of `retros.toml`, and categories it no longer declares are dropped.
pub fn update(previous: Option<&Lock>, config: &Config, report: &Report) -> Lock {
    let mut lock = Lock::default();

    for category in &config.categories {
        let ran: Vec<_> = report
            .categories
            .iter()
            .filter(|ran| ran.name == category.name)
            .filter_map(|ran| Some((ran.label(), ran.result.as_ref().ok()?)))
            .collect();

        // A category that did not run keeps every subject it recorded before.
        let entries: Vec<(String, Vec<(String, Entry)>)> = if ran.is_empty() {
            previous
                .into_iter()
                .flat_map(|lock| &lock.categories)
                .filter(|(label, _)| split(label).1 == category.name)
                .cloned()
                .collect()
        } else {
            ran.into_iter()
                .map(|(label, scored)| (label, scored.metrics.iter().filter_map(entry).collect()))
                .collect()
        };

        lock.categories.extend(
            entries
                .into_iter()
                .filter(|(_, entries)| !entries.is_empty()),
        );
    }

    lock
}

/// Returns every difference between the categories that ran and the lock, in report order.
pub fn compare(lock: &Lock, report: &Report) -> Vec<Change> {
    let mut changes = Vec::new();

    for category in &report.categories {
        let Ok(scored) = &category.result else {
            continue;
        };
        let recorded = entries(lock, &category.label()).unwrap_or_default();
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
            let before = recorded.iter().find(|(m, _)| m == name).map(|(_, e)| e);
            let after = entry(metric).map(|(_, entry)| entry);

            // A timing has no recorded value, so a missing record means no line target.
            if varies(metric) {
                let none = Entry::Items(Vec::new());
                let records = pair(before.unwrap_or(&none), after.as_ref().unwrap_or(&none));
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

            for (item, before, after) in pair(before, &after) {
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

        for (metric, _) in recorded {
            if !scored
                .metrics
                .iter()
                .any(|scored| scored.metric.name == *metric)
            {
                push(metric, None, None, None, Direction::Gone);
            }
        }
    }

    // A subject the lock records for a category that ran, but that this run did not write.
    for (label, entries) in &lock.categories {
        let (subject, name) = split(label);
        let ran = report
            .categories
            .iter()
            .any(|category| category.name == name && category.result.is_ok());
        let written = report
            .categories
            .iter()
            .any(|category| category.label() == *label);
        if !ran || written {
            continue;
        }
        for (metric, _) in entries {
            changes.push(Change {
                after: None,
                before: None,
                category: name.to_owned(),
                direction: Direction::Gone,
                item: None,
                metric: metric.clone(),
                subject: subject.map(str::to_owned),
            });
        }
    }

    changes
}

/// Splits a lock category name into its subject and category, such as `zc: footprint`.
fn split(label: &str) -> (Option<&str>, &str) {
    match label.split_once(": ") {
        Some((subject, category)) => (Some(subject), category),
        None => (None, label),
    }
}

/// Returns whether a metric's values differ between runs: a time, or a size or count that
/// `retros.toml` marks with `varies`. The lock records only its line targets.
fn varies(metric: &MetricScore) -> bool {
    metric.kind == Kind::Time || (metric.metric.varies && metric.kind != Kind::Check)
}

fn entries<'a>(lock: &'a Lock, category: &str) -> Option<&'a [(String, Entry)]> {
    lock.categories
        .iter()
        .find(|(name, _)| name == category)
        .map(|(_, entries)| entries.as_slice())
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
fn entry(metric: &MetricScore) -> Option<(String, Entry)> {
    let timing = varies(metric);

    let entry = if metric.items.is_empty() {
        let value = match metric.kind {
            Kind::Check => Some(Value::Check(metric.value >= 1.0)),
            _ if timing => None,
            kind => Some(Value::of(kind, metric.value)),
        };
        if value.is_none() && metric.line_target.is_none() {
            return None;
        }
        Entry::Single(Record {
            target: metric.line_target,
            value,
        })
    } else {
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
        Entry::Items(items)
    };

    Some((metric.metric.name.clone(), entry))
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
fn record(source: &str, value: &Spanned<DeValue<'_>>) -> Result<Record, Error> {
    const EXPECTED: &str = "a size, count, or check, optionally followed by `target <value>`";
    let at = value.span().start;

    let text = match value.get_ref() {
        DeValue::Integer(integer) if integer.radix() == 10 => {
            return integer
                .as_str()
                .replace('_', "")
                .parse()
                .map(|n| Record {
                    target: None,
                    value: Some(Value::Count(n)),
                })
                .map_err(|_| invalid(source, at, "a number"));
        }
        DeValue::Float(float) => {
            return float
                .as_str()
                .replace('_', "")
                .parse()
                .map(|n| Record {
                    target: None,
                    value: Some(Value::Count(n)),
                })
                .map_err(|_| invalid(source, at, "a number"));
        }
        DeValue::String(text) => text,
        _ => return Err(invalid(source, at, EXPECTED)),
    };

    let (recorded, target) = match text.strip_prefix("target ") {
        Some(target) => (None, Some(target)),
        None => match text.split_once(" target ") {
            Some((recorded, target)) => (Some(recorded), Some(target)),
            None => (Some(text.as_ref()), None),
        },
    };

    let value = recorded
        .map(|recorded| {
            value::parse(recorded)
                .ok()
                .filter(|value| value.kind() != Kind::Time)
        })
        .map(|value| value.ok_or_else(|| invalid(source, at, EXPECTED)))
        .transpose()?;
    let target = target
        .map(|target| {
            value::parse(target)
                .ok()
                .filter(|target| target.amount().is_some())
        })
        .map(|target| target.ok_or_else(|| invalid(source, at, EXPECTED)))
        .transpose()?;

    Ok(Record { target, value })
}

#[cold]
fn invalid(source: &str, offset: usize, expected: &str) -> Error {
    Error::at(
        Code::InvalidValue,
        source,
        offset,
        format!("expected {expected}. Run `retros --update` to rewrite retros.lock"),
    )
}

/// Formats a record as a TOML literal: a number for a count without a target, and otherwise a
/// string in base units, such as `"61000B target 80000B"`.
fn literal(record: Record) -> String {
    let unit = |value: Value| match value {
        Value::Check(true) => "pass".to_owned(),
        Value::Check(false) => "fail".to_owned(),
        Value::Count(n) => format!("{n}"),
        Value::Size(bytes) => format!("{bytes}B"),
        Value::Time(nanos) => format!("{nanos}ns"),
    };

    if let (Some(Value::Count(n)), None) = (record.value, record.target) {
        return format!("{n}");
    }

    let parts: Vec<String> = [
        record.value.map(unit),
        record
            .target
            .map(|target| format!("target {}", unit(target))),
    ]
    .into_iter()
    .flatten()
    .collect();
    format!("\"{}\"", parts.join(" "))
}

/// Formats a key, bare when TOML allows it.
fn key(name: &str) -> Cow<'_, str> {
    if name
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
    {
        Cow::Borrowed(name)
    } else {
        Cow::Owned(quoted(name))
    }
}

/// Formats a quoted TOML key or string.
fn quoted(text: &str) -> String {
    let mut out = String::from('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{:04X}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
