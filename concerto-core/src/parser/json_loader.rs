//! Loads Concerto models from metamodel JSON.
//!
//! This is the practical path for the POC. CTO text parsing is still a stub,
//! but the JSON metamodel is enough to build and validate real fixtures, and
//! honestly it made iteration way faster because I could stay in `serde_json`
//! land instead of debugging a parser too.
//!
//! The annoying part here is the giant pile of `$class` strings. Every
//! declaration, property, import, validator, and map endpoint shows up with
//! one, so the loader has to decode them into Rust enums and structs.
//!
//! Deliberate choice: unknown declaration kinds get skipped with a warning.
//! Missing required fields are hard errors. That split felt right after testing
//! a few broken fixtures -- one bad extension type should not nuke the whole
//! model if the rest is still useful.

use serde_json::Value;
use std::collections::HashMap;

use crate::declaration::{
    ClassDeclaration, Declaration, EnumDeclaration, MapDeclaration, ScalarDeclaration,
};
use crate::error::ConcertoError;
use crate::model_file::{Imports, ModelFile};
use crate::property::{NumericValidator, Property, PropertyType, StringValidator};

const CLASS_FIELD: &str = "$class";
const MODEL_NAMESPACE_FIELD: &str = "namespace";
const DECLARATIONS_FIELD: &str = "declarations";
const IMPORTS_FIELD: &str = "imports";
const MODEL_NAME_FIELD: &str = "name";
const TYPES_FIELD: &str = "types";
const ALIASED_TYPES_FIELD: &str = "aliasedTypes";

// declarations
const CONCEPT_DECL: &str = "concerto.metamodel@1.0.0.ConceptDeclaration";
const ASSET_DECL: &str = "concerto.metamodel@1.0.0.AssetDeclaration";
const PARTICIPANT_DECL: &str = "concerto.metamodel@1.0.0.ParticipantDeclaration";
const TRANSACTION_DECL: &str = "concerto.metamodel@1.0.0.TransactionDeclaration";
const EVENT_DECL: &str = "concerto.metamodel@1.0.0.EventDeclaration";
const ENUM_DECL: &str = "concerto.metamodel@1.0.0.EnumDeclaration";
const MAP_DECL: &str = "concerto.metamodel@1.0.0.MapDeclaration";
const BOOLEAN_SCALAR_DECL: &str = "concerto.metamodel@1.0.0.BooleanScalar";
const INTEGER_SCALAR_DECL: &str = "concerto.metamodel@1.0.0.IntegerScalar";
const LONG_SCALAR_DECL: &str = "concerto.metamodel@1.0.0.LongScalar";
const DOUBLE_SCALAR_DECL: &str = "concerto.metamodel@1.0.0.DoubleScalar";
const STRING_SCALAR_DECL: &str = "concerto.metamodel@1.0.0.StringScalar";
const DATETIME_SCALAR_DECL: &str = "concerto.metamodel@1.0.0.DateTimeScalar";

// imports
const IMPORT_ALL_DECL: &str = "concerto.metamodel@1.0.0.ImportAll";
const IMPORT_TYPE_DECL: &str = "concerto.metamodel@1.0.0.ImportType";
const IMPORT_TYPES_DECL: &str = "concerto.metamodel@1.0.0.ImportTypes";

// properties
const STRING_PROP: &str = "concerto.metamodel@1.0.0.StringProperty";
const INTEGER_PROP: &str = "concerto.metamodel@1.0.0.IntegerProperty";
const LONG_PROP: &str = "concerto.metamodel@1.0.0.LongProperty";
const DOUBLE_PROP: &str = "concerto.metamodel@1.0.0.DoubleProperty";
const BOOLEAN_PROP: &str = "concerto.metamodel@1.0.0.BooleanProperty";
const DATETIME_PROP: &str = "concerto.metamodel@1.0.0.DateTimeProperty";
const OBJECT_PROP: &str = "concerto.metamodel@1.0.0.ObjectProperty";
const RELATIONSHIP_PROP: &str = "concerto.metamodel@1.0.0.RelationshipProperty";
const ENUM_PROP: &str = "concerto.metamodel@1.0.0.EnumProperty";

