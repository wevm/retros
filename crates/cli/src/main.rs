//! The `retros` command line.
//!
//! The command line is a thin adapter around the engine. It runs measurement scripts, reads the
//! files they write, and writes reports. Commands land in the phases of `.agents/plan.md`.

use std::io::Write;
use std::process::ExitCode;

const USAGE: &str = "Usage: retros [--help | --version]\n";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    match args.as_slice() {
        [flag] if flag == "--help" || flag == "-h" => {
            let _ = std::io::stdout().write_all(USAGE.as_bytes());
            ExitCode::SUCCESS
        }
        [flag] if flag == "--version" || flag == "-V" => {
            let _ = writeln!(std::io::stdout(), "retros {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        [] => fail("expected an argument"),
        [arg, ..] => fail(&format!("unknown argument '{arg}'")),
    }
}

/// Writes an error and the usage to standard error, and returns the usage exit code.
#[cold]
fn fail(message: &str) -> ExitCode {
    let _ = write!(std::io::stderr(), "error: {message}\n\n{USAGE}");
    ExitCode::from(2)
}
