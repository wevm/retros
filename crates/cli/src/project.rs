//! Finds and reads the project's `retros.yaml`, at the project root or in its `.agents` directory.

use std::io;
use std::path::{Path, PathBuf};

use engine::config::{self, Config};

/// Where a project keeps `retros.yaml`, relative to its root.
const PLACES: [&str; 2] = ["retros.yaml", ".agents/retros.yaml"];

/// A project: its parsed configuration, its root, and the directory that holds `retros.yaml`.
pub(crate) struct Project {
    /// The parsed `retros.yaml`.
    pub(crate) config: Config,
    /// The directory that holds `retros.yaml` and `retros.lock`: the root or its `.agents`
    /// directory.
    pub(crate) dir: PathBuf,
    /// The path of `retros.yaml` relative to the current directory, such as
    /// `../.agents/retros.yaml`.
    pub(crate) name: String,
    /// The path of the root relative to the current directory, such as `../`, or empty.
    pub(crate) prefix: String,
    /// The directory that holds `retros.yaml` or `.agents/retros.yaml`. Scripts run from here,
    /// and reports go to its `.retros` directory.
    pub(crate) root: PathBuf,
    /// The text of `retros.yaml`, for errors that point into it.
    pub(crate) source: String,
}

/// Finds `retros.yaml` or `.agents/retros.yaml` in `cwd` or the nearest parent directory that
/// has one, and parses it.
///
/// # Errors
///
/// Returns a message for the terminal when no directory has either file, when one directory has
/// both, when the file cannot be read, or when it is invalid. A configuration error names the
/// file relative to `cwd`, such as `../.agents/retros.yaml`, with its line and column.
pub(crate) fn find(cwd: &Path) -> Result<Project, String> {
    for (depth, root) in cwd.ancestors().enumerate() {
        let prefix = "../".repeat(depth);
        let mut found: Option<(&str, String)> = None;

        for place in PLACES {
            // `.agents/retros.yaml` belongs to the directory above `.agents`, so scripts run
            // from the project root even when Retros starts inside `.agents`.
            if place == "retros.yaml" && root.file_name().is_some_and(|name| name == ".agents") {
                continue;
            }

            let source = match std::fs::read_to_string(root.join(place)) {
                Ok(source) => source,
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => {
                    return Err(format!("error: could not read {prefix}{place}: {error}"));
                }
            };
            if let Some((other, _)) = found {
                return Err(format!(
                    "error: {prefix}{other} and {prefix}{place} both exist. Keep one"
                ));
            }
            found = Some((place, source));
        }

        let Some((place, source)) = found else {
            continue;
        };
        let name = format!("{prefix}{place}");
        let dir = root.join(place.trim_end_matches("retros.yaml"));

        return match config::parse(&source) {
            Ok(config) => Ok(Project {
                config,
                dir,
                name,
                prefix,
                root: root.to_owned(),
                source,
            }),
            Err(error) => Err(crate::diagnostic("error", &error, &name, &source)),
        };
    }

    Err(
        "error: no retros.yaml or .agents/retros.yaml in this directory or any parent directory"
            .to_owned(),
    )
}
