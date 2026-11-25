//! Scans a pack's files for what their code can do, and renders the review that `uphill add` and
//! `uphill update` print before they write a pack.
//!
//! The scan matches patterns in source text, line by line, and skips comment lines. It reads
//! TypeScript and JavaScript, shell scripts, the pack's configuration, and `package.json`. Every
//! text file is also checked for content a reviewer cannot read: binaries, minified lines,
//! high-entropy strings, and bidirectional or zero-width characters. A finding is high severity
//! when it defeats review or reaches for secrets, and information otherwise, since running tools
//! is a pack's purpose.
//!
//! ```text
//! capabilities
//! + high  remote code   npx            metrics/slop.ts:12
//!   info  process       cargo          metrics/rust.ts:221
//! - info  network       example.com
//! ```

use std::fmt::Write;

use crate::format;
use crate::pack;
use crate::value::Kind;

/// The longest line that the scan reads as source rather than minified code.
const LONG_LINE: usize = 500;

/// A file of a pack.
#[derive(Clone, Copy, Debug)]
pub struct File<'a> {
    /// The file's contents.
    pub bytes: &'a [u8],
    /// Whether the file is executable.
    pub executable: bool,
    /// The path relative to the pack's directory.
    pub path: &'a str,
}

/// How much a finding matters to a reviewer. The order is observable: high findings list first.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Severity {
    /// Code that defeats review or reaches for secrets.
    High,
    /// An inventory entry, such as an executable the pack runs.
    Info,
}

/// One thing a pack's code can do, at its first place in the files.
#[derive(Clone, Debug, PartialEq)]
pub struct Finding {
    /// The kind, such as `network` or `remote code`.
    pub capability: &'static str,
    /// What the code names, such as `fetch(`, `example.com`, or `GITHUB_TOKEN`.
    pub detail: String,
    /// The line of the first place, from 1, or 0 for a whole file.
    pub line: usize,
    /// The path of the file with the first place.
    pub path: String,
    /// How much the finding matters.
    pub severity: Severity,
}

/// Patterns in TypeScript and JavaScript, each with its capability and severity.
const CODE: &[(&str, &str, Severity)] = &[
    ("Bun.$", "process", Severity::Info),
    ("Bun.connect", "network", Severity::Info),
    ("Bun.listen", "network", Severity::Info),
    ("Bun.serve", "network", Severity::Info),
    ("Bun.spawn", "process", Severity::Info),
    ("EventSource", "network", Severity::Info),
    ("WebAssembly.", "dynamic code", Severity::High),
    ("WebSocket", "network", Severity::Info),
    ("XMLHttpRequest", "network", Severity::Info),
    ("bun:ffi", "dynamic code", Severity::High),
    ("child_process", "process", Severity::Info),
    ("eval(", "dynamic code", Severity::High),
    ("execa", "process", Severity::Info),
    ("fetch(", "network", Severity::Info),
    ("new Function", "dynamic code", Severity::High),
    ("node:dgram", "network", Severity::Info),
    ("node:dns", "network", Severity::Info),
    ("node:http", "network", Severity::Info),
    ("node:net", "network", Severity::Info),
    ("node:tls", "network", Severity::Info),
    ("node:vm", "dynamic code", Severity::High),
    ("process.binding", "dynamic code", Severity::High),
    ("process.dlopen", "dynamic code", Severity::High),
    ("sendBeacon", "network", Severity::Info),
];

/// Words in any source file that run code from the network, or fetch over it.
const WORDS: &[(&str, &str, Severity)] = &[
    ("bunx", "remote code", Severity::High),
    ("curl", "network", Severity::Info),
    ("dlx", "remote code", Severity::High),
    ("npx", "remote code", Severity::High),
    ("uvx", "remote code", Severity::High),
    ("wget", "network", Severity::Info),
];

/// Paths that hold credentials.
const SECRET_PATHS: &[&str] = &[
    ".aws/credentials",
    ".git-credentials",
    ".netrc",
    ".npmrc",
    ".ssh/",
    "/etc/passwd",
    "id_ed25519",
    "id_rsa",
];

