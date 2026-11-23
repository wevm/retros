//! The configuration: `retros.yaml` or `retros.toml`, which describe the same categories.
//!
//! Each top-level key is a category. A category's `run` key is the command that measures it,
//! `timeout` and `description` are optional, and every other key is a metric. A top-level `run`
//! and `timeout` apply to every category that sets none, and categories with the same command
//! share one run of it. A top-level `setup` command prepares each checkout that
//! `retros --compare` creates. A metric is a mapping of optional keys: `target`, `targets`,
//! `weight`, `score`, `better`, `varies`, `title`, `description`, and `baseline`, which
//! `retros --update` writes. `target` is the default for every item, a metric line can set its
//! own, and `targets` sets targets by item pattern and has the final say.
//!
//! ```yaml
//! footprint:
//!   run: bun metrics/footprint.ts
//!   export-size: { target: 5kB, targets: { viem: 60kB, "viem/chains/*": 1kB } }
//! ```
//!
//! ```toml
//! [footprint]
//! run = "bun metrics/footprint.ts"
//! export-size = { target = "5kB", targets = { "viem" = "60kB", "viem/chains/*" = "1kB" } }
//! ```

use std::time::Duration;

use yaml_rust2::Yaml;

use crate::baseline::Baseline;
use crate::document::{self, Kind, Node};
use crate::error::{Code, Error, Location};
use crate::lines::is_name;
use crate::value::{self, Value};

/// Keys a metric accepts, in the order error messages list them.
const METRIC_KEYS: [&str; 9] = [
    "baseline",
    "better",
    "description",
    "score",
    "target",
    "targets",
    "title",
    "varies",
    "weight",
];

/// Time a script may run when its category sets no `timeout`.
const DEFAULT_TIMEOUT: Duration = Duration::from_mins(10);

/// The format of a configuration file.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Format {
    /// `retros.toml`.
    Toml,
    /// `retros.yaml`.
    Yaml,
}

impl Format {
    /// Returns the example for `format`: `yaml` for YAML and `toml` for TOML.
    pub(crate) fn pick(self, yaml: String, toml: String) -> String {
        match self {
            Self::Toml => toml,
            Self::Yaml => yaml,
        }
    }
}

/// A parsed configuration.
#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    /// Categories in the order the file declares them.
    pub categories: Vec<Category>,
    /// The command that prepares a checkout `retros --compare` creates, such as installing
    /// packages, or `None` when it needs nothing.
    pub setup: Option<Setup>,
}

/// The top-level `setup` command.
#[derive(Clone, Debug, PartialEq)]
pub struct Setup {
    /// The shell command, run once from the project root of each new checkout.
    pub run: String,
    /// How long it may take before Retros stops it: the top-level `timeout`, or 10 minutes.
    pub timeout: Duration,
}

/// Returns whether two configurations measure alike: equal apart from baselines and positions in
/// the file.
pub fn alike(a: &Config, b: &Config) -> bool {
    let plain = |config: &Config| {
        let mut config = without_locations(config.clone());
        for metric in config.categories.iter_mut().flat_map(|c| &mut c.metrics) {
            metric.baseline = None;
        }
        config
    };
    plain(a) == plain(b)
}

