//! Packs: categories that one project shares with others. A pack is a directory laid out like a
//! project, with an `uphill.yaml` or `uphill.toml` and the scripts its `run` commands name, so any
//! project or example is a pack. `uphill add` copies a pack's categories into a project's
//! configuration and its scripts into the project, and the project then owns them.
//!
//! A YAML pack in a YAML project keeps its text. Any other pair of formats writes each category
//! from the pack's parsed keys, with the comments above the category and above each key.
//!
//! Each category keeps its text, so the comments that say what each metric measures come with it.
//! Its `run` command points at the copied scripts, and it takes the pack's top-level `run` and
//! `timeout` when it sets none. A comment above it records the command that added it and the
//! pack's commit, so `uphill add --update` fetches the pack again and merges its changes. The
//! default targets and patterns of the pack's baseline file go into the project's baseline file,
//! once, for each metric that has none there. The project then owns its targets, so an update
//! leaves them alone.
//!
//! ```yaml
//! # Added by `uphill add wevm/uphill/examples/web-app performance` at 3e0053b4f1c2.
//! performance:
//!   run: bun .agents/metrics/lighthouse.ts performance
//!   # Lighthouse's five metrics on each page, with Lighthouse's weights.
//!   first-contentful-paint: { weight: 10 }
//! ```
//!
//! An update compares three versions of each category and each script: the pack at the recorded
//! commit, the project, and the pack now. A version that only one side changed wins, and a
//! version that both changed is a conflict.

use std::fmt::Write as _;
use std::time::Duration;

use crate::baseline::{self, quoted, toml_key, yaml_key};
use crate::config::{self, Category, Config, Format, without_locations};
use crate::document::{self, Kind, Node};
use crate::edit::{after_colon, line_end, line_start};
use crate::error::{Code, Error};

/// Time a script may run when neither its category nor the configuration sets a `timeout`.
const DEFAULT_TIMEOUT: Duration = Duration::from_mins(10);

/// The start of the comment above each added category.
const ADDED: &str = "# Added by `uphill add ";

/// The project's configuration after `uphill add`.
#[derive(Clone, Debug, PartialEq)]
pub struct Added {
    /// The project's baseline file with the targets of the added categories' metrics, or `None`
    /// when the pack's baseline file sets none that the project lacks.
    pub baselines: Option<String>,
    /// The names of the added categories, in the order the pack declares them.
    pub categories: Vec<String>,
    /// The text of the project's `uphill.yaml` with the added categories at its end.
    pub config: String,
    /// The pack's top-level `setup` command when the project's configuration sets none, such as a
    /// package install that the scripts need.
    pub setup: Option<String>,
}

/// What `add` copies from a pack, and where to.
pub struct Options<'a> {
    /// The text of the project's baseline file, or `None` when it has none.
    pub baselines: Option<&'a str>,
    /// The pack's categories to add, or every category when it is empty.
    pub categories: &'a [String],
    /// The pack's commit, or `None` for a local directory.
    pub commit: Option<&'a str>,
    /// The format of the project's configuration, which a new configuration takes too.
    pub format: Format,
    /// The name to add the category under, which needs exactly one category.
    pub name: Option<&'a str>,
    /// The text of the pack's `uphill.yaml` or `uphill.toml`.
    pub pack: &'a str,
    /// The text of the pack's baseline file, or `None` when it has none. Its targets come with
    /// the categories, and its sizes, counts, and checks stay behind.
    pub pack_baselines: Option<&'a str>,
    /// The format of the pack's configuration.
    pub pack_format: Format,
    /// The text of the project's configuration, or `None` when the project has none yet. A new
    /// configuration takes the pack's `setup`.
    pub project: Option<&'a str>,
    /// Returns the `run` command for the project, given each added category's command in the pack.
    pub run: &'a mut dyn FnMut(&str) -> String,
    /// The pack as `uphill add` takes it, such as `wevm/uphill/examples/web-app`.
    pub source: &'a str,
}

