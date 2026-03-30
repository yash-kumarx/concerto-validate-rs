//! Tests for `instance_validator`.
//!
//! These grew in the same order the validator grew. First the obvious happy
//! path, then required fields, then type checks, then the annoying stuff once
//! inheritance and imports got involved.
//!
//! The bottom of the file is basically a bug diary. If a comment sounds too
//! specific, that's probably because that exact thing broke once and I was not
//! in the mood to rediscover it later.

use concerto_core::{ConcertoError, ErrorKind, ModelManager, ValidationError, ValidationResult};
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
        {"$class": "concerto.metamodel@1.0.0.StringProperty", "name": "email", "isArray": false, "isOptional": false},
        {"$class": "concerto.metamodel@1.0.0.StringProperty", "name": "firstName", "isArray": false, "isOptional": false},
        {"$class": "concerto.metamodel@1.0.0.StringProperty", "name": "lastName", "isArray": false, "isOptional": true},
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

const CONTACT_MODEL_JSON: &str = r#"
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
          "name": "email",
          "isArray": false,
          "isOptional": false,
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
        {"$class": "concerto.metamodel@1.0.0.EnumProperty", "name": "MALE"},
        {"$class": "concerto.metamodel@1.0.0.EnumProperty", "name": "FEMALE"},
        {"$class": "concerto.metamodel@1.0.0.EnumProperty", "name": "OTHER"}
      ]
    },
    {
      "$class": "concerto.metamodel@1.0.0.ConceptDeclaration",
      "name": "PersonWithGender",
      "isAbstract": false,
      "properties": [
        {"$class": "concerto.metamodel@1.0.0.StringProperty", "name": "email", "isArray": false, "isOptional": false},
        {"$class": "concerto.metamodel@1.0.0.StringProperty", "name": "firstName", "isArray": false, "isOptional": false},
        {
          "$class": "concerto.metamodel@1.0.0.IntegerProperty",
          "name": "age",
          "isArray": false,
          "isOptional": false,
          "validator": {"$class": "concerto.metamodel@1.0.0.IntegerDomainValidator", "lower": 0, "upper": 150}
        },
        {
          "$class": "concerto.metamodel@1.0.0.ObjectProperty",
          "name": "gender",
          "isArray": false,
          "isOptional": false,
          "type": {"$class": "concerto.metamodel@1.0.0.TypeIdentifier", "name": "Gender"}
        }
      ]
    }
  ]
}
"#;

const EMPLOYEE_MODEL_JSON: &str = r#"
{
  "$class": "concerto.metamodel@1.0.0.Model",
  "namespace": "org.example@1.0.0",
  "declarations": [
    {
      "$class": "concerto.metamodel@1.0.0.ConceptDeclaration",
      "name": "Person",
      "isAbstract": false,
      "properties": [
        {"$class": "concerto.metamodel@1.0.0.StringProperty", "name": "email", "isArray": false, "isOptional": false},
        {"$class": "concerto.metamodel@1.0.0.StringProperty", "name": "firstName", "isArray": false, "isOptional": false},
        {
          "$class": "concerto.metamodel@1.0.0.IntegerProperty",
          "name": "age",
          "isArray": false,
          "isOptional": false,
          "validator": {"$class": "concerto.metamodel@1.0.0.IntegerDomainValidator", "lower": 0, "upper": 150}
        }
      ]
    },
    {
      "$class": "concerto.metamodel@1.0.0.ConceptDeclaration",
      "name": "Employee",
      "isAbstract": false,
      "superType": {
        "$class": "concerto.metamodel@1.0.0.TypeIdentifier",
        "name": "Person"
      },
      "properties": [
        {"$class": "concerto.metamodel@1.0.0.StringProperty", "name": "employeeId", "isArray": false, "isOptional": false}
      ]
    }
  ]
}
"#;

const OPTIONAL_CONSTRAINT_MODEL_JSON: &str = r#"
{
  "$class": "concerto.metamodel@1.0.0.Model",
  "namespace": "org.example@1.0.0",
  "declarations": [
    {
      "$class": "concerto.metamodel@1.0.0.ConceptDeclaration",
      "name": "Profile",
      "isAbstract": false,
      "properties": [
        {"$class": "concerto.metamodel@1.0.0.StringProperty", "name": "username", "isArray": false, "isOptional": false},
        {
          "$class": "concerto.metamodel@1.0.0.IntegerProperty",
          "name": "age",
          "isArray": false,
          "isOptional": true,
          "validator": {
            "$class": "concerto.metamodel@1.0.0.IntegerDomainValidator",
            "lower": 0,
            "upper": 150
          }
        }
      ]
    }
  ]
}
"#;

