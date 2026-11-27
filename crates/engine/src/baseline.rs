//! Baselines and targets: the baseline file beside the configuration, `uphill.baseline.yaml` or
//! `uphill.baseline.toml`, holds each metric's targets and the sizes, counts, and checks that
//! `uphill --update` accepted. Each run reports what changed since. The configuration holds how
//! each metric is measured and scored, and Uphill rewrites the whole baseline file instead of
//! editing it in place, so its comments do not survive an update.
//!
//! A metric without items records one baseline and its target. A metric with items records each
//! item, sorted by name, with its baseline and its own target side by side. A category whose lines name
//! subjects records each subject first, in the order the run wrote them. Timings and metrics that
//! vary record only their targets, because their values differ between machines and runs, so a
//! check that varies records nothing.
//!
//! A key with `*` and only a target is a pattern, which sets the target of each item without its
//! own, and `*` alone is the default. An item's own target has the final say, then the first
//! pattern that matches, then the target on its line, then the default. `uphill --update` writes
//! the target each item scored against as its own, unless it equals the pattern or default that
//! applies without it. Updating twice from the same code writes the same file.
//!
//! The items of a check start with a default record, `*` with a `baseline`, which holds the check
//! of each item without its own record. `uphill --update` writes the most common check there and
//! lists only the items that differ from it.
//!
//! ```yaml
//! footprint:
//!   bundle: { baseline: 27.5kB, target: 30kB }
//!   modules: { baseline: 412 }
//!   export-size:
//!     "*": { target: 5kB }
//!     "viem/chains/*": { target: 80kB }
//!     viem: { baseline: 1kB }
//!     viem/chains: { baseline: 61kB, target: 70kB }
//!   links:
//!     "*": { baseline: pass }
//!     about.html: { baseline: fail }
//!
//! speed:
//!   import-time:
//!     viem: { target: 20ms }
//! ```
//!
//! Uphill still reads the short form of older files, such as `61kB target 80kB`, and older files
//! that list every check.
//!
//! Older configurations set each metric's `target`, `targets`, and `baseline` keys instead. Uphill
//! still reads them, so older revisions compare and keep their history, and `uphill --update`
//! moves them into the baseline file.

use std::borrow::Cow;
use std::fmt::Write;

use yaml_rust2::Yaml;

use crate::check::Source;
use crate::config::{self, Better, Config, Format, Metric, Pattern, without_locations};
use crate::document::{self, Node};
use crate::edit;
use crate::error::{Code, Error, Location};
use crate::format::{SIZES, TIMES};
use crate::pack::Pack;
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
    /// The measurement's own target, which has the final say over patterns, its line, and the
    /// default.
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
    /// Returns the entry as the configuration writes it. With `compact`, the items of a check
    /// start with a default record, `*`, that holds the most common check, and only the items
    /// that differ from it follow. A tie goes to `pass`, then `skip`, then `fail`.
    fn baseline(self, compact: bool) -> Baseline {
        let mut items = match self {
            Self::Items(items) => items,
            Self::Single(record) => return Baseline::Record(record),
        };

        let checks = [Value::Check(true), Value::Skip, Value::Check(false)];
        let count = |check: Value| {
            items
                .iter()
                .filter(|(_, record)| *record == untargeted(check))
                .count()
        };
        let common = checks
            .into_iter()
            .map(|check| (check, count(check)))
            .filter(|(_, count)| *count > 0)
            .reduce(|best, next| if next.1 > best.1 { next } else { best });
        if compact && let Some((check, _)) = common {
            items.retain(|(_, record)| *record != untargeted(check));
            items.insert(0, (DEFAULT.to_owned(), untargeted(check)));
        }

        Baseline::Map(
            items
                .into_iter()
                .map(|(item, record)| (item, Baseline::Record(record)))
                .collect(),
        )
    }
}

/// The key of the default record, which holds the check of each item without its own record.
const DEFAULT: &str = "*";

