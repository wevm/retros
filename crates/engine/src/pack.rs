//! Packs: the categories a project runs from other configurations.
//!
//! The configuration's top-level `packs` key lists them. A remote entry names a directory in a Git
//! repository and the full commit that pins it: `owner/repo/path@<commit>` on GitHub, or an
//! `https` URL followed by `//path@<commit>`. A comment at the end of the entry's line names the
//! ref that `uphill update` follows. The command line vendors each remote pack into
//! `.agents/packs/<name>/`, so runs read only committed files. A local entry starts with `./` or
//! `../`, and the project's own commit pins it. `{ pack, categories, install }` takes only some of
//! a pack's categories, or approves the install of its dependencies. An optional top-level
//! `allowed-packs` key lists patterns, such as `wevm/*`, that every remote entry must match.
//!
//! ```yaml
//! packs:
//!   - wevm/uphill/packs/rust@3e0053b4f1c2a9e7d8b6c5a4f3e2d1c0b9a8f7e6 # main
//!   - { pack: ./.agents/packs/api, categories: [latency] }
//! ```
//!
//! The module also edits the `packs` key in place, validates the tree of a fetched commit, and
//! renders the review that `uphill add` and `uphill update` print (see [`crate::scan`]).

use crate::config::{Config, Format};
use crate::document::{self, Kind, Node};
use crate::edit;
use crate::error::{Code, Error, Location};

/// The most bytes in one file of a pack.
pub const MAX_FILE: u64 = 1_000_000;

/// The most files in a pack.
const MAX_FILES: usize = 1_000;

/// The most directory levels in a pack.
const MAX_DEPTH: usize = 32;

/// The most bytes in all files of a pack.
const MAX_TOTAL: u64 = 10_000_000;

/// One entry of the `packs` key.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    /// The categories to take from the pack, or `None` for every category.
    pub categories: Option<Vec<String>>,
    /// Whether the project approves installing the pack's dependencies.
    pub install: bool,
    /// Where the entry is in the project's configuration.
    pub location: Location,
    /// The pack as messages name it: the source without its commit, or the local directory.
    pub name: String,
    /// The byte offset of the entry in the project's configuration.
    pub offset: usize,
    /// The ref that `uphill update` follows, from the comment at the end of the entry's line, or
    /// `None` for the repository's default branch.
    pub follow: Option<String>,
    /// Where the pack comes from.
    pub source: Source,
}

impl Entry {
    /// Returns the name of the pack's directory: the last component of its path, or the
    /// repository's name for a pack at a repository's root.
    pub fn directory(&self) -> &str {
        match &self.source {
            Source::Local(path) => path
                .trim_end_matches('/')
                .rsplit('/')
                .next()
                .unwrap_or(path),
            Source::Remote(remote) => remote.directory(),
        }
    }
}

/// Where a pack comes from.
#[derive(Clone, Debug, PartialEq)]
pub enum Source {
    /// A directory relative to the project root, which starts with `./` or `../`.
    Local(String),
    /// A directory in a Git repository.
    Remote(Remote),
}

/// A directory in a Git repository at one ref.
#[derive(Clone, Debug, PartialEq)]
pub struct Remote {
    /// The path of the pack in the repository, without leading or trailing slashes, or empty for
    /// the repository's root.
    pub path: String,
    /// The ref as written after `@`: a full commit in a configuration, and also a branch or tag
    /// for `uphill add`. `None` names the default branch.
    pub reference: Option<String>,
    /// The repository: `owner/repo` on GitHub, or an `https` URL.
    pub repository: String,
}

impl Remote {
    /// Returns the URL that Git fetches, such as `https://github.com/wevm/uphill.git`.
    pub fn url(&self) -> String {
        if self.repository.starts_with("https://") {
            return self.repository.clone();
        }
        format!("https://github.com/{}.git", self.repository)
    }

    /// Returns the pack without its ref, such as `wevm/uphill/packs/rust`, which `allowed-packs`
    /// patterns match.
    pub fn name(&self) -> String {
        match (
            self.path.is_empty(),
            self.repository.starts_with("https://"),
        ) {
            (true, _) => self.repository.clone(),
            (false, true) => format!("{}//{}", self.repository, self.path),
            (false, false) => format!("{}/{}", self.repository, self.path),
        }
    }

