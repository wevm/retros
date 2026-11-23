//! The dashboard: the latest noted runs of several repositories, as a terminal summary and as one
//! self-contained HTML page that makes no network requests. `uphill dashboard` reads each
//! repository's runs from `refs/notes/uphill` and its newest run's `report.json` from
//! `refs/notes/uphill-report`.
//!
//! Each subject of each repository is a row. A row shows the category scores of the newest run,
//! the trend of each category across the runs, and the metrics, targets, and opportunities of
//! the report note. Without a report note, a row shows the values of the compact note instead.

use std::fmt::Write;

use crate::config;
use crate::format;
use crate::history::{Ran, Run};
use crate::html::{self, band_class, chip, escape};
use crate::json::Json;
use crate::report;
use crate::value::Kind;

/// The dashboard's rules after the report's: each row names its subject and run under the
/// repository, and every table of metrics shares one column layout.
const STYLE: &str = ".compare th[scope=row]{white-space:normal}.compare th[scope=row] span{display:block;font-weight:400;font-size:13px}.scores{table-layout:fixed}.scores th:not(:first-child),.scores td:not(:first-child){width:104px;text-align:right}@media (max-width:600px){.scores th:not(:first-child),.scores td:not(:first-child){width:76px}}\n";

/// One repository and its noted runs.
#[derive(Clone, Debug, PartialEq)]
pub struct Repository {
    /// The repository as the command line names it, such as `wevm/uphill`.
    pub name: String,
    /// The `report.json` noted on the newest run, or `None` when that run has no report note.
    pub report: Option<String>,
    /// The runs noted on the first-parent history, oldest first. The last is the newest.
    pub runs: Vec<Run>,
}

/// One subject of one repository, with the categories of its newest run.
struct Row<'a> {
    categories: Vec<&'a Ran>,
    repository: &'a Repository,
    /// The newest run's report note, when it has one and it reads as JSON.
    report: Option<&'a Json>,
    run: &'a Run,
    subject: Option<&'a str>,
}

impl Row<'_> {
    /// Returns the report note's object for a category of this row.
    fn category(&self, name: &str) -> Option<&Json> {
        self.report?
            .get("categories")?
            .items()
            .iter()
            .find(|category| {
                category.get("name").and_then(Json::text) == Some(name)
                    && category.get("subject").and_then(Json::text) == self.subject
            })
    }

    /// Returns the title of a category: its title in the report note, or else its name in words.
    fn title(&self, name: &str) -> String {
        self.category(name)
            .and_then(|category| category.get("title")?.text())
            .map_or_else(|| config::words(name), str::to_owned)
    }

    /// Returns the repository with its subject, such as `wevm/zepto, zc`.
    fn label(&self) -> String {
        match self.subject {
            Some(subject) => format!("{}, {subject}", self.repository.name),
            None => self.repository.name.clone(),
        }
    }

    /// Returns the scores of a category across the repository's runs, oldest first, each named
    /// by its commit and date.
    fn trend(&self, name: &str) -> Vec<(String, u8)> {
        self.repository
            .runs
            .iter()
            .filter_map(|run| {
                let score = run.category(name, self.subject)?.score?;
                Some((format!("{} · {}", run.commit, run.date), score))
            })
            .collect()
    }
}

/// Splits each repository's newest run into one row per subject, in the order the run reported
/// them. A repository without runs has no rows.
fn rows<'a>(repositories: &'a [Repository], reports: &'a [Option<Json>]) -> Vec<Row<'a>> {
    let mut rows = Vec::new();
    for (repository, report) in repositories.iter().zip(reports) {
        let Some(run) = repository.runs.last() else {
            continue;
        };
        let mut subjects: Vec<Option<&str>> = Vec::new();
        for ran in &run.categories {
            if !subjects.contains(&ran.subject.as_deref()) {
                subjects.push(ran.subject.as_deref());
            }
        }
        for subject in subjects {
            rows.push(Row {
                categories: run
                    .categories
                    .iter()
                    .filter(|ran| ran.subject.as_deref() == subject)
                    .collect(),
                repository,
                report: report.as_ref(),
                run,
                subject,
            });
        }
    }
    rows
}

/// Parses each repository's report note. A note that is not JSON counts as none.
fn reports(repositories: &[Repository]) -> Vec<Option<Json>> {
    repositories
        .iter()
        .map(|repository| repository.report.as_deref().and_then(Json::parse))
        .collect()
}

/// Renders the dashboard for the terminal: each repository's newest noted run, then the score
/// of each of its categories.
pub fn terminal(repositories: &[Repository]) -> String {
    let mut out = String::new();
    let reports = reports(repositories);

    for (index, (repository, report)) in repositories.iter().zip(&reports).enumerate() {
        if index > 0 {
            out.push('\n');
        }
        let Some(run) = repository.runs.last() else {
            let _ = writeln!(out, "{}  no noted runs", repository.name);
            continue;
        };
        let source = if report.is_some() {
            "report note"
        } else {
            "compact note"
        };
        let _ = writeln!(
            out,
            "{}  {}  {}  {source}",
            repository.name, run.commit, run.date
        );
        for ran in &run.categories {
            let (band, shown) = match ran.score {
                Some(score) => (report::band(score), score.to_string()),
                None => ('–', "–".to_owned()),
            };
            let label = report::label(ran.subject.as_deref(), &ran.name);
            let _ = writeln!(out, "  {band} {shown:>3}  {label}");
        }
    }
    out
}

