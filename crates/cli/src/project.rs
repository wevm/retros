//! Finds and reads the project's `uphill.yaml` or `uphill.toml`, at the project root or in its
//! `.agents` directory, and the baseline file beside it, `uphill.baseline.yaml` or
//! `uphill.baseline.toml`. Finds each pack that its `packs` key lists: a remote pack vendored in
//! `.agents/uphill/packs/<name>/`, or a local directory. Runs read only these committed files.

use std::io;
use std::path::{Component, Path, PathBuf};

use engine::baseline;
use engine::config::{self, Config, Format};
use engine::pack::{self, Entry, Pack, Source};

/// The directory, relative to the project root, that holds each vendored remote pack.
pub(crate) const PACKS: &str = ".agents/uphill/packs";

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
    /// The directory of each pack whose dependencies the project approves installing, relative
    /// to the root.
    pub(crate) installs: Vec<PathBuf>,
    /// The format of the configuration.
    pub(crate) format: Format,
    /// The path of the configuration relative to the current directory, such as
    /// `../.agents/uphill.yaml`.
    pub(crate) name: String,
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

    let packs = packs(&[root], &source, format, &name)?;
    let installs = packs
        .iter()
        .filter(|pack| pack.entry.install)
        .map(|pack| directory(&pack.entry))
        .collect();
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
/// `None` when either does not parse. Its packs come from the first of `roots` that has each one,
/// such as the revision's checkout and then the current project.
pub(crate) fn load(
    roots: &[&Path],
    source: &str,
    baselines: Option<&str>,
    format: Format,
) -> Option<Config> {
    let packs = packs(roots, source, format, "").ok()?;
    let config = config::compose(source, format, &packs).ok()?;
    let config = match baselines {
        Some(baselines) => baseline::read(config, baselines, format).ok()?,
        None => config,
    };
    config::validate(&config).ok().map(|()| config)
}

/// Finds and parses each pack that the configuration `source` lists, from the first of `roots`
/// that has it. Each pack's `run` commands are rewritten to start at the project root, and the
/// default targets of its baseline file come with it.
///
/// # Errors
///
/// Returns a message for the terminal when the `packs` key is invalid, when a pack is not
/// installed or has no configuration, when its configuration is invalid, or when it lists packs.
fn packs(roots: &[&Path], source: &str, format: Format, name: &str) -> Result<Vec<Pack>, String> {
    let entries = pack::entries(source, format)
        .map_err(|error| crate::diagnostic("error", &error, name, source))?;

    let mut packs = Vec::new();
    for entry in entries {
        let Some((root, dir)) = roots
            .iter()
            .find_map(|root| locate(root, &entry).map(|dir| (*root, dir)))
        else {
            return Err(missing(&entry, name));
        };
        packs.push(read(root, &dir, entry)?);
    }
    Ok(packs)
}

/// Returns the directory of a pack relative to the project root: `.agents/uphill/packs/<name>` for a
/// remote pack, or the local directory.
pub(crate) fn directory(entry: &Entry) -> PathBuf {
    match &entry.source {
        Source::Local(path) => PathBuf::from(path),
        Source::Remote(_) => Path::new(PACKS).join(entry.directory()),
    }
}

/// Returns the directory of a pack under `root`, when it exists.
fn locate(root: &Path, entry: &Entry) -> Option<PathBuf> {
    let dir = root.join(directory(entry));
    dir.is_dir().then_some(dir)
}

/// Reads and parses the pack in `dir`, which `entry` names, for a project at `root`.
fn read(root: &Path, dir: &Path, entry: Entry) -> Result<Pack, String> {
    let shown = relative(dir, root);
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
        script.run = rewrite(&script.run, dir, &shown);
    }
    Ok(Pack { config, entry })
}

/// Returns `run` with each path that names a file in the pack's directory `dir` replaced by its
/// path from the project root, through the pack's place `shown`, such as
/// `.agents/uphill/packs/rust/metrics/rust.ts`.
fn rewrite(run: &str, dir: &Path, shown: &str) -> String {
    run.split(' ')
        .map(|token| {
            let word = token.trim_matches(['"', '\'']);
            let path = Path::new(word);
            let local = path.components().all(|c| matches!(c, Component::Normal(_)));
            if word.is_empty() || !local || !dir.join(path).is_file() {
                return token.to_owned();
            }
            token.replace(word, &format!("{shown}/{word}"))
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
    if matches!(entry.source, Source::Local(_)) {
        return format!("error: {at}: pack `{}` is not a directory", entry.name);
    }
    format!(
        "error: {at}: pack `{}` is not vendored in {}. Run `uphill update {}` to fetch it",
        entry.name,
        directory(entry).display(),
        entry.directory()
    )
}

/// Installs the dependencies of each pack in `installs`, relative to `root`, that has no
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
