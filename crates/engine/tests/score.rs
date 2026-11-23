//! Integration tests for scoring: the curve against values from Lighthouse's JavaScript and
//! zepto's Zig port, list roll-ups, medians across rounds, and category means.

use std::fmt::Write;

use engine::config::{self, Format};
use engine::lines;
use engine::score::{self, CategoryScore};

/// Scores `[category]` of `config` from the metric lines of each round.
fn scored(config: &str, rounds: &[&str]) -> CategoryScore {
    let config = config::parse(config, Format::Yaml).unwrap();
    let category = config.categories.first().unwrap();
    let rounds: Vec<_> = rounds
        .iter()
        .map(|source| lines::parse(source).unwrap())
        .collect();

    score::category(category, &rounds)
}

/// Renders each metric as `name value score`, with the value in base units.
fn summary(scored: &CategoryScore) -> String {
    let mut out = format!("category {:?}", scored.score);
    for metric in &scored.metrics {
        let _ = write!(
            out,
            "\n{} {} {:?}",
            metric.metric.name, metric.value, metric.score
        );
    }
    out
}

#[test]
fn curve_matches_lighthouse_and_zepto() {
    // (target, value, score) from Lighthouse's `getLogNormalScore` in shared/statistics.js at
    // 896399bc, called as `{ p10: target, median: 2 * target }`. zepto's `bench/metrics.zig` at
    // c8a97f7 prints the same doubles for `score(target, 2 * target, value)`.
    let cases = [
        (5000.0, 0.0, 1.0),
        (5000.0, 1000.0, 0.999_989_643_866_174_4),
        (5000.0, 2500.0, 0.994_812_911_341_374_8),
        (5000.0, 5000.0, 0.9),
        (5000.0, 7500.0, 0.702_599_604_029_262_8),
        (5000.0, 10000.0, 0.5),
        (5000.0, 20000.0, 0.100_000_068_596_147_52),
        (5000.0, 27500.0, 0.030_718_462_957_005_088),
        (5000.0, 110_800.0, 4.360_941_909_142_824e-6),
        (5000.0, 227_700.0, 3.777_852_308_761_709e-9),
        (30.0, 31.0, 0.888_943_074_909_612_1),
        (30.0, 45.0, 0.702_599_604_029_262_8),
        (30.0, 60.0, 0.5),
        (30.0, 3000.0, 2.381_428_387_820_961e-13),
    ];

    for (target, value, expected) in cases {
        let actual = score::curve(target, 2.0 * target, value);
        assert!(
            (actual - expected).abs() < 1e-12,
            "curve({target}, {value}) = {actual}, expected {expected}"
        );
    }
}