/// Calls whose first argument names an executable.
const SPAWNS: &[&str] = &[
    "Bun.spawn(",
    "Bun.spawnSync(",
    "execFile(",
    "execFileSync(",
    "execSync(",
    "spawn(",
    "spawnSync(",
];

/// Scans the files of a pack. Each capability and detail appears once, at its first place, with
/// high findings first.
pub fn scan(files: &[File<'_>]) -> Vec<Finding> {
    let mut findings: Vec<Finding> = Vec::new();
    for file in files {
        scan_file(file, &mut findings);
    }

    let mut unique: Vec<Finding> = Vec::new();
    for finding in findings {
        if !unique
            .iter()
            .any(|other| other.capability == finding.capability && other.detail == finding.detail)
        {
            unique.push(finding);
        }
    }
    unique.sort_by_key(|finding| (finding.severity, finding.capability));
    unique
}

/// Adds the findings of one file.
fn scan_file(file: &File<'_>, findings: &mut Vec<Finding>) {
    let path = file.path;
    let name = path.rsplit('/').next().unwrap_or(path);
    let extension = name.rsplit_once('.').map_or("", |(_, extension)| extension);
    let mut add = |capability, detail: &str, line, severity| {
        findings.push(Finding {
            capability,
            detail: pack::printable(detail),
            line,
            path: path.to_owned(),
            severity,
        });
    };

    let native = matches!(
        extension,
        "dll" | "dylib" | "exe" | "jar" | "node" | "so" | "wasm"
    );
    let magic = [
        &b"\x7fELF"[..],
        b"MZ",
        b"\xfe\xed\xfa\xce",
        b"\xfe\xed\xfa\xcf",
        b"\xcf\xfa\xed\xfe",
        b"\xca\xfe\xba\xbe",
        b"\0asm",
    ];
    let head = file.bytes.get(..8_000).unwrap_or(file.bytes);
    let text = std::str::from_utf8(file.bytes).ok();
    let Some(text) = text.filter(|_| {
        !native && !head.contains(&0) && !magic.iter().any(|m| file.bytes.starts_with(m))
    }) else {
        add("unreviewable", "binary file", 0, Severity::High);
        return;
    };

    let lockfile = matches!(
        name,
        "bun.lock" | "package-lock.json" | "pnpm-lock.yaml" | "yarn.lock"
    );
    let script = matches!(
        extension,
        "cjs" | "cts" | "js" | "jsx" | "mjs" | "mts" | "ts" | "tsx"
    );
    let shell = matches!(extension, "bash" | "sh" | "zsh") || text.starts_with("#!");
    let config = matches!(name, "uphill.toml" | "uphill.yaml");
    let source = script || shell || config;

    for (index, line) in text.lines().enumerate() {
        let number = index + 1;
        let hidden = line
            .char_indices()
            .any(|(at, c)| pack::hidden(c) && !(index == 0 && at == 0 && c == '\u{feff}'));
        if hidden {
            add(
                "unreviewable",
                "bidi or zero-width character",
                number,
                Severity::High,
            );
        }
        if !source || lockfile {
            continue;
        }

        let trimmed = line.trim_start();
        let comment = trimmed.starts_with("//")
            || trimmed.starts_with("/*")
            || trimmed.starts_with('*')
            || (trimmed.starts_with('#') && !trimmed.starts_with("#!"));
        if comment {
            continue;
        }
        if line.len() > LONG_LINE {
            add("unreviewable", "minified line", number, Severity::High);
            continue;
        }
        if let Some(token) = entropic(line) {
            add(
                "unreviewable",
                &format!("high-entropy string {}…", token.get(..12).unwrap_or(token)),
                number,
                Severity::High,
            );
        }

        if script {
            for (pattern, capability, severity) in CODE {
                if line.contains(pattern) {
                    add(capability, pattern, number, *severity);
                }
            }
            for call in ["import(", "require("] {
                if dynamic(line, call) {
                    add("dynamic code", &format!("{call}…)"), number, Severity::High);
                }
            }
            for scheme in [
                "'http:", "'https:", "'npm:", "'jsr:", "\"http:", "\"https:", "\"npm:", "\"jsr:",
            ] {
                if line.contains(&format!("from {scheme}"))
                    || line.contains(&format!("import({scheme}"))
                {
                    add("remote code", "import from a URL", number, Severity::High);
                }
            }
        }
        if (shell || config) && piped_to_shell(line) {
            add("remote code", "pipe into a shell", number, Severity::High);
        }
        for (word, capability, severity) in WORDS {
            if has_word(line, word) {
                add(capability, word, number, *severity);
            }
        }
        for secret in SECRET_PATHS {
            if line.contains(secret) {
                add("secret", secret, number, Severity::High);
            }
        }
        for host in hosts(line) {
            add("network", host, number, Severity::Info);
        }
        for variable in variables(line, shell || config) {
            let upper = variable.to_ascii_uppercase();
            let secret = ["CREDENTIAL", "KEY", "PASSWORD", "SECRET", "TOKEN"]
                .iter()
                .any(|word| upper.contains(word));
            if secret {
                add("secret", variable, number, Severity::High);
            } else {
                add("environment", variable, number, Severity::Info);
            }
        }
        if config {
            for key in ["run:", "setup:", "run =", "setup ="] {
                if let Some(command) = trimmed.strip_prefix(key) {
                    let word = command
                        .trim()
                        .trim_matches(['"', '\''])
                        .split_whitespace()
                        .next();
                    if let Some(word) = word {
                        add("process", word, number, Severity::Info);
                    }
                }
            }
        }
    }

    if script {
        for executable in executables(text) {
            add("process", &executable.0, executable.1, Severity::Info);
        }
    }
    if name == "package.json" {
        for (dependency, spec) in dependencies(text) {
            let exotic = ["file:", "git", "github:", "http:", "https:", "link:"]
                .iter()
                .any(|prefix| spec.starts_with(prefix));
            let severity = if exotic {
                Severity::High
            } else {
                Severity::Info
            };
            add("dependency", &format!("{dependency}@{spec}"), 0, severity);
        }
    }
}

/// Returns whether `line` calls `call` with an argument that is not a string literal.
fn dynamic(line: &str, call: &str) -> bool {
    line.match_indices(call).any(|(at, _)| {
        let before = line.get(..at).and_then(|before| before.chars().last());
        if before.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '.') {
            return false;
        }
        // A call inside a string literal, such as the rule name `'import(no-cycle)'`, runs nothing.
        let quoted = line.get(..at).is_some_and(|before| {
            ['\'', '"', '`']
                .iter()
                .any(|quote| before.matches(*quote).count() % 2 == 1)
        });
        if quoted {
            return false;
        }
        let argument = line.get(at + call.len()..).unwrap_or_default().trim_start();
        !argument.is_empty() && !argument.starts_with(['\'', '"', '`', ')'])
    })
}