// validators
const STRING_REGEX_VALIDATOR: &str = "concerto.metamodel@1.0.0.StringRegexValidator";
const STRING_LENGTH_VALIDATOR: &str = "concerto.metamodel@1.0.0.StringLengthValidator";
const INTEGER_DOMAIN_VALIDATOR: &str = "concerto.metamodel@1.0.0.IntegerDomainValidator";
const LONG_DOMAIN_VALIDATOR: &str = "concerto.metamodel@1.0.0.LongDomainValidator";
const DOUBLE_DOMAIN_VALIDATOR: &str = "concerto.metamodel@1.0.0.DoubleDomainValidator";

// map endpoint types
const STRING_MAP_KEY_TYPE: &str = "concerto.metamodel@1.0.0.StringMapKeyType";
const DATETIME_MAP_KEY_TYPE: &str = "concerto.metamodel@1.0.0.DateTimeMapKeyType";
const OBJECT_MAP_KEY_TYPE: &str = "concerto.metamodel@1.0.0.ObjectMapKeyType";
const BOOLEAN_MAP_VALUE_TYPE: &str = "concerto.metamodel@1.0.0.BooleanMapValueType";
const DATETIME_MAP_VALUE_TYPE: &str = "concerto.metamodel@1.0.0.DateTimeMapValueType";
const STRING_MAP_VALUE_TYPE: &str = "concerto.metamodel@1.0.0.StringMapValueType";
const INTEGER_MAP_VALUE_TYPE: &str = "concerto.metamodel@1.0.0.IntegerMapValueType";
const LONG_MAP_VALUE_TYPE: &str = "concerto.metamodel@1.0.0.LongMapValueType";
const DOUBLE_MAP_VALUE_TYPE: &str = "concerto.metamodel@1.0.0.DoubleMapValueType";
const OBJECT_MAP_VALUE_TYPE: &str = "concerto.metamodel@1.0.0.ObjectMapValueType";
const RELATIONSHIP_MAP_VALUE_TYPE: &str = "concerto.metamodel@1.0.0.RelationshipMapValueType";

/// Parses one Concerto metamodel JSON string into a `ModelFile`.
///
/// Empty `declarations` arrays are allowed. Missing `namespace` is not.
///
/// # Errors
///
/// Returns `ConcertoError::Parse` for malformed JSON or missing required
/// fields, and `ConcertoError::Semantic` for things like invalid regexes or
/// impossible numeric ranges.
pub(crate) fn load_model_from_json(json: &str) -> Result<ModelFile, ConcertoError> {
    let root: Value = serde_json::from_str(json)?;

    let namespace = root
        .get(MODEL_NAMESPACE_FIELD)
        .and_then(Value::as_str)
        .ok_or_else(|| ConcertoError::Parse(format!("model is missing '{MODEL_NAMESPACE_FIELD}'")))?
        .to_string();

    let mut model = ModelFile::new(&namespace);
    model.imports = parse_imports(&root)?;

    if let Some(decls) = root.get(DECLARATIONS_FIELD).and_then(Value::as_array) {
        for (index, decl_val) in decls.iter().enumerate() {
            if let Some(decl) = parse_declaration(decl_val, &namespace, index)? {
                model.add_declaration(decl);
            }
        }
    }

    Ok(model)
}

