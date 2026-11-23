//! Runs one measurement script.
//!
//! A script runs through `sh -c` from the project root, in its own process group, with a fresh
//! empty file at `$UPHILL_METRICS`. Its standard output and standard error go to files rather
//! than pipes, so a background process that keeps them open cannot stall the run. When the
//! script exits or times out, Uphill kills its process group, which also stops processes the
//! script left running.

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use rustix::process::{Pid, Signal, kill_process_group};

/// How often Uphill checks whether a script has exited. Checks start fast, so a short script is
/// not timed at the polling interval, and slow down to the longest interval.
const FIRST_POLL: Duration = Duration::from_micros(50);
const LONGEST_POLL: Duration = Duration::from_millis(10);

/// The most standard error that a failed run keeps, in lines and in bytes.
const TAIL_LINES: usize = 20;
const TAIL_BYTES: u64 = 64 * 1024;

/// How a script run ended.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Outcome {
    /// The script exited with this status.
    Exited(i32),
    /// A signal stopped the script.
    Signaled(i32),
    /// The script ran past its timeout, and Uphill killed it.
    TimedOut,
}

/// The result of one script run.
pub(crate) struct Run {
    /// The bytes the script wrote to `$UPHILL_METRICS`.
    pub(crate) metrics: Vec<u8>,
    /// How the run ended.
    pub(crate) outcome: Outcome,
    /// The last lines of the script's standard error.
    pub(crate) stderr: String,
}

/// Runs `command` from `root` and stops it after `timeout`.
///
/// # Errors
///
/// Returns an error when Uphill cannot create the run's files, start `sh`, or read the files
/// back. A script that fails is not an error; its outcome says how it ended.
pub(crate) fn run(command: &str, root: &Path, timeout: Duration) -> io::Result<Run> {
    let dir = tempfile::tempdir()?;
    let metrics = dir.path().join("metrics");
    let stderr = dir.path().join("stderr");

    File::create(&metrics)?;

    let mut child = Command::new("sh")
        .arg("-c")
        .arg(command)
        .current_dir(root)
        .env("UPHILL_METRICS", &metrics)
        .stdin(Stdio::null())
        .stdout(File::create(dir.path().join("stdout"))?)
        .stderr(File::create(&stderr)?)
        .process_group(0)
        .spawn()?;
    let group = Pid::from_child(&child);
    let started = Instant::now();
    let mut poll = FIRST_POLL;

    let outcome = loop {
        if let Some(status) = child.try_wait()? {
            break match (status.code(), status.signal()) {
                (Some(code), _) => Outcome::Exited(code),
                (None, signal) => Outcome::Signaled(signal.unwrap_or_default()),
            };
        }

        if started.elapsed() >= timeout {
            let _ = kill_process_group(group, Signal::KILL);
            child.wait()?;
            break Outcome::TimedOut;
        }

        thread::sleep(poll);
        poll = (poll * 2).min(LONGEST_POLL);
    };

    // Stop processes the script started and left running. The group is usually empty by now.
    let _ = kill_process_group(group, Signal::KILL);

    Ok(Run {
        metrics: std::fs::read(&metrics)?,
        outcome,
        stderr: tail(&stderr)?,
    })
}

/// Returns the last lines of a file, reading at most its last 64 KiB.
fn tail(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let length = file.metadata()?.len();
    let start = length.saturating_sub(TAIL_BYTES);
    let mut bytes = Vec::new();

    file.seek(SeekFrom::Start(start))?;
    file.read_to_end(&mut bytes)?;

    let text = String::from_utf8_lossy(&bytes);
    let mut lines: Vec<&str> = text.lines().collect();

    // A read that starts inside the file may start inside a line.
    if start > 0 && !lines.is_empty() {
        lines.remove(0);
    }

    let keep = lines.len().saturating_sub(TAIL_LINES);
    Ok(lines.get(keep..).unwrap_or_default().join("\n"))
}
