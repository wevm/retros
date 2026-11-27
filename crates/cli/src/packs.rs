//! `uphill add`, `uphill update`, and `uphill remove`: the commands that pin a project's packs.
//!
//! `add` and `update` fetch a pack at one commit (see [`crate::fetch`]), check that a branch or
//! tag of the repository reaches it and that it is at least a day old, print its files and
//! capabilities, and write nothing until the user confirms. The entry in the configuration takes
//! the full commit, and the files go to the cache of fetched packs, which runs read. `update` also
//! refuses a commit that does not descend from the pinned one. No command here runs a pack's code.
//! After a change to the configuration, each command rewrites the rules file (see [`crate::rules`]).

use std::fmt::Write as _;
use std::io::{self, BufRead as _, IsTerminal as _, Write as _};
use std::path::Path;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use engine::config;
use engine::pack::{self, Entry, Source};
use engine::scan;

use crate::error::Error;
use crate::fetch::{self, File};
use crate::project::{self, Configuration, PLACES};
use crate::rules;

/// The youngest commit that `add` and `update` take without `--allow-fresh`, in seconds.
const MIN_AGE: i64 = 24 * 60 * 60;

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
pub(crate) fn add(source: &str, categories: &[String], flags: Flags) -> Result<ExitCode, Error> {
    let found = configuration()?;
    let parsed = pack::source(source).map_err(|error| format!("error: {}", error.message))?;
    let entries = entries(&found)?;

    let Source::Remote(remote) = parsed else {
        let dir = found.root.join(source);
        if !PLACES.iter().any(|(place, _)| dir.join(place).is_file()) {
            return Err(format!(
                "error: {source} has no uphill.yaml or uphill.toml, at its root, in .agents, or in .agents/uphill"
            ).into());
        }
        let text = edit(
            pack::add(&found.source, found.format, source, categories, false, None),
            &found,
        )?;
        write(&found.root.join(found.place), text.as_bytes())?;
        print(&format!("Added {source} to {}\n", found.name));
        rules::refresh();
        return Ok(ExitCode::SUCCESS);
    };

    let name = remote.name();
    if !pack::permits(&found.source, found.format, &name)
        .map_err(|error| crate::diagnostic("error", &error, &found.name, &found.source))?
    {
        return Err(format!(
            "error: pack `{name}` matches no pattern of `allowed-packs` in {}",
            found.name
        )
        .into());
    }
    if let Some(entry) = entries
        .iter()
        .find(|entry| entry.directory() == remote.directory())
    {
        return Err(format!(
            "error: {} already lists `{}`. Run `uphill update {}` to move it",
            found.name,
            entry.name,
            entry.directory()
        )
        .into());
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
        return Err(format!("error: pack `{name}` has no category `{missing}`").into());
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
    fetch::store(&remote, &commit, &files)?;
    write(&found.root.join(found.place), text.as_bytes())?;

    print(&format!("\nAdded {name} to {}\n", found.name));
    rules::refresh();
    let mut out = String::new();
    if let Some(readme) = files.iter().find(|file| file.path == "README.md") {
        let _ = write!(
            out,
            "\n{}\n",
            pack::printable(String::from_utf8_lossy(&readme.bytes).trim_end())
        );
    }
    out.push_str("\nCommit these files, then run `uphill check`\n");
    print(&out);
    Ok(ExitCode::SUCCESS)
}

/// Moves each remote pack, or the ones `names` lists, to the newest commit of the ref it follows.
///
/// # Errors
///
/// Returns a message for the terminal when a name lists no pack, when a check fails, when the user
/// declines, or when a file cannot be written.
pub(crate) fn update(names: &[String], flags: Flags) -> Result<ExitCode, Error> {
    let found = configuration()?;
    let entries = select(&found, names)?;
    let mut text = found.source.clone();
    let mut declined = false;
    let mut updated = false;

    for entry in entries {
        let Source::Remote(remote) = &entry.source else {
            continue;
        };
        let pinned = remote.reference.clone().unwrap_or_default();

        let repository = fetch::open(remote)?;
        let commit = repository.resolve(entry.follow.as_deref())?;
        if commit == pinned {
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
            ).into());
        }
        let age = age(&repository, &commit, flags.allow_fresh)?;
        let files = repository.files(&commit, &remote.path)?;
        read_pack(&files, &entry.name)?;
        let before = repository.files(&pinned, &remote.path)?;

        let mut out = format!(
            "{}  {} → {}{}\n",
            entry.directory(),
            short(&pinned),
            short(&commit),
            detail(entry.follow.as_deref(), &age)
        );
        out.push_str(&scan::review(Some(&views(&before)), &views(&files)));
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

        fetch::store(remote, &commit, &files)?;
        let current = pack::entries(&text, found.format)
            .map_err(|error| crate::diagnostic("error", &error, &found.name, &text))?;
        if let Some(current) = current.iter().find(|other| other.name == entry.name) {
            text = edit(pack::pin(&text, found.format, current, &commit), &found)?;
        }
        write(&found.root.join(found.place), text.as_bytes())?;
        print(&format!("updated  {} to {}\n", entry.name, short(&commit)));
        updated = true;
    }
    if updated {
        rules::refresh();
    }
    Ok(if declined {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}

/// Removes the pack `name` from the configuration.
///
/// # Errors
///
/// Returns a message for the terminal when the name lists no pack, or a file cannot be written.
pub(crate) fn remove(name: &str) -> Result<ExitCode, Error> {
    let found = configuration()?;
    let entries = select(&found, &[name.to_owned()])?;
    let Some(entry) = entries.first() else {
        return Err(format!("error: {} lists no pack `{name}`", found.name).into());
    };

    let text = edit(pack::remove(&found.source, found.format, entry), &found)?;
    write(&found.root.join(found.place), text.as_bytes())?;
    print(&format!("Removed {} from {}\n", entry.name, found.name));
    rules::refresh();
    Ok(ExitCode::SUCCESS)
}

/// Finds the project's configuration from the current directory.
fn configuration() -> Result<Configuration, Error> {
    let cwd = std::env::current_dir()
        .map_err(|error| format!("error: could not read the current directory: {error}"))?;
    project::configuration(&cwd)
}

/// Returns the entries of the project's `packs` key.
fn entries(found: &Configuration) -> Result<Vec<Entry>, Error> {
    pack::entries(&found.source, found.format)
        .map_err(|error| crate::diagnostic("error", &error, &found.name, &found.source).into())
}

/// Returns the entries that `names` lists by directory or source, or every entry without names.
fn select(found: &Configuration, names: &[String]) -> Result<Vec<Entry>, Error> {
    let entries = entries(found)?;
    if let Some(name) = names.iter().find(|name| {
        !entries
            .iter()
            .any(|entry| entry.directory() == name.as_str() || entry.name == name.as_str())
    }) {
        return Err(format!("error: {} lists no pack `{name}`", found.name).into());
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
fn age(repository: &fetch::Repository, commit: &str, allow_fresh: bool) -> Result<String, Error> {
    let (time, parent) = repository.dates(commit)?;
    if parent.is_some_and(|parent| time < parent) {
        return Err(format!(
            "error: the commit {} is dated before its parent, so its date cannot be trusted",
            short(commit)
        )
        .into());
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
        ).into());
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
fn read_pack(files: &[File], name: &str) -> Result<Vec<String>, Error> {
    let Some((place, format, file)) = PLACES.iter().find_map(|(place, format)| {
        files
            .iter()
            .find(|file| file.path == *place)
            .map(|file| (*place, *format, file))
    }) else {
        return Err(format!(
            "error: pack `{name}` has no uphill.yaml or uphill.toml, at its root, in .agents, or in .agents/uphill"
        ).into());
    };
    let source = String::from_utf8_lossy(&file.bytes);
    let shown = format!("{name}/{place}");
    let nested = pack::entries(&source, format)
        .map_err(|error| crate::diagnostic("error", &error, &shown, &source))?;
    if !nested.is_empty() {
        return Err(format!(
            "error: {shown} lists packs. A pack's own `packs` are not read, so list them in the project"
        ).into());
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

/// Asks the user to confirm, unless `yes`. Without a terminal on standard input, it requires `yes`.
fn confirm(yes: bool, question: &str) -> Result<bool, Error> {
    if yes {
        return Ok(true);
    }
    if !io::stdin().is_terminal() {
        return Err("error: standard input is not a terminal, so nothing was written. Review the summary above, then run again with --yes".into());
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
) -> Result<String, Error> {
    result.map_err(|error| crate::diagnostic("error", &error, &found.name, &found.source).into())
}

/// Writes a file and the directories above it.
fn write(path: &Path, contents: &[u8]) -> Result<(), Error> {
    path.parent()
        .map(std::fs::create_dir_all)
        .transpose()
        .and_then(|_| std::fs::write(path, contents))
        .map_err(|error| format!("error: could not write {}: {error}", path.display()).into())
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
