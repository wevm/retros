//! Writes metric baselines into the configuration text in place, so `uphill --update` keeps the
//! file's comments, order, and style. Lines outside a metric's `baseline` stay as they were.
//!
//! In YAML, a baseline with items or subjects takes one line per entry, so a change shows as one
//! line in a diff. A flow metric that gets one becomes a block mapping, and its other keys keep
//! their text. A single value stays on the metric's line. A TOML baseline takes one line.
//!
//! The editor reads only as much syntax as it needs: where a key's value starts and ends. It skips
//! a form it cannot edit, such as a TOML metric defined with dotted keys, and the caller checks the
//! result by parsing it again.

use std::borrow::Cow;
use std::fmt::Write;
use std::ops::Range;

use toml::Spanned;
use toml::de::{DeString, DeTable, DeValue};
use yaml_rust2::Yaml;

use crate::baseline::{self, Baseline};
use crate::config::Format;
use crate::document::{self, Kind, Node};

/// Returns `source` with each metric's `baseline` replaced, added, or removed when it is `None`.
/// Each change names its category and metric.
pub(crate) fn baselines(
    source: &str,
    format: Format,
    changes: &[(&str, &str, Option<Baseline>)],
) -> String {
    let mut edits: Vec<(Range<usize>, String)> = Vec::new();

    match format {
        Format::Toml => {
            let Ok(document) = DeTable::parse(source) else {
                return source.to_owned();
            };
            for (category, metric, baseline) in changes {
                let Some(DeValue::Table(fields)) =
                    field(document.get_ref(), category).map(Spanned::get_ref)
                else {
                    continue;
                };
                if let Some(value) = field(fields, metric) {
                    edits.extend(toml(source, value, baseline.as_ref()));
                }
            }
        }
        Format::Yaml => {
            let Ok(Some(root)) = document::parse(source, Format::Yaml) else {
                return source.to_owned();
            };
            for (category, metric, baseline) in changes {
                let found = find(&root, category).and_then(|(_, category)| find(category, metric));
                if let Some((key, value)) = found {
                    edits.extend(yaml(source, key, value, baseline.as_ref()));
                }
            }
        }
    }

    // Later edits go first, so earlier offsets stay valid.
    edits.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));
    let mut out = source.to_owned();
    for (range, text) in edits {
        if out.get(range.clone()).is_some() {
            out.replace_range(range, &text);
        }
    }
    out
}

