//! `uphill log` and `uphill blame`: one metric across the commits of the repository, and the notes
//! under `refs/notes/uphill` that keep past runs.
//!
//! `log` reads the metric's baseline in each version of the baseline file, or of an older
//! configuration's `baseline` key, on the first-parent history, so it lists the commits that
//! accepted a new value. A metric without baselines, such as a timing, comes from the runs
//! `uphill --note` kept. `blame` names the commit that last
//! moved the metric. It reads the same baselines when they record the metric. Otherwise it checks
//! out commits in temporary worktrees, measures each, and bisects. Each commit runs the category's
//! `run` command from its own configuration when that declares the category, so a script that
//! moved still runs, and this checkout's command otherwise. An exact value moved at the first commit
//! that measures the same value as `HEAD`. A time moved at the first commit that is within noise of
//! `HEAD` in interleaved pairs.

use std::fmt::Write as _;
use std::io::{self, Write as _};
use std::path::Path;
use std::process::{Command, ExitCode, Stdio};

use engine::baseline;
use engine::compare::Timing;
use engine::config::Category;
use engine::history::Run;
use engine::lines::Line;
use engine::value::{Kind, Value};

use crate::error::Error;
use crate::project::{self, Project};
use crate::worktree::{self, Worktree};
use crate::{measure, score};

/// The metric a command follows.
pub(crate) struct Selector {
    pub(crate) category: String,
    /// The item, for a metric that covers a list.
    pub(crate) item: Option<String>,
    pub(crate) metric: String,
    /// The subject, when the category's lines name one, such as `zc`.
    pub(crate) subject: Option<String>,
}

impl Selector {
    /// Names the metric as the terminal report does, such as `zc footprint install-size`.
    fn name(&self) -> String {
        let mut name = self
            .subject
            .as_ref()
            .map(|subject| format!("{subject} "))
            .unwrap_or_default();
        let _ = write!(name, "{} {}", self.category, self.metric);
        match &self.item {
            Some(item) if item.contains(char::is_whitespace) => {
                let _ = write!(name, " \"{item}\"");
            }
            Some(item) => {
                let _ = write!(name, " {item}");
            }
            None => {}
        }
        name
    }

    /// Returns whether a line measures this metric.
    fn matches(&self, line: &Line<'_>) -> bool {
        line.metric == self.metric
            && line.subject == self.subject.as_deref()
            && line.item.as_deref() == self.item.as_deref()
    }

    /// Returns whether a timing measures this metric.
    fn times(&self, timing: &Timing) -> bool {
        timing.metric == self.metric && timing.subject == self.subject && timing.item == self.item
    }

    /// Returns the category this metric belongs to.
    ///
    /// # Errors
    ///
    /// Returns a message for the terminal when the configuration declares no such category, or
    /// when the category configures no such metric.
    fn category<'p>(&self, project: &'p Project) -> Result<&'p Category, Error> {
        let category = project.category(&self.category)?;
        if !category.metrics.iter().any(|m| m.name == self.metric) {
            let names: Vec<&str> = category.metrics.iter().map(|m| m.name.as_str()).collect();
            return Err(format!(
                "error: unknown metric '{}'. {} configures {}",
                self.metric,
                category.name,
                names.join(", ")
            )
            .into());
        }
        Ok(category)
    }
}

/// A commit on the first-parent history.
struct Commit {
    date: String,
    hash: String,
    short: String,
    title: String,
}

impl Commit {
    /// Reads the commits of `git log` with [`FORMAT`], one per line.
    fn parse(log: &str) -> Vec<Commit> {
        log.lines()
            .filter_map(|line| {
                let mut fields = line.splitn(4, '\t');
                let (hash, short, date) = (fields.next()?, fields.next()?, fields.next()?);
                Some(Commit {
                    date: date.to_owned(),
                    hash: hash.to_owned(),
                    short: short.to_owned(),
                    title: fields.next().unwrap_or_default().to_owned(),
                })
            })
            .collect()
    }
}

