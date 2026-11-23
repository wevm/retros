//! The HTML report: one self-contained page, with inline styles and a small inline script, that
//! makes no network requests. It follows Lighthouse's report: a tab per subject, a gauge per
//! category, and for each category a grid of metrics, each with its value against its target, then
//! a collapsible row per list of items. Opportunities and the changes
//! since the baselines come first. Like the other reports, it has no timestamps or paths, so the
//! same scores always produce the same bytes.

use std::fmt::Write;

use crate::baseline::{Change, Direction};
use crate::config::Better;
use crate::format;
use crate::report::{self, CategoryReport, Report};
use crate::score::{CategoryScore, MetricScore};
use crate::value::{Kind, Value};

/// How many items of a list the page shows under its metric, worst first.
const ITEMS: usize = 100;

/// Renders the report as one HTML page.
pub fn html(report: &Report) -> String {
    let mut out = String::from(HEAD);
    let subjects = report::subjects(report);
    let named = subjects.iter().any(Option::is_some);

    out.push_str("<main>\n<h1>Retros report</h1>\n");

    if named {
        compare(&mut out, report, &subjects);

        out.push_str("<div class=\"tabs\" role=\"tablist\" aria-label=\"Subjects\">\n");
        for (index, subject) in subjects.iter().enumerate() {
            let selected = index == 0;
            let _ = writeln!(
                out,
                "<button role=\"tab\" id=\"tab-{index}\" aria-controls=\"panel-{index}\" aria-selected=\"{selected}\" tabindex=\"{}\">{}</button>",
                if selected { "0" } else { "-1" },
                escape(subject.unwrap_or("Other"))
            );
        }
        out.push_str("</div>\n");
    }

    for (index, subject) in subjects.iter().enumerate() {
        if named {
            let _ = writeln!(
                out,
                "<section class=\"panel\" role=\"tabpanel\" id=\"panel-{index}\" aria-labelledby=\"tab-{index}\" tabindex=\"0\"{}>",
                if index == 0 { "" } else { " hidden" }
            );
        } else {
            out.push_str("<section class=\"panel\">\n");
        }
        panel(&mut out, report, *subject, index);
        out.push_str("</section>\n");
    }

    out.push_str("</main>\n");
    out.push_str(SCRIPT);
    out.push_str("</body>\n</html>\n");
    out
}

/// Writes a table of every subject's category scores, so subjects compare at a glance.
fn compare(out: &mut String, report: &Report, subjects: &[Option<&str>]) {
    let names = report::names(report);

    out.push_str("<div class=\"scroll\"><table class=\"compare\">\n<thead><tr><th scope=\"col\">Subject</th>");
    for name in &names {
        let title = report
            .categories
            .iter()
            .find(|category| category.name == *name)
            .map_or(*name, |category| category.title.as_str());
        let _ = write!(out, "<th scope=\"col\">{}</th>", escape(title));
    }
    out.push_str("</tr></thead>\n<tbody>\n");

    for subject in subjects {
        let _ = write!(
            out,
            "<tr><th scope=\"row\">{}</th>",
            escape(subject.unwrap_or("Other"))
        );
        for name in &names {
            let score = report
                .categories
                .iter()
                .find(|category| category.subject.as_deref() == *subject && category.name == *name)
                .map(|category| category.result.as_ref().map(|scored| scored.score));
            let cell = match score {
                Some(Ok(score)) => chip(score),
                Some(Err(_)) => "<span class=\"chip failed\">failed</span>".to_owned(),
                None => "<span class=\"muted\">–</span>".to_owned(),
            };
            let _ = write!(out, "<td>{cell}</td>");
        }
        out.push_str("</tr>\n");
    }

    out.push_str("</tbody>\n</table></div>\n");
}