#[test]
fn scores_metrics_like_lighthouse_audits_with_their_control_points() {
    // Lighthouse 13.6.0's mobile control points, with `poor` as each median.
    let config = r#"performance:
  run: "true"
  first-contentful-paint: { target: 1800ms, poor: 3000ms, weight: 10 }
  largest-contentful-paint: { target: 2500ms, poor: 4000ms, weight: 25 }
  total-blocking-time: { target: 200ms, poor: 600ms, weight: 30 }
  cumulative-layout-shift: { target: 0.1, poor: 0.25, weight: 25 }
  speed-index: { target: 3387ms, poor: 5800ms, weight: 10 }
"#;

    // Lighthouse's `Util.computeLogNormalScore` at edc407d7 scores these as 0.09, 0.87, 0.73, 1,
    // and 0.89: it rounds down, so 0.0988, 0.8775, and 0.8984 do not round up. Its
    // `ReportScoring.arithmeticMean` makes the category 0.78.
    let audits = scored(
        config,
        &["first-contentful-paint 5013.7ms
largest-contentful-paint 2611.4ms
total-blocking-time 350ms
cumulative-layout-shift 0.012
speed-index 3400.25ms
"],
    );
    insta::assert_snapshot!(summary(&audits), @"
    category Some(78)
    first-contentful-paint 5013700000 Some(9)
    largest-contentful-paint 2611400000 Some(87)
    total-blocking-time 350000000 Some(73)
    cumulative-layout-shift 0.012 Some(100)
    speed-index 3400250000 Some(89)
    ");

    // Scores of 0, 0, 3, 50, and 11 have a weighted mean of exactly 14.5 points, but Lighthouse
    // sums fractions of 1 and reports 14.
    let halfway = scored(
        config,
        &["first-contentful-paint 30s
largest-contentful-paint 40s
total-blocking-time 2700ms
cumulative-layout-shift 0.25
speed-index 9550ms
"],
    );
    insta::assert_snapshot!(summary(&halfway), @"
    category Some(14)
    first-contentful-paint 30000000000 Some(0)
    largest-contentful-paint 40000000000 Some(0)
    total-blocking-time 2700000000 Some(3)
    cumulative-layout-shift 0.25 Some(50)
    speed-index 9550000000 Some(11)
    ");
}

#[test]
fn scales_poor_with_each_items_own_target() {
    let config = r#"footprint:
  run: "true"
  entry-size: { target: 1kB, poor: 3kB, targets: { ".": 4kB } }
"#;
    // `poor` is three times the default target, so the root's 4 kB target makes 12 kB score 50,
    // and `./date`, at 3 kB, scores 50 against the default.
    let root = scored(config, &["entry-size . 12kB\nentry-size ./date 1kB\n"]);
    let date = scored(config, &["entry-size . 4kB\nentry-size ./date 3kB\n"]);

    insta::assert_snapshot!(summary(&root), @"
    category Some(50)
    entry-size 12000 Some(50)
    ");
    insta::assert_snapshot!(summary(&date), @"
    category Some(50)
    entry-size 3000 Some(50)
    ");
}

#[test]
fn checks_that_skip_have_no_score() {
    let config = r#"accessibility:
  run: "true"
  image-alt: { weight: 10 }
  video-caption: { weight: 10 }
  links: { weight: 3 }
"#;
    // A skipped check, like an audit Lighthouse marks not applicable, leaves the category, and a
    // skipped item counts toward neither side of a list.
    let lines = "image-alt fail
video-caption skip
links a pass
links b skip
links c pass
";

    let scored = scored(config, &[lines]);

    insta::assert_snapshot!(summary(&scored), @"
    category Some(23)
    image-alt 0 Some(0)
    video-caption 1 None
    links 1 Some(100)
    ");
}

#[test]
fn scores_values_lists_checks_and_unscored_metrics() {
    let config = r#"footprint:
  run: "true"
  bundle: { target: "5kB" }
  export-size: { target: "5kB" }
  modules: {}
  links: {}
"#;
    let lines = "bundle 27.5kB
export-size c 3kB
export-size j 10kB
export-size a 1kB
export-size h 8kB
export-size e 5kB
export-size b 2kB
export-size i 9kB
export-size d 4kB
export-size g 7kB
export-size f 6kB
modules 412
links index.html pass
links about.html fail
links blog.html pass
";

    let scored = scored(config, &[lines]);

    // Against a 5 kB target, Lighthouse scores 27.5 kB as 3 and the p90, 9 kB, as 57. Two of
    // three links pass, 67. The mean of 3, 57, and 67 is 42.
    insta::assert_snapshot!(summary(&scored), @"
    category Some(42)
    bundle 27500 Some(3)
    export-size 9000 Some(57)
    modules 412 None
    links 0.6666666666666666 Some(67)
    ");

    let items: Vec<String> = scored.metrics[1]
        .items
        .iter()
        .chain(&scored.metrics[3].items)
        .map(|item| format!("{} {:?}", item.name, item.value))
        .collect();
    insta::assert_debug_snapshot!(items, @r#"
    [
        "j Size(10000.0)",
        "i Size(9000.0)",
        "h Size(8000.0)",
        "g Size(7000.0)",
        "f Size(6000.0)",
        "e Size(5000.0)",
        "d Size(4000.0)",
        "c Size(3000.0)",
        "b Size(2000.0)",
        "a Size(1000.0)",
        "about.html Check(false)",
        "index.html Check(true)",
        "blog.html Check(true)",
    ]
    "#);
}

#[test]
fn collapses_lists_with_each_rollup() {
    let config = r#"sizes:
  run: "true"
  p90: {}
  p10: { better: "higher" }
  median: { score: "median" }
  max: { score: "max" }
  min: { score: "min" }
  sum: { score: "sum" }
"#;
    let list: String = [7, 2, 9, 4, 1, 10, 3, 8, 5, 6]
        .iter()
        .flat_map(|n| {
            ["p90", "p10", "median", "max", "min", "sum"]
                .map(|metric| format!("{metric} item-{n} {n}\n"))
        })
        .collect();

    let scored = scored(config, &[&list]);

    // Ten items from 1 to 10: the p90 has one item above it, the p10 one item below it, and the
    // median is the upper middle item, so each one is a value the script wrote.
    insta::assert_snapshot!(summary(&scored), @"
    category None
    p90 9 None
    p10 2 None
    median 6 None
    max 10 None
    min 1 None
    sum 55 None
    ");
}

#[test]
fn takes_timing_medians_across_rounds_and_exact_values_from_the_first() {
    let config = r#"speed:
  run: "true"
  render: { target: "2ms" }
  throughput: { target: 2000, better: "higher", weight: 2 }
"#;
    let rounds = [
        "render parse 9ms\nrender encode 2ms\nthroughput 2588\n",
        "render parse 1ms\nrender encode 2ms\nthroughput 100\n",
        "render parse 5ms\nrender encode 2ms\nthroughput 100\n",
        "render parse 3ms\nrender encode 2ms\nthroughput 100\n",
        "render parse 7ms\nrender encode 2ms\nthroughput 100\n",
    ];

    let scored = scored(config, &rounds);

    // The median of `parse` is 5 ms, and the p90 of two items is the larger, 5 ms, which
    // Lighthouse scores 33 against a 2 ms target. Throughput keeps the first round's 2588, which
    // scores 96 against a target of 2000. With weight 2 on throughput, the category is 75.
    assert_eq!(scored.rounds, 5);
    insta::assert_snapshot!(summary(&scored), @"
    category Some(75)
    render 5000000 Some(33)
    throughput 2588 Some(96)
    ");
}

#[test]
fn takes_the_mean_of_two_middle_rounds() {
    let config = "speed:\n  run: \"true\"\n  boot: {}\n";
    let rounds = ["boot 4ms\n", "boot 1ms\n", "boot 10ms\n", "boot 2ms\n"];

    let scored = scored(config, &rounds);

    insta::assert_snapshot!(summary(&scored), @"
    category None
    boot 3000000 None
    ");
}

#[test]
fn leaves_weightless_and_unanchored_metrics_out_of_the_category() {
    let config = r#"footprint:
  run: "true"
  bundle: { target: "5kB", weight: 0 }
  modules: {}
"#;

    let scored = scored(config, &["bundle 5kB\nmodules 412\n"]);

    insta::assert_snapshot!(summary(&scored), @"
    category None
    bundle 5000 Some(90)
    modules 412 None
    ");
}

#[test]
fn scores_zero_when_higher_is_better_and_the_value_is_zero() {
    let config = "api:\n  run: \"true\"\n  throughput: { target: 2000, better: \"higher\" }\n";

    let scored = scored(config, &["throughput 0\n"]);

    insta::assert_snapshot!(summary(&scored), @"
    category Some(0)
    throughput 0 Some(0)
    ");
}

#[test]
fn scores_each_item_against_its_own_target() {
    let config = r#"footprint:
  run: "true"
  entry-size: { target: "1kB" }
"#;
    // The root entry is 4 kB against its own 5 kB target, so `./number`, 1.6 times its 1 kB
    // default, is the item furthest past its target. Lighthouse scores 1.6 kB against 1 kB as 66.
    let lines = "entry-size . 4kB target 5kB
entry-size ./date 900B
entry-size ./number 1.6kB
";

    let scored = scored(config, &[lines]);
    let metric = &scored.metrics[0];

    insta::assert_snapshot!(summary(&scored), @"
    category Some(66)
    entry-size 1600 Some(66)
    ");
    insta::assert_debug_snapshot!((&metric.at, metric.target), @r#"
    (
        Some(
            "./number",
        ),
        Some(
            Size(
                1000.0,
            ),
        ),
    )
    "#);
}

#[test]
fn patterns_in_targets_have_the_final_say() {
    let config = r#"footprint:
  run: "true"
  entry-size: { target: "1kB", targets: { "./chains/*": "500B", ".": "8kB" } }
"#;
    // The pattern `.` overrides the root's line target, so the root is 6 kB against 8 kB, not
    // 4 kB. `./chains/*` matches `./chains/mainnet` but not `./chains`, which keeps the 1 kB
    // default. `./chains/mainnet`, 600 B against 500 B, is furthest past its target, and
    // Lighthouse scores it 82.
    let lines = "entry-size . 6kB target 4kB
entry-size ./chains/mainnet 600B
entry-size ./chains 900B
entry-size ./actions 1.1kB target 2kB
";

    let scored = scored(config, &[lines]);
    let metric = &scored.metrics[0];
    let targets: Vec<String> = metric
        .items
        .iter()
        .map(|item| format!("{} {:?}", item.name, item.target))
        .collect();

    insta::assert_snapshot!(summary(&scored), @"
    category Some(82)
    entry-size 600 Some(82)
    ");
    insta::assert_debug_snapshot!((&metric.at, targets), @r#"
    (
        Some(
            "./chains/mainnet",
        ),
        [
            "./chains/mainnet Some(Size(500.0))",
            "./chains None",
            ". Some(Size(8000.0))",
            "./actions Some(Size(2000.0))",
        ],
    )
    "#);
}

#[test]
fn finds_opportunities_largest_gain_first() {
    let config = r#"footprint:
  run: "true"
  bundle: { target: "5kB" }
  export-size: { target: "5kB" }
  links: {}
"#;
    let lines = "bundle 27.5kB
export-size a 9kB
export-size b 3kB
links index.html pass
links about.html fail
";

    let scored = scored(config, &[lines]);
    let opportunities: Vec<String> = score::opportunities(&scored)
        .iter()
        .map(|o| {
            format!(
                "{:.2} {} {:?} {:?} -> {:?} ({})",
                o.gain, o.metric, o.item, o.value, o.target, o.count
            )
        })
        .collect();

    // Category 37 is the mean of bundle 3, export-size 57 (9 kB, the p90 of two items), and links
    // 50. Meeting a target lifts a metric to 90, or a check list to 100: bundle adds 29,
    // `about.html` adds 16.67, and `a` adds 11.
    insta::assert_snapshot!(summary(&scored), @"
    category Some(37)
    bundle 27500 Some(3)
    export-size 9000 Some(57)
    links 0.5 Some(50)
    ");
    insta::assert_debug_snapshot!(opportunities, @r#"
    [
        "29.00 bundle None Size(27500.0) -> Size(5000.0) (1)",
        "16.67 links Some(\"about.html\") Check(false) -> Check(true) (1)",
        "11.00 export-size Some(\"a\") Size(9000.0) -> Size(5000.0) (1)",
    ]
    "#);
}

#[test]
fn groups_a_list_when_no_single_item_moves_its_score() {
    let config = "footprint:\n  run: \"true\"\n  export-size: { target: \"1kB\" }\n";
    let lines = (1..=20).fold(String::new(), |mut lines, n| {
        let _ = writeln!(lines, "export-size item-{n} 3kB");
        lines
    });

    // Twenty items at 3 kB score 22 against 1 kB. Fixing one leaves the p90 at 3 kB, so the only
    // opportunity is all twenty, which lifts the metric to 90: 68 points.
    let scored = scored(config, &[&lines]);
    let opportunities: Vec<String> = score::opportunities(&scored)
        .iter()
        .map(|o| {
            format!(
                "{:.2} {} {:?} {:?} -> {:?} ({})",
                o.gain, o.metric, o.item, o.value, o.target, o.count
            )
        })
        .collect();

    insta::assert_debug_snapshot!(opportunities, @r#"
    [
        "68.00 export-size None Size(3000.0) -> Size(1000.0) (20)",
    ]
    "#);
}

#[test]
fn scores_each_subject_and_takes_medians_of_metrics_that_vary() {
    let config = r#"footprint:
  run: "true"
  turn-rss: { target: "16MB", varies: true }
  install-size: { target: "10MB" }
"#;
    // zc's peak memory varies: 4, 2, and 3 MB have a median of 3 MB. Its install size is exact,
    // so the first round's 1 MB counts. ts writes no install size, so it has no score for it.
    let rounds: Vec<_> = [
        "zc: turn-rss 4MB\nzc: install-size 1MB\nts: turn-rss 64MB\n",
        "zc: turn-rss 2MB\nzc: install-size 9MB\nts: turn-rss 8MB\n",
        "zc: turn-rss 3MB\nzc: install-size 9MB\nts: turn-rss 40MB\n",
    ]
    .iter()
    .map(|source| lines::parse(source).unwrap())
    .collect();
    let config = config::parse(config, Format::Yaml).unwrap();

    let subjects = score::subjects(config.categories.first().unwrap(), &rounds);

    let rendered: Vec<String> = subjects
        .iter()
        .map(|(subject, scored)| format!("{subject:?}\n{}", summary(scored)))
        .collect();
    insta::assert_snapshot!(rendered.join("\n"), @r#"
    Some("zc")
    category Some(100)
    turn-rss 3000000 Some(100)
    install-size 1000000 Some(100)
    Some("ts")
    category Some(33)
    turn-rss 40000000 Some(33)
    "#);
}