/// Where an added category came from, as the comment above it records.
#[derive(Clone, Debug, PartialEq)]
pub struct Origin {
    /// The category's name in the project.
    pub category: String,
    /// The first 12 characters of the pack's commit when the category was added or last updated,
    /// or `None` for a local directory.
    pub commit: Option<String>,
    /// The category's name in the pack.
    pub name: String,
    /// The pack as `uphill add` takes it.
    pub source: String,
}

/// What a three-way merge keeps: the project's version, the pack's version, or neither.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Merge {
    /// The project and the pack both changed it since the recorded commit.
    Conflict,
    /// The project's version: the pack did not change it, or both versions are equal.
    Keep,
    /// The pack's version: the project did not change it.
    Take,
}

/// One category to update from its pack.
pub struct Update<'a> {
    /// The category as the pack at the recorded commit renders it, or `None` when that version is
    /// unknown, such as for a local directory.
    pub base: Option<&'a str>,
    /// The pack's commit now, or `None` for a local directory.
    pub commit: Option<&'a str>,
    /// Whether a conflict takes the pack's version.
    pub force: bool,
    /// The format of the project's configuration.
    pub format: Format,
    /// Where the category came from.
    pub origin: &'a Origin,
    /// The text of the project's configuration.
    pub project: &'a str,
    /// The category as the pack now renders it.
    pub theirs: &'a str,
}

/// The project's configuration after one category's update.
#[derive(Clone, Debug, PartialEq)]
pub struct Updated {
    /// The text of the project's `uphill.yaml`, unchanged on a conflict.
    pub config: String,
    /// What the merge kept.
    pub merge: Merge,
    /// Whether the project's category already equals the pack's.
    pub same: bool,
}

/// Returns the project's configuration with the pack's categories added.
///
/// # Errors
///
/// Returns an error when either configuration is invalid, when the pack declares no category of
/// that name, when the project already declares one, when `name` comes with more than one
/// category, or when a category is a flow mapping, which `add` cannot copy.
pub fn add(options: &mut Options<'_>) -> Result<Added, Error> {
    let pack = config::parse(options.pack, options.pack_format)?;
    let project = options
        .project
        .map(|source| config::parse(source, options.format))
        .transpose()?;

    if let Some(name) = options.categories.iter().find(|name| {
        !pack
            .categories
            .iter()
            .any(|category| category.name == **name)
    }) {
        return Err(unknown(name, &pack));
    }
    let selected: Vec<&Category> = pack
        .categories
        .iter()
        .filter(|category| {
            options.categories.is_empty() || options.categories.contains(&category.name)
        })
        .collect();
    if options.name.is_some() && selected.len() != 1 {
        return Err(Error {
            code: Code::InvalidValue,
            location: None,
            message: format!(
                "--as names one category, and the pack has {}. Name the category to add, such as `uphill add <pack> {} --as <name>`",
                selected.len(),
                selected
                    .first()
                    .map_or("<category>", |category| category.name.as_str())
            ),
        });
    }

    let mut blocks = Vec::new();
    let mut expected = Vec::new();
    for category in &selected {
        let name = options.name.unwrap_or(&category.name);
        if let Some(project) = &project
            && project.categories.iter().any(|other| other.name == name)
        {
            return Err(Error::at(
                Code::DuplicateCategory,
                options.pack,
                key_offset(options.pack, options.pack_format, &category.name).unwrap_or_default(),
                format!(
                    "the project already declares category `{name}`. Add it under another name with `--as <name>`"
                ),
            ));
        }

        let run = (options.run)(&category.run);
        let formats = (options.pack_format, options.format);
        let Some(text) = render_category(options.pack, formats, &pack, &category.name, name, &run)?
        else {
            continue;
        };
        let comment = comment(options.source, &category.name, name, options.commit);
        blocks.push(format!("{comment}\n{text}"));

        let mut category = (*category).clone();
        name.clone_into(&mut category.name);
        category.run = run;
        expected.push(category);
    }

    // A new configuration takes the pack's `setup`, and an existing one without it gets a hint.
    let setup = pack
        .setup
        .clone()
        .filter(|_| {
            project
                .as_ref()
                .is_none_or(|project| project.setup.is_none())
        })
        .map(|setup| setup.run);
    let (config, setup) = match options.project.map(str::trim_end) {
        Some(project) if !project.is_empty() => {
            (format!("{project}\n\n{}\n", blocks.join("\n\n")), setup)
        }
        _ => {
            let line = setup
                .map(|run| match options.format {
                    Format::Toml => format!("setup = {}\n\n", quoted(&run)),
                    Format::Yaml => format!("setup: {}\n\n", scalar(&run)),
                })
                .unwrap_or_default();
            (format!("{line}{}\n", blocks.join("\n\n")), None)
        }
    };

    verify(&config, options.format, expected)?;
    let baselines = targets(options, &pack, &selected, &config)?;
    Ok(Added {
        baselines,
        categories: selected
            .iter()
            .map(|category| options.name.unwrap_or(&category.name).to_owned())
            .collect(),
        config,
        setup,
    })
}

