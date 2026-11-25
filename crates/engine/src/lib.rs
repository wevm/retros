//! The pure Uphill engine.
//!
//! The engine parses `uphill.yaml` and the metric lines that scripts write, collapses each metric
//! to one value, scores values on Lighthouse's log-normal curve, finds the opportunities that
//! would raise each score, compares runs with each metric's baseline and with another revision's
//! runs, keeps runs as notes, renders reports and a dashboard of several repositories' noted
//! runs, composes a project's categories with its packs' categories, and scans packs for what
//! their code can do. It reads no files, environment, clock, or process state, so the same inputs
//! always produce the same scores, report bytes, and baselines.
//! The `uphill` command line supplies inputs and writes outputs.

pub mod baseline;
pub mod check;
pub mod compare;
pub mod config;
pub mod dashboard;
mod document;
mod edit;
pub mod error;
pub mod format;
pub mod history;
pub mod html;
mod json;
pub mod lines;
pub mod pack;
pub mod report;
pub mod scan;
pub mod score;
pub mod value;