const ARRAY_MODEL_JSON: &str = r#"
{
  "$class": "concerto.metamodel@1.0.0.Model",
  "namespace": "org.arrays@1.0.0",
  "declarations": [
    {
      "$class": "concerto.metamodel@1.0.0.ConceptDeclaration",
      "name": "TaggedPerson",
      "isAbstract": false,
      "properties": [
        {"$class": "concerto.metamodel@1.0.0.StringProperty", "name": "name", "isArray": false, "isOptional": false},
        {
          "$class": "concerto.metamodel@1.0.0.IntegerProperty",
          "name": "scores",
          "isArray": true,
          "isOptional": false,
          "validator": {"$class": "concerto.metamodel@1.0.0.IntegerDomainValidator", "lower": 0, "upper": 100}
        }
      ]
    }
  ]
}
"#;

const DOUBLE_MODEL_JSON: &str = r#"
{
  "$class": "concerto.metamodel@1.0.0.Model",
  "namespace": "org.double@1.0.0",
  "declarations": [
    {
      "$class": "concerto.metamodel@1.0.0.ConceptDeclaration",
      "name": "Measurement",
      "isAbstract": false,
      "properties": [
        {
          "$class": "concerto.metamodel@1.0.0.DoubleProperty",
          "name": "value",
          "isArray": false,
          "isOptional": false
        }
      ]
    }
  ]
}
"#;

const COMMON_ADDRESS_MODEL_JSON: &str = r#"
{
  "$class": "concerto.metamodel@1.0.0.Model",
  "namespace": "org.common@1.0.0",
  "declarations": [
    {
      "$class": "concerto.metamodel@1.0.0.ConceptDeclaration",
      "name": "PostalAddress",
      "isAbstract": false,
      "properties": [
        {"$class": "concerto.metamodel@1.0.0.StringProperty", "name": "street", "isArray": false, "isOptional": false}
      ]
    }
  ]
}
"#;

const CUSTOMER_MODEL_JSON: &str = r#"
{
  "$class": "concerto.metamodel@1.0.0.Model",
  "namespace": "org.customer@1.0.0",
  "imports": [
    {
      "$class": "concerto.metamodel@1.0.0.ImportType",
      "namespace": "org.common@1.0.0",
      "name": "PostalAddress"
    }
  ],
  "declarations": [
    {
      "$class": "concerto.metamodel@1.0.0.ConceptDeclaration",
      "name": "Customer",
      "isAbstract": false,
      "properties": [
        {"$class": "concerto.metamodel@1.0.0.StringProperty", "name": "name", "isArray": false, "isOptional": false},
        {
          "$class": "concerto.metamodel@1.0.0.ObjectProperty",
          "name": "address",
          "isArray": false,
          "isOptional": false,
          "type": {"$class": "concerto.metamodel@1.0.0.TypeIdentifier", "name": "PostalAddress"}
        }
      ]
    }
  ]
}
"#;

const ABSTRACT_MODEL_JSON: &str = r#"
{
  "$class": "concerto.metamodel@1.0.0.Model",
  "namespace": "org.abstracts@1.0.0",
  "declarations": [
    {
      "$class": "concerto.metamodel@1.0.0.ConceptDeclaration",
      "name": "BaseRecord",
      "isAbstract": true,
      "properties": [
        {"$class": "concerto.metamodel@1.0.0.StringProperty", "name": "id", "isArray": false, "isOptional": false}
      ]
    }
  ]
}
"#;

const UNRELATED_TYPE_MODEL_JSON: &str = r#"
{
  "$class": "concerto.metamodel@1.0.0.Model",
  "namespace": "org.other@1.0.0",
  "declarations": [
    {
      "$class": "concerto.metamodel@1.0.0.ConceptDeclaration",
      "name": "Invoice",
      "isAbstract": false,
      "properties": [
        {"$class": "concerto.metamodel@1.0.0.StringProperty", "name": "invoiceNumber", "isArray": false, "isOptional": false}
      ]
    }
  ]
}
"#;