/// Returns the project's baseline file with the default targets and patterns that the pack's
/// baseline file sets for the added categories' metrics, where the project sets none, or `None`
/// when there are none to add. The pack's sizes, counts, checks, and item targets stay behind,
/// since they belong to the pack's own code.
fn targets(
    options: &Options<'_>,
    pack: &Config,
    selected: &[&Category],
    config: &str,
) -> Result<Option<String>, Error> {
    let Some(pack_baselines) = options.pack_baselines else {
        return Ok(None);
    };
    let pack = baseline::read(pack.clone(), pack_baselines, options.pack_format)?;
    let project = config::parse(config, options.format)?;
    let mut project = match options.baselines {
        Some(source) => baseline::read(project, source, options.format)?,
        None => project,
    };

    let mut changed = false;
    for category in selected {
        let name = options.name.unwrap_or(&category.name);
        let from = pack.categories.iter().find(|c| c.name == category.name);
        let to = project.categories.iter_mut().find(|c| c.name == name);
        let (Some(from), Some(to)) = (from, to) else {
            continue;
        };
        for metric in &mut to.metrics {
            let Some(source) = from.metrics.iter().find(|m| m.name == metric.name) else {
                continue;
            };
            if metric.target.is_none() && source.target.is_some() {
                metric.target = source.target;
                changed = true;
            }
            if metric.targets.is_empty() && !source.targets.is_empty() {
                metric.targets.clone_from(&source.targets);
                changed = true;
            }
        }
    }

    Ok(changed.then(|| baseline::render(&project, options.format)))
}

/// Returns the category `name` of the pack, with its text and format, as `uphill add` writes it
/// under `category` in a project of `format`, without the comment above it, or `None` when the
/// pack does not declare it. `run` maps the category's command in the pack to the project's.
///
/// # Errors
///
/// Returns an error when the pack's configuration is invalid or the category is a flow mapping.
pub fn render(
    pack: (&str, Format),
    name: &str,
    category: &str,
    format: Format,
    run: &mut dyn FnMut(&str) -> String,
) -> Result<Option<String>, Error> {
    let config = config::parse(pack.0, pack.1)?;
    let Some(found) = config.categories.iter().find(|c| c.name == name) else {
        return Ok(None);
    };
    let run = run(&found.run);
    render_category(pack.0, (pack.1, format), &config, name, category, &run)
}

/// Returns where each category that `uphill add` wrote came from, in the order the project
/// declares them.
pub fn origins(project: &str, format: Format) -> Vec<Origin> {
    let Ok(Some(Node {
        kind: Kind::Mapping(pairs),
        ..
    })) = document::parse(project, format)
    else {
        return Vec::new();
    };

    pairs
        .iter()
        .filter_map(|(key, _)| {
            let category = key.text()?;
            let start = attached(project, key.offset);
            let comments = project.get(start..line_start(project, key.offset))?;
            let line = comments.lines().find(|line| line.starts_with(ADDED))?;
            let (command, rest) = line.strip_prefix(ADDED)?.split_once('`')?;
            let words: Vec<&str> = command.split_whitespace().collect();
            let (source, name) = match words.as_slice() {
                [source, name, "--as", _] | [source, name] => (*source, *name),
                _ => return None,
            };
            let commit = rest
                .strip_prefix(" at ")
                .and_then(|rest| rest.strip_suffix('.'))
                .map(str::to_owned);
            Some(Origin {
                category: category.to_owned(),
                commit,
                name: name.to_owned(),
                source: source.to_owned(),
            })
        })
        .collect()
}

