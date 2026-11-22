//! Integration tests for scoring: the curve against values from Lighthouse's JavaScript and
//! zepto's Zig port, list roll-ups, medians across rounds, and category means.

use std::fmt::Write;

use engine::score::{self, CategoryScore};
use engine::{config, lines};

/// Scores `[category]` of `config` from the metric lines of each round.
fn scored(config: &str, rounds: &[&str]) -> CategoryScore {
    let config = config::parse(config).unwrap();
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
        let actual = score::curve(target, value);
        assert!(
            (actual - expected).abs() < 1e-12,
            "curve({target}, {value}) = {actual}, expected {expected}"
        );
    }
}

#[test]
fn scores_values_lists_checks_and_unscored_metrics() {
    let config = r#"[footprint]
run = "true"
bundle = { target = "5kB" }
export-size = { target = "5kB" }
modules = {}
links = {}
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

    // Against a 5 kB target, Lighthouse scores 27.5 kB as 3 and the p90, 9 kB, as 58. Two of
    // three links pass, 67. The mean of 3, 58, and 67 is 43.
    insta::assert_snapshot!(summary(&scored), @"
    category Some(43)
    bundle 27500 Some(3)
    export-size 9000 Some(58)
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
    let config = r#"[sizes]
run = "true"
p90 = {}
p10 = { better = "higher" }
median = { score = "median" }
max = { score = "max" }
min = { score = "min" }
sum = { score = "sum" }
"#;
    let list: String = [7, 2, 9, 4, 1, 10, 3, 8, 5, 6]
        .iter()
        .flat_map(|n| {
            ["p90", "p10", "median", "max", "min", "sum"]
                .map(|metric| format!("{metric} item-{n} {n}\n"))
        })
        .collect();

    let scored = scored(config, &[&list]);

    // Ten items from 1 to 10: the p90 has one item above it and the p10 one item below it.
    insta::assert_snapshot!(summary(&scored), @"
    category None
    p90 9 None
    p10 2 None
    median 5.5 None
    max 10 None
    min 1 None
    sum 55 None
    ");
}

#[test]
fn takes_timing_medians_across_rounds_and_exact_values_from_the_first() {
    let config = r#"[speed]
run = "true"
render = { target = "2ms" }
throughput = { target = 2000, better = "higher", weight = 2 }
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
    // Lighthouse scores 34 against a 2 ms target. Throughput keeps the first round's 2588, which
    // scores 96 against a target of 2000. With weight 2 on throughput, the category is 75.
    assert_eq!(scored.rounds, 5);
    insta::assert_snapshot!(summary(&scored), @"
    category Some(75)
    render 5000000 Some(34)
    throughput 2588 Some(96)
    ");
}

#[test]
fn takes_the_mean_of_two_middle_rounds() {
    let config = "[speed]\nrun = \"true\"\nboot = {}\n";
    let rounds = ["boot 4ms\n", "boot 1ms\n", "boot 10ms\n", "boot 2ms\n"];

    let scored = scored(config, &rounds);

    insta::assert_snapshot!(summary(&scored), @"
    category None
    boot 3000000 None
    ");
}

#[test]
fn leaves_weightless_and_unanchored_metrics_out_of_the_category() {
    let config = r#"[footprint]
run = "true"
bundle = { target = "5kB", weight = 0 }
modules = {}
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
    let config = "[api]\nrun = \"true\"\nthroughput = { target = 2000, better = \"higher\" }\n";

    let scored = scored(config, &["throughput 0\n"]);

    insta::assert_snapshot!(summary(&scored), @"
    category Some(0)
    throughput 0 Some(0)
    ");
}
