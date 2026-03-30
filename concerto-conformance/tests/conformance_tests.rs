//! Hand-written conformance checks.
//!
//! These came before the fixture runner got decent. I kept them because they
//! still read nicely when I want a quick sanity check without opening twelve
//! JSON files in a row.

use concerto_core::{ErrorKind, ModelManager, ValidationError, ValidationResult};
use serde_json::{json, Value};

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
        {"$class":"concerto.metamodel@1.0.0.StringProperty","name":"email","isArray":false,"isOptional":false},
        {"$class":"concerto.metamodel@1.0.0.StringProperty","name":"firstName","isArray":false,"isOptional":false},
        {"$class":"concerto.metamodel@1.0.0.StringProperty","name":"lastName","isArray":false,"isOptional":true},
        {
          "$class":"concerto.metamodel@1.0.0.IntegerProperty","name":"age","isArray":false,"isOptional":false,
          "validator":{"$class":"concerto.metamodel@1.0.0.IntegerDomainValidator","lower":0,"upper":150}
        },
        {"$class":"concerto.metamodel@1.0.0.DateTimeProperty","name":"dateOfBirth","isArray":false,"isOptional":true}
      ]
    }
  ]
}
"#;

const EMAIL_CONSTRAINED_MODEL_JSON: &str = r#"
{
  "$class": "concerto.metamodel@1.0.0.Model",
  "namespace": "org.example@1.0.0",
  "declarations": [
    {
      "$class": "concerto.metamodel@1.0.0.ConceptDeclaration",
      "name": "Contact",
      "isAbstract": false,
      "properties": [
        {
          "$class": "concerto.metamodel@1.0.0.StringProperty",
          "name": "email", "isArray": false, "isOptional": false,
          "validator": {
            "$class": "concerto.metamodel@1.0.0.StringRegexValidator",
            "pattern": "^[^@]+@[^@]+\\.[^@]+$",
            "flags": ""
          }
        }
      ]
    }
  ]
}
"#;

const PERSON_WITH_GENDER_MODEL_JSON: &str = r#"
{
  "$class": "concerto.metamodel@1.0.0.Model",
  "namespace": "org.example@1.0.0",
  "declarations": [
    {
      "$class": "concerto.metamodel@1.0.0.EnumDeclaration",
      "name": "Gender",
      "properties": [
        {"$class":"concerto.metamodel@1.0.0.EnumProperty","name":"MALE"},
        {"$class":"concerto.metamodel@1.0.0.EnumProperty","name":"FEMALE"},
        {"$class":"concerto.metamodel@1.0.0.EnumProperty","name":"OTHER"}
      ]
    },
    {
      "$class": "concerto.metamodel@1.0.0.ConceptDeclaration",
      "name": "PersonWithGender",
      "isAbstract": false,
      "properties": [
        {"$class":"concerto.metamodel@1.0.0.StringProperty","name":"email","isArray":false,"isOptional":false},
        {"$class":"concerto.metamodel@1.0.0.StringProperty","name":"firstName","isArray":false,"isOptional":false},
        {
          "$class":"concerto.metamodel@1.0.0.IntegerProperty","name":"age","isArray":false,"isOptional":false,
          "validator":{"$class":"concerto.metamodel@1.0.0.IntegerDomainValidator","lower":0,"upper":150}
        },
        {
          "$class":"concerto.metamodel@1.0.0.ObjectProperty","name":"gender","isArray":false,"isOptional":false,
          "type":{"$class":"concerto.metamodel@1.0.0.TypeIdentifier","name":"Gender"}
        }
      ]
    }
  ]
}
"#;

const EMPLOYEE_EXTENDS_PERSON_JSON: &str = r#"
{
  "$class": "concerto.metamodel@1.0.0.Model",
  "namespace": "org.example@1.0.0",
  "declarations": [
    {
      "$class": "concerto.metamodel@1.0.0.ConceptDeclaration",
      "name": "Person",
      "isAbstract": false,
      "properties": [
        {"$class":"concerto.metamodel@1.0.0.StringProperty","name":"email","isArray":false,"isOptional":false},
        {"$class":"concerto.metamodel@1.0.0.StringProperty","name":"firstName","isArray":false,"isOptional":false},
        {
          "$class":"concerto.metamodel@1.0.0.IntegerProperty","name":"age","isArray":false,"isOptional":false,
          "validator":{"$class":"concerto.metamodel@1.0.0.IntegerDomainValidator","lower":0,"upper":150}
        }
      ]
    },
    {
      "$class": "concerto.metamodel@1.0.0.ConceptDeclaration",
      "name": "Employee",
      "isAbstract": false,
      "superType":{"$class":"concerto.metamodel@1.0.0.TypeIdentifier","name":"Person"},
      "properties": [
        {"$class":"concerto.metamodel@1.0.0.StringProperty","name":"employeeId","isArray":false,"isOptional":false}
      ]
    }
  ]
}
"#;

