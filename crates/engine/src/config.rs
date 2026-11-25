//! The configuration: `uphill.yaml` or `uphill.toml`, which describe the same categories.
//!
//! Each top-level key is a category. A category's `run` key is the command that measures it,
//! `timeout` and `description` are optional, and every other key is a metric. A top-level `run`
//! and `timeout` apply to every category that sets none, and categories with the same command
//! share one run of it. A top-level `setup` command prepares each checkout that
//! `uphill --compare` creates. A metric is a mapping of optional keys: `poor`, `weight`, `score`,
//! `better`, `varies`, `title`, and `description`. `poor` is the value that scores 50 against the
//! default target, and twice the target without it.
//!
//! A top-level `packs` key lists packs whose categories the project runs too (see
//! [`crate::pack`]). Categories with the same name merge into one: each keeps its own script, and
//! a metric that two packs declare takes the items of both. A project category with the name of
//! a pack's category adds its own metrics, overrides a pack metric key by key, and drops one with
//! `metric: false`.
//!
//! The baseline file beside the configuration holds the targets (see [`crate::baseline`]). Older
//! configurations set `target`, `targets`, and `baseline` on each metric instead. Uphill still
//! reads them, and `uphill --update` moves them into the baseline file.
//!
//! ```yaml
//! footprint:
//!   run: bun metrics/footprint.ts
//!   export-size: { score: max }
//! ```
//!
//! ```toml
//! [footprint]
//! run = "bun metrics/footprint.ts"
//! export-size = { score = "max" }
//! ```

use std::time::Duration;

use yaml_rust2::Yaml;

use crate::baseline::Baseline;
use crate::check::Source;
use crate::document::{self, Kind, Node};
use crate::error::{Code, Error, Location};
use crate::lines::is_name;
use crate::pack::Pack;
use crate::value::{self, Value};

/// Keys a metric accepts, in the order error messages list them.
const METRIC_KEYS: [&str; 10] = [
    "baseline",
    "better",
    "description",
    "poor",
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
    /// `uphill.toml`.
    Toml,
    /// `uphill.yaml`.
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
    /// The command that prepares a checkout `uphill --compare` creates, such as installing
    /// packages, or `None` when it needs nothing.
    pub setup: Option<Setup>,
}

/// The top-level `setup` command.
#[derive(Clone, Debug, PartialEq)]
pub struct Setup {
    /// The shell command, run once from the project root of each new checkout.
    pub run: String,
    /// How long it may take before Uphill stops it: the top-level `timeout`, or 10 minutes.
    pub timeout: Duration,
}

/// Returns whether two configurations measure alike: equal apart from baselines, and from where the
/// files set each target.
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

/// Returns `config` with every location cleared, and every pattern from the configuration, to
/// compare it with the same configuration after lines or targets moved.
pub(crate) fn without_locations(mut config: Config) -> Config {
    let nowhere = Location { column: 0, line: 0 };
    for category in &mut config.categories {
        category.location = nowhere;
        for metric in &mut category.metrics {
            metric.location = nowhere;
            for pattern in &mut metric.targets {
                pattern.location = nowhere;
                pattern.source = Source::Config;
            }
        }
    }
    config
}

/// A category: the metrics it scores and the scripts that write them.
#[derive(Clone, Debug, PartialEq)]
pub struct Category {
    /// What the category measures, for reports.
    pub description: Option<String>,
    /// The pack's metrics that the project drops with `metric: false`. Their lines are ignored.
    pub dropped: Vec<String>,
    /// The position of the category's name.
    pub location: Location,
    /// Metrics in the order the file declares them.
    pub metrics: Vec<Metric>,
    /// The category name, such as `footprint`.
    pub name: String,
    /// The scripts that write the metrics, one per declaration of the category: the project's
    /// and each pack's. A category of one file has one.
    pub scripts: Vec<Script>,
    /// The name reports show, such as `Best Practices`.
    pub title: Option<String>,
}

impl Category {
    /// Returns the command of the first script. A category from [`Category::units`] has one.
    pub fn run(&self) -> &str {
        self.scripts.first().map_or("", |script| &script.run)
    }