const RELATIONSHIP_MODEL_JSON: &str = r#"
{
  "$class": "concerto.metamodel@1.0.0.Model",
  "namespace": "org.fleet@1.0.0",
  "declarations": [
    {
      "$class": "concerto.metamodel@1.0.0.AssetDeclaration",
      "name": "Vehicle",
      "isAbstract": false,
      "properties": [
        {"$class": "concerto.metamodel@1.0.0.StringProperty", "name": "vin", "isArray": false, "isOptional": false}
      ]
    },
    {
      "$class": "concerto.metamodel@1.0.0.AssetDeclaration",
      "name": "Driver",
      "isAbstract": false,
      "properties": [
        {"$class": "concerto.metamodel@1.0.0.StringProperty", "name": "licenseId", "isArray": false, "isOptional": false}
      ]
    },
    {
      "$class": "concerto.metamodel@1.0.0.ConceptDeclaration",
      "name": "Assignment",
      "isAbstract": false,
      "properties": [
        {
          "$class": "concerto.metamodel@1.0.0.RelationshipProperty",
          "name": "vehicle",
          "isArray": false,
          "isOptional": false,
          "type": {"$class": "concerto.metamodel@1.0.0.TypeIdentifier", "name": "Vehicle"}
        }
      ]
    }
  ]
}
"#;

const SCALAR_MODEL_JSON: &str = r#"
{
  "$class": "concerto.metamodel@1.0.0.Model",
  "namespace": "org.scalar@1.0.0",
  "declarations": [
    {
      "$class": "concerto.metamodel@1.0.0.StringScalar",
      "name": "PostalCode",
      "validator": {
        "$class": "concerto.metamodel@1.0.0.StringRegexValidator",
        "pattern": "^[0-9]{5}$",
        "flags": ""
      },
      "lengthValidator": {
        "$class": "concerto.metamodel@1.0.0.StringLengthValidator",
        "minLength": 5,
        "maxLength": 5
      }
    },
    {
      "$class": "concerto.metamodel@1.0.0.ConceptDeclaration",
      "name": "Parcel",
      "isAbstract": false,
      "properties": [
        {
          "$class": "concerto.metamodel@1.0.0.ObjectProperty",
          "name": "postalCode",
          "isArray": false,
          "isOptional": false,
          "type": {"$class": "concerto.metamodel@1.0.0.TypeIdentifier", "name": "PostalCode"}
        }
      ]
    }
  ]
}
"#;

const MAP_MODEL_JSON: &str = r#"
{
  "$class": "concerto.metamodel@1.0.0.Model",
  "namespace": "org.map@1.0.0",
  "declarations": [
    {
      "$class": "concerto.metamodel@1.0.0.MapDeclaration",
      "name": "Checklist",
      "key": {"$class": "concerto.metamodel@1.0.0.DateTimeMapKeyType"},
      "value": {"$class": "concerto.metamodel@1.0.0.BooleanMapValueType"}
    },
    {
      "$class": "concerto.metamodel@1.0.0.ConceptDeclaration",
      "name": "Inspection",
      "isAbstract": false,
      "properties": [
        {
          "$class": "concerto.metamodel@1.0.0.ObjectProperty",
          "name": "checks",
          "isArray": false,
          "isOptional": false,
          "type": {"$class": "concerto.metamodel@1.0.0.TypeIdentifier", "name": "Checklist"}
        }
      ]
    }
  ]
}
"#;

fn model_manager_from_json(model_json: &str) -> ModelManager {
    let mut model_manager = ModelManager::new();
    if let Err(error) = model_manager.add_model_from_json(model_json) {
        panic!("fixture model should load: {error}");
    }
    model_manager
}

fn model_manager_from_many(model_jsons: &[&str]) -> ModelManager {
    let mut model_manager = ModelManager::new();
    for model_json in model_jsons {
        if let Err(error) = model_manager.add_model_from_json(model_json) {
            panic!("fixture model should load: {error}");
        }
    }
    model_manager
}

fn validate_fixture(model_json: &str, instance: &Value, type_name: &str) -> ValidationResult {
    let model_manager = model_manager_from_json(model_json);
    validate_with_manager(&model_manager, instance, type_name)
}