/// The `git log` format that [`Commit::parse`] reads.
const FORMAT: &str = "--format=%H%x09%h%x09%ad%x09%s";

/// A commit whose configuration records a different baseline than the commit before it.
struct Point {
    before: Option<Value>,
    commit: Commit,
    /// The recorded value, or `None` when the baseline is gone.
    value: Option<Value>,
}

/// Lists the commits that changed the baseline of the metric `selector` names, newest first.
///
/// # Errors
///
/// Returns a message for the terminal when the metric is unknown or Git fails.
pub(crate) fn log(project: &Project, selector: &Selector) -> Result<ExitCode, Error> {
    let category = selector.category(project)?;
    let points = points(project, selector)?;
    let name = selector.name();

    let mut out = String::new();
    if points.is_empty() {
        let rows = noted(project, category, selector);
        if rows.is_empty() {
            let _ = writeln!(
                out,
                "No baseline or note records {name}. `uphill --note` keeps each run, and `uphill blame` measures commits"
            );
        } else {
            let _ = writeln!(out, "{name} in the runs noted in {NOTES}\n");
            out.push_str(&table(&rows));
        }
    } else {
        let _ = writeln!(
            out,
            "{name} in the baselines of {}\n",
            project.baselines_name
        );
        let shown = |value: Option<Value>| value.map_or("none".to_owned(), engine::format::value);
        let rows: Vec<[String; 5]> = points
            .iter()
            .rev()
            .map(|point| {
                [
                    point.commit.short.clone(),
                    point.commit.date.clone(),
                    shown(point.value),
                    point
                        .before
                        .map(|before| format!("from {}", engine::format::value(before)))
                        .unwrap_or_default(),
                    point.commit.title.clone(),
                ]
            })
            .collect();
        out.push_str(&table(&rows));
    }

    let _ = io::stdout().write_all(out.as_bytes());
    Ok(ExitCode::SUCCESS)
}

/// Names the commit that last moved the metric `selector` names: from its baselines, or by
/// bisecting the commits since `since`, or every commit, when no baseline records it.
///
/// # Errors
///
/// Returns a message for the terminal when the metric is unknown, when Git fails, when a checkout
/// cannot be created or set up, or when `HEAD` does not measure the metric.
pub(crate) fn blame(
    project: &Project,
    selector: &Selector,
    since: Option<&str>,
) -> Result<ExitCode, Error> {
    let category = selector.category(project)?;
    let name = selector.name();

    let mut out = String::new();
    match points(project, selector)?.pop() {
        Some(Point {
            before,
            commit,
            value: Some(value),
        }) => {
            let value = engine::format::value(value);
            let _ = match before {
                Some(before) => writeln!(
                    out,
                    "{} moved {name} from {} to {value}, by the baselines in {}",
                    commit.short,
                    engine::format::value(before),
                    project.baselines_name
                ),
                None => writeln!(
                    out,
                    "{} recorded {name} at {value}, the first baseline in {}",
                    commit.short, project.baselines_name
                ),
            };
            let _ = writeln!(out, "  {}  {}", commit.date, commit.title);
        }
        _ => out = bisect(project, category, selector, since)?,
    }

    let _ = io::stdout().write_all(out.as_bytes());
    Ok(ExitCode::SUCCESS)
}

