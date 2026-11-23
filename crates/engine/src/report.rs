//! Reports rendered from scored categories: the terminal output, `report.json`, and `report.md`.
//! [`crate::html`] renders `report.html` from the same data.
//! Each report shows the scores, the opportunities that would raise them most, and the changes
//! since the baselines. Rendering is deterministic, with no timestamps or paths, so the same
//! scores always produce the same bytes.

use std::fmt::Write;

use crate::baseline::{Change, Direction};
use crate::config::{self, Better, Rollup};
use crate::format;
use crate::score::{self, CategoryScore, MetricScore, Opportunity};
use crate::value::{Kind, Value};

/// How many of a list metric's worst items the terminal and Markdown reports show.
const WORST: usize = 3;

/// How many opportunities the terminal shows, and the Markdown report.
const TERMINAL_OPPORTUNITIES: usize = 5;
const MARKDOWN_OPPORTUNITIES: usize = 10;

/// How many changes the terminal shows. The Markdown and JSON reports list every change.
const TERMINAL_CHANGES: usize = 10;

/// Everything one run measured.
#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    /// Categories in the order they ran.
    pub categories: Vec<CategoryReport>,
    /// The changes since the baselines, or `None` when no metric has a baseline.
    pub changes: Option<Vec<Change>>,
}

/// One category's scores for one subject, or the reason it has none.
#[derive(Clone, Debug, PartialEq)]
pub struct CategoryReport {
    /// What the category measures, from `retros.yaml`.
    pub description: Option<String>,
    /// The category name.
    pub name: String,
    /// The category's scores, or why its script produced none.
    pub result: Result<CategoryScore, Failure>,
    /// The subject the scores belong to, or `None` when the lines name no subject.
    pub subject: Option<String>,
    /// The name the HTML and Markdown reports show, such as `Speed`, from
    /// [`crate::config::Category::title`].
    pub title: String,
}

impl CategoryReport {
    /// Returns the category's name as the terminal shows it: `speed`, or `zc: speed` with a
    /// subject.
    pub fn label(&self) -> String {
        label(self.subject.as_deref(), &self.name)
    }

    /// Returns the title of one of the category's metrics, or the metric name in words when the
    /// category scored no such metric.
    pub(crate) fn metric_title(&self, metric: &str) -> String {
        self.result
            .as_ref()
            .ok()
            .and_then(|scored| scored.metrics.iter().find(|m| m.metric.name == metric))
            .map_or_else(|| config::words(metric), |m| m.metric.title())
    }
}

/// Formats a category name with its subject, such as `zc: speed`, as metric lines write it.
pub fn label(subject: Option<&str>, category: &str) -> String {
    match subject {
        Some(subject) => format!("{subject}: {category}"),
        None => category.to_owned(),
    }
}

/// Why a category has no scores.
#[derive(Clone, Debug, PartialEq)]
pub struct Failure {
    /// Lines that explain the failure, such as the end of standard error. May be empty.
    pub details: String,
    /// A one-line reason, such as `script exited with status 1`.
    pub message: String,
}

/// A miss that fails `--require-targets`.
#[derive(Clone, Debug, PartialEq)]
pub enum Miss {
    /// A category whose script failed, so its metrics have no values.
    Failed {
        /// The category as the terminal names it, such as `zc: speed`.
        category: String,
        /// Why the category has no values.
        message: String,
    },
    /// A metric whose value is past its target, or a check that fails.
    Missed {
        /// The category as the terminal names it, such as `zc: speed`.
        category: String,
        /// The item the roll-up selected, or the check item that fails.
        item: Option<String>,
        /// The metric name.
        metric: String,
        /// The target, or `None` for a check.
        target: Option<Value>,
        /// The value past the target, or `fail`.
        value: Value,
    },
}

