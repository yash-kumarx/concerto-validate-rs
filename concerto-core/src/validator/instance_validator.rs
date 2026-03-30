//! The instance validator.
//!
//! This file is basically the point of the whole repo. Take some JSON, pick a
//! Concerto type, then see if the JSON is actually a valid instance of that
//! type or if it is just pretending.
//!
//! The rough flow is simple enough: resolve `$class`, figure out what the full
//! property set is after inheritance, check missing fields, then do type and
//! constraint checks. turns out the annoying part is that none of those steps
//! really stays simple once versioned namespaces and nested object refs show up.
//!
//! Inheritance ate the most time here. First draft only looked one supertype
//! up. That was fine right until I tried a three-level chain and everything
//! fell apart. Had to redo it with recursive walking plus a visited set so
//! circular models don't spin forever.

use serde_json::Value;
use std::collections::{HashMap, HashSet};

use crate::declaration::Declaration;
use crate::error::ConcertoError;
use crate::model_manager::ModelManager;
use crate::property::{NumericValidator, Property, PropertyType, StringValidator};
use crate::validator::{numeric_validator, string_validator, type_resolver};

const CLASS_FIELD: &str = "$class";
const ROOT_PATH: &str = "$";
const RESOURCE_PREFIX: &str = "resource:";

/// Result of validating one JSON value against one Concerto type.
///
/// This keeps every collected validation error. It does not bail on the first
/// bad field -- matching the JS validator mattered here because people expect
/// to fix all the issues in one pass, not play error whack-a-mole.
#[derive(Debug, serde::Serialize)]
pub struct ValidationResult {
    /// Whether the instance passed every check.
    pub valid: bool,
    /// Collected validation errors in traversal order.
    pub errors: Vec<ValidationError>,
}

/// One validation error with its JSON path and category.
#[derive(Debug, serde::Serialize)]
pub struct ValidationError {
    /// Path to the bad value, using the same `$`-rooted style as the tests.
    pub path: String,
    /// Short message meant for CLI output, demos, and bindings.
    pub message: String,
    /// Structured error kind for callers that do not want to parse text.
    pub error_type: ErrorKind,
}

/// Kinds of validation failure the runtime can report.
#[derive(Debug, serde::Serialize)]
pub enum ErrorKind {
    /// A required field was missing.
    MissingRequiredProperty,
    /// A value had the wrong JSON type.
    TypeMismatch {
        /// The type the validator expected to see.
        expected: String,
        /// The type it actually saw.
        found: String,
    },
    /// A declared validator rejected the value.
    ConstraintViolation {
        /// Short label for the failed constraint.
        constraint: String,
    },
    /// The requested type or a referenced type could not be resolved.
    UnknownType,
    /// The instance had a field that is not declared on the type.
    UnknownProperty,
    /// A relationship string was malformed or pointed at the wrong thing.
    InvalidRelationship,
    /// A regex check failed.
    PatternMismatch {
        /// The regex pattern that failed.
        pattern: String,
    },
    /// An enum field got a string outside the declared set.
    InvalidEnumValue {
        /// Allowed enum literal names.
        valid_values: Vec<String>,
    },
    /// `$class` points at an abstract declaration.
    AbstractTypeInstantiation,
}

/// Validates a JSON value against a named Concerto type.
///
/// `type_name` should be fully qualified, like `"org.example@1.0.0.Person"`.
/// Unversioned `$class` values still work for old fixtures because that case
/// keeps showing up in Concerto examples and tbh the demo feels broken if we
/// reject them.
///
/// All validation errors are collected before returning.
///
/// # Errors
///
/// Returns `TypeNotFound` or `NamespaceNotFound` if the requested type cannot
/// be resolved before validation starts.
/// Returns `CircularDependency` if walking the supertype chain loops back on
/// itself.
pub fn validate_instance(
    model_manager: &ModelManager,
    instance: &Value,
    type_name: &str,
) -> Result<ValidationResult, ConcertoError> {
    let mut errors = Vec::new();
    validate_object(model_manager, instance, type_name, ROOT_PATH, &mut errors)?;

    Ok(ValidationResult {
        valid: errors.is_empty(),
        errors,
    })
}