/// Reads the metric's baseline in each version of the configuration and its baseline file on the
/// first-parent history and returns the commits where it changed, oldest first.
///
/// # Errors
///
/// Returns a message for the terminal when the project is not in a Git repository or Git fails.
fn points(project: &Project, selector: &Selector) -> Result<Vec<Point>, Error> {
    worktree::repository(&project.root)?;
    let relative = |path: &Path| {
        path.strip_prefix(&project.root)
            .unwrap_or(path)
            .to_string_lossy()
            .into_owned()
    };
    let (place, sidecar) = (relative(&project.path), relative(&project.baselines));
    let log = worktree::git(
        &project.root,
        &[
            "log",
            "--first-parent",
            "--reverse",
            "--date=short",
            FORMAT,
            "--",
            &place,
            &sidecar,
        ],
    )?;

    let mut points = Vec::new();
    let mut last = None;
    for commit in Commit::parse(&log) {
        let show = |path: &str| {
            worktree::git(
                &project.root,
                &["show", &format!("{}:./{path}", commit.hash)],
            )
            .ok()
        };
        let baselines = show(&sidecar);
        let value = show(&place)
            .and_then(|source| {
                project::load(
                    &[&project.root],
                    &source,
                    baselines.as_deref(),
                    project.format,
                )
            })
            .and_then(|config| {
                baseline::value(
                    &config,
                    &selector.category,
                    &selector.metric,
                    selector.subject.as_deref(),
                    selector.item.as_deref(),
                )
            });
        if value != last {
            points.push(Point {
                before: last,
                commit,
                value,
            });
            last = value;
        }
    }
    Ok(points)
}

/// The ref that holds the notes of past runs.
pub(crate) const NOTES: &str = "refs/notes/uphill";

/// The ref that holds each noted run's `report.json`, for readers outside the repository.
pub(crate) const REPORTS: &str = "refs/notes/uphill-report";

/// Reads the runs noted on the first-parent history of `HEAD` in the repository that holds `root`,
/// at most `limit` of them, oldest first. With `skip_head`, it leaves out a note on `HEAD`, which
/// the current run stands for. Returns none outside a Git repository or without notes.
pub(crate) fn runs(root: &Path, limit: usize, skip_head: bool) -> Vec<Run> {
    if worktree::git(root, &["rev-parse", "--verify", "--quiet", NOTES]).is_err() {
        return Vec::new();
    }
    let skip = if skip_head { "--skip=1" } else { "--skip=0" };
    let notes = format!("--notes={NOTES}");
    let Ok(log) = worktree::git(
        root,
        &[
            "log",
            "--first-parent",
            "-n",
            "1000",
            skip,
            "--date=short",
            &notes,
            "--format=%x1e%H%x09%h%x09%ad%x09%s%n%N",
            "HEAD",
        ],
    ) else {
        return Vec::new();
    };

    let mut runs: Vec<Run> = log
        .split('\u{1e}')
        .filter_map(|record| {
            let (head, note) = record.split_once('\n')?;
            let commit = Commit::parse(head).pop()?;
            let categories = engine::history::parse(note);
            (!categories.is_empty()).then_some(Run {
                categories,
                commit: commit.short,
                date: commit.date,
                title: commit.title,
            })
        })
        .take(limit)
        .collect();
    runs.reverse();
    runs
}

/// Returns an error unless the checkout's tracked files match `HEAD`, so that a note describes the
/// commit it is written on.
///
/// # Errors
///
/// Returns a message for the terminal when the project is not in a Git repository, has no commit,
/// or has changes to tracked files.
pub(crate) fn clean(project: &Project) -> Result<(), Error> {
    worktree::repository(&project.root)?;
    worktree::query(&project.root, &["rev-parse", "--verify", "--quiet", "HEAD"])?
        .ok_or_else(|| Error::from("error: --note needs a commit to write the note on"))?;
    let status = worktree::git(
        &project.root,
        &["status", "--porcelain", "--untracked-files=no"],
    )?;
    if status.is_empty() {
        Ok(())
    } else {
        Err("error: --note needs tracked files that match HEAD, so the note describes that commit. Commit or stash the changes first".into())
    }
}

/// Writes `note` as the note on `HEAD` under `reference`, such as [`NOTES`], replacing any note
/// there, and returns `HEAD`'s short hash.
///
/// # Errors
///
/// Returns a message for the terminal when Git cannot write the note.
pub(crate) fn note(project: &Project, reference: &str, note: &str) -> Result<String, Error> {
    let failed = |error: &dyn std::fmt::Display| {
        Error::from(format!("error: could not write a note: {error}"))
    };
    let mut child = Command::new("git")
        .args([
            "notes", "--ref", reference, "add", "--force", "--file", "-", "HEAD",
        ])
        .current_dir(&project.root)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| failed(&error))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(note.as_bytes())
            .map_err(|error| failed(&error))?;
    }
    let output = child.wait_with_output().map_err(|error| failed(&error))?;
    if !output.status.success() {
        return Err(failed(&String::from_utf8_lossy(&output.stderr).trim()));
    }
    worktree::git(&project.root, &["rev-parse", "--short", "HEAD"])
}