/// Writes one subject's gauges, opportunities, changes, and categories.
fn panel(out: &mut String, report: &Report, subject: Option<&str>, index: usize) {
    let categories: Vec<&CategoryReport> = report
        .categories
        .iter()
        .filter(|category| category.subject.as_deref() == subject)
        .collect();
    let anchor = |category: &CategoryReport| format!("s{index}-{}", category.name);

    out.push_str("<nav class=\"gauges\" aria-label=\"Categories\">\n");
    for category in &categories {
        let score = category
            .result
            .as_ref()
            .ok()
            .and_then(|scored| scored.score);
        let _ = writeln!(
            out,
            "<a class=\"gauge {}\" href=\"#{}\">{}<span class=\"gauge-label\">{}</span></a>",
            band_class(score),
            anchor(category),
            gauge(score, category.result.is_err()),
            escape(&category.title)
        );
    }
    out.push_str("</nav>\n");
    out.push_str(SCALE);

    let opportunities: Vec<_> = report::opportunities(report)
        .into_iter()
        .filter(|(category, _)| category.subject.as_deref() == subject)
        .collect();
    if !opportunities.is_empty() {
        let most = opportunities
            .iter()
            .map(|(_, opportunity)| opportunity.gain)
            .fold(0.0, f64::max);
        out.push_str("<section class=\"block\">\n<h2>Opportunities</h2>\n<p class=\"muted\">The changes that would raise each category most, in category points.</p>\n<ul class=\"opportunities\">\n");
        for (category, opportunity) in &opportunities {
            let metric = category.metric_title(&opportunity.metric);
            let _ = writeln!(
                out,
                "<li><span class=\"gain\">{}</span><span class=\"bar-gain\" aria-hidden=\"true\"><span style=\"width:{:.0}%\"></span></span><span class=\"what\"><span class=\"muted\">{}</span> {}</span><span class=\"to\">{} → {}</span></li>",
                escape(&report::gain(opportunity)),
                opportunity.gain / most * 100.0,
                escape(&category.title),
                inline(&report::titled(
                    &metric,
                    opportunity.item.as_deref(),
                    opportunity.count
                )),
                escape(&format::value(opportunity.value)),
                escape(&format::value(opportunity.target))
            );
        }
        out.push_str("</ul>\n</section>\n");
    }

    if let Some(changes) = &report.changes {
        let changes: Vec<&Change> = report::sorted(changes)
            .into_iter()
            .filter(|change| change.subject.as_deref() == subject)
            .collect();
        out.push_str("<section class=\"block\">\n<h2>Changes since the baseline</h2>\n");
        if changes.is_empty() {
            out.push_str("<p class=\"muted\">No changes.</p>\n");
        } else {
            out.push_str("<ul class=\"changes\">\n");
            for change in changes {
                let (category, metric) = report::titles(report, change);
                let _ = writeln!(
                    out,
                    "<li><span class=\"chip {}\">{}</span><span class=\"what\"><span class=\"muted\">{}</span> {}</span><span class=\"to\">{}</span></li>",
                    direction_class(change.direction),
                    change.direction.as_str(),
                    escape(&category),
                    inline(&report::titled(&metric, change.item.as_deref(), 1)),
                    escape(&report::values(change))
                );
            }
            out.push_str("</ul>\n");
        }
        out.push_str("</section>\n");
    }

    if let Some(comparison) = &report.comparison {
        let moved: Vec<_> = report::moved(comparison)
            .into_iter()
            .filter(|(change, _)| change.subject.as_deref() == subject)
            .collect();
        let _ = writeln!(
            out,
            "<section class=\"block\">\n<h2>Changes against {}</h2>",
            escape(&comparison.reference)
        );
        if moved.is_empty() {
            out.push_str("<p class=\"muted\">No changes.</p>\n");
        } else {
            out.push_str("<ul class=\"changes\">\n");
            for (change, timing) in moved {
                let (category, metric) = report::titles(report, &change);
                let evidence = timing.map_or_else(String::new, |timing| {
                    format!(
                        "{} <span class=\"muted\">{}</span>",
                        escape(&report::percent(timing)),
                        escape(&report::agreement(timing))
                    )
                });
                let _ = writeln!(
                    out,
                    "<li><span class=\"chip {}\">{}</span><span class=\"what\"><span class=\"muted\">{}</span> {}</span><span class=\"to\">{}{evidence}</span></li>",
                    direction_class(change.direction),
                    change.direction.as_str(),
                    escape(&category),
                    inline(&report::titled(&metric, change.item.as_deref(), 1)),
                    escape(&report::values(&change))
                );
            }
            out.push_str("</ul>\n");
        }
        let timings = comparison.timings.iter();
        if let Some(noise) = report::noise(timings.filter(|t| t.subject.as_deref() == subject)) {
            let _ = writeln!(out, "<p class=\"muted\">{}.</p>", escape(&noise));
        }
        out.push_str("</section>\n");
    }

    for category in categories {
        self::category(out, category, &anchor(category));
    }
}