/// Returns the edit that sets the `baseline` of the YAML metric at `key`.
fn yaml(
    source: &str,
    key: &Node,
    value: &Node,
    baseline: Option<&Baseline>,
) -> Option<(Range<usize>, String)> {
    let colon = after_colon(source, key.offset);
    let start = skip(source, colon, b" \t");
    let column = column(source, key.offset);
    let pairs = match &value.kind {
        Kind::Mapping(pairs) => pairs.as_slice(),
        _ => &[],
    };
    let existing = pairs
        .iter()
        .find(|(key, _)| key.text() == Some("baseline"))
        .map(|(key, _)| key.offset);
    let block_key = source
        .get(line_start(source, key.offset)..key.offset)
        .is_some_and(|indent| indent.bytes().all(|byte| byte == b' '));
    let opening = source.as_bytes().get(start) == Some(&b'{');

    // A metric without keys, such as `modules:` or `modules: ~`, or a flow mapping. The value
    // ends at its closing brace, or after the null it writes, if any.
    if opening || !matches!(value.kind, Kind::Mapping(_)) {
        let scanned = opening.then(|| scan(source, start));
        let end = if let Some((close, _)) = scanned {
            close + 1
        } else {
            let token = value
                .text()
                .filter(|text| {
                    source
                        .get(start..)
                        .is_some_and(|rest| rest.starts_with(text))
                })
                .map_or(0, str::len);
            if token == 0 { colon } else { start + token }
        };

        // A baseline with items or subjects takes one line per entry, so a change shows as one
        // line in a diff. The metric becomes a block mapping, and its other keys keep their text.
        if let (true, Some(baseline @ Baseline::Map(_))) = (block_key, baseline) {
            let after = skip(source, end, b" \t");
            let (end, comment) = if source.as_bytes().get(after) == Some(&b'#') {
                let stop = line_end(source, after);
                (stop, source.get(after..stop))
            } else {
                (end, None)
            };
            let indent = " ".repeat(column + 2);
            let mut out = comment
                .map(|comment| format!(" {comment}"))
                .unwrap_or_default();
            for (other, _) in pairs
                .iter()
                .filter(|(other, _)| other.text() != Some("baseline"))
            {
                let text = source
                    .get(other.offset..flow_end(source, after_colon(source, other.offset)))
                    .unwrap_or_default();
                let _ = write!(out, "\n{indent}{text}");
            }
            let _ = write!(out, "\n{indent}baseline:{}", block(baseline, column + 2, 2));
            return Some((colon..end, out));
        }

        let Some((close, last)) = scanned else {
            return baseline
                .map(|baseline| (colon..end, format!(" {{ baseline: {} }}", flow(baseline))));
        };
        return match (existing, baseline) {
            (Some(key), Some(baseline)) => {
                let colon = after_colon(source, key);
                Some((
                    colon..flow_end(source, colon),
                    format!(" {}", flow(baseline)),
                ))
            }
            (Some(key), None) => {
                let end = flow_end(source, after_colon(source, key));
                let previous = source
                    .as_bytes()
                    .get(..key)
                    .and_then(|before| before.iter().rposition(|byte| !byte.is_ascii_whitespace()));
                let next = skip(source, end, b" \t\r\n");
                Some(match previous {
                    Some(comma) if source.as_bytes().get(comma) == Some(&b',') => {
                        (comma..end, String::new())
                    }
                    _ if source.as_bytes().get(next) == Some(&b',') => {
                        (key..skip(source, next + 1, b" \t"), String::new())
                    }
                    _ => (start..close + 1, "{}".to_owned()),
                })
            }
            (None, Some(baseline)) if last == start + 1 => Some((
                start..close + 1,
                format!("{{ baseline: {} }}", flow(baseline)),
            )),
            (None, Some(baseline)) => {
                let comma = source.as_bytes().get(last - 1) == Some(&b',');
                let separator = if comma { " " } else { ", " };
                Some((
                    last..last,
                    format!("{separator}baseline: {}", flow(baseline)),
                ))
            }
            (None, None) => None,
        };
    }

    // A block mapping: the baseline goes after the metric's other keys, indented like them.
    let child = pairs
        .first()
        .map_or(column + 2, |(key, _)| self::column(source, key.offset));
    let step = child.saturating_sub(column).max(1);

    match (existing, baseline) {
        (Some(key), Some(baseline)) => {
            let colon = after_colon(source, key);
            let end = block_value_end(source, key, colon);
            Some((colon..end, block(baseline, self::column(source, key), step)))
        }
        (Some(key), None) => {
            let end = block_value_end(source, key, after_colon(source, key));
            let to = (line_end(source, end) + 1).min(source.len());
            Some((line_start(source, key)..to, String::new()))
        }
        (None, Some(baseline)) => {
            let at = block_end(source, column, key.offset);
            let indent = " ".repeat(child);
            Some((
                at..at,
                format!("\n{indent}baseline:{}", block(baseline, child, step)),
            ))
        }
        (None, None) => None,
    }
}

/// Returns the edit that sets the `baseline` of a TOML metric: an inline table, or a table with
/// its own header such as `[footprint.export-size]`.
fn toml(
    source: &str,
    value: &Spanned<DeValue<'_>>,
    baseline: Option<&Baseline>,
) -> Option<(Range<usize>, String)> {
    let DeValue::Table(fields) = value.get_ref() else {
        return None;
    };
    let span = value.span();
    let existing = fields
        .iter()
        .find(|(key, _)| key.get_ref() == "baseline")
        .map(|(key, value)| (key.span(), value.span()));
    let opening = source.as_bytes().get(span.start).copied();

    // A baseline written as a table of its own, such as `[footprint.size.baseline]`, is skipped.
    if let Some((_, value)) = &existing
        && source.as_bytes().get(value.start) == Some(&b'[')
    {
        return None;
    }

    match (opening, existing, baseline) {
        (Some(b'{' | b'['), Some((_, value)), Some(baseline)) => Some((value, inline(baseline))),
        (Some(b'{'), Some((key, value)), None) => {
            let previous = fields
                .iter()
                .filter(|(other, _)| other.span().start < key.start)
                .map(|(_, value)| value.span().end)
                .max();
            let next = fields
                .iter()
                .filter(|(other, _)| other.span().start > key.start)
                .map(|(other, _)| other.span().start)
                .min();
            Some(match (previous, next) {
                (Some(previous), _) => (previous..value.end, String::new()),
                (None, Some(next)) => (key.start..next, String::new()),
                (None, None) => (span, "{}".to_owned()),
            })
        }
        (Some(b'['), Some((key, value)), None) => {
            let to = (line_end(source, value.end) + 1).min(source.len());
            Some((line_start(source, key.start)..to, String::new()))
        }
        (Some(b'{'), None, Some(baseline)) => {
            let last = fields.iter().map(|(_, value)| value.span().end).max();
            Some(match last {
                Some(end) => (end..end, format!(", baseline = {}", inline(baseline))),
                None => (span, format!("{{ baseline = {} }}", inline(baseline))),
            })
        }
        (Some(b'['), None, Some(baseline)) => {
            let last = fields.iter().map(|(_, value)| value.span().end).max();
            let at = line_end(source, last.unwrap_or(span.end));
            Some((at..at, format!("\nbaseline = {}", inline(baseline))))
        }
        _ => None,
    }
}

