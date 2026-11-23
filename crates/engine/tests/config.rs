//! Integration tests for parsing `retros.yaml` and `retros.toml`: typed categories with defaults, and every
//! configuration error rendered with its file, line, and column.

use engine::config::{self, Config, Format};
use engine::error::Location;

/// Parses `source` as TOML and renders the error it must produce, prefixed with its code.
fn toml_error(source: &str) -> String {
    let error =
        config::parse(source, Format::Toml).expect_err("the configuration should be rejected");
    format!(
        "[{}] {}",
        error.code.as_str(),
        error.render("retros.toml", source)
    )
}

/// Returns `config` with every location at line 0, column 0, to compare one configuration
/// written in both formats.
fn without_locations(mut config: Config) -> Config {
    let nowhere = Location { column: 0, line: 0 };
    for category in &mut config.categories {
        category.location = nowhere;
        for metric in &mut category.metrics {
            metric.location = nowhere;
            for pattern in &mut metric.targets {
                pattern.location = nowhere;
            }
        }
    }
    config
}

/// Parses `source` and renders the error it must produce, prefixed with its code.
fn error(source: &str) -> String {
    let error =
        config::parse(source, Format::Yaml).expect_err("the configuration should be rejected");
    format!(
        "[{}] {}",
        error.code.as_str(),
        error.render("retros.yaml", source)
    )
}