/// Returns what a three-way merge keeps, given the pack's version at the recorded commit, the
/// project's version, and the pack's version now. `None` is a version that does not exist, or a
/// recorded version that is unknown.
pub fn merge(base: Option<&[u8]>, ours: Option<&[u8]>, theirs: Option<&[u8]>) -> Merge {
    if ours == theirs {
        return Merge::Keep;
    }
    if base.is_some() && base == theirs {
        return Merge::Keep;
    }
    if base == ours {
        return Merge::Take;
    }
    Merge::Conflict
}

/// Returns the project's configuration with one added category merged with its pack, and the
/// comment above it naming the pack's new commit. A conflict without `force` changes nothing.
///
/// # Errors
///
/// Returns an error when the project no longer declares the category or the result is invalid.
pub fn update(options: &Update<'_>) -> Result<Updated, Error> {
    let project = options.project;
    let origin = options.origin;
    let missing = || Error {
        code: Code::InvalidValue,
        location: None,
        message: format!(
            "the project no longer declares category `{}`",
            origin.category
        ),
    };
    let Ok(Some(Node {
        kind: Kind::Mapping(pairs),
        ..
    })) = document::parse(project, options.format)
    else {
        return Err(missing());
    };
    let key = pairs
        .iter()
        .map(|(key, _)| key)
        .find(|key| key.text() == Some(origin.category.as_str()))
        .ok_or_else(missing)?;
    let start = attached(project, key.offset);
    let end = trimmed_end(project, key.offset, next(project, &pairs, key.offset));

    // The comment that records the origin is not part of the category's text.
    let text = project.get(start..end).unwrap_or_default();
    let ours: String = text
        .split_inclusive('\n')
        .filter(|line| !line.starts_with(ADDED))
        .collect();
    let same = ours == options.theirs;

    let merged = match merge(
        options.base.map(str::as_bytes),
        Some(ours.as_bytes()),
        Some(options.theirs.as_bytes()),
    ) {
        Merge::Conflict if options.force => Merge::Take,
        merged => merged,
    };
    let kept = match merged {
        Merge::Conflict => {
            return Ok(Updated {
                config: project.to_owned(),
                merge: merged,
                same,
            });
        }
        Merge::Keep => ours.as_str(),
        Merge::Take => options.theirs,
    };

    let comment = comment(
        &origin.source,
        &origin.name,
        &origin.category,
        options.commit,
    );
    let config = format!(
        "{}{comment}\n{kept}{}",
        project.get(..start).unwrap_or_default(),
        project.get(end..).unwrap_or_default()
    );
    let parsed = config::parse(&config, options.format).map_err(|error| Error {
        code: error.code,
        location: None,
        message: format!("the updated configuration is invalid: {}", error.message),
    })?;
    if !parsed.categories.iter().any(|c| c.name == origin.category) {
        return Err(missing());
    }
    Ok(Updated {
        config,
        merge: merged,
        same,
    })
}

/// Returns the comment above an added category: the command that adds it again, and the first 12
/// characters of the commit.
fn comment(source: &str, name: &str, category: &str, commit: Option<&str>) -> String {
    let rename = if name == category {
        String::new()
    } else {
        format!(" --as {category}")
    };
    let at = commit
        .map(|commit| format!(" at {}", commit.get(..12).unwrap_or(commit)))
        .unwrap_or_default();
    format!("{ADDED}{source} {name}{rename}`{at}.")
}

