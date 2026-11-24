//! `uphill add <pack> [category...]`: copies a pack's categories into the project's configuration
//! and the scripts their `run` commands name into the project, which then owns them.
//! `uphill add --update [category...]` fetches the pack of each added category again and merges
//! its changes.
//!
//! A pack is any directory with an `uphill.yaml` or `uphill.toml`, at its root or in `.agents/`,
//! such as another project or one of Uphill's examples. It is a local directory, `owner/name/path`
//! on GitHub, or a Git URL or repository path followed by `//path`, each with an optional `@ref`.
//! Uphill fetches a remote pack into a temporary directory with Git's own credentials and removes
//! it afterwards. A local directory is recorded relative to the project root when they share a
//! parent directory, and as an absolute path otherwise.
//!
//! Scripts go beside the project's configuration, in `.agents/` for a project without one, under
//! the path the pack gives them. A script in a directory, such as `metrics/lighthouse.ts`, brings
//! the whole directory, so the files it imports come too. Uphill writes nothing when a copied file
//! would replace a different one, or when an update finds a conflict.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::{self, Write as _};
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;

use engine::config::Format;
use engine::pack::{self, Merge, Options, Origin, Update};
use tempfile::TempDir;

use crate::project;
use crate::worktree;

/// Where a pack keeps its configuration, relative to its directory, and the format of each file.
const PLACES: [(&str, Format); 4] = [
    ("uphill.yaml", Format::Yaml),
    ("uphill.toml", Format::Toml),
    (".agents/uphill.yaml", Format::Yaml),
    (".agents/uphill.toml", Format::Toml),
];

/// A pack's directory and configuration, and the temporary directory that holds a fetched pack.
struct Pack {
    /// The commit of a fetched pack, or `None` for a local directory.
    commit: Option<String>,
    /// The directory that holds the pack's configuration or `.agents/`.
    dir: PathBuf,
    /// The format of the pack's configuration.
    format: Format,
    /// The name of the pack's configuration file, such as `uphill.toml`.
    file: &'static str,
    /// The directory of the pack's configuration relative to `dir`: empty or `.agents`. Paths in
    /// `run` commands start at `dir`.
    place: PathBuf,
    /// The text of the pack's configuration.
    source: String,
    _temp: Option<TempDir>,
}

/// The scripts that one pack's `run` commands name, and where they go in the project.
struct Scripts<'a> {
    /// Each file or directory to copy from the pack, and its place in the project.
    copies: BTreeMap<PathBuf, PathBuf>,
    /// The directory of the project's configuration, where scripts go.
    destination: &'a Path,
    pack: &'a Pack,
    root: &'a Path,
}

