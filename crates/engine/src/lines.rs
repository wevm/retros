//! Metric lines: what a script writes to the file at `$UPHILL_METRICS`.
//!
//! Each line is `metric value` or `metric item value`, with fields separated by spaces or tabs.
//! An item that contains a space is quoted, such as `"ABI Decode (500 results)"`, and a quoted
//! item escapes `"` and `\` with a backslash. A line may end with `target <value>` to set that
//! measurement's own target, such as `entry-size "." 408B target 2kB`, and then with
//! `note <text>` to say why the measurement is what it is, such as
//! `alt-text /pricing fail note "hero.svg has no alt text"`. A line may start with a subject and
//! a colon, such as `zc: startup 0.76ms`, when one script measures several things that each get
//! their own scores. Blank lines are ignored.

use std::borrow::Cow;

use crate::error::{Code, Error, Location};
use crate::value::{self, Value};

/// One measurement that a script wrote.
#[derive(Clone, Debug, PartialEq)]
pub struct Line<'a> {
    /// The item the measurement belongs to, for a metric that covers a list.
    pub item: Option<Cow<'a, str>>,
    /// The position of the line's first field.
    pub location: Location,
    /// The metric name.
    pub metric: &'a str,
    /// Why the measurement is what it is, such as `hero.svg has no alt text`.
    pub note: Option<Cow<'a, str>>,
    /// The line's own target, which takes the place of the metric's target in `uphill.yaml`.
    pub target: Option<Value>,
    /// The subject the measurement belongs to, such as `zc` in `zc: startup 0.76ms`.
    pub subject: Option<&'a str>,
    /// The measured value.
    pub value: Value,
}

/// Parses every line of `source`.
///
/// # Errors
///
/// Returns the first line that is not `[subject:] metric [item] value [target <value>]
/// [note <text>]`, or whose subject, name, value, target, or note is invalid.
pub fn parse(source: &str) -> Result<Vec<Line<'_>>, Error> {
    let mut lines = Vec::new();
    let mut start = 0;

    for raw in source.split_inclusive('\n') {
        let text = raw.trim_end_matches(['\n', '\r']);
        if !text.trim().is_empty() {
            lines.push(line(source, start, text)?);
        }
        start += raw.len();
    }

    Ok(lines)
}

/// Returns whether `text` is a valid category or metric name: an ASCII letter or digit, then
/// ASCII letters, digits, `-`, `_`, and `.`.
pub(crate) fn is_name(text: &str) -> bool {
    let mut bytes = text.bytes();
    bytes.next().is_some_and(|b| b.is_ascii_alphanumeric())
        && bytes.all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

/// A field of a line, with its byte offset in the whole source.
struct Field<'a> {
    offset: usize,
    text: Cow<'a, str>,
}

