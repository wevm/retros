//! `uphill add <pack> [category...]`: copies a pack's categories into the project's `uphill.yaml`
//! and the scripts their `run` commands name into the project, which then owns them.
//!
//! A pack is any directory with an `uphill.yaml`, at its root or in `.agents/`, such as another
//! project or one of Uphill's examples. It is a local directory, `owner/name/path` on GitHub, or a
//! Git URL or repository path followed by `//path`, each with an optional `@ref`. Uphill fetches a
//! remote pack into a temporary directory with Git's own credentials and removes it afterwards.
//!
//! Scripts go beside the project's configuration, in `.agents/` for a project without one, under
//! the path the pack gives them. A script in a directory, such as `metrics/lighthouse.ts`, brings
//! the whole directory, so the files it imports come too. Uphill writes nothing when a copied file
//! would replace a different one.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::{self, Write as _};
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;

use engine::config::Format;
use engine::pack::{self, Options};
use tempfile::TempDir;

use crate::{project, worktree};

/// Where a pack keeps its configuration, relative to its directory.
const PLACES: [&str; 2] = ["uphill.yaml", ".agents/uphill.yaml"];

/// A pack's directory, and the temporary directory that holds it when Uphill fetched it.
struct Pack {
    dir: PathBuf,
    /// The pack as the added categories' comment names it, with the commit for a fetched pack.
    label: String,
    _temp: Option<TempDir>,
}