/// Renders the dashboard as one HTML page: a table of every row's category scores, then a
/// section per row with each category's trend and the newest run's metrics.
pub fn html(repositories: &[Repository]) -> String {
    let reports = reports(repositories);
    let rows = rows(repositories, &reports);

    let mut out = html::head("Uphill dashboard", STYLE);
    if repositories
        .iter()
        .any(|repository| repository.runs.len() > 1)
    {
        out.push_str(html::TREND_DEFS);
    }
    out.push_str("<main>\n<h1>Uphill dashboard</h1>\n");

    // Categories in the order rows first report them, so related repositories line up.
    let mut names: Vec<&str> = Vec::new();
    for row in &rows {
        for ran in &row.categories {
            if !names.contains(&ran.name.as_str()) {
                names.push(&ran.name);
            }
        }
    }

    out.push_str("<div class=\"scroll\"><table class=\"compare\">\n<thead><tr><th scope=\"col\">Repository</th>");
    for name in &names {
        let _ = write!(
            out,
            "<th scope=\"col\">{}</th>",
            escape(&config::words(name))
        );
    }
    out.push_str("</tr></thead>\n<tbody>\n");
    for (index, row) in rows.iter().enumerate() {
        let run = match row.subject {
            Some(subject) => format!("{subject}, {}", row.run.commit),
            None => row.run.commit.clone(),
        };
        let _ = write!(
            out,
            "<tr><th scope=\"row\"><a href=\"#row-{index}\">{}</a><span class=\"muted\">{}</span></th>",
            escape(&row.repository.name),
            escape(&run)
        );
        for name in &names {
            let cell = row
                .categories
                .iter()
                .find(|ran| ran.name == *name)
                .map_or_else(
                    || "<span class=\"muted\">–</span>".to_owned(),
                    |ran| chip(ran.score),
                );
            let _ = write!(out, "<td>{cell}</td>");
        }
        out.push_str("</tr>\n");
    }
    out.push_str("</tbody>\n</table></div>\n");

    let empty: Vec<&str> = repositories
        .iter()
        .filter(|repository| repository.runs.is_empty())
        .map(|repository| repository.name.as_str())
        .collect();
    if !empty.is_empty() {
        let _ = writeln!(
            out,
            "<p class=\"muted\">No noted runs: {}.</p>",
            escape(&empty.join(", "))
        );
    }
    out.push_str(html::SCALE);

    for (index, row) in rows.iter().enumerate() {
        section(&mut out, row, index);
    }

    out.push_str("</main>\n");
    out.push_str(html::SCRIPT);
    out.push_str("</body>\n</html>\n");
    out
}

/// Writes one row's section: the newest run, each category's score and trend, and the run's
/// metrics and opportunities from its report note, or its values from its compact note.
fn section(out: &mut String, row: &Row<'_>, index: usize) {
    let source = if row.report.is_some() {
        "From its report note."
    } else {
        "From its compact note, which has no targets or metric scores."
    };
    let _ = writeln!(
        out,
        "<section class=\"block\" id=\"row-{index}\">\n<h2>{}</h2>\n<p class=\"muted\">Newest noted run: <code>{}</code> on {}, {}. {source}</p>",
        escape(&row.label()),
        escape(&row.run.commit),
        escape(&row.run.date),
        escape(&row.run.title)
    );

    out.push_str("<ul class=\"grid\">\n");
    for ran in &row.categories {
        let _ = write!(
            out,
            "<li class=\"metric\"><div class=\"metric-head\"><span class=\"metric-title\">{}</span><span class=\"metric-score {}\">{}</span></div>",
            escape(&row.title(&ran.name)),
            band_class(ran.score),
            ran.score
                .map_or_else(|| "–".to_owned(), |score| score.to_string())
        );
        let trend = row.trend(&ran.name);
        if trend.len() > 1 {
            out.push_str(&html::score_trend(&trend, &row.title(&ran.name)));
        }
        out.push_str("</li>\n");
    }
    out.push_str("</ul>\n");

    if row.report.is_some() {
        for ran in &row.categories {
            metrics(out, row, &ran.name);
        }
        opportunities(out, row);
    } else {
        out.push_str("<pre>");
        for ran in &row.categories {
            let header = report::label(ran.subject.as_deref(), &ran.name);
            let score = ran
                .score
                .map(|score| format!(" {score}"))
                .unwrap_or_default();
            let _ = write!(out, "[{}{score}]\n{}", escape(&header), escape(&ran.source));
        }
        out.push_str("</pre>\n");
    }
    out.push_str("</section>\n");
}

