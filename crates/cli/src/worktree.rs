//! Checks out another revision for `uphill --compare` and `uphill blame`: a detached Git worktree
//! in a temporary directory, removed when the command is done with it, whether it succeeded or
//! failed.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

use crate::error::Error;

/// A checkout of another revision of the project's repository.
pub(crate) struct Worktree {
    /// The worktree's directory, inside `dir`.
    checkout: PathBuf,
    /// The temporary directory that holds the worktree. Dropped after the worktree is removed.
    _dir: TempDir,
    /// The revision as reports name it, such as `main (3e0053b)`.
    pub(crate) label: String,
    /// The repository's top level, where Git removes the worktree.
    repository: PathBuf,
    /// The project root inside the worktree.
    pub(crate) root: PathBuf,
}

/// Checks out `reference` in a new worktree of the repository that holds `root`.
///
/// # Errors
///
/// Returns a message for the terminal when `root` is not in a Git repository, when `reference`
/// names no commit, or when Git cannot create the worktree.
pub(crate) fn add(root: &Path, reference: &str) -> Result<Worktree, Error> {
    let root = root
        .canonicalize()
        .map_err(|error| format!("error: could not read {}: {error}", root.display()))?;
    let repository = PathBuf::from(repository(&root)?);
    let commit = query(
        &root,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{reference}^{{commit}}"),
        ],
    )?
    .ok_or_else(|| format!("error: unknown revision '{reference}'"))?;
    let short = git(&root, &["rev-parse", "--short", &commit])?;

    let dir = tempfile::tempdir()
        .map_err(|error| format!("error: could not create a temporary directory: {error}"))?;
    let checkout = dir.path().join("checkout");
    let path = checkout.to_string_lossy().into_owned();
    git(
        &repository,
        &["worktree", "add", "--detach", "--quiet", &path, &commit],
    )?;

    // The project may sit in a subdirectory of its repository.
    let project = checkout.join(root.strip_prefix(&repository).unwrap_or(Path::new("")));
    Ok(Worktree {
        checkout,
        _dir: dir,
        label: if commit.starts_with(reference) {
            short
        } else {
            format!("{reference} ({short})")
        },
        repository,
        root: project,
    })
}

impl Drop for Worktree {
    fn drop(&mut self) {
        let path = self.checkout.to_string_lossy().into_owned();
        if git(&self.repository, &["worktree", "remove", "--force", &path]).is_err() {
            let _ = std::fs::remove_dir_all(&self.checkout);
            let _ = git(&self.repository, &["worktree", "prune"]);
        }
    }
}

/// Returns the top level of the Git repository that holds `root`.
///
/// # Errors
///
/// Returns a message for the terminal when `root` is not in a Git repository.
pub(crate) fn repository(root: &Path) -> Result<String, Error> {
    query(root, &["rev-parse", "--show-toplevel"])?
        .ok_or_else(|| format!("error: {} is not in a Git repository", root.display()).into())
}

/// Runs Git in `dir` and returns its standard output without the trailing newline.
///
/// # Errors
///
/// Returns a message for the terminal when Git cannot run or fails.
pub(crate) fn git(dir: &Path, args: &[&str]) -> Result<String, Error> {
    let output = run(dir, args)?;
    if !output.status.success() {
        return Err(failed(args, &output.stderr));
    }
    Ok(stdout(&output))
}

/// Runs Git in `dir` and returns its standard output without the trailing newline, or `None` when
/// Git exits with a failure status, such as for a revision that names no commit.
///
/// # Errors
///
/// Returns a message for the terminal when Git cannot run.
pub(crate) fn query(dir: &Path, args: &[&str]) -> Result<Option<String>, Error> {
    let output = run(dir, args)?;
    Ok(output.status.success().then(|| stdout(&output)))
}

/// Runs Git in `dir` and returns its output and status.
fn run(dir: &Path, args: &[&str]) -> Result<Output, Error> {
    Ok(Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .map_err(|error| format!("error: could not run git: {error}"))?)
}

/// Returns Git's standard output without the trailing newline.
fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout)
        .trim_end()
        .to_owned()
}

#[cold]
fn failed(args: &[&str], stderr: &[u8]) -> Error {
    format!(
        "error: `git {}` failed: {}",
        args.join(" "),
        String::from_utf8_lossy(stderr).trim()
    )
    .into()
}
