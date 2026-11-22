//! The pure Retros engine.
//!
//! The engine parses `retros.toml` and the metric lines that scripts write, collapses each metric
//! to one value, scores values on Lighthouse's log-normal curve, compares two revisions, and
//! renders reports. It reads no files, environment, clock, or process state, so the same inputs
//! always produce the same scores and report bytes. The `retros` command line supplies inputs and
//! writes outputs.

pub mod check;
pub mod config;
pub mod error;
pub mod format;
pub mod lines;
pub mod value;