fn validate_object(
    model_manager: &ModelManager,
    instance: &Value,
    type_name: &str,
    path: &str,
    errors: &mut Vec<ValidationError>,
) -> Result<(), ConcertoError> {
    if !instance.is_object() {
        errors.push(ValidationError {
            path: path.to_string(),
            message: format!("expected object, got {}", render_found_value(instance)),
            error_type: ErrorKind::TypeMismatch {
                expected: "Object".to_string(),
                found: json_type_name(instance).to_string(),
            },
        });
        return Ok(());
    }

    let Some((decl, _resolved_fqn)) =
        resolve_class(model_manager, instance, path, type_name, errors)?
    else {
        return Ok(());
    };

    let props = match collect_properties(model_manager, decl, errors) {
        Ok(props) => props,
        Err(error) => {
            errors.push(ValidationError {
                path: format!("{path}.{CLASS_FIELD}"),
                message: format!("couldn't collect props: {error}"),
                error_type: ErrorKind::UnknownType,
            });
            return Ok(());
        }
    };

    check_missing_properties(instance, &props, path, errors);
    check_unknown_properties(instance, &props, path, errors);
    validate_present_properties(model_manager, instance, &props, path, errors)?;

    Ok(())
}

fn resolve_class<'a>(
    model_manager: &'a ModelManager,
    instance: &Value,
    path: &str,
    type_name: &str,
    errors: &mut Vec<ValidationError>,
) -> Result<Option<(&'a Declaration, String)>, ConcertoError> {
    let class_name = match instance.get(CLASS_FIELD).and_then(Value::as_str) {
        Some(class_name) => class_name,
        None => {
            errors.push(ValidationError {
                path: format!("{path}.{CLASS_FIELD}"),
                message: format!("missing required field '{CLASS_FIELD}'"),
                error_type: ErrorKind::MissingRequiredProperty,
            });
            return Ok(None);
        }
    };

    // old fixtures still use org.example.Person and not org.example@1.0.0.Person
    // so we keep supporting both. otherwise a bunch of legit samples fail for no
    // good reason
    let actual_decl = match type_resolver::resolve(model_manager, class_name) {
        Ok(decl) => decl,
        Err(_) => {
            errors.push(ValidationError {
                path: format!("{path}.{CLASS_FIELD}"),
                message: format!("can't resolve '{class_name}'"),
                error_type: ErrorKind::UnknownType,
            });
            return Ok(None);
        }
    };

    let requested_decl = type_resolver::resolve(model_manager, type_name)?;
    let actual_fqn = format!("{}.{}", actual_decl.namespace(), actual_decl.name());
    let requested_fqn = format!("{}.{}", requested_decl.namespace(), requested_decl.name());

    let assignable = match is_assignable_to(model_manager, actual_decl, requested_decl) {
        Ok(assignable) => assignable,
        Err(error) => {
            errors.push(ValidationError {
                path: format!("{path}.{CLASS_FIELD}"),
                message: format!("supertype walk blew up: {error}"),
                error_type: ErrorKind::UnknownType,
            });
            return Ok(None);
        }
    };

    if !assignable {
        errors.push(ValidationError {
            path: format!("{path}.{CLASS_FIELD}"),
            message: format!("'{CLASS_FIELD}' is '{class_name}', expected '{requested_fqn}'"),
            error_type: ErrorKind::TypeMismatch {
                expected: requested_fqn,
                found: class_name.to_string(),
            },
        });
        return Ok(None);
    }

    if matches!(
        actual_decl,
        Declaration::Concept(class_decl)
            | Declaration::Asset(class_decl)
            | Declaration::Participant(class_decl)
            | Declaration::Transaction(class_decl)
            | Declaration::Event(class_decl)
            if class_decl.is_abstract
    ) {
        errors.push(ValidationError {
            path: format!("{path}.{CLASS_FIELD}"),
            message: format!("can't instantiate abstract type '{actual_fqn}'"),
            error_type: ErrorKind::AbstractTypeInstantiation,
        });
    }

    Ok(Some((actual_decl, actual_fqn)))
}