    /// Returns the pack pinned to `commit`, as a configuration writes it.
    pub fn pinned(&self, commit: &str) -> String {
        format!("{}@{commit}", self.name())
    }

    /// Returns the name of the pack's directory: the last component of its path, or the
    /// repository's name.
    pub fn directory(&self) -> &str {
        let last = if self.path.is_empty() {
            &self.repository
        } else {
            &self.path
        };
        let name = last.rsplit('/').next().unwrap_or(last);
        name.strip_suffix(".git").unwrap_or(name)
    }
}

/// Returns whether `text` is a full commit: 40 or 64 lowercase hexadecimal digits.
pub fn is_commit(text: &str) -> bool {
    matches!(text.len(), 40 | 64)
        && text
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

/// Parses a pack's source: a local directory that starts with `./` or `../`, `owner/repo/path`
/// on GitHub, or an `https` URL followed by `//path`, each remote one with an optional `@ref`.
///
/// # Errors
///
/// Returns a message for a source of another form, such as an `http`, `ssh`, or `file` URL, a
/// source or ref that starts with `-`, and a path with `..`, `.git`, or characters other than
/// ASCII letters, digits, `.`, `_`, `+`, and `-`.
pub fn source(text: &str) -> Result<Source, String> {
    if text.starts_with("./") || text.starts_with("../") {
        if text.chars().any(char::is_control) {
            return Err(format!("`{text}` holds a control character"));
        }
        return Ok(Source::Local(text.to_owned()));
    }

    let expected = "use `owner/repo/path` on GitHub or an `https` URL followed by `//path`, each with an optional `@ref`";
    let (rest, reference) = match text.rsplit_once('@') {
        Some((rest, reference)) if !reference.contains(':') => (rest, Some(reference)),
        _ => (text, None),
    };
    if let Some(reference) = reference {
        let valid = reference
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._/+-".contains(&byte));
        if reference.is_empty()
            || reference.starts_with(['-', '/'])
            || reference.contains("..")
            || !valid
        {
            return Err(format!("`{reference}` is not a branch, tag, or commit"));
        }
    }

    let (repository, path) = if let Some(url) = rest.strip_prefix("https://") {
        let (repository, path) = url.split_once("//").unwrap_or((url, ""));
        let mut parts = repository.split('/');
        let host = parts.next().unwrap_or_default();
        if host.is_empty()
            || host.contains('@')
            || !parts.clone().all(component)
            || parts.count() == 0
        {
            return Err(format!("`{text}` is not a pack source: {expected}"));
        }
        (format!("https://{repository}"), path)
    } else if rest.contains(':') {
        return Err(format!("`{text}` is not a pack source: {expected}"));
    } else {
        let mut parts = rest.splitn(3, '/');
        let (Some(owner), Some(repo)) = (parts.next(), parts.next()) else {
            return Err(format!("`{text}` is not a pack source: {expected}"));
        };
        if !component(owner) || !component(repo) {
            return Err(format!("`{text}` is not a pack source: {expected}"));
        }
        (format!("{owner}/{repo}"), parts.next().unwrap_or_default())
    };

    let path = path.trim_matches('/');
    if !path.is_empty() && !path.split('/').all(component) {
        return Err(format!(
            "`{path}` is not a path in a repository: {expected}"
        ));
    }
    Ok(Source::Remote(Remote {
        path: path.to_owned(),
        reference: reference.map(str::to_owned),
        repository,
    }))
}

/// Returns whether `part` is a safe component of a path or repository name.
fn component(part: &str) -> bool {
    !part.is_empty()
        && !part.starts_with(['-', '.'])
        && !part.eq_ignore_ascii_case(".git")
        && part
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._+-".contains(&byte))
}

/// A pack that the command line found and parsed: its entry and its configuration, with each
/// `run` command rewritten to start at the project root, and the default targets of the pack's
/// baseline file.
#[derive(Clone, Debug, PartialEq)]
pub struct Pack {
    /// The pack's configuration.
    pub config: Config,
    /// The entry that lists the pack.
    pub entry: Entry,
}

/// Returns the entries of the `packs` key of a configuration, in the order it lists them, or none
/// when it has no `packs` key.
///
/// # Errors
///
/// Returns an error for YAML or TOML syntax, a `packs` value that is not a list, an entry that is
/// not a source or a mapping with `pack` and the optional `categories` and `install`, an invalid
/// source, a remote entry without a full commit, and a remote entry that no pattern of
/// `allowed-packs` matches.
pub fn entries(source: &str, format: Format) -> Result<Vec<Entry>, Error> {
    let Some(document) = document::parse(source, format)? else {
        return Ok(Vec::new());
    };
    let Kind::Mapping(pairs) = &document.kind else {
        return Ok(Vec::new());
    };
    let allowed = allowed(source, pairs, format)?;
    let Some((_, value)) = pairs.iter().find(|(key, _)| key.text() == Some("packs")) else {
        return Ok(Vec::new());
    };
    let items = match &value.kind {
        Kind::Sequence(items) => items.as_slice(),
        _ if value.is_null() => &[],
        _ => return Err(invalid(source, value, format)),
    };

    let mut entries: Vec<Entry> = Vec::new();
    for item in items {
        let entry = match &item.kind {
            Kind::Scalar { text, .. } => entry(source, item, text, None, false)?,
            Kind::Mapping(fields) => mapping(source, item, fields, format)?,
            Kind::Sequence(_) => return Err(invalid(source, item, format)),
        };
        if let Source::Remote(remote) = &entry.source {
            if !remote.reference.as_deref().is_some_and(is_commit) {
                return Err(Error::at(
                    Code::Unpinned,
                    source,
                    item.offset,
                    format!(
                        "pack `{}` must be pinned to a full commit, such as `{}`. `uphill add {}` writes the pin",
                        entry.name,
                        remote.pinned("<40-character commit>"),
                        entry.name,
                    ),
                ));
            }
            if let Some(patterns) = &allowed
                && !patterns.iter().any(|pattern| matches(pattern, &entry.name))
            {
                return Err(Error::at(
                    Code::NotAllowed,
                    source,
                    item.offset,
                    format!(
                        "pack `{}` matches no pattern of `allowed-packs`: {}",
                        entry.name,
                        patterns.join(", ")
                    ),
                ));
            }
        }
        let remote = |entry: &Entry| matches!(entry.source, Source::Remote(_));
        if let Some(other) = entries
            .iter()
            .find(|other| remote(other) && remote(&entry) && other.directory() == entry.directory())
        {
            return Err(Error::at(
                Code::InvalidValue,
                source,
                item.offset,
                format!(
                    "packs `{}` and `{}` both vendor to the directory `{}`. Keep one",
                    other.name,
                    entry.name,
                    entry.directory()
                ),
            ));
        }
        entries.push(entry);
    }
    Ok(entries)
}

/// Returns whether `allowed-packs` permits the remote pack `name` in a configuration, which is
/// true when the configuration has no `allowed-packs` key.
///
/// # Errors
///
/// Returns an error for YAML or TOML syntax and an `allowed-packs` value that is not a list of
/// patterns.
pub fn permits(source: &str, format: Format, name: &str) -> Result<bool, Error> {
    let Some(document) = document::parse(source, format)? else {
        return Ok(true);
    };
    let Kind::Mapping(pairs) = &document.kind else {
        return Ok(true);
    };
    Ok(allowed(source, pairs, format)?
        .is_none_or(|patterns| patterns.iter().any(|pattern| matches(pattern, name))))
}

/// Returns the patterns of the top-level `allowed-packs` key, or `None` without one.
fn allowed(
    source: &str,
    pairs: &[(Node, Node)],
    format: Format,
) -> Result<Option<Vec<String>>, Error> {
    let Some((_, value)) = pairs
        .iter()
        .find(|(key, _)| key.text() == Some("allowed-packs"))
    else {
        return Ok(None);
    };
    let message = || {
        Error::at(
            Code::InvalidType,
            source,
            value.offset,
            format.pick(
                "`allowed-packs` must list patterns of pack sources, such as `allowed-packs: [wevm/*]`".to_owned(),
                "`allowed-packs` must list patterns of pack sources, such as `allowed-packs = [\"wevm/*\"]`".to_owned(),
            ),
        )
    };
    let Kind::Sequence(items) = &value.kind else {
        return Err(message());
    };
    items
        .iter()
        .map(|item| item.text().map(str::to_owned))
        .collect::<Option<Vec<String>>>()
        .map(Some)
        .ok_or_else(message)
}

/// Returns whether `name` matches `pattern`, where each `*` matches any run of characters.
fn matches(pattern: &str, name: &str) -> bool {
    let mut parts = pattern.split('*');
    let first = parts.next().unwrap_or_default();
    let Some(mut rest) = name.strip_prefix(first) else {
        return false;
    };
    let parts: Vec<&str> = parts.collect();
    let Some((last, middle)) = parts.split_last() else {
        return rest.is_empty();
    };
    for part in middle {
        let Some(at) = rest.find(part) else {
            return false;
        };
        rest = rest.get(at + part.len()..).unwrap_or_default();
    }
    rest.ends_with(last)
}

/// Parses an entry written as a mapping: `{ pack: <source>, categories: [...], install: true }`.
fn mapping(
    source: &str,
    item: &Node,
    fields: &[(Node, Node)],
    format: Format,
) -> Result<Entry, Error> {
    let mut categories = None;
    let mut install = false;
    let mut pack = None;
    for (key, value) in fields {
        match key.text() {
            Some("categories") => {
                let Kind::Sequence(names) = &value.kind else {
                    return Err(invalid(source, value, format));
                };
                let names = names
                    .iter()
                    .map(|node| node.text().map(str::to_owned))
                    .collect::<Option<Vec<String>>>()
                    .ok_or_else(|| invalid(source, value, format))?;
                categories = Some(names);
            }
            Some("install") => {
                install = match value.text() {
                    Some("true") => true,
                    Some("false") => false,
                    _ => {
                        return Err(Error::at(
                            Code::InvalidType,
                            source,
                            value.offset,
                            "`install` must be `true` or `false`".to_owned(),
                        ));
                    }
                };
            }
            Some("pack") => {
                pack = Some(value.text().ok_or_else(|| invalid(source, value, format))?);
            }
            _ => {
                return Err(Error::at(
                    Code::UnexpectedKey,
                    source,
                    key.offset,
                    format!(
                        "unknown key `{}` in a pack, expected `pack`, `categories`, or `install`",
                        key.text().unwrap_or_default()
                    ),
                ));
            }
        }
    }

    let pack = pack.ok_or_else(|| invalid(source, item, format))?;
    entry(source, item, pack, categories, install)
}

/// Builds the entry at `item` for the source `text`.
fn entry(
    source: &str,
    item: &Node,
    text: &str,
    categories: Option<Vec<String>>,
    install: bool,
) -> Result<Entry, Error> {
    let parsed = self::source(text)
        .map_err(|message| Error::at(Code::InvalidValue, source, item.offset, message))?;
    let name = match &parsed {
        Source::Local(path) => path.clone(),
        Source::Remote(remote) => remote.name(),
    };
    Ok(Entry {
        categories,
        follow: comment(source, item.offset),
        install,
        location: Location::of(source, item.offset),
        name,
        offset: item.offset,
        source: parsed,
    })
}

/// Returns the text of the comment at the end of the line that holds `offset`, when it is a ref.
fn comment(source: &str, offset: usize) -> Option<String> {
    let line = source.get(offset..edit::line_end(source, offset))?;
    let at = line.match_indices('#').find(|(at, _)| {
        line.get(..*at)
            .is_some_and(|before| before.ends_with([' ', '\t']))
    })?;
    let text = line.get(at.0 + 1..)?.trim();
    let valid = text
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || b"._/+-".contains(&byte));
    (!text.is_empty() && !text.starts_with('-') && valid).then(|| text.to_owned())
}