/// Returns every miss in a report, in report order: each failed category, each metric whose
/// collapsed value is past its target, and each check item that fails. A value equal to its
/// target meets it. Metrics without a target and metrics a subject did not write never miss.
pub fn misses(report: &Report) -> Vec<Miss> {
    let mut misses = Vec::new();

    for category in &report.categories {
        let scored = match &category.result {
            Ok(scored) => scored,
            Err(failure) => {
                misses.push(Miss::Failed {
                    category: category.label(),
                    message: failure.message.clone(),
                });
                continue;
            }
        };

        for metric in &scored.metrics {
            let missed = |item: Option<String>, value, target| Miss::Missed {
                category: category.label(),
                item,
                metric: metric.metric.name.clone(),
                target,
                value,
            };

            if metric.kind == Kind::Check {
                let failing = metric
                    .items
                    .iter()
                    .filter(|item| item.value == Value::Check(false));
                misses.extend(
                    failing.map(|item| missed(Some(item.name.clone()), Value::Check(false), None)),
                );
                if metric.items.is_empty() && metric.value < 1.0 {
                    misses.push(missed(None, Value::Check(false), None));
                }
                continue;
            }

            let Some(target) = metric.target else {
                continue;
            };
            let past = target
                .amount()
                .is_some_and(|amount| match metric.metric.better {
                    Better::Higher => metric.value < amount,
                    Better::Lower => metric.value > amount,
                });
            if past {
                let value = Value::of(metric.kind, metric.value);
                misses.push(missed(metric.at.clone(), value, Some(target)));
            }
        }
    }

    misses
}

/// Renders the misses that fail `--require-targets` for the terminal, such as
/// `footprint  bundle  27.5 kB  target 5 kB`.
pub fn missed(misses: &[Miss]) -> String {
    let rows: Vec<[String; 4]> = misses
        .iter()
        .map(|miss| match miss {
            Miss::Failed { category, message } => [
                category.clone(),
                message.clone(),
                String::new(),
                String::new(),
            ],
            Miss::Missed {
                category,
                item,
                metric,
                target,
                value,
            } => [
                category.clone(),
                item.as_ref()
                    .map_or_else(|| metric.clone(), |item| format!("{metric} {item}")),
                format::value(*value),
                target
                    .map(|target| format!("target {}", format::value(target)))
                    .unwrap_or_default(),
            ],
        })
        .collect();

    let mut out = String::from("\nMissed targets\n");
    table(&mut out, &rows);
    out
}

/// Returns Lighthouse's band for a score: `●` for 90 to 100, `■` for 50 to 89, and `▲` for 0 to
/// 49.
pub fn band(score: u8) -> char {
    match score {
        90.. => '●',
        50.. => '■',
        _ => '▲',
    }
}

/// Renders one category for the terminal: its score, then each metric's score, value, and
/// target, and the worst items of each list.
pub fn terminal(category: &CategoryReport) -> String {
    let mut out = String::new();

    let scored = match &category.result {
        Ok(scored) => scored,
        Err(failure) => {
            let _ = writeln!(out, "✗ {}  {}", category.label(), failure.message);
            for line in failure.details.lines() {
                let _ = writeln!(out, "    {line}");
            }
            return out;
        }
    };

    let rounds = if scored.rounds > 1 {
        format!("  median of {} rounds", scored.rounds)
    } else {
        String::new()
    };
    let _ = writeln!(out, "{}  {}{rounds}", cell(scored.score), category.label());

    let rows: Vec<(&MetricScore, String)> = scored
        .metrics
        .iter()
        .map(|metric| (metric, value(metric)))
        .collect();
    let name_width = width(rows.iter().map(|(metric, _)| metric.metric.name.as_str()));
    let value_width = width(rows.iter().map(|(_, value)| value.as_str()));

    for (metric, value) in &rows {
        let line = format!(
            "  {}  {:name_width$}  {value:value_width$}  {}",
            cell(metric.score),
            metric.metric.name,
            target(metric)
        );
        let _ = writeln!(out, "{}", line.trim_end());

        let items: Vec<(String, String)> = worst(metric)
            .into_iter()
            .map(|item| {
                let target = item
                    .target
                    .map(|target| format!("  target {}", format::value(target)))
                    .unwrap_or_default();
                (format::value(item.value), format!("{}{target}", item.name))
            })
            .collect();
        let item_width = width(items.iter().map(|(value, _)| value.as_str()));
        for (value, name) in items {
            let _ = writeln!(
                out,
                "  {:5}  {:name_width$}  {value:item_width$}  {name}",
                "", ""
            );
        }
    }

    out
}