fn is_assignable_to(
    model_manager: &ModelManager,
    actual: &Declaration,
    requested: &Declaration,
) -> Result<bool, ConcertoError> {
    if actual.namespace() == requested.namespace() && actual.name() == requested.name() {
        return Ok(true);
    }

    let mut current = actual;
    let mut visited = HashSet::new();

    loop {
        let class_decl = match current {
            Declaration::Concept(class_decl)
            | Declaration::Asset(class_decl)
            | Declaration::Participant(class_decl)
            | Declaration::Transaction(class_decl)
            | Declaration::Event(class_decl) => class_decl,
            Declaration::Enum(_) | Declaration::Scalar(_) | Declaration::Map(_) => {
                return Ok(false)
            }
        };

        let current_fqn = format!("{}.{}", class_decl.namespace, class_decl.name);
        if !visited.insert(current_fqn.clone()) {
            return Err(ConcertoError::CircularDependency(current_fqn));
        }

        let Some(super_type) = &class_decl.super_type else {
            return Ok(false);
        };

        current =
            type_resolver::resolve_in_context(model_manager, super_type, &class_decl.namespace)?;
        if current.namespace() == requested.namespace() && current.name() == requested.name() {
            return Ok(true);
        }
    }
}

fn collect_properties<'a>(
    model_manager: &'a ModelManager,
    decl: &'a Declaration,
    errors: &mut Vec<ValidationError>,
) -> Result<HashMap<&'a str, &'a Property>, ConcertoError> {
    let mut visited = HashSet::new();
    collect_properties_inner(model_manager, decl, errors, &mut visited)
}

fn collect_properties_inner<'a>(
    model_manager: &'a ModelManager,
    decl: &'a Declaration,
    errors: &mut Vec<ValidationError>,
    visited: &mut HashSet<String>,
) -> Result<HashMap<&'a str, &'a Property>, ConcertoError> {
    let class_decl = match decl {
        Declaration::Concept(class_decl)
        | Declaration::Asset(class_decl)
        | Declaration::Participant(class_decl)
        | Declaration::Transaction(class_decl)
        | Declaration::Event(class_decl) => class_decl,
        Declaration::Enum(_) | Declaration::Scalar(_) | Declaration::Map(_) => {
            return Ok(HashMap::new());
        }
    };

    let current_fqn = format!("{}.{}", class_decl.namespace, class_decl.name);
    if visited.contains(&current_fqn) {
        return Err(ConcertoError::CircularDependency(current_fqn));
    }
    visited.insert(current_fqn);

    let mut props = HashMap::new();

    if let Some(super_type) = &class_decl.super_type {
        match type_resolver::resolve_in_context(model_manager, super_type, &class_decl.namespace) {
            Ok(parent_decl) => {
                let parent_props =
                    collect_properties_inner(model_manager, parent_decl, errors, visited)?;
                props.extend(parent_props);
            }
            Err(_) => {
                errors.push(ValidationError {
                    path: ROOT_PATH.to_string(),
                    message: format!("can't resolve super type '{super_type}'"),
                    error_type: ErrorKind::UnknownType,
                });
            }
        }
    }

    for (name, prop) in &class_decl.properties {
        // child wins on collisions. JS does the same thing and it makes sense
        // anyway -- closest declaration should be the one that sticks
        props.insert(name.as_str(), prop);
    }

    Ok(props)
}

fn check_missing_properties(
    instance: &Value,
    props: &HashMap<&str, &Property>,
    path: &str,
    errors: &mut Vec<ValidationError>,
) {
    for (name, prop) in props {
        if !prop.is_optional && instance.get(*name).is_none() {
            errors.push(ValidationError {
                path: format!("{path}.{name}"),
                message: format!("missing required field '{name}'"),
                error_type: ErrorKind::MissingRequiredProperty,
            });
        }
    }
}

