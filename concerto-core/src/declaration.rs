//! Parsed declaration shapes.
//!
//! The metamodel JSON is pretty noisy. After loading it once, I only really
//! need the shapes the validator cares about: class-like declarations, enums,
//! scalars, and maps.

use crate::property::Property;
use crate::property::PropertyType;
use std::collections::HashMap;

/// One parsed Concerto declaration.
///
/// By the time values reach this enum, the loader has already turned the raw
/// metamodel JSON into something the validator can match on directly.
#[derive(Debug, Clone, PartialEq)]
pub enum Declaration {
    /// A `concept` declaration.
    Concept(ClassDeclaration),
    /// An `asset` declaration.
    Asset(ClassDeclaration),
    /// A `participant` declaration.
    Participant(ClassDeclaration),
    /// A `transaction` declaration.
    Transaction(ClassDeclaration),
    /// An `event` declaration.
    Event(ClassDeclaration),
    /// An `enum` declaration.
    Enum(EnumDeclaration),
    /// A scalar declaration.
    Scalar(ScalarDeclaration),
    /// A map declaration.
    Map(MapDeclaration),
}

impl Declaration {
    /// Returns the short declaration name.
    ///
    /// Example: `"Person"` instead of `"org.example@1.0.0.Person"`.
    pub fn name(&self) -> &str {
        match self {
            Declaration::Concept(class_decl)
            | Declaration::Asset(class_decl)
            | Declaration::Participant(class_decl)
            | Declaration::Transaction(class_decl)
            | Declaration::Event(class_decl) => &class_decl.name,
            Declaration::Enum(enum_decl) => &enum_decl.name,
            Declaration::Scalar(scalar_decl) => &scalar_decl.name,
            Declaration::Map(map_decl) => &map_decl.name,
        }
    }

    /// Returns the namespace this declaration belongs to.
    pub fn namespace(&self) -> &str {
        match self {
            Declaration::Concept(class_decl)
            | Declaration::Asset(class_decl)
            | Declaration::Participant(class_decl)
            | Declaration::Transaction(class_decl)
            | Declaration::Event(class_decl) => &class_decl.namespace,
            Declaration::Enum(enum_decl) => &enum_decl.namespace,
            Declaration::Scalar(scalar_decl) => &scalar_decl.namespace,
            Declaration::Map(map_decl) => &map_decl.namespace,
        }
    }
}

/// Shared data for concept-like declarations.
///
/// Concept, asset, participant, transaction, and event all ended up sharing
/// the same shape here, which made the validator much less repetitive.
#[derive(Debug, Clone, PartialEq)]
pub struct ClassDeclaration {
    /// Short type name, like `"Person"`.
    pub name: String,
    /// Namespace that owns the type.
    pub namespace: String,
    /// Whether the type is abstract.
    pub is_abstract: bool,
    /// Supertype name, if one exists.
    pub super_type: Option<String>,
    /// Properties declared directly on this type.
    pub properties: HashMap<String, Property>,
}

/// Enum declaration with a fixed set of values.
#[derive(Debug, Clone, PartialEq)]
pub struct EnumDeclaration {
    /// Short enum name.
    pub name: String,
    /// Namespace that owns the enum.
    pub namespace: String,
    /// Valid enum values in declaration order.
    pub values: Vec<String>,
}

/// Scalar declaration.
///
/// This is basically a named primitive rule, like a reusable validated string.
#[derive(Debug, Clone, PartialEq)]
pub struct ScalarDeclaration {
    /// Short scalar name.
    pub name: String,
    /// Namespace that owns the scalar.
    pub namespace: String,
    /// Underlying scalar rule.
    pub scalar_type: PropertyType,
}

/// Map declaration.
///
/// Concerto maps are a bit odd because JSON object keys are always strings, so
/// key validation has its own little corner in the validator.
#[derive(Debug, Clone, PartialEq)]
pub struct MapDeclaration {
    /// Short map name.
    pub name: String,
    /// Namespace that owns the map.
    pub namespace: String,
    /// Key type rule.
    pub key_type: PropertyType,
    /// Value type rule.
    pub value_type: PropertyType,
}