impl Scripts<'_> {
    /// Returns `run` with each script it names in the pack replaced by its path in the project.
    fn rewrite(&mut self, run: &str) -> String {
        run.split(' ')
            .map(|token| {
                let word = token.trim_matches(['"', '\'']);
                let path = Path::new(word);
                let Ok(inner) = path.strip_prefix(&self.pack.place) else {
                    return token.to_owned();
                };
                let local = path.components().all(|c| matches!(c, Component::Normal(_)));
                if word.is_empty() || !local || !self.pack.dir.join(path).is_file() {
                    return token.to_owned();
                }

                // A script in a directory brings the directory, and a script beside the
                // configuration comes alone.
                let unit = match inner.components().next() {
                    Some(first) if inner.components().count() > 1 => {
                        PathBuf::from(first.as_os_str())
                    }
                    _ => inner.to_path_buf(),
                };
                self.copies.insert(
                    self.pack.dir.join(&self.pack.place).join(&unit),
                    self.destination.join(&unit),
                );
                token.replace(word, &relative(&self.destination.join(inner), self.root))
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Returns each file to copy, by its path in the project, with its path in the pack.
    fn files(&self) -> Result<BTreeMap<PathBuf, PathBuf>, String> {
        let mut files = BTreeMap::new();
        for (from, to) in &self.copies {
            collect(from, to, &mut files)?;
        }
        Ok(files)
    }
}

/// Adds the pack's categories, or the ones `categories` names, to the project in the current
/// directory, under `name` when it is set.
///
/// # Errors
///
/// Returns a message for the terminal when the pack cannot be fetched or has no configuration,
/// when either configuration is invalid, when a category cannot be added, or when a
/// file cannot be read or written.
pub(crate) fn run(
    source: &str,
    categories: &[String],
    name: Option<&str>,
) -> Result<ExitCode, String> {
    let cwd = std::env::current_dir()
        .map_err(|error| format!("error: could not read the current directory: {error}"))?;
    let existing = project::exists(&cwd)
        .then(|| project::find(&cwd))
        .transpose()?;
    let pack = fetch(source, &cwd, None)?;
    // A new configuration takes the pack's format.
    let (root, config_path, format) = match &existing {
        Some(project) => (project.root.clone(), project.path.clone(), project.format),
        None => (
            cwd.clone(),
            cwd.join(".agents").join(pack.file),
            pack.format,
        ),
    };
    let destination = config_path.parent().unwrap_or(&root).to_path_buf();
    let config_name = relative(&config_path, &root);

    // A local directory is recorded from the project root, so `--update` finds it from anywhere.
    let recorded = if local(source, &cwd) {
        relative_to(&cwd.join(source), &root)
    } else {
        source.to_owned()
    };
    let mut scripts = Scripts {
        copies: BTreeMap::new(),
        destination: &destination,
        pack: &pack,
        root: &root,
    };
    let added = pack::add(&mut Options {
        categories,
        commit: pack.commit.as_deref(),
        format,
        name,
        pack: &pack.source,
        pack_format: pack.format,
        project: existing.as_ref().map(|project| project.source.as_str()),
        run: &mut |run| scripts.rewrite(run),
        source: &recorded,
    })
    .map_err(|error| diagnostic(&error, source, &pack))?;

    let files = scripts.files()?;
    for (to, from) in &files {
        if std::fs::read(to).is_ok_and(|bytes| std::fs::read(from).ok() != Some(bytes)) {
            return Err(format!(
                "error: {} exists and differs from the pack's copy. Move it, then run `uphill add` again",
                relative(to, &root)
            ));
        }
    }
    for (to, from) in &files {
        let bytes = std::fs::read(from)
            .map_err(|error| format!("error: could not read {}: {error}", from.display()))?;
        write(to, &bytes, &root)?;
    }
    write(&config_path, added.config.as_bytes(), &root)?;

    let mut out = format!("Added {} to {config_name}\n", added.categories.join(", "));
    for to in scripts.copies.values() {
        let shown = relative(to, &root);
        let slash = if to.is_dir() { "/" } else { "" };
        let _ = writeln!(out, "Added {shown}{slash}");
    }
    if let Some(setup) = &added.setup {
        let _ = writeln!(
            out,
            "\nThe pack prepares each checkout with `setup: {setup}`. Add it to {config_name} if the scripts need it"
        );
    }
    if let Ok(readme) = std::fs::read_to_string(pack.dir.join(&pack.place).join("README.md")) {
        let _ = write!(out, "\n{}\n", readme.trim_end());
    }
    out.push_str("\nFit the scripts and targets to this project, then run `uphill check`\n");
    let _ = io::stdout().write_all(out.as_bytes());
    Ok(ExitCode::SUCCESS)
}

/// One added category's merge: its new configuration and the files it copies.
struct Merged {
    merge: Merge,
    origin: Origin,
    same: bool,
    /// The pack's commits before and after, shortened.
    commits: (Option<String>, Option<String>),
}

/// Fetches the pack of each added category, or of the ones `categories` names, and merges its
/// changes into the project. The comment above each category records the pack's new commit.
///
/// # Errors
///
/// Returns a message for the terminal when no category came from
/// `uphill add`, when a pack cannot be fetched, or when a file cannot be read or written.
pub(crate) fn update(categories: &[String], force: bool) -> Result<ExitCode, String> {
    let cwd = std::env::current_dir()
        .map_err(|error| format!("error: could not read the current directory: {error}"))?;
    let project = project::find(&cwd)?;
    let root = &project.root;
    let destination = project.path.parent().unwrap_or(root).to_path_buf();

    let origins = pack::origins(&project.source, project.format);
    if let Some(name) = categories
        .iter()
        .find(|name| !origins.iter().any(|origin| origin.category == **name))
    {
        return Err(format!(
            "error: category '{name}' in {} did not come from `uphill add`",
            project.name
        ));
    }
    let origins: Vec<Origin> = origins
        .into_iter()
        .filter(|origin| categories.is_empty() || categories.contains(&origin.category))
        .collect();
    if origins.is_empty() {
        return Err(format!(
            "error: no category in {} came from `uphill add`",
            project.name
        ));
    }

    let mut config = project.source.clone();
    let mut merged = Vec::new();
    // Each file in the project, with its versions in the pack at the recorded commit and now.
    let mut files: BTreeMap<PathBuf, (Option<PathBuf>, Option<PathBuf>)> = BTreeMap::new();
    let mut packs = Vec::new();

    for origin in origins {
        let theirs = fetch(&origin.source, root, None)?;
        let base = origin
            .commit
            .as_deref()
            .map(|commit| fetch(&origin.source, root, Some(commit)))
            .transpose()?;

        let mut ours = Scripts {
            copies: BTreeMap::new(),
            destination: &destination,
            pack: &theirs,
            root,
        };
        let rendered = pack::render(
            (&theirs.source, theirs.format),
            &origin.name,
            &origin.category,
            project.format,
            &mut |run| ours.rewrite(run),
        )
        .map_err(|error| diagnostic(&error, &origin.source, &theirs))?
        .ok_or_else(|| {
            format!(
                "error: {} no longer declares category '{}'",
                origin.source, origin.name
            )
        })?;
        let mut before = None;
        let mut old = BTreeMap::new();
        if let Some(base) = &base {
            let mut scripts = Scripts {
                copies: BTreeMap::new(),
                destination: &destination,
                pack: base,
                root,
            };
            before = pack::render(
                (&base.source, base.format),
                &origin.name,
                &origin.category,
                project.format,
                &mut |run| scripts.rewrite(run),
            )
            .ok()
            .flatten();
            old = scripts.files()?;
        }

        let updated = pack::update(&Update {
            base: before.as_deref(),
            commit: theirs.commit.as_deref(),
            force,
            format: project.format,
            origin: &origin,
            project: &config,
            theirs: &rendered,
        })
        .map_err(|error| crate::diagnostic("error", &error, &project.name, &config))?;
        config = updated.config;

        for (to, from) in ours.files()? {
            files.entry(to).or_default().1 = Some(from);
        }
        for (to, from) in old {
            files.entry(to).or_default().0 = Some(from);
        }
        merged.push(Merged {
            commits: (
                origin.commit.as_deref().map(short),
                theirs.commit.as_deref().map(short),
            ),
            merge: updated.merge,
            origin,
            same: updated.same,
        });
        packs.push((theirs, base));
    }

    let read = |path: &Option<PathBuf>| path.as_ref().and_then(|path| std::fs::read(path).ok());
    let mut writes = Vec::new();
    let mut conflicts = Vec::new();
    for (to, (base, theirs)) in &files {
        let (base, ours, theirs) = (read(base), std::fs::read(to).ok(), read(theirs));
        match pack::merge(base.as_deref(), ours.as_deref(), theirs.as_deref()) {
            Merge::Keep => {}
            Merge::Conflict if !force => conflicts.push(relative(to, root)),
            Merge::Conflict | Merge::Take => writes.push((to, theirs)),
        }
    }
    conflicts.splice(
        0..0,
        merged
            .iter()
            .filter(|merged| merged.merge == Merge::Conflict)
            .map(|merged| merged.origin.category.clone()),
    );

    let mut out = String::new();
    if !conflicts.is_empty() {
        for conflict in &conflicts {
            let _ = writeln!(
                out,
                "conflict  {conflict}  changed in both this project and the pack"
            );
        }
        let _ = writeln!(
            out,
            "\nNothing was written. Merge by hand, or run `uphill add --update --force` to take the pack's version"
        );
        let _ = io::stdout().write_all(out.as_bytes());
        return Ok(ExitCode::FAILURE);
    }

    for (to, contents) in &writes {
        match contents {
            Some(bytes) => write(to, bytes, root)?,
            None => std::fs::remove_file(to).map_err(|error| {
                format!("error: could not remove {}: {error}", relative(to, root))
            })?,
        }
    }
    if config != project.source {
        write(&project.path, config.as_bytes(), root)?;
    }

    for merged in &merged {
        let category = &merged.origin.category;
        let commits = match &merged.commits {
            (Some(old), Some(new)) if old != new => format!(" from {old} to {new}"),
            (_, Some(new)) => format!(" at {new}"),
            _ => String::new(),
        };
        let _ = match (merged.merge, merged.same) {
            (Merge::Take, _) => writeln!(out, "updated   {category}{commits}"),
            (_, true) => writeln!(out, "current   {category}{commits}"),
            _ => writeln!(
                out,
                "kept      {category}{commits}, with this project's changes"
            ),
        };
    }
    for (to, contents) in &writes {
        let verb = if contents.is_some() {
            "updated "
        } else {
            "removed "
        };
        let _ = writeln!(out, "{verb}  {}", relative(to, root));
    }
    drop(packs);
    let _ = io::stdout().write_all(out.as_bytes());
    Ok(ExitCode::SUCCESS)
}

/// Returns the pack that `source` names: a local directory, relative to `cwd`, or a repository
/// fetched at `commit`, or at its ref or default branch without one.
///
/// # Errors
///
/// Returns a message for the terminal when Git cannot fetch the repository or the ref, or when the
/// pack has no configuration.
fn fetch(source: &str, cwd: &Path, commit: Option<&str>) -> Result<Pack, String> {
    let (dir, commit, temp) = if local(source, cwd) {
        (cwd.join(source), None, None)
    } else {
        let (dir, commit, temp) = clone(source, commit)?;
        (dir, Some(commit), Some(temp))
    };

    let (place, format) = PLACES
        .into_iter()
        .find(|(place, _)| dir.join(place).is_file())
        .ok_or_else(|| {
            format!("error: {source} has no uphill.yaml or uphill.toml, at its root or in .agents")
        })?;
    let text = std::fs::read_to_string(dir.join(place))
        .map_err(|error| format!("error: could not read {source}/{place}: {error}"))?;
    Ok(Pack {
        commit,
        dir,
        file: place.rsplit('/').next().unwrap_or(place),
        format,
        place: Path::new(place)
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default(),
        source: text,
        _temp: temp,
    })
}

/// Fetches the repository that `source` names into a temporary directory, at `commit` or at the
/// source's ref, and returns the pack's directory in it and the full commit hash.
fn clone(source: &str, commit: Option<&str>) -> Result<(PathBuf, String, TempDir), String> {
    // A ref follows the last `@`, unless that `@` belongs to a URL such as `git@github.com:o/r`.
    let (rest, reference) = match source.rsplit_once('@') {
        Some((rest, reference)) if !reference.is_empty() && !reference.contains(['/', ':']) => {
            (rest, Some(reference))
        }
        _ => (source, None),
    };
    let (repository, path) = match separator(rest) {
        Some(at) => (
            rest.get(..at).unwrap_or(rest).to_owned(),
            rest.get(at + 2..).unwrap_or_default(),
        ),
        None if rest.contains(':') || Path::new(rest).exists() => (rest.to_owned(), ""),
        None => github(rest),
    };
    let url = crate::dashboard::url(&repository)?;

    let temp = tempfile::tempdir()
        .map_err(|error| format!("error: could not create a temporary directory: {error}"))?;
    let git = |args: &[&str]| {
        worktree::git(temp.path(), args).map_err(|error| {
            let error = error.strip_prefix("error: ").unwrap_or(&error);
            format!("error: could not fetch {source}: {error}")
        })
    };
    git(&["init", "--quiet"])?;
    git(&[
        "fetch",
        "--quiet",
        "--depth",
        "1",
        "--no-tags",
        &url,
        commit.or(reference).unwrap_or("HEAD"),
    ])?;
    git(&["checkout", "--quiet", "FETCH_HEAD"])?;
    let hash = git(&["rev-parse", "FETCH_HEAD"])?;

    let dir = temp.path().join(path.trim_matches('/'));
    if !dir.is_dir() {
        return Err(format!("error: {repository} has no directory {path}"));
    }
    Ok((dir, hash, temp))
}

/// Returns whether `source` names a local directory. A source with `//` names a path in a
/// repository, even when the operating system reads it as a directory.
fn local(source: &str, cwd: &Path) -> bool {
    separator(source).is_none() && cwd.join(source).is_dir()
}

/// Returns the offset of the `//` that separates a repository from the path inside it, apart from
/// the one in `https://`.
fn separator(source: &str) -> Option<usize> {
    source.match_indices("//").map(|(at, _)| at).find(|at| {
        !source
            .get(..*at)
            .is_some_and(|before| before.ends_with(':'))
    })
}

/// Splits `owner/name/path` into the GitHub repository `owner/name` and the path inside it.
fn github(source: &str) -> (String, &str) {
    let mut parts = source.splitn(3, '/');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(owner), Some(name), path) => (format!("{owner}/{name}"), path.unwrap_or("")),
        _ => (source.to_owned(), ""),
    }
}