#[test]
fn parses_categories_in_order_with_defaults() {
    let source = r#"
speed:
  run: node metrics/speed.mjs
  timeout: 1.5s
  throughput: { target: 2000, better: higher, weight: 2 }
  decode: { score: median }

footprint:
  run: node metrics/footprint.mjs
  export-size: { target: 1.1kB, targets: { viem: 60kB, "viem/chains/*": 1kB } }
  bundle: { target: 45.6MB }
  modules:
"#;

    let config = config::parse(source, Format::Yaml).unwrap();

    insta::assert_debug_snapshot!(config, @r#"
    Config {
        categories: [
            Category {
                description: None,
                location: Location {
                    column: 1,
                    line: 2,
                },
                metrics: [
                    Metric {
                        better: Higher,
                        description: None,
                        location: Location {
                            column: 3,
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
                        title: None,
                        varies: false,
                        weight: 2.0,
                    },
                    Metric {
                        better: Lower,
                        description: None,
                        location: Location {
                            column: 3,
                            line: 6,
                        },
                        name: "decode",
                        score: Median,
                        target: None,
                        targets: [],
                        title: None,
                        varies: false,
                        weight: 1.0,
                    },
                ],
                name: "speed",
                run: "node metrics/speed.mjs",
                timeout: 1.5s,
                title: None,
            },
            Category {
                description: None,
                location: Location {
                    column: 1,
                    line: 8,
                },
                metrics: [
                    Metric {
                        better: Lower,
                        description: None,
                        location: Location {
                            column: 3,
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
                                    column: 44,
                                    line: 10,
                                },
                                pattern: "viem",
                                target: Size(
                                    60000.0,
                                ),
                            },
                            Pattern {
                                location: Location {
                                    column: 56,
                                    line: 10,
                                },
                                pattern: "viem/chains/*",
                                target: Size(
                                    1000.0,
                                ),
                            },
                        ],
                        title: None,
                        varies: false,
                        weight: 1.0,
                    },
                    Metric {
                        better: Lower,
                        description: None,
                        location: Location {
                            column: 3,
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
                        title: None,
                        varies: false,
                        weight: 1.0,
                    },
                    Metric {
                        better: Lower,
                        description: None,
                        location: Location {
                            column: 3,
                            line: 12,
                        },
                        name: "modules",
                        score: P90,
                        target: None,
                        targets: [],
                        title: None,
                        varies: false,
                        weight: 1.0,
                    },
                ],
                name: "footprint",
                run: "node metrics/footprint.mjs",
                timeout: 600s,
                title: None,
            },
        ],
    }
    "#);
}

#[test]
fn renders_timeouts_in_their_largest_exact_unit() {
    let source = r#"
a:
  run: "true"
  timeout: 300ms
  m: {}
b:
  run: "true"
  timeout: 90s
  n: {}
c:
  run: "true"
  o: {}
d:
  run: "true"
  timeout: 2h
  p: {}
"#;

    let config = config::parse(source, Format::Yaml).unwrap();
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
fn rejects_invalid_yaml_and_structure() {
    insta::assert_snapshot!(error("footprint:\n  run: [true\n"), @"[yaml] retros.yaml:3:1: while parsing a flow sequence, expected ',' or ']'");

    insta::assert_snapshot!(error("footprint:\n  run: \"true\"\n  m: {}\n  m: {}\n"), @"
    [yaml] retros.yaml:4:3: duplicate key `m`
      4 |   m: {}
        |   ^
    ");

    insta::assert_snapshot!(error("# nothing yet\n"), @"[nothing-configured] retros.yaml: no categories. Add one such as `footprint:` with a `run` command and a metric");

    insta::assert_snapshot!(error("- footprint\n"), @"
    [invalid-type] retros.yaml:1:3: retros.yaml must map category names to categories, such as `footprint:`
      1 | - footprint
        |   ^
    ");

    insta::assert_snapshot!(error("size: 5kB\nfootprint:\n  run: \"true\"\n  m: {}\n"), @"
    [invalid-type] retros.yaml:1:1: top-level key `size` must be a category: a mapping with a `run` command and metrics
      1 | size: 5kB
        | ^
    ");

    insta::assert_snapshot!(error("footprint:\n  export-size: {}\n"), @"
    [run-missing] retros.yaml:1:1: category `footprint` has no `run` command. Add one, or a top-level `run` that every category shares
      1 | footprint:
        | ^
    ");

    insta::assert_snapshot!(error("footprint:\n  run: \"  \"\n  export-size: {}\n"), @r#"
    [invalid-value] retros.yaml:2:8: `run` must be a command
      2 |   run: "  "
        |        ^
    "#);

    insta::assert_snapshot!(error("footprint:\n  run: \"true\"\n"), @"
    [nothing-configured] retros.yaml:1:1: category `footprint` declares no metrics
      1 | footprint:
        | ^
    ");

    insta::assert_snapshot!(error("foot print:\n  run: \"true\"\n  m: {}\n"), @"
    [invalid-name] retros.yaml:1:1: invalid category name `foot print`. Names use ASCII letters, digits, `-`, `_`, and `.`
      1 | foot print:
        | ^
    ");

    insta::assert_snapshot!(error("footprint:\n  run: \"true\"\n  export-size: 5kB\n"), @"
    [invalid-type] retros.yaml:3:16: metric `export-size` must be a mapping, such as `export-size: { target: 5kB }` or `export-size: {}`
      3 |   export-size: 5kB
        |                ^
    ");
}

#[test]
fn rejects_invalid_category_settings() {
    insta::assert_snapshot!(error("speed:\n  run: \"true\"\n  timeout: 10 minutes\n  m: {}\n"), @"
    [invalid-value] retros.yaml:3:12: invalid `timeout` `10 minutes`, expected a duration such as `30s`, `10m`, or `500ms`
      3 |   timeout: 10 minutes
        |            ^
    ");

    insta::assert_snapshot!(error("speed:\n  run: \"true\"\n  timeout: [10m]\n  m: {}\n"), @"
    [invalid-type] retros.yaml:3:13: `timeout` must be a string
      3 |   timeout: [10m]
        |             ^
    ");
}

#[test]
fn rejects_invalid_metric_settings() {
    insta::assert_snapshot!(error("footprint:\n  run: \"true\"\n  size: { target: 5kB, goal: 1 }\n"), @"
    [unexpected-key] retros.yaml:3:24: unknown key `goal` in metric `size`, expected one of `better`, `description`, `score`, `target`, `targets`, `title`, `varies`, `weight`
      3 |   size: { target: 5kB, goal: 1 }
        |                        ^
    ");

    insta::assert_snapshot!(error("footprint:\n  run: \"true\"\n  size: { target: 5kB, limit: 50kB }\n"), @"
    [unexpected-key] retros.yaml:3:24: unknown key `limit` in metric `size`, expected one of `better`, `description`, `score`, `target`, `targets`, `title`, `varies`, `weight`
      3 |   size: { target: 5kB, limit: 50kB }
        |                        ^
    ");

    insta::assert_snapshot!(error("footprint:\n  run: \"true\"\n  size: { better: smaller }\n"), @"
    [invalid-value] retros.yaml:3:19: unknown `better` value `smaller`, expected `lower` or `higher`
      3 |   size: { better: smaller }
        |                   ^
    ");

    insta::assert_snapshot!(error("footprint:\n  run: \"true\"\n  size: { score: p95 }\n"), @"
    [invalid-value] retros.yaml:3:18: unknown `score` value `p95`, expected one of `p90`, `p10`, `median`, `max`, `min`, `sum`
      3 |   size: { score: p95 }
        |                  ^
    ");

    insta::assert_snapshot!(error("footprint:\n  run: \"true\"\n  size: { weight: -1 }\n"), @"
    [invalid-value] retros.yaml:3:19: `weight` cannot be negative
      3 |   size: { weight: -1 }
        |                   ^
    ");

    insta::assert_snapshot!(error("footprint:\n  run: \"true\"\n  size: { weight: 0x10 }\n"), @"
    [invalid-type] retros.yaml:3:19: `weight` must be a finite number
      3 |   size: { weight: 0x10 }
        |                   ^
    ");
}

#[test]
fn rejects_targets_that_cannot_score() {
    insta::assert_snapshot!(error("footprint:\n  run: \"true\"\n  size: { target: \"5kb\" }\n"), @r#"
    [unknown-unit] retros.yaml:3:21: unknown unit `kb`, expected one of `B`, `kB`, `MB`, `GB`, `ns`, `µs`, `us`, `ms`, `s`
      3 |   size: { target: "5kb" }
        |                     ^
    "#);

    // The column counts characters, so the é before it must not shift the caret.
    insta::assert_snapshot!(error("# é\nfootprint:\n  run: \"true\"\n  size: { target: 5kb }\n"), @"
    [unknown-unit] retros.yaml:4:20: unknown unit `kb`, expected one of `B`, `kB`, `MB`, `GB`, `ns`, `µs`, `us`, `ms`, `s`
      4 |   size: { target: 5kb }
        |                    ^
    ");

    insta::assert_snapshot!(error("speed:\n  run: \"true\"\n  errors: { target: 0 }\n"), @"
    [target-range] retros.yaml:3:21: the `target` of metric `errors` must be greater than zero
      3 |   errors: { target: 0 }
        |                     ^
    ");

    insta::assert_snapshot!(error("footprint:\n  run: \"true\"\n  size: { target: -5kB }\n"), @"
    [invalid-number] retros.yaml:3:19: sizes cannot be negative
      3 |   size: { target: -5kB }
        |                   ^
    ");

    insta::assert_snapshot!(error("tests:\n  run: \"true\"\n  passing: { target: pass }\n"), @"
    [target-unit] retros.yaml:3:22: metric `passing` cannot have a check as its target. A check scores the share of items that pass
      3 |   passing: { target: pass }
        |                      ^
    ");

    insta::assert_snapshot!(error("footprint:\n  run: \"true\"\n  size: { target: [5kB] }\n"), @"
    [invalid-type] retros.yaml:3:20: `target` must be a value such as `5kB`, `20ms`, or a number
      3 |   size: { target: [5kB] }
        |                    ^
    ");

    insta::assert_snapshot!(error("footprint:\n  run: \"true\"\n  size: { targets: 5kB }\n"), @r#"
    [invalid-type] retros.yaml:3:20: `targets` must map item patterns to targets, such as `targets: { "viem/chains/*": 1kB }`
      3 |   size: { targets: 5kB }
        |                    ^
    "#);

    insta::assert_snapshot!(error("footprint:\n  run: \"true\"\n  size: { targets: { \"viem/*\": 0 } }\n"), @r#"
    [target-range] retros.yaml:3:32: the `target` of metric `size` must be greater than zero
      3 |   size: { targets: { "viem/*": 0 } }
        |                                ^
    "#);
}

#[test]
fn shares_a_top_level_run_and_rejects_a_metric_in_two_sharing_categories() {
    let shared = config::parse(
        "run: zig build metrics\ntimeout: 2m\n\nspeed:\n  startup: { description: Spawn to exit }\n\nfootprint:\n  run: du -b zc\n  turn-rss: { varies: true }\n",
        Format::Yaml,
    )
    .unwrap();
    let summary: Vec<String> = shared
        .categories
        .iter()
        .map(|category| {
            let metric = category.metrics.first().unwrap();
            format!(
                "{} `{}` {} {:?} varies={}",
                category.name,
                category.run,
                engine::format::duration(category.timeout),
                metric.description,
                metric.varies
            )
        })
        .collect();
    insta::assert_debug_snapshot!(summary, @r#"
    [
        "speed `zig build metrics` 2m Some(\"Spawn to exit\") varies=false",
        "footprint `du -b zc` 2m None varies=true",
    ]
    "#);

    insta::assert_snapshot!(error("run: zig build metrics\n\nspeed:\n  startup: {}\n\nscale:\n  startup: {}\n"), @"
    [duplicate-metric] retros.yaml:7:3: metric `startup` is in `speed` and `scale`, which share the same `run`, so Retros cannot tell which category its lines belong to
      7 |   startup: {}
        |   ^
    ");
    insta::assert_snapshot!(error("footprint:\n  run: \"true\"\n  turn-rss: { varies: yes }\n"), @"
    [invalid-type] retros.yaml:3:23: `varies` must be `true` or `false`
      3 |   turn-rss: { varies: yes }
        |                       ^
    ");
}

#[test]
fn parses_toml_into_the_same_configuration_as_yaml() {
    let yaml = r#"
run: sh metrics.sh
timeout: 2m

speed:
  description: Time per call.
  startup: { target: 30ms, weight: 1.5, description: "Spawn to exit." }
  throughput: { target: 2000, better: higher, score: median }

footprint:
  run: sh footprint.sh
  timeout: 90s
  entry-size: { target: 1kB, targets: { ".": 3kB, "./chains/*": 500B } }
  peak-memory: { target: 16MB, varies: true }
  modules: {}
"#;
    let toml = r#"
run = "sh metrics.sh"
timeout = "2m"

[speed]
description = "Time per call."
startup = { target = "30ms", weight = 1.5, description = "Spawn to exit." }
throughput = { target = 2_000, better = "higher", score = "median" }

[footprint]
run = "sh footprint.sh"
timeout = "90s"
entry-size = { target = "1kB", targets = { "." = "3kB", "./chains/*" = "500B" } }
peak-memory = { target = "16MB", varies = true }
modules = {}
"#;

    assert_eq!(
        without_locations(config::parse(toml, Format::Toml).unwrap()),
        without_locations(config::parse(yaml, Format::Yaml).unwrap()),
    );
}

#[test]
fn rejects_invalid_toml_with_toml_examples() {
    insta::assert_snapshot!(toml_error("[footprint\nrun = \"true\"\n"), @"
    [toml] retros.toml:1:11: unclosed table, expected `]`
      1 | [footprint
        |           ^
    ");

    insta::assert_snapshot!(toml_error("# nothing yet\n"), @"[nothing-configured] retros.toml: no categories. Add a table such as `[footprint]` with a `run` command");

    insta::assert_snapshot!(toml_error("size = \"5kB\"\n[footprint]\nrun = \"true\"\nm = {}\n"), @r#"
    [invalid-type] retros.toml:1:1: top-level key `size` must be a category table, such as `[size]`
      1 | size = "5kB"
        | ^
    "#);

    insta::assert_snapshot!(toml_error("[footprint]\nrun = \"true\"\nexport-size = \"5kB\"\n"), @r#"
    [invalid-type] retros.toml:3:15: metric `export-size` must be a table, such as `export-size = { target = "5kB" }` or `export-size = {}`
      3 | export-size = "5kB"
        |               ^
    "#);

    insta::assert_snapshot!(toml_error("[footprint]\nrun = \"true\"\nsize = { target = \"5kb\" }\n"), @r#"
    [unknown-unit] retros.toml:3:21: unknown unit `kb`, expected one of `B`, `kB`, `MB`, `GB`, `ns`, `µs`, `us`, `ms`, `s`
      3 | size = { target = "5kb" }
        |                     ^
    "#);

    insta::assert_snapshot!(toml_error("[footprint]\nrun = \"true\"\nsize = { weight = 0x10 }\n"), @"
    [invalid-type] retros.toml:3:19: `weight` must be a finite number
      3 | size = { weight = 0x10 }
        |                   ^
    ");

    insta::assert_snapshot!(toml_error("[footprint]\nrun = \"true\"\nsize = { targets = \"5kB\" }\n"), @r#"
    [invalid-type] retros.toml:3:20: `targets` must be a table of item patterns and their targets, such as `targets = { "viem/chains/*" = "1kB" }`
      3 | size = { targets = "5kB" }
        |                    ^
    "#);
}