/// Renders a baseline as a YAML flow value, such as `{ viem: 1kB, viem/chains: 61kB }`.
fn flow(baseline: &Baseline) -> String {
    match baseline {
        Baseline::Map(entries) => {
            let entries: Vec<String> = entries
                .iter()
                .map(|(key, value)| format!("{}: {}", yaml_key(key), flow(value)))
                .collect();
            format!("{{ {} }}", entries.join(", "))
        }
        Baseline::Record(record) => baseline::text(*record).0,
    }
}

/// Renders a baseline as the YAML block value of a key at `column`: a record on the key's line,
/// or each entry on its own line, `step` columns further in.
fn block(baseline: &Baseline, column: usize, step: usize) -> String {
    match baseline {
        Baseline::Map(entries) => {
            let indent = " ".repeat(column + step);
            let mut out = String::new();
            for (key, value) in entries {
                let value = block(value, column + step, step);
                let _ = write!(out, "\n{indent}{}:{value}", yaml_key(key));
            }
            out
        }
        Baseline::Record(record) => format!(" {}", baseline::text(*record).0),
    }
}

/// Renders a baseline as a TOML value: a number, a string, or an inline table.
fn inline(baseline: &Baseline) -> String {
    match baseline {
        Baseline::Map(entries) => {
            let entries: Vec<String> = entries
                .iter()
                .map(|(key, value)| format!("{} = {}", toml_key(key), inline(value)))
                .collect();
            format!("{{ {} }}", entries.join(", "))
        }
        Baseline::Record(record) => match baseline::text(*record) {
            (text, true) => text,
            (text, false) => quoted(&text),
        },
    }
}

/// Formats a YAML key, unquoted when it is a plain name that YAML reads as a string.
fn yaml_key(name: &str) -> Cow<'_, str> {
    let plain = name
        .bytes()
        .next()
        .is_some_and(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        && name.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.' | b'/' | b'@' | b'_')
        })
        && matches!(Yaml::from_str(name), Yaml::String(_));
    if plain {
        Cow::Borrowed(name)
    } else {
        Cow::Owned(quoted(name))
    }
}

/// Formats a TOML key, bare when TOML allows it.
fn toml_key(name: &str) -> Cow<'_, str> {
    if !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        Cow::Borrowed(name)
    } else {
        Cow::Owned(quoted(name))
    }
}