fn check_unknown_properties(
    instance: &Value,
    props: &HashMap<&str, &Property>,
    path: &str,
    errors: &mut Vec<ValidationError>,
) {
    // not doing the @openapi relaxed case here yet -- for the poc the strict
    // rule is more useful and it matched most of the fixtures i was using
    if let Some(obj) = instance.as_object() {
        for key in obj.keys() {
            if key == CLASS_FIELD {
                continue;
            }

            if !props.contains_key(key.as_str()) {
                errors.push(ValidationError {
                    path: format!("{path}.{key}"),
                    message: format!("'{key}' isn't declared here"),
                    error_type: ErrorKind::UnknownProperty,
                });
            }
        }
    }
}

fn validate_present_properties(
    model_manager: &ModelManager,
    instance: &Value,
    props: &HashMap<&str, &Property>,
    path: &str,
    errors: &mut Vec<ValidationError>,
) -> Result<(), ConcertoError> {
    if let Some(obj) = instance.as_object() {
        for (name, val) in obj {
            if name == CLASS_FIELD {
                continue;
            }

            let Some(prop) = props.get(name.as_str()) else {
                continue;
            };

            if prop.is_optional && val.is_null() {
                continue;
            }

            // clone here because we need the path string in a couple places and
            // trying to reuse one buffer made this way harder to read
            let prop_path = format!("{path}.{name}");
            validate_property(model_manager, val, prop, &prop_path, errors)?;
        }
    }

    Ok(())
}

fn validate_property(
    model_manager: &ModelManager,
    val: &Value,
    prop: &Property,
    path: &str,
    errors: &mut Vec<ValidationError>,
) -> Result<(), ConcertoError> {
    if prop.is_array {
        match val.as_array() {
            Some(items) => {
                for (index, item) in items.iter().enumerate() {
                    let item_path = format!("{path}[{index}]");
                    validate_scalar_property(model_manager, item, prop, &item_path, errors)?;
                }
            }
            None => {
                errors.push(ValidationError {
                    path: path.to_string(),
                    message: format!(
                        "'{}' should be array, got {}",
                        prop.name,
                        render_found_value(val)
                    ),
                    error_type: ErrorKind::TypeMismatch {
                        expected: "Array".to_string(),
                        found: json_type_name(val).to_string(),
                    },
                });
            }
        }
    } else if val.is_array() {
        errors.push(ValidationError {
            path: path.to_string(),
            message: format!(
                "'{}' should be {}, got array",
                prop.name,
                prop.property_type.describe()
            ),
            error_type: ErrorKind::TypeMismatch {
                expected: prop.property_type.describe(),
                found: "Array".to_string(),
            },
        });
    } else {
        validate_scalar_property(model_manager, val, prop, path, errors)?;
    }

    Ok(())
}

fn validate_scalar_property(
    model_manager: &ModelManager,
    val: &Value,
    prop: &Property,
    path: &str,
    errors: &mut Vec<ValidationError>,
) -> Result<(), ConcertoError> {
    validate_type(
        model_manager,
        val,
        &prop.property_type,
        &prop.declaring_namespace,
        path,
        &prop.name,
        errors,
    )
}