fn parse_imports(root: &Value) -> Result<Imports, ConcertoError> {
    let mut imports = Imports::default();

    if let Some(entries) = root.get(IMPORTS_FIELD).and_then(Value::as_array) {
        for (index, entry) in entries.iter().enumerate() {
            let class = entry
                .get(CLASS_FIELD)
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    ConcertoError::Parse(format!("import #{index} is missing '{CLASS_FIELD}'"))
                })?;

            let namespace = entry
                .get(MODEL_NAMESPACE_FIELD)
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    ConcertoError::Parse(format!(
                        "import #{index} is missing '{MODEL_NAMESPACE_FIELD}'"
                    ))
                })?;

            match class {
                IMPORT_ALL_DECL => imports.wildcard_namespaces.push(namespace.to_string()),
                IMPORT_TYPE_DECL => {
                    let name = entry
                        .get(MODEL_NAME_FIELD)
                        .and_then(Value::as_str)
                        .ok_or_else(|| {
                            ConcertoError::Parse(format!(
                                "ImportType #{index} is missing '{MODEL_NAME_FIELD}'"
                            ))
                        })?;

                    imports
                        .explicit
                        .insert(name.to_string(), format!("{namespace}.{name}"));
                }
                IMPORT_TYPES_DECL => {
                    if let Some(types) = entry.get(TYPES_FIELD).and_then(Value::as_array) {
                        for ty in types.iter().filter_map(Value::as_str) {
                            imports
                                .explicit
                                .insert(ty.to_string(), format!("{namespace}.{ty}"));
                        }
                    }

                    if let Some(aliases) = entry.get(ALIASED_TYPES_FIELD).and_then(Value::as_array)
                    {
                        for alias in aliases {
                            let orig = alias
                                .get(MODEL_NAME_FIELD)
                                .and_then(Value::as_str)
                                .ok_or_else(|| {
                                    ConcertoError::Parse(
                                        "aliased type is missing 'name'".to_string(),
                                    )
                                })?;
                            let alias_name = alias
                                .get("aliasedName")
                                .and_then(Value::as_str)
                                .ok_or_else(|| {
                                    ConcertoError::Parse(
                                        "aliased type is missing 'aliasedName'".to_string(),
                                    )
                                })?;

                            imports
                                .explicit
                                .insert(alias_name.to_string(), format!("{namespace}.{orig}"));
                        }
                    }
                }
                other => {
                    return Err(ConcertoError::Parse(format!(
                        "unknown import $class '{other}'"
                    )));
                }
            }
        }
    }

    Ok(imports)
}

fn parse_declaration(
    val: &Value,
    namespace: &str,
    index: usize,
) -> Result<Option<Declaration>, ConcertoError> {
    let class = val
        .get(CLASS_FIELD)
        .and_then(Value::as_str)
        .ok_or_else(|| {
            ConcertoError::Parse(format!("declaration #{index} is missing '{CLASS_FIELD}'"))
        })?;

    match class {
        CONCEPT_DECL => Ok(Some(Declaration::Concept(parse_class_decl(
            val, namespace, index,
        )?))),
        ASSET_DECL => Ok(Some(Declaration::Asset(parse_class_decl(
            val, namespace, index,
        )?))),
        PARTICIPANT_DECL => Ok(Some(Declaration::Participant(parse_class_decl(
            val, namespace, index,
        )?))),
        TRANSACTION_DECL => Ok(Some(Declaration::Transaction(parse_class_decl(
            val, namespace, index,
        )?))),
        EVENT_DECL => Ok(Some(Declaration::Event(parse_class_decl(
            val, namespace, index,
        )?))),
        ENUM_DECL => Ok(Some(Declaration::Enum(parse_enum_decl(
            val, namespace, index,
        )?))),
        MAP_DECL => Ok(Some(Declaration::Map(parse_map_decl(
            val, namespace, index,
        )?))),
        BOOLEAN_SCALAR_DECL | INTEGER_SCALAR_DECL | LONG_SCALAR_DECL | DOUBLE_SCALAR_DECL
        | STRING_SCALAR_DECL | DATETIME_SCALAR_DECL => Ok(Some(Declaration::Scalar(
            parse_scalar_decl(val, namespace, index, class)?,
        ))),
        other => {
            log::warn!("skipping unknown declaration $class '{other}'");
            Ok(None)
        }
    }
}

fn parse_class_decl(
    val: &Value,
    namespace: &str,
    index: usize,
) -> Result<ClassDeclaration, ConcertoError> {
    let name = get_decl_name(val, index)?;
    let is_abstract = val
        .get("isAbstract")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let super_type = parse_type_name(val.get("superType"), namespace);

    let mut props = HashMap::new();
    if let Some(prop_vals) = val.get("properties").and_then(Value::as_array) {
        for prop_val in prop_vals {
            let prop = parse_property(prop_val, namespace, name)?;
            props.insert(prop.name.clone(), prop);
        }
    }

    Ok(ClassDeclaration {
        name: name.to_string(),
        namespace: namespace.to_string(),
        is_abstract,
        super_type,
        properties: props,
    })
}