/// Returns the category `name` of a parsed pack as the project writes it under `category`.
/// Returns the category `name` of a parsed pack as the project writes it under `category`. The
/// formats are the pack's and the project's.
fn render_category(
    source: &str,
    formats: (Format, Format),
    pack: &Config,
    name: &str,
    category: &str,
    run: &str,
) -> Result<Option<String>, Error> {
    let Some(found) = pack.categories.iter().find(|c| c.name == name) else {
        return Ok(None);
    };
    let Some(Node {
        kind: Kind::Mapping(pairs),
        ..
    }) = document::parse(source, formats.0)?
    else {
        return Ok(None);
    };
    let Some((key, value)) = pairs.iter().find(|(key, _)| key.text() == Some(name)) else {
        return Ok(None);
    };
    let block = Block {
        category: found,
        end: next(source, &pairs, key.offset),
        key,
        name: category,
        run,
        source,
        start: attached(source, key.offset),
        value,
    };
    match formats {
        (Format::Yaml, Format::Yaml) => self::block(&block).map(Some),
        (_, format) => Ok(Some(structured(&block, format))),
    }
}

/// Returns the start of the text of the top-level key after the one at `offset`, by position in
/// the source, or the end of the source.
fn next(source: &str, pairs: &[(Node, Node)], offset: usize) -> usize {
    pairs
        .iter()
        .map(|(key, _)| key.offset)
        .filter(|other| *other > offset)
        .min()
        .map_or(source.len(), |other| attached(source, other))
}

/// Checks that `config` parses and declares the `expected` categories as they are.
fn verify(config: &str, format: Format, expected: Vec<Category>) -> Result<(), Error> {
    // The categories keep their meaning, so the configuration must parse to the same categories.
    let written = config::parse(config, format).map_err(|error| Error {
        code: error.code,
        location: None,
        message: format!(
            "the project cannot take these categories: {}",
            error.message
        ),
    })?;
    let added = Config {
        categories: written
            .categories
            .into_iter()
            .filter(|category| expected.iter().any(|other| other.name == category.name))
            .collect(),
        setup: None,
    };
    let expected = Config {
        categories: expected,
        setup: None,
    };
    if without_locations(added) != without_locations(expected) {
        return Err(Error {
            code: Code::InvalidType,
            location: None,
            message:
                "uphill add cannot copy these categories as they are written. Copy them by hand"
                    .to_owned(),
        });
    }
    Ok(())
}

/// Returns the byte offset of the top-level key `name`.
fn key_offset(source: &str, format: Format, name: &str) -> Option<usize> {
    let Ok(Some(Node {
        kind: Kind::Mapping(pairs),
        ..
    })) = document::parse(source, format)
    else {
        return None;
    };
    pairs
        .iter()
        .find(|(key, _)| key.text() == Some(name))
        .map(|(key, _)| key.offset)
}

/// Returns the end of a category's text before `end`, without the trailing whitespace and the
/// unindented comments that belong to whatever follows it.
fn trimmed_end(source: &str, key: usize, mut end: usize) -> usize {
    loop {
        let text = source.get(key..end).unwrap_or_default();
        let trimmed = key + text.trim_end().len();
        let last = line_start(source, trimmed);
        let line = source.get(last..trimmed).unwrap_or_default();
        if last > key && line.starts_with('#') {
            end = last;
        } else {
            return trimmed;
        }
    }
}

/// One category of the pack and how to write it into the project.
struct Block<'a> {
    category: &'a Category,
    /// The end of the category's text: the start of the next category, or of the file's end.
    end: usize,
    key: &'a Node,
    name: &'a str,
    run: &'a str,
    source: &'a str,
    /// The start of the category's text, at the first comment line attached to its key.
    start: usize,
    value: &'a Node,
}