/// Renders the end of the terminal output: the opportunities that would raise scores most, and
/// the changes since the baselines.
pub fn summary(report: &Report) -> String {
    let mut out = String::new();

    let opportunities = opportunities(report);
    if !opportunities.is_empty() {
        out.push_str("\nOpportunities\n");
        let rows: Vec<[String; 4]> = opportunities
            .iter()
            .take(TERMINAL_OPPORTUNITIES)
            .map(|(category, opportunity)| {
                [
                    gain(opportunity),
                    category.label(),
                    subject(opportunity),
                    format!(
                        "{} → {}",
                        format::value(opportunity.value),
                        format::value(opportunity.target)
                    ),
                ]
            })
            .collect();
        table(&mut out, &rows);
    }

    match &report.changes {
        None => out.push_str(
            "\nNo baseline yet. Run `retros --update` to record these values as the baseline.\n",
        ),
        Some(changes) if changes.is_empty() => out.push_str("\nNo changes since the baseline\n"),
        Some(changes) => {
            out.push_str("\nSince the baseline\n");
            let sorted = sorted(changes);
            let rows: Vec<[String; 4]> = sorted
                .iter()
                .take(TERMINAL_CHANGES)
                .map(|change| {
                    [
                        change.direction.as_str().to_owned(),
                        label(change.subject.as_deref(), &change.category),
                        changed(change),
                        values(change),
                    ]
                })
                .collect();
            table(&mut out, &rows);

            if sorted.len() > TERMINAL_CHANGES {
                let _ = writeln!(
                    out,
                    "  and {} more in report.md",
                    sorted.len() - TERMINAL_CHANGES
                );
            }
        }
    }

    out
}

/// Renders the report as JSON. Values are in base units: bytes for sizes and nanoseconds for
/// times. A check's value is the share of items that pass, from 0 to 1.
pub fn json(report: &Report) -> String {
    let categories = report.categories.iter().map(|category| {
        let name = ("name", Json::String(category.name.clone()));
        let title = ("title", Json::String(category.title.clone()));
        let subject = ("subject", optional(category.subject.as_deref()));
        let description = ("description", optional(category.description.as_deref()));

        match &category.result {
            Err(failure) => Json::Object(vec![
                name,
                title,
                subject,
                description,
                ("score", Json::Null),
                (
                    "error",
                    Json::Object(vec![
                        ("message", Json::String(failure.message.clone())),
                        ("details", Json::String(failure.details.clone())),
                    ]),
                ),
            ]),
            Ok(scored) => Json::Object(vec![
                name,
                title,
                subject,
                description,
                ("score", score_json(scored.score)),
                ("rounds", Json::Number(count(scored.rounds))),
                (
                    "metrics",
                    Json::Array(scored.metrics.iter().map(metric_json).collect()),
                ),
            ]),
        }
    });

    let opportunities = opportunities(report)
        .into_iter()
        .map(|(category, opportunity)| {
            Json::Object(vec![
                ("category", Json::String(category.name.clone())),
                ("subject", optional(category.subject.as_deref())),
                ("metric", Json::String(opportunity.metric.clone())),
                ("item", optional(opportunity.item.as_deref())),
                ("items", Json::Number(count(opportunity.count))),
                ("value", value_json(opportunity.value)),
                ("target", value_json(opportunity.target)),
                ("gain", Json::Number(opportunity.gain.round())),
            ])
        });

    let changes = report.changes.as_ref().map_or(Json::Null, |changes| {
        Json::Array(
            sorted(changes)
                .into_iter()
                .map(|change| {
                    Json::Object(vec![
                        ("category", Json::String(change.category.clone())),
                        ("subject", optional(change.subject.as_deref())),
                        ("metric", Json::String(change.metric.clone())),
                        ("item", optional(change.item.as_deref())),
                        ("change", Json::String(change.direction.as_str().to_owned())),
                        ("before", change.before.map_or(Json::Null, value_json)),
                        ("after", change.after.map_or(Json::Null, value_json)),
                    ])
                })
                .collect(),
        )
    });

    let mut out = String::new();
    Json::Object(vec![
        ("version", Json::Number(1.0)),
        ("categories", Json::Array(categories.collect())),
        ("opportunities", Json::Array(opportunities.collect())),
        ("changes", changes),
    ])
    .write(&mut out, 0);
    out.push('\n');
    out
}

