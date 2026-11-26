//! Fetches a pack from a Git repository without running any of its code.
//!
//! Git fetches into a temporary bare repository, so it writes no working tree and runs no hooks,
//! filters, LFS, or submodules. Uphill reads the pack's tree with `git ls-tree` and its files with
//! `git cat-file`, and checks the tree before it returns any file. Git never prompts, never fetches
//! a missing object behind Uphill's back, and speaks only `https`, or `file` and `ssh` when the
//! user's own configuration rewrites a URL to them.
//!
//! Fetched packs live in a cache outside the project, `$XDG_CACHE_HOME/uphill/packs/` or
//! `~/.cache/uphill/packs/`, one directory per commit and path. A commit names its tree exactly, so
//! a cached directory never changes, and every project and checkout on the machine shares it.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use engine::pack::{self, Remote, TreeEntry};
use tempfile::TempDir;

/// A file of a fetched pack.
pub(crate) struct File {
    /// The file's contents.
    pub(crate) bytes: Vec<u8>,
    /// Whether the file is executable.
    pub(crate) executable: bool,
    /// The path relative to the pack's directory.
    pub(crate) path: String,
}

/// A temporary bare repository that fetches from one remote, and the remote's refs.
pub(crate) struct Repository {
    dir: TempDir,
    /// Each ref of the remote and its commit, with tags peeled to their commits.
    refs: Vec<(String, String)>,
    /// The source as messages name it.
    source: String,
}

/// Returns the directory of the pack `remote` at its pinned commit, and fetches it into the cache
/// when the cache does not hold it. The fetch checks that a branch or tag reaches the commit and
/// checks the tree, as `uphill add` does, and runs none of the pack's code.
///
/// # Errors
///
/// Returns a message for the terminal when the entry has no commit, when the cache has no place,
/// when Git cannot fetch the commit, or when a check fails.
pub(crate) fn pack(remote: &Remote) -> Result<PathBuf, String> {
    let Some(commit) = remote.reference.as_deref().filter(|r| pack::is_commit(r)) else {
        return Err(format!(
            "error: pack `{}` is not pinned to a full commit",
            remote.name()
        ));
    };
    let dir = cached(remote, commit)?;
    if dir.is_dir() {
        return Ok(dir);
    }

    let repository = open(remote)?;
    repository.reachable(commit)?;
    let files = repository.files(commit, &remote.path)?;
    store(remote, commit, &files)
}

/// Returns the directory that holds `remote` at `commit` in the cache, whether or not it exists.
///
/// # Errors
///
/// Returns a message for the terminal when neither `XDG_CACHE_HOME` nor `HOME` is set.
fn cached(remote: &Remote, commit: &str) -> Result<PathBuf, String> {
    let root = std::env::var_os("XDG_CACHE_HOME")
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .filter(|dir| !dir.is_empty())
                .map(|home| Path::new(&home).join(".cache"))
        })
        .ok_or(
            "error: neither XDG_CACHE_HOME nor HOME is set, so Uphill has no place to cache packs",
        )?;
    // `@` cannot occur in a pack's path, so each path has its own directory, and the commit's
    // root is `@`.
    let path = if remote.path.is_empty() {
        "@".to_owned()
    } else {
        remote.path.replace('/', "@")
    };
    Ok(root.join("uphill/packs").join(commit).join(path))
}

