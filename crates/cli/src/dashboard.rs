//! `uphill dashboard`: the latest noted runs of several repositories in one page. For each
//! repository, Uphill fetches the default branch as commits only, and the notes under
//! `refs/notes/uphill` and `refs/notes/uphill-report`, into a temporary bare repository with
//! Git's own credentials. It reads the noted runs there, removes the repository, and writes
//! `.uphill/dashboard.html` in the current directory.

use std::io::{self, Write as _};
use std::path::Path;
use std::process::ExitCode;

use engine::dashboard::Repository;

use crate::history::{self, REPORTS};
use crate::score::TRENDS;
use crate::worktree;

/// Reads each repository's noted runs, prints a summary, and writes `.uphill/dashboard.html`.
///
/// # Errors
///
/// Returns a message for the terminal when Git cannot fetch a repository or the dashboard cannot
/// be written.
pub(crate) fn run(names: &[String]) -> Result<ExitCode, String> {
    let repositories = names
        .iter()
        .map(|name| read(name))
        .collect::<Result<Vec<_>, _>>()?;

    let mut out = engine::dashboard::terminal(&repositories);
    std::fs::create_dir_all(".uphill")
        .map_err(|error| format!("error: could not create .uphill: {error}"))?;
    std::fs::write(
        ".uphill/dashboard.html",
        engine::dashboard::html(&repositories),
    )
    .map_err(|error| format!("error: could not write .uphill/dashboard.html: {error}"))?;
    out.push_str("\nWrote .uphill/dashboard.html\n");
    let _ = io::stdout().write_all(out.as_bytes());
    Ok(ExitCode::SUCCESS)
}

/// Fetches one repository into a temporary bare repository and reads its noted runs.
fn read(name: &str) -> Result<Repository, String> {
    let url = url(name)?;
    let dir = tempfile::tempdir()
        .map_err(|error| format!("error: could not create a temporary directory: {error}"))?;
    let git = |args: &[&str]| worktree::git(dir.path(), args);
    let fetched = |result: Result<String, String>| {
        result.map_err(|error| {
            let error = error.strip_prefix("error: ").unwrap_or(&error);
            format!("error: could not fetch {name}: {error}")
        })
    };

    git(&["init", "--quiet", "--bare"])?;
    git(&["symbolic-ref", "HEAD", "refs/heads/main"])?;
    git(&["remote", "add", "origin", &url])?;
    // The notes come first and whole. Commits come without trees or files, which the runs never
    // read, so a large repository fetches in seconds.
    fetched(git(&[
        "fetch",
        "--quiet",
        "--no-tags",
        "origin",
        "+refs/notes/uphill*:refs/notes/uphill*",
    ]))?;
    fetched(git(&[
        "fetch",
        "--quiet",
        "--no-tags",
        "--filter=tree:0",
        "origin",
        "+HEAD:refs/heads/main",
    ]))?;

    let runs = history::runs(dir.path(), TRENDS, false);
    let report = runs
        .last()
        .and_then(|run| git(&["notes", "--ref", REPORTS, "show", &run.commit]).ok());
    Ok(Repository {
        name: name.to_owned(),
        report,
        runs,
    })
}

/// Returns what Git fetches for a repository: a local path as an absolute path, `owner/name` as
/// its GitHub URL, and anything else, such as a URL, as it is.
pub(crate) fn url(name: &str) -> Result<String, String> {
    let path = Path::new(name);
    if path.exists() {
        return path
            .canonicalize()
            .map(|path| path.to_string_lossy().into_owned())
            .map_err(|error| format!("error: could not read {name}: {error}"));
    }
    let github = name.split_once('/').is_some_and(|(owner, repository)| {
        // GitHub names never start with a dot, so a missing path such as `./app` stays a path.
        let part = |text: &str| {
            !text.is_empty()
                && !text.starts_with('.')
                && text
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        };
        part(owner) && part(repository)
    });
    Ok(if github {
        format!("https://github.com/{name}.git")
    } else {
        name.to_owned()
    })
}