/// Renders the report as Markdown: a summary table of categories, the top opportunities, the
/// changes since the baselines, then a section per category with a table of metrics and the worst
/// items of each list.
pub fn markdown(report: &Report) -> String {
    let mut out = String::from("# Retros report\n\n");
    let score = |category: &CategoryReport| match &category.result {
        Ok(scored) => compact(scored.score),
        Err(failure) => format!("✗ {}", failure.message),
    };

    // With subjects, one row per subject and one column per category, so subjects compare at a
    // glance.
    let subjects = subjects(report);
    if subjects.iter().any(Option::is_some) {
        let names = names(report);
        let titles: Vec<String> = names
            .iter()
            .filter_map(|name| report.categories.iter().find(|c| c.name == *name))
            .map(|category| escape(&category.title))
            .collect();
        let _ = writeln!(
            out,
            "| Subject | {} |\n| --- |{}",
            titles.join(" | "),
            " --- |".repeat(names.len())
        );
        for subject in &subjects {
            let cells: Vec<String> = names
                .iter()
                .map(|name| {
                    report
                        .categories
                        .iter()
                        .find(|c| c.subject.as_deref() == *subject && c.name == *name)
                        .map_or_else(|| "–".to_owned(), |category| escape(&score(category)))
                })
                .collect();
            let _ = writeln!(
                out,
                "| {} | {} |",
                escape(subject.unwrap_or("–")),
                cells.join(" | ")
            );
        }
    } else {
        out.push_str("| Category | Score |\n| --- | --- |\n");
        for category in &report.categories {
            let _ = writeln!(
                out,
                "| {} | {} |",
                escape(&category.title),
                escape(&score(category))
            );
        }
    }

    let opportunities = opportunities(report);
    if !opportunities.is_empty() {
        out.push_str(
            "\n## Opportunities\n\n| Gain | Category | Change | Value | Target |\n| --- | --- | --- | --- | --- |\n",
        );
        for (category, opportunity) in opportunities.iter().take(MARKDOWN_OPPORTUNITIES) {
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} | {} |",
                gain(opportunity),
                escape(&label(category.subject.as_deref(), &category.title)),
                escape(&titled(
                    &category.metric_title(&opportunity.metric),
                    opportunity.item.as_deref(),
                    opportunity.count
                )),
                format::value(opportunity.value),
                format::value(opportunity.target)
            );
        }
    }

    if let Some(changes) = report
        .changes
        .as_ref()
        .filter(|changes| !changes.is_empty())
    {
        out.push_str(
            "\n## Changes since the baseline\n\n| Change | Category | Metric | Before | After |\n| --- | --- | --- | --- | --- |\n",
        );
        for change in sorted(changes) {
            let (category, metric) = titles(report, change);
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} | {} |",
                change.direction.as_str(),
                escape(&label(change.subject.as_deref(), &category)),
                escape(&titled(&metric, change.item.as_deref(), 1)),
                side_cell(change, change.before),
                side_cell(change, change.after)
            );
        }
    }

    for category in &report.categories {
        let _ = write!(
            out,
            "\n## {}\n\n",
            label(category.subject.as_deref(), &category.title)
        );

        let scored = match &category.result {
            Ok(scored) => scored,
            Err(failure) => {
                let _ = writeln!(out, "✗ {}", failure.message);
                if !failure.details.is_empty() {
                    let _ = write!(out, "\n```text\n{}\n```\n", failure.details);
                }
                continue;
            }
        };

        if scored.rounds > 1 {
            let _ = write!(
                out,
                "Values that vary between runs are the median of {} rounds.\n\n",
                scored.rounds
            );
        }

        out.push_str("| Score | Metric | Value | Target |\n| --- | --- | --- | --- |\n");
        for metric in &scored.metrics {
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} |",
                compact(metric.score),
                escape(&metric.metric.title()),
                escape(&value(metric)),
                metric.target.map(format::value).unwrap_or_default()
            );
        }

        for metric in &scored.metrics {
            let items = worst(metric);
            if items.is_empty() {
                continue;
            }

            let heading = if metric.kind == Kind::Check {
                "Failed items"
            } else {
                "Worst items"
            };
            let _ = write!(
                out,
                "\n{heading} of {}:\n\n| Item | Value | Target |\n| --- | --- | --- |\n",
                metric.metric.title()
            );
            for item in items {
                let _ = writeln!(
                    out,
                    "| {} | {} | {} |",
                    escape(&item.name),
                    format::value(item.value),
                    item.target
                        .or(metric.metric.target)
                        .map(format::value)
                        .unwrap_or_default()
                );
            }
        }
    }

    out
}