fn model_manager_from_json(model_json: &str) -> ModelManager {
    let mut mm = ModelManager::new();
    if let Err(error) = mm.add_model_from_json(model_json) {
        panic!("fixture model should load: {error}");
    }
    mm
}

fn validate_fixture(model_json: &str, instance: &Value, type_name: &str) -> ValidationResult {
    let mm = model_manager_from_json(model_json);
    match mm.validate_instance(instance, type_name) {
        Ok(result) => result,
        Err(error) => panic!("validation should return structured result: {error}"),
    }
}

fn error_at_path<'a>(result: &'a ValidationResult, path: &str) -> &'a ValidationError {
    match result.errors.iter().find(|error| error.path == path) {
        Some(error) => error,
        None => panic!(
            "expected validation error at {path}, got {:?}",
            result.errors
        ),
    }
}

#[test]
fn conformance_01_valid_instance_passes() {
    // first obvious check: good payload should just pass
    let instance = json!({
        "$class": "org.example@1.0.0.Person",
        "email": "alice@example.com",
        "firstName": "Alice",
        "age": 30
    });

    let result = validate_fixture(PERSON_MODEL_JSON, &instance, "org.example@1.0.0.Person");
    assert!(
        result.valid,
        "valid instance should pass: {:?}",
        result.errors
    );
    assert!(result.errors.is_empty());
}

#[test]
fn conformance_02_missing_required_property() {
    // this is where "collect all errors" starts mattering
    let instance = json!({
        "$class": "org.example@1.0.0.Person",
        "email": "alice@example.com"
    });

    let result = validate_fixture(PERSON_MODEL_JSON, &instance, "org.example@1.0.0.Person");
    assert!(!result.valid);
    assert!(matches!(
        error_at_path(&result, "$.firstName").error_type,
        ErrorKind::MissingRequiredProperty
    ));
    assert!(matches!(
        error_at_path(&result, "$.age").error_type,
        ErrorKind::MissingRequiredProperty
    ));
}

#[test]
fn conformance_03_string_regex_constraint() {
    // basic validator path, but still useful
    let instance = json!({
        "$class": "org.example@1.0.0.Contact",
        "email": "not-an-email"
    });

    let result = validate_fixture(
        EMAIL_CONSTRAINED_MODEL_JSON,
        &instance,
        "org.example@1.0.0.Contact",
    );
    assert!(matches!(
        &error_at_path(&result, "$.email").error_type,
        ErrorKind::PatternMismatch { .. }
    ));
}

#[test]
fn conformance_04_enum_value_check() {
    // enums were easy to miss early on bc they come through object refs
    let instance = json!({
        "$class": "org.example@1.0.0.PersonWithGender",
        "email": "alice@example.com",
        "firstName": "Alice",
        "age": 30,
        "gender": "INVALID"
    });

    let result = validate_fixture(
        PERSON_WITH_GENDER_MODEL_JSON,
        &instance,
        "org.example@1.0.0.PersonWithGender",
    );
    assert!(matches!(
        &error_at_path(&result, "$.gender").error_type,
        ErrorKind::InvalidEnumValue { .. }
    ));
}

#[test]
fn conformance_05_inheritance_still_requires_parent_fields() {
    // old one-level-up inheritance draft broke this
    let instance = json!({
        "$class": "org.example@1.0.0.Employee",
        "firstName": "Bob",
        "age": 40,
        "employeeId": "E-1"
    });

    let result = validate_fixture(
        EMPLOYEE_EXTENDS_PERSON_JSON,
        &instance,
        "org.example@1.0.0.Employee",
    );
    assert!(matches!(
        error_at_path(&result, "$.email").error_type,
        ErrorKind::MissingRequiredProperty
    ));
}

#[test]
fn conformance_06_child_instance_is_valid_for_parent_request() {
    // important polymorphic case for the actual runtime story
    let instance = json!({
        "$class": "org.example@1.0.0.Employee",
        "email": "bob@company.com",
        "firstName": "Bob",
        "age": 40,
        "employeeId": "E-1"
    });

    let result = validate_fixture(
        EMPLOYEE_EXTENDS_PERSON_JSON,
        &instance,
        "org.example@1.0.0.Person",
    );
    assert!(
        result.valid,
        "child instance should validate for parent request: {:?}",
        result.errors
    );
}
