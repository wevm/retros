//! `retros.lock`: the accepted values of every size, count, and check, and the changes since.
//!
//! The lock is TOML. Each category is a table, a metric without items is a key, and a metric
//! with items is a table of items sorted by name, so a change shows as one line in a diff.
//! Timings are left out: they differ between machines, and comparing them needs interleaved runs
//! on one machine. Without timings, updating the lock twice from the same code writes the same
//! file.
//!
//! ```toml
//! [footprint]
//! bundle = "27500B"
//! modules = 412
//!
//! [footprint.export-size]
//! "viem" = "1000B"
//! "viem/chains" = "61000B"
//! ```

use std::borrow::Cow;
use std::fmt::Write;

use toml::Spanned;
use toml::de::{DeTable, DeValue};

use crate::config::{Better, Config};
use crate::error::{Code, Error, Location};
use crate::report::Report;
use crate::score::MetricScore;
use crate::value::{self, Kind, Value};

/// A parsed `retros.lock`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Lock {
    /// Categories with their recorded metrics, in the order of `retros.toml`.
    pub categories: Vec<(String, Vec<(String, Entry)>)>,
}

/// One metric's recorded value.
#[derive(Clone, Debug, PartialEq)]
pub enum Entry {
    /// Each item and its value, sorted by name.
    Items(Vec<(String, Value)>),
    /// The value of a metric without items.
    Value(Value),
}

/// One difference between a run and the lock.
#[derive(Clone, Debug, PartialEq)]
pub struct Change {
    /// The value in this run, or `None` for a metric or item that is gone.
    pub after: Option<Value>,
    /// The recorded value, or `None` for a new metric or item.
    pub before: Option<Value>,
    /// The category name.
    pub category: String,
    /// Whether the change is better, worse, new, or gone.
    pub direction: Direction,
    /// The item that changed, or `None` for the whole metric.
    pub item: Option<String>,
    /// The metric name.
    pub metric: String,
}

/// The kind of a change.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction {
    /// The value moved in its better direction, or a check now passes.
    Better,
    /// The metric or item is in the lock but not in this run.
    Gone,
    /// The metric or item is in this run but not in the lock.
    New,
    /// The value moved in its worse direction, or a check now fails.
    Worse,
}

impl Direction {
    /// Returns the word reports use for the change, such as `worse`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Better => "better",
            Self::Gone => "gone",
            Self::New => "new",
            Self::Worse => "worse",
        }
    }
}

/// Parses `retros.lock` source text.
///
/// # Errors
///
/// Returns the first error: TOML syntax, a value that is not a size, count, or check, or a
/// table where a value belongs.
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
                        recorded.push((item.get_ref().to_string(), self::value(source, value)?));
                    }
                    Entry::Items(recorded)
                }
                _ => Entry::Value(self::value(source, value)?),
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
        "# The accepted value of every size, count, and check. `retros --update` writes this file.\n# Commit it, so each run reports what changed since.\n",
    );

    for (category, entries) in &lock.categories {
        let _ = write!(out, "\n[{}]\n", key(category));
        for (metric, entry) in entries {
            if let Entry::Value(value) = entry {
                let _ = writeln!(out, "{} = {}", key(metric), literal(*value));
            }
        }

        for (metric, entry) in entries {
            if let Entry::Items(items) = entry {
                let _ = write!(out, "\n[{}.{}]\n", key(category), key(metric));
                for (item, value) in items {
                    let _ = writeln!(out, "{} = {}", quoted(item), literal(*value));
                }
            }
        }
    }

    out
}

