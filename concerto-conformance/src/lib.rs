//! Helpers for the fixture-backed conformance checks.
//!
//! Once I had both "regression for a bug I just hit" tests and "does this still
//! look like Concerto behavior?" tests, mixing them in one place got messy.
//! This crate is the second bucket.

use std::fs;
use std::path::{Path, PathBuf};

/// Expected outcome for one fixture scenario.
///
/// `error_count` is here on purpose so extra bogus errors do not silently pass.
#[derive(Debug, serde::Deserialize)]
pub struct ScenarioExpectation {
    /// Whether validation should succeed.
    pub valid: bool,
    /// Exact number of validation errors expected.
    pub error_count: usize,
    /// Error paths that must exist in the result.
    #[serde(default)]
    pub error_paths: Vec<String>,
}

/// One scenario loaded from JSON.
///
/// The point is to keep most conformance growth in fixture files, not Rust code.
#[derive(Debug, serde::Deserialize)]
pub struct ScenarioFixture {
    /// Scenario name used in panic messages.
    pub name: String,
    /// Model fixture files to load.
    pub model_files: Vec<String>,
    /// Instance fixture file to validate.
    pub instance_file: String,
    /// Requested target type.
    pub type_name: String,
    /// Expected validation result.
    pub expect: ScenarioExpectation,
}

/// Loads one scenario fixture from `fixtures/scenarios`.
pub fn load_scenario_fixture(filename: &str) -> ScenarioFixture {
    let path = fixture_root().join("scenarios").join(filename);
    let json = fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!("couldn't read scenario fixture {}: {error}", path.display())
    });
    serde_json::from_str(&json)
        .unwrap_or_else(|error| panic!("scenario fixture {} is bad json: {error}", path.display()))
}

/// Loads one raw fixture file from `fixtures`.
pub fn load_fixture_text(relative_path: &str) -> String {
    let path = fixture_root().join(relative_path);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("couldn't read fixture {}: {error}", path.display()))
}

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}
