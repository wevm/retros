//! Finds and reads the project's `retros.yaml` or `retros.toml`, at the project root or in its
//! `.agents` directory.

use std::io;
use std::path::{Path, PathBuf};

use engine::config::{self, Config, Format};

/// Where a project keeps its configuration, relative to its root, and the format of each file.
const PLACES: [(&str, Format); 4] = [
    ("retros.yaml", Format::Yaml),
    ("retros.toml", Format::Toml),
    (".agents/retros.yaml", Format::Yaml),
    (".agents/retros.toml", Format::Toml),
];

/// A project: its parsed configuration, its root, and the directory that holds the configuration.
pub(crate) struct Project {
    /// The parsed configuration.
    pub(crate) config: Config,
    /// The directory that holds the configuration and `retros.lock`: the root or its `.agents`
    /// directory.
    pub(crate) dir: PathBuf,
    /// The path of `retros.lock` relative to the current directory, such as
    /// `../.agents/retros.lock`.
    pub(crate) lock_name: String,
    /// The path of the configuration relative to the current directory, such as
    /// `../.agents/retros.yaml`.
    pub(crate) name: String,
    /// The path of the root relative to the current directory, such as `../`, or empty.
    pub(crate) prefix: String,
    /// The directory that holds the configuration or the `.agents` directory with it. Scripts
    /// run from here, and reports go to its `.retros` directory.
    pub(crate) root: PathBuf,
    /// The text of the configuration, for errors that point into it.
    pub(crate) source: String,
}

/// Finds `retros.yaml` or `retros.toml`, at the root or in `.agents`, in `cwd` or the nearest
/// parent directory that has one, and parses it.
///
/// # Errors
///
/// Returns a message for the terminal when no directory has a configuration, when one directory
/// has more than one, when the file cannot be read, or when it is invalid. A configuration error
/// names the file relative to `cwd`, such as `../.agents/retros.yaml`, with its line and column.
pub(crate) fn find(cwd: &Path) -> Result<Project, String> {
    for (depth, root) in cwd.ancestors().enumerate() {
        let prefix = "../".repeat(depth);
        let mut found: Option<(&str, Format, String)> = None;

        for (place, format) in PLACES {
            // `.agents/retros.yaml` belongs to the directory above `.agents`, so scripts run
            // from the project root even when Retros starts inside `.agents`.
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
        let folder = place.rsplit_once('/').map_or("", |(folder, _)| folder);
        let dir = root.join(folder);
        let lock_name = Path::new(&prefix)
            .join(folder)
            .join("retros.lock")
            .to_string_lossy()
            .into_owned();

        return match config::parse(&source, format) {
            Ok(config) => Ok(Project {
                config,
                dir,
                lock_name,
                name,
                prefix,
                root: root.to_owned(),
                source,
            }),
            Err(error) => Err(crate::diagnostic("error", &error, &name, &source)),
        };
    }

    Err(
        "error: no retros.yaml or retros.toml, at the root or in .agents, in this directory or any parent directory"
            .to_owned(),
    )
}