fn validate_with_manager(
    model_manager: &ModelManager,
    instance: &Value,
    type_name: &str,
) -> ValidationResult {
    match model_manager.validate_instance(instance, type_name) {
        Ok(validation_result) => validation_result,
        Err(error) => panic!("validation should return structured result: {error}"),
    }
}

fn error_at_path<'a>(validation_result: &'a ValidationResult, path: &str) -> &'a ValidationError {
    match validation_result
        .errors
        .iter()
        .find(|validation_error| validation_error.path == path)
    {
        Some(validation_error) => validation_error,
        None => panic!(
            "expected error at path {path}, got {:?}",
            validation_result.errors
        ),
    }
}

#[cfg(test)]
mod required_properties {
    use super::*;

    // first thing i wrote. if this fails the rest of the file is just noise
    #[test]
    fn valid_person_passes() {
        let instance = json!({
            "$class": "org.example@1.0.0.Person",
            "email": "alice@example.com",
            "firstName": "Alice",
            "age": 30
        });

        let result = validate_fixture(PERSON_MODEL_JSON, &instance, "org.example@1.0.0.Person");
        assert!(result.valid, "expected valid, got {:?}", result.errors);
        assert!(result.errors.is_empty());
    }

    // obvious bad case -- and important bc concerto-core JS collects all of
    // them, not just the first missing field
    #[test]
    fn missing_required_properties_are_reported_at_exact_paths() {
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

    // optional means optional. sounds silly, still worth pinning down
    #[test]
    fn optional_property_can_be_missing() {
        let instance = json!({
            "$class": "org.example@1.0.0.Person",
            "email": "alice@example.com",
            "firstName": "Alice",
            "age": 25
        });

        let result = validate_fixture(PERSON_MODEL_JSON, &instance, "org.example@1.0.0.Person");
        assert!(result.valid);
    }

    // first draft put this on "$" which was useless in the wasm demo
    #[test]
    fn missing_class_field_uses_dollar_class_path() {
        let instance = json!({
            "email": "alice@example.com",
            "firstName": "Alice",
            "age": 30
        });

        let result = validate_fixture(PERSON_MODEL_JSON, &instance, "org.example@1.0.0.Person");
        assert!(matches!(
            error_at_path(&result, "$.$class").error_type,
            ErrorKind::MissingRequiredProperty
        ));
    }
}

#[cfg(test)]
mod type_checking {
    use super::*;

    // pretty normal API bug. string where number should be
    #[test]
    fn integer_property_rejects_string() {
        let instance = json!({
            "$class": "org.example@1.0.0.Person",
            "email": "alice@example.com",
            "firstName": "Alice",
            "age": "thirty"
        });

        let result = validate_fixture(PERSON_MODEL_JSON, &instance, "org.example@1.0.0.Person");
        let age_error = error_at_path(&result, "$.age");

        assert!(matches!(
            &age_error.error_type,
            ErrorKind::TypeMismatch { expected, found }
                if expected == "Integer" && found == "String"
        ));
    }

    // turns out enums come through ObjectProperty in the metamodel. missed
    // that the first time and spent way too long wondering why this passed
    #[test]
    fn enum_property_rejects_unknown_value() {
        let instance = json!({
            "$class": "org.example@1.0.0.PersonWithGender",
            "email": "alice@example.com",
            "firstName": "Alice",
            "age": 25,
            "gender": "UNKNOWN"
        });

        let result = validate_fixture(
            PERSON_WITH_GENDER_MODEL_JSON,
            &instance,
            "org.example@1.0.0.PersonWithGender",
        );

        assert!(matches!(
            &error_at_path(&result, "$.gender").error_type,
            ErrorKind::InvalidEnumValue { valid_values }
                if valid_values.contains(&"MALE".to_string())
        ));
    }

    // easy to accidentally treat DateTime as just string and move on
    #[test]
    fn datetime_property_rejects_non_iso_string() {
        let instance = json!({
            "$class": "org.example@1.0.0.Person",
            "email": "alice@example.com",
            "firstName": "Alice",
            "age": 30,
            "dateOfBirth": "31-12-2000"
        });

        let result = validate_fixture(PERSON_MODEL_JSON, &instance, "org.example@1.0.0.Person");

        assert!(matches!(
            &error_at_path(&result, "$.dateOfBirth").error_type,
            ErrorKind::TypeMismatch { expected, .. } if expected == "DateTime (RFC 3339)"
        ));
    }

