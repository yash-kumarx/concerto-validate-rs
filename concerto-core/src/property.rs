//! Property metadata after model loading.
//!
//! Same idea as `declaration.rs`: take the chatty metamodel JSON shape and turn
//! it into something the validator can work with without re-parsing JSON nodes
//! over and over.

/// One property declared on a class-like type.
///
/// This is the flattened version after loading, not the raw metamodel JSON node.
#[derive(Debug, Clone, PartialEq)]
pub struct Property {
    /// Property name from the model.
    pub name: String,
    /// Namespace of the declaration that introduced this property.
    pub declaring_namespace: String,
    /// Whether the property can be absent.
    pub is_optional: bool,
    /// Whether the property is an array.
    pub is_array: bool,
    /// Semantic property type plus any attached validator.
    pub property_type: PropertyType,
}

/// Semantic type of a property.
///
/// Concerto has more source-level surface area than this, but these are the
/// variants the validator actually needs once parsing is done.
#[derive(Debug, Clone, PartialEq)]
pub enum PropertyType {
    /// String with optional validators.
    String(Option<StringValidator>),
    /// Boolean value.
    Boolean,
    /// Integer with optional range limits.
    Integer(Option<NumericValidator<i64>>),
    /// Long with optional range limits.
    Long(Option<NumericValidator<i64>>),
    /// Double with optional range limits.
    Double(Option<NumericValidator<f64>>),
    /// RFC 3339 date-time string.
    DateTime,
    /// Relationship URI that points at another declaration.
    Relationship {
        /// Referenced target type.
        type_ref: String,
    },
    /// Nested object reference.
    ObjectRef {
        /// Referenced target type.
        type_ref: String,
    },
}

impl PropertyType {
    /// Returns a short human-readable label used in diagnostics and CLI output.
    pub fn describe(&self) -> String {
        match self {
            PropertyType::String(_) => "String".to_string(),
            PropertyType::Boolean => "Boolean".to_string(),
            PropertyType::Integer(_) => "Integer".to_string(),
            PropertyType::Long(_) => "Long".to_string(),
            PropertyType::Double(_) => "Double".to_string(),
            PropertyType::DateTime => "DateTime".to_string(),
            PropertyType::Relationship { type_ref } => format!("Relationship<{type_ref}>"),
            PropertyType::ObjectRef { type_ref } => format!("Object<{type_ref}>"),
        }
    }
}

/// String-specific validation rules.
///
/// Regex and length live together here because they both end up attached to
/// the same string property in practice.
#[derive(Debug, Clone, PartialEq)]
pub struct StringValidator {
    /// Optional regex pattern.
    pub regex: Option<String>,
    /// Optional inclusive minimum length.
    pub min_length: Option<usize>,
    /// Optional inclusive maximum length.
    pub max_length: Option<usize>,
}

/// Numeric lower/upper bound validator.
///
/// Used for Integer, Long, and Double with inclusive bounds.
#[derive(Debug, Clone, PartialEq)]
pub struct NumericValidator<T> {
    /// Inclusive lower bound.
    pub lower: Option<T>,
    /// Inclusive upper bound.
    pub upper: Option<T>,
}
