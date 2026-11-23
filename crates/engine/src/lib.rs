//! The pure Retros engine.
//!
//! The engine parses `retros.yaml` and the metric lines that scripts write, collapses each metric
//! to one value, scores values on Lighthouse's log-normal curve, finds the opportunities that
//! would raise each score, compares runs with each metric's baseline and with another revision's
//! runs, keeps runs as notes, and renders reports. It reads no files, environment, clock, or
//! process state, so the same inputs always produce the same scores, report bytes, and baselines.
//! The `retros` command line supplies inputs and writes outputs.

pub mod baseline;
pub mod check;
pub mod compare;
pub mod config;
mod document;
mod edit;
pub mod error;
pub mod format;
pub mod history;
pub mod html;
pub mod lines;
pub mod report;
pub mod score;
pub mod value;
