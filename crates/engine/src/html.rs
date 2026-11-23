//! The HTML report: one self-contained page, with inline styles and a small inline script, that
//! makes no network requests. It shows a tab per subject, a gauge per category, each metric with
//! its value, target, and description, the worst items of each list, the opportunities that would
//! raise each category most, and the changes since `retros.lock`. Like the other reports, it has
//! no timestamps or paths, so the same scores always produce the same bytes.

use std::fmt::Write;

use crate::format;
use crate::lock::{Change, Direction};
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
        let _ = write!(out, "<th scope=\"col\">{}</th>", escape(name));
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
            escape(&category.name)
        );
    }
    out.push_str("</nav>\n");

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
            let _ = writeln!(
                out,
                "<li><span class=\"gain\">{}</span><span class=\"bar\" aria-hidden=\"true\"><span style=\"width:{:.0}%\"></span></span><span class=\"what\"><span class=\"muted\">{}</span> {}</span><span class=\"to\">{} → {}</span></li>",
                escape(&report::gain(opportunity)),
                opportunity.gain / most * 100.0,
                escape(&category.name),
                escape(&report::subject(opportunity)),
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
        out.push_str("<section class=\"block\">\n<h2>Changes since retros.lock</h2>\n");
        if changes.is_empty() {
            out.push_str("<p class=\"muted\">No changes.</p>\n");
        } else {
            out.push_str("<ul class=\"changes\">\n");
            for change in changes {
                let _ = writeln!(
                    out,
                    "<li><span class=\"chip {}\">{}</span><span class=\"what\"><span class=\"muted\">{}</span> {}</span><span class=\"to\">{}</span></li>",
                    direction_class(change.direction),
                    change.direction.as_str(),
                    escape(&change.category),
                    escape(&report::changed(change)),
                    escape(&report::values(change))
                );
            }
            out.push_str("</ul>\n");
        }
        out.push_str("</section>\n");
    }

    for category in categories {
        let _ = writeln!(
            out,
            "<section class=\"category\" id=\"{}\">",
            anchor(category)
        );
        let score = category
            .result
            .as_ref()
            .ok()
            .and_then(|scored| scored.score);
        let _ = writeln!(
            out,
            "<h2><span class=\"mini {}\">{}</span>{}</h2>",
            band_class(score),
            score.map_or_else(|| "–".to_owned(), |score| score.to_string()),
            escape(&category.name)
        );
        if let Some(description) = &category.description {
            let _ = writeln!(out, "<p class=\"muted\">{}</p>", inline(description));
        }

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
            Ok(scored) => metrics(out, scored),
        }
        out.push_str("</section>\n");
    }
}

/// Writes a category's metrics in the order `retros.yaml` declares them.
fn metrics(out: &mut String, scored: &CategoryScore) {
    if scored.rounds > 1 {
        let _ = writeln!(
            out,
            "<p class=\"muted\">Values that vary between runs are the median of {} rounds.</p>",
            scored.rounds
        );
    }

    out.push_str("<ul class=\"metrics\">\n");
    for metric in &scored.metrics {
        let row = row(metric);
        let shown: Vec<_> = metric.items.iter().take(ITEMS).collect();

        if shown.is_empty() {
            let _ = writeln!(out, "<li><div class=\"row\">{row}</div></li>");
            continue;
        }

        // Checks have no targets, so their table leaves the column out.
        let check = metric.kind == Kind::Check;
        let _ = writeln!(
            out,
            "<li><details><summary class=\"row\">{row}</summary>\n<div class=\"scroll\"><table class=\"items\">\n<thead><tr><th scope=\"col\">Item</th><th scope=\"col\">Value</th>{}</tr></thead>\n<tbody>",
            if check {
                ""
            } else {
                "<th scope=\"col\">Target</th>"
            }
        );
        for item in &shown {
            let value = match item.value {
                Value::Check(passed) => format!(
                    "<span class=\"chip {}\">{}</span>",
                    if passed { "good" } else { "poor" },
                    if passed { "pass" } else { "fail" }
                ),
                value => escape(&format::value(value)),
            };
            let target = if check {
                String::new()
            } else {
                format!(
                    "<td>{}</td>",
                    item.target
                        .or(metric.metric.target)
                        .map(|target| escape(&format::value(target)))
                        .unwrap_or_default()
                )
            };
            let _ = writeln!(
                out,
                "<tr><td>{}</td><td>{value}</td>{target}</tr>",
                escape(&item.name)
            );
        }
        out.push_str("</tbody>\n</table></div>\n");
        if metric.items.len() > ITEMS {
            let _ = writeln!(
                out,
                "<p class=\"muted\">And {} more in report.json.</p>",
                metric.items.len() - ITEMS
            );
        }
        out.push_str("</details></li>\n");
    }
    out.push_str("</ul>\n");
}