/// Writes one category: a header with its gauge, title, and description, then its metrics and
/// its lists of items, or why it has no scores.
fn category(out: &mut String, category: &CategoryReport, id: &str) {
    let score = category
        .result
        .as_ref()
        .ok()
        .and_then(|scored| scored.score);

    let _ = writeln!(
        out,
        "<section class=\"category\" id=\"{id}\">\n<header class=\"category-head\"><div class=\"gauge large {}\">{}</div><h2>{}</h2>",
        band_class(score),
        gauge(score, category.result.is_err()),
        escape(&category.title)
    );
    if let Some(description) = &category.description {
        let _ = writeln!(out, "<p class=\"muted\">{}</p>", inline(description));
    }
    out.push_str("</header>\n");

    match &category.result {
        Err(failure) => {
            let _ = writeln!(
                out,
                "<p class=\"failure\">✗ {}</p>",
                escape(&failure.message)
            );
            if !failure.details.is_empty() {
                let _ = writeln!(out, "<pre>{}</pre>", escape(&failure.details));
            }
        }
        Ok(scored) => {
            metrics(out, scored, id);
            lists(out, scored);
        }
    }
    out.push_str("</section>\n");
}

/// Writes a category's metrics as a grid in the order `retros.yaml` declares them.
fn metrics(out: &mut String, scored: &CategoryScore, id: &str) {
    out.push_str("<div class=\"group-head\"><h3>Metrics</h3>");
    if scored
        .metrics
        .iter()
        .any(|metric| metric.metric.description.is_some())
    {
        let _ = write!(
            out,
            "<button class=\"expand\" type=\"button\" aria-controls=\"{id}-metrics\" aria-expanded=\"false\">Expand view</button>"
        );
    }
    out.push_str("</div>\n");

    if scored.rounds > 1 {
        let _ = writeln!(
            out,
            "<p class=\"note muted\">Values that vary between runs are the median of {} rounds.</p>",
            scored.rounds
        );
    }

    let _ = writeln!(out, "<ul class=\"grid\" id=\"{id}-metrics\">");
    for metric in &scored.metrics {
        card(out, metric);
    }
    out.push_str("</ul>\n");
}

/// Writes one metric of the grid: its band, title, and score, its value in its band's color, its
/// value against its target, its line's note, and its description, which shows in the expanded
/// view.
fn card(out: &mut String, metric: &MetricScore) {
    let band = band_class(metric.score);
    let _ = write!(
        out,
        "<li class=\"metric\"><div class=\"metric-head\"><span class=\"shape {band}\" aria-hidden=\"true\"></span><span class=\"metric-title\">{}</span><span class=\"metric-score {band}\">{}</span></div><div class=\"metric-value {band}\">{}</div>",
        escape(&metric.metric.title()),
        metric
            .score
            .map_or_else(|| "–".to_owned(), |score| score.to_string()),
        escape(&headline(metric))
    );

    if metric.kind != Kind::Check && !metric.items.is_empty() {
        let count = metric.items.len();
        let items = format!("{count} {}", if count == 1 { "item" } else { "items" });
        let rollup = report::rollup(metric.metric.score);
        let at = match (&metric.at, rollup) {
            (_, "sum") => format!("Sum of {items}"),
            (Some(item), _) => format!("{rollup} of {items}, at <code>{}</code>", escape(item)),
            (None, _) => format!("{rollup} of {items}"),
        };
        let _ = write!(out, "<div class=\"metric-at muted\">{at}</div>");
    }

    let target = metric.target.and_then(Value::amount);
    match (metric.kind, target) {
        (Kind::Check, _) if !metric.items.is_empty() => {
            let _ = write!(
                out,
                "<div class=\"ratio\" aria-hidden=\"true\"><span style=\"width:{:.1}%\"></span></div>",
                metric.value * 100.0
            );
        }
        (Kind::Check, _) | (_, None) => {}
        (_, Some(target)) => {
            out.push_str(&bar(metric.value, target, metric.metric.better, band));
        }
    }

    let goal = match (metric.kind, metric.target) {
        (Kind::Check, _) => String::new(),
        (_, Some(target)) => format!("Target {}", format::value(target)),
        (_, None) => "No target".to_owned(),
    };
    let _ = write!(out, "<div class=\"metric-foot\">{}</div>", escape(&goal));
    if let Some(note) = &metric.note {
        let _ = write!(out, "<p class=\"why\">{}</p>", inline(note));
    }

    if let Some(description) = &metric.metric.description {
        let _ = write!(
            out,
            "<p class=\"metric-description\">{}</p>",
            inline(description)
        );
    }
    out.push_str("</li>\n");
}

/// Returns a metric's value as the grid shows it: `27.1 ms`, `pass`, or `5 of 6 pass`.
fn headline(metric: &MetricScore) -> String {
    if metric.kind == Kind::Check {
        return report::value(metric);
    }
    format::amount(metric.kind, metric.value)
}