    /// Returns how long the first script may run. A category from [`Category::units`] has one.
    pub fn timeout(&self) -> Duration {
        self.scripts
            .first()
            .map_or(DEFAULT_TIMEOUT, |script| script.timeout)
    }

    /// Returns the name reports show: the `title`, or else the name in words, such as `Speed`.
    pub fn title(&self) -> String {
        self.title.clone().unwrap_or_else(|| words(&self.name))
    }

    /// Returns one category per script, each with that script and only the metrics it writes, in
    /// the order of the scripts. Uphill measures these and scores the whole category.
    pub fn units(&self) -> Vec<Category> {
        self.scripts
            .iter()
            .map(|script| Category {
                metrics: self
                    .metrics
                    .iter()
                    .filter(|metric| script.metrics.contains(&metric.name))
                    .cloned()
                    .collect(),
                scripts: vec![script.clone()],
                ..self.clone()
            })
            .collect()
    }
}

/// One script of a category and the metrics it writes there.
#[derive(Clone, Debug, PartialEq)]
pub struct Script {
    /// The names of the metrics the script writes for the category.
    pub metrics: Vec<String>,
    /// The shell command that runs the script. Scripts with the same command share one run.
    pub run: String,
    /// How long one run may take before Uphill stops it. Defaults to 10 minutes.
    pub timeout: Duration,
}

/// A configured metric.
#[derive(Clone, Debug, PartialEq)]
pub struct Metric {
    /// The sizes, counts, and checks that `uphill --update` last accepted, and each measurement's
    /// own target, from the baseline file or an older configuration's `baseline` key, or `None`
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
    /// The value that scores 50 against `target`, Lighthouse's median control point. `None` for
    /// twice the target, or half the target when higher is better. Each item's own target scales
    /// it by the same factor.
    pub poor: Option<Value>,
    /// How a list of items collapses to one value. Defaults to `p90`, or `p10` when higher is
    /// better.
    pub score: Rollup,
    /// The default target, which scores 90: the target of a metric without items, or the `*`
    /// pattern of a metric with items, which an item's own target, a pattern, or its line
    /// replaces. A metric without any target is shown but not scored.
    pub target: Option<Value>,
    /// Targets by item pattern, in the order the file declares them. The first pattern that
    /// matches an item without its own target sets its target, over the item's line and the
    /// default.
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