    // this feels wrong in rust terms, but JS only has one number type so the
    // upstream validator accepts it. matching that on purpose
    #[test]
    fn integer_json_value_accepted_for_double_property() {
        let instance = json!({
            "$class": "org.double@1.0.0.Measurement",
            "value": 30
        });

        let result = validate_fixture(DOUBLE_MODEL_JSON, &instance, "org.double@1.0.0.Measurement");
        assert!(result.valid, "this is deliberate JS compatibility");
    }
}

#[cfg(test)]
mod constraints {
    use super::*;

    // first constraint path i added after basic type checks
    #[test]
    fn age_out_of_range_reports_constraint_error() {
        let instance = json!({
            "$class": "org.example@1.0.0.Person",
            "email": "alice@example.com",
            "firstName": "Alice",
            "age": 200
        });

        let result = validate_fixture(PERSON_MODEL_JSON, &instance, "org.example@1.0.0.Person");
        assert!(matches!(
            &error_at_path(&result, "$.age").error_type,
            ErrorKind::ConstraintViolation { constraint } if constraint.contains("150")
        ));
    }

    // regex path plus nicer user msg. also exercises the cache
    #[test]
    fn regex_constraint_failure_keeps_pattern() {
        let instance = json!({
            "$class": "org.example@1.0.0.Contact",
            "email": "not-an-email"
        });

        let result = validate_fixture(CONTACT_MODEL_JSON, &instance, "org.example@1.0.0.Contact");
        assert!(matches!(
            &error_at_path(&result, "$.email").error_type,
            ErrorKind::PatternMismatch { pattern } if pattern.contains("[^@]+@[^@]+")
        ));
    }

    // optional + validated is the kind of combo that quietly breaks if you
    // only test one branch
    #[test]
    fn optional_property_constraint_is_checked_only_when_present() {
        let model_manager = model_manager_from_json(OPTIONAL_CONSTRAINT_MODEL_JSON);

        let absent_instance = json!({
            "$class": "org.example@1.0.0.Profile",
            "username": "alice"
        });
        let absent_result = validate_with_manager(
            &model_manager,
            &absent_instance,
            "org.example@1.0.0.Profile",
        );
        assert!(absent_result.valid);

        let invalid_instance = json!({
            "$class": "org.example@1.0.0.Profile",
            "username": "alice",
            "age": 200
        });
        let invalid_result = validate_with_manager(
            &model_manager,
            &invalid_instance,
            "org.example@1.0.0.Profile",
        );

        assert!(matches!(
            &error_at_path(&invalid_result, "$.age").error_type,
            ErrorKind::ConstraintViolation { constraint } if constraint.contains("150")
        ));
    }
}

#[cfg(test)]
mod inheritance {
    use super::*;

    // inheritance ate most of a day tbh
    #[test]
    fn child_instance_inherits_parent_properties() {
        let instance = json!({
            "$class": "org.example@1.0.0.Employee",
            "email": "bob@company.com",
            "firstName": "Bob",
            "age": 35,
            "employeeId": "E001"
        });

        let result = validate_fixture(EMPLOYEE_MODEL_JSON, &instance, "org.example@1.0.0.Employee");
        assert!(result.valid);
    }

    // older draft only walked one level up. this caught that immediately
    #[test]
    fn inherited_required_property_still_has_to_exist() {
        let instance = json!({
            "$class": "org.example@1.0.0.Employee",
            "firstName": "Bob",
            "age": 35,
            "employeeId": "E001"
        });

        let result = validate_fixture(EMPLOYEE_MODEL_JSON, &instance, "org.example@1.0.0.Employee");
        assert!(matches!(
            error_at_path(&result, "$.email").error_type,
            ErrorKind::MissingRequiredProperty
        ));
    }

