//! Removes the `baseline` keys of older configurations in place, so `uphill --update` moves each
//! baseline into the baseline file and keeps the configuration's comments, order, and style.
//! Lines outside a metric's `baseline` stay as they were.
//!
//! The editor reads only as much syntax as it needs: where a key's value starts and ends. It skips
//! a form it cannot edit, such as a TOML baseline written as a table of its own, and the caller
//! checks the result by parsing it again.

use std::ops::Range;

use toml::Spanned;
use toml::de::{DeString, DeTable, DeValue};

use crate::config::Format;
use crate::document::{self, Kind, Node};

/// Returns `source` without the `baseline` key of each metric, named by category and metric.
pub(crate) fn remove(source: &str, format: Format, metrics: &[(&str, &str)]) -> String {
    let mut edits: Vec<(Range<usize>, &str)> = Vec::new();

    match format {
        Format::Toml => {
            let Ok(document) = DeTable::parse(source) else {
                return source.to_owned();
            };
            for (category, metric) in metrics {
                let Some(DeValue::Table(fields)) =
                    field(document.get_ref(), category).map(Spanned::get_ref)
                else {
                    continue;
                };
                if let Some(value) = field(fields, metric) {
                    edits.extend(toml(source, value));
                }
            }
        }
        Format::Yaml => {
            let Ok(Some(root)) = document::parse(source, Format::Yaml) else {
                return source.to_owned();
            };
            for (category, metric) in metrics {
                let found = find(&root, category).and_then(|(_, category)| find(category, metric));
                if let Some((key, value)) = found {
                    edits.extend(yaml(source, key, value));
                }
            }
        }
    }

    // Later edits go first, so earlier offsets stay valid.
    edits.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));
    let mut out = source.to_owned();
    for (range, text) in edits {
        if out.get(range.clone()).is_some() {
            out.replace_range(range, text);
        }
    }
    out
}

/// Returns the edit that removes the `baseline` key of the YAML metric at `key`.
fn yaml(source: &str, key: &Node, value: &Node) -> Option<(Range<usize>, &'static str)> {
    let Kind::Mapping(pairs) = &value.kind else {
        return None;
    };
    let existing = pairs
        .iter()
        .find(|(key, _)| key.text() == Some("baseline"))
        .map(|(key, _)| key.offset)?;
    let start = skip(source, after_colon(source, key.offset), b" \t");

    // A flow mapping: the key goes with one comma, or the mapping becomes `{}`.
    if source.as_bytes().get(start) == Some(&b'{') {
        let (close, _) = scan(source, start);
        let end = flow_end(source, after_colon(source, existing));
        let previous = source
            .as_bytes()
            .get(..existing)
            .and_then(|before| before.iter().rposition(|byte| !byte.is_ascii_whitespace()));
        let next = skip(source, end, b" \t\r\n");
        return Some(match previous {
            Some(comma) if source.as_bytes().get(comma) == Some(&b',') => (comma..end, ""),
            _ if source.as_bytes().get(next) == Some(&b',') => {
                (existing..skip(source, next + 1, b" \t"), "")
            }
            _ => (start..close + 1, "{}"),
        });
    }

    // A block mapping: the key goes with its lines.
    let end = block_value_end(source, existing, after_colon(source, existing));
    let to = (line_end(source, end) + 1).min(source.len());
    Some((line_start(source, existing)..to, ""))
}

/// Returns the edit that removes the `baseline` key of a TOML metric: an inline table, or a table
/// with its own header such as `[footprint.export-size]`.
fn toml(source: &str, value: &Spanned<DeValue<'_>>) -> Option<(Range<usize>, &'static str)> {
    let DeValue::Table(fields) = value.get_ref() else {
        return None;
    };
    let span = value.span();
    let (key, value) = fields
        .iter()
        .find(|(key, _)| key.get_ref() == "baseline")
        .map(|(key, value)| (key.span(), value.span()))?;

    // A baseline written as a table of its own, such as `[footprint.size.baseline]`, is skipped.
    if source.as_bytes().get(value.start) == Some(&b'[') {
        return None;
    }

    match source.as_bytes().get(span.start) {
        Some(b'{') => {
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
                (Some(previous), _) => (previous..value.end, ""),
                (None, Some(next)) => (key.start..next, ""),
                (None, None) => (span, "{}"),
            })
        }
        Some(b'[') => {
            let to = (line_end(source, value.end) + 1).min(source.len());
            Some((line_start(source, key.start)..to, ""))
        }
        _ => None,
    }
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
pub(crate) fn after_colon(source: &str, key: usize) -> usize {
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

pub(crate) fn line_start(source: &str, at: usize) -> usize {
    source
        .as_bytes()
        .get(..at)
        .and_then(|before| before.iter().rposition(|byte| *byte == b'\n'))
        .map_or(0, |newline| newline + 1)
}

pub(crate) fn line_end(source: &str, at: usize) -> usize {
    source
        .as_bytes()
        .get(at..)
        .and_then(|rest| rest.iter().position(|byte| *byte == b'\n'))
        .map_or(source.len(), |newline| at + newline)
}