/// Returns `config` with every location cleared, to compare it with the same configuration after
/// lines moved.
pub(crate) fn without_locations(mut config: Config) -> Config {
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

/// A category: one script and the metrics it writes.
#[derive(Clone, Debug, PartialEq)]
pub struct Category {
    /// What the category measures, for reports.
    pub description: Option<String>,
    /// The position of the category's name.
    pub location: Location,
    /// Metrics in the order the file declares them.
    pub metrics: Vec<Metric>,
    /// The category name, such as `footprint`.
    pub name: String,
    /// The shell command that runs the script. Categories with the same command share one run.
    pub run: String,
    /// How long one run may take before Retros stops it. Defaults to 10 minutes.
    pub timeout: Duration,
    /// The name reports show, such as `Best Practices`.
    pub title: Option<String>,
}

impl Category {
    /// Returns the name reports show: the `title`, or else the name in words, such as `Speed`.
    pub fn title(&self) -> String {
        self.title.clone().unwrap_or_else(|| words(&self.name))
    }
}

/// A configured metric.
#[derive(Clone, Debug, PartialEq)]
pub struct Metric {
    /// The sizes, counts, checks, and line targets that `retros --update` last accepted, or `None`
    /// before the first update.
    pub baseline: Option<Baseline>,
    /// Which direction is better. Defaults to lower.
    pub better: Better,
    /// What the metric measures, for reports.
    pub description: Option<String>,
    /// The position of the metric's name.
    pub location: Location,
    /// The metric name, such as `export-size`.
    pub name: String,
    /// How a list of items collapses to one value. Defaults to `p90`, or `p10` when higher is
    /// better.
    pub score: Rollup,
    /// The value each item must meet, which scores 90, unless its line or `targets` sets another.
    /// A metric without any target is shown but not scored.
    pub target: Option<Value>,
    /// Targets by item pattern, in the order the file declares them. The first pattern that
    /// matches an item sets its target, over the item's line and the default.
    pub targets: Vec<Pattern>,
    /// The name reports show, such as `Time to first token`.
    pub title: Option<String>,
    /// Whether a size or count differs between runs, such as peak memory. Like a time, it takes
    /// its median across rounds, and its baseline records only line targets. Defaults to false.
    pub varies: bool,
    /// The metric's weight in its category. Defaults to 1.
    pub weight: f64,
}

impl Metric {
    /// Returns the first pattern in `targets` that matches `item`.
    pub fn pattern(&self, item: &str) -> Option<&Pattern> {
        self.targets.iter().find(|pattern| pattern.matches(item))
    }

    /// Returns the name reports show: the `title`, or else the name in words, such as
    /// `Request out` for `request-out`.
    pub fn title(&self) -> String {
        self.title.clone().unwrap_or_else(|| words(&self.name))
    }
}

/// Writes a name in words for a report without a `title`: `-` and `_` become spaces, and the
/// first letter is upper case, so `first-token` reads `First token`.
pub(crate) fn words(name: &str) -> String {
    let spaced = name.replace(['-', '_'], " ");
    let mut chars = spaced.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_ascii_uppercase().to_string() + chars.as_str()
    })
}

/// An item pattern and the target it sets, such as `"viem/chains/*": 1kB`.
#[derive(Clone, Debug, PartialEq)]
pub struct Pattern {
    /// The position of the pattern.
    pub location: Location,
    /// The pattern. `*` matches any run of characters, and every other character matches itself.
    pub pattern: String,
    /// The target of every item the pattern matches.
    pub target: Value,
}

