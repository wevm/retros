//! Formats values for messages and reports. Every number that reaches output passes through here.

use std::time::Duration;

use crate::value::{Kind, Value};

/// Size units for display, in bytes.
const SIZES: [(&str, f64); 4] = [("B", 1.0), ("kB", 1e3), ("MB", 1e6), ("GB", 1e9)];

/// Time units for display, in nanoseconds.
const TIMES: [(&str, f64); 4] = [("ns", 1.0), ("µs", 1e3), ("ms", 1e6), ("s", 1e9)];

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
    match kind {
        Kind::Check | Kind::Count => decimal(amount, 2),
        Kind::Size => scaled(amount, &SIZES),
        Kind::Time => scaled(amount, &TIMES),
    }
}

/// Formats a value for display, such as `61 kB` or `pass`.
pub fn value(value: Value) -> String {
    match value {
        Value::Check(true) => "pass".to_owned(),
        Value::Check(false) => "fail".to_owned(),
        Value::Count(n) => amount(Kind::Count, n),
        Value::Size(n) => amount(Kind::Size, n),
        Value::Time(n) => amount(Kind::Time, n),
    }
}

fn scaled(amount: f64, units: &[(&str, f64)]) -> String {
    let mut index = units
        .iter()
        .rposition(|(_, factor)| amount.abs() >= *factor)
        .unwrap_or_default();

    // Rounding can carry into the next unit, such as 999.96 kB, which shows as 1 MB.
    if let (Some((_, factor)), Some(_)) = (units.get(index), units.get(index + 1))
        && (amount.abs() / factor * 10.0).round() >= 10_000.0
    {
        index += 1;
    }

    let (unit, factor) = units.get(index).copied().unwrap_or(("", 1.0));
    format!("{} {unit}", decimal(amount / factor, 1))
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