/// Adds the pack's categories, or the ones `categories` names, to the project in the current
/// directory, under `name` when it is set.
///
/// # Errors
///
/// Returns a message for the terminal when the pack cannot be fetched or has no `uphill.yaml`,
/// when either configuration is invalid or is TOML, when a category cannot be added, or when a
/// file cannot be read or written.
pub(crate) fn run(
    source: &str,
    categories: &[String],
    name: Option<&str>,
) -> Result<ExitCode, String> {
    let cwd = std::env::current_dir()
        .map_err(|error| format!("error: could not read the current directory: {error}"))?;
    let pack = fetch(source)?;

    let place = PLACES
        .iter()
        .find(|place| pack.dir.join(place).is_file())
        .ok_or_else(|| format!("error: {source} has no uphill.yaml, at its root or in .agents"))?;
    let pack_source = std::fs::read_to_string(pack.dir.join(place))
        .map_err(|error| format!("error: could not read {source}/{place}: {error}"))?;
    // Paths in a pack's `run` commands start at the directory above `.agents`.
    let pack_config_dir = PathBuf::from(place)
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default();

    let existing = project::exists(&cwd)
        .then(|| project::find(&cwd))
        .transpose()?;
    if existing
        .as_ref()
        .is_some_and(|project| project.format == Format::Toml)
    {
        return Err(
            "error: uphill add writes categories into uphill.yaml, and this project uses uphill.toml"
                .to_owned(),
        );
    }
    let (root, config_path) = match &existing {
        Some(project) => (project.root.clone(), project.path.clone()),
        None => (cwd.clone(), cwd.join(".agents/uphill.yaml")),
    };
    let config_dir = config_path.parent().unwrap_or(&root).to_path_buf();
    let config_name = relative(&config_path, &root);

    // Each script a `run` command names maps to its place in the project.
    let mut copies: BTreeMap<PathBuf, PathBuf> = BTreeMap::new();
    let mut rewrite = |run: &str| {
        run.split(' ')
            .map(|token| {
                let word = token.trim_matches(['"', '\'']);
                let path = Path::new(word);
                let Ok(inner) = path.strip_prefix(&pack_config_dir) else {
                    return token.to_owned();
                };
                let local = path.components().all(|c| matches!(c, Component::Normal(_)));
                if word.is_empty() || !local || !pack.dir.join(path).is_file() {
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
                copies.insert(
                    pack.dir.join(&pack_config_dir).join(&unit),
                    config_dir.join(&unit),
                );
                token.replace(word, &relative(&config_dir.join(inner), &root))
            })
            .collect::<Vec<_>>()
            .join(" ")
    };

    let comment = format!("Added by `uphill add {}`.", pack.label);
    let added = pack::add(&mut Options {
        categories,
        comment: &comment,
        name,
        pack: &pack_source,
        project: existing.as_ref().map(|project| project.source.as_str()),
        run: &mut rewrite,
    })
    .map_err(|error| {
        crate::diagnostic("error", &error, &format!("{source}/{place}"), &pack_source)
    })?;

    let mut files = Vec::new();
    for (from, to) in &copies {
        collect(from, to, &mut files)?;
    }
    for (from, to) in &files {
        if std::fs::read(to).is_ok_and(|bytes| std::fs::read(from).ok() != Some(bytes)) {
            return Err(format!(
                "error: {} exists and differs from the pack's copy. Move it, then run `uphill add` again",
                relative(to, &root)
            ));
        }
    }

    let write = |path: &Path, contents: &[u8]| {
        path.parent()
            .map(std::fs::create_dir_all)
            .transpose()
            .and_then(|_| std::fs::write(path, contents))
            .map_err(|error| format!("error: could not write {}: {error}", relative(path, &root)))
    };
    for (from, to) in &files {
        let bytes = std::fs::read(from)
            .map_err(|error| format!("error: could not read {}: {error}", from.display()))?;
        write(to, &bytes)?;
    }
    write(&config_path, added.config.as_bytes())?;

    let mut out = format!("Added {} to {config_name}\n", added.categories.join(", "));
    for to in copies.values() {
        let shown = relative(to, &root);
        let _ = if to.is_dir() {
            writeln!(out, "Added {shown}/")
        } else {
            writeln!(out, "Added {shown}")
        };
    }
    if let Some(setup) = &added.setup {
        let _ = writeln!(
            out,
            "\nThe pack prepares each checkout with `setup: {setup}`. Add it to {config_name} if the scripts need it"
        );
    }
    if let Ok(readme) = std::fs::read_to_string(pack.dir.join(&pack_config_dir).join("README.md")) {
        let _ = write!(out, "\n{}\n", readme.trim_end());
    }
    out.push_str("\nFit the scripts and targets to this project, then run `uphill check`\n");
    let _ = io::stdout().write_all(out.as_bytes());
    Ok(ExitCode::SUCCESS)
}

/// Returns the pack's directory: a local directory as it is, or a repository fetched at its ref.
///
/// # Errors
///
/// Returns a message for the terminal when Git cannot fetch the repository or the ref.
fn fetch(source: &str) -> Result<Pack, String> {
    if Path::new(source).is_dir() {
        return Ok(Pack {
            dir: PathBuf::from(source),
            label: source.to_owned(),
            _temp: None,
        });
    }

    // A ref follows the last `@`, unless that `@` belongs to a URL such as `git@github.com:o/r`.
    let (rest, reference) = match source.rsplit_once('@') {
        Some((rest, reference)) if !reference.is_empty() && !reference.contains(['/', ':']) => {
            (rest, Some(reference))
        }
        _ => (source, None),
    };
    // `//` separates a repository from the path inside it, apart from the one in `https://`.
    let split = rest
        .match_indices("//")
        .map(|(at, _)| at)
        .find(|at| !rest.get(..*at).is_some_and(|before| before.ends_with(':')));
    let (repository, path) = match split {
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
        reference.unwrap_or("HEAD"),
    ])?;
    git(&["checkout", "--quiet", "FETCH_HEAD"])?;
    let short = git(&["rev-parse", "--short", "FETCH_HEAD"])?;

    let dir = temp.path().join(path.trim_matches('/'));
    if !dir.is_dir() {
        return Err(format!("error: {repository} has no directory {path}"));
    }
    let named = if path.is_empty() {
        repository.clone()
    } else if repository.contains(':') || Path::new(&repository).exists() {
        format!("{repository}//{path}")
    } else {
        format!("{repository}/{path}")
    };
    Ok(Pack {
        dir,
        label: format!("{named}@{short}"),
        _temp: Some(temp),
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

/// Lists each file under `from`, or `from` itself, with its place under `to`. Dependencies that a
/// package manager installs, such as `node_modules`, stay behind.
fn collect(from: &Path, to: &Path, files: &mut Vec<(PathBuf, PathBuf)>) -> Result<(), String> {
    if !from.is_dir() {
        files.push((from.to_path_buf(), to.to_path_buf()));
        return Ok(());
    }
    let mut entries: Vec<_> = std::fs::read_dir(from)
        .map_err(|error| format!("error: could not read {}: {error}", from.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.file_name())
        .filter(|name| name != "node_modules" && name != ".git")
        .collect();
    entries.sort();
    for name in entries {
        collect(&from.join(&name), &to.join(&name), files)?;
    }
    Ok(())
}

/// Returns `path` relative to `root`, such as `.agents/metrics/lighthouse.ts`.
fn relative(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}
