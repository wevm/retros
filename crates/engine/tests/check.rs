//! Integration tests for checking one category run: the metric lines a script wrote against the
//! metrics its category configures.

use std::fmt::Write;

use engine::check::{self, Report, Source};
use engine::config::{self, Format};
use engine::lines;

const CONFIG: &str = r#"footprint:
  run: "node metrics/footprint.mjs"
  export-size: { target: "5kB" }
  modules: {}
"#;

/// Checks `metrics` against the `footprint` category and renders every finding.
fn check(metrics: &str) -> String {
    check_in(CONFIG, metrics)
}

/// Checks `metrics` against the first category of `config` and renders every finding.
fn check_in(config: &str, metrics: &str) -> String {
    let parsed = config::parse(config, Format::Yaml).unwrap();
    let category = parsed.categories.first().unwrap();
    let lines = lines::parse(metrics).unwrap();

    render(&check::category(category, &lines), config, metrics)
}

fn render(report: &Report, config: &str, metrics: &str) -> String {
    let findings = report
        .errors
        .iter()
        .map(|finding| ("error", finding))
        .chain(report.warnings.iter().map(|finding| ("warning", finding)));
    let mut out = format!("passed: {}, metrics: {}", report.passed(), report.metrics);

    for (severity, finding) in findings {
        let (name, source) = match finding.source {
            Source::Config => ("uphill.yaml", config),
            Source::Metrics => ("footprint metrics", metrics),
        };
        let _ = write!(
            out,
            "\n{severity} [{}] {}",
            finding.error.code.as_str(),
            finding.error.render(name, source)
        );
    }

    out
}

#[test]
fn passes_when_every_metric_is_written_once() {
    let report = check("export-size viem 4.2kB\nexport-size \"viem/chains\" 61kB\nmodules 412\n");

    insta::assert_snapshot!(report, @"passed: true, metrics: 2");
}