fn parse_enum_decl(
    val: &Value,
    namespace: &str,
    index: usize,
) -> Result<EnumDeclaration, ConcertoError> {
    let name = get_decl_name(val, index)?;
    let mut values = Vec::new();

    if let Some(props) = val.get("properties").and_then(Value::as_array) {
        for p in props {
            values.push(get_prop_name(p, name)?.to_string());
        }
    }

    Ok(EnumDeclaration {
        name: name.to_string(),
        namespace: namespace.to_string(),
        values,
    })
}

fn parse_scalar_decl(
    val: &Value,
    namespace: &str,
    index: usize,
    class: &str,
) -> Result<ScalarDeclaration, ConcertoError> {
    let name = get_decl_name(val, index)?;

    let scalar_type = match class {
        BOOLEAN_SCALAR_DECL => PropertyType::Boolean,
        INTEGER_SCALAR_DECL => {
            PropertyType::Integer(parse_integer_validator_node(val.get("validator"))?)
        }
        LONG_SCALAR_DECL => PropertyType::Long(parse_integer_validator_node(val.get("validator"))?),
        DOUBLE_SCALAR_DECL => {
            PropertyType::Double(parse_double_validator_node(val.get("validator"))?)
        }
        STRING_SCALAR_DECL => PropertyType::String(parse_string_scalar_validator(val)?),
        DATETIME_SCALAR_DECL => PropertyType::DateTime,
        other => {
            return Err(ConcertoError::Parse(format!(
                "unknown scalar $class '{other}'"
            )))
        }
    };

    Ok(ScalarDeclaration {
        name: name.to_string(),
        namespace: namespace.to_string(),
        scalar_type,
    })
}

fn parse_map_decl(
    val: &Value,
    namespace: &str,
    index: usize,
) -> Result<MapDeclaration, ConcertoError> {
    let name = get_decl_name(val, index)?;
    let key_type = parse_map_endpoint(
        val.get("key")
            .ok_or_else(|| ConcertoError::Parse("map declaration is missing 'key'".to_string()))?,
        namespace,
    )?;
    let value_type = parse_map_endpoint(
        val.get("value").ok_or_else(|| {
            ConcertoError::Parse("map declaration is missing 'value'".to_string())
        })?,
        namespace,
    )?;

    Ok(MapDeclaration {
        name: name.to_string(),
        namespace: namespace.to_string(),
        key_type,
        value_type,
    })
}

fn parse_map_endpoint(val: &Value, default_namespace: &str) -> Result<PropertyType, ConcertoError> {
    match get_class(val)? {
        STRING_MAP_KEY_TYPE | STRING_MAP_VALUE_TYPE => Ok(PropertyType::String(None)),
        DATETIME_MAP_KEY_TYPE | DATETIME_MAP_VALUE_TYPE => Ok(PropertyType::DateTime),
        BOOLEAN_MAP_VALUE_TYPE => Ok(PropertyType::Boolean),
        INTEGER_MAP_VALUE_TYPE => Ok(PropertyType::Integer(None)),
        LONG_MAP_VALUE_TYPE => Ok(PropertyType::Long(None)),
        DOUBLE_MAP_VALUE_TYPE => Ok(PropertyType::Double(None)),
        OBJECT_MAP_KEY_TYPE | OBJECT_MAP_VALUE_TYPE => Ok(PropertyType::ObjectRef {
            type_ref: parse_type_name(val.get("type"), default_namespace).ok_or_else(|| {
                ConcertoError::Parse("object map endpoint is missing 'type'".to_string())
            })?,
        }),
        RELATIONSHIP_MAP_VALUE_TYPE => Ok(PropertyType::Relationship {
            type_ref: parse_type_name(val.get("type"), default_namespace).ok_or_else(|| {
                ConcertoError::Parse("relationship map endpoint is missing 'type'".to_string())
            })?,
        }),
        other => Err(ConcertoError::Parse(format!(
            "unsupported map endpoint $class '{other}'"
        ))),
    }
}

