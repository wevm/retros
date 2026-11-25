//! `uphill add`, `uphill update`, `uphill remove`, and `uphill audit`: the commands that pin,
//! vendor, and check a project's remote packs.
//!
//! `add` and `update` fetch a pack at one commit (see [`crate::fetch`]), check that a branch or
//! tag of the repository reaches it and that it is at least a day old, print its files and
//! capabilities, and write nothing until the user confirms. The files go to
//! `.agents/packs/<name>/`, which the project commits, and the entry in the configuration takes
//! the full commit. `update` also refuses a commit that does not descend from the pinned one.
//! `audit` fetches each pinned commit again and compares it with the vendored files. No command
//! here runs a pack's code.

use std::fmt::Write as _;
use std::io::{self, BufRead as _, IsTerminal as _, Write as _};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use engine::config::{self, Format};
use engine::pack::{self, Entry, Source};
use engine::scan;

use crate::fetch::{self, File};
use crate::project::{self, Configuration};

/// The youngest commit that `add` and `update` take without `--allow-fresh`, in seconds.
const MIN_AGE: i64 = 24 * 60 * 60;

/// Where a pack keeps its configuration, relative to its directory.
const PLACES: [(&str, Format); 4] = [
    ("uphill.yaml", Format::Yaml),
    ("uphill.toml", Format::Toml),
    (".agents/uphill.yaml", Format::Yaml),
    (".agents/uphill.toml", Format::Toml),
];

/// The flags of `add` and `update`.
#[derive(Clone, Copy)]
pub(crate) struct Flags {
    /// Whether to take a commit younger than a day.
    pub(crate) allow_fresh: bool,
    /// Whether to write without asking.
    pub(crate) yes: bool,
}

/// Adds the pack `source` to the project, with only `categories` when any are named.
///
/// # Errors
///
/// Returns a message for the terminal when the source is invalid or already listed, when a check
/// fails, when the user declines, or when a file cannot be written.
pub(crate) fn add(source: &str, categories: &[String], flags: Flags) -> Result<ExitCode, String> {
    let found = configuration()?;
    let parsed = pack::source(source).map_err(|message| format!("error: {message}"))?;
    let entries = entries(&found)?;

    let Source::Remote(remote) = parsed else {
        let dir = found.root.join(source);
        if !PLACES.iter().any(|(place, _)| dir.join(place).is_file()) {
            return Err(format!(
                "error: {source} has no uphill.yaml or uphill.toml, at its root or in .agents"
            ));
        }
        let text = edit(
            pack::add(&found.source, found.format, source, categories, false, None),
            &found,
        )?;
        write(&found.root.join(found.place), text.as_bytes())?;
        print(&format!("Added {source} to {}\n", found.name));
        return Ok(ExitCode::SUCCESS);
    };

    let name = remote.name();
    if !pack::permits(&found.source, found.format, &name)
        .map_err(|error| crate::diagnostic("error", &error, &found.name, &found.source))?
    {
        return Err(format!(
            "error: pack `{name}` matches no pattern of `allowed-packs` in {}",
            found.name
        ));
    }
    let directory = Path::new(".agents/packs").join(remote.directory());
    if let Some(entry) = entries
        .iter()
        .find(|entry| entry.directory() == remote.directory())
    {
        return Err(format!(
            "error: {} already lists `{}`. Run `uphill update {}` to move it",
            found.name,
            entry.name,
            entry.directory()
        ));
    }
    if found.root.join(&directory).exists() {
        return Err(format!(
            "error: {} exists. Move it, then run `uphill add` again",
            directory.display()
        ));
    }

    let repository = fetch::open(&remote)?;
    let commit = repository.resolve(remote.reference.as_deref())?;
    repository.reachable(&commit)?;
    repository.history(&commit)?;
    let age = age(&repository, &commit, flags.allow_fresh)?;
    let files = repository.files(&commit, &remote.path)?;
    let pack_categories = read_pack(&files, &name)?;
    if let Some(missing) = categories
        .iter()
        .find(|wanted| !pack_categories.contains(wanted))
    {
        return Err(format!("error: pack `{name}` has no category `{missing}`"));
    }

    let follow = remote
        .reference
        .as_deref()
        .filter(|reference| !pack::is_commit(reference));
    let install = files.iter().any(|file| file.path == "package.json");
    let mut out = format!(
        "{}  {}{}\n",
        remote.directory(),
        remote.pinned(&short(&commit)),
        detail(follow, &age)
    );
    out.push_str(&scan::review(None, &views(&files)));
    if install {
        out.push_str("\nIts dependencies install from its lockfile, without install scripts, before its categories run\n");
    }
    print(&out);
    if !confirm(flags.yes, "Add this pack?")? {
        print("Nothing was written\n");
        return Ok(ExitCode::FAILURE);
    }

    let text = edit(
        pack::add(
            &found.source,
            found.format,
            &remote.pinned(&commit),
            categories,
            install,
            follow,
        ),
        &found,
    )?;
    vendor(&found.root, &directory, &files)?;
    write(&found.root.join(found.place), text.as_bytes())?;

    let mut out = format!(
        "\nAdded {name} to {} and {}/\n",
        found.name,
        directory.display()
    );
    if let Some(readme) = files.iter().find(|file| file.path == "README.md") {
        let _ = write!(
            out,
            "\n{}\n",
            pack::printable(String::from_utf8_lossy(&readme.bytes).trim_end())
        );
    }
    out.push_str("\nCommit both, then run `uphill check`\n");
    print(&out);
    Ok(ExitCode::SUCCESS)
}