impl Pattern {
    /// Returns whether the pattern matches an item name.
    pub fn matches(&self, item: &str) -> bool {
        let mut parts = self.pattern.split('*');
        let Some(mut rest) = item.strip_prefix(parts.next().unwrap_or_default()) else {
            return false;
        };
        let parts: Vec<&str> = parts.collect();
        let Some((last, middle)) = parts.split_last() else {
            return rest.is_empty();
        };

        // Each middle part matches at its first occurrence, which leaves the most room for the
        // parts after it.
        for part in middle {
            match rest.find(part) {
                Some(at) => rest = &rest[at + part.len()..],
                None => return false,
            }
        }

        rest.ends_with(last)
    }
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

/// Parses the source text of a configuration in `format`.
///
/// # Errors
///
/// Returns the first error: YAML or TOML syntax, an unknown key, a value of the wrong type, a missing
/// `run` command, a target that cannot score, or a metric in two categories that share a run.
pub fn parse(source: &str, format: Format) -> Result<Config, Error> {
    let Some(document) = document::parse(source, format)? else {
        return Err(nothing_configured(format));
    };
    let Kind::Mapping(pairs) = &document.kind else {
        return Err(Error::at(
            Code::InvalidType,
            source,
            document.offset,
            "retros.yaml must map category names to categories, such as `footprint:`".to_owned(),
        ));
    };

    // Top-level `run` and `timeout` are defaults for every category.
    let mut defaults = Defaults {
        run: None,
        timeout: DEFAULT_TIMEOUT,
    };
    let mut setup = None;
    for (key, value) in pairs {
        match self::key(source, key)? {
            "run" => defaults.run = Some(command(source, value, "run")?),
            "setup" => setup = Some(command(source, value, "setup")?),
            "timeout" => defaults.timeout = duration(source, value)?,
            _ => {}
        }
    }

    let mut categories: Vec<Category> = Vec::new();

    for (key, value) in pairs {
        let name = self::key(source, key)?;
        if matches!(name, "run" | "setup" | "timeout") {
            continue;
        }
        check_name(source, key, name, "category")?;

        let Kind::Mapping(fields) = &value.kind else {
            return Err(Error::at(
                Code::InvalidType,
                source,
                key.offset,
                format.pick(
                    format!(
                        "top-level key `{name}` must be a category: a mapping with a `run` command and metrics"
                    ),
                    format!("top-level key `{name}` must be a category table, such as `[{name}]`"),
                ),
            ));
        };

        let category = category(source, format, key, name, fields, &defaults)?;

        // A shared run gives each line to the category that configures its metric, so a metric
        // name can belong to only one of the categories that share it.
        for other in categories.iter().filter(|other| other.run == category.run) {
            if let Some(metric) = category
                .metrics
                .iter()
                .find(|metric| other.metrics.iter().any(|m| m.name == metric.name))
            {
                return Err(Error {
                    code: Code::DuplicateMetric,
                    location: Some(metric.location),
                    message: format!(
                        "metric `{}` is in `{}` and `{}`, which share the same `run`, so Retros cannot tell which category its lines belong to",
                        metric.name, other.name, category.name
                    ),
                });
            }
        }

        categories.push(category);
    }

    if categories.is_empty() {
        return Err(nothing_configured(format));
    }

    Ok(Config {
        categories,
        setup: setup.map(|run| Setup {
            run,
            timeout: defaults.timeout,
        }),
    })
}

/// The top-level keys that apply to every category that does not set its own.
struct Defaults {
    run: Option<String>,
    timeout: Duration,
}

fn nothing_configured(format: Format) -> Error {
    Error {
        code: Code::NothingConfigured,
        location: None,
        message: format.pick(
            "no categories. Add one such as `footprint:` with a `run` command and a metric"
                .to_owned(),
            "no categories. Add a table such as `[footprint]` with a `run` command".to_owned(),
        ),
    }
}

fn category(
    source: &str,
    format: Format,
    key: &Node,
    name: &str,
    fields: &[(Node, Node)],
    defaults: &Defaults,
) -> Result<Category, Error> {
    let mut description = None;
    let mut metrics = Vec::new();
    let mut run = None;
    let mut timeout = defaults.timeout;
    let mut title = None;

    for (field, value) in fields {
        match self::key(source, field)? {
            "description" => description = Some(string(source, value, "description")?.to_owned()),
            "run" => run = Some(command(source, value, "run")?),
            "timeout" => timeout = duration(source, value)?,
            "title" => title = Some(string(source, value, "title")?.to_owned()),
            metric => {
                check_name(source, field, metric, "metric")?;

                // A metric without keys, such as `modules:`, is shown but not scored.
                let empty = Vec::new();
                let keys = match &value.kind {
                    Kind::Mapping(keys) => keys,
                    _ if value.is_null() => &empty,
                    _ => {
                        return Err(Error::at(
                            Code::InvalidType,
                            source,
                            value.offset,
                            format.pick(
                                format!(
                                    "metric `{metric}` must be a mapping, such as `{metric}: {{ target: 5kB }}` or `{metric}: {{}}`"
                                ),
                                format!(
                                    "metric `{metric}` must be a table, such as `{metric} = {{ target = \"5kB\" }}` or `{metric} = {{}}`"
                                ),
                            ),
                        ));
                    }
                };

                metrics.push(self::metric(source, format, field, metric, keys)?);
            }
        }
    }

    let Some(run) = run.or_else(|| defaults.run.clone()) else {
        return Err(Error::at(
            Code::RunMissing,
            source,
            key.offset,
            format!(
                "category `{name}` has no `run` command. Add one, or a top-level `run` that every category shares"
            ),
        ));
    };

    if metrics.is_empty() {
        return Err(Error::at(
            Code::NothingConfigured,
            source,
            key.offset,
            format!("category `{name}` declares no metrics"),
        ));
    }

    Ok(Category {
        description,
        location: Location::of(source, key.offset),
        metrics,
        name: name.to_owned(),
        run,
        timeout,
        title,
    })
}

fn metric(
    source: &str,
    format: Format,
    key: &Node,
    name: &str,
    keys: &[(Node, Node)],
) -> Result<Metric, Error> {
    let mut baseline = None;
    let mut better = Better::Lower;
    let mut description = None;
    let mut score = None;
    let mut target = None;
    let mut targets = Vec::new();
    let mut title = None;
    let mut varies = false;
    let mut weight = 1.0;

    for (field, value) in keys {
        match self::key(source, field)? {
            "baseline" => baseline = Some(crate::baseline::parse(source, value)?),
            "better" => {
                better = match string(source, value, "better")? {
                    "higher" => Better::Higher,
                    "lower" => Better::Lower,
                    other => {
                        return Err(Error::at(
                            Code::InvalidValue,
                            source,
                            value.offset,
                            format!(
                                "unknown `better` value `{other}`, expected `lower` or `higher`"
                            ),
                        ));
                    }
                };
            }
            "description" => description = Some(string(source, value, "description")?.to_owned()),
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
                            value.offset,
                            format!(
                                "unknown `score` value `{other}`, expected one of `p90`, `p10`, `median`, `max`, `min`, `sum`"
                            ),
                        ));
                    }
                });
            }
            "target" => target = Some(self::target(source, name, value)?),
            "targets" => {
                let Kind::Mapping(patterns) = &value.kind else {
                    return Err(Error::at(
                        Code::InvalidType,
                        source,
                        value.offset,
                        format.pick(
                            "`targets` must map item patterns to targets, such as `targets: { \"viem/chains/*\": 1kB }`".to_owned(),
                            "`targets` must be a table of item patterns and their targets, such as `targets = { \"viem/chains/*\" = \"1kB\" }`".to_owned(),
                        ),
                    ));
                };

                for (pattern, value) in patterns {
                    targets.push(Pattern {
                        location: Location::of(source, pattern.offset),
                        pattern: self::key(source, pattern)?.to_owned(),
                        target: self::target(source, name, value)?,
                    });
                }
            }
            "title" => title = Some(string(source, value, "title")?.to_owned()),
            "varies" => {
                let flag = match &value.kind {
                    Kind::Scalar { plain: true, text } => match Yaml::from_str(text) {
                        Yaml::Boolean(flag) => Some(flag),
                        _ => None,
                    },
                    _ => None,
                };
                varies = flag.ok_or_else(|| {
                    Error::at(
                        Code::InvalidType,
                        source,
                        value.offset,
                        "`varies` must be `true` or `false`".to_owned(),
                    )
                })?;
            }
            "weight" => weight = non_negative(source, value, "weight")?,
            other => {
                let expected: Vec<String> = METRIC_KEYS.iter().map(|k| format!("`{k}`")).collect();
                return Err(Error::at(
                    Code::UnexpectedKey,
                    source,
                    field.offset,
                    format!(
                        "unknown key `{other}` in metric `{name}`, expected one of {}",
                        expected.join(", ")
                    ),
                ));
            }
        }
    }

    Ok(Metric {
        baseline,
        better,
        description,
        location: Location::of(source, key.offset),
        name: name.to_owned(),
        score: score.unwrap_or(match better {
            Better::Higher => Rollup::P10,
            Better::Lower => Rollup::P90,
        }),
        target,
        targets,
        title,
        varies,
        weight,
    })
}

