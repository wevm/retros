//! The `retros.toml` configuration.
//!
//! Each top-level table is a category. A category's `run` key is the command that measures it,
//! `rounds` and `timeout` are optional, and every other key is a metric. A metric is a table of
//! optional keys: `target`, `limit`, `weight`, `score`, and `better`.
//!
//! ```toml
//! [footprint]
//! run = "node metrics/footprint.ts"
//! export-size = { target = "5kB", limit = "50kB" }
//! ```

use std::borrow::Cow;
use std::time::Duration;

use toml::Spanned;
use toml::de::{DeTable, DeValue};

use crate::error::{Code, Error, Location};
use crate::lines::is_name;
use crate::value::{self, Value};

/// Keys a metric table accepts, in the order error messages list them.
const METRIC_KEYS: [&str; 5] = ["better", "limit", "score", "target", "weight"];

/// Rounds a timing metric runs when its category sets no `rounds`.
const DEFAULT_ROUNDS: u32 = 5;

/// Time a script may run when its category sets no `timeout`.
const DEFAULT_TIMEOUT: Duration = Duration::from_mins(10);

/// A parsed `retros.toml`.
#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    /// Categories in the order the file declares them.
    pub categories: Vec<Category>,
}

/// A category: one script and the metrics it writes.
#[derive(Clone, Debug, PartialEq)]
pub struct Category {
    /// The position of the category's name.
    pub location: Location,
    /// Metrics in the order the file declares them.
    pub metrics: Vec<Metric>,
    /// The category name, such as `footprint`.
    pub name: String,
    /// How many times timing metrics run. Defaults to 5.
    pub rounds: u32,
    /// The shell command that runs the script.
    pub run: String,
    /// How long one run may take before Retros stops it. Defaults to 10 minutes.
    pub timeout: Duration,
}

/// A configured metric.
#[derive(Clone, Debug, PartialEq)]
pub struct Metric {
    /// The values that score 90 and 50, when the metric is scored.
    pub anchors: Option<Anchors>,
    /// Which direction is better. Defaults to lower.
    pub better: Better,
    /// The position of the metric's name.
    pub location: Location,
    /// The metric name, such as `export-size`.
    pub name: String,
    /// How a list of items collapses to one value. Defaults to `p90`, or `p10` when higher is
    /// better.
    pub score: Rollup,
    /// The metric's weight in its category. Defaults to 1.
    pub weight: f64,
}

/// The two values that place a metric on the scoring curve.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Anchors {
    /// The value that scores 50. Past it, the metric is poor.
    pub limit: Value,
    /// The value that scores 90. Meeting it puts the metric in the good band.
    pub target: Value,
}

/// The direction in which a metric improves.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Better {
    /// Larger values are better, such as throughput.
    Higher,
    /// Smaller values are better, such as size and time.
    Lower,
}

/// How a metric with items collapses to one value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Rollup {
    /// The largest item.
    Max,
    /// The middle item.
    Median,
    /// The smallest item.
    Min,
    /// The 10th percentile, the bad tail when higher is better.
    P10,
    /// The 90th percentile, the bad tail when lower is better.
    P90,
    /// The total of every item.
    Sum,
}

/// Parses `retros.toml` source text.
///
/// # Errors
///
/// Returns the first error: TOML syntax, an unknown key, a value of the wrong type, a missing
/// `run` command, or anchors that cannot score.
pub fn parse(source: &str) -> Result<Config, Error> {
    let document = DeTable::parse(source).map_err(|error| Error {
        code: Code::Toml,
        location: error.span().map(|span| Location::of(source, span.start)),
        message: error.message().to_owned(),
    })?;

    let mut categories = Vec::new();

    for (key, value) in document.get_ref() {
        let name = key.get_ref().as_ref();
        check_name(source, key, "category")?;

        let DeValue::Table(table) = value.get_ref() else {
            return Err(Error::at(
                Code::InvalidType,
                source,
                key.span().start,
                format!("top-level key `{name}` must be a category table, such as `[{name}]`"),
            ));
        };

        categories.push(category(source, key, table)?);
    }

    if categories.is_empty() {
        return Err(Error {
            code: Code::NothingConfigured,
            location: None,
            message: "no categories. Add a table such as `[footprint]` with a `run` command"
                .to_owned(),
        });
    }

    Ok(Config { categories })
}