/// Returns the configuration `source` with a new entry at the end of its `packs` key, and the key
/// itself when it has none. The entry is `pack`, with its `categories` and `install`, and the
/// comment `follow` that names the ref `uphill update` follows.
///
/// # Errors
///
/// Returns an error when the configuration does not parse, and when the `packs` key has a form
/// this editor does not change, such as a flow sequence, so the entry must be added by hand.
pub fn add(
    source: &str,
    format: Format,
    pack: &str,
    categories: &[String],
    install: bool,
    follow: Option<&str>,
) -> Result<String, Error> {
    let before = entries(source, format)?;
    let value = render(format, pack, categories, install);
    let comment = follow
        .map(|follow| format!(" # {follow}"))
        .unwrap_or_default();
    let by_hand = || Error {
        code: Code::InvalidValue,
        location: None,
        message: format!(
            "could not add the pack to the `packs` key. Add `{value}{comment}` to it by hand"
        ),
    };

    let out = match (format, before.last()) {
        (Format::Yaml, None) => {
            let key = top_level(source, format, "packs");
            match key {
                // An empty `packs:` key takes the entry on the next line.
                Some(offset) => {
                    let end = edit::line_end(source, offset);
                    let line = source.get(offset..end).unwrap_or_default();
                    if line.trim_end() != "packs:" {
                        return Err(by_hand());
                    }
                    insert(source, end, &format!("\n  - {value}{comment}"))
                }
                None => format!("{}\npacks:\n  - {value}{comment}\n", with_newline(source)),
            }
        }
        (Format::Yaml, Some(last)) => {
            // A flow mapping's offset is at its first key, so the prefix ends at the `- `.
            let start = edit::line_start(source, last.offset);
            let line = source.get(start..last.offset).unwrap_or_default();
            let indent = line.len() - line.trim_start().len();
            if !line.trim_start().starts_with("- ") {
                return Err(by_hand());
            }
            let prefix = line.get(..indent + 2).unwrap_or_default();
            let end = item_end(source, last.offset, indent);
            insert(source, end, &format!("\n{prefix}{value}{comment}"))
        }
        (Format::Toml, None) => {
            if top_level(source, format, "packs").is_some() {
                return Err(by_hand());
            }
            // Top-level keys come before the first table.
            let table = source
                .match_indices('[')
                .map(|(at, _)| at)
                .find(|at| edit::line_start(source, *at) == *at);
            // The comment lines above a table belong to it.
            let table = table.map(|mut at| {
                while at > 0 {
                    let start = edit::line_start(source, at - 1);
                    if !source
                        .get(start..at)
                        .is_some_and(|line| line.starts_with('#'))
                    {
                        break;
                    }
                    at = start;
                }
                at
            });
            let text = format!("packs = [\n  {value},{comment}\n]\n");
            match table {
                Some(at) => insert(source, at, &format!("{text}\n")),
                None => format!("{}\n{text}", with_newline(source)),
            }
        }
        (Format::Toml, Some(last)) => {
            let end = edit::line_end(source, last.offset);
            let line = source.get(last.offset..end).unwrap_or_default();
            let code = line.split(" #").next().unwrap_or(line).trim_end();
            if !code.ends_with(',') {
                return Err(by_hand());
            }
            let start = edit::line_start(source, last.offset);
            let indent = source.get(start..last.offset).unwrap_or_default();
            insert(source, end, &format!("\n{indent}{value},{comment}"))
        }
    };

    let out = with_newline(&out);
    let after = entries(&out, format).map_err(|_| by_hand())?;
    let same = after.len() == before.len() + 1
        && after.iter().zip(&before).all(|(a, b)| a.name == b.name)
        && after
            .last()
            .is_some_and(|entry| entry.follow.as_deref() == follow);
    if same { Ok(out) } else { Err(by_hand()) }
}