/// Parses one non-blank line that starts at byte `start` of `source`.
fn line<'a>(source: &'a str, start: usize, text: &'a str) -> Result<Line<'a>, Error> {
    let mut fields = fields(source, start, text)?;

    // A first field that ends with `:` names the subject. Metric names cannot hold `:`.
    let subject = match fields.first() {
        Some(Field {
            offset,
            text: Cow::Borrowed(first),
        }) if first.ends_with(':') => {
            let name = first.trim_end_matches(':');
            if !is_name(name) || name.len() + 1 != first.len() {
                return Err(Error::at(
                    Code::InvalidName,
                    source,
                    *offset,
                    format!(
                        "invalid subject `{first}`. A subject is a name followed by `:`, such as `zc:`, and names use ASCII letters, digits, `-`, `_`, and `.`"
                    ),
                ));
            }
            if fields.len() == 1 {
                return Err(Error::at(
                    Code::LineSyntax,
                    source,
                    offset + first.len(),
                    format!("expected a metric after subject `{first}`"),
                ));
            }
            fields.remove(0);
            Some(name)
        }
        _ => None,
    };

    let (fields, note) = match fields.as_slice() {
        [rest @ .., keyword, note] if rest.len() >= 2 && keyword.text == "note" => {
            if note.text.trim().is_empty() {
                return Err(Error::at(
                    Code::LineSyntax,
                    source,
                    note.offset,
                    "a note must not be empty".to_owned(),
                ));
            }
            (rest, Some(note))
        }
        [.., keyword] if fields.len() >= 3 && keyword.text == "note" => {
            return Err(Error::at(
                Code::LineSyntax,
                source,
                keyword.offset + keyword.text.len(),
                "expected text after `note`, such as `note \"hero.svg has no alt text\"`"
                    .to_owned(),
            ));
        }
        all => (all, None),
    };

    let (fields, target) = match fields {
        [rest @ .., keyword, target] if rest.len() >= 2 && keyword.text == "target" => {
            (rest, Some(target))
        }
        [.., keyword] if fields.len() >= 3 && keyword.text == "target" => {
            return Err(Error::at(
                Code::LineSyntax,
                source,
                keyword.offset + keyword.text.len(),
                "expected a value after `target`".to_owned(),
            ));
        }
        all => (all, None),
    };

    let (metric, item, value) = match fields {
        [metric, value] => (metric, None, value),
        [metric, item, value] => (metric, Some(item), value),
        [metric] => {
            return Err(Error::at(
                Code::LineSyntax,
                source,
                metric.offset + metric.text.len(),
                format!("expected a value after metric `{}`", metric.text),
            ));
        }
        [_, _, _, extra, ..] => {
            return Err(Error::at(
                Code::LineSyntax,
                source,
                extra.offset,
                "expected `metric [item] value [target <value>] [note <text>]`, found more fields. Quote an item or note that contains spaces"
                    .to_owned(),
            ));
        }
        [] => {
            return Err(Error::at(
                Code::LineSyntax,
                source,
                start,
                "expected `metric [item] value`".to_owned(),
            ));
        }
    };

    let Cow::Borrowed(name) = metric.text else {
        return Err(invalid_name(source, metric));
    };
    if !is_name(name) {
        return Err(invalid_name(source, metric));
    }

    let parse = |field: &Field<'_>| {
        value::parse(&field.text).map_err(|error| {
            Error::at(
                error.code,
                source,
                field.offset + error.offset,
                error.message,
            )
        })
    };
    let parsed = parse(value)?;

    let target = match target {
        None => None,
        Some(field) => {
            let target = parse(field)?;
            let error = |code, message| Err(Error::at(code, source, field.offset, message));

            match target.amount() {
                None => return error(Code::TargetUnit, "a check cannot be a target".to_owned()),
                Some(amount) if amount <= 0.0 => {
                    return error(
                        Code::TargetRange,
                        "a target must be greater than zero".to_owned(),
                    );
                }
                Some(_) if target.kind() != parsed.kind() => {
                    return error(
                        Code::TargetUnit,
                        format!(
                            "the target is {}, but the value is {}",
                            target.kind().singular(),
                            parsed.kind().singular()
                        ),
                    );
                }
                Some(_) => Some(target),
            }
        }
    };

    Ok(Line {
        item: item.map(|item| item.text.clone()),
        location: Location::of(source, metric.offset),
        metric: name,
        note: note.map(|note| note.text.clone()),
        subject,
        target,
        value: parsed,
    })
}

#[cold]
fn invalid_name(source: &str, field: &Field<'_>) -> Error {
    Error::at(
        Code::InvalidName,
        source,
        field.offset,
        format!(
            "invalid metric name `{}`. Names use ASCII letters, digits, `-`, `_`, and `.`",
            field.text
        ),
    )
}

/// Splits a line into fields, unquoting quoted fields.
fn fields<'a>(source: &'a str, start: usize, text: &'a str) -> Result<Vec<Field<'a>>, Error> {
    let mut fields = Vec::new();
    let mut rest = text;

    loop {
        rest = rest.trim_start_matches([' ', '\t']);
        if rest.is_empty() {
            return Ok(fields);
        }

        let offset = start + (text.len() - rest.len());

        if let Some(quoted) = rest.strip_prefix('"') {
            let (value, consumed) = unquote(quoted).ok_or_else(|| {
                Error::at(
                    Code::LineSyntax,
                    source,
                    offset,
                    "unterminated or invalid quoted field. Escape only `\"` and `\\` with `\\`"
                        .to_owned(),
                )
            })?;

            rest = quoted.get(consumed..).unwrap_or_default();
            if !rest.is_empty() && !rest.starts_with([' ', '\t']) {
                return Err(Error::at(
                    Code::LineSyntax,
                    source,
                    start + (text.len() - rest.len()),
                    "expected a space after the quoted field".to_owned(),
                ));
            }

            fields.push(Field {
                offset,
                text: value,
            });
        } else {
            let end = rest.find([' ', '\t']).unwrap_or(rest.len());
            let (field, after) = rest.split_at(end);

            fields.push(Field {
                offset,
                text: Cow::Borrowed(field),
            });
            rest = after;
        }
    }
}

/// Reads a quoted field after its opening quote. Returns the unescaped text and the number of
/// bytes consumed, including the closing quote.
fn unquote(text: &str) -> Option<(Cow<'_, str>, usize)> {
    let close = text.find(['"', '\\'])?;
    if text.as_bytes().get(close) == Some(&b'"') {
        return Some((Cow::Borrowed(text.get(..close)?), close + 1));
    }

    let mut out = String::from(text.get(..close)?);
    let mut chars = text.get(close..)?.char_indices();

    while let Some((index, c)) = chars.next() {
        match c {
            '"' => return Some((Cow::Owned(out), close + index + 1)),
            '\\' => match chars.next()? {
                (_, escaped @ ('"' | '\\')) => out.push(escaped),
                _ => return None,
            },
            other => out.push(other),
        }
    }

    None
}
