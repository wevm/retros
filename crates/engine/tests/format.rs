//! Integration tests for formatting values in reports.

use engine::format;
use engine::value::{Kind, Value};

#[test]
fn formats_sizes_times_and_counts() {
    let sizes: Vec<String> = [950.0, 1100.0, 27_500.0, 593_040.0, 999_960.0, 45_600_000.0]
        .map(|bytes| format::amount(Kind::Size, bytes))
        .to_vec();
    let times: Vec<String> = [850.0, 236_000.0, 456_000.0, 1_798_000.0, 2.5e9]
        .map(|nanos| format::amount(Kind::Time, nanos))
        .to_vec();
    let counts: Vec<String> = [17.0, 2588.0, 0.25, 1.0 / 3.0]
        .map(|n| format::amount(Kind::Count, n))
        .to_vec();

    insta::assert_debug_snapshot!(sizes, @r#"
    [
        "950 B",
        "1.1 kB",
        "27.5 kB",
        "593 kB",
        "1 MB",
        "45.6 MB",
    ]
    "#);
    insta::assert_debug_snapshot!(times, @r#"
    [
        "850 ns",
        "236 µs",
        "456 µs",
        "1.8 ms",
        "2.5 s",
    ]
    "#);
    insta::assert_debug_snapshot!(counts, @r#"
    [
        "17",
        "2588",
        "0.25",
        "0.33",
    ]
    "#);
}

#[test]
fn pairs_read_differently_when_their_values_differ() {
    let pairs = [
        (Value::Size(2200.0), Value::Size(2179.0)),
        (Value::Size(27_500.0), Value::Size(30_000.0)),
        (Value::Time(1_000_000.0), Value::Time(1_000_400.0)),
        (Value::Count(84.6), Value::Count(84.604)),
        (Value::Size(1_000_000.0), Value::Size(1_000_001.0)),
        (Value::Size(2200.0), Value::Size(2200.0)),
    ]
    .map(|(before, after)| format::pair(before, after));

    insta::assert_debug_snapshot!(pairs, @r#"
    [
        (
            "2.2 kB",
            "2.18 kB",
        ),
        (
            "27.5 kB",
            "30 kB",
        ),
        (
            "1 ms",
            "1.0004 ms",
        ),
        (
            "84.6",
            "84.604",
        ),
        (
            "1000000 B",
            "1000001 B",
        ),
        (
            "2200 B",
            "2200 B",
        ),
    ]
    "#);
}
