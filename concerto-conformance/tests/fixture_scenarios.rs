//! Fixture-driven conformance scenarios.
//!
//! This is probably the most scalable bit of the conformance story in the repo.
//! If someone wants to add another behavior case, they should mostly be able to
//! do it by dropping JSON into `fixtures/` and not touching Rust at all.

use std::fs;
use std::path::Path;

use concerto_conformance::{load_fixture_text, load_scenario_fixture};
use concerto_core::ModelManager;

#[test]
fn fixture_scenarios_match_expected_results() {
    let scenario_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/scenarios");
    let mut scenario_paths = fs::read_dir(&scenario_dir)
        .unwrap_or_else(|error| panic!("couldn't read {}: {error}", scenario_dir.display()))
        .map(|entry| entry.expect("couldn't read scenario dir entry").path())
        .collect::<Vec<_>>();
    scenario_paths.sort();

    for scenario_path in scenario_paths {
        let filename = scenario_path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("scenario filename should be utf-8");
        let scenario = load_scenario_fixture(filename);

        let mut mm = ModelManager::new();
        for model_file in &scenario.model_files {
            mm.add_model_from_json(&load_fixture_text(model_file))
                .unwrap_or_else(|error| {
                    panic!(
                        "scenario '{}' failed to load {}: {error}",
                        scenario.name, model_file
                    )
                });
        }

        let instance = serde_json::from_str(&load_fixture_text(&scenario.instance_file))
            .unwrap_or_else(|error| {
                panic!(
                    "scenario '{}' has bad instance json in {}: {error}",
                    scenario.name, scenario.instance_file
                )
            });

        let result = mm
            .validate_instance(&instance, &scenario.type_name)
            .unwrap_or_else(|error| panic!("scenario '{}' errored: {error}", scenario.name));

        assert_eq!(
            result.valid, scenario.expect.valid,
            "scenario '{}' validity mismatch: {:?}",
            scenario.name, result.errors
        );
        assert_eq!(
            result.errors.len(),
            scenario.expect.error_count,
            "scenario '{}' error count mismatch: {:?}",
            scenario.name,
            result.errors
        );

        for expected_path in &scenario.expect.error_paths {
            assert!(
                result
                    .errors
                    .iter()
                    .any(|error| &error.path == expected_path),
                "scenario '{}' is missing expected error path {} in {:?}",
                scenario.name,
                expected_path,
                result.errors
            );
        }
    }
}
