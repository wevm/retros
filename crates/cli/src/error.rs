//! The error that stops a command, with the message the terminal prints.

use std::fmt;

/// A failure that stops a command. `main` writes its message to standard error and exits with
/// status 2. The message already holds its severity, such as `error: unknown revision 'main'`.
#[derive(Debug)]
pub(crate) struct Error {
    message: String,
}

impl Error {
    /// Returns the message the terminal prints.
    pub(crate) fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for Error {}

impl From<String> for Error {
    #[cold]
    fn from(message: String) -> Self {
        Self { message }
    }
}

impl From<&str> for Error {
    #[cold]
    fn from(message: &str) -> Self {
        Self {
            message: message.to_owned(),
        }
    }
}