/// Returns the configuration `source` with the commit of `entry` replaced by `commit`.
///
/// # Errors
///
/// Returns an error when the entry's commit is not on its line, so the pin must change by hand.
pub fn pin(source: &str, format: Format, entry: &Entry, commit: &str) -> Result<String, Error> {
    let Source::Remote(remote) = &entry.source else {
        return Ok(source.to_owned());
    };
    let old = format!("@{}", remote.reference.as_deref().unwrap_or_default());
    let end = edit::line_end(source, entry.offset);
    let by_hand = || Error {
        code: Code::InvalidValue,
        location: Some(entry.location),
        message: format!(
            "could not pin `{}`. Write `{}` by hand",
            entry.name,
            remote.pinned(commit)
        ),
    };
    let at = source
        .get(entry.offset..end)
        .and_then(|line| line.find(&old))
        .ok_or_else(by_hand)?;
    let start = entry.offset + at;
    let out = format!(
        "{}@{commit}{}",
        source.get(..start).unwrap_or_default(),
        source.get(start + old.len()..).unwrap_or_default()
    );

    let after = entries(&out, format).map_err(|_| by_hand())?;
    let pinned = after.iter().any(|other| {
        other.name == entry.name
            && matches!(&other.source, Source::Remote(r) if r.reference.as_deref() == Some(commit))
    });
    if pinned { Ok(out) } else { Err(by_hand()) }
}