/// Writes `files` of `remote` at `commit` to the cache and returns their directory. The files go
/// to a temporary directory first, which takes its place only once every file is written.
///
/// # Errors
///
/// Returns a message for the terminal when a file cannot be written.
pub(crate) fn store(remote: &Remote, commit: &str, files: &[File]) -> Result<PathBuf, String> {
    use std::os::unix::fs::PermissionsExt as _;

    let target = cached(remote, commit)?;
    if target.is_dir() {
        return Ok(target);
    }
    let parent = target.parent().unwrap_or(&target);
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("error: could not create {}: {error}", parent.display()))?;
    let temp = tempfile::Builder::new()
        .prefix(".tmp-")
        .tempdir_in(parent)
        .map_err(|error| format!("error: could not create a temporary directory: {error}"))?;

    for file in files {
        let path = temp.path().join(&file.path);
        path.parent()
            .map(std::fs::create_dir_all)
            .transpose()
            .and_then(|_| std::fs::write(&path, &file.bytes))
            .and_then(|()| {
                let mode = if file.executable { 0o755 } else { 0o644 };
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode))
            })
            .map_err(|error| format!("error: could not write {}: {error}", path.display()))?;
    }

    // Another run that cached the same commit first wrote the same files.
    let kept = temp.keep();
    if let Err(error) = std::fs::rename(&kept, &target) {
        let _ = std::fs::remove_dir_all(&kept);
        if !target.is_dir() {
            return Err(format!(
                "error: could not write {}: {error}",
                target.display()
            ));
        }
    }
    Ok(target)
}

/// Creates a temporary bare repository for `remote` and lists the remote's refs.
///
/// # Errors
///
/// Returns a message for the terminal when Git cannot run or cannot reach the remote.
pub(crate) fn open(remote: &Remote) -> Result<Repository, String> {
    let dir = tempfile::tempdir()
        .map_err(|error| format!("error: could not create a temporary directory: {error}"))?;
    let repository = Repository {
        dir,
        refs: Vec::new(),
        source: remote.name(),
    };
    repository.git(&["init", "--quiet", "--bare"])?;
    repository.git(&["remote", "add", "origin", &remote.url()])?;

    let listed = repository.git(&["ls-remote", "origin"])?;
    let mut refs: Vec<(String, String)> = Vec::new();
    for line in listed.lines() {
        let Some((commit, name)) = line.split_once('\t') else {
            continue;
        };
        // A peeled tag, `refs/tags/v1^{}`, names the commit that the tag object points to.
        if let Some(tag) = name.strip_suffix("^{}") {
            refs.retain(|(other, _)| other != tag);
            refs.push((tag.to_owned(), commit.to_owned()));
        } else {
            refs.push((name.to_owned(), commit.to_owned()));
        }
    }
    Ok(Repository { refs, ..repository })
}

impl Repository {
    /// Returns the commit that `reference` names: itself when it is a full commit, a branch or a
    /// tag, or the default branch for `None`.
    ///
    /// # Errors
    ///
    /// Returns a message when the ref names nothing, or names both a branch and a tag.
    pub(crate) fn resolve(&self, reference: Option<&str>) -> Result<String, String> {
        let Some(reference) = reference else {
            return self
                .commit("HEAD")
                .ok_or_else(|| format!("error: {} has no default branch", self.source));
        };
        if pack::is_commit(reference) {
            return Ok(reference.to_owned());
        }

        let branch = self.commit(&format!("refs/heads/{reference}"));
        let tag = self.commit(&format!("refs/tags/{reference}"));
        match (branch, tag) {
            (Some(_), Some(_)) => Err(format!(
                "error: `{reference}` names both a branch and a tag in {}. Name a full commit instead",
                self.source
            )),
            (Some(commit), None) | (None, Some(commit)) => Ok(commit),
            (None, None) => Err(format!(
                "error: {} has no branch or tag `{reference}`. Name a branch, a tag, or a full commit",
                self.source
            )),
        }
    }

    /// Returns the commit of the ref `name`.
    fn commit(&self, name: &str) -> Option<String> {
        self.refs
            .iter()
            .find(|(other, _)| other == name)
            .map(|(_, commit)| commit.clone())
    }

    /// Checks that `commit` belongs to the repository: a branch or tag reaches it. A commit that
    /// only a fork has can still be fetched through its parent's URL, and this rejects it.
    ///
    /// # Errors
    ///
    /// Returns a message when no branch or tag reaches the commit.
    pub(crate) fn reachable(&self, commit: &str) -> Result<(), String> {
        let tip = self.refs.iter().any(|(name, other)| {
            other == commit && (name.starts_with("refs/heads/") || name.starts_with("refs/tags/"))
        });
        if tip {
            return Ok(());
        }

        self.git(&[
            "fetch",
            "--quiet",
            "--no-tags",
            "--filter=tree:0",
            "origin",
            "+refs/heads/*:refs/remotes/origin/*",
            "+refs/tags/*:refs/tags/*",
        ])?;
        let contains = self
            .git(&[
                "for-each-ref",
                "--count=1",
                "--format=%(refname)",
                "--contains",
                commit,
                "refs/remotes",
                "refs/tags",
            ])
            .unwrap_or_default();
        if contains.is_empty() {
            return Err(format!(
                "error: no branch or tag of {} contains the commit {commit}. It may come from a fork",
                self.source
            ));
        }
        Ok(())
    }

