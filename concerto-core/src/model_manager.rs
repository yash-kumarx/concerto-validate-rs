//! Model manager -- keeps loaded namespaces around and answers lookup questions.
//!
//! I thought this would stay tiny. It did not. Once versioned namespaces,
//! imports, and "is this child type allowed where the parent was requested?"
//! all showed up, the clean place for that logic was here.
//!
//! So this ended up being the registry plus the lookup rules. The validator
//! leans on it pretty hard. That was the right call tbh -- better one place
//! knows the weird name-resolution rules than spreading them through three
//! files and trying to remember which one is authoritative later.
//!
//! One thing still a bit rough: some lookups are linear scans over loaded
//! models. Fine for the POC. If this grows into the real runtime I'd build a
//! type index at load time and stop rescanning everything on hot paths.

use serde_json::Value;
use std::collections::HashMap;

use crate::declaration::Declaration;
use crate::error::ConcertoError;
use crate::model_file::{Imports, ModelFile};
use crate::parser::json_loader;
use crate::validator::instance_validator::{self, ValidationResult};
use crate::validator::type_resolver;

/// Holds the loaded Concerto models for one validation session.
pub struct ModelManager {
    models: HashMap<String, ModelFile>,
}

impl ModelManager {
    /// Creates an empty model manager.
    ///
    /// Nothing fancy here, just an empty namespace map to start filling.
    pub fn new() -> Self {
        Self {
            models: HashMap::new(),
        }
    }

    /// Loads one Concerto metamodel JSON document.
    ///
    /// If the same namespace is loaded twice, the newer one wins. I kept that
    /// behavior simple on purpose because it made local CLI/demo iteration less
    /// annoying while I was swapping fixture files around.
    ///
    /// # Errors
    ///
    /// Returns `ConcertoError::Parse` or `ConcertoError::Semantic` if the JSON
    /// metamodel is malformed.
    pub fn add_model_from_json(&mut self, json: &str) -> Result<(), ConcertoError> {
        let model_file = json_loader::load_model_from_json(json)?;
        self.models.insert(model_file.namespace.clone(), model_file);
        Ok(())
    }

    /// Validates one JSON instance against a named Concerto type.
    ///
    /// Collects validation errors instead of stopping at the first one. That
    /// matches the JS validator and honestly makes the browser demo way more
    /// usable because you see the whole mess in one go.
    ///
    /// `type_name` should usually be fully qualified, like
    /// `"org.example@1.0.0.Person"`.
    ///
    /// # Errors
    ///
    /// Returns `ConcertoError::TypeNotFound`, `ConcertoError::NamespaceNotFound`,
    /// or `ConcertoError::CircularDependency` if type lookup fails before
    /// validation can even start.
    pub fn validate_instance(
        &self,
        instance: &Value,
        type_name: &str,
    ) -> Result<ValidationResult, ConcertoError> {
        instance_validator::validate_instance(self, instance, type_name)
    }

    /// Resolves a type name relative to one namespace, including imports.
    ///
    /// This is the path object properties use when they say `"PostalAddress"`
    /// instead of spelling out the full namespace inline.
    ///
    /// # Errors
    ///
    /// Returns `ConcertoError::TypeNotFound` if nothing visible matches,
    /// `ConcertoError::NamespaceNotFound` if `context_namespace` is not loaded,
    /// or `ConcertoError::Semantic` if wildcard imports make the name ambiguous.
    pub fn resolve_type_in_context(
        &self,
        type_name: &str,
        context_namespace: &str,
    ) -> Result<&Declaration, ConcertoError> {
        if type_resolver::split_fqn(type_name).is_some() {
            return self.resolve_type(type_name);
        }

        let Some(model_file) = self.get_model(context_namespace) else {
            return Err(ConcertoError::TypeNotFound(format!(
                "can't resolve '{type_name}' -- namespace '{context_namespace}' isn't loaded"
            )));
        };

        if let Some(local_decl) = model_file.get_declaration(type_name) {
            return Ok(local_decl);
        }

        if let Some(imported_fqn) = model_file.imports.explicit.get(type_name) {
            return self.resolve_type(imported_fqn);
        }

        // still doing a linear walk over wildcard imports here. for real big
        // model graphs this is probably the first lookup path I'd optimize.
        let mut matches = model_file
            .imports
            .wildcard_namespaces
            .iter()
            .filter_map(|ns| self.get_model(ns))
            .filter_map(|imported_model| imported_model.get_declaration(type_name))
            .collect::<Vec<_>>();

        if matches.len() == 1 {
            return Ok(matches.remove(0));
        }

        if matches.len() > 1 {
            return Err(ConcertoError::Semantic(format!(
                "'{type_name}' is ambiguous in '{context_namespace}'"
            )));
        }

        Err(ConcertoError::TypeNotFound(format!(
            "can't resolve '{type_name}' in '{context_namespace}'"
        )))
    }

    /// Resolves a fully-qualified type name to a loaded declaration.
    ///
    /// # Errors
    ///
    /// Returns `ConcertoError::TypeNotFound` if the name is malformed or the
    /// declaration does not exist, and `ConcertoError::NamespaceNotFound` if
    /// the namespace part has not been loaded.
    pub fn resolve_type(&self, fqn: &str) -> Result<&Declaration, ConcertoError> {
        let (namespace, short_name) = type_resolver::split_fqn(fqn)
            .ok_or_else(|| ConcertoError::TypeNotFound(format!("can't parse type '{fqn}'")))?;

        // took me a while to realize you can't borrow self in a closure here
        // while also hanging onto the split pieces above in a more convoluted
        // version. this plain lookup ended up clearer anyway
        let model_file = self.get_model(namespace).ok_or_else(|| {
            ConcertoError::NamespaceNotFound(format!("namespace '{namespace}' isn't loaded"))
        })?;

        model_file.get_declaration(short_name).ok_or_else(|| {
            ConcertoError::TypeNotFound(format!("type '{short_name}' not found in '{namespace}'"))
        })
    }

    /// Returns the imports recorded for one loaded namespace.
    ///
    /// Mostly useful in tests and debugging right now.
    pub fn imports_for(&self, namespace: &str) -> Option<&Imports> {
        self.get_model(namespace).map(|model| &model.imports)
    }

    /// Iterates over all loaded namespace strings.
    pub fn namespaces(&self) -> impl Iterator<Item = &str> {
        self.models.keys().map(String::as_str)
    }

    /// Iterates over every declaration from every loaded model.
    ///
    /// This is the blunt instrument version. Fine for CLI/info paths.
    pub fn all_declarations(&self) -> impl Iterator<Item = &Declaration> {
        self.models
            .values()
            .flat_map(|model| model.declarations.values())
    }

    fn get_model(&self, namespace: &str) -> Option<&ModelFile> {
        self.models
            .get(namespace)
            .or_else(|| self.find_versionless_match(namespace))
    }

    fn find_versionless_match(&self, namespace: &str) -> Option<&ModelFile> {
        if namespace.contains('@') {
            return None;
        }

        let mut matches = self
            .models
            .iter()
            .filter(|(loaded_ns, _)| {
                loaded_ns.split('@').next().unwrap_or(loaded_ns.as_str()) == namespace
            })
            .map(|(_, model)| model);

        let first = matches.next()?;

        // if more than one version matches the same versionless name, guessing
        // would be worse than failing. hit this while sketching future fixtures
        if matches.next().is_some() {
            None
        } else {
            Some(first)
        }
    }
}

impl Default for ModelManager {
    fn default() -> Self {
        Self::new()
    }
}
