//! Runs the `retros` binary as a consumer does and checks its output and exit codes. Project
//! tests copy a fixture from `tests/fixtures/` into a temporary directory and run real scripts.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};
use std::time::{Duration, Instant};

use tempfile::TempDir;

fn retros(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_retros"))
        .args(args)
        .output()
        .expect("the retros binary runs")
}

/// Runs `retros check` from `cwd`.
fn check(cwd: &Path, envs: &[(&str, &Path)]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_retros"))
        .arg("check")
        .current_dir(cwd)
        .envs(envs.iter().copied())
        .output()
        .expect("the retros binary runs")
}

/// Copies the fixture project `name` into a new temporary directory.
fn fixture(name: &str) -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    copy(
        &Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name),
        dir.path(),
    );
    dir
}

fn copy(from: &Path, to: &Path) {
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());

        if entry.file_type().unwrap().is_dir() {
            fs::create_dir(&target).unwrap();
            copy(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn version() {
    let output = retros(&["--version"]);

    assert!(output.status.success());
    insta::assert_snapshot!(stdout(&output), @"retros 0.0.0");
}

#[test]
fn help() {
    let output = retros(&["--help"]);

    assert!(output.status.success());
    insta::assert_snapshot!(stdout(&output), @"
    Usage: retros <command>

    Commands:
      check  Run each script once and check the metric lines it writes

    Options:
      -h, --help     Print help
      -V, --version  Print version
    ");
}

#[test]
fn unknown_argument() {
    let output = retros(&["--nope"]);

    assert_eq!(output.status.code(), Some(2));
    insta::assert_snapshot!(stderr(&output), @"
    error: unknown argument '--nope'

    Usage: retros <command>

    Commands:
      check  Run each script once and check the metric lines it writes

    Options:
      -h, --help     Print help
      -V, --version  Print version
    ");
}

#[test]
fn unknown_command() {
    let output = retros(&["bench"]);

    assert_eq!(output.status.code(), Some(2));
    insta::assert_snapshot!(stderr(&output), @"
    error: unknown command 'bench'

    Usage: retros <command>

    Commands:
      check  Run each script once and check the metric lines it writes

    Options:
      -h, --help     Print help
      -V, --version  Print version
    ");
}

#[test]
fn unknown_check_argument() {
    let output = retros(&["check", "--fast"]);

    assert_eq!(output.status.code(), Some(2));
    insta::assert_snapshot!(stderr(&output), @"
    error: unknown argument '--fast'

    Usage: retros <command>

    Commands:
      check  Run each script once and check the metric lines it writes

    Options:
      -h, --help     Print help
      -V, --version  Print version
    ");
}

#[test]
fn no_arguments() {
    let output = retros(&[]);

    assert_eq!(output.status.code(), Some(2));
    insta::assert_snapshot!(stderr(&output), @"
    error: expected a command

    Usage: retros <command>

    Commands:
      check  Run each script once and check the metric lines it writes

    Options:
      -h, --help     Print help
      -V, --version  Print version
    ");
}

#[test]
fn check_passes_from_a_subdirectory() {
    let project = fixture("passing");

    let output = check(&project.path().join("metrics"), &[]);

    assert_eq!(output.status.code(), Some(0));
    insta::assert_snapshot!(stdout(&output), @"
    ✓ footprint  2 metrics
    ✓ speed      1 metric

    2 passed
    ");
    insta::assert_snapshot!(stderr(&output), @"");
}

#[test]
fn check_reports_failed_scripts_with_the_end_of_stderr() {
    let project = fixture("failing");

    let output = check(project.path(), &[]);

    assert_eq!(output.status.code(), Some(1));
    insta::assert_snapshot!(stdout(&output), @"
    ✓ footprint  1 metric
    ✗ build      script exited with status 3
        build log line 6
        build log line 7
        build log line 8
        build log line 9
        build log line 10
        build log line 11
        build log line 12
        build log line 13
        build log line 14
        build log line 15
        build log line 16
        build log line 17
        build log line 18
        build log line 19
        build log line 20
        build log line 21
        build log line 22
        build log line 23
        build log line 24
        build log line 25
    ✗ crash      script was stopped by signal 9

    1 passed, 2 failed
    ");
}

#[test]
fn check_reports_a_malformed_line() {
    let project = fixture("malformed");

    let output = check(project.path(), &[]);

    assert_eq!(output.status.code(), Some(1));
    insta::assert_snapshot!(stdout(&output), @r#"
    ✗ footprint  invalid metric line
        error[unknown-unit] footprint metrics:2:29: unknown unit `kb`, expected one of `B`, `kB`, `MB`, `GB`, `ns`, `µs`, `us`, `ms`, `s`
          2 | export-size "viem/chains" 61kb
            |                             ^

    0 passed, 1 failed
    "#);
}

#[test]
fn check_reports_missing_and_unconfigured_metrics() {
    let project = fixture("missing");

    let output = check(project.path(), &[]);

    assert_eq!(output.status.code(), Some(1));
    insta::assert_snapshot!(stdout(&output), @"
    ✗ footprint  2 metrics, 1 error, 1 warning
      error[metric-missing] retros.toml:5:1: metric `modules` is configured, but the script did not write it
        5 | modules = {}
          | ^
      warning[unconfigured-metric] footprint metrics:2:1: metric `chunks` is written, but `[footprint]` does not configure it
        2 | chunks 9
          | ^

    0 passed, 1 failed, 1 warning
    ");
}

#[test]
fn check_kills_a_script_that_times_out() {
    let project = fixture("timeout");
    let pid_file = project.path().join("pid");
    let started = Instant::now();

    let output = check(project.path(), &[("RETROS_TEST_PID", &pid_file)]);

    assert_eq!(output.status.code(), Some(1));
    assert!(started.elapsed() < Duration::from_secs(10));
    insta::assert_snapshot!(stdout(&output), @"
    ✗ speed  script timed out after 300ms

    0 passed, 1 failed
    ");

    let pid = fs::read_to_string(&pid_file).unwrap();
    let pid = pid.trim();
    let deadline = Instant::now() + Duration::from_secs(5);
    let alive = loop {
        // `ps` prints nothing for a process that is gone, and `Z` for one that awaits its parent.
        let ps = Command::new("ps")
            .args(["-o", "stat=", "-p", pid])
            .output()
            .unwrap();
        let state = String::from_utf8_lossy(&ps.stdout).trim().to_owned();
        if state.is_empty() || state.starts_with('Z') || Instant::now() > deadline {
            break !(state.is_empty() || state.starts_with('Z'));
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    assert!(
        !alive,
        "the script's background process {pid} is still running"
    );
}

#[test]
fn check_without_a_config() {
    let dir = tempfile::tempdir().unwrap();

    let output = check(dir.path(), &[]);

    assert_eq!(output.status.code(), Some(2));
    insta::assert_snapshot!(stderr(&output), @"error: no retros.toml in this directory or any parent directory");
}

#[test]
fn check_with_an_invalid_config_names_it_relative_to_the_current_directory() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("retros.toml"),
        "[footprint]\nrun = \"true\"\nsize = { target = \"5kB\", limit = \"50kb\" }\n",
    )
    .unwrap();
    fs::create_dir(dir.path().join("src")).unwrap();

    let output = check(&dir.path().join("src"), &[]);

    assert_eq!(output.status.code(), Some(2));
    insta::assert_snapshot!(stderr(&output), @r#"
    error[unknown-unit] ../retros.toml:3:37: unknown unit `kb`, expected one of `B`, `kB`, `MB`, `GB`, `ns`, `µs`, `us`, `ms`, `s`
      3 | size = { target = "5kB", limit = "50kb" }
        |                                     ^
    "#);
}