fn category(
    source: &str,
    key: &Spanned<Cow<'_, str>>,
    table: &DeTable<'_>,
) -> Result<Category, Error> {
    let name = key.get_ref().as_ref();
    let mut metrics = Vec::new();
    let mut rounds = DEFAULT_ROUNDS;
    let mut run = None;
    let mut timeout = DEFAULT_TIMEOUT;

    for (field, value) in table {
        match field.get_ref().as_ref() {
            "rounds" => rounds = positive_integer(source, value, "rounds")?,
            "run" => {
                let command = string(source, value, "run")?;
                if command.trim().is_empty() {
                    return Err(Error::at(
                        Code::InvalidValue,
                        source,
                        value.span().start,
                        "`run` must be a command".to_owned(),
                    ));
                }
                run = Some(command.to_owned());
            }
            "timeout" => timeout = duration(source, value)?,
            metric => {
                check_name(source, field, "metric")?;

                let DeValue::Table(fields) = value.get_ref() else {
                    return Err(Error::at(
                        Code::InvalidType,
                        source,
                        value.span().start,
                        format!(
                            "metric `{metric}` must be a table, such as `{metric} = {{ target = \"5kB\", limit = \"50kB\" }}` or `{metric} = {{}}`"
                        ),
                    ));
                };

                metrics.push(self::metric(source, field, fields)?);
            }
        }
    }

    let Some(run) = run else {
        return Err(Error::at(
            Code::RunMissing,
            source,
            key.span().start,
            format!("category `{name}` has no `run` command"),
        ));
    };

    if metrics.is_empty() {
        return Err(Error::at(
            Code::NothingConfigured,
            source,
            key.span().start,
            format!("category `{name}` declares no metrics"),
        ));
    }

    Ok(Category {
        location: Location::of(source, key.span().start),
        metrics,
        name: name.to_owned(),
        rounds,
        run,
        timeout,
    })
}

fn metric(source: &str, key: &Spanned<Cow<'_, str>>, table: &DeTable<'_>) -> Result<Metric, Error> {
    let name = key.get_ref().as_ref();
    let mut better = Better::Lower;
    let mut limit = None;
    let mut score = None;
    let mut target = None;
    let mut weight = 1.0;

    for (field, value) in table {
        match field.get_ref().as_ref() {
            "better" => {
                better = match string(source, value, "better")? {
                    "higher" => Better::Higher,
                    "lower" => Better::Lower,
                    other => {
                        return Err(Error::at(
                            Code::InvalidValue,
                            source,
                            value.span().start,
                            format!(
                                "unknown `better` value `{other}`, expected `lower` or `higher`"
                            ),
                        ));
                    }
                };
            }
            "limit" => limit = Some((anchor(source, value, "limit")?, value.span().start)),
            "score" => {
                score = Some(match string(source, value, "score")? {
                    "max" => Rollup::Max,
                    "median" => Rollup::Median,
                    "min" => Rollup::Min,
                    "p10" => Rollup::P10,
                    "p90" => Rollup::P90,
                    "sum" => Rollup::Sum,
                    other => {
                        return Err(Error::at(
                            Code::InvalidValue,
                            source,
                            value.span().start,
                            format!(
                                "unknown `score` value `{other}`, expected one of `p90`, `p10`, `median`, `max`, `min`, `sum`"
                            ),
                        ));
                    }
                });
            }
            "target" => target = Some((anchor(source, value, "target")?, value.span().start)),
            "weight" => weight = non_negative(source, value, "weight")?,
            other => {
                let expected: Vec<String> = METRIC_KEYS.iter().map(|k| format!("`{k}`")).collect();
                return Err(Error::at(
                    Code::UnexpectedKey,
                    source,
                    field.span().start,
                    format!(
                        "unknown key `{other}` in metric `{name}`, expected one of {}",
                        expected.join(", ")
                    ),
                ));
            }
        }
    }

    let anchors = match (target, limit) {
        (None, None) => None,
        (Some((target, target_at)), Some((limit, _))) => {
            Some(anchors(source, name, better, target, limit, target_at)?)
        }
        (Some(_), None) | (None, Some(_)) => {
            return Err(Error::at(
                Code::IncompleteAnchors,
                source,
                key.span().start,
                format!("metric `{name}` needs both `target` and `limit` to be scored, or neither"),
            ));
        }
    };

    Ok(Metric {
        anchors,
        better,
        location: Location::of(source, key.span().start),
        name: name.to_owned(),
        score: score.unwrap_or(match better {
            Better::Higher => Rollup::P10,
            Better::Lower => Rollup::P90,
        }),
        weight,
    })
}

/// Checks that two anchors share a unit, are positive, and run from `target` to `limit` in the
/// worse direction.
fn anchors(
    source: &str,
    name: &str,
    better: Better,
    target: Value,
    limit: Value,
    offset: usize,
) -> Result<Anchors, Error> {
    let error = |code, message| Err(Error::at(code, source, offset, message));

    let (target_amount, limit_amount) = match (target, limit) {
        (Value::Count(t), Value::Count(l))
        | (Value::Size(t), Value::Size(l))
        | (Value::Time(t), Value::Time(l)) => (t, l),
        (Value::Check(_), _) | (_, Value::Check(_)) => {
            return error(
                Code::AnchorUnit,
                format!(
                    "metric `{name}` cannot have check anchors. A check scores the share of items that pass"
                ),
            );
        }
        _ => {
            return error(
                Code::AnchorUnit,
                format!(
                    "the anchors of metric `{name}` mix {} and {}",
                    target.kind().plural(),
                    limit.kind().plural()
                ),
            );
        }
    };

    if target_amount <= 0.0 || limit_amount <= 0.0 {
        return error(
            Code::AnchorRange,
            format!("the anchors of metric `{name}` must be greater than zero"),
        );
    }

    let ordered = match better {
        Better::Higher => target_amount > limit_amount,
        Better::Lower => target_amount < limit_amount,
    };
    if !ordered {
        let (relation, hint) = match better {
            Better::Higher => ("greater", ""),
            Better::Lower => (
                "smaller",
                ". Set `better = \"higher\"` for a metric where larger is better",
            ),
        };
        return error(
            Code::AnchorOrder,
            format!("the `target` of metric `{name}` must be {relation} than its `limit`{hint}"),
        );
    }

    Ok(Anchors { limit, target })
}

