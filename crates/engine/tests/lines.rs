//! Integration tests for parsing the metric lines that scripts write: items, quoting, units,
//! checks, and every line error rendered with its line and column.

use engine::lines;

/// Parses `source` and renders the error it must produce, prefixed with its code.
fn error(source: &str) -> String {
    let error = lines::parse(source).expect_err("the lines should be rejected");
    format!(
        "[{}] {}",
        error.code.as_str(),
        error.render("footprint metrics", source)
    )
}

#[test]
fn parses_values_in_base_units() {
    let source = "export-size 1.1kB\nbundle 45.6MB\nboot 236µs\nrequest 1.82ms\nidle 2s\nretries 3\nratio 0.25\nlint pass\n";

    let lines = lines::parse(source).unwrap();

    insta::assert_debug_snapshot!(lines, @r#"
    [
        Line {
            item: None,
            location: Location {
                column: 1,
                line: 1,
            },
            metric: "export-size",
            target: None,
            subject: None,
            value: Size(
                1100.0,
            ),
        },
        Line {
            item: None,
            location: Location {
                column: 1,
                line: 2,
            },
            metric: "bundle",
            target: None,
            subject: None,
            value: Size(
                45600000.0,
            ),
        },
        Line {
            item: None,
            location: Location {
                column: 1,
                line: 3,
            },
            metric: "boot",
            target: None,
            subject: None,
            value: Time(
                236000.0,
            ),
        },
        Line {
            item: None,
            location: Location {
                column: 1,
                line: 4,
            },
            metric: "request",
            target: None,
            subject: None,
            value: Time(
                1820000.0,
            ),
        },
        Line {
            item: None,
            location: Location {
                column: 1,
                line: 5,
            },
            metric: "idle",
            target: None,
            subject: None,
            value: Time(
                2000000000.0,
            ),
        },
        Line {
            item: None,
            location: Location {
                column: 1,
                line: 6,
            },
            metric: "retries",
            target: None,
            subject: None,
            value: Count(
                3.0,
            ),
        },
        Line {
            item: None,
            location: Location {
                column: 1,
                line: 7,
            },
            metric: "ratio",
            target: None,
            subject: None,
            value: Count(
                0.25,
            ),
        },
        Line {
            item: None,
            location: Location {
                column: 1,
                line: 8,
            },
            metric: "lint",
            target: None,
            subject: None,
            value: Check(
                true,
            ),
        },
    ]
    "#);
}

#[test]
fn parses_items_quotes_tabs_and_blank_lines() {
    let source = "render \"ABI Decode (500 results)\" 1.82ms\r\n\r\nrender\tencode\t236us\n   \nlint \"say \\\"hi\\\" \\\\ bye\" fail";

    let lines = lines::parse(source).unwrap();

    insta::assert_debug_snapshot!(lines, @r#"
    [
        Line {
            item: Some(
                "ABI Decode (500 results)",
            ),
            location: Location {
                column: 1,
                line: 1,
            },
            metric: "render",
            target: None,
            subject: None,
            value: Time(
                1820000.0,
            ),
        },
        Line {
            item: Some(
                "encode",
            ),
            location: Location {
                column: 1,
                line: 3,
            },
            metric: "render",
            target: None,
            subject: None,
            value: Time(
                236000.0,
            ),
        },
        Line {
            item: Some(
                "say \"hi\" \\ bye",
            ),
            location: Location {
                column: 1,
                line: 5,
            },
            metric: "lint",
            target: None,
            subject: None,
            value: Check(
                false,
            ),
        },
    ]
    "#);
}

#[test]
fn rejects_malformed_lines() {
    insta::assert_snapshot!(error("export-size\n"), @"
    [line-syntax] footprint metrics:1:12: expected a value after metric `export-size`
      1 | export-size
        |            ^
    ");

    insta::assert_snapshot!(error("render ABI Decode 1.82ms\n"), @"
    [line-syntax] footprint metrics:1:19: expected `metric [item] value [target <value>]`, found more fields. Quote an item that contains spaces
      1 | render ABI Decode 1.82ms
        |                   ^
    ");

    insta::assert_snapshot!(error("render \"ABI Decode 1.82ms\n"), @r#"
    [line-syntax] footprint metrics:1:8: unterminated or invalid quoted item. Escape only `"` and `\` with `\`
      1 | render "ABI Decode 1.82ms
        |        ^
    "#);

    insta::assert_snapshot!(error("render \"ABI\"Decode 1.82ms\n"), @r#"
    [line-syntax] footprint metrics:1:13: expected a space after the quoted item
      1 | render "ABI"Decode 1.82ms
        |             ^
    "#);

    insta::assert_snapshot!(error("render \"bad \\n escape\" 1.82ms\n"), @r#"
    [line-syntax] footprint metrics:1:8: unterminated or invalid quoted item. Escape only `"` and `\` with `\`
      1 | render "bad \n escape" 1.82ms
        |        ^
    "#);

    insta::assert_snapshot!(error("\"export size\" 5kB\n"), @r#"
    [invalid-name] footprint metrics:1:1: invalid metric name `export size`. Names use ASCII letters, digits, `-`, `_`, and `.`
      1 | "export size" 5kB
        | ^
    "#);

    insta::assert_snapshot!(error("-size 5kB\n"), @"
    [invalid-name] footprint metrics:1:1: invalid metric name `-size`. Names use ASCII letters, digits, `-`, `_`, and `.`
      1 | -size 5kB
        | ^
    ");
}

#[test]
fn rejects_invalid_values() {
    insta::assert_snapshot!(error("size 1kB\nboot \"µs item\" 5kb\n"), @r#"
    [unknown-unit] footprint metrics:2:17: unknown unit `kb`, expected one of `B`, `kB`, `MB`, `GB`, `ns`, `µs`, `us`, `ms`, `s`
      2 | boot "µs item" 5kb
        |                 ^
    "#);

    insta::assert_snapshot!(error("size -5kB\n"), @"
    [invalid-number] footprint metrics:1:6: sizes cannot be negative
      1 | size -5kB
        |      ^
    ");

    insta::assert_snapshot!(error("size five\n"), @"
    [invalid-number] footprint metrics:1:6: expected a number, a number with a unit, `pass`, or `fail`, found `five`
      1 | size five
        |      ^
    ");

    insta::assert_snapshot!(error("size 5.kB\n"), @"
    [invalid-number] footprint metrics:1:6: expected a number, a number with a unit, `pass`, or `fail`, found `5.kB`
      1 | size 5.kB
        |      ^
    ");

    insta::assert_snapshot!(error("boot 1e3ms\n"), @"
    [unknown-unit] footprint metrics:1:7: unknown unit `e3ms`, expected one of `B`, `kB`, `MB`, `GB`, `ns`, `µs`, `us`, `ms`, `s`
      1 | boot 1e3ms
        |       ^
    ");
}

#[test]
fn parses_line_targets() {
    let source = "entry-size \".\" 408B target 2kB\nentry-size ./date 247B\nboot 1.2ms target 1ms\nentry-size target 5kB target 6kB\n";

    let lines = lines::parse(source).unwrap();
    let targets: Vec<String> = lines
        .iter()
        .map(|line| {
            format!(
                "{} {:?} {:?} {:?}",
                line.metric, line.item, line.value, line.target
            )
        })
        .collect();

    insta::assert_debug_snapshot!(targets, @r#"
    [
        "entry-size Some(\".\") Size(408.0) Some(Size(2000.0))",
        "entry-size Some(\"./date\") Size(247.0) None",
        "boot None Time(1200000.0) Some(Time(1000000.0))",
        "entry-size Some(\"target\") Size(5000.0) Some(Size(6000.0))",
    ]
    "#);
}

#[test]
fn rejects_invalid_line_targets() {
    insta::assert_snapshot!(error("entry-size \".\" 408B target\n"), @r#"
    [line-syntax] footprint metrics:1:27: expected a value after `target`
      1 | entry-size "." 408B target
        |                           ^
    "#);

    insta::assert_snapshot!(error("entry-size \".\" 408B target 2ms\n"), @r#"
    [target-unit] footprint metrics:1:28: the target is a time, but the value is a size
      1 | entry-size "." 408B target 2ms
        |                            ^
    "#);

    insta::assert_snapshot!(error("entry-size \".\" 408B target 0B\n"), @r#"
    [target-range] footprint metrics:1:28: a target must be greater than zero
      1 | entry-size "." 408B target 0B
        |                            ^
    "#);

    insta::assert_snapshot!(error("lint index.html pass target pass\n"), @"
    [target-unit] footprint metrics:1:29: a check cannot be a target
      1 | lint index.html pass target pass
        |                             ^
    ");

    insta::assert_snapshot!(error("entry-size \".\" 408B target 2kb\n"), @r#"
    [unknown-unit] footprint metrics:1:29: unknown unit `kb`, expected one of `B`, `kB`, `MB`, `GB`, `ns`, `µs`, `us`, `ms`, `s`
      1 | entry-size "." 408B target 2kb
        |                             ^
    "#);
}

#[test]
fn parses_subjects() {
    let source = "zc: startup 0.76ms\nzeptocode-ts: turn-rss 48.1MB target 57MB\nzc: export-size \"a b\" 1kB\nmodules 412\n";

    let parsed: Vec<String> = lines::parse(source)
        .unwrap()
        .iter()
        .map(|line| {
            format!(
                "{:?} {} {:?} {:?} {:?}",
                line.subject, line.metric, line.item, line.value, line.target
            )
        })
        .collect();

    insta::assert_debug_snapshot!(parsed, @r#"
    [
        "Some(\"zc\") startup None Time(760000.0) None",
        "Some(\"zeptocode-ts\") turn-rss None Size(48100000.0) Some(Size(57000000.0))",
        "Some(\"zc\") export-size Some(\"a b\") Size(1000.0) None",
        "None modules None Count(412.0) None",
    ]
    "#);
}

#[test]
fn rejects_invalid_subjects() {
    insta::assert_snapshot!(error("zc:\n"), @"
    [line-syntax] footprint metrics:1:4: expected a metric after subject `zc:`
      1 | zc:
        |    ^
    ");
    insta::assert_snapshot!(error("zc:: startup 1ms\n"), @"
    [invalid-name] footprint metrics:1:1: invalid subject `zc::`. A subject is a name followed by `:`, such as `zc:`, and names use ASCII letters, digits, `-`, `_`, and `.`
      1 | zc:: startup 1ms
        | ^
    ");
    insta::assert_snapshot!(error("z c: startup 1ms\n"), @"
    [line-syntax] footprint metrics:1:14: expected `metric [item] value [target <value>]`, found more fields. Quote an item that contains spaces
      1 | z c: startup 1ms
        |              ^
    ");
}
