//! Finds and reads the project's `retros.toml`.

use std::io;
use std::path::{Path, PathBuf};

use engine::config::{self, Config};

/// A project: its parsed configuration and the directory that holds `retros.toml`.
pub(crate) struct Project {
    /// The parsed `retros.toml`.
    pub(crate) config: Config,
    /// The path of `retros.toml` relative to the current directory, such as `../retros.toml`.
    pub(crate) name: String,
    /// The directory that holds `retros.toml`. Scripts run from here.
    pub(crate) root: PathBuf,
    /// The text of `retros.toml`, for errors that point into it.
    pub(crate) source: String,
}

/// Finds `retros.toml` in `cwd` or the nearest parent directory that has one, and parses it.
///
/// # Errors
///
/// Returns a message for the terminal when no directory has `retros.toml`, when it cannot be
/// read, or when it is invalid. A configuration error names the file relative to `cwd`, such as
/// `../retros.toml`, with its line and column.
pub(crate) fn find(cwd: &Path) -> Result<Project, String> {
    for (depth, dir) in cwd.ancestors().enumerate() {
        let name = format!("{}retros.toml", "../".repeat(depth));
        let source = match std::fs::read_to_string(dir.join("retros.toml")) {
            Ok(source) => source,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(format!("error: could not read {name}: {error}")),
        };

        return match config::parse(&source) {
            Ok(config) => Ok(Project {
                config,
                name,
                root: dir.to_owned(),
                source,
            }),
            Err(error) => Err(crate::diagnostic("error", &error, &name, &source)),
        };
    }

    Err("error: no retros.toml in this directory or any parent directory".to_owned())
}
