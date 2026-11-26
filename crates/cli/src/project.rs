//! Finds and reads the project's `uphill.yaml` or `uphill.toml`, at the project root or in its
//! `.agents` directory, and the baseline file beside it, `uphill.baseline.yaml` or
//! `uphill.baseline.toml`. Finds each pack that its `packs` key lists: a remote pack at its pinned
//! commit in the cache of fetched packs (see [`crate::fetch`]), or a local directory.

use std::io;
use std::path::{Component, Path, PathBuf};

use engine::baseline;
use engine::config::{self, Config, Format};
use engine::pack::{self, Entry, Pack, Source};

/// Where a project keeps its configuration, relative to its root, and the format of each file.
const PLACES: [(&str, Format); 4] = [
    ("uphill.yaml", Format::Yaml),
    ("uphill.toml", Format::Toml),
    (".agents/uphill.yaml", Format::Yaml),
    (".agents/uphill.toml", Format::Toml),
];

/// A project: its parsed configuration, its root, and the directory that holds the configuration.
pub(crate) struct Project {
    /// The path of the baseline file, where `uphill --update` writes baselines.
    pub(crate) baselines: PathBuf,
    /// The path of the baseline file relative to the current directory, such as
    /// `../.agents/uphill.baseline.yaml`.
    pub(crate) baselines_name: String,
    /// The text of the baseline file, or `None` when it does not exist.
    pub(crate) baselines_source: Option<String>,
    /// The parsed configuration, with the baselines of the baseline file.
    pub(crate) config: Config,
    /// The directory of each pack whose dependencies the project approves installing: relative
    /// to the root for a local pack, and absolute for a remote pack in the cache.
    pub(crate) installs: Vec<PathBuf>,
    /// The format of the configuration.
    pub(crate) format: Format,
    /// The path of the configuration relative to the current directory, such as
    /// `../.agents/uphill.yaml`.
    pub(crate) name: String,
    /// The packs that the configuration lists, in its order.
    pub(crate) packs: Vec<Pack>,
    /// The path of the configuration.
    pub(crate) path: PathBuf,
    /// The path of the root relative to the current directory, such as `../`, or empty.
    pub(crate) prefix: String,
    /// The directory that holds the configuration or the `.agents` directory with it. Scripts
    /// run from here, and reports go to its `.uphill` directory.
    pub(crate) root: PathBuf,
    /// The text of the configuration, for errors that point into it.
    pub(crate) source: String,
}

/// Finds `uphill.yaml` or `uphill.toml`, at the root or in `.agents`, in `cwd` or the nearest
/// parent directory that has one, and parses it with the baseline file beside it.
///
/// # Errors
///
/// Returns a message for the terminal when no directory has a configuration, when one directory
/// has more than one, when a file cannot be read, or when it is invalid. An error names the file
/// relative to `cwd`, such as `../.agents/uphill.yaml`, with its line and column.
pub(crate) fn find(cwd: &Path) -> Result<Project, String> {
    let Configuration {
        format,
        name,
        place,
        prefix,
        root,
        source,
    } = configuration(cwd)?;
    let root = root.as_path();

    let (packs, installs) = packs(&[root], &source, format, &name)?;
    let config = config::compose(&source, format, &packs)
        .map_err(|error| crate::diagnostic("error", &error, &name, &source))?;

    let sidecar = baselines(place);
    let baselines_name = format!("{prefix}{sidecar}");
    let baselines_source = match std::fs::read_to_string(root.join(&sidecar)) {
        Ok(source) => Some(source),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => {
            return Err(format!("error: could not read {baselines_name}: {error}"));
        }
    };
    let config = match &baselines_source {
        Some(text) => baseline::read(config, text, format)
            .map_err(|error| crate::diagnostic("error", &error, &baselines_name, text))?,
        None => config,
    };
    config::validate(&config)
        .map_err(|error| crate::diagnostic("error", &error, &name, &source))?;

    Ok(Project {
        baselines: root.join(&sidecar),
        baselines_name,
        baselines_source,
        config,
        format,
        installs,
        name,
        packs,
        path: root.join(place),
        prefix,
        root: root.to_owned(),
        source,
    })
}

/// A project's configuration file, before its packs are read.
pub(crate) struct Configuration {
    /// The format of the configuration.
    pub(crate) format: Format,
    /// The path of the configuration relative to the current directory.
    pub(crate) name: String,
    /// The path of the configuration relative to the root, such as `.agents/uphill.yaml`.
    pub(crate) place: &'static str,
    /// The path of the root relative to the current directory, such as `../`, or empty.
    pub(crate) prefix: String,
    /// The project root.
    pub(crate) root: PathBuf,
    /// The text of the configuration.
    pub(crate) source: String,
}

