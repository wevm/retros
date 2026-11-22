//! Integration tests for parsing `retros.toml`: typed categories with defaults, and every
//! configuration error rendered with its file, line, and column.

use engine::config;

/// Parses `source` and renders the error it must produce, prefixed with its code.
fn error(source: &str) -> String {
    let error = config::parse(source).expect_err("the configuration should be rejected");
    format!(
        "[{}] {}",
        error.code.as_str(),
        error.render("retros.toml", source)
    )
}

#[test]
fn parses_categories_in_order_with_defaults() {
    let source = r#"
[speed]
run = "node metrics/speed.mjs"
timeout = "1.5s"
throughput = { target = 2000, better = "higher", weight = 2 }
decode = { score = "median" }

[footprint]
run = "node metrics/footprint.mjs"
export-size = { target = "1.1kB", targets = { "viem" = "60kB", "viem/chains/*" = "1kB" } }
bundle = { target = "45.6MB" }
modules = {}
"#;

    let config = config::parse(source).unwrap();

    insta::assert_debug_snapshot!(config, @r#"
    Config {
        categories: [
            Category {
                location: Location {
                    column: 2,
                    line: 2,
                },
                metrics: [
                    Metric {
                        better: Higher,
                        location: Location {
                            column: 1,
                            line: 5,
                        },
                        name: "throughput",
                        score: P10,
                        target: Some(
                            Count(
                                2000.0,
                            ),
                        ),
                        targets: [],
                        weight: 2.0,
                    },
                    Metric {
                        better: Lower,
                        location: Location {
                            column: 1,
                            line: 6,
                        },
                        name: "decode",
                        score: Median,
                        target: None,
                        targets: [],
                        weight: 1.0,
                    },
                ],
                name: "speed",
                run: "node metrics/speed.mjs",
                timeout: 1.5s,
            },
            Category {
                location: Location {
                    column: 2,
                    line: 8,
                },
                metrics: [
                    Metric {
                        better: Lower,
                        location: Location {
                            column: 1,
                            line: 10,
                        },
                        name: "export-size",
                        score: P90,
                        target: Some(
                            Size(
                                1100.0,
                            ),
                        ),
                        targets: [
                            Pattern {
                                location: Location {
                                    column: 47,
                                    line: 10,
                                },
                                pattern: "viem",
                                target: Size(
                                    60000.0,
                                ),
                            },
                            Pattern {
                                location: Location {
                                    column: 64,
                                    line: 10,
                                },
                                pattern: "viem/chains/*",
                                target: Size(
                                    1000.0,
                                ),
                            },
                        ],
                        weight: 1.0,
                    },
                    Metric {
                        better: Lower,
                        location: Location {
                            column: 1,
                            line: 11,
                        },
                        name: "bundle",
                        score: P90,
                        target: Some(
                            Size(
                                45600000.0,
                            ),
                        ),
                        targets: [],
                        weight: 1.0,
                    },
                    Metric {
                        better: Lower,
                        location: Location {
                            column: 1,
                            line: 12,
                        },
                        name: "modules",
                        score: P90,
                        target: None,
                        targets: [],
                        weight: 1.0,
                    },
                ],
                name: "footprint",
                run: "node metrics/footprint.mjs",
                timeout: 600s,
            },
        ],
    }
    "#);
}

#[test]
fn renders_timeouts_in_their_largest_exact_unit() {
    let source = r#"
[a]
run = "true"
timeout = "300ms"
m = {}

[b]
run = "true"
timeout = "90s"
m = {}

[c]
run = "true"
m = {}

[d]
run = "true"
timeout = "2h"
m = {}
"#;

    let config = config::parse(source).unwrap();
    let timeouts: Vec<String> = config
        .categories
        .iter()
        .map(|category| engine::format::duration(category.timeout))
        .collect();

    insta::assert_debug_snapshot!(timeouts, @r#"
    [
        "300ms",
        "90s",
        "10m",
        "2h",
    ]
    "#);
}

#[test]
fn rejects_invalid_toml_and_structure() {
    insta::assert_snapshot!(error("[footprint\nrun = \"true\"\n"), @"
    [toml] retros.toml:1:11: unclosed table, expected `]`
      1 | [footprint
        |           ^
    ");

    insta::assert_snapshot!(error("# nothing yet\n"), @"[nothing-configured] retros.toml: no categories. Add a table such as `[footprint]` with a `run` command");

    insta::assert_snapshot!(error("run = \"node metrics.mjs\"\n"), @r#"
    [invalid-type] retros.toml:1:1: top-level key `run` must be a category table, such as `[run]`
      1 | run = "node metrics.mjs"
        | ^
    "#);

    insta::assert_snapshot!(error("[footprint]\nexport-size = {}\n"), @"
    [run-missing] retros.toml:1:2: category `footprint` has no `run` command
      1 | [footprint]
        |  ^
    ");

    insta::assert_snapshot!(error("[footprint]\nrun = \"  \"\nexport-size = {}\n"), @r#"
    [invalid-value] retros.toml:2:7: `run` must be a command
      2 | run = "  "
        |       ^
    "#);

    insta::assert_snapshot!(error("[footprint]\nrun = \"true\"\n"), @"
    [nothing-configured] retros.toml:1:2: category `footprint` declares no metrics
      1 | [footprint]
        |  ^
    ");

    insta::assert_snapshot!(error("[\"foot print\"]\nrun = \"true\"\nm = {}\n"), @r#"
    [invalid-name] retros.toml:1:2: invalid category name `foot print`. Names use ASCII letters, digits, `-`, `_`, and `.`
      1 | ["foot print"]
        |  ^
    "#);

    insta::assert_snapshot!(error("[footprint]\nrun = \"true\"\nexport-size = \"5kB\"\n"), @r#"
    [invalid-type] retros.toml:3:15: metric `export-size` must be a table, such as `export-size = { target = "5kB" }` or `export-size = {}`
      3 | export-size = "5kB"
        |               ^
    "#);
}

#[test]
fn rejects_invalid_category_settings() {
    insta::assert_snapshot!(error("[speed]\nrun = \"true\"\ntimeout = \"10 minutes\"\nm = {}\n"), @r#"
    [invalid-value] retros.toml:3:11: invalid `timeout` `10 minutes`, expected a duration such as "30s", "10m", or "500ms"
      3 | timeout = "10 minutes"
        |           ^
    "#);

    insta::assert_snapshot!(error("[speed]\nrun = \"true\"\ntimeout = 600\nm = {}\n"), @"
    [invalid-type] retros.toml:3:11: `timeout` must be a string
      3 | timeout = 600
        |           ^
    ");
}

#[test]
fn rejects_invalid_metric_settings() {
    insta::assert_snapshot!(error("[footprint]\nrun = \"true\"\nsize = { target = \"5kB\", goal = 1 }\n"), @r#"
    [unexpected-key] retros.toml:3:26: unknown key `goal` in metric `size`, expected one of `better`, `score`, `target`, `targets`, `weight`
      3 | size = { target = "5kB", goal = 1 }
        |                          ^
    "#);

    insta::assert_snapshot!(error("[footprint]\nrun = \"true\"\nsize = { target = \"5kB\", limit = \"50kB\" }\n"), @r#"
    [unexpected-key] retros.toml:3:26: unknown key `limit` in metric `size`, expected one of `better`, `score`, `target`, `targets`, `weight`
      3 | size = { target = "5kB", limit = "50kB" }
        |                          ^
    "#);

    insta::assert_snapshot!(error("[footprint]\nrun = \"true\"\nsize = { better = \"smaller\" }\n"), @r#"
    [invalid-value] retros.toml:3:19: unknown `better` value `smaller`, expected `lower` or `higher`
      3 | size = { better = "smaller" }
        |                   ^
    "#);

    insta::assert_snapshot!(error("[footprint]\nrun = \"true\"\nsize = { score = \"p95\" }\n"), @r#"
    [invalid-value] retros.toml:3:18: unknown `score` value `p95`, expected one of `p90`, `p10`, `median`, `max`, `min`, `sum`
      3 | size = { score = "p95" }
        |                  ^
    "#);

    insta::assert_snapshot!(error("[footprint]\nrun = \"true\"\nsize = { weight = -1 }\n"), @"
    [invalid-value] retros.toml:3:19: `weight` cannot be negative
      3 | size = { weight = -1 }
        |                   ^
    ");

    insta::assert_snapshot!(error("[footprint]\nrun = \"true\"\nsize = { weight = 0x10 }\n"), @"
    [invalid-type] retros.toml:3:19: `weight` must be a finite number
      3 | size = { weight = 0x10 }
        |                   ^
    ");
}

#[test]
fn rejects_targets_that_cannot_score() {
    insta::assert_snapshot!(error("[footprint]\nrun = \"true\"\nsize = { target = \"5kb\" }\n"), @r#"
    [unknown-unit] retros.toml:3:21: unknown unit `kb`, expected one of `B`, `kB`, `MB`, `GB`, `ns`, `µs`, `us`, `ms`, `s`
      3 | size = { target = "5kb" }
        |                     ^
    "#);

    insta::assert_snapshot!(error("[speed]\nrun = \"true\"\nerrors = { target = 0 }\n"), @"
    [target-range] retros.toml:3:21: the `target` of metric `errors` must be greater than zero
      3 | errors = { target = 0 }
        |                     ^
    ");

    insta::assert_snapshot!(error("[footprint]\nrun = \"true\"\nsize = { target = \"-5kB\" }\n"), @r#"
    [invalid-number] retros.toml:3:20: sizes cannot be negative
      3 | size = { target = "-5kB" }
        |                    ^
    "#);

    insta::assert_snapshot!(error("[tests]\nrun = \"true\"\npassing = { target = \"pass\" }\n"), @r#"
    [target-unit] retros.toml:3:22: metric `passing` cannot have a check as its target. A check scores the share of items that pass
      3 | passing = { target = "pass" }
        |                      ^
    "#);

    insta::assert_snapshot!(error("[footprint]\nrun = \"true\"\nsize = { target = [\"5kB\"] }\n"), @r#"
    [invalid-type] retros.toml:3:19: `target` must be a value such as "5kB", "20ms", or a number
      3 | size = { target = ["5kB"] }
        |                   ^
    "#);

    insta::assert_snapshot!(error("[footprint]\nrun = \"true\"\nsize = { targets = \"5kB\" }\n"), @r#"
    [invalid-type] retros.toml:3:20: `targets` must be a table of item patterns and their targets, such as `targets = { "viem/chains/*" = "1kB" }`
      3 | size = { targets = "5kB" }
        |                    ^
    "#);

    insta::assert_snapshot!(error("[footprint]\nrun = \"true\"\nsize = { targets = { \"viem/*\" = 0 } }\n"), @r#"
    [target-range] retros.toml:3:33: the `target` of metric `size` must be greater than zero
      3 | size = { targets = { "viem/*" = 0 } }
        |                                 ^
    "#);
}