/// Returns the configuration `source` without `entry`.
///
/// # Errors
///
/// Returns an error when the entry shares its lines with other text, so it must be removed by
/// hand.
pub fn remove(source: &str, format: Format, entry: &Entry) -> Result<String, Error> {
    let before = entries(source, format)?;
    let start = edit::line_start(source, entry.offset);
    let indent = source
        .get(start..entry.offset)
        .map_or(0, |prefix| prefix.len() - prefix.trim_start().len());
    let end = match format {
        Format::Yaml => item_end(source, entry.offset, indent),
        Format::Toml => edit::line_end(source, entry.offset),
    };
    let end = (end + 1).min(source.len());
    let out = format!(
        "{}{}",
        source.get(..start).unwrap_or_default(),
        source.get(end..).unwrap_or_default()
    );

    let by_hand = || Error {
        code: Code::InvalidValue,
        location: Some(entry.location),
        message: format!(
            "could not remove `{}` from the `packs` key. Remove it by hand",
            entry.name
        ),
    };
    let after = entries(&out, format).map_err(|_| by_hand())?;
    let expected: Vec<&str> = before
        .iter()
        .filter(|other| other.offset != entry.offset)
        .map(|other| other.name.as_str())
        .collect();
    let names: Vec<&str> = after.iter().map(|other| other.name.as_str()).collect();
    if names == expected {
        Ok(out)
    } else {
        Err(by_hand())
    }
}