/// Lists each file under `from`, or `from` itself, by its place under `to`. Dependencies that a
/// package manager installs, such as `node_modules`, stay behind.
fn collect(from: &Path, to: &Path, files: &mut BTreeMap<PathBuf, PathBuf>) -> Result<(), String> {
    if !from.is_dir() {
        files.insert(to.to_path_buf(), from.to_path_buf());
        return Ok(());
    }
    let entries = std::fs::read_dir(from)
        .map_err(|error| format!("error: could not read {}: {error}", from.display()))?;
    for name in entries
        .filter_map(Result::ok)
        .map(|entry| entry.file_name())
    {
        if name != "node_modules" && name != ".git" {
            collect(&from.join(&name), &to.join(&name), files)?;
        }
    }
    Ok(())
}

/// Writes a file and the directories above it.
fn write(path: &Path, contents: &[u8], root: &Path) -> Result<(), String> {
    path.parent()
        .map(std::fs::create_dir_all)
        .transpose()
        .and_then(|_| std::fs::write(path, contents))
        .map_err(|error| format!("error: could not write {}: {error}", relative(path, root)))
}

/// Renders an error in a pack's configuration.
fn diagnostic(error: &engine::error::Error, source: &str, pack: &Pack) -> String {
    let place = pack.place.join(pack.file);
    crate::diagnostic(
        "error",
        error,
        &format!("{source}/{}", place.display()),
        &pack.source,
    )
}

/// Returns the first 7 characters of a commit hash.
fn short(commit: &str) -> String {
    commit.chars().take(7).collect()
}

/// Returns `path` relative to `root`, such as `.agents/metrics/lighthouse.ts`.
fn relative(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}

/// Returns the path from `root` to `path`, with `..` where it leaves `root`, such as
/// `../shared/pack`, or `path` itself when the two share only the filesystem root. Both paths are
/// resolved first, and a path that cannot be resolved is kept.
fn relative_to(path: &Path, root: &Path) -> String {
    let (Ok(path), Ok(root)) = (path.canonicalize(), root.canonicalize()) else {
        return path.to_string_lossy().into_owned();
    };
    let common = path
        .components()
        .zip(root.components())
        .take_while(|(a, b)| a == b)
        .count();
    // Paths that share only the filesystem root stay absolute.
    if common <= 1 {
        return path.to_string_lossy().into_owned();
    }
    let mut out = PathBuf::new();
    for _ in root.components().skip(common) {
        out.push("..");
    }
    for part in path.components().skip(common) {
        out.push(part);
    }
    if out.as_os_str().is_empty() {
        ".".to_owned()
    } else {
        out.to_string_lossy().into_owned()
    }
}