    /// Fetches the history of `commit`, without trees or files, for [`Self::descends`] and
    /// [`Self::dates`].
    ///
    /// # Errors
    ///
    /// Returns a message when Git cannot fetch the commit.
    pub(crate) fn history(&self, commit: &str) -> Result<(), String> {
        self.git(&[
            "fetch",
            "--quiet",
            "--no-tags",
            "--filter=tree:0",
            "origin",
            commit,
        ])
        .map(|_| ())
    }

    /// Returns whether `commit` descends from `ancestor`, after [`Self::history`] fetched it.
    pub(crate) fn descends(&self, commit: &str, ancestor: &str) -> bool {
        self.git(&["merge-base", "--is-ancestor", ancestor, commit])
            .is_ok()
    }

    /// Returns the committer time of `commit` in seconds since the Unix epoch, and the latest
    /// committer time of its parents, after [`Self::history`] fetched it.
    ///
    /// # Errors
    ///
    /// Returns a message when Git cannot read the commits.
    pub(crate) fn dates(&self, commit: &str) -> Result<(i64, Option<i64>), String> {
        let read = |commit: &str| {
            let text = self.git(&["log", "-1", "--format=%ct %P", commit])?;
            let mut parts = text.split(' ');
            let time = parts
                .next()
                .and_then(|time| time.parse::<i64>().ok())
                .ok_or_else(|| format!("error: could not read the date of {commit}"))?;
            Ok::<_, String>((time, parts.map(str::to_owned).collect::<Vec<_>>()))
        };
        let (time, parents) = read(commit)?;
        let mut latest = None;
        for parent in parents.iter().filter(|parent| !parent.is_empty()) {
            let (parent, _) = read(parent)?;
            latest = latest.max(Some(parent));
        }
        Ok((time, latest))
    }

    /// Fetches the pack at `path` in `commit` and returns its files, after checking its tree with
    /// [`pack::validate`]. Run it after the checks that need history, since it fetches only the
    /// one commit.
    ///
    /// # Errors
    ///
    /// Returns a message when Git cannot fetch the commit, when the commit has no directory at
    /// `path`, and when the tree fails a check.
    pub(crate) fn files(&self, commit: &str, path: &str) -> Result<Vec<File>, String> {
        // `--refetch` fetches the commit's trees and files even when its history is here. Files
        // over the limit stay behind, and their size reads as missing.
        let limit = format!("--filter=blob:limit={}", pack::MAX_FILE + 1);
        self.git(&[
            "fetch",
            "--quiet",
            "--no-tags",
            "--refetch",
            "--depth=1",
            &limit,
            "origin",
            commit,
        ])?;

        let tree = if path.is_empty() {
            commit.to_owned()
        } else {
            format!("{commit}:{path}")
        };
        if self.git(&["cat-file", "-t", &tree]).ok().as_deref() != Some("tree") {
            return Err(format!(
                "error: {} has no directory `{path}` at {commit}",
                self.source
            ));
        }

        let listed = self.git_bytes(&["ls-tree", "-r", "-z", &tree], b"")?;
        let mut entries = Vec::new();
        let mut objects = Vec::new();
        for record in listed
            .split(|byte| *byte == 0)
            .filter(|record| !record.is_empty())
        {
            let record = String::from_utf8_lossy(record);
            let Some((meta, path)) = record.split_once('\t') else {
                continue;
            };
            let mut fields = meta.split(' ');
            let (Some(mode), Some(_), Some(object)) = (fields.next(), fields.next(), fields.next())
            else {
                continue;
            };
            entries.push(TreeEntry {
                mode: mode.to_owned(),
                path: path.to_owned(),
                size: None,
            });
            objects.push(object.to_owned());
        }

        // Sizes come before contents, so a file over the limit is never read.
        let input = objects.join("\n") + "\n";
        let checked = self.git_bytes(&["cat-file", "--batch-check"], input.as_bytes())?;
        let sizes = String::from_utf8_lossy(&checked);
        for (entry, line) in entries.iter_mut().zip(sizes.lines()) {
            let mut fields = line.split(' ');
            entry.size = match (fields.next(), fields.next(), fields.next()) {
                (_, Some("blob"), Some(size)) => size.parse().ok(),
                _ => None,
            };
        }
        let executable = pack::validate(&entries).map_err(|message| {
            format!("error: {} at {commit} is unsafe: {message}", self.source)
        })?;

        let contents = self.git_bytes(&["cat-file", "--batch"], input.as_bytes())?;
        let mut rest = contents.as_slice();
        let mut files = Vec::new();
        for (entry, executable) in entries.into_iter().zip(executable) {
            let header_end = rest
                .iter()
                .position(|byte| *byte == b'\n')
                .unwrap_or(rest.len());
            let size = usize::try_from(entry.size.unwrap_or_default()).unwrap_or(usize::MAX);
            let start = header_end + 1;
            let bytes = rest
                .get(start..start + size)
                .ok_or_else(|| {
                    format!("error: could not read {} from {}", entry.path, self.source)
                })?
                .to_vec();
            rest = rest.get(start + size + 1..).unwrap_or_default();
            files.push(File {
                bytes,
                executable,
                path: entry.path,
            });
        }
        Ok(files)
    }