/// Renders an entry as the configuration writes it.
fn render(format: Format, pack: &str, categories: &[String], install: bool) -> String {
    let quote = |text: &str| match format {
        Format::Toml => format!("\"{text}\""),
        Format::Yaml => text.to_owned(),
    };
    if categories.is_empty() && !install {
        return quote(pack);
    }
    let (assign, open, close) = match format {
        Format::Toml => (" = ", "{ ", " }"),
        Format::Yaml => (": ", "{ ", " }"),
    };
    let mut fields = vec![format!("pack{assign}{}", quote(pack))];
    if !categories.is_empty() {
        let names: Vec<String> = categories.iter().map(|name| quote(name)).collect();
        fields.push(format!("categories{assign}[{}]", names.join(", ")));
    }
    if install {
        fields.push(format!("install{assign}true"));
    }
    format!("{open}{}{close}", fields.join(", "))
}

/// Returns the offset of the top-level key `name`, or `None` without one.
fn top_level(source: &str, format: Format, name: &str) -> Option<usize> {
    let document = document::parse(source, format).ok()??;
    let Kind::Mapping(pairs) = &document.kind else {
        return None;
    };
    pairs
        .iter()
        .find(|(key, _)| key.text() == Some(name))
        .map(|(key, _)| key.offset)
}

/// Returns the end of the YAML sequence item at `offset`, whose `-` is at column `indent`: the
/// end of its last line that is indented more than the `-`.
fn item_end(source: &str, offset: usize, indent: usize) -> usize {
    let mut end = edit::line_end(source, offset);
    while end < source.len() {
        let next = edit::line_end(source, end + 1);
        let line = source.get(end + 1..next).unwrap_or_default();
        let content = line.trim_start();
        if content.is_empty() || line.len() - content.len() <= indent {
            break;
        }
        end = next;
    }
    end
}

/// Returns `source` with `text` inserted at `at`.
fn insert(source: &str, at: usize, text: &str) -> String {
    format!(
        "{}{text}{}",
        source.get(..at).unwrap_or_default(),
        source.get(at..).unwrap_or_default()
    )
}