fn validate_type(
    model_manager: &ModelManager,
    val: &Value,
    prop_type: &PropertyType,
    context_namespace: &str,
    path: &str,
    prop_name: &str,
    errors: &mut Vec<ValidationError>,
) -> Result<(), ConcertoError> {
    match prop_type {
        PropertyType::String(validator) => {
            validate_string_property(val, validator, path, prop_name, errors)
        }
        PropertyType::Boolean => validate_boolean_property(val, path, prop_name, errors),
        PropertyType::Integer(validator) => {
            validate_integerish_property(val, validator, path, prop_name, "Integer", errors)
        }
        PropertyType::Long(validator) => {
            validate_integerish_property(val, validator, path, prop_name, "Long", errors)
        }
        PropertyType::Double(validator) => {
            validate_double_property(val, validator, path, prop_name, errors)
        }
        PropertyType::DateTime => validate_datetime_property(val, path, prop_name, errors),
        PropertyType::Relationship { type_ref } => validate_relationship(
            model_manager,
            val,
            type_ref,
            context_namespace,
            path,
            prop_name,
            errors,
        )?,
        PropertyType::ObjectRef { type_ref } => validate_object_ref(
            model_manager,
            val,
            type_ref,
            context_namespace,
            path,
            prop_name,
            errors,
        )?,
    }

    Ok(())
}

fn validate_string_property(
    val: &Value,
    validator: &Option<StringValidator>,
    path: &str,
    prop_name: &str,
    errors: &mut Vec<ValidationError>,
) {
    match val.as_str() {
        Some(string_val) => {
            let mut string_errors =
                string_validator::validate_string(string_val, validator, path, prop_name);
            errors.append(&mut string_errors);
        }
        None => errors.push(type_mismatch(path, "String", val, prop_name)),
    }
}

fn validate_boolean_property(
    val: &Value,
    path: &str,
    prop_name: &str,
    errors: &mut Vec<ValidationError>,
) {
    if !val.is_boolean() {
        errors.push(type_mismatch(path, "Boolean", val, prop_name));
    }
}

fn validate_integerish_property(
    val: &Value,
    validator: &Option<NumericValidator<i64>>,
    path: &str,
    prop_name: &str,
    expected_type: &str,
    errors: &mut Vec<ValidationError>,
) {
    match val.as_i64() {
        Some(number) if val.is_i64() || is_integer_f64(val) => {
            let mut numeric_errors =
                numeric_validator::validate_numeric(number, validator, path, prop_name);
            errors.append(&mut numeric_errors);
        }
        _ => errors.push(type_mismatch(path, expected_type, val, prop_name)),
    }
}

fn validate_double_property(
    val: &Value,
    validator: &Option<NumericValidator<f64>>,
    path: &str,
    prop_name: &str,
    errors: &mut Vec<ValidationError>,
) {
    // i almost rejected integer json here, then remembered JS only has Number
    // and concerto-core accepts 30 for a Double field. so we match that
    match val.as_f64() {
        Some(number) => {
            let mut numeric_errors =
                numeric_validator::validate_numeric(number, validator, path, prop_name);
            errors.append(&mut numeric_errors);
        }
        None => errors.push(type_mismatch(path, "Double", val, prop_name)),
    }
}

fn validate_datetime_property(
    val: &Value,
    path: &str,
    prop_name: &str,
    errors: &mut Vec<ValidationError>,
) {
    match val.as_str() {
        Some(datetime) => {
            if chrono::DateTime::parse_from_rfc3339(datetime).is_err() {
                errors.push(ValidationError {
                    path: path.to_string(),
                    message: format!(
                        "'{}' should be rfc3339 datetime, got {:?}",
                        prop_name, datetime
                    ),
                    error_type: ErrorKind::TypeMismatch {
                        expected: "DateTime (RFC 3339)".to_string(),
                        found: format!("String ({datetime})"),
                    },
                });
            }
        }
        None => errors.push(type_mismatch(path, "DateTime", val, prop_name)),
    }
}

