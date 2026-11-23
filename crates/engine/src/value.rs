//! Measured values and their units.
//!
//! A value is a number with an optional unit, such as `45.6MB`, `236ms`, or `54774`, or a check
//! result, `pass`, `fail`, or `skip`. Parsing converts sizes to bytes and times to nanoseconds,
//! and size units are decimal, so `1kB` is 1,000 bytes.

use crate::error::Code;

/// Size units and their factors in bytes, in the order error messages list them.
const SIZES: [(&str, f64); 4] = [("B", 1.0), ("kB", 1e3), ("MB", 1e6), ("GB", 1e9)];

/// Time units and their factors in nanoseconds, in the order error messages list them.
const TIMES: [(&str, f64); 5] = [
    ("ns", 1.0),
    ("µs", 1e3),
    ("us", 1e3),
    ("ms", 1e6),
    ("s", 1e9),
];

/// The kind of quantity a value measures. A metric measures one kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    /// A check that passes or fails.
    Check,
    /// A plain number.
    Count,
    /// A size in bytes.
    Size,
    /// A duration in nanoseconds.
    Time,
}

impl Kind {
    /// Returns the singular noun with its article that messages use for the kind, such as
    /// `a size`.
    pub fn singular(self) -> &'static str {
        match self {
            Self::Check => "a check",
            Self::Count => "a count",
            Self::Size => "a size",
            Self::Time => "a time",
        }
    }

    /// Returns the plural noun that messages use for the kind, such as `sizes`.
    pub fn plural(self) -> &'static str {
        match self {
            Self::Check => "checks",
            Self::Count => "counts",
            Self::Size => "sizes",
            Self::Time => "times",
        }
    }
}

/// A measured value in its base unit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Value {
    /// A check result.
    Check(bool),
    /// A check that does not apply, such as an audit with nothing on the page to audit. It has no
    /// score and never misses.
    Skip,
    /// A plain number.
    Count(f64),
    /// A size in bytes.
    Size(f64),
    /// A duration in nanoseconds.
    Time(f64),
}

impl Value {
    /// Returns a value of `kind` with `amount` in base units. A check passes when `amount` is
    /// positive.
    pub fn of(kind: Kind, amount: f64) -> Self {
        match kind {
            Kind::Check => Self::Check(amount > 0.0),
            Kind::Count => Self::Count(amount),
            Kind::Size => Self::Size(amount),
            Kind::Time => Self::Time(amount),
        }
    }

    /// Returns the number in base units, or `None` for a check.
    pub fn amount(self) -> Option<f64> {
        match self {
            Self::Check(_) | Self::Skip => None,
            Self::Count(amount) | Self::Size(amount) | Self::Time(amount) => Some(amount),
        }
    }

    /// Returns the kind of quantity the value measures.
    pub fn kind(self) -> Kind {
        match self {
            Self::Check(_) | Self::Skip => Kind::Check,
            Self::Count(_) => Kind::Count,
            Self::Size(_) => Kind::Size,
            Self::Time(_) => Kind::Time,
        }
    }
}

/// An error in value text, with the byte offset of the problem inside that text.
#[derive(Debug)]
pub(crate) struct ParseError {
    pub(crate) code: Code,
    pub(crate) message: String,
    pub(crate) offset: usize,
}

/// Parses value text such as `45.6MB`, `236ms`, `54774`, `pass`, or `skip`.
pub(crate) fn parse(text: &str) -> Result<Value, ParseError> {
    match text {
        "fail" => return Ok(Value::Check(false)),
        "pass" => return Ok(Value::Check(true)),
        "skip" => return Ok(Value::Skip),
        _ => {}
    }

    let (number, unit) = split(text).ok_or_else(|| ParseError {
        code: Code::InvalidNumber,
        message: format!(
            "expected a number, a number with a unit, `pass`, `fail`, or `skip`, found `{text}`"
        ),
        offset: 0,
    })?;

    if unit.is_empty() {
        return Ok(Value::Count(number.times(1.0)));
    }

    let size = SIZES.iter().find(|(name, _)| *name == unit);
    let time = TIMES.iter().find(|(name, _)| *name == unit);
    let value = match (size, time) {
        (Some((_, factor)), _) => Value::Size(number.times(*factor)),
        (_, Some((_, factor))) => Value::Time(number.times(*factor)),
        (None, None) => {
            let expected: Vec<String> = SIZES
                .iter()
                .chain(TIMES.iter())
                .map(|(name, _)| format!("`{name}`"))
                .collect();

            return Err(ParseError {
                code: Code::UnknownUnit,
                message: format!(
                    "unknown unit `{unit}`, expected one of {}",
                    expected.join(", ")
                ),
                offset: text.len() - unit.len(),
            });
        }
    };

    if number.digits < 0.0 {
        return Err(ParseError {
            code: Code::InvalidNumber,
            message: format!("{} cannot be negative", value.kind().plural()),
            offset: 0,
        });
    }

    Ok(value)
}

/// A decimal number as digits and a scale, so `45.6` is 456 with scale 1. Multiplying before
/// dividing by the scale keeps `1.1kB` at exactly 1,100 bytes.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Number {
    pub(crate) digits: f64,
    scale: i32,
}

impl Number {
    /// Returns the number multiplied by `factor`.
    pub(crate) fn times(self, factor: f64) -> f64 {
        self.digits * factor / 10f64.powi(self.scale)
    }
}

/// Splits text into a number and the unit that follows it. The number is an optional minus sign,
/// digits, and an optional fraction, such as `-12` or `45.6`.
pub(crate) fn split(text: &str) -> Option<(Number, &str)> {
    let bytes = text.as_bytes();
    let mut end = usize::from(bytes.first() == Some(&b'-'));
    let digits = |from: usize| {
        bytes.get(from..).map_or(0, |rest| {
            rest.iter().take_while(|b| b.is_ascii_digit()).count()
        })
    };

    let whole = digits(end);
    if whole == 0 {
        return None;
    }
    end += whole;

    let mut scale = 0;
    let mut mantissa = text.get(..end)?.to_owned();

    if bytes.get(end) == Some(&b'.') {
        let fraction = digits(end + 1);
        if fraction == 0 {
            return None;
        }
        mantissa.push_str(text.get(end + 1..end + 1 + fraction)?);
        scale = i32::try_from(fraction).ok()?;
        end += 1 + fraction;
    }

    let digits: f64 = mantissa.parse().ok()?;
    let number = Number { digits, scale };
    number
        .times(1.0)
        .is_finite()
        .then(|| (number, text.get(end..).unwrap_or_default()))
}