/// Moves each remote pack, or the ones `names` lists, to the newest commit of the ref it follows.
///
/// # Errors
///
/// Returns a message for the terminal when a name lists no pack, when a check fails, when the user
/// declines, or when a file cannot be written.
pub(crate) fn update(names: &[String], flags: Flags) -> Result<ExitCode, String> {
    let found = configuration()?;
    let entries = select(&found, names)?;
    let mut text = found.source.clone();
    let mut declined = false;

    for entry in entries {
        let Source::Remote(remote) = &entry.source else {
            continue;
        };
        let pinned = remote.reference.clone().unwrap_or_default();
        let directory = project::directory(&entry);
        let vendored = read_dir(&found.root.join(&directory))?;

        let repository = fetch::open(remote)?;
        let commit = repository.resolve(entry.follow.as_deref())?;
        if commit == pinned && vendored.is_some() {
            print(&format!("current  {} at {}\n", entry.name, short(&commit)));
            continue;
        }
        repository.reachable(&commit)?;
        repository.history(&commit)?;
        if commit != pinned && !repository.descends(&commit, &pinned) {
            return Err(format!(
                "error: {} at {} does not descend from the pinned commit {}. The ref may have been force-pushed, or the repository replaced. Review it, then run `uphill remove {}` and `uphill add`",
                entry.name,
                short(&commit),
                short(&pinned),
                entry.directory()
            ));
        }
        let age = age(&repository, &commit, flags.allow_fresh)?;
        let files = repository.files(&commit, &remote.path)?;
        read_pack(&files, &entry.name)?;

        let before = vendored.as_deref().map(views);
        let mut out = format!(
            "{}  {} → {}{}\n",
            entry.directory(),
            short(&pinned),
            short(&commit),
            detail(entry.follow.as_deref(), &age)
        );
        out.push_str(&scan::review(before.as_deref(), &views(&files)));
        if files.iter().any(|file| file.path == "package.json") && !entry.install {
            out.push_str(
                "\nIt has dependencies. Set `install: true` on its entry to install them\n",
            );
        }
        print(&out);
        if !confirm(flags.yes, "Write this update?")? {
            print("Skipped\n");
            declined = true;
            continue;
        }

        vendor(&found.root, &directory, &files)?;
        let current = pack::entries(&text, found.format)
            .map_err(|error| crate::diagnostic("error", &error, &found.name, &text))?;
        if let Some(current) = current.iter().find(|other| other.name == entry.name) {
            text = edit(pack::pin(&text, found.format, current, &commit), &found)?;
        }
        write(&found.root.join(found.place), text.as_bytes())?;
        print(&format!("updated  {} to {}\n", entry.name, short(&commit)));
    }
    Ok(if declined {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}

/// Removes the pack `name` from the configuration, and its vendored files for a remote pack.
///
/// # Errors
///
/// Returns a message for the terminal when the name lists no pack, or a file cannot be written.
pub(crate) fn remove(name: &str) -> Result<ExitCode, String> {
    let found = configuration()?;
    let entries = select(&found, &[name.to_owned()])?;
    let Some(entry) = entries.first() else {
        return Err(format!("error: {} lists no pack `{name}`", found.name));
    };

    let text = edit(pack::remove(&found.source, found.format, entry), &found)?;
    write(&found.root.join(found.place), text.as_bytes())?;
    let mut out = format!("Removed {} from {}\n", entry.name, found.name);
    if matches!(entry.source, Source::Remote(_)) {
        let directory = project::directory(entry);
        let dir = found.root.join(&directory);
        if dir.exists() {
            std::fs::remove_dir_all(&dir).map_err(|error| {
                format!("error: could not remove {}: {error}", directory.display())
            })?;
            let _ = writeln!(out, "Removed {}/", directory.display());
        }
    }
    print(&out);
    Ok(ExitCode::SUCCESS)
}

/// Fetches each remote pack at its pinned commit and compares it with the vendored files.
///
/// # Errors
///
/// Returns a message for the terminal when the configuration is invalid. A pack that differs or
/// fails a check is reported in the output, with exit status 1.
pub(crate) fn audit() -> Result<ExitCode, String> {
    let found = configuration()?;
    let entries = entries(&found)?;
    let mut failed = false;

    for entry in &entries {
        let Source::Remote(remote) = &entry.source else {
            continue;
        };
        let pinned = remote.reference.as_deref().unwrap_or_default();
        let directory = project::directory(entry);
        let result = (|| {
            let repository = fetch::open(remote)?;
            repository.reachable(pinned)?;
            let files = repository.files(pinned, &remote.path)?;
            let vendored = read_dir(&found.root.join(&directory))?
                .ok_or_else(|| format!("{} is missing", directory.display()))?;
            Ok::<_, String>(differences(&files, &vendored))
        })();

        let line = match result {
            Ok(differences) if differences.is_empty() => {
                format!("ok       {} at {}\n", entry.name, short(pinned))
            }
            Ok(differences) => {
                failed = true;
                let mut line = format!("differs  {} at {}\n", entry.name, short(pinned));
                for difference in differences {
                    let _ = writeln!(line, "  {}", pack::printable(&difference));
                }
                line
            }
            Err(message) => {
                failed = true;
                let message = message.strip_prefix("error: ").unwrap_or(&message);
                format!("failed   {}: {message}\n", entry.name)
            }
        };
        print(&line);
    }
    if entries
        .iter()
        .all(|entry| matches!(entry.source, Source::Local(_)))
    {
        print(&format!("{} lists no remote packs\n", found.name));
    }
    Ok(if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}

/// Finds the project's configuration from the current directory.
fn configuration() -> Result<Configuration, String> {
    let cwd = std::env::current_dir()
        .map_err(|error| format!("error: could not read the current directory: {error}"))?;
    project::configuration(&cwd)
}

/// Returns the entries of the project's `packs` key.
fn entries(found: &Configuration) -> Result<Vec<Entry>, String> {
    pack::entries(&found.source, found.format)
        .map_err(|error| crate::diagnostic("error", &error, &found.name, &found.source))
}

/// Returns the entries that `names` lists by directory or source, or every entry without names.
fn select(found: &Configuration, names: &[String]) -> Result<Vec<Entry>, String> {
    let entries = entries(found)?;
    if let Some(name) = names.iter().find(|name| {
        !entries
            .iter()
            .any(|entry| entry.directory() == name.as_str() || entry.name == name.as_str())
    }) {
        return Err(format!("error: {} lists no pack `{name}`", found.name));
    }
    Ok(entries
        .into_iter()
        .filter(|entry| {
            names.is_empty()
                || names
                    .iter()
                    .any(|name| entry.directory() == name.as_str() || entry.name == name.as_str())
        })
        .collect())
}

/// Checks the commit's age and returns how old it is, such as `3 days old`.
fn age(repository: &fetch::Repository, commit: &str, allow_fresh: bool) -> Result<String, String> {
    let (time, parent) = repository.dates(commit)?;
    if parent.is_some_and(|parent| time < parent) {
        return Err(format!(
            "error: the commit {} is dated before its parent, so its date cannot be trusted",
            short(commit)
        ));
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| {
            i64::try_from(since.as_secs()).unwrap_or(i64::MAX)
        });
    let seconds = now - time;
    if seconds < MIN_AGE && !allow_fresh {
        return Err(format!(
            "error: the commit {} is less than a day old. Wait for review of the change, or run again with --allow-fresh",
            short(commit)
        ));
    }
    Ok(match seconds / MIN_AGE {
        0 => "less than a day old".to_owned(),
        1 => "1 day old".to_owned(),
        days => format!("{days} days old"),
    })
}

/// Returns the text after a pack's commit in a review: the ref it follows and its age.
fn detail(follow: Option<&str>, age: &str) -> String {
    match follow {
        Some(follow) => format!(" ({follow}, {age})"),
        None => format!(" ({age})"),
    }
}

/// Parses a fetched pack's configuration and returns its category names.
fn read_pack(files: &[File], name: &str) -> Result<Vec<String>, String> {
    let Some((place, format, file)) = PLACES.iter().find_map(|(place, format)| {
        files
            .iter()
            .find(|file| file.path == *place)
            .map(|file| (*place, *format, file))
    }) else {
        return Err(format!(
            "error: pack `{name}` has no uphill.yaml or uphill.toml, at its root or in .agents"
        ));
    };
    let source = String::from_utf8_lossy(&file.bytes);
    let shown = format!("{name}/{place}");
    let nested = pack::entries(&source, format)
        .map_err(|error| crate::diagnostic("error", &error, &shown, &source))?;
    if !nested.is_empty() {
        return Err(format!(
            "error: {shown} lists packs. A pack's own `packs` are not read, so list them in the project"
        ));
    }
    let config = config::parse(&source, format)
        .map_err(|error| crate::diagnostic("error", &error, &shown, &source))?;
    Ok(config
        .categories
        .into_iter()
        .map(|category| category.name)
        .collect())
}

/// Returns the files as the scan reads them.
fn views(files: &[File]) -> Vec<scan::File<'_>> {
    files
        .iter()
        .map(|file| scan::File {
            bytes: &file.bytes,
            executable: file.executable,
            path: &file.path,
        })
        .collect()
}

/// Returns each difference between the fetched `files` and the `vendored` ones.
fn differences(files: &[File], vendored: &[File]) -> Vec<String> {
    let mut out = Vec::new();
    for file in files {
        match vendored.iter().find(|other| other.path == file.path) {
            None => out.push(format!("{} is missing", file.path)),
            Some(other) if other.bytes != file.bytes => out.push(format!("{} differs", file.path)),
            Some(other) if other.executable != file.executable => {
                out.push(format!("{} differs in its executable bit", file.path));
            }
            Some(_) => {}
        }
    }
    for other in vendored {
        if !files.iter().any(|file| file.path == other.path) {
            out.push(format!("{} is not in the pack", other.path));
        }
    }
    out
}

/// Reads the files of a vendored pack, apart from `node_modules`, or returns `None` when the
/// directory does not exist. A symbolic link reads as a file that holds its target, so it differs.
fn read_dir(dir: &Path) -> Result<Option<Vec<File>>, String> {
    if !dir.is_dir() {
        return Ok(None);
    }
    let mut files = Vec::new();
    let mut pending = vec![PathBuf::new()];
    while let Some(relative) = pending.pop() {
        let read = std::fs::read_dir(dir.join(&relative))
            .map_err(|error| format!("error: could not read {}: {error}", dir.display()))?;
        for item in read.filter_map(Result::ok) {
            let name = item.file_name();
            let path = relative.join(&name);
            let kind = item
                .file_type()
                .map_err(|error| format!("error: could not read {}: {error}", path.display()))?;
            if kind.is_dir() {
                if name != "node_modules" {
                    pending.push(path);
                }
                continue;
            }
            let full = dir.join(&path);
            let (bytes, executable) = if kind.is_symlink() {
                let target = std::fs::read_link(&full).unwrap_or_default();
                (target.to_string_lossy().into_owned().into_bytes(), false)
            } else {
                let bytes = std::fs::read(&full).map_err(|error| {
                    format!("error: could not read {}: {error}", full.display())
                })?;
                (bytes, executable(&full))
            };
            files.push(File {
                bytes,
                executable,
                path: path.to_string_lossy().into_owned(),
            });
        }
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(Some(files))
}

/// Returns whether the file at `path` is executable by its owner.
fn executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::metadata(path).is_ok_and(|metadata| metadata.permissions().mode() & 0o100 != 0)
}

/// Writes `files` to `directory` under `root`, replacing what it held. The files go to a
/// temporary directory first, which takes the place of the old one only once every file is
/// written.
fn vendor(root: &Path, directory: &Path, files: &[File]) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt as _;

    let packs = root.join(".agents/packs");
    std::fs::create_dir_all(&packs)
        .map_err(|error| format!("error: could not create .agents/packs: {error}"))?;
    let ignore = packs.join(".gitignore");
    if !ignore.exists() {
        write(&ignore, b"*/node_modules/\n")?;
    }

    let temp = tempfile::Builder::new()
        .prefix(".uphill-")
        .tempdir_in(&packs)
        .map_err(|error| format!("error: could not create a temporary directory: {error}"))?;
    for file in files {
        let path = temp.path().join(&file.path);
        write(&path, &file.bytes)?;
        let mode = if file.executable { 0o755 } else { 0o644 };
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode))
            .map_err(|error| format!("error: could not write {}: {error}", file.path))?;
    }

    let target = root.join(directory);
    if target.exists() {
        std::fs::remove_dir_all(&target)
            .map_err(|error| format!("error: could not remove {}: {error}", directory.display()))?;
    }
    let kept = temp.keep();
    std::fs::rename(&kept, &target).map_err(|error| {
        let _ = std::fs::remove_dir_all(&kept);
        format!("error: could not write {}: {error}", directory.display())
    })
}