/// Parses an anchor: a value string such as `"5kB"`, or a TOML number for a count.
fn anchor(source: &str, value: &Spanned<DeValue<'_>>, key: &str) -> Result<Value, Error> {
    match value.get_ref() {
        DeValue::Integer(_) | DeValue::Float(_) => Ok(Value::Count(number(source, value, key)?)),
        DeValue::String(text) => {
            let content = content_offset(source, value.span().start);
            value::parse(text).map_err(|error| {
                Error::at(error.code, source, content + error.offset, error.message)
            })
        }
        _ => Err(Error::at(
            Code::InvalidType,
            source,
            value.span().start,
            format!("`{key}` must be a value such as \"5kB\", \"20ms\", or a number"),
        )),
    }
}

/// Parses a duration such as `"10m"`, `"30s"`, or `"500ms"`.
fn duration(source: &str, value: &Spanned<DeValue<'_>>) -> Result<Duration, Error> {
    /// Duration units and their factors in nanoseconds.
    const UNITS: [(&str, f64); 4] = [("ms", 1e6), ("s", 1e9), ("m", 6e10), ("h", 3.6e12)];

    let text = string(source, value, "timeout")?;
    let invalid = || {
        Error::at(
            Code::InvalidValue,
            source,
            value.span().start,
            format!(
                "invalid `timeout` `{text}`, expected a duration such as \"30s\", \"10m\", or \"500ms\""
            ),
        )
    };

    let (number, unit) = value::split(text).ok_or_else(invalid)?;
    let (_, factor) = UNITS
        .iter()
        .find(|(name, _)| *name == unit)
        .ok_or_else(invalid)?;
    let nanos = number.times(*factor).round();

    if !(1.0..=1e18).contains(&nanos) {
        return Err(invalid());
    }

    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the value is rounded and between 1 and 1e18, inside u64"
    )]
    Ok(Duration::from_nanos(nanos as u64))
}

fn check_name(source: &str, key: &Spanned<Cow<'_, str>>, what: &str) -> Result<(), Error> {
    let name = key.get_ref().as_ref();
    if is_name(name) {
        return Ok(());
    }

    Err(Error::at(
        Code::InvalidName,
        source,
        key.span().start,
        format!("invalid {what} name `{name}`. Names use ASCII letters, digits, `-`, `_`, and `.`"),
    ))
}

/// Returns the byte offset of a string's content, after its opening quotes.
fn content_offset(source: &str, start: usize) -> usize {
    let quotes = source.get(start..).map_or(0, |rest| {
        rest.bytes()
            .take(3)
            .take_while(|b| matches!(b, b'"' | b'\''))
            .count()
    });
    start + quotes
}

fn non_negative(source: &str, value: &Spanned<DeValue<'_>>, key: &str) -> Result<f64, Error> {
    let amount = number(source, value, key)?;
    if amount < 0.0 {
        return Err(Error::at(
            Code::InvalidValue,
            source,
            value.span().start,
            format!("`{key}` cannot be negative"),
        ));
    }
    Ok(amount)
}

fn number(source: &str, value: &Spanned<DeValue<'_>>, key: &str) -> Result<f64, Error> {
    let parsed = match value.get_ref() {
        DeValue::Float(float) => float.as_str().replace('_', "").parse::<f64>().ok(),
        DeValue::Integer(integer) if integer.radix() == 10 => {
            integer.as_str().replace('_', "").parse::<f64>().ok()
        }
        _ => None,
    };

    parsed.filter(|n| n.is_finite()).ok_or_else(|| {
        Error::at(
            Code::InvalidType,
            source,
            value.span().start,
            format!("`{key}` must be a finite number"),
        )
    })
}

fn positive_integer(source: &str, value: &Spanned<DeValue<'_>>, key: &str) -> Result<u32, Error> {
    let parsed = match value.get_ref() {
        DeValue::Integer(integer) => {
            u32::from_str_radix(&integer.as_str().replace('_', ""), integer.radix()).ok()
        }
        _ => None,
    };

    parsed.filter(|n| *n > 0).ok_or_else(|| {
        Error::at(
            Code::InvalidValue,
            source,
            value.span().start,
            format!("`{key}` must be a whole number greater than zero"),
        )
    })
}

fn string<'a>(source: &str, value: &'a Spanned<DeValue<'_>>, key: &str) -> Result<&'a str, Error> {
    match value.get_ref() {
        DeValue::String(text) => Ok(text),
        _ => Err(Error::at(
            Code::InvalidType,
            source,
            value.span().start,
            format!("`{key}` must be a string"),
        )),
    }
}