/// Writes a collapsible row for each metric with items, as Lighthouse lists its audits. Lists
/// with an item that misses its target come first, and lists that meet every target close under
/// a Passed group.
fn lists(out: &mut String, scored: &CategoryScore) {
    let lists: Vec<&MetricScore> = scored
        .metrics
        .iter()
        .filter(|metric| !metric.items.is_empty())
        .collect();
    if lists.is_empty() {
        return;
    }

    let (passed, open): (Vec<&MetricScore>, Vec<&MetricScore>) =
        lists.into_iter().partition(|metric| {
            let judged: Vec<bool> = metric
                .items
                .iter()
                .filter_map(|item| report::misses_target(item, metric))
                .collect();
            !judged.is_empty() && !judged.contains(&true)
        });

    out.push_str("<div class=\"group-head\"><h3>Items</h3></div>\n");
    if !open.is_empty() {
        out.push_str("<ul class=\"audits\">\n");
        for metric in open {
            audit(out, metric);
        }
        out.push_str("</ul>\n");
    }
    if !passed.is_empty() {
        let _ = writeln!(
            out,
            "<details class=\"passed\"><summary>Passed <span class=\"muted\">({})</span></summary>\n<ul class=\"audits\">",
            passed.len()
        );
        for metric in passed {
            audit(out, metric);
        }
        out.push_str("</ul>\n</details>\n");
    }
}

/// Writes one list's row: its band, title, and how many items miss, which opens on a table of its
/// items, worst first, each against its own target.
fn audit(out: &mut String, metric: &MetricScore) {
    let count = metric.items.len();
    let judged: Vec<bool> = metric
        .items
        .iter()
        .filter_map(|item| report::misses_target(item, metric))
        .collect();
    let misses = judged.iter().filter(|missed| **missed).count();
    let check = metric.kind == Kind::Check;

    let summary = match (check, judged.is_empty(), misses) {
        (true, _, 0) => format!("All {count} pass"),
        (true, _, misses) => format!("{misses} of {count} fail"),
        (false, true, _) => format!("{count} items"),
        (false, false, 0) => format!("All {count} meet their target"),
        (false, false, misses) => format!("{misses} of {count} miss their target"),
    };

    let _ = writeln!(
        out,
        "<li><details><summary class=\"audit\"><span class=\"shape {}\" aria-hidden=\"true\"></span><span class=\"audit-title\">{}</span><span class=\"audit-summary\">{}</span></summary>\n<div class=\"scroll\"><table class=\"items\">\n<thead><tr><th scope=\"col\">Item</th><th scope=\"col\">{}</th>{}</tr></thead>\n<tbody>",
        band_class(metric.score),
        escape(&metric.metric.title()),
        escape(&summary),
        if check { "Result" } else { "Value" },
        if check {
            ""
        } else {
            "<th scope=\"col\">Target</th><th class=\"cell-bar\" scope=\"col\"><span class=\"hidden\">Against target</span></th>"
        }
    );

    for item in metric.items.iter().take(ITEMS) {
        let target = item.target.or(metric.metric.target);
        let cells = match (item.value, target.and_then(Value::amount)) {
            (Value::Check(passed), _) => format!(
                "<td><span class=\"chip {}\">{}</span></td>",
                if passed { "good" } else { "poor" },
                if passed { "pass" } else { "fail" }
            ),
            (value, Some(goal)) => {
                let amount = value.amount().unwrap_or_default();
                let band = band_of(amount, goal, metric.metric.better);
                format!(
                    "<td class=\"value {band}\">{}</td><td>{}</td><td class=\"cell-bar\">{}</td>",
                    escape(&format::value(value)),
                    escape(&target.map(format::value).unwrap_or_default()),
                    bar(amount, goal, metric.metric.better, band)
                )
            }
            (value, None) => format!(
                "<td class=\"value\">{}</td><td></td><td class=\"cell-bar\"></td>",
                escape(&format::value(value))
            ),
        };
        let note = item.note.as_deref().map_or_else(String::new, |note| {
            format!("<div class=\"why\">{}</div>", inline(note))
        });
        let _ = writeln!(out, "<tr><td>{}{note}</td>{cells}</tr>", escape(&item.name));
    }
    out.push_str("</tbody>\n</table></div>\n");

    if count > ITEMS {
        let _ = writeln!(
            out,
            "<p class=\"muted more\">And {} more in report.json.</p>",
            count - ITEMS
        );
    }
    out.push_str("</details></li>\n");
}

