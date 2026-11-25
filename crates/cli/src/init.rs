//! `uphill init`: prints a brief that a person gives their agent to set up Uphill in the project.
//! The brief points at the example closest to the project in the current directory: a Rust API
//! for a Cargo project, a website for a package with pages or a web framework, and a TypeScript
//! library for any other package, with the pack of conventions for its language. The brief ends with the `devising-metrics` skill in the
//! repository's `SKILL.md`, so the binary and the repository share one copy.

use std::fmt::Write as _;
use std::io::Write as _;
use std::path::Path;
use std::process::ExitCode;

use crate::project;

/// The brief, with `{example}` where the closest example goes and `{skill}` where the skill goes.
const BRIEF: &str = include_str!("init.md");

/// The skill that guides the agent to devise metrics and targets, with its frontmatter.
const SKILL: &str = include_str!("../../../SKILL.md");

/// Where the examples live.
const EXAMPLES: &str = "https://github.com/wevm/uphill/tree/main/examples";

/// Dependencies in `package.json` that mark a website.
const WEB: [&str; 6] = [
    "\"astro\"",
    "\"lighthouse\"",
    "\"next\"",
    "\"react-dom\"",
    "\"@sveltejs/kit\"",
    "\"vite\"",
];

/// Prints the brief for the project in the current directory.
pub(crate) fn run() -> ExitCode {
    let cwd = std::env::current_dir().unwrap_or_default();

    let mut example = match closest(&cwd) {
        Some((name, kind, pack)) => format!(
            "The closest example is {kind}: {EXAMPLES}/{name}. Read its `uphill.yaml` and `metrics/` before writing anything. `uphill add wevm/uphill/packs/{pack}@main` adds the `{pack}` pack of conventions, pinned to a commit."
        ),
        None => format!(
            "Read the closest example before writing anything: a TypeScript library ({EXAMPLES}/typescript-library), a website ({EXAMPLES}/web-app), or a Rust API ({EXAMPLES}/rust-api)."
        ),
    };
    if let Ok(project) = project::find(&cwd) {
        let _ = write!(
            example,
            " This project already has `{}`. Extend it instead of starting over.",
            project.name
        );
    }

    // The frontmatter is only for agents that load skills, so the brief takes the skill from its title.
    let skill = SKILL
        .strip_prefix("---\n")
        .and_then(|rest| rest.split_once("\n---\n"))
        .map_or(SKILL, |(_, body)| body)
        .trim_start();

    let brief = BRIEF
        .replace("{example}", &example)
        .replace("{skill}", skill);
    let _ = std::io::stdout().write_all(brief.as_bytes());
    ExitCode::SUCCESS
}

/// Returns the name and kind of the example closest to the project at `dir`, and the pack for its
/// language, or `None` when it is neither a Cargo project nor a package.
fn closest(dir: &Path) -> Option<(&'static str, &'static str, &'static str)> {
    if dir.join("Cargo.toml").is_file() {
        return Some(("rust-api", "a Rust API", "rust"));
    }
    let package = std::fs::read_to_string(dir.join("package.json")).ok()?;
    let web = WEB.iter().any(|name| package.contains(name))
        || dir.join("index.html").is_file()
        || dir.join("pages").is_dir();
    Some(if web {
        ("web-app", "a website", "web")
    } else {
        ("typescript-library", "a TypeScript library", "typescript")
    })
}
