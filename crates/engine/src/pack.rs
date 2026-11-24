//! Packs: categories that one project shares with others. A pack is a directory laid out like a
//! project, with an `uphill.yaml` and the scripts its `run` commands name, so any project or
//! example is a pack. `uphill add` copies a pack's categories into a project's `uphill.yaml` and
//! its scripts into the project, and the project then owns them.
//!
//! Each category keeps its text, so the comments that say what each metric measures come with it.
//! Its `run` command points at the copied scripts, and it takes the pack's top-level `run` and
//! `timeout` when it sets none. A comment above it records the command that added it and the
//! pack's commit, so `uphill add --update` fetches the pack again and merges its changes.
//!
//! ```yaml
//! # Added by `uphill add wevm/uphill/examples/web-app performance` at 3e0053b4f1c2….
//! performance:
//!   run: bun .agents/metrics/lighthouse.ts performance
//!   # Lighthouse's five metrics on each page, with Lighthouse's weights.
//!   first-contentful-paint: { target: 800ms, weight: 10 }
//! ```
//!
//! An update compares three versions of each category and each script: the pack at the recorded
//! commit, the project, and the pack now. A version that only one side changed wins, and a
//! version that both changed is a conflict.

use std::fmt::Write as _;
use std::time::Duration;

use crate::baseline::{quoted, yaml_key};
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
    /// The pack's categories to add, or every category when it is empty.
    pub categories: &'a [String],
    /// The pack's commit, or `None` for a local directory.
    pub commit: Option<&'a str>,
    /// The name to add the category under, which needs exactly one category.
    pub name: Option<&'a str>,
    /// The text of the pack's `uphill.yaml`.
    pub pack: &'a str,
    /// The text of the project's `uphill.yaml`, or `None` when the project has none yet. A new
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
    /// The pack's commit when the category was added or last updated, or `None` for a local
    /// directory.
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
    /// Where the category came from.
    pub origin: &'a Origin,
    /// The text of the project's `uphill.yaml`.
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
    let pack = config::parse(options.pack, Format::Yaml)?;
    let project = options
        .project
        .map(|source| config::parse(source, Format::Yaml))
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
                key_offset(options.pack, &category.name).unwrap_or_default(),
                format!(
                    "the project already declares category `{name}`. Add it under another name with `--as <name>`"
                ),
            ));
        }

        let run = (options.run)(&category.run);
        let Some(text) = render_category(options.pack, &pack, &category.name, name, &run)? else {
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
                .map(|run| format!("setup: {}\n\n", scalar(&run)))
                .unwrap_or_default();
            (format!("{line}{}\n", blocks.join("\n\n")), None)
        }
    };

    verify(&config, expected)?;
    Ok(Added {
        categories: selected
            .iter()
            .map(|category| options.name.unwrap_or(&category.name).to_owned())
            .collect(),
        config,
        setup,
    })
}

/// Returns the category `name` of the pack as `uphill add` writes it under `category`, without the
/// comment above it, or `None` when the pack does not declare it. `run` maps the category's
/// command in the pack to the project's.
///
/// # Errors
///
/// Returns an error when the pack's configuration is invalid or the category is a flow mapping.
pub fn render(
    pack: &str,
    name: &str,
    category: &str,
    run: &mut dyn FnMut(&str) -> String,
) -> Result<Option<String>, Error> {
    let config = config::parse(pack, Format::Yaml)?;
    let Some(found) = config.categories.iter().find(|c| c.name == name) else {
        return Ok(None);
    };
    let run = run(&found.run);
    render_category(pack, &config, name, category, &run)
}

/// Returns where each category that `uphill add` wrote came from, in the order the project
/// declares them.
pub fn origins(project: &str) -> Vec<Origin> {
    let Ok(Some(Node {
        kind: Kind::Mapping(pairs),
        ..
    })) = document::parse(project, Format::Yaml)
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
    })) = document::parse(project, Format::Yaml)
    else {
        return Err(missing());
    };
    let index = pairs
        .iter()
        .position(|(key, _)| key.text() == Some(origin.category.as_str()))
        .ok_or_else(missing)?;
    let key = pairs.get(index).map(|(key, _)| key).ok_or_else(missing)?;
    let start = attached(project, key.offset);
    let next = pairs
        .get(index + 1)
        .map_or(project.len(), |(key, _)| attached(project, key.offset));
    let end = trimmed_end(project, key.offset, next);

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
    let parsed = config::parse(&config, Format::Yaml).map_err(|error| Error {
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

/// Returns the comment above an added category: the command that adds it again, and the commit.
fn comment(source: &str, name: &str, category: &str, commit: Option<&str>) -> String {
    let rename = if name == category {
        String::new()
    } else {
        format!(" --as {category}")
    };
    let at = commit
        .map(|commit| format!(" at {commit}"))
        .unwrap_or_default();
    format!("{ADDED}{source} {name}{rename}`{at}.")
}

/// Returns the category `name` of a parsed pack as the project writes it under `category`.
fn render_category(
    source: &str,
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
    }) = document::parse(source, Format::Yaml)?
    else {
        return Ok(None);
    };
    let Some(index) = pairs.iter().position(|(key, _)| key.text() == Some(name)) else {
        return Ok(None);
    };
    let Some((key, value)) = pairs.get(index) else {
        return Ok(None);
    };
    let end = pairs
        .get(index + 1)
        .map_or(source.len(), |(key, _)| attached(source, key.offset));
    block(&Block {
        category: found,
        end,
        key,
        name: category,
        run,
        source,
        start: attached(source, key.offset),
        value,
    })
    .map(Some)
}

/// Checks that `config` parses and declares the `expected` categories as they are.
fn verify(config: &str, expected: Vec<Category>) -> Result<(), Error> {
    // The categories keep their text, so the configuration must parse to the same categories.
    let written = config::parse(config, Format::Yaml).map_err(|error| Error {
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
fn key_offset(source: &str, name: &str) -> Option<usize> {
    let Ok(Some(Node {
        kind: Kind::Mapping(pairs),
        ..
    })) = document::parse(source, Format::Yaml)
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