fn parse_property(
    val: &Value,
    namespace: &str,
    decl_name: &str,
) -> Result<Property, ConcertoError> {
    let class = val
        .get(CLASS_FIELD)
        .and_then(Value::as_str)
        .ok_or_else(|| {
            ConcertoError::Parse(format!(
                "property in '{decl_name}' is missing '{CLASS_FIELD}'"
            ))
        })?;
    let name = get_prop_name(val, decl_name)?;
    let is_optional = val
        .get("isOptional")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let is_array = val.get("isArray").and_then(Value::as_bool).unwrap_or(false);

    let property_type = match class {
        STRING_PROP => PropertyType::String(parse_string_validator_node(val.get("validator"))?),
        BOOLEAN_PROP => PropertyType::Boolean,
        INTEGER_PROP => PropertyType::Integer(parse_integer_validator_node(val.get("validator"))?),
        LONG_PROP => PropertyType::Long(parse_integer_validator_node(val.get("validator"))?),
        DOUBLE_PROP => PropertyType::Double(parse_double_validator_node(val.get("validator"))?),
        DATETIME_PROP => PropertyType::DateTime,
        OBJECT_PROP => PropertyType::ObjectRef {
            type_ref: parse_type_name(val.get("type"), namespace).ok_or_else(|| {
                ConcertoError::Parse(format!(
                    "object property '{name}' in '{decl_name}' is missing 'type'"
                ))
            })?,
        },
        RELATIONSHIP_PROP => PropertyType::Relationship {
            type_ref: parse_type_name(val.get("type"), namespace).ok_or_else(|| {
                ConcertoError::Parse(format!(
                    "relationship property '{name}' in '{decl_name}' is missing 'type'"
                ))
            })?,
        },
        // EnumProperty is actually how enum VALUES are declared inside an
        // EnumDeclaration -- so it shouldn't appear in a concept's properties
        // at all. but some older models send it here anyway (concerto-core JS
        // accepts it silently). if there's a type field, treat it like
        // ObjectProperty and the enum membership check comes for free.
        // if there's no type field, we can't check membership so warn and
        // fall back to unconstrained string -- at least it won't explode
        ENUM_PROP => match parse_type_name(val.get("type"), namespace) {
            Some(type_ref) => PropertyType::ObjectRef { type_ref },
            None => {
                log::warn!(
                    "EnumProperty '{}' in '{}' has no 'type' field -- enum membership won't be validated",
                    name, decl_name
                );
                PropertyType::String(None)
            }
        },
        other => {
            return Err(ConcertoError::Parse(format!(
                "unknown property $class '{other}'"
            )))
        }
    };

    Ok(Property {
        name: name.to_string(),
        declaring_namespace: namespace.to_string(),
        is_optional,
        is_array,
        property_type,
    })
}

fn parse_type_name(type_node: Option<&Value>, default_namespace: &str) -> Option<String> {
    let type_id = type_node?;
    let name = type_id.get("name").and_then(Value::as_str)?;

    match type_id.get("namespace").and_then(Value::as_str) {
        Some(namespace) if !namespace.is_empty() => Some(format!("{namespace}.{name}")),
        // keeping local refs as-is means import resolution stays in ModelManager
        // instead of getting half-done here and half-done there
        _ => {
            let _ = default_namespace;
            Some(name.to_string())
        }
    }
}

fn parse_string_validator_node(
    validator_node: Option<&Value>,
) -> Result<Option<StringValidator>, ConcertoError> {
    let validator = match validator_node {
        Some(v) => v,
        None => return Ok(None),
    };

    let class = validator
        .get(CLASS_FIELD)
        .and_then(Value::as_str)
        .ok_or_else(|| ConcertoError::Parse("validator is missing '$class'".to_string()))?;

    match class {
        STRING_REGEX_VALIDATOR => {
            let pattern = validator
                .get("pattern")
                .and_then(Value::as_str)
                .map(str::to_string)
                .ok_or_else(|| {
                    ConcertoError::Parse("StringRegexValidator is missing 'pattern'".to_string())
                })?;

            regex::Regex::new(&pattern).map_err(|error| {
                ConcertoError::Semantic(format!("bad regex '{pattern}': {error}"))
            })?;

            Ok(Some(StringValidator {
                regex: Some(pattern),
                min_length: None,
                max_length: None,
            }))
        }
        STRING_LENGTH_VALIDATOR => {
            let min_length = validator
                .get("minLength")
                .and_then(Value::as_u64)
                .map(|value| value as usize);
            let max_length = validator
                .get("maxLength")
                .and_then(Value::as_u64)
                .map(|value| value as usize);

            if min_length.is_none() && max_length.is_none() {
                return Err(ConcertoError::Parse(
                    "StringLengthValidator needs 'minLength', 'maxLength', or both".to_string(),
                ));
            }

            Ok(Some(StringValidator {
                regex: None,
                min_length,
                max_length,
            }))
        }
        other => Err(ConcertoError::Parse(format!(
            "unsupported string validator $class '{other}'"
        ))),
    }
}