/// Returns a row for each noted run that measured the metric, newest first.
fn noted(project: &Project, category: &Category, selector: &Selector) -> Vec<[String; 5]> {
    let Some(metric) = category.metrics.iter().find(|m| m.name == selector.metric) else {
        return Vec::new();
    };
    runs(&project.root, usize::MAX, false)
        .into_iter()
        .rev()
        .filter_map(|run| {
            let ran = run.category(&category.name, selector.subject.as_deref())?;
            let scored = ran.metric(metric)?;
            let shown = match &selector.item {
                Some(item) => {
                    engine::format::value(scored.items.iter().find(|i| &i.name == item)?.value)
                }
                None => engine::report::headline(&scored),
            };
            Some([run.commit, run.date, shown, String::new(), run.title])
        })
        .collect()
}

/// What one commit measured for the metric.
struct Probe {
    /// Why the commit does not measure the metric, such as `script exited with status 1`.
    reason: String,
    /// Whether the value equals the value at `HEAD`, or is within noise of it for a time.
    same: bool,
    /// The value at the commit, the median for a time, or `None` when it does not measure it.
    value: Option<Value>,
}

impl Probe {
    /// A commit that does not measure the metric, for `reason`.
    fn missing(reason: String) -> Self {
        Probe {
            reason,
            same: false,
            value: None,
        }
    }
}

/// A bisect of one metric: what `HEAD` measures, and how to measure another commit beside it.
struct Bisect<'a> {
    /// The value at `HEAD`, the median of its rounds for a time.
    at_head: Value,
    category: &'a Category,
    /// The categories that share the category's `run` command.
    group: Vec<&'a Category>,
    head: Worktree,
    /// The category's position in `group`.
    index: usize,
    project: &'a Project,
    selector: &'a Selector,
    /// The position of the measured script among the category's scripts.
    unit: usize,
    /// Whether the metric is a time or varies, so each commit runs in pairs beside `HEAD`.
    timed: bool,
}