/// Writes a table of one category's metrics from the report note: each value against its
/// target, and its score.
fn metrics(out: &mut String, row: &Row<'_>, name: &str) {
    let Some(category) = row.category(name) else {
        return;
    };
    let metrics = category.get("metrics").map_or(&[][..], Json::items);
    let _ = writeln!(
        out,
        "<div class=\"group-head\"><h3>{}</h3></div>",
        escape(&row.title(name))
    );
    if let Some(error) = category
        .get("error")
        .and_then(|error| error.get("message")?.text())
    {
        let _ = writeln!(out, "<p class=\"failure\">✗ {}</p>", escape(error));
        return;
    }

    out.push_str("<table class=\"items scores\"><thead><tr><th scope=\"col\">Metric</th><th scope=\"col\">Value</th><th scope=\"col\">Target</th><th scope=\"col\">Score</th></tr></thead><tbody>\n");
    for metric in metrics {
        let kind = metric.get("kind").and_then(Json::text).and_then(kind);
        let score = metric.get("score").and_then(Json::number).and_then(score);
        let _ = writeln!(
            out,
            "<tr><td>{}</td><td class=\"value\">{}</td><td class=\"value\">{}</td><td>{}</td></tr>",
            escape(metric.get("title").and_then(Json::text).unwrap_or_default()),
            escape(&shown(kind, metric.get("value"))),
            escape(&shown(kind, metric.get("target"))),
            chip(score)
        );
    }
    out.push_str("</tbody></table>\n");
}

/// Writes the report note's opportunities for this row's subject, largest first.
fn opportunities(out: &mut String, row: &Row<'_>) {
    let all = row
        .report
        .and_then(|report| report.get("opportunities"))
        .map_or(&[][..], Json::items);
    let mine: Vec<&Json> = all
        .iter()
        .filter(|opportunity| opportunity.get("subject").and_then(Json::text) == row.subject)
        .collect();
    if mine.is_empty() {
        return;
    }

    out.push_str(
        "<div class=\"group-head\"><h3>Opportunities</h3></div>\n<ul class=\"opportunities\">\n",
    );
    let most = mine
        .iter()
        .filter_map(|opportunity| opportunity.get("gain")?.number())
        .fold(0.0, f64::max);
    for opportunity in mine {
        let text = |key: &str| opportunity.get(key).and_then(Json::text);
        let category = text("category").unwrap_or_default();
        let metric_name = text("metric").unwrap_or_default();
        let metric = row.category(category).and_then(|category| {
            category
                .get("metrics")?
                .items()
                .iter()
                .find(|metric| metric.get("name").and_then(Json::text) == Some(metric_name))
        });
        let kind = metric
            .and_then(|metric| metric.get("kind")?.text())
            .and_then(kind);
        let title = metric
            .and_then(|metric| metric.get("title")?.text())
            .map_or_else(|| config::words(metric_name), str::to_owned);
        let what = match text("item") {
            Some(item) => format!("{title} {item}"),
            None => title,
        };
        let gain = opportunity
            .get("gain")
            .and_then(Json::number)
            .unwrap_or(0.0);
        let width = if most > 0.0 { gain / most * 100.0 } else { 0.0 };
        let _ = writeln!(
            out,
            "<li><span class=\"gain\">+{}</span><span class=\"bar-gain\" aria-hidden=\"true\"><span style=\"width:{width:.0}%\"></span></span><span class=\"what\"><span class=\"muted\">{}</span> {}</span><span class=\"to\">{} → {}</span></li>",
            format::amount(Kind::Count, gain),
            escape(&row.title(category)),
            escape(&what),
            escape(&shown(kind, opportunity.get("value"))),
            escape(&shown(kind, opportunity.get("target")))
        );
    }
    out.push_str("</ul>\n");
}

/// Reads a metric kind as `report.json` names it.
fn kind(name: &str) -> Option<Kind> {
    match name {
        "check" => Some(Kind::Check),
        "count" => Some(Kind::Count),
        "size" => Some(Kind::Size),
        "time" => Some(Kind::Time),
        _ => None,
    }
}

/// Reads a score from JSON, or `None` outside 0 to 100.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a rounded number from 0 to 100 fits a u8 exactly"
)]
fn score(value: f64) -> Option<u8> {
    (0.0..=100.0).contains(&value).then(|| value.round() as u8)
}

/// Formats a value from `report.json` in base units. A check reads `pass` or `fail`, and a list
/// of checks reads the share that pass. A missing value reads `–`.
fn shown(kind: Option<Kind>, value: Option<&Json>) -> String {
    match (kind, value) {
        (_, Some(Json::Bool(true))) => "pass".to_owned(),
        (_, Some(Json::Bool(false))) => "fail".to_owned(),
        (Some(Kind::Check), Some(Json::Number(share))) if *share >= 1.0 => "pass".to_owned(),
        (Some(Kind::Check), Some(Json::Number(share))) if *share <= 0.0 => "fail".to_owned(),
        (Some(Kind::Check), Some(Json::Number(share))) => {
            format!("{}% pass", format::amount(Kind::Count, share * 100.0))
        }
        (Some(kind), Some(Json::Number(amount))) => format::amount(kind, *amount),
        _ => "–".to_owned(),
    }
}
