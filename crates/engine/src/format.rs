//! Formats values for messages and reports. Every number that reaches output passes through here.

use std::time::Duration;

use crate::value::{Kind, Value};

/// Size units for display, in bytes.
pub(crate) const SIZES: [(&str, f64); 4] = [("B", 1.0), ("kB", 1e3), ("MB", 1e6), ("GB", 1e9)];

/// Time units for display, in nanoseconds.
pub(crate) const TIMES: [(&str, f64); 4] = [("ns", 1.0), ("µs", 1e3), ("ms", 1e6), ("s", 1e9)];

/// Formats a duration in the largest unit that shows it exactly, such as `300ms`, `30s`, or `10m`.
pub fn duration(duration: Duration) -> String {
    let millis = duration.as_millis();

    match millis {
        0..1_000 => format!("{millis}ms"),
        _ if millis.is_multiple_of(3_600_000) => format!("{}h", millis / 3_600_000),
        _ if millis.is_multiple_of(60_000) => format!("{}m", millis / 60_000),
        _ if millis.is_multiple_of(1_000) => format!("{}s", millis / 1_000),
        _ => {
            let fraction = format!("{:03}", millis % 1_000);
            format!("{}.{}s", millis / 1_000, fraction.trim_end_matches('0'))
        }
    }
}

/// Formats an amount in base units for display, such as `593 kB`, `1.8 ms`, or `2588`. Sizes and
/// times use the largest unit that keeps the number at 1 or more, with one decimal place.
pub fn amount(kind: Kind, amount: f64) -> String {
    precise(kind, amount, 0)
}

/// Formats a value for display, such as `61 kB`, `pass`, or `skip`.
pub fn value(value: Value) -> String {
    match value {
        Value::Check(true) => "pass".to_owned(),
        Value::Check(false) => "fail".to_owned(),
        Value::Skip => "skip".to_owned(),
        Value::Count(n) => amount(Kind::Count, n),
        Value::Size(n) => amount(Kind::Size, n),
        Value::Time(n) => amount(Kind::Time, n),
    }
}

/// Formats two values of a change so that they read differently when they differ, with up to three
/// more decimal places than [`value`], such as `2.2 kB` and `2.18 kB` instead of `2.2 kB` twice, or
/// else in base units.
pub fn pair(before: Value, after: Value) -> (String, String) {
    let shown = (value(before), value(after));
    if let (Some(first), Some(second), true) = (before.amount(), after.amount(), shown.0 == shown.1)
    {
        for extra in 1..=3 {
            let more = (
                precise(before.kind(), first, extra),
                precise(after.kind(), second, extra),
            );
            if more.0 != more.1 {
                return more;
            }
        }
        // A tiny change to a large value shows in base units, such as `1000001 B`.
        return (base(before.kind(), first), base(after.kind(), second));
    }
    shown
}

/// Formats an amount in its base unit, such as `1000001 B` or `2.5 ns`.
fn base(kind: Kind, amount: f64) -> String {
    match kind {
        Kind::Check | Kind::Count => decimal(amount, 6),
        Kind::Size => format!("{} B", decimal(amount, 3)),
        Kind::Time => format!("{} ns", decimal(amount, 3)),
    }
}

/// Formats an amount like [`amount`], with `extra` more decimal places.
fn precise(kind: Kind, amount: f64, extra: usize) -> String {
    match kind {
        Kind::Check | Kind::Count => decimal(amount, 2 + extra),
        Kind::Size => scaled(amount, &SIZES, 1 + extra),
        Kind::Time => scaled(amount, &TIMES, 1 + extra),
    }
}

fn scaled(amount: f64, units: &[(&str, f64)], places: usize) -> String {
    let mut index = units
        .iter()
        .rposition(|(_, factor)| amount.abs() >= *factor)
        .unwrap_or_default();

    // Rounding can carry into the next unit, such as 999.96 kB, which shows as 1 MB.
    let scale: f64 = (0..places).fold(1.0, |scale, _| scale * 10.0);
    if let (Some((_, factor)), Some(_)) = (units.get(index), units.get(index + 1))
        && (amount.abs() / factor * scale).round() >= 1000.0 * scale
    {
        index += 1;
    }

    let (unit, factor) = units.get(index).copied().unwrap_or(("", 1.0));
    format!("{} {unit}", decimal(amount / factor, places))
}

/// Formats a number with at most `places` decimal places and no trailing zeros.
fn decimal(amount: f64, places: usize) -> String {
    let text = format!("{amount:.places$}");

    if text.contains('.') {
        let trimmed = text.trim_end_matches('0').trim_end_matches('.');
        if trimmed == "-0" { "0" } else { trimmed }.to_owned()
    } else {
        text
    }
}
