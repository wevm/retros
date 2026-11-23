//! Checks out another revision for `retros --compare` and `retros blame`: a detached Git worktree
//! in a temporary directory, removed when the command is done with it, whether it succeeded or
//! failed.

use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;

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
pub(crate) fn add(root: &Path, reference: &str) -> Result<Worktree, String> {
    let root = root
        .canonicalize()
        .map_err(|error| format!("error: could not read {}: {error}", root.display()))?;
    let repository = PathBuf::from(repository(&root)?);
    let commit = git(
        &root,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{reference}^{{commit}}"),
        ],
    )
    .map_err(|_| format!("error: unknown revision '{reference}'"))?;
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
pub(crate) fn repository(root: &Path) -> Result<String, String> {
    git(root, &["rev-parse", "--show-toplevel"])
        .map_err(|_| format!("error: {} is not in a Git repository", root.display()))
}

/// Runs Git in `dir` and returns its standard output without the trailing newline.
///
/// # Errors
///
/// Returns a message for the terminal when Git cannot run or fails.
pub(crate) fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .map_err(|error| format!("error: could not run git: {error}"))?;
    if !output.status.success() {
        return Err(failed(args, &output.stderr));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .trim_end()
        .to_owned())
}

#[cold]
fn failed(args: &[&str], stderr: &[u8]) -> String {
    format!(
        "error: `git {}` failed: {}",
        args.join(" "),
        String::from_utf8_lossy(stderr).trim()
    )
}
