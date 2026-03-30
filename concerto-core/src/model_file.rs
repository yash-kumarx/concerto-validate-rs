//! One loaded model namespace.
//!
//! This file stays intentionally dumb. It just holds declarations and imports
//! for one namespace and leaves the weird lookup rules to `ModelManager`.

use crate::declaration::Declaration;
use std::collections::HashMap;

/// Import data for one model file.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Imports {
    /// Explicit short-name to fully-qualified-name mappings.
    pub explicit: HashMap<String, String>,
    /// Wildcard imported namespaces.
    pub wildcard_namespaces: Vec<String>,
}

/// One parsed Concerto model file.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelFile {
    /// Namespace declared by the model.
    pub namespace: String,
    /// Recorded imports for the model.
    pub imports: Imports,
    /// Declarations keyed by short name.
    pub declarations: HashMap<String, Declaration>,
}

impl ModelFile {
    /// Creates an empty model file for one namespace.
    pub fn new(namespace: impl Into<String>) -> Self {
        ModelFile {
            namespace: namespace.into(),
            imports: Imports::default(),
            declarations: HashMap::new(),
        }
    }

    /// Adds or replaces one declaration.
    pub fn add_declaration(&mut self, decl: Declaration) {
        self.declarations.insert(decl.name().to_string(), decl);
    }

    /// Looks up one declaration by short name.
    pub fn get_declaration(&self, name: &str) -> Option<&Declaration> {
        self.declarations.get(name)
    }
}
