//! Formats values for messages and reports. Every number that reaches output passes through here.

use std::time::Duration;

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