/// Returns `source` ending in a newline, or empty.
fn with_newline(source: &str) -> String {
    if source.is_empty() || source.ends_with('\n') {
        return source.to_owned();
    }
    format!("{source}\n")
}

/// A file in the tree of a fetched commit, as `git ls-tree` lists it.
#[derive(Clone, Debug, PartialEq)]
pub struct TreeEntry {
    /// The Git mode, such as `100644`.
    pub mode: String,
    /// The path relative to the pack's directory.
    pub path: String,
    /// The size in bytes, or `None` when Git did not fetch the object because it is too large.
    pub size: Option<u64>,
}

/// Checks the tree of a fetched pack before any file is written: only regular files, with safe
/// paths, within the limits on files, depth, and size. Returns whether each file is executable,
/// in the order of `entries`.
///
/// # Errors
///
/// Returns a message for a symbolic link, a submodule, a path with `..`, `.git`, or characters
/// other than ASCII letters, digits, `.`, `_`, `@`, `+`, and `-`, two paths that differ only in
/// case, more than 1,000 files or 32 levels, a file over 1 MB, and more than 10 MB in all.
pub fn validate(entries: &[TreeEntry]) -> Result<Vec<bool>, String> {
    if entries.len() > MAX_FILES {
        return Err(format!(
            "it has {} files, more than {MAX_FILES}",
            entries.len()
        ));
    }

    let mut total = 0_u64;
    let mut seen: Vec<String> = Vec::new();
    let mut executable = Vec::new();
    for entry in entries {
        let path = &entry.path;
        match entry.mode.as_str() {
            "100644" => executable.push(false),
            "100755" => executable.push(true),
            "120000" => return Err(format!("{} is a symbolic link", printable(path))),
            "160000" => return Err(format!("{} is a submodule", printable(path))),
            mode => {
                return Err(format!(
                    "{} has the mode {mode}, not a regular file",
                    printable(path)
                ));
            }
        }

        let parts: Vec<&str> = path.split('/').collect();
        let safe = parts.iter().all(|part| {
            !part.is_empty()
                && *part != "."
                && *part != ".."
                && !part.eq_ignore_ascii_case(".git")
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"._@+-".contains(&byte))
        });
        if !safe {
            return Err(format!("the path {} is not allowed", printable(path)));
        }
        if parts.len() > MAX_DEPTH {
            return Err(format!(
                "{} is more than {MAX_DEPTH} levels deep",
                printable(path)
            ));
        }

        let folded = path.to_ascii_lowercase();
        if let Some(other) = seen.iter().position(|other| *other == folded) {
            let other = entries.get(other).map_or("", |other| other.path.as_str());
            return Err(format!("{other} and {path} differ only in case"));
        }
        seen.push(folded);

        let size = entry
            .size
            .filter(|size| *size <= MAX_FILE)
            .ok_or_else(|| format!("{path} is larger than 1 MB"))?;
        total += size;
        if total > MAX_TOTAL {
            return Err("its files hold more than 10 MB".to_owned());
        }
    }
    Ok(executable)
}

/// Returns `text` without control, bidirectional, and zero-width characters, so pack text that
/// reaches a terminal cannot move the cursor or reorder what it shows. Newlines and tabs stay.
pub fn printable(text: &str) -> String {
    text.chars()
        .filter(|c| matches!(c, '\n' | '\t') || !(c.is_control() || hidden(*c)))
        .collect()
}

/// Returns whether `c` is a bidirectional control or a zero-width character.
pub(crate) fn hidden(c: char) -> bool {
    matches!(c, '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2060}'..='\u{2064}' | '\u{2066}'..='\u{2069}' | '\u{feff}')
}

#[cold]
fn invalid(source: &str, node: &Node, format: Format) -> Error {
    Error::at(
        Code::InvalidType,
        source,
        node.offset,
        format.pick(
            "`packs` must list pack sources, such as `packs: [owner/repo/path@<commit>, { pack: ./packs/api, categories: [latency] }]`".to_owned(),
            "`packs` must list pack sources, such as `packs = [\"owner/repo/path@<commit>\", { pack = \"./packs/api\", categories = [\"latency\"] }]`".to_owned(),
        ),
    )
}
