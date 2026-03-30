//! Benchmarks for the validator hotspot.
//!
//! Kept this intentionally small. I mainly wanted something that would tell me
//! "did validation just get slower?" without building a giant benchmark suite.

use concerto_core::ModelManager;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use serde_json::json;

const PERSON_MODEL_JSON: &str = r#"
{
  "$class": "concerto.metamodel@1.0.0.Model",
  "namespace": "org.example@1.0.0",
  "declarations": [
    {
      "$class": "concerto.metamodel@1.0.0.ConceptDeclaration",
      "name": "Person",
      "isAbstract": false,
      "properties": [
        {"$class": "concerto.metamodel@1.0.0.StringProperty",  "name": "email",     "isArray": false, "isOptional": false},
        {"$class": "concerto.metamodel@1.0.0.StringProperty",  "name": "firstName", "isArray": false, "isOptional": false},
        {"$class": "concerto.metamodel@1.0.0.StringProperty",  "name": "lastName",  "isArray": false, "isOptional": true},
        {
          "$class": "concerto.metamodel@1.0.0.IntegerProperty",
          "name": "age",
          "isArray": false,
          "isOptional": false,
          "validator": {
            "$class": "concerto.metamodel@1.0.0.IntegerDomainValidator",
            "lower": 0,
            "upper": 150
          }
        },
        {"$class": "concerto.metamodel@1.0.0.DateTimeProperty", "name": "dateOfBirth", "isArray": false, "isOptional": true}
      ]
    }
  ]
}
"#;

fn setup_model() -> ModelManager {
    let mut mm = ModelManager::new();
    if let Err(error) = mm.add_model_from_json(PERSON_MODEL_JSON) {
        panic!("benchmark fixture model should load: {error}");
    }
    mm
}

fn benchmark_entity_validation(criterion: &mut Criterion) {
    let mm = setup_model();

    let valid_instance = json!({
        "$class": "org.example@1.0.0.Person",
        "email": "alice@example.com",
        "firstName": "Alice",
        "age": 30
    });

    let invalid_instance = json!({
        "$class": "org.example@1.0.0.Person",
        "email": "alice@example.com",
        "firstName": "Alice",
        "age": 200
    });

    let missing_props_instance = json!({
        "$class": "org.example@1.0.0.Person"
    });

    criterion.bench_with_input(
        BenchmarkId::new("validate_valid_instance", "Person"),
        &valid_instance,
        |bench, instance| {
            bench.iter(|| {
                if let Err(error) = mm.validate_instance(instance, "org.example@1.0.0.Person") {
                    panic!("valid benchmark instance should not hard-error: {error}");
                }
            })
        },
    );

    criterion.bench_with_input(
        BenchmarkId::new("validate_invalid_instance", "Person"),
        &invalid_instance,
        |bench, instance| {
            bench.iter(|| {
                if let Err(error) = mm.validate_instance(instance, "org.example@1.0.0.Person") {
                    panic!("invalid benchmark instance should still return a result: {error}");
                }
            })
        },
    );

    criterion.bench_with_input(
        BenchmarkId::new("validate_missing_props_instance", "Person"),
        &missing_props_instance,
        |bench, instance| {
            bench.iter(|| {
                if let Err(error) = mm.validate_instance(instance, "org.example@1.0.0.Person") {
                    panic!(
                        "missing-props benchmark instance should still return a result: {error}"
                    );
                }
            })
        },
    );
}

fn benchmark_model_loading(criterion: &mut Criterion) {
    criterion.bench_function("load_person_model_from_json", |bench| {
        bench.iter(|| {
            let mut mm = ModelManager::new();
            if let Err(error) = mm.add_model_from_json(PERSON_MODEL_JSON) {
                panic!("benchmark model fixture should load: {error}");
            }
            mm
        })
    });
}

criterion_group!(
    benches,
    benchmark_entity_validation,
    benchmark_model_loading
);
criterion_main!(benches);