    // this is the actual polymorphic path i care about for the proposal --
    // caller asks for parent type, payload says child type
    #[test]
    fn child_instance_can_be_checked_as_parent_type() {
        let instance = json!({
            "$class": "org.example@1.0.0.Employee",
            "email": "bob@company.com",
            "firstName": "Bob",
            "age": 35,
            "employeeId": "E001"
        });

        let result = validate_fixture(EMPLOYEE_MODEL_JSON, &instance, "org.example@1.0.0.Person");
        assert!(
            result.valid,
            "child instance should be assignable to parent: {:?}",
            result.errors
        );
    }
}

#[cfg(test)]
mod arrays {
    use super::*;

    // yes this really happened. only array[0] got checked in one draft
    #[test]
    fn every_array_element_gets_checked() {
        let instance = json!({
            "$class": "org.arrays@1.0.0.TaggedPerson",
            "name": "Alice",
            "scores": [95, 101, -1]
        });

        let result = validate_fixture(ARRAY_MODEL_JSON, &instance, "org.arrays@1.0.0.TaggedPerson");

        assert!(matches!(
            error_at_path(&result, "$.scores[1]").error_type,
            ErrorKind::ConstraintViolation { .. }
        ));
        assert!(matches!(
            error_at_path(&result, "$.scores[2]").error_type,
            ErrorKind::ConstraintViolation { .. }
        ));
    }

    // fail fast on scalar-vs-array mismatch before touching per-item checks
    #[test]
    fn scalar_value_rejected_for_array_property() {
        let instance = json!({
            "$class": "org.arrays@1.0.0.TaggedPerson",
            "name": "Alice",
            "scores": 99
        });

        let result = validate_fixture(ARRAY_MODEL_JSON, &instance, "org.arrays@1.0.0.TaggedPerson");
        assert!(matches!(
            &error_at_path(&result, "$.scores").error_type,
            ErrorKind::TypeMismatch { expected, found }
                if expected == "Array" && found == "Integer"
        ));
    }
}

#[cfg(test)]
mod edge_cases {
    use super::*;

    // some older fixtures still do org.example.Person even when the model is
    // versioned. both should work or this feels random
    #[test]
    fn versioned_and_unversioned_class_names_both_work() {
        let model_manager = model_manager_from_json(PERSON_MODEL_JSON);

        let versioned_instance = json!({
            "$class": "org.example@1.0.0.Person",
            "email": "alice@example.com",
            "firstName": "Alice",
            "age": 30
        });
        assert!(
            validate_with_manager(
                &model_manager,
                &versioned_instance,
                "org.example@1.0.0.Person"
            )
            .valid
        );

        let unversioned_instance = json!({
            "$class": "org.example.Person",
            "email": "alice@example.com",
            "firstName": "Alice",
            "age": 30
        });
        assert!(
            validate_with_manager(
                &model_manager,
                &unversioned_instance,
                "org.example@1.0.0.Person"
            )
            .valid
        );
    }

    // strict for now. @openapi changes this later, not in scope yet
    #[test]
    fn unknown_property_is_rejected_at_its_own_path() {
        let instance = json!({
            "$class": "org.example@1.0.0.Person",
            "email": "alice@example.com",
            "firstName": "Alice",
            "age": 30,
            "nickname": "Al"
        });

        let result = validate_fixture(PERSON_MODEL_JSON, &instance, "org.example@1.0.0.Person");
        assert!(matches!(
            error_at_path(&result, "$.nickname").error_type,
            ErrorKind::UnknownProperty
        ));
    }
}

#[cfg(test)]
mod advanced_semantics {
    use super::*;

    // first import test that felt like a real integration check, not just unit stuff
    #[test]
    fn imported_type_resolves_from_import_table() {
        let model_manager =
            model_manager_from_many(&[COMMON_ADDRESS_MODEL_JSON, CUSTOMER_MODEL_JSON]);

        let instance = json!({
            "$class": "org.customer@1.0.0.Customer",
            "name": "Alice",
            "address": {
                "$class": "org.common@1.0.0.PostalAddress",
                "street": "42 Market Street"
            }
        });

        let result =
            validate_with_manager(&model_manager, &instance, "org.customer@1.0.0.Customer");
        assert!(
            result.valid,
            "imported type should resolve: {:?}",
            result.errors
        );
    }

