//! Runs the `uphill` binary as a consumer does and checks its output and exit codes. Project
//! tests copy a fixture from `tests/fixtures/` into a temporary directory and run real scripts.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};
use std::time::{Duration, Instant};

use tempfile::TempDir;

fn uphill(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_uphill"))
        .args(args)
        .output()
        .expect("the uphill binary runs")
}

/// Runs `uphill` with `args` from `cwd`.
fn uphill_in(cwd: &Path, args: &[&str], envs: &[(&str, &Path)]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_uphill"))
        .args(args)
        .current_dir(cwd)
        .envs(envs.iter().copied())
        .output()
        .expect("the uphill binary runs")
}

/// Runs `uphill check` from `cwd`.
fn check(cwd: &Path, envs: &[(&str, &Path)]) -> Output {
    uphill_in(cwd, &["check"], envs)
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
        if [".uphill", "node_modules", "target"].contains(&entry.file_name().to_str().unwrap()) {
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

/// Copies the fixture project `name` into a new Git repository with one commit.
fn repository(name: &str) -> TempDir {
    let dir = fixture(name);
    git(dir.path(), &["init", "-q", "-b", "main"]);
    commit(dir.path());
    dir
}

/// Commits every change in the repository at `dir`.
fn commit(dir: &Path) {
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", "change"]);
}

/// Runs Git in `dir` with a fixed identity and returns its standard output.
fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args([
            "-c",
            "user.name=Uphill",
            "-c",
            "user.email=uphill@example.com",
        ])
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git runs");
    assert!(output.status.success(), "{}", stderr(&output));
    stdout(&output)
}

/// Runs `uphill --compare HEAD` in `dir`, and returns its output with the commit hash masked.
fn compare_head(dir: &Path, args: &[&str]) -> (Output, String) {
    let output = uphill_in(dir, &[&["--compare", "HEAD"], args].concat(), &[]);
    let text = masked(dir, &stdout(&output));
    (output, text)
}

/// Replaces the short hash of the commit at `HEAD` in `text` with `abc1234`.
fn masked(dir: &Path, text: &str) -> String {
    let short = git(dir, &["rev-parse", "--short", "HEAD"]);
    text.replace(short.trim(), "abc1234")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn version() {
    let long = uphill(&["--version"]);
    let short = uphill(&["-V"]);

    // `pnpm changeset:version` sets the version on each release.
    assert!(long.status.success());
    insta::assert_snapshot!(
        stdout(&long).replace(env!("CARGO_PKG_VERSION"), "[version]"),
        @"[version]"
    );
    assert_eq!(stdout(&short), stdout(&long));
}

#[test]
fn help() {
    let output = uphill(&["--help"]);

    assert!(output.status.success());
    insta::assert_snapshot!(stdout(&output), @"
    uphill@0.0.3 — Score metrics against their targets and compare them with their baselines

    Usage: uphill [category] [options] | <command>

    Arguments:
      category  Run only this category

    Options:
      --compact           Print the scores, missed targets, and opportunities in a few lines
      --compare <string>  Compare with another revision: timings in interleaved pairs, and sizes, counts, and checks with its baselines
      --note              Keep this run as a Git note on HEAD, for trends and `uphill log`
      --require-baseline  Exit with status 1 when anything differs from the baselines
      --require-targets   Exit with status 1 when a metric misses its target or a script fails
      --update            Write this run's sizes, counts, checks, and line targets to the baseline file

    Runs each category's script, scores its metrics, lists the opportunities that would raise the scores most, and compares sizes, counts, checks, and line targets with each metric's baseline. Reads uphill.yaml or uphill.toml, at the root or in .agents, from this directory or the nearest parent. Writes .uphill/report.json, report.md, and report.html.

    Commands:
      add        Copy a pack's categories into uphill.yaml and its scripts into the project: a local directory, owner/name/path on GitHub, or a Git URL followed by //path, each with an optional @ref. With --update, merge the changes of each added category's pack
      blame      Name the commit that last moved a metric: from its baselines, or by measuring commits since <ref>, or every commit, in temporary checkouts and bisecting
      check      Run each script once and check the metric lines it writes
      dashboard  Write .uphill/dashboard.html from the latest noted runs of each repository: a Git URL, a local path, or owner/name on GitHub
      init       Print a brief for an agent to set up Uphill in this project
      log        List the commits that changed a metric's baseline, or the runs noted for it

    Integrations:
      completions  Generate shell completion script
      skills       Sync skill files to agents (add, list)

    Global Options:
      --filter-output <keys>              Filter output by key paths (e.g. foo,bar.baz,a[0,3])
      --format <toon|json|yaml|md|jsonl>  Output format
      --full-output                       Show full output envelope
      --help                              Show help
      --llms, --llms-full                 Print LLM-readable manifest
      --schema                            Show JSON Schema for command
      --token-count                       Print token count of output (instead of output)
      --token-limit <n>                   Limit output to n tokens
      --token-offset <n>                  Skip first n tokens of output
      --version                           Show version
    ");
}

#[test]
fn unknown_argument() {
    let output = uphill(&["--nope"]);

    assert_eq!(output.status.code(), Some(2));
    insta::assert_snapshot!(stderr(&output), @r#"
    code: UNKNOWN
    message: "Unknown flag: --nope"
    "#);
}

#[test]
fn unknown_check_argument() {
    let output = uphill(&["check", "--fast"]);

    assert_eq!(output.status.code(), Some(2));
    insta::assert_snapshot!(stderr(&output), @r#"
    code: UNKNOWN
    message: "Unknown flag: --fast"
    "#);
}

#[test]
fn scores_every_category_and_writes_reports() {
    let project = fixture("scored");

    let output = uphill_in(project.path(), &[], &[]);

    assert_eq!(output.status.code(), Some(0));
    insta::assert_snapshot!(stdout(&output), @"
    ▲  42  footprint
      ▲   3  bundle       27.5 kB                   target 5 kB
                          includes all of lodash
      ■  57  export-size  p90 9 kB (viem/op-stack)  target 5 kB
                          10 kB  viem/chains
                                 bundles every chain definition
                          9 kB   viem/op-stack
                          8 kB   viem/utils
          –  modules      412
      ■  67  links        2 of 3 pass
                          fail  about.html
                                links to /team, which is gone

    ■  75  speed  median of 5 rounds
      ▲  33  render      p90 5 ms (parse)  target 2 ms
                         5 ms  parse
                         2 ms  encode
      ●  96  throughput  2588              target 2000, weight 2

    Opportunities
      +29  footprint  bundle                     27.5 kB → 5 kB
      +19  speed      render parse               5 ms → 2 ms
      +11  footprint  links about.html           fail → pass
      +3   footprint  export-size viem/chains    10 kB → 5 kB
      +3   footprint  export-size viem/op-stack  9 kB → 5 kB

    No baseline yet. Run `uphill --update` to record these values as the baseline.

    Wrote .uphill/report.json, .uphill/report.md, .uphill/report.html
    ");

    let json = fs::read_to_string(project.path().join(".uphill/report.json")).unwrap();
    insta::assert_snapshot!(json, @r#"
    {
      "version": 1,
      "categories": [
        {
          "name": "footprint",
          "title": "Footprint",
          "subject": null,
          "description": null,
          "score": 42,
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
              "default": 5000,
              "note": "includes all of lodash",
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
              "score": 57,
              "target": 5000,
              "default": 5000,
              "note": null,
              "better": "lower",
              "weight": 1,
              "rollup": "p90",
              "items": [
                {
                  "name": "viem/chains",
                  "value": 10000,
                  "target": null,
                  "note": "bundles every chain definition"
                },
                {
                  "name": "viem/op-stack",
                  "value": 9000,
                  "target": null,
                  "note": null
                },
                {
                  "name": "viem/utils",
                  "value": 8000,
                  "target": null,
                  "note": null
                },
                {
                  "name": "viem/zksync",
                  "value": 7000,
                  "target": null,
                  "note": null
                },
                {
                  "name": "viem/window",
                  "value": 6000,
                  "target": null,
                  "note": null
                },
                {
                  "name": "viem/siwe",
                  "value": 5000,
                  "target": null,
                  "note": null
                },
                {
                  "name": "viem/ens",
                  "value": 4000,
                  "target": null,
                  "note": null
                },
                {
                  "name": "viem/actions",
                  "value": 3000,
                  "target": null,
                  "note": null
                },
                {
                  "name": "viem/accounts",
                  "value": 2000,
                  "target": null,
                  "note": null
                },
                {
                  "name": "viem",
                  "value": 1000,
                  "target": null,
                  "note": null
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
              "default": null,
              "note": null,
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
              "default": null,
              "note": null,
              "better": "lower",
              "weight": 1,
              "rollup": null,
              "items": [
                {
                  "name": "about.html",
                  "value": false,
                  "target": null,
                  "note": "links to /team, which is gone"
                },
                {
                  "name": "index.html",
                  "value": true,
                  "target": null,
                  "note": null
                },
                {
                  "name": "blog/first post.html",
                  "value": true,
                  "target": null,
                  "note": null
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
              "score": 33,
              "target": 2000000,
              "default": 2000000,
              "note": null,
              "better": "lower",
              "weight": 1,
              "rollup": "p90",
              "items": [
                {
                  "name": "parse",
                  "value": 5000000,
                  "target": null,
                  "note": null
                },
                {
                  "name": "encode",
                  "value": 2000000,
                  "target": null,
                  "note": "reuses one buffer"
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
              "default": 2000,
              "note": null,
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
      "changes": null,
      "comparison": null
    }
    "#);

    let markdown = fs::read_to_string(project.path().join(".uphill/report.md")).unwrap();
    insta::assert_snapshot!(markdown, @"
    # Hill climb report

    | Category | Score |
    | --- | --- |
    | Footprint | ▲ 42 |
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

    | Score | Metric | Value | Target | Note |
    | --- | --- | --- | --- | --- |
    | ▲ 3 | Bundle | 27.5 kB | 5 kB | includes all of lodash |
    | ■ 57 | Export size | p90 9 kB (viem/op-stack) | 5 kB |  |
    | – | Modules | 412 |  |  |
    | ■ 67 | Links | 2 of 3 pass |  |  |

    Worst items of Export size:

    | Item | Value | Target | Note |
    | --- | --- | --- | --- |
    | viem/chains | 10 kB | 5 kB | bundles every chain definition |
    | viem/op-stack | 9 kB | 5 kB |  |
    | viem/utils | 8 kB | 5 kB |  |

    Failed items of Links:

    | Item | Value | Target | Note |
    | --- | --- | --- | --- |
    | about.html | fail |  | links to /team, which is gone |

    ## Speed

    Values that vary between runs are the median of 5 rounds.

    | Score | Metric | Value | Target |
    | --- | --- | --- | --- |
    | ▲ 33 | Render | p90 5 ms (parse) | 2 ms |
    | ● 96 | Throughput | 2588 | 2000 |

    Worst items of Render:

    | Item | Value | Target | Note |
    | --- | --- | --- | --- |
    | parse | 5 ms | 2 ms |  |
    | encode | 2 ms | 2 ms | reuses one buffer |
    ");
}

#[test]
fn scores_one_category_from_a_subdirectory() {
    let project = fixture("scored");
    fs::create_dir(project.path().join("src")).unwrap();

    let output = uphill_in(&project.path().join("src"), &["footprint"], &[]);

    assert_eq!(output.status.code(), Some(0));
    insta::assert_snapshot!(stdout(&output), @"
    ▲  42  footprint
      ▲   3  bundle       27.5 kB                   target 5 kB
                          includes all of lodash
      ■  57  export-size  p90 9 kB (viem/op-stack)  target 5 kB
                          10 kB  viem/chains
                                 bundles every chain definition
                          9 kB   viem/op-stack
                          8 kB   viem/utils
          –  modules      412
      ■  67  links        2 of 3 pass
                          fail  about.html
                                links to /team, which is gone

    Opportunities
      +29  footprint  bundle                     27.5 kB → 5 kB
      +11  footprint  links about.html           fail → pass
      +3   footprint  export-size viem/chains    10 kB → 5 kB
      +3   footprint  export-size viem/op-stack  9 kB → 5 kB

    No baseline yet. Run `uphill --update` to record these values as the baseline.

    Wrote ../.uphill/report.json, ../.uphill/report.md, ../.uphill/report.html
    ");
}

#[test]
fn scoring_reports_failed_categories() {
    let project = fixture("failing");

    let output = uphill_in(project.path(), &[], &[]);

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

    No baseline yet. Run `uphill --update` to record these values as the baseline.

    Wrote .uphill/report.json, .uphill/report.md, .uphill/report.html
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

    let missing = uphill_in(project.path(), &["footprint", "--require-baseline"], &[]);
    assert_eq!(missing.status.code(), Some(1));
    assert!(
        stdout(&missing).ends_with(
            "\n--require-baseline needs a baseline. Run `uphill --update` to record one\n"
        )
    );

    // Each metric's baseline records the targets lines set, including a timing's, whose value it
    // leaves out. A baseline with items takes one line per item.
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
    let config = fs::read_to_string(project.path().join("uphill.yaml")).unwrap();
    let recorded = uphill_in(project.path(), &["--update"], &[]);

    // The configuration stays as people wrote it, and the baseline file keeps the declared order.
    assert_eq!(recorded.status.code(), Some(0));
    assert_eq!(
        fs::read_to_string(project.path().join("uphill.yaml")).unwrap(),
        config
    );
    let baselines = fs::read_to_string(project.path().join("uphill.baseline.yaml")).unwrap();
    insta::assert_snapshot!(baselines, @r#"
    # Written by `uphill --update`: the accepted sizes, counts, checks, and line targets of each
    # metric. Each run reports what changed since.

    footprint:
      bundle: 27.5kB
      export-size:
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
      modules: 412
      links:
        about.html: fail
        "blog/first post.html": pass
        index.html: pass

    speed:
      render:
        encode: target 3ms
      throughput: 2588
    "#);

    // An improvement alone fails the gate until it is accepted, so it becomes the new floor.
    edit(
        "footprint.sh",
        "links about.html fail",
        "links about.html pass",
    );
    let improved = uphill_in(project.path(), &["footprint", "--require-baseline"], &[]);

    assert_eq!(improved.status.code(), Some(1));
    assert!(stdout(&improved).ends_with(
        "
Since the baseline
  better  footprint  links about.html  fail → pass

Wrote .uphill/report.json, .uphill/report.md, .uphill/report.html

1 change since the baseline. Run `uphill --update` to accept it
"
    ));
    let accepted = uphill_in(project.path(), &["footprint", "--update"], &[]);
    assert_eq!(accepted.status.code(), Some(0));

    // Updating again from the same code leaves the baseline file as it is.
    let baselines = fs::read_to_string(project.path().join("uphill.baseline.yaml")).unwrap();
    let again = uphill_in(project.path(), &["footprint", "--update"], &[]);
    assert_eq!(again.status.code(), Some(0));
    assert_eq!(
        fs::read_to_string(project.path().join("uphill.baseline.yaml")).unwrap(),
        baselines
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
    let gated = uphill_in(project.path(), &["footprint", "--require-baseline"], &[]);

    assert_eq!(gated.status.code(), Some(1));
    insta::assert_snapshot!(stdout(&gated), @"
    ■  53  footprint
      ▲   3  bundle       27.5 kB                   target 5 kB
                          includes all of lodash
      ■  57  export-size  p90 9 kB (viem/op-stack)  target 5 kB
                          12 kB  viem/chains
                                 bundles every chain definition
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

    Wrote .uphill/report.json, .uphill/report.md, .uphill/report.html

    4 changes since the baseline. Run `uphill --update` to accept them
    ");

    // Accepting the changes rewrites only the lines of the items that changed.
    let accepted = uphill_in(project.path(), &["footprint", "--update"], &[]);
    assert_eq!(accepted.status.code(), Some(0));
    let expected = baselines
        .replace("    viem: 1kB target 2kB\n", "    viem: 1kB target 3kB\n")
        .replace("    viem/chains: 10kB\n", "    viem/chains: 12kB\n")
        .replace(
            "    viem/siwe: 5kB\n",
            "    viem/siwe: 5kB\n    viem/tempo: 4kB\n",
        )
        .replace("    viem/window: 6kB\n", "");
    assert_eq!(
        fs::read_to_string(project.path().join("uphill.baseline.yaml")).unwrap(),
        expected
    );
}

#[test]
fn subjects_share_one_run_and_score_on_their_own() {
    let project = fixture("subjects");

    let output = uphill_in(project.path(), &["--update"], &[]);

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
      ●  94  turn-rss      48 MB   target 57 MB
      ● 100  install-size  4 MB    target 10 MB
      ●  97  wasm-size     250 kB  target 350 kB

    Opportunities
      +13  ts: speed  startup  40 ms → 30 ms

    Since the baseline
      looser  ts: footprint  turn-rss      target 16 MB → 57 MB
      new     zc: footprint  install-size
      new     ts: footprint  install-size
      new     ts: footprint  wasm-size

    Wrote .uphill/report.json, .uphill/report.md, .uphill/report.html, uphill.baseline.yaml
    ");

    // Baselines record each subject. zc's peak memory varies, so only ts's line target is kept.
    let baselines = fs::read_to_string(project.path().join("uphill.baseline.yaml")).unwrap();
    insta::assert_snapshot!(baselines, @"
    # Written by `uphill --update`: the accepted sizes, counts, checks, and line targets of each
    # metric. Each run reports what changed since.

    footprint:
      turn-rss:
        ts: target 57MB
      install-size:
        zc: 2MB
        ts: 4MB
      wasm-size:
        ts: 250kB
    ");

    let markdown = fs::read_to_string(project.path().join(".uphill/report.md")).unwrap();
    insta::assert_snapshot!(markdown, @"
    # Hill climb report

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
    | ● 94 | Peak memory, one turn | 48 MB | 57 MB |
    | ● 100 | Install size | 4 MB | 10 MB |
    | ● 97 | Wasm size | 250 kB | 350 kB |
    ");

    // ts's peak memory is scored against its line target, and `default` keeps the metric's target.
    let json = fs::read_to_string(project.path().join(".uphill/report.json")).unwrap();
    let metric = json.rfind("\"name\": \"turn-rss\"").unwrap();
    let start = metric + json[metric..].find("\"target\"").unwrap();
    let end = metric + json[metric..].find("\"note\"").unwrap();
    insta::assert_snapshot!(json[start..end], @r#"
    "target": 57000000,
              "default": 16000000,
    "#);

    // The page has a tab per subject, shows titles, and loads nothing from the network.
    let html = fs::read_to_string(project.path().join(".uphill/report.html")).unwrap();
    assert_eq!(html.matches("role=\"tab\"").count(), 2);
    assert!(html.contains(">zc</button>") && html.contains(">ts</button>"));
    assert!(html.contains("<h2>Size and memory</h2>"));
    assert!(html.contains("<span class=\"metric-title\">Peak memory, one turn</span>"));
    assert!(!html.contains("http://") && !html.contains("https://"));

    // The next 5 runs write zc's memory as 6 to 10 MB, which is not a change.
    let again = uphill_in(project.path(), &["--require-baseline"], &[]);
    assert_eq!(again.status.code(), Some(0), "{}", stdout(&again));
    assert!(stdout(&again).contains("\nNo changes since the baseline\n"));
}

#[test]
fn require_targets_fails_on_each_missed_target_and_failed_script() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("uphill.yaml"),
        "quality:\n  run: sh metrics.sh\n  size: { target: 2kB }\n  rate: { target: 2000, better: higher }\n  ok: {}\n",
    )
    .unwrap();
    let script = dir.path().join("metrics.sh");

    // A value equal to its target meets it, in either direction.
    fs::write(
        &script,
        "printf 'size 2kB\\nrate 2000\\nok pass\\n' >> \"$UPHILL_METRICS\"\n",
    )
    .unwrap();
    let met = uphill_in(dir.path(), &["--require-targets"], &[]);
    assert_eq!(met.status.code(), Some(0), "{}", stdout(&met));
    assert!(!stdout(&met).contains("Missed targets"));

    fs::write(
        &script,
        "printf 'size 2.1kB\\nrate 1999\\nok pass\\n' >> \"$UPHILL_METRICS\"\n",
    )
    .unwrap();
    let past = uphill_in(dir.path(), &["--require-targets"], &[]);
    assert_eq!(past.status.code(), Some(1));
    assert!(stdout(&past).ends_with(
        "
Missed targets
  quality  size  2.1 kB  target 2 kB
  quality  rate  1999    target 2000

2 targets missed
"
    ));

    // A list misses at the item its roll-up selects, each failing check item is a miss, and each
    // miss shows its note.
    let scored = fixture("scored");
    let output = uphill_in(scored.path(), &["--require-targets"], &[]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stdout(&output).ends_with(
        "
Missed targets
  footprint  bundle                     27.5 kB  target 5 kB  includes all of lodash
  footprint  export-size viem/op-stack  9 kB     target 5 kB
  footprint  links about.html           fail                  links to /team, which is gone
  speed      render parse               5 ms     target 2 ms

4 targets missed
"
    ));

    let failing = fixture("failing");
    let output = uphill_in(failing.path(), &["--require-targets"], &[]);
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
fn compare_reports_what_changed_since_a_revision_and_leaves_timing_noise_out() {
    let project = repository("compare");
    let dir = project.path();
    // This checkout is prepared by hand, and `setup` prepares the revision's checkout.
    fs::write(dir.join(".ready"), "").unwrap();

    let (output, unchanged) = compare_head(dir, &[]);
    assert_eq!(output.status.code(), Some(0), "{unchanged}");
    insta::assert_snapshot!(unchanged, @"
    Comparing with HEAD (abc1234)

    ●  92  footprint
      ●  92  bundle  27.5 kB  target 30 kB

    ●  99  speed  median of 8 rounds
      ●  98  encode  2 ms   target 3 ms
      ● 100  parse   10 ms  target 30 ms

    No baseline yet. Run `uphill --update` to record these values as the baseline.

    No changes against HEAD (abc1234)
      2 timings within noise

    Wrote .uphill/report.json, .uphill/report.md, .uphill/report.html
    ");

    // A larger bundle, a slower encode in every pair, and a parse time that is slower in only
    // 5 of 8 pairs. The script that measures speed changes too.
    fs::write(dir.join("bundle.txt"), "30kB\n").unwrap();
    fs::write(dir.join("encode.txt"), "2.5ms\n").unwrap();
    fs::write(dir.join("parse.txt"), "20\n20\n20\n20\n20\n5\n5\n5\n").unwrap();
    let speed = fs::read_to_string(dir.join("speed.sh")).unwrap();
    fs::write(dir.join("speed.sh"), speed + "# Changed.\n").unwrap();

    let (output, changed) = compare_head(dir, &[]);
    assert_eq!(output.status.code(), Some(0), "{changed}");
    insta::assert_snapshot!(changed, @"
    Comparing with HEAD (abc1234)
    warning: speed.sh differs in HEAD (abc1234), so its changes may come from the script

    ●  90  footprint
      ●  90  bundle  30 kB  target 30 kB

    ●  96  speed  median of 8 rounds
      ●  94  encode  2.5 ms  target 3 ms
      ●  98  parse   20 ms   target 30 ms

    No baseline yet. Run `uphill --update` to record these values as the baseline.

    Against HEAD (abc1234)
      worse  footprint  bundle  27.5 kB → 30 kB
      worse  speed      encode  2 ms → 2.5 ms (+25%)  8 of 8 pairs
      1 timing within noise

    Wrote .uphill/report.json, .uphill/report.md, .uphill/report.html
    ");

    let markdown = masked(
        dir,
        &fs::read_to_string(dir.join(".uphill/report.md")).unwrap(),
    );
    let start = markdown.find("## Changes against").unwrap();
    let end = markdown.find("## Footprint").unwrap();
    insta::assert_snapshot!(markdown[start..end], @"
    ## Changes against HEAD (abc1234)

    | Change | Category | Metric | Before | After | Pairs |
    | --- | --- | --- | --- | --- | --- |
    | worse | Footprint | Bundle | 27.5 kB | 30 kB |  |
    | worse | Speed | Encode | 2 ms | 2.5 ms (+25%) | 8 of 8 pairs |

    1 timing within noise.
    ");

    let json = masked(
        dir,
        &fs::read_to_string(dir.join(".uphill/report.json")).unwrap(),
    );
    let start = json.find("\"comparison\"").unwrap();
    insta::assert_snapshot!(json[start..], @r#"
    "comparison": {
        "reference": "HEAD (abc1234)",
        "changes": [
          {
            "category": "footprint",
            "subject": null,
            "metric": "bundle",
            "item": null,
            "change": "worse",
            "before": 27500,
            "after": 30000
          }
        ],
        "timings": [
          {
            "category": "speed",
            "subject": null,
            "metric": "encode",
            "item": null,
            "change": "worse",
            "before": 2000000,
            "after": 2500000,
            "pairs": 8,
            "agree": 8
          },
          {
            "category": "speed",
            "subject": null,
            "metric": "parse",
            "item": null,
            "change": "within noise",
            "before": 10000000,
            "after": 20000000,
            "pairs": 8,
            "agree": 5
          }
        ]
      }
    }
    "#);

    // Only the repository's own checkout remains.
    assert_eq!(git(dir, &["worktree", "list"]).lines().count(), 1);
}

#[test]
fn compare_uses_the_revision_baselines_and_always_removes_its_checkout() {
    let project = repository("compare");
    let dir = project.path();
    fs::write(dir.join(".ready"), "").unwrap();
    let config = fs::read_to_string(dir.join("uphill.yaml")).unwrap();

    // Without baselines the revision is measured, and a script that fails there leaves its
    // category out of the comparison.
    let footprint = fs::read_to_string(dir.join("footprint.sh")).unwrap();
    fs::write(dir.join("footprint.sh"), "exit 3\n").unwrap();
    commit(dir);
    fs::write(dir.join("footprint.sh"), footprint).unwrap();
    let (output, text) = compare_head(dir, &["footprint"]);
    assert_eq!(output.status.code(), Some(0), "{text}");
    insta::assert_snapshot!(text, @"
    Comparing with HEAD (abc1234)
    warning: footprint.sh differs in HEAD (abc1234), so its changes may come from the script

    ●  92  footprint
      ●  92  bundle  27.5 kB  target 30 kB
        warning: footprint failed in HEAD (abc1234): script exited with status 3

    No baseline yet. Run `uphill --update` to record these values as the baseline.

    No changes against HEAD (abc1234)

    Wrote .uphill/report.json, .uphill/report.md, .uphill/report.html
    ");
    assert_eq!(git(dir, &["worktree", "list"]).lines().count(), 1);

    // The revision's baseline wins over measuring it, which would find the same 27.5 kB.
    fs::write(
        dir.join("uphill.yaml"),
        config.replace("{ target: 30kB }", "{ target: 30kB, baseline: 25kB }"),
    )
    .unwrap();
    commit(dir);
    let (output, text) = compare_head(dir, &["footprint"]);
    assert_eq!(output.status.code(), Some(0), "{text}");
    insta::assert_snapshot!(text, @"
    Comparing with HEAD (abc1234)

    ●  92  footprint
      ●  92  bundle  27.5 kB  target 30 kB

    Since the baseline
      worse  footprint  bundle  25 kB → 27.5 kB

    Against HEAD (abc1234)
      worse  footprint  bundle  25 kB → 27.5 kB

    Wrote .uphill/report.json, .uphill/report.md, .uphill/report.html
    ");

    // A failed setup stops the run before any script.
    fs::write(
        dir.join("uphill.yaml"),
        config.replace(
            "setup: touch .ready",
            "setup: echo 'no network' >&2; exit 4",
        ),
    )
    .unwrap();
    let (output, _) = compare_head(dir, &[]);
    assert_eq!(output.status.code(), Some(2));
    insta::assert_snapshot!(masked(dir, &stderr(&output)), @"
    error: setup in HEAD (abc1234) exited with status 4
    no network
    ");
    assert_eq!(git(dir, &["worktree", "list"]).lines().count(), 1);

    let output = uphill_in(dir, &["--compare", "no-such-branch"], &[]);
    assert_eq!(output.status.code(), Some(2));
    insta::assert_snapshot!(stderr(&output), @"error: unknown revision 'no-such-branch'");
}

#[test]
fn blame_bisects_commits_until_baselines_record_the_metric_and_log_lists_them() {
    let project = repository("compare");
    let dir = project.path();
    // Three more commits: one moves the bundle, one slows encode, and one changes nothing.
    let mut day = 2;
    let mut commit_as = |dir: &Path, title: &str| {
        git(dir, &["add", "-A"]);
        let date = format!("2026-01-0{day}T12:00:00");
        git(dir, &["commit", "-q", "-m", title, "--date", &date]);
        day += 1;
    };
    fs::write(dir.join("bundle.txt"), "30kB\n").unwrap();
    commit_as(dir, "feat: inline the icons");
    fs::write(dir.join("encode.txt"), "2.5ms\n").unwrap();
    commit_as(dir, "refactor: encode in one pass");
    fs::write(dir.join("README.md"), "Docs.\n").unwrap();
    commit_as(dir, "docs: add a readme");

    // Names each commit by its position, oldest first, so snapshots do not depend on hashes.
    let named = |text: &str| {
        let log = git(dir, &["log", "--reverse", "--format=%h"]);
        log.lines()
            .enumerate()
            .fold(text.to_owned(), |text, (index, hash)| {
                text.replace(hash, &format!("commit{}", index + 1))
            })
    };

    let output = uphill_in(dir, &["blame", "footprint", "bundle"], &[]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    insta::assert_snapshot!(named(&stdout(&output)), @"
    Bisecting footprint bundle across 4 commits, measuring each in a temporary checkout
      HEAD     30 kB
      commit1  27.5 kB
      commit2  30 kB  same as HEAD

    commit2 moved footprint bundle from 27.5 kB to 30 kB
      2026-01-02  feat: inline the icons
    ");

    let output = uphill_in(dir, &["blame", "speed", "encode"], &[]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    insta::assert_snapshot!(named(&stdout(&output)), @"
    Bisecting speed encode across 4 commits, measuring each in a temporary checkout
      HEAD     2.5 ms
      commit1  2 ms
      commit2  2 ms
      commit3  2.5 ms  same as HEAD

    commit3 moved speed encode from 2 ms to 2.5 ms
      2026-01-03  refactor: encode in one pass
    ");

    // Parse writes the same times at every commit, so nothing moved it.
    let output = uphill_in(dir, &["blame", "speed", "parse"], &[]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    insta::assert_snapshot!(named(&stdout(&output)), @"
    Bisecting speed parse across 4 commits, measuring each in a temporary checkout
      HEAD     10 ms
      commit1  10 ms  same as HEAD

    Nothing moved speed parse since commit1: it measures 10 ms there too
    ");
    assert_eq!(git(dir, &["worktree", "list"]).lines().count(), 1);

    let output = uphill_in(dir, &["log", "footprint", "bundle"], &[]);
    insta::assert_snapshot!(stdout(&output), @"No baseline or note records footprint bundle. `uphill --note` keeps each run, and `uphill blame` measures commits");

    // Once baselines record the bundle, its history comes from them.
    fs::write(dir.join(".ready"), "").unwrap();
    uphill_in(dir, &["footprint", "--update"], &[]);
    commit_as(dir, "chore: record baselines");
    fs::write(dir.join("bundle.txt"), "28kB\n").unwrap();
    uphill_in(dir, &["footprint", "--update"], &[]);
    commit_as(dir, "perf: subset the font");

    let output = uphill_in(dir, &["log", "footprint", "bundle"], &[]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    insta::assert_snapshot!(named(&stdout(&output)), @"
    footprint bundle in the baselines of uphill.baseline.yaml

      commit6  2026-01-06  28 kB  from 30 kB  perf: subset the font
      commit5  2026-01-05  30 kB              chore: record baselines
    ");

    let output = uphill_in(dir, &["blame", "footprint", "bundle"], &[]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    insta::assert_snapshot!(named(&stdout(&output)), @"
    commit6 moved footprint bundle from 30 kB to 28 kB, by the baselines in uphill.baseline.yaml
      2026-01-06  perf: subset the font
    ");

    // After the speed script moves, older commits still run the command their configuration
    // declares.
    fs::create_dir(dir.join("scripts")).unwrap();
    fs::rename(dir.join("speed.sh"), dir.join("scripts/speed.sh")).unwrap();
    let config = fs::read_to_string(dir.join("uphill.yaml")).unwrap();
    fs::write(
        dir.join("uphill.yaml"),
        config.replace("run: sh speed.sh", "run: sh scripts/speed.sh"),
    )
    .unwrap();
    commit_as(dir, "chore: move the scripts");
    let output = uphill_in(dir, &["blame", "speed", "encode"], &[]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    insta::assert_snapshot!(named(&stdout(&output)), @"
    Bisecting speed encode across 7 commits, measuring each in a temporary checkout
      HEAD     2.5 ms
      commit1  2 ms
      commit4  2.5 ms  same as HEAD
      commit2  2 ms
      commit3  2.5 ms  same as HEAD

    commit3 moved speed encode from 2 ms to 2.5 ms
      2026-01-03  refactor: encode in one pass
    ");

    let output = uphill_in(dir, &["blame", "footprint", "size"], &[]);
    assert_eq!(output.status.code(), Some(2));
    insta::assert_snapshot!(stderr(&output), @"error: unknown metric 'size'. footprint configures bundle");

    let output = uphill_in(dir, &["log", "footprint"], &[]);
    assert_eq!(output.status.code(), Some(2));
    insta::assert_snapshot!(stderr(&output).lines().next().unwrap(), @"code: VALIDATION_ERROR");
}

#[test]
fn notes_keep_runs_for_log_and_the_trends_in_report_html() {
    let project = repository("compare");
    let dir = project.path();
    fs::write(dir.join(".ready"), "").unwrap();
    // `uphill --note` commits to the notes ref with the repository's identity.
    git(dir, &["config", "user.name", "Uphill"]);
    git(dir, &["config", "user.email", "uphill@example.com"]);
    let mut day = 1;
    let mut dated = |dir: &Path, args: &[&str]| {
        let date = format!("2026-01-0{day}T12:00:00");
        git(dir, &[args, &["--date", &date]].concat());
        day += 1;
    };
    dated(
        dir,
        &[
            "commit",
            "-q",
            "--amend",
            "-m",
            "feat: measure the bundle and speed",
        ],
    );
    let named = |text: &str| {
        let log = git(dir, &["log", "--reverse", "--format=%h"]);
        log.lines()
            .enumerate()
            .fold(text.to_owned(), |text, (index, hash)| {
                text.replace(hash, &format!("commit{}", index + 1))
            })
    };

    let output = uphill_in(dir, &["--note"], &[]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(named(&stdout(&output)).ends_with("report.html, a note on commit1\n"));
    insta::assert_snapshot!(git(dir, &["notes", "--ref", "refs/notes/uphill", "show", "HEAD"]), @"
    [footprint 92]
    bundle 27.5kB
    [speed 99]
    encode 2ms
    parse 10ms
    ");
    // The report note holds the same bytes as report.json, for readers outside the repository.
    assert_eq!(
        git(
            dir,
            &["notes", "--ref", "refs/notes/uphill-report", "show", "HEAD"]
        ),
        fs::read_to_string(dir.join(".uphill/report.json")).unwrap()
    );

    // Encode slows in one commit and the bundle grows in the next, and each run is noted.
    fs::write(dir.join("encode.txt"), "2.5ms\n").unwrap();
    git(dir, &["add", "-A"]);
    dated(dir, &["commit", "-q", "-m", "refactor: encode in one pass"]);
    uphill_in(dir, &["--note"], &[]);
    fs::write(dir.join("bundle.txt"), "30kB\n").unwrap();
    git(dir, &["add", "-A"]);
    dated(dir, &["commit", "-q", "-m", "feat: inline the icons"]);
    uphill_in(dir, &["--note"], &[]);

    let output = uphill_in(dir, &["log", "speed", "encode"], &[]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    insta::assert_snapshot!(named(&stdout(&output)), @"
    speed encode in the runs noted in refs/notes/uphill

      commit3  2026-01-03  2.5 ms  feat: inline the icons
      commit2  2026-01-02  2.5 ms  refactor: encode in one pass
      commit1  2026-01-01  2 ms    feat: measure the bundle and speed
    ");
    let output = uphill_in(dir, &["log", "footprint", "bundle"], &[]);
    insta::assert_snapshot!(named(&stdout(&output)), @"
    footprint bundle in the runs noted in refs/notes/uphill

      commit3  2026-01-03  30 kB    feat: inline the icons
      commit2  2026-01-02  27.5 kB  refactor: encode in one pass
      commit1  2026-01-01  27.5 kB  feat: measure the bundle and speed
    ");

    // The report leaves out the note on HEAD, which this run stands for.
    uphill_in(dir, &[], &[]);
    let html = fs::read_to_string(dir.join(".uphill/report.html")).unwrap();
    let trends: Vec<&str> = html
        .split("<div class=\"plot\"")
        .skip(1)
        .filter_map(|plot| plot.split("aria-label=\"").nth(1)?.split('"').next())
        .collect();
    insta::assert_debug_snapshot!(trends, @r#"
    [
        "Score over the last 3 runs, from 92 to 90",
        "Bundle over the last 3 runs, from 27.5 kB to 30 kB",
        "Score over the last 3 runs, from 99 to 97",
        "Encode over the last 3 runs, from 2 ms to 2.5 ms",
        "Parse over the last 3 runs, from 10 ms to 10 ms",
    ]
    "#);

    fs::write(dir.join("bundle.txt"), "31kB\n").unwrap();
    let output = uphill_in(dir, &["--note"], &[]);
    assert_eq!(output.status.code(), Some(2));
    insta::assert_snapshot!(stderr(&output), @"error: --note needs tracked files that match HEAD, so the note describes that commit. Commit or stash the changes first");
}

#[test]
fn dashboard_reads_each_repository_from_its_report_or_compact_notes() {
    // One repository notes two runs, one notes a run before report notes existed, and one notes
    // none.
    let (noted, compact, none) = (
        repository("compare"),
        repository("compare"),
        repository("compare"),
    );
    for dir in [noted.path(), compact.path(), none.path()] {
        fs::write(dir.join(".ready"), "").unwrap();
        git(dir, &["config", "user.name", "Uphill"]);
        git(dir, &["config", "user.email", "uphill@example.com"]);
    }
    uphill_in(noted.path(), &["--note"], &[]);
    fs::write(noted.path().join("encode.txt"), "2.5ms\n").unwrap();
    commit(noted.path());
    uphill_in(noted.path(), &["--note"], &[]);
    uphill_in(compact.path(), &["--note"], &[]);
    git(
        compact.path(),
        &["update-ref", "-d", "refs/notes/uphill-report"],
    );

    let out = tempfile::tempdir().unwrap();
    let names = [noted.path(), compact.path(), none.path()].map(|dir| dir.to_str().unwrap());
    let shown = |text: &str| {
        let text = masked(compact.path(), &masked(noted.path(), text));
        let date = git(noted.path(), &["log", "-1", "--format=%ad", "--date=short"]);
        names.iter().zip(["noted", "compact", "none"]).fold(
            text.replace(date.trim(), "2026-01-01"),
            |text, (name, shown)| text.replace(name, shown),
        )
    };

    let output = uphill_in(out.path(), &[&["dashboard"][..], &names].concat(), &[]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    insta::assert_snapshot!(shown(&stdout(&output)), @"
    noted  abc1234  2026-01-01  report note
      ●  92  footprint
      ●  97  speed

    compact  abc1234  2026-01-01  compact note
      ●  92  footprint
      ●  99  speed

    none  no noted runs

    Wrote .uphill/dashboard.html
    ");

    let html = fs::read_to_string(out.path().join(".uphill/dashboard.html")).unwrap();
    let sections: Vec<String> = html
        .split("<section class=\"block\"")
        .skip(1)
        .map(|section| {
            let trends = section.matches("<div class=\"plot\"").count();
            let source = section
                .split("<p class=\"muted\">")
                .nth(1)
                .unwrap_or_default();
            let source = source.split("</p>").next().unwrap_or_default();
            format!("{trends} trends, {}", shown(source))
        })
        .collect();
    insta::assert_snapshot!(sections.join("\n"), @"
    2 trends, Newest noted run: <code>abc1234</code> on 2026-01-01, change. From its report note.
    0 trends, Newest noted run: <code>abc1234</code> on 2026-01-01, change. From its compact note, which has no targets or metric scores.
    ");
    // The report note's targets, which the compact note does not hold.
    assert!(
        html.contains(
            "<td>Encode</td><td class=\"value\">2.5 ms</td><td class=\"value\">3 ms</td>"
        )
    );
    assert!(shown(&html).contains("<p class=\"muted\">No noted runs: none.</p>"));

    let output = uphill_in(out.path(), &["dashboard", "missing"], &[]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).starts_with("error: could not fetch missing: "));
}

#[test]
fn compact_prints_scores_misses_and_opportunities_in_a_few_lines() {
    let project = fixture("scored");

    // Each miss shows once, with its note, even when --require-targets fails the run.
    let output = uphill_in(project.path(), &["--compact", "--require-targets"], &[]);

    assert_eq!(output.status.code(), Some(1));
    insta::assert_snapshot!(stdout(&output), @"
    footprint 42, speed 75

    Missed targets
      footprint  bundle                     27.5 kB  target 5 kB  includes all of lodash
      footprint  export-size viem/op-stack  9 kB     target 5 kB
      footprint  links about.html           fail                  links to /team, which is gone
      speed      render parse               5 ms     target 2 ms

    Opportunities
      +29  footprint  bundle                     27.5 kB → 5 kB
      +19  speed      render parse               5 ms → 2 ms
      +11  footprint  links about.html           fail → pass
      +3   footprint  export-size viem/chains    10 kB → 5 kB
      +3   footprint  export-size viem/op-stack  9 kB → 5 kB

    No baseline yet. Run `uphill --update` to record these values as the baseline.

    Wrote .uphill/report.json, .uphill/report.md, .uphill/report.html

    4 targets missed
    ");

    // When no single item moves the score enough, the failing items of a list are one
    // opportunity, and they go from fail to pass.
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("uphill.yaml"),
        "quality:\n  run: sh metrics.sh\n  ok: {}\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("metrics.sh"),
        "i=0\nwhile [ $i -lt 250 ]; do\n  if [ $i -lt 150 ]; then r=pass; else r=fail; fi\n  echo \"ok item$i $r\" >> \"$UPHILL_METRICS\"\n  i=$((i + 1))\ndone\n",
    )
    .unwrap();
    let output = uphill_in(dir.path(), &["--compact"], &[]);
    let opportunities = stdout(&output)
        .split("Opportunities\n")
        .nth(1)
        .and_then(|rest| rest.split("\n\n").next())
        .map(str::to_owned);
    insta::assert_debug_snapshot!(opportunities, @r#"
    Some(
        "  +40  quality  ok, 100 items  fail → pass",
    )
    "#);
}

#[test]
fn init_points_the_brief_at_the_closest_example() {
    let brief = |files: &[(&str, &str)]| {
        let dir = tempfile::tempdir().unwrap();
        for (path, contents) in files {
            let path = dir.path().join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, contents).unwrap();
        }
        let output = uphill_in(dir.path(), &["init"], &[]);
        assert_eq!(output.status.code(), Some(0));
        let text = stdout(&output);
        text.lines()
            .find(|line| line.contains("example"))
            .unwrap()
            .to_owned()
    };

    insta::assert_snapshot!(brief(&[("Cargo.toml", "[package]\n")]), @"The closest example is a Rust API: https://github.com/wevm/uphill/tree/main/examples/rust-api. Read its `uphill.yaml` and `metrics/` before writing anything. `uphill add wevm/uphill/examples/rust-api` copies its categories and scripts to fit.");
    insta::assert_snapshot!(brief(&[("package.json", r#"{ "devDependencies": { "vite": "7.0.0" } }"#)]), @"The closest example is a website: https://github.com/wevm/uphill/tree/main/examples/web-app. Read its `uphill.yaml` and `metrics/` before writing anything. `uphill add wevm/uphill/examples/web-app` copies its categories and scripts to fit.");
    insta::assert_snapshot!(brief(&[("package.json", "{}"), ("pages/index.html", "")]), @"The closest example is a website: https://github.com/wevm/uphill/tree/main/examples/web-app. Read its `uphill.yaml` and `metrics/` before writing anything. `uphill add wevm/uphill/examples/web-app` copies its categories and scripts to fit.");
    insta::assert_snapshot!(brief(&[("package.json", r#"{ "name": "lib" }"#)]), @"The closest example is a TypeScript library: https://github.com/wevm/uphill/tree/main/examples/typescript-library. Read its `uphill.yaml` and `metrics/` before writing anything. `uphill add wevm/uphill/examples/typescript-library` copies its categories and scripts to fit.");
    insta::assert_snapshot!(brief(&[("README.md", "")]), @"Read the closest example before writing anything: a TypeScript library (https://github.com/wevm/uphill/tree/main/examples/typescript-library), a website (https://github.com/wevm/uphill/tree/main/examples/web-app), or a Rust API (https://github.com/wevm/uphill/tree/main/examples/rust-api).");
    insta::assert_snapshot!(brief(&[("package.json", "{}"), (".agents/uphill.yaml", "a:\n  run: \"true\"\n  b: {}\n")]), @"The closest example is a TypeScript library: https://github.com/wevm/uphill/tree/main/examples/typescript-library. Read its `uphill.yaml` and `metrics/` before writing anything. `uphill add wevm/uphill/examples/typescript-library` copies its categories and scripts to fit. This project already has `.agents/uphill.yaml`. Extend it instead of starting over.");
}

#[test]
fn add_copies_a_packs_categories_and_scripts_into_the_project() {
    let parent = tempfile::tempdir().unwrap();
    let pack = parent.path().join("pack");
    let project = parent.path().join("project");
    fs::create_dir_all(&pack).unwrap();
    fs::create_dir_all(&project).unwrap();
    copy(fixture("pack").path(), &pack);
    let config = "own:\n  run: echo \"files 2\" >> \"$UPHILL_METRICS\"\n  files: {}\n";
    fs::write(project.join("uphill.yaml"), config).unwrap();

    // Each category keeps its comments, takes the pack's top-level `run` and `timeout` when it
    // sets none, and runs the copied scripts beside the project's configuration. Its comment
    // records the command that adds it again, with a local pack's path from the project root.
    let output = uphill_in(&project, &["add", "../pack"], &[]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    insta::assert_snapshot!(stdout(&output), @"
    Added footprint, lines to uphill.yaml
    Added metrics/

    Fit the scripts and targets to this project, then run `uphill check`
    ");
    let added = fs::read_to_string(project.join("uphill.yaml")).unwrap();
    insta::assert_snapshot!(added, @r#"
    own:
      run: echo "files 2" >> "$UPHILL_METRICS"
      files: {}

    # Added by `uphill add ../pack footprint`.
    # Bytes the project ships.
    footprint:
      run: sh metrics/sizes.sh
      timeout: 30s
      # Sum of every shipped file.
      total: { target: 1kB }

    # Added by `uphill add ../pack lines`.
    lines:
      run: sh metrics/lines.sh
      timeout: 30s
      # Lines in the project's sources.
      count: {}
    "#);
    let output = check(&project, &[]);
    assert_eq!(output.status.code(), Some(0), "{}", stdout(&output));

    // A category the project declares stops the command.
    let output = uphill_in(&project, &["add", "../pack", "lines"], &[]);
    assert_eq!(output.status.code(), Some(2));
    insta::assert_snapshot!(stderr(&output), @"
    error[duplicate-category] ../pack/.agents/uphill.yaml:13:1: the project already declares category `lines`. Add it under another name with `--as <name>`
      13 | lines:
         | ^
    ");

    // An update of an unchanged pack changes nothing. A local pack records no commit, so a script
    // that differs from the pack's is a conflict until `--force` takes the pack's version.
    let output = uphill_in(&project, &["add", "--update"], &[]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    insta::assert_snapshot!(stdout(&output), @"
    current   footprint
    current   lines
    ");
    fs::write(pack.join(".agents/metrics/lib.sh"), "count=5\n").unwrap();
    let output = uphill_in(&project, &["add", "--update"], &[]);
    assert_eq!(output.status.code(), Some(1));
    insta::assert_snapshot!(stdout(&output), @"
    conflict  metrics/lib.sh  changed in both this project and the pack

    Nothing was written. Merge by hand, or run `uphill add --update --force` to take the pack's version
    ");
    assert_eq!(
        fs::read_to_string(project.join("metrics/lib.sh")).unwrap(),
        "count=3\n"
    );
    let output = uphill_in(&project, &["add", "--update", "--force"], &[]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    insta::assert_snapshot!(stdout(&output), @"
    current   footprint
    current   lines
    updated   metrics/lib.sh
    ");
    assert_eq!(
        fs::read_to_string(project.join("metrics/lib.sh")).unwrap(),
        "count=5\n"
    );
    assert_eq!(
        fs::read_to_string(project.join("uphill.yaml")).unwrap(),
        added
    );

    // `--as` adds a category under another name. A script that differs from the pack's copy
    // stops the command before it writes anything.
    let other = parent.path().join("other");
    fs::create_dir_all(other.join("metrics")).unwrap();
    let config = "lines:\n  run: echo \"words 5\" >> \"$UPHILL_METRICS\"\n  words: {}\n";
    fs::write(other.join("uphill.yaml"), config).unwrap();
    fs::write(other.join("metrics/lib.sh"), "count=4\n").unwrap();
    let output = uphill_in(&other, &["add", "../pack", "lines", "--as", "loc"], &[]);
    assert_eq!(output.status.code(), Some(2));
    insta::assert_snapshot!(stderr(&output), @"error: metrics/lib.sh exists and differs from the pack's copy. Move it, then run `uphill add` again");
    assert_eq!(
        fs::read_to_string(other.join("uphill.yaml")).unwrap(),
        config
    );
    assert!(!other.join("metrics/lines.sh").exists());

    fs::remove_file(other.join("metrics/lib.sh")).unwrap();
    let output = uphill_in(&other, &["add", "../pack", "lines", "--as", "loc"], &[]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let added = fs::read_to_string(other.join("uphill.yaml")).unwrap();
    insta::assert_snapshot!(added.strip_prefix(config).unwrap(), @"

    # Added by `uphill add ../pack lines --as loc`.
    loc:
      run: sh metrics/lines.sh
      timeout: 30s
      # Lines in the project's sources.
      count: {}
    ");
}

#[test]
fn add_update_merges_a_packs_changes_since_its_recorded_commit() {
    // The pack lives in a Git repository, and each commit's hash is masked as it is made.
    let repository = tempfile::tempdir().unwrap();
    let packed = repository.path().join("packs/sizes");
    fs::create_dir_all(&packed).unwrap();
    copy(fixture("pack").path(), &packed);
    git(repository.path(), &["init", "-q", "-b", "main"]);
    let mut commits: Vec<String> = Vec::new();
    let commit_pack = |commits: &mut Vec<String>| {
        commit(repository.path());
        commits.push(
            git(repository.path(), &["rev-parse", "HEAD"])
                .trim()
                .to_owned(),
        );
    };
    commit_pack(&mut commits);
    let mask = |text: &str, commits: &[String]| {
        let mut text = text.replace(&repository.path().display().to_string(), "[repository]");
        for (index, commit) in commits.iter().enumerate() {
            text = text
                .replace(commit.as_str(), &format!("[c{}]", index + 1))
                .replace(&commit[..7], &format!("[c{}s]", index + 1));
        }
        text
    };
    let edit = |path: &Path, from: &str, to: &str| {
        let text = fs::read_to_string(path).unwrap();
        assert!(text.contains(from), "{} has `{from}`", path.display());
        fs::write(path, text.replace(from, to)).unwrap();
    };

    // A project without a configuration gets `.agents/uphill.yaml`, and each comment records the
    // pack's commit.
    let project = tempfile::tempdir().unwrap();
    let source = format!("{}//packs/sizes", repository.path().display());
    let config = project.path().join(".agents/uphill.yaml");
    let output = uphill_in(project.path(), &["add", &source], &[]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    insta::assert_snapshot!(mask(&fs::read_to_string(&config).unwrap(), &commits), @"
    # Added by `uphill add [repository]//packs/sizes footprint` at [c1].
    # Bytes the project ships.
    footprint:
      run: sh .agents/metrics/sizes.sh
      timeout: 30s
      # Sum of every shipped file.
      total: { target: 1kB }

    # Added by `uphill add [repository]//packs/sizes lines` at [c1].
    lines:
      run: sh .agents/metrics/lines.sh
      timeout: 30s
      # Lines in the project's sources.
      count: {}
    ");
    let output = check(project.path(), &[]);
    assert_eq!(output.status.code(), Some(0), "{}", stdout(&output));

    // The pack changes footprint and a script, and the project changes lines. Each side's change
    // comes through, and every comment records the new commit.
    edit(
        &packed.join(".agents/uphill.yaml"),
        "total: { target: 1kB }",
        "total: { target: 2kB }",
    );
    edit(&packed.join(".agents/metrics/lib.sh"), "count=3", "count=5");
    commit_pack(&mut commits);
    edit(&config, "  count: {}", "  count: { weight: 2 }");
    let output = uphill_in(project.path(), &["add", "--update"], &[]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    insta::assert_snapshot!(mask(&stdout(&output), &commits), @"
    updated   footprint from [c1s] to [c2s]
    kept      lines from [c1s] to [c2s], with this project's changes
    updated   .agents/metrics/lib.sh
    ");
    let merged = fs::read_to_string(&config).unwrap();
    insta::assert_snapshot!(mask(&merged, &commits), @"
    # Added by `uphill add [repository]//packs/sizes footprint` at [c2].
    # Bytes the project ships.
    footprint:
      run: sh .agents/metrics/sizes.sh
      timeout: 30s
      # Sum of every shipped file.
      total: { target: 2kB }

    # Added by `uphill add [repository]//packs/sizes lines` at [c2].
    lines:
      run: sh .agents/metrics/lines.sh
      timeout: 30s
      # Lines in the project's sources.
      count: { weight: 2 }
    ");
    assert_eq!(
        fs::read_to_string(project.path().join(".agents/metrics/lib.sh")).unwrap(),
        "count=5\n"
    );

    // When the pack changes lines too, the update writes nothing until `--force` takes the pack's
    // version.
    edit(
        &packed.join(".agents/uphill.yaml"),
        "# Lines in the project's sources.",
        "# Lines of source code.",
    );
    commit_pack(&mut commits);
    let output = uphill_in(project.path(), &["add", "--update"], &[]);
    assert_eq!(output.status.code(), Some(1));
    insta::assert_snapshot!(stdout(&output), @"
    conflict  lines  changed in both this project and the pack

    Nothing was written. Merge by hand, or run `uphill add --update --force` to take the pack's version
    ");
    assert_eq!(fs::read_to_string(&config).unwrap(), merged);

    let output = uphill_in(
        project.path(),
        &["add", "--update", "lines", "--force"],
        &[],
    );
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    insta::assert_snapshot!(mask(&stdout(&output), &commits), @"updated   lines from [c2s] to [c3s]");
    insta::assert_snapshot!(mask(&fs::read_to_string(&config).unwrap(), &commits), @"
    # Added by `uphill add [repository]//packs/sizes footprint` at [c2].
    # Bytes the project ships.
    footprint:
      run: sh .agents/metrics/sizes.sh
      timeout: 30s
      # Sum of every shipped file.
      total: { target: 2kB }

    # Added by `uphill add [repository]//packs/sizes lines` at [c3].
    lines:
      run: sh .agents/metrics/lines.sh
      timeout: 30s
      # Lines of source code.
      count: {}
    ");
}

#[test]
fn rejects_an_invalid_baseline() {
    let project = fixture("scored");
    fs::write(
        project.path().join("uphill.baseline.yaml"),
        "footprint:\n  bundle: 27.5 kilobytes\n",
    )
    .unwrap();

    let output = uphill_in(project.path(), &["footprint"], &[]);

    assert_eq!(output.status.code(), Some(2));
    insta::assert_snapshot!(stderr(&output), @"
    error[invalid-value] uphill.baseline.yaml:2:11: expected a size, count, or check, optionally followed by `target <value>`, or a mapping of items or subjects to them. Run `uphill --update` to rewrite the baseline
      2 |   bundle: 27.5 kilobytes
        |           ^
    ");
}

#[test]
fn update_writes_a_toml_baseline_file_and_moves_inline_baselines_into_it() {
    let project = fixture("passing");
    let path = project.path().join("uphill.toml");
    let config = "[footprint]\nrun = \"sh metrics/footprint.sh\"\nexport-size.target = \"5kB\" # gzipped\nmodules = { baseline = 400 }\n\n[speed]\nrun = \"sh metrics/speed.sh\"\n\n[speed.render]\ntarget = \"2ms\" # per item\n";
    fs::write(&path, config).unwrap();
    let speed = project.path().join("metrics/speed.sh");
    let script = fs::read_to_string(&speed).unwrap();
    fs::write(
        &speed,
        script.replace("render encode 236us", "render encode 236us target 300us"),
    )
    .unwrap();

    // The inline baseline moves into the baseline file, and a metric with dotted keys needs no
    // edit. A baseline with items takes a table of its own, one line per item.
    let output = uphill_in(project.path(), &["--update"], &[]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(
        stdout(&output).ends_with(
            "\nWrote .uphill/report.json, .uphill/report.md, .uphill/report.html, uphill.baseline.toml, uphill.toml\n"
        ),
        "{}",
        stdout(&output)
    );
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        config.replace("modules = { baseline = 400 }", "modules = {}")
    );
    let baselines = fs::read_to_string(project.path().join("uphill.baseline.toml")).unwrap();
    insta::assert_snapshot!(baselines, @r#"
    # Written by `uphill --update`: the accepted sizes, counts, checks, and line targets of each
    # metric. Each run reports what changed since.

    [footprint]
    modules = 412

    [footprint.export-size]
    viem = "4.2kB"
    "viem/chains" = "61kB"

    [speed.render]
    encode = "target 300µs"
    "#);

    // Without its line target, the timing records nothing, so its table goes.
    fs::write(&speed, script).unwrap();
    let output = uphill_in(project.path(), &["--update"], &[]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(
        fs::read_to_string(project.path().join("uphill.baseline.toml")).unwrap(),
        baselines.replace("\n[speed.render]\nencode = \"target 300µs\"\n", "")
    );
}

#[test]
fn unknown_category() {
    let project = fixture("scored");

    let output = uphill_in(project.path(), &["bench"], &[]);

    assert_eq!(output.status.code(), Some(2));
    insta::assert_snapshot!(stderr(&output), @"error: unknown category 'bench'. uphill.yaml declares footprint, speed");
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
        error[metric-missing] uphill.yaml:5:3: metric `modules` is configured, but the script did not write it
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

    let output = check(project.path(), &[("UPHILL_TEST_PID", &pid_file)]);

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
    insta::assert_snapshot!(stderr(&output), @"error: no uphill.yaml or uphill.toml, at the root or in .agents, in this directory or any parent directory");
}

#[test]
fn check_with_an_invalid_config_names_it_relative_to_the_current_directory() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join(".agents")).unwrap();
    fs::write(
        dir.path().join(".agents/uphill.yaml"),
        "footprint:\n  run: \"true\"\n  size: { target: 50kb }\n",
    )
    .unwrap();
    fs::create_dir(dir.path().join("src")).unwrap();

    let output = check(&dir.path().join("src"), &[]);

    assert_eq!(output.status.code(), Some(2));
    insta::assert_snapshot!(stderr(&output), @"
    error[unknown-unit] ../.agents/uphill.yaml:3:21: unknown unit `kb`, expected one of `B`, `kB`, `MB`, `GB`, `ns`, `µs`, `us`, `ms`, `s`
      3 |   size: { target: 50kb }
        |                     ^
    ");
}

#[test]
fn agents_config_runs_from_the_project_root_and_moves_inline_baselines_beside_it() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join(".agents/metrics")).unwrap();
    // An older configuration records the baseline in the metric's `baseline` key.
    fs::write(
        dir.path().join(".agents/uphill.yaml"),
        "footprint:\n  run: sh .agents/metrics/manifests.sh\n  manifests: { target: 2, baseline: 3 } # manifests\n",
    )
    .unwrap();
    // The script counts manifests in the directory it runs from, which is the project root.
    fs::write(
        dir.path().join(".agents/metrics/manifests.sh"),
        "echo \"manifests $(ls *.json | wc -l)\" >> \"$UPHILL_METRICS\"\n",
    )
    .unwrap();
    fs::write(dir.path().join("package.json"), "{}").unwrap();

    // Started inside `.agents`, Uphill still runs from the directory above it.
    let output = uphill_in(&dir.path().join(".agents"), &["--update"], &[]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    insta::assert_snapshot!(stdout(&output), @"
    ●  99  footprint
      ●  99  manifests  1  target 2

    Since the baseline
      better  footprint  manifests  3 → 1

    Wrote ../.uphill/report.json, ../.uphill/report.md, ../.uphill/report.html, ../.agents/uphill.baseline.yaml, ../.agents/uphill.yaml
    ");
    insta::assert_snapshot!(fs::read_to_string(dir.path().join(".agents/uphill.yaml")).unwrap(), @"
    footprint:
      run: sh .agents/metrics/manifests.sh
      manifests: { target: 2 } # manifests
    ");
    insta::assert_snapshot!(fs::read_to_string(dir.path().join(".agents/uphill.baseline.yaml")).unwrap(), @"
    # Written by `uphill --update`: the accepted sizes, counts, checks, and line targets of each
    # metric. Each run reports what changed since.

    footprint:
      manifests: 1
    ");
    assert!(dir.path().join(".uphill/report.html").exists());

    fs::write(
        dir.path().join("uphill.yaml"),
        "speed:\n  run: \"true\"\n  m: {}\n",
    )
    .unwrap();
    let output = check(dir.path(), &[]);
    assert_eq!(output.status.code(), Some(2));
    insta::assert_snapshot!(stderr(&output), @"error: uphill.yaml and .agents/uphill.yaml both exist. Keep one");
}
