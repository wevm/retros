//! Baselines: the baseline file beside the configuration, `uphill.baseline.yaml` or
//! `uphill.baseline.toml`, records the sizes, counts, and checks that `uphill --update` accepted
//! for each metric, and the targets scripts set on their lines. Each run reports what changed
//! since. The configuration holds what people write, and the baseline file holds what
//! `uphill --update` writes, so Uphill rewrites the whole file instead of editing it in place.
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
//!   bundle: 27.5kB
//!   modules: 412
//!   export-size:
//!     viem: 1kB
//!     viem/chains: 61kB target 80kB
//!
//! speed:
//!   import-time:
//!     viem: target 20ms
//! ```
//!
//! Older configurations set each metric's `baseline` key instead. Uphill still reads it, so older
//! revisions compare and keep their history, and `uphill --update` moves it into the baseline file.

use std::borrow::Cow;
use std::fmt::Write;

use yaml_rust2::Yaml;

use crate::config::{self, Better, Config, Format, Metric, without_locations};
use crate::document::{self, Kind as NodeKind, Node};
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

/// Returns `config` with the baselines that the baseline file `source` records. An entry for a
/// category or metric that `config` does not declare is left out, and the next `uphill --update`
/// drops it. An entry replaces a `baseline` key that an older configuration sets.
///
/// # Errors
///
/// Returns an error for YAML or TOML syntax, a file that does not map categories to metrics, and
/// a baseline that does not parse.
pub fn read(mut config: Config, source: &str, format: Format) -> Result<Config, Error> {
    let Some(document) = document::parse(source, format)? else {
        return Ok(config);
    };
    let NodeKind::Mapping(categories) = &document.kind else {
        return Err(not_mapping(source, document.offset));
    };

    for (key, value) in categories {
        let NodeKind::Mapping(metrics) = &value.kind else {
            return Err(not_mapping(source, value.offset));
        };
        let mut category = config
            .categories
            .iter_mut()
            .find(|category| Some(category.name.as_str()) == key.text());
        for (key, value) in metrics {
            let baseline = parse(source, value)?;
            let metric = category.as_deref_mut().and_then(|category| {
                category
                    .metrics
                    .iter_mut()
                    .find(|metric| Some(metric.name.as_str()) == key.text())
            });
            if let Some(metric) = metric {
                metric.baseline = Some(baseline);
            }
        }
    }

    Ok(config)
}

/// Returns the baseline file with this run's sizes, counts, checks, and line targets for each
/// metric of `config`, or an empty string when no metric records any. A category that did not
/// run or failed keeps its baselines, so updating twice from the same code writes the same file.
///
/// Categories and metrics keep the order the configuration declares, and a baseline with items or
/// subjects takes one line per entry, so a change shows as one line in a diff.
pub fn write(config: &Config, report: &Report, format: Format) -> String {
    let config = apply(config, &updates(config, report));
    let categories: Vec<(&str, Vec<(String, Baseline)>)> = config
        .categories
        .iter()
        .map(|category| {
            let metrics = category
                .metrics
                .iter()
                .filter_map(|metric| Some((metric.name.clone(), metric.baseline.clone()?)))
                .collect::<Vec<_>>();
            (category.name.as_str(), metrics)
        })
        .filter(|(_, metrics)| !metrics.is_empty())
        .collect();
    if categories.is_empty() {
        return String::new();
    }

    let mut out = String::from(
        "# Written by `uphill --update`: the accepted sizes, counts, checks, and line targets of each\n# metric. Each run reports what changed since.\n",
    );
    for (category, metrics) in &categories {
        match format {
            Format::Toml => toml_table(&mut out, &[category], metrics),
            Format::Yaml => {
                let _ = write!(out, "\n{}:", yaml_key(category));
                yaml_block(&mut out, metrics, 2);
                out.push('\n');
            }
        }
    }
    out
}

/// Writes YAML entries, one per line at `indent` spaces: a record after its key, or a mapping on
/// the lines below it.
fn yaml_block(out: &mut String, entries: &[(String, Baseline)], indent: usize) {
    for (key, baseline) in entries {
        let _ = write!(out, "\n{:indent$}{}:", "", yaml_key(key));
        match baseline {
            Baseline::Map(entries) => yaml_block(out, entries, indent + 2),
            Baseline::Record(record) => {
                let _ = write!(out, " {}", text(*record).0);
            }
        }
    }
}

/// Writes a TOML table at `path` with the records of `entries`, then a table of its own for each
/// mapping, such as `[footprint.export-size]`. A table without records gets no header.
fn toml_table(out: &mut String, path: &[&str], entries: &[(String, Baseline)]) {
    let header: Vec<String> = path.iter().map(|key| toml_key(key).into_owned()).collect();
    let mut records = entries.iter().filter_map(|(key, baseline)| match baseline {
        Baseline::Record(record) => Some((key, record)),
        Baseline::Map(_) => None,
    });
    if let Some(first) = records.next() {
        let _ = write!(out, "\n[{}]\n", header.join("."));
        for (key, record) in std::iter::once(first).chain(records) {
            let value = match text(*record) {
                (text, true) => text,
                (text, false) => quoted(&text),
            };
            let _ = writeln!(out, "{} = {value}", toml_key(key));
        }
    }

    for (key, baseline) in entries {
        if let Baseline::Map(entries) = baseline {
            let path: Vec<&str> = path.iter().copied().chain([key.as_str()]).collect();
            toml_table(out, &path, entries);
        }
    }
}

