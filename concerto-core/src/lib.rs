//! Core crate for the Concerto Rust POC.
//!
//! This is the part that actually matters. Load model JSON, resolve types,
//! validate instances, return useful errors.
//!
//! Scope stayed intentionally small here. I did not try to rewrite all of
//! Concerto at once. That would have turned into six half-finished things
//! instead of one validator path that really works.
//!
//! # Quick start
//!
//! ```no_run
//! use concerto_core::ModelManager;
//! use serde_json::json;
//!
//! fn main() -> Result<(), concerto_core::ConcertoError> {
//!     let mut mm = ModelManager::new();
//!     mm.add_model_from_json(r#"
//!       {
//!         "$class": "concerto.metamodel@1.0.0.Model",
//!         "namespace": "org.example@1.0.0",
//!         "declarations": [
//!           {
//!             "$class": "concerto.metamodel@1.0.0.ConceptDeclaration",
//!             "name": "Person",
//!             "isAbstract": false,
//!             "properties": [
//!               {"$class":"concerto.metamodel@1.0.0.StringProperty","name":"email","isArray":false,"isOptional":false}
//!             ]
//!           }
//!         ]
//!       }
//!     "#)?;
//!
//!     let instance = json!({
//!         "$class": "org.example@1.0.0.Person",
//!         "email": "alice@example.com"
//!     });
//!
//!     let result = mm.validate_instance(&instance, "org.example@1.0.0.Person")?;
//!     assert!(result.valid);
//!     Ok(())
//! }
//! ```

#![deny(missing_docs)]

pub mod declaration;
pub mod error;
pub mod model_file;
pub mod model_manager;
mod parser;
pub mod property;
mod validator;

// re-exported bc the wrapper crates really do not need to care about the
// internal module tree here
pub use error::ConcertoError;
pub use model_manager::ModelManager;
pub use validator::instance_validator::{ErrorKind, ValidationError, ValidationResult};