    // abstract rejection came in later so it gets an explicit test
    #[test]
    fn abstract_type_cannot_be_instantiated() {
        let instance = json!({
            "$class": "org.abstracts@1.0.0.BaseRecord",
            "id": "A-1"
        });

        let result = validate_fixture(
            ABSTRACT_MODEL_JSON,
            &instance,
            "org.abstracts@1.0.0.BaseRecord",
        );
        assert!(matches!(
            error_at_path(&result, "$.$class").error_type,
            ErrorKind::AbstractTypeInstantiation
        ));
    }

    // nasty bug: validator trusted $class too much and mostly ignored the
    // requested type. this keeps that from sneaking back in
    #[test]
    fn unrelated_requested_type_is_rejected_even_if_instance_class_is_valid() {
        let model_manager =
            model_manager_from_many(&[EMPLOYEE_MODEL_JSON, UNRELATED_TYPE_MODEL_JSON]);

        let instance = json!({
            "$class": "org.example@1.0.0.Employee",
            "email": "bob@company.com",
            "firstName": "Bob",
            "age": 35,
            "employeeId": "E001"
        });

        let result = validate_with_manager(&model_manager, &instance, "org.other@1.0.0.Invoice");
        assert!(matches!(
            &error_at_path(&result, "$.$class").error_type,
            ErrorKind::TypeMismatch { expected, found }
                if expected == "org.other@1.0.0.Invoice" && found == "org.example@1.0.0.Employee"
        ));
    }

    #[test]
    fn missing_requested_type_stays_a_hard_error() {
        let model_manager = model_manager_from_json(PERSON_MODEL_JSON);
        let instance = json!({
            "$class": "org.example@1.0.0.Person",
            "email": "alice@example.com",
            "firstName": "Alice",
            "age": 30
        });

        let error = model_manager
            .validate_instance(&instance, "org.example@1.0.0.Missing")
            .expect_err("missing requested type should stay a hard error");

        assert!(matches!(error, ConcertoError::TypeNotFound(_)));
    }

    // relationship strings are very stringly typed... wanted at least one test
    // that checks target type and not just URI shape
    #[test]
    fn relationship_target_type_is_checked() {
        let instance = json!({
            "$class": "org.fleet@1.0.0.Assignment",
            "vehicle": "resource:org.fleet@1.0.0.Driver#D-1"
        });

        let result = validate_fixture(
            RELATIONSHIP_MODEL_JSON,
            &instance,
            "org.fleet@1.0.0.Assignment",
        );
        assert!(matches!(
            error_at_path(&result, "$.vehicle").error_type,
            ErrorKind::InvalidRelationship
        ));
    }

    // scalar decls are basically named validators. easy to parse and then
    // accidentally never use
    #[test]
    fn scalar_constraints_are_enforced() {
        let instance = json!({
            "$class": "org.scalar@1.0.0.Parcel",
            "postalCode": "12A4"
        });

        let result = validate_fixture(SCALAR_MODEL_JSON, &instance, "org.scalar@1.0.0.Parcel");
        assert!(matches!(
            &error_at_path(&result, "$.postalCode").error_type,
            ErrorKind::PatternMismatch { .. } | ErrorKind::ConstraintViolation { .. }
        ));
    }

    // map support came in late. ugly fixture on purpose -- hits bad key and bad value
    #[test]
    fn map_validation_checks_both_key_and_value() {
        let instance = json!({
            "$class": "org.map@1.0.0.Inspection",
            "checks": {
                "not-a-datetime": true,
                "2026-03-30T12:00:00Z": "yes"
            }
        });

        let result = validate_fixture(MAP_MODEL_JSON, &instance, "org.map@1.0.0.Inspection");
        assert!(matches!(
            error_at_path(&result, "$.checks[\"not-a-datetime\"]").error_type,
            ErrorKind::TypeMismatch { .. }
        ));
        assert!(matches!(
            error_at_path(&result, "$.checks.2026-03-30T12:00:00Z").error_type,
            ErrorKind::TypeMismatch { .. }
        ));
    }

    // TODO(week 3 notes): split out a tiny abstract fixture again
    // current one works, just noisier than i want
    // #[test] fn abstract_concept_cannot_be_instantiated_at_class_path() {}

    // TODO(week 5 notes): add nested cross-namespace object regression
    // kept putting this off because the fixture json was getting unreadable
    // #[test] fn nested_object_property_resolves_across_namespaces() {}
}