/// Formats a YAML key, unquoted when it is a plain name that YAML reads as a string.
pub(crate) fn yaml_key(name: &str) -> Cow<'_, str> {
    let plain = name
        .bytes()
        .next()
        .is_some_and(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        && name.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.' | b'/' | b'@' | b'_')
        })
        && matches!(Yaml::from_str(name), Yaml::String(_));
    if plain {
        Cow::Borrowed(name)
    } else {
        Cow::Owned(quoted(name))
    }
}

/// Formats a TOML key, bare when TOML allows it.
fn toml_key(name: &str) -> Cow<'_, str> {
    if !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        Cow::Borrowed(name)
    } else {
        Cow::Owned(quoted(name))
    }
}

/// Formats a double-quoted string, which YAML and TOML escape the same way.
pub(crate) fn quoted(text: &str) -> String {
    let mut out = String::from('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{:04X}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Returns the configuration `source` without the `baseline` keys that older configurations set,
/// or `source` itself when it sets none.
///
/// # Errors
///
/// Returns an error when `source` does not parse, or when a metric's `baseline` has a form that
/// Uphill cannot remove, such as a TOML table of its own.
pub fn strip(source: &str, format: Format) -> Result<String, Error> {
    let config = config::parse(source, format)?;
    let inline: Vec<(&str, &str)> = config
        .categories
        .iter()
        .flat_map(|category| {
            category
                .metrics
                .iter()
                .filter(|metric| metric.baseline.is_some())
                .map(|metric| (category.name.as_str(), metric.name.as_str()))
        })
        .collect();
    if inline.is_empty() {
        return Ok(source.to_owned());
    }
    let stripped = edit::remove(source, format, &inline);

    // The editor reads only part of the syntax, so the file it writes must parse to the same
    // configuration without baselines.
    let mut expected = without_locations(config.clone());
    for metric in expected.categories.iter_mut().flat_map(|c| &mut c.metrics) {
        metric.baseline = None;
    }
    let written = config::parse(&stripped, format).ok().map(without_locations);
    if written.as_ref() == Some(&expected) {
        return Ok(stripped);
    }

    // Blames the first metric whose baseline is still there.
    let blamed = inline
        .iter()
        .find(|(category, metric)| {
            written
                .as_ref()
                .and_then(|written| find(written, category, metric))
                .is_none_or(|written| written.baseline.is_some())
        })
        .or(inline.first());
    let (category, metric) = blamed.map_or(("", ""), |(category, metric)| (*category, *metric));
    let location = find(&config, category, metric).map(|metric| metric.location);
    Err(cannot(metric, location))
}

/// Returns `config` with the baselines a run records and no others, as a first `uphill --update`
/// would write them.
pub fn accepted(config: &Config, report: &Report) -> Config {
    let mut cleared = config.clone();
    for metric in cleared.categories.iter_mut().flat_map(|c| &mut c.metrics) {
        metric.baseline = None;
    }
    let changes = updates(&cleared, report);
    apply(&cleared, &changes)
}

/// A metric's new baseline, by category and metric name.
type Update<'c> = (&'c str, &'c str, Option<Baseline>);

/// Returns each metric whose baseline differs from what the run records, with its new baseline.
/// A category that did not run or failed keeps its baselines.
fn updates<'c>(config: &'c Config, report: &Report) -> Vec<Update<'c>> {
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

    changes
}

/// Returns `config` with each updated metric's new baseline.
fn apply(config: &Config, changes: &[Update<'_>]) -> Config {
    let mut updated = config.clone();
    for category in &mut updated.categories {
        for metric in &mut category.metrics {
            let change = changes
                .iter()
                .find(|(c, m, _)| *c == category.name && *m == metric.name);
            if let Some((_, _, baseline)) = change {
                metric.baseline.clone_from(baseline);
            }
        }
    }
    updated
}

/// Returns the size, count, or check that `config` records for one metric of a subject, or for one
/// of its items, or `None` when it records none.
pub fn value(
    config: &Config,
    category: &str,
    metric: &str,
    subject: Option<&str>,
    item: Option<&str>,
) -> Option<Value> {
    match (
        recorded_entry(find(config, category, metric)?, subject)?,
        item,
    ) {
        (Entry::Single(record), None) => record.value,
        (Entry::Items(items), Some(item)) => items.iter().find(|(name, _)| name == item)?.1.value,
        _ => None,
    }
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
pub(crate) fn exact(value: Value) -> String {
    let (amount, units) = match value {
        Value::Check(true) => return "pass".to_owned(),
        Value::Check(false) => return "fail".to_owned(),
        Value::Skip => return "skip".to_owned(),
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
        let value = (!timing).then(|| metric.single());
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
    // A check that starts or stops skipping starts or stops having a result.
    match (before, after) {
        (Value::Skip, Value::Skip) => return None,
        (Value::Skip, _) => return Some(Direction::New),
        (_, Value::Skip) => return Some(Direction::Gone),
        _ => {}
    }

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
fn cannot(metric: &str, location: Option<Location>) -> Error {
    Error {
        code: Code::InvalidType,
        location,
        message: format!(
            "Uphill cannot move the baseline of metric `{metric}` into the baseline file. Delete the metric's `baseline` key, then run `uphill --update` again"
        ),
    }
}

#[cold]
fn not_mapping(source: &str, offset: usize) -> Error {
    Error::at(
        Code::InvalidType,
        source,
        offset,
        "expected a mapping of category names to metrics, and of metric names to baselines. Run `uphill --update` to rewrite the baseline file".to_owned(),
    )
}

#[cold]
fn invalid(source: &str, offset: usize) -> Error {
    Error::at(
        Code::InvalidValue,
        source,
        offset,
        "expected a size, count, or check, optionally followed by `target <value>`, or a mapping of items or subjects to them. Run `uphill --update` to rewrite the baseline".to_owned(),
    )
}