/// Renders a metric's row: its band, score, name, description, value, and target.
fn row(metric: &MetricScore) -> String {
    let mut row = format!(
        "<span class=\"shape {}\" aria-hidden=\"true\"></span><span class=\"score\">{}</span><span class=\"name\">{}",
        band_class(metric.score),
        metric
            .score
            .map_or_else(|| "–".to_owned(), |score| score.to_string()),
        escape(&metric.metric.name)
    );
    if let Some(description) = &metric.metric.description {
        let _ = write!(
            row,
            "<span class=\"description\">{}</span>",
            inline(description)
        );
    }
    let _ = write!(
        row,
        "</span><span class=\"value\">{}</span><span class=\"target\">{}</span>",
        escape(&report::value(metric)),
        escape(&report::target(metric))
    );
    if metric.kind != Kind::Check && metric.items.len() > 1 {
        let _ = write!(
            row,
            "<span class=\"count\">{} items</span>",
            metric.items.len()
        );
    }
    row
}

/// Renders a score gauge: a ring filled to the score in its band's color.
fn gauge(score: Option<u8>, failed: bool) -> String {
    let (fill, text) = match score {
        Some(score) => (score, score.to_string()),
        None if failed => (0, "✗".to_owned()),
        None => (0, "–".to_owned()),
    };
    format!(
        "<svg viewBox=\"0 0 120 120\" aria-hidden=\"true\"><circle class=\"track\" cx=\"60\" cy=\"60\" r=\"52\"/><circle class=\"arc\" cx=\"60\" cy=\"60\" r=\"52\" pathLength=\"100\" stroke-dasharray=\"{fill} 100\" transform=\"rotate(-90 60 60)\"/></svg><span class=\"gauge-score\">{text}</span>"
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
h2{font-size:18px;margin:0 0 8px;display:flex;align-items:center;gap:10px}
a{color:inherit}
:focus-visible{outline:2px solid var(--focus);outline-offset:2px}
.muted{color:var(--muted)}
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
.gauges{display:flex;flex-wrap:wrap;justify-content:center;gap:12px 28px;padding:12px 0 24px;border-bottom:1px solid var(--line);margin-bottom:24px}
.gauge{position:relative;display:flex;flex-direction:column;align-items:center;min-width:104px;text-decoration:none;border-radius:8px;padding:4px}
.gauge svg{width:88px;height:88px}
.gauge .track{fill:var(--bg);stroke:var(--soft);stroke-width:8}
.gauge.good .track{fill:var(--good-bg)}.gauge.average .track{fill:var(--average-bg)}.gauge.poor .track{fill:var(--poor-bg)}
.gauge .arc{fill:none;stroke-width:8;stroke-linecap:round;stroke:var(--none);animation:fill .8s ease-out}
.gauge.good .arc{stroke:var(--good)}.gauge.average .arc{stroke:var(--average)}.gauge.poor .arc{stroke:var(--poor)}
.gauge-score{position:absolute;top:4px;left:0;right:0;height:88px;display:flex;align-items:center;justify-content:center;font-size:26px;font-weight:600;font-variant-numeric:tabular-nums}
.gauge.good .gauge-score{color:var(--good-fg)}.gauge.average .gauge-score{color:var(--average-fg)}.gauge.poor .gauge-score{color:var(--poor-fg)}.gauge.none .gauge-score{color:var(--none-fg)}
.gauge-label{margin-top:6px;font-weight:600;text-align:center;white-space:nowrap}
@keyframes fill{from{stroke-dasharray:0 100}}
@media (prefers-reduced-motion:reduce){.gauge .arc{animation:none}}
.block,.category{margin:0 0 32px}
.mini{display:inline-flex;align-items:center;justify-content:center;flex:none;width:42px;height:42px;border-radius:50%;font-size:14px;font-variant-numeric:tabular-nums;font-weight:600;border:3px solid var(--none);color:var(--none-fg)}
.mini.good{border-color:var(--good);color:var(--good-fg)}.mini.average{border-color:var(--average);color:var(--average-fg)}.mini.poor{border-color:var(--poor);color:var(--poor-fg)}
.metrics,.opportunities,.changes{list-style:none;margin:0;padding:0;border-top:1px solid var(--line)}
.metrics>li,.opportunities>li,.changes>li{border-bottom:1px solid var(--line)}
.row{display:grid;grid-template-columns:14px 34px minmax(0,1fr) auto minmax(110px,auto);align-items:baseline;gap:4px 12px;padding:10px 4px}
summary.row{cursor:pointer;list-style:none;grid-template-columns:14px 34px minmax(0,1fr) auto minmax(110px,auto) 16px}
summary.row::-webkit-details-marker{display:none}
summary.row::after{grid-column:6;grid-row:1;content:"";width:8px;height:8px;border-right:2px solid var(--muted);border-bottom:2px solid var(--muted);transform:rotate(45deg);align-self:center;justify-self:center}
details[open]>summary.row::after{transform:rotate(-135deg)}
summary.row:hover{background:var(--soft)}
.shape{width:12px;height:12px;align-self:start;margin-top:5px;background:var(--none)}
.shape.good{background:var(--good);border-radius:50%}
.shape.average{background:var(--average)}
.shape.poor{width:0;height:0;background:none;border-left:6px solid transparent;border-right:6px solid transparent;border-bottom:11px solid var(--poor)}
.shape.none{border-radius:50%;background:var(--none)}
.score{font-weight:600;font-variant-numeric:tabular-nums}
.name{font-weight:600;overflow-wrap:anywhere}
.description{display:block;font-weight:400;color:var(--muted);font-size:14px}
code{font:13px ui-monospace,SFMono-Regular,Menlo,monospace;background:var(--soft);padding:1px 4px;border-radius:4px}
.value{font-variant-numeric:tabular-nums;text-align:right;overflow-wrap:anywhere}
.target{color:var(--muted);text-align:right}
.count{grid-column:3;grid-row:2;color:var(--muted);font-size:13px}
.items{margin:0 0 12px;font-size:14px}
.items td:nth-child(2),.items td:nth-child(3),.items th:nth-child(2),.items th:nth-child(3){text-align:right}
.items td:first-child{white-space:normal;overflow-wrap:anywhere}
.opportunities li,.changes li{display:grid;grid-template-columns:auto minmax(0,1fr) auto;align-items:center;gap:4px 12px;padding:10px 4px}
.opportunities li{grid-template-columns:44px 120px minmax(0,1fr) auto}
.gain{font-weight:600;color:var(--good-fg);font-variant-numeric:tabular-nums}
.bar{height:8px;border-radius:4px;background:var(--soft);overflow:hidden}
.bar span{display:block;height:100%;background:var(--good)}
.what{overflow-wrap:anywhere}
.to{font-variant-numeric:tabular-nums;white-space:nowrap}
.failure{color:var(--poor-fg);font-weight:600}
pre{background:var(--soft);padding:12px;border-radius:6px;overflow-x:auto;font-size:13px}
@media (max-width:600px){
main{padding:16px 12px 48px}
.gauges{gap:8px 12px}
.gauge{min-width:84px}.gauge svg{width:68px;height:68px}.gauge-score{height:68px;font-size:20px}.gauge-label{font-size:14px}
.row,summary.row{grid-template-columns:14px 34px minmax(0,1fr) 16px}
.row:not(summary){grid-template-columns:14px 34px minmax(0,1fr)}
.value,.target{grid-column:3;grid-row:auto;text-align:left}
.count{grid-row:auto}
summary.row::after{grid-column:4;grid-row:1}
.opportunities li{grid-template-columns:44px minmax(0,1fr)}
.opportunities .bar{display:none}
.opportunities .to,.changes .to{grid-column:2;white-space:normal}
}
</style>
<noscript><style>[role=tabpanel][hidden]{display:block}.tabs{display:none}</style></noscript>
</head>
<body>
"#;

/// Switches subject tabs on click and with the arrow, Home, and End keys.
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
</script>
";