/// Returns a category's text for the project: its key, its `run` and `timeout`, and its other
/// lines and comments as the pack writes them.
fn block(block: &Block<'_>) -> Result<String, Error> {
    let source = block.source;
    let Kind::Mapping(fields) = &block.value.kind else {
        return Err(flow(block));
    };
    if line_start(source, block.value.offset) == line_start(source, block.key.offset) {
        return Err(flow(block));
    }

    let end = trimmed_end(source, block.key.offset, block.end);

    let indent = fields
        .first()
        .map_or(2, |(key, _)| key.offset - line_start(source, key.offset));
    let field = |name: &str| fields.iter().find(|(key, _)| key.text() == Some(name));
    let mut edits: Vec<(usize, usize, String)> = Vec::new();

    let colon = after_colon(source, block.key.offset);
    if block.name != block.category.name {
        edits.push((
            block.key.offset,
            colon - 1,
            yaml_key(block.name).into_owned(),
        ));
    }

    let mut inserted = String::new();
    match field("run") {
        Some((key, _)) if block.run != block.category.run => {
            edits.push((
                key.offset,
                line_end(source, key.offset),
                format!("run: {}", scalar(block.run)),
            ));
        }
        Some(_) => {}
        None => {
            let _ = write!(inserted, "\n{:indent$}run: {}", "", scalar(block.run));
        }
    }
    if field("timeout").is_none() && block.category.timeout != DEFAULT_TIMEOUT {
        let timeout = duration(block.category.timeout);
        let _ = write!(inserted, "\n{:indent$}timeout: {timeout}", "");
    }
    if !inserted.is_empty() {
        // After the category's `run`, or after its key when it takes the pack's.
        let after = field("run").map_or(block.key.offset, |(key, _)| key.offset);
        let at = line_end(source, after);
        edits.push((at, at, inserted));
    }

    let mut text = source.get(block.start..end).unwrap_or_default().to_owned();
    edits.sort_by_key(|(start, _, _)| std::cmp::Reverse(*start));
    for (from, to, replacement) in edits {
        if let (Some(from), Some(to)) = (from.checked_sub(block.start), to.checked_sub(block.start))
            && text.get(from..to).is_some()
        {
            text.replace_range(from..to, &replacement);
        }
    }
    Ok(text)
}

/// Returns a category's text for a project of `format`, written from the pack's parsed keys: the
/// comments above the category and above each key, the category's header, its `run` and
/// `timeout`, and one line per key. A pack in the other format, or a TOML pack, takes this path,
/// so comments at the end of a line stay behind.
fn structured(block: &Block<'_>, format: Format) -> String {
    let source = block.source;
    let fields: &[(Node, Node)] = match &block.value.kind {
        Kind::Mapping(fields) => fields,
        _ => &[],
    };
    let indent = match format {
        Format::Toml => "",
        Format::Yaml => "  ",
    };
    let comments = |out: &mut String, from: usize, to: usize, indent: &str| {
        for line in source.get(from..to).unwrap_or_default().lines() {
            let line = line.trim();
            if line.starts_with('#') {
                let _ = writeln!(out, "{indent}{line}");
            }
        }
    };
    let line = |out: &mut String, key: &str, value: &str| {
        let _ = match format {
            Format::Toml => writeln!(out, "{} = {value}", toml_key(key)),
            Format::Yaml => writeln!(out, "  {}: {value}", yaml_key(key)),
        };
    };
    let text = |text: &str| match format {
        Format::Toml => quoted(text),
        Format::Yaml => scalar(text),
    };

    let mut out = String::new();
    comments(
        &mut out,
        block.start,
        line_start(source, block.key.offset),
        "",
    );
    let _ = match format {
        Format::Toml => writeln!(out, "[{}]", toml_key(block.name)),
        Format::Yaml => writeln!(out, "{}:", yaml_key(block.name)),
    };

    let has = |name: &str| fields.iter().any(|(key, _)| key.text() == Some(name));
    let extra = |out: &mut String| {
        if block.category.timeout != DEFAULT_TIMEOUT && !has("timeout") {
            line(out, "timeout", &text(&duration(block.category.timeout)));
        }
    };
    if !has("run") {
        line(&mut out, "run", &text(block.run));
        extra(&mut out);
    }
    for (key, value) in fields {
        let Some(name) = key.text() else {
            continue;
        };
        comments(
            &mut out,
            attached(source, key.offset),
            line_start(source, key.offset),
            indent,
        );
        match (name, &value.kind) {
            ("run", _) => {
                line(&mut out, "run", &text(block.run));
                extra(&mut out);
            }
            (_, Kind::Scalar { .. }) if !value.is_null() => {
                line(&mut out, name, &inline(value, format, false));
            }
            _ => line(&mut out, name, &inline(value, format, true)),
        }
    }
    out.trim_end().to_owned()
}