fn parse_string_scalar_validator(root: &Value) -> Result<Option<StringValidator>, ConcertoError> {
    let regex_validator = parse_string_validator_node(root.get("validator"))?;
    let length_validator = parse_string_validator_node(root.get("lengthValidator"))?;

    let regex = regex_validator.and_then(|v| v.regex);
    let min_length = length_validator.as_ref().and_then(|v| v.min_length);
    let max_length = length_validator.as_ref().and_then(|v| v.max_length);

    if regex.is_none() && min_length.is_none() && max_length.is_none() {
        Ok(None)
    } else {
        Ok(Some(StringValidator {
            regex,
            min_length,
            max_length,
        }))
    }
}

fn parse_integer_validator_node(
    validator_node: Option<&Value>,
) -> Result<Option<NumericValidator<i64>>, ConcertoError> {
    let validator = match validator_node {
        Some(v) => v,
        None => return Ok(None),
    };

    let class = validator
        .get(CLASS_FIELD)
        .and_then(Value::as_str)
        .ok_or_else(|| ConcertoError::Parse("validator is missing '$class'".to_string()))?;

    if class != INTEGER_DOMAIN_VALIDATOR && class != LONG_DOMAIN_VALIDATOR {
        return Err(ConcertoError::Parse(format!(
            "unsupported integer validator $class '{class}'"
        )));
    }

    let lower = validator.get("lower").and_then(Value::as_i64);
    let upper = validator.get("upper").and_then(Value::as_i64);
    validate_numeric_bounds(lower, upper, "integer")?;
    Ok(Some(NumericValidator { lower, upper }))
}

fn parse_double_validator_node(
    validator_node: Option<&Value>,
) -> Result<Option<NumericValidator<f64>>, ConcertoError> {
    let validator = match validator_node {
        Some(v) => v,
        None => return Ok(None),
    };

    let class = validator
        .get(CLASS_FIELD)
        .and_then(Value::as_str)
        .ok_or_else(|| ConcertoError::Parse("validator is missing '$class'".to_string()))?;

    if class != DOUBLE_DOMAIN_VALIDATOR {
        return Err(ConcertoError::Parse(format!(
            "unsupported double validator $class '{class}'"
        )));
    }

    let lower = validator.get("lower").and_then(Value::as_f64);
    let upper = validator.get("upper").and_then(Value::as_f64);
    validate_numeric_bounds(lower, upper, "double")?;
    Ok(Some(NumericValidator { lower, upper }))
}

fn validate_numeric_bounds<T>(
    lower: Option<T>,
    upper: Option<T>,
    label: &str,
) -> Result<(), ConcertoError>
where
    T: PartialOrd + std::fmt::Display,
{
    if let (Some(lower), Some(upper)) = (lower, upper) {
        if lower > upper {
            return Err(ConcertoError::Semantic(format!(
                "{label} range is backwards: {lower} > {upper}"
            )));
        }
    }

    Ok(())
}

fn get_class(val: &Value) -> Result<&str, ConcertoError> {
    val.get(CLASS_FIELD)
        .and_then(Value::as_str)
        .ok_or_else(|| ConcertoError::Parse("JSON node is missing '$class'".to_string()))
}

fn get_decl_name(val: &Value, index: usize) -> Result<&str, ConcertoError> {
    val.get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| ConcertoError::Parse(format!("declaration #{index} is missing 'name'")))
}

fn get_prop_name<'a>(val: &'a Value, decl_name: &str) -> Result<&'a str, ConcertoError> {
    val.get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| ConcertoError::Parse(format!("property in '{decl_name}' is missing 'name'")))
}