/// Returns whether `line` pipes into a shell or an interpreter, such as `curl … | sh`.
fn piped_to_shell(line: &str) -> bool {
    line.split('|').skip(1).any(|after| {
        let word = after.split_whitespace().next().unwrap_or_default();
        matches!(
            word,
            "bash" | "bun" | "node" | "python" | "python3" | "sh" | "zsh"
        )
    })
}

/// Returns whether `line` holds `word` between characters that are not part of a name.
fn has_word(line: &str, word: &str) -> bool {
    let name = |c: char| c.is_alphanumeric() || c == '_' || c == '-';
    line.match_indices(word).any(|(at, _)| {
        let before = line.get(..at).and_then(|before| before.chars().last());
        let after = line
            .get(at + word.len()..)
            .and_then(|after| after.chars().next());
        !before.is_some_and(name) && !after.is_some_and(name)
    })
}

/// Returns the host of each `http` or `https` URL in `line`.
fn hosts(line: &str) -> Vec<&str> {
    let mut hosts = Vec::new();
    for scheme in ["http://", "https://"] {
        for (at, _) in line.match_indices(scheme) {
            let rest = line.get(at + scheme.len()..).unwrap_or_default();
            let end = rest
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '.' || c == '-'))
                .unwrap_or(rest.len());
            let host = rest.get(..end).unwrap_or_default();
            if host.contains('.') && !hosts.contains(&host) {
                hosts.push(host);
            }
        }
    }
    hosts
}

