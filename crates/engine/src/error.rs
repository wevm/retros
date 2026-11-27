//! Errors with a stable code, a message, and the position in the source text they describe.

use std::fmt::Write;

/// A position in source text. Lines and columns start at 1, and columns count characters.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Location {
    /// Column of the first character the error describes.
    pub column: u32,
    /// Line that holds the error.
    pub line: u32,
}

impl Location {
    /// Returns the location of a byte offset in `source`. An offset past the end returns the end.
    pub fn of(source: &str, offset: usize) -> Self {
        let mut offset = offset.min(source.len());
        while !source.is_char_boundary(offset) {
            offset -= 1;
        }

        let before = source.get(..offset).unwrap_or_default();
        let line_start = before.rfind('\n').map_or(0, |index| index + 1);
        let line = before.matches('\n').count() + 1;
        let column = before.get(line_start..).unwrap_or_default().chars().count() + 1;

        Self {
            column: u32::try_from(column).unwrap_or(u32::MAX),
            line: u32::try_from(line).unwrap_or(u32::MAX),
        }
    }
}

/// The kind of an error. Codes are public contract and keep their meaning across releases.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Code {
    /// A metric line written twice in one run, or an item written twice for one metric.
    Duplicate,
    /// A metric in two categories that share one `run` command.
    DuplicateMetric,
    /// A name with characters other than ASCII letters, digits, `-`, `_`, and `.`.
    InvalidName,
    /// A number that does not parse.
    InvalidNumber,
    /// A configuration value of the wrong type.
    InvalidType,
    /// A configuration value of the right type that is not allowed.
    InvalidValue,
    /// A metric line that is not `[subject:] metric [item] value`.
    LineSyntax,
    /// A configured metric that the script did not write.
    MetricMissing,
    /// A metric written both with and without items.
    MixedItems,
    /// A category written both with and without subjects.
    MixedSubjects,
    /// A metric that sets a target on some lines but not others, with no default target.
    MixedTargets,
    /// A metric written with more than one kind of unit.
    MixedUnits,
    /// A remote pack that no pattern of `allowed-packs` matches.
    NotAllowed,
    /// A category without metrics, or a configuration without categories.
    NothingConfigured,
    /// A category without a `run` command.
    RunMissing,
    /// TOML syntax in `uphill.toml` that does not parse.
    Toml,
    /// A metric the script wrote that `uphill.yaml` does not configure.
    UnconfiguredMetric,
    /// A target that is not greater than zero. The scoring curve needs a positive target.
    TargetRange,
    /// A target that is a check, or that does not match the unit the script writes.
    TargetUnit,
    /// A key that `uphill.yaml` does not define.
    UnexpectedKey,
    /// A pattern in a metric's `targets` that matches no item the script wrote.
    UnmatchedPattern,
    /// A remote pack without a full commit.
    Unpinned,
    /// A unit that is not a size, time, or check unit.
    UnknownUnit,
    /// A fetched pack with a file that Uphill does not write, such as a symbolic link, an unsafe path,
    /// or a file over the size limits.
    UnsafePack,
    /// YAML in `uphill.yaml` that does not parse, or that Uphill does not accept, such as a
    /// duplicate key or a tag.
    Yaml,
}

impl Code {
    /// Returns the code as it appears in reports, such as `unknown-unit`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Duplicate => "duplicate",
            Self::DuplicateMetric => "duplicate-metric",
            Self::InvalidName => "invalid-name",
            Self::InvalidNumber => "invalid-number",
            Self::InvalidType => "invalid-type",
            Self::InvalidValue => "invalid-value",
            Self::LineSyntax => "line-syntax",
            Self::MetricMissing => "metric-missing",
            Self::MixedItems => "mixed-items",
            Self::MixedSubjects => "mixed-subjects",
            Self::MixedTargets => "mixed-targets",
            Self::MixedUnits => "mixed-units",
            Self::NotAllowed => "not-allowed",
            Self::NothingConfigured => "nothing-configured",
            Self::RunMissing => "run-missing",
            Self::Toml => "toml",
            Self::TargetRange => "target-range",
            Self::TargetUnit => "target-unit",
            Self::UnconfiguredMetric => "unconfigured-metric",
            Self::UnexpectedKey => "unexpected-key",
            Self::UnmatchedPattern => "unmatched-pattern",
            Self::Unpinned => "unpinned",
            Self::UnknownUnit => "unknown-unit",
            Self::UnsafePack => "unsafe-pack",
            Self::Yaml => "yaml",
        }
    }
}

/// An error in the configuration or in the metric lines that a script writes.
#[derive(Clone, Debug, PartialEq)]
pub struct Error {
    /// The kind of error.
    pub code: Code,
    /// The position the error describes, when it describes one.
    pub location: Option<Location>,
    /// A message that names the problem and, where useful, the accepted values.
    pub message: String,
}

impl Error {
    /// Creates an error at a byte offset in `source`.
    #[cold]
    pub(crate) fn at(code: Code, source: &str, offset: usize, message: String) -> Self {
        Self {
            code,
            location: Some(Location::of(source, offset)),
            message,
        }
    }

    /// Returns this error with the message of the error that caused it appended after a colon.
    #[cold]
    #[must_use]
    pub fn because(mut self, cause: &Error) -> Self {
        self.message = format!("{}: {}", self.message, cause.message);
        self
    }

    /// Renders the error as `name:line:column: message`, followed by the source line and a caret
    /// under the column. An error without a location renders as `name: message`.
    pub fn render(&self, name: &str, source: &str) -> String {
        let Some(location) = self.location else {
            return format!("{name}: {}", self.message);
        };

        let mut out = format!(
            "{name}:{}:{}: {}",
            location.line, location.column, self.message
        );
        let index = usize::try_from(location.line.saturating_sub(1)).unwrap_or(usize::MAX);

        if let Some(text) = source.lines().nth(index) {
            let number = location.line.to_string();
            let gutter = " ".repeat(number.len());
            let caret = " ".repeat(usize::try_from(location.column.saturating_sub(1)).unwrap_or(0));
            let _ = write!(out, "\n  {number} | {text}\n  {gutter} | {caret}^");
        }

        out
    }
}