/// Asks the user to confirm, unless `yes`. Without a terminal on standard input, it requires `yes`.
fn confirm(yes: bool, question: &str) -> Result<bool, String> {
    if yes {
        return Ok(true);
    }
    if !io::stdin().is_terminal() {
        return Err(
            "error: standard input is not a terminal, so nothing was written. Review the summary above, then run again with --yes"
                .to_owned(),
        );
    }
    print(&format!("\n{question} [y/N] "));
    let mut answer = String::new();
    io::stdin()
        .lock()
        .read_line(&mut answer)
        .map_err(|error| format!("error: could not read the answer: {error}"))?;
    Ok(matches!(answer.trim(), "y" | "Y" | "yes"))
}

/// Returns the result of a configuration edit, or its error for the terminal.
fn edit(
    result: Result<String, engine::error::Error>,
    found: &Configuration,
) -> Result<String, String> {
    result.map_err(|error| crate::diagnostic("error", &error, &found.name, &found.source))
}

/// Writes a file and the directories above it.
fn write(path: &Path, contents: &[u8]) -> Result<(), String> {
    path.parent()
        .map(std::fs::create_dir_all)
        .transpose()
        .and_then(|_| std::fs::write(path, contents))
        .map_err(|error| format!("error: could not write {}: {error}", path.display()))
}

/// Writes `text` to standard output and flushes it.
fn print(text: &str) {
    let mut stdout = io::stdout().lock();
    let _ = stdout.write_all(text.as_bytes());
    let _ = stdout.flush();
}

/// Returns the first 7 characters of a commit.
fn short(commit: &str) -> String {
    commit.chars().take(7).collect()
}
