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

/// Copies the project `name` from the repository's `examples/` into a new temporary directory,
/// and installs its packages from its lockfile.
fn example(name: &str) -> TempDir {
    let dir = copied(
        &Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples")
            .join(name),
    );
    if dir.path().join("package.json").exists() {
        let install = Command::new("bun")
            .args(["install", "--frozen-lockfile"])
            .current_dir(dir.path())
            .output()
            .expect("bun runs");
        assert!(install.status.success(), "{}", stderr(&install));
    }
    dir
}

fn copied(project: &Path) -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    copy(project, dir.path());
    dir
}

/// Copies a project, leaving out installed packages, build output, and reports from earlier runs.
fn copy(from: &Path, to: &Path) {
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        if [".retros", "node_modules", "target"].contains(&entry.file_name().to_str().unwrap()) {
            continue;
        }
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
    Usage: retros [category] [--update] [--require-baseline] [--require-targets]
           retros check

    Run each category's script, score its metrics, list the opportunities that would raise the
    scores most, and compare sizes, counts, checks, and line targets with each metric's baseline.
    Retros reads retros.yaml or retros.toml, at the root or in .agents, from this directory or the
    nearest parent. Reports go to .retros/report.json, report.md, and report.html. Name a category
    to run only that one.

    Commands:
      check  Run each script once and check the metric lines it writes

    Options:
      --update            Write this run's sizes, counts, checks, and line targets as baselines
      --require-baseline  Exit with status 1 when anything differs from the baselines
      --require-targets   Exit with status 1 when a metric misses its target or a script fails
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

    Usage: retros [category] [--update] [--require-baseline] [--require-targets]
           retros check

    Run each category's script, score its metrics, list the opportunities that would raise the
    scores most, and compare sizes, counts, checks, and line targets with each metric's baseline.
    Retros reads retros.yaml or retros.toml, at the root or in .agents, from this directory or the
    nearest parent. Reports go to .retros/report.json, report.md, and report.html. Name a category
    to run only that one.

    Commands:
      check  Run each script once and check the metric lines it writes

    Options:
      --update            Write this run's sizes, counts, checks, and line targets as baselines
      --require-baseline  Exit with status 1 when anything differs from the baselines
      --require-targets   Exit with status 1 when a metric misses its target or a script fails
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

    Usage: retros [category] [--update] [--require-baseline] [--require-targets]
           retros check

    Run each category's script, score its metrics, list the opportunities that would raise the
    scores most, and compare sizes, counts, checks, and line targets with each metric's baseline.
    Retros reads retros.yaml or retros.toml, at the root or in .agents, from this directory or the
    nearest parent. Reports go to .retros/report.json, report.md, and report.html. Name a category
    to run only that one.

    Commands:
      check  Run each script once and check the metric lines it writes

    Options:
      --update            Write this run's sizes, counts, checks, and line targets as baselines
      --require-baseline  Exit with status 1 when anything differs from the baselines
      --require-targets   Exit with status 1 when a metric misses its target or a script fails
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

    No baseline yet. Run `retros --update` to record these values as the baseline.

    Wrote .retros/report.json, .retros/report.md, .retros/report.html
    ");

    let json = fs::read_to_string(project.path().join(".retros/report.json")).unwrap();
    insta::assert_snapshot!(json, @r#"
    {
      "version": 1,
      "categories": [
        {
          "name": "footprint",
          "title": "Footprint",
          "subject": null,
          "description": null,
          "score": 43,
          "rounds": 1,
          "metrics": [
            {
              "name": "bundle",
              "title": "Bundle",
              "description": null,
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
              "title": "Export size",
              "description": null,
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
              "title": "Modules",
              "description": null,
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
              "title": "Links",
              "description": null,
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
          "title": "Speed",
          "subject": null,
          "description": null,
          "score": 75,
          "rounds": 5,
          "metrics": [
            {
              "name": "render",
              "title": "Render",
              "description": null,
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
              "title": "Throughput",
              "description": null,
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
          "subject": null,
          "metric": "bundle",
          "item": null,
          "items": 1,
          "value": 27500,
          "target": 5000,
          "gain": 29
        },
        {
          "category": "speed",
          "subject": null,
          "metric": "render",
          "item": "parse",
          "items": 1,
          "value": 5000000,
          "target": 2000000,
          "gain": 19
        },
        {
          "category": "footprint",
          "subject": null,
          "metric": "links",
          "item": "about.html",
          "items": 1,
          "value": false,
          "target": true,
          "gain": 11
        },
        {
          "category": "footprint",
          "subject": null,
          "metric": "export-size",
          "item": "viem/chains",
          "items": 1,
          "value": 10000,
          "target": 5000,
          "gain": 3
        },
        {
          "category": "footprint",
          "subject": null,
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
    | Footprint | ▲ 43 |
    | Speed | ■ 75 |

    ## Opportunities

    | Gain | Category | Change | Value | Target |
    | --- | --- | --- | --- | --- |
    | +29 | Footprint | Bundle | 27.5 kB | 5 kB |
    | +19 | Speed | Render `parse` | 5 ms | 2 ms |
    | +11 | Footprint | Links `about.html` | fail | pass |
    | +3 | Footprint | Export size `viem/chains` | 10 kB | 5 kB |
    | +3 | Footprint | Export size `viem/op-stack` | 9 kB | 5 kB |

    ## Footprint

    | Score | Metric | Value | Target |
    | --- | --- | --- | --- |
    | ▲ 3 | Bundle | 27.5 kB | 5 kB |
    | ■ 58 | Export size | p90 9 kB (viem/op-stack) | 5 kB |
    | – | Modules | 412 |  |
    | ■ 67 | Links | 2 of 3 pass |  |

    Worst items of Export size:

    | Item | Value | Target |
    | --- | --- | --- |
    | viem/chains | 10 kB | 5 kB |
    | viem/op-stack | 9 kB | 5 kB |
    | viem/utils | 8 kB | 5 kB |

    Failed items of Links:

    | Item | Value | Target |
    | --- | --- | --- |
    | about.html | fail |  |

    ## Speed

    Values that vary between runs are the median of 5 rounds.

    | Score | Metric | Value | Target |
    | --- | --- | --- | --- |
    | ▲ 34 | Render | p90 5 ms (parse) | 2 ms |
    | ● 96 | Throughput | 2588 | 2000 |

    Worst items of Render:

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

    No baseline yet. Run `retros --update` to record these values as the baseline.

    Wrote ../.retros/report.json, ../.retros/report.md, ../.retros/report.html
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

    No baseline yet. Run `retros --update` to record these values as the baseline.

    Wrote .retros/report.json, .retros/report.md, .retros/report.html
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
    assert!(
        stdout(&missing).ends_with(
            "\n--require-baseline needs a baseline. Run `retros --update` to record one\n"
        )
    );

    // Each metric's baseline records the targets lines set, including a timing's, whose value it
    // leaves out. A baseline with items takes one line per item, so a flow metric becomes a block.
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
    let config = fs::read_to_string(project.path().join("retros.yaml")).unwrap();
    insta::assert_snapshot!(config, @r#"
    # Scores one value, a list, an unscored count, and a list of checks, then timings over 5 rounds
    # and a count where higher is better.

    footprint:
      run: sh footprint.sh
      bundle: { target: 5kB, baseline: 27.5kB }
      export-size:
        target: 5kB
        baseline:
          viem: 1kB target 2kB
          viem/accounts: 2kB
          viem/actions: 3kB
          viem/chains: 10kB
          viem/ens: 4kB
          viem/op-stack: 9kB
          viem/siwe: 5kB
          viem/utils: 8kB
          viem/window: 6kB
          viem/zksync: 7kB
      modules: { baseline: 412 }
      links:
        baseline:
          about.html: fail
          "blog/first post.html": pass
          index.html: pass

    speed:
      run: sh speed.sh
      render:
        target: 2ms
        baseline:
          encode: target 3ms
      throughput: { target: 2000, better: higher, weight: 2, baseline: 2588 }
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
Since the baseline
  better  footprint  links about.html  fail → pass

Wrote .retros/report.json, .retros/report.md, .retros/report.html

1 change since the baseline. Run `retros --update` to accept it
"
    ));
    let accepted = retros_in(project.path(), &["footprint", "--update"], &[]);
    assert_eq!(accepted.status.code(), Some(0));

    // Updating again from the same code leaves the configuration as it is.
    let config = fs::read_to_string(project.path().join("retros.yaml")).unwrap();
    let again = retros_in(project.path(), &["footprint", "--update"], &[]);
    assert_eq!(again.status.code(), Some(0));
    assert_eq!(
        fs::read_to_string(project.path().join("retros.yaml")).unwrap(),
        config
    );

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

    Since the baseline
      worse   footprint  export-size viem/chains  10 kB → 12 kB
      looser  footprint  export-size viem         target 2 kB → 3 kB
      new     footprint  export-size viem/tempo   4 kB
      gone    footprint  export-size viem/window  6 kB

    Wrote .retros/report.json, .retros/report.md, .retros/report.html

    4 changes since the baseline. Run `retros --update` to accept them
    ");

    // Accepting the changes rewrites only the lines of the items that changed.
    let accepted = retros_in(project.path(), &["footprint", "--update"], &[]);
    assert_eq!(accepted.status.code(), Some(0));
    let expected = config
        .replace(
            "      viem: 1kB target 2kB\n",
            "      viem: 1kB target 3kB\n",
        )
        .replace("      viem/chains: 10kB\n", "      viem/chains: 12kB\n")
        .replace(
            "      viem/siwe: 5kB\n",
            "      viem/siwe: 5kB\n      viem/tempo: 4kB\n",
        )
        .replace("      viem/window: 6kB\n", "");
    assert_eq!(
        fs::read_to_string(project.path().join("retros.yaml")).unwrap(),
        expected
    );
}

#[test]
fn subjects_share_one_run_and_score_on_their_own() {
    let project = fixture("subjects");

    let output = retros_in(project.path(), &["--update"], &[]);

    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    // Both categories come from the same 5 runs: the startup times repeat the script.
    let runs = fs::read_to_string(project.path().join("runs.txt")).unwrap();
    assert_eq!(runs.lines().count(), 5);
    insta::assert_snapshot!(stdout(&output), @"
    ● 100  zc: speed  median of 5 rounds
      ● 100  startup  1 ms  target 30 ms

    ● 100  zc: footprint  median of 5 rounds
      ● 100  turn-rss      3 MB  target 16 MB
      ● 100  install-size  2 MB  target 10 MB

    ■  77  ts: speed  median of 5 rounds
      ■  77  startup  40 ms  target 30 ms

    ●  97  ts: footprint  median of 5 rounds
      ●  95  turn-rss      48 MB   target 57 MB
      ● 100  install-size  4 MB    target 10 MB
      ●  97  wasm-size     250 kB  target 350 kB

    Opportunities
      +13  ts: speed  startup  40 ms → 30 ms

    Since the baseline
      looser  ts: footprint  turn-rss      target 16 MB → 57 MB
      new     zc: footprint  install-size
      new     ts: footprint  install-size
      new     ts: footprint  wasm-size

    Wrote .retros/report.json, .retros/report.md, .retros/report.html, retros.yaml
    ");

    // Baselines record each subject. zc's peak memory varies, so only ts's line target is kept.
    let config = fs::read_to_string(project.path().join("retros.yaml")).unwrap();
    insta::assert_snapshot!(config, @r#"
    # Two subjects, zc and ts, measured by one script that both categories share. zc's peak memory
    # varies from round to round, and ts writes a wasm size that zc does not. Footprint and its peak
    # memory have titles, and the other names show in words.
    run: sh harnesses.sh

    speed:
      description: Time the harness spends that is not the model's.
      startup: { target: 30ms, description: "Spawn to exit of `--version`." }

    footprint:
      title: Size and memory
      turn-rss:
        target: 16MB
        varies: true
        title: "Peak memory, one turn"
        baseline:
          ts: target 57MB
      install-size:
        target: 10MB
        baseline:
          zc: 2MB
          ts: 4MB
      wasm-size:
        target: 350kB
        baseline:
          ts: 250kB
    "#);

    let markdown = fs::read_to_string(project.path().join(".retros/report.md")).unwrap();
    insta::assert_snapshot!(markdown, @"
    # Retros report

    | Subject | Speed | Size and memory |
    | --- | --- | --- |
    | zc | ● 100 | ● 100 |
    | ts | ■ 77 | ● 97 |

    ## Opportunities

    | Gain | Category | Change | Value | Target |
    | --- | --- | --- | --- | --- |
    | +13 | ts: Speed | Startup | 40 ms | 30 ms |

    ## Changes since the baseline

    | Change | Category | Metric | Before | After |
    | --- | --- | --- | --- | --- |
    | looser | ts: Size and memory | Peak memory, one turn | target 16 MB | target 57 MB |
    | new | zc: Size and memory | Install size |  |  |
    | new | ts: Size and memory | Install size |  |  |
    | new | ts: Size and memory | Wasm size |  |  |

    ## zc: Speed

    Values that vary between runs are the median of 5 rounds.

    | Score | Metric | Value | Target |
    | --- | --- | --- | --- |
    | ● 100 | Startup | 1 ms | 30 ms |

    ## zc: Size and memory

    Values that vary between runs are the median of 5 rounds.

    | Score | Metric | Value | Target |
    | --- | --- | --- | --- |
    | ● 100 | Peak memory, one turn | 3 MB | 16 MB |
    | ● 100 | Install size | 2 MB | 10 MB |

    ## ts: Speed

    Values that vary between runs are the median of 5 rounds.

    | Score | Metric | Value | Target |
    | --- | --- | --- | --- |
    | ■ 77 | Startup | 40 ms | 30 ms |

    ## ts: Size and memory

    Values that vary between runs are the median of 5 rounds.

    | Score | Metric | Value | Target |
    | --- | --- | --- | --- |
    | ● 95 | Peak memory, one turn | 48 MB | 57 MB |
    | ● 100 | Install size | 4 MB | 10 MB |
    | ● 97 | Wasm size | 250 kB | 350 kB |
    ");

    // The page has a tab per subject, shows titles, and loads nothing from the network.
    let html = fs::read_to_string(project.path().join(".retros/report.html")).unwrap();
    assert_eq!(html.matches("role=\"tab\"").count(), 2);
    assert!(html.contains(">zc</button>") && html.contains(">ts</button>"));
    assert!(html.contains("<h2>Size and memory</h2>"));
    assert!(html.contains("<span class=\"metric-title\">Peak memory, one turn</span>"));
    assert!(!html.contains("http://") && !html.contains("https://"));

    // The next 5 runs write zc's memory as 6 to 10 MB, which is not a change.
    let again = retros_in(project.path(), &["--require-baseline"], &[]);
    assert_eq!(again.status.code(), Some(0), "{}", stdout(&again));
    assert!(stdout(&again).contains("\nNo changes since the baseline\n"));
}

#[test]
fn require_targets_fails_on_each_missed_target_and_failed_script() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("retros.yaml"),
        "quality:\n  run: sh metrics.sh\n  size: { target: 2kB }\n  rate: { target: 2000, better: higher }\n  ok: {}\n",
    )
    .unwrap();
    let script = dir.path().join("metrics.sh");

    // A value equal to its target meets it, in either direction.
    fs::write(
        &script,
        "printf 'size 2kB\\nrate 2000\\nok pass\\n' >> \"$RETROS_METRICS\"\n",
    )
    .unwrap();
    let met = retros_in(dir.path(), &["--require-targets"], &[]);
    assert_eq!(met.status.code(), Some(0), "{}", stdout(&met));
    assert!(!stdout(&met).contains("Missed targets"));

    fs::write(
        &script,
        "printf 'size 2.1kB\\nrate 1999\\nok pass\\n' >> \"$RETROS_METRICS\"\n",
    )
    .unwrap();
    let past = retros_in(dir.path(), &["--require-targets"], &[]);
    assert_eq!(past.status.code(), Some(1));
    assert!(stdout(&past).ends_with(
        "
Missed targets
  quality  size  2.1 kB  target 2 kB
  quality  rate  1999    target 2000

2 targets missed
"
    ));

    // A list misses at the item its roll-up selects, and each failing check item is a miss.
    let scored = fixture("scored");
    let output = retros_in(scored.path(), &["--require-targets"], &[]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stdout(&output).ends_with(
        "
Missed targets
  footprint  bundle                     27.5 kB  target 5 kB
  footprint  export-size viem/op-stack  9 kB     target 5 kB
  footprint  links about.html           fail
  speed      render parse               5 ms     target 2 ms

4 targets missed
"
    ));

    let failing = fixture("failing");
    let output = retros_in(failing.path(), &["--require-targets"], &[]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stdout(&output).ends_with(
        "
Missed targets
  build  script exited with status 3
  crash  script was stopped by signal 9

2 targets missed
"
    ));
}

#[test]
fn rejects_an_invalid_baseline() {
    let project = fixture("scored");
    let path = project.path().join("retros.yaml");
    let config = fs::read_to_string(&path).unwrap();
    fs::write(
        &path,
        config.replace(
            "bundle: { target: 5kB }",
            "bundle: { target: 5kB, baseline: 27.5 kilobytes }",
        ),
    )
    .unwrap();

    let output = retros_in(project.path(), &["footprint"], &[]);

    assert_eq!(output.status.code(), Some(2));
    insta::assert_snapshot!(stderr(&output), @"
    error[invalid-value] retros.yaml:6:36: expected a size, count, or check, optionally followed by `target <value>`, or a mapping of items or subjects to them. Run `retros --update` to rewrite the baseline
      6 |   bundle: { target: 5kB, baseline: 27.5 kilobytes }
        |                                    ^
    ");
}

#[test]
fn update_writes_toml_baselines_in_place() {
    let project = fixture("passing");
    let path = project.path().join("retros.toml");
    fs::write(
        &path,
        "[footprint]\nrun = \"sh metrics/footprint.sh\"\nexport-size = { target = \"5kB\" } # gzipped\nmodules = {}\n\n[speed]\nrun = \"sh metrics/speed.sh\"\n\n[speed.render]\ntarget = \"2ms\" # per item\n",
    )
    .unwrap();
    let speed = project.path().join("metrics/speed.sh");
    let script = fs::read_to_string(&speed).unwrap();
    fs::write(
        &speed,
        script.replace("render encode 236us", "render encode 236us target 300us"),
    )
    .unwrap();

    // Inline tables take the baseline on their line, and a table with a header takes a new line.
    let output = retros_in(project.path(), &["--update"], &[]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let config = fs::read_to_string(&path).unwrap();
    insta::assert_snapshot!(config, @r#"
    [footprint]
    run = "sh metrics/footprint.sh"
    export-size = { target = "5kB", baseline = { viem = "4.2kB", "viem/chains" = "61kB" } } # gzipped
    modules = { baseline = 412 }

    [speed]
    run = "sh metrics/speed.sh"

    [speed.render]
    target = "2ms" # per item
    baseline = { encode = "target 300µs" }
    "#);

    // Without its line target, the timing records nothing, so its baseline goes.
    fs::write(&speed, script).unwrap();
    let output = retros_in(project.path(), &["--update"], &[]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        config.replace("baseline = { encode = \"target 300µs\" }\n", "")
    );

    // A metric defined with dotted keys is left alone, and nothing is written.
    let dotted = config.replace(
        "export-size = { target = \"5kB\", baseline = { viem = \"4.2kB\", \"viem/chains\" = \"61kB\" } }",
        "export-size.target = \"5kB\"",
    );
    fs::write(&path, &dotted).unwrap();
    let output = retros_in(project.path(), &["--update"], &[]);
    assert_eq!(output.status.code(), Some(2));
    insta::assert_snapshot!(stderr(&output), @r#"
    error[invalid-type] retros.toml:3:1: Retros cannot write the baseline of metric `export-size` in this form. Write the metric as an inline table, such as `export-size = { target = "5kB" }`, then run `retros --update` again
      3 | export-size.target = "5kB" # gzipped
        | ^
    "#);
    assert_eq!(fs::read_to_string(&path).unwrap(), dotted);
}

#[test]
fn unknown_category() {
    let project = fixture("scored");

    let output = retros_in(project.path(), &["bench"], &[]);

    assert_eq!(output.status.code(), Some(2));
    insta::assert_snapshot!(stderr(&output), @"error: unknown category 'bench'. retros.yaml declares footprint, speed");
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
        error[metric-missing] retros.yaml:5:3: metric `modules` is configured, but the script did not write it
          5 |   modules: {}
            |   ^
        warning[unconfigured-metric] footprint metrics:2:1: metric `chunks` is written, but `footprint` does not configure it
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
    // Lighthouse, Cargo, and the benchmarks each take a while, so the examples run at once.
    let [typescript_library, web_app, rust_api] = std::thread::scope(|scope| {
        ["typescript-library", "web-app", "rust-api"]
            .map(|name| scope.spawn(move || check(example(name).path(), &[])))
            .map(|handle| handle.join().unwrap())
    });

    assert_eq!(typescript_library.status.code(), Some(0));
    insta::assert_snapshot!(stdout(&typescript_library), @"
    ✓ footprint  6 metrics
    ✓ speed      2 metrics
    ✓ quality    6 metrics

    3 passed
    ");

    assert_eq!(web_app.status.code(), Some(0));
    insta::assert_snapshot!(stdout(&web_app), @"
    ✓ performance     6 metrics
    ✓ accessibility   1 metric
    ✓ best-practices  1 metric
    ✓ seo             1 metric

    4 passed
    ");

    assert_eq!(rust_api.status.code(), Some(0));
    insta::assert_snapshot!(stdout(&rust_api), @"
    ✓ footprint    2 metrics
    ✓ latency      3 metrics
    ✓ reliability  4 metrics
    ✓ quality      4 metrics

    4 passed
    ");
}

#[test]
fn check_without_a_config() {
    let dir = tempfile::tempdir().unwrap();

    let output = check(dir.path(), &[]);

    assert_eq!(output.status.code(), Some(2));
    insta::assert_snapshot!(stderr(&output), @"error: no retros.yaml or retros.toml, at the root or in .agents, in this directory or any parent directory");
}

#[test]
fn check_with_an_invalid_config_names_it_relative_to_the_current_directory() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join(".agents")).unwrap();
    fs::write(
        dir.path().join(".agents/retros.yaml"),
        "footprint:\n  run: \"true\"\n  size: { target: 50kb }\n",
    )
    .unwrap();
    fs::create_dir(dir.path().join("src")).unwrap();

    let output = check(&dir.path().join("src"), &[]);

    assert_eq!(output.status.code(), Some(2));
    insta::assert_snapshot!(stderr(&output), @"
    error[unknown-unit] ../.agents/retros.yaml:3:21: unknown unit `kb`, expected one of `B`, `kB`, `MB`, `GB`, `ns`, `µs`, `us`, `ms`, `s`
      3 |   size: { target: 50kb }
        |                     ^
    ");
}

#[test]
fn agents_config_runs_from_the_project_root_and_records_its_baselines_in_place() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join(".agents/metrics")).unwrap();
    fs::write(
        dir.path().join(".agents/retros.yaml"),
        "footprint:\n  run: sh .agents/metrics/manifests.sh\n  manifests: { target: 2 }\n",
    )
    .unwrap();
    // The script counts manifests in the directory it runs from, which is the project root.
    fs::write(
        dir.path().join(".agents/metrics/manifests.sh"),
        "echo \"manifests $(ls *.json | wc -l)\" >> \"$RETROS_METRICS\"\n",
    )
    .unwrap();
    fs::write(dir.path().join("package.json"), "{}").unwrap();

    // Started inside `.agents`, Retros still runs from the directory above it.
    let output = retros_in(&dir.path().join(".agents"), &["--update"], &[]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    insta::assert_snapshot!(stdout(&output), @"
    ●  99  footprint
      ●  99  manifests  1  target 2

    Since the baseline
      new  footprint  manifests

    Wrote ../.retros/report.json, ../.retros/report.md, ../.retros/report.html, ../.agents/retros.yaml
    ");
    insta::assert_snapshot!(fs::read_to_string(dir.path().join(".agents/retros.yaml")).unwrap(), @"
    footprint:
      run: sh .agents/metrics/manifests.sh
      manifests: { target: 2, baseline: 1 }
    ");
    assert!(dir.path().join(".retros/report.html").exists());

    fs::write(
        dir.path().join("retros.yaml"),
        "speed:\n  run: \"true\"\n  m: {}\n",
    )
    .unwrap();
    let output = check(dir.path(), &[]);
    assert_eq!(output.status.code(), Some(2));
    insta::assert_snapshot!(stderr(&output), @"error: retros.yaml and .agents/retros.yaml both exist. Keep one");
}
