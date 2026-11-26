//! Renders `comment.md`, the short report a pull request comment shows: a heading and one table
//! row per category, with its score gauge in the compared revision and in this run.
//!
//! GitHub shows only images that it fetches by URL, and it removes inline SVG and `data:` URLs.
//! So each gauge links one of the SVG files in `assets/gauges/` of wevm/uphill, which
//! `assets/gauges.ts` writes. This is the only report that links anything outside itself.

use std::fmt::Write;

use crate::html::escape;
use crate::report::{CategoryReport, Report};

/// Where the gauges live: one SVG per score from 0 to 100, and `failed.svg`.
const GAUGES: &str = "https://raw.githubusercontent.com/wevm/uphill/main/assets/gauges";

/// Renders the comment for `report`. Each row names a category, then its gauge in the revision of
/// the comparison, when there is one, then its gauge in this run. A category that failed shows the
/// failed gauge, and one without a score shows `–`.
pub fn markdown(report: &Report) -> String {
    let mut out = String::from("## Uphill\n\n<table><tr><th align=\"left\">Category</th>");
    if let Some(comparison) = &report.comparison {
        let _ = write!(out, "<th>{}</th>", escape(revision(&comparison.reference)));
    }
    out.push_str("<th>This run</th></tr>");

    for category in &report.categories {
        let label = label(category);
        let _ = write!(out, "<tr><td><b>{}</b></td>", escape(&label));
        if let Some(comparison) = &report.comparison {
            let before = comparison
                .scores
                .iter()
                .find(|score| score.category == category.name && score.subject == category.subject)
                .map_or_else(
                    || "–".to_owned(),
                    |score| gauge(&label, score.score, score.error.is_some()),
                );
            let _ = write!(out, "<td align=\"center\">{before}</td>");
        }
        let after = match &category.result {
            Ok(scored) => gauge(&label, scored.score, false),
            Err(_) => gauge(&label, None, true),
        };
        let _ = write!(out, "<td align=\"center\">{after}</td></tr>");
    }

    out.push_str("</table>\n");
    out
}

/// Returns a category's row label: its title, after its subject when it has one.
fn label(category: &CategoryReport) -> String {
    match &category.subject {
        Some(subject) => format!("{subject} · {}", category.title),
        None => category.title.clone(),
    }
}

/// Returns the revision a comparison names, without its commit or remote: `main` for
/// `origin/main (ac66366)`.
fn revision(reference: &str) -> &str {
    let name = reference.split(' ').next().unwrap_or(reference);
    name.strip_prefix("origin/").unwrap_or(name)
}

/// Returns the image of a score's gauge, the failed gauge, or `–` without a score.
fn gauge(label: &str, score: Option<u8>, failed: bool) -> String {
    let file = match score {
        Some(score) => score.to_string(),
        None if failed => "failed".to_owned(),
        None => return "–".to_owned(),
    };
    format!(
        "<img src=\"{GAUGES}/{file}.svg\" width=\"48\" height=\"48\" alt=\"{}: {file}\">",
        escape(label)
    )
}
