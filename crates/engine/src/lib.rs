//! The pure Retros engine.
//!
//! The engine parses `retros.yaml` and the metric lines that scripts write, collapses each metric
//! to one value, scores values on Lighthouse's log-normal curve, finds the opportunities that
//! would raise each score, compares runs with `retros.lock`, and renders reports. It reads no
//! files, environment, clock, or process state, so the same inputs always produce the same scores
//! and report bytes. The `retros` command line supplies inputs and writes outputs.

pub mod check;
pub mod config;
pub mod error;
pub mod format;
pub mod html;
pub mod lines;
pub mod lock;
pub mod report;
pub mod score;
pub mod value;
mod yaml;