/// Returns every category's opportunities, largest gain first.
pub(crate) fn opportunities(report: &Report) -> Vec<(&CategoryReport, Opportunity)> {
    let mut all: Vec<(&CategoryReport, Opportunity)> = report
        .categories
        .iter()
        .filter_map(|category| Some((category, category.result.as_ref().ok()?)))
        .flat_map(|(category, scored)| {
            score::opportunities(scored)
                .into_iter()
                .map(move |opportunity| (category, opportunity))
        })
        .collect();
    all.sort_by(|(_, a), (_, b)| b.gain.total_cmp(&a.gain));
    all
}

/// Returns the subjects of a report in the order they first appear. A report without subjects
/// returns one `None`.
pub(crate) fn subjects(report: &Report) -> Vec<Option<&str>> {
    let mut subjects = Vec::new();
    for category in &report.categories {
        if !subjects.contains(&category.subject.as_deref()) {
            subjects.push(category.subject.as_deref());
        }
    }
    subjects
}

/// Returns the category names of a report in the order they first appear.
pub(crate) fn names(report: &Report) -> Vec<&str> {
    let mut names = Vec::new();
    for category in &report.categories {
        if !names.contains(&category.name.as_str()) {
            names.push(category.name.as_str());
        }
    }
    names
}

/// Orders changes worst first: worse values, looser targets, new and gone metrics or items,
/// tighter targets, then better values.
pub(crate) fn sorted(changes: &[Change]) -> Vec<&Change> {
    let rank = |change: &&Change| match change.direction {
        Direction::Worse => 0,
        Direction::Looser => 1,
        Direction::New => 2,
        Direction::Gone => 3,
        Direction::Tighter => 4,
        Direction::Better => 5,
    };
    let mut sorted: Vec<&Change> = changes.iter().collect();
    sorted.sort_by_key(rank);
    sorted
}

/// Formats an opportunity's gain in category points, such as `+29`.
pub(crate) fn gain(opportunity: &Opportunity) -> String {
    format!("+{}", opportunity.gain.round())
}

/// Names what an opportunity changes, such as `bundle`, `export-size viem/chains`, or
/// `export-size, 4 items`.
fn subject(opportunity: &Opportunity) -> String {
    match (&opportunity.item, opportunity.count) {
        (Some(item), _) => format!("{} {item}", opportunity.metric),
        (None, 1) => opportunity.metric.clone(),
        (None, count) => format!("{}, {count} items", opportunity.metric),
    }
}

/// Names what a change concerns, such as `export-size viem/chains`.
fn changed(change: &Change) -> String {
    match &change.item {
        Some(item) => format!("{} {item}", change.metric),
        None => change.metric.clone(),
    }
}

/// Names a metric by its title, with its item as a Markdown code span, for the HTML and Markdown
/// reports: `Bundle`, `Export size` followed by `viem/chains` in backticks, or
/// `Export size, 4 items`.
pub(crate) fn titled(title: &str, item: Option<&str>, count: usize) -> String {
    match (item, count) {
        (Some(item), _) => format!("{title} `{item}`"),
        (None, 1) => title.to_owned(),
        (None, count) => format!("{title}, {count} items"),
    }
}

/// Returns the titles of a change's category and metric, from the category of this run that
/// scored it, or their names in words when this run has none.
pub(crate) fn titles(report: &Report, change: &Change) -> (String, String) {
    let category = report
        .categories
        .iter()
        .find(|category| category.subject == change.subject && category.name == change.category);
    match category {
        Some(category) => (
            category.title.clone(),
            category.metric_title(&change.metric),
        ),
        None => (
            config::words(&change.category),
            config::words(&change.metric),
        ),
    }
}

/// Formats a change's values, such as `61 kB → 64 kB`, one value for a new or gone item, or the
/// targets of a target change, such as `target 5 kB → 8 kB`.
pub(crate) fn values(change: &Change) -> String {
    if change.direction.is_target() {
        return format!("target {} → {}", side(change.before), side(change.after));
    }

    match (change.before, change.after) {
        (Some(before), Some(after)) => {
            format!("{} → {}", format::value(before), format::value(after))
        }
        (Some(value), None) | (None, Some(value)) => format::value(value),
        (None, None) => String::new(),
    }
}