/// Returns each environment variable that `line` reads: `process.env.NAME`,
/// `process.env['NAME']`, and `Bun.env.NAME` in code, `$NAME` and `${NAME}` in shell, and `(all)`
/// for the whole environment.
fn variables(line: &str, shell: bool) -> Vec<&str> {
    let mut names = Vec::new();
    let name_end = |rest: &str| {
        rest.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .unwrap_or(rest.len())
    };
    for prefix in ["process.env", "Bun.env", "import.meta.env"] {
        for (at, _) in line.match_indices(prefix) {
            let rest = line.get(at + prefix.len()..).unwrap_or_default();
            let rest = rest
                .strip_prefix('.')
                .or_else(|| rest.strip_prefix("['"))
                .or_else(|| rest.strip_prefix("[\""));
            let name = rest.map_or("", |rest| rest.get(..name_end(rest)).unwrap_or_default());
            names.push(if name.is_empty() { "(all)" } else { name });
        }
    }
    if shell {
        for (at, _) in line.match_indices('$') {
            let rest = line.get(at + 1..).unwrap_or_default();
            let rest = rest.strip_prefix('{').unwrap_or(rest);
            let name = rest.get(..name_end(rest)).unwrap_or_default();
            if name.len() > 1
                && name
                    .bytes()
                    .all(|byte| byte.is_ascii_uppercase() || byte == b'_' || byte.is_ascii_digit())
            {
                names.push(name);
            }
        }
    }
    names
}

/// Returns the first token of 32 or more characters in `line` whose characters carry more than
/// 4.5 bits each, as encoded secrets and packed code do.
fn entropic(line: &str) -> Option<&str> {
    line.split(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '=' | '_' | '-')))
        .filter(|token| token.len() >= 32)
        .find(|token| {
            let mut counts = [0_u32; 128];
            for byte in token.bytes() {
                if let Some(count) = counts.get_mut(usize::from(byte)) {
                    *count += 1;
                }
            }
            let length = f64::from(u32::try_from(token.len()).unwrap_or(u32::MAX));
            let bits: f64 = counts
                .iter()
                .filter(|count| **count > 0)
                .map(|count| {
                    let p = f64::from(*count) / length;
                    -p * p.log2()
                })
                .sum();
            bits > 4.5
        })
}

/// Returns each executable that a call in `text` spawns by a string literal, with its line.
fn executables(text: &str) -> Vec<(String, usize)> {
    let mut found = Vec::new();
    for call in SPAWNS {
        for (at, _) in text.match_indices(call) {
            let before = text.get(..at).and_then(|before| before.chars().last());
            if call.starts_with(char::is_lowercase)
                && before.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '.')
            {
                continue;
            }
            let rest = text.get(at + call.len()..).unwrap_or_default();
            let rest = rest.trim_start();
            let rest = rest.strip_prefix('[').unwrap_or(rest).trim_start();
            let Some(quote) = rest
                .chars()
                .next()
                .filter(|c| matches!(c, '\'' | '"' | '`'))
            else {
                continue;
            };
            let literal = rest
                .get(1..)
                .and_then(|rest| rest.split(quote).next())
                .unwrap_or_default();
            if let Some(word) = literal.split_whitespace().next() {
                let line = text
                    .get(..at)
                    .map_or(1, |before| before.matches('\n').count() + 1);
                found.push((word.to_owned(), line));
            }
        }
    }
    found.sort_by_key(|(_, line)| *line);
    found
}