fn validate_relationship(
    model_manager: &ModelManager,
    val: &Value,
    type_ref: &str,
    context_namespace: &str,
    path: &str,
    prop_name: &str,
    errors: &mut Vec<ValidationError>,
) -> Result<(), ConcertoError> {
    let uri = match val.as_str() {
        Some(uri) => uri,
        None => {
            errors.push(ValidationError {
                path: path.to_string(),
                message: format!(
                    "'{}' should be relationship string, got {}",
                    prop_name,
                    render_found_value(val)
                ),
                error_type: ErrorKind::InvalidRelationship,
            });
            return Ok(());
        }
    };

    let declared_target =
        match type_resolver::resolve_in_context(model_manager, type_ref, context_namespace) {
            Ok(decl) => decl,
            Err(_) => {
                errors.push(ValidationError {
                    path: path.to_string(),
                    message: format!("relationship target '{}' isn't loaded", type_ref),
                    error_type: ErrorKind::UnknownType,
                });
                return Ok(());
            }
        };

    let actual_type = match parse_relationship_uri(uri) {
        Some(actual_type) => actual_type,
        None => {
            errors.push(ValidationError {
                path: path.to_string(),
                message: format!(
                    "'{}' should look like resource:Type#id, got {:?}",
                    prop_name, uri
                ),
                error_type: ErrorKind::InvalidRelationship,
            });
            return Ok(());
        }
    };

    let actual_target = match type_resolver::resolve(model_manager, &actual_type) {
        Ok(decl) => decl,
        Err(_) => {
            errors.push(ValidationError {
                path: path.to_string(),
                message: format!(
                    "'{}' points at '{}', but that type isn't loaded",
                    prop_name, actual_type
                ),
                error_type: ErrorKind::UnknownType,
            });
            return Ok(());
        }
    };

    if actual_target.namespace() != declared_target.namespace()
        || actual_target.name() != declared_target.name()
    {
        errors.push(ValidationError {
            path: path.to_string(),
            message: format!(
                "'{}' should point to '{}.{}', got '{}.{}'",
                prop_name,
                declared_target.namespace(),
                declared_target.name(),
                actual_target.namespace(),
                actual_target.name()
            ),
            error_type: ErrorKind::InvalidRelationship,
        });
    }

    Ok(())
}

fn validate_object_ref(
    model_manager: &ModelManager,
    val: &Value,
    type_ref: &str,
    context_namespace: &str,
    path: &str,
    prop_name: &str,
    errors: &mut Vec<ValidationError>,
) -> Result<(), ConcertoError> {
    let decl = match type_resolver::resolve_in_context(model_manager, type_ref, context_namespace) {
        Ok(decl) => decl,
        Err(_) => {
            errors.push(ValidationError {
                path: path.to_string(),
                message: format!("can't resolve '{}'", type_ref),
                error_type: ErrorKind::UnknownType,
            });
            return Ok(());
        }
    };

    match decl {
        Declaration::Enum(enum_decl) => match val.as_str() {
            Some(enum_val) => {
                if !enum_decl
                    .values
                    .iter()
                    .any(|candidate| candidate == enum_val)
                {
                    errors.push(ValidationError {
                        path: path.to_string(),
                        message: format!(
                            "'{}' should be one of {:?}, got {:?}",
                            prop_name, enum_decl.values, enum_val
                        ),
                        error_type: ErrorKind::InvalidEnumValue {
                            valid_values: enum_decl.values.clone(),
                        },
                    });
                }
            }
            None => errors.push(type_mismatch(path, "String (Enum)", val, prop_name)),
        },
        Declaration::Concept(_)
        | Declaration::Asset(_)
        | Declaration::Participant(_)
        | Declaration::Transaction(_)
        | Declaration::Event(_) => {
            if val.is_object() {
                let expected_fqn = format!("{}.{}", decl.namespace(), decl.name());
                validate_object(model_manager, val, &expected_fqn, path, errors)?;
            } else {
                errors.push(type_mismatch(path, "Object", val, prop_name));
            }
        }
        Declaration::Scalar(scalar_decl) => {
            validate_type(
                model_manager,
                val,
                &scalar_decl.scalar_type,
                &scalar_decl.namespace,
                path,
                prop_name,
                errors,
            )?;
        }
        Declaration::Map(map_decl) => {
            validate_map(model_manager, val, map_decl, path, prop_name, errors)?;
        }
    }

    Ok(())
}