/// Formats a value on one line: a TOML value or inline table, or a YAML flow value. A null or a
/// mapping without keys is `{}`. `flow` is whether a YAML scalar sits inside a flow mapping.
fn inline(node: &Node, format: Format, flow: bool) -> String {
    match &node.kind {
        Kind::Mapping(pairs) if !pairs.is_empty() => {
            let pairs: Vec<String> = pairs
                .iter()
                .map(|(key, value)| {
                    let key = key.text().unwrap_or_default();
                    match format {
                        Format::Toml => {
                            format!("{} = {}", toml_key(key), inline(value, format, true))
                        }
                        Format::Yaml => {
                            format!("{}: {}", yaml_key(key), inline(value, format, true))
                        }
                    }
                })
                .collect();
            format!("{{ {} }}", pairs.join(", "))
        }
        Kind::Sequence(items) => {
            let items: Vec<String> = items
                .iter()
                .map(|item| inline(item, format, true))
                .collect();
            format!("[{}]", items.join(", "))
        }
        Kind::Scalar { plain, text } if !node.is_null() => {
            let bare = *plain
                && (text.parse::<f64>().is_ok_and(f64::is_finite)
                    || matches!(text.as_str(), "true" | "false"));
            match format {
                _ if bare => text.clone(),
                Format::Toml => quoted(text),
                Format::Yaml if flow => flow_scalar(text),
                Format::Yaml => scalar(text),
            }
        }
        _ => "{}".to_owned(),
    }
}

/// Formats a YAML scalar inside a flow mapping: plain when YAML reads it back as the same string.
fn flow_scalar(text: &str) -> String {
    let plain = yaml_rust2::YamlLoader::load_from_str(&format!("key: {{ value: {text} }}"))
        .ok()
        .and_then(|documents| documents.into_iter().next())
        .is_some_and(|document| document["key"]["value"].as_str() == Some(text));
    if plain { text.to_owned() } else { quoted(text) }
}

/// Returns the start of the line of the key at `offset`, or of the comment lines right above it.
fn attached(source: &str, offset: usize) -> usize {
    let mut start = line_start(source, offset);
    while let Some(previous) = start.checked_sub(1).map(|end| line_start(source, end)) {
        let line = source.get(previous..start - 1).unwrap_or_default();
        if !line.trim_start().starts_with('#') {
            break;
        }
        start = previous;
    }
    start
}

/// Formats a YAML scalar: plain when YAML reads it back as the same string, and quoted otherwise.
fn scalar(text: &str) -> String {
    let plain = yaml_rust2::YamlLoader::load_from_str(&format!("key: {text}"))
        .ok()
        .and_then(|documents| documents.into_iter().next())
        .is_some_and(|document| document["key"].as_str() == Some(text));
    if plain { text.to_owned() } else { quoted(text) }
}

/// Formats a duration as the configuration writes it, such as `30s` or `1500ms`.
fn duration(duration: Duration) -> String {
    if duration.subsec_millis() == 0 {
        format!("{}s", duration.as_secs())
    } else {
        format!("{}ms", duration.as_millis())
    }
}

#[cold]
fn flow(block: &Block<'_>) -> Error {
    Error::at(
        Code::InvalidType,
        block.source,
        block.key.offset,
        format!(
            "uphill add copies a category written as a block mapping. Write `{}` with one key per line",
            block.category.name
        ),
    )
}

#[cold]
fn unknown(name: &str, pack: &Config) -> Error {
    let names: Vec<&str> = pack
        .categories
        .iter()
        .map(|category| category.name.as_str())
        .collect();
    Error {
        code: Code::InvalidValue,
        location: None,
        message: format!(
            "the pack declares no category `{name}`. It declares {}",
            names.join(", ")
        ),
    }
}