/// Returns each dependency of a `package.json` and its version, from its `dependencies`,
/// `devDependencies`, `optionalDependencies`, and `peerDependencies`.
fn dependencies(text: &str) -> Vec<(String, String)> {
    let mut found = Vec::new();
    for key in [
        "\"dependencies\"",
        "\"devDependencies\"",
        "\"optionalDependencies\"",
        "\"peerDependencies\"",
    ] {
        let Some(at) = text.find(key) else {
            continue;
        };
        let rest = text.get(at + key.len()..).unwrap_or_default();
        let Some(open) = rest.find('{') else {
            continue;
        };
        let body = rest.get(open + 1..).unwrap_or_default();
        let body = body
            .get(..body.find('}').unwrap_or(body.len()))
            .unwrap_or_default();
        let strings: Vec<&str> = body.split('"').skip(1).step_by(2).collect();
        for pair in strings.chunks(2) {
            if let [name, spec] = pair {
                found.push(((*name).to_owned(), (*spec).to_owned()));
            }
        }
    }
    found
}

/// Renders the review of a pack: its files, and the capabilities that the scan finds. With
/// `before`, the files and capabilities show what changed: `+` for new, `~` for a changed file,
/// and `-` for removed.
pub fn review(before: Option<&[File<'_>]>, after: &[File<'_>]) -> String {
    let mut out = String::new();
    let size: usize = after.iter().map(|file| file.bytes.len()).sum();
    let size = format::amount(
        Kind::Size,
        f64::from(u32::try_from(size).unwrap_or(u32::MAX)),
    );

    match before {
        None => {
            let _ = writeln!(out, "files  {} ({size})", after.len());
            for file in after {
                let _ = writeln!(out, "  {}", pack::printable(file.path));
            }
        }
        Some(before) => {
            let mut lines = Vec::new();
            let (mut added, mut changed, mut removed) = (0, 0, 0);
            for file in after {
                match before.iter().find(|old| old.path == file.path) {
                    None => {
                        added += 1;
                        lines.push(format!("  + {}", file.path));
                    }
                    Some(old) if old.bytes != file.bytes || old.executable != file.executable => {
                        changed += 1;
                        lines.push(format!("  ~ {}", file.path));
                    }
                    Some(_) => {}
                }
            }
            for old in before {
                if !after.iter().any(|file| file.path == old.path) {
                    removed += 1;
                    lines.push(format!("  - {}", old.path));
                }
            }
            let _ = writeln!(out, "files  +{added} ~{changed} -{removed} ({size})");
            for line in lines {
                let _ = writeln!(out, "{}", pack::printable(&line));
            }
        }
    }

    let found = scan(after);
    let old = before.map(scan);
    let mut rows: Vec<(char, &Finding)> = Vec::new();
    for finding in &found {
        let new = old.as_ref().is_some_and(|old| {
            !old.iter().any(|other| {
                other.capability == finding.capability && other.detail == finding.detail
            })
        });
        rows.push((if new { '+' } else { ' ' }, finding));
    }
    for finding in old.iter().flatten() {
        if !found
            .iter()
            .any(|other| other.capability == finding.capability && other.detail == finding.detail)
        {
            rows.push(('-', finding));
        }
    }

    let _ = writeln!(out, "capabilities");
    if rows.is_empty() {
        let _ = writeln!(out, "  none");
    }
    let width = rows
        .iter()
        .map(|(_, finding)| finding.detail.chars().count())
        .max()
        .unwrap_or(0)
        .min(40);
    for (mark, finding) in rows {
        let severity = match finding.severity {
            Severity::High => "high",
            Severity::Info => "info",
        };
        let place = match (mark, finding.line) {
            ('-', _) => String::new(),
            (_, 0) => finding.path.clone(),
            (_, line) => format!("{}:{line}", finding.path),
        };
        let row = format!(
            "{mark} {severity}  {:<12}  {:<width$}  {place}",
            finding.capability, finding.detail
        );
        let _ = writeln!(out, "{}", row.trim_end());
    }
    out
}
