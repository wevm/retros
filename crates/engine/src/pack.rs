//! Packs: categories that one project shares with others. A pack is a directory laid out like a
//! project, with an `uphill.yaml` and the scripts its `run` commands name, so any project or
//! example is a pack. `uphill add` copies a pack's categories into a project's `uphill.yaml` and
//! its scripts into the project, and the project then owns them.
//!
//! Each category keeps its text, so the comments that say what each metric measures come with it.
//! Its `run` command points at the copied scripts, and it takes the pack's top-level `run` and
//! `timeout` when it sets none. A comment above it names where it came from.
//!
//! ```yaml
//! # Added by `uphill add wevm/uphill/examples/web-app@3e0053b`.
//! performance:
//!   run: bun .agents/metrics/lighthouse.ts performance
//!   # Lighthouse's five metrics on each page, with Lighthouse's weights.
//!   first-contentful-paint: { target: 800ms, weight: 10 }
//! ```

use std::fmt::Write as _;
use std::time::Duration;

use crate::baseline::{quoted, yaml_key};
use crate::config::{self, Category, Config, Format, without_locations};
use crate::document::{self, Kind, Node};
use crate::edit::{after_colon, line_end, line_start};
use crate::error::{Code, Error};

/// Time a script may run when neither its category nor the configuration sets a `timeout`.
const DEFAULT_TIMEOUT: Duration = Duration::from_mins(10);

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
    /// The text of the comment above each added category, such as
    /// "Added by `uphill add wevm/uphill/examples/web-app@3e0053b`.".
    pub comment: &'a str,
    /// The name to add the category under, which needs exactly one category.
    pub name: Option<&'a str>,
    /// The text of the pack's `uphill.yaml`.
    pub pack: &'a str,
    /// The text of the project's `uphill.yaml`, or `None` when the project has none yet. A new
    /// configuration takes the pack's `setup`.
    pub project: Option<&'a str>,
    /// Returns the `run` command for the project, given each added category's command in the pack.
    pub run: &'a mut dyn FnMut(&str) -> String,
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

    let Some(Node {
        kind: Kind::Mapping(pairs),
        ..
    }) = document::parse(options.pack, Format::Yaml)?
    else {
        return Ok(Added {
            categories: Vec::new(),
            config: options.project.unwrap_or_default().to_owned(),
            setup: None,
        });
    };
    let starts: Vec<usize> = pairs
        .iter()
        .map(|(key, _)| attached(options.pack, key.offset))
        .collect();

    let mut blocks = Vec::new();
    let mut expected = Vec::new();
    for category in &selected {
        let name = options.name.unwrap_or(&category.name);
        let Some((index, (key, value))) = pairs
            .iter()
            .enumerate()
            .find(|(_, (key, _))| key.text() == Some(category.name.as_str()))
        else {
            continue;
        };
        if let Some(project) = &project
            && project.categories.iter().any(|other| other.name == name)
        {
            return Err(Error::at(
                Code::DuplicateCategory,
                options.pack,
                key.offset,
                format!(
                    "the project already declares category `{name}`. Add it under another name with `--as <name>`"
                ),
            ));
        }
        let end = starts.get(index + 1).copied().unwrap_or(options.pack.len());
        let run = (options.run)(&category.run);
        blocks.push(block(&Block {
            category,
            comment: options.comment,
            end,
            key,
            name,
            run: &run,
            source: options.pack,
            start: starts.get(index).copied().unwrap_or_default(),
            value,
        })?);

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

    // The categories keep their text, so the configuration must parse to the same categories.
    let written = config::parse(&config, Format::Yaml).map_err(|error| Error {
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

    Ok(Added {
        categories: selected
            .iter()
            .map(|category| options.name.unwrap_or(&category.name).to_owned())
            .collect(),
        config,
        setup,
    })
}

/// One category of the pack and how to write it into the project.
struct Block<'a> {
    category: &'a Category,
    comment: &'a str,
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

/// Returns a category's text for the project: its comment, its key, its `run` and `timeout`, and
/// its other lines as the pack writes them.
fn block(block: &Block<'_>) -> Result<String, Error> {
    let source = block.source;
    let Kind::Mapping(fields) = &block.value.kind else {
        return Err(flow(block));
    };
    if line_start(source, block.value.offset) == line_start(source, block.key.offset) {
        return Err(flow(block));
    }

    // Unindented comments at the end belong to whatever follows the category.
    let mut end = block.end;
    loop {
        let text = source.get(block.key.offset..end).unwrap_or_default();
        let trimmed = block.key.offset + text.trim_end().len();
        let last = line_start(source, trimmed);
        let line = source.get(last..trimmed).unwrap_or_default();
        if last > block.key.offset && line.starts_with('#') {
            end = last;
        } else {
            end = trimmed;
            break;
        }
    }

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
    Ok(format!("# {}\n{text}", block.comment))
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
