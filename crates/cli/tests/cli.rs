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

/// Runs `retros` with `args` from `cwd`.
fn retros_in(cwd: &Path, args: &[&str], envs: &[(&str, &Path)]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_retros"))
        .args(args)
        .current_dir(cwd)
        .envs(envs.iter().copied())
        .output()
        .expect("the retros binary runs")
}

/// Runs `retros check` from `cwd`.
fn check(cwd: &Path, envs: &[(&str, &Path)]) -> Output {
    retros_in(cwd, &["check"], envs)
}

/// Copies the fixture project `name` into a new temporary directory.
fn fixture(name: &str) -> TempDir {
    copied(
        &Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name),
    )
}

/// Copies the project `name` from the repository's `examples/` into a new temporary directory.
fn example(name: &str) -> TempDir {
    copied(
        &Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples")
            .join(name),
    )
}

fn copied(project: &Path) -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    copy(project, dir.path());
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
    Usage: retros [category] [--update] [--require-baseline]
           retros check

    Run each category's script, score its metrics, list the opportunities that would raise the
    scores most, and compare sizes, counts, checks, and line targets with retros.lock. Reports go
    to .retros/report.json and .retros/report.md. Name a category to run only that one.

    Commands:
      check  Run each script once and check the metric lines it writes

    Options:
      --update            Record this run's sizes, counts, checks, and line targets in retros.lock
      --require-baseline  Exit with status 1 when anything differs from retros.lock
      -h, --help          Print help
      -V, --version       Print version
    ");
}