fn validate_map(
    model_manager: &ModelManager,
    val: &Value,
    map_decl: &crate::declaration::MapDeclaration,
    path: &str,
    prop_name: &str,
    errors: &mut Vec<ValidationError>,
) -> Result<(), ConcertoError> {
    let obj = match val.as_object() {
        Some(obj) => obj,
        None => {
            errors.push(type_mismatch(path, "Object", val, prop_name));
            return Ok(());
        }
    };

    for (map_key, map_val) in obj {
        let key_path = format!("{path}[{map_key:?}]");
        validate_map_key(&map_decl.key_type, map_key, &key_path, prop_name, errors);
        validate_type(
            model_manager,
            map_val,
            &map_decl.value_type,
            &map_decl.namespace,
            &format!("{path}.{map_key}"),
            prop_name,
            errors,
        )?;
    }

    Ok(())
}

fn validate_map_key(
    key_type: &PropertyType,
    map_key: &str,
    path: &str,
    prop_name: &str,
    errors: &mut Vec<ValidationError>,
) {
    match key_type {
        PropertyType::String(validator) => {
            let mut key_errors =
                string_validator::validate_string(map_key, validator, path, prop_name);
            errors.append(&mut key_errors);
        }
        PropertyType::DateTime => {
            if chrono::DateTime::parse_from_rfc3339(map_key).is_err() {
                errors.push(ValidationError {
                    path: path.to_string(),
                    message: format!(
                        "map key for '{}' should be rfc3339 datetime, got {:?}",
                        prop_name, map_key
                    ),
                    error_type: ErrorKind::TypeMismatch {
                        expected: "DateTime (RFC 3339)".to_string(),
                        found: "String".to_string(),
                    },
                });
            }
        }
        _ => {
            errors.push(ValidationError {
                path: path.to_string(),
                message: format!(
                    "map '{}' has key type '{}' but json keys are strings",
                    prop_name,
                    key_type.describe()
                ),
                error_type: ErrorKind::TypeMismatch {
                    expected: "String-compatible key type".to_string(),
                    found: key_type.describe(),
                },
            });
        }
    }
}

fn parse_relationship_uri(uri: &str) -> Option<String> {
    let resource_part = uri.strip_prefix(RESOURCE_PREFIX)?;
    let (type_part, id_part) = resource_part.split_once('#')?;

    if type_part.is_empty() || id_part.is_empty() {
        None
    } else {
        Some(type_part.to_string())
    }
}

fn type_mismatch(
    path: &str,
    expected: &str,
    found_val: &Value,
    prop_name: &str,
) -> ValidationError {
    let found_type = json_type_name(found_val);
    ValidationError {
        path: path.to_string(),
        message: format!(
            "'{}' should be {}, got {}",
            prop_name,
            expected,
            render_found_value(found_val)
        ),
        error_type: ErrorKind::TypeMismatch {
            expected: expected.to_string(),
            found: found_type.to_string(),
        },
    }
}

fn render_found_value(value: &Value) -> String {
    match value {
        Value::String(string_val) => format!("string ({string_val:?})"),
        Value::Bool(bool_val) => format!("bool ({bool_val})"),
        Value::Number(number_val) => format!("{} ({number_val})", json_type_name(value)),
        Value::Null => "null".to_string(),
        Value::Array(_) => "array".to_string(),
        Value::Object(_) => "object".to_string(),
    }
}

fn json_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "Null",
        Value::Bool(_) => "Boolean",
        // serde_json::Number is a bit annoying here -- it doesn't really tell
        // you int vs float directly. spent way too long on this before checking
        // the docs and the Number helpers properly
        Value::Number(number) if number.is_f64() && !is_integer_f64(value) => "Double",
        Value::Number(_) => "Integer",
        Value::String(_) => "String",
        Value::Array(_) => "Array",
        Value::Object(_) => "Object",
    }
}

fn is_integer_f64(value: &Value) -> bool {
    if let Some(number) = value.as_f64() {
        (number.fract() == 0.0) && (number >= i64::MIN as f64) && (number <= i64::MAX as f64)
    } else {
        false
    }
}