/// Returns the record of a value without a target.
fn untargeted(value: Value) -> Record {
    Record {
        target: None,
        value: Some(value),
    }
}

/// Returns whether any metric in `config` records a size, count, or check. Targets alone are not a
/// baseline.
pub fn recorded(config: &Config) -> bool {
    config
        .categories
        .iter()
        .flat_map(|category| &category.metrics)
        .any(|metric| metric.baseline.as_ref().is_some_and(valued))
}

/// Returns whether a baseline records a size, count, or check, not only targets.
fn valued(baseline: &Baseline) -> bool {
    match baseline {
        Baseline::Map(entries) => entries.iter().any(|(_, baseline)| valued(baseline)),
        Baseline::Record(record) => record.value.is_some(),
    }
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
                for Paired {
                    after,
                    before,
                    item,
                } in records
                {
                    let (before, after) = (before.unwrap_or_default(), after.unwrap_or_default());
                    if let Some(TargetChange {
                        after,
                        before,
                        direction,
                    }) = target_change(&metric.metric, item, before.target, after.target)
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

            for Paired {
                after,
                before,
                item,
            } in pair(&before, &after)
            {
                match (before, after) {
                    (Some(before), Some(after)) => {
                        // A record with only a target has no accepted value yet.
                        match (before.value, after.value) {
                            (Some(old), Some(new)) => {
                                if let Some(direction) = direction(old, new, metric.metric.better) {
                                    push(name, item, Some(old), Some(new), direction);
                                }
                            }
                            (None, Some(new)) => push(name, item, None, Some(new), Direction::New),
                            _ => {}
                        }
                        if let Some(change) =
                            target_change(&metric.metric, item, before.target, after.target)
                        {
                            push(name, item, change.before, change.after, change.direction);
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
            let valued = |entry: Entry| match entry {
                Entry::Items(items) => items.iter().any(|(_, record)| record.value.is_some()),
                Entry::Single(record) => record.value.is_some(),
            };
            if !written && recorded_entry(metric, subject).is_some_and(valued) {
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
            for (subject, baseline) in subjects {
                if !valued(baseline)
                    || ran
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

/// Returns `config` with the targets and baselines that the baseline file `source` records. An
/// entry for a category or metric that `config` does not declare is left out, and the next
/// `uphill --update` drops it. An entry replaces the `baseline` key that an older configuration
/// sets, its default replaces `target`, and its patterns replace `targets`.
///
/// # Errors
///
/// Returns an error for YAML or TOML syntax, a file that does not map categories to metrics, and
/// a baseline that does not parse.
pub fn read(mut config: Config, source: &str, format: Format) -> Result<Config, Error> {
    let Some(document) = document::parse(source, format)? else {
        return Ok(config);
    };
    let document::Kind::Mapping(categories) = &document.kind else {
        return Err(not_mapping(source, document.offset));
    };

    for (key, value) in categories {
        let document::Kind::Mapping(metrics) = &value.kind else {
            return Err(not_mapping(source, value.offset));
        };
        let mut category = config
            .categories
            .iter_mut()
            .find(|category| Some(category.name.as_str()) == key.text());
        for (key, value) in metrics {
            let Targets {
                baseline,
                default,
                patterns,
            } = targets(source, value)?;
            let metric = category.as_deref_mut().and_then(|category| {
                category
                    .metrics
                    .iter_mut()
                    .find(|metric| Some(metric.name.as_str()) == key.text())
            });
            if let Some(metric) = metric {
                metric.baseline = baseline;
                if default.is_some() {
                    metric.target = default;
                }
                if !patterns.is_empty() {
                    metric.targets = patterns;
                }
            }
        }
    }

    Ok(config)
}

/// One metric of the baseline file: its baseline, its default target, and its patterns.
struct Targets {
    baseline: Option<Baseline>,
    default: Option<Value>,
    patterns: Vec<Pattern>,
}

/// Parses one metric of the baseline file into its baseline, its default target, and its
/// patterns. A record without items sets the default too, so `bundle: { target: 5kB }` targets a
/// metric before its first update.
fn targets(source: &str, node: &Node) -> Result<Targets, Error> {
    if let Some(record) = record(source, node) {
        let record = record?;
        return Ok(Targets {
            baseline: Some(Baseline::Record(record)),
            default: record.target,
            patterns: Vec::new(),
        });
    }
    let document::Kind::Mapping(pairs) = &node.kind else {
        return Err(invalid(source, node.offset));
    };

    let mut default = None;
    let mut patterns = Vec::new();
    let mut entries = Vec::new();
    for (key, value) in pairs {
        let name = key.text().ok_or_else(|| invalid(source, key.offset))?;
        let pattern = match record(source, value) {
            Some(Ok(Record {
                target: Some(target),
                value: None,
            })) if name.contains('*') => Some(target),
            _ => None,
        };
        match pattern {
            Some(target) if name == DEFAULT => default = Some(target),
            Some(target) => patterns.push(Pattern {
                location: Location::of(source, key.offset),
                pattern: name.to_owned(),
                source: Source::Baselines,
                target,
            }),
            None => {
                let baseline = parse_at(source, value, 1)?;
                // The default record of a check can hold the default target too.
                if let (DEFAULT, Baseline::Record(record)) = (name, &baseline) {
                    default = default.or(record.target);
                }
                entries.push((name.to_owned(), baseline));
            }
        }
    }

    Ok(Targets {
        baseline: (!entries.is_empty()).then_some(Baseline::Map(entries)),
        default,
        patterns,
    })
}

/// Returns the baseline file with each metric's targets, and this run's sizes, counts, and checks
/// with the target each measurement scored against, or an empty string when no metric has any. A
/// category that did not run or failed keeps its baselines, so updating twice from the same code
/// writes the same file.
pub fn write(config: &Config, report: &Report, format: Format) -> String {
    render(&apply(config, &updates(config, report, true)), format)
}

/// Returns the baseline file that holds the targets and baselines of `config`, or an empty string
/// when no metric has any.
///
/// Categories and metrics keep the order the configuration declares. A metric with items or
/// subjects takes one line per pattern, then one per entry, so a change shows as one line in a
/// diff.
pub fn render(config: &Config, format: Format) -> String {
    let categories: Vec<(&str, Vec<(String, Baseline)>)> = config
        .categories
        .iter()
        .map(|category| {
            let metrics = category
                .metrics
                .iter()
                .filter_map(|metric| Some((metric.name.clone(), file_entry(metric)?)))
                .collect::<Vec<_>>();
            (category.name.as_str(), metrics)
        })
        .filter(|(_, metrics)| !metrics.is_empty())
        .collect();
    if categories.is_empty() {
        return String::new();
    }

    let mut out = String::from(
        "# Each measurement's `baseline`, the size, count, or check `uphill --update` accepted, and its\n# `target`. Edit a target here, and each run reports what changed since the baseline.\n",
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

/// Returns what the baseline file holds for a metric: its record with its target, or its default
/// and patterns before its items or subjects, or `None` when it has neither targets nor baseline.
fn file_entry(metric: &Metric) -> Option<Baseline> {
    let only = |target| {
        Baseline::Record(Record {
            target: Some(target),
            value: None,
        })
    };
    let entries = match &metric.baseline {
        Some(Baseline::Record(record)) => {
            return Some(Baseline::Record(Record {
                target: record.target.or(metric.target),
                value: record.value,
            }));
        }
        Some(Baseline::Map(entries)) => entries.as_slice(),
        None if metric.targets.is_empty() => return metric.target.map(only),
        None => &[],
    };

    // A pattern without `*`, from an older configuration, names one item, and the item's own
    // target takes its place.
    let patterns = metric
        .target
        .map(|target| ("*".to_owned(), only(target)))
        .into_iter()
        .chain(
            metric
                .targets
                .iter()
                .map(|p| (p.pattern.clone(), only(p.target))),
        )
        .filter(|(key, _)| !entries.iter().any(|(entry, _)| entry == key));

    // The default record holds the default target, since one key cannot appear twice.
    let entries = entries
        .iter()
        .cloned()
        .map(|(key, baseline)| match baseline {
            Baseline::Record(record) if key == DEFAULT => {
                let target = record.target.or(metric.target);
                (key, Baseline::Record(Record { target, ..record }))
            }
            baseline => (key, baseline),
        });
    Some(Baseline::Map(patterns.chain(entries).collect()))
}

/// Writes YAML entries, one per line at `indent` spaces: a record after its key, or a mapping on
/// the lines below it.
fn yaml_block(out: &mut String, entries: &[(String, Baseline)], indent: usize) {
    for (key, baseline) in entries {
        let _ = write!(out, "\n{:indent$}{}:", "", yaml_key(key));
        match baseline {
            Baseline::Map(entries) => yaml_block(out, entries, indent + 2),
            Baseline::Record(record) => {
                let _ = write!(out, " {}", inline(*record, Format::Yaml));
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
            let _ = writeln!(out, "{} = {}", toml_key(key), inline(*record, Format::Toml));
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
pub(crate) fn toml_key(name: &str) -> Cow<'_, str> {
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

/// Returns the configuration `source` without the `target`, `targets`, and `baseline` keys that
/// older configurations set, or `source` itself when it sets none. `source` composes with
/// `packs`, so a project category that overrides a pack's category, such as with `metric: false`,
/// parses as it does in a run.
///
/// # Errors
///
/// Returns an error when `source` does not parse, or when one of those keys has a form that Uphill
/// cannot remove, such as a TOML baseline in a table of its own.
pub fn strip(source: &str, format: Format, packs: &[Pack]) -> Result<String, Error> {
    // A pack's targets come from its own baseline file, so the packs compose without them and
    // only the keys that `source` sets move.
    let packs: Vec<Pack> = packs
        .iter()
        .cloned()
        .map(|mut pack| {
            for metric in pack
                .config
                .categories
                .iter_mut()
                .flat_map(|c| &mut c.metrics)
            {
                metric.baseline = None;
                metric.target = None;
                metric.targets.clear();
            }
            pack
        })
        .collect();
    let config = config::compose(source, format, &packs)?;
    let inline: Vec<(&str, &str)> = config
        .categories
        .iter()
        .flat_map(|category| {
            category
                .metrics
                .iter()
                .filter(|metric| moves(metric))
                .map(|metric| (category.name.as_str(), metric.name.as_str()))
        })
        .collect();
    if inline.is_empty() {
        return Ok(source.to_owned());
    }
    // One key at a time, so the edits of one pass never overlap.
    let stripped = ["baseline", "target", "targets"]
        .into_iter()
        .fold(source.to_owned(), |text, key| {
            edit::remove(&text, format, &inline, key)
        });

    // The editor reads only part of the syntax, so the file it writes must parse to the same
    // configuration without targets and baselines.
    let mut expected = without_locations(config.clone());
    for metric in expected.categories.iter_mut().flat_map(|c| &mut c.metrics) {
        metric.baseline = None;
        metric.target = None;
        metric.targets.clear();
    }
    let written = config::compose(&stripped, format, &packs)
        .ok()
        .map(without_locations);
    if written.as_ref() == Some(&expected) {
        return Ok(stripped);
    }

    // Blames the first metric whose keys are still there.
    let blamed = inline
        .iter()
        .find(|(category, metric)| {
            written
                .as_ref()
                .and_then(|written| find(written, category, metric))
                .is_none_or(moves)
        })
        .or(inline.first());
    let (category, metric) = blamed.map_or(("", ""), |(category, metric)| (*category, *metric));
    let location = find(&config, category, metric).map(|metric| metric.location);
    Err(cannot(metric, location))
}

/// Returns whether an older configuration sets a metric's `target`, `targets`, or `baseline`.
fn moves(metric: &Metric) -> bool {
    metric.baseline.is_some() || metric.target.is_some() || !metric.targets.is_empty()
}

/// Returns `config` with the baselines a run records and no others, as a first `uphill --update`
/// would write them, but with every item of a check listed, so an item that the other run no
/// longer writes reports as gone.
pub fn accepted(config: &Config, report: &Report) -> Config {
    let mut cleared = config.clone();
    for metric in cleared.categories.iter_mut().flat_map(|c| &mut c.metrics) {
        metric.baseline = None;
    }
    let changes = updates(&cleared, report, false);
    apply(&cleared, &changes)
}

/// A metric's new baseline, by category and metric name.
struct Update<'c> {
    baseline: Option<Baseline>,
    category: &'c str,
    metric: &'c str,
}

/// Returns each metric whose baseline differs from what the run records, with its new baseline.
/// A category that did not run or failed keeps its baselines. With `compact`, the items of a
/// check record a default and only the items that differ from it.
fn updates<'c>(config: &'c Config, report: &Report, compact: bool) -> Vec<Update<'c>> {
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
                    .map(|(subject, entry)| (subject, entry.baseline(compact)))
                    .collect();
                (!subjects.is_empty()).then_some(Baseline::Map(subjects))
            } else {
                entries.next().map(|(_, entry)| entry.baseline(compact))
            };

            if baseline != metric.baseline {
                changes.push(Update {
                    baseline,
                    category: category.name.as_str(),
                    metric: metric.name.as_str(),
                });
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
                .find(|change| change.category == category.name && change.metric == metric.name);
            if let Some(change) = change {
                metric.baseline.clone_from(&change.baseline);
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
    lookup(find(config, category, metric)?, subject, item, true)?.value
}

/// Returns the own target the baseline file sets for one measurement of a metric: of a subject,
/// and of one of its items.
pub(crate) fn target(metric: &Metric, subject: Option<&str>, item: Option<&str>) -> Option<Value> {
    lookup(metric, subject, item, false)?.target
}

/// Returns a metric's record of one measurement: of a subject, and of one of its items. With
/// `default`, an item without a record takes the default record of its items.
fn lookup<'m>(
    metric: &'m Metric,
    subject: Option<&str>,
    item: Option<&str>,
    default: bool,
) -> Option<&'m Record> {
    let mut baseline = metric.baseline.as_ref()?;
    let keys = [(subject, false), (item, default)];
    for (key, default) in keys.into_iter().filter_map(|(key, d)| Some((key?, d))) {
        let Baseline::Map(entries) = baseline else {
            return None;
        };
        let find = |key: &str| entries.iter().find(|(name, _)| name == key);
        baseline = &find(key).or_else(|| find(DEFAULT).filter(|_| default))?.1;
    }
    match baseline {
        Baseline::Record(record) => Some(record),
        Baseline::Map(_) => None,
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
    parse_at(source, node, 0)
}

/// Parses a baseline at `depth` mappings below the metric, which allows two.
fn parse_at(source: &str, node: &Node, depth: usize) -> Result<Baseline, Error> {
    if let Some(record) = record(source, node) {
        return record.map(Baseline::Record);
    }
    match &node.kind {
        document::Kind::Mapping(pairs) if depth < 2 && !pairs.is_empty() => pairs
            .iter()
            .map(|(key, value)| {
                let key = key.text().ok_or_else(|| invalid(source, key.offset))?;
                Ok((key.to_owned(), parse_at(source, value, depth + 1)?))
            })
            .collect::<Result<_, Error>>()
            .map(Baseline::Map),
        _ => Err(invalid(source, node.offset)),
    }
}

/// Formats a record as an inline mapping, such as `{ baseline: 61kB, target: 80kB }` in YAML or
/// `{ baseline = "61kB", target = "80kB" }` in TOML, which writes counts without quotes.
fn inline(record: Record, format: Format) -> String {
    let parts: Vec<String> = [("baseline", record.value), ("target", record.target)]
        .into_iter()
        .filter_map(|(key, value)| {
            let value = value?;
            Some(match (format, value) {
                (Format::Toml, Value::Count(_)) => format!("{key} = {}", exact(value)),
                (Format::Toml, _) => format!("{key} = {}", quoted(&exact(value))),
                (Format::Yaml, _) => format!("{key}: {}", exact(value)),
            })
        })
        .collect();
    format!("{{ {} }}", parts.join(", "))
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

/// Returns whether a metric's values differ between runs: a time, or a metric that the
/// configuration marks with `varies`. Its baseline records only targets, so a check that varies
/// records nothing.
fn varies(metric: &MetricScore) -> bool {
    metric.kind == Kind::Time || metric.metric.varies
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

/// One item's records in two entries, as [`pair`] returns them.
struct Paired<'a> {
    after: Option<Record>,
    before: Option<Record>,
    item: Option<&'a str>,
}

/// Pairs the records of two entries by item: each item in either, with its record on each side.
/// An item without a record before takes the default record, if `before` has one. Two entries
/// without items pair as one record with no item.
fn pair<'a>(before: &'a Entry, after: &'a Entry) -> Vec<Paired<'a>> {
    match (before, after) {
        (Entry::Single(before), Entry::Single(after)) => vec![Paired {
            after: Some(*after),
            before: Some(*before),
            item: None,
        }],
        (Entry::Items(before), Entry::Items(after)) => {
            let find = |items: &[(String, Record)], name: &str| {
                items.iter().find(|(item, _)| item == name).map(|(_, r)| *r)
            };
            let default = find(before, DEFAULT).filter(|record| record.value.is_some());
            let mut pairs: Vec<_> = after
                .iter()
                .map(|(item, record)| {
                    let recorded = find(before, item).or(default);
                    Paired {
                        after: Some(*record),
                        before: recorded,
                        item: Some(item.as_str()),
                    }
                })
                .collect();
            pairs.extend(
                before
                    .iter()
                    .filter(|(item, _)| item != DEFAULT && find(after, item).is_none())
                    .map(|(item, record)| Paired {
                        after: None,
                        before: Some(*record),
                        item: Some(item.as_str()),
                    }),
            );
            pairs
        }
        (Entry::Single(before), Entry::Items(_)) => vec![Paired {
            after: None,
            before: Some(*before),
            item: None,
        }],
        (Entry::Items(_), Entry::Single(after)) => vec![Paired {
            after: Some(*after),
            before: None,
            item: None,
        }],
    }
}

/// Returns the entry a scored metric records, with the target each measurement scored against,
/// or `None` for a timing without targets.
fn entry(metric: &MetricScore) -> Option<Entry> {
    let timing = varies(metric);
    let default = metric.metric.target;

    // A target that equals the pattern or default it falls back to records nothing. A line
    // target that differs from the default still records, so the file holds it for editing.
    let own = |target: Option<Value>, pattern: Option<Value>, line: Option<Value>| {
        let target = target.or(default);
        let inherited = match pattern {
            Some(pattern) => target == Some(pattern),
            None => target == default && line.is_none_or(|line| Some(line) == default),
        };
        target.filter(|_| !inherited)
    };

    if metric.items.is_empty() {
        let value = (!timing).then(|| metric.single());
        let target = own(metric.own_target, None, metric.line_target);
        if value.is_none() && target.is_none() {
            return None;
        }
        return Some(Entry::Single(Record { target, value }));
    }

    let mut items: Vec<(String, Record)> = metric
        .items
        .iter()
        .map(|item| {
            let pattern = metric.metric.pattern(&item.name).map(|p| p.target);
            let record = Record {
                target: own(item.target, pattern, item.line_target),
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

/// A line target's change: the targets an item scores against before and after, and the direction.
struct TargetChange {
    after: Option<Value>,
    before: Option<Value>,
    direction: Direction,
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
) -> Option<TargetChange> {
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

    Some(TargetChange {
        after,
        before,
        direction,
    })
}

/// Returns whether two amounts are equal within rounding.
fn same(a: f64, b: f64) -> bool {
    (a - b).abs() <= f64::EPSILON * a.abs().max(b.abs())
}

/// Parses a record node: a mapping with `baseline`, `target`, or both, such as
/// `{ baseline: 61kB, target: 80kB }`, or the short form of older files, such as
/// `61kB target 80kB`. Returns `None` for a node of another form, such as a mapping of items.
fn record(source: &str, node: &Node) -> Option<Result<Record, Error>> {
    let pairs = match &node.kind {
        document::Kind::Scalar { text, .. } if !node.is_null() => {
            return Some(short(text).ok_or_else(|| invalid(source, node.offset)));
        }
        document::Kind::Mapping(pairs)
            if pairs
                .iter()
                .all(|(_, value)| matches!(value.kind, document::Kind::Scalar { .. }))
                && pairs
                    .iter()
                    .any(|(key, _)| matches!(key.text(), Some("baseline" | "target"))) =>
        {
            pairs
        }
        _ => return None,
    };

    let mut record = Record::default();
    for (key, node) in pairs {
        let text = node.text().filter(|_| !node.is_null());
        let parsed = match key.text() {
            Some("baseline") => text
                .and_then(baseline_value)
                .map(|value| record.value = Some(value)),
            Some("target") => text
                .and_then(target_value)
                .map(|target| record.target = Some(target)),
            _ => return Some(Err(invalid(source, key.offset))),
        };
        if parsed.is_none() {
            return Some(Err(invalid(source, node.offset)));
        }
    }
    Some(Ok(record))
}

/// Parses the short form of a record: a value, a value followed by `target <value>`, or
/// `target <value>` alone for a timing.
fn short(text: &str) -> Option<Record> {
    let (recorded, target) = match text.strip_prefix("target ") {
        Some(target) => (None, Some(target)),
        None => match text.split_once(" target ") {
            Some((recorded, target)) => (Some(recorded), Some(target)),
            None => (Some(text), None),
        },
    };

    Some(Record {
        target: match target {
            Some(target) => Some(target_value(target)?),
            None => None,
        },
        value: match recorded {
            Some(recorded) => Some(baseline_value(recorded)?),
            None => None,
        },
    })
}

/// Parses an accepted value: a size, count, or check, since a timing has none.
fn baseline_value(text: &str) -> Option<Value> {
    value::parse(text)
        .ok()
        .filter(|value| value.kind() != Kind::Time)
}

/// Parses a target, which has an amount, so a check cannot be one.
fn target_value(text: &str) -> Option<Value> {
    value::parse(text)
        .ok()
        .filter(|target| target.amount().is_some())
}

#[cold]
fn cannot(metric: &str, location: Option<Location>) -> Error {
    Error {
        code: Code::InvalidType,
        location,
        message: format!(
            "Uphill cannot move the `target`, `targets`, or `baseline` key of metric `{metric}` into the baseline file. Move them by hand, then run `uphill --update` again"
        ),
    }
}

#[cold]
fn not_mapping(source: &str, offset: usize) -> Error {
    Error::at(
        Code::InvalidType,
        source,
        offset,
        "expected a mapping of category names to metrics, and of metric names to targets and baselines".to_owned(),
    )
}

#[cold]
fn invalid(source: &str, offset: usize) -> Error {
    Error::at(
        Code::InvalidValue,
        source,
        offset,
        "expected `{ baseline: <value>, target: <value> }` with either key or both, or a mapping of items, patterns, or subjects to them. A baseline is a size, count, or check, and a target is a size, time, or count".to_owned(),
    )
}