#[test]
fn unknown_argument() {
    let output = retros(&["--nope"]);

    assert_eq!(output.status.code(), Some(2));
    insta::assert_snapshot!(stderr(&output), @"
    error: unknown argument '--nope'

    Usage: retros [category] [--update] [--require-baseline]
           retros check

    Run each category's script, score its metrics, list the opportunities that would raise the
    scores most, and compare sizes, counts, checks, and line targets with retros.lock. Reports go
    to .retros/report.json and .retros/report.md. Name a category to run only that one.

    Commands:
      check  Run each script once and check the metric lines it writes

    Options:
      --update            Record this run's sizes, counts, checks, and line targets in retros.lock
      --require-baseline  Exit with status 1 when anything differs from retros.lock
      -h, --help          Print help
      -V, --version       Print version
    ");
}

#[test]
fn unknown_check_argument() {
    let output = retros(&["check", "--fast"]);

    assert_eq!(output.status.code(), Some(2));
    insta::assert_snapshot!(stderr(&output), @"
    error: unknown argument '--fast'

    Usage: retros [category] [--update] [--require-baseline]
           retros check

    Run each category's script, score its metrics, list the opportunities that would raise the
    scores most, and compare sizes, counts, checks, and line targets with retros.lock. Reports go
    to .retros/report.json and .retros/report.md. Name a category to run only that one.

    Commands:
      check  Run each script once and check the metric lines it writes

    Options:
      --update            Record this run's sizes, counts, checks, and line targets in retros.lock
      --require-baseline  Exit with status 1 when anything differs from retros.lock
      -h, --help          Print help
      -V, --version       Print version
    ");
}

#[test]
fn scores_every_category_and_writes_reports() {
    let project = fixture("scored");

    let output = retros_in(project.path(), &[], &[]);

    assert_eq!(output.status.code(), Some(0));
    insta::assert_snapshot!(stdout(&output), @"
    ▲  43  footprint
      ▲   3  bundle       27.5 kB                   target 5 kB
      ■  58  export-size  p90 9 kB (viem/op-stack)  target 5 kB
                          10 kB  viem/chains
                          9 kB   viem/op-stack
                          8 kB   viem/utils
          –  modules      412
      ■  67  links        2 of 3 pass
                          fail  about.html

    ■  75  speed  median of 5 rounds
      ▲  34  render      p90 5 ms (parse)  target 2 ms
                         5 ms  parse
                         2 ms  encode
      ●  96  throughput  2588              target 2000, weight 2

    Opportunities
      +29  footprint  bundle                     27.5 kB → 5 kB
      +19  speed      render parse               5 ms → 2 ms
      +11  footprint  links about.html           fail → pass
      +3   footprint  export-size viem/chains    10 kB → 5 kB
      +3   footprint  export-size viem/op-stack  9 kB → 5 kB

    No retros.lock yet. Run `retros --update` to record these values as the baseline.

    Wrote .retros/report.json, .retros/report.md
    ");

    let json = fs::read_to_string(project.path().join(".retros/report.json")).unwrap();
    insta::assert_snapshot!(json, @r#"
    {
      "version": 1,
      "categories": [
        {
          "name": "footprint",
          "score": 43,
          "rounds": 1,
          "metrics": [
            {
              "name": "bundle",
              "kind": "size",
              "unit": "B",
              "value": 27500,
              "item": null,
              "score": 3,
              "target": 5000,
              "better": "lower",
              "weight": 1,
              "rollup": "p90",
              "items": []
            },
            {
              "name": "export-size",
              "kind": "size",
              "unit": "B",
              "value": 9000,
              "item": "viem/op-stack",
              "score": 58,
              "target": 5000,
              "better": "lower",
              "weight": 1,
              "rollup": "p90",
              "items": [
                {
                  "name": "viem/chains",
                  "value": 10000,
                  "target": null
                },
                {
                  "name": "viem/op-stack",
                  "value": 9000,
                  "target": null
                },
                {
                  "name": "viem/utils",
                  "value": 8000,
                  "target": null
                },
                {
                  "name": "viem/zksync",
                  "value": 7000,
                  "target": null
                },
                {
                  "name": "viem/window",
                  "value": 6000,
                  "target": null
                },
                {
                  "name": "viem/siwe",
                  "value": 5000,
                  "target": null
                },
                {
                  "name": "viem/ens",
                  "value": 4000,
                  "target": null
                },
                {
                  "name": "viem/actions",
                  "value": 3000,
                  "target": null
                },
                {
                  "name": "viem/accounts",
                  "value": 2000,
                  "target": null
                },
                {
                  "name": "viem",
                  "value": 1000,
                  "target": null
                }
              ]
            },
            {
              "name": "modules",
              "kind": "count",
              "unit": null,
              "value": 412,
              "item": null,
              "score": null,
              "target": null,
              "better": "lower",
              "weight": 1,
              "rollup": "p90",
              "items": []
            },
            {
              "name": "links",
              "kind": "check",
              "unit": null,
              "value": 0.6666666666666666,
              "item": null,
              "score": 67,
              "target": null,
              "better": "lower",
              "weight": 1,
              "rollup": null,
              "items": [
                {
                  "name": "about.html",
                  "value": false,
                  "target": null
                },
                {
                  "name": "index.html",
                  "value": true,
                  "target": null
                },
                {
                  "name": "blog/first post.html",
                  "value": true,
                  "target": null
                }
              ]
            }
          ]
        },
        {
          "name": "speed",
          "score": 75,
          "rounds": 5,
          "metrics": [
            {
              "name": "render",
              "kind": "time",
              "unit": "ns",
              "value": 5000000,
              "item": "parse",
              "score": 34,
              "target": 2000000,
              "better": "lower",
              "weight": 1,
              "rollup": "p90",
              "items": [
                {
                  "name": "parse",
                  "value": 5000000,
                  "target": null
                },
                {
                  "name": "encode",
                  "value": 2000000,
                  "target": null
                }
              ]
            },
            {
              "name": "throughput",
              "kind": "count",
              "unit": null,
              "value": 2588,
              "item": null,
              "score": 96,
              "target": 2000,
              "better": "higher",
              "weight": 2,
              "rollup": "p10",
              "items": []
            }
          ]
        }
      ],
      "opportunities": [
        {
          "category": "footprint",
          "metric": "bundle",
          "item": null,
          "items": 1,
          "value": 27500,
          "target": 5000,
          "gain": 29
        },
        {
          "category": "speed",
          "metric": "render",
          "item": "parse",
          "items": 1,
          "value": 5000000,
          "target": 2000000,
          "gain": 19
        },
        {
          "category": "footprint",
          "metric": "links",
          "item": "about.html",
          "items": 1,
          "value": false,
          "target": true,
          "gain": 11
        },
        {
          "category": "footprint",
          "metric": "export-size",
          "item": "viem/chains",
          "items": 1,
          "value": 10000,
          "target": 5000,
          "gain": 3
        },
        {
          "category": "footprint",
          "metric": "export-size",
          "item": "viem/op-stack",
          "items": 1,
          "value": 9000,
          "target": 5000,
          "gain": 3
        }
      ],
      "changes": null
    }
    "#);

    let markdown = fs::read_to_string(project.path().join(".retros/report.md")).unwrap();
    insta::assert_snapshot!(markdown, @"
    # Retros report

    | Category | Score |
    | --- | --- |
    | footprint | ▲ 43 |
    | speed | ■ 75 |

    ## Opportunities

    | Gain | Category | Change | Value | Target |
    | --- | --- | --- | --- | --- |
    | +29 | footprint | bundle | 27.5 kB | 5 kB |
    | +19 | speed | render parse | 5 ms | 2 ms |
    | +11 | footprint | links about.html | fail | pass |
    | +3 | footprint | export-size viem/chains | 10 kB | 5 kB |
    | +3 | footprint | export-size viem/op-stack | 9 kB | 5 kB |

    ## footprint

    | Score | Metric | Value | Target |
    | --- | --- | --- | --- |
    | ▲ 3 | bundle | 27.5 kB | 5 kB |
    | ■ 58 | export-size | p90 9 kB (viem/op-stack) | 5 kB |
    | – | modules | 412 |  |
    | ■ 67 | links | 2 of 3 pass |  |

    Worst items of `export-size`:

    | Item | Value | Target |
    | --- | --- | --- |
    | viem/chains | 10 kB | 5 kB |
    | viem/op-stack | 9 kB | 5 kB |
    | viem/utils | 8 kB | 5 kB |

    Failed items of `links`:

    | Item | Value | Target |
    | --- | --- | --- |
    | about.html | fail |  |

    ## speed

    Timings are the median of 5 rounds.

    | Score | Metric | Value | Target |
    | --- | --- | --- | --- |
    | ▲ 34 | render | p90 5 ms (parse) | 2 ms |
    | ● 96 | throughput | 2588 | 2000 |

    Worst items of `render`:

    | Item | Value | Target |
    | --- | --- | --- |
    | parse | 5 ms | 2 ms |
    | encode | 2 ms | 2 ms |
    ");
}

#[test]
fn scores_one_category_from_a_subdirectory() {
    let project = fixture("scored");
    fs::create_dir(project.path().join("src")).unwrap();

    let output = retros_in(&project.path().join("src"), &["footprint"], &[]);

    assert_eq!(output.status.code(), Some(0));
    insta::assert_snapshot!(stdout(&output), @"
    ▲  43  footprint
      ▲   3  bundle       27.5 kB                   target 5 kB
      ■  58  export-size  p90 9 kB (viem/op-stack)  target 5 kB
                          10 kB  viem/chains
                          9 kB   viem/op-stack
                          8 kB   viem/utils
          –  modules      412
      ■  67  links        2 of 3 pass
                          fail  about.html

    Opportunities
      +29  footprint  bundle                     27.5 kB → 5 kB
      +11  footprint  links about.html           fail → pass
      +3   footprint  export-size viem/chains    10 kB → 5 kB
      +3   footprint  export-size viem/op-stack  9 kB → 5 kB

    No retros.lock yet. Run `retros --update` to record these values as the baseline.

    Wrote ../.retros/report.json, ../.retros/report.md
    ");
}

#[test]
fn scoring_reports_failed_categories() {
    let project = fixture("failing");

    let output = retros_in(project.path(), &[], &[]);

    assert_eq!(output.status.code(), Some(1));
    insta::assert_snapshot!(stdout(&output), @"
        –  footprint
          –  size  1 kB

    ✗ build  script exited with status 3
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

    ✗ crash  script was stopped by signal 9

    No retros.lock yet. Run `retros --update` to record these values as the baseline.

    Wrote .retros/report.json, .retros/report.md
    ");
}

#[test]
fn baselines_record_line_targets_and_gate_every_change() {
    let project = fixture("scored");
    let edit = |file: &str, from: &str, to: &str| {
        let path = project.path().join(file);
        let script = fs::read_to_string(&path).unwrap();
        assert!(script.contains(from), "{file} writes `{from}`");
        fs::write(&path, script.replace(from, to)).unwrap();
    };

    let missing = retros_in(project.path(), &["footprint", "--require-baseline"], &[]);
    assert_eq!(missing.status.code(), Some(1));
    assert!(stdout(&missing).ends_with(
        "\n--require-baseline needs retros.lock. Run `retros --update` to record one\n"
    ));

    // The lock records the targets lines set, including a timing's, whose value it leaves out.
    edit(
        "footprint.sh",
        "export-size viem 1kB\n",
        "export-size viem 1kB target 2kB\n",
    );
    edit(
        "speed.sh",
        "render encode 2ms",
        "render encode 2ms target 3ms",
    );
    let recorded = retros_in(project.path(), &["--update"], &[]);

    assert_eq!(recorded.status.code(), Some(0));
    let lock = fs::read_to_string(project.path().join("retros.lock")).unwrap();
    insta::assert_snapshot!(lock, @r#"
    # The accepted value of every size, count, and check, and the targets scripts set on their
    # lines. `retros --update` writes this file. Commit it, so each run reports what changed since.

    [footprint]
    bundle = "27500B"
    modules = 412

    [footprint.export-size]
    "viem" = "1000B target 2000B"
    "viem/accounts" = "2000B"
    "viem/actions" = "3000B"
    "viem/chains" = "10000B"
    "viem/ens" = "4000B"
    "viem/op-stack" = "9000B"
    "viem/siwe" = "5000B"
    "viem/utils" = "8000B"
    "viem/window" = "6000B"
    "viem/zksync" = "7000B"

    [footprint.links]
    "about.html" = "fail"
    "blog/first post.html" = "pass"
    "index.html" = "pass"

    [speed]
    throughput = 2588

    [speed.render]
    "encode" = "target 3000000ns"
    "#);

    // An improvement alone fails the gate until it is accepted, so it becomes the new floor.
    edit(
        "footprint.sh",
        "links about.html fail",
        "links about.html pass",
    );
    let improved = retros_in(project.path(), &["footprint", "--require-baseline"], &[]);

    assert_eq!(improved.status.code(), Some(1));
    assert!(stdout(&improved).ends_with(
        "
Since retros.lock
  better  footprint  links about.html  fail → pass

Wrote .retros/report.json, .retros/report.md

1 change since retros.lock. Run `retros --update` to accept it
"
    ));
    let accepted = retros_in(project.path(), &["footprint", "--update"], &[]);
    assert_eq!(accepted.status.code(), Some(0));

    edit(
        "footprint.sh",
        "export-size viem/chains 10kB",
        "export-size viem/chains 12kB",
    );
    edit(
        "footprint.sh",
        "export-size viem/window 6kB\n",
        "export-size viem/tempo 4kB\n",
    );
    edit(
        "footprint.sh",
        "export-size viem 1kB target 2kB",
        "export-size viem 1kB target 3kB",
    );
    let gated = retros_in(project.path(), &["footprint", "--require-baseline"], &[]);

    assert_eq!(gated.status.code(), Some(1));
    insta::assert_snapshot!(stdout(&gated), @"
    ■  54  footprint
      ▲   3  bundle       27.5 kB                   target 5 kB
      ■  58  export-size  p90 9 kB (viem/op-stack)  target 5 kB
                          12 kB  viem/chains
                          9 kB   viem/op-stack
                          8 kB   viem/utils
          –  modules      412
      ● 100  links        3 of 3 pass

    Opportunities
      +29  footprint  bundle                     27.5 kB → 5 kB
      +3   footprint  export-size viem/chains    12 kB → 5 kB
      +3   footprint  export-size viem/op-stack  9 kB → 5 kB

    Since retros.lock
      worse   footprint  export-size viem/chains  10 kB → 12 kB
      looser  footprint  export-size viem         target 2 kB → 3 kB
      new     footprint  export-size viem/tempo   4 kB
      gone    footprint  export-size viem/window  6 kB

    Wrote .retros/report.json, .retros/report.md

    4 changes since retros.lock. Run `retros --update` to accept them
    ");
}

#[test]
fn rejects_an_invalid_lock() {
    let project = fixture("scored");
    fs::write(
        project.path().join("retros.lock"),
        "[footprint]\nbundle = \"27.5 kilobytes\"\n",
    )
    .unwrap();

    let output = retros_in(project.path(), &["footprint"], &[]);

    assert_eq!(output.status.code(), Some(2));
    insta::assert_snapshot!(stderr(&output), @r#"
    error[invalid-value] retros.lock:2:10: expected a size, count, or check, optionally followed by `target <value>`. Run `retros --update` to rewrite retros.lock
      2 | bundle = "27.5 kilobytes"
        |          ^
    "#);
}

#[test]
fn unknown_category() {
    let project = fixture("scored");

    let output = retros_in(project.path(), &["bench"], &[]);

    assert_eq!(output.status.code(), Some(2));
    insta::assert_snapshot!(stderr(&output), @"error: unknown category 'bench'. retros.toml declares footprint, speed");
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

    0 passed, 1 failed
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
fn check_passes_every_example() {
    let static_site = check(example("static-site").path(), &[]);
    let node_library = check(example("node-library").path(), &[]);
    let python_api = check(example("python-api").path(), &[]);

    assert_eq!(static_site.status.code(), Some(0));
    insta::assert_snapshot!(stdout(&static_site), @"
    ✓ pages  3 metrics

    1 passed
    ");

    assert_eq!(node_library.status.code(), Some(0));
    insta::assert_snapshot!(stdout(&node_library), @"
    ✓ footprint  1 metric
    ✓ speed      1 metric

    2 passed
    ");

    assert_eq!(python_api.status.code(), Some(0));
    insta::assert_snapshot!(stdout(&python_api), @"
    ✓ api  3 metrics

    1 passed
    ");
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
        "[footprint]\nrun = \"true\"\nsize = { target = \"50kb\" }\n",
    )
    .unwrap();
    fs::create_dir(dir.path().join("src")).unwrap();

    let output = check(&dir.path().join("src"), &[]);

    assert_eq!(output.status.code(), Some(2));
    insta::assert_snapshot!(stderr(&output), @r#"
    error[unknown-unit] ../retros.toml:3:22: unknown unit `kb`, expected one of `B`, `kB`, `MB`, `GB`, `ns`, `µs`, `us`, `ms`, `s`
      3 | size = { target = "50kb" }
        |                      ^
    "#);
}