/// Finds `uphill.yaml` or `uphill.toml`, at the root or in `.agents`, in `cwd` or the nearest
/// parent directory that has one, and reads it without parsing it.
///
/// # Errors
///
/// Returns a message for the terminal when no directory has a configuration, when one directory
/// has more than one, or when a file cannot be read.
pub(crate) fn configuration(cwd: &Path) -> Result<Configuration, String> {
    for (depth, root) in cwd.ancestors().enumerate() {
        let prefix = "../".repeat(depth);
        let mut found: Option<(&str, Format, String)> = None;

        for (place, format) in PLACES {
            // `.agents/uphill.yaml` belongs to the directory above `.agents`, so scripts run
            // from the project root even when Uphill starts inside `.agents`.
            if !place.contains('/') && root.file_name().is_some_and(|name| name == ".agents") {
                continue;
            }

            let source = match std::fs::read_to_string(root.join(place)) {
                Ok(source) => source,
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => {
                    return Err(format!("error: could not read {prefix}{place}: {error}"));
                }
            };
            if let Some((other, _, _)) = found {
                return Err(format!(
                    "error: {prefix}{other} and {prefix}{place} both exist. Keep one"
                ));
            }
            found = Some((place, format, source));
        }

        let Some((place, format, source)) = found else {
            continue;
        };
        return Ok(Configuration {
            format,
            name: format!("{prefix}{place}"),
            place,
            prefix,
            root: root.to_owned(),
            source,
        });
    }

    Err(
        "error: no uphill.yaml or uphill.toml, at the root or in .agents, in this directory or any parent directory"
            .to_owned(),
    )
}

/// Returns the path of the baseline file beside the configuration at `place`, such as
/// `.agents/uphill.baseline.yaml` for `.agents/uphill.yaml`.
pub(crate) fn baselines(place: &str) -> String {
    place
        .replace("uphill.yaml", "uphill.baseline.yaml")
        .replace("uphill.toml", "uphill.baseline.toml")
}

/// Parses a configuration and its baseline file, as another revision holds them, or returns
/// `None` when either does not parse. Its local packs come from the first of `roots` that has each
/// one, such as the revision's checkout and then the current project, and its remote packs from
/// the cache.
pub(crate) fn load(
    roots: &[&Path],
    source: &str,
    baselines: Option<&str>,
    format: Format,
) -> Option<Config> {
    let (packs, _) = packs(roots, source, format, "").ok()?;
    let config = config::compose(source, format, &packs).ok()?;
    let config = match baselines {
        Some(baselines) => baseline::read(config, baselines, format).ok()?,
        None => config,
    };
    config::validate(&config).ok().map(|()| config)
}

/// Finds and parses each pack that the configuration `source` lists: a local pack from the first
/// of `roots` that has it, and a remote pack from the cache, which fetches it when it is missing.
/// Each pack's `run` commands are rewritten to start at the project root, and the default targets
/// of its baseline file come with it. Also returns the directory of each pack whose install the
/// project approves, as [`Project::installs`] holds it.
///
/// # Errors
///
/// Returns a message for the terminal when the `packs` key is invalid, when a local pack is not a
/// directory, when a remote pack cannot be fetched, when a pack has no configuration, when its
/// configuration is invalid, or when it lists packs.
fn packs(
    roots: &[&Path],
    source: &str,
    format: Format,
    name: &str,
) -> Result<(Vec<Pack>, Vec<PathBuf>), String> {
    let entries = pack::entries(source, format)
        .map_err(|error| crate::diagnostic("error", &error, name, source))?;

    let mut packs = Vec::new();
    let mut installs = Vec::new();
    for entry in entries {
        let (shown, dir, install) = match &entry.source {
            Source::Local(path) => {
                let Some(root) = roots.iter().find(|root| root.join(path).is_dir()) else {
                    return Err(missing(&entry, name));
                };
                let dir = root.join(path);
                (relative(&dir, root), dir, PathBuf::from(path))
            }
            Source::Remote(remote) => {
                let dir = crate::fetch::pack(remote)?;
                (dir.display().to_string(), dir.clone(), dir)
            }
        };
        if entry.install {
            installs.push(install);
        }
        packs.push(read(&dir, &shown, entry)?);
    }
    Ok((packs, installs))
}