    /// Runs Git in the repository and returns its standard output without the trailing newline.
    fn git(&self, args: &[&str]) -> Result<String, String> {
        let bytes = self.git_bytes(args, b"")?;
        Ok(String::from_utf8_lossy(&bytes).trim_end().to_owned())
    }

    /// Runs Git in the repository with `input` on standard input, and returns its standard output.
    fn git_bytes(&self, args: &[&str], input: &[u8]) -> Result<Vec<u8>, String> {
        let mut child = command(self.dir.path())
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("error: could not run git: {error}"))?;

        // A thread writes standard input, so a full output pipe cannot block the write.
        let mut stdin = child.stdin.take();
        let input = input.to_vec();
        let writer = std::thread::spawn(move || {
            if let Some(stdin) = stdin.as_mut() {
                let _ = stdin.write_all(&input);
            }
        });
        let mut stdout = Vec::new();
        if let Some(mut out) = child.stdout.take() {
            let _ = out.read_to_end(&mut stdout);
        }
        let _ = writer.join();
        let output = child
            .wait_with_output()
            .map_err(|error| format!("error: could not run git: {error}"))?;

        if !output.status.success() {
            let stderr = pack::printable(String::from_utf8_lossy(&output.stderr).trim());
            return Err(format!(
                "error: could not fetch {}: `git {}` failed: {stderr}",
                self.source,
                args.first().unwrap_or(&"")
            ));
        }
        Ok(stdout)
    }
}

/// Returns a Git command for the repository in `dir` that cannot prompt, fetch lazily, run hooks,
/// or use a transport other than `https`, and `file` or `ssh` that the user configured.
fn command(dir: &Path) -> Command {
    let mut command = Command::new("git");
    command
        .current_dir(dir)
        .args([
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "protocol.allow=never",
            "-c",
            "protocol.https.allow=always",
            "-c",
            "protocol.file.allow=user",
            "-c",
            "protocol.ssh.allow=user",
        ])
        .env("GIT_LFS_SKIP_SMUDGE", "1")
        .env("GIT_NO_LAZY_FETCH", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env_remove("GIT_PROTOCOL_FROM_USER");
    if std::env::var_os("GIT_ASKPASS").is_none() {
        command.env("GIT_ASKPASS", "echo");
    }
    if std::env::var_os("GIT_SSH_COMMAND").is_none() {
        command.env("GIT_SSH_COMMAND", "ssh -oBatchMode=yes");
    }
    command
}