/// Parses a target: a value such as `5kB` or `20ms`, or a number for a count. A target must be
/// greater than zero, and a check cannot have one.
fn target(source: &str, name: &str, value: &Node) -> Result<Value, Error> {
    let at = value.offset;
    let target = match &value.kind {
        Kind::Scalar { plain: true, text } if is_number(text) => {
            Value::Count(number(source, value, "target")?)
        }
        Kind::Scalar { text, .. } if !value.is_null() => {
            let content = content_offset(source, at);
            value::parse(text).map_err(|error| {
                Error::at(error.code, source, content + error.offset, error.message)
            })?
        }
        _ => {
            return Err(Error::at(
                Code::InvalidType,
                source,
                at,
                "`target` must be a value such as `5kB`, `20ms`, or a number".to_owned(),
            ));
        }
    };

    match target.amount() {
        None => Err(Error::at(
            Code::TargetUnit,
            source,
            at,
            format!(
                "metric `{name}` cannot have a check as its target. A check scores the share of items that pass"
            ),
        )),
        Some(amount) if amount <= 0.0 => Err(Error::at(
            Code::TargetRange,
            source,
            at,
            format!("the `target` of metric `{name}` must be greater than zero"),
        )),
        Some(_) => Ok(target),
    }
}

/// Parses a `run` or `setup` command, which must not be blank.
fn command(source: &str, value: &Node, key: &str) -> Result<String, Error> {
    let command = string(source, value, key)?;
    if command.trim().is_empty() {
        return Err(Error::at(
            Code::InvalidValue,
            source,
            value.offset,
            format!("`{key}` must be a command"),
        ));
    }
    Ok(command.to_owned())
}

