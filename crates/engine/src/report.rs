//! Reports rendered from scored categories: the terminal summary, `report.json`, and
//! `report.md`. Rendering is deterministic, with no timestamps or paths, so the same scores always
//! produce the same bytes.

use std::fmt::Write;

use crate::config::{Better, Rollup};
use crate::format;
use crate::score::{CategoryScore, MetricScore};
use crate::value::{Kind, Value};

/// How many of a list metric's worst items the terminal and Markdown reports show.
const WORST: usize = 3;

/// Everything one run measured.
#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    /// Categories in the order they ran.
    pub categories: Vec<CategoryReport>,
}

/// One category's scores, or the reason it has none.
#[derive(Clone, Debug, PartialEq)]
pub struct CategoryReport {
    /// The category name.
    pub name: String,
    /// The category's scores, or why its script produced none.
    pub result: Result<CategoryScore, Failure>,
}

/// Why a category has no scores.
#[derive(Clone, Debug, PartialEq)]
pub struct Failure {
    /// Lines that explain the failure, such as the end of standard error. May be empty.
    pub details: String,
    /// A one-line reason, such as `script exited with status 1`.
    pub message: String,
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
            let _ = writeln!(out, "✗ {}  {}", category.name, failure.message);
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
    let _ = writeln!(out, "{}  {}{rounds}", cell(scored.score), category.name);

    let rows: Vec<(&MetricScore, String)> = scored
        .metrics
        .iter()
        .map(|metric| (metric, value(metric)))
        .collect();
    let name_width = rows
        .iter()
        .map(|(metric, _)| metric.metric.name.chars().count())
        .max()
        .unwrap_or_default();
    let value_width = rows
        .iter()
        .map(|(_, value)| value.chars().count())
        .max()
        .unwrap_or_default();

    for (metric, value) in &rows {
        let line = format!(
            "  {}  {:name_width$}  {value:value_width$}  {}",
            cell(metric.score),
            metric.metric.name,
            target(metric)
        );
        let _ = writeln!(out, "{}", line.trim_end());

        let items: Vec<(String, &str)> = worst(metric)
            .into_iter()
            .map(|item| (format::value(item.value), item.name.as_str()))
            .collect();
        let item_width = items
            .iter()
            .map(|(value, _)| value.chars().count())
            .max()
            .unwrap_or_default();
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

/// Renders the report as JSON. Values are in base units: bytes for sizes and nanoseconds for
/// times. A check's value is the share of items that pass, from 0 to 1.
pub fn json(report: &Report) -> String {
    let categories = report.categories.iter().map(|category| {
        let name = ("name", Json::String(category.name.clone()));

        match &category.result {
            Err(failure) => Json::Object(vec![
                name,
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
                (
                    "score",
                    scored
                        .score
                        .map_or(Json::Null, |s| Json::Number(f64::from(s))),
                ),
                ("rounds", Json::Number(count(scored.rounds))),
                (
                    "metrics",
                    Json::Array(scored.metrics.iter().map(metric_json).collect()),
                ),
            ]),
        }
    });

    let mut out = String::new();
    Json::Object(vec![
        ("version", Json::Number(1.0)),
        ("categories", Json::Array(categories.collect())),
    ])
    .write(&mut out, 0);
    out.push('\n');
    out
}

/// Renders the report as Markdown: a summary table of categories, then a section per category
/// with a table of metrics and the worst items of each list.
pub fn markdown(report: &Report) -> String {
    let mut out = String::from("# Retros report\n\n| Category | Score |\n| --- | --- |\n");

    for category in &report.categories {
        let score = match &category.result {
            Ok(scored) => compact(scored.score),
            Err(failure) => format!("✗ {}", failure.message),
        };
        let _ = writeln!(out, "| {} | {} |", escape(&category.name), escape(&score));
    }

    for category in &report.categories {
        let _ = write!(out, "\n## {}\n\n", category.name);

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
                "Timings are the median of {} rounds.\n\n",
                scored.rounds
            );
        }

        out.push_str("| Score | Metric | Value | Target |\n| --- | --- | --- | --- |\n");
        for metric in &scored.metrics {
            let target = metric.metric.target.map(format::value).unwrap_or_default();
            let _ = writeln!(
                out,
                "| {} | {} | {} | {target} |",
                compact(metric.score),
                escape(&metric.metric.name),
                escape(&value(metric)),
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
                "\n{heading} of `{}`:\n\n| Item | Value |\n| --- | --- |\n",
                metric.metric.name
            );
            for item in items {
                let _ = writeln!(
                    out,
                    "| {} | {} |",
                    escape(&item.name),
                    format::value(item.value)
                );
            }
        }
    }

    out
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

/// Formats a metric's collapsed value, such as `27.5 kB`, `p90 9 kB`, or `2 of 3 pass`.
fn value(metric: &MetricScore) -> String {
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

    format!("{} {amount}", rollup(metric.metric.score))
}

/// Formats a metric's target and any weight other than 1, such as `target 5 kB, weight 2`.
fn target(metric: &MetricScore) -> String {
    let mut parts = Vec::new();

    if let Some(target) = metric.metric.target {
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
fn worst(metric: &MetricScore) -> Vec<&crate::score::Item> {
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
        let value = match item.value {
            Value::Check(passed) => Json::Bool(passed),
            other => Json::Number(other.amount().unwrap_or_default()),
        };
        Json::Object(vec![
            ("name", Json::String(item.name.clone())),
            ("value", value),
        ])
    });

    Json::Object(vec![
        ("name", Json::String(metric.metric.name.clone())),
        ("kind", Json::String(kind.to_owned())),
        ("unit", unit),
        ("value", Json::Number(metric.value)),
        (
            "score",
            metric
                .score
                .map_or(Json::Null, |s| Json::Number(f64::from(s))),
        ),
        (
            "target",
            metric
                .metric
                .target
                .and_then(Value::amount)
                .map_or(Json::Null, Json::Number),
        ),
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

fn rollup(rollup: Rollup) -> &'static str {
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

#[expect(clippy::cast_precision_loss, reason = "round counts never reach 2^53")]
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