/// Renders a value against its target on a bar of the three score bands. Where lower is better,
/// the bar runs from zero to three times the target, so the target, which scores 90, sits a third
/// of the way along, and twice the target, which scores 50, two thirds. Where higher is better,
/// it runs from zero to one and a half times the target, so half the target sits a third along
/// and the target two thirds. A value past either end sits at that end.
fn bar(value: f64, target: f64, better: Better, band: &str) -> String {
    let (span, tick, direction) = match better {
        Better::Lower => (3.0 * target, 100.0 / 3.0, "lower"),
        Better::Higher => (1.5 * target, 200.0 / 3.0, "higher"),
    };
    let at = (value / span).clamp(0.0, 1.0) * 100.0;
    format!(
        "<div class=\"bar {direction}\" aria-hidden=\"true\"><span class=\"tick\" style=\"left:{tick:.1}%\"></span><span class=\"marker {band}\" style=\"left:{at:.1}%\"></span></div>"
    )
}

/// Returns the class of the band a value falls in against its target, on the boundaries of
/// [`crate::score::curve`]: at or better than the target is good, up to twice the target (half,
/// where higher is better) needs work, and beyond that is poor.
fn band_of(value: f64, target: f64, better: Better) -> &'static str {
    let ratio = match better {
        Better::Lower => value / target,
        Better::Higher if value <= 0.0 => f64::INFINITY,
        Better::Higher => target / value,
    };
    match ratio {
        r if r <= 1.0 => "good",
        r if r <= 2.0 => "average",
        _ => "poor",
    }
}

/// Renders a score gauge: a ring filled to the score in its band's color. Without a score the
/// ring stays empty, since a round cap would draw even an arc of length zero.
fn gauge(score: Option<u8>, failed: bool) -> String {
    let (arc, text) = match score {
        Some(score) => (
            format!(
                "<circle class=\"arc\" cx=\"60\" cy=\"60\" r=\"52\" pathLength=\"100\" stroke-dasharray=\"{score} 100\" transform=\"rotate(-90 60 60)\"/>"
            ),
            score.to_string(),
        ),
        None if failed => (String::new(), "✗".to_owned()),
        None => (String::new(), "–".to_owned()),
    };
    format!(
        "<svg viewBox=\"0 0 120 120\" aria-hidden=\"true\"><circle class=\"track\" cx=\"60\" cy=\"60\" r=\"52\"/>{arc}</svg><span class=\"gauge-score\">{text}</span>"
    )
}

/// Renders a score as a small chip in its band's color.
fn chip(score: Option<u8>) -> String {
    format!(
        "<span class=\"chip {}\">{}</span>",
        band_class(score),
        score.map_or_else(|| "–".to_owned(), |score| score.to_string())
    )
}

/// Returns the class of a score's Lighthouse band.
fn band_class(score: Option<u8>) -> &'static str {
    match score {
        Some(90..) => "good",
        Some(50..) => "average",
        Some(_) => "poor",
        None => "none",
    }
}

/// Returns the class of a change: red for worse and looser, green for better and tighter.
fn direction_class(direction: Direction) -> &'static str {
    match direction {
        Direction::Worse => "poor",
        Direction::Looser => "average",
        Direction::Better | Direction::Tighter => "good",
        Direction::New | Direction::Gone => "none",
    }
}

/// Escapes a description and renders its `code` spans, as Markdown writes them. A backtick
/// without its pair stays as text.
fn inline(text: &str) -> String {
    let parts: Vec<&str> = text.split('`').collect();
    let mut out = String::new();
    for (index, part) in parts.iter().enumerate() {
        let code = index % 2 == 1;
        if code && index + 1 == parts.len() {
            out.push('`');
            out.push_str(&escape(part));
        } else if code {
            let _ = write!(out, "<code>{}</code>", escape(part));
        } else {
            out.push_str(&escape(part));
        }
    }
    out
}

/// Escapes text for HTML content and attribute values.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

/// Lighthouse's legend of the three score bands.
const SCALE: &str = "<div class=\"scale\"><span><span class=\"shape poor\" aria-hidden=\"true\"></span>0–49</span><span><span class=\"shape average\" aria-hidden=\"true\"></span>50–89</span><span><span class=\"shape good\" aria-hidden=\"true\"></span>90–100</span></div>\n";