impl Bisect<'_> {
    /// Measures one commit in its own checkout and prints what it measured. A commit whose
    /// checkout cannot be set up does not measure the metric.
    ///
    /// # Errors
    ///
    /// Returns a message for the terminal when Uphill cannot run the script at all.
    fn probe(&self, commit: &Commit) -> Result<Probe, Error> {
        let (group, index) = (&self.group, self.index);
        let checkout = checkout(self.project, &commit.hash);
        let own = checkout
            .as_ref()
            .map(|checkout| self.own(&checkout.root))
            .unwrap_or_default();
        let own: Vec<&Category> = own.iter().collect();
        let probe = match checkout {
            Err(message) => Probe::missing(
                message
                    .message()
                    .lines()
                    .next()
                    .unwrap_or_default()
                    .to_owned(),
            ),
            Ok(checkout) if self.timed => {
                let (here, there) = measure::pairs(
                    self.project,
                    &self.head.root,
                    &checkout.root,
                    group,
                    &own,
                    false,
                )?;
                match (
                    here.into_iter().nth(index),
                    there.and_then(|there| there.into_iter().nth(index)),
                ) {
                    (Some(Ok(here)), Some(Ok(there))) => {
                        let head = score::rounds(group, index, &here.sources)?;
                        let base = score::rounds(group, index, &there.sources)?;
                        match engine::compare::timings(self.category, &base, &head)
                            .into_iter()
                            .find(|timing| self.selector.times(timing))
                        {
                            Some(timing) => Probe {
                                reason: String::new(),
                                same: timing.direction.is_none(),
                                value: Some(timing.before),
                            },
                            None => Probe::missing("not measured".to_owned()),
                        }
                    }
                    (_, Some(Err(failure))) => Probe::missing(failure.message),
                    _ => Probe::missing("not measured".to_owned()),
                }
            }
            Ok(checkout) => {
                match measure::group(self.project, &checkout.root, &own, false)?
                    .into_iter()
                    .nth(index)
                {
                    Some(Ok(measured)) => {
                        let rounds = score::rounds(group, index, &measured.sources)?;
                        match median(self.selector, &rounds) {
                            Some(value) => Probe {
                                reason: String::new(),
                                same: value == self.at_head,
                                value: Some(value),
                            },
                            None => Probe::missing("not measured".to_owned()),
                        }
                    }
                    Some(Err(failure)) => Probe::missing(failure.message),
                    None => Probe::missing("not measured".to_owned()),
                }
            }
        };

        let shown = probe
            .value
            .map_or_else(|| probe.reason.clone(), engine::format::value);
        let same = if probe.same { "  same as HEAD" } else { "" };
        print(&format!("  {}  {shown}{same}\n", commit.short));
        Ok(probe)
    }

    /// Returns the categories of the group with the `run` command that the configuration in the
    /// checkout at `root` declares for the category's measured script, when it declares it.
    fn own(&self, root: &Path) -> Vec<Category> {
        let run = project::find(root)
            .ok()
            .filter(|found| found.root == root)
            .and_then(|found| {
                found
                    .config
                    .categories
                    .into_iter()
                    .find(|category| category.name == self.category.name)
            })
            .and_then(|category| {
                category
                    .scripts
                    .into_iter()
                    .nth(self.unit)
                    .map(|script| script.run)
            });
        self.group
            .iter()
            .map(|&category| {
                let mut category = category.clone();
                if let Some(run) = &run {
                    for script in &mut category.scripts {
                        script.run.clone_from(run);
                    }
                }
                category
            })
            .collect()
    }
}

/// Bisects the first-parent commits since `since`, or every commit, for the first commit that
/// measures the metric as `HEAD` does. Returns what to print.
///
/// # Errors
///
/// Returns a message for the terminal when Git fails, when a checkout cannot be created, when
/// `setup` fails at `HEAD`, or when `HEAD` does not measure the metric.
fn bisect(
    project: &Project,
    category: &Category,
    selector: &Selector,
    since: Option<&str>,
) -> Result<String, Error> {
    let name = selector.name();
    let range = since.map_or_else(|| "HEAD".to_owned(), |since| format!("{since}..HEAD"));
    let mut commits = Commit::parse(&worktree::git(
        &project.root,
        &[
            "log",
            "--first-parent",
            "--reverse",
            "--date=short",
            FORMAT,
            &range,
        ],
    )?);
    if let Some(since) = since {
        let first = worktree::git(
            &project.root,
            &["log", "-1", "--date=short", FORMAT, since, "--"],
        )?;
        commits.splice(0..0, Commit::parse(&first));
    }

    let units: Vec<Category> = project
        .config
        .categories
        .iter()
        .flat_map(Category::units)
        .collect();
    let all: Vec<&Category> = units.iter().collect();
    let groups = measure::groups(&all);

    print(&format!(
        "Bisecting {name} across {}, measuring each in a temporary checkout\n",
        measure::count(commits.len(), "commit")
    ));

    let head = checkout(project, "HEAD")?;
    // A merged category's metric can come from several scripts, so the first script that
    // measures the selection at `HEAD` is the one to bisect.
    let mut found = None;
    for (unit, own) in category.units().iter().enumerate() {
        if !own
            .metrics
            .iter()
            .any(|metric| metric.name == selector.metric)
        {
            continue;
        }
        let Some((group, index)) = groups.iter().find_map(|group| {
            group
                .iter()
                .position(|c| c.name == own.name && c.scripts == own.scripts)
                .map(|index| (group.clone(), index))
        }) else {
            continue;
        };
        let measured = measure::group(project, &head.root, &group, true)?
            .into_iter()
            .nth(index)
            .ok_or_else(|| format!("error: HEAD does not measure {name}"))?
            .map_err(|failure| format!("error: {name} failed at HEAD: {}", failure.message))?;
        let rounds = score::rounds(&group, index, &measured.sources)?;
        if let Some(value) = median(selector, &rounds) {
            found = Some((group, index, unit, value));
            break;
        }
    }
    let (group, index, unit, at_head) =
        found.ok_or_else(|| format!("error: HEAD does not measure {name}"))?;
    let measuring = group.get(index).copied().unwrap_or(category);
    let varies = category
        .metrics
        .iter()
        .any(|metric| metric.name == selector.metric && metric.varies);
    let bisect = Bisect {
        at_head,
        category: measuring,
        group,
        head,
        index,
        project,
        selector,
        unit,
        timed: at_head.kind() == Kind::Time || (varies && at_head.kind() != Kind::Check),
    };
    let after = engine::format::value(at_head);
    print(&format!("  HEAD     {after}\n"));

    let Some(first) = commits.first() else {
        return Ok(format!("\nNo commits to bisect for {name}\n"));
    };
    let mut before = bisect.probe(first)?;
    if before.same {
        return Ok(format!(
            "\nNothing moved {name} since {}: it measures {after} there too\n",
            first.short
        ));
    }

    // The commit at `low` measures differently from `HEAD`, and the one at `high` the same.
    let (mut low, mut high) = (0, commits.len().saturating_sub(1));
    while high - low > 1 {
        let middle = low + (high - low) / 2;
        let Some(commit) = commits.get(middle) else {
            break;
        };
        let probe = bisect.probe(commit)?;
        if probe.same {
            high = middle;
        } else {
            low = middle;
            before = probe;
        }
    }

    let mut out = String::from("\n");
    if let Some(commit) = commits.get(high) {
        let _ = match before.value {
            Some(value) => writeln!(
                out,
                "{} moved {name} from {} to {after}",
                commit.short,
                engine::format::value(value)
            ),
            None => writeln!(out, "{} first measured {name} at {after}", commit.short),
        };
        let _ = writeln!(out, "  {}  {}", commit.date, commit.title);
    }
    Ok(out)
}

