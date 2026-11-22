//! Integration tests for formatting values in reports.

use engine::format;
use engine::value::Kind;

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
