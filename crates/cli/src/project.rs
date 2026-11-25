//! Finds and reads the project's `uphill.yaml` or `uphill.toml`, at the project root or in its
//! `.agents` directory, and the baseline file beside it, `uphill.baseline.yaml` or
//! `uphill.baseline.toml`.

use std::io;
use std::path::{Path, PathBuf};

use engine::baseline;
use engine::config::{self, Config, Format};

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
        let name = format!("{prefix}{place}");
        let config = config::parse(&source, format)
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

        return Ok(Project {
            baselines: root.join(&sidecar),
            baselines_name,
            baselines_source,
            config,
            format,
            name,
            path: root.join(place),
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

/// Returns whether `cwd` or a parent directory has a configuration, valid or not.
pub(crate) fn exists(cwd: &Path) -> bool {
    cwd.ancestors()
        .any(|root| PLACES.iter().any(|(place, _)| root.join(place).is_file()))
}

/// Returns the path of the baseline file beside the configuration at `place`, such as
/// `.agents/uphill.baseline.yaml` for `.agents/uphill.yaml`.
pub(crate) fn baselines(place: &str) -> String {
    place
        .replace("uphill.yaml", "uphill.baseline.yaml")
        .replace("uphill.toml", "uphill.baseline.toml")
}

/// Parses a configuration and its baseline file, as another revision holds them, or returns
/// `None` when either does not parse.
pub(crate) fn load(source: &str, baselines: Option<&str>, format: Format) -> Option<Config> {
    let config = config::parse(source, format).ok()?;
    let config = match baselines {
        Some(baselines) => baseline::read(config, baselines, format).ok()?,
        None => config,
    };
    config::validate(&config).ok().map(|()| config)
}