/// Returns the median of the metric's values across rounds, or `None` when no round measures it.
fn median(selector: &Selector, rounds: &[Vec<Line<'_>>]) -> Option<Value> {
    let values: Vec<Value> = rounds
        .iter()
        .filter_map(|lines| lines.iter().find(|line| selector.matches(line)))
        .map(|line| line.value)
        .collect();
    let first = *values.first()?;
    let Some(mut amounts) = values
        .iter()
        .map(|value| value.amount())
        .collect::<Option<Vec<f64>>>()
    else {
        return Some(first);
    };
    amounts.sort_by(f64::total_cmp);
    let middle = *amounts.get(amounts.len() / 2)?;
    Some(Value::of(first.kind(), middle))
}

/// Checks out `reference` in a temporary worktree and runs the configuration's `setup` there.
///
/// # Errors
///
/// Returns a message for the terminal when the worktree cannot be created or `setup` fails.
fn checkout(project: &Project, reference: &str) -> Result<Worktree, Error> {
    let worktree = worktree::add(&project.root, reference)?;
    score::setup(project, &worktree)?;
    Ok(worktree)
}

/// Writes `text` to standard output now, so a long bisect shows its progress.
fn print(text: &str) {
    let mut stdout = io::stdout().lock();
    let _ = stdout.write_all(text.as_bytes());
    let _ = stdout.flush();
}

/// Lays out rows in columns two spaces apart, indented by two spaces, without trailing spaces.
fn table(rows: &[[String; 5]]) -> String {
    let mut widths = [0; 5];
    for row in rows {
        for (width, cell) in widths.iter_mut().zip(row) {
            *width = (*width).max(cell.chars().count());
        }
    }

    let mut out = String::new();
    for row in rows {
        let mut line = String::from(" ");
        for (width, cell) in widths.iter().zip(row) {
            if *width > 0 {
                let _ = write!(line, " {cell:<width$} ");
            }
        }
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out
}