    /// Returns the value in base units that scores 50 for an item scored against `target`: the
    /// metric's `poor` scaled by the item's target over the default `target`, or else twice the
    /// target, or half the target when higher is better.
    pub fn poor(&self, target: f64) -> f64 {
        match (
            self.poor.and_then(Value::amount),
            self.target.and_then(Value::amount),
        ) {
            // An item at the default target keeps `poor` exactly, so its score matches
            // Lighthouse's for the same control points.
            (Some(poor), Some(default)) => poor * (target / default),
            _ => match self.better {
                Better::Higher => target / 2.0,
                Better::Lower => target * 2.0,
            },
        }
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

/// An item pattern and the target it sets, such as `"viem/chains/*": { target: 1kB }`.
#[derive(Clone, Debug, PartialEq)]
pub struct Pattern {
    /// The position of the pattern in the file that `source` names.
    pub location: Location,
    /// The pattern. `*` matches any run of characters, and every other character matches itself.
    pub pattern: String,
    /// The file that sets the pattern: the baseline file, or an older configuration's `targets`.
    pub source: Source,
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

/// Parses the source text of a configuration in `format`, without its packs.
///
/// # Errors
///
/// Returns the first error: YAML or TOML syntax, an unknown key, a value of the wrong type, a missing
/// `run` command, a target that cannot score, or a metric in two categories that share a run.
pub fn parse(source: &str, format: Format) -> Result<Config, Error> {
    compose(source, format, &[])
}

/// Parses the source text of a configuration in `format` and adds the categories of its packs,
/// in the order `packs` lists them, after the project's own. Pack categories with the same name
/// merge in the order of the entries: the category keeps every script, and a metric that an
/// earlier pack declares keeps that pack's keys and takes the items of both scripts. A project
/// category with a pack category's name takes its place in the project's order, adds its own
/// metrics with its own script, overrides a pack metric key by key, and drops one with
/// `metric: false`. A pack's sizes, counts, and checks
/// stay behind, and each pack category and metric takes the location of its entry, so an error
/// points into the project's configuration.
///
/// # Errors
///
/// Returns the errors of [`parse`], an entry's category that its pack does not declare, and a
/// `run` in a project category that only overrides a pack's metrics.
pub fn compose(source: &str, format: Format, packs: &[Pack]) -> Result<Config, Error> {
    let Some(document) = document::parse(source, format)? else {
        return Err(nothing_configured(format));
    };
    let Kind::Mapping(pairs) = &document.kind else {
        return Err(Error::at(
            Code::InvalidType,
            source,
            document.offset,
            "uphill.yaml must map category names to categories, such as `footprint:`".to_owned(),
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

    let mut packed = from_packs(packs)?;
    let mut categories: Vec<Category> = Vec::new();

    for (key, value) in pairs {
        let name = self::key(source, key)?;
        if matches!(
            name,
            "allowed-packs" | "packs" | "run" | "setup" | "timeout"
        ) {
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

        // A pack category that the project declares takes the project's place in the order.
        let base = packed
            .iter()
            .position(|base| base.name == name)
            .map(|index| packed.remove(index));
        categories.push(self::category(
            source,
            format,
            key,
            name,
            fields,
            &defaults,
            base.as_ref(),
        )?);
    }
    categories.append(&mut packed);

    let units: Vec<Category> = categories.iter().flat_map(Category::units).collect();
    for (index, unit) in units.iter().enumerate() {
        // A shared run gives each line to the category that configures its metric, so a metric
        // name can belong to only one of the categories that share it.
        for other in units
            .iter()
            .take(index)
            .filter(|other| other.name != unit.name && other.run() == unit.run())
        {
            if let Some(metric) = unit
                .metrics
                .iter()
                .find(|metric| other.metrics.iter().any(|m| m.name == metric.name))
            {
                return Err(Error {
                    code: Code::DuplicateMetric,
                    location: Some(metric.location),
                    message: format!(
                        "metric `{}` is in `{}` and `{}`, which share the same `run`, so Uphill cannot tell which category its lines belong to",
                        metric.name, other.name, unit.name
                    ),
                });
            }
        }
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

/// Returns the categories of the packs, in the order of their entries, each with the location of its
/// entry and without the pack's baselines.
fn from_packs(packs: &[Pack]) -> Result<Vec<Category>, Error> {
    let mut categories: Vec<Category> = Vec::new();
    for pack in packs {
        let entry = &pack.entry;
        if let Some(names) = &entry.categories
            && let Some(name) = names
                .iter()
                .find(|name| !pack.config.categories.iter().any(|c| &c.name == *name))
        {
            return Err(Error {
                code: Code::InvalidValue,
                location: Some(entry.location),
                message: format!("pack `{}` has no category `{name}`", entry.name),
            });
        }

        for category in &pack.config.categories {
            if entry
                .categories
                .as_ref()
                .is_some_and(|names| !names.contains(&category.name))
            {
                continue;
            }
            let mut category = category.clone();
            category.location = entry.location;
            for metric in &mut category.metrics {
                metric.baseline = None;
                metric.location = entry.location;
                for pattern in &mut metric.targets {
                    pattern.location = entry.location;
                }
            }

            let Some(earlier) = categories.iter_mut().find(|c| c.name == category.name) else {
                categories.push(category);
                continue;
            };
            earlier.description = earlier.description.take().or(category.description);
            earlier.title = earlier.title.take().or(category.title);
            for name in category.dropped {
                if !earlier.dropped.contains(&name) {
                    earlier.dropped.push(name);
                }
            }
            for metric in category.metrics {
                if !earlier.metrics.iter().any(|m| m.name == metric.name) {
                    earlier.metrics.push(metric);
                }
            }
            earlier.scripts.append(&mut category.scripts);
        }
    }
    Ok(categories)
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

/// Parses a category, or an override of `base`, a pack's category, when it is set.
fn category(
    source: &str,
    format: Format,
    key: &Node,
    name: &str,
    fields: &[(Node, Node)],
    defaults: &Defaults,
    base: Option<&Category>,
) -> Result<Category, Error> {
    let mut description = base.and_then(|base| base.description.clone());
    let mut dropped = base.map(|base| base.dropped.clone()).unwrap_or_default();
    let mut metrics = base.map(|base| base.metrics.clone()).unwrap_or_default();
    // The metrics this declaration adds, which its own script writes.
    let mut own: Vec<String> = Vec::new();
    let mut run = None;
    let mut timeout = None;
    let mut title = base.and_then(|base| base.title.clone());

    for (field, value) in fields {
        match self::key(source, field)? {
            "description" => description = Some(string(source, value, "description")?.to_owned()),
            "run" => run = Some((command(source, value, "run")?, field.offset)),
            "timeout" => timeout = Some((duration(source, value)?, field.offset)),
            "title" => title = Some(string(source, value, "title")?.to_owned()),
            metric => {
                check_name(source, field, metric, "metric")?;

                let inherited = base
                    .filter(|base| base.metrics.iter().any(|m| m.name == metric))
                    .and_then(|_| metrics.iter().position(|m| m.name == metric));
                if is_false(value) {
                    let Some(index) = inherited else {
                        let message = match base {
                            Some(_) => format!(
                                "`{metric}: false` drops a pack's metric, and no pack's `{name}` declares `{metric}`"
                            ),
                            None => format!(
                                "`{metric}: false` drops a pack's metric, and `{name}` is not a pack's category"
                            ),
                        };
                        return Err(Error::at(Code::InvalidType, source, value.offset, message));
                    };
                    dropped.push(metrics.remove(index).name);
                    continue;
                }

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

                if let Some(index) = inherited {
                    let base = metrics.remove(index);
                    let merged = self::metric(source, format, field, metric, keys, Some(base))?;
                    metrics.insert(index, merged);
                } else {
                    own.push(metric.to_owned());
                    metrics.push(self::metric(source, format, field, metric, keys, None)?);
                }
            }
        }
    }

    // The project's own metrics come before the packs' in reports.
    metrics.sort_by_key(|metric| !own.contains(&metric.name));

    let mut scripts = base.map(|base| base.scripts.clone()).unwrap_or_default();
    // A pack category that the project only adjusts runs the pack's scripts alone.
    if base.is_some() && own.is_empty() {
        if let Some((_, offset)) = run {
            return Err(Error::at(
                Code::UnexpectedKey,
                source,
                offset,
                format!(
                    "`run` runs the metrics that `{name}` adds, and it adds none. A pack's metrics keep the pack's script"
                ),
            ));
        }
        if let Some((timeout, _)) = timeout {
            for script in &mut scripts {
                script.timeout = timeout;
            }
        }
    } else {
        let Some(run) = run.map(|(run, _)| run).or_else(|| defaults.run.clone()) else {
            return Err(Error::at(
                Code::RunMissing,
                source,
                key.offset,
                format!(
                    "category `{name}` has no `run` command. Add one, or a top-level `run` that every category shares"
                ),
            ));
        };
        scripts.push(Script {
            metrics: own,
            run,
            timeout: timeout.map_or(defaults.timeout, |(timeout, _)| timeout),
        });
    }

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
        dropped,
        location: base.map_or_else(|| Location::of(source, key.offset), |base| base.location),
        metrics,
        name: name.to_owned(),
        scripts,
        title,
    })
}

/// Parses a metric, or the keys that override `base`, a pack's metric, when it is set.
fn metric(
    source: &str,
    format: Format,
    key: &Node,
    name: &str,
    keys: &[(Node, Node)],
    base: Option<Metric>,
) -> Result<Metric, Error> {
    let mut baseline = None;
    let mut better = base.as_ref().map_or(Better::Lower, |base| base.better);
    let mut better_set = false;
    let mut description = base.as_ref().and_then(|base| base.description.clone());
    let mut poor = None;
    let mut score = None;
    let mut target = base.as_ref().and_then(|base| base.target);
    let mut targets = base
        .as_ref()
        .map(|base| base.targets.clone())
        .unwrap_or_default();
    let mut title = base.as_ref().and_then(|base| base.title.clone());
    let mut varies = base.as_ref().is_some_and(|base| base.varies);
    let mut weight = base.as_ref().map_or(1.0, |base| base.weight);

    for (field, value) in keys {
        match self::key(source, field)? {
            "baseline" => baseline = Some(crate::baseline::parse(source, value)?),
            "better" => {
                better_set = true;
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
            "poor" => poor = Some((self::target(source, name, "poor", value)?, value.offset)),
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
            "target" => target = Some(self::target(source, name, "target", value)?),
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
                        source: Source::Config,
                        target: self::target(source, name, "target", value)?,
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

    // A `poor` without a `target` waits for the baseline file, which holds the targets now.
    if let Some((value, offset)) = poor
        && let Some((code, message)) = poor_problem(name, better, value, target)
    {
        return Err(Error::at(code, source, offset, message));
    }

    // An override keeps the pack's rollup unless it sets `score`, or `better`, which picks one.
    let rollup = match better {
        Better::Higher => Rollup::P10,
        Better::Lower => Rollup::P90,
    };
    let score = match (score, &base) {
        (Some(score), _) => score,
        (None, Some(base)) if !better_set => base.score,
        (None, _) => rollup,
    };

    Ok(Metric {
        baseline,
        better,
        description,
        location: Location::of(source, key.offset),
        name: name.to_owned(),
        poor: poor
            .map(|(value, _)| value)
            .or(base.and_then(|base| base.poor)),
        score,
        target,
        targets,
        title,
        varies,
        weight,
    })
}

/// Checks each metric's `poor` against its `target`, once the baseline file has set the targets.
///
/// # Errors
///
/// Returns an error at the metric for a `poor` without a `target`, of another unit, or on the wrong
/// side of the target.
pub fn validate(config: &Config) -> Result<(), Error> {
    for metric in config
        .categories
        .iter()
        .flat_map(|category| &category.metrics)
    {
        let Some(poor) = metric.poor else {
            continue;
        };
        let problem = match metric.target {
            None => Some((
                Code::InvalidValue,
                format!(
                    "metric `{}` sets `poor` without a `target`. Set the `target` that scores 90 in the baseline file",
                    metric.name
                ),
            )),
            Some(target) => poor_problem(&metric.name, metric.better, poor, Some(target)),
        };
        if let Some((code, message)) = problem {
            return Err(Error {
                code,
                location: Some(metric.location),
                message,
            });
        }
    }
    Ok(())
}

/// Returns the problem with a `poor` of another unit than `target`, or on the wrong side of it.
fn poor_problem(
    name: &str,
    better: Better,
    poor: Value,
    target: Option<Value>,
) -> Option<(Code, String)> {
    let target = target?;
    if target.kind() != poor.kind() {
        return Some((
            Code::TargetUnit,
            format!(
                "the `poor` of metric `{name}` is {}, but its `target` is {}",
                poor.kind().singular(),
                target.kind().singular()
            ),
        ));
    }
    let (poor, target) = (
        poor.amount().unwrap_or_default(),
        target.amount().unwrap_or_default(),
    );
    match better {
        Better::Lower if poor <= target => Some((
            Code::TargetRange,
            format!("the `poor` of metric `{name}` must be greater than its `target`"),
        )),
        Better::Higher if poor >= target => Some((
            Code::TargetRange,
            format!(
                "the `poor` of metric `{name}` must be less than its `target`, since higher is better"
            ),
        )),
        _ => None,
    }
}

/// Parses a target or `poor`, named by `key`: a value such as `5kB` or `20ms`, or a number for a
/// count. It must be greater than zero, and a check cannot have one.
fn target(source: &str, name: &str, key: &str, value: &Node) -> Result<Value, Error> {
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
                format!("`{key}` must be a value such as `5kB`, `20ms`, or a number"),
            ));
        }
    };

    match target.amount() {
        None => Err(Error::at(
            Code::TargetUnit,
            source,
            at,
            format!(
                "metric `{name}` cannot have a check as its {key}. A check scores the share of items that pass"
            ),
        )),
        Some(amount) if amount <= 0.0 => Err(Error::at(
            Code::TargetRange,
            source,
            at,
            format!("the `{key}` of metric `{name}` must be greater than zero"),
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

/// Returns whether a value is a plain `false`, which drops a pack's metric.
fn is_false(value: &Node) -> bool {
    matches!(&value.kind, Kind::Scalar { plain: true, text } if Yaml::from_str(text) == Yaml::Boolean(false))
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