/// Parses a duration such as `10m`, `30s`, or `500ms`.
fn duration(source: &str, value: &Node) -> Result<Duration, Error> {
    /// Duration units and their factors in nanoseconds.
    const UNITS: [(&str, f64); 4] = [("ms", 1e6), ("s", 1e9), ("m", 6e10), ("h", 3.6e12)];

    let text = string(source, value, "timeout")?;
    let invalid = || {
        Error::at(
            Code::InvalidValue,
            source,
            value.offset,
            format!(
                "invalid `timeout` `{text}`, expected a duration such as `30s`, `10m`, or `500ms`"
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

/// Returns a mapping key's text. Keys are names, so a mapping or a sequence cannot be one.
fn key<'n>(source: &str, key: &'n Node) -> Result<&'n str, Error> {
    key.text().ok_or_else(|| {
        Error::at(
            Code::InvalidType,
            source,
            key.offset,
            "a key must be a name, not a mapping or a list".to_owned(),
        )
    })
}

fn check_name(source: &str, key: &Node, name: &str, what: &str) -> Result<(), Error> {
    if is_name(name) {
        return Ok(());
    }

    Err(Error::at(
        Code::InvalidName,
        source,
        key.offset,
        format!("invalid {what} name `{name}`. Names use ASCII letters, digits, `-`, `_`, and `.`"),
    ))
}

/// Returns the byte offset of a string's content, after its opening quotes: one for YAML, and up
/// to three for a TOML multi-line string.
fn content_offset(source: &str, start: usize) -> usize {
    let quotes = source.get(start..).map_or(0, |rest| {
        rest.bytes()
            .take(3)
            .take_while(|b| matches!(b, b'"' | b'\''))
            .count()
    });
    start + quotes
}

/// Returns whether YAML reads a plain scalar as a number.
fn is_number(text: &str) -> bool {
    matches!(Yaml::from_str(text), Yaml::Integer(_) | Yaml::Real(_))
}

fn non_negative(source: &str, value: &Node, key: &str) -> Result<f64, Error> {
    let amount = number(source, value, key)?;
    if amount < 0.0 {
        return Err(Error::at(
            Code::InvalidValue,
            source,
            value.offset,
            format!("`{key}` cannot be negative"),
        ));
    }
    Ok(amount)
}

/// Parses a decimal number written as a plain scalar.
fn number(source: &str, value: &Node, key: &str) -> Result<f64, Error> {
    let parsed = match &value.kind {
        Kind::Scalar { plain: true, text } if is_number(text) => text.parse::<f64>().ok(),
        _ => None,
    };

    parsed.filter(|n| n.is_finite()).ok_or_else(|| {
        Error::at(
            Code::InvalidType,
            source,
            value.offset,
            format!("`{key}` must be a finite number"),
        )
    })
}

fn string<'n>(source: &str, value: &'n Node, key: &str) -> Result<&'n str, Error> {
    match value.text() {
        Some(text) if !value.is_null() => Ok(text),
        _ => Err(Error::at(
            Code::InvalidType,
            source,
            value.offset,
            format!("`{key}` must be a string"),
        )),
    }
}