/// Formats one side of a change for a Markdown cell: a value, a target such as `target 5 kB`, or
/// `none` for a target change to or from no target.
fn side_cell(change: &Change, value: Option<Value>) -> String {
    if change.direction.is_target() {
        format!("target {}", side(value))
    } else {
        value.map(format::value).unwrap_or_default()
    }
}

/// Formats a target, or `none` without one.
fn side(target: Option<Value>) -> String {
    target.map_or_else(|| "none".to_owned(), format::value)
}

/// Writes rows as aligned columns, each line indented by two spaces.
fn table<const N: usize>(out: &mut String, rows: &[[String; N]]) {
    let widths: Vec<usize> = (0..N)
        .map(|column| {
            width(
                rows.iter()
                    .filter_map(|row| row.get(column).map(String::as_str)),
            )
        })
        .collect();

    for row in rows {
        let mut line = String::from(" ");
        for (cell, width) in row.iter().zip(&widths) {
            let _ = write!(line, " {cell:width$} ");
        }
        let _ = writeln!(out, "{}", line.trim_end());
    }
}

/// Returns the widest text in characters.
fn width<'a>(texts: impl Iterator<Item = &'a str>) -> usize {
    texts
        .map(|text| text.chars().count())
        .max()
        .unwrap_or_default()
}

/// Formats a score with its band, such as `● 100` or `■  63`, or `    –` without a score.
fn cell(score: Option<u8>) -> String {
    score.map_or_else(
        || format!("{:>5}", "–"),
        |score| format!("{} {score:>3}", band(score)),
    )
}

/// Formats a score with its band for a Markdown cell, such as `■ 63`, or `–` without a score.
fn compact(score: Option<u8>) -> String {
    score.map_or_else(
        || "–".to_owned(),
        |score| format!("{} {score}", band(score)),
    )
}

/// Formats a metric's collapsed value, such as `27.5 kB`, `p90 9 kB (viem/op-stack)`, or
/// `2 of 3 pass`.
pub(crate) fn value(metric: &MetricScore) -> String {
    if metric.kind == Kind::Check {
        let passed = metric
            .items
            .iter()
            .filter(|item| item.value == Value::Check(true))
            .count();
        return match metric.items.len() {
            0 if metric.value >= 1.0 => "pass".to_owned(),
            0 => "fail".to_owned(),
            n => format!("{passed} of {n} pass"),
        };
    }

    let amount = format::amount(metric.kind, metric.value);
    if metric.items.is_empty() {
        return amount;
    }

    match &metric.at {
        Some(item) => format!("{} {amount} ({item})", rollup(metric.metric.score)),
        None => format!("{} {amount}", rollup(metric.metric.score)),
    }
}

/// Formats a metric's target and any weight other than 1, such as `target 5 kB, weight 2`.
pub(crate) fn target(metric: &MetricScore) -> String {
    let mut parts = Vec::new();

    if let Some(target) = metric.target.or(metric.metric.target) {
        parts.push(format!("target {}", format::value(target)));
    }
    if (metric.metric.weight - 1.0).abs() > f64::EPSILON {
        parts.push(format!(
            "weight {}",
            format::amount(Kind::Count, metric.metric.weight)
        ));
    }

    parts.join(", ")
}

/// Returns the items a report lists: the worst numbers of a list, or the failed checks.
fn worst(metric: &MetricScore) -> Vec<&score::Item> {
    metric
        .items
        .iter()
        .filter(|item| item.value != Value::Check(true))
        .take(WORST)
        .collect()
}