/// Reads and parses the pack in `dir`, which `entry` names. `shown` is the directory as `run`
/// commands and messages name it: relative to the project root for a local pack, or absolute.
fn read(dir: &Path, shown: &str, entry: Entry) -> Result<Pack, String> {
    let Some((place, format, source)) = PLACES.iter().find_map(|(place, format)| {
        std::fs::read_to_string(dir.join(place))
            .ok()
            .map(|source| (*place, *format, source))
    }) else {
        return Err(format!(
            "error: pack `{}` at {shown} has no uphill.yaml or uphill.toml, at its root or in .agents",
            entry.name
        ));
    };
    let file = format!("{shown}/{place}");

    let parse = |source: &str| {
        let nested = pack::entries(source, format)
            .map_err(|error| crate::diagnostic("error", &error, &file, source))?;
        if !nested.is_empty() {
            return Err(format!(
                "error: {file} lists packs. A pack's own `packs` are not read, so list them in the project"
            ));
        }
        config::parse(source, format)
            .map_err(|error| crate::diagnostic("error", &error, &file, source))
    };
    let mut config = parse(&source)?;

    let sidecar = baselines(place);
    if let Ok(text) = std::fs::read_to_string(dir.join(&sidecar)) {
        config = baseline::read(config, &text, format).map_err(|error| {
            crate::diagnostic("error", &error, &format!("{shown}/{sidecar}"), &text)
        })?;
    }
    config::validate(&config)
        .map_err(|error| crate::diagnostic("error", &error, &file, &source))?;

    for script in config
        .categories
        .iter_mut()
        .flat_map(|category| &mut category.scripts)
    {
        script.run = rewrite(&script.run, dir, shown);
    }
    Ok(Pack { config, entry })
}

/// Returns `run` with each path that names a file in the pack's directory `dir` replaced by its
/// path through the pack's place `shown`, such as `packs/rust/metrics/rust.ts` for a local pack.
/// An unquoted path with characters that the shell reads, such as a space in the cache's path,
/// is quoted.
fn rewrite(run: &str, dir: &Path, shown: &str) -> String {
    run.split(' ')
        .map(|token| {
            let word = token.trim_matches(['"', '\'']);
            let path = Path::new(word);
            let local = path.components().all(|c| matches!(c, Component::Normal(_)));
            if word.is_empty() || !local || !dir.join(path).is_file() {
                return token.to_owned();
            }
            let full = format!("{shown}/{word}");
            let plain = full
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"/._@+-".contains(&byte));
            if token == word && !plain {
                return format!("'{}'", full.replace('\'', r"'\''"));
            }
            token.replace(word, &full)
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Returns `path` relative to `root`, with `../` for each level above it, or the path itself
/// when they share no parent.
fn relative(path: &Path, root: &Path) -> String {
    if let Ok(inner) = path.strip_prefix(root) {
        return inner.display().to_string();
    }
    root.ancestors()
        .enumerate()
        .find_map(|(depth, parent)| {
            path.strip_prefix(parent)
                .ok()
                .map(|inner| format!("{}{}", "../".repeat(depth), inner.display()))
        })
        .unwrap_or_else(|| path.display().to_string())
}

#[cold]
fn missing(entry: &Entry, name: &str) -> String {
    let at = format!("{name}:{}:{}", entry.location.line, entry.location.column);
    format!("error: {at}: pack `{}` is not a directory", entry.name)
}

/// Installs the dependencies of each pack in `installs`, relative to `root` or absolute, that has no
/// `node_modules` yet. The install follows the pack's lockfile exactly and runs no scripts.
///
/// # Errors
///
/// Returns a message for the terminal when a pack has no lockfile, or its install fails.
pub(crate) fn install(root: &Path, installs: &[PathBuf]) -> Result<(), String> {
    for relative in installs {
        let dir = root.join(relative);
        if !dir.join("package.json").is_file() || dir.join("node_modules").is_dir() {
            continue;
        }
        let shown = relative.display();
        let command: &[&str] = if dir.join("bun.lock").is_file() || dir.join("bun.lockb").is_file()
        {
            &["bun", "install", "--frozen-lockfile", "--ignore-scripts"]
        } else if dir.join("pnpm-lock.yaml").is_file() {
            &[
                "pnpm",
                "install",
                "--frozen-lockfile",
                "--ignore-scripts",
                "--ignore-workspace",
            ]
        } else if dir.join("package-lock.json").is_file() {
            &["npm", "ci", "--ignore-scripts"]
        } else {
            return Err(format!(
                "error: pack {shown} has a package.json without a lockfile, so Uphill does not install it"
            ));
        };

        let output = std::process::Command::new(command.first().unwrap_or(&"bun"))
            .args(command.get(1..).unwrap_or_default())
            .current_dir(&dir)
            .stdin(std::process::Stdio::null())
            .output()
            .map_err(|error| {
                format!(
                    "error: could not run `{}` in {shown}: {error}",
                    command.join(" ")
                )
            })?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!(
                "error: `{}` failed in {shown}\n{}",
                command.join(" "),
                stderr.trim_end()
            ));
        }
    }
    Ok(())
}