#[test]
fn reports_a_configured_metric_that_was_not_written() {
    let report = check("export-size viem 4.2kB\n");

    insta::assert_snapshot!(report, @"
    passed: false, metrics: 1
    error [metric-missing] uphill.yaml:4:3: metric `modules` is configured, but the script did not write it
      4 |   modules: {}
        |   ^
    ");
}

#[test]
fn warns_about_a_metric_that_is_not_configured() {
    let report = check("export-size 4.2kB\nmodules 412\nchunks 9\n");

    insta::assert_snapshot!(report, @"
    passed: true, metrics: 3
    warning [unconfigured-metric] footprint metrics:3:1: metric `chunks` is written, but `footprint` does not configure it
      3 | chunks 9
        | ^
    ");
}

#[test]
fn reports_mixed_units_and_items() {
    insta::assert_snapshot!(check("export-size viem 4.2kB\nexport-size chains 12ms\nmodules 412\n"), @"
    passed: false, metrics: 2
    error [mixed-units] footprint metrics:2:1: metric `export-size` mixes sizes and times. Line 1 writes sizes
      2 | export-size chains 12ms
        | ^
    ");

    insta::assert_snapshot!(check("export-size viem 4.2kB\nexport-size 61kB\nmodules 412\n"), @"
    passed: false, metrics: 2
    error [mixed-items] footprint metrics:2:1: metric `export-size` mixes lines with and without items
      2 | export-size 61kB
        | ^
    ");
}

#[test]
fn reports_duplicates() {
    insta::assert_snapshot!(check("export-size 4.2kB\nmodules 412\nmodules 413\n"), @"
    passed: false, metrics: 2
    error [duplicate] footprint metrics:3:1: metric `modules` is written more than once. Give each line an item to write a list
      3 | modules 413
        | ^
    ");

    insta::assert_snapshot!(check("export-size viem 4.2kB\nexport-size viem 4.3kB\nmodules 412\n"), @"
    passed: false, metrics: 2
    error [duplicate] footprint metrics:2:1: item `viem` of metric `export-size` is written more than once
      2 | export-size viem 4.3kB
        | ^
    ");
}

#[test]
fn reports_values_that_do_not_match_their_target() {
    let report = check("export-size 412\nmodules 412\n");

    insta::assert_snapshot!(report, @"
    passed: false, metrics: 2
    error [target-unit] footprint metrics:1:1: metric `export-size` is written as counts, but its configured `target` is a size
      1 | export-size 412
        | ^
    ");
}

#[test]
fn reports_targets_on_some_lines_only() {
    let report = check("export-size 4.2kB\nmodules viem 412 target 500\nmodules ox 120\n");

    insta::assert_snapshot!(report, @"
    passed: false, metrics: 2
    error [mixed-targets] footprint metrics:3:1: metric `modules` sets a target on some items but not this one. Set a target on every line, or a default `target` for the metric
      3 | modules ox 120
        | ^
    ");
}

#[test]
fn checks_target_patterns() {
    let config = r#"footprint:
  run: "true"
  modules: { targets: { "viem*": 500, "ox": "1kB", "abitype": 50 } }
"#;
    // Patterns give `viem` and `viem/chains` their targets, so only `zod` lacks one.
    let report = check_in(
        config,
        "modules viem 412\nmodules viem/chains 30\nmodules ox 120 target 200\nmodules zod 9\n",
    );

    insta::assert_snapshot!(report, @r#"
    passed: false, metrics: 1
    error [mixed-targets] footprint metrics:4:1: metric `modules` sets a target on some items but not this one. Set a target on every line, or a default `target` for the metric
      4 | modules zod 9
        | ^
    error [target-unit] uphill.yaml:3:39: metric `modules` is written as counts, but its target for `ox` is a size
      3 |   modules: { targets: { "viem*": 500, "ox": "1kB", "abitype": 50 } }
        |                                       ^
    warning [unmatched-pattern] uphill.yaml:3:52: pattern `abitype` in the `targets` of metric `modules` matches no item
      3 |   modules: { targets: { "viem*": 500, "ox": "1kB", "abitype": 50 } }
        |                                                    ^
    "#);
}

#[test]
fn checks_duplicates_within_each_subject() {
    let config = "speed:\n  run: \"true\"\n  startup: {}\n";

    insta::assert_snapshot!(check_in(config, "zc: startup 1ms\nts: startup 30ms\n"), @"passed: true, metrics: 1");
    insta::assert_snapshot!(check_in(config, "zc: startup 1ms\nzc: startup 2ms\n"), @"
    passed: false, metrics: 1
    error [duplicate] footprint metrics:2:5: metric `startup` is written more than once for subject `zc`. Give each line an item to write a list
      2 | zc: startup 2ms
        |     ^
    ");
}

#[test]
fn rejects_lines_with_and_without_subjects() {
    let config = "speed:\n  run: \"true\"\n  startup: {}\n  render: {}\n";

    insta::assert_snapshot!(check_in(config, "zc: startup 1ms\nrender 2ms\n"), @"
    passed: false, metrics: 2
    error [mixed-subjects] footprint metrics:2:1: `speed` mixes lines with and without subjects. Line 1 names a subject
      2 | render 2ms
        | ^
    ");
}

#[test]
fn routes_shared_lines_to_the_category_that_configures_them() {
    let config =
        "run: \"zig build metrics\"\n\nspeed:\n  startup: {}\n\nfootprint:\n  install-size: {}\n";
    let parsed = config::parse(config, Format::Yaml).unwrap();
    let categories: Vec<_> = parsed.categories.iter().collect();
    let lines = lines::parse("zc: install-size 2MB\nzc: startup 1ms\nzc: chunks 3\n").unwrap();

    let routed: Vec<Vec<&str>> = check::route(&categories, &lines)
        .iter()
        .map(|lines| lines.iter().map(|line| line.metric).collect())
        .collect();

    // `chunks` is in neither category, so it goes to the first, which warns about it.
    insta::assert_debug_snapshot!(routed, @r#"
    [
        [
            "startup",
            "chunks",
        ],
        [
            "install-size",
        ],
    ]
    "#);
}
