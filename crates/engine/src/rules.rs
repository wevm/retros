//! Renders `uphill rules`: the rules of a project's categories as Markdown, for an agent to follow
//! while it writes code. Each metric's `description` is its rule, and its default target and
//! patterns from the baseline file follow it. A metric without a description has no entry. Categories and metrics keep the composed order of
//! the configuration and its packs.
//!
//! The rules file, `.agents/uphill/rules.md`, holds the same Markdown, and the project's AGENTS.md
//! @-mentions it, so agents that read AGENTS.md load every rule with the rest of their guidance.

use std::fmt::Write;

use crate::config::{Better, Category, Metric};
use crate::format;

/// The rules file, relative to the project root.
pub const FILE: &str = ".agents/uphill/rules.md";

/// The line that AGENTS.md holds to load the rules file: a link for readers, and an @-mention that
/// agents resolve relative to AGENTS.md at the project root. The mention ends the line, so no
/// punctuation follows the path.
const MENTION: &str = "Follow the [Uphill rules](.agents/uphill/rules.md), which `uphill rules --write` rewrites from each metric's description: @.agents/uphill/rules.md";

/// Returns the text of AGENTS.md with the line that mentions the rules file at its top, under the
/// title when the file starts with one, or `None` when `agents` already mentions the file. An empty
/// `agents` is a new AGENTS.md.
pub fn mention(agents: &str) -> Option<String> {
    if agents.contains("@.agents/uphill/rules.md") {
        return None;
    }

    // A first line of `# ` is the file's title, so the mention goes below it.
    let (title, rest) = match agents.split_once('\n') {
        Some((first, rest)) if first.starts_with("# ") => (Some(first), rest),
        _ if agents.starts_with("# ") => (Some(agents), ""),
        _ => (None, agents),
    };

    let mut text = String::new();
    if let Some(title) = title {
        text.push_str(title);
        text.push_str("\n\n");
    }
    text.push_str(MENTION);
    text.push('\n');
    let rest = rest.trim_start_matches('\n');
    if !rest.is_empty() {
        text.push('\n');
        text.push_str(rest);
    }
    Some(text)
}

/// Renders the rules of `categories`: a heading for each category with rules, its description, and
/// one list entry per metric with a description. A metric without a description states no rule, so
/// it has no entry.
pub fn markdown(categories: &[&Category]) -> String {
    let mut out = String::from(
        "# Rules\n\nUphill writes these rules from each metric's `description`. Write code that meets them. `uphill` scores each metric against its target, and `uphill --compare HEAD --compact` lists what a change made worse.\n",
    );

    for category in categories {
        let entries: Vec<String> = category.metrics.iter().filter_map(entry).collect();
        if entries.is_empty() {
            continue;
        }

        let _ = write!(out, "\n## {}\n\n", category.title());
        if let Some(description) = &category.description {
            let _ = write!(out, "{}\n\n", description.trim());
        }
        for entry in entries {
            let _ = writeln!(out, "{entry}");
        }
    }

    out
}

/// Returns a metric's list entry: its description, then its targets, or `None` without a
/// description.
fn entry(metric: &Metric) -> Option<String> {
    let bound = match metric.better {
        Better::Higher => "least",
        Better::Lower => "most",
    };

    let mut parts = vec![metric.description.as_deref()?.trim().to_owned()];
    if let Some(target) = metric.target {
        parts.push(format!("At {bound} {}.", format::value(target)));
    }
    for pattern in &metric.targets {
        parts.push(format!(
            "`{}`: at {bound} {}.",
            pattern.pattern,
            format::value(pattern.target)
        ));
    }

    Some(format!("- {}", parts.join(" ")))
}
