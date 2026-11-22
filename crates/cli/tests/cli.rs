//! Runs the `retros` binary as a consumer does and checks its output and exit codes.

use std::process::{Command, Output};

fn retros(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_retros"))
        .args(args)
        .output()
        .expect("the retros binary runs")
}

#[test]
fn version() {
    let output = retros(&["--version"]);

    assert!(output.status.success());
    insta::assert_snapshot!(String::from_utf8_lossy(&output.stdout), @"retros 0.0.0");
}

#[test]
fn help() {
    let output = retros(&["--help"]);

    assert!(output.status.success());
    insta::assert_snapshot!(String::from_utf8_lossy(&output.stdout), @"Usage: retros [--help | --version]");
}

#[test]
fn unknown_argument() {
    let output = retros(&["--nope"]);

    assert_eq!(output.status.code(), Some(2));
    insta::assert_snapshot!(String::from_utf8_lossy(&output.stderr), @"
    error: unknown argument '--nope'

    Usage: retros [--help | --version]
    ");
}

#[test]
fn no_arguments() {
    let output = retros(&[]);

    assert_eq!(output.status.code(), Some(2));
    insta::assert_snapshot!(String::from_utf8_lossy(&output.stderr), @"
    error: expected an argument

    Usage: retros [--help | --version]
    ");
}