fn metric_json(metric: &MetricScore) -> Json {
    let (kind, unit) = match metric.kind {
        Kind::Check => ("check", Json::Null),
        Kind::Count => ("count", Json::Null),
        Kind::Size => ("size", Json::String("B".to_owned())),
        Kind::Time => ("time", Json::String("ns".to_owned())),
    };
    let items = metric.items.iter().map(|item| {
        Json::Object(vec![
            ("name", Json::String(item.name.clone())),
            ("value", value_json(item.value)),
            ("target", item.target.map_or(Json::Null, value_json)),
        ])
    });

    Json::Object(vec![
        ("name", Json::String(metric.metric.name.clone())),
        ("title", Json::String(metric.metric.title())),
        (
            "description",
            optional(metric.metric.description.as_deref()),
        ),
        ("kind", Json::String(kind.to_owned())),
        ("unit", unit),
        ("value", Json::Number(metric.value)),
        ("item", optional(metric.at.as_deref())),
        ("score", score_json(metric.score)),
        ("target", metric.target.map_or(Json::Null, value_json)),
        (
            "better",
            Json::String(
                match metric.metric.better {
                    Better::Higher => "higher",
                    Better::Lower => "lower",
                }
                .to_owned(),
            ),
        ),
        ("weight", Json::Number(metric.metric.weight)),
        (
            "rollup",
            if metric.kind == Kind::Check {
                Json::Null
            } else {
                Json::String(rollup(metric.metric.score).to_owned())
            },
        ),
        ("items", Json::Array(items.collect())),
    ])
}

/// Returns a value as JSON: a number in base units, or a boolean for a check.
fn value_json(value: Value) -> Json {
    match value {
        Value::Check(passed) => Json::Bool(passed),
        other => Json::Number(other.amount().unwrap_or_default()),
    }
}

fn score_json(score: Option<u8>) -> Json {
    score.map_or(Json::Null, |score| Json::Number(f64::from(score)))
}

fn optional(text: Option<&str>) -> Json {
    text.map_or(Json::Null, |text| Json::String(text.to_owned()))
}

/// Returns the name of a roll-up as reports write it, such as `p90`.
pub(crate) fn rollup(rollup: Rollup) -> &'static str {
    match rollup {
        Rollup::Max => "max",
        Rollup::Median => "median",
        Rollup::Min => "min",
        Rollup::P10 => "p10",
        Rollup::P90 => "p90",
        Rollup::Sum => "sum",
    }
}

/// Escapes text for a Markdown table cell.
fn escape(text: &str) -> String {
    text.replace('|', "\\|").replace(['\n', '\r'], " ")
}

#[expect(clippy::cast_precision_loss, reason = "counts never reach 2^53")]
fn count(n: usize) -> f64 {
    n as f64
}

/// A JSON value, written with two-space indentation and keys in insertion order.
enum Json {
    Array(Vec<Json>),
    Bool(bool),
    Null,
    Number(f64),
    Object(Vec<(&'static str, Json)>),
    String(String),
}

impl Json {
    fn write(&self, out: &mut String, depth: usize) {
        let indent = "  ".repeat(depth + 1);
        let close = "  ".repeat(depth);

        match self {
            Self::Array(values) if values.is_empty() => out.push_str("[]"),
            Self::Array(values) => {
                out.push_str("[\n");
                for (index, value) in values.iter().enumerate() {
                    out.push_str(&indent);
                    value.write(out, depth + 1);
                    out.push_str(if index + 1 < values.len() {
                        ",\n"
                    } else {
                        "\n"
                    });
                }
                let _ = write!(out, "{close}]");
            }
            Self::Bool(value) => {
                let _ = write!(out, "{value}");
            }
            Self::Null => out.push_str("null"),
            Self::Number(value) => {
                let _ = write!(out, "{value}");
            }
            Self::Object(fields) => {
                out.push_str("{\n");
                for (index, (key, value)) in fields.iter().enumerate() {
                    let _ = write!(out, "{indent}\"{key}\": ");
                    value.write(out, depth + 1);
                    out.push_str(if index + 1 < fields.len() {
                        ",\n"
                    } else {
                        "\n"
                    });
                }
                let _ = write!(out, "{close}}}");
            }
            Self::String(text) => {
                out.push('"');
                for c in text.chars() {
                    match c {
                        '"' => out.push_str("\\\""),
                        '\\' => out.push_str("\\\\"),
                        '\n' => out.push_str("\\n"),
                        '\r' => out.push_str("\\r"),
                        '\t' => out.push_str("\\t"),
                        c if u32::from(c) < 0x20 => {
                            let _ = write!(out, "\\u{:04x}", u32::from(c));
                        }
                        c => out.push(c),
                    }
                }
                out.push('"');
            }
        }
    }
}