/// Returns the lock after a run: categories that ran record their sizes, counts, and checks, and
/// categories that did not run or failed keep their previous entries. Categories follow the order
/// of `retros.toml`, and categories it no longer declares are dropped.
pub fn update(previous: Option<&Lock>, config: &Config, report: &Report) -> Lock {
    let mut lock = Lock::default();

    for category in &config.categories {
        let ran = report
            .categories
            .iter()
            .find(|ran| ran.name == category.name)
            .and_then(|ran| ran.result.as_ref().ok());

        let entries = match ran {
            Some(scored) => scored.metrics.iter().filter_map(entry).collect(),
            None => match previous.and_then(|lock| entries(lock, &category.name)) {
                Some(entries) => entries.to_vec(),
                None => continue,
            },
        };

        if !entries.is_empty() {
            lock.categories.push((category.name.clone(), entries));
        }
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
        let recorded = entries(lock, &category.name).unwrap_or_default();
        let change = |metric: &str, item: Option<&str>, before, after, direction| Change {
            after,
            before,
            category: category.name.clone(),
            direction,
            item: item.map(str::to_owned),
            metric: metric.to_owned(),
        };

        for metric in &scored.metrics {
            let Some(current) = entry(metric) else {
                continue;
            };
            let name = metric.metric.name.as_str();
            let better = metric.metric.better;

            match (
                recorded.iter().find(|(m, _)| m == name).map(|(_, e)| e),
                current.1,
            ) {
                (Some(Entry::Value(before)), Entry::Value(after)) => {
                    if let Some(direction) = direction(*before, after, better) {
                        changes.push(change(name, None, Some(*before), Some(after), direction));
                    }
                }
                (Some(Entry::Items(before)), Entry::Items(after)) => {
                    for (item, value) in &after {
                        match before.iter().find(|(name, _)| name == item) {
                            Some((_, old)) => {
                                if let Some(direction) = direction(*old, *value, better) {
                                    changes.push(change(
                                        name,
                                        Some(item),
                                        Some(*old),
                                        Some(*value),
                                        direction,
                                    ));
                                }
                            }
                            None => changes.push(change(
                                name,
                                Some(item),
                                None,
                                Some(*value),
                                Direction::New,
                            )),
                        }
                    }
                    for (item, value) in before {
                        if !after.iter().any(|(name, _)| name == item) {
                            changes.push(change(
                                name,
                                Some(item),
                                Some(*value),
                                None,
                                Direction::Gone,
                            ));
                        }
                    }
                }
                _ => changes.push(change(name, None, None, None, Direction::New)),
            }
        }

        for (metric, _) in recorded {
            let reported = scored
                .metrics
                .iter()
                .any(|scored| scored.metric.name == *metric && scored.kind != Kind::Time);
            if !reported {
                changes.push(change(metric, None, None, None, Direction::Gone));
            }
        }
    }

    changes
}

fn entries<'a>(lock: &'a Lock, category: &str) -> Option<&'a [(String, Entry)]> {
    lock.categories
        .iter()
        .find(|(name, _)| name == category)
        .map(|(_, entries)| entries.as_slice())
}

/// Returns the entry a scored metric records, or `None` for a timing.
fn entry(metric: &MetricScore) -> Option<(String, Entry)> {
    if metric.kind == Kind::Time {
        return None;
    }

    let entry = if metric.items.is_empty() {
        let value = match metric.kind {
            Kind::Check => Value::Check(metric.value >= 1.0),
            kind => Value::of(kind, metric.value),
        };
        Entry::Value(value)
    } else {
        let mut items: Vec<(String, Value)> = metric
            .items
            .iter()
            .map(|item| (item.name.clone(), item.value))
            .collect();
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
    if (after - before).abs() <= f64::EPSILON * before.abs().max(after.abs()) {
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

fn value(source: &str, value: &Spanned<DeValue<'_>>) -> Result<Value, Error> {
    let at = value.span().start;

    match value.get_ref() {
        DeValue::Integer(integer) if integer.radix() == 10 => integer
            .as_str()
            .replace('_', "")
            .parse()
            .map(Value::Count)
            .map_err(|_| invalid(source, at, "a number")),
        DeValue::Float(float) => float
            .as_str()
            .replace('_', "")
            .parse()
            .map(Value::Count)
            .map_err(|_| invalid(source, at, "a number")),
        DeValue::String(text) => value::parse(text)
            .ok()
            .filter(|value| value.kind() != Kind::Time)
            .ok_or_else(|| invalid(source, at, "a size, count, or check")),
        _ => Err(invalid(source, at, "a size, count, or check")),
    }
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

/// Formats a value as a TOML literal: a string for sizes and checks, a number for counts.
fn literal(value: Value) -> String {
    match value {
        Value::Check(true) => "\"pass\"".to_owned(),
        Value::Check(false) => "\"fail\"".to_owned(),
        Value::Count(n) => format!("{n}"),
        Value::Size(bytes) => format!("\"{bytes}B\""),
        Value::Time(nanos) => format!("\"{nanos}ns\""),
    }
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