/// The document head and styles. Colors follow Lighthouse's bands, in light and dark schemes.
const HEAD: &str = r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="color-scheme" content="light dark">
<title>Retros report</title>
<style>
:root{--bg:#fff;--fg:#212121;--muted:#5f6368;--line:#e0e0e0;--soft:#f5f5f5;--focus:#1a73e8;--good:#0c6;--good-fg:#008800;--good-bg:#e5f9ee;--average:#fa3;--average-fg:#c33300;--average-bg:#fff3e0;--poor:#f33;--poor-fg:#c00;--poor-bg:#ffebee;--none:#9e9e9e;--none-fg:#5f6368;--none-bg:#eee}
@media (prefers-color-scheme:dark){:root{--bg:#202124;--fg:#e8eaed;--muted:#9aa0a6;--line:#3c4043;--soft:#2a2b2e;--focus:#8ab4f8;--good-fg:#0c6;--good-bg:rgba(0,204,102,.15);--average-fg:#ffa400;--average-bg:rgba(255,164,0,.15);--poor-fg:#ff4e42;--poor-bg:rgba(255,78,66,.15);--none-fg:#9aa0a6;--none-bg:#3c4043}}
*{box-sizing:border-box}
body{margin:0;background:var(--bg);color:var(--fg);font:15px/1.5 system-ui,-apple-system,"Segoe UI",Roboto,sans-serif}
main{max-width:960px;margin:0 auto;padding:24px 20px 64px}
h1{font-size:22px;margin:0 0 16px}
h2{font-size:18px;margin:0 0 8px}
a{color:inherit}
:focus-visible{outline:2px solid var(--focus);outline-offset:2px}
.muted{color:var(--muted)}
.hidden{position:absolute;width:1px;height:1px;overflow:hidden;clip-path:inset(50%);white-space:nowrap}
.scroll{overflow-x:auto}
table{border-collapse:collapse;width:100%}
th,td{text-align:left;padding:6px 10px;border-bottom:1px solid var(--line);white-space:nowrap}
th{font-weight:600}
.compare{margin-bottom:20px}
.compare td{text-align:center}
.compare thead th:not(:first-child){text-align:center}
.chip{display:inline-block;min-width:36px;padding:1px 8px;border-radius:10px;font-weight:600;font-size:13px;text-align:center;font-variant-numeric:tabular-nums}
.chip.good{background:var(--good-bg);color:var(--good-fg)}
.chip.average{background:var(--average-bg);color:var(--average-fg)}
.chip.poor,.chip.failed{background:var(--poor-bg);color:var(--poor-fg)}
.chip.none{background:var(--none-bg);color:var(--none-fg)}
.tabs{display:flex;gap:4px;border-bottom:1px solid var(--line);margin-bottom:20px;overflow-x:auto}
.tabs button{font:inherit;font-weight:600;color:var(--muted);background:none;border:0;border-bottom:3px solid transparent;padding:10px 14px;cursor:pointer;white-space:nowrap}
.tabs button[aria-selected=true]{color:var(--fg);border-bottom-color:var(--focus)}
.tabs button:hover{color:var(--fg)}
.gauges{display:flex;flex-wrap:wrap;justify-content:center;gap:12px 28px;padding:12px 0 8px}
.gauge{position:relative;display:flex;flex-direction:column;align-items:center;min-width:104px;text-decoration:none;border-radius:8px;padding:4px}
.gauge svg{width:88px;height:88px}
.gauge .track{fill:var(--bg);stroke:var(--soft);stroke-width:8}
.gauge.good .track{fill:var(--good-bg)}.gauge.average .track{fill:var(--average-bg)}.gauge.poor .track{fill:var(--poor-bg)}
.gauge .arc{fill:none;stroke-width:8;stroke-linecap:round;stroke:var(--none);animation:fill .8s ease-out}
.gauge.good .arc{stroke:var(--good)}.gauge.average .arc{stroke:var(--average)}.gauge.poor .arc{stroke:var(--poor)}
.gauge-score{position:absolute;top:4px;left:0;right:0;height:88px;display:flex;align-items:center;justify-content:center;font-size:26px;font-weight:600;font-variant-numeric:tabular-nums}
.gauge.good .gauge-score{color:var(--good-fg)}.gauge.average .gauge-score{color:var(--average-fg)}.gauge.poor .gauge-score{color:var(--poor-fg)}.gauge.none .gauge-score{color:var(--none-fg)}
.gauge-label{margin-top:6px;font-weight:600;text-align:center;white-space:nowrap}
.gauge.large{min-width:0;padding:0}
.gauge.large svg{width:112px;height:112px}
.gauge.large .gauge-score{top:0;height:112px;font-size:34px}
@keyframes fill{from{stroke-dasharray:0 100}}
@media (prefers-reduced-motion:reduce){.gauge .arc{animation:none}}
.scale{display:flex;justify-content:center;gap:20px;font-size:13px;color:var(--muted);padding:4px 0 20px;border-bottom:1px solid var(--line);margin-bottom:28px}
.scale>span{display:inline-flex;align-items:center;gap:6px}
.block{margin:0 0 32px}
.category{margin:0 0 48px;padding-top:8px}
.category-head{display:flex;flex-direction:column;align-items:center;text-align:center;margin-bottom:20px}
.category-head h2{font-size:22px;font-weight:500;margin:10px 0 4px}
.category-head p{max-width:640px;margin:0}
.group-head{display:flex;align-items:baseline;justify-content:space-between;gap:12px;border-bottom:1px solid var(--line);padding:8px 0 6px;margin-top:20px}
.group-head h3{margin:0;font-size:12px;font-weight:600;letter-spacing:.08em;text-transform:uppercase;color:var(--muted)}
.expand{font:inherit;font-size:13px;color:var(--focus);background:none;border:0;padding:2px 4px;cursor:pointer}
.expand:hover{text-decoration:underline}
.note{font-size:13px;margin:8px 0 0}
.grid{list-style:none;margin:6px 0 0;padding:0;display:grid;grid-template-columns:repeat(2,minmax(0,1fr));column-gap:40px}
.metric{border-bottom:1px solid var(--line);padding:14px 0 12px}
.metric-head{display:flex;align-items:center;gap:10px}
.metric-title{font-weight:500;overflow-wrap:anywhere}
.metric-score{margin-left:auto;font-size:14px;font-weight:600;font-variant-numeric:tabular-nums}
.metric-value{font-size:28px;line-height:1.25;margin:4px 0 0;font-variant-numeric:tabular-nums}
.good.metric-value,.good.metric-score,.value.good{color:var(--good-fg)}
.average.metric-value,.average.metric-score,.value.average{color:var(--average-fg)}
.poor.metric-value,.poor.metric-score,.value.poor{color:var(--poor-fg)}
.none.metric-score{color:var(--none-fg)}
.metric-at{font-size:13px;overflow-wrap:anywhere}
.bar{position:relative;height:6px;border-radius:3px;margin:12px 7px 8px;background:linear-gradient(to right,color-mix(in srgb,var(--good) 40%,transparent) 0 33.33%,color-mix(in srgb,var(--average) 40%,transparent) 33.33% 66.67%,color-mix(in srgb,var(--poor) 40%,transparent) 66.67%)}
.bar.higher{background:linear-gradient(to right,color-mix(in srgb,var(--poor) 40%,transparent) 0 33.33%,color-mix(in srgb,var(--average) 40%,transparent) 33.33% 66.67%,color-mix(in srgb,var(--good) 40%,transparent) 66.67%)}
.tick{position:absolute;top:-4px;bottom:-4px;width:2px;margin-left:-1px;border-radius:1px;background:var(--fg)}
.marker{position:absolute;top:50%;width:14px;height:14px;margin:-7px 0 0 -7px;border-radius:50%;border:2px solid var(--bg);background:var(--none)}
.marker.good{background:var(--good)}.marker.average{background:var(--average)}.marker.poor{background:var(--poor)}
.ratio{height:6px;border-radius:3px;margin:12px 0 8px;background:var(--poor);overflow:hidden}
.ratio>span{display:block;height:100%;background:var(--good)}
.metric-foot{font-size:13px;color:var(--muted)}
.metric-description{display:none;margin:8px 0 0;font-size:14px;color:var(--muted)}
.why{margin:4px 0 0;font-size:13px;color:var(--muted);overflow-wrap:anywhere}
.category.expanded .metric-description{display:block}
code{font:13px ui-monospace,SFMono-Regular,Menlo,monospace;background:var(--soft);padding:1px 4px;border-radius:4px}
.shape{display:inline-block;flex:none;width:12px;height:12px;background:var(--none);border-radius:50%}
.shape.good{background:var(--good)}
.shape.average{background:var(--average);border-radius:0}
.shape.poor{width:0;height:0;background:none;border-radius:0;border-left:6px solid transparent;border-right:6px solid transparent;border-bottom:11px solid var(--poor)}
.audits{list-style:none;margin:0;padding:0}
.audits>li{border-bottom:1px solid var(--line)}
summary.audit{display:grid;grid-template-columns:14px minmax(0,1fr) auto 16px;align-items:center;gap:4px 12px;padding:11px 4px;cursor:pointer;list-style:none}
summary.audit::-webkit-details-marker,.passed>summary::-webkit-details-marker{display:none}
summary.audit::after,.passed>summary::after{content:"";width:8px;height:8px;border-right:2px solid var(--muted);border-bottom:2px solid var(--muted);transform:rotate(45deg);justify-self:center}
details[open]>summary.audit::after,.passed[open]>summary::after{transform:rotate(-135deg)}
summary.audit:hover,.passed>summary:hover{background:var(--soft)}
.audit-title{font-weight:500;overflow-wrap:anywhere}
.audit-summary{font-size:14px;color:var(--muted);text-align:right}
.passed>summary{display:flex;align-items:center;gap:8px;padding:11px 4px;cursor:pointer;list-style:none;font-size:12px;font-weight:600;letter-spacing:.08em;text-transform:uppercase;border-bottom:1px solid var(--line)}
.passed>summary::after{margin-left:auto}
.items{margin:0 0 12px;font-size:14px}
.items td:first-child{white-space:normal;overflow-wrap:anywhere}
.items .value{text-align:right;font-variant-numeric:tabular-nums}
.items th:nth-child(2){text-align:right}
.cell-bar{width:160px}
.cell-bar .bar{margin:0 7px}
.more{font-size:14px}
.opportunities,.changes{list-style:none;margin:0;padding:0;border-top:1px solid var(--line)}
.opportunities>li,.changes>li{border-bottom:1px solid var(--line)}
.opportunities li,.changes li{display:grid;grid-template-columns:auto minmax(0,1fr) auto;align-items:center;gap:4px 12px;padding:10px 4px}
.opportunities li{grid-template-columns:44px 120px minmax(0,1fr) auto}
.gain{font-weight:600;color:var(--good-fg);font-variant-numeric:tabular-nums}
.bar-gain{height:8px;border-radius:4px;background:var(--soft);overflow:hidden}
.bar-gain span{display:block;height:100%;background:var(--good)}
.what{overflow-wrap:anywhere}
.to{font-variant-numeric:tabular-nums;white-space:nowrap}
.failure{color:var(--poor-fg);font-weight:600}
pre{background:var(--soft);padding:12px;border-radius:6px;overflow-x:auto;font-size:13px}
@media (max-width:600px){
main{padding:16px 12px 48px}
.gauges{gap:8px 12px}
.gauge{min-width:84px}.gauge svg{width:68px;height:68px}.gauge-score{height:68px;font-size:20px}.gauge-label{font-size:14px}
.gauge.large svg{width:96px;height:96px}.gauge.large .gauge-score{height:96px;font-size:30px}
.grid{grid-template-columns:minmax(0,1fr)}
summary.audit{grid-template-columns:14px minmax(0,1fr) 16px}
summary.audit::after{grid-column:3;grid-row:1}
.audit-summary{grid-column:2;text-align:left}
.cell-bar{display:none}
.opportunities li{grid-template-columns:44px minmax(0,1fr)}
.opportunities .bar-gain{display:none}
.opportunities .to,.changes .to{grid-column:2;white-space:normal}
}
</style>
<noscript><style>[role=tabpanel][hidden]{display:block}.tabs,.expand{display:none}.metric-description{display:block}</style></noscript>
</head>
<body>
"#;

/// Switches subject tabs on click and with the arrow, Home, and End keys, and expands each
/// category's metric descriptions.
const SCRIPT: &str = r"<script>
const tabs = [...document.querySelectorAll('[role=tab]')]
const select = (tab) => {
  for (const other of tabs) {
    const selected = other === tab
    other.setAttribute('aria-selected', String(selected))
    other.tabIndex = selected ? 0 : -1
    document.getElementById(other.getAttribute('aria-controls')).hidden = !selected
  }
}
tabs.forEach((tab, index) => {
  tab.addEventListener('click', () => select(tab))
  tab.addEventListener('keydown', (event) => {
    const next = { ArrowRight: index + 1, ArrowLeft: index - 1, Home: 0, End: tabs.length - 1 }[event.key]
    if (next === undefined) return
    const target = tabs[(next + tabs.length) % tabs.length]
    select(target)
    target.focus()
    event.preventDefault()
  })
})
for (const button of document.querySelectorAll('.expand')) {
  button.addEventListener('click', () => {
    const expanded = button.getAttribute('aria-expanded') !== 'true'
    button.setAttribute('aria-expanded', String(expanded))
    button.textContent = expanded ? 'Collapse view' : 'Expand view'
    button.closest('.category').classList.toggle('expanded', expanded)
  })
}
</script>
";