/// Formats a double-quoted string, which YAML and TOML escape the same way.
fn quoted(text: &str) -> String {
    let mut out = String::from('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{:04X}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Returns the key and value of `name` in a YAML mapping.
fn find<'n>(node: &'n Node, name: &str) -> Option<(&'n Node, &'n Node)> {
    let Kind::Mapping(pairs) = &node.kind else {
        return None;
    };
    pairs
        .iter()
        .find(|(key, _)| key.text() == Some(name))
        .map(|(key, value)| (key, value))
}

/// Returns the value of `name` in a TOML table.
fn field<'t, 's>(table: &'t DeTable<'s>, name: &str) -> Option<&'t Spanned<DeValue<'s>>> {
    table
        .iter()
        .find(|(key, _)| DeString::as_ref(key.get_ref()) == name)
        .map(|(_, value)| value)
}

/// Returns the offset just past the `:` that follows the YAML key at `key`.
fn after_colon(source: &str, key: usize) -> usize {
    let bytes = source.as_bytes();
    let mut at = key;
    if matches!(bytes.get(at), Some(b'"' | b'\'')) {
        at = quoted_end(source, at);
    }
    while let Some(&byte) = bytes.get(at) {
        at += 1;
        if byte == b':' {
            break;
        }
    }
    at
}

/// Returns the end of a YAML value that follows a key's `:` in a block mapping: its text on the
/// key's line, without a comment, and every line after it indented more than the key.
fn block_value_end(source: &str, key: usize, colon: usize) -> usize {
    let start = skip(source, colon, b" \t");
    let inline = match source.as_bytes().get(start) {
        Some(b'{' | b'[') => scan(source, start).0 + 1,
        Some(b'"' | b'\'') => quoted_end(source, start),
        Some(b'#' | b'\r' | b'\n') | None => colon,
        Some(_) => plain_end(source, start, false),
    };
    block_end(source, column(source, key), inline)
}

/// Returns the end of the last line after `from` that is indented more than `column`, before the
/// first line that is not, or `from` when the next line is not.
fn block_end(source: &str, column: usize, from: usize) -> usize {
    let mut end = from;
    let mut at = line_end(source, from);
    while at < source.len() {
        let start = at + 1;
        let stop = line_end(source, start);
        let line = source.get(start..stop).unwrap_or_default();
        let content = line.trim_start_matches(' ');
        if !content.trim().is_empty() {
            if line.len() - content.len() <= column {
                break;
            }
            end = stop;
        }
        at = stop;
    }
    end
}

/// Returns the end of a YAML value that follows a key's `:` in a flow mapping.
fn flow_end(source: &str, colon: usize) -> usize {
    let start = skip(source, colon, b" \t\r\n");
    match source.as_bytes().get(start) {
        Some(b'{' | b'[') => scan(source, start).0 + 1,
        Some(b'"' | b'\'') => quoted_end(source, start),
        _ => plain_end(source, start, true),
    }
}

/// Returns the offset of the bracket that closes the flow collection opening at `open`, and the
/// end of the last token inside it, which is `open + 1` when the collection is empty.
fn scan(source: &str, open: usize) -> (usize, usize) {
    let bytes = source.as_bytes();
    let (mut at, mut depth, mut last) = (open, 0_usize, open + 1);
    while let Some(&byte) = bytes.get(at) {
        let token = at == 0
            || bytes
                .get(at - 1)
                .is_some_and(|before| before.is_ascii_whitespace() || b"{[,:".contains(before));
        match byte {
            b'{' | b'[' => {
                depth += 1;
                at += 1;
                if depth > 1 {
                    last = at;
                }
            }
            b'}' | b']' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return (at, last);
                }
                at += 1;
                last = at;
            }
            b'"' | b'\'' if token => {
                at = quoted_end(source, at);
                last = at;
            }
            b'#' if token => at = line_end(source, at),
            _ if byte.is_ascii_whitespace() => at += 1,
            _ => {
                at += 1;
                last = at;
            }
        }
    }
    (bytes.len(), last)
}

/// Returns the offset just past the quoted scalar that opens at `open`.
fn quoted_end(source: &str, open: usize) -> usize {
    let bytes = source.as_bytes();
    let Some(&quote) = bytes.get(open) else {
        return open;
    };
    let mut at = open + 1;
    while let Some(&byte) = bytes.get(at) {
        at += 1;
        if byte == b'\\' && quote == b'"' {
            at += 1;
        } else if byte == quote {
            // A single-quoted scalar writes a quote as two.
            if quote == b'\'' && bytes.get(at) == Some(&b'\'') {
                at += 1;
            } else {
                return at;
            }
        }
    }
    bytes.len()
}

/// Returns the end of a plain scalar starting at `start`, before a comment and trailing spaces. In
/// a flow mapping, a `,` or closing bracket also ends it.
fn plain_end(source: &str, start: usize, flow: bool) -> usize {
    let bytes = source.as_bytes();
    let (mut at, mut end) = (start, start);
    while let Some(&byte) = bytes.get(at) {
        let comment = byte == b'#'
            && at
                .checked_sub(1)
                .and_then(|before| bytes.get(before))
                .is_some_and(u8::is_ascii_whitespace);
        if byte == b'\n' || comment || (flow && matches!(byte, b',' | b'}' | b']')) {
            break;
        }
        at += 1;
        if !byte.is_ascii_whitespace() {
            end = at;
        }
    }
    end
}

/// Returns the first offset at or after `at` whose byte is not in `bytes`.
fn skip(source: &str, at: usize, bytes: &[u8]) -> usize {
    let rest = source.as_bytes().get(at..).unwrap_or_default();
    at + rest.iter().take_while(|byte| bytes.contains(byte)).count()
}

/// Returns the column of `at` in bytes, which for an indented key is its indentation.
fn column(source: &str, at: usize) -> usize {
    at - line_start(source, at)
}

fn line_start(source: &str, at: usize) -> usize {
    source
        .as_bytes()
        .get(..at)
        .and_then(|before| before.iter().rposition(|byte| *byte == b'\n'))
        .map_or(0, |newline| newline + 1)
}

fn line_end(source: &str, at: usize) -> usize {
    source
        .as_bytes()
        .get(at..)
        .and_then(|rest| rest.iter().position(|byte| *byte == b'\n'))
        .map_or(source.len(), |newline| at + newline)
}
