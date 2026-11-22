//! Integration tests for checking one category run: the metric lines a script wrote against the
//! metrics its category configures.

use std::fmt::Write;

use engine::check::{self, Report, Source};
use engine::{config, lines};

const CONFIG: &str = r#"[footprint]
run = "node metrics/footprint.mjs"
export-size = { target = "5kB", limit = "50kB" }
modules = {}
"#;

/// Checks `metrics` against the `[footprint]` category and renders every finding.
fn check(metrics: &str) -> String {
    let config = config::parse(CONFIG).unwrap();
    let category = config.categories.first().unwrap();
    let lines = lines::parse(metrics).unwrap();

    render(&check::category(category, &lines), metrics)
}

fn render(report: &Report, metrics: &str) -> String {
    let findings = report
        .errors
        .iter()
        .map(|finding| ("error", finding))
        .chain(report.warnings.iter().map(|finding| ("warning", finding)));
    let mut out = format!("passed: {}, metrics: {}", report.passed(), report.metrics);

    for (severity, finding) in findings {
        let (name, source) = match finding.source {
            Source::Config => ("retros.toml", CONFIG),
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
    error [metric-missing] retros.toml:4:1: metric `modules` is configured, but the script did not write it
      4 | modules = {}
        | ^
    ");
}

#[test]
fn warns_about_a_metric_that_is_not_configured() {
    let report = check("export-size 4.2kB\nmodules 412\nchunks 9\n");

    insta::assert_snapshot!(report, @"
    passed: true, metrics: 3
    warning [unconfigured-metric] footprint metrics:3:1: metric `chunks` is written, but `[footprint]` does not configure it
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
fn reports_values_that_do_not_match_their_anchors() {
    let report = check("export-size 412\nmodules 412\n");

    insta::assert_snapshot!(report, @"
    passed: false, metrics: 2
    error [anchor-unit] footprint metrics:1:1: metric `export-size` is written as counts, but its anchors in retros.toml are sizes
      1 | export-size 412
        | ^
    ");
}
